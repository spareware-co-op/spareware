//! The march's pieces along an edge, and how it joins them, held to what each case is known to give
//! (G170, chapter 12.8 steps 6 to 8): the arc of a cylinder's end circle lying in a plane, in one
//! settled pair or several. A piece along an edge goes from one start to the nearest other start on
//! that edge, so the start it goes to, the order of the cuts between them and which end a branch of
//! one piece begins from all show in the branch's vertices, read in order along the edge, and in the
//! work the march charges.
//!
//! Run at the scale of the SSI corpus (1024 steps to the unit, floor 8, ceiling 10.24), in both
//! orders of the pair. The spans of the cylinder's turn are chosen so that a start stands at a turn
//! of exactly zero, or two starts at turns of opposite sign: a gap between starts measured by a
//! quotient or a sum then comes to zero, and no piece is found.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_curve::Frame;
use geom_intersect::general::subdivide;
use geom_intersect::march::{Branch, march};
use geom_intersect::result::CurveEnd;
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, Plane, SurfaceKind};

const UNIT: f64 = 1024.0;
const FLOOR: f64 = 8.0;
const CEILING: f64 = 10.24;

/// The middle of an interval.
fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// Whether `x` holds `at` to within the floor, in the plane's parameters: a vertex a step puts
/// down is an enclosure of a point on the branch, read at the middle of its turn.
fn near(x: Interval, at: f64) -> bool {
    let room = 4.0 * FLOOR / UNIT;
    x.lo() - room <= at && at <= x.hi() + room
}

/// The plane z = 0, in units.
fn ground() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [UNIT, 0.0, 0.0],
            [0.0, UNIT, 0.0],
            ((-2.0, 2.0), (-2.0, 2.0)),
        )
        .unwrap(),
    )
}

/// The cylinder of radius one unit about z over turns `lo` to `hi` (in half-turns): its edge v = 0
/// is an arc of the circle that lies in [`ground`].
fn tube(lo: f64, hi: f64) -> SurfaceKind {
    SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            UNIT,
            Frame::standard().unwrap(),
            ((lo, hi), (0.0, UNIT)),
        )
        .unwrap(),
    )
}

/// The one branch of the pair, and the work its march charged: nothing flagged, nothing left
/// unproven, an edge of a domain at both ends.
fn traced(a: &SurfaceKind, b: &SurfaceKind) -> (Branch, u128) {
    let found = subdivide(a, b, FLOOR, &mut WorkMeter::new(Budget::of_work(1 << 24))).unwrap();
    assert!(found.flagged.is_empty(), "{:#?}", found.flagged);
    let mut meter = WorkMeter::new(Budget::of_work(1 << 24));
    let traced = march(a, b, &found, FLOOR, CEILING, &mut meter).unwrap();
    assert!(traced.unproven.is_empty(), "{:#?}", traced.unproven);
    let [branch] = traced.branches.as_slice() else {
        panic!("{:#?}", traced.branches);
    };
    assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
    (branch.clone(), meter.used().work)
}

/// One case's answer: the span of turn, whether the cylinder is first, the turn the branch begins
/// at, its number of vertices, and the march's work.
type Case = ((f64, f64), bool, f64, usize, u128);

/// Each case's answer, recorded.
const RECORDED: [Case; 6] = [
    ((-0.25, 0.25), true, -0.25, 17, 2067),
    ((-0.25, 0.25), false, -0.25, 17, 2067),
    ((-0.5, 0.0), true, -0.5, 21, 2509),
    ((-0.5, 0.0), false, -0.5, 25, 3039),
    ((0.0, 0.5), true, 0.5, 23, 2748),
    ((0.0, 0.5), false, 0.5, 22, 2655),
];

/// The arc in the plane, over a span of turn about zero (one settled pair), one ending at zero and
/// one beginning there (several pairs), in both orders. Every vertex is on the circle, the
/// vertices run one way along the edge from the end the march began at, with no cut out of its
/// place, and the number of them and the work are as recorded.
#[test]
fn an_arc_along_an_edge_runs_one_way_from_its_first_start() {
    let plane = ground();
    let mut seen = Vec::new();
    for ((lo, hi), tube_first, _, _, _) in RECORDED {
        let cylinder = tube(lo, hi);
        let (branch, work, [t, height, x, y]) = if tube_first {
            let (b, w) = traced(&cylinder, &plane);
            (b, w, [0, 1, 2, 3])
        } else {
            let (b, w) = traced(&plane, &cylinder);
            (b, w, [2, 3, 0, 1])
        };
        assert!(branch.deviation <= CEILING, "{}", branch.deviation);
        for v in &branch.vertices {
            let angle = core::f64::consts::PI * mid(v[t]);
            assert!(near(v[height], 0.0), "{v:?}");
            assert!(near(v[x], angle.cos()) && near(v[y], angle.sin()), "{v:?}");
        }
        let places: Vec<f64> = branch.vertices.iter().map(|v| mid(v[t])).collect();
        let first = places[0];
        let rising = first < places[places.len() - 1];
        for pair in places.windows(2) {
            assert!(
                if rising {
                    pair[0] < pair[1]
                } else {
                    pair[0] > pair[1]
                },
                "({lo}, {hi}) {tube_first}: {places:?}"
            );
        }
        assert!(first == lo || first == hi, "{places:?}");
        seen.push(((lo, hi), tube_first, first, places.len(), work));
    }
    assert_eq!(seen, RECORDED, "{seen:?}");
}
