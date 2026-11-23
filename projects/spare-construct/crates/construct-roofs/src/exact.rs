//! The exact arithmetic the shape needs: whole numbers rounded half up, and the nearest whole
//! number to a length or an area carried along a slope, `x * sqrt(rise^2 + run^2) / run`, found
//! from integer square roots with no float.
//!
//! Rounding half up (to the larger whole number at a tie) is the one rounding here, because it
//! moves with a whole number added: `round(x + k) = round(x) + k` for every whole `k`, so a face
//! offset by a whole number of steps stays exactly parallel to the face it is offset from.

use crate::error::RoofError;
use construct_params::Slope;
use num_bigint::BigUint;

/// The farthest a coordinate may be from the origin, in grid steps: the kernel's grid, `2^40 - 1`
/// steps (171.8 km).
pub const GRID_LIMIT: i64 = 1_099_511_627_775;

/// `v` if it is on the kernel's grid.
pub(crate) fn on_grid(v: i128) -> Result<i64, RoofError> {
    i64::try_from(v)
        .ok()
        .filter(|v| v.unsigned_abs() <= GRID_LIMIT.unsigned_abs())
        .ok_or(RoofError::OutOfRange)
}

/// The nearest whole number to `num / den`, a half up; `den` above zero.
pub(crate) fn round_half_up(num: i128, den: i128) -> Result<i128, RoofError> {
    // floor((2 num + den) / (2 den)).
    let twice = den.checked_mul(2).ok_or(RoofError::OutOfRange)?;
    num.checked_mul(2)
        .and_then(|n| n.checked_add(den))
        .and_then(|n| n.checked_div_euclid(twice))
        .ok_or(RoofError::OutOfRange)
}

/// `rise * a / run` to the nearest whole step, a half up: the height of a plane of slope `slope`
/// `a` steps from where it is at 0.
pub(crate) fn height(slope: Slope, a: i64) -> Result<i64, RoofError> {
    let num = i128::from(slope.rise())
        .checked_mul(i128::from(a))
        .ok_or(RoofError::OutOfRange)?;
    on_grid(round_half_up(num, i128::from(slope.run()))?)
}

/// The nearest whole number to `x * sqrt(rise^2 + run^2) / run`, a half up: `x` (at or above
/// zero) carried along the slope, as a face `x` steps square to it stands that many steps higher
/// measured upright, or a plan area is that much as laid on the slope.
///
/// Exact: with `N = x^2 (rise^2 + run^2)` and `d = run`, the answer is
/// `floor((isqrt(4 N) + d) / (2 d))`, since `floor((y + d) / (2 d)) = floor((floor(y) + d) / (2 d))`
/// for any real `y` and whole `d` above zero.
pub(crate) fn along(x: u128, slope: Slope) -> Result<u128, RoofError> {
    let (rise, run) = (slope.rise().unsigned_abs(), slope.run().unsigned_abs());
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "arbitrary-precision whole numbers, which never overflow; run is above zero"
    )]
    let nearest = {
        let x = BigUint::from(x);
        let (rise, run) = (BigUint::from(rise), BigUint::from(run));
        let n: BigUint = &x * &x * (&rise * &rise + &run * &run);
        let root = (n << 2_u32).sqrt();
        (root + &run) / (run << 1_u32)
    };
    u128::try_from(&nearest).map_err(|_| RoofError::OutOfRange)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]
mod tests {
    use super::*;

    /// Half up at a tie either side of zero, so adding a whole number moves the result by it.
    #[test]
    fn rounding_is_half_up_and_moves_with_a_whole_number() {
        assert_eq!(round_half_up(1, 2).unwrap(), 1);
        assert_eq!(round_half_up(-1, 2).unwrap(), 0);
        assert_eq!(round_half_up(-3, 2).unwrap(), -1);
        assert_eq!(round_half_up(5, 4).unwrap(), 1);
        assert_eq!(round_half_up(-5, 4).unwrap(), -1);
        for num in -20..20 {
            assert_eq!(
                round_half_up(num + 12, 4).unwrap(),
                round_half_up(num, 4).unwrap() + 3
            );
        }
        let quarter = Slope::new(1, 4).unwrap();
        assert_eq!(height(quarter, 6).unwrap(), 2);
        assert_eq!(height(quarter, -6).unwrap(), -1);
    }

    /// Along a 3 in 4 slope a length grows by 5/4 exactly; along 1 in 1 by the square root of two,
    /// to the nearest whole number; along 1 in 4 by sqrt(17)/4.
    #[test]
    fn a_length_along_a_slope_is_the_nearest_whole_number() {
        let three_four = Slope::new(3, 4).unwrap();
        assert_eq!(along(400, three_four).unwrap(), 500);
        assert_eq!(along(2, three_four).unwrap(), 3, "2.5 goes up");
        let one = Slope::new(1, 1).unwrap();
        assert_eq!(along(1_000_000, one).unwrap(), 1_414_214);
        assert_eq!(along(0, one).unwrap(), 0);
        let quarter = Slope::new(1, 4).unwrap();
        // 960,000 x sqrt(17) / 4 = 989,545.35...
        assert_eq!(along(960_000, quarter).unwrap(), 989_545);
    }
}
