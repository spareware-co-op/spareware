//! Attacks on piece G56 at the edges its mutation gate found unchecked: derivatives and elevation
//! up to the largest degree, the refusals each on its own, the error texts, rational surfaces, the
//! meter's charges, and tolerances met exactly.
//!
//! What this file proves: a curve and a surface of degree [`MAX_DEGREE`] take derivatives of every
//! order up to it, and refuse the next; elevation reaches [`MAX_DEGREE`] exactly; a knot at either
//! end of the domain or NaN is refused; a tolerance that is infinite, NaN or negative is refused;
//! every error writes a sentence; a rational surface's points are the weighted average of its
//! control points; the knot operations on surfaces charge the meter for the result's size; and a
//! tolerance equal to the deviation an operation reports is met, not refused.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::many_single_char_names,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod common;

use common::Lcg;
use geom_nurbs::{Basis, Curve, Direction, Limit, MAX_DEGREE, NurbsError, Surface};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The clamped Bezier basis of degree `p` on [0, 1].
fn bezier(p: usize) -> Basis {
    let mut k = vec![0.0; p + 1];
    k.extend(vec![1.0; p + 1]);
    Basis::new(p, k).unwrap()
}

/// `n` points with coordinates in [−1, 1).
fn points(rng: &mut Lcg, n: usize) -> Vec<[f64; 3]> {
    (0..n)
        .map(|_| {
            [
                rng.unit() * 2.0 - 1.0,
                rng.unit() * 2.0 - 1.0,
                rng.unit() * 2.0 - 1.0,
            ]
        })
        .collect()
}

#[test]
fn derivatives_of_every_order_up_to_the_largest_degree() {
    let mut rng = Lcg(56);
    let p = MAX_DEGREE;
    let curve = Curve::new(bezier(p), &points(&mut rng, p + 1)).unwrap();
    // Every order up to p: the p-th derivative of a Bezier curve of degree p is constant,
    // p!·Δ^p P, the same at both ends.
    let mut out = vec![[0.0; 3]; p + 1];
    curve.derivs(0.0, Limit::FromRight, &mut out).unwrap();
    let mut end = vec![[0.0; 3]; p + 1];
    curve.derivs(1.0, Limit::FromLeft, &mut end).unwrap();
    for c in 0..3 {
        let (a, b) = (out[p][c], end[p][c]);
        assert!((a - b).abs() <= 1e-9 * (a.abs() + b.abs()), "{a} {b}");
    }
    assert_eq!(out[0], curve.point(0.0, Limit::FromRight).unwrap());
    let mut more = vec![[0.0; 3]; p + 2];
    assert_eq!(
        curve.derivs(0.5, Limit::FromRight, &mut more),
        Err(NurbsError::DerivativeOrder { order: p + 1 })
    );

    // A surface of degree p in both directions, derivatives up to (p, p).
    let surface = Surface::new(bezier(p), bezier(p), &points(&mut rng, (p + 1) * (p + 1))).unwrap();
    let mut out = vec![[0.0; 3]; (p + 1) * (p + 1)];
    surface
        .derivs(
            0.25,
            0.75,
            (Limit::FromRight, Limit::FromRight),
            (p, p),
            &mut out,
        )
        .unwrap();
    let at = surface
        .point(0.25, 0.75, (Limit::FromRight, Limit::FromRight))
        .unwrap();
    for c in 0..3 {
        assert!((out[0][c] - at[c]).abs() <= 1e-12, "{:?} {at:?}", out[0]);
    }
    let mut more = vec![[0.0; 3]; (p + 2) * (p + 1)];
    assert_eq!(
        surface.derivs(
            0.25,
            0.75,
            (Limit::FromRight, Limit::FromRight),
            (p + 1, p),
            &mut more
        ),
        Err(NurbsError::DerivativeOrder { order: p + 1 })
    );
    let mut more = vec![[0.0; 3]; (p + 1) * (p + 2)];
    assert_eq!(
        surface.derivs(
            0.25,
            0.75,
            (Limit::FromRight, Limit::FromRight),
            (p, p + 1),
            &mut more
        ),
        Err(NurbsError::DerivativeOrder { order: p + 1 })
    );
}

#[test]
fn elevation_reaches_the_largest_degree() {
    let mut rng = Lcg(57);
    let curve = Curve::new(bezier(3), &points(&mut rng, 4)).unwrap();
    let up = curve.elevate(MAX_DEGREE - 3, &mut meter()).unwrap();
    assert_eq!(up.value.degree(), MAX_DEGREE);
    for u in [0.0, 0.3, 1.0] {
        let (a, b) = (
            curve.point(u, Limit::FromRight).unwrap(),
            up.value.point(u, Limit::FromRight).unwrap(),
        );
        for c in 0..3 {
            assert!((a[c] - b[c]).abs() <= 1e-9, "{u}: {a:?} {b:?}");
        }
    }
    assert_eq!(
        curve.elevate(MAX_DEGREE - 2, &mut meter()).unwrap_err(),
        NurbsError::Degree {
            degree: MAX_DEGREE + 1
        }
    );
    let surface = Surface::new(bezier(2), bezier(3), &points(&mut rng, 12)).unwrap();
    for (dir, p) in [(Direction::U, 2), (Direction::V, 3)] {
        let up = surface.elevate(dir, MAX_DEGREE - p, &mut meter()).unwrap();
        let degree = match dir {
            Direction::U => up.value.basis_u().degree(),
            Direction::V => up.value.basis_v().degree(),
        };
        assert_eq!(degree, MAX_DEGREE);
        assert_eq!(
            surface
                .elevate(dir, MAX_DEGREE - p + 1, &mut meter())
                .unwrap_err(),
            NurbsError::Degree {
                degree: MAX_DEGREE + 1
            }
        );
    }
}

#[test]
fn each_refusal_on_its_own() {
    let mut rng = Lcg(58);
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]).unwrap();
    let surface = Surface::new(basis.clone(), bezier(1), &points(&mut rng, 8)).unwrap();
    for dir in [Direction::U, Direction::V] {
        for u in [f64::NAN, 0.0, 1.0] {
            assert_eq!(
                surface.insert_knot(dir, u, 1, &mut meter()).unwrap_err(),
                NurbsError::KnotOutside,
                "{u}"
            );
        }
        surface.insert_knot(dir, 0.25, 1, &mut meter()).unwrap();
    }
    let curve = Curve::new(basis, &points(&mut rng, 4)).unwrap();
    for tol in [f64::INFINITY, f64::NAN, -1.0, f64::NEG_INFINITY] {
        assert_eq!(
            curve.reduce_degree(tol, &mut meter()).unwrap_err(),
            NurbsError::Tolerance,
            "{tol}"
        );
        assert_eq!(
            curve.remove_knot(0.5, 1, tol, &mut meter()).unwrap_err(),
            NurbsError::Tolerance,
            "{tol}"
        );
    }
    assert_ne!(
        curve.reduce_degree(0.0, &mut meter()).err(),
        Some(NurbsError::Tolerance)
    );
}

#[test]
fn every_error_writes_a_sentence() {
    let errors = [
        NurbsError::Degree { degree: 0 },
        NurbsError::EmptyDomain,
        NurbsError::KnotRange,
        NurbsError::ParameterOutside,
        NurbsError::KnotOutside,
        NurbsError::Tolerance,
        NurbsError::OutsideTolerance,
        NurbsError::TooManyKnots,
        NurbsError::NotClamped,
        NurbsError::Discontinuous,
        NurbsError::Rational,
        NurbsError::DerivativeOrder { order: 29 },
        NurbsError::Defect,
    ];
    let texts: Vec<String> = errors.iter().map(ToString::to_string).collect();
    for (e, t) in errors.iter().zip(&texts) {
        assert!(t.len() > 10, "{e:?} writes {t:?}");
    }
    assert!(texts[0].contains("degree 0"));
    assert!(texts[11].contains("29"));
}

#[test]
fn a_rational_surface_is_the_weighted_average_of_its_points() {
    // Bilinear: at (u, v) the four functions are (1−u)(1−v), u(1−v), (1−u)v and uv.
    let pts = [
        [0.0, 0.0, 1.0],
        [2.0, 0.0, 3.0],
        [0.0, 4.0, -1.0],
        [2.0, 4.0, 5.0],
    ];
    let w = [1.0, 3.0, 0.5, 2.0];
    let s = Surface::new_rational(bezier(1), bezier(1), &pts, &w).unwrap();
    for (u, v) in [(0.25, 0.5), (0.5, 0.5), (0.75, 0.125), (0.0, 1.0)] {
        // Row-major in u: index i·2 + j for u-index i, v-index j.
        let n = [(1.0 - u) * (1.0 - v), (1.0 - u) * v, u * (1.0 - v), u * v];
        let total = n[0] * w[0] + n[1] * w[1] + n[2] * w[2] + n[3] * w[3];
        let got = s.point(u, v, (Limit::FromRight, Limit::FromRight)).unwrap();
        for c in 0..3 {
            let want = (n[0] * w[0] * pts[0][c]
                + n[1] * w[1] * pts[1][c]
                + n[2] * w[2] * pts[2][c]
                + n[3] * w[3] * pts[3][c])
                / total;
            assert!(
                (got[c] - want).abs() <= 1e-14,
                "({u}, {v}) {c}: {} {want}",
                got[c]
            );
        }
    }
    assert_eq!(s.weights(), Some(&w[..]));
}

#[test]
fn surface_knot_operations_charge_for_the_result() {
    // The work each operation charges, pinned: it grows with the result's net in the direction
    // the knots change (one line more per knot inserted), so a charge for the wrong size shows.
    let mut rng = Lcg(59);
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]).unwrap();
    let surface = Surface::new(basis, bezier(3), &points(&mut rng, 16)).unwrap();
    let mut used = Vec::new();
    for dir in [Direction::U, Direction::V] {
        let mut m = meter();
        surface.insert_knot(dir, 0.25, 2, &mut m).unwrap();
        used.push(m.used().work);
        let mut m = meter();
        surface.refine(dir, &[0.125, 0.375, 0.625], &mut m).unwrap();
        used.push(m.used().work);
        let mut m = meter();
        surface.elevate(dir, 1, &mut m).unwrap();
        used.push(m.used().work);
    }
    assert_eq!(used, USED);
}

/// The work charged by `surface_knot_operations_charge_for_the_result`, in its order.
const USED: [u128; 6] = [81, 94, 82, 82, 95, 70];

#[test]
fn a_tolerance_equal_to_the_deviation_is_met() {
    let mut rng = Lcg(60);
    // Reduction: the deviation found with a loose tolerance is accepted as the tolerance.
    let raised = Curve::new(bezier(3), &points(&mut rng, 4))
        .unwrap()
        .elevate(1, &mut meter())
        .unwrap()
        .value;
    let loose = raised.reduce_degree(f64::MAX, &mut meter()).unwrap();
    let exact = raised.reduce_degree(loose.deviation, &mut meter()).unwrap();
    assert_eq!(exact.deviation, loose.deviation);
    assert_eq!(exact.value, loose.value);
    // Removal: likewise, for a knot inserted and then removed.
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let curve = Curve::new(basis, &points(&mut rng, 5)).unwrap();
    let with = curve.insert_knot(0.5, 1, &mut meter()).unwrap().value;
    let (loose, removed) = with.remove_knot(0.5, 2, f64::MAX, &mut meter()).unwrap();
    assert!(removed >= 1);
    let (exact, again) = with
        .remove_knot(0.5, 2, loose.deviation, &mut meter())
        .unwrap();
    assert_eq!((exact.deviation, again), (loose.deviation, removed));
}

#[test]
fn a_candidate_weight_of_exactly_zero_is_refused() {
    // Removal: in a quadratic with knots 0, 0, 0, 1/2, 1, 1, 1, removing 1/2 sweeps with α = β = 2
    // (exact), so the new middle point's weight is ((2w1 − w0) + (2w2 − w3))/2, zero for weights
    // 2, 1, 1, 2. The removal stops there, with nothing removed.
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]).unwrap();
    let pts = [
        [0.0, 0.0, 0.0],
        [1.0, 2.0, 0.0],
        [2.0, 2.0, 1.0],
        [3.0, 0.0, 1.0],
    ];
    let c = Curve::new_rational(basis, &pts, &[2.0, 1.0, 1.0, 2.0]).unwrap();
    let (kept, removed) = c.remove_knot(0.5, 1, f64::MAX, &mut meter()).unwrap();
    assert_eq!(removed, 0);
    assert_eq!(kept.value, c);
    // Reduction: a cubic Bezier piece reduces with α = β = 3/2, so the middle weight is
    // ((3w1 − w0) + (3w2 − w3))/4, zero for weights 3, 1, 1, 3: refused, as outside any tolerance.
    let cubic = Curve::new_rational(bezier(3), &pts, &[3.0, 1.0, 1.0, 3.0]).unwrap();
    assert_eq!(
        cubic.reduce_degree(f64::MAX, &mut meter()).unwrap_err(),
        NurbsError::OutsideTolerance
    );
}
