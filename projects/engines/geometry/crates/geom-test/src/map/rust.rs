//! Rust source as tokens, for the map: enough of the language's lexical grammar to find items
//! without being misled by comments, strings or character literals.
//!
//! What it does: [`tokens`] splits a source file into [`Token`]s: identifiers, punctuation,
//! literals, lifetimes and outer doc comments (`///` and `/** */`), each with its text and byte
//! offset. Plain comments, inner doc comments (`//!`, `/*! */`) and whitespace are dropped. The
//! item scanner in [`crate::map::errors`] reads the tokens.
//!
//! Guarantees:
//!
//! - **Comments and literals never look like code.** Text inside a comment, a string, a raw string
//!   (any number of `#`), a byte or C string or a character literal is never an identifier or
//!   punctuation token, and block comments nest as Rust nests them.
//! - **An error, never a guess.** A block comment, string, raw string or character literal that
//!   does not end is a [`LexError`] naming its byte offset, so a file the lexer cannot read is
//!   reported, not mapped from a wrong reading.
//! - **No recursion and no panics.** One pass over the characters, with an explicit counter for
//!   nested block comments; every position step is a checked or saturating method.
//!
//! Method: the lexical grammar of The Rust Reference: comments, nested block comments and which
//! comments are doc comments (<https://doc.rust-lang.org/reference/comments.html>); string, raw
//! string, byte, C string and character literals, raw identifiers and lifetimes
//! (<https://doc.rust-lang.org/reference/tokens.html>). A `'` opens a character literal when the
//! character after the next one is a `'`, or when the next one cannot start an identifier, and a
//! lifetime otherwise, which is the rule of `rustc_lexer`'s `lifetime_or_char`. Numbers are read
//! loosely (digits, letters, `_`, and a `.` or exponent sign inside a decimal literal): the map
//! needs only to step over them.
//!
//! Cost: linear in the length of the source, plus one `char` and its offset held per character.
//!
//! Consulted: `rustc_lexer`, MIT OR Apache-2.0,
//!   <https://github.com/rust-lang/rust/blob/1731faea0da5b4ebf881d8e53f98066c17a74711/compiler/rustc_lexer/src/lib.rs>,
//!   for its rule telling a character literal from a lifetime and for which block comments are doc
//!   comments. No code copied.

use core::fmt;

/// What a token is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An identifier or keyword; a raw identifier `r#name` is given as `name`.
    Ident,
    /// One punctuation character.
    Punct,
    /// A number, string, raw string, byte string, C string or character literal.
    Literal,
    /// A lifetime or label, such as `'a`.
    Lifetime,
    /// An outer doc comment: one `///` line, or one `/** */` block. The text is the comment's
    /// content, without its markers.
    Doc,
}

/// One token: its kind, its text and the byte offset where it starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// What it is.
    pub kind: Kind,
    /// Its text: the identifier, the punctuation character, the literal as written, or a doc
    /// comment's content.
    pub text: String,
    /// The byte offset of its first character in the source.
    pub at: usize,
}

impl Token {
    /// Whether this is the identifier or keyword `word`.
    #[must_use]
    pub fn is_ident(&self, word: &str) -> bool {
        self.kind == Kind::Ident && self.text == word
    }

    /// Whether this is the punctuation character `c`.
    #[must_use]
    pub fn is_punct(&self, c: char) -> bool {
        self.kind == Kind::Punct && self.text.chars().eq(core::iter::once(c))
    }
}

/// Why a source could not be split into tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexError {
    /// The byte offset where the unfinished comment or literal starts.
    pub at: usize,
    /// What did not end.
    pub what: &'static str,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {} does not end", self.what, self.at)
    }
}

impl std::error::Error for LexError {}

/// A position in the source's characters.
struct Cursor<'a> {
    source: &'a str,
    chars: Vec<(usize, char)>,
    pos: usize,
}

impl Cursor<'_> {
    /// The character `ahead` places after the current one, if any.
    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars
            .get(self.pos.saturating_add(ahead))
            .map(|&(_, c)| c)
    }

    /// Moves past the current character and returns it.
    fn bump(&mut self) -> Option<char> {
        let c = self.peek(0)?;
        self.pos = self.pos.saturating_add(1);
        Some(c)
    }

    /// The byte offset of the current character, or the source's length at its end.
    fn offset(&self) -> usize {
        self.chars
            .get(self.pos)
            .map_or(self.source.len(), |&(at, _)| at)
    }

    /// Moves past every character for which `keep` holds.
    fn eat_while(&mut self, keep: impl Fn(char) -> bool) {
        while self.peek(0).is_some_and(&keep) {
            self.pos = self.pos.saturating_add(1);
        }
    }

    /// The source text from byte `start` to the current position.
    fn text_from(&self, start: usize) -> String {
        self.source
            .get(start..self.offset())
            .unwrap_or_default()
            .to_owned()
    }
}

/// Whether `c` can start an identifier (Unicode's `XID_Start` approximated by `is_alphabetic`).
fn id_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

/// Whether `c` can continue an identifier.
fn id_continue(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

/// The tokens of `source`, in order.
///
/// # Errors
///
/// [`LexError`] for a block comment, string, raw string or character literal that does not end.
pub fn tokens(source: &str) -> Result<Vec<Token>, LexError> {
    let mut cursor = Cursor {
        source,
        chars: source.char_indices().collect(),
        pos: 0,
    };
    let mut out = Vec::new();
    while let Some(c) = cursor.peek(0) {
        let at = cursor.offset();
        if c.is_whitespace() {
            cursor.bump();
        } else if c == '/' && cursor.peek(1) == Some('/') {
            line_comment(&mut cursor, at, &mut out);
        } else if c == '/' && cursor.peek(1) == Some('*') {
            block_comment(&mut cursor, at, &mut out)?;
        } else if c == '"' {
            cursor.bump();
            quoted(&mut cursor, at, '"', "a string")?;
            out.push(literal(&cursor, at));
        } else if c == '\'' {
            out.push(quote_or_lifetime(&mut cursor, at)?);
        } else if c.is_ascii_digit() {
            number(&mut cursor);
            out.push(literal(&cursor, at));
        } else if id_start(c) {
            out.push(word(&mut cursor, at)?);
        } else {
            cursor.bump();
            out.push(Token {
                kind: Kind::Punct,
                text: c.to_string(),
                at,
            });
        }
    }
    Ok(out)
}

/// A literal token from `at` to the cursor.
fn literal(cursor: &Cursor<'_>, at: usize) -> Token {
    Token {
        kind: Kind::Literal,
        text: cursor.text_from(at),
        at,
    }
}

/// A `//` comment: an outer doc comment when it starts `///` and not `////`.
fn line_comment(cursor: &mut Cursor<'_>, at: usize, out: &mut Vec<Token>) {
    let doc = cursor.peek(2) == Some('/') && cursor.peek(3) != Some('/');
    cursor.eat_while(|c| c != '\n');
    if doc {
        let line = cursor.text_from(at);
        let rest = line.get(3..).unwrap_or_default().trim_end();
        out.push(Token {
            kind: Kind::Doc,
            text: rest.strip_prefix(' ').unwrap_or(rest).to_owned(),
            at,
        });
    }
}

/// A `/* */` comment, nested as Rust nests them: an outer doc comment when it starts `/**` and is
/// neither `/**/` nor `/***`.
fn block_comment(cursor: &mut Cursor<'_>, at: usize, out: &mut Vec<Token>) -> Result<(), LexError> {
    let doc = cursor.peek(2) == Some('*') && !matches!(cursor.peek(3), Some('*' | '/'));
    cursor.bump();
    cursor.bump();
    let mut depth: usize = 1;
    while depth > 0 {
        match cursor.bump() {
            None => {
                return Err(LexError {
                    at,
                    what: "a block comment",
                });
            }
            Some('/') if cursor.peek(0) == Some('*') => {
                cursor.bump();
                depth = depth.saturating_add(1);
            }
            Some('*') if cursor.peek(0) == Some('/') => {
                cursor.bump();
                depth = depth.saturating_sub(1);
            }
            Some(_) => {}
        }
    }
    if doc {
        let text = cursor.text_from(at);
        let inner = text
            .strip_prefix("/**")
            .and_then(|t| t.strip_suffix("*/"))
            .unwrap_or_default();
        let lines: Vec<&str> = inner
            .lines()
            .map(|l| {
                let l = l.trim();
                l.strip_prefix("* ")
                    .or_else(|| l.strip_prefix('*'))
                    .unwrap_or(l)
            })
            .collect();
        out.push(Token {
            kind: Kind::Doc,
            text: lines.join("\n").trim().to_owned(),
            at,
        });
    }
    Ok(())
}

/// The rest of a literal quoted by `close`, the opening quote already read; `\` escapes the next
/// character.
fn quoted(
    cursor: &mut Cursor<'_>,
    at: usize,
    close: char,
    what: &'static str,
) -> Result<(), LexError> {
    loop {
        match cursor.bump() {
            None => return Err(LexError { at, what }),
            Some('\\') => {
                cursor.bump();
            }
            Some(c) if c == close => return Ok(()),
            Some(_) => {}
        }
    }
}

/// The rest of a raw string, from its `#`s and opening quote: it ends at a `"` followed by as many
/// `#` as it opened with.
fn raw(cursor: &mut Cursor<'_>, at: usize) -> Result<(), LexError> {
    let mut hashes: usize = 0;
    while cursor.peek(0) == Some('#') {
        cursor.bump();
        hashes = hashes.saturating_add(1);
    }
    if cursor.bump() != Some('"') {
        return Err(LexError {
            at,
            what: "a raw string's opening quote",
        });
    }
    loop {
        match cursor.bump() {
            None => {
                return Err(LexError {
                    at,
                    what: "a raw string",
                });
            }
            Some('"') if (0..hashes).all(|k| cursor.peek(k) == Some('#')) => {
                for _ in 0..hashes {
                    cursor.bump();
                }
                return Ok(());
            }
            Some(_) => {}
        }
    }
}

/// A `'`: a character literal or a lifetime, by `rustc_lexer`'s rule (module header).
fn quote_or_lifetime(cursor: &mut Cursor<'_>, at: usize) -> Result<Token, LexError> {
    cursor.bump();
    let lifetime = cursor.peek(1) != Some('\'')
        && cursor
            .peek(0)
            .is_some_and(|c| id_start(c) || c.is_ascii_digit());
    if lifetime {
        cursor.bump();
        cursor.eat_while(id_continue);
        if cursor.peek(0) == Some('\'') {
            // A quoted word such as 'ab': not valid Rust, but read as one literal, never as code.
            cursor.bump();
            return Ok(literal(cursor, at));
        }
        return Ok(Token {
            kind: Kind::Lifetime,
            text: cursor.text_from(at),
            at,
        });
    }
    quoted(cursor, at, '\'', "a character literal")?;
    Ok(literal(cursor, at))
}

/// A number: digits, letters and `_`, a `.` followed by a digit, and an exponent's sign in a
/// decimal literal.
fn number(cursor: &mut Cursor<'_>) {
    let hex_or_binary =
        cursor.peek(0) == Some('0') && matches!(cursor.peek(1), Some('x' | 'b' | 'o'));
    loop {
        cursor.eat_while(id_continue);
        let last = cursor
            .pos
            .checked_sub(1)
            .and_then(|p| cursor.chars.get(p))
            .map(|&(_, c)| c);
        match cursor.peek(0) {
            Some('.') if cursor.peek(1).is_some_and(|c| c.is_ascii_digit()) => {
                cursor.bump();
            }
            Some('+' | '-')
                if !hex_or_binary
                    && matches!(last, Some('e' | 'E'))
                    && cursor.peek(1).is_some_and(|c| c.is_ascii_digit()) =>
            {
                cursor.bump();
            }
            _ => return,
        }
    }
}

/// An identifier, a raw identifier, or a prefixed literal (`b"…"`, `b'…'`, `c"…"`, `r"…"`,
/// `r#"…"#`, `br…`, `cr…`).
fn word(cursor: &mut Cursor<'_>, at: usize) -> Result<Token, LexError> {
    cursor.eat_while(id_continue);
    let text = cursor.text_from(at);
    let next = cursor.peek(0);
    match (text.as_str(), next) {
        ("b" | "c", Some('"')) => {
            cursor.bump();
            quoted(cursor, at, '"', "a byte or C string")?;
            Ok(literal(cursor, at))
        }
        ("b", Some('\'')) => {
            cursor.bump();
            quoted(cursor, at, '\'', "a byte literal")?;
            Ok(literal(cursor, at))
        }
        ("r" | "br" | "cr", Some('"')) => {
            raw(cursor, at)?;
            Ok(literal(cursor, at))
        }
        ("r" | "br" | "cr", Some('#')) => {
            if text == "r" && cursor.peek(1).is_some_and(id_start) {
                // A raw identifier: `r#name` is the identifier `name`.
                cursor.bump();
                let start = cursor.offset();
                cursor.eat_while(id_continue);
                return Ok(Token {
                    kind: Kind::Ident,
                    text: cursor.text_from(start),
                    at,
                });
            }
            raw(cursor, at)?;
            Ok(literal(cursor, at))
        }
        _ => Ok(Token {
            kind: Kind::Ident,
            text,
            at,
        }),
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;

    fn idents(source: &str) -> Vec<String> {
        tokens(source)
            .unwrap()
            .into_iter()
            .filter(|t| t.kind == Kind::Ident)
            .map(|t| t.text)
            .collect()
    }

    #[test]
    fn comments_and_literals_hide_code() {
        let source = r###"
            // pub enum HidA {}
            /* pub enum HidB { /* nested pub enum HidC */ } */
            const S: &str = "pub enum HidD \" pub enum HidE";
            const R: &str = r#"pub enum HidF "# ;
            const R2: &str = r##"a "# pub enum HidG"##;
            const BS: &[u8] = b"pub enum HidH";
            const C: char = '"';
            const Q: char = '\'';
            const U: char = '\u{1F600}';
            fn f<'a>(x: &'a str) -> &'a str { x }
            pub enum Real {}
        "###;
        let names = idents(source);
        for hidden in [
            "HidA", "HidB", "HidC", "HidD", "HidE", "HidF", "HidG", "HidH",
        ] {
            assert!(!names.contains(&hidden.to_owned()), "{hidden} in {names:?}");
        }
        assert!(names.contains(&"Real".to_owned()));
        let lifetimes = tokens(source)
            .unwrap()
            .into_iter()
            .filter(|t| t.kind == Kind::Lifetime)
            .count();
        assert_eq!(lifetimes, 3);
    }

    #[test]
    fn doc_comments_are_kept_and_others_dropped() {
        let source = "//! inner\n/// outer one\n//// not a doc\n/** block\n * doc */\n/**/\n/*** no */\n\
                      /*! inner block */\nfn f() {}";
        let docs: Vec<String> = tokens(source)
            .unwrap()
            .into_iter()
            .filter(|t| t.kind == Kind::Doc)
            .map(|t| t.text)
            .collect();
        assert_eq!(docs, ["outer one", "block\ndoc"]);
    }

    #[test]
    fn raw_identifiers_and_numbers() {
        assert_eq!(idents("r#enum 1.5e-3 0x1e 1..2 x"), ["enum", "x"]);
        let lits: Vec<String> = tokens("1.5e-3 0x1e 1..2")
            .unwrap()
            .into_iter()
            .filter(|t| t.kind == Kind::Literal)
            .map(|t| t.text)
            .collect();
        assert_eq!(lits, ["1.5e-3", "0x1e", "1", "2"]);
    }

    #[test]
    fn unterminated_things_are_errors() {
        for (source, what) in [
            ("/* open", "a block comment"),
            ("/* a /* b */", "a block comment"),
            ("\"open", "a string"),
            ("r#\"open\"", "a raw string"),
            ("'\\", "a character literal"),
            ("b\"x", "a byte or C string"),
        ] {
            let error = tokens(source).unwrap_err();
            assert_eq!(error.what, what, "{source}");
            assert_eq!(error.at, 0, "{source}");
        }
    }

    #[test]
    fn offsets_are_byte_offsets() {
        let t = tokens("é x").unwrap();
        assert_eq!(t[0].text, "é");
        assert_eq!(t[1].at, 3);
    }
}
