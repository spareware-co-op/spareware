//! Inputs for the counter-based random numbers of `geom_num::rng`: the Philox4x32-10 block
//! function ([`philox4x32_10`]), a stream started at a block ([`stream_from`]), the five draws
//! ([`next_u32`], [`next_u64`], [`next_f64`], [`below`], [`range_inclusive`]) and the two ways a
//! stream range hands out streams ([`range_philox`], [`chunk`]).
//!
//! Every function there is a pure function of explicit integers, so its cost depends on the path
//! an input takes, never on the values drawn. Classes:
//!
//! - `typical`: seeds, stream ids, counters and keys drawn across all 64 (or 32) bits; streams part
//!   of the way along, at each of the four word places of a block in rotation, so that one
//!   `next_u32` in four computes the next block, as any long run of draws does; bounds and ranges
//!   of every size up to 2^40, where Lemire's method almost never rejects a word; stream ranges
//!   that are a whole seed's or a chunk of one, asked for a stream or a chunk inside them.
//! - `extreme`: the last draws a stream gives, each still succeeding (a stream in its last block,
//!   block 2^64 − 1, with from 1 to 4 words left, or from 2 for a 64-bit draw); for
//!   [`stream_from`], blocks, seeds and stream ids at 0 and 2^64 − 1; for [`below`] and
//!   [`range_inclusive`], in rotation, bounds just above 2^63, where Lemire's method rejects a word
//!   with probability close to one half, the bound 1, the bound 2^64 − 1, the whole of `u64` (one
//!   word, never rejected), a range of one value, and the empty bound 0 and empty range (low above
//!   high), refused; for [`chunk`], in rotation, the last chunk of a whole seed's range in chunks of
//!   2^63, 2^64 − 1 and 1 streams, and a chunk of no streams, refused.
//! - `overflow`: draws from streams with too few words left, refused with `Exhausted`; stream
//!   ranges asked for a stream or a chunk past their last stream, refused with `TooFewStreams`
//!   (including a first id plus an index, and an index times a size, past 2^64 − 1).
//!
//! Guarantees: every batch is full and the same on every target, and takes the paths listed
//! (`tests/inputs.rs` checks each claim); every result is checked against an independent
//! Philox4x32-10 and independent draw definitions by `tests/attack_g06.rs`.
//!
//! Method: boundary-value analysis (<https://en.wikipedia.org/wiki/Boundary-value_analysis>) on the
//! paths `geom_num::rng` documents: the block refill of `next_u32`, the end of a stream's 2^66
//! words, the rejection in Lemire's method (2019, <https://doi.org/10.1145/3230636>), whose
//! probability is (2^64 mod n) / 2^64, and the checked steps of a stream range.
//!
//! Cost: a few draws of [`SplitMix64`] per input, and at most four `next_u32` draws to place a
//! stream, once per batch, outside every measured run.

use geom_num::rng::{Philox, Seed, Stream, StreamId, StreamRange};

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// 2^63.
const HALF: u64 = 1 << 63;

/// A generator for a pseudo-random seed and stream id.
fn philox(g: &mut SplitMix64) -> Philox {
    Philox::new(Seed(g.next_u64()), StreamId(g.next_u64()))
}

/// `stream` after `words` more 32-bit draws (fewer if it runs out first).
fn advanced(mut stream: Stream, words: usize) -> Stream {
    for _ in 0..words {
        if stream.next_u32().is_err() {
            break;
        }
    }
    stream
}

/// A stream part of the way along: a pseudo-random block below 2^63, so far from the stream's end,
/// then `place` words into it (`place` < 4 is the word's place in its block).
fn mid_stream(g: &mut SplitMix64, place: usize) -> Stream {
    let block = g.next_u64() >> 1;
    advanced(philox(g).stream_from(block), place)
}

/// A stream with `left` words left (`left` ≤ 4): its last block, block 2^64 − 1, then 4 − `left`
/// words into it.
fn near_end(g: &mut SplitMix64, left: usize) -> Stream {
    advanced(
        philox(g).stream_from(u64::MAX),
        4_usize.saturating_sub(left),
    )
}

/// A bound from 2 up to about 2^40, its size spread over every power of two in between.
fn typical_bound(g: &mut SplitMix64) -> u64 {
    let bits = 1 + g.below(40);
    2 + (g.next_u64() >> (64 - bits))
}

/// A stream range: the whole of a seed's streams half the time, otherwise a chunk of fewer than
/// 2^20 streams at an offset below 2^40.
fn typical_range(g: &mut SplitMix64, whole: bool) -> Option<StreamRange> {
    let all = StreamRange::all(Seed(g.next_u64()));
    if whole {
        return Some(all);
    }
    let size = 1 + g.below(1 << 20);
    all.chunk(g.below(1 << 20), size).ok()
}

/// The stream range's number of streams, as a `u64` when it is below 2^64.
fn count(range: StreamRange) -> Option<u64> {
    u64::try_from(range.count()).ok()
}

/// (counter, key) pairs for `rng::philox4x32_10`.
#[must_use]
pub fn philox4x32_10(class: Class) -> Vec<([u32; 4], [u32; 2])> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("rng::philox4x32_10", class);
    let mut word = || (g.next_u64() >> 32) as u32;
    batch_of(|_| Some(([word(), word(), word(), word()], [word(), word()])))
}

/// (generator, block) pairs for `Philox::stream_from`.
#[must_use]
pub fn stream_from(class: Class) -> Vec<(Philox, u64)> {
    let mut g = rng("rng::stream_from", class);
    match class {
        Class::Typical => batch_of(|_| Some((philox(&mut g), g.next_u64()))),
        // Every combination of 0 and 2^64 − 1 for the seed, the stream id and the block, in turn.
        Class::Extreme => batch_of(|i| {
            let end = |bit: usize| if (i >> bit) & 1 == 0 { 0 } else { u64::MAX };
            Some((Philox::new(Seed(end(0)), StreamId(end(1))), end(2)))
        }),
        _ => Vec::new(),
    }
}

/// Streams for a draw of `words` 32-bit words (1 for `next_u32`, 2 for `next_u64` and `next_f64`):
/// typical streams at the four places of a block in rotation; the last draws a stream gives; and
/// streams with fewer than `words` words left.
fn draws(op: &str, class: Class, words: usize) -> Vec<Stream> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|i| Some(mid_stream(&mut g, i % 4))),
        // `words` to 4 words left, in rotation.
        Class::Extreme => batch_of(|i| Some(near_end(&mut g, words + i % (5 - words)))),
        // 0 to `words` − 1 words left, in rotation.
        Class::Overflow => batch_of(|i| Some(near_end(&mut g, i % words))),
        _ => Vec::new(),
    }
}

/// Streams for `Stream::next_u32`.
#[must_use]
pub fn next_u32(class: Class) -> Vec<Stream> {
    draws("rng::next_u32", class, 1)
}

/// Streams for `Stream::next_u64`.
#[must_use]
pub fn next_u64(class: Class) -> Vec<Stream> {
    draws("rng::next_u64", class, 2)
}

/// Streams for `Stream::next_f64`.
#[must_use]
pub fn next_f64(class: Class) -> Vec<Stream> {
    draws("rng::next_f64", class, 2)
}

/// (stream, bound) pairs for `Stream::below`.
#[must_use]
pub fn below(class: Class) -> Vec<(Stream, u64)> {
    let mut g = rng("rng::below", class);
    match class {
        Class::Typical => batch_of(|i| {
            let stream = mid_stream(&mut g, i % 4);
            Some((stream, typical_bound(&mut g)))
        }),
        Class::Extreme => batch_of(|i| {
            let stream = mid_stream(&mut g, i % 4);
            let bound = match i % 5 {
                // 2^64 mod (2^63 + k) = 2^63 − k: about half of all words are rejected.
                0 => HALF + 1,
                1 => HALF + 1 + g.below(1 << 32),
                2 => 1,
                3 => u64::MAX,
                // Empty: refused with `ZeroBound`.
                _ => 0,
            };
            Some((stream, bound))
        }),
        Class::Overflow => batch_of(|i| {
            let stream = near_end(&mut g, i % 2);
            Some((stream, typical_bound(&mut g)))
        }),
        _ => Vec::new(),
    }
}

/// (stream, low, high) triples for `Stream::range_inclusive`.
#[must_use]
pub fn range_inclusive(class: Class) -> Vec<(Stream, u64, u64)> {
    let mut g = rng("rng::range_inclusive", class);
    match class {
        Class::Typical => batch_of(|i| {
            let stream = mid_stream(&mut g, i % 4);
            let (low, high) = typical_span(&mut g);
            Some((stream, low, high))
        }),
        Class::Extreme => batch_of(|i| {
            let stream = mid_stream(&mut g, i % 4);
            let x = g.next_u64() >> 1;
            let (low, high) = match i % 5 {
                // The whole of `u64`: one word, never rejected.
                0 => (0, u64::MAX),
                // One value.
                1 => (x, x),
                // 2^63 + 1 values, from either end: about half of all words are rejected.
                2 => (x >> 1, (x >> 1) + HALF),
                3 => (u64::MAX - HALF, u64::MAX),
                // Empty: refused with `LowAboveHigh`.
                _ => (x + 1 + g.below(1 << 20), x),
            };
            Some((stream, low, high))
        }),
        Class::Overflow => batch_of(|i| {
            let stream = near_end(&mut g, i % 2);
            let (low, high) = typical_span(&mut g);
            Some((stream, low, high))
        }),
        _ => Vec::new(),
    }
}

/// A range of 1 to about 2^40 values from a low end below 2^63, so the high end fits.
fn typical_span(g: &mut SplitMix64) -> (u64, u64) {
    let low = g.next_u64() >> 1;
    (low, low + typical_bound(g) - 2)
}

/// (range, index) pairs for `StreamRange::philox`.
#[must_use]
pub fn range_philox(class: Class) -> Vec<(StreamRange, u64)> {
    let mut g = rng("rng::range_philox", class);
    match class {
        Class::Typical => batch_of(|i| {
            let range = typical_range(&mut g, i % 2 == 0)?;
            let index = match count(range) {
                Some(n) => g.below(n),
                None => g.next_u64(),
            };
            Some((range, index))
        }),
        Class::Overflow => batch_of(|i| {
            let range = typical_range(&mut g, false)?;
            let n = count(range)?;
            // One past the last stream and beyond, or so far that first + index passes 2^64 − 1.
            let index = if i % 2 == 0 {
                n + g.below(1 << 20)
            } else {
                u64::MAX - g.below(1 << 20)
            };
            Some((range, index))
        }),
        _ => Vec::new(),
    }
}

/// (range, index, size) triples for `StreamRange::chunk`.
#[must_use]
pub fn chunk(class: Class) -> Vec<(StreamRange, u64, u64)> {
    let mut g = rng("rng::chunk", class);
    match class {
        Class::Typical => batch_of(|i| {
            let range = typical_range(&mut g, i % 2 == 0)?;
            match count(range) {
                // The whole range: chunks of fewer than 2^20 streams, fewer than 2^20 chunks in.
                None => Some((range, g.below(1 << 20), 1 + g.below(1 << 20))),
                Some(n) => {
                    let size = 1 + g.below(n);
                    Some((range, g.below(n / size), size))
                }
            }
        }),
        Class::Extreme => batch_of(|i| {
            let all = StreamRange::all(Seed(g.next_u64()));
            Some(match i % 4 {
                // Chunk 1 of 2^63: streams 2^63 to 2^64 − 1, the last.
                0 => (all, 1, HALF),
                // Chunk 0 of 2^64 − 1: every stream but the last.
                1 => (all, 0, u64::MAX),
                // Chunk 2^64 − 1 of one stream: the last stream alone.
                2 => (all, u64::MAX, 1),
                // No streams: refused with `EmptyChunk`.
                _ => (all, g.next_u64(), 0),
            })
        }),
        Class::Overflow => batch_of(|i| {
            if i % 2 == 0 {
                // One chunk past the last that fits, or further.
                let range = typical_range(&mut g, false)?;
                let n = count(range)?;
                let size = 1 + g.below(n);
                Some((range, n / size + g.below(4), size))
            } else {
                // An index times a size past 2^64 − 1.
                let all = StreamRange::all(Seed(g.next_u64()));
                Some((all, 2 + g.below(1 << 20), HALF))
            }
        }),
        _ => Vec::new(),
    }
}
