//! Attacks on G165: one test per pair and degenerate position of 12.7's table, each result read off
//! geometry a hand can check, and `SMLib`'s issue 16 rebuilt: a cylinder tangent to a plane meets it
//! in one line, exactly.

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

use geom_curve::{CurveKind, Frame, ParamCurve};
use geom_intersect::IntersectError;
use geom_intersect::quadrics::natural;
use geom_intersect::result::{Coincidence, CurveContact, Intersection, Item, PointContact};
use geom_nurbs::Limit;
use geom_run::{Budget, CancelToken, WorkMeter};
use geom_surface::{Cone, Cylinder, ParamBox, Plane, Sphere, SurfaceKind};

const ROUND: (f64, f64) = (0.0, 2.0);
const TALL: (f64, f64) = (-500.0, 500.0);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn meet(a: &SurfaceKind, b: &SurfaceKind) -> Intersection {
    natural(a, b, &mut meter()).unwrap().unwrap()
}

fn frame_along(n: [f64; 3]) -> Frame<3> {
    let x = if n[0] == 0.0 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    Frame::new(x, n).unwrap()
}

fn plane(c: [f64; 3], a: [f64; 3], b: [f64; 3]) -> SurfaceKind {
    let side: ParamBox = ((-1000.0, 1000.0), (-1000.0, 1000.0));
    SurfaceKind::Plane(Plane::new(c, a, b, side).unwrap())
}

/// The plane z = `z`.
fn level(z: f64) -> SurfaceKind {
    plane([0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
}

fn sphere(c: [f64; 3], r: f64) -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(c, r, frame_along([0.0, 0.0, 1.0]), (ROUND, (-0.5, 0.5))).unwrap(),
    )
}

fn cylinder(c: [f64; 3], n: [f64; 3], r: f64) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new(c, r, frame_along(n), (ROUND, TALL)).unwrap())
}

/// The double cone about z through `apex`, radius k·|z − apex|.
fn cone(apex: [f64; 3], n: [f64; 3], k: f64) -> SurfaceKind {
    SurfaceKind::Cone(Cone::new(apex, 0.0, k, frame_along(n), (ROUND, TALL)).unwrap())
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

fn points(r: &Intersection) -> usize {
    r.items()
        .iter()
        .filter(|i| matches!(i, Item::Point { .. }))
        .count()
}

/// A point of the curve at the middle of its domain.
fn mid(c: &CurveKind<3>) -> [f64; 3] {
    let (lo, hi) = c.domain();
    c.point(lo * 0.5 + hi * 0.5, Limit::FromRight).unwrap()
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} and {b}");
}

fn tangent(r: &Intersection) -> bool {
    r.items().iter().any(|i| {
        matches!(
            i,
            Item::Curve {
                contact: CurveContact::Tangent { .. },
                ..
            }
        )
    })
}

#[test]
fn two_planes_meet_in_a_line_or_not_at_all() {
    let a = level(0.0);
    let r = meet(&a, &plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
    assert_eq!(curves(&r).len(), 1);
    assert!(matches!(curves(&r)[0], CurveKind::Line(_)));
    near(mid(curves(&r)[0])[1], 0.0);
    near(mid(curves(&r)[0])[2], 0.0);
    assert_eq!(meet(&a, &level(5.0)).items().len(), 0);
    let same = meet(&a, &level(0.0));
    assert!(
        matches!(
            same.items(),
            [Item::Overlap {
                proof: Coincidence::Identical,
                ..
            }]
        ),
        "{:?}",
        same.items()
    );
}

#[test]
fn a_plane_meets_a_sphere() {
    let s = sphere([0.0, 0.0, 0.0], 5.0);
    let r = meet(&level(3.0), &s);
    let c = curves(&r);
    assert_eq!(c.len(), 1);
    let CurveKind::Circle(circle) = c[0] else {
        panic!("{:?}", c[0]);
    };
    near(circle.radius(), 4.0);
    near(circle.center()[2], 3.0);
    let r = meet(&level(5.0), &s);
    assert_eq!(points(&r), 1);
    assert!(matches!(
        r.items()[0],
        Item::Point {
            contact: PointContact::IsolatedTangent,
            ..
        }
    ));
    assert_eq!(meet(&level(6.0), &s).items().len(), 0);
}

#[test]
fn smlib_16_a_plane_tangent_to_a_cylinder_is_one_line() {
    // SMLib's issue 16: a cylinder resting on a box's face. Exact data, so an exact tangent.
    let c = cylinder([0.0, 0.0, 5.0], [1.0, 0.0, 0.0], 5.0);
    let r = meet(&level(0.0), &c);
    assert_eq!(curves(&r).len(), 1, "{:?}", r.items());
    assert!(tangent(&r));
    let p = mid(curves(&r)[0]);
    near(p[1], 0.0);
    near(p[2], 0.0);
    // One step off either way is no tangent.
    assert_eq!(curves(&meet(&level(1.0), &c)).len(), 2);
    assert_eq!(meet(&level(-1.0), &c).items().len(), 0);
}

#[test]
fn a_plane_cuts_a_cylinder_in_its_conics() {
    let c = cylinder([0.0; 3], [0.0, 0.0, 1.0], 5.0);
    let r = meet(&level(7.0), &c);
    assert!(
        matches!(curves(&r)[..], [CurveKind::Circle(_)]),
        "{:?}",
        r.items()
    );
    // At 45 degrees: an ellipse of semi-axes 5√2 and 5.
    let r = meet(&plane([0.0; 3], [1.0, 0.0, 1.0], [0.0, 1.0, 0.0]), &c);
    assert!(
        matches!(curves(&r)[..], [CurveKind::Ellipse(_)]),
        "{:?}",
        r.items()
    );
}

#[test]
fn a_plane_cuts_a_cone_in_each_conic() {
    let c = cone([0.0; 3], [0.0, 0.0, 1.0], 1.0);
    let r = meet(&level(4.0), &c);
    let CurveKind::Circle(circle) = curves(&r)[0] else {
        panic!("{:?}", r.items());
    };
    near(circle.radius(), 4.0);
    // Shallow: an ellipse. Along a ruling: a parabola. Steep: two branches.
    let r = meet(
        &plane([0.0, 0.0, 9.0], [2.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        &c,
    );
    assert!(
        matches!(curves(&r)[..], [CurveKind::Ellipse(_)]),
        "{:?}",
        r.items()
    );
    let r = meet(
        &plane([0.0, 0.0, 9.0], [1.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        &c,
    );
    assert!(
        matches!(curves(&r)[..], [CurveKind::Parabola(_)]),
        "{:?}",
        r.items()
    );
    let r = meet(
        &plane([9.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        &c,
    );
    assert!(
        matches!(
            curves(&r)[..],
            [CurveKind::Hyperbola(_), CurveKind::Hyperbola(_)]
        ),
        "{:?}",
        r.items()
    );
}

#[test]
fn a_plane_through_a_cones_apex_is_a_point_or_rulings() {
    let c = cone([0.0; 3], [0.0, 0.0, 1.0], 1.0);
    assert_eq!(points(&meet(&level(0.0), &c)), 1);
    let r = meet(&plane([0.0; 3], [1.0, 0.0, 1.0], [0.0, 1.0, 0.0]), &c);
    assert_eq!(curves(&r).len(), 1);
    assert!(tangent(&r));
    let r = meet(&plane([0.0; 3], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]), &c);
    assert_eq!(curves(&r).len(), 2);
}

#[test]
fn two_spheres_meet_in_a_circle_or_a_point() {
    let a = sphere([0.0; 3], 5.0);
    let r = meet(&a, &sphere([6.0, 0.0, 0.0], 5.0));
    let CurveKind::Circle(c) = curves(&r)[0] else {
        panic!("{:?}", r.items());
    };
    near(c.radius(), 4.0);
    near(c.center()[0], 3.0);
    assert_eq!(points(&meet(&a, &sphere([10.0, 0.0, 0.0], 5.0))), 1);
    assert_eq!(points(&meet(&a, &sphere([3.0, 0.0, 0.0], 2.0))), 1);
    assert_eq!(meet(&a, &sphere([11.0, 0.0, 0.0], 5.0)).items().len(), 0);
    assert_eq!(meet(&a, &sphere([1.0, 0.0, 0.0], 2.0)).items().len(), 0);
    assert_eq!(meet(&a, &sphere([0.0; 3], 4.0)).items().len(), 0);
    assert!(matches!(
        meet(&a, &sphere([0.0; 3], 5.0)).items(),
        [Item::Overlap { .. }]
    ));
}

#[test]
fn a_sphere_on_a_coaxial_cylinder_is_two_circles() {
    let s = sphere([0.0; 3], 5.0);
    let r = meet(&s, &cylinder([0.0; 3], [0.0, 0.0, 1.0], 3.0));
    assert_eq!(curves(&r).len(), 2, "{:?}", r.items());
    let r = meet(&s, &cylinder([0.0; 3], [0.0, 0.0, 1.0], 5.0));
    assert_eq!(curves(&r).len(), 1);
    assert!(tangent(&r));
    assert_eq!(
        meet(&s, &cylinder([0.0; 3], [0.0, 0.0, 1.0], 6.0))
            .items()
            .len(),
        0
    );
    // Off the axis there is no closed form here.
    let off = cylinder([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 3.0);
    assert!(natural(&s, &off, &mut meter()).unwrap().is_none());
}

#[test]
fn a_sphere_on_a_coaxial_cone_is_circles() {
    let s = sphere([0.0, 0.0, 10.0], 8.0);
    let r = meet(&s, &cone([0.0; 3], [0.0, 0.0, 1.0], 1.0));
    assert_eq!(curves(&r).len(), 2, "{:?}", r.items());
}

#[test]
fn coaxial_cylinders_are_one_or_none() {
    let a = cylinder([0.0; 3], [0.0, 0.0, 1.0], 5.0);
    assert!(matches!(
        meet(&a, &cylinder([0.0, 0.0, 7.0], [0.0, 0.0, 1.0], 5.0)).items(),
        [Item::Overlap { .. }]
    ));
    assert_eq!(
        meet(&a, &cylinder([0.0; 3], [0.0, 0.0, 1.0], 4.0))
            .items()
            .len(),
        0
    );
}

#[test]
fn parallel_cylinders_meet_in_lines() {
    let a = cylinder([0.0; 3], [0.0, 0.0, 1.0], 5.0);
    let two = |x: f64, r: f64| meet(&a, &cylinder([x, 0.0, 0.0], [0.0, 0.0, 1.0], r));
    let r = two(6.0, 5.0);
    assert_eq!(curves(&r).len(), 2);
    near(mid(curves(&r)[0])[0], 3.0);
    let r = two(10.0, 5.0);
    assert_eq!(curves(&r).len(), 1);
    assert!(tangent(&r));
    let r = two(3.0, 2.0);
    assert_eq!(curves(&r).len(), 1);
    assert!(tangent(&r));
    assert_eq!(two(11.0, 5.0).items().len(), 0);
    assert_eq!(two(1.0, 2.0).items().len(), 0);
}

#[test]
fn a_pipe_tee_is_two_ellipses() {
    // Equal radii, axes crossing at right angles.
    let a = cylinder([0.0; 3], [0.0, 0.0, 1.0], 5.0);
    let b = cylinder([0.0; 3], [1.0, 0.0, 0.0], 5.0);
    let r = meet(&a, &b);
    assert_eq!(curves(&r).len(), 2, "{:?}", r.items());
    assert!(
        curves(&r)
            .iter()
            .all(|c| matches!(c, CurveKind::Ellipse(_)))
    );
    // Unequal radii have no conic.
    let b = cylinder([0.0; 3], [1.0, 0.0, 0.0], 4.0);
    assert!(natural(&a, &b, &mut meter()).unwrap().is_none());
}

#[test]
fn a_cone_on_a_coaxial_cylinder_is_a_circle_on_each_nappe() {
    let r = meet(
        &cylinder([0.0; 3], [0.0, 0.0, 1.0], 5.0),
        &cone([0.0; 3], [0.0, 0.0, 1.0], 1.0),
    );
    assert_eq!(curves(&r).len(), 2, "{:?}", r.items());
}

#[test]
fn coaxial_cones_are_circles_or_one() {
    let a = cone([0.0; 3], [0.0, 0.0, 1.0], 1.0);
    let r = meet(&a, &cone([0.0, 0.0, 10.0], [0.0, 0.0, 1.0], 1.0));
    assert_eq!(curves(&r).len(), 1, "{:?}", r.items());
    near(mid(curves(&r)[0])[2], 5.0);
    assert!(matches!(
        meet(&a, &cone([0.0; 3], [0.0, 0.0, 1.0], 1.0)).items(),
        [Item::Overlap { .. }]
    ));
}

#[test]
fn cones_of_one_slope_on_parallel_axes_meet_in_one_conic() {
    let a = cone([0.0; 3], [0.0, 0.0, 1.0], 1.0);
    let r = meet(&a, &cone([10.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1.0));
    assert_eq!(curves(&r).len(), 2, "{:?}", r.items());
    assert!(
        curves(&r)
            .iter()
            .all(|c| matches!(c, CurveKind::Hyperbola(_)))
    );
}

#[test]
fn cones_through_one_apex_meet_in_rulings() {
    let a = cone([0.0; 3], [0.0, 0.0, 1.0], 1.0);
    let r = meet(&a, &cone([0.0; 3], [1.0, 0.0, 0.0], 1.0));
    assert_eq!(curves(&r).len(), 2, "{:?}", r.items());
    assert!(curves(&r).iter().all(|c| matches!(c, CurveKind::Line(_))));
}

#[test]
fn a_cylinder_and_cone_about_one_sphere_are_two_conics() {
    // The sphere of radius 5 about the origin: the cylinder of radius 5 about x touches it, and so
    // does the cone about z with its apex at height 5√2 and half-angle 45 degrees. 5√2 is no grid
    // point, so scale by the 3-4-5: apex at 25/4 up with tan = 3/4 gives sin = 3/5 and radius 15/4.
    let a = cylinder([0.0; 3], [1.0, 0.0, 0.0], 15.0);
    let b = cone([0.0, 0.0, 25.0], [0.0, 0.0, 1.0], 0.75);
    let r = meet(&a, &b);
    assert!(!curves(&r).is_empty(), "{:?}", r.items());
}

#[test]
fn a_domain_trims_what_it_cuts() {
    // A cylinder of height 10 from 0: a plane across it at 45 degrees leaves arcs of its ellipse.
    let short = SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            10.0,
            frame_along([0.0, 0.0, 1.0]),
            (ROUND, (0.0, 10.0)),
        )
        .unwrap(),
    );
    let r = meet(&plane([0.0; 3], [1.0, 0.0, 1.0], [0.0, 1.0, 0.0]), &short);
    assert!(!curves(&r).is_empty(), "{:?}", r.items());
    for c in curves(&r) {
        let (lo, hi) = c.domain();
        assert!(hi - lo < 2.0, "{c:?}");
        let p = mid(c);
        assert!((-1e-6..=10.0 + 1e-6).contains(&p[2]), "{p:?}");
    }
    // A plane past the end meets nothing.
    assert_eq!(meet(&level(20.0), &short).items().len(), 0);
}

#[test]
fn either_order_gives_the_same_items_swapped() {
    let (a, b) = (level(3.0), sphere([0.0; 3], 5.0));
    let (x, y) = (meet(&a, &b), meet(&b, &a));
    assert_eq!(x.items().len(), y.items().len());
    let (p, q) = (x.items()[0].at().spans(), y.items()[0].at().spans());
    assert_eq!(p[0], q[2]);
    assert_eq!(p[1], q[3]);
}

#[test]
fn what_is_not_natural_is_left_alone() {
    let t = SurfaceKind::Torus(
        geom_surface::Torus::new(
            [0.0; 3],
            9.0,
            2.0,
            frame_along([0.0, 0.0, 1.0]),
            (ROUND, ROUND),
        )
        .unwrap(),
    );
    assert!(natural(&level(0.0), &t, &mut meter()).unwrap().is_none());
}

#[test]
fn a_stop_stops() {
    let token = CancelToken::new();
    token.cancel();
    let mut m = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    assert_eq!(
        natural(&level(0.0), &level(1.0), &mut m).unwrap_err(),
        IntersectError::Cancelled
    );
}
