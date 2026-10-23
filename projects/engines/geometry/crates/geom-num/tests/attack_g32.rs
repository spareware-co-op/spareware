//! Adversarial tests for `geom_num::rng` (piece G32): every promise the module makes, attacked with
//! oracles written independently of its code, and every rule the board, the randomised-method rule
//! and chapter 21 set for the piece.
//!
//! Oracles, each written from its definition and sharing no code with `src/rng.rs`:
//! - **Philox4x32-R a second way:** 64-bit arithmetic with explicit reductions modulo 2^32 (no
//!   wrapping operations) and the round count as a parameter. It is first held to all six
//!   `philox4x32` lines of Random123's known answers, the 7-round lines as well as the 10-round
//!   ones, so the oracle's round structure is pinned apart from its round count. Then
//!   `philox4x32_10` and `Philox::block` are compared with it on edge and generated inputs.
//! - **A model of a stream:** a position and nothing else, every word recomputed from the oracle,
//!   Lemire's rule with its threshold taken as a 128-bit remainder, and the double made by dividing
//!   by 2^53 rather than multiplying by 2^−53. Random sequences of every draw, started anywhere
//!   (both sides of the 32-bit carry, the last blocks of a stream), must give the same values, the
//!   same errors and the same positions as `Stream`; after each draw the stream must equal the one
//!   `stream_from` builds at that position; and a failed draw must change nothing.
//!
//! Also attacked: avalanche over every key, block and stream bit; draws with rejections evaluated
//! at compile time; that no type has a `Default` and `Stream` is not `Copy`; and the clippy ban,
//! read from `clippy.toml` itself. Every path the ban tests ask for was checked, in a scratch crate
//! outside this repository, to resolve and to fire under clippy 0.1.99 (the pinned toolchain).
//!
//! The known answers are Random123's `tests/kat_vectors`
//! (<https://github.com/DEShawResearch/random123/blob/9545ff6413f258be2f04c1d319d99aaef7521150/tests/kat_vectors>),
//! BSD-3-Clause, "Copyright 2010-2012, D. E. Shaw Research"
//! (<https://github.com/DEShawResearch/random123/blob/9545ff6413f258be2f04c1d319d99aaef7521150/LICENSE>).
//! Test vectors are facts; the code here is ours. Quoted with each run of spaces reduced to one:
//!
//! ```text
//! philox4x32 7 00000000 00000000 00000000 00000000 00000000 00000000 5f6fb709 0d893f64 4f121f81 4f730a48
//! philox4x32 7 ffffffff ffffffff ffffffff ffffffff ffffffff ffffffff 5207ddc2 45165e59 4d8ee751 8c52f662
//! philox4x32 7 243f6a88 85a308d3 13198a2e 03707344 a4093822 299f31d0 4dfccaba 190a87f0 c47362ba b6b5242a
//! philox4x32 10 00000000 00000000 00000000 00000000 00000000 00000000 6627e8d5 e169c58d bc57ac4c 9b00dbd8
//! philox4x32 10 ffffffff ffffffff ffffffff ffffffff ffffffff ffffffff 408f276d 41c83b0e a20bc7c6 6d5451fd
//! philox4x32 10 243f6a88 85a308d3 13198a2e 03707344 a4093822 299f31d0 d16cfe09 94fdcceb 5001e420 24126ea1
//! ```
//!
//! Property tests run from a fixed seed with no failure files, so they are the same on every run
//! and every target.

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
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;
use std::marker::PhantomData;

use geom_num::Real;
use geom_num::rng::{Philox, RngError, Seed, Stream, StreamId, WORDS_PER_STREAM, philox4x32_10};
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

/// 2^32.
const TWO_32: u64 = 1 << 32;

/// 2^64.
const TWO_64: u128 = 1 << 64;

/// 2^53 as a double.
const TWO_53: f64 = 9_007_199_254_740_992.0;

/// Words in one stream, written from the module documentation: 2^64 blocks of four.
const WORDS: u128 = 1 << 66;

/// The crate's `clippy.toml`, the ban under attack.
const CLIPPY_TOML: &str = include_str!("../clippy.toml");

/// The crate root, whose header states the crate's invariants.
const LIB_RS: &str = include_str!("../src/lib.rs");

// ------------------------------------------------------------------------------------------------
// The oracle
// ------------------------------------------------------------------------------------------------

/// One known answer: rounds, counter, key, expected block.
struct Kat {
    rounds: u32,
    counter: [u32; 4],
    key: [u32; 2],
    expected: [u32; 4],
}

/// The six `philox4x32` lines of Random123's `tests/kat_vectors`, in its column order.
const KATS: [Kat; 6] = [
    Kat {
        rounds: 7,
        counter: [0, 0, 0, 0],
        key: [0, 0],
        expected: [0x5f6f_b709, 0x0d89_3f64, 0x4f12_1f81, 0x4f73_0a48],
    },
    Kat {
        rounds: 7,
        counter: [u32::MAX; 4],
        key: [u32::MAX; 2],
        expected: [0x5207_ddc2, 0x4516_5e59, 0x4d8e_e751, 0x8c52_f662],
    },
    Kat {
        rounds: 7,
        counter: [0x243f_6a88, 0x85a3_08d3, 0x1319_8a2e, 0x0370_7344],
        key: [0xa409_3822, 0x299f_31d0],
        expected: [0x4dfc_caba, 0x190a_87f0, 0xc473_62ba, 0xb6b5_242a],
    },
    Kat {
        rounds: 10,
        counter: [0, 0, 0, 0],
        key: [0, 0],
        expected: [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8],
    },
    Kat {
        rounds: 10,
        counter: [u32::MAX; 4],
        key: [u32::MAX; 2],
        expected: [0x408f_276d, 0x41c8_3b0e, 0xa20b_c7c6, 0x6d54_51fd],
    },
    Kat {
        rounds: 10,
        counter: [0x243f_6a88, 0x85a3_08d3, 0x1319_8a2e, 0x0370_7344],
        key: [0xa409_3822, 0x299f_31d0],
        expected: [0xd16c_fe09, 0x94fd_cceb, 0x5001_e420, 0x2412_6ea1],
    },
];

/// Philox4x32 with `rounds` rounds, from the SC '11 paper and Random123's `philox.h`, written in
/// 64-bit arithmetic: each product of two 32-bit words is exact in 64 bits, `mulhi` is the product
/// divided by 2^32 and `mullo` its remainder, and the key schedule adds the Weyl constants and
/// reduces modulo 2^32. Round 1 uses the key as given; every later round first bumps it.
fn oracle(rounds: u32, counter: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    const MUL: [u64; 2] = [0xD251_1F53, 0xCD9E_8D57];
    const WEYL: [u64; 2] = [0x9E37_79B9, 0xBB67_AE85];
    let mut x = counter.map(u64::from);
    let mut k = key.map(u64::from);
    for round in 0..rounds {
        if round > 0 {
            k = [(k[0] + WEYL[0]) % TWO_32, (k[1] + WEYL[1]) % TWO_32];
        }
        let p0 = MUL[0] * x[0];
        let p1 = MUL[1] * x[2];
        x = [
            (p1 / TWO_32) ^ x[1] ^ k[0],
            p1 % TWO_32,
            (p0 / TWO_32) ^ x[3] ^ k[1],
            p0 % TWO_32,
        ];
    }
    x.map(|w| u32::try_from(w).unwrap())
}

/// The low and high 32-bit halves of `v`, by division.
fn split(v: u64) -> [u32; 2] {
    [
        u32::try_from(v % TWO_32).unwrap(),
        u32::try_from(v / TWO_32).unwrap(),
    ]
}

/// Block `block` of stream `stream` of `seed`, by the layout the module documents.
fn oracle_block(seed: u64, stream: u64, block: u64) -> [u32; 4] {
    let [b0, b1] = split(block);
    let [s0, s1] = split(stream);
    oracle(10, [b0, b1, s0, s1], split(seed))
}

/// A stream as nothing but a position; every word recomputed from the oracle.
#[derive(Clone, Debug)]
struct Model {
    seed: u64,
    stream: u64,
    pos: u128,
}

impl Model {
    fn word(&self, pos: u128) -> u32 {
        let block = u64::try_from(pos / 4).unwrap();
        oracle_block(self.seed, self.stream, block)[usize::try_from(pos % 4).unwrap()]
    }

    fn exhausted(&self) -> RngError {
        RngError::Exhausted {
            stream: StreamId(self.stream),
        }
    }

    fn next_u32(&mut self) -> Result<u32, RngError> {
        if self.pos >= WORDS {
            return Err(self.exhausted());
        }
        let w = self.word(self.pos);
        self.pos += 1;
        Ok(w)
    }

    fn next_u64(&mut self) -> Result<u64, RngError> {
        if WORDS - self.pos < 2 {
            return Err(self.exhausted());
        }
        let low = u64::from(self.next_u32()?);
        let high = u64::from(self.next_u32()?);
        Ok(low + high * TWO_32)
    }

    /// Division by 2^53, not multiplication by 2^−53: the two agree bit for bit only if both are
    /// exact, which is what the module claims.
    fn next_f64(&mut self) -> Result<f64, RngError> {
        self.next_u64().map(|x| (x >> 11) as f64 / TWO_53)
    }

    /// Lemire's rule from its definition: reject while the low half of x · n is below 2^64 mod n.
    fn below(&mut self, n: u64) -> Result<u64, RngError> {
        if n == 0 {
            return Err(RngError::ZeroBound);
        }
        let start = self.pos;
        let n = u128::from(n);
        let threshold = TWO_64 % n;
        loop {
            match self.next_u64() {
                Ok(x) => {
                    let product = u128::from(x) * n;
                    if product % TWO_64 >= threshold {
                        return Ok(u64::try_from(product / TWO_64).unwrap());
                    }
                }
                Err(e) => {
                    self.pos = start;
                    return Err(e);
                }
            }
        }
    }

    fn range_inclusive(&mut self, low: u64, high: u64) -> Result<u64, RngError> {
        if low > high {
            return Err(RngError::LowAboveHigh { low, high });
        }
        if low == 0 && high == u64::MAX {
            return self.next_u64();
        }
        self.below(high - low + 1).map(|v| low + v)
    }
}

/// The stream of `philox` at word position `pos`, built by skipping to its block and drawing the
/// rest word by word.
fn stream_at(philox: Philox, pos: u128) -> Stream {
    let (block, extra) = if pos == WORDS {
        (u64::MAX, 4)
    } else {
        (u64::try_from(pos / 4).unwrap(), pos % 4)
    };
    let mut s = philox.stream_from(block);
    for _ in 0..extra {
        s.next_u32().unwrap();
    }
    s
}

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), a test-only input generator from a fixed
/// seed (rule 6: test generators are separate and their seeds fixed).
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Property tests from a fixed seed, with no failure files written.
fn config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        rng_seed: RngSeed::Fixed(0x4733_3241_5454_4143),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

// ------------------------------------------------------------------------------------------------
// The block function
// ------------------------------------------------------------------------------------------------

/// The oracle reproduces all six published `philox4x32` answers, at 7 rounds and at 10, so it is
/// a trustworthy second implementation.
#[test]
fn the_oracle_gives_all_six_published_answers() {
    for kat in &KATS {
        assert_eq!(
            oracle(kat.rounds, kat.counter, kat.key),
            kat.expected,
            "oracle, {} rounds, counter {:08x?}, key {:08x?}",
            kat.rounds,
            kat.counter,
            kat.key
        );
    }
}

/// `philox4x32_10` is the 10-round function, not the 7-round one, and matches the oracle at the
/// published inputs.
#[test]
fn philox4x32_10_is_ten_rounds_at_the_published_inputs() {
    for kat in &KATS {
        let got = philox4x32_10(kat.counter, kat.key);
        assert_eq!(
            got,
            oracle(10, kat.counter, kat.key),
            "counter {:08x?}, key {:08x?}",
            kat.counter,
            kat.key
        );
        if kat.rounds == 7 {
            assert_ne!(got, kat.expected, "philox4x32_10 gave the 7-round answer");
        }
    }
}

/// Every combination of four edge words in all six input words (4^6 = 4,096 inputs), and each
/// input word through eight edge values with the others fixed.
#[test]
fn philox4x32_10_matches_the_oracle_on_edge_inputs() {
    let corners = [0u32, 1, 0x8000_0000, u32::MAX];
    for i in 0..4096usize {
        let mut digits = [0u32; 6];
        let mut rest = i;
        for d in &mut digits {
            *d = corners[rest % 4];
            rest /= 4;
        }
        let counter = [digits[0], digits[1], digits[2], digits[3]];
        let key = [digits[4], digits[5]];
        assert_eq!(
            philox4x32_10(counter, key),
            oracle(10, counter, key),
            "counter {counter:08x?}, key {key:08x?}"
        );
    }
    let edges = [
        0u32,
        1,
        0x7fff_ffff,
        0x8000_0000,
        0xffff_fffe,
        u32::MAX,
        0xD251_1F53,
        0xCD9E_8D57,
    ];
    let base = [
        0x243f_6a88u32,
        0x85a3_08d3,
        0x1319_8a2e,
        0x0370_7344,
        0xa409_3822,
        0x299f_31d0,
    ];
    for slot in 0..6 {
        for &edge in &edges {
            let mut w = base;
            w[slot] = edge;
            let counter = [w[0], w[1], w[2], w[3]];
            let key = [w[4], w[5]];
            assert_eq!(
                philox4x32_10(counter, key),
                oracle(10, counter, key),
                "counter {counter:08x?}, key {key:08x?}"
            );
        }
    }
}

proptest! {
    #![proptest_config(config(4096))]

    /// Generated counters and keys.
    #[test]
    fn philox4x32_10_matches_the_oracle(counter in any::<[u32; 4]>(), key in any::<[u32; 2]>()) {
        prop_assert_eq!(philox4x32_10(counter, key), oracle(10, counter, key));
    }

    /// The layout: key = the seed's halves, counter = the block's halves then the stream's.
    #[test]
    fn philox_block_follows_the_documented_layout(
        seed in any::<u64>(),
        stream in any::<u64>(),
        block in any::<u64>(),
    ) {
        let philox = Philox::new(Seed(seed), StreamId(stream));
        let [b0, b1] = split(block);
        let [s0, s1] = split(stream);
        prop_assert_eq!(philox.key(), split(seed));
        prop_assert_eq!(philox.counter(block), [b0, b1, s0, s1]);
        prop_assert_eq!(philox.block(block), oracle_block(seed, stream, block));
        prop_assert_eq!(philox.seed(), Seed(seed));
        prop_assert_eq!(philox.stream_id(), StreamId(stream));
    }
}

// ------------------------------------------------------------------------------------------------
// Streams against the model
// ------------------------------------------------------------------------------------------------

/// One draw of the model test.
#[derive(Clone, Copy, Debug)]
enum Draw {
    U32,
    U64,
    F64,
    Below(u64),
    Range(u64, u64),
}

/// Bounds across the whole range, with many that reject often.
fn bound() -> impl Strategy<Value = u64> {
    prop_oneof![
        0u64..=17,
        any::<u64>(),
        (0u32..64).prop_map(|k| 1u64 << k),
        (1u32..64).prop_map(|k| (1u64 << k) + 1),
        (1u32..=64).prop_map(|k| u64::try_from((1u128 << k) - 1).unwrap()),
        (1u64 << 63)..=u64::MAX,
    ]
}

fn draw() -> impl Strategy<Value = Draw> {
    prop_oneof![
        Just(Draw::U32),
        Just(Draw::U64),
        Just(Draw::F64),
        bound().prop_map(Draw::Below),
        (any::<u64>(), any::<u64>()).prop_map(|(a, b)| Draw::Range(a, b)),
        (any::<u64>(), bound()).prop_map(|(a, n)| Draw::Range(a, a.saturating_add(n))),
        Just(Draw::Range(0, u64::MAX)),
        Just(Draw::Range(u64::MAX, u64::MAX)),
    ]
}

/// Starting blocks: the start, both sides of the 32-bit carry, the last blocks, anywhere.
fn start_block() -> impl Strategy<Value = u64> {
    prop_oneof![
        0u64..4,
        0xffff_fff0u64..=0x1_0000_0010,
        (u64::MAX - 6)..=u64::MAX,
        any::<u64>(),
    ]
}

fn stream_id() -> impl Strategy<Value = u64> {
    prop_oneof![
        0u64..3,
        Just(0xffff_ffff),
        Just(1 << 32),
        Just(u64::MAX),
        any::<u64>(),
    ]
}

/// Applies one draw to the stream and to the model and compares them; a failed draw must leave
/// the stream as it was. Returns whether the draw failed.
fn step(s: &mut Stream, m: &mut Model, d: Draw) -> Result<bool, TestCaseError> {
    let before = s.clone();
    let failed = match d {
        Draw::U32 => {
            let (got, want) = (s.next_u32(), m.next_u32());
            prop_assert_eq!(got, want, "next_u32 at {}", before.position());
            got.is_err()
        }
        Draw::U64 => {
            let (got, want) = (s.next_u64(), m.next_u64());
            prop_assert_eq!(got, want, "next_u64 at {}", before.position());
            got.is_err()
        }
        Draw::F64 => {
            let (got, want) = (s.next_f64().map(Real::get), m.next_f64());
            prop_assert_eq!(
                got.map(f64::to_bits),
                want.map(f64::to_bits),
                "next_f64 at {}",
                before.position()
            );
            if let Ok(v) = got {
                prop_assert!(
                    (0.0..1.0).contains(&v) && v.to_bits() >> 63 == 0,
                    "next_f64 {}",
                    v
                );
            }
            got.is_err()
        }
        Draw::Below(n) => {
            let (got, want) = (s.below(n), m.below(n));
            prop_assert_eq!(got, want, "below({}) at {}", n, before.position());
            if let Ok(v) = got {
                prop_assert!(v < n, "below({}) gave {}", n, v);
            }
            got.is_err()
        }
        Draw::Range(low, high) => {
            let (got, want) = (s.range_inclusive(low, high), m.range_inclusive(low, high));
            prop_assert_eq!(
                got,
                want,
                "range_inclusive({}, {}) at {}",
                low,
                high,
                before.position()
            );
            if let Ok(v) = got {
                prop_assert!(
                    low <= v && v <= high,
                    "range_inclusive({}, {}) gave {}",
                    low,
                    high,
                    v
                );
            }
            got.is_err()
        }
    };
    prop_assert_eq!(s.position(), m.pos, "position after {:?}", d);
    prop_assert_eq!(
        s.remaining(),
        WORDS_PER_STREAM - m.pos,
        "remaining after {:?}",
        d
    );
    if failed {
        prop_assert_eq!(&*s, &before, "{:?} failed but changed the stream", d);
    }
    // Two streams at the same position are equal: the one drawn here and the one built there.
    prop_assert_eq!(&*s, &stream_at(s.philox(), m.pos), "state after {:?}", d);
    Ok(failed)
}

proptest! {
    #![proptest_config(config(768))]

    /// Every draw function, in random order, from anywhere in a stream, against the model.
    #[test]
    fn a_stream_is_its_model(
        seed in any::<u64>(),
        stream in stream_id(),
        start in start_block(),
        skip in 0u32..4,
        draws in proptest::collection::vec(draw(), 1..48),
    ) {
        let philox = Philox::new(Seed(seed), StreamId(stream));
        let mut s = philox.stream_from(start);
        let mut m = Model { seed, stream, pos: u128::from(start) * 4 };
        prop_assert_eq!(s.position(), m.pos);
        for _ in 0..skip {
            step(&mut s, &mut m, Draw::U32)?;
        }
        for d in draws {
            step(&mut s, &mut m, d)?;
        }
    }
}

/// The model test must reach exhaustion inside every draw, from every word offset of the last
/// block, including a `below` that rejects and then runs out.
#[test]
fn every_draw_runs_out_cleanly_from_every_offset_of_the_last_block() {
    let draws = [
        Draw::U32,
        Draw::U64,
        Draw::F64,
        Draw::Below(3),
        Draw::Below((1 << 63) + 1),
        Draw::Range(5, 9),
        Draw::Range(0, u64::MAX),
        Draw::Range(1, u64::MAX),
    ];
    let mut exhausted = 0u32;
    for seed in 0..32u64 {
        let stream = seed ^ 0xdead_beef;
        for offset in 0..4 {
            for &first in &draws {
                for &second in &draws {
                    let mut s = Philox::new(Seed(seed), StreamId(stream)).stream_from(u64::MAX);
                    let mut m = Model {
                        seed,
                        stream,
                        pos: WORDS - 4,
                    };
                    let mut run = || -> Result<u32, TestCaseError> {
                        let mut failed = 0;
                        for _ in 0..offset {
                            failed += u32::from(step(&mut s, &mut m, Draw::U32)?);
                        }
                        for d in [first, second, Draw::U32] {
                            failed += u32::from(step(&mut s, &mut m, d)?);
                        }
                        Ok(failed)
                    };
                    match run() {
                        Ok(failed) => exhausted += failed,
                        Err(e) => {
                            panic!("seed {seed}, offset {offset}, {first:?} then {second:?}: {e}")
                        }
                    }
                }
            }
        }
    }
    // The sequences are built to run out; make sure many draws did.
    assert!(exhausted > 4000, "only {exhausted} draws ran out");
}

// ------------------------------------------------------------------------------------------------
// Independence and avalanche
// ------------------------------------------------------------------------------------------------

/// Flipping any one of the 64 key bits, 64 block bits or 64 stream bits flips each of the 128
/// output bits about half the time: 1,024 fixed inputs per input bit, each count within six
/// standard deviations (16 each) of 512. A layout that let a key or stream bit reach only part of
/// the block, or a weak round, fails this. Deterministic: it passes or fails the same way always.
#[test]
fn every_seed_block_and_stream_bit_reaches_every_output_bit() {
    const SAMPLES: u32 = 1024;
    let mut state = 0x6733_325f_6176_616c_u64;
    let inputs: Vec<(u64, u64, u64)> = (0..SAMPLES)
        .map(|_| {
            (
                splitmix(&mut state),
                splitmix(&mut state),
                splitmix(&mut state),
            )
        })
        .collect();
    let mut worst = (0u32, 0usize, 0usize);
    for input_bit in 0..192usize {
        let mut flips = [0u32; 128];
        for &(seed, stream, block) in &inputs {
            let base = Philox::new(Seed(seed), StreamId(stream)).block(block);
            let mask = 1u64 << (input_bit % 64);
            let (s2, st2, b2) = match input_bit / 64 {
                0 => (seed ^ mask, stream, block),
                1 => (seed, stream, block ^ mask),
                _ => (seed, stream ^ mask, block),
            };
            let other = Philox::new(Seed(s2), StreamId(st2)).block(b2);
            for (w, (a, b)) in base.iter().zip(other).enumerate() {
                let x = a ^ b;
                for bit in 0..32 {
                    flips[w * 32 + bit] += (x >> bit) & 1;
                }
            }
        }
        for (out_bit, &count) in flips.iter().enumerate() {
            let dev = count.abs_diff(SAMPLES / 2);
            if dev > worst.0 {
                worst = (dev, input_bit, out_bit);
            }
            assert!(
                dev <= 96,
                "flipping input bit {input_bit} (0-63 seed, 64-127 block, 128-191 stream) flipped \
                 output bit {out_bit} {count} times in {SAMPLES}; expected 512 ± 96"
            );
        }
    }
    println!("worst avalanche deviation {worst:?} (count off 512, input bit, output bit)");
}

/// Consecutive seeds, the way callers will number them, give unrelated first blocks: the mean
/// Hamming distance between the first blocks of seeds k and k + 1 is 64 of 128 bits.
#[test]
fn consecutive_seeds_and_streams_give_unrelated_blocks() {
    let n = 1u64 << 14;
    for (name, f) in [
        (
            "seed",
            (|k: u64| Philox::new(Seed(k), StreamId(0)).block(0)) as fn(u64) -> [u32; 4],
        ),
        ("stream", |k: u64| {
            Philox::new(Seed(0), StreamId(k)).block(0)
        }),
        ("block", |k: u64| Philox::new(Seed(0), StreamId(0)).block(k)),
    ] {
        let mut total = 0u64;
        for k in 0..n {
            let (a, b) = (f(k), f(k + 1));
            // Summed in a written order, word 0 first (chapter 23: no reduction of unwritten
            // order).
            for (x, y) in a.iter().zip(b) {
                total += u64::from((x ^ y).count_ones());
            }
        }
        // Each distance has variance 32; the mean of 2^14 has standard deviation 0.044.
        let mean = total as f64 / n as f64;
        assert!(
            (mean - 64.0).abs() < 0.3,
            "consecutive {name}s: mean Hamming distance {mean}, expected 64 ± 0.3"
        );
    }
}

// ------------------------------------------------------------------------------------------------
// Compile time, types and size
// ------------------------------------------------------------------------------------------------

/// A `below` with rejections, and doubles, evaluated by the compiler, give the run-time bits.
#[test]
fn compile_time_draws_with_rejections_match_run_time() {
    const BOUND: u64 = (1 << 63) + 1;
    const TABLE: ([u64; 16], [u64; 4], u128) = {
        let mut s = Philox::new(Seed(0x5350_4152_454b_4954), StreamId(3)).stream();
        let mut ints = [0u64; 16];
        let mut i = 0;
        while i < 16 {
            ints[i] = match s.below(BOUND) {
                Ok(v) => v,
                Err(_) => u64::MAX,
            };
            i += 1;
        }
        let mut doubles = [0u64; 4];
        let mut j = 0;
        while j < 4 {
            doubles[j] = match s.next_f64() {
                Ok(v) => v.get().to_bits(),
                Err(_) => u64::MAX,
            };
            j += 1;
        }
        (ints, doubles, s.position())
    };
    let mut s = Philox::new(Seed(0x5350_4152_454b_4954), StreamId(3)).stream();
    let ints: Vec<u64> = (0..16).map(|_| s.below(BOUND).unwrap()).collect();
    let doubles: Vec<u64> = (0..4)
        .map(|_| s.next_f64().unwrap().get().to_bits())
        .collect();
    assert_eq!(ints, TABLE.0);
    assert_eq!(doubles, TABLE.1);
    assert_eq!(s.position(), TABLE.2);
    // 16 draws of 2 words and 4 doubles of 2 words take 40 words with no rejection.
    assert!(
        TABLE.2 > 40,
        "the compile-time table never rejected (position {})",
        TABLE.2
    );
}

/// Detects a trait implementation without needing it: the inherent method is chosen when the bound
/// holds, the trait's fallback otherwise.
struct Probe<T>(PhantomData<T>);

trait Fallback {
    fn has_default(&self) -> bool {
        false
    }
    fn is_copy(&self) -> bool {
        false
    }
}

impl<T> Fallback for Probe<T> {}

impl<T: Default> Probe<T> {
    #[allow(
        clippy::unused_self,
        reason = "the receiver is the probe: method resolution picks this inherent method over the \
                  trait's only when the bound holds"
    )]
    fn has_default(&self) -> bool {
        true
    }
}

struct CopyProbe<T>(PhantomData<T>);

impl<T> Fallback for CopyProbe<T> {}

impl<T: Copy> CopyProbe<T> {
    #[allow(
        clippy::unused_self,
        reason = "the receiver is the probe: method resolution picks this inherent method over the \
                  trait's only when the bound holds"
    )]
    fn is_copy(&self) -> bool {
        true
    }
}

/// Explicit inputs only: no `Default` seed, stream id, generator or stream; and a stream is not
/// `Copy`. The probe itself is checked on types whose answers are known.
#[test]
fn no_default_seed_and_streams_are_not_copy() {
    assert!(Probe::<u64>(PhantomData).has_default());
    assert!(!Probe::<core::num::NonZeroU64>(PhantomData).has_default());
    assert!(CopyProbe::<u64>(PhantomData).is_copy());
    assert!(!CopyProbe::<String>(PhantomData).is_copy());
    let defaults = [
        ("Seed", Probe::<Seed>(PhantomData).has_default()),
        ("StreamId", Probe::<StreamId>(PhantomData).has_default()),
        ("Philox", Probe::<Philox>(PhantomData).has_default()),
        ("Stream", Probe::<Stream>(PhantomData).has_default()),
    ];
    for (name, has) in defaults {
        assert!(
            !has,
            "{name} implements Default, so a seed can be made without writing one"
        );
    }
    assert!(
        !CopyProbe::<Stream>(PhantomData).is_copy(),
        "Stream is Copy, so a stray copy replays its numbers"
    );
}

/// "A Stream is a few dozen bytes and allocates nothing."
#[test]
fn a_stream_is_a_few_dozen_bytes() {
    let size = core::mem::size_of::<Stream>();
    assert!(size <= 64, "Stream is {size} bytes");
}

// ------------------------------------------------------------------------------------------------
// The clippy ban
// ------------------------------------------------------------------------------------------------

/// The `path` entries of one list of `clippy.toml`.
fn banned(list: &str) -> BTreeSet<String> {
    let start = CLIPPY_TOML
        .find(&format!("{list} = ["))
        .unwrap_or_else(|| panic!("clippy.toml has no {list} list"));
    let rest = &CLIPPY_TOML[start..];
    let end = rest.find("\n]").expect("an unterminated list");
    rest[..end]
        .split("path = \"")
        .skip(1)
        .map(|p| p.split('"').next().unwrap().to_owned())
        .collect()
}

/// Which of `wanted` the list lacks.
fn missing(list: &str, wanted: &[&str]) -> Vec<String> {
    let have = banned(list);
    wanted
        .iter()
        .filter(|p| !have.contains(**p))
        .map(|p| (*p).to_owned())
        .collect()
}

/// Every source the board and the piece's task name is banned: std's randomly seeded hasher and
/// the hash-map constructors that seed it, both clocks, and the rand and getrandom entry points.
#[test]
fn clippy_bans_every_source_the_board_names() {
    let types = missing(
        "disallowed-types",
        &[
            "std::hash::RandomState",
            "rand::rngs::ThreadRng",
            "rand::rngs::OsRng",
            "rand::rngs::StdRng",
            "rand::rngs::SmallRng",
            "rand_core::OsRng",
            "getrandom::SysRng",
        ],
    );
    let methods = missing(
        "disallowed-methods",
        &[
            "std::hash::RandomState::new",
            "std::collections::HashMap::new",
            "std::collections::HashSet::new",
            "std::time::SystemTime::now",
            "std::time::Instant::now",
            "std::process::id",
            "rand::thread_rng",
            "rand::rng",
            "rand::random",
            "rand_core::SeedableRng::from_entropy",
            "rand_core::SeedableRng::from_os_rng",
            "getrandom::getrandom",
            "getrandom::fill",
        ],
    );
    assert!(
        types.is_empty() && methods.is_empty(),
        "missing types {types:?}, missing methods {methods:?}"
    );
}

/// Rule 1 forbids a "process or thread id" and says each clippy.toml "bans every such source that
/// has a path"; rule 3 forbids numbering a stream "by a thread". The process id is banned, the
/// thread's identity is not. `std::thread::current`, `std::thread::Thread::id` and the type
/// `std::thread::ThreadId` all resolve and fire under clippy 0.1.99. They matter in `geom-pool`,
/// the one kernel crate with `std` (chapter 27), where a per-thread stream id is the obvious
/// mistake.
#[test]
fn clippy_bans_thread_identity() {
    let methods = banned("disallowed-methods");
    let types = banned("disallowed-types");
    let banned_any = methods.contains("std::thread::current")
        || methods.contains("std::thread::Thread::id")
        || types.contains("std::thread::ThreadId");
    assert!(
        banned_any,
        "expected one of disallowed-methods std::thread::current / std::thread::Thread::id or \
         disallowed-types std::thread::ThreadId; clippy.toml bans none, so \
         `StreamId(hash(std::thread::current().id()))` passes `cargo clippy -D warnings`. \
         Banned methods: {methods:?}"
    );
}

/// rayon is the one registry crate a kernel crate may use besides libm (G12: "rayon allowed only
/// in geom-pool"). Its thread index is the thread-numbered stream id rule 3 forbids, and
/// work-stealing makes it differ run to run. Both paths resolve and fire under clippy 0.1.99
/// against rayon 1.12; the rand and getrandom entries show the file already bans crates by path
/// before they are depended on (`allow-invalid`).
#[test]
fn clippy_bans_rayons_thread_index() {
    let gaps = missing(
        "disallowed-methods",
        &[
            "rayon::current_thread_index",
            "rayon::ThreadPool::current_thread_index",
        ],
    );
    assert!(
        gaps.is_empty(),
        "clippy.toml does not ban {gaps:?}, so `StreamId(rayon::current_thread_index().unwrap_or(0) \
         as u64)` in geom-pool passes clippy"
    );
}

/// The task names "hash-map default hasher construction". Banning `HashMap::new`,
/// `HashMap::with_capacity` and their `HashSet` twins leaves every other constructor that seeds a
/// `RandomState` from the operating system: `HashMap::default()`, `HashMap::from([..])`,
/// `iter.collect::<HashMap<_, _>>()`, `HashSet::from_iter(..)` and
/// `HashMap::with_hasher(Default::default())` all pass this clippy.toml (checked under clippy
/// 0.1.99). Trait methods cannot be banned by path (clippy: "`std::collections::HashMap::default`
/// does not refer to a reachable function"), so only the type ban closes them; chapter 21 asks for
/// exactly that ban, and it caught all five in the same check.
#[test]
fn clippy_bans_every_default_hasher_construction() {
    let gaps = missing(
        "disallowed-types",
        &["std::collections::HashMap", "std::collections::HashSet"],
    );
    assert!(
        gaps.is_empty(),
        "disallowed-types lacks {gaps:?}: `let m: HashMap<u8, u8> = HashMap::default();`, \
         `HashMap::from([(1, 2)])`, `.collect::<HashMap<_, _>>()` and `HashSet::from_iter([1])` \
         each construct std's randomly seeded hasher and pass clippy -D warnings"
    );
}

/// "A clippy rule bans any other randomness in kernel crates." The file bans the rand and getrandom
/// crates by path, which a kernel crate does not use, but not the randomly seeded hashers and
/// generators a `no_std` crate would actually reach for: hashbrown's default hasher (foldhash,
/// seeded per process) and fastrand's entropy-seeded generator. Every path below resolves and
/// fires under clippy 0.1.99 (hashbrown 0.16.1, foldhash 0.2.0, fastrand 2.3.0).
#[test]
fn clippy_bans_no_std_randomly_seeded_hashers_and_generators() {
    let types = missing(
        "disallowed-types",
        &[
            "hashbrown::DefaultHashBuilder",
            "foldhash::fast::RandomState",
            "foldhash::quality::RandomState",
        ],
    );
    let methods = missing(
        "disallowed-methods",
        &[
            "hashbrown::HashMap::new",
            "hashbrown::HashSet::new",
            "fastrand::Rng::new",
        ],
    );
    assert!(
        types.is_empty() && methods.is_empty(),
        "clippy.toml does not ban types {types:?} or methods {methods:?}: \
         `hashbrown::HashMap::new()` (no_std, randomly seeded by foldhash) and \
         `fastrand::Rng::new()` pass clippy -D warnings"
    );
}

// ------------------------------------------------------------------------------------------------
// The crate header
// ------------------------------------------------------------------------------------------------

/// The crate header lists the crate's jobs and invariants (principle 10). With `rng` added it must
/// name the random generator, and its invariant "Arithmetic on fixed-width integers is checked. A
/// result that does not fit is `None`, never a silently wrapped value" must carve out the
/// specification's arithmetic modulo 2^32, which `rng` does with `wrapping_mul` and `wrapping_add`.
#[test]
fn the_crate_header_states_the_generator_and_its_modular_arithmetic() {
    // The header's text as one line of words, so a phrase broken across lines still matches.
    let header: Vec<&str> = LIB_RS
        .lines()
        .take_while(|l| l.starts_with("//!"))
        .flat_map(|l| l.trim_start_matches("//!").split_whitespace())
        .collect();
    let header = header.join(" ");
    let lower = header.to_lowercase();
    let names_generator = lower.contains("random") || header.contains("rng");
    let claims_no_wrap = header.contains("never a silently wrapped value");
    let carves_out_wrap = lower.contains("modulo 2^32") || lower.contains("wrapping");
    assert!(
        names_generator && (!claims_no_wrap || carves_out_wrap),
        "src/lib.rs header: names the random generator = {names_generator} (expected true); \
         claims no wrapped value = {claims_no_wrap} with an rng exception = {carves_out_wrap} \
         (expected no claim, or an exception). Header:\n{header}"
    );
}
