//! What the subdivision of two surfaces decides at its limits, each held to its exact answer (12.8):
//! patches of one carrier that wrap or touch a turn round, a patch one double wide that no cut can
//! halve, start points on the end of a cone's height (over part of a turn and over a whole one),
//! a start point on the end of a sphere's longitude, where a sphere has no room past its end, and
//! an edge a hair inside that end, which is not taken as a start on it, and an edge that meets
//! both ends of a sphere's longitude in one pair.
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
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind};

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
/// the half-turns `turns` and v from 0 to one unit; and the plane x = 1.1 units over y from −2 to
/// 2 units and z from 0 to one unit.
fn cone_and_wall(turns: (f64, f64)) -> (SurfaceKind, SurfaceKind) {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let cone =
        SurfaceKind::Cone(Cone::new([0.0; 3], UNIT, 0.5, frame, (turns, (0.0, UNIT))).unwrap());
    let wall = SurfaceKind::Plane(
        Plane::new(
            [1.1 * UNIT, 0.0, 0.0],
            [0.0, UNIT, 0.0],
            [0.0, 0.0, UNIT],
            ((-2.0, 2.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    (cone, wall)
}

/// A sphere of radius one unit about the origin, its longitude u over the half-turns `longitude`
/// (counted from x towards y) and its latitude v from −0.3 to 0.4; and the plane z = 1/2 unit over
/// x from −1 to 1.5 units and y over `across` units.
fn sphere_and_lid(longitude: (f64, f64), across: (f64, f64)) -> (SurfaceKind, SurfaceKind) {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let sphere =
        SurfaceKind::Sphere(Sphere::new([0.0; 3], UNIT, frame, (longitude, (-0.3, 0.4))).unwrap());
    let lid = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 0.5 * UNIT],
            [UNIT, 0.0, 0.0],
            [0.0, UNIT, 0.0],
            ((-1.0, 1.5), across),
        )
        .unwrap(),
    );
    (sphere, lid)
}

/// A sphere of radius 5/4 unit about the origin, its longitude u from 0 to 3/4 of a half-turn and
/// its latitude v from −0.3 to 0.4; and the plane z = 3/4 unit swept by x and by x + y, over u from
/// −1.5 to 1 and v from −0.5 to 1.5. They meet in the circle of radius one unit, from where the
/// plane's edge u = 1, the line x − y = 1 unit, crosses the sphere's end u = 0 at (1, 0, 3/4) units,
/// exactly on the sphere (3, 4, 5), round to the sphere's end u = 3/4 inside the plane. The edge
/// crosses that end at an angle, so each two of its three rows there determine the zero.
fn slanted() -> (SurfaceKind, SurfaceKind) {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let sphere = SurfaceKind::Sphere(
        Sphere::new([0.0; 3], 1.25 * UNIT, frame, ((0.0, 0.75), (-0.3, 0.4))).unwrap(),
    );
    let lid = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 0.75 * UNIT],
            [UNIT, 0.0, 0.0],
            [UNIT, UNIT, 0.0],
            ((-1.5, 1.0), (-0.5, 1.5)),
        )
        .unwrap(),
    );
    (sphere, lid)
}

/// The cone and plane of `cone_and_wall`, the cone over the eighth of a turn either side of x (its
/// turns counted from 10 half-turns on). They meet in one curve, from its lowest point on x up both
/// arms to the top, where the plane's edge z = 1 unit meets the cone on the end of its height.
/// Those two start points stand on the end of the cone's domain, so they are proven in the room
/// past it, and the pair is settled with nothing flagged, in both orders.
#[test]
fn a_plane_meets_a_cone_on_the_end_of_its_height() {
    let (cone, wall) = cone_and_wall((9.75, 10.25));
    for (a, b) in [(&cone, &wall), (&wall, &cone)] {
        let found = subdivide(a, b, FLOOR, &mut meter(1 << 24)).unwrap();
        assert!(found.flagged.is_empty(), "{found:#?}");
        assert!(!found.settled.is_empty(), "{found:#?}");
    }
}

/// The cone and plane above, the cone now over a whole turn, from 0.25 half-turns to 2.25. The arm
/// at +y meets the plane's top edge at about 2.238 half-turns, in the patch that ends the cone's
/// turn: that patch has room past the end of the cone's height but none round its turn, which is
/// already whole, and the start point there is proven in the room as before. The pair is settled
/// with nothing flagged, in both orders.
#[test]
fn a_plane_meets_a_whole_turn_of_cone_on_the_end_of_its_height() {
    let (cone, wall) = cone_and_wall((0.25, 2.25));
    for (a, b) in [(&cone, &wall), (&wall, &cone)] {
        let found = subdivide(a, b, FLOOR, &mut meter(1 << 24)).unwrap();
        assert!(found.flagged.is_empty(), "{found:#?}");
        assert!(!found.settled.is_empty(), "{found:#?}");
    }
}

/// The sphere and plane of `sphere_and_lid`, the sphere's longitude from 0 to 3/4 of a half-turn
/// and the plane's y from 0 to one unit. They meet in the circle of radius √3/2 at
/// latitude 1/6, from where the plane's edge y = 0 crosses the sphere's end u = 0, at
/// (√3/2, 0, 1/2) units, round to the sphere's end u = 3/4 inside the plane. The first of those
/// two start points stands exactly on the end of the sphere's domain, where a sphere has no room
/// past it: the plane edge's system is solved on that end, proven there, and the pair is settled
/// with nothing flagged, in both orders, with a start holding the plane's (√3/2, 0) and the
/// sphere's (0, 1/6).
#[test]
fn a_plane_edge_meets_a_sphere_on_the_end_of_its_longitude() {
    let (sphere, lid) = sphere_and_lid((0.0, 0.75), (0.0, 1.0));
    // The start on both ends, as (the plane's x, y, the sphere's u, v), each the nearest double.
    let (x, v) = (0.75_f64.sqrt(), 1.0 / 6.0);
    for (swapped, (a, b)) in [(false, (&lid, &sphere)), (true, (&sphere, &lid))] {
        let found = subdivide(a, b, FLOOR, &mut meter(1 << 24)).unwrap();
        assert!(found.flagged.is_empty(), "{found:#?}");
        let starts: Vec<_> = found
            .settled
            .iter()
            .flat_map(|p| p.starts.clone())
            .collect();
        assert_eq!(starts.len() % 2, 0, "{found:#?}");
        let want = if swapped {
            [0.0, v, x, 0.0]
        } else {
            [x, 0.0, 0.0, v]
        };
        let holds = |s: &[_; 4]| {
            s.iter()
                .zip(want)
                .all(|(i, w): (&Interval, f64)| i.lo() <= w && w <= i.hi())
        };
        assert!(starts.iter().any(holds), "{found:#?}");
    }
}

/// The work a run charges is part of its answer, since a budget ends a run. So the start points on
/// the ends of domains above, on the end of the cone's height (over part of a turn and over a whole
/// one) and on the end of the sphere's longitude, are each held to the exact work their subdivision
/// charges, as recorded, with the settled pairs, start points and flagged pairs it gives, in both
/// orders. Two more of the sphere: the first case mirrored in y = 0, so the start stands on the
/// high end of the sphere's longitude (u = 0, from −3/4) and of the plane's y (0, from −1), and the
/// sphere over the half-turn from 0 to 1 with the plane's y from 0 to one unit, where the plane's
/// edge y = 0 meets the sphere on both ends of its longitude, at x = ±√3/2 units. On a sphere's end
/// the edge's system is searched only on an end its flagged boxes stand on and only near them, a
/// box whose range leaves out zero is dropped before the operator is charged for it, and a zero
/// replaces only the flagged boxes it meets: each end's own start is found on that end.
#[test]
fn the_ends_of_domains_charge_their_recorded_work() {
    let (cone, wall) = cone_and_wall((9.75, 10.25));
    let (whole, round) = cone_and_wall((0.25, 2.25));
    let (sphere, lid) = sphere_and_lid((0.0, 0.75), (0.0, 1.0));
    let (mirrored, under) = sphere_and_lid((-0.75, 0.0), (-1.0, 0.0));
    let (half, over) = sphere_and_lid((0.0, 1.0), (0.0, 1.0));
    let (ball, slant) = slanted();
    let runs = [
        (&cone, &wall),
        (&wall, &cone),
        (&whole, &round),
        (&round, &whole),
        (&lid, &sphere),
        (&sphere, &lid),
        (&under, &mirrored),
        (&mirrored, &under),
        (&over, &half),
        (&half, &over),
        (&slant, &ball),
        (&ball, &slant),
    ];
    let seen: Vec<_> = runs
        .into_iter()
        .map(|(a, b)| {
            let mut m = meter(1 << 24);
            let found = subdivide(a, b, FLOOR, &mut m).unwrap();
            let starts: usize = found.settled.iter().map(|p| p.starts.len()).sum();
            (
                m.used().work,
                found.settled.len(),
                starts,
                found.flagged.len(),
            )
        })
        .collect();
    // (work, settled pairs, start points, flagged pairs), in the order of `runs`.
    let want = [
        (2439, 1, 2, 0),
        (2439, 1, 2, 0),
        (7138, 1, 2, 0),
        (7138, 1, 2, 0),
        (7756, 3, 6, 0),
        (10817, 4, 8, 0),
        (12414, 5, 10, 0),
        (10253, 4, 8, 0),
        (12920, 4, 8, 0),
        (12051, 4, 8, 0),
        (44465, 6, 12, 0),
        (33907, 6, 12, 0),
    ];
    assert_eq!(seen, want);
}

/// The sphere and plane of `sphere_and_lid`, the sphere's longitude from 0 to 3/4 of a half-turn
/// and the plane's y from 2^−70 of a unit to one unit, so its edge runs a hair inside the sphere's
/// end u = 0, far below a step of the corpus. The interior system can neither prove nor exclude that
/// meeting so near the end, and on the end itself two of the edge's rows have a zero whose third,
/// the edge's y less the end's, is the hair and holds no zero: it is not taken as a start on the
/// end, and the pair there is left flagged as undecided at the floor, in both orders, with the work
/// its search charges, as recorded.
#[test]
fn an_edge_a_hair_inside_the_end_of_a_spheres_longitude_is_left_flagged() {
    let (sphere, lid) = sphere_and_lid((0.0, 0.75), (2.0_f64.powi(-70), 1.0));
    let seen: Vec<_> = [(&lid, &sphere), (&sphere, &lid)]
        .into_iter()
        .map(|(a, b)| {
            let mut m = meter(1 << 24);
            let found = subdivide(a, b, FLOOR, &mut m).unwrap();
            let starts: usize = found.settled.iter().map(|p| p.starts.len()).sum();
            let whys: Vec<_> = found.flagged.iter().map(|f| f.why).collect();
            (m.used().work, found.settled.len(), starts, whys)
        })
        .collect();
    let floor = Unresolved::UndecidedAtFloor {
        test: Test::Exclusion,
    };
    // (work, settled pairs, start points, why each pair is flagged), in the order above.
    let want = [(7363, 2, 4, vec![floor]), (11468, 3, 6, vec![floor])];
    assert_eq!(seen, want);
}

/// A sphere of radius 5/4 unit about the origin, its longitude u from 0 to 0.2951672353008666, the
/// half-turns of atan2(4/5, 3/5) to within a few doubles, and its latitude v from −0.3 to 0.4; and
/// the plane z = 3/4 unit, its edge v = 0 the line 2x + y = 1 unit, a chord of the circle of radius
/// one unit where they meet, over u from −0.5 to 1.5 and v from 0 to 1, swept outward. The chord
/// meets the sphere's end u = 0 exactly at (1, 0, 3/4) units and, within rounding, its other end at
/// (0.6, 0.8, 3/4): one patch pair, whose edge system has flagged boxes on both ends at the same
/// latitude. The zero proven on one end replaces only the flagged boxes it meets, so the other end
/// is solved too; the pair is settled with its two start points and nothing flagged, in both
/// orders, with the work its search charges, as recorded.
#[test]
fn a_chord_meets_both_ends_of_a_spheres_longitude_in_one_pair() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let end = 0.295_167_235_300_866_6;
    let ball = SurfaceKind::Sphere(
        Sphere::new([0.0; 3], 1.25 * UNIT, frame, ((0.0, end), (-0.3, 0.4))).unwrap(),
    );
    let chord = SurfaceKind::Plane(
        Plane::new(
            [UNIT, 0.0, 0.75 * UNIT],
            [-0.5 * UNIT, UNIT, 0.0],
            [0.5 * UNIT, 0.25 * UNIT, 0.0],
            ((-0.5, 1.5), (0.0, 1.0)),
        )
        .unwrap(),
    );
    let seen: Vec<_> = [(&chord, &ball), (&ball, &chord)]
        .into_iter()
        .map(|(a, b)| {
            let mut m = meter(1 << 24);
            let found = subdivide(a, b, FLOOR, &mut m).unwrap();
            let starts: usize = found.settled.iter().map(|p| p.starts.len()).sum();
            (
                m.used().work,
                found.settled.len(),
                starts,
                found.flagged.len(),
            )
        })
        .collect();
    // (work, settled pairs, start points, flagged pairs), in the order above.
    assert_eq!(seen, [(7762, 1, 2, 0), (7762, 1, 2, 0)]);
}
