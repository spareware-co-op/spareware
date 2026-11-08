//! Chained Krawczyk certificates: a proof, block by block in the plan's order, that an exact solution
//! of the sketch lies within a stated distance of the committed geometry.
//!
//! What it does: [`certify`] takes a sketch and its [`Committed`] geometry and walks the structural
//! plan (`structure::analyse`) in solve order. For each block it picks a square, independent subsystem
//! (all of a square block's equations; for an over- or under-determined part, a set of independent
//! equations and as many unknowns, by pivoted QR, the rest of the equations set aside and the rest of
//! the variables held at their committed values) and applies Krawczyk's test to it: the Jacobian over a
//! box around the committed point, a preconditioner `C` from an LU inverse of the midpoint Jacobian
//! (`geom-linalg`), and the operator `K(X) = x̃ - C F(x̃) + (I - C J(X))(X - x̃)`. If `K` lies strictly
//! inside `X`, the box holds exactly one solution, and `K` holds it. That box then stands in for the
//! block's unknowns, as an interval input, in every later block, so the radius a block proves is the
//! distance from the stored geometry to the exact solution of the whole sketch so far, with free
//! variables held at their committed values.
//!
//! Guarantees:
//!
//! - **A proof or a reason.** A block is [`Certificate::Proven`] with a radius only when Krawczyk's
//!   strict inclusion held in outward-rounded interval arithmetic; otherwise it is
//!   [`Certificate::Unproven`] with the reason, and its constraints keep their residual bounds
//!   (`check`). Nothing is rounded into a proof.
//! - **Chained.** A block's equations are evaluated with every earlier block's proven box as the
//!   interval for its unknowns, never the committed point, so an upstream uncertainty widens a
//!   downstream radius instead of being forgotten.
//! - **Dependent equations are set aside, not lost.** A redundant group is certified without its
//!   dependent equations; they are listed in [`Block::set_aside`] and keep only their residual
//!   bounds.
//! - **Deterministic.** Blocks in plan order; ties by index; the same bits on every target.
//!
//! Method: R. Krawczyk, "Newton-Algorithmen zur Bestimmung von Nullstellen mit Fehlerschranken",
//! Computing 4 (1969), doi:10.1007/BF02234767, as R. E. Moore (1977) and S. M. Rump (1983) made it a
//! proof of existence and uniqueness, with the epsilon-inflation of S. M. Rump, "Verification methods:
//! rigorous results using floating-point arithmetic", Acta Numerica 19 (2010), doi:10.1017/S096249291000005X;
//! the single-step operator of `geom_num::newton` (G52) extended to blocks of any size on
//! `geom_linalg`'s dense LU and pivoted QR; chapter 15.10's "Certificate" and chapter 12.4. The
//! selection of independent equations is chapter 15.8's pivot thresholds.
//!
//! Cost: for a block of r unknowns, O(r^3) for the preconditioner and one interval Jacobian per
//! inflation step (at most ten, usually one or two); linear in the sketch for blocks of bounded size.
//! A block of more than [`BLOCK_LIMIT`] unknowns is not attempted. No recursion.

extern crate alloc;

use alloc::vec::Vec;

use geom_linalg::matrix::{Matrix, Work};
use geom_linalg::qr::PivotedQr;
use geom_linalg::solve::Lu;
use geom_num::interval::Interval;
use geom_run::WorkMeter;

use crate::check::held_of;
use crate::commit::Committed;
use crate::equation::Assembly;
use crate::model::{ItemId, Sketch, SketchError};
use crate::structure::{Analysis, Part, analyse, substitute_read};

/// The most unknowns in a block that is certified: its dense inverse costs the cube. It is the same
/// size as the largest pivoted block a solve takes whole in practice and is a bound to measure, not
/// a derived one (section 15.5 sets the cluster limit the same way).
pub const BLOCK_LIMIT: usize = 200;

/// The pivot below which a Jacobian column or row is dependent: section 15.8's independent pivot,
/// 2^-12 (after the scaling of 15.4 every entry of the Jacobian is a ratio of lengths).
const INDEPENDENT: f64 = 1.0 / 4096.0;

/// The most times a box is widened before the block is given up.
const INFLATIONS: usize = 20;

/// How many times a proven box is narrowed by applying the operator to it again.
const REFINEMENTS: usize = 4;

/// A box this far from the committed point is no proof of anything near it: give up.
const HUGE: f64 = 1.0e12;

/// Why a block could not be proven.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// More unknowns than [`BLOCK_LIMIT`].
    TooLarge,
    /// The Jacobian at the committed point cannot be inverted: the block is singular or too nearly
    /// so for its pivots to be told from zero.
    Singular,
    /// No box around the committed point was mapped strictly into itself by Krawczyk's operator in
    /// the inflations allowed: the block is too nearly singular over its box, or the committed point
    /// is too far from a solution.
    NoInclusion,
    /// The interval arithmetic refused (a range reaching a singular set, an overflow).
    Refused,
    /// No independent equation: nothing to certify.
    Empty,
}

/// What was proved of a block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Certificate {
    /// There is exactly one solution of the block's independent equations within this many grid
    /// steps (in every unknown) of the committed point, given the boxes proven upstream.
    Proven {
        /// The distance, in grid steps, from the committed values to the farthest edge of the box.
        radius: f64,
    },
    /// Not proved, for this reason.
    Unproven(Reason),
}

/// One block of the plan, certified or not.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// The constraints whose equations were certified together, ascending.
    pub rules: Vec<ItemId>,
    /// The constraints whose equations were dependent on those and set aside, ascending.
    pub set_aside: Vec<ItemId>,
    /// How many unknowns it solved for.
    pub unknowns: usize,
    /// What was proved.
    pub certificate: Certificate,
}

/// The certificates of a sketch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Certified {
    /// Every block, in the plan's order.
    pub blocks: Vec<Block>,
    /// The proven box of every unknown that was certified: its entity, its slot, and the interval.
    pub boxes: Vec<(ItemId, u8, Interval)>,
}

fn get<T: Copy>(list: &[T], i: usize) -> Result<T, SketchError> {
    list.get(i).copied().ok_or(SketchError::Internal)
}

fn cost(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

fn push<T>(list: &mut Vec<T>, value: T) -> Result<(), SketchError> {
    list.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
    list.push(value);
    Ok(())
}

/// A dense matrix of intervals, row by row.
struct Dense {
    cols: usize,
    a: Vec<Interval>,
}

impl Dense {
    fn new(rows: usize, cols: usize, fill: Interval) -> Result<Dense, SketchError> {
        let n = rows.checked_mul(cols).ok_or(SketchError::Internal)?;
        let mut a = Vec::new();
        a.try_reserve_exact(n)
            .map_err(|_| SketchError::OutOfMemory)?;
        a.resize(n, fill);
        Ok(Dense { cols, a })
    }

    fn at(&self, i: usize, j: usize) -> Result<Interval, SketchError> {
        i.checked_mul(self.cols)
            .and_then(|k| k.checked_add(j))
            .and_then(|k| self.a.get(k))
            .copied()
            .ok_or(SketchError::Internal)
    }

    fn put(&mut self, i: usize, j: usize, v: Interval) -> Result<(), SketchError> {
        let k = i
            .checked_mul(self.cols)
            .and_then(|k| k.checked_add(j))
            .ok_or(SketchError::Internal)?;
        *self.a.get_mut(k).ok_or(SketchError::Internal)? = v;
        Ok(())
    }
}

fn mid(x: Interval) -> f64 {
    x.lo() * 0.5 + x.hi() * 0.5
}

fn magnitude(x: Interval) -> f64 {
    if x.lo().abs() >= x.hi().abs() {
        x.lo().abs()
    } else {
        x.hi().abs()
    }
}

/// The residuals of `rows` and their derivatives with respect to `cols`, over `held`.
fn evaluate(
    assembly: &Assembly,
    rules: &[usize],
    held: &[Interval],
    rows: &[usize],
    cols: &[usize],
) -> Result<(Vec<Interval>, Dense), SketchError> {
    let zero = Interval::point(0.0)?;
    let mut f = Vec::new();
    f.try_reserve_exact(rows.len())
        .map_err(|_| SketchError::OutOfMemory)?;
    f.resize(rows.len(), zero);
    let mut j = Dense::new(rows.len(), cols.len(), zero)?;
    for &rule in rules {
        let jet = assembly.jet(rule, held, cols)?;
        for (e, residual) in jet.residuals.iter().enumerate() {
            let row = jet.first.checked_add(e).ok_or(SketchError::Internal)?;
            if let Ok(i) = rows.binary_search(&row) {
                *f.get_mut(i).ok_or(SketchError::Internal)? = *residual;
            }
        }
        for &(variable, e, partial) in &jet.partials {
            let row = jet.first.checked_add(e).ok_or(SketchError::Internal)?;
            if let (Ok(i), Ok(k)) = (rows.binary_search(&row), cols.binary_search(&variable)) {
                j.put(i, k, partial)?;
            }
        }
    }
    Ok((f, j))
}

/// The residuals of `rows` over `held` without derivatives, and their derivatives with respect to
/// `cols` in doubles at the midpoint of `held`: what the preconditioner and the centre's residual
/// need, at a fraction of the cost of an interval Jacobian.
fn at_centre(
    assembly: &Assembly,
    rules: &[usize],
    held: &[Interval],
    rows: &[usize],
    cols: &[usize],
    with_values: bool,
) -> Result<(Vec<Interval>, Vec<f64>), SketchError> {
    let zero = Interval::point(0.0)?;
    let mut f = Vec::new();
    f.try_reserve_exact(rows.len())
        .map_err(|_| SketchError::OutOfMemory)?;
    f.resize(rows.len(), zero);
    let mut x = Vec::new();
    x.try_reserve_exact(held.len())
        .map_err(|_| SketchError::OutOfMemory)?;
    x.extend(held.iter().map(|h| mid(*h)));
    let mut j = Vec::new();
    j.try_reserve_exact(rows.len().saturating_mul(cols.len()))
        .map_err(|_| SketchError::OutOfMemory)?;
    j.resize(rows.len().saturating_mul(cols.len()), 0.0);
    let mut point = Vec::new();
    if with_values {
        for &v in &x {
            push(&mut point, Interval::point(v)?)?;
        }
    }
    for &rule in rules {
        let first = assembly.first_row(rule)?;
        if with_values {
            for (e, residual) in assembly.rule_values(rule, &point)?.iter().enumerate() {
                let row = first.checked_add(e).ok_or(SketchError::Internal)?;
                if let Ok(i) = rows.binary_search(&row) {
                    *f.get_mut(i).ok_or(SketchError::Internal)? = *residual;
                }
            }
        }
        for (variable, e, slope) in assembly.slopes(rule, &x)? {
            let row = first.checked_add(e).ok_or(SketchError::Internal)?;
            if let (Ok(i), Ok(k)) = (rows.binary_search(&row), cols.binary_search(&variable)) {
                let at = i
                    .checked_mul(cols.len())
                    .and_then(|q| q.checked_add(k))
                    .ok_or(SketchError::Internal)?;
                *j.get_mut(at).ok_or(SketchError::Internal)? = slope;
            }
        }
    }
    Ok((f, j))
}

/// The inverse of the square matrix `entries` (row by row), or `None` if it is singular.
fn inverse(entries: &[f64], n: usize, work: &mut Work) -> Result<Option<Vec<f64>>, SketchError> {
    let lu = match Lu::factor(&Matrix::from_rows(n, n, entries)?, work) {
        Ok(lu) => lu,
        Err(geom_linalg::LinalgError::Singular) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut inv = Vec::new();
    inv.try_reserve_exact(n.saturating_mul(n))
        .map_err(|_| SketchError::OutOfMemory)?;
    inv.resize(n.saturating_mul(n), 0.0);
    let mut unit = Vec::new();
    unit.resize(n, 0.0);
    for col in 0..n {
        *unit.get_mut(col).ok_or(SketchError::Internal)? = 1.0;
        let solved = lu.solve(&unit, work)?;
        *unit.get_mut(col).ok_or(SketchError::Internal)? = 0.0;
        for (row, v) in solved.iter().enumerate() {
            let at = row
                .checked_mul(n)
                .and_then(|k| k.checked_add(col))
                .ok_or(SketchError::Internal)?;
            *inv.get_mut(at).ok_or(SketchError::Internal)? = *v;
        }
    }
    Ok(Some(inv))
}

/// The independent equations and unknowns of a part, by pivoted QR of its midpoint Jacobian: the
/// rows to keep, the columns to solve for. Others are set aside and held.
fn select(
    j: &[f64],
    rows: usize,
    cols: usize,
    work: &mut Work,
) -> Result<(Vec<usize>, Vec<usize>), SketchError> {
    // Equations: the columns of the transpose.
    let mut transposed = Vec::new();
    transposed
        .try_reserve_exact(rows.saturating_mul(cols))
        .map_err(|_| SketchError::OutOfMemory)?;
    for c in 0..cols {
        for r in 0..rows {
            transposed.push(
                *j.get(r.saturating_mul(cols).saturating_add(c))
                    .ok_or(SketchError::Internal)?,
            );
        }
    }
    let by_equation = PivotedQr::factor(&Matrix::from_rows(cols, rows, &transposed)?, work)?;
    let mut rank = 0;
    while by_equation
        .r_diagonal(rank)
        .is_some_and(|d| d.abs() > INDEPENDENT)
    {
        rank = rank.saturating_add(1);
    }
    let chosen_rows: Vec<usize> = by_equation
        .permutation()
        .iter()
        .take(rank)
        .copied()
        .collect();
    // Unknowns: the pivot columns of the chosen rows.
    let mut picked = Vec::new();
    picked
        .try_reserve_exact(rank.saturating_mul(cols))
        .map_err(|_| SketchError::OutOfMemory)?;
    for &r in &chosen_rows {
        for c in 0..cols {
            picked.push(
                *j.get(r.saturating_mul(cols).saturating_add(c))
                    .ok_or(SketchError::Internal)?,
            );
        }
    }
    let by_unknown = PivotedQr::factor(&Matrix::from_rows(rank, cols, &picked)?, work)?;
    let chosen_cols: Vec<usize> = by_unknown
        .permutation()
        .iter()
        .take(rank)
        .copied()
        .collect();
    // Positions in the part; the caller wants the rows and columns themselves.
    Ok((chosen_rows, chosen_cols))
}

/// Krawczyk's test on the square system of `rows` and `cols`, with every other variable at `held`.
/// On success the proven box of each unknown (in `cols` order) and the radius.
#[allow(
    clippy::too_many_lines,
    reason = "one straight sequence: the residual at the point, the preconditioner, the inflations"
)]
fn krawczyk(
    assembly: &Assembly,
    rules: &[usize],
    held: &[Interval],
    rows: &[usize],
    cols: &[usize],
    work: &mut Work,
) -> Result<Result<(Vec<Interval>, f64), Reason>, SketchError> {
    let n = cols.len();
    let mut local = held.to_vec();
    // The committed point, and the residuals and Jacobian there.
    let mut centre = Vec::new();
    for &c in cols {
        push(&mut centre, mid(get(held, c)?))?;
    }
    for (&c, &x) in cols.iter().zip(&centre) {
        *local.get_mut(c).ok_or(SketchError::Internal)? = Interval::point(x)?;
    }
    let (f0, j0) = at_centre(assembly, rules, &local, rows, cols, true)?;
    let Some(c_inv) = inverse(&j0, n, work)? else {
        return Ok(Err(Reason::Singular));
    };
    // b = C F(centre)
    let mut b = Vec::new();
    for i in 0..n {
        let mut acc = Interval::point(0.0)?;
        for (k, fk) in f0.iter().enumerate() {
            let cik = *c_inv
                .get(
                    i.checked_mul(n)
                        .and_then(|q| q.checked_add(k))
                        .ok_or(SketchError::Internal)?,
                )
                .ok_or(SketchError::Internal)?;
            acc = acc.add(Interval::point(cik)?.mul(*fk)?)?;
        }
        push(&mut b, acc)?;
    }
    let mut spread = 0.0_f64;
    for bi in &b {
        let m = magnitude(*bi);
        if m > spread {
            spread = m;
        }
    }
    // Rump's epsilon-inflation: start twice the Newton step wide; each time the operator's image does
    // not lie inside, take the image widened a tenth and a hair as the next box.
    // The floor follows the size of the coordinates: a double near 4e4 is 7e-12 apart from the next,
    // and a box narrower than the rounding of the residual proves nothing.
    let mut largest = 1.0_f64;
    for &x in &centre {
        if x.abs() > largest {
            largest = x.abs();
        }
    }
    let radius = spread * 2.0 + 1.0e-12 * largest;
    let mut boxes = Vec::new();
    for &x in &centre {
        push(&mut boxes, Interval::new(x - radius, x + radius)?)?;
    }
    let step = Step {
        assembly,
        rules,
        rows,
        cols,
        c_inv: &c_inv,
        centre: &centre,
        b: &b,
    };
    for _ in 0..INFLATIONS {
        let m = step.contraction(&mut local, &boxes)?;
        let image = step.apply(&m, &boxes)?;
        let mut inside = true;
        let mut reach = 0.0_f64;
        for ((k, x_box), x) in image.iter().zip(&boxes).zip(&centre) {
            if !(k.lo() > x_box.lo() && k.hi() < x_box.hi()) {
                inside = false;
            }
            let away = magnitude(k.sub(Interval::point(*x)?)?);
            if away > reach {
                reach = away;
            }
        }
        if inside {
            // The image holds the unique zero. Applying the operator to it again narrows it, with the
            // same contraction matrix (the Jacobian over the larger box encloses it over the
            // smaller): each image is inside the box before, so every one still holds the zero.
            let mut tight = image;
            for _ in 0..REFINEMENTS {
                let again = step.apply(&m, &tight)?;
                let mut narrowed = Vec::new();
                for (a, t) in again.iter().zip(&tight) {
                    let lo = if a.lo() > t.lo() { a.lo() } else { t.lo() };
                    let hi = if a.hi() < t.hi() { a.hi() } else { t.hi() };
                    if lo > hi {
                        return Err(SketchError::Internal);
                    }
                    push(&mut narrowed, Interval::new(lo, hi)?)?;
                }
                tight = narrowed;
            }
            let mut radius = 0.0_f64;
            for (k, x) in tight.iter().zip(&centre) {
                let away = magnitude(k.sub(Interval::point(*x)?)?);
                if away > radius {
                    radius = away;
                }
            }
            return Ok(Ok((tight, radius)));
        }
        if reach > HUGE {
            break;
        }
        let mut next = Vec::new();
        for k in &image {
            let slack = (k.hi() - k.lo()) * 0.1 + 1.0e-12 * (1.0 + magnitude(*k));
            push(&mut next, Interval::new(k.lo() - slack, k.hi() + slack)?)?;
        }
        boxes = next;
    }
    Ok(Err(Reason::NoInclusion))
}

/// What Krawczyk's operator needs once the committed point and its preconditioner are fixed.
struct Step<'a> {
    assembly: &'a Assembly,
    rules: &'a [usize],
    rows: &'a [usize],
    cols: &'a [usize],
    c_inv: &'a [f64],
    centre: &'a [f64],
    b: &'a [Interval],
}

impl Step<'_> {
    /// M = I - C J(X), the contraction matrix of the box `boxes`, with every other variable as it is
    /// in `local`. J is enclosed over the box and the centre together: the mean value theorem takes
    /// the Jacobian at points on the segment from the centre to each point of the box.
    fn contraction(
        &self,
        local: &mut [Interval],
        boxes: &[Interval],
    ) -> Result<Dense, SketchError> {
        let n = self.cols.len();
        for ((&c, x_box), x) in self.cols.iter().zip(boxes).zip(self.centre) {
            let lo = if *x < x_box.lo() { *x } else { x_box.lo() };
            let hi = if *x > x_box.hi() { *x } else { x_box.hi() };
            *local.get_mut(c).ok_or(SketchError::Internal)? = Interval::new(lo, hi)?;
        }
        let (_, jx) = evaluate(self.assembly, self.rules, local, self.rows, self.cols)?;
        let mut m = Dense::new(n, n, Interval::point(0.0)?)?;
        for i in 0..n {
            for j in 0..n {
                let mut a = Interval::point(0.0)?;
                for k in 0..n {
                    let at = i
                        .checked_mul(n)
                        .and_then(|q| q.checked_add(k))
                        .ok_or(SketchError::Internal)?;
                    let cik = get(self.c_inv, at)?;
                    a = a.add(Interval::point(cik)?.mul(jx.at(k, j)?)?)?;
                }
                let identity = Interval::point(if i == j { 1.0 } else { 0.0 })?;
                m.put(i, j, identity.sub(a)?)?;
            }
        }
        Ok(m)
    }

    /// K(X) = centre - b + M (X - centre) for the box `boxes`.
    fn apply(&self, m: &Dense, boxes: &[Interval]) -> Result<Vec<Interval>, SketchError> {
        let n = self.cols.len();
        let mut image = Vec::new();
        for i in 0..n {
            let mut acc = Interval::point(get(self.centre, i)?)?.sub(get(self.b, i)?)?;
            for j in 0..n {
                let offset = get(boxes, j)?.sub(Interval::point(get(self.centre, j)?)?)?;
                acc = acc.add(m.at(i, j)?.mul(offset)?)?;
            }
            push(&mut image, acc)?;
        }
        Ok(image)
    }
}

/// The distinct rule ids of the rows, ascending.
fn rule_ids(assembly: &Assembly, rows: &[usize]) -> Result<Vec<ItemId>, SketchError> {
    let mut ids = Vec::new();
    for &row in rows {
        push(&mut ids, assembly.name_of_row(row)?)?;
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

/// The distinct rules writing `rows`, ascending.
fn rules_of_rows(assembly: &Assembly, rows: &[usize]) -> Result<Vec<usize>, SketchError> {
    let mut rules = Vec::new();
    for &row in rows {
        push(&mut rules, assembly.rule_of(row)?)?;
    }
    rules.sort_unstable();
    rules.dedup();
    Ok(rules)
}

/// A block, the box it proved if any, and the unknowns it solved for.
type Attempt = (Block, Option<Vec<Interval>>, Vec<usize>);

/// Certifies one part of a component.
fn certify_part(
    assembly: &Assembly,
    held: &mut [Interval],
    part: &Part,
    work: &mut Work,
    meter: &mut WorkMeter<'_>,
) -> Result<Block, SketchError> {
    let nothing = |why: Reason| Block {
        rules: Vec::new(),
        set_aside: Vec::new(),
        unknowns: 0,
        certificate: Certificate::Unproven(why),
    };
    let (rows_all, cols_all) = (&part.equations, &part.variables);
    meter.charge(cost(rows_all.len()).saturating_add(cost(cols_all.len())))?;
    let rules = rules_of_rows(assembly, rows_all)?;
    let attempt = (|| -> Result<Attempt, SketchError> {
        // The square, independent subsystem.
        let (rows, cols) = if rows_all.len() == cols_all.len() {
            (rows_all.clone(), cols_all.clone())
        } else {
            let (_, j) = at_centre(assembly, &rules, held, rows_all, cols_all, false)?;
            let (rows_at, cols_at) = select(&j, rows_all.len(), cols_all.len(), work)?;
            let mut rows: Vec<usize> = rows_at
                .iter()
                .filter_map(|&k| rows_all.get(k).copied())
                .collect();
            let mut cols: Vec<usize> = cols_at
                .iter()
                .filter_map(|&k| cols_all.get(k).copied())
                .collect();
            rows.sort_unstable();
            cols.sort_unstable();
            (rows, cols)
        };
        if rows.is_empty() {
            return Ok((nothing(Reason::Empty), None, Vec::new()));
        }
        let set_aside: Vec<usize> = rows_all
            .iter()
            .copied()
            .filter(|r| rows.binary_search(r).is_err())
            .collect();
        let unknowns = cols.len();
        let mut block = Block {
            rules: rule_ids(assembly, &rows)?,
            set_aside: rule_ids(assembly, &set_aside)?,
            unknowns,
            certificate: Certificate::Unproven(Reason::TooLarge),
        };
        if unknowns > BLOCK_LIMIT {
            return Ok((block, None, cols));
        }
        let rules = rules_of_rows(assembly, &rows)?;
        meter.charge(
            cost(unknowns)
                .saturating_mul(cost(unknowns))
                .saturating_mul(cost(unknowns)),
        )?;
        match krawczyk(assembly, &rules, held, &rows, &cols, work)? {
            Ok((boxes, radius)) => {
                block.certificate = Certificate::Proven { radius };
                Ok((block, Some(boxes), cols))
            }
            Err(why) => {
                block.certificate = Certificate::Unproven(why);
                Ok((block, None, cols))
            }
        }
    })();
    match attempt {
        Ok((block, boxes, cols)) => {
            if let Some(boxes) = boxes {
                for (c, b) in cols.iter().zip(boxes) {
                    *held.get_mut(*c).ok_or(SketchError::Internal)? = b;
                }
            }
            Ok(block)
        }
        // The arithmetic or the linear algebra refused: this block is unproven, the others go on.
        Err(SketchError::Numeric(_) | SketchError::Linear(_)) => Ok(nothing(Reason::Refused)),
        Err(e) => Err(e),
    }
}

/// A sketch's plan and equations, built once, to certify commits of it against.
pub struct Certifier {
    analysis: Analysis,
    assembly: Assembly,
}

impl Certifier {
    /// The structural plan and the equations of `sketch`, with each row's lengths fixed at its solved
    /// values.
    ///
    /// # Errors
    ///
    /// What assembling the equations and analysing the structure refuse; [`SketchError::Stopped`]
    /// when the meter stops it.
    pub fn new(sketch: &Sketch, meter: &mut WorkMeter<'_>) -> Result<Certifier, SketchError> {
        let (substituted, read) = substitute_read(sketch)?;
        let analysis = analyse(&substituted.graph, meter)?;
        let mut assembly = Assembly::merged_read(sketch, &substituted, &read)?;
        let start = assembly.start()?;
        assembly.rescale(&start)?;
        Ok(Certifier { analysis, assembly })
    }

    /// Certifies a sketch's committed geometry, block by block in the plan's order.
    ///
    /// # Errors
    ///
    /// What reading the committed values refuses; [`SketchError::OutOfMemory`];
    /// [`SketchError::Stopped`] when the meter stops it.
    pub fn certify(
        &self,
        sketch: &Sketch,
        committed: &Committed,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Certified, SketchError> {
        let assembly = &self.assembly;
        let mut held = held_of(sketch, committed, assembly.variables())?;
        let mut done = alloc::vec![false; held.len()];
        let mut work = Work::new(u64::MAX);
        let mut out = Certified::default();
        for component in &self.analysis.components {
            let parts = core::iter::once(&component.over)
                .chain(component.blocks.iter())
                .chain(core::iter::once(&component.under));
            for part in parts {
                if part.equations.is_empty() {
                    continue;
                }
                let block = certify_part(assembly, &mut held, part, &mut work, meter)?;
                if matches!(block.certificate, Certificate::Proven { .. }) {
                    for &c in &part.variables {
                        if let Some(flag) = done.get_mut(c) {
                            *flag = true;
                        }
                    }
                }
                push(&mut out.blocks, block)?;
            }
        }
        meter.charge(u64::MAX.saturating_sub(work.left()))?;
        for (i, v) in assembly.variables().iter().enumerate() {
            if get(&done, i)? {
                push(&mut out.boxes, (v.owner, v.slot, get(&held, i)?))?;
            }
        }
        Ok(out)
    }
}

/// Certifies a sketch's committed geometry, block by block in the plan's order: [`Certifier::new`]
/// and [`Certifier::certify`] in one call.
///
/// # Errors
///
/// As for those two.
pub fn certify(
    sketch: &Sketch,
    committed: &Committed,
    meter: &mut WorkMeter<'_>,
) -> Result<Certified, SketchError> {
    Certifier::new(sketch, meter)?.certify(sketch, committed, meter)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The meter is charged for what was asked: a count, or all of it when it cannot be told.
    #[test]
    fn the_cost_is_the_count() {
        assert_eq!(cost(0), 0);
        assert_eq!(cost(7), 7);
        assert_eq!(cost(1_000), 1_000);
    }
}
