//! Sparse matrices of finite doubles, stored by columns, with the checked steps every sparse
//! factorisation is written in.
//!
//! What it does: [`Sparse`] is a matrix in compressed sparse column form, each column's row indices
//! ascending. It is made from triplets in any order; repeated positions add up, in the order given.
//! An entry that is stored stays stored when its value is zero, so a matrix's pattern depends on the
//! positions given and never on the values: a factorisation's analysis can be reused for every
//! matrix of one pattern (chapter 15.7).
//!
//! Guarantees:
//!
//! - **Finite.** Every value is finite; a constructor refuses NaN and infinities with
//!   [`LinalgError::NotFinite`], and a sum that overflows is refused the same way.
//! - **Canonical.** One pattern and one set of values have one representation, whatever order the
//!   triplets came in, so two matrices are equal exactly when `==` says so.
//! - **No panic, no abort.** Indices are checked; allocations are fallible.
//!
//! Method: compressed sparse column storage, I. S. Duff, A. M. Erisman and J. K. Reid, *Direct
//! Methods for Sparse Matrices*, 2nd ed., Oxford (2017), doi:10.1093/acprof:oso/9780198508380.001.0001,
//! section 2.
//!
//! Cost: `n` columns and `z` entries take O(n + z) memory; construction sorts each column, O(z log z).

extern crate alloc;

use alloc::vec::Vec;

use crate::matrix::{LinalgError, Matrix, finite, zeros};

/// `n` copies of `value`, fallibly allocated.
pub(crate) fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>, LinalgError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| LinalgError::OutOfMemory)?;
    v.resize(n, value);
    Ok(v)
}

/// `value` on the end of `list`, fallibly allocated.
pub(crate) fn push<T>(list: &mut Vec<T>, value: T) -> Result<(), LinalgError> {
    list.try_reserve(1).map_err(|_| LinalgError::OutOfMemory)?;
    list.push(value);
    Ok(())
}

/// Entry `i` of `list`.
pub(crate) fn at<T: Copy>(list: &[T], i: usize) -> Result<T, LinalgError> {
    list.get(i).copied().ok_or(LinalgError::Defect)
}

/// Entry `i` of `list`, to change.
pub(crate) fn slot<T>(list: &mut [T], i: usize) -> Result<&mut T, LinalgError> {
    list.get_mut(i).ok_or(LinalgError::Defect)
}

/// `i` + 1.
pub(crate) fn after(i: usize) -> Result<usize, LinalgError> {
    i.checked_add(1).ok_or(LinalgError::Defect)
}

/// A sparse matrix of finite doubles, stored by columns.
#[derive(Clone, Debug, PartialEq)]
pub struct Sparse {
    rows: usize,
    cols: usize,
    /// Column j's entries are at `starts[j]..starts[j + 1]`.
    starts: Vec<usize>,
    /// Each entry's row, ascending within a column.
    indices: Vec<usize>,
    values: Vec<f64>,
}

impl Sparse {
    /// The `rows` × `cols` matrix with `triplets` (row, column, value) added up in the order given.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] for a position outside the matrix; [`LinalgError::NotFinite`] for a
    /// value or a sum that is not finite; [`LinalgError::OutOfMemory`].
    pub fn from_triplets(
        rows: usize,
        cols: usize,
        triplets: &[(usize, usize, f64)],
    ) -> Result<Sparse, LinalgError> {
        let mut sorted = Vec::new();
        sorted
            .try_reserve_exact(triplets.len())
            .map_err(|_| LinalgError::OutOfMemory)?;
        for (place, &(row, col, value)) in triplets.iter().enumerate() {
            if row >= rows || col >= cols {
                return Err(LinalgError::Shape);
            }
            sorted.push((col, row, place, finite(value)?));
        }
        // The place keeps repeated positions in the order given, so their sum is the same whatever
        // the sort does.
        sorted.sort_unstable_by_key(|&(col, row, place, _)| (col, row, place));
        let mut indices: Vec<usize> = Vec::new();
        let mut values: Vec<f64> = Vec::new();
        let mut starts = filled(after(cols)?, 0)?;
        let mut last = None;
        for &(col, row, _, value) in &sorted {
            if last == Some((col, row)) {
                let held = values.last_mut().ok_or(LinalgError::Defect)?;
                *held = finite(*held + value)?;
            } else {
                push(&mut indices, row)?;
                push(&mut values, value)?;
                let count = slot(&mut starts, after(col)?)?;
                *count = after(*count)?;
                last = Some((col, row));
            }
        }
        for j in 0..cols {
            let before = at(&starts, j)?;
            let here = slot(&mut starts, after(j)?)?;
            *here = here.checked_add(before).ok_or(LinalgError::Defect)?;
        }
        Ok(Sparse {
            rows,
            cols,
            starts,
            indices,
            values,
        })
    }

    /// The nonzero entries of `a`.
    ///
    /// # Errors
    ///
    /// As [`Sparse::from_triplets`].
    pub fn from_dense(a: &Matrix) -> Result<Sparse, LinalgError> {
        let mut triplets = Vec::new();
        for i in 0..a.rows() {
            for j in 0..a.cols() {
                let value = a.get(i, j)?;
                if value != 0.0 {
                    push(&mut triplets, (i, j, value))?;
                }
            }
        }
        Sparse::from_triplets(a.rows(), a.cols(), &triplets)
    }

    /// The same matrix, dense.
    ///
    /// # Errors
    ///
    /// As [`Matrix::zeros`].
    pub fn to_dense(&self) -> Result<Matrix, LinalgError> {
        let mut a = Matrix::zeros(self.rows, self.cols)?;
        for j in 0..self.cols {
            let (rows, values) = self.column(j)?;
            for (&i, &value) in rows.iter().zip(values) {
                a.set(i, j, value)?;
            }
        }
        Ok(a)
    }

    /// How many rows.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// How many columns.
    #[must_use]
    pub const fn cols(&self) -> usize {
        self.cols
    }

    /// How many entries are stored.
    #[must_use]
    pub fn nonzeros(&self) -> usize {
        self.indices.len()
    }

    /// Column `j`: its rows, ascending, and its values.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if there is no column `j`.
    pub fn column(&self, j: usize) -> Result<(&[usize], &[f64]), LinalgError> {
        if j >= self.cols {
            return Err(LinalgError::Shape);
        }
        let (lo, hi) = (at(&self.starts, j)?, at(&self.starts, after(j)?)?);
        Ok((
            self.indices.get(lo..hi).ok_or(LinalgError::Defect)?,
            self.values.get(lo..hi).ok_or(LinalgError::Defect)?,
        ))
    }

    /// Where the entry at (`row`, `col`) is stored, for [`Sparse::set_at`].
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if no entry is stored there.
    pub fn place_of(&self, row: usize, col: usize) -> Result<usize, LinalgError> {
        let (rows, _) = self.column(col)?;
        let within = rows.binary_search(&row).map_err(|_| LinalgError::Shape)?;
        at(&self.starts, col)?
            .checked_add(within)
            .ok_or(LinalgError::Defect)
    }

    /// Sets the entry stored at `place`: the pattern stays, so an analysis of it still serves.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if nothing is stored there; [`LinalgError::NotFinite`].
    pub fn set_at(&mut self, place: usize, value: f64) -> Result<(), LinalgError> {
        let held = self.values.get_mut(place).ok_or(LinalgError::Shape)?;
        *held = finite(value)?;
        Ok(())
    }

    /// Adds to the entry stored at `place`: for a pattern whose triplets name one place twice.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if nothing is stored there; [`LinalgError::NotFinite`].
    pub fn add_at(&mut self, place: usize, value: f64) -> Result<(), LinalgError> {
        let held = self.values.get_mut(place).ok_or(LinalgError::Shape)?;
        *held = finite(*held + value)?;
        Ok(())
    }

    /// The entry stored at `place`.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if nothing is stored there.
    pub fn at_place(&self, place: usize) -> Result<f64, LinalgError> {
        self.values
            .get(place)
            .copied()
            .ok_or(LinalgError::Shape)
    }

    /// A·x.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `x` is not one value per column; [`LinalgError::NotFinite`];
    /// [`LinalgError::OutOfMemory`].
    pub fn multiply(&self, x: &[f64]) -> Result<Vec<f64>, LinalgError> {
        if x.len() != self.cols {
            return Err(LinalgError::Shape);
        }
        let mut y = zeros(self.rows)?;
        for (j, &xj) in x.iter().enumerate() {
            let (rows, values) = self.column(j)?;
            for (&i, &value) in rows.iter().zip(values) {
                let held = slot(&mut y, i)?;
                *held = finite(*held + value * xj)?;
            }
        }
        Ok(y)
    }

    /// Aᵀ, whose columns are A's rows.
    ///
    /// # Errors
    ///
    /// [`LinalgError::OutOfMemory`].
    pub fn transpose(&self) -> Result<Sparse, LinalgError> {
        let mut triplets = Vec::new();
        triplets
            .try_reserve_exact(self.nonzeros())
            .map_err(|_| LinalgError::OutOfMemory)?;
        for j in 0..self.cols {
            let (rows, values) = self.column(j)?;
            for (&i, &value) in rows.iter().zip(values) {
                triplets.push((j, i, value));
            }
        }
        Sparse::from_triplets(self.cols, self.rows, &triplets)
    }

    /// Whether `starts` and `indices` are this matrix's pattern.
    pub(crate) fn has_pattern(&self, rows: usize, starts: &[usize], indices: &[usize]) -> bool {
        self.rows == rows && self.starts == starts && self.indices == indices
    }

    /// The column pointers and row indices.
    pub(crate) fn pattern(&self) -> (&[usize], &[usize]) {
        (&self.starts, &self.indices)
    }
}
