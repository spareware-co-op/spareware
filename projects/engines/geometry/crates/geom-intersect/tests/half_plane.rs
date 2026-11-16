//! Two lines parallel to within rounding meet in the plane (issue 921, its second part).
//!
//! A cone scaled by 3 met a coaxial revolution with an error: the profile's direction, rounded
//! from 30 to 30.000000000000004 by the half-plane map, was parallel to the cone's other ruling
//! to within rounding, and two lines whose exact determinant is not zero divided by an enclosure
//! of it that held zero.

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

use geom_curve::{Basis, CurveKind, Frame, Line, ParamCurve, Trimmed};
use geom_intersect::curve::curves;
use geom_intersect::quadrics::natural;
use geom_intersect::result::{Intersection, Item};
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Revolution, SurfaceKind};

const ROUND: (f64, f64) = (0.0, 2.0);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(50_000_000))
}

fn up() -> Frame<3> {
    Frame::standard().unwrap()
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
