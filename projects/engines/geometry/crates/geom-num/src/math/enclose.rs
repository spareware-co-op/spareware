//! Interval versions of the proven maths functions: [`Interval::sinpi`], [`Interval::cospi`],
//! [`Interval::exp`], [`Interval::ln`] and [`Interval::atan2`], certified enclosures for curved
//! computations (chapters 12.3 and 16.2; piece G47's "interval versions with exact peak tests").
//!
//! Method: the interval extension of a standard function by monotonicity and its critical
//! points, as in Moore, Kearfott and Cloud, *Introduction to Interval Analysis*, SIAM 2009,
//! §5.1 (<https://doi.org/10.1137/1.9780898717716>): on a stretch where a function is monotone its
//! extremes are at the ends, so only the ends are evaluated, and every critical point inside is
//! added exactly. Each end is evaluated with the correctly rounded point function of
//! [`crate::math`] and widened by one step outward with `next_down` and `next_up`, as
//! [`crate::interval`] does after every operation. `atan2` over a box takes the image of the
//! box's part inside its domain, the plane without the origin, as IEEE 1788-2015's `atan2` does
//! (§9.1, table 9.1: domain `R² \ {(0, 0)}`, range `(−π, π]`;
//! <https://doi.org/10.1109/IEEESTD.2015.7140721>).
//!
//! Cost: constant time and no allocation: the point function at each of the two ends (one or two
//! calls of the correctly rounded `sinpi`, `cospi`, `exp` or `ln`), two widening steps, and for
//! the periodic two a fixed number of exact integer comparisons for the peak tests; `atan2` at
//! most four calls of the correctly rounded `atan2`, one per corner of the box, and none when the
//! box straddles the branch cut.
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
//! - **Inclusion for `atan2`.** The result contains the angle `θ(y, x) ∈ (−π, π]` of every real
//!   point `(x, y) ≠ (0, 0)` of the box, a zero `y` being the real 0, so a point on the negative
//!   x-axis has the angle π. *Proof.* If the box has a point with `x < 0` and points with `y < 0`
//!   and `y ≥ 0` (`x.lo < 0`, `y.lo < 0 ≤ y.hi`), it holds points of the negative x-axis, angle
//!   π, and points just below it, whose angles approach −π, so the hull of its image is
//!   `[−π, π]`, which the result `[−next_up(π̃), next_up(π̃)]` contains (`π̃ = RN(π) < π <
//!   next_up(π̃)`). Otherwise the box lies in the closed right half-plane (`x ≥ 0`), the closed
//!   upper one (`y ≥ 0`) or the open lower one (`y < 0`), where `θ` is continuous away from the
//!   origin: the branch cut, the negative x-axis, is outside the first and the third, and on the
//!   boundary of the second, where `θ = π` is the limit from above. Each of the three is a
//!   half-plane whose bounding line passes through the origin, so along a segment inside it that
//!   misses the origin the direction turns one way only and `θ` is monotone, and along a segment
//!   through the origin `θ` is constant on each side. So on each edge of the box `θ` takes its
//!   extremes at the edge's ends that are not the origin, and a point inside has the angle of the
//!   point where the ray from the origin through it leaves the box. The extremes over the box
//!   without the origin are therefore at its corners other than the origin. Each corner's angle is
//!   evaluated faithfully and widened as above; it is exact only at `θ = 0`, a corner on the
//!   positive x-axis, since every other angle of a point with double coordinates is irrational
//!   (`tan θ = y/x` is rational, and by Lindemann–Weierstrass a nonzero algebraic `θ` has a
//!   transcendental tangent, so `θ` is never a nonzero double).
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
//!   [`NumError::NotFinite`] when its lower bound is zero, where the logarithm is `−∞`; `atan2`
//!   fails with [`NumError::OutsideDomain`] when the box is the origin alone, which has no angle.
//! - **Deterministic.** Only the correctly rounded functions of [`crate::math`] (their ports, on
//!   the finite bounds of an interval, which the public functions would accept unchanged), IEEE
//!   754 basic operations, comparisons, `rint` (exact), `next_up` and `next_down`.
//!
//! Proven and tested: the inclusions above are proven here, on the proven faithfulness of the
//! point functions (the module documentation of [`crate::math`]); Kani does not reach them, since
//! the ports' tables and polynomials are beyond a bounded proof. The tests check every inclusion
//! against the big-number reference of `tests/math_reference.rs` on seeded boxes and at the
//! peaks, axes, branch cut and origin (`tests/attack_g47.rs`).

use super::{atan2, exp, log, sincospi};
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
    /// Always inline: its two callers are [`periodic`]'s copies for sinpi and cospi, and left out
    /// of line it costs every call a call. G41's count gates caught LLVM leaving it out of line on
    /// aarch64, then (with `#[inline]`) on x86-64 after an unrelated merge: sinpi and cospi 4% to
    /// 7% more instructions each time. A hint is a size margin; this is not.
    #[allow(
        clippy::inline_always,
        reason = "measured: the hint alone left it out of line on one target or the other"
    )]
    #[inline(always)]
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

/// `next_up(RN(π))`, the least double above π: the bounds of every angle, as `±PI_UP`.
const PI_UP: f64 = core::f64::consts::PI.next_up();

/// The enclosure `[low, high]` of the angle of the corner `(x, y)` of a box, or `None` for the
/// origin, which has no angle. The angle is exactly 0 on the positive x-axis (a zero `y` is +0,
/// an interval's zero) and irrational everywhere else (the module documentation), so every other
/// corner's correctly rounded angle is widened by one step.
fn corner(y: f64, x: f64) -> Option<(f64, f64)> {
    if y == 0.0 && x >= 0.0 {
        return (x > 0.0).then_some((0.0, 0.0));
    }
    Some(around(atan2::atan2(y, x), false))
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
/// gate), and `#[inline]` did not bring it back. (Splitting it so the generic part only called the
/// point function cost each call about 25 instructions more on x86-64: the count gate again.)
///
/// Always inline into its one caller per copy: G47's x86-64 count gate found both copies out of
/// line, `Interval::sinpi` and `cospi` a 39-instruction call around them, 7% to 9% more
/// instructions, after `math::fma` became always inline, an edit to neither function.
#[allow(
    clippy::inline_always,
    reason = "measured: left to LLVM it went out of line after an unrelated edit, on x86-64"
)]
#[inline(always)]
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

    /// An interval containing the angle of the point `(x, y)`, in radians in `(−π, π]`, for every
    /// real point of the box with `y` in this interval and `x` in `x`, other than the origin.
    ///
    /// The bounds are reals, a zero bound the real 0, so the points of the negative x-axis have
    /// the angle π, the top of the range (IEEE 1788-2015's `atan2`). A box that holds them and
    /// points below them (`x.lo < 0` and `y.lo < 0 ≤ y.hi`) has angles from just above −π up to
    /// π, and its result is `[−π, π]`, rounded outward; so is that of a box with the origin
    /// inside it. Every other box takes its extremes at its corners other than the origin: each
    /// corner's angle is the correctly rounded [`crate::math::atan2()`], widened by one step
    /// outward, or exactly 0 on the positive x-axis. See the module documentation for the proof.
    ///
    /// The point function keeps IEEE 754's sign of zero, so [`crate::math::atan2()`] of a point
    /// with `y = −0` and `x < 0` is −π, the other side of the cut; a box whose lower `y` bound is
    /// 0 holds the real 0 only, whose angle there is π.
    ///
    /// # Errors
    ///
    /// [`NumError::OutsideDomain`] if both intervals are exactly zero: the box is the origin
    /// alone, which has no angle (IEEE 1788-2015 gives the empty set there).
    pub fn atan2(self, x: Interval) -> Result<Interval, NumError> {
        let (y_lo, y_hi) = (self.lo(), self.hi());
        let (x_lo, x_hi) = (x.lo(), x.hi());
        if x_lo < 0.0 && y_lo < 0.0 && y_hi >= 0.0 {
            return Interval::new(-PI_UP, PI_UP);
        }
        let (low, high) = [(y_lo, x_lo), (y_lo, x_hi), (y_hi, x_lo), (y_hi, x_hi)]
            .into_iter()
            .filter_map(|(y, x)| corner(y, x))
            .reduce(|(l, h), (m, k)| (min2(l, m), max2(h, k)))
            .ok_or(NumError::OutsideDomain)?;
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
