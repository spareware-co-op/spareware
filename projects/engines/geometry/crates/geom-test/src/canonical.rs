//! The canonical output of one case and its SHA-256: what a fixture's golden hash is taken over
//! (chapter 28.13; piece G96).
//!
//! What it does: [`canonical_output`] encodes one operation's [`Outcome`], a success or a failure,
//! in SKG's canonical CBOR and hashes it with SHA-256 after the domain prefix [`PREFIX`]. A success
//! is its result bodies (each by the SHA-256 of its native canonical encoding, in the operation's
//! order), its new names (a set), its history with the operation's path, its exactness class and
//! its diagnostics; a failure is its error's base kind, entities and evidence. [`canonical_bytes`]
//! gives the encoding the hash is taken over, for a test that needs to see the difference.
//!
//! Guarantees:
//!
//! - **Errors hash as deterministically as results**: an error is encoded from values only (kind,
//!   entities, evidence), never from its words, so the same failure hashes the same on every
//!   target, as 28.13 asks: AI agents replay errors.
//! - **One hash per outcome**: every part has one encoding (`geom-op`'s canonical encodings and
//!   `History::encode`), names are a set, and the prefix keeps these hashes apart from names'
//!   (32.19) and content IDs (24.2).
//!
//! Method: chapter 28.13's canonical output
//! (`projects/engines/geometry/architecture/28-testing.md`), hashed with SHA-256 (FIPS 180-4) as
//! every golden is, through [`crate::golden::sha256`].
//!
//! Cost: linear in the outcome; one encoding and one hash.

use core::fmt;

use geom_op::canonical::{encode_diagnostic, encode_error, encode_exactness};
use geom_op::cbor::{EncodeError, Encoder};
use geom_op::{Diagnostic, Exactness, History, NameError, NameSet, OpError, OpPath};

use crate::golden::sha256;

/// The bytes hashed before every canonical output: the 23 ASCII bytes `SKG canonical output v1`
/// and a zero byte.
pub const PREFIX: &[u8; 24] = b"SKG canonical output v1\0";

/// One operation's outcome, as the canonical output reads it.
#[derive(Clone, Copy, Debug)]
pub enum Outcome<'a> {
    /// A success.
    Output {
        /// Each result body's SHA-256 over its native canonical encoding, in the operation's order.
        bodies: &'a [[u8; 32]],
        /// The new names it made.
        names: &'a NameSet,
        /// Its history, with the operation's path; `None` for an operation that records none (a
        /// query).
        history: Option<(&'a OpPath, &'a History)>,
        /// Its exactness class and bound.
        exactness: &'a Exactness,
        /// Its warnings, in the order it gave them.
        diagnostics: &'a [Diagnostic],
    },
    /// A failure.
    Error(&'a OpError),
}

/// Why an outcome could not be encoded; only a defect can cause either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanonicalError {
    /// The encoder refused a value.
    Encode(EncodeError),
    /// A name could not be encoded.
    Name(NameError),
}

impl fmt::Display for CanonicalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CanonicalError::Encode(e) => write!(f, "the outcome could not be encoded: {e}"),
            CanonicalError::Name(e) => write!(f, "a name could not be encoded: {e}"),
        }
    }
}

impl core::error::Error for CanonicalError {}

impl From<EncodeError> for CanonicalError {
    fn from(e: EncodeError) -> Self {
        CanonicalError::Encode(e)
    }
}

impl From<NameError> for CanonicalError {
    fn from(e: NameError) -> Self {
        CanonicalError::Name(e)
    }
}

/// A length as CBOR counts it.
fn count(n: usize) -> Result<u64, EncodeError> {
    u64::try_from(n).map_err(|_| EncodeError::TooLong)
}

/// The canonical encoding of `outcome`: `[0, bodies, names, history, exactness, diagnostics]` for
/// a success (`history` is an empty array when there is none), `[1, error]` for a failure.
///
/// # Errors
///
/// [`CanonicalError`], which only a defect can cause.
pub fn canonical_bytes(outcome: &Outcome<'_>) -> Result<Vec<u8>, CanonicalError> {
    let mut e = Encoder::new();
    match outcome {
        Outcome::Output {
            bodies,
            names,
            history,
            exactness,
            diagnostics,
        } => {
            e.array(6)?;
            e.uint(0)?;
            e.array(count(bodies.len())?)?;
            for body in *bodies {
                e.bytes(body)?;
            }
            e.array(count(names.names().len())?)?;
            for name in names.names() {
                e.bytes(&name.to_bytes())?;
            }
            match history {
                Some((op, history)) => e.item(&history.encode(op)?)?,
                None => e.array(0)?,
            }
            e.item(&encode_exactness(exactness)?)?;
            e.array(count(diagnostics.len())?)?;
            for d in *diagnostics {
                e.item(&encode_diagnostic(d)?)?;
            }
        }
        Outcome::Error(error) => {
            e.array(2)?;
            e.uint(1)?;
            e.item(&encode_error(error)?)?;
        }
    }
    Ok(e.finish()?.into_bytes())
}

/// The canonical output's SHA-256: over [`PREFIX`] and [`canonical_bytes`].
///
/// # Errors
///
/// As [`canonical_bytes`].
pub fn canonical_output(outcome: &Outcome<'_>) -> Result<[u8; 32], CanonicalError> {
    let mut bytes = PREFIX.to_vec();
    bytes.extend(canonical_bytes(outcome)?);
    Ok(sha256(&bytes))
}
