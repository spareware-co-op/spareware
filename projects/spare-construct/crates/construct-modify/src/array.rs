//! Arrays: the motions of each copy of a linear or a radial array, for [`crate::select::Copies`].
//! A count is how many in all, the originals counted, so an array of `n` makes `n - 1` copies. In
//! this first half each copy is a plain element, with no tie to the others (an associated array,
//! kept together and edited as one, is a later part of CON-A3-F4).
//!
//! Each copy's motion is worked out from the originals, never from the copy before it, so each
//! copy's points are rounded once and nothing builds up along the array:
//! - **Linear**, by a spacing (the displacement between neighbours: copy `k` is `k` spacings away,
//!   exactly), or a span (the displacement from the originals to the last copy: copy `k` is the
//!   nearest grid step to `k / (n - 1)` of it, each axis a half away from zero, so neighbours are
//!   equal or one step apart and the last is the span exactly).
//! - **Radial**, about a centre, by an angle between neighbours (copy `k` is the turn by `k`
//!   times it) or a sweep from the originals to the last copy (copy `k` is the turn by
//!   `k / (n - 1)` of it; a sweep of a full turn either way spreads all `n` evenly round, the last
//!   step closing the circle). Each is a turn by any angle ([`crate::exact::Turn::of_fraction`]),
//!   exact for a quarter turn, so a copy that comes to a quarter turn lands exactly however small
//!   the steps; copies that would come round onto the originals are refused, naming how many fit.

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

/// A linear array of `count` in all, `spacing` apart.
///
/// # Errors
/// [`ModifyError::TooFew`], [`ModifyError::TooMany`], [`ModifyError::InPlace`] for no spacing,
/// and [`ModifyError::TooLarge`] past what a coordinate holds.
pub fn linear(spacing: Steps, count: u32) -> Result<Vec<Motion>, ModifyError> {
    let copies = copies_of(count)?;
    if spacing == [0, 0] {
        return Err(ModifyError::InPlace);
    }
    (1..=i64::from(copies))
        .map(|k| {
            let [x, y] = spacing;
            Ok(Motion::by([
                x.checked_mul(k).ok_or(ModifyError::TooLarge)?,
                y.checked_mul(k).ok_or(ModifyError::TooLarge)?,
            ]))
        })
        .collect()
}

/// The nearest whole number to `k / n` of `whole`, a half away from zero; `n` above zero.
fn share(whole: i64, k: i64, n: i64) -> Result<i64, ModifyError> {
    // |whole k| is under 2^63 x 2^14, and twice it and n under 2^79: an i128 holds them.
    let twice = i128::from(whole)
        .checked_mul(i128::from(k))
        .and_then(|p| p.checked_mul(2))
        .ok_or(ModifyError::TooLarge)?;
    let n = i128::from(n);
    let doubled = n.checked_mul(2).ok_or(ModifyError::TooLarge)?;
    let up = twice
        .unsigned_abs()
        .checked_add(n.unsigned_abs())
        .ok_or(ModifyError::TooLarge)?
        .checked_div(doubled.unsigned_abs())
        .ok_or(ModifyError::TooLarge)?;
    let up = i128::try_from(up).map_err(|_| ModifyError::TooLarge)?;
    let signed = if twice < 0 { up.saturating_neg() } else { up };
    i64::try_from(signed).map_err(|_| ModifyError::TooLarge)
}

/// A linear array of `count` in all, the last `span` from the originals.
///
/// # Errors
/// As [`linear`], with [`ModifyError::InPlace`] for no span.
pub fn linear_span(span: Steps, count: u32) -> Result<Vec<Motion>, ModifyError> {
    let copies = i64::from(copies_of(count)?);
    if span == [0, 0] {
        return Err(ModifyError::InPlace);
    }
    let [x, y] = span;
    (1..=copies)
        .map(|k| Ok(Motion::by([share(x, k, copies)?, share(y, k, copies)?])))
        .collect()
}

/// Copies of a radial array about `centre`, copy `k` turned by `k` times `step / per` angle
/// units: refused when there are none to make, or when `count` of them would come round onto the
/// originals.
fn round_about(centre: Steps, step: i64, per: i64, count: u32) -> Result<Vec<Motion>, ModifyError> {
    let copies = copies_of(count)?;
    // per is at most a count, and the units of a turn's per-th are whole in per turns.
    let circle = i128::from(per)
        .checked_mul(i128::from(FULL))
        .ok_or(ModifyError::TooLarge)?;
    let step = i128::from(step).rem_euclid(circle);
    if step == 0 {
        return Err(ModifyError::InPlace);
    }
    // The fewest steps that come back round: a whole circle over what the step shares with it.
    let round = circle
        .checked_div(step.gcd(&circle))
        .ok_or(ModifyError::TooLarge)?;
    if i128::from(count) > round {
        return Err(ModifyError::RoundAgain {
            count,
            most: u32::try_from(round).unwrap_or(u32::MAX),
        });
    }
    (1..=i128::from(copies))
        .map(|k| {
            let turned = step.checked_mul(k).ok_or(ModifyError::TooLarge)?;
            Ok(Motion::turn_about(
                centre,
                &Turn::of_fraction(turned, i128::from(per)),
            ))
        })
        .collect()
}

/// A radial array of `count` in all about `centre`, turned `angle` apart.
///
/// # Errors
/// As [`linear`]; [`ModifyError::InPlace`] for a whole number of full turns;
/// [`ModifyError::RoundAgain`] when the copies would come round onto the originals.
pub fn radial(centre: Steps, angle: Angle, count: u32) -> Result<Vec<Motion>, ModifyError> {
    round_about(centre, angle.units(), 1, count)
}

/// A radial array of `count` in all about `centre`, the last turned `sweep` from the originals;
/// a sweep of exactly a full turn, either way, spreads the `count` evenly round.
///
/// # Errors
/// As [`radial`].
pub fn radial_sweep(centre: Steps, sweep: Angle, count: u32) -> Result<Vec<Motion>, ModifyError> {
    let copies = copies_of(count)?;
    let per = if sweep.units().unsigned_abs() == FULL.unsigned_abs() {
        count
    } else {
        copies
    };
    round_about(centre, sweep.units(), i64::from(per), count)
}
