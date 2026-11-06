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
//! What the exact reference found ([`Settled`]) is part of every record, and the register's rules
//! hold the outcome to it, so a register entry can never be a waiver for a wrong answer of ours:
//!
//! - "Theirs" stands only on a [`Settled::Theirs`] verdict (the kernel within its bound of the
//!   truth, the oracle not), and carries [`Evidence`]: an address for the upstream report, the
//!   oracle's pin the difference was reproduced on (the catalogue's own, checked by
//!   [`Register::check_catalogue`]) and the exact input that reproduces it. Without all three it is
//!   not recorded: a report nobody can reproduce on the pinned oracle clears nothing.
//! - "Within bounds" stands only on [`Settled::Both`].
//! - "Ours" is a finding, and never rests on a verdict that clears the kernel.
//! - [`Register::contradicts`] holds every later job of a known signature to its record: a job the
//!   exact reference finds ours fails the night whatever the register says, and a job under a
//!   "theirs" or "within bounds" signature that the reference does not settle the same way fails
//!   it too, so a signature triaged once never covers a different answer.
//!
//! Guarantees:
//!
//! - **One record per signature**, and every record names its fixture.
//! - **"Theirs" carries its evidence, "ours" names its fix, "undecided" the question**: none can be
//!   recorded empty.
//! - **A register entry never clears a wrong answer of ours**: [`Register::contradicts`].
//! - **The nightly run fails only on a new signature or a contradiction**, so a known difference
//!   never hides a new one and is never argued twice.
//!
//! Method: chapter 28.10's triage (`projects/engines/geometry/architecture/28-testing.md`). That a
//! difference blamed on the outside tool needs a confirmed upstream report and a reproducing input,
//! and that listing a case never makes it pass, follows solidcore's difftest (its `known.rs` and
//! the regression gate in its `baseline.rs`), re-derived against exact truth: its `cpp_defect`
//! stands on the C++ being "verified wrong", ours on the exact reference settling it.
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/b4de2149aadbd737268091c51d8390a7d3cdda70/crates/difftest/src/known.rs>,
//!   for the evidence a defect blamed on the reference must carry (a confirmed upstream record and
//!   a record that names the case) and for never letting a listed case count as a pass. No code
//!   copied.
//!
//! Cost: linear in the register for each lookup.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::catalogue::Catalogue;
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

/// What the exact reference found on one difference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Settled {
    /// The kernel is not within its bound of the truth: ours, whether or not the oracle is.
    Ours,
    /// The kernel is within its bound of the truth, and the oracle is not.
    Theirs,
    /// Both are within the bound of the truth.
    Both,
    /// The exact reference was not run on it: it settles a point only, and a refusal or a shape has
    /// no value to measure.
    Unsettled,
}

/// What a "theirs" record carries so that nobody has to take the kernel's word, or the oracle's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    /// The address of the upstream report: `https://`, a host and a path.
    pub issue: String,
    /// The oracle's pin the difference was reproduced on; it must be the catalogue's pin.
    pub reproduced_on: String,
    /// The exact input that reproduces it: the job, as the oracle was given it, one JSON object.
    pub reproducer: String,
}

/// One triaged signature.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    /// The signature.
    pub signature: Signature,
    /// The outcome.
    pub outcome: Triage,
    /// What the exact reference found when it was triaged.
    pub settled: Settled,
    /// The fixture it ended as.
    pub fixture: String,
    /// The fix (ours), or what settles it (undecided); not used by the other outcomes.
    pub reference: Option<String>,
    /// What clears a "theirs"; required by it, refused on any other outcome.
    pub evidence: Option<Evidence>,
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

/// Whether `text` is an `https://` address with a host and a path, and no space in it.
#[must_use]
pub fn is_report_address(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("https://") else {
        return false;
    };
    if rest.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let Some((host, path)) = rest.split_once('/') else {
        return false;
    };
    host.contains('.') && !host.starts_with('.') && !host.ends_with('.') && !path.is_empty()
}

impl Evidence {
    fn rules(&self) -> Result<(), String> {
        if !is_report_address(&self.issue) {
            return Err("its upstream report is not an https address with a path".to_owned());
        }
        if self.reproduced_on.trim().is_empty() {
            return Err("it names no pin it was reproduced on".to_owned());
        }
        let is_object = serde_json::from_str::<serde_json::Value>(&self.reproducer)
            .is_ok_and(|v| v.as_object().is_some_and(|o| !o.is_empty()));
        if !is_object {
            return Err("its reproducer is not one input, a JSON object".to_owned());
        }
        Ok(())
    }
}

impl Record {
    fn rules(&self) -> Result<(), String> {
        let s = &self.signature;
        if !crate::fixture::is_id(&self.fixture) {
            return Err(format!("{s}: its fixture is not a fixture's id"));
        }
        if self.why.trim().is_empty() {
            return Err(format!("{s}: it says no reason"));
        }
        let blank = |r: &Option<String>| r.as_deref().is_none_or(|r| r.trim().is_empty());
        match self.outcome {
            Triage::Ours | Triage::Undecided if blank(&self.reference) => {
                return Err(format!(
                    "{s}: {:?} names its report, fix or question",
                    self.outcome
                ));
            }
            _ => {}
        }
        let stands = match self.outcome {
            Triage::Ours => matches!(self.settled, Settled::Ours | Settled::Unsettled),
            Triage::Theirs => self.settled == Settled::Theirs,
            Triage::WithinBounds => self.settled == Settled::Both,
            Triage::Undecided => true,
        };
        if !stands {
            return Err(format!(
                "{s}: {:?} cannot rest on a {:?} verdict of the exact reference",
                self.outcome, self.settled
            ));
        }
        match (&self.evidence, self.outcome) {
            (None, Triage::Theirs) => {
                return Err(format!(
                    "{s}: Theirs names its evidence: the upstream report, the pin it was \
                     reproduced on and the reproducing input"
                ));
            }
            (Some(_), outcome) if outcome != Triage::Theirs => {
                return Err(format!("{s}: only Theirs carries evidence"));
            }
            (Some(evidence), _) => evidence.rules().map_err(|e| format!("{s}: {e}"))?,
            (None, _) => {}
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

    /// Why a difference with `settled` found cannot stand under its signature's record, if it
    /// cannot.
    ///
    /// A job the exact reference finds ours never stands, under any record or none: a register
    /// entry is not a waiver for a wrong answer. A job under a "theirs" or "within bounds" record
    /// stands only if the reference settles it as the record did, so the record covers the
    /// difference it was written for and no other. A job under an "ours" or "undecided" record that
    /// is not found ours stands: the first is a fix whose fixture holds it, the second a question
    /// not yet answered.
    #[must_use]
    pub fn contradicts(&self, signature: &Signature, settled: Settled) -> Option<String> {
        if settled == Settled::Ours {
            return Some(format!(
                "{signature}: the exact reference finds the kernel wrong; no record clears that"
            ));
        }
        let record = self.records.iter().find(|r| r.signature == *signature)?;
        match record.outcome {
            Triage::Theirs | Triage::WithinBounds if record.settled != settled => Some(format!(
                "{signature}: recorded as {:?} on a {:?} verdict, and this job is {settled:?}",
                record.outcome, record.settled
            )),
            _ => None,
        }
    }

    /// Every way a record does not match the catalogue: an oracle it does not hold, and a "theirs"
    /// reproduced on a pin other than the catalogue's own.
    #[must_use]
    pub fn check_catalogue(&self, catalogue: &Catalogue) -> Vec<String> {
        let mut problems = Vec::new();
        for record in &self.records {
            let s = &record.signature;
            let Some(oracle) = catalogue.get(&s.oracle) else {
                problems.push(format!("{s}: the catalogue holds no oracle {}", s.oracle));
                continue;
            };
            let Some(evidence) = &record.evidence else {
                continue;
            };
            match &oracle.pin {
                None => problems.push(format!(
                    "{s}: {} has no pin, so nothing can have been reproduced on it",
                    s.oracle
                )),
                Some(pin) if *pin != evidence.reproduced_on => problems.push(format!(
                    "{s}: reproduced on {}, and the catalogue pins {pin}: reproduce it on the pin",
                    evidence.reproduced_on
                )),
                Some(_) => {}
            }
        }
        problems
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
