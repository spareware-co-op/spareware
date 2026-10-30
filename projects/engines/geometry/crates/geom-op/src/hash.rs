//! SHA-256, the kernel's one hash: for content IDs and, cut to 128 bits, for persistent names.
//!
//! What it does: [`Sha256`] takes a message piece by piece and gives its 32-byte digest, or the
//! digest's leftmost 16 bytes; [`sha256`] and [`sha256_128`] hash one buffer. The owner chose
//! SHA-256 for both uses (owner action G79, 32.20): a content ID is algorithm 1, the whole digest
//! of a record's payload (24.2), and a `NameId` is the leftmost 128 bits of the digest of a name's
//! domain prefix and canonical bytes (32.19, piece G87). The hashing itself is the `RustCrypto`
//! project's `sha2` crate, the implementation 32.20 approves, at the release `Cargo.lock` pins.
//!
//! Guarantees:
//!
//! - **The digest FIPS 180-4 defines.** For every message, in whatever pieces it is written, the
//!   digest is the standard's SHA-256: the tests check NIST's published test vectors (the
//!   Cryptographic Algorithm Validation Program's byte-oriented short and long messages and its
//!   Monte Carlo chain, and FIPS 180-4's examples), whole and written in pieces.
//! - **The 128-bit cut is the digest's first 16 bytes**, the leftmost 128 bits NIST SP 800-107
//!   section 5.1 allows a truncated SHA-256 to keep, and nothing else.
//! - **The same digest on every target and build.** `sha2` uses a processor's SHA instructions
//!   where it has them and portable code elsewhere, WebAssembly included; each computes the one
//!   function FIPS 180-4 defines, so the choice changes speed, never bits (its audit, cited in
//!   `deny-no-std.toml`, reads each path).
//! - **No panic, no `unsafe` here.** Writing and finishing never panic, and this module has no
//!   `unsafe` code; the crate's own, confined to its processor backends, is read in its audit.
//!
//! Method: NIST FIPS 180-4, *Secure Hash Standard* (2015),
//! <https://csrc.nist.gov/pubs/fips/180-4/upd1/final>, computed by the `sha2` crate (`RustCrypto`,
//! MIT OR Apache-2.0, a dependency, not a port); the cut follows NIST SP 800-107 Rev. 1 section
//! 5.1, <https://csrc.nist.gov/pubs/sp/800/107/r1/final>. The owner's choice and its reasons are
//! `projects/engines/geometry/architecture/32-decisions.md` (32.19 and 32.20). The test vectors
//! are NIST's, <https://csrc.nist.gov/projects/cryptographic-algorithm-validation-program/secure-hashing>.
//!
//! Cost: linear in the message, one compression per 64-byte block; the state is a fixed 112 bytes
//! or so, and nothing is allocated.

use sha2::Digest as _;

/// SHA-256 of a message written piece by piece.
///
/// `Clone` copies the state, so the digest of a prefix can be taken while hashing goes on.
#[derive(Clone, Debug, Default)]
pub struct Sha256(sha2::Sha256);

impl Sha256 {
    /// The state before any byte of the message.
    #[must_use]
    pub fn new() -> Sha256 {
        Sha256(sha2::Sha256::new())
    }

    /// Takes the message's next bytes.
    pub fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// The digest of everything written.
    #[must_use]
    pub fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }

    /// The digest's leftmost 128 bits, its first 16 bytes (NIST SP 800-107 section 5.1).
    #[must_use]
    pub fn finish_128(self) -> [u8; 16] {
        cut(self.finish())
    }
}

/// The SHA-256 digest of `bytes`.
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.write(bytes);
    hasher.finish()
}

/// The leftmost 128 bits of the SHA-256 digest of `bytes`.
#[must_use]
pub fn sha256_128(bytes: &[u8]) -> [u8; 16] {
    cut(sha256(bytes))
}

/// A digest's first 16 bytes.
fn cut(digest: [u8; 32]) -> [u8; 16] {
    let [
        x0,
        x1,
        x2,
        x3,
        x4,
        x5,
        x6,
        x7,
        x8,
        x9,
        x10,
        x11,
        x12,
        x13,
        x14,
        x15,
        ..,
    ] = digest;
    [
        x0, x1, x2, x3, x4, x5, x6, x7, x8, x9, x10, x11, x12, x13, x14, x15,
    ]
}
