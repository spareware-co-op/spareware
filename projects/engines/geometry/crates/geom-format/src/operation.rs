//! The operation record: one entry of an operation log, everything a replay needs to run the
//! operation again and get the same bits.
//!
//! What it does: an [`Operation`] is one authored operation (piece G96): its path (the permanent
//! `OpId` from the log, and labels), its name and version in the operation schemas, its inputs by
//! content ID and `NameId`, its parameters in SKG's canonical CBOR as its schema writes them, and
//! the work and logical-memory budget it runs under (23.10: a budget that can fail an operation
//! in a shared model is fixed by its version or recorded in the log). [`operation_record`] writes
//! one as a required record (tag [`OPERATION_TAG`]); [`read_operation`] reads it back.
//!
//! Guarantees:
//!
//! - **Versioned twice**: the record type has its schema, and every record names the operation's
//!   own version, which fixes its results, its charge schedule and its error details (26.2).
//! - **Parameters are canonical**: they are one item of SKG's canonical CBOR, checked on writing
//!   and on reading, so equal parameters are equal bytes and a log replays to the same bits.
//! - **One encoding**: a record is read and encoded again, and the bytes must be the stored ones.
//! - **No clock, no preview**: nothing timed and no precision is recorded; only final operations
//!   are logged, and a cancelled run writes nothing (10.1, 23.10).
//!
//! Method: chapter 21's replay and chapter 28.13's operation logs, with the recipe envelope's
//! inputs (24.4) (`projects/engines/geometry/architecture/21-determinism.md`,
//! `projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: linear in the record; the parameters are checked once and copied once.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use geom_op::OpPath;
use geom_op::cbor::{EncodeError, Encoder};
use geom_op::name::encode_op_path;
use geom_run::{Budget, WorkMeter};

use crate::cbor::{CborError, Item, check};
use crate::names::{NamesError, read_op_path};
use crate::payload::{PayloadError, Reader};
use crate::recipe::{RecipeError, RecipeInput, read_content_id};
use crate::record::{PAYLOAD_DEPTH, REQUIRED, Record};

/// The operation record's tag (REGISTRY.md).
pub const OPERATION_TAG: u64 = 20;

/// The operation record's schema.
pub const OPERATION_SCHEMA: u64 = 1;

/// One authored operation, as the log keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Operation {
    /// The operation's path: its permanent `OpId` and labels.
    pub op: OpPath,
    /// Its name in the operation schemas, printable ASCII: `combine.subtract`.
    pub name: String,
    /// Its version (26.2).
    pub version: u64,
    /// Its inputs, in its schema's order.
    pub inputs: Vec<RecipeInput>,
    /// Its parameters: one item of SKG's canonical CBOR, as its schema writes them.
    pub params: Vec<u8>,
    /// The budget it runs under.
    pub budget: Budget,
}

/// Why an operation record cannot be read or written.
#[derive(Clone, Debug, PartialEq)]
pub enum OperationError {
    /// The payload's shape or encoding is wrong.
    Payload(PayloadError),
    /// The path inside is wrong.
    Names(NamesError),
    /// An input is wrong.
    Input(RecipeError),
    /// An operation name that is empty or not printable ASCII.
    Name,
    /// Parameters that are not one item of SKG's canonical CBOR.
    Params(CborError),
    /// The payload is not the record's one canonical encoding.
    NotCanonical,
    /// The encoder failed; only a defect can cause it.
    Encode(EncodeError),
}

impl fmt::Display for OperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OperationError::Payload(e) => write!(f, "not an operation record: {e}"),
            OperationError::Names(e) => write!(f, "not an operation record: {e}"),
            OperationError::Input(e) => write!(f, "not an operation's input: {e}"),
            OperationError::Name => {
                f.write_str("an operation's name must be non-empty printable ASCII")
            }
            OperationError::Params(e) => {
                write!(f, "an operation's parameters are not canonical CBOR: {e}")
            }
            OperationError::NotCanonical => {
                f.write_str("the operation record is not in its canonical encoding")
            }
            OperationError::Encode(e) => write!(f, "the operation could not be encoded: {e}"),
        }
    }
}

impl core::error::Error for OperationError {}

impl From<PayloadError> for OperationError {
    fn from(e: PayloadError) -> Self {
        OperationError::Payload(e)
    }
}

impl From<NamesError> for OperationError {
    fn from(e: NamesError) -> Self {
        OperationError::Names(e)
    }
}

impl From<EncodeError> for OperationError {
    fn from(e: EncodeError) -> Self {
        OperationError::Encode(e)
    }
}

impl From<RecipeError> for OperationError {
    fn from(e: RecipeError) -> Self {
        OperationError::Input(e)
    }
}

impl Operation {
    /// The record's canonical encoding: `[op, name, version, inputs, params, budget]`, an input as
    /// the recipe writes one, `params` the parameters' bytes and `budget` `[work, logical-bytes]`.
    ///
    /// # Errors
    ///
    /// [`OperationError::Name`] for a name that is not printable ASCII, and
    /// [`OperationError::Params`] for parameters that are not canonical CBOR.
    pub fn encode(&self, meter: &mut WorkMeter<'_>) -> Result<Vec<u8>, OperationError> {
        if self.name.is_empty() || !self.name.bytes().all(|b| matches!(b, b'!'..=b'~')) {
            return Err(OperationError::Name);
        }
        check_params(&self.params, meter)?;
        let mut e = Encoder::new();
        e.array(6)?;
        e.item(&encode_op_path(&self.op).map_err(|n| OperationError::Names(n.into()))?)?;
        e.text(&self.name)?;
        e.uint(self.version)?;
        e.array(u64::try_from(self.inputs.len()).map_err(|_| EncodeError::TooLong)?)?;
        for input in &self.inputs {
            e.array(2)?;
            match input {
                RecipeInput::Body(id) => {
                    e.uint(0)?;
                    e.item(&id.item()?)?;
                }
                RecipeInput::Entity(name) => {
                    e.uint(1)?;
                    e.bytes(&name.to_bytes())?;
                }
            }
        }
        e.bytes(&self.params)?;
        e.array(2)?;
        e.uint(self.budget.work)?;
        e.uint(self.budget.logical_bytes)?;
        Ok(e.finish()?.into_bytes())
    }
}

/// The parameters must be exactly one item of SKG's canonical CBOR.
fn check_params(params: &[u8], meter: &mut WorkMeter<'_>) -> Result<(), OperationError> {
    check(params, PAYLOAD_DEPTH, meter).map_err(OperationError::Params)
}

/// The operation record of `operation`.
///
/// # Errors
///
/// As [`Operation::encode`].
pub fn operation_record(
    operation: &Operation,
    meter: &mut WorkMeter<'_>,
) -> Result<Record, OperationError> {
    Ok(Record {
        tag: OPERATION_TAG,
        flags: REQUIRED,
        schema: OPERATION_SCHEMA,
        depends: Vec::new(),
        payload: operation.encode(meter)?,
    })
}

/// Reads an operation record's payload.
///
/// # Errors
///
/// [`OperationError`] for a broken shape, a name that is not printable ASCII, parameters that are
/// not canonical CBOR, or bytes that are not the record's canonical encoding.
pub fn read_operation(
    payload: &[u8],
    meter: &mut WorkMeter<'_>,
) -> Result<Operation, OperationError> {
    let mut r = Reader::new(payload);
    let offset = r.offset();
    let shape =
        |offset, expected| OperationError::Payload(PayloadError::Shape { offset, expected });
    if r.array(meter, "an operation array")? != 6 {
        return Err(shape(offset, "an operation of six items"));
    }
    let op = read_op_path(&mut r, meter)?;
    let name = String::from(r.text(meter, "the operation's name")?);
    let version = r.uint(meter, "the operation's version")?;
    let count = r.array(meter, "the inputs")?;
    let mut inputs = Vec::new();
    for _ in 0..count {
        let at = r.offset();
        if r.array(meter, "an input")? != 2 {
            return Err(shape(at, "an input [0, content-id] or [1, name-id]"));
        }
        inputs.push(match r.next(meter)? {
            Item::Uint(0) => RecipeInput::Body(read_content_id(&mut r, meter)?),
            Item::Uint(1) => RecipeInput::Entity(r.name(meter, "an input's NameId")?),
            _ => return Err(shape(at, "an input [0, content-id] or [1, name-id]")),
        });
    }
    let params = r.bytes(meter, "the parameters")?.to_vec();
    let at = r.offset();
    if r.array(meter, "the budget [work, logical-bytes]")? != 2 {
        return Err(shape(at, "a budget of two items"));
    }
    let budget = Budget::new(
        r.uint(meter, "the work budget")?,
        r.uint(meter, "the logical-memory budget")?,
    );
    r.end()?;
    let operation = Operation {
        op,
        name,
        version,
        inputs,
        params,
        budget,
    };
    if operation.encode(meter)? != payload {
        return Err(OperationError::NotCanonical);
    }
    Ok(operation)
}
