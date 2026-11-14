//! The replay hash of piece G215: the column order and the sparse factorisations and solves, run
//! over a fixed corpus and hashed with SHA-256, must give the same digest on every target (chapters
//! 21 and 28.13; the golden store of piece G19; the board's standard bar: a piece's cases join the
//! replay corpus).
//!
//! The corpus: for each shape from 2 × 2 to 24 × 24, in steps of three, sparse matrices from a fixed
//! linear congruential sequence, each with about three entries a column. Each goes through
//! `colamd`, `SymbolicQr::new` and `SparseQr::factor` at 10^−10 (order, rank and the solution of a
//! seeded right-hand side); each square one's symmetrised sum with a shifted diagonal goes through
//! `symmetric`, `SymbolicCholesky::new`, `SparseCholesky::factor` and `SparseLdlt::factor`. Each
//! result is written in a canonical byte encoding (doubles as their bits, permutations as 64-bit
//! integers, each error as its name), so a target that rounds one step differently, or breaks one
//! tie differently, changes the digest. Budgets too small to start pin the refusals too. The digest
//! lives in the store; a change is a bug, or a new definition with a golden of its own (chapter
//! 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::semicolon_if_nothing_returned,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_linalg::colamd::{colamd, symmetric};
use geom_linalg::{
    LinalgError, Sparse, SparseCholesky, SparseLdlt, SparseQr, SymbolicCholesky, SymbolicQr, Work,
};
use geom_num::real::Real;
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-linalg/sparse";

#[derive(Default)]
struct Recorder {
    bytes: Vec<u8>,
    calls: u32,
}

impl Recorder {
    fn f64(&mut self, v: f64) {
        self.bytes.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    fn size(&mut self, v: usize) {
        self.bytes.extend_from_slice(&(v as u64).to_le_bytes());
    }
    fn sizes(&mut self, vs: &[usize]) {
        self.size(vs.len());
        for &v in vs {
            self.size(v);
        }
    }
    fn vector(&mut self, v: &[f64]) {
        self.size(v.len());
        for &x in v {
            self.f64(x);
        }
    }
    /// One result: 1 and what `ok` writes, or 0 and the error's name.
    fn result<T>(&mut self, r: Result<T, LinalgError>, ok: impl FnOnce(&mut Self, T)) {
        self.calls += 1;
        match r {
            Ok(v) => {
                self.bytes.push(1);
                ok(self, v);
            }
            Err(e) => {
                self.bytes.push(0);
                self.bytes.extend_from_slice(format!("{e:?}").as_bytes());
            }
        }
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }
    fn unit(&mut self) -> f64 {
        (self.next() as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
    fn below(&mut self, n: usize) -> usize {
        ((self.next() >> 20) % n as u64) as usize
    }
}

fn matrix(rng: &mut Lcg, m: usize, n: usize) -> Sparse {
    let mut triplets = Vec::new();
    for j in 0..n {
        for _ in 0..3 {
            triplets.push((rng.below(m), j, rng.unit()));
        }
    }
    Sparse::from_triplets(m, n, &triplets).unwrap()
}

/// A + Aᵀ with `shift` on the diagonal.
fn symmetrised(a: &Sparse, shift: f64) -> Sparse {
    let mut triplets = Vec::new();
    for j in 0..a.cols() {
        let (rows, values) = a.column(j).unwrap();
        for (&i, &value) in rows.iter().zip(values) {
            triplets.push((i, j, value));
            triplets.push((j, i, value));
        }
        triplets.push((j, j, shift));
    }
    Sparse::from_triplets(a.rows(), a.cols(), &triplets).unwrap()
}

fn unlimited() -> Work {
    Work::new(u64::MAX)
}

fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    let mut rng = Lcg(0x0215);
    let tol = Real::new(1e-10).unwrap();
    for m in (2..=24).step_by(3) {
        for n in (2..=24).step_by(3) {
            let a = matrix(&mut rng, m, n);
            let b: Vec<f64> = (0..m).map(|_| rng.unit()).collect();
            rec.result(colamd(&a, &mut unlimited()), |rec, order| rec.sizes(&order));
            rec.result(SymbolicQr::new(&a, &mut unlimited()), |rec, symbolic| {
                rec.sizes(symbolic.order());
                rec.size(symbolic.r_entries());
                rec.result(
                    SparseQr::factor(&symbolic, &a, tol, &mut unlimited()),
                    |rec, qr| {
                        rec.size(qr.rank());
                        rec.result(qr.least_squares(&b, &mut unlimited()), |rec, x| {
                            rec.vector(&x);
                        });
                    },
                );
            });
            if m == n {
                let s = symmetrised(&a, 8.0);
                rec.result(symmetric(&s, &mut unlimited()), |rec, order| {
                    rec.sizes(&order)
                });
                rec.result(
                    SymbolicCholesky::new(&s, &mut unlimited()),
                    |rec, symbolic| {
                        rec.size(symbolic.l_entries());
                        rec.result(
                            SparseCholesky::factor(&symbolic, &s, &mut unlimited()),
                            |rec, l| {
                                rec.result(l.solve(&b, &mut unlimited()), |rec, x| rec.vector(&x));
                            },
                        );
                        rec.result(
                            SparseLdlt::factor(&symbolic, &s, &mut unlimited()),
                            |rec, l| {
                                rec.vector(l.pivots());
                                rec.result(l.solve(&b, &mut unlimited()), |rec, x| rec.vector(&x));
                            },
                        );
                    },
                );
            }
        }
    }
    // Budgets too small to start, so the refusals are pinned too.
    let a = matrix(&mut rng, 5, 5);
    rec.result(colamd(&a, &mut Work::new(1)), |_, _| {});
    rec.result(SymbolicQr::new(&a, &mut Work::new(1)), |_, _| {});
    let symbolic = SymbolicQr::new(&a, &mut unlimited()).unwrap();
    rec.result(
        SparseQr::factor(&symbolic, &a, tol, &mut Work::new(1)),
        |_, _| {},
    );
    rec
}

#[test]
fn sparse_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-linalg sparse corpus: {} results, {} bytes, golden hash {}",
        rec.calls,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert!(rec.calls >= 200, "only {} results", rec.calls);
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different order, factor or \
             solution (compare this log's digest with another target's), or a definition changed \
             in place, which chapter 26.2 forbids"
        )
    });
}
