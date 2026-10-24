//! Double-double arithmetic: a real number held as the unevaluated sum of two doubles.
//!
//! A [`DD`] `hi + lo` carries about 106 bits of significand. The kernel uses it once per value, to
//! convert survey and map data (binary `f64` from PROJ or a file) into a frame's grid (chapters 6.1
//! and 17.1). Every operation is built on the error-free transformations of [`geom_num::eft`]:
//! `two_sum` and `two_prod`.
//!
//! Representation invariant: both parts are finite and the pair is normalized, `hi` being `hi + lo`
//! rounded to nearest, so |lo| ≤ ulp(hi)/2 ≤ u·|hi| with u = 2^−53. Every operation ends with an
//! exact `two_sum`, which restores the invariant, and checks finiteness: an overflow returns
//! [`NumError::NotFinite`], never an infinity or a NaN. A zero trailing part made by `two_sum` or
//! `two_prod` is +0 (the numbers layer's error parts have one zero).
//!
//! The integer steps (the exponents of the scalings, and the integer parts of
//! [`DD::round_shifted`]) are checked methods, so nothing depends on overflow checks being on. The
//! exponents of finite doubles lie between −1074 and 1023, so none of their sums, differences or
//! negations can fail; one that did would be reported as [`NumError::BudgetExceeded`], never
//! answered.
//!
//! Guarantees, on accuracy: relative error bounds on the exact result of the operation applied to the exact
//! values of the operands (u = 2^−53). The middle column is proven on each method below, in full,
//! with every higher-order term kept; it is the bound the kernel relies on. The last column is the
//! tighter bound Joldes, Muller and Popescu prove for the same algorithm, as the authors state it
//! (see the sources; their journal text was not available to us).
//!
//! | Operation | Algorithm | Proven here | Reported by the authors |
//! | --- | --- | --- | --- |
//! | [`DD::add`], [`DD::sub`] | `AccurateDWPlusDW` | 6u² + 45u³ | 3u²/(1 − 4u) |
//! | [`DD::add_f64`] | `DWPlusFP` | 3u²/(1 − 3u) < 3u² + 10u³ | 2u² + 5u³ |
//! | [`DD::mul_f64`] | `DWTimesFP1` | 3u² + 7u³ | 2u² |
//! | [`DD::mul`] | `DWTimesDW1` | 8u² + 23u³ | 7u² |
//! | [`DD::div`] | QD's accurate division | 6u² + 79u³ | (not analysed there) |
//! | [`DD::sqrt`] | one Newton step from the rounded root | 4.13u² | (not analysed there) |
//!
//! All of these are far below the 2^−100 = 64u² that the frame conversions need (their own bound
//! is derived on [`crate::FrameRecord::to_frame`]). Negation, [`DD::from_i64`], [`DD::from_i128`],
//! [`DD::exact_product`] and [`DD::round_shifted`] are exact. The tests check every operation
//! against exact rationals, against both columns.
//!
//! **Facts the proofs use.** RN is rounding to nearest, ties to even; the proofs assume no rounding
//! falls below 2^−1022 (*Range* below says what happens when one does).
//!
//! - (R) RN(a) = a(1 + θ) with |θ| ≤ u, and |RN(a) − a| ≤ u·|RN(a)|.
//! - (N) For a normalized pair x = (`x_h`, `x_l)`: |`x_l`| ≤ `u|x_h`|, so (1 − `u)|x_h`| ≤ |x| ≤ (1 + `u)|x_h`|.
//! - (S) If `two_sum(a, b)` returns a nonzero error, then |a + b| ≥ max(|a|, |b|)/2, and so
//!   |RN(a + b)| ≥ max(|a|, |b|)/2. Proof: say |a| ≥ |b|. If a and b have the same sign,
//!   |a + b| ≥ |a|; if they have opposite signs and |b| < |a|/2, |a + b| > |a|/2; in the remaining
//!   case, |a|/2 ≤ |b| ≤ |a| with opposite signs, Sterbenz's lemma makes a + b exact, so the error
//!   is zero. RN is monotone and max(|a|, |b|)/2 is a double.
//!
//! The constants in the table are the exact values of each derivation's final expression at
//! u = 2^−53, rounded up (checked in rational arithmetic).
//!
//! **Range.** Additions never lose anything to underflow (a sum that lands below 2^−1022 is exact),
//! so [`DD::add`], [`DD::sub`] and [`DD::add_f64`] meet their bounds everywhere they do not
//! overflow. [`DD::mul_f64`] and [`DD::mul`] run as written when `two_prod` of the leading parts is
//! exact (both normal, both below 2^996, exponents summing to between −970 and 1021). Otherwise,
//! and always for [`DD::div`] and [`DD::sqrt`], the operands are first scaled by powers of two so
//! that their leading parts lie in [1, 2) ([1, 4) for the square root), the same algorithm runs
//! there, and the result is scaled back, each scaling one correctly rounded scaleB. So a product,
//! quotient or root of finite operands is never refused unless the result itself overflows.
//! Scaling up is exact; scaling an operand down is exact except for a trailing part that falls
//! below 2^−1022, which rounds by less than 2^−1074 relative to its operand; a product inside the
//! computation that falls below 2^−1022 rounds by at most 2^−1075 absolutely, which in the scaled
//! computation (results of magnitude at least 1/2) is at most 2^−1074 relative; scaling the result
//! back is exact unless a part falls below 2^−1022, which adds at most 2^−1075 per part. So every
//! multiplication, division and square root is within its bound in the table, plus 2^−1070
//! relative, plus 2^−1073 absolute, of the exact result. The extra terms matter only within a few
//! thousand ulps of the underflow threshold.
//!
//! Changes from the sources, and why:
//!
//! - Every `Fast2Sum` of the published algorithms is replaced by `two_sum`. Where the published
//!   proof shows `Fast2Sum` exact, both return the same pair (the rounded sum and its exact error are
//!   unique), so the published bounds carry over unchanged; `two_sum` needs no ordering
//!   precondition and cannot be misused. The proofs here use only that `two_sum` is exact.
//! - Products, quotients and roots scale their operands by powers of two, as above, so `two_prod`
//!   always runs where the exact error of its product is a double and its pair is exact (the
//!   numbers layer reports a product without an exact pair by an infinite error part, which that
//!   range rules out); the published algorithms assume an unbounded exponent.
//! - Every result is checked for finiteness instead of letting an infinity or NaN through.
//! - [`DD::round_shifted`] rounds the exact value `hi + lo − offset` to the nearest integer, ties to
//!   even, with no rounding error at all: the published libraries round through `f64` first.
//!
//! Method: the papers below, whose algorithms each operation follows.
//!
//! - T. J. Dekker, "A floating-point technique for extending the available precision",
//!   *Numerische Mathematik* 18 (1971), 224–242, <https://doi.org/10.1007/BF01397083>: the
//!   double-length representation and the addition and multiplication.
//! - Y. Hida, X. S. Li and D. H. Bailey, "Algorithms for quad-double precision floating point
//!   arithmetic", *15th IEEE Symposium on Computer Arithmetic* (2001), 155–162,
//!   <https://doi.org/10.1109/ARITH.2001.930115>: the QD library's forms, including its accurate
//!   division and its square root.
//! - M. Joldes, J.-M. Muller and V. Popescu, "Tight and rigorous error bounds for basic building
//!   blocks of double-word arithmetic", *ACM Transactions on Mathematical Software* 44(2) (2017),
//!   <https://doi.org/10.1145/3121432>: the algorithm names, and the tighter bounds in the table as
//!   the authors state them in their JNCF 2017 slides,
//!   <https://jncf2017.lip6.fr/files/slides/popescu.pdf>, slide 17 (read 2026-10-21), with the
//!   `AccurateDWPlusDW` bound as 3u²/(1 − 4u) in J.-M. Muller's NAC 2024 talk,
//!   <https://webusers.imj-prg.fr/~jean-claude.bajard/NAC2024/Supports/NAC2024_JMM.pdf>
//!   (read 2026-10-21). That talk also reports the Coq formalisation of these proofs by Muller and
//!   Rideau, *ACM TOMS* 48(1) (2022), <https://doi.org/10.1145/3484514>, which found and repaired
//!   flaws in some published proofs; the kernel therefore relies only on the bounds proven here.
//!
//! Cost: constant time, no loops, no allocation; each operation is a fixed sequence of IEEE
//! operations on `two_sum` and `two_prod`. [`DD::add`] is four `two_sum`s and two additions;
//! [`DD::add_f64`] two `two_sum`s and one addition; [`DD::mul_f64`] one `two_prod`, two `two_sum`s
//! and two more operations; [`DD::mul`] one `two_prod`, one `two_sum` and four more operations;
//! [`DD::div`] three divisions, two [`DD::mul_f64`]s, two [`DD::sub`]s, one `two_sum` and one
//! [`DD::add_f64`]; [`DD::sqrt`] one correctly rounded `geom_num::math::sqrt`, one `two_prod`, one
//! [`DD::sub`], one division and one `two_sum`. Operands outside `two_prod`'s range, and every
//! division and square root, add a power-of-two scaling of each part going in and of the result
//! coming out (`scale_b`: a bit decomposition and one or two multiplications). [`DD::round_shifted`]
//! is two roundings to integers, one `two_sum` and a few `i128` additions and comparisons.

#![allow(
    clippy::many_single_char_names,
    reason = "variable names follow the formulas of the method each function cites"
)]

use core::ops::Neg;

use geom_num::eft::{two_prod, two_sum};
use geom_num::{NumError, math};

use crate::error::budget;
use crate::exact::{const_pow2, exponent, scale_b};
use crate::rounded::Rounded;

/// The largest |hi| for which [`DD::round_shifted`] answers: 2^100, so the integer parts of both
/// halves fit an `i128` with room for an offset. Evaluated when the crate compiles.
const ROUND_LIMIT: f64 = const_pow2!(100);

/// 2^62: below it, [`whole`] converts through `i64`. Evaluated when the crate compiles.
const TWO_POW_62: f64 = const_pow2!(62);

/// 2^32, the weight of the high half of an `i64` in [`DD::from_i64`]. Evaluated when the crate
/// compiles.
const TWO_POW_32: f64 = const_pow2!(32);

/// A real number `hi + lo` held as two doubles, normalized (`hi` is the sum rounded to nearest) and
/// finite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DD {
    hi: f64,
    lo: f64,
}

// The arithmetic methods return `Result` (a result may overflow, a division may be by zero), so
// they are deliberately not the `core::ops` traits, whose callers expect a plain value.
impl DD {
    /// Zero.
    pub const ZERO: DD = DD { hi: 0.0, lo: 0.0 };

    /// The normalized pair for the exact sum `a + b`, or `NotFinite`.
    fn normalized(a: f64, b: f64) -> Result<DD, NumError> {
        let (hi, lo) = two_sum(a, b);
        if hi.is_finite() && lo.is_finite() {
            Ok(DD { hi, lo })
        } else {
            Err(NumError::NotFinite)
        }
    }

    /// The value times 2^k: each part scaled by [`scale_b`] (one rounding, exact while the part
    /// stays at or above 2^−1022), then normalized again; `NotFinite` if it overflows.
    fn scaled(self, k: i32) -> Result<DD, NumError> {
        DD::normalized(budget(scale_b(self.hi, k))?, budget(scale_b(self.lo, k))?)
    }

    /// The scaling 2^−e that brings a nonzero finite part's leading bit to 2^0: the negation of its
    /// exponent, from −1023 to 1074, which fits `i32`.
    fn unscaling(x: f64) -> Result<i32, NumError> {
        budget(exponent(x).and_then(i32::checked_neg))
    }

    /// Whether `two_prod(a, b)` is certainly exact: both factors normal, both below 2^996, and
    /// exponents summing to between −970 and 1021, where the product neither overflows nor has a
    /// bit below 2^−1074, so its exact error is a double. The exponents of normal doubles always
    /// exist and their sum always fits `i32`; were a step to fail, the answer is "not known to be
    /// in range", which sends the product down the scaled path, valid for every operand.
    fn product_in_range(a: f64, b: f64) -> bool {
        if !a.is_normal() || !b.is_normal() {
            return false;
        }
        let (Some(ea), Some(eb)) = (exponent(a), exponent(b)) else {
            return false;
        };
        ea < 996
            && eb < 996
            && ea
                .checked_add(eb)
                .is_some_and(|sum| (-970..=1021).contains(&sum))
    }

    /// The exact sum `a + b` of two doubles, normalized.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `a` or `b` is not finite, or the rounded sum overflows.
    pub fn from_sum(a: f64, b: f64) -> Result<DD, NumError> {
        if !a.is_finite() || !b.is_finite() {
            return Err(NumError::NotFinite);
        }
        DD::normalized(a, b)
    }

    /// A double, exactly.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `x` is NaN or infinite.
    pub fn from_f64(x: f64) -> Result<DD, NumError> {
        if x.is_finite() {
            Ok(DD {
                hi: x + 0.0,
                lo: 0.0,
            })
        } else {
            Err(NumError::NotFinite)
        }
    }

    /// An `i64`, exactly: `hi` is `v` rounded to nearest, ties to even, and `lo` the exact rest
    /// `v − hi`, at most half an ulp of `hi` (at most 2^10), +0 when `v` is a double.
    ///
    /// Without integer arithmetic: v = h·2^32 + l with h = v >> 32 (an arithmetic shift, so
    /// −2^31 ≤ h < 2^31) and l the low 32 bits (0 ≤ l < 2^32). Both are doubles exactly, and so is
    /// h·2^32, a product by a power of two below 2^63; `two_sum` of the two returns the exact sum v
    /// rounded to nearest and its exact error: the normalized pair, which is unique.
    #[must_use]
    pub fn from_i64(v: i64) -> DD {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "an i64 shifted right by 32 lies in [−2^31, 2^31), which i32 holds exactly"
        )]
        let high = (v >> 32) as i32;
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "masked to its low 32 bits, which u32 holds exactly"
        )]
        let low = (v & 0xffff_ffff) as u32;
        let (hi, lo) = two_sum(f64::from(high) * TWO_POW_32, f64::from(low));
        DD { hi, lo }
    }

    /// An `i128` of magnitude below 2^106, exactly; `None` for larger magnitudes, which two doubles
    /// cannot always hold.
    ///
    /// Below 2^106, `hi` = `v` rounded to nearest (Rust's integer-to-float conversion rounds to
    /// nearest, ties to even) is an integer of magnitude at most 2^106 with ulp at most 2^53, so it
    /// converts back to `i128` exactly and the rest `v − hi` has magnitude at most 2^52, a double.
    /// The subtraction is checked: |v|, |hi| ≤ 2^106 cannot overflow it.
    #[must_use]
    pub fn from_i128(v: i128) -> Option<DD> {
        if v.unsigned_abs() >= 1u128 << 106 {
            return None;
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "rounding v to nearest is the intent: hi is the leading part"
        )]
        let hi = v as f64;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "hi is a whole number of magnitude at most 2^106, which i128 holds exactly"
        )]
        let hi_whole = hi as i128;
        let rest = v.checked_sub(hi_whole)?;
        #[allow(
            clippy::cast_precision_loss,
            reason = "|rest| ≤ 2^52, so the conversion to f64 is exact"
        )]
        let lo = rest as f64;
        Some(DD { hi, lo })
    }

    /// The exact product `a · b` of two doubles, when it is certain that [`two_prod`] gives it
    /// exactly: either factor zero, or both normal with |a|, |b| < 2^996 and exponents `e_a` + `e_b`
    /// between −970 and 1021. `None` otherwise (an operand not finite, subnormal, or a product too
    /// close to the underflow or overflow threshold for this range to vouch for its error).
    #[must_use]
    pub fn exact_product(a: f64, b: f64) -> Option<DD> {
        if !a.is_finite() || !b.is_finite() {
            return None;
        }
        if a == 0.0 || b == 0.0 {
            return Some(DD::ZERO);
        }
        if !DD::product_in_range(a, b) {
            return None;
        }
        let (p, e) = two_prod(a, b);
        // two_prod returns p = a·b rounded to nearest and its exact error: already normalized.
        Some(DD { hi: p, lo: e })
    }

    /// The leading part: the value rounded to nearest.
    #[must_use]
    pub const fn hi(self) -> f64 {
        self.hi
    }

    /// The trailing part: the exact remainder `value − hi`.
    #[must_use]
    pub const fn lo(self) -> f64 {
        self.lo
    }

    /// The value rounded to the nearest double: `hi`, by the normalization invariant.
    #[must_use]
    pub const fn to_f64(self) -> f64 {
        self.hi
    }

    /// The sum, by `AccurateDWPlusDW` (Joldes, Muller and Popescu 2017; QD's `ieee_add`), with
    /// `two_sum` in place of `Fast2Sum`:
    ///
    /// ```text
    /// (s_h, s_l) = two_sum(x_h, y_h);  (t_h, t_l) = two_sum(x_l, y_l)
    /// c = RN(s_l + t_h);               (v_h, v_l) = two_sum(s_h, c)
    /// w = RN(t_l + v_l);               result = two_sum(v_h, w)
    /// ```
    ///
    /// Proof of the bound 6u² + 45u³ (x = self, y = other, S = x + y = `s_h` + `s_l` + `t_h` + `t_l`). With
    /// e₁ = c − (`s_l` + `t_h`) and e₂ = w − (`t_l` + `v_l`), the result is `v_h` + w = S + e₁ + e₂.
    ///
    /// - If `s_l` = 0, then c = `t_h` exactly. If also `v_l` = 0, w = `t_l` and the result is S exactly.
    ///   If `v_l` ≠ 0, (S) gives |`t_h`| ≤ `2|s_h` + `t_h`| = `2|v_h` + `v_l`| ≤ 2(1 + `u)|v_h`|, so
    ///   |`t_l`| ≤ 2u(1 + `u)|v_h`|, e₁ = 0 and |e₂| ≤ `u(|t_l`| + |`v_l`|) ≤ u²(3 + `2u)|v_h`|, while
    ///   |S| ≥ |`v_h|(1` − u − 2u(1 + u)): relative error at most u²(3 + 2u)/(1 − 3u − 2u²).
    /// - If `s_l` ≠ 0, (S) gives `max(|x_h`|, |`y_h`|) ≤ `2|s_h`|, so (N) gives |`x_l`| + |`y_l`| ≤ `4u|s_h`|,
    ///   |`t_h`| ≤ 4u(1 + `u)|s_h`|, |`t_l`| ≤ 4u²(1 + `u)|s_h`| and |`s_l`| ≤ `u|s_h`|. Then
    ///   |e₁| ≤ `u|s_l` + `t_h`| ≤ u²(5 + `4u)|s_h`|; |c| ≤ u(1 + u)(5 + `4u)|s_h`|;
    ///   |`v_h`| ≤ (1 + u)(1 + u(1 + u)(5 + `4u))|s_h`| and |`v_l`| ≤ `u|v_h`|; |e₂| ≤ `u(|t_l`| + |`v_l`|);
    ///   and |S| ≥ |`s_h`| − |`s_l`| − |`x_l`| − |`y_l`| ≥ (1 − `5u)|s_h`|. So (|e₁| + |e₂|)/|S| is below
    ///   6u² + 45u³.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if the result overflows.
    #[allow(
        clippy::should_implement_trait,
        reason = "returns Result (it may overflow), so it cannot be core::ops::Add"
    )]
    pub fn add(self, other: DD) -> Result<DD, NumError> {
        let (sh, sl) = two_sum(self.hi, other.hi);
        let (th, tl) = two_sum(self.lo, other.lo);
        let c = sl + th;
        let (vh, vl) = two_sum(sh, c);
        let w = tl + vl;
        DD::normalized(vh, w)
    }

    /// The difference: the sum with the negation of `other` (negation is exact).
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if the result overflows.
    #[allow(
        clippy::should_implement_trait,
        clippy::arithmetic_side_effects,
        reason = "returns Result (it may overflow), so it cannot be core::ops::Sub; negating a double-double flips both parts' signs, exactly"
    )]
    pub fn sub(self, other: DD) -> Result<DD, NumError> {
        self.add(-other)
    }

    /// The sum with a double, by `DWPlusFP` (Joldes, Muller and Popescu 2017):
    /// (`s_h`, `s_l`) = `two_sum(x_h`, y); v = `RN(x_l` + `s_l`); result = `two_sum(s_h`, v).
    ///
    /// Proof of the bound 3u²/(1 − 3u): the result is x + y + E with E = v − (`x_l` + `s_l`), and
    /// |E| ≤ `u|x_l` + `s_l`|. If `s_l` = 0, v = `x_l` and E = 0. Otherwise (S) gives |`x_h`| ≤ `2|s_h`|, so
    /// |`x_l` + `s_l`| ≤ `u|x_h`| + `u|s_h`| ≤ `3u|s_h`| while |x + y| = |`s_h` + `s_l` + `x_l`| ≥ (1 − `3u)|s_h`|.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `y` is not finite or the result overflows.
    pub fn add_f64(self, y: f64) -> Result<DD, NumError> {
        if !y.is_finite() {
            return Err(NumError::NotFinite);
        }
        let (sh, sl) = two_sum(self.hi, y);
        let v = self.lo + sl;
        DD::normalized(sh, v)
    }

    /// The product with a double, by `DWTimesFP1` (Joldes, Muller and Popescu 2017), on operands
    /// scaled into `two_prod`'s range when they are outside it (see the module's *Range*):
    ///
    /// ```text
    /// (c_h, c_l1) = two_prod(x_h, y);   c_l2 = RN(x_l·y)
    /// (t_h, t_l1) = two_sum(c_h, c_l2); t_l2 = RN(t_l1 + c_l1);  result = two_sum(t_h, t_l2)
    /// ```
    ///
    /// Proof of the bound 3u² + 7u³ (P = |`x_h·y|)`: the result is x·y + e₂ + e₃ with
    /// e₂ = `c_l2` − `x_l·y` and e₃ = `t_l2` − (`t_l1` + `c_l1`). |e₂| ≤ `u|x_l·y`| ≤ u²P;
    /// |`c_l1`| ≤ uP, |`c_h`| ≤ (1 + u)P and |`c_l2`| ≤ u(1 + u)P, so |`t_h`| ≤ (1 + u)³P and
    /// |`t_l1`| ≤ u(1 + u)³P; |e₃| ≤ `u(|t_l1`| + |`c_l1`|) ≤ u²(1 + (1 + u)³)P. With |x·y| ≥ (1 − u)P
    /// the relative error is at most u²(2 + (1 + u)³)/(1 − u) < 3u² + 7u³.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `y` is not finite or the result overflows.
    /// [`NumError::BudgetExceeded`] only if a step on the exponents failed, which their range rules
    /// out (see the module documentation).
    pub fn mul_f64(self, y: f64) -> Result<DD, NumError> {
        if !y.is_finite() {
            return Err(NumError::NotFinite);
        }
        if self.hi == 0.0 || y == 0.0 {
            return Ok(DD::ZERO);
        }
        if DD::product_in_range(self.hi, y) {
            return self.mul_f64_core(y);
        }
        let (ux, uy) = (DD::unscaling(self.hi)?, DD::unscaling(y)?);
        let x = self.scaled(ux)?;
        let back = budget(ux.checked_add(uy).and_then(i32::checked_neg))?;
        x.mul_f64_core(budget(scale_b(y, uy))?)?.scaled(back)
    }

    /// `DWTimesFP1` itself, for operands inside `two_prod`'s range.
    fn mul_f64_core(self, y: f64) -> Result<DD, NumError> {
        let (ch, cl1) = two_prod(self.hi, y);
        let cl2 = self.lo * y;
        let (th, tl1) = two_sum(ch, cl2);
        let tl2 = tl1 + cl1;
        DD::normalized(th, tl2)
    }

    /// The product, by `DWTimesDW1` (Dekker 1971; Joldes, Muller and Popescu 2017), on operands
    /// scaled into `two_prod`'s range when they are outside it (see the module's *Range*):
    ///
    /// ```text
    /// (c_h, c_l1) = two_prod(x_h, y_h);  t_l1 = RN(x_h·y_l);  t_l2 = RN(x_l·y_h)
    /// c_l2 = RN(t_l1 + t_l2);  c_l3 = RN(c_l1 + c_l2);  result = two_sum(c_h, c_l3)
    /// ```
    ///
    /// Proof of the bound 8u² + 23u³ (P = |`x_h·y_h|)`: the result is
    /// x·y + e₁ + e₂ + e₃ + e₄ − `x_l·y_l`, with e₁ and e₂ the roundings of `t_l1` and `t_l2` (each at most
    /// u²P, since |`x_h·y_l`|, |`x_l·y_h`| ≤ uP), e₃ that of `c_l2` (|`t_l1`|, |`t_l2`| ≤ u(1 + u)P, so
    /// |e₃| ≤ 2u²(1 + u)P and |`c_l2`| ≤ 2u(1 + u)²P), e₄ that of `c_l3` (|`c_l1`| ≤ uP, so
    /// |e₄| ≤ u²(1 + 2(1 + u)²)P), and |`x_l·y_l`| ≤ u²P. In total at most u²(8 + 6u + 2u²)P, and
    /// |x·y| ≥ (1 − u)²P, so the relative error is at most u²(8 + 6u + 2u²)/(1 − u)² < 8u² + 23u³.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if the result overflows. [`NumError::BudgetExceeded`] only if a
    /// step on the exponents failed, which their range rules out (see the module documentation).
    #[allow(
        clippy::should_implement_trait,
        reason = "returns Result (it may overflow), so it cannot be core::ops::Mul"
    )]
    pub fn mul(self, other: DD) -> Result<DD, NumError> {
        if self.hi == 0.0 || other.hi == 0.0 {
            return Ok(DD::ZERO);
        }
        if DD::product_in_range(self.hi, other.hi) {
            return self.mul_core(other);
        }
        let (ux, uy) = (DD::unscaling(self.hi)?, DD::unscaling(other.hi)?);
        let back = budget(ux.checked_add(uy).and_then(i32::checked_neg))?;
        self.scaled(ux)?.mul_core(other.scaled(uy)?)?.scaled(back)
    }

    /// `DWTimesDW1` itself, for operands inside `two_prod`'s range.
    fn mul_core(self, other: DD) -> Result<DD, NumError> {
        let (ch, cl1) = two_prod(self.hi, other.hi);
        let tl1 = self.hi * other.lo;
        let tl2 = self.lo * other.hi;
        let cl2 = tl1 + tl2;
        let cl3 = cl1 + cl2;
        DD::normalized(ch, cl3)
    }

    /// The quotient `self / other`, by the QD library's accurate division, on operands scaled so
    /// that their leading parts lie in [1, 2) (see the module's *Range*): three quotient digits,
    /// each from the remainder left by the previous ones.
    ///
    /// ```text
    /// q₁ = RN(x_h / y_h);  r₁ = x − y·q₁    (mul_f64, then sub)
    /// q₂ = RN(r₁_h / y_h); r₂ = r₁ − y·q₂
    /// q₃ = RN(r₂_h / y_h)
    /// result = two_sum(q₁, q₂) + q₃         (add_f64)
    /// ```
    ///
    /// Proof of the bound 6u² + 79u³ (x = self, y = other, ρ = x/y; `B_m`, `B_a` and `B_f` the proven
    /// bounds of `mul_f64`, `add` and `add_f64` above; D = (3u + u²)/(1 − u)).
    ///
    /// 1. By (N) and (R), q₁ = ρ(1 + δ₁) with |δ₁| ≤ D: `x_h/y_h` = ρ(1 + β)/(1 + α) with
    ///    |α|, |β| ≤ u, then one rounding. Likewise q₂ = (r₁/y)(1 + δ₂) and q₃ = (r₂/y)(1 + δ₃),
    ///    |δ₂|, |δ₃| ≤ D.
    /// 2. The exact remainder x − y·q₁ = −x·δ₁. The computed product is y·q₁ + E₁ with
    ///    |E₁| ≤ `B_m|y·q₁`| ≤ `B_m(1` + D)|x|, and r₁ = (−x·δ₁ − E₁)(1 + η₁) with |η₁| ≤ `B_a`, so
    ///    |r₁| ≤ K|x| with K = (D + `B_m(1` + D))(1 + `B_a`). In the same way r₂ = (−r₁δ₂ − E₂)(1 + η₂),
    ///    |E₂| ≤ `B_m(1` + D)|r₁|, |r₂| ≤ K²|x|.
    /// 3. Unwinding the steps, x = y·q₁ + E₁ + (y·q₂ + E₂ + y·q₃/((1 + η₂)(1 + δ₃)))/(1 + η₁), so
    ///    with Q = q₁ + q₂ + q₃, the difference y·Q − x is y·q₂(1 − 1/(1 + η₁)) plus
    ///    y·q₃(1 − 1/((1 + η₁)(1 + η₂)(1 + δ₃))) minus E₁ + E₂/(1 + η₁). Bounding each term with
    ///    |y·q₂| ≤ (1 + D)K|x| and |y·q₃| ≤ (1 + D)K²|x| gives |Q − ρ| ≤ G|ρ| with
    ///    G = (1 + `D)K·B_a/(1` − `B_a`) + (1 + D)K²(1/((1 − `B_a)²(1` − D)) − 1) + `B_m(1` + D) plus
    ///    `B_m(1` + D)K/(1 − `B_a`).
    /// 4. `two_sum` is exact and `add_f64` adds a relative `B_f`: the result is within
    ///    G + (1 + `G)B_f` of ρ, relatively, which is below 6u² + 79u³.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] if `other` is zero; [`NumError::NotFinite`] if the result
    /// overflows. [`NumError::BudgetExceeded`] only if a step on the exponents failed, which their
    /// range rules out (see the module documentation).
    #[allow(
        clippy::should_implement_trait,
        reason = "returns Result (zero divisor or overflow), so it cannot be core::ops::Div"
    )]
    pub fn div(self, other: DD) -> Result<DD, NumError> {
        if other.hi == 0.0 {
            return Err(NumError::Degenerate);
        }
        if self.hi == 0.0 {
            return Ok(DD::ZERO);
        }
        let (ux, uy) = (DD::unscaling(self.hi)?, DD::unscaling(other.hi)?);
        // The quotient's scaling back is 2^(kx − ky) = 2^(uy − ux).
        let back = budget(uy.checked_sub(ux))?;
        let (x, y) = (self.scaled(ux)?, other.scaled(uy)?);
        let q1 = x.hi / y.hi;
        let r1 = x.sub(y.mul_f64(q1)?)?;
        let q2 = r1.hi / y.hi;
        let r2 = r1.sub(y.mul_f64(q2)?)?;
        let q3 = r2.hi / y.hi;
        DD::normalized(q1, q2)?.add_f64(q3)?.scaled(back)
    }

    /// The square root, by one Newton step from the correctly rounded root of `hi` (QD's `sqrt`,
    /// after Karp and Markstein), with the residual computed exactly, on the value scaled by an even
    /// power of two so that its leading part lies in [1, 4) (see the module's *Range*):
    ///
    /// ```text
    /// x₀ = sqrt(a_h)            (geom_num::math::sqrt, correctly rounded)
    /// r  = a − x₀²              (x₀² exact by two_prod, then sub)
    /// result = two_sum(x₀, RN(r_h / (2x₀)))
    /// ```
    ///
    /// Proof of the bound 4.13u² (a = self, s = √a, `B_a` the proven bound of `add`). By (N) and (R),
    /// x₀ = s(1 + ε) with |ε| ≤ ε̂ = (1 + u)/√(1 − u) − 1 < (1 + u)(1 + u/2 + u²/2) − 1. The exact
    /// Newton step N = x₀ + (a − x₀²)/(2x₀) satisfies N − s = (x₀ − s)²/(2x₀) = sε²/(2(1 + ε)), so
    /// 0 ≤ N − s ≤ s·ε̂²/(2(1 − ε̂)). The exact correction C = (a − x₀²)/(2x₀) has
    /// |C| = s|ε||2 + ε|/(2(1 + ε)) ≤ s·ε̂(2 + ε̂)/(2(1 − ε̂)). The computed correction is
    /// c = C(1 + η)(1 + d)/(1 + κ): η from `sub` (|η| ≤ `B_a`), κ from dropping `r_l` (|κ| ≤ u, by
    /// (N)), d from the division (|d| ≤ u; the factor 2x₀ is exact). So
    /// |c − C| ≤ |C|((1 + `B_a)(1` + u)/(1 − u) − 1), and the result x₀ + c (an exact `two_sum`)
    /// differs from s by at most |N − s| + |c − C| < 4.13u²·s.
    ///
    /// # Errors
    ///
    /// [`NumError::NegativeSqrt`] if the value is negative. [`NumError::BudgetExceeded`] only if a
    /// step on the exponents failed, which their range rules out (see the module documentation);
    /// the square root of the scaled leading part, which lies in [1, 4), is never refused, and its
    /// error would be returned if it were.
    pub fn sqrt(self) -> Result<DD, NumError> {
        if self.hi < 0.0 {
            return Err(NumError::NegativeSqrt);
        }
        if self.hi == 0.0 {
            return Ok(DD::ZERO);
        }
        let j = budget(exponent(self.hi))?.div_euclid(2);
        let a = self.scaled(budget(j.checked_mul(-2))?)?;
        let x0 = math::sqrt(a.hi)?.get();
        let (p, e) = two_prod(x0, x0);
        let r = a.sub(DD { hi: p, lo: e })?;
        let c = r.hi / (2.0 * x0);
        DD::normalized(x0, c)?.scaled(j)
    }

    /// The exact real `hi + lo − offset` rounded to the nearest integer, ties to even, with whether
    /// it was already an integer (no rounding happened). `None` if |hi| > 2^100 or the result does
    /// not fit an `i128`; also, never for a finite pair, if `math::round_ties_even` refused a part
    /// (it refuses only NaN and the infinities), which is then reported as no answer rather than a
    /// guessed one.
    ///
    /// No step rounds:
    ///
    /// 1. `r_h` = rint(hi) and `d_h` = hi − `r_h`, and likewise `r_l`, `d_l` for lo. For any double x,
    ///    x − rint(x) is exact: for |x| < 1/2 it is x itself; for |x| ≥ 2^52 it is 0; otherwise
    ///    rint(x) is a nonzero integer within 1/2 of x, so rint(x)/2 ≤ x ≤ 2·rint(x) and Sterbenz's
    ///    lemma applies. So |`d_h`|, |`d_l`| ≤ 1/2 and hi + lo = `r_h` + `r_l` + `d_h` + `d_l` exactly.
    /// 2. n = `r_h` + `r_l` − offset, in `i128` (both integers are below 2^100 in magnitude).
    /// 3. (t, `t_e`) = `two_sum(d_h`, `d_l)`: the fraction F = `d_h` + `d_l` = t + `t_e` exactly, |F| ≤ 1.
    /// 4. The value is n + F. F is compared with ±1/2 exactly: t is F rounded to nearest, and
    ///    rounding is monotone, so t > 1/2 implies F > 1/2 and t < 1/2 implies F < 1/2; when
    ///    t = 1/2 exactly, the sign of `t_e` decides. Then n + F rounds to n − 1, n or n + 1, with a
    ///    tie at F = ±1/2 going to the even neighbour.
    #[must_use]
    pub fn round_shifted(self, offset: i128) -> Option<Rounded<i128>> {
        if self.hi > ROUND_LIMIT || self.hi < -ROUND_LIMIT {
            return None;
        }
        let rh = math::round_ties_even(self.hi).ok()?.get();
        let rl = math::round_ties_even(self.lo).ok()?.get();
        let (dh, dl) = (self.hi - rh, self.lo - rl);
        let n = whole(rh).checked_add(whole(rl))?.checked_sub(offset)?;
        let (t, te) = two_sum(dh, dl);
        // How F = t + t_e compares with c (c = ±1/2, an exact double).
        let above = |c: f64| t > c || (t == c && te > 0.0);
        let equal = |c: f64| t == c && te == 0.0;
        let even = |a: i128, b: i128| if a.rem_euclid(2) == 0 { a } else { b };
        let value = if above(0.5) {
            n.checked_add(1)?
        } else if equal(0.5) {
            even(n, n.checked_add(1)?)
        } else if equal(-0.5) {
            even(n.checked_sub(1)?, n)
        } else if above(-0.5) {
            n
        } else {
            n.checked_sub(1)?
        };
        let exact = te == 0.0 && (t == 0.0 || t == 1.0 || t == -1.0);
        Some(Rounded::new(value, exact))
    }
}

/// An integral double of magnitude at most 2^100 as an `i128`, exactly: through `i64` (one
/// hardware conversion) below 2^62, directly otherwise.
fn whole(r: f64) -> i128 {
    if r > -TWO_POW_62 && r < TWO_POW_62 {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "r is a whole number below 2^62 in magnitude, which i64 holds exactly"
        )]
        let small = r as i64;
        i128::from(small)
    } else {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "r is a whole number of magnitude at most 2^100, which i128 holds exactly"
        )]
        let large = r as i128;
        large
    }
}

impl Neg for DD {
    type Output = DD;

    /// The negation `−hi − lo`. Exact, and still normalized (rounding to nearest is symmetric).
    fn neg(self) -> DD {
        DD {
            hi: -self.hi,
            lo: -self.lo,
        }
    }
}
