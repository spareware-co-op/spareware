//! Audit findings in `geom-linalg` (independent audit of the merged core, branch `audit/math`).
//!
//! Every test here FAILS on the code as merged: each one shows a finding, with the input, what the
//! kernel returns and what it should return. Nothing in `src/` is changed; the fix is the owner's
//! to decide. The right answers come from closed forms, from the kernel's own Golub-Kahan SVD on
//! the same input (which the audit's stress run checked against reconstruction and orthogonality)
//! and from exact scaling by a power of two.
//!
//! 1. One-sided Jacobi refuses exactly rank-deficient matrices (`NotFinite`).
//! 2. The symmetric eigensolver refuses, or does not converge on, small matrices whose entries are
//!    normal doubles near 2^-1000.
//! 3. The pivoted QR's diagonal can increase by a unit in the last place, against its header.

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
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    reason = "tests: native arithmetic, casts and indexing, platform maths on an independent oracle (the closed form as it is written), and a failure stops the test"
)]

use geom_linalg::{Matrix, PivotedQr, Svd, SymmetricEigen, Work};

fn work() -> Work {
    Work::new(1 << 40)
}

/// 2^k exactly, for k in the normal range, from its bits.
fn pow2(k: i32) -> f64 {
    f64::from_bits(((1023 + k) as u64) << 52)
}

/// The singular values of the 2 x 2 matrix [[a, b], [c, d]], largest first, by the closed form
/// on s = a² + b² + c² + d² and the determinant: σ² = (s ± √(s² − 4·det²))/2.
fn closed_form_2x2(a: f64, b: f64, c: f64, d: f64) -> [f64; 2] {
    let s = a * a + b * b + c * c + d * d;
    let det = a * d - b * c;
    let root = (s * s - 4.0 * det * det).max(0.0).sqrt();
    let big = ((s + root) / 2.0).sqrt();
    let small = if big == 0.0 { 0.0 } else { det.abs() / big };
    [big, small]
}

/// Finding 1, smallest case. Input: [[1, 2], [-1, -2]], rank one (the second column is twice the
/// first). Right answer: singular values (√10, 0), by the closed form, and Golub-Kahan agrees.
/// Returned: `Err(NotFinite)`.
///
/// Why: after the first rotation the smaller column is rounding noise still parallel to the other,
/// so the cosine the sweep tests (taken from the normalised columns) stays at 1 however small the
/// column is. Every sweep rotates again and shrinks the column by about 2^-53, until the ratio of
/// the two norms leaves the doubles and the rotation angle is not finite. A column is only
/// skipped when its norm is exactly zero.
#[test]
fn jacobi_refuses_a_rank_one_2x2() {
    let a = Matrix::from_rows(2, 2, &[1.0, 2.0, -1.0, -2.0]).unwrap();
    let want = closed_form_2x2(1.0, 2.0, -1.0, -2.0);
    assert_eq!(want[1], 0.0);
    let gk = Svd::golub_kahan(&a, &mut work()).unwrap();
    assert!((gk.singular_values()[0] - want[0]).abs() <= 1e-14 * want[0]);
    assert!(gk.singular_values()[1] <= 1e-15 * want[0]);
    let jacobi = Svd::jacobi(&a, &mut work());
    let svd = jacobi.expect("one-sided Jacobi on [[1, 2], [-1, -2]] (rank one)");
    assert!((svd.singular_values()[0] - want[0]).abs() <= 1e-14 * want[0]);
    assert!(svd.singular_values()[1] <= 1e-15 * want[0]);
}

/// Finding 1, how common it is. Input: every 2 x 2 matrix with entries in {-2, ..., 2} (625
/// matrices). Right answer: `Ok`, with the closed form's singular values. Returned: `NotFinite`
/// for a share of the singular ones (the audit's run of random 2 x 2 to 3 x 3 matrices of such
/// entries saw 7,329 refusals in 300,000).
#[test]
fn jacobi_factors_every_small_integer_2x2() {
    let mut refused = Vec::new();
    for code in 0..625_u32 {
        let e: Vec<f64> = (0..4)
            .map(|k| f64::from((code / 5_u32.pow(k)) % 5) - 2.0)
            .collect();
        let a = Matrix::from_rows(2, 2, &e).unwrap();
        let want = closed_form_2x2(e[0], e[1], e[2], e[3]);
        match Svd::jacobi(&a, &mut work()) {
            Ok(svd) => {
                let s = svd.singular_values();
                let scale = want[0].max(1.0);
                assert!((s[0] - want[0]).abs() <= 1e-13 * scale, "{e:?}: {s:?}");
                assert!((s[1] - want[1]).abs() <= 1e-13 * scale, "{e:?}: {s:?}");
            }
            Err(error) => refused.push((e, error)),
        }
    }
    assert!(
        refused.is_empty(),
        "{} of 625 matrices refused, the first: {:?}",
        refused.len(),
        refused.first()
    );
}

/// Finding 1, a 3 x 3 case. Input: [[-2, 0, 1], [-4, 0, 2], [0, -1, 1]] (determinant 0, rank 2).
/// Right answer: what Golub-Kahan returns for it, (5.02149126394341, 1.335898755946729, 0).
/// Returned: `Err(NotFinite)`.
#[test]
fn jacobi_refuses_a_singular_3x3() {
    let a = Matrix::from_rows(3, 3, &[-2.0, 0.0, 1.0, -4.0, 0.0, 2.0, 0.0, -1.0, 1.0]).unwrap();
    let gk = Svd::golub_kahan(&a, &mut work()).unwrap();
    let want = gk.singular_values();
    assert!(want[2] <= 1e-15 * want[0]);
    let svd = Svd::jacobi(&a, &mut work()).expect("one-sided Jacobi on a singular 3 x 3");
    for (got, want) in svd.singular_values().iter().zip(want) {
        assert!((got - want).abs() <= 1e-13 * gk.singular_values()[0]);
    }
}

/// The eigenvalues of `ints`·2^-1000 must be those of `ints` times 2^-1000: scaling a matrix by a
/// power of two scales every eigenvalue by it exactly, and 2^-1000 times anything of the size of
/// these entries is a normal double.
fn eigen_scales(ints: &[f64], n: usize) {
    let unit = Matrix::from_rows(n, n, ints).unwrap();
    let reference = SymmetricEigen::factor(&unit, &mut work()).unwrap();
    let scale = pow2(-1000);
    let small: Vec<f64> = ints.iter().map(|v| v * scale).collect();
    let a = Matrix::from_rows(n, n, &small).unwrap();
    let eig = SymmetricEigen::factor(&a, &mut work())
        .expect("the eigendecomposition of small integers times 2^-1000");
    let largest = reference
        .values()
        .iter()
        .fold(0.0_f64, |m, v| m.max(v.abs()));
    for (got, want) in eig.values().iter().zip(reference.values()) {
        assert!(
            (got / scale - want).abs() <= 1e-12 * largest,
            "{got:e} against {want:e}"
        );
    }
}

/// Finding 2. Input: [[3, 0, 1, 3], [0, 0, 0, 0], [1, 0, 2, 0], [3, 0, 0, 2]] · 2^-1000 (entries
/// about 10^-301, normal doubles; the same integers at scale 1 factor without trouble). Right
/// answer: the eigenvalues of the integer matrix times 2^-1000. Returned: `Err(NotFinite)`.
///
/// Why: a Householder column that is rounding noise beside entries of 2^-1000 is subnormal, and
/// the reflection's scale 1/(α − β) then overflows. The header promises `NotFinite` only for a
/// matrix whose eigenvalues or steps leave the doubles; nothing here does. The SVDs avoid this by
/// lifting a matrix whose entries are all below 2^-500; the eigensolver has no such step.
#[test]
fn eigen_refuses_small_normal_entries() {
    eigen_scales(
        &[
            3.0, 0.0, 1.0, 3.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 3.0, 0.0, 0.0, 2.0,
        ],
        4,
    );
}

/// Finding 2, the other way it shows. Input: [[0, 1, -2, -1], [1, 3, 1, 3], [-2, 1, -1, -1],
/// [-1, 3, -1, 3]] · 2^-1000. Right answer: the eigenvalues of the integer matrix times 2^-1000.
/// Returned: `Err(NoConvergence)`: the deflation threshold 2^-52·(|a| + |b|) underflows to zero
/// beside diagonal entries that are themselves near the subnormals, so an off-diagonal entry of
/// a few subnormal steps is never taken for zero.
#[test]
fn eigen_does_not_converge_on_small_normal_entries() {
    eigen_scales(
        &[
            0.0, 1.0, -2.0, -1.0, 1.0, 3.0, 1.0, 3.0, -2.0, 1.0, -1.0, -1.0, -1.0, 3.0, -1.0, 3.0,
        ],
        4,
    );
}

/// Finding 3 (the header claims more than holds). Input: [[2, 2, -2], [-2, 1, 2], [1, -2, 1]],
/// three columns of length exactly 3, the first two orthogonal, so in exact arithmetic
/// |R00| = |R11| = 3. The module header says R's diagonal magnitudes "never increase". Returned:
/// |R00| = 3 and |R11| = 3.0000000000000004. The claim holds in exact arithmetic; in doubles a
/// reflection's β can round above the norm the pivot was chosen by (795 of the 400,000 random
/// small matrices of the audit's run showed an increase, each of one unit in the last place).
/// `PivotedQr::rank` stops at the first small entry, so an increase of one unit in the last
/// place can only matter at a tolerance that falls between two such entries.
#[test]
fn pivoted_qr_diagonal_never_increases() {
    let a = Matrix::from_rows(3, 3, &[2.0, 2.0, -2.0, -2.0, 1.0, 2.0, 1.0, -2.0, 1.0]).unwrap();
    let qr = PivotedQr::factor(&a, &mut work()).unwrap();
    let d: Vec<f64> = (0..3).map(|k| qr.r_diagonal(k).unwrap().abs()).collect();
    assert!(
        d[0] >= d[1] && d[1] >= d[2],
        "R's diagonal magnitudes: {d:?}"
    );
}
