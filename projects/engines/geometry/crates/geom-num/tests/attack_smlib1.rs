//! Attack tests from SMLib's confirmed defects, batch 1: `geom-num`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: the exact or correctly rounded answer, or a defined
//! refusal, and never a panic, a hang, a NaN or a silently wrong value.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::manual_midpoint,
    clippy::type_complexity,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::range_minus_one,
    reason = "tests: native arithmetic, casts and indexing, platform maths on an independent oracle (the formula as it is written), prose that names defect files, and a failure stops the test"
)]

use geom_num::brent::{self, BrentError};
use geom_num::grid::{COORD_MAX, Coord, GridStep, coarsen, refine};
use geom_num::math;
use geom_num::quadrature::{self, QuadError};
use geom_num::{NumError, Real};

fn r(v: f64) -> Real {
    Real::new(v).unwrap()
}

/// solidcore plan/upstream-issues/WR-94--smintegrator-integrateit-hides-integration-evaluator-failure.md
/// (b4de2149): when the integrator failed, the wrapper reported success with the value zero. The
/// input is the defect's: the constant function 1 over [0, 1] at accuracy 0. Ours must give 1
/// exactly, and when the integrand fails, or returns something that is no number, must say so.
#[test]
fn wr_94_an_integrator_failure_is_an_error_not_a_zero() {
    // The constant 1 over [0, 1] with both tolerances zero: every panel's error is exactly zero.
    for rule in 0..6 {
        let got =
            quadrature::integrate(|_| Ok(Real::ONE), r(0.0), r(1.0), r(0.0), r(0.0), rule, 50)
                .unwrap();
        assert_eq!(got.value.get(), 1.0, "rule {rule}");
        assert_eq!(got.error.get(), 0.0);
    }
    // An integrand that fails at its first evaluation: an error, not an integral of zero.
    let mut calls = 0u32;
    let failed = quadrature::integrate(
        |_| {
            calls += 1;
            Err(NumError::NotFinite)
        },
        r(0.0),
        r(1.0),
        r(1e-9),
        r(0.0),
        0,
        50,
    );
    assert_eq!(failed, Err(QuadError::Num(NumError::NotFinite)));
    assert!(calls >= 1);
    // An integrand that fails part-way: still an error, never a partial sum.
    let mut seen = 0u32;
    let late = quadrature::integrate(
        |x| {
            seen += 1;
            if seen > 20 {
                Err(NumError::OutsideDomain)
            } else {
                Ok(x)
            }
        },
        r(0.0),
        r(1.0),
        r(0.0),
        r(0.0),
        0,
        50,
    );
    assert!(
        matches!(
            late,
            Ok(_) | Err(QuadError::Num(NumError::OutsideDomain) | QuadError::CapReached)
        ),
        "{late:?}"
    );
    if let Ok(v) = late {
        // If it was answered it was answered right (the integral of x over [0, 1] is 1/2).
        assert!((v.value.get() - 0.5).abs() < 1e-15);
    }
    // A hard integrand at accuracy zero ends in a defined refusal, not a zero or a hang.
    let hard = quadrature::integrate(
        |x| math::sqrt(x.get()),
        r(0.0),
        r(1.0),
        r(0.0),
        r(0.0),
        0,
        50,
    );
    match hard {
        Ok(v) => assert!((v.value.get() - 2.0 / 3.0).abs() < 1e-12),
        Err(e) => assert!(matches!(e, QuadError::CapReached | QuadError::TooNarrow)),
    }
    // A reversed or empty interval and a negative tolerance are refused.
    assert_eq!(
        quadrature::integrate(|_| Ok(Real::ONE), r(1.0), r(0.0), r(0.0), r(0.0), 0, 5).map(|_| ()),
        Err(QuadError::BadInterval)
    );
    assert_eq!(
        quadrature::integrate(|_| Ok(Real::ONE), r(0.0), r(1.0), r(-1.0), r(0.0), 0, 5).map(|_| ()),
        Err(QuadError::BadInterval)
    );
}

/// solidcore plan/upstream-issues/WR-19--smlocalsolve1d-without-an-interval-steps-against-newton.md
/// (b4de2149): a local solver left without an interval stepped against Newton by 1.6e19, and with
/// a failed first evaluation read an uninitialised flag. Ours takes its interval as an argument,
/// so the same shapes are an empty or reversed interval, an interval too wide to step in, and a
/// function that fails at its first evaluation.
#[test]
fn wr_19_a_search_without_a_usable_interval_refuses_or_finds_the_root() {
    let f = |x: Real| Real::new(x.get() - 0.25);
    // An empty and a reversed interval are refused.
    assert_eq!(
        brent::zero(f, r(1.0), r(1.0), r(0.0), 100).map(|z| z.x),
        Err(BrentError::BadInterval)
    );
    assert_eq!(
        brent::zero(f, r(2.0), r(-2.0), r(0.0), 100).map(|z| z.x),
        Err(BrentError::BadInterval)
    );
    assert_eq!(
        brent::minimum(
            |x| Real::new(x.get() * x.get()),
            r(1.0),
            r(-1.0),
            r(1e-9),
            100
        )
        .map(|m| m.x),
        Err(BrentError::BadInterval)
    );
    // An interval the width of the defect's step (1.58e19) still finds the root, and in range.
    let z = brent::zero(f, r(-1.58e19), r(1.58e19), r(0.0), 400).unwrap();
    assert!((z.x.get() - 0.25).abs() <= 1e-3 || z.fx.get() == 0.0);
    // The widest finite interval: the same, or a defined refusal; never a step outside it.
    let wide = brent::zero(f, r(-f64::MAX), r(f64::MAX), r(0.0), 4000);
    if let Ok(z) = wide {
        assert!(z.x.get().is_finite() && z.fx.get().is_finite());
    }
    // A function that fails at its first evaluation: an error, not a stale answer.
    assert_eq!(
        brent::zero(|_| Err(NumError::NotFinite), r(0.0), r(1.0), r(0.0), 100).map(|z| z.x),
        Err(BrentError::Num(NumError::NotFinite))
    );
    // A bracket whose ends have the same sign is not searched.
    assert_eq!(
        brent::zero(
            |x| Real::new(x.get() * x.get() + 1.0),
            r(-1.0),
            r(1.0),
            r(0.0),
            100
        )
        .map(|z| z.x),
        Err(BrentError::SameSign)
    );
}

/// solidcore plan/upstream-issues/HANG-23--polar-set-up-of-a-circle-with-a-far-step-interval-shifts-one.md
/// (b4de2149): moving an angle to the period of greatest overlap took one turn per iteration, so
/// an angle of 9223372036854775807 degrees never finished. Ours reduces by whole turns exactly
/// and at once: the sine and cosine of half-turns far from zero are exact and quick, and for a
/// double of 2^53 or more, which is an even whole number of half-turns, they are 0 and 1.
#[test]
fn hang_23_a_far_angle_reduces_at_once_and_exactly() {
    for x in [
        9_223_372_036_854_775_807.0,
        9_007_199_254_740_992.0,
        2f64.powi(61),
        -(2f64.powi(61)),
        1e100,
        -1e300,
        f64::MAX,
        f64::MIN,
    ] {
        assert_eq!(math::sinpi(x).unwrap().get(), 0.0, "sinpi({x:e})");
        assert_eq!(math::cospi(x).unwrap().get(), 1.0, "cospi({x:e})");
    }
    // Just below 2^53 the doubles are whole numbers still, odd and even alike: exact signs.
    let odd = 9_007_199_254_740_991.0;
    assert_eq!(math::sinpi(odd).unwrap().get().abs(), 0.0);
    assert_eq!(math::cospi(odd).unwrap().get(), -1.0);
    assert_eq!(math::cospi(odd - 1.0).unwrap().get(), 1.0);
    // Half-integers as far out as they exist (below 2^52 the spacing is a half): cosine 0, sine
    // alternating with the parity of the whole part.
    let half = 2_251_799_813_685_248.5;
    assert_eq!(math::cospi(half).unwrap().get(), 0.0);
    assert_eq!(math::sinpi(half).unwrap().get(), 1.0);
    assert_eq!(math::sinpi(half + 1.0).unwrap().get(), -1.0);
    // Not a number is refused, not looped on.
    assert_eq!(
        math::sinpi(f64::NAN).map(|v| v.get()),
        Err(NumError::NotFinite)
    );
    assert_eq!(
        math::cospi(f64::INFINITY).map(|v| v.get()),
        Err(NumError::NotFinite)
    );
}

/// solidcore plan/upstream-issues/UB-35--mappointtovoxel-converts-negative-doubles-to-ulong.md and
/// UB-194--sm-histogram-edgelen-index-converts-negative-doubles-to.md (b4de2149): a negative
/// number turned into an unsigned one wrapped on one machine and saturated on another. Ours has
/// no such conversion: a coordinate is a signed count of steps, and a coarser step takes the
/// nearest step for a negative coordinate as for a positive one, the same on every machine. The
/// exact oracle is Euclidean division.
#[test]
fn ub_35_and_ub_194_negative_coordinates_convert_symmetrically() {
    let pairs = [
        (GridStep::Q1, GridStep::Q0, 25i64),
        (GridStep::Q2, GridStep::Q1, 25),
        (GridStep::Q2, GridStep::Q0, 625),
    ];
    for (fine, coarse, d) in pairs {
        let mut probes: Vec<i64> = (-3 * d..=3 * d).collect();
        probes.extend([
            COORD_MAX,
            COORD_MAX - 1,
            COORD_MAX - 12,
            -COORD_MAX,
            -COORD_MAX + 1,
        ]);
        for v in probes {
            let c = Coord::new(v).unwrap();
            let got = coarsen(c, fine, coarse).unwrap();
            // The nearest multiple, ties impossible (d is odd).
            let q = v.div_euclid(d);
            let rem = v.rem_euclid(d);
            let want = if rem > d / 2 { q + 1 } else { q };
            assert_eq!(got.value().get(), want, "{v} by {d}");
            assert_eq!(got.is_exact(), rem == 0, "{v} by {d}");
            // Symmetric about zero: the negative coordinate is the negative of the positive.
            let neg = coarsen(Coord::new(-v).unwrap(), fine, coarse).unwrap();
            assert_eq!(neg.value().get(), -got.value().get(), "{v} by {d}");
            assert_eq!(neg.is_exact(), got.is_exact());
            // The nearest coarse step is never farther than half a step away.
            assert!((got.value().get() * d - v).abs() * 2 <= d);
        }
        // A negative coordinate refines exactly, and out of the frame is refused, not wrapped.
        let c = Coord::new(-7).unwrap();
        assert_eq!(refine(c, coarse, fine).unwrap().get(), -7 * d);
        assert!(matches!(
            refine(Coord::MIN, coarse, fine),
            Err(NumError::OutOfRange { value }) if value == -(i128::from(COORD_MAX) * i128::from(d))
        ));
    }
    // The wrong direction is refused in both.
    assert_eq!(
        coarsen(Coord::ZERO, GridStep::Q0, GridStep::Q1).map(|v| v.value()),
        Err(NumError::WrongDirection)
    );
    assert_eq!(
        refine(Coord::ZERO, GridStep::Q1, GridStep::Q0),
        Err(NumError::WrongDirection)
    );
}
