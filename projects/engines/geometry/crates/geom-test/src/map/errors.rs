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
//! - **Every detail with its base kind.** Chapter 22 maps every detail to one base kind, and the
//!   catalogue lists both (26.3). For an error enum whose `impl` block in the same file has a
//!   function `kind` returning `ErrorKind`, each variant's base kind is read from that function's
//!   `match self`: an arm `Enum::Variant => ErrorKind::Kind` gives `Kind`, and an arm
//!   `Enum::Variant(detail) => detail.kind()` gives [`HELD`], the kind of the detail the variant
//!   holds. A body that is not one `match self`, an arm in another form, a variant with no arm or
//!   with two, or a base kind that is not a variant of an `ErrorKind` enum in the map, is reported
//!   ([`Rule::ErrorBase`]), never guessed.
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
//! Cost: linear in the number of tokens, times a search of the crate's error type names, plus one
//! pass over each top-level `impl` block for a `kind` function.
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
    /// Its base kind, read from its enum's `kind` function; `None` when the enum has none.
    pub base: Option<Base>,
}

/// Where an error kind's base kind comes from: an arm of its enum's `kind` function (chapter 22).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Base {
    /// The arm names the kind: `XError::A => ErrorKind::NoSolution`.
    Kind(String),
    /// The arm asks the detail the variant holds: `OpErrorDetail::Num(detail) => detail.kind()`.
    Held,
}

/// How the map writes [`Base::Held`].
pub const HELD: &str = "held detail";

impl Base {
    /// The base kind's name in the map: the kind's name, or [`HELD`].
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Base::Kind(kind) => kind,
            Base::Held => HELD,
        }
    }
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
    let mut impls: Vec<usize> = Vec::new();
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
            Kind::Ident if depth == 0 && t.is_ident("impl") => {
                impls.push(at);
                pending = Pending::default();
                at = next;
            }
            _ => at = next,
        }
    }
    for start in impls {
        base_kinds(tokens, start, &mut found);
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
                    base: None,
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

/// One arm of a `kind` match: the variants its patterns name, and its base kind.
type Arm = (Vec<String>, Base);

/// Reads the `fn kind(&self) -> ErrorKind` of the `impl` block at `start` into the variants of the
/// error enum the block is for, when that enum is declared in this file; what it cannot read is
/// reported. A block for any other type, or without such a function, is passed over.
fn base_kinds(tokens: &[Token], start: usize, found: &mut Found) {
    let Some(end) = item_end(tokens, start) else {
        return;
    };
    let Some(open) = (start..end).find(|&k| tokens.get(k).is_some_and(|t| t.is_punct('{'))) else {
        return;
    };
    let header = tokens
        .get(start.saturating_add(1)..open)
        .unwrap_or_default();
    let Some(name) = self_type(header) else {
        return;
    };
    let Some(index) = found.enums.iter().position(|e| e.name == name) else {
        return;
    };
    let body = tokens
        .get(open.saturating_add(1)..end.saturating_sub(1))
        .unwrap_or_default();
    let read = match kind_function(body) {
        None => return,
        Some(Ok(arms_body)) => arms(arms_body, &name),
        Some(Err(why)) => Err(why),
    };
    let arms = match read {
        Ok(arms) => arms,
        Err(why) => {
            found.problems.push((
                Rule::ErrorBase,
                format!("`{name}::kind` cannot be read: {why}"),
            ));
            return;
        }
    };
    let mut problems = Vec::new();
    if let Some(e) = found.enums.get_mut(index) {
        for (variants, base) in arms {
            for v in variants {
                match e.variants.iter_mut().find(|x| x.name == v) {
                    Some(x) if x.base.is_none() => x.base = Some(base.clone()),
                    Some(_) => problems.push(format!(
                        "`{name}::{v}` has two arms in `{name}::kind`; give each variant one base \
                         kind"
                    )),
                    None => problems.push(format!(
                        "`{name}::kind` names `{name}::{v}`, which is not a variant of `{name}`"
                    )),
                }
            }
        }
        for v in e.variants.iter().filter(|v| v.base.is_none()) {
            problems.push(format!(
                "`{name}::{}` has no arm in `{name}::kind`, so the catalogue cannot give its base \
                 kind; name it in an arm",
                v.name
            ));
        }
    }
    found
        .problems
        .extend(problems.into_iter().map(|p| (Rule::ErrorBase, p)));
}

/// The type an `impl` header (the tokens between `impl` and `{`) is for: the last segment of the
/// path after `for`, at angle-bracket depth zero, or else of the first path after the block's own
/// generics.
fn self_type(header: &[Token]) -> Option<String> {
    let mut angle: usize = 0;
    let mut after_for = None;
    let mut first = None;
    for (k, t) in header.iter().enumerate() {
        if t.is_punct('<') {
            angle = angle.saturating_add(1);
        } else if t.is_punct('>') {
            // The `>` of `->` closes nothing.
            let arrow = k
                .checked_sub(1)
                .and_then(|p| header.get(p))
                .is_some_and(|p| p.is_punct('-'));
            if !arrow {
                angle = angle.saturating_sub(1);
            }
        } else if angle == 0 && t.kind == Kind::Ident {
            if t.is_ident("for") {
                after_for = Some(k.saturating_add(1));
            } else if first.is_none() {
                first = Some(k);
            }
        }
    }
    let mut at = after_for.or(first)?;
    let mut last = None;
    while let Some(t) = header.get(at) {
        if t.kind == Kind::Ident && !t.is_ident("where") {
            last = Some(t.text.clone());
        } else if !t.is_punct(':') {
            break;
        }
        at = at.saturating_add(1);
    }
    last
}

/// The arms of the `fn kind(&self) -> ErrorKind { match self { arms } }` at depth zero of an `impl`
/// body. `None` when the body has no function `kind` returning `ErrorKind`; an error when it has
/// one whose body is not a single `match self`.
fn kind_function(body: &[Token]) -> Option<Result<&[Token], String>> {
    let mut depth: usize = 0;
    let mut at: usize = 0;
    while let Some(t) = body.get(at) {
        if t.is_punct('{') || t.is_punct('(') || t.is_punct('[') {
            depth = depth.saturating_add(1);
        } else if t.is_punct('}') || t.is_punct(')') || t.is_punct(']') {
            depth = depth.saturating_sub(1);
        } else if depth == 0
            && t.is_ident("fn")
            && body
                .get(at.saturating_add(1))
                .is_some_and(|n| n.is_ident("kind"))
        {
            return kind_signature(body, at.saturating_add(2));
        }
        at = at.saturating_add(1);
    }
    None
}

/// The rest of a function `kind` from its parameter list at `at`: its match arms when it returns
/// `ErrorKind` (see [`kind_function`]).
fn kind_signature(body: &[Token], at: usize) -> Option<Result<&[Token], String>> {
    if !body.get(at).is_some_and(|t| t.is_punct('(')) {
        return None;
    }
    let after_params = matching(body, at)?;
    let open =
        (after_params..body.len()).find(|&k| body.get(k).is_some_and(|t| t.is_punct('{')))?;
    let returns = body.get(after_params..open).unwrap_or_default();
    let returns_kind = returns.first().is_some_and(|t| t.is_punct('-'))
        && returns.get(1).is_some_and(|t| t.is_punct('>'))
        && returns.last().is_some_and(|t| t.is_ident("ErrorKind"));
    if !returns_kind {
        return None;
    }
    let close = matching(body, open)?;
    let inner = body
        .get(open.saturating_add(1)..close.saturating_sub(1))
        .unwrap_or_default();
    let star = usize::from(inner.get(1).is_some_and(|t| t.is_punct('*')));
    let open_match = star.saturating_add(2);
    let shape = inner.first().is_some_and(|t| t.is_ident("match"))
        && inner
            .get(star.saturating_add(1))
            .is_some_and(|t| t.is_ident("self"))
        && inner.get(open_match).is_some_and(|t| t.is_punct('{'))
        && matching(inner, open_match) == Some(inner.len());
    if !shape {
        return Some(Err(
            "its body must be one `match self { ... }` whose arms the map reads".to_owned(),
        ));
    }
    Some(Ok(inner
        .get(open_match.saturating_add(1)..inner.len().saturating_sub(1))
        .unwrap_or_default()))
}

/// The tokens' text, joined by spaces, for a message.
fn spelled(tokens: &[Token]) -> String {
    tokens
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The arms of a `kind` match body: each arm's patterns `Enum::Variant` (or `Self::Variant`), each
/// with its fields or none, joined by `|`, and its value, `ErrorKind::Kind` or `detail.kind()`,
/// alone or in a block.
fn arms(body: &[Token], enum_name: &str) -> Result<Vec<Arm>, String> {
    let mut out = Vec::new();
    let mut at: usize = 0;
    while at < body.len() {
        let arrow = fat_arrow(body, at).ok_or_else(|| {
            format!(
                "`{}` is not an arm",
                spelled(body.get(at..).unwrap_or_default())
            )
        })?;
        let variants = pattern_variants(body.get(at..arrow).unwrap_or_default(), enum_name)?;
        let start = arrow.saturating_add(2);
        let (value, next) = arm_value(body, start);
        let base = base_of(value).ok_or_else(|| {
            format!(
                "the arm for `{enum_name}::{}` gives `{}`, neither `ErrorKind::<kind>` nor \
                 `<detail>.kind()`",
                variants.join(" | "),
                spelled(value)
            )
        })?;
        out.push((variants, base));
        at = next;
    }
    Ok(out)
}

/// The index of the `=` of the first `=>` at bracket depth zero from `at`.
fn fat_arrow(body: &[Token], at: usize) -> Option<usize> {
    let mut depth: usize = 0;
    for k in at..body.len() {
        let t = body.get(k)?;
        if t.is_punct('{') || t.is_punct('(') || t.is_punct('[') {
            depth = depth.saturating_add(1);
        } else if t.is_punct('}') || t.is_punct(')') || t.is_punct(']') {
            depth = depth.saturating_sub(1);
        } else if depth == 0
            && t.is_punct('=')
            && body
                .get(k.saturating_add(1))
                .is_some_and(|n| n.is_punct('>'))
        {
            return Some(k);
        }
    }
    None
}

/// The variants an arm's patterns name, each `Enum::Variant` or `Self::Variant` with at most one
/// group of fields after it.
fn pattern_variants(patterns: &[Token], enum_name: &str) -> Result<Vec<String>, String> {
    let mut variants = Vec::new();
    let mut depth: usize = 0;
    let mut part_start: usize = 0;
    for k in 0..=patterns.len() {
        let t = patterns.get(k);
        let split = match t {
            None => true,
            Some(t) if t.is_punct('{') || t.is_punct('(') || t.is_punct('[') => {
                depth = depth.saturating_add(1);
                false
            }
            Some(t) if t.is_punct('}') || t.is_punct(')') || t.is_punct(']') => {
                depth = depth.saturating_sub(1);
                false
            }
            Some(t) => depth == 0 && t.is_punct('|'),
        };
        if split {
            let part = patterns.get(part_start..k).unwrap_or_default();
            if !part.is_empty() {
                variants.push(pattern_variant(part, enum_name)?);
            }
            part_start = k.saturating_add(1);
        }
    }
    if variants.is_empty() {
        return Err("an arm names no variant".to_owned());
    }
    Ok(variants)
}

/// The variant one pattern names: `Enum::Variant`, `Self::Variant`, either followed by one group
/// of fields.
fn pattern_variant(part: &[Token], enum_name: &str) -> Result<String, String> {
    let head = part
        .first()
        .is_some_and(|t| t.is_ident(enum_name) || t.is_ident("Self"));
    let path = part.get(1).is_some_and(|t| t.is_punct(':'))
        && part.get(2).is_some_and(|t| t.is_punct(':'));
    let variant = part.get(3).filter(|t| t.kind == Kind::Ident);
    let rest_closes = match part.get(4) {
        None => true,
        Some(t) if t.is_punct('(') || t.is_punct('{') => matching(part, 4) == Some(part.len()),
        Some(_) => false,
    };
    match variant {
        Some(v) if head && path && rest_closes => Ok(v.text.clone()),
        _ => Err(format!(
            "the pattern `{}` is not `{enum_name}::<Variant>` with its fields",
            spelled(part)
        )),
    }
}

/// An arm's value from `start`, and where the next arm starts: a block and the `,` after it if
/// any, or the tokens up to the `,` at depth zero that ends it.
fn arm_value(body: &[Token], start: usize) -> (&[Token], usize) {
    if body.get(start).is_some_and(|t| t.is_punct('{'))
        && let Some(end) = matching(body, start)
    {
        let value = body
            .get(start.saturating_add(1)..end.saturating_sub(1))
            .unwrap_or_default();
        let next = if body.get(end).is_some_and(|t| t.is_punct(',')) {
            end.saturating_add(1)
        } else {
            end
        };
        return (value, next);
    }
    let mut depth: usize = 0;
    let mut k = start;
    while let Some(t) = body.get(k) {
        if t.is_punct('{') || t.is_punct('(') || t.is_punct('[') {
            depth = depth.saturating_add(1);
        } else if t.is_punct('}') || t.is_punct(')') || t.is_punct(']') {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && t.is_punct(',') {
            break;
        }
        k = k.saturating_add(1);
    }
    (body.get(start..k).unwrap_or_default(), k.saturating_add(1))
}

/// The base kind an arm's value gives: `ErrorKind::Kind`, by any path ending so, or
/// `detail.kind()` on a binding of the pattern.
fn base_of(value: &[Token]) -> Option<Base> {
    let held = value.len() == 5
        && value.first().is_some_and(|t| t.kind == Kind::Ident)
        && value.get(1).is_some_and(|t| t.is_punct('.'))
        && value.get(2).is_some_and(|t| t.is_ident("kind"))
        && value.get(3).is_some_and(|t| t.is_punct('('))
        && value.get(4).is_some_and(|t| t.is_punct(')'));
    if held {
        return Some(Base::Held);
    }
    // A path: identifiers joined by `::`, ending `ErrorKind::<Kind>`.
    let mut idents = Vec::new();
    let mut k: usize = 0;
    while let Some(t) = value.get(k) {
        if t.kind != Kind::Ident {
            return None;
        }
        idents.push(t.text.as_str());
        match value.get(k.saturating_add(1)) {
            None => break,
            Some(c) if c.is_punct(':') => {
                if !value
                    .get(k.saturating_add(2))
                    .is_some_and(|c| c.is_punct(':'))
                {
                    return None;
                }
                k = k.saturating_add(3);
            }
            Some(_) => return None,
        }
    }
    match idents.as_slice() {
        [.., "ErrorKind", kind] => Some(Base::Kind((*kind).to_owned())),
        _ => None,
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
