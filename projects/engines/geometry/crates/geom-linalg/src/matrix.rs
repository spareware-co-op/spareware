//! Dense matrices of finite doubles, the errors of every factorisation, and the work budget each
//! one charges before it starts.
//!
//! What it does: [`Matrix`] is a dense matrix stored by rows, every entry a finite `f64`, made only
//! by fallible constructors (an allocation that cannot be made is [`LinalgError::OutOfMemory`],
//! never an abort). [`Work`] is a budget in units of one multiply-add: each factorisation knows its
//! cost from its shape, charges it in full before touching a number, and refuses with
//! [`LinalgError::OutOfWork`] when the budget cannot cover it, so a factorisation either finishes
//! or never starts (chapter 15.6: "budgets in work, not time").
//!
//! Guarantees:
//!
//! - **Finite.** Every entry of every [`Matrix`] is finite; a constructor refuses NaN and
//!   infinities with [`LinalgError::NotFinite`], and a factorisation whose arithmetic overflows
//!   reports it the same way instead of returning infinities.
//! - **Charged before it starts.** A factorisation's cost is a function of its shape alone, stated
//!   on each function, and is charged before any arithmetic: the same budget gives the same
//!   answer, or the same refusal, on every target.
//! - **No panic, no abort.** Indices are checked; allocations are fallible.
//!
//! Method: row-major dense storage, as LAPACK's column-major storage transposed (E. Anderson et
//! al., *LAPACK Users' Guide*, 3rd ed., SIAM (1999), <https://www.netlib.org/lapack/lug/>), with
//! the work model of G. H. Golub and C. F. Van Loan, *Matrix Computations*, 4th ed., Johns Hopkins
//! (2013), section 1.1.15 (flop counts).
//!
//! Cost: a matrix of r × c entries is one allocation of r · c doubles; access is constant time.

extern crate alloc;

use alloc::vec::Vec;

use geom_num::NumError;

/// Why a factorisation or solve could not give its result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinalgError {
    /// The shapes do not fit: a non-square matrix where a square one is needed, or a right-hand
    /// side of the wrong length.
    Shape,
    /// An entry, or a result of the arithmetic, is NaN or infinite.
    NotFinite,
    /// The matrix is singular: a zero pivot (LU, triangular, tridiagonal).
    Singular,
    /// The matrix is not symmetric positive definite (Cholesky).
    NotPositiveDefinite,
    /// The work budget cannot cover the factorisation's cost.
    OutOfWork,
    /// An allocation could not be made.
    OutOfMemory,
    /// A step its own bounds rule out failed: a defect, reported, never answered.
    Defect,
}

impl From<NumError> for LinalgError {
    fn from(e: NumError) -> Self {
        match e {
            NumError::NotFinite => LinalgError::NotFinite,
            _ => LinalgError::Defect,
        }
    }
}

impl core::fmt::Display for LinalgError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            LinalgError::Shape => "the shapes do not fit",
            LinalgError::NotFinite => "an entry or a result is not finite",
            LinalgError::Singular => "the matrix is singular",
            LinalgError::NotPositiveDefinite => "the matrix is not symmetric positive definite",
            LinalgError::OutOfWork => "the work budget cannot cover the factorisation",
            LinalgError::OutOfMemory => "an allocation could not be made",
            LinalgError::Defect => "a step its own bounds rule out failed (a defect)",
        })
    }
}

/// A work budget, in multiply-adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Work(u64);

impl Work {
    /// A budget of `units` multiply-adds.
    #[must_use]
    pub const fn new(units: u64) -> Work {
        Work(units)
    }

    /// What is left.
    #[must_use]
    pub const fn left(self) -> u64 {
        self.0
    }

    /// Charges `units`, or refuses without charging anything.
    ///
    /// # Errors
    ///
    /// [`LinalgError::OutOfWork`] if fewer than `units` are left.
    pub fn charge(&mut self, units: u64) -> Result<(), LinalgError> {
        self.0 = self.0.checked_sub(units).ok_or(LinalgError::OutOfWork)?;
        Ok(())
    }
}

/// `n` as a `u64`, for costs: a `usize` is at most 64 bits on every target the kernel builds for.
pub(crate) fn wide(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// a · b · c, saturating: a cost too large to count is beyond every budget anyway.
pub(crate) fn cost3(a: usize, b: usize, c: usize) -> u64 {
    wide(a).saturating_mul(wide(b)).saturating_mul(wide(c))
}

/// A vector of `n` zeros, fallibly allocated.
pub(crate) fn zeros(n: usize) -> Result<Vec<f64>, LinalgError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| LinalgError::OutOfMemory)?;
    v.resize(n, 0.0);
    Ok(v)
}

/// A finite value, or `NotFinite`.
pub(crate) fn finite(v: f64) -> Result<f64, LinalgError> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(LinalgError::NotFinite)
    }
}

/// A dense matrix of finite doubles, stored by rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

impl Matrix {
    /// The `rows` × `cols` zero matrix.
    ///
    /// # Errors
    ///
    /// [`LinalgError::OutOfMemory`] if the entries cannot be allocated, or their count overflows.
    pub fn zeros(rows: usize, cols: usize) -> Result<Matrix, LinalgError> {
        let n = rows.checked_mul(cols).ok_or(LinalgError::OutOfMemory)?;
        Ok(Matrix {
            rows,
            cols,
            data: zeros(n)?,
        })
    }

    /// The `n` × `n` identity.
    ///
    /// # Errors
    ///
    /// As [`Matrix::zeros`].
    pub fn identity(n: usize) -> Result<Matrix, LinalgError> {
        let mut m = Matrix::zeros(n, n)?;
        for i in 0..n {
            m.set(i, i, 1.0)?;
        }
        Ok(m)
    }

    /// The matrix with the given entries, row by row.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if there are not `rows · cols` entries; [`LinalgError::NotFinite`]
    /// for a NaN or infinite entry; [`LinalgError::OutOfMemory`] as for [`Matrix::zeros`].
    pub fn from_rows(rows: usize, cols: usize, entries: &[f64]) -> Result<Matrix, LinalgError> {
        if rows.checked_mul(cols) != Some(entries.len()) {
            return Err(LinalgError::Shape);
        }
        let mut data = Vec::new();
        data.try_reserve_exact(entries.len())
            .map_err(|_| LinalgError::OutOfMemory)?;
        for v in entries {
            data.push(finite(*v)?);
        }
        Ok(Matrix { rows, cols, data })
    }

    /// The number of rows.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// The number of columns.
    #[must_use]
    pub const fn cols(&self) -> usize {
        self.cols
    }

    fn index(&self, i: usize, j: usize) -> Result<usize, LinalgError> {
        if i >= self.rows || j >= self.cols {
            return Err(LinalgError::Shape);
        }
        i.checked_mul(self.cols)
            .and_then(|k| k.checked_add(j))
            .ok_or(LinalgError::Defect)
    }

    /// The entry in row `i`, column `j`.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `(i, j)` is outside the matrix.
    pub fn get(&self, i: usize, j: usize) -> Result<f64, LinalgError> {
        let k = self.index(i, j)?;
        self.data.get(k).copied().ok_or(LinalgError::Defect)
    }

    /// Sets the entry in row `i`, column `j`.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `(i, j)` is outside the matrix; [`LinalgError::NotFinite`] for a
    /// NaN or infinite value.
    pub fn set(&mut self, i: usize, j: usize, v: f64) -> Result<(), LinalgError> {
        let k = self.index(i, j)?;
        let slot = self.data.get_mut(k).ok_or(LinalgError::Defect)?;
        *slot = finite(v)?;
        Ok(())
    }

    /// Row `i`.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if there is no row `i`.
    pub fn row(&self, i: usize) -> Result<&[f64], LinalgError> {
        let start = self.index(i, 0).or_else(|e| {
            if self.cols == 0 && i < self.rows {
                Ok(0)
            } else {
                Err(e)
            }
        })?;
        let end = start.checked_add(self.cols).ok_or(LinalgError::Defect)?;
        self.data.get(start..end).ok_or(LinalgError::Defect)
    }

    /// Two different rows, both writable, in the order asked.
    pub(crate) fn row_pair(
        &mut self,
        i: usize,
        j: usize,
    ) -> Result<(&mut [f64], &mut [f64]), LinalgError> {
        if i == j || i >= self.rows || j >= self.rows {
            return Err(LinalgError::Defect);
        }
        let c = self.cols;
        let (lo, hi) = if i < j { (i, j) } else { (j, i) };
        let split = hi.checked_mul(c).ok_or(LinalgError::Defect)?;
        let (head, tail) = self.data.split_at_mut(split);
        let lo_start = lo.checked_mul(c).ok_or(LinalgError::Defect)?;
        let lo_end = lo_start.checked_add(c).ok_or(LinalgError::Defect)?;
        let lo_row = head.get_mut(lo_start..lo_end).ok_or(LinalgError::Defect)?;
        let hi_row = tail.get_mut(..c).ok_or(LinalgError::Defect)?;
        Ok(if i < j {
            (lo_row, hi_row)
        } else {
            (hi_row, lo_row)
        })
    }

    /// Row `i`, writable.
    pub(crate) fn row_mut(&mut self, i: usize) -> Result<&mut [f64], LinalgError> {
        if i >= self.rows {
            return Err(LinalgError::Defect);
        }
        let start = i.checked_mul(self.cols).ok_or(LinalgError::Defect)?;
        let end = start.checked_add(self.cols).ok_or(LinalgError::Defect)?;
        self.data.get_mut(start..end).ok_or(LinalgError::Defect)
    }

    /// Swaps rows `i` and `j`.
    pub(crate) fn swap_rows(&mut self, i: usize, j: usize) -> Result<(), LinalgError> {
        if i == j {
            return Ok(());
        }
        let (a, b) = self.row_pair(i, j)?;
        for (x, y) in a.iter_mut().zip(b.iter_mut()) {
            core::mem::swap(x, y);
        }
        Ok(())
    }

    /// Swaps columns `i` and `j`.
    pub(crate) fn swap_cols(&mut self, i: usize, j: usize) -> Result<(), LinalgError> {
        if i >= self.cols || j >= self.cols {
            return Err(LinalgError::Defect);
        }
        for row in self.data.chunks_exact_mut(self.cols.max(1)) {
            row.swap(i, j);
        }
        Ok(())
    }

    /// The product `self · v`, in a fixed order (each row's sum left to right).
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `v` has not `cols` entries; [`LinalgError::NotFinite`] on
    /// overflow.
    pub fn mul_vec(&self, v: &[f64]) -> Result<Vec<f64>, LinalgError> {
        if v.len() != self.cols {
            return Err(LinalgError::Shape);
        }
        let mut out = zeros(self.rows)?;
        for (i, slot) in out.iter_mut().enumerate() {
            let row = self.row(i)?;
            *slot = finite(row.iter().zip(v).fold(0.0, |acc, (a, b)| acc + a * b))?;
        }
        Ok(out)
    }
}
