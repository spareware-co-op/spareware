//! Fuzz target and bounded proof: `geom-num`'s orient2d agrees with the exact determinant of its
//! definition, computed here from scratch.
//!
//! What it does: [`check`] takes three grid points a, b, c and computes [`reference`], the
//! determinant of the 3 × 3 matrix whose rows are (a.x, a.y, 1), (b.x, b.y, 1) and (c.x, c.y, 1),
//! expanded along its third column in `i128`: (b.x·c.y − b.y·c.x) − (a.x·c.y − a.y·c.x) +
//! (a.x·b.y − a.y·b.x). That determinant is orient2d's definition (Shewchuk 1997, section 4.1);
//! this module computes it with no code shared with `geom-num`, and by another formula than
//! `geom-num`'s (b − a) × (c − a), so the two agree only if both are right. Against it, `check`
//! holds `geom-num` to three claims:
//!
//! 1. the predicate [`geom_num::predicate::orient2d`] answers, with the reference's sign;
//! 2. the generated exact path, [`geom_num::generated::orient2d::value`], returns the reference's
//!    value, every bit of it;
//! 3. the generated `f64` filter, [`geom_num::generated::orient2d::filter`], either declines or
//!    gives the reference's sign. Its module calls that claim checked, not yet proven (chapter
//!    28.6), and the predicate runs the filter first only on some targets (`geom_num::predicate`,
//!    routing), so the filter is checked by itself, on every target.
//!
//! [`decode`] turns a fuzzer's bytes into three points with `geom-test`'s [`GridSampler`], aimed at
//! the hard cases: repeated points, collinear points, points one step off a line, and the edges of
//! the frame. [`harness`] is `decode` then `check`: the one function every fuzzing engine drives
//! (bolero's test mode, cargo-bolero's libFuzzer, `ClusterFuzzLite` and the seeded leg; the crate
//! README sets out the layout). [`proof_case`] maps a draw onto the bounded domain of the Kani
//! proof, which drives `check` itself.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to three valid
//!   grid points, reading at most [`MAX_INPUT_BYTES`] = 73 bytes; the bound is derived on the
//!   constant and a unit test builds the input that reaches it.
//! - **The oracle cannot overflow on the grid.** Each coordinate has magnitude at most
//!   M = 2^40 − 1, so each of the six products is at most M² < 2^80, each 2 × 2 minor at most
//!   2M² < 2^81, and the determinant at most 6M² < 2^83, far inside `i128`. Every step is a
//!   checked method all the same, and a step that failed would be reported as a [`Finding`]
//!   naming this harness as the defect, never a panic and never a wrapped value.
//! - **A wrong answer is a [`Finding`]**, naming the claim, the points, the reference value and
//!   what `geom-num` gave. An error from the predicate, or a `None` from the exact path, is a
//!   finding too, never read as a sign: `geom-num` proves every grid input answers.
//! - **A panic is a failure.** Nothing is caught: a panic in `geom-num` fails the run, and the
//!   engine reports the input.
//! - **What is proven and what is tested.** For every case [`proof_case`] can give with bound
//!   [`PROOF_BOUND`] (every coordinate within `PROOF_BOUND` steps of 0 or of an edge ±(2^40 − 1)
//!   of the frame, in any combination), Kani proves that all three claims hold and that nothing
//!   on the way panics or overflows (`tests/orient2d.rs`, `orient2d_proof`; Kani checks every
//!   arithmetic step, index and unwrap by default). Near the edges the filter's products round,
//!   so the proof covers the filter's error bound, not only exact arithmetic. Everywhere else in
//!   the frame the claims are tested, by fuzzing and the property tests, not proven.
//!
//! Method: an exact oracle for a predicate, the first of chapter 28.6's checks
//! (`projects/engines/geometry/architecture/28-testing.md`), with bounded Kani proofs of sign
//! agreement with an exact integer reference, as evidence report 08 recommends for predicates
//! (`projects/kit/notes/G01-evidence/08-standards-coverage-testing.md`, "Recommended test
//! architecture", item 1). The reference is the 3 × 3 determinant of the definition, expanded by
//! cofactors along the column of ones, in Shewchuk's paper:
//!
//! - J. R. Shewchuk, "Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric
//!   Predicates", *Discrete & Computational Geometry* 18 (1997), 305–363,
//!   <https://doi.org/10.1007/PL00009321>.
//!
//! Cost: per case, 6 checked `i128` products and 5 checked sums for the reference, one call each
//! to the predicate, the exact path and the filter (each constant time), and at most
//! [`MAX_INPUT_BYTES`] bytes read by [`decode`]. No allocation unless a finding is reported.

use geom_num::generated::orient2d::{filter, value};
use geom_num::grid::{COORD_MAX, P2};
use geom_num::predicate::orient2d;
use geom_num::{NumError, Sign};
use geom_test::ByteSource;
use geom_test::grid::GridSampler;

use crate::Finding;

/// The most bytes [`decode`] reads from any input: 73.
///
/// The derivation is that of `crate::predicates::MAX_INPUT_BYTES`, for three 2D points instead of
/// four: a coordinate reads at most 11 bytes, so a fresh point reads its rule byte and 22. The
/// first point reads most by rule 7 (two selectors and a factor), which falls back to a fresh
/// point because no point is remembered yet: 1 + 3 + 22 = 26. The second reads most by rule 6 (a
/// factor), which falls back because only one point is remembered: 1 + 1 + 22 = 24. From the
/// third on every rule that picks earlier points succeeds, so the most is a fresh point:
/// 1 + 22 = 23. In all 26 + 24 + 23 = 73. The unit test
/// `the_input_bound_is_reached_and_never_passed` builds that input and checks it reads exactly 73.
pub const MAX_INPUT_BYTES: usize = 73;

/// The Kani proof's bound: each coordinate of a proven case lies within this many grid steps of 0
/// or of an edge of the frame (see [`proof_case`]).
pub const PROOF_BOUND: u64 = 3;

/// One case: the three points orient2d is asked about, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Case {
    /// The first point.
    pub a: P2,
    /// The second point.
    pub b: P2,
    /// The third point.
    pub c: P2,
}

/// The exact determinant of orient2d's definition, from scratch: the 3 × 3 determinant with rows
/// (a.x, a.y, 1), (b.x, b.y, 1), (c.x, c.y, 1), expanded along the column of ones. `None` only if a
/// checked step failed, which the bound in the module header rules out for grid points.
#[must_use]
pub fn reference(a: P2, b: P2, c: P2) -> Option<i128> {
    let [ax, ay, bx, by, cx, cy] = [a.x, a.y, b.x, b.y, c.x, c.y].map(|v| i128::from(v.get()));
    let minor = |px: i128, py: i128, qx: i128, qy: i128| {
        px.checked_mul(qy)?.checked_sub(py.checked_mul(qx)?)
    };
    let bc = minor(bx, by, cx, cy)?;
    let ac = minor(ax, ay, cx, cy)?;
    let ab = minor(ax, ay, bx, by)?;
    bc.checked_sub(ac)?.checked_add(ab)
}

/// Checks the three claims of the module header against [`reference`], on `geom-num`'s own
/// functions.
///
/// # Errors
///
/// A [`Finding`] for the first claim that fails, or if the reference itself could not be
/// computed (a defect in this harness).
pub fn check(case: &Case) -> Result<(), Finding> {
    check_with(case, orient2d, filter, value)
}

/// [`check`] on the given predicate, filter and exact path, so a test can hand it broken ones and
/// see that it catches them (28.4, fault injection: a check that cannot fail proves nothing).
///
/// # Errors
///
/// As for [`check`].
pub fn check_with(
    case: &Case,
    predicate: impl Fn(P2, P2, P2) -> Result<Sign, NumError>,
    filter: impl Fn(P2, P2, P2) -> Option<Sign>,
    exact_value: impl Fn(P2, P2, P2) -> Option<i128>,
) -> Result<(), Finding> {
    let Case { a, b, c } = *case;
    let finding = |law: &str, detail: String| Finding {
        law: law.to_owned(),
        detail: format!("points: {a:?}, {b:?}, {c:?}\n{detail}"),
    };
    let Some(det) = reference(a, b, c) else {
        return Err(finding(
            "the reference determinant fits i128 for grid points (|det| ≤ 6M² < 2^83), so a \
             failed step is a defect in this harness",
            String::new(),
        ));
    };
    let sign = Sign::of_i128(det);
    let got = exact_value(a, b, c);
    if got != Some(det) {
        return Err(finding(
            "orient2d's exact path returns the determinant of its definition, every bit of it",
            format!("reference: {det}\nexact path: {got:?}"),
        ));
    }
    match predicate(a, b, c) {
        Ok(s) if s == sign => {}
        other => {
            return Err(finding(
                "orient2d answers every grid input with the sign of the determinant of its \
                 definition (an error is a defect in geom-num, which proves every grid input \
                 answers)",
                format!("reference: {det} ({sign:?})\norient2d: {other:?}"),
            ));
        }
    }
    match filter(a, b, c) {
        None => Ok(()),
        Some(s) if s == sign => Ok(()),
        Some(s) => Err(finding(
            "orient2d's f64 filter declines or gives the sign of the exact determinant: its \
             error bound is never exceeded",
            format!("reference: {det} ({sign:?})\nfilter: {s:?}"),
        )),
    }
}

/// Decodes `data` into a case, with how many of its bytes were read: at most
/// [`MAX_INPUT_BYTES`]. Past the end of `data`, bytes read as zero and are not counted.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    // One source and one sampler for the three points, read in order (struct fields are evaluated
    // in the order written), so later points can repeat or line up with earlier ones.
    let mut src = ByteSource::new(data);
    let mut grid = GridSampler::new();
    let case = Case {
        a: grid.p2(&mut src),
        b: grid.p2(&mut src),
        c: grid.p2(&mut src),
    };
    let read = data.len().saturating_sub(src.remaining());
    (case, read)
}

/// The harness every fuzzing engine drives: decode `data` into three grid points, then [`check`].
///
/// # Errors
///
/// As for [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

/// One draw of the proof's domain, for the six coordinates a.x, a.y, b.x, b.y, c.x, c.y in that
/// order: whether each lies near an edge of the frame, whether it is negative, and how many steps
/// it lies from 0 or from that edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProofDraw {
    /// Near ±(2^40 − 1) rather than near 0.
    pub near_edge: [bool; 6],
    /// Negative rather than positive.
    pub negative: [bool; 6],
    /// Steps from 0 or from the edge.
    pub offset: [u64; 6],
}

/// The coordinate a draw names: `offset` steps from 0, or `offset` steps inside the edge
/// 2^40 − 1, then negated if `negative`. `None` when `offset` exceeds `bound` or `bound` exceeds
/// 2^40 − 1, so every coordinate returned is on the grid.
#[must_use]
pub fn proof_coord(bound: u64, near_edge: bool, negative: bool, offset: u64) -> Option<i64> {
    let max = COORD_MAX.unsigned_abs();
    if bound > max || offset > bound {
        return None;
    }
    let magnitude = if near_edge {
        max.checked_sub(offset)?
    } else {
        offset
    };
    let v = i64::try_from(magnitude).ok()?;
    if negative { v.checked_neg() } else { Some(v) }
}

/// The case a draw names, every coordinate by [`proof_coord`]; `None` when any coordinate is
/// `None`.
#[must_use]
pub fn proof_case(bound: u64, draw: &ProofDraw) -> Option<Case> {
    let mut v = [0i64; 6];
    for (((out, &edge), &neg), &off) in v
        .iter_mut()
        .zip(&draw.near_edge)
        .zip(&draw.negative)
        .zip(&draw.offset)
    {
        *out = proof_coord(bound, edge, neg, off)?;
    }
    let [ax, ay, bx, by, cx, cy] = v;
    Some(Case {
        a: P2::new(ax, ay).ok()?,
        b: P2::new(bx, by).ok()?,
        c: P2::new(cx, cy).ok()?,
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

    fn p(x: i64, y: i64) -> P2 {
        P2::new(x, y).unwrap()
    }

    const M: i64 = COORD_MAX;

    #[test]
    fn the_reference_is_the_definition_on_known_cases() {
        // Counter-clockwise unit triangle: twice its area, +1.
        assert_eq!(reference(p(0, 0), p(1, 0), p(0, 1)), Some(1));
        assert_eq!(reference(p(0, 0), p(0, 1), p(1, 0)), Some(-1));
        // Collinear and coincident points.
        assert_eq!(reference(p(0, 0), p(1, 1), p(2, 2)), Some(0));
        assert_eq!(reference(p(5, 7), p(5, 7), p(-3, 9)), Some(0));
        // Half the bounding square, the grid's largest value 4M² (geom_num::predicate::orient2d).
        assert_eq!(
            reference(p(-M, -M), p(M, -M), p(M, M)),
            Some(4 * i128::from(M).pow(2))
        );
    }

    #[test]
    fn the_check_passes_geom_num_on_known_cases() {
        let cases = [
            (p(0, 0), p(1, 0), p(0, 1)),
            (p(0, 0), p(1, 1), p(2, 2)),
            (p(-M, -M), p(M, -M), p(M, M)),
            (p(M, M), p(-M, -M), p(M - 1, M)),
            (p(M, -M), p(M, -M), p(M, -M)),
        ];
        for (a, b, c) in cases {
            check(&Case { a, b, c }).unwrap();
        }
    }

    /// The check catches each claim broken on its own: a predicate with the wrong sign, a
    /// predicate that reports an error, an exact path one off, and a filter that is sure of the
    /// wrong sign. Each would otherwise pass, so a check that cannot fail would show here.
    #[test]
    fn the_check_catches_each_broken_claim() {
        let case = Case {
            a: p(0, 0),
            b: p(1, 0),
            c: p(0, 1),
        };
        let flipped = check_with(
            &case,
            |a, b, c| orient2d(a, b, c).map(Sign::negate),
            filter,
            value,
        );
        assert!(flipped.unwrap_err().law.contains("orient2d answers"));

        let failing = check_with(
            &case,
            |_, _, _| Err(NumError::BudgetExceeded),
            filter,
            value,
        );
        let e = failing.unwrap_err();
        assert!(e.detail.contains("BudgetExceeded"), "{e}");

        let off = check_with(&case, orient2d, filter, |a, b, c| {
            value(a, b, c).map(|v| v + 1)
        });
        assert!(off.unwrap_err().law.contains("exact path"));

        let missing = check_with(&case, orient2d, filter, |_, _, _| None);
        assert!(missing.unwrap_err().law.contains("exact path"));

        let sure = check_with(&case, orient2d, |_, _, _| Some(Sign::Negative), value);
        assert!(sure.unwrap_err().law.contains("filter"));

        // A filter that declines everything is sound, and passes.
        check_with(&case, orient2d, |_, _, _| None, value).unwrap();

        // A wrong Zero on collinear-looking input is caught too.
        let zero = check_with(&case, |_, _, _| Ok(Sign::Zero), filter, value);
        assert!(zero.is_err());
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
    /// 11 bytes), the first point by rule 7 and the second by rule 6, each falling back to a fresh
    /// point, and the third fresh by rule 0.
    fn worst_case() -> Vec<u8> {
        let coord = [5u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut out = Vec::new();
        for i in 0..3 {
            match i {
                0 => out.extend([7, 0, 0, 0]),
                1 => out.extend([6, 0]),
                _ => out.push(0),
            }
            for _ in 0..2 {
                out.extend(coord);
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

        let mut rng = SplitMix64::new(fixed("orient2d input bound", Seed(0x73)));
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

    #[test]
    fn proof_coordinates_cover_both_ends_of_the_frame_and_nothing_else() {
        let b = PROOF_BOUND;
        assert_eq!(proof_coord(b, false, false, 0), Some(0));
        assert_eq!(proof_coord(b, false, true, 0), Some(0));
        assert_eq!(proof_coord(b, false, true, b), Some(-(b as i64)));
        assert_eq!(proof_coord(b, true, false, 0), Some(M));
        assert_eq!(proof_coord(b, true, true, 0), Some(-M));
        assert_eq!(proof_coord(b, true, false, b), Some(M - b as i64));
        assert_eq!(proof_coord(b, true, false, b + 1), None);
        assert_eq!(proof_coord(b, false, false, b + 1), None);
        // The widest bound is the whole frame; anything wider is refused.
        let max = M as u64;
        assert_eq!(proof_coord(max, true, false, max), Some(0));
        assert_eq!(proof_coord(max, false, true, max), Some(-M));
        assert_eq!(proof_coord(max + 1, false, false, 0), None);
        assert_eq!(proof_coord(u64::MAX, true, true, u64::MAX), None);
    }

    #[test]
    fn every_proof_case_is_on_the_grid_and_passes() {
        // Every coordinate pattern of the proof's domain at offsets 0 and the bound: 4^6 sign and
        // edge patterns times 2^6 offset patterns, each checked.
        let b = PROOF_BOUND;
        for pattern in 0u32..(1 << 18) {
            let bit = |i: u32| pattern >> i & 1 == 1;
            let draw = ProofDraw {
                near_edge: core::array::from_fn(|i| bit(i as u32)),
                negative: core::array::from_fn(|i| bit(6 + i as u32)),
                offset: core::array::from_fn(|i| if bit(12 + i as u32) { b } else { 0 }),
            };
            let case = proof_case(b, &draw).unwrap();
            check(&case).unwrap();
        }
        let mut too_far = ProofDraw {
            near_edge: [false; 6],
            negative: [false; 6],
            offset: [0; 6],
        };
        too_far.offset[5] = b + 1;
        assert_eq!(proof_case(b, &too_far), None);
    }
}
