//! Tests of the promises `geom_linalg`'s public interface makes beyond the accuracy that
//! `tests/dense.rs` checks (piece G49).
//!
//! What this file proves: every error reads as documented and a `geom_num` error maps to its own
//! kind; a work budget counts down and refuses without charging; a matrix refuses every index
//! outside it, rows included, and a matrix without columns still has its rows; `mul_vec` is the
//! product, row sums left to right; the pivoted QR takes the column of largest remaining norm, the
//! first on a tie, at any scale, with the trailing norms downdated after each step, so R's diagonal
//! never grows; the rank counts diagonal entries strictly above the tolerance times the first; the
//! complete orthogonal decomposition reports the pivoted QR's column order; LU takes the first of
//! equal pivots; the solves refuse each wrong length on its own; and the tridiagonal solve
//! interchanges rows with a non-zero multiplier exactly as elimination does.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::disallowed_methods,
    reason = "oracles and generators, indexed as the formulas are; the rules, chapter 21's bans included, are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_linalg::{
    Cod, LinalgError, Lu, Matrix, PivotedQr, Work, solve_lower, solve_tridiagonal, solve_upper,
};
use geom_num::NumError;
use geom_num::real::Real;

fn plenty() -> Work {
    Work::new(u64::MAX)
}

fn tol(v: f64) -> Real {
    Real::new(v).unwrap()
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

/// The matrix whose columns are `cols`, each `m` long.
fn by_columns(m: usize, cols: &[&[f64]]) -> Matrix {
    let mut a = Matrix::zeros(m, cols.len()).unwrap();
    for (j, c) in cols.iter().enumerate() {
        for i in 0..m {
            a.set(i, j, c[i]).unwrap();
        }
    }
    a
}

fn diagonal_magnitudes(qr: &PivotedQr) -> Vec<f64> {
    (0..)
        .map_while(|k| qr.r_diagonal(k))
        .map(f64::abs)
        .collect()
}

#[test]
fn errors_read_as_documented_and_number_errors_keep_their_kind() {
    let texts = [
        (LinalgError::Shape, "the shapes do not fit"),
        (LinalgError::NotFinite, "an entry or a result is not finite"),
        (LinalgError::Singular, "the matrix is singular"),
        (
            LinalgError::NotPositiveDefinite,
            "the matrix is not symmetric positive definite",
        ),
        (
            LinalgError::OutOfWork,
            "the work budget cannot cover the factorisation",
        ),
        (LinalgError::OutOfMemory, "an allocation could not be made"),
        (
            LinalgError::Defect,
            "a step its own bounds rule out failed (a defect)",
        ),
    ];
    for (e, text) in texts {
        assert_eq!(e.to_string(), text);
    }
    assert_eq!(
        LinalgError::from(NumError::NotFinite),
        LinalgError::NotFinite
    );
    // Any other number error is one the linear algebra's own bounds rule out.
    assert_eq!(LinalgError::from(NumError::Degenerate), LinalgError::Defect);
    assert_eq!(
        LinalgError::from(NumError::NegativeSqrt),
        LinalgError::Defect
    );
}

#[test]
fn work_counts_down_and_refuses_without_charging() {
    let mut w = Work::new(10);
    assert_eq!(w.left(), 10);
    w.charge(3).unwrap();
    assert_eq!(w.left(), 7);
    assert_eq!(w.charge(8), Err(LinalgError::OutOfWork));
    assert_eq!(w.left(), 7);
    w.charge(7).unwrap();
    assert_eq!(w.left(), 0);
    // A factorisation leaves exactly its stated cost charged: LU of order 3 costs 27.
    let a = Matrix::from_rows(3, 3, &[4.0, 1.0, 0.0, 1.0, 4.0, 1.0, 0.0, 1.0, 4.0]).unwrap();
    let mut w = Work::new(100);
    Lu::factor(&a, &mut w).unwrap();
    assert_eq!(w.left(), 73);
}

#[test]
fn a_matrix_refuses_every_index_outside_it() {
    let mut a = Matrix::from_rows(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
    assert_eq!(a.get(1, 2), Ok(6.0));
    // A column past the end would land in the next row's storage; it is refused instead.
    assert_eq!(a.get(0, 3), Err(LinalgError::Shape));
    assert_eq!(a.get(2, 0), Err(LinalgError::Shape));
    assert_eq!(a.set(0, 3, 9.0), Err(LinalgError::Shape));
    assert_eq!(a.set(2, 0, 9.0), Err(LinalgError::Shape));
    assert_eq!(a.set(0, 0, f64::NAN), Err(LinalgError::NotFinite));
    assert_eq!(a.get(1, 0), Ok(4.0));
    assert_eq!(a.row(1), Ok(&[4.0, 5.0, 6.0][..]));
    assert_eq!(a.row(2), Err(LinalgError::Shape));
    // A matrix without columns still has its rows, each empty, and no row beyond them.
    let empty = Matrix::zeros(2, 0).unwrap();
    assert_eq!(empty.row(0), Ok(&[][..]));
    assert_eq!(empty.row(1), Ok(&[][..]));
    assert_eq!(empty.row(2), Err(LinalgError::Shape));
    assert_eq!(empty.row(3), Err(LinalgError::Shape));
    assert_eq!(Matrix::zeros(0, 0).unwrap().row(0), Err(LinalgError::Shape));
}

#[test]
fn mul_vec_is_the_product_with_row_sums_left_to_right() {
    let a = Matrix::from_rows(3, 2, &[1.0, 2.0, 3.0, 4.0, -5.0, 0.5]).unwrap();
    assert_eq!(a.mul_vec(&[5.0, 6.0]), Ok(vec![17.0, 39.0, -22.0]));
    // Left to right: (1 + 2^53) − 2^53 is 0 in doubles, where right to left would give 1.
    let big = 9_007_199_254_740_992.0;
    let b = Matrix::from_rows(1, 3, &[1.0, big, -big]).unwrap();
    assert_eq!(b.mul_vec(&[1.0, 1.0, 1.0]), Ok(vec![0.0]));
    assert_eq!(a.mul_vec(&[1.0]), Err(LinalgError::Shape));
    let huge = Matrix::from_rows(1, 2, &[1e308, 1e308]).unwrap();
    assert_eq!(huge.mul_vec(&[10.0, 10.0]), Err(LinalgError::NotFinite));
}

#[test]
fn pivoted_qr_takes_the_largest_remaining_column_first_at_any_scale() {
    // Every norm below 1: the second column is still the larger.
    let a = by_columns(2, &[&[0.1, 0.0], &[0.0, 0.5]]);
    let qr = PivotedQr::factor(&a, &mut plenty()).unwrap();
    assert_eq!(qr.permutation(), &[1, 0]);
    assert_eq!(diagonal_magnitudes(&qr), vec![0.5, 0.1]);
    // Ties go to the first column.
    for n in 1..=4 {
        let qr = PivotedQr::factor(&Matrix::identity(n).unwrap(), &mut plenty()).unwrap();
        assert_eq!(qr.permutation(), (0..n).collect::<Vec<_>>());
        assert_eq!(qr.r_diagonal(n), None);
    }
    // Three columns of norm 5: the first leads; the reflection leaves 3 of the second and 4 of the
    // third below it, so the third comes next.
    let b = by_columns(2, &[&[3.0, 4.0], &[0.0, 5.0], &[5.0, 0.0]]);
    let qr = PivotedQr::factor(&b, &mut plenty()).unwrap();
    assert_eq!(qr.permutation(), &[0, 2, 1]);
    let d = diagonal_magnitudes(&qr);
    assert_eq!(d[0], 5.0);
    assert!((d[1] - 4.0).abs() <= 1e-14, "{d:?}");
}

#[test]
fn pivoted_qr_downdates_the_trailing_norms_after_each_step() {
    // Column 1 starts larger than column 2 (√82 against 5), but after the first step only its
    // trailing 1 is left, so column 2 comes second.
    let a = by_columns(3, &[&[10.0, 0.0, 0.0], &[9.0, 1.0, 0.0], &[0.0, 0.0, 5.0]]);
    let qr = PivotedQr::factor(&a, &mut plenty()).unwrap();
    assert_eq!(qr.permutation(), &[0, 2, 1]);
    assert_eq!(diagonal_magnitudes(&qr), vec![10.0, 5.0, 1.0]);
    // A smaller part removed (2 of √20): column 1 keeps 4, still above column 2's 3.
    let b = by_columns(3, &[&[10.0, 0.0, 0.0], &[2.0, 4.0, 0.0], &[0.0, 0.0, 3.0]]);
    let qr = PivotedQr::factor(&b, &mut plenty()).unwrap();
    assert_eq!(qr.permutation(), &[0, 1, 2]);
    let d = diagonal_magnitudes(&qr);
    assert_eq!(d[0], 10.0);
    assert!((d[1] - 4.0).abs() <= 1e-14, "{d:?}");
    assert!((d[2] - 3.0).abs() <= 1e-14, "{d:?}");
}

#[test]
fn pivoted_qr_never_lets_the_diagonal_grow() {
    let mut rng = Lcg(11);
    for m in 1..=9 {
        for n in 1..=9 {
            for scale in [1.0, 1e-3, 1e-150, 1e150] {
                let entries: Vec<f64> = (0..m * n).map(|_| rng.int(9) as f64 * scale).collect();
                let a = Matrix::from_rows(m, n, &entries).unwrap();
                let qr = PivotedQr::factor(&a, &mut plenty()).unwrap();
                let mut sorted = qr.permutation().to_vec();
                sorted.sort_unstable();
                assert_eq!(sorted, (0..n).collect::<Vec<_>>());
                let d = diagonal_magnitudes(&qr);
                assert_eq!(d.len(), m.min(n));
                for k in 1..d.len() {
                    assert!(
                        d[k] <= d[k - 1] * (1.0 + 1e-12),
                        "{m} x {n} at scale {scale}: {d:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn rank_counts_entries_strictly_above_the_tolerance_times_the_first() {
    let a = Matrix::from_rows(2, 2, &[4.0, 0.0, 0.0, 1.0]).unwrap();
    let qr = PivotedQr::factor(&a, &mut plenty()).unwrap();
    assert_eq!(diagonal_magnitudes(&qr), vec![4.0, 1.0]);
    // 1 is exactly 0.25 · 4: not above it.
    assert_eq!(qr.rank(tol(0.25)), 1);
    assert_eq!(qr.rank(tol(0.249)), 2);
    let b = Matrix::from_rows(2, 2, &[100.0, 0.0, 0.0, 0.5]).unwrap();
    let qr = PivotedQr::factor(&b, &mut plenty()).unwrap();
    assert_eq!(qr.rank(tol(0.01)), 1);
    assert_eq!(qr.rank(tol(0.004)), 2);
    let zero = Matrix::zeros(3, 2).unwrap();
    assert_eq!(
        PivotedQr::factor(&zero, &mut plenty())
            .unwrap()
            .rank(tol(0.0)),
        0
    );
}

#[test]
fn cod_reports_the_pivoted_qr_column_order() {
    let mut rng = Lcg(12);
    for (m, n) in [(3, 5), (5, 3), (4, 4)] {
        let entries: Vec<f64> = (0..m * n).map(|_| rng.int(9) as f64).collect();
        let a = Matrix::from_rows(m, n, &entries).unwrap();
        let qr = PivotedQr::factor(&a, &mut plenty()).unwrap();
        let cod = Cod::factor(&a, tol(1e-10), &mut plenty()).unwrap();
        assert_eq!(cod.permutation(), qr.permutation());
        assert_eq!(cod.permutation().len(), n);
    }
}

#[test]
fn lu_takes_the_first_of_equal_pivots() {
    let tie = Matrix::from_rows(2, 2, &[1.0, 2.0, -1.0, 3.0]).unwrap();
    assert_eq!(
        Lu::factor(&tie, &mut plenty()).unwrap().permutation(),
        &[0, 1]
    );
    let larger = Matrix::from_rows(2, 2, &[1.0, 2.0, -2.0, 3.0]).unwrap();
    assert_eq!(
        Lu::factor(&larger, &mut plenty()).unwrap().permutation(),
        &[1, 0]
    );
}

#[test]
fn solves_refuse_each_wrong_length_on_its_own() {
    let square = Matrix::identity(2).unwrap();
    let wide = Matrix::zeros(2, 3).unwrap();
    for unit in [false, true] {
        assert_eq!(
            solve_lower(&wide, &[1.0, 1.0], unit, &mut plenty()),
            Err(LinalgError::Shape)
        );
        assert_eq!(
            solve_lower(&square, &[1.0, 1.0, 1.0], unit, &mut plenty()),
            Err(LinalgError::Shape)
        );
    }
    assert_eq!(
        solve_upper(&wide, &[1.0, 1.0], &mut plenty()),
        Err(LinalgError::Shape)
    );
    assert_eq!(
        solve_upper(&square, &[1.0, 1.0, 1.0], &mut plenty()),
        Err(LinalgError::Shape)
    );
    let (sub, diag, sup, b) = ([1.0, 1.0], [4.0, 4.0, 4.0], [1.0, 1.0], [1.0, 1.0, 1.0]);
    assert!(solve_tridiagonal(&sub, &diag, &sup, &b, &mut plenty()).is_ok());
    assert_eq!(
        solve_tridiagonal(&sub[..1], &diag, &sup, &b, &mut plenty()),
        Err(LinalgError::Shape)
    );
    assert_eq!(
        solve_tridiagonal(&sub, &diag, &sup[..1], &b, &mut plenty()),
        Err(LinalgError::Shape)
    );
    assert_eq!(
        solve_tridiagonal(&sub, &diag, &sup, &b[..2], &mut plenty()),
        Err(LinalgError::Shape)
    );
}

#[test]
fn tridiagonal_interchanges_with_a_non_zero_multiplier() {
    // |1| < |3| and then |4 − 2/3| < |5|: two interchanges, each with a non-zero multiplier.
    let (sub, diag, sup) = ([3.0, 5.0], [1.0, 2.0, 3.0], [4.0, 6.0]);
    let x = [1.0, -2.0, 3.0];
    let b = [
        diag[0] * x[0] + sup[0] * x[1],
        sub[0] * x[0] + diag[1] * x[1] + sup[1] * x[2],
        sub[1] * x[1] + diag[2] * x[2],
    ];
    let got = solve_tridiagonal(&sub, &diag, &sup, &b, &mut plenty()).unwrap();
    for i in 0..3 {
        assert!((got[i] - x[i]).abs() <= 1e-14, "{got:?}");
    }
}
