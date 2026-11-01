//! The fuzz target of the sketch text form (piece G217): any bytes at all are refused cleanly or
//! read as a sketch that writes back to one fixed text.
//!
//! What it does: [`harness`] reads the fuzzer's bytes as text and hands them to
//! `geom_sketch::text::parse`. A text it accepts must write, read back equal, and write the same
//! text again.
//!
//! Guarantees:
//!
//! - **Never a panic.** `parse` returns a sketch or a typed refusal on every input.
//! - **Round trip.** What parses, writes; what it writes parses to the same sketch; and that writes
//!   the same text.
//!
//! Method: the property-based fuzzing of 28.5
//! (`projects/engines/geometry/architecture/28-testing.md`), on the grammar of section 15.13
//! (`projects/engines/geometry/architecture/15-sketches.md`).
//!
//! Cost: linear in the input, three parses and two writes at most.

use geom_sketch::text::{parse, write};

use crate::Finding;

/// The laws on `text`.
///
/// # Errors
///
/// A [`Finding`] naming the law `text` broke.
pub fn check_text(text: &str) -> Result<(), Finding> {
    let Ok(sketch) = parse(text) else {
        return Ok(());
    };
    let Ok(written) = write(&sketch) else {
        return Err(Finding::new("what parses, writes", || text.to_owned()));
    };
    let Ok(again) = parse(&written) else {
        return Err(Finding::new("what is written parses", || written.clone()));
    };
    if again != sketch {
        return Err(Finding::new("what is written reads back equal", || {
            written.clone()
        }));
    }
    match write(&again) {
        Ok(twice) if twice == written => Ok(()),
        _ => Err(Finding::new("the text is one fixed point", || {
            written.clone()
        })),
    }
}

/// The laws on `data`, read as text.
///
/// # Errors
///
/// As [`check_text`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check_text(&String::from_utf8_lossy(data))
}
