//! Attack tests from SMLib's confirmed defects, batch 2: `geom-num`.
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

use geom_num::Real;
use geom_num::brent::{self, BrentError};

fn r(v: f64) -> Real {
    Real::new(v).unwrap()
}

/// solidcore plan/upstream-issues/WR-19--smlocalsolve1d-without-an-interval-steps-against-newton.md
/// and its addendum WR-19--addendum-2026-10-31-unbounded-newton-step.md (b4de2149): a one-
/// dimensional solve made without a solution interval replaced every step with 1.584e19 against
/// the Newton direction, so f(t) = t - 3 from 0 or 5 never converged. The inputs are the
/// defects': t - 3 from 0 and 5, t^3 - 27, t^3 + 125 and t - 1e6, over brackets with no
/// useful interval (very wide, one-sided, and with the root at either end). Ours must find each
/// root to its accuracy, or say that it could not; never step away from it.
#[test]
fn wr_19_addendum_roots_over_very_wide_or_lopsided_brackets_are_found() {
    // Kinds: 0 is t - 3, 1 is t^3 - 27, 2 is t^3 + 125, 3 is t - 1e6.
    let eval = |kind: usize, t: f64| match kind {
        0 => t - 3.0,
        1 => t * t * t - 27.0,
        2 => t * t * t + 125.0,
        _ => t - 1.0e6,
    };
    let cases: [(&str, usize, f64, (f64, f64)); 11] = [
        ("t - 3 from 0", 0, 3.0, (0.0, 5.0)),
        ("t - 3 wide", 0, 3.0, (-1.58e19, 1.58e19)),
        ("t - 3 lopsided", 0, 3.0, (-1.0, 1e19)),
        ("t - 3 from 5", 0, 3.0, (-1e19, 5.0)),
        ("t^3 - 27", 1, 3.0, (-1e5, 1e5)),
        ("t^3 - 27 lopsided", 1, 3.0, (0.0, 1e5)),
        ("t^3 + 125", 2, -5.0, (-1e5, 1e5)),
        ("t^3 + 125 lopsided", 2, -5.0, (-1e5, 0.0)),
        ("t - 1e6", 3, 1.0e6, (-1e30, 1e30)),
        ("t - 1e6 root at the end", 3, 1.0e6, (0.0, 1.0e6)),
        ("t - 1e6 root at the start", 3, 1.0e6, (1.0e6, 2.0e6)),
    ];
    for (name, kind, root, (a, b)) in cases {
        let found = brent::zero(|t| Real::new(eval(kind, t.get())), r(a), r(b), r(0.0), 4000);
        match found {
            Ok(z) => {
                let tol = 1e-9 * root.abs().max(1.0);
                assert!(
                    (z.x.get() - root).abs() <= tol,
                    "{name}: found {} against {root}",
                    z.x.get()
                );
                assert!(z.evaluations <= 4000, "{name}");
                // The other end of the final bracket has the opposite sign, or f(x) is zero.
                assert!(z.fx.get() == 0.0 || eval(kind, z.x.get()) * eval(kind, z.other.get()) <= 0.0, "{name}");
            }
            // The only refusal a bracket that holds the root may give is that the cap ran out.
            Err(e) => assert_eq!(e, BrentError::CapReached, "{name}"),
        }
    }
    // A bracket that holds no root is refused as such, not stepped across.
    for (a, b) in [(4.0, 5.0), (-1e19, 2.0), (3.5, 1e19)] {
        assert_eq!(
            brent::zero(|t| Real::new(t.get() - 3.0), r(a), r(b), r(0.0), 4000).map(|z| z.x),
            Err(BrentError::SameSign),
            "({a}, {b})"
        );
    }
    // The minimum of (t - 3)^2, the stationary point of the same step: found from either side.
    for (a, b) in [(-100.0, 100.0), (0.0, 50.0), (-1e3, 4.0)] {
        let m = brent::minimum(
            |t| Real::new((t.get() - 3.0) * (t.get() - 3.0)),
            r(a),
            r(b),
            r(1e-10),
            4000,
        )
        .unwrap_or_else(|e| panic!("({a}, {b}): {e:?}"));
        assert!((m.x.get() - 3.0).abs() <= 1e-5, "({a}, {b}): {}", m.x.get());
        assert!(m.fx.get() <= 1e-8);
    }
}
