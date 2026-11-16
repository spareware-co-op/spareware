//! A surface of revolution's reduced cases read its profile as a meridian, a curve in a half-plane
//! of its axis, and two lines parallel to within rounding meet in the plane (issue 921).
//!
//! The half-plane test read only a profile's ends and middle, and only whether they lay in the
//! plane of the axis: a circle or a NURBS curve through the plane at those three points that left
//! it between was read as a meridian, and a profile across the axis lost the rings of its far part.
//! It is decided from the curve's own data now, exactly, and what it cannot prove is left to the
//! general paths (`None`). And a cone scaled by 3 met a coaxial revolution with an error: its
//! profile's direction, rounded from 30 to 30.000000000000004 by the half-plane map, was parallel
//! to the cone's other ruling to within rounding, and two lines whose exact determinant is not zero
//! divided by an enclosure of it that held zero.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{
    Axes, Basis, Circle, CurveKind, Ellipse, Frame, Hyperbola, Line, Parabola, ParamCurve, Trimmed,
};
use geom_intersect::curve::curves;
use geom_intersect::quadrics::natural;
use geom_intersect::result::{Intersection, Item};
use geom_num::interval::Interval;
use geom_nurbs::Curve;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Revolution, SurfaceKind};

const ROUND: (f64, f64) = (0.0, 2.0);
const WIDE: (f64, f64) = (-1000.0, 1000.0);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(50_000_000))
}

fn up() -> Frame<3> {
    Frame::standard().unwrap()
}

/// The frame whose plane is y = 0, the half-plane x > 0 of the z axis holding its x̂.
fn upright() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap()
}

fn segment(a: [f64; 3], b: [f64; 3]) -> CurveKind<3> {
    CurveKind::Line(Line::segment(a, b).unwrap().value)
}

/// The surface `profile` sweeps turning once about the z axis.
fn revolution(profile: CurveKind<3>) -> SurfaceKind {
    let span = profile.domain();
    let profile = Trimmed::new(Basis::try_from(profile).unwrap(), span).unwrap();
    SurfaceKind::Revolution(Revolution::new(profile, [0.0; 3], up(), ROUND).unwrap())
}

/// The plane z = `z`, square to the axis.
fn level(z: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, z],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            (WIDE, WIDE),
        )
        .unwrap(),
    )
}

/// The plane x = 0, through the axis.
fn through() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new([0.0; 3], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], (WIDE, WIDE)).unwrap(),
    )
}

fn cylinder(r: f64, heights: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], r, up(), (ROUND, heights)).unwrap())
}

/// The rings of `r`, as (radius, height), each a circle about the z axis; a point on the axis
/// is a ring of radius 0.
fn rings(r: &Intersection) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = r
        .items()
        .iter()
        .map(|item| match item {
            Item::Curve { curve, .. } => match curve.as_ref() {
                CurveKind::Circle(c) => {
                    let [x, y, z] = c.center();
                    assert!(x.abs() < 1e-9 && y.abs() < 1e-9, "{c:?} off the axis");
                    (c.radius(), z)
                }
                other => panic!("{other:?}"),
            },
            Item::Point { xyz, .. } => {
                let (lo, hi) = (xyz.min().z.get(), xyz.max().z.get());
                (0.0, f64::midpoint(lo as f64, hi as f64))
            }
            other => panic!("{other:?}"),
        })
        .collect();
    out.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)));
    out
}

/// That `r` is the rings `want`, each to within 1e-9.
fn assert_rings(r: &Intersection, want: &[(f64, f64)]) {
    let got = rings(r);
    assert_eq!(got.len(), want.len(), "{got:?}, want {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(
            (g.0 - w.0).abs() < 1e-9 && (g.1 - w.1).abs() < 1e-9,
            "{got:?}, want {want:?}"
        );
    }
}

#[test]
fn two_lines_parallel_to_within_rounding_meet_nowhere_on_their_spans() {
    // The cone's upper ruling and the scaled profile, as the half-plane map gave them: their exact
    // determinant is 0.5·60 − 30.000000000000004 ≠ 0, an enclosure of it from rounded products
    // held zero, and the division by it was an error. They meet ~10^17 along the first, far off
    // both spans.
    let a =
        CurveKind::Line(Line::new([0.0, 24.0], [0.5, 1.0], (0.0, 4_398_046_511_104.0)).unwrap());
    let b = CurveKind::Line(
        Line::new([6.0, -30.0], [30.000_000_000_000_004, 60.0], (0.0, 1.0)).unwrap(),
    );
    let r = curves(&a, &b, &mut meter()).unwrap();
    assert!(r.items().is_empty(), "{:?}", r.items());
}

#[test]
fn a_crossing_keeps_the_boxes_of_its_quotients() {
    // Away from parallel the parameters are the interval quotients they always were, so no answer
    // moved: s = (w × e) / (d × e) and t = (w × d) / (d × e), w = p − o.
    let (o, d, p, e) = ([0.0, 0.0], [3.0, 1.0], [1.0, 2.0], [1.0, -3.0]);
    let a = CurveKind::Line(Line::new(o, d, (0.0, 10.0)).unwrap());
    let b = CurveKind::Line(Line::new(p, e, (0.0, 10.0)).unwrap());
    let r = curves(&a, &b, &mut meter()).unwrap();
    let [Item::Point { at, .. }] = r.items() else {
        panic!("{:?}", r.items())
    };
    let i = |v: f64| Interval::point(v).unwrap();
    let (wx, wy) = (i(p[0]).sub(i(o[0])).unwrap(), i(p[1]).sub(i(o[1])).unwrap());
    let det = i(d[0])
        .mul(i(e[1]))
        .unwrap()
        .sub(i(d[1]).mul(i(e[0])).unwrap())
        .unwrap();
    let s = wx
        .mul(i(e[1]))
        .unwrap()
        .sub(wy.mul(i(e[0])).unwrap())
        .unwrap()
        .div(det)
        .unwrap();
    let t = wx
        .mul(i(d[1]))
        .unwrap()
        .sub(wy.mul(i(d[0])).unwrap())
        .unwrap()
        .div(det)
        .unwrap();
    assert_eq!(at.spans(), &[s, t]);
}

#[test]
fn a_cone_scaled_by_three_meets_a_coaxial_revolution_in_its_ring() {
    // ρ = 12 − z/2 (its other ruling ρ = (z − 24)/2 above the apex) with ρ = 21 + z/2: one ring,
    // at z = −9 of radius 16.5. The profile's direction (30, 60) came out of the half-plane map as
    // 30.000000000000004, parallel to the other ruling to within rounding.
    let cone =
        SurfaceKind::Cone(Cone::new([0.0; 3], 12.0, -0.5, up(), (ROUND, (-30.0, 30.0))).unwrap());
    let funnel = revolution(segment([6.0, 0.0, -30.0], [36.0, 0.0, 30.0]));
    let r = natural(&cone, &funnel, &mut meter()).unwrap().unwrap();
    assert_rings(&r, &[(16.5, -9.0)]);
}

#[test]
fn a_profile_across_the_axis_keeps_the_rings_of_its_far_part() {
    // ρ = −3 + 6z/5 for z in [0, 10] sweeps |ρ|: the cylinder ρ = 2 meets it at ρ = −2 (z = 5/6)
    // and ρ = 2 (z = 25/6). The profile was read as a meridian, and the ring at ρ = −2 skipped
    // with every crossing off the half-plane. Either both rings or none (the general paths).
    let across = revolution(segment([-3.0, 0.0, 0.0], [9.0, 0.0, 10.0]));
    if let Some(r) = natural(&cylinder(2.0, (-20.0, 20.0)), &across, &mut meter()).unwrap() {
        assert_rings(&r, &[(2.0, 5.0 / 6.0), (2.0, 25.0 / 6.0)]);
    }
    // Cut through the axis it is two copies of the profile, each across the axis onto the other.
    assert!(
        natural(&through(), &across, &mut meter())
            .unwrap()
            .is_none()
    );
}

/// A cubic Bézier profile with its ends and middle in the plane y = 0 that leaves it between:
/// its inner control points stand ±1 off it, which cancel at t = 1/2.
fn wavy() -> CurveKind<3> {
    let basis = geom_nurbs::Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let points = [
        [2.0, 0.0, 0.0],
        [3.0, 1.0, 1.0],
        [3.0, -1.0, 2.0],
        [2.0, 0.0, 3.0],
    ];
    CurveKind::Nurbs(Curve::new(basis, &points).unwrap())
}

#[test]
fn a_nurbs_profile_that_leaves_the_plane_between_its_ends_and_middle_is_no_meridian() {
    let wavy = revolution(wavy());
    assert!(natural(&level(1.5), &wavy, &mut meter()).unwrap().is_none());
    assert!(natural(&through(), &wavy, &mut meter()).unwrap().is_none());
}

#[test]
fn a_nurbs_profile_is_a_meridian_by_its_control_points() {
    // The same in the plane, (2, 0)…(3, 1)…(3, 2)…(2, 3) in (ρ, z): z = 3/2 cuts at its middle,
    // ρ = 2 + 3/4.
    let basis = geom_nurbs::Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let points = [
        [2.0, 0.0, 0.0],
        [3.0, 0.0, 1.0],
        [3.0, 0.0, 2.0],
        [2.0, 0.0, 3.0],
    ];
    let flat = revolution(CurveKind::Nurbs(
        Curve::new(basis.clone(), &points).unwrap(),
    ));
    let r = natural(&level(1.5), &flat, &mut meter()).unwrap().unwrap();
    assert_rings(&r, &[(2.75, 1.5)]);
    // From the axis outward, its first control point on it: still a meridian.
    let points = [
        [0.0, 0.0, 0.0],
        [3.0, 0.0, 1.0],
        [3.0, 0.0, 2.0],
        [2.0, 0.0, 3.0],
    ];
    let from_axis = revolution(CurveKind::Nurbs(
        Curve::new(basis.clone(), &points).unwrap(),
    ));
    assert!(
        natural(&level(1.5), &from_axis, &mut meter())
            .unwrap()
            .is_some()
    );
    // A control point across the axis: its hull is on both sides.
    let points = [
        [2.0, 0.0, 0.0],
        [-3.0, 0.0, 1.0],
        [3.0, 0.0, 2.0],
        [2.0, 0.0, 3.0],
    ];
    let across = revolution(CurveKind::Nurbs(Curve::new(basis, &points).unwrap()));
    assert!(
        natural(&level(1.5), &across, &mut meter())
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_circle_through_the_plane_at_its_ends_and_middle_is_no_meridian() {
    // About (5, 0, 0) of radius 1, tilted about x̂: its ends (6, 0, 0) and middle (4, 0, 0) are in
    // the plane y = 0, and the rest of it is not.
    let tilted = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 1.0]).unwrap();
    let c = Circle::new([5.0, 0.0, 0.0], 1.0, tilted, ROUND).unwrap();
    let r = revolution(CurveKind::Circle(c));
    assert!(natural(&level(0.0), &r, &mut meter()).unwrap().is_none());
    assert!(natural(&through(), &r, &mut meter()).unwrap().is_none());
}

#[test]
fn a_circle_in_a_plane_of_the_axis_is_a_meridian_while_it_stays_on_one_side() {
    // A tube about ρ = 5 of radius 1: z = 0 cuts its rings ρ = 4 and 6.
    let tube = Circle::new([5.0, 0.0, 0.0], 1.0, upright(), ROUND).unwrap();
    let r = natural(
        &level(0.0),
        &revolution(CurveKind::Circle(tube)),
        &mut meter(),
    )
    .unwrap()
    .unwrap();
    assert_rings(&r, &[(4.0, 0.0), (6.0, 0.0)]);
    // Touching the axis, about ρ = 1 of radius 1: the point on the axis and ρ = 2.
    let horn = Circle::new([1.0, 0.0, 0.0], 1.0, upright(), ROUND).unwrap();
    let r = natural(
        &level(0.0),
        &revolution(CurveKind::Circle(horn)),
        &mut meter(),
    )
    .unwrap()
    .unwrap();
    assert_rings(&r, &[(0.0, 0.0), (2.0, 0.0)]);
    // Across it, about ρ = 1 of radius 2: left to the general paths.
    let across = Circle::new([1.0, 0.0, 0.0], 2.0, upright(), ROUND).unwrap();
    let across = revolution(CurveKind::Circle(across));
    assert!(
        natural(&level(0.0), &across, &mut meter())
            .unwrap()
            .is_none()
    );
    // Its plane through the axis but its centre off that plane: y = 0 holds the axis, the circle
    // stands at y = 1.
    let aside = Circle::new([5.0, 1.0, 0.0], 1.0, upright(), ROUND).unwrap();
    let aside = revolution(CurveKind::Circle(aside));
    assert!(
        natural(&level(0.0), &aside, &mut meter())
            .unwrap()
            .is_none()
    );
}

#[test]
fn an_ellipse_or_a_parabola_is_a_meridian_by_its_plane_and_a_ball_about_its_centre() {
    // About ρ = 6, semi-axes 2 along ρ and 1 along z: z = 0 cuts ρ = 4 and 8, by vectors or a
    // frame alike.
    let by_vectors = Axes::Vectors {
        u: [2.0, 0.0, 0.0],
        v: [0.0, 0.0, 1.0],
    };
    let on_frame = Axes::Frame {
        frame: upright(),
        a: 2.0,
        b: 1.0,
    };
    for axes in [by_vectors, on_frame] {
        let e = Ellipse::new([6.0, 0.0, 0.0], axes, ROUND).unwrap();
        let r = natural(
            &level(0.0),
            &revolution(CurveKind::Ellipse(e)),
            &mut meter(),
        )
        .unwrap()
        .unwrap();
        assert_rings(&r, &[(4.0, 0.0), (8.0, 0.0)]);
        // Off the plane of the axis by 1.
        let e = Ellipse::new([6.0, 1.0, 0.0], axes, ROUND).unwrap();
        let r = revolution(CurveKind::Ellipse(e));
        assert!(natural(&level(0.0), &r, &mut meter()).unwrap().is_none());
    }
    // Tilted out of the plane of the axis, its ends and middle in it.
    let tilted = Axes::Vectors {
        u: [2.0, 0.0, 0.0],
        v: [0.0, 1.0, 1.0],
    };
    let e = Ellipse::new([6.0, 0.0, 0.0], tilted, ROUND).unwrap();
    let r = revolution(CurveKind::Ellipse(e));
    assert!(natural(&level(0.0), &r, &mut meter()).unwrap().is_none());
    // Its ball, of radius √5 about ρ = 2, reaches across the axis: left to the general paths.
    let e = Ellipse::new([2.0, 0.0, 0.0], by_vectors, ROUND).unwrap();
    let r = revolution(CurveKind::Ellipse(e));
    assert!(natural(&level(0.0), &r, &mut meter()).unwrap().is_none());
    // ρ = 3 + t², z = t for t in [−1, 1]: z = 1/2 cuts ρ = 3.25. Its ball, (1 + 1)(1 + 1) = 4
    // about ρ = 3, stays off the axis; about ρ = 2 it reaches across.
    let flat = Axes::Vectors {
        u: [1.0, 0.0, 0.0],
        v: [0.0, 0.0, 1.0],
    };
    let p = Parabola::new([3.0, 0.0, 0.0], flat, (-1.0, 1.0)).unwrap();
    let r = natural(
        &level(0.5),
        &revolution(CurveKind::Parabola(p)),
        &mut meter(),
    )
    .unwrap()
    .unwrap();
    assert_rings(&r, &[(3.25, 0.5)]);
    let p = Parabola::new([2.0, 0.0, 0.0], flat, (-1.0, 1.0)).unwrap();
    let r = revolution(CurveKind::Parabola(p));
    assert!(natural(&level(0.5), &r, &mut meter()).unwrap().is_none());
    // A hyperbola has no ball here: left to the general paths.
    let h = Hyperbola::new([3.0, 0.0, 0.0], flat, (-1.0, 1.0)).unwrap();
    let r = revolution(CurveKind::Hyperbola(h));
    assert!(natural(&level(0.5), &r, &mut meter()).unwrap().is_none());
}
