//! Double-double arithmetic: a real number held as the unevaluated sum of two doubles, with a
//! proven error bound on every operation.
//!
//! A [`DD`] `hi + lo` carries about 106 bits of significand. The kernel uses it where 53 bits are
//! not enough and exact integers are not available: survey and map data (binary `f64` from PROJ or
//! a file) are converted into a frame in double-double arithmetic and rounded once to the grid
//! (chapters 6.1 and 17.1), a curved evaluation near the resolution floor is redone in it (chapter
//! 12), and it is the reserve filter stage ahead of the wide integers (chapter 11).
//!
//! **Representation.** Both parts are finite, and the pair is a double-word number in the sense of
//! Joldes, Muller and Popescu (2017, Definition 1.4): `hi` is `hi + lo` rounded to nearest, ties to
//! even, so `|lo| ≤ ulp(hi)/2` and `hi` is the value correctly rounded to a double. A zero part is
//! +0. So every real a `DD` can hold has exactly one bit pattern: equal values have equal bits, and
//! [`Ord`] (by `hi`, then `lo`) is the order of the values. Every operation ends by restoring this
//! with an exact [`two_sum`] and replacing a −0 by +0.
//!
//! **Guarantees.** With u = 2^−53 (the unit roundoff of binary64), each operation returns the exact
//! result of the operation on the exact values of its operands, `r`, within
//! `|result − r| ≤ B·|r| + A`:
//!
//! | Operation | Algorithm | Proven relative bound | Theorem | B |
//! | --- | --- | --- | --- | --- |
//! | [`DD::add`], [`DD::sub`] | `AccurateDWPlusDW` | 3u²/(1 − 4u) | JMP 3.1 | [`ADD_BOUND`] |
//! | [`DD::add_f64`], [`DD::sub_f64`] | `DWPlusFP` | 2u² | JMP 2.2 | [`ADD_F64_BOUND`] |
//! | [`DD::mul_f64`] | `DWTimesFP3` | 2u² | JMP 4.3 | [`MUL_F64_BOUND`] |
//! | [`DD::mul`] | `DWTimesDW3` | (4u² + u³/2)/(1 + u)² | MR 2.8 | [`MUL_BOUND`] |
//! | [`DD::div_f64`] | `DWDivFP3` | 3u² | JMP 6.2 | [`DIV_F64_BOUND`] |
//! | [`DD::div`] | `DWDivDW3` | 9.8u² | JMP 7.2 | [`DIV_BOUND`] |
//! | [`DD::sqrt`] | `SQRTDWtoDW` | 25u²/8 | LLMPR 3.6 | [`SQRT_BOUND`] |
//!
//! JMP is Joldes, Muller and Popescu (2017), MR is Muller and Rideau (2022) and LLMPR is Lefèvre,
//! Louvet, Muller, Picot and Rideau (2023), below. Every bound in the third column has been
//! machine-checked in Coq: JMP's by MR (their Table 1, which also improves `DWTimesDW3` from 5u²
//! to 4u²), LLMPR's in that paper. B, a public constant, is the third column, plus 2^−398 for the
//! multiplicative operations (the trailing parts dropped by *Range* below), rounded up to a double.
//! A is 0 for the additions and the root, and 2^−1074 (the smallest subnormal) for the products
//! and quotients ([`UNDERFLOW_BOUND`]): scaling the result back may round each of its two parts to
//! the subnormal grid, by half of 2^−1074 at most, which only a part below 2^−1022 needs; it
//! matters relative to the result only below about 2^−969, where two doubles cannot hold 106 bits.
//! The tests hold every operation to `B·|r| + A` against exact rationals (`dashu`), check that it
//! computes the published algorithm step for step, and reproduce each published input that comes
//! close to its bound.
//!
//! Also guaranteed:
//!
//! - **Never NaN, never infinite.** An operation whose result is too large returns
//!   [`NumError::NotFinite`]. That happens only when `|r| > (1 − 2^−52)·2^1024`; a `DD` cannot hold
//!   any `r` with `|r| ≥ 2^1024 − 2^970` at all (its `hi` would be infinite). Between the two, an
//!   operation may refuse a result a `DD` could hold, because an intermediate overflows: for
//!   example [`DD::add`] of (Ω, −2^969) and 2^970, whose sum Ω + 2^969 is the pair (Ω, 2^969),
//!   where Ω = (2 − 2^−52)·2^1023, but whose leading parts sum to 2^1024 − 2^970, which rounds to
//!   infinity. Code that works within 2^972 of the top of the range must expect `NotFinite` there.
//!   A zero divisor gives `NotFinite` too (the quotient would be infinite or NaN), and the root of
//!   a negative value [`NumError::OutsideDomain`], as [`crate::math::sqrt`] does.
//!   [`NumError::BudgetExceeded`] would mean a defect in this module (an internal exponent out of
//!   the range proven for it); no input causes one.
//! - **Commutative.** `x.add(y)` and `y.add(x)` have the same bits, and so do `x.mul(y)` and
//!   `y.mul(x)` ([`DD::add`] and [`DD::mul`] say why).
//! - **Exact where it says so.** [`DD::from_f64`], [`DD::from_i64`], [`DD::from_i128`] (below
//!   2^106, or `None`), [`DD::from_sum`], [`DD::exact_product`] (or `None`), negation, [`DD::abs`]
//!   and [`DD::round_shifted`] are exact. [`DD::to_f64`] is the value correctly rounded, which is
//!   `hi` by the invariant.
//! - **Deterministic.** Only `+`, `−`, `×` and `÷` on `f64`, each rounded once to nearest as IEEE
//!   754 and Rust (RFC 3514) specify on every target the kernel supports, reads and writes of a
//!   double's bits, and the square root and fused multiply-add of [`crate::math`], both correctly
//!   rounded (IEEE 754 requires it of both, and `libm` 0.2.16 provides it with the processor's
//!   instruction or its own routine; [`crate::interval`] gives the details). So every target gives
//!   the same bits.
//! - **No integer overflow.** The integer steps are on exponents (at most 2400 in magnitude) and
//!   on the integers of [`DD::from_i128`] and [`DD::round_shifted`], all with checked methods; a
//!   `None` from one is reported as `BudgetExceeded` (exponents, a defect) or `None` (the
//!   documented range of those two functions). No cast loses a value.
//!
//! **Method.** The algorithms are those whose bounds are proven and machine-checked:
//!
//! - addition, `AccurateDWPlusDW` (JMP Algorithm 6, the QD library's "IEEE addition" of Li et al.)
//!   and `DWPlusFP` (JMP Algorithm 4, QD's);
//! - multiplication by a double, `DWTimesFP3` (JMP Algorithm 9), and of two `DD`s, `DWTimesDW3`
//!   (JMP Algorithm 12), the ones JMP recommend when a fused multiply-add is available;
//! - division by a double, `DWDivFP3` (JMP Algorithm 15), and of two `DD`s, `DWDivDW3` (JMP
//!   Algorithm 18, the more accurate of their two divisions);
//! - the square root, `SQRTDWtoDW` (LLMPR Algorithm 8);
//! - `TwoProduct` as Kahan's `Fast2Mult` with a fused multiply-add ([`two_prod_fma`], JMP Algorithm
//!   3), and `TwoSum` as Knuth's ([`crate::eft::two_sum`]).
//!
//! Changes from the sources, and why:
//!
//! - Every `Fast2Sum` of the published algorithms is [`two_sum`]. Where the published proof shows
//!   `Fast2Sum` exact the two return the same pair (the rounded sum and its exact error are
//!   unique, and JMP note the swap leaves every bound unchanged); `two_sum` needs no ordering.
//! - The sources assume an unbounded exponent. Here every multiplicative operation runs on its
//!   operands scaled by powers of two so that their leading parts lie in [1, 2) ([1, 4) for the
//!   root), drops a scaled trailing part below 2^−400, and scales the result back once per part
//!   with a single rounding (*Range* below proves that binary64 then computes exactly what the
//!   unbounded arithmetic of the proofs computes). Additions need neither: they are exact in
//!   gradual underflow.
//! - `DWDivDW3`'s listing in JMP names `DWTimesDW2` on its last line and Algorithm 12 in its
//!   comment; its proof (via JMP Theorem 5.4) is for Algorithm 12, `DWTimesDW3`, used here.
//! - [`DD::mul`] puts its scaled operands in a fixed order (the larger magnitude first) before
//!   `DWTimesDW3`, whose listing is not symmetric in them, so that `x·y` and `y·x` have the same
//!   bits. The bound holds for either order, so it is unchanged. ([`DD::add`] is commutative bit
//!   for bit as published: [`two_sum`] is, its pair being unique and its zero +0.)
//! - Results are checked for finiteness, and zeros made +0.
//!
//! **Range.** Two facts carry the unbounded-exponent proofs over to binary64 (this module's lemma
//! G and lemma R; [`crate::eft`]'s module documentation proves the first for sums):
//!
//! - (G) An IEEE operation whose exact result `r` is finite and at most Ω = (2 − 2^−52)·2^1023 in
//!   magnitude returns what the unbounded 53-bit arithmetic returns if `|r| ≥ 2^−1022` (both round
//!   to 53 bits in the same binade) or `r` is a whole multiple of 2^−1074 (then, below 2^−1022, it
//!   has at most 52 significant bits, so both return `r` itself). Every sum of doubles is such a
//!   multiple, so [`DD::add`] and [`DD::add_f64`], which only add, meet their bounds over the whole
//!   range; the only exception is overflow, reported as above.
//! - (R) Rounding to nearest maps the multiples of 2^−g into themselves for g ≤ 1074: a multiple
//!   that needs more than 53 bits has its leading bit at 2^(53 − g) or above, so its rounding is a
//!   multiple of its unit in the last place, 2^(1 − g) or coarser; one below 2^−1022 is a double.
//!
//! The multiplicative operations scale. Write `e(v)` for the exponent of the leading bit of a
//! nonzero double v (−1074 to 1023). An operand `x` becomes `x' = x·2^−e(x_hi)` (the exponent
//! rounded down to an even one for the root): its leading part is in [1, 2) ([1, 4)), exactly,
//! and its trailing part, rounded once, is kept if it is at least 2^−400 in magnitude (then it was
//! exact, being normal) and replaced by +0 otherwise, which moves `x'` by less than 2^−400·|x'|.
//! In the scaled operands every leading part is a whole multiple of 2^−52 and every trailing part
//! of 2^−452. Each operation's documentation lists its steps' exact results on that footing: every
//! product and fused multiply-add is a whole multiple of 2^−1010 or a coarser power of two (so of
//! 2^−1074, and (G) applies), every quotient and root is zero or above 2^−1022, and nothing
//! exceeds 16. So binary64 computes the published algorithm on the scaled operands exactly as the
//! proofs' arithmetic does, and the published bound holds for them. The dropped trailing parts
//! change the exact result by less than 2^−399 relatively (less than 2δ + δ², with
//! δ < 2^−400·(1 + 2^−52), for the two operands of a product or quotient), so the bound becomes the
//! published one plus less than 2^−398. Scaling the result back multiplies each part by a power of
//! two with one correctly rounded multiplication (`scale_pow2`): exact unless a part falls below
//! 2^−1022, where it moves by at most half of 2^−1074, so by at most 2^−1074 in all; then
//! [`two_sum`] restores the invariant exactly.
//!
//! Sources:
//!
//! - M. Joldes, J.-M. Muller and V. Popescu, "Tight and rigorous error bounds for basic building
//!   blocks of double-word arithmetic", *ACM Transactions on Mathematical Software* 44(2), article
//!   15res (2017), <https://doi.org/10.1145/3121432>; open copy
//!   <https://hal.science/hal-01351529v3> (read 2026-10-21): Algorithms 3, 4, 6, 9, 12, 15 and 18,
//!   Theorems 2.2, 3.1, 4.3, 6.2 and 7.2, Property 6.1.
//! - J.-M. Muller and L. Rideau, "Formalization of double-word arithmetic, and comments on 'Tight
//!   and rigorous error bounds for basic building blocks of double-word arithmetic'", *ACM
//!   Transactions on Mathematical Software* 48(1) (2022), <https://doi.org/10.1145/3484514>; open
//!   copy <https://hal.science/hal-02972245v2> (read 2026-10-21): the Coq proofs of every JMP
//!   bound, Theorem 2.8 (`DWTimesDW3` within (4u² + u³/2)/(1 + u)²) and Property 2.1 (the input
//!   that makes `AccurateDWPlusDW` err by 3u² − 11u³ + O(u⁴)).
//! - V. Lefèvre, N. Louvet, J.-M. Muller, J. Picot and L. Rideau, "Accurate calculation of
//!   Euclidean norms using double-word arithmetic", *ACM Transactions on Mathematical Software*
//!   49(1) (2023), <https://doi.org/10.1145/3568672>; open copy <https://hal.science/hal-03482567>
//!   (read 2026-10-21): Algorithm 8 (`SQRTDWtoDW`), Theorem 3.6 (within 25u²/8, Coq-verified),
//!   and Lemma 1.5 (Boldo and Daumas: the remainder `x − s²` of a correctly rounded root is a
//!   double).
//! - T. J. Dekker, "A floating-point technique for extending the available precision", *Numerische
//!   Mathematik* 18 (1971), 224–242, <https://doi.org/10.1007/BF01397083>: the representation.
//! - Y. Hida, X. S. Li and D. H. Bailey, "Algorithms for quad-double precision floating point
//!   arithmetic", *15th IEEE Symposium on Computer Arithmetic* (2001), 155–162,
//!   <https://doi.org/10.1109/ARITH.2001.930115>, and their QD library, the most used open
//!   implementation, read at release 2.3.24 (`include/qd/dd_inline.h`, `src/dd_real.cpp`;
//!   modified BSD licence of Lawrence Berkeley National Laboratory),
//!   <https://www.davidhbailey.com/dhbsoftware/>: method only, no code taken. Its "IEEE addition"
//!   is `AccurateDWPlusDW`, but its default `+` is the "sloppy" addition, whose relative error JMP
//!   show unbounded for operands of opposite signs; its products fold the two cross terms into one
//!   rounded sum and its root is Karp's step from `1/√x_h`, neither with a published bound; so
//!   the algorithms here are JMP's and LLMPR's instead.
//! - The double-double type piece G03 wrote for `geom-math` (Spare Kit, MPL-2.0), whose exact
//!   conversions ([`DD::from_i64`], [`DD::from_i128`], [`DD::round_shifted`]) are kept here with
//!   their proofs re-checked, now without lossy casts; its operations are replaced by the
//!   machine-checked algorithms above, and its scaling by the one in *Range*, which also covers
//!   subnormal operands and trailing parts.
//!
//! **Cost.** Constant time, no allocation, and no loops other than at most three power-of-two
//! steps per scaling. Counting `two_sum` as 6 floating-point operations, a fused multiply-add as
//! one call to [`crate::math::fma`] (one instruction where the processor has one, through `libm`'s
//! runtime dispatch on x86-64; a correctly rounded integer routine on WebAssembly), and the final
//! `two_sum` that restores the invariant: [`DD::add`] is 26 operations and [`DD::add_f64`] 13;
//! [`DD::mul_f64`] 2 fused multiply-adds and 13 operations, [`DD::mul`] 3 and 15,
//! [`DD::div_f64`] 1 and 18, [`DD::div`] 6 and 44, [`DD::sqrt`] one square root, one fused
//! multiply-add and 15. The multiplicative ones add the scaling: for each operand a read of its
//! exponent bits and two power-of-two scalings (one to three multiplications each), and two
//! scalings of the result; [`DD::mul`] also compares its two scaled operands to order them.

use core::cmp::Ordering;
use core::hash::{Hash, Hasher};
use core::ops::Neg;

use crate::eft::{two_prod, two_sum};
use crate::error::NumError;
use crate::grid::Rounded;
use crate::math;
use crate::real::{Real, zero_to_plus};
use crate::sign::Sign;

/// 2^−106 = u², from its bits: exponent field 1023 − 106 = 917 = 0x395.
const U2: f64 = f64::from_bits(0x3950_0000_0000_0000);

/// The bound B of [`DD::add`] and [`DD::sub`]: (3 + 2^−49)·u², above JMP Theorem 3.1's
/// 3u²/(1 − 4u) = 3u²(1 + 4u + 16u² + …) < (3 + 3·2^−51 + 2^−99)·u².
pub const ADD_BOUND: f64 = (3.0 + TWO_POW_NEG_49) * U2;

/// The bound B of [`DD::add_f64`] and [`DD::sub_f64`]: 2u², JMP Theorem 2.2, exactly.
pub const ADD_F64_BOUND: f64 = 2.0 * U2;

/// The bound B of [`DD::mul_f64`]: (2 + 2^−49)·u², above JMP Theorem 4.3's 2u² plus the 2^−398 of
/// the module's *Range*.
pub const MUL_F64_BOUND: f64 = (2.0 + TWO_POW_NEG_49) * U2;

/// The bound B of [`DD::mul`]: 4u², above Muller and Rideau's Theorem 2.8, (4u² + u³/2)/(1 + u)²,
/// which is below 4u² − 7u³, plus the 2^−398 of the module's *Range*.
pub const MUL_BOUND: f64 = 4.0 * U2;

/// The bound B of [`DD::div_f64`]: (3 + 2^−49)·u², above JMP Theorem 6.2's 3u² plus 2^−398.
pub const DIV_F64_BOUND: f64 = (3.0 + TWO_POW_NEG_49) * U2;

/// The bound B of [`DD::div`]: 9.8 (as a double, which is 9.8 + 7.1·10^−16) times u², above JMP
/// Theorem 7.2's 9.8u² plus 2^−398.
pub const DIV_BOUND: f64 = 9.8 * U2;

/// The bound B of [`DD::sqrt`]: (25/8 + 2^−49)·u², above Theorem 3.6 of Lefèvre et al., 25u²/8,
/// plus the 2^−398 of the module's *Range* (on the root, half that of its operand). Scaling the
/// root back is exact ([`DD::sqrt`]), so it adds nothing.
pub const SQRT_BOUND: f64 = (3.125 + TWO_POW_NEG_49) * U2;

/// The absolute part A of the multiplicative operations' bounds: 2^−1074, the smallest subnormal.
pub const UNDERFLOW_BOUND: f64 = f64::from_bits(1);

/// 2^−49: the margin added to bounds that carry a term of order u³ or 2^−398. Exponent field
/// 1023 − 49 = 974 = 0x3ce.
const TWO_POW_NEG_49: f64 = f64::from_bits(0x3ce0_0000_0000_0000);

/// 2^−400: a scaled trailing part below it is dropped (module documentation, *Range*). Exponent
/// field 1023 − 400 = 623 = 0x26f.
const TRAIL_FLOOR: f64 = f64::from_bits(0x26f0_0000_0000_0000);

/// 2^−968: from it up, [`two_prod_fma`]'s fused multiply-add is exact. Exponent field
/// 1023 − 968 = 55 = 0x037.
const TWO_POW_NEG_968: f64 = f64::from_bits(0x0370_0000_0000_0000);

/// 2^1000 and 2^−1000: the largest steps [`scale_pow2`] takes. Exponent fields 2023 = 0x7e7 and
/// 23 = 0x017.
const TWO_POW_1000: f64 = f64::from_bits(0x7e70_0000_0000_0000);
const TWO_POW_NEG_1000: f64 = f64::from_bits(0x0170_0000_0000_0000);

/// 2^100: the largest `|hi|` for which [`DD::round_shifted`] answers. Exponent field 1123 = 0x463.
const ROUND_LIMIT: f64 = f64::from_bits(0x4630_0000_0000_0000);

/// The 52 fraction bits of a double.
const FRACTION_BITS: u64 = (1 << 52) - 1;

// Each power of two above is what its name says. Checked when the crate is compiled.
const _: () = {
    assert!(U2.to_bits() == (1023 - 106) << 52);
    assert!(TWO_POW_NEG_49.to_bits() == (1023 - 49) << 52);
    assert!(TRAIL_FLOOR.to_bits() == (1023 - 400) << 52);
    assert!(TWO_POW_NEG_968.to_bits() == (1023 - 968) << 52);
    assert!(TWO_POW_1000.to_bits() == (1023 + 1000) << 52);
    assert!(TWO_POW_NEG_1000.to_bits() == (1023 - 1000) << 52);
    assert!(ROUND_LIMIT.to_bits() == (1023 + 100) << 52);
    // The bounds are exact products of a double by a power of two, and above what they claim
    // (the first by 2^−49·u², far more than the 3u²(4u + 16u² + …) + 2^−398 they cover).
    assert!(ADD_BOUND > 3.0 * U2 && ADD_F64_BOUND == 2.0 * U2 && MUL_BOUND == 4.0 * U2);
};

/// The error reported when an internal integer step that is proven in range is not: a defect in
/// this module, never a property of the input.
const fn defect() -> NumError {
    NumError::BudgetExceeded
}

/// The magnitude of `x` by comparison (`−x` for `x < 0`): exact, and NaN-free for finite `x`.
const fn magnitude(x: f64) -> f64 {
    if x < 0.0 { -x } else { x }
}

/// The exponent of the leading bit of a finite nonzero `x`, `⌊log₂ |x|⌋`, from −1074 to 1023, read
/// from its bits: the exponent field minus 1023 for a normal `x`; for a subnormal one,
/// `x = f·2^−1074` with `1 ≤ f < 2^52`, whose leading bit is at 63 − (leading zeros of f).
fn exponent(x: f64) -> Result<i32, NumError> {
    let bits = x.to_bits();
    let field = u16::try_from((bits >> 52) & 0x7ff).map_err(|_| defect())?;
    if field == 0 {
        let zeros = i32::try_from((bits & FRACTION_BITS).leading_zeros()).map_err(|_| defect())?;
        (-1011_i32).checked_sub(zeros).ok_or_else(defect)
    } else {
        i32::from(field).checked_sub(1023).ok_or_else(defect)
    }
}

/// 2^k exactly, for −1074 ≤ k ≤ 1023, built from its bits: exponent field `k + 1023` for
/// `k ≥ −1022`, otherwise the subnormal with only fraction bit `k + 1074` set.
fn pow2(k: i32) -> Result<f64, NumError> {
    if k >= -1022 {
        let field = u64::try_from(k.checked_add(1023).ok_or_else(defect)?).map_err(|_| defect())?;
        if field > 2046 {
            return Err(defect());
        }
        Ok(f64::from_bits(field << 52))
    } else {
        let position =
            u32::try_from(k.checked_add(1074).ok_or_else(defect)?).map_err(|_| defect())?;
        let bit = 1_u64.checked_shl(position).ok_or_else(defect)?;
        Ok(f64::from_bits(bit))
    }
}

/// `v·2^m` correctly rounded (one rounding, to nearest, ties to even; an infinity on overflow),
/// for a finite `v` and `|m| ≤ 2400`.
///
/// - `m ≥ 0`: steps of 2^1000, then one of 2^r with `r ≤ 1000`. Multiplying a double by a power of
///   two of at least 1 is exact unless it overflows (a subnormal `v` times 2^k is `f·2^(k − 1074)`
///   with `f < 2^52`, a double while it is finite), and an infinity stays one; so the result is
///   `v·2^m` exactly, or an infinity exactly when `v·2^m` exceeds Ω, which is when its correct
///   rounding is one (no double lies between Ω and 2^1024).
/// - `−1074 ≤ m < 0`: one multiplication by the double 2^m, so one rounding.
/// - `m < −1074`: first `v₁ = v·2^(m + 1074)` in steps of 2^−1000 and one last step, then
///   `v₁·2^−1074`. If `|v·2^(m + 1074)| ≥ 2^−1022`, every step's exact result is normal (each lies
///   between `|v|` and it), so `v₁` is exact and the last multiplication is the one rounding. If
///   not, `|v·2^m| < 2^−2096`, whose correct rounding is a zero; and a rounded step never takes a
///   value above 2^−1022 (rounding is monotone and 2^−1022 is a double), so `|v₁| ≤ 2^−1022` and
///   `v₁·2^−1074` is at most 2^−2096 in magnitude, which also rounds to a zero.
///
/// The loops run at most three times for `|m| ≤ 2400`, and every exponent stays within ±2400, so
/// no checked step fails.
fn scale_pow2(v: f64, m: i32) -> Result<f64, NumError> {
    if !(-2400..=2400).contains(&m) {
        return Err(defect());
    }
    let mut v = v;
    if m >= 0 {
        let mut rest = m;
        while rest > 1000 {
            v *= TWO_POW_1000;
            rest = rest.checked_sub(1000).ok_or_else(defect)?;
        }
        return Ok(v * pow2(rest)?);
    }
    if m >= -1074 {
        return Ok(v * pow2(m)?);
    }
    let mut rest = m.checked_add(1074).ok_or_else(defect)?;
    while rest < -1000 {
        v *= TWO_POW_NEG_1000;
        rest = rest.checked_add(1000).ok_or_else(defect)?;
    }
    v *= pow2(rest)?;
    Ok(v * pow2(-1074)?)
}

/// The product `a · b` rounded, and its exact error, by Kahan's `Fast2Mult` with a fused
/// multiply-add (JMP Algorithm 3): `p = RN(a·b)`, `e = RN(a·b − p)`.
///
/// The contract is [`crate::eft::two_prod`]'s, bit for bit: `(p, e)` with `p` the IEEE product and
/// `p + e = a·b` exactly whenever an exact pair exists; otherwise `e` is an infinity with the sign
/// of the exact error, and `(p, −p)` when `p` is not finite. A zero `e` is +0. The two functions
/// return the same bits for every input (the tests check it), because the exact pair is unique.
///
/// When `p` is finite and `|p| ≥ 2^−968`, the fused multiply-add is exact: then
/// `e(a) + e(b) ≥ −970` (as [`crate::eft::two_prod`] proves), so `a·b` is a whole multiple of
/// `2^(e(a) − 52)·2^(e(b) − 52) ≥ 2^−1074`, and so is `a·b − p`, which is at most half a unit in
/// the last place of `p`, below `2^(e(a) + e(b) − 51)`: at most 52 significant bits on that grid,
/// a double. The fused multiply-add rounds that double to itself; an exact zero `a·b − p` with
/// `a·b ≠ 0` is +0 under round to nearest (IEEE 754-2019, section 6.3). Every other input (a
/// product near or below the underflow threshold, a zero factor, an overflow) goes to
/// [`crate::eft::two_prod`], which handles it by exact scaling. [`crate::math::fma`] is `libm`'s,
/// correctly rounded on every target.
#[must_use]
pub fn two_prod_fma(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    if p.is_finite() && magnitude(p) >= TWO_POW_NEG_968 {
        if let Ok(e) = math::fma(a, b, -p) {
            return (p, zero_to_plus(e.get()));
        }
    }
    two_prod(a, b)
}

/// A real number `hi + lo` held as two doubles: finite, `hi` the value rounded to nearest, a zero
/// part +0 (module documentation).
#[derive(Clone, Copy, Debug)]
pub struct DD {
    hi: f64,
    lo: f64,
}

// The arithmetic methods return `Result` (a result may overflow, a divisor may be zero), so they
// are deliberately not the `core::ops` traits, whose callers expect a plain value.
#[allow(
    clippy::should_implement_trait,
    reason = "fallible arithmetic returns Result, not Self"
)]
impl DD {
    /// Zero (+0, +0).
    pub const ZERO: DD = DD { hi: 0.0, lo: 0.0 };

    /// One.
    pub const ONE: DD = DD { hi: 1.0, lo: 0.0 };

    /// The pair for the exact sum `a + b` of two doubles: [`two_sum`], zeros made +0, or
    /// `NotFinite` if the rounded sum overflows or a part is not finite.
    fn normalized(a: f64, b: f64) -> Result<DD, NumError> {
        let (hi, lo) = two_sum(a, b);
        if hi.is_finite() && lo.is_finite() {
            Ok(DD {
                hi: zero_to_plus(hi),
                lo: zero_to_plus(lo),
            })
        } else {
            Err(NumError::NotFinite)
        }
    }

    /// A double, exactly.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `x` is NaN or infinite.
    pub const fn from_f64(x: f64) -> Result<DD, NumError> {
        if x.is_finite() {
            Ok(DD {
                hi: zero_to_plus(x),
                lo: 0.0,
            })
        } else {
            Err(NumError::NotFinite)
        }
    }

    /// A [`Real`], exactly.
    #[must_use]
    pub const fn from_real(x: Real) -> DD {
        DD {
            hi: zero_to_plus(x.get()),
            lo: 0.0,
        }
    }

    /// The exact sum `a + b` of two doubles.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `a` or `b` is not finite, or the rounded sum overflows (then no
    /// `DD` holds the sum).
    pub fn from_sum(a: f64, b: f64) -> Result<DD, NumError> {
        if !a.is_finite() || !b.is_finite() {
            return Err(NumError::NotFinite);
        }
        DD::normalized(a, b)
    }

    /// The exact product `a · b` of two doubles, or `None` when no `DD` holds it: a factor not
    /// finite, a product that overflows, or one with a bit below 2^−1074 (possible only for
    /// `|a·b| < 2^−969`). By [`two_prod_fma`], whose pair is exact or reports itself.
    #[must_use]
    pub fn exact_product(a: f64, b: f64) -> Option<DD> {
        if !a.is_finite() || !b.is_finite() {
            return None;
        }
        let (p, e) = two_prod_fma(a, b);
        if p.is_finite() && e.is_finite() {
            // `p` is the product rounded to nearest and `e` its exact error: already a pair.
            Some(DD {
                hi: zero_to_plus(p),
                lo: zero_to_plus(e),
            })
        } else {
            None
        }
    }

    /// An `i64`, exactly.
    ///
    /// `v = h·2^32 + l` with `h` its upper 32 bits as an `i32` and `l` its lower 32 bits as a `u32`
    /// (two's complement), both doubles exactly; `h·2^32` is exact, and [`two_sum`] of `h·2^32`
    /// and `l` gives `v` rounded to nearest and its exact error, which is at most 2^10 in magnitude
    /// (half a unit of `|v| < 2^64`), with no overflow.
    #[must_use]
    pub fn from_i64(v: i64) -> DD {
        let [b0, b1, b2, b3, b4, b5, b6, b7] = v.to_be_bytes();
        let high = f64::from(i32::from_be_bytes([b0, b1, b2, b3]));
        let low = f64::from(u32::from_be_bytes([b4, b5, b6, b7]));
        let (hi, lo) = two_sum(high * 4_294_967_296.0, low);
        DD {
            hi: zero_to_plus(hi),
            lo: zero_to_plus(lo),
        }
    }

    /// An `i128` of magnitude below 2^106, exactly; `None` for larger magnitudes, some of which two
    /// doubles cannot hold.
    ///
    /// `v = h·2^53 + l` with `h = ⌊v / 2^53⌋` (an arithmetic shift, `|h| < 2^53`) and
    /// `0 ≤ l < 2^53`, each a double exactly ([`DD::from_i64`] of an `i64` below 2^53 has
    /// `lo` = 0); `h·2^53` is exact, and [`two_sum`] gives `v` rounded to nearest and its exact
    /// error, at most 2^52 (half a unit of `|v| < 2^106`).
    #[must_use]
    pub fn from_i128(v: i128) -> Option<DD> {
        if v.unsigned_abs() >= 1_u128 << 106 {
            return None;
        }
        let high = i64::try_from(v >> 53).ok()?;
        let low = i64::try_from(v & ((1_i128 << 53) - 1)).ok()?;
        let (hi, lo) = two_sum(
            DD::from_i64(high).hi * 9_007_199_254_740_992.0,
            DD::from_i64(low).hi,
        );
        Some(DD {
            hi: zero_to_plus(hi),
            lo: zero_to_plus(lo),
        })
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

    /// The value correctly rounded to a double (to nearest, ties to even): `hi`, by the invariant.
    #[must_use]
    pub const fn to_f64(self) -> f64 {
        self.hi
    }

    /// The sign of the value: the sign of `hi`, which is zero only when the value is.
    #[must_use]
    pub const fn sign(self) -> Sign {
        if self.hi > 0.0 {
            Sign::Positive
        } else if self.hi < 0.0 {
            Sign::Negative
        } else {
            Sign::Zero
        }
    }

    /// The negation `−hi − lo`: exact, and still a pair in the invariant (rounding to nearest is
    /// symmetric), zeros kept +0. Also the `-` operator.
    #[must_use]
    pub const fn negate(self) -> DD {
        DD {
            hi: zero_to_plus(-self.hi),
            lo: zero_to_plus(-self.lo),
        }
    }

    /// The magnitude: the value or its negation. Exact.
    #[must_use]
    pub const fn abs(self) -> DD {
        if self.hi < 0.0 { self.negate() } else { self }
    }

    /// The sum, by `AccurateDWPlusDW` (JMP Algorithm 6) with [`two_sum`] for its `Fast2Sum`s:
    ///
    /// ```text
    /// (s_h, s_l) = two_sum(x_h, y_h)    (t_h, t_l) = two_sum(x_l, y_l)
    /// c = RN(s_l + t_h)                 (v_h, v_l) = two_sum(s_h, c)
    /// w = RN(t_l + v_l)                 result = two_sum(v_h, w)
    /// ```
    ///
    /// Relative error at most 3u²/(1 − 4u) < 3u² + 13u³ (JMP Theorem 3.1; Coq-verified by Muller
    /// and Rideau, whose Property 2.1 shows it asymptotically optimal), over the whole range: the
    /// algorithm only adds, so lemma G of the module makes binary64 compute what the proof's
    /// arithmetic computes, subnormals included. [`ADD_BOUND`] is above it.
    ///
    /// Overflow: an intermediate overflows only if `|x_h + y_h|`, `|s_h + c|` or `|v_h + w|`
    /// reaches 2^1024 − 2^970. Each differs from `x + y` by at most `|x_l| + |y_l| ≤ 2·2^970`
    /// (`|x_l| ≤ ulp(x_h)/2 ≤ 2^970`), or by the proof's small terms (`t_l` and the two roundings,
    /// below 2^922), so it needs `|x + y| ≥ 2^1024 − 3·2^970`. An infinity, once made, reaches the
    /// result (every later step adds it), which is then refused. That refuses some sums a `DD` can
    /// hold, from 2^1024 − 3·2^970 up to 2^1024 − 2^970: (Ω, −2^969) + 2^970 = (Ω, 2^969) is one
    /// (the module documentation, *Never NaN, never infinite*).
    ///
    /// Commutative bit for bit: each step treats the two operands' parts alike, and [`two_sum`] is
    /// commutative (its pair is unique and its zero +0).
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if an intermediate overflows (only when
    /// `|x + y| ≥ 2^1024 − 3·2^970`).
    pub fn add(self, other: DD) -> Result<DD, NumError> {
        let (sh, sl) = two_sum(self.hi, other.hi);
        let (th, tl) = two_sum(self.lo, other.lo);
        let c = sl + th;
        let (vh, vl) = two_sum(sh, c);
        let w = tl + vl;
        DD::normalized(vh, w)
    }

    /// The difference: the sum with the negation of `other`, which is exact. The bound and errors
    /// are [`DD::add`]'s.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] as for [`DD::add`].
    pub fn sub(self, other: DD) -> Result<DD, NumError> {
        self.add(other.negate())
    }

    /// The sum with a double, by `DWPlusFP` (JMP Algorithm 4):
    /// `(s_h, s_l) = two_sum(x_h, y)`, `v = RN(x_l + s_l)`, result `two_sum(s_h, v)`.
    ///
    /// Relative error at most 2u² (JMP Theorem 2.2; Coq-verified by Muller and Rideau), over the
    /// whole range (it only adds; lemma G). [`ADD_F64_BOUND`] is 2u².
    ///
    /// Overflow: only if `|x_h + y|` or `|s_h + v|` reaches 2^1024 − 2^970, each within
    /// `|x_l| + 2^922` of `x + y`, so only when `|x + y| ≥ 2^1024 − 2^971 − 2^922`.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `y` is not finite or an intermediate overflows.
    pub fn add_f64(self, y: f64) -> Result<DD, NumError> {
        if !y.is_finite() {
            return Err(NumError::NotFinite);
        }
        let (sh, sl) = two_sum(self.hi, y);
        let v = self.lo + sl;
        DD::normalized(sh, v)
    }

    /// The difference with a double: [`DD::add_f64`] of `−y`, with its bound and errors.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] as for [`DD::add_f64`].
    pub fn sub_f64(self, y: f64) -> Result<DD, NumError> {
        self.add_f64(-y)
    }

    /// The value scaled by `2^−k` with `k = e(hi)` (or `e(hi)` rounded down to even, for the
    /// root), its trailing part dropped below 2^−400 (module documentation, *Range*), and `k`.
    fn unit_scaled(self, even: bool) -> Result<(DD, i32), NumError> {
        let e = exponent(self.hi)?;
        let k = if even {
            e.div_euclid(2).checked_mul(2).ok_or_else(defect)?
        } else {
            e
        };
        let minus_k = k.checked_neg().ok_or_else(defect)?;
        let hi = scale_pow2(self.hi, minus_k)?;
        let lo = scale_pow2(self.lo, minus_k)?;
        let lo = if magnitude(lo) < TRAIL_FLOOR { 0.0 } else { lo };
        Ok((DD { hi, lo }, k))
    }

    /// A scaled result `(z_h, z_l)` times 2^m: each part correctly rounded by [`scale_pow2`], then
    /// [`two_sum`] (module documentation, *Range*); `NotFinite` on overflow.
    fn scaled_back(z: (f64, f64), m: i32) -> Result<DD, NumError> {
        DD::normalized(scale_pow2(z.0, m)?, scale_pow2(z.1, m)?)
    }

    /// The product with a double, by `DWTimesFP3` (JMP Algorithm 9), on the operands scaled as in
    /// the module's *Range*:
    ///
    /// ```text
    /// (c_h, c_l1) = two_prod_fma(x_h, y)    c_l3 = RN(c_l1 + x_l·y)  (fused multiply-add)
    /// result = two_sum(c_h, c_l3)
    /// ```
    ///
    /// Relative error at most 2u² (JMP Theorem 4.3; Coq-verified by Muller and Rideau) on the
    /// scaled operands, plus the module's 2^−398 for the dropped trailing part, plus an absolute
    /// 2^−1074 from scaling back: [`MUL_F64_BOUND`] and [`UNDERFLOW_BOUND`].
    ///
    /// Range (module documentation): with `x_h, y` in [1, 2) on 2^−52 and `x_l` on 2^−452 (or
    /// zero), `x_h·y` and `x_h·y − c_h` are on 2^−104 and `c_l1 + x_l·y` on 2^−504, all within 8.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `y` is not finite or the result overflows (only when
    /// `|x·y| > (1 − 2^−52)·2^1024`).
    pub fn mul_f64(self, y: f64) -> Result<DD, NumError> {
        if !y.is_finite() {
            return Err(NumError::NotFinite);
        }
        if self.hi == 0.0 || y == 0.0 {
            return Ok(DD::ZERO);
        }
        let (x, kx) = self.unit_scaled(false)?;
        let ky = exponent(y)?;
        let y = scale_pow2(y, ky.checked_neg().ok_or_else(defect)?)?;
        let z = x.times_f64(y)?;
        DD::scaled_back(z, kx.checked_add(ky).ok_or_else(defect)?)
    }

    /// `DWTimesFP3` itself, on operands in the scaled range; the pair before normalization.
    fn times_f64(self, y: f64) -> Result<(f64, f64), NumError> {
        let (ch, cl1) = two_prod_fma(self.hi, y);
        let cl3 = math::fma(self.lo, y, cl1)?.get();
        Ok(two_sum(ch, cl3))
    }

    /// The product, by `DWTimesDW3` (JMP Algorithm 12), on the operands scaled as in the module's
    /// *Range*:
    ///
    /// ```text
    /// (c_h, c_l1) = two_prod_fma(x_h, y_h)    t_l0 = RN(x_l·y_l)
    /// t_l1 = RN(x_h·y_l + t_l0)               c_l2 = RN(t_l1 + x_l·y_h)   (fused multiply-adds)
    /// c_l3 = RN(c_l1 + c_l2)                  result = two_sum(c_h, c_l3)
    /// ```
    ///
    /// Relative error at most (4u² + u³/2)/(1 + u)² < 4u² − 7u³ (Muller and Rideau, Theorem 2.8,
    /// Coq-verified, improving JMP Theorem 5.4's 5u²) on the scaled operands, plus the module's
    /// 2^−398, plus an absolute 2^−1074: [`MUL_BOUND`] and [`UNDERFLOW_BOUND`].
    ///
    /// Range: `x_h·y_h` and its error are on 2^−104; `x_l·y_l` is on 2^−904 and so is its rounding
    /// `t_l0` (lemma R); `x_h·y_l + t_l0` and `t_l1 + x_l·y_h` are on 2^−904 too; all within 8.
    ///
    /// Commutative, bit for bit. The listing is not symmetric in its operands (it rounds
    /// `x_h·y_l + t_l0` before adding `x_l·y_h`), so the two orders can differ in the last bit of
    /// the trailing part, both within the bound. So the scaled operand of the larger magnitude (by
    /// [`Ord`] on the [`DD::abs`] of each) is always taken as `x`. When the two magnitudes are
    /// equal the scaled operands are equal up to sign, and changing the sign of an operand of the
    /// listing negates every nonzero intermediate exactly (rounding to nearest is symmetric; the
    /// sign of a zero may differ, which the final [`two_sum`] and +0 absorb), so both orders give
    /// the same bits. Neither a change of sign nor a power-of-two scaling of an operand changes its scaled
    /// magnitude (unless the scaling rounds a part away), so the order chosen, and with it the
    /// product's symmetry under both, is kept too.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if the result overflows (only when `|x·y| > (1 − 2^−52)·2^1024`).
    pub fn mul(self, other: DD) -> Result<DD, NumError> {
        if self.hi == 0.0 || other.hi == 0.0 {
            return Ok(DD::ZERO);
        }
        let (x, kx) = self.unit_scaled(false)?;
        let (y, ky) = other.unit_scaled(false)?;
        let z = if x.abs() >= y.abs() {
            x.times(y)?
        } else {
            y.times(x)?
        };
        DD::scaled_back(z, kx.checked_add(ky).ok_or_else(defect)?)
    }

    /// `DWTimesDW3` itself, on operands in the scaled range; the pair before normalization.
    fn times(self, y: DD) -> Result<(f64, f64), NumError> {
        let (ch, cl1) = two_prod_fma(self.hi, y.hi);
        let tl0 = self.lo * y.lo;
        let tl1 = math::fma(self.hi, y.lo, tl0)?.get();
        let cl2 = math::fma(self.lo, y.hi, tl1)?.get();
        let cl3 = cl1 + cl2;
        Ok(two_sum(ch, cl3))
    }

    /// The sum with a double by `DWPlusFP`, on operands in the scaled range of [`DD::div`]; the
    /// pair before normalization.
    fn plus_f64(self, y: f64) -> (f64, f64) {
        let (sh, sl) = two_sum(self.hi, y);
        let v = self.lo + sl;
        two_sum(sh, v)
    }

    /// The quotient by a double, by `DWDivFP3` (JMP Algorithm 15), on the operands scaled as in the
    /// module's *Range*:
    ///
    /// ```text
    /// t_h = RN(x_h / y)          (π_h, π_l) = two_prod_fma(t_h, y)
    /// δ_h = x_h − π_h  (exact)   δ_t = δ_h − π_l  (exact)   δ = RN(δ_t + x_l)
    /// t_l = RN(δ / y)            result = two_sum(t_h, t_l)
    /// ```
    ///
    /// Relative error at most 3u² (JMP Theorem 6.2; Coq-verified by Muller and Rideau) on the
    /// scaled operands, plus the module's 2^−398, plus an absolute 2^−1074: [`DIV_F64_BOUND`] and
    /// [`UNDERFLOW_BOUND`]. The two subtractions JMP prove exact (Property 6.1 and Sterbenz's
    /// lemma) are exact here too, since binary64 computes as their arithmetic does.
    ///
    /// Range: `x_h / y` is in (1/2, 2), so `t_h` is on 2^−53; `t_h·y` and its error are on 2^−105;
    /// `δ` is on 2^−452 (lemma R), so it is zero or at least 2^−452, and `δ / y` is zero or above
    /// 2^−454; all within 4.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `y` is zero or not finite, or the result overflows (only when
    /// `|x/y| > (1 − 2^−52)·2^1024`).
    pub fn div_f64(self, y: f64) -> Result<DD, NumError> {
        if !y.is_finite() || y == 0.0 {
            return Err(NumError::NotFinite);
        }
        if self.hi == 0.0 {
            return Ok(DD::ZERO);
        }
        let (x, kx) = self.unit_scaled(false)?;
        let ky = exponent(y)?;
        let y = scale_pow2(y, ky.checked_neg().ok_or_else(defect)?)?;
        let th = x.hi / y;
        let (ph, pl) = two_prod_fma(th, y);
        let dh = x.hi - ph;
        let dt = dh - pl;
        let d = dt + x.lo;
        let tl = d / y;
        DD::scaled_back(two_sum(th, tl), kx.checked_sub(ky).ok_or_else(defect)?)
    }

    /// The quotient `self / other`, by `DWDivDW3` (JMP Algorithm 18), on the operands scaled as in
    /// the module's *Range*:
    ///
    /// ```text
    /// t_h = RN(1 / y_h)
    /// r_h = RN(1 − y_h·t_h)  (fused multiply-add; exact)    r_l = −RN(y_l·t_h)
    /// (e_h, e_l) = two_sum(r_h, r_l)
    /// (δ_h, δ_l) = DWTimesFP3(e, t_h)                         (as in DD::mul_f64)
    /// (m_h, m_l) = DWPlusFP(δ, t_h)                           (as in DD::add_f64)
    /// result = DWTimesDW3(x, m)                              (as in DD::mul)
    /// ```
    ///
    /// One Newton step refines `t_h ≈ 1/y` to `m`, which multiplies `x`. Relative error at most
    /// 9.8u² (JMP Theorem 7.2, for 53 ≥ 13 bits; Coq-verified by Muller and Rideau) on the scaled
    /// operands, plus the module's 2^−398, plus an absolute 2^−1074: [`DIV_BOUND`] and
    /// [`UNDERFLOW_BOUND`]. JMP's proof bounds the last product by Theorem 5.4's 5u², which
    /// `DWTimesDW3` meets (Muller and Rideau: 4u²).
    ///
    /// Range: `1/y_h` is in (1/2, 1], so `t_h` is on 2^−53; `1 − y_h·t_h` is on 2^−105 and exact
    /// (JMP Property 6.1); `y_l·t_h` is on 2^−505, and so are `r_l`, `e_h` and `e_l` (lemma R);
    /// `e_h·t_h`, its error and `c_l1 + e_l·t_h` are on 2^−558, and so are `δ` and `m`; in the last
    /// product `x_h·m_h` is on 2^−610, `x_l·m_l` and the fused multiply-adds on 2^−1010; all within
    /// 8.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if `other` is zero or the result overflows (only when
    /// `|x/y| > (1 − 2^−52)·2^1024`).
    pub fn div(self, other: DD) -> Result<DD, NumError> {
        if other.hi == 0.0 {
            return Err(NumError::NotFinite);
        }
        if self.hi == 0.0 {
            return Ok(DD::ZERO);
        }
        let (x, kx) = self.unit_scaled(false)?;
        let (y, ky) = other.unit_scaled(false)?;
        let th = 1.0 / y.hi;
        let rh = math::fma(-y.hi, th, 1.0)?.get();
        let rl = -(y.lo * th);
        let (eh, el) = two_sum(rh, rl);
        let (dh, dl) = DD { hi: eh, lo: el }.times_f64(th)?;
        let (mh, ml) = DD { hi: dh, lo: dl }.plus_f64(th);
        let z = x.times(DD { hi: mh, lo: ml })?;
        DD::scaled_back(z, kx.checked_sub(ky).ok_or_else(defect)?)
    }

    /// The square root, by `SQRTDWtoDW` (Lefèvre, Louvet, Muller, Picot and Rideau, Algorithm 8),
    /// on the value scaled by an even power of two so that its leading part lies in [1, 4):
    ///
    /// ```text
    /// s_h = RN(√x_h)            (crate::math::sqrt, correctly rounded)
    /// ρ_1 = RN(x_h − s_h²)      (fused multiply-add; exact, their Lemma 1.5)
    /// ρ_2 = RN(x_l + ρ_1)       s_l = RN(ρ_2 / (2·s_h))
    /// result = two_sum(s_h, s_l)
    /// ```
    ///
    /// Relative error at most 25u²/8 (their Theorem 3.6, Coq-verified, asymptotically optimal),
    /// which needs the operand at least 2^(`e_min` + p) = 2^−969: the scaled operand is at least 1.
    /// Dropping a trailing part below 2^−400 changes the root by less than 2^−400 relatively, and
    /// scaling the result back by 2^j (j from −537 to 511) is exact for both parts of
    /// `two_sum(s_h, s_l)`. `s_h` is in [1, 2) and `s_l` is zero or at least 2^−454 (Range,
    /// below). If the sum rounds to `s_h`, the parts are `s_h` and `s_l` themselves. Otherwise
    /// the sum moved `s_h`, which needs `|s_l| ≥ 2^−54` (half the unit of the doubles just below
    /// 1), so the leading part is in [1/2, 2] and the trailing part, the exact error of the sum,
    /// is zero or a whole multiple of the unit of `s_l`, at least 2^−106 (`s_h` and the rounded
    /// sum lie on coarser grids). So each part times 2^j is zero or at least 2^−991, a normal
    /// double, and the bound holds relative alone, below [`SQRT_BOUND`], with no absolute part.
    ///
    /// Range: `√x_h` is in [1, 2); `x_h − s_h²` is on 2^−104; `ρ_2` is on 2^−452 (lemma R), so
    /// `ρ_2 / (2·s_h)` is zero or above 2^−454; all within 4. A root never overflows.
    ///
    /// # Errors
    ///
    /// [`NumError::OutsideDomain`] if the value is negative.
    pub fn sqrt(self) -> Result<DD, NumError> {
        if self.hi < 0.0 {
            return Err(NumError::OutsideDomain);
        }
        if self.hi == 0.0 {
            return Ok(DD::ZERO);
        }
        let (x, k) = self.unit_scaled(true)?;
        let sh = math::sqrt(x.hi)?.get();
        let rho1 = math::fma(-sh, sh, x.hi)?.get();
        let rho2 = x.lo + rho1;
        let sl = rho2 / (2.0 * sh);
        DD::scaled_back(two_sum(sh, sl), k.div_euclid(2))
    }

    /// The exact real `hi + lo − offset` rounded to the nearest integer, ties to even, with whether
    /// it was already an integer; `None` if `|hi| > 2^100` or the result does not fit an `i128`.
    ///
    /// No step rounds:
    ///
    /// 1. `r_h = rint(hi)` and `d_h = hi − r_h`, and likewise `r_l`, `d_l` for `lo`. For any double
    ///    `v`, `v − rint(v)` is exact: for `|v| < 1/2` or `rint(v) = 0` it is `v`; for
    ///    `|v| ≥ 2^52` it is 0; otherwise `rint(v)` is a nonzero integer within 1/2 of `v`, so
    ///    `rint(v)/2 ≤ v ≤ 2·rint(v)` and Sterbenz's lemma applies. So `|d_h|, |d_l| ≤ 1/2` and
    ///    `hi + lo = r_h + r_l + d_h + d_l` exactly.
    /// 2. `s = r_h + r_l`, in `i128`, with a checked step (`|r_h| ≤ 2^100` and `|r_l| ≤ 2^47`,
    ///    half a unit of `hi`; each is converted from its bits, exactly).
    /// 3. `(t, t_e) = two_sum(d_h, d_l)`: the fraction `F = d_h + d_l = t + t_e` exactly,
    ///    `|F| ≤ 1`.
    /// 4. The value is `n + F` with `n = s − offset`. `F` is compared with ±1/2 exactly: `t` is `F`
    ///    rounded to nearest, and rounding is monotone, so `t > 1/2` implies `F > 1/2` and
    ///    `t < 1/2` implies `F < 1/2`; when `t = 1/2` the sign of `t_e` decides (likewise at −1/2).
    ///    So `n + F` rounds to `n + a` with `a` = −1, 0 or +1, a tie at `F = ±1/2` going to the even
    ///    neighbour; it was an integer exactly when `F` is 0 or ±1.
    /// 5. `a` is decided before anything is subtracted, because `n`, or the neighbour of `n` not
    ///    chosen, need not fit an `i128` when the result does: `hi = 3.5`, `lo = −2^−60` and
    ///    `offset = i128::MIN + 4` give `s = 4`, `F = −1/2 − 2^−60` and `n = 2^127`, which rounds
    ///    down to `i128::MAX`; `hi = −4.5` and `offset = i128::MAX − 3` give `n = i128::MIN` and a
    ///    tie that goes to `n` itself, the even neighbour, while `n − 1` does not fit. A tie needs
    ///    only the parity of `n`, which is the parity of `s + offset`: the lowest bits of `s` and
    ///    `offset` combined by exclusive or (two's complement, so this holds for negative values
    ///    too), a step that cannot overflow. Then the result is `(s + a) − offset`, checked:
    ///    `|s + a| ≤ 2^100 + 2^47 + 1` is far inside `i128`, and the subtraction fails exactly when
    ///    the result does not fit.
    ///
    /// From G03's `geom-math` double-double, re-checked, with the conversions to integers done from
    /// the bits instead of by casts, and with step 5 in place of G03's order (subtract `offset`
    /// first, then form both neighbours), which refused some results at the ends of `i128`.
    #[must_use]
    pub fn round_shifted(self, offset: i128) -> Option<Rounded<i128>> {
        if magnitude(self.hi) > ROUND_LIMIT {
            return None;
        }
        let rh = math::round_ties_even(self.hi).ok()?.get();
        let rl = math::round_ties_even(self.lo).ok()?.get();
        let (dh, dl) = (self.hi - rh, self.lo - rl);
        let s = whole(rh)?.checked_add(whole(rl)?)?;
        let (t, te) = two_sum(dh, dl);
        // How F = t + t_e compares with c (c = ±1/2, an exact double).
        let above = |c: f64| t > c || (t == c && te > 0.0);
        let equal = |c: f64| t == c && te == 0.0;
        // Whether n = s − offset is odd: n ≡ s + offset (mod 2), so their lowest bits decide. A tie
        // goes to the even neighbour: n itself, or n ± 1 when n is odd.
        let n_odd = (s ^ offset) & 1 == 1;
        let step: i128 = if above(0.5) {
            1
        } else if equal(0.5) {
            i128::from(n_odd)
        } else if equal(-0.5) {
            if n_odd { -1 } else { 0 }
        } else if above(-0.5) {
            0
        } else {
            -1
        };
        let value = s.checked_add(step)?.checked_sub(offset)?;
        let exact = te == 0.0 && (t == 0.0 || t == 1.0 || t == -1.0);
        Some(Rounded::new(value, exact))
    }
}

/// An integral double of magnitude at most 2^100 as an `i128`, exactly, from its bits: `v` is
/// `±f·2^(E − 1075)` with `f` the 53-bit significand (implicit bit included) and E the exponent
/// field; for E ≥ 1075 that is `f` shifted left by `E − 1075 ≤ 48` places, below 2^101, and
/// otherwise `f` shifted right by `1075 − E ≤ 52` places (v is an integer, so no set bit is shifted
/// out; every subnormal and every value below 1 is 0 here, being integral only as 0). `None` if `v`
/// is not such a value.
fn whole(v: f64) -> Option<i128> {
    if v == 0.0 {
        return Some(0);
    }
    let bits = v.to_bits();
    let field = i32::from(u16::try_from((bits >> 52) & 0x7ff).ok()?);
    if !(1023..=1123).contains(&field) {
        return None;
    }
    let significand = i128::from((bits & FRACTION_BITS) | (1 << 52));
    let magnitude = if field >= 1075 {
        significand.checked_shl(u32::try_from(field.checked_sub(1075)?).ok()?)?
    } else {
        let shift = u32::try_from(1075_i32.checked_sub(field)?).ok()?;
        let m = significand.checked_shr(shift)?;
        if m.checked_shl(shift)? != significand {
            return None;
        }
        m
    };
    if v < 0.0 {
        magnitude.checked_neg()
    } else {
        Some(magnitude)
    }
}

impl Neg for DD {
    type Output = DD;

    /// [`DD::negate`]: exact.
    fn neg(self) -> DD {
        self.negate()
    }
}

/// Equal exactly when the bits are, which by the invariant (one bit pattern per value) is exactly
/// when the values are equal.
impl PartialEq for DD {
    fn eq(&self, other: &Self) -> bool {
        self.hi.to_bits() == other.hi.to_bits() && self.lo.to_bits() == other.lo.to_bits()
    }
}

impl Eq for DD {}

/// Hashes the bits, so equal values hash equally.
impl Hash for DD {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hi.to_bits().hash(state);
        self.lo.to_bits().hash(state);
    }
}

impl PartialOrd for DD {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The order of the values: by `hi`, then by `lo`.
///
/// Why that is the order of the values: if `x < y` then `x_h ≤ y_h` (rounding is monotone), and if
/// `x_h < y_h` then `x < y` (were `x ≥ y`, monotonicity would give `x_h ≥ y_h`). So the values
/// order as the `hi`s do when those differ, and as the `lo`s do when they are equal. Both parts
/// are finite and never −0, where IEEE totalOrder (`total_cmp`) is the numeric order.
impl Ord for DD {
    fn cmp(&self, other: &Self) -> Ordering {
        self.hi
            .total_cmp(&other.hi)
            .then_with(|| self.lo.total_cmp(&other.lo))
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

    #[test]
    fn helpers_on_their_edges() {
        assert_eq!(exponent(1.0).unwrap(), 0);
        assert_eq!(exponent(f64::MAX).unwrap(), 1023);
        assert_eq!(exponent(f64::MIN_POSITIVE).unwrap(), -1022);
        assert_eq!(exponent(f64::from_bits(1)).unwrap(), -1074);
        assert_eq!(exponent(-f64::from_bits(1 << 51)).unwrap(), -1023);
        for k in -1074..=1023 {
            assert_eq!(exponent(pow2(k).unwrap()).unwrap(), k);
        }
        assert!(pow2(1024).is_err() && pow2(-1075).is_err());
        // 1.5·2^−1075 = 0.75·2^−1074 rounds up; 2^−1075 is a tie, to the even 0; 1.5·2^−1074 is a
        // tie, to the even 2·2^−1074; 3·2^−1076 − ε, through the pre-scaled path, rounds down.
        assert_eq!(scale_pow2(1.5, -1075).unwrap(), f64::from_bits(1));
        assert_eq!(scale_pow2(1.0, -1075).unwrap(), 0.0);
        assert_eq!(scale_pow2(1.5, -1074).unwrap(), f64::from_bits(2));
        assert_eq!(scale_pow2(1.5 - 2.0f64.powi(-52), -1076).unwrap(), 0.0);
        assert_eq!(
            scale_pow2(1.0 + 2.0f64.powi(-52), -1075).unwrap(),
            f64::from_bits(1)
        );
        assert_eq!(scale_pow2(1.5, -2000).unwrap(), 0.0);
        assert_eq!(scale_pow2(1.0, 1024).unwrap(), f64::INFINITY);
        assert_eq!(
            scale_pow2(f64::from_bits(1), 2097).unwrap(),
            2.0f64.powi(1023)
        );
        assert_eq!(whole(2.0f64.powi(100)), Some(1 << 100));
        assert_eq!(whole(-3.0), Some(-3));
        assert_eq!(whole(0.5), None);
    }
}
