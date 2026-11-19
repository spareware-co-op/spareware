//! The recursion bound of `geom-linalg`'s module headers, run at its bound (chapter 28.8; the
//! recursion check of piece G13): the factors of a rank-deficient matrix, which hold a second
//! factorisation inside the first, are cloned, compared, printed and dropped.

#![allow(clippy::unwrap_used, reason = "tests: a failure stops the test")]

use geom_linalg::Work;
use geom_linalg::sparse::Sparse;
use geom_linalg::sparse_qr::{SparseQr, SymbolicQr};
use geom_num::real::Real;

#[test]
fn a_staircase_clones_compares_and_prints_at_its_bound() {
    // Columns 0 and 1 are alike, so one is dead and the factors hold the factors of Rᵀ: two deep.
    let a = Sparse::from_triplets(
        3,
        3,
        &[
            (0, 0, 1.0),
            (1, 0, 1.0),
            (0, 1, 2.0),
            (1, 1, 2.0),
            (2, 2, 1.0),
        ],
    )
    .unwrap();
    let mut work = Work::new(u64::MAX);
    let symbolic = SymbolicQr::new(&a, &mut work).unwrap();
    let qr = SparseQr::factor(&symbolic, &a, Real::new(1e-12).unwrap(), &mut work).unwrap();
    assert!(qr.rank() < 3);
    let again = qr.clone();
    assert_eq!(again, qr);
    assert_ne!(format!("{qr:?}"), "");
    drop(again);
    drop(qr);
}
