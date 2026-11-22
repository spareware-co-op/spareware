//! Fuzz target: `geom-p21`'s Part 21 reader never panics, refuses inside its input, reads the same
//! however its bytes arrive, and what it accepts writes to a fixed point.
//!
//! What it does: [`harness`] reads the fuzzer's bytes as an ISO 10303-21 file, as the reader must
//! read any file (24.7: "is a fuzz target from its first commit").
//!
//! - **Whole and in pieces.** The events and their records, or the error, read from the whole
//!   input must be the same on a second run and through a source that hands over one to eight
//!   bytes a call into a buffer of one to eight bytes. A refusal names an offset inside the input
//!   and a line no greater than its line feeds allow.
//! - **Strict and lossy text.** With lossy text the reader accepts whatever it accepts strictly,
//!   to the same file with nothing replaced.
//! - **A fixed point.** An accepted file writes canonically; the bytes written are ASCII, read
//!   back strictly with no repeated id, the same header and as many instances, and write to the
//!   same bytes again.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included; nothing is decoded first, except that
//!   the first byte also chooses how the second read is cut.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming it, with the input.
//!
//! Method: the reader and writer laws of `geom-p21`'s module headers
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`, 24.7 and 24.10), on
//! arbitrary bytes, the property-based fuzzing of 28.5
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: four reads and two writes of the input at most, each linear in its length.

use geom_format::source::{ByteSource, SliceSource, SourceError};
use geom_p21::{Error, Event, Exchange, Options, Reader, Record, Totals};
use geom_run::{Budget, WorkMeter};

use crate::Finding;

/// A meter with no limit: the target fuzzes the rules, not the budget.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A source that hands over at most `step` bytes a call.
pub struct Drip<'a> {
    /// The bytes not yet handed over.
    pub rest: &'a [u8],
    /// The most bytes a call hands over.
    pub step: usize,
}

impl ByteSource for Drip<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        let n = buf.len().min(self.rest.len()).min(self.step);
        let (head, tail) = self.rest.split_at(n);
        buf.get_mut(..n)
            .ok_or(SourceError::Overrun)?
            .copy_from_slice(head);
        self.rest = tail;
        Ok(n)
    }
}

/// Everything a read gives: each event with its records, and the error that ended it, if any.
pub type Trace = (Vec<(Event, Vec<Record>)>, Option<Error>);

/// Reads `source` to its end or its error.
pub fn trace<S: ByteSource>(source: S, options: Options) -> Trace {
    let mut meter = meter();
    let mut reader = Reader::new(source, options, &mut meter);
    let mut events = Vec::new();
    loop {
        match reader.next_event() {
            Ok(Some(event)) => events.push((event, reader.tokens().records())),
            Ok(None) => return (events, None),
            Err(e) => return (events, Some(e)),
        }
    }
}

fn shown(data: &[u8]) -> String {
    format!("{:?}", String::from_utf8_lossy(data))
}

/// The laws of the streaming reader on `data`, the second read cut as `cut` says.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn check_reader(data: &[u8], cut: u8) -> Result<(), Finding> {
    check_reader_with(data, cut, |s, o| trace(s, o), |s, o| trace(s, o))
}

/// [`check_reader`] on the reads handed in, so a test can feed it a broken reader.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn check_reader_with(
    data: &[u8],
    cut: u8,
    whole: impl Fn(SliceSource<'_>, Options) -> Trace,
    pieces: impl Fn(Drip<'_>, Options) -> Trace,
) -> Result<(), Finding> {
    let first = whole(SliceSource::new(data), Options::DEFAULT);
    if whole(SliceSource::new(data), Options::DEFAULT) != first {
        return Err(Finding::new(
            "the reader gives the same on every run",
            || shown(data),
        ));
    }
    if let Some(e) = first.1 {
        let length = u64::try_from(data.len()).unwrap_or(u64::MAX);
        let mut lines = 1_u64;
        for c in data {
            if *c == b'\n' {
                lines = lines.saturating_add(1);
            }
        }
        if e.offset > length || e.line == 0 || e.line > lines {
            return Err(Finding::new(
                "a refusal names a place inside the input",
                || format!("{}: {e}", shown(data)),
            ));
        }
    }
    let step = usize::from(cut & 7).saturating_add(1);
    let buffer_bytes = usize::from((cut >> 3) & 7).saturating_add(1);
    let options = Options {
        buffer_bytes,
        ..Options::DEFAULT
    };
    if pieces(Drip { rest: data, step }, options) != first {
        return Err(Finding::new("the split of the reads never matters", || {
            format!("{} in steps of {step}, buffer {buffer_bytes}", shown(data))
        }));
    }
    Ok(())
}

fn read(data: &[u8], lossy_text: bool) -> Result<Exchange, Error> {
    let options = Options {
        lossy_text,
        ..Options::DEFAULT
    };
    Exchange::read(
        SliceSource::new(data),
        options,
        Totals::DEFAULT,
        &mut meter(),
    )
}

/// The laws of the whole file and the canonical writer on `data`.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn check_exchange(data: &[u8]) -> Result<(), Finding> {
    let strict = read(data, false);
    let Ok(first) = read(data, true) else {
        return match strict {
            Ok(_) => Err(Finding::new(
                "lossy text accepts what strict text accepts",
                || shown(data),
            )),
            Err(_) => Ok(()),
        };
    };
    match &strict {
        Ok(strict) if *strict != first || first.replaced != 0 => {
            return Err(Finding::new(
                "strict and lossy text read the same file where both accept",
                || shown(data),
            ));
        }
        Err(_) if first.replaced == 0 => {
            return Err(Finding::new(
                "lossy text accepts more only by replacing",
                || shown(data),
            ));
        }
        _ => {}
    }
    let written = first.write(Vec::new()).map_err(|e| {
        Finding::new("an accepted file writes", || {
            format!("{}: {e}", shown(data))
        })
    })?;
    if !written.is_ascii() {
        return Err(Finding::new("the canonical form is ASCII", || shown(data)));
    }
    let second = read(&written, false).map_err(|e| {
        Finding::new("the canonical form reads back strictly", || {
            format!("{} wrote {}: {e}", shown(data), shown(&written))
        })
    })?;
    if !second.duplicates.is_empty()
        || second.entities.len() != first.entities.len()
        || second.header != first.header
    {
        return Err(Finding::new(
            "the canonical form holds the same header and instances",
            || format!("{} wrote {}", shown(data), shown(&written)),
        ));
    }
    match second.write(Vec::new()) {
        Ok(again) if again == written => Ok(()),
        other => Err(Finding::new(
            "write, read, write gives the same bytes",
            || format!("{} wrote {}, then {other:?}", shown(data), shown(&written)),
        )),
    }
}

/// The fuzz harness: `data` as a Part 21 file, its first byte also choosing the second read's
/// cut.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check_reader(data, data.first().copied().unwrap_or(0))?;
    check_exchange(data)
}
