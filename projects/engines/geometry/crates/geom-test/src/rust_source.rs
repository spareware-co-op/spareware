//! Rust source, read for the quality gates: which lines are test code, whether a file has code that
//! runs, where its branches are, every `coverage(off)` mark with the proof it cites, and whether a
//! file defines a cited proof.
//!
//! What it does: [`scan`] parses a Rust file with `syn`, the parser Rust's procedural macros use,
//! and reads every token's line from `proc-macro2`'s span locations. From the syntax tree and the
//! tokens it finds:
//!
//! - **Test and proof code** ([`Scan::test_lines`]): every item, associated item, match arm,
//!   field, variant and `let` statement behind an exact `#[cfg(test)]` or `#[cfg(kani)]` outer
//!   attribute, from its first attribute's line to its last line, and the whole file when it opens
//!   with `#![cfg(test)]` or `#![cfg(kani)]`. Coverage counts the kernel's code, not its tests or
//!   its Kani harnesses (28.7: they sit beside the code under `#[cfg(kani)]`, and no test build
//!   compiles them), so the coverage gate leaves these lines out. Only the exact attributes count;
//!   any other spelling (`cfg(all(test, unix))`) is read as kernel code, which can only make the
//!   gate stricter. An out-of-line module behind one (`#[cfg(test)] mod name;`) is listed in
//!   [`Scan::test_modules`], and its file is left out as a whole.
//! - **Code that runs** ([`Scan::has_functions`]): a function with a body outside test code (a free
//!   function, a method, a trait's default method), or a macro invocation or `macro_rules!`
//!   definition outside test code whose tokens define one (`fn` followed by a name). A file with
//!   one must appear in the coverage data; a file without one (only declarations, types and
//!   constants) has nothing to count.
//! - **Branch sites** ([`Scan::branch_sites`]): the lines outside test code that hold an `if` or
//!   `while` keyword, macro bodies included, the places where branch coverage must have data, so a
//!   report made without branch coverage cannot pass the branch gate by having nothing to count.
//! - **`coverage(off)` marks** ([`Scan::coverage_off`]): every attribute, anywhere in the file and
//!   in any spelling (`#[coverage(off)]`, `#![coverage(off)]`,
//!   `#[cfg_attr(coverage_nightly, coverage(off))]`), whose tokens hold `coverage(off)`, with the
//!   proof cited in the comment lines right above it (`// proof: <path>::<name>`) and the first line
//!   of the item it marks.
//!
//! [`defines_proof`] says whether a file defines a function as a Kani harness (`#[kani::proof]` on
//! the function) or a Verus proof function (`proof fn`, inside `verus!`).
//!
//! Guarantees:
//!
//! - **The language's own grammar.** Comments, strings, raw strings, character literals and nested
//!   block comments are told apart by `proc-macro2`'s lexer, and items by `syn`'s parser, so a
//!   brace or a word inside a string or a comment is never read as code.
//! - **An error, never a partial answer.** A file that does not parse as Rust is a [`ScanError`]
//!   naming the line, because a gate must not pass on a file it could not read.
//! - **A comment is a comment.** A citation is read only from a line that starts with `//` and is
//!   not inside a string literal that spans lines; the walk up from a mark passes only over such
//!   lines and over the lines of other attributes, and stops at anything else.
//!
//! Method: `syn` 3 (MIT OR Apache-2.0, <https://docs.rs/syn/3.0.9/syn/>) parses the file into its
//! syntax tree, walked with `syn::visit`; `proc-macro2` 1 (MIT OR Apache-2.0,
//! <https://docs.rs/proc-macro2/1.0.107/proc_macro2/>) with its `span-locations` feature gives each
//! token's lines outside a procedural macro. The attribute forms are the Rust Reference's
//! (<https://doc.rust-lang.org/reference/attributes.html>, conditional compilation at
//! <https://doc.rust-lang.org/reference/conditional-compilation.html>); `coverage(off)` is the
//! unstable `coverage` attribute (<https://doc.rust-lang.org/unstable-book/language-features/coverage-attribute.html>).
//!
//! Cost: linear in the file's length: one parse, one walk of the tree and one of the tokens. The
//! span locations `proc-macro2` keeps for the file are released when the scan returns.

use core::fmt;
use core::str::FromStr;

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// A `coverage(off)` mark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageOff {
    /// The line of the attribute's `#`.
    pub line: usize,
    /// The proof cited in the comment lines right above the attribute, as written after
    /// `proof:` (`<path>::<name>`); `None` when no comment cites one.
    pub proof: Option<String>,
    /// The first line of the item the attribute marks, trimmed: the line of the first token after
    /// the attribute that is not part of another attribute. For an inner attribute (`#![...]`),
    /// the attribute's own line.
    pub item: String,
}

/// What [`scan`] found in one file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// Inclusive line ranges of test and proof code (`#[cfg(test)]`, `#[cfg(kani)]`), in order.
    pub test_lines: Vec<(usize, usize)>,
    /// The names of out-of-line test and proof modules (`#[cfg(test)] mod name;`,
    /// `#[cfg(kani)] mod name;`), whose files are left out as a whole.
    pub test_modules: Vec<String>,
    /// Whether a function with a body is defined outside test code.
    pub has_functions: bool,
    /// The lines outside test code holding an `if` or `while` keyword, in order, once each.
    pub branch_sites: Vec<usize>,
    /// Every `coverage(off)` mark, in order.
    pub coverage_off: Vec<CoverageOff>,
}

impl Scan {
    /// Whether `line` is inside test or proof code.
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

/// The tool a proof is written for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProofTool {
    /// A Kani harness: `#[kani::proof]` on a function.
    Kani,
    /// A Verus proof function: `proof fn`.
    Verus,
}

/// The lines a span covers, from 1.
fn lines(span: Span) -> (usize, usize) {
    (span.start().line, span.end().line)
}

/// The text a file is parsed from: without a byte-order mark, and with a shebang line emptied but
/// kept, as `syn::parse_file` reads it, so line numbers stay the file's own.
fn content(src: &str) -> String {
    let text = src.strip_prefix('\u{feff}').unwrap_or(src);
    if let Some(rest) = text.strip_prefix("#!")
        && !rest.trim_start().starts_with('[')
    {
        return text
            .find('\n')
            .and_then(|i| text.get(i..))
            .unwrap_or("")
            .to_owned();
    }
    text.to_owned()
}

/// Parses `src` into its tokens and its syntax tree, with the spans' line numbers.
fn parse(src: &str) -> Result<(TokenStream, syn::File), ScanError> {
    let text = content(src);
    let tokens = TokenStream::from_str(&text).map_err(|e| ScanError {
        line: e.span().start().line,
        why: format!("not Rust tokens: {e}"),
    })?;
    let file = syn::parse2::<syn::File>(tokens.clone()).map_err(|e| ScanError {
        line: e.span().start().line,
        why: format!("not a Rust file: {e}"),
    })?;
    Ok((tokens, file))
}

/// Runs `f` on a file's tokens and tree, then releases the span locations `proc-macro2` kept for
/// it, which would otherwise grow with every file a thread parses.
fn with_parsed<T>(
    src: &str,
    f: impl FnOnce(&TokenStream, &syn::File) -> T,
) -> Result<T, ScanError> {
    let result = parse(src).map(|(tokens, file)| f(&tokens, &file));
    proc_macro2::extra::invalidate_current_thread_spans();
    result
}

/// Whether an attribute is exactly `cfg(test)` or `cfg(kani)`, outer or inner: code no measured
/// test build compiles as kernel code.
fn is_cfg_test(attr: &syn::Attribute) -> bool {
    let syn::Meta::List(list) = &attr.meta else {
        return false;
    };
    let inner: Vec<TokenTree> = list.tokens.clone().into_iter().collect();
    list.path.is_ident("cfg")
        && matches!(list.delimiter, syn::MacroDelimiter::Paren(_))
        && matches!(inner.as_slice(), [TokenTree::Ident(i)] if i == "test" || i == "kani")
}

fn outer_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs
        .iter()
        .any(|a| matches!(a.style, syn::AttrStyle::Outer) && is_cfg_test(a))
}

fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(i) => &i.attrs,
        syn::Item::Enum(i) => &i.attrs,
        syn::Item::ExternCrate(i) => &i.attrs,
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::ForeignMod(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Macro(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Trait(i) => &i.attrs,
        syn::Item::TraitAlias(i) => &i.attrs,
        syn::Item::Type(i) => &i.attrs,
        syn::Item::Union(i) => &i.attrs,
        syn::Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

fn impl_item_attrs(item: &syn::ImplItem) -> &[syn::Attribute] {
    match item {
        syn::ImplItem::Const(i) => &i.attrs,
        syn::ImplItem::Fn(i) => &i.attrs,
        syn::ImplItem::Type(i) => &i.attrs,
        syn::ImplItem::Macro(i) => &i.attrs,
        _ => &[],
    }
}

fn trait_item_attrs(item: &syn::TraitItem) -> &[syn::Attribute] {
    match item {
        syn::TraitItem::Const(i) => &i.attrs,
        syn::TraitItem::Fn(i) => &i.attrs,
        syn::TraitItem::Type(i) => &i.attrs,
        syn::TraitItem::Macro(i) => &i.attrs,
        _ => &[],
    }
}

fn foreign_item_attrs(item: &syn::ForeignItem) -> &[syn::Attribute] {
    match item {
        syn::ForeignItem::Fn(i) => &i.attrs,
        syn::ForeignItem::Static(i) => &i.attrs,
        syn::ForeignItem::Type(i) => &i.attrs,
        syn::ForeignItem::Macro(i) => &i.attrs,
        _ => &[],
    }
}

/// Whether a token stream defines a function: `fn` followed by a name, at any depth.
fn defines_fn(tokens: &TokenStream) -> bool {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    trees.windows(2).any(|w| {
        matches!(w, [TokenTree::Ident(f), TokenTree::Ident(_)] if f == "fn")
    }) || trees.iter().any(|t| match t {
        TokenTree::Group(g) => defines_fn(&g.stream()),
        _ => false,
    })
}

/// The walk over the syntax tree: test code, out-of-line test modules and functions.
#[derive(Default)]
struct Items {
    test_lines: Vec<(usize, usize)>,
    test_modules: Vec<String>,
    has_functions: bool,
}

impl Items {
    /// Records a node behind `#[cfg(test)]` as test code; returns whether it was.
    fn test(&mut self, attrs: &[syn::Attribute], span: Span) -> bool {
        let is_test = outer_cfg_test(attrs);
        if is_test {
            self.test_lines.push(lines(span));
        }
        is_test
    }
}

impl<'ast> Visit<'ast> for Items {
    fn visit_item(&mut self, i: &'ast syn::Item) {
        if self.test(item_attrs(i), i.span()) {
            if let syn::Item::Mod(m) = i
                && m.content.is_none()
            {
                self.test_modules.push(m.ident.to_string());
            }
            return;
        }
        visit::visit_item(self, i);
    }

    fn visit_impl_item(&mut self, i: &'ast syn::ImplItem) {
        if !self.test(impl_item_attrs(i), i.span()) {
            visit::visit_impl_item(self, i);
        }
    }

    fn visit_trait_item(&mut self, i: &'ast syn::TraitItem) {
        if !self.test(trait_item_attrs(i), i.span()) {
            visit::visit_trait_item(self, i);
        }
    }

    fn visit_foreign_item(&mut self, i: &'ast syn::ForeignItem) {
        if !self.test(foreign_item_attrs(i), i.span()) {
            visit::visit_foreign_item(self, i);
        }
    }

    fn visit_arm(&mut self, i: &'ast syn::Arm) {
        if !self.test(&i.attrs, i.span()) {
            visit::visit_arm(self, i);
        }
    }

    fn visit_local(&mut self, i: &'ast syn::Local) {
        if !self.test(&i.attrs, i.span()) {
            visit::visit_local(self, i);
        }
    }

    fn visit_field(&mut self, i: &'ast syn::Field) {
        if !self.test(&i.attrs, i.span()) {
            visit::visit_field(self, i);
        }
    }

    fn visit_variant(&mut self, i: &'ast syn::Variant) {
        if !self.test(&i.attrs, i.span()) {
            visit::visit_variant(self, i);
        }
    }

    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        self.has_functions = true;
        visit::visit_item_fn(self, i);
    }

    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        self.has_functions = true;
        visit::visit_impl_item_fn(self, i);
    }

    fn visit_trait_item_fn(&mut self, i: &'ast syn::TraitItemFn) {
        self.has_functions |= i.default.is_some();
        visit::visit_trait_item_fn(self, i);
    }

    fn visit_macro(&mut self, i: &'ast syn::Macro) {
        self.has_functions |= defines_fn(&i.tokens);
        visit::visit_macro(self, i);
    }
}

/// One attribute found in the token stream.
struct Attr {
    /// The line of its `#`.
    line: usize,
    /// Its last line (the closing `]`).
    end: usize,
    /// `#![...]`.
    inner: bool,
    /// Whether it holds `coverage(off)`.
    coverage_off: bool,
    /// The line of the first token after it that is not part of another attribute, if any.
    item_line: Option<usize>,
}

/// Whether a token stream holds `coverage(off)` at any depth.
fn holds_coverage_off(tokens: &TokenStream) -> bool {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    trees.windows(2).any(|w| match w {
        [TokenTree::Ident(c), TokenTree::Group(g)] if c == "coverage" => {
            let inside: Vec<TokenTree> = g.stream().into_iter().collect();
            g.delimiter() == Delimiter::Parenthesis
                && matches!(inside.as_slice(), [TokenTree::Ident(o)] if o == "off")
        }
        _ => false,
    }) || trees.iter().any(|t| match t {
        TokenTree::Group(g) => holds_coverage_off(&g.stream()),
        _ => false,
    })
}

/// If an attribute starts at `trees[i]` (`#`, an optional `!`, a bracketed group): the index of
/// its group and whether it is inner.
fn attribute_at(trees: &[TokenTree], i: usize) -> Option<(usize, bool)> {
    let TokenTree::Punct(p) = trees.get(i)? else {
        return None;
    };
    if p.as_char() != '#' {
        return None;
    }
    let mut at = i.checked_add(1)?;
    let inner = matches!(trees.get(at), Some(TokenTree::Punct(b)) if b.as_char() == '!');
    if inner {
        at = at.checked_add(1)?;
    }
    match trees.get(at)? {
        TokenTree::Group(g) if g.delimiter() == Delimiter::Bracket => Some((at, inner)),
        _ => None,
    }
}

/// The walk over the tokens: attributes, branch sites and the lines inside literals that span
/// lines.
#[derive(Default)]
struct Tokens {
    attrs: Vec<Attr>,
    branch_sites: Vec<(usize, &'static str)>,
    literal_inner_lines: Vec<(usize, usize)>,
    /// `proof fn <name>` triples, by name.
    proof_fns: Vec<String>,
}

impl Tokens {
    fn walk(&mut self, stream: &TokenStream) {
        let trees: Vec<TokenTree> = stream.clone().into_iter().collect();
        let mut i = 0usize;
        while let Some(tree) = trees.get(i) {
            if let Some((group, inner)) = attribute_at(&trees, i) {
                let Some(TokenTree::Group(g)) = trees.get(group) else {
                    break;
                };
                let mut next = group.saturating_add(1);
                while let Some((g2, _)) = attribute_at(&trees, next) {
                    next = g2.saturating_add(1);
                }
                self.attrs.push(Attr {
                    line: tree.span().start().line,
                    end: g.span_close().end().line,
                    inner,
                    coverage_off: holds_coverage_off(&g.stream()),
                    item_line: trees.get(next).map(|t| t.span().start().line),
                });
                self.walk(&g.stream());
                i = group.saturating_add(1);
                continue;
            }
            match tree {
                TokenTree::Group(g) => self.walk(&g.stream()),
                TokenTree::Ident(word) => {
                    if word == "if" {
                        self.branch_sites.push((word.span().start().line, "if"));
                    } else if word == "while" {
                        self.branch_sites.push((word.span().start().line, "while"));
                    } else if word == "proof"
                        && let (Some(TokenTree::Ident(f)), Some(TokenTree::Ident(name))) =
                            (trees.get(i.saturating_add(1)), trees.get(i.saturating_add(2)))
                        && f == "fn"
                    {
                        self.proof_fns.push(name.to_string());
                    }
                }
                TokenTree::Literal(l) => {
                    let (first, last) = lines(l.span());
                    if last > first {
                        self.literal_inner_lines.push((first.saturating_add(1), last));
                    }
                }
                TokenTree::Punct(_) => {}
            }
            i = i.saturating_add(1);
        }
    }

    fn in_literal(&self, line: usize) -> bool {
        self.literal_inner_lines
            .iter()
            .any(|&(a, b)| a <= line && line <= b)
    }

    fn in_attribute(&self, line: usize) -> bool {
        self.attrs.iter().any(|a| a.line <= line && line <= a.end)
    }
}

/// The trimmed text of line `line` (from 1) of `text`.
fn line_text(text: &[&str], line: usize) -> String {
    line.checked_sub(1)
        .and_then(|i| text.get(i))
        .map_or_else(String::new, |l| l.trim().to_owned())
}

/// The proof a mark at line `mark` cites: `proof:` in the comment lines directly above it, walking
/// up over comment lines and the lines of other attributes, and stopping at anything else.
fn cited_proof(text: &[&str], tokens: &Tokens, mark: usize) -> Option<String> {
    let mut line = mark.checked_sub(1)?;
    while line >= 1 {
        let here = line_text(text, line);
        if here.starts_with("//") && !tokens.in_literal(line) {
            if let Some((_, after)) = here.split_once("proof:") {
                return after.split_whitespace().next().map(str::to_owned);
            }
        } else if here.is_empty() || !tokens.in_attribute(line) {
            return None;
        }
        line = line.checked_sub(1)?;
    }
    None
}

/// Scans one Rust file (see the module documentation).
///
/// # Errors
///
/// [`ScanError`] if the file is not Rust: its tokens do not lex, or they do not parse as a file.
pub fn scan(src: &str) -> Result<Scan, ScanError> {
    let text: Vec<&str> = src.lines().collect();
    with_parsed(src, |stream, file| {
        let mut items = Items::default();
        if file.attrs.iter().any(is_cfg_test) {
            items.test_lines.push((1, text.len().max(1)));
        } else {
            items.visit_file(file);
        }
        let mut tokens = Tokens::default();
        tokens.walk(stream);
        let mut out = Scan {
            test_lines: items.test_lines,
            test_modules: items.test_modules,
            has_functions: items.has_functions,
            ..Scan::default()
        };
        for &(line, _) in &tokens.branch_sites {
            if !out.is_test_line(line) && out.branch_sites.last() != Some(&line) {
                out.branch_sites.push(line);
            }
        }
        out.branch_sites.sort_unstable();
        out.branch_sites.dedup();
        for a in tokens.attrs.iter().filter(|a| a.coverage_off) {
            out.coverage_off.push(CoverageOff {
                line: a.line,
                proof: cited_proof(&text, &tokens, a.line),
                item: if a.inner {
                    line_text(&text, a.line)
                } else {
                    a.item_line
                        .map_or_else(String::new, |l| line_text(&text, l))
                },
            });
        }
        out
    })
}

/// The walk that finds Kani harnesses by name.
struct Harnesses<'n> {
    name: &'n str,
    found: bool,
}

fn is_kani_proof(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        let segments: Vec<String> = a
            .path()
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();
        segments == ["kani", "proof"] && matches!(a.meta, syn::Meta::Path(_))
    })
}

impl<'ast> Visit<'ast> for Harnesses<'_> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        self.found |= i.sig.ident == self.name && is_kani_proof(&i.attrs);
        visit::visit_item_fn(self, i);
    }

    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        self.found |= i.sig.ident == self.name && is_kani_proof(&i.attrs);
        visit::visit_impl_item_fn(self, i);
    }
}

/// Whether the Rust file `src` defines `name` as a proof for `tool`: a function carrying
/// `#[kani::proof]` (Kani), or `proof fn name` anywhere in its tokens, as Verus writes proof
/// functions inside its `verus!` macro (Verus).
///
/// # Errors
///
/// [`ScanError`] if the file is not Rust.
pub fn defines_proof(src: &str, name: &str, tool: ProofTool) -> Result<bool, ScanError> {
    with_parsed(src, |stream, file| match tool {
        ProofTool::Kani => {
            let mut h = Harnesses { name, found: false };
            h.visit_file(file);
            h.found
        }
        ProofTool::Verus => {
            let mut tokens = Tokens::default();
            tokens.walk(stream);
            tokens.proof_fns.iter().any(|n| n == name)
        }
    })
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
    fn strings_comments_and_chars_hide_code() {
        let src = "pub fn a() -> usize {\n    let s = \"{ if } // not a comment\";\n    \
                   let c = '{';\n    let r = r#\"} while\"#;\n    /* { if /* } */ */\n    \
                   let l: &'static str = s;\n    l.len() + usize::from(c == '{') + r.len()\n}\n";
        let s = scan(src).unwrap();
        assert!(s.has_functions);
        assert_eq!(s.branch_sites, Vec::<usize>::new());
        assert!(s.test_lines.is_empty());
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
    fn kani_harnesses_are_left_out_like_tests() {
        let src = "pub fn f() {}\n#[cfg(kani)]\nmod proofs {\n    #[kani::proof]\n    fn p() { if true {} }\n}\n\
                   #[cfg(kani)]\nmod more;\n";
        let s = scan(src).unwrap();
        assert_eq!(s.test_lines, vec![(2, 6), (7, 8)]);
        assert_eq!(s.test_modules, vec!["more".to_owned()]);
        assert!(s.branch_sites.is_empty());
    }

    #[test]
    fn only_the_exact_attribute_is_test_code() {
        let src = "#[cfg(all(test, unix))]\nmod t {\n    fn f() {}\n}\n#[cfg(not(test))]\nfn g() {}\n";
        let s = scan(src).unwrap();
        assert_eq!(s.test_lines, Vec::<(usize, usize)>::new());
        assert!(s.has_functions);
    }

    #[test]
    fn test_methods_arms_and_whole_test_files_are_test_code() {
        let src = "pub struct S;\nimpl S {\n    #[cfg(test)]\n    fn only_in_tests() {}\n}\n\
                   pub fn f(x: u8) -> u8 {\n    match x {\n        #[cfg(test)]\n        0 => 1,\n        \
                   _ => 2,\n    }\n}\n";
        let s = scan(src).unwrap();
        assert_eq!(s.test_lines, vec![(3, 4), (8, 9)]);
        let whole = scan("#![cfg(test)]\nfn f() { if true {} }\n").unwrap();
        assert_eq!(whole.test_lines, vec![(1, 2)]);
        assert!(!whole.has_functions);
        assert!(whole.branch_sites.is_empty());
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
    fn a_citation_inside_a_string_is_not_a_comment() {
        let src = "pub const S: &str = \"\n// proof: a.rs::p\n\";\n#[coverage(off)]\nfn f() {}\n";
        let s = scan(src).unwrap();
        assert_eq!(s.coverage_off.first().unwrap().proof, None);
        let blank = "// proof: a.rs::p\n\n#[coverage(off)]\nfn f() {}\n";
        assert_eq!(scan(blank).unwrap().coverage_off[0].proof, None);
    }

    #[test]
    fn unreadable_files_are_errors() {
        assert!(scan("fn a() { let s = \"open; }").is_err());
        assert!(scan("/* never closed").is_err());
        assert!(scan("#[cfg(test)]\nmod t {\n").is_err());
        assert!(scan("fn (").is_err());
    }

    #[test]
    fn function_pointer_types_are_not_functions() {
        let s = scan("pub struct S { f: fn(u32) -> u32 }\npub const X: u32 = 1;\n").unwrap();
        assert!(!s.has_functions);
        let declared = scan("pub trait T { fn f(&self); }\n").unwrap();
        assert!(!declared.has_functions);
        let by_macro = scan("macro_rules! m { () => { fn made() {} }; }\nm!();\n").unwrap();
        assert!(by_macro.has_functions);
    }

    #[test]
    fn proofs_are_found_by_tool_and_name() {
        let kani = "#[cfg(kani)]\nmod proofs {\n    #[kani::proof]\n    fn rungs() {}\n    fn helper() {}\n}\n";
        assert!(defines_proof(kani, "rungs", ProofTool::Kani).unwrap());
        assert!(!defines_proof(kani, "helper", ProofTool::Kani).unwrap());
        assert!(!defines_proof(kani, "rungs", ProofTool::Verus).unwrap());
        let verus = "verus! {\n    pub proof fn budget(x: int) ensures x == x {}\n}\n";
        assert!(defines_proof(verus, "budget", ProofTool::Verus).unwrap());
        assert!(!defines_proof(verus, "budget", ProofTool::Kani).unwrap());
        let in_string = "pub const S: &str = \"#[kani::proof] fn rungs() {}\";\n";
        assert!(!defines_proof(in_string, "rungs", ProofTool::Kani).unwrap());
    }
}
