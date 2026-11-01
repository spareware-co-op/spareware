//! Tests of `geom_linalg`'s sparse parts (piece G215): the matrix, the column order, sparse QR and
//! sparse Cholesky and L·D·Lᵀ, each held to the dense path of pieces G49 and G50.
//!
//! What this file proves: a sparse matrix is the same whatever order its triplets came in; the
//! column order is a permutation that depends only on the pattern, breaks ties by variable order
//! and keeps an arrow matrix's factors from filling; the least-squares solution of least norm
//! equals the dense complete orthogonal decomposition's on over-determined, square,
//! under-determined and rank-deficient systems; an analysis serves every matrix of its pattern and
//! refuses any other; Cholesky and L·D·Lᵀ equal their dense answers and refuse what they must; and
//! every factorisation charges before it starts, refusing a budget one unit short.
//!
//! Seeds: each test prints `seed: <hex>` and replays with `SPAREKIT_SEED`.

#![allow(
    clippy::disallowed_methods,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::needless_range_loop,
    reason = "oracles and generators, indexed as the formulas are; the rules, chapter 21's bans included, are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_linalg::colamd::{colamd, symmetric};
use geom_linalg::sparse::Sparse;
use geom_linalg::sparse_cholesky::{SparseCholesky, SparseLdlt, SymbolicCholesky};
use geom_linalg::sparse_qr::{SparseQr, SymbolicQr};
use geom_linalg::{Cholesky, Cod, LinalgError, Lu, Matrix, Work};
use geom_num::real::Real;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn work() -> Work {
    Work::new(u64::MAX)
}

fn rng(label: &str) -> SplitMix64 {
    SplitMix64::new(resolve(label, Seed::DEFAULT).unwrap())
}

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.next_u64() % n
}

fn tol() -> Real {
    Real::new(1e-11).unwrap()
}

/// A matrix with about `per` entries a column, small whole numbers, and each column non-empty.
fn scattered(rng: &mut SplitMix64, m: usize, n: usize, per: usize) -> Sparse {
    let mut triplets = Vec::new();
    for j in 0..n {
        for _ in 0..per {
            let i = below(rng, m as u64) as usize;
            let value = below(rng, 17) as f64 - 8.0;
            triplets.push((i, j, if value == 0.0 { 9.0 } else { value }));
        }
    }
    Sparse::from_triplets(m, n, &triplets).unwrap()
}

fn scale(a: &[f64]) -> f64 {
    a.iter()
        .fold(1.0, |m, x| if x.abs() > m { x.abs() } else { m })
}

fn close(a: &[f64], b: &[f64], within: f64) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| (x - y).abs() <= within * scale(b))
}

fn dense_answer(a: &Sparse, b: &[f64]) -> Vec<f64> {
    Cod::factor(&a.to_dense().unwrap(), tol(), &mut work())
        .unwrap()
        .least_squares(b, &mut work())
        .unwrap()
}

fn sparse_answer(a: &Sparse, b: &[f64]) -> (Vec<f64>, usize) {
    let symbolic = SymbolicQr::new(a, &mut work()).unwrap();
    let qr = SparseQr::factor(&symbolic, a, tol(), &mut work()).unwrap();
    (qr.least_squares(b, &mut work()).unwrap(), qr.rank())
}

/// The rank a dense pivoted factorisation finds.
fn dense_rank(a: &Sparse) -> usize {
    Cod::factor(&a.to_dense().unwrap(), tol(), &mut work())
        .unwrap()
        .rank()
}

fn permutation(order: &[usize], n: usize) -> bool {
    let mut seen = order.to_vec();
    seen.sort_unstable();
    seen == (0..n).collect::<Vec<_>>()
}

/// A column with an entry in every row beside a diagonal: the arrow that fills if eliminated first.
fn arrow(n: usize) -> Sparse {
    let mut triplets = Vec::new();
    for i in 0..n {
        triplets.push((i, 0, 1.0));
        if i > 0 {
            triplets.push((i, i, 2.0));
        }
    }
    Sparse::from_triplets(n, n, &triplets).unwrap()
}

/// A 2D grid's Laplacian, with `shift` on its diagonal.
fn grid(side: usize, shift: f64) -> Sparse {
    let n = side * side;
    let mut triplets = Vec::new();
    for x in 0..side {
        for y in 0..side {
            let i = x * side + y;
            triplets.push((i, i, 4.0 + shift));
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
    Sparse::from_triplets(n, n, &triplets).unwrap()
}

#[test]
fn a_matrix_is_the_same_whatever_order_its_triplets_came_in() {
    let forward = [
        (0, 0, 1.0),
        (2, 0, 2.0),
        (1, 1, 3.0),
        (0, 2, 4.0),
        (2, 2, 5.0),
    ];
    let mut backward = forward;
    backward.reverse();
    let (a, b) = (
        Sparse::from_triplets(3, 3, &forward).unwrap(),
        Sparse::from_triplets(3, 3, &backward).unwrap(),
    );
    assert_eq!(a, b);
    assert_eq!((a.rows(), a.cols(), a.nonzeros()), (3, 3, 5));
    assert_eq!(a.column(0).unwrap(), (&[0, 2][..], &[1.0, 2.0][..]));
    assert_eq!(a.column(1).unwrap(), (&[1][..], &[3.0][..]));
    assert_eq!(a.column(2).unwrap(), (&[0, 2][..], &[4.0, 5.0][..]));
    assert_eq!(a.column(3), Err(LinalgError::Shape));
}

#[test]
fn repeats_add_in_the_order_given_and_a_zero_stays_stored() {
    let a = Sparse::from_triplets(2, 2, &[(1, 1, 2.0), (1, 1, -2.0), (0, 1, 1.5), (0, 1, 2.5)])
        .unwrap();
    assert_eq!(a.column(1).unwrap(), (&[0, 1][..], &[4.0, 0.0][..]));
    assert_eq!(a.nonzeros(), 2);
    // Overflow in the order given is an overflow; the same numbers the other way are not.
    let big = f64::MAX;
    assert_eq!(
        Sparse::from_triplets(1, 1, &[(0, 0, big), (0, 0, big), (0, 0, -big)]),
        Err(LinalgError::NotFinite)
    );
    let fine = Sparse::from_triplets(1, 1, &[(0, 0, big), (0, 0, -big), (0, 0, big)]).unwrap();
    assert_eq!(fine.column(0).unwrap().1, &[big]);
}

#[test]
fn a_bad_triplet_is_refused() {
    assert_eq!(
        Sparse::from_triplets(2, 2, &[(2, 0, 1.0)]),
        Err(LinalgError::Shape)
    );
    assert_eq!(
        Sparse::from_triplets(2, 2, &[(0, 2, 1.0)]),
        Err(LinalgError::Shape)
    );
    assert_eq!(
        Sparse::from_triplets(2, 2, &[(0, 0, f64::NAN)]),
        Err(LinalgError::NotFinite)
    );
    assert_eq!(
        Sparse::from_triplets(2, 2, &[(0, 0, f64::INFINITY)]),
        Err(LinalgError::NotFinite)
    );
}

#[test]
fn an_entry_is_found_and_set_in_place() {
    let mut a = Sparse::from_triplets(3, 2, &[(0, 0, 1.0), (2, 0, 2.0), (1, 1, 3.0)]).unwrap();
    assert_eq!(a.place_of(0, 0), Ok(0));
    assert_eq!(a.place_of(2, 0), Ok(1));
    assert_eq!(a.place_of(1, 1), Ok(2));
    assert_eq!(a.place_of(1, 0), Err(LinalgError::Shape));
    assert_eq!(a.place_of(0, 2), Err(LinalgError::Shape));
    a.set_at(1, 7.0).unwrap();
    assert_eq!(a.column(0).unwrap().1, &[1.0, 7.0]);
    assert_eq!(a.set_at(3, 1.0), Err(LinalgError::Shape));
    assert_eq!(a.set_at(0, f64::NAN), Err(LinalgError::NotFinite));
    assert_eq!(a.column(0).unwrap().1, &[1.0, 7.0]);
}

#[test]
fn it_multiplies_transposes_and_goes_to_and_from_dense() {
    let dense = Matrix::from_rows(2, 3, &[1.0, 0.0, 2.0, 0.0, 3.0, -4.0]).unwrap();
    let a = Sparse::from_dense(&dense).unwrap();
    assert_eq!(a.nonzeros(), 4);
    assert_eq!(a.to_dense().unwrap(), dense);
    assert_eq!(a.multiply(&[1.0, 2.0, 3.0]).unwrap(), [7.0, -6.0]);
    assert_eq!(a.multiply(&[1.0]), Err(LinalgError::Shape));
    let t = a.transpose().unwrap();
    assert_eq!((t.rows(), t.cols()), (3, 2));
    assert_eq!(t.multiply(&[1.0, 1.0]).unwrap(), [1.0, 3.0, -2.0]);
    assert_eq!(t.transpose().unwrap(), a);
    let huge = Sparse::from_triplets(1, 1, &[(0, 0, f64::MAX)]).unwrap();
    assert_eq!(huge.multiply(&[2.0]), Err(LinalgError::NotFinite));
}

#[test]
fn the_order_is_a_permutation_that_reads_only_the_pattern() {
    let mut rng = rng("sparse order");
    for round in 0..200 {
        let (m, n) = (
            1 + below(&mut rng, 40) as usize,
            1 + below(&mut rng, 40) as usize,
        );
        let a = scattered(&mut rng, m, n, 1 + round % 5);
        let order = colamd(&a, &mut work()).unwrap();
        assert!(permutation(&order, n), "{order:?}");
        let mut doubled = Vec::new();
        for j in 0..n {
            let (rows, values) = a.column(j).unwrap();
            for (&i, &value) in rows.iter().zip(values) {
                doubled.push((i, j, value * 3.0 + 1.0));
            }
        }
        let same = Sparse::from_triplets(m, n, &doubled).unwrap();
        assert_eq!(colamd(&same, &mut work()).unwrap(), order);
    }
}

#[test]
fn a_tie_goes_to_the_lowest_variable() {
    let diagonal =
        Sparse::from_triplets(4, 4, &[(0, 0, 1.0), (1, 1, 1.0), (2, 2, 1.0), (3, 3, 1.0)]).unwrap();
    assert_eq!(colamd(&diagonal, &mut work()).unwrap(), [0, 1, 2, 3]);
    // Two alike columns stay together, in their own order, where a third has less degree.
    let alike = Sparse::from_triplets(
        3,
        3,
        &[
            (0, 1, 1.0),
            (0, 2, 1.0),
            (1, 1, 1.0),
            (1, 2, 1.0),
            (2, 0, 1.0),
        ],
    )
    .unwrap();
    assert_eq!(colamd(&alike, &mut work()).unwrap(), [0, 1, 2]);
}

#[test]
fn empty_and_dense_columns_go_last_in_their_own_order() {
    let empty = Sparse::from_triplets(3, 6, &[(0, 2, 1.0), (1, 3, 1.0), (2, 4, 1.0)]).unwrap();
    assert_eq!(colamd(&empty, &mut work()).unwrap(), [2, 3, 4, 0, 1, 5]);
    // A column of more than 16 entries and over 10·√m is dense: all of 200 rows here.
    let mut rows = Vec::new();
    for i in 0..200 {
        rows.push((i, 0, 1.0));
    }
    let mut row = 0;
    for j in 1..5 {
        rows.push((row, j, 1.0));
        row += 12;
    }
    let a = Sparse::from_triplets(200, 5, &rows).unwrap();
    assert_eq!(colamd(&a, &mut work()).unwrap(), [1, 2, 3, 4, 0]);
    // Not over the line: 30 of 30 rows stays in, and 10·√30 is 54.
    let mut rows = Vec::new();
    for i in 0..30 {
        rows.push((i, 1, 1.0));
    }
    rows.extend([(0, 0, 1.0), (1, 2, 1.0)]);
    let a = Sparse::from_triplets(30, 3, &rows).unwrap();
    assert_eq!(colamd(&a, &mut work()).unwrap(), [0, 1, 2]);
    // A row over the same line is ignored, and a column with only that row is set aside.
    let mut wide_row = Vec::new();
    for j in 0..200 {
        wide_row.push((0, j, 1.0));
    }
    wide_row.extend([(1, 3, 1.0), (1, 4, 1.0), (2, 5, 1.0)]);
    let a = Sparse::from_triplets(3, 200, &wide_row).unwrap();
    let order = colamd(&a, &mut work()).unwrap();
    assert!(permutation(&order, 200));
    assert_eq!(order[..3], [5, 3, 4]);
    assert_eq!(order[3..6], [0, 1, 2]);
}

#[test]
fn the_order_keeps_an_arrow_from_filling() {
    let a = arrow(64);
    let order = colamd(&a, &mut work()).unwrap();
    // The hub goes in the last two: at the end it ties with the last column left.
    assert!(order[62..].contains(&0), "{order:?}");
    let good = SymbolicQr::new(&a, &mut work()).unwrap();
    let bad = SymbolicQr::natural(&a, &mut work()).unwrap();
    assert_eq!(bad.order(), (0..64).collect::<Vec<_>>());
    assert_eq!(good.order(), order);
    assert_eq!(good.r_entries(), 63);
    assert_eq!(bad.r_entries(), 63 * 64 / 2);
}

#[test]
fn a_symmetric_order_puts_the_hub_last() {
    let mut triplets = Vec::new();
    for i in 0..32 {
        triplets.push((i, i, 4.0));
        if i > 0 {
            triplets.push((0, i, 1.0));
            triplets.push((i, 0, 1.0));
        }
    }
    let a = Sparse::from_triplets(32, 32, &triplets).unwrap();
    let order = symmetric(&a, &mut work()).unwrap();
    assert!(permutation(&order, 32));
    assert!(order[30..].contains(&0), "{order:?}");
    let symbolic = SymbolicCholesky::new(&a, &mut work()).unwrap();
    assert_eq!(symbolic.order(), order);
    assert_eq!(symbolic.l_entries(), 31);
    assert_eq!(
        symmetric(&Sparse::from_triplets(2, 3, &[]).unwrap(), &mut work()),
        Err(LinalgError::Shape)
    );
}

#[test]
fn full_rank_systems_equal_the_dense_path() {
    let mut rng = rng("sparse full rank");
    let mut runs = 0;
    for round in 0..400 {
        let n = 1 + below(&mut rng, 24) as usize;
        let m = n + below(&mut rng, 24) as usize;
        let a = scattered(&mut rng, m, n, 2 + round % 4);
        if dense_rank(&a) < n {
            continue;
        }
        let b: Vec<f64> = (0..m).map(|_| below(&mut rng, 41) as f64 - 20.0).collect();
        let (x, rank) = sparse_answer(&a, &b);
        assert_eq!(rank, n);
        assert!(close(&x, &dense_answer(&a, &b), 1e-8), "{m} × {n}");
        runs += 1;
    }
    assert!(runs > 100, "{runs}");
}

#[test]
fn under_determined_systems_take_the_least_norm() {
    let mut rng = rng("sparse under-determined");
    let mut runs = 0;
    for round in 0..400 {
        let m = 1 + below(&mut rng, 16) as usize;
        let n = m + 1 + below(&mut rng, 16) as usize;
        let a = scattered(&mut rng, m, n, 1 + round % 3);
        if dense_rank(&a) < m {
            continue;
        }
        let b: Vec<f64> = (0..m).map(|_| below(&mut rng, 41) as f64 - 20.0).collect();
        let (x, rank) = sparse_answer(&a, &b);
        assert_eq!(rank, m);
        assert!(close(&x, &dense_answer(&a, &b), 1e-8), "{m} × {n}");
        assert!(close(&a.multiply(&x).unwrap(), &b, 1e-9), "{m} × {n}");
        runs += 1;
    }
    assert!(runs > 100, "{runs}");
}

#[test]
fn rank_deficient_systems_stay_least_norm() {
    // Two columns alike and a column of nothing: the solution splits between the two, and the third
    // stays zero, which a basic solution would not do.
    let a = Sparse::from_triplets(
        3,
        4,
        &[
            (0, 0, 1.0),
            (0, 1, 1.0),
            (1, 0, 2.0),
            (1, 1, 2.0),
            (2, 3, 1.0),
        ],
    )
    .unwrap();
    let (x, rank) = sparse_answer(&a, &[5.0, 10.0, 3.0]);
    assert_eq!(rank, 2);
    assert!(close(&x, &[2.5, 2.5, 0.0, 3.0], 1e-12), "{x:?}");
    let b = [5.0, 10.0, 3.0];
    assert!(close(&x, &dense_answer(&a, &b), 1e-10), "{x:?}");
    // Random products of narrow factors have a known rank.
    let mut rng = rng("sparse rank deficient");
    let mut runs = 0;
    for round in 0..300 {
        let n = 4 + below(&mut rng, 16) as usize;
        let m = n + below(&mut rng, 8) as usize;
        let dropped = 1 + below(&mut rng, 3) as usize;
        let mut triplets = Vec::new();
        let base = scattered(&mut rng, m, n - dropped, 2 + round % 3);
        for j in 0..n - dropped {
            let (rows, values) = base.column(j).unwrap();
            for (&i, &value) in rows.iter().zip(values) {
                triplets.push((i, j, value));
            }
        }
        // Each dropped column is an earlier one, doubled.
        for k in 0..dropped {
            let copy = below(&mut rng, (n - dropped) as u64) as usize;
            let (rows, values) = base.column(copy).unwrap();
            for (&i, &value) in rows.iter().zip(values) {
                triplets.push((i, n - dropped + k, 2.0 * value));
            }
        }
        let a = Sparse::from_triplets(m, n, &triplets).unwrap();
        let want = dense_rank(&a);
        let b: Vec<f64> = (0..m).map(|_| below(&mut rng, 41) as f64 - 20.0).collect();
        let (x, rank) = sparse_answer(&a, &b);
        if rank == want {
            assert!(
                close(&x, &dense_answer(&a, &b), 1e-7),
                "{m} × {n}, rank {rank}"
            );
            runs += 1;
        }
    }
    assert!(runs > 150, "{runs}");
}

#[test]
fn zero_columns_and_rows_are_dead_at_once() {
    let a = Sparse::from_triplets(3, 3, &[(0, 0, 2.0), (2, 2, 4.0)]).unwrap();
    let (x, rank) = sparse_answer(&a, &[2.0, 7.0, 8.0]);
    assert_eq!(rank, 2);
    assert_eq!(x, [1.0, 0.0, 2.0]);
    let empty = Sparse::from_triplets(2, 2, &[]).unwrap();
    let (x, rank) = sparse_answer(&empty, &[1.0, 2.0]);
    assert_eq!(rank, 0);
    assert_eq!(x, [0.0, 0.0]);
    // A stored zero is no entry to the rank, and a column under the tolerance is dead.
    let faint = Sparse::from_triplets(2, 2, &[(0, 0, 1.0), (1, 1, 1e-14), (0, 1, 0.0)]).unwrap();
    assert_eq!(sparse_answer(&faint, &[1.0, 1.0]).1, 1);
    let symbolic = SymbolicQr::new(&faint, &mut work()).unwrap();
    let keep = SparseQr::factor(&symbolic, &faint, Real::ZERO, &mut work()).unwrap();
    assert_eq!(keep.rank(), 2);
    assert_eq!(keep.order(), symbolic.order());
}

#[test]
fn an_analysis_serves_every_matrix_of_its_pattern() {
    let mut rng = rng("sparse reuse");
    let a = scattered(&mut rng, 30, 20, 3);
    let symbolic = SymbolicQr::new(&a, &mut work()).unwrap();
    for _ in 0..20 {
        let mut triplets = Vec::new();
        for j in 0..20 {
            let (rows, _) = a.column(j).unwrap();
            for &i in rows {
                triplets.push((i, j, below(&mut rng, 19) as f64 - 9.5));
            }
        }
        let same = Sparse::from_triplets(30, 20, &triplets).unwrap();
        let b: Vec<f64> = (0..30).map(|_| below(&mut rng, 41) as f64 - 20.0).collect();
        let reused = SparseQr::factor(&symbolic, &same, tol(), &mut work()).unwrap();
        let fresh = sparse_answer(&same, &b).0;
        assert_eq!(reused.least_squares(&b, &mut work()).unwrap(), fresh);
    }
    let other = scattered(&mut rng, 30, 20, 2);
    assert_eq!(
        SparseQr::factor(&symbolic, &other, tol(), &mut work()),
        Err(LinalgError::Shape)
    );
    let wider = Sparse::from_triplets(31, 20, &[]).unwrap();
    assert_eq!(
        SparseQr::factor(&symbolic, &wider, tol(), &mut work()),
        Err(LinalgError::Shape)
    );
    let qr = SparseQr::factor(&symbolic, &a, tol(), &mut work()).unwrap();
    assert_eq!(
        qr.least_squares(&[1.0], &mut work()),
        Err(LinalgError::Shape)
    );
}

#[test]
fn cholesky_equals_the_dense_path() {
    for side in [1, 2, 3, 5, 8] {
        let a = grid(side, 0.25);
        let symbolic = SymbolicCholesky::new(&a, &mut work()).unwrap();
        let factors = SparseCholesky::factor(&symbolic, &a, &mut work()).unwrap();
        let n = side * side;
        let b: Vec<f64> = (0..n).map(|i| (i % 7) as f64 - 3.0).collect();
        let x = factors.solve(&b, &mut work()).unwrap();
        let dense = Cholesky::factor(&a.to_dense().unwrap(), &mut work())
            .unwrap()
            .solve(&b, &mut work())
            .unwrap();
        assert!(close(&x, &dense, 1e-12), "side {side}");
        assert!(close(&a.multiply(&x).unwrap(), &b, 1e-12), "side {side}");
        assert_eq!(
            factors.solve(&[1.0, 2.0, 3.0][..3.min(n.saturating_sub(1))], &mut work()),
            Err(LinalgError::Shape)
        );
    }
}

#[test]
fn ldlt_solves_a_quasi_definite_system() {
    // [H Bᵀ; B −C], H and C positive definite: no pivoting is needed in any symmetric order.
    let h = grid(4, 1.0);
    let n = 16;
    let mut triplets = Vec::new();
    for j in 0..n {
        let (rows, values) = h.column(j).unwrap();
        for (&i, &value) in rows.iter().zip(values) {
            triplets.push((i, j, value));
        }
    }
    for k in 0..8 {
        let (row, a, b) = (n + k, k, (2 * k + 3) % n);
        for (i, j, value) in [(row, a, 1.0), (a, row, 1.0), (row, b, -2.0), (b, row, -2.0)] {
            triplets.push((i, j, value));
        }
        triplets.push((row, row, -1.5));
    }
    let a = Sparse::from_triplets(n + 8, n + 8, &triplets).unwrap();
    let symbolic = SymbolicCholesky::new(&a, &mut work()).unwrap();
    let factors = SparseLdlt::factor(&symbolic, &a, &mut work()).unwrap();
    let b: Vec<f64> = (0..n + 8).map(|i| (i % 5) as f64 + 1.0).collect();
    let x = factors.solve(&b, &mut work()).unwrap();
    let dense = Lu::factor(&a.to_dense().unwrap(), &mut work())
        .unwrap()
        .solve(&b, &mut work())
        .unwrap();
    assert!(close(&x, &dense, 1e-11));
    assert!(close(&a.multiply(&x).unwrap(), &b, 1e-11));
    // Sylvester's law of inertia: 16 positive pivots and 8 negative.
    assert_eq!(factors.pivots().iter().filter(|d| **d > 0.0).count(), 16);
    assert_eq!(factors.pivots().iter().filter(|d| **d < 0.0).count(), 8);
    assert_eq!(factors.solve(&[1.0], &mut work()), Err(LinalgError::Shape));
    // On a positive definite matrix, L·D·Lᵀ and L·Lᵀ solve alike.
    let spd = grid(5, 0.5);
    let symbolic = SymbolicCholesky::new(&spd, &mut work()).unwrap();
    let rhs: Vec<f64> = (0..25).map(|i| i as f64).collect();
    let one = SparseLdlt::factor(&symbolic, &spd, &mut work())
        .unwrap()
        .solve(&rhs, &mut work())
        .unwrap();
    let two = SparseCholesky::factor(&symbolic, &spd, &mut work())
        .unwrap()
        .solve(&rhs, &mut work())
        .unwrap();
    assert!(close(&one, &two, 1e-13));
}

#[test]
fn what_a_factorisation_cannot_take_is_refused() {
    let indefinite =
        Sparse::from_triplets(2, 2, &[(0, 0, 1.0), (0, 1, 2.0), (1, 0, 2.0), (1, 1, 1.0)]).unwrap();
    let symbolic = SymbolicCholesky::new(&indefinite, &mut work()).unwrap();
    assert_eq!(
        SparseCholesky::factor(&symbolic, &indefinite, &mut work()),
        Err(LinalgError::NotPositiveDefinite)
    );
    assert!(SparseLdlt::factor(&symbolic, &indefinite, &mut work()).is_ok());
    let no_diagonal = Sparse::from_triplets(2, 2, &[(0, 1, 1.0), (1, 0, 1.0)]).unwrap();
    let symbolic = SymbolicCholesky::new(&no_diagonal, &mut work()).unwrap();
    assert_eq!(
        SparseCholesky::factor(&symbolic, &no_diagonal, &mut work()),
        Err(LinalgError::NotPositiveDefinite)
    );
    assert_eq!(
        SparseLdlt::factor(&symbolic, &no_diagonal, &mut work()),
        Err(LinalgError::Singular)
    );
    let lopsided =
        Sparse::from_triplets(2, 2, &[(0, 0, 4.0), (0, 1, 1.0), (1, 0, 2.0), (1, 1, 4.0)]).unwrap();
    let symbolic = SymbolicCholesky::new(&lopsided, &mut work()).unwrap();
    assert_eq!(
        SparseCholesky::factor(&symbolic, &lopsided, &mut work()),
        Err(LinalgError::NotSymmetric)
    );
    let other = Sparse::from_triplets(2, 2, &[(0, 0, 4.0)]).unwrap();
    assert_eq!(
        SparseLdlt::factor(&symbolic, &other, &mut work()),
        Err(LinalgError::Shape)
    );
    assert_eq!(
        SymbolicCholesky::new(&Sparse::from_triplets(2, 3, &[]).unwrap(), &mut work()),
        Err(LinalgError::Shape)
    );
}

/// `f` runs on the budget it needs, and is refused on one unit less.
fn charged_exactly<T: core::fmt::Debug>(f: impl Fn(&mut Work) -> Result<T, LinalgError>) -> u64 {
    let mut budget = work();
    f(&mut budget).unwrap();
    let spent = u64::MAX - budget.left();
    assert!(f(&mut Work::new(spent)).is_ok());
    assert_eq!(
        f(&mut Work::new(spent - 1)).unwrap_err(),
        LinalgError::OutOfWork
    );
    spent
}

#[test]
fn every_factorisation_charges_before_it_starts() {
    let mut rng = rng("sparse budget");
    let a = scattered(&mut rng, 20, 12, 3);
    let spent = charged_exactly(|w| colamd(&a, w));
    assert_eq!(spent, (a.nonzeros() as u64 + 12) * 13);
    charged_exactly(|w| SymbolicQr::new(&a, w));
    let symbolic = SymbolicQr::new(&a, &mut work()).unwrap();
    charged_exactly(|w| SparseQr::factor(&symbolic, &a, tol(), w));
    let qr = SparseQr::factor(&symbolic, &a, tol(), &mut work()).unwrap();
    let b = vec![1.0; 20];
    charged_exactly(|w| qr.least_squares(&b, w));
    let spd = grid(4, 0.5);
    charged_exactly(|w| SymbolicCholesky::new(&spd, w));
    let symbolic = SymbolicCholesky::new(&spd, &mut work()).unwrap();
    charged_exactly(|w| SparseCholesky::factor(&symbolic, &spd, w));
    charged_exactly(|w| SparseLdlt::factor(&symbolic, &spd, w));
    let factors = SparseCholesky::factor(&symbolic, &spd, &mut work()).unwrap();
    let c = vec![1.0; 16];
    charged_exactly(|w| factors.solve(&c, w));
    let ldlt = SparseLdlt::factor(&symbolic, &spd, &mut work()).unwrap();
    charged_exactly(|w| ldlt.solve(&c, w));
    // A staircase pays for the second factorisation too, and its solve.
    let flat =
        Sparse::from_triplets(2, 3, &[(0, 0, 1.0), (0, 1, 1.0), (1, 2, 1.0), (1, 0, 2.0)]).unwrap();
    let symbolic = SymbolicQr::new(&flat, &mut work()).unwrap();
    charged_exactly(|w| SparseQr::factor(&symbolic, &flat, tol(), w));
    let qr = SparseQr::factor(&symbolic, &flat, tol(), &mut work()).unwrap();
    charged_exactly(|w| qr.least_squares(&[1.0, 2.0], w));
}

#[test]
fn overflow_is_reported_not_returned() {
    let big = 1e308;
    let a = Sparse::from_triplets(2, 2, &[(0, 0, big), (1, 0, big), (0, 1, big), (1, 1, -big)])
        .unwrap();
    let symbolic = SymbolicCholesky::new(&a, &mut work()).unwrap();
    assert_eq!(
        SparseLdlt::factor(&symbolic, &a, &mut work())
            .map(|_| ())
            .unwrap_err(),
        LinalgError::NotFinite
    );
}

#[test]
fn an_analysis_serves_its_own_row_count_only() {
    // One pattern on three rows and on four: its columns are alike, and the shape is not.
    let three = Sparse::from_triplets(3, 2, &[(0, 0, 1.0), (1, 1, 1.0)]).unwrap();
    let four = Sparse::from_triplets(4, 2, &[(0, 0, 1.0), (1, 1, 1.0)]).unwrap();
    let symbolic = SymbolicQr::new(&three, &mut work()).unwrap();
    assert_eq!(
        SparseQr::factor(&symbolic, &four, Real::ZERO, &mut work()).err(),
        Some(LinalgError::Shape)
    );
}

#[test]
fn a_dead_column_is_measured_against_the_largest() {
    // The tolerance is relative: 0.5 is more than 10^-3, and less than 10^-3 of 1024.
    let a = Sparse::from_triplets(2, 2, &[(0, 0, 1024.0), (1, 1, 0.5)]).unwrap();
    let symbolic = SymbolicQr::new(&a, &mut work()).unwrap();
    let tol = Real::new(1e-3).unwrap();
    let qr = SparseQr::factor(&symbolic, &a, tol, &mut work()).unwrap();
    assert_eq!(qr.rank(), 1);
}

#[test]
fn a_solve_charges_what_it_reads() {
    let a = Sparse::from_triplets(3, 2, &[(0, 0, 1.0), (1, 0, 2.0), (2, 1, 3.0)]).unwrap();
    let symbolic = SymbolicQr::new(&a, &mut work()).unwrap();
    let qr = SparseQr::factor(&symbolic, &a, Real::ZERO, &mut work()).unwrap();
    assert!(charged_exactly(|w| qr.least_squares(&[1.0, 2.0, 3.0], w)) > 1);
}

/// The sign of det `a`, a square matrix of small whole numbers, by fraction-free elimination
/// (Bareiss): exact in i128 for the sizes used here.
fn exact_sign(a: &Sparse) -> core::cmp::Ordering {
    let dense = a.to_dense().unwrap();
    let n = a.rows();
    let mut m: Vec<Vec<i128>> = (0..n)
        .map(|i| (0..n).map(|j| dense.get(i, j).unwrap() as i128).collect())
        .collect();
    let (mut sign, mut previous) = (1_i128, 1_i128);
    for k in 0..n {
        if m[k][k] == 0 {
            let Some(i) = (k + 1..n).find(|&i| m[i][k] != 0) else {
                return core::cmp::Ordering::Equal;
            };
            m.swap(k, i);
            sign = -sign;
        }
        for i in k + 1..n {
            for j in k + 1..n {
                m[i][j] = (m[i][j] * m[k][k] - m[i][k] * m[k][j]) / previous;
            }
        }
        previous = m[k][k];
    }
    (sign * m[n - 1][n - 1]).cmp(&0)
}

#[test]
fn the_determinant_sign_is_the_exact_one() {
    let mut rng = rng("sparse determinant sign");
    let mut checked = 0;
    for round in 0..400 {
        let n = 1 + round % 8;
        let a = scattered(&mut rng, n, n, 1 + round % 3);
        let symbolic = SymbolicQr::new(&a, &mut work()).unwrap();
        let qr = SparseQr::factor(&symbolic, &a, tol(), &mut work()).unwrap();
        let exact = exact_sign(&a);
        if exact == core::cmp::Ordering::Equal {
            assert_eq!(qr.determinant_sign().unwrap(), None, "round {round}");
        } else if qr.rank() == n {
            assert_eq!(qr.determinant_sign().unwrap(), Some(exact), "round {round}");
            checked += 1;
        }
    }
    assert!(checked >= 200, "{checked}");
    // A swap of two rows turns it, and so does a negative entry; a tall matrix has none.
    let sign = |triplets: &[(usize, usize, f64)], m: usize, n: usize| {
        let a = Sparse::from_triplets(m, n, triplets).unwrap();
        let symbolic = SymbolicQr::new(&a, &mut work()).unwrap();
        SparseQr::factor(&symbolic, &a, tol(), &mut work())
            .unwrap()
            .determinant_sign()
            .unwrap()
    };
    let greater = Some(core::cmp::Ordering::Greater);
    let less = Some(core::cmp::Ordering::Less);
    assert_eq!(sign(&[(0, 0, 1.0), (1, 1, 1.0)], 2, 2), greater);
    assert_eq!(sign(&[(0, 1, 1.0), (1, 0, 1.0)], 2, 2), less);
    assert_eq!(sign(&[(0, 0, -2.0), (1, 1, 3.0)], 2, 2), less);
    assert_eq!(sign(&[(0, 0, 1.0), (1, 0, 1.0)], 2, 1), None);
}
