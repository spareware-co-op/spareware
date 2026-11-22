//! The true point at an end of a curve where two or more of (u₁, v₁, u₂, v₂) lie on edges of
//! their domains: a corner. The sheet z = y − x²/2 meets the plane z = 1/8 along the parabola
//! y = 1/8 + x²/2, which reaches x = 1 at (1, 5/8, 1/8). With the sheet's y stopping at 5/8 that
//! point is a corner of its domain, and the plane's domain can stop at x = 1 or y = 5/8 too, so the
//! curve's end holds two, three or all four parameters on edges. A branch from x = 3/10 to that
//! point is fitted, and at the curve's end there the true point is that point exactly,
//! (u₁, v₁, u₂, v₂) = (1, 5/8, 1, 5/8): it lies on two or more faces of any box about it that
//! stands in the domains, where neither the interior system nor one edge's can prove it. With
//! every parameter on an edge held, the system has more equations than unknowns; the end is proven
//! to within the rounding of the kinds' reads, in either operand order and at either end of the
//! curve. A branch that ends on a corner the true curve misses has no true point there, and says
//! so at once.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::panic,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{ParamCurve, SurfaceRef};
use geom_intersect::fit::{fit, true_point};
use geom_intersect::march::Branch;
use geom_intersect::result::CurveEnd;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};

const REFS: [SurfaceRef; 2] = [SurfaceRef(1), SurfaceRef(2)];

/// The sheet z = y − x²/2: degree 2 along x over -1 to 1, 1 across y over -1 to `top`.
fn sheet(top: f64) -> SurfaceKind {
    let along = Basis::new(2, vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]).unwrap();
    let across = Basis::new(1, vec![-1.0, -1.0, top, top]).unwrap();
    let mut net = Vec::new();
    for (x, square) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
        for y in [-1.0, top] {
            net.push([x, y, y - square / 2.0]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap())
}

/// The plane z = 1/8, read as (x, y), over -4 to `right` in x and -4 to `back` in y.
fn level(right: f64, back: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 0.125],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-4.0, right), (-4.0, back)),
        )
        .unwrap(),
    )
}

/// The branch through the points (x, y) of `ends`, the same (x, y) on both surfaces.
fn branch(ends: [[f64; 2]; 2]) -> Branch {
    let at = |[x, y]: [f64; 2]| [x, y, x, y].map(|q| Interval::point(q).unwrap());
    Branch {
        vertices: ends.map(at).to_vec(),
        ends: [CurveEnd::Boundary, CurveEnd::Boundary],
        deviation: 0.0,
        passes: Vec::new(),
    }
}

/// The parabola's point at `x`.
fn on(x: f64) -> [f64; 2] {
    [x, 0.125 + x * x / 2.0]
}

/// The true point at the end of the branch `ends` that is `ends[at_end]`, on `first` and `second`.
fn end_point(
    first: &SurfaceKind,
    second: &SurfaceKind,
    ends: [[f64; 2]; 2],
    at_end: usize,
) -> Option<[Interval; 4]> {
    let mut m = WorkMeter::new(Budget::of_work(1 << 20));
    let curve = fit((first, second), REFS, &branch(ends), 1.0 / 16.0, &mut m).unwrap();
    let (lo, hi) = ParamCurve::domain(&curve);
    let t = [lo, hi][at_end];
    let mut m = WorkMeter::new(Budget::of_work(1 << 16));
    true_point(first, second, &curve, t, &mut m).unwrap()
}

#[test]
fn the_true_point_at_a_curves_end_on_a_corner_is_proven_there() {
    let truth = [1.0, 0.625, 1.0, 0.625];
    // The sheet's y ends at 5/8 or 1, the plane's x at 1 or 4 and its y at 5/8 or 4: the
    // parameters on edges at the end are (u₁, v₁), (u₁, u₂), (u₁, v₁, u₂), (u₁, u₂, v₂) and all
    // four.
    for (top, right, back) in [
        (0.625, 4.0, 4.0),
        (1.0, 1.0, 4.0),
        (0.625, 1.0, 4.0),
        (1.0, 1.0, 0.625),
        (0.625, 1.0, 0.625),
    ] {
        let (bent, flat) = (sheet(top), level(right, back));
        for (first, second) in [(&bent, &flat), (&flat, &bent)] {
            for (ends, at_end) in [([on(0.3), on(1.0)], 1), ([on(1.0), on(0.3)], 0)] {
                let case = (top, right, back, ends);
                let x = end_point(first, second, ends, at_end)
                    .unwrap_or_else(|| panic!("{case:?}: no true point"));
                for (i, q) in x.iter().zip(truth) {
                    assert!(i.lo() <= q && q <= i.hi(), "{case:?}: {x:?}");
                    assert!(i.hi() - i.lo() <= 1e-12, "{case:?}: {x:?}");
                }
            }
        }
    }
}

#[test]
fn a_branch_ending_on_a_corner_the_curve_misses_has_no_true_point_there() {
    // The sheet's y ends at 5/8 − 1/1024, so the parabola leaves through that edge just short of
    // x = 1, and the corner (1, 5/8 − 1/1024) is 1/1024 below the plane: not on the true curve.
    let edge = 0.625 - 1.0 / 1024.0;
    let (bent, flat) = (sheet(edge), level(4.0, 4.0));
    for (first, second) in [(&bent, &flat), (&flat, &bent)] {
        for (ends, at_end) in [([on(0.3), [1.0, edge]], 1), ([[1.0, edge], on(0.3)], 0)] {
            assert_eq!(end_point(first, second, ends, at_end), None, "{ends:?}");
        }
    }
}
