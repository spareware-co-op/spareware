//! The march held to what each case is known to give (chapter 12.8): settled pairs built by hand,
//! so that each step of the march meets the one input it decides on. A start a march reaches
//! beside a pole, the edge a piece runs along, the end of a pair's patch a first step must stay
//! inside, and the starts two branches end at close together all show in the branches it gives:
//! how many, which starts each joins, how each ends, and that none is left unproven.
//!
//! Every case's curves are known exactly: a plane through a sphere's centre meets it in a great
//! circle, the plane z = 0 meets the unit cylinder about z in its circle z = 0, two coordinate
//! planes meet in an axis, and the plane x = −g meets the cone x² + y² = z² in a hyperbola.

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
use geom_intersect::general::{Settled, Subdivided, subdivide};
use geom_intersect::march::{Traced, march};
use geom_intersect::result::CurveEnd;
use geom_intersect::seam::Passing;
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

/// z = 0 as (x, y).
fn ground(domain: ((f64, f64), (f64, f64))) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], domain).unwrap())
}

/// x = `x` as (y, z), times `scale`.
fn wall(x: f64, scale: f64, domain: ((f64, f64), (f64, f64))) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new([x, 0.0, 0.0], [0.0, scale, 0.0], [0.0, 0.0, scale], domain).unwrap(),
    )
}

/// The unit cylinder about the z axis.
fn tube(domain: ((f64, f64), (f64, f64))) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 1.0, Frame::standard().unwrap(), domain).unwrap())
}

/// The sphere of radius 1024 about the origin over v in `v`.
fn ball(v: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(
            [0.0; 3],
            1024.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), v),
        )
        .unwrap(),
    )
}

#[test]
fn the_circle_of_a_plane_and_a_cylinder_leaves_a_patch_in_two_arcs() {
    // The circle x² + y² = 1 of z = 0 meets the plane's patch x ≤ 0.95, |y| ≤ 0.9 in two arcs:
    // from Q (x = 0.95, y < 0) to P (y = −0.9), and from R (x = 0.95, y > 0) to T (y = 0.9). The
    // starts are given R, Q, P, T, so the march from R passes P and Q, both done, on its way.
    let plane = ground(((-2.0, 2.0), (-2.0, 2.0)));
    let cyl = tube(((-1.0, 1.0), (-1.0, 1.0)));
    let h = 1e-12;
    let on = |y: f64, x: f64| {
        let u = y.atan2(x) / core::f64::consts::PI;
        [around(x, h), around(y, h), around(u, h), around(0.0, h)]
    };
    let x9 = (1.0f64 - 0.81).sqrt();
    let y95 = (1.0f64 - 0.9025).sqrt();
    let (p, q, r, t) = (on(-0.9, x9), on(-y95, 0.95), on(y95, 0.95), on(0.9, x9));
    let starts = vec![r, q, p, t];
    let found = one_pair(
        ((0.0, 0.95), (-0.9, 0.9)),
        ((-0.4, 0.4), (-0.5, 0.5)),
        starts.clone(),
    );
    let traced = march(&plane, &cyl, &found, 1e-3, 1e-3, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    // R and T, Q and P.
    assert_eq!(joins(&traced, &starts), vec![[0, 3], [1, 2]], "{traced:?}");
    for branch in &traced.branches {
        assert_eq!(branch.ends, [CurveEnd::Flagged; 2], "{branch:?}");
    }
    on_the_curve((&plane, &cyl), &traced, 1e-3, |[x, y, z]| {
        (x * x + y * y - 1.0).abs() + z.abs()
    });
}

#[test]
fn a_cylinders_end_circle_lying_in_a_strip_of_a_plane_is_two_arcs_along_the_edge() {
    // The cylinder's end circle v = 0 lies in z = 0; the plane's strip sin π/8 ≤ y ≤ sin 3π/8 holds
    // the arcs u in [1/8, 3/8] and [5/8, 7/8], each a piece along the cylinder's edge from one start
    // to the nearest other on it.
    let pi = core::f64::consts::PI;
    let (y0, y1) = ((pi * 0.125).sin(), (pi * 0.375).sin());
    let plane = ground(((-2.0, 2.0), (y0, y1)));
    let cyl = tube(((0.0, 1.0), (0.0, 1.0)));
    let w = 1.0 / 1_073_741_824.0;
    let start = |u: f64| {
        let a = pi * u;
        [
            around(u, w),
            pt(0.0),
            around(a.cos(), 1e-6),
            around(a.sin(), 1e-6),
        ]
    };
    let starts = vec![start(0.375), start(0.125), start(0.625), start(0.875)];
    let found = one_pair(
        ((0.1, 0.9), (0.0, 1.0)),
        ((-2.0, 2.0), (y0, y1)),
        starts.clone(),
    );
    let traced = march(&cyl, &plane, &found, 1e-3, 1e-3, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    assert_eq!(joins(&traced, &starts), vec![[0, 1], [2, 3]], "{traced:?}");
    for branch in &traced.branches {
        assert_eq!(branch.ends, [CurveEnd::Boundary; 2], "{branch:?}");
    }
    on_the_curve((&cyl, &plane), &traced, 1e-3, |[x, y, z]| {
        (x * x + y * y - 1.0).abs() + z.abs()
    });
}

#[test]
fn two_starts_on_an_axis_are_one_open_piece_not_a_loop() {
    // z = 0 and x = 0 meet in the y axis; starts at y = 0 and y = 0.005, given wide enough to
    // overlap, are two points of a line, so the branch between them is open.
    let plane = ground(((-2.0, 2.0), (-2.0, 2.0)));
    let side = wall(0.0, 1.0, ((-2.0, 2.0), (-2.0, 2.0)));
    let e = 1e-3;
    let f = [
        around(0.0, e),
        around(0.0, 0.004),
        around(0.0, 0.004),
        around(0.0, e),
    ];
    let s = [
        around(0.0, e),
        around(0.005, 0.004),
        around(0.005, 0.004),
        around(0.0, e),
    ];
    let found = one_pair(
        ((-1.0, 1.0), (-1.0, 1.0)),
        ((-1.0, 1.0), (-1.0, 1.0)),
        vec![f, s],
    );
    let traced = march(&plane, &side, &found, 0.01, 0.01, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    assert_eq!(traced.branches.len(), 1, "{traced:?}");
    let branch = &traced.branches[0];
    assert_eq!(branch.vertices.len(), 2, "{branch:?}");
    assert_eq!(branch.ends, [CurveEnd::Flagged; 2], "{branch:?}");
}

#[test]
fn a_great_circle_from_a_spheres_pole_is_marched_from_either_start() {
    // x = 0 through the centre of the sphere of radius 1024 holds the meridians u = 1/2 and 3/2;
    // one pair holds the arc of u = 1/2 from the south pole to v = −1/4, its starts in either order.
    let r = 1024.0;
    let ball = ball((-0.5, 0.5));
    let side = wall(0.0, 1.0, ((-2048.0, 2048.0), (-2048.0, 2048.0)));
    let c = (core::f64::consts::PI * 0.25).cos() * r;
    let pole = [pt(0.5), pt(-0.5), pt(0.0), pt(-r)];
    let top = [pt(0.5), pt(-0.25), around(c, 1e-9), around(-c, 1e-9)];
    for (starts, ends) in [
        (vec![pole, top], [CurveEnd::Singular, CurveEnd::Flagged]),
        (vec![top, pole], [CurveEnd::Flagged, CurveEnd::Singular]),
    ] {
        let found = one_pair(
            ((0.4, 0.6), (-0.5, -0.25)),
            ((-2048.0, 2048.0), (-2048.0, 2048.0)),
            starts,
        );
        let traced = march(&ball, &side, &found, 8.0, 10.24, &mut meter()).unwrap();
        assert!(traced.unproven.is_empty(), "{traced:?}");
        assert_eq!(traced.branches.len(), 1, "{traced:?}");
        assert_eq!(traced.branches[0].ends, ends, "{traced:?}");
        on_the_curve((&ball, &side), &traced, 8.0, |[x, ..]| x);
    }
    // And the whole circle, from the search: one loop through both poles.
    let found = subdivide(&ball, &side, 8.0, &mut meter()).unwrap();
    let traced = march(&ball, &side, &found, 8.0, 10.24, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    assert_eq!(traced.branches.len(), 1, "{traced:?}");
    let branch = &traced.branches[0];
    assert_eq!(branch.ends, [CurveEnd::Closed; 2], "{branch:?}");
    let poles = branch
        .passes
        .iter()
        .filter(|p| p.passing == Passing::Pole)
        .count();
    assert_eq!(poles, 2, "{branch:?}");
    on_the_curve((&ball, &side), &traced, 8.0, |[x, ..]| x);
}

#[test]
fn a_cap_holds_one_branch_through_its_pole() {
    // The cap v ≤ `top` of the sphere of radius 1024 meets x = 0 in one arc over the pole: down
    // u = 1/2 from a, through the pole, up u = 3/2 to b. Two starts at the pole, p1 and p2, are the
    // one point the branch passes, whichever is given first, and at the plane's scale of 1 or 10⁻⁴.
    let pi = core::f64::consts::PI;
    let r = 1024.0;
    for (top, scale) in [(-0.25, 1.0), (-0.45, 1.0), (-0.25, 1e-4), (-0.45, 1e-4)] {
        let ball = ball((-0.5, top));
        let big = 2048.0 / scale;
        let side = wall(0.0, scale, ((-big, big), (-big, big)));
        let (c, z) = ((pi * top).cos() * r, (pi * top).sin() * r);
        let w = 1e-9 / scale;
        let a = [pt(0.5), pt(top), around(c / scale, w), around(z / scale, w)];
        let b = [
            pt(1.5),
            pt(top),
            around(-c / scale, w),
            around(z / scale, w),
        ];
        let p1 = [pt(0.5), pt(-0.5), pt(0.0), pt(-r / scale)];
        let p2 = [pt(1.5), pt(-0.5), pt(0.0), pt(-r / scale)];
        for starts in [vec![a, b, p1, p2], vec![a, b, p2, p1]] {
            let found = one_pair(
                ((0.25, 1.75), (-0.5, top)),
                ((-big, big), (-big, big)),
                starts,
            );
            let traced = march(&ball, &side, &found, 8.0, 10.24, &mut meter()).unwrap();
            assert!(traced.unproven.is_empty(), "{top} {scale}: {traced:?}");
            assert_eq!(traced.branches.len(), 1, "{top} {scale}: {traced:?}");
            let branch = &traced.branches[0];
            assert_eq!(branch.ends, [CurveEnd::Boundary; 2], "{branch:?}");
            let ends = [
                mid(branch.vertices[0][0]),
                mid(branch.vertices.last().unwrap()[0]),
            ];
            assert!(ends == [0.5, 1.5] || ends == [1.5, 0.5], "{branch:?}");
            assert_eq!(branch.passes.len(), 1, "{branch:?}");
            assert_eq!(branch.passes[0].passing, Passing::Pole, "{branch:?}");
            on_the_curve((&ball, &side), &traced, 8.0, |[x, ..]| x);
        }
    }
}

#[test]
fn two_arms_of_a_hyperbola_end_at_their_own_starts_however_close() {
    // x = −g meets the cone x² + y² = z² in the hyperbola v cos πu = −g, y = v sin πu, z = v: over
    // u near 1 its arm u < 1 has y > 0 and its arm u > 1 has y < 0. The patch v ≥ 0.064 cuts each arm
    // once at the top, v = 4, and once at the bottom, where the arms are 2√(0.064² − g²) < 2 floors
    // apart: both bottom starts are within twice the floor of where each arm ends, and each arm ends
    // at its own, the nearer in parameters.
    let pi = core::f64::consts::PI;
    let g = 1.0 / 16.0;
    let (low, high) = (0.064, 4.0);
    let cone = SurfaceKind::Cone(
        Cone::new(
            [0.0; 3],
            0.0,
            1.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, high)),
        )
        .unwrap(),
    );
    let side = wall(-g, 1.0, ((-8.0, 8.0), (-8.0, 8.0)));
    let h = 1e-12;
    let start = |v: f64, arm: f64| {
        let turn = (g / v).acos() / pi;
        let y = (v * v - g * g).sqrt();
        [
            around(1.0 - arm * turn, h),
            pt(v),
            around(arm * y, h),
            pt(v),
        ]
    };
    let (t1, t2, b1, b2) = (
        start(high, 1.0),
        start(high, -1.0),
        start(low, 1.0),
        start(low, -1.0),
    );
    let floor = 1.0 / 64.0;
    assert!(mid(b1[2]) - mid(b2[2]) <= 2.0 * floor);
    for starts in [vec![t1, t2, b1, b2], vec![t1, t2, b2, b1]] {
        let found = one_pair(
            ((0.25, 1.75), (low, high)),
            ((-8.0, 8.0), (-8.0, 8.0)),
            starts.clone(),
        );
        let traced = march(&cone, &side, &found, floor, floor, &mut meter()).unwrap();
        assert!(traced.unproven.is_empty(), "{traced:?}");
        // Each arm joins its top start to the bottom start of its own sign of y.
        for branch in &traced.branches {
            let (first, last) = (&branch.vertices[0], branch.vertices.last().unwrap());
            assert!(mid(first[2]) * mid(last[2]) > 0.0, "{branch:?}");
        }
        let mut ends: Vec<_> = joins(&traced, &starts)
            .into_iter()
            .map(|[a, b]| {
                let sign = |k: usize| mid(starts[k][2]) > 0.0;
                (sign(a), sign(b))
            })
            .collect();
        ends.sort_unstable();
        assert_eq!(ends, vec![(false, false), (true, true)], "{traced:?}");
        on_the_curve((&cone, &side), &traced, floor, |[x, y, z]| {
            (x + g).abs() + (x * x + y * y - z * z).abs()
        });
    }
}

#[test]
fn a_first_step_that_fits_the_domain_only_at_the_smallest_is_taken() {
    // z = 0 over y ≥ 0 and x = 0 meet in the y axis. A pair holds it from A at y = 0, on the first
    // plane's domain edge, to B at y = L. The march starts a floor F along, so a step's box, half
    // again as long as the step behind it and F/1024 plus a 2⁻²⁰th of the step more, stays inside
    // y ≥ 0 only for steps at most F(1 − 2⁻¹⁰)/(1/2 + 2⁻²⁰). With F = 3/4 · 2⁻²⁰ that holds for the
    // smallest step, L · 2⁻²⁰, and not for twice it: the first step is the smallest, and the march
    // goes on from it to B.
    let len = 1.0 + 1.0 / 4_194_304.0;
    let floor = 0.75 / 1_048_576.0;
    let plane = ground(((-1.0, 1.0), (0.0, 2.0)));
    let side = wall(0.0, 1.0, ((-1.0, 3.0), (-1.0, 1.0)));
    let a = [pt(0.0); 4];
    let b = [pt(0.0), pt(len), pt(len), pt(0.0)];
    let starts = vec![a, b];
    let found = one_pair(
        ((-1.0, 1.0), (0.0, len)),
        ((-1.0, 3.0), (-1.0, 1.0)),
        starts.clone(),
    );
    let traced = march(&plane, &side, &found, floor, 1.0, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    assert_eq!(joins(&traced, &starts), vec![[0, 1]], "{traced:?}");
    assert_eq!(
        traced.branches[0].ends,
        [CurveEnd::Boundary, CurveEnd::Flagged],
        "{traced:?}"
    );
    on_the_curve((&plane, &side), &traced, floor, |[x, _, z]| {
        x.abs() + z.abs()
    });
}
