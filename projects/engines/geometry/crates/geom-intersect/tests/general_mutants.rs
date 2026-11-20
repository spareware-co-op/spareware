//! What the subdivision of two surfaces decides at its limits, each held to its exact answer (12.8):
//! patches of one carrier that wrap or touch a turn round, a patch one double wide that no cut can
//! halve, and start points on the end of a cone's height.
//!
//! Run at the scale of the SSI corpus (1024 steps to the unit, floor 8) where a case has a scale,
//! and in both orders of the pair.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::IntersectError;
use geom_intersect::general::{Subdivided, subdivide};
use geom_intersect::result::{StepsInterval, Test, Unresolved};
use geom_num::grid::Steps;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, SurfaceKind};

const UNIT: f64 = 1024.0;
const FLOOR: f64 = 8.0;

type Box2 = ((f64, f64), (f64, f64));

fn meter(work: u64) -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(work))
}

fn cylinder(domain: Box2) -> SurfaceKind {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], UNIT, frame, domain).unwrap())
}

/// The region `subdivide` returns as one carrier's for the pair, or `None` if it did not take the
/// pair as one. A meter of 1 is too small for any search: the shortcut costs nothing, a search is
/// refused.
fn shortcut(a: &SurfaceKind, b: &SurfaceKind) -> Option<Box2> {
    let zero = StepsInterval::new(Steps::ZERO, Steps::ZERO).unwrap();
    match subdivide(a, b, FLOOR, &mut meter(1)) {
        Ok(Subdivided { settled, flagged }) => match flagged.as_slice() {
            [pair]
                if settled.is_empty()
                    && pair.why == (Unresolved::NearCoincident { gap: zero })
                    && pair.first == pair.second =>
            {
                Some(pair.first)
            }
            _ => None,
        },
        Err(IntersectError::BudgetExceeded) => None,
        Err(e) => panic!("{e:?}"),
    }
}

/// One cylinder's turns from 0 to 1.5 and from −1 to 0.5 share [0, 0.5], and, a turn round, the
/// second's [−1, 0] is [1, 2], which shares [1, 1.5] with the first: two regions, so the pair is
/// not one carrier's region and goes the ordinary way, in both orders. The shifted copy that meets
/// the first is the one a turn up, so the count of copies has to reach it.
#[test]
fn turns_that_share_twice_a_turn_round_are_not_one_region() {
    let first = cylinder(((0.0, 1.5), (0.0, UNIT)));
    let second = cylinder(((-1.0, 0.5), (0.0, UNIT)));
    assert_eq!(shortcut(&first, &second), None);
    assert_eq!(shortcut(&second, &first), None);
}

/// Turns from 0 to 1.5 and from −0.5 to 0.5 share [0, 0.5]; a turn round, the second is [1.5, 2.5],
/// which only touches the first at 1.5. One region, so the pair is one carrier's over it, in both
/// orders.
#[test]
fn turns_that_touch_a_turn_round_share_one_region() {
    let first = cylinder(((0.0, 1.5), (0.0, UNIT)));
    let second = cylinder(((-0.5, 0.5), (0.0, UNIT)));
    let shared = Some(((0.0, 0.5), (0.0, UNIT)));
    assert_eq!(shortcut(&first, &second), shared);
    assert_eq!(shortcut(&second, &first), shared);
}

/// Two parameterisations of the plane z = 0, the second with its vectors swapped, over the one
/// square a double wide each way at 2^52, where the doubles are a whole number apart. Their
/// normals are opposite, so the pair is never settled; the patch is wider in space than the floor,
/// but no cut falls strictly inside it, so it is flagged whole, at once, as undecided at the floor.
#[test]
fn a_patch_one_double_wide_is_flagged_whole() {
    let at = 4_503_599_627_370_496.0; // 2^52
    let domain = ((at, at + 1.0), (at, at + 1.0));
    let first =
        SurfaceKind::Plane(Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], domain).unwrap());
    let second =
        SurfaceKind::Plane(Plane::new([0.0; 3], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0], domain).unwrap());
    for (a, b) in [(&first, &second), (&second, &first)] {
        let found = subdivide(a, b, 0.5, &mut meter(1 << 16)).unwrap();
        assert!(found.settled.is_empty(), "{found:#?}");
        let [pair] = found.flagged.as_slice() else {
            panic!("{found:#?}");
        };
        assert_eq!((pair.first, pair.second), (domain, domain));
        assert_eq!(
            pair.why,
            Unresolved::UndecidedAtFloor {
                test: Test::LoopFreedom
            }
        );
    }
}

/// A cone about z of radius one unit at v = 0 widening by half a unit to the unit of height, over
/// the eighth of a turn either side of x (its turns counted from 10 half-turns on) and v from 0 to
/// one unit; and the plane x = 1.1 units over y from −2 to 2 units and z from 0 to one unit. They
/// meet in one curve, from its lowest point on x up both arms to the top, where the plane's edge
/// z = 1 unit meets the cone on the end of its height. Those two start points stand on the end of
/// the cone's domain, so they are proven in the room past it, and the pair is settled with nothing
/// flagged, in both orders.
#[test]
fn a_plane_meets_a_cone_on_the_end_of_its_height() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let cone = SurfaceKind::Cone(
        Cone::new([0.0; 3], UNIT, 0.5, frame, ((9.75, 10.25), (0.0, UNIT))).unwrap(),
    );
    let wall = SurfaceKind::Plane(
        Plane::new(
            [1.1 * UNIT, 0.0, 0.0],
            [0.0, UNIT, 0.0],
            [0.0, 0.0, UNIT],
            ((-2.0, 2.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    for (a, b) in [(&cone, &wall), (&wall, &cone)] {
        let found = subdivide(a, b, FLOOR, &mut meter(1 << 24)).unwrap();
        assert!(found.flagged.is_empty(), "{found:#?}");
        assert!(!found.settled.is_empty(), "{found:#?}");
    }
}
