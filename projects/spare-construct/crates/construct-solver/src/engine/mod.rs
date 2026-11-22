//! The interim solving engine behind [`crate::solve`]. Private to this crate: callers see only
//! the sketch and outcome types, so replacing this module changes nothing they call.
//!
//! Interim: the kernel's `geom-sketch` (pieces G216 to G228, the solver CON-A27-F1-P2 names) will
//! answer behind the same types through `engine-geometry` when it lands, and this module then
//! goes. Until then it follows the kernel's chapter 15 method (projects/engines/geometry/
//! architecture/15-sketches.md), in its simplest form, in our own code:
//!
//! - Variables: x and y of every point that is not fixed, in authoring order. Fixed points are
//!   ground: constants, never moved (15.18).
//! - Scaling (15.4): the block is moved by a whole number of millimetres to sit at the origin and
//!   divided by a power of two at least its size, both exact in floating point.
//! - Residuals (15.3, 15.4): `residual.rs`, derivatives by forward-mode dual numbers.
//! - Steps (15.6): Gauss-Newton, damped as in Levenberg-Marquardt, each the least-change step:
//!   among all least-squares solutions the shortest, so geometry nobody constrained stays where it
//!   was drawn. The step comes from a singular value decomposition, never from normal equations.
//! - Convergence in millimetres, not in absolute numbers: every residual within 2^-16 mm.
//! - Work in steps, never time (15.6): running out is [`SolveError::WorkLimit`].
//! - Diagnosis (15.8): rank from the singular values with chapter 15.8's derived thresholds
//!   (independent at 2^-12 and above, dependent at 1/(64 S) and below for a block S mm across,
//!   between them "cannot tell", never rounded either way). The free motions are the right null
//!   space, and the dependency groups the left null space: a group holds (redundant) or not
//!   (conflicting). Both are read through their projectors, which do not depend on the basis the
//!   decomposition picked.
//! - Commit (15.10): `commit.rs`.
//!
//! Not yet here, and in the kernel's design: decomposition into independent blocks (15.5), root
//! choices other than the point-to-line side, continuation (15.7), dragging (15.9) and
//! certificates (15.10).
//!
//! Linear algebra is nalgebra's SVD with its default features off. Its source was read for
//! determinism (nalgebra 0.35.0: `linalg/svd.rs`, `bidiagonal.rs`, `householder.rs`,
//! `geometry/reflection.rs`, `base/blas.rs`, `base/ops.rs`): Householder reflections, Givens
//! rotations and dot products in plain scalar loops, no fused multiply-add, and the only
//! run-time processor dispatch (`matrixmultiply`, behind `std`, for products of large dynamic
//! matrices) is neither compiled in nor on these paths. Its column-pivoted QR is not used: it
//! pivots on the largest entry, not on column norms as 15.8 asks.
//!
//! Determinism (15.12): every list is in authoring order, no hash maps, no clocks, no random
//! starts; the only arithmetic is IEEE `+ - * /` and `sqrt`.

mod commit;
mod residual;

use crate::outcome::{Budget, FreePoint, SolveError, Solved, Unmet, Verdict};
use crate::sketch::{Constraint, ConstraintId, LineId, Point, PointId, Side, Sketch, diff, dot};
use nalgebra::DMatrix;
use nalgebra::linalg::SVD;
use num_dual::Dual64;
use residual::{Kind, Row};

/// Converged when every residual is within this, in millimetres.
const TAU_MM: f64 = 1.0 / 65_536.0;
/// A singular value at least this is independent (chapter 15.8's `sigma_ind`).
const SIGMA_IND: f64 = 1.0 / 4_096.0;
/// A projector diagonal above this means the point or constraint takes part (a component of
/// 2^-12, chapter 15.8's dependency-group coefficient, squared).
const SPREAD: f64 = 1.0 / 16_777_216.0;
/// A constraint is in a conflict when its share is at least this fraction of the largest.
const GROUP_RATIO: f64 = 1.0 / 4_096.0;
/// The solve has stalled when one accepted step lowers the squared residual by less than this
/// fraction.
const STALL: f64 = 1.0 / 1_073_741_824.0;
/// The first damping, as a fraction of the largest squared singular value.
const DAMP_START: f64 = 1.0 / 1_048_576.0;
/// Damping stops growing past this multiple of the largest squared singular value.
const DAMP_MAX: f64 = 1_048_576.0;

/// A sketch prepared for solving.
struct Block {
    /// The origin, in whole millimetres, as doubles (exact).
    origin: (f64, f64),
    /// Millimetres per scaled unit: a power of two.
    scale: f64,
    /// For each point, the index of its x variable (y is the next), or `None` if fixed.
    var_of: Vec<Option<usize>>,
    /// Every point's scaled position as drawn.
    drawn: Vec<(f64, f64)>,
    /// The constraints with equations, in authoring order.
    rows: Vec<Row>,
    /// For each equation, its constraint.
    owner: Vec<ConstraintId>,
    /// The number of variables.
    n: usize,
    /// Points fixed more than once: the repeats.
    repeated_fixings: Vec<ConstraintId>,
}

fn index(id: u32) -> Option<usize> {
    usize::try_from(id).ok()
}

/// A whole millimetre within the sketch's range, as a double (exact).
fn real_mm(mm: i64) -> Option<f64> {
    i32::try_from(mm).ok().map(f64::from)
}

/// The smallest power of two at least `value` (and at least 1).
fn power_of_two_above(value: f64) -> f64 {
    let mut power = 1.0;
    while power < value {
        power *= 2.0;
    }
    power
}

impl Block {
    fn new(sketch: &Sketch) -> Result<Block, SolveError> {
        let points = sketch.points();
        let ox = points.iter().map(|p| p.x_mm).min().unwrap_or(0);
        let oy = points.iter().map(|p| p.y_mm).min().unwrap_or(0);
        let (Some(left), Some(bottom)) = (real_mm(ox), real_mm(oy)) else {
            return Err(SolveError::Decomposition);
        };
        let mut extent: f64 = 1.0;
        let mut drawn = Vec::with_capacity(points.len());
        for (i, p) in points.iter().enumerate() {
            let point = PointId(u32::try_from(i).map_err(|_| SolveError::Decomposition)?);
            let (Some(x), Some(y)) = (real_mm(p.x_mm), real_mm(p.y_mm)) else {
                return Err(SolveError::OutOfRange { point });
            };
            extent = extent.max(x - left).max(y - bottom);
            drawn.push((x - left, y - bottom));
        }
        for constraint in sketch.constraints() {
            if let Constraint::Distance { mm, .. } | Constraint::PointLineDistance { mm, .. } =
                *constraint
            {
                extent = extent.max(real_mm(mm).unwrap_or(0.0));
            }
        }
        let scale = power_of_two_above(extent);
        for at in &mut drawn {
            *at = (at.0 / scale, at.1 / scale);
        }

        let mut fixed = vec![false; points.len()];
        let mut repeated_fixings = Vec::new();
        for (i, constraint) in sketch.constraints().iter().enumerate() {
            if let Constraint::Fixed(point) = *constraint {
                let id = ConstraintId(u32::try_from(i).map_err(|_| SolveError::Decomposition)?);
                let slot = index(point.0)
                    .and_then(|p| fixed.get_mut(p))
                    .ok_or(SolveError::Degenerate { constraint: id })?;
                if *slot {
                    repeated_fixings.push(id);
                }
                *slot = true;
            }
        }
        let mut var_of = Vec::with_capacity(points.len());
        let mut n: usize = 0;
        for is_fixed in &fixed {
            if *is_fixed {
                var_of.push(None);
            } else {
                var_of.push(Some(n));
                n = n.checked_add(2).ok_or(SolveError::Decomposition)?;
            }
        }

        let mut block = Block {
            origin: (left, bottom),
            scale,
            var_of,
            drawn,
            rows: Vec::new(),
            owner: Vec::new(),
            n,
            repeated_fixings,
        };
        for (i, constraint) in sketch.constraints().iter().enumerate() {
            let id = ConstraintId(u32::try_from(i).map_err(|_| SolveError::Decomposition)?);
            if let Some(kind) = block.prepare(sketch, id, *constraint)? {
                for _ in 0..kind.equations() {
                    block.owner.push(id);
                }
                block.rows.push(Row {
                    constraint: id,
                    kind,
                });
            }
        }
        Ok(block)
    }

    /// A constraint in the solver's terms, or `None` for a fixing (it has no equations).
    fn prepare(
        &self,
        sketch: &Sketch,
        id: ConstraintId,
        constraint: Constraint,
    ) -> Result<Option<Kind>, SolveError> {
        let degenerate = SolveError::Degenerate { constraint: id };
        let point = |p: PointId| index(p.0).ok_or(degenerate.clone());
        // A line's two point indices and its exact squared length as drawn.
        let line = |l: LineId| -> Result<([usize; 2], i128), SolveError> {
            let line = sketch.line(l).ok_or(degenerate.clone())?;
            let a = sketch.point(line.start).ok_or(degenerate.clone())?;
            let b = sketch.point(line.end).ok_or(degenerate.clone())?;
            let u = diff(a, b).ok_or(degenerate.clone())?;
            let squared = dot(u, u).ok_or(degenerate.clone())?;
            Ok(([point(line.start)?, point(line.end)?], squared))
        };
        // Two lines, both drawn with some length, and 1 / L for L the power of two at least the
        // longer, in scaled units.
        let two_lines =
            |a: LineId, b: LineId| -> Result<([usize; 2], [usize; 2], f64), SolveError> {
                let (a, a_squared) = line(a)?;
                let (b, b_squared) = line(b)?;
                if a_squared == 0 || b_squared == 0 {
                    return Err(degenerate.clone());
                }
                let longer = commit::real(a_squared.max(b_squared)).sqrt();
                Ok((a, b, self.scale / power_of_two_above(longer)))
            };
        let scaled = |mm: i64| {
            real_mm(mm)
                .map(|v| v / self.scale)
                .ok_or(degenerate.clone())
        };
        Ok(Some(match constraint {
            Constraint::Coincident(a, b) => Kind::Coincident {
                p: point(a)?,
                q: point(b)?,
            },
            Constraint::Horizontal(l) => {
                let ([p, q], _) = line(l)?;
                Kind::Horizontal { p, q }
            }
            Constraint::Vertical(l) => {
                let ([p, q], _) = line(l)?;
                Kind::Vertical { p, q }
            }
            Constraint::Parallel(a, b) => {
                let (a, b, inv_len) = two_lines(a, b)?;
                Kind::Parallel { a, b, inv_len }
            }
            Constraint::Perpendicular(a, b) => {
                let (a, b, inv_len) = two_lines(a, b)?;
                Kind::Perpendicular { a, b, inv_len }
            }
            Constraint::EqualLength(a, b) => {
                let (a, b, inv_len) = two_lines(a, b)?;
                Kind::EqualLength {
                    a,
                    b,
                    inv_two_len: inv_len / 2.0,
                }
            }
            Constraint::Distance { a, b, mm } => {
                let (pa, pb) = (
                    sketch.point(a).ok_or(degenerate.clone())?,
                    sketch.point(b).ok_or(degenerate.clone())?,
                );
                if pa == pb {
                    return Err(degenerate);
                }
                let d = scaled(mm)?;
                Kind::Distance {
                    p: point(a)?,
                    q: point(b)?,
                    d_squared: d * d,
                    inv_two_d: 1.0 / (2.0 * d),
                }
            }
            Constraint::PointLineDistance {
                point: p,
                line: l,
                mm,
                side,
            } => {
                let (line, squared) = line(l)?;
                if squared == 0 {
                    return Err(degenerate);
                }
                let d = scaled(mm)?;
                Kind::PointLine {
                    p: point(p)?,
                    line,
                    signed: match side {
                        Side::Left => d,
                        Side::Right => -d,
                    },
                }
            }
            Constraint::Fixed(_) => return Ok(None),
        }))
    }

    /// The residuals at `x`, and the Jacobian (one row per equation) when `with_jacobian`.
    /// `Err` names the first constraint whose value is not a finite number.
    fn evaluate(
        &self,
        x: &[f64],
        with_jacobian: bool,
    ) -> Result<(Vec<f64>, Vec<Vec<f64>>), ConstraintId> {
        let at = |seed: Option<usize>| {
            move |point: usize| -> Option<(Dual64, Dual64)> {
                if let Some(v) = *self.var_of.get(point)? {
                    let w = v.checked_add(1)?;
                    let tangent = |var: usize| if seed == Some(var) { 1.0 } else { 0.0 };
                    Some((
                        Dual64::new(*x.get(v)?, tangent(v)),
                        Dual64::new(*x.get(w)?, tangent(w)),
                    ))
                } else {
                    let (dx, dy) = *self.drawn.get(point)?;
                    Some((Dual64::from_re(dx), Dual64::from_re(dy)))
                }
            }
        };
        let mut r = Vec::with_capacity(self.owner.len());
        let mut jacobian = Vec::new();
        for row in &self.rows {
            let bad = row.constraint;
            let value = row.kind.eval(&at(None)).ok_or(bad)?;
            let mut first = vec![0.0; if with_jacobian { self.n } else { 0 }];
            let mut second = first.clone();
            if with_jacobian {
                for point in row.kind.points() {
                    let Some(v) = *self.var_of.get(point).ok_or(bad)? else {
                        continue;
                    };
                    for var in [v, v.checked_add(1).ok_or(bad)?] {
                        let seeded = row.kind.eval(&at(Some(var))).ok_or(bad)?;
                        *first.get_mut(var).ok_or(bad)? = seeded.first.eps;
                        if let (Some(slot), Some(eq)) = (second.get_mut(var), seeded.second) {
                            *slot = eq.eps;
                        }
                    }
                }
            }
            let finite = |e: &[f64]| e.iter().all(|v| v.is_finite());
            r.push(value.first.re);
            if !value.first.re.is_finite() || !finite(&first) || !finite(&second) {
                return Err(bad);
            }
            if with_jacobian {
                jacobian.push(first);
            }
            if let Some(eq) = value.second {
                if !eq.re.is_finite() {
                    return Err(bad);
                }
                r.push(eq.re);
                if with_jacobian {
                    jacobian.push(second);
                }
            }
        }
        Ok((r, jacobian))
    }
}

/// A singular value decomposition of the Jacobian padded with zeros to a square, so that both
/// null spaces are complete. The padding only adds zero singular values whose vectors lie in the
/// padding, and the projectors read below are restricted to the real rows and columns.
struct Decomposition {
    u: DMatrix<f64>,
    v_t: DMatrix<f64>,
    values: Vec<f64>,
}

fn decompose(jacobian: &[Vec<f64>], m: usize, n: usize) -> Result<Decomposition, SolveError> {
    let k = m.max(n).max(1);
    let matrix = DMatrix::from_fn(k, k, |i, c| {
        jacobian
            .get(i)
            .and_then(|row| row.get(c))
            .copied()
            .unwrap_or(0.0)
    });
    let cap = k.checked_mul(64).ok_or(SolveError::Decomposition)?;
    let svd = SVD::try_new_unordered(matrix, true, true, 5.0 * f64::EPSILON, cap)
        .ok_or(SolveError::Decomposition)?;
    let (Some(u), Some(v_t)) = (svd.u, svd.v_t) else {
        return Err(SolveError::Decomposition);
    };
    Ok(Decomposition {
        u,
        v_t,
        values: svd.singular_values.iter().copied().collect(),
    })
}

fn at(matrix: &DMatrix<f64>, row: usize, column: usize) -> f64 {
    matrix.get((row, column)).copied().unwrap_or(0.0)
}

fn norm_squared(r: &[f64]) -> f64 {
    r.iter().map(|v| v * v).sum()
}

fn largest(r: &[f64]) -> f64 {
    r.iter().fold(0.0, |most: f64, v| most.max(v.abs()))
}

impl Decomposition {
    /// The damped least-change step for residuals `r`: -sum over the kept singular values of
    /// v (s / (s^2 + lambda)) (u . r). With no damping this is the shortest least-squares step.
    fn step(&self, r: &[f64], lambda: f64, sigma_dep: f64, n: usize) -> Vec<f64> {
        let mut delta = vec![0.0; n];
        for (j, s) in self.values.iter().copied().enumerate() {
            if s <= sigma_dep {
                continue;
            }
            let ur: f64 = r
                .iter()
                .enumerate()
                .map(|(i, ri)| at(&self.u, i, j) * ri)
                .sum();
            let coefficient = s / (s * s + lambda) * ur;
            for (v, d) in delta.iter_mut().enumerate() {
                *d -= coefficient * at(&self.v_t, j, v);
            }
        }
        delta
    }

    /// The columns whose singular values are at most `sigma_dep`, and those that cannot be told.
    fn split(&self, sigma_dep: f64) -> (Vec<usize>, Vec<usize>) {
        let mut null = Vec::new();
        let mut unclear = Vec::new();
        for (j, s) in self.values.iter().copied().enumerate() {
            if s <= sigma_dep {
                null.push(j);
            } else if s < SIGMA_IND {
                unclear.push(j);
            }
        }
        (null, unclear)
    }
}

/// Each constraint's share of a per-equation score, in authoring order, keeping the largest.
fn per_constraint(
    owner: &[ConstraintId],
    score: impl Fn(usize) -> f64,
) -> Vec<(ConstraintId, f64)> {
    let mut out: Vec<(ConstraintId, f64)> = Vec::new();
    for (i, id) in owner.iter().copied().enumerate() {
        let value = score(i);
        match out.last_mut() {
            Some(last) if last.0 == id => last.1 = last.1.max(value),
            _ => out.push((id, value)),
        }
    }
    out
}

/// The constraints whose equations lie in the left singular vectors `columns`.
fn touched(d: &Decomposition, owner: &[ConstraintId], columns: &[usize]) -> Vec<ConstraintId> {
    per_constraint(owner, |i| {
        columns
            .iter()
            .map(|c| {
                let e = at(&d.u, i, *c);
                e * e
            })
            .sum()
    })
    .into_iter()
    .filter(|(_, share)| *share > SPREAD)
    .map(|(id, _)| id)
    .collect()
}

/// How many independent ways each point can move, from the right null space's projector.
fn free_points(dec: &Decomposition, block: &Block, null: &[usize]) -> Vec<FreePoint> {
    let mut free = Vec::new();
    for (index, var) in block.var_of.iter().enumerate() {
        let (Some(x_var), Ok(id)) = (*var, u32::try_from(index)) else {
            continue;
        };
        let y_var = x_var.saturating_add(1);
        // The 2 x 2 block of the null-space projector on this point's two variables.
        let (mut xx, mut xy, mut yy) = (0.0, 0.0, 0.0);
        for column in null {
            let along_x = at(&dec.v_t, *column, x_var);
            let along_y = at(&dec.v_t, *column, y_var);
            xx += along_x * along_x;
            xy += along_x * along_y;
            yy += along_y * along_y;
        }
        let half_trace = f64::midpoint(xx, yy);
        let half_gap = (xx - yy) / 2.0;
        let spread = (half_gap * half_gap + xy * xy).sqrt();
        let dof = u8::from(half_trace + spread > SPREAD)
            .saturating_add(u8::from(half_trace - spread > SPREAD));
        if dof > 0 {
            free.push(FreePoint {
                point: PointId(id),
                dof,
            });
        }
    }
    free
}

/// Counts one step against the budget.
fn take(steps: &mut u32, budget: Budget) -> Result<(), SolveError> {
    if *steps >= budget.get() {
        return Err(SolveError::WorkLimit { steps: *steps });
    }
    *steps = steps.saturating_add(1);
    Ok(())
}

/// Where the iteration stopped.
struct Reached {
    /// The variables.
    vars: Vec<f64>,
    /// The residuals there.
    residuals: Vec<f64>,
    /// The Jacobian there.
    jacobian: Vec<Vec<f64>>,
    /// Every residual within tolerance.
    converged: bool,
}

impl Block {
    /// The variables as drawn.
    fn start(&self) -> Vec<f64> {
        let mut vars = vec![0.0; self.n];
        for (point, var) in self.var_of.iter().enumerate() {
            if let (Some(v), Some(drawn)) = (*var, self.drawn.get(point)) {
                if let Some(slot) = vars.get_mut(v) {
                    *slot = drawn.0;
                }
                if let Some(slot) = v.checked_add(1).and_then(|w| vars.get_mut(w)) {
                    *slot = drawn.1;
                }
            }
        }
        vars
    }

    /// Damped least-change Gauss-Newton steps from the sketch as drawn, until every residual is
    /// within tolerance or no step lowers them any more (the least-squares best).
    fn iterate(&self, budget: Budget, steps: &mut u32) -> Result<Reached, SolveError> {
        let equations = self.owner.len();
        let tau = TAU_MM / self.scale;
        let sigma_dep = 1.0 / (64.0 * self.scale);
        let degenerate = |constraint| SolveError::Degenerate { constraint };
        let mut vars = self.start();
        take(steps, budget)?;
        let (mut residuals, mut jacobian) = self.evaluate(&vars, true).map_err(degenerate)?;
        let mut lambda: f64 = 0.0;
        let mut converged = largest(&residuals) <= tau;
        while !converged {
            let dec = decompose(&jacobian, equations, self.n)?;
            let top = dec.values.iter().fold(0.0, |most: f64, s| most.max(*s));
            let top_squared = (top * top).max(f64::MIN_POSITIVE);
            let before = norm_squared(&residuals);
            let mut accepted = None;
            loop {
                let delta = dec.step(&residuals, lambda, sigma_dep, self.n);
                if delta.iter().all(|v| *v == 0.0) {
                    break;
                }
                take(steps, budget)?;
                let trial: Vec<f64> = vars.iter().zip(&delta).map(|(a, b)| a + b).collect();
                if let Ok((after, _)) = self.evaluate(&trial, false)
                    && norm_squared(&after) < before
                {
                    accepted = Some(trial);
                    break;
                }
                lambda = if lambda == 0.0 {
                    DAMP_START * top_squared
                } else {
                    lambda * 4.0
                };
                if lambda > DAMP_MAX * top_squared {
                    break;
                }
            }
            let Some(trial) = accepted else {
                break;
            };
            vars = trial;
            lambda /= 4.0;
            if lambda < DAMP_START * top_squared {
                lambda = 0.0;
            }
            take(steps, budget)?;
            (residuals, jacobian) = self.evaluate(&vars, true).map_err(degenerate)?;
            converged = largest(&residuals) <= tau;
            if !converged && before - norm_squared(&residuals) <= STALL * before {
                break;
            }
        }
        Ok(Reached {
            vars,
            residuals,
            jacobian,
            converged,
        })
    }

    /// Why an unconverged solve failed: a conflict if the residual left over lies in the left
    /// null space (no move can remove it), otherwise a rank too close to call, otherwise not
    /// converged.
    fn failure(&self, dec: &Decomposition, residuals: &[f64]) -> SolveError {
        let tau = TAU_MM / self.scale;
        let (null, unclear) = dec.split(1.0 / (64.0 * self.scale));
        let shares: Vec<f64> = (0..self.owner.len())
            .map(|i| {
                null.iter()
                    .map(|c| {
                        let along: f64 = residuals
                            .iter()
                            .enumerate()
                            .map(|(l, rl)| at(&dec.u, l, *c) * rl)
                            .sum();
                        at(&dec.u, i, *c) * along
                    })
                    .sum::<f64>()
            })
            .collect();
        let most = largest(&shares);
        if most > tau {
            let conflicting =
                per_constraint(&self.owner, |i| shares.get(i).map_or(0.0, |v| v.abs()))
                    .into_iter()
                    .filter(|(_, share)| *share >= GROUP_RATIO * most)
                    .map(|(id, _)| id)
                    .collect();
            return SolveError::OverConstrained { conflicting };
        }
        if !unclear.is_empty() {
            return SolveError::RankUnclear {
                constraints: touched(dec, &self.owner, &unclear),
            };
        }
        let unmet = per_constraint(&self.owner, |i| residuals.get(i).map_or(0.0, |v| v.abs()))
            .into_iter()
            .filter(|(_, residual)| *residual > tau)
            .map(|(constraint, residual)| Unmet {
                constraint,
                residual_mm: residual * self.scale,
            })
            .collect();
        SolveError::NotConverged { unmet }
    }

    /// The verdict and the redundant constraints of a converged solve.
    fn verdict(&self, dec: &Decomposition) -> Result<(Verdict, Vec<ConstraintId>), SolveError> {
        let sigma_dep = 1.0 / (64.0 * self.scale);
        let (null, unclear) = dec.split(sigma_dep);
        if !unclear.is_empty() {
            return Err(SolveError::RankUnclear {
                constraints: touched(dec, &self.owner, &unclear),
            });
        }
        let mut redundant = touched(dec, &self.owner, &null);
        redundant.extend(self.repeated_fixings.iter().copied());
        redundant.sort_unstable();
        redundant.dedup();
        let rank = dec.values.iter().filter(|s| **s > sigma_dep).count();
        let dof = self.n.checked_sub(rank).ok_or(SolveError::Decomposition)?;
        let verdict = if dof == 0 {
            Verdict::FullyConstrained
        } else {
            Verdict::UnderConstrained {
                dof: u32::try_from(dof).map_err(|_| SolveError::Decomposition)?,
                free: free_points(dec, self, &null),
            }
        };
        Ok((verdict, redundant))
    }

    /// Every point in whole millimetres, by the rounding rule (`commit.rs`).
    fn rounded(&self, sketch: &Sketch, vars: &[f64]) -> Result<Vec<Point>, SolveError> {
        let mut points = Vec::with_capacity(sketch.points().len());
        for (i, (drawn, var)) in sketch.points().iter().zip(&self.var_of).enumerate() {
            let point = PointId(u32::try_from(i).map_err(|_| SolveError::Decomposition)?);
            let Some(v) = *var else {
                points.push(*drawn);
                continue;
            };
            let value = |var: Option<usize>| var.and_then(|var| vars.get(var)).copied();
            let (Some(sx), Some(sy)) = (value(Some(v)), value(v.checked_add(1))) else {
                return Err(SolveError::OutOfRange { point });
            };
            let (Some(x_mm), Some(y_mm)) = (
                commit::to_mm(self.origin.0, sx, self.scale),
                commit::to_mm(self.origin.1, sy, self.scale),
            ) else {
                return Err(SolveError::OutOfRange { point });
            };
            points.push(Point { x_mm, y_mm });
        }
        Ok(points)
    }
}

/// Solves a sketch: see [`crate::solve`].
pub(crate) fn solve(sketch: &Sketch, budget: Budget) -> Result<Solved, SolveError> {
    let block = Block::new(sketch)?;
    let mut steps: u32 = 0;
    let reached = block.iterate(budget, &mut steps)?;
    take(&mut steps, budget)?;
    let dec = decompose(&reached.jacobian, block.owner.len(), block.n)?;
    if !reached.converged {
        return Err(block.failure(&dec, &reached.residuals));
    }
    let (verdict, redundant) = block.verdict(&dec)?;
    let points = block.rounded(sketch, &reached.vars)?;
    commit::check(sketch, &points)?;
    Ok(Solved {
        points,
        verdict,
        redundant,
        steps,
    })
}
