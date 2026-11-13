//! The spline rules of piece G220 that read a spline through fixed linear maps of its control points:
//! joins and ends against lines and arcs for rational splines and to every order, arc length by
//! quadrature, and a point pinned to a parameter.
//!
//! What it does: a spline's weights and knots are fixed in a solve, so each derivative at an end,
//! and the point and speed at a parameter, is a sum of control points times numbers that do not
//! change ([`crate::linear`]). The rules here take those numbers as constants beside the control
//! points, so one residual serves every degree, every weight and every knot vector.
//!
//! - **Ends** ([`LinJoin1`] to [`LinRound3`]): the smooth rules of `equation`, written over the
//!   linear layout: for level L, the first L + 1 control points of an end, then L·(L + 1)
//!   coefficients (row k the coefficients of the k-th derivative).
//! - **Length and through** (`Assembly::run_length`, `Assembly::run_through`): rules with as many
//!   inputs as the spline has control points. A fixed-width [`Residual`] cannot hold them, so they
//!   run here: the nonlinear part, the speed √(dx² + dy²) of a node's velocity, is differentiated
//!   by the forward mode (`geom_num::dual`), and the velocity's own dependence on the control
//!   points is the coefficient, exact because it is linear. The partials are then the chain rule
//!   through that map, in a fixed order. The same sums are written once more over `Scalar`
//!   (`length_residual`, `through_residuals`), so piece G226's check encloses them in intervals.
//!
//! Guarantees: as `equation`: deterministic, a NaN anywhere is an error, no panics, no recursion.
//!
//! Method: section 15.4, `projects/engines/geometry/architecture/15-sketches.md`, and the
//! arc-length quadrature of [`crate::length`].
//!
//! Cost: linear in the inputs of the row.

extern crate alloc;

use alloc::vec::Vec;

use geom_num::NumError;
use geom_num::dual::Dual;
use geom_num::interval::Interval;
use geom_num::real::Real;
use geom_num::scalar::Scalar;

use super::{
    Assembly, Residual, Row, Singular, SketchError, Slot, Wide, cross, curvature, curvature_rate,
    dot, minus, sine,
};

/// The numbers of one spline end in the linear layout: its first `level` + 1 control points
/// (two numbers each), then the coefficients of its derivatives 1 to `level` on them.
#[must_use]
pub(super) const fn block(level: usize) -> usize {
    level
        .saturating_add(1)
        .saturating_mul(level.saturating_add(2))
}

/// The first `L` derivatives of an end laid out as [`block`].
fn lin_jets<S: Scalar, const L: usize>(x: &[S]) -> Result<[[S; 2]; L], NumError> {
    let columns = L.saturating_add(1);
    let (points, coefficients) = x
        .split_at_checked(columns.saturating_mul(2))
        .ok_or(NumError::Degenerate)?;
    let zero = S::constant(Real::ZERO);
    let mut out = [[zero; 2]; L];
    for (k, jet) in out.iter_mut().enumerate() {
        for i in 0..columns {
            let c = *coefficients
                .get(k.saturating_mul(columns).saturating_add(i))
                .ok_or(NumError::Degenerate)?;
            let px = *points
                .get(i.saturating_mul(2))
                .ok_or(NumError::Degenerate)?;
            let py = *points
                .get(i.saturating_mul(2).saturating_add(1))
                .ok_or(NumError::Degenerate)?;
            jet[0] = jet[0].add(c.mul(px)?)?;
            jet[1] = jet[1].add(c.mul(py)?)?;
        }
    }
    Ok(out)
}

/// Whether the first derivative of an end in the linear layout is zero, on doubles. Exactly, at a
/// start and at an answer alike: a derivative is a length per unit of the parameter, not a length,
/// so the solver's tolerance in steps is no margin for it.
fn lin_flat(x: &[f64], level: usize) -> bool {
    let columns = level.saturating_add(1);
    let cut = columns.saturating_mul(2);
    let (Some(points), Some(rest)) = (x.get(..cut), x.get(cut..)) else {
        return true;
    };
    let (mut dx, mut dy) = (0.0, 0.0);
    for i in 0..columns {
        let c = rest.get(i).copied().unwrap_or(0.0);
        let at = i.saturating_mul(2);
        dx += c * points.get(at).copied().unwrap_or(0.0);
        dy += c * points.get(at.saturating_add(1)).copied().unwrap_or(0.0);
    }
    dx == 0.0 && dy == 0.0
}

/// The rows of a join of two outward ends, scaled by a length: the sine of the angle between the
/// tangents, the sum of the curvatures, the difference of the rates of change of curvature.
fn join_rows<S: Scalar, const L: usize>(x: &[S]) -> Result<[S; L], NumError> {
    let h = block(L);
    let (a, rest) = x.split_at_checked(h).ok_or(NumError::Degenerate)?;
    let (b, rest) = rest.split_at_checked(h).ok_or(NumError::Degenerate)?;
    let l = *rest.first().ok_or(NumError::Degenerate)?;
    let (ja, jb) = (lin_jets::<S, L>(a)?, lin_jets::<S, L>(b)?);
    let mut out = [S::constant(Real::ZERO); L];
    let pair = |k: usize| -> Result<([S; 2], [S; 2]), NumError> {
        Ok((
            *ja.get(k).ok_or(NumError::Degenerate)?,
            *jb.get(k).ok_or(NumError::Degenerate)?,
        ))
    };
    let (a1, b1) = pair(0)?;
    *out.get_mut(0).ok_or(NumError::Degenerate)? = sine(a1, b1)?.mul(l)?;
    if L >= 2 {
        let (a2, b2) = pair(1)?;
        *out.get_mut(1).ok_or(NumError::Degenerate)? = curvature(a1, a2)?
            .add(curvature(b1, b2)?)?
            .mul(l.square()?)?;
        if L >= 3 {
            let (a3, b3) = pair(2)?;
            *out.get_mut(2).ok_or(NumError::Degenerate)? = curvature_rate(a1, a2, a3)?
                .sub(curvature_rate(b1, b2, b3)?)?
                .mul(l.square()?.mul(l)?)?;
        }
    }
    Ok(out)
}

/// The rows of a spline end against a line or an arc: `tangent` of the first derivative, then the
/// end's curvature plus `curve` (the other curve's outward curvature), then the rate of change of
/// curvature (zero for both), scaled by powers of a length.
fn against_rows<S: Scalar, const L: usize>(
    jets: &[[S; 2]; L],
    tangent: impl FnOnce([S; 2]) -> Result<S, NumError>,
    curve: S,
    l: S,
) -> Result<[S; L], NumError> {
    let mut out = [S::constant(Real::ZERO); L];
    let d1 = *jets.first().ok_or(NumError::Degenerate)?;
    *out.get_mut(0).ok_or(NumError::Degenerate)? = tangent(d1)?.mul(l)?;
    if L >= 2 {
        let d2 = *jets.get(1).ok_or(NumError::Degenerate)?;
        *out.get_mut(1).ok_or(NumError::Degenerate)? =
            curvature(d1, d2)?.add(curve)?.mul(l.square()?)?;
        if L >= 3 {
            let d3 = *jets.get(2).ok_or(NumError::Degenerate)?;
            *out.get_mut(2).ok_or(NumError::Degenerate)? =
                curvature_rate(d1, d2, d3)?.mul(l.square()?.mul(l)?)?;
        }
    }
    Ok(out)
}

/// The first five numbers of `x`.
fn five<S: Scalar>(x: &[S]) -> Result<[S; 5], NumError> {
    let at = |i: usize| x.get(i).copied().ok_or(NumError::Degenerate);
    Ok([at(0)?, at(1)?, at(2)?, at(3)?, at(4)?])
}

/// A spline end smooth with a line (inputs: the end, the line's two points, a length).
fn along_rows<S: Scalar, const L: usize>(x: &[S]) -> Result<[S; L], NumError> {
    let (end, rest) = x.split_at_checked(block(L)).ok_or(NumError::Degenerate)?;
    let [ax, ay, bx, by, l] = five(rest)?;
    let jets = lin_jets::<S, L>(end)?;
    let direction = minus([bx, by], [ax, ay])?;
    against_rows(
        &jets,
        |d1: [S; 2]| -> Result<S, NumError> { sine(d1, direction) },
        S::constant(Real::ZERO),
        l,
    )
}

/// A spline end smooth with an arc (inputs: the end, the arc's centre, its radius, the sense of
/// the join's end of the arc, a length): the tangent is across the radius from the centre to the
/// end's first point, and the arc's outward curvature is its sense over its radius.
fn round_rows<S: Scalar, const L: usize>(x: &[S]) -> Result<[S; L], NumError> {
    let (end, rest) = x.split_at_checked(block(L)).ok_or(NumError::Degenerate)?;
    let [cx, cy, r, sense, l] = five(rest)?;
    let p = [
        *end.first().ok_or(NumError::Degenerate)?,
        *end.get(1).ok_or(NumError::Degenerate)?,
    ];
    let jets = lin_jets::<S, L>(end)?;
    let radius = minus(p, [cx, cy])?;
    against_rows(
        &jets,
        |d1: [S; 2]| -> Result<S, NumError> {
            dot(radius, d1)?.div(dot(radius, radius)?.mul(dot(d1, d1)?)?.sqrt()?)
        },
        sense.div(r)?,
        l,
    )
}

/// The branch quantity of a join of two outward ends: minus the dot product of their tangents.
/// Above zero the curves leave the join in opposite directions, a smooth continuation; below zero
/// they leave the same way, a cusp. The G1 row is zero for both, so a path keeps the sign.
fn join_branch<S: Scalar, const L: usize>(x: &[S]) -> Result<S, NumError> {
    let h = block(L);
    let (a, rest) = x.split_at_checked(h).ok_or(NumError::Degenerate)?;
    let (b, _) = rest.split_at_checked(h).ok_or(NumError::Degenerate)?;
    let (ja, jb) = (lin_jets::<S, L>(a)?, lin_jets::<S, L>(b)?);
    let a1 = *ja.first().ok_or(NumError::Degenerate)?;
    let b1 = *jb.first().ok_or(NumError::Degenerate)?;
    S::constant(Real::ZERO).sub(dot(a1, b1)?)
}

/// The branch quantity of a spline end against a line: the dot product of its tangent and the
/// line's direction. The G1 row is zero whichever way the spline leaves along the line.
fn along_branch<S: Scalar, const L: usize>(x: &[S]) -> Result<S, NumError> {
    let (end, rest) = x.split_at_checked(block(L)).ok_or(NumError::Degenerate)?;
    let [ax, ay, bx, by, _] = five(rest)?;
    let jets = lin_jets::<S, L>(end)?;
    let d1 = *jets.first().ok_or(NumError::Degenerate)?;
    dot(d1, minus([bx, by], [ax, ay])?)
}

/// The branch quantity of a spline end against an arc: the cross product of the radius from the
/// centre and the tangent, positive when the spline leaves anticlockwise about the centre. The G1
/// row is zero for both ways round.
fn round_branch<S: Scalar, const L: usize>(x: &[S]) -> Result<S, NumError> {
    let (end, rest) = x.split_at_checked(block(L)).ok_or(NumError::Degenerate)?;
    let [cx, cy, _, _, _] = five(rest)?;
    let p = [
        *end.first().ok_or(NumError::Degenerate)?,
        *end.get(1).ok_or(NumError::Degenerate)?,
    ];
    let jets = lin_jets::<S, L>(end)?;
    let d1 = *jets.first().ok_or(NumError::Degenerate)?;
    cross(minus(p, [cx, cy])?, d1)
}

/// Whether either end of a join in the linear layout has no tangent.
fn flat_join(x: &[f64], level: usize) -> bool {
    let h = block(level);
    x.get(..h).is_none_or(|a| lin_flat(a, level))
        || x.get(h..h.saturating_mul(2))
            .is_none_or(|b| lin_flat(b, level))
}

/// Whether the end of a rule in the linear layout has no tangent.
fn flat_one(x: &[f64], level: usize) -> bool {
    x.get(..block(level)).is_none_or(|a| lin_flat(a, level))
}

#[doc = "G1 join of two spline ends in the linear layout."]
pub struct LinJoin1;

impl Residual<13, 1> for LinJoin1 {
    fn residuals<S: Scalar>(&self, x: [S; 13]) -> Result<[S; 1], NumError> {
        join_rows::<S, 1>(&x)
    }

    fn singular(&self, x: &[f64; 13], _within: f64) -> Option<Singular> {
        flat_join(x, 1).then_some(Singular::ZeroLengthLine)
    }

    fn branch<S: Scalar>(&self, x: [S; 13]) -> Result<Option<S>, NumError> {
        Ok(Some(join_branch::<S, 1>(&x)?))
    }
}

#[doc = "G2 join of two spline ends in the linear layout."]
pub struct LinJoin2;

impl Residual<25, 2> for LinJoin2 {
    fn residuals<S: Scalar>(&self, x: [S; 25]) -> Result<[S; 2], NumError> {
        join_rows::<S, 2>(&x)
    }

    fn singular(&self, x: &[f64; 25], _within: f64) -> Option<Singular> {
        flat_join(x, 2).then_some(Singular::ZeroLengthLine)
    }

    fn branch<S: Scalar>(&self, x: [S; 25]) -> Result<Option<S>, NumError> {
        Ok(Some(join_branch::<S, 2>(&x)?))
    }
}

#[doc = "G3 join of two spline ends in the linear layout."]
pub struct LinJoin3;

impl Residual<41, 3> for LinJoin3 {
    fn residuals<S: Scalar>(&self, x: [S; 41]) -> Result<[S; 3], NumError> {
        join_rows::<S, 3>(&x)
    }

    fn singular(&self, x: &[f64; 41], _within: f64) -> Option<Singular> {
        flat_join(x, 3).then_some(Singular::ZeroLengthLine)
    }

    fn branch<S: Scalar>(&self, x: [S; 41]) -> Result<Option<S>, NumError> {
        Ok(Some(join_branch::<S, 3>(&x)?))
    }
}

#[doc = "A spline end tangent to a line."]
pub struct LinAlong1;

impl Residual<11, 1> for LinAlong1 {
    fn residuals<S: Scalar>(&self, x: [S; 11]) -> Result<[S; 1], NumError> {
        along_rows::<S, 1>(&x)
    }

    fn singular(&self, x: &[f64; 11], _within: f64) -> Option<Singular> {
        flat_one(x, 1).then_some(Singular::ZeroLengthLine)
    }

    fn branch<S: Scalar>(&self, x: [S; 11]) -> Result<Option<S>, NumError> {
        Ok(Some(along_branch::<S, 1>(&x)?))
    }
}

#[doc = "A spline end G2 with a line."]
pub struct LinAlong2;

impl Residual<17, 2> for LinAlong2 {
    fn residuals<S: Scalar>(&self, x: [S; 17]) -> Result<[S; 2], NumError> {
        along_rows::<S, 2>(&x)
    }

    fn singular(&self, x: &[f64; 17], _within: f64) -> Option<Singular> {
        flat_one(x, 2).then_some(Singular::ZeroLengthLine)
    }

    fn branch<S: Scalar>(&self, x: [S; 17]) -> Result<Option<S>, NumError> {
        Ok(Some(along_branch::<S, 2>(&x)?))
    }
}

#[doc = "A spline end G3 with a line."]
pub struct LinAlong3;

impl Residual<25, 3> for LinAlong3 {
    fn residuals<S: Scalar>(&self, x: [S; 25]) -> Result<[S; 3], NumError> {
        along_rows::<S, 3>(&x)
    }

    fn singular(&self, x: &[f64; 25], _within: f64) -> Option<Singular> {
        flat_one(x, 3).then_some(Singular::ZeroLengthLine)
    }

    fn branch<S: Scalar>(&self, x: [S; 25]) -> Result<Option<S>, NumError> {
        Ok(Some(along_branch::<S, 3>(&x)?))
    }
}

#[doc = "A spline end tangent to an arc."]
pub struct LinRound1;

impl Residual<11, 1> for LinRound1 {
    fn residuals<S: Scalar>(&self, x: [S; 11]) -> Result<[S; 1], NumError> {
        round_rows::<S, 1>(&x)
    }

    fn singular(&self, x: &[f64; 11], _within: f64) -> Option<Singular> {
        flat_one(x, 1).then_some(Singular::ZeroLengthLine)
    }

    fn branch<S: Scalar>(&self, x: [S; 11]) -> Result<Option<S>, NumError> {
        Ok(Some(round_branch::<S, 1>(&x)?))
    }
}

#[doc = "A spline end G2 with an arc."]
pub struct LinRound2;

impl Residual<17, 2> for LinRound2 {
    fn residuals<S: Scalar>(&self, x: [S; 17]) -> Result<[S; 2], NumError> {
        round_rows::<S, 2>(&x)
    }

    fn singular(&self, x: &[f64; 17], _within: f64) -> Option<Singular> {
        flat_one(x, 2).then_some(Singular::ZeroLengthLine)
    }

    fn branch<S: Scalar>(&self, x: [S; 17]) -> Result<Option<S>, NumError> {
        Ok(Some(round_branch::<S, 2>(&x)?))
    }
}

#[doc = "A spline end G3 with an arc."]
pub struct LinRound3;

impl Residual<25, 3> for LinRound3 {
    fn residuals<S: Scalar>(&self, x: [S; 25]) -> Result<[S; 3], NumError> {
        round_rows::<S, 3>(&x)
    }

    fn singular(&self, x: &[f64; 25], _within: f64) -> Option<Singular> {
        flat_one(x, 3).then_some(Singular::ZeroLengthLine)
    }

    fn branch<S: Scalar>(&self, x: [S; 25]) -> Result<Option<S>, NumError> {
        Ok(Some(round_branch::<S, 3>(&x)?))
    }
}

/// A spline's arc length minus the stated length, over any [`Scalar`], in the layout
/// `Assembly::run_length` reads: the 2n coordinates of the control points, the length, then for
/// each node its weight and the n coefficients of its velocity. The same sums in the same order as
/// `run_length`'s value, so an enclosure over intervals holds what the solver stores.
///
/// # Errors
///
/// What the arithmetic refuses (a square root of a range below zero); [`NumError::Degenerate`] for
/// inputs of another shape.
pub(super) fn length_residual<S: Scalar>(values: &[S], n: usize) -> Result<S, NumError> {
    let head = n.checked_mul(2).ok_or(NumError::Degenerate)?;
    let width = n.checked_add(1).ok_or(NumError::Degenerate)?;
    let (points, rest) = values.split_at_checked(head).ok_or(NumError::Degenerate)?;
    let (stated, nodes) = rest.split_first().ok_or(NumError::Degenerate)?;
    let zero = S::constant(Real::ZERO);
    let mut total = zero;
    for node in nodes.chunks_exact(width) {
        let (weight, coefficients) = node.split_first().ok_or(NumError::Degenerate)?;
        let (mut dx, mut dy) = (zero, zero);
        for (c, [px, py]) in coefficients.iter().zip(points.as_chunks::<2>().0) {
            dx = dx.add(c.mul(*px)?)?;
            dy = dy.add(c.mul(*py)?)?;
        }
        let speed = dx.mul(dx)?.add(dy.mul(dy)?)?.sqrt()?;
        total = total.add(weight.mul(speed)?)?;
    }
    total.sub(*stated)
}

/// A spline's point at a parameter minus a point, over any [`Scalar`], in the layout
/// `Assembly::run_through` reads: the 2n coordinates of the control points, the point's two, then
/// the n coefficients of the curve at the parameter. The same sums in the same order as
/// `run_through`'s values.
///
/// # Errors
///
/// What the arithmetic refuses; [`NumError::Degenerate`] for inputs of another shape.
pub(super) fn through_residuals<S: Scalar>(values: &[S], n: usize) -> Result<[S; 2], NumError> {
    let head = n.checked_mul(2).ok_or(NumError::Degenerate)?;
    let (points, rest) = values.split_at_checked(head).ok_or(NumError::Degenerate)?;
    let (q, coefficients) = rest.split_at_checked(2).ok_or(NumError::Degenerate)?;
    let zero = S::constant(Real::ZERO);
    let mut out = [zero; 2];
    for (e, slot) in out.iter_mut().enumerate() {
        let mut sum = zero;
        for (c, p) in coefficients.iter().zip(points.as_chunks::<2>().0) {
            sum = sum.add(c.mul(*p.get(e).ok_or(NumError::Degenerate)?)?)?;
        }
        *slot = sum.sub(*q.get(e).ok_or(NumError::Degenerate)?)?;
    }
    Ok(out)
}

/// The residuals of a rule as wide as its spline, over any [`Scalar`]: one for a length, two for a
/// point at a parameter.
///
/// # Errors
///
/// What the arithmetic refuses; [`SketchError::OutOfMemory`].
pub(super) fn wide_residuals<S: Scalar>(
    wide: Wide,
    values: &[S],
    n: usize,
) -> Result<Vec<S>, SketchError> {
    let mut out = Vec::new();
    out.try_reserve_exact(2)
        .map_err(|_| SketchError::OutOfMemory)?;
    match wide {
        Wide::Length => out.push(length_residual(values, n)?),
        Wide::Through => out.extend(through_residuals(values, n)?),
    }
    Ok(out)
}

/// The variables of a wide row that `take` accepts, each once, in the order they first come.
fn variables_of(list: &[Slot], take: impl Fn(usize) -> bool) -> Result<Vec<usize>, SketchError> {
    let mut out: Vec<usize> = Vec::new();
    for slot in list {
        if let Slot::Variable(i) = *slot
            && take(i)
            && !out.contains(&i)
        {
            out.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
            out.push(i);
        }
    }
    Ok(out)
}

/// A wide rule's residuals, then its derivatives: the variable, which residual, and the derivative.
pub(super) type WideJet<S> = (Vec<S>, Vec<(usize, usize, S)>);

/// A wide rule's residuals at `values` (one number per slot of `list`), and their derivatives with
/// respect to each variable `take` accepts, in the forward mode one variable at a time on the same
/// text: for each residual, then each variable in the order they first come, the variable, the
/// residual and the derivative, summed where a variable comes twice. `seeds` are zero and one.
///
/// # Errors
///
/// What the arithmetic refuses; [`SketchError::OutOfMemory`].
pub(super) fn wide_jet<S: Scalar>(
    wide: Wide,
    n: usize,
    (list, values): (&[Slot], &[S]),
    take: impl Fn(usize) -> bool,
    seeds: (S, S),
) -> Result<WideJet<S>, SketchError> {
    let (zero, one) = seeds;
    let residuals = wide_residuals(wide, values, n)?;
    let owners = variables_of(list, take)?;
    let mut lanes: Vec<Vec<S>> = Vec::new();
    lanes
        .try_reserve_exact(owners.len())
        .map_err(|_| SketchError::OutOfMemory)?;
    for &v in &owners {
        let mut duals: Vec<Dual<S, 1>> = Vec::new();
        duals
            .try_reserve_exact(values.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        for (slot, value) in list.iter().zip(values) {
            let unit = if *slot == Slot::Variable(v) {
                one
            } else {
                zero
            };
            duals.push(Dual::new(*value, [unit]));
        }
        let mut lane = Vec::new();
        lane.try_reserve_exact(residuals.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        for r in wide_residuals(wide, &duals, n)? {
            let [d] = r.partials();
            lane.push(d);
        }
        lanes.push(lane);
    }
    let mut partials = Vec::new();
    partials
        .try_reserve_exact(residuals.len().saturating_mul(owners.len()))
        .map_err(|_| SketchError::OutOfMemory)?;
    for e in 0..residuals.len() {
        for (&v, lane) in owners.iter().zip(&lanes) {
            partials.push((v, e, *lane.get(e).ok_or(SketchError::Internal)?));
        }
    }
    Ok((residuals, partials))
}

impl Assembly {
    /// A wide row's slots.
    pub(super) fn row_slots(&self, row: &Row) -> Result<&[Slot], SketchError> {
        self.slots
            .get(row.at..)
            .and_then(|tail| tail.get(..row.len))
            .ok_or(SketchError::Internal)
    }

    /// The inputs of a row, read at `x`.
    fn read_row(&self, row: &Row, x: &[f64]) -> Result<Vec<f64>, SketchError> {
        let list = self
            .slots
            .get(row.at..)
            .and_then(|tail| tail.get(..row.len))
            .ok_or(SketchError::Internal)?;
        let mut values: Vec<f64> = Vec::new();
        values
            .try_reserve_exact(list.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        for slot in list {
            values.push(Assembly::read(*slot, x)?);
        }
        Ok(values)
    }

    /// A wide row's inputs over `held`, one interval per variable: a constant is its own point
    /// (piece G226's enclosures, joined with this module's wide rules).
    pub(super) fn held_row(
        &self,
        row: &Row,
        held: &[Interval],
    ) -> Result<Vec<Interval>, SketchError> {
        let list = self.row_slots(row)?;
        let mut values: Vec<Interval> = Vec::new();
        values
            .try_reserve_exact(list.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        for slot in list {
            values.push(match *slot {
                Slot::Constant(c) => Interval::point(c)?,
                Slot::Variable(i) => *held.get(i).ok_or(SketchError::Internal)?,
            });
        }
        Ok(values)
    }

    /// Stores equation `e` of a row and its partials with respect to the row's variable slots, in
    /// the slots' order, one number per slot (the constants' are skipped).
    fn store(
        &mut self,
        row: &Row,
        e: usize,
        value: Real,
        partials: &[f64],
    ) -> Result<(), SketchError> {
        let at = row.first.checked_add(e).ok_or(SketchError::Internal)?;
        *self.residuals.get_mut(at).ok_or(SketchError::Internal)? = value.get();
        let list = self
            .slots
            .get(row.at..)
            .and_then(|tail| tail.get(..row.len))
            .ok_or(SketchError::Internal)?;
        let variable_slots = list
            .iter()
            .filter(|s| matches!(s, Slot::Variable(_)))
            .count();
        let per_equation = variable_slots;
        let mut stored = self
            .places
            .get(row.from..)
            .and_then(|tail| tail.get(e.checked_mul(per_equation)?..))
            .ok_or(SketchError::Internal)?
            .iter();
        for (slot, partial) in list.iter().zip(partials) {
            if let Slot::Variable(i) = slot {
                let scale = self.variables.get(*i).ok_or(SketchError::Internal)?.scale;
                let spot = *stored.next().ok_or(SketchError::Internal)?;
                self.jacobian.add_at(spot, partial * scale)?;
            }
        }
        Ok(())
    }

    /// A spline's arc length minus the stated length: inputs are the 2n coordinates of the control
    /// points, the length, then for each node its weight and the n coefficients of its velocity.
    pub(super) fn run_length(&mut self, row: &Row, x: &[f64]) -> Result<(), SketchError> {
        let values = self.read_row(row, x)?;
        let n = row.n;
        let head = n.checked_mul(2).ok_or(SketchError::Internal)?;
        let width = n.checked_add(1).ok_or(SketchError::Internal)?;
        let (points, rest) = values.split_at_checked(head).ok_or(SketchError::Internal)?;
        let (stated, nodes) = rest.split_first().ok_or(SketchError::Internal)?;
        let mut total = Real::ZERO;
        let mut partials: Vec<f64> = Vec::new();
        partials
            .try_reserve_exact(values.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        partials.resize(values.len(), 0.0);
        for node in nodes.chunks_exact(width) {
            let (weight, coefficients) = node.split_first().ok_or(SketchError::Internal)?;
            let (mut dx, mut dy) = (Real::ZERO, Real::ZERO);
            for (c, [px, py]) in coefficients.iter().zip(points.as_chunks::<2>().0) {
                let c = Real::new(*c)?;
                dx = dx.add(c.mul(Real::new(*px)?)?)?;
                dy = dy.add(c.mul(Real::new(*py)?)?)?;
            }
            let one = Real::new(1.0)?;
            let ex = Dual::new(dx, [one, Real::ZERO]);
            let ey = Dual::new(dy, [Real::ZERO, one]);
            let speed = ex
                .mul(ex)?
                .add(ey.mul(ey)?)?
                .sqrt()
                .map_err(|_| SketchError::Degenerate(row.id))?;
            let w = Real::new(*weight)?;
            total = total.add(w.mul(speed.value())?)?;
            let [fx, fy] = speed.partials();
            for (i, c) in coefficients.iter().enumerate() {
                let c = Real::new(*c)?;
                let (gx, gy) = (w.mul(c)?.mul(fx)?.get(), w.mul(c)?.mul(fy)?.get());
                let at = i.checked_mul(2).ok_or(SketchError::Internal)?;
                *partials.get_mut(at).ok_or(SketchError::Internal)? += gx;
                *partials
                    .get_mut(at.checked_add(1).ok_or(SketchError::Internal)?)
                    .ok_or(SketchError::Internal)? += gy;
            }
        }
        *partials.get_mut(head).ok_or(SketchError::Internal)? = -1.0;
        let value = total.sub(Real::new(*stated)?)?;
        self.store(row, 0, value, &partials)
    }

    /// A spline's point at a parameter minus a point: inputs are the 2n coordinates of the control
    /// points, the point's two, then the n coefficients of the curve at the parameter.
    pub(super) fn run_through(&mut self, row: &Row, x: &[f64]) -> Result<(), SketchError> {
        let values = self.read_row(row, x)?;
        let n = row.n;
        let head = n.checked_mul(2).ok_or(SketchError::Internal)?;
        let tail = head.checked_add(2).ok_or(SketchError::Internal)?;
        let coefficients = values.get(tail..).ok_or(SketchError::Internal)?;
        for e in 0..2_usize {
            let mut sum = Real::ZERO;
            let mut partials: Vec<f64> = Vec::new();
            partials
                .try_reserve_exact(values.len())
                .map_err(|_| SketchError::OutOfMemory)?;
            partials.resize(values.len(), 0.0);
            for (i, c) in coefficients.iter().enumerate() {
                let at = i
                    .checked_mul(2)
                    .and_then(|k| k.checked_add(e))
                    .ok_or(SketchError::Internal)?;
                let p = *values.get(at).ok_or(SketchError::Internal)?;
                sum = sum.add(Real::new(*c)?.mul(Real::new(p)?)?)?;
                *partials.get_mut(at).ok_or(SketchError::Internal)? = *c;
            }
            let q_at = head.checked_add(e).ok_or(SketchError::Internal)?;
            *partials.get_mut(q_at).ok_or(SketchError::Internal)? = -1.0;
            let q = *values.get(q_at).ok_or(SketchError::Internal)?;
            self.store(row, e, sum.sub(Real::new(q)?)?, &partials)?;
        }
        Ok(())
    }
}
