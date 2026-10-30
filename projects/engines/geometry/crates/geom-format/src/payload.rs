//! Reading a record's payload field by field: the next head, a whole item's bytes, names, and the
//! end of the payload, each failure naming its byte.
//!
//! What it does: a [`Reader`] steps through a payload that is already known to be canonical CBOR
//! (the record layer checks it), one head or one whole item at a time, and turns a wrong shape into
//! [`PayloadError::Shape`] with the byte offset and what was expected. The name, history, recipe
//! and attribute records (piece G95) read their payloads with it.
//!
//! Guarantees:
//!
//! - **Every failure names its byte**: the offset in the payload of the item that did not fit, and
//!   what the record needs there.
//! - **Containers are read head by head**, so a reader never holds more than one item at a time,
//!   and every whole item it returns is exactly the bytes the decoder measured.
//!
//! Method: SKG's canonical CBOR (`projects/engines/geometry/format/SPEC.md`, section 3) read with
//! this crate's decoder, as the header record's reader reads its payload (chapter 24.3,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`).
//!
//! Cost: linear in the payload; nothing is copied but the names it returns.

use core::fmt;

use geom_op::NameId;
use geom_run::WorkMeter;

use crate::cbor::{CborError, Decoder, Item, item_len};
use crate::header::head_len;
use crate::record::PAYLOAD_DEPTH;

/// Why a payload does not have the shape its record needs.
#[derive(Clone, Debug, PartialEq)]
pub enum PayloadError {
    /// The bytes break SKG's canonical CBOR.
    Cbor(CborError),
    /// Not the record's shape: what was expected, at which byte of the payload.
    Shape {
        /// The byte offset in the payload.
        offset: usize,
        /// What the record needs there.
        expected: &'static str,
    },
}

impl fmt::Display for PayloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PayloadError::Cbor(e) => write!(f, "not canonical CBOR: {e}"),
            PayloadError::Shape { offset, expected } => {
                write!(f, "{expected} expected at byte {offset}")
            }
        }
    }
}

impl core::error::Error for PayloadError {}

impl From<CborError> for PayloadError {
    fn from(e: CborError) -> Self {
        PayloadError::Cbor(e)
    }
}

/// Reads a payload one head or one whole item at a time.
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    /// A reader at the start of `bytes`.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    /// The offset of the next item.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.at
    }

    /// The next head alone: an array's or a map's head, or a whole scalar.
    ///
    /// # Errors
    ///
    /// [`PayloadError`] at the end of the bytes or on a broken encoding.
    pub fn next(&mut self, meter: &mut WorkMeter<'_>) -> Result<Item<'a>, PayloadError> {
        let rest = self.bytes.get(self.at..).unwrap_or_default();
        let mut d = Decoder::new(rest, 1);
        let offset = self.at;
        let item = d.next(meter)?.ok_or(PayloadError::Shape {
            offset,
            expected: "an item",
        })?;
        let used = match item {
            Item::Array(_) | Item::Map(_) | Item::Tag(_) => head_len(rest),
            _ => d.offset(),
        };
        self.at = self.at.saturating_add(used);
        Ok(item)
    }

    /// The next item, which must be an array; its length.
    ///
    /// # Errors
    ///
    /// [`PayloadError::Shape`] naming `expected` for anything else.
    pub fn array(
        &mut self,
        meter: &mut WorkMeter<'_>,
        expected: &'static str,
    ) -> Result<u64, PayloadError> {
        let offset = self.at;
        match self.next(meter)? {
            Item::Array(n) => Ok(n),
            _ => Err(PayloadError::Shape { offset, expected }),
        }
    }

    /// The next item, which must be an unsigned integer.
    ///
    /// # Errors
    ///
    /// [`PayloadError::Shape`] naming `expected` for anything else.
    pub fn uint(
        &mut self,
        meter: &mut WorkMeter<'_>,
        expected: &'static str,
    ) -> Result<u64, PayloadError> {
        let offset = self.at;
        match self.next(meter)? {
            Item::Uint(n) => Ok(n),
            _ => Err(PayloadError::Shape { offset, expected }),
        }
    }

    /// The next item, which must be a byte string.
    ///
    /// # Errors
    ///
    /// [`PayloadError::Shape`] naming `expected` for anything else.
    pub fn bytes(
        &mut self,
        meter: &mut WorkMeter<'_>,
        expected: &'static str,
    ) -> Result<&'a [u8], PayloadError> {
        let offset = self.at;
        match self.next(meter)? {
            Item::Bytes(b) => Ok(b),
            _ => Err(PayloadError::Shape { offset, expected }),
        }
    }

    /// The next item, which must be a text string.
    ///
    /// # Errors
    ///
    /// [`PayloadError::Shape`] naming `expected` for anything else.
    pub fn text(
        &mut self,
        meter: &mut WorkMeter<'_>,
        expected: &'static str,
    ) -> Result<&'a str, PayloadError> {
        let offset = self.at;
        match self.next(meter)? {
            Item::Text(t) => Ok(t),
            _ => Err(PayloadError::Shape { offset, expected }),
        }
    }

    /// The next item, which must be a 16-byte string: a persistent name.
    ///
    /// # Errors
    ///
    /// [`PayloadError::Shape`] naming `expected` for anything else.
    pub fn name(
        &mut self,
        meter: &mut WorkMeter<'_>,
        expected: &'static str,
    ) -> Result<NameId, PayloadError> {
        let offset = self.at;
        let bytes = self.bytes(meter, expected)?;
        <[u8; 16]>::try_from(bytes)
            .map(NameId::from_bytes)
            .map_err(|_| PayloadError::Shape { offset, expected })
    }

    /// The bytes of the next whole item, however deep, and steps over it.
    ///
    /// # Errors
    ///
    /// [`PayloadError`] at the end of the bytes or on a broken encoding.
    pub fn item(&mut self, meter: &mut WorkMeter<'_>) -> Result<&'a [u8], PayloadError> {
        let rest = self.bytes.get(self.at..).unwrap_or_default();
        let len = item_len(rest, PAYLOAD_DEPTH, meter)?;
        let item = rest.get(..len).unwrap_or_default();
        self.at = self.at.saturating_add(len);
        Ok(item)
    }

    /// The payload must end here.
    ///
    /// # Errors
    ///
    /// [`PayloadError::Shape`] when bytes remain.
    pub fn end(&self) -> Result<(), PayloadError> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(PayloadError::Shape {
                offset: self.at,
                expected: "the end of the payload",
            })
        }
    }
}
