//! The surface kinds' fuzz target, run as tests on stable Rust: bolero's test mode, the seeded
//! leg, the input bound, every kind reached, the offsets left to their own target, and a broken
//! kind caught, in the contract and in the pass after a budget stop.
//!
//! - `surfaces_fuzz` wraps `geom_fuzz::surfaces::harness` in `bolero::check!`. Under
//!   `cargo test` it replays the files in `tests/__fuzz__/surfaces/corpus` and `crashes`, if any,
//!   then runs [`ITERATIONS`] random byte strings; bolero prints a failing one's seed as
//!   `BOLERO_RANDOM_SEED=<seed>`, which replays it. Under
//!   `cargo bolero test surfaces_fuzz --package geom-fuzz` the same test is a libFuzzer target.
//! - `surfaces_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed
//!   seed (`SPAREKIT_SEED` replays it), the same on every target.
//! - `every_kind_is_reached` decodes one input per kind tag and checks each keeps the contract,
//!   so no kind is out of the fuzzer's reach; the offset is passed unbuilt, as
//!   `tests/surface_offsets.rs` reaches it.
//! - `no_stored_file_of_this_target_is_an_offset`: no corpus or crash file here decodes to an
//!   offset, which this target would pass without testing.
//! - `a_broken_kind_is_a_finding`: a kind whose bound misses its own points is caught (28.4,
//!   fault injection: a check that cannot fail proves nothing); and
//!   `a_broken_kind_that_stops_is_a_finding`: so is one whose conversions spend the whole budget,
//!   by the pass without conversions after the stop.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their inputs.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    reason = "test inputs, and the trait's own argument names"
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

use std::path::Path;

use broken::Broken;
use common::holds;
use geom_fuzz::seeded;
use geom_fuzz::surfaces::{
    KINDS, MAX_INPUT_BYTES, OFFSET, PARAMS, Reached, VALUES, check_with, decode, harness, reached,
};
use geom_math::{Axis, GridTransform, TypedTransform};
use geom_nurbs::{Basis, Surface};
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the fuzz target in bolero's test mode.
const ITERATIONS: usize = 1_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 1_000;

/// The longest byte string the fuzz target and the seeded leg try. The harness reads at most
/// [`MAX_INPUT_BYTES`] (298) bytes; the rest let some strings carry bytes nobody reads, which
/// must change nothing.
const MAX_LEN: u16 = 320;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn surfaces_fuzz() {
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
fn surfaces_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "surfaces fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// An input for kind `kind`: the quarter turn, every double a whole number from the rule bytes
/// (rule 3: k ≥ 1 in the high bits), and parameter pairs spread over the domain.
fn input(kind: u8) -> Vec<u8> {
    let mut bytes = vec![kind, 0];
    for i in 0..VALUES {
        // Whole numbers 1 to 9, different from one value to the next, so frames and axes are
        // not parallel and every positive length is positive.
        bytes.push((((i * 7) % 9 + 1) as u8) << 4 | 3);
    }
    bytes.extend([0, 0, 85, 170, 170, 85, 255, 255]);
    bytes
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    // Every double read from the input: 2 + 32 · 9 + 2 · 4 bytes.
    let mut bytes = vec![0, 0];
    for _ in 0..VALUES {
        bytes.push(0x0F);
        bytes.extend([0x3F, 0xF0, 0, 0, 0, 0, 0, 0]);
    }
    bytes.extend([1; 2 * PARAMS]);
    bytes.extend([7; 40]);
    let (_, read) = decode(&bytes);
    assert_eq!(read, MAX_INPUT_BYTES);
    assert_eq!(decode(&[]).1, 0);
}

#[test]
fn every_kind_is_reached() {
    for kind in 0..KINDS {
        let case = decode(&input(kind)).0;
        assert_eq!(case.kind, kind);
        let reached = reached(&case).unwrap_or_else(|f| panic!("{f}"));
        // The offset is passed unbuilt: `surface_offsets` fuzzes it.
        if kind == OFFSET {
            assert_eq!(reached, Reached::Unbuilt);
        }
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the corpus folders from the host, which WASI cannot without a preopened folder"
)]
#[test]
fn no_stored_file_of_this_target_is_an_offset() {
    // This target passes an offset without building it, so an offset stored here would test
    // nothing: offsets' files belong to `tests/__fuzz__/surface_offsets`, which reads the same
    // bytes. The folder holds no file today; the test reads whatever it holds.
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/__fuzz__/surfaces");
    for kind in ["corpus", "crashes"] {
        let Ok(listing) = std::fs::read_dir(folder.join(kind)) else {
            continue;
        };
        for f in listing {
            let path = f.unwrap().path();
            let case = decode(&std::fs::read(&path).unwrap()).0;
            assert_ne!(case.kind, OFFSET, "{} is an offset", path.display());
        }
    }
}

/// A bilinear patch, broken as asked.
fn patch(moved: bool, dear: bool) -> Broken<Surface<3>> {
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
        dear,
    }
}

#[test]
fn a_broken_kind_is_a_finding() {
    let t = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    let params = [(0, 0), (85, 170), (170, 85), (255, 255)];
    holds(check_with(&patch(false, false), &params, &t));
    let found = check_with(&patch(true, false), &params, &t).unwrap_err();
    assert!(found.law.contains("kind contract"), "{found}");
}

#[test]
fn a_broken_kind_that_stops_is_a_finding() {
    // Conversions that spend the whole budget stop the contract in the transform law, before any
    // law at a pair. The stop is legal; the pass without conversions then runs those laws under
    // its own meter, and finds the bound that misses the patch.
    let t = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    let params = [(0, 0), (85, 170), (170, 85), (255, 255)];
    holds(check_with(&patch(false, true), &params, &t));
    let found = check_with(&patch(true, true), &params, &t).unwrap_err();
    assert!(
        found
            .law
            .contains("without its conversions, after a budget stop"),
        "{found}"
    );
}
