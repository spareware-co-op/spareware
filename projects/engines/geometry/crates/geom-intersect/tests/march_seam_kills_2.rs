//! The march at the exact ends of its comparisons (chapters 12.3, 12.8 and 12.10): settled pairs
//! built by hand so that one comparison stands exactly at its tie, or one start's enclosure reaches
//! a pole it is not held on, and the branches the march gives are the curve's own: how many, which
//! starts each joins, and whether the pair is left unproven.
//!
//! Every case's curve is known exactly: two coordinate planes meet in a line, the plane z = 80y
//! meets the unit cylinder about z in an ellipse, a plane through the apex of the cone
//! x² + y² = z² meets it in two of its rulings, and the plane x = z + 1024 through the south pole
//! of the sphere of radius 1024 meets it in a circle through that pole.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_curve::Frame;
use geom_intersect::general::{Settled, Subdivided};
use geom_intersect::march::{Traced, march};
use geom_num::interval::Interval;
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, ParamSurface, Plane, Sphere, SurfaceKind};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

fn around(x: f64, half: f64) -> Interval {
    Interval::new(x - half, x + half).unwrap()
}

fn pt(x: f64) -> Interval {
    Interval::point(x).unwrap()
}

fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// One settled pair and nothing flagged.
fn one_pair(
    first: ((f64, f64), (f64, f64)),
    second: ((f64, f64), (f64, f64)),
    starts: Vec<[Interval; 4]>,
) -> Subdivided {
    Subdivided {
        settled: vec![Settled {
            first,
            second,
            starts,
        }],
        flagged: Vec::new(),
    }
}

/// The point of `surface` at the middle of the parameters `(u, v)`.
fn at(surface: &SurfaceKind, u: Interval, v: Interval) -> [f64; 3] {
    surface
        .point(mid(u), mid(v), (Limit::FromRight, Limit::FromRight))
        .unwrap()
}

/// Every vertex of every branch is one point of both surfaces, to within `room`, and lies where
/// `on` says the exact curve lies.
fn on_the_curve(
    (a, b): (&SurfaceKind, &SurfaceKind),
    traced: &Traced,
    room: f64,
    on: impl Fn([f64; 3]) -> f64,
) {
    for branch in &traced.branches {
        for x in &branch.vertices {
            let (p, q) = (at(a, x[0], x[1]), at(b, x[2], x[3]));
            for k in 0..3 {
                assert!((p[k] - q[k]).abs() <= room, "{x:?}: {p:?} against {q:?}");
            }
            assert!(on(p).abs() <= room, "{x:?} at {p:?} is off the curve");
        }
    }
}

/// The start each branch begins and ends at, by index into `starts`, read from its end vertices.
fn joins(traced: &Traced, starts: &[[Interval; 4]]) -> Vec<[usize; 2]> {
    let which = |x: &[Interval; 4]| {
        starts
            .iter()
            .position(|s| {
                s.iter()
                    .zip(x)
                    .all(|(a, b)| a.lo() == b.lo() && a.hi() == b.hi())
            })
            .unwrap_or_else(|| panic!("{x:?} is no start"))
    };
    let mut out: Vec<[usize; 2]> = traced
        .branches
        .iter()
        .map(|b| {
            let mut ends = [which(&b.vertices[0]), which(b.vertices.last().unwrap())];
            ends.sort_unstable();
            ends
        })
        .collect();
    out.sort_unstable();
    out
}

/// The planes z = 0, as (x, y), and x = `side`, as (y, z), which meet in the line x = `side`,
/// z = 0, marched between starts on it at y = 0 and y = 2⁻³³ at a floor of `k` · 2⁻⁴²: what was
/// traced, and the starts.
fn two_planes(side: f64, k: f64) -> (Traced, Vec<[Interval; 4]>) {
    let one = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((side - 1.0, side + 1.0), (-1.0, 1.0)),
        )
        .unwrap(),
    );
    let two = SurfaceKind::Plane(
        Plane::new(
            [side, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            ((-1.0, 1.0), (-1.0, 1.0)),
        )
        .unwrap(),
    );
    let far = 1.0 / 8_589_934_592.0;
    let starts = vec![
        [pt(side), pt(0.0), pt(0.0), pt(0.0)],
        [pt(side), pt(far), pt(far), pt(0.0)],
    ];
    let found = one_pair(
        ((side - 0.5, side + 0.5), (-0.5, 0.5)),
        ((-0.5, 0.5), (-0.5, 0.5)),
        starts.clone(),
    );
    let floor = k / 4_398_046_511_104.0;
    let traced = march(&one, &two, &found, floor, 1.0, &mut meter()).unwrap();
    (traced, starts)
}

#[test]
fn a_step_whose_image_ends_on_its_boxs_own_end_is_not_proven() {
    // A step is proven only if the image of its box lies strictly inside the box (12.3). Along the
    // line x = ±1, z = 0 the parameter x never moves, so its side of the box is the step's pad
    // alone, a few of the doubles next to 1 and wider with the floor, while the image the proof
    // computes is wide by its own rounding alone. At a floor of 8 · 2⁻⁴² the image's end away from
    // zero is the box's own end, to the bit: the upper ends on x = 1, the lower ends on x = −1. An
    // end reached is not strictly inside, so the pair is left unproven; one more double of floor
    // and the step is proven, and the one branch joins the two starts.
    for side in [1.0, -1.0] {
        let (traced, _) = two_planes(side, 8.0);
        assert!(traced.branches.is_empty(), "x = {side}: {traced:?}");
        assert_eq!(traced.unproven.len(), 1, "x = {side}: {traced:?}");
        let (traced, starts) = two_planes(side, 9.0);
        assert!(traced.unproven.is_empty(), "x = {side}: {traced:?}");
        assert_eq!(joins(&traced, &starts), [[0, 1]], "x = {side}: {traced:?}");
    }
}

#[test]
fn two_starts_level_on_a_patchs_edge_each_end_their_own_arc() {
    // z = 80y meets the unit cylinder about z in an ellipse over the unit circle. The plane's
    // patch ends at y = cos 0.05, which cuts the top of the ellipse off: two arcs, each from a
    // start low on its side up to a start on that edge. The two starts on the edge are level, with
    // one y and one height between them, so a vertex of the left arc that comes beside them both
    // is as far, in the march's measure, from the one as from the other; the arc ends at its own.
    let m = 80.0;
    let pi = core::f64::consts::PI;
    let ramp = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, m],
            ((-2.0, 2.0), (0.0, 2.0)),
        )
        .unwrap(),
    );
    let post = SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            1.0,
            Frame::standard().unwrap(),
            ((0.05, 0.95), (0.0, 100.0)),
        )
        .unwrap(),
    );
    let e = 1e-12;
    let cut = 0.05_f64;
    // The point of the ellipse at the cylinder's angle `phi`, in half turns.
    let low = |phi: f64| {
        let (x, y) = ((pi * phi).cos(), (pi * phi).sin());
        [
            around(x, e),
            around(y, e),
            around(phi, e),
            around(m * y, 64.0 * e),
        ]
    };
    let (y, z) = (pt(cut.cos()), around(m * cut.cos(), 64.0 * e));
    let starts = vec![
        low(1.0 - 0.6 / pi),
        [around(-cut.sin(), e), y, around(0.5 + cut / pi, e), z],
        [around(cut.sin(), e), y, around(0.5 - cut / pi, e), z],
        low(0.6 / pi),
    ];
    let found = one_pair(
        ((-1.0, 1.0), (0.5, cut.cos())),
        ((0.1, 0.9), (40.0, 80.0)),
        starts.clone(),
    );
    let traced = march(&ramp, &post, &found, 0.125, 0.003, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    assert_eq!(joins(&traced, &starts), [[0, 1], [2, 3]], "{traced:?}");
    on_the_curve((&ramp, &post), &traced, 0.125, |[x, y, _]| {
        x * x + y * y - 1.0
    });
}

#[test]
fn a_start_level_with_an_edges_own_start_is_not_the_next_along_that_edge() {
    // The plane x = −z cos π/64 passes through the apex of the cone x² + y² = z² and holds its
    // rulings at the angles π(1 ∓ 1/64). The cone's domain begins at the first, so that ruling is
    // an edge of the cone lying in the plane, and the second runs inside. One pair holds both from
    // z = 2 to z = 3: two starts on each, level in twos. The inner ruling's lower start is given
    // wide, its u reaching the edge, where it stands level with the edge's own lower start. A
    // piece along an edge runs to the nearest start further along it, never to one level with its
    // own, so the edge's two starts are joined along it and the inner ruling's two by the march.
    let g = 1.0 / 64.0;
    let pi = core::f64::consts::PI;
    let (c, s) = ((pi * g).cos(), (pi * g).sin());
    let cone = SurfaceKind::Cone(
        Cone::new(
            [0.0, 0.0, 1.0],
            1.0,
            1.0,
            Frame::standard().unwrap(),
            ((1.0 - g, 1.25), (0.0, 4.0)),
        )
        .unwrap(),
    );
    // As (y, z).
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [0.0, 1.0, 0.0],
            [-c, 0.0, 1.0],
            ((-1.0, 1.0), (0.0, 5.0)),
        )
        .unwrap(),
    );
    let e = 1e-9;
    let floor = 1.0 / 64.0;
    let starts = vec![
        [pt(1.0 - g), pt(1.0), around(2.0 * s, e), around(2.0, e)],
        [
            Interval::new(1.0 - g, 1.0 + 3.0 * g).unwrap(),
            pt(1.0),
            around(-2.0 * s, e),
            around(2.0, e),
        ],
        [pt(1.0 - g), pt(2.0), around(3.0 * s, e), around(3.0, e)],
        [
            around(1.0 + g, e),
            pt(2.0),
            around(-3.0 * s, e),
            around(3.0, e),
        ],
    ];
    let found = one_pair(
        ((1.0 - g, 1.0 + 4.0 * g), (1.0, 2.0)),
        ((-0.5, 0.5), (1.5, 3.5)),
        starts.clone(),
    );
    let traced = march(&cone, &plane, &found, floor, floor, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    assert_eq!(joins(&traced, &starts), [[0, 2], [1, 3]], "{traced:?}");
    on_the_curve((&cone, &plane), &traced, floor, |[x, y, z]| {
        (x + c * z).abs() + (y.abs() - s * z).abs()
    });
}

#[test]
fn a_start_whose_box_reaches_a_pole_it_is_not_held_on_is_not_the_pole() {
    // x = z + 1024 passes through the south pole of the sphere of radius 1024 about the origin and
    // meets it in a circle through that pole. One pair holds the arc from the pole to the point a
    // thirty-second of a half turn up from it. That point's start is given wide: its v reaches
    // down to the pole line and its place on the plane reaches the pole's, though its u does not.
    // Only a start held exactly on the pole line is the pole, whatever its u, so these are two
    // points and the one branch joins them.
    let r = 1024.0;
    let pi = core::f64::consts::PI;
    let t = 1.0 / 32.0;
    let a = pi * t;
    let ball = SurfaceKind::Sphere(
        Sphere::new(
            [0.0; 3],
            r,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-0.5, 0.0)),
        )
        .unwrap(),
    );
    // As (y, x).
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, -r],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 1.0],
            ((-512.0, 512.0), (-512.0, 512.0)),
        )
        .unwrap(),
    );
    let e = 1e-9;
    let u = (a / 2.0).tan().acos() / pi;
    let y = r * a.sin() * (pi * u).sin();
    let x = 2.0 * r * (a / 2.0).sin() * (a / 2.0).sin();
    let pole = [pt(0.5), pt(-0.5), pt(0.0), pt(0.0)];
    let up = [
        around(u, e),
        Interval::new(-0.5, -0.5 + 2.0 * t).unwrap(),
        Interval::new(-e, 2.0 * y + e).unwrap(),
        Interval::new(-e, 2.0 * x + e).unwrap(),
    ];
    let starts = vec![pole, up];
    let found = one_pair(
        ((0.4, 0.6), (-0.5, -0.5 + 3.0 * t)),
        ((-8.0, 256.0), (-8.0, 64.0)),
        starts.clone(),
    );
    let traced = march(&ball, &plane, &found, 8.0, 10.24, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    assert_eq!(joins(&traced, &starts), [[0, 1]], "{traced:?}");
    on_the_curve((&ball, &plane), &traced, 8.0, |[x, _, z]| x - z - r);
}
