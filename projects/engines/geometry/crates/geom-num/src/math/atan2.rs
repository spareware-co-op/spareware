//! Correctly rounded `atan2(y, x)`: a port of CORE-MATH.
//!
//! Method: Ziv's strategy, as in CORE-MATH; the method and the proof of its bound are below.
//! Source: PORT from CORE-MATH `src/binary64/atan2/atan2.c` and `src/binary64/atan2/tint.h` at
//!   commit `0ce0cbc15935e24c656280e5a46c34e89f301b7a`, by Paul Zimmermann and Alexei Sibidanov,
//!   with the help of Silviu-Ioan Filip, who designed the code that generates its rational
//!   approximation of `atan`, MIT,
//!   <https://gitlab.inria.fr/core-math/core-math/-/blob/0ce0cbc15935e24c656280e5a46c34e89f301b7a/src/binary64/atan2/atan2.c>
//!   Copyright (c) 2024-2025 Paul Zimmermann and Alexei Sibidanov
//!   <https://gitlab.inria.fr/core-math/core-math/-/blob/0ce0cbc15935e24c656280e5a46c34e89f301b7a/src/binary64/atan2/tint.h>
//!   Copyright (c) 2023-2024 Paul Zimmermann and Alexei Sibidanov
//!   Changed for Spare Kit: translated to safe `no_std` Rust, with the 192-bit arithmetic in
//!   `u128` and `u64` steps under the crate's rule (`lib.rs`) and total shifts; round-to-nearest
//!   only, without the exception flags, `fenv` or `errno`; the canonical NaN for a NaN; the double
//!   nearest the 192-bit value where the C stops the program; tiny quotients decided exactly by
//!   integer arithmetic; written proofs of the fast-path and accurate-path bounds. Every change is
//!   listed under "Changes from the C source" below; none changes a correctly rounded result.
//! SPDX-License-Identifier: MPL-2.0 AND MIT
//!
//! The original notices of `atan2.c` and `tint.h`, reproduced as the MIT licence requires. The two
//! differ only in their copyright lines:
//!
//! ```text
//! atan2.c:
//! Copyright (c) 2024-2025 Paul Zimmermann and Alexei Sibidanov
//!
//! tint.h:
//! Copyright (c) 2023-2024 Paul Zimmermann and Alexei Sibidanov
//!
//! (both files:)
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
//! ```
//!
//! **Error bound: correctly rounded**, with one qualification below. The result is the binary64
//! number nearest the exact angle of the point `(x, y)` from the positive x-axis, in `[−π, π]`
//! (ties to even), so the error is at most half an ulp. Domain: every pair of doubles, with C99
//! Annex F's special values (`atan2(±0, +0) = ±0`, `atan2(±0, −0) = ±π`, `atan2(±∞, ±∞) =
//! ±π/4` or `±3π/4`, and so on); a NaN in either argument gives the canonical NaN
//! (`0x7ff8_0000_0000_0000`). The proof is at the end of this documentation.
//!
//! The qualification: atan2 has two arguments, so its hard-to-round cases cannot be found by
//! exhaustive search as those of the one-argument functions can. The accurate path's value has
//! relative error below 2^−182.63 (proof, step A), so its rounding can only be wrong if the exact
//! angle lies within that error of a midpoint between two doubles. No such input has ever been
//! found; CORE-MATH's code checks for one and stops the program, while this port returns the
//! double nearest the 192-bit value, whose error is then still below `½ + 2^−128` ulp, a bound
//! proven for every input. Either way the result is within one ulp, so an enclosure widened by one
//! ulp on each side (`next_down`, `next_up`) is certified.
//!
//! Method (Ziv's strategy, as CORE-MATH). With `x ≥ y > 0` after taking absolute values and
//! swapping, `atan(y/x) = atan(t₀) + atan((y − t₀x)/(x + t₀y))` for the nearest `t₀ = j/64`, with
//! `atan(t₀)` from a double-double table and the second term from an odd polynomial; the octant
//! adds `0`, `±π/2` or `±π`. A first fast path evaluates the reduced quotient in double
//! precision, a second in double-double. If neither decides, the accurate path computes `y/x`
//! (or `x/y`) and a degree-[29, 29] rational approximation of `atan` in 192-bit floating point
//! ([`Tint`]), with an error budget written out in the C source, and rounds it.
//!
//! Changes from the C source, none of which changes a correctly rounded result:
//!
//! - **Proven fast-path bounds.** CORE-MATH's first fast path uses the bound
//!   `|z|·0x1.051p−51 + 2^−90`, which its source states was measured on 1.1·10^10 random inputs
//!   and enlarged by 2.5%. A measured bound is not a proof; the proof below shows that this bound,
//!   and the second fast path's, are sufficient, so both are kept unchanged.
//! - Only round-to-nearest is implemented (Rust has no other rounding mode, RFC 3514); the
//!   floating-point exception flags, the `fenv` save and restore, and `errno` are not kept.
//! - The accurate path's final conversion never stops the program: where CORE-MATH prints
//!   "Unexpected worst-case found" and exits on a result it cannot round, this port returns the
//!   double nearest the 192-bit value (see the qualification above). No panic, no abort.
//! - NaN returns the canonical NaN instead of `y + x` (chapter 21).
//! - `fma` is `libm::fma` ([`super::support::fma`]), correctly rounded like `__builtin_fma`.
//! - **Every ingredient of the accurate path's bound re-derived** (proof, step A). CORE-MATH's
//!   atan2.sage bounds the Horner evaluations only for quotients `2^−12 ≤ z ≤ 1`, but the path
//!   also sees smaller ones; the bounds are recomputed here for every `0 < z ≤ 1 + 2^−185`, with
//!   exact arithmetic and no first-order approximation, and confirm CORE-MATH's figures. The
//!   rational approximation's error is bounded rigorously too, and the total is recomputed. No
//!   constant changes.
//! - The accurate path and the special cases are kept out of line (`#[cold]`, `#[inline(never)]`),
//!   so that the fast paths' code stays small; the accurate path's Horner loops iterate over the
//!   coefficient tables instead of indexing them.
//! - **An exact rule for tiny quotients.** For `x > 0` and `|y/x| < 2^−95` CORE-MATH rounds its
//!   192-bit quotient after subtracting 2 units in its last place (with an inverted borrow test).
//!   That is right unless `y/x` is exactly a midpoint between two subnormal numbers, which
//!   happens (CORE-MATH's `atan2.wc` lists such pairs), and the division's own error bound is
//!   larger than 2 units, so the nudge is not proven to suffice there. [`tiny_quotient`] decides
//!   the case exactly with integer arithmetic instead, with a proof.
//! - The 192-bit arithmetic uses Rust's `u128` and `u64` with total shifts ([`super::support`]),
//!   the halves of a `u128` read from its bytes instead of casts, explicitly wrapping operations
//!   where the C code computes modulo 2^64 or 2^128 on purpose (a borrow kept by comparison, as
//!   the C does), and checked operations everywhere else: every such step cannot overflow (each
//!   says why), so where the C would wrap had its proof been wrong, this port ends the
//!   computation with the canonical NaN instead, which no input reaches.
//!
//! # Proof of the error bound
//!
//! The lemmas cited are those of [`super`]. Numbers marked *checked* are computed with exact
//! arithmetic (exhaustively over the tables) by the unit test named in parentheses. `u = 2^−53`.
//!
//! **F. The fast paths.** (F1) For every table index `j` and octant, `fh = ±F2[j].high +
//! OCTANT.high` is exact (the table's high parts end in enough zero bits); `|F2[j] − atan(j/64)| ≤
//! δ_T = 2^−105` (2^−106.06 measured), the octant offsets are within `δ_O = 2^−107` of 0, ±π/2 and
//! ±π (2^−108.04), and `T2[j] = j/64` (*checked*: `tables_and_octants`). (F2) Reduction: with
//! `q = y/x ≤ 1` (after swapping), `j = round(64·q)` is read from `RN(RN(q) + 2 + 2^−7)`, so
//! `|q − t₀| ≤ 2^−7 + 2^−50` and `z* = (y − t₀·x)/(x + t₀·y)` has `|z*| ≤ z_max = 2^−7 + 2^−50`;
//! `atan(y/x) = atan(t₀) + atan(z*)`. The scalings by `2^920` and `½` are exact (`y ≥ 2^−53·x`, so
//! nothing becomes subnormal), and no quantity below underflows. (F3) First fast path: `zn` and
//! `zd` are single roundings (`fma`) and `z = RN(zn/zd) = z*·(1 + θ)`, `|θ| ≤ 3u + 4u²`; the
//! polynomial `z³·B(z²)` approximates `atan z − z` within 2^−46.96·|z|³ (subdivision of
//! `[0, z_max²]`); its evaluation and the shift from `z*` to `z` add at most `4.4u·|z|³`; the fast
//! two-sum of `fh` and `z` is exact (`fh = 0` or `|fh| ≥ atan(1/64) > z_max`); the two low-part
//! additions add `u·|fl + dz| + u·|rl|`, and the table and offsets `δ_T + δ_O + u·|fl|`. With Lemma
//! 1's `u·(|rl| + eps)`, the requirement against `eps = |z|·EPS1_REL + EPS1_ABS` reads
//! 2^−101.27 ≤ 2^−90 at `z = 0` and 2^−58.41 ≤ 2^−57.97 at `|z| = z_max`; the left side is convex
//! in `|z|` and the right linear (*checked*: `fast_path_1_bound`). (F4) Second fast path: `x + t₀·y`
//! is `dh' + dl'` within `2.01u²` relative (the fast two-sum with `x ≥ y·t₀` is exact, the low
//! parts are added once); `y − t₀·x = nh₂ + nl₂` exactly (`dt = y − nh` is exact by Sterbenz, since
//! `t₀·x` and `y` are within a factor 2, and the fast two-sum by Lemma 3(i)); one correction step
//! `zl = rdh·((nh₂ − dh'·zh) + (nl₂ − zh·dl'))`, with its residual by `fma`, gives `zh + zl` within
//! `24u²·|z*|` of `z*`; the degree-9 polynomial approximates `atan z − z` within 2^−55.59·|z|³,
//! and its evaluation and the shift from `z*` to `zh` add `3.4u·|z|³`. The requirement against
//! `eps₂ = EPS2_REL·(|zh|·zh² + EPS2_ABS)` reads 2^−101.27 ≤ 2^−100.68 at `z = 0` and
//! 2^−71.53 ≤ 2^−70.68 at `|z| = z_max` (*checked*: `fast_path_2_bound`). So CORE-MATH's measured
//! bounds are proven sufficient, unchanged.
//!
//! **A. The accurate path.** The exact quotient shortcut is right because for
//! `|t| ≤ 0x1.d12ed0af1a27fp−27`, `|atan t − t| < |t|³/3` is below half the gap below `t`, so
//! `atan t` rounds to `t`; an underflowed quotient gives ±0, or ±π for `x < 0`. Tiny quotients
//! (`|y/x| < 2^−95`, `x > 0`) are decided exactly by [`tiny_quotient`], whose proof is in its
//! documentation: if a midpoint `M ≠ q`, then `y − M·x` is a non-zero multiple of the smaller of
//! the last-place weights of `y` and `M·x`, which with `|q| < 2^−54` gives `|q − M| > |q|³/3` (for
//! a subnormal midpoint `M = (2j + 1)·2^−1075` that needs only `|q| < 2^−375`; for a normal one,
//! whose significand is odd with 54 bits, `|q| < 2^−53.2`). Everywhere else the error is assembled
//! as in atan2.c. The [`Tint`] operations follow tint.h's written analyses: the 192-bit division
//! has relative error below `ε₁ = ε₄ = 2^−185.53` (`inv_tint` and `div_tint`), each multiplication
//! below 10 units of its result's last place and each addition below 2; so the computed quotient
//! is `z = (y/x)(1 + ε₁)` with `0 < z ≤ 1 + 2^−185`. tint.h's `inv_tint` analysis assumes the
//! double it starts Newton's step from is the divisor itself; that holds for `y/x` and `x/y`, but
//! the divisor `q ≈ Q(z)` has 192 bits and is truncated to 53 first, so there the reciprocal is
//! within 2^−101.83, not 2^−103.9; re-assembled with that, tint.h's figures still give 2^−185.54
//! for the division, below `ε₄` [*checked*: `division_error_bound`]. (A1) The rational
//! approximation `z·P(z)/Q(z)` of `atan z` has relative error `ε₀` below 2^−193 on that whole
//! range: on 256 pieces of `[0, 1]`,
//! refined where needed, a Taylor expansion of `atan` (of `atan(z)/z` near 0) with a bounded
//! remainder, and the polynomials' shifted coefficients, bound it with exact arithmetic
//! (CORE-MATH's rminimax computation gives the sharper 3.99613·10^−59 = 2^−194.34) [*checked*:
//! `rational_approximation_error`]. (A2) Horner's rule in [`Tint`] arithmetic computes `z·P(z)`
//! within `ε₂ = 2^−184.14` and `Q(z)` within `ε₃ = 2^−184.19` (relative) for every such `z`: a
//! running error bound on every piece of every binade down to `[0, 2^−60]`, refined until it meets
//! those figures. CORE-MATH's `errPsplit` and `errQsplit` (atan2.sage) obtain them only for
//! `z ≥ 2^−12` and to first order; the accurate path also sees smaller quotients (an exponent
//! difference of 53 or more, or an undecided fast path), and these bounds cover them
//! [*checked*: `accurate_polynomial_evaluation`]. (A3) With `atan z = atan(y/x)(1 + ε₅)`,
//! `ε₅ ≤ ε₁·4/π`, the result `atan(y/x)·(1 + ε₀)(1 + ε₂)(1 + ε₄)(1 + ε₅)/(1 + ε₃)` has relative
//! error below 2^−182.636, so within 662 units of its last place; adding `π/2` (within 2^−197.96)
//! gives at most 524 units of 2^−192, and adding `π` (within 2^−196.96) at most 266 units of
//! 2^−191 [*checked*: `accurate_path_total`]. [`Tint::to_f64`] rounds the 192-bit value; it can
//! only round wrongly if the exact value is within that error of a midpoint, which no input has
//! ever been found to do, and then the error is below `½ + 2^−128` ulp. All 111 772 inputs of
//! CORE-MATH's atan2.wc (in all four sign combinations) are correctly rounded. ∎

use core::cmp::Ordering;

use super::hexf::hf;
use super::support::{
    entry, fast_two_sum, fma, halves, hi64, join, lo64, pow2, shl, shl128, shr, shr128, signed,
};

/// The 52 significand bits of a binary64 number.
const MANTISSA: u64 = (1 << 52) - 1;

/// π ≈ `PI_H + PI_L`, error below 2^−108.041.
const PI_H: f64 = hf!("0x1.921fb54442d18p+1");
/// See [`PI_H`].
const PI_L: f64 = hf!("0x1.1a62633145c07p-53");
/// π/2 ≈ `PI_OVER2_H + PI_OVER2_L`, error below 2^−109.041.
const PI_OVER2_H: f64 = hf!("0x1.921fb54442d18p+0");
/// See [`PI_OVER2_H`].
const PI_OVER2_L: f64 = hf!("0x1.1a62633145c07p-54");

/// First fast path: relative part of the error bound (proof, step F3).
const EPS1_REL: f64 = hf!("0x1.051p-51");
/// First fast path: absolute part of the error bound.
const EPS1_ABS: f64 = hf!("0x1p-90");
/// Second fast path: the bound is `EPS2_REL·(|zh|·zh² + EPS2_ABS)`.
const EPS2_REL: f64 = hf!("0x1.4p-50");
/// See [`EPS2_REL`].
const EPS2_ABS: f64 = hf!("0x1p-51");

/// The table's largest error, `|F2[j] − atan(j/64)| ≤ DELTA_T` (proof, step F1; measured).
#[cfg(test)]
const DELTA_T: f64 = hf!("0x1p-105");
/// The octant offsets' largest error against 0, ±π/2 and ±π (proof, step F1).
#[cfg(test)]
const DELTA_O: f64 = hf!("0x1p-107");

/// Double-double sum (C: `fastsum`), the high parts by fast two-sum.
#[inline]
fn fast_sum(xh: f64, xl: f64, yh: f64, yl: f64) -> (f64, f64) {
    let (sh, sl) = fast_two_sum(xh, yh);
    (sh, (xl + yl) + sl)
}

/// The result for a zero, infinite or NaN argument (C: `as_atan2_special`).
#[cold]
#[inline(never)]
fn special(y0: f64, x0: f64) -> f64 {
    const FINF: [[f64; 2]; 2] = [
        [hf!("0x1p-55"), hf!("0x1.921fb54442d18p-1")],
        [hf!("0x1p-54"), hf!("0x1.2d97c7f3321d2p+1")],
    ];
    let (iy, ix) = (y0.to_bits(), x0.to_bits());
    let (aiy, aix) = (iy << 1, ix << 1);
    let inf = 0x7ffu64 << 53;
    if aiy >= inf || aix >= inf {
        if aiy > inf || aix > inf {
            return super::NAN;
        }
        // Neither is NaN; at least one is ±∞.
        if aiy == inf && aix == inf {
            // atan2(±∞, +∞) = ±π/4, atan2(±∞, −∞) = ±3π/4
            let [lo, hi] = entry(&FINF, ix >> 63);
            return hi.copysign(y0) + lo.copysign(y0);
        }
        if aix == inf {
            if x0 < 0.0 {
                return PI_H.copysign(y0) + PI_L.copysign(y0);
            }
            return 0.0f64.copysign(y0); // atan2(±y, +∞) = ±0 for finite y
        }
        // y = ±∞, x finite: ±π/2
        return PI_OVER2_H.copysign(y0) + PI_OVER2_L.copysign(y0);
    }
    if aiy == 0 && aix == 0 {
        if ix == 0 {
            return y0; // atan2(±0, +0) = ±0
        }
        return if iy == 0 { PI_H + PI_L } else { -PI_H - PI_L }; // atan2(±0, −0) = ±π
    }
    if aiy == 0 {
        if x0 > 0.0 {
            return y0 / x0; // ±0
        }
        return if iy >> 63 == 0 {
            PI_H + PI_L
        } else {
            -PI_H - PI_L
        };
    }
    // Only x is zero: ±π/2 with the sign of y.
    if y0 > 0.0 {
        PI_OVER2_H + PI_OVER2_L
    } else {
        -PI_OVER2_H - PI_OVER2_L
    }
}

/// The angle of the point (x, y) from the positive x-axis, in radians, in [−π, π]: correctly
/// rounded.
///
/// Error at most half an ulp for every input pair on which the accurate path decides, which is
/// every pair ever tested; proven below `½ + 2^−128` ulp in every case. See the module
/// documentation for the method, the source and the proof. Special values follow C99 Annex F; a
/// NaN argument gives the canonical NaN, `0x7ff8_0000_0000_0000`.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the C source's variables, so the two can be read side by side"
)]
pub fn atan2(y0: f64, x0: f64) -> f64 {
    const MASK: u64 = u64::MAX >> 1;
    /// First fast path: `z³·(B[0] + B[1]·z² + B[2]·z⁴)` approximates `atan z − z` (proof, F3).
    const B: [f64; 3] = [
        hf!("-0x1.55555555554d2p-2"),
        hf!("0x1.999999860e1cap-3"),
        hf!("-0x1.248ad469844a1p-3"),
    ];
    /// Second fast path: the degree-9 polynomial of `atan z − z` (proof, F4).
    const B2: [f64; 4] = [
        hf!("-0x1.5555555555555p-2"),
        hf!("0x1.999999999755ep-3"),
        hf!("-0x1.24924883596f8p-3"),
        hf!("0x1.c6f7d73531bc2p-4"),
    ];
    let (iy, ix) = (y0.to_bits(), x0.to_bits());
    let aiy = iy & MASK;
    if aiy == 0 || aiy >= 0x7ff << 52 {
        return special(y0, x0);
    }
    let aix = ix & MASK;
    if aix == 0 || aix >= 0x7ff << 52 {
        return special(y0, x0);
    }
    let (ax, ay) = (x0.abs(), y0.abs());
    let (mut x, mut y) = if ax >= ay { (ax, ay) } else { (ay, ax) };
    let (sy, sx) = (iy >> 63, ix >> 63);
    let gt = u64::from(aix < aiy);
    // Roughly the difference of the exponents, in units of 2^52. C: unsigned arithmetic modulo
    // 2^64 on purpose: for aix < aiy the difference wraps, and the exclusive-or with all ones
    // (−gt modulo 2^64) turns it into aiy − aix − 1, its magnitude.
    let dxy = aix.wrapping_sub(aiy) ^ gt.wrapping_neg();
    if dxy >= 53 << 52 {
        return atan2_accurate(y0, x0);
    }
    let sgn = if (gt ^ sx ^ sy) != 0 { -1.0 } else { 1.0 };
    let kw = (sx << 2) | (sy << 1) | gt; // 0 ≤ kw < 8: the octant
    // jt = round(64·y/x): 0 ≤ jt ≤ 64, read from the significand of y/x + 2 + 1/128.
    let jj = (y / x + (2.0 + 1.0 / 128.0)).to_bits();
    let jt = (jj >> (52 - 7)) & 127;
    let [f2l, f2h] = entry(&F2, jt);
    let [oh, ol] = entry(&OCTANT, kw);
    let fh = f2h * sgn + oh;
    let fl = f2l * sgn + ol;
    if x < hf!("0x1p-920") {
        x *= hf!("0x1p920");
        y *= hf!("0x1p920");
    }
    if x > hf!("0x1p1022") && jt != 0 {
        x *= 0.5;
        y *= 0.5;
    }
    let t0 = entry(&T2, jt);
    let zn = fma(-t0, x, y);
    let zd = fma(t0, y, x);
    let z = zn / zd;
    let z2 = z * z;
    let z = z * sgn;
    let dz = (z * z2) * (B[0] + z2 * (B[1] + z2 * B[2]));
    let eps = z.abs() * EPS1_REL + EPS1_ABS;
    let (rh, z) = fast_two_sum(fh, z);
    let rl = (fl + dz) + z;
    let lb = rh + (rl - eps);
    let ub = rh + (rl + eps);
    if lb == ub {
        return ub;
    }

    // Second fast path: the reduced quotient in double-double.
    let dh = y * t0;
    let dl = fma(y, t0, -dh);
    let (dh, e) = fast_two_sum(x, dh);
    let rdh = 1.0 / dh;
    let dl = dl + e;
    let nh = x * t0;
    let nl = fma(x, t0, -nh);
    let dt = y - nh;
    let y1 = dt + nh;
    let (nh, nl) = if y1 == y {
        fast_two_sum(dt, -nl)
    } else {
        fast_two_sum(dt, (y - y1) - nl)
    };
    let zh = nh * rdh;
    let z2 = zh * zh;
    let mut zl = rdh * (fma(dh, -zh, nh) + (nl - (nh * rdh) * dl));
    zl += zh * z2 * ((B2[0] + z2 * B2[1]) + (z2 * z2) * (B2[2] + z2 * B2[3]));
    let zh = zh * sgn;
    let zl = zl * sgn;
    let eps = EPS2_REL * (zh.abs() * z2 + EPS2_ABS);
    let (fh, fl) = fast_sum(fh, fl, zh, zl);
    let lb = fh + (fl - eps);
    let ub = fh + (fl + eps);
    if lb != ub {
        return atan2_accurate(y0, x0);
    }
    ub
}

/// The accurate path (C: `atan2_accurate`): `y/x` or `x/y` and a degree-[29, 29] rational
/// approximation of `atan` on [0, 1] in 192-bit floating point, with a proven error bound, then
/// the octant.
#[cold]
#[inline(never)]
fn atan2_accurate(y: f64, x: f64) -> f64 {
    // The 192-bit steps cannot overflow (each says why); were one to, the result is the canonical
    // NaN, never a plausible wrong value.
    accurate(y, x).unwrap_or(super::NAN)
}

/// [`atan2_accurate`], `None` only if a 192-bit step overflowed, which none does.
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the C source's variables, so the two can be read side by side"
)]
fn accurate(y: f64, x: f64) -> Option<f64> {
    // atan(t) rounds to t when |t| ≤ 0x1.d12ed0af1a27fp−27, so first catch an exact quotient.
    let t = y / x;
    if t == 0.0 {
        // The quotient underflowed: atan2 rounds to t (±0) for x > 0, and to ±π for x < 0.
        if x > 0.0 {
            return Some(t);
        }
        return Some(if y > 0.0 { PI_H + PI_L } else { -PI_H - PI_L });
    }
    let corr = fma(t, x, -y);
    if corr == 0.0 && x > 0.0 && t.abs() <= hf!("0x1.d12ed0af1a27fp-27") {
        // t = y/x exactly, and atan(t) is slightly smaller in magnitude: t − t·2^−54 rounds to t.
        if y.abs() >= hf!("0x1p-915") {
            return Some(fma(t, hf!("-0x1p-54"), t));
        }
        // y is tiny: check exactness again with both sides scaled, where t·x − y cannot
        // underflow.
        let corr = fma(t * hf!("0x1p105"), x, -y * hf!("0x1p105"));
        if corr == 0.0 {
            return Some(fma(t, hf!("-0x1p-54"), t));
        }
    }

    let inv = y.abs() > x.abs();
    let mut z = if inv {
        Tint::div_f64(x, y)?
    } else {
        Tint::div_f64(y, x)?
    };

    // When |y/x| < 2^−95 and x > 0, atan(y/x) rounds as y/x does, except when y/x is exactly a
    // midpoint between two subnormal numbers (see `tiny_quotient`).
    if !inv && x > 0.0 && z.ex <= -96 {
        return Some(tiny_quotient(y, x));
    }

    // |z − y/x| < 2^−185.53·|z| (relative error of the division). The rational approximation is
    // for z > 0 only.
    let sz = z.sgn;
    z.sgn = 0;
    // Horner's rule from the leading coefficients down, by iteration: no index to go wrong. The
    // tables are fixed arrays of 30, so `split_last` always succeeds.
    let (Some((p_top, p_rest)), Some((q_top, q_rest))) = (P.split_last(), Q.split_last()) else {
        return None;
    };
    let mut p = *p_top;
    let mut q = *q_top;
    for (pc, qc) in p_rest.iter().zip(q_rest.iter()).rev() {
        p = p.mul(&z)?.add(pc)?;
        q = q.mul(&z)?.add(qc)?;
    }
    let p = p.mul(&z)?;
    let mut z = p.div(&q)?;
    // z·P(z)/Q(z) = atan(z)·(1 + ε0) with |ε0| < 2^−193; with the errors of the division
    // (2^−185.53), of p (2^−184.14) and of q (2^−184.19), the result has relative error below
    // 2^−182.63, that is at most 662 ulps of its 192-bit significand (proof, step A).
    let mut err = 662;
    z.sgn = sz;
    // Now z ≈ atan(y/x) if !inv, atan(x/y) if inv, with −π/4 < z < π/4.
    if inv {
        if z.sgn == 0 {
            // 0 < atan(x/y) < π/4: π/2 − z
            z.sgn = 1;
            z = PI2.add(&z)?;
        } else {
            // −π/4 < atan(x/y) < 0: −π/2 − z
            z = PI2.add(&z)?;
            z.sgn = 1;
        }
        // error below 2^−182.967, that is 524 ulps relative to π/4
        err = 524;
    }
    // Now −π/2 < z < π/2; for x < 0 move to the opposite half-plane.
    if x < 0.0 {
        if z.sgn == 0 {
            // first quadrant to third: z − π
            z.sgn = 1;
            z = PI.add(&z)?;
            z.sgn = 1;
        } else {
            // fourth quadrant to second: z + π
            z = PI.add(&z)?;
        }
        // error below 2^−182.945, that is 266 ulps relative to π/2
        err = 266;
    }
    z.to_f64(err)
}

/// `atan(y/x)` correctly rounded for `x > 0` and `0 < |y/x| < 2^−54`, exactly.
///
/// CORE-MATH rounds its 192-bit quotient after subtracting 2 units in its last place; this port
/// decides the case exactly instead (see the module documentation). Write `q = y/x`. Then
/// `atan q` lies strictly between `q − q³/3` and `q`, on the side of `q` towards zero. A rounding
/// boundary (a midpoint between consecutive doubles) is either `q` itself or farther from `q`
/// than `q³/3`: if `M ≠ q`, then `y − M·x` is a non-zero multiple of the smaller of the last-place
/// weights of `y` and `M·x`, which with `|q| < 2^−54` gives `|q − M| > |q|³/3` (see the module's
/// proof, step A). So `atan q` rounds as `q` does, which is the IEEE quotient
/// `y / x`, unless `q` is a midpoint, when it rounds to the neighbour of `q` towards zero. A
/// quotient of two doubles is never a midpoint in the normal range (`M·x` would need 54
/// significant bits), so only odd multiples of 2^−1075 are checked, with integer arithmetic.
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the C source's variables, so the two can be read side by side"
)]
fn tiny_quotient(y: f64, x: f64) -> f64 {
    let t = y / x; // the correctly rounded quotient
    if t.abs() > f64::MIN_POSITIVE {
        return t;
    }
    // y = my·2^ey and x = mx·2^ex with my and mx odd. The exponent field is below 2^11, so
    // `signed` reads its value; the exponents stay within ±1200, so no checked step fails.
    let odd_parts = |v: f64| -> Option<(u64, i64)> {
        let bits = v.to_bits() & (u64::MAX >> 1);
        let biased = signed(bits >> 52);
        let (m, e) = if biased == 0 {
            (bits, -1074)
        } else {
            ((bits & MANTISSA) | (1u64 << 52), biased.checked_sub(1075)?)
        };
        let tz = m.trailing_zeros(); // m ≠ 0: v is not zero here
        Some((shr(m, u64::from(tz)), e.checked_add(i64::from(tz))?))
    };
    let (Some((my, ey)), Some((mx, ex))) = (odd_parts(y), odd_parts(x)) else {
        return super::NAN;
    };
    let Some(d) = ey.checked_sub(ex) else {
        return super::NAN;
    };
    // q = (my/mx)·2^(ey−ex) is an odd multiple of 2^−1075 exactly when mx divides my and
    // ey − ex = −1075; then q = k·2^−1075 with k = my/mx odd and k < 2^53. (`checked_rem` is
    // `None` only for mx = 0, which the C also tests.)
    if my.checked_rem(mx) != Some(0) || d != -1075 {
        return t;
    }
    let Some(k) = my.checked_div(mx) else {
        return super::NAN; // never: mx ≠ 0 here
    };
    if k >= 1 << 53 {
        return t; // not reachable: q would be a midpoint of the normal range
    }
    // C: `(k − 1)/2`, which is ⌊k/2⌋ for the odd k: (k − 1)/2 · 2^−1074, the neighbour of q
    // towards zero.
    f64::from_bits(k / 2).copysign(y)
}

/// A 192-bit floating-point number `(−1)^sgn · (h/2^64 + m/2^128 + l/2^192) · 2^ex`
/// (C: `tint_t`). Normalised numbers have the top bit of `h` set; zero has `h = m = l = 0`.
#[derive(Clone, Copy, Default)]
struct Tint {
    /// High 64 bits of the significand.
    h: u64,
    /// Middle 64 bits.
    m: u64,
    /// Low 64 bits.
    l: u64,
    /// Exponent.
    ex: i64,
    /// Sign: 0 for positive, 1 for negative.
    sgn: u64,
}

/// Shorthand for the tables.
const fn t(h: u64, m: u64, l: u64, ex: i64, sgn: u64) -> Tint {
    Tint { h, m, l, ex, sgn }
}

/// Zero.
const ZERO: Tint = t(0, 0, 0, -1076, 0);
/// One.
const ONE: Tint = t(0x8000_0000_0000_0000, 0, 0, 1, 0);
/// π, error below 2^−196.96.
const PI: Tint = t(
    0xc90f_daa2_2168_c234,
    0xc4c6_628b_80dc_1cd1,
    0x2902_4e08_8a67_cc74,
    2,
    0,
);
/// π/2, error below 2^−197.96.
const PI2: Tint = t(
    0xc90f_daa2_2168_c234,
    0xc4c6_628b_80dc_1cd1,
    0x2902_4e08_8a67_cc74,
    1,
    0,
);

/// A 192-bit significand as (high 128 bits, low 64 bits) (C: the `_h`, `_l` view of `tint_t`).
#[derive(Clone, Copy)]
struct Wide192 {
    /// `h·2^64 + m`.
    hi: u128,
    /// `l`.
    lo: u64,
}

impl Wide192 {
    /// `self / 2^k`, truncated (C: `rshift`). Each native shift is by less than its width, as its
    /// arm's range says; the differences `64 − k` and `k − 64` are exact (`abs_diff`, with `k` on
    /// the right side of 64 in each arm).
    fn shr(self, k: u64) -> Wide192 {
        match k {
            0 => self,
            1..=63 => Wide192 {
                hi: self.hi >> k,
                // C: `(uint64_t)(h << (64 − k))`, the low 64 bits of the bits shifted across.
                lo: lo64(shl128(self.hi, 64u64.abs_diff(k))) | (self.lo >> k),
            },
            64..=191 => Wide192 {
                hi: shr128(self.hi, k),
                lo: lo64(shr128(self.hi, k.abs_diff(64))),
            },
            _ => Wide192 { hi: 0, lo: 0 },
        }
    }

    /// `self · 2^k`, truncated to 192 bits (C: `lshift`), with the shifts and differences of
    /// [`Wide192::shr`].
    fn shl(self, k: u64) -> Wide192 {
        match k {
            0 => self,
            1..=63 => Wide192 {
                hi: (self.hi << k) | u128::from(self.lo >> 64u64.abs_diff(k)),
                lo: self.lo << k,
            },
            64..=191 => Wide192 {
                hi: shl128(self.hi, k) | shl128(u128::from(self.lo), k.abs_diff(64)),
                lo: 0,
            },
            _ => Wide192 { hi: 0, lo: 0 },
        }
    }

    /// `self − b`, for `self ≥ b`: C's unsigned subtraction of 192-bit numbers, limb by limb,
    /// each limb modulo its width with the borrow kept by comparison, as the C does (and as
    /// `crate::wide` keeps its limbs' borrows). For `self ≥ b` the result is exact.
    fn sub(self, b: Wide192) -> Wide192 {
        let lo = self.lo.wrapping_sub(b.lo);
        let borrow = u128::from(lo > self.lo);
        Wide192 {
            hi: self.hi.wrapping_sub(b.hi).wrapping_sub(borrow),
            lo,
        }
    }

    /// The number of leading zero bits, for a non-zero value: at most 192, so the checked sum
    /// cannot fail.
    fn leading_zeros(self) -> Option<u64> {
        if self.hi != 0 {
            Some(u64::from(self.hi.leading_zeros()))
        } else {
            u64::from(self.lo.leading_zeros()).checked_add(128)
        }
    }
}

impl Tint {
    /// The 192-bit significand.
    fn wide(&self) -> Wide192 {
        Wide192 {
            hi: join(self.h, self.m),
            lo: self.l,
        }
    }

    /// Replaces the significand.
    fn set_wide(&mut self, w: Wide192) {
        (self.m, self.h) = halves(w.hi);
        self.l = w.lo;
    }

    /// Whether the value is zero.
    fn is_zero(&self) -> bool {
        self.h == 0
    }

    /// Compares magnitudes (C: `cmp_tint_abs`).
    fn cmp_magnitude(&self, b: &Tint) -> Ordering {
        if self.is_zero() {
            return if b.is_zero() {
                Ordering::Equal
            } else {
                Ordering::Less
            };
        }
        if b.is_zero() {
            return Ordering::Greater;
        }
        self.ex
            .cmp(&b.ex)
            .then((self.h, self.m, self.l).cmp(&(b.h, b.m, b.l)))
    }

    /// `self · b` with error below 10 ulps, `2^−187.67·|result|` (C: `mul_tint`). `None` only if
    /// a step overflowed, which none does (each says why).
    #[allow(
        clippy::many_single_char_names,
        reason = "the names of the C source's variables, so the two can be read side by side"
    )]
    fn mul(&self, b: &Tint) -> Option<Tint> {
        let (ah, am, al) = (u128::from(self.h), u128::from(self.m), u128::from(self.l));
        let (bh, bm, bl) = (u128::from(b.h), u128::from(b.m), u128::from(b.l));
        // Each 64 × 64-bit product is below (2^64 − 1)² < 2^128: no checked product fails.
        let rh = ah.checked_mul(bh)?;
        let rm1 = ah.checked_mul(bm)?;
        let rm2 = am.checked_mul(bh)?;
        let rl1 = ah.checked_mul(bl)?;
        let rl2 = am.checked_mul(bm)?;
        let rl3 = al.checked_mul(bh)?;
        let (mut m, mut h) = halves(rh);
        // Accumulate rm1. The limb sums are C's unsigned arithmetic modulo 2^64 with the carry
        // kept by comparison, as the C does; the carry into h cannot overflow it (h is the high
        // part of the product of two numbers below 2^192, so it stays below 2^64).
        let (mut l, c) = halves(rm1);
        m = m.wrapping_add(c);
        h = h.checked_add(u64::from(m < c))?;
        // accumulate rm2
        let (lo2, c) = halves(rm2);
        l = l.wrapping_add(lo2);
        let mut cm = u64::from(l < lo2); // carry into m
        m = m.wrapping_add(c);
        h = h.checked_add(u64::from(m < c))?;
        // Accumulate the high halves of rl1 + rl2 + rl3: each below 2^64, so their sum is below
        // 3·2^64 and the carry count cm at most 4.
        let s = u128::from(hi64(rl1))
            .checked_add(u128::from(hi64(rl2)))?
            .checked_add(u128::from(hi64(rl3)))?;
        let (lo3, s_hi) = halves(s);
        cm = cm.checked_add(s_hi)?;
        l = l.wrapping_add(lo3);
        cm = cm.checked_add(u64::from(l < lo3))?;
        m = m.wrapping_add(cm);
        h = h.checked_add(u64::from(m < cm))?;
        let mut r = Tint {
            h,
            m,
            l,
            ex: self.ex.checked_add(b.ex)?,
            sgn: self.sgn ^ b.sgn,
        };
        // A zero operand leaves r = 0, which the normalisation keeps zero.
        if r.h >> 63 == 0 {
            r.h = (r.h << 1) | (r.m >> 63);
            r.m = (r.m << 1) | (r.l >> 63);
            r.l <<= 1;
            r.ex = r.ex.checked_sub(1)?;
        }
        Some(r)
    }

    /// `self + b` with error below 2 ulps of the result (C: `add_tint`). `None` only if an
    /// exponent or a carry overflowed, which none does.
    fn add(&self, b: &Tint) -> Option<Tint> {
        let (a, b) = match self.cmp_magnitude(b) {
            Ordering::Equal => {
                if self.sgn ^ b.sgn != 0 {
                    return Some(ZERO);
                }
                return Some(Tint {
                    ex: self.ex.checked_add(1)?,
                    ..*self
                });
            }
            Ordering::Less => (b, self),
            Ordering::Greater => (self, b),
        };
        // Now |a| > |b|, so a.ex ≥ b.ex: the distance is a.ex − b.ex, capped at 192 as the C's
        // shift does (a larger one shifts every bit out either way).
        let sh = a.ex.abs_diff(b.ex).min(192);
        let tb = b.wide().shr(sh);
        let mut r = Tint { sgn: a.sgn, ..ZERO };
        if a.sgn ^ b.sgn != 0 {
            // Subtraction; the difference is non-zero since |a| > |b|.
            let d = a.wide().sub(tb);
            let ex = d.leading_zeros()?;
            if ex <= 1 || sh == 0 {
                // The neglected low part of b contributes less than 2 ulps after the shift; for
                // sh = 0 the subtraction is exact.
                r.set_wide(d.shl(ex));
                r.ex = a.ex.checked_sub(i64::try_from(ex).ok()?)?;
            } else {
                // ex ≥ 2 needs sh = 1: shift both left by ex first, so that no bit of b is lost
                // (C: `ex − sh`, at least 1 here).
                let tb = b.wide().shl(ex.checked_sub(sh)?);
                let ra = a.wide().shl(ex);
                let d = ra.sub(tb);
                let ex1 = d.leading_zeros()?;
                r.set_wide(d.shl(ex1));
                r.ex =
                    a.ex.checked_sub(i64::try_from(ex.checked_add(ex1)?).ok()?)?;
            }
        } else {
            // Addition, error below 1 ulp: C's unsigned limb sums modulo 2^64 and 2^128 with the
            // carries kept by comparison; the carry count ch is at most 1.
            let aw = a.wide();
            let lo = aw.lo.wrapping_add(tb.lo);
            let cl = u128::from(lo < aw.lo);
            let mut hi = aw.hi.wrapping_add(tb.hi);
            let mut ch = u128::from(hi < aw.hi);
            hi = hi.wrapping_add(cl);
            ch = ch.checked_add(u128::from(hi < cl))?;
            if ch != 0 {
                r.ex = a.ex.checked_add(1)?;
                let lo = (lo64(hi) << 63) | (lo >> 1);
                let hi = (ch << 127) | (hi >> 1);
                r.set_wide(Wide192 { hi, lo });
            } else {
                r.ex = a.ex;
                r.set_wide(Wide192 { hi, lo });
            }
        }
        Some(r)
    }

    /// The exact value of a finite non-zero double (C: `tint_fromd`). `None` never: its
    /// exponent arithmetic is on values below 2^12.
    fn from_f64(x: f64) -> Option<Tint> {
        let bits = x.to_bits();
        let ax = bits & (u64::MAX >> 1);
        let e = signed(ax >> 52); // the exponent field, below 2^11: its value
        let (h, ex) = if e != 0 {
            ((1u64 << 63) | (ax << 11), e.checked_sub(0x3fe)?)
        } else {
            let z = ax.leading_zeros();
            (
                shl(ax, u64::from(z)),
                (-0x3f2i64).checked_sub(i64::from(z))?,
            )
        };
        Some(Tint {
            h,
            m: 0,
            l: 0,
            ex,
            sgn: bits >> 63,
        })
    }

    /// The double nearest this value, which has at most `err` ulps of error in its low word
    /// (C: `tint_tod`). With `err = 0` the value is truncated to 53 bits, which is exact for the
    /// value of a double.
    ///
    /// If the value is within `err` of a midpoint between two doubles, rounding to nearest cannot
    /// be decided from it. That has never been observed for atan2 (see the module
    /// documentation); the double nearest the 192-bit value is returned then too.
    fn to_f64(self, err: u64) -> Option<f64> {
        let s = if self.sgn != 0 { -1.0 } else { 1.0 };
        if self.ex >= 1025 {
            return Some(s * f64::INFINITY); // |a| ≥ 2^1024
        }
        if self.ex <= -1074 {
            // |a| < 2^−1074: 0, or the smallest subnormal above the midpoint 2^−1075
            if self.ex < -1074 {
                return Some(s * 0.0);
            }
            let mid = self.h == 1 << 63 && self.m == 0 && self.l == 0;
            return Some(if mid { s * 0.0 } else { s * f64::from_bits(1) });
        }
        let (mut hh, mut mm, mut ll) = (self.h, self.m, self.l);
        let mut ex = self.ex;
        let mut low = hh & 0x7ff; // round bit (0x400) and the 10 bits after it
        if ex <= -1022 {
            // Subnormal result: sh = −1021 − ex, with 1 ≤ sh ≤ 52 (−1073 ≤ ex ≤ −1022), so
            // 64 − sh is exact too.
            let sh = ex.abs_diff(-1021);
            let back = 64u64.abs_diff(sh);
            ll = shl(mm, back) | shr(ll, sh) | u64::from(ll > 0);
            mm = shl(hh, back) | shr(mm, sh);
            hh = shr(hh, sh);
            low = hh & 0x7ff;
            ex = ex.checked_add_unsigned(sh)?;
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "hh >> 11 is below 2^53, so the conversion is exact"
        )]
        let h = (hh >> 11) as f64; // the 53 leading bits, exactly
        let l = if err == 0 {
            0.0
        } else if low < 0x400 {
            0.25 // below the midpoint: round down
        } else if low > 0x400 || mm != 0 || ll != 0 {
            0.75 // above the midpoint: round up
        } else {
            0.5 // exactly the midpoint: ties to even
        };
        let h = fma(l, s, s * h) * hf!("0x1p-52");
        // C: the bits `(ex − 1 + 1023) << 52`, 2^(ex−1), with −1021 ≤ ex ≤ 1024 here.
        Some(h * pow2(ex.checked_sub(1)))
    }

    /// An approximation of `1/a` for `a ≠ 0` (C: `inv_tint`): one Newton step from the reciprocal
    /// of `a` truncated to a double. For `1 ≤ r ≤ 2` its absolute error is below 2^−103.9 when `a`
    /// is a double (tint.h), and below 2^−101.83 for any `a`, whose truncation adds up to 2^−52 to
    /// the starting residual (proof, step A; `division_error_bound`).
    fn inv(a: &Tint) -> Option<Tint> {
        let mut ad = a.to_f64(0)?;
        let subnormal = ad.abs() < f64::MIN_POSITIVE;
        if subnormal {
            ad *= hf!("0x1p53");
        }
        let mut r = Tint::from_f64(1.0 / ad)?;
        if subnormal {
            r.ex = r.ex.checked_add(53)?;
        }
        let mut q = a.mul(&r)?; // ≈ a·r
        q.sgn ^= 1; // C: `1 − sgn` for a sign of 0 or 1
        let q = ONE.add(&q)?; // ≈ 1 − a·r
        let q = r.mul(&q)?; // ≈ r·(1 − a·r)
        r.add(&q)
    }

    /// `self / a` for `a ≠ 0`, relative error below 2^−185.53 (C: `div_tint`): the reciprocal,
    /// then one Karp–Markstein correction.
    fn div(&self, a: &Tint) -> Option<Tint> {
        let y = Tint::inv(a)?;
        let r = y.mul(self)?; // ≈ b/a
        let mut z = a.mul(&r)?;
        z.sgn ^= 1; // C: `1 − sgn` for a sign of 0 or 1
        let z = self.add(&z)?; // ≈ b − a·r
        let z = y.mul(&z)?;
        r.add(&z)
    }

    /// `b / a` for doubles, relative error below 2^−185.53 (C: `div_tint_d`).
    fn div_f64(b: f64, a: f64) -> Option<Tint> {
        Tint::from_f64(b)?.div(&Tint::from_f64(a)?)
    }
}

/// Numerator coefficients: `z·P(z)/Q(z)` approximates `atan(z)` on [0, 1] with relative error
/// below 2^−193 (proof, step A1; rminimax's figure is 3.99613·10^−59) (C: `P`).
#[rustfmt::skip]
const P: [Tint; 30] = [
    t(0x8270_3f8b_5311_2eca, 0x71ce_6279_d467_aa45, 0x019c_9b28_466b_6fc0, -10, 0),
    t(0xb3cf_74b4_27d5_3e03, 0xee59_0b83_9e75_1890, 0xf257_0b12_bcb2_381a, -8, 0),
    t(0xb084_98b1_7b88_c39a, 0x11e3_7821_d784_d8fa, 0x2795_b14b_84b9_d78c, -6, 0),
    t(0xfd4b_aa7b_b52b_0d83, 0xb254_25be_b0a0_4dc3, 0x9200_8d9f_b34b_68ff, -5, 0),
    t(0x9654_1948_b85f_b386, 0xe0b8_15ac_e425_a859, 0xbeca_17c6_9bb4_f7a0, -3, 0),
    t(0x96aa_a5d4_baff_c590, 0xf23d_b783_b6e4_ce55, 0x2c54_e054_d3b6_9f7e, -2, 0),
    t(0x83cd_41b5_fa86_2b11, 0xad8f_9d6e_3eb9_d833, 0x4ef4_70d8_01fa_03ca, -1, 0),
    t(0xcbbc_7752_197c_c106, 0xbc66_cfb3_edef_a6e3, 0x568b_2f9e_4feb_8910, -1, 0),
    t(0x8cf8_f7c5_8576_7042, 0xcd92_a5b9_ac13_d8c8, 0x043b_d4a0_23db_e04d, 0, 0),
    t(0xafdc_36d6_b361_39bf, 0x11df_6347_4e38_59d4, 0x42b3_0b2c_089a_2b6c, 0, 0),
    t(0xc6f8_fb14_6382_2519, 0x2f9f_04cf_55c1_0377, 0x1c44_675c_6d48_3606, 0, 0),
    t(0xcce7_9f10_f754_e315, 0x91ce_47ed_cbc8_a4ee, 0xb2a6_f721_ac65_6b42, 0, 0),
    t(0xc090_384d_bd77_132e, 0x1dfb_4d9a_1d59_5de7, 0xe506_ca89_9a39_4f70, 0, 0),
    t(0xa556_ac9e_ee25_e867, 0xd84d_afa2_5141_d203, 0xf1c7_c2a5_156b_3ec3, 0, 0),
    t(0x81bd_2677_ddfc_bc38, 0x1df3_3f27_dbb6_0939, 0xcb4c_2c73_286a_6c29, 0, 0),
    t(0xb9ed_1216_9d72_e8d9, 0xc81d_95dc_ef13_c227, 0x974b_9579_6b93_c332, -1, 0),
    t(0xf2df_6da8_75b6_66fa, 0xc29d_d3a6_9c27_4c65, 0x83b9_2609_76c8_f0c5, -2, 0),
    t(0x902d_34d4_db91_0ed8, 0x6f3e_cddf_d6af_0a9f, 0x8e95_0d7e_70a8_e9b2, -2, 0),
    t(0x9aef_a419_61ce_1207, 0xacd0_3176_6675_3751, 0x3eaf_42a5_3fcc_6534, -3, 0),
    t(0x95db_9977_c5e6_e263, 0x8536_7a63_fab8_a720, 0xfc2b_63bd_1159_98d8, -4, 0),
    t(0x817c_b6d8_236d_5f4d, 0x70c7_815f_2890_9d9b, 0x0c54_e971_55d1_2158, -5, 0),
    t(0xc5ee_c422_8c35_2098, 0x789e_56be_1dee_9b67, 0xba25_bda8_1292_ccc1, -7, 0),
    t(0x840c_38cc_fcb5_dc64, 0xb55a_4409_5bf7_f312, 0x6668_b879_2d4e_cd08, -8, 0),
    t(0x9707_4af7_b286_b19e, 0xc20a_ac21_f313_0f1d, 0x369e_9fdb_dd48_528a, -10, 0),
    t(0x9054_011c_32d4_b7a5, 0xc68d_3c5d_525f_4af4, 0xeeb3_c986_15de_e188, -12, 0),
    t(0xdddb_4a2e_aab2_bb15, 0x7537_61a8_f0e9_aa8d, 0x3f20_620c_d3e0_75bb, -15, 0),
    t(0x80ef_9a1b_3e59_abe5, 0x18e6_afe0_651f_df49, 0xd1c0_329e_6a70_2d9d, -17, 0),
    t(0xca16_dd49_cd30_854c, 0xbad8_ca37_9401_027c, 0x1fc7_f479_8548_6f16, -21, 0),
    t(0xa0ba_01b8_742b_a801, 0xe6dc_14a9_516d_7f04, 0x1f53_03d8_7890_5d2f, -25, 0),
    t(0x9c04_4089_6ce6_515c, 0x0cf3_470f_a7f1_0ac3, 0xa9dc_8068_2c11_1a9d, -59, 1),
];

/// Denominator coefficients (C: `Q`).
#[rustfmt::skip]
const Q: [Tint; 30] = [
    t(0x8270_3f8b_5311_2eca, 0x71ce_6279_d467_aa45, 0x019c_9b28_466b_6fc0, -10, 0),
    t(0xb3cf_74b4_27d5_3e03, 0xee59_0b83_9e75_1890, 0xf257_0b12_bcb2_38cf, -8, 0),
    t(0xb33c_44af_0d43_c9e8, 0xf442_6f84_6147_01db, 0x979e_49dc_5b85_8442, -6, 0),
    t(0x8264_d256_f0bf_4d61, 0xee21_431c_c0f2_9764, 0xce0c_c1b5_93ac_9952, -4, 0),
    t(0x9d97_c494_7687_b2ee, 0xef40_87c4_9dde_5229, 0x742b_c6e0_2bfb_6df8, -3, 0),
    t(0xa0f8_88dc_7aab_949e, 0xf623_f19a_c413_db23, 0x8547_44a1_64f7_0ecd, -2, 0),
    t(0x8fd9_c717_2ab0_828e, 0x1a9f_72fa_dafc_3dfb, 0xaf96_a180_ea29_7178, -1, 0),
    t(0xe381_68b7_8a51_ee15, 0x604a_4781_5c80_91eb, 0x9faa_a423_12e8_eb30, -1, 0),
    t(0xa164_ccdb_b65e_e133, 0xe623_433c_5e08_6402, 0xcb22_8afd_335a_245f, 0, 0),
    t(0xced0_c0cb_e44f_ac2f, 0xd7c2_adfc_7c5d_da9e, 0xa22b_4a31_1cf5_a5b1, 0, 0),
    t(0xf0e6_6604_38c9_0371, 0x3dca_1a0f_58f0_d943, 0x8a92_b9de_68c4_488c, 0, 0),
    t(0x8000_0000_0000_0000, 0x0000_0000_0000_0000, 0x0000_0000_0000_0000, 1, 0),
    t(0xf8ea_68f9_ad84_9103, 0xb257_e4bf_c606_967f, 0x818f_a281_6928_c4aa, 0, 0),
    t(0xddc6_4eee_e350_6234, 0x47a3_3ea4_0a2b_22e4, 0xa6a2_c129_ace2_470a, 0, 0),
    t(0xb52d_dfba_2d37_c8e2, 0x2455_1a9b_19fa_064a, 0x3d78_c68b_54bc_3850, 0, 0),
    t(0x87ac_113d_26b6_83a1, 0x4e97_756c_3878_5b0b, 0xf75f_0a45_0693_1ba8, 0, 0),
    t(0xba06_5070_b900_f3d5, 0xe49d_8f9f_238f_1eeb, 0x6805_d48d_67f1_dcd9, -1, 0),
    t(0xe902_d2a4_bfc2_2441, 0x945c_fe6c_83a5_b828, 0x519e_bb45_7f8c_cca0, -2, 0),
    t(0x84e4_05c9_15f7_769b, 0x0faa_f34c_3c51_dd39, 0xa97f_a0ae_9938_2c93, -2, 0),
    t(0x896d_a46c_7e6c_ebb3, 0xe0af_c0b6_db3d_bbf5, 0x061c_cff6_e78e_fff4, -3, 0),
    t(0x801a_0c1a_0dfe_fc92, 0xf8e0_a979_02e0_8d47, 0x8189_8e0b_7d8e_f0bc, -4, 0),
    t(0xd59d_0889_30df_07db, 0xa8cb_0002_1849_fbaf, 0xf80c_c91a_a0bd_df89, -6, 0),
    t(0x9db5_6a69_84b7_7231, 0xcf7c_2074_e048_0959, 0x3316_ecaa_6aa4_604e, -7, 0),
    t(0xcb74_50d9_d62d_2e68, 0x9787_b426_5977_d004, 0xba61_5cee_00a8_41d7, -9, 0),
    t(0xe12a_52e0_2b64_b23e, 0x97c9_139a_8ced_716c, 0x70a1_76ad_2b5f_16d2, -11, 0),
    t(0xd057_b6d0_900e_d30d, 0x5e04_bb17_eb87_9a5e, 0x23ed_135b_60be_a070, -13, 0),
    t(0x9b1f_1ca2_2fef_7ae5, 0x19a6_87f0_a407_3eee, 0x05a9_ccbc_8c14_578b, -15, 0),
    t(0xaeba_710f_053a_ac3b, 0x30ea_5d6f_b116_f775, 0x8a9b_cfef_5698_f8f1, -18, 0),
    t(0x84b9_ada0_76cd_007d, 0x13e5_5006_073f_76b1, 0x4cf4_2599_6822_8f3e, -21, 0),
    t(0xcca4_b904_6786_937d, 0x233c_1e69_1c12_7c1f, 0xf493_e0b7_a3de_459a, -26, 0),
];

/// `t₀ = j/64` for `0 ≤ j ≤ 64` (C: `T2`).
#[rustfmt::skip]
const T2: [f64; 65] = [
    hf!("0x0p+0"), hf!("0x1p-6"), hf!("0x1p-5"), hf!("0x1.8p-5"),
    hf!("0x1p-4"), hf!("0x1.4p-4"), hf!("0x1.8p-4"), hf!("0x1.cp-4"),
    hf!("0x1p-3"), hf!("0x1.2p-3"), hf!("0x1.4p-3"), hf!("0x1.6p-3"),
    hf!("0x1.8p-3"), hf!("0x1.ap-3"), hf!("0x1.cp-3"), hf!("0x1.ep-3"),
    hf!("0x1p-2"), hf!("0x1.1p-2"), hf!("0x1.2p-2"), hf!("0x1.3p-2"),
    hf!("0x1.4p-2"), hf!("0x1.5p-2"), hf!("0x1.6p-2"), hf!("0x1.7p-2"),
    hf!("0x1.8p-2"), hf!("0x1.9p-2"), hf!("0x1.ap-2"), hf!("0x1.bp-2"),
    hf!("0x1.cp-2"), hf!("0x1.dp-2"), hf!("0x1.ep-2"), hf!("0x1.fp-2"),
    hf!("0x1p-1"), hf!("0x1.08p-1"), hf!("0x1.1p-1"), hf!("0x1.18p-1"),
    hf!("0x1.2p-1"), hf!("0x1.28p-1"), hf!("0x1.3p-1"), hf!("0x1.38p-1"),
    hf!("0x1.4p-1"), hf!("0x1.48p-1"), hf!("0x1.5p-1"), hf!("0x1.58p-1"),
    hf!("0x1.6p-1"), hf!("0x1.68p-1"), hf!("0x1.7p-1"), hf!("0x1.78p-1"),
    hf!("0x1.8p-1"), hf!("0x1.88p-1"), hf!("0x1.9p-1"), hf!("0x1.98p-1"),
    hf!("0x1.ap-1"), hf!("0x1.a8p-1"), hf!("0x1.bp-1"), hf!("0x1.b8p-1"),
    hf!("0x1.cp-1"), hf!("0x1.c8p-1"), hf!("0x1.dp-1"), hf!("0x1.d8p-1"),
    hf!("0x1.ep-1"), hf!("0x1.e8p-1"), hf!("0x1.fp-1"), hf!("0x1.f8p-1"),
    hf!("0x1p+0"),
];

/// `atan(j/64)` for `0 ≤ j ≤ 64` as `[low, high]` (C: `f2`).
#[rustfmt::skip]
const F2: [[f64; 2]; 65] = [
    [hf!("0x0p+0"), hf!("0x0p+0")],
    [hf!("-0x1.95220c39d4dffp-53"), hf!("0x1.fff555bbb73p-7")],
    [hf!("0x1.2542779d776dep-53"), hf!("0x1.ffd55bba976p-6")],
    [hf!("-0x1.6061bbe3de53cp-53"), hf!("0x1.7fb818430da4p-5")],
    [hf!("-0x1.639269b0da47ep-53"), hf!("0x1.ff55bb72cfep-5")],
    [hf!("-0x1.4a7663af440f7p-55"), hf!("0x1.3f59f0e7c55ap-4")],
    [hf!("0x1.d1824d59f9e13p-53"), hf!("0x1.7ee182602f1p-4")],
    [hf!("0x1.bef71e5340b31p-55"), hf!("0x1.be39ebe6f07cp-4")],
    [hf!("0x1.b8cb225e627dp-53"), hf!("0x1.fd5ba9aac2f6p-4")],
    [hf!("0x1.b92de9bac94c2p-53"), hf!("0x1.1e1fafb04372p-3")],
    [hf!("-0x1.d3cb89e62dafdp-54"), hf!("0x1.3d6eee8c6627p-3")],
    [hf!("-0x1.882a55960087ap-53"), hf!("0x1.5c9811e3ec27p-3")],
    [hf!("0x1.1347b0b4f881dp-54"), hf!("0x1.7b97b4bce5bp-3")],
    [hf!("0x1.873d8079ed0d2p-53"), hf!("0x1.9a6a8e96c862p-3")],
    [hf!("0x1.022f621a5c1cbp-54"), hf!("0x1.b90d7529260ap-3")],
    [hf!("0x1.9c648d1534598p-53"), hf!("0x1.d77d5df20573p-3")],
    [hf!("-0x1.4ea9238610a08p-54"), hf!("0x1.f5b75f92c80ep-3")],
    [hf!("0x1.2c5c8e721970dp-53"), hf!("0x1.09dc597d8636p-2")],
    [hf!("0x1.30ca4748b1bf9p-57"), hf!("0x1.18bf5a30bf178p-2")],
    [hf!("-0x1.20ef9ba6dbf9p-53"), hf!("0x1.278372057ef48p-2")],
    [hf!("0x1.e69c5abb498d2p-53"), hf!("0x1.362773707ebc8p-2")],
    [hf!("0x1.a8a86f0ea9311p-54"), hf!("0x1.44aa436c2af08p-2")],
    [hf!("0x1.db5336feef7fp-54"), hf!("0x1.530ad9951cd48p-2")],
    [hf!("0x1.9636a3aa3b84p-54"), hf!("0x1.614840309cfep-2")],
    [hf!("0x1.1ce2a8c848b74p-55"), hf!("0x1.6f61941e4defp-2")],
    [hf!("-0x1.4b1bbd1ea6db3p-55"), hf!("0x1.7d5604b63b3f8p-2")],
    [hf!("-0x1.4925e8b916e0bp-53"), hf!("0x1.8b24d394a1b28p-2")],
    [hf!("0x1.9e6c988fd0a77p-56"), hf!("0x1.98cd5454d6b18p-2")],
    [hf!("-0x1.a49bd836a17p-53"), hf!("0x1.a64eec3cc24p-2")],
    [hf!("-0x1.ca3cf09c6b5f8p-53"), hf!("0x1.b3a911da65c7p-2")],
    [hf!("-0x1.cc1ce70934c34p-56"), hf!("0x1.c0db4c94ec9fp-2")],
    [hf!("0x1.2e982ddf3872ap-55"), hf!("0x1.cde53432c135p-2")],
    [hf!("-0x1.2ea406ee84d0fp-55"), hf!("0x1.dac670561bb5p-2")],
    [hf!("-0x1.de35847c81979p-53"), hf!("0x1.e77eb7f175a38p-2")],
    [hf!("-0x1.a3992dc382a23p-57"), hf!("0x1.f40dd0b541418p-2")],
    [hf!("-0x1.b32c949c9d593p-55"), hf!("0x1.0039c73c1a40cp-1")],
    [hf!("-0x1.d5b495f6349e6p-56"), hf!("0x1.0657e94db30dp-1")],
    [hf!("-0x1.f34582f6255fep-53"), hf!("0x1.0c6145b5b43dcp-1")],
    [hf!("0x1.ed42511e3f11dp-54"), hf!("0x1.1255d9bfbd2a8p-1")],
    [hf!("-0x1.1cef189ff9e7fp-54"), hf!("0x1.1835a88be7c14p-1")],
    [hf!("-0x1.928df287a668fp-58"), hf!("0x1.1e00babdefeb4p-1")],
    [hf!("-0x1.e3bde360c7ddbp-53"), hf!("0x1.23b71e2cc9e6cp-1")],
    [hf!("0x1.bd86313ce4fdep-54"), hf!("0x1.2958e59308e3p-1")],
    [hf!("-0x1.8e8a85803cc1dp-53"), hf!("0x1.2ee628406cbccp-1")],
    [hf!("-0x1.77ef7641c777fp-54"), hf!("0x1.345f01cce37bcp-1")],
    [hf!("0x1.b73ef3389d02fp-53"), hf!("0x1.39c391cd41718p-1")],
    [hf!("0x1.ecf8b492644fp-56"), hf!("0x1.3f13fb89e96f4p-1")],
    [hf!("0x1.c1125fd3810c7p-53"), hf!("0x1.445065b795b54p-1")],
    [hf!("0x1.2483350fe548bp-53"), hf!("0x1.4978fa3269eep-1")],
    [hf!("0x1.4a33dbeb3796cp-55"), hf!("0x1.4e8de5bb6ec04p-1")],
    [hf!("-0x1.46edd2af69483p-53"), hf!("0x1.538f57b89062p-1")],
    [hf!("-0x1.2bcb93b18b52ap-53"), hf!("0x1.587d81f732fbcp-1")],
    [hf!("0x1.0028e4bc5e7cap-57"), hf!("0x1.5d58987169b18p-1")],
    [hf!("0x1.ed487acaf1174p-53"), hf!("0x1.6220d115d7b8cp-1")],
    [hf!("-0x1.2dd4dfd7d1777p-53"), hf!("0x1.66d663923e088p-1")],
    [hf!("0x1.2bfe3cf3b9d79p-54"), hf!("0x1.6b798920b3d98p-1")],
    [hf!("-0x1.8c34d25aadef6p-56"), hf!("0x1.700a7c5784634p-1")],
    [hf!("-0x1.f426acf4d3bdbp-54"), hf!("0x1.748978fba8e1p-1")],
    [hf!("-0x1.afe57dd9ff23p-53"), hf!("0x1.78f6bbd5d316p-1")],
    [hf!("-0x1.54fbef0e862abp-54"), hf!("0x1.7d528289fa094p-1")],
    [hf!("0x1.90227758b11bap-54"), hf!("0x1.819d0b7158a4cp-1")],
    [hf!("0x1.16b66e7fc8b8cp-53"), hf!("0x1.85d69576cc2c4p-1")],
    [hf!("-0x1.55b9a5e177a1bp-55"), hf!("0x1.89ff5ff57f1f8p-1")],
    [hf!("0x1.c27cfaa9f7a14p-53"), hf!("0x1.8e17aa99cc05cp-1")],
    [hf!("0x1.1a62633145c07p-55"), hf!("0x1.921fb54442d18p-1")],
];

/// The octant's offset `[high, low]`, indexed by `4·sign(x) + 2·sign(y) + (|y| > |x|)`:
/// 0, π/2, 0, −π/2, π, π/2, −π, −π/2 (C: `O`).
#[rustfmt::skip]
const OCTANT: [[f64; 2]; 8] = [
    [hf!("0x0p+0"), hf!("0x0p+0")],
    [hf!("0x1.921fb54442d18p+0"), hf!("0x1.1a62633145c07p-54")],
    [hf!("0x0p+0"), hf!("0x0p+0")],
    [hf!("-0x1.921fb54442d18p+0"), hf!("-0x1.1a62633145c07p-54")],
    [hf!("0x1.921fb54442d18p+1"), hf!("0x1.1a62633145c07p-53")],
    [hf!("0x1.921fb54442d18p+0"), hf!("0x1.1a62633145c07p-54")],
    [hf!("-0x1.921fb54442d18p+1"), hf!("-0x1.1a62633145c07p-53")],
    [hf!("-0x1.921fb54442d18p+0"), hf!("-0x1.1a62633145c07p-54")],
];

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

    /// u = 2^−53.
    fn u() -> Dy {
        p2(-53)
    }

    /// `a ≤ b`.
    fn le(a: &Dy, b: &Dy) -> bool {
        a.cmp(b) != Ordering::Greater
    }

    /// `y^n`.
    fn pow(y: &Dy, n: u32) -> Dy {
        (0..n).fold(Dy::int(1), |a, _| a.mul(y))
    }

    /// The largest reduced argument: `|z*| ≤ |q − t₀| ≤ 2^−7 + 2^−50`.
    fn z_max() -> Dy {
        p2(-7).add(&p2(-50))
    }

    /// The sign the fast path gives the table value for each octant index (C: `asgn[GT^sx^sy]`
    /// with `kw = sx<<2 | sy<<1 | GT`).
    fn octant_sign(kw: usize) -> f64 {
        let (sx, sy, gt) = ((kw >> 2) & 1, (kw >> 1) & 1, kw & 1);
        if (gt ^ sx ^ sy) != 0 { -1.0 } else { 1.0 }
    }

    /// Steps F1 and F2: the table, the octant offsets, and the exactness of `fh`.
    /// `fh = ±F2[j].high + OCTANT[kw].high` is exact for every index pair (the table's high parts
    /// end in zeros for that purpose), `|F2[j] − atan(j/64)| ≤ δ_T`, and the offsets are within
    /// `δ_O` of 0, ±π/2 and ±π.
    #[test]
    fn tables_and_octants() {
        let mut delta_t = Dy::zero();
        for (j, f) in F2.iter().enumerate() {
            let [fl, fh] = *f;
            let t = Iv::point(Dy {
                m: (j as i64).into(),
                e: -6,
            });
            let a = atan_unit_iv(&t, P);
            delta_t = dmax(delta_t, dd_error(fh, fl, &a));
            for (kw, o) in OCTANT.iter().enumerate() {
                let s = octant_sign(kw);
                let exact = Dy::from_f64(fh * s).add(&Dy::from_f64(o[0]));
                let computed = fh * s + o[0];
                assert!(
                    Dy::from_f64(computed).eq(&exact),
                    "j = {j}, kw = {kw}: fh is exact"
                );
            }
            assert!(fl.abs() < 2f64.powi(-52));
        }
        let half_pi = pi().scale(-1);
        let mut delta_o = Dy::zero();
        for o in &OCTANT {
            let v = Dy::from_f64(o[0]).add(&Dy::from_f64(o[1]));
            let k = Dy::div(&v, &half_pi.lo, 64, false).round_to_int();
            let target = half_pi.mul(&Iv::point(Dy::big(k)), P);
            delta_o = dmax(
                delta_o,
                dmax(v.sub(&target.lo).abs(), v.sub(&target.hi).abs()),
            );
        }
        eprintln!(
            "δ_T = 2^{:.3}, δ_O = 2^{:.3}",
            log2_approx(&delta_t),
            log2_approx(&delta_o)
        );
        assert!(le(&delta_t, &Dy::from_f64(DELTA_T)));
        assert!(le(&delta_o, &Dy::from_f64(DELTA_O)));
        // T2[j] = j/64 exactly
        for (j, &t) in T2.iter().enumerate() {
            assert_eq!(t, j as f64 / 64.0);
        }
    }

    /// The approximation error of `z + z³·B(z²)` for `atan z`, relative to `|z|³`, for
    /// `|z| ≤ z_max`, by subdivision of `[0, z_max²]`.
    fn atan_poly_error(b: &[f64]) -> Dy {
        // (atan z − z)/z³ = Σ_k (−1)^(k+1) y^k/(2k + 3), y = z²
        let n = 12usize;
        let mut q: Vec<Iv> = (0..n)
            .map(|k| {
                let v = Iv::int(1).div_int(2 * k as i64 + 3, P);
                if k % 2 == 0 { v.neg() } else { v }
            })
            .collect();
        for (k, &c) in b.iter().enumerate() {
            q[k] = q[k].sub(&Iv::point(Dy::from_f64(c)), P);
        }
        let ym = z_max().mul(&z_max());
        let tail = Dy::div(&pow(&ym, n as u32), &Dy::int(2 * n as i64 + 3), P, true);
        poly_sup_bound(&q, &ym, 256, 192).add(&tail)
    }

    /// Step F3: the first fast path. `E₁(|z|) + u·(|rl| + eps)` is below `eps·(1 − u)` at
    /// `|z| = 0` and `|z| = z_max`; the left side is convex in `|z|` and the right linear.
    #[test]
    fn fast_path_1_bound() {
        const B: [f64; 3] = [
            hf!("-0x1.55555555554d2p-2"),
            hf!("0x1.999999860e1cap-3"),
            hf!("-0x1.248ad469844a1p-3"),
        ];
        let u = u();
        let one = Dy::int(1);
        let up = |d: &Dy, k: i64| d.mul(&one.add(&p2(-k)));
        let eps_b = atan_poly_error(&B);
        eprintln!(
            "first fast path polynomial: 2^{:.3}·|z|³",
            log2_approx(&eps_b)
        );
        let (dt, dof) = (Dy::from_f64(DELTA_T), Dy::from_f64(DELTA_O));
        let fl = p2(-51); // |fl| ≤ |F2 low| + |octant low| < 2^−52 + 2^−52
        let rh = Dy::int(4); // |rh| < 4
        for z in [Dy::zero(), up(&z_max(), 50)] {
            let z3 = pow(&z, 3);
            let dz = up(&z3.scale(-1), 10); // |dz| ≤ |z|³/2
            // |z − z*| ≤ (3u + 4u²)|z*| and its effect on the cubic term; the cubic's
            // approximation and evaluation errors
            let e_z = c3u(&u).mul(&z).mul(&up(&one, 50));
            let cubic = eps_b
                .add(&u.mul(&Dy::int(3)))
                .add(&u.mul(&Dy::from_f64(1.4)))
                .mul(&z3);
            let rl = fl.add(&dz).add(&u.mul(&rh)).mul(&up(&one, 50));
            let e1 = dt
                .add(&dof)
                .add(&u.mul(&fl))
                .add(&e_z)
                .add(&cubic)
                .add(&u.mul(&fl.add(&dz)))
                .add(&u.mul(&rl));
            let eps = z
                .mul(&Dy::from_f64(EPS1_REL))
                .add(&Dy::from_f64(EPS1_ABS))
                .mul(&one.sub(&u))
                .mul(&one.sub(&u));
            let need = e1.add(&u.mul(&rl.add(&eps)));
            let have = eps.mul(&one.sub(&u));
            eprintln!(
                "first fast path at |z| = {:e}: 2^{:.3} ≤ 2^{:.3}",
                z.to_f64(),
                log2_approx(&need),
                log2_approx(&have)
            );
            assert!(le(&need, &have), "F3");
        }
    }

    /// `3u + 4u²`.
    fn c3u(u: &Dy) -> Dy {
        u.mul(&Dy::int(3)).add(&u.mul(u).mul(&Dy::int(4)))
    }

    /// Step F4: the second fast path. `E₂(|z|) + u·(|fl₂| + eps₂)` is below `eps₂·(1 − u)` at
    /// `|z| = 0` and `|z| = z_max`.
    #[test]
    fn fast_path_2_bound() {
        const B2: [f64; 4] = [
            hf!("-0x1.5555555555555p-2"),
            hf!("0x1.999999999755ep-3"),
            hf!("-0x1.24924883596f8p-3"),
            hf!("0x1.c6f7d73531bc2p-4"),
        ];
        let u = u();
        let uu = u.mul(&u);
        let one = Dy::int(1);
        let up = |d: &Dy, k: i64| d.mul(&one.add(&p2(-k)));
        let eps_b2 = atan_poly_error(&B2);
        eprintln!(
            "second fast path polynomial: 2^{:.3}·|z|³",
            log2_approx(&eps_b2)
        );
        let (dt, dof) = (Dy::from_f64(DELTA_T), Dy::from_f64(DELTA_O));
        let fl = p2(-51);
        let sl = u.mul(&Dy::int(4)); // the low part of fh + zh, |fh2| < 4
        for z in [Dy::zero(), up(&z_max(), 50)] {
            let z3 = pow(&z, 3);
            let zl0 = u.mul(&Dy::from_f64(2.02)).mul(&z);
            let zl1 = up(&zl0.add(&z3.scale(-1)), 10);
            // double-double quotient (24u²|z|), cubic term: approximation, the zh-vs-z* shift
            // (2.03u|z|³), evaluation (1.35u|z|³), the final low-part additions
            let e2 = dt
                .add(&dof)
                .add(&u.mul(&fl))
                .add(&uu.mul(&Dy::int(24)).mul(&z))
                .add(&eps_b2.add(&u.mul(&Dy::from_f64(2.03 + 1.35))).mul(&z3))
                .add(&u.mul(&zl1))
                .add(&u.mul(&fl.add(&zl1)));
            let fl2 = up(&fl.add(&zl1).add(&sl), 50);
            let e2 = e2.add(&u.mul(&fl2));
            let eps2 = Dy::from_f64(EPS2_REL)
                .mul(
                    &z3.mul(&one.sub(&u))
                        .mul(&one.sub(&u))
                        .add(&Dy::from_f64(EPS2_ABS)),
                )
                .mul(&one.sub(&u))
                .mul(&one.sub(&u));
            let need = e2.add(&u.mul(&fl2.add(&eps2)));
            let have = eps2.mul(&one.sub(&u));
            eprintln!(
                "second fast path at |z| = {:e}: 2^{:.3} ≤ 2^{:.3}",
                z.to_f64(),
                log2_approx(&need),
                log2_approx(&have)
            );
            assert!(le(&need, &have), "F4");
        }
    }

    // ------------------------------------------------------------------------------------------
    // Step A: the accurate path's error. CORE-MATH's two computed ingredients (atan2.sage's
    // `errPsplit` and `errQsplit`, and rminimax's bound on the rational approximation) are
    // re-derived here with exact arithmetic, over every argument the path can see, and the error
    // assembly of atan2.c is recomputed from them.
    // ------------------------------------------------------------------------------------------

    /// Working precision of the accurate-path checks: well above the 192 bits being bounded.
    const PA: u64 = 400;

    /// ε₀: the bound proven here for the rational approximation's relative error, 2^−193
    /// (CORE-MATH's rminimax figure is 3.99613·10^−59 = 2^−194.34; this crate's rigorous bound
    /// is looser, and the total below has room for it).
    const EPS0: f64 = hf!("0x1p-193");
    /// ε₂: CORE-MATH's bound 2^−184.14 on the relative error of `p ≈ z·P(z)`, confirmed here.
    const EPS2: f64 = hf!("0x1.d0a659c929582p-185");
    /// ε₃: CORE-MATH's bound 2^−184.19 on the relative error of `q ≈ Q(z)`, confirmed here.
    const EPS3: f64 = hf!("0x1.c0d2771113e59p-185");
    /// ε₁ = ε₄: the relative error of `div_tint` and `div_tint_d`, 2^−185.53 (tint.h's analysis,
    /// re-assembled by `division_error_bound`).
    const EPS_DIV: f64 = hf!("0x1.629688baea97bp-186");
    /// The error of [`PI`], 2^−196.96, and of [`PI2`], 2^−197.96 (tint.h), checked here.
    const PI_ERR: f64 = hf!("0x1.073277be0d9fcp-197");
    /// See [`PI_ERR`].
    const PI2_ERR: f64 = hf!("0x1.073277be0d9fcp-198");

    /// The exact value of a [`Tint`].
    fn tint_value(t: &Tint) -> Dy {
        use num_bigint::BigInt;
        let m: BigInt =
            (BigInt::from(t.h) << 128usize) + (BigInt::from(t.m) << 64usize) + BigInt::from(t.l);
        Dy {
            m: if t.sgn != 0 { -m } else { m },
            e: t.ex - 192,
        }
    }

    /// The largest computed quotient: `|y/x| ≤ 1` (or `|x/y| < 1`), times `1 + ε₁` with
    /// `ε₁ < 2^−185.53`, is below `1 + 2^−185`.
    fn z_top() -> Dy {
        Dy::int(1).add(&p2(-185))
    }

    /// Splits `[a, b]` in halves, at most `depth` times, until `bound` is at most `target` on
    /// every piece; returns the largest bound on the final pieces (above `target` only if the
    /// depth ran out).
    fn refined(a: Dy, b: Dy, target: &Dy, depth: u32, bound: &dyn Fn(&Dy, &Dy) -> Dy) -> Dy {
        let mut stack = vec![(a, b, 0u32)];
        let mut worst = Dy::zero();
        while let Some((a, b, d)) = stack.pop() {
            let v = bound(&a, &b);
            if le(&v, target) || d == depth {
                worst = dmax(worst, v);
            } else {
                let m = a.add(&b).scale(-1);
                stack.push((a, m.clone(), d + 1));
                stack.push((m, b, d + 1));
            }
        }
        worst
    }

    /// An upper bound on one unit in the last place, at 192 bits, of any number whose magnitude is
    /// at most `m·(1 + 2^−150)`: `2^(k − 191)` for `2^k ≤ m·(1 + 2^−150) < 2^(k+1)`. (A [`Tint`]
    /// with exponent `ex` lies in `[2^(ex−1), 2^ex)` and its last place is worth `2^(ex−192)`.)
    fn ulp192(m: &Dy) -> Dy {
        if m.is_zero() {
            return Dy::zero();
        }
        let inflated = m.mul(&Dy::int(1).add(&p2(-150)));
        p2(inflated.exponent() - 191)
    }

    /// The smallest magnitude in an interval that does not contain zero.
    fn min_mag(v: &Iv) -> Dy {
        assert!(
            v.lo.signum() > 0 || v.hi.signum() < 0,
            "the polynomial vanishes"
        );
        let (a, b) = (v.lo.abs(), v.hi.abs());
        if le(&a, &b) { a } else { b }
    }

    /// A bound on the relative error of the accurate path's Horner evaluation of `c` (lowest
    /// degree first) at every `z` in `[a, b]`, followed by a multiplication by `z` if `times_z`
    /// (C: the loop of `mul_tint` and `add_tint`, and the final `mul_tint (p, p, z)`): the
    /// interval version of `errP` and `errQ` in CORE-MATH's atan2.sage.
    ///
    /// Each `mul_tint` errs by at most 10 ulps of its result and each `add_tint` by at most 2
    /// (tint.h's analysis). With `v_i` the exact value of a Horner step and `E_i` a bound on the
    /// computed value's distance from it, `E_i = E_{i+1}·z + 10·ulp(v_{i+1}·z) + 2·ulp(v_i)`, each
    /// ulp taken of the largest magnitude the computed value can have. Unlike atan2.sage this
    /// drops no second-order term, and it divides by `|P(z)|` (or `|Q(z)|`) at the same `z`, so
    /// it also holds on an interval reaching down to 0.
    fn horner_relative_error(c: &[Dy], a: &Dy, b: &Dy, times_z: bool) -> Dy {
        let zs = Iv::new(a.clone(), b.clone());
        let zmax = zs.mag();
        let (top, rest) = c.split_last().unwrap();
        let mut v = Iv::point(top.clone());
        let mut e = Dy::zero();
        for ci in rest.iter().rev() {
            let prod = v.mul(&zs, PA);
            let carried = e.mul(&zmax);
            let prod_err = carried.add(&ulp192(&prod.mag().add(&carried)).mul(&Dy::int(10)));
            v = prod.add(&Iv::point(ci.clone()), PA);
            e = prod_err.add(&ulp192(&v.mag().add(&prod_err)).mul(&Dy::int(2)));
        }
        let rel = Dy::div(&e, &min_mag(&v), PA, true);
        if times_z {
            // The last mul_tint errs by 10 ulps of its result: at most 10·2^−191 of it, and,
            // away from zero, 10 ulps of the largest product over the smallest.
            let crude = p2(-191)
                .mul(&Dy::int(10))
                .mul(&Dy::int(1).add(&rel))
                .mul(&Dy::int(1).add(&p2(-150)));
            let last = if a.is_zero() {
                crude
            } else {
                let w = v.mul(&zs, PA);
                let ulps = ulp192(&w.mag().add(&e.mul(&zmax))).mul(&Dy::int(10));
                let fine = Dy::div(&ulps, &min_mag(&w), PA, true);
                if le(&fine, &crude) { fine } else { crude }
            };
            rel.add(&last)
        } else {
            rel
        }
    }

    /// The largest bound of `bound` over every argument the accurate path's polynomials can
    /// see, `0 < z ≤ 1 + 2^−185` (the computed quotient): `[0, 2^−60]`, then every binade up to
    /// `[½, 1 + 2^−185]` in four pieces, each refined until it meets `target`.
    fn over_accurate_arguments(target: &Dy, bound: &dyn Fn(&Dy, &Dy) -> Dy) -> Dy {
        let mut worst = refined(Dy::zero(), p2(-60), target, 12, bound);
        for e in -59..=0i64 {
            let lo = p2(e - 1);
            let hi = if e == 0 { z_top() } else { p2(e) };
            for i in 0..4 {
                let a = lo.add(&hi.sub(&lo).mul(&Dy::int(i)).scale(-2));
                let b = if i == 3 {
                    hi.clone()
                } else {
                    lo.add(&hi.sub(&lo).mul(&Dy::int(i + 1)).scale(-2))
                };
                worst = dmax(worst, refined(a, b, target, 12, bound));
            }
        }
        worst
    }

    /// Step A2: the Horner evaluations of `z·P(z)` and `Q(z)` in [`Tint`] arithmetic have relative
    /// errors below CORE-MATH's ε₂ = 2^−184.14 and ε₃ = 2^−184.19 for every argument the
    /// accurate path can give them, `0 < z ≤ 1 + 2^−185`. atan2.sage computed this for
    /// `2^−12 ≤ z ≤ 1` only, but the path is also reached with smaller quotients (an exponent
    /// difference of 53 or more, or a fast path that cannot decide); this covers them all.
    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "exact-arithmetic proof check: seconds in release, minutes in a debug build"
    )]
    fn accurate_polynomial_evaluation() {
        let pc: Vec<Dy> = super::P.iter().map(tint_value).collect();
        let qc: Vec<Dy> = Q.iter().map(tint_value).collect();
        let (t2, t3) = (Dy::from_f64(EPS2), Dy::from_f64(EPS3));
        let worst_p = over_accurate_arguments(&t2, &|a, b| horner_relative_error(&pc, a, b, true));
        let worst_q = over_accurate_arguments(&t3, &|a, b| horner_relative_error(&qc, a, b, false));
        eprintln!(
            "accurate path: p within 2^{:.3}, q within 2^{:.3} (relative)",
            log2_approx(&worst_p),
            log2_approx(&worst_q)
        );
        assert!(le(&worst_p, &t2), "ε₂");
        assert!(le(&worst_q, &t3), "ε₃");
    }

    /// `c(x + s)` as coefficients in `x`, lowest degree first (Taylor shift by repeated
    /// synthetic division), in interval arithmetic.
    fn shift(c: &[Iv], s: &Dy) -> Vec<Iv> {
        let mut a = c.to_vec();
        let n = a.len();
        let s = Iv::point(s.clone());
        for i in 0..n.saturating_sub(1) {
            for j in (i..n - 1).rev() {
                a[j] = a[j].add(&a[j + 1].mul(&s, PA), PA);
            }
        }
        a
    }

    /// The product of two polynomials.
    fn poly_mul(a: &[Iv], b: &[Iv]) -> Vec<Iv> {
        let mut r = vec![Iv::int(0); a.len() + b.len() - 1];
        for (i, x) in a.iter().enumerate() {
            for (j, y) in b.iter().enumerate() {
                r[i + j] = r[i + j].add(&x.mul(y, PA), PA);
            }
        }
        r
    }

    /// `Σ |c_k|·ρ^k`: a bound on `|c(h)|` for `|h| ≤ ρ`.
    fn poly_mag(c: &[Iv], rho: &Dy) -> Dy {
        let mut total = Dy::zero();
        let mut pw = Dy::int(1);
        for ck in c {
            total = total.add(&ck.mag().mul(&pw)).round(PA, true);
            pw = pw.mul(rho).round(PA, true);
        }
        total
    }

    /// A lower bound on `c(h)` for `|h| ≤ ρ`: `c_0 − Σ_{k≥1} |c_k|·ρ^k`.
    fn poly_min(c: &[Iv], rho: &Dy) -> Dy {
        let (c0, rest) = c.split_first().unwrap();
        c0.lo.sub(&poly_mag(rest, rho).mul(rho))
    }

    /// A rigorous bound on the rational approximation's relative error,
    /// `|z·P(z)/Q(z) − atan z|/atan z`, over `[a, b]` with `0 ≤ a < b ≤ 1 + 2^−185`.
    ///
    /// Write `z = c + h` with `c` the midpoint and `|h| ≤ ρ`. Near zero (`b ≤ 1/16`),
    /// `T(z) = atan(z)/z = Σ (−1)^n z^(2n)/(2n+1)` is cut after `n = 32` (an alternating series
    /// with decreasing terms, so the remainder is below `b^66/67`), the polynomial `F = P − Q·T₃₂`
    /// is shifted to `c`, and the error is `|F − Q·rem|/(Q·T)` with `T ≥ 1 − b²/3`. Elsewhere
    /// `atan(c + h) = Σ a_k h^k` with `a_0 = atan c` (the reference) and `a_k = s_{k−1}/k` for the
    /// coefficients `s` of `1/(1 + (c + h)²)`, by the recurrence
    /// `(1 + c²)·s_j + 2c·s_{j−1} + s_{j−2} = 0`; since
    /// `1/(1 + z²) = (1/(z − i) − 1/(z + i))/(2i)`, `|s_j| ≤ (1 + c²)^(−(j+1)/2) ≤ 1`, so cutting
    /// after `k = 40` leaves a remainder below `ρ^41/(41·(1 − ρ))`. Then
    /// `G = (c + h)·P(c + h) − Q(c + h)·Σ_{k≤40} a_k h^k`, and the error is
    /// `|G − Q·rem|/(Q·atan a)`. Each polynomial is bounded by the sum of its coefficients'
    /// magnitudes times `ρ^k`, and `Q` from below by its constant term minus the rest.
    fn rational_error_on(a: &Dy, b: &Dy, pc: &[Iv], qc: &[Iv]) -> Dy {
        /// Where the series of `atan(c + h)` is cut (see above).
        const K: usize = 40;
        let one = Dy::int(1);
        let c = a.add(b).scale(-1);
        let rho = b.sub(a).scale(-1);
        let qs = shift(qc, &c);
        let q_min = poly_min(&qs, &rho);
        assert!(q_min.signum() > 0, "Q vanishes");
        let q_max = poly_mag(&qs, &rho);
        if le(b, &p2(-4)) {
            const N: i64 = 32;
            let t: Vec<Iv> = (0..=2 * N)
                .map(|k| {
                    if k % 2 == 1 {
                        Iv::int(0)
                    } else {
                        let v = Iv::int(1).div_int(k + 1, PA);
                        if (k / 2) % 2 == 1 { v.neg() } else { v }
                    }
                })
                .collect();
            let qt = poly_mul(qc, &t);
            let mut f: Vec<Iv> = qt.iter().map(Iv::neg).collect();
            for (k, pk) in pc.iter().enumerate() {
                f[k] = f[k].add(pk, PA);
            }
            let fs = shift(&f, &c);
            let rem = Dy::div(&pow(b, 2 * N as u32 + 2), &Dy::int(2 * N + 3), PA, true);
            let num = poly_mag(&fs, &rho).add(&q_max.mul(&rem));
            let t_min = one.sub(&Dy::div(&b.mul(b), &Dy::int(3), PA, true));
            return Dy::div(&num, &q_min.mul(&t_min), PA, true);
        }
        let big_a = Iv::point(one.add(&c.mul(&c)));
        let big_b = Iv::point(c.scale(1));
        let mut s: Vec<Iv> = vec![Iv::int(1).div(&big_a, PA)];
        s.push(s[0].mul(&big_b, PA).div(&big_a, PA).neg());
        for j in 2..K {
            let v = s[j - 1]
                .mul(&big_b, PA)
                .add(&s[j - 2], PA)
                .div(&big_a, PA)
                .neg();
            s.push(v);
        }
        let mut atan_c: Vec<Iv> = vec![atan_unit_iv(&Iv::point(c.clone()), PA)];
        for (k, sk) in s.iter().enumerate() {
            atan_c.push(sk.div_int(k as i64 + 1, PA));
        }
        // z·P(z), shifted to c
        let mut zp = vec![Iv::int(0)];
        zp.extend(pc.iter().cloned());
        let zps = shift(&zp, &c);
        let qa = poly_mul(&qs, &atan_c);
        let mut g: Vec<Iv> = qa.iter().map(Iv::neg).collect();
        for (k, v) in zps.iter().enumerate() {
            g[k] = g[k].add(v, PA);
        }
        let rem = Dy::div(
            &pow(&rho, K as u32 + 1),
            &Dy::int(K as i64 + 1).mul(&one.sub(&rho)),
            PA,
            true,
        );
        let num = poly_mag(&g, &rho).add(&q_max.mul(&rem));
        let atan_a = atan_unit_iv(&Iv::point(a.clone()), PA).lo;
        Dy::div(&num, &q_min.mul(&atan_a), PA, true)
    }

    /// Step A1: `z·P(z)/Q(z)` approximates `atan z` with relative error below ε₀ = 2^−193 for
    /// every `0 < z ≤ 1 + 2^−185`, by [`rational_error_on`] over 256 equal pieces of `[0, 1]` (the
    /// last one reaching `1 + 2^−185`), each refined until it meets the bound.
    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "exact-arithmetic proof check: seconds in release, minutes in a debug build"
    )]
    fn rational_approximation_error() {
        let pc: Vec<Iv> = super::P.iter().map(|t| Iv::point(tint_value(t))).collect();
        let qc: Vec<Iv> = Q.iter().map(|t| Iv::point(tint_value(t))).collect();
        let target = Dy::from_f64(EPS0);
        let bound = |a: &Dy, b: &Dy| rational_error_on(a, b, &pc, &qc);
        let n: i64 = 256;
        let mut worst = Dy::zero();
        for i in 0..n {
            let a = Dy::int(i).scale(-8);
            let b = if i + 1 == n {
                z_top()
            } else {
                Dy::int(i + 1).scale(-8)
            };
            worst = dmax(worst, refined(a, b, &target, 6, &bound));
        }
        eprintln!(
            "rational approximation: relative error ≤ 2^{:.3} (rminimax: 3.99613e−59 = 2^−194.34)",
            log2_approx(&worst)
        );
        assert!(le(&worst, &target), "ε₀");
    }

    /// Step A3: the error assembly of atan2.c, from the bounds above and tint.h's. With
    /// `z = (y/x)(1 + ε₁)`, `p = z·P(z)(1 + ε₂)`, `q = Q(z)(1 + ε₃)`, the quotient
    /// `p/q·(1 + ε₄)`, `z·P(z)/Q(z) = atan(z)(1 + ε₀)` and `atan z = atan(y/x)(1 + ε₅)` with
    /// `ε₅ ≤ ε₁·(4/π)` (`t/atan t ≤ 4/π` on `[0, 1]`), the result is
    /// `atan(y/x)·(1 + ε₀)(1 + ε₂)(1 + ε₄)(1 + ε₅)/(1 + ε₃)`, whose relative error is below
    /// 2^−182.63, that is 662 units of the last place of a 192-bit value. The constants `π` and
    /// `π/2` are within 2^−196.96 and 2^−197.96 of their values; adding `π/2` (2 ulps of a result
    /// below 2, at most 2^−190) gives at most 524 units of `ulp(π/4) = 2^−192`, and adding `π`
    /// (at most 2^−189) at most 266 units of `ulp(π/2) = 2^−191`: the counts the code passes to
    /// [`Tint::to_f64`].
    #[test]
    fn accurate_path_total() {
        let one = Dy::int(1);
        let half_pi = pi().scale(-1);
        let pi_err = dmax(
            tint_value(&PI).sub(&pi().lo).abs(),
            tint_value(&PI).sub(&pi().hi).abs(),
        );
        let pi2_err = dmax(
            tint_value(&PI2).sub(&half_pi.lo).abs(),
            tint_value(&PI2).sub(&half_pi.hi).abs(),
        );
        assert!(le(&pi_err, &Dy::from_f64(PI_ERR)), "π");
        assert!(le(&pi2_err, &Dy::from_f64(PI2_ERR)), "π/2");
        let e = |v: f64| Dy::from_f64(v);
        let e5 = Dy::div(&e(EPS_DIV).mul(&Dy::int(4)), &pi().lo, PA, true);
        let num = one
            .add(&e(EPS0))
            .mul(&one.add(&e(EPS2)))
            .mul(&one.add(&e(EPS_DIV)))
            .mul(&one.add(&e5));
        let eps6 = Dy::div(&num, &one.sub(&e(EPS3)), PA, true).sub(&one);
        eprintln!(
            "accurate path: relative error ≤ 2^{:.3}",
            log2_approx(&eps6)
        );
        assert!(le(&eps6, &e(hf!("0x1.4ad7a2967b268p-183"))), "2^−182.63");
        assert!(le(&eps6.scale(192), &Dy::int(662)), "662 ulps");
        // π/2 − z, with |z| < π/4 and z's absolute error below ε₆·π/4
        let quarter_pi = pi().hi.scale(-2);
        let after_half = eps6.mul(&quarter_pi).add(&e(PI2_ERR)).add(&p2(-190));
        assert!(le(&after_half, &Dy::int(524).mul(&p2(-192))), "524 ulps");
        // ±π + z, with z's absolute error below the larger of the two above
        let before = dmax(eps6.mul(&quarter_pi), after_half);
        let after_pi = before.add(&e(PI_ERR)).add(&p2(-189));
        assert!(le(&after_pi, &Dy::int(266).mul(&p2(-191))), "266 ulps");
    }

    /// Step A, the division `ε₁ = ε₄`: tint.h's error analysis of `inv_tint` and `div_tint`,
    /// assembled again from its per-operation figures (with the divisor scaled to `[½, 1)`, so
    /// that `1 < Y ≤ 2` and the quotient is in `[½, 2]`), for both starting points of Newton's
    /// step. tint.h assumes the double the step starts from is the divisor `a` itself, so that the
    /// residual `1 − a·r₀` is the reciprocal's rounding, at most `u`; that holds for
    /// [`Tint::div_f64`]. For `p/q` the 192-bit `q` is truncated to 53 bits first, which adds up to
    /// `2u` relative, so the residual is at most `3u + 2u²`. Either way the bound holds.
    #[test]
    fn division_error_bound() {
        let u = u();
        let two = Dy::int(2);
        let ulps = |n: i64, k: i64| Dy::int(n).mul(&p2(k));
        for (case, e0) in [
            ("divisor a double", u.clone()),
            (
                "divisor truncated",
                u.mul(&Dy::int(3)).add(&u.mul(&u).mul(&two)),
            ),
        ] {
            // inv_tint: Newton's residual e0² relative, at most 2·e0² absolute for Y ≤ 2; then
            // a·r (10 ulps of a value below 2, times r ≤ 2), 1 − a·r (2 ulps of a value below
            // 2^−51, times r), r·(1 − a·r) (10 ulps of a value below 2^−50) and the final sum
            // (2 ulps of a value below 2).
            let e_y = two
                .mul(&e0)
                .mul(&e0)
                .add(&ulps(20, -191))
                .add(&p2(-240))
                .add(&ulps(10, -242))
                .add(&ulps(2, -191));
            // div_tint: r₀ = Y·b (10 ulps of a value below 2) is within e_r0 of b/a; one
            // Karp–Markstein step leaves (b − a·r₀)(1 − a·Y), at most e_r0·e_y, over a ≥ ½; then
            // a·r₀ (10 ulps, times Y ≤ 2), b − a·r₀ (2 ulps of a value below 2^−101, times Y),
            // Y·(b − a·r₀) (10 ulps of a value below 2^−100) and the final sum (2 ulps).
            let e_r0 = ulps(10, -191).add(&e_y);
            let total = two
                .mul(&e_r0)
                .mul(&e_y)
                .add(&ulps(20, -191))
                .add(&p2(-291))
                .add(&ulps(10, -292))
                .add(&ulps(2, -191));
            // relative to the smallest quotient, ½
            let rel = total.mul(&two);
            eprintln!(
                "division, {case}: reciprocal within 2^{:.3}, quotient relative 2^{:.4}",
                log2_approx(&e_y),
                log2_approx(&rel)
            );
            assert!(le(&e_y, &p2(-101)), "{case}: the reciprocal's error");
            assert!(le(&rel, &Dy::from_f64(EPS_DIV)), "{case}: ε₁ = ε₄");
        }
    }
}
