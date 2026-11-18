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
///
/// In parts (decision 32.78: whole, it was too long for one Miri gate part). The short and Monte
/// Carlo files are checked whole against NIST's digests. The long file is cut at record starts
/// into [`LONG_PARTS`](the_vector_files_are_as_published::LONG_PARTS), each part's SHA-256 and
/// record count pinned: every part hashes and reads only its own bytes, so together the parts hash
/// and read every byte of the file once, as the whole did, and the join test holds the pins to the
/// whole file and to NIST's digest of it.
mod the_vector_files_are_as_published {
    use super::{FILES, LONG, MONTE, SHORT, hex, messages, sha256};

    /// The long file's parts: the byte where each starts, its SHA-256 and the records it holds.
    /// Each part ends where the next starts, and the last at the file's end.
    pub const LONG_PARTS: [(usize, &str, usize); 8] = [
        (
            0,
            "fd2f6c7d1c077f1329777fb9a4d4dfe83068f6cd776383f55a69d35de81819ca",
            22,
        ),
        (
            55_139,
            "36c73461df633a4da30cd05a393a4a1169a65b1a02d2a3445ab7bcd2848029fc",
            9,
        ),
        (
            105_251,
            "079db3d0a1c6b3d74f7175719983a3a11e8349fab41d20002d4e02b7b11c01bb",
            8,
        ),
        (
            163_259,
            "44dbad1b116b6db1ccfde4183b8421040812cf250ed9c0292321bac38c3233d6",
            6,
        ),
        (
            215_081,
            "a44852a84f0d1fa36ad571fb8074890e2d1a203e210d51427fc7a6143594ffc9",
            5,
        ),
        (
            263_711,
            "4442b2b80698dc06a683a9d03c0d6949b44ee1c48cf99eb9ab750067c739aa4e",
            5,
        ),
        (
            317_291,
            "0ab550447497a1e146f17a5475eb493c4da14cff31472370695bb622ec6f8295",
            5,
        ),
        (
            375_821,
            "88dac0bbf3768243291fdc62646dd62b0276ed91ffe9c9d8df1dd0056c5df005",
            4,
        ),
    ];

    /// Part `k` of the long file, its bytes.
    fn long_part(k: usize) -> &'static str {
        let end = LONG_PARTS.get(k + 1).map_or(LONG.len(), |next| next.0);
        &LONG[LONG_PARTS[k].0..end]
    }

    /// Part `k` of the long file hashes to its pin and holds its pinned records.
    fn check_long_part(k: usize) {
        let (_, digest, records) = LONG_PARTS[k];
        let part = long_part(k);
        assert_eq!(hex(&sha256(part.as_bytes())), digest, "part {}", k + 1);
        assert_eq!(messages(part).len(), records, "part {}", k + 1);
    }

    #[test]
    fn the_short_and_monte_carlo_files() {
        for (file, want) in [FILES[0], FILES[2]] {
            assert_eq!(hex(&sha256(file.as_bytes())), want);
        }
        assert_eq!((FILES[0].0, FILES[2].0), (SHORT, MONTE));
        assert_eq!(messages(SHORT).len(), 65);
    }

    #[test]
    fn the_long_file_part_1() {
        check_long_part(0);
    }

    #[test]
    fn the_long_file_part_2() {
        check_long_part(1);
    }

    #[test]
    fn the_long_file_part_3() {
        check_long_part(2);
    }

    #[test]
    fn the_long_file_part_4() {
        check_long_part(3);
    }

    #[test]
    fn the_long_file_part_5() {
        check_long_part(4);
    }

    #[test]
    fn the_long_file_part_6() {
        check_long_part(5);
    }

    #[test]
    fn the_long_file_part_7() {
        check_long_part(6);
    }

    #[test]
    fn the_long_file_part_8() {
        check_long_part(7);
    }

    /// The parts join to the long file as NIST published it: the first starts at its start, each
    /// other at the start of a record, in order, and the last ends at its end; its SHA-256 is
    /// NIST's; and the parts' records, in order, are its 64.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "the whole long file, too long for one Miri gate part; Miri checks it in its parts"
    )]
    fn the_parts_join_to_the_published_file() {
        assert_eq!(LONG_PARTS[0].0, 0);
        for pair in LONG_PARTS.windows(2) {
            assert!(pair[0].0 < pair[1].0, "parts in order");
            assert!(
                LONG[..pair[1].0].ends_with('\n') && LONG[pair[1].0..].starts_with("Len = "),
                "part at {} starts a record",
                pair[1].0
            );
        }
        assert_eq!(FILES[1].0, LONG);
        assert_eq!(hex(&sha256(LONG.as_bytes())), FILES[1].1);
        let whole = messages(LONG);
        assert_eq!(whole.len(), 64);
        let parts: Vec<_> = (0..LONG_PARTS.len())
            .flat_map(|k| messages(long_part(k)))
            .collect();
        assert_eq!(parts, whole);
        assert_eq!(LONG_PARTS.iter().map(|p| p.2).sum::<usize>(), 64);
    }
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
