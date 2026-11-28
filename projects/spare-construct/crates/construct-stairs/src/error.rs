//! Every refusal this crate makes, as a value the caller reads. A refusal changes nothing. One
//! that a rule makes names the limit, the value that broke it and the clause of the rule set it
//! comes from; one about a run or a landing names it by its number from the bottom, from 1.

use construct_params::Length;
use std::fmt;

/// One limit of a rule set as a refusal names it: what it limits, its value and its clause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    /// What it limits, in plain words: `the most a riser may rise`.
    pub what: String,
    /// Its value, as a length.
    pub value: Length,
    /// The clause of the rule set it comes from: `D1/AS1 4.1.1 and Table 6`.
    pub clause: String,
}

impl fmt::Display for Named {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is {} ({})", self.what, self.value, self.clause)
    }
}

/// Why a stair, its stored text, a rule set or a stair's layout was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StairError {
    /// A stair has no run.
    NoRuns,
    /// A stair has more runs than [`crate::MOST_RUNS`].
    TooManyRuns {
        /// How many it was given.
        runs: usize,
    },
    /// The landings are not one fewer than the runs: one landing goes between each two runs.
    LandingCount {
        /// How many runs.
        runs: usize,
        /// How many landings.
        landings: usize,
    },
    /// A run is given fewer than two risers: a run of one riser has no tread.
    FewRisers {
        /// The run, from 1 at the bottom.
        run: usize,
        /// How many it was given.
        risers: u32,
    },
    /// A length that must be more than nothing is not.
    NotPositive {
        /// What it is: `the width`.
        what: &'static str,
    },
    /// A length or a point is beyond what this crate holds (a length over [`crate::MOST_LENGTH`],
    /// a point beyond the kernel's grid, a count of risers over [`crate::MOST_RISERS`], or a sum
    /// that does not fit).
    OutOfRange {
        /// What it is.
        what: String,
    },
    /// A landing that turns is shorter than the stair is wide, so the run after it has no room
    /// beside it.
    TurnTooShort {
        /// The landing, from 1 at the bottom.
        landing: usize,
        /// Its length.
        length: Length,
        /// The stair's width.
        width: Length,
    },
    /// The stair names a rule set this program does not hold.
    UnknownRules(String),
    /// The stair names a category its rule set does not have.
    UnknownCategory {
        /// The rule set.
        rules: String,
        /// The category it named.
        category: String,
        /// The categories the rule set has.
        known: Vec<String>,
    },
    /// The stair's top is not above its base.
    NotRising {
        /// The height from its base to its top.
        rise: Length,
    },
    /// The going is shorter than its category's least.
    GoingTooShort {
        /// The going.
        going: Length,
        /// The category's name.
        category: String,
        /// The least going.
        least: Named,
    },
    /// The going is longer than the rule set's most.
    GoingTooLong {
        /// The going.
        going: Length,
        /// The most going.
        most: Named,
    },
    /// The stair is narrower than its category's least width.
    TooNarrow {
        /// The width.
        width: Length,
        /// The category's name.
        category: String,
        /// The least width.
        least: Named,
    },
    /// A landing is shorter than the rule set's least landing.
    LandingTooShort {
        /// The landing, from 1 at the bottom.
        landing: usize,
        /// Its length.
        length: Length,
        /// The least landing length.
        least: Named,
    },
    /// No count of risers lays the rise out within every limit ([`NoCount`] says which limits
    /// and the nearest heights that work).
    NoRiserCount(Box<NoCount>),
    /// A run rises further than its category allows between landings.
    RunRisesTooFar {
        /// The run, from 1 at the bottom.
        run: usize,
        /// How far it rises.
        rise: Length,
        /// The category's name.
        category: String,
        /// The most rise between landings.
        most: Named,
    },
    /// The risers of a flight would differ by more than the rule set allows. The risers a stair
    /// is laid out with differ by one grid step at most, so this answers only a rule set that
    /// asks for less.
    NotUniform {
        /// The rise from base to top.
        rise: Length,
        /// How many risers.
        risers: usize,
        /// The most two risers of a flight may differ.
        within: Named,
    },
    /// The waist is too thin for the run's underside to pass below every step.
    WaistTooThin {
        /// The run, from 1 at the bottom.
        run: usize,
        /// The waist.
        waist: Length,
    },
    /// One piece of the stair passes over another with less than the headroom the rule set asks
    /// for above the lower piece's pitch line (or its landing).
    Headroom {
        /// The lower piece: `run 1`, `landing 2`.
        lower: String,
        /// The upper piece.
        upper: String,
        /// The least clearance between them, rounded down to a grid step; below nothing where the
        /// two meet.
        clearance: Length,
        /// The least headroom.
        least: Named,
    },
    /// Stored text, or a rule set's text, that is not exactly as written.
    Malformed(String),
    /// A record or a recipe that is not a stair's.
    NotAStair(String),
}

/// No count of risers lays a rise out within every limit: the riser must lie between `lowest`
/// and `highest`, and no count of risers (that the runs allow) gives such risers. The nearest
/// floor-to-floor heights that would work with the same settings are named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoCount {
    /// The rise from base to top.
    pub rise: Length,
    /// The category's name.
    pub category: String,
    /// The lowest riser the limits allow: the rule set's least riser, or the riser at its least
    /// pitch on this going, whichever is higher.
    pub lowest: Named,
    /// The highest riser the limits allow: the category's most riser, or the riser at its most
    /// pitch on this going, whichever is lower.
    pub highest: Named,
    /// The going the risers were tried with.
    pub going: Length,
    /// How many risers the runs fix (a stair whose every run gives its count), or the least they
    /// allow.
    pub risers: RiserCount,
    /// The tallest floor-to-floor height below this one that works, if any.
    pub below: Option<Length>,
    /// The lowest floor-to-floor height above this one that works, if any.
    pub above: Option<Length>,
}

/// How many risers the runs allow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RiserCount {
    /// Exactly this many: every run gives its count.
    Exactly(usize),
    /// At least this many: some runs take what the others leave, two each at least.
    AtLeast(usize),
}

impl fmt::Display for RiserCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exactly(n) => write!(f, "exactly {n} risers, as its runs give them"),
            Self::AtLeast(n) => write!(f, "at least {n} risers, two to each run"),
        }
    }
}

/// `"a" and "b"`, `"a"`, or nothing, for the nearest heights.
fn heights(below: Option<Length>, above: Option<Length>) -> String {
    match (below, above) {
        (Some(b), Some(a)) => {
            format!("; the nearest floor-to-floor heights that work are {b} and {a}")
        }
        (Some(h), None) | (None, Some(h)) => {
            format!("; the nearest floor-to-floor height that works is {h}")
        }
        (None, None) => "; no floor-to-floor height works with this going".to_owned(),
    }
}

impl fmt::Display for StairError {
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per refusal, each its own words"
    )]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRuns => write!(f, "A stair needs at least one run."),
            Self::TooManyRuns { runs } => write!(
                f,
                "A stair takes at most {} runs, and this one has {runs}.",
                crate::MOST_RUNS
            ),
            Self::LandingCount { runs, landings } => write!(
                f,
                "A stair of {runs} runs has {} landings, one between each two runs, not {landings}.",
                runs.saturating_sub(1)
            ),
            Self::FewRisers { run, risers } => write!(
                f,
                "Run {run} is given {risers} riser(s): a run takes at least two, so that it has a tread."
            ),
            Self::NotPositive { what } => write!(f, "{} must be more than nothing.", capital(what)),
            Self::OutOfRange { what } => {
                write!(f, "{} is beyond what a stair holds.", capital(what))
            }
            Self::TurnTooShort {
                landing,
                length,
                width,
            } => write!(
                f,
                "Landing {landing} turns, and is {length} long: a landing that turns is at least as long as the stair is wide ({width}), so the next run fits beside it."
            ),
            Self::UnknownRules(rules) => write!(f, "No rule set is named {rules:?}."),
            Self::UnknownCategory {
                rules,
                category,
                known,
            } => write!(
                f,
                "The rule set {rules} has no stairway category {category:?}; it has {}.",
                known.join(", ")
            ),
            Self::NotRising { rise } => write!(
                f,
                "A stair rises from its base to its top, and this one's top is {rise} from its base."
            ),
            Self::GoingTooShort {
                going,
                category,
                least,
            } => write!(
                f,
                "A going of {going} is too short for a {}: {least}.",
                lower(category)
            ),
            Self::GoingTooLong { going, most } => {
                write!(f, "A going of {going} is too long: {most}.")
            }
            Self::TooNarrow {
                width,
                category,
                least,
            } => write!(
                f,
                "A width of {width} is too narrow for a {}: {least}.",
                lower(category)
            ),
            Self::LandingTooShort {
                landing,
                length,
                least,
            } => write!(f, "Landing {landing} is {length} long: {least}."),
            Self::NoRiserCount(no) => write!(
                f,
                "No count of risers lays out a rise of {} on a {} with a going of {} and {}: {}, and {}{}.",
                no.rise,
                lower(&no.category),
                no.going,
                no.risers,
                no.lowest,
                no.highest,
                heights(no.below, no.above)
            ),
            Self::RunRisesTooFar {
                run,
                rise,
                category,
                most,
            } => write!(
                f,
                "Run {run} rises {rise} on a {}: {most}. Add a landing.",
                lower(category)
            ),
            Self::NotUniform {
                rise,
                risers,
                within,
            } => write!(
                f,
                "A rise of {rise} in {risers} risers leaves risers one grid step apart: {within}."
            ),
            Self::WaistTooThin { run, waist } => write!(
                f,
                "A waist of {waist} is too thin for the underside of run {run} to pass below every step."
            ),
            Self::Headroom {
                lower,
                upper,
                clearance,
                least,
            } => write!(
                f,
                "{} passes {clearance} above {lower}'s pitch line: {least}.",
                capital(upper)
            ),
            Self::Malformed(why) => write!(f, "{why}"),
            Self::NotAStair(why) => write!(f, "That is not a stair: {why}."),
        }
    }
}

impl std::error::Error for StairError {}

/// Text with its first letter a capital.
fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// A category's name as it reads inside a sentence: `common stairway`.
fn lower(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}
