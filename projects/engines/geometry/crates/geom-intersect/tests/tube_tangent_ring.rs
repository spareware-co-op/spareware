//! A tube about a centre off its frame's axes touches a coaxial cylinder in one ring (issue 1161).
//!
//! The tube is the surface of revolution of a circle of radius r about (p, q, 0), turned about the
//! z axis, with c² = p² + q² in whole numbers; the cylinder of radius c + r about the same axis
//! holds it and touches it along its outer equator, the circle of radius c + r at z = 0. The
//! closed form read the profile's centre in the half-plane of the axis through the half-plane
//! map, which rounds: its distance from the axis came out a hair under c, the cylinder's line
//! ρ = c + r then missed the circle's image, and the pair was answered as meeting nowhere. The
//! centre is read from the profile's own data now, exactly where they are exact, so the line
//! touches the circle and the ring is found.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{Basis, Circle, CurveKind, Frame, Line, ParamCurve, Trimmed};
use geom_intersect::quadrics::natural;
use geom_intersect::result::{CurveContact, Intersection, Item};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, Revolution, SurfaceKind};

const ROUND: (f64, f64) = (0.0, 2.0);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(50_000_000))
}

/// The tube about the z axis whose profile is the circle of radius `r` about (`p`, `q`, 0), in
/// the plane of the z axis and that centre.
fn tube(p: f64, q: f64, r: f64) -> SurfaceKind {
    let frame = Frame::new([0.0, 0.0, 1.0], [-q, p, 0.0]).unwrap();
    let circle = Circle::new([p, q, 0.0], r, frame, ROUND).unwrap();
    let profile = Trimmed::new(Basis::try_from(CurveKind::Circle(circle)).unwrap(), ROUND).unwrap();
    let up = Frame::standard().unwrap();
    SurfaceKind::Revolution(Revolution::new(profile, [0.0; 3], up, ROUND).unwrap())
}

/// The cylinder of radius `r` about the z axis.
fn wall(r: f64) -> SurfaceKind {
    let up = Frame::standard().unwrap();
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], r, up, (ROUND, (-1e6, 1e6))).unwrap())
}

/// That `found` is one ring where the two touch: the circle of radius `radius` about the z axis
/// at z = 0, exactly.
fn assert_tangent_ring(found: &Intersection, radius: f64) {
    let [Item::Curve { curve, contact, .. }] = found.items() else {
        panic!("not one ring: {:?}", found.items());
    };
    let CurveKind::Circle(ring) = curve.as_ref() else {
        panic!("not a circle: {curve:?}");
    };
    assert_eq!(ring.center(), [0.0; 3]);
    assert_eq!(ring.frame().normal(), [0.0, 0.0, 1.0]);
    assert_eq!(ring.radius(), radius);
    assert!(
        matches!(contact, CurveContact::Tangent { .. }),
        "{contact:?}"
    );
}

/// That the tube about (`p`, `q`, 0), which stands `c` from the axis, touches the cylinder that
/// holds it in its outer equator, for the issue's radii: 1, 3 and c/2.
fn assert_touches(p: f64, q: f64, c: f64) {
    for r in [1.0, 3.0, c / 2.0] {
        let found = natural(&wall(c + r), &tube(p, q, r), &mut meter())
            .unwrap()
            .unwrap();
        assert_tangent_ring(&found, c + r);
    }
}

#[test]
fn a_tube_about_15_36_touches_the_cylinder_that_holds_it_in_one_ring() {
    assert_touches(15.0, 36.0, 39.0);
}

#[test]
fn a_tube_about_165_144_touches_the_cylinder_that_holds_it_in_one_ring() {
    assert_touches(165.0, 144.0, 219.0);
}

#[test]
fn a_tube_about_352_135_touches_the_cylinder_that_holds_it_in_one_ring() {
    assert_touches(352.0, 135.0, 377.0);
}

#[test]
fn a_tube_about_3_4_touches_the_cylinder_that_holds_it_in_one_ring() {
    // The centre whose image the half-plane map rounded onto 5 itself: the ring was found before.
    assert_touches(3.0, 4.0, 5.0);
}

#[test]
fn a_tube_touches_the_straight_revolution_that_holds_it_in_one_ring() {
    // The cylinder of radius 40 as a surface of revolution, the segment from (40, 0, −3) to
    // (40, 0, 3) turned about the z axis, with the tube first: the pair is met circle with line,
    // the other way round from a cylinder's.
    let segment = CurveKind::Line(
        Line::segment([40.0, 0.0, -3.0], [40.0, 0.0, 3.0])
            .unwrap()
            .value,
    );
    let span = segment.domain();
    let profile = Trimmed::new(Basis::try_from(segment).unwrap(), span).unwrap();
    let up = Frame::standard().unwrap();
    let held = SurfaceKind::Revolution(Revolution::new(profile, [0.0; 3], up, ROUND).unwrap());
    let found = natural(&tube(15.0, 36.0, 1.0), &held, &mut meter())
        .unwrap()
        .unwrap();
    assert_tangent_ring(&found, 40.0);
}

#[test]
fn a_tube_clear_of_a_coaxial_cylinder_meets_it_nowhere() {
    // The tube about (15, 36, 0) of radius 1 reaches from 38 to 40 off the axis: the cylinder of
    // radius 41 holds it and the one of radius 37 stands in its hole, each clear of it by 1.
    for radius in [41.0, 37.0] {
        let found = natural(&wall(radius), &tube(15.0, 36.0, 1.0), &mut meter())
            .unwrap()
            .unwrap();
        assert!(found.items().is_empty(), "{:?}", found.items());
    }
}

#[test]
fn a_cylinder_a_hair_from_touching_a_tube_is_left_to_the_general_paths() {
    // The tube about (1, 1, 0) of radius 1 stands √2 from the axis, which no double is, so no
    // cylinder touches it. This one's radius is the double next under √2 + 1: the tube's outer
    // equator stands 1.3e-16 outside it, and the two cross in two rings, at z = ±1.6e-8. The
    // half-plane map's image of the centre stood at that radius less 1 exactly, and the pair was
    // answered as one ring where the two touch, at z = 0. No enclosure of the centre tells this
    // discriminant from zero, so the pair is the general paths'.
    let reach = f64::from_bits(0x4003_504F_333F_9DE6);
    assert_eq!(reach, 2.414_213_562_373_095);
    let found = natural(&wall(reach), &tube(1.0, 1.0, 1.0), &mut meter()).unwrap();
    assert!(found.is_none(), "{:?}", found.map(|r| r.items().to_vec()));
}
