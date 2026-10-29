//! The singular value decomposition, two ways: one-sided Jacobi, accurate to the last bits of tiny
//! singular values of graded matrices, and Golub–Kahan, the fast general method.
//!
//! What it does: [`Svd::jacobi`] and [`Svd::golub_kahan`] factor an m × n matrix as A = U·Σ·Vᵀ,
//! with k = min(m, n) singular values in Σ, largest first, U m × k and V n × k with orthonormal
//! columns. A wide matrix is factored through its transpose.
//!
//! - **One-sided Jacobi** (Hestenes 1958; Demmel and Veselić 1992) rotates pairs of columns of A
//!   until every pair is orthogonal to working precision, |cos| ≤ m·u; then the column norms are the
//!   singular values, the normalised columns U, and the product of the rotations V. Each rotation's
//!   angle comes from the two column norms and their cosine, all computed scaled, so a column far
//!   smaller than the others keeps its own relative accuracy.
//! - **Golub–Kahan** (Golub and Kahan 1965; Golub and Reinsch 1970) reduces A to an upper
//!   bidiagonal B by Householder reflections from both sides, then chases the implicit QR step
//!   with Wilkinson's shift down B until each superdiagonal entry is negligible
//!   (|`e_i`| ≤ 2^−52·(|`d_i`| + |`d_(i+1)`|)), treating a zero on the diagonal by rotating its row
//!   (or the last column) clear first.
//!
//! [`Svd::rank`] counts the singular values above a relative tolerance; [`Svd::classify`] sorts
//! them into chapter 15.8's three verdicts against two absolute thresholds, the caller's until the
//! owner accepts their rungs of the tolerance ladder.
//!
//! Guarantees:
//!
//! - **Deterministic.** Fixed sweep order (pairs (p, q), p < q, row by row), fixed deflation and
//!   shift rules, IEEE 754 operations in the order written, the correctly rounded square root of
//!   `geom_num::math`, no fused multiply-add, and a stable sort with ties broken by column index.
//!   The same input gives the same bits on every target.
//! - **Accuracy, checked against exact rationals.** Both are backward stable: the computed factors
//!   are those of a matrix A + E with ‖E‖ a small multiple of u·‖A‖ (Golub and Van Loan, 8.6.3), so
//!   each singular value is within that of the exact one. Jacobi does better on graded matrices
//!   A = B·D with B well conditioned: each singular value to a small multiple of u·κ(B) in
//!   relative terms, however small (Demmel and Veselić, theorem 4.4). The tests check both claims
//!   with exact rational arithmetic: the residual and the orthogonality of U and V, and each
//!   singular value bracketed by exact inertia counts of Aᵀ·A.
//! - **Bounded.** Jacobi stops after [`MAX_SWEEPS`] sweeps and Golub–Kahan after 6·k² QR steps
//!   with [`LinalgError::NoConvergence`]; neither has ever needed it in the tests. Each sweep or
//!   step charges its work before it runs; the reductions charge theirs first.
//! - **Typed failures.** Columns whose norms differ by more than the doubles can hold
//!   (a rotation angle that is not finite) are [`LinalgError::NotFinite`], never a silently wrong
//!   factor.
//!
//! Method:
//!
//! - M. R. Hestenes, "Inversion of matrices by biorthogonalization and related results", *Journal
//!   of the SIAM* 6(1) (1958), 51–90, <https://doi.org/10.1137/0106005>.
//! - J. Demmel and K. Veselić, "Jacobi's method is more accurate than QR", *SIAM Journal on Matrix
//!   Analysis and Applications* 13(4) (1992), 1204–1245, <https://doi.org/10.1137/0613074>.
//! - G. Golub and W. Kahan, "Calculating the singular values and pseudo-inverse of a matrix",
//!   *Journal of the SIAM, Series B* 2(2) (1965), 205–224, <https://doi.org/10.1137/0702016>.
//! - G. H. Golub and C. F. Van Loan, *Matrix Computations*, 4th ed., Johns Hopkins (2013),
//!   algorithms 5.4.2 (bidiagonalisation), 8.6.1 and 8.6.2 (the Golub–Kahan SVD step and
//!   algorithm) and section 8.6.3 (accuracy).
//!
//! Written in our own words from these books and papers (chapter 29.4).
//!
//! Cost: Jacobi charges n²·(5m + 2n) per sweep and converges in a handful of sweeps; Golub–Kahan
//! charges 8·m·n² + 4·n³ for the reduction and the accumulation, then (k + 1)·(4(m + n) + 12) per
//! QR step. Each allocates its working copy, U and V.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the books': m × n for the shape, d and e for the bidiagonal, p and q for a pair of columns, c and s for a rotation, u and v for the factors"
)]

extern crate alloc;

use alloc::vec::Vec;

use geom_num::math;
use geom_num::real::Real;

use crate::cols::{Cols, add, at, at_mut, mul, sub};
use crate::matrix::{LinalgError, Matrix, Work, cost3, finite, wide, zeros};
use crate::qr::{householder, norm};
use crate::rotation::{givens, hypot, rotate_slices};

/// The unit roundoff, 2^−53.
const U: f64 = f64::from_bits(0x3CA0_0000_0000_0000);

/// Twice the unit roundoff, 2^−52: the relative size below which a superdiagonal entry is zero.
const EPS: f64 = f64::from_bits(0x3CB0_0000_0000_0000);

/// The most sweeps one-sided Jacobi makes.
pub const MAX_SWEEPS: usize = 64;

/// A singular value decomposition A = U·Σ·Vᵀ.
#[derive(Clone, Debug, PartialEq)]
pub struct Svd {
    u: Matrix,
    s: Vec<f64>,
    v: Matrix,
}

/// How many singular values fall in each of chapter 15.8's verdicts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ranks {
    /// At least the independence threshold.
    pub independent: usize,
    /// Between the two thresholds: reported, never rounded either way.
    pub near_singular: usize,
    /// At most the dependence threshold.
    pub dependent: usize,
}

impl Svd {
    /// The SVD of `a` by one-sided Jacobi.
    ///
    /// # Errors
    ///
    /// [`LinalgError::OutOfWork`] when a sweep's charge is more than is left;
    /// [`LinalgError::NoConvergence`] after [`MAX_SWEEPS`] sweeps; [`LinalgError::NotFinite`] if a
    /// rotation or a result is not finite; [`LinalgError::OutOfMemory`] if the factors cannot be
    /// allocated.
    pub fn jacobi(a: &Matrix, work: &mut Work) -> Result<Svd, LinalgError> {
        let wide_a = a.rows() < a.cols();
        let mut w = Cols::of(a, wide_a)?;
        let (m, n) = (w.len(), w.count());
        let mut v = Cols::identity(n, n)?;
        jacobi_sweeps(&mut w, &mut v, work)?;
        let mut s = zeros(n)?;
        for j in 0..n {
            let sigma = norm(w.col(j)?.iter().copied())?;
            *at_mut(&mut s, j)? = sigma;
            if sigma > 0.0 {
                for x in w.col_mut(j)? {
                    *x = finite(*x / sigma)?;
                }
            }
        }
        let order = descending(&s)?;
        complete(&mut w, &s, &order)?;
        let _ = m;
        finish(&w, &v, &s, &order, wide_a)
    }

    /// The SVD of `a` by Golub–Kahan bidiagonalisation and implicit QR.
    ///
    /// # Errors
    ///
    /// As for [`Svd::jacobi`], with 6·k² QR steps in place of sweeps.
    pub fn golub_kahan(a: &Matrix, work: &mut Work) -> Result<Svd, LinalgError> {
        let wide_a = a.rows() < a.cols();
        let mut w = Cols::of(a, wide_a)?;
        let (m, n) = (w.len(), w.count());
        work.charge(cost3(m, n, mul(n, 8)?).saturating_add(cost3(n, n, mul(n, 4)?)))?;
        let (mut d, mut e, mut uu, mut vv) = bidiagonalize(&mut w)?;
        bidiagonal_qr(&mut d, &mut e, &mut uu, &mut vv, work)?;
        // σ = |d|, the sign moved into V.
        for (j, dj) in d.iter_mut().enumerate() {
            if *dj < 0.0 {
                *dj = -*dj;
                for x in vv.col_mut(j)? {
                    *x = -*x;
                }
            }
        }
        let order = descending(&d)?;
        finish(&uu, &vv, &d, &order, wide_a)
    }

    /// The singular values, largest first.
    #[must_use]
    pub fn singular_values(&self) -> &[f64] {
        &self.s
    }

    /// U, m × k, orthonormal columns.
    #[must_use]
    pub fn u(&self) -> &Matrix {
        &self.u
    }

    /// V, n × k, orthonormal columns.
    #[must_use]
    pub fn v(&self) -> &Matrix {
        &self.v
    }

    /// The number of singular values above `tol` times the largest.
    #[must_use]
    pub fn rank(&self, tol: Real) -> usize {
        let Some(&first) = self.s.first() else {
            return 0;
        };
        let bound = tol.get() * first;
        self.s.iter().filter(|&&x| x > bound && x > 0.0).count()
    }

    /// The singular values sorted into chapter 15.8's verdicts: at least `independent`, at most
    /// `dependent`, or between.
    ///
    /// # Errors
    ///
    /// [`LinalgError::Shape`] if `dependent` is above `independent`.
    pub fn classify(&self, independent: Real, dependent: Real) -> Result<Ranks, LinalgError> {
        if dependent.get() > independent.get() {
            return Err(LinalgError::Shape);
        }
        let mut r = Ranks {
            independent: 0,
            near_singular: 0,
            dependent: 0,
        };
        for &x in &self.s {
            if x >= independent.get() {
                r.independent = add(r.independent, 1)?;
            } else if x <= dependent.get() {
                r.dependent = add(r.dependent, 1)?;
            } else {
                r.near_singular = add(r.near_singular, 1)?;
            }
        }
        Ok(r)
    }
}

/// The sweeps of one-sided Jacobi on the columns of `w` (len ≥ count), accumulating the rotations
/// in `v`.
fn jacobi_sweeps(w: &mut Cols, v: &mut Cols, work: &mut Work) -> Result<(), LinalgError> {
    let (m, n) = (w.len(), w.count());
    let tol = f64::from(u32::try_from(m).map_err(|_| LinalgError::Shape)?) * U;
    let per_sweep = cost3(n, n, add(mul(m, 5)?, mul(n, 2)?)?);
    for _ in 0..MAX_SWEEPS {
        work.charge(per_sweep)?;
        let mut rotated = false;
        for p in 0..n {
            for q in add(p, 1)?..n {
                let (cp, cq) = w.pair_mut(p, q)?;
                let np = norm(cp.iter().copied())?;
                let nq = norm(cq.iter().copied())?;
                if np == 0.0 || nq == 0.0 {
                    continue;
                }
                // The cosine of the angle between the columns, from the normalised columns.
                let g = cp
                    .iter()
                    .zip(cq.iter())
                    .fold(0.0, |acc, (&x, &y)| acc + (x / np) * (y / nq));
                if g.abs() <= tol {
                    continue;
                }
                rotated = true;
                let ratio = nq / np;
                let zeta = (ratio - 1.0 / ratio) / (2.0 * g);
                if !zeta.is_finite() {
                    return Err(LinalgError::NotFinite);
                }
                // The smaller root of t² + 2ζt − 1 = 0; for |ζ| beyond 2^26, 1 + ζ² is ζ².
                let t = if zeta.abs() > 67_108_864.0 {
                    1.0 / (2.0 * zeta)
                } else {
                    let sign = if zeta < 0.0 { -1.0 } else { 1.0 };
                    sign / (zeta.abs() + math::sqrt(1.0 + zeta * zeta)?.get())
                };
                let c = 1.0 / math::sqrt(1.0 + t * t)?.get();
                let s = c * t;
                // a_p ← c·a_p − s·a_q, a_q ← s·a_p + c·a_q.
                rotate_slices(cp, cq, c, -s);
                let (vp, vq) = v.pair_mut(p, q)?;
                rotate_slices(vp, vq, c, -s);
            }
        }
        if !rotated {
            return Ok(());
        }
    }
    Err(LinalgError::NoConvergence)
}

/// Column indices in order of `s`, largest first, ties by index.
fn descending(s: &[f64]) -> Result<Vec<usize>, LinalgError> {
    let mut order = Vec::new();
    order
        .try_reserve_exact(s.len())
        .map_err(|_| LinalgError::OutOfMemory)?;
    order.extend(0..s.len());
    order.sort_by(|&a, &b| {
        let (x, y) = (
            s.get(a).copied().unwrap_or(0.0),
            s.get(b).copied().unwrap_or(0.0),
        );
        y.total_cmp(&x).then(a.cmp(&b))
    });
    Ok(order)
}

/// Gives each column of `w` whose singular value is zero a unit vector orthogonal to the columns
/// before it in `order`: of the unit vectors eᵢ after two passes of Gram–Schmidt against them
/// ("twice is enough", Parlett's *The Symmetric Eigenvalue Problem*, section 6.9), the one that
/// keeps the most length, the first of equal ones. With r orthonormal columns in m dimensions
/// (r < m), the squared lengths left sum to m − r ≥ 1, so the longest keeps at least 1/√m: a fixed
/// threshold such as 1/2 can be missed by every eᵢ (three columns of a 4 × 4 Hadamard matrix leave
/// each exactly 1/2).
fn complete(w: &mut Cols, s: &[f64], order: &[usize]) -> Result<(), LinalgError> {
    let m = w.len();
    for (rank, &j) in order.iter().enumerate() {
        if at(s, j)? != 0.0 {
            continue;
        }
        let mut best: Option<(f64, Vec<f64>)> = None;
        for i in 0..m {
            let mut x = zeros(m)?;
            *at_mut(&mut x, i)? = 1.0;
            for _ in 0..2 {
                for &k in order.get(..rank).ok_or(LinalgError::Defect)? {
                    let uk = w.col(k)?;
                    let dot = uk.iter().zip(&x).fold(0.0, |acc, (&a, &b)| acc + a * b);
                    for (xi, &ui) in x.iter_mut().zip(uk) {
                        *xi -= dot * ui;
                    }
                }
            }
            let len = norm(x.iter().copied())?;
            if best.as_ref().is_none_or(|(longest, _)| len > *longest) {
                best = Some((len, x));
            }
        }
        let (len, x) = best.ok_or(LinalgError::Defect)?;
        if len == 0.0 {
            // At least 1/√m is left (above): a defect, reported, never divided by.
            return Err(LinalgError::Defect);
        }
        for (dst, &src) in w.col_mut(j)?.iter_mut().zip(&x) {
            *dst = src / len;
        }
    }
    Ok(())
}

/// The factorisation from the columns of U and V and the singular values, sorted by `order`, and
/// swapped back if the matrix was factored through its transpose.
fn finish(
    u: &Cols,
    v: &Cols,
    s: &[f64],
    order: &[usize],
    transposed: bool,
) -> Result<Svd, LinalgError> {
    let mut sorted = zeros(order.len())?;
    for (k, &j) in order.iter().enumerate() {
        *at_mut(&mut sorted, k)? = finite(at(s, j)?)?;
    }
    let (um, vm) = (u.to_matrix(order)?, v.to_matrix(order)?);
    Ok(if transposed {
        Svd {
            u: vm,
            s: sorted,
            v: um,
        }
    } else {
        Svd {
            u: um,
            s: sorted,
            v: vm,
        }
    })
}

/// Householder bidiagonalisation of the columns of `w` (len m ≥ count n): the diagonal d, the
/// superdiagonal e, and U (m × n) and V (n × n) with A = U·B·Vᵀ.
fn bidiagonalize(w: &mut Cols) -> Result<(Vec<f64>, Vec<f64>, Cols, Cols), LinalgError> {
    let (m, n) = (w.len(), w.count());
    let mut d = zeros(n)?;
    let mut e = zeros(n.saturating_sub(1))?;
    let mut tau_left = zeros(n)?;
    let mut tau_right = zeros(n)?;
    // The right reflections' vectors, row j's from column j + 1 on, stored by row.
    let mut right = Cols::zeros(n, n)?;
    for j in 0..n {
        // Left: zero column j below the diagonal.
        let (tau, beta) = householder(w.col_mut(j)?.get_mut(j..).ok_or(LinalgError::Defect)?)?;
        *at_mut(&mut tau_left, j)? = tau;
        *at_mut(&mut d, j)? = beta;
        let vj: Vec<f64> = w
            .col(j)?
            .get(add(j, 1)?..)
            .ok_or(LinalgError::Defect)?
            .to_vec();
        for c in add(j, 1)?..n {
            reflect(
                w.col_mut(c)?.get_mut(j..).ok_or(LinalgError::Defect)?,
                &vj,
                tau,
            )?;
        }
        if add(j, 1)? >= n {
            continue;
        }
        // Right: zero row j beyond the superdiagonal.
        let mut row = zeros(sub(n, add(j, 1)?)?)?;
        for (k, x) in row.iter_mut().enumerate() {
            *x = w.get(j, add(add(j, 1)?, k)?)?;
        }
        let (tau, beta) = householder(&mut row)?;
        *at_mut(&mut tau_right, j)? = tau;
        *at_mut(&mut e, j)? = beta;
        let tail = row.get(1..).ok_or(LinalgError::Defect)?;
        for (k, &x) in tail.iter().enumerate() {
            right.set(add(add(j, 2)?, k)?, j, x)?;
        }
        for i in add(j, 1)?..m {
            let mut r = zeros(sub(n, add(j, 1)?)?)?;
            for (k, x) in r.iter_mut().enumerate() {
                *x = w.get(i, add(add(j, 1)?, k)?)?;
            }
            reflect(&mut r, tail, tau)?;
            for (k, &x) in r.iter().enumerate() {
                w.set(i, add(add(j, 1)?, k)?, x)?;
            }
        }
    }
    // U = H₀ ⋯ H_(n−1) applied to the first n columns of the identity.
    let mut uu = Cols::identity(m, n)?;
    for j in (0..n).rev() {
        let vj: Vec<f64> = w
            .col(j)?
            .get(add(j, 1)?..)
            .ok_or(LinalgError::Defect)?
            .to_vec();
        let tau = at(&tau_left, j)?;
        for c in 0..n {
            reflect(
                uu.col_mut(c)?.get_mut(j..).ok_or(LinalgError::Defect)?,
                &vj,
                tau,
            )?;
        }
    }
    // V = G₀ ⋯ G_(n−2) applied to the identity; G_j acts on rows j + 1 on.
    let mut vv = Cols::identity(n, n)?;
    for j in (0..n.saturating_sub(1)).rev() {
        let tail: Vec<f64> = right.col(j)?.get(add(j, 2)?..).unwrap_or(&[]).to_vec();
        let tau = at(&tau_right, j)?;
        for c in 0..n {
            reflect(
                vv.col_mut(c)?
                    .get_mut(add(j, 1)?..)
                    .ok_or(LinalgError::Defect)?,
                &tail,
                tau,
            )?;
        }
    }
    Ok((d, e, uu, vv))
}

/// x ← (I − τ·v·vᵀ)·x with v = (1, `tail`).
fn reflect(x: &mut [f64], tail: &[f64], tau: f64) -> Result<(), LinalgError> {
    if tau == 0.0 {
        return Ok(());
    }
    let (head, rest) = x.split_first_mut().ok_or(LinalgError::Defect)?;
    let dot = rest
        .iter()
        .zip(tail)
        .fold(*head, |acc, (&a, &b)| acc + a * b);
    let f = tau * dot;
    *head = finite(*head - f)?;
    for (a, &b) in rest.iter_mut().zip(tail) {
        *a = finite(*a - f * b)?;
    }
    Ok(())
}

/// The largest magnitude of the bidiagonal (d, e).
fn bidiagonal_norm(d: &[f64], e: &[f64]) -> f64 {
    d.iter()
        .chain(e)
        .fold(0.0, |m, &x| if x.abs() > m { x.abs() } else { m })
}

/// Golub and Van Loan's algorithm 8.6.2 on the bidiagonal (d, e), updating the columns of U and
/// V, until e is zero.
fn bidiagonal_qr(
    d: &mut [f64],
    e: &mut [f64],
    u: &mut Cols,
    v: &mut Cols,
    work: &mut Work,
) -> Result<(), LinalgError> {
    let n = d.len();
    if n < 2 {
        return Ok(());
    }
    let bnorm = bidiagonal_norm(d, e);
    let small = EPS * bnorm;
    let max_steps = mul(mul(n, n)?, 6)?;
    let (m, nv) = (u.len(), v.len());
    let mut steps = 0_usize;
    let mut hi = sub(n, 1)?;
    loop {
        // Deflate from the bottom.
        while hi > 0 {
            let k = sub(hi, 1)?;
            if at(e, k)?.abs() <= EPS * (at(d, k)?.abs() + at(d, hi)?.abs()) {
                *at_mut(e, k)? = 0.0;
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
            if at(e, k)?.abs() <= EPS * (at(d, k)?.abs() + at(d, lo)?.abs()) {
                *at_mut(e, k)? = 0.0;
                break;
            }
            lo = k;
        }
        steps = add(steps, 1)?;
        if steps > max_steps {
            return Err(LinalgError::NoConvergence);
        }
        let block = add(sub(hi, lo)?, 1)?;
        work.charge(wide(block).saturating_mul(wide(add(mul(add(m, nv)?, 4)?, 12)?)))?;
        if let Some(k) = (lo..hi).find(|&k| d.get(k).is_some_and(|x| x.abs() <= small)) {
            zero_row(d, e, u, (k, hi))?;
            continue;
        }
        if at(d, hi)?.abs() <= small {
            zero_column(d, e, v, (lo, hi))?;
            continue;
        }
        golub_kahan_step(d, e, u, v, (lo, hi))?;
    }
}

/// d[k] is negligible: set it to zero and chase e[k] along row k to the right with rotations from
/// the left, until the row is clear.
fn zero_row(
    d: &mut [f64],
    e: &mut [f64],
    u: &mut Cols,
    (k, hi): (usize, usize),
) -> Result<(), LinalgError> {
    *at_mut(d, k)? = 0.0;
    let mut f = at(e, k)?;
    *at_mut(e, k)? = 0.0;
    for j in add(k, 1)?..=hi {
        let (c, s, r) = givens(at(d, j)?, f)?;
        *at_mut(d, j)? = r;
        if j < hi {
            let ej = at(e, j)?;
            f = -s * ej;
            *at_mut(e, j)? = c * ej;
        }
        let (uk, uj) = u.pair_mut(k, j)?;
        // Columns (j, k) ← (c·u_j + s·u_k, −s·u_j + c·u_k).
        rotate_slices(uj, uk, c, s);
    }
    Ok(())
}

/// d[hi] is negligible: set it to zero and chase e[hi − 1] up column hi with rotations from the
/// right, until the column is clear.
fn zero_column(
    d: &mut [f64],
    e: &mut [f64],
    v: &mut Cols,
    (lo, hi): (usize, usize),
) -> Result<(), LinalgError> {
    *at_mut(d, hi)? = 0.0;
    let top = sub(hi, 1)?;
    let mut f = at(e, top)?;
    *at_mut(e, top)? = 0.0;
    for j in (lo..hi).rev() {
        let (c, s, r) = givens(at(d, j)?, f)?;
        *at_mut(d, j)? = r;
        if j > lo {
            let k = sub(j, 1)?;
            let ek = at(e, k)?;
            f = -s * ek;
            *at_mut(e, k)? = c * ek;
        }
        let (vj, vh) = v.pair_mut(j, hi)?;
        rotate_slices(vj, vh, c, s);
    }
    Ok(())
}

/// One implicit QR step with Wilkinson's shift on the block lo..=hi of the bidiagonal (Golub and
/// Van Loan's algorithm 8.6.1), chasing the bulge from the top to the bottom.
fn golub_kahan_step(
    d: &mut [f64],
    e: &mut [f64],
    u: &mut Cols,
    v: &mut Cols,
    (lo, hi): (usize, usize),
) -> Result<(), LinalgError> {
    // The shift, from the trailing 2 × 2 of Bᵀ·B, computed on the block scaled to magnitude 1.
    let scale = bidiagonal_norm(
        d.get(lo..=hi).ok_or(LinalgError::Defect)?,
        e.get(lo..hi).ok_or(LinalgError::Defect)?,
    );
    let at_s = |x: f64| x / scale;
    let (dm, dn) = (at_s(at(d, sub(hi, 1)?)?), at_s(at(d, hi)?));
    let em = at_s(at(e, sub(hi, 1)?)?);
    let el = if sub(hi, 1)? > lo {
        at_s(at(e, sub(hi, 2)?)?)
    } else {
        0.0
    };
    let ta = dm * dm + el * el;
    let tb = dm * em;
    let tc = dn * dn + em * em;
    let delta = (ta - tc) / 2.0;
    let root = hypot(delta, tb)?;
    let denom = if delta < 0.0 {
        delta - root
    } else {
        delta + root
    };
    let mu = if denom == 0.0 {
        tc
    } else {
        tc - tb * (tb / denom)
    };
    let d0 = at_s(at(d, lo)?);
    let mut y = d0 * d0 - mu;
    let mut z = d0 * at_s(at(e, lo)?);
    for k in lo..hi {
        let k1 = add(k, 1)?;
        // From the right, on columns k and k + 1.
        let (c, s, r) = givens(y, z)?;
        if k > lo {
            *at_mut(e, sub(k, 1)?)? = r;
        }
        let (dk, ek, dk1) = (at(d, k)?, at(e, k)?, at(d, k1)?);
        y = c * dk + s * ek;
        *at_mut(e, k)? = -s * dk + c * ek;
        z = s * dk1;
        *at_mut(d, k1)? = c * dk1;
        let (vk, vk1) = v.pair_mut(k, k1)?;
        rotate_slices(vk, vk1, c, s);
        // From the left, on rows k and k + 1.
        let (c, s, r) = givens(y, z)?;
        *at_mut(d, k)? = r;
        let (ek, dk1) = (at(e, k)?, at(d, k1)?);
        y = c * ek + s * dk1;
        *at_mut(d, k1)? = -s * ek + c * dk1;
        if k1 < hi {
            let ek1 = at(e, k1)?;
            z = s * ek1;
            *at_mut(e, k1)? = c * ek1;
        }
        let (uk, uk1) = u.pair_mut(k, k1)?;
        rotate_slices(uk, uk1, c, s);
    }
    *at_mut(e, sub(hi, 1)?)? = y;
    for x in d.iter().chain(e.iter()) {
        finite(*x)?;
    }
    Ok(())
}
