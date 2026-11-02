//! Inputs for `geom-spatial`'s certified GJK filter (piece G294, decision 32.44): two small clouds
//! of grid points and a reach.
//!
//! Class `typical` only: each input is two clouds of [`CLOUD`] points within ±2^16 grid steps of
//! their centres, the second centre between 2^19 and 2^20 steps from the first in any direction,
//! and a reach of 2^17 steps. Each cloud lies within √3 · 2^16 < 2^17 of its centre, so the hulls
//! are more than 2^18 apart: farther than the reach, which the filter can prove. Its undecided
//! answers (overlap, a reach past the gap) are tested in `geom-spatial` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Clear` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the filter's work at the size the narrow phase hands it, two hulls of a few dozen
//! points (19.11). Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: about 150 draws of [`SplitMix64`] per input, once per batch, outside every measured run.

use geom_num::grid::P3;

use super::{Class, batch_of, p3, rng, within};
use crate::rng::SplitMix64;

/// Points per cloud.
pub const CLOUD: usize = 24;
/// How far a cloud's points lie from its centre on each axis, in grid steps.
const SPREAD: i64 = 1 << 16;
/// The reach every input asks for, in grid steps.
pub const REACH: u64 = 1 << 17;

/// A cloud of [`CLOUD`] points round `centre`.
fn cloud(g: &mut SplitMix64, [x, y, z]: [i64; 3]) -> Vec<P3> {
    (0..CLOUD)
        .map(|_| {
            p3(
                x + within(g, SPREAD),
                y + within(g, SPREAD),
                z + within(g, SPREAD),
            )
        })
        .collect()
}

/// Two clouds and the reach, for `geom_spatial::gjk::filter`.
#[must_use]
pub fn gjk_filter(class: Class) -> Vec<(Vec<P3>, Vec<P3>, u64)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("spatial::gjk_filter", class);
    batch_of(|_| {
        let far = 1 << 20;
        let centre = [
            within(&mut g, far),
            within(&mut g, far),
            within(&mut g, far),
        ];
        // Between 2^19 and 2^20 from the origin, else another draw.
        let length = centre.iter().fold(0_i64, |s, c| s + c * c);
        if !(1_i64 << 38..=1 << 40).contains(&length) {
            return None;
        }
        Some((cloud(&mut g, [0, 0, 0]), cloud(&mut g, centre), REACH))
    })
}
