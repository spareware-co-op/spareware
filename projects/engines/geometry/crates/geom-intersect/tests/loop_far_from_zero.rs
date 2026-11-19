//! A loop whose parameter starts far from zero (chapter 12.7's trimming). A circle's parameter is in
//! half-turns, and its domain may start any number of turns out: a cylinder's turn, or the circle an
//! extrusion is swept from, at 2^21 half-turns and beyond. A half plane across the loop cuts it
//! where its edge meets it, and each cut has to be found on the loop's own domain however far out
//! that starts. Before, a cut 2^20 turns out or more was dropped without a word, and the whole
//! circle came back, or nothing; and a cylinder whose turn started more than eight turns out met
//! nothing at all.
//!
//! Every case is the circle x² + y² = 1024², z = 0, cut by the half plane z = 0, x ≥ 512: the arc
//! from −1/3 to 1/3 of a half-turn, its ends at (512, ±512√3, 0) and its middle at (1024, 0, 0),
//! both edges' ends of a domain. Each answer is held against those points, in both orders of the
//! pair, on both sides of zero.
//!
//! Far enough out no answer can be: 2^43 turns out a double holds a turn only to a 256th of a
//! half-turn, twelve steps along this circle, so neither a cut nor the surface's own points can be
//! placed to a step. Such an operand is refused with `FrameRange`, exactly where its doubles stand
//! more than two steps apart on it, as a plane written to show the bound shows.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    clippy::cast_precision_loss,
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_curve::{Basis, Circle, CurveKind, Frame, ParamCurve, Trimmed};
use geom_intersect::IntersectError;
use geom_intersect::quadrics::natural;
use geom_intersect::result::{CurveEnd, Intersection, Item};
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, Extrusion, Plane, Sphere, SurfaceKind};

const R: f64 = 1024.0;

/// Where the loops start, in half-turns: near zero, past the eight turns a surface's turn was
/// moved by, at and past the 2^20 turns a cut was, 2^29 turns out, where the room left between two
/// cuts had grown to a half-turn, and on to 2^40, the last power of two whose doubles stand within
/// two steps on this circle by the bound the check takes: the spacing times the largest the
/// speed's enclosure reaches over a whole turn, √2·πR.
const STARTS: [f64; 15] = [
    0.0,
    6.5,
    18.0,
    -18.0,
    1_048_576.0,
    2_097_150.0,
    2_097_152.0,
    -2_097_152.0,
    -2_097_154.0,
    2_097_152.75,
    -2_097_153.25,
    1_073_741_824.75,
    -1_073_741_824.75,
    68_719_476_736.75,
    1_099_511_627_776.75,
];

/// Past that bound (2^41), past where a double holds a turn to a step at this radius at all (2^44),
/// and where it holds only whole half-turns (2^52).
const FAR: [f64; 5] = [
    -2_199_023_255_552.5,
    17_592_186_044_416.75,
    -17_592_186_044_416.5,
    4_503_599_627_370_496.0,
    -4_503_599_627_370_498.0,
];

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The half plane z = 0, x ≥ 512, out to 2048 each way.
fn half() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((512.0, 2048.0), (-2048.0, 2048.0)),
        )
        .unwrap(),
    )
}

/// The circle about z of radius 1024 swept from z = −1024 to 1024, its turn from `start`.
fn swept(start: f64) -> SurfaceKind {
    let circle = Circle::new(
        [0.0; 3],
        R,
        Frame::standard().unwrap(),
        (start, start + 2.0),
    )
    .unwrap();
    let profile = Trimmed::new(
        Basis::try_from(CurveKind::Circle(circle)).unwrap(),
        (start, start + 2.0),
    )
    .unwrap();
    SurfaceKind::Extrusion(Extrusion::new(profile, [0.0, 0.0, 1.0], (-R, R)).unwrap())
}

/// The cylinder about z of radius 1024 from z = −1024 to 1024, its turn from `start`.
fn tube(start: f64) -> SurfaceKind {
    let domain = ((start, start + 2.0), (-R, R));
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], R, Frame::standard().unwrap(), domain).unwrap())
}

/// The one curve of `r`, which must be all of it, and not closed.
fn the_arc(r: &Intersection, what: &str) -> CurveKind<3> {
    let items = r.items();
    assert_eq!(items.len(), 1, "{what}: {items:?}");
    let Item::Curve { curve, ends, .. } = &items[0] else {
        panic!("{what}: {items:?}");
    };
    assert_eq!(*ends, [CurveEnd::Boundary; 2], "{what}: {curve:?}");
    (**curve).clone()
}

/// How far apart, along the circle, two turns a double holds from `start` on can stand: the
/// spacing of the doubles there times the circle's speed, πR, and never under a millionth.
fn grain(start: f64) -> f64 {
    let far = start.abs() + 2.0;
    ((far.next_up() - far) * std::f64::consts::PI * R).max(1e-6)
}

fn assert_near(p: [f64; 3], q: [f64; 3], within: f64, what: &str) {
    let gap = (0..3).map(|k| (p[k] - q[k]).abs()).fold(0.0, f64::max);
    assert!(gap < within, "{what}: {p:?} is {gap} from {q:?}");
}

/// The arc's ends and middle against the exact ones, to `within`.
fn holds_the_arc(curve: &CurveKind<3>, within: f64, what: &str) {
    let (lo, hi) = curve.domain();
    let y = 512.0 * 3.0_f64.sqrt();
    let ends = [
        curve.point(lo, Limit::FromRight).unwrap(),
        curve.point(hi, Limit::FromLeft).unwrap(),
    ];
    // Either way round: the curve's own direction is the closed form's.
    let (first, second) = if ends[0][1] < 0.0 {
        (ends[0], ends[1])
    } else {
        (ends[1], ends[0])
    };
    assert_near(first, [512.0, -y, 0.0], within, what);
    assert_near(second, [512.0, y, 0.0], within, what);
    let middle = curve.point(lo / 2.0 + hi / 2.0, Limit::FromRight).unwrap();
    assert_near(middle, [R, 0.0, 0.0], within, what);
}

#[test]
fn a_swept_loop_is_cut_where_the_edge_meets_it_however_far_its_turn_starts() {
    for start in STARTS {
        let loop_of = swept(start);
        for (a, b, order) in [
            (&half(), &loop_of, "plane first"),
            (&loop_of, &half(), "sweep first"),
        ] {
            let what = format!("turn from {start}, {order}");
            let r = natural(a, b, &mut meter())
                .unwrap_or_else(|e| panic!("{what}: {e:?}"))
                .unwrap_or_else(|| panic!("{what}: no closed form"));
            let arc = the_arc(&r, &what);
            // The cuts come from where the edge meets the sweep, whose own turn is held only to
            // its doubles' grain.
            let within = grain(start);
            holds_the_arc(&arc, within, &what);
            // The arc is the profile's own, on its domain moved by whole turns to within two turns
            // of zero: it starts at 5/3 of a half-turn, a whole number of turns from the start
            // nearer zero than four half-turns, and is 2/3 long.
            let near = start % 4.0;
            let from = near + (5.0 / 3.0 - near).rem_euclid(2.0);
            let (lo, hi) = arc.domain();
            let turn = within / R;
            assert!((lo - from).abs() < turn, "{what}: from {lo}, not {from}");
            assert!(
                (hi - lo - 2.0 / 3.0).abs() < 2.0 * turn,
                "{what}: {lo} to {hi}"
            );
        }
    }
}

#[test]
fn a_loop_too_far_out_to_hold_a_step_is_refused() {
    for start in FAR {
        for loop_of in [swept(start), tube(start)] {
            for (a, b) in [(&half(), &loop_of), (&loop_of, &half())] {
                let r = natural(a, b, &mut meter()).map(|r| r.map(|r| r.items().to_vec()));
                assert_eq!(r, Err(IntersectError::FrameRange), "turn from {start}");
            }
        }
    }
}

/// The plane z = 0 whose `u` (or `v`) runs from 2^`at` along x at the speed two bits below 2048,
/// stored from where that parameter's start is x = 0, so its box is x from 0 to 2048 whatever
/// `at` is.
fn far_written(at: i32, u_far: bool) -> SurfaceKind {
    let speed = 2048.0 - 2.0_f64.powi(-41);
    let start = 2.0_f64.powi(at);
    let origin = [-(start * speed), 0.0, 0.0];
    let (along, across) = ([speed, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let (vectors, domain) = if u_far {
        ((along, across), ((start, start + 1.0), (0.0, 1.0)))
    } else {
        ((across, along), ((0.0, 1.0), (start, start + 1.0)))
    };
    SurfaceKind::Plane(Plane::new(origin, vectors.0, vectors.1, domain).unwrap())
}

#[test]
fn a_parameter_whose_doubles_stand_more_than_two_steps_apart_is_refused() {
    // From 2^42 a double's spacing is 2^-10, and the speed's bound, the upper end of the enclosed
    // norm of 2048 less two bits (each of its four steps widens by one), is 2048: two steps
    // exactly, which fits. From 2^43 the spacing is 2^-9 and the points four steps apart.
    let ball = SurfaceKind::Sphere(
        Sphere::new(
            [1024.0, 0.5, 0.0],
            0.25,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    );
    for u_far in [true, false] {
        let fits = far_written(42, u_far);
        assert!(
            natural(&fits, &ball, &mut meter()).is_ok(),
            "u far: {u_far}"
        );
        let apart = far_written(43, u_far);
        let r = natural(&apart, &ball, &mut meter()).map(|r| r.is_some());
        assert_eq!(r, Err(IntersectError::FrameRange), "u far: {u_far}");
    }
}

#[test]
fn a_cylinder_is_met_however_far_its_turn_starts() {
    for start in STARTS {
        let loop_of = tube(start);
        for (a, b, order) in [
            (&half(), &loop_of, "plane first"),
            (&loop_of, &half(), "cylinder first"),
        ] {
            let what = format!("turn from {start}, {order}");
            let r = natural(a, b, &mut meter())
                .unwrap_or_else(|e| panic!("{what}: {e:?}"))
                .unwrap_or_else(|| panic!("{what}: no closed form"));
            let arc = the_arc(&r, &what);
            // The arc is the plane's circle, written from zero: its ends are held to the last
            // millionth wherever the cylinder's turn starts.
            holds_the_arc(&arc, 1e-6, &what);
            // The cylinder's own turn over the arc is read on its domain.
            let spans = r.items()[0].at().spans();
            let turn = if order == "plane first" {
                spans[2]
            } else {
                spans[0]
            };
            assert!(
                turn.lo() >= start && turn.hi() <= start + 2.0,
                "{what}: turn {turn:?}"
            );
        }
    }
}
