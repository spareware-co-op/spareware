//! Which code ships: `#[cfg(...)]` and `#[cfg_attr(...)]` read for the recursion check.
//!
//! What it does: [`effective`] expands every `#[cfg_attr(predicate, attributes...)]` whose
//! predicate is not known to be false into the attributes it adds, and [`ships`] says whether an
//! item, statement or expression with those attributes is compiled into the crates that ship. A
//! predicate is read in three values: `test`, `doc`, `doctest`, `kani` and `miri` are false (test,
//! documentation, proof and interpreter builds are not what ships); every other option (`feature
//! = "..."`, `target_arch = "..."`, `debug_assertions`, ...) is unknown, because some build that
//! ships may set it. `all`, `any` and `not` combine them as Kleene's strong three-valued logic.
//!
//! Guarantees:
//!
//! - **Over every build that ships, never one.** Code is left out only when its predicate is false
//!   in every build of the crates that ship, so the call graph is the union of every feature and
//!   target, and code behind a feature or a target is analysed like the rest.
//! - **A predicate it cannot read keeps the code.** A `cfg` or `cfg_attr` whose arguments do not
//!   parse is unknown, never false, so unreadable configuration never hides code.
//!
//! Method: the conditional-compilation grammar of The Rust Reference
//! (<https://doc.rust-lang.org/reference/conditional-compilation.html>), its `cfg_attr` expansion
//! (<https://doc.rust-lang.org/reference/conditional-compilation.html#the-cfg_attr-attribute>),
//! evaluated in Kleene's strong logic of indeterminacy (S. C. Kleene, *Introduction to
//! Metamathematics*, 1952, section 64; <https://en.wikipedia.org/wiki/Three-valued_logic>).
//!
//! Cost: linear in the size of the attributes; no recursion beyond the nesting of `all`, `any`,
//! `not` and `cfg_attr` as written.

use syn::punctuated::Punctuated;
use syn::{Attribute, Meta, Token};

/// The configuration options that are false in every build that ships.
pub const NOT_SHIPPED: [&str; 5] = ["test", "doc", "doctest", "kani", "miri"];

/// A predicate's value over the builds that ship.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Truth {
    /// True in every build that ships.
    True,
    /// False in every build that ships.
    False,
    /// True in some and false in others, or not known.
    Unknown,
}

impl Truth {
    fn not(self) -> Truth {
        match self {
            Truth::True => Truth::False,
            Truth::False => Truth::True,
            Truth::Unknown => Truth::Unknown,
        }
    }
}

/// The value of one configuration predicate.
#[must_use]
pub fn predicate(meta: &Meta) -> Truth {
    match meta {
        Meta::Path(path) => {
            if NOT_SHIPPED.iter().any(|name| path.is_ident(name)) {
                Truth::False
            } else {
                Truth::Unknown
            }
        }
        Meta::NameValue(_) => Truth::Unknown,
        Meta::List(list) => {
            let Ok(parts) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            else {
                return Truth::Unknown;
            };
            let values: Vec<Truth> = parts.iter().map(predicate).collect();
            if list.path.is_ident("all") {
                if values.contains(&Truth::False) {
                    Truth::False
                } else if values.iter().all(|v| *v == Truth::True) {
                    Truth::True
                } else {
                    Truth::Unknown
                }
            } else if list.path.is_ident("any") {
                if values.contains(&Truth::True) {
                    Truth::True
                } else if values.iter().all(|v| *v == Truth::False) {
                    Truth::False
                } else {
                    Truth::Unknown
                }
            } else if list.path.is_ident("not") {
                match values.as_slice() {
                    [one] => one.not(),
                    _ => Truth::Unknown,
                }
            } else {
                Truth::Unknown
            }
        }
    }
}

/// The attributes in force: each `cfg_attr` whose predicate is not false replaced by the
/// attributes it adds (themselves expanded), every other attribute as written.
#[must_use]
pub fn effective(attrs: &[Attribute]) -> Vec<Meta> {
    let mut out = Vec::new();
    let mut work: Vec<Meta> = attrs.iter().rev().map(|a| a.meta.clone()).collect();
    while let Some(meta) = work.pop() {
        let Meta::List(list) = &meta else {
            out.push(meta);
            continue;
        };
        if !list.path.is_ident("cfg_attr") {
            out.push(meta);
            continue;
        }
        let Ok(parts) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        else {
            // Unreadable: keep it as written, so `ships` still sees no false `cfg` in it.
            out.push(meta);
            continue;
        };
        let mut parts = parts.into_iter();
        let Some(condition) = parts.next() else {
            continue;
        };
        if predicate(&condition) == Truth::False {
            continue;
        }
        let added: Vec<Meta> = parts.collect();
        work.extend(added.into_iter().rev());
    }
    out
}

/// Whether code with these attributes is compiled into some build that ships: no `#[cfg]` in
/// force is false.
#[must_use]
pub fn ships(attrs: &[Attribute]) -> bool {
    effective(attrs).iter().all(|meta| match meta {
        Meta::List(list) if list.path.is_ident("cfg") => list
            .parse_args::<Meta>()
            .map_or(true, |p| predicate(&p) != Truth::False),
        _ => true,
    })
}

/// The names in every `#[derive(...)]` in force, last path segment only, in order.
#[must_use]
pub fn derives(attrs: &[Attribute]) -> Vec<String> {
    let mut out = Vec::new();
    for meta in effective(attrs) {
        let Meta::List(list) = &meta else {
            continue;
        };
        if !list.path.is_ident("derive") {
            continue;
        }
        if let Ok(paths) =
            list.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
        {
            for path in paths {
                if let Some(last) = path.segments.last() {
                    out.push(last.ident.to_string());
                }
            }
        }
    }
    out
}

/// The value of a `#[path = "..."]` attribute in force, if any.
#[must_use]
pub fn path_attribute(attrs: &[Attribute]) -> Option<String> {
    effective(attrs).iter().find_map(|meta| match meta {
        Meta::NameValue(nv) if nv.path.is_ident("path") => match &nv.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) => Some(s.value()),
            _ => None,
        },
        _ => None,
    })
}

/// Whether the attributes in force include `#[macro_export]`.
#[must_use]
pub fn macro_export(attrs: &[Attribute]) -> bool {
    effective(attrs)
        .iter()
        .any(|meta| meta.path().is_ident("macro_export"))
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use super::*;

    fn attrs(source: &str) -> Vec<Attribute> {
        let item: syn::ItemFn = syn::parse_str(&format!("{source} fn f() {{}}")).unwrap();
        item.attrs
    }

    #[test]
    fn test_kani_doc_and_miri_code_does_not_ship() {
        for cfg in [
            "#[cfg(test)]",
            "#[cfg(kani)]",
            "#[cfg(doc)]",
            "#[cfg(doctest)]",
            "#[cfg(miri)]",
            "#[cfg(all(test, feature = \"x\"))]",
            "#[cfg(any(test, kani))]",
            "#[cfg(not(any(feature = \"x\", not(test))))]",
            "#[cfg_attr(feature = \"x\", cfg(test))]",
        ] {
            assert!(!ships(&attrs(cfg)), "{cfg}");
        }
    }

    #[test]
    fn every_build_that_may_ship_is_kept() {
        for cfg in [
            "",
            "#[cfg(not(test))]",
            "#[cfg(feature = \"threads\")]",
            "#[cfg(target_arch = \"wasm32\")]",
            "#[cfg(any(test, feature = \"x\"))]",
            "#[cfg(all(not(test), debug_assertions))]",
            "#[cfg(not(all(test, feature = \"x\")))]",
            "#[cfg_attr(test, cfg(feature = \"x\"))]",
            "#[cfg(this is not a predicate)]",
            "#[inline]",
        ] {
            assert!(ships(&attrs(cfg)), "{cfg}");
        }
    }

    #[test]
    fn derives_added_by_cfg_attr_count_unless_their_predicate_is_false() {
        let a = attrs(
            "#[derive(Clone, core::fmt::Debug)] #[cfg_attr(feature = \"x\", derive(Hash))] \
             #[cfg_attr(test, derive(PartialEq))] \
             #[cfg_attr(not(test), cfg_attr(feature = \"y\", derive(Ord)))]",
        );
        assert_eq!(derives(&a), ["Clone", "Debug", "Hash", "Ord"]);
    }

    #[test]
    fn path_attributes_are_read() {
        let a = attrs("#[path = \"x/y.rs\"]");
        assert_eq!(path_attribute(&a).as_deref(), Some("x/y.rs"));
        assert_eq!(path_attribute(&attrs("#[inline]")), None);
    }
}
