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
//!
//! The map into the half-plane was still read at the profile's middle as rounded, which for a
//! circle is not the point the proof reads its side at, its centre: a circle touching the axis at
//! its middle, as a horn torus's does, whose middle rounded across the axis was read mirrored and
//! lost its rings, and one whose middle rounded off the plane of the axis was read turned out of
//! it. A circle's map is read at its centre now.

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
use geom_math::{RationalTransform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Curve, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, ParamSurface, Plane, Revolution, Sphere, SurfaceKind};

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
    // On part of its span, t in [0, 1/2], z = 3t: a part of a meridian is one. z = 1 cuts it at
    // t = 1/3, ρ = 16/27 + 4/3 + 2/3 + 2/27 = 8/3; z = 2 is past its part.
    let curve = CurveKind::Nurbs(Curve::new(basis.clone(), &points).unwrap());
    let part = Trimmed::new(Basis::try_from(curve).unwrap(), (0.0, 0.5)).unwrap();
    let part = SurfaceKind::Revolution(Revolution::new(part, [0.0; 3], up(), ROUND).unwrap());
    let r = natural(&level(1.0), &part, &mut meter()).unwrap().unwrap();
    assert_rings(&r, &[(8.0 / 3.0, 1.0)]);
    let r = natural(&level(2.0), &part, &mut meter()).unwrap().unwrap();
    assert_rings(&r, &[]);
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

/// A cubic Bézier curve on the control points `points`.
fn bezier(points: &[[f64; 3]; 4]) -> CurveKind<3> {
    let basis = geom_nurbs::Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    CurveKind::Nurbs(Curve::new(basis, points).unwrap())
}

#[test]
fn a_nurbs_profile_takes_its_plane_from_its_first_control_point_off_the_axis() {
    // The first control point is on the z axis, where w × ẑ = 0 names no plane of the axis: the
    // plane is the next point's, y = 0, and the third stands 1 off it, so this is no meridian.
    // Read in the first point's zero "plane", every point would be in it.
    let twisted = revolution(bezier(&[
        [0.0, 0.0, 0.0],
        [3.0, 0.0, 1.0],
        [3.0, 1.0, 2.0],
        [2.0, 0.0, 3.0],
    ]));
    assert!(
        natural(&level(1.5), &twisted, &mut meter())
            .unwrap()
            .is_none()
    );
    assert!(
        natural(&through(), &twisted, &mut meter())
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_nurbs_profile_about_a_slanted_axis_is_read_in_the_plane_its_control_points_name() {
    // About the axis along e = (1, 1, 1), stored as it is given, every w × e has three nonzero
    // entries off the axis. The plane of e and u = (1, −1, 0) holds a·e + b·u = (a + b, a − b, a);
    // w × e for each point of it is a multiple of (1, 1, −2), the plane's normal.
    let slant = Frame::new([1.0, -1.0, 0.0], [1.0, 1.0, 1.0]).unwrap();
    let about = |profile: CurveKind<3>| {
        let span = profile.domain();
        let profile = Trimmed::new(Basis::try_from(profile).unwrap(), span).unwrap();
        SurfaceKind::Revolution(Revolution::new(profile, [0.0; 3], slant, ROUND).unwrap())
    };
    let tube = SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 3.5, slant, (ROUND, WIDE)).unwrap());
    // In that plane, b from 2 out to 2.75 and back: ρ = b·√2 runs from 2.83 to 3.89 and back, so
    // the coaxial cylinder ρ = 3.5 meets it in two rings.
    let flat = about(bezier(&[
        [2.0, -2.0, 0.0],
        [4.0, -2.0, 1.0],
        [5.0, -1.0, 2.0],
        [5.0, 1.0, 3.0],
    ]));
    let r = natural(&tube, &flat, &mut meter()).unwrap().unwrap();
    assert_eq!(r.items().len(), 2, "{r:?}");
    for item in r.items() {
        assert!(
            matches!(item, Item::Curve { curve, .. } if matches!(curve.as_ref(), CurveKind::Circle(_))),
            "{item:?}"
        );
    }
    // The first control point off the axis names the plane even where a later one is on the axis
    // (2, 2, 2), whose w × e is zero: the last, (6, 2, 1), stands (1, 1, −2) off the plane, so this
    // is no meridian.
    let twisted = about(bezier(&[
        [2.0, -2.0, 0.0],
        [4.0, -2.0, 1.0],
        [2.0, 2.0, 2.0],
        [6.0, 2.0, 1.0],
    ]));
    assert!(natural(&tube, &twisted, &mut meter()).unwrap().is_none());
}

#[test]
fn a_meridian_whose_plane_is_exact_only_with_the_errors_of_its_products_is_one() {
    // About the slanted axis e = (1, 1, 1), the control points α·e + β·w for one point w of
    // 48-bit coordinates, each sum exact: all four are in the plane of e and w. Each product of
    // two coordinates in the test that they are rounds, and the rounded products alone sum to
    // about 6e−16, 3e−16 and −6e−16 for the last three points, not zero: only with the error of
    // each product kept is the sum exactly zero. ρ runs from |w⊥| = 3.64 to 1.1875 times that,
    // 4.32, and back, so the coaxial cylinder ρ = 4 meets it in two rings.
    let slant = Frame::new([1.0, -1.0, 0.0], [1.0, 1.0, 1.0]).unwrap();
    let w = [
        2.345_678_900_000_003,
        -2.718_281_799_999_999_7,
        0.618_033_900_000_000_4,
    ];
    let at = |alpha: f64, beta: f64| w.map(|x| alpha + beta * x);
    let profile = bezier(&[at(0.0, 1.0), at(1.0, 1.25), at(2.0, 1.25), at(3.0, 1.0)]);
    let span = profile.domain();
    let profile = Trimmed::new(Basis::try_from(profile).unwrap(), span).unwrap();
    let turned = SurfaceKind::Revolution(Revolution::new(profile, [0.0; 3], slant, ROUND).unwrap());
    let tube = SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 4.0, slant, (ROUND, WIDE)).unwrap());
    let r = natural(&tube, &turned, &mut meter()).unwrap().unwrap();
    assert_eq!(r.items().len(), 2, "{r:?}");
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
    // ρ = 3 + t², z = t for t in [−1, 1]: z = 1/2 cuts ρ = 3.25. Its ball, of squared radius
    // (1 + 1)(1 + 1) = 4 about ρ = 3, stays off the axis.
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
    // About ρ = 2 the ball, of radius 2, touches the axis and does not cross it, so the parabola,
    // ρ = 2 + t² ≥ 2, is proven in the half-plane and z = 1/2 cuts ρ = 2.25. (This case was
    // written expecting a refusal, as if touching were crossing; a closed ball that touches the
    // axis lies in the closed half-plane, as a horn torus's circle does.)
    let p = Parabola::new([2.0, 0.0, 0.0], flat, (-1.0, 1.0)).unwrap();
    let r = natural(
        &level(0.5),
        &revolution(CurveKind::Parabola(p)),
        &mut meter(),
    )
    .unwrap()
    .unwrap();
    assert_rings(&r, &[(2.25, 0.5)]);
    // About ρ = 1.5 the ball reaches across the axis: left to the general paths, though the
    // parabola itself does not cross it.
    let p = Parabola::new([1.5, 0.0, 0.0], flat, (-1.0, 1.0)).unwrap();
    let r = revolution(CurveKind::Parabola(p));
    assert!(natural(&level(0.5), &r, &mut meter()).unwrap().is_none());
    // A hyperbola has no ball here: left to the general paths.
    let h = Hyperbola::new([3.0, 0.0, 0.0], flat, (-1.0, 1.0)).unwrap();
    let r = revolution(CurveKind::Hyperbola(h));
    assert!(natural(&level(0.5), &r, &mut meter()).unwrap().is_none());
}

#[test]
fn a_curved_profile_with_its_ends_and_middle_on_one_side_that_crosses_the_axis_is_no_meridian() {
    // The three points the old test read stand on one side of the axis, and each curve crosses it
    // between them, so a cut there meets the far part, whose rings the old test lost (a crossing at
    // ρ < 0 is dropped). Either the true rings or none (the general paths).
    //
    // The circle about ρ = 1 of radius 2 starting at its top: its ends (1, 0, 2) and its middle
    // (1, 0, −2) are at ρ = 1, and it reaches ρ = −1 at z = 0, where z = 0 cuts the rings ρ = 1
    // and ρ = 3.
    let top = Frame::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0]).unwrap();
    let round = Circle::new([1.0, 0.0, 0.0], 2.0, top, ROUND).unwrap();
    let round = revolution(CurveKind::Circle(round));
    if let Some(r) = natural(&level(0.0), &round, &mut meter()).unwrap() {
        assert_rings(&r, &[(1.0, 0.0), (3.0, 0.0)]);
    }
    // A cubic Bézier curve, z = 3t, at ρ = 2 at its ends and ρ = 1/2 at its middle: ρ(1/4) = −1/4,
    // so z = 3/4 cuts the ring ρ = 1/4.
    let bent = revolution(bezier(&[
        [2.0, 0.0, 0.0],
        [-4.0, 0.0, 1.0],
        [4.0, 0.0, 2.0],
        [2.0, 0.0, 3.0],
    ]));
    if let Some(r) = natural(&level(0.75), &bent, &mut meter()).unwrap() {
        assert_rings(&r, &[(0.25, 0.75)]);
    }
}

#[test]
fn the_issues_cone_scaled_by_three_meets_the_scaled_funnel_in_its_ring() {
    // The issue's input as it was met: the cone ρ = 4 − z/2 and the revolution ρ = 7 + z/2, parallel
    // to the cone's other ruling, meet in the ring ρ = 11/2 at z = −3; both moved by x ↦ 3x, they
    // gave Err(InvalidInput). Moved, the ring is ρ = 33/2 at z = −9.
    let cone = Cone::new([0.0; 3], 4.0, -0.5, up(), (ROUND, (-10.0, 10.0))).unwrap();
    let funnel = revolution(segment([2.0, 0.0, -10.0], [12.0, 0.0, 10.0]));
    let three = [[3, 0, 0], [0, 3, 0], [0, 0, 3]];
    let three = TypedTransform::from(RationalTransform::new(three, [0, 0, 0], 1).unwrap());
    let moved = |s: &SurfaceKind| {
        let m = s.transformed(&three, &mut meter()).unwrap();
        assert_eq!(m.deviation, 0.0, "{s:?}");
        m.value
    };
    let (cone, funnel) = (moved(&SurfaceKind::Cone(cone)), moved(&funnel));
    let r = natural(&cone, &funnel, &mut meter()).unwrap().unwrap();
    assert_rings(&r, &[(16.5, -9.0)]);
}

#[test]
fn a_horn_circle_whose_middle_rounds_across_the_axis_is_read_on_its_centre_side() {
    // A horn torus's tube, about ρ = 1 of radius 1 in y = 0, which touches the axis: its frame is
    // turned a little about its normal and its span starts opposite the point that touches, so its
    // middle is that point. As rounded, the middle stands across the axis: the meridian was read
    // mirrored, about ρ = −1, so z = 0 gave the point on the axis alone and lost the ring ρ = 2,
    // and the cylinder ρ = 3/2 and the sphere of radius 3/2 gave nothing.
    let turned = Frame::new(
        [0.999_925_666_296_434_9, 0.0, 0.012_192_697_881_553_619],
        [0.0, 1.0, 0.0],
    );
    let span = (-1.996_118_847_557_761, 0.003_881_152_442_238_966_5);
    let tube = CurveKind::Circle(Circle::new([1.0, 0.0, 0.0], 1.0, turned.unwrap(), span).unwrap());
    let (lo, hi) = tube.domain();
    let middle = tube.point(lo * 0.5 + hi * 0.5, Limit::FromRight).unwrap();
    assert!(middle[0] < 0.0, "{middle:?}");
    let horn = revolution(tube);
    // The span is short of a whole turn by 1.1e−16 half-turns, and the ring ρ = 2 stands 6.9e−18
    // half-turns inside the arc's end: its turn, read on the profile's stored frame, not on its
    // rounded image in the half-plane, is told inside the arc (issue 1142).
    let mut m = meter();
    let r = natural(&level(0.0), &horn, &mut m).unwrap().unwrap();
    assert_rings(&r, &[(0.0, 0.0), (2.0, 0.0)]);
    assert_eq!(m.used().work, 4284);
    let z = 0.75_f64.sqrt();
    let r = natural(&cylinder(1.5, WIDE), &horn, &mut meter())
        .unwrap()
        .unwrap();
    assert_rings(&r, &[(1.5, -z), (1.5, z)]);
    // ρ² + z² = 9/4 and (ρ − 1)² + z² = 1 meet at ρ = 9/8.
    let ball = Sphere::new([0.0; 3], 1.5, up(), (ROUND, (-0.5, 0.5))).unwrap();
    let z = (2.25_f64 - 1.265_625).sqrt();
    let r = natural(&SurfaceKind::Sphere(ball), &horn, &mut meter())
        .unwrap()
        .unwrap();
    assert_rings(&r, &[(1.125, -z), (1.125, z)]);
}

#[test]
fn a_profile_from_a_spheres_pole_meets_it_at_the_pole_by_its_proof() {
    // The segment from the pole (0, 0, 5) of the sphere of radius 5 to (8, 0, 11) sweeps a funnel
    // that stands above the sphere's cap, so they meet at the pole alone. The profile's rounded
    // image in the half-plane misses the pole by 1.7e−15, so the meeting's turn reaches the half
    // circle's end 1/2: the pole is proven on the profile by exact signs on its stored doubles
    // (issue 1142).
    let ball = Sphere::new([0.0; 3], 5.0, up(), (ROUND, (-0.5, 0.5))).unwrap();
    let funnel = revolution(segment([0.0, 0.0, 5.0], [8.0, 0.0, 11.0]));
    let mut m = meter();
    let r = natural(&SurfaceKind::Sphere(ball), &funnel, &mut m)
        .unwrap()
        .unwrap();
    assert!(matches!(r.items(), [Item::Point { .. }]), "{r:?}");
    assert_rings(&r, &[(0.0, 5.0)]);
    assert_eq!(m.used().work, 104);
}

#[test]
fn a_profile_whose_line_crosses_the_axis_past_its_span_is_read_on_its_own_side() {
    // The profile is the piece t in (2, 3) of the line (21 − 6t, 0, 13 − 2t), from (9, 0, 9) to
    // (3, 0, 7); its line crosses the axis at t = 3.5, past its end. The sphere of radius 10 meets
    // the line at t = 2.5, on the span, in the ring ρ = 6 at z = 8, and at t = 5.1, off it. The
    // true line is read in the half-plane toward the span's middle, t = 2.5: read toward a point
    // past the crossing (t = 4 or more) it is the mirror image, whose meeting (−6, 8) is off the
    // sphere's half circle, and the ring is lost (issue 1142).
    let ball = Sphere::new([0.0; 3], 10.0, up(), (ROUND, (-0.5, 0.5))).unwrap();
    let line = Line::new([21.0, 0.0, 13.0], [-6.0, 0.0, -2.0], (2.0, 3.0)).unwrap();
    let funnel = revolution(CurveKind::Line(line));
    let r = natural(&SurfaceKind::Sphere(ball), &funnel, &mut meter())
        .unwrap()
        .unwrap();
    assert_rings(&r, &[(6.0, 8.0)]);
}

#[test]
fn a_circle_near_the_axis_in_a_plane_of_a_slanted_direction_is_read_in_that_plane() {
    // In the plane of the z axis and (1, 3, 0), of normal (3, −1, 0): the circle about (1, 3, 0),
    // √10 off the axis, of radius r = 3.162277660168379, the largest double whose square is at most
    // 10, so it is proven off the axis and all but touches it at its middle. As rounded, the middle
    // stands 2.5e−16 off the axis and 3.5e−17 off the plane, so the map read there turned the plane
    // by about 8 degrees, and z = 0 cut the far ring at ρ = 6.26099, not √10 + r = 6.32456.
    let r = 3.162_277_660_168_379;
    let tilted = Frame::new([1.0, 3.0, -1.5], [3.0, -1.0, 0.0]).unwrap();
    let tm = -0.859_017_035_971_376;
    let tube = Circle::new([1.0, 3.0, 0.0], r, tilted, (tm - 1.0, tm + 1.0)).unwrap();
    let near = revolution(CurveKind::Circle(tube));
    let got = natural(&level(0.0), &near, &mut meter()).unwrap().unwrap();
    assert_rings(&got, &[(0.0, 0.0), (10.0_f64.sqrt() + r, 0.0)]);
    let gap = 3.0 - 10.0_f64.sqrt();
    let z = (r * r - gap * gap).sqrt();
    let got = natural(&cylinder(3.0, WIDE), &near, &mut meter())
        .unwrap()
        .unwrap();
    assert_rings(&got, &[(3.0, -z), (3.0, z)]);
}
