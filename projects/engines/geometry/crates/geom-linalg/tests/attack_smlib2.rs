//! Attack tests from SMLib's confirmed defects, batch 2: `geom-linalg`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: the exact answer, or a defined refusal, and never a
//! panic, a NaN, or an answer that depends on what ran before.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::manual_midpoint,
    clippy::type_complexity,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::range_minus_one,
    reason = "tests: native arithmetic, casts and indexing, platform maths on an independent oracle (the formula as it is written), prose that names defect files, and a failure stops the test"
)]

use geom_linalg::{
    LinalgError, Lu, Matrix, SymmetricEigen, Work, solve_lower, solve_tridiagonal, solve_upper,
};

fn work() -> Work {
    Work::new(1 << 40)
}

/// A column of an eigenvector matrix.
fn column(m: &Matrix, k: usize) -> Vec<f64> {
    (0..m.rows()).map(|i| m.get(i, k).unwrap()).collect()
}

/// solidcore plan/upstream-issues/UB-21--smmatrix-compute3x3eigenvectors-returns-uninitialised.md
/// (b4de2149): the eigenvectors of a non-symmetric 3 x 3 matrix were returned from uninitialised
/// storage. The input is the defect's: a non-symmetric 3 x 3. Ours refuses a matrix that is not
/// exactly symmetric (`NotSymmetric`), a non-square one (`Shape`) and a NaN entry (`NotFinite`),
/// and for a symmetric one returns unit eigenvectors that satisfy A v = lambda v.
#[test]
fn ub_21_a_non_symmetric_3x3_is_refused_and_a_symmetric_one_gets_real_eigenvectors() {
    let nonsym = Matrix::from_rows(3, 3, &[2.0, 1.0, 0.0, 0.0, 3.0, 1.0, 1.0, 0.0, 4.0]).unwrap();
    assert_eq!(
        SymmetricEigen::factor(&nonsym, &mut work()).map(|_| ()),
        Err(LinalgError::NotSymmetric)
    );
    // One unit in the last place of asymmetry is still asymmetry.
    let tiny = Matrix::from_rows(
        3,
        3,
        &[2.0, 1.0, 0.0, 1.0 + f64::EPSILON, 3.0, 1.0, 0.0, 1.0, 4.0],
    )
    .unwrap();
    assert_eq!(
        SymmetricEigen::factor(&tiny, &mut work()).map(|_| ()),
        Err(LinalgError::NotSymmetric)
    );
    let wide = Matrix::zeros(3, 2).unwrap();
    assert_eq!(
        SymmetricEigen::factor(&wide, &mut work()).map(|_| ()),
        Err(LinalgError::Shape)
    );
    assert_eq!(
        Matrix::from_rows(3, 3, &[1.0, 0.0, 0.0, 0.0, f64::NAN, 0.0, 0.0, 0.0, 1.0]),
        Err(LinalgError::NotFinite)
    );
    // A symmetric one: the second difference matrix, eigenvalues 2 - sqrt 2, 2, 2 + sqrt 2.
    let a = Matrix::from_rows(3, 3, &[2.0, -1.0, 0.0, -1.0, 2.0, -1.0, 0.0, -1.0, 2.0]).unwrap();
    let e = SymmetricEigen::factor(&a, &mut work()).unwrap();
    let want = [2.0 - 2f64.sqrt(), 2.0, 2.0 + 2f64.sqrt()];
    for (got, want) in e.values().iter().zip(want) {
        assert!((got - want).abs() < 1e-12, "{got} against {want}");
    }
    for k in 0..3 {
        let v = column(e.vectors(), k);
        let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!((norm - 1.0).abs() < 1e-12, "column {k} has norm {norm}");
        let av = a.mul_vec(&v).unwrap();
        for i in 0..3 {
            assert!((av[i] - e.values()[k] * v[i]).abs() < 1e-12, "column {k}, row {i}");
        }
        for j in 0..k {
            let w = column(e.vectors(), j);
            let dot: f64 = v.iter().zip(&w).map(|(x, y)| x * y).sum();
            assert!(dot.abs() < 1e-12, "columns {j} and {k}: {dot}");
        }
    }
}

/// solidcore plan/upstream-issues/WR-10--smgu-bestfitlinetopoints-takes-a-fixed-eigenvector-not-the.md
/// (b4de2149): the best-fit line to points took a fixed eigenvector of their scatter matrix, not
/// the principal one, so axis-aligned collinear points gave a direction perpendicular to them
/// while points along (1, 1, 1) gave the right one. The inputs are the defect's: collinear points
/// along each axis and along (1, 1, 1), (1, -1, 0), (0, 2, -1). The eigenvector of the scatter
/// matrix for the largest eigenvalue is the line's direction, and the other two eigenvalues are zero.
#[test]
fn wr_10_the_principal_direction_of_collinear_points_is_their_line() {
    let dirs: [[f64; 3]; 6] = [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [1.0, -1.0, 0.0],
        [0.0, 2.0, -1.0],
    ];
    for d in dirs {
        let pts: Vec<[f64; 3]> = (-3..=4)
            .map(|k| {
                let t = f64::from(k);
                [10.0 + t * d[0], -5.0 + t * d[1], 2.0 + t * d[2]]
            })
            .collect();
        let n = pts.len() as f64;
        let mean = [0, 1, 2].map(|i| pts.iter().map(|p| p[i]).sum::<f64>() / n);
        let mut cov = vec![0.0; 9];
        for p in &pts {
            for i in 0..3 {
                for j in 0..3 {
                    cov[i * 3 + j] += (p[i] - mean[i]) * (p[j] - mean[j]);
                }
            }
        }
        // The sums are symmetric by construction: the same products in the same order.
        let m = Matrix::from_rows(3, 3, &cov).unwrap();
        let e = SymmetricEigen::factor(&m, &mut work()).unwrap();
        let [l0, l1, l2] = [e.values()[0], e.values()[1], e.values()[2]];
        assert!(l0.abs() < 1e-9 && l1.abs() < 1e-9, "{d:?}: {l0} {l1}");
        assert!(l2 > 1.0, "{d:?}: {l2}");
        let v = column(e.vectors(), 2);
        let len = d.iter().map(|x| x * x).sum::<f64>().sqrt();
        let dot: f64 = v.iter().zip(&d).map(|(x, y)| x * y / len).sum();
        assert!((dot.abs() - 1.0).abs() < 1e-12, "{d:?}: principal vector {v:?}, dot {dot}");
    }
}

/// solidcore plan/upstream-issues/UB-22--smmatrix-ludecompose-uses-a-fixed-stack-buffer-it-does-not.md
/// (b4de2149): `LUDecompose` kept a fixed stack buffer it did not check, and ran off it on a
/// 1001 x 1001 matrix. The input is the defect's, at sizes either side of the common fixed sizes (a
/// power of two, one more, 257): ours solves a diagonally dominant system to its right-hand side,
/// and a system with two equal rows is `Singular`, at every size.
#[test]
fn ub_22_lu_of_matrices_beyond_a_fixed_buffer_solves_or_refuses() {
    for n in [1usize, 2, 8, 127, 128, 129, 130, 255, 256, 257] {
        let mut entries = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                entries[i * n + j] = if i == j {
                    n as f64 + 1.0
                } else {
                    ((i * 7 + j * 13) % 11) as f64 / 11.0 - 0.5
                };
            }
        }
        let a = Matrix::from_rows(n, n, &entries).unwrap();
        let x0: Vec<f64> = (0..n).map(|i| (i % 5) as f64 - 2.0).collect();
        let b = a.mul_vec(&x0).unwrap();
        let lu = Lu::factor(&a, &mut work()).unwrap();
        assert_eq!(lu.permutation().len(), n);
        let x = lu.solve(&b, &mut work()).unwrap();
        assert_eq!(x.len(), n);
        for i in 0..n {
            assert!((x[i] - x0[i]).abs() < 1e-9, "n = {n}, entry {i}: {} against {}", x[i], x0[i]);
        }
        // The right-hand side of the wrong length is refused, not read past.
        assert_eq!(lu.solve(&b[..n - 1], &mut work()), Err(LinalgError::Shape));
        let mut longer = b.clone();
        longer.push(0.0);
        assert_eq!(lu.solve(&longer, &mut work()), Err(LinalgError::Shape));
        // Two equal rows: singular, at every size above 1.
        if n >= 2 {
            let mut s = entries.clone();
            let (first, rest) = s.split_at_mut(n);
            rest[..n].copy_from_slice(first);
            let sing = Matrix::from_rows(n, n, &s).unwrap();
            assert_eq!(
                Lu::factor(&sing, &mut work()).map(|_| ()),
                Err(LinalgError::Singular),
                "n = {n}"
            );
        }
    }
}

/// solidcore plan/upstream-issues/UB-17--banded-and-triangular-matrix-routines-index-outside-rows.md
/// (b4de2149): the banded and triangular routines indexed outside their rows for a 2 x 3 triangular
/// operand, a band wider than the matrix, and similar shapes. The inputs are the defects': a 2 x 3
/// and a 3 x 2 triangular operand, right-hand sides of the wrong length, tridiagonal diagonals of
/// the wrong length, a zero diagonal. Ours refuses each with `Shape` or `Singular`; a real
/// triangular or tridiagonal system is solved exactly.
#[test]
fn ub_17_triangular_and_tridiagonal_solves_refuse_the_wrong_shape() {
    let two_by_three = Matrix::from_rows(2, 3, &[1.0, 0.0, 0.0, 1.0, 1.0, 0.0]).unwrap();
    let three_by_two = Matrix::from_rows(3, 2, &[1.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    for m in [&two_by_three, &three_by_two] {
        for b in [vec![1.0, 2.0], vec![1.0, 2.0, 3.0]] {
            assert_eq!(solve_lower(m, &b, false, &mut work()), Err(LinalgError::Shape));
            assert_eq!(solve_lower(m, &b, true, &mut work()), Err(LinalgError::Shape));
            assert_eq!(solve_upper(m, &b, &mut work()), Err(LinalgError::Shape));
        }
    }
    let l = Matrix::from_rows(3, 3, &[2.0, 0.0, 0.0, 1.0, 4.0, 0.0, -1.0, 2.0, 8.0]).unwrap();
    assert_eq!(solve_lower(&l, &[1.0, 2.0], false, &mut work()), Err(LinalgError::Shape));
    assert_eq!(solve_upper(&l, &[1.0; 4], &mut work()), Err(LinalgError::Shape));
    // 2 x + 0 = 2; x + 4 y = 5; -x + 2 y + 8 z = 6 gives x = 1, y = 1, z = 5/8 (exact in binary).
    let x = solve_lower(&l, &[2.0, 5.0, 6.0], false, &mut work()).unwrap();
    assert_eq!(x, vec![1.0, 1.0, 0.625]);
    // Upper: the transpose system, solved from the bottom.
    let u = Matrix::from_rows(3, 3, &[8.0, 2.0, -1.0, 0.0, 4.0, 1.0, 0.0, 0.0, 2.0]).unwrap();
    let x = solve_upper(&u, &[6.0, 5.0, 2.0], &mut work()).unwrap();
    assert_eq!(x, vec![0.625, 1.0, 1.0]);
    // A zero on the diagonal is singular, not a division.
    let zero = Matrix::from_rows(2, 2, &[1.0, 0.0, 1.0, 0.0]).unwrap();
    assert_eq!(solve_lower(&zero, &[1.0, 1.0], false, &mut work()), Err(LinalgError::Singular));
    assert_eq!(solve_upper(&zero, &[1.0, 1.0], &mut work()), Err(LinalgError::Singular));
    // The unit diagonal does not need the stored one.
    let unit = solve_lower(&zero, &[1.0, 3.0], true, &mut work()).unwrap();
    assert_eq!(unit, vec![1.0, 2.0]);
    // Tridiagonal: lengths must be n, n - 1, n - 1 and n.
    let d = [4.0, 4.0, 4.0];
    assert_eq!(
        solve_tridiagonal(&[1.0], &d, &[1.0, 1.0], &[1.0, 2.0, 3.0], &mut work()),
        Err(LinalgError::Shape)
    );
    assert_eq!(
        solve_tridiagonal(&[1.0, 1.0], &d, &[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0], &mut work()),
        Err(LinalgError::Shape)
    );
    assert_eq!(
        solve_tridiagonal(&[1.0, 1.0], &d, &[1.0, 1.0], &[1.0, 2.0], &mut work()),
        Err(LinalgError::Shape)
    );
    // The system itself: rows (4, 1, 0), (1, 4, 1), (0, 1, 4) against (5, 6, 5) has x = (1, 1, 1).
    let x = solve_tridiagonal(&[1.0, 1.0], &d, &[1.0, 1.0], &[5.0, 6.0, 5.0], &mut work()).unwrap();
    for v in x {
        assert!((v - 1.0).abs() < 1e-14, "{v}");
    }
    // A zero pivot with nothing to swap in: singular.
    assert_eq!(
        solve_tridiagonal(&[0.0], &[0.0, 0.0], &[0.0], &[1.0, 1.0], &mut work()),
        Err(LinalgError::Singular)
    );
}

/// solidcore plan/upstream-issues/UB-222--size-constructors-and-setters-write-through-a-failed-smos.md
/// (b4de2149): the size constructors (`SmMatrix(2^31, 5)` and the like) wrote through the failed
/// allocation of an oversized request. The input is the defect's kind: sizes whose entry count or
/// byte count overflows. Ours returns a typed refusal for each, never a panic, an abort or a
/// matrix that claims entries it does not hold.
#[test]
fn ub_222_a_matrix_whose_size_overflows_is_refused() {
    for (rows, cols) in [
        (usize::MAX, 2),
        (2, usize::MAX),
        (1usize << 62, 2),
        (1usize << 32, 1usize << 32),
        (usize::MAX, usize::MAX),
        (usize::MAX, 1),
    ] {
        assert_eq!(
            Matrix::zeros(rows, cols).map(|_| ()),
            Err(LinalgError::OutOfMemory),
            "{rows} x {cols}"
        );
        assert!(Matrix::from_rows(rows, cols, &[1.0, 2.0]).is_err());
    }
    // The identity of an overflowing size too.
    assert_eq!(Matrix::identity(usize::MAX).map(|_| ()), Err(LinalgError::OutOfMemory));
    assert_eq!(Matrix::identity(1usize << 40).map(|_| ()), Err(LinalgError::OutOfMemory));
    // A sane size still works and is all zero.
    let m = Matrix::zeros(3, 4).unwrap();
    assert_eq!((m.rows(), m.cols()), (3, 4));
}
