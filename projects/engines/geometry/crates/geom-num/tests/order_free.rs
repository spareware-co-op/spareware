//! The order-free golden: every `geom-num` operation whose contract says the order of its inputs
//! does not matter, run through the input-shuffle hook and pinned by a golden digest.
//!
//! Chapter 28.13 lists shuffled inputs among the perturbations that must change nothing "wherever
//! an operation's contract says order does not matter". In `geom-num` three contracts say so:
//!
//! - **`ExactSum`** (piece G25): "The result depends on the multiset of terms only: not on their
//!   order, on how they were split into slices or accumulators, on the order or shape of the
//!   merges, or on which of the two internal paths took them" (`src/exact_sum.rs`).
//! - **`DD::add` and `DD::mul`**: "Commutative, bit for bit" (`src/dd.rs`).
//! - **`two_sum`**: commutative, its pair unique and its zero +0 (`src/eft.rs`, `src/dd.rs`).
//!
//! The corpus, from `SplitMix64` with a fixed seed (integer arithmetic, and products of doubles by
//! powers of two, which IEEE 754 makes exact or rounds the same way on every target, so the same
//! terms on every target): 240 term sets, mostly short and a few past the large accumulator's 512-term
//! switch and the 2,047-addition carry allowance, drawn from six families (every exponent and both
//! signs; one binade, for cancellation; pairs that cancel exactly around a residue; exact ties
//! between two doubles, for round-half-to-even; terms near the largest double, for overflow; and
//! sets with infinities and NaNs of chosen payloads, for Neal's special values); and 2,000 pairs
//! of double-double values at every exponent.
//!
//! For each term set, `geom_test::golden::shuffle::order_free` runs the case on the terms in their
//! drawn order, reversed and in [`SHUFFLES`] seeded shuffled orders. Each run sums the terms four
//! ways (one by one, as a slice, as a stream, and in seeded chunks merged in a seeded order),
//! requires the four to agree bit for bit, and writes the result (`round` and `round_ieee`) in a
//! canonical encoding; every order must give the same bytes. Each double-double pair runs in both
//! orders the same way. The whole corpus's bytes are the golden `geom-num/order-free` in the golden
//! store, checked by `geom_test::golden::check`, which runs the corpus twice in the process on two
//! differently perturbed heaps; `kernel.yml` runs it on every target and build mode.
//!
//! The encoding, every value little-endian: per term set, its length as a `u64`, then `round`'s
//! result (0 and the value's bits, or 1 and `NotFinite`'s code 3), then `round_ieee`'s bits as they
//! are (a NaN's payload is Neal's rule, made from integers, so its bits are fixed too); per pair,
//! `add`'s and `mul`'s results (0 and `hi`, `lo`, or 1 and the error's code) and `two_sum`'s pair
//! of the two high parts.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use core::num::NonZeroU64;

use geom_num::eft::two_sum;
use geom_num::{DD, ExactSum, NumError, Real};
use geom_test::golden::shuffle::{Orders, order_free};
use geom_test::golden::{self, Sha256};
use geom_test::{Seed, SplitMix64};

/// The golden in the golden store.
const GOLDEN: &str = "geom-num/order-free";

/// The corpus's seed: "ORDRFREE" in ASCII.
const SEED: u64 = 0x4f52_4452_4652_4545;

/// Shuffled orders per term set and per pair, beside the drawn order and its reverse.
const SHUFFLES: usize = 3;

/// Term sets in the corpus.
const SETS: usize = 240;

/// Double-double pairs in the corpus.
const PAIRS: usize = 2_000;

/// The sizes term sets are drawn with: mostly short, a few past the large accumulator's switch
/// (512 terms) and past the carry allowance (2,047 additions).
const SIZES: [u64; 12] = [0, 1, 2, 3, 4, 5, 8, 13, 40, 300, 600, 2_100];

/// A value in `0..n`.
fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

/// A double with a random sign, the given exponent field (0 for zero and subnormals, 2046 at most)
/// and a random fraction.
fn with_field(rng: &mut SplitMix64, field: u64) -> f64 {
    let sign = rng.next_u64() & (1 << 63);
    let fraction = rng.next_u64() & ((1 << 52) - 1);
    f64::from_bits(sign | (field << 52) | fraction)
}

/// A double as [`with_field`] makes it, with an exponent field in `lo..lo + n`.
fn in_fields(rng: &mut SplitMix64, lo: u64, n: u64) -> f64 {
    let field = lo + below(rng, n);
    with_field(rng, field)
}

/// One term set from one of the six families.
fn term_set(rng: &mut SplitMix64) -> Vec<f64> {
    let n = SIZES[below(rng, SIZES.len() as u64) as usize] as usize;
    match below(rng, 6) {
        // Every exponent, both signs, subnormals and zeros included.
        0 => (0..n).map(|_| in_fields(rng, 0, 2047)).collect(),
        // One binade: heavy cancellation between terms of one size.
        1 => {
            let field = 1 + below(rng, 2046);
            (0..n).map(|_| with_field(rng, field)).collect()
        }
        // Pairs that cancel exactly, around a small residue.
        2 => {
            let mut out = Vec::with_capacity(n + 1);
            out.push(in_fields(rng, 0, 1100));
            while out.len() + 1 < n.max(2) {
                let x = in_fields(rng, 900, 1146);
                out.push(x);
                out.push(-x);
            }
            out
        }
        // Exact ties: 1 + 2^-53 lies halfway between two doubles, and so does every scaled copy.
        3 => {
            let mut out = Vec::new();
            for _ in 0..n.max(1) {
                let scale = f64::from_bits((1 + below(rng, 1900)) << 52);
                let sign = if rng.coin() { 1.0 } else { -1.0 };
                out.push(sign * scale);
                out.push(sign * scale * f64::from_bits(970 << 52)); // 2^-53 of it
            }
            out.truncate(n.max(2));
            out
        }
        // Near the largest double: sums that overflow and sums that come back.
        4 => (0..n)
            .map(|_| {
                let x = in_fields(rng, 2040, 7);
                if rng.coin() { x } else { x.abs() }
            })
            .collect(),
        // Infinities and NaNs among finite terms (Neal's special values).
        _ => {
            let mut out: Vec<f64> = (0..n).map(|_| in_fields(rng, 0, 2047)).collect();
            let specials = 1 + below(rng, 3);
            for _ in 0..specials {
                let v = match below(rng, 3) {
                    0 => f64::INFINITY,
                    1 => f64::NEG_INFINITY,
                    _ => {
                        let sign = rng.next_u64() & (1 << 63);
                        let payload = 1 + below(rng, (1 << 52) - 1);
                        f64::from_bits(sign | (0x7ff << 52) | payload)
                    }
                };
                let at = below(rng, out.len() as u64 + 1) as usize;
                out.insert(at, v);
            }
            out
        }
    }
}

/// The canonical bytes of one sum: `round`'s result, then `round_ieee`'s bits.
fn encode_sum(out: &mut Vec<u8>, s: &ExactSum) {
    match s.round() {
        Ok(v) => {
            out.push(0);
            out.extend_from_slice(&v.get().to_bits().to_le_bytes());
        }
        Err(e) => {
            out.push(1);
            out.push(error_code(e));
        }
    }
    out.extend_from_slice(&s.round_ieee().to_bits().to_le_bytes());
}

/// The code of an error in the encoding.
fn error_code(e: NumError) -> u8 {
    match e {
        NumError::NotFinite => 3,
        NumError::OutsideDomain => 8,
        _ => 255,
    }
}

/// One term set's canonical bytes, summed four ways that must agree; `split` decides the chunks and
/// the order of their merges, the same for every order of the terms.
fn sum_case(terms: &[f64], split: Seed) -> Vec<u8> {
    let mut one_by_one = ExactSum::new();
    for &t in terms {
        one_by_one.add(t);
    }
    let mut slice = ExactSum::new();
    slice.add_slice(terms);
    // A stream whose length is not known in advance (`from_fn` gives no size hint), so the switch
    // to the large accumulator happens part-way.
    let mut next = terms.iter().copied();
    let stream: ExactSum = std::iter::from_fn(|| next.next()).collect();
    let mut rng = SplitMix64::new(split);
    let mut chunks: Vec<ExactSum> = Vec::new();
    let mut rest = terms;
    while !rest.is_empty() {
        let take = 1 + below(&mut rng, rest.len() as u64) as usize;
        let (head, tail) = rest.split_at(take);
        let mut c = ExactSum::new();
        if rng.coin() {
            c.add_slice(head);
        } else {
            c.add_iter(head.iter().copied());
        }
        chunks.push(c);
        rest = tail;
    }
    Orders::new(Seed(rng.next_u64())).shuffle(&mut chunks);
    let mut merged = ExactSum::new();
    for c in &chunks {
        merged.merge(c);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&(terms.len() as u64).to_le_bytes());
    encode_sum(&mut out, &one_by_one);
    for (way, s) in [("slice", &slice), ("stream", &stream), ("chunks", &merged)] {
        let mut other = Vec::new();
        other.extend_from_slice(&(terms.len() as u64).to_le_bytes());
        encode_sum(&mut other, s);
        assert_eq!(
            other,
            out,
            "ExactSum as a {way} differs from one by one on {} terms",
            terms.len()
        );
    }
    out
}

/// A double-double at every exponent, or a small one.
fn dd(rng: &mut SplitMix64) -> DD {
    loop {
        let field = if rng.coin() {
            below(rng, 2047)
        } else {
            960 + below(rng, 128)
        };
        let hi = with_field(rng, field);
        let lo = hi
            * f64::from_bits((1023 - 53 - below(rng, 20)) << 52)
            * if rng.coin() { 0.5 } else { -0.25 };
        if let Ok(d) = DD::from_sum(hi, lo) {
            return d;
        }
    }
}

/// The canonical bytes of a double-double result.
fn encode_dd(out: &mut Vec<u8>, r: Result<DD, NumError>) {
    match r {
        Ok(d) => {
            out.push(0);
            out.extend_from_slice(&d.hi().to_bits().to_le_bytes());
            out.extend_from_slice(&d.lo().to_bits().to_le_bytes());
        }
        Err(e) => {
            out.push(1);
            out.push(error_code(e));
        }
    }
}

/// One pair's canonical bytes, in the order given.
fn pair_case(xs: &[DD]) -> Vec<u8> {
    let (x, y) = (xs[0], xs[1]);
    let mut out = Vec::new();
    encode_dd(&mut out, x.add(y));
    encode_dd(&mut out, x.mul(y));
    let (s, e) = two_sum(x.hi(), y.hi());
    out.extend_from_slice(&s.to_bits().to_le_bytes());
    out.extend_from_slice(&e.to_bits().to_le_bytes());
    out
}

/// What the corpus reached, so a corpus that stopped testing a hard case fails.
#[derive(Default)]
struct Reached {
    sets: usize,
    terms: usize,
    long_sets: usize,
    not_finite: usize,
    nan: usize,
    zero: usize,
    dd_errors: usize,
}

/// The whole corpus: its bytes, and what it reached.
fn corpus() -> (Vec<u8>, Reached) {
    let mut rng = SplitMix64::new(Seed(SEED));
    let mut bytes = Vec::new();
    let mut reached = Reached::default();
    for index in 0..SETS {
        let terms = term_set(&mut rng);
        let split = Seed(rng.next_u64());
        let shuffle = Seed(rng.next_u64());
        let label = format!("ExactSum, term set {index} ({} terms)", terms.len());
        let digest = order_free(&label, &terms, shuffle, SHUFFLES, |ts| {
            golden::sha256(&sum_case(ts, split))
        })
        .unwrap_or_else(|e| panic!("{e}"));
        let case = sum_case(&terms, split);
        assert_eq!(golden::sha256(&case), digest);
        let round_ieee = f64::from_bits(u64::from_le_bytes(
            case[case.len() - 8..].try_into().unwrap(),
        ));
        reached.sets += 1;
        reached.terms += terms.len();
        reached.long_sets += usize::from(terms.len() > 2_047);
        reached.not_finite += usize::from(case[8] == 1);
        reached.nan += usize::from(round_ieee.is_nan());
        reached.zero += usize::from(round_ieee.to_bits() == 0);
        bytes.extend_from_slice(&case);
    }
    for index in 0..PAIRS {
        let pair = [dd(&mut rng), dd(&mut rng)];
        let shuffle = Seed(rng.next_u64());
        let label = format!("DD and two_sum, pair {index}");
        let digest = order_free(&label, &pair, shuffle, SHUFFLES, |xs| {
            golden::sha256(&pair_case(xs))
        })
        .unwrap_or_else(|e| panic!("{e}"));
        let case = pair_case(&pair);
        assert_eq!(golden::sha256(&case), digest);
        // `add`'s tag is first; `mul`'s follows `add`'s 16 bytes, or its error's code.
        let mul_tag = if case[0] == 0 { case[17] } else { case[2] };
        reached.dd_errors += usize::from(case[0] == 1 || mul_tag == 1);
        bytes.extend_from_slice(&case);
    }
    (bytes, reached)
}

/// The order-free golden: every order gives one answer, and the answers are the store's.
#[test]
fn order_free_contracts_give_one_answer_in_every_order() {
    let checked = golden::check(GOLDEN, || {
        let (bytes, reached) = corpus();
        let mut h = Sha256::new();
        h.write(&bytes);
        (h.finish(), (bytes.len(), reached))
    });
    let (len, r) = &checked.value;
    println!(
        "geom-num order-free corpus: {} term sets ({} terms, {} past the carry allowance), \
         {PAIRS} double-double pairs, {len} bytes; {} sums not finite, {} NaN, {} exact zero, \
         {} double-double errors",
        r.sets, r.terms, r.long_sets, r.not_finite, r.nan, r.zero, r.dd_errors
    );
    assert_eq!(r.sets, SETS);
    assert!(r.long_sets >= 3, "only {} long term sets", r.long_sets);
    assert!(r.not_finite >= 10, "only {} sums not finite", r.not_finite);
    assert!(r.nan >= 3, "only {} NaN sums", r.nan);
    assert!(r.zero >= 3, "only {} exact zeros", r.zero);
    assert!(
        r.dd_errors >= 5,
        "only {} double-double errors",
        r.dd_errors
    );
    checked.verdict().unwrap_or_else(|e| panic!("{e}"));
}

/// `Real` is what `round` returns; the encoding writes its value's bits.
#[test]
fn round_returns_a_real_whose_bits_the_encoding_writes() {
    let mut s = ExactSum::new();
    s.add(1.5);
    let mut out = Vec::new();
    encode_sum(&mut out, &s);
    assert_eq!(out[0], 0);
    assert_eq!(
        u64::from_le_bytes(out[1..9].try_into().unwrap()),
        Real::new(1.5).unwrap().get().to_bits()
    );
}
