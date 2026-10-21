//! Inputs for exactness classes: [`combine`] (`Exactness::combine`), [`within`]
//! (`Exactness::within`) and [`combine_within`] (`Exactness::combine_within`).
//!
//! A bound is a count of grid steps together with the step it counts (`Q0`, `Q1` or `Q2`), and
//! combining or comparing two bounds first converts both to the finer of their steps, 25 per rung.
//! Each operation has one class, `typical`, whose batch takes every path in rotation:
//!
//! - `combine`: all nine pairings of exact, certified and approximate, with bounds of ordinary
//!   size on pseudo-random steps, so pairs on one step and on different steps both occur; in every
//!   27 inputs the last nine have bounds near `u64::MAX`, so the sum, or a bound converted to the
//!   finer step, does not fit and `combine` reports `None` (an exact operand leaves the other as it
//!   is, so those pairs still give a class).
//! - `within`: exact, certified and approximate classes against a ceiling on a pseudo-random step,
//!   with the bound, compared in the finer of the two steps, in turn one step below the ceiling,
//!   equal to it, one step above it, and near `u64::MAX` (`ToleranceExceeded`: by comparison, and
//!   in every other round, from `Q0` against a ceiling on `Q1`, because the conversion to the
//!   finer step does not fit).
//! - `combine_within`: the nine pairings of `combine`, against a ceiling one step below the
//!   combined bound, at it and one step above it, in the combined bound's step; and pairs with
//!   bounds near `u64::MAX`, whose combined bound does not fit or is far above the ceiling
//!   (`ToleranceExceeded` either way).
//!
//! Method: one input per arm of each match (chapter 9 of the kernel architecture: the less exact
//! class wins and bounds add, in the finer step; chapter 6.5: a bound over its ceiling is an
//! error), with boundary values at the ceiling and at the end of `u64` (boundary-value analysis,
//! <https://en.wikipedia.org/wiki/Boundary-value_analysis>).
//!
//! Guarantees: every batch is full, the same on every target, and takes the paths listed
//! (`tests/inputs.rs`), and every result agrees with chapters 9 and 6.5, computed exactly in
//! 128 bits (`tests/attack_g06.rs`).
//!
//! Cost: a few draws of [`crate::rng::SplitMix64`] per input, once per batch, outside every
//! measured run.

use geom_num::Exactness;
use geom_num::grid::{GridStep, Steps};

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// The three grid steps, coarsest first.
const STEPS: [GridStep; 3] = [GridStep::Q0, GridStep::Q1, GridStep::Q2];

/// A pseudo-random grid step.
fn step(g: &mut SplitMix64) -> Option<GridStep> {
    STEPS.get(g.index(STEPS.len())).copied()
}

/// A bound of ordinary size (below 2^32 steps), or near `u64::MAX` when `huge`.
fn bound(g: &mut SplitMix64, huge: bool) -> u64 {
    if huge {
        u64::MAX - (g.next_u64() >> 40)
    } else {
        g.next_u64() >> 32
    }
}

/// An exactness class of kind `kind` mod 3 (exact, certified, approximate), with this bound and
/// step when it has one.
fn class_of(kind: usize, bound: u64, step: GridStep) -> Exactness {
    match kind % 3 {
        0 => Exactness::Exact,
        1 => Exactness::Certified {
            bound: Steps(bound),
            step,
        },
        _ => Exactness::Approximate {
            deviation: Steps(bound),
            step,
        },
    }
}

/// `count` steps of the coarsest step, `Q0`, counted in steps of `step`: 25 per rung below `Q0`.
/// Every caller keeps `count` at most 2^30, so the result is below 2^40.
fn in_step(count: u64, step: GridStep) -> u64 {
    match step {
        GridStep::Q0 => count,
        GridStep::Q1 => count * 25,
        GridStep::Q2 => count * 625,
    }
}

/// Pairs for `Exactness::combine`.
#[must_use]
pub fn combine(class: Class) -> Vec<(Exactness, Exactness)> {
    let mut g = rng("exactness::combine", class);
    match class {
        Class::Typical => batch_of(|n| {
            let huge = n % 27 >= 18;
            let x = class_of(n, bound(&mut g, huge), step(&mut g)?);
            let y = class_of(n / 3, bound(&mut g, huge), step(&mut g)?);
            Some((x, y))
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// (class, ceiling, ceiling's step) triples for `Exactness::within`.
#[must_use]
pub fn within(class: Class) -> Vec<(Exactness, Steps, GridStep)> {
    let mut g = rng("exactness::within", class);
    match class {
        Class::Typical => batch_of(|n| {
            let case = (n / 3) % 4;
            let drawn = (step(&mut g)?, step(&mut g)?);
            // Every other round of the huge case puts the bound on Q0 and the ceiling on Q1, so
            // that converting the bound to the finer step does not fit.
            let (bound_step, ceiling_step) = if case == 3 && (n / 12).is_multiple_of(2) {
                (GridStep::Q0, GridStep::Q1)
            } else {
                drawn
            };
            // One length, a whole number of Q0 steps from 1 to 2^30, counted in each step: in the
            // finer of the two steps the ceiling and `at` are the same count.
            let length = g.below(1 << 30) + 1;
            let ceiling = in_step(length, ceiling_step);
            let at = in_step(length, bound_step);
            let bound = match case {
                // One step of the bound's own step below the length: below the ceiling in every
                // step (`at` is at least 1).
                0 => at - 1,
                1 => at,
                // One step above: above the ceiling in every step.
                2 => at + 1,
                // Above every ceiling, by comparison, or because its conversion to the finer step
                // does not fit.
                _ => bound(&mut g, true),
            };
            Some((class_of(n, bound, bound_step), Steps(ceiling), ceiling_step))
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// (class, class, ceiling, ceiling's step) quadruples for `Exactness::combine_within`.
#[must_use]
pub fn combine_within(class: Class) -> Vec<(Exactness, Exactness, Steps, GridStep)> {
    let mut g = rng("exactness::combine_within", class);
    match class {
        Class::Typical => batch_of(|n| {
            let (pairing, case) = (n % 9, (n / 9) % 4);
            let huge = case == 3;
            let x = class_of(pairing, bound(&mut g, huge), step(&mut g)?);
            let y = class_of(pairing / 3, bound(&mut g, huge), step(&mut g)?);
            // A ceiling one step below the combined bound, at it, or one step above it, in its
            // step; an ordinary ceiling on a pseudo-random step when there is no combined bound
            // (two exact classes) or the bounds are huge.
            let (ceiling, ceiling_step) = match x.combine(y).and_then(Exactness::bound) {
                Some((combined, combined_step)) if !huge => {
                    let at = combined.get();
                    let ceiling = match case {
                        0 => at.checked_sub(1)?,
                        1 => at,
                        _ => at.checked_add(1)?,
                    };
                    (Steps(ceiling), combined_step)
                }
                _ => (Steps(bound(&mut g, false)), step(&mut g)?),
            };
            Some((x, y, ceiling, ceiling_step))
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}
