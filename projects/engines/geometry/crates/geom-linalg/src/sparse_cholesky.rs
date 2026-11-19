//! Sparse Cholesky and L·D·Lᵀ of symmetric matrices, the analysis kept apart from the numbers.
//!
//! What it does: [`SymbolicCholesky`] reads a symmetric pattern: the order, the elimination tree,
//! and the pattern of each row of L. It is computed once for every matrix of that pattern (chapter
//! 15.7). [`SparseCholesky::factor`] gives P·A·Pᵀ = L·Lᵀ, and [`SparseLdlt::factor`] gives
//! P·A·Pᵀ = L·D·Lᵀ with L unit lower triangular and D diagonal, for a symmetric quasi-definite
//! matrix: neither pivots, so the order is the pattern's alone. Each row of L is solved in turn
//! against the rows above it.
//!
//! Guarantees:
//!
//! - **Deterministic.** Plain scalar loops in a fixed order, and an order from the pattern alone
//!   (chapter 15.6).
//! - **Symmetric, or refused.** A matrix that is not its own transpose, entry for entry, is refused:
//!   nothing is silently read from one triangle.
//! - **Refused, never approximate.** A pivot at or below zero stops Cholesky, and a zero pivot stops
//!   L·D·Lᵀ: there is no shifting and no pivoting behind the caller's back.
//! - **Charged first.** The analysis charges [`crate::colamd::symmetric`]'s cost and 2·z more; a
//!   factorisation Σⱼ |L₍ⱼ₎|², from the pattern alone; a solve 2·|L|.
//!
//! Method: the elimination tree and row subtrees, J. W. H. Liu, "The role of elimination trees in
//! sparse factorization", SIAM J. Matrix Anal. Appl. 11(1) (1990), doi:10.1137/0611010; the up-looking
//! factorisation by rows, T. A. Davis, "Algorithm 849: a concise sparse Cholesky factorization
//! package", ACM TOMS 31(4) (2005), doi:10.1145/1114268.1114277, read for its method only; L·D·Lᵀ of
//! quasi-definite matrices without pivoting, R. J. Vanderbei, "Symmetric quasidefinite matrices",
//! SIAM J. Optim. 5(1) (1995), doi:10.1137/0805005.
//!
//! Cost: as charged; O(|L| + n) memory. No recursion.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the papers': n the size, k and j for rows and columns, i a row, x and y vectors, d the pivots"
)]

extern crate alloc;

use alloc::vec::Vec;

use geom_num::math;

use crate::colamd::symmetric;
use crate::matrix::{LinalgError, Work, finite, wide, zeros};
use crate::sparse::{Sparse, after, at, filled, push, slot};

/// No parent: a root of the tree.
const NONE: usize = usize::MAX;

/// A symmetric matrix's analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolicCholesky {
    rows: usize,
    starts: Vec<usize>,
    indices: Vec<usize>,
    /// Row k of P·A·Pᵀ is row `order[k]` of A.
    order: Vec<usize>,
    /// Where each original row went.
    place: Vec<usize>,
    /// Each column's parent in the elimination tree, or [`NONE`].
    parent: Vec<usize>,
    /// Each column of L's size, below the diagonal, and where it starts.
    l_starts: Vec<usize>,
    cost: u64,
}

impl SymbolicCholesky {
    /// The analysis of `a`'s pattern.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] for a matrix that is not square; [`LinalgError::OutOfWork`];
    /// [`LinalgError::OutOfMemory`].
    pub fn new(a: &Sparse, work: &mut Work) -> Result<SymbolicCholesky, LinalgError> {
        let order = symmetric(a, work)?;
        work.charge(wide(a.nonzeros()).saturating_mul(2))?;
        let n = order.len();
        let mut place = filled(n, 0)?;
        for (k, &i) in order.iter().enumerate() {
            *slot(&mut place, i)? = k;
        }
        let parent = tree(a, &order, &place)?;
        let mut counts = filled(n, 0_usize)?;
        let mut seen = filled(n, NONE)?;
        for (k, &column) in order.iter().enumerate() {
            let (rows, _) = a.column(column)?;
            for &row in rows {
                let mut j = at(&place, row)?;
                while j < k && at(&seen, j)? != k {
                    *slot(&mut seen, j)? = k;
                    let count = slot(&mut counts, j)?;
                    *count = after(*count)?;
                    j = at(&parent, j)?;
                }
            }
        }
        let mut l_starts = filled(after(n)?, 0)?;
        let mut cost = 0_u64;
        for j in 0..n {
            let count = at(&counts, j)?;
            let before: usize = at(&l_starts, j)?;
            let total = before.checked_add(count).ok_or(LinalgError::Defect)?;
            *slot(&mut l_starts, after(j)?)? = total;
            let wider = wide(count).saturating_add(1);
            cost = cost.saturating_add(wider.saturating_mul(wider));
        }
        let (starts, indices) = a.pattern();
        Ok(SymbolicCholesky {
            rows: a.rows(),
            starts: starts.to_vec(),
            indices: indices.to_vec(),
            order,
            place,
            parent,
            l_starts,
            cost,
        })
    }

    /// The order: row k of P·A·Pᵀ is row `order()[k]` of A.
    #[must_use]
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    /// How many entries L has below its diagonal.
    #[must_use]
    pub fn l_entries(&self) -> usize {
        self.l_starts.last().copied().unwrap_or(0)
    }
}

/// The elimination tree of P·A·Pᵀ, each entry above the diagonal joining its row's path to k.
fn tree(a: &Sparse, order: &[usize], place: &[usize]) -> Result<Vec<usize>, LinalgError> {
    let n = order.len();
    let mut parent = filled(n, NONE)?;
    let mut ancestor = filled(n, NONE)?;
    for (k, &column) in order.iter().enumerate() {
        let (rows, _) = a.column(column)?;
        for &row in rows {
            let mut i = at(place, row)?;
            while i != NONE && i < k {
                let up = at(&ancestor, i)?;
                *slot(&mut ancestor, i)? = k;
                if up == NONE {
                    *slot(&mut parent, i)? = k;
                }
                i = up;
            }
        }
    }
    Ok(parent)
}

/// `a`'s transpose is `a`, entry for entry.
fn check_symmetric(a: &Sparse) -> Result<(), LinalgError> {
    if *a == a.transpose()? {
        Ok(())
    } else {
        Err(LinalgError::NotSymmetric)
    }
}

/// The lower factor by columns, each column's rows ascending, and the pivots.
#[derive(Clone, Debug, PartialEq)]
struct Lower {
    order: Vec<usize>,
    starts: Vec<usize>,
    rows: Vec<usize>,
    values: Vec<f64>,
    /// How many of each column's entries are filled so far.
    filled: Vec<usize>,
    pivots: Vec<f64>,
}

/// How a row's pivot comes out of what is left of its diagonal, and a multiplier from a value.
trait Pivoting {
    /// The stored pivot, from what is left of the diagonal.
    fn pivot(left: f64) -> Result<f64, LinalgError>;
    /// The multiplier L(k, j) from the value y(j) and pivot j.
    fn multiplier(y: f64, pivot: f64) -> f64;
    /// What L(k, j) and y(j) take off row k's diagonal.
    fn take(multiplier: f64, y: f64) -> f64;
    /// What column j's entries subtract through: L(i, j) times this.
    fn through(multiplier: f64, y: f64) -> f64;
}

/// L·Lᵀ: the pivots are L's diagonal.
struct Root;

impl Pivoting for Root {
    fn pivot(left: f64) -> Result<f64, LinalgError> {
        if left > 0.0 {
            Ok(math::sqrt(left)?.get())
        } else {
            Err(LinalgError::NotPositiveDefinite)
        }
    }
    fn multiplier(y: f64, pivot: f64) -> f64 {
        y / pivot
    }
    fn take(multiplier: f64, _y: f64) -> f64 {
        multiplier * multiplier
    }
    fn through(multiplier: f64, _y: f64) -> f64 {
        multiplier
    }
}

/// L·D·Lᵀ: the pivots are D.
struct Plain;

impl Pivoting for Plain {
    fn pivot(left: f64) -> Result<f64, LinalgError> {
        if left == 0.0 {
            Err(LinalgError::Singular)
        } else {
            Ok(left)
        }
    }
    fn multiplier(y: f64, pivot: f64) -> f64 {
        y / pivot
    }
    fn take(multiplier: f64, y: f64) -> f64 {
        multiplier * y
    }
    fn through(_multiplier: f64, y: f64) -> f64 {
        y
    }
}

impl Lower {
    /// Factors by rows: row k of L solves against the rows above it, the columns its row subtree
    /// reaches, in ascending order, which every child keeps before its parent.
    fn factor<P: Pivoting>(
        symbolic: &SymbolicCholesky,
        a: &Sparse,
        work: &mut Work,
    ) -> Result<Lower, LinalgError> {
        if !a.has_pattern(symbolic.rows, &symbolic.starts, &symbolic.indices) {
            return Err(LinalgError::Shape);
        }
        check_symmetric(a)?;
        work.charge(symbolic.cost)?;
        let n = symbolic.order.len();
        let entries = symbolic.l_starts.last().copied().unwrap_or(0);
        let mut lower = Lower {
            order: symbolic.order.clone(),
            starts: symbolic.l_starts.clone(),
            rows: filled(entries, 0)?,
            values: zeros(entries)?,
            filled: filled(n, 0)?,
            pivots: zeros(n)?,
        };
        let mut x = zeros(n)?;
        let mut seen = filled(n, NONE)?;
        let mut pattern: Vec<usize> = Vec::new();
        for k in 0..n {
            let (rows, values) = a.column(at(&symbolic.order, k)?)?;
            pattern.clear();
            let mut left = 0.0;
            for (&row, &value) in rows.iter().zip(values) {
                let i = at(&symbolic.place, row)?;
                if i == k {
                    left = value;
                } else if i < k {
                    *slot(&mut x, i)? = value;
                    let mut j = i;
                    while j < k && at(&seen, j)? != k {
                        *slot(&mut seen, j)? = k;
                        push(&mut pattern, j)?;
                        j = at(&symbolic.parent, j)?;
                    }
                }
            }
            pattern.sort_unstable();
            for &j in &pattern {
                left -= lower.eliminate::<P>(j, k, &mut x)?;
            }
            *slot(&mut lower.pivots, k)? = P::pivot(finite(left)?)?;
        }
        Ok(lower)
    }

    /// Row k's entry in column j: takes y(j) from `x`, stores the multiplier, updates `x` through the
    /// column so far, and gives back what it takes off the diagonal.
    fn eliminate<P: Pivoting>(
        &mut self,
        j: usize,
        k: usize,
        x: &mut [f64],
    ) -> Result<f64, LinalgError> {
        let y = core::mem::take(slot(x, j)?);
        let pivot = at(&self.pivots, j)?;
        let multiplier = finite(P::multiplier(y, pivot))?;
        let through = P::through(multiplier, y);
        let lo = at(&self.starts, j)?;
        let used = at(&self.filled, j)?;
        let hi = lo.checked_add(used).ok_or(LinalgError::Defect)?;
        let rows = self.rows.get(lo..hi).ok_or(LinalgError::Defect)?;
        let values = self.values.get(lo..hi).ok_or(LinalgError::Defect)?;
        for (&i, &value) in rows.iter().zip(values) {
            let held = slot(x, i)?;
            *held = finite(*held - value * through)?;
        }
        *slot(&mut self.rows, hi)? = k;
        *slot(&mut self.values, hi)? = multiplier;
        *slot(&mut self.filled, j)? = after(used)?;
        Ok(P::take(multiplier, y))
    }

    /// Column j: its rows and values.
    fn column(&self, j: usize) -> Result<(&[usize], &[f64]), LinalgError> {
        let (lo, hi) = (at(&self.starts, j)?, at(&self.starts, after(j)?)?);
        Ok((
            self.rows.get(lo..hi).ok_or(LinalgError::Defect)?,
            self.values.get(lo..hi).ok_or(LinalgError::Defect)?,
        ))
    }

    /// `b`, in the factor's order.
    fn ordered(&self, b: &[f64], work: &mut Work) -> Result<Vec<f64>, LinalgError> {
        if b.len() != self.order.len() {
            return Err(LinalgError::Shape);
        }
        work.charge(wide(self.rows.len()).saturating_mul(2))?;
        let mut y = zeros(b.len())?;
        for (place, &i) in self.order.iter().enumerate() {
            *slot(&mut y, place)? = at(b, i)?;
        }
        Ok(y)
    }

    /// L·z = y, from the first column, dividing by the diagonal if `divide`.
    fn forward(&self, y: &mut [f64], divide: bool) -> Result<(), LinalgError> {
        for j in 0..self.order.len() {
            let held = slot(y, j)?;
            if divide {
                *held = finite(*held / at(&self.pivots, j)?)?;
            }
            let value = *held;
            let (rows, values) = self.column(j)?;
            for (&i, &entry) in rows.iter().zip(values) {
                let below = slot(y, i)?;
                *below = finite(*below - entry * value)?;
            }
        }
        Ok(())
    }

    /// Lᵀ·z = y, from the last column, dividing by the diagonal if `divide`.
    fn backward(&self, y: &mut [f64], divide: bool) -> Result<(), LinalgError> {
        for j in (0..self.order.len()).rev() {
            let mut sum = at(y, j)?;
            let (rows, values) = self.column(j)?;
            for (&i, &entry) in rows.iter().zip(values) {
                sum -= entry * at(y, i)?;
            }
            *slot(y, j)? = finite(if divide {
                sum / at(&self.pivots, j)?
            } else {
                sum
            })?;
        }
        Ok(())
    }

    /// `y`, back in the original order.
    fn restored(&self, y: &[f64]) -> Result<Vec<f64>, LinalgError> {
        let mut x = zeros(y.len())?;
        for (place, &i) in self.order.iter().enumerate() {
            *slot(&mut x, i)? = at(y, place)?;
        }
        Ok(x)
    }
}

/// P·A·Pᵀ = L·Lᵀ for a symmetric positive definite A.
#[derive(Clone, Debug, PartialEq)]
pub struct SparseCholesky {
    lower: Lower,
}

impl SparseCholesky {
    /// Factors `a`, whose pattern `symbolic` read.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `a` has another pattern; [`LinalgError::NotSymmetric`];
    /// [`LinalgError::NotPositiveDefinite`]; [`LinalgError::NotFinite`];
    /// [`LinalgError::OutOfWork`]; [`LinalgError::OutOfMemory`].
    pub fn factor(
        symbolic: &SymbolicCholesky,
        a: &Sparse,
        work: &mut Work,
    ) -> Result<SparseCholesky, LinalgError> {
        Ok(SparseCholesky {
            lower: Lower::factor::<Root>(symbolic, a, work)?,
        })
    }

    /// The solution of A·x = b, charging 2·|L|.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `b` is the wrong length; [`LinalgError::NotFinite`];
    /// [`LinalgError::OutOfWork`]; [`LinalgError::OutOfMemory`].
    pub fn solve(&self, b: &[f64], work: &mut Work) -> Result<Vec<f64>, LinalgError> {
        let mut y = self.lower.ordered(b, work)?;
        self.lower.forward(&mut y, true)?;
        self.lower.backward(&mut y, true)?;
        self.lower.restored(&y)
    }
}

/// P·A·Pᵀ = L·D·Lᵀ for a symmetric quasi-definite A.
#[derive(Clone, Debug, PartialEq)]
pub struct SparseLdlt {
    lower: Lower,
}

impl SparseLdlt {
    /// Factors `a`, whose pattern `symbolic` read.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `a` has another pattern; [`LinalgError::NotSymmetric`];
    /// [`LinalgError::Singular`] for a zero pivot; [`LinalgError::NotFinite`];
    /// [`LinalgError::OutOfWork`]; [`LinalgError::OutOfMemory`].
    pub fn factor(
        symbolic: &SymbolicCholesky,
        a: &Sparse,
        work: &mut Work,
    ) -> Result<SparseLdlt, LinalgError> {
        Ok(SparseLdlt {
            lower: Lower::factor::<Plain>(symbolic, a, work)?,
        })
    }

    /// The pivots D, in the factor's order.
    #[must_use]
    pub fn pivots(&self) -> &[f64] {
        &self.lower.pivots
    }

    /// The solution of A·x = b, charging 2·|L|.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `b` is the wrong length; [`LinalgError::NotFinite`];
    /// [`LinalgError::OutOfWork`]; [`LinalgError::OutOfMemory`].
    pub fn solve(&self, b: &[f64], work: &mut Work) -> Result<Vec<f64>, LinalgError> {
        let mut y = self.lower.ordered(b, work)?;
        self.lower.forward(&mut y, false)?;
        for (value, &pivot) in y.iter_mut().zip(&self.lower.pivots) {
            *value = finite(*value / pivot)?;
        }
        self.lower.backward(&mut y, false)?;
        self.lower.restored(&y)
    }
}
