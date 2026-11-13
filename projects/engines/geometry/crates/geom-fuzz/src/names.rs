//! Fuzz target: `geom-format`'s names, history, recipe and attributes records, read from any
//! payload, give the same answer twice, refuse inside their input, and accept only the one
//! canonical encoding of what they read.
//!
//! What it does: [`harness`] takes the fuzzer's first byte as a [`Reader`] and the rest as a
//! payload, and reads it with that reader: `read_names` under the header's `NameScheme`,
//! `read_history`, `read_recipe`, `read_attributes`, or `read_topo_name` for one name. The corpus
//! starts from payloads the writers made from the records of `geom-format`'s names and history
//! tests, so the fuzzer edits real records.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming it, with the input in
//!   hexadecimal. The laws (24.4 and 32.19, as the module headers state them): the same verdict
//!   twice; a refusal's offset inside the payload; and what is accepted, written again by its own
//!   writer, is exactly the payload read, since each reader reads by decoding and encoding again.
//!
//! Method: the laws of `geom-format`'s `names`, `history`, `recipe` and `attributes` module
//! headers (chapter 24.4, `projects/engines/geometry/architecture/24-file-format-exchange.md`, and
//! decision 32.19, `projects/engines/geometry/architecture/32-decisions.md`), driven by
//! coverage-guided fuzzing (28.5, `projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: two reads of the payload and one write, each linear in it.

use geom_format::attributes::{AttributesError, encode_attributes, read_attributes};
use geom_format::cbor::CborError;
use geom_format::header::NAME_SCHEME;
use geom_format::history::{HistoryRecordError, history_record, read_history};
use geom_format::names::{NamesError, names_record, read_names, read_topo_name};
use geom_format::payload::PayloadError;
use geom_format::recipe::{RecipeError, read_recipe, recipe_record};
use geom_run::{Budget, WorkMeter};

use crate::Finding;
use crate::cbor::hex;

/// Which reader a case's payload goes to: its first byte, modulo 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reader {
    /// `read_names`, under the header's `NameScheme`.
    Names,
    /// `read_history`.
    History,
    /// `read_recipe`.
    Recipe,
    /// `read_attributes`.
    Attributes,
    /// `read_topo_name`.
    TopoName,
}

/// The reader and payload of `data`.
#[must_use]
pub fn decode(data: &[u8]) -> (Reader, &[u8]) {
    let (first, payload) = data.split_first().map_or((0, data), |(f, rest)| (*f, rest));
    let reader = match first % 5 {
        0 => Reader::Names,
        1 => Reader::History,
        2 => Reader::Recipe,
        3 => Reader::Attributes,
        _ => Reader::TopoName,
    };
    (reader, payload)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn payload_offset(e: &PayloadError) -> usize {
    match e {
        PayloadError::Cbor(CborError { offset, .. }) | PayloadError::Shape { offset, .. } => {
            *offset
        }
    }
}

/// The offset a names refusal names, if any.
pub(crate) fn names_offset(e: &NamesError) -> Option<usize> {
    match e {
        NamesError::Payload(p) => Some(payload_offset(p)),
        NamesError::Order { offset }
        | NamesError::NotCanonical { offset }
        | NamesError::WrongId { offset, .. } => Some(*offset),
        _ => None,
    }
}

/// The offset a recipe refusal names, if any.
pub(crate) fn recipe_offset(e: &RecipeError) -> Option<usize> {
    match e {
        RecipeError::Payload(p) => Some(payload_offset(p)),
        RecipeError::Names(n) => names_offset(n),
        _ => None,
    }
}

/// What one reader made of a payload: the payload its writer gives back, or the refusal's
/// offset and words.
type Verdict = Result<Option<Vec<u8>>, (Option<usize>, String)>;

/// Reads `payload` with `reader`, and writes back what it read.
fn verdict(reader: Reader, payload: &[u8]) -> Verdict {
    let m = &mut meter();
    let err = |at: Option<usize>, e: &dyn core::fmt::Debug| (at, format!("{e:?}"));
    match reader {
        Reader::Names => match read_names(payload, NAME_SCHEME, m) {
            Ok(g) => Ok(names_record(&g).ok().map(|r| r.payload)),
            Err(e) => Err(err(names_offset(&e), &e)),
        },
        Reader::History => match read_history(payload, m) {
            Ok((op, h)) => Ok(history_record(&op, &h).ok().map(|r| r.payload)),
            Err(e) => {
                let at = match &e {
                    HistoryRecordError::Names(n) => names_offset(n),
                    _ => None,
                };
                Err(err(at, &e))
            }
        },
        Reader::Recipe => match read_recipe(payload, m) {
            Ok(r) => Ok(recipe_record(&r).ok().map(|r| r.payload)),
            Err(e) => Err(err(recipe_offset(&e), &e)),
        },
        Reader::Attributes => match read_attributes(payload, m) {
            Ok((d, a)) => Ok(encode_attributes(&d, &a).ok()),
            Err(e) => {
                let at = match &e {
                    AttributesError::Payload(p) => Some(payload_offset(p)),
                    _ => None,
                };
                Err(err(at, &e))
            }
        },
        Reader::TopoName => match read_topo_name(payload, m) {
            Ok(n) => Ok(n.encode().ok().map(geom_op::cbor::Item::into_bytes)),
            Err(e) => Err(err(names_offset(&e), &e)),
        },
    }
}

/// The fuzz harness: [`decode`], then the laws on the payload.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    let (reader, payload) = decode(data);
    let law = |law: &str, what: String| {
        Finding::new(law, || format!("{reader:?} {}: {what}", hex(payload)))
    };
    let first = verdict(reader, payload);
    if verdict(reader, payload) != first {
        return Err(law("a payload reads the same twice", format!("{first:?}")));
    }
    match first {
        Err((Some(at), e)) if at > payload.len() => Err(law(
            "a refusal is inside the payload",
            format!("{e} at {at}"),
        )),
        Ok(back) if back.as_deref() != Some(payload) => Err(law(
            "what is read is its writer's one encoding",
            format!("written back as {:?}", back.as_deref().map(hex)),
        )),
        _ => Ok(()),
    }
}
