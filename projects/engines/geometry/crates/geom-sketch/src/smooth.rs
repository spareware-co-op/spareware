//! Spline ends: their derivatives, and the smoothness of a join measured (piece G220; chapter 15.3,
//! "Smoothness").
//!
//! What it does: [`end_layout`] reads a spline end the way the constraints do: the first control
//! points counted from the end, and the factors that turn their differences into the end's
//! derivatives. [`end_jets`] gives those derivatives, outward from the end, and [`continuity`]
//! measures how smooth the join of two spline ends is, on a sketch as it stands: how far the
//! tangents are from one line (G1), how far the curvatures are from agreeing (G2) and how far the
//! rates of change of curvature are (G3).
//! [`measure_joins`] measures every spline join of a sketch, against lines and arcs too, and
//! [`solve_and_measure`] does it at the solution of the solver, so the continuity is that of the solved
//! curves. [`join_sense`] says which way round a join's tangent root is (a continuation or a cusp),
//! the quantity a path of the solver holds (piece G220a). The constraints of `equation` ([`crate::model::ConstraintKind::SplineJoin`])
//! write the same residuals over the same layout, so a join the solver meets measures as smooth
//! here.
//!
//! - **Derivatives from control points.** The k-th derivative of a B-spline is a B-spline of degree
//!   p − k whose control points are scaled differences of the last level's (Piegl and Tiller, *The
//!   NURBS Book*, 2nd ed., Springer (1997), eq. 3.8, doi:10.1007/978-3-642-59223-2); at a clamped
//!   end its value is the first control point. The finish is read as the start of the spline run
//!   backwards, so every end gives derivatives outward, and a join is two outward ends.
//! - **Curvature and its rate** are the signed curvature of a plane curve, `(d1 × d2) / |d1|³`, and
//!   its derivative along the arc, `((d1 × d3) |d1|² − 3 (d1 × d2)(d1 · d2)) / |d1|⁶` (the quotient
//!   rule on the first, then a division by the speed). Joined end to end, the travelling curve's
//!   curvature is minus the first end's outward curvature and the second's own, so G2 is the sum of
//!   the outward curvatures being zero and G3 is the outward rates being equal.
//!
//! Guarantees:
//!
//! - **Not rational, clamped.** Only splines with unit weights and clamped ends are read
//!   ([`Sketch::spline_jets`]); anything else is [`SketchError::WrongKind`], never a wrong number.
//! - **Deterministic.** The derivatives are the residuals' own functions on [`Real`].
//! - **No panics, no recursion, no unsafe code.**
//!
//! Method: section 15.3 (`projects/engines/geometry/architecture/15-sketches.md`), with
//! L. Piegl and W. Tiller, *The NURBS Book*, 2nd ed., Springer (1997),
//! <https://doi.org/10.1007/978-3-642-59223-2>, section 3.3, for the derivative control points.
//!
//! Cost: O(degree) to read an end and a few dozen arithmetic operations to take its derivatives.

extern crate alloc;

use alloc::vec::Vec;

use geom_num::real::Real;
use geom_num::scalar::Scalar;

use crate::equation::{Assembly, Sense, curvature, curvature_rate, jet1, jet2, jet3};
use geom_run::WorkMeter;

use crate::model::{ConstraintKind, ItemId, Sketch, SketchError, Smoothness, SplineEnd};
use crate::solve::{Solved, solve};

/// A spline end's first control points and derivative factors, outward from the end.
///
/// The control points are the first `level` + 1 counted from the end. The factors are those of the
/// derivative control points, level by level: for the k-th derivative's control point i, (degree −
/// k + 1) / (the knot span it differences over).
///
/// # Errors
///
/// [`SketchError::WrongKind`] for a spline that is rational, not clamped, too short for `level` or
/// whose knots repeat inside a span a factor divides by; [`SketchError::OutOfMemory`].
pub(crate) fn end_layout(
    sketch: &Sketch,
    spline: ItemId,
    end: SplineEnd,
    level: usize,
) -> Result<(Vec<ItemId>, Vec<f64>), SketchError> {
    let (degree, points, knots) = sketch.spline_jets(spline, level)?;
    let mut ids: Vec<ItemId> = Vec::new();
    let mut ks: Vec<f64> = Vec::new();
    ids.try_reserve(points.len())
        .map_err(|_| SketchError::OutOfMemory)?;
    ks.try_reserve(knots.len())
        .map_err(|_| SketchError::OutOfMemory)?;
    match end {
        SplineEnd::Start => {
            ids.extend_from_slice(points);
            ks.extend_from_slice(knots);
        }
        SplineEnd::Finish => {
            ids.extend(points.iter().rev().copied());
            ks.extend(knots.iter().rev().map(|k| -k));
        }
    }
    ids.truncate(level.saturating_add(1));
    let p = f64::from(u32::try_from(degree).map_err(|_| SketchError::Internal)?);
    let mut factors: Vec<f64> = Vec::new();
    for k in 1..=level {
        let lowered = f64::from(u32::try_from(k).map_err(|_| SketchError::Internal)?);
        for at in 0..=level.saturating_sub(k) {
            let far = at
                .checked_add(degree)
                .and_then(|j| j.checked_add(1))
                .and_then(|j| ks.get(j))
                .ok_or(SketchError::Internal)?;
            let near = at
                .checked_add(k)
                .and_then(|j| ks.get(j))
                .ok_or(SketchError::Internal)?;
            let span = far - near;
            if span <= 0.0 {
                return Err(SketchError::WrongKind(spline));
            }
            factors
                .try_reserve(1)
                .map_err(|_| SketchError::OutOfMemory)?;
            factors.push((p - lowered + 1.0) / span);
        }
    }
    Ok((ids, factors))
}

/// `values` as an array of `N` reals.
fn reals<const N: usize>(values: &[f64]) -> Result<[Real; N], SketchError> {
    let mut list: Vec<Real> = Vec::new();
    list.try_reserve(values.len())
        .map_err(|_| SketchError::OutOfMemory)?;
    for v in values {
        list.push(Real::new(*v)?);
    }
    list.try_into().map_err(|_| SketchError::Internal)
}

/// The derivatives of a spline end, outward: the first `level` of them (1, 2 or 3), the rest zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jets {
    /// The first derivative: the tangent, as fast as the spline's parameter runs.
    pub first: [f64; 2],
    /// The second derivative, or zero below level 2.
    pub second: [f64; 2],
    /// The third derivative, or zero below level 3.
    pub third: [f64; 2],
}

/// The first `level` derivatives of the spline `spline` at `end`, outward, at the sketch's values.
///
/// # Errors
///
/// [`SketchError::WrongKind`] as [`end_layout`], and for a `level` that is not 1, 2 or 3; what the
/// arithmetic refuses.
pub fn end_jets(
    sketch: &Sketch,
    spline: ItemId,
    end: SplineEnd,
    level: usize,
) -> Result<Jets, SketchError> {
    if sketch.spline_is_rational(spline)? {
        return rational_jets(sketch, spline, end, level);
    }
    let (ids, factors) = end_layout(sketch, spline, end, level)?;
    let mut values: Vec<f64> = Vec::new();
    for id in &ids {
        let [x, y] = sketch.point(*id)?;
        values.push(x);
        values.push(y);
    }
    values.extend_from_slice(&factors);
    let real = |v: [Real; 2]| [v[0].get(), v[1].get()];
    let zero = [0.0, 0.0];
    Ok(match level {
        1 => Jets {
            first: real(jet1(reals::<5>(&values)?)?),
            second: zero,
            third: zero,
        },
        2 => {
            let (d1, d2) = jet2(reals::<9>(&values)?)?;
            Jets {
                first: real(d1),
                second: real(d2),
                third: zero,
            }
        }
        3 => {
            let [d1, d2, d3] = jet3(reals::<14>(&values)?)?;
            Jets {
                first: real(d1),
                second: real(d2),
                third: real(d3),
            }
        }
        _ => return Err(SketchError::WrongKind(spline)),
    })
}

/// The derivatives of a rational spline's end: sums of its first control points times the
/// coefficients of [`crate::linear::end_coefficients`], each term and sum on [`Real`].
fn rational_jets(
    sketch: &Sketch,
    spline: ItemId,
    end: SplineEnd,
    level: usize,
) -> Result<Jets, SketchError> {
    if !(1..=3).contains(&level) {
        return Err(SketchError::WrongKind(spline));
    }
    let (ids, coefficients) = crate::linear::end_coefficients(sketch, spline, end, level)?;
    let columns = ids.len();
    let mut out = [[0.0_f64; 2]; 3];
    for (k, jet) in out.iter_mut().enumerate().take(level) {
        let (mut x, mut y) = (Real::ZERO, Real::ZERO);
        for (i, id) in ids.iter().enumerate() {
            let at = k
                .checked_mul(columns)
                .and_then(|r| r.checked_add(i))
                .ok_or(SketchError::Internal)?;
            let c = Real::new(*coefficients.get(at).ok_or(SketchError::Internal)?)?;
            let [px, py] = sketch.point(*id)?;
            x = x.add(c.mul(Real::new(px)?)?)?;
            y = y.add(c.mul(Real::new(py)?)?)?;
        }
        *jet = [x.get(), y.get()];
    }
    let [first, second, third] = out;
    Ok(Jets {
        first,
        second,
        third,
    })
}

/// How smooth a join of two spline ends is, measured: each field is zero for a join that holds
/// that order, and `None` for an order not asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Continuity {
    /// The angle between the first end's tangent and the reverse of the second's, in half-turns,
    /// from −1 to 1: zero for a tangent join.
    pub tangent: f64,
    /// The sum of the two outward curvatures, in one over a length: zero for equal curvature.
    pub curvature: Option<f64>,
    /// The difference of the outward rates of change of curvature along the curve, in one over a
    /// length squared: zero for G3.
    pub rate: Option<f64>,
}

/// Measures the join of the ends `a_end` of spline `a` and `b_end` of spline `b` up to `order`, at
/// the sketch's values. The ends are taken to be at one point; a gap between them is a coincidence's
/// to report.
///
/// # Errors
///
/// As [`end_jets`].
pub fn continuity(
    sketch: &Sketch,
    (a, a_end): (ItemId, SplineEnd),
    (b, b_end): (ItemId, SplineEnd),
    order: Smoothness,
) -> Result<Continuity, SketchError> {
    let level = order.level();
    let (ja, jb) = (
        end_jets(sketch, a, a_end, level)?,
        end_jets(sketch, b, b_end, level)?,
    );
    continuity_of(ja, jb, level)
}

/// The continuity of two outward ends from their jets, up to `level`.
fn continuity_of(ja: Jets, jb: Jets, level: usize) -> Result<Continuity, SketchError> {
    let back = Real::new(-1.0)?;
    let (a1, b1) = (
        [Real::new(ja.first[0])?, Real::new(ja.first[1])?],
        [Real::new(jb.first[0])?, Real::new(jb.first[1])?],
    );
    let against = [b1[0].mul(back)?, b1[1].mul(back)?];
    let turned = (a1[0].mul(against[1])?.sub(a1[1].mul(against[0])?)?)
        .atan2(a1[0].mul(against[0])?.add(a1[1].mul(against[1])?)?)?;
    let tangent = turned.div(Real::PI)?.get();
    let pair = |v: [f64; 2]| -> Result<[Real; 2], SketchError> {
        Ok([Real::new(v[0])?, Real::new(v[1])?])
    };
    let curvature_gap = if level >= 2 {
        let ka = curvature(pair(ja.first)?, pair(ja.second)?)?;
        let kb = curvature(pair(jb.first)?, pair(jb.second)?)?;
        Some(ka.add(kb)?.get())
    } else {
        None
    };
    let rate_gap = if level >= 3 {
        let ra = curvature_rate(pair(ja.first)?, pair(ja.second)?, pair(ja.third)?)?;
        let rb = curvature_rate(pair(jb.first)?, pair(jb.second)?, pair(jb.third)?)?;
        Some(ra.sub(rb)?.get())
    } else {
        None
    };
    Ok(Continuity {
        tangent,
        curvature: curvature_gap,
        rate: rate_gap,
    })
}

/// The signed curvature of a plane curve with first and second derivatives `first` and `second`.
///
/// # Errors
///
/// What the arithmetic refuses, such as a zero speed.
pub fn curvature_of(first: [f64; 2], second: [f64; 2]) -> Result<f64, SketchError> {
    let pair = |v: [f64; 2]| -> Result<[Real; 2], SketchError> {
        Ok([Real::new(v[0])?, Real::new(v[1])?])
    };
    Ok(curvature(pair(first)?, pair(second)?)?.get())
}

/// The rate of change of curvature along a plane curve with first, second and third derivatives
/// `first`, `second` and `third`.
///
/// # Errors
///
/// What the arithmetic refuses, such as a zero speed.
pub fn curvature_rate_of(
    first: [f64; 2],
    second: [f64; 2],
    third: [f64; 2],
) -> Result<f64, SketchError> {
    let pair = |v: [f64; 2]| -> Result<[Real; 2], SketchError> {
        Ok([Real::new(v[0])?, Real::new(v[1])?])
    };
    Ok(curvature_rate(pair(first)?, pair(second)?, pair(third)?)?.get())
}

/// One join of a sketch and how smooth it is as the sketch stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Joined {
    /// The constraint that states the join.
    pub constraint: ItemId,
    /// The order it states.
    pub order: Smoothness,
    /// What the three gaps are, up to that order.
    pub continuity: Continuity,
}

/// The outward jets of the line or arc `other` at `other_end`, as a curve of unit speed: the
/// tangent, the curvature times the left normal, and the derivative of that along the curve. A line
/// has no curvature; an arc runs anticlockwise from its start, so its outward direction is
/// anticlockwise at its start and clockwise at its end, and its curvature is plus or minus one over
/// its radius.
fn round_jets(sketch: &Sketch, other: ItemId, other_end: SplineEnd) -> Result<Jets, SketchError> {
    let pair = |v: [f64; 2]| -> Result<[Real; 2], SketchError> {
        Ok([Real::new(v[0])?, Real::new(v[1])?])
    };
    let zero = [0.0, 0.0];
    if let Ok([a, b]) = sketch.line(other) {
        let (p, q) = (pair(sketch.point(a)?)?, pair(sketch.point(b)?)?);
        let d = [q[0].sub(p[0])?, q[1].sub(p[1])?];
        let speed = d[0].mul(d[0])?.add(d[1].mul(d[1])?)?.sqrt()?;
        let first = [d[0].div(speed)?.get(), d[1].div(speed)?.get()];
        return Ok(Jets {
            first,
            second: zero,
            third: zero,
        });
    }
    let [centre, start, finish] = sketch.arc(other)?;
    let radius = Real::new(sketch.radius(other)?)?;
    let at = if other_end == SplineEnd::Start {
        start
    } else {
        finish
    };
    let (c, p) = (pair(sketch.point(centre)?)?, pair(sketch.point(at)?)?);
    let r = [p[0].sub(c[0])?, p[1].sub(c[1])?];
    let length = r[0].mul(r[0])?.add(r[1].mul(r[1])?)?.sqrt()?;
    // The anticlockwise tangent is the radius turned a quarter; the clockwise one is its reverse.
    let sign = Real::new(if other_end == SplineEnd::Start {
        1.0
    } else {
        -1.0
    })?;
    let t = [
        sign.mul(Real::new(0.0)?.sub(r[1])?)?.div(length)?,
        sign.mul(r[0])?.div(length)?,
    ];
    let kappa = sign.div(radius)?;
    let normal = [Real::new(0.0)?.sub(t[1])?, t[0]];
    let square = kappa.mul(kappa)?;
    Ok(Jets {
        first: [t[0].get(), t[1].get()],
        second: [kappa.mul(normal[0])?.get(), kappa.mul(normal[1])?.get()],
        third: [
            Real::new(0.0)?.sub(square.mul(t[0])?)?.get(),
            Real::new(0.0)?.sub(square.mul(t[1])?)?.get(),
        ],
    })
}

/// The jets of a line turned to leave the join against the spline's tangent `first` (a line has no
/// direction of its own at the join; its sense is the other way from the spline's).
fn line_against(mut line: Jets, first: [f64; 2]) -> Jets {
    if line.first[0] * first[0] + line.first[1] * first[1] > 0.0 {
        line.first = [-line.first[0], -line.first[1]];
    }
    line
}

/// Measures every spline join of `sketch` as it stands: each [`ConstraintKind::SplineJoin`], each
/// [`ConstraintKind::SplineTangentLine`] (at G1) and each [`ConstraintKind::SplineAgainst`], in the
/// constraints' order. On a solved sketch it is the continuity of the solved curves.
///
/// # Errors
///
/// As [`end_jets`]; [`SketchError::OutOfMemory`].
pub fn measure_joins(sketch: &Sketch) -> Result<Vec<Joined>, SketchError> {
    let mut out = Vec::new();
    for c in sketch.constraints() {
        let (order, continuity) = match c.kind {
            ConstraintKind::SplineJoin {
                a,
                a_end,
                b,
                b_end,
                order,
            } => (order, continuity(sketch, (a, a_end), (b, b_end), order)?),
            ConstraintKind::SplineTangentLine { spline, end, line } => (Smoothness::Tangent, {
                let ours = end_jets(sketch, spline, end, 1)?;
                let theirs = line_against(round_jets(sketch, line, SplineEnd::Start)?, ours.first);
                continuity_of(ours, theirs, 1)?
            }),
            ConstraintKind::SplineAgainst {
                spline,
                end,
                other,
                other_end,
                order,
            } => (order, {
                let ours = end_jets(sketch, spline, end, order.level())?;
                let mut theirs = round_jets(sketch, other, other_end)?;
                if sketch.line(other).is_ok() {
                    theirs = line_against(theirs, ours.first);
                }
                continuity_of(ours, theirs, order.level())?
            }),
            _ => continue,
        };
        out.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
        out.push(Joined {
            constraint: c.id,
            order,
            continuity,
        });
    }
    Ok(out)
}

/// Solves `sketch` and measures its spline joins at the solution: the continuity of the solved
/// curves (chapter 15.3, "Smoothness").
///
/// # Errors
///
/// What [`crate::solve::solve`] and [`measure_joins`] refuse.
pub fn solve_and_measure(
    sketch: &Sketch,
    meter: &mut WorkMeter<'_>,
) -> Result<(Solved, Vec<Joined>), SketchError> {
    let solved = solve(sketch, meter)?;
    let joins = measure_joins(&solved.sketch)?;
    Ok((solved, joins))
}

/// Which way round the join `join` is, as the sketch stands: the sense of its tangent root, or
/// `None` where interval arithmetic cannot certify one (the tangents are at a right angle, say).
///
/// - [`ConstraintKind::SplineJoin`]: [`Sense::Positive`] when the two curves leave the join in
///   opposite directions, a continuation, and [`Sense::Negative`] when they leave the same way, a
///   cusp. The G1 row is zero in both, so only this tells them apart.
/// - [`ConstraintKind::SplineTangentLine`] and [`ConstraintKind::SplineAgainst`] against a line:
///   [`Sense::Positive`] when the spline leaves along the line's direction.
/// - [`ConstraintKind::SplineAgainst`] against an arc: [`Sense::Positive`] when the spline leaves
///   anticlockwise about the arc's centre.
///
/// A path (`crate::solve::continue_to`) holds the sense it starts with.
///
/// # Errors
///
/// [`SketchError::Unknown`] if no constraint has that id; what the assembly refuses.
pub fn join_sense(sketch: &Sketch, join: ItemId) -> Result<Option<Sense>, SketchError> {
    let assembly = Assembly::new(sketch)?;
    let x = assembly.start()?;
    assembly.sense_of_id(join, &x)
}
