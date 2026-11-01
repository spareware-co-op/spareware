//! Attacks on G166's curves: pairs of a sphere, cylinder and cone whose meetings a hand can count,
//! each branch sampled, and every sample's enclosure held against both surfaces' own equations.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_curve::Frame;
use geom_intersect::quartic::{Arc, Meeting, meet};
use geom_num::interval::Interval;
use geom_num::pencil::{Quartic, Segre};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Sphere, SurfaceKind};

const ROUND: (f64, f64) = (0.0, 2.0);
const TALL: (f64, f64) = (-5000.0, 5000.0);

fn frame_along(n: [f64; 3]) -> Frame<3> {
    let x = if n[0] == 0.0 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    Frame::new(x, n).unwrap()
}

fn sphere(c: [f64; 3], r: f64) -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(c, r, frame_along([0.0, 0.0, 1.0]), (ROUND, (-0.5, 0.5))).unwrap(),
    )
}

fn cylinder(c: [f64; 3], n: [f64; 3], r: f64) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new(c, r, frame_along(n), (ROUND, TALL)).unwrap())
}

fn cone(apex: [f64; 3], n: [f64; 3], k: f64) -> SurfaceKind {
    SurfaceKind::Cone(Cone::new(apex, 0.0, k, frame_along(n), (ROUND, TALL)).unwrap())
}

const Z: [f64; 3] = [0.0, 0.0, 1.0];
const X: [f64; 3] = [1.0, 0.0, 0.0];

fn meeting(a: &SurfaceKind, b: &SurfaceKind) -> Meeting {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    meet(a, b, &mut meter).unwrap().unwrap()
}

/// Whether the enclosure `p` can lie on `s`, by the surface's equation in intervals.
fn on(s: &SurfaceKind, p: [Interval; 3]) -> bool {
    let point = |v: f64| Interval::point(v).unwrap();
    let minus = |c: [f64; 3]| {
        [
            p[0].sub(point(c[0])).unwrap(),
            p[1].sub(point(c[1])).unwrap(),
            p[2].sub(point(c[2])).unwrap(),
        ]
    };
    let dot = |a: [Interval; 3], b: [Interval; 3]| {
        a[0].mul(b[0])
            .unwrap()
            .add(a[1].mul(b[1]).unwrap())
            .unwrap()
            .add(a[2].mul(b[2]).unwrap())
            .unwrap()
    };
    let axis = |n: [f64; 3]| {
        let size = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        [point(n[0] / size), point(n[1] / size), point(n[2] / size)]
    };
    let slack = |x: Interval| x.lo() <= 1e-6 && x.hi() >= -1e-6;
    match s {
        SurfaceKind::Sphere(x) => {
            let w = minus(x.center());
            let r = x.radius();
            slack(dot(w, w).sub(point(r * r)).unwrap())
        }
        SurfaceKind::Cylinder(x) => {
            let w = minus(x.center());
            let along = dot(w, axis(x.frame().normal()));
            let r = x.radius();
            slack(
                dot(w, w)
                    .sub(along.square().unwrap())
                    .unwrap()
                    .sub(point(r * r))
                    .unwrap(),
            )
        }
        SurfaceKind::Cone(x) => {
            let w = minus(x.center());
            let along = dot(w, axis(x.frame().normal()));
            let (_, k) = x.radius_and_slope();
            let turned = along.square().unwrap().mul(point(1.0 + k * k)).unwrap();
            slack(dot(w, w).sub(turned).unwrap())
        }
        _ => panic!("no equation"),
    }
}

/// Every sample of every branch lies on both, and how many samples there were.
fn sampled(a: &SurfaceKind, b: &SurfaceKind, found: &Meeting) -> usize {
    let curve = found.curve.as_ref().expect("a curve");
    let mut count = 0;
    let mut check = |theta: f64| {
        let t = Interval::point(theta).unwrap();
        // Inside a branch the discriminant is never below zero.
        let d = curve.discriminant(t).unwrap();
        assert!(d.hi() >= 0.0, "{d:?} below zero inside a branch at {theta}");
        for upper in [true, false] {
            if let Some(p) = curve.point(t, upper).unwrap() {
                assert!(on(a, p), "{p:?} off the first at {theta}");
                assert!(on(b, p), "{p:?} off the second at {theta}");
                count += 1;
            }
        }
    };
    for arc in &found.arcs {
        match arc {
            Arc::Whole => {
                for i in 0..64 {
                    check(f64::from(i) / 64.0 + 1e-3);
                }
            }
            Arc::Between { from, to } => {
                // Each end brackets its sign change of the discriminant: below zero before the
                // rise and after the fall, above it inside.
                let sign = |theta: f64| {
                    let d = curve
                        .discriminant(Interval::point(theta.fract()).unwrap())
                        .unwrap();
                    assert!(d.lo() > 0.0 || d.hi() < 0.0, "{d:?} at {theta}");
                    d.lo() > 0.0
                };
                assert!(!sign(from.lo()) && sign(from.hi()), "{from:?}");
                assert!(sign(to.lo()) && !sign(to.hi()), "{to:?}");
                let (lo, hi) = (from.hi(), to.lo());
                assert!(lo <= hi);
                for i in 1..16 {
                    check(lo + (hi - lo) * f64::from(i) / 16.0);
                }
            }
        }
    }
    count
}

fn two_ovals(a: &SurfaceKind, b: &SurfaceKind) {
    let found = meeting(a, b);
    assert_eq!(found.topology.quartic, Some(Quartic::Smooth { ovals: 2 }));
    assert!(
        found.arcs.len() == 2 || matches!(found.arcs[..], [Arc::Whole]),
        "{:?}",
        found.arcs
    );
    assert!(sampled(a, b, &found) > 8);
}

fn one_oval(a: &SurfaceKind, b: &SurfaceKind) {
    let found = meeting(a, b);
    assert_eq!(found.topology.quartic, Some(Quartic::Smooth { ovals: 1 }));
    assert_eq!(found.arcs.len(), 1);
    assert!(sampled(a, b, &found) > 8);
}

#[test]
fn a_thin_cylinder_through_a_sphere_leaves_two_loops() {
    two_ovals(&sphere([0.0; 3], 5.0), &cylinder([1.0, 0.0, 0.0], Z, 1.0));
    // In either order, and off the grid, where intervals decide.
    two_ovals(&cylinder([1.0, 0.0, 0.0], Z, 1.0), &sphere([0.0; 3], 5.0));
    two_ovals(
        &sphere([0.25, 0.5, 0.0], 5.3),
        &cylinder([1.1, 0.0, 0.0], Z, 1.7),
    );
}

#[test]
fn a_slanted_pipe_through_a_sphere_leaves_two_bounded_loops() {
    // Two ovals the walk reads as two arcs, each from a rise to a fall, not one whole branch.
    let (a, b) = (
        sphere([0.0; 3], 5.0),
        cylinder([2.0, 0.0, 0.0], [0.0, 1.0, 1.0], 1.5),
    );
    two_ovals(&a, &b);
    let found = meeting(&a, &b);
    assert_eq!(found.arcs.len(), 2);
    assert!(
        found
            .arcs
            .iter()
            .all(|arc| matches!(arc, Arc::Between { .. }))
    );
}

#[test]
fn a_cylinder_across_a_sphere_s_edge_leaves_one() {
    one_oval(&sphere([0.0; 3], 5.0), &cylinder([5.0, 0.0, 0.0], Z, 1.0));
    one_oval(
        &sphere([7.0, 3.0, 2.0], 5.0),
        &cylinder([12.0, 3.0, 2.0], Z, 1.0),
    );
}

#[test]
fn skew_pipes() {
    let a = cylinder([0.0; 3], Z, 1.0);
    // Equal, one apart: one loop. A thin pipe through a wide one: two.
    one_oval(&a, &cylinder([0.0, 1.0, 0.0], X, 1.0));
    two_ovals(&a, &cylinder([0.0, 0.25, 0.0], X, 2.0));
    // A loop all round but for a stretch of the walk one sample wide: its arc wraps past the turn.
    one_oval(&a, &cylinder([0.0, 0.302, 0.0], X, 1.3));
    // Slanted axes, whole direction.
    one_oval(&a, &cylinder([3.0, 0.0, 0.0], [3.0, 4.0, 0.0], 3.0));
}

#[test]
fn viviani_s_window_crosses_itself() {
    let (a, b) = (sphere([0.0; 3], 2.0), cylinder([1.0, 0.0, 0.0], Z, 1.0));
    let found = meeting(&a, &b);
    assert_eq!(found.segre, Segre::Double);
    assert_eq!(found.topology.quartic, Some(Quartic::Crossing));
    assert_eq!(found.arcs.len(), 1);
    assert!(sampled(&a, &b, &found) > 8);
}

#[test]
fn a_cone_and_a_cylinder() {
    let c = cone([0.0; 3], Z, 1.0);
    // The cylinder holds the apex: a node there.
    let tube = cylinder([1.0, 0.0, 0.0], Z, 1.0);
    let found = meeting(&c, &tube);
    assert_eq!(found.topology.quartic, Some(Quartic::Crossing));
    assert!(sampled(&c, &tube, &found) > 8);
    // A cylinder off the axis that holds the axis inside it, cutting both nappes.
    let tube = cylinder([1.0, 0.0, 0.0], Z, 2.0);
    two_ovals(&c, &tube);
    // Off to one side, wide enough to reach the cone, off the grid.
    let side = cylinder([0.0, 3.1, 0.0], X, 3.7);
    let found = meeting(&c, &side);
    assert!(found.topology.quartic.is_some());
    assert!(sampled(&c, &side, &found) > 8);
}

#[test]
fn a_cone_and_a_sphere_off_its_axis() {
    let c = cone([0.0; 3], Z, 1.0);
    two_ovals(&c, &sphere([1.0, 0.0, 0.0], 3.0));
    let s = sphere([0.0, 0.0, 6.0], 3.0);
    // Round the axis: the closed forms' case, two circles, no quartic and no curve.
    let found = meeting(&c, &s);
    assert!(found.curve.is_none());
}

#[test]
fn two_cones_apart() {
    let a = cone([0.0; 3], Z, 1.0);
    let b = cone([5.0, 0.0, 0.0], [1.0, 1.0, 0.0], 1.0);
    let found = meeting(&a, &b);
    assert!(found.topology.quartic.is_some(), "{:?}", found.topology);
    assert!(sampled(&a, &b, &found) > 4);
}

#[test]
fn a_cubic_and_its_line_are_walked_too() {
    let c = cone([0.0; 3], Z, 1.0);
    let tube = cylinder([0.0, 1.0, 0.0], [1.0, 0.0, 1.0], 1.0);
    let found = meeting(&c, &tube);
    assert_eq!(found.topology.cubics, 1);
    assert_eq!(found.arcs.len(), 1, "{:?}", found.arcs);
    assert!(sampled(&c, &tube, &found) > 4);
}

#[test]
fn conics_are_left_to_the_closed_forms() {
    // Two pipes of one radius square to each other meet in two ellipses: a pencil member is ruled,
    // but no quartic or cubic is there to walk, so there is no curve.
    let found = meeting(&cylinder([0.0; 3], Z, 1.0), &cylinder([0.0; 3], X, 1.0));
    assert_eq!(found.segre, Segre::Pair);
    assert_eq!(found.topology.conics, 2);
    assert!(found.curve.is_none());
    assert_eq!(found.arcs.len(), 0);
}

#[test]
fn other_kinds_and_unreadable_pairs_are_none() {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let plane = SurfaceKind::Plane(
        geom_surface::Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-1.0, 1.0), (-1.0, 1.0)),
        )
        .unwrap(),
    );
    let s = sphere([0.0; 3], 1.0);
    assert!(meet(&plane, &s, &mut meter).unwrap().is_none());
    assert!(meet(&s, &plane, &mut meter).unwrap().is_none());
    // Tangent, off the grid: no interval tells a double root.
    let a = sphere([0.0; 3], 1.5);
    let b = cylinder([0.75, 0.0, 0.0], Z, 0.75);
    assert!(meet(&a, &b, &mut meter).unwrap().is_none());
}

#[test]
fn a_budget_stops_the_walk() {
    let (a, b) = (sphere([0.0; 3], 5.0), cylinder([1.0, 0.0, 0.0], Z, 1.0));
    let mut meter = WorkMeter::new(Budget::new(3, 0));
    assert!(meet(&a, &b, &mut meter).is_err());
}
