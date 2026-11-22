//! Cases at the edges of the closed forms' decisions, each with its true answer: a far plane whose
//! parameter runs exactly two units, read past its end by the corner offsets (32.94); a box that
//! ends exactly on a cylinder's end, which reaches it; a loop whose turn starts at 1, cut so that
//! its one stretch on both domains runs over the loop's join; and a line along a very flat ellipse
//! whose quadratic's roots lie past the doubles, where the closed form cannot tell and the engine
//! answers.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::disallowed_methods,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly, and the names are the closed form's"
)]

use geom_curve::{
    Axes, Basis, Circle, CurveKind, Ellipse, Frame, Line, ParamCurve, Trimmed, tight,
};
use geom_intersect::curve::curves;
use geom_intersect::quadrics::natural;
use geom_intersect::result::{CurveEnd, Intersection, Item};
use geom_num::interval::Interval;
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, Extrusion, Plane, Sphere, SurfaceKind};

type Vector = [Interval; 3];

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// 2^e exactly, for |e| up to 63.
fn two_to(e: i32) -> f64 {
    let p = (1u64 << e.unsigned_abs()) as f64;
    if e < 0 { 1.0 / p } else { p }
}

/// 2^e exactly, from its bits, for e from −1074 to 1023.
fn pow2(e: i32) -> f64 {
    if e >= -1022 {
        f64::from_bits(u64::try_from(e + 1023).unwrap() << 52)
    } else {
        f64::from_bits(1u64 << u32::try_from(e + 1074).unwrap())
    }
}

fn curve_items(r: &Intersection) -> Vec<&CurveKind<3>> {
    r.items()
        .iter()
        .filter_map(|i| match i {
            Item::Curve { curve, .. } => Some(&**curve),
            _ => None,
        })
        .collect()
}

fn sphere(c: [f64; 3], r: f64) -> SurfaceKind {
    let domain = ((0.0, 2.0), (-0.5, 0.5));
    SurfaceKind::Sphere(Sphere::new(c, r, Frame::standard().unwrap(), domain).unwrap())
}

/// How many items `natural` finds for the pair, or `None` where it leaves the pair to the engine.
fn found(a: &SurfaceKind, b: &SurfaceKind) -> Option<usize> {
    natural(a, b, &mut meter())
        .unwrap_or_else(|e| panic!("{e:?} for {a:?} and {b:?}"))
        .map(|r| r.items().len())
}

/// A plane's parameter range exactly two units long is still an edge at both ends: only a round
/// kind's turn has a join there. The plane z = 0 is written as x = u − 2^52 and y = v − 2^52, its
/// u over two units from 2^52, where a double's step is 1, so x runs from 0 to 2, and its v over
/// 32 units, y from −16 to 16, wide enough that the box of a v near 2^52 stays inside it. A ball
/// of radius 1/4 about (2.5, 0.5, 0) meets it in a circle from x = 2.25 to 2.75, past u's end, and
/// no edge comes near it. The box of the circle's middle, (2.25, 0.5), reaches u's end (2^52 + 2.25
/// is held from 2^52 + 1), so the plane is read again from its corners, whose offset says the
/// circle lies a quarter past: it is nothing. Moved onto the domain the ball's circle is kept whole.
#[test]
fn a_plane_whose_parameter_runs_two_units_has_edges_at_both_ends() {
    let start = two_to(52);
    let sheet = SurfaceKind::Plane(
        Plane::new(
            [-start, -start, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((start, start + 2.0), (start - 16.0, start + 16.0)),
        )
        .unwrap(),
    );
    let past = sphere([2.5, 0.5, 0.0], 0.25);
    assert_eq!(found(&sheet, &past), Some(0));
    assert_eq!(found(&past, &sheet), Some(0));
    let on = sphere([1.0, 0.5, 0.0], 0.25);
    for (a, b) in [(&sheet, &on), (&on, &sheet)] {
        let r = natural(a, b, &mut meter()).unwrap().unwrap();
        assert_eq!(curve_items(&r).len(), 1, "{:?}", r.items());
    }
}

// The closed form's own interval steps, written out as `quadrics` takes them, to read the box it
// reads: each is one of `Interval`'s operations, rounded and widened the same way.

fn lift(v: [f64; 3]) -> Vector {
    v.map(|x| Interval::point(x).unwrap())
}

fn dot(a: Vector, b: Vector) -> Interval {
    let [p, q, r] = a;
    let [x, y, z] = b;
    let (px, qy, rz) = (p.mul(x).unwrap(), q.mul(y).unwrap(), r.mul(z).unwrap());
    px.add(qy).unwrap().add(rz).unwrap()
}

fn cross(a: Vector, b: Vector) -> Vector {
    let [p, q, r] = a;
    let [x, y, z] = b;
    let less = |s: Interval, t: Interval| s.sub(t).unwrap();
    [
        less(q.mul(z).unwrap(), r.mul(y).unwrap()),
        less(r.mul(x).unwrap(), p.mul(z).unwrap()),
        less(p.mul(y).unwrap(), q.mul(x).unwrap()),
    ]
}

fn minus(a: Vector, b: Vector) -> Vector {
    [0, 1, 2].map(|k| a[k].sub(b[k]).unwrap())
}

fn plus(a: Vector, b: Vector) -> Vector {
    [0, 1, 2].map(|k| a[k].add(b[k]).unwrap())
}

fn unit(v: Vector) -> Vector {
    let length = tight::norm(&v).unwrap();
    v.map(|x| x.div(length).unwrap())
}

fn wide(v: Vector) -> f64 {
    let [x, y, z] = v.map(|c| c.hi() - c.lo());
    tight::larger(tight::larger(x, y), z)
}

/// The height of the plane below.
const H: f64 = 0.3;

/// The plane z = 0.3, out to 4 each way.
fn level() -> SurfaceKind {
    let side = (-4.0, 4.0);
    SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, H],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            (side, side),
        )
        .unwrap(),
    )
}

/// The cylinder of radius 1 about z, a whole turn, over the heights `v`.
fn tube(v: (f64, f64)) -> SurfaceKind {
    let frame = Frame::standard().unwrap();
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 1.0, frame, ((0.0, 2.0), v)).unwrap())
}

/// A box that ends exactly at an end of a domain that is an edge reaches it, from inside the
/// domain or from outside; only one wholly past the end is off it (32.94). The plane z = 0.3
/// meets the cylinder of radius 1 about z in a circle, and the closed form reads whether it lies
/// on the cylinder's heights by the box of its middle's height: the stored circle's point there,
/// grown by the piece's reach (how far its centre, the axis point the plane cuts, may stand from
/// the stored one), its offset from the centre along the axis's unit vector, divided by the axial
/// factor. It is read here the same way, from l to h. No edge comes near the circle: the plane's
/// edges stand 4 out, and the cylinder's end circles are level, off the plane. A cylinder whose
/// heights start exactly at h, or end exactly at l, has its end on the box, where the circle may
/// stand, so the pair is the engine's; one double further each way the box is wholly past the
/// end, and the pair is nothing.
#[test]
fn a_box_that_ends_exactly_on_a_cylinders_end_reaches_it() {
    let plane = level();
    let tall = tube((-2.0, 2.0));
    let r = natural(&plane, &tall, &mut meter()).unwrap().unwrap();
    let [circle] = curve_items(&r)[..] else {
        panic!("{:?}", r.items())
    };
    let SurfaceKind::Cylinder(c) = &tall else {
        panic!("{tall:?}")
    };
    let (_, [_, _, n]) = c.frame().unit_vectors().unwrap();
    // The piece's reach: the width of the enclosed centre, where the plane cuts the axis.
    let normal = unit(cross(lift([1.0, 0.0, 0.0]), lift([0.0, 1.0, 0.0])));
    let one = tight::point(1.0).unwrap();
    let along = n.map(|x| tight::mul(x, one).unwrap());
    let axis_at = lift(c.center());
    let tilt = dot(normal, along);
    let k = dot(minus(lift([0.0, 0.0, H]), axis_at), normal)
        .div(tilt)
        .unwrap();
    let centre = plus(axis_at, along.map(|x| x.mul(k).unwrap()));
    let reach = tight::larger(wide(centre), 0.0);
    assert!(reach > 0.0 && reach < 1e-14, "{reach}");
    let (lo, hi) = circle.domain();
    let middle = circle
        .enclose_point(lo * 0.5 + hi * 0.5, Limit::FromRight, &mut meter())
        .unwrap()
        .unwrap();
    let room = Interval::new(-reach, reach).unwrap();
    let at = plus(middle, [room; 3]);
    let height = dot(minus(at, lift(c.center())), n)
        .div(Interval::point(c.axial_sign()).unwrap())
        .unwrap();
    let (l, h) = (height.lo(), height.hi());
    assert!(
        H - 1e-14 < l && l < H && H < h && h < H + 1e-14,
        "{height:?}"
    );
    for (ends, reached) in [
        ((h, 2.0), true),
        ((h.next_up(), 2.0), false),
        ((-2.0, l), true),
        ((-2.0, l.next_down()), false),
    ] {
        let short = tube(ends);
        for (a, b) in [(&plane, &short), (&short, &plane)] {
            let want = if reached { None } else { Some(0) };
            assert_eq!(found(a, b), want, "heights {ends:?}");
        }
    }
}

const R: f64 = 1024.0;

/// The circle about z of radius 1024 swept from z = −1024 to 1024, its turn from `start`.
fn swept(start: f64) -> SurfaceKind {
    let turn = (start, start + 2.0);
    let circle = Circle::new([0.0; 3], R, Frame::standard().unwrap(), turn).unwrap();
    let profile = Trimmed::new(Basis::try_from(CurveKind::Circle(circle)).unwrap(), turn).unwrap();
    SurfaceKind::Extrusion(Extrusion::new(profile, [0.0, 0.0, 1.0], (-R, R)).unwrap())
}

fn assert_near(p: [f64; 3], q: [f64; 3], what: &str) {
    let gap = (0..3).map(|k| (p[k] - q[k]).abs()).fold(0.0, f64::max);
    assert!(gap < 1e-6, "{what}: {p:?} is {gap} from {q:?}");
}

/// A loop's one stretch on both domains that runs over the loop's join ends a whole turn on, a
/// turn being the loop's period, its domain's length. The plane z = 0 over x up to 512 and y from
/// −1024 meets the sweep of the circle x² + y² = 1024² in that circle. Its edge x = 512 cuts the
/// circle at (512, ±512√3, 0), a third of a half-turn either side of +x, and its edge y = −1024
/// touches it at (0, −1024, 0), where the circle stays on the domain either side. A turn from 1
/// (or 2.5) is moved by no whole turns (the window loops are moved to), so the loop runs from 1 and is cut
/// from the touch at 3/2 on: on to the cut at 5/3, off past +x to 7/3, and on again to the touch a
/// turn later. The stretch kept is the run from 7/3 over the join to 5/3 a turn on, 11/3: the arc
/// through −x, from (512, 512√3, 0) round to (512, −512√3, 0), 4/3 of a half-turn long. The same
/// for a turn from 2.5, from 13/3 to 17/3.
#[test]
fn a_stretch_over_a_loops_join_ends_a_turn_on() {
    let sheet = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-2048.0, 512.0), (-R, 2048.0)),
        )
        .unwrap(),
    );
    let y = 512.0 * 3.0_f64.sqrt();
    for (start, from) in [(1.0, 7.0 / 3.0), (2.5, 13.0 / 3.0)] {
        let sweep = swept(start);
        for (a, b) in [(&sheet, &sweep), (&sweep, &sheet)] {
            let what = format!("turn from {start}");
            let r = natural(a, b, &mut meter()).unwrap().unwrap();
            let items = r.items();
            assert_eq!(items.len(), 1, "{what}: {items:?}");
            let Item::Curve { curve, ends, .. } = &items[0] else {
                panic!("{what}: {items:?}")
            };
            assert_eq!(*ends, [CurveEnd::Boundary; 2], "{what}");
            let (lo, hi) = curve.domain();
            assert!((lo - from).abs() < 1e-9, "{what}: from {lo}");
            assert!((hi - (from + 4.0 / 3.0)).abs() < 1e-9, "{what}: to {hi}");
            let at = |t: f64| curve.point(t, Limit::FromRight).unwrap();
            assert_near(at(lo), [512.0, y, 0.0], &what);
            assert_near(at(hi), [512.0, -y, 0.0], &what);
            assert_near(at(lo * 0.5 + hi * 0.5), [-R, 0.0, 0.0], &what);
        }
    }
}

/// A discriminant whose enclosure holds zero cannot count the roots, whatever count its roots'
/// enclosures would give, and the pair goes to the engine. The ellipse x² + (2^500·y)² = 1, its
/// axes u = (1, 0) and v = (0, 2^−500), is a sliver along the x axis from −1 to 1, never more than
/// 2^−500 off it. The segment of x = 1 from (1, −1/2) along (0, 2^−1030) over t within ±2^1000 runs
/// up y only from −1/2 − 2^−30 to −1/2 + 2^−30, half a unit below the ellipse: they do not meet.
/// In the ellipse's own coordinates (x, 2^500·y) the line is (1, −2^499) + t·(0, 2^−530): the
/// quadratic's leading coefficient is 2^−1060, held to a handful of bits below the normal doubles,
/// and its discriminant, which the line's tangency to the whole line's ellipse at (1, 0) makes
/// zero, is held across zero, so the closed form leaves the pair to the engine. The roots it would
/// give, near t = 2^1029, lie past every double.
#[test]
fn a_discriminant_held_across_zero_leaves_a_line_to_the_engine() {
    let flat = CurveKind::Ellipse(
        Ellipse::<2>::new(
            [0.0, 0.0],
            Axes::Vectors {
                u: [1.0, 0.0],
                v: [0.0, pow2(-500)],
            },
            (0.0, 2.0),
        )
        .unwrap(),
    );
    let span = (-pow2(1000), pow2(1000));
    let up = Line::<2>::new([1.0, -0.5], [0.0, pow2(-1030)], span).unwrap();
    let segment = CurveKind::Line(up);
    for (a, b) in [(&segment, &flat), (&flat, &segment)] {
        let r = curves(a, b, &mut meter()).unwrap();
        assert!(r.items().is_empty(), "{:?}", r.items());
    }
}
