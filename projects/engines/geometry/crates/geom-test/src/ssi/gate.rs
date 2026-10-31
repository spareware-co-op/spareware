//! The acceptance gate for surface/surface intersection (chapter 12.17; piece G160).
//!
//! What it does: [`judge`] holds one case's verdict from the scorer, and the answer behind it, to
//! 12.17's rules. [`run`] does so for a whole run and gives a [`Report`], which publishes the flag
//! count, total flagged size and time of every case, the corpus's credit and each case's source.
//! [`Ceilings`] records, per class, the most flagged regions and the largest total size any case in
//! it may have; they may be lowered freely, and raised only with a reason.
//!
//! Guarantees: a case passes only if all of these hold.
//!
//! - **Certify mode**: the verdict is PASS and nothing is flagged.
//! - **Default mode**: no crash, no error, no wrong topology, no inaccuracy, and nothing the scorer
//!   could not judge. INCOMPLETE passes only in a class that may flag, only with nothing spurious
//!   beside it, and only if every missing sample lies inside a flagged region of a matching reason.
//! - **No flags where 12.17 allows none**: the transversal, seam-and-pole, coincident,
//!   patch-boundary and high-degree classes.
//! - **Every flagged region holds a true feature of a matching kind**, and reaches no further from
//!   it than [`allowed`]. A region's reach is how far its furthest corner lies from that feature,
//!   so a long contact comes back as a chain of small boxes, not one box around it.
//! - **The ceilings hold**: the count and total size stay within the class's [`Ceiling`].
//! - **Both readings agree**: [`super::cover`] and the scorer agree on what is missing.
//!
//! [`allowed`] is derived. A box at the floor is one step (12.3). A feature on a shared face can land
//! in either neighbour, so a region at the floor is [`FLOOR_SPAN`] boxes wide on each axis, and its
//! furthest corner lies at most that span's diagonal from the feature, plus how far placing the
//! case moved the surfaces. A tangent contact's band is √(2δ/Δκ) (12.9), with δ the surfaces'
//! movement plus one step, and Δκ the flattest curvature the case's size allows; a contact of higher
//! order takes the same bound at its order ([`super::place::holds_contact`]).
//!
//! Method: chapter 12.17's acceptance gate, 12.3's floor and 12.9's band
//! (`projects/engines/geometry/architecture/12-intersections.md`).
//!
//! Cost: for each case, [`super::cover`]'s cost, and regions × samples.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::case::{Case, Class, Contact, Feature, V3, norm, sub};
use super::cover::{Seen, compare, effective};
use super::output::{Flag, Output, SUCCESS, Why};
use super::place::{Placed, holds_contact, place};
use super::{SsiError, larger};

/// Boxes a region at the floor spans on each axis: one, and its neighbour on either side.
pub const FLOOR_SPAN: f64 = 3.0;

/// The register of ceilings, built in.
pub const CEILINGS: &str = include_str!("../../ssi/ceilings.json");

/// Which of 12.17's two sets of rules a run answers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Everything proven: certify mode (12.14).
    Certify,
    /// The default: flagged regions are part of the answer.
    Default,
}

/// The scorer's verdict on one case, most severe first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// The runner died or ran out of time.
    #[serde(rename = "CRASH")]
    Crash,
    /// The runner reported a failure.
    #[serde(rename = "ERROR")]
    Error,
    /// A piece of the truth is not covered.
    #[serde(rename = "INCOMPLETE")]
    Incomplete,
    /// Spurious output, or the wrong connections.
    #[serde(rename = "WRONG TOPOLOGY")]
    WrongTopology,
    /// The output is too far from a surface.
    #[serde(rename = "INACCURATE")]
    Inaccurate,
    /// All of it right.
    #[serde(rename = "PASS")]
    Pass,
    /// Below the scorer's floor for a run at default tolerance.
    #[serde(rename = "N/A")]
    NotJudged,
}

/// One case's line in the scorer's file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scored {
    /// The case.
    pub id: String,
    /// The verdict.
    pub verdict: Verdict,
    /// Everything it noted.
    #[serde(default)]
    pub flags: Vec<String>,
    /// How long the runner took.
    #[serde(default)]
    pub time_ms: Option<f64>,
}

/// A rule a case broke.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "rule")]
pub enum Failure {
    /// The verdict is not one this mode and class allow.
    Verdict {
        /// The verdict.
        verdict: Verdict,
    },
    /// A case marked incomplete also holds output off the truth.
    Spurious,
    /// Certify mode returned flagged regions.
    FlaggedInCertify {
        /// How many.
        count: usize,
    },
    /// A class that may not flag returned flagged regions.
    FlaggedInClean {
        /// How many.
        count: usize,
    },
    /// A missing feature lies outside every flagged region that could stand for it.
    Uncovered {
        /// The feature's place in [`Case::features`].
        feature: usize,
    },
    /// A flagged region holds no true feature.
    NoFeature {
        /// The region's place in the answer.
        region: usize,
    },
    /// A flagged region holds a feature, but none of a kind its reason can stand for.
    WrongKind {
        /// The region's place in the answer.
        region: usize,
        /// Its reason.
        why: Why,
    },
    /// A flagged region reaches too far from the feature it holds.
    TooWide {
        /// The region's place in the answer.
        region: usize,
        /// How far it reaches, in steps.
        reach: f64,
    },
    /// The flagged regions pass the class's ceiling.
    OverCeiling {
        /// How many there are.
        count: usize,
        /// Their total size, in steps.
        size: u64,
    },
    /// The scorer and [`super::cover`] disagree on what is missing.
    Disagree {
        /// The scorer's verdict.
        scorer: Verdict,
        /// How many features ours found missing.
        missing: usize,
        /// How many spurious pieces ours found.
        spurious: usize,
    },
    /// The answer was made at another scale than the case's placement.
    OffScale {
        /// The scale it claims.
        log2: u32,
    },
}

impl Failure {
    /// The rule's name, as a report and `--must-fail` write it.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Failure::Verdict { .. } => "verdict",
            Failure::Spurious => "spurious",
            Failure::FlaggedInCertify { .. } => "flagged-in-certify",
            Failure::FlaggedInClean { .. } => "flagged-in-clean",
            Failure::Uncovered { .. } => "uncovered",
            Failure::NoFeature { .. } => "no-feature",
            Failure::WrongKind { .. } => "wrong-kind",
            Failure::TooWide { .. } => "too-wide",
            Failure::OverCeiling { .. } => "over-ceiling",
            Failure::Disagree { .. } => "disagree",
            Failure::OffScale { .. } => "off-scale",
        }
    }
}

/// The most a class's cases may flag.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ceiling {
    /// Flagged regions in one case.
    pub count: u64,
    /// Their total size in one case, in steps.
    pub size: u64,
    /// Why it stands where it does.
    pub why: String,
}

/// The ceilings of every class.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ceilings {
    /// By class.
    pub classes: BTreeMap<Class, Ceiling>,
}

impl Ceilings {
    /// Reads ceilings from JSON text, and checks them: every class is there, and a class that may
    /// not flag stands at zero.
    ///
    /// # Errors
    ///
    /// [`SsiError::Malformed`] if not.
    pub fn parse(text: &str) -> Result<Ceilings, SsiError> {
        let malformed = |why: String| SsiError::Malformed {
            what: "the ceilings".into(),
            why,
        };
        let ceilings: Ceilings =
            serde_json::from_str(text).map_err(|e| malformed(e.to_string()))?;
        for class in Class::ALL {
            let ceiling = ceilings
                .classes
                .get(&class)
                .ok_or_else(|| malformed(format!("no ceiling for {class:?}")))?;
            if !class.may_flag() && (ceiling.count != 0 || ceiling.size != 0) {
                return Err(malformed(format!("{class:?} may not flag")));
            }
        }
        Ok(ceilings)
    }

    /// The ceiling of `class`.
    fn of(&self, class: Class) -> Ceiling {
        self.classes.get(&class).cloned().unwrap_or_default()
    }

    /// The ceilings a set of answers needs: the most each class's cases flagged.
    pub fn raise(&mut self, class: Class, count: u64, size: u64, why: &str) {
        let ceiling = self.classes.entry(class).or_default();
        ceiling.count = ceiling.count.max(count);
        ceiling.size = ceiling.size.max(size);
        why.clone_into(&mut ceiling.why);
    }
}

/// `flags`' total size, in whole steps, rounded up.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a sum of diagonals on the grid is positive and far under 2^64"
)]
pub fn total_size(flags: &[Flag]) -> u64 {
    flags.iter().map(Flag::diagonal).sum::<f64>().ceil() as u64
}

/// Whether a region flagged `why` can stand for a feature with contact `contact`.
#[must_use]
pub const fn stands_for(why: Why, contact: Contact) -> bool {
    match why {
        Why::TangentBand => matches!(
            contact,
            Contact::Tangent | Contact::NearTangent | Contact::Isolated
        ),
        Why::HigherOrderContact => matches!(
            contact,
            Contact::Tangent | Contact::Isolated | Contact::Singular
        ),
        Why::NearCoincident => matches!(
            contact,
            Contact::Coincident | Contact::CoincidentBorder | Contact::NearTangent
        ),
        Why::NearMiss => matches!(
            contact,
            Contact::Crossing | Contact::Tangent | Contact::NearTangent | Contact::Isolated
        ),
        Why::NonSmooth => matches!(contact, Contact::Singular),
        Why::KnownCurveUnproven => !matches!(contact, Contact::Isolated | Contact::Singular),
        Why::BoundExceeded | Why::UndecidedAtFloor => true,
    }
}

/// How far a flagged region at the floor may reach from its feature, in steps.
#[must_use]
pub fn floor_reach(placed: &Placed) -> f64 {
    FLOOR_SPAN * 3.0_f64.sqrt() + placed.moved()
}

/// The half-width of an order-two band for `placed`, in steps: √(2δL), δ at least one step.
#[must_use]
pub fn band(placed: &Placed) -> f64 {
    (2.0 * (placed.moved() + 1.0) * placed.extent).sqrt()
}

/// Whether `reach` steps is within what a region flagged `why` may reach in `placed`.
#[must_use]
pub fn allowed(why: Why, reach: f64, placed: &Placed) -> bool {
    if reach <= floor_reach(placed) {
        return true;
    }
    match why {
        Why::TangentBand | Why::HigherOrderContact | Why::NearMiss | Why::NearCoincident => {
            holds_contact(reach, placed.moved() + 1.0, placed.extent, placed.order)
                && reach * reach <= band_ceiling(placed)
        }
        _ => false,
    }
}

/// The square of the widest band a case's order allows: for order two, [`band`] squared.
fn band_ceiling(placed: &Placed) -> f64 {
    let mut width = band(placed);
    if placed.order > 2 {
        // The band of order n is the width where holds_contact turns; find it by doubling.
        while !holds_contact(width, placed.moved() + 1.0, placed.extent, placed.order) {
            width *= 2.0;
        }
    }
    width * width
}

/// Whether the segment `a`–`b` meets the box `lo`–`hi` grown by `margin`, by clipping it against
/// each pair of faces in turn.
fn meets(a: V3, b: V3, lo: V3, hi: V3, margin: f64) -> bool {
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for axis in 0..3 {
        let (low, high) = (lo[axis] - margin, hi[axis] + margin);
        let d = b[axis] - a[axis];
        if d == 0.0 {
            if a[axis] < low || a[axis] > high {
                return false;
            }
            continue;
        }
        let (mut near, mut far) = ((low - a[axis]) / d, (high - a[axis]) / d);
        if near > far {
            core::mem::swap(&mut near, &mut far);
        }
        t0 = larger(t0, near);
        t1 = super::smaller(t1, far);
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// `feature` in steps.
fn in_steps(feature: &Feature<'_>, placed: &Placed) -> Vec<V3> {
    feature.points.iter().map(|p| placed.steps(*p)).collect()
}

/// Whether the polyline `line` meets the box grown by `margin`.
fn holds(flag: &Flag, line: &[V3], margin: f64) -> bool {
    let (lo, hi) = flag.corners();
    match line {
        [only] => meets(*only, *only, lo, hi, margin),
        _ => line.windows(2).any(|w| meets(w[0], w[1], lo, hi, margin)),
    }
}

/// How far the box's furthest corner lies from the polyline `line`, in steps.
fn reach(flag: &Flag, line: &[V3]) -> f64 {
    let (lo, hi) = flag.corners();
    let mut furthest = 0.0;
    for mask in 0..8_u32 {
        let corner = [0_usize, 1, 2].map(|axis| {
            if mask >> axis & 1 == 0 {
                lo[axis]
            } else {
                hi[axis]
            }
        });
        let near = line
            .iter()
            .fold(f64::INFINITY, |d, p| super::smaller(d, norm(sub(corner, *p))));
        furthest = larger(furthest, near);
    }
    furthest
}

/// Everything [`judge`] looks at for one case.
#[derive(Clone, Copy, Debug)]
pub struct Judging<'a> {
    /// The case.
    pub case: &'a Case,
    /// Its placement.
    pub placed: &'a Placed,
    /// The rules it answers to.
    pub mode: Mode,
    /// The scorer's line.
    pub scored: &'a Scored,
    /// The answer.
    pub output: &'a Output,
    /// How close to the surfaces the run had to stay.
    pub accuracy: Option<f64>,
}

/// The rules on one flagged region.
fn region(index: usize, flag: &Flag, j: &Judging<'_>, failures: &mut Vec<Failure>) {
    let margin = j.placed.moved() + 1.0;
    let held: Vec<(Contact, Vec<V3>)> = j
        .case
        .features()
        .iter()
        .map(|f| (f.contact, in_steps(f, j.placed)))
        .filter(|(_, line)| holds(flag, line, margin))
        .collect();
    if held.is_empty() {
        failures.push(Failure::NoFeature { region: index });
        return;
    }
    let matching: Vec<&Vec<V3>> = held
        .iter()
        .filter(|(contact, _)| stands_for(flag.why, *contact))
        .map(|(_, line)| line)
        .collect();
    if matching.is_empty() {
        failures.push(Failure::WrongKind {
            region: index,
            why: flag.why,
        });
        return;
    }
    let least = matching
        .iter()
        .fold(f64::INFINITY, |d, line| super::smaller(d, reach(flag, line)));
    if !allowed(flag.why, least, j.placed) {
        failures.push(Failure::TooWide {
            region: index,
            reach: least,
        });
    }
}

/// Whether every sample `seen` calls missing lies inside a region that can stand for it.
fn uncovered(seen: &Seen, j: &Judging<'_>, failures: &mut Vec<Failure>) {
    let margin = j.placed.moved() + 1.0;
    for missing in &seen.missing {
        let inside = |q: &V3| {
            let q = j.placed.steps(*q);
            j.output
                .flagged
                .iter()
                .any(|f| stands_for(f.why, missing.contact) && holds(f, &[q], margin))
        };
        if !missing.samples.iter().all(inside) {
            failures.push(Failure::Uncovered {
                feature: missing.feature,
            });
        }
    }
}

/// Whether the scorer and [`compare`] read the answer alike.
fn agree(scored: &Scored, seen: &Seen) -> bool {
    match scored.verdict {
        Verdict::Pass => seen.missing.is_empty() && seen.spurious == 0,
        Verdict::Incomplete => !seen.missing.is_empty(),
        _ => true,
    }
}

/// The rules `j` breaks, none if it passes.
///
/// # Errors
///
/// [`SsiError::Malformed`] for a case with a class the corpus does not have.
pub fn judge(j: &Judging<'_>, ceiling: &Ceiling) -> Result<Vec<Failure>, SsiError> {
    let mut failures = Vec::new();
    let class = j.case.class()?;
    let flagged = j.output.flagged.len();
    if j.output.log2 != j.placed.log2 {
        failures.push(Failure::OffScale {
            log2: j.output.log2,
        });
    }
    let seen = compare(j.case, j.output, effective(j.case, j.accuracy));
    if !agree(j.scored, &seen) {
        failures.push(Failure::Disagree {
            scorer: j.scored.verdict,
            missing: seen.missing.len(),
            spurious: seen.spurious,
        });
    }
    let verdict = j.scored.verdict;
    let may_miss = j.mode == Mode::Default && verdict == Verdict::Incomplete && class.may_flag();
    if verdict != Verdict::Pass && !may_miss {
        failures.push(Failure::Verdict { verdict });
    }
    if may_miss {
        let off = |f: &String| {
            f.starts_with("output curve")
                || f.starts_with("output point")
                || f.contains("non-finite")
        };
        if j.scored.flags.iter().any(off) {
            failures.push(Failure::Spurious);
        }
        uncovered(&seen, j, &mut failures);
    }
    if j.mode == Mode::Certify && flagged > 0 {
        failures.push(Failure::FlaggedInCertify { count: flagged });
    }
    if !class.may_flag() && flagged > 0 {
        failures.push(Failure::FlaggedInClean { count: flagged });
    }
    for (index, flag) in j.output.flagged.iter().enumerate() {
        region(index, flag, j, &mut failures);
    }
    let size = total_size(&j.output.flagged);
    if flagged as u64 > ceiling.count || size > ceiling.size {
        failures.push(Failure::OverCeiling {
            count: flagged,
            size,
        });
    }
    Ok(failures)
}

/// One case's line in a report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// The case.
    pub id: String,
    /// Its class.
    pub class: Class,
    /// The scorer's verdict.
    pub verdict: Verdict,
    /// How long the runner took.
    pub time_ms: Option<f64>,
    /// The scale the case was placed at.
    pub log2: u32,
    /// Flagged regions.
    pub flags: u64,
    /// Their total size, in steps.
    pub size: u64,
    /// The answer's digest, none for a runner that wrote none.
    pub digest: Option<String>,
    /// The case's source, where it derives from a paper.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The rules it broke.
    pub failures: Vec<Failure>,
}

/// A run's published result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// What answered.
    pub engine: String,
    /// The rules it answered to.
    pub mode: Mode,
    /// The approximation tolerance of the run.
    pub tolerance: f64,
    /// The credit the corpus's licences require.
    pub credit: String,
    /// One line per case.
    pub rows: Vec<Row>,
}

impl Report {
    /// How many cases broke a rule.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.rows.iter().filter(|r| !r.failures.is_empty()).count()
    }

    /// Whether any case broke the rule called `name`.
    #[must_use]
    pub fn broke(&self, name: &str) -> bool {
        self.rows
            .iter()
            .any(|r| r.failures.iter().any(|f| f.name() == name))
    }

    /// The cases whose answers differ from `other`'s: 12.17's replay, bit for bit.
    #[must_use]
    pub fn differs_from(&self, other: &Report) -> Vec<String> {
        let theirs: BTreeMap<&str, &Option<String>> = other
            .rows
            .iter()
            .map(|r| (r.id.as_str(), &r.digest))
            .collect();
        self.rows
            .iter()
            .filter(|r| theirs.get(r.id.as_str()) != Some(&&r.digest))
            .map(|r| r.id.clone())
            .collect()
    }
}

/// Where a run's files are, and what to hold it to.
#[derive(Clone, Copy, Debug)]
pub struct Run<'a> {
    /// The folder of cases.
    pub cases: &'a Path,
    /// The folder of the runner's answers.
    pub raw: &'a Path,
    /// The scorer's file.
    pub scores: &'a Path,
    /// What answered.
    pub engine: &'a str,
    /// The rules it answers to.
    pub mode: Mode,
    /// The approximation tolerance of the run.
    pub tolerance: f64,
    /// The ceilings.
    pub ceilings: &'a Ceilings,
    /// The credit to carry.
    pub credit: &'a str,
}

/// The file at `path`, as text.
fn text(path: &Path) -> Result<String, SsiError> {
    fs::read_to_string(path).map_err(|e| SsiError::File {
        path: path.display().to_string(),
        why: e.to_string(),
    })
}

/// How close to a surface a run at `tolerance` must stay: the scorer's ten times.
#[must_use]
pub fn accuracy_of(tolerance: f64) -> f64 {
    10.0 * tolerance
}

/// Judges one line of the scorer's file.
fn row(r: &Run<'_>, scored: &Scored) -> Result<Row, SsiError> {
    let case = Case::read(&r.cases.join(format!("{}.json", scored.id)))?;
    let class = case.class()?;
    let accuracy = accuracy_of(r.tolerance);
    let placed = place(&case, Some(accuracy))?;
    let answer = fs::read_to_string(r.raw.join(format!("{}.json", scored.id))).ok();
    let output = answer
        .map(|t| {
            serde_json::from_str::<Output>(&t).map_err(|e| SsiError::Malformed {
                what: format!("{}'s answer", scored.id),
                why: e.to_string(),
            })
        })
        .transpose()?;
    let mut failures = Vec::new();
    match &output {
        Some(output) => {
            let judging = Judging {
                case: &case,
                placed: &placed,
                mode: r.mode,
                scored,
                output,
                accuracy: Some(accuracy),
            };
            failures = judge(&judging, &r.ceilings.of(class))?;
        }
        None => failures.push(Failure::Verdict {
            verdict: scored.verdict,
        }),
    }
    let flags = output.as_ref().map_or(&[][..], |o| &o.flagged);
    Ok(Row {
        id: case.id.clone(),
        class,
        verdict: scored.verdict,
        time_ms: scored.time_ms,
        log2: placed.log2,
        flags: flags.len() as u64,
        size: total_size(flags),
        digest: output.as_ref().filter(|o| o.status == SUCCESS).map(Output::digest),
        source: case.source,
        failures,
    })
}

/// Judges a whole run.
///
/// # Errors
///
/// [`SsiError`] for a file that cannot be read or a case that cannot be placed. A run that breaks
/// a rule is not an error: it is the report.
pub fn run(r: &Run<'_>) -> Result<Report, SsiError> {
    let scored: Vec<Scored> = serde_json::from_str(&text(r.scores)?).map_err(|e| {
        SsiError::Malformed {
            what: r.scores.display().to_string(),
            why: e.to_string(),
        }
    })?;
    let rows = scored
        .iter()
        .map(|s| row(r, s))
        .collect::<Result<Vec<Row>, SsiError>>()?;
    Ok(Report {
        engine: r.engine.to_owned(),
        mode: r.mode,
        tolerance: r.tolerance,
        credit: r.credit.to_owned(),
        rows,
    })
}
