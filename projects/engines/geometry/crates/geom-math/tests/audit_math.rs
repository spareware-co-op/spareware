//! Audit findings in `geom-math` (independent audit of the merged core, branch `audit/math`).
//!
//! Every test here FAILS on the code as merged: each one shows a finding, with the input, what the
//! crate returns and what its own documentation says it returns. Nothing in `src/` is changed;
//! the fix is the owner's to decide. All three are small: a statement in a doc comment that the
//! code does not keep at an edge. The audit found no wrong classification, orientation, rounding
//! or image in this crate (its report lists what was checked and how).
//!
//! 1. `Transform::acts_in_plane` says a planar transform's classification "holds in the plane
//!    too"; a uniform scale of the plane is classified `NonUniform`.
//! 2. `Pythagorean::new` reports a defect (`BudgetExceeded`) for an input that is merely not a
//!    Pythagorean triple.
//! 3. `F2::length` and `F3::length` state a relative error bound that a subnormal length cannot
//!    meet.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "tests: native arithmetic, platform maths on an independent oracle (the formula as it is written), and a failure stops the test"
)]

use geom_math::{F2, Pythagorean, Transform, TransformKind};
use geom_num::NumError;

/// Finding 1. Input: `Transform::scale(2, 2, 1)`, the only way to write the uniform scale by 2 of
/// a 2D frame (a transform acts in the plane only when its z row is exactly (0, 0, 1, 0)).
/// `acts_in_plane` documents that for such a transform "its classification holds in the plane
/// too". In the plane this map multiplies every length by 2 and keeps circles, which is
/// `UniformScale`. Returned: `NonUniform`, whose meaning is "a circle becomes an ellipse", so
/// `keeps_circles()` is false. The classification is right for 3D (a sphere does become an
/// ellipsoid) and never claims a kind is kept when it is not; it is the sentence about the plane
/// that does not hold. `geom-curve` already works around it by classifying a planar transform
/// with z scaled to match (`moved.rs`); any other 2D caller that trusts the doc comment turns
/// circles into ellipses under a plain 2D scale.
#[test]
fn a_planar_uniform_scale_keeps_circles_in_the_plane() {
    let t = Transform::scale(2.0, 2.0, 1.0).unwrap();
    assert!(t.acts_in_plane());
    let class = t.classify().unwrap();
    assert_eq!(class.kind, TransformKind::UniformScale);
    assert!(class.keeps_circles());
}

/// Finding 2. Input: `Pythagorean::new(i64::MIN, i64::MIN, 1)`. Documented: `Degenerate` "unless
/// a² + b² = c² with c ≠ 0". Returned: `BudgetExceeded`, the error the crate keeps for a defect
/// of its own ("can fail only through a defect in this crate"). The comment beside the sum says
/// the squares of `i64` values are below 2^126 and their sum below 2^127; `i64::MIN` squared is
/// exactly 2^126, and two of them are 2^127, one past `i128`.
#[test]
fn a_non_triple_at_the_ends_of_i64_is_degenerate_not_a_defect() {
    assert_eq!(
        Pythagorean::new(i64::MIN, i64::MIN, 1),
        Err(NumError::Degenerate)
    );
}

/// Finding 3. Input: `F2::new(2^-1074, 2^-1074).length()`. The module header bounds the result
/// "within a relative 2.01u" of the exact length (plus 2^-1070 relative for components that round
/// when scaled; these do not). Exact length: √2 · 2^-1074. Returned: 2^-1074, the nearest double,
/// 29% away: no double is closer, so the bound cannot hold for a subnormal length, and the header
/// states it for every vector.
#[test]
fn the_stated_length_bound_holds_for_a_subnormal_length() {
    let tiny = f64::from_bits(1);
    let length = F2::new(tiny, tiny).unwrap().length().unwrap();
    // In units of 2^-1074 the exact length is √2 and the result a whole number of units.
    let units = length / tiny;
    let exact = 2.0_f64.sqrt();
    let relative = (units - exact).abs() / exact;
    let u = f64::EPSILON / 2.0;
    assert!(
        relative <= 2.01 * u + f64::from_bits(16),
        "the length is {units} units against √2: relative error {relative}"
    );
}
