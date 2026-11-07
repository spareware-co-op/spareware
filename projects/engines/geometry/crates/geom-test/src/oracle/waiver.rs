//! The night's report, the ratchet against the last one, and the waivers a reviewer can read
//! (chapter 28.10; piece G113).
//!
//! What it does: [`Report::from_night`] folds a night's judged jobs into one [`Report`], a cell for
//! each operation and input family with its jobs, its agreements and what the differences were,
//! settled or not ([`Settled`]). [`ratchet`] holds a night's report to the earlier one: a cell the
//! earlier night covered must still be covered, so a generator that quietly stops drawing a family
//! cannot make a night greener. A [`Violation`] is waived only by an entry of `oracles/waivers.toml`
//! ([`WAIVERS_PATH`], [`Waivers`]), and [`apply`] says what was waived, what was not, and which
//! waivers no longer cover anything.
//!
//! The waiver file is TOML, one `[[waiver]]` table to a waiver, so each carries its reason beside it:
//!
//! ```toml
//! format = 1
//!
//! [[waiver]]
//! oracle = "geomdl"
//! check = "coverage"
//! operation = "nurbs.insert"      # optional; leave out to cover every operation
//! family = "degree-3-2d"          # optional; leave out to cover every family
//! reason = "the generator draws degree-3 curves in 3D only until piece G200 restores 2D"
//! ```
//!
//! Guarantees:
//!
//! - **A waiver can never hide a wrong answer of ours.** The only check there is to waive is
//!   [`Check::Coverage`], a cell that went missing. A difference has no waiver: it is a signature
//!   in the triage register, held to what the exact reference found
//!   ([`super::triage::Register::contradicts`]), and no field of a waiver names a signature, a kind
//!   or a job.
//! - **Nothing a typing mistake widens.** An unknown field, a blank operation or family, a missing
//!   or blank reason, an unknown check, a format that is not 1 and a waiver given twice are each an
//!   error: a misspelt `family` would otherwise read as absent, which covers every family.
//! - **A waiver is a reviewed exception, not a standing one.** [`apply`] lists a waiver that covers
//!   no violation as unused, so one left behind after its cause is gone is seen.
//!
//! Method: chapter 28.10's nightly run and decision 32.6 (`projects/engines/geometry/architecture/`).
//! The report-to-report ratchet and the waiver file follow solidcore's difftest: its regression gate
//! compares a run with the earlier run's reports and waives a violation only by an entry that names
//! the unit, the check and optionally the case, with a reason (`baseline.rs`, `Waivers`; README,
//! "Regression gate"). Here the cell stands where its unit and case stood, and the pass/fail
//! ratchet it also holds is carried by the triage register instead.
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/b4de2149aadbd737268091c51d8390a7d3cdda70/crates/difftest/src/baseline.rs>,
//!   for the ratchet against the earlier report, the waiver's fields and its strict reading, and
//!   the rule that a waiver is removed once it covers nothing. No code copied.
//!
//! Cost: linear in the jobs for a report, in the cells for a ratchet and in the waivers times the
//! violations for [`apply`].

use core::fmt;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::catalogue::Catalogue;
use super::triage::Settled;

/// The waivers' path, relative to the workspace root, one component per entry.
pub const WAIVERS_PATH: [&str; 7] = [
    "projects",
    "engines",
    "geometry",
    "crates",
    "geom-test",
    "oracles",
    "waivers.toml",
];

/// The waivers' text as this crate was built with it.
pub const EMBEDDED: &str = include_str!("../../oracles/waivers.toml");

/// The format of the waivers and of the report this reader reads.
pub const FORMAT: u32 = 1;

/// What a violation or a waiver is of. One check only, on purpose: see the module's guarantees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Check {
    /// A cell the earlier night covered is not covered now.
    Coverage,
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Check::Coverage => f.write_str("coverage"),
        }
    }
}

/// One judged job, as [`Report::from_night`] reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observed {
    /// The job's operation.
    pub operation: String,
    /// The job's input family.
    pub family: String,
    /// `None` if the answers agreed; else the kind of difference and what the reference found.
    pub found: Option<(String, Settled)>,
}

/// One kind of difference in a cell, and how often.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Found {
    /// The kind of difference.
    pub kind: String,
    /// What the exact reference found.
    pub settled: Settled,
    /// How many jobs.
    pub count: u64,
}

/// An operation on an input family: what a night covered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    /// The operation.
    pub operation: String,
    /// The input family.
    pub family: String,
    /// The jobs drawn.
    pub jobs: u64,
    /// The jobs whose answers agreed.
    pub agree: u64,
    /// The differences, by kind and verdict, in order.
    pub differences: Vec<Found>,
}

/// A night's judged jobs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// The format, [`FORMAT`].
    pub format: u32,
    /// The oracle.
    pub oracle: String,
    /// The jobs, in all.
    pub jobs: u64,
    /// Each operation and family, in order.
    pub cells: Vec<Cell>,
}

/// Why a report cannot be read, or a night cannot be held to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReportError {
    /// The text is not a report (serde's message).
    Json(String),
    /// A rule is broken.
    Rule(String),
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReportError::Json(e) => write!(f, "not a night's report: {e}"),
            ReportError::Rule(e) => f.write_str(e),
        }
    }
}

impl core::error::Error for ReportError {}

impl Report {
    /// The report of `observed`, jobs of `oracle`.
    #[must_use]
    pub fn from_night(oracle: &str, observed: impl IntoIterator<Item = Observed>) -> Report {
        type Counts = (u64, u64, BTreeMap<(String, Settled), u64>);
        let mut cells: BTreeMap<(String, String), Counts> = BTreeMap::new();
        let mut jobs = 0_u64;
        for o in observed {
            jobs = jobs.saturating_add(1);
            let cell = cells.entry((o.operation, o.family)).or_default();
            cell.0 = cell.0.saturating_add(1);
            match o.found {
                None => cell.1 = cell.1.saturating_add(1),
                Some(found) => {
                    let count = cell.2.entry(found).or_default();
                    *count = count.saturating_add(1);
                }
            }
        }
        Report {
            format: FORMAT,
            oracle: oracle.to_owned(),
            jobs,
            cells: cells
                .into_iter()
                .map(|((operation, family), (jobs, agree, found))| Cell {
                    operation,
                    family,
                    jobs,
                    agree,
                    differences: found
                        .into_iter()
                        .map(|((kind, settled), count)| Found {
                            kind,
                            settled,
                            count,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    /// Reads a report.
    ///
    /// # Errors
    ///
    /// [`ReportError`]: not a report, another format, or a cell whose counts do not add up.
    pub fn parse(text: &str) -> Result<Report, ReportError> {
        let report: Report =
            serde_json::from_str(text).map_err(|e| ReportError::Json(e.to_string()))?;
        if report.format != FORMAT {
            return Err(ReportError::Rule(format!(
                "report format {}, this reader reads {FORMAT}",
                report.format
            )));
        }
        let mut total = 0_u64;
        for cell in &report.cells {
            let found = cell
                .differences
                .iter()
                .fold(0_u64, |n, f| n.saturating_add(f.count));
            if cell.agree.saturating_add(found) != cell.jobs {
                return Err(ReportError::Rule(format!(
                    "{}/{}: {} agree and {found} differ, of {} jobs",
                    cell.operation, cell.family, cell.agree, cell.jobs
                )));
            }
            total = total.saturating_add(cell.jobs);
        }
        if total != report.jobs {
            return Err(ReportError::Rule(format!(
                "the cells hold {total} jobs, the report says {}",
                report.jobs
            )));
        }
        Ok(report)
    }

    /// The report's one spelling: pretty JSON, ending in one newline.
    ///
    /// # Errors
    ///
    /// [`ReportError::Json`], which only a defect can cause.
    pub fn to_json(&self) -> Result<String, ReportError> {
        let mut text =
            serde_json::to_string_pretty(self).map_err(|e| ReportError::Json(e.to_string()))?;
        text.push('\n');
        Ok(text)
    }
}

/// A way a night is worse than the one it is held to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    /// The oracle.
    pub oracle: String,
    /// The check.
    pub check: Check,
    /// The operation of the cell.
    pub operation: String,
    /// The family of the cell.
    pub family: String,
    /// What is wrong, in words.
    pub detail: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}/{}/{}: {}",
            self.check, self.operation, self.oracle, self.family, self.detail
        )
    }
}

/// Holds `now` to `base`: every cell `base` covered is covered by `now`.
///
/// # Errors
///
/// [`ReportError::Rule`] if the two reports are of different oracles: they are not comparable.
pub fn ratchet(base: &Report, now: &Report) -> Result<Vec<Violation>, ReportError> {
    if base.oracle != now.oracle {
        return Err(ReportError::Rule(format!(
            "the earlier report is of {}, this night of {}",
            base.oracle, now.oracle
        )));
    }
    let covered: BTreeMap<(&str, &str), u64> = now
        .cells
        .iter()
        .map(|c| ((c.operation.as_str(), c.family.as_str()), c.jobs))
        .collect();
    Ok(base
        .cells
        .iter()
        .filter(|c| c.jobs > 0)
        .filter(|c| {
            covered
                .get(&(c.operation.as_str(), c.family.as_str()))
                .is_none_or(|jobs| *jobs == 0)
        })
        .map(|c| Violation {
            oracle: base.oracle.clone(),
            check: Check::Coverage,
            operation: c.operation.clone(),
            family: c.family.clone(),
            detail: format!(
                "the earlier night drew {} jobs here and this one none",
                c.jobs
            ),
        })
        .collect())
}

/// One reviewed exception.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Waiver {
    /// The oracle.
    pub oracle: String,
    /// The check it waives.
    pub check: Check,
    /// The operation it covers; `None` covers every operation.
    pub operation: Option<String>,
    /// The family it covers; `None` covers every family.
    pub family: Option<String>,
    /// Why the violation is acceptable, in words a reviewer can check.
    pub reason: String,
}

impl Waiver {
    /// Whether this waiver covers `v`.
    #[must_use]
    pub fn covers(&self, v: &Violation) -> bool {
        self.oracle == v.oracle
            && self.check == v.check
            && self.operation.as_deref().is_none_or(|o| o == v.operation)
            && self.family.as_deref().is_none_or(|f| f == v.family)
    }
}

/// The waivers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Waivers {
    /// The format, [`FORMAT`].
    pub format: u32,
    /// Every waiver.
    #[serde(default)]
    pub waiver: Vec<Waiver>,
}

/// Why the waivers cannot be read, or which rule they break.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WaiverError {
    /// The text is not the waivers' TOML shape (the parser's message).
    Toml(String),
    /// A rule is broken.
    Rule(String),
}

impl fmt::Display for WaiverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WaiverError::Toml(e) => write!(f, "not the waivers: {e}"),
            WaiverError::Rule(e) => f.write_str(e),
        }
    }
}

impl core::error::Error for WaiverError {}

impl Waivers {
    /// Reads the waivers and checks their rules.
    ///
    /// # Errors
    ///
    /// [`WaiverError`], at the first thing that does not hold.
    pub fn parse(text: &str) -> Result<Waivers, WaiverError> {
        let waivers: Waivers =
            toml::from_str(text).map_err(|e| WaiverError::Toml(e.to_string()))?;
        if waivers.format != FORMAT {
            return Err(WaiverError::Rule(format!(
                "waivers format {}, this reader reads {FORMAT}",
                waivers.format
            )));
        }
        for (i, w) in waivers.waiver.iter().enumerate() {
            let n = i.saturating_add(1);
            if w.oracle.trim().is_empty() {
                return Err(WaiverError::Rule(format!("waiver {n}: it names no oracle")));
            }
            if w.reason.trim().is_empty() {
                return Err(WaiverError::Rule(format!(
                    "waiver {n} ({}): it gives no reason",
                    w.oracle
                )));
            }
            for (what, value) in [("operation", &w.operation), ("family", &w.family)] {
                if value.as_deref().is_some_and(|v| v.trim().is_empty()) {
                    return Err(WaiverError::Rule(format!(
                        "waiver {n} ({}): a blank {what}; leave it out to cover every {what}",
                        w.oracle
                    )));
                }
            }
            let twice = waivers.waiver.iter().take(i).any(|other| {
                other.oracle == w.oracle
                    && other.check == w.check
                    && other.operation == w.operation
                    && other.family == w.family
            });
            if twice {
                return Err(WaiverError::Rule(format!(
                    "waiver {n} ({}): given twice",
                    w.oracle
                )));
            }
        }
        Ok(waivers)
    }

    /// The waivers compiled into this crate.
    ///
    /// # Errors
    ///
    /// As [`Waivers::parse`].
    pub fn embedded() -> Result<Waivers, WaiverError> {
        Waivers::parse(EMBEDDED)
    }

    /// Every waiver for an oracle the catalogue does not hold.
    #[must_use]
    pub fn check_catalogue(&self, catalogue: &Catalogue) -> Vec<String> {
        self.waiver
            .iter()
            .filter(|w| catalogue.get(&w.oracle).is_none())
            .map(|w| {
                format!(
                    "a waiver for {}, which the catalogue does not hold",
                    w.oracle
                )
            })
            .collect()
    }
}

/// What [`apply`] made of a night's violations.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// The violations nothing covers: they fail the night.
    pub unwaived: Vec<Violation>,
    /// The violations a waiver covers, with its reason.
    pub waived: Vec<(Violation, String)>,
    /// The waivers that covered nothing: remove them.
    pub unused: Vec<Waiver>,
}

/// Which of `violations` the `waivers` cover.
#[must_use]
pub fn apply(waivers: &Waivers, violations: Vec<Violation>) -> Applied {
    let mut applied = Applied::default();
    let mut used = vec![false; waivers.waiver.len()];
    for v in violations {
        let mut covering = None;
        for (i, w) in waivers.waiver.iter().enumerate() {
            if w.covers(&v) {
                if let Some(flag) = used.get_mut(i) {
                    *flag = true;
                }
                covering.get_or_insert(w);
            }
        }
        match covering {
            Some(w) => applied.waived.push((v, w.reason.clone())),
            None => applied.unwaived.push(v),
        }
    }
    applied.unused = waivers
        .waiver
        .iter()
        .zip(used)
        .filter(|(_, used)| !used)
        .map(|(w, _)| w.clone())
        .collect();
    applied
}
