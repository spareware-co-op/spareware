//! Attacks on piece G51's root finding and quadrature at the edges the coverage and mutation gates
//! of the foundation set (piece G21) look at.
//!
//! What this file proves: every error reads as documented; every refusal of `zero`, `minimum`,
//! `gauss_legendre` and `integrate` is the one its documentation names, each condition on its own;
//! a panel too narrow to split is refused whether its midpoint rounds down to its start or up to
//! its end; the integrator stops at its panel limit as at its bisection cap; `minimum` still
//! converges when the parabola's arithmetic overflows (its q is refused by name), when the vertex
//! falls left or right of the interval, and on a function of plateaus whose values tie; and the
//! result of each stays inside its interval and below every value it compares against.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::many_single_char_names,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_num::NumError;
use geom_num::brent::{BrentError, Minimum, minimum, zero};
use geom_num::quadrature::{MAX_PANELS, QuadError, gauss_legendre, integrate};
use geom_num::real::Real;

fn r(x: f64) -> Real {
    Real::new(x).unwrap()
}

fn ok(x: f64) -> Result<Real, NumError> {
    Real::new(x)
}

#[test]
fn errors_read_as_documented() {
    assert_eq!(
        BrentError::SameSign.to_string(),
        "f has the same sign at both ends of the interval"
    );
    assert_eq!(
        BrentError::BadInterval.to_string(),
        "the interval is empty or reversed, or the tolerance is negative"
    );
    assert_eq!(
        BrentError::CapReached.to_string(),
        "the evaluation cap was reached"
    );
    assert_eq!(
        BrentError::Num(NumError::NotFinite).to_string(),
        NumError::NotFinite.to_string()
    );
    assert_eq!(
        QuadError::BadInterval.to_string(),
        "the interval is empty or reversed, or a tolerance is negative"
    );
    assert_eq!(
        QuadError::NoSuchRule.to_string(),
        "no quadrature rule of that size"
    );
    assert_eq!(
        QuadError::CapReached.to_string(),
        "the tolerance was not met within the cap"
    );
    assert_eq!(
        QuadError::TooNarrow.to_string(),
        "a panel became too narrow to split"
    );
    assert_eq!(
        QuadError::Num(NumError::Degenerate).to_string(),
        NumError::Degenerate.to_string()
    );
    assert_eq!(
        BrentError::from(NumError::NotFinite),
        BrentError::Num(NumError::NotFinite)
    );
    assert_eq!(
        QuadError::from(NumError::NotFinite),
        QuadError::Num(NumError::NotFinite)
    );
}

#[test]
fn every_refusal_on_its_own() {
    let line = |x: Real| ok(x.get() - 0.5);
    let bowl = |x: Real| ok((x.get() - 0.5) * (x.get() - 0.5));
    assert_eq!(
        zero(line, r(1.0), r(0.0), r(0.0), 100),
        Err(BrentError::BadInterval)
    );
    assert_eq!(
        zero(line, r(0.0), r(1.0), r(-1e-9), 100),
        Err(BrentError::BadInterval)
    );
    assert_eq!(
        minimum(bowl, r(1.0), r(1.0), r(0.0), 100),
        Err(BrentError::BadInterval)
    );
    assert_eq!(
        minimum(bowl, r(0.0), r(1.0), r(-1e-9), 100),
        Err(BrentError::BadInterval)
    );
    assert!(minimum(bowl, r(0.0), r(1.0), r(0.0), 100).is_ok());
    let one = |_: Real| ok(1.0);
    assert_eq!(
        gauss_legendre(one, r(1.0), r(1.0), 5),
        Err(QuadError::BadInterval)
    );
    assert_eq!(
        gauss_legendre(one, r(0.0), r(1.0), 0),
        Err(QuadError::NoSuchRule)
    );
    assert_eq!(
        gauss_legendre(one, r(0.0), r(1.0), 21),
        Err(QuadError::NoSuchRule)
    );
    assert_eq!(gauss_legendre(one, r(0.0), r(2.0), 20), Ok(r(2.0)));
    let i = |a: f64, b: f64, tol: f64, rel: f64, rule: usize| {
        integrate(one, r(a), r(b), r(tol), r(rel), rule, 10)
    };
    assert_eq!(i(1.0, 1.0, 0.0, 0.0, 0), Err(QuadError::BadInterval));
    assert_eq!(i(0.0, 1.0, -1e-9, 0.0, 0), Err(QuadError::BadInterval));
    assert_eq!(i(0.0, 1.0, 0.0, -1e-9, 0), Err(QuadError::BadInterval));
    assert_eq!(i(0.0, 1.0, 0.0, 0.0, 6), Err(QuadError::NoSuchRule));
    let whole = i(0.0, 1.0, 0.0, 0.0, 5).unwrap();
    assert_eq!(
        (whole.value, whole.error, whole.panels, whole.evaluations),
        (r(1.0), r(0.0), 1, 61)
    );
}

/// A step at `at`: 0 below, 1 from it on. Gauss and Kronrod never agree across it.
fn step(at: f64) -> impl FnMut(Real) -> Result<Real, NumError> {
    move |x: Real| ok(if x.get() >= at { 1.0 } else { 0.0 })
}

#[test]
fn a_panel_too_narrow_to_split_is_refused_either_way_its_midpoint_rounds() {
    let eps = f64::EPSILON;
    // [1, 1 + ε]: the midpoint 1 + ε/2 rounds to the even 1, the start. Below 1 the doubles are
    // twice as dense, so the nodes reach 1 − ε/2 and see the step there.
    let down = integrate(
        |x: Real| ok(if x.get() < 1.0 { 1.0 } else { 0.0 }),
        r(1.0),
        r(1.0 + eps),
        r(0.0),
        r(0.0),
        0,
        10,
    );
    assert_eq!(down, Err(QuadError::TooNarrow));
    // [−1 − ε, −1]: the midpoint −1 − ε/2 rounds to the even −1, the end. Above −1 the doubles
    // are twice as dense, so the nodes reach −1 + ε/2 and see the step there.
    let up = integrate(
        |x: Real| ok(if x.get() > -1.0 { 1.0 } else { 0.0 }),
        r(-1.0 - eps),
        r(-1.0),
        r(0.0),
        r(0.0),
        0,
        10,
    );
    assert_eq!(up, Err(QuadError::TooNarrow));
}

#[test]
fn the_integrator_stops_at_its_panel_limit_as_at_its_cap() {
    // Three kinks never meet a zero tolerance: each panel holding one keeps an error of the order
    // of its width squared. The 127 bisections the panels allow are shared among the three, so no
    // panel comes near the spacing of the doubles (each kink gets about 42 halvings, the doubles
    // there allow 54): the panels run out first with a generous cap, and a small cap first
    // otherwise.
    let kink = |x: Real| {
        let v = x.get();
        ok((v - 0.123_456_7).abs() + (v - 0.456_789_1).abs() + (v - 0.789_123_4).abs())
    };
    let many = integrate(kink, r(0.0), r(1.0), r(0.0), r(0.0), 0, 1000);
    assert_eq!(many, Err(QuadError::CapReached));
    let few = integrate(kink, r(0.0), r(1.0), r(0.0), r(0.0), 0, 3);
    assert_eq!(few, Err(QuadError::CapReached));
    let at = 2.0f64.sqrt() - 1.0;
    // With a loose enough tolerance the same step is answered, in fewer than MAX_PANELS panels.
    let loose = integrate(step(at), r(0.0), r(1.0), r(1e-3), r(0.0), 0, 1000).unwrap();
    assert!(loose.panels < u32::try_from(MAX_PANELS).unwrap());
    assert!((loose.value.get() - (1.0 - at)).abs() <= 2e-3);
}

/// Every point `minimum` evaluates, with its value, and the minimum it returns.
fn trace(
    f: impl Fn(f64) -> f64,
    a: f64,
    b: f64,
    tol: f64,
) -> (Vec<(f64, f64)>, Result<Minimum, BrentError>) {
    let mut seen = Vec::new();
    let m = minimum(
        |x: Real| {
            let v = f(x.get());
            seen.push((x.get(), v));
            ok(v)
        },
        r(a),
        r(b),
        r(tol),
        10_000,
    );
    (seen, m)
}

/// The minimum lies inside the interval, is the least value evaluated, and every evaluation
/// stays strictly inside (a, b).
fn sound(seen: &[(f64, f64)], m: &Minimum, a: f64, b: f64) {
    for &(x, v) in seen {
        assert!(a < x && x < b, "evaluated at {x} outside ({a}, {b})");
        assert!(m.fx.get() <= v, "returned {} above {v} at {x}", m.fx.get());
    }
    assert_eq!(seen.len(), m.evaluations as usize);
}

#[test]
fn minimum_survives_an_overflowing_parabola() {
    // Values up to 1.7e308 over an interval of width 4: the parabola's products (x − v)(fx − fw)
    // overflow, so q is infinite and the step is a golden section instead.
    let f = |x: f64| 1.7e308 * ((x - 1.0) / 2.0) * ((x - 1.0) / 2.0);
    let (seen, m) = trace(f, -1.0, 3.0, 1e-9);
    let m = m.unwrap();
    sound(&seen, &m, -1.0, 3.0);
    assert!((m.x.get() - 1.0).abs() <= 1e-6, "{}", m.x.get());
}

#[test]
fn minimum_refuses_vertices_left_and_right_of_the_interval() {
    // Monotone functions: the least value is at an end, and the parabola through three points of a
    // cubic puts its vertex beyond the interval on that side, so the vertex is refused and a
    // golden section taken.
    for (f, a, b, end) in [
        (
            Box::new(|x: f64| x * x * x) as Box<dyn Fn(f64) -> f64>,
            0.0,
            1.0,
            0.0,
        ),
        (Box::new(|x: f64| -(x * x * x)), 0.0, 1.0, 1.0),
        (Box::new(|x: f64| (x - 0.5).abs()), 0.0, 2.0, 0.5),
        // A parabola whose vertex, 0, lies left of the interval, and one whose vertex lies right
        // of it: the fitted vertex is the true one, outside, and is refused.
        (Box::new(|x: f64| x * x), 0.1, 1.0, 0.1),
        (Box::new(|x: f64| x * x), -1.0, -0.1, -0.1),
    ] {
        let (seen, m) = trace(f, a, b, 1e-10);
        let m = m.unwrap();
        sound(&seen, &m, a, b);
        assert!((m.x.get() - end).abs() <= 1e-4, "{} for {end}", m.x.get());
    }
    // Concave: both ends are least, at −1/4; the search ends near one of them.
    let (seen, m) = trace(|x: f64| -((x - 0.5) * (x - 0.5)), 0.0, 1.0, 1e-10);
    let m = m.unwrap();
    sound(&seen, &m, 0.0, 1.0);
    assert!(m.fx.get() <= -0.2499, "{}", m.fx.get());
}

#[test]
fn minimum_on_plateaus_whose_values_tie() {
    // A staircase: values repeat, so new points tie with the best, the second best and the one
    // before it, and the bookkeeping of x, w and v meets every equality.
    for steps in [2.0, 3.0, 8.0, 64.0] {
        let f = |x: f64| ((x - 0.37).abs() * steps).floor();
        let (seen, m) = trace(f, 0.0, 1.0, 1e-8);
        let m = m.unwrap();
        sound(&seen, &m, 0.0, 1.0);
        assert_eq!(m.fx.get(), 0.0);
    }
    // A constant: every value ties.
    let (seen, m) = trace(|_| 5.0, -2.0, 3.0, 1e-6);
    let m = m.unwrap();
    sound(&seen, &m, -2.0, 3.0);
    assert_eq!(m.fx.get(), 5.0);
}
