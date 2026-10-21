//! Error-free transformations: a rounded sum or product of two doubles together with its exact error.
//!
//! `two_sum(a, b)` returns `(s, e)` where `s` is `a + b` rounded and `s + e = a + b` exactly, in real
//! arithmetic. `two_prod(a, b)` returns `(p, e)` where `p` is `a · b` rounded and `p + e = a · b`
//! exactly. They are the building blocks of double-double arithmetic (the one-time conversion of
//! survey and map data into a frame, chapter 6.1) and of floating-point expansions.
//!
//! What every function here guarantees, under the precondition its own documentation states:
//!
//! - **Exact.** The identity holds in real arithmetic, not approximately. The tests check it against
//!   exact rationals (`num-rational`) on random and adversarial inputs.
//! - **The rounded part is the IEEE 754 result.** `s` is `a + b` and `p` is `a * b` as Rust computes
//!   them (round to nearest, ties to even), and the error is at most half a unit in the last place of
//!   that result, so the pair does not overlap: `s + e` rounds back to `s`.
//! - **Deterministic.** Only `+`, `−` and `×` on `f64`, which Rust computes to IEEE 754 on every target
//!   (RFC 3514). Rust never fuses `a * b + c` into one fused multiply-add, which would break these
//!   algorithms, and none is used on purpose either: `two_prod` follows Dekker's splitting instead of
//!   `mul_add`, which `core` lacks and which is a slow software call on wasm32 (evidence note 05,
//!   section 1).
//!
//! Method: Dekker 1971 (`fast_two_sum`, Veltkamp's `split`, `two_prod`) and Knuth (`two_sum`), in the
//! form and with the proofs of Shewchuk 1997: Theorem 6 (fast two-sum), Theorem 7 (two-sum),
//! Theorem 17 (split) and Theorem 18 (two-product). Shewchuk's proofs assume an unbounded exponent
//! (his section 2.1 leaves overflow and underflow aside); the overflow and underflow conditions below
//! are derived here for binary64 and tested at their edges.
//!
//! - T. J. Dekker, "A floating-point technique for extending the available precision", *Numerische
//!   Mathematik* 18 (1971), 224–242, <https://doi.org/10.1007/BF01397083>.
//! - J. R. Shewchuk, "Adaptive precision floating-point arithmetic and fast robust geometric
//!   predicates", *Discrete & Computational Geometry* 18 (1997), 305–363,
//!   <https://doi.org/10.1007/PL00009321>.
//! - D. E. Knuth, *The Art of Computer Programming*, volume 2, section 4.2.2.
//!
//! Notation used in the proofs: Ω = (2 − 2^−52) · 2^1023 is the largest finite double. For a nonzero
//! normal double x, e_x is its binary exponent, so 2^e_x ≤ |x| < 2^(e_x + 1), and the last bit of its
//! significand is worth u_x = 2^(e_x − 52). A sum or difference of two doubles that lands in the
//! subnormal range is exact (its exact value is a whole multiple of 2^−1074 below 2^−1022), so
//! gradual underflow never spoils an addition.

/// Veltkamp's splitting constant for binary64, 2^27 + 1: splits a 53-bit significand into two
/// halves of 26 bits each (Shewchuk 1997, Theorem 17, with splitting point s = 27).
pub const SPLITTER: f64 = 134_217_729.0;

/// The sum `a + b` rounded, and its exact error, for `|a| ≥ |b|` or `a == 0` (Dekker's fast two-sum).
///
/// Returns `(s, e)` with `s = a + b` rounded to nearest and `s + e = a + b` exactly, `|e|` at most
/// half a unit in the last place of `s`. Three operations and no branch.
///
/// Precondition: `a` and `b` finite, `s` finite, and `|a| ≥ |b|` or `a == 0` (which makes `s = b`
/// and `e = 0` exactly). Without the ordering the result is not exact; [`two_sum`] needs no ordering.
///
/// Proof (Shewchuk 1997, Theorem 6): with `|a| ≥ |b|`, `z = s − a` and `e = b − z` are computed
/// exactly, with or without gradual underflow. Overflow: both are exact, so each is a double, and
/// neither can overflow.
///
/// Finite inputs never produce NaN here, even outside the precondition: `z` is `s` or `±∞` minus a
/// finite number and `e` is a finite number minus `z`, so no step subtracts two infinities.
#[must_use]
pub const fn fast_two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let z = s - a;
    let e = b - z;
    (s, e)
}

/// The sum `a + b` rounded, and its exact error, for any finite `a` and `b` (Knuth's two-sum).
///
/// Returns `(s, e)` with `s = a + b` rounded to nearest and `s + e = a + b` exactly, `|e|` at most
/// half a unit in the last place of `s`.
///
/// Precondition: `a` and `b` finite, and `s` finite (the rounded sum does not overflow). No ordering
/// of `a` and `b` is needed.
///
/// Proof: Knuth's six operations (Shewchuk 1997, Theorem 7) are exact when no intermediate
/// overflows, with or without gradual underflow. With a bounded exponent one intermediate can
/// overflow although `s` does not: for `a = −3 · 2^970` and `b = Ω`, `s = Ω − 2^971` (a tie, rounded
/// to even), and `s − a` is exactly Ω + 2^970, the midpoint between Ω and 2^1024, which rounds to
/// infinity. That can only happen when `|b| > |a|`: when `|a| ≥ |b|`, `s − a` and `b − (s − a)` are
/// exact (Shewchuk 1997, Theorem 6), the next value is exactly `a`, and every step is exact. An
/// infinity, once made, is never absorbed by a later addition (each gives an infinity or NaN), and
/// every intermediate feeds `e`, so any intermediate overflow shows as a non-finite `e`. In that case
/// the function returns [`fast_two_sum`]`(b, a)`, which is exact because `|b| > |a|`, and has the same
/// `s` because IEEE addition is commutative. So the result is exact for every finite input whose
/// rounded sum is finite.
///
/// Finite inputs never produce NaN here: if `s` overflows, the same fallback returns `s` and the
/// opposite infinity.
#[must_use]
pub const fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let b_virtual = s - a;
    let a_virtual = s - b_virtual;
    let e = (a - a_virtual) + (b - b_virtual);
    if e.is_finite() { (s, e) } else { fast_two_sum(b, a) }
}

/// Veltkamp's split of `a` into a high and a low half: `a = hi + lo` exactly, each half a
/// 26-bit value, and `|hi| ≥ |lo|`.
///
/// Precondition: `a == 0`, or `a` normal (`|a| ≥ 2^−1022`) and `|a| < 2^996`.
///
/// Bit budget (Shewchuk 1997, Theorem 17, with p = 53 and splitting point s = 27, multiplier
/// [`SPLITTER`] = 2^27 + 1): `hi` has p − s = 26 significant bits and `lo` has s − 1 = 26; the 53rd
/// bit of `a` is carried by the sign of `lo`. `hi` has the exponent of `a` or one more (then it is a
/// single bit), so `|hi| ≤ 2^(e_a + 1)`. Both halves are whole multiples of u_a: `hi` is the
/// difference of two doubles at least as large as `a`, and `lo = a − hi`.
///
/// Overflow: `|a| < 2^996` gives `|SPLITTER · a| < 2^996 · (2^27 + 1) = 2^1023 + 2^996 < Ω`; `c − a`
/// is smaller than `c` in magnitude (same sign); the last two operations are exact (Sterbenz's
/// lemma). So nothing overflows. Underflow: for normal `a` the two rounded operations have results of
/// magnitude above 2^26 · 2^−1022, so they are normal and Shewchuk's proof applies unchanged.
/// `split(0)` is `(0, 0)`.
#[must_use]
pub const fn split(a: f64) -> (f64, f64) {
    let c = SPLITTER * a;
    let big = c - a;
    let hi = c - big;
    let lo = a - hi;
    (hi, lo)
}

/// The product `a · b` rounded, and its exact error, by Dekker's algorithm without fused
/// multiply-add.
///
/// Returns `(p, e)` with `p = a * b` rounded to nearest and `p + e = a · b` exactly, `|e|` at most
/// half a unit in the last place of `p`.
///
/// Precondition, in two parts:
/// 1. `|a| < 2^996` and `|b| < 2^996`, so neither split overflows; and
/// 2. `a == 0` or `b == 0`; or `a` and `b` both normal with −970 ≤ e_a + e_b ≤ 1021. The exponent
///    condition holds whenever `2^−968 ≤ |a · b| < 2^1022`.
///
/// Outside the precondition the pair is not exact and may be infinite or NaN; callers establish it
/// from their own bounds (products of grid coordinates, below 2^41, are far inside it).
///
/// Bit budget and proof (Shewchuk 1997, Theorem 18, Veltkamp's method in Dekker's paper):
/// - [`split`] gives `a = a_hi + a_lo` and `b = b_hi + b_lo`, each half of 26 bits, so each of the
///   four partial products `a_hi·b_hi`, `a_lo·b_hi`, `a_hi·b_lo`, `a_lo·b_lo` has at most
///   26 + 26 = 52 ≤ 53 significant bits.
/// - Underflow: every half of `a` is a whole multiple of u_a and every half of `b` of u_b. So every
///   partial product, `p` (normal, since `|a · b| ≥ 2^(e_a + e_b) ≥ 2^−970`), the exact error and
///   every intermediate is a whole multiple of u_a · u_b = 2^(e_a + e_b − 104) ≥ 2^(−970 − 104) =
///   2^−1074, the smallest subnormal. A value on that grid with at most 53 significant bits is a
///   double, so every step Shewchuk proves exact in unbounded arithmetic is exact here too.
/// - Overflow: `|a_hi| ≤ 2^(e_a + 1)` and `|a_lo| ≤ |a_hi|` (likewise for `b`), so every partial
///   product is at most 2^(e_a + e_b + 2) ≤ 2^1023 in magnitude, and so is `p`. The remaining
///   operations are exact, so each result is a double and none overflows.
/// - With a zero factor both halves of that factor are zero (the other factor's halves are finite
///   by part 1), every partial product is zero, and `p` and `e` are zero.
#[must_use]
pub const fn two_prod(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    let (a_hi, a_lo) = split(a);
    let (b_hi, b_lo) = split(b);
    let err1 = p - a_hi * b_hi;
    let err2 = err1 - a_lo * b_hi;
    let err3 = err2 - a_hi * b_lo;
    let e = a_lo * b_lo - err3;
    (p, e)
}
