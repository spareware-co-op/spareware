//! Fuzz target: the dense decompositions are refused or finite, and the spectral ones backward
//! stable, on any bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a matrix of up to
//! [`MAX_DIM`] × [`MAX_DIM`] entries. [`check`] runs `geom-linalg`'s decompositions on it and
//! holds each to its stated guarantees:
//!
//! - **Refused, never guessed** (piece G49): LU, Cholesky, the pivoted QR and the complete
//!   orthogonal decomposition either refuse with a typed error or return finite factors whose
//!   permutations are permutations and whose rank is at most min(m, n).
//! - **Backward stable** (G50): both SVDs return min(m, n) singular values, non-negative and in
//!   descending order, with every entry of A − U·Σ·Vᵀ within 8·(m + n)²·u·max|`a_ij`| and every
//!   entry of `UᵀU − I` and `VᵀV − I` within 8·(m + n)²·u, decided exactly; the symmetric
//!   eigensolver, on the matrix's symmetric part, likewise with `Q·Λ·Qᵀ` and `QᵀQ − I`. These are the
//!   bounds `geom-linalg`'s own tests hold its "small multiple of u·‖A‖" to; each also allows
//!   gradual underflow, 8·(m + n)²·2^−1074.
//!
//! [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, through the rule bytes of the `curves` target: 0, ±1, whole
//!   numbers and quarters, tiny and huge powers of two, or a double read from the input.
//! - **What is a finding.** A law above broken, decided exactly in dashu's rationals. Every typed
//!   error is legal (the guarantees say which refusals each method makes); a panic fails.
//! - **No hang.** Each call has a work budget of [`WORK`] units.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below.
//!
//! Method: the guarantees in `geom_linalg::solve`, `qr`, `svd` and `eigen`, checked exactly as
//! their tests do, driven by coverage-guided fuzzing (chapter 28.5,
//! `projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: each decomposition of a matrix of at most [`MAX_DIM`]² entries, and their exact
//! residuals, O(n³) rational operations; at most [`MAX_INPUT_BYTES`] bytes read.

// The lint is for fixed-width arithmetic, which can overflow. The oracle's arithmetic is on
// dashu's arbitrary-precision rationals, which cannot; the indices are below the matrix's sizes.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "exact rational arithmetic in the oracle, indices below the matrix's sizes, and the formulas' names (a, u, v, q, m, n)"
)]

use dashu_ratio::RBig;
use geom_linalg::{Cholesky, Cod, Lu, Matrix, PivotedQr, Svd, SymmetricEigen, Work};
use geom_num::Real;
use geom_test::{ByteSource, Entropy};

use crate::Finding;
use crate::curves::value;

/// The largest number of rows or columns.
pub const MAX_DIM: usize = 5;

/// The work units each decomposition may spend.
pub const WORK: u64 = 1_000_000;

/// The most bytes [`decode`] reads from any input: 227.
///
/// Two size bytes, then [`MAX_DIM`]² entries, each a rule byte and at most eight bytes more:
/// 2 + 25 · 9 = 227. The unit test `the_input_bound_is_reached_and_never_passed` builds an input
/// that reads exactly this many.
pub const MAX_INPUT_BYTES: usize = 2 + MAX_DIM * MAX_DIM * 9;

/// One input: a matrix's sizes and its entries, row by row.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// Rows, 1 to [`MAX_DIM`].
    pub rows: usize,
    /// Columns, 1 to [`MAX_DIM`].
    pub cols: usize,
    /// The entries, row by row.
    pub entries: Vec<f64>,
}

/// Any bytes as a case, and how many bytes were read.
#[must_use]
pub fn decode(bytes: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(bytes);
    let rows = usize::from(src.byte()) % MAX_DIM + 1;
    let cols = usize::from(src.byte()) % MAX_DIM + 1;
    let entries = (0..rows * cols).map(|_| value(&mut src)).collect();
    let read = bytes.len().saturating_sub(src.remaining());
    (
        Case {
            rows,
            cols,
            entries,
        },
        read,
    )
}

fn q(x: f64) -> RBig {
    RBig::try_from(x).unwrap_or(RBig::ZERO)
}

fn abs(x: &RBig) -> RBig {
    if *x < RBig::ZERO {
        -x.clone()
    } else {
        x.clone()
    }
}

/// The exact entries of a matrix.
fn exact(a: &Matrix) -> Vec<Vec<RBig>> {
    (0..a.rows())
        .map(|i| {
            (0..a.cols())
                .map(|j| q(a.get(i, j).unwrap_or(0.0)))
                .collect()
        })
        .collect()
}

/// u = 2^−53, and the underflow unit 2^−1074, exactly.
fn units() -> (RBig, RBig) {
    (q(1.0 / 9_007_199_254_740_992.0), q(f64::from_bits(1)))
}

/// The bounds for sizes m and n: (residual per max|a|, orthogonality, underflow), as
/// 8·(m + n)²·u, 8·(m + n)²·u and 8·(m + n)²·2^−1074.
fn bounds(m: usize, n: usize) -> (RBig, RBig) {
    let (u, tiny) = units();
    let p = RBig::from(8 * (m + n) * (m + n));
    (&p * u, p * tiny)
}

/// The largest |entry| of `a`.
fn max_abs(a: &[Vec<RBig>]) -> RBig {
    a.iter()
        .flatten()
        .map(abs)
        .fold(RBig::ZERO, |m, x| if x > m { x } else { m })
}

/// The largest |entry| of `QᵀQ − I` for the columns of `q` (n × k).
fn orthogonality(qm: &[Vec<RBig>]) -> RBig {
    let k = qm.first().map_or(0, Vec::len);
    let mut worst = RBig::ZERO;
    for i in 0..k {
        for j in 0..k {
            let mut d = qm
                .iter()
                .fold(RBig::ZERO, |acc, row| match (row.get(i), row.get(j)) {
                    (Some(x), Some(y)) => acc + x * y,
                    _ => acc,
                });
            if i == j {
                d -= RBig::ONE;
            }
            let d = abs(&d);
            if d > worst {
                worst = d;
            }
        }
    }
    worst
}

fn found(law: &str, detail: impl FnOnce() -> String) -> Result<(), Finding> {
    Err(Finding::new(law, detail))
}

fn is_permutation(p: &[usize], n: usize) -> bool {
    let mut seen = vec![false; n];
    p.len() == n
        && p.iter().all(|&i| {
            seen.get_mut(i)
                .is_some_and(|s| !core::mem::replace(s, true))
        })
}

fn finite(a: &Matrix) -> bool {
    (0..a.rows()).all(|i| (0..a.cols()).all(|j| a.get(i, j).is_ok_and(f64::is_finite)))
}

fn dense(a: &Matrix, case: &Case) -> Result<(), Finding> {
    let (m, n) = (case.rows, case.cols);
    if let Ok(lu) = Lu::factor(a, &mut Work::new(WORK))
        && !(finite(lu.factors()) && is_permutation(lu.permutation(), m))
    {
        return found("LU is refused or finite, with a permutation", || {
            format!("{case:?}")
        });
    }
    if let Ok(ch) = Cholesky::factor(a, &mut Work::new(WORK))
        && !finite(ch.factor_g())
    {
        return found("Cholesky is refused or finite", || format!("{case:?}"));
    }
    let Ok(tol) = Real::new(1e-12) else {
        return Ok(());
    };
    if let Ok(qr) = PivotedQr::factor(a, &mut Work::new(WORK)) {
        if !is_permutation(qr.permutation(), n) || qr.rank(tol) > m.min(n) {
            return found(
                "a pivoted QR's pivots are a permutation and its rank at most min(m, n)",
                || format!("{case:?}"),
            );
        }
    }
    if let Ok(cod) = Cod::factor(a, tol, &mut Work::new(WORK))
        && (!is_permutation(cod.permutation(), n) || cod.rank() > m.min(n))
    {
        return found(
            "a complete orthogonal decomposition's pivots are a permutation and its rank at most min(m, n)",
            || format!("{case:?}"),
        );
    }
    Ok(())
}

fn spectral(a: &Matrix, case: &Case) -> Result<(), Finding> {
    let (m, n) = (case.rows, case.cols);
    let ae = exact(a);
    let scale = max_abs(&ae);
    let (per, tiny) = bounds(m, n);
    for (name, f) in [
        ("Jacobi", Svd::jacobi as fn(&Matrix, &mut Work) -> _),
        ("Golub–Kahan", Svd::golub_kahan),
    ] {
        let Ok(svd) = f(a, &mut Work::new(WORK)) else {
            continue;
        };
        let s = svd.singular_values();
        if s.len() != m.min(n)
            || s.iter().any(|&x| x < 0.0 || !x.is_finite())
            || s.windows(2).any(|w| matches!(w, [a, b] if a < b))
        {
            return found(
                "an SVD's singular values are min(m, n), finite, non-negative and descending",
                || format!("{name} on {case:?}: {s:?}"),
            );
        }
        let (u, v) = (exact(svd.u()), exact(svd.v()));
        let mut res = RBig::ZERO;
        for (ae_row, u_row) in ae.iter().zip(&u) {
            for (aij, v_row) in ae_row.iter().zip(&v) {
                let mut r = aij.clone();
                for ((ut, vt), st) in u_row.iter().zip(v_row).zip(s) {
                    r -= ut * q(*st) * vt;
                }
                let r = abs(&r);
                if r > res {
                    res = r;
                }
            }
        }
        if res > &per * &scale + &tiny {
            return found(
                "an SVD is backward stable: A − U·Σ·Vᵀ within 8·(m + n)²·u·max|a|",
                || format!("{name} on {case:?}: {res}"),
            );
        }
        if orthogonality(&u) > &per + &tiny || orthogonality(&v) > &per + &tiny {
            return found(
                "an SVD's U and V are orthonormal within 8·(m + n)²·u",
                || format!("{name} on {case:?}"),
            );
        }
    }
    // The symmetric part, exactly symmetric: (A + Aᵀ)/2, each pair rounded the same way.
    if m == n {
        let mut sym = Vec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                let entry = |r: usize, c: usize| case.entries.get(r * n + c).copied();
                let (x, y) = (entry(i, j).unwrap_or(0.0), entry(j, i).unwrap_or(0.0));
                // IEEE addition is commutative, so the pair (i, j), (j, i) gets the same value.
                sym.push(x / 2.0 + y / 2.0);
            }
        }
        if let Ok(s) = Matrix::from_rows(n, n, &sym)
            && let Ok(eig) = SymmetricEigen::factor(&s, &mut Work::new(WORK))
        {
            let se = exact(&s);
            let qm = exact(eig.vectors());
            let vals = eig.values();
            let scale = max_abs(&se);
            let (per, tiny) = bounds(n, n);
            let mut res = RBig::ZERO;
            for (se_row, q_row) in se.iter().zip(&qm) {
                for (k, val) in vals.iter().enumerate() {
                    // (A·Q − Q·Λ)_ik.
                    let Some(qik) = q_row.get(k) else { continue };
                    let mut r = -(qik * q(*val));
                    for (sij, q_j) in se_row.iter().zip(&qm) {
                        if let Some(qjk) = q_j.get(k) {
                            r += sij * qjk;
                        }
                    }
                    let r = abs(&r);
                    if r > res {
                        res = r;
                    }
                }
            }
            if res > &per * &scale + &tiny || orthogonality(&qm) > &per + &tiny {
                return found(
                    "the symmetric eigensolver is backward stable within 8·(2n)²·u",
                    || format!("{sym:?}: residual {res}"),
                );
            }
        }
    }
    Ok(())
}

/// The laws on the case (module documentation).
///
/// # Errors
///
/// A [`Finding`] for the first law broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    let Ok(a) = Matrix::from_rows(case.rows, case.cols, &case.entries) else {
        return Ok(());
    };
    dense(&a, case)?;
    spectral(&a, case)
}

/// [`decode`], then [`check`].
///
/// # Errors
///
/// As [`check`].
pub fn harness(bytes: &[u8]) -> Result<(), Finding> {
    check(&decode(bytes).0)
}
