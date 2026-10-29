//! Small total helpers shared by the correctly rounded ports: the fused multiply-add and the exact
//! error of a product, shifts that cannot overflow, the halves and the two's complement reading of an integer without a cast,
//! table reads that cannot panic, and the C sources' fast two-sum.
//!
//! The C sources lean on things that are undefined, partial or silent in C and would panic, be
//! partial or be denied in Rust: `__builtin_fma`, shifts by a variable count, casts between
//! integer widths and signedness, and array indexing. Each has one total, deterministic
//! replacement here, so the ports themselves contain no `unsafe`, no indexing, no cast that could
//! lose a value and no shift that could overflow.
//!
//! The ports' other integer steps follow the crate's rule (`lib.rs`): a `checked_*` method whose
//! `None`, which each call site proves cannot happen, ends the computation with the canonical NaN
//! (as a table read out of range does here, [`entry`]), so a broken proof shows as a NaN, which the
//! public functions report as `NotFinite`, never as a plausible wrong number; `abs_diff`, exact for
//! every input, where the order of the operands is known; and a `wrapping_*` method only where the
//! C source computes modulo 2^64 or 2^128 on purpose, each with a comment saying why.
//!
//! Guarantees: every helper is total and deterministic, so the ports contain no `unsafe`, no
//! indexing, no cast that could lose a value and no shift that could overflow; a table read out
//! of range gives the canonical NaN, never a panic or a plausible wrong number.
//!
//! Cost: constant time and no allocation: each helper is a few integer or floating-point
//! operations.
//!
//! Methods: the fused multiply-add is IEEE 754-2019's fusedMultiplyAdd (§5.4.1,
//! <https://doi.org/10.1109/IEEESTD.2019.8766229>); the fast two-sum is Dekker's (T. J. Dekker, "A
//! floating-point technique for extending the available precision", *Numerische Mathematik* 18
//! (1971), <https://doi.org/10.1007/BF01397083>), in the three operations the C sources write.

/// `a · b + c` rounded once (IEEE 754 fusedMultiplyAdd), as C's `__builtin_fma`.
///
/// From the pure-Rust `libm` crate, which uses the hardware instruction when the processor has
/// one and a correctly rounded software algorithm otherwise. Correct rounding fixes the result
/// bit for bit, so both give the same bits on every target (chapter 21). Rust never contracts
/// `a * b + c` into a fused operation on its own (RFC 3514), so every fused operation in the ports
/// is one of these calls, exactly where the C source calls `__builtin_fma`.
#[inline]
pub(super) fn fma(a: f64, b: f64, c: f64) -> f64 {
    libm::fma(a, b, c)
}

/// The exact error `a·b − h` of the product `h = RN(a·b)`, for a product whose error a double
/// holds (`|a·b| ≥ 2^−969`, Lemma 3 (ii) of [`super`]), as C's `__builtin_fma(a, b, -h)`.
///
/// By the fused multiply-add [`fma`], except on WebAssembly, which has no fused multiply-add
/// instruction: there `libm` computes one in software, about three times the cost of Dekker's
/// two-product, which gives the same exact error without one (evidence note 05, section 7: "An
/// FMA `TwoProduct` would therefore be a slow software call there, so keep Dekker"), so there the
/// error is [`crate::eft::two_prod`]'s. The two are the same bits: each is the exact value
/// `a·b − h`, and a zero is +0 in both (IEEE 754's sign of an exact zero sum of operands of
/// opposite sign, and `crate::eft`'s one-zero rule). Natively the code is the C source's.
#[inline]
pub(super) fn product_error(a: f64, b: f64, h: f64) -> f64 {
    #[cfg(not(target_family = "wasm"))]
    {
        fma(a, b, -h)
    }
    #[cfg(target_family = "wasm")]
    {
        let _ = h;
        crate::eft::two_prod(a, b).1
    }
}

/// `x · 2^n` truncated to 64 bits, for any `n`: 0 once every bit has been shifted out.
///
/// C leaves a shift by 64 or more undefined, and Rust panics on it with overflow checks on and
/// shifts by `n mod 64` without them; this is the mathematical meaning of the shift that the C
/// code relies on. The native shift runs only for `n < 64`.
#[inline]
pub(super) const fn shl(x: u64, n: u64) -> u64 {
    if n < 64 { x << n } else { 0 }
}

/// `⌊x / 2^n⌋` for any `n`: 0 once every bit has been shifted out.
#[inline]
pub(super) const fn shr(x: u64, n: u64) -> u64 {
    if n < 64 { x >> n } else { 0 }
}

/// `⌊x / 2^n⌋` for a signed `x` (an arithmetic shift) and any `n`: −1 or 0 once every bit has been
/// shifted out.
#[inline]
pub(super) const fn sar(x: i64, n: u64) -> i64 {
    if n < 64 {
        x >> n
    } else if x < 0 {
        -1
    } else {
        0
    }
}

/// `x · 2^n` truncated to 128 bits, for any `n`.
#[inline]
pub(super) const fn shl128(x: u128, n: u64) -> u128 {
    if n < 128 { x << n } else { 0 }
}

/// `⌊x / 2^n⌋` for 128-bit `x` and any `n`.
#[inline]
pub(super) const fn shr128(x: u128, n: u64) -> u128 {
    if n < 128 { x >> n } else { 0 }
}

/// The low and high 64-bit halves of `x`, `x = high · 2^64 + low` (C: `(uint64_t) x` and
/// `x >> 64`): read from its little-endian bytes, so there is no cast and no shift, as
/// [`crate::wide`] takes a product apart.
#[inline]
pub(super) const fn halves(x: u128) -> (u64, u64) {
    let [
        b0,
        b1,
        b2,
        b3,
        b4,
        b5,
        b6,
        b7,
        b8,
        b9,
        b10,
        b11,
        b12,
        b13,
        b14,
        b15,
    ] = x.to_le_bytes();
    (
        u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7]),
        u64::from_le_bytes([b8, b9, b10, b11, b12, b13, b14, b15]),
    )
}

/// The low 64 bits of `x`, `x mod 2^64` (C: `(uint64_t) x`, which keeps exactly these).
#[inline]
pub(super) const fn lo64(x: u128) -> u64 {
    halves(x).0
}

/// The high 64 bits of `x`, `⌊x / 2^64⌋` (C: `x >> 64`).
#[inline]
pub(super) const fn hi64(x: u128) -> u64 {
    halves(x).1
}

/// The 128-bit integer `high · 2^64 + low`, from its bytes.
#[inline]
pub(super) const fn join(high: u64, low: u64) -> u128 {
    let [b0, b1, b2, b3, b4, b5, b6, b7] = low.to_le_bytes();
    let [b8, b9, b10, b11, b12, b13, b14, b15] = high.to_le_bytes();
    u128::from_le_bytes([
        b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15,
    ])
}

/// The 64 bits of `x` read as a two's complement integer (C: `(int64_t) x`). The same bits, so
/// exact as a reinterpretation; for `x < 2^63` it is the value of `x` itself.
#[inline]
pub(super) const fn signed(x: u64) -> i64 {
    i64::from_le_bytes(x.to_le_bytes())
}

/// The two's complement bits of `x` as an unsigned integer (C: `(uint64_t) x`): `x` for `x ≥ 0`,
/// `x + 2^64` for `x < 0`. The same bits, so exact as a reinterpretation.
#[inline]
pub(super) const fn unsigned(x: i64) -> u64 {
    u64::from_le_bytes(x.to_le_bytes())
}

/// The double with the bits computed by a checked step, or [`super::NAN`] if the step's `None`
/// came (each call site proves it cannot): the poison of [`entry`], never a plausible wrong value.
#[inline]
pub(super) const fn from_checked_bits(bits: Option<u64>) -> f64 {
    match bits {
        Some(b) => f64::from_bits(b),
        None => super::NAN,
    }
}

/// `2^n` for `−1022 ≤ n ≤ 1023`, from its exponent field `n + 1023` (C: building the bits of a
/// power of two). [`super::NAN`] for an `n` outside that range, or for `None` (the result of a
/// checked step that overflowed), which each call site proves cannot come. The field is checked
/// to be 1 to 2046 before it is shifted into place, so the shift keeps every bit.
#[inline]
pub(super) fn pow2(n: Option<i64>) -> f64 {
    from_checked_bits(
        n.and_then(|n| n.checked_add(1023))
            .and_then(|field| u64::try_from(field).ok())
            .filter(|field| (1..=2046).contains(field))
            .map(|field| field << 52),
    )
}

/// A value that marks a table read that went out of range: NaN in every component.
///
/// Every call of [`entry`] reads in range by construction, so this value is never produced. If a
/// future change broke that, the NaN would propagate to the function's result (a NaN rounding
/// test is never passed, and a NaN sum is NaN), where every test catches it and the public
/// functions report it as `NotFinite`, instead of a plausible wrong number.
pub(super) trait Poison: Copy {
    /// The poison value.
    const POISON: Self;
}

impl Poison for f64 {
    const POISON: f64 = super::NAN;
}

impl<const N: usize> Poison for [f64; N] {
    const POISON: [f64; N] = [super::NAN; N];
}

/// Entry `i` of a table, or [`Poison::POISON`] if `i` is out of range.
///
/// Every call site's index is in range by construction (each says why: a mask, a range check or
/// the table's own construction), so the poison is never returned; it exists so that a table
/// read is total, cannot panic, and could never silently yield a plausible value. The index is a
/// `usize` or, as the ports compute it from bit fields, a `u64`; one that does not fit a `usize`
/// is out of range too.
#[inline]
pub(super) fn entry<T: Poison, I: TryInto<usize>>(table: &[T], i: I) -> T {
    match i.try_into().ok().and_then(|i| table.get(i)) {
        Some(v) => *v,
        None => T::POISON,
    }
}

/// The sum `x + y` rounded and its error (Dekker's fast two-sum): the C sources' `fasttwosum`
/// (`fast_two_sum` in log.c), the same three operations in the same order, so that each port
/// computes exactly what its C source computes.
///
/// The pair is exact, `s + e = x + y`, when `x = 0`, when `|x| ≥ |y|`, or when `x` is an integer
/// multiple of the last-place weight of `y` (Lemma 3(i) of [`super`]); each call site's proof says
/// which applies there. Deliberately not [`crate::eft::fast_two_sum`], which first puts its
/// operands in order of magnitude: that is a different computation from the C source's (a
/// comparison more on every fast path), and these ports must not change when that module does.
#[inline]
pub(super) const fn fast_two_sum(x: f64, y: f64) -> (f64, f64) {
    let s = x + y;
    let z = s - x;
    (s, y - z)
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

    /// The helpers against the native operations they replace, on the edges of their ranges.
    #[test]
    fn helpers_match_the_native_operations() {
        let words = [0u64, 1, 2, 0x7ff, 1 << 52, (1 << 63) - 1, 1 << 63, u64::MAX];
        for &x in &words {
            for n in [0u64, 1, 11, 52, 63, 64, 65, 127, 128, 1000, u64::MAX] {
                let want_l = if n < 64 { x << n } else { 0 };
                let want_r = if n < 64 { x >> n } else { 0 };
                assert_eq!(shl(x, n), want_l, "{x} {n}");
                assert_eq!(shr(x, n), want_r, "{x} {n}");
                let s = x as i64;
                let want_a = if n < 64 { s >> n } else { s >> 63 };
                assert_eq!(sar(s, n), want_a, "{s} {n}");
                let w = (u128::from(x) << 64) | u128::from(x ^ 0x5555);
                let left128 = if n < 128 { w << n } else { 0 };
                let right128 = if n < 128 { w >> n } else { 0 };
                assert_eq!(shl128(w, n), left128);
                assert_eq!(shr128(w, n), right128);
            }
            assert_eq!(signed(x), x as i64);
            assert_eq!(unsigned(x as i64), x);
            for &y in &words {
                let w = (u128::from(y) << 64) | u128::from(x);
                assert_eq!(halves(w), (x, y));
                assert_eq!((lo64(w), hi64(w)), (w as u64, (w >> 64) as u64));
                assert_eq!(join(y, x), w);
            }
        }
    }

    /// `pow2` builds every normal power of two and refuses the rest with the poison.
    #[test]
    fn powers_of_two() {
        let mut p = f64::MIN_POSITIVE;
        for n in -1022..=1023 {
            assert_eq!(pow2(Some(n)), p, "{n}");
            p *= 2.0;
        }
        for n in [
            Some(-1075),
            Some(-1023),
            Some(1024),
            Some(i64::MIN),
            Some(i64::MAX),
            None,
        ] {
            assert_eq!(pow2(n).to_bits(), super::super::NAN.to_bits(), "{n:?}");
        }
        assert_eq!(
            from_checked_bits(None).to_bits(),
            super::super::NAN.to_bits()
        );
        assert_eq!(from_checked_bits(Some(1.5f64.to_bits())), 1.5);
    }

    /// A table read out of range, by `usize` or beyond it, is the poison.
    #[test]
    fn table_reads_are_total() {
        let t = [1.0, 2.0];
        assert_eq!(entry(&t, 1u64), 2.0);
        assert_eq!(entry(&t, 0usize), 1.0);
        assert!(entry(&t, 2u64).is_nan());
        assert!(entry(&t, u64::MAX).is_nan());
        assert!(entry(&t, usize::MAX).is_nan());
        let pairs = [[1.0, 2.0]];
        assert!(entry(&pairs, 1u64).iter().all(|v| v.is_nan()));
    }
}
