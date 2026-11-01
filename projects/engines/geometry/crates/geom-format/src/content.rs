//! Content IDs: a record's payload named by its hash, behind an algorithm code (24.2).
//!
//! What it does: a [`ContentId`] is `[alg, digest]`. Algorithm 1 is the SHA-256 of the payload's
//! bytes (owner action G79, 32.20), computed by `geom-op`'s hash, so names and content share one
//! implementation. An ID of another algorithm is kept as read, so a file written once a stronger
//! hash joins as algorithm 2 still carries its IDs through this reader byte for byte.
//!
//! Guarantees:
//!
//! - **Algorithm 1 is SHA-256 of exactly the payload's bytes**, its digest 32 bytes; an ID that
//!   says algorithm 1 with any other length is refused when read.
//! - **One encoding.** [`ContentId::item`] writes `[alg, h'digest']` in `geom-op`'s canonical
//!   encoding.
//!
//! Method: chapter 24.2's content-id rule, `content-id = [alg: uint, digest: bstr]`, in
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`; SHA-256 by NIST FIPS
//! 180-4, <https://csrc.nist.gov/pubs/fips/180-4/upd1/final>, through `geom_op::hash`.
//!
//! Cost: one SHA-256 pass over the payload for [`ContentId::of_payload`]; the digest is copied.

use alloc::vec::Vec;

use geom_op::cbor::{EncodeError, Encoder, Item};
use geom_op::hash::sha256;

/// The algorithm code of SHA-256 over the payload's bytes (24.2).
pub const SHA256: u64 = 1;

/// A content ID: an algorithm code and its digest.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ContentId {
    alg: u64,
    digest: Vec<u8>,
}

impl ContentId {
    /// The content ID of a payload: algorithm 1, its SHA-256.
    #[must_use]
    pub fn of_payload(payload: &[u8]) -> ContentId {
        ContentId {
            alg: SHA256,
            digest: sha256(payload).to_vec(),
        }
    }

    /// An ID as read: refused (`None`) when it says algorithm 1 and its digest is not 32 bytes.
    #[must_use]
    pub fn new(alg: u64, digest: &[u8]) -> Option<ContentId> {
        if alg == SHA256 && digest.len() != 32 {
            return None;
        }
        Some(ContentId {
            alg,
            digest: digest.to_vec(),
        })
    }

    /// The algorithm code.
    #[must_use]
    pub fn alg(&self) -> u64 {
        self.alg
    }

    /// The digest's bytes.
    #[must_use]
    pub fn digest(&self) -> &[u8] {
        &self.digest
    }

    /// The canonical encoding, `[alg, h'digest']`.
    ///
    /// # Errors
    ///
    /// None in practice: the encoder refuses nothing this writes. The error is the encoder's own.
    pub fn item(&self) -> Result<Item, EncodeError> {
        let mut e = Encoder::new();
        e.array(2)?;
        e.uint(self.alg)?;
        e.bytes(&self.digest)?;
        e.finish()
    }
}
