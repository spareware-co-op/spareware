//! Attack tests from SMLib's confirmed defects, batch 3, third line: the procedural intersection
//! curve (`geom-intersect`, piece G175).
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib defects
//! (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: a curve whose reported bound holds against the exact curve
//! the surfaces' own equations give, or a defined refusal, and never a panic, a hang, a NaN or a
//! bound that the curve breaks.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::type_complexity,
    clippy::similar_names,
    reason = "tests: native arithmetic and indexing, platform maths on an independent oracle (the surfaces' own equations), prose that names defect files, and a failure stops the test"
)]

use geom_curve::{Frame, IntersectionCurve, ParamCurve, SurfaceRef};
use geom_intersect::IntersectError;
use geom_intersect::fit::fit;
use geom_intersect::general::subdivide;
use geom_intersect::march::{Branch, march};
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, ParamSurface, Plane, Sphere, SurfaceKind};

const FLOOR: f64 = 1.0 / 512.0;
const MARCH: f64 = 1.0 / 64.0;
const CEILING: f64 = 1.0 / 256.0;
const REFS: [SurfaceRef; 2] = [SurfaceRef(1), SurfaceRef(2)];
const TURN: f64 = 2.0;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

fn side(t: f64, (_, hi): (f64, f64)) -> Limit {
    if t < hi {
        Limit::FromRight
    } else {
        Limit::FromLeft
    }
}

fn branches(first: &SurfaceKind, second: &SurfaceKind) -> Vec<Branch> {
    let found = subdivide(first, second, FLOOR, &mut meter()).unwrap();
    let traced = march(first, second, &found, FLOOR, MARCH, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    traced.branches
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1]).hypot(a[2] - b[2])
}

/// What a fitted curve owes (12.12): every span's bound within the ceiling, and, at each of
/// `samples` parameters, the curve within its span's bound of `exact`, the distance to the true
/// curve by the surfaces' own equations, and each pcurve, mapped through its surface, within its
/// span's bound of the 3D curve.
fn honest(
    surfaces: [&SurfaceKind; 2],
    curve: &IntersectionCurve<3>,
    samples: u32,
    ceiling: f64,
    exact: impl Fn([f64; 3]) -> f64,
) {
    assert!(curve.error() <= ceiling, "{}", curve.error());
    let (lo, hi) = ParamCurve::domain(curve);
    for k in 0..=samples {
        let t = lo + (hi - lo) * f64::from(k) / f64::from(samples);
        let span = curve
            .spans()
            .iter()
            .find(|s| s.at.0 <= t && t <= s.at.1)
            .unwrap();
        assert!(span.curve <= ceiling, "{span:?}");
        let c = ParamCurve::point(curve, t, side(t, (lo, hi))).unwrap();
        assert!(c.iter().all(|x| x.is_finite()), "{t}: {c:?}");
        let off = exact(c);
        assert!(
            off <= span.curve + 1e-12,
            "{t}: {off} off the exact curve, past the bound {}",
            span.curve
        );
        for (i, (surface, pcurve)) in surfaces.into_iter().zip(curve.pcurves()).enumerate() {
            let [u, v] = pcurve.point(t, side(t, (lo, hi))).unwrap();
            let q = surface
                .point(u, v, (Limit::FromRight, Limit::FromRight))
                .unwrap();
            assert!(
                dist(q, c) <= span.pcurves[i] + 1e-12,
                "{t}, pcurve {i}: {} past {}",
                dist(q, c),
                span.pcurves[i]
            );
        }
    }
}

/// solidcore plan/upstream-issues/WR-495--mateedges-marches-offset-edges-between-analytic-faces.md,
/// WR-545--shell-edges-between-analytic-faces-stay-marched-approximations.md and
/// ISSUE-489--facefaceintersect-discards-the-intersection-uv-curves-when.md (b4de2149; WR-495
/// confirmed on the pinned C++): an edge between two analytic faces was a marched B-spline
/// approximation 2.4e-6 off the circle it should be, and the face's own UV curves were thrown away
/// for a projection that added another approximation, leaving gaps of 2e-6 to 4e-6. The input is
/// the defects': a plane through a sphere, where the exact curve is the circle of radius 4 about
/// (1, 2) at height 0, marched and fitted at a ceiling of 1/256 and again at 1/4096. The curve is
/// fitted within its ceiling, or refused as out of budget, and never with a bound its distance
/// from the exact circle breaks, nor with pcurves that, mapped through their surfaces, stray from
/// it by more than their own bounds.
#[test]
fn wr_495_545_a_marched_circle_is_fitted_within_a_bound_it_keeps() {
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.0625, 0.03125, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-9.0, 9.0), (-9.0, 9.0)),
        )
        .unwrap(),
    );
    let sphere = SurfaceKind::Sphere(
        Sphere::new(
            [1.0, 2.0, 3.0],
            5.0,
            Frame::standard().unwrap(),
            ((0.0625, TURN), (-0.49, 0.49)),
        )
        .unwrap(),
    );
    let found = branches(&plane, &sphere);
    assert_eq!(found.len(), 1, "{found:?}");
    let circle = |p: [f64; 3]| p[2].hypot((p[0] - 1.0).hypot(p[1] - 2.0) - 4.0);
    let mut fitted = 0;
    for ceiling in [CEILING, 1.0 / 4096.0] {
        match fit((&plane, &sphere), REFS, &found[0], ceiling, &mut meter()) {
            Ok(curve) => {
                fitted += 1;
                honest([&plane, &sphere], &curve, 200, ceiling, circle);
            }
            Err(IntersectError::BudgetExceeded) => {}
            Err(e) => panic!("{ceiling}: {e:?}"),
        }
    }
    assert!(fitted >= 1, "not even the looser ceiling was reached");
}

/// A cylinder about the line through the origin along `axis`, whose parameter u starts at `from`,
/// a unit vector square to `axis`, and runs over `turn` half-turns, with v over [-3, 3].
fn pipe(axis: [f64; 3], from: [f64; 3], turn: f64) -> SurfaceKind {
    SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            1.0,
            Frame::new(from, axis).unwrap(),
            ((0.0, turn), (-3.0, 3.0)),
        )
        .unwrap(),
    )
}

/// solidcore plan/upstream-issues/WR-697--mitre-turn-marched-not-exact.md (b4de2149, confirmed in
/// the port and on the fork's C++ build): the mitre of two equal cylinder rounds whose axes meet
/// is an ellipse in the plane of symmetry, and was a marched and fitted curve 1.4e-5 off both
/// rounds, with its trim curves 1.4e-5 off that. The input is the defect's: two cylinders of
/// radius 1, one along x and one along y, cut to the part where the curve leaves the domain of one
/// at each end and meets no singular point: the mitre is the ellipse in the plane x = y, the point
/// (cos t, cos t, sin t). The fitted curve is within its bound of the plane and of both cylinders,
/// the pcurves within theirs, or the fit is refused as out of budget.
#[test]
fn wr_697_the_mitre_of_two_equal_cylinders_is_fitted_within_a_bound_it_keeps() {
    // Along x, whose points are (v, cos a, sin a) for a from -0.1 pi to 0.4 pi.
    let a0 = -0.1 * core::f64::consts::PI;
    let along_x = pipe([1.0, 0.0, 0.0], [0.0, a0.cos(), a0.sin()], 0.5);
    // Along y, whose points are (cos s, v, sin s) for s from 0.45 pi down to 0.
    let b0 = 0.45 * core::f64::consts::PI;
    let along_y = pipe([0.0, 1.0, 0.0], [b0.cos(), 0.0, b0.sin()], 0.45);
    let found = branches(&along_x, &along_y);
    assert_eq!(found.len(), 1, "{found:?}");
    let mitre = |p: [f64; 3]| {
        let to_plane = (p[0] - p[1]).abs() / core::f64::consts::SQRT_2;
        let a = (p[1].hypot(p[2]) - 1.0).abs();
        let b = (p[0].hypot(p[2]) - 1.0).abs();
        to_plane.max(a).max(b)
    };
    match fit((&along_x, &along_y), REFS, &found[0], CEILING, &mut meter()) {
        Ok(curve) => honest([&along_x, &along_y], &curve, 200, CEILING, mitre),
        Err(IntersectError::BudgetExceeded) => {}
        Err(e) => panic!("{e:?}"),
    }
}
