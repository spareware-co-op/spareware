//! The DEFLATE reader's fuzz target (piece G414b): `geom-exchange`'s inflate never panics, keeps
//! its budget, refuses inside its input, reads the same bytes the same however the source splits
//! them, and agrees with `miniz_oxide`, the crate its decoder is ported from, byte for byte.
//!
//! What it does: [`harness`] reads the fuzzer's bytes as one raw DEFLATE stream with
//! `inflate_slice` within [`LIMIT`] bytes of output, again through a source that hands out one
//! byte per read, and once more through `miniz_oxide` 0.9.1, which decodes the whole input in one
//! call into a buffer of [`LIMIT`] bytes. It checks: both reads give the same output or the same
//! error, kind and offset; an output fits the limit; an error's offset lies inside the input; and
//! the verdicts agree: the same output for a stream both accept, trailing bytes at the offset where
//! `miniz_oxide` stopped, a stream cut short or past the limit for both, and a malformed stream
//! for both.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included; nothing is decoded first.
//! - **Every law above is checked on every input**, and a broken one is a [`Finding`] naming it
//!   with the input in hexadecimal.
//!
//! Method: property-based fuzzing of a reader's laws, and differential testing against the
//! reader's origin (chapter 28.5, `projects/engines/geometry/architecture/28-testing.md`): the
//! guarantees of `geom-exchange`'s `inflate` module header
//! (`projects/engines/geometry/crates/geom-exchange/src/inflate.rs`, chapter 24.7,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`) as properties.
//!
//! Cost: three decodes of the input, each linear in the input and at most [`LIMIT`] bytes of
//! output; two buffers of [`LIMIT`] bytes.

use core::fmt::Write;

use geom_exchange::inflate::{
    InflateBudget, InflateError, InflateErrorKind, inflate, inflate_slice,
};
use geom_format::source::{ByteSource, SourceError};
use geom_run::{Budget, WorkMeter};
use miniz_oxide::inflate::TINFLStatus;
use miniz_oxide::inflate::core::{DecompressorOxide, decompress, inflate_flags};

use crate::Finding;

/// The most output a case may decode to: enough for long matches, small enough that a bomb is
/// refused quickly.
pub const LIMIT: usize = 1 << 16;

/// The budget every read is held to: the limit, with no ratio.
const BUDGET: InflateBudget = InflateBudget {
    output: LIMIT,
    ratio: u64::MAX,
    grace: 0,
};

/// A meter with no limit: the target fuzzes the decoder, not the meter.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn hex(data: &[u8]) -> String {
    data.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// A source that hands out one byte per read.
struct Trickle<'a> {
    rest: &'a [u8],
}

impl ByteSource for Trickle<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        match (self.rest.split_first(), buf.first_mut()) {
            (Some((b, rest)), Some(slot)) => {
                *slot = *b;
                self.rest = rest;
                Ok(1)
            }
            _ => Ok(0),
        }
    }
}

/// What `miniz_oxide` made of a stream.
#[derive(Debug)]
enum Oracle {
    /// Done, having used this many bytes, with this output.
    Done(usize, Vec<u8>),
    Truncated,
    TooLarge,
    Failed,
    /// A status a whole-input call never gives.
    Other,
}

fn oracle(data: &[u8]) -> Oracle {
    let mut out = vec![0; LIMIT];
    let mut r = Box::<DecompressorOxide>::default();
    let (status, used, written) = decompress(
        &mut r,
        data,
        &mut out,
        0,
        inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
    );
    match status {
        TINFLStatus::Done => {
            out.truncate(written);
            Oracle::Done(used, out)
        }
        TINFLStatus::FailedCannotMakeProgress => Oracle::Truncated,
        TINFLStatus::HasMoreOutput => Oracle::TooLarge,
        TINFLStatus::Failed => Oracle::Failed,
        _ => Oracle::Other,
    }
}

/// Whether a refusal is one of the malformed-stream kinds.
const fn malformed(kind: InflateErrorKind) -> bool {
    matches!(
        kind,
        InflateErrorKind::BlockType
            | InflateErrorKind::StoredLength
            | InflateErrorKind::TableSizes
            | InflateErrorKind::CodeLengths
            | InflateErrorKind::RepeatFirst
            | InflateErrorKind::RepeatOverrun
            | InflateErrorKind::Literal
            | InflateErrorKind::Distance
            | InflateErrorKind::DistanceTooFar
    )
}

/// Whether the reader's result is `miniz_oxide`'s verdict on the same bytes.
fn agrees(data: &[u8], ours: &Result<Vec<u8>, InflateError>, theirs: &Oracle) -> bool {
    let len = u64::try_from(data.len()).unwrap_or(u64::MAX);
    match (ours, theirs) {
        (Ok(out), Oracle::Done(used, want)) => *used == data.len() && out == want,
        (Err(e), Oracle::Done(used, _)) => {
            e.kind == InflateErrorKind::TrailingBytes
                && u64::try_from(*used).is_ok_and(|u| u == e.offset)
        }
        (Err(e), Oracle::Truncated) => e.kind == InflateErrorKind::Truncated && e.offset == len,
        (Err(e), Oracle::TooLarge) => e.kind == InflateErrorKind::TooLarge,
        (Err(e), Oracle::Failed) => malformed(e.kind),
        _ => false,
    }
}

/// The harness: see the module documentation.
///
/// # Errors
///
/// A [`Finding`] for the first law the input breaks.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    let len = u64::try_from(data.len()).unwrap_or(u64::MAX);
    let whole = inflate_slice(data, &BUDGET, &mut meter());
    let split = inflate(&mut Trickle { rest: data }, len, &BUDGET, &mut meter());
    if whole != split {
        return Err(Finding::new(
            "the same bytes read the same however they are split",
            || {
                format!(
                    "{} gave {whole:?} whole and {split:?} a byte at a time",
                    hex(data)
                )
            },
        ));
    }
    match &whole {
        Ok(out) if out.len() > LIMIT => {
            return Err(Finding::new("the output keeps its limit", || hex(data)));
        }
        Err(e) if e.offset > len => {
            return Err(Finding::new(
                "a refusal's offset lies inside the input",
                || format!("{e:?} for {}", hex(data)),
            ));
        }
        _ => {}
    }
    let theirs = oracle(data);
    if !agrees(data, &whole, &theirs) {
        return Err(Finding::new(
            "every stream is judged as miniz_oxide judges it, byte for byte",
            || format!("{}: ours {whole:?}, miniz_oxide {theirs:?}", hex(data)),
        ));
    }
    Ok(())
}
