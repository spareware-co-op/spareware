//! A curve whose pcurve on a NURBS surface is an isoparameter line inside the domain (#1185). The
//! sheet z = y − x²/2 over −1 to 1 both ways meets the plane y = c, for c = ±1/2, in the parabola
//! (x, c, c − x²/2): the sheet's pcurve is (x, c), the plane's (x, c − x²/2). The parabola is the
//! true curve, known exactly. A branch through 2 of its points (which the fit must refine: the
//! chord is 1/8 from the parabola at its middle) or 3 is fitted within the ceiling, in either
//! operand order and either direction, its chain the branch's vertices, and the cache is within
//! its bound of the parabola. The sheet's pcurve is bounded over a piece in a strip a few steps of
//! the double wide in v, where the NURBS kind's partials are rounding, so without a speed read
//! over a wider box no piece had a bound and every round of refinement failed.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly; x, y, z, c and t are the geometry's own names"
)]

use geom_curve::{ParamCurve, SurfaceRef};
use geom_intersect::fit::fit;
use geom_intersect::march::Branch;
use geom_intersect::result::CurveEnd;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};

const REFS: [SurfaceRef; 2] = [SurfaceRef(1), SurfaceRef(2)];
const CEILING: f64 = 1.0 / 16.0;

/// The sheet z = y − x²/2: degree 2 along x, 1 across y, over −1 to 1 both ways.
fn sheet() -> SurfaceKind {
    let along = Basis::new(2, vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]).unwrap();
    let across = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for (x, square) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
        for y in [-1.0, 1.0] {
            net.push([x, y, y - square / 2.0]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap())
}

/// The plane y = `c`, read as (x, z).
fn wall(c: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0, c, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            ((-4.0, 4.0), (-4.0, 4.0)),
        )
        .unwrap(),
    )
}

/// The parabola's point at x on y = `c`, as the branch's vertex: (x, c) on the sheet and
/// (x, c − x²/2) on the plane, the sheet first or second.
fn vertex(c: f64, x: f64, sheet_first: bool) -> [Interval; 4] {
    let z = c - x * x / 2.0;
    let q = if sheet_first {
        [x, c, x, z]
    } else {
        [x, z, x, c]
    };
    q.map(|q| Interval::point(q).unwrap())
}

/// Fits the branch through `along` on y = `c` and checks the curve.
fn fit_along(c: f64, sheet_first: bool, along: &[f64]) {
    let case = (c, sheet_first, along);
    let (s, w) = (sheet(), wall(c));
    let (first, second) = if sheet_first { (&s, &w) } else { (&w, &s) };
    let branch = Branch {
        vertices: along.iter().map(|&x| vertex(c, x, sheet_first)).collect(),
        ends: [CurveEnd::Boundary, CurveEnd::Boundary],
        deviation: 0.0,
        passes: Vec::new(),
    };
    let mut m = WorkMeter::new(Budget::of_work(1 << 20));
    let curve = fit((first, second), REFS, &branch, CEILING, &mut m)
        .unwrap_or_else(|e| panic!("{case:?}: {e:?}"));
    // The definition is the branch's vertices; refinement's points are fit data only.
    let chain = curve.chain();
    assert_eq!(chain.len(), along.len(), "{case:?}");
    for (marched, x) in chain.iter().zip(&branch.vertices) {
        assert_eq!(marched.at, *x, "{case:?}");
    }
    let k = usize::from(!sheet_first);
    let pcurve = &curve.pcurves()[k];
    for span in curve.spans() {
        assert!(span.curve <= CEILING, "{case:?}: {span:?}");
        let (a, b) = span.at;
        for i in 0..=16 {
            let t = if i == 16 {
                b
            } else {
                a + (b - a) * f64::from(i) / 16.0
            };
            let side = if t < b {
                Limit::FromRight
            } else {
                Limit::FromLeft
            };
            // The parabola's point at the sheet's pcurve's x: the cache's distance from it bounds
            // its distance from the true curve, which the span's bound must hold.
            let x = pcurve.point(t, side).unwrap()[0];
            let truth = [x, c, c - x * x / 2.0];
            let at = curve.cache().point(t, side).unwrap();
            let d = [at[0] - truth[0], at[1] - truth[1], at[2] - truth[2]];
            let far = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            assert!(
                far <= span.curve * span.curve,
                "{case:?}: {t} {d:?} {span:?}"
            );
        }
    }
}

#[test]
fn a_curve_on_an_isoparameter_line_of_a_sheet_is_fitted() {
    let two = [-0.5, 0.5];
    let three = [-0.5, 0.0, 0.5];
    for c in [0.5, -0.5] {
        for sheet_first in [true, false] {
            for forward in [&two[..], &three[..]] {
                let backward: Vec<f64> = forward.iter().rev().copied().collect();
                for along in [forward, &backward[..]] {
                    fit_along(c, sheet_first, along);
                }
            }
        }
    }
}
