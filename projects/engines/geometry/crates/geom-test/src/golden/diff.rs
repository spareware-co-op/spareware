//! The owner-approval rule for edits to the golden store: what a change to the store does, what it
//! may never do, and when a kernel owner must approve it.
//!
//! What it does: [`diff`] compares the store at a pull request's base with the store at its head
//! and returns a [`Diff`]: the goldens added, the goldens changed (and how), and every
//! [`Diff::refusals`] entry, a change no approval can make. [`Diff::needs_approval`] says whether a
//! kernel owner's approval is required. [`approved`] decides, from a pull request's reviews, whether
//! that approval has been given. The binary `golden` runs both in
//! `.github/workflows/kernel-golden.yml` on every pull request and every review.
//!
//! The rules, from chapter 28.13 ("Within an edition, a Stable operation's golden hash never
//! changes. A changed hash is a bug, or an improvement that ships as a new operation version with
//! new goldens beside the old. A pull request that edits a golden says which, and a kernel owner
//! approves it."), 28.16 ("Fixtures are never deleted") and 28.19 ("Never changes a Stable golden
//! hash within an edition"):
//!
//! - **Refused outright:** any change to a frozen edition (its goldens, digests, histories, checks
//!   or state); removing or renaming an edition, or reordering editions; removing a golden;
//!   changing a golden's digest without appending exactly one history record whose digest is the
//!   base's value; rewriting or removing history; and two open editions.
//! - **Needs a kernel owner's approval:** a changed digest (with its record); any other change to an
//!   existing golden (where it is checked, the file it pins, what it covers, its operation
//!   versions, its history); and releasing an edition (the open edition frozen, a new open edition
//!   after it).
//! - **Ordinary review only:** adding goldens, which only adds checks.
//!
//! Who counts as a kernel owner: chapter 32.5 proposes a named group, not yet named, and
//! `.github/CODEOWNERS` asks `@spareware-co-op/owners` to review the store. A workflow's token
//! cannot read a team's members, so [`approved`] counts an approving review on the pull request's
//! current head commit from a reviewer GitHub reports as a member or owner of the organisation, or
//! as having write, maintain or admin permission on the repository, who is not the pull request's
//! author, with no such reviewer's latest review asking for changes. With branch protection's
//! "Require review from Code Owners" (an owner's setting on GitHub) that is exactly the owners
//! team; without it, it is the nearest a workflow can check.
//!
//! Guarantees:
//!
//! - **Every difference is seen.** Goldens are matched by edition, id and operation versions; every
//!   field is compared; a golden present on one side only is an addition or a removal.
//! - **A refusal is final.** [`Diff::is_acceptable`] is false whenever a refusal exists, whatever
//!   the reviews say.
//! - **Approval is per head commit.** A review of an earlier commit does not approve later
//!   changes, so an approved store cannot be changed again without a fresh approval.
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

use super::store::{Edition, Golden, State, Store};

/// A golden's key: edition, id and operation versions.
pub type Key = (String, String, BTreeMap<String, u32>);

/// What a pull request does to the golden store.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    /// Goldens added.
    pub added: Vec<Key>,
    /// Goldens changed, each with what changed in words.
    pub changed: Vec<(Key, Vec<String>)>,
    /// Editions released: frozen here, open at the base.
    pub released: Vec<String>,
    /// Changes no approval can make.
    pub refusals: Vec<String>,
}

impl Diff {
    /// Whether a kernel owner must approve the change.
    #[must_use]
    pub fn needs_approval(&self) -> bool {
        !self.changed.is_empty() || !self.released.is_empty()
    }

    /// Whether the change can merge with the right approval: no refusal.
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
        for (k, what) in &self.changed {
            writeln!(f, "changed: {}: {}", show(k), what.join("; "))?;
        }
        for e in &self.released {
            writeln!(f, "released: edition {e} is frozen")?;
        }
        for r in &self.refusals {
            writeln!(f, "refused: {r}")?;
        }
        if self.added.is_empty()
            && self.changed.is_empty()
            && self.released.is_empty()
            && self.refusals.is_empty()
        {
            writeln!(f, "the golden store is unchanged")?;
        }
        write!(
            f,
            "{}",
            if !self.is_acceptable() {
                "verdict: refused; no approval can merge these changes"
            } else if self.needs_approval() {
                "verdict: a kernel owner's approval is required"
            } else {
                "verdict: no golden is edited; ordinary review"
            }
        )
    }
}

/// What changed in one golden, in words, and whether the change breaks a rule outright.
fn compare(base: &Golden, head: &Golden) -> (Vec<String>, Vec<String>) {
    let mut what = Vec::new();
    let mut refused = Vec::new();
    if base.sha256 != head.sha256 {
        what.push(format!("digest {} -> {}", base.sha256, head.sha256));
        let appended = head.history.len() == base.history.len().saturating_add(1)
            && head.history.starts_with(&base.history);
        let recorded = appended
            && head
                .history
                .last()
                .is_some_and(|record| record.sha256 == base.sha256);
        if !recorded {
            refused.push(
                "its digest changed without exactly one new history record holding the old \
                 digest, its kind (bug, improvement or corpus), its piece and why"
                    .to_owned(),
            );
        }
    } else if base.history != head.history {
        what.push("history".to_owned());
        if head.history.starts_with(&base.history) {
            refused.push(
                "a history record was added but the digest did not change; a record says what a \
                 change replaced"
                    .to_owned(),
            );
        } else {
            refused.push("its history was rewritten; history only grows".to_owned());
        }
    }
    if base.check != head.check {
        what.push(format!("check {:?} -> {:?}", base.check, head.check));
    }
    if base.file != head.file {
        what.push(format!("file {:?} -> {:?}", base.file, head.file));
    }
    if base.what != head.what {
        what.push("what it covers".to_owned());
    }
    (what, refused)
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
            d.released.push(b.edition.clone());
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
                Some(hg) => {
                    let (what, refused) = compare(g, hg);
                    for r in refused {
                        d.refusals.push(format!("{}: {r}", show(&k)));
                    }
                    if !what.is_empty() {
                        d.changed.push((k, what));
                    }
                }
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

/// Why a pull request's reviews do not approve a golden edit.
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
/// repository permission where the workflow could read it.
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

    fn record(sha: &str) -> String {
        format!(r#"{{"sha256": "{sha}", "kind": "improvement", "piece": "G1", "why": "w"}}"#)
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

    #[test]
    fn a_new_store_only_adds() {
        let head = open(&[golden("geom-x/a", A, "")]);
        let d = diff(None, &head);
        assert_eq!(d.added.len(), 1);
        assert!(d.is_acceptable() && !d.needs_approval());
        assert!(d.to_string().contains("ordinary review"));
    }

    #[test]
    fn unchanged_and_added_goldens_need_no_approval() {
        let base = open(&[golden("geom-x/a", A, "")]);
        let head = open(&[golden("geom-x/a", A, ""), golden("geom-x/b", B, "")]);
        let d = diff(Some(&base), &head);
        assert_eq!(d.added.len(), 1);
        assert!(d.changed.is_empty() && d.is_acceptable() && !d.needs_approval());
        assert!(diff(Some(&base), &base).to_string().contains("unchanged"));
    }

    #[test]
    fn a_recorded_change_needs_approval() {
        let base = open(&[golden("geom-x/a", A, "")]);
        let head = open(&[golden("geom-x/a", B, &record(A))]);
        let d = diff(Some(&base), &head);
        assert!(d.is_acceptable(), "{d}");
        assert!(d.needs_approval());
        assert!(d.to_string().contains("approval is required"));
    }

    #[test]
    fn every_refusal_fires() {
        let base = open(&[golden("geom-x/a", B, &record(C))]);
        let cases: Vec<(&str, Store)> = vec![
            (
                "unrecorded change",
                open(&[golden("geom-x/a", A, &record(C))]),
            ),
            (
                "record of the wrong digest",
                open(&[golden(
                    "geom-x/a",
                    A,
                    &format!("{},{}", record(C), record(D)),
                )]),
            ),
            (
                "two records at once",
                open(&[golden(
                    "geom-x/a",
                    A,
                    &format!("{},{},{}", record(C), record(A), record(B)),
                )]),
            ),
            (
                "history rewritten",
                open(&[golden("geom-x/a", B, &record(A))]),
            ),
            ("history dropped", open(&[golden("geom-x/a", B, "")])),
            ("golden removed", open(&[golden("geom-x/z", B, "")])),
        ];
        for (name, head) in cases {
            let d = diff(Some(&base), &head);
            assert!(!d.is_acceptable(), "{name} was accepted:\n{d}");
            assert!(d.to_string().contains("refused"), "{name}");
        }
    }

    #[test]
    fn frozen_editions_never_change_and_editions_never_move() {
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
        assert!(d.is_acceptable(), "{d}");
        assert!(d.needs_approval());
        assert_eq!(d.released, ["pre-edition"]);
    }

    #[test]
    fn moving_a_check_needs_approval() {
        let base = open(&[golden("geom-x/a", A, "")]);
        let head = open(&[golden("geom-x/a", A, "").replace("\"f\"", "\"g\"")]);
        let d = diff(Some(&base), &head);
        assert!(d.is_acceptable() && d.needs_approval(), "{d}");
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
