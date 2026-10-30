//! Correctly rounded natural logarithm `ln(x)`: a port of CORE-MATH.
//!
//! Method: Ziv's strategy, as in CORE-MATH (<https://core-math.gitlabpages.inria.fr/>); the
//! algorithm and the proof of its bound are below.
//! Source: PORT from CORE-MATH `src/binary64/log/log.c` and `src/binary64/log/dint.h` at commit
//!   `0ce0cbc15935e24c656280e5a46c34e89f301b7a`, by Paul Zimmermann and Tom Hubrecht (log.c) and
//!   Tom Hubrecht (dint.h, from his correctly rounded `pow`), MIT,
//!   <https://gitlab.inria.fr/core-math/core-math/-/blob/0ce0cbc15935e24c656280e5a46c34e89f301b7a/src/binary64/log/log.c>
//!   Copyright (c) 2022 INRIA and CERN.
//!   Authors: Paul Zimmermann and Tom Hubrecht.
//!   <https://gitlab.inria.fr/core-math/core-math/-/blob/0ce0cbc15935e24c656280e5a46c34e89f301b7a/src/binary64/log/dint.h>
//!   Copyright (c) 2022 CERN.
//!   Author: Tom Hubrecht
//!   Changed for Spare Kit: translated to safe `no_std` Rust, with the 128-bit arithmetic in
//!   `u128` and `u64` steps under the crate's rule (`lib.rs`) and total shifts; round-to-nearest
//!   only, without the exception flags or `errno`; the canonical NaN for a NaN or a negative input;
//!   the fast path's seven `fma`s replaced by plain operations, with its bound re-derived; a Taylor
//!   series in the accurate path just below 1; the accurate path's tables trimmed to the entries
//!   its index reaches; a written proof of the error bound. Every change is listed under "Changes
//!   from the C source" below; none changes a correctly rounded result.
//! SPDX-License-Identifier: MPL-2.0 AND MIT
//!
//! The original notices of `log.c` and `dint.h`, reproduced as the MIT licence requires, each as
//! published (`dint.h`'s reads `exp_dRESS` where `log.c`'s reads `EXPRESS`):
//!
//! ```text
//! log.c:
//! Copyright (c) 2022 INRIA and CERN.
//! Authors: Paul Zimmermann and Tom Hubrecht.
//!
//! This file is part of the CORE-MATH project
//! (https://core-math.gitlabpages.inria.fr/).
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.
//!
//! dint.h:
//! Copyright (c) 2022 CERN.
//! Author: Tom Hubrecht
//!
//! This file is part of the CORE-MATH project
//! (https://core-math.gitlabpages.inria.fr/).
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, exp_dRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.
//! ```
//!
//! Guarantees: correctly rounded, resting on the completeness of the published worst-case
//! lists, which their own statement hedges (step A of the proof). For every binary64 input checked
//! the result is the binary64 number nearest the exact `ln x` (ties to even), so the error is at
//! most half an ulp. Proven outright: every input whose value has fewer than 55 identical bits
//! after its round bit, and every input in CORE-MATH's log.wc (134 951, all checked), is correctly
//! rounded; an unlisted input with more, which no proof here excludes, would still have an error
//! below `½ + 2^−56.6` ulp. Domain: every `x ≥ 0`, subnormals included. `ln(1) = +0`,
//! `ln(±0) = −∞`, `ln(+∞) = +∞`, and a negative input or NaN gives the canonical NaN
//! (`0x7ff8_0000_0000_0000`). The proof is at the end of this documentation.
//!
//! Cost: constant time, no allocation and no recursion. Almost every argument takes the fast
//! path, a fixed sequence of floating-point operations and table reads; only when its rounding
//! test cannot decide does the accurate path run, also of fixed length, with a fixed-degree polynomial in double-double. No loop's
//! length depends on the argument.
//!
//! The algorithm (Ziv's strategy, as CORE-MATH). Fast path: `x = 2^e·y` with `y` in `[1/√2, √2)`; a
//! 10-bit `r ≈ 1/y` from a table makes `z = r·y − 1` exact with `|z| < 0.00213`; then
//! `ln x = e·ln 2 − ln r + ln(1 + z)`, with `−ln r` from a double-double table and `ln(1 + z)` from a
//! degree-6 polynomial. If `h + l ± ERR` round to the same double, that is the result. Otherwise
//! the accurate path recomputes in 128-bit floating point ([`Dint`]) with an 8-bit table reduction
//! and a degree-13 polynomial, and rounds that result. The hardest case,
//! `x = 0x1.62a88613629b6p+678`, has 64 identical bits after the round bit (Lefèvre and Muller's
//! exhaustive search, <https://www.vinc17.net/research/testlibm/>, reproduced in CORE-MATH's
//! `log.wc`).
//!
//! Changes from the C source, none of which changes a correctly rounded result:
//!
//! - Only round-to-nearest is implemented (Rust has no other rounding mode, RFC 3514); the
//!   floating-point exception flags and `errno` are not kept.
//! - NaN, negative inputs and the poles return their IEEE values directly; NaN is the canonical
//!   NaN instead of `x + x` or `0/0` (chapter 21).
//! - **Fewer fused operations in the fast path.** The C source's seven `fma`s there become plain
//!   operations, because `libm::fma` is a call through a function pointer on x86-64 (and software
//!   in WebAssembly) and kept this function above twice the time of `libm::log`. Two of them give
//!   the same value exactly: `z = r·y − 1` is computed as `(r·y_hi − 1) + r·y_lo` with `y` split
//!   after its top 43 significand bits (`r` has at most 10, so both products are exact, the
//!   subtraction is exact by Sterbenz, and so is the sum, `z` fitting in 53 bits), and
//!   `e·LOG2_H + l1` is exact either way. The other five (four Horner steps and `e·LOG2_L + l`)
//!   now round twice, which step F3 accounts for; the bound changes by less than 0.001 in its
//!   binary logarithm, and `ERR` still covers it. As before, the fast path returns only a result
//!   its test proves correctly rounded, so no result changes.
//! - The accurate path is kept out of line (`#[cold]`, `#[inline(never)]`), so that the fast
//!   path's code does not save the registers the 128-bit arithmetic needs.
//! - The 128-bit arithmetic uses Rust's `u128` and `u64` with checked operations, total shifts
//!   ([`super::support`]) and the halves of a `u128` read from its bytes instead of casts. Every
//!   step the C code does in unsigned or signed arithmetic is one that cannot overflow here (each
//!   says why), so where the C would wrap had its proof been wrong, this port's checked step ends
//!   the computation with the canonical NaN instead, which no input reaches (the tests run every
//!   listed hard case through it); that includes the two table indices, `i − 362` and `i − 181`.
//!   One kind of step keeps the C's modular arithmetic on purpose: a limb sum whose carry is kept
//!   (`overflowing_add`), as `crate::wide` keeps its own. One consequence is written out: the
//!   aligned-operand rounding step `B + bit` cannot overflow to zero here when `B` is all ones
//!   (where C's would); that case does not arise in this function's evaluation, and elsewhere the
//!   result is identical.
//! - **A Taylor series just below 1.** For `x ∈ [1 − 2^−8, 1)` the accurate path's table entry
//!   is exactly 1, so its argument is `z = x − 1 < 0`, where CORE-MATH's polynomial (fitted for
//!   `z ≥ 0`) has a relative error up to 2^−100.8: not enough for the worst-case argument with the
//!   published lists (see the proof). This port evaluates the Taylor series of `ln(1 + z)` to
//!   degree 16 there instead ([`log1p_taylor`]), with error below 2^−120.
//! - The accurate path's two 240-entry tables are trimmed to the 182 entries the index can reach
//!   (53 to 234), as this crate keeps no table larger than needed.
//!
//! # Proof of the error bound
//!
//! The lemmas cited are those of [`super`]. Numbers marked *checked* are computed with exact
//! arithmetic (exhaustively over the tables) by the unit test named in parentheses. `u = 2^−53`.
//!
//! **F. The fast path.** (F1) For every one of the 363 intervals `i·2^−9 ≤ y < (i + 1)·2^−9`,
//! `z = r·y − 1` fits in 53 bits, so the C source's `fma` computes it exactly, and so does this
//! port's `(r·y_hi − 1) + r·y_lo` (every `r` has at most 10 significant bits), and
//! `−0.00202941894531250 ≤ z ≤ 0.00212097167968735`; `LOG2_H` and `l1` are multiples of `2^−42`, so
//! `e·LOG2_H` and `e·LOG2_H + l1` are exact; and `|l1 + l2 + ln r| ≤ 2^−96` (2^−97.007 measured)
//! [*checked*: `fast_tables`]. The fast two-sum of `e·LOG2_H + l1` and `z` is exact: the first is 0
//! or at least `0.0029` in magnitude, the second at most `0.00213`. (F2) The polynomial's
//! approximation error on that interval is at most `7.1·10^−22` (2^−70.256 by subdivision;
//! CORE-MATH's Sollya figure is `6.98575·10^−22`), and `|ln 2 − LOG2_H − LOG2_L| ≤ 1.95853·10^−31`
//! [*checked*: `fast_polynomial_and_log2`]. (F3) CORE-MATH bounds `|h + l − ln x|` by `2^−68.22` by
//! hand (log.c's comments) and by `2.8925·10^−21` with Gappa (its `log1_template.g`), both for the
//! fused evaluation. Re-deriving the hand analysis with rounding errors of `u·|a|` instead of
//! `ulp(a)`, and with this port's unfused Horner steps and `e·LOG2_L + l` (each adding the rounding
//! of its product, `u·|a·z|` or `u·|e·LOG2_L|`), gives 2^−68.79. (F4) Neither analysis covers the
//! rounding of `l ± ERR` in the test; with it (Lemma 1, `|l| < 2^−18.69`), the requirement is
//! 2^−68.62 ≤ `ERR·(1 − u)` = 2^−68.23, so CORE-MATH's `ERR` is proven sufficient (*checked*:
//! `fast_path_bound`).
//!
//! **A. The accurate path** has no rounding test. Its relative error is at most 2^−109.67 (A1 and
//! A2 below), below `2^−56.6·ulp`, so by Lemma 2 it rounds correctly, unconditionally, every input
//! whose value has fewer than 55 identical bits after its round bit. CORE-MATH's log.wc holds
//! Lefèvre's hard cases for the logarithm (which his page describes as not all those he found, but
//! at least the most important) and further `BaCSeL` results, and says "we should have all worst
//! cases with 47 identical bits or more in this file"; all 134 951 listed inputs are correctly
//! rounded. Inputs with 55 or more identical bits exist (Lefèvre's hardest has 64, and is
//! listed); that the list holds every one of them is that hedged statement, not a proof. An
//! unlisted one would be rounded from a value within `2^−56.6·ulp` of the exact one, an error below
//! `½ + 2^−56.6` ulp. By Lemma 2 with the list's threshold, the path is correctly rounded if its
//! relative error is below `2^−101`. (A1) For each of the 182 reachable
//! table entries, `LOG_INV_2` is within 2^−129 of `−ln(INVERSE_2)`, which is all that matters:
//! `ln x = E·ln 2 + ln(1 + z) − ln(inv)` holds for any `inv`. (A2) For each entry, on its interval
//! of reduced arguments: the polynomial's approximation error (subdivision, on the side of zero
//! that `z` takes there); Horner's running error with `mul_dint` below 5 ulps of its result and
//! `add_dint` below 2 ulps of its larger operand (the truncated part of the smaller operand, with
//! no renormalisation in this file's version); the errors of `z` itself from `mul_dint(x, inv)`
//! and the subtraction of 1 (none when `inv = 1`, around `x = 1`); the two final additions; and,
//! for `E ≠ 0`, `LOG2`'s error (below 2^−129.97) times `|E| ≤ 1075` and `mul_dint_2`. Divided by
//! the smallest `|ln x|` the entry can produce (`|ln x| ≥ 0.346` when `E ≠ 0`; relative to `|z|`
//! where `inv = 1`), the error is at most 2^−109.67 (*checked*: `accurate_path_bound`). ∎
//!
//! Without this port's change for `x ∈ [1 − 2^−8, 1)`, the accurate path there would evaluate
//! `P2_COEFFS` at `z ∈ [−2^−8, 0)`: the polynomial was fitted for `z ≥ 0`, and its error at
//! `z = −2^−8` is 2^−108.78, a relative error of 2^−100.8, which Lemma 2 does not cover with the
//! published threshold `t = 47`.

use core::cmp::Ordering;

use super::hexf::hf;
use super::support::{
    entry, fast_two_sum, halves, hi64, join, lo64, pow2, shl, shl128, shr128, signed,
};

/// The 52 significand bits of a binary64 number.
const MANTISSA: u64 = (1 << 52) - 1;

/// The fast path's bound for the rounding test: CORE-MATH's value (its Gappa bound
/// 2.89253666698316e−21 rounded up), proven sufficient, with the test's own rounding, in steps F3
/// and F4 of the proof.
const ERR: f64 = hf!("0x1.b6p-69");

/// `ln(x)` for `1 ≤ v < 2` and `x = 2^e·v` in double-double, `|h + l − ln x| < ERR`
/// (C: `cr_log_fast`).
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the C source's variables, so the two can be read side by side"
)]
fn log_fast(e: i64, v: f64) -> Option<(f64, f64)> {
    /// `z + P[1]·z² + … + P[5]·z⁶` approximates `ln(1 + z)` on
    /// `[−0.00202941894531250, 0.00212097167968735]` with absolute error below 2^−70.278.
    const P: [f64; 6] = [
        hf!("0x1p0"),
        hf!("-0x1.ffffffffffffap-2"),
        hf!("0x1.555555554f4d8p-2"),
        hf!("-0x1.0000000537df6p-2"),
        hf!("0x1.999a14758b084p-3"),
        hf!("-0x1.55362255e0f63p-3"),
    ];
    /// `ln 2 ≈ LOG2_H + LOG2_L` to 2^−102.01; `LOG2_H` is a multiple of 2^−42, so `e·LOG2_H` is
    /// exact.
    const LOG2_H: f64 = hf!("0x1.62e42fefa38p-1");
    /// See [`LOG2_H`].
    const LOG2_L: f64 = hf!("0x1.ef35793c7673p-45");

    // v = m/2^52. C: `(1 << 52) + field`, a sum of disjoint bit fields, which is their union.
    let m = (1u64 << 52) | (v.to_bits() & MANTISSA);
    // Above √2, halve v to avoid cancellation.
    let above = m >= 0x16_a09e_667f_3bcd;
    // C: `e += above`; |e| ≤ 1074, so the checked sum cannot overflow. Were it to, the caller's
    // NaN pair fails the rounding test and the accurate path, which starts from x, gives the result.
    let e = e.checked_add(i64::from(above))?;
    let (i, y) = if above {
        (m >> 44, v * 0.5)
    } else {
        (m >> 43, v)
    };
    // 362 ≤ i ≤ 724: i·2^−9 ≤ y < (i + 1)·2^−9. C: `INVERSE[i − 362]`. The checked difference
    // cannot fail (i ≥ 362); were it to, the caller's NaN pair fails the rounding test and the
    // accurate path, which starts from x, gives the result, as for `e` above.
    let k = i.checked_sub(362)?;
    let r = entry(&INVERSE, k);
    let [l1, l2] = entry(&LOG_INV, k);
    // z = r·y − 1 exactly, the C source's fma(r, y, −1) without a fused operation: r has at
    // most 10 significant bits, so with y split into its top 43 and low 10 significand bits both
    // products are exact; r·y_hi is within 2^−6 of 1, so subtracting 1 is exact (Sterbenz); and z
    // fits in 53 bits (proof, F1), so the last addition is exact too.
    let y_hi = f64::from_bits(y.to_bits() & !0x3ff);
    let y_lo = y - y_hi;
    let z = (r * y_hi - 1.0) + r * y_lo;
    let z2 = z * z;
    // The polynomial, unfused where the C source has four fma (proof, F3).
    let p45 = P[5] * z + P[4];
    let p23 = P[3] * z + P[2];
    let ph = p45 * z2 + p23;
    let ph = ph * z + P[1];
    let ph = ph * z2;
    #[allow(
        clippy::cast_precision_loss,
        reason = "|e| ≤ 1075, far below 2^53, so the conversion is exact"
    )]
    let ee = e as f64;
    // e·LOG2_H + l1 is exact (proof, F1), so this unfused sum is the C source's fma.
    let (h, l) = fast_two_sum(ee * LOG2_H + l1, z);
    let l = ph + (l + l2);
    let l = ee * LOG2_L + l; // C: fma(e, LOG2_L, l)
    Some((h, l))
}

/// Natural logarithm, correctly rounded.
///
/// Error at most half an ulp for every input checked, and proven below `½ + 2^−56.6` ulp for every
/// input; see the module documentation for the method, the source, the proof and what correct
/// rounding rests on. `ln(1) = +0`, `ln(±0) = −∞`, `ln(+∞) = +∞`; negative inputs and NaN give
/// the canonical NaN.
#[must_use]
pub fn ln(x: f64) -> f64 {
    ln_checked(x).unwrap_or(super::NAN)
}

/// [`ln`], `None` only if one of its integer steps failed, which no double makes (each is noted
/// below): [`ln`] then returns NaN, never a plausible wrong value.
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the C source's variables, so the two can be read side by side"
)]
fn ln_checked(x: f64) -> Option<f64> {
    let mut bits = x.to_bits();
    // The exponent field with the sign bit above it, below 2^12, so `signed` reads its value, and
    // the checked subtraction cannot overflow (C: `(int64_t)(bits >> 52) − 0x3ff`).
    let mut e = signed(bits >> 52).checked_sub(0x3ff)?;
    if e >= 0x400 || e == -0x3ff {
        // x ≤ 0, NaN, ±∞, or subnormal
        if x.is_nan() || x < 0.0 {
            return Some(super::NAN); // includes −∞
        }
        if x == f64::INFINITY {
            return Some(x);
        }
        if x == 0.0 {
            return Some(f64::NEG_INFINITY); // pole
        }
        // subnormal: scale into the normal range
        bits = (x * hf!("0x1p52")).to_bits();
        e = signed(bits >> 52).checked_sub(0x3ff + 52)?;
    }
    // x = 2^e·v with 1 ≤ v < 2 and −1074 ≤ e ≤ 1023
    let v = f64::from_bits((0x3ff << 52) | (bits & MANTISSA));
    if v == 1.0 && e == 0 {
        return Some(0.0);
    }
    // A failed step in the fast path gives the NaN pair, which fails the rounding test below.
    let (h, l) = log_fast(e, v).unwrap_or((super::NAN, super::NAN));
    let left = h + (l - ERR);
    let right = h + (l + ERR);
    if left == right {
        return Some(left);
    }
    // CORE-MATH measures the fast path's failure rate at about 2^−11.5.
    Some(log_accurate(x))
}

/// The accurate path (C: `cr_log_accurate`): `ln x` in 128-bit floating point, then rounded.
#[cold]
#[inline(never)]
fn log_accurate(x: f64) -> f64 {
    // `log` reads its tables in range by construction, and its integer steps cannot overflow;
    // were either ever broken, the result is NaN, never a plausible wrong value (see
    // `support::entry`).
    Dint::from_f64(x)
        .and_then(Dint::log)
        .map_or(super::NAN, Dint::to_f64)
}

/// A 128-bit floating-point number `(−1)^sgn · (hi + lo/2^64) · 2^(ex − 63)` (C: `dint64_t`).
///
/// Normalised numbers have the top bit of `hi` set, so `2^ex ≤ |value| < 2^(ex+1)`; zero has
/// `hi = lo = 0`.
#[derive(Clone, Copy, Default)]
struct Dint {
    /// High 64 bits of the significand.
    hi: u64,
    /// Low 64 bits of the significand.
    lo: u64,
    /// Exponent.
    ex: i64,
    /// Sign: 0 for positive, 1 for negative.
    sgn: u64,
}

/// Shorthand for the tables.
const fn d(hi: u64, lo: u64, ex: i64, sgn: u64) -> Dint {
    Dint { hi, lo, ex, sgn }
}

/// Zero.
const ZERO: Dint = d(0, 0, 0, 0);
/// −1.
const M_ONE: Dint = d(0x8000_0000_0000_0000, 0, 0, 1);
/// ln 2 with absolute error below 2^−129.97.
const LOG2: Dint = d(0xb172_17f7_d1cf_79ab, 0xc9e3_b398_03f2_f6af, -1, 0);

impl Dint {
    /// The significand as one 128-bit integer.
    const fn significand(self) -> u128 {
        join(self.hi, self.lo)
    }

    /// A number from a significand and the other fields.
    const fn with_significand(s: u128, ex: i64, sgn: u64) -> Dint {
        let (lo, hi) = halves(s);
        Dint { hi, lo, ex, sgn }
    }

    /// Whether the value is zero.
    const fn is_zero(self) -> bool {
        self.hi | self.lo == 0
    }

    /// Compares magnitudes, assuming both are normalised and non-zero (C: `cmp_dint`).
    fn cmp_magnitude(self, b: Dint) -> Ordering {
        self.ex
            .cmp(&b.ex)
            .then(self.hi.cmp(&b.hi))
            .then(self.lo.cmp(&b.lo))
    }

    /// `self + b` with an error below 2 ulps of the result (C: `add_dint`). `None` only if an
    /// exponent or a rounding step overflowed, which no operand of this file's evaluation makes.
    fn add(self, b: Dint) -> Option<Dint> {
        if self.is_zero() {
            return Some(b);
        }
        if b.is_zero() {
            return Some(self);
        }
        let (a, b) = match self.cmp_magnitude(b) {
            Ordering::Equal => {
                if self.sgn ^ b.sgn != 0 {
                    return Some(ZERO);
                }
                return Some(Dint {
                    ex: self.ex.checked_add(1)?,
                    ..self
                });
            }
            Ordering::Less => (b, self),
            Ordering::Greater => (self, b),
        };
        // Now |a| > |b|.
        let big_a = a.significand();
        let mut big_b = b.significand();
        let mut m_ex = a.ex;
        if a.ex > b.ex {
            let sh = a.ex.abs_diff(b.ex); // a.ex − b.ex ≥ 1
            // C: B += 1 & (B >> (sh − 1)) for sh ≤ 128, then B >>= sh (or B = 0 for sh ≥ 128).
            // The addition can only carry out of 128 bits when B is all ones; then the shifted
            // value is 2^(128−sh), which C's wrapped sum would have lost.
            if sh < 128 {
                let bit = shr128(big_b, sh.abs_diff(1)) & 1;
                big_b = match big_b.checked_add(bit) {
                    Some(v) => shr128(v, sh),
                    None => shl128(1, 128u64.abs_diff(sh)),
                };
            } else {
                big_b = 0;
            }
        }
        let c = if a.sgn ^ b.sgn != 0 {
            // C: unsigned subtraction. |a| > |b| and b is aligned to a's exponent (and rounded to
            // at most 2^127 ≤ big_a when shifted), so big_a ≥ big_b: the checked step cannot fail.
            big_a.checked_sub(big_b)?
        } else {
            let (sum, carry) = big_a.overflowing_add(big_b);
            if carry {
                m_ex = m_ex.checked_add(1)?;
                // The carried-out bit becomes the top one; sum ≤ 2^128 − 2, so rounding the
                // shifted-out bit in cannot overflow.
                (1u128 << 127) | (sum.checked_add(sum & 1)? >> 1)
            } else {
                sum
            }
        };
        if c == 0 {
            return Some(ZERO);
        }
        let ex = c.leading_zeros(); // < 128 since c ≠ 0
        Some(Dint::with_significand(
            c << ex,
            m_ex.checked_sub(i64::from(ex))?,
            a.sgn,
        ))
    }

    /// `self · b` with 126 bits of accuracy (C: `mul_dint`). `None` only if an exponent
    /// overflowed, which no operand of this file's evaluation makes.
    fn mul(self, b: Dint) -> Option<Dint> {
        // Each 64 × 64-bit product is below (2^64 − 1)² < 2^128, so no checked product fails.
        let mut t = u128::from(self.hi).checked_mul(u128::from(b.hi))?;
        let m1 = u128::from(self.hi).checked_mul(u128::from(b.lo))?;
        let m2 = u128::from(self.lo).checked_mul(u128::from(b.hi))?;
        // The middle sum keeps its carry, as `crate::wide` keeps a limb's.
        let (m, carry) = m1.overflowing_add(m2);
        // t plus the middle terms' high part is at most the high 128 bits of the exact product
        // of the two 128-bit significands, which is below 2^128: neither checked sum fails.
        t = t.checked_add(u128::from(carry) << 64)?;
        t = t.checked_add(u128::from(hi64(m)))?;
        // Normalise so that the top bit is set.
        let ex = i64::from(t >> 127 == 0);
        if ex != 0 {
            t <<= 1;
        }
        // C: `t += (uint64_t) m >> 63`, the rounding bit below t. t ≤ 2^128 − 2 here (the high
        // part of a product of two numbers below 2^128), so the sum cannot overflow.
        t = t.checked_add(u128::from(lo64(m) >> 63))?;
        let ex = self.ex.checked_add(b.ex)?.checked_sub(ex)?.checked_add(1)?;
        Some(Dint::with_significand(t, ex, self.sgn ^ b.sgn))
    }

    /// `b · a` for an integer `b` with `|b|` small (here `|b| ≤ 1075`) (C: `mul_dint_2`). `None`
    /// only if a step overflowed, which no such `b` makes.
    #[allow(
        clippy::many_single_char_names,
        reason = "the names of the C source's variables, so the two can be read side by side"
    )]
    fn mul_int(b: i64, a: Dint) -> Option<Dint> {
        if b == 0 {
            return Some(ZERO);
        }
        let c = b.unsigned_abs();
        let sgn = if b < 0 { u64::from(a.sgn == 0) } else { a.sgn };
        // Below 2^64 · 2^11 = 2^75: the checked products cannot fail.
        let mut t = u128::from(a.hi).checked_mul(u128::from(c))?;
        let hi = hi64(t);
        let m = if hi != 0 { hi.leading_zeros() } else { 64 };
        t <<= m;
        // m ≥ 53 here, since a.hi·|b| < 2^75 makes hi < 2^11; so m − 1 is exact.
        let l = shl128(
            u128::from(a.lo).checked_mul(u128::from(c))?,
            u64::from(m.checked_sub(1)?),
        ) >> 63;
        // C adds with a carry out of 128 bits (then `t += t & 1` and the carried bit on top), a
        // branch its only call, `mul_int(E, LOG2)`, never takes for any exponent E a double has
        // (`mul_int_never_carries_for_ln`, for every E in ±2000). So the sum is a checked step:
        // were it ever to carry, `ln` would give NaN, never a plausible wrong value.
        t = l.checked_add(t)?;
        let ex = a.ex.checked_add(64)?.checked_sub(i64::from(m))?;
        Some(Dint::with_significand(t, ex, sgn))
    }

    /// The exact value of a positive finite double (C: `dint_fromd`). `None` never: its
    /// exponent arithmetic is on values below 2^12.
    fn from_f64(x: f64) -> Option<Dint> {
        let bits = x.to_bits();
        let biased = signed((bits >> 52) & 0x7ff); // below 2^11: its value
        let m = (bits & MANTISSA) | if biased != 0 { 1u64 << 52 } else { 0 };
        let t = m.leading_zeros();
        let shift = if t > 11 { i64::from(t.abs_diff(12)) } else { 0 }; // t − 12
        Some(Dint {
            hi: shl(m, u64::from(t)),
            lo: 0,
            ex: biased.checked_sub(0x3ff)?.checked_sub(shift)?,
            sgn: bits >> 63,
        })
    }

    /// The double nearest this value, assuming it is in the normal range and not within the
    /// accurate path's error of a rounding boundary (C: `dint_tod`).
    ///
    /// The bits after the round bit are treated as non-zero: for `ln` they always are, since
    /// the hardest case has 64 identical bits after the round bit. So the result is the 53 top
    /// bits plus a quarter ulp, or plus three quarters if the round bit is set, rounded once.
    fn to_f64(self) -> f64 {
        let r = f64::from_bits((self.hi >> 11) | (0x3ff << 52) | (self.sgn << 63)); // 1 ≤ |r| < 2
        let mut rd = hf!("0x1p-54");
        if (self.hi >> 10) & 1 != 0 {
            rd += hf!("0x1p-53");
        }
        let r = if self.sgn == 0 { r + rd } else { r - rd };
        // C: the bits `((ex + 1023) & 0x7ff) << 52`, 2^ex. Here −54 ≤ ex ≤ 9 (`|ln x|` is between
        // 2^−54 and 745), so the field is in range and C's mask changes nothing; `pow2` would give
        // the canonical NaN rather than a wrapped field outside it.
        let scale = pow2(Some(self.ex));
        r * scale
    }

    /// `ln(self)` for a normalised positive `self` (C: `log_2`): `ln x = E·ln 2 + ln(1 + z) −
    /// ln(INVERSE_2[i])` with `z = x·INVERSE_2[i] − 1`, `i` the top 8 fractional bits.
    #[allow(
        clippy::many_single_char_names,
        reason = "the names of the C source's variables, so the two can be read side by side"
    )]
    fn log(self) -> Option<Dint> {
        let mut x = self;
        let mut big_e = x.ex;
        let mut i = x.hi >> 55; // 256 ≤ i < 512
        if x.hi > 0xb504_f333_f9de_6484 {
            // x > √2·2^E: use x/2
            big_e = big_e.checked_add(1)?;
            i >>= 1;
        }
        x.ex = x.ex.checked_sub(big_e)?;
        // 181 ≤ i ≤ 362, so the table index i − 181 is in range of the trimmed tables (C:
        // `i − 181`), and the checked difference cannot fail.
        let k = i.checked_sub(181)?;
        let index = usize::try_from(k).ok()?;
        let inverse = *INVERSE_2.get(index)?;
        let log_inverse = *LOG_INV_2.get(index)?;
        let z = x.mul(inverse)?.add(M_ONE)?;
        let r = Dint::mul_int(big_e, LOG2)?;
        // For i = 255 the table's inverse is 1, so z = x − 1 ∈ [−2^−8, 0): outside the range
        // [0, 2^−7.5] that `P2_COEFFS` was fitted on (see the module documentation).
        let lz = if k == NEAR_ONE_BELOW {
            log1p_taylor(z)?
        } else {
            poly2(z)?
        };
        let p = lz.add(log_inverse)?;
        r.add(p)
    }
}

/// `z·P(z)`, the degree-13 approximation of `ln(1 + z)` of the accurate path (C: `p_2`).
fn poly2(z: Dint) -> Option<Dint> {
    let [first, rest @ ..] = &P2_COEFFS;
    let mut r = *first;
    for c in rest {
        r = r.mul(z)?.add(*c)?;
    }
    r.mul(z)
}

/// The table index (`i − 181`) of the reduced arguments in [255/256, 1), where the inverse is 1.
const NEAR_ONE_BELOW: u64 = 255 - 181;

/// `ln(1 + z)` for `−2^−8 ≤ z ≤ 0` by its Taylor series to degree 16 (a change from the C
/// source; see the module documentation). The remainder is below `|z|^17/17/(1 − |z|) ≤ 2^−140`.
fn log1p_taylor(z: Dint) -> Option<Dint> {
    let [first, rest @ ..] = &LOG1P_TAYLOR;
    let mut r = *first;
    for c in rest {
        r = r.mul(z)?.add(*c)?;
    }
    r.mul(z)
}

/// `(−1)^(n+1)/n` for `n` = 16 down to 1, each rounded to nearest at 128 bits.
#[rustfmt::skip]
const LOG1P_TAYLOR: [Dint; 16] = [
    d(0x8000_0000_0000_0000, 0x0000_0000_0000_0000, -4, 1), // n = 16
    d(0x8888_8888_8888_8888, 0x8888_8888_8888_8889, -4, 0), // n = 15
    d(0x9249_2492_4924_9249, 0x2492_4924_9249_2492, -4, 1), // n = 14
    d(0x9d89_d89d_89d8_9d89, 0xd89d_89d8_9d89_d89e, -4, 0), // n = 13
    d(0xaaaa_aaaa_aaaa_aaaa, 0xaaaa_aaaa_aaaa_aaab, -4, 1), // n = 12
    d(0xba2e_8ba2_e8ba_2e8b, 0xa2e8_ba2e_8ba2_e8ba, -4, 0), // n = 11
    d(0xcccc_cccc_cccc_cccc, 0xcccc_cccc_cccc_cccd, -4, 1), // n = 10
    d(0xe38e_38e3_8e38_e38e, 0x38e3_8e38_e38e_38e4, -4, 0), // n = 9
    d(0x8000_0000_0000_0000, 0x0000_0000_0000_0000, -3, 1), // n = 8
    d(0x9249_2492_4924_9249, 0x2492_4924_9249_2492, -3, 0), // n = 7
    d(0xaaaa_aaaa_aaaa_aaaa, 0xaaaa_aaaa_aaaa_aaab, -3, 1), // n = 6
    d(0xcccc_cccc_cccc_cccc, 0xcccc_cccc_cccc_cccd, -3, 0), // n = 5
    d(0x8000_0000_0000_0000, 0x0000_0000_0000_0000, -2, 1), // n = 4
    d(0xaaaa_aaaa_aaaa_aaaa, 0xaaaa_aaaa_aaaa_aaab, -2, 0), // n = 3
    d(0x8000_0000_0000_0000, 0x0000_0000_0000_0000, -1, 1), // n = 2
    d(0x8000_0000_0000_0000, 0x0000_0000_0000_0000, 0, 0), // n = 1
];

/// The coefficients of `P`, highest degree (12) first (C: `P_2`).
#[rustfmt::skip]
const P2_COEFFS: [Dint; 13] = [
    d(0x99df_88a0_4308_13ca, 0xa1cf_fb6e_966a_70f6, -4, 0),
    d(0xaaa0_2d43_f696_c3e4, 0x4dbe_7546_67b6_bc48, -4, 1),
    d(0xba2e_7a1e_af85_6174, 0x70e5_c5a5_ebbe_0226, -4, 0),
    d(0xcccc_ccb9_ec01_7492, 0xf934_e28d_924e_76d4, -4, 1),
    d(0xe38e_38e3_807c_fa4b, 0xc976_e6cb_d22e_203f, -4, 0),
    d(0xffff_ffff_fff9_24cc, 0x05b3_08e3_9fa7_dfb5, -4, 1),
    d(0x9249_2492_4924_911d, 0x862b_c3d3_3abb_3649, -3, 0),
    d(0xaaaa_aaaa_aaaa_aaaa, 0x6637_fd4b_1974_3eec, -3, 1),
    d(0xcccc_cccc_cccc_cccc, 0xccc2_ca18_b08f_e343, -3, 0),
    d(0xffff_ffff_ffff_ffff, 0xffff_ff22_4582_3ae0, -3, 1),
    d(0xaaaa_aaaa_aaaa_aaaa, 0xaaaa_aaaa_a5c4_8b54, -2, 0),
    d(0xffff_ffff_ffff_ffff, 0xffff_ffff_ffff_ebd8, -2, 1),
    d(0x8000_0000_0000_0000, 0x0000_0000_0000_0000, 0, 0),
];

/// `r` with `r·y − 1` exact for `i·2^−9 ≤ y < (i + 1)·2^−9`, at index `i − 362`, `362 ≤ i ≤ 724`
/// (C: `_INVERSE`). `|r·y − 1| ≤ 0.00212097167968735`; the entries around `y = 1` are 1.
#[rustfmt::skip]
const INVERSE: [f64; 363] = [
    hf!("0x1.698p+0"), hf!("0x1.688p+0"), hf!("0x1.678p+0"), hf!("0x1.668p+0"),
    hf!("0x1.658p+0"), hf!("0x1.648p+0"), hf!("0x1.638p+0"), hf!("0x1.63p+0"),
    hf!("0x1.62p+0"), hf!("0x1.61p+0"), hf!("0x1.6p+0"), hf!("0x1.5fp+0"),
    hf!("0x1.5ep+0"), hf!("0x1.5dp+0"), hf!("0x1.5cp+0"), hf!("0x1.5bp+0"),
    hf!("0x1.5a8p+0"), hf!("0x1.598p+0"), hf!("0x1.588p+0"), hf!("0x1.578p+0"),
    hf!("0x1.568p+0"), hf!("0x1.56p+0"), hf!("0x1.55p+0"), hf!("0x1.54p+0"),
    hf!("0x1.53p+0"), hf!("0x1.52p+0"), hf!("0x1.518p+0"), hf!("0x1.508p+0"),
    hf!("0x1.4f8p+0"), hf!("0x1.4fp+0"), hf!("0x1.4ep+0"), hf!("0x1.4dp+0"),
    hf!("0x1.4cp+0"), hf!("0x1.4b8p+0"), hf!("0x1.4a8p+0"), hf!("0x1.4ap+0"),
    hf!("0x1.49p+0"), hf!("0x1.48p+0"), hf!("0x1.478p+0"), hf!("0x1.468p+0"),
    hf!("0x1.458p+0"), hf!("0x1.45p+0"), hf!("0x1.44p+0"), hf!("0x1.43p+0"),
    hf!("0x1.428p+0"), hf!("0x1.418p+0"), hf!("0x1.41p+0"), hf!("0x1.4p+0"),
    hf!("0x1.3f8p+0"), hf!("0x1.3e8p+0"), hf!("0x1.3ep+0"), hf!("0x1.3dp+0"),
    hf!("0x1.3cp+0"), hf!("0x1.3b8p+0"), hf!("0x1.3a8p+0"), hf!("0x1.3ap+0"),
    hf!("0x1.39p+0"), hf!("0x1.388p+0"), hf!("0x1.378p+0"), hf!("0x1.37p+0"),
    hf!("0x1.36p+0"), hf!("0x1.358p+0"), hf!("0x1.35p+0"), hf!("0x1.34p+0"),
    hf!("0x1.338p+0"), hf!("0x1.328p+0"), hf!("0x1.32p+0"), hf!("0x1.31p+0"),
    hf!("0x1.308p+0"), hf!("0x1.3p+0"), hf!("0x1.2fp+0"), hf!("0x1.2e8p+0"),
    hf!("0x1.2d8p+0"), hf!("0x1.2dp+0"), hf!("0x1.2c8p+0"), hf!("0x1.2b8p+0"),
    hf!("0x1.2bp+0"), hf!("0x1.2ap+0"), hf!("0x1.298p+0"), hf!("0x1.29p+0"),
    hf!("0x1.28p+0"), hf!("0x1.278p+0"), hf!("0x1.27p+0"), hf!("0x1.26p+0"),
    hf!("0x1.258p+0"), hf!("0x1.25p+0"), hf!("0x1.24p+0"), hf!("0x1.238p+0"),
    hf!("0x1.23p+0"), hf!("0x1.228p+0"), hf!("0x1.218p+0"), hf!("0x1.21p+0"),
    hf!("0x1.208p+0"), hf!("0x1.2p+0"), hf!("0x1.1fp+0"), hf!("0x1.1e8p+0"),
    hf!("0x1.1ep+0"), hf!("0x1.1dp+0"), hf!("0x1.1c8p+0"), hf!("0x1.1cp+0"),
    hf!("0x1.1b8p+0"), hf!("0x1.1bp+0"), hf!("0x1.1ap+0"), hf!("0x1.198p+0"),
    hf!("0x1.19p+0"), hf!("0x1.188p+0"), hf!("0x1.18p+0"), hf!("0x1.17p+0"),
    hf!("0x1.168p+0"), hf!("0x1.16p+0"), hf!("0x1.158p+0"), hf!("0x1.15p+0"),
    hf!("0x1.14p+0"), hf!("0x1.138p+0"), hf!("0x1.13p+0"), hf!("0x1.128p+0"),
    hf!("0x1.12p+0"), hf!("0x1.118p+0"), hf!("0x1.11p+0"), hf!("0x1.1p+0"),
    hf!("0x1.0f8p+0"), hf!("0x1.0fp+0"), hf!("0x1.0e8p+0"), hf!("0x1.0ep+0"),
    hf!("0x1.0d8p+0"), hf!("0x1.0dp+0"), hf!("0x1.0c8p+0"), hf!("0x1.0cp+0"),
    hf!("0x1.0bp+0"), hf!("0x1.0a8p+0"), hf!("0x1.0ap+0"), hf!("0x1.098p+0"),
    hf!("0x1.09p+0"), hf!("0x1.088p+0"), hf!("0x1.08p+0"), hf!("0x1.078p+0"),
    hf!("0x1.07p+0"), hf!("0x1.068p+0"), hf!("0x1.06p+0"), hf!("0x1.058p+0"),
    hf!("0x1.05p+0"), hf!("0x1.048p+0"), hf!("0x1.04p+0"), hf!("0x1.038p+0"),
    hf!("0x1.03p+0"), hf!("0x1.028p+0"), hf!("0x1.02p+0"), hf!("0x1.018p+0"),
    hf!("0x1.01p+0"), hf!("0x1p+0"), hf!("0x1p+0"), hf!("0x1.fe8p-1"),
    hf!("0x1.fd8p-1"), hf!("0x1.fc8p-1"), hf!("0x1.fb8p-1"), hf!("0x1.fa8p-1"),
    hf!("0x1.f98p-1"), hf!("0x1.f88p-1"), hf!("0x1.f78p-1"), hf!("0x1.f68p-1"),
    hf!("0x1.f58p-1"), hf!("0x1.f5p-1"), hf!("0x1.f4p-1"), hf!("0x1.f3p-1"),
    hf!("0x1.f2p-1"), hf!("0x1.f1p-1"), hf!("0x1.fp-1"), hf!("0x1.efp-1"),
    hf!("0x1.eep-1"), hf!("0x1.edp-1"), hf!("0x1.ec8p-1"), hf!("0x1.eb8p-1"),
    hf!("0x1.ea8p-1"), hf!("0x1.e98p-1"), hf!("0x1.e88p-1"), hf!("0x1.e78p-1"),
    hf!("0x1.e7p-1"), hf!("0x1.e6p-1"), hf!("0x1.e5p-1"), hf!("0x1.e4p-1"),
    hf!("0x1.e3p-1"), hf!("0x1.e28p-1"), hf!("0x1.e18p-1"), hf!("0x1.e08p-1"),
    hf!("0x1.df8p-1"), hf!("0x1.dfp-1"), hf!("0x1.dep-1"), hf!("0x1.ddp-1"),
    hf!("0x1.dcp-1"), hf!("0x1.db8p-1"), hf!("0x1.da8p-1"), hf!("0x1.d98p-1"),
    hf!("0x1.d9p-1"), hf!("0x1.d8p-1"), hf!("0x1.d7p-1"), hf!("0x1.d6p-1"),
    hf!("0x1.d58p-1"), hf!("0x1.d48p-1"), hf!("0x1.d38p-1"), hf!("0x1.d3p-1"),
    hf!("0x1.d2p-1"), hf!("0x1.d1p-1"), hf!("0x1.d08p-1"), hf!("0x1.cf8p-1"),
    hf!("0x1.ce8p-1"), hf!("0x1.cep-1"), hf!("0x1.cdp-1"), hf!("0x1.cc8p-1"),
    hf!("0x1.cb8p-1"), hf!("0x1.ca8p-1"), hf!("0x1.cap-1"), hf!("0x1.c9p-1"),
    hf!("0x1.c88p-1"), hf!("0x1.c78p-1"), hf!("0x1.c68p-1"), hf!("0x1.c6p-1"),
    hf!("0x1.c5p-1"), hf!("0x1.c48p-1"), hf!("0x1.c38p-1"), hf!("0x1.c3p-1"),
    hf!("0x1.c2p-1"), hf!("0x1.c18p-1"), hf!("0x1.c08p-1"), hf!("0x1.bf8p-1"),
    hf!("0x1.bfp-1"), hf!("0x1.bep-1"), hf!("0x1.bd8p-1"), hf!("0x1.bc8p-1"),
    hf!("0x1.bcp-1"), hf!("0x1.bbp-1"), hf!("0x1.ba8p-1"), hf!("0x1.b98p-1"),
    hf!("0x1.b9p-1"), hf!("0x1.b8p-1"), hf!("0x1.b78p-1"), hf!("0x1.b68p-1"),
    hf!("0x1.b6p-1"), hf!("0x1.b58p-1"), hf!("0x1.b48p-1"), hf!("0x1.b4p-1"),
    hf!("0x1.b3p-1"), hf!("0x1.b28p-1"), hf!("0x1.b18p-1"), hf!("0x1.b1p-1"),
    hf!("0x1.bp-1"), hf!("0x1.af8p-1"), hf!("0x1.afp-1"), hf!("0x1.aep-1"),
    hf!("0x1.ad8p-1"), hf!("0x1.ac8p-1"), hf!("0x1.acp-1"), hf!("0x1.ab8p-1"),
    hf!("0x1.aa8p-1"), hf!("0x1.aap-1"), hf!("0x1.a9p-1"), hf!("0x1.a88p-1"),
    hf!("0x1.a8p-1"), hf!("0x1.a7p-1"), hf!("0x1.a68p-1"), hf!("0x1.a6p-1"),
    hf!("0x1.a5p-1"), hf!("0x1.a48p-1"), hf!("0x1.a4p-1"), hf!("0x1.a3p-1"),
    hf!("0x1.a28p-1"), hf!("0x1.a2p-1"), hf!("0x1.a1p-1"), hf!("0x1.a08p-1"),
    hf!("0x1.ap-1"), hf!("0x1.9fp-1"), hf!("0x1.9e8p-1"), hf!("0x1.9ep-1"),
    hf!("0x1.9dp-1"), hf!("0x1.9c8p-1"), hf!("0x1.9cp-1"), hf!("0x1.9bp-1"),
    hf!("0x1.9a8p-1"), hf!("0x1.9ap-1"), hf!("0x1.998p-1"), hf!("0x1.988p-1"),
    hf!("0x1.98p-1"), hf!("0x1.978p-1"), hf!("0x1.968p-1"), hf!("0x1.96p-1"),
    hf!("0x1.958p-1"), hf!("0x1.95p-1"), hf!("0x1.94p-1"), hf!("0x1.938p-1"),
    hf!("0x1.93p-1"), hf!("0x1.928p-1"), hf!("0x1.92p-1"), hf!("0x1.91p-1"),
    hf!("0x1.908p-1"), hf!("0x1.9p-1"), hf!("0x1.8f8p-1"), hf!("0x1.8e8p-1"),
    hf!("0x1.8ep-1"), hf!("0x1.8d8p-1"), hf!("0x1.8dp-1"), hf!("0x1.8c8p-1"),
    hf!("0x1.8b8p-1"), hf!("0x1.8bp-1"), hf!("0x1.8a8p-1"), hf!("0x1.8ap-1"),
    hf!("0x1.898p-1"), hf!("0x1.888p-1"), hf!("0x1.88p-1"), hf!("0x1.878p-1"),
    hf!("0x1.87p-1"), hf!("0x1.868p-1"), hf!("0x1.86p-1"), hf!("0x1.85p-1"),
    hf!("0x1.848p-1"), hf!("0x1.84p-1"), hf!("0x1.838p-1"), hf!("0x1.83p-1"),
    hf!("0x1.828p-1"), hf!("0x1.82p-1"), hf!("0x1.81p-1"), hf!("0x1.808p-1"),
    hf!("0x1.8p-1"), hf!("0x1.7f8p-1"), hf!("0x1.7fp-1"), hf!("0x1.7e8p-1"),
    hf!("0x1.7ep-1"), hf!("0x1.7d8p-1"), hf!("0x1.7c8p-1"), hf!("0x1.7cp-1"),
    hf!("0x1.7b8p-1"), hf!("0x1.7bp-1"), hf!("0x1.7a8p-1"), hf!("0x1.7ap-1"),
    hf!("0x1.798p-1"), hf!("0x1.79p-1"), hf!("0x1.788p-1"), hf!("0x1.78p-1"),
    hf!("0x1.778p-1"), hf!("0x1.77p-1"), hf!("0x1.76p-1"), hf!("0x1.758p-1"),
    hf!("0x1.75p-1"), hf!("0x1.748p-1"), hf!("0x1.74p-1"), hf!("0x1.738p-1"),
    hf!("0x1.73p-1"), hf!("0x1.728p-1"), hf!("0x1.72p-1"), hf!("0x1.718p-1"),
    hf!("0x1.71p-1"), hf!("0x1.708p-1"), hf!("0x1.7p-1"), hf!("0x1.6f8p-1"),
    hf!("0x1.6fp-1"), hf!("0x1.6e8p-1"), hf!("0x1.6ep-1"), hf!("0x1.6d8p-1"),
    hf!("0x1.6dp-1"), hf!("0x1.6c8p-1"), hf!("0x1.6cp-1"), hf!("0x1.6b8p-1"),
    hf!("0x1.6bp-1"), hf!("0x1.6a8p-1"), hf!("0x1.6ap-1"),
];

/// `−ln r ≈ h + l` for each [`INVERSE`] entry, `h` a multiple of 2^−42 and `|l| < 2^−43`, with
/// error below 2^−97 (C: `_LOG_INV`).
#[rustfmt::skip]
const LOG_INV: [[f64; 2]; 363] = [
    [hf!("-0x1.615ddb4becp-2"), hf!("-0x1.3c7ca90bc04b2p-46")],
    [hf!("-0x1.5e87b20c29p-2"), hf!("-0x1.527d18f7738fap-44")],
    [hf!("-0x1.5baf846aa2p-2"), hf!("0x1.39ae8f873fa41p-44")],
    [hf!("-0x1.58d54f86ep-2"), hf!("-0x1.791f30a795215p-45")],
    [hf!("-0x1.55f9107a44p-2"), hf!("0x1.1e64778df4a62p-46")],
    [hf!("-0x1.531ac457eep-2"), hf!("-0x1.df83b7d931501p-44")],
    [hf!("-0x1.503a682cb2p-2"), hf!("0x1.a68c8f16f9b5dp-45")],
    [hf!("-0x1.4ec97326p-2"), hf!("-0x1.34d7aaf04d104p-45")],
    [hf!("-0x1.4be5f95778p-2"), hf!("0x1.d7c92cd9ad824p-44")],
    [hf!("-0x1.4900680401p-2"), hf!("0x1.8bccffe1a0f8cp-44")],
    [hf!("-0x1.4618bc21c6p-2"), hf!("0x1.3d82f484c84ccp-46")],
    [hf!("-0x1.432ef2a04fp-2"), hf!("0x1.fb129931715adp-44")],
    [hf!("-0x1.404308686ap-2"), hf!("-0x1.f8ef43049f7d3p-44")],
    [hf!("-0x1.3d54fa5c1fp-2"), hf!("-0x1.c3e1cd9a395e3p-44")],
    [hf!("-0x1.3a64c55694p-2"), hf!("-0x1.7a71cbcd735dp-44")],
    [hf!("-0x1.3772662bfep-2"), hf!("0x1.e9436ac53b023p-44")],
    [hf!("-0x1.35f865c933p-2"), hf!("0x1.b07de4ea1a54ap-44")],
    [hf!("-0x1.3302c16586p-2"), hf!("-0x1.6217dc2a3e08bp-44")],
    [hf!("-0x1.300aead063p-2"), hf!("-0x1.42f568b75fcacp-44")],
    [hf!("-0x1.2d10dec508p-2"), hf!("-0x1.60c61f7088353p-44")],
    [hf!("-0x1.2a1499f763p-2"), hf!("0x1.0dbbf51f3aadcp-44")],
    [hf!("-0x1.2895a13de8p-2"), hf!("-0x1.a8d7ad24c13fp-44")],
    [hf!("-0x1.2596010df7p-2"), hf!("-0x1.8e7bc224ea3e3p-44")],
    [hf!("-0x1.22941fbcf8p-2"), hf!("0x1.a6976f5eb0963p-44")],
    [hf!("-0x1.1f8ff9e48ap-2"), hf!("-0x1.7946c040cbe77p-45")],
    [hf!("-0x1.1c898c169ap-2"), hf!("0x1.81410e5c62affp-44")],
    [hf!("-0x1.1b05791f08p-2"), hf!("0x1.2dd466dc55e2dp-44")],
    [hf!("-0x1.17fb98e151p-2"), hf!("0x1.a8a8ba74a2684p-44")],
    [hf!("-0x1.14ef67f887p-2"), hf!("0x1.e97a65dfc9794p-44")],
    [hf!("-0x1.136870293bp-2"), hf!("0x1.d3e8499d67123p-44")],
    [hf!("-0x1.1058bf9ae5p-2"), hf!("0x1.4ab9d817d52cdp-44")],
    [hf!("-0x1.0d46b579abp-2"), hf!("-0x1.d2c81f640e1e6p-44")],
    [hf!("-0x1.0a324e2739p-2"), hf!("-0x1.c6bee7ef4030ep-47")],
    [hf!("-0x1.08a73667c5p-2"), hf!("-0x1.ebc1d40c5a329p-44")],
    [hf!("-0x1.058f3c703fp-2"), hf!("0x1.0e866bcd236adp-44")],
    [hf!("-0x1.0402594b4dp-2"), hf!("-0x1.036b89ef42d7fp-48")],
    [hf!("-0x1.00e6c45ad5p-2"), hf!("-0x1.cc68d52e01203p-50")],
    [hf!("-0x1.fb9186d5e4p-3"), hf!("0x1.d572aab993c87p-47")],
    [hf!("-0x1.f871b28956p-3"), hf!("0x1.f75fd6a526efep-44")],
    [hf!("-0x1.f22e5e72f2p-3"), hf!("0x1.f454f1417e41fp-44")],
    [hf!("-0x1.ebe61f4dd8p-3"), hf!("0x1.3d45330fdca4dp-45")],
    [hf!("-0x1.e8c0252aa6p-3"), hf!("0x1.6805b80e8e6ffp-45")],
    [hf!("-0x1.e27076e2bp-3"), hf!("0x1.a342c2af0003cp-44")],
    [hf!("-0x1.dc1bca0abep-3"), hf!("-0x1.8fac1a628ccc6p-44")],
    [hf!("-0x1.d8ef91af32p-3"), hf!("0x1.5105fc364c784p-46")],
    [hf!("-0x1.d293581b6cp-3"), hf!("0x1.83270128aaa5fp-44")],
    [hf!("-0x1.cf6354e09cp-3"), hf!("-0x1.771239a07d55bp-45")],
    [hf!("-0x1.c8ff7c79aap-3"), hf!("0x1.7794f689f8434p-45")],
    [hf!("-0x1.c5cba543aep-3"), hf!("-0x1.0929decb454fcp-45")],
    [hf!("-0x1.bf601bb0e4p-3"), hf!("-0x1.386a947c378b5p-45")],
    [hf!("-0x1.bc286742d8p-3"), hf!("-0x1.9ac53f39d121cp-44")],
    [hf!("-0x1.b5b519e8fcp-3"), hf!("0x1.4b722ec011f31p-44")],
    [hf!("-0x1.af3c94e80cp-3"), hf!("0x1.a4e633fcd9066p-52")],
    [hf!("-0x1.abfe5ae462p-3"), hf!("0x1.b68f5395f139dp-44")],
    [hf!("-0x1.a57df28244p-3"), hf!("-0x1.b99c8ca1d9abbp-44")],
    [hf!("-0x1.a23bc1fe2cp-3"), hf!("0x1.539cd91dc9f0bp-44")],
    [hf!("-0x1.9bb362e7ep-3"), hf!("0x1.1f2a8a1ce0ffcp-45")],
    [hf!("-0x1.986d322818p-3"), hf!("-0x1.93b564dd44p-48")],
    [hf!("-0x1.91dcc8c34p-3"), hf!("-0x1.7bc6abddeff46p-44")],
    [hf!("-0x1.8e928de886p-3"), hf!("-0x1.a8154b13d72d5p-44")],
    [hf!("-0x1.87fa06520cp-3"), hf!("-0x1.22120401202fcp-44")],
    [hf!("-0x1.84abb75866p-3"), hf!("0x1.d8daadf4e2bd2p-44")],
    [hf!("-0x1.815c0a1436p-3"), hf!("0x1.02a52f9201ce8p-44")],
    [hf!("-0x1.7ab890210ep-3"), hf!("0x1.bdb9072534a58p-45")],
    [hf!("-0x1.7764c128f2p-3"), hf!("-0x1.274903479e3d1p-47")],
    [hf!("-0x1.70b8f97a1ap-3"), hf!("-0x1.4ea64f6a95befp-44")],
    [hf!("-0x1.6d60fe719ep-3"), hf!("0x1.bc6e557134767p-44")],
    [hf!("-0x1.66acd4272ap-3"), hf!("-0x1.aa1bdbfc6c785p-44")],
    [hf!("-0x1.6350a28aaap-3"), hf!("-0x1.d5ec0ab8163afp-45")],
    [hf!("-0x1.5ff3070a7ap-3"), hf!("0x1.8586f183bebf2p-44")],
    [hf!("-0x1.59338d9982p-3"), hf!("-0x1.0ba68b7555d4ap-48")],
    [hf!("-0x1.55d1ad4232p-3"), hf!("-0x1.add94dda647e8p-44")],
    [hf!("-0x1.4f099f4a24p-3"), hf!("0x1.e9bf2fafeaf27p-44")],
    [hf!("-0x1.4ba36f39a6p-3"), hf!("0x1.4354bb3f219e5p-44")],
    [hf!("-0x1.483bccce6ep-3"), hf!("-0x1.eea52723f6369p-46")],
    [hf!("-0x1.41682bf728p-3"), hf!("0x1.10047081f849dp-45")],
    [hf!("-0x1.3dfc2b0eccp-3"), hf!("-0x1.8a72a62b8c13fp-45")],
    [hf!("-0x1.371fc201e8p-3"), hf!("-0x1.ee8779b2d8abcp-44")],
    [hf!("-0x1.33af57577p-3"), hf!("-0x1.c9ecca2fe72a5p-44")],
    [hf!("-0x1.303d718e48p-3"), hf!("0x1.680b5ce3ecb05p-50")],
    [hf!("-0x1.29552f82p-3"), hf!("0x1.5b967f4471dfcp-44")],
    [hf!("-0x1.25ded0abc6p-3"), hf!("-0x1.5a3854f176449p-44")],
    [hf!("-0x1.2266f190a6p-3"), hf!("0x1.4d20ab840e7f6p-45")],
    [hf!("-0x1.1b72ad52f6p-3"), hf!("-0x1.e80a41811a396p-45")],
    [hf!("-0x1.17f6458fcap-3"), hf!("-0x1.843fad093c8dcp-45")],
    [hf!("-0x1.1478584674p-3"), hf!("-0x1.563451027c75p-46")],
    [hf!("-0x1.0d77e7cd08p-3"), hf!("-0x1.cb2cd2ee2f482p-44")],
    [hf!("-0x1.09f561ee72p-3"), hf!("0x1.8f3057157d1a8p-45")],
    [hf!("-0x1.0671512ca6p-3"), hf!("0x1.a47579cdc0a3dp-45")],
    [hf!("-0x1.02ebb42bf4p-3"), hf!("0x1.5a8fa5ce00e5dp-46")],
    [hf!("-0x1.f7b79fec38p-4"), hf!("0x1.10987e897ed01p-47")],
    [hf!("-0x1.f0a30c0118p-4"), hf!("0x1.d599e83368e91p-44")],
    [hf!("-0x1.e98b54967p-4"), hf!("-0x1.4677489c50e97p-44")],
    [hf!("-0x1.e27076e2bp-4"), hf!("0x1.a342c2af0003cp-45")],
    [hf!("-0x1.d4313d66ccp-4"), hf!("0x1.9454379135713p-45")],
    [hf!("-0x1.cd0cdbf8cp-4"), hf!("-0x1.3e14db50dd743p-44")],
    [hf!("-0x1.c5e548f5bcp-4"), hf!("-0x1.d0c57585fbe06p-46")],
    [hf!("-0x1.b78c82bb1p-4"), hf!("0x1.25ef7bc3987e7p-44")],
    [hf!("-0x1.b05b49bee4p-4"), hf!("-0x1.ff22c18f84a5ep-47")],
    [hf!("-0x1.a926d3a4acp-4"), hf!("-0x1.563650bd22a9cp-44")],
    [hf!("-0x1.a1ef1d806p-4"), hf!("-0x1.cd4176df97bcbp-44")],
    [hf!("-0x1.9ab4246204p-4"), hf!("0x1.8a64826787061p-45")],
    [hf!("-0x1.8c345d6318p-4"), hf!("-0x1.b20f5acb42a66p-44")],
    [hf!("-0x1.84ef898e84p-4"), hf!("0x1.7d5cd246977c9p-44")],
    [hf!("-0x1.7da766d7bp-4"), hf!("-0x1.2cc844480c89bp-44")],
    [hf!("-0x1.765bf23a6cp-4"), hf!("0x1.ecbc035c4256ap-48")],
    [hf!("-0x1.6f0d28ae58p-4"), hf!("0x1.4b4641b664613p-44")],
    [hf!("-0x1.60658a9374p-4"), hf!("-0x1.0c3b1dee9c4f8p-44")],
    [hf!("-0x1.590cafdfp-4"), hf!("-0x1.c284f5722abaap-44")],
    [hf!("-0x1.51b073f06p-4"), hf!("-0x1.83f69278e686ap-44")],
    [hf!("-0x1.4a50d3aa1cp-4"), hf!("0x1.f7fe1308973e2p-45")],
    [hf!("-0x1.42edcbea64p-4"), hf!("-0x1.bc0eeea7c9acdp-46")],
    [hf!("-0x1.341d7961bcp-4"), hf!("-0x1.1d0929983761p-44")],
    [hf!("-0x1.2cb0283f5cp-4"), hf!("-0x1.e1ee2ca657021p-44")],
    [hf!("-0x1.253f62f0ap-4"), hf!("-0x1.416f8fb69a701p-44")],
    [hf!("-0x1.1dcb263dbp-4"), hf!("-0x1.9444f5e9e8981p-44")],
    [hf!("-0x1.16536eea38p-4"), hf!("0x1.47c5e768fa309p-46")],
    [hf!("-0x1.0ed839b554p-4"), hf!("0x1.901f46d48abb4p-44")],
    [hf!("-0x1.075983599p-4"), hf!("0x1.b8ecfe4b59987p-44")],
    [hf!("-0x1.f0a30c0118p-5"), hf!("0x1.d599e83368e91p-45")],
    [hf!("-0x1.e19070c278p-5"), hf!("0x1.fea4664629e86p-45")],
    [hf!("-0x1.d276b8adbp-5"), hf!("-0x1.6a423c78a64bp-46")],
    [hf!("-0x1.c355dd092p-5"), hf!("-0x1.f2ccc9abf8388p-45")],
    [hf!("-0x1.b42dd71198p-5"), hf!("0x1.c827ae5d6704cp-46")],
    [hf!("-0x1.a4fe9ffa4p-5"), hf!("0x1.6e584a0402925p-44")],
    [hf!("-0x1.95c830ec9p-5"), hf!("0x1.c148297c5feb8p-45")],
    [hf!("-0x1.868a83084p-5"), hf!("0x1.2623a134ac693p-46")],
    [hf!("-0x1.77458f633p-5"), hf!("0x1.181dce586af09p-44")],
    [hf!("-0x1.58a5bafc9p-5"), hf!("0x1.b2b739570ad39p-45")],
    [hf!("-0x1.494acc34d8p-5"), hf!("-0x1.11c78a56fd247p-45")],
    [hf!("-0x1.39e87b9fe8p-5"), hf!("-0x1.eafd480ad9015p-44")],
    [hf!("-0x1.2a7ec2215p-5"), hf!("0x1.78ce77a9163fep-45")],
    [hf!("-0x1.1b0d98924p-5"), hf!("0x1.3401e9ae889bbp-44")],
    [hf!("-0x1.0b94f7c198p-5"), hf!("0x1.e89896f022783p-45")],
    [hf!("-0x1.f829b0e78p-6"), hf!("-0x1.980267c7e09e4p-45")],
    [hf!("-0x1.d91a66c54p-6"), hf!("-0x1.e61f1658cfb9ap-45")],
    [hf!("-0x1.b9fc027bp-6"), hf!("0x1.b9a010ae6922ap-44")],
    [hf!("-0x1.9ace7551dp-6"), hf!("0x1.d75d97ec7c41p-45")],
    [hf!("-0x1.7b91b07d6p-6"), hf!("0x1.3b955b602ace4p-44")],
    [hf!("-0x1.5c45a51b9p-6"), hf!("0x1.63bb6216d87d8p-45")],
    [hf!("-0x1.3cea44347p-6"), hf!("0x1.6a2c432d6a40bp-44")],
    [hf!("-0x1.1d7f7eb9fp-6"), hf!("0x1.4193a83fcc7a6p-46")],
    [hf!("-0x1.fc0a8b0fcp-7"), hf!("-0x1.f1e7cf6d3a69cp-50")],
    [hf!("-0x1.bcf712c74p-7"), hf!("-0x1.c25e097bd9771p-46")],
    [hf!("-0x1.7dc475f82p-7"), hf!("0x1.eb1245b5da1f5p-44")],
    [hf!("-0x1.3e7295d26p-7"), hf!("0x1.609c1ff29a114p-45")],
    [hf!("-0x1.fe02a6b1p-8"), hf!("-0x1.9e23f0dda40e4p-46")],
    [hf!("-0x1.7ee11ebd8p-8"), hf!("-0x1.749d3c2d23a07p-47")],
    [hf!("-0x1.ff00aa2bp-9"), hf!("-0x1.0bc04a086b56ap-45")],
    [hf!("0x0p+0"), hf!("0x0p+0")],
    [hf!("0x0p+0"), hf!("0x0p+0")],
    [hf!("0x1.809048288p-9"), hf!("0x1.85c0696a70c0cp-45")],
    [hf!("0x1.40c8a7478p-8"), hf!("0x1.e3871df070002p-46")],
    [hf!("0x1.c189cbb1p-8"), hf!("-0x1.d80551258856p-44")],
    [hf!("0x1.2145e939ep-7"), hf!("0x1.e3d1238c4eap-44")],
    [hf!("0x1.61e77e8b6p-7"), hf!("-0x1.8073eeaf8eaf3p-44")],
    [hf!("0x1.a2a9c6c18p-7"), hf!("-0x1.f73bc4d6d3472p-44")],
    [hf!("0x1.e38ce3034p-7"), hf!("-0x1.9de88a3da281ap-44")],
    [hf!("0x1.12487a55p-6"), hf!("0x1.fdbe5fed4b393p-44")],
    [hf!("0x1.32db0ea13p-6"), hf!("0x1.710cb130895fcp-45")],
    [hf!("0x1.537e3f45fp-6"), hf!("0x1.ab259d2d7f253p-45")],
    [hf!("0x1.63d617869p-6"), hf!("0x1.7abf389596542p-47")],
    [hf!("0x1.8492528c9p-6"), hf!("-0x1.aa0ba325a0c34p-45")],
    [hf!("0x1.a55f548c6p-6"), hf!("-0x1.de0709f2d03c9p-45")],
    [hf!("0x1.c63d2ec15p-6"), hf!("-0x1.5439ce030a687p-44")],
    [hf!("0x1.e72bf2814p-6"), hf!("-0x1.8d75149774d47p-45")],
    [hf!("0x1.0415d89e78p-5"), hf!("-0x1.dddc7f461c516p-44")],
    [hf!("0x1.149e3e4008p-5"), hf!("-0x1.2b98a9a4168fdp-44")],
    [hf!("0x1.252f32f8dp-5"), hf!("0x1.83e9ae021b67bp-45")],
    [hf!("0x1.35c8bfaa1p-5"), hf!("0x1.8357d5ef9eb35p-44")],
    [hf!("0x1.3e18c1ca08p-5"), hf!("0x1.748ed3f6e378ep-44")],
    [hf!("0x1.4ebf4334ap-5"), hf!("-0x1.d9150f73be773p-45")],
    [hf!("0x1.5f6e73079p-5"), hf!("-0x1.0485a8012494cp-45")],
    [hf!("0x1.70265a551p-5"), hf!("-0x1.888df11fd5ce7p-45")],
    [hf!("0x1.80e7023d9p-5"), hf!("-0x1.99dc16f28bf45p-44")],
    [hf!("0x1.91b073efd8p-5"), hf!("-0x1.9d7c53f76ca96p-46")],
    [hf!("0x1.9a187b574p-5"), hf!("-0x1.0c22e4ec4d90dp-44")],
    [hf!("0x1.aaef2d0fbp-5"), hf!("0x1.0fc1a353bb42ep-45")],
    [hf!("0x1.bbcebfc69p-5"), hf!("-0x1.7bf868c317c2ap-46")],
    [hf!("0x1.ccb73cddd8p-5"), hf!("0x1.965c36e09f5fep-44")],
    [hf!("0x1.dda8adc68p-5"), hf!("-0x1.1b1ac64d9e42fp-45")],
    [hf!("0x1.e624c4a0b8p-5"), hf!("-0x1.0f25c74676689p-44")],
    [hf!("0x1.f723b518p-5"), hf!("-0x1.d6eb0dd5610d3p-44")],
    [hf!("0x1.0415d89e74p-4"), hf!("0x1.111c05cf1d753p-46")],
    [hf!("0x1.0c9e615ac4p-4"), hf!("0x1.c2da80974d976p-45")],
    [hf!("0x1.10e45b3cbp-4"), hf!("-0x1.7cf69284a3465p-44")],
    [hf!("0x1.1973bd1464p-4"), hf!("0x1.566d154f930b3p-44")],
    [hf!("0x1.2207b5c784p-4"), hf!("0x1.49d8cfc10c7bfp-44")],
    [hf!("0x1.2aa04a447p-4"), hf!("0x1.7a48ba8b1cb41p-44")],
    [hf!("0x1.2eee507b4p-4"), hf!("0x1.8081edd77c86p-47")],
    [hf!("0x1.378dd7f748p-4"), hf!("0x1.7141128f1facap-44")],
    [hf!("0x1.403207b414p-4"), hf!("0x1.6fd84aa8157cp-45")],
    [hf!("0x1.4485e03dbcp-4"), hf!("0x1.fad46e8d26ab7p-44")],
    [hf!("0x1.4d3115d208p-4"), hf!("-0x1.53a2582f4e1efp-48")],
    [hf!("0x1.55e10050ep-4"), hf!("0x1.c1d740c53c72ep-47")],
    [hf!("0x1.5e95a4d978p-4"), hf!("0x1.1cb7ce1d17171p-44")],
    [hf!("0x1.62f1be7d78p-4"), hf!("-0x1.179957ed63c4ep-45")],
    [hf!("0x1.6bad83c188p-4"), hf!("0x1.daf3cc08926aep-47")],
    [hf!("0x1.746e100228p-4"), hf!("-0x1.126d16e1e21d2p-44")],
    [hf!("0x1.78d02263d8p-4"), hf!("0x1.69b5794b69fb7p-47")],
    [hf!("0x1.8197e2f41p-4"), hf!("-0x1.c0fe460d20041p-44")],
    [hf!("0x1.8a6477a91cp-4"), hf!("0x1.c28c0af9bd6dfp-44")],
    [hf!("0x1.8ecc933aecp-4"), hf!("-0x1.22f39be67f7aap-45")],
    [hf!("0x1.97a07024ccp-4"), hf!("-0x1.8bcc1732093cep-48")],
    [hf!("0x1.a0792e9278p-4"), hf!("-0x1.a9ce6c9ad51bfp-47")],
    [hf!("0x1.a4e7640b1cp-4"), hf!("-0x1.e42b6b94407c8p-47")],
    [hf!("0x1.adc77ee5bp-4"), hf!("-0x1.573b209c31904p-44")],
    [hf!("0x1.b23965a53p-4"), hf!("-0x1.ff64eea137079p-49")],
    [hf!("0x1.bb20e936d8p-4"), hf!("-0x1.68ba835459b8ep-44")],
    [hf!("0x1.c40d6425a4p-4"), hf!("0x1.cb1121d1930ddp-44")],
    [hf!("0x1.c885801bc4p-4"), hf!("0x1.646d1c65aacd3p-45")],
    [hf!("0x1.d179788218p-4"), hf!("0x1.36433b5efbeedp-44")],
    [hf!("0x1.d5f556592p-4"), hf!("0x1.0e239cc185469p-44")],
    [hf!("0x1.def0d8d468p-4"), hf!("-0x1.24750412e9a74p-44")],
    [hf!("0x1.e7f1691a34p-4"), hf!("-0x1.2c1c59bc77bfap-44")],
    [hf!("0x1.ec739830ap-4"), hf!("0x1.11fcba80cdd1p-44")],
    [hf!("0x1.f57bc7d9p-4"), hf!("0x1.76a6c9ea8b04ep-46")],
    [hf!("0x1.fa01c9db58p-4"), hf!("-0x1.8f351fa48a73p-47")],
    [hf!("0x1.0188d2ecf6p-3"), hf!("0x1.3f9651cff9dfep-47")],
    [hf!("0x1.03cdc0a51ep-3"), hf!("0x1.81a9cf169fc5cp-44")],
    [hf!("0x1.08598b59e4p-3"), hf!("-0x1.7e5dd7009902cp-45")],
    [hf!("0x1.0aa0691268p-3"), hf!("-0x1.45519d7032129p-44")],
    [hf!("0x1.0f301717dp-3"), hf!("-0x1.e09b441ae86c5p-44")],
    [hf!("0x1.13c2605c3ap-3"), hf!("-0x1.cf5fdd94f6509p-45")],
    [hf!("0x1.160c8024b2p-3"), hf!("0x1.ec2d2a9009e3dp-45")],
    [hf!("0x1.1aa2b7e24p-3"), hf!("-0x1.1ac38dde3b366p-44")],
    [hf!("0x1.1ceed09854p-3"), hf!("-0x1.15c1c39192af9p-44")],
    [hf!("0x1.2188fd9808p-3"), hf!("-0x1.b3a1e7f50c701p-44")],
    [hf!("0x1.23d712a49cp-3"), hf!("0x1.00d238fd3df5cp-46")],
    [hf!("0x1.28753bc11ap-3"), hf!("0x1.7494e359302e6p-44")],
    [hf!("0x1.2ac55095f6p-3"), hf!("-0x1.d3466d0c6c8a8p-46")],
    [hf!("0x1.2f677cbbcp-3"), hf!("0x1.52b302160f40dp-44")],
    [hf!("0x1.31b994d3a4p-3"), hf!("0x1.f098ee3a5081p-44")],
    [hf!("0x1.365fcb015ap-3"), hf!("-0x1.fd3a0afb9691bp-44")],
    [hf!("0x1.38b3e9e028p-3"), hf!("-0x1.70ef0545c17f9p-44")],
    [hf!("0x1.3d5e3126bcp-3"), hf!("0x1.3fb2f85096c4bp-46")],
    [hf!("0x1.3fb45a5992p-3"), hf!("0x1.19713c0cae559p-44")],
    [hf!("0x1.420b32741p-3"), hf!("-0x1.16282c85a0884p-46")],
    [hf!("0x1.46baf0f9f6p-3"), hf!("-0x1.249cd0790841ap-46")],
    [hf!("0x1.4913d8333cp-3"), hf!("-0x1.53e43558124c4p-44")],
    [hf!("0x1.4dc7b897bcp-3"), hf!("0x1.c79b60ae1ff0fp-47")],
    [hf!("0x1.5022b292f6p-3"), hf!("0x1.48a05ff36a25bp-44")],
    [hf!("0x1.54dabc261p-3"), hf!("0x1.746fee5c8d0d8p-45")],
    [hf!("0x1.5737cc9018p-3"), hf!("0x1.9baa7a6b887f6p-44")],
    [hf!("0x1.5bf406b544p-3"), hf!("-0x1.27023eb68981cp-46")],
    [hf!("0x1.5e533144c2p-3"), hf!("-0x1.1ce0bf3b290eap-44")],
    [hf!("0x1.60b3100b0ap-3"), hf!("-0x1.71456c988f814p-44")],
    [hf!("0x1.6574ebe8c2p-3"), hf!("-0x1.98c1d34f0f462p-44")],
    [hf!("0x1.67d6e9d786p-3"), hf!("-0x1.11e8830a706d3p-44")],
    [hf!("0x1.6c9d07d204p-3"), hf!("-0x1.c73fafd9b2dcap-50")],
    [hf!("0x1.6f0128b756p-3"), hf!("0x1.577390d31ef0fp-44")],
    [hf!("0x1.716600c914p-3"), hf!("0x1.51b157cec3838p-49")],
    [hf!("0x1.7631d82936p-3"), hf!("-0x1.5e77dc7c5f3e1p-45")],
    [hf!("0x1.7898d85444p-3"), hf!("0x1.8e67be3dbaf3fp-44")],
    [hf!("0x1.7d6903caf6p-3"), hf!("-0x1.4c06b17c301d7p-45")],
    [hf!("0x1.7fd22ff59ap-3"), hf!("-0x1.58bebf457b7d2p-46")],
    [hf!("0x1.823c16551ap-3"), hf!("0x1.e0ddb9a631e83p-46")],
    [hf!("0x1.871213750ep-3"), hf!("0x1.328eb42f9af75p-44")],
    [hf!("0x1.897e2b17b2p-3"), hf!("-0x1.96b37380cbe9ep-45")],
    [hf!("0x1.8beafeb39p-3"), hf!("-0x1.73d54aae92cd1p-47")],
    [hf!("0x1.90c6db9fccp-3"), hf!("-0x1.935f57718d7cap-46")],
    [hf!("0x1.9335e5d594p-3"), hf!("0x1.3115c3abd47dap-44")],
    [hf!("0x1.95a5adcf7p-3"), hf!("0x1.7f22858a0ff6fp-47")],
    [hf!("0x1.9a8778debap-3"), hf!("0x1.470fa3efec39p-44")],
    [hf!("0x1.9cf97cdcep-3"), hf!("0x1.d862f10c414e3p-44")],
    [hf!("0x1.9f6c40708ap-3"), hf!("-0x1.337d94bcd3f43p-44")],
    [hf!("0x1.a454082e6ap-3"), hf!("0x1.60a77c81f7171p-44")],
    [hf!("0x1.a6c90d44b8p-3"), hf!("-0x1.f63b7f037b0c6p-44")],
    [hf!("0x1.a93ed3c8aep-3"), hf!("-0x1.8724350562169p-45")],
    [hf!("0x1.ae2ca6f672p-3"), hf!("0x1.7a8d5ae54f55p-44")],
    [hf!("0x1.b0a4b48fc2p-3"), hf!("-0x1.2e72d5c3998edp-45")],
    [hf!("0x1.b31d8575bcp-3"), hf!("0x1.c794e562a63cbp-44")],
    [hf!("0x1.b811730b82p-3"), hf!("0x1.e90683b9cd768p-46")],
    [hf!("0x1.ba8c90ae4ap-3"), hf!("0x1.a32e7f44432dap-44")],
    [hf!("0x1.bd087383bep-3"), hf!("-0x1.d4bc4595412b6p-45")],
    [hf!("0x1.c2028ab18p-3"), hf!("-0x1.92e0ee55c7ac6p-45")],
    [hf!("0x1.c480c0005cp-3"), hf!("0x1.9a294d5e44e76p-44")],
    [hf!("0x1.c6ffbc6fp-3"), hf!("0x1.ee138d3a69d43p-44")],
    [hf!("0x1.c97f8079d4p-3"), hf!("0x1.3b161a8c6e6c5p-45")],
    [hf!("0x1.ce816157f2p-3"), hf!("-0x1.9e0aba2099515p-45")],
    [hf!("0x1.d1037f2656p-3"), hf!("-0x1.84a7e75b6f6e4p-47")],
    [hf!("0x1.d38666872p-3"), hf!("-0x1.73650b38932bcp-44")],
    [hf!("0x1.d88e93fb3p-3"), hf!("-0x1.75f280234bf51p-44")],
    [hf!("0x1.db13db0d48p-3"), hf!("0x1.2806a847527e6p-44")],
    [hf!("0x1.dd99edaf6ep-3"), hf!("-0x1.02ec669c756ebp-44")],
    [hf!("0x1.e020cc6236p-3"), hf!("-0x1.52b00adb91424p-45")],
    [hf!("0x1.e530effe72p-3"), hf!("-0x1.fdbdbb13f7c18p-44")],
    [hf!("0x1.e7ba35eb78p-3"), hf!("-0x1.d5eee23793649p-47")],
    [hf!("0x1.ea4449f04ap-3"), hf!("0x1.5e91663732a36p-44")],
    [hf!("0x1.eccf2c8feap-3"), hf!("-0x1.bec63a3e7564p-44")],
    [hf!("0x1.ef5ade4ddp-3"), hf!("-0x1.a211565bb8e11p-51")],
    [hf!("0x1.f474b134ep-3"), hf!("-0x1.bae49f1df7b5ep-44")],
    [hf!("0x1.f702d36778p-3"), hf!("-0x1.0819516673e23p-46")],
    [hf!("0x1.f991c6cb3cp-3"), hf!("-0x1.90d04cd7cc834p-44")],
    [hf!("0x1.fc218be62p-3"), hf!("0x1.4bba46f1cf6ap-44")],
    [hf!("0x1.00a1c6addap-2"), hf!("0x1.1cd8d688b9e18p-44")],
    [hf!("0x1.01eae5626cp-2"), hf!("0x1.a43dcfade85aep-44")],
    [hf!("0x1.03346e0106p-2"), hf!("0x1.89ff8a966395cp-48")],
    [hf!("0x1.047e60cde8p-2"), hf!("0x1.dbdf10d397f3cp-45")],
    [hf!("0x1.05c8be0d96p-2"), hf!("0x1.ad0f1c77ccb58p-45")],
    [hf!("0x1.085eb8f8aep-2"), hf!("0x1.e5d513f45fe7bp-44")],
    [hf!("0x1.09aa572e6cp-2"), hf!("0x1.b50a1e1734342p-44")],
    [hf!("0x1.0af660eb9ep-2"), hf!("0x1.3c7c3f528d80ap-45")],
    [hf!("0x1.0c42d67616p-2"), hf!("0x1.7188b163ceae9p-45")],
    [hf!("0x1.0d8fb813ebp-2"), hf!("0x1.ee8c88753fa35p-46")],
    [hf!("0x1.102ac0a35dp-2"), hf!("-0x1.f1fbddfdfd686p-45")],
    [hf!("0x1.1178e8227ep-2"), hf!("0x1.1ef78ce2d07f2p-44")],
    [hf!("0x1.12c77cd007p-2"), hf!("0x1.3b2948a11f797p-46")],
    [hf!("0x1.14167ef367p-2"), hf!("0x1.e0c07824daaf5p-44")],
    [hf!("0x1.1565eed456p-2"), hf!("-0x1.e75adfb6aba25p-49")],
    [hf!("0x1.16b5ccbadp-2"), hf!("-0x1.23299042d74bfp-44")],
    [hf!("0x1.1956d3b9bcp-2"), hf!("0x1.7d2f73ad1aa14p-45")],
    [hf!("0x1.1aa7fd638dp-2"), hf!("0x1.9f60a9616f7ap-45")],
    [hf!("0x1.1bf99635a7p-2"), hf!("-0x1.1ac89575c2125p-44")],
    [hf!("0x1.1d4b9e796cp-2"), hf!("0x1.22a667c42e56dp-45")],
    [hf!("0x1.1e9e16788ap-2"), hf!("-0x1.82eaed3c8b65ep-44")],
    [hf!("0x1.1ff0fe7cf4p-2"), hf!("0x1.e9d5b513ff0c1p-44")],
    [hf!("0x1.214456d0ecp-2"), hf!("-0x1.caf0428b728a3p-44")],
    [hf!("0x1.23ec5991ecp-2"), hf!("-0x1.6dbe448a2e522p-44")],
    [hf!("0x1.25410494e5p-2"), hf!("0x1.b1d7ac0ef77f2p-44")],
    [hf!("0x1.269621134ep-2"), hf!("-0x1.1b61f10522625p-44")],
    [hf!("0x1.27ebaf58d9p-2"), hf!("-0x1.b198800b4bda7p-45")],
    [hf!("0x1.2941afb187p-2"), hf!("-0x1.210c2b730e28bp-44")],
    [hf!("0x1.2a982269a4p-2"), hf!("-0x1.2058e557285cfp-45")],
    [hf!("0x1.2bef07cdc9p-2"), hf!("0x1.a9cfa4a5004f4p-45")],
    [hf!("0x1.2d46602addp-2"), hf!("-0x1.88d0ddcd54196p-45")],
    [hf!("0x1.2ff66b04ebp-2"), hf!("-0x1.8aed2541e6e2ep-44")],
    [hf!("0x1.314f1e1d36p-2"), hf!("-0x1.8e27ad3213cb8p-45")],
    [hf!("0x1.32a8456512p-2"), hf!("0x1.4f928139af5d6p-47")],
    [hf!("0x1.3401e12aedp-2"), hf!("-0x1.17c73556e291dp-44")],
    [hf!("0x1.355bf1bd83p-2"), hf!("-0x1.ba99b8964f0e8p-45")],
    [hf!("0x1.36b6776be1p-2"), hf!("0x1.16ecdb0f177c8p-46")],
    [hf!("0x1.3811728565p-2"), hf!("-0x1.a71e493a0702bp-45")],
    [hf!("0x1.396ce359bcp-2"), hf!("-0x1.5839c5663663dp-47")],
    [hf!("0x1.3ac8ca38e6p-2"), hf!("-0x1.d0befbc02be4ap-45")],
    [hf!("0x1.3c25277333p-2"), hf!("0x1.83b54b606bd5cp-46")],
    [hf!("0x1.3d81fb5947p-2"), hf!("-0x1.22c7c2a9d37a4p-45")],
    [hf!("0x1.3edf463c17p-2"), hf!("-0x1.f067c297f2c3fp-44")],
    [hf!("0x1.419b423d5fp-2"), hf!("-0x1.ce379226de3ecp-44")],
    [hf!("0x1.42f9f3ff62p-2"), hf!("0x1.906440f7d3354p-44")],
    [hf!("0x1.44591e053ap-2"), hf!("-0x1.6e95892923d88p-47")],
    [hf!("0x1.45b8c0a17ep-2"), hf!("-0x1.d9120e7d0a853p-47")],
    [hf!("0x1.4718dc271cp-2"), hf!("0x1.06c18fb4c14c5p-44")],
    [hf!("0x1.487970e958p-2"), hf!("0x1.dc1b8465cf25fp-44")],
    [hf!("0x1.49da7f3bccp-2"), hf!("0x1.07b334daf4b9ap-44")],
    [hf!("0x1.4b3c077268p-2"), hf!("-0x1.65b4681052b9fp-46")],
    [hf!("0x1.4c9e09e173p-2"), hf!("-0x1.e20891b0ad8a4p-45")],
    [hf!("0x1.4e0086dd8cp-2"), hf!("-0x1.4d692a1e44788p-44")],
    [hf!("0x1.4f637ebbaap-2"), hf!("-0x1.fc158cb3124b9p-44")],
    [hf!("0x1.50c6f1d11cp-2"), hf!("-0x1.a0e6b7e827c2cp-44")],
    [hf!("0x1.522ae0738ap-2"), hf!("0x1.ebe708164c759p-45")],
    [hf!("0x1.538f4af8f7p-2"), hf!("0x1.7ec02e45547cep-45")],
    [hf!("0x1.54f431b7bep-2"), hf!("0x1.a8954c0910952p-46")],
    [hf!("0x1.5659950695p-2"), hf!("0x1.4c5fd2badc774p-46")],
    [hf!("0x1.57bf753c8dp-2"), hf!("0x1.fadedee5d40efp-46")],
    [hf!("0x1.5925d2b113p-2"), hf!("-0x1.69bf5a7a56f34p-44")],
    [hf!("0x1.5a8cadbbeep-2"), hf!("-0x1.7c79b0af7ecf8p-48")],
    [hf!("0x1.5bf406b544p-2"), hf!("-0x1.27023eb68981cp-45")],
    [hf!("0x1.5d5bddf596p-2"), hf!("-0x1.a0b2a08a465dcp-47")],
    [hf!("0x1.5ec433d5c3p-2"), hf!("0x1.6b71a1229d17fp-44")],
    [hf!("0x1.602d08af09p-2"), hf!("0x1.ebe9176df3f65p-46")],
    [hf!("0x1.61965cdb03p-2"), hf!("-0x1.f08ad603c488ep-45")],
    [hf!("0x1.630030b3abp-2"), hf!("-0x1.db623e731aep-45")],
];

/// `1/(i/256)` for `181 ≤ i ≤ 362`, at index `i − 181` (C: `_INVERSE_2` entries 53 to 234; the
/// comment gives the C index).
#[rustfmt::skip]
const INVERSE_2: [Dint; 182] = [
    d(0xb509_e68a_9b94_8220, 0x0000_0000_0000_0000, 0, 0),  // 53
    d(0xb40b_40b4_0b40_b40c, 0x0000_0000_0000_0000, 0, 0),  // 54
    d(0xb30f_6352_8917_c80c, 0x0000_0000_0000_0000, 0, 0),  // 55
    d(0xb216_42c8_590b_2165, 0x0000_0000_0000_0000, 0, 0),  // 56
    d(0xb11f_d3b8_0b11_fd3c, 0x0000_0000_0000_0000, 0, 0),  // 57
    d(0xb02c_0b02_c0b0_2c0c, 0x0000_0000_0000_0000, 0, 0),  // 58
    d(0xaf3a_ddc6_80af_3ade, 0x0000_0000_0000_0000, 0, 0),  // 59
    d(0xae4c_415c_9882_b932, 0x0000_0000_0000_0000, 0, 0),  // 60
    d(0xad60_2b58_0ad6_02b6, 0x0000_0000_0000_0000, 0, 0),  // 61
    d(0xac76_9184_0ac7_6919, 0x0000_0000_0000_0000, 0, 0),  // 62
    d(0xab8f_69e2_8359_cd12, 0x0000_0000_0000_0000, 0, 0),  // 63
    d(0xaaaa_aaaa_aaaa_aaab, 0x0000_0000_0000_0000, 0, 0),  // 64
    d(0xa9c8_4a47_a07f_5638, 0x0000_0000_0000_0000, 0, 0),  // 65
    d(0xa8e8_3f57_17c0_a8e9, 0x0000_0000_0000_0000, 0, 0),  // 66
    d(0xa80a_80a8_0a80_a80b, 0x0000_0000_0000_0000, 0, 0),  // 67
    d(0xa72f_0539_7829_cbc2, 0x0000_0000_0000_0000, 0, 0),  // 68
    d(0xa655_c439_2d7b_73a8, 0x0000_0000_0000_0000, 0, 0),  // 69
    d(0xa57e_b502_95fa_d40b, 0x0000_0000_0000_0000, 0, 0),  // 70
    d(0xa4a9_cf1d_9683_3752, 0x0000_0000_0000_0000, 0, 0),  // 71
    d(0xa3d7_0a3d_70a3_d70b, 0x0000_0000_0000_0000, 0, 0),  // 72
    d(0xa306_5e3f_ae7c_d0e1, 0x0000_0000_0000_0000, 0, 0),  // 73
    d(0xa237_c32b_16cf_d773, 0x0000_0000_0000_0000, 0, 0),  // 74
    d(0xa16b_312e_a8fc_377d, 0x0000_0000_0000_0000, 0, 0),  // 75
    d(0xa0a0_a0a0_a0a0_a0a1, 0x0000_0000_0000_0000, 0, 0),  // 76
    d(0x9fd8_09fd_809f_d80a, 0x0000_0000_0000_0000, 0, 0),  // 77
    d(0x9f11_65e7_2548_13e3, 0x0000_0000_0000_0000, 0, 0),  // 78
    d(0x9e4c_ad23_dd5f_3a21, 0x0000_0000_0000_0000, 0, 0),  // 79
    d(0x9d89_d89d_89d8_9d8a, 0x0000_0000_0000_0000, 0, 0),  // 80
    d(0x9cc8_e160_c3fb_19b9, 0x0000_0000_0000_0000, 0, 0),  // 81
    d(0x9c09_c09c_09c0_9c0a, 0x0000_0000_0000_0000, 0, 0),  // 82
    d(0x9b4c_6f9e_f03a_3caa, 0x0000_0000_0000_0000, 0, 0),  // 83
    d(0x9a90_e7d9_5bc6_09aa, 0x0000_0000_0000_0000, 0, 0),  // 84
    d(0x99d7_22da_bde5_8f07, 0x0000_0000_0000_0000, 0, 0),  // 85
    d(0x991f_1a51_5885_fb38, 0x0000_0000_0000_0000, 0, 0),  // 86
    d(0x9868_c809_868c_8099, 0x0000_0000_0000_0000, 0, 0),  // 87
    d(0x97b4_25ed_097b_425f, 0x0000_0000_0000_0000, 0, 0),  // 88
    d(0x9701_2e02_5c04_b80a, 0x0000_0000_0000_0000, 0, 0),  // 89
    d(0x964f_da6c_0964_fda7, 0x0000_0000_0000_0000, 0, 0),  // 90
    d(0x95a0_2568_095a_0257, 0x0000_0000_0000_0000, 0, 0),  // 91
    d(0x94f2_094f_2094_f20a, 0x0000_0000_0000_0000, 0, 0),  // 92
    d(0x9445_8094_4580_9446, 0x0000_0000_0000_0000, 0, 0),  // 93
    d(0x939a_85c4_0939_a85d, 0x0000_0000_0000_0000, 0, 0),  // 94
    d(0x92f1_1384_0497_889d, 0x0000_0000_0000_0000, 0, 0),  // 95
    d(0x9249_2492_4924_924a, 0x0000_0000_0000_0000, 0, 0),  // 96
    d(0x91a2_b3c4_d5e6_f80a, 0x0000_0000_0000_0000, 0, 0),  // 97
    d(0x90fd_bc09_0fdb_c091, 0x0000_0000_0000_0000, 0, 0),  // 98
    d(0x905a_3863_3e06_c43b, 0x0000_0000_0000_0000, 0, 0),  // 99
    d(0x8fb8_23ee_08fb_823f, 0x0000_0000_0000_0000, 0, 0),  // 100
    d(0x8f17_79d9_fdc3_a219, 0x0000_0000_0000_0000, 0, 0),  // 101
    d(0x8e78_356d_1408_e784, 0x0000_0000_0000_0000, 0, 0),  // 102
    d(0x8dda_5202_3769_4809, 0x0000_0000_0000_0000, 0, 0),  // 103
    d(0x8d3d_cb08_d3dc_b08e, 0x0000_0000_0000_0000, 0, 0),  // 104
    d(0x8ca2_9c04_6514_e024, 0x0000_0000_0000_0000, 0, 0),  // 105
    d(0x8c08_c08c_08c0_8c09, 0x0000_0000_0000_0000, 0, 0),  // 106
    d(0x8b70_344a_139b_c75b, 0x0000_0000_0000_0000, 0, 0),  // 107
    d(0x8ad8_f2fb_a938_6823, 0x0000_0000_0000_0000, 0, 0),  // 108
    d(0x8a42_f870_5669_db47, 0x0000_0000_0000_0000, 0, 0),  // 109
    d(0x89ae_4089_ae40_89af, 0x0000_0000_0000_0000, 0, 0),  // 110
    d(0x891a_c73a_e981_9b51, 0x0000_0000_0000_0000, 0, 0),  // 111
    d(0x8888_8888_8888_8889, 0x0000_0000_0000_0000, 0, 0),  // 112
    d(0x87f7_8087_f780_87f8, 0x0000_0000_0000_0000, 0, 0),  // 113
    d(0x8767_ab5f_34e4_7ef2, 0x0000_0000_0000_0000, 0, 0),  // 114
    d(0x86d9_0544_7a34_acc7, 0x0000_0000_0000_0000, 0, 0),  // 115
    d(0x864b_8a7d_e6d1_d609, 0x0000_0000_0000_0000, 0, 0),  // 116
    d(0x85bf_3761_2cee_3c9b, 0x0000_0000_0000_0000, 0, 0),  // 117
    d(0x8534_0853_4085_3409, 0x0000_0000_0000_0000, 0, 0),  // 118
    d(0x84a9_f9c8_084a_9f9d, 0x0000_0000_0000_0000, 0, 0),  // 119
    d(0x8421_0842_1084_2109, 0x0000_0000_0000_0000, 0, 0),  // 120
    d(0x8399_3052_3fbe_3368, 0x0000_0000_0000_0000, 0, 0),  // 121
    d(0x8312_6e97_8d4f_df3c, 0x0000_0000_0000_0000, 0, 0),  // 122
    d(0x828c_bfbe_b9a0_20a4, 0x0000_0000_0000_0000, 0, 0),  // 123
    d(0x8208_2082_0820_8209, 0x0000_0000_0000_0000, 0, 0),  // 124
    d(0x8184_8da8_faf0_d278, 0x0000_0000_0000_0000, 0, 0),  // 125
    d(0x8102_0408_1020_4082, 0x0000_0000_0000_0000, 0, 0),  // 126
    d(0x8000_0000_0000_0000, 0x0000_0000_0000_0000, 0, 0),  // 127
    d(0x8000_0000_0000_0000, 0x0000_0000_0000_0000, 0, 0),  // 128
    d(0xff00_ff00_ff00_ff02, 0x0000_0000_0000_0000, -1, 0),  // 129
    d(0xfe03_f80f_e03f_80ff, 0x0000_0000_0000_0000, -1, 0),  // 130
    d(0xfd08_e550_0fd0_8e56, 0x0000_0000_0000_0000, -1, 0),  // 131
    d(0xfc0f_c0fc_0fc0_fc11, 0x0000_0000_0000_0000, -1, 0),  // 132
    d(0xfb18_8565_06dd_aba7, 0x0000_0000_0000_0000, -1, 0),  // 133
    d(0xfa23_2cf2_5213_8ac1, 0x0000_0000_0000_0000, -1, 0),  // 134
    d(0xf92f_b221_1855_a866, 0x0000_0000_0000_0000, -1, 0),  // 135
    d(0xf83e_0f83_e0f8_3e11, 0x0000_0000_0000_0000, -1, 0),  // 136
    d(0xf74e_3fc2_2c70_0f76, 0x0000_0000_0000_0000, -1, 0),  // 137
    d(0xf660_3d98_0f66_03db, 0x0000_0000_0000_0000, -1, 0),  // 138
    d(0xf574_03d5_d00f_5741, 0x0000_0000_0000_0000, -1, 0),  // 139
    d(0xf489_8d5f_85bb_3951, 0x0000_0000_0000_0000, -1, 0),  // 140
    d(0xf3a0_d52c_ba87_2337, 0x0000_0000_0000_0000, -1, 0),  // 141
    d(0xf2b9_d648_0f2b_9d66, 0x0000_0000_0000_0000, -1, 0),  // 142
    d(0xf1d4_8bce_e0d3_99fb, 0x0000_0000_0000_0000, -1, 0),  // 143
    d(0xf0f0_f0f0_f0f0_f0f2, 0x0000_0000_0000_0000, -1, 0),  // 144
    d(0xf00f_00f0_0f00_f010, 0x0000_0000_0000_0000, -1, 0),  // 145
    d(0xef2e_b71f_c434_5239, 0x0000_0000_0000_0000, -1, 0),  // 146
    d(0xee50_0ee5_00ee_5010, 0x0000_0000_0000_0000, -1, 0),  // 147
    d(0xed73_03b5_cc0e_d731, 0x0000_0000_0000_0000, -1, 0),  // 148
    d(0xec97_9118_f3fc_4da3, 0x0000_0000_0000_0000, -1, 0),  // 149
    d(0xebbd_b2a5_c161_9c8d, 0x0000_0000_0000_0000, -1, 0),  // 150
    d(0xeae5_6403_ab95_9010, 0x0000_0000_0000_0000, -1, 0),  // 151
    d(0xea0e_a0ea_0ea0_ea10, 0x0000_0000_0000_0000, -1, 0),  // 152
    d(0xe939_651f_e2d8_d35d, 0x0000_0000_0000_0000, -1, 0),  // 153
    d(0xe865_ac7b_7603_a198, 0x0000_0000_0000_0000, -1, 0),  // 154
    d(0xe793_72e2_25fe_30da, 0x0000_0000_0000_0000, -1, 0),  // 155
    d(0xe6c2_b448_1cd8_568a, 0x0000_0000_0000_0000, -1, 0),  // 156
    d(0xe5f3_6cb0_0e5f_36cc, 0x0000_0000_0000_0000, -1, 0),  // 157
    d(0xe525_982a_f70c_880f, 0x0000_0000_0000_0000, -1, 0),  // 158
    d(0xe459_32d7_dc52_100f, 0x0000_0000_0000_0000, -1, 0),  // 159
    d(0xe38e_38e3_8e38_e38f, 0x0000_0000_0000_0000, -1, 0),  // 160
    d(0xe2c4_a688_6a4c_2e11, 0x0000_0000_0000_0000, -1, 0),  // 161
    d(0xe1fc_780e_1fc7_80e3, 0x0000_0000_0000_0000, -1, 0),  // 162
    d(0xe135_a9c9_7500_e137, 0x0000_0000_0000_0000, -1, 0),  // 163
    d(0xe070_381c_0e07_0383, 0x0000_0000_0000_0000, -1, 0),  // 164
    d(0xdfac_1f74_346c_5760, 0x0000_0000_0000_0000, -1, 0),  // 165
    d(0xdee9_5c4c_a037_ba58, 0x0000_0000_0000_0000, -1, 0),  // 166
    d(0xde27_eb2c_41f3_d9d2, 0x0000_0000_0000_0000, -1, 0),  // 167
    d(0xdd67_c8a6_0dd6_7c8b, 0x0000_0000_0000_0000, -1, 0),  // 168
    d(0xdca8_f158_c7f9_1ab9, 0x0000_0000_0000_0000, -1, 0),  // 169
    d(0xdbeb_61ee_d19c_5959, 0x0000_0000_0000_0000, -1, 0),  // 170
    d(0xdb2f_171d_f770_291a, 0x0000_0000_0000_0000, -1, 0),  // 171
    d(0xda74_0da7_40da_740f, 0x0000_0000_0000_0000, -1, 0),  // 172
    d(0xd9ba_4256_c036_6e92, 0x0000_0000_0000_0000, -1, 0),  // 173
    d(0xd901_b203_6406_c80f, 0x0000_0000_0000_0000, -1, 0),  // 174
    d(0xd84a_598e_c915_1f44, 0x0000_0000_0000_0000, -1, 0),  // 175
    d(0xd794_35e5_0d79_435f, 0x0000_0000_0000_0000, -1, 0),  // 176
    d(0xd6df_43fc_a482_f00e, 0x0000_0000_0000_0000, -1, 0),  // 177
    d(0xd62b_80d6_2b80_d62d, 0x0000_0000_0000_0000, -1, 0),  // 178
    d(0xd578_e97c_3f5f_e552, 0x0000_0000_0000_0000, -1, 0),  // 179
    d(0xd4c7_7b03_531d_ec0e, 0x0000_0000_0000_0000, -1, 0),  // 180
    d(0xd417_3289_870a_c52f, 0x0000_0000_0000_0000, -1, 0),  // 181
    d(0xd368_0d36_80d3_680e, 0x0000_0000_0000_0000, -1, 0),  // 182
    d(0xd2ba_083b_4452_50ac, 0x0000_0000_0000_0000, -1, 0),  // 183
    d(0xd20d_20d2_0d20_d20e, 0x0000_0000_0000_0000, -1, 0),  // 184
    d(0xd161_543e_28e5_0275, 0x0000_0000_0000_0000, -1, 0),  // 185
    d(0xd0b6_9fcb_d258_0d0c, 0x0000_0000_0000_0000, -1, 0),  // 186
    d(0xd00d_00d0_0d00_d00e, 0x0000_0000_0000_0000, -1, 0),  // 187
    d(0xcf64_74a8_819e_c8ea, 0x0000_0000_0000_0000, -1, 0),  // 188
    d(0xcebc_f8bb_5b41_69cc, 0x0000_0000_0000_0000, -1, 0),  // 189
    d(0xce16_8a77_2508_0ce2, 0x0000_0000_0000_0000, -1, 0),  // 190
    d(0xcd71_2752_a886_d243, 0x0000_0000_0000_0000, -1, 0),  // 191
    d(0xcccc_cccc_cccc_ccce, 0x0000_0000_0000_0000, -1, 0),  // 192
    d(0xcc29_786c_7607_f9a0, 0x0000_0000_0000_0000, -1, 0),  // 193
    d(0xcb87_27c0_65c3_93e1, 0x0000_0000_0000_0000, -1, 0),  // 194
    d(0xcae5_d85f_1bbd_6c96, 0x0000_0000_0000_0000, -1, 0),  // 195
    d(0xca45_87e6_b74f_032a, 0x0000_0000_0000_0000, -1, 0),  // 196
    d(0xc9a6_33fc_d967_300e, 0x0000_0000_0000_0000, -1, 0),  // 197
    d(0xc907_da4e_8711_46ae, 0x0000_0000_0000_0000, -1, 0),  // 198
    d(0xc86a_7890_0c86_a78a, 0x0000_0000_0000_0000, -1, 0),  // 199
    d(0xc7ce_0c7c_e0c7_ce0d, 0x0000_0000_0000_0000, -1, 0),  // 200
    d(0xc732_93d7_89b9_f839, 0x0000_0000_0000_0000, -1, 0),  // 201
    d(0xc698_0c69_80c6_980d, 0x0000_0000_0000_0000, -1, 0),  // 202
    d(0xc5fe_7403_17f9_d00d, 0x0000_0000_0000_0000, -1, 0),  // 203
    d(0xc565_c87b_5f9d_4d1d, 0x0000_0000_0000_0000, -1, 0),  // 204
    d(0xc4ce_07b0_0c4c_e07c, 0x0000_0000_0000_0000, -1, 0),  // 205
    d(0xc437_2f85_5d82_4ca7, 0x0000_0000_0000_0000, -1, 0),  // 206
    d(0xc3a1_3de6_0495_c774, 0x0000_0000_0000_0000, -1, 0),  // 207
    d(0xc30c_30c3_0c30_c30d, 0x0000_0000_0000_0000, -1, 0),  // 208
    d(0xc278_0613_c030_9e03, 0x0000_0000_0000_0000, -1, 0),  // 209
    d(0xc1e4_bbd5_95f6_e948, 0x0000_0000_0000_0000, -1, 0),  // 210
    d(0xc152_500c_1525_00c2, 0x0000_0000_0000_0000, -1, 0),  // 211
    d(0xc0c0_c0c0_c0c0_c0c2, 0x0000_0000_0000_0000, -1, 0),  // 212
    d(0xc030_0c03_00c0_300d, 0x0000_0000_0000_0000, -1, 0),  // 213
    d(0xbfa0_2fe8_0bfa_0300, 0x0000_0000_0000_0000, -1, 0),  // 214
    d(0xbf11_2a8a_d278_e8de, 0x0000_0000_0000_0000, -1, 0),  // 215
    d(0xbe82_fa0b_e82f_a0c0, 0x0000_0000_0000_0000, -1, 0),  // 216
    d(0xbdf5_9c91_700b_df5b, 0x0000_0000_0000_0000, -1, 0),  // 217
    d(0xbd69_1047_0766_1aa4, 0x0000_0000_0000_0000, -1, 0),  // 218
    d(0xbcdd_535d_b1cc_5b7c, 0x0000_0000_0000_0000, -1, 0),  // 219
    d(0xbc52_640b_c526_40bd, 0x0000_0000_0000_0000, -1, 0),  // 220
    d(0xbbc8_408c_d630_69a2, 0x0000_0000_0000_0000, -1, 0),  // 221
    d(0xbb3e_e721_a54d_880d, 0x0000_0000_0000_0000, -1, 0),  // 222
    d(0xbab6_5610_0bab_6562, 0x0000_0000_0000_0000, -1, 0),  // 223
    d(0xba2e_8ba2_e8ba_2e8d, 0x0000_0000_0000_0000, -1, 0),  // 224
    d(0xb9a7_862a_0ff4_6589, 0x0000_0000_0000_0000, -1, 0),  // 225
    d(0xb921_43fa_36f5_e02f, 0x0000_0000_0000_0000, -1, 0),  // 226
    d(0xb89b_c36c_e3e0_453b, 0x0000_0000_0000_0000, -1, 0),  // 227
    d(0xb817_02e0_5c0b_8171, 0x0000_0000_0000_0000, -1, 0),  // 228
    d(0xb793_00b7_9300_b794, 0x0000_0000_0000_0000, -1, 0),  // 229
    d(0xb70f_bb5a_19be_365a, 0x0000_0000_0000_0000, -1, 0),  // 230
    d(0xb68d_3134_0e43_07d9, 0x0000_0000_0000_0000, -1, 0),  // 231
    d(0xb60b_60b6_0b60_b60c, 0x0000_0000_0000_0000, -1, 0),  // 232
    d(0xb58a_4855_18d1_e7e5, 0x0000_0000_0000_0000, -1, 0),  // 233
    d(0xb509_e68a_9b94_8220, 0x0000_0000_0000_0000, -1, 0),  // 234
];

/// `−ln(INVERSE_2[k])` for each entry (C: `_LOG_INV_2` entries 53 to 234).
#[rustfmt::skip]
const LOG_INV_2: [Dint; 182] = [
    d(0xb180_1859_d562_49de, 0x98ce_51ff_f994_79cb, -2, 1),  // 53
    d(0xaead_eefa_caf9_7d37, 0x9dd6_e688_ebb1_3b01, -2, 1),  // 54
    d(0xabdf_ba9e_468f_d6f9, 0x472e_a077_49ce_6bd1, -2, 1),  // 55
    d(0xa915_7039_c51e_be72, 0xe164_c759_686a_2207, -2, 1),  // 56
    d(0xa64f_04f0_b961_df78, 0x54f5_275c_2d15_c21e, -2, 1),  // 57
    d(0xa38c_6e13_8e20_d834, 0xd698_298a_dddd_7f30, -2, 1),  // 58
    d(0xa0cd_a11e_af46_390e, 0x6324_3827_3918_db7d, -2, 1),  // 59
    d(0x9e12_93b9_998c_1dad, 0x3b03_5eae_273a_855c, -2, 1),  // 60
    d(0x9b5b_3bb5_f088_b768, 0x5078_bbe3_d392_be24, -2, 1),  // 61
    d(0x98a7_8f0e_9ae7_1d87, 0x64de_c347_8470_7838, -2, 1),  // 62
    d(0x95f7_83e6_e49a_9cfc, 0x0250_04f3_ef06_3312, -2, 1),  // 63
    d(0x934b_1089_a6dc_93c2, 0xdf5b_b3b6_0554_e151, -2, 1),  // 64
    d(0x90a2_2b68_75c6_a1f8, 0x8e91_aeba_609c_8876, -2, 1),  // 65
    d(0x8dfc_cb1a_d35c_a6ef, 0x9947_bdb6_ddca_f59a, -2, 1),  // 66
    d(0x8b5a_e65d_67db_9acf, 0x7ba5_1681_26a5_8b99, -2, 1),  // 67
    d(0x88bc_7411_3f23_def3, 0xbc5a_0fe3_96f4_0f1c, -2, 1),  // 68
    d(0x8621_6b3b_0b17_188c, 0x363c_eae8_8f72_0f1d, -2, 1),  // 69
    d(0x8389_c302_6ac3_139d, 0x6add_a9d2_270f_a1f3, -2, 1),  // 70
    d(0x80f5_72b1_3634_87bc, 0xedbd_0b5b_3479_d5f2, -2, 1),  // 71
    d(0xfcc8_e365_9d9b_cbf1, 0x8a0c_df30_1431_b60b, -3, 1),  // 72
    d(0xf7ad_6f26_e7ff_2efc, 0x9cd2_238f_75f9_69ad, -3, 1),  // 73
    d(0xf298_77ff_3880_9097, 0x2b02_0fa1_820c_948d, -3, 1),  // 74
    d(0xed89_ed86_a44a_01ab, 0x09d4_9f96_cb88_317a, -3, 1),  // 75
    d(0xe881_bf93_2af3_dac3, 0x2524_848e_3443_e03f, -3, 1),  // 76
    d(0xe37f_de37_807b_84e3, 0x5e9a_750b_6b68_781c, -3, 1),  // 77
    d(0xde84_39c1_dec5_687c, 0x9d57_da94_5b5d_0aa6, -3, 1),  // 78
    d(0xd98e_c2ba_de71_e53e, 0xd0a9_8f2a_d65b_ee96, -3, 1),  // 79
    d(0xd49f_69e4_56cf_1b7a, 0x5f53_bd2e_406e_66e7, -3, 1),  // 80
    d(0xcfb6_2038_44b3_209b, 0x18cb_02f3_3f79_c16b, -3, 1),  // 81
    d(0xcad2_d6e7_b80b_f915, 0xcc50_7fb7_a3d0_bf69, -3, 1),  // 82
    d(0xc5f5_7f59_c7f4_6156, 0x9a8b_6997_a402_bf30, -3, 1),  // 83
    d(0xc11e_0b2a_8d1e_0de1, 0xda63_1e83_0fd3_08fe, -3, 1),  // 84
    d(0xbc4c_6c2a_2263_99f6, 0x276e_bcfb_2016_a433, -3, 1),  // 85
    d(0xb780_945b_ab55_dcea, 0xb4c7_bc3d_3275_0fd9, -3, 1),  // 86
    d(0xb2ba_75f4_6099_cf8f, 0x243c_2e77_904a_fa76, -3, 1),  // 87
    d(0xadfa_035a_a1ed_8fdd, 0x5497_67e4_1031_6d2b, -3, 1),  // 88
    d(0xa93f_2f25_0dac_67d5, 0x9ad2_fb8d_4805_4add, -3, 1),  // 89
    d(0xa489_ec19_9dab_06f4, 0x59fb_6cf0_ecb4_11b7, -3, 1),  // 90
    d(0x9fda_2d2c_c946_5c52, 0x6b2b_9565_f535_5180, -3, 1),  // 91
    d(0x9b2f_e580_ac80_b182, 0x011a_5b94_4aca_8705, -3, 1),  // 92
    d(0x968b_0864_3409_ceb9, 0xd5c0_da50_6a08_8482, -3, 1),  // 93
    d(0x91eb_8952_4e10_0d28, 0xbfd3_df5c_52d6_7e77, -3, 1),  // 94
    d(0x8d51_5bf1_1fb9_4f22, 0xa071_3268_840c_bcbb, -3, 1),  // 95
    d(0x88bc_7411_3f23_def7, 0x9c5a_0fe3_96f4_0f19, -3, 1),  // 96
    d(0x842c_c5ac_f1d0_344b, 0x6fec_dfa8_19b9_6092, -3, 1),  // 97
    d(0xff44_89ce_deab_2ca6, 0xe17b_d40d_8d92_91ec, -4, 1),  // 98
    d(0xf639_cc18_5088_fe62, 0x5066_e87f_2c0f_733d, -4, 1),  // 99
    d(0xed39_3b1c_2235_1281, 0xff4e_2e66_0317_d55f, -4, 1),  // 100
    d(0xe442_c00d_e259_1b4c, 0xe96a_b34c_e0bc_cd10, -4, 1),  // 101
    d(0xdb56_446d_6ad8_df09, 0x2811_2e35_a60e_636f, -4, 1),  // 102
    d(0xd273_b205_8de1_bd4b, 0x36bb_f837_b4d3_20c6, -4, 1),  // 103
    d(0xc99a_f2ea_ca4c_457b, 0xeaf5_1f66_6928_44b2, -4, 1),  // 104
    d(0xc0cb_f17a_071f_80e9, 0x396f_fdf7_6a14_7cc2, -4, 1),  // 105
    d(0xb806_9857_5607_07a7, 0x0a67_7b4c_8bec_22e0, -4, 1),  // 106
    d(0xaf4a_d26c_bc8e_5bef, 0x9e8b_8b88_a14f_f0c9, -4, 1),  // 107
    d(0xa698_8ae9_03f5_62f1, 0x7e85_8f08_597b_3a68, -4, 1),  // 108
    d(0x9def_ad3e_8f73_2186, 0x476d_3b5b_45f6_ca02, -4, 1),  // 109
    d(0x9550_2522_38bd_2468, 0x658e_5a0b_811c_596d, -4, 1),  // 110
    d(0x8cb9_de8a_32ab_3694, 0x97c9_8595_30a4_514c, -4, 1),  // 111
    d(0x842c_c5ac_f1d0_344c, 0x1fec_dfa8_19b9_6094, -4, 1),  // 112
    d(0xf751_8e00_35c3_dd92, 0x606d_8909_3278_a931, -5, 1),  // 113
    d(0xe65b_9e6e_ed96_5c4f, 0x609f_5fe2_058d_5ff2, -5, 1),  // 114
    d(0xd577_9687_d887_e0ee, 0x49dd_a170_56e4_5ebb, -5, 1),  // 115
    d(0xc4a5_50a4_fd9a_19bb, 0x3e97_660a_23cc_5402, -5, 1),  // 116
    d(0xb3e4_a796_a5da_c213, 0x07cc_a0bc_c06c_2f8e, -5, 1),  // 117
    d(0xa335_76a1_6f1f_4c79, 0x1210_16bd_904d_c95a, -5, 1),  // 118
    d(0x9297_997c_68c1_f4e6, 0x610d_b3d4_dd42_3bc9, -5, 1),  // 119
    d(0x820a_ec4f_3a22_2397, 0xb9e3_aea6_c444_eef6, -5, 1),  // 120
    d(0xe31e_9760_a557_8c6d, 0xf9eb_2f28_4f31_c35a, -6, 1),  // 121
    d(0xc249_2946_4655_f482, 0xda5f_3cc0_b325_1da6, -6, 1),  // 122
    d(0xa195_492c_c066_0519, 0x4a18_dff7_cdb4_ae33, -6, 1),  // 123
    d(0x8102_b2c4_9ac2_3a86, 0x91d0_82dc_e3dd_cd08, -6, 1),  // 124
    d(0xc122_451c_4515_5150, 0xb161_37f0_9a00_2b0e, -7, 1),  // 125
    d(0x8080_abac_46f3_89c4, 0x662d_417c_ed00_79c9, -7, 1),  // 126
    d(0x0000_0000_0000_0000, 0x0000_0000_0000_0000, 127, 0),  // 127
    d(0x0000_0000_0000_0000, 0x0000_0000_0000_0000, 127, 0),  // 128
    d(0xff80_5515_885e_014e, 0x435a_b4da_6a5b_b50f, -9, 0),  // 129
    d(0xff01_5358_833c_4762, 0xbb48_1c8e_e141_6999, -8, 0),  // 130
    d(0xbee2_3afc_0853_b6a8, 0xa897_82c2_0df3_50c2, -7, 0),  // 131
    d(0xfe05_4587_e01f_1e2b, 0xf6d3_a69b_d5ea_b72f, -7, 0),  // 132
    d(0x9e75_221a_352b_a751, 0x452b_7ea6_2f21_98ea, -6, 0),  // 133
    d(0xbdc8_d83e_ad88_d518, 0x7faa_638b_5e00_ee90, -6, 0),  // 134
    d(0xdcfe_013d_7c8c_bfc5, 0x632d_bac4_6f30_d009, -6, 0),  // 135
    d(0xfc14_d873_c198_0236, 0xc7e0_9e3d_e453_f5fc, -6, 0),  // 136
    d(0x8d86_cc49_1ecb_fe03, 0xf177_6453_b7e8_2558, -5, 0),  // 137
    d(0x9cf4_3dcf_f5ea_fd2f, 0x2ad9_0155_c8a7_236a, -5, 0),  // 138
    d(0xac52_dd7e_4726_a456, 0xa47a_963a_91bb_3018, -5, 0),  // 139
    d(0xbba2_c7b1_96e7_e224, 0xe795_0f72_52c1_63cf, -5, 0),  // 140
    d(0xcae4_1876_471f_5bde, 0x91d0_0a41_7e33_0f8e, -5, 0),  // 141
    d(0xda16_eb88_cb8d_f5fb, 0x28a6_3ecf_b66e_94c0, -5, 0),  // 142
    d(0xe93b_5c56_d85a_9083, 0xce29_92bf_ea38_e76b, -5, 0),  // 143
    d(0xf851_8600_8b15_32f9, 0xe64b_8b77_5997_8998, -5, 0),  // 144
    d(0x83ac_c1ac_c723_8978, 0x5a53_33c4_5b7f_442e, -4, 0),  // 145
    d(0x8b29_b775_1bd7_073b, 0x02e0_b9ee_992f_2372, -4, 0),  // 146
    d(0x929f_b178_50a0_b7be, 0x5b4d_3807_6605_16a4, -4, 0),  // 147
    d(0x9a0e_bcb0_de8e_848e, 0x2c1b_b082_689b_a814, -4, 0),  // 148
    d(0xa176_e5f5_3237_81d2, 0xdcf9_3599_6c92_e8d4, -4, 0),  // 149
    d(0xa8d8_39f8_30c1_fb40, 0x4c73_4351_7c8a_c264, -4, 0),  // 150
    d(0xb032_c549_ba86_1d83, 0x774e_27bc_92ce_3373, -4, 0),  // 151
    d(0xb786_9457_2b5a_5cd3, 0x24cd_cf68_cdb2_067c, -4, 0),  // 152
    d(0xbed3_b36b_d896_6419, 0x7c06_44d7_d9ed_08b4, -4, 0),  // 153
    d(0xc61a_2eb1_8cd9_07a1, 0xe5a1_532f_6d5a_1ac1, -4, 0),  // 154
    d(0xcd5a_1231_019d_66d7, 0x761e_3e7b_171e_44b2, -4, 0),  // 155
    d(0xd493_69d2_56ab_1b1f, 0x9e91_54e1_d526_3cda, -4, 0),  // 156
    d(0xdbc6_415d_876d_0839, 0x3e33_c0c9_f882_4f54, -4, 0),  // 157
    d(0xe2f2_a47a_de3a_18a8, 0xa0bf_7c0b_0d8b_b4ef, -4, 0),  // 158
    d(0xea18_9eb3_659a_eaeb, 0x93b2_a3b2_1f44_8259, -4, 0),  // 159
    d(0xf138_3b71_5797_2f48, 0x543f_ff0f_f4f0_aaf1, -4, 0),  // 160
    d(0xf851_8600_8b15_3302, 0x5e4b_8b77_5997_8993, -4, 0),  // 161
    d(0xff64_898e_df55_d548, 0x428c_cfc9_9271_dffa, -4, 0),  // 162
    d(0x8338_a896_52cb_714a, 0xb247_eb86_498c_2ce7, -3, 0),  // 163
    d(0x86bb_f3e6_8472_cb2f, 0x0b8b_d206_1574_7126, -3, 0),  // 164
    d(0x8a3c_2c23_3a15_6341, 0x9027_c74f_e0e6_f64f, -3, 0),  // 165
    d(0x8db9_56a9_7b3d_0143, 0xf023_472c_d739_f9e1, -3, 0),  // 166
    d(0x9133_78c8_52d6_5be6, 0x977e_3013_d10f_7525, -3, 0),  // 167
    d(0x94aa_97c0_ffa9_1a5d, 0x4ee3_880f_b7d3_4429, -3, 0),  // 168
    d(0x981e_b8c7_23fe_97f2, 0x1f1c_134f_b702_d433, -3, 0),  // 169
    d(0x9b8f_e100_f47b_a1d8, 0x04b6_2af1_89fc_ba0d, -3, 0),  // 170
    d(0x9efe_1587_6631_4e4f, 0x4d71_827e_fe89_2fc8, -3, 0),  // 171
    d(0xa269_5b66_5be8_f338, 0x4eca_87c3_f0f0_6211, -3, 0),  // 172
    d(0xa5d1_b79c_d2af_2aca, 0x8837_986c_eabf_bed6, -3, 0),  // 173
    d(0xa937_2f1d_0da1_bd10, 0x580e_b71e_58cd_36e5, -3, 0),  // 174
    d(0xac99_c6cc_c104_2e94, 0x3dd5_5752_8315_838d, -3, 0),  // 175
    d(0xaff9_8385_3c9e_9e40, 0x5f10_5039_091d_d7f5, -3, 0),  // 176
    d(0xb356_6a13_956a_86f4, 0x471b_1e15_74d9_fd55, -3, 0),  // 177
    d(0xb6b0_7f38_ce90_e463, 0x7bb2_e265_d0de_37e1, -3, 0),  // 178
    d(0xba07_c7aa_01bd_2648, 0x43f9_d57b_324b_d05f, -3, 0),  // 179
    d(0xbd5c_4810_86c8_48db, 0xbb59_6b50_3040_3242, -3, 0),  // 180
    d(0xc0ae_050a_1abf_56ad, 0x2f7f_8c5f_a9c5_0d76, -3, 0),  // 181
    d(0xc3fd_0329_0648_847d, 0x3048_0bee_4cbb_d698, -3, 0),  // 182
    d(0xc749_46f4_436a_054e, 0xf4f5_cb53_1201_c0d3, -3, 0),  // 183
    d(0xca92_d4e7_a2b5_a3ad, 0xc983_a9c5_c4b3_b135, -3, 0),  // 184
    d(0xcdd9_b173_efdc_1aaa, 0x8863_e007_c184_a1e7, -3, 0),  // 185
    d(0xd11d_e0ff_15ab_18c6, 0xd88d_83d4_cc61_3f21, -3, 0),  // 186
    d(0xd45f_67e4_4178_c612, 0x5486_e73c_6151_58b4, -3, 0),  // 187
    d(0xd79e_4a74_05ff_96c3, 0x1300_c9be_67ae_5da0, -3, 0),  // 188
    d(0xdada_8cf4_7dad_236d, 0xdffb_833c_3409_ee7e, -3, 0),  // 189
    d(0xde14_33a1_6c66_b14c, 0xde74_4870_f54f_0f18, -3, 0),  // 190
    d(0xe14b_42ac_60c6_0512, 0x4e38_eb80_92a0_1f06, -3, 0),  // 191
    d(0xe47f_be3c_d4d1_0d5b, 0x2ec0_f797_fdcd_125c, -3, 0),  // 192
    d(0xe7b1_aa70_4e2e_e240, 0xb40f_aab6_d2ad_0841, -3, 0),  // 193
    d(0xeae1_0b5a_7ddc_8ad8, 0x806b_2fc9_a803_8790, -3, 0),  // 194
    d(0xee0d_e505_5f63_eb01, 0x90a3_3316_df83_ba5a, -3, 0),  // 195
    d(0xf138_3b71_5797_2f4a, 0xb43f_ff0f_f4f0_aaf1, -3, 0),  // 196
    d(0xf460_1295_52d2_ff41, 0xe62e_3201_bb2b_bdce, -3, 0),  // 197
    d(0xf785_6e5e_e2c9_b28a, 0x76f2_a1b8_4190_a7dc, -3, 0),  // 198
    d(0xfaa8_52b2_5bd9_b833, 0xa6db_fa03_186e_0666, -3, 0),  // 199
    d(0xfdc8_c36a_f1f1_5468, 0x0a33_61bc_a696_504a, -3, 0),  // 200
    d(0x8073_622d_6a80_e631, 0xe897_0090_1531_6073, -2, 0),  // 201
    d(0x8201_2ca5_a682_06d5, 0x8fde_85af_dd2b_c88a, -2, 0),  // 202
    d(0x838d_c2fe_6ac8_68e7, 0x1a3f_cbde_f401_00cb, -2, 0),  // 203
    d(0x8519_2713_9c87_1af8, 0x67bd_00c3_8061_c51f, -2, 0),  // 204
    d(0x86a3_5abc_d5ba_5901, 0x5481_c3cb_d925_ccd2, -2, 0),  // 205
    d(0x882c_5fcd_7256_a8c1, 0x3905_5a65_98e7_c29e, -2, 0),  // 206
    d(0x89b4_3814_9d45_82f5, 0x3453_1dba_493e_b5a6, -2, 0),  // 207
    d(0x8b3a_e55d_5d30_701a, 0xc63e_ab88_3717_0480, -2, 0),  // 208
    d(0x8cc0_696e_a11b_7b36, 0x9436_1c9a_28d3_8a6a, -2, 0),  // 209
    d(0x8e44_c60b_4ccf_d7dc, 0x1473_aa01_c777_8679, -2, 0),  // 210
    d(0x8fc7_fcf2_4517_946a, 0x380c_be76_9f2c_6793, -2, 0),  // 211
    d(0x914a_0fde_7bcb_2d0e, 0xc429_ed3a_ea19_7a60, -2, 0),  // 212
    d(0x92cb_0086_fbb1_cf75, 0xa29d_47c5_0b11_82d0, -2, 0),  // 213
    d(0x944a_d09e_f435_1af1, 0xa498_27e0_81cb_16ba, -2, 0),  // 214
    d(0x95c9_81d5_c4e9_24ea, 0x4540_4f5a_a577_d6b4, -2, 0),  // 215
    d(0x9747_15d7_08e9_84dd, 0x6648_d428_40d9_e6fb, -2, 0),  // 216
    d(0x98c3_8e4a_a20c_27d2, 0x8467_67ec_990d_7333, -2, 0),  // 217
    d(0x9a3e_ecd4_c3ea_a6ae, 0xdb3a_7f6e_6087_b947, -2, 0),  // 218
    d(0x9bb9_3315_fec2_d790, 0x7f58_9fba_0865_790f, -2, 0),  // 219
    d(0x9d32_62ab_4a2f_4e37, 0xa1ae_6ba0_6846_fae0, -2, 0),  // 220
    d(0x9eaa_7d2e_0fb8_7c35, 0xff47_2bc6_ce64_8a7d, -2, 0),  // 221
    d(0xa021_8434_353f_1de4, 0xd493_efa6_3253_0acc, -2, 0),  // 222
    d(0xa197_7950_2740_9daa, 0x1dd1_d4a6_df96_0357, -2, 0),  // 223
    d(0xa30c_5e10_e2f6_13e4, 0x9bd9_bd99_e39a_20b3, -2, 0),  // 224
    d(0xa480_3402_004e_865c, 0x31cb_e0e8_8241_16cd, -2, 0),  // 225
    d(0xa5f2_fcab_bbc5_06d8, 0x68ca_4fb7_ec32_3d74, -2, 0),  // 226
    d(0xa764_b993_0013_4d79, 0x0d04_d104_7430_1862, -2, 0),  // 227
    d(0xa8d5_6c39_6fc1_684c, 0x01eb_067d_578c_4756, -2, 0),  // 228
    d(0xaa45_161d_6e93_167b, 0x9b08_1cf7_2249_f5b2, -2, 0),  // 229
    d(0xabb3_b8ba_2ad3_62a1, 0x1db6_506c_c17a_01f5, -2, 0),  // 230
    d(0xad21_5587_a67f_0cdf, 0xe890_422c_b86b_7cb1, -2, 0),  // 231
    d(0xae8d_edfa_c04e_5282, 0xac70_7b8f_fc22_b3e8, -2, 0),  // 232
    d(0xaff9_8385_3c9e_9e3f, 0xc510_5039_091d_d7f8, -2, 0),  // 233
    d(0xb164_1795_ce3c_a978, 0xfaf9_1530_0e51_7393, -2, 0),  // 234
];

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
#[allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "the quantities of the proof, named as the module documentation names them"
)]
mod tests {
    //! The numeric ingredients of the proof in the module documentation, checked exhaustively or
    //! with exact arithmetic. Each test names the step of the proof it checks.

    use super::*;
    use crate::math::reference::*;
    use core::cmp::Ordering;

    /// Working precision of the exact checks.
    const P: u64 = 256;

    /// `d`, the tables' shorthand, which only the constant tables call (so only the compiler ran
    /// it): each argument lands in its own field.
    #[test]
    fn the_table_shorthand_fills_each_field() {
        let x = d(0x8000_0000_0000_0001, 2, -3, 1);
        assert_eq!((x.hi, x.lo, x.ex, x.sgn), (0x8000_0000_0000_0001, 2, -3, 1));
    }

    /// u = 2^−53.
    fn u() -> Dy {
        p2(-53)
    }

    /// `a ≤ b`.
    fn le(a: &Dy, b: &Dy) -> bool {
        a.cmp(b) != Ordering::Greater
    }

    /// `ln(v)` for a positive dyadic interval, by the oracle's atanh series around 1.
    fn ln_iv(v: &Iv) -> Iv {
        // v ∈ (0.70, 1.42): ln v = 2·atanh((v − 1)/(v + 1))
        let one = Iv::int(1);
        let s = v.sub(&one, P).div(&v.add(&one, P), P);
        atanh_iv(&s, P).scale(1)
    }

    /// The exact value of a [`Dint`].
    fn dint_value(d: Dint) -> Dy {
        let m = (num_bigint::BigInt::from(d.hi) << 64usize) + num_bigint::BigInt::from(d.lo);
        let v = Dy { m, e: d.ex - 127 };
        if d.sgn != 0 { v.neg() } else { v }
    }

    /// The largest `|z|` the fast path can see: `|r·y − 1|` at the ends of every interval.
    const Z_FAST: f64 = 0.002_120_971_679_687_35;
    /// The fast polynomial's interval, from CORE-MATH: [−0.00202941894531250, `Z_FAST`].
    const Z_FAST_NEG: f64 = 0.002_029_418_945_312_5;
    /// The fast polynomial's approximation error on that interval, as bounded by
    /// `fast_polynomial_and_log2` (CORE-MATH's Sollya figure is 6.98575e−22).
    const EPS_P: f64 = 7.1e-22;

    /// Step F1: the fast tables. For every interval `i·2^−9 ≤ y < (i + 1)·2^−9`, `z = r·y − 1` is
    /// exact (it fits in 53 bits), `−0.00202941894531250 ≤ z ≤ Z_FAST`, `l1` is a multiple of
    /// 2^−42 with `e·LOG2_H + l1` exact, and `|l1 + l2 + ln r| ≤ 2^−96`.
    #[test]
    fn fast_tables() {
        let mut worst = Dy::zero();
        for i in 362..=724i64 {
            let k = (i - 362) as usize;
            let r = INVERSE[k];
            let [l1, l2] = LOG_INV[k];
            // y = v·½ for i < 512 (lsb 2^−53), y = v for i ≥ 512 (lsb 2^−52)
            let lsb_y = if i < 512 { -53 } else { -52 };
            let lsb_r = i64::from(r.to_bits().trailing_zeros()).min(52) - 52
                + ((r.to_bits() >> 52) as i64 - 1023);
            for (yv, closed) in [
                (Dy { m: i.into(), e: -9 }, true),
                (
                    Dy {
                        m: (i + 1).into(),
                        e: -9,
                    },
                    false,
                ),
            ] {
                // at the open end use the largest double below (i + 1)·2^−9
                let y = if closed { yv } else { yv.sub(&p2(lsb_y)) };
                let z = Dy::from_f64(r).mul(&y).sub(&Dy::int(1));
                assert!(le(&z.abs(), &p2(53 + lsb_r + lsb_y)), "i = {i}: z exact");
                assert!(
                    le(&z, &Dy::from_f64(Z_FAST)) && le(&Dy::from_f64(-Z_FAST_NEG), &z),
                    "i = {i}: z range"
                );
            }
            let l1d = Dy::from_f64(l1);
            assert!(
                l1d.is_zero()
                    || l1d.e >= -42
                    || i64::from(l1.to_bits().trailing_zeros())
                        >= -42 - ((l1.abs().to_bits() >> 52) as i64 - 1075)
            );
            assert!(l2.abs() < f64::EPSILON * 512.0); // 2^−43
            let lr = ln_iv(&Iv::point(Dy::from_f64(r)));
            let s = l1d.add(&Dy::from_f64(l2));
            worst = dmax(worst, dmax(s.add(&lr.lo).abs(), s.add(&lr.hi).abs()));
        }
        eprintln!(
            "fast table: |l1 + l2 + ln r| ≤ 2^{:.3}",
            log2_approx(&worst)
        );
        assert!(le(&worst, &p2(-96)));
        // e·LOG2_H and e·LOG2_H + l1 are exact: both are multiples of 2^−42 below 2^11 in
        // magnitude (|e| ≤ 1075, LOG2_H < 1), so each fits in 53 bits.
        assert!(hf!("0x1.62e42fefa38p-1").to_bits().trailing_zeros() >= 10);
        // z = (r·y_hi − 1) + r·y_lo is exact (log_fast): every r has at most 10 significant bits.
        for &r in &INVERSE {
            let m = (r.to_bits() & ((1 << 52) - 1)) | (1 << 52);
            assert!(m.trailing_zeros() >= 43, "r = {r:e} has more than 10 bits");
        }
    }

    /// Step F2: the fast polynomial and the split of ln 2.
    #[test]
    fn fast_polynomial_and_log2() {
        const PP: [f64; 6] = [
            hf!("0x1p0"),
            hf!("-0x1.ffffffffffffap-2"),
            hf!("0x1.555555554f4d8p-2"),
            hf!("-0x1.0000000537df6p-2"),
            hf!("0x1.999a14758b084p-3"),
            hf!("-0x1.55362255e0f63p-3"),
        ];
        // ln(1 + z) = Σ (−1)^(n+1) z^n/n to degree 14, then a remainder ≤ |z|^15/15/(1 − |z|)
        let target: Vec<Iv> = (0..15)
            .map(|n| {
                if n == 0 {
                    Iv::int(0)
                } else {
                    let v = Iv::int(1).div_int(i64::from(n), P);
                    if n % 2 == 0 { v.neg() } else { v }
                }
            })
            .collect();
        let coeffs: Vec<(usize, f64)> = PP.iter().enumerate().map(|(k, &c)| (k + 1, c)).collect();
        let q = error_poly(&target, &coeffs, 0, 1);
        let qneg: Vec<Iv> = q
            .iter()
            .enumerate()
            .map(|(j, c)| if j % 2 == 1 { c.neg() } else { c.clone() })
            .collect();
        let zp = Dy::from_f64(Z_FAST);
        let zn = Dy::from_f64(Z_FAST_NEG);
        let sup = dmax(
            poly_sup_bound(&q, &zp, 400, 192),
            poly_sup_bound(&qneg, &zn, 400, 192),
        );
        let zm = dmax(zp, zn);
        let tail = Dy::div(
            &(0..15).fold(Dy::int(1), |a, _| a.mul(&zm)),
            &Dy::int(14),
            P,
            true,
        );
        let eps_p = sup.add(&tail);
        eprintln!(
            "fast polynomial: 2^{:.3} (CORE-MATH: 6.98575e−22 = 2^−70.278)",
            log2_approx(&eps_p)
        );
        assert!(le(&eps_p, &Dy::from_f64(EPS_P)));
        let l2 =
            Dy::from_f64(hf!("0x1.62e42fefa38p-1")).add(&Dy::from_f64(hf!("0x1.ef35793c7673p-45")));
        let d = dmax(ln2().lo.sub(&l2).abs(), ln2().hi.sub(&l2).abs());
        assert!(
            le(&d, &Dy::from_f64(1.95853e-31)),
            "|ln 2 − LOG2_H − LOG2_L| ≤ 1.95853e−31"
        );
    }

    /// Steps F3 and F4: the fast path's total error, CORE-MATH's hand analysis re-derived with
    /// u-based rounding bounds, plus the rounding of `l ± ERR` in the test (Lemma 1).
    #[test]
    fn fast_path_bound() {
        let u = u();
        let one = Dy::int(1);
        let up = |d: &Dy, k: i64| d.mul(&one.add(&p2(-k)));
        let zm = Dy::from_f64(Z_FAST);
        let z2 = zm.mul(&zm);
        // running error of ph = (((P5 z + P4) z² + (P3 z + P2)) z + P1)·z², each fma rounded once,
        // z² rounded once (u·z²), magnitudes from the comments: |p45| < 0.22, |p23| < 0.35,
        // |ph1| < 0.36, |ph2| < 0.501
        // Each Horner step `a·z + b` is a rounded product and a rounded sum (this port has no
        // fused operation there), so it adds u·|a·z| to the u·|result| of the C source's fma.
        let e_z2 = u.mul(&z2);
        let e_p45 = u
            .mul(&Dy::from_f64(0.22))
            .add(&u.mul(&Dy::from_f64(hf!("0x1.55362255e0f63p-3")).mul(&zm)));
        let e_p23 = u
            .mul(&Dy::from_f64(0.35))
            .add(&u.mul(&Dy::from_f64(hf!("0x1.0000000537df6p-2")).mul(&zm)));
        let e_ph1 = u
            .mul(&Dy::from_f64(0.36))
            .add(&u.mul(&Dy::from_f64(0.22)).mul(&z2))
            .add(&e_p45.mul(&z2))
            .add(&Dy::from_f64(0.22).mul(&e_z2))
            .add(&e_p23);
        let e_ph2 = u
            .mul(&Dy::from_f64(0.501))
            .add(&u.mul(&Dy::from_f64(0.36)).mul(&zm))
            .add(&e_ph1.mul(&zm));
        let ph3 = up(&Dy::from_f64(0.501).mul(&z2), 40);
        let e_ph3 = u
            .mul(&ph3)
            .add(&e_ph2.mul(&z2))
            .add(&Dy::from_f64(0.501).mul(&e_z2));
        // l_2 = RN(ph3 + RN(l_1 + l2)): |l_1| ≤ 2^−44 (half an ulp of h < 745), |l2| < 2^−43
        let l12 = p2(-44).add(&p2(-43));
        let l_2 = up(&ph3.add(&l12), 50);
        // l_3 = RN(RN(e·LOG2_L) + l_2), |e| ≤ 1075: the product is rounded too (no fma here)
        let el = Dy::int(1075).mul(&Dy::from_f64(hf!("0x1.ef35793c7673p-45")));
        let l_3 = up(&l_2.add(&up(&el, 53)), 50);
        let e = Dy::from_f64(EPS_P) // polynomial
            .add(&Dy::int(1075).mul(&Dy::from_f64(1.95853e-31))) // e·(ln 2 split)
            .add(&p2(-96)) // table
            .add(&e_ph3) // polynomial evaluation
            .add(&u.mul(&l12)) // RN(l_1 + l2)
            .add(&u.mul(&l_2)) // RN(ph3 + …)
            .add(&u.mul(&el)) // the rounded product e·LOG2_L
            .add(&u.mul(&l_3)); // the last sum
        let err = Dy::from_f64(ERR);
        let need = e.add(&u.mul(&l_3.add(&err)));
        let have = err.mul(&one.sub(&u));
        eprintln!(
            "fast path: |h + l − ln x| ≤ 2^{:.4} (CORE-MATH: 2^−68.22 by hand, 2.8925e−21 by Gappa); with the test's rounding 2^{:.4} ≤ ERR(1 − u) = 2^{:.4}",
            log2_approx(&e),
            log2_approx(&need),
            log2_approx(&have)
        );
        assert!(le(&need, &have));
    }

    /// Steps A1 and A2: the accurate path's relative error is below 2^−101, checked for every
    /// table index against the smallest |ln x| it can produce.
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one proof step, its ingredients computed and checked in order"
    )]
    fn accurate_path_bound() {
        let one = Dy::int(1);
        let ulp = p2(-127); // one unit in the last place of a Dint, relative
        let mul = ulp.mul(&Dy::int(5)); // mul_dint: below 5 ulps of the result
        let add = ulp.mul(&Dy::int(2)); // add_dint: below 2 ulps of the larger operand
        let p2c: Vec<Dy> = P2_COEFFS.iter().map(|&d| dint_value(d)).collect(); // degree 12 first
        let ln_target: Vec<Iv> = (0..=24)
            .map(|n| {
                if n == 0 {
                    Iv::int(0)
                } else {
                    let v = Iv::int(1).div_int(i64::from(n), P);
                    if n % 2 == 0 { v.neg() } else { v }
                }
            })
            .collect();
        let mut worst = Dy::zero();
        for (k, (&inv_d, &loginv_d)) in INVERSE_2.iter().zip(LOG_INV_2.iter()).enumerate() {
            let i = (k + 181) as i64;
            let inv = dint_value(inv_d);
            let loginv = dint_value(loginv_d);
            let exact = ln_iv(&Iv::point(inv.clone())).neg();
            let table = dmax(loginv.sub(&exact.lo).abs(), loginv.sub(&exact.hi).abs());
            // the reduced x lies in [i/256, (i + 1)/256)
            let (xa, xb) = (
                Dy { m: i.into(), e: -8 },
                Dy {
                    m: (i + 1).into(),
                    e: -8,
                },
            );
            let (za, zb) = (xa.mul(&inv).sub(&one), xb.mul(&inv).sub(&one));
            let zmax = dmax(za.abs(), zb.abs());
            // the polynomial used: P_2, or the Taylor series at i = 255 (z ≤ 0 there)
            let coeffs: Vec<Dy> = if k as u64 == NEAR_ONE_BELOW {
                LOG1P_TAYLOR.iter().map(|&d| dint_value(d)).collect()
            } else {
                p2c.clone()
            };
            let degree = coeffs.len(); // z·P(z) has this degree
            // approximation error of z·P(z) for ln(1 + z) on [za, zb] (subdivision, by sign)
            let mut q = ln_target.clone();
            for (j, c) in coeffs.iter().enumerate() {
                q[degree - j] = q[degree - j].sub(&Iv::point(c.clone()), P);
            }
            let qneg: Vec<Iv> = q
                .iter()
                .enumerate()
                .map(|(j, c)| if j % 2 == 1 { c.neg() } else { c.clone() })
                .collect();
            let tail = Dy::div(
                &(0..25).fold(Dy::int(1), |a, _| a.mul(&zmax)),
                &Dy::int(24),
                P,
                true,
            )
            .mul(&Dy::int(2));
            let pos = if zb.signum() > 0 {
                poly_sup_bound(&q, &zb, 128, 192)
            } else {
                Dy::zero()
            };
            let neg = if za.signum() < 0 {
                poly_sup_bound(&qneg, &za.abs(), 128, 192)
            } else {
                Dy::zero()
            };
            let approx = dmax(pos, neg).add(&tail);
            // Horner's running error: r ← mul(r, z) then add(r, c), then a final mul by z
            let mut r = coeffs[0].abs();
            let mut e = Dy::zero();
            for c in &coeffs[1..] {
                let rz = r.mul(&zmax);
                e = e.mul(&zmax).add(&mul.mul(&rz));
                let s = rz.add(&c.abs());
                e = e.add(&add.mul(&dmax(rz, c.abs())));
                r = s;
            }
            let pz = r.mul(&zmax);
            let horner = e.mul(&zmax).add(&mul.mul(&pz));
            // z's own error: mul_dint(x, inv) and the add of −1; none when inv = 1
            let z_err = if inv.eq(&one) {
                Dy::zero()
            } else {
                mul.mul(&Dy::from_f64(1.008)).add(&add)
            };
            let lnz = zmax.mul(&Dy::from_f64(1.01)); // ≥ |ln(1 + z)|
            let p_err = approx.add(&horner).add(&z_err.mul(&Dy::from_f64(1.01)));
            // E = 0: the add of ln(inv) (and of E·ln 2 = 0, exact)
            let e0 = table
                .add(&p_err)
                .add(&add.mul(&dmax(lnz.clone(), loginv.abs())));
            let rel0 = if i == 255 || i == 256 {
                // inv = 1, ln(inv) = 0: everything is relative to |z|, and |ln x| ≥ 0.99·|z|
                assert!(inv.eq(&one) && loginv.is_zero());
                let horner_rel = Dy::div(&horner, &pz, P, true);
                // the approximation error divided by z, bounded as a polynomial on the interval
                let qd: Vec<Iv> = q[1..].to_vec();
                let qdn: Vec<Iv> = qd
                    .iter()
                    .enumerate()
                    .map(|(j, c)| if j % 2 == 1 { c.neg() } else { c.clone() })
                    .collect();
                let tail_rel = Dy::div(&tail, &zmax, P, true);
                let pos = if zb.signum() > 0 {
                    poly_sup_bound(&qd, &zb, 128, 192)
                } else {
                    Dy::zero()
                };
                let neg = if za.signum() < 0 {
                    poly_sup_bound(&qdn, &za.abs(), 128, 192)
                } else {
                    Dy::zero()
                };
                let approx_rel = dmax(pos, neg).add(&tail_rel);
                Dy::div(&horner_rel.add(&approx_rel), &Dy::from_f64(0.99), P, true)
            } else {
                let la = ln_iv(&Iv::point(xa.clone()));
                let lb = ln_iv(&Iv::point(xb.clone()));
                assert!(
                    la.lo.signum() == lb.hi.signum() && la.lo.signum() != 0,
                    "i = {i}: ln x keeps its sign"
                );
                let vmin = [la.lo.abs(), la.hi.abs(), lb.lo.abs(), lb.hi.abs()]
                    .into_iter()
                    .reduce(|a, b| if a.cmp(&b) == Ordering::Less { a } else { b })
                    .unwrap();
                Dy::div(&e0, &vmin, P, true)
            };
            // |E| ≥ 1, up to 1075: LOG2's error (≤ 2^−129.97), mul_dint_2 and the last add, against
            // |ln x| ≥ ln 2 − |ln(reduced x)| ≥ 0.346
            let big_e = Dy::int(1075);
            let eln2 = big_e.mul(&ln2().hi);
            let e1 = e0
                .add(&big_e.mul(&p2(-129)))
                .add(&ulp.mul(&Dy::int(3)).mul(&eln2))
                .add(&add.mul(&eln2));
            let rel1 = Dy::div(&e1, &Dy::from_f64(0.346), P, true);
            if std::env::var_os("GEOM_NUM_PROOF_DETAIL").is_some() {
                eprintln!(
                    "i = {i}: table 2^{:.1} approx 2^{:.1} horner 2^{:.1} z 2^{:.1} rel0 2^{:.2} rel1 2^{:.2}",
                    log2_approx(&table),
                    log2_approx(&approx),
                    log2_approx(&horner),
                    log2_approx(&z_err),
                    log2_approx(&rel0),
                    log2_approx(&rel1)
                );
            }
            worst = dmax(worst, dmax(rel0, rel1));
        }
        eprintln!(
            "accurate path: relative error ≤ 2^{:.3}",
            log2_approx(&worst)
        );
        assert!(le(&worst, &p2(-101)));
    }

    /// `mul_int`'s checked sum never fails. Its only call is `mul_int(E, LOG2)`, with `E` the
    /// input's binary exponent (`|E| ≤ 1075`), and for every `|E| ≤ 2000` the sum `l + t` that
    /// could carry out of 128 bits fits: `mul_int`'s own steps up to that sum, repeated here.
    #[test]
    fn mul_int_never_carries_for_ln() {
        for b in (-2000i64..=2000).filter(|&b| b != 0) {
            let c = u128::from(b.unsigned_abs());
            let mut t = u128::from(LOG2.hi) * c;
            let hi = hi64(t);
            let m = if hi != 0 { hi.leading_zeros() } else { 64 };
            t <<= m;
            let l = shl128(u128::from(LOG2.lo) * c, u64::from(m - 1)) >> 63;
            assert!(l.checked_add(t).is_some(), "E = {b}");
        }
    }

    /// `Dint::add`'s arms that `ln`'s evaluation never takes (see the test above), each checked
    /// against the exact sum of the operands: equal magnitudes (exactly zero, or exactly twice),
    /// an addend 128 places or more below the other (the larger, unchanged), an aligned addend
    /// whose rounding carries out of 128 bits, and a difference that this rounding cancels to zero.
    /// Each result is within two units of the larger operand's last place of the exact sum.
    #[test]
    fn dint_add_arms_ln_never_takes() {
        let x = Dint {
            hi: 0xb504_f333_f9de_6484,
            lo: 0x5979_6b3e_9f7a_3c21,
            ex: 3,
            sgn: 0,
        };
        let close = |a: Dint, b: Dint| {
            let r = a.add(b).unwrap();
            let exact = dint_value(a).add(&dint_value(b));
            // Two units of the last place of the larger operand, 2^(ex − 127) each.
            let two_ulps = Dy::pow2(a.ex.max(b.ex) - 126);
            assert!(
                le(&dint_value(r).sub(&exact).abs(), &two_ulps),
                "({:x}, {:x}, {}, {}) + ({:x}, {:x}, {}, {}) = ({:x}, {:x}, {}, {})",
                a.hi,
                a.lo,
                a.ex,
                a.sgn,
                b.hi,
                b.lo,
                b.ex,
                b.sgn,
                r.hi,
                r.lo,
                r.ex,
                r.sgn
            );
            r
        };
        // Equal magnitudes: opposite signs give exactly zero, equal signs exactly twice.
        assert!(close(x, Dint { sgn: 1, ..x }).is_zero());
        let twice = close(x, x);
        assert_eq!((twice.hi, twice.lo, twice.ex), (x.hi, x.lo, x.ex + 1));
        // An addend 200 places below: the larger, unchanged, in either order.
        let tiny = Dint {
            ex: x.ex - 200,
            ..x
        };
        for r in [close(x, tiny), close(tiny, x)] {
            assert_eq!((r.hi, r.lo, r.ex, r.sgn), (x.hi, x.lo, x.ex, x.sgn));
        }
        // An all-ones significand one place below 2^ex: aligned, its rounding bit carries it out
        // of 128 bits, to exactly 2^ex's significand. Added to 2^ex it doubles it; subtracted,
        // the difference cancels to zero.
        let power = Dint {
            hi: 1 << 63,
            lo: 0,
            ex: 7,
            sgn: 0,
        };
        let ones = Dint {
            hi: u64::MAX,
            lo: u64::MAX,
            ex: 6,
            sgn: 0,
        };
        let sum = close(power, ones);
        assert_eq!((sum.hi, sum.lo, sum.ex), (1 << 63, 0, 8));
        assert!(close(power, Dint { sgn: 1, ..ones }).is_zero());
    }

    /// No power of two reaches the accurate path: for every `2^k`, `k` from −1074 to 1023 (`ln 1`
    /// returns before), the fast path's two bounds round alike, so `ln` returns from it. The
    /// accurate path's equal-magnitude additions would need one (`Dint::add`).
    #[test]
    fn no_power_of_two_reaches_the_accurate_path() {
        for k in (-1074i64..=1023).filter(|&k| k != 0) {
            let (h, l) = log_fast(k, 1.0).unwrap();
            assert_eq!(
                (h + (l - ERR)).to_bits(),
                (h + (l + ERR)).to_bits(),
                "2^{k}"
            );
        }
    }
}

/// Bounded proofs (chapter 28.7): [`Dint::is_zero`]'s own contract, for every bit pattern.
#[cfg(kani)]
// cargo-mutants reads this attribute from the source and skips the module (its harnesses are
// proven by Kani, and no test build compiles them); no build evaluates it, since the module
// exists only under Kani, where `test` is off.
#[cfg_attr(test, mutants::skip)]
mod proofs {
    use super::*;

    /// `is_zero` holds exactly when both significand words are zero, whatever the exponent and
    /// sign.
    #[kani::proof]
    fn is_zero_reads_both_words() {
        let a = Dint {
            hi: kani::any(),
            lo: kani::any(),
            ex: kani::any(),
            sgn: kani::any(),
        };
        assert_eq!(a.is_zero(), a.hi == 0 && a.lo == 0);
    }
}
