//! Inputs for `geom-bvh`'s hot paths (pieces G54 and G55, decision 32.44): small scenes for the
//! box tree (build, then the boxes meeting a query box) and for the Morton-sorted cell index
//! (build, then the points within a radius of a centre).
//!
//! Class `typical` only: each input is a scene of [`SCENE`] entries with distinct keys, boxes or
//! points within ±2^20 grid steps of the origin (boxes up to 2^16 steps a side), and a query box or
//! centre of the same reach with a radius of 2^19 steps, so most queries find some entries and
//! none finds all. The structures' refusals (duplicate keys, a stopped meter) are tested in
//! `geom-bvh` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the queries of `geom-bvh`'s tests (`tests/tree.rs`, `tests/cells.rs`, each checked
//! there against brute force), built and queried whole under an unlimited budget on the serial
//! executor, at one scene size.
//! Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: about a hundred draws of [`SplitMix64`] per input, once per batch, outside every measured
//! run.

use geom_bvh::{Aabb, Entry, Point};
use geom_num::grid::P3;

use super::{Class, batch_of, p3, rng, within};
use crate::rng::SplitMix64;

/// Entries per scene.
pub const SCENE: usize = 32;
/// The reach of a scene's coordinates, in grid steps.
const REACH: i64 = 1 << 20;

/// A box within the scene's reach, up to 2^16 steps a side.
fn bbox(g: &mut SplitMix64) -> Option<Aabb<3>> {
    let lo = [within(g, REACH), within(g, REACH), within(g, REACH)];
    let size = [
        g.range(0, 1 << 16),
        g.range(0, 1 << 16),
        g.range(0, 1 << 16),
    ];
    Aabb::new(lo, [lo[0] + size[0], lo[1] + size[1], lo[2] + size[2]]).ok()
}

/// Scenes of boxes and a query box for `BoxTree::build` and `BoxTree::overlapping`.
#[must_use]
pub fn tree_build_query(class: Class) -> Vec<(Vec<Entry<3>>, Aabb<3>)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("bvh::tree_build_query", class);
    batch_of(|_| {
        let mut entries = Vec::with_capacity(SCENE);
        for key in 0..SCENE as u64 {
            entries.push(Entry {
                key,
                bbox: bbox(&mut g)?,
            });
        }
        let lo = [
            within(&mut g, REACH),
            within(&mut g, REACH),
            within(&mut g, REACH),
        ];
        let hi = [lo[0] + (1 << 19), lo[1] + (1 << 19), lo[2] + (1 << 19)];
        Some((entries, Aabb::new(lo, hi).ok()?))
    })
}

/// Scenes of points, a centre and a radius for `CellIndex::build` and `CellIndex::within`.
#[must_use]
pub fn cells_within(class: Class) -> Vec<(Vec<Point>, P3, u64)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("bvh::cells_within", class);
    batch_of(|_| {
        let points = (0..SCENE as u64)
            .map(|key| Point {
                key,
                at: p3(
                    within(&mut g, REACH),
                    within(&mut g, REACH),
                    within(&mut g, REACH),
                ),
            })
            .collect();
        let center = p3(
            within(&mut g, REACH),
            within(&mut g, REACH),
            within(&mut g, REACH),
        );
        Some((points, center, 1 << 19))
    })
}
