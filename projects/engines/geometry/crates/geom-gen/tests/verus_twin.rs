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
//! - `the_wide_twin_is_the_shipped_arithmetic_token_for_token` (piece G37): every function of
//!   `geom-num`'s wide-integer twin (`verus/wide.rs`), annotations removed, is the one in
//!   `src/wide.rs`; `every_function_the_wide_twin_runs_is_proven_and_compared` and
//!   `the_order_is_the_proven_comparison` leave nothing out, and
//!   `any_change_to_the_wide_arithmetic_is_seen` shows the comparison catches a change.
//! - `every_predicate_has_a_twin_and_the_rest_is_refused`: every predicate of the kernel has a
//!   twin, and a description with a step wider than `I384` is refused with the reason, never
//!   emitted as a twin that models it wrongly.

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

use geom_gen::predicate_derive::derive;
use geom_gen::predicate_emit::verus_twin;
use geom_gen::predicates::{PREDICATES, derivation, generate_verus_orient2d};
use proc_macro2::{Group, TokenStream, TokenTree};

/// The committed twin.
const TWIN: &str = include_str!("../../geom-num/verus/generated/orient2d.rs");

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
        if (is_ident(t, "proof") || is_ident(t, "proof_decl"))
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

/// The tokens as one canonical string: equal exactly when the token trees are equal, except for
/// the spacing of punctuation where it cannot change the program: after a `?` (no operator begins
/// with one) and before anything that is not punctuation. So a line rustfmt breaks after a `?`
/// still compares equal, and `&&` still differs from `& &`.
fn canonical(trees: &[TokenTree]) -> String {
    let mut out = String::new();
    render(trees, &mut out);
    out
}

fn render(trees: &[TokenTree], out: &mut String) {
    for (k, t) in trees.iter().enumerate() {
        match t {
            TokenTree::Group(g) => {
                let (open, close) = match g.delimiter() {
                    proc_macro2::Delimiter::Parenthesis => ("(", ")"),
                    proc_macro2::Delimiter::Brace => ("{", "}"),
                    proc_macro2::Delimiter::Bracket => ("[", "]"),
                    proc_macro2::Delimiter::None => ("", ""),
                };
                out.push_str(open);
                out.push(' ');
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                render(&inner, out);
                out.push_str(close);
                out.push(' ');
            }
            TokenTree::Ident(i) => {
                out.push_str(&i.to_string());
                out.push(' ');
            }
            TokenTree::Literal(l) => {
                out.push_str(&l.to_string());
                out.push(' ');
            }
            TokenTree::Punct(p) => {
                out.push(p.as_char());
                let next_is_punct = matches!(trees.get(k + 1), Some(TokenTree::Punct(_)));
                let joint = p.spacing() == proc_macro2::Spacing::Joint;
                if !(joint && next_is_punct && p.as_char() != '?') {
                    out.push(' ');
                }
            }
        }
    }
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
            item.contains(&format!("{coordinate} . on_grid ( )")),
            "{coordinate}: {item}"
        );
    }
    assert!(item.contains("spec_value ( a , b , c )"), "{item}");
    // A bound before each of the two products, and each of the three operations' bound after it.
    assert_eq!(item.matches("lemma_product_bound").count(), 2, "{item}");
    assert_eq!(item.matches("assert ( - bound_of_").count(), 3, "{item}");
    assert_eq!(item.matches("proof !").count(), 5, "{item}");
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
    assert!(MODEL.contains(&format!(
        "pub const COORD_MAX: i64 = {};",
        geom_num::grid::COORD_MAX
    )));
}

/// The model of `geom_num::grid`'s types the predicate twins verify against (piece G38).
const MODEL: &str = include_str!("../../geom-num/verus/model.rs");

/// Every predicate twin of the Verus crate (pieces G36 and G38) and the module it shadows.
const PREDICATE_TWINS: [(&str, &str, &str); 11] = [
    (
        "orient2d",
        include_str!("../../geom-num/verus/generated/orient2d.rs"),
        include_str!("../../geom-num/src/generated/orient2d.rs"),
    ),
    (
        "orient3d",
        include_str!("../../geom-num/verus/generated/orient3d.rs"),
        include_str!("../../geom-num/src/generated/orient3d.rs"),
    ),
    (
        "incircle",
        include_str!("../../geom-num/verus/generated/incircle.rs"),
        include_str!("../../geom-num/src/generated/incircle.rs"),
    ),
    (
        "insphere",
        include_str!("../../geom-num/verus/generated/insphere.rs"),
        include_str!("../../geom-num/src/generated/insphere.rs"),
    ),
    (
        "ssi_weight",
        include_str!("../../geom-num/verus/generated/ssi_weight.rs"),
        include_str!("../../geom-num/src/generated/ssi_weight.rs"),
    ),
    (
        "ssi_side",
        include_str!("../../geom-num/verus/generated/ssi_side.rs"),
        include_str!("../../geom-num/src/generated/ssi_side.rs"),
    ),
    (
        "plane_side",
        include_str!("../../geom-num/verus/generated/plane_side.rs"),
        include_str!("../../geom-num/src/generated/plane_side.rs"),
    ),
    (
        "tpi_weight",
        include_str!("../../geom-num/verus/generated/tpi_weight.rs"),
        include_str!("../../geom-num/src/generated/tpi_weight.rs"),
    ),
    (
        "tpi_side",
        include_str!("../../geom-num/verus/generated/tpi_side.rs"),
        include_str!("../../geom-num/src/generated/tpi_side.rs"),
    ),
    (
        "lpi_weight",
        include_str!("../../geom-num/verus/generated/lpi_weight.rs"),
        include_str!("../../geom-num/src/generated/lpi_weight.rs"),
    ),
    (
        "lpi_side",
        include_str!("../../geom-num/verus/generated/lpi_side.rs"),
        include_str!("../../geom-num/src/generated/lpi_side.rs"),
    ),
];

/// The Verus crate's root, which must hold every predicate twin as a module.
const CRATE_ROOT: &str = include_str!("../../geom-num/verus/lib.rs");

#[test]
fn every_predicate_twin_is_its_shipped_value_token_for_token() {
    let shipped_generators = [
        generate_verus_orient2d,
        geom_gen::predicates::generate_verus_orient3d,
        geom_gen::predicates::generate_verus_incircle,
        geom_gen::predicates::generate_verus_insphere,
        geom_gen::predicates::generate_verus_ssi_weight,
        geom_gen::predicates::generate_verus_ssi_side,
        geom_gen::predicates::generate_verus_plane_side,
        geom_gen::predicates::generate_verus_tpi_weight,
        geom_gen::predicates::generate_verus_tpi_side,
        geom_gen::predicates::generate_verus_lpi_weight,
        geom_gen::predicates::generate_verus_lpi_side,
    ];
    for ((name, twin, shipped), generate) in PREDICATE_TWINS.iter().zip(shipped_generators) {
        let t = find_fn(&tokens(twin), "value").unwrap_or_else(|| panic!("{name}: no value"));
        let s = find_fn(&tokens(shipped), "value").unwrap_or_else(|| panic!("{name}: no value"));
        assert_eq!(canonical(&strip(&t)), canonical(&s), "{name}");
        assert!(canonical(&t).contains("verus_spec"), "{name}");
        let fresh = String::from_utf8(generate().unwrap()).unwrap();
        assert_eq!(fresh, twin.replace("\r\n", "\n"), "{name} is not current");
        assert!(
            CRATE_ROOT.replace("\r\n", "\n").contains(&format!(
                "#[path = \"generated/{name}.rs\"]\npub mod {name};"
            )),
            "{name} is not a module of the crate"
        );
    }
}

#[test]
fn every_predicate_has_a_twin_and_the_rest_is_refused() {
    for p in &PREDICATES {
        let d = derivation(p).unwrap();
        let twin = verus_twin(&d, "test", "shipped.rs");
        assert!(twin.is_ok(), "{}: {twin:?}", p.comparison.name);
    }
    // A step wider than I384, the widest budgeted width: refused with the reason.
    let wide = derive(
        "predicate too_wide
input n m: rounded 125 in n
let a = n * m
let b = a * a
sign b
",
    )
    .unwrap();
    let e = verus_twin(&wide, "test", "shipped.rs").unwrap_err().0;
    assert!(e.contains("up to I384"), "{e}");
}

// ---------------------------------------------------------------------------------------------
// geom-num's wide-integer twin (piece G37).

/// The wide-integer twin and the module it shadows.
const WIDE_TWIN: &str = include_str!("../../geom-num/verus/wide.rs");
const WIDE: &str = include_str!("../../geom-num/src/wide.rs");

/// Every function the wide twin runs, all of them proven there.
const WIDE_PROVEN: [&str; 18] = [
    "fill",
    "is_zero",
    "signum",
    "from_magnitude",
    "checked_mul_i128",
    "resize",
    "wrapping_add_limbs",
    "wrapping_sub_limbs",
    "compare_limbs",
    "halves",
    "wrapping_neg",
    "magnitude",
    "checked_add",
    "checked_sub",
    "checked_neg",
    "compare",
    "checked_mul",
    "is_negative",
];

/// Whether the token at `k` opens the `verus!` block: `verus`, `!`, then a group.
fn is_verus_block(trees: &[TokenTree], k: usize) -> bool {
    k >= 2 && is_punct(&trees[k - 1], '!') && is_ident(&trees[k - 2], "verus")
}

/// The item `fn name` in `trees`, at any depth outside the `verus!` block, with its attributes and
/// visibility.
fn find_fn(trees: &[TokenTree], name: &str) -> Option<Vec<TokenTree>> {
    find_fns(trees, name).into_iter().next()
}

/// Every item `fn name` in `trees` outside the `verus!` block, in the order they are written (a
/// name two `impl` blocks both define comes twice).
fn find_fns(trees: &[TokenTree], name: &str) -> Vec<Vec<TokenTree>> {
    let mut found = Vec::new();
    for (k, t) in trees.iter().enumerate() {
        if is_ident(t, "fn") && trees.get(k + 1).is_some_and(|n| is_ident(n, name)) {
            if let Some(item) = item_at(trees, k) {
                found.push(item);
            }
        }
        if let TokenTree::Group(g) = t {
            if !is_verus_block(trees, k) {
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                found.extend(find_fns(&inner, name));
            }
        }
    }
    found
}

/// The item whose `fn` token is at `at`: its attributes and visibility, its signature and body.
fn item_at(trees: &[TokenTree], at: usize) -> Option<Vec<TokenTree>> {
    {
        let mut start = at;
        if start > 0 && is_ident(&trees[start - 1], "const") {
            start -= 1;
        }
        if start > 0 && is_ident(&trees[start - 1], "pub") {
            start -= 1;
        } else if start >= 2
            && is_ident(&trees[start - 2], "pub")
            && matches!(&trees[start - 1], TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Parenthesis)
        {
            // `pub(crate)` and the like.
            start -= 2;
        }
        while start >= 2
            && is_punct(&trees[start - 2], '#')
            && matches!(&trees[start - 1], TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Bracket)
        {
            start -= 2;
        }
        // The body is the first brace group that ends the signature: a brace group inside a Verus
        // clause (`ensures match r { ... },`, or `if c { ... } else { ... }`) is followed by a
        // comma, `else`, a method call or another brace group, and is skipped.
        let is_brace = |t: &TokenTree| matches!(t, TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Brace);
        let continues = |t: Option<&TokenTree>| {
            t.is_some_and(|t| {
                is_punct(t, ',') || is_punct(t, '.') || is_ident(t, "else") || is_brace(t)
            })
        };
        let body =
            (at..trees.len()).find(|&k| is_brace(&trees[k]) && !continues(trees.get(k + 1)))?;
        Some(trees[start..=body].to_vec())
    }
}

/// The names of every `fn` in `trees` outside the `verus!` block.
fn fn_names(trees: &[TokenTree], names: &mut Vec<String>) {
    for (k, t) in trees.iter().enumerate() {
        if is_ident(t, "fn") {
            if let Some(TokenTree::Ident(n)) = trees.get(k + 1) {
                names.push(n.to_string());
            }
        }
        if let TokenTree::Group(g) = t {
            if !is_verus_block(trees, k) {
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                fn_names(&inner, names);
            }
        }
    }
}

/// Whether every proven function of a twin, annotations removed, is the shipped one.
fn same_functions(twin: &str, shipped: &str) -> bool {
    let (twin, shipped) = (tokens(twin), tokens(shipped));
    WIDE_PROVEN.iter().all(
        |name| match (find_fn(&twin, name), find_fn(&shipped, name)) {
            (Some(t), Some(s)) => canonical(&strip(&t)) == canonical(&s),
            _ => false,
        },
    )
}

#[test]
fn the_wide_twin_is_the_shipped_arithmetic_token_for_token() {
    let (twin, shipped) = (tokens(WIDE_TWIN), tokens(WIDE));
    for name in WIDE_PROVEN {
        let t = find_fn(&twin, name).unwrap_or_else(|| panic!("the twin has no `fn {name}`"));
        let s = find_fn(&shipped, name).unwrap_or_else(|| panic!("wide.rs has no `fn {name}`"));
        assert_eq!(canonical(&strip(&t)), canonical(&s), "`fn {name}` differs");
        assert!(
            canonical(&t).contains("verus_spec"),
            "`fn {name}` has no specification in the twin"
        );
    }
}

#[test]
fn every_function_the_wide_twin_runs_is_proven_and_compared() {
    let mut names = Vec::new();
    fn_names(&tokens(WIDE_TWIN), &mut names);
    names.sort();
    let mut proven: Vec<String> = WIDE_PROVEN.iter().map(|n| (*n).to_owned()).collect();
    proven.sort();
    assert_eq!(names, proven);
}

#[test]
fn the_order_is_the_proven_comparison() {
    let cmp = find_fn(&tokens(WIDE), "cmp").unwrap();
    assert_eq!(
        canonical(&cmp),
        canonical(&tokens(
            "fn cmp(&self, other: &Self) -> Ordering { self.compare(other) }"
        ))
    );
}

#[test]
fn any_change_to_the_wide_arithmetic_is_seen() {
    assert!(same_functions(WIDE_TWIN, WIDE));
    for (from, to) in [
        ("let b = other.magnitude();", "let b = self.magnitude();"),
        ("carry = c1 || c2;", "carry = c1 && c2;"),
        ("i = i.wrapping_sub(1);", "i = i.wrapping_sub(2);"),
        ("let k = i.checked_add(shift)?;", "let k = i;"),
        ("} else if limb != 0 {", "} else if limb > 1 {"),
        (
            "let overflow = self.is_negative() & r.is_negative();",
            "let overflow = false;",
        ),
    ] {
        assert!(WIDE.contains(from) && WIDE_TWIN.contains(from), "{from}");
        assert!(
            !same_functions(&WIDE_TWIN.replacen(from, to, 1), WIDE),
            "twin: {from}"
        );
        assert!(
            !same_functions(WIDE_TWIN, &WIDE.replacen(from, to, 1)),
            "wide.rs: {from}"
        );
    }
}

/// The item `fn name` inside the `verus!` block of `trees`, as written there: attributes,
/// signature with its Verus clauses, and body.
fn find_fn_in_verus(trees: &[TokenTree], name: &str) -> Option<Vec<TokenTree>> {
    trees.iter().enumerate().find_map(|(k, t)| match t {
        TokenTree::Group(g) if is_verus_block(trees, k) => {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            find_fn(&inner, name)
        }
        _ => None,
    })
}

/// The parts of a function the twin must share with the shipped code when the twin writes it in
/// Verus's own syntax: its attributes (doc comment included), its parameters and its body, with
/// the body's `proof { ... }` blocks removed.
fn shared_parts(item: &[TokenTree]) -> String {
    let at = item.iter().position(|t| is_ident(t, "fn")).unwrap();
    let mut head = item[..at].to_vec();
    if head.last().is_some_and(|t| is_ident(t, "pub")) {
        head.pop();
    }
    let params = item[at + 2..]
        .iter()
        .find(|t| matches!(t, TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Parenthesis))
        .cloned()
        .unwrap();
    let body = item.last().cloned().unwrap();
    let body: Vec<TokenTree> = match body {
        TokenTree::Group(g) => g.stream().into_iter().collect(),
        _ => panic!("no body"),
    };
    let mut kept = Vec::new();
    let mut k = 0;
    while k < body.len() {
        if is_ident(&body[k], "proof")
            && matches!(body.get(k + 1), Some(TokenTree::Group(g)) if g.delimiter() == proc_macro2::Delimiter::Brace)
        {
            k += 2;
            continue;
        }
        kept.push(body[k].clone());
        k += 1;
    }
    format!(
        "{} | {} | {}",
        canonical(&head),
        canonical(&[params]),
        canonical(&strip(&kept))
    )
}

#[test]
fn from_i128_is_the_shipped_one_in_doc_attributes_parameters_and_body() {
    let twin = find_fn_in_verus(&tokens(WIDE_TWIN), "from_i128").expect("the twin's from_i128");
    let shipped = find_fn(&tokens(WIDE), "from_i128").expect("wide.rs's from_i128");
    assert_eq!(shared_parts(&twin), shared_parts(&shipped));
    let text = canonical(&twin);
    assert!(text.contains("ensures r . value ( ) == v as int"), "{text}");
    // A change on either side is seen.
    let changed = WIDE.replacen("(1u128 << 127)", "(1u128 << 126)", 1);
    let changed = find_fn(&tokens(&changed), "from_i128").unwrap();
    assert_ne!(shared_parts(&twin), shared_parts(&changed));
}

#[test]
fn product_i128_is_the_shipped_one_in_doc_attributes_parameters_and_body() {
    let twin =
        find_fn_in_verus(&tokens(WIDE_TWIN), "product_i128").expect("the twin's product_i128");
    let shipped = find_fn(&tokens(WIDE), "product_i128").expect("wide.rs's product_i128");
    assert_eq!(shared_parts(&twin), shared_parts(&shipped));
    let text = canonical(&twin);
    assert!(
        text.contains("# [ doc"),
        "the doc comment is compared: {text}"
    );
    assert!(text.contains("pub ( crate ) fn product_i128"), "{text}");
    assert!(
        text.contains("w . value ( ) == a as int * b as int"),
        "{text}"
    );
    // A change on either side is seen.
    let changed = WIDE.replacen(
        "let (l2, l3) = halves(upper);",
        "let (l3, l2) = halves(upper);",
        1,
    );
    assert!(changed != WIDE);
    let changed = find_fn(&tokens(&changed), "product_i128").unwrap();
    assert_ne!(shared_parts(&twin), shared_parts(&changed));
}

// ---------------------------------------------------------------------------------------------
// The range check of a rounded input (piece G39).

/// The twin of `within` and the generated module that ships it.
const ROUNDED_TWIN: &str = include_str!("../../geom-num/verus/rounded.rs");
const GENERATED_MOD: &str = include_str!("../../geom-num/src/generated/mod.rs");

/// Whether the twin's `within`, annotations removed and its `pub` (the predicate twins, in other
/// modules, call it) dropped, is the shipped one.
fn same_within(twin: &str, shipped: &str) -> bool {
    let t = find_fn(&tokens(twin), "within").expect("the twin's within");
    let s = find_fn(&tokens(shipped), "within").expect("mod.rs's within");
    canonical(&strip(&t)).replacen("pub fn within", "fn within", 1) == canonical(&s)
}

#[test]
fn within_is_the_shipped_range_check_token_for_token() {
    assert!(same_within(ROUNDED_TWIN, GENERATED_MOD));
    let t = find_fn(&tokens(ROUNDED_TWIN), "within").unwrap();
    assert!(canonical(&t).contains("verus_spec"));
    // A change on either side is seen.
    let (from, to) = (
        "if v.unsigned_abs() <= limit {",
        "if v.unsigned_abs() < limit {",
    );
    assert!(ROUNDED_TWIN.contains(from) && GENERATED_MOD.contains(from));
    assert!(!same_within(
        &ROUNDED_TWIN.replacen(from, to, 1),
        GENERATED_MOD
    ));
    assert!(!same_within(
        ROUNDED_TWIN,
        &GENERATED_MOD.replacen(from, to, 1)
    ));
}

// ---------------------------------------------------------------------------------------------
// The sign, plane and crossing twins (piece G39).

const SIGN_TWIN: &str = include_str!("../../geom-num/verus/sign.rs");
const SIGN: &str = include_str!("../../geom-num/src/sign.rs");
const PLANE_TWIN: &str = include_str!("../../geom-num/verus/plane.rs");
const PLANE: &str = include_str!("../../geom-num/src/plane.rs");
const CROSSING_TWIN: &str = include_str!("../../geom-num/verus/crossing.rs");
const CROSSING: &str = include_str!("../../geom-num/src/crossing.rs");

/// A hand-written twin: its name, its text, the shipped module's text, the functions it proves
/// below its `verus!` block (a name twice when two `impl` blocks both define it), and the
/// associated functions without `self` it writes inside the block.
type HandTwin = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
);

const HAND_TWINS: [HandTwin; 3] = [
    ("sign", SIGN_TWIN, SIGN, &["mul"], &["of_i128"]),
    (
        "plane",
        PLANE_TWIN,
        PLANE,
        &[
            "classify",
            "coefficients",
            "cross",
            "difference",
            "dot",
            "dot_point",
            "homogeneous",
            "homogeneous",
            "homogeneous_checked",
            "lpi_axis",
            "lpi_classify",
            "mul_sub",
            "numerator",
            "product_difference",
            "side",
            "side_of_lpi_hand",
            "side_of_tpi_hand",
            "weight_and_offset",
            "within_budget",
        ],
        &["through", "new", "new"],
    ),
    (
        "crossing",
        CROSSING_TWIN,
        CROSSING,
        &[
            "cross",
            "homogeneous",
            "offsets",
            "orient2d_hand",
            "side",
            "ssi_axis",
        ],
        &["new"],
    ),
];

/// Every item `fn name` inside the `verus!` block of `trees`, in order.
fn find_fns_in_verus(trees: &[TokenTree], name: &str) -> Vec<Vec<TokenTree>> {
    let mut found = Vec::new();
    for (k, t) in trees.iter().enumerate() {
        if let TokenTree::Group(g) = t {
            if is_verus_block(trees, k) {
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                let mut items = Vec::new();
                collect_fns_anywhere(&inner, name, &mut items);
                found.extend(items);
            }
        }
    }
    found
}

/// Every item `fn name` at any depth of `trees`.
fn collect_fns_anywhere(trees: &[TokenTree], name: &str, found: &mut Vec<Vec<TokenTree>>) {
    for (k, t) in trees.iter().enumerate() {
        if is_ident(t, "fn") && trees.get(k + 1).is_some_and(|n| is_ident(n, name)) {
            if let Some(item) = item_at(trees, k) {
                found.push(item);
            }
        }
        if let TokenTree::Group(g) = t {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            collect_fns_anywhere(&inner, name, found);
        }
    }
}

/// Whether a twin's functions, below and inside its `verus!` block, are the shipped ones.
fn hand_twin_matches(
    twin: &str,
    shipped: &str,
    below: &[&str],
    inside: &[&str],
) -> Result<(), String> {
    let (twin, shipped) = (tokens(twin), tokens(shipped));
    let mut names: Vec<&str> = below.to_vec();
    names.dedup();
    for name in names {
        let (t, s) = (find_fns(&twin, name), find_fns(&shipped, name));
        if t.len() != below.iter().filter(|n| **n == name).count() || t.len() != s.len() {
            return Err(format!(
                "`fn {name}`: {} in the twin, {} shipped",
                t.len(),
                s.len()
            ));
        }
        for (k, (t, s)) in t.iter().zip(&s).enumerate() {
            if canonical(&strip(t)) != canonical(s) {
                return Err(format!("`fn {name}` (number {}) differs", k + 1));
            }
            if !canonical(t).contains("verus_spec") {
                return Err(format!("`fn {name}` has no specification in the twin"));
            }
        }
    }
    let mut names: Vec<&str> = inside.to_vec();
    names.dedup();
    for name in names {
        let (t, s) = (find_fns_in_verus(&twin, name), find_fns(&shipped, name));
        if t.len() != inside.iter().filter(|n| **n == name).count() || t.len() != s.len() {
            return Err(format!(
                "`fn {name}` inside verus!: {} in the twin, {} shipped",
                t.len(),
                s.len()
            ));
        }
        for (k, (t, s)) in t.iter().zip(&s).enumerate() {
            if shared_parts(t) != shared_parts(s) {
                return Err(format!("`fn {name}` (number {}) differs", k + 1));
            }
        }
    }
    Ok(())
}

#[test]
fn every_hand_twin_is_the_shipped_code_token_for_token() {
    for (name, twin, shipped, below, inside) in HAND_TWINS {
        hand_twin_matches(twin, shipped, below, inside).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut found = Vec::new();
        fn_names(&tokens(twin), &mut found);
        found.sort_unstable();
        let mut proven: Vec<String> = below.iter().map(|n| (*n).to_owned()).collect();
        proven.sort_unstable();
        assert_eq!(
            found, proven,
            "{name}: every function below the verus! block is proven and compared"
        );
        assert!(
            CRATE_ROOT
                .replace("\r\n", "\n")
                .contains(&format!("pub mod {name};")),
            "{name} is not a module of the crate"
        );
    }
}

#[test]
fn any_change_to_a_hand_twin_is_seen() {
    let changes: [(usize, &str, &str); 5] = [
        (
            0,
            "(Sign::Positive, Sign::Positive) | (Sign::Negative, Sign::Negative) => Sign::Positive,",
            "(Sign::Positive, Sign::Positive) | (Sign::Negative, Sign::Negative) => Sign::Negative,",
        ),
        (
            1,
            "Some(([0, 0, 0], _)) => Err(NumError::Degenerate),",
            "Some(([0, 0, 1], _)) => Err(NumError::Degenerate),",
        ),
        (
            1,
            "numerator(&dp, &dq, &dr, &qr_y, &rp_y, &pq_y)?,",
            "numerator(&dp, &dq, &dr, &qr_y, &pq_y, &rp_y)?,",
        ),
        (
            1,
            "lpi_classify::<4>(w1, s2p, w2, s1p)",
            "lpi_classify::<4>(w1, s1p, w2, s2p)",
        ),
        (2, "difference(c.y, a.y)?,", "difference(c.x, a.y)?,"),
    ];
    for (k, from, to) in changes {
        let (name, twin, shipped, below, inside) = HAND_TWINS[k];
        assert!(
            hand_twin_matches(twin, shipped, below, inside).is_ok(),
            "{name}"
        );
        assert!(
            twin.contains(from) && shipped.contains(from),
            "{name}: {from}"
        );
        assert!(
            hand_twin_matches(&twin.replacen(from, to, 1), shipped, below, inside).is_err(),
            "{name}, twin: {from}"
        );
        assert!(
            hand_twin_matches(twin, &shipped.replacen(from, to, 1), below, inside).is_err(),
            "{name}, shipped: {from}"
        );
    }
}

/// The plane twin states the budget's limits and widths as numbers; `src/plane.rs` derives them
/// and asserts the derivation when it compiles. The two must say the same.
#[test]
fn the_plane_twins_limits_are_the_shipped_ones() {
    for line in [
        "assert!(GRID_DIFFERENCE_BITS == 41 && NORMAL_BITS == 83);",
        "assert!(OFFSET_BITS == 125 && SIDE_BITS == 126);",
        "const NORMAL_LIMIT: u128 = 1 << NORMAL_BITS;",
        "const OFFSET_LIMIT: u128 = 1 << OFFSET_BITS;",
        "pub(crate) const BUDGET_LIMBS: usize = 6;",
        "pub(crate) const TOTAL_LIMBS: usize = 8;",
    ] {
        assert!(PLANE.contains(line), "src/plane.rs: {line}");
    }
    for line in [
        "const NORMAL_LIMIT: u128 = 0x800000000000000000000;",
        "const OFFSET_LIMIT: u128 = 0x20000000000000000000000000000000;",
        "pub(crate) const BUDGET_LIMBS: usize = 6;",
        "pub(crate) const TOTAL_LIMBS: usize = 8;",
    ] {
        assert!(PLANE_TWIN.contains(line), "verus/plane.rs: {line}");
    }
    assert_eq!(0x0008_0000_0000_0000_0000_0000_u128, 1 << 83);
    assert_eq!(0x2000_0000_0000_0000_0000_0000_0000_0000_u128, 1 << 125);
}
