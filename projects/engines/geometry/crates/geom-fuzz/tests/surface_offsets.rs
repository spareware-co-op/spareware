//! The offset surface's fuzz target, run as tests on stable Rust: bolero's test mode, the seeded
//! leg, the input bound, the same bytes as the surfaces target, the offset reached, issue #765's
//! smallest offset reaching the pass after its budget stop, and a broken kind that stops caught.
//!
//! - `surface_offsets_fuzz` wraps `geom_fuzz::surface_offsets::harness` in `bolero::check!`. Under
//!   `cargo test` it replays the files in `tests/__fuzz__/surface_offsets/corpus` and `crashes`,
//!   if any, then runs [`ITERATIONS`] random byte strings; bolero prints a failing one's seed as
//!   `BOLERO_RANDOM_SEED=<seed>`, which replays it. Under
//!   `cargo bolero test surface_offsets_fuzz --package geom-fuzz` the same test is a libFuzzer
//!   target.
//! - `surface_offsets_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the
//!   printed seed (`SPAREKIT_SEED` replays it), the same on every target.
//! - `the_bytes_are_read_as_the_surfaces_target_reads_them`: every byte string decodes to the
//!   `surfaces` target's case with the kind made an offset, so the files moved here from that
//!   target's folder are the cases they were.
//! - `the_offset_is_reached`: an offset is built and held to the contract.
//! - `the_smallest_offset_of_issue_765_reaches_the_pass`: issue #765's 15-byte input stops on the
//!   budget, and the pass without conversions then runs every law it holds, to the end.
//! - `a_broken_kind_that_stops_is_a_finding`: a kind whose bound misses its own points and whose
//!   conversions spend the whole budget is caught by the pass (28.4, fault injection).

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their inputs.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test inputs; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

#[path = "broken/surface.rs"]
mod broken;
mod common;

use broken::Broken;
use common::holds;
use geom_fuzz::surface_offsets::{MAX_INPUT_BYTES, Reached, check_with, decode, harness, reached};
use geom_fuzz::{seeded, surfaces};
use geom_math::{Axis, GridTransform, TypedTransform};
use geom_nurbs::{Basis, Surface};
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the fuzz target in bolero's test mode: about as
/// many offsets as the surfaces target's 1,000 held before the split (one in eleven), an offset's
/// contract being the dearest of all the kinds'.
const ITERATIONS: usize = 100;

/// Byte strings in the seeded leg, for the same reason.
const SEEDED_CASES: u32 = 100;

/// The longest byte string the fuzz target and the seeded leg try: the surfaces target's, as the
/// bytes are read the same way.
const MAX_LEN: u16 = 320;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn surface_offsets_fuzz() {
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
fn surface_offsets_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "surface offsets fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_bytes_are_read_as_the_surfaces_target_reads_them() {
    let mut rng = SplitMix64::new(Seed::DEFAULT);
    for len in 0..=usize::from(MAX_LEN) {
        let bytes: Vec<u8> = (0..len).map(|_| rng.next_u64() as u8).collect();
        let (plain, read) = surfaces::decode(&bytes);
        let (offset, offset_read) = decode(&bytes);
        assert_eq!(offset_read, read);
        assert_eq!(
            offset,
            surfaces::Case {
                kind: surfaces::OFFSET,
                ..plain
            }
        );
    }
    // The bound is the surfaces target's, reached by the same bytes.
    let mut bytes = vec![0, 0];
    for _ in 0..surfaces::VALUES {
        bytes.push(0x0F);
        bytes.extend([0x3F, 0xF0, 0, 0, 0, 0, 0, 0]);
    }
    bytes.extend([1; 2 * surfaces::PARAMS]);
    bytes.extend([7; 40]);
    assert_eq!(decode(&bytes).1, MAX_INPUT_BYTES);
    assert_eq!(decode(&[]).1, 0);
}

#[test]
fn the_offset_is_reached() {
    // The quarter turn, every double a whole number from the rule bytes (rule 3: k ≥ 1 in the
    // high bits), and parameter pairs spread over the domain; the kind byte is any.
    let mut bytes = vec![0, 0];
    for i in 0..surfaces::VALUES {
        bytes.push((((i * 7) % 9 + 1) as u8) << 4 | 3);
    }
    bytes.extend([0, 0, 85, 170, 170, 85, 255, 255]);
    let case = decode(&bytes).0;
    assert_eq!(case.kind, surfaces::OFFSET);
    let got = reached(&case).unwrap_or_else(|f| panic!("{f}"));
    assert_ne!(got, Reached::Unbuilt);
}

#[test]
fn the_smallest_offset_of_issue_765_reaches_the_pass() {
    // Issue #765's 15-byte surfaces input (base64 `pMikyIQxyITIyDHIyMgR`): an offset of a
    // cylinder. Its contract spends the whole budget; the pass without conversions then holds it
    // to every law it runs, at every pair, and ends.
    let bytes = [
        164, 200, 164, 200, 132, 49, 200, 132, 200, 200, 49, 200, 200, 200, 17,
    ];
    let got = reached(&decode(&bytes).0).unwrap_or_else(|f| panic!("{f}"));
    let Reached::Stopped { pass, .. } = got else {
        panic!("{got:?}");
    };
    assert_eq!(pass, Ok(()));
}

#[test]
fn a_broken_kind_that_stops_is_a_finding() {
    let t = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    let params = [(0, 0), (85, 170), (170, 85), (255, 255)];
    let patch = |moved| {
        let knots = || Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
        let net = [
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
        ];
        Broken {
            inner: Surface::new(knots(), knots(), &net).unwrap(),
            moved,
            dear: true,
        }
    };
    holds(check_with(&patch(false), &params, &t));
    let found = check_with(&patch(true), &params, &t).unwrap_err();
    assert!(
        found
            .law
            .contains("without its conversions, after a budget stop"),
        "{found}"
    );
}
