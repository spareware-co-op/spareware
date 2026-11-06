//! Attacks on piece G60's claims for curves: the kind contract's margins (each law fires just past
//! what it allows and not just inside it), its refusal probes one at a time, the NURBS kind at
//! knots approached from either side and on spans that touch them, the transform's error bound
//! where doubles are not exact, and every error's text.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "tests: native arithmetic and indexing, and a failure stops the test"
)]

use geom_curve::affine::{Entries, int_interval};
use geom_curve::contract::check;
use geom_curve::tight::larger;
use geom_curve::{
    ContractError, CurveError, CurveKind, DerivativeBound, Law, NurbsForm, ParamCurve, ParamMap,
};
use geom_math::{
    Axis, GridTransform, MathError, RationalRotation, RationalTransform, Rotation, TypedTransform,
};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Bounds, Changed, Curve, Limit, NurbsError};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The contract's relative slack between doubles and an enclosure, 2^−30.
const SLACK: f64 = f64::from_bits(0x3E10_0000_0000_0000);

/// The quarter turn about z, exact.
fn quarter() -> TypedTransform {
    TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1))
}

/// A cubic on [0, 1] near (100, 100, 100), where the slack's scale is about 300.
fn cubic() -> Curve<3> {
    Curve::new(
        Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap(),
        &[
            [100.0, 100.0, 100.0],
            [101.0, 103.0, 101.0],
            [103.0, 98.0, 102.0],
            [104.0, 101.0, 100.0],
        ],
    )
    .unwrap()
}

fn params() -> Vec<f64> {
    (0..=8).map(|k| f64::from(k) / 8.0).collect()
}

/// `c` moved by `dx` along x, as a NURBS curve.
fn shifted(c: &Curve<3>, dx: f64) -> Curve<3> {
    let w = c.weights();
    let pts: Vec<[f64; 3]> = c
        .homogeneous()
        .iter()
        .enumerate()
        .map(|(i, h)| [h[0] + dx * w.map_or(1.0, |w| w[i]), h[1], h[2]])
        .collect();
    Curve::from_homogeneous(c.basis().clone(), pts, w.map(<[f64]>::to_vec)).unwrap()
}

/// One way a kind can be wrong, or right but at the edge of what a law allows.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fault {
    None,
    /// `to_nurbs` moved by the first number along x, with the second as its stated deviation.
    Nurbs(f64, f64),
    /// The double point's x set `k`·slack·scale beyond the enclosure's upper end (`true`) or below
    /// its lower end.
    Evaluate(f64, bool),
    /// A point past the end is not refused.
    PointOutside,
    /// A span beyond the start is not refused.
    SpanOutside,
    /// A split at the start is not refused.
    SplitAtStart,
    /// Bounds over spans other than the whole domain shrunk to their middles.
    LocalBound,
    /// Derivative bounds moved far away.
    DerivativeBound,
}

#[derive(Clone)]
struct Faulty {
    inner: Curve<3>,
    fault: Fault,
}

impl ParamCurve<3> for Faulty {
    fn domain(&self) -> (f64, f64) {
        ParamCurve::domain(&self.inner)
    }
    fn point(&self, t: f64, side: Limit) -> Result<[f64; 3], CurveError> {
        let (_, b) = self.domain();
        if self.fault == Fault::PointOutside && t > b {
            return ParamCurve::point(&self.inner, b, side);
        }
        ParamCurve::point(&self.inner, t, side)
    }
    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; 3]]) -> Result<(), CurveError> {
        ParamCurve::derivatives(&self.inner, t, side, out)?;
        if let Fault::Evaluate(k, above) = self.fault {
            let whole = self.inner.enclose(t, side, 2, &mut meter())?;
            let (lo, hi) = (whole[0][0].lo(), whole[0][0].hi());
            let end = if above { hi } else { lo };
            let largest = whole
                .iter()
                .flatten()
                .fold(0.0, |m, i| larger(m, larger(i.lo().abs(), i.hi().abs())));
            let scale = 1.0 + largest + 2.0 * end.abs() + lo.abs() + hi.abs() - end.abs();
            out[0][0] = if above {
                hi + k * SLACK * scale
            } else {
                lo - k * SLACK * scale
            };
        }
        Ok(())
    }
    fn enclose(
        &self,
        t: f64,
        side: Limit,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 3]>, CurveError> {
        self.inner.enclose(t, side, order, meter)
    }
    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        let dom = self.domain();
        if self.fault == Fault::SpanOutside && span.0 < dom.0 {
            return ParamCurve::bound(&self.inner, dom, meter);
        }
        let mut b = ParamCurve::bound(&self.inner, span, meter)?;
        if self.fault == Fault::LocalBound && span != dom {
            for d in 0..3 {
                let mid = b.lo[d] * 0.5 + b.hi[d] * 0.5;
                b.lo[d] = mid;
                b.hi[d] = mid;
            }
        }
        Ok(b)
    }
    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<3>, CurveError> {
        let b = self.inner.derivative_bound(span, order, meter)?;
        if self.fault == Fault::DerivativeBound {
            return Ok(DerivativeBound::Bounded(Bounds {
                lo: [1e9; 3],
                hi: [2e9; 3],
            }));
        }
        Ok(b)
    }
    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<3>>, CurveError> {
        if let Fault::Nurbs(dx, deviation) = self.fault {
            return Ok(Changed {
                value: NurbsForm {
                    curve: shifted(&self.inner, dx),
                    map: ParamMap::Same,
                },
                deviation,
            });
        }
        self.inner.to_nurbs(meter)
    }
    fn split(
        &self,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<3>>, Changed<CurveKind<3>>), CurveError> {
        let (a, b) = self.domain();
        if self.fault == Fault::SplitAtStart && t == a {
            return ParamCurve::split(&self.inner, f64::midpoint(a, b), meter);
        }
        ParamCurve::split(&self.inner, t, meter)
    }
    fn reversed(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<3>>, CurveError> {
        self.inner.reversed(meter)
    }
    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<3>>, CurveError> {
        self.inner.transformed(transform, meter)
    }
}

fn run(fault: Fault) -> Result<(), ContractError> {
    let kind = Faulty {
        inner: cubic(),
        fault,
    };
    check(&kind, &params(), &quarter(), &mut meter())
}

/// Claim: a conversion within its stated deviation passes, whichever way it is off, and one off by
/// more fails: each enclosure is widened by the deviation on both sides, no more.
#[test]
fn a_conversion_passes_within_its_deviation_either_way_and_fails_beyond() {
    assert_eq!(run(Fault::None), Ok(()));
    for dx in [1e-3, -1e-3] {
        assert_eq!(run(Fault::Nurbs(dx, 1e-3)), Ok(()), "moved {dx}");
        let e = run(Fault::Nurbs(dx, 1e-4)).unwrap_err();
        assert_eq!(e.law, Law::NurbsWithinItsError, "moved {dx}: {e}");
    }
}

/// Claim: doubles may lie up to slack·(1 + the largest value + |x| + |lo| + |hi|) outside the
/// enclosure, on either side, and not further: just inside the margin passes, just outside fails.
#[test]
fn evaluation_in_doubles_has_exactly_its_slack() {
    for above in [true, false] {
        assert_eq!(run(Fault::Evaluate(0.999, above)), Ok(()), "above {above}");
        let e = run(Fault::Evaluate(1.001, above)).unwrap_err();
        assert_eq!(e.law, Law::EvaluationIsEnclosed, "above {above}: {e}");
    }
}

/// Claim: each refusal is probed on its own: a kind that misses any one of them is caught.
#[test]
fn each_missing_refusal_is_caught_alone() {
    for fault in [Fault::PointOutside, Fault::SpanOutside, Fault::SplitAtStart] {
        let e = run(fault).unwrap_err();
        assert_eq!(e.law, Law::RefusesOutside, "{fault:?}: {e}");
    }
}

/// Claim: a bound wrong only on part of the domain, or a derivative bound that holds nothing, is
/// caught by its law.
#[test]
fn a_local_bound_or_a_derivative_bound_that_misses_is_caught() {
    assert_eq!(
        run(Fault::LocalBound).unwrap_err().law,
        Law::BoundHoldsPoints
    );
    assert_eq!(
        run(Fault::DerivativeBound).unwrap_err().law,
        Law::DerivativeBoundHoldsDerivatives
    );
}

/// Claim: the refusal probes lie outside the domain whatever it is: on [3.3, 4] a probe built from
/// the domain's ends without care could land inside it.
#[test]
fn the_probes_lie_outside_any_domain() {
    let c = Curve::new(
        Basis::new(1, vec![3.3, 3.3, 4.0, 4.0]).unwrap(),
        &[[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]],
    )
    .unwrap();
    check(&c, &[3.3, 3.65, 4.0], &quarter(), &mut meter()).unwrap();
}

/// The polyline (0, 0) → (1, 0) → (1, 1) on knots 0, 1, 2: a corner at t = 1, where the first
/// derivative jumps from (1, 0) to (0, 1).
fn corner() -> Curve<2> {
    Curve::new(
        Basis::new(1, vec![0.0, 0.0, 1.0, 2.0, 2.0]).unwrap(),
        &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
    )
    .unwrap()
}

fn holds(b: &Bounds<2>, v: [f64; 2]) -> bool {
    (0..2).all(|d| b.lo[d] <= v[d] && v[d] <= b.hi[d])
}

fn first_derivative_bound(c: &Curve<2>, span: (f64, f64)) -> Bounds<2> {
    match c.derivative_bound(span, 1, &mut meter()).unwrap() {
        DerivativeBound::Bounded(b) => b,
        DerivativeBound::Unbounded => panic!("unbounded"),
    }
}

/// Claim: at a corner, evaluation and bounds take the side asked for: the enclosure from the
/// right is the right piece's, a span ending at the corner bounds only the left piece's
/// derivative, one starting there only the right's, and the single parameter both.
#[test]
fn a_corner_is_approached_from_the_side_asked_for() {
    let c = corner();
    let m = &mut meter();
    let right = c.enclose(1.0, Limit::FromRight, 1, m).unwrap();
    let left = c.enclose(1.0, Limit::FromLeft, 1, m).unwrap();
    assert!(
        right[1][0].contains(0.0) && right[1][1].contains(1.0),
        "{right:?}"
    );
    assert!(
        left[1][0].contains(1.0) && left[1][1].contains(0.0),
        "{left:?}"
    );
    let (dl, dr) = ([1.0, 0.0], [0.0, 1.0]);
    let before = first_derivative_bound(&c, (0.5, 1.0));
    assert!(holds(&before, dl) && !holds(&before, dr), "{before:?}");
    let after = first_derivative_bound(&c, (1.0, 1.5));
    assert!(holds(&after, dr) && !holds(&after, dl), "{after:?}");
    let at = first_derivative_bound(&c, (1.0, 1.0));
    assert!(holds(&at, dl) && holds(&at, dr), "{at:?}");
    // Inside a piece, a degenerate span bounds that piece only.
    let inside = first_derivative_bound(&c, (0.5, 0.5));
    assert!(holds(&inside, dl) && !holds(&inside, dr), "{inside:?}");
}

/// Claim: bounds over part of a piece are the part's, not the piece's: a line from 0 to 10 bounded
/// over its first tenth, and a parabola's derivative over its first tenth.
#[test]
fn bounds_over_part_of_a_piece_are_the_parts() {
    let line = Curve::new(
        Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap(),
        &[[0.0, 0.0], [10.0, 0.0]],
    )
    .unwrap();
    let b = ParamCurve::bound(&line, (0.0, 0.1), &mut meter()).unwrap();
    assert!(b.hi[0] <= 1.0 + 1e-12, "{b:?}");
    let parabola = Curve::new(
        Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap(),
        &[[0.0, 0.0], [0.5, 0.0], [1.0, 1.0]],
    )
    .unwrap();
    // y′(t) = 2t on [0, 1]: over [0, 0.1] at most 0.2.
    let d = first_derivative_bound(&parabola, (0.0, 0.1));
    assert!(d.hi[1] <= 0.2 + 1e-12, "{d:?}");
}

/// Claim: refusals come first and cost nothing; a split must lie strictly inside the domain.
#[test]
fn refusals_come_first() {
    let c = corner();
    let mut m = meter();
    assert_eq!(
        c.enclose(2.5, Limit::FromRight, 0, &mut m),
        Err(CurveError::ParameterOutside)
    );
    assert_eq!(
        c.enclose(-0.5, Limit::FromLeft, 0, &mut m),
        Err(CurveError::ParameterOutside)
    );
    assert_eq!(m.used().work, 0);
    for t in [0.0, 2.0] {
        assert_eq!(
            ParamCurve::split(&c, t, &mut meter()).err(),
            Some(CurveError::ParameterOutside),
            "{t}"
        );
    }
}

/// Claim: a transform reports a deviation wherever its image in doubles may not be exact: entries
/// that are not doubles (3/5 and 4/5), products near underflow, and sums that round.
#[test]
fn transforms_report_a_deviation_where_doubles_round() {
    let m = &mut meter();
    let line = |a: [f64; 3], b: [f64; 3]| {
        Curve::new(Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap(), &[a, b]).unwrap()
    };
    let rot = TypedTransform::from(
        RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap(),
    );
    let c = line([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    assert!(c.transformed(&rot, m).unwrap().deviation > 0.0);
    check(&c, &[0.0, 0.5, 1.0], &rot, m).unwrap();
    // Products of 2^−970 with ±1 lie below 2^−968, where exact products are not trusted.
    let tiny = f64::from_bits(0x0350_0000_0000_0000);
    let c = line([tiny, tiny, 0.0], [2.0 * tiny, tiny, 0.0]);
    assert!(c.transformed(&quarter(), m).unwrap().deviation > 0.0);
    // 10^17 + 1 rounds: a translation by one is not exact there.
    let shift = TypedTransform::Exact(GridTransform::translation([1, 0, 0]).unwrap());
    let c = line([1e17, 0.0, 0.0], [1e17, 1.0, 0.0]);
    assert!(c.transformed(&shift, m).unwrap().deviation > 0.0);
    // At a normal scale with integers, an exact transform is exact.
    let c = line([3.0, 4.0, 0.0], [5.0, -2.0, 1.0]);
    assert_eq!(c.transformed(&quarter(), m).unwrap().deviation, 0.0);
}

/// Claim: an integer enclosed as an interval holds it exactly, and narrowly when its double rounds.
#[test]
fn integers_are_enclosed_exactly() {
    assert_eq!(int_interval(5).unwrap(), Interval::point(5.0).unwrap());
    for v in [
        (1_i128 << 53) + 1,
        -((1_i128 << 60) + 3),
        (1_i128 << 100) + 7,
    ] {
        let x = int_interval(v).unwrap();
        // Both ends are whole numbers here, so they compare with v exactly as integers.
        assert!(x.lo() as i128 <= v && v <= x.hi() as i128, "{v}: {x:?}");
        assert!(x.hi() - x.lo() <= x.hi().abs() * 1e-15, "{v}: {x:?}");
    }
}

/// Claim: every error has its own text, and a geometry-math refusal to stay in the plane is the
/// curve's.
#[test]
fn every_error_says_what_it_is() {
    for (e, part) in [
        (CurveError::ParameterOutside, "outside"),
        (CurveError::SpanOutside, "span"),
        (CurveError::NotInPlane, "planar"),
        (CurveError::Dimension { dimension: 4 }, "4"),
        (CurveError::NotFinite, "not finite"),
        (CurveError::Defect, "defect"),
        (CurveError::Nurbs(NurbsError::KnotOutside), "knot"),
    ] {
        let text = e.to_string();
        assert!(text.contains(part), "{e:?}: {text}");
    }
    let c = ContractError {
        law: Law::SplitKeepsTheCurve,
        at: 0.25,
        error: Some(CurveError::NotFinite),
    };
    let text = c.to_string();
    assert!(
        text.contains("0.25") && text.contains("not finite"),
        "{text}"
    );
    assert_eq!(
        CurveError::from(MathError::NotInPlane),
        CurveError::NotInPlane
    );
    assert_eq!(CurveError::from(MathError::Singular), CurveError::NotFinite);
}

/// Claim: an exact rotation too large for a rational transform (|q|² above 2^40) gives entries
/// enclosing M(q)/|q|², tightly, with no translation.
#[test]
fn exact_rotations_give_their_entries() {
    let r = RationalRotation::new([1 << 21, 3, 5, 7]).unwrap();
    let t = TypedTransform::from(r);
    assert!(matches!(t, TypedTransform::Rotation(_)), "{t:?}");
    let e = Entries::of(&t).unwrap();
    let (m, n2) = (r.matrix().unwrap(), r.norm2() as f64);
    let zero = Interval::point(0.0).unwrap();
    // No translation: the origin stays within a few subnormal steps of itself.
    for c in e.image_enclosed([zero; 3]).unwrap() {
        assert!(
            c.lo() <= 0.0 && 0.0 <= c.hi() && c.hi() - c.lo() < 1e-300,
            "{c:?}"
        );
    }
    for j in 0..3 {
        let mut x = [zero; 3];
        x[j] = Interval::point(1.0).unwrap();
        let column = e.image_enclosed(x).unwrap();
        for (i, c) in column.iter().enumerate() {
            let near = m[i][j] as f64 / n2;
            assert!(
                c.lo() <= near + 1e-15 && near - 1e-15 <= c.hi(),
                "{i}, {j}: {c:?}"
            );
            assert!(c.hi() - c.lo() < 1e-14, "{i}, {j}: {c:?}");
        }
    }
}
