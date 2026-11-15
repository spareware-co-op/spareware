//! The record rule for edits to the golden store: what a change to the store does, what it may
//! never do, and the record every moved answer carries.
//!
//! What it does: [`diff`] compares the store at a pull request's base with the store at its head
//! and returns a [`Diff`]: the goldens added, the answers that moved (each with the history record
//! that says why), the other edits to existing goldens, and every [`Diff::refusals`] entry, a
//! change that cannot merge. The binary `golden` runs it in `.github/workflows/kernel-golden.yml`
//! on every pull request that edits the store, and prints each moved answer with its record, for
//! the merge record to name (decision 32.54).
//!
//! The rules, from chapter 28.13 ("Within an edition, a Stable operation's golden hash never
//! changes. A changed hash is a bug, or an improvement that ships as a new operation version with
//! new goldens beside the old. A pull request that edits a golden says which, in the record it
//! appends to the golden's history."), 26.2 ("Fixing a bug or improving a result makes a new
//! version."), 28.16 ("Fixtures are never deleted") and 28.19 ("Never changes a Stable golden hash
//! within an edition"):
//!
//! - **A moved answer carries its record.** A golden whose digest changed appends exactly one
//!   history record, whose `sha256` is the base's digest, whose kind the golden allows
//!   ([`Golden::allows`]: only `corpus` on a golden that covers operation versions), and which
//!   names its piece and says why. A move with no record, with two, with a record of another
//!   digest, with a kind the golden does not allow, or with an empty piece or reason is refused.
//! - **Refused outright:** besides a move without its record, any change to a frozen edition (its
//!   goldens, digests, histories, checks or state); removing or renaming an edition, or reordering
//!   editions; removing a golden, which includes changing its key (its id or its operation
//!   versions: a new operation version is a new golden beside the old, 28.13); a history record
//!   added without a changed digest; rewriting or removing history; two open editions; and
//!   releasing an edition (the open edition frozen, a new open edition after it), which no rule
//!   here can hold to anything yet: the first Stable release (board piece G482) writes that rule.
//! - **Accepted:** adding goldens; a moved answer with its record; and an edit that moves no answer
//!   (where a golden is checked, the file it pins, what it covers), which `golden check`, in the
//!   same job, holds to the repository: every golden is checked where the store says, and every
//!   pinned file has the store's digest.
//!
//! Until decision 32.54 a moved answer, and every other edit to an existing golden, also waited for
//! a kernel owner's approving review on the pull request's head commit. One account opens every
//! pull request and no kernel owner reviews them, so that review never came and the check's red
//! carried no signal. The record is what a reviewer would have read, and this module checks all of
//! it. [`approved`], the review rule, stays for the fixtures' own edits (`fixture diff`, 28.16): it
//! counts an approving review on the pull request's current head commit from a reviewer GitHub
//! reports as a member or owner of the organisation, or as having write, maintain or admin
//! permission on the repository, who is not the pull request's author, with no such reviewer's
//! latest review asking for changes.
//!
//! Guarantees:
//!
//! - **Every difference is seen.** Goldens are matched by edition, id and operation versions; every
//!   field is compared; a golden present on one side only is an addition or a removal.
//! - **A refusal is final.** [`Diff::is_acceptable`] is false whenever a refusal exists.
//! - **Every moved answer is named.** [`Diff::moved`] holds each moved digest with the record that
//!   explains it, and the report prints each one.
//! - **Approval is per head commit.** For [`approved`], a review of an earlier commit does not
//!   approve later changes.
//! - **Deterministic.** Reports list editions in store order and goldens in key order.
//!
//! Method: a three-way rule check over the two parsed stores (base, head) in the manner of
//! `cargo-semver-checks`' comparison of a crate's public API across two versions
//! (<https://github.com/obi1kenobi/cargo-semver-checks>): every difference is classified against a
//! written rule; the review rule follows GitHub's documentation of pull request reviews
//! (<https://docs.github.com/en/rest/pulls/reviews>) and author associations
//! (<https://docs.github.com/en/graphql/reference/enums#commentauthorassociation>).
//!
//! Cost: linear in the two stores' sizes, with a logarithmic factor for the keyed maps; linear in
//! the number of reviews.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use super::store::{Change, Edition, Golden, State, Store};

/// A golden's key: edition, id and operation versions.
pub type Key = (String, String, BTreeMap<String, u32>);

/// A stored answer that moved, with the record that says why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moved {
    /// The golden.
    pub key: Key,
    /// Its digest at the base.
    pub was: String,
    /// Its digest at the head.
    pub now: String,
    /// The history record the head appended, which holds `was`.
    pub record: Change,
}

/// What a pull request does to the golden store.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    /// Goldens added.
    pub added: Vec<Key>,
    /// Answers that moved, each with its record.
    pub moved: Vec<Moved>,
    /// Other edits to existing goldens, each with what changed in words; none moves an answer.
    pub changed: Vec<(Key, Vec<String>)>,
    /// Changes that cannot merge.
    pub refusals: Vec<String>,
}

impl Diff {
    /// Whether the change can merge: no refusal.
    #[must_use]
    pub fn is_acceptable(&self) -> bool {
        self.refusals.is_empty()
    }
}

fn key(edition: &Edition, golden: &Golden) -> Key {
    (
        edition.edition.clone(),
        golden.id.clone(),
        golden.operations.clone(),
    )
}

fn show(key: &Key) -> String {
    if key.2.is_empty() {
        format!("{} in edition {}", key.1, key.0)
    } else {
        format!("{} {:?} in edition {}", key.1, key.2, key.0)
    }
}

impl fmt::Display for Diff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for k in &self.added {
            writeln!(f, "added: {}", show(k))?;
        }
        for m in &self.moved {
            writeln!(
                f,
                "moved: {}: {} -> {}; recorded as {} by {}: {}",
                show(&m.key),
                m.was,
                m.now,
                m.record.kind,
                m.record.piece,
                m.record.why
            )?;
        }
        for (k, what) in &self.changed {
            writeln!(f, "changed: {}: {}", show(k), what.join("; "))?;
        }
        for r in &self.refusals {
            writeln!(f, "refused: {r}")?;
        }
        if self.added.is_empty()
            && self.moved.is_empty()
            && self.changed.is_empty()
            && self.refusals.is_empty()
        {
            writeln!(f, "the golden store is unchanged")?;
        }
        write!(
            f,
            "{}",
            if !self.is_acceptable() {
                "verdict: refused; these changes cannot merge"
            } else if !self.moved.is_empty() {
                "verdict: every moved answer carries its record; the merge record names each one"
            } else {
                "verdict: no stored answer moved"
            }
        )
    }
}

/// Holds one golden's edit to the rules: a moved digest needs exactly one new history record
/// holding the old digest, of a kind the golden allows, naming its piece and saying why. What it
/// finds goes into `d`.
fn compare(d: &mut Diff, key: &Key, base: &Golden, head: &Golden) {
    let at = show(key);
    if base.sha256 != head.sha256 {
        let appended = head.history.len() == base.history.len().saturating_add(1)
            && head.history.starts_with(&base.history);
        match head.history.last().filter(|_| appended) {
            None => d.refusals.push(format!(
                "{at}: its digest moved from {} to {} without exactly one new history record \
                 holding the old digest, its kind (bug, improvement or corpus), its piece and why",
                base.sha256, head.sha256
            )),
            Some(record) if record.sha256 != base.sha256 => d.refusals.push(format!(
                "{at}: its digest moved from {} to {}, but its new history record holds {}, not \
                 the old digest",
                base.sha256, head.sha256, record.sha256
            )),
            Some(record) if !head.allows(record.kind) => d.refusals.push(format!(
                "{at}: its new history record is of kind {}, which this golden does not allow: a \
                 golden that covers operation versions records only `corpus`, and a fixed or \
                 improved result is a new operation version beside the old (26.2, 28.13)",
                record.kind
            )),
            Some(record) if record.piece.trim().is_empty() || record.why.trim().is_empty() => {
                d.refusals.push(format!(
                    "{at}: its new history record names no piece or gives no reason"
                ));
            }
            Some(record) => d.moved.push(Moved {
                key: key.clone(),
                was: base.sha256.clone(),
                now: head.sha256.clone(),
                record: record.clone(),
            }),
        }
    } else if base.history != head.history {
        if head.history.starts_with(&base.history) {
            d.refusals.push(format!(
                "{at}: a history record was added but the digest did not change; a record says \
                 what a change replaced"
            ));
        } else {
            d.refusals.push(format!(
                "{at}: its history was rewritten; history only grows"
            ));
        }
    }
    let mut what = Vec::new();
    if base.check != head.check {
        what.push(format!("check {:?} -> {:?}", base.check, head.check));
    }
    if base.file != head.file {
        what.push(format!("file {:?} -> {:?}", base.file, head.file));
    }
    if base.what != head.what {
        what.push("what it covers".to_owned());
    }
    if !what.is_empty() {
        d.changed.push((key.clone(), what));
    }
}

/// Compares the store at a pull request's base (`None` when the base has no store) with the store
/// at its head.
#[must_use]
pub fn diff(base: Option<&Store>, head: &Store) -> Diff {
    let mut d = Diff::default();
    let empty: &[Edition] = &[];
    let base_editions = base.map_or(empty, |s| s.editions.as_slice());
    // Editions: the base's, in order, then new ones.
    let base_labels: Vec<&str> = base_editions.iter().map(|e| e.edition.as_str()).collect();
    let head_labels: Vec<&str> = head.editions.iter().map(|e| e.edition.as_str()).collect();
    if !head_labels.starts_with(&base_labels) {
        d.refusals.push(format!(
            "editions {base_labels:?} became {head_labels:?}: an edition is never removed, \
             renamed or moved"
        ));
    }
    let open_count = head
        .editions
        .iter()
        .filter(|e| e.state == State::Open)
        .count();
    if open_count != 1 {
        d.refusals
            .push(format!("{open_count} open editions; exactly one is open"));
    }
    let head_by_label: BTreeMap<&str, &Edition> = head
        .editions
        .iter()
        .map(|e| (e.edition.as_str(), e))
        .collect();
    for b in base_editions {
        let Some(h) = head_by_label.get(b.edition.as_str()) else {
            continue;
        };
        if b.state == State::Frozen {
            if *h != b {
                d.refusals.push(format!(
                    "edition {} is frozen (released) and was changed; nothing in a frozen edition \
                     ever changes (28.13, 28.19)",
                    b.edition
                ));
            }
            continue;
        }
        if h.state == State::Frozen {
            d.refusals.push(format!(
                "edition {} was released (frozen, with a new open edition after it); a pull \
                 request cannot release an edition until the first Stable release (G482) writes \
                 the rule a release is held to",
                b.edition
            ));
        }
        let head_goldens: BTreeMap<Key, &Golden> =
            h.goldens.iter().map(|g| (key(h, g), g)).collect();
        let mut base_keys = BTreeSet::new();
        for g in &b.goldens {
            let k = key(b, g);
            base_keys.insert(k.clone());
            match head_goldens.get(&k) {
                None => d.refusals.push(format!(
                    "{} was removed; goldens are never deleted (28.16)",
                    show(&k)
                )),
                Some(hg) => compare(&mut d, &k, g, hg),
            }
        }
        for k in head_goldens.keys() {
            if !base_keys.contains(k) {
                d.added.push(k.clone());
            }
        }
    }
    for h in head.editions.iter().skip(base_editions.len()) {
        for g in &h.goldens {
            d.added.push(key(h, g));
        }
    }
    d.added.sort();
    d.moved.sort_by(|a, b| a.key.cmp(&b.key));
    d.changed.sort();
    d
}

/// One review of a pull request, as GitHub's REST API lists it (the fields used).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Review {
    /// The reviewer.
    pub user: Option<Reviewer>,
    /// `APPROVED`, `CHANGES_REQUESTED`, `COMMENTED`, `DISMISSED` or `PENDING`.
    pub state: String,
    /// The commit the review was made on.
    pub commit_id: Option<String>,
    /// The reviewer's association with the repository: `OWNER`, `MEMBER`, `COLLABORATOR`, ...
    pub author_association: String,
}

/// A reviewer's account.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Reviewer {
    /// The account's login.
    pub login: String,
}

/// Why a pull request's reviews do not approve an edit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalError {
    /// The reviews are not GitHub's JSON (serde's message).
    Json(String),
    /// No qualifying approval on the head commit.
    NoApproval,
    /// A qualifying reviewer's latest review asks for changes.
    ChangesRequested(String),
}

impl fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApprovalError::Json(m) => write!(f, "the reviews cannot be read: {m}"),
            ApprovalError::NoApproval => write!(
                f,
                "this pull request edits a golden, and no kernel owner has approved its current \
                 head commit: a review approving this commit, from an organisation member or a \
                 reviewer with write access who is not the author, is required (chapter 28.13; \
                 .github/CODEOWNERS names @spareware-co-op/owners)"
            ),
            ApprovalError::ChangesRequested(who) => write!(
                f,
                "{who} asked for changes in their latest review; an edit to a golden merges only \
                 when no kernel owner's latest review asks for changes"
            ),
        }
    }
}

impl core::error::Error for ApprovalError {}

/// Whether a reviewer's association or permission qualifies them as a kernel owner (see the module
/// documentation).
fn qualifies(association: &str, permission: Option<&str>) -> bool {
    matches!(association, "OWNER" | "MEMBER")
        || matches!(permission, Some("admin" | "maintain" | "write"))
}

/// Decides whether `reviews` (GitHub's JSON list of a pull request's reviews, oldest first)
/// approve the head commit `head` of a pull request by `author`; `permissions` maps a login to its
/// repository permission where the workflow could read it. The fixtures' rule (`fixture diff`)
/// asks for it; the golden store's own edits are held to their records instead (decision 32.54).
///
/// # Errors
///
/// An [`ApprovalError`] saying what is missing.
pub fn approved(
    reviews: &str,
    head: &str,
    author: &str,
    permissions: &BTreeMap<String, String>,
) -> Result<String, ApprovalError> {
    let reviews: Vec<Review> =
        serde_json::from_str(reviews).map_err(|e| ApprovalError::Json(e.to_string()))?;
    // Each reviewer's latest review that is not a plain comment or a pending draft.
    let mut latest: BTreeMap<String, &Review> = BTreeMap::new();
    for r in &reviews {
        let Some(user) = &r.user else { continue };
        if matches!(r.state.as_str(), "COMMENTED" | "PENDING") {
            continue;
        }
        latest.insert(user.login.clone(), r);
    }
    let mut approvers = Vec::new();
    for (login, r) in &latest {
        if login == author {
            continue;
        }
        if !qualifies(
            &r.author_association,
            permissions.get(login).map(String::as_str),
        ) {
            continue;
        }
        match r.state.as_str() {
            "CHANGES_REQUESTED" => return Err(ApprovalError::ChangesRequested(login.clone())),
            "APPROVED" if r.commit_id.as_deref() == Some(head) => approvers.push(login.clone()),
            _ => {}
        }
    }
    if approvers.is_empty() {
        Err(ApprovalError::NoApproval)
    } else {
        Ok(approvers.join(", "))
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use super::super::store::ChangeKind;
    use super::*;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const D: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

    fn golden(id: &str, sha: &str, history: &str) -> String {
        format!(
            r#"{{"id": "{id}", "what": "w", "sha256": "{sha}", "history": [{history}],
                "check": {{"crate": "geom-x", "test": "t", "function": "f"}}}}"#
        )
    }

    /// A golden that covers the operation `extrude` at version 1.
    fn versioned(id: &str, sha: &str, history: &str) -> String {
        golden(id, sha, history).replace(
            "\"history\"",
            "\"operations\": {\"extrude\": 1}, \"history\"",
        )
    }

    fn record(sha: &str) -> String {
        record_of(sha, "improvement")
    }

    fn record_of(sha: &str, kind: &str) -> String {
        format!(r#"{{"sha256": "{sha}", "kind": "{kind}", "piece": "G1", "why": "w"}}"#)
    }

    fn store(editions: &[(&str, &str, &[String])]) -> Store {
        let editions: Vec<String> = editions
            .iter()
            .map(|(label, state, goldens)| {
                format!(
                    r#"{{"edition": "{label}", "state": "{state}", "goldens": [{}]}}"#,
                    goldens.join(",")
                )
            })
            .collect();
        Store::parse(&format!(
            r#"{{"format": 1, "about": "t", "editions": [{}]}}"#,
            editions.join(",")
        ))
        .unwrap()
    }

    fn open(goldens: &[String]) -> Store {
        store(&[("pre-edition", "open", goldens)])
    }

    /// The last history record of the first golden of the open edition, to edit in memory past
    /// what a parse allows.
    fn last_record(s: &mut Store) -> &mut Change {
        s.editions[0].goldens[0].history.last_mut().unwrap()
    }

    #[test]
    fn a_new_store_only_adds() {
        let head = open(&[golden("geom-x/a", A, "")]);
        let d = diff(None, &head);
        assert_eq!(d.added.len(), 1);
        assert!(d.is_acceptable() && d.moved.is_empty());
        let text = d.to_string();
        assert!(text.contains("no stored answer moved") && !text.contains("unchanged"));
    }

    #[test]
    fn unchanged_and_added_goldens_move_nothing() {
        let base = open(&[golden("geom-x/a", A, "")]);
        let head = open(&[golden("geom-x/a", A, ""), golden("geom-x/b", B, "")]);
        let d = diff(Some(&base), &head);
        assert_eq!(d.added.len(), 1);
        assert!(d.changed.is_empty() && d.moved.is_empty() && d.is_acceptable());
        let same = diff(Some(&base), &base).to_string();
        assert!(same.contains("unchanged") && same.contains("no stored answer moved"));
    }

    /// The planted move with a proper record passes, and the report names it with its record.
    #[test]
    fn a_recorded_move_passes_and_is_named() {
        let base = open(&[golden("geom-x/a", A, "")]);
        let head = open(&[golden("geom-x/a", B, &record(A))]);
        let d = diff(Some(&base), &head);
        assert!(d.is_acceptable(), "{d}");
        assert_eq!(d.moved.len(), 1);
        let m = &d.moved[0];
        assert_eq!((m.was.as_str(), m.now.as_str()), (A, B));
        assert_eq!(m.record.kind, ChangeKind::Improvement);
        assert!(d.changed.is_empty(), "{d}");
        let text = d.to_string();
        assert!(
            text.contains(&format!(
                "moved: geom-x/a in edition pre-edition: {A} -> {B}; recorded as improvement by \
                 G1: w"
            )),
            "{text}"
        );
        assert!(text.contains("carries its record"), "{text}");
        assert!(!text.contains("unchanged"), "{text}");
        // A golden of operation versions moves with a corpus record.
        let base = open(&[versioned("geom-x/a", A, "")]);
        let head = open(&[versioned("geom-x/a", B, &record_of(A, "corpus"))]);
        let d = diff(Some(&base), &head);
        assert!(d.is_acceptable(), "{d}");
        assert_eq!(d.moved.len(), 1);
        assert!(d.to_string().contains("{\"extrude\": 1}"), "{d}");
    }

    #[test]
    fn every_refusal_fires() {
        let base = open(&[golden("geom-x/a", B, &record(C))]);
        let cases: Vec<(&str, Store, &str)> = vec![
            (
                "a move with no record",
                open(&[golden("geom-x/a", A, &record(C))]),
                "without exactly one new history record",
            ),
            (
                "a record of the wrong old digest",
                open(&[golden(
                    "geom-x/a",
                    A,
                    &format!("{},{}", record(C), record(D)),
                )]),
                "not the old digest",
            ),
            (
                "two records at once",
                open(&[golden(
                    "geom-x/a",
                    A,
                    &format!("{},{},{}", record(C), record(A), record(B)),
                )]),
                "without exactly one new history record",
            ),
            (
                "history rewritten",
                open(&[golden("geom-x/a", B, &record(A))]),
                "rewritten",
            ),
            (
                "history dropped",
                open(&[golden("geom-x/a", B, "")]),
                "rewritten",
            ),
            (
                "history added without a move",
                open(&[golden(
                    "geom-x/a",
                    B,
                    &format!("{},{}", record(C), record(D)),
                )]),
                "did not change",
            ),
            (
                "golden removed",
                open(&[golden("geom-x/z", B, "")]),
                "removed",
            ),
        ];
        for (name, head, says) in cases {
            let d = diff(Some(&base), &head);
            assert!(!d.is_acceptable(), "{name} was accepted:\n{d}");
            let text = d.to_string();
            assert!(
                text.contains("refused") && text.contains(says),
                "{name}:\n{text}"
            );
            assert!(d.moved.is_empty(), "{name}");
        }
    }

    /// What a parse already refuses is refused here too, for a store built in memory: an empty
    /// reason or piece, and a kind the golden does not allow.
    #[test]
    fn an_empty_reason_and_a_kind_the_golden_does_not_allow_are_refused() {
        let base = open(&[golden("geom-x/a", A, "")]);
        let good = open(&[golden("geom-x/a", B, &record(A))]);
        assert!(diff(Some(&base), &good).is_acceptable());
        for blank in ["", "  "] {
            let mut no_reason = good.clone();
            last_record(&mut no_reason).why = blank.to_owned();
            let mut no_piece = good.clone();
            last_record(&mut no_piece).piece = blank.to_owned();
            for head in [no_reason, no_piece] {
                let d = diff(Some(&base), &head);
                assert!(!d.is_acceptable(), "{d}");
                assert!(d.to_string().contains("no piece or gives no reason"), "{d}");
            }
        }
        let base = open(&[versioned("geom-x/a", A, "")]);
        let good = open(&[versioned("geom-x/a", B, &record_of(A, "corpus"))]);
        assert!(diff(Some(&base), &good).is_acceptable());
        for kind in [ChangeKind::Bug, ChangeKind::Improvement] {
            let mut head = good.clone();
            last_record(&mut head).kind = kind;
            let d = diff(Some(&base), &head);
            assert!(!d.is_acceptable(), "{kind} was accepted:\n{d}");
            assert!(
                d.to_string()
                    .contains(&format!("of kind {kind}, which this golden does not allow")),
                "{d}"
            );
        }
    }

    #[test]
    fn frozen_editions_never_change_editions_never_move_and_none_is_released_here() {
        let g = golden("geom-x/a", A, "");
        let base = store(&[
            ("one", "frozen", std::slice::from_ref(&g)),
            ("pre-edition", "open", std::slice::from_ref(&g)),
        ]);
        let changed_frozen = store(&[
            ("one", "frozen", &[golden("geom-x/a", B, &record(A))]),
            ("pre-edition", "open", std::slice::from_ref(&g)),
        ]);
        assert!(!diff(Some(&base), &changed_frozen).is_acceptable());
        let added_to_frozen = store(&[
            ("one", "frozen", &[g.clone(), golden("geom-x/b", B, "")]),
            ("pre-edition", "open", std::slice::from_ref(&g)),
        ]);
        assert!(!diff(Some(&base), &added_to_frozen).is_acceptable());
        let removed_edition = store(&[("pre-edition", "open", std::slice::from_ref(&g))]);
        assert!(!diff(Some(&base), &removed_edition).is_acceptable());
        // Releasing the open edition: frozen, with a new open edition after it.
        let released = store(&[
            ("one", "frozen", std::slice::from_ref(&g)),
            ("pre-edition", "frozen", std::slice::from_ref(&g)),
            ("two", "open", std::slice::from_ref(&g)),
        ]);
        let d = diff(Some(&base), &released);
        assert!(!d.is_acceptable(), "{d}");
        assert!(
            d.to_string().contains("edition pre-edition was released"),
            "{d}"
        );
        assert_eq!(d.refusals.len(), 1, "{d}");
        // Two open editions.
        let mut two_open = base.clone();
        two_open.editions[0].state = State::Open;
        assert!(
            diff(Some(&base), &two_open)
                .to_string()
                .contains("2 open editions")
        );
    }

    #[test]
    fn an_edit_that_moves_no_answer_passes_and_is_listed() {
        let base = open(&[golden("geom-x/a", A, "")]);
        let edits = [
            golden("geom-x/a", A, "").replace("\"f\"", "\"g\""),
            golden("geom-x/a", A, "").replace("\"what\": \"w\"", "\"what\": \"v\""),
            golden("geom-x/a", A, "").replace(
                "\"check\": {\"crate\": \"geom-x\", \"test\": \"t\", \"function\": \"f\"}",
                "\"file\": \"a/b.txt\"",
            ),
        ];
        for (edit, says) in edits.iter().zip(["check", "what it covers", "file"]) {
            let d = diff(Some(&base), &open(std::slice::from_ref(edit)));
            assert!(d.is_acceptable() && d.moved.is_empty(), "{d}");
            assert_eq!(d.changed.len(), 1, "{d}");
            let text = d.to_string();
            assert!(
                text.contains("changed: geom-x/a") && text.contains(says),
                "{text}"
            );
            assert!(text.contains("no stored answer moved"), "{text}");
        }
    }

    fn review(login: &str, state: &str, commit: &str, association: &str) -> String {
        format!(
            r#"{{"user": {{"login": "{login}"}}, "state": "{state}", "commit_id": "{commit}",
                "author_association": "{association}", "body": "", "id": 1}}"#
        )
    }

    #[test]
    fn approval_needs_a_qualified_reviewer_on_the_head_commit() {
        let none = BTreeMap::new();
        let ok = format!("[{}]", review("owner1", "APPROVED", "h", "MEMBER"));
        assert_eq!(approved(&ok, "h", "author", &none), Ok("owner1".to_owned()));
        // On an older commit.
        assert_eq!(
            approved(&ok, "h2", "author", &none),
            Err(ApprovalError::NoApproval)
        );
        // The author's own approval, and an outsider's.
        let own = format!("[{}]", review("author", "APPROVED", "h", "MEMBER"));
        assert_eq!(
            approved(&own, "h", "author", &none),
            Err(ApprovalError::NoApproval)
        );
        let outsider = format!("[{}]", review("x", "APPROVED", "h", "CONTRIBUTOR"));
        assert_eq!(
            approved(&outsider, "h", "author", &none),
            Err(ApprovalError::NoApproval)
        );
        // An outsider by association with write permission counts.
        let mut perms = BTreeMap::new();
        perms.insert("x".to_owned(), "write".to_owned());
        assert_eq!(
            approved(&outsider, "h", "author", &perms),
            Ok("x".to_owned())
        );
        perms.insert("x".to_owned(), "read".to_owned());
        assert!(approved(&outsider, "h", "author", &perms).is_err());
        // A later request for changes overrides an approval; a later comment does not.
        let reversed = format!(
            "[{},{}]",
            review("owner1", "APPROVED", "h", "MEMBER"),
            review("owner1", "CHANGES_REQUESTED", "h", "MEMBER")
        );
        assert!(matches!(
            approved(&reversed, "h", "author", &none),
            Err(ApprovalError::ChangesRequested(_))
        ));
        let commented = format!(
            "[{},{}]",
            review("owner1", "APPROVED", "h", "MEMBER"),
            review("owner1", "COMMENTED", "h", "MEMBER")
        );
        assert!(approved(&commented, "h", "author", &none).is_ok());
        // A dismissed approval does not count.
        let dismissed = format!(
            "[{},{}]",
            review("owner1", "APPROVED", "h", "MEMBER"),
            review("owner1", "DISMISSED", "h", "MEMBER")
        );
        assert!(approved(&dismissed, "h", "author", &none).is_err());
        assert!(matches!(
            approved("not json", "h", "a", &none),
            Err(ApprovalError::Json(_))
        ));
    }
}
