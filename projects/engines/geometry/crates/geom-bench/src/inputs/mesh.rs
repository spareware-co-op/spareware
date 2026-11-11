//! Inputs for the triangulation's hot path (piece G153, decision 32.44): a small terrain, a scatter
//! of survey points with breaklines across it.
//!
//! Class `typical` only: each input is a [`Terrain`] of [`POINTS`] distinct grid points and
//! [`BREAKLINE`] constraints spread across them. The triangulation's refusals are tested in
//! `geom-mesh` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the board's scatter with breaklines (chapter 19.2's target) at a size a count run can
//! afford, measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: one pass over the points per input, once per batch, outside every measured run.

use std::collections::BTreeSet;

use geom_mesh::Site;
use geom_num::grid::P2;

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// The points of one terrain.
pub const POINTS: u16 = 96;

/// The breaklines of one terrain.
pub const BREAKLINE: u16 = 8;

/// What `geom_mesh::triangulate` takes.
#[derive(Clone, Debug)]
pub struct Terrain {
    /// The survey points, keyed by their order.
    pub sites: Vec<Site<u32>>,
    /// The breaklines, between points spread across the scatter.
    pub constraints: Vec<[u32; 2]>,
}

/// One terrain from `rng`.
fn terrain(r: &mut SplitMix64) -> Option<Terrain> {
    let mut seen = BTreeSet::new();
    while seen.len() < usize::from(POINTS) {
        let (x, y) = (r.next_u64() >> 44, r.next_u64() >> 44);
        seen.insert((i64::try_from(x).ok()?, i64::try_from(y).ok()?));
    }
    let mut sites = Vec::with_capacity(usize::from(POINTS));
    for (&(x, y), key) in seen.iter().zip(0_u32..) {
        sites.push(Site {
            at: P2::new(x, y).ok()?,
            key,
        });
    }
    let step = u32::from(POINTS / BREAKLINE);
    let constraints = (0..u32::from(BREAKLINE))
        .map(|i| {
            let start = i.saturating_mul(step);
            [start, start.saturating_add(step).saturating_sub(1)]
        })
        .collect();
    Some(Terrain { sites, constraints })
}

/// Terrains for `geom_mesh::triangulate`.
#[must_use]
pub fn cdt(class: Class) -> Vec<Terrain> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("mesh::cdt", class);
    batch_of(|_| terrain(&mut g))
}
