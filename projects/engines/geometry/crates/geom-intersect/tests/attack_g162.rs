//! Attacks on piece G162's claims: nearest points by closed form and by certified search, typed
//! sets, ties, trimmed domains, and zero wrong answers on 10,000 cases with known feet (in a release
//! build; a debug one runs fewer, so a mutant fails in seconds).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::axes::Axes;
use geom_curve::conic::{Circle, Ellipse};
use geom_curve::line::Line;
use geom_curve::transition::{CurvatureLaw, Transition};
use geom_curve::{CurveKind, Frame, ParamCurve, Polynomial};
use geom_intersect::project::{negative, positive};
use geom_intersect::{
    Continuum, Place, Projection, invert_curve, invert_surface, project_curve, project_surface,
};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Curve, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, ParamSurface, Plane, Sphere, SurfaceKind, Torus};
use geom_test::Seed;
use geom_test::rng::SplitMix64;

/// A meter above what any case here takes, so a search that loses its way fails, not hangs.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 18))
}

const FLOOR: f64 = 1e-6;
const TURN: f64 = 2.0;

fn circle(radius: f64, domain: (f64, f64)) -> CurveKind<2> {
    CurveKind::Circle(Circle::new([1.0, 2.0], radius, Frame::standard().unwrap(), domain).unwrap())
}

fn rng(label: &str, seed: u64) -> SplitMix64 {
    SplitMix64::new(geom_test::seed::resolve(label, Seed(seed)).unwrap())
}

fn unit(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

#[test]
fn a_lines_foot_is_its_closed_form_and_a_past_foot_is_its_end() {
    let segment = CurveKind::Line(Line::segment([0.0, 0.0], [10.0, 0.0]).unwrap().value);
    let inside = project_curve(&segment, &[3.0, 4.0], FLOOR, &mut meter()).unwrap();
    let foot = inside.first().unwrap();
    assert!(foot.at[0].contains(0.3), "{foot:?}");
    assert!(foot.distance.contains(4.0), "{foot:?}");
    assert_eq!(foot.place, Place::Foot);
    let past = project_curve(&segment, &[30.0, 4.0], FLOOR, &mut meter()).unwrap();
    let end = past.first().unwrap();
    assert_eq!(end.place, Place::Boundary);
    assert!(end.point[0].contains(10.0), "{end:?}");
}

#[test]
fn a_point_on_a_circles_axis_has_the_whole_circle() {
    let c = circle(5.0, (0.0, TURN));
    let found = project_curve(&c, &[1.0, 2.0], FLOOR, &mut meter()).unwrap();
    let Projection::Set { set, distance } = found else {
        panic!("{found:?}");
    };
    assert_eq!(set, Continuum::Curve);
    assert!(distance.contains(5.0), "{distance:?}");
}

#[test]
fn a_tie_keeps_both_and_says_so() {
    // A half circle's two ends, seen from straight above its middle's far side: both nearest.
    let c = circle(5.0, (0.0, 1.0));
    let found = project_curve(&c, &[1.0, -8.0], FLOOR, &mut meter()).unwrap();
    assert!(found.tie_broken(), "{found:?}");
    let Projection::Points(points) = &found else {
        panic!("{found:?}");
    };
    assert_eq!(points.len(), 2);
    assert!(points[0].at[0].contains(0.0), "{points:?}");
    assert!(points[1].at[0].contains(1.0), "{points:?}");
    assert_eq!(found.first().unwrap().place, Place::Boundary);
}

/// `t` on `curve`, moved `d` along the normal, whose nearest point is t's while `d` keeps it inside
/// the curve's evolute: the foot must enclose `t`.
fn holds_curve(curve: &CurveKind<2>, t: f64, d: f64) {
    let mut out = [[0.0; 2]; 2];
    curve.derivatives(t, Limit::FromRight, &mut out).unwrap();
    let [c, v] = out;
    let len = v[0].hypot(v[1]);
    let p = [c[0] - v[1] / len * d, c[1] + v[0] / len * d];
    let found = project_curve(curve, &p, FLOOR, &mut meter()).unwrap();
    let slack = 2e-5;
    let foot = found.first().unwrap_or_else(|| panic!("{found:?}"));
    assert!(
        foot.at[0].lo() - slack <= t && t <= foot.at[0].hi() + slack,
        "t {t} d {d}: {foot:?}"
    );
    assert!(foot.distance.lo() - 1e-5 <= d.abs() && d.abs() <= foot.distance.hi() + 1e-5);
}

#[test]
fn zero_wrong_answers_on_curves_with_known_feet() {
    let mut r = rng("g162 curves", 0x4731_3632);
    for _ in 0..3_000 {
        let radius = 1.0 + unit(&mut r) * 100.0;
        let t = unit(&mut r) * 1.8 + 0.1;
        let d = (unit(&mut r) - 0.5) * radius * 0.9;
        holds_curve(&circle(radius, (0.0, TURN)), t, d);
    }
    for _ in 0..3_000 {
        let a = 5.0 + unit(&mut r) * 50.0;
        let b = 1.0 + unit(&mut r) * a * 0.5;
        // An offset below the smallest radius of curvature, b²/a, keeps the foot nearest.
        let limit = 0.45 * b * b / a;
        let d = (unit(&mut r) - 0.5) * 2.0 * limit;
        let e = CurveKind::Ellipse(
            Ellipse::new(
                [0.0, 0.0],
                Axes::Frame {
                    frame: Frame::standard().unwrap(),
                    a,
                    b,
                },
                (0.0, TURN),
            )
            .unwrap(),
        );
        holds_curve(&e, unit(&mut r) * 1.8 + 0.1, d);
    }
}

fn holds_surface(s: &SurfaceKind, u: f64, v: f64, d: f64) {
    let [c, su, sv] = s
        .partials(u, v, (Limit::FromRight, Limit::FromRight))
        .unwrap();
    let n = [
        su[1] * sv[2] - su[2] * sv[1],
        su[2] * sv[0] - su[0] * sv[2],
        su[0] * sv[1] - su[1] * sv[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    let p = [
        c[0] + n[0] / len * d,
        c[1] + n[1] / len * d,
        c[2] + n[2] / len * d,
    ];
    let found = project_surface(s, &p, FLOOR, &mut meter()).unwrap();
    let foot = found.first().unwrap_or_else(|| panic!("{found:?}"));
    let slack = 1e-4;
    assert!(
        foot.distance.lo() - slack <= d.abs() && d.abs() <= foot.distance.hi() + slack,
        "{u} {v} {d}: {foot:?}"
    );
    let [a, b] = foot.at;
    assert!(a.lo() - slack <= u && u <= a.hi() + slack, "{u}: {foot:?}");
    assert!(b.lo() - slack <= v && v <= b.hi() + slack, "{v}: {foot:?}");
}

#[test]
fn zero_wrong_answers_on_surfaces_with_known_feet() {
    // Each search takes about 65 ms unoptimised, so a debug build runs a fortieth of the cases.
    let rounds = if cfg!(debug_assertions) { 25 } else { 1_000 };
    let mut r = rng("g162 surfaces", 0x4731_3633);
    let frame = Frame::standard().unwrap();
    for _ in 0..rounds {
        let radius = 2.0 + unit(&mut r) * 20.0;
        let s = SurfaceKind::Cylinder(
            Cylinder::new([1.0, 2.0, 3.0], radius, frame, ((0.0, TURN), (-30.0, 30.0))).unwrap(),
        );
        let d = (unit(&mut r) - 0.5) * radius * 0.9;
        holds_surface(&s, unit(&mut r) * 1.8 + 0.1, unit(&mut r) * 40.0 - 20.0, d);
    }
    for _ in 0..rounds {
        let radius = 2.0 + unit(&mut r) * 20.0;
        let s = SurfaceKind::Sphere(
            Sphere::new([1.0, 2.0, 3.0], radius, frame, ((0.0, TURN), (-0.5, 0.5))).unwrap(),
        );
        let d = (unit(&mut r) - 0.5) * radius * 0.9;
        holds_surface(&s, unit(&mut r) * 1.8 + 0.1, unit(&mut r) * 0.6 - 0.3, d);
    }
    for _ in 0..rounds {
        let minor = 1.0 + unit(&mut r) * 3.0;
        let major = minor * (2.0 + unit(&mut r));
        let s = SurfaceKind::Torus(
            Torus::new(
                [0.0, 0.0, 0.0],
                major,
                minor,
                frame,
                ((0.0, TURN), (0.0, TURN)),
            )
            .unwrap(),
        );
        let d = (unit(&mut r) - 0.5) * minor * 0.9;
        holds_surface(&s, unit(&mut r) * 1.8 + 0.1, unit(&mut r) * 1.8 + 0.1, d);
    }
    for _ in 0..rounds {
        let s = SurfaceKind::Plane(
            Plane::new(
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                ((-10.0, 10.0), (-10.0, 10.0)),
            )
            .unwrap(),
        );
        let d = unit(&mut r) * 5.0 + 0.1;
        holds_surface(&s, unit(&mut r) * 16.0 - 8.0, unit(&mut r) * 16.0 - 8.0, d);
    }
}

/// One foot on a domain's edge, reached by the edge's search and by boxes that touch only through
/// each other, is one nearest point, whose enclosures hold the exact foot: no tie.
#[test]
fn boxes_around_one_foot_are_one_nearest_point() {
    let frame = Frame::standard().unwrap();
    let s =
        SurfaceKind::Sphere(Sphere::new([0.0; 3], 6.0, frame, ((0.0, 1.0), (-0.5, 0.5))).unwrap());
    let p = [1.5, 0.0, -2.5];
    let found = project_surface(&s, &p, FLOOR, &mut meter()).unwrap();
    assert!(!found.tie_broken(), "{found:?}");
    let foot = found.first().unwrap();
    let length = 8.5f64.sqrt();
    assert!(foot.at[0].contains(0.0), "{foot:?}");
    assert!(foot.distance.lo() <= 6.0 - length, "{foot:?}");
    assert!(foot.distance.hi() >= 6.0 - length, "{foot:?}");
    let exact = [1.5 * 6.0 / length, 0.0, -2.5 * 6.0 / length];
    for (x, e) in foot.point.iter().zip(exact) {
        assert!(x.lo() - 1e-12 <= e && e <= x.hi() + 1e-12, "{foot:?}");
    }
}

#[test]
fn a_round_kind_seen_from_its_axis_is_a_typed_set() {
    let frame = Frame::standard().unwrap();
    let domain = ((0.0, TURN), (-5.0, 5.0));
    let cases: [(SurfaceKind, [f64; 3], f64); 2] = [
        (
            SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 4.0, frame, domain).unwrap()),
            [0.0, 0.0, 1.0],
            1.0,
        ),
        (
            SurfaceKind::Cone(Cone::new([0.0; 3], 4.0, 0.0, frame, domain).unwrap()),
            [0.0, 0.0, 2.0],
            2.0,
        ),
    ];
    for (surface, p, v) in cases {
        let found = project_surface(&surface, &p, FLOOR, &mut meter()).unwrap();
        let Projection::Set {
            set: Continuum::Parallel { v: at },
            distance,
        } = found
        else {
            panic!("{found:?}");
        };
        assert!(at.lo() - 1e-5 <= v && v <= at.hi() + 1e-5, "{at:?}");
        assert!(distance.lo() - 1e-5 <= 4.0 && 4.0 <= distance.hi() + 1e-5);
    }
    let ball = SurfaceKind::Sphere(
        Sphere::new([1.0, 2.0, 3.0], 5.0, frame, ((0.0, TURN), (-0.5, 0.5))).unwrap(),
    );
    let found = project_surface(&ball, &[1.0, 2.0, 3.0], FLOOR, &mut meter()).unwrap();
    assert!(
        matches!(
            found,
            Projection::Set {
                set: Continuum::Surface,
                ..
            }
        ),
        "{found:?}"
    );
}

#[test]
fn a_trimmed_surfaces_nearest_point_is_on_its_edge() {
    let s = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    let found = project_surface(&s, &[3.0, 0.5, 2.0], FLOOR, &mut meter()).unwrap();
    let edge = found.first().unwrap();
    assert_eq!(edge.place, Place::Boundary);
    assert!(edge.at[0].contains(1.0), "{edge:?}");
    // The foot on that edge is exactly at the middle of a span: it must still be narrowed.
    assert!(edge.at[1].hi() - edge.at[1].lo() < 1e-5, "{edge:?}");
    assert!(edge.at[1].contains(0.5), "{edge:?}");
    assert!(edge.distance.lo() - 1e-5 <= 8.0f64.sqrt(), "{found:?}");
}

/// Chapter 12.4's hardest everyday case: a point against a 50 km clothoid, certified to the grid.
#[test]
fn a_station_on_a_fifty_kilometre_clothoid_is_certified() {
    let length = 50_000.0;
    let law = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 1.0 / 2_000.0,
    };
    let curve = CurveKind::Transition(
        Transition::new([0.0, 0.0], [1.0, 0.0], length, law, &mut meter()).unwrap(),
    );
    let (a, b) = curve.domain();
    for station in [0.13, 0.5, 0.87] {
        let t = a + (b - a) * station;
        let mut out = [[0.0; 2]; 2];
        curve.derivatives(t, Limit::FromRight, &mut out).unwrap();
        let [c, v] = out;
        let len = v[0].hypot(v[1]);
        let d = 3.5;
        let p = [c[0] - v[1] / len * d, c[1] + v[0] / len * d];
        let found = project_curve(&curve, &p, 1e-3, &mut meter()).unwrap();
        let foot = found.first().unwrap_or_else(|| panic!("{found:?}"));
        let image = (foot.at[0].hi() - foot.at[0].lo()) * len;
        assert!(image <= 2e-3, "{image}");
        assert!(
            foot.at[0].lo() - 1e-3 / len <= t && t <= foot.at[0].hi() + 1e-3 / len,
            "{t}: {foot:?}"
        );
    }
}

#[test]
fn inversion_finds_a_point_on_the_kind_and_refuses_one_off_it() {
    let c = circle(5.0, (0.0, TURN));
    let on = c.point(0.25, Limit::FromRight).unwrap();
    let projected = project_curve(&c, &on, FLOOR, &mut meter()).unwrap();
    let found = invert_curve(&c, &on, 2.0 * FLOOR, FLOOR, &mut meter()).unwrap();
    let found = found.unwrap_or_else(|| panic!("{projected:?}"));
    assert!(found.at[0].lo() - 2e-6 <= 0.25 && 0.25 <= found.at[0].hi() + 2e-6);
    let off = [on[0] + 1.0, on[1]];
    assert_eq!(
        invert_curve(&c, &off, 1e-9, FLOOR, &mut meter()).unwrap(),
        None
    );
}

#[test]
fn the_budget_stops_the_search() {
    let c = circle(5.0, (0.0, TURN));
    let mut tight = WorkMeter::new(Budget::new(3, u64::MAX));
    let stopped = project_curve(&c, &[9.0, 2.0], FLOOR, &mut tight).unwrap_err();
    assert_eq!(stopped, geom_intersect::IntersectError::BudgetExceeded);
}

#[test]
fn a_floor_that_is_not_positive_is_refused() {
    let c = circle(5.0, (0.0, TURN));
    for floor in [0.0, -1.0, f64::NAN] {
        assert_eq!(
            project_curve(&c, &[9.0, 2.0], floor, &mut meter()).unwrap_err(),
            geom_intersect::IntersectError::InvalidInput
        );
    }
}

#[test]
fn a_sign_is_told_only_clear_of_zero() {
    let i = |a, b| Interval::new(a, b).unwrap();
    assert!(positive(i(1.0, 2.0)));
    assert!(!positive(i(0.0, 2.0)));
    assert!(!positive(i(-1.0, 2.0)));
    assert!(negative(i(-2.0, -1.0)));
    assert!(!negative(i(-2.0, 0.0)));
    assert!(!negative(i(-2.0, 1.0)));
}

#[test]
fn a_rays_nearest_point_is_its_foot_or_its_start() {
    let ray = CurveKind::Line(Line::new([0.0, 0.0], [1.0, 0.0], (0.0, f64::INFINITY)).unwrap());
    let behind = project_curve(&ray, &[-3.0, 4.0], FLOOR, &mut meter()).unwrap();
    let start = behind.first().unwrap();
    assert_eq!(start.place, Place::Boundary);
    assert!(start.at[0].contains(0.0), "{start:?}");
    assert!(start.distance.contains(5.0), "{start:?}");
    let ahead = project_curve(&ray, &[3.0, 4.0], FLOOR, &mut meter()).unwrap();
    let foot = ahead.first().unwrap();
    assert_eq!(foot.place, Place::Foot);
    assert!(foot.at[0].contains(3.0), "{foot:?}");
    assert!(foot.distance.contains(4.0), "{foot:?}");
}

/// A foot exactly on an end: the end and the search find it together, and it is a foot.
#[test]
fn a_foot_on_an_end_is_a_foot() {
    let arc = circle(5.0, (0.0, 1.0));
    let found = project_curve(&arc, &[9.0, 2.0], FLOOR, &mut meter()).unwrap();
    assert!(!found.tie_broken(), "{found:?}");
    let foot = found.first().unwrap();
    assert_eq!(foot.place, Place::Foot, "{found:?}");
    assert!(foot.at[0].contains(0.0), "{foot:?}");
    assert!(foot.distance.contains(3.0), "{foot:?}");
}

/// y = x² from the centre of curvature at its vertex: the distance grows only with t⁴, so no test
/// can settle the vertex, and what the floor leaves is one point.
#[test]
fn a_quartic_contact_is_one_point() {
    let parabola = CurveKind::Polynomial(
        Polynomial::new(vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], (-1.0, 1.0)).unwrap(),
    );
    let found = project_curve(&parabola, &[0.0, 0.5], FLOOR, &mut meter()).unwrap();
    let Projection::Points(points) = &found else {
        panic!("{found:?}");
    };
    assert_eq!(points.len(), 1, "{found:?}");
    assert!(points[0].at[0].contains(0.0), "{found:?}");
    assert!(points[0].distance.contains(0.5), "{found:?}");
}

/// A rational quarter circle is a NURBS, so no closed form sees its centre: every point is at one
/// distance, and the search says so instead of naming a foot.
#[test]
fn a_continuum_no_kind_names_is_undecided_not_a_foot() {
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let points = [[0.5, 0.0], [0.5, 0.5], [0.0, 0.5]];
    let weights = [1.0, core::f64::consts::FRAC_1_SQRT_2, 1.0];
    let arc = CurveKind::Nurbs(Curve::new_rational(basis, &points, &weights).unwrap());
    let found = project_curve(&arc, &[0.0, 0.0], 1e-2, &mut meter()).unwrap();
    let Projection::NonIsolated { enclosure: [t] } = found else {
        panic!("{found:?}");
    };
    assert!(t.hi() - t.lo() > 0.9, "{t:?}");
}

/// The same along a surface's second parameter, whose first bends nothing: the Hessian's first
/// entry alone is positive, and a box there is no bowl.
#[test]
fn a_continuum_along_a_surfaces_second_parameter_is_undecided() {
    let line = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let arc = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    let mut weights = Vec::new();
    for z in [-0.25, 0.25] {
        net.extend([[0.5, 0.0, z], [0.5, 0.5, z], [0.0, 0.5, z]]);
        weights.extend([1.0, core::f64::consts::FRAC_1_SQRT_2, 1.0]);
    }
    let surface = SurfaceKind::Nurbs(Surface::new_rational(line, arc, &net, &weights).unwrap());
    let found = project_surface(&surface, &[0.0, 0.0, 0.0], 1e-2, &mut meter());
    assert!(
        matches!(found, Ok(Projection::NonIsolated { .. })),
        "{found:?}"
    );
}

#[test]
fn each_round_kind_from_its_axis_has_its_own_set() {
    let frame = Frame::standard().unwrap();
    // A cylinder from its own centre: a parallel, not the whole surface.
    let cylinder = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 4.0, frame, ((0.0, TURN), (-5.0, 5.0))).unwrap(),
    );
    let found = project_surface(&cylinder, &[0.0; 3], FLOOR, &mut meter()).unwrap();
    let Projection::Set { set, distance } = found else {
        panic!("{found:?}");
    };
    assert!(matches!(set, Continuum::Parallel { .. }), "{set:?}");
    assert!(distance.contains(4.0), "{distance:?}");
    // A sphere from its axis, off its centre: the band's near edge, not the whole sphere.
    let band = SurfaceKind::Sphere(
        Sphere::new([0.0; 3], 5.0, frame, ((0.0, TURN), (-0.25, 0.25))).unwrap(),
    );
    let found = project_surface(&band, &[0.0, 0.0, 3.0], FLOOR, &mut meter()).unwrap();
    let Projection::Set { set, .. } = found else {
        panic!("{found:?}");
    };
    let Continuum::Parallel { v } = set else {
        panic!("{set:?}");
    };
    assert!(v.contains(0.25), "{v:?}");
    // A torus from its axis: the parallel nearest in its half-plane.
    let torus = SurfaceKind::Torus(
        Torus::new([0.0; 3], 6.0, 2.0, frame, ((0.0, TURN), (-1.0, 1.0))).unwrap(),
    );
    let found = project_surface(&torus, &[0.0, 0.0, 3.0], FLOOR, &mut meter()).unwrap();
    let Projection::Set { set, distance } = found else {
        panic!("{found:?}");
    };
    assert!(matches!(set, Continuum::Parallel { .. }), "{set:?}");
    let expected = 45.0f64.sqrt() - 2.0;
    assert!(
        distance.lo() - 1e-6 <= expected && expected <= distance.hi() + 1e-6,
        "{distance:?}"
    );
}

#[test]
fn inversion_finds_a_point_on_a_surface_and_refuses_one_off_it() {
    let frame = Frame::standard().unwrap();
    let cylinder = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 4.0, frame, ((0.0, 1.0), (-5.0, 5.0))).unwrap(),
    );
    let on = invert_surface(&cylinder, &[0.0, 4.0, 3.0], 1e-5, FLOOR, &mut meter()).unwrap();
    let foot = on.unwrap();
    assert!((foot.at[0].lo() - 0.5).abs() < 1e-5, "{foot:?}");
    assert!((foot.at[1].lo() - 3.0).abs() < 1e-5, "{foot:?}");
    let off = invert_surface(&cylinder, &[0.0, 7.0, 3.0], 1e-5, FLOOR, &mut meter()).unwrap();
    assert_eq!(off, None);
}

#[test]
fn a_floor_that_is_not_positive_is_refused_on_a_surface_too() {
    let frame = Frame::standard().unwrap();
    let sphere = SurfaceKind::Sphere(
        Sphere::new([0.0; 3], 4.0, frame, ((0.0, TURN), (-0.25, 0.25))).unwrap(),
    );
    for floor in [0.0, -1.0, f64::NAN] {
        assert_eq!(
            project_surface(&sphere, &[1.0, 1.0, 1.0], floor, &mut meter()),
            Err(geom_intersect::IntersectError::InvalidInput)
        );
    }
}

/// A spline's bound is clipped only along a span with a width, so an edge's enclosure must come from
/// its partials, or every edge would tie with the true foot.
#[test]
fn a_nurbs_surfaces_edges_do_not_tie_with_its_foot() {
    let basis = || Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let net = [
        [0.0, 0.0, 0.0],
        [0.0, 4.0, 0.0],
        [4.0, 0.0, 0.0],
        [4.0, 4.0, 0.0],
    ];
    let square = SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap());
    let inside = project_surface(&square, &[1.0, 3.0, 2.0], FLOOR, &mut meter()).unwrap();
    let foot = inside.first().unwrap();
    assert_eq!(foot.place, Place::Foot);
    assert!((foot.at[0].lo() - 0.25).abs() < 1e-5, "{foot:?}");
    assert!((foot.at[1].lo() - 0.75).abs() < 1e-5, "{foot:?}");
    assert!((foot.distance.lo() - 2.0).abs() < 1e-5, "{foot:?}");
    for (p, u, v) in [
        ([6.0, 1.0, 0.0], 1.0, 0.25),
        ([1.0, 6.0, 0.0], 0.25, 1.0),
        ([6.0, 6.0, 0.0], 1.0, 1.0),
    ] {
        let found = project_surface(&square, &p, FLOOR, &mut meter()).unwrap();
        let edge = found.first().unwrap();
        assert_eq!(edge.place, Place::Boundary, "{found:?}");
        assert!(
            edge.at[0].contains(u) || (edge.at[0].lo() - u).abs() < 1e-5,
            "{edge:?}"
        );
        assert!(
            edge.at[1].contains(v) || (edge.at[1].lo() - v).abs() < 1e-5,
            "{edge:?}"
        );
    }
}

/// A floor no double can reach: the search runs out of doubles between its cuts, and stops.
#[test]
fn a_floor_below_the_doubles_still_ends() {
    let arc = circle(5.0, (0.0, 1.5));
    let found = project_curve(&arc, &[4.0, 6.0], 1e-300, &mut meter()).unwrap();
    assert!(found.first().is_some(), "{found:?}");
    let frame = Frame::standard().unwrap();
    let sphere =
        SurfaceKind::Sphere(Sphere::new([0.0; 3], 4.0, frame, ((0.1, 1.9), (-0.3, 0.3))).unwrap());
    let found = project_surface(&sphere, &[1.0, 2.0, 0.5], 1e-300, &mut meter()).unwrap();
    assert!(found.first().is_some(), "{found:?}");
}

/// The least budget a call finishes under.
fn least_work(call: &dyn Fn(&mut WorkMeter<'_>) -> bool) -> u64 {
    let (mut too_little, mut enough) = (0u64, 1u64 << 22);
    while too_little + 1 < enough {
        let middle = too_little.midpoint(enough);
        if call(&mut WorkMeter::new(Budget::of_work(middle))) {
            enough = middle;
        } else {
            too_little = middle;
        }
    }
    enough
}

/// The searches' work, to the unit: a search is a pure function, so a change here is a change to
/// what is pruned, which no answer shows. The pinned numbers move only with the search itself.
#[test]
fn each_search_takes_the_work_it_always_took() {
    let frame = Frame::standard().unwrap();
    let domain = ((0.1, 1.9), (-0.3, 0.3));
    let sphere = SurfaceKind::Sphere(Sphere::new([0.0; 3], 4.0, frame, domain).unwrap());
    let cone = SurfaceKind::Cone(Cone::new([0.0; 3], 2.0, 0.5, frame, domain).unwrap());
    let torus = SurfaceKind::Torus(
        Torus::new([0.0; 3], 6.0, 2.0, frame, ((0.1, 1.9), (0.1, 1.9))).unwrap(),
    );
    let arc = circle(5.0, (0.0, 1.5));
    let ellipse = CurveKind::Ellipse(
        Ellipse::new(
            [0.0, 0.0],
            Axes::Frame {
                frame: Frame::standard().unwrap(),
                a: 9.0,
                b: 4.0,
            },
            (0.0, TURN),
        )
        .unwrap(),
    );
    let curves: [(&CurveKind<2>, [f64; 2], u64); 3] = [
        (&arc, [4.0, 6.0], 165),
        (&arc, [9.0, 2.0], 405),
        (&ellipse, [1.0, 2.0], 191),
    ];
    let mut work = Vec::new();
    let mut pins = Vec::new();
    for (curve, p, pinned) in curves {
        let call = |m: &mut WorkMeter<'_>| project_curve(curve, &p, FLOOR, m).is_ok();
        work.push(least_work(&call));
        pins.push(pinned);
    }
    let surfaces: [(&SurfaceKind, [f64; 3], u64); 3] = [
        (&sphere, [1.0, 2.0, 0.5], 46_607),
        (&cone, [1.0, 2.0, 0.5], 2_093),
        (&torus, [5.0, 2.0, 1.0], 59_498),
    ];
    for (surface, p, pinned) in surfaces {
        let call = |m: &mut WorkMeter<'_>| project_surface(surface, &p, FLOOR, m).is_ok();
        work.push(least_work(&call));
        pins.push(pinned);
    }
    assert_eq!(work, pins);
}
