//! Two cylinders on parallel axes a hair apart, through [`natural`] (issue 984). The axis runs along
//! (0.6, 0.8, 0), whose parts are not doubles, so a centre 300 along it in decimals,
//! (180, 240, 0), stands about 1.3e-14 off the stored axis through the origin. Rounding at the size
//! of the centres swallows that offset, and the closed form takes it from exact sums instead.
//!
//! Each case is asked both ways round, so each cylinder is once the one the offset is measured
//! from. The two-line cases use a small radius: trimming runs the engine on each end circle
//! against the other cylinder, which it lies a hair from all round, and that costs it in
//! proportion to the circle (at radius 10, about 22 million work units and most of a minute a
//! call in a debug build; at a thousandth, a quarter of a million).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{CurveKind, Frame};
use geom_intersect::quadrics::natural;
use geom_intersect::result::{Intersection, Item};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, SurfaceKind};

/// The axis, along (0.6, 0.8, 0).
const AXIS: [f64; 3] = [0.6, 0.8, 0.0];

/// The centre 300 along the axis, in decimals: a hair off the stored axis through the origin.
const FAR: [f64; 3] = [180.0, 240.0, 0.0];

/// An axis off every coordinate plane, along (0.36, 0.48, 0.8): each part of the offset between
/// the axes is then a sum of products that are not zero.
const ASKEW: [f64; 3] = [0.36, 0.48, 0.8];

/// The centre 300 along it, in decimals.
const ASKEW_FAR: [f64; 3] = [108.0, 144.0, 240.0];

/// The radius of the two-line cases.
const SMALL: f64 = 1e-3;

fn tube(axis: [f64; 3], centre: [f64; 3], radius: f64, v: (f64, f64)) -> SurfaceKind {
    let frame = Frame::new([0.0, 0.0, 1.0], axis).unwrap();
    SurfaceKind::Cylinder(Cylinder::new(centre, radius, frame, ((0.0, 2.0), v)).unwrap())
}

/// One cylinder of a pair: its surface, centre and radius.
type Side = (SurfaceKind, [f64; 3], f64);

/// The pair along `axis` (one about `far`, 300 along it, of radius `r`, one about the origin of
/// radius `q`) over one shared stretch of the axis ten long, both ways round.
fn pair(axis: [f64; 3], far: [f64; 3], r: f64, q: f64) -> [(Side, Side); 2] {
    let a = (tube(axis, far, r, (-305.0, -295.0)), far, r);
    let b = (tube(axis, [0.0; 3], q, (-5.0, 5.0)), [0.0; 3], q);
    [(a.clone(), b.clone()), (b, a)]
}

fn meet(a: &SurfaceKind, b: &SurfaceKind) -> Intersection {
    let mut meter = WorkMeter::new(Budget::of_work(50_000_000));
    natural(a, b, &mut meter).unwrap().unwrap()
}

fn sub(p: [f64; 3], q: [f64; 3]) -> [f64; 3] {
    [p[0] - q[0], p[1] - q[1], p[2] - q[2]]
}

fn cross(p: [f64; 3], q: [f64; 3]) -> [f64; 3] {
    [
        p[1] * q[2] - p[2] * q[1],
        p[2] * q[0] - p[0] * q[2],
        p[0] * q[1] - p[1] * q[0],
    ]
}

fn size(p: [f64; 3]) -> f64 {
    (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
}

/// The part of `p − c` square to `axis`.
fn square_part(axis: [f64; 3], p: [f64; 3], c: [f64; 3]) -> [f64; 3] {
    let w = sub(p, c);
    let e = axis.map(|x| x / size(axis));
    let along = w[0] * e[0] + w[1] * e[1] + w[2] * e[2];
    [
        w[0] - along * e[0],
        w[1] - along * e[1],
        w[2] - along * e[2],
    ]
}

/// The answer's items, each checked to be a crossing line along the axis on both cylinders;
/// their points nearest the origin's axis.
fn lines_on_both(
    found: &Intersection,
    axis: [f64; 3],
    sides: [(&[f64; 3], f64); 2],
) -> Vec<[f64; 3]> {
    let mut at = Vec::new();
    for item in found.items() {
        let Item::Curve { curve, .. } = item else {
            panic!("not a curve: {item:?}");
        };
        let CurveKind::Line(l) = curve.as_ref() else {
            panic!("not a line: {curve:?}");
        };
        let d = l.direction();
        let skew = size(cross(d, axis)) / size(d);
        assert!(skew < 1e-12, "a line {skew} off the axis's direction");
        for (c, radius) in sides {
            let off = size(square_part(axis, l.origin(), *c)) - radius;
            assert!(off.abs() < 1e-11, "{off} off a cylinder of radius {radius}");
        }
        at.push(square_part(axis, l.origin(), [0.0; 3]));
    }
    at
}

/// The two lines stand one each side of the plane of the axes, a diameter apart, and their middle
/// is the foot on the offset between the axes, d/2 + (r² − q²)/2d from the first, which stands
/// `(near, far)` from the axis through the origin.
fn one_each_side(at: &[[f64; 3]], r: f64, (near, far): (f64, f64)) {
    let [p, q] = at else {
        panic!("{} lines", at.len());
    };
    let apart = size(sub(*p, *q));
    assert!((apart - 2.0 * r).abs() < 1e-11, "{apart} apart");
    let middle = size([0, 1, 2].map(|i| f64::midpoint(p[i], q[i])));
    assert!(
        middle >= near && middle < far,
        "their middle {middle} off the axes"
    );
}

/// The search's input `022d3c0009030002000000090305` in small: equal cylinders on parallel axes a
/// hair apart meet in two lines, one each side of the plane of the axes.
#[test]
fn equal_cylinders_on_parallel_axes_a_hair_apart_meet_in_two_lines() {
    for ((a, c, r), (b, g, q)) in pair(AXIS, FAR, SMALL, SMALL) {
        let found = meet(&a, &b);
        let at = lines_on_both(&found, AXIS, [(&c, r), (&g, q)]);
        // Equal radii: the foot is half the hair, on both axes to a rounding of the centres.
        one_each_side(&at, SMALL, (0.0, 1e-11));
    }
}

/// The same axes, with radii two steps apart: less than the hair between the axes, so neither
/// cylinder lies inside the other and they still cross in two lines.
#[test]
fn cylinders_whose_radii_differ_by_less_than_the_hair_between_their_axes_meet_in_two_lines() {
    let smaller = SMALL.next_down().next_down();
    for ((a, c, r), (b, g, q)) in pair(AXIS, FAR, SMALL, smaller) {
        let found = meet(&a, &b);
        let at = lines_on_both(&found, AXIS, [(&c, r), (&g, q)]);
        // (r² − q²)/2d: about 8.7e-22 over twice a hair of about 1.3e-14, some 3.3e-8 from the
        // axes.
        one_each_side(&at, SMALL, (2e-8, 5e-8));
    }
}

/// The same axes, with radii 10 and 5: one inside the other, so they meet nowhere.
#[test]
fn unequal_cylinders_on_parallel_axes_a_hair_apart_meet_nowhere() {
    let (a, b) = (
        tube(AXIS, FAR, 10.0, (-600.0, 0.0)),
        tube(AXIS, [0.0; 3], 5.0, (-300.0, 300.0)),
    );
    for (p, q) in [(&a, &b), (&b, &a)] {
        let found = meet(p, q);
        assert!(found.items().is_empty(), "{:?}", found.items());
    }
}

/// Equal cylinders a hair apart on an axis off every coordinate plane meet in two lines: here no
/// part of the offset between the axes is zero, so each of its exact sums counts.
#[test]
fn equal_cylinders_on_askew_axes_a_hair_apart_meet_in_two_lines() {
    for ((a, c, r), (b, g, q)) in pair(ASKEW, ASKEW_FAR, SMALL, SMALL) {
        let found = meet(&a, &b);
        let at = lines_on_both(&found, ASKEW, [(&c, r), (&g, q)]);
        one_each_side(&at, SMALL, (0.0, 1e-11));
    }
}
