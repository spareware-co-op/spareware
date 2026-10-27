//! SHA-256, the hash of every benchmark's golden digests: the `sha2` crate's, with the golden
//! digest type the gates compare and the hexadecimal form `golden.txt` stores.
//!
//! The kernel's golden hashes are SHA-256 (chapters 21 and 28.13), so the benchmarks' output hashes
//! are too (chapter 23.13, "Output hash"). The hash is the standard `sha2` crate's (MIT
//! OR Apache-2.0), and the hexadecimal form the standard `hex` crate's (MIT OR Apache-2.0): chapter
//! 29.1's rule since the owner's direction of 2026-10-22 (piece G12a) is to use well-known
//! permissive crates rather than hand-roll what they already do, and nothing here is kernel code
//! (this is a development crate, chapter 27). This module adds only what the gates need on top:
//! [`Digest`], a 32-byte value that is `Copy`, ordered and hashable so the tests can key maps by it,
//! and [`Sha256`], which feeds bytes in and gives a [`Digest`] out.
//!
//! Guarantees:
//!
//! - **The digest FIPS 180-4 defines**, whatever pieces the message is fed in: `sha2`'s, which the
//!   tests check against the standard's own examples, a million bytes in uneven pieces and every
//!   padding length.
//! - **The same digest on every target**: SHA-256 is fully specified integer arithmetic, and
//!   `sha2`'s portable and processor-specific back ends compute the same function (the tests run on
//!   every target the gates run on, wasm32 included).
//! - **A hexadecimal digest reads back as itself**: [`Digest::from_hex`] takes exactly 64 digits,
//!   either case, and refuses anything else; [`Digest`]'s `Display` writes 64 lowercase digits.
//! - No method panics.
//!
//! Method: the Secure Hash Standard, NIST FIPS 180-4
//! (<https://csrc.nist.gov/pubs/fips/180-4/upd1/final>), as the `sha2` crate implements it
//! (<https://docs.rs/sha2/0.11.0/sha2/>), checked against the standard's example values
//! (<https://csrc.nist.gov/projects/cryptographic-standards-and-guidelines/example-values>).
//!
//! Cost: one compression of 64 rounds per 64-byte block, so linear in the message; the state is
//! `sha2`'s fixed block buffer, and nothing is allocated except the string `Display` writes.

use core::fmt;

use sha2::Digest as _;

/// A SHA-256 digest: 32 bytes, written as 64 lowercase hexadecimal digits.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest(pub [u8; 32]);

impl Digest {
    /// The digest from 64 hexadecimal digits (either case), or `None` if `hex` is anything else.
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Digest> {
        let mut out = [0u8; 32];
        hex::decode_to_slice(hex, &mut out).ok()?;
        Some(Digest(out))
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// A SHA-256 computation in progress: feed it bytes with [`Sha256::update`], then take the digest
/// with [`Sha256::finish`].
#[derive(Clone, Debug, Default)]
pub struct Sha256(sha2::Sha256);

impl Sha256 {
    /// A computation over the empty message.
    #[must_use]
    pub fn new() -> Sha256 {
        Sha256(sha2::Sha256::new())
    }

    /// Appends `bytes` to the message.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// The digest of the message.
    #[must_use]
    pub fn finish(self) -> Digest {
        Digest(self.0.finalize().into())
    }

    /// The digest of `bytes`, in one call.
    #[must_use]
    pub fn digest(bytes: &[u8]) -> Digest {
        Digest(sha2::Sha256::digest(bytes).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        Sha256::digest(bytes).to_string()
    }

    #[test]
    fn matches_the_standards_examples() {
        // NIST's example values for SHA-256 (one block, two blocks, empty message).
        assert_eq!(
            hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let two_blocks = concat!(
            "abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmno",
            "ijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu"
        );
        assert_eq!(
            hex(two_blocks.as_bytes()),
            "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1"
        );
    }

    #[test]
    fn a_million_letters_a() {
        // The standard's long example, fed in uneven pieces so block boundaries fall everywhere.
        let mut h = Sha256::new();
        let piece = [b'a'; 997];
        let mut left = 1_000_000usize;
        while left > 0 {
            let n = left.min(piece.len());
            h.update(&piece[..n]);
            left -= n;
        }
        assert_eq!(
            h.finish().to_string(),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn every_padding_length_matches_one_call() {
        // Messages of 0 to 130 bytes cross every padding case (55, 56 and 64 bytes and beyond);
        // byte-at-a-time updates must give the one-call digest.
        let message: Vec<u8> = (0..130u8).collect();
        for len in 0..message.len() {
            let mut h = Sha256::new();
            for &b in &message[..len] {
                h.update(&[b]);
            }
            assert_eq!(h.finish(), Sha256::digest(&message[..len]), "{len} bytes");
        }
    }

    #[test]
    fn hex_round_trips() {
        let d = Sha256::digest(b"abc");
        assert_eq!(Digest::from_hex(&d.to_string()), Some(d));
        assert_eq!(Digest::from_hex(&d.to_string().to_uppercase()), Some(d));
        assert_eq!(Digest::from_hex("abc"), None);
        assert_eq!(Digest::from_hex(&"g".repeat(64)), None);
        assert_eq!(Digest::from_hex(&"0".repeat(66)), None);
        assert_eq!(Digest::from_hex(&"0".repeat(62)), None);
    }
}
