//! What a found room measures, exactly, and what becomes of a room: enclosed with its measures,
//! not enclosed, or redundant beside an earlier room in the same loop.
//!
//! - Area: the shoelace sum over the loop's grid corners, in square steps, the holes taken away.
//!   Every corner is a grid point, so twice the area is whole; an odd twice-area is refused
//!   ([`RoomError::AreaNotExact`]), never halved with a remainder lost, unless a corner where a
//!   line met a face was rounded to the grid ([`Found::rounded`]): the area is then rounded once,
//!   a half up.
//! - Perimeter: the outer loop's and every hole's edges, each to the nearest grid step (the one
//!   rounding here, as a floor's perimeter is rounded; exact for every edge along an axis and for
//!   every edge whose length is whole steps). A person reads it in their units, rounded only then.
//! - Volume: the area times the room's height, from its base to its upper limit, in cubic steps,
//!   exactly.
//!
//! An area (job 14.6) measures the first two only ([`Flat`], [`measure_flat`]): it has no height.
//! A room's volume here is its area times its height; what floors in the space take from it is
//! [`crate::volume`]'s, read from the document's floors by the layer that has them.
//!
//! Which room keeps a loop two rooms share: the one whose ID comes first. The other is
//! [`Outcome::Redundant`], naming it, and takes the loop back if the first goes.

use crate::error::RoomError;
use crate::plane::{Bound, Corner, Found, find, twice_area_of};
use construct_params::{Area, Length, Volume};
use kit_core::Id;

/// What a found room measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Measures {
    /// The floor area inside its boundary, holes taken away, in square steps.
    pub area: Area,
    /// The length of its boundary, holes included, each edge to the nearest step.
    pub perimeter: Length,
    /// Its area times its height, in cubic steps.
    pub volume: Volume,
}

/// What a found area (a flat region with no height) measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flat {
    /// The area inside its boundary, holes taken away, in square steps.
    pub area: Area,
    /// The length of its boundary, holes included, each edge to the nearest step.
    pub perimeter: Length,
}

/// The nearest whole number to the square root of `n` (never a tie, since `n` is whole).
fn nearest_root(n: u128) -> u128 {
    let root = n.isqrt();
    // (root + 1/2)^2 = root^2 + root + 1/4, so n rounds up when n > root^2 + root.
    let edge = root.saturating_mul(root).saturating_add(root);
    if n > edge {
        root.saturating_add(1)
    } else {
        root
    }
}

fn loop_length(corners: &[Corner]) -> Result<i64, RoomError> {
    let mut total = 0_i64;
    for (k, a) in corners.iter().enumerate() {
        let Some(b) = corners.get(k.saturating_add(1)).or_else(|| corners.first()) else {
            continue;
        };
        // Coordinates are within 2^40 steps, so each difference squared is under 2^84.
        let dx = i128::from(b.at[0]).abs_diff(i128::from(a.at[0]));
        let dy = i128::from(b.at[1]).abs_diff(i128::from(a.at[1]));
        let squared = dx
            .checked_mul(dx)
            .and_then(|x| dy.checked_mul(dy).and_then(|y| x.checked_add(y)))
            .ok_or(RoomError::TooBig)?;
        let edge = i64::try_from(nearest_root(squared)).map_err(|_| RoomError::TooBig)?;
        total = total.checked_add(edge).ok_or(RoomError::TooBig)?;
    }
    Ok(total)
}

/// What `found` measures, standing `height` from its base to its upper limit.
///
/// # Errors
/// [`RoomError::NoHeight`] for a height at or below zero; [`RoomError::AreaNotExact`] for an odd
/// twice-area; [`RoomError::TooBig`] when a sum or product does not fit.
pub fn measure(found: &Found, height: Length) -> Result<Measures, RoomError> {
    if height.steps() <= 0 {
        return Err(RoomError::NoHeight);
    }
    let flat = measure_flat(found)?;
    let volume = flat.area.checked_mul(height).ok_or(RoomError::TooBig)?;
    Ok(Measures {
        area: flat.area,
        perimeter: flat.perimeter,
        volume,
    })
}

/// What `found` measures in plan: its area and its perimeter.
///
/// # Errors
/// [`RoomError::AreaNotExact`] for an odd twice-area; [`RoomError::TooBig`] when a sum does not
/// fit.
pub fn measure_flat(found: &Found) -> Result<Flat, RoomError> {
    let mut twice = twice_area_of(&found.outer)?;
    let mut perimeter = loop_length(&found.outer)?;
    for hole in &found.holes {
        // Each hole runs clockwise, so its twice-area is below zero and is taken away.
        twice = twice
            .checked_add(twice_area_of(hole)?)
            .ok_or(RoomError::TooBig)?;
        perimeter = perimeter
            .checked_add(loop_length(hole)?)
            .ok_or(RoomError::TooBig)?;
    }
    let odd = twice.checked_rem(2) != Some(0);
    if odd && !found.rounded {
        return Err(RoomError::AreaNotExact);
    }
    // A corner a line's meeting rounded to the grid is the one place an area is rounded, once, a
    // half up.
    let twice = if odd {
        twice.checked_add(1).ok_or(RoomError::TooBig)?
    } else {
        twice
    };
    let area = Area::from_square_steps(twice.checked_div(2).ok_or(RoomError::TooBig)?);
    Ok(Flat {
        area,
        perimeter: Length::from_steps(perimeter),
    })
}

/// Another room on the same level, as the search reads it: its ID and its point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Neighbour {
    /// Its permanent ID.
    pub id: Id,
    /// Its point, in grid steps.
    pub at: [i64; 2],
}

/// What becomes of a room (or, with [`Flat`] measures, an area).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome<M = Measures> {
    /// Enclosed: its loop and what it measures.
    Enclosed {
        /// Its loop.
        found: Found,
        /// What it measures.
        measures: M,
    },
    /// The open space round its point is not closed by walls. The record stays, with no area.
    NotEnclosed,
    /// An earlier room (by ID) is in the same loop. The record stays, with no area.
    Redundant {
        /// The room that keeps the loop.
        first: Id,
    },
}

impl<M> Outcome<M> {
    /// The word a page, a script or an AI reads it by: `enclosed`, `not enclosed`, `redundant`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Self::Enclosed { .. } => "enclosed",
            Self::NotEnclosed => "not enclosed",
            Self::Redundant { .. } => "redundant",
        }
    }
}

/// What becomes of the room `id` at the point `at`, `height` high, among the walls `bounds` of its
/// level and the other rooms `others` of its level: its loop ([`find`]), then whether an earlier
/// room is in it, then its measures.
///
/// # Errors
/// [`find`]'s refusals and [`measure`]'s.
pub fn assess(
    id: &Id,
    at: [i64; 2],
    height: Length,
    bounds: &[Bound],
    others: &[Neighbour],
) -> Result<Outcome, RoomError> {
    assess_with(id, at, bounds, others, |found| measure(found, height))
}

/// What becomes of the area `id` at the point `at` among the area boundary lines `bounds` of its
/// level and scheme and the other areas `others` of that level and scheme: as [`assess`], with no
/// height.
///
/// # Errors
/// [`find`]'s refusals and [`measure_flat`]'s.
pub fn assess_flat(
    id: &Id,
    at: [i64; 2],
    bounds: &[Bound],
    others: &[Neighbour],
) -> Result<Outcome<Flat>, RoomError> {
    assess_with(id, at, bounds, others, measure_flat)
}

fn assess_with<M>(
    id: &Id,
    at: [i64; 2],
    bounds: &[Bound],
    others: &[Neighbour],
    measure_it: impl FnOnce(&Found) -> Result<M, RoomError>,
) -> Result<Outcome<M>, RoomError> {
    let Some(found) = find(bounds, at)? else {
        return Ok(Outcome::NotEnclosed);
    };
    if let Some(first) = others
        .iter()
        .filter(|other| other.id < *id && found.holds(other.at))
        .map(|other| other.id.clone())
        .min()
    {
        return Ok(Outcome::Redundant { first });
    }
    let measures = measure_it(&found)?;
    Ok(Outcome::Enclosed { found, measures })
}
