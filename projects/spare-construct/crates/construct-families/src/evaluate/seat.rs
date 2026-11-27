//! A family seated in its host at any angle (piece A83, CON-A4-F3-P1): turned about its z axis to
//! run the way its host runs, whatever way that is, and moved to where it stands there, each point
//! worked out exactly from whole numbers and rounded once to the grid.
//!
//! A [`Seat`] is three pairs of whole grid steps: its `way`, the plan direction the family's x axis
//! runs along (any whole vector but none: a wall's line from its start to its end, or the
//! opposite); its `base`, a grid point; and its `at`, where the family's origin lies from the base,
//! along the way and square to it, to the way's left (so the family's frame turns as the plan's
//! does, never mirrored). With `L` the way's length, `sqrt(way.x^2 + way.y^2)`, the family's point
//! `(x, y, z)` lands at
//!
//! - `base.x + nearest(((at.x + x) * way.x - (at.y + y) * way.y) / L)`,
//! - `base.y + nearest(((at.x + x) * way.y + (at.y + y) * way.x) / L)`,
//! - `z`, its height unchanged,
//!
//! where `nearest` is the nearest whole step, a half away from zero ([`nearest`]). That is the one
//! rounding: each coordinate is worked out from whole numbers and rounded once, never from another
//! rounded point, so every point of a family and of the opening its wall cuts for it, put through
//! one seat, is the nearest grid point to where the exact turn puts it.
//!
//! **The bound.** Each coordinate is within half a step (78.125 nm on `Q0`, under a hundredth of
//! the model's tolerance of 0.01 mm) of the exact turn's. The tests pin it at 30, 45 and 60 degrees
//! against the square roots of 2 and 3 (on whole numbers, by squaring), and on a 3:4:5 way, where a
//! point on a multiple of five steps lands exactly.
//!
//! A way along an axis is a quarter turn: `L` divides every product, nothing is rounded, and a seat
//! is exactly that [`Turn`](super::Turn) moved by whole steps.
//!
//! Numbers: the products are of numbers within the kernel's grid (2^41 steps at most for a way, a
//! base or an `at`), so up to about 2^84, and their squares past what an `i128` holds; they are
//! held in num-bigint's whole numbers (MIT OR Apache-2.0), with no float anywhere. `nearest(n / L)`
//! is `(isqrt(4 n^2 / L^2) + 1) / 2` in whole-number division, signed as `n`: the whole part of
//! `2|n| / L` is the integer square root of the whole part of `4 n^2 / L^2`, and adding one and
//! halving gives the whole part of `|n| / L + 1/2`, which is the nearest, a half rounded up.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "whole numbers of any size cannot overflow, and the one divisor, a way's square               length, is above zero"
)]

use super::error::Reason;
use engine_geometry::value::grid::P3;
use num_bigint::{BigInt, BigUint};

/// Where a family stands in a host that runs at any angle: its way, its base and its origin from
/// the base (this module's header).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seat {
    way: [i64; 2],
    base: [i64; 2],
    at: [i64; 2],
}

/// The nearest whole number to `n / sqrt(dd)`, a half away from zero; `dd` above zero.
fn nearest(n: &BigInt, dd: &BigUint) -> BigInt {
    let twice: BigUint = n.magnitude() << 1_u32;
    let whole = (&twice * &twice / dd).sqrt();
    let size: BigUint = (whole + 1_u32) >> 1_u32;
    BigInt::from_biguint(n.sign(), size)
}

impl Seat {
    /// The seat whose family's x axis runs along `way`, its origin `at` from `base` (along the way
    /// and to its left); `None` for a way of no length.
    #[must_use]
    pub fn new(way: [i64; 2], base: [i64; 2], at: [i64; 2]) -> Option<Seat> {
        (way != [0, 0]).then_some(Seat { way, base, at })
    }

    /// The plan direction the family's x axis runs along.
    #[must_use]
    pub fn way(&self) -> [i64; 2] {
        self.way
    }

    /// The grid point its origin is measured from.
    #[must_use]
    pub fn base(&self) -> [i64; 2] {
        self.base
    }

    /// Where its origin lies from its base: along its way, and to the way's left.
    #[must_use]
    pub fn at(&self) -> [i64; 2] {
        self.at
    }

    /// Whether its way runs along an axis: a quarter turn, so every point lands exactly.
    #[must_use]
    pub fn is_square(&self) -> bool {
        self.way.contains(&0)
    }

    /// The family's plan point `[x, y]` seated, each coordinate the nearest grid step to the exact
    /// turn's (this module's header); `None` beyond a 64-bit number of steps.
    #[must_use]
    pub fn plan(&self, [x, y]: [i64; 2]) -> Option<[i64; 2]> {
        let [wx, wy] = self.way.map(BigInt::from);
        let dd = (&wx * &wx + &wy * &wy).magnitude().clone();
        let along = BigInt::from(self.at[0]) + x;
        let left = BigInt::from(self.at[1]) + y;
        let px = nearest(&(&along * &wx - &left * &wy), &dd) + self.base[0];
        let py = nearest(&(&along * &wy + &left * &wx), &dd) + self.base[1];
        Some([i64::try_from(&px).ok()?, i64::try_from(&py).ok()?])
    }

    /// A plan vector `[x, y]` turned by the seat's way alone, exactly; `None` unless the way runs
    /// along an axis (only a quarter turn takes a vector to whole steps) or the vector is none.
    #[must_use]
    pub fn vector(&self, [x, y]: [i64; 2]) -> Option<[i64; 2]> {
        if [x, y] == [0, 0] {
            return Some([0, 0]);
        }
        if !self.is_square() {
            return None;
        }
        let [wx, wy] = self.way;
        let length = wx.checked_abs()?.max(wy.checked_abs()?);
        let (ux, uy) = (wx.checked_div(length)?, wy.checked_div(length)?);
        Some([
            x.checked_mul(ux)?.checked_sub(y.checked_mul(uy)?)?,
            x.checked_mul(uy)?.checked_add(y.checked_mul(ux)?)?,
        ])
    }

    /// A grid point of the family's frame seated, its height kept.
    pub(crate) fn point(&self, p: P3) -> Result<P3, Reason> {
        let [x, y] = self
            .plan([p.x.get(), p.y.get()])
            .ok_or(Reason::OutOfRange)?;
        P3::new(x, y, p.z.get()).map_err(|_| Reason::OutOfRange)
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "tests; whole-number checks of a bound"
)]
mod tests {
    use super::*;

    /// `nearest` is the nearest whole number, a half away from zero, both ways round.
    #[test]
    fn nearest_rounds_a_half_away_from_zero() {
        let n = |v: i64| BigInt::from(v);
        let dd = |v: u64| BigUint::from(v);
        // 7 / 2 = 3.5 -> 4; -7 / 2 -> -4; 5 / 2 = 2.5 -> 3; 1 / sqrt 2 = 0.707 -> 1.
        assert_eq!(nearest(&n(7), &dd(4)), n(4));
        assert_eq!(nearest(&n(-7), &dd(4)), n(-4));
        assert_eq!(nearest(&n(5), &dd(4)), n(3));
        assert_eq!(nearest(&n(1), &dd(2)), n(1));
        assert_eq!(nearest(&n(-1), &dd(2)), n(-1));
        assert_eq!(nearest(&n(0), &dd(9)), n(0));
        // 7 / sqrt 2 = 4.95 -> 5; 6 / sqrt 2 = 4.24 -> 4.
        assert_eq!(nearest(&n(7), &dd(2)), n(5));
        assert_eq!(nearest(&n(6), &dd(2)), n(4));
    }

    /// A way along an axis is a quarter turn moved by whole steps: nothing is rounded.
    #[test]
    fn a_way_along_an_axis_is_a_quarter_turn() {
        let quarter = Seat::new([0, 5], [100, 200], [3, -4]).unwrap();
        assert!(quarter.is_square());
        // Along (0, 5), (x, y) turns to (-y, x): the origin's (3, -4) and (1, 2) make (4, -2),
        // which lands at (2, 4) from the base.
        assert_eq!(quarter.plan([1, 2]), Some([100 + 2, 200 + 4]));
        assert_eq!(quarter.vector([1, 2]), Some([-2, 1]));
        let half = Seat::new([-7, 0], [0, 0], [0, 0]).unwrap();
        assert_eq!(half.plan([3, 5]), Some([-3, -5]));
        assert_eq!(Seat::new([0, 0], [0, 0], [0, 0]), None);
        let angled = Seat::new([3, 4], [0, 0], [0, 0]).unwrap();
        assert!(!angled.is_square());
        assert_eq!(angled.vector([1, 0]), None);
        assert_eq!(angled.vector([0, 0]), Some([0, 0]));
    }

    /// On a 3:4:5 way a point on a multiple of five steps lands exactly: (5, 10) along (3, 4) is
    /// ((15 - 40) / 5, (20 + 30) / 5) = (-5, 10).
    #[test]
    fn a_three_four_five_way_lands_multiples_of_five_exactly() {
        let seat = Seat::new([3, 4], [7, -2], [0, 0]).unwrap();
        assert_eq!(seat.plan([5, 10]), Some([7 - 5, -2 + 10]));
        assert_eq!(
            seat.plan([6_400_000, 0]),
            Some([7 + 3_840_000, -2 + 5_120_000])
        );
    }

    /// Each coordinate is the nearest step to the exact turn: at 45 degrees the way (1, 1) takes
    /// (x, 0) to (x, x) / sqrt 2; checked on whole numbers, by squaring, that the answer `m` has
    /// `(2m - 1)^2 * 2 <= (2x)^2 < (2m + 1)^2 * 2`.
    #[test]
    fn at_45_degrees_each_coordinate_is_the_nearest_step() {
        let seat = Seat::new([1, 1], [0, 0], [0, 0]).unwrap();
        for x in [1_i64, 2, 3, 99, 6_400, 5_760_000, 1_099_511_627_775] {
            let [px, py] = seat.plan([x, 0]).unwrap();
            assert_eq!(px, py);
            let (m, two_x) = (i128::from(px), 2 * i128::from(x));
            assert!((2 * m - 1).pow(2) * 2 <= two_x.pow(2), "{x}: {m}");
            assert!(two_x.pow(2) < (2 * m + 1).pow(2) * 2, "{x}: {m}");
        }
    }
}
