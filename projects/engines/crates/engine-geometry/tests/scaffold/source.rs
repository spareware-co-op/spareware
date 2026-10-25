//! Reading a crate root's own attributes from its Rust source: `no_std`, `forbid(unsafe_code)`,
//! and `extern crate std`, which links `std` into a `no_std` crate again; and, for the merge gate
//! (G12), every place a source file names global state.
//!
//! What it does: lexes a source file and reads the crate root's leading inner attributes, and finds
//! `extern crate std` anywhere in a file. `global_state` finds, with its line, every `static` item,
//! every thread-local (`thread_local!`, `#[thread_local]`, `LocalKey`) and every lazy global
//! (`OnceLock`, `LazyLock`, `OnceCell`, `LazyCell`, `lazy_static`, `once_cell`) a file names
//! (principle 7, chapter 28.8: "A check that rejects statics, thread-locals and lazy globals in
//! kernel crates").
//!
//! What it guarantees: an attribute counts only where rustc applies it to the whole crate: among
//! the crate root's leading inner attributes, never inside a comment, a string or a module. A
//! source this reader cannot lex gives the unsafe answer (not `no_std`, unsafe code not
//! forbidden, `extern crate std` present, global state not ruled out), so a limit of this reader
//! shows as a violation, never as a pass. It reads text: kernel-build.yml's build for a target
//! without `std` is what proves a crate `no_std`. For global state the keyword `static` counts
//! wherever it is the keyword: a `static` item, `static mut`, a `static` in a function body or a
//! macro, never the lifetime `'static` and never the raw identifier `r#static`, which is a name.
//! The type and macro names count as words, raw or not, outside comments and strings.
//!
//! Method: a small lexer after the Rust Reference's lexical structure (line and nested block
//! comments; string, raw string, byte and character literals; lifetimes; raw identifiers;
//! <https://doc.rust-lang.org/reference/tokens.html>) and its rule that a module's inner attributes
//! come before its items (<https://doc.rust-lang.org/reference/attributes.html>). The names are
//! the standard library's (`std::sync::OnceLock`, `std::sync::LazyLock`, `core::cell::OnceCell`,
//! `core::cell::LazyCell`, `std::thread_local!`, `std::thread::LocalKey`) and the two crates that
//! made lazy globals common, `lazy_static` and `once_cell`.
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
    /// An identifier or keyword.
    Word(String),
    /// A raw identifier, `r#name`, without its `r#`: always a name, never a keyword.
    Raw(String),
    /// One punctuation character.
    Punct(char),
    /// A string, character or number literal, as written.
    Literal(String),
}

impl Token {
    fn text(&self) -> String {
        match self {
            Token::Word(text) | Token::Literal(text) => text.clone(),
            Token::Raw(text) => format!("r#{text}"),
            Token::Punct(c) => c.to_string(),
        }
    }

    /// The identifier this token is, raw or not.
    fn name(&self) -> Option<&str> {
        match self {
            Token::Word(text) | Token::Raw(text) => Some(text),
            Token::Punct(_) | Token::Literal(_) => None,
        }
    }
}

/// A token and the line it starts on, counting from 1.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Spanned {
    token: Token,
    line: usize,
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
    Some(lex_spanned(source)?.into_iter().map(|s| s.token).collect())
}

/// `lex`, with the line each token starts on.
fn lex_spanned(source: &str) -> Option<Vec<Spanned>> {
    let chars: Vec<char> = source.chars().collect();
    // The index of every line end, so a token's line is one more than the line ends before it.
    let line_ends: Vec<usize> = chars
        .iter()
        .enumerate()
        .filter(|(_, c)| **c == '\n')
        .map(|(i, _)| i)
        .collect();
    let line_of = |index: usize| line_ends.partition_point(|&end| end < index) + 1;
    let literal = |from: usize, to: usize| Token::Literal(chars[from..to].iter().collect());
    let mut tokens = Vec::new();
    let mut push = |token: Token, start: usize| {
        tokens.push(Spanned {
            token,
            line: line_of(start),
        });
    };
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
            push(literal(at, end), at);
            at = end;
        } else if c == '\'' {
            if let Some(end) = char_literal_end(&chars, at) {
                push(literal(at, end), at);
                at = end;
            } else {
                // A lifetime or a label.
                push(Token::Punct('\''), at);
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
                    push(Token::Raw(chars[name_start..at].iter().collect()), start);
                }
                ("r" | "br" | "cr", Some('"' | '#')) => {
                    let end = raw_string_end(&chars, at)?;
                    push(literal(start, end), start);
                    at = end;
                }
                ("b" | "c", Some('"')) => {
                    let end = string_end(&chars, at + 1)?;
                    push(literal(start, end), start);
                    at = end;
                }
                ("b", Some('\'')) => {
                    let end = char_literal_end(&chars, at)?;
                    push(literal(start, end), start);
                    at = end;
                }
                _ => push(Token::Word(word), start),
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
            push(literal(start, at), start);
        } else {
            push(Token::Punct(c), at);
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
    // `extern` and `crate` are keywords, so only their plain forms count; the crate's name may be
    // written raw (`extern crate r#std`).
    tokens.windows(3).any(|w| {
        matches!(w, [Token::Word(a), Token::Word(b), c]
            if a == "extern" && b == "crate" && c.name() == Some("std"))
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

/// The names that make a thread-local: the standard library's macro, attribute and key type.
pub const THREAD_LOCAL_NAMES: [&str; 2] = ["thread_local", "LocalKey"];

/// The names that make a lazy global: the standard library's lazily initialised cells and the two
/// crates that made lazy globals common.
pub const LAZY_GLOBAL_NAMES: [&str; 6] = [
    "OnceLock",
    "LazyLock",
    "OnceCell",
    "LazyCell",
    "lazy_static",
    "once_cell",
];

/// One place a source file names global state.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlobalUse {
    /// The line it is on, counting from 1.
    pub line: usize,
    /// What it is, in words, naming the token.
    pub what: String,
}

/// Every place `source` names global state, in order (see the module header); `None` when the
/// source does not lex, so it cannot be shown free of global state.
pub fn global_state(source: &str) -> Option<Vec<GlobalUse>> {
    let tokens = lex_spanned(source)?;
    let mut found = Vec::new();
    for (i, spanned) in tokens.iter().enumerate() {
        let after_quote = i
            .checked_sub(1)
            .and_then(|j| tokens.get(j))
            .is_some_and(|before| before.token == Token::Punct('\''));
        let what = match &spanned.token {
            // `'static` is a lifetime and `r#static` a name; only the keyword makes a static.
            Token::Word(word) if word == "static" && !after_quote => "a `static` item".to_owned(),
            Token::Word(name) | Token::Raw(name) if THREAD_LOCAL_NAMES.contains(&name.as_str()) => {
                format!("a thread-local (`{name}`)")
            }
            Token::Word(name) | Token::Raw(name) if LAZY_GLOBAL_NAMES.contains(&name.as_str()) => {
                format!("a lazy global (`{name}`)")
            }
            _ => continue,
        };
        found.push(GlobalUse {
            line: spanned.line,
            what,
        });
    }
    Some(found)
}

/// The lint levels below `deny`: `allow`, `warn`, and `expect`, which allows a lint while asserting
/// it fires.
pub const LOWERING_LEVELS: [&str; 3] = ["allow", "warn", "expect"];

/// One place outside test code where a source file lowers a lint the kernel denies, or holds a
/// lint attribute this reader cannot read.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LintOverride {
    /// The line it is on, counting from 1.
    pub line: usize,
    /// What it is, in words, quoting the attribute's words.
    pub what: String,
}

/// What a source file's lint attributes say, for the merge gate's rule on lint overrides (G12).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LintAttributes {
    /// Every place outside test code that lowers a lint the kernel denies or a lint group, or that
    /// this reader cannot read.
    pub overrides: Vec<LintOverride>,
    /// The modules this file declares as test code kept in files of their own (`#[cfg(test)] mod
    /// name;`), each with the path its `#[path = "..."]` names, if any.
    pub test_modules: Vec<(String, Option<String>)>,
    /// Whether the file is test code as a whole: `#![cfg(test)]` outside any item.
    pub test_file: bool,
}

/// The index of the token that closes the delimiter at `open`; `None` when it is not closed.
fn group_end(tokens: &[Spanned], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, spanned) in tokens.iter().enumerate().skip(open) {
        match spanned.token {
            Token::Punct('[' | '(' | '{') => depth += 1,
            Token::Punct(']' | ')' | '}') => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// `tokens` split at its commas outside any delimiter; empty parts are dropped.
fn split_commas(tokens: &[Spanned]) -> Vec<&[Spanned]> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (i, spanned) in tokens.iter().enumerate() {
        match spanned.token {
            Token::Punct('[' | '(' | '{') => depth += 1,
            Token::Punct(']' | ')' | '}') => depth = depth.saturating_sub(1),
            Token::Punct(',') if depth == 0 => {
                parts.push(&tokens[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&tokens[start..]);
    parts.retain(|p| !p.is_empty());
    parts
}

/// The tokens' text, joined without spaces.
fn joined(tokens: &[Spanned]) -> String {
    tokens.iter().map(|s| s.token.text()).collect()
}

/// Whether a `cfg` predicate holds only in test builds: `test`, or `all(...)` with such an argument.
/// Explicit work list, no recursion.
fn requires_test(predicate: &[Spanned]) -> bool {
    let mut work = vec![predicate];
    while let Some(p) = work.pop() {
        match p {
            [only] if only.token.name() == Some("test") => return true,
            [all, open, .., close]
                if all.token.name() == Some("all")
                    && open.token == Token::Punct('(')
                    && close.token == Token::Punct(')') =>
            {
                work.extend(split_commas(&p[2..p.len() - 1]));
            }
            _ => {}
        }
    }
    false
}

/// Whether an attribute's body (the tokens inside its brackets) makes its item test code:
/// `cfg(test)` (or `cfg(all(test, ...))`), or the `test` attribute.
fn marks_test(body: &[Spanned]) -> bool {
    match body {
        [only] => only.token.name() == Some("test"),
        [cfg, open, predicate @ .., close]
            if cfg.token.name() == Some("cfg")
                && open.token == Token::Punct('(')
                && close.token == Token::Punct(')') =>
        {
            requires_test(predicate)
        }
        _ => false,
    }
}

/// The path an entry of a lint list names (`clippy::unwrap_used`), or `None` when it is not a plain
/// path.
fn lint_path(entry: &[Spanned]) -> Option<String> {
    let mut path = String::new();
    let mut i = 0;
    loop {
        path.push_str(entry.get(i)?.token.name()?);
        i += 1;
        match (entry.get(i), entry.get(i + 1)) {
            (None, _) => return Some(path),
            (Some(a), Some(b)) if a.token == Token::Punct(':') && b.token == Token::Punct(':') => {
                path.push_str("::");
                i += 2;
            }
            _ => return None,
        }
    }
}

/// Whether a lint path is one the kernel denies, or a clippy group that may hold one.
fn protected(path: &str, denied: &[&str], groups: &[&str]) -> bool {
    path.strip_prefix("clippy::").is_some_and(|lint| {
        let lint = lint.replace('-', "_");
        denied.contains(&lint.as_str()) || groups.contains(&lint.as_str())
    })
}

/// Reports, into `found`, what one attribute's body lowers: the entries of every `allow`, `warn`
/// or `expect` list in it, through `cfg_attr` unless its predicate holds only in test builds.
/// Explicit work list, no recursion.
fn check_attribute(
    body: &[Spanned],
    line: usize,
    denied: &[&str],
    groups: &[&str],
    found: &mut Vec<LintOverride>,
) {
    if body.first().is_some_and(|t| t.token == Token::Punct('$')) && body.len() > 2 {
        found.push(LintOverride {
            line,
            what: format!(
                "#[{}] is an attribute a macro assembles, which this reader cannot read; write lint \
                 attributes out",
                joined(body)
            ),
        });
        return;
    }
    let mut queue = vec![body];
    while let Some(attr) = queue.pop() {
        let [head, open, inner @ .., close] = attr else {
            continue;
        };
        if open.token != Token::Punct('(') || close.token != Token::Punct(')') {
            continue;
        }
        let Some(word) = head.token.name() else {
            continue;
        };
        if word == "cfg_attr" {
            let parts = split_commas(inner);
            if let Some((predicate, attrs)) = parts.split_first()
                && !requires_test(predicate)
            {
                queue.extend(attrs.iter().copied());
            }
            continue;
        }
        if !LOWERING_LEVELS.contains(&word) {
            continue;
        }
        for entry in split_commas(inner) {
            let is_reason = entry
                .first()
                .is_some_and(|t| t.token.name() == Some("reason"))
                && entry.get(1).is_some_and(|t| t.token == Token::Punct('='));
            if is_reason {
                continue;
            }
            let what = match lint_path(entry) {
                Some(path) if protected(&path, denied, groups) => format!(
                    "{word}({path}) lowers a lint the kernel denies, or a group that may hold one"
                ),
                Some(_) => continue,
                None => format!(
                    "{word}({}) names a lint this reader cannot read as a path",
                    joined(entry)
                ),
            };
            found.push(LintOverride { line, what });
        }
    }
}

/// Reports, into `found`, every `allow(...)`, `warn(...)` or `expect(...)` outside an attribute,
/// as a macro's arguments carry one to an attribute, whose list names a protected lint or a
/// clippy path the reader cannot read (`clippy::$lint`).
fn check_loose_list(
    tokens: &[Spanned],
    at: usize,
    denied: &[&str],
    groups: &[&str],
    found: &mut Vec<LintOverride>,
) {
    let Some(word) = tokens.get(at).and_then(|t| t.token.name()) else {
        return;
    };
    if !LOWERING_LEVELS.contains(&word)
        || tokens.get(at + 1).map(|t| &t.token) != Some(&Token::Punct('('))
    {
        return;
    }
    let Some(close) = group_end(tokens, at + 1) else {
        return;
    };
    for entry in split_commas(&tokens[at + 2..close]) {
        let clippy = entry
            .first()
            .is_some_and(|t| t.token.name() == Some("clippy"));
        let what = match lint_path(entry) {
            Some(path) if protected(&path, denied, groups) => {
                format!(
                    "{word}({path}) outside an attribute, as a macro carries one to an attribute"
                )
            }
            None if clippy => format!(
                "{word}({}) names a lint this reader cannot read as a path",
                joined(entry)
            ),
            _ => continue,
        };
        found.push(LintOverride {
            line: tokens[at].line,
            what,
        });
    }
}

/// An item's outer attributes, read: whether they make it test code, the brace depth they were
/// read at, and the path a `#[path = "..."]` among them names.
struct Pending {
    test: bool,
    depth: usize,
    path: Option<String>,
}

/// A run of attributes: each one's body (the tokens inside its brackets) and line.
type Run<'a> = Vec<(&'a [Spanned], usize)>;

/// The run of outer attributes (`#[...]`, one after another) that starts at `at`, and the index
/// after the run; `None` when one does not close.
fn outer_run(tokens: &[Spanned], at: usize) -> Option<(Run<'_>, usize)> {
    let mut run = Vec::new();
    let mut next = at;
    while tokens.get(next).map(|t| &t.token) == Some(&Token::Punct('#'))
        && tokens.get(next + 1).map(|t| &t.token) == Some(&Token::Punct('['))
    {
        let close = group_end(tokens, next + 1)?;
        run.push((&tokens[next + 2..close], tokens[next].line));
        next = close + 1;
    }
    Some((run, next))
}

/// The path a `#[path = "..."]` among a run of attributes names.
fn path_attribute(run: &[(&[Spanned], usize)]) -> Option<String> {
    run.iter().find_map(|(body, _)| match body {
        [word, eq, value] if word.token.name() == Some("path") && eq.token == Token::Punct('=') => {
            match &value.token {
                Token::Literal(literal) => literal
                    .strip_prefix('"')
                    .and_then(|t| t.strip_suffix('"'))
                    .map(str::to_owned),
                _ => None,
            }
        }
        _ => None,
    })
}

/// What `source` says about lints, outside test code (see `LintAttributes`); `None` when it does
/// not lex or a delimiter is not closed, so nothing can be shown about it.
///
/// Test code is what only a test build compiles: an item whose outer attributes include
/// `#[cfg(test)]` (or `cfg(all(test, ...))`) or `#[test]`, everything inside its body, a block or
/// module after `#![cfg(test)]`, and a `cfg_attr` whose predicate needs `test`. An item's body is
/// the first `{` read at the depth of its attributes before a `;` or `,` there ends it; a `{` in
/// the item's signature before its body (rare: a const argument) makes this reader treat the body
/// as code that ships, which reports more, never less.
pub fn lint_attributes(source: &str, denied: &[&str], groups: &[&str]) -> Option<LintAttributes> {
    let tokens = lex_spanned(source)?;
    let mut out = LintAttributes::default();
    // One entry per open delimiter: whether the code inside it is test code.
    let mut open: Vec<bool> = Vec::new();
    let mut pending: Option<Pending> = None;
    let mut at = 0;
    while let Some(spanned) = tokens.get(at) {
        let in_test = out.test_file || open.iter().any(|t| *t);
        match spanned.token {
            Token::Punct('#') => {
                let inner = tokens.get(at + 1).map(|t| &t.token) == Some(&Token::Punct('!'));
                let bracket = if inner { at + 2 } else { at + 1 };
                if tokens.get(bracket).map(|t| &t.token) != Some(&Token::Punct('[')) {
                    at += 1;
                    continue;
                }
                let close = group_end(&tokens, bracket)?;
                let body = &tokens[bracket + 1..close];
                if inner {
                    if marks_test(body) {
                        match open.last_mut() {
                            Some(region) => *region = true,
                            None => out.test_file = true,
                        }
                    } else if !in_test {
                        check_attribute(body, spanned.line, denied, groups, &mut out.overrides);
                    }
                    at = close + 1;
                    continue;
                }
                // A run of outer attributes, which apply to one item together.
                let (run, next) = outer_run(&tokens, at)?;
                let test_item = run.iter().any(|(body, _)| marks_test(body));
                if !test_item && !in_test {
                    for (body, line) in &run {
                        check_attribute(body, *line, denied, groups, &mut out.overrides);
                    }
                }
                pending = Some(Pending {
                    test: test_item,
                    depth: open.len(),
                    path: path_attribute(&run),
                });
                at = next;
                continue;
            }
            Token::Punct('{') => {
                let body_of_test_item = pending
                    .as_ref()
                    .is_some_and(|p| p.test && p.depth == open.len());
                if pending.as_ref().is_some_and(|p| p.depth == open.len()) {
                    pending = None;
                }
                open.push(body_of_test_item);
            }
            Token::Punct('(' | '[') => open.push(false),
            Token::Punct('}' | ')' | ']') => {
                open.pop()?;
                if pending.as_ref().is_some_and(|p| p.depth > open.len()) {
                    pending = None;
                }
            }
            Token::Punct(';' | ',') => {
                if pending.as_ref().is_some_and(|p| p.depth == open.len()) {
                    pending = None;
                }
            }
            Token::Word(ref word) if word == "mod" => {
                // `#[cfg(test)] mod name;`: a test module in a file of its own.
                if let (Some(p), Some(name), Some(semi)) =
                    (pending.as_ref(), tokens.get(at + 1), tokens.get(at + 2))
                    && p.test
                    && p.depth == open.len()
                    && semi.token == Token::Punct(';')
                    && let Some(name) = name.token.name()
                {
                    out.test_modules.push((name.to_owned(), p.path.clone()));
                }
            }
            _ => {
                if !in_test {
                    check_loose_list(&tokens, at, denied, groups, &mut out.overrides);
                }
            }
        }
        at += 1;
    }
    if !open.is_empty() {
        return None;
    }
    out.overrides.sort();
    Some(out)
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
        assert!(!names_extern_crate_std("r#extern crate std;"));
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

    fn found(source: &str) -> Vec<(usize, String)> {
        global_state(source)
            .unwrap()
            .into_iter()
            .map(|g| (g.line, g.what))
            .collect()
    }

    #[test]
    fn every_kind_of_global_state_is_found_with_its_line() {
        let source = "//! Header.\n\
            static TABLE: [u8; 2] = [1, 2];\n\
            pub static mut COUNT: u32 = 0;\n\
            pub fn f() -> u32 { static INNER: u32 = 1; INNER }\n\
            thread_local! { static T: u8 = 0; }\n\
            #[thread_local] pub static U: u8 = 0;\n\
            fn g() -> std::sync::OnceLock<u8> { std::sync::OnceLock::new() }\n\
            fn h(_: std::thread::LocalKey<u8>) {}\n\
            use core::cell::{LazyCell, OnceCell};\n\
            use std::sync::LazyLock as L;\n\
            lazy_static::lazy_static! {}\n\
            use once_cell::sync::Lazy;\n\
            type R = r#OnceLock<u8>;\n\
            macro_rules! m { () => { static M: u8 = 0; }; }\n";
        assert_eq!(
            found(source),
            [
                (2, "a `static` item"),
                (3, "a `static` item"),
                (4, "a `static` item"),
                (5, "a thread-local (`thread_local`)"),
                (5, "a `static` item"),
                (6, "a thread-local (`thread_local`)"),
                (6, "a `static` item"),
                (7, "a lazy global (`OnceLock`)"),
                (7, "a lazy global (`OnceLock`)"),
                (8, "a thread-local (`LocalKey`)"),
                (9, "a lazy global (`LazyCell`)"),
                (9, "a lazy global (`OnceCell`)"),
                (10, "a lazy global (`LazyLock`)"),
                (11, "a lazy global (`lazy_static`)"),
                (11, "a lazy global (`lazy_static`)"),
                (12, "a lazy global (`once_cell`)"),
                (13, "a lazy global (`OnceLock`)"),
                (14, "a `static` item"),
            ]
            .map(|(line, what)| (line, what.to_owned()))
        );
    }

    #[test]
    fn lifetimes_names_comments_and_strings_are_not_global_state() {
        let source = "//! A `static` table, a OnceLock and thread_local! in a comment.\n\
            /* static X: u8 = 0; LazyLock */\n\
            pub fn f(s: &'static str) -> &'static str { s }\n\
            pub fn g<T: 'static>(t: T) -> T { t }\n\
            pub const TABLE: [u8; 2] = [1, 2];\n\
            pub fn h() -> &'static str { \"static OnceLock thread_local!\" }\n\
            pub fn i() -> char { 's' }\n\
            pub fn j() { let r#static = 1; let _ = r#static; }\n\
            pub fn k() -> &'static [u8] { br#\"static\"# }\n\
            pub struct Staticky; pub fn l(once_cell_count: u8) -> u8 { once_cell_count }\n";
        assert_eq!(found(source), Vec::<(usize, String)>::new());
        // A source that does not lex cannot be shown free of global state.
        assert_eq!(global_state("pub fn f() {}\n/* open"), None);
        assert_eq!(global_state("const S: &str = \"open;\n"), None);
    }

    const DENIED: [&str; 2] = ["unwrap_used", "disallowed_methods"];
    const GROUPS: [&str; 3] = ["all", "restriction", "style"];

    fn overrides(source: &str) -> Vec<(usize, String)> {
        lint_attributes(source, &DENIED, &GROUPS)
            .unwrap()
            .overrides
            .into_iter()
            .map(|o| (o.line, o.what))
            .collect()
    }

    #[test]
    fn every_way_to_lower_a_denied_lint_outside_tests_is_found() {
        let source = "//! Header.\n\
            #![allow(clippy::unwrap_used)]\n\
            #[allow(clippy::disallowed_methods, reason = \"x\")]\n\
            fn f() {}\n\
            #[cfg_attr(feature = \"x\", warn(clippy::restriction))]\n\
            fn g() { #[expect(clippy::all)] let _x = 1; }\n\
            #[allow(clippy::many_single_char_names, dead_code)]\n\
            fn h() {}\n\
            fn i(#[cfg(test)] _a: u8) { #[allow(clippy::unwrap_used)] let _y = 1; }\n\
            macro_rules! m { ($l:ident) => { #[allow(clippy::$l)] fn z() {} }; }\n\
            macro_rules! k { ($v:ident) => { #[$v(clippy::unwrap_used)] fn y() {} }; }\n\
            n!(allow(clippy::unwrap_used));\n\
            #[cfg(any(test, feature = \"x\"))] mod a { #![allow(clippy::style)] }\n\
            #[cfg(not(test))] mod b { #![warn(clippy::unwrap_used)] }\n";
        let found = overrides(source);
        let lines: Vec<usize> = found.iter().map(|(line, _)| *line).collect();
        assert_eq!(lines, [2, 3, 5, 6, 9, 10, 11, 12, 13, 14], "{found:#?}");
        assert!(
            found[0].1.starts_with("allow(clippy::unwrap_used) lowers"),
            "{found:#?}"
        );
        assert!(found[5].1.contains("cannot read"), "{found:#?}");
        assert!(found[6].1.contains("a macro assembles"), "{found:#?}");
        assert!(found[7].1.contains("outside an attribute"), "{found:#?}");
    }

    #[test]
    fn test_code_comments_and_strings_may_lower_lints() {
        let source = "//! #![allow(clippy::unwrap_used)] in a comment.\n\
            const S: &str = \"#[allow(clippy::unwrap_used)]\";\n\
            #[cfg(test)]\n\
            #[allow(clippy::disallowed_methods, reason = \"tests\")]\n\
            mod tests { #[allow(clippy::unwrap_used)] fn t() { { #![allow(clippy::all)] } } }\n\
            #[allow(clippy::unwrap_used)] #[test] fn t2() {}\n\
            #[cfg(all(feature = \"x\", test))] mod t3 { #![allow(clippy::restriction)] }\n\
            #[cfg_attr(test, allow(clippy::unwrap_used))] fn t4() {}\n\
            mod t5 { #![cfg(test)] #![allow(clippy::unwrap_used)] }\n\
            #[cfg(test)] mod external;\n\
            #[cfg(test)] #[path = \"other.rs\"] mod external2;\n\
            mod shipped;\n\
            pub fn after() -> Option<u8> { Some(1) }\n";
        let read = lint_attributes(source, &DENIED, &GROUPS).unwrap();
        assert_eq!(read.overrides, Vec::new());
        assert!(!read.test_file);
        assert_eq!(
            read.test_modules,
            [
                ("external".to_owned(), None),
                ("external2".to_owned(), Some("other.rs".to_owned()))
            ]
        );
        // A file that is test code as a whole.
        let read = lint_attributes(
            "#![cfg(test)]\n#![allow(clippy::unwrap_used)]\n",
            &DENIED,
            &GROUPS,
        )
        .unwrap();
        assert!(read.test_file && read.overrides.is_empty());
        // After a test item's body closes, the code ships again.
        assert_eq!(
            overrides("#[cfg(test)] fn t() {}\n#[allow(clippy::unwrap_used)] fn f() {}\n").len(),
            1
        );
        // A source that does not lex, or whose delimiters do not close, cannot be read.
        assert_eq!(lint_attributes("/* open", &DENIED, &GROUPS), None);
        assert_eq!(lint_attributes("fn f() {", &DENIED, &GROUPS), None);
        assert_eq!(
            lint_attributes("#[allow(clippy::unwrap_used", &DENIED, &GROUPS),
            None
        );
    }
}
