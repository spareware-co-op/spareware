//! Triage: every difference ends in one of four outcomes, each a fixture, and each signature is
//! recorded once (chapter 28.10; piece G113).
//!
//! What it does: a [`Signature`] groups differences: operation, oracle, kind of difference, input
//! family. [`Register`] is the record of triaged signatures (`oracles/triaged.json`,
//! [`REGISTER_PATH`]): [`Register::new_signatures`] says which of a night's are new, and only those
//! fail it. [`fixture`] turns a triaged case into the [`Fixture`] 28.16 asks for.
//!
//! The outcomes are [`Triage`]'s four, and each ends differently:
//!
//! - **Ours**: the fixture records the fix, and the fails-on-base check holds it. No fixture can be
//!   written before the fix exists.
//! - **Theirs**: reported upstream with a reproducer, whose address the record holds. The kernel's
//!   result is right, so the fixture keeps it.
//! - **Within bounds**: both results stand. The fixture keeps ours.
//! - **Undecided**: settled by high-precision recomputation; recorded so it is not raised twice, with
//!   the question it waits on, and the fixture keeps what the kernel gives today.
//!
//! Guarantees:
//!
//! - **One record per signature**, and every record names its fixture.
//! - **"Theirs" names its report**, and "ours" its fix: neither can be recorded empty.
//! - **The nightly run fails only on a new signature**, so a known difference never hides a new one
//!   and is never argued twice.
//!
//! Method: chapter 28.10's triage (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: linear in the register for each lookup.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::fixture::run::Replayed;
use crate::fixture::{Fixture, Key, OpLog, Source, Speed, Status, Triage};

/// The register's path, relative to the workspace root, one component per entry.
pub const REGISTER_PATH: [&str; 7] = [
    "projects",
    "engines",
    "geometry",
    "crates",
    "geom-test",
    "oracles",
    "triaged.json",
];

/// The register's text as this crate was built with it.
pub const EMBEDDED: &str = include_str!("../../oracles/triaged.json");

/// The register format this reader reads.
pub const FORMAT: u32 = 1;

/// What groups one difference with another.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signature {
    /// The operation.
    pub operation: String,
    /// The oracle.
    pub oracle: String,
    /// The kind of difference.
    pub kind: String,
    /// The input family.
    pub family: String,
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{}/{}/{}",
            self.operation, self.oracle, self.kind, self.family
        )
    }
}

/// One triaged signature.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    /// The signature.
    pub signature: Signature,
    /// The outcome.
    pub outcome: Triage,
    /// The fixture it ended as.
    pub fixture: String,
    /// The upstream report (theirs), the fix (ours), or what settles it (undecided).
    pub reference: Option<String>,
    /// Why, in a sentence.
    pub why: String,
}

/// The record of triaged signatures.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Register {
    /// The format, [`FORMAT`].
    pub format: u32,
    /// What the file is, for a person reading it.
    pub about: String,
    /// Every record.
    pub records: Vec<Record>,
}

/// Why the register cannot be read, or which rule it breaks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TriageError {
    /// The text is not the register's JSON shape (serde's message).
    Json(String),
    /// A rule is broken.
    Rule(String),
}

impl fmt::Display for TriageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TriageError::Json(e) => write!(f, "not the triage register: {e}"),
            TriageError::Rule(e) => f.write_str(e),
        }
    }
}

impl core::error::Error for TriageError {}

impl Record {
    fn rules(&self) -> Result<(), String> {
        let s = &self.signature;
        if !crate::fixture::is_id(&self.fixture) {
            return Err(format!("{s}: its fixture is not a fixture's id"));
        }
        if self.why.trim().is_empty() {
            return Err(format!("{s}: it says no reason"));
        }
        let needs_reference = !matches!(self.outcome, Triage::WithinBounds);
        if needs_reference
            && self
                .reference
                .as_deref()
                .is_none_or(|r| r.trim().is_empty())
        {
            return Err(format!(
                "{s}: {:?} names its report, fix or question",
                self.outcome
            ));
        }
        Ok(())
    }
}

impl Register {
    /// Reads the register and checks its rules.
    ///
    /// # Errors
    ///
    /// [`TriageError`], at the first thing that does not hold.
    pub fn parse(text: &str) -> Result<Register, TriageError> {
        let register: Register =
            serde_json::from_str(text).map_err(|e| TriageError::Json(e.to_string()))?;
        if register.format != FORMAT {
            return Err(TriageError::Rule(format!(
                "register format {}, this reader reads {FORMAT}",
                register.format
            )));
        }
        let mut seen = BTreeSet::new();
        for record in &register.records {
            record.rules().map_err(TriageError::Rule)?;
            if !seen.insert(&record.signature) {
                return Err(TriageError::Rule(format!(
                    "{}: recorded twice",
                    record.signature
                )));
            }
        }
        Ok(register)
    }

    /// The register compiled into this crate.
    ///
    /// # Errors
    ///
    /// As [`Register::parse`].
    pub fn embedded() -> Result<Register, TriageError> {
        Register::parse(EMBEDDED)
    }

    /// Whether `signature` has been triaged.
    #[must_use]
    pub fn knows(&self, signature: &Signature) -> bool {
        self.records.iter().any(|r| r.signature == *signature)
    }

    /// Of `differences`, the signatures not yet triaged, each once, in order.
    #[must_use]
    pub fn new_signatures<'a>(
        &self,
        differences: impl IntoIterator<Item = &'a Signature>,
    ) -> Vec<Signature> {
        let unique: BTreeSet<&Signature> = differences.into_iter().collect();
        unique
            .into_iter()
            .filter(|s| !self.knows(s))
            .cloned()
            .collect()
    }

    /// The register's one spelling: pretty JSON, ending in one newline.
    ///
    /// # Errors
    ///
    /// [`TriageError::Json`], which only a defect can cause.
    pub fn to_json(&self) -> Result<String, TriageError> {
        let mut text =
            serde_json::to_string_pretty(self).map_err(|e| TriageError::Json(e.to_string()))?;
        text.push('\n');
        Ok(text)
    }
}

/// How many differences each signature has.
#[must_use]
pub fn group<'a>(
    differences: impl IntoIterator<Item = &'a Signature>,
) -> BTreeMap<&'a Signature, usize> {
    let mut groups = BTreeMap::new();
    for signature in differences {
        let count: &mut usize = groups.entry(signature).or_insert(0);
        *count = count.saturating_add(1);
    }
    groups
}

/// The fixture a triaged case ends as: `id`, its `log`, and what the kernel gives today under `key`.
/// A record of "ours" carries its fix in `record.reference`; the run must be the fixed one.
///
/// # Errors
///
/// Why the case cannot be a fixture: a broken run (which no status accepts), or a rule of the
/// fixture.
pub fn fixture(
    record: &Record,
    log: OpLog,
    key: Key,
    replayed: &Replayed,
) -> Result<Fixture, String> {
    if let Replayed::Broken(why) = replayed {
        return Err(format!("the run is broken: {why}"));
    }
    let expect = replayed
        .expectation()
        .ok_or_else(|| "the run holds nothing to store".to_owned())?;
    let fixture = Fixture {
        id: record.fixture.clone(),
        source: Source::Oracle {
            oracle: record.signature.oracle.clone(),
            signature: record.signature.to_string(),
            outcome: record.outcome,
        },
        fixed_by: (record.outcome == Triage::Ours)
            .then(|| record.reference.clone())
            .flatten(),
        speed: Speed::Fast,
        log,
        expect: BTreeMap::from([(key, expect)]),
        status: Status::Pass,
        history: Vec::new(),
    };
    fixture.check().map_err(|e| e.to_string())?;
    Ok(fixture)
}
