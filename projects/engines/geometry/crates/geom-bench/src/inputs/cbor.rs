//! Inputs for `geom-format`'s CBOR decoder and text form (piece G91, decision 32.46): small items
//! of SKG's deterministic CBOR, their text, and the rows of short columns.
//!
//! What it does: [`item`] writes an array of [`MAPS`] maps, each of an integer, a text string of
//! three letters and a float, the item `benches/cbor.rs` in `geom-format` builds, cut to a few
//! maps; [`check`] and [`to_text`] give one such item per call, [`from_text`] its text, and
//! [`int_column`] and [`float_column`] the little-endian rows of a column of [`ROWS`] values, as a
//! tag 79 or tag 86 byte string holds them.
//!
//! Guarantees: every typical batch is full and every call succeeds (`tests/inputs.rs`), so each
//! benchmark measures the path that accepts: every item is canonical, every text reads back to its
//! item, every row is a whole value and every float is finite. Only the typical class exists: the
//! refusals are tested in `geom-format` itself (`tests/attack_g91.rs`).
//!
//! Method: the items and columns of `geom-format`'s wall-clock benchmark of piece G91
//! (`benches/cbor.rs`), cut small, so a batch costs from tens of thousands of instructions (a
//! column's rows) to a few million (reading a text), each measured as chapter 28.14 sets
//! benchmarks out, `projects/engines/geometry/architecture/28-testing.md`.
//! The board's 500 MB/s per core for column decoding is a wall-clock reading for the reference
//! runner (piece G107), not a count: this group gates the instructions only.
//!
//! Cost: O([`BATCH`](super::BATCH)) draws and one encoding per input, once per batch, outside
//! every measured run.

use geom_op::cbor::Encoder;
use geom_run::{Budget, WorkMeter};

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// The maps of one item.
pub const MAPS: u64 = 4;
/// The rows of one column.
pub const ROWS: usize = 32;
/// The depth every item is read at: the array and its maps.
pub const DEPTH: usize = 2;

/// A float of ordinary size, with a full significand.
fn float(g: &mut SplitMix64) -> f64 {
    let unit = (g.next_u64() >> 11) as f64 / (1_u64 << 53) as f64;
    unit * 1e6 - 5e5
}

/// The bytes of one item: [`MAPS`] maps of `{1: uint, 2: text, 3: float}`.
fn item(g: &mut SplitMix64) -> Option<Vec<u8>> {
    let mut e = Encoder::new();
    e.array(MAPS).ok()?;
    for _ in 0..MAPS {
        e.map(3).ok()?;
        e.key(1).ok()?;
        e.uint(g.below(1 << 20)).ok()?;
        e.key(2).ok()?;
        let letters: String = (0..3)
            .map(|_| char::from(b'a' + (g.below(26) as u8)))
            .collect();
        e.text(&letters).ok()?;
        e.key(3).ok()?;
        e.float(float(g)).ok()?;
    }
    Some(e.finish().ok()?.into_bytes())
}

/// One item per call of `check`.
#[must_use]
pub fn check(class: Class) -> Vec<Vec<u8>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("cbor::check", class);
    batch_of(|_| item(&mut g))
}

/// One item per call of `to_text`.
#[must_use]
pub fn to_text(class: Class) -> Vec<Vec<u8>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("cbor::to_text", class);
    batch_of(|_| item(&mut g))
}

/// The text of one item per call of `from_text`.
#[must_use]
pub fn from_text(class: Class) -> Vec<String> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("cbor::from_text", class);
    batch_of(|_| {
        let bytes = item(&mut g)?;
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        geom_format::cbor::to_text(&bytes, DEPTH, &mut meter).ok()
    })
}

/// The rows of one `i64` column per call of `int_column`.
#[must_use]
pub fn int_column(class: Class) -> Vec<Vec<u8>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("cbor::int_column", class);
    batch_of(|_| Some((0..ROWS).flat_map(|_| g.next_u64().to_le_bytes()).collect()))
}

/// The rows of one `f64` column per call of `float_column`.
#[must_use]
pub fn float_column(class: Class) -> Vec<Vec<u8>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("cbor::float_column", class);
    batch_of(|_| {
        Some(
            (0..ROWS)
                .flat_map(|_| float(&mut g).to_le_bytes())
                .collect(),
        )
    })
}
