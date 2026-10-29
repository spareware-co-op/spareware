//! Matrices stored by columns, for the factorisations that work column by column (the SVDs and
//! the symmetric eigensolver), and checked index arithmetic for them.
//!
//! What it does: [`Cols`] holds `count` columns of `len` doubles each, one after another, so a
//! column is one contiguous slice and two columns can be borrowed together for a rotation.
//! [`Cols::to_matrix`] copies chosen columns, in a chosen order, into a row-major [`Matrix`].
//!
//! Guarantees:
//!
//! - **No panic, no abort.** Every index is checked and every index sum and product too; a bad one
//!   is [`LinalgError::Defect`]. The storage is allocated fallibly.
//! - **Exact copies.** Moving values between [`Cols`] and [`Matrix`] copies them unchanged.
//!
//! Method: column-major storage as LAPACK uses it (E. Anderson et al., *LAPACK Users' Guide*, 3rd
//! ed., SIAM (1999), <https://www.netlib.org/lapack/lug/>).
//!
//! Cost: one allocation of len · count doubles; access is constant time, copies linear.

extern crate alloc;

use alloc::vec::Vec;

use crate::matrix::{LinalgError, Matrix, zeros};

/// a + b.
pub(crate) fn add(a: usize, b: usize) -> Result<usize, LinalgError> {
    a.checked_add(b).ok_or(LinalgError::Defect)
}

/// a − b, never negative.
pub(crate) fn sub(a: usize, b: usize) -> Result<usize, LinalgError> {
    a.checked_sub(b).ok_or(LinalgError::Defect)
}

/// a · b.
pub(crate) fn mul(a: usize, b: usize) -> Result<usize, LinalgError> {
    a.checked_mul(b).ok_or(LinalgError::Defect)
}

/// `s[i]`.
pub(crate) fn at(s: &[f64], i: usize) -> Result<f64, LinalgError> {
    s.get(i).copied().ok_or(LinalgError::Defect)
}

/// `&mut s[i]`.
pub(crate) fn at_mut(s: &mut [f64], i: usize) -> Result<&mut f64, LinalgError> {
    s.get_mut(i).ok_or(LinalgError::Defect)
}

/// `count` columns of `len` doubles, stored one column after another.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cols {
    len: usize,
    count: usize,
    data: Vec<f64>,
}

impl Cols {
    /// `count` zero columns of length `len`.
    pub(crate) fn zeros(len: usize, count: usize) -> Result<Cols, LinalgError> {
        Ok(Cols {
            len,
            count,
            data: zeros(len.checked_mul(count).ok_or(LinalgError::OutOfMemory)?)?,
        })
    }

    /// The first `count` columns of the `len` × `len` identity.
    pub(crate) fn identity(len: usize, count: usize) -> Result<Cols, LinalgError> {
        let mut c = Cols::zeros(len, count)?;
        let diag = if len < count { len } else { count };
        for j in 0..diag {
            *at_mut(c.col_mut(j)?, j)? = 1.0;
        }
        Ok(c)
    }

    /// The columns of `a`, or of its transpose if `transpose` is true.
    pub(crate) fn of(a: &Matrix, transpose: bool) -> Result<Cols, LinalgError> {
        let (len, count) = if transpose {
            (a.cols(), a.rows())
        } else {
            (a.rows(), a.cols())
        };
        let mut c = Cols::zeros(len, count)?;
        for j in 0..count {
            let col = c.col_mut(j)?;
            for (i, v) in col.iter_mut().enumerate() {
                *v = if transpose {
                    a.get(j, i)?
                } else {
                    a.get(i, j)?
                };
            }
        }
        Ok(c)
    }

    /// The length of each column.
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// The number of columns.
    pub(crate) fn count(&self) -> usize {
        self.count
    }

    /// Column `j`.
    pub(crate) fn col(&self, j: usize) -> Result<&[f64], LinalgError> {
        let start = mul(j, self.len)?;
        self.data
            .get(start..add(start, self.len)?)
            .ok_or(LinalgError::Defect)
    }

    /// Column `j`, writable.
    pub(crate) fn col_mut(&mut self, j: usize) -> Result<&mut [f64], LinalgError> {
        let start = mul(j, self.len)?;
        let end = add(start, self.len)?;
        self.data.get_mut(start..end).ok_or(LinalgError::Defect)
    }

    /// Columns `p` and `q`, `p < q`, both writable.
    pub(crate) fn pair_mut(
        &mut self,
        p: usize,
        q: usize,
    ) -> Result<(&mut [f64], &mut [f64]), LinalgError> {
        if p >= q || q >= self.count {
            return Err(LinalgError::Defect);
        }
        let len = self.len;
        let (left, right) = self.data.split_at_mut(mul(q, len)?);
        let a = left
            .get_mut(mul(p, len)?..add(mul(p, len)?, len)?)
            .ok_or(LinalgError::Defect)?;
        let b = right.get_mut(..len).ok_or(LinalgError::Defect)?;
        Ok((a, b))
    }

    /// Entry `i` of column `j`.
    pub(crate) fn get(&self, i: usize, j: usize) -> Result<f64, LinalgError> {
        at(self.col(j)?, i)
    }

    /// Sets entry `i` of column `j`.
    pub(crate) fn set(&mut self, i: usize, j: usize, v: f64) -> Result<(), LinalgError> {
        *at_mut(self.col_mut(j)?, i)? = v;
        Ok(())
    }

    /// The matrix whose column k is column `order[k]` of these, with `len` rows.
    pub(crate) fn to_matrix(&self, order: &[usize]) -> Result<Matrix, LinalgError> {
        let mut m = Matrix::zeros(self.len, order.len())?;
        for (k, &j) in order.iter().enumerate() {
            for (i, &v) in self.col(j)?.iter().enumerate() {
                m.set(i, k, v)?;
            }
        }
        Ok(m)
    }
}
