//! The generators aimed at the hard cases of chapter 28.5, shared by the fuzz targets.
//!
//! What it does: each function reads a fuzzer's bytes and gives a coordinate, a point, a length or
//! a prism's base that lands on a hard case far more often than chance: coordinates on the coarse
//! sub-grids people draw on, so coincident faces, shared edges and tangencies are common; one grid
//! step off them, so near-coincidences are common too; the grid's own edge; and one-step sizes.
//! [`beside`] places a point on one a case already holds, or one step from it, which makes a
//! coincidence certain rather than likely. [`repeats`] gives the extreme count of 28.5: as many
//! copies as a wall has openings.
//!
//! Guarantees:
//!
//! - **Any bytes are a value**, in range for its type; the functions never fail.
//! - **Fixed reads.** Each function reads a fixed number of bytes, stated by its constant, so a
//!   target's input bound can be derived; a missing byte reads as zero ([`ByteSource`]).
//! - **Every hard case is reachable from one rule byte**, tested in `tests/adversarial.rs`.
//!
//! Method: chapter 28.5's "Generators aimed at the hard cases"
//! (`projects/engines/geometry/architecture/28-testing.md`): `SolidCraft`'s coarse placements, and
//! the full ±2^40 range that `Manifold`'s fuzzer leaves out, as report 01 section 4 sets out
//! (`projects/kit/notes/G01-evidence/01-manifold-mesh-booleans.md`).
//!
//! Cost: constant per value; [`base`] and [`repeats`] are linear in what they return.

use geom_num::grid::COORD_MAX;
use geom_num::{Coord, P3, Steps};
use geom_test::{ByteSource, Entropy};

/// The grid steps of one millimetre at the building step `Q0`: 6,400 to the millimetre.
pub const MILLIMETRE: i64 = 6_400;

/// The grid steps of 2.5 mm.
pub const TWO_AND_A_HALF: i64 = 2_500 * MILLIMETRE / 1_000;

/// The grid steps of 10 mm.
pub const TEN_MILLIMETRES: i64 = 10 * MILLIMETRE;

/// The coarse sub-grids a coordinate lands on: 1 mm, 2.5 mm and 10 mm.
pub const PITCHES: [i64; 3] = [MILLIMETRE, TWO_AND_A_HALF, TEN_MILLIMETRES];

/// The bytes one coordinate or length reads: a rule and a word.
pub const VALUE_BYTES: usize = 1 + 8;

/// The bytes a grid point reads.
pub const POINT_BYTES: usize = 3 * VALUE_BYTES;

/// The most corners a prism's base has.
pub const MAX_CORNERS: u8 = 8;

/// The grid's origin.
pub const ORIGIN: P3 = P3 {
    x: Coord::ZERO,
    y: Coord::ZERO,
    z: Coord::ZERO,
};

/// The coarse sub-grid a rule byte names.
fn pitch(rule: u8) -> i64 {
    match (rule >> 3) % 3 {
        0 => MILLIMETRE,
        1 => TWO_AND_A_HALF,
        _ => TEN_MILLIMETRES,
    }
}

/// A coordinate through its rule byte: small, on a coarse sub-grid, one step off one, at the
/// grid's edge, or anything.
pub fn coordinate(src: &mut ByteSource<'_>) -> i64 {
    let rule = src.byte();
    let any = i64::from_le_bytes(src.word().to_le_bytes()) % (1 << 40);
    let fine = (any % 1024).saturating_mul(pitch(rule));
    match rule % 6 {
        0 => any % 64,
        1 => fine,
        2 => fine.saturating_add(1),
        3 => COORD_MAX.saturating_sub((any % 8).saturating_abs()),
        4 => (any % 8).saturating_abs().saturating_sub(COORD_MAX),
        _ => any,
    }
}

/// A grid point.
pub fn point(src: &mut ByteSource<'_>) -> P3 {
    let (x, y, z) = (coordinate(src), coordinate(src), coordinate(src));
    P3::new(x, y, z).unwrap_or(ORIGIN)
}

/// A length through its rule byte: one step, small, on a coarse sub-grid, or past the grid.
pub fn length(src: &mut ByteSource<'_>) -> Steps {
    let rule = src.byte();
    let word = src.word();
    let unsigned = |p: i64| p.unsigned_abs();
    Steps(match rule % 6 {
        0 => 1,
        1 => word % 16,
        2 => (word % 1024).saturating_mul(unsigned(pitch(rule))),
        3 => word % 1_000_000,
        4 => (word % (1 << 41)).saturating_add(1),
        _ => word,
    })
}

/// A prism's base: most corners a fuzzer writes are refused, so when the count divides by three
/// they are replaced by points on a parabola, convex by construction.
pub fn base(src: &mut ByteSource<'_>) -> Vec<P3> {
    let count = usize::from(src.byte() % (MAX_CORNERS + 1));
    let mut corners = Vec::with_capacity(count);
    for _ in 0..count {
        corners.push(point(src));
    }
    let anchor = corners.first().copied().unwrap_or(ORIGIN);
    let z = anchor.z.get();
    if count % 3 == 0 {
        let (x, y) = (anchor.x.get() % 4096, anchor.y.get() % 4096);
        return (0..count)
            .filter_map(|k| {
                let k = i64::try_from(k).ok()?;
                let dy = k.saturating_mul(k);
                P3::new(x.saturating_add(k), y.saturating_add(dy), z).ok()
            })
            .collect();
    }
    corners
        .iter()
        .filter_map(|c| P3::new(c.x.get(), c.y.get(), z).ok())
        .collect()
}

/// What [`nudge`] reads: one byte, each pair of its bits one coordinate's move of −1, 0 or +1.
pub const NUDGE_BYTES: usize = 1;

/// One coordinate's move: −1, 0 or +1 step, the middle value twice as likely.
const fn sign(bits: u8) -> i64 {
    match bits & 3 {
        0 => -1,
        3 => 1,
        _ => 0,
    }
}

/// How far from a point [`beside`] lands, along x, y and z, from one byte.
pub fn nudge(src: &mut ByteSource<'_>) -> [i64; 3] {
    let byte = src.byte();
    [sign(byte), sign(byte >> 2), sign(byte >> 4)]
}

/// `anchor`, moved by `nudge`: the point itself or one step from it, kept in the grid's range.
#[must_use]
pub fn beside(anchor: P3, nudge: [i64; 3]) -> P3 {
    let [x, y, z] = nudge;
    let moved = P3::new(
        anchor.x.get().saturating_add(x),
        anchor.y.get().saturating_add(y),
        anchor.z.get().saturating_add(z),
    );
    moved.unwrap_or(anchor)
}

/// The moves of `count` copies in a row along x, `gap` apart: 300 of them are the 300 openings of
/// chapter 28.5's extreme wall.
#[must_use]
pub fn repeats(count: u16, gap: i64) -> Vec<[i64; 3]> {
    (1..=i64::from(count))
        .map(|k| [k.saturating_mul(gap), 0, 0])
        .collect()
}
