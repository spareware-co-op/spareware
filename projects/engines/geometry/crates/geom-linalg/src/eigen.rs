//! The symmetric eigenproblem: A = Q·Λ·Qᵀ for a symmetric matrix, by Householder
//! tridiagonalisation and the implicit symmetric QR algorithm with Wilkinson's shift.
//!
//! What it does: [`SymmetricEigen::factor`] reduces a symmetric n × n matrix to tridiagonal form T
//! by n − 2 Householder similarity transformations (Golub and Van Loan's algorithm 8.3.1), then
//! applies implicit QR steps with Wilkinson's shift (algorithm 8.3.2), chasing the bulge down T with
//! Givens rotations, until every subdiagonal entry is negligible
//! (|`b_i`| ≤ 2^−52·(|`a_i`| + |`a_(i+1)`|)); the diagonal is then Λ, and the accumulated
//! transformations Q. Eigenvalues come smallest first, each with its unit eigenvector as a column
//! of Q.
//!
//! Guarantees:
//!
//! - **Symmetric or refused.** A matrix that is not square is [`LinalgError::Shape`]; one whose
//!   entries are not exactly symmetric, `a_ij` ≠ `a_ji` for some pair, is
//!   [`LinalgError::NotSymmetric`]. Only the lower triangle is read after that check.
//! - **Deterministic.** Fixed order of reflections, rotations and deflation tests; the shift from
//!   the trailing 2 × 2 block, computed on the block scaled to magnitude 1; IEEE 754 operations in
//!   the order written and the correctly rounded square root; a stable sort of the eigenvalues
//!   with ties broken by position.
//! - **Accuracy, checked against exact rationals.** The method is backward stable: Q·Λ·Qᵀ is a
//!   symmetric matrix within a small multiple of u·‖A‖ of A, so each eigenvalue is within that of
//!   an exact one (Golub and Van Loan, section 8.3.6). The tests check the residual and Q's
//!   orthogonality exactly and bracket each eigenvalue by exact inertia counts of A − x·I.
//! - **Bounded.** At most 30·n QR steps, then [`LinalgError::NoConvergence`]; each step charges its
//!   work before it runs, the reduction its own first.
//!
//! Method: G. H. Golub and C. F. Van Loan, *Matrix Computations*, 4th ed., Johns Hopkins (2013),
//! algorithms 8.3.1 (Householder tridiagonalisation), 8.3.2 (the implicit symmetric QR step with
//! Wilkinson shift) and 8.3.3 (the symmetric QR algorithm), and section 8.3.6 (accuracy); J. H.
//! Wilkinson, *The Algebraic Eigenvalue Problem*, Clarendon Press (1965), chapter 8, for the shift;
//! B. N. Parlett, *The Symmetric Eigenvalue Problem*, SIAM (1998), chapter 8,
//! <https://doi.org/10.1137/1.9781611971163>, for the shift's convergence. Written in our own
//! words from the books (chapter 29.4).
//!
//! Cost: (4/3)·n³ for the reduction and 2·n³ for its accumulation, charged first as 4·n³; each QR
//! step then charges (block + 1)·(4n + 12). One working copy, the tridiagonal and Q.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the book's: a and b for the tridiagonal, c and s for a rotation, x and z for the bulge, q for Q"
)]

extern crate alloc;

use alloc::vec::Vec;

use crate::cols::{Cols, add, at, at_mut, mul, sub};
use crate::matrix::{LinalgError, Matrix, Work, cost3, finite, wide, zeros};
use crate::qr::householder;
use crate::rotation::{givens, hypot, rotate_slices};

/// Twice the unit roundoff, 2^−52: the relative size below which a subdiagonal entry is zero.
const EPS: f64 = f64::from_bits(0x3CB0_0000_0000_0000);

/// The eigenvalues and eigenvectors of a symmetric matrix.
#[derive(Clone, Debug, PartialEq)]
pub struct SymmetricEigen {
    values: Vec<f64>,
    vectors: Matrix,
}

impl SymmetricEigen {
    /// The eigendecomposition of the symmetric matrix `a`.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `a` is not square; [`LinalgError::NotSymmetric`] if it is not
    /// exactly symmetric; [`LinalgError::OutOfWork`] when a charge is more than is left;
    /// [`LinalgError::NoConvergence`] after 30·n QR steps; [`LinalgError::NotFinite`] if a result
    /// is not finite; [`LinalgError::OutOfMemory`] if the factors cannot be allocated.
    pub fn factor(a: &Matrix, work: &mut Work) -> Result<SymmetricEigen, LinalgError> {
        let n = a.rows();
        if a.cols() != n {
            return Err(LinalgError::Shape);
        }
        for i in 0..n {
            for j in 0..i {
                if a.get(i, j)? != a.get(j, i)? {
                    return Err(LinalgError::NotSymmetric);
                }
            }
        }
        work.charge(cost3(n, n, mul(n, 4)?))?;
        let (mut diag, mut off, mut q) = tridiagonalize(a)?;
        symmetric_qr(&mut diag, &mut off, &mut q, work)?;
        let mut order = Vec::new();
        order
            .try_reserve_exact(n)
            .map_err(|_| LinalgError::OutOfMemory)?;
        order.extend(0..n);
        order.sort_by(|&x, &y| {
            let (p, r) = (
                diag.get(x).copied().unwrap_or(0.0),
                diag.get(y).copied().unwrap_or(0.0),
            );
            p.total_cmp(&r).then(x.cmp(&y))
        });
        let mut values = zeros(n)?;
        for (k, &j) in order.iter().enumerate() {
            *at_mut(&mut values, k)? = finite(at(&diag, j)?)?;
        }
        Ok(SymmetricEigen {
            values,
            vectors: q.to_matrix(&order)?,
        })
    }

    /// The eigenvalues, smallest first.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// The unit eigenvectors, column k for eigenvalue k.
    #[must_use]
    pub fn vectors(&self) -> &Matrix {
        &self.vectors
    }
}

/// Householder tridiagonalisation of the symmetric `a` (its lower triangle): the diagonal, the
/// subdiagonal, and Q with A = Q·T·Qᵀ, by columns.
fn tridiagonalize(a: &Matrix) -> Result<(Vec<f64>, Vec<f64>, Cols), LinalgError> {
    let n = a.rows();
    // The working copy, by columns (symmetric, so rows and columns are alike).
    let mut w = Cols::of(a, false)?;
    let mut diag = zeros(n)?;
    let mut off = zeros(n.saturating_sub(1))?;
    let mut taus = zeros(n)?;
    for k in 0..n.saturating_sub(2) {
        let k1 = add(k, 1)?;
        // Reflect column k below the subdiagonal: x = A(k+1.., k).
        let (tau, beta) = householder(w.col_mut(k)?.get_mut(k1..).ok_or(LinalgError::Defect)?)?;
        *at_mut(&mut taus, k)? = tau;
        *at_mut(&mut off, k)? = beta;
        if tau == 0.0 {
            continue;
        }
        // v = (1, x(k+2..)); the trailing block A₂₂ = A(k+1.., k+1..) becomes H·A₂₂·H with
        // p = τ·A₂₂·v, w = p − (τ/2)(pᵀv)·v, A₂₂ ← A₂₂ − v·wᵀ − w·vᵀ.
        let len = sub(n, k1)?;
        let mut v = zeros(len)?;
        *at_mut(&mut v, 0)? = 1.0;
        for (t, x) in v.iter_mut().enumerate().skip(1) {
            *x = w.get(add(k1, t)?, k)?;
        }
        let mut p = zeros(len)?;
        for (i, pi) in p.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (j, &vj) in v.iter().enumerate() {
                sum += w.get(add(k1, i)?, add(k1, j)?)? * vj;
            }
            *pi = tau * sum;
        }
        let pv = p.iter().zip(&v).fold(0.0, |acc, (&x, &y)| acc + x * y);
        let half = tau * pv / 2.0;
        let wv: Vec<f64> = p.iter().zip(&v).map(|(&x, &y)| x - half * y).collect();
        // The lower triangle, mirrored, so the block stays exactly symmetric.
        for j in 0..len {
            for i in j..len {
                let (vi, vj, wi, wj) = (at(&v, i)?, at(&v, j)?, at(&wv, i)?, at(&wv, j)?);
                let (r, c) = (add(k1, i)?, add(k1, j)?);
                let x = finite(w.get(r, c)? - vi * wj - wi * vj)?;
                w.set(r, c, x)?;
                w.set(c, r, x)?;
            }
        }
    }
    for k in 0..n {
        *at_mut(&mut diag, k)? = w.get(k, k)?;
    }
    if n >= 2 {
        let last = sub(n, 2)?;
        *at_mut(&mut off, last)? = w.get(add(last, 1)?, last)?;
    }
    // Q = H₀ ⋯ H_(n−3) applied to the identity; H_k acts on rows k + 1 on.
    let mut q = Cols::identity(n, n)?;
    for k in (0..n.saturating_sub(2)).rev() {
        let tau = at(&taus, k)?;
        if tau == 0.0 {
            continue;
        }
        let k1 = add(k, 1)?;
        let tail: Vec<f64> = w
            .col(k)?
            .get(add(k1, 1)?..)
            .ok_or(LinalgError::Defect)?
            .to_vec();
        for c in 0..n {
            let x = q.col_mut(c)?.get_mut(k1..).ok_or(LinalgError::Defect)?;
            let (head, rest) = x.split_first_mut().ok_or(LinalgError::Defect)?;
            let dot = rest
                .iter()
                .zip(&tail)
                .fold(*head, |acc, (&a, &b)| acc + a * b);
            let f = tau * dot;
            *head = finite(*head - f)?;
            for (xa, &vb) in rest.iter_mut().zip(&tail) {
                *xa = finite(*xa - f * vb)?;
            }
        }
    }
    Ok((diag, off, q))
}

/// Golub and Van Loan's algorithm 8.3.3 on the tridiagonal (a, b), updating the columns of Q,
/// until b is zero.
fn symmetric_qr(
    a: &mut [f64],
    b: &mut [f64],
    q: &mut Cols,
    work: &mut Work,
) -> Result<(), LinalgError> {
    let n = a.len();
    if n < 2 {
        return Ok(());
    }
    let max_steps = mul(n, 30)?;
    let len = q.len();
    let mut steps = 0_usize;
    let mut hi = sub(n, 1)?;
    loop {
        while hi > 0 {
            let k = sub(hi, 1)?;
            if at(b, k)?.abs() <= EPS * (at(a, k)?.abs() + at(a, hi)?.abs()) {
                *at_mut(b, k)? = 0.0;
                hi = k;
            } else {
                break;
            }
        }
        if hi == 0 {
            return Ok(());
        }
        let mut lo = sub(hi, 1)?;
        while lo > 0 {
            let k = sub(lo, 1)?;
            if at(b, k)?.abs() <= EPS * (at(a, k)?.abs() + at(a, lo)?.abs()) {
                *at_mut(b, k)? = 0.0;
                break;
            }
            lo = k;
        }
        steps = add(steps, 1)?;
        if steps > max_steps {
            return Err(LinalgError::NoConvergence);
        }
        let block = add(sub(hi, lo)?, 1)?;
        work.charge(wide(block).saturating_mul(wide(add(mul(len, 4)?, 12)?)))?;
        qr_step(a, b, q, (lo, hi))?;
    }
}

/// One implicit symmetric QR step with Wilkinson's shift on the block lo..=hi (Golub and Van
/// Loan's algorithm 8.3.2).
fn qr_step(
    a: &mut [f64],
    b: &mut [f64],
    q: &mut Cols,
    (lo, hi): (usize, usize),
) -> Result<(), LinalgError> {
    // The shift, from the trailing 2 × 2 block, on the block scaled to magnitude 1.
    let scale = a
        .get(lo..=hi)
        .ok_or(LinalgError::Defect)?
        .iter()
        .chain(b.get(lo..hi).ok_or(LinalgError::Defect)?)
        .fold(0.0, |m, &x| if x.abs() > m { x.abs() } else { m });
    let (am, an, bm) = (
        at(a, sub(hi, 1)?)? / scale,
        at(a, hi)? / scale,
        at(b, sub(hi, 1)?)? / scale,
    );
    let d = (am - an) / 2.0;
    let root = hypot(d, bm)?;
    let denom = if d < 0.0 { d - root } else { d + root };
    let mu = if denom == 0.0 {
        an
    } else {
        an - bm * (bm / denom)
    };
    let mut x = at(a, lo)? / scale - mu;
    let mut z = at(b, lo)? / scale;
    for k in lo..hi {
        let k1 = add(k, 1)?;
        let (c, s, r) = givens(x, z)?;
        if k > lo {
            *at_mut(b, sub(k, 1)?)? = r;
        }
        // T ← J·T·Jᵀ on rows and columns k and k + 1, J = [[c, s], [−s, c]].
        let (p, t, e) = (at(a, k)?, at(a, k1)?, at(b, k)?);
        let cc = c * c;
        let ss = s * s;
        let cs = c * s;
        *at_mut(a, k)? = cc * p + 2.0 * cs * e + ss * t;
        *at_mut(a, k1)? = ss * p - 2.0 * cs * e + cc * t;
        *at_mut(b, k)? = cs * (t - p) + (cc - ss) * e;
        if k1 < hi {
            let e1 = at(b, k1)?;
            z = s * e1;
            *at_mut(b, k1)? = c * e1;
        }
        x = at(b, k)?;
        let (qk, qk1) = q.pair_mut(k, k1)?;
        rotate_slices(qk, qk1, c, s);
    }
    for v in a.iter().chain(b.iter()) {
        finite(*v)?;
    }
    Ok(())
}
