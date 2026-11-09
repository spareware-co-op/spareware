//! The zip reader's fuzz target (piece G414c): `geom-exchange`'s zip reader never panics, refuses
//! inside its input, keeps its budgets, reads the same archive and the same entries however the
//! source splits its reads, and every entry it hands back is the entry its headers declare.
//!
//! What it does: [`harness`] reads the fuzzer's bytes as a zip archive with `ZipArchive::read`
//! within [`BUDGET`], and again through a source that hands out one byte per read. For an archive
//! it accepts, it reads every entry both ways and holds every part-name check to its own rule. It
//! checks: both reads give the same archive or the same error, kind and offset, and the same
//! entries; every refusal's offset lies inside the input; an accepted archive holds at most the
//! budget's entries, under distinct names that escape no folder, each entry (its local header
//! included) before the central directory and sharing no byte with another, and declaring at most
//! the budget's bytes in all; and an entry read is exactly its declared size, with its declared
//! CRC-32.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included; nothing is decoded first.
//! - **Every law above is checked on every input**, and a broken one is a [`Finding`] naming it
//!   with the input in hexadecimal.
//!
//! Method: property-based fuzzing of a reader's laws (chapter 28.5,
//! `projects/engines/geometry/architecture/28-testing.md`): the guarantees of `geom-exchange`'s
//! `zip` module header (`projects/engines/geometry/crates/geom-exchange/src/zip.rs`, chapter 24.7,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`) as properties.
//!
//! Cost: two reads of the archive and of each entry, each linear in the input and at most the
//! budget's output; memory, the entries and their bytes.

use core::fmt::Write;

use geom_exchange::inflate::InflateBudget;
use geom_exchange::zip::{ZipArchive, ZipBudget, ZipError, part};
use geom_format::checksum::crc32;
use geom_format::source::{RangeSource, SourceError};
use geom_run::{Budget, WorkMeter};

use crate::Finding;

/// The budget every read is held to: small enough that a bomb is refused quickly.
pub const BUDGET: ZipBudget = ZipBudget {
    entries: 64,
    name: 256,
    total: 1 << 20,
    inflate: InflateBudget {
        output: 1 << 16,
        ratio: 100,
        grace: 1 << 12,
    },
};

/// A meter with no limit: the target fuzzes the reader, not the meter.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn hex(data: &[u8]) -> String {
    data.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// A source that hands out at most one byte per read.
struct Trickle<'a> {
    bytes: &'a [u8],
}

impl RangeSource for Trickle<'_> {
    fn len(&self) -> u64 {
        u64::try_from(self.bytes.len()).unwrap_or(u64::MAX)
    }

    fn read_at(&self, at: u64, buf: &mut [u8]) -> Result<usize, SourceError> {
        let at = usize::try_from(at).map_err(|_| SourceError::OutOfBounds)?;
        let rest = self.bytes.get(at..).ok_or(SourceError::OutOfBounds)?;
        match (rest.first(), buf.first_mut()) {
            (Some(b), Some(slot)) => {
                *slot = *b;
                Ok(1)
            }
            _ => Ok(0),
        }
    }
}

/// Every entry's bytes, read from `source`.
fn contents<S: RangeSource + ?Sized>(
    zip: &ZipArchive,
    source: &S,
) -> Vec<Result<Vec<u8>, ZipError>> {
    zip.entries()
        .iter()
        .map(|e| zip.read_entry(source, e, &BUDGET, &mut meter()))
        .collect()
}

/// The first law an accepted archive breaks, if any.
fn accepted(zip: &ZipArchive, len: u64) -> Option<&'static str> {
    let entries = zip.entries();
    if entries.len() > BUDGET.entries {
        return Some("an archive holds at most the budget's entries");
    }
    let mut total = 0_u64;
    let mut spans: Vec<(u64, u64)> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        if part::escapes(&e.name)
            || entries
                .get(..i)
                .is_some_and(|b| b.iter().any(|o| o.name == e.name))
        {
            return Some("names are distinct and escape no folder");
        }
        let end = e.data.checked_add(e.compressed);
        if e.data < e.header || end.is_none_or(|end| end > zip.directory()) || zip.directory() > len
        {
            return Some("every entry lies before the central directory");
        }
        spans.push((e.header, end.unwrap_or(u64::MAX)));
        total = total.saturating_add(e.size);
    }
    if total > BUDGET.total {
        return Some("the entries declare at most the budget's bytes in all");
    }
    spans.sort_unstable();
    if spans.windows(2).any(|w| matches!(w, [a, b] if b.0 < a.1)) {
        return Some("no two entries share a byte");
    }
    None
}

/// The harness: see the module documentation.
///
/// # Errors
///
/// A [`Finding`] for the first law the input breaks.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    let len = u64::try_from(data.len()).unwrap_or(u64::MAX);
    let trickle = Trickle { bytes: data };
    let whole = ZipArchive::read(data, &BUDGET, &mut meter());
    let split = ZipArchive::read(&trickle, &BUDGET, &mut meter());
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
    let zip = match whole {
        Ok(zip) => zip,
        Err(e) if e.offset > len => {
            return Err(Finding::new(
                "a refusal's offset lies inside the input",
                || format!("{e:?} for {}", hex(data)),
            ));
        }
        Err(_) => return Ok(()),
    };
    if let Some(law) = accepted(&zip, len) {
        return Err(Finding::new(law, || format!("{zip:?} from {}", hex(data))));
    }
    let read = contents(&zip, data);
    if read != contents(&zip, &trickle) {
        return Err(Finding::new(
            "an entry reads the same however its bytes are split",
            || hex(data),
        ));
    }
    for (e, got) in zip.entries().iter().zip(&read) {
        match got {
            Ok(bytes) => {
                if u64::try_from(bytes.len()).ok() != Some(e.size) || crc32(bytes) != e.crc32 {
                    return Err(Finding::new(
                        "an entry read is its declared size and CRC-32",
                        || format!("{e:?} from {}", hex(data)),
                    ));
                }
            }
            Err(err) if err.offset > len => {
                return Err(Finding::new(
                    "a refusal's offset lies inside the input",
                    || format!("{err:?} for {e:?} from {}", hex(data)),
                ));
            }
            Err(_) => {}
        }
    }
    // The part-name check answers for every archive, with a rule at an entry's header.
    if let Err(e) = zip.check_parts()
        && !zip.entries().iter().any(|entry| entry.header == e.offset)
    {
        return Err(Finding::new(
            "a part-name refusal names an entry's header",
            || format!("{e:?} from {}", hex(data)),
        ));
    }
    Ok(())
}
