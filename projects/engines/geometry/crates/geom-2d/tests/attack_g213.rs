//! Attacks on piece G213: spline fitting and biarcs (`geom_2d::fit`; chapter 14.1 and 14.3 of the
//! kernel architecture). Every test tries to break a claim of the piece's done-when: a versioned
//! fit-point to control-point algorithm and a biarc conversion within tolerance, byte-identical
//! across targets.
//!
//! - **Versioned and deterministic**: the same points give the same spline, bit for bit, twice; the
//!   spline carries its intent and [`FIT_VERSION`].
//! - **Within a stated bound**: the spline's curve, rebuilt in `geom-nurbs`, passes every fit point
//!   within the deviation the fit reports, and a tolerance below it is refused.
//! - **Biarcs are exact**: each arc's centre is rational, its circle passes through both its ends
//!   and meets the tangent at the outer end exactly, checked in whole numbers; the joint's gap is
//!   small.
//! - **Refusals**: parallel tangents, a zero tangent, a joint on a tangent line, too few points,
//!   coincident points.
//! - **Covering**: a spline of a quarter circle is covered by biarcs within the tolerance.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "p, t, c, j and the like are the geometry's"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_2d::carrier::Circle2;
use geom_2d::fit::{
    BiarcArc, FIT_VERSION, FitError, MAX_DEPTH, biarc, fit_spline, spline_to_biarcs,
};
use geom_math::F2;
use geom_nurbs::{Basis, Curve, Limit};
use geom_num::grid::{P2, Steps};
use geom_run::{Budget, WorkMeter};

fn p2(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Points on a quarter of a circle of radius `r`, by a rational parametrisation so they are
/// whole numbers only approximately: rounded to the grid.
fn quarter(r: i64, n: i64) -> Vec<P2> {
    (0..=n)
        .map(|k| {
            let a = std::f64::consts::FRAC_PI_2 * k as f64 / n as f64;
            p2((r as f64 * a.cos()).round() as i64, (r as f64 * a.sin()).round() as i64)
        })
        .collect()
}

#[test]
fn a_fit_is_deterministic_and_carries_its_intent() {
    let pts = quarter(1000, 8);
    let a = fit_spline(&pts, Some([0, 1]), Some([-1, 0]), 3, Steps(5), &mut meter()).unwrap();
    let b = fit_spline(&pts, Some([0, 1]), Some([-1, 0]), 3, Steps(5), &mut meter()).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.spline.degree(), 3);
    assert_eq!(a.spline.control().len(), pts.len());
    assert_eq!(a.spline.knots().len(), pts.len() + 4);
    let intent = a.spline.intent().unwrap();
    assert_eq!(intent.points, pts);
    assert_eq!(intent.start_tangent, Some([0, 1]));
    assert_eq!(intent.end_tangent, Some([-1, 0]));
    assert_eq!(intent.tolerance, Steps(5));
    assert_eq!(intent.version, FIT_VERSION);
    assert_eq!(FIT_VERSION, 1);
}

#[test]
fn the_knots_are_clamped_increasing_fractions() {
    let pts = quarter(500, 6);
    let fit = fit_spline(&pts, None, None, 3, Steps(4), &mut meter()).unwrap();
    let k = fit.spline.knots();
    assert!(k[..4].iter().all(|f| f.is_zero()));
    assert!(k[k.len() - 4..].iter().all(|f| f.numerator() == f.denominator()));
    assert!(k.windows(2).all(|w| w[0].at_most(w[1])));
    // Every knot is a multiple of 1 / (3 · 2^20).
    assert!(k.iter().all(|f| (3 << 20) % f.denominator() == 0));
}

#[test]
fn the_curve_passes_every_fit_point_within_the_reported_bound() {
    for (r, n) in [(100, 5), (1000, 8), (50_000, 12)] {
        let pts = quarter(r, n);
        let fit = fit_spline(&pts, None, None, 3, Steps(8), &mut meter()).unwrap();
        let knots: Vec<f64> = fit
            .spline
            .knots()
            .iter()
            .map(|f| f.numerator() as f64 / f.denominator() as f64)
            .collect();
        let ctrl: Vec<F2> = fit
            .spline
            .control()
            .iter()
            .map(|p| F2::new(p.x.get() as f64, p.y.get() as f64).unwrap())
            .collect();
        let curve = Curve::<2>::new(Basis::new(3, knots).unwrap(), &ctrl).unwrap();
        // Chord-length parameters on the rounded grid, as the fit chose them.
        let total: f64 = pts
            .windows(2)
            .map(|w| {
                let dx = (w[1].x.get() - w[0].x.get()) as f64;
                let dy = (w[1].y.get() - w[0].y.get()) as f64;
                (dx * dx + dy * dy).sqrt()
            })
            .sum();
        let mut run = 0.0;
        for (i, q) in pts.iter().enumerate() {
            if i > 0 {
                let dx = (q.x.get() - pts[i - 1].x.get()) as f64;
                let dy = (q.y.get() - pts[i - 1].y.get()) as f64;
                run += (dx * dx + dy * dy).sqrt();
            }
            let u = (run / total * 1_048_576.0).round() / 1_048_576.0;
            let at = curve.point(u, Limit::FromRight).unwrap();
            let miss = ((at[0] - q.x.get() as f64).powi(2) + (at[1] - q.y.get() as f64).powi(2)).sqrt();
            assert!(miss <= fit.deviation.0 as f64 + 1e-6, "r {r} point {i}: {miss} > {:?}", fit.deviation);
        }
    }
}

#[test]
fn a_tolerance_below_the_bound_is_refused() {
    let pts = quarter(1000, 8);
    let ok = fit_spline(&pts, None, None, 3, Steps(8), &mut meter()).unwrap();
    assert!(ok.deviation.0 >= 1, "rounding to the grid always costs something here");
    let refused = fit_spline(&pts, None, None, 3, Steps(0), &mut meter()).unwrap_err();
    assert!(matches!(refused, FitError::Tolerance { deviation } if deviation == ok.deviation));
}

#[test]
fn fewer_points_than_the_degree_needs_and_coincident_points_are_refused() {
    let two = [p2(0, 0), p2(10, 10)];
    assert_eq!(
        fit_spline(&two, None, None, 3, Steps(9), &mut meter()).unwrap_err(),
        FitError::Count
    );
    let twin = [p2(0, 0), p2(0, 0), p2(10, 10), p2(20, 0), p2(30, 10)];
    assert_eq!(
        fit_spline(&twin, None, None, 3, Steps(9), &mut meter()).unwrap_err(),
        FitError::Coincident
    );
}

#[test]
fn a_meter_that_stops_stops_the_fit() {
    let pts = quarter(1000, 8);
    let mut tight = WorkMeter::new(Budget::new(1, 1));
    assert!(matches!(
        fit_spline(&pts, None, None, 3, Steps(8), &mut tight),
        Err(FitError::Stopped(_))
    ));
}

/// The exact checks of a biarc's arc: the circle's centre is C = (x, y) / w, `on` is a grid point;
/// `tangent` touches it at `at` when (C − at) · tangent = 0, and `ends` are on the circle.
fn check_arc(arc: &BiarcArc, at: P2, tangent: [i64; 2]) {
    let Circle2::Round(round) = arc.circle else {
        panic!("a biarc's circle is a round one");
    };
    let ([x, y], w) = round.centre().coordinates();
    let rel = |p: P2| (x - w * i128::from(p.x.get()), y - w * i128::from(p.y.get()));
    let (rx, ry) = rel(at);
    assert_eq!(
        rx * i128::from(tangent[0]) + ry * i128::from(tangent[1]),
        0,
        "the tangent is not perpendicular to the radius at its end"
    );
    let d2 = |p: P2| {
        let (a, b) = rel(p);
        a * a + b * b
    };
    assert_eq!(d2(arc.start), d2(arc.end), "the ends are not on one circle");
    assert_eq!(d2(round.on()), d2(arc.start));
}

#[test]
fn a_biarc_is_exact_at_the_ends_and_small_at_the_joint() {
    let cases = [
        (p2(0, 0), [1, 0], p2(10_000, 10_000), [0, 1]),
        (p2(0, 0), [1, 0], p2(10_000, -10_000), [0, -1]),
        (p2(-5_000, 200), [3, 4], p2(7_000, 9_000), [-1, 1]),
        (p2(0, 0), [1, 2], p2(100_000, 30_000), [2, -1]),
    ];
    for (p0, t0, p1, t1) in cases {
        let b = biarc(p0, t0, p1, t1).unwrap();
        check_arc(&b.first, p0, t0);
        check_arc(&b.second, p1, t1);
        // The joint is an end of both arcs.
        let ends = |a: &BiarcArc| [a.start, a.end];
        assert!(ends(&b.first).contains(&b.joint) && ends(&b.first).contains(&p0));
        assert!(ends(&b.second).contains(&b.joint) && ends(&b.second).contains(&p1));
        assert!(b.gap.is_finite() && b.gap < 1e-3, "gap {}", b.gap);
    }
}

#[test]
fn a_biarc_turns_the_way_its_tangent_does() {
    let left = biarc(p2(0, 0), [1, 0], p2(10_000, 10_000), [0, 1]).unwrap();
    assert!(left.first.forward, "heading east to a point to the north turns left");
    let right = biarc(p2(0, 0), [1, 0], p2(10_000, -10_000), [0, -1]).unwrap();
    assert!(!right.first.forward);
}

#[test]
fn no_biarc_where_the_tangents_are_parallel_or_zero() {
    assert_eq!(
        biarc(p2(0, 0), [1, 0], p2(100, 100), [1, 0]).unwrap_err(),
        FitError::NoBiarc
    );
    assert_eq!(
        biarc(p2(0, 0), [0, 0], p2(100, 100), [0, 1]).unwrap_err(),
        FitError::NoBiarc
    );
    assert_eq!(
        biarc(p2(0, 0), [1, 0], p2(100, 100), [0, 0]).unwrap_err(),
        FitError::NoBiarc
    );
}

#[test]
fn a_quarter_circle_spline_is_covered_by_biarcs_within_the_tolerance() {
    let pts = quarter(10_000, 10);
    let fit = fit_spline(&pts, None, None, 3, Steps(8), &mut meter()).unwrap();
    let cover = spline_to_biarcs(&fit.spline, Steps(12), &mut meter()).unwrap();
    assert!(!cover.biarcs.is_empty());
    assert!(cover.worst <= 12.0, "worst {}", cover.worst);
    // The chain starts at the first control point and ends at the last.
    let first = cover.biarcs.first().unwrap();
    let last = cover.biarcs.last().unwrap();
    let c = fit.spline.control();
    assert!(first.first.start == c[0] || first.first.end == c[0]);
    assert!(last.second.start == *c.last().unwrap() || last.second.end == *c.last().unwrap());
    assert!(MAX_DEPTH >= 8);
}
