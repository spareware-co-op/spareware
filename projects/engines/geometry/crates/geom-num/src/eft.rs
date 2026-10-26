//! Error-free transformations: a rounded sum or product of two doubles and its exact error.
//!
//! `two_sum(a, b)` returns `(s, e)` where `s` is `a + b` rounded and `s + e = a + b` exactly, in
//! real arithmetic. `two_prod(a, b)` returns `(p, e)` where `p` is `a · b` rounded and
//! `p + e = a · b` exactly. They are the building blocks of double-double arithmetic (the one-time
//! conversion of survey and map data into a frame, chapter 6.1) and of floating-point expansions.
//!
//! What every function here guarantees, for every input (there are no preconditions):
//!
//! - **Exact or reported, never silently wrong.** For finite inputs the pair is exact whenever an
//!   exact pair exists, and then both parts are finite. When none exists, the second part is an
//!   infinity with the sign of the exact error, so one check, `e.is_finite()`, tells a caller
//!   whether the pair is exact. No exact pair exists only when the rounded sum or product
//!   overflows, when the high half of a split would be ±2^1024, or when the exact error of a
//!   product is finer than the smallest subnormal 2^−1074 (possible only for |a · b| < 2^−969). The
//!   tests check every case against exact rationals (`num-rational`) on random and adversarial
//!   inputs.
//! - **Never NaN from finite inputs.** The second part is NaN only when the first part is NaN: a
//!   NaN input, or an IEEE operation on infinities that is itself NaN (∞ − ∞, 0 · ∞). Any other
//!   non-finite input gives an infinite second part. So the crate's "never NaN" invariant
//!   (`lib.rs`) holds here for every finite input; for infinite inputs it holds except where IEEE
//!   754 itself makes the rounded sum or product NaN.
//! - **The rounded part is the IEEE 754 result.** `s` is `a + b` and `p` is `a * b` as Rust
//!   computes them (round to nearest, ties to even). An exact error is at most half a unit in the
//!   last place of that result, so the pair does not overlap: `s + e` rounds back to `s`.
//! - **One zero.** A second part that is zero is +0, never −0: the exact error of a sum or a
//!   product, and the low half of a split, whatever the signs of the inputs and however Veltkamp's
//!   split falls. So the second part depends on the exact pair alone, as an exact zero does in IEEE
//!   arithmetic (a zero sum of operands of opposite signs is +0 under round to nearest, IEEE
//!   754-2019, section 6.3) and as it does for a two-product by fused multiply-add. Hence
//!   [`fast_two_sum`] and [`two_sum`] agree bit for bit (a NaN aside, whose bits Rust does not
//!   fix), and `two_prod(−a, b)` is `(−p, −e)` for a nonzero `e` and `(−p, +0)` for a zero one.
//!   Where the last step of an algorithm can give −0, the function adds +0, which turns −0 into +0
//!   and leaves every other value as it is (the proof is on the crate's one helper for it,
//!   `zero_to_plus` in `real.rs`, which [`crate::interval`] uses too); each function's
//!   documentation shows where that is needed and why it is needed nowhere else.
//! - **Deterministic.** Only `+`, `−`, `×` and `÷` on `f64` (`÷` only by a power of two),
//!   comparisons, and reads and writes of a double's bits. Rust computes these to IEEE 754
//!   binary64, each rounded once, on every target the kernel builds for (RFC 3514). The RFC names
//!   the exception itself: on 32-bit x86 without SSE2 (the i586 targets) the x87 unit computes with
//!   a different precision and so can round twice, which would break these algorithms; the kernel
//!   does not support those targets. The RFC also rules out fusing `a * b + c` into one fused
//!   multiply-add, which would break them too, and none is used on purpose either: `two_prod`
//!   follows Dekker's splitting instead of `mul_add`, which `core` lacks and which is a slow
//!   software call on wasm32 (evidence note 05, section 1).
//! - **No integer overflow.** The only integer steps that run are on the 11-bit exponent fields:
//!   bit masks, shifts by 52 places that keep the field inside 64 bits, `abs_diff`, which is exact
//!   for every input, and `checked_sub`, whose `None` ends a loop. There is no other integer
//!   arithmetic at run time (the compiler alone evaluates the checks on the constants, where an
//!   overflow would stop the build) and no cast, so nothing here depends on overflow checks being
//!   on.
//! - **Bounded proofs, and what is only tested.** Kani proves (piece G41, chapter 28.7) that
//!   `split` adds back exactly or reports, for every finite double; that `two_sum` and
//!   `fast_two_sum` agree and give the rounded sum and its error for every finite operand against
//!   five others, in either order (against the largest double, for operands below 2^969, where no
//!   step overflows; the overflow fallback is only tested, because its NaN intermediate trips
//!   Kani's NaN check); and that `two_prod` gives the rounded product and its error
//!   for every finite first factor whose significand has at most 14 bits (every sign and
//!   exponent: a symbolic 53-bit significand ran past CI's hour) over six second factors, one for each of its paths (the
//!   proofs module has one harness per value, with kissat as the SAT solver). Over every pair at
//!   once, or one law's values in one harness, the solver ran past CI's hour. Every
//!   other pair, and everything through the fused multiply-add, is tested against exact
//!   rationals.
//!
//! Method: Dekker 1971 (`fast_two_sum`, Veltkamp's `split`, `two_prod`) and Knuth (`two_sum`), in
//! the form and with the proofs of Shewchuk 1997: Theorem 6 (fast two-sum), Theorem 7 (two-sum),
//! Theorem 17 (split) and Theorem 18 (two-product). Shewchuk's proofs assume an unbounded exponent
//! (his section 2.1 leaves overflow and underflow aside); each function below derives the
//! conditions under which they hold in binary64 and says what it does outside them. `fast_two_sum`
//! puts its operands in the order Dekker's method needs. `two_sum` runs Knuth's six operations and,
//! in the one case where an intermediate overflows, returns `fast_two_sum`'s pair instead. `split`
//! and `two_prod` run Veltkamp's and Dekker's operations unchanged where the conditions hold, and
//! otherwise first move their inputs into that range by exact powers of two, then check that the
//! way back is exact.
//!
//! - T. J. Dekker, "A floating-point technique for extending the available precision", *Numerische
//!   Mathematik* 18 (1971), 224–242, <https://doi.org/10.1007/BF01397083>.
//! - J. R. Shewchuk, "Adaptive precision floating-point arithmetic and fast robust geometric
//!   predicates", *Discrete & Computational Geometry* 18 (1997), 305–363,
//!   <https://doi.org/10.1007/PL00009321>.
//! - D. E. Knuth, *The Art of Computer Programming*, volume 2, section 4.2.2.
//! - S. Boldo, "Pitfalls of a full floating-point proof: example on the formal proof of the
//!   Veltkamp/Dekker algorithms", *Automated Reasoning* (IJCAR 2006), Lecture Notes in Computer
//!   Science 4130: Veltkamp's high half is the input rounded to nearest.
//! - IEEE 754-2019, *IEEE Standard for Floating-Point Arithmetic*, section 6.3 (the sign of an
//!   exact zero), <https://doi.org/10.1109/IEEESTD.2019.8766229>.
//! - RFC 3514, "Float semantics": <https://rust-lang.github.io/rfcs/3514-float-semantics.html>.
//!
//! Cost: constant time and no allocation. Each function is a fixed sequence of floating-point
//! operations, comparisons and bit reads; outside Dekker's direct range `split` also scales by
//! 2^64 and back, and `two_prod` scales by powers of two (`scale`), each scaling at most two steps.
//!
//! Notation used in the proofs: Ω = (2 − 2^−52) · 2^1023 is the largest finite double. For a
//! nonzero finite double x, `e_x` is the exponent of its leading bit, so
//! `2^e_x ≤ |x| < 2^(e_x + 1)`; `e_x` runs from −1074 (the smallest subnormal) to 1023, and for a
//! normal x it is the binary exponent. The quantum of x is the value of the last bit of its format:
//! `u_x = 2^(e_x − 52)` for a normal x and 2^−1074 for a subnormal one. Separately,
//! `t_x = 2^(e_x − 52)` is the value of the bit 52 places below the leading bit of x, whatever its
//! format: it equals `u_x` for a normal x and is finer than `u_x` for a subnormal one. Every double
//! x is a whole multiple of `u_x`, of `t_x` and of 2^−1074.
//!
//! Two facts about gradual underflow carry Shewchuk's proofs over to binary64:
//!
//! - A sum or difference of two doubles that lands below 2^−1022 is exact: its exact value is a
//!   whole multiple of 2^−1074 below 2^−1022, which is a double. So gradual underflow never spoils
//!   an addition, and Sterbenz's lemma (y/2 ≤ x ≤ 2y makes x − y exact) holds in the subnormal
//!   range.
//! - More generally, if the exact result of an operation is a whole multiple of 2^−1074, binary64
//!   and Shewchuk's unbounded 53-bit arithmetic give the same rounded value: below 2^−1022 such a
//!   result has at most 52 significant bits, so neither rounds it, and from 2^−1022 up both round
//!   to 53 bits.

use crate::real::zero_to_plus;

/// Veltkamp's splitting constant for binary64, 2^27 + 1: splits a 53-bit significand into two
/// halves of 26 bits each (Shewchuk 1997, Theorem 17, with splitting point s = 27).
pub const SPLITTER: f64 = 134_217_729.0;

/// The exponent field of a double's bits, as a mask: the bits of +∞, which are the exponent field
/// all ones and nothing else.
const EXPONENT_BITS: u64 = f64::INFINITY.to_bits();

/// The bits of 1.0: the exponent field 1023 and nothing else.
const ONE_BITS: u64 = 1.0f64.to_bits();

/// 2^996: below it, [`split`] runs Veltkamp's method directly and cannot overflow. Built from its
/// bits: exponent field 1023 + 996 = 2019 = 0x7e3.
const TWO_POW_996: f64 = f64::from_bits(0x7e30_0000_0000_0000);

/// 2^1022: a product below it has `e_a + e_b ≤ 1021`, the upper limit of Dekker's direct product.
/// Exponent field 1023 + 1022 = 2045 = 0x7fd.
const TWO_POW_1022: f64 = f64::from_bits(0x7fd0_0000_0000_0000);

/// 2^−968: a product at or above it has `e_a + e_b ≥ −970`, the lower limit of Dekker's direct
/// product. Exponent field 1023 − 968 = 55 = 0x037.
const TWO_POW_NEG_968: f64 = f64::from_bits(0x0370_0000_0000_0000);

/// 2^64: the exact scaling [`split`] undoes above 2^996, and the one that makes a subnormal factor
/// of [`two_prod`] normal. Exponent field 1023 + 64 = 1087 = 0x43f.
const TWO_POW_64: f64 = f64::from_bits(0x43f0_0000_0000_0000);

/// 2^−64: the exact scaling [`split`] applies above 2^996. Exponent field 1023 − 64 = 959 = 0x3bf.
const TWO_POW_NEG_64: f64 = f64::from_bits(0x3bf0_0000_0000_0000);

/// 2^1000: the largest step up [`scale`] takes. Exponent field 1023 + 1000 = 2023 = 0x7e7.
const TWO_POW_1000: f64 = f64::from_bits(0x7e70_0000_0000_0000);

/// 2^−1000: the largest step down [`scale`] takes. Exponent field 1023 − 1000 = 23 = 0x017.
const TWO_POW_NEG_1000: f64 = f64::from_bits(0x0170_0000_0000_0000);

// Each power of two above is what its name says: its exponent field is 1023 plus its exponent and
// its other bits are zero. Checked when the crate is compiled, so a mistyped constant cannot build.
const _: () = {
    assert!(TWO_POW_996.to_bits() == (1023 + 996) << 52);
    assert!(TWO_POW_1022.to_bits() == (1023 + 1022) << 52);
    assert!(TWO_POW_NEG_968.to_bits() == (1023 - 968) << 52);
    assert!(TWO_POW_64.to_bits() == (1023 + 64) << 52);
    assert!(TWO_POW_NEG_64.to_bits() == (1023 - 64) << 52);
    assert!(TWO_POW_1000.to_bits() == (1023 + 1000) << 52);
    assert!(TWO_POW_NEG_1000.to_bits() == (1023 - 1000) << 52);
};

/// The magnitude of `x` by comparison: −x for x < 0, x otherwise. −0 stays −0, which compares equal
/// to +0, and NaN stays NaN, which compares false, so every comparison of the result is that of
/// |x|.
const fn abs(x: f64) -> f64 {
    if x < 0.0 { -x } else { x }
}

/// The exponent field of `x`: 0 for zero and the subnormals, 1 to 2046 for the normal numbers
/// (`2^(field − 1023) ≤ |x| < 2^(field − 1022)`), 2047 for the infinities and NaN. A bit read.
const fn exponent_field(x: f64) -> u64 {
    (x.to_bits() & EXPONENT_BITS) >> 52
}

/// `x` with its exponent field replaced by that of 1: the double with the sign and fraction bits of
/// `x` and magnitude in [1, 2). For a normal `x` that is `x · 2^−e_x` exactly. Bit operations only.
const fn unit_binade(x: f64) -> f64 {
    f64::from_bits((x.to_bits() & !EXPONENT_BITS) | ONE_BITS)
}

/// `x · 2^n` when `up`, otherwise `x · 2^−n`, for any `n`, by products with powers of two that all
/// go the same way: steps of 2^±1000 while 1000 or more remain, then one step with the rest. The
/// callers pass `n` of at most 1074, so there are at most two steps. Scaling up by the rest divides
/// by 2^−rest, which IEEE 754 rounds exactly as it would the product with 2^rest (the exact
/// quotient is that product), so no power of two above 2^1000 is needed.
///
/// Exact whenever the exact value `x · 2^±n` is a double. Every intermediate's exact value lies
/// between `|x|` and the result in magnitude (so it is at most Ω), has the significand of `x`, and
/// is a whole multiple of 2^−1074: scaling up, it is `x` times a power of two of at least 1;
/// scaling down, it is (by induction over the steps, each exact so far) the result times a power of
/// two of at least 1. So it is a double and no step rounds. When the exact value is not a double
/// the result is some other double, after a rounding at one step or more; the callers detect that
/// by scaling back.
///
/// Integer steps: `checked_sub` on the amount left, whose `None` (less than 1000 left) ends the
/// loop, and `1023.abs_diff(rest)`, exact for every input. With `rest ≤ 999` it is 1023 − rest,
/// from 24 to 1023: the exponent field of the normal power of two 2^−rest, below 2^10, so shifting
/// it left by 52 places stays inside 64 bits.
const fn scale(x: f64, n: u64, up: bool) -> f64 {
    let step = if up { TWO_POW_1000 } else { TWO_POW_NEG_1000 };
    let mut x = x;
    let mut rest = n;
    while let Some(less) = rest.checked_sub(1000) {
        x *= step;
        rest = less;
    }
    let down = f64::from_bits(1023u64.abs_diff(rest) << 52);
    if up { x / down } else { x * down }
}

/// The sum `a + b` rounded, and its exact error, by Dekker's fast two-sum with the operands put in
/// order of magnitude first.
///
/// Returns `(s, e)` with `s = a + b` rounded to nearest (the IEEE sum) and `s + e = a + b` exactly,
/// `|e|` at most half a unit in the last place of `s`, for every finite `a` and `b` whose rounded
/// sum is finite; a zero `e` is +0. Dekker's method needs the operand of larger magnitude first;
/// this function orders the operands itself (one comparison), so it asks nothing of the caller, and
/// its result is [`two_sum`]'s bit for bit (when the sum is NaN both are NaN, and Rust does not fix
/// a NaN's bits, RFC 3514).
///
/// If `s` is not finite (the sum overflows, or an input is not finite) the result is `(s, −s)`: the
/// opposite infinity, or NaN when `s` is NaN (∞ − ∞, or a NaN input).
///
/// Bit budget and proof (Dekker 1971; Shewchuk 1997, Theorem 6), with `x` the operand of larger
/// magnitude (`larger` in the code), `y` the other (`smaller`), so `|x| ≥ |y|`, and `s` finite:
/// - `z = s − x` (`smaller_virtual`) is exact by Sterbenz's lemma. If `x` and `y` have the same
///   sign, `|x| ≤ |s| ≤ 2|x|` with the sign of `x` (rounding is monotone and `x`, `2x` are
///   doubles). If their signs differ and `|y| ≥ |x|/2`, then `x + y` itself is exact (Sterbenz), so
///   `s = x + y` and `z = y`. Otherwise `|x|/2 ≤ |s| ≤ |x|`, again with the sign of `x`.
/// - `e = y − z` is exact: its exact value is the rounding error `x + y − s`. `x`, `y` and `s` are
///   all whole multiples of `u_y`, the quantum of `y`: `|x| ≥ |y|` makes `u_x ≥ u_y`, and `s` is
///   `x + y` rounded to a grid no finer than `u_y`, or `x + y` itself. So the error is a whole
///   multiple of `u_y`, and it is at most `|y|` in magnitude (`x` is a double, so the nearest
///   double `s` is no farther from `x + y` than `x` is). A whole multiple of `u_y` below
///   `2^53 · u_y` in magnitude has at most 53 significant bits, so it is a double.
/// - Both steps are exact, so neither overflows, and gradual underflow changes nothing (module
///   documentation). An overflowed `s` is caught before them.
/// - The zero: `y − z` is −0 exactly when `y` is −0 and `z` is +0 (a difference of equal nonzero
///   values is +0), as in `fast_two_sum(1, −0)` and `fast_two_sum(0, −0)`. So the function returns
///   `y − z` plus +0, which is +0 then and `y − z` otherwise (module documentation, "One zero").
///   [`two_sum`]'s error is never −0, so the two agree bit for bit.
#[must_use]
pub const fn fast_two_sum(a: f64, b: f64) -> (f64, f64) {
    let (larger, smaller) = if abs(b) > abs(a) { (b, a) } else { (a, b) };
    let s = larger + smaller;
    if !s.is_finite() {
        return (s, -s);
    }
    let smaller_virtual = s - larger;
    (s, zero_to_plus(smaller - smaller_virtual))
}

/// The sum `a + b` rounded, and its exact error, for any finite `a` and `b` (Knuth's two-sum).
///
/// Returns `(s, e)` with `s = a + b` rounded to nearest (the IEEE sum) and `s + e = a + b` exactly,
/// `|e|` at most half a unit in the last place of `s`, whenever `s` is finite; a zero `e` is +0. No
/// ordering of `a` and `b` is needed. If `s` is not finite the result is `(s, −s)`, as for
/// [`fast_two_sum`].
///
/// Bit budget: the error-free pair is unique (`s` is the IEEE sum and `e` the exact remainder), so
/// it is the pair of [`fast_two_sum`] and has its budget: `e` is a whole multiple of the quantum of
/// the operand of smaller magnitude and at most that operand in magnitude, so it has at most 53
/// significant bits. Knuth's six operations reach it without a branch; each is exact by Shewchuk
/// 1997, Theorem 7 (a case analysis with Sterbenz's lemma, like the fast two-sum's), with or
/// without gradual underflow, provided no intermediate overflows.
///
/// Overflow: with a bounded exponent one intermediate can overflow although `s` does not: for
/// `a = −3 · 2^970` and `b = Ω`, `s = Ω − 2^971` (a tie, rounded to even), and `s − a` is exactly
/// Ω + 2^970, the midpoint between Ω and 2^1024, which rounds to infinity. That can only happen
/// when `|b| > |a|`: when `|a| ≥ |b|`, `s − a` and `b − (s − a)` are exact (Shewchuk 1997,
/// Theorem 6), the next value is exactly `a`, and every step is exact. An infinity, once made, is
/// never absorbed by a later addition (each gives an infinity or NaN), and every intermediate
/// feeds `e`, so any intermediate overflow shows as a non-finite `e`. In that case the function
/// returns [`fast_two_sum`]`(b, a)`, which is exact and has the same `s` because IEEE addition is
/// commutative. So the result is exact for every finite input whose rounded sum is finite. If `s`
/// overflows, or an input is not finite, `e` is not finite either and the same fallback returns
/// `(s, −s)`.
///
/// The zero: `e` is never −0, so it needs no fix. Under round to nearest a sum is −0 only when both
/// terms are −0, and a difference `u − v` only when `u` is −0 and `v` is +0. So `e` = −0 would need
/// `a` = `b` = −0 and `a_virtual` = +0; but then `s` = −0, `b_virtual = s − a` = +0 and
/// `a_virtual = s − b_virtual` = −0. The fallback's zero error is +0 too ([`fast_two_sum`]).
#[must_use]
pub const fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let b_virtual = s - a;
    let a_virtual = s - b_virtual;
    let e = (a - a_virtual) + (b - b_virtual);
    if e.is_finite() {
        (s, e)
    } else {
        fast_two_sum(b, a)
    }
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

/// Veltkamp's split of `a` into a high and a low half: `a = hi + lo` exactly, each half a 26-bit
/// value, and `|hi| ≥ |lo|`.
///
/// Exact for every finite `a` except those whose high half would be ±2^1024, which no double holds:
/// exactly the `a` with `|a| ≥ 2^1024 − 2^997`, Ω among them (its halves would be 2^1024 and
/// −2^971). For those the result is `(±∞, ∓∞)`, an infinity with the sign of `a` and the opposite
/// one, and so it is for an infinite `a`; a NaN `a` gives `(NaN, NaN)`. A zero low half is +0, so
/// `split(±0)` is `(±0, +0)`.
///
/// Bit budget (Shewchuk 1997, Theorem 17, with p = 53 and splitting point s = 27, multiplier
/// [`SPLITTER`] = 2^27 + 1): `hi` has p − s = 26 significant bits and `lo` has s − 1 = 26; the 53rd
/// bit of `a` is carried by the sign of `lo`. `hi` is `a` rounded to nearest on 26 bits, a tie
/// either way (Boldo 2006), so it has the exponent of `a` or one more (then it is a single bit),
/// and `|hi| ≤ 2^(e_a + 1)`. Both halves are whole multiples of `t_a = 2^(e_a − 52)`: `hi` is the
/// difference of two doubles at least as large as `a`, and `lo = a − hi`.
///
/// Below 2^996 Veltkamp's four operations run unchanged. Overflow:
/// `|SPLITTER · a| < 2^996 · (2^27 + 1) = 2^1023 + 2^996 < Ω`; `c − a` is smaller than `c` in
/// magnitude (same sign); the last two operations are exact (Sterbenz's lemma). So nothing
/// overflows. Underflow: every operand is a whole multiple of 2^−1074 and so is every exact result
/// (`SPLITTER` is an integer), so binary64 rounds each operation exactly as Shewchuk's unbounded
/// arithmetic does (module documentation). That holds for a subnormal `a` too, which is a double
/// with fewer significant bits, as the theorem allows.
///
/// From 2^996 up, `a · 2^−64` is exact (a normal number scaled down, staying normal), it is split
/// as above, and both halves are scaled back by 2^64. In unbounded arithmetic Veltkamp's method
/// commutes with scaling by a power of two, so these are the halves of `a`. Scaling up is exact
/// unless it overflows; `|lo| ≤ |hi|`, so only `hi` can, and then no double holds the high half and
/// the overflow is reported as above.
///
/// Which `a` are reported: below 2^1023 in magnitude the 26-bit rounding is at most 2^1023. From
/// 2^1023 to Ω the 26-bit values either side of `|a|` are 2^1024 − 2^998 and 2^1024, with midpoint
/// 2^1024 − 2^997: above it `|hi|` is 2^1024, below it at most 2^1024 − 2^998. At the midpoint
/// itself Veltkamp's operations round up: on the scaled value 2^960 − 2^933, `SPLITTER` times it is
/// exactly 2^987 − 2^933, a tie that rounds to the even 2^987; `c − a` is exactly
/// 2^987 − 2^960 + 2^933, a tie that rounds to the even 2^987 − 2^960; and `hi` is 2^960.
///
/// The zero: `lo = a − hi` is zero only when `a` and `hi` are equal, and a difference of equal
/// values under round to nearest is +0, even of two zeros of the same sign; scaling +0 keeps it +0.
/// `hi` has the sign of `a`, so −0 splits as (−0, +0).
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

/// The exact error `a · b − p` of `p = a * b` by Dekker's method, a zero error as +0, for `a`, `b`
/// and `p` inside [`two_prod`]'s direct range (`|a|, |b| < 2^996` and `2^−968 ≤ |p| < 2^1022`).
/// [`two_prod`] carries the proof.
const fn dekker_error(a: f64, b: f64, p: f64) -> f64 {
    let (a_hi, a_lo) = veltkamp(a);
    let (b_hi, b_lo) = veltkamp(b);
    let err1 = p - a_hi * b_hi;
    let err2 = err1 - a_lo * b_hi;
    let err3 = err2 - a_hi * b_lo;
    zero_to_plus(a_lo * b_lo - err3)
}

/// A nonzero finite double `x` taken apart exactly for [`two_prod`]'s scaled path: `x = m · 2^e_x`,
/// with the exponent `e_x` given by its magnitude and its sign.
struct Binade {
    /// `m = x · 2^−e_x`: the sign and fraction bits of `x`, with magnitude in [1, 2).
    m: f64,
    /// `|e_x|`, from 0 to 1074.
    magnitude: u64,
    /// Whether `e_x < 0`, that is whether `|x| < 1`.
    below_one: bool,
}

impl Binade {
    /// `x` taken apart, for a nonzero finite `x`. A normal `x` has an exponent field f from 1 to
    /// 2046, `e_x = f − 1023`, and `m` is `x` with that field set to 1023. A subnormal `x` is
    /// `j · 2^−1074` with `1 ≤ j < 2^52`, so `y = x · 2^64 = j · 2^−1010` is exact and normal
    /// (`2^−1010 ≤ |y| < 2^−958`, exponent field f from 13 to 64) with the same sign and
    /// significant bits; then `e_x = e_y − 64 = f − 1087`, from −1074 to −1023, and `m` is `y` with
    /// its field set to 1023. Either way `abs_diff` gives `|e_x|` exactly, with no overflow.
    const fn of(x: f64) -> Self {
        let field = exponent_field(x);
        if field == 0 {
            let y = x * TWO_POW_64;
            Self {
                m: unit_binade(y),
                magnitude: exponent_field(y).abs_diff(1087),
                below_one: true,
            }
        } else {
            Self {
                m: unit_binade(x),
                magnitude: field.abs_diff(1023),
                below_one: field < 1023,
            }
        }
    }
}

/// The exact error `a · b − p` of a finite `p = a * b` from finite factors outside Dekker's direct
/// range, or an infinity with the sign of that error when no double holds it. [`two_prod`] carries
/// the proof.
const fn scaled_error(a: f64, b: f64, p: f64) -> f64 {
    if a == 0.0 || b == 0.0 {
        return 0.0;
    }
    let (a_parts, b_parts) = (Binade::of(a), Binade::of(b));
    let p_m = a_parts.m * b_parts.m;
    let e_m = dekker_error(a_parts.m, b_parts.m, p_m);
    // 2^k = 2^−e_a · 2^−e_b as at most two scalings that go the same way, up when k ≥ 0: both
    // magnitudes when the exponents have the same sign, else the difference of the magnitudes.
    let (up, first, second) = if a_parts.below_one == b_parts.below_one {
        (a_parts.below_one, a_parts.magnitude, b_parts.magnitude)
    } else {
        let (negative, positive) = if a_parts.below_one {
            (a_parts.magnitude, b_parts.magnitude)
        } else {
            (b_parts.magnitude, a_parts.magnitude)
        };
        (negative >= positive, negative.abs_diff(positive), 0)
    };
    let p_s = scale(scale(p, first, up), second, up);
    if p_s == p_m {
        let e = scale(scale(e_m, first, !up), second, !up);
        if scale(scale(e, first, up), second, up) == e_m {
            return e;
        }
    }
    let direction = if p_s == p_m { e_m } else { p_m - p_s };
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
/// exactly, `|e|` at most half a unit in the last place of `p`, whenever an exact pair exists, that
/// is whenever `p` is finite and the exact error `a · b − p` is a double. For a finite `p` that
/// fails only when `a · b` has a bit below 2^−1074, which needs `e_a + e_b ≤ −971` and so
/// `|a · b| < 2^−969`; no two-product can be exact then, and `e` is an infinity with the sign of
/// the exact error, so even a product that underflows to zero keeps its sign:
/// `two_prod(2^−600, 2^−600)` is `(0, +∞)`. If `p` is not finite (the product overflows, or an
/// input is not finite) the result is `(p, −p)`: the opposite infinity, or NaN when `p` is NaN
/// (0 · ∞, or a NaN input). So `e` is finite exactly when the pair is exact, and finite inputs
/// never give NaN. A zero exact error is +0, whatever the signs of the factors: a zero factor gives
/// `(p, +0)`, and `two_prod(2 − 2^−52, 1)` gives `(2 − 2^−52, +0)`.
///
/// Every pair with `|a|, |b| < 2^996` and `2^−968 ≤ |p| < 2^1022` takes the direct path: Dekker's
/// six operations, no scaling, then +0 added to the error. Products of nonzero whole numbers below
/// 2^41 in magnitude (`1 ≤ |p| < 2^82`) are all among them: grid coordinates, at most 2^40 − 1, and
/// their differences. A zero factor takes the scaled path's zero case.
///
/// Bit budget and proof of the direct path (Shewchuk 1997, Theorem 18, Veltkamp's method in
/// Dekker's paper):
/// - Range: `|p| < 2^1022` gives `|a · b| < 2^1022` (rounding is monotone and 2^1022 is a double),
///   and `|a · b| ≥ 2^(e_a + e_b)`, so `e_a + e_b ≤ 1021`. `|p| ≥ 2^−968` gives `|a · b| > 2^−969`,
///   and `|a · b| < 2^(e_a + e_b + 2)`, so `e_a + e_b ≥ −970`. (In fact −969: the largest product
///   with `e_a + e_b = −970`, `(2 − 2^−52)^2 · 2^−970`, rounds below 2^−968. The proof needs only
///   −970.) These are leading-bit exponents, so a subnormal factor is covered.
/// - [`split`] gives `a = a_hi + a_lo` and `b = b_hi + b_lo`, each half of 26 bits, so each of the
///   four partial products `a_hi·b_hi`, `a_lo·b_hi`, `a_hi·b_lo`, `a_lo·b_lo` has at most
///   26 + 26 = 52 ≤ 53 significant bits.
/// - Underflow: every half of `a` is a whole multiple of `t_a = 2^(e_a − 52)` and every half of `b`
///   of `t_b` (module notation: for a subnormal factor this is finer than its quantum 2^−1074, and
///   the factor is a multiple of it too). So every partial product, the exact product, `p`
///   (`|a · b| > 2^−969` is in the normal range, where rounding to 53 bits keeps the multiple), the
///   exact error and every intermediate is a whole multiple of
///   `t_a · t_b = 2^(e_a + e_b − 104) ≥ 2^(−970 − 104) = 2^−1074`, the smallest subnormal. A value
///   on that grid with at most 53 significant bits is a double, so every step Shewchuk proves exact
///   in unbounded arithmetic is exact here too.
/// - Overflow: `|a_hi| ≤ 2^(e_a + 1)` and `|a_lo| ≤ |a_hi|` (likewise for `b`), so every partial
///   product is at most `2^(e_a + e_b + 2) ≤ 2^1023` in magnitude, and so is `p`. The remaining
///   operations are exact, so each result is a double and none overflows.
/// - The zero: a low half is never −0 ([`split`]), but a partial product with a +0 low half and a
///   negative factor is −0. `err1 = p − a_hi·b_hi` is not −0, since `p` is not zero, and so neither
///   is `err2` or `err3` (a difference `u − v` is −0 only when `u` is −0 and `v` is +0). So the
///   last step, `a_lo·b_lo − err3`, gives −0 exactly when `a_lo·b_lo` is −0 and `err3` is +0, as
///   for `2 − 2^−52` (low half −2^−52) times 1 (low half +0). Adding +0 turns that −0 into +0 and
///   leaves every other value as it is (module documentation, "One zero").
///
/// Every other finite pair with a finite `p` takes the scaled path:
/// - A zero factor makes the exact product zero, so `p` is zero and the error is +0.
/// - Otherwise each factor is taken apart exactly as `a = m_a · 2^e_a` and `b = m_b · 2^e_b`, with
///   `|m_a|` and `|m_b|` in [1, 2), by reading and writing exponent fields (a subnormal factor is
///   first multiplied by 2^64, which is exact and makes it normal). Their product
///   `X = m_a · m_b = a · b · 2^k`, with `k = −(e_a + e_b)`, is in [1, 4) in magnitude, inside the
///   direct range, so the direct path gives `p_m` (`X` rounded) and `e_m` with `p_m + e_m = X`
///   exactly and `|e_m| ≤ g/2`, where `g` is the unit in the last place of `X`'s binade, 2^−52 or
///   2^−51.
/// - Scaling by 2^k is done as at most two scalings that go the same way: by `|e_a|` and then by
///   `|e_b|` when `e_a` and `e_b` are both negative or both not, and otherwise by the difference of
///   the two magnitudes, which `abs_diff` gives exactly. So it is one run of products with powers
///   of two that all go one way, exact whenever its exact result is a double (`scale`). Scaling by
///   2^−k takes the same steps the other way.
/// - `p_s = p · 2^k` is at most 4 in magnitude. `|a · b| < 2^(e_a + e_b + 2)`, so by monotone
///   rounding `|p| ≤ 2^(e_a + e_b + 2)` when that power of two is a double; when it is above Ω,
///   `|p| ≤ Ω` is smaller; and when it is below 2^−1074, `|a · b| < 2^−1075` and `p` is zero.
/// - `p_s` is computed exactly. When `|a · b| ≥ 2^−1022`, `p` is `a · b` rounded to 53 bits,
///   rounding commutes with scaling, and `p_s = p_m`, a double. When `|a · b| < 2^−1022`, `k > 0`,
///   and `p` scaled up to at most 4 in magnitude keeps its significand and stays a whole multiple
///   of 2^−1074, so `p_s` is a double.
/// - If `p_s = p_m`, then `e_m = X − p_s = (a · b − p) · 2^k` exactly, so the exact error is
///   `e_m · 2^−k`. Also `p ≠ 0` (as `p_m ≠ 0`), so `|p| ≥ 2^−1074` and
///   `2^k = |p_s| / |p| ≤ 2^1076`. When the exact error is a double, scaling `e_m` by 2^−k computes
///   it exactly, and scaling that back by 2^k gives exactly `e_m`. When it is not, `k > 0` (for
///   `k ≤ 0` it is the double `e_m` scaled up to at most half a unit of `p`, 2^970, so a double),
///   and scaling by 2^−k returns some other double `e`, after at most four roundings. Each moves a
///   value by at most 2^−53 of itself or 2^−1075, so `|e| < 2 · |e_m| · 2^−k + 2^−1072` and
///   `|e · 2^k| < 2^−51 + 2^4`: scaling `e` back up only shifts a whole multiple of 2^−1074 that
///   stays far below Ω, so it is exact and cannot give `e_m`. So the check accepts exactly the case
///   where the exact error is a double, and otherwise the error is reported with the sign of `e_m`,
///   which is its sign.
/// - If `p_s ≠ p_m`, no exact pair exists. An exact pair needs `a · b` to be a whole multiple of
///   2^−1074 (`p` and `e` both are); then either `|a · b| ≥ 2^−1022`, so `p_s = p_m` as above, or
///   `a · b` is itself a double, `p = a · b`, and `p_s = X = p_m`. So here `|a · b| < 2^−1022`:
///   `p_s` is `X` rounded to the grid `G = 2^(k − 1074)` and `p_m` is `X` rounded to 53 bits, on
///   the grid `g`, with `G ≥ 2g` (`|a · b| ≥ 2^(e_X − k)` and `|a · b| < 2^−1022` give
///   `k ≥ 1023 + e_X`). Both are whole multiples of `g`, so `|p_m − p_s| ≥ g > g/2 ≥ |e_m|`, and
///   the exact error `((p_m − p_s) + e_m) · 2^−k` has the sign of `p_m − p_s`, which the rounded
///   subtraction of two distinct doubles keeps.
/// - The scaled path only multiplies and divides finite values by powers of two, subtracts two
///   doubles of at most 4 in magnitude and compares, so it never makes NaN.
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

/// Bounded proofs (chapter 28.7; piece G41), bit for bit: `split` for every finite double,
/// `two_sum` for every finite operand over [`proofs::ADDENDS`] and `two_prod` for every finite
/// first factor over [`proofs::FACTORS`]. The
/// transformations through the fused multiply-add (`two_prod_fma` and what calls it) are left out:
/// Kani cannot model the processor instruction `libm` uses for it, so those are only tested.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// A symbolic finite double.
    fn finite() -> f64 {
        let x = f64::from_bits(kani::any());
        kani::assume(x.is_finite());
        x
    }

    /// `two_sum` and `fast_two_sum` agree bit for bit, the sum is the rounded sum, and the error
    /// is below half a unit of it (adding it back rounds to the sum), for every finite first
    /// operand and the second operand `y`, in either order. It is proven for five second operands,
    /// one harness each (kissat as the SAT solver): 3 and −0.1 (inexact sums of either sign), 2^53
    /// (sums whose error is a whole number), the smallest subnormal (errors at the bottom of the
    /// range) and the largest double (overflow). Over every pair at once, or the five in one
    /// harness, the solver ran past CI's hour.
    fn two_sum_law(y: f64) {
        let x = finite();
        for (a, b) in [(x, y), (y, x)] {
            let (s, e) = two_sum(a, b);
            let (fs, fe) = fast_two_sum(a, b);
            assert!(s.to_bits() == fs.to_bits() && e.to_bits() == fe.to_bits());
            assert!(s.to_bits() == (a + b).to_bits());
            if s.is_finite() {
                assert!(e.is_finite() && s + e == s);
            }
        }
    }

    /// [`two_sum_law`] with 3: an inexact positive sum.
    #[kani::proof]
    #[kani::solver(kissat)]
    fn two_sum_with_three() {
        two_sum_law(3.0);
    }

    /// [`two_sum_law`] with −0.1: an inexact negative sum.
    #[kani::proof]
    #[kani::solver(kissat)]
    fn two_sum_with_minus_a_tenth() {
        two_sum_law(-0.1);
    }

    /// [`two_sum_law`] with 2^53: sums whose error is a whole number.
    #[kani::proof]
    #[kani::solver(kissat)]
    fn two_sum_with_two_pow_53() {
        two_sum_law(9_007_199_254_740_992.0);
    }

    /// [`two_sum_law`] with the smallest subnormal: errors at the bottom of the range.
    #[kani::proof]
    #[kani::solver(kissat)]
    fn two_sum_with_the_smallest_subnormal() {
        two_sum_law(f64::from_bits(1));
    }

    /// [`two_sum_law`] with the largest double, for every first operand below 2^969 in magnitude:
    /// sums up to the edge of overflow, where no step overflows (Ω + 2^969 is below the midpoint
    /// Ω + 2^970, so every sum and difference rounds to at most Ω). Beyond that an intermediate
    /// overflows by design and the fallback returns [`fast_two_sum`]'s pair; Kani's NaN check
    /// reports the NaN that intermediate makes (run 40323412527), so that path is tested against
    /// exact rationals (`tests/attack2_eft.rs`), not proven.
    #[kani::proof]
    #[kani::solver(kissat)]
    fn two_sum_with_the_largest_double() {
        let x = finite();
        kani::assume(abs(x) < f64::from_bits(0x7c80_0000_0000_0000));
        for (a, b) in [(x, f64::MAX), (f64::MAX, x)] {
            let (s, e) = two_sum(a, b);
            let (fs, fe) = fast_two_sum(a, b);
            assert!(s.to_bits() == fs.to_bits() && e.to_bits() == fe.to_bits());
            assert!(s.to_bits() == (a + b).to_bits());
            assert!(s.is_finite() && e.is_finite() && s + e == s);
        }
    }

    /// `split` cuts a double into two halves that add back to it exactly, for every finite double
    /// below 2^1024 − 2^997 in magnitude, and reports every larger one as (±∞, ∓∞), the infinity
    /// with the sign of `a` first (its documentation).
    #[kani::proof]
    fn split_adds_back_exactly() {
        let a = finite();
        let (hi, lo) = split(a);
        // 2^1024 − 2^997 = (2 − 2^−26) · 2^1023: exponent field 2046, significand 2^52 − 2^26.
        let reported = f64::from_bits(0x7fef_ffff_fc00_0000);
        if abs(a) < reported {
            assert!(hi + lo == a);
        } else {
            assert!(hi.is_infinite() && lo.is_infinite());
            assert!(hi.is_sign_negative() == a.is_sign_negative());
            assert!(lo.is_sign_negative() != a.is_sign_negative());
        }
    }

    /// `two_prod` gives the rounded product, and an error that, when finite, adds back to it
    /// without changing it, for every finite first factor and the second factor `b`. It is proven
    /// for six second factors, one for each of its paths, one harness each (kissat as the SAT
    /// solver): 3 and −0.1 (inexact products of either sign on the direct path), [`SPLITTER`] (a
    /// factor that splits exactly), 2^996 (the scaled path for a large factor), the smallest
    /// subnormal (the scaled path for a tiny product, whose error can be finer than any double)
    /// and the largest double (overflow). Over every pair at once, or the six in one harness, the
    /// solver ran past CI's hour.
    fn two_prod_law(b: f64) {
        // Significands of at most 14 bits (the low 39 fraction bits zero), every sign and exponent:
        // with a symbolic 53-bit significand every factor ran past CI's hour (run 40323412527).
        let a = f64::from_bits(kani::any::<u64>() & !((1 << 39) - 1));
        kani::assume(a.is_finite());
        let (p, e) = two_prod(a, b);
        assert!(p.to_bits() == (a * b).to_bits());
        if p.is_finite() && e.is_finite() {
            assert!(p + e == p);
        }
    }

    /// [`two_prod_law`] with 3: an inexact positive product on the direct path.
    #[kani::proof]
    #[kani::solver(kissat)]
    fn two_prod_with_three() {
        two_prod_law(3.0);
    }

    /// [`two_prod_law`] with −0.1: an inexact negative product on the direct path.
    #[kani::proof]
    #[kani::solver(kissat)]
    fn two_prod_with_minus_a_tenth() {
        two_prod_law(-0.1);
    }

    /// [`two_prod_law`] with [`SPLITTER`]: a factor that splits exactly.
    #[kani::proof]
    #[kani::solver(kissat)]
    fn two_prod_with_the_splitter() {
        two_prod_law(SPLITTER);
    }

    /// [`two_prod_law`] with 2^996: the scaled path for a large factor.
    #[kani::proof]
    #[kani::solver(kissat)]
    fn two_prod_with_two_pow_996() {
        two_prod_law(TWO_POW_996);
    }

    /// [`two_prod_law`] with the smallest subnormal: the scaled path for a tiny product.
    #[kani::proof]
    #[kani::solver(kissat)]
    fn two_prod_with_the_smallest_subnormal() {
        two_prod_law(f64::from_bits(1));
    }

    /// [`two_prod_law`] with the largest double: overflow.
    #[kani::proof]
    #[kani::solver(kissat)]
    fn two_prod_with_the_largest_double() {
        two_prod_law(f64::MAX);
    }
}
