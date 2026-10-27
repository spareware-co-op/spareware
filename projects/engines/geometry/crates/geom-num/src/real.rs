//! `Real`: a floating-point number that is never NaN and never infinite.
//!
//! NaN's sign and payload are not deterministic across platforms, even in WebAssembly (Rust RFC
//! 3514), so the kernel never produces one (chapter 21). A `Real` can only be made by
//! [`Real::new`], which refuses NaN and the infinities, so every `Real` is finite: the invariant is
//! kept by construction, not by a later check.
//!
//! Floating-point values leave this crate as `Real`s ([`crate::math`]'s results,
//! [`crate::exact_sum::ExactSum::round`] and [`crate::ladder::ANGULAR`]), with these exceptions,
//! each finite or reported by its own type: an [`crate::interval::Interval`]'s bounds, which its
//! invariant keeps finite; a [`crate::dd::DD`]'s parts and its rounded value
//! ([`crate::dd::DD::hi`], [`crate::dd::DD::lo`], [`crate::dd::DD::to_f64`]), which its invariant
//! keeps finite; the pairs of [`crate::eft`] and [`crate::dd::two_prod_fma`], whose second part is
//! an infinity exactly when no exact pair exists, which is their report; the constants
//! [`crate::eft::SPLITTER`] and [`crate::dd`]'s error bounds; and
//! [`crate::exact_sum::ExactSum::round_ieee`], which returns IEEE 754's sum of what was added, so it
//! is NaN only when a NaN, or infinities of both signs, were added, and infinite only when an
//! infinity was added or the exact sum overflows. Kernel code takes `round`, never `round_ieee`.
//!
//! Equality and order are both IEEE 754's totalOrder (`f64::total_cmp`), so they agree: −0 sorts
//! before +0 and the two are not equal, and two `Real`s are equal exactly when their bits are. So
//! sorting, removing duplicates, ordered maps and binary search all treat −0 and +0 as the
//! different values they are, the same way on every target.
//!
//! Guarantees:
//!
//! - Every `Real` is finite: [`Real::new`] is the only way to make one, and it refuses NaN and the
//!   infinities.
//! - Equality and order are both IEEE 754's `totalOrder`, so they agree, and −0 and +0 are
//!   different values, on every target.
//! - [`Real::total_max`] and [`Real::total_min`] have named tie rules: they pick by `totalOrder`,
//!   so −0 is below +0, and on a tie, which under `totalOrder` is always the same bits, they return
//!   the first argument.
//!
//! Method: IEEE 754-2019's `totalOrder` predicate (section 5.10,
//! <https://doi.org/10.1109/IEEESTD.2019.8766229>), through `f64::total_cmp`; never producing NaN
//! follows Rust RFC 3514 (<https://rust-lang.github.io/rfcs/3514-float-semantics.html>).
//!
//! Cost: constant: one finiteness check to make a `Real`, and one comparison of bits to compare
//! two.

use core::cmp::Ordering;
use core::hash::{Hash, Hasher};

use crate::error::NumError;

/// A finite `f64`.
#[derive(Clone, Copy, Debug)]
pub struct Real(f64);

impl Real {
    /// Zero (+0).
    pub const ZERO: Real = Real(0.0);

    /// A `Real`, or `NotFinite` for NaN and infinities.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `v` is NaN or infinite.
    pub const fn new(v: f64) -> Result<Real, NumError> {
        if v.is_finite() {
            Ok(Real(v))
        } else {
            Err(NumError::NotFinite)
        }
    }

    /// The value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }

    /// The larger of two values in IEEE totalOrder (so +0 is larger than −0); deterministic, unlike
    /// `f64::max`, which may return either zero. Tie rule: on a tie, which under totalOrder is the
    /// same bits, `self`.
    #[must_use]
    pub fn total_max(self, other: Real) -> Real {
        if self.0.total_cmp(&other.0) == Ordering::Less {
            other
        } else {
            self
        }
    }

    /// The smaller of two values in IEEE totalOrder (so −0 is smaller than +0). Tie rule: on a tie,
    /// which under totalOrder is the same bits, `self`.
    #[must_use]
    pub fn total_min(self, other: Real) -> Real {
        if self.0.total_cmp(&other.0) == Ordering::Greater {
            other
        } else {
            self
        }
    }
}

/// Equal exactly when the bits are equal. For values that are not NaN, which every `Real` is, that
/// is exactly `total_cmp` giving `Equal`: totalOrder orders every double by its bits read as a
/// sign and a magnitude, so it calls two doubles equal only when their bits are. So `==` and
/// [`Ord`] agree, as `Eq` and `Ord` require; `f64`'s own `==`, which calls −0 and +0 equal, would
/// not.
impl PartialEq for Real {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for Real {}

/// Hashes the bits, so equal `Real`s hash equally.
impl Hash for Real {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl PartialOrd for Real {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Real {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// `x` with a zero made +0 and every other value unchanged: `x + 0`. For a nonzero `x` the exact
/// sum is `x`, a double, so it is returned as it is (an infinity or NaN stays one too). For `x` =
/// ±0 the sum is an exact zero, which IEEE 754-2019, section 6.3, makes +0 under round to nearest
/// whatever the sign of `x`: (−0) + (+0) has operands of opposite signs, and (+0) + (+0) keeps the
/// sign of +0. Rust never folds `x + 0.0` into `x`, since that would change −0 (RFC 3514).
///
/// The one place this proof is written: [`crate::eft`] and [`crate::interval`] both use it, so
/// that a zero they return has one set of bits whatever its history.
pub(crate) const fn zero_to_plus(x: f64) -> f64 {
    x + 0.0
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
    use super::*;

    #[test]
    fn rejects_non_finite_and_orders_zeros() {
        assert_eq!(Real::new(f64::NAN), Err(NumError::NotFinite));
        assert_eq!(Real::new(f64::INFINITY), Err(NumError::NotFinite));
        assert_eq!(Real::new(f64::NEG_INFINITY), Err(NumError::NotFinite));
        let pz = Real::new(0.0).unwrap();
        let nz = Real::new(-0.0).unwrap();
        assert!(nz < pz);
        assert_eq!(pz.total_max(nz).get().to_bits(), 0.0f64.to_bits());
        assert_eq!(nz.total_max(pz).get().to_bits(), 0.0f64.to_bits());
        assert_eq!(pz.total_min(nz).get().to_bits(), (-0.0f64).to_bits());
        assert_eq!(nz.total_min(pz).get().to_bits(), (-0.0f64).to_bits());
    }

    /// −0 and +0 are different `Real`s, and `==` holds exactly when `cmp` gives `Equal`, on every
    /// pair of a set of values with both zeros, both signs, subnormals and the extremes.
    #[test]
    fn equality_agrees_with_the_order() {
        let values = [
            0.0,
            -0.0,
            1.0,
            -1.0,
            f64::from_bits(1),
            -f64::from_bits(1),
            f64::MIN_POSITIVE,
            f64::MAX,
            f64::MIN,
            0.1,
            0.1 + 0.2,
        ];
        let reals: Vec<Real> = values.iter().map(|&v| Real::new(v).unwrap()).collect();
        assert_ne!(Real::new(0.0), Real::new(-0.0));
        for a in &reals {
            for b in &reals {
                assert_eq!(a == b, a.cmp(b) == Ordering::Equal, "{a:?} {b:?}");
                assert_eq!(
                    a == b,
                    a.get().to_bits() == b.get().to_bits(),
                    "{a:?} {b:?}"
                );
            }
        }
        let mut sorted = reals.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), reals.len(), "no two of the values are equal");
    }

    /// `zero_to_plus` turns −0 into +0 and leaves every other value, infinities included, as it is.
    #[test]
    fn zero_to_plus_changes_only_negative_zero() {
        assert_eq!(zero_to_plus(-0.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(zero_to_plus(0.0).to_bits(), 0.0f64.to_bits());
        for v in [
            1.0,
            -1.0,
            f64::from_bits(1),
            -f64::MAX,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            assert_eq!(zero_to_plus(v).to_bits(), v.to_bits());
        }
    }
}

/// Bounded proofs (chapter 28.7; piece G41): Kani checks every double, bit for bit.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// `Real::new` accepts exactly the finite doubles, keeping their bits, and refuses NaN and
    /// both infinities, for every one of the 2^64 bit patterns: no NaN ever gets through.
    #[kani::proof]
    fn real_new_refuses_exactly_nan_and_the_infinities() {
        let bits: u64 = kani::any();
        let x = f64::from_bits(bits);
        // Finite exactly when the exponent field is not all ones (IEEE 754 binary64).
        let finite = (bits >> 52) & 0x7ff != 0x7ff;
        match Real::new(x) {
            Ok(r) => assert!(finite && r.get().to_bits() == bits),
            Err(e) => assert!(!finite && e == NumError::NotFinite),
        }
    }

    /// `total_max` and `total_min` pick by totalOrder, on a tie (the same bits) the first
    /// argument, and `Ord` agrees with `==`, for every pair of finite doubles.
    #[kani::proof]
    fn total_order_and_its_tie_rules() {
        let (a, b): (u64, u64) = (kani::any(), kani::any());
        let (Ok(x), Ok(y)) = (Real::new(f64::from_bits(a)), Real::new(f64::from_bits(b))) else {
            return;
        };
        let order = x.cmp(&y);
        assert!((order == Ordering::Equal) == (a == b));
        let max = x.total_max(y);
        let min = x.total_min(y);
        assert!(max >= x && max >= y && min <= x && min <= y);
        assert!((max == x || max == y) && (min == x || min == y));
        if a == b {
            assert!(max.get().to_bits() == a && min.get().to_bits() == a);
        }
    }
}
