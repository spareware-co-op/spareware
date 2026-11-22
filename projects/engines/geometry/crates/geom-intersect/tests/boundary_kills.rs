//! Issue #1166: a plane's edge lying inside another plane whose grid does not meet the edge's
//! ends. The edge system is searched over the edge and the room past its ends, so a box flagged
//! across an end can hold only the edge continued past it, where a run proven over the edge alone
//! meets none of its `(s, r)`. The run that explains the flagged boxes is proven over that room
//! too, where the other plane holds it, and the pair gives its one branch, the floor's edge u = 0.
//! Run at the scale of the SSI corpus (1024 steps to the unit, floor 8, ceiling 10.24).

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

/// The floor's place in a pair's four parameters, and the other surface's: `(floor u, floor v,
/// other u, other v)`.
fn places(floor_first: bool) -> (usize, usize, usize, usize) {
    if floor_first {
        (0, 1, 2, 3)
    } else {
        (2, 3, 0, 1)
    }
}

/// The floor's v at the two ends of a branch, the lower first.
fn reach(vertices: &[[Interval; 4]], floor_v: usize) -> (Interval, Interval) {
    let ys = [
        vertices.first().unwrap()[floor_v],
        vertices.last().unwrap()[floor_v],
    ];
    if ys[0].lo() < ys[1].lo() {
        (ys[0], ys[1])
    } else {
        (ys[1], ys[0])
    }
}

/// The one branch of the pair, in either order: nothing flagged, every step proven, along the line
/// x = 0, z = 0 from y = `from` to `to` units, on the floor's end u = 0 and the wall's v = 0, with
/// the floor's v and the wall's u equal at every vertex.
fn one_branch(floor: &SurfaceKind, wall: &SurfaceKind, (from, to): (f64, f64)) {
    for floor_first in [true, false] {
        let (a, b) = if floor_first {
            (floor, wall)
        } else {
            (wall, floor)
        };
        let found = subdivide(a, b, FLOOR, &mut meter()).unwrap();
        assert!(
            found.flagged.is_empty(),
            "{floor_first}: {:#?}",
            found.flagged
        );
        let traced = march(a, b, &found, FLOOR, CEILING, &mut meter()).unwrap();
        assert!(traced.unproven.is_empty(), "{:#?}", traced.unproven);
        let [branch] = traced.branches.as_slice() else {
            panic!("{floor_first}: {:#?}", traced.branches);
        };
        assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
        let (floor_u, floor_v, wall_u, wall_v) = places(floor_first);
        for v in &branch.vertices {
            assert!(near(v[floor_u], 0.0) && near(v[wall_v], 0.0), "{v:?}");
            let (y_floor, y_wall) = (v[floor_v], v[wall_u]);
            assert!(
                y_floor.lo() <= y_wall.hi() + 0.01 && y_wall.lo() <= y_floor.hi() + 0.01,
                "{v:?}"
            );
        }
        let (low, high) = reach(&branch.vertices, floor_v);
        assert!(near(low, from) && near(high, to), "{low:?} {high:?}");
    }
}

/// The wall's y-ranges, in units, of issue #1166 that were left flagged against the floor's edge
/// from y = 0 to 1: a box across an end of the edge, its `s` wholly past that end, held only the
/// edge continued there.
const UNMET: [(f64, f64); 7] = [
    (-1.0, 2.0),
    (-0.5, 1.0),
    (-0.0625, 1.0),
    (-0.25, 1.25),
    (0.0, 1.0625),
    (-0.125, 1.0),
    (-0.5, 1.5),
];

/// And those of the issue that settled already.
const MET: [(f64, f64); 4] = [(-1.0, 1.0), (-0.0625, 1.0625), (-3.0, 3.0), (0.0, 2.0)];

#[test]
fn an_edge_inside_a_plane_whose_grid_misses_its_ends_is_the_branch() {
    let floor = floor_plane((0.0, 2.0), (0.0, 1.0));
    for y in UNMET.into_iter().chain(MET) {
        one_branch(&floor, &wall(y, (-1.0, 1.0)), (0.0, 1.0));
    }
}

/// The floor's edge from y = 1 to 2: its room past each end is a sixteenth of its length. Each wall
/// here starts at 15/16 or 31/32 and reaches past that by a sixteenth of its own width, so it holds
/// the edge continued a sixteenth below y = 1 but not three sixteenths: a run proven over more room
/// than the edge's is not proven at all. Each of these was left flagged as the issue's are.
#[test]
fn the_room_a_run_is_proven_over_is_a_sixteenth_of_the_edge_past_each_end() {
    let floor = floor_plane((0.0, 2.0), (1.0, 2.0));
    for y in [(0.9375, 2.5), (0.96875, 2.25), (0.9375, 2.125)] {
        one_branch(&floor, &wall(y, (-1.0, 1.0)), (1.0, 2.0));
    }
}
