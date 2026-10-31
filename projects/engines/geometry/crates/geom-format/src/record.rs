//! The record envelope every SKG record shares: type, flags, schema, the records it was derived
//! from, its payload and a CRC-32C over the rest (24.2).
//!
//! What it does: a [`Record`] is
//!
//! ```cddl
//! record = [
//!   tag:     uint,            ; record type, from the registry; never reused
//!   flags:   uint,            ; criticality and copy policy
//!   schema:  uint,            ; layout version of this record type
//!   depends: [* content-id],  ; records this one was derived from, sorted
//!   payload: bstr,            ; one deterministic CBOR item, described by the tag's CDDL
//!   check:   uint,            ; CRC-32C of the encoding of the five items above
//! ]
//! ```
//!
//! [`Record::encode`] writes it with `geom-op`'s canonical encoder; [`Record::decode`] reads one
//! with this crate's strict decoder, so a record has exactly one encoding both ways. The flags:
//! [`REQUIRED`] (an unknown required record stops a reader, an unknown optional one is skipped and
//! listed) and [`COPY_ALWAYS`] (a program rewriting a file it only partly understands keeps the
//! record byte for byte; without it, only while every record in `depends` is present with the same
//! content ID). Any other flag bit is refused, so a bit added later reaches only readers that know
//! it.
//!
//! Guarantees:
//!
//! - **Damage is found.** The check is the CRC-32C of the bytes of the five items before it, read
//!   back exactly as stored; a record whose check differs is refused with
//!   [`RecordError::Checksum`] and nothing of it is returned.
//! - **One encoding per record.** `depends` is a set sorted by the bytes of each content ID's
//!   encoding (refused otherwise, repeats included), the payload is one item of SKG's canonical
//!   CBOR nested at most [`PAYLOAD_DEPTH`] deep, and every head is canonical.
//! - **Bounded.** A payload over [`PAYLOAD_CAP`] (64 MiB, 24.2's proposed cap) is refused both
//!   ways, before anything is allocated for it when reading from a source.
//! - **No panic.** Every refusal is a [`RecordError`] with the byte offset at fault.
//!
//! Method: chapter 24.2 of the kernel architecture,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`; PNG's critical and
//! safe-to-copy chunk rule (<https://www.w3.org/TR/png-3/#5Chunk-naming-conventions>), which 24.2's
//! flags make exact; CRC-32C by RFC 7143 (<https://www.rfc-editor.org/rfc/rfc7143>).
//!
//! Cost: linear in the record's bytes: one strict decode, one CRC and, for the payload, one check
//! of its canonical form; the payload is copied once.

use alloc::vec::Vec;
use core::fmt;

use geom_op::cbor::{EncodeError, Encoder, Item as Encoded};
use geom_run::WorkMeter;

use crate::cbor::{CborError, Decoder, Item, check};
use crate::checksum::crc32c;
use crate::content::ContentId;

/// The flag of a required record: a reader that does not know its tag and schema stops (24.2).
pub const REQUIRED: u64 = 1;
/// The flag of a record kept byte for byte by a program that rewrites a file it only partly
/// understands, whatever changed (24.2's copy-always policy).
pub const COPY_ALWAYS: u64 = 2;
/// Every flag this version knows.
const FLAGS: u64 = REQUIRED | COPY_ALWAYS;

/// The largest payload, 64 MiB (24.2, proposed; 31's list of figures to measure).
pub const PAYLOAD_CAP: u64 = 64 << 20;

/// How deep a payload's containers and tags may nest. Each record type's schema (G93's registry)
/// may set less; none needs more.
pub const PAYLOAD_DEPTH: usize = 64;

/// Containers in the envelope itself: the record, its `depends` list and each content ID.
const ENVELOPE_DEPTH: usize = 3;

/// One record (see the module header).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Record {
    /// The record type, from the registry.
    pub tag: u64,
    /// [`REQUIRED`] and [`COPY_ALWAYS`].
    pub flags: u64,
    /// The layout version of this record type.
    pub schema: u64,
    /// The records this one was derived from.
    pub depends: Vec<ContentId>,
    /// One deterministic CBOR item.
    pub payload: Vec<u8>,
}

/// Why a record has no encoding, or its bytes are not one.
#[derive(Clone, Debug, PartialEq)]
pub enum RecordError {
    /// The bytes break SKG's canonical CBOR (the decoder's error, its offset within the record).
    Cbor(CborError),
    /// The bytes are canonical CBOR but not a record: what was expected, at which offset.
    Shape {
        /// The byte offset within the record.
        offset: usize,
        /// What the envelope needs there.
        expected: &'static str,
    },
    /// A flag bit this version does not know.
    UnknownFlags(u64),
    /// `depends` out of order, or with a repeat.
    DependsOrder,
    /// A content ID of algorithm 1 whose digest is not 32 bytes.
    ContentId,
    /// A payload over [`PAYLOAD_CAP`] bytes.
    PayloadTooLarge(u64),
    /// The stored check differs from the CRC-32C of the bytes it covers.
    Checksum {
        /// The check as stored.
        stored: u64,
        /// The CRC-32C of the five items as read.
        computed: u32,
    },
    /// The encoder refused a value (writing only).
    Encode(EncodeError),
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordError::Cbor(e) => write!(f, "not canonical CBOR: {e}"),
            RecordError::Shape { offset, expected } => {
                write!(f, "not a record: {expected} expected at byte {offset}")
            }
            RecordError::UnknownFlags(flags) => write!(f, "unknown flag bits {flags:#x}"),
            RecordError::DependsOrder => f.write_str("depends is not a sorted set"),
            RecordError::ContentId => f.write_str("a SHA-256 content ID without 32 bytes"),
            RecordError::PayloadTooLarge(len) => {
                write!(f, "a payload of {len} bytes, over the 64 MiB cap")
            }
            RecordError::Checksum { stored, computed } => {
                write!(
                    f,
                    "check {stored:#x} stored, CRC-32C {computed:#x} computed"
                )
            }
            RecordError::Encode(e) => write!(f, "no encoding: {e}"),
        }
    }
}

impl core::error::Error for RecordError {}

impl From<EncodeError> for RecordError {
    fn from(e: EncodeError) -> Self {
        RecordError::Encode(e)
    }
}

impl From<CborError> for RecordError {
    fn from(e: CborError) -> Self {
        RecordError::Cbor(e)
    }
}

impl Record {
    /// Whether a reader that does not know this record must stop.
    #[must_use]
    pub fn is_required(&self) -> bool {
        self.flags & REQUIRED != 0
    }

    /// Whether a rewriting program keeps this record whatever changed.
    #[must_use]
    pub fn is_copy_always(&self) -> bool {
        self.flags & COPY_ALWAYS != 0
    }

    /// The payload's content ID (algorithm 1).
    #[must_use]
    pub fn content_id(&self) -> ContentId {
        ContentId::of_payload(&self.payload)
    }

    /// The record's canonical bytes.
    ///
    /// # Errors
    ///
    /// [`RecordError::UnknownFlags`], [`RecordError::PayloadTooLarge`], [`RecordError::Cbor`] for
    /// a payload that is not one canonical item, and [`RecordError::Encode`] for repeated
    /// `depends`.
    pub fn encode(&self, meter: &mut WorkMeter<'_>) -> Result<Vec<u8>, RecordError> {
        if self.flags & !FLAGS != 0 {
            return Err(RecordError::UnknownFlags(self.flags));
        }
        let len = u64::try_from(self.payload.len()).unwrap_or(u64::MAX);
        if len > PAYLOAD_CAP {
            return Err(RecordError::PayloadTooLarge(len));
        }
        check(&self.payload, PAYLOAD_DEPTH, meter)?;
        let depends = self
            .depends
            .iter()
            .map(ContentId::item)
            .collect::<Result<Vec<_>, _>>()?;
        let five = [
            one(|e| e.uint(self.tag))?,
            one(|e| e.uint(self.flags))?,
            one(|e| e.uint(self.schema))?,
            one(|e| e.set(depends))?,
            one(|e| e.bytes(&self.payload))?,
        ];
        let mut covered = Vec::new();
        for item in &five {
            covered.extend_from_slice(item.as_bytes());
        }
        let mut e = Encoder::new();
        e.array(6)?;
        for item in &five {
            e.item(item)?;
        }
        e.uint(u64::from(crc32c(&covered)))?;
        Ok(e.finish()?.into_bytes())
    }

    /// Reads the record that starts at the first byte of `bytes`, and how many bytes it takes.
    ///
    /// # Errors
    ///
    /// A [`RecordError`] for any byte that breaks the canonical form or the envelope, a payload
    /// over the cap, unknown flags, or a check that does not match.
    pub fn decode(bytes: &[u8], meter: &mut WorkMeter<'_>) -> Result<(Record, usize), RecordError> {
        let mut d = Decoder::new(bytes, ENVELOPE_DEPTH);
        array(&mut d, meter, 6, "a record array of 6")?;
        let start = d.offset();
        let tag = uint(&mut d, meter, "the record type")?;
        let flags = uint(&mut d, meter, "the flags")?;
        let schema = uint(&mut d, meter, "the schema")?;
        let n = any_array(&mut d, meter, "the depends array")?;
        let mut depends: Vec<ContentId> = Vec::new();
        let mut previous: Option<&[u8]> = None;
        for _ in 0..n {
            let at = d.offset();
            array(&mut d, meter, 2, "a content ID array of 2")?;
            let alg = uint(&mut d, meter, "a content ID's algorithm")?;
            let digest = bytes_item(&mut d, meter, "a content ID's digest")?;
            let id = ContentId::new(alg, digest).ok_or(RecordError::ContentId)?;
            let encoding = bytes.get(at..d.offset()).unwrap_or_default();
            if previous.is_some_and(|p| p >= encoding) {
                return Err(RecordError::DependsOrder);
            }
            previous = Some(encoding);
            depends.push(id);
        }
        let payload = bytes_item(&mut d, meter, "the payload")?;
        let len = u64::try_from(payload.len()).unwrap_or(u64::MAX);
        if len > PAYLOAD_CAP {
            return Err(RecordError::PayloadTooLarge(len));
        }
        let covered = bytes.get(start..d.offset()).unwrap_or_default();
        let stored = uint(&mut d, meter, "the check")?;
        let end = d.end()?;
        if flags & !FLAGS != 0 {
            return Err(RecordError::UnknownFlags(flags));
        }
        let computed = crc32c(covered);
        if stored != u64::from(computed) {
            return Err(RecordError::Checksum { stored, computed });
        }
        check(payload, PAYLOAD_DEPTH, meter)?;
        let record = Record {
            tag,
            flags,
            schema,
            depends,
            payload: payload.to_vec(),
        };
        Ok((record, end))
    }
}

/// One item written by `f`, finished.
fn one(f: impl FnOnce(&mut Encoder) -> Result<(), EncodeError>) -> Result<Encoded, EncodeError> {
    let mut e = Encoder::new();
    f(&mut e)?;
    e.finish()
}

/// The next head, an array of exactly `len` items.
fn array(
    d: &mut Decoder<'_>,
    meter: &mut WorkMeter<'_>,
    len: u64,
    expected: &'static str,
) -> Result<(), RecordError> {
    let offset = d.offset();
    match d.next(meter)? {
        Some(Item::Array(n)) if n == len => Ok(()),
        _ => Err(RecordError::Shape { offset, expected }),
    }
}

/// The next head, an array of any length.
fn any_array(
    d: &mut Decoder<'_>,
    meter: &mut WorkMeter<'_>,
    expected: &'static str,
) -> Result<u64, RecordError> {
    let offset = d.offset();
    match d.next(meter)? {
        Some(Item::Array(n)) => Ok(n),
        _ => Err(RecordError::Shape { offset, expected }),
    }
}

/// The next head, an unsigned integer.
fn uint(
    d: &mut Decoder<'_>,
    meter: &mut WorkMeter<'_>,
    expected: &'static str,
) -> Result<u64, RecordError> {
    let offset = d.offset();
    match d.next(meter)? {
        Some(Item::Uint(n)) => Ok(n),
        _ => Err(RecordError::Shape { offset, expected }),
    }
}

/// The next head, a byte string.
fn bytes_item<'a>(
    d: &mut Decoder<'a>,
    meter: &mut WorkMeter<'_>,
    expected: &'static str,
) -> Result<&'a [u8], RecordError> {
    let offset = d.offset();
    match d.next(meter)? {
        Some(Item::Bytes(b)) => Ok(b),
        _ => Err(RecordError::Shape { offset, expected }),
    }
}

/// Bounded proofs (chapter 28.7; piece G112), proven by `kernel-fuzz.yml`'s `kani-format` job.
#[cfg(kani)]
// cargo-mutants reads this attribute from the source and skips the module (its harnesses are
// proven by Kani, and no test build compiles them); no build evaluates it, since the module
// exists only under Kani, where `test` is off.
#[cfg_attr(test, mutants::skip)]
mod proofs {
    use geom_run::{Budget, WorkMeter};

    use super::Record;

    /// Any bytes up to 4, every head and refusal of an envelope's start, decode to a record or a
    /// refusal, never a panic. Longer raw bytes are beyond a bounded proof (every head branches
    /// on every byte): `tests/fuzz_records.rs` and `tests/record_layer.rs` cover them.
    #[kani::proof]
    #[kani::unwind(6)]
    fn decode_never_panics() {
        const LEN: usize = 4;
        let bytes: [u8; LEN] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= LEN);
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        if let Ok((_, end)) = Record::decode(&bytes[..len], &mut meter) {
            assert!(end <= len);
        }
    }

    /// A shortest record, heads fixed and every value any byte, its check any four bytes: decoding
    /// reaches every field, the flags, the checksum and the payload's own check, and gives a
    /// record or a refusal, never a panic.
    #[kani::proof]
    #[kani::unwind(14)]
    #[kani::stub(crate::cbor::decode::check_tagged, crate::cbor::decode::proofs::any_tagged)]
    #[kani::stub(crate::cbor::decode::utf8, crate::cbor::decode::proofs::any_text)]
    #[kani::stub(crate::cbor::decode::key_order, crate::cbor::decode::proofs::any_order)]
    fn decode_of_a_shaped_record_never_panics() {
        let (tag, flags, schema, payload): (u8, u8, u8, u8) = kani::any();
        kani::assume(tag < 24 && flags < 24 && schema < 24 && payload < 24);
        let check: [u8; 4] = kani::any();
        let bytes = [
            0x86, tag, flags, schema, 0x80, 0x41, payload, 0x1a, check[0], check[1], check[2],
            check[3],
        ];
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        if let Ok((_, end)) = Record::decode(&bytes, &mut meter) {
            assert!(end == bytes.len());
        }
    }
}
