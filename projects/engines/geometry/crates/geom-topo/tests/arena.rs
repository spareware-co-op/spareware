//! The typed indices and the chunked columns, through the public interface: every guarantee of
//! `ix.rs` and `column.rs`, at the chunk boundaries and the limits of a 32-bit index.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "tests may use native arithmetic and casts"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::mem::size_of;

use common::ix;
use geom_topo::{Arena, CHUNK_ROWS, Column, Face, FaceIx, Ix, Kind, Memory, Region, TopoError};

type Col = Column<Face, u64>;

fn column(rows: u32) -> Col {
    let mut c = Col::new();
    c.extend((0..rows).map(|v| u64::from(v) * 3 + 1)).unwrap();
    c
}

fn hash_of<T: Hash>(t: &T) -> u64 {
    let mut h = DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

fn chunk_lengths<K, V>(c: &Column<K, V>) -> Vec<usize> {
    c.chunks().map(<[V]>::len).collect()
}

#[test]
fn an_index_is_four_bytes_and_so_is_an_absent_one() {
    assert_eq!(size_of::<FaceIx>(), 4);
    assert_eq!(size_of::<Option<FaceIx>>(), 4);
}

#[test]
fn an_index_holds_its_row_up_to_the_largest() {
    for row in [0, 1, 2, 4095, 4096, 1 << 20, u32::MAX - 2, u32::MAX - 1] {
        let i = FaceIx::new(row).unwrap();
        assert_eq!(i.row(), row);
        assert_eq!(i.to_usize(), row as usize);
        assert_eq!(FaceIx::from_usize(row as usize), Some(i));
    }
    assert_eq!(FaceIx::MAX_ROW, u32::MAX - 1);
    assert_eq!(FaceIx::new(u32::MAX), None);
    assert_eq!(FaceIx::from_usize(u32::MAX as usize), None);
    #[cfg(target_pointer_width = "64")]
    assert_eq!(FaceIx::from_usize(1 << 40), None);
}

#[test]
fn indices_compare_and_hash_as_their_rows() {
    let rows = [7u32, 0, 4096, 3, u32::MAX - 1, 4095];
    let mut ixs: Vec<FaceIx> = rows.iter().map(|&r| ix(r)).collect();
    ixs.sort();
    let mut sorted = rows.to_vec();
    sorted.sort_unstable();
    assert_eq!(ixs.iter().map(|i| i.row()).collect::<Vec<_>>(), sorted);
    for a in rows {
        for b in rows {
            let (x, y): (FaceIx, FaceIx) = (ix(a), ix(b));
            assert_eq!(x == y, a == b);
            assert_eq!(x.cmp(&y), a.cmp(&b));
            assert_eq!(x.partial_cmp(&y), Some(a.cmp(&b)));
        }
        assert_eq!(hash_of(&ix::<Face>(a)), hash_of(&a));
    }
    let i: FaceIx = ix(5);
    let copied = i;
    #[allow(clippy::clone_on_copy, reason = "the clone itself is under test")]
    let cloned = i.clone();
    assert_eq!((copied, cloned), (i, i));
}

#[test]
fn an_index_prints_its_arena_and_row() {
    let i: FaceIx = ix(12);
    assert_eq!(format!("{i:?}"), "face#12");
    assert_eq!(format!("{i}"), "face 12");
    assert_eq!(format!("{:?}", ix::<Region>(0)), "region#0");
}

#[test]
fn every_arena_has_its_name() {
    let names: Vec<&str> = Arena::ALL.iter().map(|a| a.name()).collect();
    assert_eq!(
        names,
        [
            "region", "shell", "face", "loop", "fin", "edge", "vertex", "surface", "curve",
            "pcurve", "point"
        ]
    );
    for a in Arena::ALL {
        assert_eq!(a.to_string(), a.name());
    }
    let mut sorted = Arena::ALL;
    sorted.sort();
    assert_eq!(sorted, Arena::ALL, "ALL is in declaration order");
    assert_eq!(<Face as Kind>::ARENA, Arena::Face);
}

#[test]
fn an_empty_column_has_nothing() {
    let c = Col::new();
    assert_eq!(c.len(), 0);
    assert!(c.is_empty());
    assert_eq!(c.chunk_count(), 0);
    assert_eq!(c.get(ix(0)), None);
    assert_eq!(c.iter().count(), 0);
    assert_eq!(c.chunks().count(), 0);
    assert_eq!(Col::default(), c);
    assert_eq!(format!("{c:?}"), "[]");
    assert_eq!(c.memory(), Memory::default());
}

#[test]
fn rows_land_in_fixed_chunks_by_row_number() {
    for rows in [
        1,
        2,
        CHUNK_ROWS - 1,
        CHUNK_ROWS,
        CHUNK_ROWS + 1,
        3 * CHUNK_ROWS,
        3 * CHUNK_ROWS + 17,
    ] {
        for built in 0..3 {
            let c = match built {
                0 => column(rows),
                1 => {
                    let mut c = Col::new();
                    for v in 0..u64::from(rows) {
                        c.push(v * 3 + 1).unwrap();
                    }
                    c
                }
                _ => {
                    // In uneven runs: the chunk boundaries must not follow the runs.
                    let mut c = Col::new();
                    let mut v = 0u32;
                    let mut run = 1u32;
                    while v < rows {
                        let end = (v + run).min(rows);
                        c.extend((v..end).map(|v| u64::from(v) * 3 + 1)).unwrap();
                        v = end;
                        run = run * 7 % 5000 + 1;
                    }
                    c
                }
            };
            assert_eq!(c.len(), rows);
            assert!(!c.is_empty());
            let lengths = chunk_lengths(&c);
            assert_eq!(lengths.len() as u32, rows.div_ceil(CHUNK_ROWS));
            assert_eq!(c.chunk_count(), rows.div_ceil(CHUNK_ROWS));
            let (last, full) = lengths.split_last().unwrap();
            assert!(full.iter().all(|&n| n == CHUNK_ROWS as usize));
            assert_eq!(
                *last as u32,
                rows - CHUNK_ROWS * (rows.div_ceil(CHUNK_ROWS) - 1)
            );
            for r in [0, rows / 2, rows - 1] {
                assert_eq!(c.get(ix(r)), Some(&(u64::from(r) * 3 + 1)));
            }
            assert_eq!(c.get(ix(rows)), None);
            assert!(
                c.iter()
                    .copied()
                    .eq((0..u64::from(rows)).map(|v| v * 3 + 1))
            );
            assert_eq!(c, column(rows));
        }
    }
}

#[test]
fn chunk_n_starts_at_row_n_times_chunk_rows() {
    let c = column(2 * CHUNK_ROWS + 5);
    for (n, chunk) in c.chunks().enumerate() {
        assert_eq!(chunk[0], u64::from(n as u32 * CHUNK_ROWS) * 3 + 1);
    }
}

#[test]
fn set_changes_one_row_and_refuses_rows_past_the_end() {
    let mut c = column(CHUNK_ROWS + 3);
    for r in [0, CHUNK_ROWS - 1, CHUNK_ROWS, CHUNK_ROWS + 2] {
        c.set(ix(r), 99).unwrap();
        assert_eq!(c.get(ix(r)), Some(&99));
    }
    let missing = |row| TopoError::NoSuchRow {
        arena: Arena::Face,
        row,
    };
    let before = c.clone();
    // Past the end of the last chunk, and in a chunk that does not exist.
    assert_eq!(c.set(ix(CHUNK_ROWS + 3), 1), Err(missing(CHUNK_ROWS + 3)));
    assert_eq!(c.set(ix(2 * CHUNK_ROWS), 1), Err(missing(2 * CHUNK_ROWS)));
    assert_eq!(c.set(ix(u32::MAX - 1), 1), Err(missing(u32::MAX - 1)));
    assert_eq!(c, before);
    // A refused write copies nothing: the two still share every chunk.
    assert_eq!(c.memory().unique_chunks, 0);
    assert_eq!(before.memory().unique_chunks, 0);
    let mut empty = Col::new();
    assert_eq!(empty.set(ix(0), 1), Err(missing(0)));
}

#[test]
fn a_clone_never_sees_the_other_columns_writes() {
    let base = column(3 * CHUNK_ROWS);
    let mut edited = base.clone();
    assert_eq!(edited, base);
    edited.set(ix(5), 0).unwrap();
    edited.set(ix(2 * CHUNK_ROWS + 1), 0).unwrap();
    edited.push(7).unwrap();
    assert_eq!(base, column(3 * CHUNK_ROWS));
    assert_eq!(edited.get(ix(5)), Some(&0));
    assert_eq!(edited.len(), 3 * CHUNK_ROWS + 1);
    // And the other way: writes to the original leave the copy alone.
    let mut original = column(CHUNK_ROWS + 1);
    let copy = original.clone();
    original.set(ix(CHUNK_ROWS), 0).unwrap();
    original.push(1).unwrap();
    assert_eq!(copy, column(CHUNK_ROWS + 1));
}

#[test]
fn a_write_copies_only_the_chunk_it_lands_in() {
    let base = column(4 * CHUNK_ROWS);
    assert_eq!(base.memory().unique_chunks, 4);
    let mut edited = base.clone();
    assert_eq!(base.memory().unique_chunks, 0);
    assert_eq!(edited.memory().unique_chunks, 0);
    edited.set(ix(CHUNK_ROWS + 9), 0).unwrap();
    assert_eq!(edited.memory().unique_chunks, 1);
    assert_eq!(base.memory().unique_chunks, 1);
    // A second write to a chunk this column holds alone copies nothing more.
    edited.set(ix(CHUNK_ROWS + 10), 0).unwrap();
    assert_eq!(edited.memory().unique_chunks, 1);
    edited.set(ix(3 * CHUNK_ROWS), 0).unwrap();
    assert_eq!(edited.memory().unique_chunks, 2);
    drop(base);
    assert_eq!(edited.memory().unique_chunks, 4);
}

#[test]
fn a_push_onto_a_shared_last_chunk_copies_it_once() {
    let base = column(CHUNK_ROWS + 10);
    let mut edited = base.clone();
    edited.push(1).unwrap();
    assert_eq!(edited.memory().unique_chunks, 1);
    edited.push(2).unwrap();
    assert_eq!(edited.memory().unique_chunks, 1);
    assert_eq!(base.len(), CHUNK_ROWS + 10);
    assert_eq!(edited.len(), CHUNK_ROWS + 12);
    // Pushing past a full shared chunk starts a new chunk and copies nothing.
    let full = column(CHUNK_ROWS);
    let mut more = full.clone();
    more.push(1).unwrap();
    assert_eq!(more.memory().unique_chunks, 1);
    assert_eq!(full.memory().unique_chunks, 0);
}

#[test]
fn extend_onto_a_shared_last_chunk_copies_it_and_leaves_the_original() {
    let base = column(10);
    let mut edited = base.clone();
    edited.extend([100, 101, 102]).unwrap();
    assert_eq!(base, column(10));
    assert_eq!(edited.len(), 13);
    assert_eq!(edited.get(ix(12)), Some(&102));
    assert_eq!(edited.memory().unique_chunks, 1);
    assert_eq!(base.memory().unique_chunks, 1);
}

#[test]
fn extending_by_nothing_copies_nothing() {
    let base = column(10);
    let mut c = base.clone();
    c.extend(std::iter::empty()).unwrap();
    assert_eq!(c.memory().unique_chunks, 0);
    assert_eq!(c, base);
    let mut empty = Col::new();
    empty.extend(std::iter::empty()).unwrap();
    assert_eq!(empty.memory(), Memory::default());
}

#[test]
fn extend_reserves_exactly_and_push_doubles() {
    let size = size_of::<u64>() as u64;
    let header = (2 * size_of::<usize>() + size_of::<Vec<u64>>()) as u64;
    let pointer = size_of::<usize>() as u64;
    let mut exact = Col::new();
    exact.extend((0..5_u32).map(u64::from)).unwrap();
    assert_eq!(
        exact.memory(),
        Memory {
            chunks: 1,
            unique_chunks: 1,
            bytes: 5 * size + header + 4 * pointer,
            unique_bytes: 5 * size + header + 4 * pointer,
        }
    );
    let mut pushed = Col::new();
    for v in 0..5 {
        pushed.push(v).unwrap();
    }
    // Capacity 8 after doubling from 4; the list of chunks, like any vector's first reservation
    // of small values, took room for four pointers.
    assert_eq!(pushed.memory().bytes, 8 * size + header + 4 * pointer);
    // Exactly one chunk's rows in each full chunk, and the last reserved to its rows.
    let big = column(CHUNK_ROWS + 3);
    let m = big.memory();
    assert_eq!(m.chunks, 2);
    assert_eq!(
        m.bytes,
        (u64::from(CHUNK_ROWS) + 3) * size + 2 * header + 4 * pointer
    );
}

#[test]
fn extend_takes_only_the_rows_the_iterator_reports() {
    /// Says it holds `claimed` values and yields `actual`.
    struct Lying {
        claimed: usize,
        actual: std::ops::Range<u64>,
    }
    impl Iterator for Lying {
        type Item = u64;
        fn next(&mut self) -> Option<u64> {
            self.actual.next()
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.claimed, Some(self.claimed))
        }
    }
    impl ExactSizeIterator for Lying {}
    impl Lying {
        fn new(claimed: usize, actual: u64) -> Self {
            Lying {
                claimed,
                actual: 0..actual,
            }
        }
    }
    // Ends early in a new chunk, in a partly filled chunk, and across a chunk boundary.
    for (start, claimed, actual) in [
        (0u32, 5usize, 0u64),
        (0, 5, 3),
        (10, 5, 0),
        (10, 5000, 4500),
        (0, 10_000, 5000),
        (4096, 3, 0),
    ] {
        let mut c = column(start);
        c.extend(Lying::new(claimed, actual)).unwrap();
        let added = actual.min(claimed as u64);
        assert_eq!(u64::from(c.len()), u64::from(start) + added);
        let lengths = chunk_lengths(&c);
        if let Some((_, full)) = lengths.split_last() {
            assert!(full.iter().all(|&n| n == CHUNK_ROWS as usize));
        }
        assert!(lengths.iter().all(|&n| n > 0), "no empty chunk");
    }
    // Yields more than it says: only what it said is taken.
    let mut c = Col::new();
    c.extend(Lying::new(3, 10)).unwrap();
    assert_eq!(c.len(), 3);
    let mut c = column(4);
    c.extend(Lying::new(5000, 9000)).unwrap();
    assert_eq!(c.len(), 5004);
}

#[test]
fn filled_shares_one_chunk_until_written() {
    for rows in [0, 1, CHUNK_ROWS - 1, CHUNK_ROWS, 3 * CHUNK_ROWS + 2] {
        let c = Column::<Face, u8>::filled(7, rows).unwrap();
        assert_eq!(c.len(), rows);
        assert_eq!(c.chunk_count(), rows.div_ceil(CHUNK_ROWS));
        assert!(c.iter().all(|&v| v == 7));
        assert_eq!(c.iter().count() as u32, rows);
        let mut pushed = Column::<Face, u8>::new();
        for _ in 0..rows {
            pushed.push(7).unwrap();
        }
        assert_eq!(c, pushed);
    }
    let mut c = Column::<Face, u8>::filled(7, 3 * CHUNK_ROWS).unwrap();
    assert_eq!(c.memory().unique_chunks, 0, "three places, one chunk");
    c.set(ix(CHUNK_ROWS), 1).unwrap();
    assert_eq!(c.memory().unique_chunks, 1);
    assert_eq!(c.get(ix(0)), Some(&7));
    assert_eq!(c.get(ix(2 * CHUNK_ROWS)), Some(&7));
    assert_eq!(c.get(ix(CHUNK_ROWS)), Some(&1));
}

#[test]
fn a_full_column_refuses_one_more_row_and_leaves_itself_unchanged() {
    let mut c = Column::<Face, ()>::filled((), u32::MAX).unwrap();
    assert_eq!(c.len(), u32::MAX);
    assert_eq!(c.get(ix(u32::MAX - 1)), Some(&()));
    assert_eq!(c.push(()), Err(TopoError::Full(Arena::Face)));
    assert_eq!(c.extend([()]), Err(TopoError::Full(Arena::Face)));
    assert_eq!(c.len(), u32::MAX);
    c.extend(std::iter::empty()).unwrap();
    // One short of full: one more fits, two do not.
    let mut c = Column::<Face, ()>::filled((), u32::MAX - 1).unwrap();
    assert_eq!(c.extend([(), ()]), Err(TopoError::Full(Arena::Face)));
    assert_eq!(c.len(), u32::MAX - 1);
    let mut d = c.clone();
    assert_eq!(c.push(()).map(Ix::row), Ok(u32::MAX - 1));
    assert_eq!(c.len(), u32::MAX);
    d.extend([()]).unwrap();
    assert_eq!(d.len(), u32::MAX);
}

#[test]
fn push_returns_each_rows_index() {
    let mut c = Col::new();
    for r in 0..(CHUNK_ROWS + 2) {
        assert_eq!(c.push(u64::from(r)).unwrap().row(), r);
    }
}

#[test]
fn equality_is_by_value_whatever_is_shared() {
    let a = column(CHUNK_ROWS + 5);
    let b = column(CHUNK_ROWS + 5);
    assert_eq!(a, b);
    let mut c = a.clone();
    c.set(ix(3), 0).unwrap();
    assert_ne!(a, c);
    c.set(ix(3), 10).unwrap();
    assert_eq!(a, c, "the same values again, in different memory");
    assert_ne!(a, column(CHUNK_ROWS + 4));
    assert_eq!(format!("{:?}", column(3)), "[1, 4, 7]");
}

#[test]
fn memory_counts_shared_chunks_at_each_place() {
    let a = column(2 * CHUNK_ROWS);
    let b = a.clone();
    let chunk = u64::from(CHUNK_ROWS) * 8 + (2 * size_of::<usize>() + size_of::<Vec<u64>>()) as u64;
    let pointer = size_of::<usize>() as u64;
    // Each column's own list of chunk pointers: four reserved while extending, two in the clone.
    assert_eq!(a.memory().bytes, 2 * chunk + 4 * pointer);
    assert_eq!(b.memory().bytes, 2 * chunk + 2 * pointer);
    assert_eq!(a.memory().unique_bytes, 4 * pointer);
    assert_eq!(b.memory().unique_bytes, 2 * pointer);
    let sum = a.memory().plus(b.memory());
    assert_eq!(sum.chunks, 4);
    assert_eq!(sum.bytes, 4 * chunk + 6 * pointer);
    assert_eq!(
        Memory {
            chunks: u64::MAX,
            unique_chunks: 1,
            bytes: 2,
            unique_bytes: 3
        }
        .plus(Memory {
            chunks: 1,
            unique_chunks: 1,
            bytes: 2,
            unique_bytes: 3
        }),
        Memory {
            chunks: u64::MAX,
            unique_chunks: 2,
            bytes: 4,
            unique_bytes: 6
        }
    );
}
