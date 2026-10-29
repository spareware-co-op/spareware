//! Tests of the promises the SVD and the symmetric eigensolver make beyond the accuracy that
//! `tests/spectral.rs` checks (piece G50).
//!
//! What this file proves: the rank counts singular values strictly above the tolerance times the
//! largest, and only positive ones; the verdicts accept equal thresholds and refuse crossed ones;
//! both SVDs and the eigensolver commute with scaling by a power of two, bit for bit, since every
//! rule they apply is relative; a negligible off-diagonal entry splits the eigensolver's block, so
//! the part above it is left exactly as it was; Wilkinson's shift on a tie takes the eigenvalue of
//! the trailing block below the middle, in both iterations; and a zero singular value gets a unit
//! vector even when every coordinate vector keeps only half its length against the others.

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

use geom_linalg::{LinalgError, Matrix, Ranks, Svd, SymmetricEigen, Work};
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

fn entries(m: &Matrix) -> Vec<f64> {
    (0..m.rows())
        .flat_map(|i| (0..m.cols()).map(move |j| (i, j)))
        .map(|(i, j)| m.get(i, j).unwrap())
        .collect()
}

fn scaled(m: &Matrix, f: f64) -> Matrix {
    let e: Vec<f64> = entries(m).iter().map(|x| x * f).collect();
    Matrix::from_rows(m.rows(), m.cols(), &e).unwrap()
}

fn diagonal(d: &[f64]) -> Matrix {
    let n = d.len();
    let mut m = Matrix::zeros(n, n).unwrap();
    for (i, &x) in d.iter().enumerate() {
        m.set(i, i, x).unwrap();
    }
    m
}

#[test]
fn rank_counts_positive_values_strictly_above_the_tolerance() {
    let svd = Svd::jacobi(&diagonal(&[4.0, 1.0, 0.0]), &mut plenty()).unwrap();
    assert_eq!(svd.singular_values(), &[4.0, 1.0, 0.0]);
    // 1 is exactly 0.25 · 4: not above it.
    assert_eq!(svd.rank(tol(0.25)), 1);
    assert_eq!(svd.rank(tol(0.249)), 2);
    // A tolerance at or below zero still counts only the positive values.
    assert_eq!(svd.rank(tol(0.0)), 2);
    assert_eq!(svd.rank(tol(-1.0)), 2);
    let svd = Svd::golub_kahan(&diagonal(&[100.0, 0.5]), &mut plenty()).unwrap();
    assert_eq!(svd.rank(tol(0.01)), 1);
    assert_eq!(svd.rank(tol(0.004)), 2);
}

#[test]
fn verdicts_accept_equal_thresholds_and_refuse_crossed_ones() {
    let svd = Svd::jacobi(&diagonal(&[4.0, 1.0, 0.25, 0.0]), &mut plenty()).unwrap();
    assert_eq!(
        svd.classify(tol(1.0), tol(1.0)),
        Ok(Ranks {
            independent: 2,
            near_singular: 0,
            dependent: 2,
        })
    );
    assert_eq!(
        svd.classify(tol(1.0), tol(0.25)),
        Ok(Ranks {
            independent: 2,
            near_singular: 0,
            dependent: 2,
        })
    );
    assert_eq!(
        svd.classify(tol(2.0), tol(0.5)),
        Ok(Ranks {
            independent: 1,
            near_singular: 1,
            dependent: 2,
        })
    );
    assert_eq!(svd.classify(tol(0.5), tol(2.0)), Err(LinalgError::Shape));
}

#[test]
fn every_factorisation_commutes_with_scaling_by_a_power_of_two() {
    // Every deflation, convergence and shift rule is relative, so scaling A by 2^k scales Σ and
    // the eigenvalues by 2^k and leaves U, V and Q unchanged, bit for bit, while nothing
    // overflows or underflows.
    let mut rng = Lcg(21);
    for (m, n) in [(2, 2), (3, 3), (4, 4), (5, 3), (3, 5), (6, 6), (7, 4)] {
        for _ in 0..4 {
            let e: Vec<f64> = (0..m * n).map(|_| rng.int(9) as f64).collect();
            let a = Matrix::from_rows(m, n, &e).unwrap();
            for f in [2f64.powi(40), 2f64.powi(-40)] {
                let b = scaled(&a, f);
                for (name, svd) in [
                    ("Jacobi", Svd::jacobi as fn(&Matrix, &mut Work) -> _),
                    ("Golub-Kahan", Svd::golub_kahan),
                ] {
                    let x = svd(&a, &mut plenty()).unwrap();
                    let y = svd(&b, &mut plenty()).unwrap();
                    let xs: Vec<f64> = x.singular_values().iter().map(|s| s * f).collect();
                    assert_eq!(y.singular_values(), &xs[..], "{name}, {m} x {n}, 2^k = {f}");
                    assert_eq!(y.u(), x.u(), "{name} U, {m} x {n}");
                    assert_eq!(y.v(), x.v(), "{name} V, {m} x {n}");
                }
            }
            if m == n {
                let mut s = Matrix::zeros(n, n).unwrap();
                for i in 0..n {
                    for j in 0..n {
                        s.set(i, j, e[i * n + j] + e[j * n + i]).unwrap();
                    }
                }
                for f in [2f64.powi(40), 2f64.powi(-40)] {
                    let x = SymmetricEigen::factor(&s, &mut plenty()).unwrap();
                    let y = SymmetricEigen::factor(&scaled(&s, f), &mut plenty()).unwrap();
                    let xs: Vec<f64> = x.values().iter().map(|v| v * f).collect();
                    assert_eq!(y.values(), &xs[..], "eigenvalues, {n} x {n}");
                    assert_eq!(y.vectors(), x.vectors(), "eigenvectors, {n} x {n}");
                }
            }
        }
    }
}

#[test]
fn a_negligible_off_diagonal_entry_splits_the_eigensolver_block() {
    // The 1e-20 between the first two rows is far below 2^-52 · (1 + 2): the iteration works on
    // the lower block alone, so the first eigenpair is exactly (1, e₀).
    let a = Matrix::from_rows(3, 3, &[1.0, 1e-20, 0.0, 1e-20, 2.0, 1.0, 0.0, 1.0, 5.0]).unwrap();
    let e = SymmetricEigen::factor(&a, &mut plenty()).unwrap();
    assert_eq!(e.values()[0], 1.0);
    let v = e.vectors();
    assert_eq!(v.get(0, 0).unwrap().abs(), 1.0);
    assert_eq!(v.get(1, 0), Ok(0.0));
    assert_eq!(v.get(2, 0), Ok(0.0));
}

#[test]
fn the_eigensolver_takes_the_lower_shift_on_a_tie() {
    // The trailing block's diagonal entries are equal (d = 0), so its two eigenvalues, 0 and 2,
    // are equally near either: Wilkinson's shift takes the one below the middle (sign(0) = +1 in
    // μ = a − b² / (d + sign(d)·√(d² + b²))), μ = 0, and the first rotation turns by +45°. The
    // eigenvector of 0 is then (−c, c), c the double nearest √2/2; the other shift turns the
    // other way.
    let a = Matrix::from_rows(2, 2, &[1.0, 1.0, 1.0, 1.0]).unwrap();
    let e = SymmetricEigen::factor(&a, &mut plenty()).unwrap();
    let c = f64::from_bits(0x3FE6_A09E_667F_3BCC);
    assert_eq!(e.values(), &[0.0, f64::from_bits(0x3FFF_FFFF_FFFF_FFFE)]);
    assert_eq!(entries(e.vectors()), vec![-c, c, c, c]);
}

#[test]
fn golub_kahan_takes_the_lower_shift_on_a_tie() {
    // An upper bidiagonal matrix whose trailing 2 × 2 of BᵀB has equal diagonal entries
    // (2² + 1² both), so δ = 0 and the shift is the root below, as in the eigensolver. The bits
    // are this implementation's (the same on every target, as the golden geom-linalg/spectral
    // checks); the other root converges to other bits.
    let b = Matrix::from_rows(3, 3, &[3.0, 1.0, 0.0, 0.0, 2.0, 1.0, 0.0, 0.0, 2.0]).unwrap();
    let s = Svd::golub_kahan(&b, &mut plenty()).unwrap();
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(
        bits(s.singular_values()),
        vec![
            4_614_572_989_722_357_713,
            4_612_698_179_346_440_495,
            4_609_401_108_263_046_385
        ]
    );
    assert_eq!(
        bits(&entries(s.u())),
        vec![
            4_606_531_020_884_009_199,
            13_823_048_456_275_842_388,
            4_595_228_803_255_068_592,
            4_600_122_314_198_972_248,
            4_604_180_019_048_437_077,
            13_827_435_308_468_818_490,
            4_592_284_848_893_830_743,
            4_604_180_019_048_437_077,
            4_604_820_869_484_609_327,
        ]
    );
    assert_eq!(
        bits(&entries(s.v())),
        vec![
            4_605_812_894_129_065_381,
            13_824_398_004_167_912_509,
            4_599_762_255_012_644_827,
            4_602_686_544_100_353_851,
            4_601_025_967_313_136_701,
            13_828_421_555_111_261_069,
            4_595_420_720_347_958_484,
            4_605_529_566_940_507_198,
            4_603_135_027_674_565_566,
        ]
    );
}

#[test]
fn a_zero_singular_value_gets_a_unit_vector_when_every_coordinate_keeps_half() {
    // Three columns of a 4 × 4 Hadamard matrix, doubled, and a zero column. Every coordinate
    // vector keeps exactly half its length against the three, so a rule asking for more than half
    // finds none; the longest, the first of equal ones, is e₀, and the fourth column of U is e₀
    // made orthogonal: (1, 1, −1, −1)/2 (e₃ would give its opposite).
    let h = [
        [1.0, 1.0, 1.0, 1.0],
        [1.0, -1.0, 1.0, -1.0],
        [1.0, -1.0, -1.0, 1.0],
    ];
    let mut a = Matrix::zeros(4, 4).unwrap();
    for (j, col) in h.iter().enumerate() {
        for i in 0..4 {
            a.set(i, j, col[i]).unwrap();
        }
    }
    let svd = Svd::jacobi(&a, &mut plenty()).unwrap();
    assert_eq!(svd.singular_values(), &[2.0, 2.0, 2.0, 0.0]);
    let u = svd.u();
    let last: Vec<f64> = (0..4).map(|i| u.get(i, 3).unwrap()).collect();
    assert_eq!(last, vec![0.5, 0.5, -0.5, -0.5]);
}
