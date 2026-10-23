//! Tests of `geom_run::ChunkPlan`.
//!
//! What this file proves: chunk boundaries are those of `slice::chunks` for every length and chunk
//! size tried (including chunk sizes larger than the input and the empty input); they depend on
//! nothing but the length and the size; `chunk`, `chunks` and `count` agree; and no length, size or
//! index, up to `usize::MAX`, makes the arithmetic overflow.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their cases and oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test cases and oracles; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::plan;
use geom_run::{Chunk, ChunkPlan};

const LENGTHS: [usize; 13] = [0, 1, 2, 3, 7, 8, 9, 63, 64, 65, 100, 1000, 4097];
const SIZES: [usize; 9] = [1, 2, 3, 7, 8, 64, 1000, 4096, 5000];

#[test]
fn boundaries_are_those_of_slice_chunks() {
    for len in LENGTHS {
        let items: Vec<usize> = (0..len).collect();
        for size in SIZES {
            let plan = plan(size);
            let chunks: Vec<Chunk> = plan.chunks(len).collect();
            let expected: Vec<&[usize]> = items.chunks(size).collect();
            assert_eq!(chunks.len(), expected.len(), "len {len}, size {size}");
            assert_eq!(plan.count(len), expected.len());
            for (index, (chunk, slice)) in chunks.iter().zip(&expected).enumerate() {
                assert_eq!(chunk.index(), index);
                assert_eq!(&items[chunk.range()], *slice);
                assert_eq!(chunk.start(), slice[0]);
                assert_eq!(chunk.end(), chunk.start() + slice.len());
                assert_eq!(chunk.len(), slice.len());
                assert!(!chunk.is_empty());
                assert_eq!(plan.chunk(len, index), Some(*chunk));
            }
            assert_eq!(plan.chunk(len, plan.count(len)), None);
            assert_eq!(plan.chunk(len, usize::MAX), None);
        }
    }
}

#[test]
fn a_chunk_size_larger_than_the_input_gives_one_chunk() {
    for len in [1, 2, 63, 999] {
        let plan = plan(1000);
        assert_eq!(plan.count(len), 1);
        let only = plan.chunk(len, 0).expect("one chunk");
        assert_eq!((only.index(), only.range()), (0, 0..len));
    }
}

#[test]
fn the_empty_problem_has_no_chunks() {
    for size in SIZES {
        let plan = plan(size);
        assert_eq!(plan.count(0), 0);
        assert_eq!(plan.chunk(0, 0), None);
        assert_eq!(plan.chunks(0).next(), None);
    }
}

#[test]
fn extreme_lengths_sizes_and_indices_do_not_overflow() {
    let max = usize::MAX;
    assert_eq!(plan(1).count(max), max);
    let last = plan(1).chunk(max, max - 1).expect("last chunk");
    assert_eq!(last.range(), max - 1..max);
    assert_eq!(plan(1).chunk(max, max), None);

    let half = plan(2).chunk(max, max / 2).expect("short last chunk");
    assert_eq!((half.start(), half.end(), half.len()), (max - 1, max, 1));
    assert_eq!(plan(2).count(max), max / 2 + 1);

    assert_eq!(plan(max).count(max), 1);
    assert_eq!(plan(max).chunk(max, 0).map(Chunk::range), Some(0..max));
    // `1 · usize::MAX` fits but is past the end; `2 · usize::MAX` overflows: both are `None`.
    assert_eq!(plan(max).chunk(max, 1), None);
    assert_eq!(plan(max).chunk(max, 2), None);
    assert_eq!(plan(3).chunk(10, max), None);

    let quarter = plan(max / 4 + 1);
    let chunks: Vec<Chunk> = quarter.chunks(max).collect();
    assert_eq!(chunks.len(), 4);
    assert_eq!(chunks.last().map(|chunk| chunk.end()), Some(max));
    for pair in chunks.windows(2) {
        assert_eq!(pair[0].end(), pair[1].start());
    }
}

#[test]
fn the_chunk_iterator_is_exact_and_fused() {
    let plan = plan(7);
    let mut chunks = plan.chunks(50);
    assert_eq!(chunks.len(), 8);
    chunks.next();
    chunks.next();
    assert_eq!(chunks.size_hint(), (6, Some(6)));
    assert_eq!(chunks.by_ref().count(), 6);
    assert_eq!(chunks.next(), None);
    assert_eq!(chunks.next(), None);
    assert_eq!(chunks.len(), 0);
}

#[test]
fn fixed_plans_are_compile_time_constants() {
    const PLAN: ChunkPlan = ChunkPlan::fixed::<64>();
    const ONE: ChunkPlan = ChunkPlan::fixed::<1>();
    assert_eq!(PLAN.size().get(), 64);
    assert_eq!(PLAN, plan(64));
    assert_eq!(ONE.size().get(), 1);
}
