//! A curve that runs along an edge of a surface's domain for its whole length: one of
//! (u₁, v₁, u₂, v₂) is on that edge all the way, not only at an end (#1179). The sheet
//! z = y − x²/2 over −1 to 1 both ways meets the plane y = ±1 along its edge v = ±1, the parabola
//! (x, ±1, ±1 − x²/2), and the plane x = ±1 along its edge u = ±1, the line (±1, y, y − 1/2). The
//! plane z = 1/8 cut off at x = ±5/8 or y = ±5/8 meets the upright plane through that edge along
//! the edge's line. The edge curve is the true curve, known exactly. A branch along it, through 3
//! or 9 of its points, is fitted within the ceiling, in either operand order and either
//! direction, on each of the four edges:
//! the pcurve on the edge lies on it exactly, and the cache is within that pcurve's bound of the
//! edge curve. The pcurve's reads reach a rounding past the edge, so without the edge held no
//! piece had a bound and every round of refinement failed.
//!
//! A branch through only 2 points of the sheet's edge curve is refined (#1185): along v = ±1 the
//! chord is 1/8 from the parabola at its middle. The points refinement adds are held on the edge,
//! so the pcurve still is; found free, they came a rounding off it, and every round failed again.
//! A curve that meets the edge only at its two vertices, the parabola y = 7/8 + x²/2 of the sheet
//! and the plane z = 7/8 from (−1/2, 1) to (1/2, 1), is refined off the edge, where it runs.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly; x, y, z, s and t are the geometry's own names"
)]

use geom_curve::{Basis as Leaf, ParamCurve, SurfaceRef};
use geom_intersect::fit::fit;
use geom_intersect::march::Branch;
use geom_intersect::result::CurveEnd;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};

const REFS: [SurfaceRef; 2] = [SurfaceRef(1), SurfaceRef(2)];

/// The four edges of a domain: which parameter is held (0 for u, 1 for v) and where.
const EDGES: [(usize, f64); 4] = [(1, 1.0), (1, -1.0), (0, 1.0), (0, -1.0)];

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

/// The plane z = 1/8, read as (x, y), over −4 to 4 both ways but for its parameter `held`, which
/// ends (or starts) at `edge`.
fn ground(held: usize, edge: f64) -> SurfaceKind {
    let mut domain = [(-4.0, 4.0); 2];
    domain[held] = if edge > 0.0 {
        (-4.0, edge)
    } else {
        (edge, 4.0)
    };
    SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 0.125],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            (domain[0], domain[1]),
        )
        .unwrap(),
    )
}

/// The upright plane x = `edge` (`held` 0), read as (y, z), or y = `edge` (`held` 1), read as
/// (x, z): through the edge, and read there as (the free coordinate, z).
fn upright(held: usize, edge: f64) -> SurfaceKind {
    let (origin, along) = if held == 0 {
        ([edge, 0.0, 0.0], [0.0, 1.0, 0.0])
    } else {
        ([0.0, edge, 0.0], [1.0, 0.0, 0.0])
    };
    SurfaceKind::Plane(
        Plane::new(origin, along, [0.0, 0.0, 1.0], ((-4.0, 4.0), (-4.0, 4.0))).unwrap(),
    )
}

/// The parameters on the edged surface at `s` along the edge: `edge` held, `s` in the other.
fn on_edge(held: usize, edge: f64, s: f64) -> [f64; 2] {
    if held == 0 { [edge, s] } else { [s, edge] }
}

/// The edge curve's point at `s`, on a surface z = `height`(x, y), and the branch's vertex there,
/// the edged surface first or second: its parameters, and the upright plane's (s, z).
fn vertex(
    height: fn(f64, f64) -> f64,
    (held, edge): (usize, f64),
    s: f64,
    edged_first: bool,
) -> [Interval; 4] {
    let [a, b] = on_edge(held, edge, s);
    let z = height(a, b);
    let x = if edged_first {
        [a, b, s, z]
    } else {
        [s, z, a, b]
    };
    x.map(|q| Interval::point(q).unwrap())
}

/// Fits the branch along the edge, both orders and both directions, through 3 and through 9
/// vertices (where the least-squares pcurve's control points round past the edge), and checks
/// the curve.
fn along_the_edge(edged: &SurfaceKind, height: fn(f64, f64) -> f64, (held, edge): (usize, f64)) {
    let cut = upright(held, edge);
    let three = vec![-0.5, 0.0, 0.5];
    let nine: Vec<f64> = (0..9).map(|k| f64::from(k) / 8.0 - 0.5).collect();
    for edged_first in [true, false] {
        let (first, second) = if edged_first {
            (edged, &cut)
        } else {
            (&cut, edged)
        };
        for forward in [&three, &nine] {
            let backward: Vec<f64> = forward.iter().rev().copied().collect();
            for along in [forward, &backward] {
                fit_along((first, second), height, (held, edge), edged_first, along);
            }
        }
    }
}

/// Fits the branch through `along` and checks the curve (`along_the_edge`).
fn fit_along(
    (first, second): (&SurfaceKind, &SurfaceKind),
    height: fn(f64, f64) -> f64,
    (held, edge): (usize, f64),
    edged_first: bool,
    along: &[f64],
) {
    let case = (held, edge, edged_first, along);
    let branch = Branch {
        vertices: along
            .iter()
            .map(|&s| vertex(height, (held, edge), s, edged_first))
            .collect(),
        ends: [CurveEnd::Boundary, CurveEnd::Boundary],
        deviation: 0.0,
        passes: Vec::new(),
    };
    let mut m = WorkMeter::new(Budget::of_work(1 << 20));
    let curve = fit((first, second), REFS, &branch, 1.0 / 16.0, &mut m)
        .unwrap_or_else(|e| panic!("{case:?}: {e:?}"));
    let k = usize::from(!edged_first);
    let pcurve = &curve.pcurves()[k];
    // The pcurve on the edge lies on it exactly: every control point has the edge there.
    let Leaf::Nurbs(c) = pcurve.basis() else {
        panic!("{case:?}: not a NURBS pcurve");
    };
    assert!(c.weights().is_none(), "{case:?}");
    for p in c.homogeneous() {
        assert_eq!(p[held], edge, "{case:?}: {p:?}");
    }
    // The cache is within the pcurve's bound of the edge curve: the edged surface's point
    // at the pcurve's, which is the edge curve's point at its free coordinate.
    for span in curve.spans() {
        assert!(span.curve <= 1.0 / 16.0, "{case:?}: {span:?}");
        let (a, b) = span.at;
        for t in [a, a / 2.0 + b / 2.0, b] {
            let side = if t < b {
                Limit::FromRight
            } else {
                Limit::FromLeft
            };
            let p = pcurve.point(t, side).unwrap();
            let [x, y] = on_edge(held, edge, p[1 - held]);
            let truth = [x, y, height(x, y)];
            let at = curve.cache().point(t, side).unwrap();
            let far = (0..3)
                .map(|i| (at[i] - truth[i]).abs())
                .fold(0.0, |m: f64, d| if d > m { d } else { m });
            // A few steps of the double for this test's own reads.
            assert!(
                far <= span.pcurves[k] + 1e-15,
                "{case:?}: {t} {far} {span:?}"
            );
        }
    }
}

#[test]
fn a_curve_along_an_edge_of_a_sheet_is_fitted_on_that_edge() {
    for edge in EDGES {
        along_the_edge(&sheet(), |x, y| y - x * x / 2.0, edge);
    }
}

#[test]
fn a_line_along_an_edge_of_a_plane_is_fitted_on_that_edge() {
    for (held, edge) in EDGES {
        let edge = edge * 0.625;
        along_the_edge(&ground(held, edge), |_, _| 0.125, (held, edge));
    }
}

#[test]
fn a_curve_along_an_edge_of_a_sheet_through_two_points_is_refined_on_that_edge() {
    let height = |x: f64, y: f64| y - x * x / 2.0;
    for (held, edge) in EDGES {
        let cut = upright(held, edge);
        let sheet = sheet();
        for edged_first in [true, false] {
            let (first, second) = if edged_first {
                (&sheet, &cut)
            } else {
                (&cut, &sheet)
            };
            for along in [[-0.5, 0.5], [0.5, -0.5]] {
                fit_along((first, second), height, (held, edge), edged_first, &along);
            }
        }
    }
}

#[test]
fn a_curve_that_meets_an_edge_only_at_its_ends_is_refined_off_it() {
    // The sheet and the plane z = 7/8, read as (x, y), meet in y = 7/8 + x²/2: on the sheet's
    // edge v = 1 at x = ±1/2, below it between.
    let level = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 0.875],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-4.0, 4.0), (-4.0, 4.0)),
        )
        .unwrap(),
    );
    let sheet = sheet();
    for (sheet_first, (first, second)) in [(true, (&sheet, &level)), (false, (&level, &sheet))] {
        for along in [[-0.5, 0.5], [0.5, -0.5]] {
            let case = (sheet_first, along);
            let branch = Branch {
                vertices: along
                    .iter()
                    .map(|&x| [x, 1.0, x, 1.0].map(|q| Interval::point(q).unwrap()))
                    .collect(),
                ends: [CurveEnd::Boundary, CurveEnd::Boundary],
                deviation: 0.0,
                passes: Vec::new(),
            };
            let mut m = WorkMeter::new(Budget::of_work(1 << 20));
            let curve = fit((first, second), REFS, &branch, 1.0 / 16.0, &mut m)
                .unwrap_or_else(|e| panic!("{case:?}: {e:?}"));
            assert_eq!(curve.chain().len(), 2, "{case:?}");
            for span in curve.spans() {
                assert!(span.curve <= 1.0 / 16.0, "{case:?}: {span:?}");
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
                    // On the parabola to within the span's bound: z exactly so, y within twice it
                    // (its slope in x is at most a little over 1/2 there).
                    let [x, y, z] = curve.cache().point(t, side).unwrap();
                    assert!((z - 0.875).abs() <= span.curve, "{case:?}: {t} {z}");
                    let off = (y - (0.875 + x * x / 2.0)).abs();
                    assert!(off <= 2.0 * span.curve, "{case:?}: {t} ({x}, {y})");
                }
            }
        }
    }
}
