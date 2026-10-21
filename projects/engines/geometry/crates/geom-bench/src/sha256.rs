//! SHA-256, the hash of every benchmark's golden digests.
//!
//! The kernel's golden hashes are SHA-256 (chapters 21 and 28.13), so the benchmarks' output hashes
//! are too (chapter 23.13, "Output hash"). This is a plain, portable implementation: 32-bit integer
//! operations only, the same digest on every target, no `unsafe`, no indexing that can panic.
//!
//! Method: the Secure Hash Standard, NIST FIPS 180-4, sections 4.1.2 (functions), 4.2.2
//! (constants), 5.1.1 (padding), 5.3.3 (initial value) and 6.2.2 (the compression function):
//! <https://csrc.nist.gov/pubs/fips/180-4/upd1/final>. The message schedule is kept as a sliding
//! window of sixteen words, which computes the same W_t as the standard's 64-word array. The tests
//! check the standard's own examples (<https://csrc.nist.gov/projects/cryptographic-standards-and-guidelines/example-values>).
//!
//! Guarantees: the digest of every message below 2^61 bytes (2^64 bits, the standard's limit) is
//! the one FIPS 180-4 defines, whatever pieces the message is fed in (the tests check the
//! standard's examples, a million bytes in uneven pieces, and every padding length); and no
//! method panics.
//!
//! Cost: one compression (64 rounds of 32-bit operations) per 64-byte block, so linear in the
//! message; the state is the eight hash words, one block and two counters, and nothing is
//! allocated.

use core::fmt;

/// The 64 round constants of FIPS 180-4, section 4.2.2: the first 32 bits of the fractional parts
/// of the cube roots of the first 64 primes.
const K: [u32; 64] = [
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

/// The initial hash value of FIPS 180-4, section 5.3.3: the first 32 bits of the fractional parts
/// of the square roots of the first eight primes.
const H0: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

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
#[derive(Clone, Debug)]
pub struct Sha256 {
    /// The hash value so far (H in FIPS 180-4).
    state: [u32; 8],
    /// Bytes of the current, incomplete 64-byte block.
    block: [u8; 64],
    /// How many bytes of `block` are filled: always below 64 between calls.
    filled: usize,
    /// The message length so far, in bytes. FIPS 180-4 allows messages below 2^64 bits.
    length: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Sha256::new()
    }
}

impl Sha256 {
    /// A computation over the empty message.
    #[must_use]
    pub const fn new() -> Sha256 {
        Sha256 {
            state: H0,
            block: [0; 64],
            filled: 0,
            length: 0,
        }
    }

    /// Appends `bytes` to the message.
    pub fn update(&mut self, bytes: &[u8]) {
        // usize is at most 64 bits on every supported target, so the conversion is exact.
        let added = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        self.length = self.length.wrapping_add(added);
        for &byte in bytes {
            if let Some(slot) = self.block.get_mut(self.filled) {
                *slot = byte;
            }
            self.filled += 1;
            if self.filled == 64 {
                compress(&mut self.state, &self.block);
                self.filled = 0;
            }
        }
    }

    /// The digest of the message (FIPS 180-4, section 5.1.1 padding, then the last blocks).
    #[must_use]
    pub fn finish(mut self) -> Digest {
        let bits = self.length.wrapping_mul(8);
        self.update(&[0x80]);
        while self.filled != 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        // 32 bytes are exactly eight 4-byte words, one per word of the state.
        let mut out = [0u8; 32];
        let (words, _) = out.as_chunks_mut::<4>();
        for (chunk, word) in words.iter_mut().zip(self.state) {
            *chunk = word.to_be_bytes();
        }
        Digest(out)
    }

    /// The digest of `bytes`, in one call.
    #[must_use]
    pub fn digest(bytes: &[u8]) -> Digest {
        let mut h = Sha256::new();
        h.update(bytes);
        h.finish()
    }
}

/// The compression function of FIPS 180-4, section 6.2.2, on one 64-byte block.
fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    // W_0 .. W_15 are the block's words, big-endian (step 1).
    // A 64-byte block is exactly sixteen 4-byte words.
    let mut window = [0u32; 16];
    let (words, _) = block.as_chunks::<4>();
    for (word, &bytes) in window.iter_mut().zip(words) {
        *word = u32::from_be_bytes(bytes);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for &k in &K {
        // The window holds W_t .. W_t+15. W_t+16 = σ1(W_t+14) + W_t+9 + σ0(W_t+1) + W_t, the
        // standard's recurrence shifted by 16, so after the shift the window holds W_t+1 .. W_t+16.
        let [w0, w1, _, _, _, _, _, _, _, w9, _, _, _, _, w14, _] = window;
        let next = small_sigma1(w14)
            .wrapping_add(w9)
            .wrapping_add(small_sigma0(w1))
            .wrapping_add(w0);
        let t1 = h
            .wrapping_add(big_sigma1(e))
            .wrapping_add(ch(e, f, g))
            .wrapping_add(k)
            .wrapping_add(w0);
        let t2 = big_sigma0(a).wrapping_add(maj(a, b, c));
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
        window.rotate_left(1);
        if let Some(last) = window.last_mut() {
            *last = next;
        }
    }
    for (word, add) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *word = word.wrapping_add(add);
    }
}

/// Ch(x, y, z), FIPS 180-4 (4.2).
const fn ch(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (!x & z)
}

/// Maj(x, y, z), FIPS 180-4 (4.3).
const fn maj(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (x & z) ^ (y & z)
}

/// Σ0(x), FIPS 180-4 (4.4).
const fn big_sigma0(x: u32) -> u32 {
    x.rotate_right(2) ^ x.rotate_right(13) ^ x.rotate_right(22)
}

/// Σ1(x), FIPS 180-4 (4.5).
const fn big_sigma1(x: u32) -> u32 {
    x.rotate_right(6) ^ x.rotate_right(11) ^ x.rotate_right(25)
}

/// σ0(x), FIPS 180-4 (4.6).
const fn small_sigma0(x: u32) -> u32 {
    x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3)
}

/// σ1(x), FIPS 180-4 (4.7).
const fn small_sigma1(x: u32) -> u32 {
    x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10)
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
