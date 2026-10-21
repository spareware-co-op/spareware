//! Inputs for grid coordinates: [`coord_new`] (`Coord::new`), [`coord_from_i128`]
//! (`Coord::from_i128`), [`p2_new`] (`P2::new`) and [`p3_new`] (`P3::new`); for moving
//! coordinates between grid steps: [`refine`] (coarse to fine, exact) and [`coarsen`] (fine to
//! coarse, rounded); and for counts of steps and units: [`steps_refine`] (`Steps::refine`) and
//! [`steps_per`] (`GridStep::steps_per`).
//!
//! Classes:
//!
//! - `typical`: coordinates inside the frame, ±(2^40 − 1) itself first, then building
//!   coordinates; for `refine`, `coarsen` and `steps_refine`, every pair of steps in the allowed
//!   direction, the same step included; for `refine`, coordinates small enough to fit the finer
//!   step, and for `steps_refine`, counts of ordinary size; for `steps_per`, every step against
//!   each of the units `geom_num::grid` names (millimetre, inch, micrometre, mil, nanometre) and a
//!   pseudo-random size, in rotation, so units that are a whole number of steps and units that are
//!   not (`None`) both occur.
//! - `extreme` (`coarsen`): coordinates within 2^10 steps of ±(2^40 − 1).
//! - `overflow`: a coordinate just or far outside the frame (each position in turn for points),
//!   so the result is `OutOfRange`; for `refine`, coordinates too large for the finer step; for
//!   `steps_refine`, counts too large for `u64` in the finer step (`None`).
//! - `wrong_direction` (`refine`, `coarsen`, `steps_refine`): every pair of steps the wrong way
//!   round, so the result is `WrongDirection`, or `None` from `steps_refine`.
//!
//! Method: the frame's range and its two errors as chapter 6.1 of the kernel architecture sets
//! them, tested at 2^40 − 1 and at 2^40 steps as chapter 28.2 requires ("The range holds"), with
//! refinement and coarsening between the three steps of the grid ladder (`geom_num::grid`): the
//! values on and just past each boundary (boundary-value analysis,
//! <https://en.wikipedia.org/wiki/Boundary-value_analysis>).
//!
//! Guarantees: every batch is full, the same on every target, and its classes are what they
//! claim (`tests/inputs.rs`); every result is checked against exact integer arithmetic by
//! `tests/attack_g06.rs`.
//!
//! Cost: a few draws of [`crate::rng::SplitMix64`] per input, once per batch, outside every
//! measured run.

use geom_num::grid::{
    Coord, GridStep, QUARTER_NM_PER_INCH, QUARTER_NM_PER_MICRON, QUARTER_NM_PER_MIL,
    QUARTER_NM_PER_MM, QUARTER_NM_PER_NM, Steps,
};

use super::{Class, M, TYPICAL_REACH, batch_of, coord, extreme, rng, within};
use crate::rng::SplitMix64;

/// Every (from, to) pair with `to` no coarser than `from`.
const REFINE: [(GridStep, GridStep); 6] = [
    (GridStep::Q0, GridStep::Q0),
    (GridStep::Q0, GridStep::Q1),
    (GridStep::Q0, GridStep::Q2),
    (GridStep::Q1, GridStep::Q1),
    (GridStep::Q1, GridStep::Q2),
    (GridStep::Q2, GridStep::Q2),
];

/// The pairs that refine by at least one rung.
const REFINING: [(GridStep, GridStep); 3] = [
    (GridStep::Q0, GridStep::Q1),
    (GridStep::Q0, GridStep::Q2),
    (GridStep::Q1, GridStep::Q2),
];

/// Every (from, to) pair with `to` no finer than `from`.
const COARSEN: [(GridStep, GridStep); 6] = [
    (GridStep::Q0, GridStep::Q0),
    (GridStep::Q1, GridStep::Q0),
    (GridStep::Q2, GridStep::Q0),
    (GridStep::Q1, GridStep::Q1),
    (GridStep::Q2, GridStep::Q1),
    (GridStep::Q2, GridStep::Q2),
];

/// The three grid steps, coarsest first.
const STEPS: [GridStep; 3] = [GridStep::Q0, GridStep::Q1, GridStep::Q2];

/// The units `geom_num::grid` names, in quarter-nanometres.
const UNITS: [u64; 5] = [
    QUARTER_NM_PER_MM,
    QUARTER_NM_PER_INCH,
    QUARTER_NM_PER_MICRON,
    QUARTER_NM_PER_MIL,
    QUARTER_NM_PER_NM,
];

/// 25^k: how many finer steps one coarser step holds across k rungs.
const fn factor(rungs: u32) -> i64 {
    match rungs {
        0 => 1,
        1 => 25,
        _ => 625,
    }
}

/// The rungs between two steps, in either direction.
const fn rungs(from: GridStep, to: GridStep) -> u32 {
    from.rung().abs_diff(to.rung())
}

/// A step count inside the frame: ±(2^40 − 1) for the first two inputs, then a building
/// coordinate.
fn inside(g: &mut SplitMix64, n: usize) -> i64 {
    match n {
        0 => M,
        1 => -M,
        _ => within(g, TYPICAL_REACH),
    }
}

/// A step count outside the frame: just outside (±2^40) every other time, far outside otherwise.
fn outside(g: &mut SplitMix64, n: usize) -> i64 {
    let magnitude = if n.is_multiple_of(2) {
        M + 1
    } else {
        g.range(M + 1, i64::MAX)
    };
    g.sign() * magnitude
}

/// Step counts for `Coord::new`.
#[must_use]
pub fn coord_new(class: Class) -> Vec<i64> {
    let mut g = rng("grid::coord_new", class);
    match class {
        Class::Typical => batch_of(|n| Some(inside(&mut g, n))),
        Class::Overflow => batch_of(|n| Some(outside(&mut g, n))),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Step counts for `Coord::from_i128`: inside the frame, or outside it up to the full `i128`
/// range.
#[must_use]
pub fn coord_from_i128(class: Class) -> Vec<i128> {
    let mut g = rng("grid::coord_from_i128", class);
    match class {
        Class::Typical => batch_of(|n| Some(i128::from(inside(&mut g, n)))),
        Class::Overflow => batch_of(|n| {
            Some(match n % 4 {
                0 => i128::from(M) + 1,
                1 => -i128::from(M) - 1,
                // Up to 2^127 − 1: far outside, within the type.
                2 => (i128::from(g.next_u64()) << 63) | i128::from(g.next_u64() >> 1),
                _ => i128::MIN,
            })
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Step-count pairs for `P2::new`; in the overflow class each position is out of range in turn.
#[must_use]
pub fn p2_new(class: Class) -> Vec<(i64, i64)> {
    let mut g = rng("grid::p2_new", class);
    match class {
        Class::Typical => batch_of(|n| Some((inside(&mut g, n), inside(&mut g, n + 1)))),
        Class::Overflow => batch_of(|n| {
            let mut v = [inside(&mut g, 2), inside(&mut g, 2)];
            if let Some(bad) = v.get_mut(n % 2) {
                *bad = outside(&mut g, n);
            }
            let [x, y] = v;
            Some((x, y))
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Step-count triples for `P3::new`; in the overflow class each position is out of range in turn.
#[must_use]
pub fn p3_new(class: Class) -> Vec<(i64, i64, i64)> {
    let mut g = rng("grid::p3_new", class);
    match class {
        Class::Typical => batch_of(|n| {
            Some((
                inside(&mut g, n),
                inside(&mut g, n + 1),
                inside(&mut g, n + 2),
            ))
        }),
        Class::Overflow => batch_of(|n| {
            let mut v = [inside(&mut g, 2), inside(&mut g, 2), inside(&mut g, 2)];
            if let Some(bad) = v.get_mut(n % 3) {
                *bad = outside(&mut g, n);
            }
            let [x, y, z] = v;
            Some((x, y, z))
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// (coordinate, from, to) triples for [`geom_num::grid::refine`].
#[must_use]
pub fn refine(class: Class) -> Vec<(Coord, GridStep, GridStep)> {
    let mut g = rng("grid::refine", class);
    match class {
        Class::Typical => batch_of(|n| {
            let (from, to) = REFINE.get(n % REFINE.len()).copied()?;
            // Small enough that the refined value fits: |c| · 25^k ≤ M.
            let reach = TYPICAL_REACH.min(M / factor(rungs(from, to)));
            Some((coord(within(&mut g, reach)), from, to))
        }),
        Class::Overflow => batch_of(|n| {
            // Only pairs that actually refine; |c| above M / 25^k, so the result leaves the frame.
            let (from, to) = REFINING.get(n % REFINING.len()).copied()?;
            let least = M / factor(rungs(from, to)) + 1;
            Some((coord(g.sign() * g.range(least, M)), from, to))
        }),
        Class::WrongDirection => batch_of(|n| {
            // Refining to a coarser step: each refining pair reversed.
            let (to, from) = REFINING.get(n % REFINING.len()).copied()?;
            Some((coord(within(&mut g, TYPICAL_REACH)), from, to))
        }),
        Class::Degenerate | Class::NearlyDegenerate | Class::Extreme | Class::OnGrid => Vec::new(),
    }
}

/// (coordinate, from, to) triples for [`geom_num::grid::coarsen`].
#[must_use]
pub fn coarsen(class: Class) -> Vec<(Coord, GridStep, GridStep)> {
    let mut g = rng("grid::coarsen", class);
    match class {
        Class::Typical => batch_of(|n| {
            let (from, to) = COARSEN.get(n % COARSEN.len()).copied()?;
            // A building at the finer step: up to 2^30 coarse steps, times 25 per rung.
            let reach = TYPICAL_REACH.saturating_mul(factor(rungs(from, to))).min(M);
            Some((coord(within(&mut g, reach)), from, to))
        }),
        Class::Extreme => batch_of(|n| {
            let (from, to) = COARSEN.get(n % COARSEN.len()).copied()?;
            // ±M exactly first, then coordinates within the extreme slack of ±M.
            let c = match n {
                0 => M,
                1 => -M,
                _ => extreme(&mut g),
            };
            Some((coord(c), from, to))
        }),
        Class::WrongDirection => batch_of(|n| {
            // Coarsening to a finer step: each refining pair, which coarsen refuses.
            let (from, to) = REFINING.get(n % REFINING.len()).copied()?;
            Some((coord(within(&mut g, TYPICAL_REACH)), from, to))
        }),
        Class::Degenerate | Class::NearlyDegenerate | Class::Overflow | Class::OnGrid => Vec::new(),
    }
}

/// (count, from, to) triples for `Steps::refine`.
#[must_use]
pub fn steps_refine(class: Class) -> Vec<(Steps, GridStep, GridStep)> {
    let mut g = rng("grid::steps_refine", class);
    match class {
        Class::Typical => batch_of(|n| {
            // Counts below 2^40, which fit `u64` on every finer step (625 · 2^40 < 2^50).
            let (from, to) = REFINE.get(n % REFINE.len()).copied()?;
            Some((Steps(g.next_u64() >> 24), from, to))
        }),
        Class::Overflow => batch_of(|n| {
            // Only pairs that actually refine; above u64::MAX / 25^k, so the finer count does not
            // fit.
            let (from, to) = REFINING.get(n % REFINING.len()).copied()?;
            let least = u64::MAX / factor(rungs(from, to)).unsigned_abs() + 1;
            Some((Steps(least + g.below(u64::MAX - least)), from, to))
        }),
        Class::WrongDirection => batch_of(|n| {
            // To a coarser step, where the count would round: each refining pair reversed.
            let (to, from) = REFINING.get(n % REFINING.len()).copied()?;
            Some((Steps(g.next_u64() >> 24), from, to))
        }),
        Class::Degenerate | Class::NearlyDegenerate | Class::Extreme | Class::OnGrid => Vec::new(),
    }
}

/// (step, unit in quarter-nanometres) pairs for `GridStep::steps_per`: each step against the
/// named units and a pseudo-random size below 2^40 quarter-nanometres, in rotation.
#[must_use]
pub fn steps_per(class: Class) -> Vec<(GridStep, u64)> {
    let mut g = rng("grid::steps_per", class);
    match class {
        Class::Typical => batch_of(|n| {
            let step = STEPS.get(n % STEPS.len()).copied()?;
            let unit = match UNITS.get((n / STEPS.len()) % (UNITS.len() + 1)) {
                Some(&unit) => unit,
                None => g.next_u64() >> 24,
            };
            Some((step, unit))
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}
