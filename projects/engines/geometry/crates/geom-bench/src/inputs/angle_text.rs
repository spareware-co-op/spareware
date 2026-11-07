//! Inputs for angles as text (`geom_num::angle_text`, piece G359): readings in degrees, minutes
//! and seconds to read, and angles to write.
//!
//! - `parse`, class `typical`: readings to 0.01″ anywhere in one turn, half of them negative,
//!   written as a total station's export writes them, `123°45'06.78"`.
//! - `write`, class `typical`: angles anywhere within ±10 turns, written to 0.01″, so most round.
//! - `shortest`, class `typical`: the same, in their shortest exact text.
//! - `parse_quadrant`, class `typical`: quadrant bearings to 0.01″ in all four quadrants,
//!   `N 12°34'56.78" E`.
//! - `write_quadrant`, class `typical`: bearings anywhere in one turn, to 0.01″.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a batch is
//! `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: one class per operation, as chapter 28.14 sets benchmarks out
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: a few draws of [`SplitMix64`] and one short string per input, once per batch, outside
//! every measured run.

use geom_num::angle::{AngleUnits, Bearing, FULL_TURN};

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// A value in `0..n`, for `n` below 2^62.
fn below(g: &mut SplitMix64, n: u64) -> u64 {
    g.next_u64() % n
}

/// Readings in degrees, minutes and seconds, for `angle_text::parse`.
#[must_use]
pub fn parse(class: Class) -> Vec<String> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("angle_text::parse", class);
    batch_of(|at| {
        let hundredths = below(&mut g, 360 * 3600 * 100);
        let (d, m, s, h) = (
            hundredths / 360_000,
            hundredths / 6_000 % 60,
            hundredths / 100 % 60,
            hundredths % 100,
        );
        let sign = if at % 2 == 0 { "" } else { "-" };
        Some(format!("{sign}{d}\u{b0}{m:02}'{s:02}.{h:02}\""))
    })
}

/// Angles within ±10 turns.
fn angles(op: &str, class: Class) -> Vec<AngleUnits> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(op, class);
    let span = 20 * FULL_TURN.unsigned_abs();
    batch_of(|_| {
        #[allow(
            clippy::cast_possible_wrap,
            reason = "below 20 turns, under 2^36, so it fits i64"
        )]
        let v = below(&mut g, span) as i64;
        Some(AngleUnits(v - 10 * FULL_TURN))
    })
}

/// Angles for `angle_text::write`, to 0.01″.
#[must_use]
pub fn write(class: Class) -> Vec<AngleUnits> {
    angles("angle_text::write", class)
}

/// Angles for `angle_text::shortest`.
#[must_use]
pub fn shortest(class: Class) -> Vec<AngleUnits> {
    angles("angle_text::shortest", class)
}

/// Quadrant bearings, for `angle_text::parse_quadrant`.
#[must_use]
pub fn parse_quadrant(class: Class) -> Vec<String> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("angle_text::parse_quadrant", class);
    let letters = [("N", "E"), ("S", "E"), ("S", "W"), ("N", "W")];
    batch_of(|at| {
        let hundredths = below(&mut g, 90 * 3600 * 100);
        let (d, m, s, h) = (
            hundredths / 360_000,
            hundredths / 6_000 % 60,
            hundredths / 100 % 60,
            hundredths % 100,
        );
        let (from, to) = letters.get(at % 4).copied()?;
        Some(format!("{from} {d}\u{b0}{m:02}'{s:02}.{h:02}\" {to}"))
    })
}

/// Bearings for `angle_text::write_quadrant`, to 0.01″.
#[must_use]
pub fn write_quadrant(class: Class) -> Vec<Bearing> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("angle_text::write_quadrant", class);
    batch_of(|_| {
        #[allow(
            clippy::cast_possible_wrap,
            reason = "below a turn, under 2^32, so it fits i64"
        )]
        let v = below(&mut g, FULL_TURN.unsigned_abs()) as i64;
        Some(Bearing::new(AngleUnits(v)))
    })
}
