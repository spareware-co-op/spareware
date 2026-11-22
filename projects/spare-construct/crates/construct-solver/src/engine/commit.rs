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
//!
//! Curves. A radius is stored as a whole number of millimetres, like every length, rounded by the
//! same rule from an origin of zero; an arc's centre and ends are ordinary whole-millimetre
//! points. Before the constraints, every arc is measured: each end's distance from the centre,
//! from the exact integer squared distance and one square root, must be within the model
//! tolerance of the stored radius, or the solve is refused with [`SolveError::ArcEndRefused`]. An
//! arc whose ends round to one place, or onto its centre, is degenerate. So a stored arc's ends
//! are on its stored circle within 0.01 mm. Chapter 15.10's exact arc form (the centre a rational
//! point on the ends' bisector) is the kernel's work; until it answers, an arc whose ends cannot
//! fall on whole millimetres near its circle is refused rather than stored loose. A tangency is
//! measured as a distance (the centre from the line's carrier, or the centres from each other,
//! against the radii on the stored side or way), whichever residual form the solver used, so a
//! tangency that solved to the other side shows as a miss of about twice the radius.

use crate::outcome::{MODEL_TOLERANCE_MM, SolveError};
use crate::sketch::{
    COORD_LIMIT_MM, Constraint, ConstraintId, Curve, CurveId, Point, PointId, Side, Sketch, Touch,
    cross, diff, dot,
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

/// A curve's stored centre and radius.
fn circle(sketch: &Sketch, points: &[Point], radii: &[i64], id: CurveId) -> Option<(Point, f64)> {
    let curve = sketch.curve(id)?;
    let r = *radii.get(usize::try_from(id.0).ok()?)?;
    Some((point(points, curve.centre())?, real(i128::from(r))))
}

/// The signed distance of `p` from a line's carrier (positive to the left); `Err` for a line of
/// no length.
fn signed_from_line(start: Point, end: Point, p: Point) -> Option<Result<f64, ()>> {
    let direction = diff(start, end)?;
    let along = length(direction)?;
    if along > 0.0 {
        Some(Ok(real(cross(direction, diff(start, p)?)?) / along))
    } else {
        Some(Err(()))
    }
}

/// How far one constraint is from holding on whole-millimetre points, in millimetres.
#[expect(
    clippy::too_many_lines,
    reason = "one arm per constraint kind, side by side so each kind reads in one place"
)]
fn miss(
    sketch: &Sketch,
    points: &[Point],
    radii: &[i64],
    constraint: Constraint,
) -> Option<Result<f64, ()>> {
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
        Constraint::PointOnLine(held, l) => {
            let (start, end) = line(sketch, points, l)?;
            signed_from_line(start, end, point(points, held)?)?.map(f64::abs)
        }
        Constraint::PointOnCurve(held, c) => {
            let (centre, r) = circle(sketch, points, radii, c)?;
            Ok((length(diff(centre, point(points, held)?)?)? - r).abs())
        }
        Constraint::Concentric(a, b) => {
            let (ca, _) = circle(sketch, points, radii, a)?;
            let (cb, _) = circle(sketch, points, radii, b)?;
            let (dx, dy) = diff(ca, cb)?;
            Ok(real(dx.abs().max(dy.abs())))
        }
        Constraint::EqualRadius(a, b) => {
            let (_, ra) = circle(sketch, points, radii, a)?;
            let (_, rb) = circle(sketch, points, radii, b)?;
            Ok((ra - rb).abs())
        }
        Constraint::Radius { curve, mm } => {
            let (_, r) = circle(sketch, points, radii, curve)?;
            Ok((r - real(i128::from(mm))).abs())
        }
        Constraint::Diameter { curve, mm } => {
            let (_, r) = circle(sketch, points, radii, curve)?;
            Ok((2.0 * r - real(i128::from(mm))).abs())
        }
        Constraint::TangentLine {
            line: l,
            curve,
            side,
        } => {
            let (start, end) = line(sketch, points, l)?;
            let (centre, r) = circle(sketch, points, radii, curve)?;
            let signed = match side {
                Side::Left => r,
                Side::Right => -r,
            };
            signed_from_line(start, end, centre)?.map(|d| (d - signed).abs())
        }
        Constraint::TangentCurves { a, b, touch } => {
            let (ca, ra) = circle(sketch, points, radii, a)?;
            let (cb, rb) = circle(sketch, points, radii, b)?;
            let gap = length(diff(ca, cb)?)?;
            let wanted = match touch {
                Touch::Outside => ra + rb,
                Touch::FirstInside => rb - ra,
                Touch::SecondInside => ra - rb,
            };
            Ok((gap - wanted).abs())
        }
    })
}

/// How far an arc's ends are off its circle on whole-millimetre points, in millimetres; `Err`
/// for an arc whose ends are one place or at its centre. `None` for a circle.
fn arc_miss(points: &[Point], radius_mm: i64, curve: Curve) -> Option<Result<f64, ()>> {
    let (start, end) = curve.ends()?;
    let centre = point(points, curve.centre())?;
    let (s, e) = (point(points, start)?, point(points, end)?);
    if s == e || s == centre || e == centre {
        return Some(Err(()));
    }
    let r = real(i128::from(radius_mm));
    let off_start = (length(diff(centre, s)?)? - r).abs();
    let off_end = (length(diff(centre, e)?)? - r).abs();
    Some(Ok(off_start.max(off_end)))
}

/// Checks whole-millimetre points and radii against every arc and then every constraint, or
/// refuses them. `points` holds every point and `radii` every curve's radius in authoring order,
/// rounded by the rule above.
pub(super) fn check(sketch: &Sketch, points: &[Point], radii: &[i64]) -> Result<(), SolveError> {
    for (index, (curve, r)) in sketch.curves().iter().zip(radii).enumerate() {
        let id = CurveId(u32::try_from(index).map_err(|_| SolveError::Decomposition)?);
        match arc_miss(points, *r, *curve) {
            None => {}
            Some(Ok(residual_mm)) if residual_mm <= MODEL_TOLERANCE_MM => {}
            Some(Ok(residual_mm)) => {
                return Err(SolveError::ArcEndRefused {
                    curve: id,
                    residual_mm,
                });
            }
            Some(Err(())) => return Err(SolveError::DegenerateCurve { curve: id }),
        }
    }
    for (index, constraint) in sketch.constraints().iter().enumerate() {
        let id = ConstraintId(u32::try_from(index).map_err(|_| SolveError::Decomposition)?);
        match miss(sketch, points, radii, *constraint) {
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
