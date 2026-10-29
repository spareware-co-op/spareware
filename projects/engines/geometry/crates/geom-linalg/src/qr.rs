//! Householder QR with column pivoting, the complete orthogonal decomposition, the minimum-norm
//! least-squares solution, and the canonical null space.
//!
//! What it does: [`PivotedQr::factor`] factors an m × n matrix as A·Π = Q·R, Q a product of
//! Householder reflections, R upper triangular with diagonal magnitudes that never increase, Π the
//! column order chosen by largest remaining norm. [`Cod::factor`] decides the rank r from R's
//! diagonal and a relative tolerance, then removes R's trailing columns by reflections from the
//! right, giving A·Π = Q·[T 0; 0 0]·Zᵀ with T r × r upper triangular: the complete orthogonal
//! decomposition. From it, [`Cod::least_squares`] gives the least-squares solution of A·x = b of
//! least norm (chapter 15.6's full step: the limit of the damped steps, never a pivot-order basic
//! solution), and [`Cod::null_space`] an orthonormal basis of the null space. [`canonical_null_space`]
//! turns that basis into chapter 15.8's canonical one: reduced echelon form over the variable
//! order, each vector 1 at its own leading variable and 0 at the others', so the basis is unique.
//!
//! Guarantees:
//!
//! - **Deterministic.** Plain scalar loops in a fixed order (chapter 15.6). A pivot column is the
//!   one of largest remaining norm, the first on a tie; norms are scaled by the column's largest
//!   magnitude, so they never overflow for finite entries.
//! - **Least change, never basic.** In a rank-deficient or under-determined system the solution
//!   has the least Euclidean norm among all least-squares solutions; no free variable is set to
//!   zero in pivot order.
//! - **Rank decided once, by the caller's tolerance.** A diagonal entry of R at most `tol` times
//!   the first ends the rank; the tolerance is the caller's (chapter 15 derives it from the
//!   tolerance ladder). Like every column-pivoted QR, it can miss a near rank deficiency that no
//!   column norm shows: Kahan's matrix is the classic case (tested).
//! - **Charged first.** A factorisation charges 2·m·n·min(m, n) multiply-adds before it starts,
//!   the decomposition r²·n more, a solve 2·m·n, a null space n²·(n − r).
//!
//! Method: G. H. Golub and C. F. Van Loan, *Matrix Computations*, 4th ed., Johns Hopkins (2013):
//! algorithm 5.1.1 (the Householder vector), 5.4.1 (QR with column pivoting), section 5.4.2 (the
//! complete orthogonal decomposition) and 5.5 (least squares in the rank-deficient case); the
//! column norms downdated as LAPACK's `xLAQP2` does, recomputed when the downdate loses more than
//! √u of its relative accuracy (Z. Drmač and Z. Bujanović, *On the failure of rank-revealing QR
//! factorization software*, ACM TOMS 35 (2008), doi:10.1145/1377612.1377616); the trailing columns
//! removed as LAPACK's `xTZRZF` does (E. Anderson et al., *LAPACK Users' Guide*, 3rd ed., SIAM
//! (1999), <https://www.netlib.org/lapack/lug/>). Written in our own words from the books and
//! papers (chapter 29.4).
//!
//! Cost: as charged above; one copy of A and a few vectors of length n.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the books': m × n for the shape, r the rank, k the step, a, b, x, y, z and t as in Golub and Van Loan's algorithms"
)]

extern crate alloc;

use alloc::vec::Vec;

use geom_num::math;
use geom_num::real::Real;

use crate::matrix::{LinalgError, Matrix, Work, cost3, finite, wide, zeros};

/// √u: a downdated column norm that has lost this much of its relative accuracy is recomputed.
const SQRT_U: f64 = f64::from_bits(0x3E46_A09E_667F_3BCD);

/// The Euclidean norm of `x`, scaled by its largest magnitude so that it never overflows.
pub(crate) fn norm(x: impl Iterator<Item = f64> + Clone) -> Result<f64, LinalgError> {
    let scale = x
        .clone()
        .fold(0.0f64, |m, v| if v.abs() > m { v.abs() } else { m });
    if scale == 0.0 {
        return Ok(0.0);
    }
    let sum = x.fold(0.0, |acc, v| {
        let s = v / scale;
        acc + s * s
    });
    finite(scale * math::sqrt(sum)?.get())
}

/// A Householder reflection H = I − τ·v·vᵀ with v(0) = 1, mapping x to (β, 0, …, 0): LAPACK's
/// `xLARFG` convention, β = −sign(x₀)·‖x‖. Returns (τ, β) and leaves v(1..) in `x[1..]`; τ = 0
/// (H = I) when x(1..) is zero.
pub(crate) fn householder(x: &mut [f64]) -> Result<(f64, f64), LinalgError> {
    let (alpha, tail) = x.split_first_mut().ok_or(LinalgError::Defect)?;
    let xnorm = norm(tail.iter().copied())?;
    if xnorm == 0.0 {
        return Ok((0.0, *alpha));
    }
    let n = norm([*alpha, xnorm].into_iter())?;
    let beta = if *alpha >= 0.0 { -n } else { n };
    let tau = finite((beta - *alpha) / beta)?;
    let scale = finite(1.0 / (*alpha - beta))?;
    for v in tail.iter_mut() {
        *v = finite(*v * scale)?;
    }
    *alpha = beta;
    Ok((tau, beta))
}

/// A QR factorisation with column pivoting, A·Π = Q·R.
#[derive(Clone, Debug, PartialEq)]
pub struct PivotedQr {
    /// R on and above the diagonal; below it, each column's Householder vector without its 1.
    qr: Matrix,
    tau: Vec<f64>,
    /// Column j of A·Π is column `perm[j]` of A.
    perm: Vec<usize>,
}

impl PivotedQr {
    /// Factors `a`, charging 2·m·n·min(m, n) multiply-adds.
    ///
    /// # Errors
    ///
    /// [`LinalgError::NotFinite`] on overflow; [`LinalgError::OutOfWork`];
    /// [`LinalgError::OutOfMemory`].
    pub fn factor(a: &Matrix, work: &mut Work) -> Result<PivotedQr, LinalgError> {
        let (m, n) = (a.rows(), a.cols());
        let steps = m.min(n);
        work.charge(cost3(m, n, steps).saturating_mul(2))?;
        let mut qr = a.clone();
        let mut perm = Vec::new();
        perm.try_reserve_exact(n)
            .map_err(|_| LinalgError::OutOfMemory)?;
        perm.extend(0..n);
        let mut tau = zeros(steps)?;
        // vn1: the downdated norms of the trailing parts of the columns; vn2: the norms they were
        // last computed from.
        let mut vn1 = zeros(n)?;
        for (j, slot) in vn1.iter_mut().enumerate() {
            *slot = column_norm(&qr, j, 0)?;
        }
        let mut vn2 = vn1.clone();
        let mut col = zeros(m)?;
        for k in 0..steps {
            let mut p = k;
            let mut best = -1.0f64;
            for (j, v) in vn1.iter().enumerate().skip(k) {
                if *v > best {
                    best = *v;
                    p = j;
                }
            }
            if p != k {
                qr.swap_cols(k, p)?;
                perm.swap(k, p);
                vn1.swap(k, p);
                vn2.swap(k, p);
            }
            // The reflection for column k, rows k..m.
            for (i, c) in col.iter_mut().enumerate().take(m).skip(k) {
                *c = qr.get(i, k)?;
            }
            let part = col.get_mut(k..m).ok_or(LinalgError::Defect)?;
            let (t, beta) = householder(part)?;
            *tau.get_mut(k).ok_or(LinalgError::Defect)? = t;
            qr.set(k, k, beta)?;
            for (i, v) in part.iter().enumerate().skip(1) {
                qr.set(k.checked_add(i).ok_or(LinalgError::Defect)?, k, *v)?;
            }
            // Apply H to the trailing columns.
            if t != 0.0 {
                reflect_trailing(&mut qr, k, t)?;
            }
            // Downdate the trailing norms (LAPACK xLAQP2), recomputing those that lost accuracy.
            let next = k.checked_add(1).ok_or(LinalgError::Defect)?;
            for j in next..n {
                let (v1, v2) = (
                    *vn1.get(j).ok_or(LinalgError::Defect)?,
                    *vn2.get(j).ok_or(LinalgError::Defect)?,
                );
                if v1 == 0.0 {
                    continue;
                }
                let r = qr.get(k, j)?.abs() / v1;
                let temp = (1.0 - r * r).max_zero();
                let ratio = v1 / v2;
                if temp * ratio * ratio <= SQRT_U {
                    let fresh = column_norm(&qr, j, next)?;
                    *vn1.get_mut(j).ok_or(LinalgError::Defect)? = fresh;
                    *vn2.get_mut(j).ok_or(LinalgError::Defect)? = fresh;
                } else {
                    *vn1.get_mut(j).ok_or(LinalgError::Defect)? = v1 * math::sqrt(temp)?.get();
                }
            }
        }
        Ok(PivotedQr { qr, tau, perm })
    }

    /// The column order Π: column j of A·Π is column `permutation()[j]` of A.
    #[must_use]
    pub fn permutation(&self) -> &[usize] {
        &self.perm
    }

    /// R's diagonal entry k, or `None` beyond min(m, n).
    #[must_use]
    pub fn r_diagonal(&self, k: usize) -> Option<f64> {
        if k < self.tau.len() {
            self.qr.get(k, k).ok()
        } else {
            None
        }
    }

    /// The numerical rank: the number of leading diagonal entries of R larger than `tol` times
    /// the first in magnitude (0 for a zero matrix).
    #[must_use]
    pub fn rank(&self, tol: Real) -> usize {
        let first = self.r_diagonal(0).map_or(0.0, f64::abs);
        if first == 0.0 {
            return 0;
        }
        let limit = tol.get() * first;
        (0..self.tau.len())
            .take_while(|&k| self.r_diagonal(k).is_some_and(|d| d.abs() > limit))
            .count()
    }

    /// b ← Qᵀ·b.
    fn apply_qt(&self, b: &mut [f64]) -> Result<(), LinalgError> {
        let m = self.qr.rows();
        if b.len() != m {
            return Err(LinalgError::Shape);
        }
        for (k, t) in self.tau.iter().enumerate() {
            if *t == 0.0 {
                continue;
            }
            let mut dot = *b.get(k).ok_or(LinalgError::Defect)?;
            for (i, bi) in b
                .iter()
                .enumerate()
                .skip(k.checked_add(1).ok_or(LinalgError::Defect)?)
            {
                dot += self.qr.get(i, k)? * bi;
            }
            let f = t * dot;
            *b.get_mut(k).ok_or(LinalgError::Defect)? -= f;
            for i in k.checked_add(1).ok_or(LinalgError::Defect)?..m {
                let v = self.qr.get(i, k)?;
                let slot = b.get_mut(i).ok_or(LinalgError::Defect)?;
                *slot = finite(*slot - f * v)?;
            }
        }
        Ok(())
    }
}

/// Applies the reflection of step k (τ = `t`, v below the diagonal of column k with v(k) = 1) to
/// the trailing columns: A(k:m, j) −= τ·v·(vᵀ·A(k:m, j)), by rows. w = vᵀ·A is accumulated row
/// after row (for each column the same order as a column dot product, row k first), then each
/// row loses its multiple of w.
fn reflect_trailing(qr: &mut Matrix, k: usize, t: f64) -> Result<(), LinalgError> {
    let (m, n) = (qr.rows(), qr.cols());
    let next = k.checked_add(1).ok_or(LinalgError::Defect)?;
    let mut w = zeros(n.saturating_sub(next))?;
    w.copy_from_slice(qr.row(k)?.get(next..).ok_or(LinalgError::Defect)?);
    for i in next..m {
        let row = qr.row(i)?;
        let vi = *row.get(k).ok_or(LinalgError::Defect)?;
        for (acc, a) in w
            .iter_mut()
            .zip(row.get(next..).ok_or(LinalgError::Defect)?)
        {
            *acc += vi * a;
        }
    }
    for x in &mut w {
        *x *= t;
    }
    for (a, f) in qr
        .row_mut(k)?
        .get_mut(next..)
        .ok_or(LinalgError::Defect)?
        .iter_mut()
        .zip(&w)
    {
        *a = finite(*a - f)?;
    }
    for i in next..m {
        let row = qr.row_mut(i)?;
        let vi = *row.get(k).ok_or(LinalgError::Defect)?;
        for (a, f) in row
            .get_mut(next..)
            .ok_or(LinalgError::Defect)?
            .iter_mut()
            .zip(&w)
        {
            *a = finite(*a - f * vi)?;
        }
    }
    Ok(())
}

/// ‖A(from.., j)‖.
fn column_norm(a: &Matrix, j: usize, from: usize) -> Result<f64, LinalgError> {
    let mut col = zeros(a.rows().saturating_sub(from))?;
    for (k, slot) in col.iter_mut().enumerate() {
        *slot = a.get(from.checked_add(k).ok_or(LinalgError::Defect)?, j)?;
    }
    norm(col.iter().copied())
}

/// max(x, 0) without `f64::max` (banned for its NaN rule; x is finite here).
trait MaxZero {
    fn max_zero(self) -> f64;
}

impl MaxZero for f64 {
    fn max_zero(self) -> f64 {
        if self > 0.0 { self } else { 0.0 }
    }
}

/// A complete orthogonal decomposition A·Π = Q·[T 0; 0 0]·Zᵀ.
#[derive(Clone, Debug, PartialEq)]
pub struct Cod {
    qr: PivotedQr,
    rank: usize,
    /// T in the leading r × r triangle of `qr`'s R; each row k's right reflection vector in
    /// R(k, r..n), with its scalar in `z_tau[k]`.
    z_tau: Vec<f64>,
}

impl Cod {
    /// Factors `a` with rank tolerance `tol` (relative to R's first diagonal entry), charging the
    /// pivoted QR's cost and r²·n more.
    ///
    /// # Errors
    ///
    /// As [`PivotedQr::factor`].
    pub fn factor(a: &Matrix, tol: Real, work: &mut Work) -> Result<Cod, LinalgError> {
        let mut qr = PivotedQr::factor(a, work)?;
        let rank = qr.rank(tol);
        let n = qr.qr.cols();
        work.charge(cost3(rank, rank, n))?;
        let mut z_tau = zeros(rank)?;
        if rank < n {
            // Rows k = r − 1 down to 0: a reflection on coordinates {k} ∪ {r..n} zeroes R(k, r..n).
            for k in (0..rank).rev() {
                let mut x = zeros(n.saturating_sub(rank).saturating_add(1))?;
                *x.first_mut().ok_or(LinalgError::Defect)? = qr.qr.get(k, k)?;
                for (i, slot) in x.iter_mut().enumerate().skip(1) {
                    *slot = qr.qr.get(k, rank.saturating_add(i).saturating_sub(1))?;
                }
                let (t, beta) = householder(&mut x)?;
                *z_tau.get_mut(k).ok_or(LinalgError::Defect)? = t;
                qr.qr.set(k, k, beta)?;
                for (i, v) in x.iter().enumerate().skip(1) {
                    qr.qr.set(k, rank.saturating_add(i).saturating_sub(1), *v)?;
                }
                // Apply it to rows 0..k from the right: row ← row − τ·(row·v)·vᵀ.
                if t != 0.0 {
                    for i in 0..k {
                        let mut dot = qr.qr.get(i, k)?;
                        for (c, v) in x.iter().enumerate().skip(1) {
                            dot += qr.qr.get(i, rank.saturating_add(c).saturating_sub(1))? * v;
                        }
                        let f = t * dot;
                        qr.qr.set(i, k, qr.qr.get(i, k)? - f)?;
                        for (c, v) in x.iter().enumerate().skip(1) {
                            let j = rank.saturating_add(c).saturating_sub(1);
                            qr.qr.set(i, j, qr.qr.get(i, j)? - f * v)?;
                        }
                    }
                }
            }
        }
        Ok(Cod { qr, rank, z_tau })
    }

    /// The numerical rank r.
    #[must_use]
    pub fn rank(&self) -> usize {
        self.rank
    }

    /// The column order Π of the pivoted QR.
    #[must_use]
    pub fn permutation(&self) -> &[usize] {
        self.qr.permutation()
    }

    /// y ← Z·y for y of length n: the right reflections, applied in the order k = 0, 1, … (Z is
    /// their product in the order they were made, k = r − 1 down to 0).
    fn apply_z(&self, y: &mut [f64]) -> Result<(), LinalgError> {
        let n = self.qr.qr.cols();
        let r = self.rank;
        if r == n {
            return Ok(());
        }
        for (k, t) in self.z_tau.iter().enumerate() {
            if *t == 0.0 {
                continue;
            }
            let mut dot = *y.get(k).ok_or(LinalgError::Defect)?;
            for j in r..n {
                dot += self.qr.qr.get(k, j)? * y.get(j).ok_or(LinalgError::Defect)?;
            }
            let f = t * dot;
            *y.get_mut(k).ok_or(LinalgError::Defect)? -= f;
            for j in r..n {
                let v = self.qr.qr.get(k, j)?;
                let slot = y.get_mut(j).ok_or(LinalgError::Defect)?;
                *slot = finite(*slot - f * v)?;
            }
        }
        Ok(())
    }

    /// The least-squares solution of A·x = b of least norm: x = Π·Z·[T⁻¹·(Qᵀ·b)(0..r); 0],
    /// charging 2·m·n multiply-adds.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `b` is not m long; [`LinalgError::NotFinite`];
    /// [`LinalgError::OutOfWork`]; [`LinalgError::OutOfMemory`].
    pub fn least_squares(&self, b: &[f64], work: &mut Work) -> Result<Vec<f64>, LinalgError> {
        let (m, n, r) = (self.qr.qr.rows(), self.qr.qr.cols(), self.rank);
        if b.len() != m {
            return Err(LinalgError::Shape);
        }
        work.charge(wide(m).saturating_mul(wide(n)).saturating_mul(2))?;
        let mut c = zeros(m)?;
        c.copy_from_slice(b);
        self.qr.apply_qt(&mut c)?;
        // T·y = c(0..r), back substitution.
        let mut y = zeros(n)?;
        for i in (0..r).rev() {
            let mut sum = *c.get(i).ok_or(LinalgError::Defect)?;
            for j in i.checked_add(1).ok_or(LinalgError::Defect)?..r {
                sum -= self.qr.qr.get(i, j)? * y.get(j).ok_or(LinalgError::Defect)?;
            }
            let d = self.qr.qr.get(i, i)?;
            if d == 0.0 {
                return Err(LinalgError::Singular);
            }
            *y.get_mut(i).ok_or(LinalgError::Defect)? = finite(sum / d)?;
        }
        self.apply_z(&mut y)?;
        let mut x = zeros(n)?;
        for (j, &p) in self.qr.perm.iter().enumerate() {
            *x.get_mut(p).ok_or(LinalgError::Defect)? = *y.get(j).ok_or(LinalgError::Defect)?;
        }
        Ok(x)
    }

    /// An orthonormal basis of the null space of A, as the n − r columns of an n × (n − r)
    /// matrix: Π·Z·[0; I], charging n²·(n − r) multiply-adds.
    ///
    /// # Errors
    ///
    /// [`LinalgError::NotFinite`]; [`LinalgError::OutOfWork`]; [`LinalgError::OutOfMemory`].
    pub fn null_space(&self, work: &mut Work) -> Result<Matrix, LinalgError> {
        let (n, r) = (self.qr.qr.cols(), self.rank);
        let d = n.saturating_sub(r);
        work.charge(cost3(n, n, d))?;
        let mut basis = Matrix::zeros(n, d)?;
        for c in 0..d {
            let mut y = zeros(n)?;
            *y.get_mut(r.checked_add(c).ok_or(LinalgError::Defect)?)
                .ok_or(LinalgError::Defect)? = 1.0;
            self.apply_z(&mut y)?;
            for (j, &p) in self.qr.perm.iter().enumerate() {
                basis.set(p, c, *y.get(j).ok_or(LinalgError::Defect)?)?;
            }
        }
        Ok(basis)
    }
}

/// The canonical null space of `a` (chapter 15.8): a basis in reduced echelon form over the
/// variable order, one vector per row of the result. Vector i has 1 at its leading variable, 0 at
/// every other vector's leading variable, and the leading variables are chosen in variable order:
/// a variable leads when its row of an orthonormal null-space basis is not, within `tol`, a
/// combination of the rows of the variables already leading. The rank tolerance `tol` serves both
/// decisions. Charges the decomposition and n²·(n − r) more.
///
/// # Errors
///
/// As [`Cod::factor`] and [`Cod::null_space`].
pub fn canonical_null_space(a: &Matrix, tol: Real, work: &mut Work) -> Result<Matrix, LinalgError> {
    let cod = Cod::factor(a, tol, work)?;
    let basis = cod.null_space(work)?;
    let (n, d) = (basis.rows(), basis.cols());
    work.charge(cost3(n, d, d))?;
    // Gauss–Jordan elimination on the d × n transpose, choosing pivot columns (variables) in
    // order; among the remaining rows the largest magnitude pivots, the first on a tie.
    let mut m = Matrix::zeros(d, n)?;
    for i in 0..n {
        for c in 0..d {
            m.set(c, i, basis.get(i, c)?)?;
        }
    }
    let mut row = 0usize;
    for var in 0..n {
        if row == d {
            break;
        }
        let mut p = row;
        let mut best = 0.0f64;
        for i in row..d {
            let v = m.get(i, var)?.abs();
            if v > best {
                best = v;
                p = i;
            }
        }
        if best <= tol.get() {
            continue;
        }
        m.swap_rows(row, p)?;
        let pivot = m.get(row, var)?;
        for x in m.row_mut(row)?.iter_mut() {
            *x = finite(*x / pivot)?;
        }
        m.set(row, var, 1.0)?;
        for i in 0..d {
            if i == row {
                continue;
            }
            let (prow, target) = m.row_pair(row, i)?;
            let f = *target.get(var).ok_or(LinalgError::Defect)?;
            if f == 0.0 {
                continue;
            }
            for (x, y) in target.iter_mut().zip(prow.iter()) {
                *x = finite(*x - f * y)?;
            }
            *target.get_mut(var).ok_or(LinalgError::Defect)? = 0.0;
        }
        row = row.checked_add(1).ok_or(LinalgError::Defect)?;
    }
    if row != d {
        // The basis was not of full rank within tol: a defect of the decomposition's rank choice.
        return Err(LinalgError::Defect);
    }
    Ok(m)
}
