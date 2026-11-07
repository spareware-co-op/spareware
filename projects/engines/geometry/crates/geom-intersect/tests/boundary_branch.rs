//! A branch that runs along the end of a patch's domain (G173a, chapter 12.8 step 5). The SSI
//! corpus' `c5_curve_on_edge` is a plane cut exactly along the edge u = 0 of the other patch, and
//! `c5_shared_border_L` two patches that meet at a right angle along an edge of both: the edge
//! system against the other patch has a zero for every point of the edge, so no box of it can be
//! proven, and the march cannot stand a box across the domain's end. The edge is proven to lie in
//! the other surface to within the floor and walked itself, once. Run at the scale of the SSI
//! corpus (1024 steps to the unit, floor 8, ceiling 10.24).

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
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};

const UNIT: f64 = 1024.0;
const FLOOR: f64 = 8.0;
const CEILING: f64 = 10.24;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

/// The plane z = 0 with u along x and v along y.
fn floor_plane(u: (f64, f64), v: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new([0.0; 3], [UNIT, 0.0, 0.0], [0.0, UNIT, 0.0], (u, v)).unwrap())
}

/// The plane x = 0 with u along y and v along z.
fn wall(u: (f64, f64), v: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new([0.0; 3], [0.0, UNIT, 0.0], [0.0, 0.0, UNIT], (u, v)).unwrap())
}

/// Whether `x` encloses a point within a hundredth of `at`.
fn near(x: Interval, at: f64) -> bool {
    x.lo() <= at + 0.01 && at - 0.01 <= x.hi()
}

/// The one branch of the pair: along the line x = 0, z = 0 from y = 0 to `length` units, on the
/// floor's end u = 0 and the wall's v = 0.
fn one_branch(floor_first: bool, floor: &SurfaceKind, wall: &SurfaceKind, length: f64) {
    let (a, b) = if floor_first {
        (floor, wall)
    } else {
        (wall, floor)
    };
    let found = subdivide(a, b, FLOOR, &mut meter()).unwrap();
    assert!(found.flagged.is_empty(), "{:#?}", found.flagged);
    let traced = march(a, b, &found, FLOOR, CEILING, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{:#?}", traced.unproven);
    let [branch] = traced.branches.as_slice() else {
        panic!("{:#?}", traced.branches);
    };
    assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
    // Every vertex is on the line: y is the floor's v and the wall's u, whichever is first.
    let (floor_v, wall_u) = if floor_first { (1, 2) } else { (3, 0) };
    for v in &branch.vertices {
        let y_floor = v[floor_v];
        let y_wall = v[wall_u];
        assert!(
            y_floor.lo() <= y_wall.hi() + 0.01 && y_wall.lo() <= y_floor.hi() + 0.01,
            "{v:?}"
        );
    }
    let (first, last) = (
        branch.vertices.first().unwrap(),
        branch.vertices.last().unwrap(),
    );
    let ys = [first[floor_v], last[floor_v]];
    let (low, high) = if ys[0].lo() < ys[1].lo() {
        (ys[0], ys[1])
    } else {
        (ys[1], ys[0])
    };
    assert!(near(low, 0.0) && near(high, length), "{ys:?}");
}

#[test]
fn a_plane_cut_along_the_edge_of_the_other_patch_has_that_edge_for_its_branch() {
    let floor = floor_plane((0.0, 2.0), (0.0, 2.0));
    let wall = wall((0.0, 2.0), (-1.0, 1.0));
    one_branch(true, &floor, &wall, 2.0);
    one_branch(false, &floor, &wall, 2.0);
}

#[test]
fn two_patches_meeting_along_an_edge_of_both_trace_it_once() {
    let floor = floor_plane((0.0, 1.0), (0.0, 1.0));
    let wall = wall((0.0, 1.0), (0.0, 1.0));
    one_branch(true, &floor, &wall, 1.0);
    one_branch(false, &floor, &wall, 1.0);
}
