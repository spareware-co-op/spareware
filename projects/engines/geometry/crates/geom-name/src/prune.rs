//! The `PruneHistory` operation (24.4): an element's history cut down to what the references the
//! program holds can reach.
//!
//! What it does: names cost space, and the references that keep history alive live in programs,
//! out of a save's sight, so history is pruned only on purpose and saving never prunes. [`prune`]
//! takes an element's recorded history (its [`Step`]s, oldest first) and the set of names the
//! program's references hold, in the context of the `PruneHistory` operation, whose path the log
//! records with that set so every replica prunes alike. It returns the history with each step recorded
//! since the last prune made a [`Step::Kept`]: it keeps its place and its path, and its history
//! keeps only the records and deletions a walk from a held name can meet. A walk from any other
//! name that meets such a step answers `HistoryPruned` with the operation's path. Each kept step
//! names the operation and its set, so a program hands a reference the steps since it was made
//! as it did before.
//!
//! Guarantees:
//!
//! - **Every held reference resolves the same.** A walk follows a step's modified images of the
//!   names it holds and asks whether they were deleted, nothing else (see [`resolve`]). The set a
//!   walk from a held name can hold entering a step, made at any step of its history, lies in the
//!   set kept there: every held name, and the images of the set kept at the step before. Each kept
//!   step holds every record with an image of that set and every deletion in it, so the walk meets
//!   what the full history gives it and [`resolve`] answers as before. `tests/prune.rs` checks it
//!   on generated chains from every step and for every want.
//! - **Pruned is never Deleted.** A walk from a name the set does not hold answers `HistoryPruned`
//!   with the operation's path as soon as it meets a kept step, whatever the step keeps (24.4).
//! - **Inside its budget.** It charges [`CHARGES`], and a run stopped by its budget or by
//!   cancellation publishes nothing: the history stays as it was.
//! - **Identical on every replica.** The result is a function of the history, the set and the
//!   path alone: every set is in name order and searched by bisection, and nothing depends on an
//!   address, a hash map's order or the machine.
//! - **Pruning again prunes only what is new.** A step an earlier prune kept, and a step pruned in
//!   a linked version shared without its history, stay as they are, so a reference resolves
//!   through them as it did; only the steps recorded since are cut.
//!
//! Decisions here, where 24.4 leaves the detail open:
//!
//! - A record is kept whole when a name it modifies from is in the kept set (its other inputs are
//!   never followed, since a walk follows only the names it holds); a generated record never is,
//!   since a walk never follows generated images; topology changes (13.14) never are, since no
//!   walk reads them.
//! - A step left with nothing keeps its place, as an empty history: a walk that meets it with names
//!   the full step neither images nor deletes ends there, as it did.
//! - The name graph is not pruned: resolving any reference first classifies its name in the graph,
//!   and a name taken out of it would answer `NameUnknown`, not `HistoryPruned`.
//! - An empty set prunes every step: no reference is held, and any that turns up answers
//!   `HistoryPruned`.
//! - One charge a step and a record read, on [`geom_op::RUN`]: a step costs its records, however
//!   few are kept, since each is read.
//!
//! Method: chapter 24.4's `PruneHistory` (`projects/engines/geometry/architecture/24-file-format-exchange.md`)
//! with chapter 20.4's walk; the kept set is the forward closure of the held names under each
//! step's modified images, the marking `FreeCAD`'s element map does for the names live entities and
//! references reach (the evidence note `kit/notes/G01-evidence/07-persistent-naming.md`).
//!
//! Cost: for s steps, r records per step and k names kept at a step, O(s · r log k), searched by
//! bisection, never with hashed sets. Recursion: none.

use alloc::vec::Vec;

use geom_num::Exactness;
use geom_op::history::{History, Origin, Record};
use geom_op::name::NameSet;
use geom_op::{Charge, NameId, OpContext, OpError, OpOutput, RUN};

use crate::resolve::Step;

/// `PruneHistory`'s charge at version 1: one unit a step and a record read.
const CHARGE: Charge = Charge::new(RUN, "step or record", 1);

/// `PruneHistory`'s charge schedule at version 1 (23.10).
pub const CHARGES: &[Charge] = &[CHARGE];

/// The history `history` pruned to what references to `held` can reach, by the `PruneHistory`
/// operation at `ctx`'s path (see the module documentation).
///
/// # Errors
///
/// `BudgetExceeded` or `Cancelled` if the run stops; nothing is published then.
pub fn prune(
    ctx: &OpContext<'_>,
    history: &[Step],
    held: &NameSet,
) -> Result<OpOutput<Vec<Step>>, OpError> {
    ctx.run(|meter| {
        let mut out: Vec<Step> = Vec::new();
        // The names a walk can hold leaving the step before, in name order.
        let mut reach: Vec<NameId> = Vec::new();
        for step in history {
            let records = match step {
                Step::Recorded { history, .. } | Step::Kept { history, .. } => {
                    history.records().len()
                }
                Step::Pruned { .. } => 0,
            };
            CHARGE
                .charge(
                    meter,
                    u64::try_from(records).map_or(u64::MAX, |r| r.saturating_add(1)),
                )
                .map_err(|stopped| ctx.stopped(meter, stopped, RUN))?;
            out.push(match step {
                Step::Recorded { by: at, history } => {
                    let entering = union(&reach, held.names());
                    let kept = cut(history, &entering);
                    reach = images(&kept, &entering);
                    Step::Kept {
                        by: at.clone(),
                        history: kept,
                        pruned_by: ctx.path().clone(),
                        held: held.clone(),
                    }
                }
                Step::Kept {
                    history: kept,
                    held: kept_for,
                    ..
                } => {
                    let entering = union(&reach, kept_for.names());
                    reach = images(kept, &entering);
                    step.clone()
                }
                Step::Pruned { .. } => {
                    reach.clear();
                    step.clone()
                }
            });
        }
        Ok(OpOutput::new(out, Exactness::Exact))
    })
}

/// `history` with only the records that have a modified image of a name in `entering`, and only
/// the deletions in it.
fn cut(history: &History, entering: &[NameId]) -> History {
    let records: Vec<Record> = history
        .records()
        .iter()
        .filter(|r| from_any(&r.origin, r.output, entering))
        .cloned()
        .collect();
    let deleted: Vec<NameId> = history
        .deleted()
        .names()
        .iter()
        .copied()
        .filter(|&d| holds(entering, d))
        .collect();
    // The deletions are a part of a set, distinct and in name order, and the records a part of a
    // history's, each output once and kept whole, so neither constructor refuses anything.
    let deleted = NameSet::new(deleted).unwrap_or_default();
    History::new(records, deleted, []).unwrap_or_default()
}

/// Whether the record of `output` has a modified image of a name in `entering`, as
/// `History::relations` counts them.
fn from_any(origin: &Origin, output: NameId, entering: &[NameId]) -> bool {
    match origin {
        Origin::Unchanged => holds(entering, output),
        Origin::Modified { from } | Origin::Merged { of: from } => {
            from.names().iter().any(|&n| holds(entering, n))
        }
        Origin::Split { of, .. } => holds(entering, *of),
        Origin::Generated { .. } => false,
    }
}

/// The modified images in `history` of the names in `entering`, in name order, each once.
fn images(history: &History, entering: &[NameId]) -> Vec<NameId> {
    let mut out: Vec<NameId> = history
        .relations()
        .modified()
        .filter(|&(from, _)| holds(entering, from))
        .map(|(_, to)| to)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The names in `a` or `b`, both in name order, in name order and each once.
fn union(a: &[NameId], b: &[NameId]) -> Vec<NameId> {
    let mut out: Vec<NameId> = a.iter().chain(b).copied().collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Whether the sorted `names` hold `name`.
fn holds(names: &[NameId], name: NameId) -> bool {
    names.binary_search(&name).is_ok()
}
