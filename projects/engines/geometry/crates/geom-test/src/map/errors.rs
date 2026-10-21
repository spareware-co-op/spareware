//! Finding a kernel crate's error kinds: the public error enums in its source, with every variant
//! and its doc comment.
//!
//! What it does: [`find`] reads one source file's tokens ([`crate::map::rust`]) and returns every
//! public enum that is an error type, declared at the top level of the file, with its doc comment,
//! whether it is `#[non_exhaustive]`, and each variant's name, shape and doc comment. An enum is
//! an error type when its name contains `Error` (`NumError`, and chapter 22's `ErrorKind`,
//! `OpErrorDetail` and `IntersectError` when they arrive), or when its crate implements a trait
//! named `Error` for it or names it as the error of a `Result<_, E>` anywhere
//! ([`crate::map::items`]), so a family's own detail enum is found whatever it is called. Items
//! under `#[cfg(test)]` are skipped whole.
//!
//! Why by name and by use: chapter 22 names the kernel's shared error types `…Error`, `ErrorKind`
//! and `…ErrorDetail`, but leaves the name of each family's detail enum (a sweep's
//! `MitreMismatch { .. }` sits in one) to its piece. The name rule needs no annotation that could
//! be forgotten; the use rule finds every enum the crate's code treats as an error; and the checks
//! below make what they find complete: an error enum the scanner cannot place is reported, never
//! skipped.
//!
//! Guarantees:
//!
//! - **Found or reported.** A public error enum inside a function, an inline module or any other
//!   block that is not a `#[cfg(test)]` item is reported ([`Rule::ErrorEnum`]): error enums are
//!   declared at the top level of a module file, where the map finds them.
//! - **Every kind explained.** An error enum or a variant without a doc comment is reported
//!   ([`Rule::ErrorDoc`]), because the catalogue of 26.3 explains every error the kernel can
//!   return.
//! - **Only real code.** The scan runs on tokens, so an enum inside a comment, a string or a
//!   doc example is never found; `pub(crate)` and private enums are not part of the crate's
//!   interface and are not listed.
//!
//! Method: a single pass over the token stream at nesting depth zero, reading attributes
//! (`#[...]`), outer doc comments and `pub enum` items, with bracket depth counted over `()`, `[]`
//! and `{}`, the way Clippy's development tool finds `declare_clippy_lint!` items with a token
//! cursor rather than a full parser
//! (<https://github.com/rust-lang/rust-clippy/blob/3583eb8a9ae1161bd7d80907acfa09b3612a93f9/clippy_dev/src/parse/cursor.rs>).
//! Item syntax follows The Rust Reference on enumerations
//! (<https://doc.rust-lang.org/reference/items/enumerations.html>) and attributes
//! (<https://doc.rust-lang.org/reference/attributes.html>).
//!
//! Cost: linear in the number of tokens, times a search of the crate's error type names.
//!
//! Consulted: Clippy's `clippy_dev` token cursor, MIT OR Apache-2.0,
//!   <https://github.com/rust-lang/rust-clippy/blob/3583eb8a9ae1161bd7d80907acfa09b3612a93f9/clippy_dev/src/parse/cursor.rs>,
//!   for the approach of finding declarations with a token cursor. No code copied.

use std::collections::BTreeSet;

use crate::map::Rule;
use crate::map::rust::{Kind, Token};

/// The shape of an enum variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// No data: `Degenerate`.
    Unit,
    /// Unnamed fields: `Malformed(Record)`.
    Tuple,
    /// Named fields: `OutOfRange { value: i128 }`.
    Struct,
}

impl Shape {
    /// The shape's name in the map.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Shape::Unit => "unit",
            Shape::Tuple => "tuple",
            Shape::Struct => "struct",
        }
    }
}

/// One error kind: a variant of an error enum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    /// The variant's name.
    pub name: String,
    /// The first paragraph of its doc comment, if it has one.
    pub doc: Option<String>,
    /// Its shape.
    pub shape: Shape,
}

/// One error enum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorEnum {
    /// The enum's name.
    pub name: String,
    /// The first paragraph of its doc comment, if it has one.
    pub doc: Option<String>,
    /// Whether it is `#[non_exhaustive]` (chapter 22: the base kinds are not, the details are).
    pub non_exhaustive: bool,
    /// Its variants, in source order.
    pub variants: Vec<Variant>,
}

/// What [`find`] found in one file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// The error enums, in source order.
    pub enums: Vec<ErrorEnum>,
    /// What is wrong, each with its rule.
    pub problems: Vec<(Rule, String)>,
}

/// Whether `name` names an error enum.
#[must_use]
pub fn is_error_name(name: &str) -> bool {
    name.contains("Error")
}

/// The first paragraph of doc lines: the lines up to the first blank one, joined with spaces.
fn first_paragraph(docs: &[String]) -> Option<String> {
    let lines: Vec<&str> = docs
        .iter()
        .flat_map(|d| d.split('\n'))
        .map(str::trim)
        .skip_while(|l| l.is_empty())
        .take_while(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join(" "))
    }
}

/// The index just past the bracket that closes the one at `open`, counting `()`, `[]` and `{}`
/// together; `None` when it never closes.
pub(crate) fn matching(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth: usize = 0;
    for (at, t) in tokens.iter().enumerate().skip(open) {
        if t.kind != Kind::Punct {
            continue;
        }
        if t.is_punct('(') || t.is_punct('[') || t.is_punct('{') {
            depth = depth.saturating_add(1);
        } else if t.is_punct(')') || t.is_punct(']') || t.is_punct('}') {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return at.checked_add(1);
            }
        }
    }
    None
}

/// The index just past the item that starts at `start`: past its first `;` at depth zero, or past
/// the block that its first `{` at depth zero opens.
pub(crate) fn item_end(tokens: &[Token], start: usize) -> Option<usize> {
    let mut at = start;
    while let Some(t) = tokens.get(at) {
        if t.is_punct(';') {
            return at.checked_add(1);
        }
        if t.is_punct('{') {
            return matching(tokens, at);
        }
        if t.is_punct('(') || t.is_punct('[') {
            at = matching(tokens, at)?;
            continue;
        }
        at = at.checked_add(1)?;
    }
    None
}

/// Attributes and docs seen since the last item.
#[derive(Default)]
struct Pending {
    docs: Vec<String>,
    cfg_test: bool,
    non_exhaustive: bool,
}

/// The error enums of one file's tokens. `error_types` are the names the crate uses as errors
/// ([`crate::map::items::Items::error_impls`] and [`crate::map::items::Items::result_errors`] of
/// every file of the crate).
#[must_use]
pub fn find(tokens: &[Token], error_types: &BTreeSet<String>) -> Found {
    let mut found = Found::default();
    let mut pending = Pending::default();
    let mut depth: usize = 0;
    let mut at: usize = 0;
    while let Some(t) = tokens.get(at) {
        let next = at.saturating_add(1);
        match t.kind {
            Kind::Doc => {
                pending.docs.push(t.text.clone());
                at = next;
            }
            Kind::Punct if t.is_punct('#') => {
                at = attribute(tokens, at, &mut pending).unwrap_or(tokens.len());
            }
            Kind::Punct if t.is_punct('{') || t.is_punct('(') || t.is_punct('[') => {
                depth = depth.saturating_add(1);
                pending = Pending::default();
                at = next;
            }
            Kind::Punct if t.is_punct('}') || t.is_punct(')') || t.is_punct(']') => {
                depth = depth.saturating_sub(1);
                pending = Pending::default();
                at = next;
            }
            Kind::Punct if t.is_punct(';') => {
                pending = Pending::default();
                at = next;
            }
            Kind::Ident if pending.cfg_test => {
                // A test-only item: skipped whole, whatever it holds.
                at = item_end(tokens, at).unwrap_or(tokens.len());
                pending = Pending::default();
            }
            Kind::Ident if t.is_ident("pub") && public_enum(tokens, at) => {
                at = enum_item(tokens, at, depth, &pending, error_types, &mut found);
                pending = Pending::default();
            }
            _ => at = next,
        }
    }
    found
}

/// Whether the tokens at `at` are `pub enum <Name>` (plain `pub`, not `pub(crate)`).
fn public_enum(tokens: &[Token], at: usize) -> bool {
    tokens
        .get(at.saturating_add(1))
        .is_some_and(|t| t.is_ident("enum"))
        && tokens
            .get(at.saturating_add(2))
            .is_some_and(|t| t.kind == Kind::Ident)
}

/// Reads the attribute at `at` (`#[...]` or `#![...]`) into `pending`; the index past it.
fn attribute(tokens: &[Token], at: usize, pending: &mut Pending) -> Option<usize> {
    let mut open = at.checked_add(1)?;
    let inner = tokens.get(open).is_some_and(|t| t.is_punct('!'));
    if inner {
        open = open.checked_add(1)?;
    }
    if !tokens.get(open).is_some_and(|t| t.is_punct('[')) {
        return Some(at.saturating_add(1));
    }
    let end = matching(tokens, open)?;
    let body: Vec<&str> = tokens
        .get(open.checked_add(1)?..end.checked_sub(1)?)?
        .iter()
        .map(|t| t.text.as_str())
        .collect();
    if !inner {
        match body.as_slice() {
            ["cfg", "(", "test", ")"] => pending.cfg_test = true,
            ["non_exhaustive"] => pending.non_exhaustive = true,
            _ => {}
        }
    }
    Some(end)
}

/// Reads the `pub enum` at `at`; the index past it. Error enums at depth zero are recorded, others
/// reported.
fn enum_item(
    tokens: &[Token],
    at: usize,
    depth: usize,
    pending: &Pending,
    error_types: &BTreeSet<String>,
    found: &mut Found,
) -> usize {
    let name = tokens
        .get(at.saturating_add(2))
        .map(|t| t.text.clone())
        .unwrap_or_default();
    let end = item_end(tokens, at).unwrap_or(tokens.len());
    if !is_error_name(&name) && !error_types.contains(&name) {
        return end;
    }
    if depth > 0 {
        found.problems.push((
            Rule::ErrorEnum,
            format!(
                "`pub enum {name}` is declared inside a block; declare error enums at the top \
                 level of a module file, where the map finds them"
            ),
        ));
        return end;
    }
    let body_open = (at..end).find(|&k| tokens.get(k).is_some_and(|t| t.is_punct('{')));
    let body = body_open
        .and_then(|open| tokens.get(open.saturating_add(1)..end.saturating_sub(1)))
        .unwrap_or_default();
    let doc = first_paragraph(&pending.docs);
    if doc.is_none() {
        found.problems.push((
            Rule::ErrorDoc,
            format!("the error enum `{name}` has no doc comment"),
        ));
    }
    let variants = variants(body);
    for v in &variants {
        if v.doc.is_none() {
            found.problems.push((
                Rule::ErrorDoc,
                format!("the error kind `{name}::{}` has no doc comment", v.name),
            ));
        }
    }
    found.enums.push(ErrorEnum {
        name,
        doc,
        non_exhaustive: pending.non_exhaustive,
        variants,
    });
    end
}

/// The variants of an enum body (the tokens between its braces).
fn variants(body: &[Token]) -> Vec<Variant> {
    let mut out = Vec::new();
    let mut docs: Vec<String> = Vec::new();
    let mut at: usize = 0;
    while let Some(t) = body.get(at) {
        match t.kind {
            Kind::Doc => {
                docs.push(t.text.clone());
                at = at.saturating_add(1);
            }
            Kind::Punct if t.is_punct('#') => {
                let mut ignored = Pending::default();
                at = attribute(body, at, &mut ignored).unwrap_or(body.len());
            }
            Kind::Ident => {
                let after = at.saturating_add(1);
                let shape = match body.get(after) {
                    Some(n) if n.is_punct('(') => Shape::Tuple,
                    Some(n) if n.is_punct('{') => Shape::Struct,
                    _ => Shape::Unit,
                };
                out.push(Variant {
                    name: t.text.clone(),
                    doc: first_paragraph(&docs),
                    shape,
                });
                docs.clear();
                at = variant_end(body, after);
            }
            _ => at = at.saturating_add(1),
        }
    }
    out
}

/// The index just past the comma that ends the variant whose fields start at `at`, or the body's
/// end.
fn variant_end(body: &[Token], mut at: usize) -> usize {
    while let Some(t) = body.get(at) {
        if t.is_punct(',') {
            return at.saturating_add(1);
        }
        if t.is_punct('(') || t.is_punct('[') || t.is_punct('{') {
            at = matching(body, at).unwrap_or(body.len());
        } else {
            at = at.saturating_add(1);
        }
    }
    body.len()
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
    use crate::map::rust::tokens;

    fn scan(source: &str) -> Found {
        find(&tokens(source).unwrap(), &BTreeSet::new())
    }

    const NUM_ERROR: &str = r#"
        use core::fmt;

        /// Why a number could not be made.
        ///
        /// More detail.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum NumError {
            /// A coordinate is outside the range.
            ///
            /// Detail that is not the summary.
            OutOfRange {
                /// The value.
                value: i128,
            },
            /// Degenerate inputs.
            Degenerate,
            /// A record that cannot be read.
            #[allow(dead_code)]
            Malformed(Vec<(u8, u8)>, [u8; 2]),
        }

        impl fmt::Display for NumError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("{") }
        }

        /// Not an error.
        pub enum Sign { Negative, Zero, Positive }

        pub(crate) enum PrivateError { A }

        #[cfg(test)]
        mod tests {
            pub enum TestOnlyError { A }
        }
    "#;

    #[test]
    fn finds_the_error_enum_with_its_kinds() {
        let found = scan(NUM_ERROR);
        assert_eq!(found.problems, []);
        assert_eq!(found.enums.len(), 1);
        let e = &found.enums[0];
        assert_eq!(e.name, "NumError");
        assert_eq!(e.doc.as_deref(), Some("Why a number could not be made."));
        assert!(e.non_exhaustive);
        let kinds: Vec<(&str, Shape, Option<&str>)> = e
            .variants
            .iter()
            .map(|v| (v.name.as_str(), v.shape, v.doc.as_deref()))
            .collect();
        assert_eq!(
            kinds,
            [
                (
                    "OutOfRange",
                    Shape::Struct,
                    Some("A coordinate is outside the range.")
                ),
                ("Degenerate", Shape::Unit, Some("Degenerate inputs.")),
                (
                    "Malformed",
                    Shape::Tuple,
                    Some("A record that cannot be read.")
                ),
            ]
        );
    }

    #[test]
    fn nested_and_undocumented_error_enums_are_reported() {
        let found =
            scan("fn f() { pub enum LocalError { A } }\npub enum BareError { /// Doc.\n A, B }");
        let rules: Vec<Rule> = found.problems.iter().map(|(r, _)| *r).collect();
        assert_eq!(rules, [Rule::ErrorEnum, Rule::ErrorDoc, Rule::ErrorDoc]);
        assert!(found.problems[1].1.contains("`BareError`"));
        assert!(found.problems[2].1.contains("`BareError::B`"));
        assert_eq!(found.enums.len(), 1);
        assert!(!found.enums[0].non_exhaustive);
    }

    #[test]
    fn test_items_are_skipped_whole() {
        let found = scan(
            "#[cfg(test)]\nfn helper() { pub enum InnerError { A } }\n#[cfg(test)]\nuse x::y;\n\
             /// Kept.\npub enum KeptError {\n/// Doc.\nA }",
        );
        assert_eq!(found.problems, []);
        assert_eq!(found.enums.len(), 1);
        assert_eq!(found.enums[0].name, "KeptError");
    }

    #[test]
    fn an_enum_used_as_an_error_is_an_error_enum() {
        let source = "/// Why a sweep failed.
pub enum SweepFailure { MitreMismatch }
                      fn f() { pub enum LocalFailure { A } }";
        let names: BTreeSet<String> = ["SweepFailure".to_owned(), "LocalFailure".to_owned()]
            .into_iter()
            .collect();
        let found = find(&tokens(source).unwrap(), &names);
        assert_eq!(found.enums.len(), 1);
        assert_eq!(found.enums[0].name, "SweepFailure");
        let rules: Vec<Rule> = found.problems.iter().map(|(r, _)| *r).collect();
        assert_eq!(rules, [Rule::ErrorDoc, Rule::ErrorEnum]);
        assert_eq!(scan(source).enums, []);
    }

    #[test]
    fn generics_and_where_clauses() {
        let found =
            scan("/// G.\npub enum GenError<T> where T: Fn() -> u8 {\n/// One.\nOne(T),\n}");
        assert_eq!(found.problems, []);
        assert_eq!(found.enums[0].variants.len(), 1);
        assert_eq!(found.enums[0].variants[0].shape, Shape::Tuple);
    }
}
