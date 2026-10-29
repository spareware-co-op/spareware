//! Tests of `geom_num::brent` (piece G51): Brent's zero and minimum.
//!
//! What this file proves: `zero` returns a point where the computed function is exactly zero, or
//! a bracket of its sign change no wider than 4u|x| + `tol`, for simple, triple and
//! steep zeros anywhere in a grid of roots and brackets; `minimum` lands within Brent's tolerance
//! of the minimum of smooth unimodal functions and never evaluates at or beyond the interval's
//! ends; both count their evaluations, stop at the cap with `CapReached` rather than a best guess,
//! and report an empty interval, a negative tolerance, a non-bracket and a failing function.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use core::cell::Cell;

use geom_num::NumError;
use geom_num::brent::{BrentError, minimum, zero};
use geom_num::real::Real;

const U: f64 = 1.0 / 9_007_199_254_740_992.0;

fn r(x: f64) -> Real {
    Real::new(x).unwrap()
}

/// Roots across scales and signs, some exactly representable, some not.
fn roots() -> Vec<f64> {
    let mut v = vec![
        0.0,
        1.0,
        -1.0,
        0.1,
        -0.3,
        1.0 / 3.0,
        2.0f64.sqrt(),
        1e-6,
        -7.25,
        1234.5678,
    ];
    v.extend((0..40).map(|i| -10.0 + 0.517 * f64::from(i)));
    v
}

#[test]
fn zero_keeps_a_bracket_of_the_computed_function() {
    type Case = fn(f64, f64) -> f64;
    let cases: [(&str, Case); 4] = [
        ("linear", |x, r| x - r),
        ("cubic", |x, r| (x - r) * (x * x + 1.0)),
        ("triple", |x, r| (x - r) * (x - r) * (x - r)),
        ("steep", |x, r| (1e8 * (x - r)).clamp(-1.0, 1.0)),
    ];
    for (name, f) in cases {
        for root in roots() {
            for (left, right) in [(3.7, 1.3), (0.01, 50.0), (1e-3, 1e-3)] {
                for tol in [0.0, 1e-6] {
                    let g = |x: Real| Real::new(f(x.get(), root));
                    let (a, b) = (root - left, root + right);
                    // Brent's bound: at most (⌈log₂((b − a)/δ)⌉ + 1)² evaluations, δ the final
                    // width; about 2,600 here at worst.
                    let z = zero(g, r(a), r(b), r(tol), 3000)
                        .unwrap_or_else(|e| panic!("{name} {root} [{a}, {b}] {tol}: {e}"));
                    let (x, other) = (z.x.get(), z.other.get());
                    let (fx, fo) = (f(x, root), f(other, root));
                    assert_eq!(fx, z.fx.get(), "{name}");
                    if fx != 0.0 {
                        assert!((fx > 0.0) != (fo > 0.0), "{name} {root}: {z:?}");
                        assert!(
                            (x - other).abs() <= 4.0 * U * x.abs() + tol,
                            "{name} {root}: {z:?}"
                        );
                    }
                    assert!(
                        a <= x && x <= b && a <= other && other <= b,
                        "{name} {root}"
                    );
                    assert!(z.evaluations <= 3000);
                }
            }
        }
    }
}

#[test]
fn zero_converges_fast_on_a_simple_root() {
    // x³ − 2 on [0, 2]: superlinear, far below bisection's 53 halvings.
    let z = zero(
        |x: Real| Real::new(x.get() * x.get() * x.get() - 2.0),
        r(0.0),
        r(2.0),
        r(0.0),
        100,
    )
    .unwrap();
    // ∛2 = 1.2599210498948731647672106…: the double nearest it, or a neighbour.
    assert!(
        (z.x.get() - 1.259_921_049_894_873_2).abs() <= 2.0 * U * 2.0,
        "{z:?}"
    );
    assert!(z.evaluations < 20, "{z:?}");
}

#[test]
fn minimum_finds_smooth_minima_inside_the_interval() {
    for m in [0.0, 0.25, 1.0 / 3.0, 0.5, 0.7, 0.999, -0.4] {
        for tol in [0.0, 1e-9] {
            let lo = Cell::new(f64::INFINITY);
            let hi = Cell::new(f64::NEG_INFINITY);
            let f = |x: Real| {
                let x = x.get();
                if x < lo.get() {
                    lo.set(x);
                }
                if x > hi.get() {
                    hi.set(x);
                }
                Real::new((x - m) * (x - m) + 1.0)
            };
            let (a, b) = (-0.5, 1.0);
            let found = minimum(f, r(a), r(b), r(tol), 200).unwrap();
            let x = found.x.get();
            // Brent's bound, with room for the flat bottom the rounding of f leaves (√u wide).
            assert!(
                (x - m).abs() <= 2.0 * (U.sqrt() * x.abs() + tol / 3.0) + 4.0 * U.sqrt(),
                "{m}: {found:?}"
            );
            assert!(
                a < lo.get() && hi.get() < b,
                "{m}: evaluated at {} and {}",
                lo.get(),
                hi.get()
            );
            assert!(found.evaluations <= 200);
        }
    }
    // A minimum at an end: the search closes in on it without touching it.
    let found = minimum(|x: Real| Real::new(x.get()), r(0.0), r(1.0), r(1e-12), 200).unwrap();
    assert!(found.x.get() > 0.0 && found.x.get() < 1e-7, "{found:?}");
}

#[test]
fn caps_are_errors_never_guesses() {
    let f = |x: Real| Real::new(x.get() - 0.3);
    assert_eq!(
        zero(f, r(0.0), r(1.0), r(0.0), 2),
        Err(BrentError::CapReached)
    );
    assert_eq!(
        zero(f, r(0.0), r(1.0), r(0.0), 1),
        Err(BrentError::CapReached)
    );
    let q = |x: Real| Real::new((x.get() - 0.3) * (x.get() - 0.3));
    assert_eq!(
        minimum(q, r(0.0), r(1.0), r(0.0), 3),
        Err(BrentError::CapReached)
    );
    // The evaluations made are exactly the cap's.
    let n = Cell::new(0u32);
    let counted = |x: Real| {
        n.set(n.get() + 1);
        let d = x.get() - 0.3;
        Real::new(d * d * d)
    };
    assert_eq!(
        zero(counted, r(0.0), r(1.0), r(0.0), 5),
        Err(BrentError::CapReached)
    );
    assert_eq!(n.get(), 5);
}

#[test]
fn bad_inputs_are_refused() {
    let f = |x: Real| Real::new(x.get() - 0.3);
    assert_eq!(
        zero(f, r(1.0), r(1.0), r(0.0), 50),
        Err(BrentError::BadInterval)
    );
    assert_eq!(
        zero(f, r(1.0), r(0.0), r(0.0), 50),
        Err(BrentError::BadInterval)
    );
    assert_eq!(
        zero(f, r(0.0), r(1.0), r(-1.0), 50),
        Err(BrentError::BadInterval)
    );
    assert_eq!(
        zero(f, r(0.5), r(1.0), r(0.0), 50),
        Err(BrentError::SameSign)
    );
    assert_eq!(
        minimum(f, r(1.0), r(0.0), r(0.0), 50),
        Err(BrentError::BadInterval)
    );
    let fails = |_: Real| -> Result<Real, NumError> { Err(NumError::OutsideDomain) };
    assert_eq!(
        zero(fails, r(0.0), r(1.0), r(0.0), 50),
        Err(BrentError::Num(NumError::OutsideDomain))
    );
    assert_eq!(
        minimum(fails, r(0.0), r(1.0), r(0.0), 50),
        Err(BrentError::Num(NumError::OutsideDomain))
    );
    // A zero at an end is that end.
    let z = zero(|x: Real| Real::new(x.get()), r(0.0), r(1.0), r(0.0), 50).unwrap();
    assert_eq!((z.x.get(), z.fx.get(), z.evaluations), (0.0, 0.0, 2));
    let z = zero(
        |x: Real| Real::new(x.get() - 1.0),
        r(0.0),
        r(1.0),
        r(0.0),
        50,
    )
    .unwrap();
    assert_eq!((z.x.get(), z.fx.get()), (1.0, 0.0));
}
