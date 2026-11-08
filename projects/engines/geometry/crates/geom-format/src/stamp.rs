//! Validation stamps: which checker passed a stored body, which of chapter 8.3's groups it passed,
//! and under which kernel edition, so a program reopening its own store runs only the cheap checks
//! (24.7).
//!
//! What it does: a [`Stamp`] is an optional record (type 5) about one body index. Its `depends` is
//! that body, and its payload names the body again, so stamps about two bodies never share a
//! content ID. [`Stamp::record`] writes it; [`Stamp::read`] reads one back. Its CDDL is
//! `format/cddl/stamp.cddl` in the kernel's tree:
//!
//! ```cddl
//! stamp = [
//!   body: content-id,  ; the body index the stamp is about
//!   checker: uint,     ; the version of the checker that passed it
//!   groups: [* uint],  ; the groups of chapter 8.3 it passed, ascending
//!   edition: uint,     ; the kernel edition the check ran under
//! ]
//! ```
//!
//! Guarantees:
//!
//! - **A stamp is not part of its body.** It is a record of its own, not a field of the index, so
//!   checking a body again with a newer checker adds a stamp and leaves the body's content ID, and
//!   everything that names it, unchanged.
//! - **A stamp lapses with its body.** It is optional and not copy-always: a reader that does not
//!   know stamps skips it and checks the body in full, and a program rewriting a file keeps it only
//!   while its body is there with the same content ID (24.2's copy rule).
//! - **One encoding.** The groups strictly ascending and `depends` exactly the body named in the
//!   payload; anything else is refused with a [`StampError`].
//!
//! Method: chapter 24.7 of the kernel architecture (trust on open),
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`, and the copy rule of 24.2,
//! which takes PNG's safe-to-copy chunks (<https://www.w3.org/TR/png-3/#5Chunk-naming-conventions>).
//!
//! Cost: linear in the groups listed; a few heads.

use alloc::vec::Vec;
use core::fmt;

use geom_op::cbor::{EncodeError, Encoder};
use geom_run::WorkMeter;

use crate::cbor::{CborError, Decoder, Item};
use crate::content::ContentId;
use crate::record::Record;

/// The record type of a validation stamp.
pub const STAMP_TAG: u64 = 5;
/// Its schema.
pub const SCHEMA: u64 = 1;
/// The flags it is written with: neither required nor copy-always.
pub const FLAGS: u64 = 0;
/// The containers a stamp's payload nests at most: the stamp, then its content ID or its groups.
pub const DEPTH: usize = 2;

/// A body's validation stamp (24.7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    /// The content ID of the body's index.
    pub body: ContentId,
    /// The version of the checker that passed it.
    pub checker: u64,
    /// The groups of chapter 8.3 it passed, by number, ascending.
    pub groups: Vec<u64>,
    /// The kernel edition the check ran under.
    pub edition: u64,
}

/// Why a stamp could not be written or read.
#[derive(Clone, Debug, PartialEq)]
pub enum StampError {
    /// The encoder refused a value (writing).
    Encode(EncodeError),
    /// Bytes that break SKG's canonical CBOR.
    Cbor(CborError),
    /// A record or payload that is not a stamp of schema 1.
    Shape(&'static str),
}

impl fmt::Display for StampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StampError::Encode(e) => write!(f, "no encoding: {e}"),
            StampError::Cbor(e) => write!(f, "not canonical CBOR: {e}"),
            StampError::Shape(what) => write!(f, "not a validation stamp: {what}"),
        }
    }
}

impl core::error::Error for StampError {}

impl From<EncodeError> for StampError {
    fn from(e: EncodeError) -> Self {
        StampError::Encode(e)
    }
}

impl From<CborError> for StampError {
    fn from(e: CborError) -> Self {
        StampError::Cbor(e)
    }
}

impl Stamp {
    /// Whether the stamp says the body passed group `group`.
    #[must_use]
    pub fn passed(&self, group: u64) -> bool {
        self.groups.binary_search(&group).is_ok()
    }

    /// The stamp as a record: type 5, optional, depending on its body alone.
    ///
    /// # Errors
    ///
    /// [`StampError::Shape`] if the groups are not strictly ascending, and the encoder's own error.
    pub fn record(&self) -> Result<Record, StampError> {
        if !ascending(&self.groups) {
            return Err(StampError::Shape("groups strictly ascending"));
        }
        let mut e = Encoder::new();
        e.array(4)?;
        e.item(&self.body.item()?)?;
        e.uint(self.checker)?;
        e.array(u64::try_from(self.groups.len()).unwrap_or(u64::MAX))?;
        for &g in &self.groups {
            e.uint(g)?;
        }
        e.uint(self.edition)?;
        Ok(Record {
            tag: STAMP_TAG,
            flags: FLAGS,
            schema: SCHEMA,
            depends: alloc::vec![self.body.clone()],
            payload: e.finish()?.into_bytes(),
        })
    }

    /// Reads a stamp back from its record.
    ///
    /// # Errors
    ///
    /// [`StampError::Shape`] for a record of another type, schema or flags, a payload that is not a
    /// stamp, groups out of order, or a `depends` other than the body the payload names; the
    /// decoder's errors.
    pub fn read(record: &Record, meter: &mut WorkMeter<'_>) -> Result<Stamp, StampError> {
        if record.tag != STAMP_TAG || record.schema != SCHEMA || record.flags != FLAGS {
            return Err(StampError::Shape("a stamp record of schema 1"));
        }
        let mut d = Decoder::new(&record.payload, DEPTH);
        if d.next(meter)? != Some(Item::Array(4)) {
            return Err(StampError::Shape("a stamp of four fields"));
        }
        if d.next(meter)? != Some(Item::Array(2)) {
            return Err(StampError::Shape("a content ID of two fields"));
        }
        let alg = uint(&mut d, meter)?;
        let Some(Item::Bytes(digest)) = d.next(meter)? else {
            return Err(StampError::Shape("a digest"));
        };
        let body = ContentId::new(alg, digest).ok_or(StampError::Shape("a content ID"))?;
        let checker = uint(&mut d, meter)?;
        let Some(Item::Array(n)) = d.next(meter)? else {
            return Err(StampError::Shape("an array of groups"));
        };
        let mut groups = Vec::new();
        for _ in 0..n {
            groups.push(uint(&mut d, meter)?);
        }
        if !ascending(&groups) {
            return Err(StampError::Shape("groups strictly ascending"));
        }
        let edition = uint(&mut d, meter)?;
        if d.next(meter)?.is_some() || d.end()? != record.payload.len() {
            return Err(StampError::Shape("bytes after the stamp"));
        }
        if record.depends.as_slice() != [body.clone()] {
            return Err(StampError::Shape("depends other than the stamped body"));
        }
        Ok(Stamp {
            body,
            checker,
            groups,
            edition,
        })
    }
}

/// Whether `v` is strictly ascending.
fn ascending(v: &[u64]) -> bool {
    v.windows(2).all(|w| matches!(w, [a, b] if a < b))
}

/// The next head, which must be an unsigned integer.
fn uint(d: &mut Decoder<'_>, meter: &mut WorkMeter<'_>) -> Result<u64, StampError> {
    match d.next(meter)? {
        Some(Item::Uint(n)) => Ok(n),
        _ => Err(StampError::Shape("an unsigned integer")),
    }
}
