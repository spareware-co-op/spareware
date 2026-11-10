//! Canonical encodings of what an operation reports besides its bodies: exactness classes,
//! evidence, warnings and errors, so a result and a failure hash the same on every machine.
//!
//! What it does: [`encode_exactness`], [`encode_evidence`], [`encode_diagnostic`] and
//! [`encode_error`] write each in SKG's canonical CBOR (24.3), the parts chapter 28.13's canonical
//! output hashes: an output's exactness class and diagnostics, or an error's kind, entities and
//! evidence. `geom-test`'s canonical-output hasher combines them with the bodies, names and
//! history (piece G96).
//!
//! Guarantees:
//!
//! - **Values, never words.** A warning is its code, entities and evidence; an error its base
//!   kind, entities and evidence (28.13). Messages are plain words that may be reworded or
//!   translated, so they are never hashed, and two runs that fail the same way hash the same.
//! - **One encoding per value**: integers in the shortest form, doubles as the shortest finite
//!   float that holds them, lists in the order the operation gave them (a deterministic operation
//!   fixes it), and names as 16-byte strings.
//!
//! Method: chapter 28.13's canonical output and 24.3's canonical CBOR
//! (`projects/engines/geometry/architecture/28-testing.md`,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`).
//!
//! Cost: linear in what is encoded; one allocation for the encoding.

use crate::cbor::{EncodeError, Encoder, Item};
use crate::error::OpError;
use crate::evidence::{Evidence, Value};
use crate::id::NameId;
use crate::output::Diagnostic;
use geom_num::{Exactness, GridStep};

/// A length as CBOR counts it.
fn count(n: usize) -> Result<u64, EncodeError> {
    u64::try_from(n).map_err(|_| EncodeError::TooLong)
}

/// A grid step's code: `Q0` 0, `Q1` 1, `Q2` 2.
const fn step_code(step: GridStep) -> u64 {
    match step {
        GridStep::Q0 => 0,
        GridStep::Q1 => 1,
        GridStep::Q2 => 2,
    }
}

fn exactness(e: &mut Encoder, class: &Exactness) -> Result<(), EncodeError> {
    match class {
        Exactness::Exact => {
            e.array(1)?;
            e.uint(0)
        }
        Exactness::Certified { bound, step } => {
            e.array(3)?;
            e.uint(1)?;
            e.uint(bound.0)?;
            e.uint(step_code(*step))
        }
        Exactness::Approximate { deviation, step } => {
            e.array(3)?;
            e.uint(2)?;
            e.uint(deviation.0)?;
            e.uint(step_code(*step))
        }
    }
}

fn evidence(e: &mut Encoder, ev: &Evidence) -> Result<(), EncodeError> {
    e.array(2)?;
    e.text(ev.what)?;
    match &ev.value {
        Value::Point2(p) => {
            e.array(3)?;
            e.uint(0)?;
            e.int(i128::from(p.x.get()))?;
            e.int(i128::from(p.y.get()))
        }
        Value::Point3(p) => {
            e.array(4)?;
            e.uint(1)?;
            e.int(i128::from(p.x.get()))?;
            e.int(i128::from(p.y.get()))?;
            e.int(i128::from(p.z.get()))
        }
        Value::Coordinate(c) => {
            e.array(2)?;
            e.uint(2)?;
            e.int(*c)
        }
        Value::Length { steps, step } => {
            e.array(3)?;
            e.uint(3)?;
            e.uint(steps.0)?;
            e.uint(step_code(*step))
        }
        Value::Parameter(range) => {
            e.array(3)?;
            e.uint(4)?;
            e.float(range.lo())?;
            e.float(range.hi())
        }
        Value::Exactness(class) => {
            e.array(2)?;
            e.uint(5)?;
            exactness(e, class)
        }
        Value::Count(n) => {
            e.array(2)?;
            e.uint(6)?;
            e.int(i128::try_from(*n).map_err(|_| EncodeError::TooLong)?)
        }
    }
}

fn names(e: &mut Encoder, names: &[NameId]) -> Result<(), EncodeError> {
    e.array(count(names.len())?)?;
    for name in names {
        e.bytes(&name.to_bytes())?;
    }
    Ok(())
}

fn evidence_list(e: &mut Encoder, list: &[Evidence]) -> Result<(), EncodeError> {
    e.array(count(list.len())?)?;
    for ev in list {
        evidence(e, ev)?;
    }
    Ok(())
}

/// An exactness class: `[0]` exact, `[1, bound, step]` certified, `[2, deviation, step]`
/// approximate, a step `Q0` 0, `Q1` 1, `Q2` 2.
///
/// # Errors
///
/// The encoder's own error, which only a defect can cause.
pub fn encode_exactness(class: &Exactness) -> Result<Item, EncodeError> {
    let mut e = Encoder::new();
    exactness(&mut e, class)?;
    e.finish()
}

/// One named value: `[what, value]`, a value `[0, x, y]` (a 2D point), `[1, x, y, z]`, `[2,
/// coordinate]`, `[3, steps, step]` (a length), `[4, lo, hi]` (a parameter), `[5, class]` or `[6,
/// count]`.
///
/// # Errors
///
/// [`EncodeError`] for a count past `i128` or, as for the encoder, a defect.
pub fn encode_evidence(ev: &Evidence) -> Result<Item, EncodeError> {
    let mut e = Encoder::new();
    evidence(&mut e, ev)?;
    e.finish()
}

/// A warning: `[code, entities, evidence]`; its message is words and is left out.
///
/// # Errors
///
/// As [`encode_evidence`].
pub fn encode_diagnostic(d: &Diagnostic) -> Result<Item, EncodeError> {
    let mut e = Encoder::new();
    e.array(3)?;
    e.text(d.code)?;
    names(&mut e, &d.entities)?;
    evidence_list(&mut e, &d.at)?;
    e.finish()
}

/// An error: `[kind, entities, evidence]`, the kind by its name in chapter 22; its message and
/// remedies are left out (28.13).
///
/// # Errors
///
/// As [`encode_evidence`].
pub fn encode_error(error: &OpError) -> Result<Item, EncodeError> {
    let mut e = Encoder::new();
    e.array(3)?;
    e.text(error.kind().name())?;
    names(&mut e, error.entities())?;
    evidence_list(&mut e, error.evidence())?;
    e.finish()
}
