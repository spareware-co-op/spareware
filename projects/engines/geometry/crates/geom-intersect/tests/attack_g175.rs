//! Attacks on piece G175's claims: a marched branch becomes an intersection curve whose every span
//! is certified within the ceiling, and the bounds hold against dense refinement on the definition;
//! the pcurves, mapped through their surfaces, stay within their bounds of the 3D cache; split,
//! reversal and transform keep the contract; a ceiling the fit cannot reach is an error, never a
//! curve with a wider bound.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{CurveKind, IntersectionCurve, ParamCurve, SurfaceRef};
use geom_intersect::IntersectError;
use geom_intersect::fit::{fit, true_point};
use geom_intersect::general::subdivide;
use geom_intersect::march::{Branch, march};
use geom_intersect::result::CurveEnd;
use geom_math::{GridTransform, TypedTransform};
use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{ParamSurface, Plane, SurfaceKind};

const FLOOR: f64 = 1.0 / 512.0;
const MARCH: f64 = 1.0 / 64.0;
const CEILING: f64 = 1.0 / 256.0;
const REFS: [SurfaceRef; 2] = [SurfaceRef(1), SurfaceRef(2)];

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

fn ground() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-4.0, 4.0), (-4.0, 4.0)),
        )
        .unwrap(),
    )
}

/// z = v − k·u² over [−1, 1]², quadratic in u: it crosses the ground along the parabola
/// v = k·u², from v = −1 to v = 1 side to side for k < 1.
fn sheet(k: f64) -> SurfaceKind {
    let along = Basis::new(2, vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]).unwrap();
    let across = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    // u² on [−1, 1] has Bernstein coefficients 1, −1, 1; u has −1, 0, 1.
    for (x, square) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
        for y in [-1.0, 1.0] {
            net.push([x, y, y - k * square]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap())
}

/// A biquadratic bowl over [−r, r]²: h at its centre and h + c·r² at the middle of each edge, so
/// for h < 0 < h + c·r² with its edges above z = 0 it crosses the ground in one closed loop.
fn bowl(r: f64, h: f64, c: f64) -> SurfaceKind {
    let basis = || Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for (i, x) in [-r, 0.0, r].into_iter().enumerate() {
        for (j, y) in [-r, 0.0, r].into_iter().enumerate() {
            let w = |k: usize, t: f64| if k == 1 { -t * t } else { t * t };
            net.push([x, y, h + c * (w(i, r) + w(j, r))]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap())
}

fn branches(first: &SurfaceKind, second: &SurfaceKind) -> Vec<Branch> {
    let found = subdivide(first, second, FLOOR, &mut meter()).unwrap();
    let traced = march(first, second, &found, FLOOR, MARCH, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    traced.branches
}

fn side(t: f64, (_, hi): (f64, f64)) -> Limit {
    if t < hi {
        Limit::FromRight
    } else {
        Limit::FromLeft
    }
}

/// The bound claims of `curve` at `samples` parameters across it: the true point in the plane
/// across the cache is proven, and within the span's bound of the cache; each mapped pcurve within
/// its own.
fn check_dense(
    first: &SurfaceKind,
    second: &SurfaceKind,
    curve: &IntersectionCurve<3>,
    samples: u32,
) {
    for s in curve.spans() {
        assert!(s.curve <= CEILING, "{s:?}");
    }
    let (lo, hi) = ParamCurve::domain(curve);
    let m = &mut meter();
    // Inside the domain: at an end on a surface's edge no box stands across the point.
    for k in 1..samples {
        let t = lo + (hi - lo) * f64::from(k) / f64::from(samples);
        let span = curve
            .spans()
            .iter()
            .find(|s| s.at.0 <= t && t <= s.at.1)
            .unwrap();
        let c = ParamCurve::point(curve, t, side(t, (lo, hi))).unwrap();
        let x = true_point(first, second, curve, t, m)
            .unwrap()
            .unwrap_or_else(|| panic!("{t}: no true point"));
        let p = first
            .point(
                x[0].lo() / 2.0 + x[0].hi() / 2.0,
                x[1].lo() / 2.0 + x[1].hi() / 2.0,
                (Limit::FromRight, Limit::FromRight),
            )
            .unwrap();
        let width = x.iter().map(|i| i.hi() - i.lo()).fold(0.0, f64::max);
        let d = ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2)).sqrt();
        assert!(d <= span.curve + 4.0 * width + 1e-12, "{t}: {d} > {span:?}");
        for (i, (surface, pcurve)) in [first, second].into_iter().zip(curve.pcurves()).enumerate() {
            let [u, v] = pcurve.point(t, side(t, (lo, hi))).unwrap();
            let q = surface
                .point(u, v, (Limit::FromRight, Limit::FromRight))
                .unwrap();
            let d = ((q[0] - c[0]).powi(2) + (q[1] - c[1]).powi(2) + (q[2] - c[2]).powi(2)).sqrt();
            assert!(
                d <= span.pcurves[i] + 1e-12,
                "{t}, pcurve {i}: {d} > {span:?}"
            );
        }
    }
}

#[test]
fn an_open_branch_is_certified_span_by_span() {
    let (top, flat) = (sheet(0.5), ground());
    let found = branches(&top, &flat);
    assert_eq!(found.len(), 1, "{found:?}");
    let curve = fit((&top, &flat), REFS, &found[0], CEILING, &mut meter()).unwrap();
    assert_eq!(curve.surfaces(), REFS);
    assert_eq!(curve.chain().len(), found[0].vertices.len());
    assert!(curve.error() <= CEILING);
    check_dense(&top, &flat, &curve, 400);
    // Every point of the 3D cache is near the parabola v = u²/2 on the ground.
    let (lo, hi) = ParamCurve::domain(&curve);
    for k in 0..=100 {
        let t = lo + (hi - lo) * f64::from(k) / 100.0;
        let [x, y, z] = ParamCurve::point(&curve, t, side(t, (lo, hi))).unwrap();
        assert!(z.abs() <= CEILING, "{t}: {z}");
        assert!((y - 0.5 * x * x).abs() <= 4.0 * CEILING, "{t}: ({x}, {y})");
    }
}

#[test]
fn a_closed_loop_is_certified_and_closes() {
    let (cup, flat) = (bowl(1.0, -0.25, 0.5), ground());
    let found = branches(&cup, &flat);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].ends, [CurveEnd::Closed, CurveEnd::Closed]);
    let curve = fit((&cup, &flat), REFS, &found[0], CEILING, &mut meter()).unwrap();
    let (lo, hi) = ParamCurve::domain(&curve);
    let (a, b) = (
        ParamCurve::point(&curve, lo, Limit::FromRight).unwrap(),
        ParamCurve::point(&curve, hi, Limit::FromLeft).unwrap(),
    );
    for k in 0..3 {
        assert!((a[k] - b[k]).abs() <= CEILING, "{a:?} {b:?}");
    }
    check_dense(&cup, &flat, &curve, 400);
}

#[test]
fn either_order_is_certified() {
    let (top, flat) = (sheet(0.5), ground());
    let found = branches(&flat, &top);
    let curve = fit((&flat, &top), REFS, &found[0], CEILING, &mut meter()).unwrap();
    check_dense(&flat, &top, &curve, 200);
}

#[test]
fn split_reverse_and_transform_keep_the_contract() {
    let (top, flat) = (sheet(0.5), ground());
    let found = branches(&top, &flat);
    let curve = fit((&top, &flat), REFS, &found[0], CEILING, &mut meter()).unwrap();
    let (lo, hi) = ParamCurve::domain(&curve);
    let mid = lo + (hi - lo) * 0.375;
    let m = &mut meter();
    let (left, right) = ParamCurve::split(&curve, mid, m).unwrap();
    let (CurveKind::Intersection(l), CurveKind::Intersection(r)) = (left.value, right.value) else {
        panic!("a half is not an intersection curve");
    };
    assert_eq!(ParamCurve::domain(&l), (lo, mid));
    assert_eq!(ParamCurve::domain(&r), (mid, hi));
    assert_eq!(l.spans().last().unwrap().at.1, mid);
    assert_eq!(r.spans().first().unwrap().at.0, mid);
    assert!(l.chain().iter().all(|x| x.t <= mid) && r.chain().iter().all(|x| x.t >= mid));
    assert!(l.chain().len() + r.chain().len() >= curve.chain().len());
    for half in [&l, &r] {
        for s in half.spans() {
            let whole = curve
                .spans()
                .iter()
                .find(|w| w.at.0 <= s.at.0 && s.at.1 <= w.at.1)
                .unwrap();
            assert!(s.curve >= whole.curve, "{s:?} {whole:?}");
        }
    }
    let back = ParamCurve::reversed(&curve, m).unwrap();
    let CurveKind::Intersection(back) = back.value else {
        panic!();
    };
    assert_eq!(ParamCurve::domain(&back), (-hi, -lo));
    assert_eq!(back.spans().len(), curve.spans().len());
    assert_eq!(
        back.chain().first().unwrap().t,
        -curve.chain().last().unwrap().t
    );
    let a = ParamCurve::point(&curve, mid, Limit::FromRight).unwrap();
    let b = ParamCurve::point(&back, -mid, Limit::FromLeft).unwrap();
    for k in 0..3 {
        assert!((a[k] - b[k]).abs() <= 1e-12, "{a:?} {b:?}");
    }
    // A move by a grid translation is exact: the bounds stay as they were, the pcurves too.
    let step = TypedTransform::Exact(GridTransform::translation([1, -2, 3]).unwrap());
    let moved = ParamCurve::transformed(&curve, &step, m).unwrap();
    let CurveKind::Intersection(moved) = moved.value else {
        panic!();
    };
    assert_eq!(moved.pcurves(), curve.pcurves());
    for (s, w) in moved.spans().iter().zip(curve.spans()) {
        assert!(s.curve >= w.curve, "{s:?} {w:?}");
    }
}

#[test]
fn bad_inputs_are_refused() {
    let (top, flat) = (sheet(0.5), ground());
    let found = branches(&top, &flat);
    for ceiling in [0.0, -1.0, f64::NAN] {
        assert_eq!(
            fit((&top, &flat), REFS, &found[0], ceiling, &mut meter()),
            Err(IntersectError::InvalidInput)
        );
    }
    let mut lone = found[0].clone();
    lone.vertices.truncate(1);
    assert_eq!(
        fit((&top, &flat), REFS, &lone, CEILING, &mut meter()),
        Err(IntersectError::InvalidInput)
    );
    // A ceiling the doubles cannot reach is an error, never a wider curve.
    assert!(fit((&top, &flat), REFS, &found[0], 1e-300, &mut meter()).is_err());
    let mut small = WorkMeter::new(Budget::of_work(4));
    assert_eq!(
        fit((&top, &flat), REFS, &found[0], CEILING, &mut small),
        Err(IntersectError::BudgetExceeded)
    );
    let curve = fit((&top, &flat), REFS, &found[0], CEILING, &mut meter()).unwrap();
    let (lo, hi) = ParamCurve::domain(&curve);
    assert_eq!(
        true_point(&top, &flat, &curve, hi + 1.0, &mut meter()),
        Err(IntersectError::InvalidInput)
    );
    let again = fit((&top, &flat), REFS, &found[0], CEILING, &mut meter()).unwrap();
    assert_eq!(curve, again);
    let _ = lo;
}
