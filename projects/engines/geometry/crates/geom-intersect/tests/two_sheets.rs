//! Issue #1167: a plane's edge lying on two sheets of the other surface. A swept profile crosses
//! itself along the floor's edge u = 0, so the edge lies on the profile's sheet u = 1/2 and on its
//! sheet u = 9/10, and the intersection is two branches, the edge on each sheet. The run that
//! explains the edge system's flagged boxes is sought from the middle of the patch they are cut to,
//! so a patch that holds only the second sheet proves the run on it; the march proves its run from
//! the same patch, so each branch keeps to its own sheet at every vertex. Run at the scale of the
//! SSI corpus (1024 steps to the unit, floor 8, ceiling 10.24).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_intersect::general::subdivide;
use geom_intersect::march::march;
use geom_intersect::result::CurveEnd;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};

const UNIT: f64 = 1024.0;
const FLOOR: f64 = 8.0;
const CEILING: f64 = 10.24;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

/// Whether `x` encloses a point within a hundredth of `at`.
fn near(x: Interval, at: f64) -> bool {
    x.lo() <= at + 0.01 && at - 0.01 <= x.hi()
}

/// A Bézier basis of degree `p` over [0, 1].
fn basis(p: usize) -> Basis {
    let mut knots = vec![0.0; p + 1];
    knots.extend(vec![1.0; p + 1]);
    Basis::new(p, knots).unwrap()
}

/// The issue's swept profile: a cubic in (x, z), in steps, swept along y from −512 to 1536 (its v
/// from 0 to 1). It comes up through the origin at u = 1/2, loops over it and goes down through it
/// again at u = 9/10, every number exact, so the line x = 0, z = 0 lies on two sheets of the
/// surface, crossing at an angle. Elsewhere on x ≥ −300 the profile is below or above z = 0.
fn sheets() -> SurfaceKind {
    let profile = [
        (-8100.0, -84375.0 / 16.0),
        (8475.0 / 2.0, 3125.0 / 16.0),
        (-1675.0, 28125.0 / 16.0),
        (825.0 / 2.0, -9375.0 / 16.0),
    ];
    let mut net = Vec::new();
    for (x, z) in profile {
        for y in [-512.0, 1536.0] {
            net.push([x, y, z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(basis(3), basis(1), &net).unwrap())
}

/// The floor meets the swept profile in two branches, in either order: nothing flagged, every step
/// proven, each branch the floor's edge u = 0 from its corner (0, 0) to its corner (0, 1), one on
/// the sheet u = 1/2 and one on the sheet u = 9/10, each at every vertex on its own sheet, at the
/// profile's v = 1/4 + y/2 for the floor's v = y: one point of space.
#[test]
fn an_edge_on_two_sheets_of_the_other_surface_is_two_branches() {
    let floor = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [UNIT, 0.0, 0.0],
            [0.0, UNIT, 0.0],
            ((0.0, 2.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    let swept = sheets();
    for floor_first in [true, false] {
        let (a, b) = if floor_first {
            (&floor, &swept)
        } else {
            (&swept, &floor)
        };
        let (floor_u, floor_v, sheet_u, sheet_v) = if floor_first {
            (0, 1, 2, 3)
        } else {
            (2, 3, 0, 1)
        };
        let found = subdivide(a, b, FLOOR, &mut meter()).unwrap();
        assert!(
            found.flagged.is_empty(),
            "{floor_first}: {:#?}",
            found.flagged
        );
        let traced = march(a, b, &found, FLOOR, CEILING, &mut meter()).unwrap();
        assert!(traced.unproven.is_empty(), "{:#?}", traced.unproven);
        let [one, two] = traced.branches.as_slice() else {
            panic!("{floor_first}: {:#?}", traced.branches);
        };
        let mut on = Vec::new();
        for branch in [one, two] {
            assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
            let sheet = branch.vertices.first().unwrap()[sheet_u];
            let at = if near(sheet, 0.5) { 0.5 } else { 0.9 };
            for v in &branch.vertices {
                assert!(near(v[floor_u], 0.0) && near(v[sheet_u], at), "{v:?}");
                let y = v[floor_v];
                let across = Interval::new(0.25 + y.lo() / 2.0, 0.25 + y.hi() / 2.0).unwrap();
                assert!(
                    across.lo() <= v[sheet_v].hi() + 0.01 && v[sheet_v].lo() <= across.hi() + 0.01,
                    "{v:?}"
                );
            }
            let ends = [
                branch.vertices.first().unwrap()[floor_v],
                branch.vertices.last().unwrap()[floor_v],
            ];
            assert!(
                (near(ends[0], 0.0) && near(ends[1], 1.0))
                    || (near(ends[0], 1.0) && near(ends[1], 0.0)),
                "{ends:?}"
            );
            on.push(at < 0.7);
        }
        // One branch on each sheet.
        assert!(on.contains(&true) && on.contains(&false), "{floor_first}");
    }
}
