//! Tests of `geom_linalg`'s dense decompositions (piece G49) against an exact rational oracle
//! (dashu).
//!
//! What this file proves: LU, Cholesky, triangular and tridiagonal solves leave residuals within a
//! small multiple of the unit roundoff, checked exactly, on random integer, graded (rows and
//! columns scaled by powers of ten) and pivoting-forcing matrices; singular, indefinite and
//! asymmetric inputs are refused; the complete orthogonal decomposition finds the exact rank of
//! rank-deficient products B·C and its least-squares solution is the exact least-norm one; its
//! null space is orthonormal and annihilated by A; the canonical null space equals the exact
//! reduced echelon basis, and is the same for a matrix and for its rows reordered and rescaled;
//! Kahan's matrix shows the documented limit of column pivoting; and every factorisation charges
//! its work before it starts, refusing a budget one unit short.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::needless_range_loop,
    clippy::disallowed_methods,
    reason = "oracles and generators, indexed as the formulas are; the rules, chapter 21's bans               included, are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use dashu_ratio::RBig;
use geom_linalg::{
    Cholesky, Cod, LinalgError, Lu, Matrix, PivotedQr, Work, canonical_null_space, solve_lower,
    solve_tridiagonal, solve_upper,
};
use geom_num::real::Real;

const U: f64 = 1.0 / 9_007_199_254_740_992.0;

fn plenty() -> Work {
    Work::new(u64::MAX)
}

/// A fixed linear congruential sequence (Knuth's MMIX constants), for reproducible test data.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    /// A whole number in [−k, k].
    fn int(&mut self, k: i64) -> i64 {
        (self.next() % (2 * k as u64 + 1)) as i64 - k
    }
}

fn exact(x: f64) -> RBig {
    RBig::try_from(x).unwrap()
}

fn abs(x: &RBig) -> RBig {
    if *x < RBig::ZERO {
        -x.clone()
    } else {
        x.clone()
    }
}

fn to_f64(x: &RBig) -> f64 {
    x.to_f64().value()
}

/// ‖b − A·x‖∞ in exact rationals, and ‖A‖∞·‖x‖∞ + ‖b‖∞.
fn residual(a: &Matrix, x: &[f64], b: &[f64]) -> (f64, f64) {
    let (mut worst, mut scale_a) = (RBig::ZERO, RBig::ZERO);
    for i in 0..a.rows() {
        let mut r = exact(b[i]);
        let mut row_sum = RBig::ZERO;
        for j in 0..a.cols() {
            r -= exact(a.get(i, j).unwrap()) * exact(x[j]);
            row_sum += abs(&exact(a.get(i, j).unwrap()));
        }
        if abs(&r) > worst {
            worst = abs(&r);
        }
        if row_sum > scale_a {
            scale_a = row_sum;
        }
    }
    let xn = x
        .iter()
        .fold(0.0f64, |m, v| if v.abs() > m { v.abs() } else { m });
    let bn = b
        .iter()
        .fold(0.0f64, |m, v| if v.abs() > m { v.abs() } else { m });
    (to_f64(&worst), to_f64(&scale_a) * xn + bn)
}

fn random(rng: &mut Lcg, m: usize, n: usize, k: i64) -> Matrix {
    let entries: Vec<f64> = (0..m * n).map(|_| rng.int(k) as f64).collect();
    Matrix::from_rows(m, n, &entries).unwrap()
}

/// Rows scaled by 10^(i mod 7 − 3) and columns by 10^(3 − j mod 7): a graded matrix.
fn graded(a: &Matrix) -> Matrix {
    let mut g = a.clone();
    for i in 0..a.rows() {
        for j in 0..a.cols() {
            let r = 10f64.powi((i % 7) as i32 - 3);
            let c = 10f64.powi(3 - (j % 7) as i32);
            g.set(i, j, a.get(i, j).unwrap() * r * c).unwrap();
        }
    }
    g
}

#[test]
fn lu_solves_random_and_graded_systems_to_backward_accuracy() {
    let mut rng = Lcg(1);
    for n in 1..=24 {
        for kind in 0..2 {
            let mut a = random(&mut rng, n, n, 9);
            // Make it non-singular: a dominant diagonal on half the cases, none on the rest.
            if kind == 0 {
                for i in 0..n {
                    a.set(i, i, a.get(i, i).unwrap() + 40.0).unwrap();
                }
            }
            for a in [a.clone(), graded(&a)] {
                let b: Vec<f64> = (0..n).map(|_| rng.int(100) as f64).collect();
                match Lu::factor(&a, &mut plenty()) {
                    Ok(lu) => {
                        let x = lu.solve(&b, &mut plenty()).unwrap();
                        let (r, s) = residual(&a, &x, &b);
                        assert!(
                            r <= 16.0 * n as f64 * U * s,
                            "n = {n}: residual {r} against {s}"
                        );
                    }
                    Err(LinalgError::Singular) => {}
                    Err(e) => panic!("n = {n}: {e}"),
                }
            }
        }
    }
}

#[test]
fn lu_pivots_and_refuses_singular_matrices() {
    // A zero in the first pivot position forces an interchange.
    let a = Matrix::from_rows(2, 2, &[0.0, 1.0, 1.0, 1.0]).unwrap();
    let lu = Lu::factor(&a, &mut plenty()).unwrap();
    assert_eq!(lu.permutation(), &[1, 0]);
    assert_eq!(
        lu.solve(&[1.0, 3.0], &mut plenty()).unwrap(),
        vec![2.0, 1.0]
    );
    let singular = Matrix::from_rows(3, 3, &[1.0, 2.0, 3.0, 2.0, 4.0, 6.0, 0.0, 1.0, 1.0]).unwrap();
    assert_eq!(
        Lu::factor(&singular, &mut plenty()),
        Err(LinalgError::Singular)
    );
    assert_eq!(
        Lu::factor(&Matrix::zeros(2, 3).unwrap(), &mut plenty()),
        Err(LinalgError::Shape)
    );
}

#[test]
fn cholesky_solves_spd_systems_and_refuses_others() {
    let mut rng = Lcg(2);
    for n in 1..=20 {
        let b = random(&mut rng, n, n, 5);
        // A = BᵀB + n·I, exactly representable integers.
        let mut a = Matrix::zeros(n, n).unwrap();
        for i in 0..n {
            for j in 0..n {
                let mut s = if i == j { n as f64 } else { 0.0 };
                for k in 0..n {
                    s += b.get(k, i).unwrap() * b.get(k, j).unwrap();
                }
                a.set(i, j, s).unwrap();
            }
        }
        let rhs: Vec<f64> = (0..n).map(|_| rng.int(50) as f64).collect();
        let c = Cholesky::factor(&a, &mut plenty()).unwrap();
        let x = c.solve(&rhs, &mut plenty()).unwrap();
        let (r, s) = residual(&a, &x, &rhs);
        assert!(r <= 16.0 * n as f64 * U * s, "n = {n}: {r} against {s}");
    }
    let indefinite = Matrix::from_rows(2, 2, &[1.0, 2.0, 2.0, 1.0]).unwrap();
    assert_eq!(
        Cholesky::factor(&indefinite, &mut plenty()),
        Err(LinalgError::NotPositiveDefinite)
    );
    let asymmetric = Matrix::from_rows(2, 2, &[2.0, 1.0, 0.0, 2.0]).unwrap();
    assert_eq!(
        Cholesky::factor(&asymmetric, &mut plenty()),
        Err(LinalgError::NotPositiveDefinite)
    );
}

#[test]
fn triangular_solves_match_exact_substitution() {
    let mut rng = Lcg(3);
    for n in 1..=16 {
        let mut l = random(&mut rng, n, n, 9);
        for i in 0..n {
            l.set(i, i, (rng.int(9).abs() + 1) as f64).unwrap();
        }
        let b: Vec<f64> = (0..n).map(|_| rng.int(100) as f64).collect();
        let x = solve_lower(&l, &b, false, &mut plenty()).unwrap();
        // Residual against the lower triangle only.
        let mut low = Matrix::zeros(n, n).unwrap();
        let mut up = Matrix::zeros(n, n).unwrap();
        for i in 0..n {
            for j in 0..n {
                if j <= i {
                    low.set(i, j, l.get(i, j).unwrap()).unwrap();
                }
                if j >= i {
                    up.set(i, j, l.get(i, j).unwrap()).unwrap();
                }
            }
        }
        let (r, s) = residual(&low, &x, &b);
        assert!(r <= 4.0 * n as f64 * U * s, "lower n = {n}");
        let x = solve_upper(&l, &b, &mut plenty()).unwrap();
        let (r, s) = residual(&up, &x, &b);
        assert!(r <= 4.0 * n as f64 * U * s, "upper n = {n}");
    }
    let z = Matrix::from_rows(2, 2, &[1.0, 0.0, 1.0, 0.0]).unwrap();
    assert_eq!(
        solve_lower(&z, &[1.0, 1.0], false, &mut plenty()),
        Err(LinalgError::Singular)
    );
    assert!(solve_lower(&z, &[1.0, 1.0], true, &mut plenty()).is_ok());
}

#[test]
fn tridiagonal_solves_with_and_without_interchanges() {
    let mut rng = Lcg(4);
    for n in 1..=30 {
        for zero_diagonal in [false, true] {
            let sub: Vec<f64> = (1..n).map(|_| rng.int(9) as f64).collect();
            let sup: Vec<f64> = (1..n).map(|_| rng.int(9) as f64).collect();
            // A zero diagonal forces every interchange the method can make.
            let diag: Vec<f64> = (0..n)
                .map(|_| {
                    if zero_diagonal {
                        0.0
                    } else {
                        (rng.int(9) + 20) as f64
                    }
                })
                .collect();
            let b: Vec<f64> = (0..n).map(|_| rng.int(100) as f64).collect();
            let mut t = Matrix::zeros(n, n).unwrap();
            for i in 0..n {
                t.set(i, i, diag[i]).unwrap();
                if i + 1 < n {
                    t.set(i + 1, i, sub[i]).unwrap();
                    t.set(i, i + 1, sup[i]).unwrap();
                }
            }
            match solve_tridiagonal(&sub, &diag, &sup, &b, &mut plenty()) {
                Ok(x) => {
                    let (r, s) = residual(&t, &x, &b);
                    assert!(r <= 16.0 * n as f64 * U * s, "n = {n}: {r} against {s}");
                }
                Err(LinalgError::Singular) => {
                    // Only a truly singular matrix: LU agrees.
                    assert!(Lu::factor(&t, &mut plenty()).is_err(), "n = {n}");
                }
                Err(e) => panic!("{e}"),
            }
        }
    }
}

/// An exact rational matrix as rows.
type Exact = Vec<Vec<RBig>>;

fn exact_matrix(a: &Matrix) -> Exact {
    (0..a.rows())
        .map(|i| (0..a.cols()).map(|j| exact(a.get(i, j).unwrap())).collect())
        .collect()
}

fn mul(a: &Exact, b: &Exact) -> Exact {
    let (m, k, n) = (a.len(), b.len(), b[0].len());
    (0..m)
        .map(|i| {
            (0..n)
                .map(|j| (0..k).fold(RBig::ZERO, |acc, t| acc + &a[i][t] * &b[t][j]))
                .collect()
        })
        .collect()
}

fn transpose(a: &Exact) -> Exact {
    (0..a[0].len())
        .map(|j| a.iter().map(|r| r[j].clone()).collect())
        .collect()
}

/// The inverse of a square non-singular rational matrix, by Gauss–Jordan elimination.
fn inverse(a: &Exact) -> Exact {
    let n = a.len();
    let mut m: Exact = a
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut row = r.clone();
            row.extend((0..n).map(|j| if i == j { RBig::ONE } else { RBig::ZERO }));
            row
        })
        .collect();
    for c in 0..n {
        let p = (c..n).find(|&r| m[r][c] != RBig::ZERO).unwrap();
        m.swap(c, p);
        let piv = m[c][c].clone();
        for x in &mut m[c] {
            *x = &*x / &piv;
        }
        for r in 0..n {
            if r != c && m[r][c] != RBig::ZERO {
                let f = m[r][c].clone();
                let pr = m[c].clone();
                for (x, y) in m[r].iter_mut().zip(&pr) {
                    *x = &*x - &f * y;
                }
            }
        }
    }
    m.into_iter().map(|r| r[n..].to_vec()).collect()
}

/// The reduced row echelon form of a rational matrix (rows), and its pivot columns.
fn rref(a: &Exact) -> (Exact, Vec<usize>) {
    let mut m = a.clone();
    let (rows, cols) = (m.len(), m.first().map_or(0, Vec::len));
    let mut pivots = Vec::new();
    let mut r = 0;
    for c in 0..cols {
        if r == rows {
            break;
        }
        let Some(p) = (r..rows).find(|&i| m[i][c] != RBig::ZERO) else {
            continue;
        };
        m.swap(r, p);
        let piv = m[r][c].clone();
        for x in &mut m[r] {
            *x = &*x / &piv;
        }
        for i in 0..rows {
            if i != r && m[i][c] != RBig::ZERO {
                let f = m[i][c].clone();
                let pr = m[r].clone();
                for (x, y) in m[i].iter_mut().zip(&pr) {
                    *x = &*x - &f * y;
                }
            }
        }
        pivots.push(c);
        r += 1;
    }
    m.truncate(r);
    (m, pivots)
}

/// The exact canonical null space of A: the null-space basis from A's reduced echelon form, put
/// in reduced echelon form itself (the definition the floating version follows).
fn exact_canonical_null_space(a: &Exact) -> Exact {
    let n = a[0].len();
    let (r, pivots) = rref(a);
    let free: Vec<usize> = (0..n).filter(|c| !pivots.contains(c)).collect();
    let basis: Exact = free
        .iter()
        .map(|&f| {
            let mut v = vec![RBig::ZERO; n];
            v[f] = RBig::ONE;
            for (row, &p) in r.iter().zip(&pivots) {
                v[p] = -row[f].clone();
            }
            v
        })
        .collect();
    rref(&basis).0
}

/// A rank-r product B·C, B m × r and C r × n, small integers.
fn deficient(rng: &mut Lcg, m: usize, n: usize, r: usize) -> Matrix {
    let b = random(rng, m, r, 4);
    let c = random(rng, r, n, 4);
    let p = mul(&exact_matrix(&b), &exact_matrix(&c));
    let entries: Vec<f64> = p.iter().flatten().map(to_f64).collect();
    Matrix::from_rows(m, n, &entries).unwrap()
}

#[test]
fn cod_finds_the_rank_and_the_least_norm_solution() {
    let mut rng = Lcg(5);
    let tol = Real::new(1e-10).unwrap();
    for (m, n, r) in [
        (6, 6, 3),
        (8, 5, 2),
        (5, 9, 4),
        (10, 10, 9),
        (7, 4, 4),
        (4, 7, 4),
        (3, 3, 1),
    ] {
        let a = deficient(&mut rng, m, n, r);
        let ex = exact_matrix(&a);
        let rank_exact = rref(&ex).1.len();
        let cod = Cod::factor(&a, tol, &mut plenty()).unwrap();
        assert_eq!(cod.rank(), rank_exact, "{m} × {n}");
        let b: Vec<f64> = (0..m).map(|_| rng.int(20) as f64).collect();
        let x = cod.least_squares(&b, &mut plenty()).unwrap();
        // The exact least-norm least-squares solution: x* = A⁺·b, from A = F·G with F and G of
        // full rank (F the pivot columns of A, G the non-zero rows of its reduced echelon form):
        // A⁺ = Gᵀ(G·Gᵀ)⁻¹(FᵀF)⁻¹Fᵀ.
        let (g, pivots) = rref(&ex);
        let f: Exact = ex
            .iter()
            .map(|row| pivots.iter().map(|&p| row[p].clone()).collect())
            .collect();
        let gt = transpose(&g);
        let ft = transpose(&f);
        let pinv = mul(
            &mul(&gt, &inverse(&mul(&g, &gt))),
            &mul(&inverse(&mul(&ft, &f)), &ft),
        );
        let bx: Exact = b.iter().map(|v| vec![exact(*v)]).collect();
        let want = mul(&pinv, &bx);
        let scale = want
            .iter()
            .fold(0.0f64, |acc, v| acc.max(to_f64(&abs(&v[0]))))
            .max(1.0);
        for (xi, wi) in x.iter().zip(&want) {
            let d = (xi - to_f64(&wi[0])).abs();
            assert!(
                d <= 1e-10 * scale,
                "{m} × {n}: {xi} against {}",
                to_f64(&wi[0])
            );
        }
        // The null space: orthonormal, annihilated by A.
        let z = cod.null_space(&mut plenty()).unwrap();
        assert_eq!(z.cols(), n - rank_exact);
        for c in 0..z.cols() {
            let col: Vec<f64> = (0..n).map(|i| z.get(i, c).unwrap()).collect();
            let az = a.mul_vec(&col).unwrap();
            assert!(
                az.iter().all(|v| v.abs() <= 1e-10 * scale * 100.0),
                "{m} × {n}"
            );
            for c2 in 0..z.cols() {
                let dot: f64 = (0..n)
                    .map(|i| z.get(i, c).unwrap() * z.get(i, c2).unwrap())
                    .sum();
                let want = if c == c2 { 1.0 } else { 0.0 };
                assert!((dot - want).abs() <= 1e-12, "{m} × {n}: Zᵀ·Z");
            }
        }
    }
}

#[test]
fn the_canonical_null_space_is_the_exact_one_and_independent_of_row_order() {
    let mut rng = Lcg(6);
    let tol = Real::new(1e-10).unwrap();
    for (m, n, r) in [(4, 7, 3), (3, 6, 2), (5, 5, 3), (6, 9, 5), (2, 5, 1)] {
        let a = deficient(&mut rng, m, n, r);
        let canon = canonical_null_space(&a, tol, &mut plenty()).unwrap();
        let want = exact_canonical_null_space(&exact_matrix(&a));
        assert_eq!(canon.rows(), want.len(), "{m} × {n}");
        for (i, row) in want.iter().enumerate() {
            for (j, w) in row.iter().enumerate() {
                let got = canon.get(i, j).unwrap();
                assert!(
                    (got - to_f64(w)).abs() <= 1e-9,
                    "{m} × {n} [{i}, {j}]: {got} against {}",
                    to_f64(w)
                );
            }
        }
        // The same null space from the rows reversed and scaled: the same canonical basis.
        let mut b = Matrix::zeros(m, n).unwrap();
        for i in 0..m {
            for j in 0..n {
                b.set(m - 1 - i, j, a.get(i, j).unwrap() * (i as f64 + 2.0))
                    .unwrap();
            }
        }
        let again = canonical_null_space(&b, tol, &mut plenty()).unwrap();
        for i in 0..canon.rows() {
            for j in 0..n {
                assert!((again.get(i, j).unwrap() - canon.get(i, j).unwrap()).abs() <= 1e-9);
            }
        }
    }
}

/// Kahan's matrix K = diag(1, s, …, s^(n−1))·(I − c·N), N all ones above the diagonal, with
/// c = 7/25 and s = 24/25 (c² + s² = 1, so every column has norm 1), its column j scaled by
/// 1 − 10⁻¹⁰·j as Kahan's example is usually perturbed (Golub and Van Loan, section 5.4.3), so that
/// column pivoting never moves a column. Its last diagonal entry is about s^(n−1), yet K·x = `e_n`
/// has a solution of enormous norm: a singular value far below anything R's diagonal shows. The
/// rank decision of a column-pivoted QR cannot see it: the documented limit (module
/// documentation), shown here. (Unperturbed, the ties between equal column norms are broken by
/// rounding, pivoting happens, and R's diagonal falls to about 10⁻⁴.)
#[test]
fn kahan_matrix_shows_the_limit_of_column_pivoting() {
    let n = 40;
    let (c, s) = (7.0 / 25.0, 24.0 / 25.0);
    let mut k = Matrix::zeros(n, n).unwrap();
    let mut scale = 1.0;
    for i in 0..n {
        k.set(i, i, scale * (1.0 - 1e-10 * i as f64)).unwrap();
        for j in i + 1..n {
            k.set(i, j, -c * scale * (1.0 - 1e-10 * j as f64)).unwrap();
        }
        scale *= s;
    }
    let qr = PivotedQr::factor(&k, &mut plenty()).unwrap();
    assert_eq!(
        qr.permutation(),
        (0..n).collect::<Vec<_>>().as_slice(),
        "no column moved"
    );
    let last = (0..n)
        .filter_map(|i| qr.r_diagonal(i))
        .fold(f64::INFINITY, |m, d| m.min(d.abs()));
    assert!(last > 0.1, "R's smallest diagonal entry {last}");
    // At a relative tolerance of 10⁻³ the decision says full rank ...
    assert_eq!(qr.rank(Real::new(1e-3).unwrap()), n);
    // The exact solution of K·x = e_n (back substitution in rationals on K's exact entries).
    let ex = exact_matrix(&k);
    let mut x = vec![RBig::ZERO; n];
    for i in (0..n).rev() {
        let mut sum = if i == n - 1 { RBig::ONE } else { RBig::ZERO };
        for j in i + 1..n {
            sum -= &ex[i][j] * &x[j];
        }
        x[i] = sum / &ex[i][i];
    }
    let big = x.iter().fold(0.0f64, |m, v| m.max(to_f64(&abs(v))));
    // ... yet ‖K⁻¹‖₂ ≥ |x|∞, so the smallest singular value is at most 1/|x|∞, below 10⁻³ times
    // ‖K‖₂ ≥ 1: a deficiency at that tolerance that R's diagonal (at least 0.1) does not show.
    assert!(
        1.0 / big < 1e-3,
        "smallest singular value bound {} against R {last}",
        1.0 / big
    );
}

#[test]
fn every_factorisation_charges_its_work_first() {
    let a = Matrix::identity(5).unwrap();
    let cost = 125u64;
    assert_eq!(
        Lu::factor(&a, &mut Work::new(cost - 1)),
        Err(LinalgError::OutOfWork)
    );
    let mut w = Work::new(cost);
    assert!(Lu::factor(&a, &mut w).is_ok());
    assert_eq!(w.left(), 0);
    assert_eq!(
        Cholesky::factor(&a, &mut Work::new(cost - 1)),
        Err(LinalgError::OutOfWork)
    );
    assert_eq!(
        PivotedQr::factor(&a, &mut Work::new(2 * cost - 1)),
        Err(LinalgError::OutOfWork)
    );
    assert!(PivotedQr::factor(&a, &mut Work::new(2 * cost)).is_ok());
    assert_eq!(
        solve_tridiagonal(&[1.0], &[2.0, 2.0], &[1.0], &[1.0, 1.0], &mut Work::new(7)),
        Err(LinalgError::OutOfWork)
    );
}
