//! Fuzz target: `geom-num`'s exact predicates never panic, and swapping two points flips their sign.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into grid points with `geom-test`'s
//! [`GridSampler`] (four 2D points, then five 3D points). [`check`] calls every exact predicate on
//! them: `orient2d`, `incircle`, `orient3d` and `insphere`. For each, it checks that the predicate
//! answers, then swaps every pair of points and checks that the sign flips, `Zero` staying `Zero`.
//! [`harness`] is `decode` then `check`, the function every fuzzing engine drives (the crate
//! README sets out the layout).
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to valid grid
//!   points (magnitude at most 2^40 − 1), aimed at the hard cases: extremes of the frame, repeated
//!   and one-step-apart points, collinear points and coarse sub-grids (see `geom_test::grid`).
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes; longer inputs only add
//!   bytes nobody reads. The bound is derived below and a unit test builds the input that reaches
//!   it.
//! - **A panic is a failure.** The harness does not catch panics: a predicate that panics fails
//!   the run, and bolero reports the input and its seed.
//! - **An error is a failure.** Each predicate returns `Result<Sign, NumError>`, and `geom-num`
//!   proves that for grid points it is always `Ok`: its bit budgets are derived for every grid
//!   input and checked when it compiles, so `NumError::BudgetExceeded` can only be a defect in
//!   `geom-num` (`geom_num::predicate`'s module documentation). Every point the harness decodes is
//!   a grid point, so an error, on the first call or after any swap, is a [`Finding`] naming the
//!   predicate, the error and the points. It is never read as a sign, so a defect can never pass
//!   for an exact degeneracy or for a sign that flipped.
//! - **A broken law is a [`Finding`]** naming the predicate, the swap and the points.
//!
//! Method: the algebraic laws of section 28.6 of the kernel architecture
//! (`projects/engines/geometry/architecture/28-testing.md`, "Swapping two points flips the sign").
//! Each predicate is the sign of a determinant with one row per point (the points lifted onto the
//! paraboloid z = |p|² for in-circle and in-sphere), as in Shewchuk, "Adaptive Precision
//! Floating-Point Arithmetic and Fast Robust Geometric Predicates", Discrete & Computational
//! Geometry 18, 1997 (<https://doi.org/10.1007/PL00009321>), which `geom_num::predicate` follows.
//! Exchanging two points exchanges two rows, and by the Leibniz formula an exchange of two rows
//! negates a determinant, so every transposition must flip the sign. The board names orient2d's
//! swap; the same law is checked on the other three predicates, whose points the harness decodes
//! anyway.
//!
//! Cost: 4 + 3 + 6 + 6 + 10 = 29 predicate calls per input, each constant time, and at most
//! [`MAX_INPUT_BYTES`] bytes read.

use core::fmt::Debug;

use geom_num::grid::{P2, P3};
use geom_num::predicate::{incircle, insphere, orient2d, orient3d};
use geom_num::{NumError, Sign};
use geom_test::ByteSource;
use geom_test::grid::GridSampler;

use crate::Finding;

/// The most bytes [`decode`] reads from any input: 270.
///
/// A coordinate reads at most 11 bytes: a rule byte, then, for a coordinate snapped to a sub-grid
/// and moved one step, a sub-grid selector, an eight-byte word and a direction byte. A point reads a
/// rule byte, then either a few bytes that make it from earlier points, or, when those are missing,
/// those bytes and then a fresh point's coordinates: the line rule through two picked points (rule
/// 7) reads two selectors and a factor before it can fall back, the line rule through the two
/// latest points (rule 6) a factor, and the repeat and move rules (4 and 5) one selector. A pick
/// from a memory that holds a point always succeeds, so rule 7 falls back only for the first point
/// of its dimension and rule 6 only for the first two. The four 2D points therefore read at most
/// (1 + 3 + 22) + (1 + 1 + 22) + 23 + 23 = 96 bytes, and the five 3D points at most
/// (1 + 3 + 33) + (1 + 1 + 33) + 34 + 34 + 34 = 174: 270 in all. The unit test
/// `the_input_bound_is_reached_and_never_passed` builds that input and checks it reads exactly 270.
pub const MAX_INPUT_BYTES: usize = 270;

/// The points one input decodes to: four 2D points, then five 3D points, read in that order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Points {
    /// The 2D points, for `orient2d` (the first three) and `incircle` (all four).
    pub p2: [P2; 4],
    /// The 3D points, for `orient3d` (the first four) and `insphere` (all five).
    pub p3: [P3; 5],
}

/// Decodes `data` into the harness's points, with how many of its bytes were read: at most
/// [`MAX_INPUT_BYTES`]. Past the end of `data`, bytes read as zero and are not counted.
#[must_use]
pub fn decode(data: &[u8]) -> (Points, usize) {
    // One source and one sampler for all nine points, read in order (array elements are evaluated
    // left to right), so the 3D points can repeat the 2D points' coordinates.
    let mut src = ByteSource::new(data);
    let mut grid = GridSampler::new();
    let p2 = [
        grid.p2(&mut src),
        grid.p2(&mut src),
        grid.p2(&mut src),
        grid.p2(&mut src),
    ];
    let p3 = [
        grid.p3(&mut src),
        grid.p3(&mut src),
        grid.p3(&mut src),
        grid.p3(&mut src),
        grid.p3(&mut src),
    ];
    let read = data.len().saturating_sub(src.remaining());
    (Points { p2, p3 }, read)
}

/// The harness every fuzzing engine drives: decode `data` into grid points, then [`check`].
///
/// # Errors
///
/// As for [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

/// Calls every exact predicate on `points` and checks every swap law.
///
/// # Errors
///
/// A [`Finding`] for the first predicate call that returns an error, or the first swap that
/// does not flip a sign.
#[allow(
    clippy::many_single_char_names,
    reason = "a to e name the points as the predicates' parameters do"
)]
pub fn check(points: &Points) -> Result<(), Finding> {
    let [a, b, c, d] = points.p2;
    swap_law("orient2d", [a, b, c], |&[a, b, c]| orient2d(a, b, c))?;
    swap_law("incircle", [a, b, c, d], |&[a, b, c, d]| {
        incircle(a, b, c, d)
    })?;
    let [a, b, c, d, e] = points.p3;
    swap_law("orient3d", [a, b, c, d], |&[a, b, c, d]| {
        orient3d(a, b, c, d)
    })?;
    swap_law("insphere", [a, b, c, d, e], |&[a, b, c, d, e]| {
        insphere(a, b, c, d, e)
    })?;
    Ok(())
}

/// Checks that `f` answers on the `N` points, then swaps every pair of them and checks that `f`
/// answers and its sign flips each time.
fn swap_law<const N: usize, P: Copy + Debug>(
    name: &str,
    points: [P; N],
    f: impl Fn(&[P; N]) -> Result<Sign, NumError>,
) -> Result<(), Finding> {
    let s = answer(name, &points, &f)?;
    for i in 0..N {
        // `j` runs over i + 1..N, written without arithmetic: `i..N` with its first value skipped.
        for j in (i..N).skip(1) {
            let mut swapped = points;
            // i < j < N, so both indices are in bounds and `swap` cannot panic.
            swapped.swap(i, j);
            let t = answer(name, &swapped, &f)?;
            if t != s.negate() {
                return Err(Finding {
                    law: format!(
                        "{name} flips its sign when points {i} and {j} are swapped ({name} \
                         is the sign of a determinant with one row per point)"
                    ),
                    detail: format!("points: {points:?}\nbefore the swap: {s:?}\nafter: {t:?}"),
                });
            }
        }
    }
    Ok(())
}

/// `f`'s sign on `points`, or a [`Finding`] if it returns an error: every point the harness gives
/// it is a grid point, for which `geom-num` proves every predicate answers (module header).
fn answer<const N: usize, P: Copy + Debug>(
    name: &str,
    points: &[P; N],
    f: &impl Fn(&[P; N]) -> Result<Sign, NumError>,
) -> Result<Sign, Finding> {
    f(points).map_err(|e| Finding {
        law: format!(
            "{name} answers every grid input (geom-num proves its bit budget for every grid \
             point, so an error is a defect in geom-num)"
        ),
        detail: format!("points: {points:?}\nerror: {e:?} ({e})"),
    })
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;
    use geom_test::seed::fixed;
    use geom_test::{Seed, SplitMix64};

    /// The law check catches a predicate that ignores order, and one that is wrong only on
    /// degenerate input: a check that cannot fail would prove nothing (28.4, fault injection).
    #[test]
    fn the_law_check_catches_broken_predicates() {
        let pts = [
            P2::new(0, 0).unwrap(),
            P2::new(1, 0).unwrap(),
            P2::new(0, 1).unwrap(),
        ];
        let always_positive = swap_law("broken", pts, |_| Ok(Sign::Positive));
        assert!(always_positive.is_err());
        let e = always_positive.unwrap_err();
        assert!(e.law.contains("points 0 and 1"), "{e}");

        let line = [
            P2::new(0, 0).unwrap(),
            P2::new(1, 1).unwrap(),
            P2::new(2, 2).unwrap(),
        ];
        // Zero on the first order, Positive on the others: degenerate input must stay Zero.
        let first = line;
        let zero_once = swap_law("broken", line, |p| {
            if *p == first {
                Ok(Sign::Zero)
            } else {
                Ok(Sign::Positive)
            }
        });
        assert!(zero_once.is_err());

        // The real predicate passes on both.
        swap_law("orient2d", pts, |&[a, b, c]| orient2d(a, b, c)).unwrap();
        swap_law("orient2d", line, |&[a, b, c]| orient2d(a, b, c)).unwrap();
    }

    /// A predicate that reports an error is a finding, whether it fails on the points as given or
    /// only after a swap, and the error is never read as a sign: an error after a swap of a
    /// degenerate input must not pass for the `Zero` that a flip of `Zero` expects.
    #[test]
    fn the_law_check_catches_a_predicate_that_reports_an_error() {
        let pts = [
            P2::new(0, 0).unwrap(),
            P2::new(1, 0).unwrap(),
            P2::new(0, 1).unwrap(),
        ];
        let always = swap_law("failing", pts, |_| Err(NumError::BudgetExceeded));
        let e = always.unwrap_err();
        assert!(e.law.contains("failing answers every grid input"), "{e}");
        assert!(e.law.contains("defect in geom-num"), "{e}");
        assert!(e.detail.contains("BudgetExceeded"), "{e}");

        let line = [
            P2::new(0, 0).unwrap(),
            P2::new(1, 1).unwrap(),
            P2::new(2, 2).unwrap(),
        ];
        let first = line;
        let after_a_swap = swap_law("failing after a swap", line, |p| {
            if *p == first {
                Ok(Sign::Zero)
            } else {
                Err(NumError::BudgetExceeded)
            }
        });
        let e = after_a_swap.unwrap_err();
        assert!(e.law.contains("answers every grid input"), "{e}");
        assert!(e.detail.contains("BudgetExceeded"), "{e}");
    }

    #[test]
    fn empty_and_tiny_inputs_are_cases() {
        harness(&[]).unwrap();
        harness(&[0]).unwrap();
        harness(&[0xff; 3]).unwrap();
        harness(&[0x80; 400]).unwrap();
        assert_eq!(decode(&[]).1, 0);
    }

    /// The input of [`MAX_INPUT_BYTES`]'s derivation: every coordinate snapped and moved (rule 5,
    /// 11 bytes), the first point of each dimension by rule 7 and the second by rule 6, each
    /// falling back to a fresh point, and the rest fresh by rule 0.
    fn worst_case() -> Vec<u8> {
        let coord = [5u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut out = Vec::new();
        for (dimension, count) in [(2, 4), (3, 5)] {
            for i in 0..count {
                match i {
                    0 => out.extend([7, 0, 0, 0]),
                    1 => out.extend([6, 0]),
                    _ => out.push(0),
                }
                for _ in 0..dimension {
                    out.extend(coord);
                }
            }
        }
        out
    }

    #[test]
    fn the_input_bound_is_reached_and_never_passed() {
        let worst = worst_case();
        assert_eq!(worst.len(), MAX_INPUT_BYTES);
        assert_eq!(decode(&worst).1, MAX_INPUT_BYTES);
        let mut longer = worst.clone();
        longer.extend([0xa5; 64]);
        assert_eq!(decode(&longer).1, MAX_INPUT_BYTES);
        assert_eq!(decode(&longer).0, decode(&worst).0);

        // Random and patterned inputs never read more.
        let mut rng = SplitMix64::new(fixed("predicates input bound", Seed(0x270)));
        let mut data = vec![0u8; 2 * MAX_INPUT_BYTES];
        for _ in 0..20_000 {
            rng.fill_bytes(&mut data);
            assert!(decode(&data).1 <= MAX_INPUT_BYTES);
        }
        for byte in 0..=255u8 {
            assert!(
                decode(&[byte; 2 * MAX_INPUT_BYTES]).1 <= MAX_INPUT_BYTES,
                "{byte}"
            );
        }
    }
}
