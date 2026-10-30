//! The header record, the first record of every file: what a reader must know before it reads
//! anything else (24.4).
//!
//! What it does: a [`Header`] holds the format version, the kernel edition, the `NameScheme`, the
//! tolerance ladder in grid steps, the Unicode version of hashed text and the writing program, and
//! ends with an extension map (24.3). [`Header::v1`] fills it for this kernel from
//! `geom_num::ladder`'s rungs; [`Header::record`] writes it as a required record of type 1;
//! [`Header::read`] reads a payload back. Its CDDL is `format/cddl/header.cddl` in the kernel's
//! tree:
//!
//! ```cddl
//! header = [
//!   format: [major: uint, minor: uint],
//!   edition: uint,
//!   name-scheme: uint,
//!   ladder: { * uint => int },
//!   unicode: [uint, uint, uint],
//!   program: tstr,
//!   extensions: { * uint => any },
//! ]
//! ```
//!
//! Guarantees:
//!
//! - **One encoding** for every header: written by `geom-op`'s canonical encoder, keys ascending.
//! - **A reader refuses what it cannot honour**: a major format version other than 1
//!   ([`HeaderError::Format`]), and an extension key with its must-understand bit (bit 0) set
//!   that this version does not know ([`HeaderError::MustUnderstand`]). An unknown key without that
//!   bit is skipped and listed in [`Header::ignored`]. A newer minor version is read.
//! - **The ladder is the kernel's**: [`Header::v1`] takes every rung from `geom_num::ladder`, so a
//!   file records exactly the bounds its geometry was made under.
//!
//! Method: chapter 24.3 and 24.4 of the kernel architecture,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`; the ladder of chapter 6.5,
//! `projects/engines/geometry/architecture/06-numbers.md`; CDDL by RFC 8610,
//! <https://www.rfc-editor.org/rfc/rfc8610>.
//!
//! Cost: linear in the payload; a few small allocations for the ladder and the program's name.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use geom_num::grid::Steps;
use geom_num::ladder;
use geom_op::cbor::{EncodeError, Encoder};
use geom_run::WorkMeter;

use crate::cbor::{CborError, Decoder, Item, check, item_len};
use crate::file::HEADER_TAG;
use crate::record::{PAYLOAD_DEPTH, REQUIRED, Record};

/// The format version this kernel writes: 1.0, a draft until piece G482 freezes it.
pub const FORMAT: (u64, u64) = (1, 0);
/// The kernel edition before the first release: 0.
pub const PRE_EDITION: u64 = 0;
/// `NameScheme v1` (32.19).
pub const NAME_SCHEME: u64 = 1;
/// The Unicode version of hashed text under `NameScheme v1`: 18.0.0 (32.19).
pub const UNICODE: [u64; 3] = [18, 0, 0];
/// The header record's schema.
pub const SCHEMA: u64 = 1;

/// The ladder's rung codes, the keys of the header's `ladder` map.
pub mod rung {
    /// The fit bound, in steps.
    pub const FIT: u64 = 1;
    /// The per-operation bound, in steps.
    pub const PER_OPERATION: u64 = 2;
    /// This document's import healing distance, in steps.
    pub const HEAL: u64 = 3;
    /// The cap on import healing, in steps.
    pub const HEAL_CAP: u64 = 4;
    /// The short-edge warning, in steps.
    pub const SHORT_EDGE_WARNING: u64 = 5;
    /// Commit rounding per axis, in half steps.
    pub const COMMIT_PER_AXIS: u64 = 6;
    /// The angular bound, as a power of two in radians.
    pub const ANGULAR_LOG2: u64 = 7;
}

/// Why a header payload cannot be read.
#[derive(Clone, Debug, PartialEq)]
pub enum HeaderError {
    /// The bytes break SKG's canonical CBOR.
    Cbor(CborError),
    /// Not a header: what was expected, at which byte of the payload.
    Shape {
        /// The byte offset in the payload.
        offset: usize,
        /// What the header needs there.
        expected: &'static str,
    },
    /// A major format version this reader does not know.
    Format {
        /// The major version.
        major: u64,
        /// The minor version.
        minor: u64,
    },
    /// An extension key with its must-understand bit set, unknown to this version.
    MustUnderstand(u64),
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeaderError::Cbor(e) => write!(f, "not canonical CBOR: {e}"),
            HeaderError::Shape { offset, expected } => {
                write!(f, "not a header: {expected} expected at byte {offset}")
            }
            HeaderError::Format { major, minor } => {
                write!(f, "format {major}.{minor} is not readable here")
            }
            HeaderError::MustUnderstand(key) => {
                write!(f, "extension {key} must be understood and is not")
            }
        }
    }
}

impl core::error::Error for HeaderError {}

impl From<CborError> for HeaderError {
    fn from(e: CborError) -> Self {
        HeaderError::Cbor(e)
    }
}

/// A file's header (see the module header).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// The format version, major and minor.
    pub format: (u64, u64),
    /// The kernel edition.
    pub edition: u64,
    /// The `NameScheme`.
    pub name_scheme: u64,
    /// The ladder: each rung's code and value.
    pub ladder: BTreeMap<u64, i64>,
    /// The Unicode version of hashed text.
    pub unicode: [u64; 3],
    /// The writing program.
    pub program: String,
    /// The extension keys read and skipped (none is written).
    pub ignored: Vec<u64>,
}

impl Header {
    /// This kernel's header: format 1.0, the pre-release edition, `NameScheme v1`, Unicode 18.0.0,
    /// `geom_num::ladder`'s rungs with `heal` as this document's healing distance.
    #[must_use]
    pub fn v1(program: &str, heal: Steps) -> Header {
        let wide = |s: Steps| i64::try_from(s.0).unwrap_or(i64::MAX);
        let ladder = BTreeMap::from([
            (rung::FIT, wide(ladder::FIT)),
            (rung::PER_OPERATION, wide(ladder::PER_OPERATION)),
            (rung::HEAL, wide(heal)),
            (rung::HEAL_CAP, wide(ladder::HEAL_CAP)),
            (rung::SHORT_EDGE_WARNING, wide(ladder::SHORT_EDGE_WARNING)),
            (
                rung::COMMIT_PER_AXIS,
                i64::try_from(ladder::COMMIT_PER_AXIS.0).unwrap_or(i64::MAX),
            ),
            (rung::ANGULAR_LOG2, i64::from(ladder::ANGULAR_LOG2)),
        ]);
        Header {
            format: FORMAT,
            edition: PRE_EDITION,
            name_scheme: NAME_SCHEME,
            ladder,
            unicode: UNICODE,
            program: program.to_string(),
            ignored: Vec::new(),
        }
    }

    /// The header's canonical payload; the extension map is written empty.
    ///
    /// # Errors
    ///
    /// None in practice; the encoder's own error.
    pub fn payload(&self) -> Result<Vec<u8>, EncodeError> {
        let mut e = Encoder::new();
        e.array(7)?;
        e.array(2)?;
        e.uint(self.format.0)?;
        e.uint(self.format.1)?;
        e.uint(self.edition)?;
        e.uint(self.name_scheme)?;
        e.map(u64::try_from(self.ladder.len()).unwrap_or(u64::MAX))?;
        for (&code, &value) in &self.ladder {
            e.key(code)?;
            e.int(i128::from(value))?;
        }
        e.array(3)?;
        for part in self.unicode {
            e.uint(part)?;
        }
        e.text(&self.program)?;
        e.map(0)?;
        Ok(e.finish()?.into_bytes())
    }

    /// The header as a required record of type 1, schema 1.
    ///
    /// # Errors
    ///
    /// The encoder's own error.
    pub fn record(&self) -> Result<Record, EncodeError> {
        Ok(Record {
            tag: HEADER_TAG,
            flags: REQUIRED,
            schema: SCHEMA,
            depends: Vec::new(),
            payload: self.payload()?,
        })
    }

    /// Reads a header payload.
    ///
    /// # Errors
    ///
    /// A [`HeaderError`] for bytes that are not a header, a major version other than 1, or an
    /// unknown must-understand extension.
    pub fn read(payload: &[u8], meter: &mut WorkMeter<'_>) -> Result<Header, HeaderError> {
        // The whole payload in its canonical form first (map keys sorted and distinct included);
        // the fields are then read one by one.
        check(payload, PAYLOAD_DEPTH, meter)?;
        let mut r = Fields {
            bytes: payload,
            at: 0,
        };
        r.head(meter, |i| i == Item::Array(7), "a header array of 7")?;
        r.head(meter, |i| i == Item::Array(2), "the format version")?;
        let major = r.uint(meter, "the major version")?;
        let minor = r.uint(meter, "the minor version")?;
        if major != FORMAT.0 {
            return Err(HeaderError::Format { major, minor });
        }
        let edition = r.uint(meter, "the edition")?;
        let name_scheme = r.uint(meter, "the name scheme")?;
        let pairs = r.map(meter, "the ladder")?;
        let mut ladder = BTreeMap::new();
        for _ in 0..pairs {
            let code = r.uint(meter, "a rung code")?;
            let value = r.int(meter, "a rung value")?;
            ladder.insert(code, value);
        }
        r.head(meter, |i| i == Item::Array(3), "the Unicode version")?;
        let unicode = [
            r.uint(meter, "the Unicode major")?,
            r.uint(meter, "the Unicode minor")?,
            r.uint(meter, "the Unicode update")?,
        ];
        let program = r.text(meter, "the program")?;
        let extensions = r.map(meter, "the extensions")?;
        let mut ignored = Vec::new();
        for _ in 0..extensions {
            let key = r.uint(meter, "an extension key")?;
            if key & 1 == 1 {
                return Err(HeaderError::MustUnderstand(key));
            }
            r.skip(meter)?;
            ignored.push(key);
        }
        r.end()?;
        Ok(Header {
            format: (major, minor),
            edition,
            name_scheme,
            ladder,
            unicode,
            program,
            ignored,
        })
    }
}

/// Reads a payload one head or one whole item at a time.
struct Fields<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Fields<'a> {
    /// The next head alone (an array's or map's head, or a whole scalar).
    fn next(&mut self, meter: &mut WorkMeter<'_>) -> Result<Item<'a>, HeaderError> {
        let rest = self.bytes.get(self.at..).unwrap_or_default();
        let mut d = Decoder::new(rest, 1);
        let offset = self.at;
        let item = d.next(meter)?.ok_or(HeaderError::Shape {
            offset,
            expected: "an item",
        })?;
        let used = match item {
            // A container's head: step over the head only; its items are read one by one.
            Item::Array(_) | Item::Map(_) | Item::Tag(_) => head_len(rest),
            _ => d.offset(),
        };
        self.at = self.at.saturating_add(used);
        Ok(item)
    }

    fn head(
        &mut self,
        meter: &mut WorkMeter<'_>,
        fits: impl FnOnce(Item<'a>) -> bool,
        expected: &'static str,
    ) -> Result<(), HeaderError> {
        let offset = self.at;
        if fits(self.next(meter)?) {
            Ok(())
        } else {
            Err(HeaderError::Shape { offset, expected })
        }
    }

    fn uint(
        &mut self,
        meter: &mut WorkMeter<'_>,
        expected: &'static str,
    ) -> Result<u64, HeaderError> {
        let offset = self.at;
        match self.next(meter)? {
            Item::Uint(n) => Ok(n),
            _ => Err(HeaderError::Shape { offset, expected }),
        }
    }

    fn int(
        &mut self,
        meter: &mut WorkMeter<'_>,
        expected: &'static str,
    ) -> Result<i64, HeaderError> {
        let offset = self.at;
        let shape = HeaderError::Shape { offset, expected };
        match self.next(meter)? {
            Item::Uint(n) => i64::try_from(n).map_err(|_| shape),
            // −1 − n, for n up to i64::MAX: at least i64::MIN, so the subtraction never fails.
            Item::Nint(n) => i64::try_from(n)
                .ok()
                .and_then(|n| (-1_i64).checked_sub(n))
                .ok_or(shape),
            _ => Err(shape),
        }
    }

    fn map(
        &mut self,
        meter: &mut WorkMeter<'_>,
        expected: &'static str,
    ) -> Result<u64, HeaderError> {
        let offset = self.at;
        match self.next(meter)? {
            Item::Map(n) => Ok(n),
            _ => Err(HeaderError::Shape { offset, expected }),
        }
    }

    fn text(
        &mut self,
        meter: &mut WorkMeter<'_>,
        expected: &'static str,
    ) -> Result<String, HeaderError> {
        let offset = self.at;
        match self.next(meter)? {
            Item::Text(t) => Ok(t.to_string()),
            _ => Err(HeaderError::Shape { offset, expected }),
        }
    }

    /// Steps over one whole item, however deep.
    fn skip(&mut self, meter: &mut WorkMeter<'_>) -> Result<(), HeaderError> {
        let rest = self.bytes.get(self.at..).unwrap_or_default();
        let len = item_len(rest, PAYLOAD_DEPTH, meter)?;
        self.at = self.at.saturating_add(len);
        Ok(())
    }

    /// The payload must end here.
    fn end(&self) -> Result<(), HeaderError> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(HeaderError::Shape {
                offset: self.at,
                expected: "the end of the header",
            })
        }
    }
}

/// The length of the head at the start of `bytes`: 1, 2, 3, 5 or 9 bytes (RFC 8949 section 3).
pub(crate) fn head_len(bytes: &[u8]) -> usize {
    match bytes.first().map(|b| b & 0x1f) {
        Some(24) => 2,
        Some(25) => 3,
        Some(26) => 5,
        Some(27) => 9,
        _ => 1,
    }
}
