//! Sparse QR with Householder reflections, its analysis kept apart from its numbers, and the
//! least-squares solution of least norm.
//!
//! What it does: [`SymbolicQr`] reads a pattern: the column order, the elimination tree of Aᵀ·A and
//! the pattern of R. It is computed once for every matrix of that pattern (chapter 15.7).
//! [`SparseQr::factor`] then factors one matrix, column by column from the left: each column takes
//! the reflections of the earlier columns R says reach it, and makes its own on the rows it holds.
//! A row enters at the first column it touches, and each column hands the rows it did not use as its
//! pivot to its parent in the tree. A column with nothing left, to the rank tolerance, is dead: it
//! makes no reflection and uses no pivot, and its rows go on up, so A·Π = Q·R with R a staircase.
//! [`SparseQr::least_squares`] solves A·x = b in the least-squares sense with the least norm. On a
//! staircase, and on any under-determined system, it factors Rᵀ too and solves through that. Where
//! solvi fixes its rows in its symbolic step and fails on a rank-deficient matrix, a dead column here
//! hands its rows on and the solve stays least-norm.
//!
//! Guarantees:
//!
//! - **Deterministic.** Plain scalar loops in a fixed order, the order and the tree coming from the
//!   pattern alone; a column's pivot is the lowest-numbered row it holds (chapter 15.6).
//! - **Least change, never basic.** The solution has the least Euclidean norm among all the
//!   least-squares solutions of the kept rows: no free variable is set to zero by its place in the
//!   order. It equals the dense path, [`crate::Cod::least_squares`], to rounding, while both find
//!   the same rank.
//! - **Rank decided once, and only roughly.** A column is dead when what is left of it is at most
//!   `tol` times the largest column of A. There is no column pivoting, so this can miss a near rank
//!   deficiency that column pivoting would find, which is why chapter 15.8's verdicts rest on the
//!   dense path.
//! - **Never the normal equations.** Aᵀ·A is never formed; its tree is read from A's rows.
//! - **Charged first.** [`SymbolicQr::new`] charges [`crate::colamd::colamd`]'s cost and 2·z more;
//!   [`SparseQr::factor`] charges 2·(Σⱼ |R₍ⱼ₎|·|V₍ⱼ₎| + Σⱼ |V₍ⱼ₎|) from the pattern alone, V the
//!   Householder vectors, and a staircase the second factorisation's own; a solve charges
//!   2·(|V| + |R|).
//!
//! Method: the elimination tree of Aᵀ·A, read from A's rows, J. R. Gilbert, X. S. Li, E. G. Ng and
//! B. W. Peyton, "Computing row and column counts for sparse QR and LU factorization", BIT 41(4)
//! (2001), doi:10.1023/A:1021943902025; R's pattern as the row subtrees of that tree, J. W. H. Liu,
//! "The role of elimination trees in sparse factorization", SIAM J. Matrix Anal. Appl. 11(1) (1990),
//! doi:10.1137/0611010; left-looking Householder QR with rows handed up the tree, in the structure of
//! solvi's `decomposition/sparse/qr.rs`; dead columns and the staircase, M. T. Heath, "Some extensions
//! of an algorithm for sparse linear least squares problems", SIAM J. Sci. Stat. Comput. 3(2) (1982),
//! doi:10.1137/0903014; the minimum-norm solve through the QR of Rᵀ, G. H. Golub and C. F. Van Loan,
//! *Matrix Computations*, 4th ed., Johns Hopkins (2013), section 5.6.
//!
//! Consulted: solvi (Linebender), `Apache-2.0 OR MIT`,
//!   <https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/solvi/src/decomposition/sparse/qr.rs>,
//!   for its left-looking structure and the handing of rows up the tree.
//!
//! Cost: as charged; O(|R| + |V| + m + n) memory. No recursion.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the books': m × n for the shape, k and j for columns, i for rows, x, y and z vectors, r for the rank"
)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;

use geom_num::real::Real;

use crate::colamd::colamd;
use crate::matrix::{LinalgError, Work, finite, wide, zeros};
use crate::qr::{householder, norm};
use crate::sparse::{Sparse, after, at, filled, push, slot};

/// No column: the root of the tree's parent, or the first column of an empty row.
const NONE: usize = usize::MAX;

/// A sparse matrix's analysis: what every matrix of one pattern shares.
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolicQr {
    rows: usize,
    starts: Vec<usize>,
    indices: Vec<usize>,
    /// Column k of A·Π is column `order[k]` of A.
    order: Vec<usize>,
    /// Each row's first column in that order, or [`NONE`].
    first: Vec<usize>,
    /// Each column's parent in the elimination tree of (A·Π)ᵀ·(A·Π), or [`NONE`].
    parent: Vec<usize>,
    /// Column k's earlier columns in R, ascending, at `r_starts[k]..r_starts[k + 1]`.
    r_starts: Vec<usize>,
    r_rows: Vec<usize>,
    /// What [`SparseQr::factor`] charges.
    cost: u64,
}

/// The tree and each row's first column, from A's rows: two rows share a clique in Aᵀ·A, so each
/// column joins the column before it in each of its rows, with the paths compressed.
fn tree(a: &Sparse, order: &[usize]) -> Result<(Vec<usize>, Vec<usize>), LinalgError> {
    let n = order.len();
    let mut parent = filled(n, NONE)?;
    let mut ancestor = filled(n, NONE)?;
    let mut previous = filled(a.rows(), NONE)?;
    let mut first = filled(a.rows(), NONE)?;
    for (k, &column) in order.iter().enumerate() {
        let (rows, _) = a.column(column)?;
        for &i in rows {
            let mut j = at(&previous, i)?;
            if j == NONE {
                *slot(&mut first, i)? = k;
            }
            while j != NONE && j < k {
                let up = at(&ancestor, j)?;
                *slot(&mut ancestor, j)? = k;
                if up == NONE {
                    *slot(&mut parent, j)? = k;
                }
                j = up;
            }
            *slot(&mut previous, i)? = k;
        }
    }
    Ok((parent, first))
}

/// R's pattern, column by column: from each row's first column up the tree to k.
fn r_pattern(
    a: &Sparse,
    order: &[usize],
    first: &[usize],
    parent: &[usize],
) -> Result<(Vec<usize>, Vec<usize>), LinalgError> {
    let n = order.len();
    let mut starts = filled(after(n)?, 0)?;
    let mut pattern = Vec::new();
    let mut seen = filled(n, NONE)?;
    for (k, &column) in order.iter().enumerate() {
        let (rows, _) = a.column(column)?;
        let from = pattern.len();
        for &i in rows {
            let mut j = at(first, i)?;
            while j < k && at(&seen, j)? != k {
                *slot(&mut seen, j)? = k;
                push(&mut pattern, j)?;
                j = at(parent, j)?;
            }
        }
        pattern
            .get_mut(from..)
            .ok_or(LinalgError::Defect)?
            .sort_unstable();
        *slot(&mut starts, after(k)?)? = pattern.len();
    }
    Ok((starts, pattern))
}

impl SymbolicQr {
    /// The analysis of `a`'s pattern, its columns ordered by [`crate::colamd::colamd`].
    ///
    /// # Errors
    ///
    /// [`LinalgError::OutOfWork`]; [`LinalgError::OutOfMemory`].
    pub fn new(a: &Sparse, work: &mut Work) -> Result<SymbolicQr, LinalgError> {
        let order = colamd(a, work)?;
        SymbolicQr::in_order(a, order, work)
    }

    /// The analysis of `a`'s pattern, its columns in their own order.
    ///
    /// # Errors
    ///
    /// As [`SymbolicQr::new`].
    pub fn natural(a: &Sparse, work: &mut Work) -> Result<SymbolicQr, LinalgError> {
        let mut order = Vec::new();
        order
            .try_reserve_exact(a.cols())
            .map_err(|_| LinalgError::OutOfMemory)?;
        order.extend(0..a.cols());
        SymbolicQr::in_order(a, order, work)
    }

    fn in_order(a: &Sparse, order: Vec<usize>, work: &mut Work) -> Result<SymbolicQr, LinalgError> {
        work.charge(wide(a.nonzeros()).saturating_mul(2))?;
        let (parent, first) = tree(a, &order)?;
        let (r_starts, r_rows) = r_pattern(a, &order, &first, &parent)?;
        let cost = cost(&first, &parent, &r_starts, &r_rows)?;
        let (starts, indices) = a.pattern();
        Ok(SymbolicQr {
            rows: a.rows(),
            starts: starts.to_vec(),
            indices: indices.to_vec(),
            order,
            first,
            parent,
            r_starts,
            r_rows,
            cost,
        })
    }

    /// The column order Π: column k of A·Π is column `order()[k]` of A.
    #[must_use]
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    /// How many entries R has above its diagonal, at most.
    #[must_use]
    pub fn r_entries(&self) -> usize {
        self.r_rows.len()
    }
}

/// What a factorisation of this pattern costs: each column's reflection on its rows, and each
/// reflection on each later column R says it reaches, the rows counted as they are handed up.
fn cost(
    first: &[usize],
    parent: &[usize],
    r_starts: &[usize],
    r_rows: &[usize],
) -> Result<u64, LinalgError> {
    let n = parent.len();
    let mut held = filled(n, 0_usize)?;
    for &k in first {
        if k != NONE {
            let count = slot(&mut held, k)?;
            *count = after(*count)?;
        }
    }
    let mut total = 0_u64;
    for k in 0..n {
        let size = at(&held, k)?;
        total = total.saturating_add(wide(size).saturating_mul(2));
        let up = at(parent, k)?;
        if up != NONE {
            let count = slot(&mut held, up)?;
            *count = count.saturating_add(size.saturating_sub(1));
        }
    }
    for k in 0..n {
        let (lo, hi) = (at(r_starts, k)?, at(r_starts, after(k)?)?);
        for &j in r_rows.get(lo..hi).ok_or(LinalgError::Defect)? {
            total = total.saturating_add(wide(at(&held, j)?).saturating_mul(2));
        }
    }
    Ok(total)
}

/// One matrix's factors: A·Π = Q·R, R a staircase if any column is dead.
#[derive(Clone, Debug, PartialEq)]
pub struct SparseQr {
    rows: usize,
    order: Vec<usize>,
    /// Column k's pivot row, or [`NONE`] for a dead column.
    pivot: Vec<usize>,
    /// Column k's reflection: its rows, the pivot first, and v on them, the pivot's 1 stored.
    v_starts: Vec<usize>,
    v_rows: Vec<usize>,
    v_values: Vec<f64>,
    tau: Vec<f64>,
    /// Column k's entries above the diagonal: the live columns j and R(j, k).
    r_starts: Vec<usize>,
    r_rows: Vec<usize>,
    r_values: Vec<f64>,
    /// Column k's diagonal, 0 for a dead column.
    diagonal: Vec<f64>,
    rank: usize,
    /// The QR of Rᵀ, when some column is dead.
    second: Option<Box<SparseQr>>,
}

/// H·x, for the reflection with `rows` and `values` and its `tau`.
fn reflect(rows: &[usize], values: &[f64], tau: f64, x: &mut [f64]) -> Result<(), LinalgError> {
    let mut dot = 0.0;
    for (&i, &v) in rows.iter().zip(values) {
        dot += v * at(x, i)?;
    }
    let f = tau * dot;
    for (&i, &v) in rows.iter().zip(values) {
        let held = slot(x, i)?;
        *held = finite(*held - f * v)?;
    }
    Ok(())
}

impl SparseQr {
    /// Factors `a`, whose pattern `symbolic` read, with columns at most `tol` of its largest dead.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `a` has another pattern; [`LinalgError::NotFinite`] on overflow;
    /// [`LinalgError::OutOfWork`]; [`LinalgError::OutOfMemory`].
    pub fn factor(
        symbolic: &SymbolicQr,
        a: &Sparse,
        tol: Real,
        work: &mut Work,
    ) -> Result<SparseQr, LinalgError> {
        let mut qr = SparseQr::plain(symbolic, a, tol, work)?;
        if qr.rank < a.cols() {
            let transpose = qr.staircase_transpose()?;
            let symbolic = SymbolicQr::new(&transpose, work)?;
            let second = SparseQr::plain(&symbolic, &transpose, Real::ZERO, work)?;
            // Each row of R starts at a diagonal that was not dead, so its rows are independent. A
            // column lost here is rounding on the edge of the rank.
            if second.rank < transpose.cols() {
                return Err(LinalgError::Singular);
            }
            qr.second = Some(Box::new(second));
        }
        Ok(qr)
    }

    /// The factors of `a`, on a staircase if any column is dead.
    fn plain(
        symbolic: &SymbolicQr,
        a: &Sparse,
        tol: Real,
        work: &mut Work,
    ) -> Result<SparseQr, LinalgError> {
        if !a.has_pattern(symbolic.rows, &symbolic.starts, &symbolic.indices) {
            return Err(LinalgError::Shape);
        }
        work.charge(symbolic.cost)?;
        let (m, n) = (a.rows(), a.cols());
        let mut largest = 0.0_f64;
        for j in 0..n {
            let (_, values) = a.column(j)?;
            let size = norm(values.iter().copied())?;
            if size > largest {
                largest = size;
            }
        }
        let floor = tol.get().abs() * largest;
        let mut qr = SparseQr {
            rows: m,
            order: symbolic.order.clone(),
            pivot: filled(n, NONE)?,
            v_starts: filled(after(n)?, 0)?,
            v_rows: Vec::new(),
            v_values: Vec::new(),
            tau: zeros(n)?,
            r_starts: filled(after(n)?, 0)?,
            r_rows: Vec::new(),
            r_values: Vec::new(),
            diagonal: zeros(n)?,
            rank: 0,
            second: None,
        };
        let mut x = zeros(m)?;
        let mut held: Vec<Vec<usize>> = filled(n, Vec::new())?;
        for (i, &k) in symbolic.first.iter().enumerate() {
            if k != NONE {
                push(slot(&mut held, k)?, i)?;
            }
        }
        for k in 0..n {
            let (rows, values) = a.column(at(&symbolic.order, k)?)?;
            for (&i, &value) in rows.iter().zip(values) {
                *slot(&mut x, i)? = value;
            }
            qr.take_earlier(symbolic, k, &mut x)?;
            let mut mine = core::mem::take(held.get_mut(k).ok_or(LinalgError::Defect)?);
            mine.sort_unstable();
            let handed = qr.reflect_column(k, &mine, floor, &mut x)?;
            let up = at(&symbolic.parent, k)?;
            if up != NONE {
                let list = held.get_mut(up).ok_or(LinalgError::Defect)?;
                for &i in handed {
                    push(list, i)?;
                }
            }
            *slot(&mut qr.v_starts, after(k)?)? = qr.v_rows.len();
            *slot(&mut qr.r_starts, after(k)?)? = qr.r_rows.len();
            for &i in &mine {
                *slot(&mut x, i)? = 0.0;
            }
        }
        Ok(qr)
    }

    /// The earlier reflections R says reach column k, and its entries in their pivot rows.
    fn take_earlier(
        &mut self,
        symbolic: &SymbolicQr,
        k: usize,
        x: &mut [f64],
    ) -> Result<(), LinalgError> {
        let (lo, hi) = (
            at(&symbolic.r_starts, k)?,
            at(&symbolic.r_starts, after(k)?)?,
        );
        for &j in symbolic.r_rows.get(lo..hi).ok_or(LinalgError::Defect)? {
            let pivot = at(&self.pivot, j)?;
            if pivot == NONE {
                continue;
            }
            let (rows, values) = self.reflection(j)?;
            let tau = at(&self.tau, j)?;
            reflect(rows, values, tau, x)?;
            let entry = core::mem::take(slot(x, pivot)?);
            push(&mut self.r_rows, j)?;
            push(&mut self.r_values, entry)?;
        }
        Ok(())
    }

    /// Makes column k's reflection on the rows it holds, or finds it dead. Gives back the rows to
    /// hand up.
    fn reflect_column<'r>(
        &mut self,
        k: usize,
        mine: &'r [usize],
        floor: f64,
        x: &mut [f64],
    ) -> Result<&'r [usize], LinalgError> {
        let mut part = zeros(mine.len())?;
        for (value, &i) in part.iter_mut().zip(mine) {
            *value = at(x, i)?;
        }
        let left = norm(part.iter().copied())?;
        let Some((&pivot, rest)) = mine.split_first() else {
            return Ok(mine);
        };
        if left <= floor || left == 0.0 {
            return Ok(mine);
        }
        let (tau, beta) = householder(&mut part)?;
        *slot(&mut self.tau, k)? = tau;
        *slot(&mut self.diagonal, k)? = beta;
        *slot(&mut self.pivot, k)? = pivot;
        self.rank = after(self.rank)?;
        for (place, (&i, &value)) in mine.iter().zip(&part).enumerate() {
            push(&mut self.v_rows, i)?;
            push(&mut self.v_values, if place == 0 { 1.0 } else { value })?;
        }
        Ok(rest)
    }

    /// Column k's reflection: its rows and v.
    fn reflection(&self, k: usize) -> Result<(&[usize], &[f64]), LinalgError> {
        let (lo, hi) = (at(&self.v_starts, k)?, at(&self.v_starts, after(k)?)?);
        Ok((
            self.v_rows.get(lo..hi).ok_or(LinalgError::Defect)?,
            self.v_values.get(lo..hi).ok_or(LinalgError::Defect)?,
        ))
    }

    /// Column k's entries above the diagonal.
    fn entries(&self, k: usize) -> Result<(&[usize], &[f64]), LinalgError> {
        let (lo, hi) = (at(&self.r_starts, k)?, at(&self.r_starts, after(k)?)?);
        Ok((
            self.r_rows.get(lo..hi).ok_or(LinalgError::Defect)?,
            self.r_values.get(lo..hi).ok_or(LinalgError::Defect)?,
        ))
    }

    /// Rᵀ: one column for each live column of R, on rows that are the columns of A·Π.
    fn staircase_transpose(&self) -> Result<Sparse, LinalgError> {
        let n = self.order.len();
        let mut live = filled(n, NONE)?;
        let mut count = 0_usize;
        for k in 0..n {
            if at(&self.pivot, k)? != NONE {
                *slot(&mut live, k)? = count;
                count = after(count)?;
            }
        }
        let mut triplets = Vec::new();
        for k in 0..n {
            let (above, values) = self.entries(k)?;
            for (&j, &value) in above.iter().zip(values) {
                push(&mut triplets, (k, at(&live, j)?, value))?;
            }
            let place = at(&live, k)?;
            if place != NONE {
                push(&mut triplets, (k, place, at(&self.diagonal, k)?))?;
            }
        }
        Sparse::from_triplets(n, count, &triplets)
    }

    /// The rank found: how many columns are not dead.
    #[must_use]
    pub const fn rank(&self) -> usize {
        self.rank
    }

    /// The column order Π.
    #[must_use]
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    /// The smallest pivot: the least |R[k, k]|, and 0 if a column is dead.
    #[must_use]
    pub fn smallest_pivot(&self) -> f64 {
        if self.rank < self.diagonal.len() {
            return 0.0;
        }
        let mut least = f64::INFINITY;
        for pivot in &self.diagonal {
            if pivot.abs() < least {
                least = pivot.abs();
            }
        }
        least
    }

    /// What a solve costs.
    fn solve_cost(&self) -> u64 {
        wide(self.v_rows.len())
            .saturating_add(wide(self.r_rows.len()))
            .saturating_mul(2)
    }

    /// Qᵀ·c.
    fn apply_qt(&self, c: &mut [f64]) -> Result<(), LinalgError> {
        for k in 0..self.order.len() {
            if at(&self.pivot, k)? != NONE {
                let (rows, values) = self.reflection(k)?;
                reflect(rows, values, at(&self.tau, k)?, c)?;
            }
        }
        Ok(())
    }

    /// Q·c.
    fn apply_q(&self, c: &mut [f64]) -> Result<(), LinalgError> {
        for k in (0..self.order.len()).rev() {
            if at(&self.pivot, k)? != NONE {
                let (rows, values) = self.reflection(k)?;
                reflect(rows, values, at(&self.tau, k)?, c)?;
            }
        }
        Ok(())
    }

    /// The solution y of R·y = (Qᵀ·b) on a full staircase, by columns from the last.
    fn back_substitute(&self, c: &[f64]) -> Result<Vec<f64>, LinalgError> {
        let n = self.order.len();
        let mut top = zeros(n)?;
        for k in 0..n {
            *slot(&mut top, k)? = at(c, at(&self.pivot, k)?)?;
        }
        for k in (0..n).rev() {
            let y = finite(at(&top, k)? / at(&self.diagonal, k)?)?;
            *slot(&mut top, k)? = y;
            let (above, values) = self.entries(k)?;
            for (&j, &value) in above.iter().zip(values) {
                let held = slot(&mut top, j)?;
                *held = finite(*held - value * y)?;
            }
        }
        Ok(top)
    }

    /// The least-norm z of Aᵀ·z = `rhs` for this QR of A, with `rhs` one value per column of A and
    /// every column live: z = Q·R⁻ᵀ·Πᵀ·rhs.
    fn transpose_solve(&self, rhs: &[f64]) -> Result<Vec<f64>, LinalgError> {
        let n = self.order.len();
        let mut y = zeros(n)?;
        // Rᵀ·y = Πᵀ·rhs, from the first column on.
        for k in 0..n {
            let mut sum = at(rhs, at(&self.order, k)?)?;
            let (above, values) = self.entries(k)?;
            for (&j, &value) in above.iter().zip(values) {
                sum -= value * at(&y, j)?;
            }
            *slot(&mut y, k)? = finite(sum / at(&self.diagonal, k)?)?;
        }
        let mut z = zeros(self.rows)?;
        for k in 0..n {
            *slot(&mut z, at(&self.pivot, k)?)? = at(&y, k)?;
        }
        self.apply_q(&mut z)?;
        Ok(z)
    }

    /// The least-squares solution of A·x = b of least norm, charging the factors' 2·(|V| + |R|), and
    /// that of Rᵀ's when any column is dead.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `b` is not one value per row; [`LinalgError::NotFinite`];
    /// [`LinalgError::OutOfWork`]; [`LinalgError::OutOfMemory`].
    pub fn least_squares(&self, b: &[f64], work: &mut Work) -> Result<Vec<f64>, LinalgError> {
        if b.len() != self.rows {
            return Err(LinalgError::Shape);
        }
        work.charge(self.solve_cost())?;
        let n = self.order.len();
        let mut c = zeros(self.rows)?;
        c.copy_from_slice(b);
        self.apply_qt(&mut c)?;
        let y = if let Some(second) = &self.second {
            work.charge(second.solve_cost())?;
            let mut top = Vec::new();
            top.try_reserve_exact(self.rank)
                .map_err(|_| LinalgError::OutOfMemory)?;
            for k in 0..n {
                let pivot = at(&self.pivot, k)?;
                if pivot != NONE {
                    top.push(at(&c, pivot)?);
                }
            }
            second.transpose_solve(&top)?
        } else {
            self.back_substitute(&c)?
        };
        let mut x = zeros(n)?;
        for (k, &column) in self.order.iter().enumerate() {
            *slot(&mut x, column)? = at(&y, k)?;
        }
        Ok(x)
    }
}
