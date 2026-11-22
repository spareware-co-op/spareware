//! What the subdivision of two surfaces and its march decide at four more limits, each held to its
//! exact answer (12.8, 12.10, 12.11): a wall whose profile is trimmed at a crease of its basis,
//! read at that end of its domain from the inside, at the end after it and at the end before it; a
//! flag near a known curve that a lone pole also lies in, which says the known curve; and a patch
//! one double wide where the cut a share across rounds, by a tie, onto its far end, which no cut
//! can halve.
//!
//! Run at the scale of the SSI corpus (1024 steps to the unit, floor 8, ceiling 10.24) where a
//! case has a scale, and in both orders of the pair.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{Basis as CurveBasis, Frame, Trimmed};
use geom_intersect::general::subdivide;
use geom_intersect::known::{Known, KnownContact, subdivide_known};
use geom_intersect::march::{Branch, march};
use geom_intersect::result::{CurveEnd, Degeneracy, Test, Unresolved};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Curve};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Extrusion, Plane, SurfaceKind};

const UNIT: f64 = 1024.0;
const FLOOR: f64 = 8.0;
const CEILING: f64 = 10.24;

fn meter(work: u64) -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(work))
}

/// The middle of an interval.
fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// Whether `x` holds `at` to within the floor, in parameters of a unit's speed: a vertex a step
/// puts down is an enclosure of a point on the branch.
fn near(x: Interval, at: f64) -> bool {
    let room = 4.0 * FLOOR / UNIT;
    x.lo() - room <= at && at <= x.hi() + room
}

/// A wall over the degree-1 profile through (−1, 0, 0), (0, 0, 0) and (0, 1, 0) units, knots
/// 0, 0, 1/2, 1, 1, so its parameter turns a right angle at the inner knot 1/2: it runs along x
/// before it, at a speed of two units in +x, and along y after it, at two units in +y. The profile
/// is trimmed to `span`, which ends or begins at that knot, and swept one unit up z (v from 0 to
/// 1).
fn wall(span: (f64, f64)) -> SurfaceKind {
    let basis = Basis::new(1, vec![0.0, 0.0, 0.5, 1.0, 1.0]).unwrap();
    let net = [[-UNIT, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, UNIT, 0.0]];
    let profile = Trimmed::new(CurveBasis::Nurbs(Curve::new(basis, &net).unwrap()), span).unwrap();
    SurfaceKind::Extrusion(Extrusion::new(profile, [0.0, 0.0, UNIT], (0.0, 1.0)).unwrap())
}

/// The one branch of the pair: nothing flagged, nothing left unproven, an edge of a domain at both
/// ends.
fn traced(a: &SurfaceKind, b: &SurfaceKind) -> Branch {
    let found = subdivide(a, b, FLOOR, &mut meter(1 << 24)).unwrap();
    assert!(found.flagged.is_empty(), "{:#?}", found.flagged);
    let traced = march(a, b, &found, FLOOR, CEILING, &mut meter(1 << 24)).unwrap();
    assert!(traced.unproven.is_empty(), "{:#?}", traced.unproven);
    let [branch] = traced.branches.as_slice() else {
        panic!("{:#?}", traced.branches);
    };
    assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
    branch.clone()
}

/// Holds `branch`, (u, v) of the wall first when `wall_first`, to the segment `on` names: each
/// vertex's (u, v, s, t), wall then plane, is `on(u)`, and the branch runs from the wall's `from`
/// to its `to`, both read in u.
fn on_segment(branch: &Branch, wall_first: bool, on: impl Fn(f64) -> [f64; 4], ends: [f64; 2]) {
    let order = if wall_first {
        [0, 1, 2, 3]
    } else {
        [2, 3, 0, 1]
    };
    for v in &branch.vertices {
        let want = on(mid(v[order[0]]));
        for (k, w) in order.iter().zip(want) {
            assert!(near(v[*k], w), "{v:?} {want:?}");
        }
    }
    let first = mid(branch.vertices[0][order[0]]);
    let last = mid(branch.vertices[branch.vertices.len() - 1][order[0]]);
    let mut seen = [first, last];
    seen.sort_by(f64::total_cmp);
    for (s, e) in seen.iter().zip(ends) {
        assert!((s - e).abs() <= 4.0 * FLOOR / UNIT, "{seen:?} {ends:?}");
    }
}

/// The wall trimmed to [0, 1/2], so its domain ends on the crease: the plane x = 0 to −1 unit,
/// y = 0, with z rising as x. The plane z = x + 1/2 unit meets it in the segment from
/// (−1/2, 0, 0) units, on the wall's foot at u = 1/4, to (0, 0, 1/2) units, on its end u = 1/2 at
/// half its height: on the wall v = 2u − 1/2, and on the plane, swept by (1, 0, 1) and (0, 1, 0)
/// units, s = 2u − 1 and t = 0. A start stands on the end u = 1/2, where the wall's partials are
/// read: only the limit from the left is the wall's own (the knot's right holds the profile's turn
/// along y, not the wall), and from there the march traces the one branch to the foot, every
/// vertex on the segment, in both orders.
#[test]
fn a_wall_is_read_from_inside_on_the_end_its_profile_creases_at() {
    let left = wall((0.0, 0.5));
    let rising = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 0.5 * UNIT],
            [UNIT, 0.0, UNIT],
            [0.0, UNIT, 0.0],
            ((-1.5, 1.0), (-1.0, 1.0)),
        )
        .unwrap(),
    );
    let on = |u: f64| [u, 2.0 * u - 0.5, 2.0 * u - 1.0, 0.0];
    on_segment(&traced(&left, &rising), true, on, [0.25, 0.5]);
    on_segment(&traced(&rising, &left), false, on, [0.25, 0.5]);
}

/// The wall trimmed to [1/2, 1], so its domain begins on the crease: the plane x = 0, y = 0 to 1
/// unit. The plane z = y + 1/2 unit meets it in the segment from (0, 0, 1/2) units, on its end
/// u = 1/2 at half its height, to (0, 1/2, 1) units, on its top at u = 3/4: on the wall
/// v = 2u − 1/2, and on the plane, swept by (1, 0, 0) and (0, 1, 1) units, s = 0 and t = 2u − 1.
/// A start stands on the end u = 1/2, where only the limit from the right is the wall's own, and
/// the march traces the one branch to the top, every vertex on the segment, in both orders.
#[test]
fn a_wall_is_read_from_inside_on_the_end_its_profile_creases_before() {
    let right = wall((0.5, 1.0));
    let rising = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 0.5 * UNIT],
            [UNIT, 0.0, 0.0],
            [0.0, UNIT, UNIT],
            ((-1.0, 1.0), (-1.5, 1.0)),
        )
        .unwrap(),
    );
    let on = |u: f64| [u, 2.0 * u - 0.5, 0.0, 2.0 * u - 1.0];
    on_segment(&traced(&right, &rising), true, on, [0.5, 0.75]);
    on_segment(&traced(&rising, &right), false, on, [0.5, 0.75]);
}

/// The plane z = x/4 through the apex of a cone whose generators rise at 45° (attack G173's case):
/// it meets the cone at the apex alone, a pole with no branch out of it, flagged as one at the
/// floor. Given as known a crossing curve along the apex (every turn at height 0 on the cone, the
/// origin on the plane), whose stretch no cell can prove, since no branch runs there, every pair
/// the search flags lies near that unproven stretch, so each says the known curve, as 12.11 asks,
/// and none the pole; without it each says the pole.
#[test]
fn a_flag_near_an_unproven_known_curve_says_so_at_a_lone_pole() {
    let frame = Frame::standard().unwrap();
    let cone =
        SurfaceKind::Cone(Cone::new([0.0; 3], 0.0, 1.0, frame, ((0.0, 2.0), (0.0, 4.0))).unwrap());
    let shallow = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.25],
            [0.0, 1.0, 0.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        )
        .unwrap(),
    );
    let p = |x: f64| Interval::point(x).unwrap();
    let along_apex = Known {
        vertices: vec![
            [p(0.0), p(0.0), p(0.0), p(0.0)],
            [p(2.0), p(0.0), p(0.0), p(0.0)],
        ],
        bound: 0.01,
        contact: KnownContact::Crossing,
    };
    let pole = Unresolved::NonSmooth {
        at: Degeneracy::Pole,
    };
    for (known, why) in [
        (vec![], pole),
        (vec![along_apex], Unresolved::KnownCurveUnproven),
    ] {
        let found = subdivide_known(&cone, &shallow, &known, 1e-3, &mut meter(1 << 22)).unwrap();
        assert!(found.settled.is_empty(), "{found:?}");
        assert!(!found.flagged.is_empty(), "{found:?}");
        for pair in &found.flagged {
            assert_eq!(pair.why, why, "{found:?}");
        }
    }
}

/// Two parameterisations of the plane z = 0, the second with its vectors swapped, over the one
/// square a double wide each way at 2^−1021 + 2^−1073, an odd double whose next, 2^−1021 + 2^−1072,
/// is even; floor 2^−1074. Their normals are opposite, so the pair is never settled, and the patch
/// is wider than the floor. The cut 7/16 of the way across is that double plus 7/16 of 2^−1073,
/// which rounds to 2^−1074, half the width: the sum is a tie, rounded to the even far end, so no
/// cut falls strictly inside, and the pair is flagged whole, at once, as undecided at the floor, in
/// both orders.
#[test]
fn a_patch_whose_cut_rounds_onto_its_end_is_flagged_whole() {
    let at = f64::from_bits(0x0020_0000_0000_0001);
    let next = f64::from_bits(0x0020_0000_0000_0002);
    let domain = ((at, next), (at, next));
    let first =
        SurfaceKind::Plane(Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], domain).unwrap());
    let second =
        SurfaceKind::Plane(Plane::new([0.0; 3], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0], domain).unwrap());
    for (a, b) in [(&first, &second), (&second, &first)] {
        let found = subdivide(a, b, f64::from_bits(1), &mut meter(1 << 16)).unwrap();
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
