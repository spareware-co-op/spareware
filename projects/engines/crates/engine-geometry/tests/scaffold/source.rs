//! Reading a crate root's own attributes from its Rust source: `no_std`, `forbid(unsafe_code)`,
//! and `extern crate std`, which links `std` into a `no_std` crate again.
//!
//! What it does: lexes a source file and reads the crate root's leading inner attributes, and finds
//! `extern crate std` anywhere in a file.
//!
//! What it guarantees: an attribute counts only where rustc applies it to the whole crate: among
//! the crate root's leading inner attributes, never inside a comment, a string or a module. A
//! source this reader cannot lex gives the unsafe answer (not `no_std`, unsafe code not
//! forbidden, `extern crate std` present), so a limit of this reader shows as a violation, never
//! as a pass. It reads text: kernel-build.yml's build for a target without `std` is what proves a
//! crate `no_std`.
//!
//! Method: a small lexer after the Rust Reference's lexical structure (line and nested block
//! comments; string, raw string, byte and character literals; lifetimes;
//! <https://doc.rust-lang.org/reference/tokens.html>) and its rule that a module's inner attributes
//! come before its items (<https://doc.rust-lang.org/reference/attributes.html>).
//!
//! Cost: linear in the length of the file; no recursion.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The kernel lints deny native arithmetic in kernel code (Cargo.toml). This is test code: its
// readers count positions in a text they hold whole, which tests may do with native arithmetic.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "test code; the rule is for kernel code"
)]

/// The crate-level attributes the check needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Attributes {
    /// The crate does not link `std`: `#![no_std]`, or `#![cfg_attr(not(test), no_std)]` (which
    /// keeps `std` for unit tests only), among the crate root's leading attributes, and no
    /// `extern crate std` in the file.
    pub no_std: bool,
    /// `#![forbid(unsafe_code)]`, alone or in a list, among the crate root's leading attributes.
    pub forbids_unsafe: bool,
}

/// One token of Rust source, as far as this reader needs: words, punctuation, and literals kept
/// whole, so the text inside a string never reads as code.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    /// An identifier or keyword; a raw identifier without its `r#`.
    Word(String),
    /// One punctuation character.
    Punct(char),
    /// A string, character or number literal, as written.
    Literal(String),
}

impl Token {
    fn text(&self) -> String {
        match self {
            Token::Word(text) | Token::Literal(text) => text.clone(),
            Token::Punct(c) => c.to_string(),
        }
    }
}

fn is_word_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

fn is_word_char(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

/// The index after a block comment that opens at `at` (`/*`), counting nested comments; `None`
/// when it is not closed.
fn block_comment_end(chars: &[char], at: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = at;
    loop {
        match (chars.get(i)?, chars.get(i + 1)) {
            ('/', Some('*')) => {
                depth += 1;
                i += 2;
            }
            ('*', Some('/')) => {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => i += 1,
        }
    }
}

/// The index after a quoted string whose opening quote is just before `from`; `None` when it is
/// not closed.
fn string_end(chars: &[char], from: usize) -> Option<usize> {
    let mut i = from;
    loop {
        match chars.get(i)? {
            '\\' => i += 2,
            '"' => return Some(i + 1),
            _ => i += 1,
        }
    }
}

/// The index after a raw string whose hashes or quote start at `at`; `None` when there is no raw
/// string there or it is not closed.
fn raw_string_end(chars: &[char], at: usize) -> Option<usize> {
    let mut i = at;
    while chars.get(i) == Some(&'#') {
        i += 1;
    }
    let hashes = i - at;
    if chars.get(i) != Some(&'"') {
        return None;
    }
    i += 1;
    loop {
        if *chars.get(i)? == '"' && (1..=hashes).all(|k| chars.get(i + k) == Some(&'#')) {
            return Some(i + 1 + hashes);
        }
        i += 1;
    }
}

/// The index after a character literal whose quote is at `at`, or `None` when the quote opens a
/// lifetime or a label instead.
fn char_literal_end(chars: &[char], at: usize) -> Option<usize> {
    if chars.get(at + 1) == Some(&'\\') {
        let mut i = at + 3;
        while chars.get(i).is_some_and(|&c| c != '\'' && c != '\n') {
            i += 1;
        }
        (chars.get(i) == Some(&'\'')).then_some(i + 1)
    } else if chars.get(at + 2) == Some(&'\'') {
        Some(at + 3)
    } else {
        None
    }
}

/// The tokens of `source`, comments dropped; `None` when a comment, string or character literal
/// is not closed, or a raw string is malformed.
fn lex(source: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = source.chars().collect();
    let literal = |from: usize, to: usize| Token::Literal(chars[from..to].iter().collect());
    let mut tokens = Vec::new();
    let mut at = 0;
    while let Some(&c) = chars.get(at) {
        let next = chars.get(at + 1).copied();
        if c.is_whitespace() {
            at += 1;
        } else if c == '/' && next == Some('/') {
            while chars.get(at).is_some_and(|&c| c != '\n') {
                at += 1;
            }
        } else if c == '/' && next == Some('*') {
            at = block_comment_end(&chars, at)?;
        } else if c == '"' {
            let end = string_end(&chars, at + 1)?;
            tokens.push(literal(at, end));
            at = end;
        } else if c == '\'' {
            if let Some(end) = char_literal_end(&chars, at) {
                tokens.push(literal(at, end));
                at = end;
            } else {
                // A lifetime or a label.
                tokens.push(Token::Punct('\''));
                at += 1;
            }
        } else if is_word_start(c) {
            let start = at;
            while chars.get(at).copied().is_some_and(is_word_char) {
                at += 1;
            }
            let word: String = chars[start..at].iter().collect();
            let after = chars.get(at).copied();
            let after_next = chars.get(at + 1).copied();
            match (word.as_str(), after) {
                ("r", Some('#')) if after_next.is_some_and(is_word_start) => {
                    // A raw identifier, `r#name`.
                    let name_start = at + 1;
                    at = name_start;
                    while chars.get(at).copied().is_some_and(is_word_char) {
                        at += 1;
                    }
                    tokens.push(Token::Word(chars[name_start..at].iter().collect()));
                }
                ("r" | "br" | "cr", Some('"' | '#')) => {
                    let end = raw_string_end(&chars, at)?;
                    tokens.push(literal(start, end));
                    at = end;
                }
                ("b" | "c", Some('"')) => {
                    let end = string_end(&chars, at + 1)?;
                    tokens.push(literal(start, end));
                    at = end;
                }
                ("b", Some('\'')) => {
                    let end = char_literal_end(&chars, at)?;
                    tokens.push(literal(start, end));
                    at = end;
                }
                _ => tokens.push(Token::Word(word)),
            }
        } else if c.is_ascii_digit() {
            let start = at;
            loop {
                match (chars.get(at).copied(), chars.get(at + 1).copied()) {
                    (Some(d), _) if is_word_char(d) => at += 1,
                    (Some('.'), Some(d)) if d.is_ascii_digit() => at += 1,
                    _ => break,
                }
            }
            tokens.push(literal(start, at));
        } else {
            tokens.push(Token::Punct(c));
            at += 1;
        }
    }
    Some(tokens)
}

/// The crate root's leading inner attributes (`#![...]`), each as its text inside the brackets
/// without whitespace. Inner attributes must come before a module's items, so these are all the
/// attributes that apply to the whole crate; reading stops at the first other token.
fn leading_inner_attributes(tokens: &[Token]) -> Vec<String> {
    let mut found = Vec::new();
    let mut at = 0;
    while tokens.get(at) == Some(&Token::Punct('#'))
        && tokens.get(at + 1) == Some(&Token::Punct('!'))
        && tokens.get(at + 2) == Some(&Token::Punct('['))
    {
        let mut depth = 0usize;
        let mut text = String::new();
        let mut i = at + 2;
        let end = loop {
            let Some(token) = tokens.get(i) else {
                // Not closed: it does not count, and nothing after it is read.
                return found;
            };
            match token {
                Token::Punct('[' | '(' | '{') => depth += 1,
                Token::Punct(']' | ')' | '}') => {
                    if depth == 0 {
                        return found;
                    }
                    depth -= 1;
                }
                _ => {}
            }
            if depth == 0 {
                break i;
            }
            if i > at + 2 {
                text.push_str(&token.text());
            }
            i += 1;
        };
        found.push(text);
        at = end + 1;
    }
    found
}

fn names_std(tokens: &[Token]) -> bool {
    tokens.windows(3).any(|w| {
        matches!(w, [Token::Word(a), Token::Word(b), Token::Word(c)]
            if a == "extern" && b == "crate" && c == "std")
    })
}

/// Reads the crate-level attributes of a `lib.rs`. A source this reader cannot lex has neither.
pub fn attributes(source: &str) -> Attributes {
    let Some(tokens) = lex(source) else {
        return Attributes::default();
    };
    let leading = leading_inner_attributes(&tokens);
    let no_std = leading
        .iter()
        .any(|a| a == "no_std" || a == "cfg_attr(not(test),no_std)");
    let forbids_unsafe = leading.iter().any(|a| {
        a.strip_prefix("forbid(")
            .and_then(|r| r.strip_suffix(')'))
            .is_some_and(|list| list.split(',').any(|lint| lint == "unsafe_code"))
    });
    Attributes {
        no_std: no_std && !names_std(&tokens),
        forbids_unsafe,
    }
}

/// Whether a source file declares `extern crate std` outside comments and strings, which links
/// `std` whatever the crate root says. A source this reader cannot lex counts as declaring it.
pub fn names_extern_crate_std(source: &str) -> bool {
    lex(source).is_none_or(|tokens| names_std(&tokens))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scaffold_attributes() {
        let source = "//! Spare Kit geometry kernel: a thing.\n//!\n//! One job: things.\n\
            #![cfg_attr(not(test), no_std)]\n#![forbid(unsafe_code)]\n";
        assert_eq!(
            attributes(source),
            Attributes {
                no_std: true,
                forbids_unsafe: true
            }
        );
    }

    #[test]
    fn attributes_need_the_exact_forms() {
        assert_eq!(
            attributes("#![no_std]\n#![forbid(missing_docs, unsafe_code)]\n"),
            Attributes {
                no_std: true,
                forbids_unsafe: true,
            }
        );
        assert_eq!(
            attributes(
                "// #![no_std]\n#![deny(unsafe_code)]\n#![cfg_attr(feature = \"x\", no_std)]\n"
            ),
            Attributes::default()
        );
    }

    #[test]
    fn only_the_crate_roots_leading_attributes_count() {
        let both = Attributes {
            no_std: true,
            forbids_unsafe: true,
        };
        // Inside a module, after an item, inside a block or nested comment, inside a string.
        for source in [
            "pub mod inner {\n    #![no_std]\n    #![forbid(unsafe_code)]\n}\n",
            "pub fn f() {}\n#![no_std]\n#![forbid(unsafe_code)]\n",
            "/*\n#![no_std]\n#![forbid(unsafe_code)]\n*/\npub fn f() {}\n",
            "/* outer /* inner */ #![no_std] #![forbid(unsafe_code)] */\n",
            "const S: &str = \"\n#![no_std]\n#![forbid(unsafe_code)]\n\";\n",
            "const S: &str = r#\"\n#![no_std]\n\"#;\n#![forbid(unsafe_code)]\n",
            "#[doc = \"x\"]\n#![no_std]\n#![forbid(unsafe_code)]\n",
        ] {
            assert_eq!(attributes(source), Attributes::default(), "{source}");
        }
        // Comments, doc comments and blank lines before them are fine.
        for source in [
            "//! Header.\n/* note */\n\n#![cfg_attr(not(test), no_std)]\n#![forbid(unsafe_code)]\n",
            "#![forbid(unsafe_code)]\n#![doc = \"a ] in a string\"]\n#![no_std]\npub fn f() {}\n",
        ] {
            assert_eq!(attributes(source), both, "{source}");
        }
        // A source that does not lex has neither.
        assert_eq!(attributes("#![no_std]\n/* open"), Attributes::default());
    }

    #[test]
    fn extern_crate_std_brings_std_back() {
        let source = "#![no_std]\n#![forbid(unsafe_code)]\nmod m {\n    extern crate std;\n}\n";
        assert_eq!(
            attributes(source),
            Attributes {
                no_std: false,
                forbids_unsafe: true,
            }
        );
        assert!(names_extern_crate_std("extern crate std as s;"));
        assert!(names_extern_crate_std("extern crate r#std;"));
        assert!(names_extern_crate_std("\"unclosed"));
        for clean in [
            "// extern crate std;\n",
            "/* extern crate std; */\n",
            "const S: &str = \"extern crate std\";\n",
            "extern crate alloc;\nfn f<'a>(x: &'a str) -> char { 'x' }\n",
            "const C: char = '\\''; const B: u8 = b'\"'; // extern crate std\n",
        ] {
            assert!(!names_extern_crate_std(clean), "{clean}");
        }
    }
}
