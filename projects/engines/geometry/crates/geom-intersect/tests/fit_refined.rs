//! The fit's refinement (G175, chapter 12.12): a branch too sparse for its first fit gets points of
//! the true curve between its data until every span is certified, and those points are fit data
//! only, never the definition. The branch here is two vertices on z = v − k·u² and the plane
//! z = c, which meet along v = c + k·u²: a line through two points is far from that parabola, so
//! the first round fails and the true point between them must be found. The ends are inside both
//! domains, so the true point is proven at the curve's own ends too, read from the inside.

#![allow(
    clippy::unwrap_used,
    clippy::many_single_char_names,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{IntersectionCurve, ParamCurve, SurfaceRef};
use geom_intersect::IntersectError;
use geom_intersect::fit::{fit, true_point};
use geom_intersect::march::Branch;
use geom_intersect::result::CurveEnd;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{ParamSurface, Plane, SurfaceKind};

const CEILING: f64 = 1.0 / 256.0;
const REFS: [SurfaceRef; 2] = [SurfaceRef(1), SurfaceRef(2)];
const K: f64 = 0.5;
const C: f64 = 0.125;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

/// The plane z = `c`, its parameters x and y.
fn level(c: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, c],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-4.0, 4.0), (-4.0, 4.0)),
        )
        .unwrap(),
    )
}

/// z = v − k·u² over [−1, 1]², its parameters x and y.
fn sheet(k: f64) -> SurfaceKind {
    let along = Basis::new(2, vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]).unwrap();
    let across = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for (x, square) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
        for y in [-1.0, 1.0] {
            net.push([x, y, y - k * square]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap())
}

/// The vertex of the crossing at x = `u`: (u, v) on both, v = c + k·u².
fn vertex(u: f64) -> [Interval; 4] {
    let v = C + K * u * u;
    [u, v, u, v].map(|x| Interval::point(x).unwrap())
}

fn sparse(us: &[f64], ends: [CurveEnd; 2]) -> Branch {
    Branch {
        vertices: us.iter().map(|&u| vertex(u)).collect(),
        ends,
        deviation: 0.0,
        passes: Vec::new(),
    }
}

fn side(t: f64, (_, hi): (f64, f64)) -> Limit {
    if t < hi {
        Limit::FromRight
    } else {
        Limit::FromLeft
    }
}

/// The curve's claims at `t`: the true point there is proven, on the crossing, and within its
/// span's bound of the cache.
fn true_point_holds(
    first: &SurfaceKind,
    second: &SurfaceKind,
    curve: &IntersectionCurve<3>,
    t: f64,
) {
    let domain = ParamCurve::domain(curve);
    let span = curve
        .spans()
        .iter()
        .find(|s| s.at.0 <= t && t <= s.at.1)
        .unwrap();
    let x = true_point(first, second, curve, t, &mut meter())
        .unwrap()
        .unwrap_or_else(|| panic!("{t}: no true point"));
    let width = x.iter().map(|i| i.hi() - i.lo()).fold(0.0, f64::max);
    let m = x.map(|i| i.lo() / 2.0 + i.hi() / 2.0);
    // On the crossing: the same point on both, v = c + k·u², and the first's on the sheet.
    assert!(
        (m[0] - m[2]).abs() <= 1e-9 && (m[1] - m[3]).abs() <= 1e-9,
        "{t}: {x:?}"
    );
    assert!((m[1] - (C + K * m[0] * m[0])).abs() <= 1e-9, "{t}: {x:?}");
    let p = first
        .point(m[0], m[1], (Limit::FromRight, Limit::FromRight))
        .unwrap();
    let c = ParamCurve::point(curve, t, side(t, domain)).unwrap();
    let d = ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2)).sqrt();
    assert!(d <= span.curve + 4.0 * width + 1e-12, "{t}: {d} > {span:?}");
    // In the plane across the cache at t.
    let mut dc = [[0.0; 3]; 2];
    ParamCurve::derivatives(curve.cache(), t, side(t, domain), &mut dc).unwrap();
    let [at, tangent] = dc;
    let across: f64 = (0..3).map(|k| tangent[k] * (p[k] - at[k])).sum();
    let size = (0..3).map(|k| tangent[k] * tangent[k]).sum::<f64>().sqrt();
    assert!(across.abs() <= 1e-9 * size, "{t}: {across}");
}

fn refined(first: &SurfaceKind, second: &SurfaceKind, us: &[f64]) {
    let branch = sparse(us, [CurveEnd::Boundary, CurveEnd::Boundary]);
    let curve = fit((first, second), REFS, &branch, CEILING, &mut meter()).unwrap();
    // The definition is the branch's two vertices, at the curve's two ends; the points the
    // refinement added are fit data only.
    let (lo, hi) = ParamCurve::domain(&curve);
    let chain = curve.chain();
    assert_eq!(chain.len(), 2, "{chain:?}");
    assert_eq!((chain[0].t, chain[1].t), (lo, hi));
    assert_eq!(chain[0].at, branch.vertices[0]);
    assert_eq!(chain[1].at, branch.vertices[1]);
    assert!(curve.error() <= CEILING);
    for s in curve.spans() {
        assert!(
            s.curve <= CEILING && s.pcurves.iter().all(|&d| d <= CEILING),
            "{s:?}"
        );
    }
    // Every point of the cache is near the parabola on the plane z = c, from end to end.
    for k in 0..=200 {
        let t = lo + (hi - lo) * f64::from(k) / 200.0;
        let [x, y, z] = ParamCurve::point(&curve, t, side(t, (lo, hi))).unwrap();
        assert!((z - C).abs() <= CEILING, "{t}: {z}");
        assert!(
            (y - (C + K * x * x)).abs() <= 2.0 * CEILING,
            "{t}: ({x}, {y})"
        );
        assert!(
            x >= us[0].min(us[1]) - CEILING && x <= us[0].max(us[1]) + CEILING,
            "{t}: {x}"
        );
    }
    let (a, b) = (
        ParamCurve::point(&curve, lo, Limit::FromRight).unwrap(),
        ParamCurve::point(&curve, hi, Limit::FromLeft).unwrap(),
    );
    assert!(
        (a[0] - us[0]).abs() <= CEILING && (b[0] - us[1]).abs() <= CEILING,
        "{a:?} {b:?}"
    );
    // The true point is proven everywhere, the ends too: neither end is on a domain's edge.
    for k in 0..=16 {
        let t = lo + (hi - lo) * f64::from(k) / 16.0;
        let t = if k == 16 { hi } else { t };
        true_point_holds(first, second, &curve, t);
    }
    for t in [lo - 1.0, hi + 1.0, f64::NAN] {
        assert_eq!(
            true_point(first, second, &curve, t, &mut meter()),
            Err(IntersectError::InvalidInput),
            "{t}"
        );
    }
}

#[test]
fn a_two_vertex_branch_is_refined_into_a_certified_curve() {
    let (top, flat) = (sheet(K), level(C));
    refined(&top, &flat, &[-0.6, 0.7]);
    refined(&top, &flat, &[0.7, -0.6]);
}

#[test]
fn a_two_vertex_branch_on_the_plane_first_is_refined_too() {
    // Both take (x, y) for parameters, so the vertices read the same either way round.
    let (top, flat) = (sheet(K), level(C));
    refined(&flat, &top, &[-0.5, 0.6]);
}

#[test]
fn a_closed_loop_of_three_vertices_is_refined_and_closes() {
    // The plane z = c cuts the bowl z = x² + y² − 1/4 in the circle of radius √(c + 1/4).
    let along = || Basis::new(2, vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for (x, sx) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
        for (y, sy) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
            net.push([x, y, sx + sy - 0.25]);
        }
    }
    let bowl = SurfaceKind::Nurbs(Surface::new(along(), along(), &net).unwrap());
    let flat = level(C);
    let r = (C + 0.25_f64).sqrt();
    let at = |a: f64| {
        let (x, y) = (r * a.cos(), r * a.sin());
        [x, y, x, y].map(|q| Interval::point(q).unwrap())
    };
    let third = core::f64::consts::TAU / 3.0;
    let branch = Branch {
        vertices: vec![at(0.0), at(third), at(2.0 * third)],
        ends: [CurveEnd::Closed, CurveEnd::Closed],
        deviation: 0.0,
        passes: Vec::new(),
    };
    let curve = fit((&bowl, &flat), REFS, &branch, CEILING, &mut meter()).unwrap();
    // The definition is the three vertices; the closing point and the refinement's are data.
    assert_eq!(curve.chain().len(), 3);
    assert!(curve.error() <= CEILING);
    let (lo, hi) = ParamCurve::domain(&curve);
    let (a, b) = (
        ParamCurve::point(&curve, lo, Limit::FromRight).unwrap(),
        ParamCurve::point(&curve, hi, Limit::FromLeft).unwrap(),
    );
    for k in 0..3 {
        assert!((a[k] - b[k]).abs() <= CEILING, "{a:?} {b:?}");
    }
    for k in 0..=100 {
        let t = lo + (hi - lo) * f64::from(k) / 100.0;
        let [x, y, z] = ParamCurve::point(&curve, t, side(t, (lo, hi))).unwrap();
        assert!((z - C).abs() <= CEILING, "{t}: {z}");
        assert!(
            ((x * x + y * y).sqrt() - r).abs() <= 2.0 * CEILING,
            "{t}: ({x}, {y})"
        );
    }
}
