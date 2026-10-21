//! Intervals of doubles that certainly contain an exact real result, in round-to-nearest
//! arithmetic.
//!
//! An [`Interval`] `[lo, hi]` of finite doubles stands for a real number known only to lie between
//! its bounds. Every operation returns an interval that contains the exact real result of the
//! operation applied to any reals in its operands (the inclusion property), so a curved computation
//! carries a certified bound on its own rounding error (chapter 6.4). Intervals are not a stage of
//! the exact predicates: those are exact integers now, and a generated semi-static filter (build
//! piece G27, a later piece) will go ahead of them (chapter 6.2).
//!
//! How: each operation rounds to nearest as usual, then moves the lower bound down and the upper
//! bound up by one floating-point step, with `next_down` and `next_up`. The rounding mode is never
//! changed: Rust makes changing it undefined behaviour and WebAssembly has no rounding modes
//! (evidence note 05, section 7).
//!
//! Why one step is enough: round to nearest is faithful, that is the rounded result `r` of an exact
//! value `x` is `x` itself or one of the two doubles either side of `x`. Either way
//! `next_down(r) ≤ x ≤ next_up(r)`, at binade boundaries, in the subnormal range and at zero alike.
//! A faithfully rounded square root (the correctly rounded one of [`crate::math::sqrt`]) is covered
//! by the same argument.
//!
//! Guarantees:
//!
//! - **Inclusion**, as above, for `add`, `sub`, `mul`, `div`, `mul_add`, `square`, `sqrt`, `norm`
//!   and `neg`. These are built from the operations IEEE 754 rounds correctly, so one step outward
//!   is a proven enclosure. An enclosure that needs a function whose error `libm` does not prove
//!   (`sin`, `cos`, `atan2`, `exp`, `ln`) is refused with [`NumError::UnprovenMaths`] until proven
//!   versions exist (build piece G47).
//! - **Finite, ordered, never NaN.** Both bounds are finite and `lo ≤ hi`. An operation whose
//!   widened bound would leave the finite doubles (a rounded bound that reaches
//!   ±(2 − 2^−52) · 2^1023 or overflows) returns [`NumError::NotFinite`] instead.
//! - **Tight.** Each computed bound is the first or second double beyond the exact bound of the
//!   operation over its operands, never farther: the rounding lands on the exact bound or on one of
//!   the two doubles either side of it, and the widening moves one double outward. So the lower
//!   bound is at least the double below the largest double at or below the exact lower bound, and
//!   the upper bound at most the double above the smallest double at or above the exact upper
//!   bound. A bound that is exact by construction is not widened, so it is the exact bound
//!   itself: both bounds of `neg`, the lower bound 0 of `square` over an interval holding zero,
//!   and a zero bound of `sqrt`. `norm` chains several operations, so each of its steps is tight
//!   in this sense but the whole is not.
//!
//!   Counted in steps, where the step at a value is the spacing of the doubles in its binade, a
//!   computed bound is at most 2.5 steps of the exact bound outside it, at most 2 steps of the
//!   computed bound, less than 2 steps of the larger of the two, and at most 1.5 steps when the
//!   two steps are equal. The first two maxima are reached at ties next to a power of two, where
//!   the spacing halves: 1 + (−2^−54) = 1 − 2^−54 is a tie that rounds to the even 1 and is widened
//!   to 1 + 2^−52, which is 2.5 steps of 2^−53 (the spacing just below 1) above the exact value;
//!   and 1 + 2^−53 is a tie that also rounds to 1 and is widened to 1 − 2^−53, exactly 2 steps of
//!   2^−53 (the spacing at that bound) below the exact value.
//! - **Deterministic.** Only IEEE 754 basic operations and comparisons, which Rust computes to IEEE
//!   754 binary64 on every target the kernel builds for (RFC 3514, whose own exception, 32-bit x86
//!   without SSE2, is not one of them); `next_up` and `next_down`, which are integer operations on
//!   the bits; and the square root and fused multiply-add of [`crate::math`]. The fused
//!   multiply-add is `libm` 0.2.16's `fma`, correctly rounded as IEEE 754 requires whether `libm`
//!   uses the processor's instruction or its own routine. That root is not Rust's but `libm`
//!   0.2.16's `sqrt`, which is the processor's IEEE 754 square root on x86-64 (`sqrtsd`, with SSE2)
//!   and on aarch64 (`fsqrt`, with NEON), and on wasm32 libm's portable integer routine
//!   (`src/math/generic/sqrt.rs`: Goldschmidt iterations, then musl's decision of the last bit),
//!   because libm uses WebAssembly's square root instruction only with its `unstable-intrinsics`
//!   feature, which is off. IEEE 754 requires the square root to be correctly rounded, and the
//!   portable routine is written to the same result, so every route gives the same bits. Inclusion
//!   for the roots needs only a faithful root; equality across targets needs the correctly rounded
//!   one, which on wasm32 rests on libm's routine rather than on hardware, and libm's own comments
//!   rest that routine's last bit on testing, not on a proof. So `tests/attack2_interval.rs` holds
//!   `math::sqrt` bit for bit to an exact oracle (roots within a tiny fraction of a unit of a
//!   midpoint, subnormal operands, exact roots), and CI runs it on wasm32-wasip1 too
//!   (`.github/workflows/kernel.yml`). Minimum and maximum are written as comparisons, not with
//!   `f64::min` and `f64::max`, which may return either zero when given −0 and +0. A zero bound is
//!   always stored as +0, so equal intervals have equal bits whatever their history.
//!
//! Every rounded bound is widened, even one that happens to be exact: `3 − 3` gives
//! [−2^−1074, 2^−1074], not [0, 0], so `certain_sign` of a computed zero is `None` and the decision
//! falls to the exact predicates (chapter 6.2), which is where a zero is decided. The one cost is
//! for quantities that are never negative by construction, such as a squared distance: widening can
//! put their lower bound just below zero, where [`Interval::sqrt`] refuses. [`Interval::square`]
//! knows that x² ≥ 0 and [`Interval::norm`] knows that a sum of squares is ≥ 0, so a length near
//! zero, even between coincident points, is still certified.
//!
//! There is deliberately no public root of "the non-negative part" of an interval on the caller's
//! word that the exact value is not negative: when the caller is wrong, such a root is a false
//! certificate and nothing says so. For example `point(0).sub(point(2^−1074))` is
//! [−2^−1073, 0], whose exact value is −2^−1074, and the root of its non-negative part is [0, 0],
//! an exact zero. That operation is private, for [`Interval::norm`], which proves its radicand
//! non-negative itself.
//!
//! Method: interval arithmetic in round-to-nearest with outward widening, as in Geogram's
//! `intervalRN` (after Harris) and in CGAL's widening with `nextafter` when it cannot change the
//! rounding mode. Harris widens each bound by the relative factor 1 ± 2^−52, a second rounded
//! operation; this module moves each bound by exactly one step, which is tighter and needs no
//! further rounding. The square is the interval power of Moore, Kearfott and Cloud, which is
//! IEEE Std 1788-2015's `sqr`: over an interval holding zero x² is smallest at zero, otherwise at
//! the bound nearer zero, and largest at the bound farther from zero. The roots follow IEEE Std
//! 1788-2015's `sqrt`, whose domain is [0, +∞): `sqrt` refuses an operand that reaches outside the
//! domain, and the private root inside `norm` is the standard's set-based evaluation over the part
//! of the operand inside it. The length is the natural interval extension of √(c₁² + … + cₙ²)
//! (Moore, Kearfott and Cloud), built from those operations. The fused multiply-add is the natural
//! extension of x · y + z over the box of operands, with IEEE 754's correctly rounded
//! `fusedMultiplyAdd` (IEEE 754-2019, section 5.4.1) at the corners.
//!
//! - R. Harris, "Why interval arithmetic won't cure your floating point blues", *Overload* 19(103),
//!   June 2011, <https://accu.org/journals/overload/19/103/harris_1974/>.
//! - Geogram `interval_nt.h` (BSD-3-Clause), the round-to-nearest `intervalRN` class:
//!   <https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/interval_nt.h#L455-L462>.
//! - CGAL `FPU.h`, widening with `nextafter`:
//!   <https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Number_types/include/CGAL/FPU.h#L347-L358>.
//! - R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to Interval Analysis*, SIAM, 2009:
//!   the integer power of an interval and the natural interval extension of an expression.
//! - IEEE Std 1788-2015, *IEEE Standard for Interval Arithmetic*: the required elementary functions
//!   `sqr` and `sqrt`, the set-based flavour, and the decoration `trv` that reports a cut domain.
//! - `libm` 0.2.16 (MIT), `src/math/sqrt.rs`, `src/math/arch/` and `src/math/generic/sqrt.rs`:
//!   <https://github.com/rust-lang/compiler-builtins/tree/dfd2203a4d6110820ad7bb65cafe1bf331a03a3d/libm/src/math>.
//!
//! Cost: constant time and no allocation per operation; [`Interval::norm`] is linear in its number
//! of components.
//!
//! Consulted: Geogram `interval_nt.h`, BSD-3-Clause,
//!   <https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/interval_nt.h#L455-L462>,
//!   for its round-to-nearest interval class `intervalRN`, which the Method above compares.
//! Consulted: CGAL `FPU.h`, `LGPL-3.0-or-later OR LicenseRef-Commercial`,
//!   <https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Number_types/include/CGAL/FPU.h#L347-L358>,
//!   for its widening with `nextafter` when the rounding mode cannot change, which the Method
//!   above names.
//! Consulted: `libm` 0.2.16, MIT,
//!   <https://github.com/rust-lang/compiler-builtins/tree/dfd2203a4d6110820ad7bb65cafe1bf331a03a3d/libm/src/math>,
//!   for which instruction or routine its square root and fused multiply-add use on each target,
//!   as the guarantees above state.

use crate::error::NumError;
use crate::math;
use crate::real::{Real, zero_to_plus};
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

/// The smaller of two doubles that are not NaN; either zero when given −0 and +0, which the
/// widening that always follows makes irrelevant (`next_down` of either zero is −2^−1074). Not
/// [`crate::real::Real::total_min`], which orders the zeros: these are raw rounded bounds, made
/// finite and given one zero only by [`widened`].
const fn min2(x: f64, y: f64) -> f64 {
    if y < x { y } else { x }
}

/// The larger of two doubles that are not NaN; either zero when given −0 and +0, which the widening
/// that always follows makes irrelevant (`next_up` of either zero is +2^−1074). Not
/// [`crate::real::Real::total_max`], for the same reason as [`min2`].
const fn max2(x: f64, y: f64) -> f64 {
    if y > x { y } else { x }
}

/// The interval from rounded bounds `lo ≤ hi`, each the faithfully rounded value of an exact bound,
/// widened outward by one step each; `NotFinite` if a widened bound is infinite.
///
/// Correct for every input that is not NaN: if a rounded bound overflowed, then either `lo` is −∞
/// or `hi` is +∞ (because `lo ≤ hi`), and `next_down(−∞)` and `next_up(+∞)` stay infinite, so the
/// check catches it. NaN, which no operation here produces, would fail the check too.
const fn widened(lo: f64, hi: f64) -> Result<Interval, NumError> {
    let lo = zero_to_plus(lo.next_down());
    let hi = zero_to_plus(hi.next_up());
    if lo.is_finite() && hi.is_finite() {
        Ok(Interval { lo, hi })
    } else {
        Err(NumError::NotFinite)
    }
}

// The arithmetic methods return `Result` (overflow, division by an interval containing zero), so
// they are deliberately not the `core::ops` traits, whose callers expect a plain value.
#[allow(
    clippy::should_implement_trait,
    reason = "fallible arithmetic returns Result, not Self"
)]
impl Interval {
    /// The interval holding exactly `x`.
    ///
    /// It encloses the double `x` itself, nothing more. If `x` was computed, for example by a
    /// [`crate::math`] function, the interval does not enclose the exact value of that computation,
    /// so it is no certificate for it: a certified bound on a computed value comes only from this
    /// module's operations, each of which widens its result (and refuses the functions it cannot
    /// prove).
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
            Ok(Interval {
                lo: zero_to_plus(lo),
                hi: zero_to_plus(hi),
            })
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
        Interval {
            lo: zero_to_plus(-self.hi),
            hi: zero_to_plus(-self.lo),
        }
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
    /// extremes are at corners. Rounding to nearest is monotone, so the smallest rounded corner is
    /// the rounded smallest corner, and likewise for the largest. A product of finite doubles is
    /// never NaN.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a widened bound is not finite.
    pub const fn mul(self, other: Interval) -> Result<Interval, NumError> {
        let ll = self.lo * other.lo;
        let lh = self.lo * other.hi;
        let hl = self.hi * other.lo;
        let hh = self.hi * other.hi;
        widened(
            min2(min2(ll, lh), min2(hl, hh)),
            max2(max2(ll, lh), max2(hl, hh)),
        )
    }

    /// The quotient `self / other`: the smallest and largest of the four quotients of bounds,
    /// rounded and widened by one step.
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
        widened(
            min2(min2(ll, lh), min2(hl, hh)),
            max2(max2(ll, lh), max2(hl, hh)),
        )
    }

    /// The fused multiply-add `self · b + c`: the smallest and largest of `x · y + z` over the
    /// corners, each computed as one fused multiply-add (rounded once), widened by one step.
    ///
    /// Why the corners suffice: for a fixed z, x · y + z is linear in each of x and y, so over the
    /// box of operands its extremes are at corners of the box of x and y; and it increases with z,
    /// so the smallest is at the lower bound of `c` and the largest at its upper bound. A fused
    /// multiply-add is correctly rounded, a single rounding of the exact x · y + z, and rounding
    /// to nearest is monotone, so the smallest rounded corner is the rounded smallest corner, and
    /// likewise for the largest. Each bound is then a faithfully rounded exact bound, and one step
    /// outward encloses it, as for [`Interval::mul`]. Tighter than `self.mul(b)?.add(c)`, which
    /// rounds and widens twice.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a bound is not finite. A corner that overflows makes a bound
    /// overflow too: each corner of the upper bound is the same product as a corner of the lower
    /// bound, with the upper bound of `c` in place of the lower one, so a corner that overflows
    /// upward makes the upper bound overflow, and one that overflows downward the lower bound.
    pub fn mul_add(self, b: Interval, c: Interval) -> Result<Interval, NumError> {
        let low = |x: f64, y: f64| math::fma(x, y, c.lo).map(Real::get);
        let high = |x: f64, y: f64| math::fma(x, y, c.hi).map(Real::get);
        let (ll, lh, hl, hh) = (
            low(self.lo, b.lo)?,
            low(self.lo, b.hi)?,
            low(self.hi, b.lo)?,
            low(self.hi, b.hi)?,
        );
        let lo = min2(min2(ll, lh), min2(hl, hh));
        let (ll, lh, hl, hh) = (
            high(self.lo, b.lo)?,
            high(self.lo, b.hi)?,
            high(self.hi, b.lo)?,
            high(self.hi, b.hi)?,
        );
        widened(lo, max2(max2(ll, lh), max2(hl, hh)))
    }

    /// The sine over the interval: refused, until a sine with a proven error bound exists.
    ///
    /// `libm`'s sine is accurate, but its error is not proven, so an interval built from it would
    /// not be a certificate (principle 2). Proven versions are build piece G47, which keeps this
    /// signature. Until then, a caller that can accept an unproven value computes
    /// [`crate::math::sin`] and marks what it makes from it
    /// [`Approximate`](crate::exactness::Exactness::Approximate).
    ///
    /// # Errors
    ///
    /// Always [`NumError::UnprovenMaths`].
    #[allow(
        clippy::unused_self,
        reason = "refuses every interval until G47, which keeps this signature"
    )]
    pub const fn sin(self) -> Result<Interval, NumError> {
        Err(NumError::UnprovenMaths)
    }

    /// The cosine over the interval: refused, until a cosine with a proven error bound exists, for
    /// the reason given on [`Interval::sin`].
    ///
    /// # Errors
    ///
    /// Always [`NumError::UnprovenMaths`].
    #[allow(
        clippy::unused_self,
        reason = "refuses every interval until G47, which keeps this signature"
    )]
    pub const fn cos(self) -> Result<Interval, NumError> {
        Err(NumError::UnprovenMaths)
    }

    /// The angle of the points (x, y) for y in this interval and x in `x`: refused, until an
    /// arctangent with a proven error bound exists, for the reason given on [`Interval::sin`].
    ///
    /// # Errors
    ///
    /// Always [`NumError::UnprovenMaths`].
    #[allow(
        clippy::unused_self,
        reason = "refuses every interval until G47, which keeps this signature"
    )]
    pub const fn atan2(self, x: Interval) -> Result<Interval, NumError> {
        let _ = x;
        Err(NumError::UnprovenMaths)
    }

    /// e to the power of the interval: refused, until an exponential with a proven error bound
    /// exists, for the reason given on [`Interval::sin`].
    ///
    /// # Errors
    ///
    /// Always [`NumError::UnprovenMaths`].
    #[allow(
        clippy::unused_self,
        reason = "refuses every interval until G47, which keeps this signature"
    )]
    pub const fn exp(self) -> Result<Interval, NumError> {
        Err(NumError::UnprovenMaths)
    }

    /// The natural logarithm over the interval: refused, until a logarithm with a proven error
    /// bound exists, for the reason given on [`Interval::sin`].
    ///
    /// # Errors
    ///
    /// Always [`NumError::UnprovenMaths`].
    #[allow(
        clippy::unused_self,
        reason = "refuses every interval until G47, which keeps this signature"
    )]
    pub const fn ln(self) -> Result<Interval, NumError> {
        Err(NumError::UnprovenMaths)
    }

    /// The square: every x² for x in the interval, which is never negative.
    ///
    /// Not the same as `self.mul(self)`, which treats its two operands as independent reals: for
    /// [−1, 2], `mul` gives about [−2, 4], but the squares are [0, 4]. Over the interval, x² is
    /// smallest at the point nearest zero and largest at the bound farthest from it, so:
    ///
    /// - When the interval holds zero, the lower bound is exactly 0, the square of zero, with no
    ///   rounding and no widening.
    /// - Otherwise the lower bound is the square of the bound nearer zero, rounded and moved down
    ///   one step, but never below 0: when that square underflows to 0, moving down would give
    ///   −2^−1074, and 0 is still a lower bound because every square is ≥ 0.
    /// - The upper bound is the square of the bound farther from zero, rounded and moved up one
    ///   step.
    ///
    /// Inclusion follows as for [`Interval::mul`] (faithful rounding, one step outward). Tight as
    /// the module states: each bound is the first or second double beyond the exact bound, except
    /// the lower bound 0 over an interval holding zero, which is the exact bound itself. A lower
    /// bound kept at 0 because the square underflowed is still the first double below that
    /// square, which is positive and at most 2^−1074. The lower bound is never negative.
    ///
    /// Method: the interval power of Moore, Kearfott and Cloud, IEEE Std 1788-2015's `sqr`, with
    /// the rounding and widening of every other operation here.
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
        let hi = zero_to_plus((far * far).next_up());
        if !hi.is_finite() {
            return Err(NumError::NotFinite);
        }
        let lo = if near == 0.0 {
            0.0
        } else {
            zero_to_plus((near * near).next_down())
        };
        Ok(Interval {
            lo: if lo < 0.0 { 0.0 } else { lo },
            hi,
        })
    }

    /// The square root: `[√lo, √hi]`, each bound rounded and widened by one step, except that a
    /// zero bound stays zero (the square root of zero is exact).
    ///
    /// The widened lower bound is never negative: for `lo > 0` the rounded root is at least 2^−537,
    /// and `next_down` of a positive double is at least +0. The upper bound is at most about 2^512,
    /// so it is always finite.
    ///
    /// Method: IEEE Std 1788-2015's `sqrt`, whose domain is [0, +∞), refusing an operand that
    /// reaches outside the domain rather than cutting it; the root itself is the correctly rounded
    /// one of [`crate::math::sqrt`].
    ///
    /// # Errors
    ///
    /// [`NumError::NegativeSqrt`] if `lo < 0`.
    pub fn sqrt(self) -> Result<Interval, NumError> {
        if self.lo < 0.0 {
            return Err(NumError::NegativeSqrt);
        }
        // Each root is of a finite, positive double, which `math::sqrt` never refuses; were it to,
        // its error is returned, not a bound.
        let lo = if self.lo == 0.0 {
            0.0
        } else {
            zero_to_plus(math::sqrt(self.lo)?.get().next_down())
        };
        let hi = if self.hi == 0.0 {
            0.0
        } else {
            zero_to_plus(math::sqrt(self.hi)?.get().next_up())
        };
        Ok(Interval { lo, hi })
    }

    /// The square root of the interval's non-negative part: [`Interval::sqrt`] of
    /// `[max(lo, 0), hi]`. Private, for [`Interval::norm`] alone.
    ///
    /// The result contains the square root of every non-negative real in the interval. It says
    /// nothing about negative reals, which have no real root, so it certifies the root of the exact
    /// value only when that value is not negative, and this function cannot check that: a lower
    /// bound below zero may come from widening alone or from a value that really is negative. When
    /// the value is negative the result is not a certificate and nothing says so: for example
    /// `point(0).sub(point(2^−1074))` is [−2^−1073, 0], whose exact value is −2^−1074, and this
    /// function gives [0, 0], whose `certain_sign` is `Zero`. So it is not public. Its one caller,
    /// [`Interval::norm`], passes it a sum of squares, whose exact value is never negative.
    /// [`Interval::sqrt`] stays the public, strict form, which refuses any interval with a
    /// negative value in it.
    ///
    /// Method: IEEE Std 1788-2015's `sqrt` in its set-based flavour, the root over the part of the
    /// operand inside the domain [0, +∞). The standard reports a cut domain with the decoration
    /// `trv`; this function does not, which is sound only because its caller has the proof.
    ///
    /// # Errors
    ///
    /// [`NumError::NegativeSqrt`] if `hi < 0`: every real in the interval is negative, so the exact
    /// value cannot be the sum of squares its caller passes.
    fn sqrt_nonneg(self) -> Result<Interval, NumError> {
        if self.hi < 0.0 {
            return Err(NumError::NegativeSqrt);
        }
        let lo = if self.lo < 0.0 { 0.0 } else { self.lo };
        Interval { lo, hi: self.hi }.sqrt()
    }

    /// The Euclidean length √(c₁² + … + cₙ²) of a vector whose components lie in the given
    /// intervals, such as the coordinate differences of two points; exactly [0, 0] for no
    /// components.
    ///
    /// Each component is squared with [`Interval::square`], the squares are summed in order with
    /// [`Interval::add`], and the root is taken of the sum's non-negative part (a private
    /// operation, `sqrt_nonneg`). The sum is a sum of squares, so its exact value is never
    /// negative, and it is this function, not its caller, that knows it: a lower bound that
    /// widening pushed below zero is cut at 0 with that proof in hand, which a root on a caller's
    /// word cannot have. For coincident points the
    /// result starts at exactly 0, holds 0 and has no certain sign, so the exact predicates decide
    /// whether the points coincide.
    ///
    /// Inclusion follows from the inclusion of each step. Each step is tight as the module states,
    /// but the steps compound: every square and every sum adds one rounding and one step to each
    /// bound before the root.
    ///
    /// Method: the natural interval extension of the length (Moore, Kearfott and Cloud), with the
    /// square and root above. There is no rescaling as in `hypot`: a component whose square, or a
    /// sum of squares, leaves the finite doubles is reported, not rescued.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a square or a partial sum has a widened bound that is not finite
    /// (a component of magnitude about 2^512 or more, or squares that together reach about 2^1024).
    /// Never [`NumError::NegativeSqrt`]: every square, and so every sum, has a positive upper
    /// bound.
    pub fn norm(components: &[Interval]) -> Result<Interval, NumError> {
        let Some((first, rest)) = components.split_first() else {
            return Ok(Interval { lo: 0.0, hi: 0.0 });
        };
        let mut sum = first.square()?;
        for component in rest {
            sum = sum.add(component.square()?)?;
        }
        sum.sqrt_nonneg()
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    //! The private root of the non-negative part, which only `norm` may use, and the fused
    //! multiply-add against exact rationals.

    use num_rational::BigRational;

    use super::Interval;
    use crate::error::NumError;

    /// The doubles where interval code goes wrong, both signs.
    const VALUES: [f64; 14] = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        3.0,
        -7.25,
        1e-300,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        -f64::from_bits(1),
        1.0 + f64::EPSILON,
        1e300,
        -1e300,
    ];

    /// Every interval with bounds from `VALUES`.
    fn intervals() -> Vec<Interval> {
        let mut out = Vec::new();
        for &lo in &VALUES {
            for &hi in &VALUES {
                if let Ok(x) = Interval::new(lo, hi) {
                    out.push(x);
                }
            }
        }
        out
    }

    fn exact(x: f64) -> BigRational {
        BigRational::from_float(x).unwrap()
    }

    /// `sqrt_nonneg` is `sqrt` of `[max(lo, 0), hi]`, bit for bit, and refuses only an interval
    /// with every value negative; `norm` of one component is it applied to the square.
    #[test]
    fn the_private_root_follows_its_method() {
        for x in intervals() {
            let got = x.sqrt_nonneg();
            if x.hi() < 0.0 {
                assert_eq!(got, Err(NumError::NegativeSqrt), "{x:?}");
            } else {
                let lo = if x.lo() < 0.0 { 0.0 } else { x.lo() };
                let want = Interval::new(lo, x.hi()).unwrap().sqrt();
                assert_eq!(got, want, "{x:?}");
            }
            if let Ok(square) = x.square() {
                assert_eq!(Interval::norm(&[x]), square.sqrt_nonneg(), "{x:?}");
            }
        }
    }

    /// Why the root of the non-negative part is private: it gives an exact zero for a value that
    /// is negative, and no public operation can now make that claim.
    #[test]
    fn the_private_root_would_certify_a_negative_value_as_zero() {
        let tiny = f64::from_bits(1);
        let d = Interval::point(0.0)
            .unwrap()
            .sub(Interval::point(tiny).unwrap())
            .unwrap();
        assert_eq!(d.lo().to_bits(), (-2.0 * tiny).to_bits());
        let cut = d.sqrt_nonneg().unwrap();
        assert_eq!((cut.lo(), cut.hi()), (0.0, 0.0));
        assert_eq!(d.sqrt(), Err(NumError::NegativeSqrt));
    }

    /// `mul_add` holds the exact x · y + z at every corner of every box from `VALUES`, its bounds
    /// are within two doubles of the exact extremes (the first or second double beyond them), it
    /// is never wider than `mul` then `add`, and an overflow is `NotFinite`.
    #[test]
    fn mul_add_encloses_every_corner() {
        let boxes = intervals();
        let cs = [
            Interval::new(-1.0, 1.0).unwrap(),
            Interval::point(-1.0).unwrap(),
            Interval::new(1e-300, 3.0).unwrap(),
            Interval::point(1e300).unwrap(),
        ];
        let mut checked = 0;
        for &a in &boxes {
            for &b in boxes.iter().step_by(7) {
                for &c in &cs {
                    let Ok(r) = a.mul_add(b, c) else {
                        assert_eq!(a.mul_add(b, c), Err(NumError::NotFinite));
                        continue;
                    };
                    let mut corners = Vec::new();
                    for x in [a.lo(), a.hi()] {
                        for y in [b.lo(), b.hi()] {
                            for z in [c.lo(), c.hi()] {
                                corners.push(exact(x) * exact(y) + exact(z));
                            }
                        }
                    }
                    let least = corners.iter().min().unwrap().clone();
                    let most = corners.iter().max().unwrap().clone();
                    assert!(
                        exact(r.lo()) <= least && most <= exact(r.hi()),
                        "{a:?} {b:?} {c:?}"
                    );
                    // At most two doubles beyond: the second double beyond is still beyond.
                    let (lo2, hi2) = (r.lo().next_up().next_up(), r.hi().next_down().next_down());
                    assert!(exact(lo2) > least, "{a:?} {b:?} {c:?}: lower bound too low");
                    assert!(exact(hi2) < most, "{a:?} {b:?} {c:?}: upper bound too high");
                    if let Ok(two_steps) = a.mul(b).and_then(|p| p.add(c)) {
                        assert!(two_steps.lo() <= r.lo() && r.hi() <= two_steps.hi());
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked > 1000, "{checked}");
        let huge = Interval::point(f64::MAX).unwrap();
        assert_eq!(huge.mul_add(huge, huge), Err(NumError::NotFinite));
        assert_eq!(huge.neg().mul_add(huge, huge), Err(NumError::NotFinite));
    }

    /// The fused multiply-add keeps what two roundings lose: (1 + 2^−52)(1 − 2^−52) − 1 is −2^−104.
    #[test]
    fn mul_add_rounds_once() {
        let e = f64::EPSILON;
        let x = Interval::point(1.0 + e).unwrap();
        let y = Interval::point(1.0 - e).unwrap();
        let r = x.mul_add(y, Interval::point(-1.0).unwrap()).unwrap();
        assert!(r.hi() < 0.0, "{r:?}");
        assert!(r.contains(-(e * e)));
    }

    /// The functions whose error `libm` does not prove refuse every interval.
    #[test]
    fn unproven_functions_are_refused() {
        for x in intervals() {
            assert_eq!(x.sin(), Err(NumError::UnprovenMaths));
            assert_eq!(x.cos(), Err(NumError::UnprovenMaths));
            assert_eq!(x.atan2(x), Err(NumError::UnprovenMaths));
            assert_eq!(x.exp(), Err(NumError::UnprovenMaths));
            assert_eq!(x.ln(), Err(NumError::UnprovenMaths));
        }
    }
}
