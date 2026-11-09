//! The XML reader's fuzz target (piece G414a): `geom-exchange`'s hardened XML reader never panics,
//! refuses inside its input, keeps its budgets, nests its events, and reads the same bytes to the
//! same events however the source splits them.
//!
//! What it does: [`harness`] reads the fuzzer's bytes with `XmlText::read` under small budgets
//! ([`BUDGET`]), through a slice source and again through a source that hands out one byte per
//! call, and pulls every event. It checks: both reads give the same result, error or events; an
//! error's offset lies inside the input; every `Start` is closed by an `End` of the same name, at
//! most [`BUDGET`]`.depth` deep, and the whole document ends balanced; no element repeats an
//! attribute; and every value and text run fits its budget, and decodes to no more bytes than it
//! was written in.
//!
//! Guarantees:
//!
//! - **Every law above is checked on every input**, and a broken one is a [`Finding`] naming it
//!   with the input's bytes.
//!
//! Method: property-based fuzzing of a reader's laws (chapter 28.5,
//! `projects/engines/geometry/architecture/28-testing.md`): the reader's guarantees in its module
//! header (`projects/engines/geometry/crates/geom-exchange/src/xml.rs`, chapter 24.7,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`) as properties.
//!
//! Cost: two reads and two pulls of the input, linear in its length; the comparison of each
//! attribute with the earlier ones of its element, at most [`BUDGET`]`.attributes`.

use geom_exchange::xml::{Event, XmlBudget, XmlError, XmlName, XmlText};
use geom_format::source::{ByteSource, SliceSource, SourceError};
use geom_run::{Budget, WorkMeter};

use crate::Finding;

/// Small budgets, so random inputs reach every refusal.
pub const BUDGET: XmlBudget = XmlBudget {
    bytes: 4_096,
    depth: 8,
    attributes: 6,
    name: 24,
    value: 64,
    text: 128,
};

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

/// An event, owned, for comparing two reads.
#[derive(Debug, PartialEq, Eq)]
enum Seen {
    Start(String, String),
    Attribute(String, String, String),
    Open,
    End(String, String),
    Text(String),
    Other,
}

fn owned(name: XmlName<'_>) -> (String, String) {
    (String::from(name.prefix), String::from(name.local))
}

/// Reads `source` to its end: the events, or the first error.
fn pull(
    text: Result<XmlText, XmlError>,
    data: &[u8],
) -> Result<Result<Vec<Seen>, XmlError>, Finding> {
    let text = match text {
        Ok(t) => t,
        Err(e) => return Ok(Err(e)),
    };
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let mut events = text.events(BUDGET);
    let mut seen = Vec::new();
    let mut open: Vec<(String, String)> = Vec::new();
    let mut attributes: Vec<(String, String)> = Vec::new();
    let mut pending: Option<(String, String)> = None;
    loop {
        let event = match events.next(&mut meter) {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(e) => return Ok(Err(e)),
        };
        let law = |what: &str| Finding::new(what, || format!("{data:?}"));
        let item = match event {
            Event::Start(name) => {
                attributes.clear();
                pending = Some(owned(name));
                let (p, l) = owned(name);
                Seen::Start(p, l)
            }
            Event::Attribute { name, value } => {
                let key = owned(name);
                if attributes.contains(&key) {
                    return Err(law("no element repeats an attribute"));
                }
                if attributes.len() >= BUDGET.attributes || value.raw().len() > BUDGET.value {
                    return Err(law("attributes keep their budgets"));
                }
                let mut decoded = String::new();
                value
                    .decode_into(&mut decoded)
                    .map_err(|_| law("a checked value decodes"))?;
                if decoded.len() > value.raw().len() {
                    return Err(law(
                        "a value decodes to no more bytes than it was written in",
                    ));
                }
                attributes.push(key.clone());
                Seen::Attribute(key.0, key.1, decoded)
            }
            Event::Open => {
                let name = pending.take().ok_or_else(|| law("Open follows a Start"))?;
                open.push(name);
                if open.len() > BUDGET.depth {
                    return Err(law("elements nest within the depth budget"));
                }
                Seen::Open
            }
            Event::End(name) => {
                let name = owned(name);
                let closes = match pending.take() {
                    Some(p) => p == name,
                    None => open.pop() == Some(name.clone()),
                };
                if !closes {
                    return Err(law("every End closes the element its Start opened"));
                }
                Seen::End(name.0, name.1)
            }
            Event::Text(value) => {
                if value.raw().len() > BUDGET.text {
                    return Err(law("text keeps its budget"));
                }
                let mut decoded = String::new();
                value
                    .decode_into(&mut decoded)
                    .map_err(|_| law("checked text decodes"))?;
                if decoded.len() > value.raw().len() {
                    return Err(law("text decodes to no more bytes than it was written in"));
                }
                Seen::Text(decoded)
            }
            Event::Instruction { .. } | Event::Comment(_) => Seen::Other,
        };
        seen.push(item);
    }
    if !open.is_empty() || pending.is_some() {
        return Err(Finding::new("a document that reads ends balanced", || {
            format!("{data:?}")
        }));
    }
    Ok(Ok(seen))
}

/// The harness: see the module documentation.
///
/// # Errors
///
/// A [`Finding`] for the first law the input breaks.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    let whole = pull(XmlText::read(&mut SliceSource::new(data), &BUDGET), data)?;
    let split = pull(XmlText::read(&mut Trickle { rest: data }, &BUDGET), data)?;
    if whole != split {
        return Err(Finding::new(
            "the same bytes read to the same events however they are split",
            || format!("{data:?}"),
        ));
    }
    match whole {
        Err(e) if e.offset > data.len() => Err(Finding::new(
            "a refusal's offset lies inside the input",
            || format!("{e:?} for {data:?}"),
        )),
        _ => Ok(()),
    }
}
