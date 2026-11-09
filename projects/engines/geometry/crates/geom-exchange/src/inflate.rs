//! Inflate: a raw DEFLATE stream (RFC 1951), as a zip entry holds it, read through a [`ByteSource`]
//! into the bytes it encodes, within a size budget and a ratio budget, so a zip bomb is refused with
//! a typed error before it is written out.
//!
//! What it does: [`inflate`] reads exactly `input_len` compressed bytes from a byte source (a zip
//! entry's compressed size, from its central directory), decodes them with `decode`, the decoder
//! ported from `miniz_oxide`, and returns the output; [`inflate_slice`] does the same for bytes in
//! memory. The output may be at most [`InflateBudget::limit`] bytes long, fixed before the first
//! byte is read: the size budget, or the ratio budget times `input_len` where that is smaller,
//! but never less than the grace size. The output grows in steps, never past that limit.
//!
//! **Why a zip bomb has nothing to act on.** A DEFLATE stream can expand about a thousand times (a
//! 258-byte match in two bits), and nested or overlapping zip entries multiply that. Here every
//! stream is held to its own limit while it is decoded: the decoder is never given room past it,
//! and a stream that needs more is refused with [`InflateErrorKind::TooLarge`] or
//! [`InflateErrorKind::Ratio`] at the input offset where it asked. The default ratio, 100, and
//! grace size, 100 KiB, are Apache POI's for the same attack (its minimum inflate ratio of 0.01
//! above a grace entry size); the default size budget is [`crate::xml`]'s document budget, 256 MiB.
//!
//! Guarantees:
//!
//! - **Refused with a typed error and a byte offset, never a panic.** Every failure is an
//!   [`InflateError`] naming its [`InflateErrorKind`] and the offset in the compressed bytes: of
//!   the byte holding the first bit the decoder had not used when it found the fault, for a
//!   malformed stream or a breached budget; `input_len` for a stream cut short; the first byte
//!   after the stream, for trailing bytes.
//! - **Every budget holds.** The output is never longer than the limit, and no allocation is made
//!   past it; an allocation the allocator refuses is [`InflateErrorKind::OutOfMemory`].
//! - **Exactly one stream.** A stream that has not ended after `input_len` bytes is
//!   [`InflateErrorKind::Truncated`]; one that ends before them is
//!   [`InflateErrorKind::TrailingBytes`], so a zip entry's bytes hide nothing after its stream.
//! - **The split never matters.** The output, or the error's kind and offset, are the same however
//!   the source splits its reads; only a source's own failure and a stop of the work meter depend
//!   on when they happen.
//! - **The same bytes as `miniz_oxide`.** Every stream it accepts gives exactly the bytes `miniz_oxide`
//!   0.9.1 gives, and every stream it refuses as malformed, `miniz_oxide` refuses too
//!   (`tests/inflate_differential.rs`).
//! - **Work is charged as it is done.** The meter is charged one unit for each compressed byte
//!   used and each byte written, after each step of at most 4 KiB of input and 64 KiB of output,
//!   so a cancellation is seen within a step.
//!
//! Method: RFC 1951's format, decoded by `decode`; the output grown by doubling up to a limit,
//! as `miniz_oxide`'s `decompress_to_vec_with_limit` grows its vector; the zip-bomb thresholds of
//! Apache POI's `ZipSecureFile`, with the ratio taken over the whole stream, whose compressed
//! length the caller knows, so the limit is fixed before decoding and no split can move it.
//!
//! - P. Deutsch, "DEFLATE Compressed Data Format Specification version 1.3", RFC 1951 (1996),
//!   <https://www.rfc-editor.org/rfc/rfc1951>.
//!
//! Consulted: `miniz_oxide`, MIT,
//!   <https://github.com/Frommi/miniz_oxide/blob/b28c237471f63efebe152bddd962dc57bbb531eb/miniz_oxide/src/inflate/mod.rs>,
//!   for `decompress_to_vec_inner`'s loop: the whole output given to each call as the match
//!   window, and the vector doubled up to the limit. No code copied.
//! Consulted: Apache POI, Apache-2.0,
//!   <https://github.com/apache/poi/blob/519e0a3c3945364e8ea75c172fe1a15fb6d4a202/poi-ooxml/src/main/java/org/apache/poi/openxml4j/util/ZipSecureFile.java>,
//!   for its zip-bomb thresholds: a minimum compressed-to-inflated ratio of 0.01, checked above a
//!   grace size of 100 KiB. No code copied.
//!
//! Cost: linear in the input and the output (see `decode`); memory, the output, grown by
//! doubling from 64 KiB to at most the limit, plus a 10 KiB decoder and a 4 KiB input buffer. No
//! recursion.

mod decode;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;

use geom_format::source::{ByteSource, SliceSource, SourceError};
use geom_run::WorkMeter;

use decode::{Decoder, Status};

/// The budgets a stream is decoded within.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InflateBudget {
    /// The most bytes a stream may decode to.
    pub output: usize,
    /// The most bytes a stream may decode to for each compressed byte, once past `grace`.
    pub ratio: u64,
    /// The bytes a stream may decode to whatever its ratio.
    pub grace: usize,
}

impl InflateBudget {
    /// 256 MiB of output, as [`crate::xml`]'s documents; at most 100 bytes for each compressed
    /// byte, beyond the first 100 KiB, as Apache POI allows. A caller lowers the size budget to an
    /// entry's declared size.
    pub const DEFAULT: InflateBudget = InflateBudget {
        output: 256 << 20,
        ratio: 100,
        grace: 100 << 10,
    };

    /// The most bytes a stream of `input_len` compressed bytes may decode to, and the error a
    /// stream that needs more is refused with: [`InflateErrorKind::Ratio`] when the ratio budget
    /// sets the limit, else [`InflateErrorKind::TooLarge`].
    #[must_use]
    pub fn limit(&self, input_len: u64) -> (usize, InflateErrorKind) {
        let by_ratio = usize::try_from(input_len.saturating_mul(self.ratio))
            .unwrap_or(usize::MAX)
            .max(self.grace);
        if by_ratio < self.output {
            (by_ratio, InflateErrorKind::Ratio)
        } else {
            (self.output, InflateErrorKind::TooLarge)
        }
    }
}

/// Why a stream was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InflateErrorKind {
    /// The byte source failed, or ended before `input_len` bytes.
    Source(SourceError),
    /// The stream had not ended after `input_len` bytes.
    Truncated,
    /// The stream ended before `input_len` bytes.
    TrailingBytes,
    /// A block of type 3, which RFC 1951 reserves.
    BlockType,
    /// A stored block whose length and its complement disagree.
    StoredLength,
    /// A dynamic block with more than 286 literal and length codes or more than 30 distance
    /// codes.
    TableSizes,
    /// Code lengths that over-subscribe a code, or leave one incomplete with codes longer than
    /// one bit (or the code-length code incomplete at all).
    CodeLengths,
    /// A repeat of the previous code length (16) with no length before it.
    RepeatFirst,
    /// Repeated code lengths that run past the literal and length and distance codes.
    RepeatOverrun,
    /// A literal and length symbol of 286 or 287, or bits no code starts with.
    Literal,
    /// A distance symbol of 30 or 31, or bits no distance code starts with.
    Distance,
    /// A match whose distance reaches before the first byte of output.
    DistanceTooFar,
    /// The output would pass the size budget.
    TooLarge,
    /// The output would pass the ratio budget times the input length.
    Ratio,
    /// The work meter stopped the read.
    Stopped,
    /// The allocator refused.
    OutOfMemory,
}

/// A refused stream: what, and the offset in the compressed bytes where it was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InflateError {
    /// What was refused.
    pub kind: InflateErrorKind,
    /// The byte offset in the compressed stream.
    pub offset: u64,
}

impl fmt::Display for InflateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DEFLATE stream refused at byte {}: {:?}",
            self.offset, self.kind
        )
    }
}

impl core::error::Error for InflateError {}

const fn refused(kind: InflateErrorKind, offset: u64) -> InflateError {
    InflateError { kind, offset }
}

/// Compressed bytes read from the source at a time.
const CHUNK: usize = 4_096;

/// The most output one decoder step writes, so the meter is charged at least this often.
const STEP_OUTPUT: usize = 64 << 10;

/// The output's first size; it doubles from there.
const FIRST_OUTPUT: usize = 64 << 10;

/// The byte holding the first bit the decoder has not used: of `used` bytes it was given, `held`
/// bits are still unread.
fn position(used: u64, held: u32) -> u64 {
    used.saturating_sub(u64::from(held.div_ceil(8)))
}

/// Decodes the raw DEFLATE stream that is exactly the next `input_len` bytes of `source`.
///
/// # Errors
///
/// [`InflateErrorKind::Source`] if the source fails or ends early; a malformed stream's kind;
/// [`InflateErrorKind::Truncated`] or [`InflateErrorKind::TrailingBytes`] if the stream does not
/// end at `input_len`; [`InflateErrorKind::TooLarge`] or [`InflateErrorKind::Ratio`] past the
/// limit of [`InflateBudget::limit`]; [`InflateErrorKind::Stopped`] if the meter stops;
/// [`InflateErrorKind::OutOfMemory`].
pub fn inflate<S: ByteSource + ?Sized>(
    source: &mut S,
    input_len: u64,
    budget: &InflateBudget,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<u8>, InflateError> {
    let (limit, breach) = budget.limit(input_len);
    let mut decoder = Box::<Decoder>::default();
    let mut out: Vec<u8> = Vec::new();
    let mut out_pos = 0_usize;
    let mut chunk = [0_u8; CHUNK];
    // The unused input is chunk[start..end]; `fed` bytes were read from the source and `used` of
    // them given to the decoder for good.
    let (mut start, mut end) = (0_usize, 0_usize);
    let mut fed = 0_u64;
    let mut used = 0_u64;
    loop {
        if start == end && fed < input_len {
            let want =
                usize::try_from(input_len.saturating_sub(fed)).map_or(CHUNK, |r| r.min(CHUNK));
            let Some(into) = chunk.get_mut(..want) else {
                return Err(refused(InflateErrorKind::Source(SourceError::Overrun), fed));
            };
            let n = source
                .read(into)
                .map_err(|e| refused(InflateErrorKind::Source(e), fed))?;
            if n == 0 {
                return Err(refused(
                    InflateErrorKind::Source(SourceError::UnexpectedEnd),
                    fed,
                ));
            }
            if n > want {
                return Err(refused(InflateErrorKind::Source(SourceError::Overrun), fed));
            }
            fed = fed.saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
            (start, end) = (0, n);
        }
        if out_pos == out.len() && out.len() < limit {
            let to = out.len().saturating_mul(2).max(FIRST_OUTPUT).min(limit);
            let more = to.saturating_sub(out.len());
            out.try_reserve_exact(more).map_err(|_| {
                refused(
                    InflateErrorKind::OutOfMemory,
                    position(used, decoder.held_bits()),
                )
            })?;
            out.resize(to, 0);
        }
        let window_end = out.len().min(out_pos.saturating_add(STEP_OUTPUT));
        // Both ranges hold: `start <= end <= CHUNK` and `window_end <= out.len()`.
        let (Some(input), Some(window)) = (chunk.get(start..end), out.get_mut(..window_end)) else {
            return Err(refused(
                InflateErrorKind::OutOfMemory,
                position(used, decoder.held_bits()),
            ));
        };
        let step = decoder.step(input, window, out_pos);
        start = start.saturating_add(step.consumed);
        out_pos = out_pos.saturating_add(step.written);
        let consumed = u64::try_from(step.consumed).unwrap_or(u64::MAX);
        used = used.saturating_add(consumed);
        let work = consumed.saturating_add(u64::try_from(step.written).unwrap_or(u64::MAX));
        meter.charge(work).map_err(|_| {
            refused(
                InflateErrorKind::Stopped,
                position(used, decoder.held_bits()),
            )
        })?;
        match step.status {
            Status::Done => {
                let stream_end = position(used, decoder.held_bits());
                if stream_end < input_len {
                    return Err(refused(InflateErrorKind::TrailingBytes, stream_end));
                }
                out.truncate(out_pos);
                return Ok(out);
            }
            Status::NeedsInput => {
                if fed == input_len {
                    return Err(refused(InflateErrorKind::Truncated, input_len));
                }
            }
            Status::OutputFull => {
                if out_pos == out.len() && out.len() >= limit {
                    return Err(refused(breach, position(used, decoder.held_bits())));
                }
            }
            Status::Failed(kind) => {
                return Err(refused(kind, position(used, decoder.held_bits())));
            }
        }
    }
}

/// Decodes `input`, which is exactly one raw DEFLATE stream, as [`inflate`] does from a source.
///
/// # Errors
///
/// As [`inflate`]; a slice never fails as a source.
pub fn inflate_slice(
    input: &[u8],
    budget: &InflateBudget,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<u8>, InflateError> {
    let len = u64::try_from(input.len()).unwrap_or(u64::MAX);
    inflate(&mut SliceSource::new(input), len, budget, meter)
}
