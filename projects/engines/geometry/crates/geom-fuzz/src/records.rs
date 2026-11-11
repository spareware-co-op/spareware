//! Fuzz target: `geom-format`'s record layer reads any bytes the same way twice and however its
//! source splits them, refuses inside its input, and accepts only canonical records and files.
//!
//! What it does: [`harness`] reads the fuzzer's bytes as the record layer must read any file (24.7:
//! every reader is a fuzz target), with [`check_bytes`]: one record with `Record::decode`, a whole
//! file with `file::read`, from a slice and from a source that splits its reads by the input's own
//! bytes, and a damaged file with `file::salvage`. It then reads the same bytes as a recipe for
//! records ([`decode`]: up to [`MAX_RECORDS`] records, each a type, flags, a schema, `depends` and
//! a payload), writes them with `Record::encode` and `file::write`, and holds what it wrote to the
//! same laws with [`check_built`], so the paths behind a matching check run too.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming it, with the input in
//!   hexadecimal. The laws: the same verdict on a second read and through a split source; every
//!   refusal's offset inside the input; a record or file accepted is exactly the bytes its writer
//!   gives back; every record salvage keeps is the bytes at its offset, and every damaged span
//!   lies inside the input; whatever the writers accept reads back to what they were given, and
//!   writes again to the same bytes.
//!
//! Method: the laws of `geom-format`'s `record` and `file` module headers (chapter 24.2 and 24.5,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`), the property harness of
//! `geom-format`'s `tests/fuzz_records.rs` made a fuzz target, driven by coverage-guided fuzzing
//! (28.5, `projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: each read twice and once split, linear in the input; salvage tries a record at each byte
//! of a damaged span, all under a work meter of [`WORK`] units.

use std::collections::BTreeSet;

use geom_format::cbor::CborError;
use geom_format::content::ContentId;
use geom_format::file::{FileError, HEADER_TAG, ReadFile, read, salvage, write};
use geom_format::record::{Record, RecordError};
use geom_format::source::{ByteSource, SliceSource, SourceError};
use geom_op::cbor::Encoder;
use geom_run::{Budget, WorkMeter};
use geom_test::{ByteSource as Bytes, Entropy};

use crate::Finding;
use crate::cbor::hex;

/// The work each read may do, as `tests/fuzz_records.rs` allows it.
pub const WORK: u64 = 2_000_000;

/// The most records [`decode`] builds from one input.
pub const MAX_RECORDS: u8 = 4;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(WORK))
}

/// A source that hands out at most a few bytes a call, as many as the input's bytes say.
struct Split<'a> {
    rest: &'a [u8],
    sizes: Bytes<'a>,
}

impl ByteSource for Split<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        let most = usize::from(self.sizes.byte() % 7).saturating_add(1);
        let n = most.min(buf.len()).min(self.rest.len());
        let (head, tail) = self.rest.split_at(n);
        buf.get_mut(..n)
            .ok_or(SourceError::Overrun)?
            .copy_from_slice(head);
        self.rest = tail;
        Ok(n)
    }
}

fn law(law: &str, data: &[u8], what: impl FnOnce() -> String) -> Finding {
    Finding::new(law, || format!("{}: {}", hex(data), what()))
}

fn wide(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// The offset a record refusal names, inside the record.
fn record_offset(e: &RecordError) -> Option<usize> {
    match e {
        RecordError::Cbor(CborError { offset, .. }) | RecordError::Shape { offset, .. } => {
            Some(*offset)
        }
        _ => None,
    }
}

/// The last byte a file refusal names, from the start of the file.
fn file_offset(e: &FileError) -> Option<u64> {
    match e {
        FileError::Record { offset, error } => {
            Some(offset.saturating_add(record_offset(error).map_or(0, wide)))
        }
        FileError::Source { offset, .. }
        | FileError::Malformed { offset }
        | FileError::UnsupportedRecord { offset, .. }
        | FileError::Misplaced { offset, .. }
        | FileError::TrailingBytes { offset } => Some(*offset),
        _ => None,
    }
}

/// The laws on `data` read as one record, as a file and as a damaged file.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn check_bytes(data: &[u8]) -> Result<(), Finding> {
    let one = Record::decode(data, &mut meter());
    if Record::decode(data, &mut meter()) != one {
        return Err(law("a record reads the same twice", data, String::new));
    }
    match &one {
        Err(e) if record_offset(e).is_some_and(|at| at > data.len()) => {
            return Err(law("a record refusal is inside the input", data, || {
                format!("{e:?}")
            }));
        }
        Ok((record, len)) if record.encode(&mut meter()).ok().as_deref() != data.get(..*len) => {
            return Err(law("an accepted record is its own encoding", data, || {
                format!("{record:?}, {len} bytes")
            }));
        }
        _ => {}
    }
    let whole = read(&mut SliceSource::new(data), |_, _| true, &mut meter());
    let mut split = Split {
        rest: data,
        sizes: Bytes::new(data),
    };
    if read(&mut SliceSource::new(data), |_, _| true, &mut meter()) != whole
        || read(&mut split, |_, _| true, &mut meter()) != whole
    {
        return Err(law(
            "a file reads the same twice and however its reads split",
            data,
            || format!("{whole:?}"),
        ));
    }
    match &whole {
        Err(e) if file_offset(e).is_some_and(|at| at > wide(data.len())) => {
            return Err(law("a file refusal is inside the input", data, || {
                format!("{e:?}")
            }));
        }
        Ok(file)
            if write(&file.header, &file.records, &mut meter())
                .ok()
                .as_deref()
                != Some(data) =>
        {
            return Err(law("an accepted file is its own writing", data, || {
                format!("{file:?}")
            }));
        }
        _ => {}
    }
    check_salvage(data)
}

fn check_salvage(data: &[u8]) -> Result<(), Finding> {
    let saved = salvage(data, &mut meter());
    if salvage(data, &mut meter()) != saved {
        return Err(law("salvage is the same twice", data, String::new));
    }
    let Ok(saved) = saved else { return Ok(()) };
    for (at, record) in &saved.records {
        let bytes = record.encode(&mut meter()).unwrap_or_default();
        let held = usize::try_from(*at)
            .ok()
            .and_then(|at| data.get(at..at.checked_add(bytes.len())?));
        if bytes.is_empty() || held != Some(bytes.as_slice()) {
            return Err(law(
                "a salvaged record is the bytes at its offset",
                data,
                || format!("{record:?} at {at}"),
            ));
        }
    }
    if let Some(span) = saved
        .damaged
        .iter()
        .find(|(from, to)| from >= to || *to > wide(data.len()))
    {
        return Err(law("a damaged span lies inside the input", data, || {
            format!("{span:?}")
        }));
    }
    Ok(())
}

/// Records built from `data`: the first is the header.
#[must_use]
pub fn decode(data: &[u8]) -> Vec<Record> {
    let mut src = Bytes::new(data);
    let count = (src.byte() % MAX_RECORDS).saturating_add(1);
    (0..count)
        .map(|i| {
            let tag = if i == 0 {
                HEADER_TAG
            } else {
                u64::from(src.byte() % 24)
            };
            let flags = u64::from(src.byte() % 8);
            let schema = u64::from(src.byte() % 3);
            let depends = (0..src.byte() % 4)
                .map(|_| ContentId::of_payload(&[src.byte() % 4]))
                .collect();
            let raw: Vec<u8> = (0..src.byte() % 32).map(|_| src.byte()).collect();
            // An odd length byte wraps the bytes as one CBOR byte string, so most payloads are
            // canonical and the record behind them is written.
            let payload = if raw.len() % 2 == 1 {
                let mut e = Encoder::new();
                e.bytes(&raw)
                    .ok()
                    .and_then(|()| e.finish().ok())
                    .map_or(raw, geom_op::cbor::Item::into_bytes)
            } else {
                raw
            };
            Record {
                tag,
                flags,
                schema,
                depends,
                payload,
            }
        })
        .collect()
}

/// Whether `a` and `b` are one record: `depends` is a set, which the writer sorts.
fn same(a: &Record, b: &Record) -> bool {
    let set = |r: &Record| r.depends.iter().cloned().collect::<BTreeSet<_>>();
    (a.tag, a.flags, a.schema, &a.payload) == (b.tag, b.flags, b.schema, &b.payload)
        && set(a) == set(b)
}

/// The laws on what the writers make of `records`.
///
/// # Errors
///
/// A [`Finding`] naming the law the records broke.
pub fn check_built(data: &[u8], records: &[Record]) -> Result<(), Finding> {
    for record in records {
        let Ok(bytes) = record.encode(&mut meter()) else {
            continue;
        };
        match Record::decode(&bytes, &mut meter()) {
            Ok((back, len)) if len == bytes.len() && same(&back, record) => {}
            other => {
                return Err(law("a written record reads back as itself", data, || {
                    format!("{record:?} gave {other:?}")
                }));
            }
        }
    }
    let Some((header, rest)) = records.split_first() else {
        return Ok(());
    };
    let Ok(file) = write(header, rest, &mut meter()) else {
        return Ok(());
    };
    match read(&mut SliceSource::new(&file), |_, _| true, &mut meter()) {
        Ok(ReadFile {
            header: h, records, ..
        }) if same(&h, header)
            && records.len() == rest.len()
            && records.iter().zip(rest).all(|(a, b)| same(a, b)) => {}
        other => {
            return Err(law(
                "a written file reads back as its records",
                data,
                || format!("{} gave {other:?}", hex(&file)),
            ));
        }
    }
    check_bytes(&file)
}

/// The fuzz harness: `data` as bytes to read, then as records to write and read back.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check_bytes(data)?;
    check_built(data, &decode(data))
}
