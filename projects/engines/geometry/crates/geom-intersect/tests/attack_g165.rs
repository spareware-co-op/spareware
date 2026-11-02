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
    clippy::cast_precision_loss,
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_curve::{CurveKind, Frame, ParamCurve};
use geom_intersect::IntersectError;
use geom_intersect::quadrics::natural;
use geom_intersect::result::{
    Coincidence, CurveContact, CurveEnd, Intersection, Item, PointContact,
};
use geom_nurbs::Limit;
use geom_run::{Budget, CancelToken, WorkMeter};
use geom_surface::{Cone, Cylinder, ParamBox, ParamSurface, Plane, Sphere, SurfaceKind};
use geom_test::{Seed, SplitMix64};

const ROUND: (f64, f64) = (0.0, 2.0);
const TALL: (f64, f64) = (-500.0, 500.0);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The meeting of `a` and `b`, checked on the way out: every curve lies on both surfaces by
/// their own equations, no two are one, and one that does not close ends on an edge of a domain.
fn meet(a: &SurfaceKind, b: &SurfaceKind) -> Intersection {
    let r = natural(a, b, &mut meter())
        .unwrap_or_else(|e| panic!("{e:?} for {a:?} and {b:?}"))
        .unwrap_or_else(|| panic!("no closed form for {a:?} and {b:?}"));
    holds(&r, a, b);
    r
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn scale3(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// How far `p` stands off `s`, by its implicit equation.
fn off(s: &SurfaceKind, p: [f64; 3]) -> f64 {
    let radial = |w: [f64; 3], n: [f64; 3]| {
        let n = scale3(n, 1.0 / length(n));
        (length(sub(w, scale3(n, dot3(w, n)))), dot3(w, n))
    };
    match s {
        SurfaceKind::Plane(plane) => {
            let (c, a, b) = plane.vectors();
            let n = cross3(a, b);
            (dot3(sub(p, c), n) / length(n)).abs()
        }
        SurfaceKind::Sphere(x) => (length(sub(p, x.center())) - x.radius()).abs(),
        SurfaceKind::Cylinder(x) => {
            let (rho, _) = radial(sub(p, x.center()), x.frame().normal());
            (rho - x.radius()).abs()
        }
        SurfaceKind::Cone(x) => {
            let (r0, k) = x.radius_and_slope();
            let (rho, z) = radial(sub(p, x.center()), x.frame().normal());
            (rho - (r0 + k * z * x.axial_sign()).abs()).abs()
        }
        _ => 0.0,
    }
}

/// How far `p` stands from the polyline through `line`.
fn from_polyline(p: [f64; 3], line: &[[f64; 3]]) -> f64 {
    line.windows(2)
        .map(|w| {
            let d = sub(w[1], w[0]);
            let t = (dot3(sub(p, w[0]), d) / dot3(d, d).max(1e-300)).clamp(0.0, 1.0);
            length(sub(
                p,
                [w[0][0] + t * d[0], w[0][1] + t * d[1], w[0][2] + t * d[2]],
            ))
        })
        .fold(f64::INFINITY, f64::min)
}

fn holds(r: &Intersection, a: &SurfaceKind, b: &SurfaceKind) {
    let all = samples(r, 200);
    for (i, c) in all.iter().enumerate() {
        for p in c {
            for s in [a, b] {
                let gap = off(s, *p);
                assert!(
                    gap < 1e-6 * (1.0 + length(*p)),
                    "curve {i} at {p:?} stands {gap} off {s:?}"
                );
            }
        }
        for (j, d) in all.iter().enumerate().skip(i + 1) {
            let one = [c[37], c[91], c[163]]
                .iter()
                .all(|p| from_polyline(*p, d) < 1e-6 * (1.0 + length(*p)));
            assert!(!one, "curves {i} and {j} are one: {:?}", r.items());
        }
    }
    for item in r.items() {
        if let Item::Curve { curve, ends, .. } = item {
            if ends[0] == CurveEnd::Closed {
                continue;
            }
            let (lo, hi) = curve.domain();
            for (t, side) in [(lo, Limit::FromRight), (hi, Limit::FromLeft)] {
                let p = curve.point(t, side).unwrap();
                assert!(
                    inside_by(a, p, CUT).is_none() || inside_by(b, p, CUT).is_none(),
                    "a piece ends at {p:?}, inside both domains: {:?}",
                    r.items()
                );
            }
        }
    }
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

const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

/// `s` over a whole turn and every height, so that its point at any parameters can be read.
fn unbounded(s: &SurfaceKind) -> SurfaceKind {
    let far = (-1e9, 1e9);
    match s {
        SurfaceKind::Cylinder(x) => SurfaceKind::Cylinder(
            Cylinder::new(x.center(), x.radius(), x.frame(), (ROUND, far)).unwrap(),
        ),
        SurfaceKind::Cone(x) => {
            let (r0, k) = x.radius_and_slope();
            SurfaceKind::Cone(Cone::new(x.center(), r0, k, x.frame(), (ROUND, far)).unwrap())
        }
        SurfaceKind::Sphere(x) => SurfaceKind::Sphere(
            Sphere::new(x.center(), x.radius(), x.frame(), (ROUND, (-0.5, 0.5))).unwrap(),
        ),
        other => other.clone(),
    }
}

fn on(s: &SurfaceKind, u: f64, v: f64) -> [f64; 3] {
    s.point(u, v, SIDES).unwrap()
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(a: [f64; 3]) -> f64 {
    dot3(a, a).sqrt()
}

/// The turn of (x, y) in [0, 2).
fn turn(x: f64, y: f64) -> f64 {
    let t = y.atan2(x) / core::f64::consts::PI;
    if t < 0.0 { t + 2.0 } else { t }
}

/// Where `p` stands in `s`'s parameters, read off the surface's own points: a plane by its vectors,
/// a sphere about its centre, a cylinder or cone about the axis through the middles of opposite
/// points. `None` where no turn can be read (on an axis, at an apex).
fn parameters_of(s: &SurfaceKind, p: [f64; 3]) -> Option<(f64, f64)> {
    let s = &unbounded(s);
    match s {
        SurfaceKind::Plane(plane) => {
            let (c, a, b) = plane.vectors();
            let w = sub(p, c);
            let (aa, ab, bb) = (dot3(a, a), dot3(a, b), dot3(b, b));
            let (wa, wb) = (dot3(w, a), dot3(w, b));
            let det = aa * bb - ab * ab;
            Some(((wa * bb - wb * ab) / det, (wb * aa - wa * ab) / det))
        }
        SurfaceKind::Sphere(sphere) => {
            let c = sphere.center();
            let (e0, e1, up) = (
                sub(on(s, 0.0, 0.0), c),
                sub(on(s, 0.5, 0.0), c),
                sub(on(s, 0.0, 0.5), c),
            );
            let w = sub(p, c);
            let (x, y, z) = (
                dot3(w, e0) / length(e0),
                dot3(w, e1) / length(e1),
                dot3(w, up) / length(up),
            );
            let round = x.hypot(y);
            (round > 1e-6).then(|| (turn(x, y), z.atan2(round) / core::f64::consts::PI))
        }
        _ => {
            let middle = |v: f64| {
                let (a, b) = (on(s, 0.0, v), on(s, 1.0, v));
                [
                    a[0] * 0.5 + b[0] * 0.5,
                    a[1] * 0.5 + b[1] * 0.5,
                    a[2] * 0.5 + b[2] * 0.5,
                ]
            };
            let (o, top) = (middle(0.0), middle(1.0));
            let axis = sub(top, o);
            let v = dot3(sub(p, o), axis) / dot3(axis, axis);
            let centre = middle(v);
            let (e0, e1) = (sub(on(s, 0.0, v), centre), sub(on(s, 0.5, v), centre));
            if length(e0) < 1e-6 {
                return None;
            }
            let w = sub(p, centre);
            Some((turn(dot3(w, e0), dot3(w, e1)), v))
        }
    }
}

/// How far one unit of each parameter moves a point of `s` at `(u, v)`, in space.
fn reach_of(s: &SurfaceKind, v: f64) -> (f64, f64) {
    let pi = core::f64::consts::PI;
    match s {
        SurfaceKind::Plane(plane) => {
            let (_, a, b) = plane.vectors();
            (length(a), length(b))
        }
        SurfaceKind::Cylinder(x) => (pi * x.radius(), 1.0),
        SurfaceKind::Cone(x) => {
            let (r0, k) = x.radius_and_slope();
            (pi * (r0 + k * v).abs(), (1.0 + k * k).sqrt())
        }
        SurfaceKind::Sphere(x) => (pi * x.radius() * (pi * v).cos(), pi * x.radius()),
        _ => (1.0, 1.0),
    }
}

/// Whether `p`, a point of `s`, lies on its domain: `None` within `slack` of a true edge, in
/// space, where a cut or a rounding may put it either side, and where no parameter can be read.
fn inside_by(s: &SurfaceKind, p: [f64; 3], slack: f64) -> Option<bool> {
    let (u, v) = parameters_of(s, p)?;
    let (per_u, per_v) = reach_of(s, v);
    let ((u0, u1), (v0, v1)) = ParamSurface::domain(s);
    let round = !matches!(s, SurfaceKind::Plane(_));
    let mut sides = Vec::new();
    if !round || u1 - u0 < 2.0 {
        // A turn is read in [0, 2): the copy of it nearest the domain's range.
        let gap = |x: f64| tight_gap(x, u0, u1);
        let u = if round {
            [u - 2.0, u, u + 2.0]
                .into_iter()
                .min_by(|a, b| gap(*a).total_cmp(&gap(*b)))
                .unwrap()
        } else {
            u
        };
        sides.extend([(u - u0) * per_u, (u1 - u) * per_u]);
    }
    let sphere = matches!(s, SurfaceKind::Sphere(_));
    for (side, end) in [(v - v0, v0), (v1 - v, v1)] {
        if !sphere || end.abs() < 0.5 {
            sides.push(side * per_v);
        }
    }
    let least = sides.iter().fold(f64::INFINITY, |a, b| a.min(*b));
    (least.abs() >= slack).then_some(least > 0.0)
}

/// `inside_by` with the slack of a rounding.
fn inside(s: &SurfaceKind, p: [f64; 3]) -> Option<bool> {
    inside_by(s, p, 1e-6 * (1.0 + length(p)))
}

/// How near an edge a cut may leave a piece's end, or skip a stretch: a stretch shorter than two
/// steps is below what a cut can tell (12.7's trimming).
const CUT: f64 = 2.5;

/// How far `x` lies outside [lo, hi].
fn tight_gap(x: f64, lo: f64, hi: f64) -> f64 {
    if x < lo {
        lo - x
    } else if x > hi {
        x - hi
    } else {
        0.0
    }
}

/// `n` points along each curve of `r`, end to end.
fn samples(r: &Intersection, n: usize) -> Vec<Vec<[f64; 3]>> {
    curves(r)
        .into_iter()
        .map(|c| {
            let (lo, hi) = c.domain();
            (0..=n)
                .map(|i| {
                    let (t, side) = if i == n {
                        (hi, Limit::FromLeft)
                    } else {
                        (lo + (hi - lo) * i as f64 / n as f64, Limit::FromRight)
                    };
                    c.point(t, side).unwrap()
                })
                .collect()
        })
        .collect()
}

/// The trimming oracle: `part`, the meeting of `a` and `b`, against `whole`, the meeting of the
/// same surfaces with domains that hold theirs. Away from its ends every piece of `part` lies on
/// both domains, and every run of `whole`'s curves on both domains longer than two steps (shorter
/// ones are below what a cut can tell, 12.7's trimming) lies on a piece of `part`.
fn trims(whole: &Intersection, part: &Intersection, a: &SurfaceKind, b: &SurfaceKind, what: &str) {
    let reference = samples(whole, 1500);
    assert!(!reference.is_empty(), "{what}: no curve to trim");
    let pieces = samples(part, 1500);
    for (i, piece) in pieces.iter().enumerate() {
        for q in &piece[30..piece.len() - 30] {
            for s in [a, b] {
                assert_ne!(
                    inside(s, *q),
                    Some(false),
                    "{what}: piece {i} leaves a domain at {q:?}"
                );
            }
        }
    }
    let spacing = pieces
        .iter()
        .flat_map(|p| p.windows(2).map(|w| length(sub(w[0], w[1]))))
        .fold(0.0, f64::max);
    for curve in &reference {
        let on_both = |p: &[f64; 3]| {
            inside_by(a, *p, CUT) == Some(true) && inside_by(b, *p, CUT) == Some(true)
        };
        let mut run: Vec<[f64; 3]> = Vec::new();
        let check = |run: &mut Vec<[f64; 3]>| {
            let long: f64 = run.windows(2).map(|w| length(sub(w[0], w[1]))).sum();
            if long > 4.0 {
                for p in run.iter() {
                    let near = pieces
                        .iter()
                        .flatten()
                        .map(|q| length(sub(*p, *q)))
                        .fold(f64::INFINITY, f64::min);
                    assert!(
                        near <= 2.0 * spacing + 1e-6,
                        "{what}: {p:?} lies on both domains, {near} from every piece"
                    );
                }
            }
            run.clear();
        };
        for p in curve {
            if on_both(p) {
                run.push(*p);
            } else {
                check(&mut run);
            }
        }
        check(&mut run);
    }
}

fn span(rng: &mut SplitMix64, lo: f64, hi: f64) -> f64 {
    lo + (hi - lo) * ((rng.next_u64() >> 11) as f64 / 2.0_f64.powi(53))
}

/// Part of a turn, within [0, 2], and a range of heights from `height`.
fn round_domain(rng: &mut SplitMix64, height: (f64, f64)) -> ParamBox {
    let turn = span(rng, 0.3, 1.8);
    let start = span(rng, 0.0, 2.0 - turn);
    let (lo, hi) = height;
    let (a, b) = (span(rng, lo, hi), span(rng, lo, hi));
    ((start, start + turn), (a.min(b) - 1.0, a.max(b) + 1.0))
}

#[test]
fn trimming_keeps_exactly_what_lies_on_both_domains() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("g165 trims", Seed::DEFAULT));
    let up = frame_along([0.0, 0.0, 1.0]);
    let big: ParamBox = ((-1e5, 1e5), (-1e5, 1e5));
    let random_plane = |rng: &mut SplitMix64, at: [f64; 3], domain: ParamBox| {
        loop {
            let a = [
                span(rng, -2.0, 2.0),
                span(rng, -2.0, 2.0),
                span(rng, -2.0, 2.0),
            ];
            let b = [
                span(rng, -2.0, 2.0),
                span(rng, -2.0, 2.0),
                span(rng, -2.0, 2.0),
            ];
            if let Ok(p) = Plane::new(at, a, b, domain) {
                let n = [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ];
                if length(n) > 0.5 && n[2].abs() > 0.3 * length(n) {
                    return SurfaceKind::Plane(p);
                }
            }
        }
    };
    for case in 0..24 {
        let what = format!("case {case}");
        let centre = [
            span(&mut rng, -50.0, 50.0),
            span(&mut rng, -50.0, 50.0),
            0.0,
        ];
        let r = span(&mut rng, 100.0, 250.0);
        let near = [centre[0], centre[1], span(&mut rng, -0.5 * r, 0.5 * r)];
        // A plane of a finite domain, or one running on past some of its sides, with a sphere.
        let wide = random_plane(&mut rng, near, big);
        let SurfaceKind::Plane(w) = &wide else {
            panic!()
        };
        let (c, a, b) = w.vectors();
        let reach_u = r / length(a);
        let reach_v = r / length(b);
        let mut side = |reach: f64, open: bool| {
            let (lo, hi) = (
                -reach * span(&mut rng, 0.2, 1.1),
                reach * span(&mut rng, 0.2, 1.1),
            );
            if open && case % 2 == 0 {
                (lo, f64::INFINITY)
            } else {
                (lo, hi)
            }
        };
        let cut = SurfaceKind::Plane(
            Plane::new(
                c,
                a,
                b,
                (side(reach_u, case % 4 == 0), side(reach_v, case % 3 == 0)),
            )
            .unwrap(),
        );
        let ball = SurfaceKind::Sphere(Sphere::new(centre, r, up, (ROUND, (-0.5, 0.5))).unwrap());
        trims(
            &meet(&wide, &ball),
            &meet(&cut, &ball),
            &cut,
            &ball,
            &format!("{what} plane"),
        );
        // A sphere cut to a band of turns and latitudes.
        let (turns, _) = round_domain(&mut rng, (0.0, 1.0));
        let low = span(&mut rng, -0.45, 0.25);
        let band = (low, low + span(&mut rng, 0.1, 0.2));
        let part = SurfaceKind::Sphere(Sphere::new(centre, r, up, (turns, band)).unwrap());
        trims(
            &meet(&wide, &ball),
            &meet(&wide, &part),
            &wide,
            &part,
            &format!("{what} sphere"),
        );
        // A cylinder and a cone cut to a range of turns and heights.
        let tube = |domain| SurfaceKind::Cylinder(Cylinder::new(centre, r, up, domain).unwrap());
        let tall = (ROUND, (-1e4, 1e4));
        let short = round_domain(&mut rng, (-r, r));
        trims(
            &meet(&wide, &tube(tall)),
            &meet(&wide, &tube(short)),
            &wide,
            &tube(short),
            &format!("{what} cylinder"),
        );
        let k = span(&mut rng, 0.3, 2.0);
        let funnel = |domain| SurfaceKind::Cone(Cone::new(centre, 0.0, k, up, domain).unwrap());
        let short = round_domain(&mut rng, (-r, r));
        trims(
            &meet(&wide, &funnel(tall)),
            &meet(&wide, &funnel(short)),
            &wide,
            &funnel(short),
            &format!("{what} cone"),
        );
    }
}

#[test]
fn a_half_plane_keeps_the_half_circle_on_it() {
    // z = 0 for x ≥ 0 only, through a sphere about the origin: the half of its equator with x ≥ 0.
    let half = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, f64::INFINITY), (f64::NEG_INFINITY, f64::INFINITY)),
        )
        .unwrap(),
    );
    let r = meet(&half, &sphere([0.0; 3], 50.0));
    let c = curves(&r);
    assert_eq!(c.len(), 1, "{:?}", r.items());
    let (lo, hi) = c[0].domain();
    near(hi - lo, 1.0);
    let p = mid(c[0]);
    near(p[0], 50.0);
    near(p[1], 0.0);
}

#[test]
fn lines_and_parabolas_are_trimmed_too() {
    let up = frame_along([0.0, 0.0, 1.0]);
    // Two parallel cylinders cross in two lines; one of them stands from −50 to 80 over half a turn.
    let a = |domain| SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 100.0, up, domain).unwrap());
    let b = cylinder([120.0, 30.0, 0.0], [0.0, 0.0, 1.0], 90.0);
    let short = ((0.0, 1.0), (-50.0, 80.0));
    trims(
        &meet(&a((ROUND, TALL)), &b),
        &meet(&a(short), &b),
        &a(short),
        &b,
        "lines",
    );
    // Two planes of finite domains cross in a line.
    let flat = |side: ParamBox| {
        SurfaceKind::Plane(Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], side).unwrap())
    };
    let leaning = |side: ParamBox| {
        SurfaceKind::Plane(
            Plane::new([3.0, 0.0, 0.0], [1.0, 2.0, 1.0], [0.0, 1.0, 2.0], side).unwrap(),
        )
    };
    let wide: ParamBox = ((-1e5, 1e5), (-1e5, 1e5));
    let (p, q) = (
        ((-40.0, 70.0), (-30.0, 90.0)),
        ((-60.0, 50.0), (-45.0, 25.0)),
    );
    trims(
        &meet(&flat(wide), &leaning(wide)),
        &meet(&flat(p), &leaning(q)),
        &flat(p),
        &leaning(q),
        "planes",
    );
    // x − z = 60 is parallel to the ruling (1, 0, 1) of the cone of slope 1 about z: a parabola.
    let funnel = |domain| SurfaceKind::Cone(Cone::new([0.0; 3], 0.0, 1.0, up, domain).unwrap());
    let sheet = plane([60.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 1.0, 0.0]);
    let short = ((0.25, 1.75), (-200.0, 300.0));
    let whole = meet(&sheet, &funnel((ROUND, (-1e4, 1e4))));
    assert!(
        curves(&whole)
            .iter()
            .all(|c| matches!(c, CurveKind::Parabola(_))),
        "{:?}",
        whole.items()
    );
    trims(
        &whole,
        &meet(&sheet, &funnel(short)),
        &sheet,
        &funnel(short),
        "parabola",
    );
}

#[test]
fn leaning_whole_data_decide_exactly() {
    // The plane through (1, 2, 3) on (3, 0, −1) and (0, 2, −1), normal (2, 3, 6) of length 7: no
    // vector along an axis, so each coordinate of its exact points counts.
    let lean = plane([1.0, 2.0, 3.0], [3.0, 0.0, -1.0], [0.0, 2.0, -1.0]);
    // A ball of radius 7 about (3, 5, 9), seven along the normal, touches it at (1, 2, 3).
    let r = meet(&lean, &sphere([3.0, 5.0, 9.0], 7.0));
    assert_eq!(points(&r), 1, "{:?}", r.items());
    assert!(
        matches!(
            r.items(),
            [Item::Point {
                contact: PointContact::IsolatedTangent,
                ..
            }]
        ),
        "{:?}",
        r.items()
    );
    // A cylinder of radius 7 about the line through (3, 5, 9) along (3, 2, −2), which lies in the
    // plane's directions: tangent along one line.
    let tube = cylinder([3.0, 5.0, 9.0], [3.0, 2.0, -2.0], 7.0);
    let r = meet(&lean, &tube);
    assert_eq!(curves(&r).len(), 1, "{:?}", r.items());
    assert!(tangent(&r), "{:?}", r.items());
    // A cone of slope 1 with its apex at (1, 2, 3) on the plane, about (5, −3, 8), which stands at
    // 45 degrees to the normal: the plane touches it along one ruling.
    let funnel = cone([1.0, 2.0, 3.0], [5.0, -3.0, 8.0], 1.0);
    let r = meet(&lean, &funnel);
    assert_eq!(curves(&r).len(), 1, "{:?}", r.items());
    assert!(tangent(&r), "{:?}", r.items());
    // A cone of radius 2 at its centre, slope 1: its apex is at v = −2, and a plane there square
    // to the axis meets it in that one point.
    let up = frame_along([0.0, 0.0, 1.0]);
    let blunt = SurfaceKind::Cone(Cone::new([0.0; 3], 2.0, 1.0, up, (ROUND, TALL)).unwrap());
    let r = meet(&level(-2.0), &blunt);
    assert_eq!(points(&r), 1, "{:?}", r.items());
    assert!(curves(&r).is_empty(), "{:?}", r.items());
}

#[test]
fn a_cone_ending_at_its_apex_is_cut_there() {
    // One nappe, 3 out for every 4 up, from its apex at the origin to 400 up: the apex is an edge
    // of one point, and a line through it leaves the domain there. Each piece runs from the apex
    // to the top circle, its middle 200 up, within what a cut can tell.
    let up = frame_along([0.0, 0.0, 1.0]);
    let nappe =
        SurfaceKind::Cone(Cone::new([0.0; 3], 0.0, 0.75, up, (ROUND, (0.0, 400.0))).unwrap());
    // A plane through the axis: two rays from the apex.
    let r = meet(&plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), &nappe);
    let c = curves(&r);
    assert_eq!(c.len(), 2, "{:?}", r.items());
    for ray in &c {
        let p = mid(ray);
        assert!((p[2] - 200.0).abs() < 0.01, "{p:?}");
        assert!((p[0].abs() - 150.0).abs() < 0.01, "{p:?}");
    }
    // A plane touching it along the ruling (3, 0, 4): one tangent ray.
    let r = meet(&plane([0.0; 3], [3.0, 0.0, 4.0], [0.0, 1.0, 0.0]), &nappe);
    assert_eq!(curves(&r).len(), 1, "{:?}", r.items());
    assert!(tangent(&r), "{:?}", r.items());
    let p = mid(curves(&r)[0]);
    assert!((p[2] - 200.0).abs() < 0.01, "{p:?}");
    assert!((p[0] - 150.0).abs() < 0.01, "{p:?}");
}

/// A sphere, cylinder or cone on part of its turn and height: `turn` in half-turns, `height` in v.
fn part(
    kind: &str,
    c: [f64; 3],
    n: [f64; 3],
    r: f64,
    turn: (f64, f64),
    height: (f64, f64),
) -> SurfaceKind {
    let f = frame_along(n);
    match kind {
        "sphere" => SurfaceKind::Sphere(Sphere::new(c, r, f, (turn, height)).unwrap()),
        "cylinder" => SurfaceKind::Cylinder(Cylinder::new(c, r, f, (turn, height)).unwrap()),
        _ => SurfaceKind::Cone(Cone::new(c, 0.0, r, f, (turn, height)).unwrap()),
    }
}

/// One pair of each closed form, on whole surfaces and on parts of them, by what it makes. Each
/// answer is checked by `holds` as it is read (every curve on both surfaces, none twice, an open
/// one ending on an edge of a domain), so the pins hold the trims, the ends and the classes: a
/// change to any of them is a change to what is made.
#[test]
fn every_closed_form_makes_what_it_made() {
    let z = [0.0, 0.0, 1.0];
    let x = [1.0, 0.0, 0.0];
    let y = [0.0, 1.0, 0.0];
    let tilted = |c: [f64; 3]| plane(c, [1.0, 0.0, 0.4], [0.0, 1.0, 0.0]);
    let pairs: Vec<(SurfaceKind, SurfaceKind)> = vec![
        // Planes.
        (level(1.0), plane([0.0, 0.0, 0.0], x, z)),
        (level(1.0), level(1.0)),
        // A plane and a sphere: through, tangent, and a part of the sphere.
        (sphere([0.0, 0.0, 0.0], 5.0), level(3.0)),
        (sphere([0.0, 0.0, 0.0], 5.0), level(5.0)),
        (
            part("sphere", [0.0, 0.0, 0.0], z, 5.0, (0.25, 1.25), (-0.5, 0.5)),
            level(3.0),
        ),
        (
            part("sphere", [0.0, 0.0, 0.0], z, 5.0, (1.5, 2.5), (0.0, 0.5)),
            tilted([0.0, 0.0, 1.0]),
        ),
        // A plane and a cylinder: square, slanted, along, tangent, and parts of it.
        (cylinder([0.0, 0.0, 0.0], z, 4.0), level(2.0)),
        (cylinder([0.0, 0.0, 0.0], z, 4.0), tilted([0.0, 0.0, 2.0])),
        (
            cylinder([0.0, 0.0, 0.0], z, 4.0),
            plane([1.0, 0.0, 0.0], y, z),
        ),
        (
            cylinder([0.0, 0.0, 0.0], z, 4.0),
            plane([4.0, 0.0, 0.0], y, z),
        ),
        (
            part("cylinder", [0.0, 0.0, 0.0], z, 4.0, (0.3, 1.7), (0.0, 6.0)),
            tilted([0.0, 0.0, 2.0]),
        ),
        (
            part("cylinder", [0.0, 0.0, 0.0], z, 4.0, (1.2, 2.6), (-3.0, 3.0)),
            level(1.0),
        ),
        (
            part("cylinder", [0.0, 0.0, 0.0], z, 4.0, (0.0, 1.0), (0.0, 6.0)),
            plane([1.0, 0.0, 0.0], y, z),
        ),
        // A plane and a cone: each conic, through the apex, and parts of it.
        (cone([0.0, 0.0, 0.0], z, 0.5), level(2.0)),
        (cone([0.0, 0.0, 0.0], z, 0.5), tilted([0.0, 0.0, 2.0])),
        (
            cone([0.0, 0.0, 0.0], z, 0.5),
            plane([0.0, 0.0, 2.0], [1.0, 0.0, 2.0], y),
        ),
        (cone([0.0, 0.0, 0.0], z, 0.5), plane([1.0, 0.0, 0.0], y, z)),
        (cone([0.0, 0.0, 0.0], z, 0.5), plane([0.0, 0.0, 0.0], x, z)),
        (
            part("cone", [0.0, 0.0, 0.0], z, 0.5, (0.25, 1.75), (0.5, 8.0)),
            tilted([0.0, 0.0, 2.0]),
        ),
        (
            part("cone", [0.0, 0.0, 0.0], z, 0.5, (1.0, 2.5), (-8.0, 8.0)),
            level(-2.0),
        ),
        // Two spheres: a circle, a touch, apart, and parts of them.
        (sphere([0.0, 0.0, 0.0], 5.0), sphere([6.0, 0.0, 0.0], 4.0)),
        (sphere([0.0, 0.0, 0.0], 5.0), sphere([9.0, 0.0, 0.0], 4.0)),
        (sphere([0.0, 0.0, 0.0], 5.0), sphere([20.0, 0.0, 0.0], 4.0)),
        (
            part("sphere", [0.0, 0.0, 0.0], z, 5.0, (0.5, 1.5), (-0.5, 0.5)),
            sphere([0.0, 0.0, 6.0], 4.0),
        ),
        // Coaxial pairs: a sphere on a cylinder and a cone, cylinders, cones, a cone on a cylinder.
        (
            sphere([0.0, 0.0, 0.0], 5.0),
            cylinder([0.0, 0.0, 0.0], z, 3.0),
        ),
        (sphere([0.0, 0.0, 1.0], 5.0), cone([0.0, 0.0, 0.0], z, 0.5)),
        (
            cylinder([0.0, 0.0, 0.0], z, 3.0),
            cylinder([0.0, 0.0, 5.0], z, 3.0),
        ),
        (
            cylinder([0.0, 0.0, 0.0], z, 3.0),
            cone([0.0, 0.0, 0.0], z, 0.5),
        ),
        (
            cone([0.0, 0.0, 0.0], z, 0.5),
            cone([0.0, 0.0, 3.0], z, 0.25),
        ),
        (cone([0.0, 0.0, 0.0], z, 0.5), cone([0.0, 0.0, 0.0], z, 0.5)),
        (
            part("cylinder", [0.0, 0.0, 0.0], z, 3.0, (0.5, 1.5), (-4.0, 4.0)),
            sphere([0.0, 0.0, 0.0], 5.0),
        ),
        (
            part("cone", [0.0, 0.0, 0.0], z, 0.5, (0.0, 1.0), (0.0, 8.0)),
            cone([0.0, 0.0, 3.0], z, 0.25),
        ),
        // Parallel cylinders and a tee.
        (
            cylinder([0.0, 0.0, 0.0], z, 3.0),
            cylinder([4.0, 0.0, 0.0], z, 2.0),
        ),
        (
            cylinder([0.0, 0.0, 0.0], z, 3.0),
            cylinder([0.0, 0.0, 0.0], x, 2.0),
        ),
        (
            part("cylinder", [0.0, 0.0, 0.0], z, 3.0, (0.0, 1.0), (-4.0, 4.0)),
            cylinder([4.0, 0.0, 0.0], z, 2.0),
        ),
    ];
    let mut out = Vec::new();
    for (a, b) in &pairs {
        out.push(match natural(a, b, &mut meter()) {
            Ok(Some(r)) => {
                holds(&r, a, b);
                format!("{:?} {:?}", r.items(), r.coverage())
            }
            other => format!("{other:?}"),
        });
    }
    for (k, line) in out.iter().enumerate() {
        println!("{k}: {line}");
    }
    assert_eq!(
        out.len(),
        CLOSED_FORMS.len(),
        "the pins are the lines printed above"
    );
    for (k, (got, want)) in out.iter().zip(CLOSED_FORMS).enumerate() {
        assert_eq!(got, want, "pair {k}");
    }
}

/// What [`every_closed_form_makes_what_it_made`] read.
const CLOSED_FORMS: &[&str] = &[
    "[Curve { at: ParamBox { spans: [Interval { lo: -1000.0, hi: 1000.0 }, Interval { lo: -2.4753e-320, hi: 2.4753e-320 }, Interval { lo: -1000.0, hi: 1000.0 }, Interval { lo: 0.9999999999999976, hi: 1.000000000000003 }], unknowns: 4 }, curve: Line(Line { origin: [0.0, 0.0, 1.0000000000000002], direction: [1.0, 0.0, 0.0], domain: (-999.9999999999998, 999.9999999999998), form: Form { shape: Linear, doubles: [[0.0, 0.0, 1.0000000000000002], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }], [Interval { lo: 1.0, hi: 1.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Overlap { at: ParamBox { spans: [Interval { lo: -1000.0, hi: 1000.0 }, Interval { lo: -1000.0, hi: 1000.0 }, Interval { lo: -1000.0, hi: 1000.0 }, Interval { lo: -1000.0, hi: 1000.0 }], unknowns: 4 }, region: OverlapRegion { boundary: [] }, proof: Identical, class: Exact }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.15521307072001272, hi: 0.5 }, Interval { lo: -4.000000000000012, hi: 4.000000000000012 }, Interval { lo: -4.000000000000014, hi: 4.000000000000014 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 3.0], radius: 4.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 3.0], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0, hi: 3.0 }], [Interval { lo: 4.0, hi: 4.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.0, hi: 4.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Point { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.4999999999999998, hi: 0.5 }, Interval { lo: -1.6e-322, hi: 1.6e-322 }, Interval { lo: -1.6e-322, hi: 1.6e-322 }], unknowns: 4 }, xyz: Box3 { min: P3 { x: Coord(-1), y: Coord(-1), z: Coord(4) }, max: P3 { x: Coord(1), y: Coord(1), z: Coord(6) } }, contact: IsolatedTangent, class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.25, hi: 1.25 }, Interval { lo: 0.15521307072001272, hi: 0.5 }, Interval { lo: -4.000000000000012, hi: 2.8284271247461996 }, Interval { lo: -2.828427124746199, hi: 4.000000000000014 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 3.0], radius: 4.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.25, 1.25), form: Form { shape: Trig, doubles: [[0.0, 0.0, 3.0], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0, hi: 3.0 }], [Interval { lo: 4.0, hi: 4.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.0, hi: 4.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 1.5000001618753527, hi: 2.4999998381246478 }, Interval { lo: 0.04885726094367689, hi: 0.4999997048361899 }, Interval { lo: 2.4913583744766974e-6, hi: 4.216812605283799 }, Interval { lo: -4.898979282147085, hi: 4.898979282147085 }], unknowns: 4 }, curve: Circle(Circle { center: [-0.3448275862068966, 0.0, 0.8620689655172415], radius: 4.913036844405175, frame: Frame { x: [0.0, 1.0, 0.0], normal: [-0.3713906763541037, 0.0, 0.9284766908852593] }, domain: (1.02408511530837, 1.97591488469163), form: Form { shape: Trig, doubles: [[-0.3448275862068966, 0.0, 0.8620689655172415], [0.0, 4.913036844405175, 0.0], [-4.561640191490674, 0.0, -1.8246560765962694], [-0.0, 0.0, 0.0]], exact: [[Interval { lo: -0.3448275862068966, hi: -0.3448275862068966 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.8620689655172415, hi: 0.8620689655172415 }], [Interval { lo: -3e-323, hi: 3e-323 }, Interval { lo: 4.913036844405169, hi: 4.913036844405181 }, Interval { lo: -3e-323, hi: 3e-323 }], [Interval { lo: -4.561640191490677, hi: -4.56164019149067 }, Interval { lo: -3e-323, hi: 3e-323 }, Interval { lo: -1.8246560765962707, hi: -1.824656076596268 }], [Interval { lo: -5e-324, hi: 5e-324 }, Interval { lo: -5e-324, hi: 5e-324 }, Interval { lo: -5e-324, hi: 5e-324 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.9999999999999993, hi: 2.000000000000002 }, Interval { lo: -4.000000000000012, hi: 4.000000000000012 }, Interval { lo: -4.000000000000014, hi: 4.000000000000014 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 2.0000000000000004], radius: 4.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 2.0000000000000004], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0000000000000004, hi: 2.0000000000000004 }], [Interval { lo: 4.0, hi: 4.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.0, hi: 4.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.40000000000000024, hi: 3.600000000000003 }, Interval { lo: -4.000000000000015, hi: 4.000000000000015 }, Interval { lo: -4.000000000000015, hi: 4.000000000000015 }], unknowns: 4 }, curve: Ellipse(Ellipse { center: [0.0, 0.0, 2.0000000000000004], axes: Vectors { u: [-4.0, 0.0, -1.5999999999999996], v: [0.0, 4.0, 0.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 2.0000000000000004], [-4.0, 0.0, -1.5999999999999996], [0.0, 4.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0000000000000004, hi: 2.0000000000000004 }], [Interval { lo: -4.0, hi: -4.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -1.5999999999999996, hi: -1.5999999999999996 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.0, hi: 4.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 1.5804306232551657, hi: 1.5804306232551668 }, Interval { lo: -500.0, hi: 500.0 }, Interval { lo: -3.872983346207425, hi: -3.872983346207407 }, Interval { lo: -500.000000000001, hi: 500.000000000001 }], unknowns: 4 }, curve: Line(Line { origin: [1.0000000000000002, -3.872983346207417, 0.0], direction: [0.0, 0.0, 1.0], domain: (-499.99999999999994, 499.99999999999994), form: Form { shape: Linear, doubles: [[1.0000000000000002, -3.872983346207417, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }, Interval { lo: -3.872983346207417, hi: -3.872983346207417 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.41956937674483347, hi: 0.41956937674483397 }, Interval { lo: -500.0, hi: 500.0 }, Interval { lo: 3.872983346207407, hi: 3.872983346207425 }, Interval { lo: -500.000000000001, hi: 500.000000000001 }], unknowns: 4 }, curve: Line(Line { origin: [1.0000000000000002, 3.872983346207417, 0.0], direction: [0.0, 0.0, 1.0], domain: (-499.99999999999994, 499.99999999999994), form: Form { shape: Linear, doubles: [[1.0000000000000002, 3.872983346207417, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }, Interval { lo: 3.872983346207417, hi: 3.872983346207417 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1e-323 }, Interval { lo: -500.0, hi: 500.0 }, Interval { lo: -1.24e-320, hi: 1.24e-320 }, Interval { lo: -500.000000000001, hi: 500.000000000001 }], unknowns: 4 }, curve: Line(Line { origin: [4.000000000000001, 0.0, 0.0], direction: [0.0, 0.0, 1.0], domain: (-499.99999999999994, 499.99999999999994), form: Form { shape: Linear, doubles: [[4.000000000000001, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 4.000000000000001, hi: 4.000000000000001 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Tangent { band: Steps(0), gap: StepsInterval { lo: Steps(0), hi: Steps(0) } }, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.3, hi: 1.7 }, Interval { lo: 0.40000000000000024, hi: 2.94045640366796 }, Interval { lo: -4.000000000000015, hi: 2.3511410091699014 }, Interval { lo: -4.000000000000015, hi: 4.000000000000015 }], unknowns: 4 }, curve: Ellipse(Ellipse { center: [0.0, 0.0, 2.0000000000000004], axes: Vectors { u: [-4.0, 0.0, -1.5999999999999996], v: [0.0, 4.0, 0.0] }, domain: (1.3, 2.6999999999999997), form: Form { shape: Trig, doubles: [[0.0, 0.0, 2.0000000000000004], [-4.0, 0.0, -1.5999999999999996], [0.0, 4.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0000000000000004, hi: 2.0000000000000004 }], [Interval { lo: -4.0, hi: -4.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -1.5999999999999996, hi: -1.5999999999999996 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.0, hi: 4.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 1.2, hi: 2.6 }, Interval { lo: 0.9999999999999997, hi: 1.000000000000001 }, Interval { lo: -3.2360679774997996, hi: 4.000000000000012 }, Interval { lo: -4.000000000000014, hi: 4.000000000000014 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 1.0000000000000002], radius: 4.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (1.2, 2.6), form: Form { shape: Trig, doubles: [[0.0, 0.0, 1.0000000000000002], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }], [Interval { lo: 4.0, hi: 4.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.0, hi: 4.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.41956937674483347, hi: 0.41956937674483397 }, Interval { lo: 0.0, hi: 6.0 }, Interval { lo: 3.872983346207407, hi: 3.872983346207425 }, Interval { lo: -1.43e-322, hi: 6.000000000000014 }], unknowns: 4 }, curve: Line(Line { origin: [1.0000000000000002, 3.872983346207417, 0.0], direction: [0.0, 0.0, 1.0], domain: (0.0, 6.0), form: Form { shape: Linear, doubles: [[1.0000000000000002, 3.872983346207417, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }, Interval { lo: 3.872983346207417, hi: 3.872983346207417 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.9999999999999993, hi: 2.000000000000002 }, Interval { lo: -1.0000000000000033, hi: 1.0000000000000033 }, Interval { lo: -1.0000000000000038, hi: 1.0000000000000038 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 2.0000000000000004], radius: 1.0000000000000002, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 2.0000000000000004], [1.0000000000000002, 0.0, 0.0], [0.0, 1.0000000000000002, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.0000000000000004, hi: 2.0000000000000004 }], [Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 1.666666666666666, hi: 2.500000000000001 }, Interval { lo: -0.8333333333333361, hi: 1.2500000000000024 }, Interval { lo: -1.0206207261596612, hi: 1.0206207261596612 }], unknowns: 4 }, curve: Ellipse(Ellipse { center: [0.20833333333333243, 0.0, 2.083333333333333], axes: Vectors { u: [1.041666666666666, 0.0, 0.4166666666666661], v: [0.0, 1.0206207261596572, 0.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.20833333333333243, 0.0, 2.083333333333333], [1.041666666666666, 0.0, 0.4166666666666661], [0.0, 1.0206207261596572, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.20833333333333243, hi: 0.20833333333333243 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.083333333333333, hi: 2.083333333333333 }], [Interval { lo: 1.041666666666666, hi: 1.041666666666666 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.4166666666666661, hi: 0.4166666666666661 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0206207261596572, hi: 1.0206207261596572 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.9999999999999996, hi: 500.0 }, Interval { lo: -0.5000000000000016, hi: 249.00000000000352 }, Interval { lo: -22.338307903688868, hi: 22.338307903688733 }], unknowns: 4 }, curve: Parabola(Parabola { vertex: [-0.5, 0.0, 1.0], axes: Vectors { u: [0.4999999999999999, 0.0, 1.0], v: [0.0, 0.9999999999999999, 0.0] }, domain: (-22.3383079036888, 22.338307903688666), form: Form { shape: Parabolic, doubles: [[-0.5, 0.0, 1.0], [0.4999999999999999, 0.0, 1.0], [0.0, 0.9999999999999999, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: -0.5, hi: -0.5 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.4999999999999999, hi: 0.4999999999999999 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.9999999999999999, hi: 0.9999999999999999 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.5012732429400648, hi: 1.4987267570598248 }, Interval { lo: -500.0, hi: -1.9999999999999987 }, Interval { lo: -249.99799999200067, hi: 249.99799997030271 }, Interval { lo: -500.0000000000012, hi: -1.9999999999999944 }], unknowns: 4 }, curve: Hyperbola(Hyperbola { center: [1.0000000000000002, 0.0, 0.0], axes: Vectors { u: [0.0, 0.0, -1.9999999999999996], v: [0.0, -1.0000000000000004, 0.0] }, domain: (-6.2146040983113995, 6.214604098398191), form: Form { shape: Hyperbolic, doubles: [[1.0000000000000002, 0.0, 0.0], [0.0, 0.0, -1.9999999999999996], [0.0, -1.0000000000000004, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -1.9999999999999996, hi: -1.9999999999999996 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -1.0000000000000004, hi: -1.0000000000000004 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 0.49872675705983366 }, Interval { lo: 1.9999999999999987, hi: 500.0 }, Interval { lo: -249.99799999200067, hi: 249.99799997207904 }, Interval { lo: 1.9999999999999944, hi: 500.0000000000012 }], unknowns: 4 }, curve: Hyperbola(Hyperbola { center: [1.0000000000000002, 0.0, 0.0], axes: Vectors { u: [0.0, 0.0, 1.9999999999999996], v: [0.0, -1.0000000000000004, 0.0] }, domain: (-6.214604098318505, 6.214604098398191), form: Form { shape: Hyperbolic, doubles: [[1.0000000000000002, 0.0, 0.0], [0.0, 0.0, 1.9999999999999996], [0.0, -1.0000000000000004, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.9999999999999996, hi: 1.9999999999999996 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -1.0000000000000004, hi: -1.0000000000000004 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -500.0, hi: 500.0 }, Interval { lo: -250.0000000000008, hi: 250.0000000000008 }, Interval { lo: -500.00000000000097, hi: 500.00000000000097 }], unknowns: 4 }, curve: Line(Line { origin: [0.0, 0.0, 0.0], direction: [-0.5000000000000004, 0.0, 1.0], domain: (-499.9999999999999, 499.9999999999999), form: Form { shape: Linear, doubles: [[0.0, 0.0, 0.0], [-0.5000000000000004, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: -0.5000000000000004, hi: -0.5000000000000004 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(1), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -500.0, hi: 500.0 }, Interval { lo: -250.0000000000008, hi: 250.0000000000008 }, Interval { lo: -500.00000000000097, hi: 500.00000000000097 }], unknowns: 4 }, curve: Line(Line { origin: [0.0, 0.0, 0.0], direction: [0.5000000000000004, 0.0, 1.0], domain: (-499.9999999999999, 499.9999999999999), form: Form { shape: Linear, doubles: [[0.0, 0.0, 0.0], [0.5000000000000004, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.5000000000000004, hi: 0.5000000000000004 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(1), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.25, hi: 1.75 }, Interval { lo: 1.666666666666666, hi: 2.3294313392598176 }, Interval { lo: -0.8333333333333361, hi: 0.8235783481495406 }, Interval { lo: -1.0206207261596612, hi: 1.0206207261596612 }], unknowns: 4 }, curve: Ellipse(Ellipse { center: [0.20833333333333243, 0.0, 2.083333333333333], axes: Vectors { u: [1.041666666666666, 0.0, 0.4166666666666661], v: [0.0, 1.0206207261596572, 0.0] }, domain: (0.29887723391413984, 1.70112276608586), form: Form { shape: Trig, doubles: [[0.20833333333333243, 0.0, 2.083333333333333], [1.041666666666666, 0.0, 0.4166666666666661], [0.0, 1.0206207261596572, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.20833333333333243, hi: 0.20833333333333243 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.083333333333333, hi: 2.083333333333333 }], [Interval { lo: 1.041666666666666, hi: 1.041666666666666 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.4166666666666661, hi: 0.4166666666666661 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0206207261596572, hi: 1.0206207261596572 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 1.0, hi: 2.5 }, Interval { lo: -2.000000000000002, hi: -1.9999999999999993 }, Interval { lo: -1.0000000000000033, hi: 1.0000000000000033 }, Interval { lo: -1.0000000000000038, hi: 1.0000000000000038 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -2.0000000000000004], radius: 1.0000000000000002, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 1.5), form: Form { shape: Trig, doubles: [[0.0, 0.0, -2.0000000000000004], [1.0000000000000002, 0.0, 0.0], [0.0, 1.0000000000000002, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -2.0000000000000004, hi: -2.0000000000000004 }], [Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 0.23005345616261608 }, Interval { lo: -0.2300534561626162, hi: 0.2300534561626162 }, Interval { lo: 0.6901603684878472, hi: 1.3098396315121528 }, Interval { lo: -0.30983963151215277, hi: 0.30983963151215277 }], unknowns: 4 }, curve: Circle(Circle { center: [3.7500000000000004, 0.0, 0.0], radius: 3.3071891388307377, frame: Frame { x: [0.0, 1.0, 0.0], normal: [1.0, 0.0, 0.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[3.7500000000000004, 0.0, 0.0], [0.0, 3.3071891388307377, 0.0], [0.0, 0.0, 3.3071891388307377], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 3.7500000000000004, hi: 3.7500000000000004 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.3071891388307377, hi: 3.3071891388307377 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.3071891388307377, hi: 3.3071891388307377 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Point { at: ParamBox { spans: [Interval { lo: 0.0, hi: 1e-323 }, Interval { lo: -1e-323, hi: 1e-323 }, Interval { lo: 0.9999999999999999, hi: 1.0000000000000002 }, Interval { lo: -1.5e-323, hi: 1.5e-323 }], unknowns: 4 }, xyz: Box3 { min: P3 { x: Coord(4), y: Coord(-1), z: Coord(-1) }, max: P3 { x: Coord(6), y: Coord(1), z: Coord(1) } }, contact: IsolatedTangent, class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.5, hi: 1.5 }, Interval { lo: 0.21512261722614978, hi: 0.5 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -0.5, hi: -0.14272679134726263 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 3.7500000000000004], radius: 3.307189138830738, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.5, 1.5), form: Form { shape: Trig, doubles: [[0.0, 0.0, 3.7500000000000004], [3.307189138830738, 0.0, 0.0], [0.0, 3.307189138830738, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.7500000000000004, hi: 3.7500000000000004 }], [Interval { lo: 3.307189138830738, hi: 3.307189138830738 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.307189138830738, hi: 3.307189138830738 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -0.5, hi: -0.24063253699046108 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -4.000000000000004, hi: -3.9999999999999987 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -4.000000000000001], radius: 3.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, -4.000000000000001], [3.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -4.000000000000001, hi: -4.000000000000001 }], [Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.24063253699046108, hi: 0.5 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 3.9999999999999987, hi: 4.000000000000004 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 4.000000000000001], radius: 3.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 4.000000000000001], [3.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.000000000000001, hi: 4.000000000000001 }], [Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -0.5, hi: -0.3386772836875673 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -3.654211490264019, hi: -3.6542114902640153 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -3.654211490264017], radius: 1.8271057451320085, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, -3.654211490264017], [1.8271057451320085, 0.0, 0.0], [0.0, 1.8271057451320085, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -3.654211490264017, hi: -3.654211490264017 }], [Interval { lo: 1.8271057451320085, hi: 1.8271057451320085 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.8271057451320085, hi: 1.8271057451320085 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.27149240488700555, hi: 0.5 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 5.254211490264014, hi: 5.254211490264021 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 5.254211490264018], radius: 2.627105745132009, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 5.254211490264018], [2.627105745132009, 0.0, 0.0], [0.0, 2.627105745132009, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 5.254211490264018, hi: 5.254211490264018 }], [Interval { lo: 2.627105745132009, hi: 2.627105745132009 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 2.627105745132009, hi: 2.627105745132009 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Overlap { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -500.0, hi: 500.0 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -500.0, hi: 500.0 }], unknowns: 4 }, region: OverlapRegion { boundary: [] }, proof: Identical, class: Exact }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -6.0000000000000036, hi: -5.9999999999999964 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -6.0000000000000036, hi: -5.9999999999999964 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -6.0], radius: 3.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, -6.0], [3.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -6.0, hi: -6.0 }], [Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 5.9999999999999964, hi: 6.0000000000000036 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 5.9999999999999964, hi: 6.0000000000000036 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 6.0], radius: 3.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 6.0], [3.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 6.0, hi: 6.0 }], [Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -3.000000000000002, hi: -2.9999999999999987 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -6.0000000000000036, hi: -5.9999999999999964 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -3.0000000000000004], radius: 1.5000000000000002, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, -3.0000000000000004], [1.5000000000000002, 0.0, 0.0], [0.0, 1.5000000000000002, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -3.0000000000000004, hi: -3.0000000000000004 }], [Interval { lo: 1.5000000000000002, hi: 1.5000000000000002 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.5000000000000002, hi: 1.5000000000000002 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.9999999999999997, hi: 1.000000000000001 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -2.0000000000000013, hi: -1.999999999999999 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 1.0000000000000002], radius: 0.5000000000000001, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.0, 2.0), form: Form { shape: Trig, doubles: [[0.0, 0.0, 1.0000000000000002], [0.5000000000000001, 0.0, 0.0], [0.0, 0.5000000000000001, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0000000000000002, hi: 1.0000000000000002 }], [Interval { lo: 0.5000000000000001, hi: 0.5000000000000001 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.5000000000000001, hi: 0.5000000000000001 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Closed, Closed], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[Overlap { at: ParamBox { spans: [Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -500.0, hi: 500.0 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -500.0, hi: 500.0 }], unknowns: 4 }, region: OverlapRegion { boundary: [] }, proof: Identical, class: Exact }] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.5, hi: 1.5 }, Interval { lo: -4.0, hi: -3.9999999999999987 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: -0.5, hi: -0.24063253699046108 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, -4.000000000000001], radius: 3.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.5, 1.5), form: Form { shape: Trig, doubles: [[0.0, 0.0, -4.000000000000001], [3.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: -4.000000000000001, hi: -4.000000000000001 }], [Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 0.5, hi: 1.5 }, Interval { lo: 3.9999999999999987, hi: 4.0 }, Interval { lo: 0.0, hi: 2.0 }, Interval { lo: 0.24063253699046108, hi: 0.5 }], unknowns: 4 }, curve: Circle(Circle { center: [0.0, 0.0, 4.000000000000001], radius: 3.0, frame: Frame { x: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] }, domain: (0.5, 1.5), form: Form { shape: Trig, doubles: [[0.0, 0.0, 4.000000000000001], [3.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 4.000000000000001, hi: 4.000000000000001 }], [Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 3.0, hi: 3.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "[] TopologyCertified",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.16086124651033215, hi: 0.16086124651033265 }, Interval { lo: -500.0, hi: 500.0 }, Interval { lo: 0.7412918697654984, hi: 0.7412918697654992 }, Interval { lo: -500.0, hi: 500.0 }], unknowns: 4 }, curve: Line(Line { origin: [2.625, 1.4523687548277806, 0.0], direction: [0.0, 0.0, 1.0], domain: (-499.99999999999994, 499.99999999999994), form: Form { shape: Linear, doubles: [[2.625, 1.4523687548277806, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 2.625, hi: 2.625 }, Interval { lo: 1.4523687548277806, hi: 1.4523687548277806 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }, Curve { at: ParamBox { spans: [Interval { lo: 1.839138753489667, hi: 1.8391387534896682 }, Interval { lo: -500.0, hi: 500.0 }, Interval { lo: 1.2587081302345007, hi: 1.2587081302345016 }, Interval { lo: -500.0, hi: 500.0 }], unknowns: 4 }, curve: Line(Line { origin: [2.625, -1.4523687548277806, 0.0], direction: [0.0, 0.0, 1.0], domain: (-499.99999999999994, 499.99999999999994), form: Form { shape: Linear, doubles: [[2.625, -1.4523687548277806, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 2.625, hi: 2.625 }, Interval { lo: -1.4523687548277806, hi: -1.4523687548277806 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
    "Ok(None)",
    "[Curve { at: ParamBox { spans: [Interval { lo: 0.16086124651033215, hi: 0.16086124651033265 }, Interval { lo: -4.0, hi: 4.0 }, Interval { lo: 0.7412918697654984, hi: 0.7412918697654992 }, Interval { lo: -4.0000000000000036, hi: 4.0000000000000036 }], unknowns: 4 }, curve: Line(Line { origin: [2.625, 1.4523687548277806, 0.0], direction: [0.0, 0.0, 1.0], domain: (-4.0, 4.0), form: Form { shape: Linear, doubles: [[2.625, 1.4523687548277806, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]], exact: [[Interval { lo: 2.625, hi: 2.625 }, Interval { lo: 1.4523687548277806, hi: 1.4523687548277806 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 1.0, hi: 1.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }], [Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }, Interval { lo: 0.0, hi: 0.0 }]] } }), contact: Crossing, ends: [Boundary, Boundary], class: Certified { bound: Steps(2), step: Q0 } }] TopologyCertified",
];
