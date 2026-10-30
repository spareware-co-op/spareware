//! Correctly rounded `exp(x) = e^x`: a port of CORE-MATH.
//!
//! Method: Ziv's strategy, as in CORE-MATH (<https://core-math.gitlabpages.inria.fr/>); the
//! algorithm and the proof of its bound are below.
//! Source: PORT from CORE-MATH `src/binary64/exp/exp.c` at commit
//!   `0ce0cbc15935e24c656280e5a46c34e89f301b7a`, by Alexei Sibidanov, MIT,
//!   <https://gitlab.inria.fr/core-math/core-math/-/blob/0ce0cbc15935e24c656280e5a46c34e89f301b7a/src/binary64/exp/exp.c>
//!   Copyright (c) 2022-2026 Alexei Sibidanov <sibid@uvic.ca>.
//!   Changed for Spare Kit: translated to safe `no_std` Rust, with total table reads and the
//!   integer steps under the crate's rule (`lib.rs`); round-to-nearest only, without the exception
//!   flags or `errno`; the overflow and underflow results returned directly; the canonical NaN for
//!   a NaN; a written proof of the error bound. Every change is listed under "Changes from the C
//!   source" below; none changes a result.
//! SPDX-License-Identifier: MPL-2.0 AND MIT
//!
//! The original notice of `exp.c`, reproduced as the MIT licence requires:
//!
//! ```text
//! Copyright (c) 2022-2026 Alexei Sibidanov <sibid@uvic.ca>.
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
//! Guarantees: correctly rounded, resting on CORE-MATH's worst-case searches, whose
//! completeness its list does not claim (step A of the proof). For every binary64 input checked
//! the result is the binary64 number nearest the exact `e^x` (ties to even), so the error is at
//! most half an ulp, including in the subnormal range. Proven outright: every input whose result
//! has fewer than 48 identical bits after its round bit (fewer than 33 for results below
//! 2^−1059), and every input in CORE-MATH's exp.wc (1 129 427, all checked), is correctly rounded;
//! an input outside the list with more identical bits, which no proof here excludes, would still
//! have an error below `½ + 2^−49.2` ulp (`½ + 2^−34` ulp below 2^−1059). Domain: every `x`.
//! `exp(x)` is `+∞` for `x ≥ 0x1.62e42fefa39fp+9` (overflow) and `+0` for
//! `x ≤ −0x1.74910d52d3052p+9` (underflow), as the correctly rounded results; `exp(−∞) = +0`,
//! `exp(+∞) = +∞`, and NaN gives the canonical NaN (`0x7ff8_0000_0000_0000`). The proof is at the
//! end of this documentation.
//!
//! Cost: constant time, no allocation and no recursion. Almost every argument takes the fast
//! path, a fixed sequence of floating-point operations and table reads; only when its rounding
//! test cannot decide does the accurate path run, also of fixed length. No loop's
//! length depends on the argument.
//!
//! The algorithm (Ziv's strategy, as CORE-MATH): `x = (64·i0 + i1 + 4096·ie)·ln 2/4096 + dx` with
//! `|dx| ≤ ln 2/2^13`, by Cody–Waite reduction with a three-part `ln 2/4096`. Then
//! `e^x = 2^ie · 2^(i0/64) · 2^(i1/4096) · e^dx`, the two powers of two from 64-entry
//! double-double tables and `e^dx` from a degree-4 polynomial. If the fast path's result,
//! widened by its error bound `eps`, rounds to one double, that is the answer; otherwise an
//! accurate path evaluates a degree-7 polynomial in double-double. The accurate path's rare
//! undecidable results are the hard-to-round inputs, which CORE-MATH found by exhaustive search
//! (its `exp.wc` file) and lists below with their correctly rounded values. Results in the
//! subnormal range are rounded once, at the subnormal precision, by adding `2^(−1022−ie)` before
//! the exponent field is cleared.
//!
//! Changes from the C source, none of which changes a result:
//!
//! - Only round-to-nearest is implemented (Rust has no other rounding mode, RFC 3514); the
//!   floating-point exception flags and `errno` are not kept. The overflow and underflow results
//!   are returned directly instead of being computed by an overflowing or underflowing product.
//! - NaN returns the canonical NaN instead of `x + x` (chapter 21).
//! - `fma` is `libm::fma` ([`super::support::fma`]), correctly rounded like `__builtin_fma`; the
//!   C source's unfused `ah * bl + f` stays unfused (its no-contraction configuration).
//! - `roundeven_finite` is `libm::rint` (round to nearest, ties to even, exact).
//! - `muldd`'s exact product error, `__builtin_fma(ch, xh, -ahhh)`, is
//!   [`super::support::product_error`]: that fused multiply-add natively, and on WebAssembly,
//!   whose fused multiply-add `libm` computes in software, Dekker's two-product, the same exact
//!   value (piece G47: it brings `exp` within twice `libm`'s time there).
//! - Table reads are total ([`super::support::entry`]); the accurate path is kept out of line
//!   (`#[cold]`, `#[inline(never)]`) so that the fast path's code stays small.
//! - The C code's integer steps follow the crate's rule (`lib.rs`): the conversion of the rounded
//!   `x·S` to an integer, exact by step R's bound, is the one cast, written beside that bound; the
//!   table indices are read from the bits of its two's complement; the exponent offset of a
//!   subnormal result is built by a checked step ([`super::support::pow2`]); and the additions to
//!   bit patterns that the C does modulo 2^64 (`as_ldexp`, the hard-case test) are wrapping, each
//!   saying why. The same values result.
//!
//! # Proof of the error bound
//!
//! The lemmas cited are those of [`super`]. Numbers marked *checked* are computed with exact
//! arithmetic (exhaustively over the tables) by the unit test named in parentheses. `u = 2^−53`.
//!
//! **R. Reduction.** `t = rint(RN(x·S))` with `S = RN(2^12/ln 2)` and `|x| < 746`, so `|t| < 2^22.1`
//! and `|t − x·2^12/ln 2| ≤ ½ + 2^−29.9`. `L2H` has 29 significant bits, so `L2H·t` is exact, and
//! `d = x − L2H·t` is exact: both are multiples of `2^−67` (or `t = 0`) and `|d| < 2^−13.5`.
//! The code reduces by `t·(L2H − L2L)` (fast path) or `t·(L2H − L2L − L2LL)` (accurate path), which
//! differ from `t·ln 2/2^12` by at most `|t|·2^−101.3` and `|t|·2^−156.4`. So the reduced argument
//! `r = x − t·ln 2/2^12` satisfies `|r| ≤ R_MAX = 0x1.62e44p−14`, and the computed `dx` is within
//! `u·|L2L·t| + u·|dx| + |t|·2^−101.3` of it (*checked*: `reduction_constants`). Then
//! `e^x = 2^ie·T·e^r` with `T = 2^((64·i0 + i1)/2^12) ∈ [1, 2)`.
//!
//! **F. The fast path.** (F1) The table product `th + tl` is within `τ_e = 2^−103` of `T`
//! relative, and `|tl| ≤ 2.5u·th`, for all 4096 index pairs (2^−103.82 and 2.47u measured)
//! (*checked*: `table_products`). (F2) `|e^r − 1 − r·Pe(r)| ≤ ε_p = 2^−76` for `|r| ≤ R_MAX`
//! (2^−76.16 by subdivision; CORE-MATH's Sollya figure is 2^−76.173) [*checked*:
//! `fast_polynomial`]. (F3) With `p = Pe(dx)·(1 + θ)`, `|θ| ≤ 2.001u`, and `tx = RN(th·dx)`, the
//! error of `th + fl` is at most `τ_e·T·e^r + T·ε_p + |tl|·|r|·Pe + th·(|r − dx|·Pe +
//! |dx|·|r − dx|/2 + 5u·|dx|·Pe) + u·|tx·p| + u·|fl|`, which with Lemma 1's `u·|fl|` is at most
//! 2^−62.01; CORE-MATH's bound `EPS·(1 − u) = 2^−61.80` covers it, so its fast path is proven as it
//! stands (*checked*: `fast_path_bound`). For results in the subnormal range the code adds
//! `2^(−1022−ie)` to `th` (an exact fast two-sum by Dekker's exponent condition) so that the sum's
//! last place is the result's; the extra rounding of `fl + e` is at most `u·ulp(fh)`. For
//! `ie ≥ −1060` that is below 2^−67 and Lemma 1 still applies; below, `ulp(fh) ≥ 2^−13`, and the
//! value that is rounded, `fh + RN(RN(fl + e) ± EPS)`, is within the error bound plus `EPS` plus
//! the two low-part roundings of the exact one, below `2^−34·ulp`, so Lemma 2 applies with the list
//! of subnormal results (`t = 33`) (*checked*).
//!
//! **A. The accurate path** has no rounding test. Its relative error is at most 2^−102.54 (A1 and
//! A2 below), below `2^−49.5·ulp`, and below `2^−49.2·ulp` for subnormal results with the extra
//! rounding described at the end; so by Lemma 2 it rounds correctly, unconditionally, every input
//! whose result has fewer than 48 identical bits after its round bit. CORE-MATH's exp.wc lists the
//! inputs its `BaCSeL` searches found with at least `t = 41` identical bits after the round bit
//! (`t = 33` for subnormal results), and all 1 129 427 of them are correctly rounded. But the
//! file's header says only that "for most inputs x" those bits were searched, and it combines
//! several separate searches, so whether an unlisted input has 48 or more identical bits is not
//! proven, here or in CORE-MATH (exp.c claims no completeness either); such an input would be
//! rounded from a value within `2^−49.2·ulp` of the exact one, an error below `½ + 2^−49.2` ulp.
//! The same holds below 2^−1059 (step F3) with 33 bits and `½ + 2^−34` ulp.
//! (A1) The table as in F1. (A2) The reduction
//! `dxh + dxl` is within 2^−118 of `r` (its fast two-sum is exact by Lemma 3(i): `d` is a multiple
//! of `2^−67`, hence of `ulp(dxl) ≤ 2^−76`); the degree-7 double-double polynomial has
//! approximation error 2^−129.2; the Horner steps (`FastFMA_DW`, with its last two operations
//! unfused as in the C source, each below `7.1u²` of its value), the products by `dx` and by
//! `th + tl` (Lemma 4: `12.6u²` and `17.8u²`) are scaled by `|r| ≤ R_MAX`, and the final fast sum
//! adds at most `3u²`. The relative error is at most 2^−102.54, below the `2^−95` that Lemma 2
//! needs with `t = 41` (*checked*: `accurate_path_bound`). In the subnormal range the same
//! offset as in F3 makes the single final rounding happen at the result's precision, with an extra
//! error below `2^−50·ulp`, so Lemma 2 applies with `t = 33`. ∎

use super::hexf::hf;
use super::support::{entry, fast_two_sum, fma, pow2, product_error, unsigned};

/// `2^12/ln 2`, rounded: the reduction's scale.
const S: f64 = hf!("0x1.71547652b82fep+12");
/// `ln 2/2^12 ≈ L2H + L2L + L2LL`. `L2H` has 29 significant bits, so `L2H·t` is exact for
/// `|t| < 2^23` (true since `|x| < 746`).
const L2H: f64 = hf!("0x1.62e42ffp-13");
/// See [`L2H`].
const L2L: f64 = hf!("0x1.718432a1b0e26p-47");
/// See [`L2H`]; used only by the accurate path.
const L2LL: f64 = hf!("0x1.9ff0342542fc3p-102");

/// The fast path's bound on the error of `fh + fl`, absolute (`fh` is in `[1, 2)`): CORE-MATH's
/// value, proven sufficient in step F3 of the proof.
const EPS: f64 = hf!("0x1.27p-62");

/// Bound on the reduced argument, `|dx| ≤ R_MAX` (proof, step R).
#[cfg(test)]
const R_MAX: f64 = hf!("0x1.62e44p-14");
/// The table products' relative error `τ_e` (proof, step F1; measured exhaustively).
#[cfg(test)]
const TAU_E: f64 = hf!("0x1p-103");
/// The table products' low parts, `|tl| ≤ SIGMA_E·th` (measured exhaustively).
#[cfg(test)]
const SIGMA_E: f64 = hf!("0x1.4p-52");
/// The fast polynomial's approximation error on `|r| ≤ R_MAX` (proof, step F2).
#[cfg(test)]
const EPS_P: f64 = hf!("0x1p-76");

/// Bit pattern above which a negative `x` (that is, `x < −0x1.6232bdd7abcd2p+9`) may give a
/// subnormal result.
const SUBNORMAL_THRESHOLD: u64 = 0xc086_232b_dd7a_bcd2;

/// Double-double product `(xh + xl)·(ch + cl)` (C: `muldd`), low-order terms unfused. The exact
/// error of `ch·xh` is C's `__builtin_fma(ch, xh, -ahhh)`, through [`product_error`]: Lemma 4's
/// proof takes it exact, so every call has `|ch·xh| ≥ 2^−969`.
#[inline]
fn mul_dd(xh: f64, xl: f64, ch: f64, cl: f64) -> (f64, f64) {
    let ahhh = ch * xh;
    let l = (ch * xl + cl * xh) + product_error(ch, xh, ahhh);
    (ahhh, l)
}

/// Double-double sum (C: `fastsum`), the high parts by fast two-sum.
#[inline]
fn fast_sum(xh: f64, xl: f64, yh: f64, yl: f64) -> (f64, f64) {
    let (sh, sl) = fast_two_sum(xh, yh);
    (sh, (xl + yl) + sl)
}

/// `(ah + al)·(bh + bl) + (ch + cl)` in double-double (C: `FastFMA_DW`): Algorithm 4 of
/// Jeannerod, Joldes, Louvet and Muller, "Extended-precision FMA under parameterized double-word
/// overlap: tight error bounds and examples", ARITH 2026 (<https://inria.hal.science/hal-05517451>),
/// with its last two fused operations unfused, as in the C source.
#[inline]
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the C source's variables, so the two can be read side by side"
)]
fn fma_dd(ah: f64, al: f64, bh: f64, bl: f64, ch: f64, cl: f64) -> (f64, f64) {
    let dh = fma(ah, bh, ch);
    let t = ch - dh;
    let e = fma(ah, bh, t);
    let f = e + cl;
    let g = ah * bl + f;
    let l = al * bh + g;
    (dh, l)
}

/// Horner evaluation of `Σ c[i]·x^i` in double-double with [`fma_dd`] (C: `opolydd`). The
/// coefficient list is never empty; an empty one is treated as the zero polynomial.
fn poly_dd(xh: f64, xl: f64, c: &[[f64; 2]]) -> (f64, f64) {
    let Some((last, rest)) = c.split_last() else {
        return (0.0, 0.0);
    };
    let (mut ch, mut cl) = (last[0], last[1]);
    for ci in rest.iter().rev() {
        (ch, cl) = fma_dd(xh, xl, ch, cl, ci[0], ci[1]);
    }
    (ch, cl)
}

/// `x · 2^i` by adding `i` to the exponent field (C: `as_ldexp`). Valid when the result stays a
/// normal number, which the callers guarantee.
///
/// C: `x.u += (int64_t) i << 52`, the two's complement of `i` shifted into the exponent field and
/// added modulo 2^64, so that a negative `i` subtracts from the field; the bits above the field,
/// the sign, take the carry or borrow modulo 2^64, which leaves them unchanged when the field
/// stays in range. That modular sum is the computation, so it is a wrapping addition.
#[inline]
fn add_exponent(x: f64, i: i64) -> f64 {
    f64::from_bits(x.to_bits().wrapping_add(unsigned(i) << 52))
}

/// The significand field of `x` read as a subnormal number (C: `as_todenormal`): clears the sign
/// and exponent fields.
#[inline]
fn significand_as_subnormal(x: f64) -> f64 {
    f64::from_bits(x.to_bits() & ((1u64 << 52) - 1))
}

/// Splits `x` for the reduction: the integer `jt = rint(x·S)` gives `ie = jt >> 12` and the table
/// indices `i0 = (jt >> 6) mod 64`, `i1 = jt mod 64`, and `th + tl ≈ 2^(i0/64 + i1/4096)`.
/// Returns `(t, ie, th, tl)` with `t = jt` as a double.
#[inline]
fn reduce(x: f64) -> (f64, i64, f64, f64) {
    let t = libm::rint(x * S);
    #[allow(
        clippy::cast_possible_truncation,
        reason = "t is a whole number with |t| < 2^22.1 (proof, step R), so it converts exactly"
    )]
    let jt = t as i64;
    // Masked: in range of the 64-entry tables. C: `(jt >> 6) & 0x3f` and `jt & 0x3f`, the same
    // bits of jt's two's complement as these.
    let i0 = (unsigned(jt) >> 6) & 0x3f;
    let i1 = unsigned(jt) & 0x3f;
    let ie = jt >> 12;
    let [t0l, t0h] = entry(&T0, i0);
    let [t1l, t1h] = entry(&T1, i1);
    let (th, tl) = mul_dd(t0h, t0l, t1h, t1l);
    (t, ie, th, tl)
}

/// The correctly rounded result for a listed hard case, else `f` (C: `as_exp_database`). The
/// table is sorted by input; the value returned is still to be scaled by `2^ie`.
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the C source's variables, so the two can be read side by side"
)]
fn hard_case(x: f64, f: f64) -> f64 {
    // Binary search with the invariant HARD[a].x ≤ x < HARD[b].x (b = len meaning +∞), so a < b
    // throughout: C's `a + 1 < b` is `b − a > 1`, and its `(a + b) / 2` the midpoint rounded down.
    let (mut a, mut b) = (0usize, HARD.len());
    while b.abs_diff(a) > 1 {
        let m = a.midpoint(b);
        if x < entry(&HARD, m)[0] {
            b = m;
        } else {
            a = m;
        }
    }
    let [hx, hr, hd] = entry(&HARD, a);
    if x == hx { hr + hd } else { f }
}

/// The accurate path, for `|x| > 2^−54` (C: `as_exp_accurate`): the reduced argument in
/// double-double and a degree-7 polynomial for `e^dx − 1` in double-double.
#[cold]
#[inline(never)]
fn exp_accurate(x: f64) -> f64 {
    /// `Σ CH[k]·dx^(k+1)` approximates `e^dx − 1` for `|dx| < 2^−13.528`.
    #[rustfmt::skip]
    const CH: [[f64; 2]; 7] = [
        [hf!("0x1p+0"), hf!("0x0p+0")],
        [hf!("0x1p-1"), hf!("0x1.712f72ecec2cfp-99")],
        [hf!("0x1.5555555555555p-3"), hf!("0x1.5555555554d07p-57")],
        [hf!("0x1.5555555555555p-5"), hf!("0x1.55194d28275dap-59")],
        [hf!("0x1.1111111111111p-7"), hf!("0x1.12faa0e1c0f7bp-63")],
        [hf!("0x1.6c16c16da6973p-10"), hf!("-0x1.4ba45ab25d2a3p-64")],
        [hf!("0x1.a01a019eb7f31p-13"), hf!("-0x1.9091d845ecd36p-67")],
    ];
    let (t, ie, th, tl) = reduce(x);
    // Cody–Waite: L2H·t is exact, so dx = x − L2H·t is exact (Sterbenz).
    let dx = x - L2H * t;
    let dxl = L2L * t;
    let dxll = L2LL * t + fma(L2L, t, -dxl);
    let (dxh, dxl) = fast_two_sum(dx, dxl);
    let dxl = dxl + dxll;
    let (fh, fl) = poly_dd(dxh, dxl, &CH);
    let (fh, fl) = mul_dd(dxh, dxl, fh, fl);
    let (fh, fl) = mul_dd(fh, fl, th, tl);
    let (fh, fl) = fast_sum(th, tl, fh, fl);
    if x.to_bits() > SUBNORMAL_THRESHOLD {
        // x < −0x1.6232bdd7abcd2p+9: add 2^(−1022−ie) so that the sum's last place is the last
        // place of the subnormal result, round once, then read the significand field. C: the
        // field `1 − ie`; here −1076 ≤ ie ≤ −1022, so −1022 − ie is 0 to 54.
        let offset = pow2((-1022i64).checked_sub(ie));
        let (fh, e) = fast_two_sum(offset, fh);
        significand_as_subnormal(fh + (fl + e))
    } else {
        let (fh, fl) = fast_two_sum(fh, fl);
        // A low part whose significand is within 4 units of a power of two flags a possible
        // hard case. C: unsigned addition, of which only the significand field is kept, so the
        // sum is wanted modulo 2^52.
        let d = fl.to_bits().wrapping_add(4) & ((1u64 << 52) - 1);
        let fh = if d <= 4 { hard_case(x, fh) } else { fh };
        add_exponent(fh, ie)
    }
}

/// e to the power x, correctly rounded.
///
/// Error at most half an ulp for every input checked, subnormal results included, and proven
/// below `½ + 2^−49.2` ulp for every input (`½ + 2^−34` ulp for results below 2^−1059); see the
/// module documentation for the method, the source, the proof and what correct rounding rests on.
/// `exp(±0) = 1`, `exp(−∞) = +0`, `exp(+∞) = +∞`; overflow gives `+∞` and underflow `+0`, as
/// correct rounding requires; NaN gives the canonical NaN, `0x7ff8_0000_0000_0000`.
#[must_use]
pub fn exp(x: f64) -> f64 {
    /// `1 + dx·(CH[0] + CH[1]·dx + CH[2]·dx² + CH[3]·dx³)` approximates `e^dx` on
    /// `|dx| ≤ 0x1.62e44p−14` with absolute error below 2^−76 (proof, steps R and F2).
    const CH: [f64; 4] = [
        hf!("0x1p+0"),
        hf!("0x1p-1"),
        hf!("0x1.55555557e54ffp-3"),
        hf!("0x1.55555553a12f4p-5"),
    ];
    let ix = x.to_bits();
    let aix = ix & (u64::MAX >> 1);
    if aix <= 0x3c90_0000_0000_0000 {
        // |x| ≤ 2^−54: e^x rounds to 1 (and 1 + x does too).
        return 1.0 + x;
    }
    if aix >= 0x4086_2e42_fefa_39f0 {
        // |x| ≥ 0x1.62e42fefa39fp+9
        if aix > 0x7ff0_0000_0000_0000 {
            return super::NAN;
        }
        if aix == 0x7ff0_0000_0000_0000 {
            return if ix >> 63 != 0 { 0.0 } else { x };
        }
        if ix >> 63 == 0 {
            return f64::INFINITY; // overflow
        }
        if aix >= 0x4087_4910_d52d_3052 {
            return 0.0; // x ≤ −0x1.74910d52d3052p+9: underflow to +0
        }
    }
    let (t, ie, th, tl) = reduce(x);
    let dx = (x - L2H * t) + L2L * t;
    let dx2 = dx * dx;
    let p = (CH[0] + dx * CH[1]) + dx2 * (CH[2] + dx * CH[3]);
    let tx = th * dx;
    let fl = tl + tx * p;
    if ix > SUBNORMAL_THRESHOLD {
        // Possibly subnormal: see `exp_accurate`.
        let offset = pow2((-1022i64).checked_sub(ie));
        let (fh, e) = fast_two_sum(offset, th);
        let fl = fl + e;
        let ub = fh + (fl + EPS);
        let lb = fh + (fl - EPS);
        if ub != lb {
            return exp_accurate(x);
        }
        significand_as_subnormal(lb)
    } else {
        let ub = th + (fl + EPS);
        let lb = th + (fl - EPS);
        if ub != lb {
            return exp_accurate(x);
        }
        add_exponent(lb, ie)
    }
}

/// `2^(i/64)` for `0 ≤ i < 64` as `[low, high]` (C: `t0`). Some entries differ slightly from the
/// nearest double-double, chosen by CORE-MATH to minimise the accurate path's hard cases.
#[rustfmt::skip]
const T0: [[f64; 2]; 64] = [
    [hf!("0x0p+0"), hf!("0x1p+0")],
    [hf!("-0x1.19083535b085cp-56"), hf!("0x1.02c9a3e778061p+0")],
    [hf!("0x1.d73e2a475b466p-55"), hf!("0x1.059b0d3158574p+0")],
    [hf!("0x1.186be4bb284fdp-57"), hf!("0x1.0874518759bc8p+0")],
    [hf!("0x1.8a62e4adc610bp-54"), hf!("0x1.0b5586cf9890fp+0")],
    [hf!("0x1.03a1727c57b55p-59"), hf!("0x1.0e3ec32d3d1a2p+0")],
    [hf!("-0x1.6c51039449b3ap-54"), hf!("0x1.11301d0125b51p+0")],
    [hf!("-0x1.32fbf9af1369ep-54"), hf!("0x1.1429aaea92dep+0")],
    [hf!("-0x1.19041b9d78a76p-55"), hf!("0x1.172b83c7d517bp+0")],
    [hf!("0x1.e5b4c7b4968e4p-55"), hf!("0x1.1a35beb6fcb75p+0")],
    [hf!("0x1.e016e00a2643cp-54"), hf!("0x1.1d4873168b9aap+0")],
    [hf!("0x1.dc775814a8496p-55"), hf!("0x1.2063b88628cd6p+0")],
    [hf!("0x1.9b07eb6c70572p-54"), hf!("0x1.2387a6e756238p+0")],
    [hf!("0x1.2bd339940e9dap-55"), hf!("0x1.26b4565e27cddp+0")],
    [hf!("0x1.612e8afad1255p-55"), hf!("0x1.29e9df51fdee1p+0")],
    [hf!("0x1.0024754db41d4p-54"), hf!("0x1.2d285a6e4030bp+0")],
    [hf!("0x1.6f46ad23182e4p-55"), hf!("0x1.306fe0a31b715p+0")],
    [hf!("0x1.32721843659a6p-54"), hf!("0x1.33c08b26416ffp+0")],
    [hf!("-0x1.63aeabf42eaep-54"), hf!("0x1.371a7373aa9cbp+0")],
    [hf!("-0x1.5e436d661f5dbp-56"), hf!("0x1.3a7db34e59ff7p+0")],
    [hf!("0x1.ada0911f09ebap-55"), hf!("0x1.3dea64c123422p+0")],
    [hf!("-0x1.ef3691c309278p-58"), hf!("0x1.4160a21f72e2ap+0")],
    [hf!("0x1.89b7a04ef80dp-59"), hf!("0x1.44e086061892dp+0")],
    [hf!("0x1.3c1a3b69062fp-56"), hf!("0x1.486a2b5c13cdp+0")],
    [hf!("0x1.d4397afec42e2p-56"), hf!("0x1.4bfdad5362a27p+0")],
    [hf!("-0x1.4b309d25957e3p-54"), hf!("0x1.4f9b2769d2ca7p+0")],
    [hf!("-0x1.07abe1db13cabp-55"), hf!("0x1.5342b569d4f82p+0")],
    [hf!("0x1.9bb2c011d93acp-54"), hf!("0x1.56f4736b527dap+0")],
    [hf!("0x1.6324c054647acp-54"), hf!("0x1.5ab07dd485429p+0")],
    [hf!("0x1.ba6f93080e65ep-54"), hf!("0x1.5e76f15ad2148p+0")],
    [hf!("-0x1.383c17e40b497p-54"), hf!("0x1.6247eb03a5585p+0")],
    [hf!("-0x1.bb60987591c34p-54"), hf!("0x1.6623882552225p+0")],
    [hf!("-0x1.bdd3413b26456p-54"), hf!("0x1.6a09e667f3bcdp+0")],
    [hf!("-0x1.bbe3a683c88aap-57"), hf!("0x1.6dfb23c651a2fp+0")],
    [hf!("-0x1.16e4786887a9ap-55"), hf!("0x1.71f75e8ec5f74p+0")],
    [hf!("-0x1.0245957316dd4p-54"), hf!("0x1.75feb564267c9p+0")],
    [hf!("-0x1.41577ee04993p-55"), hf!("0x1.7a11473eb0187p+0")],
    [hf!("0x1.05d02ba15797ep-56"), hf!("0x1.7e2f336cf4e62p+0")],
    [hf!("-0x1.d4c1dd41532d8p-54"), hf!("0x1.82589994cce13p+0")],
    [hf!("-0x1.fc6f89bd4f6bap-54"), hf!("0x1.868d99b4492edp+0")],
    [hf!("0x1.6e9f156864b29p-54"), hf!("0x1.8ace5422aa0dbp+0")],
    [hf!("0x1.5cc13a2e3976fp-55"), hf!("0x1.8f1ae99157736p+0")],
    [hf!("-0x1.75fc781b57ebcp-57"), hf!("0x1.93737b0cdc5e5p+0")],
    [hf!("-0x1.d185b7c1b85dp-54"), hf!("0x1.97d829fde4e5p+0")],
    [hf!("0x1.c7c46b071f2bep-56"), hf!("0x1.9c49182a3f09p+0")],
    [hf!("-0x1.359495d1cd532p-54"), hf!("0x1.a0c667b5de565p+0")],
    [hf!("-0x1.d2f6edb8d41e3p-54"), hf!("0x1.a5503b23e255dp+0")],
    [hf!("0x1.0fac90ef7fd32p-54"), hf!("0x1.a9e6b5579fdbfp+0")],
    [hf!("0x1.7a1cd345dcc82p-54"), hf!("0x1.ae89f995ad3adp+0")],
    [hf!("-0x1.2805e3084d708p-57"), hf!("0x1.b33a2b84f15fbp+0")],
    [hf!("-0x1.5584f7e54ac3bp-56"), hf!("0x1.b7f76f2fb5e47p+0")],
    [hf!("0x1.23dd07a2d9e84p-55"), hf!("0x1.bcc1e904bc1d2p+0")],
    [hf!("0x1.11065895048dep-55"), hf!("0x1.c199bdd85529cp+0")],
    [hf!("0x1.2884dff483cadp-54"), hf!("0x1.c67f12e57d14bp+0")],
    [hf!("0x1.503cbd1e949d9p-56"), hf!("0x1.cb720dcef9069p+0")],
    [hf!("-0x1.cbc3743797a9cp-54"), hf!("0x1.d072d4a07897cp+0")],
    [hf!("0x1.2ed02d75b3706p-55"), hf!("0x1.d5818dcfba487p+0")],
    [hf!("0x1.c2300696db533p-54"), hf!("0x1.da9e603db3285p+0")],
    [hf!("-0x1.1a5cd4f184b5cp-54"), hf!("0x1.dfc97337b9b5fp+0")],
    [hf!("0x1.39e8980a9cc9p-55"), hf!("0x1.e502ee78b3ff6p+0")],
    [hf!("-0x1.e9c23179c2894p-54"), hf!("0x1.ea4afa2a490dap+0")],
    [hf!("0x1.dc7f486a4b6b1p-54"), hf!("0x1.efa1bee615a27p+0")],
    [hf!("0x1.9d3e12dd8a18dp-54"), hf!("0x1.f50765b6e454p+0")],
    [hf!("0x1.74853f3a5931fp-55"), hf!("0x1.fa7c1819e90d8p+0")],
];

/// `2^(i/4096)` for `0 ≤ i < 64` as `[low, high]` (C: `t1`), adjusted like [`T0`].
#[rustfmt::skip]
const T1: [[f64; 2]; 64] = [
    [hf!("0x0p+0"), hf!("0x1p+0")],
    [hf!("0x1.ae8e38c59c72cp-54"), hf!("0x1.000b175effdc7p+0")],
    [hf!("-0x1.7b5d0d58ea8f4p-58"), hf!("0x1.00162f3904052p+0")],
    [hf!("0x1.4115cb6b16a8dp-54"), hf!("0x1.0021478e11ce6p+0")],
    [hf!("-0x1.d7c96f201bb2ep-55"), hf!("0x1.002c605e2e8cfp+0")],
    [hf!("0x1.84711d4c35eap-54"), hf!("0x1.003779a95f959p+0")],
    [hf!("-0x1.0484245243778p-55"), hf!("0x1.0042936faa3d8p+0")],
    [hf!("-0x1.4b237da2025fap-54"), hf!("0x1.004dadb113dap+0")],
    [hf!("-0x1.5e00e62d6b30ep-56"), hf!("0x1.0058c86da1c0ap+0")],
    [hf!("0x1.a1d6cedbb948p-54"), hf!("0x1.0063e3a559473p+0")],
    [hf!("-0x1.4acf197a00141p-54"), hf!("0x1.006eff583fc3dp+0")],
    [hf!("-0x1.eaf2ea42391a6p-57"), hf!("0x1.007a1b865a8cap+0")],
    [hf!("0x1.da93f90835f7ap-56"), hf!("0x1.0085382faef83p+0")],
    [hf!("-0x1.6a79084ab0937p-55"), hf!("0x1.00905554425d4p+0")],
    [hf!("0x1.86364f8fbe8f8p-54"), hf!("0x1.009b72f41a12bp+0")],
    [hf!("-0x1.82e8e14e3110bp-55"), hf!("0x1.00a6910f3b6fdp+0")],
    [hf!("-0x1.4f6b2a7609f72p-55"), hf!("0x1.00b1afa5abcbfp+0")],
    [hf!("-0x1.e1a258ea8f71ap-56"), hf!("0x1.00bcceb7707ecp+0")],
    [hf!("0x1.4362ca5bc26f2p-56"), hf!("0x1.00c7ee448ee02p+0")],
    [hf!("0x1.095a56c919d02p-54"), hf!("0x1.00d30e4d0c483p+0")],
    [hf!("-0x1.406ac4e81a646p-57"), hf!("0x1.00de2ed0ee0f5p+0")],
    [hf!("0x1.b5a6902767e08p-54"), hf!("0x1.00e94fd0398ep+0")],
    [hf!("-0x1.91b206085932p-54"), hf!("0x1.00f4714af41d3p+0")],
    [hf!("0x1.427068ab22306p-55"), hf!("0x1.00ff93412315cp+0")],
    [hf!("0x1.c1d0660524e08p-54"), hf!("0x1.010ab5b2cbd11p+0")],
    [hf!("-0x1.e7bdfb3204be8p-54"), hf!("0x1.0115d89ff3a8bp+0")],
    [hf!("0x1.843aa8b9cbbc6p-55"), hf!("0x1.0120fc089ff63p+0")],
    [hf!("-0x1.34104ee7edae8p-56"), hf!("0x1.012c1fecd613bp+0")],
    [hf!("-0x1.2b6aeb6176892p-56"), hf!("0x1.0137444c9b5b5p+0")],
    [hf!("0x1.a8cd33b8a1bb3p-56"), hf!("0x1.01426927f5278p+0")],
    [hf!("0x1.2edc08e5da99ap-56"), hf!("0x1.014d8e7ee8d2fp+0")],
    [hf!("0x1.57ba2dc7e0c72p-55"), hf!("0x1.0158b4517bb88p+0")],
    [hf!("0x1.b61299ab8cdb8p-54"), hf!("0x1.0163da9fb3335p+0")],
    [hf!("-0x1.90565902c5f43p-54"), hf!("0x1.016f0169949edp+0")],
    [hf!("0x1.70fc41c5c2d54p-55"), hf!("0x1.017a28af25567p+0")],
    [hf!("0x1.4b9a6e145d76cp-54"), hf!("0x1.018550706ab62p+0")],
    [hf!("-0x1.008eff5142bfap-56"), hf!("0x1.019078ad6a19fp+0")],
    [hf!("-0x1.77669f033c7dep-54"), hf!("0x1.019ba16628de2p+0")],
    [hf!("-0x1.09bb78eeead0ap-54"), hf!("0x1.01a6ca9aac5f3p+0")],
    [hf!("0x1.371231477ece4p-54"), hf!("0x1.01b1f44af9f9ep+0")],
    [hf!("0x1.5e7626621eb5ap-56"), hf!("0x1.01bd1e77170b4p+0")],
    [hf!("-0x1.bc72b100828a4p-54"), hf!("0x1.01c8491f08f08p+0")],
    [hf!("-0x1.ce39cbbab8bbep-57"), hf!("0x1.01d37442d507p+0")],
    [hf!("0x1.16996709da2e5p-55"), hf!("0x1.01de9fe280ac8p+0")],
    [hf!("-0x1.c11f5239bf536p-55"), hf!("0x1.01e9cbfe113efp+0")],
    [hf!("0x1.e1d4eb5edc6b4p-55"), hf!("0x1.01f4f8958c1c6p+0")],
    [hf!("-0x1.afb99946ee3fp-54"), hf!("0x1.020025a8f6a35p+0")],
    [hf!("-0x1.8f06d8a148a32p-54"), hf!("0x1.020b533856324p+0")],
    [hf!("-0x1.2bf310fc54eb8p-55"), hf!("0x1.02168143b0281p+0")],
    [hf!("-0x1.c95a035eb4177p-54"), hf!("0x1.0221afcb09e3ep+0")],
    [hf!("-0x1.491793e46834cp-54"), hf!("0x1.022cdece68c4fp+0")],
    [hf!("-0x1.3e8d0d9c4908fp-56"), hf!("0x1.02380e4dd22adp+0")],
    [hf!("-0x1.314aa16278aa4p-54"), hf!("0x1.02433e494b755p+0")],
    [hf!("0x1.48daf888e965p-55"), hf!("0x1.024e6ec0da046p+0")],
    [hf!("0x1.56dc8046821f4p-55"), hf!("0x1.02599fb483385p+0")],
    [hf!("0x1.45b42356b9d46p-54"), hf!("0x1.0264d1244c719p+0")],
    [hf!("-0x1.082ef51b61d7ep-56"), hf!("0x1.027003103b10ep+0")],
    [hf!("0x1.2106ed0920a32p-56"), hf!("0x1.027b357854772p+0")],
    [hf!("-0x1.fd4cf26ea5d11p-54"), hf!("0x1.0286685c9e059p+0")],
    [hf!("-0x1.09f8775e78085p-54"), hf!("0x1.02919bbd1d1d8p+0")],
    [hf!("0x1.64cbba902ca25p-58"), hf!("0x1.029ccf99d720ap+0")],
    [hf!("0x1.4383ef231d208p-54"), hf!("0x1.02a803f2d170dp+0")],
    [hf!("0x1.4a47a505b3a46p-54"), hf!("0x1.02b338c811703p+0")],
    [hf!("0x1.e47120223467fp-54"), hf!("0x1.02be6e199c811p+0")],
];

/// Hard cases of the accurate path as `[x, r, d]`, sorted by `x`: the correctly rounded `e^x`
/// is `(r + d)·2^ie` (C: `db`).
#[rustfmt::skip]
const HARD: [[f64; 3]; 21] = [
    [hf!("-0x1.8f80e06f3a04cp+4"), hf!("0x1.f80aafa92b498p+0"), hf!("-0x1.191595b7e88edp-106")],
    [hf!("-0x1.daf3afee51991p+2"), hf!("0x1.39c6240f78d7p+0"), hf!("-0x1.2b74a3833bf1p-105")],
    [hf!("-0x1.07e05e6e03b83p-2"), hf!("0x1.8bb0d03b36f1cp+0"), hf!("0x1.ffffffffffffep-54")],
    [hf!("-0x1.daf693d64fadap-4"), hf!("0x1.c7f14af0a08ebp+0"), hf!("-0x1.2aa7e4ef70195p-108")],
    [hf!("-0x1.a8f783d749a8fp-4"), hf!("0x1.cd8abd4de5c33p+0"), hf!("0x1.fffffffffffffp-54")],
    [hf!("-0x1.40a990279c2cp-6"), hf!("0x1.f613a44b93cdcp+0"), hf!("0x1.ffffffffffffdp-54")],
    [hf!("-0x1.a2fefefd580dfp-13"), hf!("0x1.ffe5d0bb7eabfp+0"), hf!("0x1.81905a57d6a44p-111")],
    [hf!("-0x1.664716b68a409p-14"), hf!("0x1.fff4cde6a0bfbp+0"), hf!("-0x1.fffffffffffffp-54")],
    [hf!("-0x1.0401ae48409b5p-28"), hf!("0x1.ffffffdf7fca3p-1"), hf!("0x1.fffffffffffffp-55")],
    [hf!("-0x1.ff171507f8ba5p-30"), hf!("0x1.fffffff007475p-1"), hf!("0x1.fffffffffffffp-55")],
    [hf!("-0x1.85e60704a3a9cp-30"), hf!("0x1.fffffff3d0cfdp-1"), hf!("-0x1.fffffffffffffp-55")],
    [hf!("-0x1.4bd46601ae1efp-31"), hf!("0x1.fffffffad0ae7p-1"), hf!("-0x1.fffffffffffffp-55")],
    [hf!("0x1p-53"), hf!("0x1.0000000000001p+0"), hf!("-0x1.fffffffffffffp-54")],
    [hf!("0x1.ffff7fffe0001p-36"), hf!("0x1.000000001ffffp+0"), hf!("0x1.fffffffffffffp-54")],
    [hf!("0x1.8cd99fffb319cp-34"), hf!("0x1.0000000063367p+0"), hf!("-0x1.fffffffffffffp-54")],
    [hf!("0x1.413f33fe6ce07p-31"), hf!("0x1.00000002827e7p+0"), hf!("-0x1.fffffffffffffp-54")],
    [hf!("0x1.f8e165f8388f7p-30"), hf!("0x1.00000007e3859p+0"), hf!("0x1.fffffffffffffp-54")],
    [hf!("0x1.282aae3169ee7p-20"), hf!("0x1.00001282ab8e7p+0"), hf!("-0x1.fffffffffffffp-54")],
    [hf!("0x1.e5ba92aa9b1efp-20"), hf!("0x1.00001e5baaf77p+0"), hf!("0x1.ffffffffffffep-54")],
    [hf!("0x1.6a4d1af9cc989p-8"), hf!("0x1.016b4df3299d7p+0"), hf!("0x1.ffffffffffffep-54")],
    [hf!("0x1.aca7ae8da5a7bp+0"), hf!("0x1.557d4acd7e557p+0"), hf!("-0x1.fffffffffffffp-54")],
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

    /// Step R: the reduction. `|t − x·2^12/ln 2| ≤ ½ + 2^−29.9`, the split of `ln 2/2^12`, and so
    /// the bound `|dx| ≤ R_MAX` on the reduced argument.
    #[test]
    fn reduction_constants() {
        let target = ln2().scale(-12);
        // the code reduces by t·(L2H − L2L) in the fast path and t·(L2H − L2L − L2LL) in the
        // accurate path
        let two = Dy::from_f64(L2H).sub(&Dy::from_f64(L2L));
        let three = two.sub(&Dy::from_f64(L2LL));
        let e2 = dmax(target.lo.sub(&two).abs(), target.hi.sub(&two).abs());
        let e3 = dmax(target.lo.sub(&three).abs(), target.hi.sub(&three).abs());
        eprintln!(
            "|ln2/4096 − (L2H − L2L)| = 2^{:.3}, − L2LL: 2^{:.3}",
            log2_approx(&e2),
            log2_approx(&e3)
        );
        assert!(le(&e2, &p2(-99)) && le(&e3, &p2(-152)));
        // S = RN(2^12/ln 2)
        let s_exact = Iv::point(Dy::pow2(12)).div(&ln2(), P);
        let ds = dmax(
            s_exact.lo.sub(&Dy::from_f64(S)).abs(),
            s_exact.hi.sub(&Dy::from_f64(S)).abs(),
        );
        assert!(
            le(&ds, &Dy::div(&s_exact.hi, &p2(53), P, true)),
            "S is 2^12/ln 2 rounded to nearest"
        );
        // |t − x·2^12/ln 2| ≤ ½ + 746·S·2^−52 (S's rounding and RN(x·S)), so
        // |r| ≤ (½ + 746·S·2^−52)·ln 2/2^12 ≤ R_MAX.
        let slack = Dy::from_f64(746.0 * S).mul(&p2(-52));
        assert!(le(&slack, &p2(-29)), "slack 2^{}", log2_approx(&slack));
        let r_bound = Dy::pow2(-1).add(&slack).mul(&ln2().hi).scale(-12);
        assert!(le(&r_bound, &Dy::from_f64(R_MAX)), "R_MAX");
        // L2H has 29 significant bits, so L2H·t is exact for |t| < 2^23 (t ≤ 746·S < 2^22.1).
        assert_eq!(L2H.to_bits() & ((1 << 24) - 1), 0);
        const { assert!(746.0 * S < 8_388_608.0) }; // 2^23
    }

    /// Steps F1 and A1: the table products `th + tl = mul_dd(T0[i0], T1[i1])` approximate
    /// `2^((64·i0 + i1)/4096)` with relative error at most `τ_e` and `|tl| ≤ σ_e·th`, for all 4096
    /// index pairs.
    #[test]
    fn table_products() {
        let mut worst = Dy::zero();
        let mut sigma = 0f64;
        for (i0, &[t0l, t0h]) in T0.iter().enumerate() {
            for (i1, &[t1l, t1h]) in T1.iter().enumerate() {
                let (th, tl) = mul_dd(t0h, t0l, t1h, t1l);
                let j = (64 * i0 + i1) as i64;
                // 2^(j/4096) = exp(j·ln 2/8192)², each factor's argument below ½
                let half = ln2().mul_int(j, P).div_int(8192, P);
                let h = exp_small_iv(&half, P);
                let v = h.mul(&h, P);
                let err = dd_error(th, tl, &v);
                worst = dmax(worst, Dy::div(&err, &v.lo, P, true));
                // By comparison, not `f64::max` (chapter 21's ban).
                let ratio = tl.abs() / th;
                if ratio > sigma {
                    sigma = ratio;
                }
                assert!((1.0..2.0).contains(&th));
            }
        }
        eprintln!(
            "table products: relative error 2^{:.3}, |tl/th| ≤ {:.3}u",
            log2_approx(&worst),
            sigma / (f64::EPSILON / 2.0) // in units of 2^−53
        );
        assert!(le(&worst, &Dy::from_f64(TAU_E)));
        assert!(sigma <= SIGMA_E);
    }

    /// Step F2: the fast polynomial, `|e^r − 1 − r·Pe(r)| ≤ ε_p` for `|r| ≤ R_MAX`.
    #[test]
    fn fast_polynomial() {
        const CH: [f64; 4] = [
            hf!("0x1p+0"),
            hf!("0x1p-1"),
            hf!("0x1.55555557e54ffp-3"),
            hf!("0x1.55555553a12f4p-5"),
        ];
        let r_max = Dy::from_f64(R_MAX);
        // e^r − 1 − Σ CH[k] r^(k+1): Taylor coefficients 1/n! to degree 12, then a remainder.
        let target: Vec<Iv> = (0..13)
            .map(|n| {
                if n == 0 {
                    Iv::int(0)
                } else {
                    Iv::int(1).div(&Iv::point(Dy::big(factorial(n))), P)
                }
            })
            .collect();
        let coeffs: Vec<(usize, f64)> = CH.iter().enumerate().map(|(k, &c)| (k + 1, c)).collect();
        let q = error_poly(&target, &coeffs, 0, 1);
        // the polynomial in r over [−R_MAX, R_MAX]: bound |q(r)| for r ≥ 0 and r ≤ 0 separately
        let qneg: Vec<Iv> = q
            .iter()
            .enumerate()
            .map(|(j, c)| if j % 2 == 1 { c.neg() } else { c.clone() })
            .collect();
        let sup = dmax(
            poly_sup_bound(&q, &r_max, 400, 192),
            poly_sup_bound(&qneg, &r_max, 400, 192),
        );
        let tail = Dy::div(
            &pow(&r_max, 13).mul(&Dy::int(2)),
            &Dy::big(factorial(13)),
            P,
            true,
        );
        let eps_p = sup.add(&tail);
        eprintln!("fast polynomial: 2^{:.3}", log2_approx(&eps_p));
        assert!(le(&eps_p, &Dy::from_f64(EPS_P)));
    }

    /// Step F3: the fast path's total error and the rounding test. Normal results: Lemma 1 with
    /// `EPS`. Subnormal results: Lemma 1 for `ie ≥ −1060`, Lemma 2 below.
    #[test]
    fn fast_path_bound() {
        let u = u();
        let one = Dy::int(1);
        let up = |d: &Dy, k: i64| d.mul(&one.add(&p2(-k)));
        let r_max = Dy::from_f64(R_MAX);
        let tau = Dy::from_f64(TAU_E);
        let sigma = Dy::from_f64(SIGMA_E);
        let eps_p = Dy::from_f64(EPS_P);
        let t_max = Dy::int(2); // th < 2
        // |dx − r| ≤ u·|L2L·t| + u·|dx| + |t|·|ln2/4096 − L2H − L2L|
        let t_abs = Dy::from_f64(746.0 * S);
        let l2l_t = Dy::from_f64(L2L).abs().mul(&t_abs);
        let d_r = u
            .mul(&l2l_t)
            .add(&u.mul(&up(&r_max, 20)))
            .add(&t_abs.mul(&p2(-99)));
        let dx_max = up(&r_max.add(&d_r), 50);
        // p = Pe(dx)(1 + θ), |θ| ≤ 4u (running error of (1 + dx/2) + dx²·(c2 + dx·c3)); Pe ≤ 1.0001
        let pe = Dy::int(1).add(&dx_max.scale(-1)).add(&dx_max.mul(&dx_max));
        let th_r_pe = t_max.mul(
            &d_r.mul(&up(&pe, 10))
                .add(&dx_max.mul(&d_r).scale(-1).mul(&up(&one, 10)))
                .add(&dx_max.mul(&pe).mul(&u).mul(&Dy::int(5))),
        );
        let txp = up(&t_max.mul(&dx_max).mul(&pe), 49);
        let fl = up(&sigma.mul(&t_max).add(&txp), 50);
        let e = tau
            .mul(&t_max)
            .mul(&up(&one, 10))
            .add(&t_max.mul(&eps_p))
            .add(&sigma.mul(&t_max).mul(&dx_max).mul(&pe))
            .add(&th_r_pe)
            .add(&u.mul(&txp))
            .add(&u.mul(&fl));
        let need = e.add(&u.mul(&fl));
        let have = Dy::from_f64(EPS).mul(&one.sub(&u));
        eprintln!(
            "fast path: 2^{:.3} ≤ 2^{:.3}",
            log2_approx(&need),
            log2_approx(&have)
        );
        assert!(le(&need, &have), "F3, normal results");
        // Subnormal results, ie ≥ −1060: the extra low-part roundings 2u·|e| ≤ u·ulp(fh) ≤ 2^−67.
        let extra = p2(-53 - 1022 + 1060 - 52);
        assert!(
            le(&need.add(&extra).add(&u.mul(&extra)), &have),
            "F3, subnormal, ie ≥ −1060"
        );
        // ie < −1060: ulp(fh) ≥ 2^−13, and the value rounded, fh + RN(RN(fl + e) ± EPS), is within
        // e + EPS + the low-part roundings of the exact value: below 2^−34 ulp (Lemma 2
        // with the subnormal list's 33 identical bits).
        let ulp_min = p2(-13);
        let near = e.add(&Dy::from_f64(EPS)).add(&u.mul(&ulp_min).scale(1));
        let rel = Dy::div(&near, &ulp_min, P, true).add(&u.scale(1));
        assert!(le(&rel, &p2(-34)), "F3, deep subnormal");
    }

    /// Step A2: the accurate polynomial and reduction are accurate to far below 2^−95.
    #[test]
    fn accurate_path_bound() {
        const CH: [[f64; 2]; 7] = [
            [hf!("0x1p+0"), hf!("0x0p+0")],
            [hf!("0x1p-1"), hf!("0x1.712f72ecec2cfp-99")],
            [hf!("0x1.5555555555555p-3"), hf!("0x1.5555555554d07p-57")],
            [hf!("0x1.5555555555555p-5"), hf!("0x1.55194d28275dap-59")],
            [hf!("0x1.1111111111111p-7"), hf!("0x1.12faa0e1c0f7bp-63")],
            [hf!("0x1.6c16c16da6973p-10"), hf!("-0x1.4ba45ab25d2a3p-64")],
            [hf!("0x1.a01a019eb7f31p-13"), hf!("-0x1.9091d845ecd36p-67")],
        ];
        let u = u();
        let uu = u.mul(&u);
        let r_max = Dy::from_f64(R_MAX).mul(&Dy::int(1).add(&p2(-20)));
        // CH (double-double) for e^r − 1 = Σ r^(k+1)·CH[k], degree 7: coefficient errors and
        // the remainder r^8/8!·e^r.
        let mut q: Vec<Iv> = (0..14)
            .map(|n| {
                if n == 0 {
                    Iv::int(0)
                } else {
                    Iv::int(1).div(&Iv::point(Dy::big(factorial(n))), P)
                }
            })
            .collect();
        for (k, c) in CH.iter().enumerate() {
            q[k + 1] = q[k + 1].sub(&Iv::point(Dy::from_f64(c[0]).add(&Dy::from_f64(c[1]))), P);
        }
        let qneg: Vec<Iv> = q
            .iter()
            .enumerate()
            .map(|(j, c)| if j % 2 == 1 { c.neg() } else { c.clone() })
            .collect();
        let sup = dmax(
            poly_sup_bound(&q, &r_max, 400, 256),
            poly_sup_bound(&qneg, &r_max, 400, 256),
        );
        let eps_a = sup.add(&Dy::div(
            &pow(&r_max, 14).mul(&Dy::int(2)),
            &Dy::big(factorial(14)),
            P,
            true,
        ));
        // Step A2's terms, relative to the result (≥ th·e^(−R_MAX), th ≥ 1): the table τ_e; the
        // final fast sum (≤ 3u²); the polynomial branch's roundings, scaled by |r| ≤ R_MAX: the
        // Horner steps (≤ 7.1u² of a value ≤ 1.0001), the product by dx (≤ 12.6u²) and by th + tl
        // (≤ 17.8u², with th < 2); the approximation ε_a; the reduction's low parts (≤ 2^−118).
        let branch = uu
            .mul(&Dy::from_f64(7.1 + 12.6 + 17.8))
            .mul(&r_max)
            .mul(&Dy::int(2));
        let e = Dy::from_f64(TAU_E)
            .add(&uu.mul(&Dy::int(3)))
            .add(&branch)
            .add(&eps_a.mul(&Dy::int(2)))
            .add(&p2(-118))
            .mul(&Dy::int(1).add(&p2(-12)));
        eprintln!(
            "accurate polynomial 2^{:.3}; accurate path relative ≤ 2^{:.3}",
            log2_approx(&eps_a),
            log2_approx(&e)
        );
        assert!(le(&e, &p2(-95)), "A2");
    }
}
