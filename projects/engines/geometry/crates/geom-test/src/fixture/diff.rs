//! What a pull request may do to the fixtures already stored, and the record every changed
//! expectation carries (chapters 28.13 and 28.16; piece G97; decision 32.55).
//!
//! What it does: [`diff`] compares the fixtures of a base commit with a pull request's and sorts
//! every change into refused, changed on its record, or free. `.github/workflows/`
//! `kernel-golden.yml` runs it through the `fixture diff` binary and holds the pull request to its
//! answer, which prints each changed expectation with its record, for the merge record to name.
//!
//! The rules:
//!
//! - **Fixtures are never deleted**, and a fixture's id, log and source never change: a new case is
//!   a new fixture. A fix may be recorded on a fixture that had none.
//! - **Frozen editions never change**: an expectation of a frozen edition cannot move or go.
//! - **A changed expectation carries its record.** An expectation of the open edition may change
//!   only with a new [`super::Change`] at its key (edition and operation versions) whose `was` is
//!   the stored expectation, whose kind the fixture's rules allow for that change
//!   ([`super::Change::allows`]), and which names its piece and says why. A change with no record,
//!   a record of another old value, a kind the rules do not allow, or an empty piece or reason is
//!   refused. It cannot be removed.
//! - **A `Known` fixture's wanted result** changes on the same terms: its record holds the old
//!   wanted result, at the log's own operation versions in the open edition.
//! - **`Pass` never becomes `Known`**: a stored success that fails is a regression, not a status.
//! - **`Known` becomes `Pass` freely** when the new expectation is the one it wanted: that is
//!   [`super::run::Verdict::Improved`] being carried out, and no result anyone relied on moves.
//! - **History is only added to**, and only by the records of the changes made: as many new
//!   records as changed values.
//! - **A work count moves only with a new operation version** (G98): an expectation that differs in
//!   its count and nothing else is refused, recorded or not.
//!
//! Until decision 32.55 a recorded change also waited for a kernel owner's approving review on the
//! pull request's head commit. One account opens every pull request and no kernel owner reviews
//! them, so that review never came; the record is what a reviewer would have read, and this module
//! checks all of it, as the golden store's check does since decision 32.54 (`golden::diff`).
//!
//! Guarantees: every change to a stored fixture is sorted, so one that fits no rule is refused;
//! every changed value that stands is listed with its record ([`Diff::changed`]); the answer
//! depends only on the two sets given; nothing reads a clock or the network.
//!
//! Method: chapters 28.13 and 28.16 (`projects/engines/geometry/architecture/28-testing.md`), and
//! the golden store's rules in `golden::diff`.
//!
//! Cost: linear in the two sets, and in the new history records for each changed value.

use core::fmt;
use std::collections::BTreeMap;

use super::run::{Replayed, matches};
use super::{Change, Expect, Fixture, Spend, Status};
use crate::golden::store::State;

/// A stored value that changed, with the record that says why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changed {
    /// The fixture.
    pub id: String,
    /// What changed, in words: an expectation and its key, or the wanted result.
    pub what: String,
    /// The history record the pull request appended, which holds what it was.
    pub record: Change,
}

/// What a pull request does to the fixtures.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    /// Fixtures it adds.
    pub added: Vec<String>,
    /// Changes that cannot merge, each said in words.
    pub refused: Vec<String>,
    /// Stored values that changed, each with its record.
    pub changed: Vec<Changed>,
    /// Changes that need no record.
    pub free: Vec<String>,
}

impl Diff {
    /// Whether the change can merge: nothing refused.
    #[must_use]
    pub fn is_acceptable(&self) -> bool {
        self.refused.is_empty()
    }
}

impl fmt::Display for Diff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for id in &self.added {
            writeln!(f, "added: {id}")?;
        }
        for line in &self.free {
            writeln!(f, "{line}")?;
        }
        for c in &self.changed {
            writeln!(
                f,
                "changed: {}: {}; recorded as {} by {}: {}",
                c.id, c.what, c.record.kind, c.record.piece, c.record.why
            )?;
        }
        for line in &self.refused {
            writeln!(f, "refused: {line}")?;
        }
        write!(
            f,
            "{}",
            if !self.is_acceptable() {
                "verdict: refused; these changes cannot merge"
            } else if !self.changed.is_empty() {
                "verdict: every changed expectation carries its record; the merge record names each \
                 one"
            } else {
                "verdict: no stored expectation changed"
            }
        )
    }
}

/// Whether `head`'s expectation is the success `wanted` stated.
fn is_wanted(head: &Expect, wanted: &Expect) -> bool {
    match head {
        Expect::Ok {
            hash: Some(hash),
            measures,
            ..
        } => matches(
            &Replayed::Done {
                hash: *hash,
                measures: measures.clone(),
                work: Spend::default(),
            },
            wanted,
        ),
        _ => false,
    }
}

/// Whether two expectations differ in their work count and nothing else.
fn only_work_moved(was: &Expect, now: &Expect) -> bool {
    let moved = |before: &Option<Spend>, after: &Option<Spend>| before != after;
    match (was, now) {
        (
            Expect::Ok {
                hash,
                work,
                measures,
            },
            Expect::Ok {
                hash: new_hash,
                work: new_work,
                measures: new_measures,
            },
        ) => hash == new_hash && measures == new_measures && moved(work, new_work),
        (
            Expect::Err {
                kind,
                entities,
                work,
            },
            Expect::Err {
                kind: new_kind,
                entities: new_entities,
                work: new_work,
            },
        ) => kind == new_kind && entities == new_entities && moved(work, new_work),
        _ => false,
    }
}

/// One stored value that changed from `was` to `now`.
struct Move<'a> {
    /// The fixture.
    id: &'a str,
    /// What changed, in words.
    what: String,
    /// What it was.
    was: &'a Expect,
    /// What it is now.
    now: &'a Expect,
}

/// Holds one changed value to its record among `recorded`, the history records the pull request
/// appended; `at` says which of them sit where its record must.
fn hold(m: Move<'_>, recorded: &[Change], at: impl Fn(&Change) -> bool, out: &mut Diff) {
    let Move { id, what, was, now } = m;
    let placed: Vec<&Change> = recorded.iter().filter(|c| at(c)).collect();
    match placed.iter().find(|c| c.was == *was) {
        None if placed.is_empty() => out.refused.push(format!(
            "{id}: {what} changed without a record: a new history record holding what it was, its \
             kind (bug, improvement or corpus), its piece and why"
        )),
        None => out.refused.push(format!(
            "{id}: {what} changed, but its new history record holds another old value than the \
             stored one"
        )),
        Some(record) if !record.allows(now) => out.refused.push(format!(
            "{id}: {what} changed on a record of kind {}, which the fixture's rules do not allow \
             for it: `corpus` only where the hash alone moved, and `bug` or `improvement` only in \
             pre-edition, since after it a fixed or improved result is a new operation version \
             (26.2, 28.13)",
            record.kind
        )),
        Some(record) if record.piece.trim().is_empty() || record.why.trim().is_empty() => {
            out.refused.push(format!(
                "{id}: {what} changed on a record that names no piece or gives no reason"
            ));
        }
        Some(record) => out.changed.push(Changed {
            id: id.to_owned(),
            what,
            record: (*record).clone(),
        }),
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
    let added = head.history.len().saturating_sub(base.history.len());
    let recorded = head.history.get(base.history.len()..).unwrap_or_default();
    let mut changes = 0_usize;
    let improved = match (&base.status, &head.status) {
        (Status::Pass, Status::Known { .. }) => {
            out.refused.push(format!(
                "{id}: Pass became Known; a stored success that fails is a regression"
            ));
            None
        }
        (Status::Known { wanted }, Status::Pass) => Some(wanted),
        (Status::Known { wanted: was }, Status::Known { wanted: now }) if was != now => {
            changes = changes.saturating_add(1);
            let own = head.log.versions();
            hold(
                Move {
                    id,
                    what: "its wanted result".to_owned(),
                    was,
                    now,
                },
                recorded,
                |c| c.operations == own && state_of(&c.edition) == Some(State::Open),
                out,
            );
            None
        }
        _ => None,
    };
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
                Some(State::Open) if only_work_moved(was, now) => {
                    out.refused.push(format!(
                        "{id}: the work count of {edition} at {operations:?} moved; a count \
                         changes only with a new operation version"
                    ));
                }
                Some(State::Open) => {
                    changes = changes.saturating_add(1);
                    hold(
                        Move {
                            id,
                            what: format!("the expectation of {edition} at {operations:?}"),
                            was,
                            now,
                        },
                        recorded,
                        |c| c.edition == *edition && c.operations == *operations,
                        out,
                    );
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
