//! Arrays: the motions of each copy of a linear or a radial array, for [`crate::select::Copies`].
//! A count is how many in all, the originals counted, so an array of `n` makes `n - 1` copies;
//! copy `k` is the first copy's motion `k` times over. In this first half each copy is a plain
//! element, with no tie to the others (an associated array, kept together and edited as one, is a
//! later part of CON-A3-F4).
//!
//! - **Linear**, by a spacing (the displacement between neighbours) or a span (the displacement
//!   from the originals to the last copy, split into `n - 1` equal steps: a span that does not
//!   split exactly on the length grid is refused, naming the nearest counts that do).
//! - **Radial**, about a centre, by an angle between neighbours or a sweep from the originals to
//!   the last copy, split the same way into whole angle units. Each step is a turn by an angle, so
//!   it is a whole number of quarter turns ([`crate::exact::Turn::of_angle`]); copies that would
//!   come round onto the originals are refused, naming how many fit.

use crate::error::ModifyError;
use crate::exact::{FULL, Motion, Steps, Turn};
use crate::select::MOST_MADE;
use construct_params::Angle;
use num_integer::Integer;

/// The count checked: at least two, and no more copies than one command makes.
fn copies_of(count: u32) -> Result<u32, ModifyError> {
    if count < 2 {
        return Err(ModifyError::TooFew);
    }
    let copies = count.saturating_sub(1);
    if usize::try_from(copies).map_or(true, |copies| copies > MOST_MADE) {
        return Err(ModifyError::TooMany { most: MOST_MADE });
    }
    Ok(copies)
}

/// Copy `k` of each for `k` from 1 to `copies`: `step` `k` times over.
fn repeated(step: &Motion, copies: u32) -> Result<Vec<Motion>, ModifyError> {
    if step.is_identity() {
        return Err(ModifyError::InPlace);
    }
    let mut motions = Vec::new();
    let mut at = Motion::IDENTITY;
    for _ in 0..copies {
        at = at.then(step)?;
        motions.push(at);
    }
    Ok(motions)
}

/// A linear array of `count` in all, `spacing` apart.
///
/// # Errors
/// [`ModifyError::TooFew`], [`ModifyError::TooMany`], and [`ModifyError::InPlace`] for no spacing.
pub fn linear(spacing: Steps, count: u32) -> Result<Vec<Motion>, ModifyError> {
    let copies = copies_of(count)?;
    repeated(&Motion::by(spacing), copies)
}

/// Whether `span` splits into `steps` equal steps on the grid.
fn splits(span: Steps, steps: u32) -> bool {
    let steps = i64::from(steps);
    span.iter().all(|c| c.checked_rem(steps) == Some(0))
}

/// A linear array of `count` in all, the last `span` from the originals.
///
/// # Errors
/// As [`linear`], and [`ModifyError::Span`] for a span that does not split into `count - 1`
/// equal steps on the grid, naming the nearest counts that do.
pub fn linear_span(span: Steps, count: u32) -> Result<Vec<Motion>, ModifyError> {
    let copies = copies_of(count)?;
    if !splits(span, copies) {
        let most = u32::try_from(MOST_MADE).unwrap_or(u32::MAX);
        let below = (1..copies)
            .rev()
            .find(|steps| splits(span, *steps))
            .map(|steps| steps.saturating_add(1));
        let above = (copies.saturating_add(1)..=most)
            .find(|steps| splits(span, *steps))
            .map(|steps| steps.saturating_add(1));
        return Err(ModifyError::Span {
            count,
            below,
            above,
        });
    }
    let steps = i64::from(copies);
    let [x, y] = span;
    let spacing = [
        x.checked_div(steps).ok_or(ModifyError::TooLarge)?,
        y.checked_div(steps).ok_or(ModifyError::TooLarge)?,
    ];
    repeated(&Motion::by(spacing), copies)
}

/// A radial array of `count` in all about `centre`, turned `angle` apart.
///
/// # Errors
/// As [`linear`]; [`ModifyError::AngleOffGrid`] for an angle that is not a whole number of
/// quarter turns; [`ModifyError::RoundAgain`] when the copies would come round onto the
/// originals.
pub fn radial(centre: Steps, angle: Angle, count: u32) -> Result<Vec<Motion>, ModifyError> {
    let copies = copies_of(count)?;
    let turn = Turn::of_angle(angle)?;
    let units = angle.units().rem_euclid(FULL);
    if units == 0 {
        return Err(ModifyError::InPlace);
    }
    // The fewest steps that come back round: a full turn over what the angle shares with it.
    let round = FULL
        .checked_div(units.gcd(&FULL))
        .ok_or(ModifyError::TooLarge)?;
    if i64::from(count) > round {
        return Err(ModifyError::RoundAgain {
            count,
            most: u32::try_from(round).unwrap_or(u32::MAX),
        });
    }
    repeated(&Motion::turn_about(centre, turn)?, copies)
}

/// A radial array of `count` in all about `centre`, the last turned `sweep` from the originals.
///
/// # Errors
/// As [`radial`], and [`ModifyError::Sweep`] for a sweep that does not split into `count - 1`
/// equal turns of whole angle units.
pub fn radial_sweep(centre: Steps, sweep: Angle, count: u32) -> Result<Vec<Motion>, ModifyError> {
    let copies = copies_of(count)?;
    let steps = i64::from(copies);
    if sweep.units().checked_rem(steps) != Some(0) {
        return Err(ModifyError::Sweep { count });
    }
    let step = sweep
        .units()
        .checked_div(steps)
        .ok_or(ModifyError::TooLarge)?;
    radial(centre, Angle::from_units(step), count)
}
