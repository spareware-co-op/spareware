//! Both checkers on one body: `geom-check`'s full check and the second checker of chapter 28.4
//! ([`crate::second_check`]), and whether they agree (Gate 4, G156: "the second checker agrees on
//! the whole corpus").
//!
//! What it does: [`judge`] runs both on a body and sorts what each finds into one list of
//! [`Rule`]s, the rules of chapter 8.3 both read: structure, three planes that do not meet, a
//! corner off its plane or its line, a corner outside its face's box, crossing edges, crossing
//! faces, a loop's sense, a shell's sense and a shell's nesting. Two checkers agree when they find
//! the same rules broken: both pass, or both refuse with matching findings. What a checker says it
//! cannot judge (`geom-check`'s `Unchecked` and `PairUnchecked`, the second checker's
//! [`Finding::Unjudged`]) is counted beside the verdict, never read as a pass or a fault, so a
//! corpus can state how much of it the second checker did not read.
//!
//! Guarantees:
//!
//! - **One map.** Every fault kind of `geom-check` and every finding of the second checker has
//!   exactly one rule or is counted as not judged ([`rule_of_kind`], [`rule_of_finding`]); a test
//!   holds every kind to it.
//! - **Nothing is dropped.** [`Verdict::agree`] compares the whole sets of rules, so a fault one
//!   checker finds and the other misses is a disagreement, whichever finds it.
//!
//! Method: chapter 28.4's second checker standing behind the first
//! (`projects/engines/geometry/architecture/28-testing.md`), with chapter 8.3's rules as the
//! common ground (`projects/engines/geometry/architecture/08-topology.md`).
//!
//! Cost: both checkers' costs: the full check's, and the second checker's product of the faces'
//! edges for each pair of faces.

use std::collections::BTreeSet;

use geom_check::{FaultKind, Level, check};
use geom_num::Exactness;
use geom_run::{Budget, Stopped, WorkMeter};
use geom_topo::Body;

use crate::second_check::{self, Finding};

/// A rule of chapter 8.3 that both checkers read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    /// The lists and rings: loops close nose to tail, each edge of a solid has two opposite fins,
    /// every entity in its place.
    Structure,
    /// A vertex of three planes that do not meet in one point.
    Singular,
    /// A corner off its face's plane, or an edge off its face.
    OffPlane,
    /// A vertex off its edge's line.
    OffLine,
    /// A corner outside its face's stored box.
    OutOfBox,
    /// Two edges of a face meeting away from a vertex they share.
    EdgesCross,
    /// Two faces meeting away from anything they share.
    FacesCross,
    /// A face without one loop counter-clockwise from outside and the rest clockwise.
    LoopSense,
    /// A solid region without exactly one shell facing outward.
    ShellSense,
    /// A shell where its region does not allow it.
    Nesting,
}

/// The rule a fault kind of `geom-check` breaks, `None` for a kind that says only that no
/// certified check exists yet, or that the body's class does not fit it.
#[must_use]
pub const fn rule_of_kind(kind: FaultKind) -> Option<Rule> {
    match kind {
        FaultKind::Unchecked
        | FaultKind::PairUnchecked
        | FaultKind::NotFlat
        | FaultKind::NotStraight
        | FaultKind::CurvedPcurve => None,
        FaultKind::MeetSingular => Some(Rule::Singular),
        FaultKind::VertexOffSurface | FaultKind::EdgeOffSurface => Some(Rule::OffPlane),
        FaultKind::VertexOffCurve => Some(Rule::OffLine),
        FaultKind::FaceBound => Some(Rule::OutOfBox),
        FaultKind::EdgesCross => Some(Rule::EdgesCross),
        FaultKind::FacesCross => Some(Rule::FacesCross),
        FaultKind::LoopSense => Some(Rule::LoopSense),
        FaultKind::ShellInward | FaultKind::ShellOutward => Some(Rule::ShellSense),
        FaultKind::ShellNesting => Some(Rule::Nesting),
        _ => Some(Rule::Structure),
    }
}

/// The rule a finding of the second checker breaks, `None` for [`Finding::Unjudged`].
#[must_use]
pub const fn rule_of_finding(finding: Finding) -> Option<Rule> {
    match finding {
        Finding::Unjudged => None,
        Finding::Structure => Some(Rule::Structure),
        Finding::Singular(_) => Some(Rule::Singular),
        Finding::OffPlane(_) => Some(Rule::OffPlane),
        Finding::OffLine(_) => Some(Rule::OffLine),
        Finding::OutOfBox(_) => Some(Rule::OutOfBox),
        Finding::EdgesCross(_) => Some(Rule::EdgesCross),
        Finding::FacesCross(..) => Some(Rule::FacesCross),
        Finding::LoopSense(_) => Some(Rule::LoopSense),
        Finding::ShellSense => Some(Rule::ShellSense),
        Finding::Nesting(_) => Some(Rule::Nesting),
    }
}

/// What both checkers say of one body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    /// The rules `geom-check` finds broken.
    pub first: BTreeSet<Rule>,
    /// The rules the second checker finds broken.
    pub second: BTreeSet<Rule>,
    /// How many faults `geom-check` reports it has no certified check for.
    pub unchecked: usize,
    /// How many faces and edges the second checker says it cannot judge ([`Finding::Unjudged`]).
    pub unjudged: usize,
}

impl Verdict {
    /// Whether the checkers agree: both pass, or both refuse with the same rules broken.
    #[must_use]
    pub fn agree(&self) -> bool {
        self.first == self.second
    }

    /// Whether both pass.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.first.is_empty() && self.second.is_empty()
    }
}

/// Both checkers on `body`, recorded at `exactness`.
///
/// # Errors
///
/// A stop of the full check's meter; an unlimited one never stops.
pub fn judge(body: &Body, exactness: Exactness) -> Result<Verdict, Stopped> {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let report = check(body, exactness, Level::Full, &mut meter)?;
    let kinds: Vec<FaultKind> = report.faults().iter().map(|f| f.kind).collect();
    let findings = second_check::check(body, exactness);
    Ok(Verdict {
        first: kinds.iter().filter_map(|k| rule_of_kind(*k)).collect(),
        second: findings
            .iter()
            .filter_map(|f| rule_of_finding(*f))
            .collect(),
        unchecked: kinds.iter().filter(|k| rule_of_kind(**k).is_none()).count(),
        unjudged: findings
            .iter()
            .filter(|f| matches!(f, Finding::Unjudged))
            .count(),
    })
}
