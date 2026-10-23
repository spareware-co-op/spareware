//! SHA-256 (FIPS 180-4), the hash chapters 21 and 28.13 specify for golden hashes and digests,
//! checked here against the standard's own test vectors and constants.
//!
//! **This is a copy of `geom-num`'s SHA-256**, from `geom-num`'s `tests/determinism.rs`: its
//! constants, `Sha256`, `hex`, and the two tests that check them with their helpers, documentation
//! included, copied unchanged except that the items the test files use are `pub`. It stays a copy
//! until the golden store (piece G19, "Determinism matrix and golden store") holds the one shared
//! copy every kernel crate's goldens use; then this file is deleted and the test files use that
//! one. Until then, a change to either copy is made to both.
//!
//! The test files that hash with it (`determinism.rs` and `attack_g03.rs`) include it with
//! `#[path = "common/sha256.rs"]`, so its test vectors run in each of them.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    clippy::single_match_else,
    clippy::unreadable_literal,
    reason = "test cases and oracles, named as in the formulas they check; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken hash must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
#![allow(
    clippy::arithmetic_side_effects,
    reason = "SHA-256's own arithmetic: test support, not kernel code"
)]
// geom-num declares `rust-version = "1.86"`, so clippy does not suggest these there; geom-math
// declares none, so clippy suggests them here. The copy keeps geom-num's code as it is.
#![allow(
    clippy::chunks_exact_to_as_chunks,
    clippy::manual_is_multiple_of,
    reason = "`as_chunks` (1.88) and `is_multiple_of` (1.87) are newer than geom-num's 1.86"
)]

use std::fmt::Write;

/// SHA-256's initial hash value: the first 32 bits of the fractional parts of the square roots of
/// the first eight primes (FIPS 180-4, section 5.3.3). Checked against that definition below.
const SHA256_INITIAL: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

/// SHA-256's round constants: the first 32 bits of the fractional parts of the cube roots of the
/// first 64 primes (FIPS 180-4, section 4.2.2). Checked against that definition below.
const SHA256_ROUND: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

/// SHA-256, as FIPS 180-4 (*Secure Hash Standard*, NIST, 2015) specifies it: the message is
/// padded with a one bit, zeros and its length in bits (section 5.1.1), and compressed 512 bits at
/// a time by the function of section 6.2.2. Bytes are taken as they come, so the whole corpus is
/// never held in memory. Every addition is modulo 2^32, as the standard defines it.
#[derive(Clone)]
pub struct Sha256 {
    /// The hash value so far, H(i).
    state: [u32; 8],
    /// The block being filled.
    block: [u8; 64],
    /// Bytes in `block`.
    filled: usize,
    /// Message bytes so far.
    length: u64,
}

impl Sha256 {
    /// The hash of the empty message so far.
    pub const fn new() -> Sha256 {
        Sha256 {
            state: SHA256_INITIAL,
            block: [0; 64],
            filled: 0,
            length: 0,
        }
    }

    /// Takes the message's next bytes.
    pub fn write(&mut self, bytes: &[u8]) {
        self.length += u64::try_from(bytes.len()).unwrap();
        for &b in bytes {
            self.push(b);
        }
    }

    /// One byte into the block, compressing a full block.
    fn push(&mut self, b: u8) {
        self.block[self.filled] = b;
        self.filled += 1;
        if self.filled == 64 {
            self.compress();
            self.filled = 0;
        }
    }

    /// The compression function on the full block (FIPS 180-4, section 6.2.2).
    fn compress(&mut self) {
        let mut w = [0u32; 64];
        for (word, bytes) in w.iter_mut().zip(self.block.chunks_exact(4)) {
            *word = u32::from_be_bytes(bytes.try_into().unwrap());
        }
        for t in 16..64 {
            let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
            let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
            w[t] = w[t - 16]
                .wrapping_add(s0)
                .wrapping_add(w[t - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for (&k, &wt) in SHA256_ROUND.iter().zip(&w) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(choose)
                .wrapping_add(k)
                .wrapping_add(wt);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (state, v) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *state = state.wrapping_add(v);
        }
    }

    /// The digest: the message padded (section 5.1.1) and its last blocks compressed.
    pub fn finish(mut self) -> [u8; 32] {
        let bits = self.length.wrapping_mul(8);
        self.push(0x80);
        while self.filled != 56 {
            self.push(0);
        }
        for b in bits.to_be_bytes() {
            self.push(b);
        }
        let mut digest = [0u8; 32];
        for (out, word) in digest.chunks_exact_mut(4).zip(self.state) {
            out.copy_from_slice(&word.to_be_bytes());
        }
        digest
    }
}

/// A digest in hexadecimal, as `GOLDEN` is written.
pub fn hex(digest: [u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for b in digest {
        write!(out, "{b:02x}").unwrap();
    }
    out
}

/// The SHA-256 of `bytes`, written in `pieces`-byte pieces.
fn sha256_in_pieces(bytes: &[u8], pieces: usize) -> String {
    let mut h = Sha256::new();
    for piece in bytes.chunks(pieces) {
        h.write(piece);
    }
    hex(h.finish())
}

/// SHA-256 gives the digests of FIPS 180-4's examples (NIST's "SHA-256" example document and the
/// standard's test vectors): the empty message, "abc" (one block), the 448-bit and 896-bit
/// messages (two blocks, padding in a block of its own) and one million "a"s; whatever the size of
/// the pieces the bytes arrive in.
#[test]
fn sha256_matches_the_standards_test_vectors() {
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
            let got = sha256_in_pieces(message.as_bytes(), pieces);
            assert_eq!(got, want, "{message:?} in {pieces}-byte pieces");
        }
    }
    let million = vec![b'a'; 1_000_000];
    let want = "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0";
    assert_eq!(sha256_in_pieces(&million, 4_096), want);
}

/// The first `n` primes.
fn primes(n: usize) -> Vec<u128> {
    let mut out: Vec<u128> = Vec::new();
    let mut candidate = 2u128;
    while out.len() < n {
        if out.iter().all(|p| candidate % p != 0) {
            out.push(candidate);
        }
        candidate += 1;
    }
    out
}

/// The first 32 bits of the fractional part of the `k`-th root of `p`: the integer `k`-th root
/// of p · 2^(32k), by bisection, modulo 2^32. Exact in `u128` for the primes used (p · 2^96 below
/// 2^105).
fn root_fraction_bits(p: u128, k: u32) -> u32 {
    let target = p << (32 * k);
    let (mut lo, mut hi) = (0u128, 1u128 << 40);
    while hi - lo > 1 {
        let mid = lo.midpoint(hi);
        if mid.pow(k) <= target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    u32::try_from(lo & 0xffff_ffff).unwrap()
}

/// SHA-256's constants are what FIPS 180-4 defines them to be, computed here from the primes, so
/// a mistyped constant cannot hide behind a matching test vector.
#[test]
fn sha256_constants_follow_their_definitions() {
    let initial: Vec<u32> = primes(8)
        .iter()
        .map(|&p| root_fraction_bits(p, 2))
        .collect();
    assert_eq!(initial, SHA256_INITIAL);
    let round: Vec<u32> = primes(64)
        .iter()
        .map(|&p| root_fraction_bits(p, 3))
        .collect();
    assert_eq!(round, SHA256_ROUND);
}
