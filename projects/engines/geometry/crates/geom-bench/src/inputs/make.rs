//! Inputs for `geom-make`'s hot paths (piece G101, decision 32.44): a box and a cylinder built from
//! their inputs, and a box moved and quarter-turned, each with its checker postcondition.
//!
//! Class `typical` only: each input is a primitive within ±2^20 grid steps of the origin, its sizes
//! from 1 to 2^16 steps, so every operation succeeds and the rows it builds are the same on every
//! input of a kind. The operations' refusals are tested in `geom-make` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the operations of `geom-make`'s attack tests (`tests/attack_g101.rs`), under an unlimited
//! budget, in the build a program ships. Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few draws of [`SplitMix64`] per input and, for the transforms, one box built, once per
//! batch, outside every measured run.

use geom_make::{BoxInput, Made, RoundInput, make_box};
use geom_num::grid::{GridStep, P3, Steps};
use geom_op::{OpContext, OpId, OpOutput, OpPath};
use geom_run::Budget;

use super::{Class, batch_of, p3, rng, within};
use crate::rng::SplitMix64;

/// The reach of an input's coordinates, in grid steps.
const REACH: i64 = 1 << 20;

/// A size in steps.
fn size(g: &mut SplitMix64) -> Steps {
    Steps(g.range(1, 1 << 16).unsigned_abs())
}

/// A point within the reach.
fn point(g: &mut SplitMix64) -> P3 {
    p3(within(g, REACH), within(g, REACH), within(g, REACH))
}

/// A box within the reach.
fn a_box(g: &mut SplitMix64) -> BoxInput {
    BoxInput {
        min: point(g),
        size: [size(g), size(g), size(g)],
    }
}

/// Boxes for `make_box`.
#[must_use]
pub fn boxes(class: Class) -> Vec<BoxInput> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("make::boxes", class);
    batch_of(|_| Some(a_box(&mut g)))
}

/// Cylinders for `cylinder`.
#[must_use]
pub fn cylinders(class: Class) -> Vec<RoundInput> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("make::cylinders", class);
    batch_of(|_| {
        Some(RoundInput {
            base: point(&mut g),
            radius: size(&mut g),
            height: size(&mut g),
            step: GridStep::Q0,
        })
    })
}

/// Built boxes, and an offset each, for `move_by`.
#[must_use]
pub fn moved_boxes(class: Class) -> Vec<(OpOutput<Made>, [i64; 3])> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("make::moved_boxes", class);
    let ctx = OpContext::new(OpPath::new(OpId::from_bytes([1; 16])), Budget::UNLIMITED);
    batch_of(|_| {
        let body = make_box(&ctx, &a_box(&mut g)).ok()?;
        let offset = [
            within(&mut g, REACH),
            within(&mut g, REACH),
            within(&mut g, REACH),
        ];
        Some((body, offset))
    })
}

/// Built boxes, and a centre each, for a quarter turn by `rotate`.
#[must_use]
pub fn turned_boxes(class: Class) -> Vec<(OpOutput<Made>, P3)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("make::turned_boxes", class);
    let ctx = OpContext::new(OpPath::new(OpId::from_bytes([1; 16])), Budget::UNLIMITED);
    batch_of(|_| {
        let body = make_box(&ctx, &a_box(&mut g)).ok()?;
        Some((body, point(&mut g)))
    })
}
