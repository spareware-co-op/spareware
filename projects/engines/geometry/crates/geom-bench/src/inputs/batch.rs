//! Inputs for the batch runner's hot paths (piece G104, decision 32.44): `geom-op`'s driver over a
//! list of items, and the flat arrays of a curve's and a surface's points.
//!
//! Class `typical` only: each [`run_items`] input is [`ITEMS`] distinct items, each [`curve_points`]
//! input [`POINTS`] parameters on the helix and each [`surface_points`] input [`POINTS`] pairs on
//! the torus of `curve.rs` and `surface.rs`. The driver's refusals (repeated keys, a cancelled
//! batch, a budget run out) are tested in `geom-op` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the batches of `geom-op`'s tests (`tests/batch.rs`) and of the flat arrays' tests, at
//! one size, measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: [`ITEMS`] or [`POINTS`] draws of [`SplitMix64`] per input, once per batch, outside every
//! measured run.

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// The items of one call of the driver.
pub const ITEMS: usize = 32;
/// The points of one flat array.
pub const POINTS: usize = 16;

/// A value in [0, 1) at 2^−52 resolution.
fn unit(g: &mut SplitMix64) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "the value is below 2^53, so the conversion is exact"
    )]
    let v = (g.next_u64() >> 12) as f64;
    v / (1u64 << 52) as f64
}

/// Items for `Batch::run`, each a distinct number that names its key and sets its work.
#[must_use]
pub fn run_items(class: Class) -> Vec<Vec<u64>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("batch::run_items", class);
    batch_of(|_| {
        let base = g.next_u64() >> 8;
        Some((0..ITEMS as u64).map(|i| (base << 8) | i).collect())
    })
}

/// Parameters for `ParamCurve::points` on the helix.
#[must_use]
pub fn curve_points(class: Class) -> Vec<Vec<f64>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("batch::curve_points", class);
    batch_of(|_| Some((0..POINTS).map(|_| 8.0 * unit(&mut g)).collect()))
}

/// Parameter pairs, flat, for `ParamSurface::points` on the torus.
#[must_use]
pub fn surface_points(class: Class) -> Vec<Vec<f64>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("batch::surface_points", class);
    batch_of(|_| Some((0..2 * POINTS).map(|_| unit(&mut g)).collect()))
}
