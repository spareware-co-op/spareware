//! Inputs for `geom-op`'s canonical CBOR encoder and SHA-256 (piece G86, decision 32.46): a small
//! map of every kind of scalar and a set, a row of doubles, and a message to hash.
//!
//! What it does: [`maps`] draws, per input, an integer, a double, a short text and a set of small
//! integers, which the benchmark writes as a map of four keys; [`floats`] draws eight doubles per
//! input, in turn ones that binary16 holds, ones that binary32 holds and ones only binary64 holds,
//! so the encoder's narrowing takes each of its three ends; [`messages`] draws 256 bytes per
//! input for SHA-256, four of its 64-byte blocks and the padding block.
//!
//! Guarantees: every typical batch is full, every value is finite and every set holds distinct
//! items, so every call succeeds (`tests/inputs.rs`), and each input has its own draws. Only the
//! typical class exists: the encoder's refusals (a non-finite double, a repeated set item, a key
//! out of order, an item written out of place) are tested in `geom-op` itself.
//!
//! Method: the values of `geom-op`'s own encoder tests (`tests/cbor.rs`) and of a names record,
//! cut small; each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few draws of [`SplitMix64`] per input, once per batch, outside every measured run.

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// The bytes of each message hashed.
pub const MESSAGE_BYTES: usize = 256;

/// The doubles in each row.
pub const ROW: usize = 8;

/// The integers in each set.
pub const SET: usize = 6;

/// A short text: three to twelve lower-case letters.
fn word(g: &mut SplitMix64) -> String {
    let len = 3 + g.index(10);
    (0..len)
        .map(|_| char::from(b'a' + u8::try_from(g.below(26)).unwrap_or(0)))
        .collect()
}

/// One map's values per call: an integer, a double, a text and a set of distinct small integers.
#[must_use]
pub fn maps(class: Class) -> Vec<(i64, f64, String, Vec<u64>)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("encoder::map", class);
    batch_of(|_| {
        let int = g.range(-(1 << 40), 1 << 40);
        let float = f64::from(u32::try_from(g.below(1 << 20)).ok()?) / 1024.0;
        let text = word(&mut g);
        let mut set: Vec<u64> = (0..SET).map(|_| g.below(1 << 16)).collect();
        set.sort_unstable();
        set.dedup();
        (set.len() == SET).then_some((int, float, text, set))
    })
}

/// A double that binary16 holds, one that binary32 holds and one that only binary64 holds, in turn.
fn double(g: &mut SplitMix64, k: usize) -> f64 {
    match k % 3 {
        // Eleven significant bits, a small exponent: binary16 holds it.
        0 => f64::from(u16::try_from(g.below(1 << 11)).unwrap_or(0)) / 64.0,
        // Twenty-four significant bits: binary32 holds it, binary16 does not.
        1 => f64::from(u32::try_from(g.below(1 << 24) | 1 << 23).unwrap_or(1)) / 4096.0,
        // A full significand in [1, 2): only binary64 holds it.
        _ => f64::from_bits(0x3ff0_0000_0000_0000 | (g.next_u64() >> 12) | 1),
    }
}

/// One row of [`ROW`] doubles per call, each its own kind in turn.
#[must_use]
pub fn floats(class: Class) -> Vec<Vec<f64>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("encoder::float", class);
    batch_of(|n| Some((0..ROW).map(|k| double(&mut g, n + k)).collect()))
}

/// One message of [`MESSAGE_BYTES`] bytes per call of `sha256`.
#[must_use]
pub fn messages(class: Class) -> Vec<Vec<u8>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("encoder::sha256", class);
    batch_of(|_| {
        Some(
            (0..MESSAGE_BYTES / 8)
                .flat_map(|_| g.next_u64().to_le_bytes())
                .collect(),
        )
    })
}
