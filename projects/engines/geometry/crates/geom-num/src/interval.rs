//! Intervals of doubles that certainly contain an exact real result, in round-to-nearest arithmetic.
//!
//! An [`Interval`] `[lo, hi]` of finite doubles stands for a real number known only to lie between its
//! bounds. Every operation returns an interval that contains the exact real result of the operation
//! applied to any reals in its operands (the inclusion property), so a curved computation carries a
//! certified bound on its own rounding error (chapter 6.4). Intervals are not a stage of the exact
//! predicates: those use a semi-static filter and then exact integers (chapter 6.2).
//!
//! How: each operation rounds to nearest as usual, then moves the lower bound down and the upper bound
//! up by one floating-point step, with `next_down` and `next_up`. The rounding mode is never changed:
//! Rust makes changing it undefined behaviour and WebAssembly has no rounding modes (evidence note
//! 05, section 7).
//!
//! Why one step is enough: round to nearest is faithful, that is the rounded result `r` of an exact
//! value `x` is `x` itself or one of the two doubles either side of `x`. Either way
//! `next_down(r) ≤ x ≤ next_up(r)`, at binade boundaries, in the subnormal range and at zero alike.
//! A faithfully rounded square root (the correctly rounded one of [`crate::math::sqrt`]) is covered
//! by the same argument.
//!
//! Guarantees:
//!
//! - **Inclusion**, as above, for `add`, `sub`, `mul`, `div`, `square`, `sqrt` and `neg`; and for
//!   `sqrt_nonneg`, over the non-negative reals of its operand.
//! - **Finite, ordered, never NaN.** Both bounds are finite and `lo ≤ hi`. An operation whose widened
//!   bound would leave the finite doubles (a rounded bound that reaches ±(2 − 2^−52) · 2^1023 or
//!   overflows) returns [`NumError::NotFinite`] instead.
//! - **Tight.** Each computed bound is less than two floating-point steps outside the exact bound of
//!   the result: one rounding and one widening.
//! - **Deterministic.** Only IEEE 754 basic operations, comparisons, `next_up`, `next_down` and the
//!   square root of [`crate::math`], which Rust computes to IEEE 754 binary64 on every target the
//!   kernel builds for (RFC 3514, whose own exception, 32-bit x86 without SSE2, is not one of them).
//!   Minimum and maximum are written as comparisons, not with `f64::min` and `f64::max`, which may
//!   return either zero when given −0 and +0. A zero bound is always stored as +0, so equal intervals
//!   have equal bits whatever their history.
//!
//! Every rounded bound is widened, even one that happens to be exact: `3 − 3` gives
//! [−2^−1074, 2^−1074], not [0, 0], so `certain_sign` of a computed zero is `None` and the decision
//! falls to the exact predicates (chapter 6.2), which is where a zero is decided. The one cost is for
//! quantities that are never negative by construction, such as a squared distance: widening can put
//! their lower bound just below zero, where [`Interval::sqrt`] refuses. [`Interval::square`] knows
//! that x² ≥ 0, and [`Interval::sqrt_nonneg`] takes the root of the non-negative part, so a distance
//! near zero, even between coincident points, is still certified.
//!
//! Method: interval arithmetic in round-to-nearest with outward widening, as in Geogram's
//! `intervalRN` (after Harris) and in CGAL's widening with `nextafter` when it cannot change the
//! rounding mode. Harris widens each bound by the relative factor 1 ± 2^−52, a second rounded
//! operation; this module moves each bound by exactly one step, which is tighter and needs no
//! further rounding.
//!
//! - R. Harris, "Why interval arithmetic won't cure your floating point blues", *Overload* 19(103),
//!   June 2011, <https://accu.org/journals/overload/19/103/harris_1974/>.
//! - Geogram `interval_nt.h` (BSD-3-Clause), the round-to-nearest `intervalRN` class:
//!   <https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/interval_nt.h#L455-L462>.
//! - CGAL `FPU.h`, widening with `nextafter`:
//!   <https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Number_types/include/CGAL/FPU.h#L347-L358>.

use crate::error::NumError;
use crate::math;
use crate::sign::Sign;

/// A closed interval `[lo, hi]` of finite doubles, `lo ≤ hi`, that contains an unknown real number.
///
/// Made only by [`Interval::point`], [`Interval::new`] and the operations, so the invariant always
/// holds. Zero bounds are stored as +0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    lo: f64,
    hi: f64,
}

/// −0 becomes +0; every other value, infinities included, is unchanged (IEEE: −0 + +0 = +0 when
/// rounding to nearest, and `x + 0 = x` for every other `x`).
const fn canon(x: f64) -> f64 {
    x + 0.0
}

/// The smaller of two doubles that are not NaN; either zero when given −0 and +0, which the widening
/// that always follows makes irrelevant (`next_down` of either zero is −2^−1074).
const fn min2(x: f64, y: f64) -> f64 {
    if y < x { y } else { x }
}

/// The larger of two doubles that are not NaN; either zero when given −0 and +0, which the widening
/// that always follows makes irrelevant (`next_up` of either zero is +2^−1074).
const fn max2(x: f64, y: f64) -> f64 {
    if y > x { y } else { x }
}

/// The interval from rounded bounds `lo ≤ hi`, each the faithfully rounded value of an exact bound,
/// widened outward by one step each; `NotFinite` if a widened bound is infinite.
///
/// Correct for every input that is not NaN: if a rounded bound overflowed, then either `lo` is −∞ or
/// `hi` is +∞ (because `lo ≤ hi`), and `next_down(−∞)` and `next_up(+∞)` stay infinite, so the check
/// catches it. NaN, which no operation here produces, would fail the check too.
const fn widened(lo: f64, hi: f64) -> Result<Interval, NumError> {
    let lo = canon(lo.next_down());
    let hi = canon(hi.next_up());
    if lo.is_finite() && hi.is_finite() {
        Ok(Interval { lo, hi })
    } else {
        Err(NumError::NotFinite)
    }
}

// The arithmetic methods return `Result` (overflow, division by an interval containing zero), so
// they are deliberately not the `core::ops` traits, whose callers expect a plain value.
#[allow(clippy::should_implement_trait, reason = "fallible arithmetic returns Result, not Self")]
impl Interval {
    /// The interval holding exactly `x`.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `x` is NaN or infinite.
    pub const fn point(x: f64) -> Result<Interval, NumError> {
        Interval::new(x, x)
    }

    /// The interval `[lo, hi]`.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if either bound is NaN or infinite (checked first);
    /// [`NumError::EmptyInterval`] if `lo > hi`.
    pub const fn new(lo: f64, hi: f64) -> Result<Interval, NumError> {
        if !lo.is_finite() || !hi.is_finite() {
            Err(NumError::NotFinite)
        } else if lo > hi {
            Err(NumError::EmptyInterval)
        } else {
            Ok(Interval { lo: canon(lo), hi: canon(hi) })
        }
    }

    /// The lower bound.
    #[must_use]
    pub const fn lo(self) -> f64 {
        self.lo
    }

    /// The upper bound.
    #[must_use]
    pub const fn hi(self) -> f64 {
        self.hi
    }

    /// Whether `lo ≤ x ≤ hi`. Always false for NaN.
    #[must_use]
    pub fn contains(self, x: f64) -> bool {
        (self.lo..=self.hi).contains(&x)
    }

    /// The sign every real in the interval has, when they all have the same one.
    ///
    /// `Positive` when `lo > 0`, `Negative` when `hi < 0`, `Zero` when the interval is exactly
    /// `[0, 0]`, and `None` when it holds zero and other values: the sign is not decided, and the
    /// caller falls back to exact arithmetic.
    #[must_use]
    pub const fn certain_sign(self) -> Option<Sign> {
        if self.lo > 0.0 {
            Some(Sign::Positive)
        } else if self.hi < 0.0 {
            Some(Sign::Negative)
        } else if self.lo == 0.0 && self.hi == 0.0 {
            Some(Sign::Zero)
        } else {
            None
        }
    }

    /// The negation `[−hi, −lo]`. Exact: negating a double never rounds, so there is no widening.
    #[must_use]
    pub const fn neg(self) -> Interval {
        Interval { lo: canon(-self.hi), hi: canon(-self.lo) }
    }

    /// The sum: `[lo₁ + lo₂, hi₁ + hi₂]`, each bound rounded and widened by one step.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a widened bound is not finite.
    pub const fn add(self, other: Interval) -> Result<Interval, NumError> {
        widened(self.lo + other.lo, self.hi + other.hi)
    }

    /// The difference: `[lo₁ − hi₂, hi₁ − lo₂]`, each bound rounded and widened by one step.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a widened bound is not finite.
    pub const fn sub(self, other: Interval) -> Result<Interval, NumError> {
        widened(self.lo - other.hi, self.hi - other.lo)
    }

    /// The product: the smallest and largest of the four products of bounds, rounded and widened by
    /// one step.
    ///
    /// Why the corners suffice: `x · y` is linear in each variable, so over the box of operands its
    /// extremes are at corners. Rounding to nearest is monotone, so the smallest rounded corner is the
    /// rounded smallest corner, and likewise for the largest. A product of finite doubles is never
    /// NaN.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a widened bound is not finite.
    pub const fn mul(self, other: Interval) -> Result<Interval, NumError> {
        let ll = self.lo * other.lo;
        let lh = self.lo * other.hi;
        let hl = self.hi * other.lo;
        let hh = self.hi * other.hi;
        widened(min2(min2(ll, lh), min2(hl, hh)), max2(max2(ll, lh), max2(hl, hh)))
    }

    /// The quotient `self / other`: the smallest and largest of the four quotients of bounds, rounded
    /// and widened by one step.
    ///
    /// Why the corners suffice: with zero outside the divisor, `x / y` is monotone in each variable
    /// over the box of operands, so its extremes are at corners; rounding to nearest is monotone. A
    /// quotient of finite doubles by a nonzero double is never NaN.
    ///
    /// # Errors
    ///
    /// [`NumError::DivisorContainsZero`] if `other` contains zero (checked first);
    /// [`NumError::NotFinite`] if a widened bound is not finite.
    pub const fn div(self, other: Interval) -> Result<Interval, NumError> {
        if other.lo <= 0.0 && other.hi >= 0.0 {
            return Err(NumError::DivisorContainsZero);
        }
        let ll = self.lo / other.lo;
        let lh = self.lo / other.hi;
        let hl = self.hi / other.lo;
        let hh = self.hi / other.hi;
        widened(min2(min2(ll, lh), min2(hl, hh)), max2(max2(ll, lh), max2(hl, hh)))
    }

    /// The square: every x² for x in the interval, which is never negative.
    ///
    /// Not the same as `self.mul(self)`, which treats its two operands as independent reals: for
    /// [−1, 2], `mul` gives about [−2, 4], but the squares are [0, 4]. Over the interval, x² is
    /// smallest at the point nearest zero and largest at the bound farthest from it, so:
    ///
    /// - When the interval holds zero, the lower bound is exactly 0, the square of zero, with no
    ///   rounding and no widening.
    /// - Otherwise the lower bound is the square of the bound nearer zero, rounded and moved down one
    ///   step, but never below 0: when that square underflows to 0, moving down would give −2^−1074,
    ///   and 0 is still a lower bound because every square is ≥ 0.
    /// - The upper bound is the square of the bound farther from zero, rounded and moved up one step.
    ///
    /// Inclusion follows as for [`Interval::mul`] (faithful rounding, one step outward); the result is
    /// tight in the same sense, and never has a negative lower bound.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if the widened upper bound is not finite.
    pub const fn square(self) -> Result<Interval, NumError> {
        let (near, far) = if self.lo >= 0.0 {
            (self.lo, self.hi)
        } else if self.hi <= 0.0 {
            (-self.hi, -self.lo)
        } else {
            (0.0, max2(-self.lo, self.hi))
        };
        let hi = canon((far * far).next_up());
        if !hi.is_finite() {
            return Err(NumError::NotFinite);
        }
        let lo = if near == 0.0 {
            0.0
        } else {
            canon((near * near).next_down())
        };
        Ok(Interval { lo: if lo < 0.0 { 0.0 } else { lo }, hi })
    }

    /// The square root: `[√lo, √hi]`, each bound rounded and widened by one step, except that a zero
    /// bound stays zero (the square root of zero is exact).
    ///
    /// The widened lower bound is never negative: for `lo > 0` the rounded root is at least 2^−537,
    /// and `next_down` of a positive double is at least +0. The upper bound is at most about 2^512,
    /// so it is always finite.
    ///
    /// # Errors
    ///
    /// [`NumError::NegativeSqrt`] if `lo < 0`.
    pub fn sqrt(self) -> Result<Interval, NumError> {
        if self.lo < 0.0 {
            return Err(NumError::NegativeSqrt);
        }
        let lo = if self.lo == 0.0 { 0.0 } else { canon(math::sqrt(self.lo).next_down()) };
        let hi = if self.hi == 0.0 { 0.0 } else { canon(math::sqrt(self.hi).next_up()) };
        Ok(Interval { lo, hi })
    }

    /// The square root of the interval's non-negative part: [`Interval::sqrt`] of
    /// `[max(lo, 0), hi]`.
    ///
    /// For a quantity that is never negative by construction, such as a square, a sum of squares or
    /// a squared distance, whose interval reaches below zero only through rounding and widening:
    /// for coincident points, dx = x − x is [−2^−1074, 2^−1074] and dx · dx + dy · dy has a negative
    /// lower bound, where [`Interval::sqrt`] refuses, although the exact distance is 0.
    ///
    /// The result contains the square root of every non-negative real in the interval. It says
    /// nothing about negative reals, which have no real root: the caller vouches that the exact value
    /// is not one of them, as the construction does. [`Interval::sqrt`] stays the strict form, which
    /// refuses any interval with a negative value in it.
    ///
    /// # Errors
    ///
    /// [`NumError::NegativeSqrt`] if `hi < 0`: every real in the interval is negative, so the exact
    /// value cannot be the non-negative quantity the caller vouched for.
    pub fn sqrt_nonneg(self) -> Result<Interval, NumError> {
        if self.hi < 0.0 {
            return Err(NumError::NegativeSqrt);
        }
        let lo = if self.lo < 0.0 { 0.0 } else { self.lo };
        Interval { lo, hi: self.hi }.sqrt()
    }
}
