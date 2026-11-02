//! The pruned history record: the steps of one element's history a `PruneHistory` operation cut,
//! stored so that a reloaded model resolves every reference exactly as the pruned model did.
//!
//! What it does: [`pruned_record`] writes the steps one `PruneHistory` operation cut (24.4; piece
//! G109) as one required record (tag [`PRUNED_TAG`]): the operation's path, the set of names its
//! references held, and each step's path and kept history in the history record's own encoding
//! ([`History::encode`]), oldest first. [`read_pruned`] reads one back; the program makes each
//! step a `geom_name::resolve::Step::Kept` with the operation and the set.
//!
//! Guarantees:
//!
//! - **What is read is what was written**: the payload is read and encoded again, and the bytes
//!   must be the stored ones, so a set out of order, a step in another spelling, or any second
//!   spelling is refused; each step is read as a history record is, so its own forms hold.
//! - **The set is stored once**: every step of the record shares the operation and its set, so a
//!   file pays for the held names once per prune, not once per step.
//!
//! Method: chapter 24.4's `PruneHistory` and names and history records
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`), read by decoding and
//! encoding again, as the history record is.
//!
//! Cost: linear in the record; each step is decoded and encoded once, and the whole once more.

use alloc::vec::Vec;
use core::fmt;

use geom_op::cbor::{EncodeError, Encoder, Item};
use geom_op::name::encode_op_path;
use geom_op::{History, NameError, NameSet, OpPath};
use geom_run::WorkMeter;

use crate::history::{HistoryRecordError, read_history};
use crate::names::{NamesError, read_names_list, read_op_path};
use crate::payload::{PayloadError, Reader};
use crate::record::{REQUIRED, Record};

/// The pruned history record's tag (REGISTRY.md).
pub const PRUNED_TAG: u64 = 21;

/// The pruned history record's schema.
pub const PRUNED_SCHEMA: u64 = 1;

/// Why a pruned history record cannot be read or written.
#[derive(Clone, Debug, PartialEq)]
pub enum PrunedError {
    /// The payload's shape or encoding is wrong.
    Payload(PayloadError),
    /// The operation's path or the held set is wrong.
    Names(NamesError),
    /// A step is not a history record's canonical payload.
    Step(HistoryRecordError),
    /// The payload is not the record's one canonical encoding.
    NotCanonical,
    /// The encoder failed; only a defect can cause it.
    Encode(EncodeError),
}

impl fmt::Display for PrunedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrunedError::Payload(e) => write!(f, "not a pruned history record: {e}"),
            PrunedError::Names(e) => write!(f, "not a pruned history record: {e}"),
            PrunedError::Step(e) => write!(f, "not a pruned step: {e}"),
            PrunedError::NotCanonical => {
                f.write_str("the pruned history is not in its canonical encoding")
            }
            PrunedError::Encode(e) => write!(f, "the pruned history could not be encoded: {e}"),
        }
    }
}

impl core::error::Error for PrunedError {}

impl From<PayloadError> for PrunedError {
    fn from(e: PayloadError) -> Self {
        PrunedError::Payload(e)
    }
}

impl From<NamesError> for PrunedError {
    fn from(e: NamesError) -> Self {
        PrunedError::Names(e)
    }
}

impl From<NameError> for PrunedError {
    fn from(e: NameError) -> Self {
        PrunedError::Names(NamesError::Name(e))
    }
}

impl From<HistoryRecordError> for PrunedError {
    fn from(e: HistoryRecordError) -> Self {
        PrunedError::Step(e)
    }
}

impl From<EncodeError> for PrunedError {
    fn from(e: EncodeError) -> Self {
        PrunedError::Encode(e)
    }
}

/// The record's canonical encoding: `[pruned-by, held, steps]`, `held` a set of names and each
/// step its history record's payload.
///
/// # Errors
///
/// [`PrunedError::Encode`] or [`PrunedError::Names`] with an encoding error, which only a defect
/// can cause.
pub fn encode_pruned(
    pruned_by: &OpPath,
    held: &NameSet,
    steps: &[(OpPath, History)],
) -> Result<Vec<u8>, PrunedError> {
    let mut e = Encoder::new();
    e.array(3)?;
    e.item(&encode_op_path(pruned_by)?)?;
    let mut names: Vec<Item> = Vec::new();
    for name in held.names() {
        let mut n = Encoder::new();
        n.bytes(&name.to_bytes())?;
        names.push(n.finish()?);
    }
    e.set(names)?;
    e.array(u64::try_from(steps.len()).map_err(|_| EncodeError::TooLong)?)?;
    for (by, history) in steps {
        e.item(&history.encode(by)?)?;
    }
    Ok(e.finish()?.into_bytes())
}

/// The pruned history record of the steps `steps` that the `PruneHistory` operation at
/// `pruned_by` cut, holding `held`.
///
/// # Errors
///
/// As [`encode_pruned`].
pub fn pruned_record(
    pruned_by: &OpPath,
    held: &NameSet,
    steps: &[(OpPath, History)],
) -> Result<Record, PrunedError> {
    Ok(Record {
        tag: PRUNED_TAG,
        flags: REQUIRED,
        schema: PRUNED_SCHEMA,
        depends: Vec::new(),
        payload: encode_pruned(pruned_by, held, steps)?,
    })
}

/// The steps a pruned history record holds: the `PruneHistory` operation's path, the set of names
/// its references held, and each step's path and kept history, oldest first.
pub type Pruned = (OpPath, NameSet, Vec<(OpPath, History)>);

/// Reads a pruned history record's payload.
///
/// # Errors
///
/// [`PrunedError`] for a broken shape, a step that is not a history record's canonical payload,
/// or bytes that are not the record's canonical encoding.
pub fn read_pruned(payload: &[u8], meter: &mut WorkMeter<'_>) -> Result<Pruned, PrunedError> {
    let mut r = Reader::new(payload);
    let offset = r.offset();
    if r.array(meter, "a pruned history array")? != 3 {
        return Err(PayloadError::Shape {
            offset,
            expected: "a pruned history of three items",
        }
        .into());
    }
    let pruned_by = read_op_path(&mut r, meter)?;
    let held = NameSet::new(read_names_list(&mut r, meter)?)?;
    let count = r.array(meter, "the pruned steps")?;
    let mut steps = Vec::new();
    for _ in 0..count {
        steps.push(read_history(r.item(meter)?, meter)?);
    }
    r.end()?;
    if encode_pruned(&pruned_by, &held, &steps)? != payload {
        return Err(PrunedError::NotCanonical);
    }
    Ok((pruned_by, held, steps))
}
