//! Inputs for the core queries (piece G102, decision 32.46): each of the six queries on a built
//! body, half of them boxes and half prisms on a right triangle, and a face of each for the area.
//!
//! Class `typical` only: each body is a primitive within ±2^20 grid steps of the origin, its sizes
//! from 1 to 2^16 steps, so every query succeeds, every face is a polygon on a plane and each kind
//! of body has the same rows on every input. The queries' refusals (a curved face or edge, a face
//! with no loop, a body that is not a solid, a list that does not end) are tested in `geom-topo`,
//! `geom-check` and `engine-geometry` themselves.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the query's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the bodies of `engine-geometry`'s contract and query laws (`tests/contract_laws.rs`,
//! `tests/query_laws.rs`), under an unlimited budget, in the build a program ships. Each is
//! measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few draws of [`SplitMix64`] and one body built per input, once per batch, outside
//! every measured run.

use geom_make::{BoxInput, Made, PrismInput, make_box, prism};
use geom_num::grid::{P3, Steps};
use geom_op::{OpContext, OpId, OpOutput, OpPath};
use geom_run::Budget;
use geom_topo::Arena;

use super::{Class, batch_of, p3, rng, within};
use crate::rng::SplitMix64;

/// The reach of a body's corner, in grid steps.
const REACH: i64 = 1 << 20;

/// A size in steps.
fn size(g: &mut SplitMix64) -> u64 {
    g.range(1, 1 << 16).unsigned_abs()
}

/// The `n`-th body: a box for even `n`, a prism on a right triangle for odd.
fn body(g: &mut SplitMix64, n: usize) -> Option<OpOutput<Made>> {
    let ctx = OpContext::new(OpPath::new(OpId::from_bytes([1; 16])), Budget::UNLIMITED);
    let (x, y, z) = (within(g, REACH), within(g, REACH), within(g, REACH));
    let (a, b, h) = (size(g), size(g), size(g));
    if n.is_multiple_of(2) {
        let input = BoxInput {
            min: p3(x, y, z),
            size: [Steps(a), Steps(b), Steps(h)],
        };
        return make_box(&ctx, &input).ok();
    }
    let corner = |dx: u64, dy: u64| -> Option<P3> {
        let dx = i64::try_from(dx).ok()?;
        let dy = i64::try_from(dy).ok()?;
        Some(p3(x.checked_add(dx)?, y.checked_add(dy)?, z))
    };
    let input = PrismInput {
        base: vec![corner(0, 0)?, corner(a, 0)?, corner(0, b)?],
        height: Steps(h),
    };
    prism(&ctx, &input).ok()
}

/// Built bodies from the stream `label`: each benchmark draws its own, so no two share inputs.
fn built(label: &str, class: Class) -> Vec<OpOutput<Made>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(label, class);
    batch_of(|n| body(&mut g, n))
}

/// Built bodies, for `query.volume`.
#[must_use]
pub fn volumes(class: Class) -> Vec<OpOutput<Made>> {
    built("query::volumes", class)
}

/// Built bodies, for `query.bounding_box`.
#[must_use]
pub fn boxes(class: Class) -> Vec<OpOutput<Made>> {
    built("query::boxes", class)
}

/// Built bodies, for `query.exactness`.
#[must_use]
pub fn reports(class: Class) -> Vec<OpOutput<Made>> {
    built("query::reports", class)
}

/// Built bodies, for `query.faces`.
#[must_use]
pub fn listed(class: Class) -> Vec<OpOutput<Made>> {
    built("query::listed", class)
}

/// Built bodies, for `query.validity`.
#[must_use]
pub fn validities(class: Class) -> Vec<OpOutput<Made>> {
    built("query::validities", class)
}

/// Built bodies, and a face of each, for `query.area`.
#[must_use]
pub fn faces(class: Class) -> Vec<(OpOutput<Made>, u32)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("query::faces", class);
    batch_of(|n| {
        let out = body(&mut g, n)?;
        let rows = u64::from(out.result.body.arenas().rows(Arena::Face));
        let face = u32::try_from(g.below(rows)).ok()?;
        Some((out, face))
    })
}
