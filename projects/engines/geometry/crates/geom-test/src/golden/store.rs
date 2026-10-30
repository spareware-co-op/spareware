//! The golden store: every golden digest of the kernel, keyed by kernel edition, golden and
//! operation versions, with the record of every change.
//!
//! What it does: [`Store::parse`] reads the store's JSON text (`golden/store.json` in this crate's
//! folder, [`STORE_PATH`]) into typed records and checks the rules below; [`Store::embedded`]
//! parses the copy compiled into this crate, which every golden test reads, on every target
//! (WebAssembly test runs have no file system); [`Store::expected`] gives the digest a golden must
//! reproduce in the current edition.
//!
//! The store, as chapter 28.13 asks: "Golden hashes are stored per kernel edition and per operation
//! version. Within an edition, a Stable operation's golden hash never changes. A changed hash is a
//! bug, or an improvement that ships as a new operation version with new goldens beside the old. A
//! pull request that edits a golden says which, and a kernel owner approves it." So:
//!
//! - An [`Edition`] is `open` (results may still change, with approval) or `frozen` (released:
//!   nothing in it ever changes). The last edition is the one open edition, the current one; every
//!   earlier one is frozen. Before the first kernel edition there is one open edition,
//!   `pre-edition`: chapter 32.12, recommendation A, has each change approved by a kernel owner in
//!   its pull request, and editions and operation versions start with the first Stable release
//!   (board piece G482). Nothing here decides that recommendation; the store only records which
//!   state each edition is in.
//! - A [`Golden`] is keyed by its `id` and the operation versions it covers (`operations`, empty for
//!   the numbers-layer corpora, which call functions rather than versioned operations; the
//!   operation-log replay corpus of phase 3 fills it). An improvement to a Stable operation in a
//!   released edition is a new key, the new operation version, beside the old one.
//! - Every change to a golden's digest is recorded in its `history`, oldest first: the digest it
//!   replaced, the kind (`bug`, `improvement`, or `corpus` when the inputs or their encoding
//!   changed and no result did), the piece and why. [`super::diff`] refuses a change without its
//!   record, a rewritten history, a removed golden and any change to a frozen edition, and says
//!   when a kernel owner's approval is required; `.github/workflows/kernel-golden.yml` holds the
//!   pull request to that approval.
//! - A golden is checked in one of two ways. A `check` names the test function that computes it
//!   (crate, test target, function), which calls [`super::check`] with the golden's id. A `file`
//!   names a committed golden file that its own harness checks line by line (`geom-bench`'s
//!   `src/golden.txt`, chapter 23.13's output hashes), and its digest is the SHA-256 of the file's
//!   text with `\n` line ends ([`text_digest`]), so every edit to that file is an edit to the store.
//!
//! A pull request that adds a golden or a history record runs this crate's `tests/attack_g19.rs`
//! before it pushes, besides `golden check`: those attacks plant malformed variants of the real
//! store, and a new record that parses can still break one of them, on every test leg at once.
//!
//! Guarantees:
//!
//! - **Nothing ambiguous parses.** Unknown fields, a missing field, a digest that is not 64
//!   lowercase hexadecimal digits, a malformed id or edition label, a golden with both or neither
//!   of `check` and `file`, a `check` in another crate than its id names, two goldens with one key
//!   in an edition, two editions with one label, no open edition, an open edition that is not the
//!   last, a history record whose digest equals the value that replaced it, an unknown kind, an
//!   empty piece or reason: each is a [`StoreError`], never a silent choice.
//! - **One answer per key.** [`Store::expected`] finds the golden by id and operation versions in
//!   the current edition, or says that it is not there.
//! - **Deterministic.** Records keep the file's order; maps are `BTreeMap`s; nothing reads a clock.
//!
//! Method: golden hashes per edition and operation version, chapter 28.13 of the kernel
//! architecture (`projects/engines/geometry/architecture/28-testing.md`), with the editions of
//! 26.2 (`projects/engines/geometry/architecture/26-how-it-grows.md`) and the rule for goldens
//! before the first edition in 32.12 (`projects/engines/geometry/architecture/32-decisions.md`).
//! The records are JSON (RFC 8259, <https://www.rfc-editor.org/rfc/rfc8259>), read with `serde`.
//!
//! Cost: linear in the size of the store for a parse; a lookup scans the current edition's goldens.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use super::sha256::{Digest, sha256};

/// The store's path, relative to the workspace root, one component per entry.
pub const STORE_PATH: [&str; 7] = [
    "projects",
    "engines",
    "geometry",
    "crates",
    "geom-test",
    "golden",
    "store.json",
];

/// The store's text as this crate was built with it.
pub const EMBEDDED: &str = include_str!("../../golden/store.json");

/// The store format this reader reads.
pub const FORMAT: u32 = 1;

/// The label of the edition before the first kernel edition (chapter 32.12).
pub const PRE_EDITION: &str = "pre-edition";

/// The golden store.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Store {
    /// The store format, [`FORMAT`].
    pub format: u32,
    /// What the file is, for a person reading it.
    pub about: String,
    /// Every edition, oldest first; the last is the open one.
    pub editions: Vec<Edition>,
}

/// One kernel edition and its goldens.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edition {
    /// Its label: lowercase letters, digits and hyphens.
    pub edition: String,
    /// Whether its results may still change.
    pub state: State,
    /// Its goldens, in the file's order.
    pub goldens: Vec<Golden>,
}

/// Whether an edition's goldens may still change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// Not released: a golden may change, with its change recorded and a kernel owner's approval.
    Open,
    /// Released: nothing in it ever changes.
    Frozen,
}

/// One golden digest.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Golden {
    /// `<crate>/<name>`: lowercase letters, digits and hyphens on each side of one `/`.
    pub id: String,
    /// What it covers, in a sentence.
    pub what: String,
    /// The test function that computes it, for a golden checked by a test.
    #[serde(default)]
    pub check: Option<TestCheck>,
    /// The committed golden file it pins, relative to the workspace root with `/`, for a golden
    /// checked by its own harness.
    #[serde(default)]
    pub file: Option<String>,
    /// The operation versions it covers: operation name to version. Empty for corpora of
    /// functions.
    #[serde(default)]
    pub operations: BTreeMap<String, u32>,
    /// The digest, 64 lowercase hexadecimal digits.
    pub sha256: String,
    /// Every digest it had before, oldest first, with why each changed.
    #[serde(default)]
    pub history: Vec<Change>,
}

/// Where a test computes a golden.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestCheck {
    /// The crate.
    #[serde(rename = "crate")]
    pub krate: String,
    /// The test target: an integration test's name (the file `tests/<name>.rs`, lowercase letters,
    /// digits, underscores and hyphens), or `lib` for the crate's unit tests.
    pub test: String,
    /// The test function's path inside the target, as libtest names it (`golden_hash_is_...`, or
    /// `grid::tests::sampler_golden` in a library).
    pub function: String,
}

/// One earlier digest of a golden, and why it changed.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    /// The digest that was replaced.
    pub sha256: String,
    /// What kind of change replaced it.
    pub kind: ChangeKind,
    /// The piece (or pull request) that changed it.
    pub piece: String,
    /// Why, and what moved.
    pub why: String,
}

/// The kinds of change chapter 28.13 lets a pull request record, and a corpus change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    /// The old digest recorded a wrong result, now fixed.
    Bug,
    /// The results changed on purpose: more correct, or a new method.
    Improvement,
    /// The inputs or their canonical encoding changed; no result did.
    Corpus,
}

/// Why a store cannot be read, or which rule it breaks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// The text is not the store's JSON shape (serde's message).
    Json(String),
    /// A store of another format.
    Format(u32),
    /// A rule of the store is broken.
    Rule {
        /// Where: the edition and golden, or the store.
        at: String,
        /// What is wrong, and what to write.
        what: String,
    },
    /// The current edition has no golden under this key.
    NotFound {
        /// The current edition's label.
        edition: String,
        /// The id looked for.
        id: String,
        /// The operation versions looked for.
        operations: BTreeMap<String, u32>,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Json(message) => write!(f, "the golden store is not valid: {message}"),
            StoreError::Format(found) => write!(
                f,
                "the golden store is format {found}; this reader reads format {FORMAT}"
            ),
            StoreError::Rule { at, what } => write!(f, "golden store, {at}: {what}"),
            StoreError::NotFound {
                edition,
                id,
                operations,
            } => write!(
                f,
                "the golden store's current edition {edition:?} has no golden {id:?} with \
                 operations {operations:?}; add it to {} (with what it covers and where it is \
                 checked)",
                STORE_PATH.join("/")
            ),
        }
    }
}

impl core::error::Error for StoreError {}

/// Whether `s` is lowercase letters, digits and hyphens, starting and ending with a letter or
/// digit, with no two hyphens in a row.
fn is_slug(s: &str) -> bool {
    let ok_chars = s
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    ok_chars && !s.starts_with('-') && !s.ends_with('-') && !s.contains("--") && !s.is_empty()
}

/// Whether `s` is a test target's name as cargo takes it from `tests/<name>.rs`: lowercase letters,
/// digits, underscores and hyphens, not empty.
fn is_target_name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Whether `s` is a Rust path of identifiers (`a::b::c`), as libtest names a test.
fn is_test_path(s: &str) -> bool {
    !s.is_empty()
        && s.split("::").all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

/// Whether `s` is a relative path with `/` separators, no `.` or `..` part and no empty part.
fn is_relative_path(s: &str) -> bool {
    !s.is_empty()
        && !s.contains('\\')
        && s.split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// The digest of a text file as the store pins it: SHA-256 of its text with every `\r\n` line end
/// read as `\n`, so a checkout that writes `\r\n` (Git's `core.autocrlf` on Windows) and one that
/// writes `\n` give the same digest.
#[must_use]
pub fn text_digest(text: &str) -> Digest {
    Digest(sha256(text.replace("\r\n", "\n").as_bytes()))
}

impl Store {
    /// Reads and checks a store.
    ///
    /// # Errors
    ///
    /// [`StoreError::Json`] for text that is not the store's shape, [`StoreError::Format`] for
    /// another format, and [`StoreError::Rule`] for the first broken rule (the module
    /// documentation lists them).
    pub fn parse(text: &str) -> Result<Store, StoreError> {
        let store: Store =
            serde_json::from_str(text).map_err(|e| StoreError::Json(e.to_string()))?;
        if store.format != FORMAT {
            return Err(StoreError::Format(store.format));
        }
        store.validate()?;
        Ok(store)
    }

    /// The store compiled into this crate.
    ///
    /// # Errors
    ///
    /// As [`Store::parse`].
    pub fn embedded() -> Result<Store, StoreError> {
        Store::parse(EMBEDDED)
    }

    /// The open edition: the last one.
    ///
    /// # Errors
    ///
    /// A [`StoreError::Rule`] if the store has no editions (which [`Store::parse`] refuses).
    pub fn current(&self) -> Result<&Edition, StoreError> {
        self.editions.last().ok_or_else(|| StoreError::Rule {
            at: "the store".to_owned(),
            what: "it has no edition".to_owned(),
        })
    }

    /// The digest the golden `id` with no operation versions must reproduce in the current edition.
    ///
    /// # Errors
    ///
    /// [`StoreError::NotFound`] when the current edition has no such golden.
    pub fn expected(&self, id: &str) -> Result<Digest, StoreError> {
        self.expected_with(id, &BTreeMap::new())
    }

    /// The digest of the golden `id` covering exactly `operations` in the current edition.
    ///
    /// # Errors
    ///
    /// [`StoreError::NotFound`] when the current edition has no such golden.
    pub fn expected_with(
        &self,
        id: &str,
        operations: &BTreeMap<String, u32>,
    ) -> Result<Digest, StoreError> {
        let edition = self.current()?;
        let found = edition
            .goldens
            .iter()
            .find(|g| g.id == id && &g.operations == operations);
        match found {
            Some(g) => Digest::from_hex(&g.sha256).ok_or_else(|| StoreError::Rule {
                at: format!("edition {:?}, golden {id:?}", edition.edition),
                what: "its digest is not 64 hexadecimal digits".to_owned(),
            }),
            None => Err(StoreError::NotFound {
                edition: edition.edition.clone(),
                id: id.to_owned(),
                operations: operations.clone(),
            }),
        }
    }

    /// Checks every rule of the module documentation.
    fn validate(&self) -> Result<(), StoreError> {
        let rule = |at: String, what: String| Err(StoreError::Rule { at, what });
        if self.about.trim().is_empty() {
            return rule("the store".to_owned(), "`about` says nothing".to_owned());
        }
        let Some(last) = self.editions.last() else {
            return rule("the store".to_owned(), "it has no edition".to_owned());
        };
        if last.state != State::Open {
            return rule(
                format!("edition {:?}", last.edition),
                "the last edition must be the open one (a released edition is frozen, and a new \
                 open edition follows it)"
                    .to_owned(),
            );
        }
        let mut labels = BTreeSet::new();
        let count = self.editions.len();
        for (index, edition) in self.editions.iter().enumerate() {
            let at = format!("edition {:?}", edition.edition);
            if !is_slug(&edition.edition) {
                return rule(
                    at,
                    "an edition's label is lowercase letters, digits and hyphens".to_owned(),
                );
            }
            if !labels.insert(edition.edition.as_str()) {
                return rule(at, "two editions have this label".to_owned());
            }
            let is_last = index.checked_add(1) == Some(count);
            if !is_last && edition.state != State::Frozen {
                return rule(
                    at,
                    "only the last edition may be open; every earlier one is frozen".to_owned(),
                );
            }
            edition.validate()?;
        }
        Ok(())
    }
}

impl Edition {
    /// The golden under `id` and `operations`, if this edition has it.
    #[must_use]
    pub fn golden(&self, id: &str, operations: &BTreeMap<String, u32>) -> Option<&Golden> {
        self.goldens
            .iter()
            .find(|g| g.id == id && &g.operations == operations)
    }

    /// Checks this edition's goldens.
    fn validate(&self) -> Result<(), StoreError> {
        let mut keys = BTreeSet::new();
        for golden in &self.goldens {
            let at = format!("edition {:?}, golden {:?}", self.edition, golden.id);
            golden.validate(&at)?;
            if !keys.insert((golden.id.as_str(), &golden.operations)) {
                return Err(StoreError::Rule {
                    at,
                    what: "two goldens in this edition have this id and these operation versions"
                        .to_owned(),
                });
            }
        }
        Ok(())
    }
}

impl Golden {
    /// The crate its id names: the part before the `/`.
    #[must_use]
    pub fn krate(&self) -> &str {
        self.id.split_once('/').map_or("", |(krate, _)| krate)
    }

    /// Checks one golden.
    fn validate(&self, at: &str) -> Result<(), StoreError> {
        let fail = |what: &str| {
            Err(StoreError::Rule {
                at: at.to_owned(),
                what: what.to_owned(),
            })
        };
        let well_formed_id = self
            .id
            .split_once('/')
            .is_some_and(|(krate, name)| is_slug(krate) && is_slug(name));
        if !well_formed_id {
            return fail("an id is `<crate>/<name>`, each lowercase letters, digits and hyphens");
        }
        if self.what.trim().is_empty() {
            return fail("`what` says nothing");
        }
        if !Digest::is_canonical_hex(&self.sha256) {
            return fail("`sha256` must be 64 lowercase hexadecimal digits");
        }
        match (&self.check, &self.file) {
            (Some(check), None) => {
                if check.krate != self.krate() {
                    return fail("its check is in another crate than its id names");
                }
                if !is_target_name(&check.test) {
                    return fail("a check's test is an integration test's name or `lib`");
                }
                if !is_test_path(&check.function) {
                    return fail("a check's function is a test path such as `a::b::name`");
                }
            }
            (None, Some(file)) => {
                if !is_relative_path(file) {
                    return fail("a file is a relative path with `/` and no `.` or `..` part");
                }
            }
            _ => return fail("a golden has exactly one of `check` and `file`"),
        }
        for name in self.operations.keys() {
            if name.trim().is_empty() {
                return fail("an operation's name says nothing");
            }
        }
        let mut next = self.sha256.as_str();
        for change in self.history.iter().rev() {
            if !Digest::is_canonical_hex(&change.sha256) {
                return fail("a history record's `sha256` must be 64 lowercase hexadecimal digits");
            }
            if change.sha256 == next {
                return fail(
                    "a history record's digest equals the digest that replaced it: nothing changed",
                );
            }
            if change.piece.trim().is_empty() || change.why.trim().is_empty() {
                return fail("a history record names its piece and says why");
            }
            next = change.sha256.as_str();
        }
        Ok(())
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

    fn store(goldens: &str) -> String {
        format!(
            r#"{{"format": 1, "about": "test", "editions": [
                {{"edition": "pre-edition", "state": "open", "goldens": [{goldens}]}}]}}"#
        )
    }

    fn golden(id: &str, sha: &str) -> String {
        format!(
            r#"{{"id": "{id}", "what": "w", "sha256": "{sha}",
                "check": {{"crate": "{}", "test": "t", "function": "f"}}}}"#,
            id.split('/').next().unwrap()
        )
    }

    #[test]
    fn the_embedded_store_parses_and_its_current_edition_is_open() {
        let s = Store::embedded().unwrap();
        assert_eq!(s.current().unwrap().state, State::Open);
        assert!(s.current().unwrap().goldens.len() >= 10);
    }

    #[test]
    fn a_good_store_parses_and_answers_lookups() {
        let s = Store::parse(&store(&golden("geom-x/one", A))).unwrap();
        assert_eq!(s.expected("geom-x/one").unwrap().to_string(), A);
        assert!(matches!(
            s.expected("geom-x/two"),
            Err(StoreError::NotFound { .. })
        ));
        let mut ops = BTreeMap::new();
        ops.insert("extrude".to_owned(), 1);
        assert!(matches!(
            s.expected_with("geom-x/one", &ops),
            Err(StoreError::NotFound { .. })
        ));
    }

    #[test]
    fn every_rule_refuses_its_violation() {
        let bad: Vec<(&str, String)> = vec![
            ("unknown field", store(&golden("geom-x/one", A)).replace("\"about\"", "\"extra\": 1, \"about\"")),
            ("format", store(&golden("geom-x/one", A)).replace("\"format\": 1", "\"format\": 2")),
            ("upper-case digest", store(&golden("geom-x/one", &A.to_uppercase()))),
            ("short digest", store(&golden("geom-x/one", &A[1..]))),
            ("id without crate", store(&golden("one", A))),
            ("id with capital", store(&golden("geom-x/One", A))),
            ("two of one key", store(&format!("{},{}", golden("geom-x/one", A), golden("geom-x/one", B)))),
            ("frozen last", store(&golden("geom-x/one", A)).replace("\"open\"", "\"frozen\"")),
            ("bad state", store(&golden("geom-x/one", A)).replace("\"open\"", "\"closed\"")),
            ("no editions", r#"{"format": 1, "about": "x", "editions": []}"#.to_owned()),
            ("empty about", store(&golden("geom-x/one", A)).replace("\"about\": \"test\"", "\"about\": \" \"")),
            (
                "check in another crate",
                store(&golden("geom-x/one", A)).replace("\"crate\": \"geom-x\"", "\"crate\": \"geom-y\""),
            ),
            (
                "both check and file",
                store(&golden("geom-x/one", A)).replace("\"sha256\"", "\"file\": \"a/b.txt\", \"sha256\""),
            ),
            (
                "neither check nor file",
                store(&format!(r#"{{"id": "geom-x/one", "what": "w", "sha256": "{A}"}}"#)),
            ),
            (
                "file with dot-dot",
                store(&format!(r#"{{"id": "geom-x/one", "what": "w", "sha256": "{A}", "file": "a/../b"}}"#)),
            ),
            (
                "history equal to the next value",
                store(&golden("geom-x/one", A)).replace(
                    "\"sha256\": \"aaaa",
                    &format!("\"history\": [{{\"sha256\": \"{A}\", \"kind\": \"bug\", \"piece\": \"G1\", \"why\": \"x\"}}], \"sha256\": \"aaaa"),
                ),
            ),
            (
                "history without a reason",
                store(&golden("geom-x/one", A)).replace(
                    "\"sha256\": \"aaaa",
                    &format!("\"history\": [{{\"sha256\": \"{B}\", \"kind\": \"bug\", \"piece\": \"G1\", \"why\": \"\"}}], \"sha256\": \"aaaa"),
                ),
            ),
            (
                "unknown kind",
                store(&golden("geom-x/one", A)).replace(
                    "\"sha256\": \"aaaa",
                    &format!("\"history\": [{{\"sha256\": \"{B}\", \"kind\": \"tweak\", \"piece\": \"G1\", \"why\": \"x\"}}], \"sha256\": \"aaaa"),
                ),
            ),
            (
                "bad function path",
                store(&golden("geom-x/one", A)).replace("\"function\": \"f\"", "\"function\": \"a b\""),
            ),
        ];
        for (name, text) in bad {
            assert!(Store::parse(&text).is_err(), "{name} passed:\n{text}");
        }
        // Two editions: the earlier must be frozen, and labels unique.
        let two = |first: &str, label: &str| {
            format!(
                r#"{{"format": 1, "about": "t", "editions": [
                {{"edition": "{label}", "state": "{first}", "goldens": []}},
                {{"edition": "pre-edition", "state": "open", "goldens": []}}]}}"#
            )
        };
        assert!(Store::parse(&two("frozen", "one")).is_ok());
        assert!(Store::parse(&two("open", "one")).is_err());
        assert!(Store::parse(&two("frozen", "pre-edition")).is_err());
        assert!(Store::parse(&two("frozen", "One")).is_err());
    }

    #[test]
    fn a_valid_history_parses() {
        let text = store(&golden("geom-x/one", A)).replace(
            "\"sha256\": \"aaaa",
            &format!(
                "\"history\": [{{\"sha256\": \"{B}\", \"kind\": \"improvement\", \"piece\": \"G5\", \
                 \"why\": \"correct rounding\"}}], \"sha256\": \"aaaa"
            ),
        );
        let s = Store::parse(&text).unwrap();
        let g = &s.current().unwrap().goldens[0];
        assert_eq!(g.history[0].kind, ChangeKind::Improvement);
        assert_eq!(g.krate(), "geom-x");
    }

    #[test]
    fn text_digest_reads_crlf_as_lf() {
        assert_eq!(text_digest("a\r\nb\r\n"), text_digest("a\nb\n"));
        assert_ne!(text_digest("a\nb\n"), text_digest("a\nb"));
        assert_eq!(
            text_digest("").to_string(),
            super::super::sha256::hex(sha256(b""))
        );
    }

    #[test]
    fn slugs_and_paths() {
        for good in ["a", "geom-num", "pre-edition", "x1"] {
            assert!(is_slug(good), "{good}");
        }
        for bad in ["", "-a", "a-", "a--b", "A", "a_b", "a/b", "a b"] {
            assert!(!is_slug(bad), "{bad}");
        }
        for good in ["lib", "attack_g25", "rng_determinism", "a-b"] {
            assert!(is_target_name(good), "{good}");
        }
        for bad in ["", "A", "a b", "a/b", "a.rs"] {
            assert!(!is_target_name(bad), "{bad}");
        }
        assert!(is_test_path("grid::tests::golden_one"));
        assert!(is_test_path("_x"));
        for bad in ["", "a::", "::a", "1a", "a-b", "a b"] {
            assert!(!is_test_path(bad), "{bad}");
        }
        assert!(is_relative_path("projects/a/b.txt"));
        for bad in ["", "/a", "a//b", "a/./b", "a/../b", "a\\b"] {
            assert!(!is_relative_path(bad), "{bad}");
        }
    }
}
