//! What a source file declares, read from its tokens: whether it holds code, the functions it
//! defines outside trait implementations, the types it uses as errors, and its doc comments.
//!
//! What it does: [`scan`] walks one file's tokens ([`crate::map::rust`]) and returns its
//! [`Items`]: whether it declares any item other than `mod` and `use` lines (a crate root that
//! does is a module too, with a module's fields); every function with a body that is not part of
//! a trait implementation (a module that defines one implements an algorithm, so `Method: none`
//! does not fit it); every type a trait implementation of `Error` is for, and every type a
//! `Result<_, E>` names as its error (so an error enum is found by what it is, whatever its name);
//! and every outer doc comment (so a link to outside code in an item's documentation needs a
//! credit as one in the header does). Items under `#[cfg(test)]` are skipped whole.
//!
//! Guarantees:
//!
//! - **Only real code.** The scan runs on tokens, so text in comments, strings and doc examples
//!   is never read as an item; `fn` in a type (`fn(&[u8]) -> u8`) is not a function.
//! - **A trait implementation is told from an inherent one by its `for`.** `impl Trait for Type`
//!   bodies are trait implementations; `for<'a>` (a higher-ranked bound) is not that `for`.
//! - **Total.** No input panics, and every step moves forward, so the scan ends.
//!
//! Method: a single pass over the token stream with a stack of open braces, as the error-kind
//! scanner ([`crate::map::errors`]) reads items, after the item grammar of The Rust Reference:
//! implementations (<https://doc.rust-lang.org/reference/items/implementations.html>), functions
//! (<https://doc.rust-lang.org/reference/items/functions.html>) and the `Result` type's
//! parameters (<https://doc.rust-lang.org/std/result/enum.Result.html>).
//!
//! Cost: linear in the number of tokens, plus one forward look per `impl`, `fn` and `Result`,
//! each to the end of its header.

use std::collections::BTreeSet;

use crate::map::errors::{item_end, matching};
use crate::map::rust::{Kind, Token};

/// What one file declares.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Items {
    /// Whether it declares any item other than `mod` and `use` lines and attributes.
    pub code: bool,
    /// Every function with a body outside a trait implementation, by name, in source order.
    pub functions: Vec<String>,
    /// Every type a trait implementation of a trait named `Error` is for.
    pub error_impls: Vec<String>,
    /// Every type named as the error of a `Result<_, E>`.
    pub result_errors: Vec<String>,
    /// Every outer doc comment's text, in source order.
    pub docs: Vec<String>,
}

/// The item keywords that make a file hold code.
const CODE_ITEMS: [&str; 10] = [
    "struct",
    "enum",
    "union",
    "fn",
    "const",
    "static",
    "trait",
    "type",
    "impl",
    "macro_rules",
];

/// Whether the token at `at` is a `>` that closes an angle bracket, not the `>` of `->` or `=>`.
fn closes_angle(tokens: &[Token], at: usize) -> bool {
    tokens.get(at).is_some_and(|t| t.is_punct('>'))
        && !at
            .checked_sub(1)
            .and_then(|b| tokens.get(b))
            .is_some_and(|t| t.is_punct('-') || t.is_punct('='))
}

/// The last identifier at angle depth zero in `tokens`, stopping at `stop` (a `{`, `where` or the
/// end): the name of a path such as `core::error::Error` or `X<T>`.
fn last_name(tokens: &[Token]) -> Option<String> {
    let mut angle: usize = 0;
    let mut name = None;
    for (k, t) in tokens.iter().enumerate() {
        if t.is_punct('<') {
            angle = angle.saturating_add(1);
        } else if closes_angle(tokens, k) {
            angle = angle.saturating_sub(1);
        } else if angle == 0 && t.kind == Kind::Ident {
            if t.is_ident("where") {
                break;
            }
            if !matches!(t.text.as_str(), "dyn" | "mut" | "const" | "unsafe") {
                name = Some(t.text.clone());
            }
        }
    }
    name
}

/// The index of the `{` that opens the body of the item starting at `at`: its first `{` outside
/// parentheses and brackets, or `None` when a `;` ends the item first.
fn body_open(tokens: &[Token], at: usize) -> Option<usize> {
    let mut k = at;
    while let Some(t) = tokens.get(k) {
        if t.is_punct(';') {
            return None;
        }
        if t.is_punct('{') {
            return Some(k);
        }
        k = if t.is_punct('(') || t.is_punct('[') {
            matching(tokens, k)?
        } else {
            k.checked_add(1)?
        };
    }
    None
}

/// Reads the `impl` at `at`: whether it is a trait implementation (and the index of its body's
/// `{`), the trait's name and the type's name.
fn implementation(tokens: &[Token], at: usize) -> Option<(usize, Option<(String, String)>)> {
    let open = body_open(tokens, at)?;
    let header = tokens.get(at.checked_add(1)?..open)?;
    let mut angle: usize = 0;
    let mut for_at = None;
    for (k, t) in header.iter().enumerate() {
        if t.is_punct('<') {
            angle = angle.saturating_add(1);
        } else if closes_angle(header, k) {
            angle = angle.saturating_sub(1);
        } else if angle == 0
            && t.is_ident("for")
            && !header
                .get(k.saturating_add(1))
                .is_some_and(|n| n.is_punct('<'))
        {
            for_at = Some(k);
            break;
        }
    }
    let names = for_at.and_then(|k| {
        let trait_name = last_name(header.get(..k)?)?;
        let type_name = last_name(header.get(k.checked_add(1)?..)?)?;
        Some((trait_name, type_name))
    });
    Some((open, names))
}

/// Reads the `Result` at `at` when it is `Result<_, E>`: the name of `E`.
fn result_error(tokens: &[Token], at: usize) -> Option<String> {
    let open = at.checked_add(1)?;
    if !tokens.get(open).is_some_and(|t| t.is_punct('<')) {
        return None;
    }
    let mut angle: usize = 0;
    let mut nested: usize = 0;
    let mut comma: Option<usize> = None;
    for k in open..tokens.len() {
        let t = tokens.get(k)?;
        if t.is_punct('<') {
            angle = angle.saturating_add(1);
        } else if closes_angle(tokens, k) {
            angle = angle.saturating_sub(1);
            if angle == 0 {
                let after = comma?.checked_add(1)?;
                return last_path(tokens.get(after..k)?);
            }
        } else if t.is_punct('(') || t.is_punct('[') {
            nested = nested.saturating_add(1);
        } else if t.is_punct(')') || t.is_punct(']') {
            nested = nested.saturating_sub(1);
        } else if t.is_punct(',') && angle == 1 && nested == 0 {
            comma = Some(k);
        } else if t.is_punct(';') || t.is_punct('{') {
            return None;
        }
    }
    None
}

/// The last identifier of the path that opens `tokens`, before any `<`.
fn last_path(tokens: &[Token]) -> Option<String> {
    let mut name = None;
    for t in tokens {
        if t.is_punct('<') {
            break;
        }
        if t.kind == Kind::Ident {
            name = Some(t.text.clone());
        }
    }
    name
}

/// Whether the attribute at `at` (`#[...]`) is `#[cfg(test)]`, and the index past it.
fn attribute(tokens: &[Token], at: usize) -> (bool, usize) {
    let next = at.saturating_add(1);
    let inner = tokens.get(next).is_some_and(|t| t.is_punct('!'));
    let open = if inner { next.saturating_add(1) } else { next };
    if !tokens.get(open).is_some_and(|t| t.is_punct('[')) {
        return (false, next);
    }
    let Some(end) = matching(tokens, open) else {
        return (false, tokens.len());
    };
    let body: Vec<&str> = tokens
        .get(open.saturating_add(1)..end.saturating_sub(1))
        .unwrap_or_default()
        .iter()
        .map(|t| t.text.as_str())
        .collect();
    (!inner && body == ["cfg", "(", "test", ")"], end)
}

/// What one file declares (module header).
#[must_use]
pub fn scan(tokens: &[Token]) -> Items {
    let mut items = Items::default();
    // For each open brace, whether it is the body of a trait implementation.
    let mut braces: Vec<bool> = Vec::new();
    let mut trait_bodies: BTreeSet<usize> = BTreeSet::new();
    let mut cfg_test = false;
    let mut docs: Vec<String> = Vec::new();
    let mut at: usize = 0;
    while let Some(t) = tokens.get(at) {
        let next = at.saturating_add(1);
        let top = braces.is_empty();
        match t.kind {
            Kind::Doc => {
                docs.push(t.text.clone());
                at = next;
                continue;
            }
            Kind::Punct if t.is_punct('#') => {
                let (test, end) = attribute(tokens, at);
                cfg_test |= test;
                at = end.max(next);
                continue;
            }
            Kind::Ident if cfg_test => {
                // A test-only item: skipped whole, its docs with it.
                at = item_end(tokens, at).unwrap_or(tokens.len()).max(next);
                cfg_test = false;
                docs.clear();
                continue;
            }
            _ => {}
        }
        items.docs.append(&mut docs);
        match t.kind {
            Kind::Punct if t.is_punct('{') => braces.push(trait_bodies.contains(&at)),
            Kind::Punct if t.is_punct('}') => {
                braces.pop();
            }
            Kind::Punct if t.is_punct('!') && top => {
                // A macro invocation at the top level generates items.
                let macro_call = at
                    .checked_sub(1)
                    .and_then(|b| tokens.get(b))
                    .is_some_and(|p| p.kind == Kind::Ident);
                items.code |= macro_call;
            }
            Kind::Ident => {
                if top && CODE_ITEMS.contains(&t.text.as_str()) {
                    items.code = true;
                }
                if top && t.is_ident("mod") {
                    // An inline module's body is code; `mod x;` is not.
                    let inline = tokens
                        .get(at.saturating_add(2))
                        .is_some_and(|b| b.is_punct('{'));
                    items.code |= inline;
                }
                if t.is_ident("impl")
                    && let Some((open, names)) = implementation(tokens, at)
                    && let Some((trait_name, type_name)) = names
                {
                    trait_bodies.insert(open);
                    if trait_name == "Error" {
                        items.error_impls.push(type_name);
                    }
                }
                if t.is_ident("fn")
                    && let Some(name) = tokens.get(next).filter(|n| n.kind == Kind::Ident)
                {
                    let has_body = item_end(tokens, at)
                        .and_then(|end| end.checked_sub(1))
                        .and_then(|last| tokens.get(last))
                        .is_some_and(|last| last.is_punct('}'));
                    if has_body && !braces.iter().any(|b| *b) {
                        items.functions.push(name.text.clone());
                    }
                }
                if t.is_ident("Result")
                    && let Some(error) = result_error(tokens, at)
                {
                    items.result_errors.push(error);
                }
            }
            _ => {}
        }
        cfg_test = false;
        at = next;
    }
    items
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

    fn of(source: &str) -> Items {
        scan(&tokens(source).unwrap())
    }

    #[test]
    fn functions_outside_trait_implementations() {
        let items = of("
            /// A sign.
            pub enum Sign { Negative, Zero }
            impl Sign { pub const fn negate(self) -> Sign { self } }
            impl core::ops::Neg for Sign { type Output = Sign; fn neg(self) -> Sign { self } }
            impl<T> From<T> for Sign where T: for<'a> Fn(&'a u8) { fn from(_: T) -> Sign { Sign::Zero } }
            pub trait Shape { fn area(&self) -> u8; fn twice(&self) -> u8 { 2 } }
            fn free() { fn inner() {} }
            pub type Harness = fn(&[u8]) -> Result<(), Finding>;
            #[cfg(test)]
            mod tests { fn helper() {} }
        ");
        assert_eq!(items.functions, ["negate", "twice", "free", "inner"]);
        assert!(items.code);
        assert_eq!(items.result_errors, ["Finding"]);
        assert_eq!(items.docs, ["A sign."]);
    }

    #[test]
    fn error_types_by_what_they_are() {
        let items = of("
            pub enum SweepFailure { MitreMismatch }
            impl core::error::Error for SweepFailure {}
            impl<E> std::error::Error for Wrapped<E> {}
            impl fmt::Display for SweepFailure { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { Ok(()) } }
            pub fn sweep(x: Vec<(u8, u8)>) -> Result<Vec<u8>, crate::sweep::SweepFailure> { Ok(x) }
            pub fn other() -> io::Result<u8> { Ok(1) }
            pub fn boxed() -> Result<u8, Box<dyn Error>> { Ok(1) }
        ");
        assert_eq!(items.error_impls, ["SweepFailure", "Wrapped"]);
        assert_eq!(items.result_errors, ["SweepFailure", "Box"]);
        assert!(items.functions.contains(&"sweep".to_owned()));
        assert!(!items.functions.contains(&"fmt".to_owned()));
    }

    #[test]
    fn a_root_of_modules_and_uses_holds_no_code() {
        let root = "#![forbid(unsafe_code)]\n//! Doc.\npub mod a;\nmod b;\npub use a::X;\n\
                    extern crate alloc;\n#[cfg(test)]\nfn test_only() {}";
        assert!(!of(root).code);
        assert!(of("mod inline { pub fn f() {} }").code);
        assert!(of("include!(\"x.rs\");").code);
        assert!(of("pub const X: u8 = 1;").code);
    }
}
