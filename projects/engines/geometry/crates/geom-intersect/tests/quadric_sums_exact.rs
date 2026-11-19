//! A stored vector added onto a grid point for the closed forms of two quadrics must be added
//! exactly (issue #NNNN). Those closed forms take a plane as three grid points, its origin c and
//! c + a and c + b, and an axis as two, its centre c and c + n, n the frame's stored normal (a
//! cone's apex and apex + n alike). The far points were summed in doubles and then checked to be
//! whole, but near 2^39 a double holds only 2^-13, so a vector's fraction under half of that
//! rounds away and the sum lands on the grid: the closed form was handed another plane or axis
//! from the stored one, and answered about that other surface. Issues #1058 and #1063 found the
//! same in a line's sum and in a curve-surface pair's axis.
//!
//! Every case here stands at 2^39 with a stored vector whose fraction is 2^-15 there, so its sum
//! rounds to the vector without the fraction: a plane's vector (2^-15, 1, 0) to (0, 1, 0), and a
//! frame's normal (2^-15, 0, 1) to (0, 0, 1), upright. Each true meeting is worked by hand below.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{CurveKind, Frame};
use geom_intersect::quadrics::natural;
use geom_intersect::result::{CurveContact, Intersection, Item};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind};

/// 2^39: where a double holds a coordinate only to 2^-13.
const FAR: f64 = 549_755_813_888.0;

/// 2^20, the radius of the sphere and the cylinders.
const R: f64 = 1_048_576.0;

/// 2^-15, a stored vector's fraction, under half of 2^-13 so `FAR + TILT` rounds to `FAR`.
const TILT: f64 = 1.0 / 32_768.0;

/// How near a point a curve is read to pass, in steps: well under the 2^5 or more each fault
/// moves the meeting by, and well over the enclosures' rounding at 2^39.
const NEAR: f64 = 0.01;

const ALL: (f64, f64) = (f64::NEG_INFINITY, f64::INFINITY);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(50_000_000))
}

/// √(1 + TILT²): the length of a stored vector with the fraction.
fn length() -> f64 {
    (1.0 + TILT * TILT).sqrt()
}

/// The frame whose stored normal is (2^-15, 0, 1), its x̂ = (0, 1, 0), so the seam stands on +y,
/// away from every meeting here.
fn tilted() -> Frame<3> {
    Frame::new([0.0, 1.0, 0.0], [TILT, 0.0, 1.0]).unwrap()
}

/// The plane through `c` along `a` and `b`, without end.
fn plane(c: [f64; 3], a: [f64; 3], b: [f64; 3]) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(c, a, b, (ALL, ALL)).unwrap())
}

fn sub(p: [f64; 3], q: [f64; 3]) -> [f64; 3] {
    [p[0] - q[0], p[1] - q[1], p[2] - q[2]]
}

fn dot(p: [f64; 3], q: [f64; 3]) -> f64 {
    p[0] * q[0] + p[1] * q[1] + p[2] * q[2]
}

/// How far `p` stands from the line through `o` along `d`.
fn off(p: [f64; 3], o: [f64; 3], d: [f64; 3]) -> f64 {
    let w = sub(p, o);
    let along = dot(w, d) / dot(d, d);
    let foot = [
        w[0] - along * d[0],
        w[1] - along * d[1],
        w[2] - along * d[2],
    ];
    dot(foot, foot).sqrt()
}

/// Each curve item's curve and contact; any other item fails.
fn curves(r: &Intersection) -> Vec<(CurveKind<3>, CurveContact)> {
    r.items()
        .iter()
        .map(|i| match i {
            Item::Curve { curve, contact, .. } => (curve.as_ref().clone(), *contact),
            other => panic!("not a curve: {other:?}"),
        })
        .collect()
}

/// Each line item's origin and direction, crossing; any other item fails.
fn crossing_lines(r: &Intersection) -> Vec<([f64; 3], [f64; 3])> {
    curves(r)
        .into_iter()
        .map(|(curve, contact)| {
            assert_eq!(contact, CurveContact::Crossing, "{curve:?}");
            let CurveKind::Line(l) = curve else {
                panic!("not a line: {curve:?}");
            };
            (l.origin(), l.direction())
        })
        .collect()
}

/// `sheet`'s far points. The plane through (2^39, 0, 0) along (2^-15, 1, 0) and (0, 0, 1) is
/// x = 2^39 + 2^-15·y; its far point c + a rounded to (2^39, 1, 0), the plane x = 2^39. The
/// sphere of radius 2^20 about (2^39 + 2^20 + 2^14, 2^30, 0) stands 2^20 + 2^14 off that plane,
/// so the closed form found nothing. The stored plane passes 2^15 nearer at y = 2^30: the centre
/// stands h = (2^20 − 2^14)/√(1 + 2^-30) off it (worked by hand), short of the radius, so they
/// meet in a circle of radius √(R² − h²), about 184,640, about the foot of the centre on the plane.
#[test]
fn a_plane_whose_vector_rounds_onto_the_grid_cuts_a_sphere_in_a_circle() {
    let sheet = plane([FAR, 0.0, 0.0], [TILT, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let centre = [FAR + R + 16_384.0, 1_073_741_824.0, 0.0];
    let ball = SurfaceKind::Sphere(
        Sphere::new(
            centre,
            R,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    );
    let found = curves(&natural(&sheet, &ball, &mut meter()).unwrap().unwrap());
    assert_eq!(found.len(), 1, "{found:?}");
    let (curve, contact) = &found[0];
    assert_eq!(*contact, CurveContact::Crossing, "{found:?}");
    let CurveKind::Circle(c) = curve else {
        panic!("not a circle: {found:?}");
    };
    let normal = [1.0 / length(), -TILT / length(), 0.0];
    let h = (R - 16_384.0) / length();
    let foot = [
        centre[0] - h * normal[0],
        centre[1] - h * normal[1],
        centre[2],
    ];
    let gap = sub(c.center(), foot);
    assert!(dot(gap, gap).sqrt() < NEAR, "{found:?}");
    let radius = (R * R - h * h).sqrt();
    assert!((c.radius() - radius).abs() < NEAR, "{found:?}");
}

/// `axis`'s far point. Two cylinders of radius 2^20 about (2^39, 0, 0) and (2^39 + 2^21, 0, 0),
/// both on the stored normal (2^-15, 0, 1): about the upright axis the first's c + n rounded to,
/// the second's centre stands exactly 2^21 off, the sum of the radii, so the closed form found one
/// tangent line. The stored axes stand 2^21/√(1 + 2^-30) apart (the offset (2^21, 0, 0) less its
/// part along the axis), short of 2^21, so the cylinders cross in two lines, halfway between the
/// axes and R·2^-15/√(1 + 2^-30), just short of 32, either side of their plane (worked by hand).
#[test]
fn cylinders_on_axes_that_round_upright_cross_in_two_lines() {
    let tube =
        |x: f64| Cylinder::new([x, 0.0, 0.0], R, tilted(), ((0.0, 2.0), (-64.0, 64.0))).unwrap();
    let (a, b) = (tube(FAR), tube(FAR + 2.0 * R));
    let side = R * TILT / length();
    for (p, q) in [(&a, &b), (&b, &a)] {
        let (p, q) = (
            SurfaceKind::Cylinder(p.clone()),
            SurfaceKind::Cylinder(q.clone()),
        );
        let mut found = crossing_lines(&natural(&p, &q, &mut meter()).unwrap().unwrap());
        assert_eq!(found.len(), 2, "{found:?}");
        found.sort_by(|s, t| s.0[1].total_cmp(&t.0[1]));
        for ((o, d), y) in found.iter().zip([-side, side]) {
            for c in [&a, &b] {
                // The line's own direction, and each axis, along the stored normal.
                assert!(
                    off(*o, c.center(), c.frame().normal()) - R < NEAR,
                    "{found:?}"
                );
                assert!(
                    R - off(*o, c.center(), c.frame().normal()) < NEAR,
                    "{found:?}"
                );
            }
            assert!(off([FAR + R, y, 0.0], *o, *d) < NEAR, "{found:?}");
        }
    }
}

/// `funnel`'s far point. The cone of slope 1 with its apex at (2^39, 0, 0), on the stored normal
/// (2^-15, 0, 1), over heights 16 to 1,024 on one nappe; the plane x − z = 2^39 through the apex,
/// along (1, 0, 1) and (0, 1, 0). About the upright axis apex + n rounded to, the plane holds the
/// ruling along (1, 0, 1): the closed form found it, one tangent line. The stored cone leans 2^-15
/// toward +x, so the plane cuts it in two lines through the apex: d = α·(1, 0, 1)/√2 + β·(0, 1, 0)
/// lies on it where (d·ê)² = |d|²/2, ê = (2^-15, 0, 1)/√(1 + 2^-30), so β/α = ±√(2·2^-15/(1 +
/// 2^-30)) (worked by hand), and each runs in y by ±2·√(2^-15/(1 + 2^-30)), about 0.011, for each
/// unit in x: about ±11 at the top of the domain.
#[test]
fn a_plane_through_the_apex_of_a_cone_whose_axis_rounds_upright_cuts_it_in_two_lines() {
    let apex = [FAR, 0.0, 0.0];
    let cone = SurfaceKind::Cone(
        Cone::new(apex, 0.0, 1.0, tilted(), ((0.0, 2.0), (16.0, 1024.0))).unwrap(),
    );
    let sheet = plane(apex, [1.0, 0.0, 1.0], [0.0, 1.0, 0.0]);
    let mut found = crossing_lines(&natural(&sheet, &cone, &mut meter()).unwrap().unwrap());
    assert_eq!(found.len(), 2, "{found:?}");
    found.sort_by(|s, t| (s.1[1] / s.1[0]).total_cmp(&(t.1[1] / t.1[0])));
    let run = 2.0 * (TILT / (1.0 + TILT * TILT)).sqrt();
    for ((o, d), sign) in found.iter().zip([-1.0, 1.0]) {
        assert!(off(apex, *o, *d) < NEAR, "{found:?}");
        assert!((d[1] / d[0] - sign * run).abs() < 1e-9, "{found:?}");
        assert!((d[2] / d[0] - 1.0).abs() < 1e-9, "{found:?}");
    }
}

/// `plane_plane`'s far points of its second plane. The planes x = 2^39 and x = 2^39 + 2^-15·y,
/// both through (2^39, 0, 0): the second's far point o + u rounded to (2^39, 1, 0), onto the
/// first, so the closed form found them one plane. They cross in the line x = 2^39, y = 0, along z.
#[test]
fn a_plane_whose_vector_rounds_onto_another_plane_crosses_it_in_a_line() {
    let upright = plane([FAR, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let leaning = plane([FAR, 0.0, 0.0], [TILT, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let found = crossing_lines(&natural(&upright, &leaning, &mut meter()).unwrap().unwrap());
    assert_eq!(found.len(), 1, "{found:?}");
    let (o, d) = found[0];
    assert!(off([FAR, 0.0, 0.0], o, d) < NEAR, "{found:?}");
    assert!(off([FAR, 0.0, 1_000_000.0], o, d) < NEAR, "{found:?}");
}
