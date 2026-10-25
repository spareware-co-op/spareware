//! Reading Rust source for the scaffold check and the merge gate: a crate root's own attributes
//! (`no_std`, `forbid(unsafe_code)`), `extern crate std`, which links `std` into a `no_std` crate
//! again, every place a source file names global state, and every lint attribute that lowers a
//! lint the kernel denies outside test code.
//!
//! What it does: `attributes` parses a crate root with `syn` and reads its leading inner
//! attributes, the ones rustc applies to the whole crate. `names_extern_crate_std` and
//! `global_state` read a file's tokens, from `proc-macro2`, the lexer `syn` itself parses with:
//! `extern crate std` anywhere, macro bodies included, and, with its line, every `static` item,
//! every thread-local (`thread_local!`, `#[thread_local]`, `LocalKey`) and every lazy global
//! (`OnceLock`, `LazyLock`, `OnceCell`, `LazyCell`, `lazy_static`, `once_cell`) a file names
//! (principle 7, chapter 28.8: "A check that rejects statics, thread-locals and lazy globals in
//! kernel crates"). `lint_attributes` parses a file with `syn` and walks its syntax tree: every
//! `allow`, `warn` or `expect` attribute outside test code that names a lint the caller protects,
//! read through `cfg_attr`, and inside macro invocations and definitions every attribute and every
//! `allow(...)`, `warn(...)` or `expect(...)` list a macro could carry to an attribute.
//!
//! What it guarantees: an attribute counts only where rustc applies it, as the parser places it:
//! a crate's attributes only among the crate root's leading inner attributes, never inside a
//! comment, a string or a module. A source the parser or the lexer refuses gives the unsafe answer
//! (not `no_std`, unsafe code not forbidden, `extern crate std` present, global state not ruled
//! out, lint attributes unreadable), so a malformed file shows as a violation, never as a pass.
//! For global state the keyword `static` counts wherever it is the keyword: a `static` item,
//! `static mut`, a `static` in a function body or a macro, never the lifetime `'static` and never
//! the raw identifier `r#static`, which is a name. The type and macro names count as identifiers,
//! raw or not, outside comments and strings. Test code is what only a test build compiles: an
//! item, statement, expression, field, variant, parameter or match arm whose attributes include
//! `#[cfg(test)]` (or `cfg(all(test, ...))`) or `#[test]`, all of it, a module or block whose inner
//! attributes include `#![cfg(test)]`, a file that opens with `#![cfg(test)]`, and a `cfg_attr`
//! whose predicate needs `test`. Inside a macro's tokens nothing is test code, so a lowering
//! attribute there is reported even under a `cfg(test)` the macro writes: more, never less.
//!
//! Method: the `syn` crate's parser and syntax-tree visitor (<https://docs.rs/syn/3.0.9/syn/>) and
//! the `proc-macro2` crate's lexer with line numbers (<https://docs.rs/proc-macro2/1.0.107/>), after
//! the Rust Reference's rules on attributes, lint-check attributes and `cfg`
//! (<https://doc.rust-lang.org/reference/attributes.html>,
//! <https://doc.rust-lang.org/reference/attributes/diagnostics.html#lint-check-attributes>,
//! <https://doc.rust-lang.org/reference/conditional-compilation.html>); token trees are walked with
//! an explicit stack, no recursion. Piece G12a put the two crates in place of a lexer written here
//! (chapter 29.1, the owner's direction of 2026-10-22: use well-known permissive crates rather
//! than hand-rolling). The names are the standard library's (`std::sync::OnceLock`,
//! `std::sync::LazyLock`, `core::cell::OnceCell`, `core::cell::LazyCell`, `std::thread_local!`,
//! `std::thread::LocalKey`) and the two crates that made lazy globals common, `lazy_static` and
//! `once_cell`.
//!
//! Cost: linear in the length of the file, for the lexer and for the parse. `syn`'s parser
//! recurses as deep as the source nests, which rustc's own parser bounds for every file that
//! compiles.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use proc_macro2::{Delimiter, Spacing, Span, TokenStream, TokenTree};
use syn::ext::IdentExt as _;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, Lit, Meta, Token};

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

/// The tokens of `source`, by `proc-macro2`'s lexer; `None` when it does not lex (a comment, a
/// literal or a delimiter that does not close).
fn lex(source: &str) -> Option<TokenStream> {
    source.parse().ok()
}

/// The line, counting from 1, where `span` starts.
fn line(span: Span) -> usize {
    span.start().line
}

/// Every token of `stream`, in source order, groups opened in place: each with whether the token
/// before it in its own group is a `'` joined to it, which makes an identifier a lifetime or a
/// label. A group's delimiters are not tokens here; its contents follow it. Explicit stack.
fn flatten(stream: TokenStream) -> Vec<(TokenTree, bool)> {
    let mut out = Vec::new();
    let mut stack = vec![(stream.into_iter(), false)];
    while let Some((tokens, after_quote)) = stack.last_mut() {
        let Some(token) = tokens.next() else {
            stack.pop();
            continue;
        };
        let quoted = *after_quote;
        *after_quote = matches!(&token, TokenTree::Punct(p) if p.as_char() == '\'' && p.spacing() == Spacing::Joint);
        if let TokenTree::Group(group) = &token {
            let inner = group.stream().into_iter();
            out.push((token.clone(), quoted));
            stack.push((inner, false));
        } else {
            out.push((token, quoted));
        }
    }
    out
}

/// An identifier's name without `r#`, and whether it was written raw.
fn ident_name(token: &TokenTree) -> Option<(String, bool)> {
    let TokenTree::Ident(ident) = token else {
        return None;
    };
    let text = ident.to_string();
    match text.strip_prefix("r#") {
        Some(name) => Some((name.to_owned(), true)),
        None => Some((text, false)),
    }
}

/// Whether a token stream says `extern crate std` (the crate's name raw or not) in any group.
fn names_std(stream: TokenStream) -> bool {
    let tokens: Vec<TokenTree> = flatten(stream).into_iter().map(|(t, _)| t).collect();
    tokens.windows(3).any(|w| {
        let plain = |t: &TokenTree, word: &str| ident_name(t) == Some((word.to_owned(), false));
        plain(&w[0], "extern")
            && plain(&w[1], "crate")
            && ident_name(&w[2]).is_some_and(|(name, _)| name == "std")
    })
}

/// Whether an attribute is exactly `no_std`, or `cfg_attr(not(test), no_std)`.
fn is_no_std(attr: &Attribute) -> bool {
    match &attr.meta {
        Meta::Path(path) => path.is_ident("no_std"),
        Meta::List(list) => {
            let tokens: String = list
                .tokens
                .to_string()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            list.path.is_ident("cfg_attr") && tokens == "not(test),no_std"
        }
        Meta::NameValue(_) => false,
    }
}

/// Whether an attribute is `forbid(...)` with `unsafe_code` in its list.
fn forbids_unsafe(attr: &Attribute) -> bool {
    let Meta::List(list) = &attr.meta else {
        return false;
    };
    list.path.is_ident("forbid")
        && list
            .parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
            .is_ok_and(|lints| lints.iter().any(|lint| lint.is_ident("unsafe_code")))
}

/// Reads the crate-level attributes of a `lib.rs`. A source the parser refuses has neither.
pub fn attributes(source: &str) -> Attributes {
    let Ok(file) = syn::parse_file(source) else {
        return Attributes::default();
    };
    let no_std = file.attrs.iter().any(is_no_std);
    let forbids_unsafe = file.attrs.iter().any(forbids_unsafe);
    let std_back = lex(source).is_none_or(names_std);
    Attributes {
        no_std: no_std && !std_back,
        forbids_unsafe,
    }
}

/// Whether a source file declares `extern crate std` outside comments and strings, which links
/// `std` whatever the crate root says. A source the lexer refuses counts as declaring it.
pub fn names_extern_crate_std(source: &str) -> bool {
    lex(source).is_none_or(names_std)
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
    let mut found = Vec::new();
    for (token, after_quote) in flatten(lex(source)?) {
        let Some((name, raw)) = ident_name(&token) else {
            continue;
        };
        let what = if name == "static" && !raw && !after_quote {
            // `'static` is a lifetime and `r#static` a name; only the keyword makes a static.
            "a `static` item".to_owned()
        } else if THREAD_LOCAL_NAMES.contains(&name.as_str()) {
            format!("a thread-local (`{name}`)")
        } else if LAZY_GLOBAL_NAMES.contains(&name.as_str()) {
            format!("a lazy global (`{name}`)")
        } else {
            continue;
        };
        found.push(GlobalUse {
            line: line(token.span()),
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
    /// Whether the file is test code as a whole: `#![cfg(test)]` among its leading attributes.
    pub test_file: bool,
}

/// `tokens` split at its commas (outside any group, which is one token here); empty parts are
/// dropped.
fn split_commas(tokens: &[TokenTree]) -> Vec<&[TokenTree]> {
    tokens
        .split(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == ','))
        .filter(|part| !part.is_empty())
        .collect()
}

/// The tokens' text, joined without spaces.
fn joined(tokens: &[TokenTree]) -> String {
    tokens
        .iter()
        .map(ToString::to_string)
        .collect::<String>()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// A group's contents, when `token` is a group with `delimiter`.
fn group_of(token: &TokenTree, delimiter: Delimiter) -> Option<Vec<TokenTree>> {
    match token {
        TokenTree::Group(g) if g.delimiter() == delimiter => Some(g.stream().into_iter().collect()),
        _ => None,
    }
}

/// Whether a `cfg` predicate holds only in test builds: `test`, or `all(...)` with such an argument.
/// Explicit work list, no recursion.
fn requires_test(predicate: &[TokenTree]) -> bool {
    let mut work: Vec<Vec<TokenTree>> = vec![predicate.to_vec()];
    while let Some(p) = work.pop() {
        match p.as_slice() {
            [only] if ident_name(only).is_some_and(|(n, _)| n == "test") => return true,
            [all, args] if ident_name(all).is_some_and(|(n, _)| n == "all") => {
                if let Some(inner) = group_of(args, Delimiter::Parenthesis) {
                    work.extend(split_commas(&inner).into_iter().map(<[TokenTree]>::to_vec));
                }
            }
            _ => {}
        }
    }
    false
}

/// Whether an attribute makes its item test code: `#[test]`, or `#[cfg(...)]` whose predicate
/// needs `test`.
fn marks_test(attr: &Attribute) -> bool {
    match &attr.meta {
        Meta::Path(path) => path.is_ident("test"),
        Meta::List(list) if list.path.is_ident("cfg") => {
            let tokens: Vec<TokenTree> = list.tokens.clone().into_iter().collect();
            requires_test(&tokens)
        }
        _ => false,
    }
}

/// Whether any of `attrs` makes its node test code.
fn is_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(marks_test)
}

/// The path an entry of a lint list names (`clippy::unwrap_used`, each name without `r#`), parsed
/// by `syn` as a path; `None` when it is not a plain one (no leading `::`, no generic arguments).
fn lint_path(entry: &[TokenTree]) -> Option<String> {
    let path: syn::Path = syn::parse2(entry.iter().cloned().collect()).ok()?;
    if path.leading_colon.is_some() {
        return None;
    }
    let names: Option<Vec<String>> = path
        .segments
        .iter()
        .map(|segment| {
            segment
                .arguments
                .is_none()
                .then(|| segment.ident.unraw().to_string())
        })
        .collect();
    Some(names?.join("::"))
}

/// Whether a lint path is one the kernel denies, or a clippy group that may hold one.
fn protected(path: &str, denied: &[&str], groups: &[&str]) -> bool {
    path.strip_prefix("clippy::").is_some_and(|lint| {
        let lint = lint.replace('-', "_");
        denied.contains(&lint.as_str()) || groups.contains(&lint.as_str())
    })
}

/// What the lint checks need from their caller.
struct Rules<'a> {
    denied: &'a [&'a str],
    groups: &'a [&'a str],
}

impl Rules<'_> {
    /// Reports, into `found`, what one attribute body (its head, then a parenthesised list) lowers:
    /// the entries of every `allow`, `warn` or `expect` list in it, through `cfg_attr` unless its
    /// predicate holds only in test builds. Explicit work list, no recursion.
    fn check_list(
        &self,
        head: &str,
        inner: Vec<TokenTree>,
        at: usize,
        found: &mut Vec<LintOverride>,
    ) {
        let mut queue: Vec<(String, Vec<TokenTree>)> = vec![(head.to_owned(), inner)];
        while let Some((word, inner)) = queue.pop() {
            if word == "cfg_attr" {
                let parts = split_commas(&inner);
                if let Some((predicate, attrs)) = parts.split_first()
                    && !requires_test(predicate)
                {
                    for attr in attrs {
                        if let [head, list] = attr
                            && let Some((head, _)) = ident_name(head)
                            && let Some(list) = group_of(list, Delimiter::Parenthesis)
                        {
                            queue.push((head, list));
                        }
                    }
                }
                continue;
            }
            if !LOWERING_LEVELS.contains(&word.as_str()) {
                continue;
            }
            for entry in split_commas(&inner) {
                let is_reason = entry
                    .first()
                    .is_some_and(|t| ident_name(t).is_some_and(|(n, _)| n == "reason"))
                    && matches!(entry.get(1), Some(TokenTree::Punct(p)) if p.as_char() == '=');
                if is_reason {
                    continue;
                }
                let what = match lint_path(entry) {
                    Some(path) if protected(&path, self.denied, self.groups) => format!(
                        "{word}({path}) lowers a lint the kernel denies, or a group that may hold one"
                    ),
                    Some(_) => continue,
                    None => format!(
                        "{word}({}) names a lint this reader cannot read as a path",
                        joined(entry)
                    ),
                };
                found.push(LintOverride { line: at, what });
            }
        }
    }

    /// Reports, into `found`, what one attribute of the syntax tree lowers.
    fn check_attribute(&self, attr: &Attribute, found: &mut Vec<LintOverride>) {
        let Meta::List(list) = &attr.meta else {
            return;
        };
        let Some(head) = list.path.get_ident() else {
            return;
        };
        let inner: Vec<TokenTree> = list.tokens.clone().into_iter().collect();
        self.check_list(
            &head.to_string(),
            inner,
            line(attr.pound_token.spans[0]),
            found,
        );
    }

    /// Reports, into `found`, what a macro's tokens could carry to an attribute: every attribute
    /// written in them (`#[...]` or `#![...]`), read as an attribute of the tree is, an attribute a
    /// macro assembles (`#[$level(...)]`), which this reader cannot read, and every `allow(...)`,
    /// `warn(...)` or `expect(...)` outside an attribute whose list names a protected lint or a
    /// clippy path this reader cannot read (`clippy::$lint`). Every group is opened, in order.
    fn check_macro_tokens(&self, tokens: TokenStream, found: &mut Vec<LintOverride>) {
        let mut stack: Vec<Vec<TokenTree>> = vec![tokens.into_iter().collect()];
        while let Some(stream) = stack.pop() {
            let mut i = 0;
            while let Some(token) = stream.get(i) {
                let is_pound = matches!(token, TokenTree::Punct(p) if p.as_char() == '#');
                let bang = matches!(stream.get(i.saturating_add(1)), Some(TokenTree::Punct(p)) if p.as_char() == '!');
                let bracket_at = if bang {
                    i.saturating_add(2)
                } else {
                    i.saturating_add(1)
                };
                if is_pound
                    && let Some(body) = stream
                        .get(bracket_at)
                        .and_then(|t| group_of(t, Delimiter::Bracket))
                {
                    let at = line(token.span());
                    match body.as_slice() {
                        [TokenTree::Punct(dollar), ..]
                            if dollar.as_char() == '$' && body.len() > 2 =>
                        {
                            found.push(LintOverride {
                                line: at,
                                what: format!(
                                    "#[{}] is an attribute a macro assembles, which this reader \
                                     cannot read; write lint attributes out",
                                    joined(&body)
                                ),
                            });
                        }
                        [head, list] => {
                            if let (Some((head, _)), Some(list)) =
                                (ident_name(head), group_of(list, Delimiter::Parenthesis))
                            {
                                self.check_list(&head, list, at, found);
                            }
                        }
                        _ => {}
                    }
                    i = bracket_at.saturating_add(1);
                    continue;
                }
                if let Some((word, false)) = ident_name(token)
                    && LOWERING_LEVELS.contains(&word.as_str())
                    && let Some(list) = stream
                        .get(i.saturating_add(1))
                        .and_then(|t| group_of(t, Delimiter::Parenthesis))
                {
                    for entry in split_commas(&list) {
                        let clippy = entry
                            .first()
                            .is_some_and(|t| ident_name(t).is_some_and(|(n, _)| n == "clippy"));
                        let what = match lint_path(entry) {
                            Some(path) if protected(&path, self.denied, self.groups) => format!(
                                "{word}({path}) outside an attribute, as a macro carries one to an \
                                 attribute"
                            ),
                            None if clippy => format!(
                                "{word}({}) names a lint this reader cannot read as a path",
                                joined(entry)
                            ),
                            _ => continue,
                        };
                        found.push(LintOverride {
                            line: line(token.span()),
                            what,
                        });
                    }
                }
                if let TokenTree::Group(group) = token {
                    stack.push(group.stream().into_iter().collect());
                }
                i = i.saturating_add(1);
            }
        }
    }
}

/// The path a `#[path = "..."]` among `attrs` names.
fn path_attribute(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| match &attr.meta {
        Meta::NameValue(nv) if nv.path.is_ident("path") => match &nv.value {
            Expr::Lit(lit) => match &lit.lit {
                Lit::Str(s) => Some(s.value()),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    })
}

/// The attributes of an item.
fn item_attrs(item: &syn::Item) -> &[Attribute] {
    use syn::Item;
    match item {
        Item::Const(i) => &i.attrs,
        Item::Enum(i) => &i.attrs,
        Item::ExternCrate(i) => &i.attrs,
        Item::Fn(i) => &i.attrs,
        Item::ForeignMod(i) => &i.attrs,
        Item::Impl(i) => &i.attrs,
        Item::Macro(i) => &i.attrs,
        Item::Mod(i) => &i.attrs,
        Item::Static(i) => &i.attrs,
        Item::Struct(i) => &i.attrs,
        Item::Trait(i) => &i.attrs,
        Item::TraitAlias(i) => &i.attrs,
        Item::Type(i) => &i.attrs,
        Item::Union(i) => &i.attrs,
        Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

/// The attributes of an expression.
fn expr_attrs(expr: &Expr) -> &[Attribute] {
    match expr {
        Expr::Array(e) => &e.attrs,
        Expr::Assign(e) => &e.attrs,
        Expr::Async(e) => &e.attrs,
        Expr::Await(e) => &e.attrs,
        Expr::Binary(e) => &e.attrs,
        Expr::Block(e) => &e.attrs,
        Expr::Break(e) => &e.attrs,
        Expr::Call(e) => &e.attrs,
        Expr::Cast(e) => &e.attrs,
        Expr::Closure(e) => &e.attrs,
        Expr::Const(e) => &e.attrs,
        Expr::Continue(e) => &e.attrs,
        Expr::Field(e) => &e.attrs,
        Expr::ForLoop(e) => &e.attrs,
        Expr::Group(e) => &e.attrs,
        Expr::If(e) => &e.attrs,
        Expr::Index(e) => &e.attrs,
        Expr::Infer(e) => &e.attrs,
        Expr::Let(e) => &e.attrs,
        Expr::Lit(e) => &e.attrs,
        Expr::Loop(e) => &e.attrs,
        Expr::Macro(e) => &e.attrs,
        Expr::Match(e) => &e.attrs,
        Expr::MethodCall(e) => &e.attrs,
        Expr::Paren(e) => &e.attrs,
        Expr::Path(e) => &e.attrs,
        Expr::Range(e) => &e.attrs,
        Expr::RawAddr(e) => &e.attrs,
        Expr::Reference(e) => &e.attrs,
        Expr::Repeat(e) => &e.attrs,
        Expr::Return(e) => &e.attrs,
        Expr::Struct(e) => &e.attrs,
        Expr::Try(e) => &e.attrs,
        Expr::TryBlock(e) => &e.attrs,
        Expr::Tuple(e) => &e.attrs,
        Expr::Unary(e) => &e.attrs,
        Expr::Unsafe(e) => &e.attrs,
        Expr::While(e) => &e.attrs,
        Expr::Yield(e) => &e.attrs,
        _ => &[],
    }
}

/// The walk over a syntax tree that finds the lint attributes outside test code.
struct LintWalk<'a> {
    rules: Rules<'a>,
    out: LintAttributes,
}

impl<'ast> Visit<'ast> for LintWalk<'_> {
    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        self.rules.check_attribute(attr, &mut self.out.overrides);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.rules
            .check_macro_tokens(mac.tokens.clone(), &mut self.out.overrides);
        visit::visit_macro(self, mac);
    }

    fn visit_item(&mut self, item: &'ast syn::Item) {
        if !is_test(item_attrs(item)) {
            visit::visit_item(self, item);
            return;
        }
        // `#[cfg(test)] mod name;`: a test module in a file of its own.
        if let syn::Item::Mod(module) = item
            && module.content.is_none()
        {
            self.out
                .test_modules
                .push((module.ident.to_string(), path_attribute(&module.attrs)));
        }
    }

    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        let attrs: &[Attribute] = match item {
            syn::ImplItem::Const(i) => &i.attrs,
            syn::ImplItem::Fn(i) => &i.attrs,
            syn::ImplItem::Type(i) => &i.attrs,
            syn::ImplItem::Macro(i) => &i.attrs,
            _ => &[],
        };
        if !is_test(attrs) {
            visit::visit_impl_item(self, item);
        }
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        let attrs: &[Attribute] = match item {
            syn::TraitItem::Const(i) => &i.attrs,
            syn::TraitItem::Fn(i) => &i.attrs,
            syn::TraitItem::Type(i) => &i.attrs,
            syn::TraitItem::Macro(i) => &i.attrs,
            _ => &[],
        };
        if !is_test(attrs) {
            visit::visit_trait_item(self, item);
        }
    }

    fn visit_foreign_item(&mut self, item: &'ast syn::ForeignItem) {
        let attrs: &[Attribute] = match item {
            syn::ForeignItem::Fn(i) => &i.attrs,
            syn::ForeignItem::Static(i) => &i.attrs,
            syn::ForeignItem::Type(i) => &i.attrs,
            syn::ForeignItem::Macro(i) => &i.attrs,
            _ => &[],
        };
        if !is_test(attrs) {
            visit::visit_foreign_item(self, item);
        }
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if !is_test(&local.attrs) {
            visit::visit_local(self, local);
        }
    }

    fn visit_stmt_macro(&mut self, mac: &'ast syn::StmtMacro) {
        if !is_test(&mac.attrs) {
            visit::visit_stmt_macro(self, mac);
        }
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if !is_test(expr_attrs(expr)) {
            visit::visit_expr(self, expr);
        }
    }

    fn visit_field(&mut self, field: &'ast syn::Field) {
        if !is_test(&field.attrs) {
            visit::visit_field(self, field);
        }
    }

    fn visit_variant(&mut self, variant: &'ast syn::Variant) {
        if !is_test(&variant.attrs) {
            visit::visit_variant(self, variant);
        }
    }

    fn visit_fn_arg(&mut self, arg: &'ast syn::FnArg) {
        let attrs = match arg {
            syn::FnArg::Receiver(r) => &r.attrs,
            syn::FnArg::Typed(t) => &t.attrs,
        };
        if !is_test(attrs) {
            visit::visit_fn_arg(self, arg);
        }
    }

    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        if !is_test(&arm.attrs) {
            visit::visit_arm(self, arm);
        }
    }

    fn visit_field_value(&mut self, field: &'ast syn::FieldValue) {
        if !is_test(&field.attrs) {
            visit::visit_field_value(self, field);
        }
    }
}

/// What `source` says about lints, outside test code (see `LintAttributes` and the module header);
/// `None` when it does not parse, so nothing can be shown about it.
pub fn lint_attributes(source: &str, denied: &[&str], groups: &[&str]) -> Option<LintAttributes> {
    let file = syn::parse_file(source).ok()?;
    let mut walk = LintWalk {
        rules: Rules { denied, groups },
        out: LintAttributes::default(),
    };
    if is_test(&file.attrs) {
        walk.out.test_file = true;
        return Some(walk.out);
    }
    walk.visit_file(&file);
    walk.out.overrides.sort();
    Some(walk.out)
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
        // A source that does not parse has neither.
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
        // In a macro's body too: the macro may expand to it.
        assert!(names_extern_crate_std(
            "macro_rules! m { () => { extern crate std; }; }"
        ));
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
            pub struct Staticky; pub fn l(once_cell_count: u8) -> u8 { once_cell_count }\n\
            pub fn m() { 'static_label: loop { break 'static_label; } }\n";
        assert_eq!(found(source), Vec::<(usize, String)>::new());
        // A source that does not lex cannot be shown free of global state.
        assert_eq!(global_state("pub fn f() {}\n/* open"), None);
        assert_eq!(global_state("const S: &str = \"open;\n"), None);
        assert_eq!(global_state("pub fn f() { (]"), None);
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
        // A test statement, expression, field and match arm are test code; the rest is not.
        let nodes = "fn f(x: u8) -> u8 {\n\
            #[cfg(test)] #[allow(clippy::unwrap_used)] let _a = 1;\n\
            #[cfg(test)] { #![allow(clippy::unwrap_used)] }\n\
            match x { #[cfg(test)] 0 => { #[allow(clippy::unwrap_used)] let _b = 2; 0 } _ => x }\n\
            }\n\
            struct S { #[cfg(test)] #[allow(clippy::unwrap_used)] a: u8 }\n\
            impl S { #[cfg(test)] #[allow(clippy::unwrap_used)] fn t() {} \
            #[allow(clippy::unwrap_used)] fn shipped() {} }\n";
        let found = overrides(nodes);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].0, 7, "{found:#?}");
        // A source that does not parse, or whose delimiters do not close, cannot be read.
        assert_eq!(lint_attributes("/* open", &DENIED, &GROUPS), None);
        assert_eq!(lint_attributes("fn f() {", &DENIED, &GROUPS), None);
        assert_eq!(
            lint_attributes("#[allow(clippy::unwrap_used", &DENIED, &GROUPS),
            None
        );
    }
}
