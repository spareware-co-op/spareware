//! The Part 21 fuzz target (piece G415), as tests: `p21_fuzz` is bolero's test mode, which
//! cargo-bolero and `ClusterFuzzLite` select by name, and `p21_seeded` the seeded leg that runs on
//! every target, WebAssembly included (the crate README sets out the layout). Each hands the
//! fuzzer's raw bytes to `geom_fuzz::p21::harness`, which reads them as an ISO 10303-21 file with
//! `geom-p21`.
//!
//! `seeded_token_cases` adds a leg random bytes rarely reach: byte strings built from Part 21's
//! own tokens around a file's frame, so the reader's and the writer's paths run on every pull
//! request too. The two `a_broken_*` tests show the harness catches a reader that breaks its laws.

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

use core::num::NonZeroU64;

use common::holds;

use geom_fuzz::p21::{check_reader_with, harness, trace};
use geom_fuzz::seeded;
use geom_p21::Options;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 20_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 20_000;

/// The longest byte string the seeded leg draws; the harness reads all of it.
const MAX_LEN: u16 = 512;

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn p21_fuzz() {
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
fn p21_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "p21 fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn seeded_token_cases() {
    let seed = resolve("p21 fuzz target, tokens", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let tokens = [
        "ISO-10303-21;",
        "HEADER;",
        "ENDSEC;",
        "DATA;",
        "END-ISO-10303-21;",
        "FILE_SCHEMA(('S'));",
        "ANCHOR;",
        "SIGNATURE",
        "#1=",
        "#2=",
        "#",
        "A(",
        "!B(",
        "(",
        ")",
        ");",
        ",",
        ";",
        "$",
        "*",
        "'",
        "''",
        "'x'",
        "\\X2\\",
        "00E9",
        "D83D",
        "DE00",
        "\\X0\\",
        "\\X4\\",
        "0001F600",
        "\\X\\E9",
        "\\S\\",
        "\\",
        "1",
        "-2",
        "1.5",
        "1.E-3",
        "1E400",
        ".5",
        ".T.",
        ".",
        "\"0F\"",
        "\"",
        "/*",
        "*/",
        " ",
        "\n",
        "E",
        "\u{e9}",
        "99999999999999999999",
    ];
    let mut accepted = 0_u32;
    for _ in 0..20_000 {
        let mut text = String::new();
        let framed = rng.below(NonZeroU64::new(4).unwrap()) > 0;
        if framed {
            text.push_str("ISO-10303-21;HEADER;ENDSEC;DATA;");
        }
        let n = rng.below(NonZeroU64::new(16).unwrap());
        for _ in 0..n {
            text.push_str(
                tokens[rng.below(NonZeroU64::new(tokens.len() as u64).unwrap()) as usize],
            );
        }
        if framed {
            text.push_str("ENDSEC;END-ISO-10303-21;");
        }
        holds(harness(text.as_bytes()));
        accepted += u32::from(
            trace(
                geom_format::source::SliceSource::new(text.as_bytes()),
                Options::DEFAULT,
            )
            .1
            .is_none(),
        );
    }
    assert!(
        accepted > 200,
        "only {accepted} of the token cases were files"
    );
}

const FILE: &[u8] =
    b"ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n#1=A(1,\n'x');\nENDSEC;\nEND-ISO-10303-21;\n";

#[test]
fn a_broken_split_is_a_finding() {
    assert!(check_reader_with(FILE, 0, |s, o| trace(s, o), |s, o| trace(s, o)).is_ok());
    // A reader that loses an event when its bytes come in pieces.
    let finding = check_reader_with(
        FILE,
        0,
        |s, o| trace(s, o),
        |source, options| {
            let mut read = trace(source, options);
            read.0.pop();
            read
        },
    )
    .unwrap_err();
    assert!(finding.to_string().contains("split"), "{finding}");
}

#[test]
fn a_refusal_outside_the_input_is_a_finding() {
    let truncated = &FILE[..FILE.len() - 20];
    assert!(check_reader_with(truncated, 9, |s, o| trace(s, o), |s, o| trace(s, o)).is_ok());
    for (offset, line) in [(1_000, 1), (0, 0), (0, 1_000)] {
        let wrong = |source: geom_format::source::SliceSource<'_>, options| {
            let mut read = trace(source, options);
            if let Some(e) = read.1.as_mut() {
                e.offset += offset;
                e.line = if line == 1 { e.line } else { line };
            }
            read
        };
        let finding = check_reader_with(truncated, 9, wrong, |s, o| trace(s, o)).unwrap_err();
        assert!(finding.to_string().contains("inside"), "{finding}");
    }
}
