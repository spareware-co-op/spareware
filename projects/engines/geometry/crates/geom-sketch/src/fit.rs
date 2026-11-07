//! Fit-point splines (piece G220; chapter 15.2, "Fit-point splines").
//!
//! What it does: a fit-point spline is a spline whose author states the points it passes through.
//! [`author`] interpolates the fit points with `geom-nurbs` (The NURBS Book, section 9.2 and
//! algorithm A9.1: chord-length parameters, averaged knots, a banded solve), and gives the plain
//! items a caller adds: a point for each control point, the spline on them, and for each fit point
//! one [`ConstraintKind::SplineThrough`] at the parameter the fit used. Within a solve the
//! parametrisation and knots are frozen, so the control points are a fixed linear function of the
//! fit points and the Jacobian of the through-constraints is exact. [`reinterpolate`] is the step
//! after a solve: it interpolates the fit points as they now stand and says how far the control
//! points and the parameters moved, so the solver's loop (piece G220a, which has the solver)
//! repeats until they move by less than q/64, and keeps the last parametrisation, which is the one
//! stored in the constraints.
//!
//! Guarantees:
//!
//! - **A certified fit.** [`FitSpline::error`] is `geom-nurbs`'s certified bound on how far the
//!   interpolant misses any fit point.
//! - **Plain items.** The items returned are ones a caller could write by hand; nothing is
//!   changed in the sketch, so a refusal leaves it as it was.
//! - **Replayable.** Keys run up from the caller's first key, so one replay authors one set of
//!   ids.
//! - **No panics, no recursion, no unsafe code.**
//!
//! Method: L. Piegl and W. Tiller, *The NURBS Book*, 2nd ed., Springer (1997),
//! <https://doi.org/10.1007/978-3-642-59223-2>, section 9.2.1 and algorithm A9.1, through
//! `geom_nurbs::Curve::interpolate` (versioned, byte-identical across targets).
//!
//! Cost: one banded interpolation, linear in the fit points.

extern crate alloc;

use alloc::vec::Vec;

use geom_nurbs::{Curve, FitVersion, Parametrization};
use geom_run::{Budget, WorkMeter};

use crate::solve::{Solved, TOLERANCE, solve};

use crate::length::larger;
use crate::model::{
    Constraint, ConstraintKind, Entity, EntityKind, ItemId, Sketch, SketchError, Standing,
};

/// The work one interpolation may charge.
const WORK: u64 = 1 << 40;

/// A spline interpolated through fit points.
#[derive(Clone, Debug, PartialEq)]
pub struct FitSpline {
    /// The degree.
    pub degree: u32,
    /// The knots.
    pub knots: Vec<f64>,
    /// The control points.
    pub points: Vec<[f64; 2]>,
    /// The parameter of each fit point, in the fit points' order.
    pub parameters: Vec<f64>,
    /// A certified bound on how far the spline misses a fit point.
    pub error: f64,
}

/// Interpolates `fit` (at least `degree` + 1 points) with a clamped spline of `degree`.
///
/// # Errors
///
/// [`SketchError::WrongKind`] if `geom-nurbs` refuses the data (too few points, a repeated
/// point, a degree it does not build); [`SketchError::Stopped`]; [`SketchError::OutOfMemory`].
pub fn interpolate(fit: &[[f64; 2]], degree: u32) -> Result<FitSpline, SketchError> {
    let mut meter = WorkMeter::new(Budget::of_work(WORK));
    let p = usize::try_from(degree).map_err(|_| SketchError::Internal)?;
    let done = Curve::<2>::interpolate(
        fit,
        p,
        Parametrization::ChordLength,
        FitVersion::CURRENT,
        &mut meter,
    )
    .map_err(|e| match e {
        geom_nurbs::NurbsError::Stopped(_) => SketchError::Stopped,
        _ => SketchError::WrongKind(ItemId::new(0, 0)),
    })?;
    let mut points: Vec<[f64; 2]> = Vec::new();
    points
        .try_reserve_exact(done.curve.basis().count())
        .map_err(|_| SketchError::OutOfMemory)?;
    for i in 0..done.curve.basis().count() {
        points.push(
            done.curve
                .control_point(i)
                .map_err(|_| SketchError::Internal)?,
        );
    }
    Ok(FitSpline {
        degree,
        knots: done.curve.basis().knots().to_vec(),
        points,
        parameters: done.parameters,
        error: done.error,
    })
}

/// The items of a fit-point spline.
#[derive(Clone, Debug, PartialEq)]
pub struct Authored {
    /// A point for each control point, then the spline.
    pub entities: Vec<Entity>,
    /// One [`ConstraintKind::SplineThrough`] for each fit point.
    pub constraints: Vec<Constraint>,
    /// The interpolation they come from.
    pub fit: FitSpline,
}

/// The items for the spline `spline` through the points `fit_points` of `sketch`: the control
/// points are ordinary points with ids in `sequence`, keys from `first_key`, and the through
/// constraints take the keys after them.
///
/// # Errors
///
/// What [`interpolate`] refuses; [`SketchError::Unknown`] and [`SketchError::WrongKind`] for a fit
/// point that is not a point of the sketch; [`SketchError::OutOfMemory`].
pub fn author(
    sketch: &Sketch,
    fit_points: &[ItemId],
    degree: u32,
    spline: ItemId,
    sequence: u64,
    first_key: u64,
) -> Result<Authored, SketchError> {
    let mut at: Vec<[f64; 2]> = Vec::new();
    at.try_reserve_exact(fit_points.len())
        .map_err(|_| SketchError::OutOfMemory)?;
    for id in fit_points {
        at.push(sketch.point(*id)?);
    }
    let fit = interpolate(&at, degree)?;
    let mut key = first_key;
    let mut next = || -> Result<ItemId, SketchError> {
        let id = ItemId::new(sequence, key);
        key = key.checked_add(1).ok_or(SketchError::Internal)?;
        Ok(id)
    };
    let mut entities: Vec<Entity> = Vec::new();
    let mut ids: Vec<ItemId> = Vec::new();
    for [x, y] in &fit.points {
        let id = next()?;
        ids.push(id);
        entities.push(Entity {
            id,
            kind: EntityKind::Point { x: *x, y: *y },
            construction: true,
        });
    }
    entities.push(Entity {
        id: spline,
        kind: EntityKind::Spline {
            degree,
            points: ids,
            knots: fit.knots.clone(),
            weights: Vec::new(),
        },
        construction: false,
    });
    let mut constraints: Vec<Constraint> = Vec::new();
    for (point, u) in fit_points.iter().zip(&fit.parameters) {
        constraints.push(Constraint {
            id: next()?,
            kind: ConstraintKind::SplineThrough {
                spline,
                point: *point,
                at: *u,
            },
            standing: Standing::Driving,
            priority: 0,
        });
    }
    Ok(Authored {
        entities,
        constraints,
        fit,
    })
}

/// What re-interpolating a fit-point spline after a solve found.
#[derive(Clone, Debug, PartialEq)]
pub struct Refit {
    /// The interpolation through the fit points as they now stand.
    pub fit: FitSpline,
    /// The through-constraints' points, in parameter order.
    pub fit_points: Vec<ItemId>,
    /// The largest change of a control point's x or y from where the sketch has it.
    pub control_moved: f64,
    /// The largest change of a fit point's parameter.
    pub parameter_moved: f64,
}

/// Interpolates the fit points of `spline`, the points of its [`ConstraintKind::SplineThrough`]
/// constraints in order of their parameters, as they stand in `sketch`.
///
/// # Errors
///
/// [`SketchError::WrongKind`] if `spline` has no through-constraints or is not a spline of the
/// sketch's, or what [`interpolate`] refuses.
pub fn reinterpolate(sketch: &Sketch, spline: ItemId) -> Result<Refit, SketchError> {
    let EntityKind::Spline { degree, points, .. } = &sketch.entity(spline)?.kind else {
        return Err(SketchError::WrongKind(spline));
    };
    let mut through: Vec<(f64, ItemId)> = Vec::new();
    for c in sketch.constraints() {
        if let ConstraintKind::SplineThrough {
            spline: s,
            point,
            at,
        } = c.kind
            && s == spline
        {
            through.push((at, point));
        }
    }
    through.sort_by(|a, b| a.0.total_cmp(&b.0));
    if through.is_empty() {
        return Err(SketchError::WrongKind(spline));
    }
    let mut data: Vec<[f64; 2]> = Vec::new();
    let mut fit_points: Vec<ItemId> = Vec::new();
    for (_, id) in &through {
        data.push(sketch.point(*id)?);
        fit_points.push(*id);
    }
    let fit = interpolate(&data, *degree)?;
    let mut control_moved = 0.0_f64;
    for (id, new) in points.iter().zip(&fit.points) {
        let old = sketch.point(*id)?;
        control_moved = larger(
            control_moved,
            larger((old[0] - new[0]).abs(), (old[1] - new[1]).abs()),
        );
    }
    let mut parameter_moved = 0.0_f64;
    for ((old, _), new) in through.iter().zip(&fit.parameters) {
        parameter_moved = larger(parameter_moved, (old - new).abs());
    }
    Ok(Refit {
        fit,
        fit_points,
        control_moved,
        parameter_moved,
    })
}

/// The most re-interpolations [`settle`] makes: a fixed number of rounds, as section 15.2 asks.
pub const MOST_ROUNDS: u32 = 16;

/// A fit-point spline solved, and whether its parametrisation settled.
#[derive(Clone, Debug, PartialEq)]
pub struct Settled {
    /// The sketch at the last solve. Its through-constraints hold the parameters and its spline
    /// the knots that solve used: the last parametrisation, stored with the spline.
    pub solved: Solved,
    /// How many solves were made, from 1.
    pub rounds: u32,
    /// Whether the last re-interpolation moved every control point by less than [`TOLERANCE`], a
    /// 64th of a grid step (section 15.6): the report's answer to "did it settle".
    pub settled: bool,
    /// How far the last re-interpolation would have moved a control point.
    pub moved: f64,
}

/// `sketch` with `refit` laid in: the control points where it puts them, the spline on its new
/// knots, and each fit point's through-constraint at its new parameter.
fn lay_in(sketch: &Sketch, spline: ItemId, refit: &Refit) -> Result<Sketch, SketchError> {
    let EntityKind::Spline { degree, points, .. } = &sketch.entity(spline)?.kind else {
        return Err(SketchError::WrongKind(spline));
    };
    let mut values: Vec<f64> = Vec::new();
    values
        .try_reserve_exact(sketch.variables()?.len())
        .map_err(|_| SketchError::OutOfMemory)?;
    for variable in sketch.variables()? {
        let mut value = variable.value;
        if let Some(i) = points.iter().position(|p| *p == variable.owner)
            && let Some(point) = refit.fit.points.get(i)
        {
            value = point
                .get(usize::from(variable.slot))
                .copied()
                .unwrap_or(value);
        }
        values.push(value);
    }
    let mut next = sketch.with_values(&values)?;
    next = next.with_entity(Entity {
        id: spline,
        kind: EntityKind::Spline {
            degree: *degree,
            points: points.clone(),
            knots: refit.fit.knots.clone(),
            weights: Vec::new(),
        },
        construction: sketch.entity(spline)?.construction,
    })?;
    for constraint in sketch.constraints() {
        if let ConstraintKind::SplineThrough {
            spline: s, point, ..
        } = constraint.kind
            && s == spline
            && let Some(i) = refit.fit_points.iter().position(|p| *p == point)
            && let Some(at) = refit.fit.parameters.get(i)
        {
            next = next.with_constraint(Constraint {
                kind: ConstraintKind::SplineThrough {
                    spline,
                    point,
                    at: *at,
                },
                ..*constraint
            })?;
        }
    }
    Ok(next)
}

/// Solves `sketch` with the fit-point spline `spline` in it, and re-interpolates the spline through
/// its fit points as the solve left them, solving again, until the control points move by less than
/// [`TOLERANCE`] or [`MOST_ROUNDS`] rounds are used (section 15.2). Within each solve the
/// parametrisation and knots are frozen, so the control points are a fixed linear function of the
/// fit points and the Jacobian is exact.
///
/// # Errors
///
/// What [`solve`] and [`reinterpolate`] refuse.
pub fn settle(
    sketch: &Sketch,
    spline: ItemId,
    meter: &mut WorkMeter<'_>,
) -> Result<Settled, SketchError> {
    let mut current = sketch.clone();
    let mut round = 0_u32;
    loop {
        round = round.checked_add(1).ok_or(SketchError::Internal)?;
        let solved = solve(&current, meter)?;
        let refit = reinterpolate(&solved.sketch, spline)?;
        let done = refit.control_moved < TOLERANCE;
        if done || round >= MOST_ROUNDS {
            return Ok(Settled {
                moved: refit.control_moved,
                solved,
                rounds: round,
                settled: done,
            });
        }
        current = lay_in(&solved.sketch, spline, &refit)?;
    }
}
