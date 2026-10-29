//! Attacks on piece G51's root finding and quadrature at the edges the coverage and mutation gates
//! of the foundation set (piece G21) look at.
//!
//! What this file proves: every error reads as documented; every refusal of `zero`, `minimum`,
//! `gauss_legendre` and `integrate` is the one its documentation names, each condition on its own;
//! a panel too narrow to split is refused whether its midpoint rounds down to its start or up to
//! its end; the integrator stops at its panel limit as at its bisection cap; `minimum` still
//! converges when the parabola's arithmetic overflows (its q is refused by name), when the vertex
//! falls left or right of the interval, and on a function of plateaus whose values tie; the
//! result of each stays inside its interval and below every value it compares against; and, with
//! functions scripted call by call (an `FnMut` may answer by call, not by point), both searches
//! take exactly the steps the model of the method predicts through exact ties of their tests.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::unreadable_literal,
    reason = "oracles and generators, and doubles written as their bits; the rules are for kernel code"
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
    // A well 1e308 deep and about 3 wide: the best three points differ in value by most of that
    // over distances of order 1, so the parabola's products (x − v)(fx − fw) overflow, q is
    // infinite, and the step is a golden section instead. (The inputs were found by searching with
    // a model of the method that repeats its double arithmetic, as the pull request describes.)
    let (c, s) = (0.491_606_779_558_807_7, 3.044_611_860_264_698_8);
    let well = |x: f64| {
        let t = (x - c) / s;
        -1e308 * geom_num::math::exp(-(t * t)).unwrap().get()
    };
    for tol in [0.0, 1e-9] {
        let (seen, m) = trace(well, -4.630_307_883_315_865, 5.259_378_900_455_251, tol);
        let m = m.unwrap();
        sound(&seen, &m, -4.630_307_883_315_865, 5.259_378_900_455_251);
        assert!((m.x.get() - c).abs() <= 1e-6, "{}", m.x.get());
        assert_eq!(m.fx.get(), -1e308);
    }
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
        // A cubic and its mirror image, on intervals where the fitted parabola's vertex lands right
        // of b, and left of a (found with the same model).
        (
            Box::new(|x: f64| {
                let t = x + 0.573_270_789_663_270_7;
                t * t * t
            }),
            -0.937_044_652_441_112,
            2.468_053_206_564_516,
            -0.937_044_652_441_112,
        ),
        (
            Box::new(|x: f64| {
                let t = x - 0.573_270_789_663_270_7;
                -(t * t * t)
            }),
            -2.468_053_206_564_516,
            0.937_044_652_441_112,
            0.937_044_652_441_112,
        ),
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

/// Which search a scripted case runs.
enum Search {
    Minimum,
    Zero,
}

/// What a scripted function answers once its scripted values run out, at point t on call k.
enum Tail {
    /// |t − c|.
    Abs(f64),
    /// (t − c)².
    Square(f64),
    /// c + k.
    Count(f64),
    /// t − c.
    Line(f64),
    /// c + |t|.
    Plus(f64),
}

/// A function scripted call by call, and what the search does with it: how many evaluations, at
/// which points (the first 64 listed), and its outcome (`x` and `fx`, and `other` for
/// a zero), or `None` for the cap of 400 evaluations.
struct Scripted {
    search: Search,
    a: f64,
    b: f64,
    tol: f64,
    values: &'static [f64],
    tail: Tail,
    evaluations: usize,
    points: &'static [u64],
    outcome: Option<&'static [f64]>,
}

/// Cases found by searching with a model of both methods that repeats their double arithmetic
/// (as the pull request describes), each steering a test into an exact tie or an exact value: a
/// parabola step equal to half the step before last; a vertex exactly at either end; a vertex
/// exactly the guard 2·tol1 from either end; a step before last exactly tol1; a step of exactly
/// zero from the point 0 with no tolerance, after which f answers the same point higher
/// (`u == x`); an interpolation of exactly zero; interpolations exactly on the bracket bound and
/// exactly half the step before last; and interpolations accepted just inside the acceptance
/// bound. A change to any of those tests moves a point.
const SCRIPTED: [Scripted; 13] = [
    // vertex_step_ties_half_the_step_before_last (mutant 317 of the pull request's model)
    Scripted {
        search: Search::Minimum,
        a: f64::from_bits(0xbff0000000000000),
        b: f64::from_bits(0x3ff0000000000000),
        tol: f64::from_bits(0x3f50624dd2f1a9fc),
        values: &[
            f64::from_bits(0x0000000000000000),
            f64::from_bits(0x4000000000000000),
            f64::from_bits(0x4000000000000000),
            f64::from_bits(0x0000000000000000),
            f64::from_bits(0x4008000000000000),
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x4000000000000000),
            f64::from_bits(0x4008000000000000),
        ],
        tail: Tail::Abs(f64::from_bits(0xbfebb9fccaf16004)),
        evaluations: 8,
        points: &[
            0xbfce3779b97f4a7c,
            0x3fce3779b97f4a7e,
            0xbfe0e44323405ac2,
            0xbfc2acc969c11045,
            0xbfc8722191a02d61,
            0x3c90000000000000,
            0xbfc2a1dd3291f81d,
            0xbfc2b7b5a0f0286d,
        ],
        outcome: Some(&[
            f64::from_bits(0xbfc2acc969c11045),
            f64::from_bits(0x0000000000000000),
        ]),
    },
    // vertex_exactly_at_the_left_end (mutant 318a,318b of the pull request's model)
    Scripted {
        search: Search::Minimum,
        a: f64::from_bits(0xc000000000000000),
        b: f64::from_bits(0xbff8000000000000),
        tol: f64::from_bits(0x0000000000000000),
        values: &[
            f64::from_bits(0x4018000000000000),
            f64::from_bits(0x0000000000000000),
            f64::from_bits(0x0000000000000000),
            f64::from_bits(0x4018000000000000),
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x3ff0000000000000),
        ],
        tail: Tail::Square(f64::from_bits(0xbffc000000000000)),
        evaluations: 35,
        points: &[
            0xbffcf1bbcdcbfa54,
            0xbffb0e44323405ac,
            0xbff9e3779b97f4a8,
            0xbffa78dde6e5fd2a,
            0xbff92acc969c1105,
            0xbff99cee283c3e66,
            0xbffa1c8864680b58,
            0xbff9c8864680b584,
            0xbff9f943aaba3037,
            0xbff9d92d0faded7f,
            0xbff9ebcb002e90a5,
            0xbff9df894cf11e51,
            0xbff9e6a5ba398d11,
            0xbff9e1f73b8d78cb,
            0xbff9e4ae92e621e5,
            0xbff9e2e4ca1f5767,
            0xbff9e3ee62e0e3f6,
            0xbff9e33f873898c3,
            0xbff9e3a4fa249556,
            0xbff9e3622ff27e3b,
            0xbff9e388eff4e764,
            0xbff9e36f6d06ed45,
            0xbff9e37e3a222c2d,
            0xbff9e3747b8a54ed,
            0xbff9e37a22d9a87c,
            0xbff9e3766a001cda,
            0xbff9e37892d2d89e,
            0xbff9e37726de0cf9,
            0xbff9e377fa06ecb7,
            0xbff9e3776f021568,
            0xbff9e377bfa9f8e0,
            0xbff9e3778a903e98,
            0xbff9e377a95f0940,
            0xbff9e3779516b1b7,
            0xbff9e377a0db2e38,
        ],
        outcome: Some(&[
            f64::from_bits(0xbff9e3779b97f4a8),
            f64::from_bits(0x0000000000000000),
        ]),
    },
    // vertex_exactly_at_the_right_end (mutant 319 of the pull request's model)
    Scripted {
        search: Search::Minimum,
        a: f64::from_bits(0xbfe0000000000000),
        b: f64::from_bits(0x3fe0000000000000),
        tol: f64::from_bits(0x3f50624dd2f1a9fc),
        values: &[
            f64::from_bits(0x0000000000000000),
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x4008000000000000),
        ],
        tail: Tail::Abs(f64::from_bits(0xbfd0000000000000)),
        evaluations: 15,
        points: &[
            0xbfbe3779b97f4a7c,
            0x3fbe3779b97f4a7e,
            0xbfd0e44323405ac2,
            0xbfb2acc969c11045,
            0xbfc63dd5f6c27b4e,
            0xbfc13c637c935e6f,
            0xbfb9cee283c3e655,
            0xbfbc8864680b583e,
            0xbfbfd78d3e51e5c1,
            0xbfbd92d0faded7ea,
            0xbfbed667084fa9e8,
            0xbfbdf894cf11e504,
            0xbfbe742e211dcd7a,
            0xbfbe1f73b8d78ca6,
            0xbfbe4ea9a18a740b,
        ],
        outcome: Some(&[
            f64::from_bits(0xbfbe3779b97f4a7c),
            f64::from_bits(0x0000000000000000),
        ]),
    },
    // a_step_of_zero_at_zero (mutant 358 of the pull request's model)
    Scripted {
        search: Search::Minimum,
        a: f64::from_bits(0xbfd8722191a02d61),
        b: f64::from_bits(0x3fe3c6ef372fe950),
        tol: f64::from_bits(0x0000000000000000),
        values: &[
            f64::from_bits(0x0000000000000000),
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x3fd8722191a02d60),
            f64::from_bits(0x4014000000000000),
        ],
        tail: Tail::Count(f64::from_bits(0x401c000000000000)),
        evaluations: 400,
        points: &[
            0x0000000000000000,
            0x3fce3779b97f4a7d,
            0xbfc2acc969c11046,
            0x0000000000000000,
            0xbfac8864680b583f,
            0xbf95cc0f223b8f49,
            0xbf80a6c92d37faba,
            0xbf6970f50cc34674,
            0xbf536f74e2d3d0ea,
            0xbf3db1d23bd3975c,
            0xbf26af46505704d9,
            0xbf11545cf2bbbfa5,
            0xbefa7a28de14d4f0,
            0xbee43a0d9e9f0077,
            0xbecee75c7ecd5e11,
            0xbeb79bbe03d830b0,
            0xbea209021bdbabda,
            0xbe8b8e292f8a88bd,
            0xbe750ce63e61d59d,
            0xbe6014c0b820deb7,
            0xbe4891d6ae7bc3b1,
            0xbe32c50255dfa06d,
            0xbe1cad66929d6795,
            0xbe05e854c0b36a17,
            0xbdf0bc6239bede36,
            0xbdd991f4af5719d3,
            0xbdc388aa4e13a941,
            0xbdadd8562e332071,
            0xbd96ccb2a49578a0,
            0xbd816ad722b451fc,
            0xbd6a9c807bc812ce,
            0xbd545449d10de0f4,
            0xbd3f0f71ee65f4fe,
            0xbd27ba5d0df4b656,
            0xbd1220669a24720d,
            0xbcfbb1e6ca0fa5eb,
            0xbce52833eb3a531b,
            0xbcd0299c5b1f5af7,
            0xbcb8b1b4eba5aab8,
            0xbca2dd5aace72896,
            0xbc8cd298bda8914a,
            0xbc7604bf0aba230a,
            0xbc60d21749ba8d12,
            0xbc49b31f1eed8483,
            0xbc33a2006bbcb281,
            0xbc1dff0c156c39f2,
            0xbc06ea4522a3c7a3,
            0xbbf1816e7a25c608,
            0xbbdabf04a4a70b47,
            0xbbc46ea80abc1369,
            0xbbaf37bb5b988eb0,
            0xbb97d923cfb2bcda,
            0xbb8237e96fce325e,
            0xbb6bd5d2c014759c,
            0xbb5543a502092eba,
            0xbb403e930be541ed,
            0xbb28d1bc7e75a14f,
            0xbb12f5d29797b871,
            0xbafcf7fb276f9ad3,
            0xbae6214e2fdfdd68,
            0xbad0e7e88180c523,
            0xbab9d474930a5268,
            0xbaa3bb776637c558,
            0xba8e25f4324aace4,
        ],
        outcome: None,
    },
    // a_vertex_exactly_the_guard_from_the_left_end (mutant 324a of the pull request's model)
    Scripted {
        search: Search::Minimum,
        a: f64::from_bits(0xbfe0000000000000),
        b: f64::from_bits(0x3ff8000000000000),
        tol: f64::from_bits(0x3fd0000000000000),
        values: &[
            f64::from_bits(0x40052163a178a433),
            f64::from_bits(0x400410a677e6c977),
            f64::from_bits(0x3fe69f5e9e971790),
        ],
        tail: Tail::Abs(f64::from_bits(0x3fe8c21187a577e2)),
        evaluations: 5,
        points: &[
            0x3fd0e44323405ac2,
            0x3fe78dde6e5fd2a0,
            0x3ff0722191a02d61,
            0x3ff354cda58fbbee,
            0x3ff1ff784ccf9153,
        ],
        outcome: Some(&[
            f64::from_bits(0x3ff1ff784ccf9153),
            f64::from_bits(0x3fd679be23f35588),
        ]),
    },
    // a_vertex_exactly_the_guard_from_the_right_end (mutant 324b of the pull request's model)
    Scripted {
        search: Search::Minimum,
        a: f64::from_bits(0xc000000000000000),
        b: f64::from_bits(0x3ff0000000000000),
        tol: f64::from_bits(0x3fd0000000000000),
        values: &[
            f64::from_bits(0x4006126790aff1f1),
            f64::from_bits(0x4004f0fdff9f8d4a),
            f64::from_bits(0x3fcc682d637b27d8),
            f64::from_bits(0x3fd76d8a200eb88c),
        ],
        tail: Tail::Abs(f64::from_bits(0xbff24adb4e77eace)),
        evaluations: 7,
        points: &[
            0xbfeb54cda58fbbee,
            0xbfc2acc969c11044,
            0x3fd2acc969c11046,
            0x3fe1fe68f0af33cc,
            0x3fd9522730197981,
            0x3fbfe68f0af33cbc,
            0x3fcaaee8223d3275,
        ],
        outcome: Some(&[
            f64::from_bits(0x3fd2acc969c11046),
            f64::from_bits(0x3fcc682d637b27d8),
        ]),
    },
    // a_step_before_last_exactly_the_tolerance (mutant 298 of the pull request's model)
    Scripted {
        search: Search::Minimum,
        a: f64::from_bits(0xbff8722191a02d61),
        b: f64::from_bits(0x4003c6ef372fe950),
        tol: f64::from_bits(0x3fb8000000000000),
        values: &[
            f64::from_bits(0x0000000000000000),
            f64::from_bits(0x4002f9f80ea10aeb),
            f64::from_bits(0x4010000000000000),
            f64::from_bits(0x4008000000000000),
            f64::from_bits(0x3fef3ae9b52aa091),
            f64::from_bits(0x4002dacf1c73f6a4),
            f64::from_bits(0x3fcac33e4ad91059),
            f64::from_bits(0x3fee0a382ad6d831),
            f64::from_bits(0x3fe006ab93d1f3ae),
        ],
        tail: Tail::Plus(f64::from_bits(0x4024000000000000)),
        evaluations: 8,
        points: &[
            0x0000000000000000,
            0x3fee3779b97f4a7d,
            0xbfe2acc969c11046,
            0x3fd11a8d8df88e57,
            0xbfcc8864680b583f,
            0x3fa0000000000000,
            0xbfbabf72721c0011,
            0xbfa46efbec72eb39,
        ],
        outcome: Some(&[
            f64::from_bits(0x0000000000000000),
            f64::from_bits(0x0000000000000000),
        ]),
    },
    // an_interpolation_of_zero (mutant 225 of the pull request's model)
    Scripted {
        search: Search::Zero,
        a: f64::from_bits(0xc000000000000000),
        b: f64::from_bits(0x3ff0000000000000),
        tol: f64::from_bits(0x0000000000000000),
        values: &[
            f64::from_bits(0x4020000000000000),
            f64::from_bits(0xc010000000000000),
            f64::from_bits(0xbfc0000000000000),
            f64::from_bits(0x4020000000000000),
            f64::from_bits(0x4020000000000000),
            f64::from_bits(0x4020000000000000),
        ],
        tail: Tail::Line(f64::from_bits(0xbff38a2b0aa04b26)),
        evaluations: 315,
        points: &[
            0xc000000000000000,
            0x3ff0000000000000,
            0x0000000000000000,
            0xbfa04314b56c5f08,
            0xbf400308931fdf8a,
            0xbedf87f160bccbe4,
            0xbe7f0bc2575f4ea9,
            0xbe470fb4961fc2e8,
            0xbe11215a8b9cded2,
            0xbdd973008fdf9984,
            0xbda2e76eee2b3d45,
            0xbd6c15986b73ffb2,
            0xbd34dc87ca756874,
            0xbcfefe09ff85338d,
            0xbcc70583709180a1,
            0xbc9119c86d232233,
            0xbc5967c13cdd6bc0,
            0xbc22df1420e9e017,
            0xbbec092ef4f86f65,
            0xbbb4d34f8718fa1c,
            0xbb7ef05786b7566f,
            0xbb46fb56c8bc8424,
            0xbb111239a6fc203f,
            0xbad95c86e2599477,
            0xbaa2d6bd04e7df86,
            0xba6bfccafac6c961,
            0xba34ca1b56dc9980,
            0xb9fee2ab1b9b899b,
            0xb9c6f12ea00a240d,
            0xb9910aae37e95c6f,
            0xb95951517e29f77a,
            0xb922ce6998840c5d,
            0xb8ebf06c7a72809e,
            0xb8b4c0eb37f344dd,
            0xb87ed504bb84e8be,
            0xb846e70af47da0fa,
            0xb81103261e70ef28,
            0xb7d946210e1d27d1,
            0xb7a2c619da1d5db9,
            0xb76be41371900632,
            0xb734b7bf2890c4cd,
            0xb6fec76463c7be0d,
            0xb6c6dcebc41b1c60,
            0xb690fba1591997d3,
            0xb6593af59002b0a6,
            0xb622bdcdc8138308,
            0xb5ebd7bfddb4dd07,
            0xb5b4ae9726e9ad50,
            0xb57eb9ca11b981e6,
            0xb546d2d10ce79827,
            0xb510f41fe66abc91,
            0xb4d92fcf01ab14cf,
            0xb4a2b58560c6e3b3,
            0xb46bcb71bc77995b,
            0xb434a57331335d71,
            0xb3feac35c2b0da50,
            0xb3c6c8bacce8f64b,
            0xb390eca1c4ec69f7,
            0xb35924ad60e7ce6e,
            0xb322ad40a2989ed4,
            0xb2ebbf290b6fe050,
            0xb2b49c5345a3feed,
            0xb27e9ea774059a70,
            0xb246bea90225f877,
        ],
        outcome: Some(&[
            f64::from_bits(0x0000000000000000),
            f64::from_bits(0xbfc0000000000000),
            f64::from_bits(0x8000000000000001),
        ]),
    },
    // an_interpolation_within_tol_of_the_bisection_rule (mutant 236minus+ of the pull request's model)
    Scripted {
        search: Search::Zero,
        a: f64::from_bits(0x0000000000000000),
        b: f64::from_bits(0x3fe0000000000000),
        tol: f64::from_bits(0x3fd0000000000000),
        values: &[
            f64::from_bits(0x3ffc54536778fe38),
            f64::from_bits(0xbffb12b1e8d91d23),
            f64::from_bits(0xbfe9b543b0b95b30),
            f64::from_bits(0xbfd80b5047d629e0),
            f64::from_bits(0x4005960ea70bc07e),
            f64::from_bits(0x3fee15a9bd97b090),
            f64::from_bits(0x3ffdad19997c5f74),
            f64::from_bits(0x3fd40ee4f0fd8f00),
        ],
        tail: Tail::Line(f64::from_bits(0x3fd3ee35a21434b2)),
        evaluations: 4,
        points: &[
            0x0000000000000000,
            0x3fe0000000000000,
            0x3fd05ce2bdb5f464,
            0x3fc05ce2bdb5f464,
        ],
        outcome: Some(&[
            f64::from_bits(0x3fc05ce2bdb5f464),
            f64::from_bits(0xbfd80b5047d629e0),
            f64::from_bits(0x0000000000000000),
        ]),
    },
    // an_interpolation_with_integer_values (mutant 236minus/ of the pull request's model)
    Scripted {
        search: Search::Zero,
        a: f64::from_bits(0xc010000000000000),
        b: f64::from_bits(0x0000000000000000),
        tol: f64::from_bits(0x0000000000000000),
        values: &[
            f64::from_bits(0xc010000000000000),
            f64::from_bits(0x4008000000000000),
            f64::from_bits(0xc010000000000000),
        ],
        tail: Tail::Line(f64::from_bits(0xbfef782ad3e91dd8)),
        evaluations: 10,
        points: &[
            0xc010000000000000,
            0x0000000000000000,
            0xbffb6db6db6db6db,
            0xbfe7829cbc14e5e0,
            0xbfe982dd16a0edbc,
            0xbfef326f82694f60,
            0xbfef75a349a33ea5,
            0xbfef7829b43c2e1c,
            0xbfef782ad3e47886,
            0xbfef782ad3e91dd8,
        ],
        outcome: Some(&[
            f64::from_bits(0xbfef782ad3e91dd8),
            f64::from_bits(0x0000000000000000),
            f64::from_bits(0xbffb6db6db6db6db),
        ]),
    },
    // an_interpolation_with_dyadic_values (mutant 236mul/ of the pull request's model)
    Scripted {
        search: Search::Zero,
        a: f64::from_bits(0xc010000000000000),
        b: f64::from_bits(0x0000000000000000),
        tol: f64::from_bits(0x3fe0000000000000),
        values: &[
            f64::from_bits(0xc020000000000000),
            f64::from_bits(0x3fc0000000000000),
            f64::from_bits(0x4020000000000000),
            f64::from_bits(0xbfc0000000000000),
            f64::from_bits(0xc010000000000000),
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x3fe0000000000000),
        ],
        tail: Tail::Line(f64::from_bits(0xbff5ea9ceafe6a80)),
        evaluations: 8,
        points: &[
            0xc010000000000000,
            0x0000000000000000,
            0xbfd0000000000000,
            0xc001000000000000,
            0xbffdfffffffffffe,
            0xbff0ffffffffffff,
            0xbff5000000000000,
            0xbff9000000000001,
        ],
        outcome: Some(&[
            f64::from_bits(0xbff9000000000001),
            f64::from_bits(0xbfc8ab18a80cac08),
            f64::from_bits(0xbff5000000000000),
        ]),
    },
    // an_interpolation_exactly_on_the_bracket_bound (mutant 236a of the pull request's model)
    Scripted {
        search: Search::Zero,
        a: f64::from_bits(0x0000000000000000),
        b: f64::from_bits(0x3ff0000000000000),
        tol: f64::from_bits(0x3fd0000000000000),
        values: &[
            f64::from_bits(0x400708d7b334e89a),
            f64::from_bits(0xbfeec1789077d348),
            f64::from_bits(0xbfe5e5979a9db557),
        ],
        tail: Tail::Line(f64::from_bits(0x3fc1b4aafe58b7c4)),
        evaluations: 5,
        points: &[
            0x0000000000000000,
            0x3ff0000000000000,
            0x3fe7fdd95a692048,
            0x3fd7fdd95a692048,
            0x3fdffdd95a69204a,
        ],
        outcome: Some(&[
            f64::from_bits(0x3fdffdd95a69204a),
            f64::from_bits(0x3fd72383db3cc468),
            f64::from_bits(0x3fe7fdd95a692048),
        ]),
    },
    // an_interpolation_exactly_half_the_step_before_last (mutant 236b of the pull request's model)
    Scripted {
        search: Search::Zero,
        a: f64::from_bits(0x0000000000000000),
        b: f64::from_bits(0x4000000000000000),
        tol: f64::from_bits(0x0000000000000000),
        values: &[
            f64::from_bits(0x3ff095e22c480414),
            f64::from_bits(0xc0040549274f0472),
            f64::from_bits(0x4005d1a8ee67a0f0),
            f64::from_bits(0xc0108f87feedda4e),
        ],
        tail: Tail::Line(f64::from_bits(0x3ff7576ff7acad5a)),
        evaluations: 56,
        points: &[
            0x0000000000000000,
            0x4000000000000000,
            0x3fe2beba8df25b7e,
            0x3ff52c2bb3507dbf,
            0x3fee8b88fa49ab7e,
            0x3fecb42131313126,
            0x3fe7b96ddf91c652,
            0x3fe53c1436c210e8,
            0x3fe3fd67625a3633,
            0x3fe35e10f82648d8,
            0x3fe30e65c30c522b,
            0x3fe2e690287f56d4,
            0x3fe2d2a55b38d929,
            0x3fe2c8aff4959a54,
            0x3fe2c3b54143fae9,
            0x3fe2c137e79b2b34,
            0x3fe2bff93ac6c359,
            0x3fe2bf59e45c8f6c,
            0x3fe2bf0a39277575,
            0x3fe2bee2638ce87a,
            0x3fe2bece78bfa1fc,
            0x3fe2bec48358febd,
            0x3fe2bebf88a5ad1e,
            0x3fe2bebd0b4c044e,
            0x3fe2bebbcc9f2fe6,
            0x3fe2bebb2d48c5b2,
            0x3fe2bebadd9d9098,
            0x3fe2bebab5c7f60b,
            0x3fe2bebaa1dd28c4,
            0x3fe2beba97e7c221,
            0x3fe2beba92ed0ed0,
            0x3fe2beba906fb527,
            0x3fe2beba8f310852,
            0x3fe2beba8e91b1e8,
            0x3fe2beba8e4206b3,
            0x3fe2beba8e1a3118,
            0x3fe2beba8e06464b,
            0x3fe2beba8dfc50e4,
            0x3fe2beba8df75631,
            0x3fe2beba8df4d8d8,
            0x3fe2beba8df39a2b,
            0x3fe2beba8df2fad4,
            0x3fe2beba8df2ab29,
            0x3fe2beba8df28354,
            0x3fe2beba8df26f69,
            0x3fe2beba8df26574,
            0x3fe2beba8df26079,
            0x3fe2beba8df25dfc,
            0x3fe2beba8df25cbd,
            0x3fe2beba8df25c1e,
            0x3fe2beba8df25bce,
            0x3fe2beba8df25ba6,
            0x3fe2beba8df25b92,
            0x3fe2beba8df25b88,
            0x3fe2beba8df25b83,
            0x3fe2beba8df25b80,
        ],
        outcome: Some(&[
            f64::from_bits(0x3fe2beba8df25b80),
            f64::from_bits(0xbfebf0256166ff34),
            f64::from_bits(0x3fe2beba8df25b7e),
        ]),
    },
];

#[test]
fn scripted_values_steer_both_searches_through_exact_ties() {
    for (i, s) in SCRIPTED.iter().enumerate() {
        let mut seen = Vec::new();
        let mut k = 0usize;
        let f = |x: Real| {
            let t = x.get();
            seen.push(t);
            let v = if k < s.values.len() {
                s.values[k]
            } else {
                match s.tail {
                    Tail::Abs(c) => (t - c).abs(),
                    Tail::Square(c) => (t - c) * (t - c),
                    Tail::Count(c) => c + k as f64,
                    Tail::Line(c) => t - c,
                    Tail::Plus(c) => c + t.abs(),
                }
            };
            k += 1;
            ok(v)
        };
        let got: Result<Vec<f64>, BrentError> = match s.search {
            Search::Minimum => minimum(f, r(s.a), r(s.b), r(s.tol), 400).map(|m| {
                assert_eq!(m.evaluations as usize, s.evaluations, "case {i}");
                vec![m.x.get(), m.fx.get()]
            }),
            Search::Zero => zero(f, r(s.a), r(s.b), r(s.tol), 400).map(|z| {
                assert_eq!(z.evaluations as usize, s.evaluations, "case {i}");
                vec![z.x.get(), z.fx.get(), z.other.get()]
            }),
        };
        match s.outcome {
            Some(want) => {
                let got: Vec<u64> = got.unwrap().iter().map(|v| v.to_bits()).collect();
                let want: Vec<u64> = want.iter().map(|v| v.to_bits()).collect();
                assert_eq!(got, want, "case {i}");
            }
            None => assert_eq!(got, Err(BrentError::CapReached), "case {i}"),
        }
        assert_eq!(seen.len(), s.evaluations, "case {i}");
        let bits: Vec<u64> = seen.iter().take(64).map(|v| v.to_bits()).collect();
        assert_eq!(bits, s.points, "case {i}");
    }
}
