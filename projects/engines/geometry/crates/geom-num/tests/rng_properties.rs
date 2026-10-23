//! Property tests for `geom_num::rng` (chapter 28's standard bar): the block function and every
//! draw function, over generated seeds, stream ids, positions and bounds, against definitions
//! written here.
//!
//! - **The block function is a bijection on counters.** Philox's round is invertible: the low half
//!   of each product gives back its input word, because the multipliers are odd and so have inverses
//!   modulo 2^32, and the exclusive ors undo themselves. The inverse is written here from the
//!   round's definition, and inverting `philox4x32_10` gives back every generated counter. So for
//!   one seed, no two (stream, block) pairs share a block: the module's "streams never overlap".
//! - **A stream is its blocks in order, and skipping ahead is drawing.**
//! - **Every draw follows its written definition**: `next_u64`, `next_f64`, `below` (Lemire's rule
//!   with the threshold taken as a 128-bit remainder) and `range_inclusive`, with the same values
//!   and the same positions afterwards.
//! - **A failed draw changes nothing**, from every offset of a stream's last blocks.
//!
//! Property tests run from a fixed seed and write no failure files, so they are the same on every
//! run and every target (chapter 28.17: fixed seeds on every pull request).

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their oracles, models and generators.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "oracles, models and generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken definition must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_num::rng::{Philox, RngError, Seed, Stream, StreamId, philox4x32_10};
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

/// Philox-4x32's multipliers, for counter words 0 and 2 (the paper's section 4.3).
const MULTIPLIERS: [u32; 2] = [0xD251_1F53, 0xCD9E_8D57];

/// The Weyl steps of the key schedule.
const WEYL: [u32; 2] = [0x9E37_79B9, 0xBB67_AE85];

/// 2^64.
const TWO_64: u128 = 1 << 64;

/// 2^53 as a double.
const TWO_53: f64 = 9_007_199_254_740_992.0;

/// Property tests from a fixed seed, with no failure files written.
fn config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        rng_seed: RngSeed::Fixed(0x4733_3250_524f_5053),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// The inverse of an odd number modulo 2^32, by Newton's iteration x ← x · (2 − m · x), which
/// doubles the number of correct low bits each step: m is its own inverse modulo 8 (three bits),
/// so four steps give 48 ≥ 32 bits. Checked against its definition.
fn inverse_mod_2_32(m: u32) -> u32 {
    assert_eq!(m % 2, 1, "only odd numbers have inverses modulo 2^32");
    let mut x = m;
    for _ in 0..4 {
        x = x.wrapping_mul(2u32.wrapping_sub(m.wrapping_mul(x)));
    }
    assert_eq!(
        m.wrapping_mul(x),
        1,
        "{m:#x} times {x:#x} is not 1 modulo 2^32"
    );
    x
}

/// The high half of the exact product of two 32-bit words.
fn mulhi(a: u32, b: u32) -> u32 {
    u32::try_from((u64::from(a) * u64::from(b)) >> 32).unwrap()
}

/// The key round `round` (counted from 0) uses: the key plus `round` Weyl steps, modulo 2^32.
fn round_key(key: [u32; 2], round: u32) -> [u32; 2] {
    [
        key[0].wrapping_add(WEYL[0].wrapping_mul(round)),
        key[1].wrapping_add(WEYL[1].wrapping_mul(round)),
    ]
}

/// Undoes one round. The round maps `[c0, c1, c2, c3]` to
/// `[hi(M1·c2) ^ c1 ^ k0, lo(M1·c2), hi(M0·c0) ^ c3 ^ k1, lo(M0·c0)]`, so the low halves give back
/// `c2` and `c0` through the inverse multipliers, and with them the high halves, which the
/// exclusive ors then remove.
fn unround(out: [u32; 4], key: [u32; 2], inverses: [u32; 2]) -> [u32; 4] {
    let c0 = out[3].wrapping_mul(inverses[0]);
    let c2 = out[1].wrapping_mul(inverses[1]);
    let c1 = out[0] ^ mulhi(MULTIPLIERS[1], c2) ^ key[0];
    let c3 = out[2] ^ mulhi(MULTIPLIERS[0], c0) ^ key[1];
    [c0, c1, c2, c3]
}

/// The inverse of Philox4x32-10: the ten rounds undone in reverse order.
fn unphilox(block: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    let inverses = MULTIPLIERS.map(inverse_mod_2_32);
    let mut x = block;
    for round in (0..10).rev() {
        x = unround(x, round_key(key, round), inverses);
    }
    x
}

/// Seeds and stream ids: small, around the 32-bit boundary, the ends, and anything.
fn word64() -> impl Strategy<Value = u64> {
    prop_oneof![
        0u64..4,
        0xffff_fffeu64..=0x1_0000_0001,
        (u64::MAX - 3)..=u64::MAX,
        any::<u64>(),
    ]
}

/// Bounds for `below`, many of which reject often (just above a power of two).
fn bound() -> impl Strategy<Value = u64> {
    prop_oneof![
        1u64..=17,
        (0u32..64).prop_map(|k| 1u64 << k),
        (1u32..64).prop_map(|k| (1u64 << k) + 1),
        (1u64 << 63)..=u64::MAX,
        any::<u64>().prop_map(|n| n.max(1)),
    ]
}

/// The stream of `philox` advanced `words` words by drawing them one at a time.
fn drawn(philox: Philox, words: u32) -> Stream {
    let mut s = philox.stream();
    for _ in 0..words {
        s.next_u32().unwrap();
    }
    s
}

/// The inverse is right on the published inputs: it takes each known answer back to its counter.
#[test]
fn the_inverse_undoes_the_known_answers() {
    let kats = [
        (
            [0u32; 4],
            [0u32; 2],
            [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8],
        ),
        (
            [u32::MAX; 4],
            [u32::MAX; 2],
            [0x408f_276d, 0x41c8_3b0e, 0xa20b_c7c6, 0x6d54_51fd],
        ),
        (
            [0x243f_6a88, 0x85a3_08d3, 0x1319_8a2e, 0x0370_7344],
            [0xa409_3822, 0x299f_31d0],
            [0xd16c_fe09, 0x94fd_cceb, 0x5001_e420, 0x2412_6ea1],
        ),
    ];
    for (counter, key, block) in kats {
        assert_eq!(unphilox(block, key), counter, "key {key:08x?}");
    }
}

proptest! {
    #![proptest_config(config(2048))]

    /// Inverting the block function gives back the counter, so it is a bijection on counters.
    #[test]
    fn philox4x32_10_is_a_bijection_on_counters(
        counter in any::<[u32; 4]>(),
        key in any::<[u32; 2]>(),
    ) {
        prop_assert_eq!(unphilox(philox4x32_10(counter, key), key), counter);
    }

    /// Through the layout: a block of a stream gives back its block index and stream id.
    #[test]
    fn a_block_gives_back_its_stream_and_index(
        seed in word64(),
        stream in word64(),
        block in word64(),
    ) {
        let philox = Philox::new(Seed(seed), StreamId(stream));
        let counter = unphilox(philox.block(block), philox.key());
        let join = |low: u32, high: u32| (u64::from(high) << 32) | u64::from(low);
        prop_assert_eq!(join(counter[0], counter[1]), block);
        prop_assert_eq!(join(counter[2], counter[3]), stream);
    }
}

proptest! {
    #![proptest_config(config(512))]

    /// The first eight words from block `start` are blocks `start` and `start + 1`, in order.
    #[test]
    fn a_stream_is_its_blocks_in_order(
        seed in word64(),
        stream in word64(),
        start in word64().prop_map(|b| b.min(u64::MAX - 1)),
    ) {
        let philox = Philox::new(Seed(seed), StreamId(stream));
        let mut s = philox.stream_from(start);
        let words: Vec<u32> = (0..8).map(|_| s.next_u32().unwrap()).collect();
        let mut expected = philox.block(start).to_vec();
        expected.extend(philox.block(start + 1));
        prop_assert_eq!(words, expected);
        prop_assert_eq!(s.position(), (u128::from(start) + 2) * 4);
    }

    /// Skipping to a block and drawing on is the same as drawing every word from the start: the
    /// same words and an equal stream.
    #[test]
    fn skipping_ahead_is_drawing(
        seed in word64(),
        stream in word64(),
        block in 0u32..48,
        extra in 0u32..12,
    ) {
        let philox = Philox::new(Seed(seed), StreamId(stream));
        let mut walked = drawn(philox, block * 4);
        let mut skipped = philox.stream_from(u64::from(block));
        prop_assert_eq!(&walked, &skipped);
        for _ in 0..extra {
            prop_assert_eq!(walked.next_u32(), skipped.next_u32());
        }
        prop_assert_eq!(&walked, &skipped);
    }

    /// `next_u64` is the next word plus the one after it times 2^32, and `next_f64` is its top 53
    /// bits divided by 2^53, in [0, 1), from any offset within a block.
    #[test]
    fn next_u64_and_next_f64_follow_their_definitions(
        seed in word64(),
        stream in word64(),
        start in word64().prop_map(|b| b.min(u64::MAX - 2)),
        offset in 0u32..4,
    ) {
        let philox = Philox::new(Seed(seed), StreamId(stream));
        let mut s = philox.stream_from(start);
        for _ in 0..offset {
            s.next_u32().unwrap();
        }
        let mut words = s.clone();
        let low = u64::from(words.next_u32().unwrap());
        let high = u64::from(words.next_u32().unwrap());
        let mut as_u64 = s.clone();
        prop_assert_eq!(as_u64.next_u64(), Ok(low + (high << 32)));
        prop_assert_eq!(&as_u64, &words);
        let x = s.next_f64().unwrap().get();
        prop_assert_eq!(x.to_bits(), (((low + (high << 32)) >> 11) as f64 / TWO_53).to_bits());
        prop_assert!((0.0..1.0).contains(&x) && x.to_bits() >> 63 == 0, "next_f64 gave {}", x);
        prop_assert_eq!(&s, &words);
    }

    /// `below` is Lemire's rule by its definition: draw 64-bit words until the low half of
    /// `x · n` is at least 2^64 mod n (a 128-bit remainder here), and return the high half. The
    /// value, the words used and the stream afterwards all match, through runs of draws on one
    /// stream so that rejections land anywhere in a block.
    #[test]
    fn below_follows_lemire_by_definition(
        seed in word64(),
        stream in word64(),
        start in 0u64..1_000_000,
        bounds in proptest::collection::vec(bound(), 1..32),
    ) {
        let philox = Philox::new(Seed(seed), StreamId(stream));
        let mut s = philox.stream_from(start);
        let mut reference = s.clone();
        for n in bounds {
            let wide = u128::from(n);
            let threshold = TWO_64 % wide;
            let expected = loop {
                let product = u128::from(reference.next_u64().unwrap()) * wide;
                if product % TWO_64 >= threshold {
                    break u64::try_from(product / TWO_64).unwrap();
                }
            };
            let got = s.below(n);
            prop_assert_eq!(got, Ok(expected), "below({})", n);
            prop_assert!(expected < n);
            prop_assert_eq!(&s, &reference);
        }
    }

    /// `range_inclusive(low, high)` is `low + below(high − low + 1)`, or `next_u64` for all of
    /// `u64`, and an empty range is an error that draws nothing.
    #[test]
    fn range_inclusive_is_low_plus_below(
        seed in word64(),
        stream in word64(),
        low in word64(),
        high in word64(),
    ) {
        let mut s = Philox::new(Seed(seed), StreamId(stream)).stream();
        let mut reference = s.clone();
        let got = s.range_inclusive(low, high);
        if low > high {
            prop_assert_eq!(got, Err(RngError::LowAboveHigh { low, high }));
        } else if low == 0 && high == u64::MAX {
            prop_assert_eq!(got, reference.next_u64());
        } else {
            let v = reference.below(high - low + 1).unwrap();
            prop_assert_eq!(got, Ok(low + v));
            prop_assert!((low..=high).contains(&(low + v)));
        }
        prop_assert_eq!(&s, &reference);
    }

    /// Near the end of a stream, every draw either succeeds as its definition says or fails with
    /// `Exhausted` and leaves the stream exactly as it was.
    #[test]
    fn a_failed_draw_changes_nothing(
        seed in word64(),
        stream in word64(),
        last_blocks in 0u64..3,
        offset in 0u32..4,
        draws in proptest::collection::vec((0u8..5, bound()), 1..16),
    ) {
        let philox = Philox::new(Seed(seed), StreamId(stream));
        let mut s = philox.stream_from(u64::MAX - last_blocks);
        for _ in 0..offset {
            s.next_u32().unwrap();
        }
        let exhausted = Err(RngError::Exhausted { stream: StreamId(stream) });
        for (kind, n) in draws {
            let before = s.clone();
            let failed = match kind {
                0 => s.next_u32().map(u64::from) == exhausted,
                1 => s.next_u64() == exhausted,
                2 => s.next_f64().map(|x| x.get().to_bits()) == exhausted,
                3 => s.below(n) == exhausted,
                _ => s.range_inclusive(n / 2, n) == exhausted,
            };
            if failed {
                prop_assert_eq!(&s, &before, "a failed draw of kind {} changed the stream", kind);
            } else {
                prop_assert!(s.position() > before.position());
            }
        }
    }
}
