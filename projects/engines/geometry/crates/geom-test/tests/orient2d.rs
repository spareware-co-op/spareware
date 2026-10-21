//! `geom-num`'s orient2d through `geom-test`'s helpers: a unit test, a property test and a seeded
//! sweep, the three shapes every kernel test takes.
//!
//! The law (28.6, "Algebraic laws"): swapping two points flips the sign of orient2d, and the even
//! permutations (the rotations a, b, c → b, c, a) keep it. `Zero`, exact collinearity, stays `Zero`.
//! orient2d returns `Result<Sign, NumError>`, `Ok` for every grid point (`geom-num` proves its bit
//! budget), so an error on any arrangement fails the law too, and is never read as a sign.
//!
//! - `orient2d_known_cases` is a unit test on chosen inputs, the extremes of the frame included.
//! - `orient2d_swaps_flip_the_sign` is the proptest property, run by `geom_test::prop::run` from a
//!   printed seed, on `geom_test::prop::p2` points.
//! - `orient2d_swaps_flip_the_sign_on_hard_cases` runs the same law on points from
//!   `geom_test::grid::GridSampler` driven by `geom_test::SplitMix64`, which make repeated and
//!   collinear points common, from a printed seed.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_num::Sign;
use geom_num::grid::{COORD_MAX, P2};
use geom_num::predicate::orient2d;
use geom_test::grid::GridSampler;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64, prop};
use proptest::prelude::prop_assert_eq;
use proptest::test_runner::TestCaseError;

/// The largest coordinate magnitude, 2^40 − 1.
const M: i64 = COORD_MAX;

fn p(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

/// orient2d's sign, or what it reported instead, with the arrangement's name.
fn orient(name: &str, a: P2, b: P2, c: P2) -> Result<Sign, String> {
    orient2d(a, b, c).map_err(|e| format!("{name}: orient2d did not answer: {e:?} ({e})"))
}

/// Every swap of two of a, b, c flips the sign and every rotation keeps it, or the name of the
/// first rearrangement that breaks the law or for which orient2d reports an error.
fn swap_law(a: P2, b: P2, c: P2) -> Result<Sign, String> {
    let s = orient("a b c", a, b, c)?;
    let swaps = [("b a c", b, a, c), ("a c b", a, c, b), ("c b a", c, b, a)];
    for (name, x, y, z) in swaps {
        if orient(name, x, y, z)? != s.negate() {
            return Err(format!("swap to {name}: {s:?} did not flip"));
        }
    }
    let rotations = [("b c a", b, c, a), ("c a b", c, a, b)];
    for (name, x, y, z) in rotations {
        if orient(name, x, y, z)? != s {
            return Err(format!("rotation to {name}: {s:?} changed"));
        }
    }
    Ok(s)
}

#[test]
fn orient2d_known_cases() {
    // Counter-clockwise, clockwise and collinear, on small values and at the frame's corners.
    assert_eq!(orient2d(p(0, 0), p(1, 0), p(0, 1)), Ok(Sign::Positive));
    assert_eq!(orient2d(p(0, 0), p(0, 1), p(1, 0)), Ok(Sign::Negative));
    assert_eq!(orient2d(p(0, 0), p(1, 1), p(2, 2)), Ok(Sign::Zero));
    assert_eq!(orient2d(p(-M, -M), p(M, -M), p(M, M)), Ok(Sign::Positive));
    assert_eq!(orient2d(p(-M, -M), p(M, M), p(M, -M)), Ok(Sign::Negative));
    assert_eq!(orient2d(p(-M, -M), p(0, 0), p(M, M)), Ok(Sign::Zero));
    // One step off the long diagonal of the frame: the smallest possible non-zero answer.
    assert_eq!(
        orient2d(p(-M, -M), p(M, M), p(M - 1, M)),
        Ok(Sign::Positive)
    );
    assert_eq!(
        orient2d(p(-M, -M), p(M, M), p(M, M - 1)),
        Ok(Sign::Negative)
    );
    // A repeated point is degenerate.
    assert_eq!(orient2d(p(M, 3), p(M, 3), p(-5, 9)), Ok(Sign::Zero));
    for (a, b, c) in [
        (p(0, 0), p(1, 0), p(0, 1)),
        (p(-M, -M), p(M, M), p(M - 1, M)),
        (p(-M, -M), p(0, 0), p(M, M)),
    ] {
        swap_law(a, b, c).unwrap();
    }
}

#[test]
fn orient2d_swaps_flip_the_sign() -> Result<(), prop::Failure> {
    let points = (prop::p2(), prop::p2(), prop::p2());
    prop::run(
        "orient2d swaps flip the sign",
        Seed::DEFAULT,
        4_096,
        &points,
        |(a, b, c)| {
            let s = orient("a b c", a, b, c).map_err(TestCaseError::fail)?;
            prop_assert_eq!(swap_law(a, b, c), Ok(s));
            Ok(())
        },
    )
}

#[test]
fn orient2d_swaps_flip_the_sign_on_hard_cases() {
    let seed = resolve("orient2d swaps flip the sign, hard cases", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut grid = GridSampler::new();
    let mut zeros = 0;
    for case in 0..20_000 {
        let (a, b, c) = (grid.p2(&mut rng), grid.p2(&mut rng), grid.p2(&mut rng));
        match swap_law(a, b, c) {
            Ok(s) => zeros += usize::from(s.is_zero()),
            Err(e) => panic!("case {case}, seed: {seed}: {e}\n{a:?}\n{b:?}\n{c:?}"),
        }
    }
    // The hard-case sampler must actually reach the degenerate answer often.
    println!("{zeros} of 20000 cases were exactly collinear");
    assert!(zeros > 1_000, "{zeros}");
}
