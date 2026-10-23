//! Stream ranges of `geom_num::rng` (rule 3 of the randomised-method rule): how a method hands each
//! consumer and each nested sub-method streams that no other consumer uses.
//!
//! - **Against a model.** `StreamRange::chunk` and `StreamRange::philox` are compared with their
//!   written definitions computed in arbitrary-precision integers (`num-bigint`), so the model has
//!   no overflow of its own to get wrong: the same ranges, the same streams and the same errors,
//!   over generated ranges, indices and sizes that reach the ends of the 64-bit id space.
//! - **The promise itself.** Chunks of one size never overlap and lie inside their range, however
//!   deep the nesting; consumers given chunks in order get distinct streams, counted exhaustively
//!   on a small range; and a range too small is an error, never a wrap.
//! - **Compile time.** Ranges cut by the compiler match ranges cut at run time.
//!
//! Property tests run from a fixed seed and write no failure files, so they are the same on every
//! run and every target. The stream-range golden hash is in `rng_determinism.rs`.

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
// clippy.toml says tests may use these, and a broken model must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;

use geom_num::rng::{Philox, RngError, Seed, StreamId, StreamRange};
use num_bigint::BigUint;
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

/// Property tests from a fixed seed, with no failure files written.
fn config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        rng_seed: RngSeed::Fixed(0x4733_3252_414e_4745),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// A big integer.
fn big(v: u64) -> BigUint {
    BigUint::from(v)
}

/// A big integer back to 64 bits.
fn small(v: &BigUint) -> u64 {
    u64::try_from(v).unwrap()
}

/// The error for a request past the end of `range`.
fn too_few(range: StreamRange) -> RngError {
    RngError::TooFewStreams {
        first: range.first(),
        last: range.last(),
    }
}

/// Chunk `index` of size `size`, by its definition in arbitrary precision: ids
/// `first + index · size` to `first + index · size + size − 1`, which must not pass `last`.
fn model_chunk(range: StreamRange, index: u64, size: u64) -> Result<(u64, u64), RngError> {
    if size == 0 {
        return Err(RngError::EmptyChunk);
    }
    let start = big(range.first().0) + big(index) * big(size);
    let end = &start + big(size) - 1u32;
    if end > big(range.last().0) {
        return Err(too_few(range));
    }
    Ok((small(&start), small(&end)))
}

/// Stream `index`, by its definition: id `first + index`, which must not pass `last`.
fn model_stream(range: StreamRange, index: u64) -> Result<u64, RngError> {
    let id = big(range.first().0) + big(index);
    if id > big(range.last().0) {
        return Err(too_few(range));
    }
    Ok(small(&id))
}

/// A range's ends.
fn ends(range: StreamRange) -> (u64, u64) {
    (range.first().0, range.last().0)
}

/// Indices and sizes that reach every part of the 64-bit space.
fn word64() -> impl Strategy<Value = u64> {
    prop_oneof![
        0u64..8,
        (1u32..64).prop_map(|k| 1u64 << k),
        (1u32..64).prop_map(|k| (1u64 << k) - 1),
        (u64::MAX - 7)..=u64::MAX,
        any::<u64>(),
    ]
}

/// A range: the whole space of a seed, cut down by up to four chunks.
fn range() -> impl Strategy<Value = StreamRange> {
    (
        any::<u64>(),
        proptest::collection::vec((word64(), word64()), 0..4),
    )
        .prop_map(|(seed, cuts)| {
            let mut range = StreamRange::all(Seed(seed));
            for (index, size) in cuts {
                // A cut that does not fit is skipped, so every range here is a real one.
                if let Ok(chunk) = range.chunk(index, size) {
                    range = chunk;
                }
            }
            range
        })
}

#[test]
fn all_is_every_stream_of_its_seed() {
    let all = StreamRange::all(Seed(7));
    assert_eq!(all.seed(), Seed(7));
    assert_eq!(ends(all), (0, u64::MAX));
    assert_eq!(all.count(), 1u128 << 64);
    assert_eq!(all.philox(0), Ok(Philox::new(Seed(7), StreamId(0))));
    assert_eq!(
        all.philox(u64::MAX),
        Ok(Philox::new(Seed(7), StreamId(u64::MAX)))
    );
    // The whole space is one chunk of 2^64 − 1 streams plus one left over, or 2^64 chunks of one.
    assert_eq!(all.chunk(0, u64::MAX).map(ends), Ok((0, u64::MAX - 1)));
    assert_eq!(all.chunk(1, u64::MAX), Err(too_few(all)));
    assert_eq!(all.chunk(u64::MAX, 1).map(ends), Ok((u64::MAX, u64::MAX)));
    assert_eq!(
        all.chunk(1 << 32, 1 << 32),
        Err(too_few(all)),
        "2^32 · 2^32 = 2^64 is past the last id"
    );
    assert_eq!(
        all.chunk((1 << 32) - 1, 1 << 32).map(ends),
        Ok((u64::MAX - 0xffff_ffff, u64::MAX))
    );
}

/// The largest products: index and size both near 2^64 never wrap into a small id.
#[test]
fn huge_requests_are_errors_never_wraps() {
    let all = StreamRange::all(Seed(1));
    for (index, size) in [
        (u64::MAX, u64::MAX),
        (u64::MAX, 2),
        (2, u64::MAX),
        (1 << 63, 2),
        (4, 1 << 62),
    ] {
        assert_eq!(
            all.chunk(index, size),
            Err(too_few(all)),
            "chunk({index}, {size})"
        );
    }
    let one = all.chunk(5, 1).unwrap();
    assert_eq!(one.count(), 1);
    assert_eq!(one.philox(1), Err(too_few(one)));
    assert_eq!(one.philox(u64::MAX), Err(too_few(one)));
    assert_eq!(one.chunk(0, 2), Err(too_few(one)));
    assert_eq!(one.chunk(0, 0), Err(RngError::EmptyChunk));
}

/// Consumers given chunks in order get distinct streams, all inside the parent: every stream of
/// every chunk of 7 cut from a range of 100 streams, counted one by one. Fourteen chunks fit, the
/// fifteenth does not, and the two leftover streams belong to no chunk.
#[test]
fn consumers_given_chunks_get_distinct_streams() {
    let parent = StreamRange::all(Seed(3)).chunk(41, 100).unwrap();
    assert_eq!(ends(parent), (4_100, 4_199));
    let mut seen = BTreeSet::new();
    for i in 0..14 {
        let chunk = parent.chunk(i, 7).unwrap();
        for j in 0..7 {
            let id = chunk.philox(j).unwrap().stream_id().0;
            assert!(
                (4_100..=4_199).contains(&id),
                "stream {id} is outside its parent"
            );
            assert!(seen.insert(id), "stream {id} was handed out twice");
        }
        assert_eq!(chunk.philox(7), Err(too_few(chunk)));
    }
    assert_eq!(parent.chunk(14, 7), Err(too_few(parent)));
    assert_eq!(seen.len(), 98);
    let leftover: Vec<u64> = (4_100..=4_199).filter(|id| !seen.contains(id)).collect();
    assert_eq!(leftover, [4_198, 4_199]);
}

/// A sub-method numbering its consumers inside the chunk it was given never reaches the streams
/// of its caller's other consumers, at any depth: three levels of chunks, every stream counted.
#[test]
fn nested_methods_never_share_a_stream() {
    let operation = StreamRange::all(Seed(11)).chunk(0, 4 * 3 * 5).unwrap();
    let mut seen = BTreeSet::new();
    for a in 0..4 {
        let level1 = operation.chunk(a, 3 * 5).unwrap();
        for b in 0..3 {
            let level2 = level1.chunk(b, 5).unwrap();
            for c in 0..5 {
                let id = level2.philox(c).unwrap().stream_id().0;
                assert!(seen.insert(id), "stream {id} was handed out twice");
            }
        }
    }
    assert_eq!(seen.len(), 60);
    assert_eq!(
        seen.iter().copied().collect::<Vec<_>>(),
        (0..60).collect::<Vec<_>>()
    );
}

/// Ranges cut by the compiler give the same ranges and errors as at run time.
#[test]
fn ranges_cut_at_compile_time_match_run_time() {
    const ALL: StreamRange = StreamRange::all(Seed(0x5350_4152_454b_4954));
    const CHUNK: Result<StreamRange, RngError> = ALL.chunk(9, 1 << 20);
    const INNER: Result<StreamRange, RngError> = match CHUNK {
        Ok(c) => c.chunk(3, 1000),
        Err(e) => Err(e),
    };
    const STREAM: Result<Philox, RngError> = match INNER {
        Ok(c) => c.philox(999),
        Err(e) => Err(e),
    };
    const PAST: Result<Philox, RngError> = match INNER {
        Ok(c) => c.philox(1000),
        Err(e) => Err(e),
    };
    const COUNT: u128 = ALL.count();
    let all = StreamRange::all(Seed(0x5350_4152_454b_4954));
    let chunk = all.chunk(9, 1 << 20);
    let inner = chunk.and_then(|c| c.chunk(3, 1000));
    assert_eq!(CHUNK, chunk);
    assert_eq!(INNER, inner);
    assert_eq!(STREAM, inner.and_then(|c| c.philox(999)));
    assert_eq!(PAST, inner.and_then(|c| c.philox(1000)));
    assert_eq!(PAST, Err(too_few(inner.unwrap())));
    assert_eq!(COUNT, 1u128 << 64);
    assert_eq!(
        STREAM.map(Philox::stream_id),
        Ok(StreamId(9 * (1 << 20) + 3 * 1000 + 999))
    );
}

#[test]
fn range_errors_say_what_went_wrong() {
    assert_eq!(
        RngError::EmptyChunk.to_string(),
        "a chunk of no random streams was asked for"
    );
    assert_eq!(
        RngError::TooFewStreams {
            first: StreamId(4_100),
            last: StreamId(4_199)
        }
        .to_string(),
        "random streams 4100 to 4199 are too few for what was asked"
    );
}

proptest! {
    #![proptest_config(config(2048))]

    /// `chunk` is its definition: the same range or the same error as the arbitrary-precision
    /// model, the seed kept and the count equal to the size.
    #[test]
    fn chunk_is_its_definition(range in range(), index in word64(), size in word64()) {
        let got = range.chunk(index, size);
        prop_assert_eq!(got.map(ends), model_chunk(range, index, size));
        if let Ok(chunk) = got {
            prop_assert_eq!(chunk.seed(), range.seed());
            prop_assert_eq!(chunk.count(), u128::from(size));
        }
    }

    /// `philox` is its definition, and the same as a chunk of one.
    #[test]
    fn philox_is_its_definition_and_a_chunk_of_one(range in range(), index in word64()) {
        let got = range.philox(index);
        prop_assert_eq!(got.map(|p| p.stream_id().0), model_stream(range, index));
        if let Ok(p) = got {
            prop_assert_eq!(p.seed(), range.seed());
        }
        prop_assert_eq!(got, range.chunk(index, 1).and_then(|c| c.philox(0)));
    }

    /// `count` is the number of ids from first to last.
    #[test]
    fn count_is_last_minus_first_plus_one(range in range()) {
        let (first, last) = ends(range);
        prop_assert!(first <= last);
        prop_assert_eq!(range.count(), u128::from(last - first) + 1);
    }

    /// Two different chunks of one size never overlap, and both lie inside their range.
    #[test]
    fn chunks_of_one_size_never_overlap(
        range in range(),
        size in word64(),
        i in word64(),
        j in word64(),
    ) {
        let (first, last) = ends(range);
        if let (Ok(a), Ok(b)) = (range.chunk(i, size), range.chunk(j, size)) {
            for c in [a, b] {
                prop_assert!(first <= c.first().0 && c.last().0 <= last);
            }
            if i == j {
                prop_assert_eq!(a, b);
            } else {
                prop_assert!(
                    a.last().0 < b.first().0 || b.last().0 < a.first().0,
                    "chunks {} and {} of size {} overlap: {:?} and {:?}",
                    i,
                    j,
                    size,
                    ends(a),
                    ends(b)
                );
            }
        }
    }

    /// However deep the nesting, every chunk lies inside the range it was cut from.
    #[test]
    fn nested_chunks_stay_inside(
        seed in any::<u64>(),
        cuts in proptest::collection::vec((word64(), word64()), 1..8),
    ) {
        let mut range = StreamRange::all(Seed(seed));
        for (index, size) in cuts {
            match range.chunk(index, size) {
                Ok(chunk) => {
                    prop_assert!(range.first().0 <= chunk.first().0);
                    prop_assert!(chunk.last().0 <= range.last().0);
                    range = chunk;
                }
                Err(e) => {
                    let expected = if size == 0 { RngError::EmptyChunk } else { too_few(range) };
                    prop_assert_eq!(e, expected);
                }
            }
        }
    }
}
