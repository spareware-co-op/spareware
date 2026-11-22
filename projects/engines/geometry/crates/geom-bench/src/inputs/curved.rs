//! Inputs for the curved builder's broad phase (piece G242, decision 32.46): a scene of points
//! padded by the contact radius, as boxes with distinct keys, for the pairs whose boxes meet.
//!
//! Class `typical` only: each input is a scene of [`SCENE`] points of a cube 512 steps on a side, so
//! each box (a point padded by 64 steps) meets several others and none meets them all. The
//! refusals of the broad phase (a box past the tree's range) are tested in `geom-bool` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the broad phase of `geom-bool`'s attack tests (`tests/near_contact_booleans.rs`, checked
//! there against brute force), built and queried whole under an unlimited budget on the serial
//! executor. Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: about a hundred draws of [`SplitMix64`] per input, once per batch, outside every measured
//! run.

use geom_bool::broad::point_entries;
use geom_bool::carrier::PointGeom;
use geom_bool::distance::Carriers;
use geom_bvh::Entry;

use super::{Class, batch_of, p3, rng, within};

/// Points per scene.
pub const SCENE: usize = 32;
/// Half the side of the cube a scene lies in, in grid steps.
const REACH: i64 = 256;

/// Scenes of padded point boxes for `candidate_pairs`.
#[must_use]
pub fn broad_scenes(class: Class) -> Vec<Vec<Entry<3>>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("curved::broad_scenes", class);
    let carriers = Carriers::new();
    batch_of(|_| {
        let points: Vec<PointGeom> = (0..SCENE)
            .map(|_| {
                PointGeom::Grid(p3(
                    within(&mut g, REACH),
                    within(&mut g, REACH),
                    within(&mut g, REACH),
                ))
            })
            .collect();
        point_entries(&carriers, &points).ok()
    })
}
