//! `SplitMix64`, the deterministic generator behind every seeded test input, and [`Entropy`], the
//! source of words that test-data builders draw from.
//!
//! What it does: [`SplitMix64`] turns a [`Seed`] into a stream of 64-bit words. [`ByteSource`]
//! turns a fuzzer's bytes into the same kind of stream. Both implement [`Entropy`], so one builder
//! (for example [`crate::grid::GridSampler`]) makes test data from a seed in a property test and
//! from coverage-guided bytes in a fuzz target.
//!
//! Guarantees:
//!
//! - **The same words on every target.** `SplitMix64` is wrapping 64-bit integer arithmetic only,
//!   so a seed gives the same stream on every target, in every build profile.
//! - **Every word, once per period.** The state is a counter advanced by an odd constant, so it
//!   visits all 2^64 values once per period, and the output function is a bijection of the state, so
//!   over one period every 64-bit word is returned exactly once.
//! - **Unbiased bounded draws.** [`SplitMix64::below`] returns each value in `0..n` with exactly
//!   equal probability over the generator's period.
//! - **Test only, and independent of the kernel.** The kernel has no randomness (28.17); this is not
//!   the counter-based generator chapter 27 plans for `geom-num`, so a change there never changes a
//!   test's inputs. It is not cryptographic and must never seed anything that ships.
//! - **[`ByteSource`] never fails.** Past the end of its bytes it yields zeros, so every input,
//!   including the empty one, decodes to some test case.
//!
//! Method: `SplitMix64` of Steele, Lea and Flood, "Fast Splittable Pseudorandom Number Generators",
//! OOPSLA 2014 (<https://doi.org/10.1145/2660193.2660195>), with the constants of Vigna's reference
//! implementation `splitmix.c` (<https://prng.di.unimi.it/splitmix.c>, public domain): the
//! golden-ratio increment `0x9e3779b97f4a7c15` and Stafford's "Mix13" output function
//! (<https://zimbry.blogspot.com/2011/09/better-bit-mixing-improving-on.html>). The unit tests check
//! it against 50 outputs of that reference implementation. Bounded draws use Lemire's
//! multiply-and-reject method, "Fast Random Integer Generation in an Interval", ACM Transactions on
//! Modeling and Computer Simulation 29(1), 2019 (<https://doi.org/10.1145/3230636>).
//!
//! Cost: a handful of integer operations per word. [`SplitMix64::below`] draws one word, and draws
//! again with probability below `n / 2^64`, so fewer than two words on average.
//!
//! Consulted: S. Vigna, `splitmix.c`, public domain, <https://prng.di.unimi.it/splitmix.c>, for the
//!   generator's constants and the 50 reference outputs the unit tests check. No code copied.

use core::num::NonZeroU64;

use crate::seed::Seed;

/// The increment of `SplitMix64`'s state: 2^64 divided by the golden ratio, rounded to odd.
const GOLDEN_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

/// The first multiplier of Stafford's Mix13 output function.
const MIX13_A: u64 = 0xbf58_476d_1ce4_e5b9;

/// The second multiplier of Stafford's Mix13 output function.
const MIX13_B: u64 = 0x94d0_49bb_1331_11eb;

/// A source of 64-bit words for test-data builders: a seeded [`SplitMix64`] or a fuzzer's
/// [`ByteSource`].
pub trait Entropy {
    /// The next 64-bit word.
    fn word(&mut self) -> u64;

    /// The next byte. By default the low eight bits of the next word; [`ByteSource`] reads one byte.
    fn byte(&mut self) -> u8 {
        let [low, ..] = self.word().to_le_bytes();
        low
    }
}

/// The `SplitMix64` generator: a 64-bit state and its stream of words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// A generator whose state starts at `seed`, as `splitmix64.c` does with `x = seed`.
    #[must_use]
    pub const fn new(seed: Seed) -> SplitMix64 {
        SplitMix64 { state: seed.0 }
    }

    /// The next word: advance the state by the golden-ratio increment, then mix it.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(MIX13_A);
        z = (z ^ (z >> 27)).wrapping_mul(MIX13_B);
        z ^ (z >> 31)
    }

    /// A value in `0..n`, each with exactly equal probability, by Lemire's method.
    ///
    /// The loop ends: each pass rejects with probability below `n / 2^64`, and because the generator
    /// returns every 64-bit word once per period, an accepted word always comes.
    pub fn below(&mut self, n: NonZeroU64) -> u64 {
        let bound = n.get();
        let mut m = product(self.next_u64(), bound);
        let mut low = low_word(m);
        if low < bound {
            // 2^64 mod n, the number of words whose products fall in the short, biased stripe:
            // `wrapping_neg` gives 2^64 − n, which has the same remainder, and a remainder by a
            // `NonZeroU64` cannot fail.
            let threshold = bound.wrapping_neg() % n;
            while low < threshold {
                m = product(self.next_u64(), bound);
                low = low_word(m);
            }
        }
        high_word(m)
    }

    /// A value in `lo..=hi`, each with exactly equal probability, or `None` if `lo > hi`.
    pub fn range_i64(&mut self, lo: i64, hi: i64) -> Option<i64> {
        if lo > hi {
            return None;
        }
        // hi − lo + 1 is at least 1 and at most 2^64, exactly 2^64 only for the whole range of
        // i64, and lo + offset lies in lo..=hi: every value fits i128 (and the last fits i64), so
        // no checked step below returns `None` once lo ≤ hi.
        let span = i128::from(hi).checked_sub(i128::from(lo))?.checked_add(1)?;
        let offset = match u64::try_from(span).ok().and_then(NonZeroU64::new) {
            Some(n) => i128::from(self.below(n)),
            None => i128::from(self.next_u64()),
        };
        i64::try_from(i128::from(lo).checked_add(offset)?).ok()
    }

    /// A fair coin: the top bit of the next word.
    pub fn coin(&mut self) -> bool {
        self.next_u64() >> 63 == 1
    }

    /// Fills `out` with the next words' bytes, little-endian, eight to a word.
    pub fn fill_bytes(&mut self, out: &mut [u8]) {
        for chunk in out.chunks_mut(8) {
            let word = self.next_u64().to_le_bytes();
            for (d, s) in chunk.iter_mut().zip(word) {
                *d = s;
            }
        }
    }
}

impl Entropy for SplitMix64 {
    fn word(&mut self) -> u64 {
        self.next_u64()
    }
}

/// A fuzzer's bytes read as a stream of words and bytes, zeros past the end.
#[derive(Clone, Debug)]
pub struct ByteSource<'a> {
    rest: &'a [u8],
}

impl<'a> ByteSource<'a> {
    /// A source that reads `bytes` from the start.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> ByteSource<'a> {
        ByteSource { rest: bytes }
    }

    /// How many bytes are left before the zeros begin.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.rest.len()
    }
}

impl Entropy for ByteSource<'_> {
    /// The next eight bytes, little-endian; missing bytes read as zero.
    fn word(&mut self) -> u64 {
        let mut word = [0u8; 8];
        let take = self.rest.len().min(8);
        let (head, tail) = self.rest.split_at(take);
        for (d, s) in word.iter_mut().zip(head) {
            *d = *s;
        }
        self.rest = tail;
        u64::from_le_bytes(word)
    }

    /// The next byte, or zero past the end.
    fn byte(&mut self) -> u8 {
        match self.rest.split_first() {
            Some((first, tail)) => {
                self.rest = tail;
                *first
            }
            None => 0,
        }
    }
}

/// The exact 128-bit product of two 64-bit words. (2^64 − 1)² = 2^128 − 2^65 + 1 is below 2^128,
/// so every product fits and `saturating_mul` never saturates: it is the exact product, written
/// without a native operator.
fn product(a: u64, b: u64) -> u128 {
    u128::from(a).saturating_mul(u128::from(b))
}

/// The low 64 bits of a 128-bit product: its first eight little-endian bytes.
const fn low_word(m: u128) -> u64 {
    let [b0, b1, b2, b3, b4, b5, b6, b7, ..] = m.to_le_bytes();
    u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7])
}

/// The high 64 bits of a 128-bit product: its last eight little-endian bytes.
const fn high_word(m: u128) -> u64 {
    let [.., b8, b9, b10, b11, b12, b13, b14, b15] = m.to_le_bytes();
    u64::from_le_bytes([b8, b9, b10, b11, b12, b13, b14, b15])
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
    use crate::seed::fixed;

    /// The first 50 outputs of the reference `splitmix64.c` with `x = 1477776061723855037`, as
    /// published in the tests of `rand_xoshiro` 0.7.0 (`src/splitmix64.rs`, MIT OR Apache-2.0),
    /// which produced them with the reference implementation.
    const REFERENCE_SEED: u64 = 1_477_776_061_723_855_037;
    #[allow(
        clippy::unreadable_literal,
        reason = "digit for digit as published, so each can be compared with the source by eye"
    )]
    const REFERENCE: [u64; 50] = [
        1985237415132408290,
        2979275885539914483,
        13511426838097143398,
        8488337342461049707,
        15141737807933549159,
        17093170987380407015,
        16389528042912955399,
        13177319091862933652,
        10841969400225389492,
        17094824097954834098,
        3336622647361835228,
        9678412372263018368,
        11111587619974030187,
        7882215801036322410,
        5709234165213761869,
        7799681907651786826,
        4616320717312661886,
        4251077652075509767,
        7836757050122171900,
        5054003328188417616,
        12919285918354108358,
        16477564761813870717,
        5124667218451240549,
        18099554314556827626,
        7603784838804469118,
        6358551455431362471,
        3037176434532249502,
        3217550417701719149,
        9958699920490216947,
        5965803675992506258,
        12000828378049868312,
        12720568162811471118,
        245696019213873792,
        8351371993958923852,
        14378754021282935786,
        5655432093647472106,
        5508031680350692005,
        8515198786865082103,
        6287793597487164412,
        14963046237722101617,
        3630795823534910476,
        8422285279403485710,
        10554287778700714153,
        10871906555720704584,
        8659066966120258468,
        9420238805069527062,
        10338115333623340156,
        13514802760105037173,
        14635952304031724449,
        15419692541594102413,
    ];

    #[test]
    fn matches_the_reference_implementation() {
        let mut rng = SplitMix64::new(fixed("rng.rs unit test", Seed(REFERENCE_SEED)));
        for (i, want) in REFERENCE.iter().enumerate() {
            assert_eq!(rng.next_u64(), *want, "output {i}");
        }
    }

    #[test]
    fn below_is_in_range_and_reaches_every_value() {
        let mut rng = SplitMix64::new(fixed("rng.rs unit test", Seed(7)));
        for n in [1u64, 2, 3, 5, 7, 10, 64, 1000] {
            let nz = NonZeroU64::new(n).unwrap();
            let mut seen = vec![false; n as usize];
            for _ in 0..20 * n {
                let v = rng.below(nz);
                assert!(v < n);
                seen[v as usize] = true;
            }
            assert!(seen.iter().all(|&s| s), "n = {n}: {seen:?}");
        }
        let top = NonZeroU64::new(u64::MAX).unwrap();
        for _ in 0..1000 {
            assert!(rng.below(top) < u64::MAX);
        }
    }

    /// Lemire's method is exactly the high word of the product when the low word clears the
    /// threshold. Checked against a direct computation for a stream where no draw is rejected.
    #[allow(
        clippy::many_single_char_names,
        reason = "n, w and m are the bound, the word and the product, as in Lemire's paper; \
                  a and b the two streams"
    )]
    #[test]
    fn below_is_the_high_word_of_the_product() {
        let n = 1_000_003u64;
        let mut a = SplitMix64::new(fixed("rng.rs unit test", Seed(99)));
        let mut b = a.clone();
        let threshold = n.wrapping_neg() % n;
        for _ in 0..10_000 {
            let w = b.next_u64();
            let m = u128::from(w) * u128::from(n);
            if (m as u64) < threshold {
                // A rejection: resynchronise the two streams and carry on.
                a = b.clone();
                continue;
            }
            assert_eq!(a.below(NonZeroU64::new(n).unwrap()), (m >> 64) as u64);
        }
    }

    /// Exact uniformity over a full cycle of a tiny analogue: with a 4-bit word and n = 3, the
    /// multiply-and-reject method accepts the same number of words for every value. This checks the
    /// threshold formula 2^w mod n on every word, which a 64-bit test cannot.
    #[test]
    fn lemire_threshold_gives_equal_counts() {
        for w_bits in [4u32, 5, 6, 8] {
            let size = 1u32 << w_bits;
            for n in 1..size {
                let threshold = (size - n) % n; // 2^w mod n
                let mut counts = vec![0u32; n as usize];
                for x in 0..size {
                    let m = x * n;
                    if m % size >= threshold {
                        counts[(m / size) as usize] += 1;
                    }
                }
                assert!(counts.iter().all(|&c| c == counts[0]), "w {w_bits}, n {n}");
            }
        }
    }

    #[test]
    fn range_i64_covers_its_ends_and_refuses_empty_ranges() {
        let mut rng = SplitMix64::new(fixed("rng.rs unit test", Seed(3)));
        let (mut lo_seen, mut hi_seen) = (false, false);
        for _ in 0..1000 {
            let v = rng.range_i64(-2, 2).unwrap();
            assert!((-2..=2).contains(&v));
            lo_seen |= v == -2;
            hi_seen |= v == 2;
        }
        assert!(lo_seen && hi_seen);
        assert_eq!(rng.range_i64(5, 5), Some(5));
        assert_eq!(rng.range_i64(1, 0), None);
        assert!(rng.range_i64(i64::MIN, i64::MAX).is_some());
        assert!(rng.range_i64(i64::MIN, i64::MIN + 1).unwrap() <= i64::MIN + 1);
        assert!(rng.range_i64(i64::MAX - 1, i64::MAX).unwrap() >= i64::MAX - 1);
    }

    #[test]
    fn fill_bytes_is_the_words_little_endian() {
        let mut a = SplitMix64::new(fixed("rng.rs unit test", Seed(11)));
        let mut b = a.clone();
        let mut out = [0u8; 21];
        a.fill_bytes(&mut out);
        let mut want = Vec::new();
        for _ in 0..3 {
            want.extend_from_slice(&b.next_u64().to_le_bytes());
        }
        assert_eq!(&out[..], &want[..21]);
    }

    #[test]
    fn coin_and_byte_come_from_the_next_word() {
        let mut a = SplitMix64::new(fixed("rng.rs unit test", Seed(5)));
        let mut b = a.clone();
        assert_eq!(a.coin(), b.next_u64() >> 63 == 1);
        assert_eq!(a.byte(), b.next_u64() as u8);
    }

    #[test]
    fn byte_source_reads_in_order_then_zeros() {
        let data = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let mut s = ByteSource::new(&data);
        assert_eq!(s.byte(), 1);
        assert_eq!(s.word(), u64::from_le_bytes([2, 3, 4, 5, 6, 7, 8, 9]));
        assert_eq!(s.remaining(), 1);
        assert_eq!(s.word(), 10);
        assert_eq!(s.remaining(), 0);
        assert_eq!(s.word(), 0);
        assert_eq!(s.byte(), 0);
        let mut empty = ByteSource::new(&[]);
        assert_eq!(empty.word(), 0);
        assert_eq!(empty.byte(), 0);
    }
}
