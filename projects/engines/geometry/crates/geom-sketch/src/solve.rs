//! The block solver and continuation: Gauss–Newton steps with Moré's damping on each block of a
//! sketch, least change where it is under-determined, and a followed path when its values change.
//!
//! What it does: a plan is what depends on a sketch's structure alone: the merged variables, the
//! Jacobian's pattern, the stages in solve order and an analysis of each. Each stage is a component's
//! over-determined part, one of its square blocks, or its under-determined part, and holds everything
//! before it fixed. [`solve`] runs every stage from the sketch's values until each residual is within
//! a 64th of a grid step, and returns the sketch at those values with a [`Report`]. [`continue_to`]
//! moves the changed values along a straight path instead of jumping, solving at each step from the
//! last, and halves a step that does not converge, puts a stored sense at risk or nears a singular
//! configuration.
//!
//! Guarantees:
//!
//! - **Least change.** The full step is the least-norm least-squares step, the limit of the damped
//!   steps as the damping goes to zero, so a free variable does not jump (chapter 15.6). Every
//!   variable has weight one: after the scaling of chapter 15.4 each acts as a length. Weights of
//!   their own are the dragging piece's (G228).
//! - **QR, never normal equations.** A damped step is the least-squares solution of J stacked on
//!   √λ·I, by `geom_linalg`'s sparse QR; the pattern of each is analysed once, in the plan.
//! - **Convergence in grid steps.** A stage has converged when each of its residuals is at most
//!   [`TOLERANCE`]. Every residual is a length with a gradient near one, so that means the same at
//!   any size, and no further division by the block's size would change a step.
//! - **Never a partial answer.** A block that does not converge is [`SketchError::Unsolved`], naming
//!   its worst rule, and running out of budget is [`SketchError::Stopped`]; neither returns a sketch.
//! - **A singular start is nudged once.** A rule on its singular set has its points moved by small
//!   amounts derived from its id alone, which the report records; a rule still on it is
//!   [`SketchError::Degenerate`] (chapter 15.4).
//! - **The branch is followed.** Each stored sense is checked after each step of a path with interval
//!   arithmetic, and a sign that cannot be certified counts as at risk; the smallest pivot of each
//!   square block is held at or above [`INDEPENDENT`]. After [`MOST_HALVINGS`] halvings at one place
//!   the answer is [`SketchError::BranchLost`], with the rule and how far along the path it stopped.
//! - **Deterministic.** Stages, rows and steps run in an order that depends on stored data alone, and
//!   the schedule of a path on the stored state and the new values alone (chapter 15.7).
//!
//! Method: Levenberg–Marquardt as J. J. Moré, *The Levenberg–Marquardt algorithm: implementation and
//! theory*, Numerical Analysis, Springer LNM 630 (1978), <https://doi.org/10.1007/BFb0067700>: accept
//! a step on the ratio of actual to predicted reduction, and move the damping by a factor of four on
//! the same ratio. The trust-region radius of that paper is replaced by the damping itself, as in
//! Marquardt's original. Continuation is section 15.7
//! (`projects/engines/geometry/architecture/15-sketches.md`).
//!
//! Consulted: fiksi (Endoli), MIT,
//!   <https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/solve/lm.rs>,
//!   for solving the stacked system instead of the normal equations; no code is taken.
//!
//! Cost: each iteration of a stage is one factorisation of its rows, charged to the work meter with
//! the rows' evaluation, and the meter is checked once an iteration. No recursion.

extern crate alloc;

use alloc::vec::Vec;

use geom_linalg::matrix::Work;
use geom_linalg::sparse_qr::{SparseQr, SymbolicQr};
use geom_linalg::{LinalgError, Sparse};
use geom_num::real::Real;
use geom_num::scalar::Scalar;
use geom_run::WorkMeter;

use crate::equation::{Assembly, Sense};
use crate::model::{ItemId, Sketch, SketchError};
use crate::structure::{Part, analyse, substitute};

/// A residual this small is met: a 64th of a grid step, in steps (section 15.6).
pub const TOLERANCE: f64 = 0.015_625;

/// A pivot at least this large is independent: section 15.8's `σ_ind`, 2^-12.
pub const INDEPENDENT: f64 = 0.000_244_140_625;

/// How many halvings at one place of a path before it is lost (section 15.7).
pub const MOST_HALVINGS: u32 = 10;

/// How many iterations a stage may take. A block that is converging takes a few.
const MOST_ITERATIONS: u32 = 200;

/// The first damping tried, and below which the damping goes back to zero.
const LEAST_DAMPING: f64 = 1e-6;

/// The damping past which no step will ever help.
const MOST_DAMPING: f64 = 1e12;

/// The share of the predicted reduction a step must make to be kept.
const ACCEPTED: f64 = 1e-4;

/// A share of |r|² so small that a step predicting less has nowhere left to go.
const FLAT: f64 = 1e-12;

/// The pivot, against the largest column, below which a full step treats a column as dead.
const DEAD: f64 = 1e-9;

/// What a solve did.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// Gauss–Newton iterations, over every stage and step.
    pub iterations: u64,
    /// The steps of a path, 1 for a plain solve.
    pub steps: u32,
    /// The stages run: parts of components and square blocks.
    pub stages: usize,
    /// The sketch's structural freedoms.
    pub freedoms: usize,
    /// The largest residual at the answer, in steps.
    pub largest: f64,
    /// The rules whose start was nudged off a singular set.
    pub nudged: Vec<ItemId>,
}

/// A solved sketch and how it went.
#[derive(Clone, Debug, PartialEq)]
pub struct Solved {
    /// The sketch at the solution.
    pub sketch: Sketch,
    /// How it went.
    pub report: Report,
}

/// One part of a component, with everything to take a step on it.
struct Stage {
    /// The assembly's rules that write these rows.
    rules: Vec<usize>,
    /// The assembly's rows.
    rows: Vec<usize>,
    /// The assembly's columns.
    columns: Vec<usize>,
    /// Each column's scale.
    scales: Vec<f64>,
    /// For each stored entry: its place in the assembly's Jacobian, in `plain` and in `stacked`.
    map: Vec<(usize, usize, usize)>,
    /// J on these rows and columns.
    plain: Sparse,
    /// J over a diagonal.
    stacked: Sparse,
    /// The diagonal's places in `stacked`.
    diagonal: Vec<usize>,
    plain_order: SymbolicQr,
    stacked_order: SymbolicQr,
    /// Whether it is a square block, whose pivots a path watches.
    square: bool,
}

/// What depends on a sketch's structure alone: computed once, and reused along a path.
struct Plan {
    assembly: Assembly,
    stages: Vec<Stage>,
    /// Each sketch variable's column.
    merged: Vec<usize>,
    freedoms: usize,
}

fn at(list: &[usize], i: usize) -> Result<usize, SketchError> {
    list.get(i).copied().ok_or(SketchError::Internal)
}

fn push<T>(list: &mut Vec<T>, value: T) -> Result<(), SketchError> {
    list.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
    list.push(value);
    Ok(())
}

fn zeros(n: usize) -> Result<Vec<f64>, SketchError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| SketchError::OutOfMemory)?;
    v.resize(n, 0.0);
    Ok(v)
}

fn wide(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// |v|².
fn square(v: &[f64]) -> Result<f64, SketchError> {
    let mut sum = Real::ZERO;
    for x in v {
        sum = sum.add(Real::new(*x)?.square()?)?;
    }
    Ok(sum.get())
}

/// The largest absolute value in `v`, and where.
fn worst(v: &[f64]) -> (f64, usize) {
    let mut most = (0.0, 0);
    for (i, x) in v.iter().enumerate() {
        if x.abs() > most.0 {
            most = (x.abs(), i);
        }
    }
    most
}

impl Stage {
    fn new(
        assembly: &Assembly,
        part: &Part,
        square: bool,
        work: &mut Work,
    ) -> Result<Stage, SketchError> {
        let (rows, columns) = (part.equations.clone(), part.variables.clone());
        let jacobian = assembly.jacobian();
        let mut local = Vec::new();
        local
            .try_reserve_exact(jacobian.rows())
            .map_err(|_| SketchError::OutOfMemory)?;
        local.resize(jacobian.rows(), None);
        let mut rules: Vec<usize> = Vec::new();
        for (i, &row) in rows.iter().enumerate() {
            *local.get_mut(row).ok_or(SketchError::Internal)? = Some(i);
            let rule = assembly.rule_of(row)?;
            if rules.last() != Some(&rule) {
                push(&mut rules, rule)?;
            }
        }
        let mut entries = Vec::new();
        let mut scales = Vec::new();
        for (k, &column) in columns.iter().enumerate() {
            push(&mut scales, assembly.scale_of(column)?)?;
            let (held, _) = jacobian.column(column)?;
            for &row in held {
                if let Some(i) = local.get(row).copied().flatten() {
                    push(&mut entries, (i, k, jacobian.place_of(row, column)?))?;
                }
            }
        }
        let (m, n) = (rows.len(), columns.len());
        let mut triplets = Vec::new();
        for &(i, k, _) in &entries {
            push(&mut triplets, (i, k, 0.0))?;
        }
        let plain = Sparse::from_triplets(m, n, &triplets)?;
        for k in 0..n {
            let row = m.checked_add(k).ok_or(SketchError::Internal)?;
            push(&mut triplets, (row, k, 0.0))?;
        }
        let tall = m.checked_add(n).ok_or(SketchError::Internal)?;
        let stacked = Sparse::from_triplets(tall, n, &triplets)?;
        let mut map = Vec::new();
        for &(i, k, place) in &entries {
            push(
                &mut map,
                (place, plain.place_of(i, k)?, stacked.place_of(i, k)?),
            )?;
        }
        let mut diagonal = Vec::new();
        for k in 0..n {
            let row = m.checked_add(k).ok_or(SketchError::Internal)?;
            push(&mut diagonal, stacked.place_of(row, k)?)?;
        }
        Ok(Stage {
            plain_order: SymbolicQr::new(&plain, work)?,
            stacked_order: SymbolicQr::new(&stacked, work)?,
            rules,
            rows,
            columns,
            scales,
            map,
            plain,
            stacked,
            diagonal,
            square,
        })
    }

    /// Evaluates this stage's rules at `x`, and reads its residuals into `r`.
    fn evaluate(
        &self,
        assembly: &mut Assembly,
        x: &[f64],
        r: &mut [f64],
    ) -> Result<(), SketchError> {
        for &rule in &self.rules {
            assembly.evaluate_rule(rule, x)?;
        }
        let all = assembly.residuals();
        for (out, &row) in r.iter_mut().zip(&self.rows) {
            *out = *all.get(row).ok_or(SketchError::Internal)?;
        }
        Ok(())
    }

    /// Copies the Jacobian's entries in.
    fn load(&mut self, assembly: &Assembly) -> Result<(), SketchError> {
        let jacobian = assembly.jacobian();
        for &(place, plain, stacked) in &self.map {
            let value = jacobian.at_place(place)?;
            self.plain.set_at(plain, value)?;
            self.stacked.set_at(stacked, value)?;
        }
        Ok(())
    }

    /// The step at damping `lambda`: the least-change step at zero, and if J will not factor there,
    /// `None`.
    fn step(
        &mut self,
        r: &[f64],
        lambda: f64,
        work: &mut Work,
    ) -> Result<Option<Vec<f64>>, SketchError> {
        if lambda == 0.0 {
            let tol = Real::new(DEAD)?;
            let qr = match SparseQr::factor(&self.plain_order, &self.plain, tol, work) {
                Err(LinalgError::Singular) => return Ok(None),
                other => other?,
            };
            let mut b = zeros(r.len())?;
            for (out, value) in b.iter_mut().zip(r) {
                *out = -*value;
            }
            return Ok(Some(qr.least_squares(&b, work)?));
        }
        let root = Real::new(lambda)?.sqrt()?.get();
        for &place in &self.diagonal {
            self.stacked.set_at(place, root)?;
        }
        let qr = SparseQr::factor(&self.stacked_order, &self.stacked, Real::ZERO, work)?;
        let tall = r
            .len()
            .checked_add(self.columns.len())
            .ok_or(SketchError::Internal)?;
        let mut b = zeros(tall)?;
        for (out, value) in b.iter_mut().zip(r) {
            *out = -*value;
        }
        Ok(Some(qr.least_squares(&b, work)?))
    }

    /// The reduction of |r|² a step of `delta` predicts.
    fn predicted(&self, r: &[f64], delta: &[f64]) -> Result<f64, SketchError> {
        let moved = self.plain.multiply(delta)?;
        let mut after = Vec::new();
        for (value, change) in r.iter().zip(&moved) {
            push(&mut after, *value + *change)?;
        }
        Ok(square(r)? - square(&after)?)
    }

    /// Adds `delta`, in the solver's scaled variables, to `x`.
    fn advance(&self, x: &mut [f64], delta: &[f64], sign: f64) -> Result<(), SketchError> {
        for ((&column, change), scale) in self.columns.iter().zip(delta).zip(&self.scales) {
            let held = x.get_mut(column).ok_or(SketchError::Internal)?;
            *held += sign * *change * *scale;
        }
        Ok(())
    }

    /// The rule of this stage's row `i`.
    fn rule_name(&self, assembly: &Assembly, i: usize) -> Result<ItemId, SketchError> {
        assembly.name_of_row(at(&self.rows, i)?)
    }

    /// Solves this stage from `x`, holding everything else fixed.
    fn solve(
        &mut self,
        assembly: &mut Assembly,
        x: &mut [f64],
        report: &mut Report,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(), SketchError> {
        let mut r = zeros(self.rows.len())?;
        self.start(assembly, x, &mut r, report)?;
        let mut trial = zeros(self.rows.len())?;
        let mut lambda = 0.0_f64;
        for _ in 0..MOST_ITERATIONS {
            let (most, row) = worst(&r);
            if most <= TOLERANCE {
                return Ok(());
            }
            report.iterations = report.iterations.saturating_add(1);
            meter.charge(wide(self.map.len()))?;
            self.load(assembly)?;
            loop {
                if lambda > MOST_DAMPING {
                    return Err(SketchError::Unsolved(self.rule_name(assembly, row)?));
                }
                let mut work = Work::new(u64::MAX);
                let step = self.step(&r, lambda, &mut work)?;
                meter.charge(u64::MAX.saturating_sub(work.left()))?;
                let Some(delta) = step else {
                    lambda = LEAST_DAMPING;
                    continue;
                };
                let predicted = self.predicted(&r, &delta)?;
                if predicted <= FLAT * square(&r)? {
                    return Err(SketchError::Unsolved(self.rule_name(assembly, row)?));
                }
                self.advance(x, &delta, 1.0)?;
                let ratio = match self.evaluate(assembly, x, &mut trial) {
                    Ok(()) => (square(&r)? - square(&trial)?) / predicted,
                    Err(SketchError::Degenerate(_) | SketchError::Numeric(_)) => 0.0,
                    Err(e) => return Err(e),
                };
                if ratio > 0.75 {
                    lambda /= 4.0;
                    if lambda < LEAST_DAMPING {
                        lambda = 0.0;
                    }
                } else if ratio < 0.25 {
                    lambda = if lambda < LEAST_DAMPING {
                        LEAST_DAMPING
                    } else {
                        lambda * 4.0
                    };
                }
                if ratio > ACCEPTED {
                    r.copy_from_slice(&trial);
                    break;
                }
                self.advance(x, &delta, -1.0)?;
                // The rejected point's evaluation stands in the assembly; the next load reads
                // the kept point's.
                self.evaluate(assembly, x, &mut trial)?;
                self.load(assembly)?;
            }
        }
        let (_, row) = worst(&r);
        Err(SketchError::Unsolved(self.rule_name(assembly, row)?))
    }

    /// The first evaluation, with the one nudge a singular start is allowed.
    fn start(
        &self,
        assembly: &mut Assembly,
        x: &mut [f64],
        r: &mut [f64],
        report: &mut Report,
    ) -> Result<(), SketchError> {
        match self.evaluate(assembly, x, r) {
            Err(SketchError::Degenerate(id)) => {
                assembly.nudge(id, x, &self.columns)?;
                push(&mut report.nudged, id)?;
                self.evaluate(assembly, x, r)
            }
            other => other,
        }
    }

    /// The smallest pivot of this stage's Jacobian at the last evaluation.
    fn smallest_pivot(
        &mut self,
        assembly: &mut Assembly,
        x: &[f64],
        r: &mut [f64],
        meter: &mut WorkMeter<'_>,
    ) -> Result<f64, SketchError> {
        self.evaluate(assembly, x, r)?;
        self.load(assembly)?;
        let mut work = Work::new(u64::MAX);
        let factored = SparseQr::factor(&self.plain_order, &self.plain, Real::ZERO, &mut work);
        meter.charge(u64::MAX.saturating_sub(work.left()))?;
        match factored {
            Ok(qr) => Ok(qr.smallest_pivot()),
            Err(LinalgError::Singular) => Ok(0.0),
            Err(e) => Err(e.into()),
        }
    }
}

impl Plan {
    /// The plan of `sketch`.
    ///
    /// # Errors
    ///
    /// [`SketchError::Stopped`]; what its analysis and pattern refuse.
    fn new(sketch: &Sketch, meter: &mut WorkMeter<'_>) -> Result<Plan, SketchError> {
        let substituted = substitute(sketch)?;
        let analysis = analyse(&substituted.graph, meter)?;
        let assembly = Assembly::merged(sketch, &substituted)?;
        let mut stages = Vec::new();
        let mut freedoms = 0_usize;
        let mut work = Work::new(u64::MAX);
        for component in &analysis.components {
            freedoms = freedoms
                .checked_add(component.freedoms)
                .ok_or(SketchError::Internal)?;
            let parts = core::iter::once((&component.over, false))
                .chain(component.blocks.iter().map(|block| (block, true)))
                .chain(core::iter::once((&component.under, false)));
            for (part, square) in parts {
                if !part.equations.is_empty() {
                    push(&mut stages, Stage::new(&assembly, part, square, &mut work)?)?;
                }
            }
        }
        meter.charge(u64::MAX.saturating_sub(work.left()))?;
        Ok(Plan {
            assembly,
            stages,
            merged: substituted.merged,
            freedoms,
        })
    }

    /// Every stage, in order, from `x`.
    fn run(
        &mut self,
        x: &mut [f64],
        report: &mut Report,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(), SketchError> {
        for stage in &mut self.stages {
            stage.solve(&mut self.assembly, x, report, meter)?;
        }
        Ok(())
    }

    /// The sketch `base` at `x`, and the report finished.
    fn finish(
        &mut self,
        base: &Sketch,
        x: &[f64],
        mut report: Report,
    ) -> Result<Solved, SketchError> {
        self.assembly.evaluate_at(x)?;
        let (largest, row) = worst(self.assembly.residuals());
        if largest > TOLERANCE {
            return Err(SketchError::Unsolved(self.assembly.name_of_row(row)?));
        }
        report.largest = largest;
        report.stages = self.stages.len();
        report.freedoms = self.freedoms;
        let mut values = Vec::new();
        values
            .try_reserve_exact(self.merged.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        for &column in &self.merged {
            values.push(*x.get(column).ok_or(SketchError::Internal)?);
        }
        Ok(Solved {
            sketch: base.with_values(&values)?,
            report,
        })
    }

    /// The first rule whose stored sense has flipped, or cannot be certified, at `x`, or a square
    /// stage's first rule if its smallest pivot has fallen below [`INDEPENDENT`].
    fn at_risk(
        &mut self,
        x: &[f64],
        senses: &[(usize, Sense)],
        watched: &[bool],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<ItemId>, SketchError> {
        for &(rule, sense) in senses {
            if self.assembly.sense_of(rule, x)? != Some(sense) {
                return Ok(Some(self.assembly.name_of_rule(rule)?));
            }
        }
        let mut r = Vec::new();
        for (stage, watch) in self.stages.iter_mut().zip(watched) {
            if *watch {
                r.resize(stage.rows.len(), 0.0);
                if stage.smallest_pivot(&mut self.assembly, x, &mut r, meter)? < INDEPENDENT {
                    return Ok(Some(stage.rule_name(&self.assembly, 0)?));
                }
            }
        }
        Ok(None)
    }
}

const fn empty_report() -> Report {
    Report {
        iterations: 0,
        steps: 1,
        stages: 0,
        freedoms: 0,
        largest: 0.0,
        nudged: Vec::new(),
    }
}

/// Solves `sketch` from its values.
///
/// # Errors
///
/// [`SketchError::Unsolved`] for a block that does not converge; [`SketchError::Degenerate`] for a
/// rule still on its singular set after a nudge; [`SketchError::Stopped`] when the budget runs out;
/// what its analysis and pattern refuse.
pub fn solve(sketch: &Sketch, meter: &mut WorkMeter<'_>) -> Result<Solved, SketchError> {
    let mut plan = Plan::new(sketch, meter)?;
    let mut x = plan.assembly.start()?;
    let mut report = empty_report();
    plan.run(&mut x, &mut report, meter)?;
    plan.finish(sketch, &x, report)
}

/// Follows `from`, which must be solved, to the values of `to`, the same sketch but for its values:
/// dimensions, fixes and the positions of references (chapter 15.7).
///
/// # Errors
///
/// [`SketchError::NotTheSameSketch`] for a `to` of another structure;
/// [`SketchError::BranchLost`] after [`MOST_HALVINGS`] halvings at one place;
/// [`SketchError::Stopped`] when the budget runs out; what its analysis and pattern refuse.
pub fn continue_to(
    from: &Sketch,
    to: &Sketch,
    meter: &mut WorkMeter<'_>,
) -> Result<Solved, SketchError> {
    let mut plan = Plan::new(from, meter)?;
    let target = Assembly::merged(to, &substitute(to)?)?;
    if !plan.assembly.same_structure(&target) {
        return Err(SketchError::NotTheSameSketch);
    }
    let (old, new) = (plan.assembly.slots().to_vec(), target.slots().to_vec());
    let mut x = plan.assembly.start()?;
    let mut report = empty_report();
    report.steps = 0;
    plan.run(&mut x, &mut report, meter)?;
    let senses = plan.assembly.senses(&x)?;
    let mut watched = Vec::new();
    let mut r = Vec::new();
    for stage in &mut plan.stages {
        r.resize(stage.rows.len(), 0.0);
        let pivot = stage.smallest_pivot(&mut plan.assembly, &x, &mut r, meter)?;
        push(&mut watched, stage.square && pivot >= INDEPENDENT)?;
    }
    let (mut done, mut size, mut halvings) = (0.0_f64, 1.0_f64, 0_u32);
    while done < 1.0 {
        let next = if done + size >= 1.0 { 1.0 } else { done + size };
        plan.assembly.move_along(&old, &new, next, &x)?;
        let mut y = x.clone();
        let reached = match plan.run(&mut y, &mut report, meter) {
            Ok(()) => plan.at_risk(&y, &senses, &watched, meter)?,
            Err(SketchError::Unsolved(id) | SketchError::Degenerate(id)) => Some(id),
            Err(e) => return Err(e),
        };
        if let Some(id) = reached {
            halvings = halvings.checked_add(1).ok_or(SketchError::Internal)?;
            if halvings > MOST_HALVINGS {
                return Err(SketchError::BranchLost {
                    constraint: id,
                    along: done,
                });
            }
            size /= 2.0;
        } else {
            x = y;
            done = next;
            report.steps = report.steps.saturating_add(1);
            halvings = 0;
            size *= 2.0;
        }
    }
    plan.finish(to, &x, report)
}
