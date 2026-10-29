//! The benchmark of pieces G49 and G50 (chapter 30): a column-pivoted QR of 256 unknowns, the
//! board's "under 20 ms", with LU and the complete orthogonal decomposition of the same matrix
//! beside it; and both SVDs and the symmetric eigensolver on 64 × 64.
//!
//! Run with `cargo bench -p geom-linalg`. It uses the standard library's clock only (wall-clock
//! time on this machine; the reference machine and instruction counts are geom-bench's, piece
//! G18). The matrix comes from a fixed linear congruential sequence, so every run measures the same
//! work.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_linalg::{Cod, Lu, Matrix, PivotedQr, Svd, SymmetricEigen, Work};
use geom_num::real::Real;

fn matrix(n: usize) -> Matrix {
    let mut state = 7u64;
    let entries: Vec<f64> = (0..n * n)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((state >> 33) % 2001) as f64 - 1000.0
        })
        .collect();
    Matrix::from_rows(n, n, &entries).unwrap()
}

fn time(name: &str, runs: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let ms = start.elapsed().as_secs_f64() * 1e3 / f64::from(runs);
    println!("{name:<44} {ms:>8.2} ms");
}

fn main() {
    let a = matrix(256);
    time("pivoted QR, 256 × 256", 10, || {
        black_box(PivotedQr::factor(black_box(&a), &mut Work::new(u64::MAX)).unwrap());
    });
    time("complete orthogonal decomposition, 256 × 256", 10, || {
        black_box(
            Cod::factor(
                black_box(&a),
                Real::new(1e-12).unwrap(),
                &mut Work::new(u64::MAX),
            )
            .unwrap(),
        );
    });
    time("LU with partial pivoting, 256 × 256", 10, || {
        black_box(Lu::factor(black_box(&a), &mut Work::new(u64::MAX)).unwrap());
    });
    // Piece G50: the SVDs and the symmetric eigensolver on 64 × 64.
    let b = matrix(64);
    time("SVD by one-sided Jacobi, 64 × 64", 5, || {
        black_box(Svd::jacobi(black_box(&b), &mut Work::new(u64::MAX)).unwrap());
    });
    time("SVD by Golub–Kahan, 64 × 64", 5, || {
        black_box(Svd::golub_kahan(black_box(&b), &mut Work::new(u64::MAX)).unwrap());
    });
    let mut sym = vec![0.0; 64 * 64];
    for i in 0..64 {
        for j in 0..64 {
            sym[i * 64 + j] = b.get(i, j).unwrap() + b.get(j, i).unwrap();
        }
    }
    let s = Matrix::from_rows(64, 64, &sym).unwrap();
    time("symmetric eigensolver, 64 × 64", 5, || {
        black_box(SymmetricEigen::factor(black_box(&s), &mut Work::new(u64::MAX)).unwrap());
    });
}
