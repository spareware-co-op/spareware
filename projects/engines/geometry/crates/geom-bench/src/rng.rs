//! A fixed-seed generator for benchmark inputs: the same numbers on every target, every run.
//!
//! Benchmark inputs must be identical wherever and whenever they are built, or two runs would
//! measure different work. So nothing here is random: [`SplitMix64`] is a pure function of its
//! seed, and every input set starts from a seed derived from its own name by [`seed_for`].
//!
//! Method: SplitMix64, from G. L. Steele, D. Lea and C. H. Flood, "Fast splittable pseudorandom
//! number generators", OOPSLA 2014 (<https://doi.org/10.1145/2660193.2660195>), with the constants
//! of Vigna's reference code (<https://prng.di.unimi.it/splitmix64.c>, public domain). Seeds are
//! derived with 64-bit FNV-1a (<http://www.isthe.com/chongo/tech/comp/fnv/>). The tests check
//! both against their published values.
//! Consulted: Vigna's SplitMix64 reference code, public domain,
//!   <https://prng.di.unimi.it/splitmix64.c>, for the constants and steps of SplitMix64, which
//!   the tests check against its published outputs. No code copied.
//!
//! Guarantees: every draw is a pure function of the seed and the number of draws before it, in
//! wrapping 64-bit integer arithmetic only, so the sequence is the same on every target, in every
//! build profile and with overflow checks on or off; [`SplitMix64::range`] stays inside its range
//! for every pair of `i64` bounds, and no method panics.
//!
//! Cost: a few wrapping integer operations per draw, and one per byte of a seed's name.

/// FNV-1a's 64-bit offset basis.
pub const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;

/// FNV-1a's 64-bit prime.
pub const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

/// The 64-bit FNV-1a hash of `bytes`.
#[must_use]
pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV_OFFSET, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(FNV_PRIME)
    })
}

/// The seed for the input set of operation `op` in class `class`: a hash of both names, so every
/// set has its own sequence and adding a set never changes another.
#[must_use]
pub fn seed_for(op: &str, class: &str) -> u64 {
    fnv1a(op.as_bytes()) ^ fnv1a(class.as_bytes()).rotate_left(32)
}

/// SplitMix64: a 64-bit state advanced by a fixed odd constant and mixed into each output.
#[derive(Clone, Debug)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// A generator starting from `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> SplitMix64 {
        SplitMix64 { state: seed }
    }

    /// The next 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n`, or 0 when `n` is 0. Reduced by remainder: the slight bias toward small
    /// values does not matter for benchmark inputs, and the result is the same everywhere.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }

    /// A value in `lo..=hi`; `lo` when `hi < lo`. Exact for every pair of `i64` values.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        // hi − lo + 1 fits in u64 except for the full range, where `span` wraps to 0 and any 64-bit
        // value is in range.
        let span = hi.abs_diff(lo).wrapping_add(1);
        let offset = if span == 0 {
            self.next_u64()
        } else {
            self.below(span)
        };
        lo.wrapping_add_unsigned(offset)
    }

    /// +1 or −1, each half the time.
    pub fn sign(&mut self) -> i64 {
        if self.next_u64() >> 63 == 0 { 1 } else { -1 }
    }

    /// A value in `0..n` as an index, or 0 when `n` is 0.
    pub fn index(&mut self, n: usize) -> usize {
        // usize is at most 64 bits on every supported target, so both conversions are exact.
        let n = u64::try_from(n).unwrap_or(u64::MAX);
        usize::try_from(self.below(n)).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_reference_sequence() {
        // The first outputs of Vigna's splitmix64.c from seed 1234567.
        let mut g = SplitMix64::new(1_234_567);
        let expected = [
            6_457_827_717_110_365_317u64,
            3_203_168_211_198_807_973,
            9_817_491_932_198_370_423,
            4_593_380_528_125_082_431,
            16_408_922_859_458_223_821,
        ];
        for e in expected {
            assert_eq!(g.next_u64(), e);
        }
    }

    #[test]
    fn range_stays_inside() {
        let mut g = SplitMix64::new(7);
        for _ in 0..10_000 {
            let v = g.range(-5, 5);
            assert!((-5..=5).contains(&v));
        }
        assert_eq!(g.range(3, 3), 3);
        assert_eq!(g.range(3, 2), 3);
        let _ = g.range(i64::MIN, i64::MAX);
    }

    #[test]
    fn fnv1a_matches_published_vectors() {
        // From the FNV test suite: "" and "a".
        assert_eq!(fnv1a(b""), 0xCBF2_9CE4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xAF63_DC4C_8601_EC8C);
    }
}
