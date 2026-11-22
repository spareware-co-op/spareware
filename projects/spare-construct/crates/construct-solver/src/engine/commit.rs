//! From solution to stored geometry (chapter 15.10, in its simplest form). The solver works in
//! floating point; the sketch stores whole grid steps of the kernel (`Q0`, 1/6,400 mm).
//!
//! The rounding rule: each solved coordinate is unscaled once, as origin + value x scale (the
//! origin is a whole grid step and the scale a power of two, so only the sum rounds), and then
//! rounded to the nearest grid step, halves to even, as the kernel commits a point to its grid
//! (chapter 6.1 and 6.3): at most half a step, 78 nm, per coordinate. A fixed point is never
//! touched.
//!
//! After rounding every constraint is measured again on the grid points, from exact integer
//! differences, cross and dot products, and must hold within the model tolerance (0.01 mm, 64
//! steps). If one does not, the solve is refused with the first such constraint in authoring
//! order. Coincidences, horizontals and verticals therefore hold exactly; every other kind holds
//! within the tolerance (chapter 15.10's "bounded" row; its exact carrier forms are the kernel's
//! work). Half a step is far inside the tolerance, so sloped lines, arcs ended anywhere and
//! sloped tangencies are accepted; a refusal is left only where no grid points near the answer
//! meet a constraint, such as a line a fraction of a micrometre long held parallel to a long one.
//!
//! Curves. A radius is stored as a whole number of grid steps, like every length, rounded by the
//! same rule from an origin of zero; an arc's centre and ends are ordinary grid points. Before
//! the constraints, every arc is measured: each end's distance from the centre, from the exact
//! integer squared distance and one square root, must be within the model tolerance of the
//! stored radius, or the solve is refused with [`SolveError::ArcEndRefused`]. An arc whose ends
//! round to one place, or onto its centre, is degenerate. So a stored arc's ends are on its
//! stored circle within 0.01 mm. Chapter 15.10's exact arc form (the centre a rational point on
//! the ends' bisector) is the kernel's work. A tangency is measured as a distance (the centre
//! from the line's carrier, or the centres from each other, against the radii on the stored side
//! or way), whichever residual form the solver used, so a tangency that solved to the other side
//! shows as a miss of about twice the radius.

use crate::outcome::{MODEL_TOLERANCE_STEPS, SolveError};
use crate::sketch::{
    COORD_LIMIT, Constraint, ConstraintId, Curve, CurveId, Point, PointId, STEPS_PER_MM, Side,
    Sketch, Touch, cross, diff, dot,
};

/// An exact integer as the nearest double. Every value here is below 2^127.
#[expect(
    clippy::cast_precision_loss,
    reason = "rounding to the nearest double is the stated measuring rule, the same everywhere"
)]
pub(super) fn real(value: i128) -> f64 {
    value as f64
}

/// A solved, unscaled coordinate as a whole grid step, by the rounding rule above.
pub(super) fn to_steps(origin: f64, value: f64, scale: f64) -> Option<i64> {
    let steps = (origin + value * scale).round_ties_even();
    // COORD_LIMIT is below 2^53, so it is exact as a double.
    let limit = real(i128::from(COORD_LIMIT));
    if !steps.is_finite() || steps.abs() > limit {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "steps is whole and |steps| is at most COORD_LIMIT, checked above"
    )]
    Some(steps as i64)
}

/// A distance in grid steps as millimetres, for a report.
pub(super) fn in_mm(steps: f64) -> f64 {
    steps / real(i128::from(STEPS_PER_MM))
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

/// How far one constraint is from holding on grid points, in grid steps.
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
        Constraint::Distance { a, b, steps } => {
            let gap = length(diff(point(points, a)?, point(points, b)?)?)?;
            Ok((gap - real(i128::from(steps))).abs())
        }
        Constraint::PointLineDistance {
            point: held,
            line: l,
            steps,
            side,
        } => {
            let (start, end) = line(sketch, points, l)?;
            let direction = diff(start, end)?;
            let along = length(direction)?;
            if along > 0.0 {
                let signed = match side {
                    Side::Left => real(i128::from(steps)),
                    Side::Right => -real(i128::from(steps)),
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
        Constraint::Radius { curve, steps } => {
            let (_, r) = circle(sketch, points, radii, curve)?;
            Ok((r - real(i128::from(steps))).abs())
        }
        Constraint::Diameter { curve, steps } => {
            let (_, r) = circle(sketch, points, radii, curve)?;
            Ok((2.0 * r - real(i128::from(steps))).abs())
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

/// How far an arc's ends are off its circle on grid points, in grid steps; `Err`
/// for an arc whose ends are one place or at its centre. `None` for a circle.
fn arc_miss(points: &[Point], radius: i64, curve: Curve) -> Option<Result<f64, ()>> {
    let (start, end) = curve.ends()?;
    let centre = point(points, curve.centre())?;
    let (s, e) = (point(points, start)?, point(points, end)?);
    if s == e || s == centre || e == centre {
        return Some(Err(()));
    }
    let r = real(i128::from(radius));
    let off_start = (length(diff(centre, s)?)? - r).abs();
    let off_end = (length(diff(centre, e)?)? - r).abs();
    Some(Ok(off_start.max(off_end)))
}

/// Checks grid points and radii against every arc and then every constraint, or
/// refuses them. `points` holds every point and `radii` every curve's radius in authoring order,
/// rounded by the rule above.
pub(super) fn check(sketch: &Sketch, points: &[Point], radii: &[i64]) -> Result<(), SolveError> {
    let tolerance = real(i128::from(MODEL_TOLERANCE_STEPS));
    for (index, (curve, r)) in sketch.curves().iter().zip(radii).enumerate() {
        let id = CurveId(u32::try_from(index).map_err(|_| SolveError::Decomposition)?);
        match arc_miss(points, *r, *curve) {
            None => {}
            Some(Ok(residual)) if residual <= tolerance => {}
            Some(Ok(residual)) => {
                return Err(SolveError::ArcEndRefused {
                    curve: id,
                    residual_mm: in_mm(residual),
                });
            }
            Some(Err(())) => return Err(SolveError::DegenerateCurve { curve: id }),
        }
    }
    for (index, constraint) in sketch.constraints().iter().enumerate() {
        let id = ConstraintId(u32::try_from(index).map_err(|_| SolveError::Decomposition)?);
        match miss(sketch, points, radii, *constraint) {
            Some(Ok(residual)) if residual <= tolerance => {}
            Some(Ok(residual)) => {
                return Err(SolveError::RoundingRefused {
                    constraint: id,
                    residual_mm: in_mm(residual),
                });
            }
            Some(Err(())) | None => return Err(SolveError::Degenerate { constraint: id }),
        }
    }
    Ok(())
}
