//! Every refusal this crate makes, as a value the caller reads. A refusal changes nothing. A
//! railing's limits are not refusals: a railing that breaks one is a warning ([`crate::warning`]),
//! so a person can draw it and then fix it. What is refused is what cannot be built: a path that
//! goes nowhere, a type whose parts do not fit, text that does not read.

use construct_params::Length;
use std::fmt;

/// Why a railing, its stored text, a rule set or a railing's layout was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RailingError {
    /// Stored text or a rule set that does not read, in words.
    Malformed(String),
    /// A rule set this program does not hold.
    UnknownRules(String),
    /// A category the rule set does not have.
    UnknownCategory {
        /// The rule set's ID.
        rules: String,
        /// The category asked for.
        category: String,
        /// The categories it has, in its order.
        known: Vec<String>,
    },
    /// An openings rule the category does not take.
    UnknownOpenings {
        /// The category's ID.
        category: String,
        /// The openings rule asked for.
        openings: String,
        /// The openings rules it takes, the first its default.
        allowed: Vec<String>,
    },
    /// A material the default library does not have.
    UnknownMaterial(String),
    /// A length that must be more than nothing is not.
    NotPositive {
        /// What it is: `the top rail's height`.
        what: &'static str,
    },
    /// A length below nothing.
    Negative {
        /// What it is.
        what: &'static str,
    },
    /// A length, a count or a point beyond what this crate holds.
    OutOfRange {
        /// What it is.
        what: String,
    },
    /// A sketched path of fewer than two points.
    FewPoints {
        /// How many it has.
        points: usize,
    },
    /// Two points of a path one after the other at the same place in plan.
    NoLength {
        /// The segment, from 1.
        segment: usize,
    },
    /// A path that turns straight back on itself at a point.
    TurnsBack {
        /// The point, from 1.
        point: usize,
    },
    /// Balusters deeper along the path than the spacing they are laid at.
    Crowded {
        /// The spacing.
        spacing: Length,
        /// How deep each is along the path.
        depth: Length,
    },
    /// Balusters with no height left between their foot and the top rail.
    NoBalusterHeight {
        /// The line, from 1.
        line: usize,
        /// The segment, from 1.
        segment: usize,
    },
    /// A sloped piece of path that does not run along an axis in plan (a stair's runs all do).
    SlopedOffAxis {
        /// The line, from 1.
        line: usize,
        /// The segment, from 1.
        segment: usize,
    },
    /// A railing on both sides of a stair has no other side to flip to.
    BothSides,
    /// A record or recipe that is not a railing's.
    NotARailing(String),
    /// A railing on a stair built with no stair to follow, or a sketched one given one.
    Host(String),
}

impl RailingError {
    /// A stable code for the refusal, as a build failure carries it.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Malformed(_) => "BadRailing",
            Self::UnknownRules(_) | Self::UnknownCategory { .. } | Self::UnknownOpenings { .. } => {
                "UnknownRule"
            }
            Self::UnknownMaterial(_) => "UnknownMaterial",
            Self::NotPositive { .. } | Self::Negative { .. } | Self::Crowded { .. } => {
                "BadRailingType"
            }
            Self::OutOfRange { .. } => "TooBig",
            Self::FewPoints { .. }
            | Self::NoLength { .. }
            | Self::TurnsBack { .. }
            | Self::SlopedOffAxis { .. } => "BadPath",
            Self::NoBalusterHeight { .. } => "NoBalusterHeight",
            Self::BothSides => "BothSides",
            Self::NotARailing(_) => "NotARailing",
            Self::Host(_) => "HostMissing",
        }
    }
}

impl fmt::Display for RailingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(why) | Self::Host(why) => write!(f, "{why}"),
            Self::UnknownRules(id) => write!(f, "There is no railing rule set {id:?}."),
            Self::UnknownCategory {
                rules,
                category,
                known,
            } => write!(
                f,
                "The rule set {rules} has no barrier category {category:?}; it has {}.",
                known.join(", ")
            ),
            Self::UnknownOpenings {
                category,
                openings,
                allowed,
            } => write!(
                f,
                "A barrier of category {category} is checked by the openings rule {}, not {openings:?}.",
                allowed.join(" or ")
            ),
            Self::UnknownMaterial(id) => {
                write!(f, "There is no material {id:?} in the default library.")
            }
            Self::NotPositive { what } => {
                write!(f, "A railing's {what} must be more than nothing.")
            }
            Self::Negative { what } => write!(f, "A railing's {what} cannot be below nothing."),
            Self::OutOfRange { what } => write!(f, "A railing's {what} is beyond what it holds."),
            Self::FewPoints { points } => write!(
                f,
                "A railing's path needs at least two points, not {points}."
            ),
            Self::NoLength { segment } => write!(
                f,
                "Segment {segment} of a railing's path starts and ends at one place."
            ),
            Self::TurnsBack { point } => write!(
                f,
                "A railing's path turns straight back on itself at point {point}."
            ),
            Self::Crowded { spacing, depth } => write!(
                f,
                "Balusters {depth} deep along the path do not fit at a spacing of {spacing}."
            ),
            Self::NoBalusterHeight { line, segment } => write!(
                f,
                "On segment {segment} of line {line}, the balusters' foot is at or above the top rail's underside: they have no height."
            ),
            Self::SlopedOffAxis { line, segment } => write!(
                f,
                "Segment {segment} of line {line} slopes and does not run along an axis in plan."
            ),
            Self::BothSides => write!(
                f,
                "A railing on both sides of a stair has no other side to flip to."
            ),
            Self::NotARailing(what) => write!(f, "{what} is not a railing."),
        }
    }
}

impl std::error::Error for RailingError {}
