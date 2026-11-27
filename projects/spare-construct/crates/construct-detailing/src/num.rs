//! The checked whole-number steps every module shares. A value or a product that does not fit is a
//! refusal naming the grid, never a wrap.

use crate::error::DetailError;
use construct_grids::Point;
use construct_params::Length;
use construct_solver::COORD_LIMIT;

pub(crate) fn too_big() -> DetailError {
    DetailError::OutOfRange(
        "That is too far from the origin, or too large, for the grid.".to_owned(),
    )
}

pub(crate) fn mul(a: i128, b: i128) -> Result<i128, DetailError> {
    a.checked_mul(b).ok_or_else(too_big)
}

pub(crate) fn add(a: i128, b: i128) -> Result<i128, DetailError> {
    a.checked_add(b).ok_or_else(too_big)
}

pub(crate) fn sub(a: i128, b: i128) -> Result<i128, DetailError> {
    a.checked_sub(b).ok_or_else(too_big)
}

/// A point as grid steps.
pub(crate) fn at(p: Point) -> [i128; 2] {
    [i128::from(p.x.steps()), i128::from(p.y.steps())]
}

/// A length as steps, refused past the kernel's grid.
pub(crate) fn in_grid(length: Length) -> Result<Length, DetailError> {
    if length.steps().unsigned_abs() > COORD_LIMIT.unsigned_abs() {
        return Err(DetailError::OutOfRange(format!(
            "{length} is beyond the kernel's grid (171.8 km from the origin)."
        )));
    }
    Ok(length)
}

/// A point, refused past the kernel's grid.
pub(crate) fn point_in_grid(p: Point) -> Result<Point, DetailError> {
    in_grid(p.x)?;
    in_grid(p.y)?;
    Ok(p)
}

/// A point of whole steps, as a point.
pub(crate) fn from_steps(p: [i64; 2]) -> Point {
    Point::new(Length::from_steps(p[0]), Length::from_steps(p[1]))
}
