//! Attack tests from SMLib's confirmed defects, batch 1: `geom-linalg`.
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

use geom_linalg::{Cholesky, LinalgError, Lu, Matrix, Svd, Work, solve_tridiagonal};

fn work() -> Work {
    Work::new(1 << 40)
}

/// solidcore plan/upstream-issues/UB-204--smmatrix-rows-cols-leaves-its-inline-storage-uninitialised.md
/// (b4de2149): a system of up to 6 x 6 solved with a Jacobian its evaluator never filled used the
/// stale stack slot of an earlier solve, so the step depended on what ran before. Ours has no
/// unfilled matrix: a new matrix is exactly zero, a zero Jacobian is singular, and a solve after
/// an earlier solve of anything gives the same answer as one made first.
#[test]
fn ub_204_an_unfilled_jacobian_is_zero_and_singular_whatever_ran_before() {
    for n in 1..=8 {
        // A dirty earlier solve of a different, full system, in the same sizes.
        let mut full = Matrix::zeros(n, n).unwrap();
        for i in 0..n {
            for j in 0..n {
                full.set(
                    i,
                    j,
                    1.0 + (i * 31 + j * 17) as f64 + if i == j { 100.0 } else { 0.0 },
                )
                .unwrap();
            }
        }
        let before = Lu::factor(&full, &mut work()).unwrap();
        let rhs: Vec<f64> = (0..n).map(|i| i as f64 - 2.5).collect();
        let x = before.solve(&rhs, &mut work()).unwrap();
        assert!(x.iter().all(|v| v.is_finite()));
        // The fresh matrix after it: every entry is positive zero, bit for bit.
        let fresh = Matrix::zeros(n, n).unwrap();
        for i in 0..n {
            for j in 0..n {
                assert_eq!(
                    fresh.get(i, j).unwrap().to_bits(),
                    0u64,
                    "{n}x{n} at {i},{j}"
                );
            }
        }
        // Solving with it is refused, the same way every time.
        for _ in 0..3 {
            assert_eq!(
                Lu::factor(&fresh, &mut work()).map(|_| ()),
                Err(LinalgError::Singular)
            );
        }
        // And the earlier full system still solves to the same bits afterwards.
        let again = Lu::factor(&full, &mut work())
            .unwrap()
            .solve(&rhs, &mut work())
            .unwrap();
        assert_eq!(x, again);
    }
    // A Jacobian with a zero row, which is what a failed evaluator leaves: also refused.
    let mut j = Matrix::identity(4).unwrap();
    for c in 0..4 {
        j.set(2, c, 0.0).unwrap();
    }
    assert_eq!(
        Lu::factor(&j, &mut work()).map(|_| ()),
        Err(LinalgError::Singular)
    );
    // A NaN in it is refused as not finite, not solved.
    let mut j = Matrix::identity(3).unwrap();
    let bad = j.set(1, 1, f64::NAN);
    if bad.is_ok() {
        assert_eq!(
            Lu::factor(&j, &mut work()).map(|_| ()),
            Err(LinalgError::NotFinite)
        );
    } else {
        assert_eq!(bad, Err(LinalgError::NotFinite));
    }
}

/// solidcore plan/upstream-issues/UB-60--sm-createcorner-reads-ssolutions-0-of-an-empty-array.md
/// and UB-36--findcoincidentpairs-reads-getat-0-of-an-empty-bucket-list.md (b4de2149): the first
/// element of an empty array was read. Ours is asked for the empty system, the empty right-hand
/// side, and a system with one unknown and no off-diagonals: each must answer, or refuse, and
/// the answer for nothing is nothing.
#[test]
fn ub_60_and_ub_36_empty_systems_answer_or_refuse_and_never_read_a_first_element() {
    // The tridiagonal system with no unknowns, and with one.
    match solve_tridiagonal(&[], &[], &[], &[], &mut work()) {
        Ok(x) => assert!(x.is_empty()),
        Err(e) => assert_eq!(e, LinalgError::Shape),
    }
    assert_eq!(
        solve_tridiagonal(&[], &[4.0], &[], &[2.0], &mut work()).unwrap(),
        vec![0.5]
    );
    // Lengths that do not fit are a shape error, not a read past the end.
    assert_eq!(
        solve_tridiagonal(&[1.0], &[4.0], &[], &[2.0], &mut work()),
        Err(LinalgError::Shape)
    );
    assert_eq!(
        solve_tridiagonal(&[], &[], &[], &[1.0], &mut work()),
        Err(LinalgError::Shape)
    );
    // The empty and the one-by-one matrices through every factorisation.
    for n in [0usize, 1] {
        match Matrix::zeros(n, n) {
            Err(e) => assert_eq!(e, LinalgError::Shape),
            Ok(m) => {
                assert_eq!(m.rows(), n);
                match Lu::factor(&m, &mut work()) {
                    Ok(lu) => assert_eq!(lu.solve(&vec![0.0; n], &mut work()).unwrap().len(), n),
                    Err(e) => assert_eq!(e, LinalgError::Singular),
                }
                match Cholesky::factor(&m, &mut work()) {
                    Ok(c) => assert_eq!(c.solve(&vec![0.0; n], &mut work()).unwrap().len(), n),
                    Err(e) => assert_eq!(e, LinalgError::NotPositiveDefinite),
                }
                if let Ok(s) = Svd::jacobi(&m, &mut work()) {
                    assert!(s.singular_values().len() <= n);
                }
            }
        }
    }
    // The one-by-one matrix that is not empty solves exactly.
    let m = Matrix::from_rows(1, 1, &[8.0]).unwrap();
    assert_eq!(
        Lu::factor(&m, &mut work())
            .unwrap()
            .solve(&[4.0], &mut work())
            .unwrap(),
        vec![0.5]
    );
    // A right-hand side of the wrong length is refused.
    assert_eq!(
        Lu::factor(&m, &mut work()).unwrap().solve(&[], &mut work()),
        Err(LinalgError::Shape)
    );
}

/// solidcore plan/upstream-issues/UB-240--n-initrealmatrix-leaves-the-matrix-type-indeterminate.md
/// (b4de2149): a matrix made empty had an indeterminate type field, and the null test read it.
/// Ours has no field to leave unset: a matrix of any shape, including an empty one, has defined
/// contents and a defined refusal for an entry that is not there, however many matrices were made
/// and dirtied before it.
#[test]
fn ub_240_every_new_matrix_is_fully_defined() {
    // Dirty a run of allocations of every small shape first.
    for rows in 0..7 {
        for cols in 0..7 {
            if let Ok(mut m) = Matrix::zeros(rows, cols) {
                for i in 0..rows {
                    for j in 0..cols {
                        m.set(i, j, f64::MAX).unwrap();
                    }
                }
            }
        }
    }
    for rows in 0..7 {
        for cols in 0..7 {
            let Ok(m) = Matrix::zeros(rows, cols) else {
                continue;
            };
            assert_eq!((m.rows(), m.cols()), (rows, cols));
            for i in 0..rows {
                for j in 0..cols {
                    assert_eq!(m.get(i, j).unwrap().to_bits(), 0u64);
                }
                assert_eq!(m.row(i).unwrap().len(), cols);
            }
            // One past the last row or column is refused, never read.
            assert!(m.get(rows, 0).is_err());
            assert!(m.get(0, cols).is_err());
            assert!(m.row(rows).is_err());
        }
    }
    // The identity is exact, and a matrix built from the wrong number of entries is refused.
    let id = Matrix::identity(5).unwrap();
    for i in 0..5 {
        for j in 0..5 {
            assert_eq!(id.get(i, j).unwrap(), if i == j { 1.0 } else { 0.0 });
        }
    }
    assert_eq!(Matrix::from_rows(2, 3, &[1.0; 5]), Err(LinalgError::Shape));
    assert_eq!(Matrix::from_rows(2, 3, &[1.0; 7]), Err(LinalgError::Shape));
    assert_eq!(id.mul_vec(&[1.0; 4]), Err(LinalgError::Shape));
}
