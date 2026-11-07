//! The operation-log minimiser: `geom-test` removes operations and inputs one at a time while the
//! failure persists (chapter 28.16, step 1; piece G97).
//!
//! What it does: [`minimise`] takes a log and a predicate that says whether a log still fails, and
//! tries, in a fixed order, removing each operation, each input of each operation and each stored
//! input, keeping every removal after which the predicate still holds. It repeats until a whole
//! pass removes nothing. [`same_failure`] is the usual predicate: the log, run as the fixture's,
//! fails in the same class of way.
//!
//! Guarantees:
//!
//! - **The result still fails**: the predicate held for it, on the last removal kept or on the
//!   first call, which refuses a log that does not fail.
//! - **1-minimal**: removing any one operation, input or stored input from the result makes the
//!   predicate false. The method is delta debugging's end state, reached by single removals, which
//!   is what the chapter asks for; a log is short, so nothing is gained from removing chunks.
//! - **Deterministic**: the order of trials is fixed, so one failure minimises to one log.
//! - **Removal only**: nothing is added or reordered, and no parameter is changed.
//!
//! Method: Zeller and Hildebrandt, "Simplifying and Isolating Failure-Inducing Input", IEEE TSE
//! 28(2), 2002, for 1-minimality; chapter 28.16 for the units removed
//! (`projects/engines/geometry/architecture/28-testing.md`). Written here: the units are this
//! crate's, and the loop is shorter than any adapter to a delta-debugging crate.
//!
//! Cost: each pass tries every unit once, so `O(n^2)` predicate calls for `n` units in the worst
//! case and `n + 1` when nothing can go.

use super::run::{Engine, Failure, Verdict, run};
use super::{Fixture, OpLog};

/// Why a log cannot be minimised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinimiseError {
    /// The predicate does not hold of the log given: there is no failure to keep.
    NotFailing,
}

impl core::fmt::Display for MinimiseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("the log does not fail, so there is nothing to minimise")
    }
}

impl core::error::Error for MinimiseError {}

/// One thing that can be removed from a log.
#[derive(Clone, Copy, Debug)]
enum Unit {
    Operation(usize),
    Input(usize, usize),
    Stored(usize),
}

/// `log` without `unit`, or `None` if `unit` is past the end.
fn without(log: &OpLog, unit: Unit) -> Option<OpLog> {
    let mut less = log.clone();
    match unit {
        Unit::Operation(at) => {
            if at >= less.operations.len() {
                return None;
            }
            less.operations.remove(at);
        }
        Unit::Input(at, input) => {
            let inputs = &mut less.operations.get_mut(at)?.inputs;
            if input >= inputs.len() {
                return None;
            }
            inputs.remove(input);
        }
        Unit::Stored(at) => {
            if at >= less.inputs.len() {
                return None;
            }
            less.inputs.remove(at);
        }
    }
    Some(less)
}

/// Keeps every removal of `make(0)`, `make(1)`, ... after which `fails` holds, walking from the
/// first; a kept removal shifts the rest down, so its index is tried again.
fn sweep(
    log: &mut OpLog,
    fails: &mut impl FnMut(&OpLog) -> bool,
    make: impl Fn(usize) -> Unit,
) -> bool {
    let mut changed = false;
    let mut at = 0;
    while let Some(less) = without(log, make(at)) {
        if fails(&less) {
            *log = less;
            changed = true;
        } else {
            at = at.saturating_add(1);
        }
    }
    changed
}

/// Removes operations and inputs from `log` one at a time while `fails` holds.
///
/// # Errors
///
/// [`MinimiseError::NotFailing`] if `fails` does not hold of `log`.
pub fn minimise(
    log: &OpLog,
    mut fails: impl FnMut(&OpLog) -> bool,
) -> Result<OpLog, MinimiseError> {
    if !fails(log) {
        return Err(MinimiseError::NotFailing);
    }
    let mut log = log.clone();
    loop {
        let mut changed = sweep(&mut log, &mut fails, Unit::Operation);
        for operation in 0..log.operations.len() {
            changed |= sweep(&mut log, &mut fails, |input| Unit::Input(operation, input));
        }
        changed |= sweep(&mut log, &mut fails, Unit::Stored);
        if !changed {
            return Ok(log);
        }
    }
}

/// Whether `log`, in place of `fixture`'s, still fails in `failure`'s class of way under `edition`.
#[must_use]
pub fn same_failure(
    fixture: &Fixture,
    edition: &str,
    engine: &mut dyn Engine,
    failure: &Failure,
    log: &OpLog,
) -> bool {
    let mut trial = fixture.clone();
    trial.log = log.clone();
    matches!(run(&trial, edition, engine), Verdict::Fail(found) if found.class() == failure.class())
}
