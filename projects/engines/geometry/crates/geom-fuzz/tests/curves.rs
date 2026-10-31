//! The curve kinds' fuzz target, run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, and a broken kind caught.
//!
//! - `curves_fuzz` wraps `geom_fuzz::curves::harness` in `bolero::check!`. Under `cargo test` it
//!   replays the files in `tests/__fuzz__/curves/corpus` and `crashes`, if any, then runs
//!   [`ITERATIONS`] random byte strings; bolero prints a failing one's seed as
//!   `BOLERO_RANDOM_SEED=<seed>`, which replays it. Under
//!   `cargo bolero test curves_fuzz --package geom-fuzz` the same test is a libFuzzer target.
//! - `curves_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), so every pull request also runs a fixed set of cases, the same
//!   on every target.
//! - `every_kind_is_reached` decodes one input per kind tag and checks each builds and keeps the
//!   contract, so no kind is out of the fuzzer's reach.
//! - `a_broken_kind_is_a_finding`: a kind whose bound misses its own points is caught (28.4,
//!   fault injection: a check that cannot fail proves nothing).

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

mod common;

use common::holds;
use geom_curve::{CurveError, CurveKind, Line, ParamCurve};
use geom_fuzz::curves::{KINDS, MAX_INPUT_BYTES, PARAMS, check, check_with, decode, harness};
use geom_fuzz::seeded;
use geom_math::{GridTransform, TypedTransform};
use geom_nurbs::{Bounds, Limit};
use geom_run::WorkMeter;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the fuzz target in bolero's test mode.
const ITERATIONS: usize = 2_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 2_000;

/// The longest byte string the fuzz target and the seeded leg try. The harness reads at most
/// [`MAX_INPUT_BYTES`] (223) bytes; the rest let some strings carry bytes nobody reads, which
/// must change nothing.
const MAX_LEN: u16 = 256;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn curves_fuzz() {
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
fn curves_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "curves fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// An input for kind `kind`: in space or not, the quarter turn, every double a whole number
/// from the rule bytes (rule 3: k ≥ 1 in the high bits), and parameters spread over the domain.
fn input(kind: u8, space: bool) -> Vec<u8> {
    let mut bytes = vec![kind, u8::from(space), 0];
    for i in 0..geom_fuzz::curves::VALUES {
        // Whole numbers 1 to 9, different from one value to the next, so frames and axes are
        // not parallel and every positive length is positive.
        bytes.push((((i * 7) % 9 + 1) as u8) << 4 | 3);
    }
    bytes.extend([0, 85, 170, 255]);
    bytes
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    // Every double read from the input: 3 + 24 · 9 + 4 bytes.
    let mut bytes = vec![0, 0, 0];
    for _ in 0..geom_fuzz::curves::VALUES {
        bytes.push(0x0F);
        bytes.extend([0x3F, 0xF0, 0, 0, 0, 0, 0, 0]);
    }
    bytes.extend([1; PARAMS]);
    bytes.extend([7; 40]);
    let (_, read) = decode(&bytes);
    assert_eq!(read, MAX_INPUT_BYTES);
    assert_eq!(decode(&[]).1, 0);
}

#[test]
fn every_kind_is_reached() {
    for kind in 0..KINDS {
        for space in [false, true] {
            let case = decode(&input(kind, space)).0;
            assert_eq!(case.kind, kind);
            holds(check(&case));
        }
    }
}

/// A line whose bound over a span misses the line: moved by one.
#[derive(Debug)]
struct Moved(Line<2>);

impl ParamCurve<2> for Moved {
    fn domain(&self) -> (f64, f64) {
        self.0.domain()
    }
    fn point(&self, t: f64, side: Limit) -> Result<[f64; 2], CurveError> {
        self.0.point(t, side)
    }
    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; 2]]) -> Result<(), CurveError> {
        self.0.derivatives(t, side, out)
    }
    fn enclose(
        &self,
        t: f64,
        side: Limit,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[geom_num::interval::Interval; 2]>, CurveError> {
        self.0.enclose(t, side, order, meter)
    }
    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<2>, CurveError> {
        let b = self.0.bound(span, meter)?;
        Ok(Bounds {
            lo: b.lo.map(|x| x + 1.0),
            hi: b.hi.map(|x| x + 1.0),
        })
    }
    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<geom_curve::DerivativeBound<2>, CurveError> {
        self.0.derivative_bound(span, order, meter)
    }
    fn to_nurbs(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<geom_nurbs::Changed<geom_curve::map::NurbsForm<2>>, CurveError> {
        self.0.to_nurbs(meter)
    }
    fn split(
        &self,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<
        (
            geom_nurbs::Changed<CurveKind<2>>,
            geom_nurbs::Changed<CurveKind<2>>,
        ),
        CurveError,
    > {
        self.0.split(t, meter)
    }
    fn reversed(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<geom_nurbs::Changed<CurveKind<2>>, CurveError> {
        self.0.reversed(meter)
    }
    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<geom_nurbs::Changed<CurveKind<2>>, CurveError> {
        self.0.transformed(transform, meter)
    }
}

#[test]
fn a_broken_kind_is_a_finding() {
    let line = Line::new([0.0, 0.0], [1.0, 2.0], (0.0, 1.0)).unwrap();
    let t = TypedTransform::Exact(GridTransform::quarter_turns(geom_math::Axis::Z, 1));
    holds(check_with(&line, &[0, 85, 170, 255], &t));
    let found = check_with(&Moved(line), &[0, 85, 170, 255], &t).unwrap_err();
    assert!(found.law.contains("kind contract"), "{found}");
}
