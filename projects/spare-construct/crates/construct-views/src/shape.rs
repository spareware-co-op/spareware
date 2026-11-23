//! The exact plane geometry a view's question needs, in whole grid steps (1/6,400 mm) as `i128`.
//!
//! Boxes are construct-model's whole-millimetre bounds; a view's heights and a section's line are
//! construct-params' lengths, in steps. Both are brought to steps and compared there, so nothing
//! is rounded between a stored value and an answer except the one place written below: the far
//! corners of a section's region. Every step is checked: a view or an element so far out that a
//! product would not fit is refused as [`ViewError::OutOfRange`], never wrapped.

use crate::cut::Side;
use crate::error::ViewError;
use construct_params::Length;

/// A point in steps.
pub(crate) type P = (i128, i128);

/// A box of the plan in steps: its low and high corners.
pub(crate) struct Rect {
    pub(crate) min: P,
    pub(crate) max: P,
}

fn out_of_range() -> ViewError {
    ViewError::OutOfRange(
        "a view and an element are too far apart, or too large, to test exactly".to_owned(),
    )
}

fn mul(a: i128, b: i128) -> Result<i128, ViewError> {
    a.checked_mul(b).ok_or_else(out_of_range)
}

fn add(a: i128, b: i128) -> Result<i128, ViewError> {
    a.checked_add(b).ok_or_else(out_of_range)
}

fn sub(a: i128, b: i128) -> Result<i128, ViewError> {
    a.checked_sub(b).ok_or_else(out_of_range)
}

/// A whole-millimetre coordinate in steps.
pub(crate) fn mm_to_steps(mm: i64) -> Result<i128, ViewError> {
    mul(i128::from(mm), i128::from(Length::STEPS_PER_MM))
}

/// Steps down to whole millimetres, rounded toward minus infinity.
pub(crate) fn floor_mm(steps: i128) -> i128 {
    steps.div_euclid(i128::from(Length::STEPS_PER_MM))
}

/// Steps up to whole millimetres, rounded toward plus infinity.
pub(crate) fn ceil_mm(steps: i128) -> i128 {
    let per = i128::from(Length::STEPS_PER_MM);
    let down = steps.div_euclid(per);
    if steps.rem_euclid(per) == 0 {
        down
    } else {
        down.saturating_add(1)
    }
}

/// Whether a convex polygon of steps, or a segment (two points), and a box share a point, touching
/// counted. By the separating axis rule: the box's two axes, and for each edge of the polygon its
/// direction and its normal. Any axis that keeps the two apart proves they do not meet; none
/// proves they do.
pub(crate) fn meets(poly: &[P], rect: &Rect) -> Result<bool, ViewError> {
    let corners = [
        rect.min,
        (rect.max.0, rect.min.1),
        rect.max,
        (rect.min.0, rect.max.1),
    ];
    let mut axes: Vec<P> = vec![(1, 0), (0, 1)];
    for (from, to) in poly.iter().zip(poly.iter().cycle().skip(1)) {
        let edge = (sub(to.0, from.0)?, sub(to.1, from.1)?);
        axes.push(edge);
        axes.push((edge.1.checked_neg().ok_or_else(out_of_range)?, edge.0));
    }
    for axis in axes {
        if axis == (0, 0) {
            continue;
        }
        let project = |point: &P| -> Result<i128, ViewError> {
            add(mul(axis.0, point.0)?, mul(axis.1, point.1)?)
        };
        let (poly_low, poly_high) = range_of(poly.iter().map(project))?;
        let (box_low, box_high) = range_of(corners.iter().map(project))?;
        if poly_high < box_low || box_high < poly_low {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The lowest and highest of some projections, or the first refusal among them.
fn range_of(
    values: impl Iterator<Item = Result<i128, ViewError>>,
) -> Result<(i128, i128), ViewError> {
    let mut range: Option<(i128, i128)> = None;
    for value in values {
        let value = value?;
        range = Some(match range {
            None => (value, value),
            Some((low, high)) => (low.min(value), high.max(value)),
        });
    }
    range.ok_or_else(out_of_range)
}

/// The four corners of the region a section or elevation sees: the line's two ends, and the two
/// points `far` away from them, square to the line on the side the view looks to.
///
/// The far points are the only rounded numbers: each coordinate of the offset is the exact
/// distance rounded up to the next whole step, so the region is the true one to within one grid
/// step (156 nm, a fraction of the 0.01 mm model tolerance) and never smaller on that account.
pub(crate) fn region(from: P, to: P, look: Side, far: i128) -> Result<[P; 4], ViewError> {
    let direction = (sub(to.0, from.0)?, sub(to.1, from.1)?);
    let normal = match look {
        Side::Left => (
            direction.1.checked_neg().ok_or_else(out_of_range)?,
            direction.0,
        ),
        Side::Right => (
            direction.1,
            direction.0.checked_neg().ok_or_else(out_of_range)?,
        ),
    };
    let square = add(
        mul(direction.0, direction.0)?,
        mul(direction.1, direction.1)?,
    )?;
    let square = u128::try_from(square).map_err(|_| out_of_range())?;
    let far = u128::try_from(far).map_err(|_| out_of_range())?;
    let offset = |component: i128| -> Result<i128, ViewError> {
        if component == 0 {
            return Ok(0);
        }
        let reach = component
            .unsigned_abs()
            .checked_mul(far)
            .ok_or_else(out_of_range)?;
        let reach = reach.checked_mul(reach).ok_or_else(out_of_range)?;
        let magnitude = i128::try_from(reach.checked_div(square).ok_or_else(out_of_range)?.isqrt())
            .map_err(|_| out_of_range())?
            .checked_add(1)
            .ok_or_else(out_of_range)?;
        if component < 0 {
            magnitude.checked_neg().ok_or_else(out_of_range)
        } else {
            Ok(magnitude)
        }
    };
    let push = (offset(normal.0)?, offset(normal.1)?);
    Ok([
        from,
        to,
        (add(to.0, push.0)?, add(to.1, push.1)?),
        (add(from.0, push.0)?, add(from.1, push.1)?),
    ])
}

/// The box that holds all of some points.
pub(crate) fn bounding(points: &[P]) -> Rect {
    let mut rect = Rect {
        min: (i128::MAX, i128::MAX),
        max: (i128::MIN, i128::MIN),
    };
    for point in points {
        rect.min = (rect.min.0.min(point.0), rect.min.1.min(point.1));
        rect.max = (rect.max.0.max(point.0), rect.max.1.max(point.1));
    }
    rect
}
