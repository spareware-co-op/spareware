//! Attacks on G167: one test per row of chapter 12.7 past the natural quadrics, each result read off
//! geometry a hand can check: tori square to, through and at Villarceau's angle to a plane, about a
//! quadric or another torus; revolutions cut by planes; extrusions cut by planes and by each other.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_curve::{Basis, Circle, CurveKind, Frame, Line, ParamCurve, Trimmed};
use geom_intersect::quadrics::natural;
use geom_intersect::result::{Coincidence, CurveContact, Intersection, Item};
use geom_nurbs::{Curve, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Extrusion, Plane, Revolution, Sphere, SurfaceKind, Torus};

const ROUND: (f64, f64) = (0.0, 2.0);
const TALL: (f64, f64) = (-500.0, 500.0);
const WIDE: (f64, f64) = (-1000.0, 1000.0);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn meet(a: &SurfaceKind, b: &SurfaceKind) -> Intersection {
    natural(a, b, &mut meter()).unwrap().unwrap()
}

fn up() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap()
}

fn plane(c: [f64; 3], a: [f64; 3], b: [f64; 3]) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(c, a, b, (WIDE, WIDE)).unwrap())
}

fn level(z: f64) -> SurfaceKind {
    plane([0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
}

fn torus(z: f64, major: f64, minor: f64) -> SurfaceKind {
    SurfaceKind::Torus(Torus::new([0.0, 0.0, z], major, minor, up(), (ROUND, ROUND)).unwrap())
}

fn sphere(z: f64, r: f64) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere::new([0.0, 0.0, z], r, up(), (ROUND, (-0.5, 0.5))).unwrap())
}

fn cylinder(r: f64) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], r, up(), (ROUND, TALL)).unwrap())
}

fn cone(z: f64, k: f64) -> SurfaceKind {
    SurfaceKind::Cone(Cone::new([0.0, 0.0, z], 0.0, k, up(), (ROUND, TALL)).unwrap())
}

fn curves(r: &Intersection) -> Vec<&CurveKind<3>> {
    r.items()
        .iter()
        .filter_map(|i| match i {
            Item::Curve { curve, .. } => Some(&**curve),
            _ => None,
        })
        .collect()
}

/// The radii and heights of the circles of `r`, sorted.
fn rings(r: &Intersection) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = curves(r)
        .iter()
        .map(|c| match c {
            CurveKind::Circle(c) => (c.radius(), c.center()[2]),
            other => panic!("{other:?}"),
        })
        .collect();
    out.sort_by(f64::total_cmp_pair);
    out
}

trait Pair {
    fn total_cmp_pair(a: &(f64, f64), b: &(f64, f64)) -> core::cmp::Ordering;
}

impl Pair for f64 {
    fn total_cmp_pair(a: &(f64, f64), b: &(f64, f64)) -> core::cmp::Ordering {
        a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1))
    }
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-3, "{a} and {b}");
}

/// The circles of `r` are these, to within the certified reach.
fn rings_are(r: &Intersection, want: &[(f64, f64)]) {
    let got = rings(r);
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want) {
        near(g.0, w.0);
        near(g.1, w.1);
    }
}

fn tangent(r: &Intersection) -> bool {
    r.items().iter().all(|i| {
        matches!(
            i,
            Item::Curve {
                contact: CurveContact::Tangent { .. },
                ..
            }
        )
    })
}

/// Samples of a curve.
fn samples(c: &CurveKind<3>) -> Vec<[f64; 3]> {
    let (lo, hi) = c.domain();
    (0..=8)
        .map(|i| {
            let t = lo + (hi - lo) * f64::from(i) / 8.0;
            c.point(t, Limit::FromRight).unwrap()
        })
        .collect()
}

/// How far `p` stands from the torus at the origin.
fn off_torus(p: [f64; 3], major: f64, minor: f64) -> f64 {
    let rho = (p[0] * p[0] + p[1] * p[1]).sqrt();
    ((rho - major).powi(2) + p[2] * p[2]).sqrt() - minor
}

#[test]
fn a_plane_square_to_a_torus() {
    let t = torus(0.0, 5.0, 1.0);
    let r = meet(&level(0.0), &t);
    rings_are(&r, &[(4.0, 0.0), (6.0, 0.0)]);
    let r = meet(&level(1.0), &t);
    assert_eq!(rings(&r).len(), 1);
    near(rings(&r)[0].0, 5.0);
    assert!(tangent(&r));
    assert_eq!(meet(&level(2.0), &t).items().len(), 0);
    let r = meet(&level(0.6), &t);
    let got = rings(&r);
    near(got[0].0, 4.2);
    near(got[1].0, 5.8);
}

#[test]
fn a_plane_through_a_torus_s_axis() {
    let t = torus(0.0, 5.0, 1.0);
    let r = meet(&plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), &t);
    let c = curves(&r);
    assert_eq!(c.len(), 2);
    for curve in c {
        let CurveKind::Circle(circle) = curve else {
            panic!("{curve:?}");
        };
        near(circle.radius(), 1.0);
        near(circle.center()[0].abs(), 5.0);
        for p in samples(curve) {
            near(off_torus(p, 5.0, 1.0), 0.0);
        }
    }
    // Off the axis it is no special position.
    let off = plane([0.0, 2.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    assert!(natural(&off, &t, &mut meter()).unwrap().is_none());
}

#[test]
fn villarceau_s_two_circles() {
    // R = 5, r = 3: sin θ = 3/5, a plane through the centre with slope 3/4.
    let t = torus(0.0, 5.0, 3.0);
    let cut = plane([0.0; 3], [4.0, 0.0, 3.0], [0.0, 1.0, 0.0]);
    let r = meet(&cut, &t);
    let c = curves(&r);
    assert_eq!(c.len(), 2);
    for curve in c {
        let CurveKind::Circle(circle) = curve else {
            panic!("{curve:?}");
        };
        near(circle.radius(), 5.0);
        near(circle.center()[1].abs(), 3.0);
        near(circle.center()[0], 0.0);
        for p in samples(curve) {
            near(off_torus(p, 5.0, 3.0), 0.0);
            near(p[0] * 3.0 - p[2] * 4.0, 0.0);
        }
    }
    // One step off the angle is general position, and so is a parallel plane.
    let steeper = plane([0.0; 3], [4.0, 0.0, 2.0], [0.0, 1.0, 0.0]);
    assert!(natural(&steeper, &t, &mut meter()).unwrap().is_none());
    let shifted = plane([0.0, 0.0, 1.0], [4.0, 0.0, 3.0], [0.0, 1.0, 0.0]);
    assert!(natural(&shifted, &t, &mut meter()).unwrap().is_none());
    // A torus off the grid has no exact angle to read.
    let round = torus(0.0, 5.5, 3.3);
    assert!(natural(&cut, &round, &mut meter()).unwrap().is_none());
}

#[test]
fn quadrics_about_a_torus_s_axis() {
    let t = torus(0.0, 5.0, 1.0);
    // Cylinders through the tube, touching it, and past it.
    let r = meet(&cylinder(5.0), &t);
    rings_are(&r, &[(5.0, -1.0), (5.0, 1.0)]);
    let r = meet(&cylinder(6.0), &t);
    rings_are(&r, &[(6.0, 0.0)]);
    assert!(tangent(&r));
    assert_eq!(meet(&cylinder(7.0), &t).items().len(), 0);
    // Spheres about its centre.
    let r = meet(&sphere(0.0, 6.0), &t);
    rings_are(&r, &[(6.0, 0.0)]);
    assert!(tangent(&r));
    assert_eq!(meet(&sphere(0.0, 3.0), &t).items().len(), 0);
    let r = meet(&sphere(0.0, 5.0), &t);
    assert_eq!(rings(&r).len(), 2);
    for (radius, z) in rings(&r) {
        near(radius * radius + z * z, 25.0);
    }
    // Off the axis there is no ring.
    let aside =
        SurfaceKind::Sphere(Sphere::new([1.0, 0.0, 0.0], 5.0, up(), (ROUND, (-0.5, 0.5))).unwrap());
    assert!(natural(&aside, &t, &mut meter()).unwrap().is_none());
    // A cone from its centre: slope 1 cuts the tube in the two ends of a 45-degree chord, each nappe.
    let r = meet(&cone(0.0, 1.0), &torus(0.0, 2.0, 1.5));
    assert_eq!(rings(&r).len(), 4);
    for (radius, z) in rings(&r) {
        near(radius, z.abs());
    }
}

#[test]
fn two_tori_about_one_axis() {
    let a = torus(0.0, 5.0, 1.0);
    let r = meet(&a, &torus(1.0, 5.0, 1.0));
    assert_eq!(rings(&r).len(), 2);
    let r = meet(&a, &torus(2.0, 5.0, 1.0));
    rings_are(&r, &[(5.0, 1.0)]);
    assert!(tangent(&r));
    assert_eq!(meet(&a, &torus(3.0, 5.0, 1.0)).items().len(), 0);
    let same = meet(&a, &torus(0.0, 5.0, 1.0));
    assert!(matches!(
        same.items(),
        [Item::Overlap {
            proof: Coincidence::Identical,
            ..
        }]
    ));
}

fn trimmed(c: CurveKind<3>) -> Trimmed<3> {
    let (lo, hi) = c.domain();
    Trimmed::new(Basis::try_from(c).unwrap(), (lo, hi)).unwrap()
}

/// The surface of a segment turned about z.
fn cup() -> SurfaceKind {
    let line = Line::segment([1.0, 0.0, 0.0], [3.0, 0.0, 4.0])
        .unwrap()
        .value;
    let profile = trimmed(CurveKind::Line(line));
    SurfaceKind::Revolution(Revolution::new(profile, [0.0; 3], up(), ROUND).unwrap())
}

/// A quadratic in the plane y = 0, turned about z.
fn vase() -> SurfaceKind {
    let basis = geom_nurbs::Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let points = [[2.0, 0.0, 0.0], [6.0, 0.0, 2.0], [2.0, 0.0, 4.0]];
    let profile = Curve::new(basis, &points).unwrap();
    let kind = CurveKind::Nurbs(profile);
    SurfaceKind::Revolution(Revolution::new(trimmed(kind), [0.0; 3], up(), ROUND).unwrap())
}

#[test]
fn a_plane_square_to_a_revolution_cuts_a_circle_at_each_root() {
    let r = meet(&level(2.0), &cup());
    assert_eq!(rings(&r).len(), 1);
    near(rings(&r)[0].0, 2.0);
    assert_eq!(meet(&level(5.0), &cup()).items().len(), 0);
    // The quadratic's middle reaches ρ = 4 at z = 2.
    let r = meet(&level(2.0), &vase());
    near(rings(&r)[0].0, 4.0);
    near(rings(&r)[0].1, 2.0);
}

#[test]
fn a_quadric_about_a_revolution() {
    // The quadratic at ρ = 3 twice, once on each side of z = 2.
    let r = meet(&cylinder(3.0), &vase());
    assert_eq!(rings(&r).len(), 2);
    for (radius, z) in rings(&r) {
        near(radius, 3.0);
        near((z - 2.0).abs(), 2.0_f64.sqrt());
    }
    // A cylinder holding the profile's whole straight side would overlap; a sphere meets the cone.
    let r = meet(&sphere(0.0, 5.0), &cup());
    assert_eq!(rings(&r).len(), 1);
}

#[test]
fn a_plane_through_a_revolution_s_axis_cuts_two_copies() {
    let cut = plane([0.0; 3], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let r = meet(&cut, &cup());
    let c = curves(&r);
    assert_eq!(c.len(), 2);
    for curve in &c {
        assert!(matches!(curve, CurveKind::Line(_)), "{curve:?}");
        let [a, .., b] = samples(curve)[..] else {
            panic!();
        };
        near(a[0], 0.0);
        near(a[1].abs().min(b[1].abs()), 1.0);
        near(a[1].abs().max(b[1].abs()), 3.0);
    }
    let r = meet(&cut, &vase());
    let c = curves(&r);
    assert_eq!(c.len(), 2);
    for curve in &c {
        assert!(
            matches!(curve, CurveKind::Nurbs(_) | CurveKind::Trimmed(_)),
            "{curve:?}"
        );
    }
}

/// A box wall: a segment pushed along z.
fn wall(a: [f64; 3], b: [f64; 3]) -> SurfaceKind {
    let line = Line::segment(a, b).unwrap().value;
    SurfaceKind::Extrusion(
        Extrusion::new(trimmed(CurveKind::Line(line)), [0.0, 0.0, 1.0], (0.0, 10.0)).unwrap(),
    )
}

/// A quarter of a circle of radius 5 pushed along z.
fn curved_wall() -> SurfaceKind {
    let arc = Circle::new([0.0; 3], 5.0, up(), (0.0, 0.5)).unwrap();
    SurfaceKind::Extrusion(
        Extrusion::new(
            trimmed(CurveKind::Circle(arc)),
            [0.0, 0.0, 1.0],
            (0.0, 10.0),
        )
        .unwrap(),
    )
}

#[test]
fn a_plane_along_an_extrusion_cuts_lines() {
    let cut = plane([3.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let r = meet(&cut, &curved_wall());
    let c = curves(&r);
    assert_eq!(c.len(), 1);
    let CurveKind::Line(line) = c[0] else {
        panic!("{:?}", c[0]);
    };
    near(line.origin()[0], 3.0);
    near(line.origin()[1], 4.0);
    assert_eq!(line.direction(), [0.0, 0.0, 1.0]);
    // Past the profile's end: no line.
    let past = plane([6.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    assert_eq!(meet(&past, &curved_wall()).items().len(), 0);
    // A wall in the plane is the plane.
    let same = meet(
        &plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        &wall([0.0; 3], [5.0, 0.0, 0.0]),
    );
    assert!(matches!(same.items(), [Item::Overlap { .. }]));
}

#[test]
fn a_plane_across_an_extrusion_cuts_a_copy_of_the_profile() {
    // Square: the profile itself at that height.
    let r = meet(&level(4.0), &wall([0.0; 3], [5.0, 0.0, 0.0]));
    let c = curves(&r);
    assert_eq!(c.len(), 1);
    assert!(matches!(c[0], CurveKind::Line(_)));
    for p in samples(c[0]) {
        near(p[2], 4.0);
        near(p[1], 0.0);
    }
    // Tilted: lines stay lines, an arc becomes an ellipse on the plane.
    let tilted = plane([0.0, 0.0, 5.0], [1.0, 0.0, 0.25], [0.0, 1.0, 0.0]);
    let r = meet(&tilted, &curved_wall());
    let c = curves(&r);
    assert_eq!(c.len(), 1);
    assert!(matches!(c[0], CurveKind::Ellipse(_)), "{:?}", c[0]);
    for p in samples(c[0]) {
        near(p[0] * p[0] + p[1] * p[1], 25.0);
        near(p[2], 5.0 + p[0] * 0.25);
    }
    // Past the sweep's end at one edge: a shorter stretch of the copy.
    let steep = plane([0.0, 0.0, 8.0], [1.0, 0.0, 1.0], [0.0, 1.0, 0.0]);
    let r = meet(&steep, &curved_wall());
    let c = curves(&r);
    assert_eq!(c.len(), 1);
    for p in samples(c[0]) {
        assert!(p[2] <= 10.0 + 1e-6, "{p:?}");
        near(p[0] * p[0] + p[1] * p[1], 25.0);
    }
    assert_eq!(
        meet(&level(20.0), &wall([0.0; 3], [5.0, 0.0, 0.0]))
            .items()
            .len(),
        0
    );
}

#[test]
fn two_sweeps_of_one_direction_meet_in_lines() {
    let a = wall([0.0, -5.0, 0.0], [0.0, 5.0, 0.0]);
    let b = wall([-5.0, 1.0, 0.0], [5.0, 1.0, 0.0]);
    let r = meet(&a, &b);
    let c = curves(&r);
    assert_eq!(c.len(), 1);
    let CurveKind::Line(line) = c[0] else {
        panic!("{:?}", c[0]);
    };
    near(line.origin()[0], 0.0);
    near(line.origin()[1], 1.0);
    let (lo, hi) = line.domain();
    near(hi - lo, 10.0);
    // A curved wall with a straight one, and with a cylinder of its own direction.
    let r = meet(&wall([3.0, -5.0, 0.0], [3.0, 5.0, 0.0]), &curved_wall());
    assert_eq!(curves(&r).len(), 1);
    let tube =
        SurfaceKind::Cylinder(Cylinder::new([5.0, 0.0, 0.0], 5.0, up(), (ROUND, TALL)).unwrap());
    let r = meet(&tube, &curved_wall());
    let c = curves(&r);
    assert_eq!(c.len(), 1);
    // Crossing lines through (2.5, ±4.33): the arc's first quadrant keeps one.
    let CurveKind::Line(line) = c[0] else {
        panic!("{:?}", c[0]);
    };
    near(line.origin()[0], 2.5);
    // Walls of two directions are no sweep of one.
    let other = SurfaceKind::Extrusion(
        Extrusion::new(
            trimmed(CurveKind::Line(
                Line::segment([0.0; 3], [5.0, 0.0, 0.0]).unwrap().value,
            )),
            [0.0, 1.0, 0.0],
            (0.0, 10.0),
        )
        .unwrap(),
    );
    assert!(natural(&a, &other, &mut meter()).unwrap().is_none());
}

#[test]
fn a_part_of_a_turn_is_left_to_the_general_paths() {
    let half =
        SurfaceKind::Torus(Torus::new([0.0; 3], 5.0, 1.0, up(), ((0.0, 1.0), ROUND)).unwrap());
    assert!(natural(&level(0.0), &half, &mut meter()).unwrap().is_none());
}
