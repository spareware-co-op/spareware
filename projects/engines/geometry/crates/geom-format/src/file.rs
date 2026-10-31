//! A standalone `.skgf` file: the label, the header record, the records and the END record (24.2),
//! written, read from any source however it splits its reads, and salvaged when damaged.
//!
//! What it does: [`write`] lays out RFC 9277's 12-byte label (`d9 d9 f8`, the protocol tag
//! `da 53 4b 47 46`, "SKGF", owner action G77, 32.18, and the byte string `43 42 4f 52`), the
//! header record, the records, and an END record whose payload is `[count, h'sha256']`: how many
//! records came before it, the header's included, and the SHA-256 of every byte before it, the label
//! included. [`read`] reads such a file from a [`ByteSource`] (or a [`RangeSource`] through
//! [`RangeCursor`]), record by record, and [`salvage`] keeps every intact record of damaged bytes
//! and names the damaged spans.
//!
//! Guarantees:
//!
//! - **The read does not depend on how the bytes arrive.** [`read`] takes every byte through
//!   `read_exact`, so a source that returns one byte a call gives the same result, error offsets
//!   included, as one that returns everything at once (the tests split reads at random).
//! - **Unknown records are handled as 24.2 says.** A record the caller's registry does not know
//!   stops the read with [`FileError::UnsupportedRecord`] (tag, schema, byte offset) if it is
//!   required, and is skipped and listed in [`ReadFile::skipped`] if it is optional.
//! - **Every length is checked before anything is allocated:** a string head longer than
//!   [`PAYLOAD_CAP`], or a record longer than [`RECORD_CAP`], is refused before its bytes are read.
//! - **The END record is checked**: its count and digest must match what was read, nothing may
//!   follow it, and a file without one is refused.
//! - **Salvage keeps what is intact**: every record that decodes with a matching check is kept with
//!   its offset, and each run of bytes between them is named as damaged.
//! - **No panic, no recursion**: nested items are counted with an explicit counter.
//!
//! Method: chapter 24.2 and 24.5 of the kernel architecture,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`; RFC 9277's labelled CBOR
//! Sequence, <https://www.rfc-editor.org/rfc/rfc9277>, and RFC 8742's CBOR Sequences,
//! <https://www.rfc-editor.org/rfc/rfc8742>; the item lengths of RFC 8949 section 3.
//!
//! Cost: linear in the file for [`write`] and [`read`], one SHA-256 pass over it; [`salvage`]
//! tries a record at each byte of a damaged span, so a span of `d` bytes costs up to `d` attempts,
//! each charged to the meter.

use alloc::vec::Vec;
use core::fmt;

use geom_op::cbor::{EncodeError, Encoder};
use geom_op::hash::Sha256;
use geom_run::{Stopped, WorkMeter};

use crate::cbor::{CborErrorKind, Decoder, Item};
use crate::record::{PAYLOAD_CAP, REQUIRED, Record, RecordError};
use crate::source::{ByteSource, RangeSource, SourceError, read_exact};

/// RFC 9277's label for SKG: tag 55800, the protocol tag 0x534B4746 ("SKGF"), and `h'424f52'`.
pub const LABEL: [u8; 12] = [
    0xd9, 0xd9, 0xf8, 0xda, 0x53, 0x4b, 0x47, 0x46, 0x43, 0x42, 0x4f, 0x52,
];

/// The record type of the header, the first record of every file (24.4). G93's registry records
/// it.
pub const HEADER_TAG: u64 = 1;
/// The record type of the END record, the last record of a standalone file (24.2).
pub const END_TAG: u64 = 2;

/// The largest record: the payload cap, and room for its envelope and `depends`.
pub const RECORD_CAP: u64 = PAYLOAD_CAP + (1 << 20);

/// Why a file could not be written or read.
#[derive(Clone, Debug, PartialEq)]
pub enum FileError {
    /// The source failed or ended early at this byte offset.
    Source {
        /// The offset of the read that failed.
        offset: u64,
        /// The source's error.
        error: SourceError,
    },
    /// The first 12 bytes are not SKG's label.
    Label,
    /// A head that no SKG record can hold (a reserved or indefinite length), or a string or record
    /// over its cap, at this offset.
    Malformed {
        /// The offset of the head.
        offset: u64,
    },
    /// The record at this offset is damaged or not a record.
    Record {
        /// The record's first byte.
        offset: u64,
        /// What is wrong with it.
        error: RecordError,
    },
    /// A required record the caller's registry does not know (24.2).
    UnsupportedRecord {
        /// Its type.
        tag: u64,
        /// Its schema.
        schema: u64,
        /// Its first byte.
        offset: u64,
    },
    /// The first record is not the header, or a header or END record is out of place.
    Misplaced {
        /// The record type found.
        tag: u64,
        /// Its first byte, or for writing its position in the list.
        offset: u64,
    },
    /// The END record's payload is not `[count, h'digest']`, or its count or digest differs.
    End,
    /// Bytes after the END record.
    TrailingBytes {
        /// The first byte past the END record.
        offset: u64,
    },
    /// The work meter stopped the read.
    Stopped(Stopped),
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileError::Source { offset, error } => write!(f, "at byte {offset}: {error}"),
            FileError::Label => f.write_str("not an SKG file: the label differs"),
            FileError::Malformed { offset } => write!(f, "at byte {offset}: no record holds this"),
            FileError::Record { offset, error } => write!(f, "record at byte {offset}: {error}"),
            FileError::UnsupportedRecord {
                tag,
                schema,
                offset,
            } => write!(
                f,
                "record at byte {offset}: required type {tag} schema {schema} is not supported"
            ),
            FileError::Misplaced { tag, offset } => {
                write!(f, "record type {tag} out of place at {offset}")
            }
            FileError::End => f.write_str("the END record does not match the file"),
            FileError::TrailingBytes { offset } => write!(f, "bytes after END, from {offset}"),
            FileError::Stopped(s) => write!(f, "stopped: {s:?}"),
        }
    }
}

impl core::error::Error for FileError {}

/// A file as read: its header, its records in order, and the optional records skipped.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadFile {
    /// The header record.
    pub header: Record,
    /// Every other record the caller knows, in file order.
    pub records: Vec<Record>,
    /// Each optional record skipped: its type, schema and first byte.
    pub skipped: Vec<(u64, u64, u64)>,
}

/// Damaged bytes, salvaged: every intact record with its offset, and the damaged spans.
#[derive(Clone, Debug, PartialEq)]
pub struct Salvage {
    /// Whether the bytes open with SKG's label.
    pub label: bool,
    /// Each intact record and its first byte.
    pub records: Vec<(u64, Record)>,
    /// Each damaged span, as its first byte and the byte after it.
    pub damaged: Vec<(u64, u64)>,
}

/// A [`RangeSource`] read in order from an offset, so [`read`] reads it too.
#[derive(Debug)]
pub struct RangeCursor<'a, S: RangeSource + ?Sized> {
    source: &'a S,
    at: u64,
}

impl<'a, S: RangeSource + ?Sized> RangeCursor<'a, S> {
    /// A cursor over `source` from `at`.
    pub fn new(source: &'a S, at: u64) -> Self {
        RangeCursor { source, at }
    }
}

impl<S: RangeSource + ?Sized> ByteSource for RangeCursor<'_, S> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        if self.at >= self.source.len() {
            return Ok(0);
        }
        let n = self.source.read_at(self.at, buf)?;
        let step = u64::try_from(n).map_err(|_| SourceError::Overrun)?;
        self.at = self.at.checked_add(step).ok_or(SourceError::Overrun)?;
        Ok(n)
    }
}

/// The bytes of a file holding `header`, then `records`, then the END record.
///
/// # Errors
///
/// [`FileError::Misplaced`] if `header` is not a header record or a record of `records` is a
/// header or END record, and [`FileError::Record`] for a record that has no encoding.
pub fn write(
    header: &Record,
    records: &[Record],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<u8>, FileError> {
    if header.tag != HEADER_TAG {
        return Err(FileError::Misplaced {
            tag: header.tag,
            offset: 0,
        });
    }
    let mut out = Vec::from(LABEL);
    let mut count: u64 = 0;
    for (position, record) in core::iter::once(header).chain(records).enumerate() {
        let offset = u64::try_from(position).unwrap_or(u64::MAX);
        if position > 0 && (record.tag == HEADER_TAG || record.tag == END_TAG) {
            return Err(FileError::Misplaced {
                tag: record.tag,
                offset,
            });
        }
        let bytes = record
            .encode(meter)
            .map_err(|error| FileError::Record { offset, error })?;
        out.extend_from_slice(&bytes);
        count = count.saturating_add(1);
    }
    let mut hasher = Sha256::new();
    hasher.write(&out);
    let end = Record {
        tag: END_TAG,
        flags: REQUIRED,
        schema: 1,
        depends: Vec::new(),
        payload: end_payload(count, &hasher.finish()).map_err(|e| FileError::Record {
            offset: count,
            error: RecordError::Encode(e),
        })?,
    };
    let bytes = end.encode(meter).map_err(|error| FileError::Record {
        offset: count,
        error,
    })?;
    out.extend_from_slice(&bytes);
    Ok(out)
}

/// END's payload: `[count, h'digest']`.
fn end_payload(count: u64, digest: &[u8; 32]) -> Result<Vec<u8>, EncodeError> {
    let mut e = Encoder::new();
    e.array(2)?;
    e.uint(count)?;
    e.bytes(digest)?;
    Ok(e.finish()?.into_bytes())
}

/// Reads a file from `source`, keeping the records `known(tag, schema)` accepts.
///
/// # Errors
///
/// A [`FileError`] at the first byte that does not hold: the label, a damaged record, an unknown
/// required record, a missing or mismatched END record, bytes after it, a failing source or the
/// meter.
pub fn read<S: ByteSource + ?Sized>(
    source: &mut S,
    known: impl Fn(u64, u64) -> bool,
    meter: &mut WorkMeter<'_>,
) -> Result<ReadFile, FileError> {
    let mut label = [0_u8; 12];
    read_exact(source, &mut label).map_err(|error| FileError::Source { offset: 0, error })?;
    if label != LABEL {
        return Err(FileError::Label);
    }
    let mut hasher = Sha256::new();
    hasher.write(&label);
    let mut offset: u64 = 12;
    let mut header = None;
    let mut records = Vec::new();
    let mut skipped = Vec::new();
    let mut count: u64 = 0;
    loop {
        let bytes = pull_record(source, offset, meter)?;
        let (record, _) =
            Record::decode(&bytes, meter).map_err(|error| stopped_or(offset, error))?;
        if record.tag == END_TAG {
            check_end(&record, count, hasher.finish())?;
            let after = offset.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
            let mut one = [0_u8; 1];
            return match source.read(&mut one) {
                Ok(0) => match header {
                    Some(header) => Ok(ReadFile {
                        header,
                        records,
                        skipped,
                    }),
                    None => Err(FileError::Misplaced {
                        tag: END_TAG,
                        offset,
                    }),
                },
                Ok(_) => Err(FileError::TrailingBytes { offset: after }),
                Err(error) => Err(FileError::Source {
                    offset: after,
                    error,
                }),
            };
        }
        let first = header.is_none();
        if first != (record.tag == HEADER_TAG) {
            return Err(FileError::Misplaced {
                tag: record.tag,
                offset,
            });
        }
        if first {
            header = Some(record);
        } else if known(record.tag, record.schema) {
            records.push(record);
        } else if record.is_required() {
            return Err(FileError::UnsupportedRecord {
                tag: record.tag,
                schema: record.schema,
                offset,
            });
        } else {
            skipped.push((record.tag, record.schema, offset));
        }
        hasher.write(&bytes);
        count = count.saturating_add(1);
        offset = offset.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
    }
}

/// A record error, or the meter's stop, which is not damage.
fn stopped_or(offset: u64, error: RecordError) -> FileError {
    match error {
        RecordError::Cbor(e) => match e.kind {
            CborErrorKind::Stopped(s) => FileError::Stopped(s),
            _ => FileError::Record {
                offset,
                error: RecordError::Cbor(e),
            },
        },
        error => FileError::Record { offset, error },
    }
}

/// Checks END's payload against the records read and their digest.
fn check_end(end: &Record, count: u64, digest: [u8; 32]) -> Result<(), FileError> {
    let mut meter = WorkMeter::new(geom_run::Budget::UNLIMITED);
    let mut d = Decoder::new(&end.payload, 1);
    let read = (d.next(&mut meter), d.next(&mut meter), d.next(&mut meter));
    match read {
        (Ok(Some(Item::Array(2))), Ok(Some(Item::Uint(n))), Ok(Some(Item::Bytes(b))))
            if n == count && b == digest =>
        {
            Ok(())
        }
        _ => Err(FileError::End),
    }
}

/// Reads one record's bytes from `source`, head by head, checking every length against its cap
/// before reading it. Items are counted, not recursed into.
fn pull_record<S: ByteSource + ?Sized>(
    source: &mut S,
    offset: u64,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<u8>, FileError> {
    let mut out = Vec::new();
    let mut pending: u64 = 1;
    while pending > 0 {
        pending = pending.saturating_sub(1);
        meter.charge(1).map_err(FileError::Stopped)?;
        let at = offset.saturating_add(u64::try_from(out.len()).unwrap_or(u64::MAX));
        let &[first] = take(source, &mut out, 1, at)? else {
            return Err(FileError::Malformed { offset: at });
        };
        let major = first >> 5;
        let info = first & 0x1f;
        let width = match info {
            0..=23 => 0,
            24 => 1,
            25 => 2,
            26 => 4,
            27 => 8,
            _ => return Err(FileError::Malformed { offset: at }),
        };
        let argument = if width == 0 {
            u64::from(info)
        } else {
            let bytes = take(source, &mut out, width, at)?;
            bytes.iter().fold(0_u64, |a, &b| (a << 8) | u64::from(b))
        };
        match major {
            2 | 3 => {
                if argument > PAYLOAD_CAP {
                    return Err(FileError::Malformed { offset: at });
                }
                let len =
                    usize::try_from(argument).map_err(|_| FileError::Malformed { offset: at })?;
                take(source, &mut out, len, at)?;
            }
            4 => pending = pending.saturating_add(argument),
            5 => pending = pending.saturating_add(argument.saturating_mul(2)),
            6 => pending = pending.saturating_add(1),
            _ => {}
        }
        if u64::try_from(out.len()).unwrap_or(u64::MAX) > RECORD_CAP {
            return Err(FileError::Malformed { offset });
        }
    }
    Ok(out)
}

/// Reads `len` more bytes from `source` onto the end of `out`, and returns them.
fn take<'o, S: ByteSource + ?Sized>(
    source: &mut S,
    out: &'o mut Vec<u8>,
    len: usize,
    at: u64,
) -> Result<&'o [u8], FileError> {
    let start = out.len();
    let end = start
        .checked_add(len)
        .ok_or(FileError::Malformed { offset: at })?;
    out.resize(end, 0);
    let tail = out
        .get_mut(start..)
        .ok_or(FileError::Malformed { offset: at })?;
    read_exact(source, tail).map_err(|error| FileError::Source { offset: at, error })?;
    Ok(out.get(start..).unwrap_or_default())
}

/// Every intact record of `bytes`, and the damaged spans between them (24.2's salvage read).
///
/// # Errors
///
/// [`FileError::Stopped`] if the meter stops; damage is never an error here.
pub fn salvage(bytes: &[u8], meter: &mut WorkMeter<'_>) -> Result<Salvage, FileError> {
    let label = bytes.starts_with(&LABEL);
    let mut at = if label { LABEL.len() } else { 0 };
    let mut records = Vec::new();
    let mut damaged = Vec::new();
    let mut damage_from: Option<usize> = None;
    while at < bytes.len() {
        let rest = bytes.get(at..).unwrap_or_default();
        if let Ok((record, len)) = Record::decode(rest, meter) {
            if let Some(from) = damage_from.take() {
                damaged.push((wide(from), wide(at)));
            }
            records.push((wide(at), record));
            at = at.saturating_add(len.max(1));
        } else {
            // A meter that stopped inside the decode stays stopped (WorkMeter latches its first
            // stop), so the charge below returns that stop: a stop is never counted as damage.
            damage_from.get_or_insert(at);
            meter.charge(1).map_err(FileError::Stopped)?;
            at = at.saturating_add(1);
        }
    }
    if let Some(from) = damage_from {
        damaged.push((wide(from), wide(bytes.len())));
    }
    Ok(Salvage {
        label,
        records,
        damaged,
    })
}

/// A byte offset as a `u64`.
fn wide(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}
