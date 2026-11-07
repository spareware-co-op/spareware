//! Inputs for angle units (`geom_num::angle`, piece G359): angles for their sine and cosine,
//! enclosures in radians for the nearest unit, and integer vectors for their bearings.
//!
//! - `sin_cos`, class `typical`: angles anywhere in one turn, so every octant and both halves of
//!   each, its swap of sine and cosine, occur; class `extreme`: angles within 2^20 units of
//!   `i64::MIN` and `i64::MAX`, whose reduction to one turn is the whole of their work's integer
//!   part.
//! - `nearest_radians`, class `typical`: enclosures of whole numbers of units in (−π, π], as an
//!   `atan2` result gives them, computed by interval arithmetic from the unit.
//! - `of_vector`, class `typical`: vectors with components within ±2^30 steps, the difference of
//!   two typical grid points, off the axes and the diagonals, so each takes `atan2`'s path.
//! - `reduced`, class `typical`: any `i64`; class `extreme`: within 2^20 of either end.
//! - `rotated`, `reversed`, `from_ifc` and `to_ifc`, class `typical`: bearings anywhere in one turn,
//!   and angles anywhere in `i64` to turn them by or to read as IFC's.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a batch is
//! `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: one class per path of each operation, as chapter 28.14 sets benchmarks out
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: a few draws of [`SplitMix64`] per input, once per batch, outside every measured run.

use geom_num::angle::{AngleUnits, Bearing, FULL_TURN, HALF_TURN};
use geom_num::interval::Interval;

use super::{Class, TYPICAL_REACH, batch_of, rng};
use crate::rng::SplitMix64;

/// A value in `0..n`, for `n` below 2^62.
fn below(g: &mut SplitMix64, n: u64) -> i64 {
    #[allow(
        clippy::cast_possible_wrap,
        reason = "every caller passes n below 2^62, so the remainder fits i64"
    )]
    let v = (g.next_u64() % n) as i64;
    v
}

/// Angles for `AngleUnits::sin_cos`.
#[must_use]
pub fn sin_cos(class: Class) -> Vec<AngleUnits> {
    let mut g = rng("angle::sin_cos", class);
    match class {
        Class::Typical => batch_of(|_| Some(AngleUnits(below(&mut g, FULL_TURN.unsigned_abs())))),
        Class::Extreme => batch_of(|at| {
            let near = below(&mut g, 1 << 20);
            Some(AngleUnits(if at % 2 == 0 {
                i64::MAX - near
            } else {
                i64::MIN + near
            }))
        }),
        _ => Vec::new(),
    }
}

/// Enclosures in radians of whole numbers of units, for `AngleUnits::nearest_radians`.
#[must_use]
pub fn nearest_radians(class: Class) -> Vec<Interval> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("angle::nearest_radians", class);
    batch_of(|_| {
        let units = below(&mut g, 2 * HALF_TURN.unsigned_abs()) - HALF_TURN + 1;
        #[allow(
            clippy::cast_precision_loss,
            reason = "within ±HALF_TURN, below 2^31, so the conversion is exact"
        )]
        let at = Interval::point(units as f64).ok()?;
        #[allow(
            clippy::cast_precision_loss,
            reason = "HALF_TURN is below 2^31, so the conversion is exact"
        )]
        let half = Interval::point(HALF_TURN as f64).ok()?;
        at.mul(Interval::PI).ok()?.div(half).ok()
    })
}

/// Vectors (east, north) off the axes and the diagonals, for `Bearing::of_vector`.
#[must_use]
pub fn of_vector(class: Class) -> Vec<(i64, i64)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("angle::of_vector", class);
    let reach = 2 * TYPICAL_REACH.unsigned_abs() + 1;
    batch_of(|_| {
        let east = below(&mut g, reach) - TYPICAL_REACH;
        let north = below(&mut g, reach) - TYPICAL_REACH;
        (east != 0 && north != 0 && east.unsigned_abs() != north.unsigned_abs())
            .then_some((east, north))
    })
}

/// Any `i64` (`typical`), or within 2^20 of either end (`extreme`).
fn any_angle(op: &str, class: Class) -> Vec<AngleUnits> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            #[allow(
                clippy::cast_possible_wrap,
                reason = "a draw of 64 bits read as two's complement: every i64"
            )]
            let v = g.next_u64() as i64;
            Some(AngleUnits(v))
        }),
        Class::Extreme => batch_of(|at| {
            let near = below(&mut g, 1 << 20);
            Some(AngleUnits(if at % 2 == 0 {
                i64::MAX - near
            } else {
                i64::MIN + near
            }))
        }),
        _ => Vec::new(),
    }
}

/// Bearings anywhere in one turn.
fn bearings(op: &str, class: Class) -> Vec<Bearing> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(op, class);
    batch_of(|_| {
        Some(Bearing::new(AngleUnits(below(
            &mut g,
            FULL_TURN.unsigned_abs(),
        ))))
    })
}

/// Angles for `AngleUnits::reduced`.
#[must_use]
pub fn reduced(class: Class) -> Vec<AngleUnits> {
    any_angle("angle::reduced", class)
}

/// Bearings and angles to turn them by, for `Bearing::rotated`.
#[must_use]
pub fn rotated(class: Class) -> Vec<(Bearing, AngleUnits)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let by = any_angle("angle::rotated_by", class);
    bearings("angle::rotated", class)
        .into_iter()
        .zip(by)
        .collect()
}

/// Bearings for `Bearing::reversed`.
#[must_use]
pub fn reversed(class: Class) -> Vec<Bearing> {
    bearings("angle::reversed", class)
}

/// IFC angles for `Bearing::from_ifc`.
#[must_use]
pub fn from_ifc(class: Class) -> Vec<AngleUnits> {
    if class != Class::Typical {
        return Vec::new();
    }
    any_angle("angle::from_ifc", class)
}

/// Bearings for `Bearing::to_ifc`.
#[must_use]
pub fn to_ifc(class: Class) -> Vec<Bearing> {
    bearings("angle::to_ifc", class)
}
