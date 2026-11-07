//! The surface kinds' fuzz target, run as tests on stable Rust: bolero's test mode, the seeded
//! leg, the input bound, every kind reached, and a broken kind caught.
//!
//! - `surfaces_fuzz` wraps `geom_fuzz::surfaces::harness` in `bolero::check!`. Under
//!   `cargo test` it replays the files in `tests/__fuzz__/surfaces/corpus` and `crashes`, if any,
//!   then runs [`ITERATIONS`] random byte strings; bolero prints a failing one's seed as
//!   `BOLERO_RANDOM_SEED=<seed>`, which replays it. Under
//!   `cargo bolero test surfaces_fuzz --package geom-fuzz` the same test is a libFuzzer target.
//! - `surfaces_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed
//!   seed (`SPAREKIT_SEED` replays it), the same on every target.
//! - `every_kind_is_reached` decodes one input per kind tag and checks each builds and keeps the
//!   contract, so no kind is out of the fuzzer's reach.
//! - `a_broken_kind_is_a_finding`: a kind whose bound misses its own points is caught (28.4,
//!   fault injection: a check that cannot fail proves nothing).

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

mod common;

use common::holds;
use geom_curve::{CurveError, DerivativeBound};
use geom_fuzz::seeded;
use geom_fuzz::surfaces::{
    KINDS, MAX_INPUT_BYTES, PARAMS, VALUES, check, check_with, decode, harness,
};
use geom_math::{Axis, GridTransform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Bounds, Changed, Direction, Limit, Surface};
use geom_run::WorkMeter;
use geom_surface::{ParamBox, ParamSurface, SurfaceForm, SurfaceKind};
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
        holds(check(&case));
    }
}

/// A NURBS patch whose bound over any box is moved a million units from the truth.
#[derive(Debug)]
struct Moved(Surface<3>);

impl ParamSurface for Moved {
    fn domain(&self) -> ParamBox {
        ParamSurface::domain(&self.0)
    }
    fn point(&self, u: f64, v: f64, s: (Limit, Limit)) -> Result<[f64; 3], CurveError> {
        ParamSurface::point(&self.0, u, v, s)
    }
    fn partials(&self, u: f64, v: f64, s: (Limit, Limit)) -> Result<[[f64; 3]; 3], CurveError> {
        self.0.partials(u, v, s)
    }
    fn enclose(
        &self,
        u: f64,
        v: f64,
        s: (Limit, Limit),
        m: &mut WorkMeter<'_>,
    ) -> Result<[[Interval; 3]; 3], CurveError> {
        self.0.enclose(u, v, s, m)
    }
    fn bound(&self, r: ParamBox, m: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        let b = ParamSurface::bound(&self.0, r, m)?;
        Ok(Bounds {
            lo: b.lo.map(|x| x + 1e6),
            hi: b.hi.map(|x| x + 1e6),
        })
    }
    fn derivative_bound(
        &self,
        r: ParamBox,
        d: Direction,
        m: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<3>, CurveError> {
        ParamSurface::derivative_bound(&self.0, r, d, m)
    }
    fn partials_over(
        &self,
        r: ParamBox,
        s: (Limit, Limit),
        k: usize,
        m: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 3]>, CurveError> {
        self.0.partials_over(r, s, k, m)
    }
    fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
        self.0.breaks()
    }
    fn normal_bound(&self, r: ParamBox, m: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        self.0.normal_bound(r, m)
    }
    fn to_nurbs(&self, m: &mut WorkMeter<'_>) -> Result<Changed<SurfaceForm>, CurveError> {
        self.0.to_nurbs(m)
    }
    fn split(
        &self,
        d: Direction,
        t: f64,
        m: &mut WorkMeter<'_>,
    ) -> Result<(Changed<SurfaceKind>, Changed<SurfaceKind>), CurveError> {
        ParamSurface::split(&self.0, d, t, m)
    }
    fn reversed(
        &self,
        d: Direction,
        m: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        self.0.reversed(d, m)
    }
    fn transformed(
        &self,
        t: &TypedTransform,
        m: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        self.0.transformed(t, m)
    }
}

#[test]
fn a_broken_kind_is_a_finding() {
    let knots = || Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let net = [
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 1.0],
    ];
    let s = Surface::new(knots(), knots(), &net).unwrap();
    let t = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    let params = [(0, 0), (85, 170), (170, 85), (255, 255)];
    holds(check_with(&s, &params, &t));
    let found = check_with(&Moved(s), &params, &t).unwrap_err();
    assert!(found.law.contains("kind contract"), "{found}");
}
