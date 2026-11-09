//! The XML reader's fuzz target (piece G414a), as tests: `xml_fuzz` is bolero's test mode, which
//! cargo-bolero, `ClusterFuzzLite` and Kani select by name, and `xml_seeded` the seeded leg that
//! runs on every target, WebAssembly included (the crate README sets out the layout). Each hands
//! the fuzzer's raw bytes to `geom_fuzz::xml::harness`, which reads them with `geom-exchange`'s
//! hardened XML reader.
//!
//! `seeded_markup` adds a leg random bytes rarely reach: documents built from XML's own pieces
//! (tags, attributes, references, comments, CDATA, document types), so the reader's paths run on
//! every pull request too.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::holds;

use geom_fuzz::seeded;
use geom_fuzz::xml::harness;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 20_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 20_000;

/// The longest byte string the seeded leg draws.
const MAX_LEN: u16 = 512;

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn xml_fuzz() {
    println!(
        "bolero test mode: corpus and crash files, then {ITERATIONS} random inputs whose seeds \
         bolero draws from the operating system; a failing input's seed is printed as \
         BOLERO_RANDOM_SEED=<seed>, which replays it"
    );
    bolero::check!()
        .with_iterations(ITERATIONS)
        .with_max_len(usize::from(MAX_LEN))
        .for_each(|data: &[u8]| holds(harness(data)));
}

#[test]
fn xml_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "xml fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn seeded_markup() {
    let seed = resolve("xml fuzz target, markup", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let pieces = [
        "<?xml version=\"1.0\"?>",
        "<?xml version='1.0' encoding='UTF-8' standalone='yes'?>",
        "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>",
        "<a>",
        "</a>",
        "<b:c>",
        "</b:c>",
        "<a/>",
        "<a x=\"1\">",
        "<a x='1' x='2'>",
        "<a x=\"&amp;&#65;&#x42;\" y=\"&lt;\">",
        " z=\"&ent;\"",
        "text",
        " ",
        "&lt;&gt;&amp;&apos;&quot;",
        "&#0;",
        "&#x110000;",
        "&ent;",
        "<!-- note -->",
        "<!-- bad -- note -->",
        "<![CDATA[ <raw> ]]>",
        "]]>",
        "<?pi data?>",
        "<!DOCTYPE a [<!ENTITY e \"x\">]>",
        "<!DOCTYPE a SYSTEM \"file:///etc/passwd\">",
        "<!ENTITY x \"y\">",
        "\u{FEFF}",
        "\u{1F600}",
        "<\u{E9}l\u{E9}ment>",
    ];
    let cases = 20_000;
    for _ in 0..cases {
        let mut doc = String::new();
        let n = 1 + rng.next_u64() % 12;
        for _ in 0..n {
            doc.push_str(pieces[(rng.next_u64() % pieces.len() as u64) as usize]);
        }
        holds(harness(doc.as_bytes()));
    }
}
