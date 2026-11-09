//! Inputs for `geom-2d`'s core COGO (piece G359, decision 32.44): points, bearings and distances
//! for forward, pairs of points for inverse, and traverses for their misclosure.
//!
//! - `forward`, class `typical`: a point within ±2^30 steps, a bearing anywhere in one turn and a
//!   distance below 2^29 steps (about 84 m at the building step), so the point stays in the frame.
//! - `inverse`, class `typical`: two points within ±2^30 steps.
//! - `misclosure`, class `typical`: traverses of eight legs from a point within ±2^30 steps, each
//!   leg a bearing anywhere in one turn and a distance below 2^26 steps, to an end point near the
//!   start, as a loop that nearly closes.
//! - `nearest_root`, class `typical`: squared distances below 2^84, as two points of a frame give,
//!   a quarter of them perfect squares.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a batch is
//! `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: one class per operation, as chapter 28.14 sets benchmarks out
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: a few draws of [`SplitMix64`] per value, once per batch, outside every measured run.

use geom_2d::cogo::Leg;
use geom_num::angle::{AngleUnits, Bearing, FULL_TURN};
use geom_num::grid::{P2, Steps};

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

/// A typical grid point, within ±2^30 steps.
fn point(g: &mut SplitMix64) -> Option<P2> {
    let reach = 2 * TYPICAL_REACH.unsigned_abs() + 1;
    P2::new(
        below(g, reach) - TYPICAL_REACH,
        below(g, reach) - TYPICAL_REACH,
    )
    .ok()
}

/// A bearing anywhere in one turn.
fn bearing(g: &mut SplitMix64) -> Bearing {
    Bearing::new(AngleUnits(below(g, FULL_TURN.unsigned_abs())))
}

/// A distance below `2^bits` steps.
fn distance(g: &mut SplitMix64, bits: u32) -> Steps {
    Steps(g.next_u64() >> (64 - bits))
}

/// Points, bearings and distances for `cogo::forward`.
#[must_use]
pub fn forward(class: Class) -> Vec<(P2, Bearing, Steps)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("cogo::forward", class);
    batch_of(|_| Some((point(&mut g)?, bearing(&mut g), distance(&mut g, 29))))
}

/// Pairs of points for `cogo::inverse`.
#[must_use]
pub fn inverse(class: Class) -> Vec<(P2, P2)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("cogo::inverse", class);
    batch_of(|_| Some((point(&mut g)?, point(&mut g)?)))
}

/// Traverses (start, legs, end) for `cogo::misclosure`.
#[must_use]
pub fn misclosure(class: Class) -> Vec<(P2, Vec<Leg>, P2)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("cogo::misclosure", class);
    batch_of(|_| {
        let start = point(&mut g)?;
        let legs = (0..8)
            .map(|_| Leg {
                bearing: bearing(&mut g),
                distance: distance(&mut g, 26),
            })
            .collect();
        let end = P2::new(
            start.x.get() + below(&mut g, 1_001) - 500,
            start.y.get() + below(&mut g, 1_001) - 500,
        )
        .ok()?;
        Some((start, legs, end))
    })
}

/// Squared distances, a quarter of them perfect squares, for `cogo::nearest_root`.
#[must_use]
pub fn nearest_root(class: Class) -> Vec<u128> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("cogo::nearest_root", class);
    batch_of(|at| {
        Some(if at % 4 == 0 {
            let k = u128::from(g.next_u64() >> 22);
            k * k
        } else {
            (u128::from(g.next_u64()) << 20) | u128::from(g.next_u64() >> 44)
        })
    })
}
