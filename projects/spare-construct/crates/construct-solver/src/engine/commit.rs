//! From solution to stored geometry (chapter 15.10, in its simplest form). The solver works in
//! floating point; the sketch stores whole millimetres.
//!
//! The rounding rule: each solved coordinate is unscaled once, as origin + value x scale (the
//! origin is a whole millimetre and the scale a power of two, so only the sum rounds), and then
//! rounded to the nearest whole millimetre, halves away from zero. A fixed point is never touched.
//!
//! After rounding every constraint is measured again on the whole-millimetre points, from exact
//! integer differences, cross and dot products, and must hold within the model tolerance
//! (0.01 mm). If one does not, the solve is refused with the first such constraint in authoring
//! order. Coincidences, horizontals and verticals therefore hold exactly; every other kind holds
//! within the tolerance (chapter 15.10's "bounded" row; its exact carrier forms are the kernel's
//! work).

use crate::outcome::{MODEL_TOLERANCE_MM, SolveError};
use crate::sketch::{
    COORD_LIMIT_MM, Constraint, ConstraintId, Point, PointId, Side, Sketch, cross, diff, dot,
};

/// An exact integer as the nearest double. Every value here is below 2^127.
#[expect(
    clippy::cast_precision_loss,
    reason = "rounding to the nearest double is the stated measuring rule, the same everywhere"
)]
pub(super) fn real(value: i128) -> f64 {
    value as f64
}

/// A solved, unscaled coordinate as a whole millimetre, by the rounding rule above.
pub(super) fn to_mm(origin: f64, value: f64, scale: f64) -> Option<i64> {
    let mm = (origin + value * scale).round();
    let limit = f64::from(i32::try_from(COORD_LIMIT_MM).ok()?);
    if !mm.is_finite() || mm.abs() > limit {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "mm is whole and |mm| is at most COORD_LIMIT_MM, checked above"
    )]
    Some(mm as i64)
}

fn point(points: &[Point], id: PointId) -> Option<Point> {
    points.get(usize::try_from(id.0).ok()?).copied()
}

fn line(sketch: &Sketch, points: &[Point], id: crate::sketch::LineId) -> Option<(Point, Point)> {
    let line = sketch.line(id)?;
    Some((point(points, line.start)?, point(points, line.end)?))
}

fn length(u: (i128, i128)) -> Option<f64> {
    Some(real(dot(u, u)?).sqrt())
}

/// How far one constraint is from holding on whole-millimetre points, in millimetres.
fn miss(sketch: &Sketch, points: &[Point], constraint: Constraint) -> Option<Result<f64, ()>> {
    let two_lines = |a, b| -> Option<((i128, i128), (i128, i128))> {
        let (a0, a1) = line(sketch, points, a)?;
        let (b0, b1) = line(sketch, points, b)?;
        Some((diff(a0, a1)?, diff(b0, b1)?))
    };
    let shorter = |u, v| -> Option<Result<f64, ()>> {
        let shorter = length(u)?.min(length(v)?);
        Some(if shorter > 0.0 { Ok(shorter) } else { Err(()) })
    };
    Some(match constraint {
        Constraint::Coincident(a, b) => {
            let (dx, dy) = diff(point(points, a)?, point(points, b)?)?;
            Ok(real(dx.abs().max(dy.abs())))
        }
        Constraint::Horizontal(l) => {
            let (a, b) = line(sketch, points, l)?;
            Ok(real(diff(a, b)?.1.abs()))
        }
        Constraint::Vertical(l) => {
            let (a, b) = line(sketch, points, l)?;
            Ok(real(diff(a, b)?.0.abs()))
        }
        // How far the longer line's far end is from running the stated way.
        Constraint::Parallel(a, b) => {
            let (u, v) = two_lines(a, b)?;
            let turn = real(cross(u, v)?.abs());
            shorter(u, v)?.map(|s| turn / s)
        }
        Constraint::Perpendicular(a, b) => {
            let (u, v) = two_lines(a, b)?;
            let along = real(dot(u, v)?.abs());
            shorter(u, v)?.map(|s| along / s)
        }
        Constraint::EqualLength(a, b) => {
            let (u, v) = two_lines(a, b)?;
            Ok((length(u)? - length(v)?).abs())
        }
        Constraint::Distance { a, b, mm } => {
            let gap = length(diff(point(points, a)?, point(points, b)?)?)?;
            Ok((gap - real(i128::from(mm))).abs())
        }
        Constraint::PointLineDistance {
            point: held,
            line: l,
            mm,
            side,
        } => {
            let (start, end) = line(sketch, points, l)?;
            let direction = diff(start, end)?;
            let along = length(direction)?;
            if along > 0.0 {
                let signed = match side {
                    Side::Left => real(i128::from(mm)),
                    Side::Right => -real(i128::from(mm)),
                };
                let turn = real(cross(direction, diff(start, point(points, held)?)?)?);
                Ok((turn / along - signed).abs())
            } else {
                Err(())
            }
        }
        Constraint::Fixed(_) => Ok(0.0),
    })
}

/// Checks whole-millimetre points against every constraint, or refuses them. `points` holds
/// every point in authoring order, rounded by the rule above.
pub(super) fn check(sketch: &Sketch, points: &[Point]) -> Result<(), SolveError> {
    for (index, constraint) in sketch.constraints().iter().enumerate() {
        let id = ConstraintId(u32::try_from(index).map_err(|_| SolveError::Decomposition)?);
        match miss(sketch, points, *constraint) {
            Some(Ok(residual_mm)) if residual_mm <= MODEL_TOLERANCE_MM => {}
            Some(Ok(residual_mm)) => {
                return Err(SolveError::RoundingRefused {
                    constraint: id,
                    residual_mm,
                });
            }
            Some(Err(())) | None => return Err(SolveError::Degenerate { constraint: id }),
        }
    }
    Ok(())
}
