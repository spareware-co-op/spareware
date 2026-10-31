//! Fixtures: the stored test cases every failure ends as, and the rules they keep (chapter 28.16,
//! piece G97).
//!
//! What it does: a [`Fixture`] is one permanent test case: its [`Source`], its [`OpLog`] in the
//! native format, one [`Expect`] per kernel edition and set of operation versions, and its
//! [`Status`]. [`Fixture::parse`] reads one from its JSON text and checks every rule below;
//! [`Fixture::to_json`] writes the one spelling that text may have. The submodules hold the rest:
//!
//! - [`log`]: the operation log, written as a standalone `.skgf` file.
//! - [`run`]: replaying a log through an [`run::Engine`] and judging it: Pass, Fail or Improved.
//! - [`kernel`]: the kernel as that engine.
//! - [`minimise`]: removing operations and inputs one at a time while the failure persists.
//! - [`diff`]: what a pull request may do to the fixtures already stored.
//! - [`repo`]: the fixtures folder, its embedded copy, and the fails-on-base list.
//!
//! The types are chapter 28.16's. Where the chapter left a detail open, this is what was decided
//! (also recorded there as "As built"):
//!
//! - A log's hash is taken over every operation's canonical output, in order ([`run`]); a failing
//!   operation ends the log, and its error is the log's `Err`.
//! - [`Expect::Ok`]'s `hash` is required in `expect` and may be left out of `wanted`: nobody can
//!   write the hash of a success the kernel has not produced yet, so a `Known` fixture's wanted
//!   result is usually its measures, and it is judged on what it states.
//! - A fix is recorded in `fixed_by`, beside the source, because any source can lead to one.
//! - [`Speed`] says which tier a fixture runs in, since 28.17 runs fast fixtures on every pull
//!   request and slow ones nightly.
//! - `history` records every change to a stored expectation in an open edition, as the golden
//!   store's does, with the expectation it replaced.
//!
//! Guarantees (each is a [`FixtureError`], never a silent choice):
//!
//! - **Nothing ambiguous parses.** Unknown fields, a missing field, a malformed id, digest, name or
//!   edition label, a log that is not its one canonical encoding, two entries with one key, and
//!   text that is not [`Fixture::to_json`]'s one spelling are all refused.
//! - **Only a typed error can be `Known`.** A `Known` fixture expects `Err` under every key, never
//!   of kind `Internal`, and wants a success. A panic, a hang, an invalid body, a broken law, a
//!   wrong but valid result, or a disagreement with an exact oracle can never be `Known` (28.16).
//! - **Only what replays is stored.** `Cancelled` and `OutOfMemory` depend on the caller and the
//!   machine and are never logged (chapter 22), so no expectation may name them.
//! - **The log and the keys agree.** Every key names exactly the operations the log runs, and the
//!   log's own versions are one of the keys.
//!
//! Method: chapter 28.16's fixtures and statuses
//! (`projects/engines/geometry/architecture/28-testing.md`), which follow `OpenCascade`'s test suite
//! (`projects/kit/notes/G01-evidence/08-standards-coverage-testing.md`, section D). The records are
//! JSON (RFC 8259), read and written with `serde`, as the golden store's are.
//!
//! Cost: linear in the fixture for a parse or a write.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use dashu_int::IBig;
use geom_op::{ErrorKind, Exactness, NameId};
use serde::{Deserialize, Serialize};

use crate::golden::Digest;
use crate::golden::store::ChangeKind;

pub mod diff;
pub mod kernel;
pub mod log;
pub mod minimise;
pub mod repo;
pub mod run;

pub use log::OpLog;

/// The fixture format this reader reads.
pub const FORMAT: u32 = 1;

/// Operation name to version: the set of versions one golden holds for.
pub type OpVersions = BTreeMap<String, u64>;

/// One golden's key: the kernel edition and the operation versions.
pub type Key = (String, OpVersions);

/// A digest, in the store's 64 lowercase hexadecimal digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Hash(pub [u8; 32]);

impl TryFrom<String> for Hash {
    type Error = String;

    fn try_from(text: String) -> Result<Hash, String> {
        if Digest::is_canonical_hex(&text) {
            Digest::from_hex(&text)
                .map(|d| Hash(d.0))
                .ok_or_else(|| format!("{text:?} is not a digest"))
        } else {
            Err(format!("{text:?} is not 64 lowercase hexadecimal digits"))
        }
    }
}

impl From<Hash> for String {
    fn from(hash: Hash) -> String {
        Digest(hash.0).to_string()
    }
}

/// A persistent name, as 32 lowercase hexadecimal digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Name(pub NameId);

impl TryFrom<String> for Name {
    type Error = String;

    fn try_from(text: String) -> Result<Name, String> {
        let mut bytes = [0u8; 16];
        hex::decode_to_slice(&text, &mut bytes)
            .map_err(|e| format!("{text:?} is not a name: {e}"))?;
        let name = Name(NameId::from_bytes(bytes));
        if String::from(name) == text {
            Ok(name)
        } else {
            Err(format!("{text:?} is not in lowercase"))
        }
    }
}

impl From<Name> for String {
    fn from(name: Name) -> String {
        hex::encode(name.0.to_bytes())
    }
}

/// An error's base kind, by chapter 22's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Kind(pub ErrorKind);

impl TryFrom<String> for Kind {
    type Error = String;

    fn try_from(text: String) -> Result<Kind, String> {
        ErrorKind::ALL
            .into_iter()
            .find(|k| k.name() == text)
            .map(Kind)
            .ok_or_else(|| format!("{text:?} is not an error kind"))
    }
}

impl From<Kind> for String {
    fn from(kind: Kind) -> String {
        kind.0.name().to_owned()
    }
}

/// A quantity that may be enclosed rather than exact: integers `lo` and `hi`, written in decimal
/// because a six-times volume overflows every native width. Equal bounds mean an exact value.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quantity {
    /// The lower bound.
    pub lo: String,
    /// The upper bound.
    pub hi: String,
}

impl Quantity {
    /// An exact value.
    #[must_use]
    pub fn exact(value: impl fmt::Display) -> Quantity {
        let text = value.to_string();
        Quantity {
            lo: text.clone(),
            hi: text,
        }
    }

    /// Whether both bounds are canonical decimal integers with `lo <= hi`.
    fn is_well_formed(&self) -> bool {
        let read = |text: &str| {
            IBig::from_str_radix(text, 10)
                .ok()
                .filter(|value| value.to_string() == text)
        };
        matches!((read(&self.lo), read(&self.hi)), (Some(lo), Some(hi)) if lo <= hi)
    }
}

/// How many of each entity a result holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    /// Bodies.
    pub bodies: u64,
    /// Shells.
    pub shells: u64,
    /// Faces.
    pub faces: u64,
    /// Edges.
    pub edges: u64,
    /// Vertices.
    pub vertices: u64,
}

/// An exactness class, as a fixture writes it: a person reads these in a pull request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum Class {
    /// Exact.
    Exact,
    /// Certified within `bound` steps of grid `step`.
    Certified {
        /// The bound, in steps.
        bound: u64,
        /// The grid step's code: `Q0` 0, `Q1` 1, `Q2` 2.
        step: u8,
    },
    /// Approximate, within `deviation` steps of grid `step`.
    Approximate {
        /// The deviation, in steps.
        deviation: u64,
        /// The grid step's code.
        step: u8,
    },
}

impl From<&Exactness> for Class {
    fn from(class: &Exactness) -> Class {
        use geom_num::GridStep;
        let code = |step: &GridStep| match step {
            GridStep::Q0 => 0,
            GridStep::Q1 => 1,
            GridStep::Q2 => 2,
        };
        match class {
            Exactness::Exact => Class::Exact,
            Exactness::Certified { bound, step } => Class::Certified {
                bound: bound.0,
                step: code(step),
            },
            Exactness::Approximate { deviation, step } => Class::Approximate {
                deviation: deviation.0,
                step: code(step),
            },
        }
    }
}

/// The measures chapter 28.16 keeps beside a hash, so a change says what moved: volume, area,
/// counts, box, exactness class and bound.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measures {
    /// Six times the volume, in cubed steps; `None` for a result with no volume.
    pub volume6: Option<Quantity>,
    /// Twice the area, in squared steps; `None` for a result with no area.
    pub area2: Option<Quantity>,
    /// The entity counts.
    pub counts: Counts,
    /// The box, lowest corner then highest, in grid steps; `None` for an empty result.
    #[serde(rename = "box")]
    pub bounds: Option<[[i64; 3]; 2]>,
    /// The exactness class and bound.
    pub exactness: Class,
}

/// What a replay of a fixture's log must give.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
#[allow(
    clippy::large_enum_variant,
    reason = "a fixture holds a few; they read better flat"
)]
pub enum Expect {
    /// A success.
    Ok {
        /// The golden hash ([`run`]); left out only of a `Known` fixture's `wanted`.
        hash: Option<Hash>,
        /// The measures.
        measures: Measures,
    },
    /// A typed error.
    Err {
        /// Its base kind.
        kind: Kind,
        /// The entities it names, in its order.
        entities: Vec<Name>,
    },
}

/// A fixture's status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
#[allow(
    clippy::large_enum_variant,
    reason = "a fixture holds one; it reads better flat"
)]
pub enum Status {
    /// Must match `expect` exactly.
    Pass,
    /// A typed error today.
    Known {
        /// The success we want.
        wanted: Expect,
    },
}

/// The four outcomes of an oracle difference (28.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Triage {
    /// Our bug.
    Ours,
    /// Their bug, reported upstream with a reproducer.
    Theirs,
    /// Both within their stated bounds.
    WithinBounds,
    /// Undecided, to settle by high-precision recomputation.
    Undecided,
}

/// Where a fixture came from.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Source {
    /// A report by a person or an AI.
    Issue {
        /// The issue number.
        issue: u64,
    },
    /// A fuzz crash.
    Fuzz {
        /// The fuzz target.
        target: String,
        /// The seed or crash input's hash.
        seed: String,
    },
    /// A property test's counterexample.
    Property {
        /// The test.
        test: String,
        /// The printed seed.
        seed: String,
    },
    /// A fault the checker missed.
    Checker {
        /// The fault.
        fault: String,
    },
    /// An oracle difference, triaged (28.10).
    Oracle {
        /// The oracle.
        oracle: String,
        /// The difference's signature.
        signature: String,
        /// The outcome.
        outcome: Triage,
    },
    /// An entry of a failure catalogue (28.12).
    Catalogue {
        /// The catalogue.
        catalogue: String,
        /// The entry.
        entry: String,
    },
    /// A case with a closed-form answer.
    ClosedForm {
        /// Where the answer comes from.
        derivation: String,
    },
}

/// Which tier a fixture runs in (28.17).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speed {
    /// Every pull request.
    Fast,
    /// Nightly.
    Slow,
}

/// One change to a stored expectation in an open edition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    /// The edition of the expectation changed.
    pub edition: String,
    /// Its operation versions.
    pub operations: OpVersions,
    /// What it was.
    pub was: Expect,
    /// What kind of change replaced it.
    pub kind: ChangeKind,
    /// The piece or pull request that changed it.
    pub piece: String,
    /// Why, and what moved.
    pub why: String,
}

/// One stored test case (28.16).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fixture {
    /// `<family>/<number>-<slug>`, the file's path under the fixtures folder without `.json`.
    pub id: String,
    /// Where it came from.
    pub source: Source,
    /// The fix in our code it proves, a pull request or commit; `None` when it records no fix.
    pub fixed_by: Option<String>,
    /// Which tier it runs in.
    pub speed: Speed,
    /// The operations and inputs, in the native format.
    pub log: OpLog,
    /// One golden per edition and set of operation versions.
    pub expect: BTreeMap<Key, Expect>,
    /// Its status.
    pub status: Status,
    /// Every change to a stored expectation, oldest first.
    pub history: Vec<Change>,
}

/// One entry of `expect` in the file: JSON has no composite keys.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    edition: String,
    operations: OpVersions,
    expect: Expect,
}

/// A fixture as its JSON file holds it, field for field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    format: u32,
    id: String,
    source: Source,
    fixed_by: Option<String>,
    speed: Speed,
    log: String,
    expect: Vec<Entry>,
    status: Status,
    history: Vec<Change>,
}

/// Why a fixture cannot be read, or which rule it breaks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FixtureError {
    /// The text is not a fixture's JSON shape (serde's message).
    Json(String),
    /// A fixture of another format.
    Format(u32),
    /// The log is not hexadecimal, or not a log.
    Log(String),
    /// A rule is broken.
    Rule {
        /// The fixture.
        id: String,
        /// The rule.
        what: String,
    },
}

impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FixtureError::Json(e) => write!(f, "not a fixture: {e}"),
            FixtureError::Format(n) => write!(f, "fixture format {n}, this reader reads {FORMAT}"),
            FixtureError::Log(e) => write!(f, "not a fixture's log: {e}"),
            FixtureError::Rule { id, what } => write!(f, "fixture {id}: {what}"),
        }
    }
}

impl core::error::Error for FixtureError {}

/// Whether `text` is one or more lowercase letters, digits and hyphens.
fn is_label(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Whether `id` is `<family>/<number>-<slug>`: a label, four or more digits and a label.
#[must_use]
pub fn is_id(id: &str) -> bool {
    let Some((family, case)) = id.split_once('/') else {
        return false;
    };
    let Some((number, slug)) = case.split_once('-') else {
        return false;
    };
    is_label(family)
        && number.len() >= 4
        && number.bytes().all(|b| b.is_ascii_digit())
        && is_label(slug)
}

impl Expect {
    /// The rules every expectation keeps, wherever it sits.
    fn check(&self) -> Result<(), String> {
        match self {
            Expect::Ok { measures, .. } => {
                for quantity in [&measures.volume6, &measures.area2].into_iter().flatten() {
                    if !quantity.is_well_formed() {
                        return Err(format!(
                            "{quantity:?} is not two canonical decimal integers in order"
                        ));
                    }
                }
                if let Some([low, high]) = measures.bounds
                    && low.iter().zip(high).any(|(l, h)| *l > h)
                {
                    return Err(format!("the box {low:?} to {high:?} is inside out"));
                }
                Ok(())
            }
            Expect::Err { kind, .. } => {
                if matches!(kind.0, ErrorKind::Cancelled | ErrorKind::OutOfMemory) {
                    Err(format!(
                        "{} is never logged, so it cannot be expected (chapter 22)",
                        kind.0.name()
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }
}

impl Fixture {
    /// Reads a fixture from its JSON text, and checks every rule of the module header.
    ///
    /// # Errors
    ///
    /// [`FixtureError`], at the first thing that does not hold.
    pub fn parse(text: &str) -> Result<Fixture, FixtureError> {
        let file: File =
            serde_json::from_str(text).map_err(|e| FixtureError::Json(e.to_string()))?;
        if file.format != FORMAT {
            return Err(FixtureError::Format(file.format));
        }
        let bytes = hex::decode(&file.log).map_err(|e| FixtureError::Log(e.to_string()))?;
        let log = OpLog::decode(&bytes).map_err(|e| FixtureError::Log(e.to_string()))?;
        let mut expect = BTreeMap::new();
        for entry in file.expect {
            let key = (entry.edition, entry.operations);
            if expect.insert(key.clone(), entry.expect).is_some() {
                return Err(FixtureError::Rule {
                    id: file.id,
                    what: format!("two expectations for {key:?}"),
                });
            }
        }
        let fixture = Fixture {
            id: file.id,
            source: file.source,
            fixed_by: file.fixed_by,
            speed: file.speed,
            log,
            expect,
            status: file.status,
            history: file.history,
        };
        fixture.check()?;
        if fixture.to_json()? != text {
            return Err(FixtureError::Rule {
                id: fixture.id,
                what: "the text is not the fixture's one spelling, which `to_json` writes"
                    .to_owned(),
            });
        }
        Ok(fixture)
    }

    /// The fixture's one spelling: pretty JSON in field order, ending in one newline.
    ///
    /// # Errors
    ///
    /// [`FixtureError::Log`] for a log with no encoding, and [`FixtureError::Json`] if `serde_json`
    /// refuses, which only a defect can cause.
    pub fn to_json(&self) -> Result<String, FixtureError> {
        let log = self
            .log
            .encode()
            .map_err(|e| FixtureError::Log(e.to_string()))?;
        let file = File {
            format: FORMAT,
            id: self.id.clone(),
            source: self.source.clone(),
            fixed_by: self.fixed_by.clone(),
            speed: self.speed,
            log: hex::encode(log),
            expect: self
                .expect
                .iter()
                .map(|((edition, operations), expect)| Entry {
                    edition: edition.clone(),
                    operations: operations.clone(),
                    expect: expect.clone(),
                })
                .collect(),
            status: self.status.clone(),
            history: self.history.clone(),
        };
        let mut text =
            serde_json::to_string_pretty(&file).map_err(|e| FixtureError::Json(e.to_string()))?;
        text.push('\n');
        Ok(text)
    }

    /// Every rule of the module header.
    ///
    /// # Errors
    ///
    /// [`FixtureError::Rule`] naming the first rule broken.
    pub fn check(&self) -> Result<(), FixtureError> {
        self.rules().map_err(|what| FixtureError::Rule {
            id: self.id.clone(),
            what,
        })
    }

    fn rules(&self) -> Result<(), String> {
        if !is_id(&self.id) {
            return Err("the id is not <family>/<number>-<slug>".to_owned());
        }
        if self.expect.is_empty() {
            return Err("no expectation".to_owned());
        }
        if self.log.operations.is_empty() {
            return Err("the log runs nothing".to_owned());
        }
        let ran: BTreeSet<&str> = self
            .log
            .operations
            .iter()
            .map(|o| o.name.as_str())
            .collect();
        for ((edition, operations), expect) in &self.expect {
            if !is_label(edition) {
                return Err(format!("the edition {edition:?} is malformed"));
            }
            let named: BTreeSet<&str> = operations.keys().map(String::as_str).collect();
            if named != ran {
                return Err(format!(
                    "the key {operations:?} does not name the operations the log runs, {ran:?}"
                ));
            }
            expect.check()?;
            if matches!(expect, Expect::Ok { hash: None, .. }) {
                return Err("an expectation without a hash".to_owned());
            }
        }
        let own = self.log.versions();
        if !self.expect.keys().any(|(_, operations)| *operations == own) {
            return Err(format!("no expectation is for the log's versions, {own:?}"));
        }
        if let Status::Known { wanted } = &self.status {
            if self.fixed_by.is_some() {
                return Err("a Known fixture records no fix: it still fails".to_owned());
            }
            if !matches!(wanted, Expect::Ok { .. }) {
                return Err("`wanted` must be a success".to_owned());
            }
            wanted.check()?;
            for expect in self.expect.values() {
                match expect {
                    Expect::Err { kind, .. } if kind.0 != ErrorKind::Internal => {}
                    Expect::Err { .. } => {
                        return Err(
                            "an Internal error is a defect and can never be Known".to_owned()
                        );
                    }
                    Expect::Ok { .. } => {
                        return Err("only a typed error can be Known (28.16)".to_owned());
                    }
                }
            }
        }
        Ok(())
    }

    /// Whether the fails-on-base check holds this fixture: its source records a fix in our code,
    /// and it is neither `Known` nor an oracle's "their bug" or "both within bounds" (28.16).
    #[must_use]
    pub fn must_fail_on_base(&self) -> bool {
        self.fixed_by.is_some()
            && self.status == Status::Pass
            && !matches!(
                self.source,
                Source::Oracle {
                    outcome: Triage::Theirs | Triage::WithinBounds,
                    ..
                }
            )
    }

    /// The keys of `edition`, in key order.
    pub fn keys_of<'a>(
        &'a self,
        edition: &'a str,
    ) -> impl Iterator<Item = (&'a OpVersions, &'a Expect)> {
        self.expect
            .iter()
            .filter(move |((e, _), _)| e == edition)
            .map(|((_, operations), expect)| (operations, expect))
    }
}
