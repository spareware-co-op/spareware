//! Rust source, read for the quality gates: which lines are test code, whether a file has code that
//! runs, and every `coverage(off)` mark with the proof it cites.
//!
//! What it does: [`scan`] splits a Rust file into tokens with their line numbers, skipping
//! comments (kept apart, for citations), strings, raw strings and character literals, so a brace or
//! a word inside a string or a comment is never read as code. From the tokens it finds:
//!
//! - **Test code** ([`Scan::test_lines`]): every item behind an exact `#[cfg(test)]` attribute,
//!   from the attribute's line to the line of the item's closing brace (or semicolon). Coverage
//!   counts the kernel's code, not its tests, so the coverage gate leaves these lines out. Only the
//!   exact attribute counts; any other spelling (`cfg(all(test, ...))`) is read as kernel code,
//!   which can only make the gate stricter. An out-of-line test module (`#[cfg(test)] mod name;`)
//!   is listed in [`Scan::test_modules`], and its file is test code as a whole.
//! - **Code that runs** ([`Scan::has_functions`]): a function definition (`fn` followed by a name)
//!   outside test code. A file with one must appear in the coverage data; a file without one (only
//!   declarations, types and constants) has nothing to count.
//! - **Branch sites** ([`Scan::branch_sites`]): the lines outside test code that hold an `if` or
//!   `while` keyword, the places where branch coverage must have data, so a report made without
//!   branch coverage cannot pass the branch gate by having nothing to count.
//! - **`coverage(off)` marks** ([`Scan::coverage_off`]): every attribute whose tokens hold
//!   `coverage ( off )`, in any spelling (`#[coverage(off)]`, `#![coverage(off)]`,
//!   `#[cfg_attr(coverage_nightly, coverage(off))]`), with the proof cited in the comment lines
//!   right above it (`proof: <path>::<name>`) and the first line of the item it marks.
//!
//! Guarantees: a file the lexer cannot read to its end (an unterminated string, character literal
//! or block comment, or unbalanced braces after `#[cfg(test)]`) is a [`ScanError`], never a
//! partial answer, because a gate must not pass on a file it could not read.
//!
//! Method: a hand-written lexer for the token classes of the Rust Reference's lexical structure
//! (<https://doc.rust-lang.org/reference/tokens.html>, comments at
//! <https://doc.rust-lang.org/reference/comments.html>): nested block comments, escapes, raw
//! strings with any number of `#`, byte and C strings, character literals told apart from
//! lifetimes by their closing quote. Items are found by bracket matching over the tokens.
//!
//! Cost: linear in the file's length.

use core::fmt;

/// One token: an identifier or keyword, or one punctuation character, with its 1-based line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// What the token is.
    pub kind: TokenKind,
    /// The line it starts on, from 1.
    pub line: usize,
}

/// The token classes the scans need; literals and lifetimes are kept only as markers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// An identifier or keyword, raw identifiers without their `r#`.
    Ident(String),
    /// One punctuation character.
    Punct(char),
    /// A string, byte string, character, byte or number literal.
    Literal,
    /// A lifetime or loop label.
    Lifetime,
}

/// One comment, line or block, with the line it starts on and its text after `//` or `/*`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    /// The line it starts on, from 1.
    pub line: usize,
    /// The text, without the opening `//` or `/*` (and a block comment's closing `*/`).
    pub text: String,
}

/// A `coverage(off)` mark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageOff {
    /// The line of the attribute's `#`.
    pub line: usize,
    /// The proof cited in the comment lines right above the attribute, as written after
    /// `proof:` (`<path>::<name>`); `None` when no comment cites one.
    pub proof: Option<String>,
    /// The first line of the item the attribute marks, trimmed: the next line that is not blank,
    /// a comment or an attribute. For an inner attribute (`#![...]`), the attribute's own line.
    pub item: String,
}

/// What [`scan`] found in one file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// Inclusive line ranges of test code, in order.
    pub test_lines: Vec<(usize, usize)>,
    /// The names of out-of-line test modules, `#[cfg(test)] mod name;`, whose files are test code
    /// as a whole.
    pub test_modules: Vec<String>,
    /// Whether a function is defined outside test code.
    pub has_functions: bool,
    /// The lines outside test code holding an `if` or `while` keyword, in order, once each.
    pub branch_sites: Vec<usize>,
    /// Every `coverage(off)` mark, in order.
    pub coverage_off: Vec<CoverageOff>,
}

impl Scan {
    /// Whether `line` is inside test code.
    #[must_use]
    pub fn is_test_line(&self, line: usize) -> bool {
        self.test_lines
            .iter()
            .any(|&(first, last)| first <= line && line <= last)
    }
}

/// Why a file could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanError {
    /// The line where the problem starts.
    pub line: usize,
    /// What is wrong.
    pub why: String,
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.why)
    }
}

impl std::error::Error for ScanError {}

/// A character cursor that counts lines.
struct Cursor<'a> {
    chars: core::iter::Peekable<core::str::Chars<'a>>,
    line: usize,
}

impl Cursor<'_> {
    fn next(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            // A file never has usize::MAX lines; saturating keeps the lint's promise.
            self.line = self.line.saturating_add(1);
        }
        Some(c)
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    /// The character `k` places after the next one (`peek_at(0)` is `peek`).
    fn peek_at(&self, k: usize) -> Option<char> {
        self.chars.clone().nth(k)
    }
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

/// Splits `src` into tokens and comments.
///
/// # Errors
///
/// [`ScanError`] for an unterminated block comment, string, raw string or character literal.
pub fn lex(src: &str) -> Result<(Vec<Token>, Vec<Comment>), ScanError> {
    let mut cur = Cursor {
        chars: src.chars().peekable(),
        line: 1,
    };
    let mut tokens = Vec::new();
    let mut comments = Vec::new();
    while let Some(c) = cur.peek() {
        let line = cur.line;
        if c.is_whitespace() {
            cur.next();
        } else if c == '/' && cur.peek_at(1) == Some('/') {
            cur.next();
            cur.next();
            let mut text = String::new();
            while let Some(c) = cur.peek() {
                if c == '\n' {
                    break;
                }
                text.push(c);
                cur.next();
            }
            comments.push(Comment { line, text });
        } else if c == '/' && cur.peek_at(1) == Some('*') {
            comments.push(Comment {
                line,
                text: block_comment(&mut cur)?,
            });
        } else if c == '"' {
            cur.next();
            quoted(&mut cur, '"', line)?;
            tokens.push(Token {
                kind: TokenKind::Literal,
                line,
            });
        } else if c == '\'' {
            tokens.push(Token {
                kind: quote_or_lifetime(&mut cur, line)?,
                line,
            });
        } else if c.is_ascii_digit() {
            number(&mut cur);
            tokens.push(Token {
                kind: TokenKind::Literal,
                line,
            });
        } else if is_ident_start(c) {
            let mut word = String::new();
            while let Some(c) = cur.peek().filter(|&c| is_ident_continue(c)) {
                word.push(c);
                cur.next();
            }
            tokens.push(Token {
                kind: word_token(&mut cur, word, line)?,
                line,
            });
        } else {
            cur.next();
            tokens.push(Token {
                kind: TokenKind::Punct(c),
                line,
            });
        }
    }
    Ok((tokens, comments))
}

/// A block comment, nested, starting at its `/*`; returns its text.
fn block_comment(cur: &mut Cursor<'_>) -> Result<String, ScanError> {
    let line = cur.line;
    cur.next();
    cur.next();
    let mut depth: usize = 1;
    let mut text = String::new();
    loop {
        let Some(c) = cur.next() else {
            return Err(ScanError {
                line,
                why: "unterminated block comment".to_owned(),
            });
        };
        if c == '/' && cur.peek() == Some('*') {
            cur.next();
            depth = depth.saturating_add(1);
            text.push_str("/*");
        } else if c == '*' && cur.peek() == Some('/') {
            cur.next();
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Ok(text);
            }
            text.push_str("*/");
        } else {
            text.push(c);
        }
    }
}

/// The rest of a string or character literal after its opening quote, escapes included.
fn quoted(cur: &mut Cursor<'_>, close: char, line: usize) -> Result<(), ScanError> {
    loop {
        match cur.next() {
            None => {
                return Err(ScanError {
                    line,
                    why: format!("unterminated literal (no closing {close})"),
                });
            }
            Some('\\') => {
                cur.next();
            }
            Some(c) if c == close => return Ok(()),
            Some(_) => {}
        }
    }
}

/// A raw string after its prefix letters: `#`s, a quote, and the text up to a quote followed by
/// the same number of `#`s.
fn raw_string(cur: &mut Cursor<'_>, line: usize) -> Result<(), ScanError> {
    let mut hashes: usize = 0;
    while cur.peek() == Some('#') {
        cur.next();
        hashes = hashes.saturating_add(1);
    }
    if cur.next() != Some('"') {
        return Err(ScanError {
            line,
            why: "a raw string prefix not followed by a quote".to_owned(),
        });
    }
    loop {
        match cur.next() {
            None => {
                return Err(ScanError {
                    line,
                    why: "unterminated raw string".to_owned(),
                });
            }
            Some('"') => {
                let closing = (0..hashes).all(|k| cur.peek_at(k) == Some('#'));
                if closing {
                    for _ in 0..hashes {
                        cur.next();
                    }
                    return Ok(());
                }
            }
            Some(_) => {}
        }
    }
}

/// After an identifier-like word: a literal when the word is a string or character prefix
/// (`b"..."`, `r#"..."#`, `br"..."`, `c"..."`, `b'x'`), a raw identifier for `r#name`, otherwise
/// the word itself.
fn word_token(cur: &mut Cursor<'_>, word: String, line: usize) -> Result<TokenKind, ScanError> {
    let next = cur.peek();
    let raw_prefix = matches!(word.as_str(), "r" | "br" | "cr");
    let quote_prefix = matches!(word.as_str(), "b" | "c");
    if raw_prefix && next == Some('"') {
        raw_string(cur, line)?;
        return Ok(TokenKind::Literal);
    }
    if raw_prefix && next == Some('#') {
        let after = cur.peek_at(1);
        if after == Some('"') || after == Some('#') {
            raw_string(cur, line)?;
            return Ok(TokenKind::Literal);
        }
        if word == "r" && after.is_some_and(is_ident_start) {
            cur.next();
            let mut name = String::new();
            while let Some(c) = cur.peek().filter(|&c| is_ident_continue(c)) {
                name.push(c);
                cur.next();
            }
            return Ok(TokenKind::Ident(name));
        }
    }
    if quote_prefix && next == Some('"') {
        cur.next();
        quoted(cur, '"', line)?;
        return Ok(TokenKind::Literal);
    }
    if word == "b" && next == Some('\'') {
        cur.next();
        quoted(cur, '\'', line)?;
        return Ok(TokenKind::Literal);
    }
    Ok(TokenKind::Ident(word))
}

/// A character literal (`'a'`, `'\n'`, `'{'`) or a lifetime or label (`'a`, `'static`).
fn quote_or_lifetime(cur: &mut Cursor<'_>, line: usize) -> Result<TokenKind, ScanError> {
    cur.next();
    match (cur.peek(), cur.peek_at(1)) {
        (Some('\\'), _) => {
            quoted(cur, '\'', line)?;
            Ok(TokenKind::Literal)
        }
        (Some(_), Some('\'')) => {
            cur.next();
            cur.next();
            Ok(TokenKind::Literal)
        }
        (Some(c), _) if is_ident_start(c) => {
            while cur.peek().is_some_and(is_ident_continue) {
                cur.next();
            }
            Ok(TokenKind::Lifetime)
        }
        _ => Err(ScanError {
            line,
            why: "a quote that starts neither a character literal nor a lifetime".to_owned(),
        }),
    }
}

/// A number literal: digits, letters (suffixes, hexadecimal, exponents) and underscores, and a
/// point only when a digit follows it, so `0..n` stays a range.
fn number(cur: &mut Cursor<'_>) {
    while let Some(c) = cur.peek() {
        let continues = is_ident_continue(c)
            || (c == '.' && cur.peek_at(1).is_some_and(|d| d.is_ascii_digit()));
        if !continues {
            break;
        }
        cur.next();
    }
}

fn is_punct(token: Option<&Token>, c: char) -> bool {
    matches!(token, Some(Token { kind: TokenKind::Punct(p), .. }) if *p == c)
}

fn is_ident(token: Option<&Token>, word: &str) -> bool {
    matches!(token, Some(Token { kind: TokenKind::Ident(w), .. }) if w == word)
}

/// The index of the bracket that closes the one at `open`, which must be `(`, `[` or `{`.
fn matching(tokens: &[Token], open: usize) -> Option<usize> {
    let (o, c) = match tokens.get(open)?.kind {
        TokenKind::Punct('(') => ('(', ')'),
        TokenKind::Punct('[') => ('[', ']'),
        TokenKind::Punct('{') => ('{', '}'),
        _ => return None,
    };
    let mut depth: usize = 0;
    for (i, t) in tokens.iter().enumerate().skip(open) {
        if t.kind == TokenKind::Punct(o) {
            depth = depth.saturating_add(1);
        } else if t.kind == TokenKind::Punct(c) {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// An attribute starting at `i` (`#[...]` or `#![...]`): the index of its `[` and of its `]`.
fn attribute_at(tokens: &[Token], i: usize) -> Option<(usize, usize)> {
    if !is_punct(tokens.get(i), '#') {
        return None;
    }
    let mut open = i.saturating_add(1);
    if is_punct(tokens.get(open), '!') {
        open = open.saturating_add(1);
    }
    if !is_punct(tokens.get(open), '[') {
        return None;
    }
    Some((open, matching(tokens, open)?))
}

/// Whether the tokens strictly between `open` and `close` are exactly `cfg ( test )`.
fn is_cfg_test(tokens: &[Token], open: usize, close: usize) -> bool {
    let inner = tokens.get(open.saturating_add(1)..close).unwrap_or(&[]);
    inner.len() == 4
        && is_ident(inner.first(), "cfg")
        && is_punct(inner.get(1), '(')
        && is_ident(inner.get(2), "test")
        && is_punct(inner.get(3), ')')
}

/// Whether the tokens strictly between `open` and `close` hold `coverage ( off )`.
fn has_coverage_off(tokens: &[Token], open: usize, close: usize) -> bool {
    let inner = tokens.get(open.saturating_add(1)..close).unwrap_or(&[]);
    inner.windows(4).any(|w| {
        is_ident(w.first(), "coverage")
            && is_punct(w.get(1), '(')
            && is_ident(w.get(2), "off")
            && is_punct(w.get(3), ')')
    })
}

/// The end of the item that starts at token `start` (after its attributes): the index of the
/// closing brace of its first top-level `{`, or of its first top-level `;`.
fn item_end(tokens: &[Token], start: usize) -> Option<usize> {
    let mut i = start;
    while let Some(t) = tokens.get(i) {
        match t.kind {
            TokenKind::Punct('{') => return matching(tokens, i),
            TokenKind::Punct(';') => return Some(i),
            TokenKind::Punct('(' | '[') => i = matching(tokens, i)?,
            _ => {}
        }
        i = i.saturating_add(1);
    }
    None
}

/// The lines of `src`, trimmed, from line 1.
fn line_text(lines: &[&str], line: usize) -> Option<String> {
    lines.get(line.checked_sub(1)?).map(|l| l.trim().to_owned())
}

/// The proof a mark cites: `proof:` in the comment lines directly above line `mark`, walking up
/// over comment and attribute lines and stopping at the first other line.
fn cited_proof(lines: &[&str], mark: usize) -> Option<String> {
    let mut line = mark.checked_sub(1)?;
    while line >= 1 {
        let text = line_text(lines, line)?;
        if let Some(comment) = text.strip_prefix("//") {
            if let Some((_, after)) = comment.split_once("proof:") {
                return after.split_whitespace().next().map(str::to_owned);
            }
        } else if !text.starts_with("#[") {
            return None;
        }
        line = line.checked_sub(1)?;
    }
    None
}

/// The first line after `mark` that is not blank, a comment or an attribute, trimmed.
fn marked_item(lines: &[&str], mark: usize) -> String {
    lines
        .iter()
        .skip(mark)
        .map(|l| l.trim())
        .find(|t| !t.is_empty() && !t.starts_with("//") && !t.starts_with("#["))
        .unwrap_or("")
        .to_owned()
}

/// Scans one Rust file.
///
/// # Errors
///
/// [`ScanError`] if the file cannot be lexed, or an item behind `#[cfg(test)]` has no end.
pub fn scan(src: &str) -> Result<Scan, ScanError> {
    let (tokens, _comments) = lex(src)?;
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Scan::default();
    let mut i = 0usize;
    while let Some(token) = tokens.get(i) {
        let Some((open, close)) = attribute_at(&tokens, i) else {
            i = i.saturating_add(1);
            continue;
        };
        if has_coverage_off(&tokens, open, close) {
            let inner = open != i.saturating_add(1);
            out.coverage_off.push(CoverageOff {
                line: token.line,
                proof: cited_proof(&lines, token.line),
                item: if inner {
                    line_text(&lines, token.line).unwrap_or_default()
                } else {
                    marked_item(&lines, token.line)
                },
            });
        }
        if is_cfg_test(&tokens, open, close) && open == i.saturating_add(1) {
            let mut start = close.saturating_add(1);
            while let Some((_, end)) = attribute_at(&tokens, start) {
                start = end.saturating_add(1);
            }
            let end = item_end(&tokens, start).ok_or_else(|| ScanError {
                line: token.line,
                why: "an item behind #[cfg(test)] never ends".to_owned(),
            })?;
            let last = tokens.get(end).map_or(token.line, |t| t.line);
            out.test_lines.push((token.line, last));
            if let [
                Some(Token {
                    kind: TokenKind::Ident(m),
                    ..
                }),
                Some(Token {
                    kind: TokenKind::Ident(name),
                    ..
                }),
                Some(Token {
                    kind: TokenKind::Punct(';'),
                    ..
                }),
            ] = [
                tokens.get(start),
                tokens.get(start.saturating_add(1)),
                tokens.get(start.saturating_add(2)),
            ] && m == "mod"
            {
                out.test_modules.push(name.clone());
            }
            // Marks inside test code are found by continuing inside it.
        }
        i = close.saturating_add(1);
    }
    let mut prev_fn = false;
    for t in &tokens {
        let TokenKind::Ident(word) = &t.kind else {
            prev_fn = false;
            continue;
        };
        if out.is_test_line(t.line) {
            prev_fn = false;
            continue;
        }
        if prev_fn {
            out.has_functions = true;
        }
        if (word == "if" || word == "while") && out.branch_sites.last() != Some(&t.line) {
            out.branch_sites.push(t.line);
        }
        prev_fn = word == "fn";
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

    #[test]
    fn strings_comments_and_chars_hide_braces() {
        let src = "fn a() {\n    let s = \"{ } // not a comment\";\n    let c = '{';\n    \
                   let r = r#\"}\"#;\n    /* { nested /* } */ */\n    let l: &'static str = s;\n}\n";
        let (tokens, comments) = lex(src).unwrap();
        let opens = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Punct('{'))
            .count();
        let closes = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Punct('}'))
            .count();
        assert_eq!((opens, closes), (1, 1));
        assert_eq!(comments.len(), 1);
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Lifetime));
    }

    #[test]
    fn test_modules_are_found_and_kernel_code_is_not() {
        let src = "pub fn kernel(x: i32) -> i32 {\n    if x > 0 { 1 } else { 2 }\n}\n\n\
                   #[cfg(test)]\n#[allow(clippy::all)]\nmod tests {\n    #[test]\n    fn t() {\n        \
                   if true {}\n    }\n}\n";
        let s = scan(src).unwrap();
        assert_eq!(s.test_lines, vec![(5, 12)]);
        assert!(s.has_functions);
        assert_eq!(s.branch_sites, vec![2]);
        assert!(s.is_test_line(9) && !s.is_test_line(2));
    }

    #[test]
    fn out_of_line_test_modules_are_named() {
        let src = "pub fn f() {}\n#[cfg(test)]\nmod tests;\n#[cfg(test)]\nuse std::fmt;\n";
        let s = scan(src).unwrap();
        assert_eq!(s.test_modules, vec!["tests".to_owned()]);
        assert_eq!(s.test_lines, vec![(2, 3), (4, 5)]);
    }

    #[test]
    fn only_the_exact_attribute_is_test_code() {
        let src = "#[cfg(all(test, unix))]\nmod t {\n    fn f() {}\n}\n";
        let s = scan(src).unwrap();
        assert_eq!(s.test_lines, Vec::<(usize, usize)>::new());
        assert!(s.has_functions);
    }

    #[test]
    fn coverage_off_marks_carry_their_citation() {
        let src = "// Unreachable: the rung difference is at most 2.\n\
                   // proof: projects/x/proofs.rs::pow25_rungs\n\
                   #[cfg_attr(coverage_nightly, coverage(off))]\n\
                   #[inline]\nfn pow25_other() -> Option<i128> {\n    None\n}\n\n\
                   #[coverage(off)]\nfn bare() {}\n";
        let s = scan(src).unwrap();
        assert_eq!(s.coverage_off.len(), 2);
        let first = s.coverage_off.first().unwrap();
        assert_eq!(first.line, 3);
        assert_eq!(
            first.proof.as_deref(),
            Some("projects/x/proofs.rs::pow25_rungs")
        );
        assert_eq!(first.item, "fn pow25_other() -> Option<i128> {");
        let second = s.coverage_off.get(1).unwrap();
        assert_eq!(second.proof, None);
        assert_eq!(second.item, "fn bare() {}");
    }

    #[test]
    fn unreadable_files_are_errors() {
        assert!(scan("fn a() { let s = \"open; }").is_err());
        assert!(scan("/* never closed").is_err());
        assert!(scan("#[cfg(test)]\nmod t {\n").is_err());
    }

    #[test]
    fn function_pointer_types_are_not_functions() {
        let s = scan("pub struct S { f: fn(u32) -> u32 }\npub const X: u32 = 1;\n").unwrap();
        assert!(!s.has_functions);
    }
}
