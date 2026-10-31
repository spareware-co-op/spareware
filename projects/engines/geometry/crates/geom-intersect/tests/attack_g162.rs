//! Attacks on piece G162's claims: nearest points by closed form and by certified search, typed
//! sets, ties, trimmed domains, and zero wrong answers on 10,000 cases with known feet.

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
use geom_curve::{CurveKind, Frame, ParamCurve};
use geom_intersect::{Continuum, Place, Projection, invert_curve, project_curve, project_surface};
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, ParamSurface, Plane, Sphere, SurfaceKind, Torus};
use geom_test::Seed;
use geom_test::rng::SplitMix64;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
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
    // Each search takes about 65 ms unoptimised, so a debug build runs a tenth of the cases.
    let rounds = if cfg!(debug_assertions) { 100 } else { 1_000 };
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
