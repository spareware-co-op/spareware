//! What a pull request may do to the fixtures already stored (chapters 28.13 and 28.16; piece G97).
//!
//! What it does: [`diff`] compares the fixtures of a base commit with a pull request's and sorts
//! every change into refused, needing a kernel owner's approval, or free. `.github/workflows/`
//! `kernel-golden.yml` runs it through the `fixture diff` binary and holds the pull request to its
//! answer, with the golden store's own approval rule (`golden::diff::approved`).
//!
//! The rules:
//!
//! - **Fixtures are never deleted**, and a fixture's id, log and source never change: a new case is
//!   a new fixture. A fix may be recorded on a fixture that had none.
//! - **Frozen editions never change**: an expectation of a frozen edition cannot move or go.
//! - **An expectation of the open edition** may change only with a [`super::Change`] that holds
//!   what it was, and a kernel owner's approval on the pull request, as a golden's may. It cannot
//!   be removed.
//! - **`Pass` never becomes `Known`**: a stored success that fails is a regression, not a status.
//! - **`Known` becomes `Pass` freely** when the new expectation is the one it wanted: that is
//!   [`super::run::Verdict::Improved`] being carried out, and no result anyone relied on moves.
//! - **History is only added to.**
//!
//! Guarantees: every change to a stored fixture is sorted, so one that fits no rule is refused;
//! the answer depends only on the two sets given; nothing reads a clock or the network.
//!
//! Method: chapters 28.13 and 28.16 (`projects/engines/geometry/architecture/28-testing.md`), and
//! the golden store's rules in `golden::diff`.
//!
//! Cost: linear in the two sets.

use std::collections::BTreeMap;

use super::run::{Replayed, matches};
use super::{Expect, Fixture, Status};
use crate::golden::store::State;

/// What a pull request does to the fixtures.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    /// Fixtures it adds.
    pub added: Vec<String>,
    /// Changes no approval can make, each said in words.
    pub refused: Vec<String>,
    /// Changes that need a kernel owner's approval.
    pub approval: Vec<String>,
    /// Changes that need neither.
    pub free: Vec<String>,
}

impl Diff {
    /// Whether every change can stand without an approval.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.refused.is_empty() && self.approval.is_empty()
    }
}

/// Whether `head`'s expectation is the success `wanted` stated.
fn is_wanted(head: &Expect, wanted: &Expect) -> bool {
    match head {
        Expect::Ok {
            hash: Some(hash),
            measures,
        } => matches(
            &Replayed::Done {
                hash: *hash,
                measures: measures.clone(),
            },
            wanted,
        ),
        _ => false,
    }
}

/// One stored fixture against its new form.
fn compare(
    base: &Fixture,
    head: &Fixture,
    state_of: &impl Fn(&str) -> Option<State>,
    out: &mut Diff,
) {
    let id = &base.id;
    if base.log != head.log {
        out.refused.push(format!(
            "{id}: its log changed; a new case is a new fixture"
        ));
    }
    if base.source != head.source {
        out.refused.push(format!("{id}: its source changed"));
    }
    match (&base.fixed_by, &head.fixed_by) {
        (a, b) if a == b => {}
        (None, Some(fix)) => out.free.push(format!("{id}: records the fix {fix}")),
        _ => out.refused.push(format!("{id}: its recorded fix changed")),
    }
    if base.speed != head.speed {
        out.free.push(format!("{id}: moved to {:?}", head.speed));
    }
    if !head.history.starts_with(&base.history) {
        out.refused.push(format!("{id}: its history was rewritten"));
    }
    let improved = match (&base.status, &head.status) {
        (Status::Pass, Status::Known { .. }) => {
            out.refused.push(format!(
                "{id}: Pass became Known; a stored success that fails is a regression"
            ));
            None
        }
        (Status::Known { wanted }, Status::Pass) => Some(wanted),
        _ => None,
    };
    if matches!(base.status, Status::Known { .. })
        && matches!(head.status, Status::Known { .. })
        && base.status != head.status
    {
        out.approval
            .push(format!("{id}: its wanted result changed"));
    }
    let added = head.history.len().saturating_sub(base.history.len());
    let recorded = head.history.get(base.history.len()..).unwrap_or_default();
    let mut changes = 0_usize;
    for (key, was) in &base.expect {
        let (edition, operations) = key;
        match head.expect.get(key) {
            None => out.refused.push(format!(
                "{id}: the expectation of {edition} at {operations:?} was removed"
            )),
            Some(now) if now == was => {}
            Some(now) => match state_of(edition) {
                Some(State::Open) if improved.is_some_and(|wanted| is_wanted(now, wanted)) => {
                    out.free.push(format!("{id}: Known moved to Pass at {operations:?}"));
                }
                Some(State::Open) => {
                    changes = changes.saturating_add(1);
                    if recorded.iter().any(|c| {
                        c.edition == *edition && c.operations == *operations && c.was == *was
                    }) {
                        out.approval.push(format!(
                            "{id}: the expectation of {edition} at {operations:?} changed"
                        ));
                    } else {
                        out.refused.push(format!(
                            "{id}: the expectation of {edition} at {operations:?} changed without a record"
                        ));
                    }
                }
                _ => out.refused.push(format!(
                    "{id}: the expectation of {edition} at {operations:?} changed, and that edition is frozen"
                )),
            },
        }
    }
    if added != changes {
        out.refused.push(format!(
            "{id}: {added} new history records for {changes} changed expectations"
        ));
    }
    for ((edition, operations), _) in head
        .expect
        .iter()
        .filter(|(k, _)| !base.expect.contains_key(*k))
    {
        out.free.push(format!(
            "{id}: a new expectation of {edition} at {operations:?}"
        ));
    }
}

/// Sorts what `head` does to `base`. `state_of` says whether an edition is open or frozen, from
/// the golden store; an edition it does not know counts as frozen.
#[must_use]
pub fn diff(base: &[Fixture], head: &[Fixture], state_of: impl Fn(&str) -> Option<State>) -> Diff {
    let before: BTreeMap<&str, &Fixture> = base.iter().map(|f| (f.id.as_str(), f)).collect();
    let after: BTreeMap<&str, &Fixture> = head.iter().map(|f| (f.id.as_str(), f)).collect();
    let mut out = Diff::default();
    for (id, fixture) in &before {
        match after.get(id) {
            Some(now) => compare(fixture, now, &state_of, &mut out),
            None => out
                .refused
                .push(format!("{id}: removed; fixtures are never deleted")),
        }
    }
    out.added = after
        .keys()
        .filter(|id| !before.contains_key(*id))
        .map(|id| (*id).to_owned())
        .collect();
    out
}
