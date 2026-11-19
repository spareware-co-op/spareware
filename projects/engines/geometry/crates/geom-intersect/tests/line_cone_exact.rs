//! A line through a cone's apex, and a line tangent to a cylinder or a cone off the grid (decision
//! 32.87, issue 995, groups 1a and 1b). Where the enclosure of the discriminant reaches zero, its
//! sign is decided exactly: on whole-number data by the exact closed forms, and on any stored
//! doubles by a sum of products of six (a cylinder) or eight (a cone whose radius at v = 0 is zero)
//! of them. A line through the apex, or tangent, is one point; one a step inside is two; one a
//! step outside is none. Before, the engine bisected round the touching point to the floor: one
//! flagged item for 0.48 to 4.15 million units. A cone of any other start radius (decision 32.88,
//! issue 1016 and issue 995's group Q) is decided by the exact sign of P + Q·√N in whole numbers
//! of the stored doubles: the issue's edge beside one misses it, a tangent is one point, and a
//! step either side two or none.

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

/// The cone about [`APEX`] on the z axis of radius `r0` at v = 0 and slope `k`, and the line along
/// y at height 1 that touches it for r0 = 0.5 and k = 1, at x = 2, t = 3 (its apex at z = −0.375
/// is off the grid).
fn tangent_to_a_started_cone(r0: f64, k: f64) -> Pair {
    (
        line([2.0, 0.25 - 3.0, 1.125], [0.0, 1.0, 0.0]),
        cone(APEX, r0, k, ON_Z),
    )
}

/// The same cone's start circle, of radius `r0` at v = 0, and the line along y in its plane that
/// touches it for r0 = 0.5, at t = 3.
fn tangent_to_a_start_circle(r0: f64) -> Pair {
    (
        line([1.0, 0.25 - 3.0, 0.125], [0.0, 1.0, 0.0]),
        cone(APEX, r0, 1.0, ON_Z),
    )
}

/// The issue's cone input with its radius `r0` at v = 0 (decision 32.88, issue 1016): the edge
/// (−136, −4, 100) + t·(0.8, −0.6, 0) of the natural target's `00deffceffffffffff030463ffff`, and
/// the cone about (−4, 12, 16) of slope 2 on the frame off the grid. Worked in exact rationals the
/// edge stands exactly 84 from the axis, nearest it at t = 96, and for r0 = 100 the cone's radius
/// there is 84 less about 8.5e-15.
fn the_issue_cone(r0: f64) -> Pair {
    (
        line([-136.0, -4.0, 100.0], [0.8, -0.6, 0.0]),
        cone([-4.0, 12.0, 16.0], r0, 2.0, OFF),
    )
}

#[test]
fn the_issue_cone_edge_misses_in_closed_form() {
    // Before decision 32.88 the engine bisected round the near touch and flagged it, 3,843,784
    // units for one call of the natural target.
    let (edge, surface) = the_issue_cone(100.0);
    let (found, work) = meet(&edge, &surface);
    assert!(found.items().is_empty(), "{:?}", found.items());
    assert_eq!(work, 0);
}

#[test]
fn a_tangent_to_a_cone_with_a_start_radius_is_one_exact_point() {
    // On halves about the z axis, the line touching a circle above the start circle, and the
    // start circle itself; on the slant axis, a line along the circle through (2.5, 2.75, 1.125),
    // three up the axis where the radius is 0.75 + 3·0.25, and one across it in the tangent plane
    // there, so every product counts.
    let (above, cone_above) = tangent_to_a_started_cone(0.5, 1.0);
    let (start, cone_start) = tangent_to_a_start_circle(0.5);
    let slant = cone(APEX, 0.75, 0.25, SLANT);
    for (edge, surface, t) in [
        (above, cone_above, 3.0),
        (start, cone_start, 3.0),
        (
            line([4.5, 0.75, 2.125], [-2.0, 2.0, -1.0]),
            slant.clone(),
            1.0,
        ),
        (line([2.5, -2.25, 0.125], [0.0, 5.0, 1.0]), slant, 1.0),
    ] {
        let (found, _) = meet(&edge, &surface);
        one_touch(&found, t);
    }
}

#[test]
fn a_step_from_a_tangent_to_a_cone_with_a_start_radius_is_two_points_or_none() {
    // A step of the start radius or the slope out from the line crosses it twice, a step in
    // misses; on the issue's cone the radius there is |r0 + 2z| with z near −92, past the apex, so
    // a step down of r0 crosses and a step up misses. Each crossing's enclosure holds the double
    // nearest its true t, worked to 80 digits from the stored doubles' exact values.
    let outward: [(Pair, f64, [f64; 2]); 4] = [
        (
            tangent_to_a_started_cone(0.5_f64.next_up(), 1.0),
            3.0,
            [2.999_999_981_749_879, 3.000_000_018_250_121],
        ),
        (
            tangent_to_a_started_cone(0.5, 1.0_f64.next_up()),
            3.0,
            [2.999_999_974_190_431_7, 3.000_000_025_809_568_3],
        ),
        (
            tangent_to_a_start_circle(0.5_f64.next_up()),
            3.0,
            [2.999_999_989_463_288, 3.000_000_010_536_712],
        ),
        (
            the_issue_cone(100.0_f64.next_down()),
            96.0,
            [95.999_999_022_774_63, 96.000_000_977_225_38],
        ),
    ];
    for ((edge, surface), t, held) in outward {
        let (found, _) = meet(&edge, &surface);
        two_crossings(&found, t);
        for (item, at) in found.items().iter().zip(held) {
            let (lo, hi) = t_of(item);
            assert!(lo <= at && at <= hi, "{lo} {hi} against {at}");
        }
    }
    let inward = [
        tangent_to_a_started_cone(0.5_f64.next_down(), 1.0),
        tangent_to_a_started_cone(0.5, 1.0_f64.next_down()),
        tangent_to_a_start_circle(0.5_f64.next_down()),
        the_issue_cone(100.0_f64.next_up()),
    ];
    for (edge, surface) in inward {
        let (found, _) = meet(&edge, &surface);
        assert!(found.items().is_empty(), "{:?}", found.items());
    }
}

/// Two crossings of cones with a start radius that the enclosures already counted, off the grid
/// and on it, each with its t's enclosure as it was before decision 32.88: the exact sign is taken
/// only where the enclosure of the discriminant reaches zero, so these answers keep every bit.
#[test]
fn a_crossing_of_a_cone_with_a_start_radius_keeps_its_answer() {
    let pairs = [
        (
            line([1.5, 0.25 - 3.0, 1.125], [0.0, 1.0, 0.0]),
            cone(APEX, 0.5, 1.0, ON_Z),
            vec![
                (1.881_966_011_250_068, 1.881_966_011_250_139_7),
                (4.118_033_988_749_854, 4.118_033_988_749_935),
            ],
        ),
        (
            line([3.0, -3.0, 0.0], [0.0, 1.0, 0.0]),
            cone([0.0; 3], 5.0, 1.0, ON_Z),
            vec![
                (-1.000_000_000_000_018, -0.999_999_999_999_982_7),
                (6.999_999_999_999_97, 7.000_000_000_000_027),
            ],
        ),
    ];
    let got: Vec<Vec<(f64, f64)>> = pairs
        .iter()
        .map(|(edge, surface, _)| meet(edge, surface).0.items().iter().map(t_of).collect())
        .collect();
    let held: Vec<Vec<(f64, f64)>> = pairs.iter().map(|(_, _, held)| held.clone()).collect();
    assert_eq!(got, held);
}

#[test]
fn a_tangent_to_a_started_cone_whose_data_span_too_far_is_the_engines() {
    // The same line with its origin at y = 1e-30 rather than −2.75: the stored lengths as whole
    // numbers would need more than 127 bits, so no exact sign is taken, and the engine flags the
    // touching point or stops on the budget.
    let (_, surface) = tangent_to_a_started_cone(0.5, 1.0);
    let far = line([2.0, 1e-30, 1.125], [0.0, 1.0, 0.0]);
    let mut meter = WorkMeter::new(Budget::of_work(BUDGET));
    match curve_surface(&far, &surface, &mut meter) {
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

/// Issue 995's group Q: the cone about the z axis through (−4, −260, −4) of start radius 256 and
/// slope `k`, as the piercing target draws it, and the line along x through (−4, `y`, −4), which
/// touches its start circle for y = −4, at t = 0.
fn group_q(y: f64, k: f64) -> Pair {
    (
        line([-4.0, y, -4.0], [1.0, 0.0, 0.0]),
        cone([-4.0, -260.0, -4.0], 256.0, k, ON_Z),
    )
}

/// The piercing target's slopes of group Q, 7/16 and 9/16 (apex 4096/7 and 4096/9 below the
/// centre, off the grid), and 1/8 (apex on the grid).
const SLOPES_Q: [f64; 3] = [0.4375, 0.5625, 0.125];

#[test]
fn a_segment_touching_a_start_circle_at_its_end_is_one_point() {
    // The piercing target's segment (−4, −4, −4) to (−3, −4, −4), t from 0 to 1, touches at its
    // start: before decision 32.88 the engine spent 387,549 and 597,383 units on slopes 7/16 and
    // 9/16 for one flagged item; slope 1/8 was a closed form already. Now 4 units each.
    for k in SLOPES_Q {
        let (_, surface) = group_q(-4.0, k);
        let segment = CurveKind::Line(Line::segment([-4.0; 3], [-3.0, -4.0, -4.0]).unwrap().value);
        let (found, work) = meet(&segment, &surface);
        one_touch(&found, 0.0);
        let (lo, _) = t_of(&found.items()[0]);
        assert_eq!((lo, work), (0.0, 4), "{k}");
    }
}

#[test]
fn a_step_from_group_q_s_touch_is_two_points_or_none() {
    // One step of y nearer the axis, the line crosses the start circle twice, at t = ±6.74e-7
    // (worked to 80 digits from the stored doubles' exact values); one step farther it misses.
    for k in SLOPES_Q {
        let (edge, surface) = group_q((-4.0_f64).next_down(), k);
        let (found, _) = meet(&edge, &surface);
        two_crossings(&found, 0.0);
        for (item, at) in found
            .items()
            .iter()
            .zip([-6.743_495_761_743_046e-7, 6.743_495_761_743_046e-7])
        {
            let (lo, hi) = t_of(item);
            assert!(lo <= at && at <= hi, "{lo} {hi} against {at}");
        }
        let (edge, surface) = group_q((-4.0_f64).next_up(), k);
        let (found, _) = meet(&edge, &surface);
        assert!(found.items().is_empty(), "{:?}", found.items());
    }
}
