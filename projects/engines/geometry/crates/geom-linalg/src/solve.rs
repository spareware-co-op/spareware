//! Square systems: LU with partial pivoting, Cholesky, triangular and tridiagonal solves.
//!
//! What it does: [`Lu::factor`] factors a square matrix as P·A = L·U by Gaussian elimination with
//! partial pivoting, and [`Lu::solve`] solves A·x = b with it. [`Cholesky::factor`] factors a
//! symmetric positive definite matrix as A = G·Gᵀ, and [`Cholesky::solve`] solves with it.
//! [`solve_lower`] and [`solve_upper`] are forward and back substitution on a triangle, and
//! [`solve_tridiagonal`] solves a tridiagonal system by Gaussian elimination with partial pivoting.
//!
//! Guarantees:
//!
//! - **Deterministic.** Plain scalar loops in a fixed order: no SIMD, no fused multiply-add, no
//!   threads (chapter 15.6). A pivot is the largest magnitude in its column, the first such row
//!   on a tie, so the same matrix gives the same factors, bit for bit, on every target.
//! - **Refused, never guessed.** A zero pivot is [`LinalgError::Singular`], a non-positive
//!   Cholesky pivot or an asymmetric matrix [`LinalgError::NotPositiveDefinite`], an overflow
//!   [`LinalgError::NotFinite`]: no result holds an infinity or a NaN. Tiny pivots are allowed:
//!   conditioning is the caller's to judge (the complete orthogonal decomposition in
//!   [`crate::qr`] decides rank).
//! - **Charged first.** Each factorisation charges its cost to the [`Work`] budget before any
//!   arithmetic: n³ multiply-adds for LU and Cholesky (an upper bound: n³/3 and n³/6 are the
//!   leading terms), n² for a solve with factors, n for a tridiagonal solve.
//!
//! Method: G. H. Golub and C. F. Van Loan, *Matrix Computations*, 4th ed., Johns Hopkins (2013):
//! algorithm 3.4.1 (Gaussian elimination with partial pivoting, outer-product form), 3.1.1 and
//! 3.1.2 (row-oriented forward and back substitution), 4.2.2 (Cholesky, gaxpy form) and section
//! 4.3.6 with LAPACK's `dgtsv` (tridiagonal elimination with partial pivoting, which fills one
//! extra superdiagonal; E. Anderson et al., *LAPACK Users' Guide*, 3rd ed., SIAM (1999),
//! <https://www.netlib.org/lapack/lug/>), written in our own words from the books (chapter 29.4's
//! row for LAPACK, Golub and Van Loan and the Thomas algorithm).
//!
//! Cost: as charged above; one allocation of n² doubles for LU or Cholesky, n doubles per solve.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the books': n the order, a, b, l, u, x and y as in Golub and Van Loan's algorithms"
)]

extern crate alloc;

use alloc::vec::Vec;

use geom_num::math;

use crate::matrix::{LinalgError, Matrix, Work, cost3, finite, wide, zeros};

/// Forward substitution: the solution of L·x = b for the lower triangle of `l` (its diagonal
/// taken as 1 where `unit`).
///
/// # Errors
///
/// [`LinalgError::Shape`] if `l` is not square or `b` not its size; [`LinalgError::Singular`] for
/// a zero diagonal entry (not `unit`); [`LinalgError::NotFinite`] on overflow;
/// [`LinalgError::OutOfWork`] if the budget cannot cover n² multiply-adds.
pub fn solve_lower(
    l: &Matrix,
    b: &[f64],
    unit: bool,
    work: &mut Work,
) -> Result<Vec<f64>, LinalgError> {
    let n = l.rows();
    if l.cols() != n || b.len() != n {
        return Err(LinalgError::Shape);
    }
    work.charge(wide(n).saturating_mul(wide(n)))?;
    let mut x = zeros(n)?;
    x.copy_from_slice(b);
    for i in 0..n {
        let row = l.row(i)?;
        let (done, rest) = x.split_at_mut(i);
        let slot = rest.first_mut().ok_or(LinalgError::Defect)?;
        let sum = row
            .iter()
            .zip(done.iter())
            .fold(*slot, |acc, (a, v)| acc - a * v);
        let d = if unit {
            1.0
        } else {
            *row.get(i).ok_or(LinalgError::Defect)?
        };
        if d == 0.0 {
            return Err(LinalgError::Singular);
        }
        *slot = finite(sum / d)?;
    }
    Ok(x)
}

/// Back substitution: the solution of U·x = b for the upper triangle of `u`.
///
/// # Errors
///
/// As [`solve_lower`] (a zero diagonal entry is singular).
pub fn solve_upper(u: &Matrix, b: &[f64], work: &mut Work) -> Result<Vec<f64>, LinalgError> {
    let n = u.rows();
    if u.cols() != n || b.len() != n {
        return Err(LinalgError::Shape);
    }
    work.charge(wide(n).saturating_mul(wide(n)))?;
    let mut x = zeros(n)?;
    x.copy_from_slice(b);
    for i in (0..n).rev() {
        let row = u.row(i)?;
        let (head, done) = x.split_at_mut(i.checked_add(1).ok_or(LinalgError::Defect)?);
        let slot = head.last_mut().ok_or(LinalgError::Defect)?;
        let tail = row
            .get(i.checked_add(1).ok_or(LinalgError::Defect)?..)
            .ok_or(LinalgError::Defect)?;
        let sum = tail
            .iter()
            .zip(done.iter())
            .fold(*slot, |acc, (a, v)| acc - a * v);
        let d = *row.get(i).ok_or(LinalgError::Defect)?;
        if d == 0.0 {
            return Err(LinalgError::Singular);
        }
        *slot = finite(sum / d)?;
    }
    Ok(x)
}

/// An LU factorisation with partial pivoting: P·A = L·U, L unit lower triangular below the
/// diagonal of `lu`, U on and above it.
#[derive(Clone, Debug, PartialEq)]
pub struct Lu {
    lu: Matrix,
    /// Row i of P·A is row `perm[i]` of A.
    perm: Vec<usize>,
}

impl Lu {
    /// Factors the square matrix `a`, charging n³ multiply-adds.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `a` is not square; [`LinalgError::Singular`] if a column has no
    /// non-zero pivot; [`LinalgError::NotFinite`] on overflow; [`LinalgError::OutOfWork`];
    /// [`LinalgError::OutOfMemory`].
    pub fn factor(a: &Matrix, work: &mut Work) -> Result<Lu, LinalgError> {
        let n = a.rows();
        if a.cols() != n {
            return Err(LinalgError::Shape);
        }
        work.charge(cost3(n, n, n))?;
        let mut lu = a.clone();
        let mut perm = Vec::new();
        perm.try_reserve_exact(n)
            .map_err(|_| LinalgError::OutOfMemory)?;
        perm.extend(0..n);
        for k in 0..n {
            // The pivot: the largest magnitude in column k at or below row k, the first on a tie.
            let mut p = k;
            let mut best = 0.0f64;
            for i in k..n {
                let v = lu.get(i, k)?.abs();
                if v > best {
                    best = v;
                    p = i;
                }
            }
            if best == 0.0 {
                return Err(LinalgError::Singular);
            }
            lu.swap_rows(k, p)?;
            perm.swap(k, p);
            let pivot = lu.get(k, k)?;
            for i in k.checked_add(1).ok_or(LinalgError::Defect)?..n {
                let (top, row) = lu.row_pair(k, i)?;
                let m = finite(*row.get(k).ok_or(LinalgError::Defect)? / pivot)?;
                if let Some(slot) = row.get_mut(k) {
                    *slot = m;
                }
                for (x, t) in row
                    .iter_mut()
                    .zip(top.iter())
                    .skip(k.checked_add(1).ok_or(LinalgError::Defect)?)
                {
                    *x = finite(*x - m * t)?;
                }
            }
        }
        Ok(Lu { lu, perm })
    }

    /// The combined factors: L strictly below the diagonal (its unit diagonal not stored), U on
    /// and above it.
    #[must_use]
    pub fn factors(&self) -> &Matrix {
        &self.lu
    }

    /// The row permutation: row i of P·A is row `permutation()[i]` of A.
    #[must_use]
    pub fn permutation(&self) -> &[usize] {
        &self.perm
    }

    /// The solution of A·x = b, charging 2n² multiply-adds.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `b` is not n long; [`LinalgError::NotFinite`] on overflow;
    /// [`LinalgError::OutOfWork`].
    pub fn solve(&self, b: &[f64], work: &mut Work) -> Result<Vec<f64>, LinalgError> {
        let n = self.lu.rows();
        if b.len() != n {
            return Err(LinalgError::Shape);
        }
        let mut pb = zeros(n)?;
        for (slot, &i) in pb.iter_mut().zip(&self.perm) {
            *slot = *b.get(i).ok_or(LinalgError::Defect)?;
        }
        let y = solve_lower(&self.lu, &pb, true, work)?;
        solve_upper(&self.lu, &y, work)
    }
}

/// A Cholesky factorisation A = G·Gᵀ, G lower triangular with a positive diagonal.
#[derive(Clone, Debug, PartialEq)]
pub struct Cholesky {
    g: Matrix,
}

impl Cholesky {
    /// Factors the symmetric positive definite matrix `a`, charging n³ multiply-adds.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `a` is not square; [`LinalgError::NotPositiveDefinite`] if `a`
    /// is not exactly symmetric or a pivot is not positive; [`LinalgError::NotFinite`];
    /// [`LinalgError::OutOfWork`]; [`LinalgError::OutOfMemory`].
    pub fn factor(a: &Matrix, work: &mut Work) -> Result<Cholesky, LinalgError> {
        let n = a.rows();
        if a.cols() != n {
            return Err(LinalgError::Shape);
        }
        for i in 0..n {
            for j in 0..i {
                if a.get(i, j)? != a.get(j, i)? {
                    return Err(LinalgError::NotPositiveDefinite);
                }
            }
        }
        work.charge(cost3(n, n, n))?;
        let mut g = Matrix::zeros(n, n)?;
        for j in 0..n {
            // v = A(j:n, j) − G(j:n, 0:j)·G(j, 0:j)ᵀ, then G(j:n, j) = v / √v(j).
            let gj: Vec<f64> = {
                let row = g.row(j)?;
                let mut v = zeros(j)?;
                v.copy_from_slice(row.get(..j).ok_or(LinalgError::Defect)?);
                v
            };
            let mut diag = 0.0;
            for i in j..n {
                let row = g.row(i)?;
                let dot = row
                    .iter()
                    .zip(gj.iter())
                    .fold(0.0, |acc, (x, y)| acc + x * y);
                let v = finite(a.get(i, j)? - dot)?;
                if i == j {
                    if v <= 0.0 {
                        return Err(LinalgError::NotPositiveDefinite);
                    }
                    diag = math::sqrt(v)?.get();
                    g.set(j, j, diag)?;
                } else {
                    g.set(i, j, v / diag)?;
                }
            }
        }
        Ok(Cholesky { g })
    }

    /// The factor G, lower triangular.
    #[must_use]
    pub fn factor_g(&self) -> &Matrix {
        &self.g
    }

    /// The solution of A·x = b: G·y = b, then Gᵀ·x = y, charging 2n² multiply-adds.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `b` is not n long; [`LinalgError::NotFinite`];
    /// [`LinalgError::OutOfWork`].
    pub fn solve(&self, b: &[f64], work: &mut Work) -> Result<Vec<f64>, LinalgError> {
        let n = self.g.rows();
        if b.len() != n {
            return Err(LinalgError::Shape);
        }
        let y = solve_lower(&self.g, b, false, work)?;
        // Gᵀ·x = y by back substitution on the transpose, read from G's columns.
        work.charge(wide(n).saturating_mul(wide(n)))?;
        let mut x = zeros(n)?;
        x.copy_from_slice(&y);
        for i in (0..n).rev() {
            let mut sum = *x.get(i).ok_or(LinalgError::Defect)?;
            for k in i.checked_add(1).ok_or(LinalgError::Defect)?..n {
                sum -= self.g.get(k, i)? * x.get(k).ok_or(LinalgError::Defect)?;
            }
            let slot = x.get_mut(i).ok_or(LinalgError::Defect)?;
            *slot = finite(sum / self.g.get(i, i)?)?;
        }
        Ok(x)
    }
}

/// The solution of a tridiagonal system T·x = b, T with subdiagonal `sub` (n − 1 entries),
/// diagonal `diag` (n) and superdiagonal `sup` (n − 1), by Gaussian elimination with partial
/// pivoting (LAPACK's `dgtsv` method), charging 4n multiply-adds. A row interchange fills a
/// second superdiagonal, kept in a work vector.
///
/// # Errors
///
/// [`LinalgError::Shape`] if the lengths do not fit; [`LinalgError::Singular`] for a zero pivot;
/// [`LinalgError::NotFinite`]; [`LinalgError::OutOfWork`]; [`LinalgError::OutOfMemory`].
pub fn solve_tridiagonal(
    sub: &[f64],
    diag: &[f64],
    sup: &[f64],
    b: &[f64],
    work: &mut Work,
) -> Result<Vec<f64>, LinalgError> {
    let n = diag.len();
    let off = n.saturating_sub(1);
    if b.len() != n || sub.len() != off || sup.len() != off {
        return Err(LinalgError::Shape);
    }
    work.charge(wide(n).saturating_mul(4))?;
    let copy = |v: &[f64]| -> Result<Vec<f64>, LinalgError> {
        let mut out = zeros(v.len())?;
        for (o, x) in out.iter_mut().zip(v) {
            *o = finite(*x)?;
        }
        Ok(out)
    };
    // d: the diagonal; u1, u2: the first and second superdiagonals of U; l: the subdiagonal.
    let (mut d, mut u1, l, mut x) = (copy(diag)?, copy(sup)?, copy(sub)?, copy(b)?);
    let mut u2 = zeros(n.saturating_sub(2))?;
    let at = |v: &[f64], i: usize| v.get(i).copied().ok_or(LinalgError::Defect);
    for i in 0..off {
        let next = i.checked_add(1).ok_or(LinalgError::Defect)?;
        let (di, li) = (at(&d, i)?, at(&l, i)?);
        if di.abs() >= li.abs() {
            // No interchange: eliminate l(i) with row i.
            if di == 0.0 {
                return Err(LinalgError::Singular);
            }
            let m = finite(li / di)?;
            let dn = at(&d, next)? - m * at(&u1, i)?;
            set(&mut d, next, dn)?;
            let xn = at(&x, next)? - m * at(&x, i)?;
            set(&mut x, next, xn)?;
        } else {
            // Interchange rows i and i + 1, then eliminate.
            let m = finite(di / li)?;
            set(&mut d, i, li)?;
            let (ui, dn) = (at(&u1, i)?, at(&d, next)?);
            set(&mut u1, i, dn)?;
            set(&mut d, next, ui - m * dn)?;
            if next < off {
                let un = at(&u1, next)?;
                set(&mut u2, i, un)?;
                set(&mut u1, next, -m * un)?;
            }
            let (xi, xn) = (at(&x, i)?, at(&x, next)?);
            set(&mut x, i, xn)?;
            set(&mut x, next, xi - m * xn)?;
        }
    }
    // Back substitution with up to two superdiagonals.
    for i in (0..n).rev() {
        let mut sum = at(&x, i)?;
        if let Some(j) = i.checked_add(1).filter(|j| *j < n) {
            sum -= at(&u1, i)? * at(&x, j)?;
        }
        if let Some(j) = i.checked_add(2).filter(|j| *j < n) {
            sum -= at(&u2, i)? * at(&x, j)?;
        }
        let di = at(&d, i)?;
        if di == 0.0 {
            return Err(LinalgError::Singular);
        }
        set(&mut x, i, sum / di)?;
    }
    Ok(x)
}

/// Sets `v[i]` to a finite value.
fn set(v: &mut [f64], i: usize, value: f64) -> Result<(), LinalgError> {
    *v.get_mut(i).ok_or(LinalgError::Defect)? = finite(value)?;
    Ok(())
}
