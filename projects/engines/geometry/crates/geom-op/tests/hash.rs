//! SHA-256 and its 128-bit cut against NIST's published test vectors.
//!
//! The vectors are the Cryptographic Algorithm Validation Program's byte-oriented SHA-256 files
//! (`SHA256ShortMsg.rsp`, `SHA256LongMsg.rsp` and the Monte Carlo chain `SHA256Monte.rsp`, CAVS
//! 11.0 and 11.1), from NIST's archive `shabytetestvectors.zip`
//! (<https://csrc.nist.gov/projects/cryptographic-algorithm-validation-program/secure-hashing>),
//! kept in `tests/vectors/` byte for byte as published. They are works of the United States
//! government, in the public domain there, and are vendored under chapter 29's rule for public
//! data, with this credit. The Monte Carlo chain follows NIST's SHAVS document, section 6.4
//! (<https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/shs/SHAVS.pdf>).

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
#![allow(
    clippy::arithmetic_side_effects,
    reason = "test cases and splits; the rules are for kernel code"
)]

mod common;

use common::{hex, unhex};
use geom_op::hash::{Sha256, sha256, sha256_128};

/// The vector files, and the SHA-256 of each as NIST's archive holds it (checked by
/// `the_vector_files_are_as_published`).
const SHORT: &str = include_str!("vectors/SHA256ShortMsg.rsp");
const LONG: &str = include_str!("vectors/SHA256LongMsg.rsp");
const MONTE: &str = include_str!("vectors/SHA256Monte.rsp");
const FILES: [(&str, &str); 3] = [
    (
        SHORT,
        "75e1cb83994638481808e225b9eb0c1ebd0c232d952ac42b61abce6363be283c",
    ),
    (
        LONG,
        "6fac36f37360bcf74ffcf4465c18e30d6d5a04cc90885b901fc3130c16060974",
    ),
    (
        MONTE,
        "29ea30c6bb4b84e425fb8c1d731c6bb852dac935825f2bd1143e5d3c4f10bfb9",
    ),
];

/// The `(message, digest)` pairs of a `Len`/`Msg`/`MD` file. `Len = 0` writes its empty message
/// as `Msg = 00`.
fn messages(file: &str) -> Vec<(Vec<u8>, Vec<u8>)> {
    let text = file.replace("\r\n", "\n");
    let mut out = Vec::new();
    let mut len = None;
    let mut msg = None;
    for line in text.lines() {
        if let Some(bits) = line.strip_prefix("Len = ") {
            len = Some(bits.parse::<usize>().unwrap());
        } else if let Some(m) = line.strip_prefix("Msg = ") {
            msg = Some(unhex(m));
        } else if let Some(md) = line.strip_prefix("MD = ") {
            let bytes = len.take().unwrap() / 8;
            let mut m = msg.take().unwrap();
            m.truncate(bytes);
            assert_eq!(m.len(), bytes);
            out.push((m, unhex(md)));
        }
    }
    out
}

/// Every file is the one NIST published, so a vector cannot be edited to pass.
#[test]
fn the_vector_files_are_as_published() {
    for (file, want) in FILES {
        assert_eq!(hex(&sha256(file.as_bytes())), want);
    }
    assert_eq!(messages(SHORT).len(), 65);
    assert_eq!(messages(LONG).len(), 64);
}

/// Every short and long message, whole and written in pieces of every length up to 70 bytes
/// (each padding edge, 55, 56 and 64, falls inside a piece and between two), and its 128-bit cut.
#[test]
fn nist_short_and_long_messages() {
    for (message, digest) in messages(SHORT).into_iter().chain(messages(LONG)) {
        assert_eq!(sha256(&message).as_slice(), digest.as_slice());
        assert_eq!(sha256_128(&message).as_slice(), &digest[..16]);
        let mut whole = Sha256::new();
        whole.write(&message);
        assert_eq!(whole.clone().finish_128().as_slice(), &digest[..16]);
        assert_eq!(whole.finish().as_slice(), digest.as_slice());
        for piece in 1..=70 {
            let mut hasher = Sha256::new();
            for chunk in message.chunks(piece) {
                hasher.write(chunk);
            }
            assert_eq!(
                hasher.finish().as_slice(),
                digest.as_slice(),
                "pieces of {piece}"
            );
        }
    }
}

/// NIST's Monte Carlo chain: 100 checkpoints of 1,000 hashes each, every message the three
/// digests before it (SHAVS section 6.4).
#[test]
fn nist_monte_carlo_chain() {
    let text = MONTE.replace("\r\n", "\n");
    let seed = text
        .lines()
        .find_map(|line| line.strip_prefix("Seed = "))
        .unwrap();
    let checkpoints: Vec<Vec<u8>> = text
        .lines()
        .filter_map(|line| line.strip_prefix("MD = "))
        .map(unhex)
        .collect();
    assert_eq!(checkpoints.len(), 100);
    let mut seed: [u8; 32] = unhex(seed).try_into().unwrap();
    for checkpoint in &checkpoints {
        let (mut a, mut b, mut c) = (seed, seed, seed);
        for _ in 3..1003 {
            let mut hasher = Sha256::new();
            hasher.write(&a);
            hasher.write(&b);
            hasher.write(&c);
            let next = hasher.finish();
            (a, b, c) = (b, c, next);
        }
        seed = c;
        assert_eq!(seed.as_slice(), checkpoint.as_slice());
    }
}

/// FIPS 180-4's own examples: "abc", the 448-bit two-block message and one million "a"s.
#[test]
fn fips_180_4_examples() {
    assert_eq!(
        hex(&sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        hex(&sha256(
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
        )),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    let mut hasher = Sha256::default();
    for _ in 0..1000 {
        hasher.write(&[b'a'; 1000]);
    }
    assert_eq!(
        hex(&hasher.finish()),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
    assert_eq!(hex(&sha256_128(b"abc")), "ba7816bf8f01cfea414140de5dae2223");
}

/// A clone carries the state: the digest of a prefix can be taken while hashing goes on.
#[test]
fn a_clone_finishes_the_prefix() {
    let mut hasher = Sha256::new();
    hasher.write(b"ab");
    let prefix = hasher.clone();
    hasher.write(b"c");
    assert_eq!(prefix.finish(), sha256(b"ab"));
    assert_eq!(hasher.finish(), sha256(b"abc"));
}
