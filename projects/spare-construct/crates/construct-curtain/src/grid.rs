//! How a curtain wall type lays its grid lines out in one direction: none, a fixed spacing with a
//! justification, a fixed count of equal panels, or a most spacing (the fewest equal panels none
//! wider than it). Each is asked for the lines it puts strictly inside an extent (the wall's
//! length for its vertical lines, its height for its horizontal ones); the borders are not grid
//! lines of a layout, they are the curtain wall's own.
//!
//! The rules, the same on every machine:
//!
//! - **Fixed spacing**, justified at the **start**: lines at `s`, `2s`, ... from the start (the
//!   wall's start for vertical lines, its base for horizontal ones); at the **end**: at `s`, `2s`,
//!   ... back from the far border; at the **centre**: a line on the middle and lines every `s`
//!   either side of it, so the two end panels are equal. The middle of an extent of an odd number
//!   of steps is half a step off the grid: the whole pattern is put half a step up, its one
//!   rounding.
//! - **Fixed count** `n`: `n` equal panels, line `k` at the nearest step to `k / n` of the extent
//!   (a half up), each rounded once, so neighbouring panels differ by at most one step (156.25
//!   nm).
//! - **Most spacing** `s`: the fewest equal panels none wider than `s`, laid out as a fixed count.
//! - A line of a fixed spacing nearer a border than the model's tolerance (0.01 mm,
//!   [`TOLERANCE`]) is that border, and is not a line; a fixed count whose panels would be
//!   narrower than the tolerance is refused.

use crate::error::CurtainError;
use crate::length::{self, TOLERANCE, ZERO};
use construct_params::Length;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The most grid lines a curtain wall has in one direction, from its type and its own together.
pub const MOST_LINES: usize = 1_000;

/// The most panels a fixed count asks for.
pub const MOST_COUNT: u32 = 1_000;

/// Which way grid lines run. Vertical lines stand at distances along the wall from its start and
/// divide it into columns; horizontal lines stand at heights above its base and divide it into
/// rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Lines that run up the wall.
    Vertical,
    /// Lines that run along the wall.
    Horizontal,
}

impl Direction {
    /// Both directions, vertical first.
    pub const BOTH: [Direction; 2] = [Direction::Vertical, Direction::Horizontal];

    /// The other direction.
    #[must_use]
    pub fn other(self) -> Direction {
        match self {
            Direction::Vertical => Direction::Horizontal,
            Direction::Horizontal => Direction::Vertical,
        }
    }

    /// Its stored and typed word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Direction::Vertical => "vertical",
            Direction::Horizontal => "horizontal",
        }
    }

    /// The direction a word names.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Direction> {
        Direction::BOTH.into_iter().find(|d| d.word() == word)
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.word())
    }
}

/// Where a fixed spacing is measured from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Justify {
    /// From the start (the wall's start, or its base).
    Start,
    /// From the middle, a line on it.
    Centre,
    /// From the end (the wall's end, or its top).
    End,
}

/// How a type lays its grid lines out in one direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layout {
    /// No grid lines: one panel across.
    None,
    /// A line every `spacing`, measured as `justify` says.
    Spacing {
        /// The distance between lines.
        spacing: Length,
        /// Where it is measured from.
        justify: Justify,
    },
    /// `count` equal panels.
    Count {
        /// How many panels.
        count: u32,
    },
    /// The fewest equal panels none wider than `spacing`.
    Most {
        /// The widest a panel may be.
        spacing: Length,
    },
}

impl Layout {
    /// Whether the layout is one a type may hold: a spacing above the model's tolerance, a count
    /// from 1 to [`MOST_COUNT`].
    ///
    /// # Errors
    /// [`CurtainError::NotPositive`] for a spacing at or below the tolerance;
    /// [`CurtainError::BadCount`] for a count of none or more than [`MOST_COUNT`].
    pub fn check(&self, direction: Direction) -> Result<(), CurtainError> {
        match *self {
            Layout::None => Ok(()),
            Layout::Spacing { spacing, .. } | Layout::Most { spacing } => {
                if spacing <= TOLERANCE {
                    Err(CurtainError::NotPositive {
                        what: format!("{direction} spacing (above 0.01 mm)"),
                        value: spacing,
                    })
                } else {
                    Ok(())
                }
            }
            Layout::Count { count } => {
                if count == 0 || count > MOST_COUNT {
                    Err(CurtainError::BadCount { direction, count })
                } else {
                    Ok(())
                }
            }
        }
    }

    /// The lines this layout puts strictly inside `extent`, more than the tolerance from either
    /// border, in order from the start: as the module header says.
    ///
    /// # Errors
    /// [`Layout::check`]'s refusals; [`CurtainError::TooMany`] for more than [`MOST_LINES`];
    /// [`CurtainError::TooFine`] for equal panels narrower than the tolerance;
    /// [`CurtainError::OutOfRange`] for an extent the arithmetic cannot hold.
    pub fn lines(&self, direction: Direction, extent: Length) -> Result<Vec<Length>, CurtainError> {
        self.check(direction)?;
        if extent <= ZERO {
            return Err(CurtainError::NotPositive {
                what: format!("extent of the {direction} grid"),
                value: extent,
            });
        }
        let high = length::difference(extent, TOLERANCE).ok_or(CurtainError::OutOfRange)?;
        let inside = |p: Length| p > TOLERANCE && p < high;
        match *self {
            Layout::None => Ok(Vec::new()),
            Layout::Spacing { spacing, justify } => {
                let fit = length::whole_times(extent, spacing).ok_or(CurtainError::OutOfRange)?;
                too_many(direction, fit)?;
                let mut out = Vec::new();
                match justify {
                    Justify::Start => {
                        for k in 1..=fit {
                            let p = length::times(spacing, k).ok_or(CurtainError::OutOfRange)?;
                            if inside(p) {
                                out.push(p);
                            }
                        }
                    }
                    Justify::End => {
                        for k in (1..=fit).rev() {
                            let back = length::times(spacing, k).ok_or(CurtainError::OutOfRange)?;
                            let p =
                                length::difference(extent, back).ok_or(CurtainError::OutOfRange)?;
                            if inside(p) {
                                out.push(p);
                            }
                        }
                    }
                    Justify::Centre => {
                        // Twice each line's place: the extent, plus or less twice k spacings.
                        let reach = fit
                            .checked_div(2)
                            .and_then(|r| r.checked_add(1))
                            .ok_or(CurtainError::OutOfRange)?;
                        let start = reach.checked_neg().ok_or(CurtainError::OutOfRange)?;
                        for k in start..=reach {
                            let twice = i128::from(spacing.steps())
                                .checked_mul(i128::from(k))
                                .and_then(|ks| ks.checked_mul(2))
                                .and_then(|ks| ks.checked_add(i128::from(extent.steps())))
                                .ok_or(CurtainError::OutOfRange)?;
                            if twice < 0 {
                                continue;
                            }
                            let p = length::half_up(twice).ok_or(CurtainError::OutOfRange)?;
                            if inside(p) {
                                out.push(p);
                            }
                        }
                    }
                }
                Ok(out)
            }
            Layout::Count { count } => equal(direction, extent, i64::from(count)),
            Layout::Most { spacing } => {
                let fit = length::whole_times(extent, spacing).ok_or(CurtainError::OutOfRange)?;
                let exact = length::times(spacing, fit).is_some_and(|whole| whole == extent);
                let count = if exact {
                    fit
                } else {
                    fit.checked_add(1).ok_or(CurtainError::OutOfRange)?
                };
                too_many(direction, count.saturating_sub(1))?;
                equal(direction, extent, count)
            }
        }
    }
}

/// Refuses more than [`MOST_LINES`] lines.
fn too_many(direction: Direction, count: i64) -> Result<(), CurtainError> {
    let most = u64::try_from(MOST_LINES).unwrap_or(u64::MAX);
    match u64::try_from(count) {
        Ok(count) if count <= most => Ok(()),
        _ => Err(CurtainError::TooMany {
            what: format!("{direction} grid lines"),
            count: u64::try_from(count).unwrap_or(u64::MAX),
            most,
        }),
    }
}

/// The lines of `n` equal divisions of `extent`, each the nearest step.
fn equal(direction: Direction, extent: Length, n: i64) -> Result<Vec<Length>, CurtainError> {
    too_many(direction, n.saturating_sub(1))?;
    let mut out: Vec<Length> = Vec::new();
    let mut before = ZERO;
    for k in 1..n {
        let p = length::share(extent, k, n).ok_or(CurtainError::OutOfRange)?;
        let gap = length::difference(p, before).ok_or(CurtainError::OutOfRange)?;
        if gap <= TOLERANCE {
            return Err(CurtainError::TooFine {
                direction,
                count: n,
            });
        }
        out.push(p);
        before = p;
    }
    let last = length::difference(extent, before).ok_or(CurtainError::OutOfRange)?;
    if n > 1 && last <= TOLERANCE {
        return Err(CurtainError::TooFine {
            direction,
            count: n,
        });
    }
    Ok(out)
}
