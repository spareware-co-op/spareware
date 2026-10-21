//! Streams of `geom_num::rng`: the counter layout and its carries across 32-bit word boundaries,
//! the end of a stream, failed draws that change nothing, independent streams, and every draw
//! function against a plain reference implementation of its documented rule.
//!
//! The known answers of the block function itself are in `rng_kat.rs`; the golden hash over many
//! streams, checked on every target, is in `rng_determinism.rs`.

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
// clippy.toml says tests may use these, and a broken corpus must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;

use geom_num::rng::{Philox, RngError, Seed, Stream, StreamId, WORDS_PER_STREAM, philox4x32_10};

/// "SPAREKIT" in ASCII.
const SEED: Seed = Seed(0x5350_4152_454b_4954);

/// A stream id whose two halves differ, so a mix-up of counter words shows.
const STREAM: StreamId = StreamId(0x0123_4567_89ab_cdef);

/// 2^53 as a double.
const TWO_POW_53: f64 = 9_007_199_254_740_992.0;

/// The low and high 32 bits of `v`.
fn halves(v: u64) -> [u32; 2] {
    [(v & 0xffff_ffff) as u32, (v >> 32) as u32]
}

/// Lemire's rule written plainly, with 128-bit arithmetic and the threshold 2^64 mod n computed
/// directly: draw 64-bit words until the low half of `x · n` is at least 2^64 mod n, then return the
/// high half.
fn reference_below(stream: &mut Stream, bound: u64) -> u64 {
    let n = u128::from(bound);
    let threshold = (1u128 << 64) % n;
    loop {
        let product = u128::from(stream.next_u64().unwrap()) * n;
        if product % (1u128 << 64) >= threshold {
            return u64::try_from(product >> 64).unwrap();
        }
    }
}

/// Whether Lemire's rule rejects the word `x` for the bound `n`.
fn rejects(x: u64, n: u64) -> bool {
    let n = u128::from(n);
    (u128::from(x) * n) % (1u128 << 64) < (1u128 << 64) % n
}

#[test]
fn a_stream_is_its_blocks_in_order() {
    let philox = Philox::new(SEED, STREAM);
    let mut s = philox.stream();
    for block in 0..64 {
        for word in philox.block(block) {
            assert_eq!(s.next_u32(), Ok(word), "block {block}");
        }
    }
    assert_eq!(s.position(), 256);
    assert_eq!(s.remaining(), WORDS_PER_STREAM - 256);
}

#[test]
fn stream_from_is_the_same_as_drawing_up_to_there() {
    let philox = Philox::new(SEED, STREAM);
    let mut drawn = philox.stream();
    for block in 0..6 {
        assert_eq!(drawn, philox.stream_from(block), "block {block}");
        for _ in 0..4 {
            drawn.next_u32().unwrap();
        }
    }
}

/// The block index fills counter words 0 and 1; moving to the next block carries from word 0 into
/// word 1 at every 32-bit boundary, and the stream's words are untouched.
#[test]
fn the_block_index_carries_from_word_0_into_word_1() {
    let philox = Philox::new(SEED, STREAM);
    let [s0, s1] = halves(STREAM.0);
    let key = philox.key();
    assert_eq!(key, halves(SEED.0));
    for (before, after) in [
        (0xffff_ffff_u64, [0_u32, 1_u32]),
        (0x1_ffff_ffff, [0, 2]),
        (0x7fff_ffff_ffff_ffff, [0, 0x8000_0000]),
        (0xffff_fffe_ffff_ffff, [0, 0xffff_ffff]),
    ] {
        let [b0, b1] = halves(before);
        assert_eq!(philox.counter(before), [b0, b1, s0, s1]);
        assert_eq!(philox.counter(before + 1), [after[0], after[1], s0, s1]);

        let mut s = philox.stream_from(before);
        let words: Vec<u32> = (0..8).map(|_| s.next_u32().unwrap()).collect();
        assert_eq!(words[..4], philox4x32_10([b0, b1, s0, s1], key));
        assert_eq!(words[4..], philox4x32_10([after[0], after[1], s0, s1], key));
    }
}

/// The last block of a stream is block 2^64 − 1. After it the stream is exhausted: a 128-bit
/// counter increment would carry into the stream's words and repeat the next stream's first block,
/// and that never happens.
#[test]
fn the_last_block_never_carries_into_the_stream_id() {
    let philox = Philox::new(SEED, STREAM);
    let [s0, s1] = halves(STREAM.0);
    let mut s = philox.stream_from(u64::MAX);
    assert_eq!(s.remaining(), 4);
    let words: Vec<u32> = (0..4).map(|_| s.next_u32().unwrap()).collect();
    assert_eq!(
        words,
        philox4x32_10([u32::MAX, u32::MAX, s0, s1], philox.key())
    );
    assert_eq!(s.position(), WORDS_PER_STREAM);
    assert_eq!(s.remaining(), 0);
    let exhausted = Err(RngError::Exhausted { stream: STREAM });
    for _ in 0..3 {
        assert_eq!(s.next_u32(), exhausted);
        assert_eq!(s.position(), WORDS_PER_STREAM);
    }
    // Where a plain 128-bit increment would have gone: the next stream's block 0.
    let next = Philox::new(SEED, StreamId(STREAM.0 + 1));
    assert_eq!(next.counter(0), [0, 0, s0 + 1, s1]);
}

/// Every draw that fails leaves its stream exactly as it was, so the last word is still there.
#[test]
fn a_failed_draw_changes_nothing() {
    let philox = Philox::new(SEED, STREAM);
    let mut s = philox.stream_from(u64::MAX);
    for _ in 0..3 {
        s.next_u32().unwrap();
    }
    let before = s.clone();
    let exhausted = RngError::Exhausted { stream: STREAM };
    assert_eq!(s.next_u64(), Err(exhausted));
    assert_eq!(s, before);
    assert_eq!(s.next_f64(), Err(exhausted));
    assert_eq!(s, before);
    assert_eq!(s.below(10), Err(exhausted));
    assert_eq!(s, before);
    assert_eq!(s.range_inclusive(1, 10), Err(exhausted));
    assert_eq!(s, before);
    assert_eq!(s.range_inclusive(0, u64::MAX), Err(exhausted));
    assert_eq!(s, before);
    assert_eq!(s.below(0), Err(RngError::ZeroBound));
    assert_eq!(s, before);
    assert_eq!(
        s.range_inclusive(9, 8),
        Err(RngError::LowAboveHigh { low: 9, high: 8 })
    );
    assert_eq!(s, before);
    assert_eq!(s.next_u32(), Ok(philox.block(u64::MAX)[3]));
    assert_eq!(s.next_u32(), Err(exhausted));
}

/// `below` can reject words and then run out. It still changes nothing: the words its rejected
/// passes drew are given back.
#[test]
fn a_below_that_rejects_and_then_runs_out_changes_nothing() {
    // For this bound 2^64 mod n = 2^63 − 1, so about half of all words are rejected.
    let bound = (1u64 << 63) + 1;
    let mut found = 0;
    for seed in 0..64 {
        let philox = Philox::new(Seed(seed), STREAM);
        let block = philox.block(u64::MAX);
        let first = u64::from(block[0]) | (u64::from(block[1]) << 32);
        let second = u64::from(block[2]) | (u64::from(block[3]) << 32);
        let mut s = philox.stream_from(u64::MAX);
        let before = s.clone();
        if rejects(first, bound) && rejects(second, bound) {
            assert_eq!(s.below(bound), Err(RngError::Exhausted { stream: STREAM }));
            assert_eq!(s, before);
            found += 1;
        } else {
            let expected = reference_below(&mut before.clone(), bound);
            assert_eq!(s.below(bound), Ok(expected));
        }
    }
    // With rejection probability near one half per word, about a quarter of the 64 seeds reject both.
    assert!(found >= 4, "only {found} seeds rejected both words");
}

/// `next_u64` takes the next word as its low half and the one after as its high half, across a
/// block boundary too.
#[test]
fn next_u64_is_two_words_low_first() {
    let philox = Philox::new(SEED, STREAM);
    let b0 = philox.block(0);
    let b1 = philox.block(1);
    let mut s = philox.stream();
    assert_eq!(s.next_u32(), Ok(b0[0]));
    assert_eq!(
        s.next_u64(),
        Ok(u64::from(b0[1]) | (u64::from(b0[2]) << 32))
    );
    assert_eq!(
        s.next_u64(),
        Ok(u64::from(b0[3]) | (u64::from(b1[0]) << 32))
    );
    assert_eq!(s.position(), 5);
}

/// `next_f64` is the top 53 bits of `next_u64` times 2^−53, exactly: a whole number of 2^−53 steps
/// in [0, 1), positive, never rounded.
#[test]
fn next_f64_is_the_top_53_bits_exactly() {
    let mut s = Philox::new(SEED, STREAM).stream();
    for _ in 0..20_000 {
        let word = s.clone().next_u64().unwrap();
        let v = s.next_f64().unwrap().get();
        assert!((0.0..1.0).contains(&v));
        assert_eq!(v.to_bits() >> 63, 0, "negative zero or negative");
        let k = v * TWO_POW_53;
        assert_eq!(k.fract(), 0.0);
        assert_eq!(k as u64, word >> 11);
    }
}

/// `below` returns what the plain rule returns and draws exactly the words the plain rule draws,
/// for bounds across the whole range, rejections included.
#[test]
fn below_follows_the_rule_exactly() {
    let bounds = [
        1u64,
        2,
        3,
        5,
        6,
        7,
        10,
        100,
        1000,
        (1 << 31) - 1,
        (1 << 32) - 1,
        1 << 32,
        (1 << 32) + 1,
        3 << 40,
        (1 << 63) - 1,
        1 << 63,
        (1 << 63) + 1,
        3 << 62,
        u64::MAX - 1,
        u64::MAX,
    ];
    let mut rejected = 0;
    for (i, bound) in bounds.into_iter().enumerate() {
        let mut s = Philox::new(SEED, StreamId(i as u64)).stream();
        for _ in 0..2_000 {
            let mut reference = s.clone();
            let expected = reference_below(&mut reference, bound);
            let start = s.position();
            assert_eq!(s.below(bound), Ok(expected), "bound {bound}");
            assert!(expected < bound);
            assert_eq!(s, reference, "bound {bound}: different words drawn");
            if s.position() - start > 2 {
                rejected += 1;
            }
        }
    }
    assert!(
        rejected > 1_000,
        "the rejection path ran only {rejected} times"
    );
}

/// `below(1)` is always 0 and still draws one 64-bit word, so how many words a draw takes depends
/// only on whether words are rejected, never on the bound's value otherwise.
#[test]
fn below_one_is_zero_and_still_draws() {
    let mut s = Philox::new(SEED, STREAM).stream();
    for i in 1..=100u128 {
        assert_eq!(s.below(1), Ok(0));
        assert_eq!(s.position(), 2 * i);
    }
}

/// `range_inclusive(low, high)` is `low + below(high − low + 1)`, or `next_u64` for the whole of
/// `u64`, and stays inside its range.
#[test]
fn range_inclusive_is_low_plus_below() {
    let ranges = [
        (0u64, 0u64),
        (7, 7),
        (0, 1),
        (10, 20),
        (1_000_000, 1_000_005),
        (u64::MAX - 1, u64::MAX),
        (u64::MAX, u64::MAX),
        (1, u64::MAX),
        (0, u64::MAX - 1),
        (0, u64::MAX),
        (1 << 63, u64::MAX),
    ];
    for (i, (low, high)) in ranges.into_iter().enumerate() {
        let mut s = Philox::new(SEED, StreamId(1000 + i as u64)).stream();
        for _ in 0..1_000 {
            let mut reference = s.clone();
            let expected = if high - low == u64::MAX {
                reference.next_u64().unwrap()
            } else {
                low + reference_below(&mut reference, high - low + 1)
            };
            let v = s.range_inclusive(low, high).unwrap();
            assert_eq!(v, expected, "range {low}..={high}");
            assert!(low <= v && v <= high);
            assert_eq!(s, reference);
        }
    }
}

/// Different stream ids, and different seeds, give different blocks. For one seed this is certain,
/// because Philox is a bijection on counters and every (stream, block) pair is a different counter;
/// across seeds a repeat would be a 2^−128 coincidence.
#[test]
fn different_streams_and_seeds_give_different_blocks() {
    let seeds = [0u64, 1, 2, SEED.0, 0x8000_0000_0000_0000, u64::MAX];
    let streams = [
        0u64,
        1,
        2,
        0xffff_ffff,
        1 << 32,
        (1 << 32) + 1,
        STREAM.0,
        u64::MAX - 1,
        u64::MAX,
    ];
    let blocks = [0u64, 1, 2, 0xffff_ffff, 1 << 32, u64::MAX - 1, u64::MAX];
    let mut seen = BTreeSet::new();
    let mut count = 0;
    for seed in seeds {
        for stream in streams {
            let philox = Philox::new(Seed(seed), StreamId(stream));
            for block in blocks {
                seen.insert(philox.block(block));
                count += 1;
            }
        }
    }
    assert_eq!(seen.len(), count);

    // Two neighbouring streams share no word in their first 4,096.
    let a: BTreeSet<u32> = {
        let mut s = Philox::new(SEED, StreamId(0)).stream();
        (0..4_096).map(|_| s.next_u32().unwrap()).collect()
    };
    let mut b = Philox::new(SEED, StreamId(1)).stream();
    let shared = (0..4_096)
        .filter(|_| a.contains(&b.next_u32().unwrap()))
        .count();
    // 4,096 × 4,096 / 2^32 ≈ 0.004 shared words are expected by chance.
    assert!(shared <= 1, "{shared} shared words");
}

/// A smoke test, not a quality claim (the statistical evidence is the authors' `TestU01` results):
/// every bit of the first 2^16 words is set about half the time, and `below(6)` fills its six
/// values evenly. A wrong word order or a broken round would fail this; it is deterministic, so it
/// passes or fails the same way every time.
#[test]
fn bits_and_small_ranges_look_uniform() {
    let mut s = Philox::new(SEED, STREAM).stream();
    let n = 1u32 << 16;
    let mut ones = [0u32; 32];
    for _ in 0..n {
        let w = s.next_u32().unwrap();
        for (bit, count) in ones.iter_mut().enumerate() {
            *count += (w >> bit) & 1;
        }
    }
    // Mean 2^15 and standard deviation 2^7: allow five standard deviations.
    for (bit, count) in ones.into_iter().enumerate() {
        assert!(count.abs_diff(n / 2) < 5 * 128, "bit {bit}: {count}");
    }

    let draws = 60_000u64;
    let mut counts = [0u64; 6];
    for _ in 0..draws {
        counts[usize::try_from(s.below(6).unwrap()).unwrap()] += 1;
    }
    let expected = draws / 6;
    // Summed in a written order, first count first (chapter 23: no reduction of unwritten order).
    let mut squares = 0u64;
    for &c in &counts {
        squares += c.abs_diff(expected).pow(2);
    }
    let chi2 = squares / expected;
    // The 99.99th percentile of chi-squared with 5 degrees of freedom is about 25.7.
    assert!(chi2 < 26, "chi-squared {chi2}, counts {counts:?}");
}

#[test]
fn errors_say_what_went_wrong() {
    let text = |e: RngError| e.to_string();
    assert_eq!(
        text(RngError::Exhausted {
            stream: StreamId(42)
        }),
        "random stream 42 has too few of its 2^66 words left for this draw"
    );
    assert_eq!(
        text(RngError::ZeroBound),
        "a random integer below zero was asked for"
    );
    assert_eq!(
        text(RngError::LowAboveHigh { low: 9, high: 8 }),
        "a random integer from 9 to 8 was asked for, but 9 is above 8"
    );
    assert_eq!(
        text(RngError::Defect),
        "a random draw step that cannot fail did fail (a defect in geom-num)"
    );
}
