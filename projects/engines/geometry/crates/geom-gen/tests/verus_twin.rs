//! Attacks on piece G36 (the Verus twin spike): orient2d's exact path has a generated
//! Verus-annotated twin, and the twin, with its annotations removed, is token for token the code
//! that ships (chapter 28.7, "Predicates, by generated twins"). Verus verifies the twin in
//! `kernel-dev.yml`'s `verus` job; these tests hold the twin to the shipped code.
//!
//! - `the_twins_value_is_the_shipped_value_token_for_token`: the committed twin's `value`, with
//!   its `#[verus_spec]` attribute and `proof!` blocks removed, has exactly the tokens of the
//!   committed `geom-num` module's `value`, doc comment and attributes included.
//! - `the_twin_carries_its_annotations`: the twin's `value` states the grid for every coordinate
//!   and the description's value as its result, and bounds both products, so removing the
//!   annotations removes something.
//! - `any_change_to_either_side_is_seen`: the comparison fails on a changed operator, a changed
//!   name, an added statement, a changed doc comment and a removed attribute, on either side.
//! - `the_twin_is_current`: the committed twin is what the generator writes now, from geom-num's
//!   grid bound.
//! - `the_twin_refuses_what_it_does_not_model`: a description with rounded inputs (`plane_side`),
//!   or with a step wider than `i128` (`incircle`, `insphere`), is refused with the reason, never
//!   emitted as a twin that models it wrongly; `orient2d` and `orient3d` are covered.

// Tests may use native arithmetic to walk their token lists.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "fixtures; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken fixture must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_gen::predicate_emit::verus_twin;
use geom_gen::predicates::{PREDICATES, derivation, generate_verus_orient2d};
use proc_macro2::{Group, TokenStream, TokenTree};

/// The committed twin.
const TWIN: &str = include_str!("../generated/verus-orient2d.rs");

/// The committed module the twin shadows.
const SHIPPED: &str = include_str!("../../geom-num/src/generated/orient2d.rs");

/// The token trees of a Rust (or Verus) source.
fn tokens(text: &str) -> Vec<TokenTree> {
    text.parse::<TokenStream>()
        .unwrap_or_else(|e| panic!("does not tokenize: {e}"))
        .into_iter()
        .collect()
}

/// Whether a token is the identifier `name`.
fn is_ident(t: &TokenTree, name: &str) -> bool {
    matches!(t, TokenTree::Ident(i) if i == name)
}

/// Whether a token is the punctuation `c`.
fn is_punct(t: &TokenTree, c: char) -> bool {
    matches!(t, TokenTree::Punct(p) if p.as_char() == c)
}

/// The top-level item `fn value`: its attributes (doc comments are `#[doc]` attributes), its
/// visibility, its signature and its body.
fn value_item(text: &str) -> Vec<TokenTree> {
    let all = tokens(text);
    let at = (0..all.len().saturating_sub(1))
        .find(|&k| is_ident(&all[k], "fn") && is_ident(&all[k + 1], "value"))
        .unwrap_or_else(|| panic!("no top-level `fn value`"));
    let mut start = at;
    if start > 0 && is_ident(&all[start - 1], "pub") {
        start -= 1;
    }
    while start >= 2
        && is_punct(&all[start - 2], '#')
        && matches!(&all[start - 1], TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Bracket)
    {
        start -= 2;
    }
    let body = (at..all.len())
        .find(|&k| matches!(&all[k], TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Brace))
        .unwrap_or_else(|| panic!("`fn value` has no body"));
    all[start..=body].to_vec()
}

/// The tokens with the twin's annotations removed: every `#[verus_spec(...)]` and
/// `#[verus_verify(...)]` attribute, and every `proof! { ... }` (with its `;`, if any), at any
/// depth.
fn strip(trees: &[TokenTree]) -> Vec<TokenTree> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < trees.len() {
        let t = &trees[k];
        if is_punct(t, '#') {
            if let Some(TokenTree::Group(g)) = trees.get(k + 1) {
                let first = g.stream().into_iter().next();
                if first.is_some_and(|f| is_ident(&f, "verus_spec") || is_ident(&f, "verus_verify"))
                {
                    k += 2;
                    continue;
                }
            }
        }
        if is_ident(t, "proof")
            && trees.get(k + 1).is_some_and(|n| is_punct(n, '!'))
            && matches!(trees.get(k + 2), Some(TokenTree::Group(_)))
        {
            k += 3;
            if trees.get(k).is_some_and(|n| is_punct(n, ';')) {
                k += 1;
            }
            continue;
        }
        out.push(match t {
            TokenTree::Group(g) => {
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                TokenTree::Group(Group::new(
                    g.delimiter(),
                    strip(&inner).into_iter().collect(),
                ))
            }
            other => other.clone(),
        });
        k += 1;
    }
    out
}

/// The tokens as one canonical string: equal exactly when the token trees are equal.
fn canonical(trees: &[TokenTree]) -> String {
    trees.iter().cloned().collect::<TokenStream>().to_string()
}

/// `text` with the first `from` inside `value`'s item (from its doc comment on) replaced by `to`.
fn in_value(text: &str, from: &str, to: &str) -> String {
    let at = text
        .find("`orient2d`'s exact value")
        .unwrap_or_else(|| panic!("no doc comment of `value`"));
    let (before, item) = text.split_at(at);
    assert!(item.contains(from), "`value` has no {from:?}");
    format!("{before}{}", item.replacen(from, to, 1))
}

/// Whether a twin's `value`, annotations removed, is a shipped module's `value`, token for token.
fn same_value(twin: &str, shipped: &str) -> bool {
    canonical(&strip(&value_item(twin))) == canonical(&value_item(shipped))
}

#[test]
fn the_twins_value_is_the_shipped_value_token_for_token() {
    let twin = canonical(&strip(&value_item(TWIN)));
    let shipped = canonical(&value_item(SHIPPED));
    assert_eq!(twin, shipped);
    // The shipped module has no annotation to strip, so this is not stripping both sides alike.
    assert_eq!(canonical(&strip(&value_item(SHIPPED))), shipped);
}

#[test]
fn the_twin_carries_its_annotations() {
    let item = canonical(&value_item(TWIN));
    assert!(item.contains("verus_spec"), "{item}");
    for coordinate in ["a . x", "a . y", "b . x", "b . y", "c . x", "c . y"] {
        assert!(
            item.contains(&format!("{coordinate} . on_grid ()")),
            "{coordinate}: {item}"
        );
    }
    assert!(item.contains("spec_value (a , b , c)"), "{item}");
    assert_eq!(item.matches("proof !").count(), 2, "{item}");
    assert!(item.len() > canonical(&value_item(SHIPPED)).len());
}

#[test]
fn any_change_to_either_side_is_seen() {
    assert!(same_value(TWIN, SHIPPED));
    let changes = [
        ("det_1.checked_sub(det_2)?", "det_1.checked_add(det_2)?"),
        ("let ba_y = b.y.get()", "let ba_y = b.x.get()"),
        ("    Some(det)\n", "    let det = det;\n    Some(det)\n"),
        (
            "`None` only if a step overflowed",
            "`None` if a step overflowed",
        ),
        ("#[must_use]\n", ""),
    ];
    for (from, to) in changes {
        assert!(
            !same_value(&in_value(TWIN, from, to), SHIPPED),
            "twin: {from} -> {to}"
        );
        assert!(
            !same_value(TWIN, &in_value(SHIPPED, from, to)),
            "shipped: {from} -> {to}"
        );
    }
}

#[test]
fn the_twin_is_current() {
    let fresh = String::from_utf8(generate_verus_orient2d().unwrap()).unwrap();
    assert_eq!(fresh, TWIN.replace("\r\n", "\n"));
    assert!(TWIN.contains(&format!(
        "pub const COORD_MAX: i64 = {};",
        geom_num::grid::COORD_MAX
    )));
}

#[test]
fn the_twin_refuses_what_it_does_not_model() {
    for p in &PREDICATES {
        let d = derivation(p).unwrap();
        let twin = verus_twin(&d, "test", "shipped.rs");
        match p.comparison.name {
            "orient2d" | "orient3d" => {
                assert!(twin.is_ok(), "{}: {twin:?}", p.comparison.name);
            }
            "plane_side" => {
                let e = twin.unwrap_err().0;
                assert!(e.contains("rounded input"), "{e}");
            }
            "incircle" | "insphere" => {
                let e = twin.unwrap_err().0;
                assert!(e.contains("wider") || e.contains("wide integers"), "{e}");
            }
            other => panic!("a predicate this test does not know: {other}"),
        }
    }
}
