//! Known-answer tests for Philox4x32-10 (`geom_num::rng`): the authors' published vectors, checked
//! through the raw function, through this crate's counter layout, through a stream, and at compile
//! time.
//!
//! The vectors are the three `philox4x32 10` lines of Random123's `tests/kat_vectors`, by D. E. Shaw
//! Research, the authors of Philox:
//! <https://github.com/DEShawResearch/random123/blob/9545ff6413f258be2f04c1d319d99aaef7521150/tests/kat_vectors>.
//! Random123 is under the BSD-3-Clause licence, "Copyright 2010-2012, D. E. Shaw Research":
//! <https://github.com/DEShawResearch/random123/blob/9545ff6413f258be2f04c1d319d99aaef7521150/LICENSE>.
//! Test vectors are facts; the code that checks them is ours. The lines are quoted below with each
//! run of spaces reduced to one. Their columns are the generator, the number of rounds, the four
//! counter words, the two key words and the four expected output words. The three inputs are all
//! zeros, all ones, and the first hexadecimal digits of π (the file's comment: counter
//! `digits_of_pi`, key `more_digits_of_pi`).
//!
//! ```text
//! philox4x32 10 00000000 00000000 00000000 00000000 00000000 00000000 6627e8d5 e169c58d bc57ac4c 9b00dbd8
//! philox4x32 10 ffffffff ffffffff ffffffff ffffffff ffffffff ffffffff 408f276d 41c83b0e a20bc7c6 6d5451fd
//! philox4x32 10 243f6a88 85a308d3 13198a2e 03707344 a4093822 299f31d0 d16cfe09 94fdcceb 5001e420 24126ea1
//! ```
//!
//! Matching all three pins down the multipliers, which multiplier takes which word, the Weyl
//! constants, the order of key steps and rounds, the P-box and the number of rounds: a mistake in
//! any of them changes every output word.

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

use geom_num::rng::{Philox, Seed, StreamId, philox4x32_10};

/// One known answer: counter, key, expected block.
struct Kat {
    counter: [u32; 4],
    key: [u32; 2],
    expected: [u32; 4],
}

/// The three `philox4x32 10` vectors of Random123's `tests/kat_vectors`, in its column order.
const KATS: [Kat; 3] = [
    Kat {
        counter: [0x0000_0000, 0x0000_0000, 0x0000_0000, 0x0000_0000],
        key: [0x0000_0000, 0x0000_0000],
        expected: [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8],
    },
    Kat {
        counter: [0xffff_ffff, 0xffff_ffff, 0xffff_ffff, 0xffff_ffff],
        key: [0xffff_ffff, 0xffff_ffff],
        expected: [0x408f_276d, 0x41c8_3b0e, 0xa20b_c7c6, 0x6d54_51fd],
    },
    Kat {
        counter: [0x243f_6a88, 0x85a3_08d3, 0x1319_8a2e, 0x0370_7344],
        key: [0xa409_3822, 0x299f_31d0],
        expected: [0xd16c_fe09, 0x94fd_cceb, 0x5001_e420, 0x2412_6ea1],
    },
];

/// The seed, stream id and block index that this crate's layout maps onto a Philox counter and key:
/// the key is the seed's low and high words, and the counter is the block index's low and high
/// words, then the stream id's.
fn layout(kat: &Kat) -> (Seed, StreamId, u64) {
    let join = |low: u32, high: u32| (u64::from(high) << 32) | u64::from(low);
    (
        Seed(join(kat.key[0], kat.key[1])),
        StreamId(join(kat.counter[2], kat.counter[3])),
        join(kat.counter[0], kat.counter[1]),
    )
}

#[test]
fn the_raw_function_gives_the_published_answers() {
    for kat in &KATS {
        assert_eq!(
            philox4x32_10(kat.counter, kat.key),
            kat.expected,
            "counter {:08x?}, key {:08x?}",
            kat.counter,
            kat.key
        );
    }
}

#[test]
fn the_counter_layout_gives_the_published_answers() {
    for kat in &KATS {
        let (seed, stream, block) = layout(kat);
        let philox = Philox::new(seed, stream);
        assert_eq!(philox.key(), kat.key);
        assert_eq!(philox.counter(block), kat.counter);
        assert_eq!(philox.block(block), kat.expected);
        assert_eq!(philox.seed(), seed);
        assert_eq!(philox.stream_id(), stream);
    }
}

#[test]
fn a_stream_gives_the_published_answers_in_order() {
    for kat in &KATS {
        let (seed, stream, block) = layout(kat);
        let mut s = Philox::new(seed, stream).stream_from(block);
        assert_eq!(s.position(), u128::from(block) * 4);
        let words: Vec<u32> = (0..4).map(|_| s.next_u32().unwrap()).collect();
        assert_eq!(words, kat.expected);
        assert_eq!(s.position(), u128::from(block) * 4 + 4);
    }
}

/// The generator is a `const fn`, and the compiler's own evaluation gives the published answers
/// too, so a constant table of random numbers is the same as one computed at run time.
#[test]
fn compile_time_evaluation_gives_the_published_answers() {
    const ZEROS: [u32; 4] = philox4x32_10([0; 4], [0; 2]);
    const ONES: [u32; 4] = Philox::new(Seed(u64::MAX), StreamId(u64::MAX)).block(u64::MAX);
    const PI: [u32; 4] = philox4x32_10(
        [0x243f_6a88, 0x85a3_08d3, 0x1319_8a2e, 0x0370_7344],
        [0xa409_3822, 0x299f_31d0],
    );
    assert_eq!(ZEROS, KATS[0].expected);
    assert_eq!(ONES, KATS[1].expected);
    assert_eq!(PI, KATS[2].expected);
}

/// Drawing from a stream is a `const fn` too, and the compiler's evaluation draws the same numbers
/// as the target does at run time.
#[test]
fn compile_time_draws_match_run_time_draws() {
    const DRAWS: (u32, u64, u64, f64, u64) = {
        let mut s = Philox::new(Seed(0), StreamId(0)).stream();
        let a = match s.next_u32() {
            Ok(v) => v,
            Err(_) => 0,
        };
        let b = match s.next_u64() {
            Ok(v) => v,
            Err(_) => 0,
        };
        let c = match s.below(1_000_003) {
            Ok(v) => v,
            Err(_) => 0,
        };
        let d = match s.next_f64() {
            Ok(v) => v.get(),
            Err(_) => -1.0,
        };
        let e = match s.range_inclusive(10, 20) {
            Ok(v) => v,
            Err(_) => 0,
        };
        (a, b, c, d, e)
    };
    let zeros = KATS[0].expected;
    assert_eq!(DRAWS.0, zeros[0]);
    assert_eq!(DRAWS.1, u64::from(zeros[1]) | (u64::from(zeros[2]) << 32));

    let mut s = Philox::new(Seed(0), StreamId(0)).stream();
    assert_eq!(s.next_u32().unwrap(), DRAWS.0);
    assert_eq!(s.next_u64().unwrap(), DRAWS.1);
    assert_eq!(s.below(1_000_003).unwrap(), DRAWS.2);
    assert_eq!(s.next_f64().unwrap().get().to_bits(), DRAWS.3.to_bits());
    assert_eq!(s.range_inclusive(10, 20).unwrap(), DRAWS.4);
}
