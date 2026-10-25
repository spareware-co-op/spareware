//! Rust source as tokens, for the map: the language's lexical grammar, from the standard lexer, so
//! that items are found without being misled by comments, strings or character literals.
//!
//! What it does: [`tokens`] splits a source file into [`Token`]s: identifiers, punctuation (each
//! delimiter included), literals, lifetimes and outer doc comments (`///` and `/** */`), each with
//! its text and byte offset. Plain comments, inner doc comments (`//!`, `/*! */`) and whitespace
//! are dropped. The item scanner in [`crate::map::errors`] reads the tokens.
//!
//! Guarantees:
//!
//! - **Comments and literals never look like code.** Text inside a comment, a string, a raw string
//!   (any number of `#`), a byte or C string or a character literal is never an identifier or
//!   punctuation token, and block comments nest as Rust nests them: the lexer is the one `syn`
//!   parses with, which follows rustc's.
//! - **An error, never a guess.** A comment or literal that does not end, and a delimiter that
//!   does not close, is a [`LexError`] naming the byte offset where the lexer stopped, so a file
//!   the lexer cannot read is reported, not mapped from a wrong reading.
//! - **No recursion and no panics** here: groups are opened with an explicit stack, and every
//!   offset is the lexer's own.
//!
//! Method: the `proc-macro2` crate's lexer (MIT OR Apache-2.0,
//! <https://docs.rs/proc-macro2/1.0.107/proc_macro2/>), with its `span-locations` feature for byte
//! offsets: the lexical grammar of The Rust Reference (<https://doc.rust-lang.org/reference/tokens.html>,
//! <https://doc.rust-lang.org/reference/comments.html>) as `syn` reads it. It gives a doc comment
//! as the attribute it means, `#[doc = "..."]`, with the comment's span on every token, so a `#`
//! whose span covers a comment in the source is that comment, read from the source text with its
//! markers dropped. Piece G12a put the crate in place of a lexer written here (chapter 29.1, the
//! owner's direction of 2026-10-22: use well-known permissive crates rather than hand-rolling).
//!
//! Cost: linear in the length of the source, for the lexer and for the walk over its tokens.

use core::fmt;

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};

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
    /// The byte offset where the lexer stopped.
    pub at: usize,
    /// What it could not read there.
    pub what: &'static str,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {} does not end", self.what, self.at)
    }
}

impl std::error::Error for LexError {}

/// What a [`LexError`] says the lexer could not read.
pub const UNREADABLE: &str = "a comment, a literal or a delimiter";

/// The text of an outer doc comment, `comment` being its source text with its markers: a `///`
/// line without them, one space after them and its trailing spaces; a `/** */` block without its
/// markers, each line trimmed and stripped of a leading `* ` or `*`, the whole trimmed.
fn doc_text(comment: &str) -> String {
    if let Some(line) = comment.strip_prefix("///") {
        let line = line.trim_end();
        return line.strip_prefix(' ').unwrap_or(line).to_owned();
    }
    let inner = comment
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
    lines.join("\n").trim().to_owned()
}

/// The opening and closing characters of a delimiter; none for an invisible group.
fn delimiters(delimiter: Delimiter) -> Option<(char, char)> {
    match delimiter {
        Delimiter::Parenthesis => Some(('(', ')')),
        Delimiter::Brace => Some(('{', '}')),
        Delimiter::Bracket => Some(('[', ']')),
        Delimiter::None => None,
    }
}

/// The tokens of `source`, in order.
///
/// # Errors
///
/// [`LexError`] for a comment or literal that does not end, or a delimiter that does not close.
pub fn tokens(source: &str) -> Result<Vec<Token>, LexError> {
    let stream: TokenStream = source
        .parse()
        .map_err(|e: proc_macro2::LexError| LexError {
            at: e.span().byte_range().start,
            what: UNREADABLE,
        })?;
    let mut out = Vec::new();
    // Each level: the rest of a group's tokens, and the closing delimiter to give after them.
    let mut stack: Vec<(
        std::iter::Peekable<proc_macro2::token_stream::IntoIter>,
        Option<Token>,
    )> = vec![(stream.into_iter().peekable(), None)];
    while let Some((trees, close)) = stack.last_mut() {
        let Some(tree) = trees.next() else {
            if let Some(close) = close.take() {
                out.push(close);
            }
            stack.pop();
            continue;
        };
        let range = tree.span().byte_range();
        let at = range.start;
        match tree {
            TokenTree::Group(group) => {
                let close = delimiters(group.delimiter()).map(|(open, close)| {
                    out.push(Token {
                        kind: Kind::Punct,
                        text: open.to_string(),
                        at: group.span_open().byte_range().start,
                    });
                    Token {
                        kind: Kind::Punct,
                        text: close.to_string(),
                        at: group.span_close().byte_range().start,
                    }
                });
                stack.push((group.stream().into_iter().peekable(), close));
            }
            TokenTree::Ident(ident) => {
                let text = ident.to_string();
                out.push(Token {
                    kind: Kind::Ident,
                    text: text
                        .strip_prefix("r#")
                        .map_or_else(|| text.clone(), str::to_owned),
                    at,
                });
            }
            TokenTree::Literal(literal) => out.push(Token {
                kind: Kind::Literal,
                text: literal.to_string(),
                at,
            }),
            TokenTree::Punct(punct) => {
                let spelled = source.get(range.clone()).unwrap_or_default();
                let comment = spelled.starts_with("//") || spelled.starts_with("/*");
                if punct.as_char() == '#' && comment {
                    // A doc comment, lexed as `#`, for an inner one `!`, and `[doc = "..."]`, all
                    // with the comment's span: an outer one is kept, an inner one dropped.
                    if trees
                        .peek()
                        .is_some_and(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == '!'))
                    {
                        trees.next();
                    }
                    trees.next();
                    let outer = spelled.starts_with("///") || spelled.starts_with("/**");
                    if outer {
                        out.push(Token {
                            kind: Kind::Doc,
                            text: doc_text(spelled),
                            at,
                        });
                    }
                    continue;
                }
                let lifetime = punct.as_char() == '\''
                    && punct.spacing() == Spacing::Joint
                    && matches!(trees.peek(), Some(TokenTree::Ident(_)));
                if lifetime && let Some(TokenTree::Ident(name)) = trees.next() {
                    out.push(Token {
                        kind: Kind::Lifetime,
                        text: format!("'{name}"),
                        at,
                    });
                    continue;
                }
                out.push(Token {
                    kind: Kind::Punct,
                    text: punct.as_char().to_string(),
                    at,
                });
            }
        }
    }
    Ok(out)
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
        // Each is refused where the lexer stops: at the unfinished comment or literal, or at the
        // delimiter that does not close.
        for (source, at) in [
            ("/* open", 0),
            ("/* a /* b */", 0),
            ("\"open", 0),
            ("r#\"open\"", 0),
            ("'\\", 0),
            ("b\"x", 0),
            ("x /* open", 2),
            ("fn f() { (]", 10),
            ("fn f() {", 7),
            ("fn f() }", 7),
        ] {
            let error = tokens(source).unwrap_err();
            assert_eq!(error.what, UNREADABLE, "{source}");
            assert_eq!(error.at, at, "{source}: {error}");
        }
    }

    #[test]
    fn delimiters_are_tokens_at_their_offsets() {
        let t = tokens("f(a[1], {b});").unwrap();
        let shown: Vec<(&str, usize)> = t.iter().map(|t| (t.text.as_str(), t.at)).collect();
        assert_eq!(
            shown,
            [
                ("f", 0),
                ("(", 1),
                ("a", 2),
                ("[", 3),
                ("1", 4),
                ("]", 5),
                (",", 6),
                ("{", 8),
                ("b", 9),
                ("}", 10),
                (")", 11),
                (";", 12),
            ]
        );
        // A real `#[doc]` attribute is code, not a doc comment; a doc comment gives its offset.
        let t = tokens("#[doc = \"x\"]\n/// y\nfn f() {}").unwrap();
        assert_eq!(t[0].text, "#");
        let doc: Vec<(&str, usize)> = t
            .iter()
            .filter(|t| t.kind == Kind::Doc)
            .map(|t| (t.text.as_str(), t.at))
            .collect();
        assert_eq!(doc, [("y", 13)]);
    }

    #[test]
    fn offsets_are_byte_offsets() {
        let t = tokens("é x").unwrap();
        assert_eq!(t[0].text, "é");
        assert_eq!(t[1].at, 3);
    }
}
