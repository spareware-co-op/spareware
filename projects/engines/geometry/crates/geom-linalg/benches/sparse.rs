//! The benchmark of piece G215 (chapter 30): a system of 10,000 unknowns, the board's "in 20 ms".
//!
//! Run with `cargo bench -p geom-linalg --bench sparse`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18). The systems have the shape a sketch gives: a chain of 10,000 unknowns, three to a
//! constraint, each constraint linking its neighbours, with a fixed linear congruential sequence for
//! the values; and a 100 × 100 grid's Laplacian for the symmetric paths.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_linalg::colamd::colamd;
use geom_linalg::{
    Sparse, SparseCholesky, SparseLdlt, SparseQr, SymbolicCholesky, SymbolicQr, Work,
};
use geom_num::real::Real;

fn chain(n: usize) -> Sparse {
    let mut state = 7u64;
    let mut value = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) % 2001) as f64 / 1000.0 - 1.0
    };
    let mut triplets = Vec::new();
    for i in 0..n {
        triplets.push((i, i, 3.0));
        if i >= 1 {
            triplets.push((i, i - 1, value()));
        }
        if i >= 2 {
            triplets.push((i, i - 2, value()));
        }
    }
    Sparse::from_triplets(n, n, &triplets).unwrap()
}

fn grid(side: usize) -> Sparse {
    let mut triplets = Vec::new();
    for x in 0..side {
        for y in 0..side {
            let i = x * side + y;
            triplets.push((i, i, 4.25));
            if x + 1 < side {
                triplets.push((i, i + side, -1.0));
                triplets.push((i + side, i, -1.0));
            }
            if y + 1 < side {
                triplets.push((i, i + 1, -1.0));
                triplets.push((i + 1, i, -1.0));
            }
        }
    }
    Sparse::from_triplets(side * side, side * side, &triplets).unwrap()
}

fn time(name: &str, runs: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let ms = start.elapsed().as_secs_f64() * 1e3 / f64::from(runs);
    println!("{name:<52} {ms:>8.2} ms");
}

fn main() {
    let a = chain(10_000);
    let b = vec![1.0; 10_000];
    let tol = Real::new(1e-12).unwrap();
    time("order, chain of 10,000", 10, || {
        black_box(colamd(black_box(&a), &mut Work::new(u64::MAX)).unwrap());
    });
    let symbolic = SymbolicQr::new(&a, &mut Work::new(u64::MAX)).unwrap();
    time("analysis, chain of 10,000", 10, || {
        black_box(SymbolicQr::new(black_box(&a), &mut Work::new(u64::MAX)).unwrap());
    });
    time("QR and solve, chain of 10,000", 10, || {
        let qr = SparseQr::factor(&symbolic, black_box(&a), tol, &mut Work::new(u64::MAX)).unwrap();
        black_box(qr.least_squares(&b, &mut Work::new(u64::MAX)).unwrap());
    });
    let g = grid(100);
    let c = vec![1.0; 10_000];
    let symbolic = SymbolicCholesky::new(&g, &mut Work::new(u64::MAX)).unwrap();
    println!("L of the grid has {} entries", symbolic.l_entries());
    time("analysis, grid of 10,000", 5, || {
        black_box(SymbolicCholesky::new(black_box(&g), &mut Work::new(u64::MAX)).unwrap());
    });
    time("Cholesky and solve, grid of 10,000", 10, || {
        let l = SparseCholesky::factor(&symbolic, black_box(&g), &mut Work::new(u64::MAX)).unwrap();
        black_box(l.solve(&c, &mut Work::new(u64::MAX)).unwrap());
    });
    time("LDLT and solve, grid of 10,000", 10, || {
        let l = SparseLdlt::factor(&symbolic, black_box(&g), &mut Work::new(u64::MAX)).unwrap();
        black_box(l.solve(&c, &mut Work::new(u64::MAX)).unwrap());
    });
}
