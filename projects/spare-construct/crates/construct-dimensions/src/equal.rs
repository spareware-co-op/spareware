//! Making segments equal ("EQ"): how far each reference of a linear dimension must move, along
//! the dimension's line, so that the segments of each group are equal.
//!
//! The rule, for each group in turn: its run is from the reference its first segment starts at to
//! the one its last segment ends at. Those two do not move. A segment of the run that is not in
//! the group keeps its length; the group's segments share what is left equally. So a group of a
//! whole string shares the string's overall length; a later group changes only its own segments,
//! and so never undoes an earlier one (the groups share no segment). Where a share is not a whole
//! number of steps, each reference goes to the nearest step of its exact place, and the segments
//! are then equal to within one step (156.25 nm), which [`crate::EqualState::holds`] reports.
//!
//! Each reference is moved by moving its element in the dimension's direction (a wall, a grid
//! line), or along its host (a door, a window, an opening): the caller does that, through the edit
//! log, and refuses by name a reference whose element cannot move so. What is refused here: a
//! reference that cannot be used now, one element at two references that would have to move by
//! two amounts, a run whose ends are at one place, and a share that would leave segments of no
//! length.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "big integers and fractions cannot overflow; every conversion back is checked"
)]

use crate::dimension::{Dimension, Equality, Kind};
use crate::error::DimensionError;
use crate::exact::{Axis, Frac};
use crate::locate::Locator;
use crate::measure::{place, steps};
use kit_core::Id;
use num_bigint::BigInt;
use std::collections::BTreeMap;

/// How far one element moves so that the segments are equal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    /// The element.
    pub element: Id,
    /// The vector it moves by in plan, in steps: along the dimension's direction, each
    /// coordinate the nearest step.
    pub by: [i64; 2],
    /// The references of the dimension it is, from 0.
    pub references: Vec<usize>,
}

fn cannot(why: String) -> DimensionError {
    DimensionError::CannotEqualize(why)
}

fn zero() -> Frac {
    Frac::from_integer(BigInt::ZERO)
}

/// Where every reference meets the offset line now, as a parameter along it.
fn places_now(
    dimension: &Dimension,
    locator: &dyn Locator,
    axis: &Axis,
) -> Result<Vec<Frac>, DimensionError> {
    let mut warnings = Vec::new();
    let mut found = Vec::new();
    for at in 0..dimension.references().len() {
        if let (Some(s), _) = place(dimension, locator, axis, at, &mut warnings) {
            found.push(s);
        } else {
            let words = warnings.pop().map_or_else(
                || format!("reference {} cannot be used", at.saturating_add(1)),
                |warning| warning.words,
            );
            return Err(cannot(words.trim_end_matches('.').to_owned()));
        }
    }
    Ok(found)
}

/// The places after each group's segments share their run equally (the module header).
fn shared_out(groups: &[Equality], before: &[Frac]) -> Result<Vec<Frac>, DimensionError> {
    let mut after = before.to_vec();
    for group in groups {
        let (Some(&first), Some(&last)) = (group.first(), group.last()) else {
            continue;
        };
        let end = last.saturating_add(1);
        let at = |k: usize| after.get(k).cloned().unwrap_or_else(zero);
        let mut kept = zero();
        for segment in first..end {
            if !group.contains(&segment) {
                kept += at(segment.saturating_add(1)) - at(segment);
            }
        }
        let run = at(end) - at(first);
        if run == zero() {
            return Err(cannot(format!(
                "references {} and {}, the ends of segments {} to {}, are at one place",
                first.saturating_add(1),
                end.saturating_add(1),
                first.saturating_add(1),
                last.saturating_add(1)
            )));
        }
        let share = (run - kept) / Frac::from_integer(BigInt::from(group.len()));
        if share == zero() {
            return Err(cannot(format!(
                "the segments not held equal between references {} and {} take its whole length",
                first.saturating_add(1),
                end.saturating_add(1)
            )));
        }
        let mut walked = at(first);
        let mut next = after.clone();
        for segment in first..end {
            walked += if group.contains(&segment) {
                share.clone()
            } else {
                at(segment.saturating_add(1)) - at(segment)
            };
            if let Some(slot) = next.get_mut(segment.saturating_add(1)) {
                *slot = walked.clone();
            }
        }
        after = next;
    }
    Ok(after)
}

/// The moves that take each element from its places `before` to `after`, along `d`.
fn moves_of(
    dimension: &Dimension,
    d: [i64; 2],
    before: &[Frac],
    after: &[Frac],
) -> Result<Vec<Move>, DimensionError> {
    let mut moves: Vec<Move> = Vec::new();
    let mut by_element: BTreeMap<Id, [i64; 2]> = BTreeMap::new();
    for (at, (was, now)) in before.iter().zip(after).enumerate() {
        let shift = now - was;
        let round = |c: i64| -> Result<i64, DimensionError> {
            let along = &shift * Frac::from_integer(BigInt::from(c));
            i64::try_from(&along.round().to_integer()).map_err(|_| {
                cannot(format!(
                    "reference {} would move beyond the kernel's grid",
                    at.saturating_add(1)
                ))
            })
        };
        let [dx, dy] = d;
        let by = [round(dx)?, round(dy)?];
        let Some(reference) = dimension.references().get(at) else {
            continue;
        };
        let element = reference.element();
        match by_element.get(element) {
            Some(other) if *other != by => {
                let first = dimension
                    .references()
                    .iter()
                    .position(|r| r.element() == element)
                    .unwrap_or(at);
                return Err(cannot(format!(
                    "references {} and {} are both of {element}, which would have to move by two \
                     amounts",
                    first.saturating_add(1),
                    at.saturating_add(1)
                )));
            }
            Some(_) => {}
            None => {
                by_element.insert(element.clone(), by);
            }
        }
        if by != [0, 0] {
            match moves.iter_mut().find(|m| m.element == *element) {
                Some(found) => found.references.push(at),
                None => moves.push(Move {
                    element: element.clone(),
                    by,
                    references: vec![at],
                }),
            }
        }
    }
    Ok(moves)
}

/// The moves that make the segments of each group in `groups` equal, for `dimension` as its
/// references are now (`locator`): one per element that moves, in the order of its first
/// reference. Nothing is moved here.
///
/// # Errors
/// [`DimensionError::BadEquality`] for groups [`Dimension::with_equalities`] refuses;
/// [`DimensionError::CannotEqualize`], in words naming the reference, for a reference that
/// cannot be used now, an element that would have to move by two amounts, and a run whose
/// segments would have no length.
pub fn equalize(
    dimension: &Dimension,
    groups: &[Equality],
    locator: &dyn Locator,
) -> Result<Vec<Move>, DimensionError> {
    if dimension.kind() != Kind::Linear {
        return Err(cannot(format!(
            "{} has no string of segments to make equal",
            dimension.kind().words().to_lowercase()
        )));
    }
    // The groups are checked as they would be stored.
    dimension.clone().with_equalities(groups.to_vec())?;
    let (origin, d) = dimension.layout().line();
    let axis = Axis {
        origin: steps(origin),
        d,
    };
    let before = places_now(dimension, locator, &axis)?;
    let after = shared_out(groups, &before)?;
    moves_of(dimension, d, &before, &after)
}
