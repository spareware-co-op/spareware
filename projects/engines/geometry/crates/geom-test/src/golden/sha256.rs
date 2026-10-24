//! SHA-256, the hash of every golden digest: the one implementation every kernel test hashes with.
//!
//! What it does: [`Sha256`] takes a canonical byte stream piece by piece and gives its 32-byte
//! digest; [`sha256`] hashes one buffer; [`hex`] writes a digest as the 64 lowercase hexadecimal
//! digits the golden store keeps, and [`Digest`] reads them back. The hashing itself is the
//! `RustCrypto` project's `sha2` crate. Before this module, each crate's golden tests carried a
//! hand-written copy of `geom-num`'s implementation (chapter 21: "Each crate's tests carry a copy
//! of `geom-num`'s implementation until the golden store holds the one copy"); every copy now uses
//! this one, and every digest they pinned is unchanged, since SHA-256 is one function whoever
//! computes it.
//!
//! Guarantees:
//!
//! - **The digest FIPS 180-4 defines.** For every message, in whatever pieces it is written, the
//!   digest is the standard's SHA-256. The tests check the standard's examples (the empty message,
//!   "abc", the two-block 448-bit and 896-bit messages and one million "a"s) in pieces of every
//!   size around the 55-, 56- and 64-byte padding edges, and every message length from 0 to 130
//!   bytes against one call.
//! - **The same digest on every target and build.** `sha2` picks a processor's SHA instructions
//!   where it has them (x86 SHA-NI, the Arm SHA-2 extension) and portable 32-bit code elsewhere,
//!   WebAssembly included; each computes the one function FIPS 180-4 defines, so the choice changes
//!   speed, never bits. The kernel's goldens pass on every leg of `kernel.yml` with it.
//! - **No panic.** Writing, finishing and formatting never panic; [`Digest::from_hex`] refuses
//!   anything but exactly 64 hexadecimal digits with `None`.
//!
//! Method: NIST FIPS 180-4, *Secure Hash Standard* (2015),
//! <https://csrc.nist.gov/pubs/fips/180-4/upd1/final>, computed by the `sha2` crate (`RustCrypto`,
//! MIT OR Apache-2.0, a dependency, not a port). Chapters 21 and 28.13 of the kernel architecture
//! (`projects/engines/geometry/architecture/28-testing.md`) name SHA-256 as the golden hash. The
//! test vectors are NIST's examples,
//! <https://csrc.nist.gov/projects/cryptographic-standards-and-guidelines/example-values>.
//!
//! Cost: linear in the message, one compression per 64-byte block; the state is a fixed 112 bytes
//! or so and nothing is allocated, except the 64-character `String` of [`hex`].

use core::fmt;
use core::fmt::Write as _;

use sha2::Digest as _;

/// SHA-256 of a byte stream written piece by piece.
///
/// `Clone` copies the state, so a digest of a prefix can be taken while hashing goes on (the
/// section digests of `geom-num`'s corpus do this).
#[derive(Clone, Debug, Default)]
pub struct Sha256(sha2::Sha256);

impl Sha256 {
    /// The hash of the empty message so far.
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
}

/// The SHA-256 of `bytes`.
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.write(bytes);
    h.finish()
}

/// A digest as 64 lowercase hexadecimal digits, the form the golden store keeps.
#[must_use]
pub fn hex(digest: [u8; 32]) -> String {
    Digest(digest).to_string()
}

/// A SHA-256 digest, written and read as 64 lowercase hexadecimal digits.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest(pub [u8; 32]);

impl Digest {
    /// Reads exactly 64 hexadecimal digits (either case), or `None`.
    #[must_use]
    pub fn from_hex(text: &str) -> Option<Digest> {
        let bytes = text.as_bytes();
        if bytes.len() != 64 {
            return None;
        }
        let mut out = [0u8; 32];
        for (byte, pair) in out.iter_mut().zip(bytes.chunks_exact(2)) {
            let &[high, low] = pair else {
                return None;
            };
            *byte = (nibble(high)? << 4) | nibble(low)?;
        }
        Some(Digest(out))
    }

    /// Whether `text` is exactly the form [`Digest`]'s `Display` writes: 64 lowercase digits.
    #[must_use]
    pub fn is_canonical_hex(text: &str) -> bool {
        Digest::from_hex(text).is_some_and(|d| d.to_string() == text)
    }
}

/// The value of one hexadecimal digit.
fn nibble(digit: u8) -> Option<u8> {
    char::from(digit)
        .to_digit(16)
        .and_then(|v| u8::try_from(v).ok())
}

impl fmt::Display for Digest {
    /// 64 lowercase hexadecimal digits.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::with_capacity(64);
        for b in self.0 {
            write!(out, "{b:02x}")?;
        }
        f.write_str(&out)
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Digest({self})")
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use super::*;

    /// The SHA-256 of `bytes`, written in `pieces`-byte pieces.
    fn in_pieces(bytes: &[u8], pieces: usize) -> String {
        let mut h = Sha256::new();
        for piece in bytes.chunks(pieces) {
            h.write(piece);
        }
        hex(h.finish())
    }

    /// FIPS 180-4's examples (NIST's "SHA-256" example document and the standard's test vectors),
    /// whatever the size of the pieces the bytes arrive in.
    #[test]
    fn matches_the_standards_examples() {
        let cases = [
            (
                "",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                "abc",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
            (
                concat!(
                    "abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmn",
                    "hijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu",
                ),
                "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1",
            ),
        ];
        for (message, want) in cases {
            for pieces in [1, 3, 55, 56, 63, 64, 65, 1000] {
                assert_eq!(
                    in_pieces(message.as_bytes(), pieces),
                    want,
                    "{message:?} in {pieces}-byte pieces"
                );
            }
            assert_eq!(hex(sha256(message.as_bytes())), want);
        }
        let million = vec![b'a'; 1_000_000];
        let want = "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0";
        assert_eq!(in_pieces(&million, 4_096), want);
        assert_eq!(in_pieces(&million, 997), want);
    }

    /// Every padding case (a message ending anywhere in a block, the length in the same block or
    /// the next) gives the same digest written whole or a byte at a time, and a clone taken midway
    /// finishes to the digest of the prefix.
    #[test]
    fn every_length_and_every_split_agree() {
        let message: Vec<u8> = (0..130u32).map(|i| (i * 37 + 11) as u8).collect();
        for len in 0..=message.len() {
            let whole = sha256(&message[..len]);
            let mut bytewise = Sha256::new();
            for (at, b) in message[..len].iter().enumerate() {
                if at == len / 2 {
                    assert_eq!(bytewise.clone().finish(), sha256(&message[..at]));
                }
                bytewise.write(&[*b]);
            }
            assert_eq!(bytewise.finish(), whole, "length {len}");
        }
    }

    #[test]
    fn hex_round_trips_and_refuses_anything_else() {
        let d = sha256(b"abc");
        let text = hex(d);
        assert_eq!(text.len(), 64);
        assert_eq!(Digest::from_hex(&text), Some(Digest(d)));
        assert_eq!(Digest::from_hex(&text.to_uppercase()), Some(Digest(d)));
        assert!(Digest::is_canonical_hex(&text));
        assert!(!Digest::is_canonical_hex(&text.to_uppercase()));
        for bad in [
            String::new(),
            text[..63].to_owned(),
            format!("{text}0"),
            format!("g{}", &text[1..]),
            format!(" {}", &text[1..]),
            format!("+{}", &text[1..]),
        ] {
            assert_eq!(Digest::from_hex(&bad), None, "{bad:?}");
        }
        assert_eq!(format!("{:?}", Digest(d)), format!("Digest({text})"));
    }
}
