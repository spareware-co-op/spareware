//! `ExactSum`: the exact sum of any number of doubles, rounded once, to nearest, at the end.
//!
//! An [`ExactSum`] is a superaccumulator: a fixed-point number wide enough to hold the sum of any
//! realistic number of doubles exactly. Terms are added with integer operations only, so nothing
//! is rounded until [`ExactSum::round`] rounds the exact sum once, to nearest with ties to even.
//! Chapter 23.4 routes every float reduction whose order could vary (chunks, threads, container
//! order) through it: two accumulators merge by integer addition ([`ExactSum::merge`]), so a sum
//! split across chunks or threads gives the same bits however it is split.
//!
//! Guarantees:
//!
//! - **Exact, then correctly rounded.** [`ExactSum::round`] returns the exact sum of every finite
//!   term added (directly, by [`ExactSum::add_slice`] or [`ExactSum::add_iter`], or through
//!   merges) rounded to the nearest double, ties to even, as a [`Real`]. An exact zero is +0,
//!   whatever the signs of the zero terms (as in Neal's code). The tests hold it bit for bit to an
//!   exact oracle (`dashu`).
//! - **Order-free.** The result depends on the multiset of terms only: not on their order, on how
//!   they were split into slices or accumulators, on the order or shape of the merges, or on which
//!   of the two internal paths (below) took them. The state's value is the exact sum, the special
//!   flags are an OR and a maximum, and the rounding is a function of the value.
//! - **Special values as Neal specifies.** A NaN or an infinity among the terms does not disturb
//!   the finite sum; [`ExactSum::round_ieee`] then returns, in this order of precedence: a NaN, the
//!   one with the largest payload (its 52 fraction bits read as an integer) with its sign cleared,
//!   if any term was a NaN; a NaN if both +∞ and −∞ were added; the infinity if only one sign was;
//!   otherwise the rounded finite sum, which is ±∞ when its magnitude is at least 2^1024 − 2^970
//!   (an overflow, possible even when every term is finite). [`ExactSum::round`] reports all of
//!   these as [`NumError::NotFinite`], so the kernel never sees a NaN or an infinity from it.
//! - **No silent overflow.** Integer arithmetic is checked, except the one addition Neal's
//!   method defines modulo 2^64 (the large accumulator's chunks, below), where wrapping is the
//!   specification. The accumulator holds magnitudes below 2^1100, so it cannot overflow before
//!   2^64 terms have been added (a term at most Ω < 2^1024, counted once for every accumulator it
//!   reaches through merges); should an internal step ever fail, the sum is marked lost and
//!   [`ExactSum::round_ieee`] gives Neal's report for it, a NaN with every payload bit set.
//!
//! Method: R. M. Neal, "Fast exact summation using small and large superaccumulators", arXiv
//! 1505.05571 (2015), <https://arxiv.org/abs/1505.05571>, and his C code `xsum` (MIT licence,
//! copyright 2015–2024 Radford M. Neal), <https://github.com/radfordneal/xsum>, read at commit
//! `b766b3e104cdbd6769c8df033fb430700fce32dd` (2024-11-13, `stable-version-3`). Both of Neal's
//! superaccumulators are used, rebuilt to this crate's rules:
//!
//! - **The small superaccumulator**, the value an [`ExactSum`] holds: 67 signed 64-bit chunks,
//!   chunk i weighing 2^(32i − 1075), so consecutive chunks overlap by 32 bits and carries can
//!   wait. A double with exponent field E (1 for subnormals) and 53-bit significand M (52 bits for
//!   subnormals) is `M·2^(E − 1075)`; with `E = 32j + k`, it adds or subtracts `M·2^k mod 2^32`
//!   to chunk j and `⌊M·2^k / 2^32⌋ < 2^52` to chunk j + 1. A carry propagation (each chunk keeps
//!   its low 32 bits, the rest goes to the next) puts every chunk but the top one in [0, 2^32);
//!   2047 additions can follow before the next, since 2^32 + 2047·2^52 < 2^63. Neal, section 3.
//! - **The large superaccumulator**, used by [`ExactSum::add_slice`] for long slices and by
//!   [`ExactSum::add_iter`] (with [`Extend`] and [`FromIterator`]) for long streams: 4096
//!   unsigned 64-bit chunks, one for each sign-and-exponent pattern, each with a count. A term is
//!   added to its chunk as a whole 64-bit pattern, with no masking; after n additions the chunk
//!   holds `n·(index·2^52) + Σ fractions` modulo 2^64, so at most 4096 additions per chunk keep
//!   `Σ fractions < 2^64` recoverable exactly: `chunk − n·index·2^52`, modulo 2^64. That chunk is
//!   then moved into the small accumulator (fractions plus `n·2^52` implicit bits, as three 32-bit
//!   parts), when its count runs out and at the end. Neal, section 4.
//!
//! Changes from Neal's code, and why:
//!
//! - The rounding is rewritten (`round_finite`): the magnitude is made by negating and
//!   propagating again, then rounded from a 96-bit window and a sticky bit, the same way for both
//!   signs. Neal rounds negative sums with separate two's-complement logic, where his own 2024
//!   release fixed a bug (a negative result whose significand is all ones rounded the wrong way).
//! - Carry propagation sweeps all 67 chunks, without Neal's search for the topmost nonzero chunk
//!   or his folding of a top chunk of −1; it runs once every 2047 additions, so its cost (about 67
//!   steps) is 0.03 steps per term, and the rounding needs no canonical top chunk.
//! - Only the small accumulator is kept between calls (about 560 bytes, cheap to merge and
//!   move); the large one (41 KB) lives on the stack inside one [`ExactSum::add_slice`] or
//!   [`ExactSum::add_iter`] call and empties into the small one before it returns. Neal keeps both
//!   for the life of a sum, so his `xsum_large_add1` is fast for terms added one at a time; here
//!   [`ExactSum::add`] uses the small one alone, and a long stream of terms goes through
//!   [`ExactSum::add_iter`] to get the large one's speed. The crate is `no_std` without
//!   allocation, so the large accumulator is zeroed rather than left uninitialised.
//! - A sum of +∞ and −∞ gives the quiet NaN with only its top fraction bit set, the same bits on
//!   every target; Neal computes `∞ − ∞`, whose bits depend on the processor.
//! - Every integer step that is not Neal's modulo-2^64 chunk sum is a checked method, with the
//!   sticky "lost" flag as the report a failure would make (it cannot happen, as above).
//!
//! Cost: [`ExactSum::add`]: about twenty integer operations, two of them checked additions
//! into chunks, plus a propagation (about 67 steps) every 2047 terms. [`ExactSum::add_slice`] from
//! [`LARGE_SLICE`] terms up: about eight per term (a shift, two loads, a checked decrement, the
//! unmasked addition and two stores), plus zeroing 41 KB once per call, one transfer (about twenty
//! steps) per 4096 terms with the same sign and exponent, and a final transfer of each chunk used.
//! [`ExactSum::add_iter`]: the same per term, after at most [`LARGE_SLICE`] terms one by one when
//! the stream's length is not known. Neal measures exact summation of large arrays this way in
//! under twice the time of plain ordered summation; `tests/exact_sum_bench.rs` holds `add_slice`
//! and `add_iter` to that on 10^6 terms, including the inputs hardest for the large accumulator
//! (a few chunks hit in random order). Measured there in release on an Intel i7-10750H with the
//! machine otherwise quiet (2026-10-22), against a plain ordered sum of the same terms: `add_slice`
//! and `add_iter` 0.9 to 1.4 times on uniform, spread, coordinate and one-binade terms, 1.7 times
//! on the hardest case (subnormals of random signs); [`ExactSum::add`] one term at a time 3 to 6
//! times. So the board's bar is met by the large accumulator's paths, which kernel code with many
//! terms uses; [`ExactSum::add`] is for a few terms at a time. (Under load from other processes
//! the large accumulator's loop, bound by loads and stores, slows more than the plain sum, bound
//! by the latency of one addition, so the ratio is only meaningful on a quiet machine.)
//! [`ExactSum::merge`]: two propagations and 67 checked additions. [`ExactSum::round`]: a copy of
//! the 67 chunks, at most two propagations and a scan.

use crate::error::NumError;
use crate::real::Real;

/// The number of chunks of the small accumulator: 64 for the 6 high bits of an exponent field,
/// one for the second chunk a term reaches, and two for carries and transfers above them (Neal's
/// 67).
const CHUNKS: usize = 67;

/// The low 32 bits of a chunk.
const LOW_MASK: i64 = 0xffff_ffff;

/// The 52 fraction bits of a double.
const FRACTION_BITS: u64 = (1 << 52) - 1;

/// The exponent field of an infinity or NaN.
const SPECIAL_FIELD: u64 = 0x7ff;

/// Additions allowed after a carry propagation: each changes a chunk by less than 2^52, and a
/// propagated chunk is below 2^32 in magnitude, so 2^32 + 2047·2^52 < 2^63 keeps every chunk inside
/// an `i64` (Neal: 2^11 − 1).
const ADDS_AFTER_PROPAGATION: u32 = 2047;

/// Additions allowed after a merge, whose chunks are sums of two propagated chunks, below 2^33:
/// 2^33 + 2046·2^52 < 2^63.
const ADDS_AFTER_MERGE: u32 = 2046;

/// The large accumulator's chunks: one for each 12-bit sign-and-exponent pattern.
const LARGE_CHUNKS: usize = 4096;

/// Additions to one large chunk before it must move to the small accumulator: n terms add less
/// than n·2^52 of fraction bits, which stays below 2^64 for n ≤ 4096 = 2^12.
const LARGE_ADDS: u16 = 4096;

/// From this many terms, [`ExactSum::add_slice`] and [`ExactSum::add_iter`] use the large
/// accumulator: below it, zeroing 41 KB costs more than the large accumulator saves. Measured in
/// release on an x86-64 workstation (2026-10-21), for terms uniform in [−1, 1): 256 terms took
/// 1.4 µs one by one and 1.8 µs through the large accumulator, 512 terms 3.1 µs and 2.2 µs, 1024
/// terms 6.8 µs and 3.0 µs. Neal finds the crossover near a thousand terms on his machines.
pub const LARGE_SLICE: usize = 512;

/// A NaN whose payload is every fraction bit: Neal's report of a sum that left the accumulator.
const LOST_NAN: f64 = f64::from_bits(0x7fff_ffff_ffff_ffff);

/// The quiet NaN with only the top fraction bit set: the result of +∞ and −∞ together.
const INFINITIES_NAN: f64 = f64::from_bits(0x7ff8_0000_0000_0000);

const _: () = {
    // The overflow margins the two constants rest on.
    assert!((1_i128 << 32) + (ADDS_AFTER_PROPAGATION as i128) * (1 << 52) < (1 << 63));
    assert!((1_i128 << 33) + (ADDS_AFTER_MERGE as i128) * (1 << 52) < (1 << 63));
    // n ≤ 4096 terms' fractions, each below 2^52, fit 64 bits.
    assert!((LARGE_ADDS as u128) * ((1 << 52) - 1) < (1 << 64));
    assert!(LARGE_CHUNKS == 1 << 12);
};

/// The exact sum of doubles, rounded once at the end (module documentation).
///
/// The value is `Σ chunks[i]·2^(32i − 1075)`, exactly.
#[derive(Clone, Debug)]
pub struct ExactSum {
    /// The small superaccumulator's chunks.
    chunks: [i64; CHUNKS],
    /// Additions left before a carry propagation is due.
    adds_left: u32,
    /// Whether +∞ was added.
    positive_infinity: bool,
    /// Whether −∞ was added.
    negative_infinity: bool,
    /// The largest NaN payload added (its 52 fraction bits, never 0 for a NaN), or 0 if none.
    nan_payload: u64,
    /// Whether an internal step failed, so the value is lost (cannot happen; module documentation).
    lost: bool,
}

impl Default for ExactSum {
    fn default() -> Self {
        ExactSum::new()
    }
}

/// [`ExactSum::add_iter`]: `sum.extend(terms)`.
impl Extend<f64> for ExactSum {
    fn extend<I: IntoIterator<Item = f64>>(&mut self, terms: I) {
        self.add_iter(terms);
    }
}

/// An [`ExactSum`] of every term, by [`ExactSum::add_iter`]: `terms.collect::<ExactSum>()`, the
/// kernel's replacement for `Iterator::sum` (chapter 23.4).
impl FromIterator<f64> for ExactSum {
    fn from_iter<I: IntoIterator<Item = f64>>(terms: I) -> Self {
        let mut sum = ExactSum::new();
        sum.add_iter(terms);
        sum
    }
}

/// The 12 sign and exponent bits of a double's bits, as an index below 4096.
#[allow(
    clippy::cast_possible_truncation,
    reason = "a u64 shifted right by 52 places is below 2^12, which every usize holds"
)]
const fn sign_and_exponent(bits: u64) -> usize {
    (bits >> 52) as usize
}

/// The exponent field of a double's bits, as a chunk position: 11 bits, below 2^11.
#[allow(
    clippy::cast_possible_truncation,
    reason = "masked to 11 bits, below 2^11, which every u32 holds"
)]
const fn exponent_field(bits: u64) -> u32 {
    ((bits >> 52) & SPECIAL_FIELD) as u32
}

/// A value below 2^63 as an `i64`, which it fits.
#[allow(
    clippy::cast_possible_wrap,
    reason = "every caller passes a value masked to 32 bits or shifted below 2^52 (bounds beside \
              each call), below 2^63"
)]
const fn small(v: u64) -> i64 {
    v as i64
}

impl ExactSum {
    /// An empty sum: zero.
    #[must_use]
    pub const fn new() -> ExactSum {
        ExactSum {
            chunks: [0; CHUNKS],
            adds_left: ADDS_AFTER_PROPAGATION,
            positive_infinity: false,
            negative_infinity: false,
            nan_payload: 0,
            lost: false,
        }
    }

    /// Adds one term, exactly. NaN and infinities are recorded as the module documentation says.
    ///
    /// Through the small superaccumulator only, which costs several times a plain `+=` per term
    /// (module documentation, *Cost*). For many terms, [`ExactSum::add_slice`] and
    /// [`ExactSum::add_iter`] (or [`Extend`] and [`FromIterator`]) are within twice a plain sum.
    pub fn add(&mut self, x: f64) {
        self.count_addition();
        self.add_bits(x.to_bits());
    }

    /// Adds every term of a slice, exactly; the same result as calling [`ExactSum::add`] on each.
    ///
    /// From [`LARGE_SLICE`] terms up it goes through Neal's large superaccumulator, which costs
    /// a few operations per term, and empties it into this sum before returning.
    pub fn add_slice(&mut self, terms: &[f64]) {
        if terms.len() < LARGE_SLICE {
            for &x in terms {
                self.add(x);
            }
            return;
        }
        let mut large = Large::new();
        for &x in terms {
            large.add(x.to_bits(), self);
        }
        large.empty_into(self);
    }

    /// Adds every term an iterator yields, up to its first `None`, exactly; the same result as
    /// calling [`ExactSum::add`] on each. For terms computed on the fly, which are not in a slice.
    ///
    /// Like [`ExactSum::add_slice`], a long stream goes through Neal's large superaccumulator, one
    /// term at a time as his `xsum_large_add1` does, at the same cost per term. The length is not
    /// known in advance, so unless the iterator's lower size bound is already [`LARGE_SLICE`] or
    /// more, the first [`LARGE_SLICE`] terms are added one by one, and the large accumulator is
    /// made only if more follow: a short stream never pays for zeroing it.
    pub fn add_iter<I: IntoIterator<Item = f64>>(&mut self, terms: I) {
        let mut terms = terms.into_iter().fuse();
        if terms.size_hint().0 < LARGE_SLICE {
            for x in terms.by_ref().take(LARGE_SLICE) {
                self.add(x);
            }
        }
        let Some(next) = terms.next() else {
            return;
        };
        let mut large = Large::new();
        large.add(next.to_bits(), self);
        for x in terms {
            large.add(x.to_bits(), self);
        }
        large.empty_into(self);
    }

    /// Adds another sum to this one, exactly: chunk by chunk, after a carry propagation of each,
    /// with the special values combined (OR of the infinities, the larger NaN payload). The order
    /// of merges never changes the result.
    pub fn merge(&mut self, other: &ExactSum) {
        self.propagate();
        let mut theirs = other.chunks;
        if propagate(&mut theirs).is_none() {
            self.lost = true;
        }
        for (mine, add) in self.chunks.iter_mut().zip(theirs) {
            match mine.checked_add(add) {
                Some(sum) => *mine = sum,
                None => self.lost = true,
            }
        }
        self.adds_left = ADDS_AFTER_MERGE;
        self.positive_infinity |= other.positive_infinity;
        self.negative_infinity |= other.negative_infinity;
        if other.nan_payload > self.nan_payload {
            self.nan_payload = other.nan_payload;
        }
        self.lost |= other.lost;
    }

    /// The exact sum rounded to the nearest double, ties to even, as a [`Real`]; an exact zero is
    /// +0.
    ///
    /// # Errors
    ///
    /// [`NumError::NotFinite`] if a NaN or an infinity was added, or if the sum's magnitude is at
    /// least 2^1024 − 2^970, so that it rounds to an infinity (the cases where
    /// [`ExactSum::round_ieee`] is not finite).
    pub fn round(&self) -> Result<Real, NumError> {
        Real::new(self.round_ieee())
    }

    /// The result as a double, NaN and infinities included, after Neal: a NaN if a NaN was added
    /// (the largest payload, sign cleared) or if both infinities were; an infinity if one was;
    /// otherwise the exact sum of the finite terms rounded to nearest, ties to even, which is ±∞
    /// on overflow. An exact zero is +0, even when every term was −0, where IEEE 754 addition
    /// would give −0 (as Neal's code does). (Module documentation.)
    #[must_use]
    pub fn round_ieee(&self) -> f64 {
        if self.lost {
            return LOST_NAN;
        }
        if self.nan_payload != 0 {
            return f64::from_bits((SPECIAL_FIELD << 52) | self.nan_payload);
        }
        match (self.positive_infinity, self.negative_infinity) {
            (true, true) => INFINITIES_NAN,
            (true, false) => f64::INFINITY,
            (false, true) => f64::NEG_INFINITY,
            (false, false) => round_finite(&self.chunks).unwrap_or(LOST_NAN),
        }
    }

    /// Takes one addition from the allowance, propagating carries first when none is left.
    fn count_addition(&mut self) {
        if let Some(left) = self.adds_left.checked_sub(1) {
            self.adds_left = left;
        } else {
            self.propagate();
            self.adds_left = ADDS_AFTER_PROPAGATION - 1;
        }
    }

    /// A carry propagation of this sum's chunks.
    fn propagate(&mut self) {
        if propagate(&mut self.chunks).is_none() {
            self.lost = true;
        }
        self.adds_left = ADDS_AFTER_PROPAGATION;
    }

    /// Records a NaN or an infinity (exponent field all ones): an infinity by its sign, a NaN by
    /// keeping the largest payload. Neal's `xsum_small_add_inf_nan`.
    fn add_special(&mut self, bits: u64) {
        let payload = bits & FRACTION_BITS;
        if payload == 0 {
            if bits >> 63 == 0 {
                self.positive_infinity = true;
            } else {
                self.negative_infinity = true;
            }
        } else if payload > self.nan_payload {
            self.nan_payload = payload;
        }
    }

    /// Adds the term with these bits to two chunks, the addition already counted (Neal's
    /// `xsum_add1_no_carry`). With E the exponent field (1 for a subnormal, whose significand M has
    /// no implicit bit) and `E = 32j + k`, the term is `±M·2^(E − 1075)`, which is
    /// `±(low·2^(32j − 1075) + high·2^(32(j + 1) − 1075))` with `low = M·2^k mod 2^32` and
    /// `high = ⌊M / 2^(32 − k)⌋ < 2^52` (M < 2^53 and 32 − k ≥ 1): `M·2^k = high·2^32 + low`.
    /// `j ≤ 63`, so both chunks exist. `M << k` may lose bits above 64; only its low 32 are kept,
    /// so that is the specification. A zero changes nothing.
    fn add_bits(&mut self, bits: u64) {
        let field = exponent_field(bits);
        let fraction = bits & FRACTION_BITS;
        let (exponent, significand) = if field == 0 {
            if fraction == 0 {
                return;
            }
            (1, fraction)
        } else if u64::from(field) == SPECIAL_FIELD {
            self.add_special(bits);
            return;
        } else {
            (field, fraction | (1 << 52))
        };
        let k = exponent & 31;
        let low = small((significand << k) & 0xffff_ffff);
        let high = small(significand >> 32_u32.abs_diff(k));
        let negative = bits >> 63 != 0;
        let Ok(j) = usize::try_from(exponent >> 5) else {
            self.lost = true;
            return;
        };
        if let Some([first, second, ..]) = self.chunks.get_mut(j..) {
            let ok = bump(first, low, negative) && bump(second, high, negative);
            if !ok {
                self.lost = true;
            }
        } else {
            self.lost = true;
        }
    }

    /// Adds `±S·2^(E − 1075)` for a large chunk: S the sum of n terms' significands (fractions plus
    /// `n·2^52` implicit bits when E ≥ 1), E its exponent field (1 for subnormals). S < 2^65, and
    /// with `E = 32j + k`, `S·2^k < 2^96` splits into three 32-bit parts for chunks j, j + 1 and
    /// j + 2 (j ≤ 63, so chunk 65 at most). Counted as one addition: each part is below 2^32.
    fn add_significands(&mut self, field: u32, significands: u128, negative: bool) {
        self.count_addition();
        let exponent = field.max(1);
        let shifted = significands << (exponent & 31);
        let parts = [
            shifted & 0xffff_ffff,
            (shifted >> 32) & 0xffff_ffff,
            shifted >> 64,
        ];
        let Ok(j) = usize::try_from(exponent >> 5) else {
            self.lost = true;
            return;
        };
        let Some([a, b, c, ..]) = self.chunks.get_mut(j..) else {
            self.lost = true;
            return;
        };
        for (chunk, part) in [a, b, c].into_iter().zip(parts) {
            let ok = u64::try_from(part)
                .is_ok_and(|part| part < (1 << 32) && bump(chunk, small(part), negative));
            if !ok {
                self.lost = true;
            }
        }
    }
}

/// Adds `d` to a chunk, or subtracts it when `negative`; false if that would overflow (which the
/// propagation allowance rules out).
fn bump(chunk: &mut i64, d: i64, negative: bool) -> bool {
    let next = if negative {
        chunk.checked_sub(d)
    } else {
        chunk.checked_add(d)
    };
    match next {
        Some(v) => {
            *chunk = v;
            true
        }
        None => false,
    }
}

/// Carry propagation (Neal, section 3): from the lowest chunk up, each chunk keeps its low 32 bits,
/// `c mod 2^32` in [0, 2^32), and passes `⌊c / 2^32⌋` (an arithmetic shift) to the next, so
/// `c = carry·2^32 + low` and the value is unchanged. Afterwards every chunk below the top is in
/// [0, 2^32) and the top one has the sign of the value: were it negative while the value is not,
/// the lower chunks, which sum to less than the top chunk's weight, could not make up for it; and
/// likewise the other way. `None` if a checked addition fails, which needs a chunk within 2^31 of
/// the `i64` limit: the allowances rule that out below the top, and at the top it needs a value of
/// 2^1100 (module documentation).
fn propagate(chunks: &mut [i64; CHUNKS]) -> Option<()> {
    let (top, body) = chunks.split_last_mut()?;
    let mut carry = 0_i64;
    for chunk in body {
        let v = chunk.checked_add(carry)?;
        carry = v >> 32;
        *chunk = v & LOW_MASK;
    }
    *top = top.checked_add(carry)?;
    Some(())
}

/// The exact value of the chunks rounded to the nearest double, ties to even; `None` only if a
/// checked step fails, which the bounds rule out.
///
/// 1. Propagate a copy. If the top chunk is negative the value is, and its magnitude is made by
///    negating every chunk (each is within `i64` after propagation, so negation cannot fail) and
///    propagating again; rounding to nearest, ties to even, is symmetric, so the sign goes on at
///    the end. Now every chunk is in [0, 2^32), the top one aside, which is nonnegative.
/// 2. A nonzero top chunk (index 66) means a magnitude of at least 2^(32·66 − 1075) = 2^1037: an
///    overflow, so ±∞. No nonzero chunk: +0.
/// 3. Otherwise let `highest` = i ≤ 65 be the highest nonzero chunk, and `window` =
///    `c_i·2^64 + c_(i−1)·2^32 + c_(i−2)` (absent chunks 0), so the magnitude is
///    `window·2^base + R` with `base = 32(i − 2) − 1075` and `0 ≤ R < 2^base`, and `sticky` is
///    whether R ≠ 0 (some chunk below i − 2 is nonzero). `c_i ≥ 1`, so the window's leading bit,
///    `leading`, is 64 to 95, and the magnitude's leading-bit `exponent` is `leading + base`.
/// 4. The result's unit in the last place is 2^`quantum`, `quantum = max(exponent − 52, −1074)`:
///    53 significant bits for a normal result, the subnormal grid below 2^−1022. The `shift`,
///    `quantum − base`, is 12 to 65: for a normal result it is `leading − 52 ≥ 12`; a subnormal
///    one has `exponent < −1022`, so `32(i − 2) < 53 − leading ≤ −11`, i ≤ 1, and the shift is
///    `65 − 32i`. So `kept = ⌊window / 2^shift⌋`, the `rest`, `window mod 2^shift`, and the
///    `half`, 2^(shift − 1), decide the rounding: up when `rest > half`, or `rest = half` and
///    (`sticky` or `kept` odd). (The lowest bit of chunk 0, worth 2^−1075, is always 0: every term
///    is a multiple of 2^−1074.)
/// 5. The bits are `(F − 1)·2^52 + kept` with `F = quantum + 1075` (1 for a subnormal result):
///    `kept` is in [2^52, 2^53] for a normal result, so one rounded up to 2^53 carries into the
///    exponent field, which is the right double, and gives +∞ when it carries to 2047; it is at
///    most 2^52 for a subnormal result, where 2^52 is the smallest normal. An `exponent` of 1024 or
///    more is an overflow before rounding: ±∞.
fn round_finite(chunks: &[i64; CHUNKS]) -> Option<f64> {
    let mut work = *chunks;
    propagate(&mut work)?;
    let negative = work.last().is_some_and(|&top| top < 0);
    if negative {
        for chunk in &mut work {
            *chunk = chunk.checked_neg()?;
        }
        propagate(&mut work)?;
    }
    let sign = if negative { 1_u64 << 63 } else { 0 };
    let (top, body) = work.split_last()?;
    if *top != 0 {
        return Some(f64::from_bits(sign | (SPECIAL_FIELD << 52)));
    }
    let Some(highest) = body.iter().rposition(|&chunk| chunk != 0) else {
        return Some(0.0);
    };
    let (window, below) = match body.get(..=highest)? {
        [rest @ .., third, second, first] => (window3(*first, *second, *third)?, rest),
        [second, first] => (window3(*first, *second, 0)?, &[][..]),
        [first] => (window3(*first, 0, 0)?, &[][..]),
        [] => return None,
    };
    let sticky = below.iter().any(|&chunk| chunk != 0);
    let base = i32::try_from(highest)
        .ok()?
        .checked_sub(2)?
        .checked_mul(32)?
        .checked_sub(1075)?;
    let leading = i32::try_from(127_u32.checked_sub(window.leading_zeros())?).ok()?;
    let exponent = leading.checked_add(base)?;
    if exponent >= 1024 {
        return Some(f64::from_bits(sign | (SPECIAL_FIELD << 52)));
    }
    let quantum = exponent.checked_sub(52)?.max(-1074);
    let shift = u32::try_from(quantum.checked_sub(base)?).ok()?;
    if !(12..=65).contains(&shift) {
        return None;
    }
    let kept = window >> shift;
    let rest = window & (1_u128 << shift).checked_sub(1)?;
    let half = 1_u128 << shift.checked_sub(1)?;
    let up = rest > half || (rest == half && (sticky || kept & 1 == 1));
    let kept = if up { kept.checked_add(1)? } else { kept };
    let field = u64::try_from(quantum.checked_add(1075)?).ok()?;
    let bits = (field.checked_sub(1)? << 52).checked_add(u64::try_from(kept).ok()?)?;
    Some(f64::from_bits(sign | bits))
}

/// `c0·2^64 + c1·2^32 + c2` for propagated chunks, each in [0, 2^32).
fn window3(c0: i64, c1: i64, c2: i64) -> Option<u128> {
    let part = |c: i64| u128::try_from(c).ok().filter(|&c| c < (1 << 32));
    Some((part(c0)? << 64) | (part(c1)? << 32) | part(c2)?)
}

/// Neal's large superaccumulator, for one [`ExactSum::add_slice`] or [`ExactSum::add_iter`] call.
struct Large {
    /// The chunk for each sign-and-exponent pattern: the sum of the terms' bits modulo 2^64.
    chunks: [u64; LARGE_CHUNKS],
    /// Additions left for each chunk before it must move to the small accumulator; 0 for a chunk
    /// not in use (or the patterns of NaN and the infinities, never in use).
    counts: [u16; LARGE_CHUNKS],
    /// One bit for each chunk in use.
    used: [u64; LARGE_CHUNKS / 64],
}

impl Large {
    /// An empty large accumulator.
    #[allow(
        clippy::large_stack_arrays,
        reason = "Neal's large superaccumulator is 4096 chunks and this no_std crate has no heap: \
                  41 KB for the length of one add_slice or add_iter call, inside the 1 MB stack of \
                  a WebAssembly module or of a native thread"
    )]
    fn new() -> Large {
        Large {
            chunks: [0; LARGE_CHUNKS],
            counts: [0; LARGE_CHUNKS],
            used: [0; LARGE_CHUNKS / 64],
        }
    }

    /// Adds a term (Neal's `xsum_large_addv`): one decrement and one modulo-2^64 addition while its
    /// chunk has additions left; otherwise the slow path.
    #[inline]
    fn add(&mut self, bits: u64, sum: &mut ExactSum) {
        let index = sign_and_exponent(bits);
        if let (Some(count), Some(chunk)) = (self.counts.get_mut(index), self.chunks.get_mut(index))
        {
            if let Some(left) = count.checked_sub(1) {
                *count = left;
                // The chunk is a sum modulo 2^64 by design (module documentation): wrapping is the
                // specification.
                *chunk = chunk.wrapping_add(bits);
                return;
            }
        }
        self.add_slow(index, bits, sum);
    }

    /// A term whose chunk has no additions left (Neal's `xsum_large_add_value_inf_nan`): a NaN or
    /// an infinity goes to the small accumulator's flags; a full chunk moves to the small
    /// accumulator and starts again; an unused one is put in use. Then the term is added.
    fn add_slow(&mut self, index: usize, bits: u64, sum: &mut ExactSum) {
        if u64::from(exponent_field(bits)) == SPECIAL_FIELD {
            sum.add_special(bits);
            return;
        }
        let (word, bit) = (index >> 6, 1_u64 << (index & 63));
        let Some(used) = self.used.get_mut(word) else {
            sum.lost = true;
            return;
        };
        if *used & bit == 0 {
            *used |= bit;
        } else {
            self.transfer(index, sum);
        }
        if let (Some(count), Some(chunk)) = (self.counts.get_mut(index), self.chunks.get_mut(index))
        {
            *count = LARGE_ADDS - 1;
            *chunk = bits;
        } else {
            sum.lost = true;
        }
    }

    /// Moves a chunk in use to the small accumulator and empties it (Neal's
    /// `xsum_add_lchunk_to_small`). With n = 4096 − count terms added, the chunk is
    /// `n·(index·2^52) + F` modulo 2^64, where F, the sum of their fraction fields, is below
    /// `n·2^52 ≤ 2^64`; so `F = chunk − n·index·2^52`, computed modulo 2^64 (the wrapping is the
    /// specification: the true F is in [0, 2^64), so the residue is F itself). Their significands
    /// sum to F plus `n·2^52` implicit bits when the exponent field is not 0.
    fn transfer(&mut self, index: usize, sum: &mut ExactSum) {
        let (Some(count), Some(chunk)) = (self.counts.get_mut(index), self.chunks.get_mut(index))
        else {
            sum.lost = true;
            return;
        };
        let Some(n) = LARGE_ADDS.checked_sub(*count) else {
            sum.lost = true;
            return;
        };
        let Ok(pattern) = u64::try_from(index) else {
            sum.lost = true;
            return;
        };
        let fractions = chunk.wrapping_sub((pattern << 52).wrapping_mul(u64::from(n)));
        let field = exponent_field(pattern << 52);
        let implicit = if field == 0 { 0 } else { u128::from(n) << 52 };
        let Some(significands) = u128::from(fractions).checked_add(implicit) else {
            sum.lost = true;
            return;
        };
        *chunk = 0;
        *count = 0;
        sum.add_significands(field, significands, pattern >> 11 != 0);
    }

    /// Moves every chunk in use to the small accumulator, by the bits of `used`.
    fn empty_into(&mut self, sum: &mut ExactSum) {
        let used = self.used;
        for (word, bits) in used.into_iter().enumerate() {
            let mut left = bits;
            while left != 0 {
                let position = left.trailing_zeros();
                left &= !(1_u64 << position);
                let Ok(position) = usize::try_from(position) else {
                    sum.lost = true;
                    return;
                };
                self.transfer((word << 6) | position, sum);
            }
        }
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
    fn small_sums_round_like_ieee() {
        let cases: [(f64, f64); 6] = [
            (1.0, 2.0f64.powi(-53)),
            (1.0, 2.0f64.powi(-53) + 2.0f64.powi(-105)),
            (-f64::MAX, -2.0f64.powi(970)),
            (f64::MAX, 2.0f64.powi(969)),
            (f64::from_bits(1), f64::from_bits(3)),
            (0.1, -0.1),
        ];
        for (a, b) in cases {
            let mut s = ExactSum::new();
            s.add(a);
            s.add(b);
            assert_eq!(s.round_ieee().to_bits(), (a + b).to_bits(), "{a:e} + {b:e}");
        }
    }

    #[test]
    fn large_path_matches_small_path() {
        let terms: Vec<f64> = (0..10_000).map(|i| f64::from(i) * 0.37 - 1000.0).collect();
        let mut by_one = ExactSum::new();
        for &x in &terms {
            by_one.add(x);
        }
        let mut by_slice = ExactSum::new();
        by_slice.add_slice(&terms);
        assert_eq!(
            by_one.round_ieee().to_bits(),
            by_slice.round_ieee().to_bits()
        );
    }
}

/// Bounded proofs (chapter 28.7; piece G41): every double, bit for bit.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// Adding any double never panics or overflows, and one finite double, rounded back, is that
    /// double (a zero comes back +0, the sum's one zero).
    #[kani::proof]
    fn one_term_rounds_back_to_itself() {
        let x = f64::from_bits(kani::any());
        let mut sum = ExactSum::new();
        sum.add(x);
        if x.is_finite() {
            let back = sum.round().unwrap().get();
            if x == 0.0 {
                assert!(back.to_bits() == 0);
            } else {
                assert!(back.to_bits() == x.to_bits());
            }
        } else {
            assert!(sum.round().is_err());
        }
    }

    /// A finite double and its negation cancel exactly, to +0.
    #[kani::proof]
    fn a_term_and_its_negation_cancel() {
        let x = f64::from_bits(kani::any());
        kani::assume(x.is_finite());
        let mut sum = ExactSum::new();
        sum.add(x);
        sum.add(-x);
        assert!(sum.round().unwrap().get().to_bits() == 0);
    }
}
