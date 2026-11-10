//! Audit findings in the certified root-isolation engine, `geom_num::isolate` (independent audit
//! of the merged core, branch `audit/math`).
//!
//! Every test here FAILS on the code as merged: each one shows a finding, with the input, what the
//! engine returns and what it should return. Nothing in `src/` is changed; the fix is the owner's
//! to decide. The zeros of every system here are known in closed form (integers).
//!
//! Neither finding loses a zero: the audit found no input where a zero lies outside every
//! returned enclosure and flagged region. Both are places where the engine does less than its
//! header says.
//!
//! 1. Simple, isolated, interior zeros of a smooth system are returned as flagged regions, not
//!    proven, when one unknown converges much faster than another.
//! 2. A system that is defined and smooth near its zeros, and defined on the whole domain, is
//!    refused with `Num(NegativeSqrt)` when it takes the root of a sum of squares over a box
//!    holding the origin.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests: native arithmetic, casts and indexing on small numbers, and a failure stops the test"
)]

use geom_num::NumError;
use geom_num::interval::Interval;
use geom_num::isolate::{Isolation, Options, System, isolate};
use geom_num::newton::{Operator, Residuals};
use geom_num::real::Real;
use geom_num::scalar::Scalar;
use geom_run::{Budget, WorkMeter};

fn r(x: f64) -> Real {
    Real::new(x).unwrap()
}

fn iv(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

fn c<S: Scalar>(v: f64) -> S {
    S::constant(r(v))
}

/// sin(πx) = 0, sin(πy) = 0: one simple zero at every point with whole coordinates, each a
/// distance of 1 from the next, the Jacobian ±π on its diagonal there. The unknowns are in units
/// of 10^6 grid steps, so the zeros are a million steps apart.
struct Lattice;

impl Residuals<2> for Lattice {
    fn residuals<S: Scalar>(&self, x: [S; 2]) -> Result<[S; 2], NumError> {
        Ok([x[0].sinpi()?, x[1].sinpi()?])
    }
}

impl System<2, 2> for Lattice {
    fn position<S: Scalar>(&self, x: [S; 2]) -> Result<[S; 2], NumError> {
        Ok([x[0].mul(c(1e6))?, x[1].mul(c(1e6))?])
    }
}

/// √(x² + y²) = 5, y = 3: the circle of radius 5 about the origin and a line, meeting at (4, 3)
/// and (−4, 3). The residuals are defined at every point of the plane; the first is not
/// differentiable at the origin only, where it is −5.
struct CircleAndLine;

impl Residuals<2> for CircleAndLine {
    fn residuals<S: Scalar>(&self, x: [S; 2]) -> Result<[S; 2], NumError> {
        Ok([
            x[0].square()?.add(x[1].square()?)?.sqrt()?.sub(c(5.0))?,
            x[1].sub(c(3.0))?,
        ])
    }
}

impl System<2, 2> for CircleAndLine {
    fn position<S: Scalar>(&self, x: [S; 2]) -> Result<[S; 2], NumError> {
        Ok([x[0].mul(c(64.0))?, x[1].mul(c(64.0))?])
    }
}

fn holds(b: &[Interval; 2], p: [f64; 2]) -> bool {
    b.iter().zip(p).all(|(i, v)| i.contains(v))
}

/// No zero of `zeros` lies outside every enclosure and flagged region (the engine's first
/// guarantee, which holds in every test here).
fn none_lost(found: &Isolation<2>, zeros: &[[f64; 2]]) {
    for &p in zeros {
        let kept = found.zeros.iter().any(|z| holds(&z.enclosure, p))
            || found.flagged.iter().any(|f| holds(&f.region, p));
        assert!(
            kept,
            "the zero {p:?} is in no enclosure and no flagged region"
        );
    }
}

/// Finding 1. Input: the lattice system on [−3.5, 3.5]², Krawczyk's operator, target one step.
/// The domain holds 49 zeros, none on its boundary, none closer than a million steps to another,
/// none multiple. Right answer: 49 proven zeros and no flagged region; the header's "Where it can
/// stop short" names zeros on the domain's boundary, zeros closer than the floor, multiple zeros,
/// zeros without a derivative and fixed unknowns, and these are none of those. Returned: 34
/// zeros and 26 flagged regions about 10^-15 wide, holding the other 15 zeros, among them (−3, 1),
/// (−2, 2) and (3, 3). Hansen and Sengupta's operator returns 34 and 30.
///
/// Where it comes from: a box cut against an earlier zero's proof box is long and thin (the
/// strip beside the proof box); the next test runs the engine on one such strip alone. Each zero
/// left unproven comes back as two regions a few doubles wide in x that share the face x = the
/// zero exactly, so a split went through the zero. The audit's reading of the code, not traced
/// step by step: while the box is long in y its contraction bound is at or above one, so a step
/// only narrows it and splits it; the narrowing in x is at full strength all the while, x closes
/// on the zero in a few steps, symmetric about it, and the midpoint split then cuts through it.
/// From then on the zero is on an edge of boxes a few doubles wide in x, where the rounding of a
/// step is as wide as the box, and neither the operator nor the inflation step (half a width on
/// each side) can put an image strictly inside; when y has shrunk too the box is below the floor
/// and is flagged. The same system on [−1.5, 2.5]², or in one unknown, proves every zero.
#[test]
fn simple_interior_zeros_are_proven_not_flagged() {
    let mut zeros = Vec::new();
    for x in -3..=3 {
        for y in -3..=3 {
            zeros.push([f64::from(x), f64::from(y)]);
        }
    }
    for operator in [Operator::Krawczyk, Operator::HansenSengupta] {
        let mut meter = WorkMeter::new(Budget::of_work(50_000_000));
        let options = Options {
            operator,
            target: Real::ONE,
        };
        let domain = [iv(-3.5, 3.5), iv(-3.5, 3.5)];
        let found = isolate(&Lattice, domain, options, &mut meter).unwrap();
        none_lost(&found, &zeros);
        assert_eq!(
            (found.zeros.len(), found.flagged.len()),
            (49, 0),
            "{operator:?}: proven zeros and flagged regions"
        );
    }
}

/// Finding 1, on one strip. Input: the lattice system on x in [−3.0243186260217074,
/// −2.960537814773123], y in [0.26, 3.5], the part of the upper half of the domain above that is
/// left beside the proof box of the zero (−3, 0), which holds the three zeros (−3, 1), (−3, 2)
/// and (−3, 3), each well inside it; Krawczyk's operator. Right answer: three proven zeros.
/// Returned: two zeros, and (−3, 1) in two flagged regions, x in [−3.0000000000000013, −3] and
/// [−3, −2.9999999999999987], y in [0.9999999999999997, 1.0000000000000007]. (Hansen and
/// Sengupta's operator proves all three here.)
#[test]
fn zeros_in_a_thin_strip_are_proven_not_flagged() {
    let zeros = [[-3.0, 1.0], [-3.0, 2.0], [-3.0, 3.0]];
    let mut meter = WorkMeter::new(Budget::of_work(50_000_000));
    let domain = [
        iv(-3.024_318_626_021_707_4, -2.960_537_814_773_123),
        iv(0.26, 3.5),
    ];
    let found = isolate(&Lattice, domain, Options::DEFAULT, &mut meter).unwrap();
    none_lost(&found, &zeros);
    assert_eq!(
        (found.zeros.len(), found.flagged.len()),
        (3, 0),
        "proven zeros and flagged regions: {:?}",
        found.flagged
    );
}

/// Finding 2. Input: the circle and the line on [−8, 8]², Krawczyk's operator, target one step.
/// Right answer: the zeros (4, 3) and (−4, 3), proven. Returned: `Err(Num(NegativeSqrt))`.
///
/// Why: over any box whose x and y both hold zero, x² and y² have the exact lower bound 0, and
/// their interval sum is widened one double below it (`Interval::add` widens every bound, exact
/// or not), so `Interval::sqrt` refuses the sum as possibly negative. `Interval::norm` knows a
/// sum of squares is never negative, but a residual written over `Scalar` cannot reach it, and
/// the engine treats a residual that fails in doubles as a system not defined on its domain.
/// This one is: its value at the origin is −5.
#[test]
fn a_distance_residual_is_defined_over_a_box_holding_the_origin() {
    let mut meter = WorkMeter::new(Budget::of_work(50_000_000));
    let domain = [iv(-8.0, 8.0), iv(-8.0, 8.0)];
    let found = isolate(&CircleAndLine, domain, Options::DEFAULT, &mut meter)
        .expect("the circle of radius 5 and the line y = 3 on [-8, 8] x [-8, 8]");
    let zeros = [[4.0, 3.0], [-4.0, 3.0]];
    none_lost(&found, &zeros);
    assert_eq!(found.zeros.len(), 2);
}
