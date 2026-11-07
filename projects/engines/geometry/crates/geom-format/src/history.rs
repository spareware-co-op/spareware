//! The history record: one operation's origins, deletions and topology changes (chapter 20.3),
//! stored so that a reloaded model resolves its references through exactly the history it had.
//!
//! What it does: [`history_record`] writes an operation's history as one required record (tag
//! [`HISTORY_TAG`]) whose payload is `geom-op`'s canonical encoding of it ([`History::encode`]):
//! the operation's path, its records in output-name order, its deleted inputs and its topology
//! changes. [`read_history`] reads one back.
//!
//! Guarantees:
//!
//! - **What is read is what was written**: a payload is read into an `OpPath` and a `History`,
//!   which is encoded again; the bytes must be the stored ones, so a record out of order, a set
//!   out of order, an unknown origin or change, or any second spelling is refused.
//! - **The history's own forms hold**: two records of one output, a modification from nothing or a
//!   merge of one are refused, as `History::new` refuses them.
//!
//! Method: chapter 20.3's records and 24.4's names and history record
//! (`projects/engines/geometry/architecture/20-naming-history.md`,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`), read by decoding and
//! encoding again, as the names record is.
//!
//! Cost: linear in the record; the history is decoded and encoded once.

use alloc::vec::Vec;
use core::fmt;

use geom_op::cbor::EncodeError;
use geom_op::{
    History, HistoryError, LoopChange, NameError, NameList, NameSet, OpPath, Origin,
    Record as HistoryEntry, TopologyChange, Vanish,
};
use geom_run::WorkMeter;

use crate::names::{NamesError, read_names_list, read_op_path, read_piece, read_role};
use crate::payload::{PayloadError, Reader};
use crate::record::{REQUIRED, Record};

/// The history record's tag (REGISTRY.md).
pub const HISTORY_TAG: u64 = 17;

/// The history record's schema.
pub const HISTORY_SCHEMA: u64 = 1;

/// Why a history record cannot be read or written.
#[derive(Clone, Debug, PartialEq)]
pub enum HistoryRecordError {
    /// A part read as names are (the path, a role, a piece key, a set) is wrong.
    Names(NamesError),
    /// The records break `History`'s own forms.
    History(HistoryError),
    /// The payload is not the history's one canonical encoding.
    NotCanonical,
}

impl fmt::Display for HistoryRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HistoryRecordError::Names(e) => write!(f, "not a history record: {e}"),
            HistoryRecordError::History(e) => write!(f, "not a history: {e}"),
            HistoryRecordError::NotCanonical => {
                f.write_str("the history is not in its canonical encoding")
            }
        }
    }
}

impl core::error::Error for HistoryRecordError {}

impl From<NamesError> for HistoryRecordError {
    fn from(e: NamesError) -> Self {
        HistoryRecordError::Names(e)
    }
}

impl From<PayloadError> for HistoryRecordError {
    fn from(e: PayloadError) -> Self {
        HistoryRecordError::Names(NamesError::Payload(e))
    }
}

impl From<NameError> for HistoryRecordError {
    fn from(e: NameError) -> Self {
        HistoryRecordError::Names(NamesError::Name(e))
    }
}

impl From<EncodeError> for HistoryRecordError {
    fn from(e: EncodeError) -> Self {
        HistoryRecordError::Names(NamesError::Encode(e))
    }
}

/// The history record of the operation at `op`.
///
/// # Errors
///
/// [`HistoryRecordError::Names`] with an encoding error, which only a defect can cause.
pub fn history_record(op: &OpPath, history: &History) -> Result<Record, HistoryRecordError> {
    Ok(Record {
        tag: HISTORY_TAG,
        flags: REQUIRED,
        schema: HISTORY_SCHEMA,
        depends: Vec::new(),
        payload: history.encode(op)?.into_bytes(),
    })
}

/// Reads a history record's payload: the operation's path and its history.
///
/// # Errors
///
/// [`HistoryRecordError`] for a broken shape, records `History` refuses, or bytes that are not the
/// history's canonical encoding.
pub fn read_history(
    payload: &[u8],
    meter: &mut WorkMeter<'_>,
) -> Result<(OpPath, History), HistoryRecordError> {
    let mut r = Reader::new(payload);
    let offset = r.offset();
    if r.array(meter, "a history array")? != 4 {
        return Err(PayloadError::Shape {
            offset,
            expected: "a history of four items",
        }
        .into());
    }
    let op = read_op_path(&mut r, meter)?;
    let count = r.array(meter, "the history's records")?;
    let mut entries = Vec::new();
    for _ in 0..count {
        let at = r.offset();
        if r.array(meter, "a record [output, origin]")? != 2 {
            return Err(PayloadError::Shape {
                offset: at,
                expected: "a record of two items",
            }
            .into());
        }
        let output = r.name(meter, "the record's output")?;
        entries.push(HistoryEntry {
            output,
            origin: read_origin(&mut r, meter)?,
        });
    }
    let deleted = NameSet::new(read_names_list(&mut r, meter)?)?;
    let count = r.array(meter, "the history's topology changes")?;
    let mut changes = Vec::new();
    for _ in 0..count {
        changes.push(read_change(&mut r, meter)?);
    }
    r.end()?;
    let history = History::new(entries, deleted, changes).map_err(HistoryRecordError::History)?;
    if history.encode(&op)?.as_bytes() != payload {
        return Err(HistoryRecordError::NotCanonical);
    }
    Ok((op, history))
}

/// Reads an origin (see `History::encode`).
fn read_origin(
    r: &mut Reader<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Origin, HistoryRecordError> {
    let offset = r.offset();
    let len = r.array(meter, "an origin")?;
    let origin = match (r.uint(meter, "an origin's kind")?, len) {
        (0, 1) => Origin::Unchanged,
        (1, 2) => Origin::Modified {
            from: NameSet::new(read_names_list(r, meter)?)?,
        },
        (2, 3) => Origin::Generated {
            from: NameSet::new(read_names_list(r, meter)?)?,
            role: read_role(r, meter)?,
        },
        (3, 3) => Origin::Split {
            of: r.name(meter, "the input split")?,
            piece: read_piece(r, meter)?,
        },
        (4, 2) => Origin::Merged {
            of: NameSet::new(read_names_list(r, meter)?)?,
        },
        _ => {
            return Err(PayloadError::Shape {
                offset,
                expected: "an origin 0 to 4 with its fields",
            }
            .into());
        }
    };
    Ok(origin)
}

/// Reads a topology change (see `History::encode`).
fn read_change(
    r: &mut Reader<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<TopologyChange, HistoryRecordError> {
    let offset = r.offset();
    let shape = PayloadError::Shape {
        offset,
        expected: "a topology change 0 to 8 with its fields",
    };
    let len = r.array(meter, "a topology change")?;
    let change = match (r.uint(meter, "a topology change's class")?, len) {
        (0, 3) => TopologyChange::VertexSplit {
            vertex: r.name(meter, "the vertex")?,
            edges: NameList::new(read_names_list(r, meter)?),
        },
        (1, 3) => TopologyChange::FaceVanished {
            face: r.name(meter, "the face")?,
            why: match r.uint(meter, "collapsed 0 or inside out 1")? {
                0 => Vanish::Collapsed,
                1 => Vanish::InsideOut,
                _ => return Err(shape.into()),
            },
        },
        (2, 3) => TopologyChange::SelfOverlapCut {
            face: r.name(meter, "the face")?,
            along: NameList::new(read_names_list(r, meter)?),
        },
        (3, 2) => TopologyChange::ClashTrimmed {
            faces: NameSet::new(read_names_list(r, meter)?)?,
        },
        (4, 2) => TopologyChange::OvershootRemoved {
            faces: NameSet::new(read_names_list(r, meter)?)?,
        },
        (5, 3) => TopologyChange::FacesMerged {
            faces: NameSet::new(read_names_list(r, meter)?)?,
            into: r.name(meter, "the merged face")?,
        },
        (6, 3) => TopologyChange::FaceAdded {
            face: r.name(meter, "the face")?,
            role: read_role(r, meter)?,
        },
        (7, 3) => TopologyChange::LoopChanged {
            face: r.name(meter, "the face")?,
            change: match r.uint(meter, "hole closed 0 or face split 1")? {
                0 => LoopChange::HoleClosed,
                1 => LoopChange::FaceSplit,
                _ => return Err(shape.into()),
            },
        },
        (8, 2) => TopologyChange::BodySplit {
            lumps: NameList::new(read_names_list(r, meter)?),
        },
        _ => return Err(shape.into()),
    };
    Ok(change)
}
