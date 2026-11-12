//! Fuzz target: `geom-format`'s operation record, read from any payload, gives the same answer
//! twice, refuses inside its input, and accepts only the one canonical encoding of what it reads.
//!
//! What it does: [`harness`] reads the fuzzer's bytes as an operation record's payload with
//! `read_operation`, the reader every replay of an operation log goes through, and writes back
//! what it read with `Operation::encode`. The corpus starts from payloads written from the
//! operations of `geom-format`'s operation record tests, so the fuzzer edits real records.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming it, with the input in
//!   hexadecimal. The laws (the `operation` module header's): the same verdict twice; a refusal's
//!   offset inside the payload; and an operation accepted, encoded again, is exactly the payload
//!   read, its parameters one canonical item.
//!
//! Method: the laws of `geom-format`'s `operation` module header (chapter 21's replay and 28.13's
//! operation logs, `projects/engines/geometry/architecture/21-determinism.md` and
//! `28-testing.md`), driven by coverage-guided fuzzing (28.5).
//!
//! Cost: two reads of the payload and one write, each linear in it.

use geom_format::cbor::CborError;
use geom_format::operation::{OperationError, read_operation};
use geom_format::payload::PayloadError;
use geom_run::{Budget, WorkMeter};

use crate::Finding;
use crate::cbor::hex;
use crate::names::{names_offset, recipe_offset};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The offset an operation refusal names, if any.
fn offset(e: &OperationError) -> Option<usize> {
    match e {
        OperationError::Payload(
            PayloadError::Cbor(CborError { offset, .. }) | PayloadError::Shape { offset, .. },
        )
        | OperationError::Params(CborError { offset, .. }) => Some(*offset),
        OperationError::Names(n) => names_offset(n),
        OperationError::Input(r) => recipe_offset(r),
        _ => None,
    }
}

/// The fuzz harness: the laws on `data` read as an operation record's payload.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    let law = |law: &str, what: String| Finding::new(law, || format!("{}: {what}", hex(data)));
    let first = read_operation(data, &mut meter());
    if read_operation(data, &mut meter()) != first {
        return Err(law("a payload reads the same twice", format!("{first:?}")));
    }
    match first {
        Err(e) if offset(&e).is_some_and(|at| at > data.len()) => {
            Err(law("a refusal is inside the payload", format!("{e:?}")))
        }
        Ok(op) => match op.encode(&mut meter()) {
            Ok(back) if back == data => Ok(()),
            back => Err(law(
                "an operation read is its one encoding",
                format!("{op:?} written back as {back:?}"),
            )),
        },
        Err(_) => Ok(()),
    }
}
