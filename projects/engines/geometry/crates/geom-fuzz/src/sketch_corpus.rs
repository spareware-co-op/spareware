//! The fuzz target of the generated sketch families (piece G231): any bytes are read as a seed for a
//! night of cases, each with the diagnosis it is built to have, and the kernel must give it.
//!
//! What it does: [`harness`] reads the first eight bytes as a seed and the next as a count of cases
//! (one to twelve), draws them with `geom_test::oracle::families::cases`, and holds the kernel's
//! diagnosis of each to what its family is built to have. The bytes after those pick one case and
//! scale one of its lengths, which makes a sketch no family promised anything of; the block solver
//! must then either refuse it or meet every rule.
//!
//! Guarantees:
//!
//! - **Never a panic.** Every sketch is diagnosed and solved or refused with a typed error.
//! - **A family is what it is built to be.** Degrees of freedom, redundant groups, conflicting
//!   groups and whether the rules can all hold are the family's, for every seed.
//! - **A solve is a solve.** When the block solver answers, every rule holds within a grid step.
//! - **Deterministic.** The same bytes give the same cases and the same replay text.
//!
//! Method: the property-based fuzzing of 28.5
//! (`projects/engines/geometry/architecture/28-testing.md`), on the families of
//! `geom_test::oracle::families` (15.8 and 15.16 in
//! `projects/engines/geometry/architecture/15-sketches.md`).
//!
//! Cost: a few diagnoses and solves of sketches of up to a dozen points per input.

use geom_test::oracle::families::{Case, cases, check, holds, replay_text};
use geom_test::oracle::sketches::Rule;
use geom_test::seed::Seed;

use crate::Finding;

/// The most cases an input draws.
const MOST: u64 = 12;

/// The laws on `case`, as built.
///
/// # Errors
///
/// A [`Finding`] naming the law `case` broke.
pub fn check_case(case: &Case) -> Result<(), Finding> {
    let broke = |what: &str, why: String| Finding::new(what, move || why.clone());
    if let Err(why) = check(case) {
        return Err(broke("a family has the diagnosis it is built to have", why));
    }
    if case.expect.consistent {
        match holds(case) {
            Ok(true) => {}
            Ok(false) => {
                return Err(broke(
                    "a consistent family's solve meets every rule",
                    format!("case {} ({})", case.job.id, case.family.name()),
                ));
            }
            Err(why) => return Err(broke("a consistent family is solved", why)),
        }
    }
    Ok(())
}

/// The laws on `case` with its `which`th length scaled by `by`, a sketch nothing was promised of.
///
/// # Errors
///
/// A [`Finding`] naming the law the edited case broke.
pub fn check_edited(case: &Case, which: usize, by: f64) -> Result<(), Finding> {
    let mut edited = case.clone();
    let lengths: Vec<usize> = edited
        .job
        .rules
        .iter()
        .enumerate()
        .filter(|(_, r)| matches!(r, Rule::Distance { .. }))
        .map(|(i, _)| i)
        .collect();
    let Some(&at) = lengths.get(which.checked_rem(lengths.len().max(1)).unwrap_or(0)) else {
        return Ok(());
    };
    if let Some(Rule::Distance { value, .. }) = edited.job.rules.get_mut(at) {
        let old = geom_test::oracle::exact::read_float(value).unwrap_or(1.0);
        if let Some(text) = geom_test::oracle::exact::float(old * by) {
            *value = text;
        }
    }
    // Solved, every rule holds; refused, nothing is promised. `holds` is an error for a refusal.
    match holds(&edited) {
        Ok(false) => Err(Finding::new(
            "a solve of an edited sketch meets every rule",
            || format!("case {} edited by {by}", edited.job.id),
        )),
        _ => Ok(()),
    }
}

/// The laws on the cases the bytes `data` describe.
///
/// # Errors
///
/// As [`check_case`] and [`check_edited`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    let mut seed = [0_u8; 8];
    for (slot, byte) in seed.iter_mut().zip(data) {
        *slot = *byte;
    }
    let count = 1 + u64::from(data.get(8).copied().unwrap_or(0)) % MOST;
    let night = cases(Seed(u64::from_le_bytes(seed)), count);
    for case in &night {
        check_case(case)?;
    }
    match (replay_text(&night), replay_text(&night)) {
        (Ok(a), Ok(b)) if a == b => {}
        _ => {
            return Err(Finding::new("the replay text is the same twice", || {
                format!("{data:?}")
            }));
        }
    }
    if let (Some(&pick), Some(&scale)) = (data.get(9), data.get(10)) {
        let by = 0.25 + f64::from(scale) / 64.0;
        if let Some(case) = night.get(usize::from(pick) % night.len().max(1)) {
            check_edited(case, usize::from(data.get(11).copied().unwrap_or(0)), by)?;
        }
    }
    Ok(())
}
