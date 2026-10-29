//! Correctly rounded `sinpi(x) = sin(πx)` and `cospi(x) = cos(πx)`: a port of CORE-MATH.
//!
//! Method: Ziv's strategy, as in CORE-MATH (<https://core-math.gitlabpages.inria.fr/>); the
//! algorithm and the proof of its bound are below.
//! Source: PORT from CORE-MATH `src/binary64/sinpi/sinpi.c` and `src/binary64/cospi/cospi.c` at
//!   commit `0ce0cbc15935e24c656280e5a46c34e89f301b7a`, by Alexei Sibidanov, MIT,
//!   <https://gitlab.inria.fr/core-math/core-math/-/blob/0ce0cbc15935e24c656280e5a46c34e89f301b7a/src/binary64/sinpi/sinpi.c>
//!   <https://gitlab.inria.fr/core-math/core-math/-/blob/0ce0cbc15935e24c656280e5a46c34e89f301b7a/src/binary64/cospi/cospi.c>
//!   Copyright (c) 2023-2026 Alexei Sibidanov <sibid@uvic.ca>.
//!   Changed for Spare Kit: translated to safe `no_std` Rust, with total shifts and table reads
//!   and the integer steps under the crate's rule (`lib.rs`); round-to-nearest only, without the
//!   exception flags or `errno`; the canonical NaN for NaN and infinite inputs; the main fast
//!   path's error constant raised from 2^−78 to `ER0 = 0x1.1p−78`, which the proof covers; a
//!   written proof of the error bound. Every change is listed under "Changes from the C source"
//!   below; none changes a correctly rounded result.
//! SPDX-License-Identifier: MPL-2.0 AND MIT
//!
//! The original notice of `sinpi.c`, reproduced as the MIT licence requires; `cospi.c` carries the
//! same notice and author:
//!
//! ```text
//! Copyright (c) 2023-2026 Alexei Sibidanov <sibid@uvic.ca>.
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
//! ```
//!
//! Guarantees: correctly rounded, resting on CORE-MATH's worst-case searches as step A of the
//! proof states. For every binary64 input the result is the binary64 number nearest the exact
//! `sin(πx)` or `cos(πx)` (ties to even), so the error is at most half an ulp and the result is
//! fixed bit for bit by the specification (IEEE 754-2019 §9.2 recommends correctly rounded `sinPi`
//! and `cosPi`). Proven outright: every input whose value has fewer than 44 identical bits after
//! its round bit, and every input in CORE-MATH's lists (which state that they hold every input
//! with 43 or more), is correctly rounded; had the lists' searches missed an input, its error
//! would still be below `½ + 2^−45.6` ulp. Domain: every finite `x`, with no limit on its size,
//! because the reduction modulo 2 is exact. `±∞` and NaN give the canonical NaN
//! (`0x7ff8_0000_0000_0000`). The proof is at the end of this documentation.
//!
//! **Exact at quarter turns.** At every multiple of ½ the result is exactly 0 or ±1, from a branch
//! that does no floating-point arithmetic. Signs of zero follow IEEE 754-2019 §9.2.1:
//! `sinpi(±0) = ±0`, `sinpi(n) = +0` for a positive integer `n` and `−0` for a negative one, and
//! `cospi(n + ½) = +0` for every integer `n`.
//!
//! Cost: constant time, no allocation and no recursion. Almost every argument takes the fast
//! path, a fixed sequence of floating-point operations and table reads; only when its rounding
//! test cannot decide does the accurate path run, also of fixed length, and a scan of a fixed list of hard cases when the rounding is in doubt. No loop's
//! length depends on the argument.
//!
//! The algorithm (Ziv's strategy, as CORE-MATH): `x` is split exactly into `iq/2^11` half-turns plus a
//! remainder `z·2^−75` half-turns, `|z| ≤ 2^63`. A fast path evaluates `sin` and `cos` of the
//! remainder by short polynomials and combines them with double-double values of `sin` and `cos`
//! at `π·iq/2^11` ([`sincosn`]), giving the result as `sh + r` with a bound `er` on the error. If
//! `(r − er) + sh` and `(r + er) + sh` round to the same double, that double is the correctly
//! rounded result, because rounding is monotone. Otherwise an accurate path recomputes in
//! double-double with longer polynomials and more accurate tables ([`sincosn2`]). Its result is
//! correctly rounded except for inputs closer to a rounding boundary than its error; those are
//! hard-to-round cases, which CORE-MATH found by exhaustive search (its `sinpi.wc` and `cospi.wc`
//! worst-case files) and lists below with their correctly rounded values. Near zero, `sinpi(x)` is
//! `πx` in double-double, and both functions have their own short polynomials.
//!
//! Changes from the C source, none of which changes a correctly rounded result:
//!
//! - **A proven fast-path bound.** CORE-MATH's main fast path accepts its result when it is within
//!   `|z|·2^−123 + 2^−78` of a decision boundary, but its table's own error reaches 2^−77.956 (at
//!   index 347 and its mirror images), so for small `|z|` that bound is not proven. This port uses
//!   `|z|·2^−123 + ER0` with `ER0 = 0x1.1p−78`, which the proof below covers. A larger bound only
//!   sends a few more inputs to the accurate path.
//! - Only round-to-nearest is implemented: Rust has no other rounding mode (RFC 3514). The C code's
//!   floating-point exception flags and `errno` are not kept.
//! - NaN and infinite inputs return the canonical NaN instead of `x + x`, whose payload and sign
//!   are not deterministic (chapter 21).
//! - `fma` is `libm::fma` ([`super::support::fma`]), correctly rounded like `__builtin_fma`. Rust
//!   never contracts other operations into fused ones; CORE-MATH supports and tests that
//!   configuration (it is what a C compiler does without FMA contraction).
//! - The accurate and near-zero paths are kept out of line (`#[cold]`, `#[inline(never)]`), so that
//!   the fast paths' code stays small.
//! - Shifts and table reads are total ([`super::support`]). The sticky adjustment at the end of
//!   the near-zero paths skips a low part that is exactly zero, where the C code's decrement of
//!   the bit pattern would make a NaN; the adjustment cannot change the sum in that case.
//! - The C code's signed and unsigned integer steps follow the crate's rule (`lib.rs`): exponent
//!   distances by `abs_diff` on the unsigned exponent field, the two's complement reading of a
//!   value without a cast ([`super::support::signed`]), checked steps whose `None` cannot come
//!   (each says why) and would give the canonical NaN, and wrapping ones only where the C keeps
//!   just the low bits of a sum or negates in two's complement. The same values result.
//!
//! # Proof of the error bound
//!
//! The lemmas cited are those of [`super`] (Lemma 1: rounding test; Lemma 2: worst cases; Lemma 3:
//! exact transformations; Lemma 4: double-double products). Numbers marked *checked* are computed
//! with exact arithmetic, exhaustively over the tables where they depend on a table, by the unit
//! test named in parentheses. `u = 2^−53`. No quantity bounded by a relative rounding error below is
//! subnormal: the smallest are `|z|·2^−123`, at least 2^−105 for `sinpi` and 2^−112 for `cospi`
//! unless `z = 0` (when every product with `z` is an exact zero), and products of `x ≥ 2^−54` with
//! constants.
//!
//! **Reduction (exact).** For `0x1.2p−5 < |x| < 2^41`, `x·2^11 ≡ iq + k·2^−64 (mod 4096)` with
//! `iq` the integer computed and `|k| ≤ 2^63`; the low `e − 1000 ≥ 18` bits of `k` are zero, so `k`
//! has at most 46 significant bits and `z = k` exactly. With `A = π·iq/2^11` and
//! `B = π·k·2^−75`, `|B| ≤ b_max = π·2^−12`, `sinpi(x) = sin(A + B)`; for `cospi`, on
//! `2^−12 < |x| < 2^41`, the same holds for `x + ½`, which the reduction adds exactly, except that
//! only the low `e − 1000 ≥ 11` bits of `k` are known to be zero: `k` has at most 53 significant
//! bits, so `z = k` is still exact.
//!
//! **L. Large arguments, `|x| ≥ 2^41`.** Multiples of ½ return exactly first. Otherwise `B = 0` and
//! the result is `RN(sh + sl)` for the fast table entry, with no test; for each of the 4094 entries
//! that can occur it is the correctly rounded sine [*checked*:
//! `large_arguments_are_correctly_rounded_exhaustively`].
//!
//! **F. The fast path.** (F1) The fast table [`sincosn`] gives `|sh + sl − sin A|` and
//! `|ch + cl − cos A|` at most `τ₁ = 0x1.08p−78`, the largest being 2^−77.956 at `iq = 347` and its
//! mirror images, with `|sl|, |cl| ≤ σ₁ = 2^−54` and `|sh|, |ch| ≤ 1` (*checked*: `table_errors`).
//! (F2) With `P_s(t) = SN0 + SN1·t² + SN2·t⁴` and `P_c(t) = CN0 + CN1·t²`, and `b = |B|`,
//! `|sin B − k·P_s(k)| ≤ ε_s(b) = s₁·b + s₃·b³ + s₅·b⁵ + b⁷/7!` and
//! `|cos B − 1 − k²·P_c(k)| ≤ ε_c(b) = c₂·b² + c₄·b⁴ + b⁶/6!`, where `s_j` and `c_j` are the
//! coefficients' differences from Taylor's, relative to `(π·2^−75)^j` (`s₁ ≈ 2^−54.5`)
//! (*checked*: `fast_path_bound`). (F3) Rounding errors: `z2 = RN(k²)`, the two Horner evaluations
//! and the products give `w = RN(z·fs) = k·P_s(k)·(1 + θ)` with `|θ| ≤ 2.0001u`, so
//! `p2 = RN(ch·w)` is within `ε_s + 3.0002u·(b + ε_s)` of `ch·sin B`; likewise
//! `p1 = RN(sh·RN(z2·fc))` is within `ε_c + 4.0002u·(b²/2 + ε_c)` of `sh·(cos B − 1)`;
//! `a = RN(sl + p1)` and `r = RN(a + p2)` add `u·|sl + p1|` and `u·(|a| + |p2|)`. Since
//! `sin(A + B) = sh + sl + sh·(cos B − 1) + ch·sin B + [sl·(cos B − 1) + cl·sin B − (sh + sl −
//! sin A)·cos B − (ch + cl − cos A)·sin B]`, the error of `sh + r` is at most
//! `E_F(b) = τ₁·(1 + b) + σ₁·(b + b²/2)` plus the four terms above. (F4) By Lemma 1 the test is
//! valid when `(1 − u)²·(|k|·2^−123 + ER0) ≥ E_F(b) + u·|r|`. Evaluated exactly at `b = 0` it reads
//! 2^−77.956 ≤ 2^−77.913, and at `b = b_max` 2^−60.80 ≤ 2^−60.00; the right side is convex in `b`
//! and the left linear, so it holds for every `b` (*checked*: `fast_path_bound`). With
//! CORE-MATH's 2^−78 in place of `ER0` it fails at `b = 0`.
//!
//! **Z. The near-zero fast paths.** (Z1) `sinpi`, `2^−54 ≤ |x| ≤ X1 = 0x1.2p−5`: `πx` is
//! `PH·x + PL·x`, the first product's error recovered exactly by `fma` (Lemma 3), with
//! `|π − PH − PL| ≤ 2^−108.04`, and `|sin πx − πx − x³·Pz(x²)| ≤ 2^−49.197·|x|³` (subdivision of
//! `[0, X1²]` with Taylor terms to degree 15 and the remainder) [*checked*:
//! `near_zero_fast_path_bounds`]. (Z2) The evaluation's rounding errors are bounded term by term
//! with `γ_n = nu/(1 − nu)` (Higham, *Accuracy and Stability of Numerical Algorithms*, §3.1); the
//! total plus Lemma 1's `u·|zl|` is `|x|·L + |x|³·C` with constants `L` and `C`, affine in `x²`,
//! and is below `|eps|·(1 − u)` at `x² = 0` (`L ≤ 2^−102·(1 − u)³`) and at `x² = X1²`
//! (2^−57.01 ≤ 2^−56.66, per unit of `|x|`) (*checked*). (Z3) `cospi`,
//! `X0 = 0x1.ccf6429be6621p−29 < |x| ≤ 2^−12`: the polynomial error is at most 2^−48.77·x², and the
//! total with rounding is 2^−47.56·x² ≤ `eps·(1 − u)` = 2^−47.30·x². Below `X0`, `(πx)² ≤ 2^−53`, so
//! `1 − 2^−54 < cos πx ≤ 1` and 1 is the correctly rounded result (*checked*).
//!
//! **A. The accurate paths and the tiny path** have no rounding test. Their errors (A1 to A6) are
//! at most `2^−98.64·|v|`, below `2^−45.6·ulp(v)`, so by Lemma 2 they round correctly every input
//! whose value has fewer than 44 identical bits after its round bit, unconditionally; an input
//! with more that the lists below missed would still be within `½ + 2^−45.6` ulp. CORE-MATH's
//! lists, from its `BaCSeL` searches, state that they hold every input with at least `t = 43`
//! identical bits: for `sinpi` (sinpi.wc) the subnormal range (set (a)), `[2^−1022, 2^−1021)`
//! (set (b)) with its images under every power of two up to `[2^−54, 2^−53)`, and `[2^−53, 1)`
//! (set (c)), extended to `[1, 2^52]` by translation by integers; for `cospi` (cospi.wc) `[X0, 1)`
//! and its translations. Every listed input is correctly rounded, with both signs: the 74 271
//! `sinpi` and 51 558 `cospi` inputs of the files (148 542 and 103 116 checks) and, for `sinpi`,
//! all 991 232 images of set (b) (`tests/math_full_worst_cases.rs`). For inputs not listed, the
//! argument rests on those searches being complete, as the files state. By Lemma 2 a path is then
//! correctly rounded if its error is below `2^−44·ulp(v)`, for which `2^−97·|v|` suffices.
//! (A1) The accurate table [`sincosn2`]: its errors are measured per index, and its low parts are
//! within `24u` of its high parts (*checked*: `table_errors`). (A2, A3) Refine: the sine and
//! cosine polynomials of the remainder differ from `sin B` and `cos B − 1` by at most
//! `ε_rs = 2^−109.43` and `ε_rc = 2^−122.53` over `|x| ≤ 1`; their double-double evaluations add at
//! most `12.1u²·b` and `22u²·b²/2` (`poly_dd`, `mul_dd_d`, Lemma 4); the two products obey Lemma 4
//! with this index's measured low-part ratios; both fast two-sums are exact (their order is checked
//! per index); and the final low-part additions are bounded by `u` times their magnitudes. Divided
//! by the smallest `|sin(A ± b)|` on the reduced interval, the total is at most 2^−98.64 for every
//! index, and 2^−98.94 for `iq` = 0 and 2048, where `v = ±sin B` (*checked*: `refine_bound`).
//! (A4) `sinpi_near_zero`: relative error at most 2^−105.97. (A5) `cospi_near_zero`: absolute error
//! at most 2^−103 on values near 1. (A6) The tiny path, `|x| < 2^−54`: relative error at most
//! 2^−104.74; in its scaled branch for `|x| < 2^−960` the final fused operation rounds once at the
//! subnormal precision and adds at most 2^−1127, below `2^−44·2^−1074` [*checked*:
//! `near_zero_accurate_bounds`]. (A7) The images of set (b). The file lists the hard cases of
//! `πx`, which scale exactly by powers of two, but `sinpi(x)` differs from `πx` by up to
//! `(πx)³/6`. For `|x| < 2^−53` that is less than `δ = (π·2^−53)²/6·2^53/(1 − (π·2^−53)²/6)`, below
//! 2^−52.28, units in the last place of `v = sin(πx)` (as `ulp(v) > 2^−53·|v|`), provided `πx` and
//! `v` lie in the same binade. They do: for every binade boundary `2^k ≥ 2^−1021`, the double `x₀`
//! nearest above `2^k/π` has `πx₀ − 2^k > (πx₀)³/6`, and below `2^−1021` the last place is
//! `2^−1074` on both sides of a boundary. So for an input outside the images, `πx` is at least
//! `2^−44` units from every midpoint and `v` at least `2^−44 − δ`, while the near-zero accurate
//! path and the tiny path err by less than `2^−104·|v| < 2^−51·ulp(v)`, which is less; so, by the
//! argument of Lemma 2, they round it correctly [*checked*: `scaled_worst_cases_near_zero`]. ∎

use super::hexf::hf;
use super::support::{
    entry, fast_two_sum, fma, from_checked_bits, sar, shl, shr, signed, unsigned,
};

/// The 52 significand bits of a binary64 number.
const MANTISSA: u64 = (1 << 52) - 1;

/// Double-double sum `(xh + xl) + (ch + cl)` (C: `adddd`), the high part by an exact two-sum.
#[inline]
fn add_dd(xh: f64, xl: f64, ch: f64, cl: f64) -> (f64, f64) {
    let s = xh + ch;
    let d = s - xh;
    let l = ((ch - d) + (xh + (d - s))) + (xl + cl);
    (s, l)
}

/// Double-double product `(xh + xl) · (ch + cl)` (C: `muldd_acc`).
#[inline]
#[allow(
    clippy::similar_names,
    reason = "the names of the C source's variables, so the two can be read side by side"
)]
fn mul_dd(xh: f64, xl: f64, ch: f64, cl: f64) -> (f64, f64) {
    let ahlh = ch * xl;
    let alhh = cl * xh;
    let ahhh = ch * xh;
    let mut ahhl = fma(ch, xh, -ahhh);
    ahhl += alhh + ahlh;
    fast_two_sum(ahhh, ahhl)
}

/// Double-double times double, `(xh + xl) · ch` (C: `mulddd`).
#[inline]
#[allow(
    clippy::similar_names,
    reason = "the names of the C source's variables, so the two can be read side by side"
)]
fn mul_dd_d(xh: f64, xl: f64, ch: f64) -> (f64, f64) {
    let ahlh = ch * xl;
    let ahhh = ch * xh;
    let mut ahhl = fma(ch, xh, -ahhh);
    ahhl += ahlh;
    let h = ahhh + ahhl;
    let l = (ahhh - h) + ahhl;
    (h, l)
}

/// Horner evaluation in double-double of `Σ c[i]·x^i` at `x = xh + xl`, with `l` added to the
/// leading coefficient's low part (C: `polydd`). The coefficient list is never empty; an empty
/// one is treated as the zero polynomial.
fn poly_dd(xh: f64, xl: f64, c: &[[f64; 2]], l: f64) -> (f64, f64) {
    let Some((last, rest)) = c.split_last() else {
        return (l, 0.0);
    };
    let mut ch = last[0] + l;
    let mut cl = ((last[0] - ch) + l) + last[1];
    for ci in rest.iter().rev() {
        (ch, cl) = mul_dd(xh, xl, ch, cl);
        let th = ch + ci[0];
        let tl = (ci[0] - th) + ch;
        ch = th;
        cl += tl + ci[1];
    }
    (ch, cl)
}

/// If `y1` is a power of two, moves it one unit towards the sign of `y2`, so that `y0 + y1` rounds
/// as the full sum `y0 + y1 + y2` would (`y2` then acts as a sticky bit). A zero `y1` is left
/// alone: there the C code's decrement of the bit pattern would make a NaN, and the adjustment
/// cannot change `y0 + y1` anyway, because both callers have `|y0| ≥ 2^−54`.
#[inline]
fn sticky(y1: f64, y2: f64) -> f64 {
    let t = y1.to_bits();
    if t & MANTISSA == 0 && (t << 1) != 0 {
        let w = y2.to_bits();
        // C: `t ± 1` on the bit pattern, the neighbouring double towards zero (when the signs of
        // y1 and y2 differ) or away from it. `t` has 52 trailing zero bits and is not zero, so
        // neither checked step can overflow.
        let nudged = if (w ^ t) >> 63 != 0 {
            t.checked_sub(1)
        } else {
            t.checked_add(1)
        };
        from_checked_bits(nudged)
    } else {
        y1
    }
}

/// `sinpi(x)` for `2^−54 ≤ |x| ≤ 0x1.2p−5` when the fast path's rounding test failed
/// (C: `as_sinpi_zero`): `πx` plus an odd degree-9 correction, in double-double.
#[cold]
#[inline(never)]
fn sinpi_near_zero(x: f64) -> f64 {
    #[rustfmt::skip]
    const CH: [[f64; 2]; 4] = [
        [hf!("-0x1.4abbce625be53p+2"), hf!("0x1.05511c68477bep-52")],
        [hf!("0x1.466bc6775aae2p+1"), hf!("-0x1.6dc0cbefae1dap-54")],
        [hf!("-0x1.32d2cce62bd86p-1"), hf!("0x1.066bd54973829p-55")],
        [hf!("0x1.50783487ee781p-4"), hf!("0x1.832989f39a743p-58")],
    ];
    #[rustfmt::skip]
    const CL: [f64; 3] = [hf!("-0x1.e3074fde861fp-8"), hf!("0x1.e8f4344534da6p-12"), hf!("-0x1.6f9cd7b8cb9dbp-16")];
    // π ≈ PI0 + PI1 + PI2; PI0 has 8 significant bits.
    const PI0: f64 = hf!("0x1.92p+1");
    const PI1: f64 = hf!("0x1.fb54442d1846ap-11");
    const PI2: f64 = hf!("-0x1.d9cceba3f91f2p-65");

    let x2 = x * x;
    let dx2 = fma(x, x, -x2);
    let x3 = x2 * x;
    let dx3 = fma(x2, x, -x3) + dx2 * x;
    let fl = x2 * (CL[0] + x2 * (CL[1] + x2 * CL[2]));
    let (fh, fl) = poly_dd(x2, dx2, &CH, fl);
    let (fh, fl) = mul_dd(fh, fl, x3, dx3);

    let mut y0 = PI0 * x;
    // As in the C source: add and subtract a constant made of y0's significand bits under the
    // exponent field 42 (about 2^−981). Here |y0| ≥ 2^−53, so the constant is far below half an
    // ulp of y0 and y0 is unchanged; y0l below is the exact error of PI0·x either way. The C adds
    // the field to the bits (a carry from bit 51 is possible); the sum is below 2^52 + 85·2^51 <
    // 2^59, so the checked addition cannot overflow.
    let b = from_checked_bits((y0.to_bits() & MANTISSA).checked_add(85u64 << 51));
    y0 = (y0 + b) - b;
    let y0l = fma(PI0, x, -y0);
    let y1 = PI1 * x;
    let y2 = fma(PI1, x, -y1) + PI2 * x;
    let (y1, y2) = add_dd(y1, y2, y0l, 0.0);
    let (y1, y2) = add_dd(y1, y2, fh, fl);
    let (y0, y1) = fast_two_sum(y0, y1);
    let (y1, y2) = fast_two_sum(y1, y2);
    y0 + sticky(y1, y2)
}

/// `cospi(x)` for `2^−29 < |x| ≤ 2^−12` when the fast path's rounding test failed
/// (C: `as_cospi_zero`): `1 + x²·P(x²)` in double-double.
#[cold]
#[inline(never)]
fn cospi_near_zero(x: f64) -> f64 {
    #[rustfmt::skip]
    const CH: [[f64; 2]; 2] = [
        [hf!("-0x1.3bd3cc9be45dep+2"), hf!("-0x1.692b71366cc04p-52")],
        [hf!("0x1.03c1f081b5ac4p+2"), hf!("-0x1.32b33fda9113cp-52")],
    ];
    const CL: [f64; 2] = [hf!("-0x1.55d3c7e3cbff9p+0"), hf!("0x1.e1f50604fa0ffp-3")];

    let x2 = x * x;
    let dx2 = fma(x, x, -x2);
    let fl = x2 * (CL[0] + x2 * CL[1]);
    let (fh, fl) = poly_dd(x2, dx2, &CH, fl);
    let (fh, fl) = mul_dd(x2, dx2, fh, fl);
    let (y0, y1) = fast_two_sum(1.0, fh);
    let (y1, y2) = fast_two_sum(y1, fl);
    y0 + sticky(y1, y2)
}

/// A hard-to-round input of the accurate path and its correctly rounded result.
#[derive(Clone, Copy)]
struct HardCase {
    /// Table index of the input, `0 ≤ iq ≤ 2048` after folding.
    iq: u64,
    /// The remainder `z·2^−63` of the input.
    x: f64,
    /// The nearest double to the result.
    r: f64,
    /// A tiny offset on the side of the exact value, so that `r + d` rounds to `r`.
    d: f64,
}

/// The accurate path for `|x| > 0x1.2p−5` (sinpi) or `|x| > 2^−12` (cospi):
/// `sin(π·iq/2^11 + π·z·2^−75)` in double-double (C: `as_sinpi_refine` and `as_cospi_refine`,
/// identical apart from their lists of hard cases).
#[cold]
#[inline(never)]
fn refine(iq: u64, z: f64, hard: &[HardCase]) -> f64 {
    #[rustfmt::skip]
    const SH: [[f64; 2]; 3] = [
        [hf!("0x1.921fb54442d18p+1"), hf!("0x1.1a62633145c06p-53")],
        [hf!("-0x1.4abbce625be53p-22"), hf!("0x1.05511cbc65743p-76")],
        [hf!("0x1.466bc6775aae1p-47"), hf!("-0x1.9c3c168d990ap-114")],
    ];
    #[rustfmt::skip]
    const CH: [[f64; 2]; 2] = [
        [hf!("-0x1.3bd3cc9be45dep-22"), hf!("-0x1.692b71366cc04p-76")],
        [hf!("0x1.03c1f081b5ac4p-46"), hf!("-0x1.32b33fda9113cp-100")],
    ];
    const S3L: f64 = hf!("-0x1.32d2cc920dcb4p-73");
    const C2L: f64 = hf!("-0x1.55d3c7e3cbff9p-72");
    const C3L: f64 = hf!("0x1.e1f50604fa0ffp-99");

    let x = z * hf!("0x1p-63");
    let x2 = x * x;
    let dx2 = fma(x, x, -x2);
    let sll = S3L * x2;
    let (slh, sll) = poly_dd(x2, dx2, &SH, sll);
    let (slh, sll) = mul_dd_d(slh, sll, x * hf!("0x1p-12"));
    let cll = x2 * (C2L + C3L * x2);
    let (clh, cll) = poly_dd(x2, dx2, &CH, cll);
    let (clh, cll) = mul_dd(clh, cll, x2, dx2);
    let (sbh, sbl, cbh, cbl) = sincosn2(iq);
    let (csh, csl) = mul_dd(clh, cll, sbh, sbl);
    let (sch, scl) = mul_dd(slh, sll, cbh, cbl);
    let (tsh, mut tsl) = fast_two_sum(sch, csh);
    tsl += csl + scl;
    let (tsh, tsl2) = fast_two_sum(sbh, tsh);
    let tsl = sbl + tsl + tsl2;
    // The rounding of tsh + tsl can only be in doubt when the low part's significand is all ones
    // or all zeros; then the input may be one of the listed hard cases.
    let t = tsl.to_bits();
    if (t | (0xfff << 52)) == u64::MAX || (t << 12) == 0 {
        let sgn = if iq > 2048 { -1.0 } else { 1.0 };
        let iq = iq & 0x7ff;
        for h in hard {
            // At most one of the two conditions can hold.
            // C: `iq == 2048 − h.iq`; every listed index is below 2048.
            if (x == h.x && iq == h.iq) || (x == -h.x && 2048u64.checked_sub(h.iq) == Some(iq)) {
                return sgn * h.r + sgn * h.d;
            }
        }
    }
    tsh + tsl
}

/// Hard cases of `sinpi`'s accurate path (C: `db` in `as_sinpi_refine`).
#[rustfmt::skip]
const SINPI_HARD: [HardCase; 4] = [
    HardCase { iq: 76, x: hf!("-0x1.276b3fef466p-2"), r: hf!("0x1.db8a79a80c3a0p-4"), d: hf!("0x1p-110") },
    HardCase { iq: 108, x: hf!("-0x1.33caea0f24cp-2"), r: hf!("0x1.5146c0bc45bcep-3"), d: hf!("0x1p-109") },
    HardCase { iq: 490, x: hf!("-0x1.8a1e8a3e82cp-1"), r: hf!("0x1.5d6561936b699p-1"), d: hf!("-0x1p-55") },
    HardCase { iq: 903, x: hf!("-0x1.bdd02d1ad60p-2"), r: hf!("0x1.f72c906962631p-1"), d: hf!("0x1p-55") },
];

/// Hard cases of `cospi`'s accurate path (C: `db` in `as_cospi_refine`).
#[rustfmt::skip]
const COSPI_HARD: [HardCase; 8] = [
    HardCase { iq: 903, x: hf!("-0x1.bdd02d1ad60p-2"), r: hf!("0x1.f72c906962631p-1"), d: hf!("0x1p-55") },
    HardCase { iq: 1029, x: hf!("-0x1.a4ad070549dp-3"), r: hf!("0x1.fffc4d2c6ca51p-1"), d: hf!("0x1p-55") },
    HardCase { iq: 1078, x: hf!("0x1.fbdb79ca3dap-3"), r: hf!("0x1.fe3c8219054c3p-1"), d: hf!("0x1p-55") },
    HardCase { iq: 1217, x: hf!("0x1.c0ccee4ada2p-1"), r: hf!("0x1.e99fd53791bcfp-1"), d: hf!("-0x1p-55") },
    HardCase { iq: 1025, x: hf!("0x1.b536647b1fe98p-2"), r: hf!("0x1.ffffc5ddd0738p-1"), d: hf!("0x1p-55") },
    HardCase { iq: 1026, x: hf!("-0x1.dc93eaad12a18p-2"), r: hf!("0x1.ffff84b21c731p-1"), d: hf!("0x1p-55") },
    HardCase { iq: 1033, x: hf!("0x1.e78f0e592b36p-1"), r: hf!("0x1.fff2270422604p-1"), d: hf!("-0x1p-55") },
    HardCase { iq: 1235, x: hf!("0x1.f13412a48d8p-2"), r: hf!("0x1.e55a7fa9a24c4p-1"), d: hf!("-0x1p-55") },
];

/// The fast path's error bound is `|z|·2^−123 + ER0`. CORE-MATH uses 2^−78 for this constant,
/// but the fast table's error reaches 2^−77.956 at index 347 (and its mirror images), so that
/// bound is not proven at small `|z|`; `ER0` covers the measured table error with margin (see the
/// proof, step F4). A larger bound sends a few more inputs to the accurate path; it never changes
/// a correctly rounded result.
const ER0: f64 = hf!("0x1.1p-78");
/// The fast table's largest error, τ₁ (measured exhaustively: 2^−77.956).
#[cfg(test)]
const TAU1: f64 = hf!("0x1.08p-78");
/// The fast table's largest low part, σ₁ (measured exhaustively).
#[cfg(test)]
const SIGMA1: f64 = hf!("0x1p-54");

/// Fast path: `sin(π·z·2^−75) ≈ z·(SN[0] + SN[1]·z² + SN[2]·z⁴)` (C: `sn`).
#[rustfmt::skip]
const SN: [f64; 3] = [hf!("0x1.921fb54442d18p-74"), hf!("-0x1.4abbce625be51p-223"), hf!("0x1.466bc6044ba16p-374")];
/// Fast path: `cos(π·z·2^−75) − 1 ≈ z²·(CN[0] + CN[1]·z²)` (C: `cn`).
const CN: [f64; 2] = [
    hf!("-0x1.3bd3cc9be45dbp-148"),
    hf!("0x1.03c1f00186416p-298"),
];

/// Sine of an angle given in half-turns: `sinpi(x) = sin(πx)`, correctly rounded.
///
/// Error at most half an ulp for every input; see the module documentation for the method, the
/// source and the proof. Exactly 0 or ±1 at every multiple of ½: `sinpi(n) = ±0` with the sign
/// of `n` (IEEE 754-2019 §9.2.1), `sinpi(2n + ½) = 1`, `sinpi(2n − ½) = −1`, `sinpi(±0) = ±0`.
/// `±∞` and NaN give the canonical NaN, `0x7ff8_0000_0000_0000`.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the C source's variables, so the two can be read side by side"
)]
pub fn sinpi(x: f64) -> f64 {
    /// π ≈ PH + PL, for the near-zero paths.
    const PH: f64 = hf!("0x1.921fb54442d18p+1");
    /// See [`PH`].
    const PL: f64 = hf!("0x1.1a62633145c07p-53");
    /// The near-zero fast path's polynomial coefficients, as the C source writes them.
    #[rustfmt::skip]
    const C: [f64; 4] = [
        hf!("-0x1.4abbce625be51p+2"), hf!("0x1.466bc67754b46p+1"),
        hf!("-0x1.32d2cc12a51f4p-1"), hf!("0x1.5060540058476p-4"),
    ];
    let ix = x.to_bits();
    let ax = ix & (u64::MAX >> 1);
    if ax == 0 {
        return x;
    }
    let e = ax >> 52; // the exponent field: 2^(e−1023) ≤ |x| < 2^(e−1022)
    let m0 = (ix & MANTISSA) | (1 << 52); // |x| = m0·2^(e−1075)
    // C: `m = (m0 ^ sgn) − sgn` with `sgn` = 0 or −1, ±m0 as an int64_t, which the shifts below
    // read as a uint64_t: m0, or its two's complement negation modulo 2^64. m0 < 2^53, so that is
    // the C's value exactly.
    let m = if ix >> 63 == 0 { m0 } else { m0.wrapping_neg() };
    if e >= 1064 {
        // |x| ≥ 2^41
        if e == 0x7ff {
            return super::NAN; // ±∞ (invalid) or NaN
        }
        let s = e.abs_diff(1064); // e − 1064: 2^(41+s) ≤ |x| < 2^(42+s)
        if s > 10 {
            return 0.0f64.copysign(x); // |x| ≥ 2^52: an integer
        }
        // C: `(uint64_t) m << s`, x·2^11 modulo 2^64 in two's complement; only its low 12 bits
        // are read.
        let iq = shl(m, s);
        if iq.trailing_zeros() >= 11 {
            return 0.0f64.copysign(x); // an integer
        }
        let (sh, sl, _, _) = sincosn(iq);
        return sh + sl;
    }
    // Here e ≤ 1063: s = 1063 − e ≥ 0, and 2^(40−s) ≤ |x| < 2^(41−s).
    let s = 1063u64.abs_diff(e);

    if ax <= 0x3fa2_0000_0000_0000 {
        // |x| ≤ 0x1.2p−5
        if x.abs() < hf!("0x1p-54") {
            if x.abs() < hf!("0x1p-960") {
                // Scale up so that the low part of πx does not underflow, then round once into the
                // (possibly subnormal) result with a fused operation.
                let t = x * hf!("0x1p106");
                let zh = PH * t;
                let zl = fma(PH, t, -zh) + PL * t;
                let r = zh + zl;
                let rs = r * hf!("0x1p-106");
                let rt = rs * hf!("0x1p106");
                return fma((zh - rt) + zl, hf!("0x1p-106"), rs);
            }
            let zh = PH * x;
            let zl = fma(PH, x, -zh) + PL * x;
            return zh + zl;
        }
        let zh = PH * x;
        let mut zl = fma(PH, x, -zh) + PL * x;
        let x2 = x * x;
        let x3 = x2 * x;
        let x4 = x2 * x2;
        let eps = x * (x2 * hf!("0x1p-47") + hf!("0x1p-102"));
        zl += x3 * ((C[0] + x2 * C[1]) + x4 * (C[2] + x2 * C[3]));
        let lb = (zl - eps) + zh;
        let ub = (zl + eps) + zh;
        if lb == ub {
            return lb;
        }
        return sinpi_near_zero(x);
    }

    // Here 0x1.2p−5 < |x| < 2^41, so 1018 ≤ e ≤ 1063, 0 ≤ s ≤ 45 and 7 ≤ si ≤ 52.
    let si = e.abs_diff(1011); // e − 1011
    // C: `m0 << (si + 1)`: the shift by si, then by one more (si + 1 ≤ 53).
    if shl(m0, si) << 1 == 0 {
        // x is an integer or a half-integer
        if shl(m0, si) == 0 {
            return 0.0f64.copysign(x); // an integer
        }
        // t = 0 if |x| = ½ mod 2, 1 if |x| = 3/2 mod 2 (C: `m0 << (si − 1)`; si ≥ 7, so the
        // difference is si − 1).
        let t = shr(shl(m0, si.abs_diff(1)), 63);
        return if t == 0 {
            1.0f64.copysign(x)
        } else {
            -(1.0f64.copysign(x))
        };
    }

    // iq = round(x·2^11) mod 2^12 and z/2^64 = x·2^11 − iq exactly, |z| ≤ 2^63. C: `(m >> s) &
    // 8191` with m an int64_t (an arithmetic shift), then `(iq + 1) >> 1`, which is ⌈iq/2⌉.
    let iq = unsigned(sar(signed(m), s)) & 8191;
    let iq = iq.div_ceil(2);
    // C: `(int64_t)((uint64_t) m << (e − 1000))`, the bits below x·2^11's integer part, with
    // 18 ≤ e − 1000 ≤ 63 here.
    let k = signed(shl(m, e.abs_diff(1000)));
    #[allow(
        clippy::cast_precision_loss,
        reason = "k has at most 46 significant bits (proof, reduction), so the conversion is exact"
    )]
    let z = k as f64;
    let z2 = z * z;
    let fs = SN[0] + z2 * (SN[1] + z2 * SN[2]);
    let fc = CN[0] + z2 * CN[1];
    let (sh, sl, ch, _) = sincosn(iq);
    let er = z.abs() * hf!("0x1p-123") + ER0;
    let r = sl + sh * (z2 * fc) + ch * (z * fs);
    let lb = (r - er) + sh;
    let ub = (r + er) + sh;
    if lb == ub {
        return lb;
    }
    refine(iq, z, &SINPI_HARD)
}

/// Cosine of an angle given in half-turns: `cospi(x) = cos(πx)`, correctly rounded.
///
/// Error at most half an ulp for every input; see the module documentation. Exactly 0 or ±1 at
/// every multiple of ½: `cospi(2n) = 1`, `cospi(2n + 1) = −1`, `cospi(n + ½) = +0`,
/// `cospi(±0) = 1`. `±∞` and NaN give the canonical NaN, `0x7ff8_0000_0000_0000`.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the C source's variables, so the two can be read side by side"
)]
pub fn cospi(x: f64) -> f64 {
    /// The near-zero fast path's polynomial coefficients, as the C source writes them.
    #[rustfmt::skip]
    const C: [f64; 4] = [
        hf!("-0x1.3bd3cc9be45dcp+2"), hf!("0x1.03c1f081b0833p+2"),
        hf!("-0x1.55d3c6fc9af15p+0"), hf!("0x1.e1d3ff2ae3f9ap-3"),
    ];
    let ix = x.to_bits();
    let ax = ix & (u64::MAX >> 1);
    if ax == 0 {
        return 1.0;
    }
    let e = ax >> 52; // the exponent field: 2^(e−1023) ≤ |x| < 2^(e−1022)
    let m = (ix & MANTISSA) | (1 << 52); // |x| = m·2^(e−1075)
    if e >= 1064 {
        // |x| ≥ 2^41
        if e == 0x7ff {
            return super::NAN; // ±∞ (invalid) or NaN
        }
        let s = e.abs_diff(1064); // e − 1064: 2^(41+s) ≤ |x| < 2^(42+s)
        if s > 11 {
            return 1.0; // |x| ≥ 2^53: an even integer
        }
        // cos(πx) = sin(π(x + ½)): add a quarter turn, 2^10 units of π/2^11. C: unsigned
        // arithmetic, whose low 12 bits are all that is read; m·2^s ≤ 2^64 − 2^11 for s ≤ 11, so
        // the sum does not even wrap.
        let iq = shl(m, s).wrapping_add(1024);
        if iq.trailing_zeros() >= 11 {
            return 0.0; // a half-integer
        }
        let (sh, sl, _, _) = sincosn(iq);
        return sh + sl;
    }
    // Here e ≤ 1063: s = 1063 − e ≥ 0, and 2^(40−s) ≤ |x| < 2^(41−s).
    let s = 1063u64.abs_diff(e);
    if ax <= 0x3f30_0000_0000_0000 {
        // |x| ≤ 2^−12
        if ax <= 0x3e2c_cf64_29be_6621 {
            return 1.0 - hf!("0x1p-55"); // rounds to 1
        }
        let x2 = x * x;
        let x4 = x2 * x2;
        let eps = x2 * hf!("0x1.ap-48");
        let p = x2 * ((C[0] + x2 * C[1]) + x4 * (C[2] + x2 * C[3]));
        let lb = (p - eps) + 1.0;
        let ub = (p + eps) + 1.0;
        if lb == ub {
            return lb;
        }
        return cospi_near_zero(x);
    }

    // Here 2^−12 < |x| < 2^41, so 1011 ≤ e ≤ 1063, 0 ≤ s ≤ 52 and 0 ≤ si ≤ 52.
    let si = e.abs_diff(1011); // e − 1011
    // C: `m << (si + 1)`: the shift by si, then by one more (si + 1 ≤ 53).
    if shl(m, si) << 1 == 0 {
        // x is an integer or a half-integer
        if shl(m, si) == 0 {
            // x is an integer, which needs si ≥ 12, so the difference si − 1 is exact.
            // t = 0 if |x| = 0 mod 2, 1 if |x| = 1 mod 2
            let t = shr(shl(m, si.abs_diff(1)), 63);
            return if t != 0 { -1.0 } else { 1.0 };
        }
        return 0.0;
    }

    // As in sinpi, for x + ½. C: `((m >> s) + 2048) & 8191`, unsigned arithmetic of which only the
    // low 13 bits are kept; m >> s < 2^53, so the sum does not even wrap. Then ⌈iq/2⌉.
    let iq = shr(m, s).wrapping_add(2048) & 8191;
    let iq = iq.div_ceil(2);
    // C: `(int64_t)(m << (e − 1000))`, with 11 ≤ e − 1000 ≤ 63 here.
    let k = signed(shl(m, e.abs_diff(1000)));
    #[allow(
        clippy::cast_precision_loss,
        reason = "k has at most 53 significant bits (proof, reduction), so the conversion is exact"
    )]
    let z = k as f64;
    let z2 = z * z;
    let fs = SN[0] + z2 * (SN[1] + z2 * SN[2]);
    let fc = CN[0] + z2 * CN[1];
    let (sh, sl, ch, _) = sincosn(iq);
    let er = z.abs() * hf!("0x1p-123") + ER0;
    let r = sl + sh * (z2 * fc) + ch * (z * fs);
    let lb = (r - er) + sh;
    let ub = (r + er) + sh;
    if lb == ub {
        return lb;
    }
    refine(iq, z, &COSPI_HARD)
}

/// Table indices and signs for the angle `π·s/2^11` (only the low 12 bits of `s` matter).
///
/// `s` is folded into the first quadrant: `j = s mod 2^10`, mirrored to `2^10 − j` in the second
/// and fourth quadrants, then split as `j = 32·is + jm`. Returns
/// `(is, ic, jm, sine_negative, cosine_negative)` with `ic = 32 − is`; since `0 ≤ j ≤ 2^10`,
/// `0 ≤ is, ic ≤ 32` and `0 ≤ jm < 32`, so every table read is in range.
#[inline]
fn fold(s: u64) -> (u64, u64, u64, bool, bool) {
    let s = s & 0xfff;
    let j = s & 0x3ff;
    // C: `1024 − j`, with j ≤ 1023.
    let j = if (s >> 10) & 1 != 0 {
        1024u64.abs_diff(j)
    } else {
        j
    };
    let is = j >> 5;
    let ic = 32u64.abs_diff(is); // 32 − is, with is ≤ 32
    let jm = j & 0x1f;
    let sine_negative = (s >> 11) & 1 != 0;
    // C: `((s + 1024) >> 11) & 1`. Adding 2^10 carries into bit 11 exactly when bit 10 is set, so
    // that bit is bit 11 of s exclusive-or bit 10.
    let cosine_negative = ((s >> 11) ^ (s >> 10)) & 1 != 0;
    (is, ic, jm, sine_negative, cosine_negative)
}

/// `sin` and `cos` of `π·s/2^11` as unevaluated sums `sh + sl` and `ch + cl`, for the fast path
/// (C: `sincosn`). Returns `(sh, sl, ch, cl)`.
///
/// With the folding of [`fold`], the angle is `π·is/64 + π·jm/2^11`: a coarse entry `SN1[is]`
/// (and `SN1[32 − is]` for the cosine) and fine entries `SM1[jm]` and `CM1[jm]`, combined by the
/// angle-sum formulas. Each table value is a short high part, so that products of high parts
/// are exact, plus a correction.
fn sincosn(s: u64) -> (f64, f64, f64, f64) {
    let (is, ic, jm, sine_negative, cosine_negative) = fold(s);
    let [sbh, sbl] = entry(&SN1, is);
    let [cbh, cbl] = entry(&SN1, ic);
    let [slh, sll] = entry(&SM1, jm);
    let [clh, cll] = entry(&CM1, jm);

    let sb = sbh + sbl;
    let cb = cbh + cbl;
    let ch = cbh * clh - sbh * slh;
    let cl = clh * cbl - slh * sbl + cb * cll - sb * sll;
    let sh = sbh * clh + cbh * slh;
    let sl = slh * cbl + clh * sbl + cb * sll + sb * cll;

    let tch = ch + cl;
    let tcl = (ch - tch) + cl;
    let tsh = sh + sl;
    let tsl = (sh - tsh) + sl;

    let cs = if cosine_negative { -1.0 } else { 1.0 };
    let ss = if sine_negative { -1.0 } else { 1.0 };
    (ss * tsh, ss * tsl, cs * tch, cs * tcl)
}

/// `sin` and `cos` of `π·s/2^11` in double-double, for the accurate path (C: `sincosn2`).
/// Returns `(sh, sl, ch, cl)`.
///
/// Same folding as [`sincosn`]; the fine tables hold `sin(π·jm/2^11)` and `1 − cos(π·jm/2^11)`
/// and the coarse table `sin(π·is/64)`, all in double-double.
#[allow(
    clippy::similar_names,
    reason = "the names of the C source's variables, so the two can be read side by side"
)]
fn sincosn2(s: u64) -> (f64, f64, f64, f64) {
    let (is, ic, jm, sine_negative, cosine_negative) = fold(s);
    let [sbh, sbl] = entry(&SN2, is);
    let [cbh, cbl] = entry(&SN2, ic);
    let [slh, sll] = entry(&SM2, jm);
    let [clh, cll] = entry(&CM2, jm);

    let (cch, ccl) = mul_dd(clh, cll, cbh, cbl);
    let (ssh, ssl) = mul_dd(slh, sll, sbh, sbl);
    let (csh, csl) = mul_dd(clh, cll, sbh, sbl);
    let (sch, scl) = mul_dd(slh, sll, cbh, cbl);

    let (tch, mut tcl) = fast_two_sum(ssh, cch);
    tcl += ccl + ssl;
    let (tsh, mut tsl) = fast_two_sum(-sch, csh);
    tsl += csl - scl;

    let (tch, tcl2) = fast_two_sum(cbh, -tch);
    let tcl = cbl - tcl + tcl2;
    let (tsh, tsl2) = fast_two_sum(sbh, -tsh);
    let tsl = sbl - tsl + tsl2;

    let cs = if cosine_negative { -1.0 } else { 1.0 };
    let ss = if sine_negative { -1.0 } else { 1.0 };
    (ss * tsh, ss * tsl, cs * tch, cs * tcl)
}

/// `sin(π·i/64)` for `0 ≤ i ≤ 32`: a short high part plus a correction (C: `Sn` in `sincosn`).
#[rustfmt::skip]
const SN1: [[f64; 2]; 33] = [
    [hf!("0x0p+0"), hf!("0x0p+0")],
    [hf!("0x1.91f66p-5"), hf!("-0x1.de44fd832257ap-30")],
    [hf!("0x1.917a6cp-4"), hf!("-0x1.eb25ea0f138c7p-31")],
    [hf!("0x1.2c8106p-3"), hf!("0x1.d1cc27444c003p-28")],
    [hf!("0x1.8f8b84p-3"), hf!("-0x1.cb2cfaa4da337p-30")],
    [hf!("0x1.f19f98p-3"), hf!("-0x1.37a839542deefp-29")],
    [hf!("0x1.294063p-2"), hf!("-0x1.2a60fa574a369p-30")],
    [hf!("0x1.58f9a7p-2"), hf!("0x1.6ac7f73f8409p-28")],
    [hf!("0x1.87de2ap-2"), hf!("0x1.abaa58b469891p-28")],
    [hf!("0x1.b5d101p-2"), hf!("-0x1.87a8cff5264eap-28")],
    [hf!("0x1.e2b5d4p-2"), hf!("-0x1.fe4271387c9dcp-28")],
    [hf!("0x1.0738798p-1"), hf!("0x1.22ffed9697fafp-29")],
    [hf!("0x1.1c73b38p-1"), hf!("0x1.ae68c86c9774ap-29")],
    [hf!("0x1.30ff8p-1"), hf!("-0x1.8f47e58f7e631p-28")],
    [hf!("0x1.44cf328p-1"), hf!("-0x1.7b7114f3fc4afp-28")],
    [hf!("0x1.57d6938p-1"), hf!("-0x1.b989b02eae413p-28")],
    [hf!("0x1.6a09e68p-1"), hf!("-0x1.80c4336f74d05p-29")],
    [hf!("0x1.7b5df2p-1"), hf!("0x1.3557d76f0ac85p-28")],
    [hf!("0x1.8bc8068p-1"), hf!("0x1.8a8ba05a743dap-28")],
    [hf!("0x1.9b3e048p-1"), hf!("-0x1.8f17e98771434p-34")],
    [hf!("0x1.a9b6628p-1"), hf!("0x1.0ea1a3033ec62p-29")],
    [hf!("0x1.b728348p-1"), hf!("-0x1.7348e1378d3e6p-28")],
    [hf!("0x1.c38b2fp-1"), hf!("0x1.80bdb0d23e9d1p-29")],
    [hf!("0x1.ced7af8p-1"), hf!("-0x1.e19c46879edafp-28")],
    [hf!("0x1.d906bdp-1"), hf!("-0x1.9ae573aea067cp-30")],
    [hf!("0x1.e212108p-1"), hf!("-0x1.84bc8da0298eep-28")],
    [hf!("0x1.e9f4158p-1"), hf!("-0x1.39d225a27d387p-29")],
    [hf!("0x1.f0a7ef8p-1"), hf!("0x1.c9186b952c7aep-28")],
    [hf!("0x1.f6297dp-1"), hf!("-0x1.1469faa77a357p-34")],
    [hf!("0x1.fa7558p-1"), hf!("-0x1.eeb5d2bd05465p-30")],
    [hf!("0x1.fd88dap-1"), hf!("0x1.e89292cf04139p-28")],
    [hf!("0x1.ff621ep-1"), hf!("0x1.bcb6bef1d421fp-28")],
    [hf!("0x1p+0"), hf!("0x0p+0")],
];

/// `sin(π·j/2^11)` for `0 ≤ j < 32`: a short high part plus a correction (C: `Sm` in `sincosn`).
#[rustfmt::skip]
const SM1: [[f64; 2]; 32] = [
    [hf!("0x0p+0"), hf!("0x0p+0")],
    [hf!("0x1.922p-10"), hf!("-0x1.54466e349ee53p-28")],
    [hf!("0x1.921f8p-9"), hf!("0x1.7d99497495d2p-30")],
    [hf!("0x1.2d978p-8"), hf!("0x1.17ccb5e27cb43p-31")],
    [hf!("0x1.921fp-8"), hf!("0x1.fcce00e23572dp-29")],
    [hf!("0x1.f6a64p-8"), hf!("0x1.f9a2a3c5885cfp-28")],
    [hf!("0x1.2d96cp-7"), hf!("-0x1.e35ed1fa23d22p-28")],
    [hf!("0x1.5fdap-7"), hf!("0x1.bd602f04014c8p-30")],
    [hf!("0x1.921d2p-7"), hf!("-0x1.909c3dccf0e28p-34")],
    [hf!("0x1.c46p-7"), hf!("-0x1.e1b7526ebf9f2p-31")],
    [hf!("0x1.f6a2ap-7"), hf!("-0x1.2a8cd06af94a2p-28")],
    [hf!("0x1.14727p-6"), hf!("0x1.b5ae26618769ep-31")],
    [hf!("0x1.2d937p-6"), hf!("-0x1.1073c40b25037p-28")],
    [hf!("0x1.46b44p-6"), hf!("-0x1.f80c5f9200607p-28")],
    [hf!("0x1.5fd4dp-6"), hf!("0x1.0fd5912ef3f57p-29")],
    [hf!("0x1.78f53p-6"), hf!("0x1.77727c10d8ca5p-28")],
    [hf!("0x1.92156p-6"), hf!("-0x1.0b933040d8eb2p-31")],
    [hf!("0x1.ab355p-6"), hf!("-0x1.3abec0d92ae48p-28")],
    [hf!("0x1.c454fp-6"), hf!("0x1.3394ec7229c28p-28")],
    [hf!("0x1.dd746p-6"), hf!("-0x1.ce6d5319d653ep-28")],
    [hf!("0x1.f6937p-6"), hf!("0x1.8e8e7807f600bp-29")],
    [hf!("0x1.07d92p-5"), hf!("-0x1.9eea00d71246dp-35")],
    [hf!("0x1.14686p-5"), hf!("-0x1.25e9f40a5121ap-28")],
    [hf!("0x1.20f77p-5"), hf!("0x1.9d6238d09f231p-30")],
    [hf!("0x1.2d8658p-5"), hf!("-0x1.4d75465d2f213p-30")],
    [hf!("0x1.3a151p-5"), hf!("-0x1.37bca0781d73ap-30")],
    [hf!("0x1.46a398p-5"), hf!("-0x1.0079e86eec954p-29")],
    [hf!("0x1.5331fp-5"), hf!("-0x1.e222f8a75de87p-28")],
    [hf!("0x1.5fc01p-5"), hf!("-0x1.6b7995e4bb32dp-28")],
    [hf!("0x1.6c4df8p-5"), hf!("-0x1.41523e139c56bp-32")],
    [hf!("0x1.78dba8p-5"), hf!("0x1.2c3a342eb5f11p-28")],
    [hf!("0x1.85692p-5"), hf!("0x1.5da89e0b235cp-28")],
];

/// `cos(π·j/2^11)` for `0 ≤ j < 32`: a short high part plus a correction (C: `Cm` in `sincosn`).
#[rustfmt::skip]
const CM1: [[f64; 2]; 32] = [
    [hf!("0x1p+0"), hf!("0x0p+0")],
    [hf!("0x1.ffffd88p-1"), hf!("0x1.61bb991af64f1p-31")],
    [hf!("0x1.ffff62p-1"), hf!("0x1.621d01d2a6063p-29")],
    [hf!("0x1.fffe9c8p-1"), hf!("0x1.8f17465de1773p-28")],
    [hf!("0x1.fffd888p-1"), hf!("-0x1.38bab6d94c71dp-28")],
    [hf!("0x1.fffc25p-1"), hf!("0x1.6bb5dd7625bcdp-30")],
    [hf!("0x1.fffa73p-1"), hf!("-0x1.b439d8a4596p-28")],
    [hf!("0x1.fff8718p-1"), hf!("0x1.37ce2eec7251bp-29")],
    [hf!("0x1.fff6218p-1"), hf!("-0x1.646d24a88970ep-29")],
    [hf!("0x1.fff3828p-1"), hf!("-0x1.9bb848bdb041ep-28")],
    [hf!("0x1.fff094p-1"), hf!("0x1.e29de85718cc2p-28")],
    [hf!("0x1.ffed57p-1"), hf!("0x1.cc695b5e89e49p-28")],
    [hf!("0x1.ffe9cb8p-1"), hf!("-0x1.da572f6a4bccap-28")],
    [hf!("0x1.ffe5f08p-1"), hf!("-0x1.0d43929b71f74p-28")],
    [hf!("0x1.ffe1c68p-1"), hf!("0x1.c32ddd89aa147p-31")],
    [hf!("0x1.ffdd4d8p-1"), hf!("0x1.fbc7a9242ccf3p-28")],
    [hf!("0x1.ffd886p-1"), hf!("0x1.099a19765595dp-30")],
    [hf!("0x1.ffd36f8p-1"), hf!("-0x1.dbaff3c93cdc4p-29")],
    [hf!("0x1.ffce0ap-1"), hf!("-0x1.8eacc3af7ac55p-28")],
    [hf!("0x1.ffc8558p-1"), hf!("-0x1.996f62d2df41dp-28")],
    [hf!("0x1.ffc252p-1"), hf!("-0x1.071603e8582dfp-28")],
    [hf!("0x1.ffbbff8p-1"), hf!("0x1.7e5454b8225e9p-31")],
    [hf!("0x1.ffb55e8p-1"), hf!("-0x1.ed0128d24b027p-28")],
    [hf!("0x1.ffae6ep-1"), hf!("0x1.5569984bd1a7ap-29")],
    [hf!("0x1.ffa72fp-1"), hf!("-0x1.08a362d33736dp-37")],
    [hf!("0x1.ff9fa1p-1"), hf!("0x1.a441ba9901fd5p-32")],
    [hf!("0x1.ff97c4p-1"), hf!("0x1.04600a0a95596p-28")],
    [hf!("0x1.ff8f988p-1"), hf!("-0x1.387d3a589fa3dp-28")],
    [hf!("0x1.ff871d8p-1"), hf!("0x1.6dc0ef98b1c67p-28")],
    [hf!("0x1.ff7e54p-1"), hf!("0x1.01907c4c59658p-28")],
    [hf!("0x1.ff753b8p-1"), hf!("0x1.8dc8b1e83ccffp-28")],
    [hf!("0x1.ff6bd48p-1"), hf!("-0x1.c4bbb2c34804p-29")],
];

/// `sin(π·i/64)` for `0 ≤ i ≤ 32` in double-double (C: `Sn` in `sincosn2`).
#[rustfmt::skip]
const SN2: [[f64; 2]; 33] = [
    [hf!("0x0p+0"), hf!("0x0p+0")],
    [hf!("0x1.91f65f10dd814p-5"), hf!("-0x1.912bd0d569a9p-61")],
    [hf!("0x1.917a6bc29b42cp-4"), hf!("-0x1.e2718d26ed688p-60")],
    [hf!("0x1.2c8106e8e613ap-3"), hf!("0x1.13000a89a11ep-58")],
    [hf!("0x1.8f8b83c69a60bp-3"), hf!("-0x1.26d19b9ff8d82p-57")],
    [hf!("0x1.f19f97b215f1bp-3"), hf!("-0x1.42deef11da2c4p-57")],
    [hf!("0x1.294062ed59f06p-2"), hf!("-0x1.5d28da2c4612dp-56")],
    [hf!("0x1.58f9a75ab1fddp-2"), hf!("-0x1.efdc0d58cf62p-62")],
    [hf!("0x1.87de2a6aea963p-2"), hf!("-0x1.72cedd3d5a61p-57")],
    [hf!("0x1.b5d1009e15ccp-2"), hf!("0x1.5b362cb974183p-57")],
    [hf!("0x1.e2b5d3806f63bp-2"), hf!("0x1.e0d891d3c6841p-58")],
    [hf!("0x1.073879922ffeep-1"), hf!("-0x1.a5a014347406cp-55")],
    [hf!("0x1.1c73b39ae68c8p-1"), hf!("0x1.b25dd267f66p-55")],
    [hf!("0x1.30ff7fce17035p-1"), hf!("-0x1.efcc626f74a6fp-57")],
    [hf!("0x1.44cf325091dd6p-1"), hf!("0x1.8076a2cfdc6b3p-57")],
    [hf!("0x1.57d69348cecap-1"), hf!("-0x1.75720992bfbb2p-55")],
    [hf!("0x1.6a09e667f3bcdp-1"), hf!("-0x1.bdd3413b26456p-55")],
    [hf!("0x1.7b5df226aafafp-1"), hf!("-0x1.0f537acdf0ad7p-56")],
    [hf!("0x1.8bc806b151741p-1"), hf!("-0x1.2c5e12ed1336dp-55")],
    [hf!("0x1.9b3e047f38741p-1"), hf!("-0x1.30ee286712474p-55")],
    [hf!("0x1.a9b66290ea1a3p-1"), hf!("0x1.9f630e8b6dac8p-60")],
    [hf!("0x1.b728345196e3ep-1"), hf!("-0x1.bc69f324e6d61p-55")],
    [hf!("0x1.c38b2f180bdb1p-1"), hf!("-0x1.6e0b1757c8d07p-56")],
    [hf!("0x1.ced7af43cc773p-1"), hf!("-0x1.e7b6bb5ab58aep-58")],
    [hf!("0x1.d906bcf328d46p-1"), hf!("0x1.457e610231ac2p-56")],
    [hf!("0x1.e212104f686e5p-1"), hf!("-0x1.014c76c126527p-55")],
    [hf!("0x1.e9f4156c62ddap-1"), hf!("0x1.760b1e2e3f81ep-55")],
    [hf!("0x1.f0a7efb9230d7p-1"), hf!("0x1.52c7adc6b4989p-56")],
    [hf!("0x1.f6297cff75cbp-1"), hf!("0x1.562172a361fd3p-56")],
    [hf!("0x1.fa7557f08a517p-1"), hf!("-0x1.7a0a8ca13571fp-55")],
    [hf!("0x1.fd88da3d12526p-1"), hf!("-0x1.87df6378811c7p-55")],
    [hf!("0x1.ff621e3796d7ep-1"), hf!("-0x1.c57bc2e24aa15p-57")],
    [hf!("0x1p+0"), hf!("0x0p+0")],
];

/// `sin(π·j/2^11)` for `0 ≤ j < 32` in double-double (C: `Sm` in `sincosn2`).
#[rustfmt::skip]
const SM2: [[f64; 2]; 32] = [
    [hf!("0x0p+0"), hf!("0x0p+0")],
    [hf!("0x1.921faaee6472ep-10"), hf!("-0x1.ee52e284a9df8p-64")],
    [hf!("0x1.921f8becca4bap-9"), hf!("0x1.2ba407bcab5b2p-63")],
    [hf!("0x1.2d97822f996bcp-8"), hf!("0x1.3e5a15ed6aa3ep-62")],
    [hf!("0x1.921f0fe670071p-8"), hf!("0x1.ab967fe6b7a9bp-64")],
    [hf!("0x1.f6a65f9a2a3c6p-8"), hf!("-0x1.de8c48783f3aep-62")],
    [hf!("0x1.2d96b0e509703p-7"), hf!("-0x1.1e9131ff52dc9p-63")],
    [hf!("0x1.5fda037ac05e1p-7"), hf!("-0x1.ff59bf4b574eep-61")],
    [hf!("0x1.921d1fcdec784p-7"), hf!("0x1.9878ebe836d9dp-61")],
    [hf!("0x1.c45ffe1e48ad9p-7"), hf!("0x1.4060e4bd32e79p-63")],
    [hf!("0x1.f6a296ab997cbp-7"), hf!("-0x1.f2943d8fe7033p-61")],
    [hf!("0x1.147270dad7133p-6"), hf!("0x1.8769e00e018p-63")],
    [hf!("0x1.2d936bbe30efdp-6"), hf!("0x1.b5f91ee371d64p-61")],
    [hf!("0x1.46b4381fce81bp-6"), hf!("0x1.ff9f89fb65be3p-60")],
    [hf!("0x1.5fd4d21fab226p-6"), hf!("-0x1.0c0a91c37851cp-61")],
    [hf!("0x1.78f535ddc9f04p-6"), hf!("0x1.b194ad9b1aa97p-61")],
    [hf!("0x1.92155f7a3667ep-6"), hf!("-0x1.b1d63091a013p-64")],
    [hf!("0x1.ab354b1504fcap-6"), hf!("-0x1.2ae47937cbdd3p-60")],
    [hf!("0x1.c454f4ce53b1dp-6"), hf!("-0x1.d63d7fef0e36cp-60")],
    [hf!("0x1.dd7458c64ab3ap-6"), hf!("-0x1.d653df3fcc281p-60")],
    [hf!("0x1.f693731d1cf01p-6"), hf!("-0x1.3fe9bc66286c7p-66")],
    [hf!("0x1.07d91ff98458p-5"), hf!("-0x1.ae248da7a9007p-60")],
    [hf!("0x1.14685db42c17fp-5"), hf!("-0x1.2890d277cb974p-59")],
    [hf!("0x1.20f770ceb11c7p-5"), hf!("-0x1.ec1b9d46693b1p-59")],
    [hf!("0x1.2d865759455cdp-5"), hf!("0x1.686f65ba93acp-61")],
    [hf!("0x1.3a150f6421afcp-5"), hf!("0x1.f8a318ba775fdp-60")],
    [hf!("0x1.46a396ff86179p-5"), hf!("0x1.136ac00fa2da9p-61")],
    [hf!("0x1.5331ec3bba0ebp-5"), hf!("0x1.442f2a9dac128p-61")],
    [hf!("0x1.5fc00d290cd43p-5"), hf!("0x1.a2669a693a8e1p-59")],
    [hf!("0x1.6c4df7d7d5b84p-5"), hf!("-0x1.39c56a9bd0a9bp-60")],
    [hf!("0x1.78dbaa5874686p-5"), hf!("-0x1.4a0ef4035c29cp-60")],
    [hf!("0x1.856922bb513c1p-5"), hf!("0x1.91adfd607cb2bp-59")],
];

/// `1 − cos(π·j/2^11)` for `0 ≤ j < 32` in double-double (C: `Cm` in `sincosn2`).
#[rustfmt::skip]
const CM2: [[f64; 2]; 32] = [
    [hf!("0x0p+0"), hf!("0x0p+0")],
    [hf!("0x1.3bd3c88cdca13p-20"), hf!("0x1.874628d2b6835p-74")],
    [hf!("0x1.3bd3bc5fc5ab4p-18"), hf!("-0x1.8a1bebf665cefp-75")],
    [hf!("0x1.634e1d173443dp-17"), hf!("0x1.198ff5804d8dcp-73")],
    [hf!("0x1.3bd38bab6d94cp-16"), hf!("0x1.c73be2184804ep-70")],
    [hf!("0x1.ed7a51288a277p-16"), hf!("-0x1.bcd47bf19555bp-70")],
    [hf!("0x1.634da1cec522dp-15"), hf!("-0x1.3fff35c1bdb61p-69")],
    [hf!("0x1.e39b20c7444e3p-15"), hf!("0x1.ae49f0be31e8cp-69")],
    [hf!("0x1.3bd2c8da49511p-14"), hf!("0x1.70df810bcc0e2p-69")],
    [hf!("0x1.8fb66ee122f6cp-14"), hf!("0x1.079ff34bffa7ep-70")],
    [hf!("0x1.ed7875885ea3ap-14"), hf!("-0x1.983278baa11c2p-69")],
    [hf!("0x1.2a8c672d4942fp-13"), hf!("-0x1.e4938f661aa14p-68")],
    [hf!("0x1.634bb4ae5ed49p-13"), hf!("0x1.e64c904cc8156p-67")],
    [hf!("0x1.a0fa1a872536ep-13"), hf!("0x1.f7469ecc1331fp-68")],
    [hf!("0x1.e3978f34889d9p-13"), hf!("0x1.5eb897cdc1b0dp-67")],
    [hf!("0x1.1592043856dbdp-12"), hf!("0x1.9865d9cdc3744p-67")],
    [hf!("0x1.3bcfbd9979a27p-12"), hf!("-0x1.595d548d9a586p-66")],
    [hf!("0x1.6484edd7f9e4ap-12"), hf!("-0x1.91df31aaa6f7fp-68")],
    [hf!("0x1.8fb18eacc3af8p-12"), hf!("-0x1.4eaa508eec2b7p-66")],
    [hf!("0x1.bd55996f62d2ep-12"), hf!("-0x1.7c51f7d8f9f71p-69")],
    [hf!("0x1.ed71071603e86p-12"), hf!("-0x1.f4827ccb50b62p-66")],
    [hf!("0x1.1001e81abab48p-11"), hf!("-0x1.12f4a2e3616d5p-66")],
    [hf!("0x1.2a86f68094692p-11"), hf!("0x1.604e1f8f76c27p-65")],
    [hf!("0x1.4647aaa599ed1p-11"), hf!("-0x1.1a79a86fb8ff5p-65")],
    [hf!("0x1.6344004228d8bp-11"), hf!("0x1.33736c96557c9p-65")],
    [hf!("0x1.817bf2ddf22b3p-11"), hf!("0x1.fc0555a81d729p-65")],
    [hf!("0x1.a0ef7dcffafabp-11"), hf!("0x1.54d46b817bca4p-65")],
    [hf!("0x1.c19e9c3e9d2c5p-11"), hf!("-0x1.70a980659e79p-70")],
    [hf!("0x1.e389491f8833ap-11"), hf!("0x1.c7313beeab883p-65")],
    [hf!("0x1.0357bf9be0ecfp-10"), hf!("-0x1.965827f33d907p-64")],
    [hf!("0x1.15889c8dd385fp-10"), hf!("0x1.98094fab77b42p-67")],
    [hf!("0x1.2857389776587p-10"), hf!("-0x1.bfdfce09aea7bp-64")],
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

    /// u = 2^−53, the unit roundoff.
    fn u() -> Dy {
        p2(-53)
    }

    /// `a ≤ b`.
    fn le(a: &Dy, b: &Dy) -> bool {
        a.cmp(b) != Ordering::Greater
    }

    /// `n + 2^−k`: the slightly enlarged constants of the proof (for example 3.0002 ≤ 3 + 2^−12).
    fn c(n: i64, k: i64) -> Dy {
        Dy::int(n).add(&p2(-k))
    }

    /// `γ_n = n·u/(1 − n·u)`, rounded up.
    fn gamma(n: i64) -> Dy {
        let nu = u().mul(&Dy::int(n));
        Dy::div(&nu, &Dy::int(1).sub(&nu), P, true)
    }

    /// `1 − u`.
    fn one_minus_u() -> Dy {
        Dy::int(1).sub(&u())
    }

    /// sin and cos of `π·s/2^11`.
    fn exact_sincos(s: u64) -> (Iv, Iv) {
        sin_cos_iv(
            &pi().mul(
                &Iv::point(Dy {
                    m: s.into(),
                    e: -11,
                }),
                P,
            ),
            P,
        )
    }

    /// Taylor coefficients up to degree `n` of `sin(β·t)` (`odd`) or `cos(β·t)` (even), with
    /// `β` an interval: degree j holds `±β^j/j!`, zero for the other parity.
    fn taylor(beta: &Iv, n: usize, odd: bool) -> Vec<Iv> {
        let mut t = vec![Iv::int(0); n + 1];
        let mut pw = Iv::int(1);
        for (j, tj) in t.iter_mut().enumerate().skip(1) {
            pw = pw.mul(beta, P);
            if (j % 2 == 1) == odd {
                let v = pw.div(&Iv::point(Dy::big(factorial(j as u64))), P);
                *tj = if (j / 2) % 2 == 1 { v.neg() } else { v };
            }
        }
        t
    }

    /// `t^n/n!` for a bound `t ≥ 0`: the Lagrange remainder of sin or cos after degree `n − 1`.
    fn lagrange(t: &Dy, n: u64) -> Dy {
        let pw = (0..n).fold(Dy::int(1), |a, _| a.mul(t));
        Dy::div(&pw, &Dy::big(factorial(n)), P, true)
    }

    /// `y^n`.
    fn pow(y: &Dy, n: u32) -> Dy {
        (0..n).fold(Dy::int(1), |a, _| a.mul(y))
    }

    /// The smallest magnitude in an interval that does not contain zero.
    fn min_abs(v: &Iv) -> Dy {
        let (a, b) = (v.lo.abs(), v.hi.abs());
        if a.cmp(&b) == Ordering::Less { a } else { b }
    }

    /// Step L of the proof: arguments |x| ≥ 2^41 that are not multiples of ½ (those return
    /// earlier) give `sh + sl` from the fast table with no rounding test. For every table
    /// argument it is the correctly rounded sine.
    #[test]
    fn large_arguments_are_correctly_rounded_exhaustively() {
        for s in (0..4096u64).filter(|s| s % 2048 != 0) {
            let (sh, sl, _, _) = sincosn(s);
            let want = crate::math::reference::sinpi(s as f64 / 2048.0);
            assert_eq!((sh + sl).to_bits(), want.to_bits(), "s = {s}");
        }
    }

    /// Steps F1 and A1: the measured table constants, τ₁ and σ₁ of the fast table and the low
    /// parts of the accurate table.
    #[test]
    fn table_errors() {
        let mut tau1 = Dy::zero();
        let mut sigma1 = 0f64;
        let mut low_ratio = 0f64;
        for s in 0..=4096u64 {
            let (sv, cv) = exact_sincos(s);
            let (sh, sl, ch, cl) = sincosn(s);
            tau1 = dmax(tau1, dmax(dd_error(sh, sl, &sv), dd_error(ch, cl, &cv)));
            // By comparison, not `f64::max` (chapter 21's ban).
            for low in [sl.abs(), cl.abs()] {
                if low > sigma1 {
                    sigma1 = low;
                }
            }
            assert!(sh.abs() <= 1.0 && ch.abs() <= 1.0);
            let (sh, sl, ch, cl) = sincosn2(s);
            for (h, l) in [(sh, sl), (ch, cl)] {
                if h == 0.0 {
                    assert_eq!(l, 0.0);
                } else {
                    let ratio = l.abs() / h.abs();
                    if ratio > low_ratio {
                        low_ratio = ratio;
                    }
                }
            }
        }
        assert!(
            le(&tau1, &Dy::from_f64(TAU1)),
            "τ₁ = 2^{}",
            log2_approx(&tau1)
        );
        assert!(sigma1 <= SIGMA1, "σ₁ = {sigma1:e}");
        assert!(
            low_ratio <= 24.0 * f64::EPSILON / 2.0,
            "accurate table: |low| ≤ 24u·|high|"
        );
        eprintln!("τ₁ = 2^{:.3}, σ₁ = {:.3e}", log2_approx(&tau1), sigma1);
    }

    /// Steps F2 to F4: `E_F(b) + u·R(b) ≤ (1 − u)²·(|k|·2^−123 + ER0)` at both ends of
    /// `0 ≤ b ≤ π·2^−12`. The left side is convex in `b` and the right side linear, so the
    /// inequality holds in between.
    #[test]
    fn fast_path_bound() {
        let u = u();
        let beta = pi().scale(-75); // the angle of one unit of k
        let ts = taylor(&beta, 5, true);
        let tc = taylor(&beta, 4, false);
        // |coefficient − Taylor coefficient|/β^j: the constants s1, s3, s5 and c2, c4
        let rel = |t: &Iv, v: f64, j: usize| {
            let bj = (0..j).fold(Iv::int(1), |a, _| a.mul(&beta, P));
            t.sub(&Iv::point(Dy::from_f64(v)), P).div(&bj, P).mag()
        };
        let (s1, s3, s5) = (
            rel(&ts[1], SN[0], 1),
            rel(&ts[3], SN[1], 3),
            rel(&ts[5], SN[2], 5),
        );
        let (c2, c4) = (rel(&tc[2], CN[0], 2), rel(&tc[4], CN[1], 4));
        let (tau1, sigma1, er0) = (Dy::from_f64(TAU1), Dy::from_f64(SIGMA1), Dy::from_f64(ER0));
        let one = Dy::int(1);
        let b_max = pi().scale(-12).hi;
        for (b, k) in [(Dy::zero(), Dy::zero()), (b_max, p2(63))] {
            let half_b2 = b.mul(&b).scale(-1);
            let eps_s = s1
                .mul(&b)
                .add(&s3.mul(&pow(&b, 3)))
                .add(&s5.mul(&pow(&b, 5)))
                .add(&lagrange(&b, 7));
            let eps_c = c2
                .mul(&pow(&b, 2))
                .add(&c4.mul(&pow(&b, 4)))
                .add(&lagrange(&b, 6));
            let p1 = half_b2.add(&eps_c).mul(&one.add(&c(4, 12).mul(&u)));
            let p2m = b.add(&eps_s).mul(&one.add(&c(3, 12).mul(&u)));
            let a = sigma1.add(&p1).mul(&one.add(&u));
            let r = a.add(&p2m).mul(&one.add(&u));
            let e_f = tau1
                .mul(&one.add(&b))
                .add(&sigma1.mul(&b.add(&half_b2)))
                .add(&eps_c)
                .add(&c(4, 12).mul(&u).mul(&half_b2.add(&eps_c)))
                .add(&eps_s)
                .add(&c(3, 12).mul(&u).mul(&b.add(&eps_s)))
                .add(&u.mul(&sigma1.add(&p1)))
                .add(&u.mul(&a.add(&p2m)));
            let need = e_f.add(&u.mul(&r));
            let have = one_minus_u()
                .mul(&one_minus_u())
                .mul(&k.scale(-123).add(&er0));
            eprintln!(
                "fast path, |k| = {:e}: 2^{:.3} ≤ 2^{:.3}",
                k.to_f64(),
                log2_approx(&need),
                log2_approx(&have)
            );
            assert!(
                le(&need, &have),
                "k = {}: need 2^{} ≤ 2^{}",
                k.to_f64(),
                log2_approx(&need),
                log2_approx(&have)
            );
        }
    }

    /// Steps Z1 to Z3: the near-zero fast paths of sinpi and cospi.
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one proof step, its ingredients computed and checked in order"
    )]
    fn near_zero_fast_path_bounds() {
        let u = u();
        let one = Dy::int(1);
        let k3 = pow(&one_minus_u(), 3);
        // sinpi, 2^−54 ≤ |x| ≤ X1.
        let x1 = Dy::from_f64(hf!("0x1.2p-5"));
        let x1sq = x1.mul(&x1);
        let cz = [
            hf!("-0x1.4abbce625be51p+2"),
            hf!("0x1.466bc67754b46p+1"),
            hf!("-0x1.32d2cc12a51f4p-1"),
            hf!("0x1.5060540058476p-4"),
        ];
        let mut tsin = taylor(&pi(), 15, true);
        tsin[1] = Iv::int(0);
        let q = error_poly(
            &tsin,
            &[(3, cz[0]), (5, cz[1]), (7, cz[2]), (9, cz[3])],
            3,
            2,
        );
        let tail = Dy::div(
            &lagrange(&pi().mul(&Iv::point(x1.clone()), P).hi, 17),
            &x1.mul(&x1sq),
            P,
            true,
        );
        let approx = poly_sup_bound(&q, &x1sq, 2000, 160).add(&tail);
        assert!(
            le(&approx, &p2(-49)),
            "Z1: sinpi polynomial error 2^{}·|x|³",
            log2_approx(&approx)
        );
        let m: Vec<Dy> = cz.iter().map(|&v| Dy::from_f64(v).abs()).collect();
        let pz = m[0]
            .add(&m[1].mul(&x1sq))
            .add(&m[2].mul(&pow(&x1sq, 2)))
            .add(&m[3].mul(&pow(&x1sq, 3)));
        let q_err = gamma(2)
            .mul(&m[0])
            .add(&gamma(4).mul(&m[1]).mul(&x1sq))
            .add(&gamma(6).mul(&m[2]).mul(&pow(&x1sq, 2)))
            .add(&gamma(8).mul(&m[3]).mul(&pow(&x1sq, 3)));
        let ph = Dy::from_f64(hf!("0x1.921fb54442d18p+1"));
        let pl = Dy::from_f64(hf!("0x1.1a62633145c07p-53"));
        let c_pi = dmax(
            pi().lo.sub(&ph.add(&pl)).abs(),
            pi().hi.sub(&ph.add(&pl)).abs(),
        );
        // E_Z + u·|zl| ≤ |x|·lin + |x|³·cub
        let zl0 = u.mul(&ph).add(&pl.mul(&one.add(&u))).mul(&one.add(&u));
        let poly = pz.add(&q_err).mul(&one.add(&gamma(3)));
        let lin = c_pi
            .add(&u.mul(&pl))
            .add(&u.mul(&u.mul(&ph).add(&pl.mul(&one.add(&u)))))
            .add(&u.mul(&zl0).scale(1).mul(&one.add(&u)));
        let cub = approx
            .add(&gamma(3).mul(&pz))
            .add(&one.add(&gamma(3)).mul(&q_err))
            .add(&u.mul(&poly).scale(1).mul(&one.add(&u)));
        assert!(le(&lin, &k3.mul(&p2(-102))), "Z2: sinpi, |x| part");
        let have = k3.mul(&x1sq.mul(&one_minus_u()).scale(-47).add(&p2(-102)));
        let need = lin.add(&cub.mul(&x1sq));
        eprintln!(
            "near-zero sinpi: polynomial 2^{:.3}·|x|³; at X1 2^{:.3} ≤ 2^{:.3}",
            log2_approx(&approx),
            log2_approx(&need),
            log2_approx(&have)
        );
        assert!(
            le(&need, &have),
            "Z2: sinpi at X1, 2^{} vs 2^{}",
            log2_approx(&need),
            log2_approx(&have)
        );

        // cospi, X0 < |x| ≤ 2^−12.
        let x0 = p2(-12);
        let x0sq = x0.mul(&x0);
        let cc = [
            hf!("-0x1.3bd3cc9be45dcp+2"),
            hf!("0x1.03c1f081b0833p+2"),
            hf!("-0x1.55d3c6fc9af15p+0"),
            hf!("0x1.e1d3ff2ae3f9ap-3"),
        ];
        let tcos = taylor(&pi(), 16, false);
        let q = error_poly(
            &tcos,
            &[(2, cc[0]), (4, cc[1]), (6, cc[2]), (8, cc[3])],
            2,
            2,
        );
        let tail = Dy::div(
            &lagrange(&pi().mul(&Iv::point(x0.clone()), P).hi, 18),
            &x0sq,
            P,
            true,
        );
        let approx = poly_sup_bound(&q, &x0sq, 200, 160).add(&tail);
        let m: Vec<Dy> = cc.iter().map(|&v| Dy::from_f64(v).abs()).collect();
        let pc = m[0]
            .add(&m[1].mul(&x0sq))
            .add(&m[2].mul(&pow(&x0sq, 2)))
            .add(&m[3].mul(&pow(&x0sq, 3)));
        let q_err = gamma(2)
            .mul(&m[0])
            .add(&gamma(4).mul(&m[1]).mul(&x0sq))
            .add(&gamma(6).mul(&m[2]).mul(&pow(&x0sq, 2)))
            .add(&gamma(8).mul(&m[3]).mul(&pow(&x0sq, 3)));
        let p_err = gamma(2).mul(&pc).add(&one.add(&gamma(2)).mul(&q_err));
        let need = approx.add(&p_err).add(&u.mul(&pc.add(&p_err)));
        let have = k3.mul(&Dy::from_f64(hf!("0x1.ap-48")));
        eprintln!(
            "near-zero cospi: polynomial 2^{:.3}·x²; 2^{:.3} ≤ 2^{:.3}",
            log2_approx(&approx),
            log2_approx(&need),
            log2_approx(&have)
        );
        assert!(
            le(&need, &have),
            "Z3: cospi, 2^{} vs 2^{}",
            log2_approx(&need),
            log2_approx(&have)
        );
        // Below X0, cos(πx) rounds to 1: (π·X0)² ≤ 2^−53.
        let px0 = pi()
            .mul(
                &Iv::point(Dy::from_f64(f64::from_bits(0x3e2c_cf64_29be_6621))),
                P,
            )
            .hi;
        assert!(le(&px0.mul(&px0), &p2(-53)), "Z3: the threshold X0");
    }

    /// Steps A2 and A3: the accurate path's relative error is below 2^−97, as the worst-case
    /// argument needs. Checked for every table index against the smallest |sin(A ± b)|.
    /// (The reference's `sinpi` used in step L is independent of this module.)
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one proof step, its ingredients computed and checked in order"
    )]
    fn refine_bound() {
        let uu = u().mul(&u());
        let target = p2(-97);
        let beta = pi().scale(-12);
        let b_max = beta.hi.clone();
        let dd = |h: f64, l: f64| Dy::from_f64(h).add(&Dy::from_f64(l));
        // ε_rs: sin(β·x) − x·2^−12·Ps(x²) over |x| ≤ 1, coefficient by coefficient
        let s_coeffs = [
            dd(hf!("0x1.921fb54442d18p+1"), hf!("0x1.1a62633145c06p-53")),
            dd(hf!("-0x1.4abbce625be53p-22"), hf!("0x1.05511cbc65743p-76")),
            dd(hf!("0x1.466bc6775aae1p-47"), hf!("-0x1.9c3c168d990ap-114")),
            Dy::from_f64(hf!("-0x1.32d2cc920dcb4p-73")),
        ];
        let mut d = taylor(&beta, 9, true);
        for (k, cf) in s_coeffs.iter().enumerate() {
            d[2 * k + 1] = d[2 * k + 1].sub(&Iv::point(cf.scale(-12)), P);
        }
        let eps_rs = d
            .iter()
            .fold(Dy::zero(), |a, t| a.add(&t.mag()))
            .add(&lagrange(&b_max, 11));
        // ε_rc: cos(β·x) − 1 − x²·Pc(x²) over |x| ≤ 1
        let c_coeffs = [
            dd(hf!("-0x1.3bd3cc9be45dep-22"), hf!("-0x1.692b71366cc04p-76")),
            dd(hf!("0x1.03c1f081b5ac4p-46"), hf!("-0x1.32b33fda9113cp-100")),
            Dy::from_f64(hf!("-0x1.55d3c7e3cbff9p-72")),
            Dy::from_f64(hf!("0x1.e1f50604fa0ffp-99")),
        ];
        let mut d = taylor(&beta, 8, false);
        for (k, cf) in c_coeffs.iter().enumerate() {
            d[2 * k + 2] = d[2 * k + 2].sub(&Iv::point(cf.clone()), P);
        }
        let eps_rc = d
            .iter()
            .fold(Dy::zero(), |a, t| a.add(&t.mag()))
            .add(&lagrange(&b_max, 10));
        // iq = 0 and 2048: v = ±sin B; ε_rs ∝ b^3 and better, the u² term ∝ b, and
        // sin b ≥ b·(1 − 2^−20): the relative bound is largest at b = b_max.
        let rel0 = Dy::div(
            &eps_rs.add(&c(12, 3).mul(&uu).mul(&b_max)),
            &b_max.mul(&Dy::int(1).sub(&p2(-20))),
            P,
            true,
        );
        assert!(
            le(&rel0, &target),
            "A2: refine at iq = 0, 2^{}",
            log2_approx(&rel0)
        );
        // other indices: the error terms of step A3 with this index's table values
        let u = u();
        let one = Dy::int(1);
        let (rho_s, rho_c) = (c(12, 3).mul(&uu), Dy::int(22).mul(&uu));
        let (lam_s, lam_c) = (c(2, 3).mul(&u), c(5, 6).mul(&u));
        let up = |d: &Dy, k: i64| d.mul(&one.add(&p2(-k))); // d·(1 + 2^−k)
        let b1 = up(&b_max, 20); // ≥ |slh + sll|
        let b2 = up(&b_max.mul(&b_max).scale(-1), 20); // ≥ |clh + cll|
        let mut worst = Dy::zero();
        for s in (1..4096u64).filter(|&s| s != 2048) {
            let a = pi().mul(
                &Iv::point(Dy {
                    m: s.into(),
                    e: -11,
                }),
                P,
            );
            let (sv, cv) = sin_cos_iv(&a, P);
            let (sbh, sbl, cbh, cbl) = sincosn2(s);
            let (es, ec) = (dd_error(sbh, sbl, &sv), dd_error(cbh, cbl, &cv));
            let [sh, sl, ch, cl] = [sbh, sbl, cbh, cbl].map(|v| Dy::from_f64(v).abs());
            let mu_s = if sbh == 0.0 {
                Dy::zero()
            } else {
                Dy::div(&sl, &sh, 64, true)
            };
            let mu_c = if cbh == 0.0 {
                Dy::zero()
            } else {
                Dy::div(&cl, &ch, 64, true)
            };
            // the two products and their magnitudes
            let t1 = up(&b2.mul(&sh.add(&sl)), 20); // ≥ |csh + csl|
            let t2 = up(&b1.mul(&ch.add(&cl)), 20); // ≥ |sch + scl|
            let mul_err = |lam: &Dy, mu: &Dy, m: &Dy| {
                uu.add(&c(3, 10).mul(&u).mul(&lam.add(mu)))
                    .add(&lam.mul(mu))
                    .mul(m)
            };
            let e_m1 = mul_err(&lam_c, &mu_s, &t1);
            let e_m2 = mul_err(&lam_s, &mu_c, &t2);
            // the low parts of the products
            let csl = up(&u.add(&lam_c).add(&mu_s).mul(&t1), 9);
            let scl = up(&u.add(&lam_s).add(&mu_c).mul(&t2), 9);
            let tsh = up(&t1.add(&t2), 50);
            let tsl = u.mul(&tsh);
            let sum_l = up(&csl.add(&scl), 52);
            let tsl1 = up(&tsl.add(&sum_l), 51);
            let e_f1 = u.mul(&sum_l).add(&u.mul(&tsl1));
            let tsh2 = up(&sh.add(&tsh), 52);
            let tsl2 = u.mul(&tsh2);
            let tsl3 = up(&sl.add(&tsl1), 51).add(&up(&tsl2, 52));
            let e_f2 = u.mul(&up(&sl.add(&tsl1), 52)).add(&u.mul(&tsl3));
            let e = es
                .add(&ec.mul(&b_max))
                .add(&ch.add(&cl).mul(&eps_rs.add(&rho_s.mul(&b1))))
                .add(&sh.add(&sl).mul(&eps_rc.add(&rho_c.mul(&b2))))
                .add(&e_m1)
                .add(&e_m2)
                .add(&e_f1)
                .add(&e_f2);
            // the fast two-sums are exact: |sch| ≥ |csh| (or cbh = 0) and |sbh| ≥ |tsh|
            assert!(
                cbh == 0.0 || le(&t1, &ch.mul(&b_max).mul(&one.sub(&p2(-10)))),
                "s = {s}: F1 order"
            );
            assert!(le(&tsh, &sh), "s = {s}: F2 order");
            let (lo, _) = sin_cos_iv(&a.sub(&Iv::point(b_max.clone()), P), P);
            let (hi, _) = sin_cos_iv(&a.add(&Iv::point(b_max.clone()), P), P);
            assert!(lo.lo.signum() == lo.hi.signum() && hi.lo.signum() == hi.hi.signum());
            assert!(
                lo.lo.signum() == hi.lo.signum(),
                "s = {s}: no zero of sin within b of A"
            );
            let vmin = {
                let (m1, m2) = (min_abs(&lo), min_abs(&hi));
                if m1.cmp(&m2) == Ordering::Less {
                    m1
                } else {
                    m2
                }
            };
            worst = dmax(worst, Dy::div(&e, &vmin, P, true));
        }
        eprintln!(
            "refine: iq = 0: 2^{:.3}; others: 2^{:.3}; ε_rs = 2^{:.3}, ε_rc = 2^{:.3}",
            log2_approx(&rel0),
            log2_approx(&worst),
            log2_approx(&eps_rs),
            log2_approx(&eps_rc)
        );
        assert!(le(&worst, &target), "A3: refine, 2^{}", log2_approx(&worst));
    }

    /// Steps A4 to A6: the near-zero accurate paths and the tiny path have relative error below
    /// 2^−97 (absolute below 2^−97 for cospi, whose value is near 1).
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one proof step, its ingredients computed and checked in order"
    )]
    fn near_zero_accurate_bounds() {
        let u = u();
        let uu = u.mul(&u);
        let target = p2(-97);
        let one = Dy::int(1);
        let dd = |h: f64, l: f64| Dy::from_f64(h).add(&Dy::from_f64(l));
        // A4: sinpi_near_zero, 2^−54 ≤ |x| ≤ X1; Qd(x²) approximates (sin πx − πx)/x³.
        let x1 = Dy::from_f64(hf!("0x1.2p-5"));
        let x1sq = x1.mul(&x1);
        let qd = [
            dd(hf!("-0x1.4abbce625be53p+2"), hf!("0x1.05511c68477bep-52")),
            dd(hf!("0x1.466bc6775aae2p+1"), hf!("-0x1.6dc0cbefae1dap-54")),
            dd(hf!("-0x1.32d2cce62bd86p-1"), hf!("0x1.066bd54973829p-55")),
            dd(hf!("0x1.50783487ee781p-4"), hf!("0x1.832989f39a743p-58")),
            Dy::from_f64(hf!("-0x1.e3074fde861fp-8")),
            Dy::from_f64(hf!("0x1.e8f4344534da6p-12")),
            Dy::from_f64(hf!("-0x1.6f9cd7b8cb9dbp-16")),
        ];
        let tsin = taylor(&pi(), 23, true);
        let q: Vec<Iv> = (0..11)
            .map(|k| {
                let t = tsin[2 * k + 3].clone();
                if k < qd.len() {
                    t.sub(&Iv::point(qd[k].clone()), P)
                } else {
                    t
                }
            })
            .collect();
        let tail = Dy::div(
            &lagrange(&pi().mul(&Iv::point(x1.clone()), P).hi, 25),
            &x1.mul(&x1sq),
            P,
            true,
        );
        let eps_q = poly_sup_bound(&q, &x1sq, 2000, 192).add(&tail);
        let pi3 = dd(hf!("0x1.92p+1"), hf!("0x1.fb54442d1846ap-11"))
            .add(&Dy::from_f64(hf!("-0x1.d9cceba3f91f2p-65")));
        let c_pi3 = dmax(pi().lo.sub(&pi3).abs(), pi().hi.sub(&pi3).abs());
        let (pi1, pi2) = (
            Dy::from_f64(hf!("0x1.fb54442d1846ap-11")),
            Dy::from_f64(hf!("-0x1.d9cceba3f91f2p-65")).abs(),
        );
        let qmag = Dy::from_f64(5.2).mul(&x1sq); // ≥ |x²·Qd(x²)|
        let cl0 = Dy::from_f64(hf!("0x1.e3074fde861fp-8"));
        let e_sz = c_pi3
            .add(
                &Dy::int(8)
                    .mul(&u)
                    .mul(&pi2.add(&u.mul(&pi1)).add(&u.mul(&qmag))),
            )
            .add(&Dy::int(32).mul(&uu).mul(&qmag))
            .add(&Dy::int(8).mul(&u).mul(&cl0).mul(&pow(&x1sq, 5)))
            .add(&eps_q.mul(&x1sq));
        let v_min = pi()
            .lo
            .mul(&one.sub(&pow(&pi().mul(&Iv::point(x1.clone()), P).hi, 2)));
        let rel = Dy::div(&e_sz, &v_min, P, true);
        eprintln!(
            "sinpi_near_zero: Qd error 2^{:.3}·|x|³, relative 2^{:.3}",
            log2_approx(&eps_q),
            log2_approx(&rel)
        );
        assert!(le(&rel, &target), "A4");
        assert!(le(&rel, &p2(NEAR_ZERO_REL_LOG2)), "A4, as A7 uses it");
        // A5: cospi_near_zero, X0 < |x| ≤ 2^−12; Qc(x²) approximates (cos πx − 1)/x².
        let x0 = p2(-12);
        let x0sq = x0.mul(&x0);
        let qc = [
            dd(hf!("-0x1.3bd3cc9be45dep+2"), hf!("-0x1.692b71366cc04p-52")),
            dd(hf!("0x1.03c1f081b5ac4p+2"), hf!("-0x1.32b33fda9113cp-52")),
            Dy::from_f64(hf!("-0x1.55d3c7e3cbff9p+0")),
            Dy::from_f64(hf!("0x1.e1f50604fa0ffp-3")),
        ];
        let tcos = taylor(&pi(), 20, false);
        let q: Vec<Iv> = (0..10)
            .map(|k| {
                let t = tcos[2 * k + 2].clone();
                if k < qc.len() {
                    t.sub(&Iv::point(qc[k].clone()), P)
                } else {
                    t
                }
            })
            .collect();
        let tail = Dy::div(
            &lagrange(&pi().mul(&Iv::point(x0.clone()), P).hi, 22),
            &x0sq,
            P,
            true,
        );
        let eps_qc = poly_sup_bound(&q, &x0sq, 200, 192).add(&tail);
        let cmag = Dy::from_f64(5.0).mul(&x0sq); // ≥ |x²·Qc(x²)|
        let e_cz = eps_qc
            .mul(&x0sq)
            .add(&Dy::int(32).mul(&uu).mul(&cmag))
            .add(&Dy::int(8).mul(&uu));
        eprintln!(
            "cospi_near_zero: Qc error 2^{:.3}·x², absolute 2^{:.3}",
            log2_approx(&eps_qc),
            log2_approx(&e_cz)
        );
        assert!(le(&e_cz, &target), "A5");
        // A6: the tiny path, |x| < 2^−54: πx in double-double, relative to π|x|(1 − (π·2^−54)²/6).
        let ph = Dy::from_f64(hf!("0x1.921fb54442d18p+1"));
        let pl = Dy::from_f64(hf!("0x1.1a62633145c07p-53"));
        let c_pi = dmax(
            pi().lo.sub(&ph.add(&pl)).abs(),
            pi().hi.sub(&ph.add(&pl)).abs(),
        );
        let t = pi().hi.scale(-54);
        let e_t = c_pi
            .add(&u.mul(&c(2, 6).mul(&pl).add(&u.mul(&ph))))
            .add(&pi().hi.mul(&t.mul(&t)).scale(-2)); // π·(πx)²/6 ≤ π·(π·2^−54)²/4
        let rel_t = Dy::div(&e_t, &pi().lo.mul(&one.sub(&t.mul(&t))), P, true);
        eprintln!("tiny path: relative 2^{:.3}", log2_approx(&rel_t));
        assert!(le(&rel_t, &target), "A6");
        assert!(le(&rel_t, &p2(NEAR_ZERO_REL_LOG2)), "A6, as A7 uses it");
    }

    /// The relative error of the near-zero accurate path and of the tiny path that step A7
    /// relies on: 2^−104 (2^−105.97 and 2^−104.74 by `near_zero_accurate_bounds`).
    const NEAR_ZERO_REL_LOG2: i64 = -104;

    /// Step A7: the images of sinpi.wc's set (b) under powers of two are the hard cases of
    /// `sinpi` near zero. `δ`, the largest distance between `πx` and `sin(πx)` in units of the
    /// last place for `|x| < 2^−53`, plus the paths' error, stays below `2^−44`, the margin Lemma 2
    /// gives an input outside the list; and no binade boundary `2^k ≥ 2^−1021` lies between
    /// `sin(πx)` and `πx` for a double `x`.
    #[test]
    fn scaled_worst_cases_near_zero() {
        let one = Dy::int(1);
        // δ < (πx)²/6·2^53/(1 − (πx)²/6) at |x| = 2^−53, the largest |x| concerned.
        let w = pi().hi.scale(-53);
        let w2 = w.mul(&w);
        let w2_6 = Dy::div(&w2, &Dy::int(6), P, true);
        let delta = Dy::div(&w2_6.scale(53), &one.sub(&w2_6), P, true);
        let path = p2(NEAR_ZERO_REL_LOG2 + 53);
        eprintln!(
            "scaled worst cases: δ < 2^{:.3} ulp, path error < 2^{:.3} ulp",
            log2_approx(&delta),
            log2_approx(&path)
        );
        assert!(
            le(&delta, &Dy::from_f64(hf!("0x1.a6p-53"))),
            "δ below 2^−52.28"
        );
        assert!(
            le(&delta.add(&path), &p2(-44)),
            "δ + path error below 2^−44"
        );
        // No binade boundary 2^k (−1021 ≤ k ≤ −52) between sin(πx₀) and πx₀ for the smallest double
        // x₀ ≥ 2^k/π (larger x are farther above): πx₀ − 2^k > (πx₀)³/6. x₀ is found from a lower
        // bound of 2^k/π, so a double below 2^k/π would show as a negative gap.
        for k in -1021..=-52i64 {
            let target = Dy::div(&p2(k), &pi().hi, P, false); // ≤ 2^k/π
            let near = target.to_f64();
            let x0 = if le(&target, &Dy::from_f64(near)) {
                near
            } else {
                near.next_up()
            };
            let x0d = Dy::from_f64(x0);
            let gap = pi().lo.mul(&x0d).sub(&p2(k)); // ≤ πx₀ − 2^k
            let px = pi().hi.mul(&x0d);
            let cubic = Dy::div(&px.mul(&px).mul(&px), &Dy::int(6), P, true);
            assert!(gap.signum() > 0 && le(&cubic, &gap), "k = {k}");
        }
    }
}
