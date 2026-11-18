//! A line through a cone's apex, and a line tangent to a cylinder or a cone off the grid (decision
//! 32.87, issue 995, groups 1a and 1b). Where the enclosure of the discriminant reaches zero, its
//! sign is decided exactly: on whole-number data by the exact closed forms, and on any stored
//! doubles by a sum of products of six (a cylinder) or eight (a cone whose radius at v = 0 is zero)
//! of them. A line through the apex, or tangent, is one point; one a step inside is two; one a
//! step outside is none. Before, the engine bisected round the touching point to the floor: one
//! flagged item for 0.48 to 4.15 million units.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly, and the names are a point's coordinates"
)]

use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_intersect::IntersectError;
use geom_intersect::curve::curves;
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::result::{Intersection, Item, PointContact};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Sphere, SurfaceKind};

/// Far past what a closed form spends, and far short of the engine's half a million units and more
/// on these pairs: a pair the closed form leaves stops here instead of running on.
const BUDGET: u64 = 100_000;

/// The frame on an axis none of whose coordinates is zero, (1, 2, 2), three long.
const SLANT: ([f64; 3], [f64; 3]) = ([1.0, 0.0, 0.0], [1.0, 2.0, 2.0]);

/// The frame on the z axis.
const ON_Z: ([f64; 3], [f64; 3]) = ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);

/// The natural target's frame off the grid: its axis is the doubles nearest 0.6 and 0.8.
const OFF: ([f64; 3], [f64; 3]) = ([0.0, 0.0, 1.0], [0.6, 0.8, 0.0]);

/// A frame by its direction in the plane and its axis.
fn frame((x, n): ([f64; 3], [f64; 3])) -> Frame<3> {
    Frame::new(x, n).unwrap()
}

/// The cone about `c` of radius `r0` at v = 0 and slope `k`, a whole turn and 300 either way.
fn cone(c: [f64; 3], r0: f64, k: f64, f: ([f64; 3], [f64; 3])) -> SurfaceKind {
    SurfaceKind::Cone(Cone::new(c, r0, k, frame(f), ((0.0, 2.0), (-300.0, 300.0))).unwrap())
}

/// The cylinder about `c` of radius `r`, a whole turn and 300 either way.
fn cylinder(c: [f64; 3], r: f64, f: ([f64; 3], [f64; 3])) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new(c, r, frame(f), ((0.0, 2.0), (-300.0, 300.0))).unwrap())
}

/// The line `o + t·d`, t in [−300, 300], as a plane's edge is.
fn line(o: [f64; 3], d: [f64; 3]) -> CurveKind<3> {
    CurveKind::Line(Line::new(o, d, (-300.0, 300.0)).unwrap())
}

/// A line and the surface it meets.
type Pair = (CurveKind<3>, SurfaceKind);

/// A pair made from one size: a radius or a slope.
type Maker = fn(f64) -> Pair;

/// The answer and the work it took.
fn meet(c: &CurveKind<3>, s: &SurfaceKind) -> (Intersection, u128) {
    let mut meter = WorkMeter::new(Budget::of_work(BUDGET));
    let found = curve_surface(c, s, &mut meter).unwrap();
    (found, meter.used().work)
}

/// The enclosure of t of a point item.
fn t_of(item: &Item) -> (f64, f64) {
    let t = item.at().spans()[0];
    (t.lo(), t.hi())
}

/// The one point of `found`, a double root, whose t holds `t`.
fn one_touch(found: &Intersection, t: f64) {
    match found.items() {
        [
            item @ Item::Point {
                contact: PointContact::IsolatedTangent,
                ..
            },
        ] => {
            let (lo, hi) = t_of(item);
            assert!(lo <= t && t <= hi, "{lo} {hi} against {t}");
        }
        other => panic!("{other:?}"),
    }
}

/// Two crossings of `found`, apart, the first below `t` and the second above it.
fn two_crossings(found: &Intersection, t: f64) {
    match found.items() {
        [
            first @ Item::Point {
                contact: PointContact::Crossing,
                ..
            },
            second @ Item::Point {
                contact: PointContact::Crossing,
                ..
            },
        ] => {
            let (a, b) = (t_of(first), t_of(second));
            assert!(a.1 < b.0, "{a:?} {b:?}");
            assert!(a.0 <= t && t <= b.1, "{a:?} {b:?} against {t}");
        }
        other => panic!("{other:?}"),
    }
}

/// Issue 995's edges (the natural target's decoding of its inputs), each with the pair it meets,
/// where the line touches it, and the work one call takes now, 4 units each:
///
/// - group 1a: the edge (0, 0, 300) + t·(0, 1, 0) through the apex of a cone on the z axis
///   (inputs `1300004b04000000000000630100`, `0300004b04000000000000000100` and
///   `0300004b00000000000000000100`, 483,244 units before), and on the frame off the grid the
///   edges (0, 300, 0) + t·(1, 0, 0) and (0, −4, −4) + t·(0, 1, 0) through the apex at (12, 300, 0)
///   and (0, −300, −4) (`93034b000003` and `5700b5ffffff0000b5ffffff00`, 3,968,348 and 4,147,759);
/// - group 1b: the edge (0, 0, −300) + t·(0.8, −0.6, 0) tangent to a cylinder of radius 232 on
///   the frame off the grid, at t = 2.4 (`00000000ffffff2a00ffefe7033a`, 3,301,137).
fn the_issue_edges() -> [(CurveKind<3>, SurfaceKind, f64, u128); 4] {
    [
        (
            line([0.0, 0.0, 300.0], [0.0, 1.0, 0.0]),
            cone([0.0, 0.0, 300.0], 0.0, 0.5, ON_Z),
            0.0,
            4,
        ),
        (
            line([0.0, 300.0, 0.0], [1.0, 0.0, 0.0]),
            cone([12.0, 300.0, 0.0], 0.0, 0.5, OFF),
            12.0,
            4,
        ),
        (
            line([0.0, -4.0, -4.0], [0.0, 1.0, 0.0]),
            cone([0.0, -300.0, -4.0], 0.0, 0.5, OFF),
            -296.0,
            4,
        ),
        (
            line([0.0, 0.0, -300.0], [0.8, -0.6, 0.0]),
            cylinder([0.0, -4.0, -68.0], 232.0, OFF),
            2.4,
            4,
        ),
    ]
}

#[test]
fn the_issue_edges_are_one_point_each_in_closed_form() {
    let mut works = Vec::new();
    let mut held = Vec::new();
    for (edge, surface, t, work) in the_issue_edges() {
        let (found, used) = meet(&edge, &surface);
        one_touch(&found, t);
        works.push(used);
        held.push(work);
    }
    assert_eq!(works, held);
}

/// A cone's apex at halves, on the z axis, with slope 1.
const APEX: [f64; 3] = [0.5, 0.25, 0.125];

#[test]
fn a_line_through_an_apex_is_one_point_on_the_grid_and_off_it() {
    // On the grid, along each axis square to the cone's.
    let grid = cone([0.0, 0.0, 300.0], 0.0, 0.5, ON_Z);
    for d in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
        let (found, _) = meet(&line([0.0, 0.0, 300.0], d), &grid);
        one_touch(&found, 0.0);
    }
    // Off it: the apex at halves, the line crossing the axis there, steeper and shallower than
    // the cone.
    let halves = cone(APEX, 0.0, 1.0, ON_Z);
    let [x, y, z] = APEX;
    for d in [[1.0, 0.0, 0.0], [1.0, 0.5, 3.0], [0.0, 1.0, 0.25]] {
        let o = [x - 3.0 * d[0], y - 3.0 * d[1], z - 3.0 * d[2]];
        let (found, _) = meet(&line(o, d), &halves);
        one_touch(&found, 3.0);
    }
}

#[test]
fn a_line_a_step_beside_an_apex_misses_the_cone() {
    // In the plane square to the axis through the apex, which holds the apex alone: on the grid
    // one unit beside it, and off it one step of the doubles beside it.
    let grid = cone([0.0, 0.0, 300.0], 0.0, 0.5, ON_Z);
    let (found, _) = meet(&line([0.0, 1.0, 300.0], [1.0, 0.0, 0.0]), &grid);
    assert!(found.items().is_empty(), "{:?}", found.items());
    let halves = cone(APEX, 0.0, 1.0, ON_Z);
    let [x, y, z] = APEX;
    for beside in [y.next_up(), y.next_down()] {
        let (found, _) = meet(&line([x - 3.0, beside, z], [1.0, 0.0, 0.0]), &halves);
        assert!(found.items().is_empty(), "{:?}", found.items());
    }
}

/// A cylinder about the z axis through (0.5, 0.25) of radius 1.5, and the line along y that
/// touches it at x = 2, z = 7.5, at t = 3.
fn tangent_to_a_cylinder(r: f64) -> Pair {
    (
        line([2.0, 0.25 - 3.0, 7.5], [0.0, 1.0, 0.0]),
        cylinder([0.5, 0.25, 0.125], r, ON_Z),
    )
}

/// The cone of slope `k` about the z axis with its apex at [`APEX`], and the line along y in its
/// tangent plane x − 0.5 = z − 0.125 that touches its ruling (1, 0, 1) one unit up, at t = 3.
fn tangent_to_a_cone(k: f64) -> Pair {
    (
        line([1.5, 0.25 - 3.0, 1.125], [0.0, 1.0, 0.0]),
        cone(APEX, 0.0, k, ON_Z),
    )
}

/// The issue's cylinder input with its radius `r`: its edge touches at t = 2.4 for r = 232.
fn the_issue_cylinder(r: f64) -> Pair {
    (
        line([0.0, 0.0, -300.0], [0.8, -0.6, 0.0]),
        cylinder([0.0, -4.0, -68.0], r, OFF),
    )
}

#[test]
fn a_line_tangent_off_the_grid_is_one_exact_point() {
    for (edge, surface, t) in [
        {
            let (e, s) = tangent_to_a_cylinder(1.5);
            (e, s, 3.0)
        },
        {
            let (e, s) = tangent_to_a_cone(1.0);
            (e, s, 3.0)
        },
        {
            let (e, s) = the_issue_cylinder(232.0);
            (e, s, 2.4)
        },
        // On the slant axis, the line along (2, −2, 1), square to it, stands 4.5 from it, so
        // every product of the cross product d × n counts.
        (
            line([-8.5, 4.75, 0.125], [2.0, -2.0, 1.0]),
            cylinder(APEX, 4.5, SLANT),
            3.0,
        ),
        // The cone of slope 1 on the slant axis, and a line along (1, 1, −1) in the plane of
        // normal (1, 0, 1) through its apex, which touches it: no coordinate of the line or the
        // apex is zero but the line's offset in y.
        (
            line([1.5, 0.25, -0.875], [1.0, 1.0, -1.0]),
            cone(APEX, 0.0, 1.0, SLANT),
            -0.8,
        ),
    ] {
        let (found, _) = meet(&edge, &surface);
        one_touch(&found, t);
    }
}

#[test]
fn a_tangent_whose_data_pass_the_exact_range_is_the_engines() {
    // The cylinder's centre moved along its axis by 1e-30: the same cylinder, but a product of
    // eight such doubles could fall below what a double holds, so no exact sum is taken, and the
    // engine flags the touching point or stops on the budget.
    let (edge, _) = tangent_to_a_cylinder(1.5);
    let moved = cylinder([0.5, 0.25, 1e-30], 1.5, ON_Z);
    let mut meter = WorkMeter::new(Budget::of_work(BUDGET));
    match curve_surface(&edge, &moved, &mut meter) {
        Err(IntersectError::BudgetExceeded) => {}
        Ok(found) => assert!(
            found
                .items()
                .iter()
                .all(|i| matches!(i, Item::Flagged { .. })),
            "{:?}",
            found.items()
        ),
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn a_line_across_a_sphere_or_a_circle_is_two_points_in_closed_form() {
    // Through the centre along x: the sphere's quadratic, and a circle's in the plane, each
    // counted by the enclosures, as before.
    let across = line([-2.5, 0.25, 0.125], [1.0, 0.0, 0.0]);
    let ball = Sphere::new(APEX, 1.5, frame(ON_Z), ((0.0, 2.0), (-0.5, 0.5))).unwrap();
    let (found, work) = meet(&across, &SurfaceKind::Sphere(ball));
    two_crossings(&found, 3.0);
    // In the plane, where a line and a circle are the conics' quadratic.
    let flat = CurveKind::Line(Line::new([-2.5, 0.25], [1.0, 0.0], (-300.0, 300.0)).unwrap());
    let ring = CurveKind::Circle(
        Circle::new([0.5, 0.25], 1.5, Frame::standard().unwrap(), (0.0, 2.0)).unwrap(),
    );
    let mut meter = WorkMeter::new(Budget::of_work(BUDGET));
    let found = curves(&flat, &ring, &mut meter).unwrap();
    assert_eq!(found.items().len(), 2, "{:?}", found.items());
    assert_eq!((work, meter.used().work), (8, 6));
}

#[test]
fn a_line_a_step_from_tangent_is_two_points_or_none() {
    // A step of the radius or the slope out from the line crosses it twice, a step in misses.
    let pairs: [(Maker, f64, f64); 3] = [
        (tangent_to_a_cylinder, 1.5, 3.0),
        (tangent_to_a_cone, 1.0, 3.0),
        (the_issue_cylinder, 232.0, 2.4),
    ];
    for (pair, size, t) in pairs {
        let (edge, surface) = pair(size.next_up());
        let (found, _) = meet(&edge, &surface);
        two_crossings(&found, t);
        let (edge, surface) = pair(size.next_down());
        let (found, _) = meet(&edge, &surface);
        assert!(found.items().is_empty(), "{:?}", found.items());
    }
}

#[test]
fn a_tangent_to_a_cone_on_the_grid_off_its_apex_is_one_point() {
    // ρ = 5 + z about the z axis, so its apex is (0, 0, −5), on the grid; the line along y at
    // x = 5 in z = 0 touches the circle of radius 5 there, at t = 3.
    let c = cone([0.0; 3], 5.0, 1.0, ON_Z);
    let (found, _) = meet(&line([5.0, -3.0, 0.0], [0.0, 1.0, 0.0]), &c);
    one_touch(&found, 3.0);
}

/// Two crossings the enclosures already counted, each with its t's enclosure as it was before
/// decision 32.87: the exact sums are taken only where the enclosure of the discriminant reaches
/// zero, so these answers keep every bit.
#[test]
fn a_crossing_the_enclosures_count_keeps_its_answer() {
    let pairs = [
        (
            line([1.0, 0.25 - 3.0, 7.5], [0.0, 1.0, 0.0]),
            cylinder([0.5, 0.25, 0.125], 1.5, ON_Z),
            [
                (1.585_786_437_626_878_4, 1.585_786_437_626_930_2),
                (4.414_213_562_373_063, 4.414_213_562_373_125),
            ],
        ),
        (
            line([1.0, 0.25 - 3.0, 1.125], [0.0, 1.0, 0.0]),
            cone(APEX, 0.0, 1.0, ON_Z),
            [
                (2.133_974_596_215_515, 2.133_974_596_215_604_7),
                (3.866_025_403_784_391_3, 3.866_025_403_784_486),
            ],
        ),
    ];
    let got: Vec<Vec<(f64, f64)>> = pairs
        .iter()
        .map(|(edge, surface, _)| meet(edge, surface).0.items().iter().map(t_of).collect())
        .collect();
    for (got, (_, _, held)) in got.iter().zip(&pairs) {
        assert_eq!(got, held);
    }
}
