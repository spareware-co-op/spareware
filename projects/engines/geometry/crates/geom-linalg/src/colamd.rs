//! Column ordering by approximate minimum degree: the order to eliminate a sparse matrix's columns
//! in, so that the factors of its QR or Cholesky decomposition stay sparse.
//!
//! What it does: [`colamd`] orders the columns of a matrix A for the Cholesky factor of Aᵀ·A, which is
//! the R of its QR, without forming Aᵀ·A. Each row of A is a clique of the columns it touches. The
//! column of least approximate degree is eliminated; the rows it touches merge into one new row, the
//! pivot row, whose columns get new scores; rows the pivot row covers are absorbed, and columns left
//! with the same rows merge into one supercolumn and are ordered together. [`symmetric`] orders a
//! symmetric matrix the same way, through a matrix with one row for each pair of neighbours.
//!
//! Guarantees:
//!
//! - **Structure only.** The order depends on the pattern alone, so it is computed once for every
//!   matrix of one pattern (chapter 15.7).
//! - **Deterministic.** The least score goes first and a tie goes to the lowest variable; supercolumns
//!   are found by sorted comparison, and no hash or address decides anything (chapter 15.6).
//! - **A permutation.** Every column appears once, and the empty columns and the dense ones last, in
//!   variable order.
//! - **Charged first.** It charges (z + n)·(n + 1) multiply-adds before it starts, z the stored
//!   entries and n the columns: each elimination visits each column's rows at most.
//!
//! Method: T. A. Davis, J. R. Gilbert, S. I. Larimore and E. G. Ng, "A column approximate minimum
//! degree ordering algorithm", ACM TOMS 30(3) (2004), doi:10.1145/1024074.1024079: its scores, its
//! row absorption, its supercolumns and its dense rows, in our own words and in safe Rust, with ties
//! broken by variable order (chapter 15.6). It is written from the paper, not from `colamd.c`, so its
//! lists are plain vectors and it has no packed workspace and no garbage collection. It leaves out
//! the paper's final postorder of the assembly tree, which changes neither the fill nor the work.
//!
//! Cost: as charged in the worst case, and near linear in z on the matrices a sketch makes; O(z + n)
//! memory for the lists. No recursion.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the paper's: c a column, r a row, n the columns, k the position"
)]

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::matrix::{LinalgError, Work, wide};
use crate::sparse::{Sparse, after, at, filled, push, slot};

/// A row denser than this many entries, or 10·√n if that is more, is ignored: as the paper does, for
/// a row whose clique touches nearly everything says nothing about the order.
const DENSE_FLOOR: usize = 16;

/// Whether a row or column of `len` entries among `n` is dense: over 16, and over 10·√n.
fn dense(len: usize, n: usize) -> bool {
    len > DENSE_FLOOR
        && wide(len)
            .checked_mul(wide(len))
            .is_none_or(|square| square > wide(n).saturating_mul(100))
}

/// What has become of a column.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    /// Waiting to be ordered.
    Live,
    /// Part of the supercolumn of this column.
    Merged,
    /// Ordered, or set aside for the end.
    Done,
}

/// The working lists: rows of columns and columns of rows, the rows absorbed and the columns' states.
struct Lists {
    /// The columns of each row, stale ones among them until they are next read.
    rows: Vec<Vec<usize>>,
    /// The rows of each column, sorted.
    cols: Vec<Vec<usize>>,
    /// Whether each row is still alive.
    alive: Vec<bool>,
    /// Each row's total of live column weights.
    size: Vec<usize>,
    state: Vec<State>,
    /// How many original columns each supercolumn stands for.
    weight: Vec<usize>,
    /// The columns merged into each supercolumn.
    merged: Vec<Vec<usize>>,
}

/// Row `r`'s pattern as lists, with its dense rows left out, its empty and dense columns set aside
/// in `last`.
fn read(a: &Sparse, last: &mut Vec<usize>) -> Result<Lists, LinalgError> {
    let (m, n) = (a.rows(), a.cols());
    let mut rows: Vec<Vec<usize>> = filled(m, Vec::new())?;
    let mut state = filled(n, State::Live)?;
    for c in 0..n {
        let (entries, _) = a.column(c)?;
        if entries.is_empty() || dense(entries.len(), m) {
            *slot(&mut state, c)? = State::Done;
            push(last, c)?;
        } else {
            for &r in entries {
                push(slot(&mut rows, r)?, c)?;
            }
        }
    }
    let mut cols: Vec<Vec<usize>> = filled(n, Vec::new())?;
    let mut alive = filled(m, false)?;
    let mut size = filled(m, 0)?;
    for (r, row) in rows.iter_mut().enumerate() {
        if dense(row.len(), n) {
            row.clear();
            continue;
        }
        *slot(&mut alive, r)? = !row.is_empty();
        *slot(&mut size, r)? = row.len();
        for &c in row.iter() {
            push(slot(&mut cols, c)?, r)?;
        }
    }
    for c in 0..n {
        let empty = cols.get(c).is_none_or(Vec::is_empty);
        if empty && at(&state, c)? == State::Live {
            // Its only rows were dense.
            *slot(&mut state, c)? = State::Done;
            push(last, c)?;
        }
    }
    Ok(Lists {
        rows,
        cols,
        alive,
        size,
        state,
        weight: filled(n, 1)?,
        merged: filled(n, Vec::new())?,
    })
}

impl Lists {
    /// The paper's first score: the sum over a column's rows of their sizes less one.
    fn first_score(&self, c: usize, limit: usize) -> Result<usize, LinalgError> {
        let mut score = 0_usize;
        for &r in self.cols.get(c).ok_or(LinalgError::Defect)? {
            score = score.saturating_add(at(&self.size, r)?.saturating_sub(1));
        }
        Ok(score.min(limit))
    }

    /// The pivot row of column `c`: the live columns of all its rows but `c`, ascending. Those rows
    /// die with it.
    fn pivot_row(&mut self, c: usize) -> Result<Vec<usize>, LinalgError> {
        let mut pattern = Vec::new();
        let rows = core::mem::take(self.cols.get_mut(c).ok_or(LinalgError::Defect)?);
        for &r in &rows {
            let row = self.rows.get(r).ok_or(LinalgError::Defect)?;
            for &j in row {
                if j != c && at(&self.state, j)? == State::Live {
                    push(&mut pattern, j)?;
                }
            }
            *slot(&mut self.alive, r)? = false;
        }
        pattern.sort_unstable();
        pattern.dedup();
        Ok(pattern)
    }

    /// Merges columns of `pattern` with the same rows, the lowest first, and drops the rest from it.
    /// Two columns with one list of rows are one column to every later step.
    fn merge_alike(&mut self, pattern: &mut Vec<usize>) -> Result<(), LinalgError> {
        let mut by_rows: Vec<usize> = pattern.clone();
        let cols = &self.cols;
        by_rows.sort_by(|a, b| cols.get(*a).cmp(&cols.get(*b)).then(a.cmp(b)));
        let mut held: Option<usize> = None;
        for &j in &by_rows {
            match held {
                Some(first) if self.cols.get(first) == self.cols.get(j) => {
                    let extra = at(&self.weight, j)?;
                    let total = slot(&mut self.weight, first)?;
                    *total = total.checked_add(extra).ok_or(LinalgError::Defect)?;
                    *slot(&mut self.state, j)? = State::Merged;
                    let list = self.merged.get_mut(first).ok_or(LinalgError::Defect)?;
                    push(list, j)?;
                }
                _ => held = Some(j),
            }
        }
        let state = &self.state;
        pattern.retain(|j| state.get(*j) == Some(&State::Live));
        Ok(())
    }
}

/// The order of `a`'s columns: position k of the result is the column to eliminate kth.
///
/// # Errors
///
/// [`LinalgError::OutOfWork`]; [`LinalgError::OutOfMemory`].
pub fn colamd(a: &Sparse, work: &mut Work) -> Result<Vec<usize>, LinalgError> {
    let n = a.cols();
    let cost = wide(a.nonzeros())
        .saturating_add(wide(n))
        .saturating_mul(wide(n).saturating_add(1));
    work.charge(cost)?;
    let mut last = Vec::new();
    let mut lists = read(a, &mut last)?;
    let mut scores: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut score = filled(n, 0)?;
    for c in 0..n {
        if at(&lists.state, c)? == State::Live {
            let s = lists.first_score(c, n.saturating_sub(1))?;
            *slot(&mut score, c)? = s;
            scores.insert((s, c));
        }
    }
    let mut order = Vec::new();
    order
        .try_reserve_exact(n)
        .map_err(|_| LinalgError::OutOfMemory)?;
    // Marks for the rows, a fresh mark for each pivot; `outside` is valid where the mark is current.
    let mut mark = filled(a.rows(), 0_usize)?;
    let mut outside = filled(a.rows(), 0_usize)?;
    let mut pivot = 0_usize;
    // How many original columns are ordered so far.
    let mut placed = 0_usize;
    while let Some((_, c)) = scores.pop_first() {
        pivot = after(pivot)?;
        placed = placed.saturating_add(at(&lists.weight, c)?);
        *slot(&mut lists.state, c)? = State::Done;
        order.push(c);
        // Reuse the column's first row for the pivot row: it is about to die.
        let keep = lists.cols.get(c).and_then(|rows| rows.first()).copied();
        let mut pattern = lists.pivot_row(c)?;
        let Some(p) = keep else { continue };
        for &j in &pattern {
            scores.remove(&(at(&score, j)?, j));
            // Dead rows go now, so the lists hold only rows that still say something.
            let alive = &lists.alive;
            let rows = lists.cols.get_mut(j).ok_or(LinalgError::Defect)?;
            rows.retain(|r| alive.get(*r) == Some(&true));
        }
        // Each row's part outside the pivot row, in weights.
        for &j in &pattern {
            let weight = at(&lists.weight, j)?;
            for &r in lists.cols.get(j).ok_or(LinalgError::Defect)? {
                if at(&mark, r)? != pivot {
                    *slot(&mut mark, r)? = pivot;
                    *slot(&mut outside, r)? = at(&lists.size, r)?;
                }
                let part = slot(&mut outside, r)?;
                *part = part.saturating_sub(weight);
            }
        }
        // A row wholly inside the pivot row is absorbed: it adds nothing to any column's degree.
        for &j in &pattern {
            let (alive, outer) = (&mut lists.alive, &outside);
            let rows = lists.cols.get_mut(j).ok_or(LinalgError::Defect)?;
            for &r in rows.iter() {
                if outer.get(r) == Some(&0) {
                    *slot(alive, r)? = false;
                }
            }
            let alive = &lists.alive;
            rows.retain(|r| alive.get(*r) == Some(&true));
        }
        let mut total = 0_usize;
        for &j in &pattern {
            total = total.saturating_add(at(&lists.weight, j)?);
            let rows = lists.cols.get_mut(j).ok_or(LinalgError::Defect)?;
            push(rows, p)?;
            rows.sort_unstable();
        }
        *slot(&mut lists.alive, p)? = true;
        *slot(&mut lists.size, p)? = total;
        lists.merge_alike(&mut pattern)?;
        let left = n.saturating_sub(placed);
        for &j in &pattern {
            // The paper's approximate external degree: the pivot row without j, and each other
            // row's part outside the pivot row, capped by the columns left.
            let weight = at(&lists.weight, j)?;
            let mut degree = total.saturating_sub(weight);
            for &r in lists.cols.get(j).ok_or(LinalgError::Defect)? {
                if r != p {
                    degree = degree.saturating_add(at(&outside, r)?);
                }
            }
            let s = degree.min(left.saturating_sub(weight));
            *slot(&mut score, j)? = s;
            scores.insert((s, j));
        }
        *slot(&mut lists.rows, p)? = pattern;
    }
    expand(&lists, &order, &last)
}

/// `order` with each supercolumn opened up after its first column, and `last` on the end. A
/// supercolumn may hold others that merged into a merged column, so the list is walked as it grows.
fn expand(lists: &Lists, order: &[usize], last: &[usize]) -> Result<Vec<usize>, LinalgError> {
    let mut out = Vec::new();
    out.try_reserve_exact(lists.state.len())
        .map_err(|_| LinalgError::OutOfMemory)?;
    for &c in order {
        let from = out.len();
        out.push(c);
        let mut walked = from;
        while let Some(&held) = out.get(walked) {
            let group = lists.merged.get(held).ok_or(LinalgError::Defect)?;
            let mut sorted = group.clone();
            sorted.sort_unstable();
            for &j in &sorted {
                push(&mut out, j)?;
            }
            walked = after(walked)?;
        }
    }
    for &c in last {
        push(&mut out, c)?;
    }
    Ok(out)
}

/// The order of a symmetric `a`'s rows and columns: [`colamd`] of a matrix with one row for each
/// entry above the diagonal, whose product with its transpose has `a`'s pattern (the paper's SYMAMD).
/// Only the pattern above the diagonal is read.
///
/// # Errors
///
/// [`LinalgError::Shape`] for a matrix that is not square; as [`colamd`].
pub fn symmetric(a: &Sparse, work: &mut Work) -> Result<Vec<usize>, LinalgError> {
    if a.rows() != a.cols() {
        return Err(LinalgError::Shape);
    }
    let mut triplets = Vec::new();
    let mut edge = 0_usize;
    for j in 0..a.cols() {
        let (rows, _) = a.column(j)?;
        for &i in rows {
            if i < j {
                push(&mut triplets, (edge, i, 1.0))?;
                push(&mut triplets, (edge, j, 1.0))?;
                edge = after(edge)?;
            }
        }
    }
    // A column with no neighbour has no entry in any edge row, so it goes last, as an empty column.
    colamd(&Sparse::from_triplets(edge, a.cols(), &triplets)?, work)
}
