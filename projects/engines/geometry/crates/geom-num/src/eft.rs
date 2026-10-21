//! Error-free transformations: a rounded sum or product of two doubles together with its exact error.
//!
//! `two_sum(a, b)` returns `(s, e)` where `s` is `a + b` rounded and `s + e = a + b` exactly, in real
//! arithmetic. `two_prod(a, b)` returns `(p, e)` where `p` is `a · b` rounded and `p + e = a · b`
//! exactly. They are the building blocks of double-double arithmetic (the one-time conversion of
//! survey and map data into a frame, chapter 6.1) and of floating-point expansions.
//!
//! What every function here guarantees, for every input (there are no preconditions):
//!
//! - **Exact or reported, never silently wrong.** For finite inputs the pair is exact whenever an
//!   exact pair exists, and then both parts are finite. When none exists, the second part is an
//!   infinity with the sign of the exact error, so one check, `e.is_finite()`, tells a caller whether
//!   the pair is exact. No exact pair exists only when the rounded sum or product overflows, when the
//!   high half of a split would be ±2^1024, or when the exact error of a product is finer than the
//!   smallest subnormal 2^−1074 (possible only for |a · b| < 2^−969). The tests check every case
//!   against exact rationals (`num-rational`) on random and adversarial inputs.
//! - **Never NaN from finite inputs.** The second part is NaN only when the first part is NaN: a NaN
//!   input, or an IEEE operation on infinities that is itself NaN (∞ − ∞, 0 · ∞). Any other non-finite
//!   input gives an infinite second part. So the crate's "never NaN" invariant (`lib.rs`) holds here
//!   for every finite input; for infinite inputs it holds except where IEEE 754 itself makes the
//!   rounded sum or product NaN.
//! - **The rounded part is the IEEE 754 result.** `s` is `a + b` and `p` is `a * b` as Rust computes
//!   them (round to nearest, ties to even). An exact error is at most half a unit in the last place of
//!   that result, so the pair does not overlap: `s + e` rounds back to `s`.
//! - **Deterministic.** Only `+`, `−`, `×`, comparisons and bit reads on `f64`. Rust computes these to
//!   IEEE 754 binary64, each rounded once, on every target the kernel builds for (RFC 3514). The RFC
//!   names the exception itself: on 32-bit x86 without SSE2 (the i586 targets) the x87 unit computes
//!   with a different precision and so can round twice, which would break these algorithms; the
//!   kernel does not support those targets. The RFC also rules out fusing `a * b + c` into one fused
//!   multiply-add, which would break them too, and none is used on purpose either: `two_prod` follows
//!   Dekker's splitting instead of `mul_add`, which `core` lacks and which is a slow software call on
//!   wasm32 (evidence note 05, section 1).
//!
//! Method: Dekker 1971 (`fast_two_sum`, Veltkamp's `split`, `two_prod`) and Knuth (`two_sum`), in the
//! form and with the proofs of Shewchuk 1997: Theorem 6 (fast two-sum), Theorem 7 (two-sum),
//! Theorem 17 (split) and Theorem 18 (two-product). Shewchuk's proofs assume an unbounded exponent
//! (his section 2.1 leaves overflow and underflow aside). Each function runs Shewchuk's algorithm
//! unchanged where the conditions derived below for binary64 make it exact, and otherwise first moves
//! its inputs into that range by exact powers of two, then checks that the way back is exact.
//!
//! - T. J. Dekker, "A floating-point technique for extending the available precision", *Numerische
//!   Mathematik* 18 (1971), 224–242, <https://doi.org/10.1007/BF01397083>.
//! - J. R. Shewchuk, "Adaptive precision floating-point arithmetic and fast robust geometric
//!   predicates", *Discrete & Computational Geometry* 18 (1997), 305–363,
//!   <https://doi.org/10.1007/PL00009321>.
//! - D. E. Knuth, *The Art of Computer Programming*, volume 2, section 4.2.2.
//! - RFC 3514, "Float semantics": <https://rust-lang.github.io/rfcs/3514-float-semantics.html>.
//!
//! Notation used in the proofs: Ω = (2 − 2^−52) · 2^1023 is the largest finite double. For a nonzero
//! finite double x, e_x is the exponent of its leading bit, so 2^e_x ≤ |x| < 2^(e_x + 1); e_x runs
//! from −1074 (the smallest subnormal) to 1023, and for a normal x it is the binary exponent. The
//! quantum of x is the value of the last bit of its format: u_x = 2^(e_x − 52) for a normal x and
//! 2^−1074 for a subnormal one. Every double is a whole multiple of its own quantum, and of 2^−1074.
//!
//! Two facts about gradual underflow carry Shewchuk's proofs over to binary64:
//!
//! - A sum or difference of two doubles that lands below 2^−1022 is exact: its exact value is a whole
//!   multiple of 2^−1074 below 2^−1022, which is a double. So gradual underflow never spoils an
//!   addition, and Sterbenz's lemma (y/2 ≤ x ≤ 2y makes x − y exact) holds in the subnormal range.
//! - More generally, if the exact result of an operation is a whole multiple of 2^−1074, binary64 and
//!   Shewchuk's unbounded 53-bit arithmetic give the same rounded value: below 2^−1022 such a result
//!   has at most 52 significant bits, so neither rounds it, and from 2^−1022 up both round to 53 bits.

/// Veltkamp's splitting constant for binary64, 2^27 + 1: splits a 53-bit significand into two
/// halves of 26 bits each (Shewchuk 1997, Theorem 17, with splitting point s = 27).
pub const SPLITTER: f64 = 134_217_729.0;

/// 2^k for −1022 ≤ k ≤ 1023, a normal double, built from its bits.
const fn pow2(k: i32) -> f64 {
    f64::from_bits(((k + 1023) as u64) << 52)
}

/// 2^996: below it, [`split`] runs Veltkamp's method directly and cannot overflow.
const TWO_POW_996: f64 = pow2(996);

/// 2^1022: a product below it has e_a + e_b ≤ 1021, the upper limit of Dekker's direct product.
const TWO_POW_1022: f64 = pow2(1022);

/// 2^−968: a product at or above it has e_a + e_b ≥ −970, the lower limit of Dekker's direct product.
const TWO_POW_NEG_968: f64 = pow2(-968);

/// 2^64: the exact scaling [`split`] undoes above 2^996.
const TWO_POW_64: f64 = pow2(64);

/// 2^−64: the exact scaling [`split`] applies above 2^996.
const TWO_POW_NEG_64: f64 = pow2(-64);

/// 2^1000: the largest step up [`scale`] takes.
const TWO_POW_1000: f64 = pow2(1000);

/// 2^−1000: the largest step down [`scale`] takes.
const TWO_POW_NEG_1000: f64 = pow2(-1000);

/// The magnitude of `x` by comparison: −x for x < 0, x otherwise. −0 stays −0, which compares equal to
/// +0, and NaN stays NaN, which compares false, so every comparison of the result is that of |x|.
const fn abs(x: f64) -> f64 {
    if x < 0.0 { -x } else { x }
}

/// e_x, the exponent of the leading bit of a finite nonzero `x` (2^e_x ≤ |x| < 2^(e_x + 1)), from
/// −1074 to 1023, read from the bits. For a normal x it is the biased exponent minus 1023. A subnormal
/// x is its 52-bit fraction times 2^−1074, so e_x is the position of the fraction's leading bit
/// (63 minus its leading zeros in 64 bits) minus 1074. Never called for zero, NaN or an infinity.
const fn exponent(x: f64) -> i32 {
    let bits = x.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    if biased == 0 {
        let fraction = bits & ((1 << 52) - 1);
        63 - fraction.leading_zeros() as i32 - 1074
    } else {
        biased - 1023
    }
}

/// `x` · 2^k, by at most three products with powers of two (steps of 2^±1000, then the rest), for
/// |k| up to 3000.
///
/// Exact whenever the exact value x · 2^k is a double: the steps all go the same way, so every
/// intermediate lies between |x| and |x · 2^k| in magnitude (so at most Ω), has the significand of
/// `x`, and is a whole multiple of 2^−1074 (it is `x` times a non-negative power of two, or the exact
/// result times one). Such a value is a double, so no step rounds. When the exact value is not a
/// double the result is some rounding of it; the callers detect that by scaling back.
const fn scale(x: f64, k: i32) -> f64 {
    let mut x = x;
    let mut k = k;
    while k > 1000 {
        x *= TWO_POW_1000;
        k -= 1000;
    }
    while k < -1000 {
        x *= TWO_POW_NEG_1000;
        k += 1000;
    }
    x * pow2(k)
}

/// The sum `a + b` rounded, and its exact error, by Dekker's fast two-sum with the operands put in
/// order of magnitude first.
///
/// Returns `(s, e)` with `s = a + b` rounded to nearest (the IEEE sum) and `s + e = a + b` exactly,
/// `|e|` at most half a unit in the last place of `s`, for every finite `a` and `b` whose rounded sum
/// is finite. Dekker's method needs the operand of larger magnitude first; this function orders the
/// operands itself (one comparison), so it asks nothing of the caller and its result equals
/// [`two_sum`]'s.
///
/// If `s` is not finite (the sum overflows, or an input is not finite) the result is `(s, −s)`: the
/// opposite infinity, or NaN when `s` is NaN (∞ − ∞, or a NaN input).
///
/// Bit budget and proof (Dekker 1971; Shewchuk 1997, Theorem 6), with `x` the operand of larger
/// magnitude, `y` the other, so `|x| ≥ |y|`, and `s` finite:
/// - `z = s − x` is exact by Sterbenz's lemma. If `x` and `y` have the same sign, `|x| ≤ |s| ≤ 2|x|`
///   with the sign of `x` (rounding is monotone and `x`, `2x` are doubles). If their signs differ and
///   `|y| ≥ |x|/2`, then `x + y` itself is exact (Sterbenz), so `s = x + y` and `z = y`. Otherwise
///   `|x|/2 ≤ |s| ≤ |x|`, again with the sign of `x`.
/// - `e = y − z` is exact: its exact value is the rounding error `x + y − s`. `x`, `y` and `s` are
///   all whole multiples of u_y, the quantum of `y`: `|x| ≥ |y|` makes u_x ≥ u_y, and `s` is `x + y`
///   rounded to a grid no finer than u_y, or `x + y` itself. So the error is a whole multiple of u_y,
///   and it is at most `|y|` in magnitude (`x` is a double, so the nearest double `s` is no farther
///   from `x + y` than `x` is). A whole multiple of u_y below 2^53 · u_y in magnitude has at most 53
///   significant bits, so it is a double.
/// - Both steps are exact, so neither overflows, and gradual underflow changes nothing (module
///   documentation). An overflowed `s` is caught before them.
#[must_use]
pub const fn fast_two_sum(a: f64, b: f64) -> (f64, f64) {
    let (x, y) = if abs(b) > abs(a) { (b, a) } else { (a, b) };
    let s = x + y;
    if !s.is_finite() {
        return (s, -s);
    }
    let z = s - x;
    (s, y - z)
}

/// The sum `a + b` rounded, and its exact error, for any finite `a` and `b` (Knuth's two-sum).
///
/// Returns `(s, e)` with `s = a + b` rounded to nearest (the IEEE sum) and `s + e = a + b` exactly,
/// `|e|` at most half a unit in the last place of `s`, whenever `s` is finite. No ordering of `a` and
/// `b` is needed. If `s` is not finite the result is `(s, −s)`, as for [`fast_two_sum`].
///
/// Bit budget: the error-free pair is unique (`s` is the IEEE sum and `e` the exact remainder), so it
/// is the pair of [`fast_two_sum`] and has its budget: `e` is a whole multiple of the quantum of the
/// operand of smaller magnitude and at most that operand in magnitude, so it has at most 53
/// significant bits. Knuth's six operations reach it without a branch; each is exact by Shewchuk
/// 1997, Theorem 7 (a case analysis with Sterbenz's lemma, like the fast two-sum's), with or without
/// gradual underflow, provided no intermediate overflows.
///
/// Overflow: with a bounded exponent one intermediate can overflow although `s` does not: for
/// `a = −3 · 2^970` and `b = Ω`, `s = Ω − 2^971` (a tie, rounded to even), and `s − a` is exactly
/// Ω + 2^970, the midpoint between Ω and 2^1024, which rounds to infinity. That can only happen when
/// `|b| > |a|`: when `|a| ≥ |b|`, `s − a` and `b − (s − a)` are exact (Shewchuk 1997, Theorem 6), the
/// next value is exactly `a`, and every step is exact. An infinity, once made, is never absorbed by a
/// later addition (each gives an infinity or NaN), and every intermediate feeds `e`, so any
/// intermediate overflow shows as a non-finite `e`. In that case the function returns
/// [`fast_two_sum`]`(b, a)`, which is exact and has the same `s` because IEEE addition is commutative.
/// So the result is exact for every finite input whose rounded sum is finite. If `s` overflows, or an
/// input is not finite, `e` is not finite either and the same fallback returns `(s, −s)`.
#[must_use]
pub const fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let b_virtual = s - a;
    let a_virtual = s - b_virtual;
    let e = (a - a_virtual) + (b - b_virtual);
    if e.is_finite() { (s, e) } else { fast_two_sum(b, a) }
}

/// Veltkamp's split exactly as published (Shewchuk 1997, Theorem 17): four operations, exact for
/// every `a` with |a| < 2^996, subnormals included. [`split`] carries the proof.
const fn veltkamp(a: f64) -> (f64, f64) {
    let c = SPLITTER * a;
    let big = c - a;
    let hi = c - big;
    let lo = a - hi;
    (hi, lo)
}

/// Veltkamp's split of `a` into a high and a low half: `a = hi + lo` exactly, each half a
/// 26-bit value, and `|hi| ≥ |lo|`.
///
/// Exact for every finite `a` except those whose high half would be ±2^1024, which no double holds
/// (some `a` within 2^997 of ±Ω, Ω itself among them: its split is 2^1024 − 2^971). For those the
/// result is `(±∞, ∓∞)`, an infinity with the sign of `a` and the opposite one, and so it is for an
/// infinite `a`; a NaN `a` gives `(NaN, NaN)`. `split(0)` is `(0, 0)`.
///
/// Bit budget (Shewchuk 1997, Theorem 17, with p = 53 and splitting point s = 27, multiplier
/// [`SPLITTER`] = 2^27 + 1): `hi` has p − s = 26 significant bits and `lo` has s − 1 = 26; the 53rd
/// bit of `a` is carried by the sign of `lo`. `hi` has the exponent of `a` or one more (then it is a
/// single bit), so `|hi| ≤ 2^(e_a + 1)`. Both halves are whole multiples of u_a = 2^(e_a − 52): `hi`
/// is the difference of two doubles at least as large as `a`, and `lo = a − hi`.
///
/// Below 2^996 Veltkamp's four operations run unchanged. Overflow: `|SPLITTER · a| < 2^996 ·
/// (2^27 + 1) = 2^1023 + 2^996 < Ω`; `c − a` is smaller than `c` in magnitude (same sign); the last two
/// operations are exact (Sterbenz's lemma). So nothing overflows. Underflow: every operand is a whole
/// multiple of 2^−1074 and so is every exact result (`SPLITTER` is an integer), so binary64 rounds
/// each operation exactly as Shewchuk's unbounded arithmetic does (module documentation). That holds
/// for a subnormal `a` too, which is a double with fewer significant bits, as the theorem allows.
///
/// From 2^996 up, `a · 2^−64` is exact (a normal number scaled down, staying normal), it is split as
/// above, and both halves are scaled back by 2^64. In unbounded arithmetic Veltkamp's method commutes
/// with scaling by a power of two, so these are the halves of `a`. Scaling up is exact unless it
/// overflows; `|lo| ≤ |hi|`, so only `hi` can, and then no double holds the high half and the
/// overflow is reported as above.
#[must_use]
pub const fn split(a: f64) -> (f64, f64) {
    if abs(a) < TWO_POW_996 {
        return veltkamp(a);
    }
    let (hi, lo) = veltkamp(a * TWO_POW_NEG_64);
    let hi = hi * TWO_POW_64;
    if hi.is_finite() {
        return (hi, lo * TWO_POW_64);
    }
    let h = if a > 0.0 {
        f64::INFINITY
    } else if a < 0.0 {
        f64::NEG_INFINITY
    } else {
        a
    };
    (h, -h)
}

/// The exact error `a · b − p` of `p = a * b` by Dekker's method, for `a`, `b` and `p` inside
/// [`two_prod`]'s direct range (|a|, |b| < 2^996 and 2^−968 ≤ |p| < 2^1022). [`two_prod`] carries
/// the proof.
const fn dekker_error(a: f64, b: f64, p: f64) -> f64 {
    let (a_hi, a_lo) = veltkamp(a);
    let (b_hi, b_lo) = veltkamp(b);
    let err1 = p - a_hi * b_hi;
    let err2 = err1 - a_lo * b_hi;
    let err3 = err2 - a_hi * b_lo;
    a_lo * b_lo - err3
}

/// The exact error `a · b − p` of a finite `p = a * b` from finite factors outside Dekker's direct
/// range, or an infinity with the sign of that error when no double holds it. [`two_prod`] carries
/// the proof.
const fn scaled_error(a: f64, b: f64, p: f64) -> f64 {
    if a == 0.0 || b == 0.0 {
        return 0.0;
    }
    let (ea, eb) = (exponent(a), exponent(b));
    let (ma, mb) = (scale(a, -ea), scale(b, -eb));
    let pm = ma * mb;
    let em = dekker_error(ma, mb, pm);
    let k = -(ea + eb);
    let ps = scale(p, k);
    if ps == pm {
        let e = scale(em, -k);
        if scale(e, k) == em {
            return e;
        }
    }
    let direction = if ps == pm { em } else { pm - ps };
    if direction > 0.0 {
        f64::INFINITY
    } else {
        f64::NEG_INFINITY
    }
}

/// The product `a · b` rounded, and its exact error, by Dekker's algorithm without fused
/// multiply-add.
///
/// Returns `(p, e)` with `p = a * b` rounded to nearest (the IEEE product) and `p + e = a · b`
/// exactly, `|e|` at most half a unit in the last place of `p`, whenever an exact pair exists, that is
/// whenever `p` is finite and the exact error `a · b − p` is a double. For a finite `p` that fails only
/// when `a · b` has a bit below 2^−1074, which needs e_a + e_b ≤ −971 and so |a · b| < 2^−969; no
/// two-product can be exact then, and `e` is an infinity with the sign of the exact error, so even a
/// product that underflows to zero keeps its sign: `two_prod(2^−600, 2^−600)` is `(0, +∞)`. If `p` is
/// not finite (the product overflows, or an input is not finite) the result is `(p, −p)`: the
/// opposite infinity, or NaN when `p` is NaN (0 · ∞, or a NaN input). So `e` is finite exactly when
/// the pair is exact, and finite inputs never give NaN. A zero factor gives `(p, 0)`.
///
/// Every pair with |a|, |b| < 2^996 and 2^−968 ≤ |p| < 2^1022, products of grid coordinates (below
/// 2^41 each) among them, takes the direct path: Dekker's six operations, no scaling.
///
/// Bit budget and proof of the direct path (Shewchuk 1997, Theorem 18, Veltkamp's method in Dekker's
/// paper):
/// - Range: |p| < 2^1022 gives |a · b| < 2^1022 (rounding is monotone and 2^1022 is a double), and
///   |a · b| ≥ 2^(e_a + e_b), so e_a + e_b ≤ 1021. |p| ≥ 2^−968 gives |a · b| > 2^−969, and
///   |a · b| < 2^(e_a + e_b + 2), so e_a + e_b ≥ −970. These are leading-bit exponents, so a subnormal
///   factor is covered.
/// - [`split`] gives `a = a_hi + a_lo` and `b = b_hi + b_lo`, each half of 26 bits, so each of the
///   four partial products `a_hi·b_hi`, `a_lo·b_hi`, `a_hi·b_lo`, `a_lo·b_lo` has at most
///   26 + 26 = 52 ≤ 53 significant bits.
/// - Underflow: every half of `a` is a whole multiple of u_a = 2^(e_a − 52) (for a subnormal `a`,
///   2^(e_a − 52) is below 2^−1074, and every double is a multiple of it) and every half of `b` of
///   u_b. So every partial product, the exact product, `p` (|a · b| > 2^−969 is in the normal range,
///   where rounding to 53 bits keeps the multiple), the exact error and every intermediate is a whole
///   multiple of u_a · u_b = 2^(e_a + e_b − 104) ≥ 2^(−970 − 104) = 2^−1074, the smallest subnormal.
///   A value on that grid with at most 53 significant bits is a double, so every step Shewchuk proves
///   exact in unbounded arithmetic is exact here too.
/// - Overflow: `|a_hi| ≤ 2^(e_a + 1)` and `|a_lo| ≤ |a_hi|` (likewise for `b`), so every partial
///   product is at most 2^(e_a + e_b + 2) ≤ 2^1023 in magnitude, and so is `p`. The remaining
///   operations are exact, so each result is a double and none overflows.
///
/// Every other finite pair with a finite `p` takes the scaled path:
/// - A zero factor makes the exact product zero, so `p` is zero and the error is 0.
/// - Otherwise each factor is scaled by an exact power of two to `m_a = a · 2^−e_a` and
///   `m_b = b · 2^−e_b`, of magnitude in [1, 2) (exact by [`scale`]: the results are doubles). Their
///   product X = m_a · m_b = a · b · 2^k, with k = −(e_a + e_b), is in [1, 4) in magnitude, inside the
///   direct range, so the direct path gives `p_m` (X rounded) and `e_m` with `p_m + e_m = X` exactly.
/// - `p_s = p · 2^k` is exact: when |a · b| ≥ 2^−1022, `p` is `a · b` rounded to 53 bits, rounding
///   commutes with scaling, and `p_s = p_m`, a double; when |a · b| < 2^−1022, k > 0, and a double
///   scaled up to at most 4 in magnitude is a double.
/// - If `p_s = p_m`, then `e_m = X − p_s = (a · b − p) · 2^k` exactly, so the exact error is
///   `e_m · 2^−k`. When that is a double, [`scale`] computes it exactly and scaling back by 2^k gives
///   `e_m`. When it is not, k > 0 (for k ≤ 0 it is the double `e_m` scaled up to at most half a unit
///   of `p`, 2^970, so a double), [`scale`] returns some other double, and scaling that back up by
///   2^k is exact and so cannot give `e_m`. So the check accepts exactly the case where the exact
///   error is a double.
/// - If `p_s ≠ p_m`, no exact pair exists. An exact pair needs `a · b` to be a whole multiple of
///   2^−1074 (`p` and `e` both are); then either |a · b| ≥ 2^−1022, so `p_s = p_m` as above, or
///   `a · b` is itself a double, `p = a · b`, and `p_s = X = p_m`. So here |a · b| < 2^−1022: `p_s` is
///   X rounded to the grid G = 2^(k − 1074) and `p_m` is X rounded to 53 bits, on a grid g with
///   G ≥ 2g. Both are whole multiples of g, so `|p_m − p_s| ≥ g > g/2 ≥ |e_m|`, and the exact error
///   `((p_m − p_s) + e_m) · 2^−k` has the sign of `p_m − p_s`, which the rounded subtraction of two
///   distinct doubles keeps.
/// - The scaled path only multiplies finite values by powers of two and compares them, so it never
///   makes NaN.
#[must_use]
pub const fn two_prod(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    if !p.is_finite() {
        return (p, -p);
    }
    let size = abs(p);
    let direct = size >= TWO_POW_NEG_968 && size < TWO_POW_1022;
    if direct && abs(a) < TWO_POW_996 && abs(b) < TWO_POW_996 {
        return (p, dekker_error(a, b, p));
    }
    (p, scaled_error(a, b, p))
}
