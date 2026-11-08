//! Degrees of freedom, redundancy and conflicts (piece G224; chapter 15.8).
//!
//! What it does: [`diagnose`] reads a sketch's Jacobian at its values and says how many ways the
//! sketch can still move, which equations repeat others, and which of those repeats conflict.
//! [`analyse_columns`] is the same on bare data, so the numerics are tested without a sketch.
//! [`free_motions`] gives the directions the sketch can still move in, in the canonical echelon
//! form, and [`SketchReport::text`] prints a report in words.
//!
//! - **Rank from a pivoted QR of Jᵀ**, each column one equation (Businger and Golub 1965, doi
//!   10.1007/BF01436084). A pivot is what is left of a column once the columns already chosen are
//!   taken out of it. Each step takes the column with the largest; among columns within a factor of
//!   2 of the largest it takes the lower priority number, then the older constraint. This choice is
//!   why the factorisation is written here and not taken from `geom-linalg`'s [`PivotedQr`], which
//!   pivots on the norm alone: the rule is chapter 15.8's, so the equations left over are the ones
//!   a person would expect to question.
//! - **Thresholds are derived** (15.8): a pivot of at least `σ_ind` = 2^-12 is independent; one at
//!   most `σ_dep` = 1/(64 S) for a block of S grid steps, and never below the factorisation's own
//!   rounding bound (Higham 2002, chapter 19), is dependent; between them is near-singular and is
//!   reported, never rounded either way. A pivot within a factor of 16 of either is confirmed by a
//!   singular value decomposition of the small group involved ([`Svd::jacobi`]).
//! - **Dependency groups** are found in our own words from the method the evidence report names
//!   (G01 report 06, section 5; planegcs's source is LGPL and was not read): an equation the
//!   pivoting leaves out is a combination of the chosen ones, and solving R₁₁z = r₁₂ gives the
//!   coefficients; the chosen equations whose coefficient is at least 2^-12 of the largest, with the
//!   one left out, are its group. A group is redundant when the same combination of the residuals
//!   holds within the tolerance, and conflicting when it does not.
//! - **Free motions** are the canonical null space of J ([`canonical_null_space`]).
//!
//! Guarantees:
//!
//! - **Deterministic.** Pivots tie-break on priority, then authoring order; reports list their
//!   constraints in authoring order; the report text is the same on every target.
//! - **Nothing hidden.** A pivot between the thresholds is a [`Verdict::NearSingular`], and the
//!   degrees of freedom are given as a least and a most when any are.
//! - **No panics, no recursion, no unsafe code.** Every index is checked.
//!
//! Method: section 15.8 (`projects/engines/geometry/architecture/15-sketches.md`): P. Businger and
//! G. H. Golub, "Linear least squares solutions by Householder transformations", Numer. Math. 7
//! (1965), doi:10.1007/BF01436084, for the pivoted QR; N. J. Higham, "Accuracy and Stability of
//! Numerical Algorithms", 2nd ed. (2002), chapter 19, doi:10.1137/1.9780898718027, for the rounding
//! bound on a pivot; and the dependency groups in our own words from the method G01 report 06,
//! section 5, describes.
//!
//! Cost: O(m·n·min(m, n)) multiply-adds for m equations and n variables, charged to the meter
//! before the work, then the same for the confirming decompositions of small groups.

#![allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "the names are the section's: n variables, m equations, k a step, z the coefficients"
)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

use geom_linalg::matrix::Work;
use geom_linalg::{Matrix, Svd, canonical_null_space};
use geom_num::real::Real;
use geom_num::scalar::Scalar;
use geom_run::WorkMeter;

use crate::equation::{Assembly, measure, rules_of};
use crate::model::{Constraint, ItemId, Sketch, SketchError, Standing, Variable, power_of_two};
use crate::solve::{INDEPENDENT, Solved, TOLERANCE, continue_to, solve};

/// How near a threshold a pivot must be, by this factor either way, to be confirmed by a singular
/// value decomposition (section 15.8).
pub const CONFIRM_WITHIN: f64 = 16.0;

/// A pivot within this factor of the largest column's may be passed over for a lower priority number.
const PREFERRED_WITHIN: f64 = 2.0;

/// The unit roundoff of binary64.
const UNIT_ROUNDOFF: f64 = 1.110_223_024_625_156_5e-16;

/// The verdict on one pivot (section 15.8's table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// At least `σ_ind`: the equation is independent of those chosen before it.
    Independent,
    /// Between the thresholds: reported, never rounded either way.
    NearSingular,
    /// At most `σ_dep`: the equation is a combination of those chosen before it.
    Dependent,
}

/// The thresholds of section 15.8.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    /// `σ_ind`: a pivot at least this is independent.
    pub independent: f64,
    /// `σ_dep`: a pivot at most this is dependent.
    pub dependent: f64,
    /// The residual, in steps, within which a dependent group holds.
    pub tolerance: f64,
    /// Whether `σ_dep` was raised to the factorisation's rounding bound, so that the block is too
    /// large to tell dependence at the grid's resolution.
    pub limited_by_rounding: bool,
}

impl Thresholds {
    /// The thresholds for a block of `size` grid steps whose matrix has `rows` × `columns` entries
    /// and largest column norm `largest`: `σ_dep` = 1/(64 S), raised to the rounding bound
    /// m·n·u·‖A‖ when that is larger.
    #[must_use]
    pub fn for_block(size: f64, rows: usize, columns: usize, largest: f64) -> Thresholds {
        let wanted = if size > 1.0 {
            1.0 / (64.0 * size)
        } else {
            1.0 / 64.0
        };
        let cells = count_f64(rows) * count_f64(columns);
        let rounding = cells * UNIT_ROUNDOFF * largest;
        let (dependent, limited_by_rounding) = if rounding > wanted {
            (rounding, true)
        } else {
            (wanted, false)
        };
        Thresholds {
            independent: INDEPENDENT,
            dependent: smaller(dependent, INDEPENDENT),
            tolerance: TOLERANCE,
            limited_by_rounding,
        }
    }

    /// Whether a pivot of `size` is within a factor of [`CONFIRM_WITHIN`] of either threshold,
    /// either way, so that a singular value decomposition of its group confirms its verdict.
    #[must_use]
    pub fn confirms(&self, size: f64) -> bool {
        near(size, self.independent) || near(size, self.dependent)
    }
}

/// One equation, as the diagnosis sees it: whose it is and how it is ranked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Equation {
    /// The constraint (or arc) the equation belongs to.
    pub constraint: ItemId,
    /// Its priority: lower is pivoted first and removed last.
    pub priority: u8,
    /// Its place in authoring order, for ties and for "the newest".
    pub order: usize,
}

/// One pivot of the factorisation, in the order it was chosen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pivot {
    /// The equation (a row of the Jacobian).
    pub equation: usize,
    /// What was left of its column when it was chosen.
    pub size: f64,
    /// The verdict.
    pub verdict: Verdict,
    /// Whether a singular value decomposition decided the verdict, not the pivot.
    pub confirmed: bool,
}

/// Whether a dependent group holds at the point diagnosed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// The group holds: the sketch is consistent, only repeating itself. A warning.
    Redundant,
    /// The group cannot hold. An error.
    Conflicting,
}

/// A set of equations one of which is a combination of the others (section 15.8).
#[derive(Clone, Debug, PartialEq)]
pub struct DependencyGroup {
    /// The equation left out by the pivoting.
    pub left_out: usize,
    /// Every equation of the group, ascending, the one left out included.
    pub equations: Vec<usize>,
    /// The constraints they belong to, in authoring order, each once.
    pub constraints: Vec<ItemId>,
    /// Whether the group holds.
    pub class: Class,
    /// How far the residuals miss the combination, in steps.
    pub defect: f64,
    /// The left-out constraint has other equations that are independent: only some of it repeats.
    pub partly: bool,
    /// The constraint to suggest removing, for a conflicting group.
    pub suggested_removal: Option<ItemId>,
}

/// Pivots too small to call either way, with the constraints involved.
#[derive(Clone, Debug, PartialEq)]
pub struct NearSingularGroup {
    /// The constraints involved, in authoring order.
    pub constraints: Vec<ItemId>,
    /// The pivot, the margin the verdict lacks.
    pub margin: f64,
}

/// What the factorisation says about a set of equations.
#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    /// How many variables.
    pub variables: usize,
    /// How many equations.
    pub equations: usize,
    /// The thresholds used.
    pub thresholds: Thresholds,
    /// The pivots, in the order chosen.
    pub pivots: Vec<Pivot>,
    /// How many pivots are independent.
    pub independent: usize,
    /// How many are near-singular.
    pub near_singular: usize,
    /// The dependent groups, one for each dependent equation, in pivot order.
    pub groups: Vec<DependencyGroup>,
    /// The near-singular pivots' groups.
    pub near: Vec<NearSingularGroup>,
}

impl Analysis {
    /// The fewest degrees of freedom the sketch can have: every near-singular pivot counted as
    /// independent.
    #[must_use]
    pub fn dof_least(&self) -> usize {
        self.variables
            .saturating_sub(self.independent.saturating_add(self.near_singular))
    }

    /// The most degrees of freedom: every near-singular pivot counted as dependent.
    #[must_use]
    pub fn dof_most(&self) -> usize {
        self.variables.saturating_sub(self.independent)
    }
}

/// The larger of two finite numbers.
fn larger(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// The smaller of two finite numbers.
fn smaller(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// A count as a double: exact below 2^53, and a size beyond that is past every budget.
#[allow(
    clippy::cast_precision_loss,
    reason = "a count of rows or columns, far below 2^53"
)]
fn count_f64(n: usize) -> f64 {
    n as f64
}

/// The dot product, summed in index order.
fn dot(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for (x, y) in a.iter().zip(b) {
        s += x * y;
    }
    s
}

/// The Jacobian's gradients over the variables, the residuals, and the number of variables.
type Gradients = (Vec<Vec<f64>>, Vec<f64>, usize);

fn get<T: Copy>(list: &[T], i: usize) -> Result<T, SketchError> {
    list.get(i).copied().ok_or(SketchError::Internal)
}

fn entry(list: &[Vec<f64>], j: usize, i: usize) -> Result<f64, SketchError> {
    list.get(j)
        .and_then(|c| c.get(i))
        .copied()
        .ok_or(SketchError::Internal)
}

fn root(x: f64) -> Result<f64, SketchError> {
    Ok(Real::new(x)?.sqrt()?.get())
}

/// The norm of column `j` from row `from` down: the sum of squares, in index order, of the entries
/// divided by the power of two at or below the largest. Dividing by a power of two is exact, so
/// wherever the plain sum of squares neither overflows nor loses its low end this is its root to the
/// bit, and where the plain sum would, it is still the norm.
fn tail_norm(columns: &[Vec<f64>], j: usize, from: usize) -> Result<f64, SketchError> {
    let column = columns.get(j).ok_or(SketchError::Internal)?;
    let tail = column.get(from..).ok_or(SketchError::Internal)?;
    let mut big = 0.0f64;
    for &x in tail {
        big = larger(big, x.abs());
    }
    let step = power_of_two(big);
    let mut sum = 0.0f64;
    for &x in tail {
        let s = x / step;
        sum += s * s;
    }
    Ok(root(sum)? * step)
}

/// Pivoted QR of the columns, by Householder reflections, choosing as 15.8 says. Returns the
/// reduced columns (R above the diagonal in the permuted order), the order the columns were taken
/// in, the pivot sizes, and the largest trailing norm at each step.
#[allow(clippy::type_complexity, reason = "four lists that travel together")]
fn factor(
    columns: &[Vec<f64>],
    equations: &[Equation],
    rows: usize,
) -> Result<(Vec<Vec<f64>>, Vec<usize>, Vec<f64>, Vec<f64>), SketchError> {
    let m = columns.len();
    let mut a: Vec<Vec<f64>> = columns.to_vec();
    let mut order: Vec<usize> = (0..m).collect();
    let mut sizes = Vec::new();
    let mut maxima = Vec::new();
    let steps = rows.min(m);
    // The trailing norms of the columns, computed afresh from the columns after each reflection.
    // Businger and Golub's downdate (as LAPACK's xGEQP3 keeps it) saves that pass, but lets its
    // rounding decide a choice at the edge of the factor of 2; here every choice rests on a
    // correctly rounded norm.
    let mut norms: Vec<f64> = Vec::new();
    norms
        .try_reserve_exact(m)
        .map_err(|_| SketchError::OutOfMemory)?;
    for j in 0..m {
        norms.push(tail_norm(&a, j, 0)?);
    }
    for k in 0..steps {
        let mut largest = 0.0f64;
        for &n in norms.iter().skip(k) {
            largest = larger(largest, n);
        }
        if largest == 0.0 {
            // Everything left is zero: each remaining column is dependent with pivot 0.
            for _ in k..m {
                sizes.push(0.0);
                maxima.push(0.0);
            }
            return Ok((a, order, sizes, maxima));
        }
        // The preferred column: within a factor of 2 of the largest, the lowest priority number,
        // then the oldest.
        let mut best: Option<usize> = None;
        for (offset, &n) in norms.iter().skip(k).enumerate() {
            if n * PREFERRED_WITHIN < largest {
                continue;
            }
            let j = k.checked_add(offset).ok_or(SketchError::Internal)?;
            let eq = get(equations, get(&order, j)?)?;
            let better = match best {
                None => true,
                Some(b) => {
                    let other = get(equations, get(&order, b)?)?;
                    (eq.priority, eq.order) < (other.priority, other.order)
                }
            };
            if better {
                best = Some(j);
            }
        }
        let p = best.ok_or(SketchError::Internal)?;
        a.swap(k, p);
        order.swap(k, p);
        norms.swap(k, p);
        sizes.push(tail_norm(&a, k, k)?);
        maxima.push(largest);
        // The reflector for column k from row k, applied to the columns after it.
        let alpha = entry(&a, k, k)?;
        let norm = tail_norm(&a, k, k)?;
        let beta = if alpha >= 0.0 { -norm } else { norm };
        // The reflector's vector from row k down: column k there, with alpha - beta on top.
        let mut v: Vec<f64> = a
            .get(k)
            .and_then(|c| c.get(k..))
            .ok_or(SketchError::Internal)?
            .to_vec();
        *v.first_mut().ok_or(SketchError::Internal)? = alpha - beta;
        // Divided by the power of two at or below its largest entry, so that its square neither
        // overflows nor loses its low end. The division is exact and the reflection does not depend
        // on the vector's length, so every column is reflected to the bit as without it.
        let step = power_of_two(v.iter().fold(0.0f64, |acc, x| larger(acc, x.abs())));
        for x in &mut v {
            *x /= step;
        }
        let vv = dot(&v, &v);
        if vv > 0.0 {
            for j in k..m {
                let column = a
                    .get_mut(j)
                    .and_then(|c| c.get_mut(k..))
                    .ok_or(SketchError::Internal)?;
                let scale = 2.0 * dot(&v, column) / vv;
                for (c, x) in column.iter_mut().zip(&v) {
                    *c -= scale * x;
                }
            }
        }
        // The norms of the columns after k, below row k.
        for j in k.saturating_add(1)..m {
            *norms.get_mut(j).ok_or(SketchError::Internal)? =
                tail_norm(&a, j, k.saturating_add(1))?;
        }
        // Zero below the diagonal of column k exactly; the reflector put beta on it.
        let column = a.get_mut(k).ok_or(SketchError::Internal)?;
        for x in column.iter_mut().skip(k.saturating_add(1)) {
            *x = 0.0;
        }
        *column.get_mut(k).ok_or(SketchError::Internal)? = beta;
    }
    // Columns past the number of rows have nothing left to pivot on: no row is left below the
    // last step, so each pivot is 0.
    for _ in steps..m {
        sizes.push(0.0);
        maxima.push(0.0);
    }
    Ok((a, order, sizes, maxima))
}

/// Solves R₁₁z = column `k`'s upper part by back substitution, for the first `r` pivots.
fn coefficients(a: &[Vec<f64>], r: usize, k: usize) -> Result<Vec<f64>, SketchError> {
    let mut z = Vec::new();
    z.try_reserve_exact(r)
        .map_err(|_| SketchError::OutOfMemory)?;
    z.resize(r, 0.0);
    for i in (0..r).rev() {
        let mut s = entry(a, k, i)?;
        for j in i.saturating_add(1)..r {
            s -= entry(a, j, i)? * get(&z, j)?;
        }
        let d = entry(a, i, i)?;
        let value = if d == 0.0 { 0.0 } else { s / d };
        *z.get_mut(i).ok_or(SketchError::Internal)? = value;
    }
    Ok(z)
}

/// The singular values of the columns `chosen` of `original`, smallest last.
fn smallest_singular(
    original: &[Vec<f64>],
    rows: usize,
    chosen: &[usize],
    work: &mut Work,
) -> Result<f64, SketchError> {
    let cols = chosen.len();
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(rows.saturating_mul(cols))
        .map_err(|_| SketchError::OutOfMemory)?;
    for i in 0..rows {
        for &j in chosen {
            entries.push(entry(original, j, i)?);
        }
    }
    let matrix = Matrix::from_rows(rows, cols, &entries)?;
    let svd = Svd::jacobi(&matrix, work)?;
    let mut least = f64::INFINITY;
    for &x in svd.singular_values() {
        least = smaller(least, x);
    }
    Ok(least)
}

fn verdict_of(size: f64, maximum: f64, t: &Thresholds) -> Verdict {
    if size >= t.independent {
        Verdict::Independent
    } else if maximum <= t.dependent {
        Verdict::Dependent
    } else {
        Verdict::NearSingular
    }
}

fn near(size: f64, threshold: f64) -> bool {
    size <= threshold * CONFIRM_WITHIN && size * CONFIRM_WITHIN >= threshold
}

/// Factors the equations' gradients and classes the result. `columns[e]` is equation `e`'s
/// gradient over the `rows` variables; `residuals[e]` its residual in steps.
///
/// # Errors
///
/// [`SketchError::Internal`] if the lists disagree in length; [`SketchError::Stopped`] if the
/// meter stops; [`SketchError::Linear`] and [`SketchError::Numeric`] for what the arithmetic
/// refuses; [`SketchError::OutOfMemory`].
pub fn analyse_columns(
    columns: &[Vec<f64>],
    equations: &[Equation],
    residuals: &[f64],
    rows: usize,
    size: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Analysis, SketchError> {
    let m = columns.len();
    if equations.len() != m || residuals.len() != m || columns.iter().any(|c| c.len() != rows) {
        return Err(SketchError::Internal);
    }
    let cost = (m as u64)
        .saturating_mul(rows as u64)
        .saturating_mul(rows.min(m) as u64)
        .saturating_mul(4);
    meter.charge(cost)?;
    let (a, order, sizes, maxima) = factor(columns, equations, rows)?;
    let mut biggest = 0.0f64;
    for j in 0..m {
        biggest = larger(biggest, tail_norm(columns, j, 0)?);
    }
    let thresholds = Thresholds::for_block(size, rows, m, biggest);
    let mut work = Work::new(u64::MAX);
    let mut pivots: Vec<Pivot> = Vec::new();
    pivots
        .try_reserve_exact(m)
        .map_err(|_| SketchError::OutOfMemory)?;
    // The first position that is dependent: all after it are.
    let mut first_dependent = m;
    for k in 0..m {
        let s = sizes.get(k).copied().unwrap_or(0.0);
        let top = maxima.get(k).copied().unwrap_or(0.0);
        let mut verdict = verdict_of(s, top, &thresholds);
        let mut confirmed = false;
        if k < rows
            && thresholds.confirms(s)
            && !(verdict == Verdict::Dependent && k >= first_dependent)
        {
            // Confirm on the group: the pivot's column and the chosen columns it leans on.
            let leaning = coefficients(&a, k.min(rows), k)?;
            let top_z = leaning.iter().fold(0.0f64, |acc, z| larger(acc, z.abs()));
            let mut chosen: Vec<usize> = Vec::new();
            for (i, z) in leaning.iter().enumerate() {
                if top_z > 0.0 && z.abs() >= top_z * thresholds.independent {
                    chosen.push(get(&order, i)?);
                }
            }
            chosen.push(get(&order, k)?);
            chosen.sort_unstable();
            let sigma = smallest_singular(columns, rows, &chosen, &mut work)?;
            let by_svd = if sigma >= thresholds.independent {
                Verdict::Independent
            } else if sigma <= thresholds.dependent {
                Verdict::Dependent
            } else {
                Verdict::NearSingular
            };
            if by_svd != verdict {
                verdict = by_svd;
                confirmed = true;
            }
        }
        if verdict == Verdict::Dependent && first_dependent == m {
            first_dependent = k;
        }
        pivots.push(Pivot {
            equation: get(&order, k)?,
            size: s,
            verdict,
            confirmed,
        });
    }
    meter.charge(u64::MAX.saturating_sub(work.left()))?;
    let independent = pivots
        .iter()
        .filter(|p| p.verdict == Verdict::Independent)
        .count();
    let near_singular = pivots
        .iter()
        .filter(|p| p.verdict == Verdict::NearSingular)
        .count();
    // The chosen prefix R₁₁ holds every pivot before the first dependent one.
    let r = first_dependent.min(rows);
    let mut groups: Vec<DependencyGroup> = Vec::new();
    for k in first_dependent..m {
        let left_out = get(&order, k)?;
        // With no pivot chosen (r = 0) there is nothing to lean on, and the list is empty.
        let z = coefficients(&a, r, k)?;
        let top = z.iter().fold(0.0f64, |acc, x| larger(acc, x.abs()));
        let mut members: Vec<usize> = Vec::new();
        let mut combined = 0.0f64;
        for (i, &c) in z.iter().enumerate() {
            let e = get(&order, i)?;
            combined += c * get(residuals, e)?;
            if top > 0.0 && c.abs() >= top * thresholds.independent {
                members.push(e);
            }
        }
        members.push(left_out);
        members.sort_unstable();
        members.dedup();
        let defect = (get(residuals, left_out)? - combined).abs();
        let class = if defect <= thresholds.tolerance {
            Class::Redundant
        } else {
            Class::Conflicting
        };
        let mut constraints: Vec<(usize, ItemId)> = Vec::new();
        for &e in &members {
            let q = get(equations, e)?;
            if !constraints.iter().any(|&(_, id)| id == q.constraint) {
                constraints.push((q.order, q.constraint));
            }
        }
        constraints.sort_unstable();
        let left = get(equations, left_out)?;
        // Another equation of the same constraint among the pivots before the first dependent
        // one, which are all independent or near-singular.
        let partly = (0..m).any(|e| {
            e != left_out
                && equations
                    .get(e)
                    .is_some_and(|q| q.constraint == left.constraint)
                && pivots.iter().take(first_dependent).any(|p| p.equation == e)
        });
        groups.push(DependencyGroup {
            left_out,
            equations: members,
            constraints: constraints.into_iter().map(|(_, id)| id).collect(),
            class,
            defect,
            partly,
            suggested_removal: None,
        });
    }
    suggest_removals(&mut groups, equations);
    let mut near_groups = Vec::new();
    for p in pivots.iter().filter(|p| p.verdict == Verdict::NearSingular) {
        near_groups.push(NearSingularGroup {
            constraints: alloc::vec![get(equations, p.equation)?.constraint],
            margin: p.size,
        });
    }
    Ok(Analysis {
        variables: rows,
        equations: m,
        thresholds,
        pivots,
        independent,
        near_singular,
        groups,
        near: near_groups,
    })
}

/// Chooses, for each conflicting group, the constraint to suggest removing: the highest priority
/// number first (the one the author ranked least), then the one in the most groups, then the newest,
/// then, among constraints given the same authoring order, the greater id.
fn suggest_removals(groups: &mut [DependencyGroup], equations: &[Equation]) {
    let mut count: Vec<(ItemId, usize)> = Vec::new();
    for g in groups.iter() {
        for &c in &g.constraints {
            match count.iter_mut().find(|(id, _)| *id == c) {
                Some((_, n)) => *n = n.saturating_add(1),
                None => count.push((c, 1)),
            }
        }
    }
    for g in groups.iter_mut() {
        if g.class != Class::Conflicting {
            continue;
        }
        let mut best: Option<(u8, usize, usize, ItemId)> = None;
        for &c in &g.constraints {
            let Some(q) = equations.iter().find(|q| q.constraint == c) else {
                continue;
            };
            let groups_in = count.iter().find(|(id, _)| *id == c).map_or(0, |&(_, n)| n);
            let key = (q.priority, groups_in, q.order, c);
            let better = match best {
                None => true,
                Some(b) => key > b,
            };
            if better {
                best = Some(key);
            }
        }
        g.suggested_removal = best.map(|b| b.3);
    }
}

/// What `diagnose` found for a sketch.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchReport {
    /// The numbers and groups.
    pub analysis: Analysis,
    /// The fewest degrees of freedom.
    pub dof_least: usize,
    /// The most degrees of freedom; equal to the fewest when no pivot is near-singular.
    pub dof_most: usize,
    /// Dependent groups that hold: warnings.
    pub redundant: Vec<DependencyGroup>,
    /// Dependent groups that cannot hold: errors.
    pub conflicting: Vec<DependencyGroup>,
    /// Pivots too close to call.
    pub near_singular: Vec<NearSingularGroup>,
    /// Dependence that comes from this configuration only: it appears at the point but not at the
    /// witness (the point moved by small amounts derived from the variables' keys).
    pub special: Vec<SpecialConfiguration>,
}

/// A dependence that holds only at the configuration diagnosed (section 15.8), such as three
/// points on one line under a triangle's distances.
#[derive(Clone, Debug, PartialEq)]
pub struct SpecialConfiguration {
    /// The constraints involved, in authoring order.
    pub constraints: Vec<ItemId>,
}

impl SketchReport {
    /// Whether the sketch has a conflict.
    #[must_use]
    pub fn has_conflict(&self) -> bool {
        !self.conflicting.is_empty()
    }

    /// The report in words. The text depends on the report alone, so it is the same on every
    /// target.
    #[must_use]
    pub fn text(&self) -> String {
        let mut out = String::new();
        let a = &self.analysis;
        let _ = writeln!(
            out,
            "{} variables, {} equations, {} independent",
            a.variables, a.equations, a.independent
        );
        if self.dof_least == self.dof_most {
            let _ = writeln!(out, "degrees of freedom: {}", self.dof_least);
        } else {
            let _ = writeln!(
                out,
                "degrees of freedom: from {} to {} (some pivots are too close to call)",
                self.dof_least, self.dof_most
            );
        }
        if a.thresholds.limited_by_rounding {
            let _ = writeln!(
                out,
                "the block is too large to tell dependence at the grid's resolution"
            );
        }
        for g in &self.redundant {
            let _ = writeln!(out, "redundant: {}{}", names(&g.constraints), partly(g));
        }
        for g in &self.conflicting {
            let _ = write!(out, "conflict: {}{}", names(&g.constraints), partly(g));
            if let Some(id) = g.suggested_removal {
                let _ = write!(out, "; suggest removing {}", name(id));
            }
            let _ = writeln!(out);
        }
        for sp in &self.special {
            let _ = writeln!(
                out,
                "special configuration: {} depend on each other here only",
                names(&sp.constraints)
            );
        }
        for n in &self.near_singular {
            let _ = writeln!(out, "near-singular: {}", names(&n.constraints));
        }
        out
    }
}

fn name(id: ItemId) -> String {
    let mut s = String::new();
    let _ = write!(s, "{}.{}", id.sequence, id.key);
    s
}

fn names(ids: &[ItemId]) -> String {
    let mut s = String::new();
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&name(*id));
    }
    s
}

fn partly(g: &DependencyGroup) -> &'static str {
    if g.partly {
        " (only some of one constraint's equations)"
    } else {
        ""
    }
}

/// The equations of a sketch, ranked: its rules in authoring order with each constraint's priority
/// (an arc's own rules rank first), one entry for each row of the Jacobian.
fn ranked(sketch: &Sketch, assembly: &Assembly) -> Result<Vec<Equation>, SketchError> {
    let rules = rules_of(sketch)?;
    let rows = assembly.residuals().len();
    let mut out = Vec::new();
    out.try_reserve_exact(rows)
        .map_err(|_| SketchError::OutOfMemory)?;
    for row in 0..rows {
        let rule = assembly.rule_of(row)?;
        let (id, _) = rules.get(rule).ok_or(SketchError::Internal)?;
        let priority = sketch
            .constraints()
            .iter()
            .find(|c| c.id == *id)
            .map_or(0, |c| c.priority);
        out.push(Equation {
            constraint: *id,
            priority,
            order: rule,
        });
    }
    Ok(out)
}

/// The Jacobian's columns as equations' gradients over the variables, with the residuals.
fn gradients(assembly: &Assembly) -> Result<Gradients, SketchError> {
    let dense = assembly.jacobian().to_dense()?;
    let (rows, cols) = (dense.rows(), dense.cols());
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(rows)
        .map_err(|_| SketchError::OutOfMemory)?;
    for i in 0..rows {
        columns.push(dense.row(i)?.to_vec());
    }
    Ok((columns, assembly.residuals().to_vec(), cols))
}

/// Diagnoses `sketch` at its own values: its degrees of freedom, redundant and conflicting groups
/// and near-singular pivots. A sketch of S grid steps across is judged with `size` = S.
///
/// # Errors
///
/// As [`analyse_columns`], and [`SketchError::Degenerate`] for a start on a singular set.
pub fn diagnose(
    sketch: &Sketch,
    size: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<SketchReport, SketchError> {
    let mut assembly = Assembly::new(sketch)?;
    assembly.evaluate(sketch)?;
    let equations = ranked(sketch, &assembly)?;
    let (columns, residuals, variables) = gradients(&assembly)?;
    let analysis = analyse_columns(&columns, &equations, &residuals, variables, size, meter)?;
    let dof_least = analysis.dof_least();
    let dof_most = analysis.dof_most();
    let (conflicting, redundant): (Vec<_>, Vec<_>) = analysis
        .groups
        .iter()
        .cloned()
        .partition(|g| g.class == Class::Conflicting);
    let near_singular = analysis.near.clone();
    let mut report = SketchReport {
        analysis,
        dof_least,
        dof_most,
        redundant,
        conflicting,
        near_singular,
        special: Vec::new(),
    };
    witness(sketch, &mut assembly, &equations, size, meter, &mut report)?;
    Ok(report)
}

/// The start moved by small amounts derived from each variable's owner, slot and key: a different
/// point for each sketch, the same for the same sketch on every target.
///
/// # Errors
///
/// What [`Assembly::start`] refuses, and [`SketchError::Internal`] if the lists disagree.
pub fn witness_point(assembly: &Assembly) -> Result<Vec<f64>, SketchError> {
    let mut x = assembly.start()?;
    for (v, var) in assembly.variables().iter().enumerate() {
        let h = var
            .owner
            .sequence
            .wrapping_mul(31)
            .wrapping_add(var.owner.key.wrapping_mul(17))
            .wrapping_add(u64::from(var.slot).wrapping_mul(7));
        // 1 to 61 of 2^-7 of a step.
        let amount = f64::from(u32::try_from(h % 61).unwrap_or(0).saturating_add(1)) / 128.0;
        let c = x.get_mut(v).ok_or(SketchError::Internal)?;
        *c += amount;
    }
    Ok(x)
}

/// Judges the rank again at the witness. A dependent group at the point that is not dependent at
/// the witness comes from this configuration only: it moves from the redundant groups to the
/// special ones.
fn witness(
    sketch: &Sketch,
    assembly: &mut Assembly,
    equations: &[Equation],
    size: f64,
    meter: &mut WorkMeter<'_>,
    report: &mut SketchReport,
) -> Result<(), SketchError> {
    if report.redundant.is_empty() {
        return Ok(());
    }
    let x = witness_point(assembly)?;
    // A witness on a singular set is no witness: leave the report as it is.
    if assembly.evaluate_at(&x).is_err() {
        assembly.evaluate(sketch)?;
        return Ok(());
    }
    let (columns, residuals, variables) = gradients(assembly)?;
    let at_witness = analyse_columns(&columns, equations, &residuals, variables, size, meter)?;
    let still: Vec<Vec<ItemId>> = at_witness
        .groups
        .iter()
        .map(|g| g.constraints.clone())
        .collect();
    let (kept, special): (Vec<_>, Vec<_>) = report
        .redundant
        .iter()
        .cloned()
        .partition(|g| still.contains(&g.constraints));
    report.redundant = kept;
    report.special = special
        .into_iter()
        .map(|g| SpecialConfiguration {
            constraints: g.constraints,
        })
        .collect();
    Ok(())
}

/// What a driving dimension takes when the sketch is solved without it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Remedy {
    /// The dimension.
    pub constraint: ItemId,
    /// The value it was stated with (its measure now, for one that names another item).
    pub asked: f64,
    /// What it measures at the least-change solution of the sketch without it. Never offered as
    /// the only value that works.
    pub measures: f64,
    /// The values the dimension can take, found by continuation from `measures`; `None` when the
    /// sketch with the dimension driving at `measures` could not be solved to start from.
    pub range: Option<ValueRange>,
}

/// The values a dimension can take with the rest of the sketch held (section 15.8). Each end is
/// where continuation along the one value could go no further, to [`RANGE_RESOLUTION`] of the last
/// step; `None` is an end not found within [`RANGE_STEPS`] steps, so the dimension is not bounded
/// on that side as far as the search could see.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueRange {
    /// The least value that fits.
    pub least: Option<f64>,
    /// The most value that fits.
    pub most: Option<f64>,
}

/// The first step of the range search, as a share of the value's size (or of 1 for a value near 0).
const RANGE_FIRST: f64 = 0.125;

/// Steps of the search outward from the measured value; each success doubles the next step, so the
/// search reaches 2^24 times the first step before it gives up on an end.
pub const RANGE_STEPS: u32 = 24;

/// How finely an end is found: the failing step is halved this many times.
pub const RANGE_HALVINGS: u32 = 12;

/// The share of the failing step left when an end is reported: 2^-[`RANGE_HALVINGS`].
pub const RANGE_RESOLUTION: f64 = 1.0 / 4096.0;

impl Remedy {
    /// The remedy in words, in the manner of section 15.8: what the dimension measures without the
    /// constraints it conflicts with, and the values that fit, never one value as the only answer.
    #[must_use]
    pub fn text(&self) -> String {
        let mut out = String::new();
        let _ = write!(
            out,
            "{} ({}) without its conflicts measures {}",
            name(self.constraint),
            self.asked,
            self.measures
        );
        if let Some(range) = self.range {
            match (range.least, range.most) {
                (Some(a), Some(b)) => {
                    let _ = write!(out, ", and any value from {a:.1} to {b:.1} fits");
                }
                (None, Some(b)) => {
                    let _ = write!(out, ", and any value up to {b:.1} fits");
                }
                (Some(a), None) => {
                    let _ = write!(out, ", and any value from {a:.1} up fits");
                }
                (None, None) => {}
            }
        }
        out
    }
}

/// `sketch` with dimension `id` driving, stated as `value`.
fn stated_as(sketch: &Sketch, id: ItemId, value: f64) -> Result<Sketch, SketchError> {
    let mut out = Sketch::new(*sketch.plane());
    for e in sketch.entities() {
        out.add_entity(e.clone())?;
    }
    for c in sketch.constraints() {
        let mut c = *c;
        if c.id == id {
            c.kind = c
                .kind
                .with_stated(value)
                .ok_or(SketchError::NotADimension(id))?;
            c.standing = Standing::Driving;
        }
        out.add_constraint(c)?;
    }
    Ok(out)
}

/// One continuation of `held` to dimension `id` stated as `to`: the solution, or `None` when the
/// value is refused or cannot be followed to.
fn follow(
    held: &Sketch,
    id: ItemId,
    to: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Solved>, SketchError> {
    let Ok(target) = stated_as(held, id, to) else {
        return Ok(None);
    };
    match continue_to(held, &target, meter) {
        Ok(done) => Ok(Some(done)),
        Err(SketchError::Stopped) => Err(SketchError::Stopped),
        Err(_) => Ok(None),
    }
}

/// Walks dimension `id` away from `start` (a solution with it driving at `at`) in direction `sign`
/// by continuation: steps that double after a success, then the failing step halved. The end is
/// the last value continuation could follow, or `None` after [`RANGE_STEPS`] steps.
fn walk(
    start: &Solved,
    id: ItemId,
    at: f64,
    sign: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<f64>, SketchError> {
    let mut held = start.sketch.clone();
    let mut here = at;
    let mut step = RANGE_FIRST * if at.abs() > 1.0 { at.abs() } else { 1.0 };
    for _ in 0..RANGE_STEPS {
        if let Some(done) = follow(&held, id, here + sign * step, meter)? {
            held = done.sketch;
            here += sign * step;
            step *= 2.0;
            continue;
        }
        // The end lies within `step` of `here`: halve the bracket, moving `here` on each success.
        let mut bad = step;
        for _ in 0..RANGE_HALVINGS {
            let mid = bad / 2.0;
            if let Some(done) = follow(&held, id, here + sign * mid, meter)? {
                held = done.sketch;
                here += sign * mid;
            }
            bad = mid;
        }
        return Ok(Some(here));
    }
    Ok(None)
}

/// The values dimension `id` can take, from the sketch solved without it (`relaxed`) where it
/// measures `measures`.
fn range_of(
    relaxed: &Solved,
    id: ItemId,
    measures: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ValueRange>, SketchError> {
    let Ok(driven) = stated_as(&relaxed.sketch, id, measures) else {
        return Ok(None);
    };
    let start = match solve(&driven, meter) {
        Ok(s) => s,
        Err(SketchError::Stopped) => return Err(SketchError::Stopped),
        Err(_) => return Ok(None),
    };
    let most = walk(&start, id, measures, 1.0, meter)?;
    let least = walk(&start, id, measures, -1.0, meter)?;
    Ok(Some(ValueRange { least, most }))
}

/// The sketch with dimension `id` made a reference dimension (measured, not solved).
fn without(sketch: &Sketch, id: ItemId) -> Result<Sketch, SketchError> {
    let mut out = Sketch::new(*sketch.plane());
    for e in sketch.entities() {
        out.add_entity(e.clone())?;
    }
    for c in sketch.constraints() {
        let mut c = *c;
        if c.id == id {
            c.standing = Standing::Reference;
        }
        out.add_constraint(c)?;
    }
    Ok(out)
}

/// For each of `candidates` that is a driving dimension, solves the sketch without it and measures
/// the value it then takes (section 15.8, "remedies with numbers"). A candidate whose sketch still
/// does not solve is skipped: removing that one dimension is no remedy.
///
/// # Errors
///
/// [`SketchError::Stopped`] when the budget runs out; what building the sketches refuses.
pub fn remedies(
    sketch: &Sketch,
    candidates: &[ItemId],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Remedy>, SketchError> {
    let mut out = Vec::new();
    for &id in candidates {
        let Some(c) = sketch.constraints().iter().find(|c| c.id == id) else {
            continue;
        };
        if c.standing != Standing::Driving || !c.kind.is_dimension() {
            continue;
        }
        let asked = match c.kind.stated() {
            Some(x) => x,
            None => measure(sketch, c)?,
        };
        let relaxed = without(sketch, id)?;
        match solve(&relaxed, meter) {
            Ok(done) => {
                let reference: &Constraint = done
                    .sketch
                    .constraints()
                    .iter()
                    .find(|k| k.id == id)
                    .ok_or(SketchError::Internal)?;
                let measures = measure(&done.sketch, reference)?;
                let range = range_of(&done, id, measures, meter)?;
                out.push(Remedy {
                    constraint: id,
                    asked,
                    measures,
                    range,
                });
            }
            Err(SketchError::Stopped) => return Err(SketchError::Stopped),
            Err(_) => {}
        }
    }
    Ok(out)
}

/// What a checked solve found.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Converged. Redundant groups, special configurations and near-singular groups are warnings
    /// in the report.
    Solved {
        /// The solved sketch.
        solved: Box<Solved>,
        /// Its diagnosis, at the solution.
        report: SketchReport,
    },
    /// The equations conflict: nothing changes.
    Conflict {
        /// The diagnosis, at the sketch's own values.
        report: SketchReport,
        /// Each conflicting dimension solved without.
        remedies: Vec<Remedy>,
    },
    /// No solution was found from this start (never "none exists"). Nothing changes.
    NotConverged {
        /// The rule the solver could not satisfy.
        worst: ItemId,
        /// The diagnosis, at the sketch's own values.
        report: SketchReport,
        /// Every driving dimension solved without.
        remedies: Vec<Remedy>,
    },
}

/// Solves `sketch` and diagnoses it: a solved sketch comes back with its report, a conflict with the
/// groups, the suggested removal and the dimensions' remedies, and a solve that stops above
/// tolerance as `NotConverged` with the remedy search over every driving dimension. A sketch of
/// S grid steps across is judged with `size` = S.
///
/// # Errors
///
/// [`SketchError::Stopped`] and what the solve and the diagnosis refuse other than a block that will
/// not converge, which is an [`Outcome`].
pub fn solve_checked(
    sketch: &Sketch,
    size: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Outcome, SketchError> {
    match solve(sketch, meter) {
        Ok(mut solved) => {
            let report = diagnose(&solved.sketch, size, meter)?;
            if report.has_conflict() {
                let ids = conflicting_ids(&report);
                let remedies = remedies(sketch, &ids, meter)?;
                return Ok(Outcome::Conflict { report, remedies });
            }
            solved.report.diagnosis = Some(Box::new(report.clone()));
            Ok(Outcome::Solved {
                solved: Box::new(solved),
                report,
            })
        }
        Err(SketchError::Unsolved(worst)) => {
            let report = diagnose(sketch, size, meter)?;
            let mut ids: Vec<ItemId> = if report.has_conflict() {
                conflicting_ids(&report)
            } else {
                sketch
                    .constraints()
                    .iter()
                    .filter(|c| c.kind.is_dimension())
                    .map(|c| c.id)
                    .collect()
            };
            ids.sort_unstable();
            ids.dedup();
            let remedies = remedies(sketch, &ids, meter)?;
            if report.has_conflict() {
                Ok(Outcome::Conflict { report, remedies })
            } else {
                Ok(Outcome::NotConverged {
                    worst,
                    report,
                    remedies,
                })
            }
        }
        Err(e) => Err(e),
    }
}

/// The constraints of the conflicting groups, each once, in id order.
fn conflicting_ids(report: &SketchReport) -> Vec<ItemId> {
    let mut ids: Vec<ItemId> = report
        .conflicting
        .iter()
        .flat_map(|g| g.constraints.iter().copied())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// One direction in which the sketch can still move.
#[derive(Clone, Debug, PartialEq)]
pub struct Motion {
    /// The variables that move, in the Jacobian's column order, with how far per unit of the
    /// leading variable, in model units.
    pub moves: Vec<(ItemId, u8, f64)>,
}

/// Entries of a null-space row at most this in magnitude are left out of a motion: they are the
/// factorisation's rounding, not a move.
pub const MOTION_CUTOFF: f64 = 1e-12;

impl Motion {
    /// The motion one row of the null space gives, over `variables` in the same order: each
    /// variable whose entry is above [`MOTION_CUTOFF`] in magnitude, in model units (the solver
    /// sees value / scale, so a unit of its variable is `scale` model units).
    #[must_use]
    pub fn of_row(row: &[f64], variables: &[Variable]) -> Motion {
        let mut moves = Vec::new();
        for (&x, var) in row.iter().zip(variables) {
            if x.abs() > MOTION_CUTOFF {
                moves.push((var.owner, var.slot, x * var.scale));
            }
        }
        Motion { moves }
    }

    /// The owners of the variables this motion moves, each once, in order.
    #[must_use]
    pub fn owners(&self) -> Vec<ItemId> {
        let mut out: Vec<ItemId> = Vec::new();
        for &(id, _, _) in &self.moves {
            if !out.contains(&id) {
                out.push(id);
            }
        }
        out
    }
}

/// The directions the sketch can still move in, from the canonical null space of its Jacobian
/// (section 15.8): one [`Motion`] for each degree of freedom, each with a 1 at its own leading
/// variable and 0 at the others'. Amounts are in model units, not the solver's scaled ones.
///
/// # Errors
///
/// As [`diagnose`].
pub fn free_motions(
    sketch: &Sketch,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Motion>, SketchError> {
    let mut assembly = Assembly::new(sketch)?;
    assembly.evaluate(sketch)?;
    let dense = assembly.jacobian().to_dense()?;
    let variables: Vec<Variable> = assembly.variables().to_vec();
    let mut work = Work::new(u64::MAX);
    let basis = canonical_null_space(&dense, Real::new(INDEPENDENT)?, &mut work)?;
    meter.charge(u64::MAX.saturating_sub(work.left()))?;
    let mut out = Vec::new();
    for c in 0..basis.rows() {
        let mut row = Vec::new();
        for v in 0..variables.len() {
            row.push(basis.get(c, v)?);
        }
        out.push(Motion::of_row(&row, &variables));
    }
    Ok(out)
}
