//! SHA-256, the hash of every benchmark's golden digests.
//!
//! The kernel's golden hashes are SHA-256 (chapters 21 and 28.13), so the benchmarks' output hashes
//! are too (chapter 23.13, "Output hash"). The hashing is RustCrypto's `sha2` crate, the one
//! SHA-256 implementation of every golden in the kernel (piece G19; `geom_test::golden` uses it
//! too); this module keeps the benchmark harnesses' interface to it: [`Sha256`], fed with
//! [`Sha256::update`] and finished into a [`Digest`]. It replaced a hand-written implementation of
//! the standard; every golden digest in `golden.txt` is unchanged, since SHA-256 is one function
//! whoever computes it.
//!
//! Method: the Secure Hash Standard, NIST FIPS 180-4,
//! <https://csrc.nist.gov/pubs/fips/180-4/upd1/final>, computed by the `sha2` crate (MIT OR
//! Apache-2.0, a dependency, not a port). The tests check the standard's own examples
//! (<https://csrc.nist.gov/projects/cryptographic-standards-and-guidelines/example-values>).
//!
//! Guarantees: the digest of every message below 2^61 bytes (2^64 bits, the standard's limit) is
//! the one FIPS 180-4 defines, whatever pieces the message is fed in (the tests check the
//! standard's examples, a million bytes in uneven pieces, and every padding length), on every
//! target: `sha2` uses a processor's SHA instructions where it finds them and portable code
//! elsewhere, WebAssembly included, and each computes the one function the standard defines; and
//! no method panics.
//!
//! Cost: one compression (64 rounds of 32-bit operations) per 64-byte block, so linear in the
//! message; the state is a fixed few hundred bytes, and nothing is allocated.

use core::fmt;

use sha2::Digest as _;

/// A SHA-256 digest: 32 bytes, written as 64 lowercase hexadecimal digits.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest(pub [u8; 32]);

impl Digest {
    /// The digest from 64 hexadecimal digits (either case), or `None` if `hex` is anything else.
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Digest> {
        let digits = hex.as_bytes();
        if digits.len() != 64 {
            return None;
        }
        // 64 digits make exactly 32 pairs, so nothing is left over.
        let (pairs, _) = digits.as_chunks::<2>();
        let mut out = [0u8; 32];
        for (byte, &[hi, lo]) in out.iter_mut().zip(pairs) {
            *byte = (nibble(hi)? << 4) | nibble(lo)?;
        }
        Some(Digest(out))
    }
}

/// The value of one hexadecimal digit.
fn nibble(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
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
        let mut h = Sha256::new();
        h.update(bytes);
        h.finish()
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
    }
}
