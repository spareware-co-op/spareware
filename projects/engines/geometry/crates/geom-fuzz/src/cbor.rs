//! Fuzz target: `geom-format`'s strict CBOR decoder and its text form never panic, refuse within
//! their input, and convert both ways bit for bit.
//!
//! What it does: [`harness`] reads the fuzzer's bytes twice, as the native reader must read any
//! file (24.7: "is a fuzz target from its first commit").
//!
//! - **As CBOR.** [`check`] the bytes at depth [`DEPTH`]. A refusal must name an offset inside the
//!   input and be the same on a second run. An accepted input must give the same length from
//!   `item_len`, a text form, and back from that text exactly the same bytes.
//! - **As text,** when the bytes are UTF-8: if `from_text` accepts them, the bytes it gives must be
//!   accepted by the decoder, and their text form must read back to the same bytes.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included; nothing is decoded first.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming it, with the input in
//!   hexadecimal.
//!
//! Method: the round-trip and determinism laws of `geom-format`'s `cbor` module headers
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`, 24.3, 24.6 and 24.7), on
//! arbitrary bytes, the property-based fuzzing of 28.5
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: one decoder pass, one text form and one parse per input, each linear in the input's
//! length; the text form is at most about three times as long as the bytes.

use core::fmt::Write;

use geom_format::cbor::{CborError, check, from_text, item_len, to_text};
use geom_run::{Budget, WorkMeter};

use crate::Finding;

/// The nesting the harness allows: deep enough that random inputs nest and are refused past it.
pub const DEPTH: usize = 16;

/// A meter with no limit: the target fuzzes the rules, not the budget.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn hex(data: &[u8]) -> String {
    data.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// The laws on `data` read as CBOR.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn check_bytes(data: &[u8]) -> Result<(), Finding> {
    let verdict = check(data, DEPTH, &mut meter());
    if check(data, DEPTH, &mut meter()) != verdict {
        return Err(Finding::new(
            "the decoder's verdict is the same on every run",
            || hex(data),
        ));
    }
    match verdict {
        Err(CborError { offset, .. }) if offset > data.len() => Err(Finding::new(
            "a refusal names an offset inside the input",
            || format!("{} refused at {offset}", hex(data)),
        )),
        Err(_) => Ok(()),
        Ok(()) => {
            if item_len(data, DEPTH, &mut meter()) != Ok(data.len()) {
                return Err(Finding::new(
                    "item_len of an accepted item is its length",
                    || hex(data),
                ));
            }
            let text = to_text(data, DEPTH, &mut meter()).map_err(|e| {
                Finding::new("an accepted item has a text form", || {
                    format!("{}: {e}", hex(data))
                })
            })?;
            match from_text(&text, DEPTH, &mut meter()) {
                Ok(back) if back == data => Ok(()),
                other => Err(Finding::new(
                    "the text form reads back to the same bytes",
                    || format!("{}\n{text}\n{other:?}", hex(data)),
                )),
            }
        }
    }
}

/// The laws on `text`.
///
/// # Errors
///
/// A [`Finding`] naming the law `text` broke.
pub fn check_text(text: &str) -> Result<(), Finding> {
    let Ok(bytes) = from_text(text, DEPTH, &mut meter()) else {
        return Ok(());
    };
    if check(&bytes, DEPTH, &mut meter()).is_err() {
        return Err(Finding::new(
            "what from_text gives the decoder accepts",
            || format!("{text:?} gave {}", hex(&bytes)),
        ));
    }
    let written = to_text(&bytes, DEPTH, &mut meter()).map_err(|e| {
        Finding::new("what from_text gives has a text form", || {
            format!("{text:?}: {e}")
        })
    })?;
    if from_text(&written, DEPTH, &mut meter()).as_ref() != Ok(&bytes) {
        return Err(Finding::new(
            "the text form of parsed text reads back to the same bytes",
            || format!("{text:?} gave {}, written {written:?}", hex(&bytes)),
        ));
    }
    Ok(())
}

/// The fuzz harness: `data` as CBOR, then as text if it is UTF-8.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check_bytes(data)?;
    match core::str::from_utf8(data) {
        Ok(text) => check_text(text),
        Err(_) => Ok(()),
    }
}
