// SPDX-License-Identifier: Apache-2.0
//! Inputs for `geom-sketch`'s diagnosis and free motions (piece G224, decision 32.44): small sketches
//! in the `sketch v1` text form, each held at its start by a fixed first point and dimensioned along
//! a chain.
//!
//! Class `typical` only: a chain of 6 to 12 points, each a distance across and up from the last
//! (100 to 355 steps), exactly determined for the diagnosis and with every y free for the motions,
//! so each diagnosis and each factorisation succeeds. The text is the input, so the count includes
//! reading it. The operations' refusals are tested in `geom-sketch` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the chains of `geom-sketch`'s `benches/diagnose.rs`, at one small size. Each is measured
//! as chapter 28.14 sets benchmarks out, `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few draws of [`SplitMix64`] and a short text per input, once per batch, outside every
//! measured run.

use core::fmt::Write as _;

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// A chain of `points` points, each `across` right of the last and, with `upward`, `up` above it.
fn chain(g: &mut SplitMix64, upward: bool) -> String {
    let points = usize::try_from(g.range(6, 12)).unwrap_or(6);
    let mut text = String::from("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n");
    let (mut x, mut y) = (0_i64, 0_i64);
    let mut links = Vec::new();
    for i in 0..points {
        let _ = writeln!(text, "entity 1.{i} point {x} {y}");
        let (across, up) = (g.range(100, 355), g.range(100, 355));
        links.push((across, up));
        x += across;
        y += up;
    }
    text.push_str("constraint 2.0 driving 0 fix 1.0 0 0\nconstraint 2.1 driving 0 fix 1.0 1 0\n");
    for (i, &(across, up)) in links.iter().enumerate().take(points - 1) {
        let _ = writeln!(
            text,
            "constraint 3.{} driving 0 across 1.{i} 1.{} {across}",
            i + 1,
            i + 1
        );
        if upward {
            let _ = writeln!(
                text,
                "constraint 4.{} driving 0 upward 1.{i} 1.{} {up}",
                i + 1,
                i + 1
            );
        }
    }
    text
}

/// Sketches for `diagnose`: exactly determined chains.
#[must_use]
pub fn determined_chains(class: Class) -> Vec<String> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("sketch::determined_chains", class);
    batch_of(|_| Some(chain(&mut g, true)))
}

/// Sketches for the sparse free motions: chains held along x only, every y free.
#[must_use]
pub fn free_chains(class: Class) -> Vec<String> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("sketch::free_chains", class);
    batch_of(|_| Some(chain(&mut g, false)))
}
