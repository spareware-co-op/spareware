//! The kind contract (piece G60) on the NURBS kind and on kinds broken on purpose: every random
//! NURBS curve, planar and spatial, rational or not, with repeated knots, passes every law under
//! exact, rational and certified transforms; and a kind whose bound, reversal or evaluation is
//! wrong is caught by the law it breaks.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::many_single_char_names,
    reason = "tests: a failure stops the test"
)]

use geom_curve::contract::check;
use geom_curve::{CurveError, CurveKind, DerivativeBound, Law, ParamCurve};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Bounds, Changed, Curve, Limit};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Test inputs only: Knuth's MMIX linear congruential sequence.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn below(&mut self, n: usize) -> usize {
        ((self.next() >> 33) % n as u64) as usize
    }
}

/// A random clamped curve: degree 1 to 4, up to five spans, a repeated interior knot sometimes.
fn curve<const D: usize>(rng: &mut Lcg) -> Curve<D> {
    let p = 1 + rng.below(4);
    let mut k = vec![-1.0; p + 1];
    let mut x = -1.0;
    for _ in 0..rng.below(5) {
        x += 0.25 + rng.unit();
        let times = if rng.below(3) == 0 { p } else { 1 };
        k.extend(std::iter::repeat_n(x, times));
    }
    x += 0.5 + rng.unit();
    k.extend(std::iter::repeat_n(x, p + 1));
    let basis = Basis::new(p, k).unwrap();
    let n = basis.count();
    let pts: Vec<[f64; D]> = (0..n)
        .map(|_| core::array::from_fn(|_| rng.unit() * 200.0 - 100.0))
        .collect();
    if rng.below(2) == 0 {
        Curve::new(basis, &pts).unwrap()
    } else {
        let w: Vec<f64> = (0..n).map(|_| 0.5 + rng.unit() * 2.0).collect();
        Curve::new_rational(basis, &pts, &w).unwrap()
    }
}

fn transforms() -> Vec<TypedTransform> {
    let quarter = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    let shift = TypedTransform::Exact(GridTransform::translation([7, -3, 0]).unwrap());
    let rot = TypedTransform::from(
        RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap(),
    );
    let turn = TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap());
    let scale = TypedTransform::from(Transform::scale(2.0, 0.5, 1.0).unwrap());
    vec![quarter, shift, rot, turn, scale]
}

fn params<const D: usize>(c: &Curve<D>) -> Vec<f64> {
    let (a, b) = c.basis().domain();
    let mut t: Vec<f64> = (0..=8).map(|k| a + (b - a) * f64::from(k) / 8.0).collect();
    t.extend(
        c.basis()
            .knots()
            .iter()
            .copied()
            .filter(|&k| a <= k && k <= b),
    );
    for x in &mut t {
        *x = x.clamp(a, b);
    }
    t
}

#[test]
fn every_nurbs_curve_keeps_the_contract() {
    let mut rng = Lcg(60);
    let all = transforms();
    for trial in 0..120 {
        let t = &all[trial % all.len()];
        let c2 = curve::<2>(&mut rng);
        check(&c2, &params(&c2), t, &mut meter())
            .unwrap_or_else(|e| panic!("trial {trial} (2D): {e}"));
        let c3 = curve::<3>(&mut rng);
        check(&c3, &params(&c3), t, &mut meter())
            .unwrap_or_else(|e| panic!("trial {trial} (3D): {e}"));
        let kind = CurveKind::Nurbs(c3);
        check(
            &kind,
            &params(match &kind {
                CurveKind::Nurbs(c) => c,
            }),
            t,
            &mut meter(),
        )
        .unwrap_or_else(|e| panic!("trial {trial} (enum): {e}"));
    }
}

#[test]
fn exact_operations_are_exact() {
    let m = &mut meter();
    let c = Curve::new(
        Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]).unwrap(),
        &[[0.0, 0.0], [4.0, 8.0], [8.0, -4.0], [12.0, 0.0]],
    )
    .unwrap();
    assert_eq!(c.to_nurbs(m).unwrap().deviation, 0.0);
    let r = c.reversed(m).unwrap();
    assert_eq!(r.deviation, 0.0);
    assert_eq!(r.value.domain(), (-2.0, 0.0));
    // A grid transform of integer control points is exact.
    let quarter = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    let moved = c.transformed(&quarter, m).unwrap();
    assert_eq!(moved.deviation, 0.0);
    assert_eq!(
        moved.value.point(0.0, Limit::FromRight).unwrap(),
        [0.0, 0.0]
    );
    assert_eq!(
        moved.value.point(2.0, Limit::FromRight).unwrap(),
        [0.0, 12.0]
    );
    // A split at a knot of a curve with dyadic data is exact too.
    let (left, right) = ParamCurve::split(&c, 1.0, m).unwrap();
    assert_eq!(left.value.domain(), (0.0, 1.0));
    assert_eq!(right.value.domain(), (1.0, 2.0));
    // Refusals.
    let z = TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1));
    assert_eq!(c.transformed(&z, m), Err(CurveError::NotInPlane));
    assert_eq!(
        ParamCurve::split(&c, 0.0, m).err(),
        Some(CurveError::ParameterOutside)
    );
    assert_eq!(
        ParamCurve::bound(&c, (1.5, 1.0), m),
        Err(CurveError::SpanOutside)
    );
    assert_eq!(
        c.enclose(2.5, Limit::FromRight, 0, m),
        Err(CurveError::ParameterOutside)
    );
}

/// A NURBS curve with one behaviour broken on purpose.
#[derive(Clone)]
struct Broken {
    inner: Curve<3>,
    bound: bool,
    reverse: bool,
    evaluate: bool,
}

impl ParamCurve<3> for Broken {
    fn domain(&self) -> (f64, f64) {
        ParamCurve::domain(&self.inner)
    }
    fn point(&self, t: f64, side: Limit) -> Result<[f64; 3], CurveError> {
        ParamCurve::point(&self.inner, t, side)
    }
    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; 3]]) -> Result<(), CurveError> {
        ParamCurve::derivatives(&self.inner, t, side, out)?;
        if self.evaluate {
            out[0][0] += 1.0;
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
        let mut b = ParamCurve::bound(&self.inner, span, meter)?;
        if self.bound {
            // Shrink the box to its middle: it no longer holds the ends.
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
        self.inner.derivative_bound(span, order, meter)
    }
    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<Curve<3>>, CurveError> {
        self.inner.to_nurbs(meter)
    }
    fn split(
        &self,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<3>>, Changed<CurveKind<3>>), CurveError> {
        ParamCurve::split(&self.inner, t, meter)
    }
    fn reversed(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<3>>, CurveError> {
        if self.reverse {
            // Not reversed at all: the same curve on the negated domain is wrong.
            let c = self.inner.transformed(
                &TypedTransform::Exact(GridTransform::translation([0, 0, 0]).unwrap()),
                meter,
            )?;
            let CurveKind::Nurbs(n) = c.value;
            let knots: Vec<f64> = n.basis().knots().iter().rev().map(|k| -k).collect();
            let shifted = Curve::from_homogeneous(
                Basis::new(n.degree(), knots).unwrap(),
                n.homogeneous().to_vec(),
                n.weights().map(<[f64]>::to_vec),
            )?;
            return Ok(Changed {
                value: CurveKind::Nurbs(shifted),
                deviation: 0.0,
            });
        }
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

#[test]
fn broken_kinds_are_caught_by_their_law() {
    let inner = Curve::new(
        Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap(),
        &[
            [0.0, 0.0, 0.0],
            [1.0, 3.0, 1.0],
            [3.0, -2.0, 2.0],
            [4.0, 1.0, 0.0],
        ],
    )
    .unwrap();
    let t = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    let ps: Vec<f64> = (0..=8).map(|k| f64::from(k) / 8.0).collect();
    let base = Broken {
        inner,
        bound: false,
        reverse: false,
        evaluate: false,
    };
    check(&base, &ps, &t, &mut meter()).unwrap();
    for (broken, law) in [
        (
            Broken {
                bound: true,
                ..base.clone()
            },
            Law::BoundHoldsPoints,
        ),
        (
            Broken {
                reverse: true,
                ..base.clone()
            },
            Law::ReverseKeepsTheCurve,
        ),
        (
            Broken {
                evaluate: true,
                ..base.clone()
            },
            Law::EvaluationIsEnclosed,
        ),
    ] {
        let e = check(&broken, &ps, &t, &mut meter()).unwrap_err();
        assert_eq!(e.law, law, "{e}");
    }
}
