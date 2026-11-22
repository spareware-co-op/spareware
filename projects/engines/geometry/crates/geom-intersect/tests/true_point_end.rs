//! The true point at an end of a curve that lies on an edge of a domain. The sheet
//! z = y − x²/2 over x and y from -1 to 1 meets the plane z = 1/8 along the parabola
//! y = 1/8 + x²/2, which leaves the sheet through its edge x = 1 at (1, 5/8, 1/8). A branch from
//! x = 3/10 to that edge is fitted, and at the curve's end on the edge the true point is that
//! point exactly, (u₁, v₁, u₂, v₂) = (1, 5/8, 1, 5/8), in the plane across the cache there too:
//! the cache ends on it. Any box about it that stands in the sheet's domain has it on its face,
//! where the interior system cannot prove it; the edge system proves it, in either operand order
//! and at either end of the curve.

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

/// The sheet z = y − x²/2: degree 2 along x, 1 across y, both over -1 to 1.
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

/// The plane z = 1/8, read as (x, y), over -4 to 4 both ways.
fn level() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 0.125],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-4.0, 4.0), (-4.0, 4.0)),
        )
        .unwrap(),
    )
}

/// The branch through the parabola's points at `xs`, the same (x, y) on both surfaces.
fn branch(xs: [f64; 2]) -> Branch {
    let at = |x: f64| {
        let y = 0.125 + x * x / 2.0;
        [x, y, x, y].map(|q| Interval::point(q).unwrap())
    };
    Branch {
        vertices: xs.map(at).to_vec(),
        ends: [CurveEnd::Boundary, CurveEnd::Boundary],
        deviation: 0.0,
        passes: Vec::new(),
    }
}

#[test]
fn the_true_point_at_a_curves_end_on_a_domains_edge_is_proven_there() {
    let (top, flat) = (sheet(), level());
    let truth = [1.0, 0.625, 1.0, 0.625];
    for (first, second) in [(&top, &flat), (&flat, &top)] {
        for (xs, at_end) in [([0.3, 1.0], 1), ([1.0, 0.3], 0)] {
            let mut m = WorkMeter::new(Budget::of_work(1 << 20));
            let curve = fit((first, second), REFS, &branch(xs), 1.0 / 16.0, &mut m).unwrap();
            let (lo, hi) = ParamCurve::domain(&curve);
            let t = [lo, hi][at_end];
            let mut m = WorkMeter::new(Budget::of_work(1 << 16));
            let x = true_point(first, second, &curve, t, &mut m)
                .unwrap()
                .unwrap_or_else(|| panic!("{xs:?} at {t}: no true point"));
            for (i, q) in x.iter().zip(truth) {
                assert!(i.lo() <= q && q <= i.hi(), "{xs:?} at {t}: {x:?}");
                assert!(i.hi() - i.lo() <= 1e-12, "{xs:?} at {t}: {x:?}");
            }
        }
    }
}
