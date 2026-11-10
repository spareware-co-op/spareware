//! Independent audit of `geom_num::scalar`: one residual, evaluated at one point, must give a
//! rounded value (`Real`) that lies in its certified enclosure (`Interval`).
//!
//! Finding shown here: on the angle's branch cut the two disagree by a whole turn. `Scalar::atan2`
//! is documented as "the angle of the point (`x`, `self`) in radians, in (−π, π]", and the interval
//! version follows that: a zero `y` is the real 0, so a point on the negative x-axis has the angle
//! π. The `Real` version returns IEEE 754's `atan2`, which reads the sign of a zero `y` and gives
//! −π for `y = −0`: a value outside the documented range, 2π away from the exact angle, and
//! outside the enclosure the same residual gives at the same point. A `Real` is −0 whenever a zero
//! is negated (`Scalar::neg` of +0, exact on purpose) or multiplied by a negative number, so the
//! residual below, the angle of a clockwise turn, reaches it at a plain argument.
//!
//! The exact value is π: the point is (−1, 0), whose principal angle in (−π, π] is π
//! (IEEE Std 1788-2015's `atan2`, the definition `Interval::atan2` cites). A `Ball` does not show
//! the disagreement only because `Ball::to_interval` widens even an exact zero by one step, so its
//! angle there is the whole range [−π, π], which holds both answers.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a broken expectation must stop the test loudly"
)]

use geom_num::interval::Interval;
use geom_num::real::Real;
use geom_num::scalar::Scalar;
use geom_num::{NumError, math};

/// The angle of the direction a clockwise turn of `t` half-turns gives the x-axis:
/// `atan2(−sin(πt), cos(πt))`. At `t = 1` the direction is (−1, 0) exactly: `sinpi(1)` is an exact
/// zero and `cospi(1)` is exactly −1.
fn clockwise_angle<S: Scalar>(t: S) -> Result<S, NumError> {
    t.sinpi()?.neg().atan2(t.cospi()?)
}

/// The rounded value lies in the certified enclosure of the same residual at the same point.
#[test]
fn the_rounded_angle_lies_in_its_enclosure_on_the_branch_cut() {
    let one = Real::ONE;
    let rounded = clockwise_angle(one).unwrap().get();
    let enclosure = clockwise_angle(Interval::from(one)).unwrap();
    // The enclosure is the two doubles either side of π: the exact angle of (−1, 0).
    assert!(
        enclosure.lo() < core::f64::consts::PI.next_up() && enclosure.lo() > 3.14,
        "the enclosure is around π: {enclosure:?}"
    );
    assert!(
        enclosure.contains(rounded),
        "Real gives {rounded:e} for the angle of (−1, 0); the enclosure of the same residual at \
         the same point is {enclosure:?}"
    );
}

/// `Scalar::atan2`'s documented range is (−π, π]: −π is outside it.
#[test]
fn the_angle_of_a_real_is_in_its_documented_range() {
    let minus_one = Real::new(-1.0).unwrap();
    let zero = Real::ZERO.neg();
    let angle = zero.atan2(minus_one).unwrap().get();
    assert!(
        angle > -core::f64::consts::PI,
        "atan2 of the Real zero that neg(+0) makes, at x = −1, is {angle:e}: outside (−π, π]"
    );
    // What the point function returns for the real 0 there, for the record.
    assert_eq!(math::atan2(0.0, -1.0).unwrap().get(), core::f64::consts::PI);
}
