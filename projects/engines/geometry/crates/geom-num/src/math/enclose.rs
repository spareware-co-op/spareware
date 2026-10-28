//! Interval versions of the proven maths functions: [`Interval::sinpi`], [`Interval::cospi`],
//! [`Interval::exp`] and [`Interval::ln`], certified enclosures for curved computations
//! (chapters 12.3 and 16.2; piece G47's "interval versions with exact peak tests").
//!
//! Method: the interval extension of a standard function by monotonicity and its critical
//! points, as in Moore, Kearfott and Cloud, *Introduction to Interval Analysis*, SIAM 2009,
//! §5.1 (<https://doi.org/10.1137/1.9780898717716>): on a stretch where a function is monotone its
//! extremes are at the ends, so only the ends are evaluated, and every critical point inside is
//! added exactly. Each end is evaluated with the correctly rounded point function of
//! [`crate::math`] and widened by one step outward with `next_down` and `next_up`, as
//! [`crate::interval`] does after every operation.
//!
//! Cost: constant time and no allocation: the point function at each of the two ends (one or two
//! calls of the correctly rounded `sinpi`, `cospi`, `exp` or `ln`), two widening steps, and for
//! the periodic two a fixed number of exact integer comparisons for the peak tests.
//!
//! Guarantees:
//!
//! - **Inclusion.** The result contains `f(t)` for every real `t` in the interval, not only for the
//!   doubles in it. *Proof.* `exp` and `ln` are increasing, so their image is `[f(lo), f(hi)]`.
//!   `sin(πt)` is monotone between consecutive half-integers, where its derivative `π·cos(πt)`
//!   vanishes; its maxima (value 1) are at `t ≡ ½` and its minima (−1) at `t ≡ −½ (mod 2)`. So
//!   its image is the hull of the two ends' values and of ±1 for every maximum or minimum inside;
//!   `cos(πt) = sin(π(t + ½))` likewise, with maxima at even and minima at odd integers. A
//!   correctly rounded result `r` of an exact value `v` satisfies `next_down(r) ≤ v ≤ next_up(r)`
//!   (round to nearest is faithful), so the widened ends enclose the exact ends. Where an end is
//!   exact (a multiple of ½ for `sinpi` and `cospi`, whose value is then exactly 0 or ±1; `exp(0)`;
//!   `ln(1)`), it is used without widening. Values of `sinpi` and `cospi` are clamped to `[−1, 1]`,
//!   and the lower bound of `exp` to `+0`, which the exact functions never leave.
//! - **Exact peak tests.** Whether the interval contains a maximum or a minimum is decided exactly,
//!   with no rounding, by [`Arc`]: the peaks of periodic fields fall at exact half-integers of
//!   their phase (chapter 16.2), and the test never misses one or invents one.
//! - **Tight.** Each bound is at most two floating-point steps outside the exact bound: the
//!   correctly rounded value plus one widening step.
//! - **Never NaN.** The results are [`Interval`]s, finite and ordered, with a zero bound stored as
//!   +0 ([`Interval::new`]); a function outside its domain returns an error, never a NaN (chapter
//!   21), with the errors of the point functions: `exp` fails with [`NumError::NotFinite`] when
//!   its upper bound overflows; `ln` fails with [`NumError::OutsideDomain`] when the interval
//!   holds a negative number, where the logarithm is not a real number, and with
//!   [`NumError::NotFinite`] when its lower bound is zero, where the logarithm is `−∞`.
//! - **Deterministic.** Only the correctly rounded functions of [`crate::math`] (their ports, on
//!   the finite bounds of an interval, which the public functions would accept unchanged), IEEE
//!   754 basic operations, comparisons, `rint` (exact), `next_up` and `next_down`.
//!
//! These are the four that chapter 16.2 names. An interval `atan2` is not provided here; its point
//! function's bound is proven (see [`crate::math::atan2()`]), so one can follow the same method: the
//! extremes over a box that avoids the origin are at its corners, except across the branch cut on
//! the negative x-axis.

use super::{exp, log, sincospi};
use crate::eft::two_sum;
use crate::error::NumError;
use crate::interval::{Interval, max2, min2};

/// Whether the real number `s + e` is at least the double `p`, where `(s, e)` is an exact two-sum
/// pair (`s` is the rounded sum, so `s = RN(s + e)`). Exact.
///
/// *Proof.* Rounding to nearest is monotone and leaves doubles fixed. If `s + e < p` then
/// `s = RN(s + e) ≤ RN(p) = p`; so `s > p` implies `s + e ≥ p`. Likewise `s < p` implies
/// `s + e < p`. If `s = p`, then `s + e ≥ p` exactly when `e ≥ 0`. An infinite `s` (an overflowed
/// sum) is `+∞` here and correctly reads as "at least `p`".
fn at_least(s: f64, e: f64, p: f64) -> bool {
    s > p || (s == p && e >= 0.0)
}

/// An interval of half-turns `[lo, hi]` placed on the circle, so that whether it contains a given
/// point `p (mod 2)` is an exact test.
///
/// With `2k` the even integer nearest `lo`, the interval is `2k + [r, r + d]` with `r = lo − 2k` in
/// `[−1, 1]` and `d = hi − lo`. If `d ≥ 2` it covers a whole turn and contains every point. If
/// `d < 2`, then `[r, r + d] ⊂ [−1, 3)`, so it contains a point `≡ p (mod 2)` if and only if it
/// contains `2k + p` for one of the representatives `p` of that point in `[−1, 3)`, which the
/// callers list.
///
/// Every quantity is exact. `rint(lo/2)` is the integer nearest `lo/2` (if `lo/2` rounds, `lo` is
/// subnormal and the nearest integer is 0 either way) and `2k` is exact. `r = lo − 2k` is exact:
/// for `k = 0` it is `lo`, and otherwise `lo` and `2k` have the same sign with
/// `|lo − 2k| ≤ 1 ≤ |k|`, so `|2k|/2 ≤ |lo| ≤ 2·|2k|` and Sterbenz's lemma applies. `hi − lo` and
/// `hi − 2k` are kept as exact two-sum pairs and compared with [`at_least`]; `hi − 2k = r + d < 3`
/// cannot overflow.
struct Arc {
    /// Whether the interval is a whole turn or more (`hi − lo ≥ 2`).
    full: bool,
    /// `lo − 2k`, exact, in `[−1, 1]`.
    r: f64,
    /// `hi − 2k` as an exact two-sum pair (only used when `full` is false).
    top: (f64, f64),
}

impl Arc {
    /// The arc of `[lo, hi]`, for finite `lo ≤ hi`.
    ///
    /// Inline: its two callers are [`periodic`]'s copies for sinpi and cospi, and out of line it
    /// costs each call a call (on aarch64, where LLVM left it out of line once `periodic` became
    /// generic: G41's count gate measured sinpi and cospi 6% and 7% more instructions).
    #[inline]
    fn new(lo: f64, hi: f64) -> Arc {
        let (ds, de) = two_sum(hi, -lo);
        let full = at_least(ds, de, 2.0);
        let two_k = 2.0 * libm::rint(lo * 0.5);
        let r = lo - two_k;
        let top = if full {
            (0.0, 0.0)
        } else {
            two_sum(hi, -two_k)
        };
        Arc { full, r, top }
    }

    /// Whether the interval contains a point `≡ p (mod 2)`, given every representative `p` of
    /// that point in `[−1, 3)`.
    fn contains_any(&self, representatives: &[f64]) -> bool {
        self.full
            || representatives
                .iter()
                .any(|&p| self.r <= p && at_least(self.top.0, self.top.1, p))
    }
}

/// Whether `t` is a multiple of ½, where `sinpi` and `cospi` are exact (0 or ±1).
///
/// For `|t| < 2^52`, `2t` is exact and must be a whole number; above, every double is a whole
/// number.
fn quarter_turn(t: f64) -> bool {
    const TWO_52: f64 = 4_503_599_627_370_496.0;
    if t.abs() >= TWO_52 {
        return true;
    }
    let w = 2.0 * t;
    w == libm::rint(w)
}

/// The enclosure `[low, high]` of the exact value whose correctly rounded value is `r`: `r` itself
/// when the value is exact, otherwise one step either side.
fn around(r: f64, exact: bool) -> (f64, f64) {
    if exact {
        (r, r)
    } else {
        (r.next_down(), r.next_up())
    }
}

/// The enclosure of a periodic function of half-turns, `f(t) = sin(π(t + shift))`, given its
/// correctly rounded point version (a port, called on the interval's finite bounds) and the
/// representatives in `[−1, 3)` of its maxima and minima. The bounds are compared with
/// [`min2`] and [`max2`], comparisons that may return either zero, which [`Interval::new`] makes
/// +0.
///
/// Generic over the point function, so [`Interval::sinpi`] and [`Interval::cospi`] each get their
/// own copy with one caller and a direct call to it. Taking a `fn` pointer, it was one function
/// with two callers, and whether LLVM inlined it (and so made the pointer a direct call) turned on
/// size margins: an unrelated edit elsewhere in the crate (G41's) left it out of line on aarch64,
/// where sinpi and cospi on extreme inputs then cost 5.4% and 6.2% more instructions (G41's count
/// gate), and `#[inline]` did not bring it back.
fn periodic(
    x: Interval,
    f: impl Fn(f64) -> f64,
    maxima: &[f64],
    minima: &[f64],
) -> Result<Interval, NumError> {
    let (lo, hi) = (x.lo(), x.hi());
    let (a_low, a_high) = around(f(lo), quarter_turn(lo));
    let (b_low, b_high) = around(f(hi), quarter_turn(hi));
    let arc = Arc::new(lo, hi);
    let low = if arc.contains_any(minima) {
        -1.0
    } else {
        max2(min2(a_low, b_low), -1.0)
    };
    let high = if arc.contains_any(maxima) {
        1.0
    } else {
        min2(max2(a_high, b_high), 1.0)
    };
    Interval::new(low, high)
}

impl Interval {
    /// An interval containing `sin(πt)` for every real `t` in this interval (`t` in half-turns).
    ///
    /// Exact at the peaks: if the interval contains a point `≡ ½ (mod 2)` the upper bound is
    /// exactly 1, and if it contains a point `≡ −½ (mod 2)` the lower bound is exactly −1, decided
    /// with no rounding. An end that is a multiple of ½ contributes its exact value (0 or ±1);
    /// any other end contributes its correctly rounded value widened by one step. See the module
    /// documentation for the proof.
    ///
    /// # Errors
    ///
    /// None in practice: every bound is finite and in `[−1, 1]`. The `Result` is that of
    /// [`Interval::new`], which builds the answer.
    pub fn sinpi(self) -> Result<Interval, NumError> {
        periodic(self, sincospi::sinpi, &[0.5, 2.5], &[-0.5, 1.5])
    }

    /// An interval containing `cos(πt)` for every real `t` in this interval (`t` in half-turns).
    ///
    /// Exact at the peaks: the upper bound is exactly 1 if the interval contains an even integer,
    /// and the lower bound exactly −1 if it contains an odd one. Ends as for
    /// [`Interval::sinpi`].
    ///
    /// # Errors
    ///
    /// None in practice, as for [`Interval::sinpi`].
    pub fn cospi(self) -> Result<Interval, NumError> {
        periodic(self, sincospi::cospi, &[0.0, 2.0], &[-1.0, 1.0])
    }

    /// An interval containing `e^t` for every real `t` in this interval: `[e^lo, e^hi]`, each end
    /// correctly rounded and widened by one step (exact at `t = 0`), the lower bound at least +0.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `e^hi` overflows (`hi ≥ 0x1.62e42fefa39fp+9`, about 709.78).
    pub fn exp(self) -> Result<Interval, NumError> {
        let (low, _) = around(exp::exp(self.lo()), self.lo() == 0.0);
        let (_, high) = around(exp::exp(self.hi()), self.hi() == 0.0);
        Interval::new(max2(low, 0.0), high)
    }

    /// An interval containing `ln t` for every real `t` in this interval: `[ln lo, ln hi]`, each
    /// end correctly rounded and widened by one step (exact at `t = 1`).
    ///
    /// # Errors
    ///
    /// [`NumError::OutsideDomain`] if the interval contains a negative number (`lo < 0`), where
    /// the logarithm is not a real number; [`NumError::NotFinite`] if its lower bound is zero,
    /// where the logarithm is `−∞`. These are the errors of [`crate::math::ln`] at `lo`.
    pub fn ln(self) -> Result<Interval, NumError> {
        if self.lo() < 0.0 {
            return Err(NumError::OutsideDomain);
        }
        if self.lo() == 0.0 {
            return Err(NumError::NotFinite);
        }
        let (low, _) = around(log::ln(self.lo()), self.lo() == 1.0);
        let (_, high) = around(log::ln(self.hi()), self.hi() == 1.0);
        Interval::new(low, high)
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
    use super::*;

    fn iv(lo: f64, hi: f64) -> Interval {
        Interval::new(lo, hi).unwrap()
    }

    #[test]
    fn peaks_are_exact_and_found_at_every_scale() {
        // A point interval at a peak is the exact value.
        assert_eq!(iv(0.5, 0.5).sinpi().unwrap(), iv(1.0, 1.0));
        assert_eq!(iv(1.5, 1.5).sinpi().unwrap(), iv(-1.0, -1.0));
        assert_eq!(iv(1.0, 1.0).cospi().unwrap(), iv(-1.0, -1.0));
        assert_eq!(iv(2.0, 2.0).sinpi().unwrap(), iv(0.0, 0.0));
        // Peaks between doubles: at 2^53 the doubles are even integers, but the real interval
        // [2^53, 2^53 + 2] holds a whole turn.
        let big = 9_007_199_254_740_992.0;
        assert_eq!(iv(big, big + 2.0).sinpi().unwrap(), iv(-1.0, 1.0));
        // [2^52, 2^52 + 1] holds the half-integer 2^52 + ½, a maximum (2^52 is even).
        let b52 = 4_503_599_627_370_496.0;
        let s = iv(b52, b52 + 1.0).sinpi().unwrap();
        assert_eq!((s.lo(), s.hi()), (0.0, 1.0));
        // Short of a peak: no peak inside, so the upper bound comes from the end.
        let s = iv(0.25, 0.49).sinpi().unwrap();
        assert!(
            s.hi() < 1.0 && s.contains(sincospi::sinpi(0.25)) && s.contains(sincospi::sinpi(0.49))
        );
        // One double short of the peak: sinpi rounds to 1 there, and the bound is 1, not above.
        let below = 0.5f64.next_down();
        assert_eq!(iv(0.25, below).sinpi().unwrap().hi(), 1.0);
        // Touching a peak at the top end.
        assert_eq!(iv(0.25, 0.5).sinpi().unwrap().hi(), 1.0);
        // A negative arc crossing a minimum.
        assert_eq!(iv(-0.6, -0.4).sinpi().unwrap().lo(), -1.0);
    }

    #[test]
    fn exp_and_ln_are_exact_at_their_exact_points_and_refuse_outside() {
        assert_eq!(iv(0.0, 0.0).exp().unwrap(), iv(1.0, 1.0));
        assert_eq!(iv(1.0, 1.0).ln().unwrap(), iv(0.0, 0.0));
        assert_eq!(iv(0.0, 1.0).ln(), Err(NumError::NotFinite));
        assert_eq!(iv(-0.0, 1.0).ln(), Err(NumError::NotFinite));
        assert_eq!(iv(-1.0, 1.0).ln(), Err(NumError::OutsideDomain));
        assert_eq!(iv(-1.0, -0.5).ln(), Err(NumError::OutsideDomain));
        assert_eq!(iv(0.0, 710.0).exp(), Err(NumError::NotFinite));
        assert_eq!(iv(-1000.0, -999.0).exp().unwrap().lo(), 0.0);
    }
}
