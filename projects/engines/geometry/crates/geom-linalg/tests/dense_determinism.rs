//! The replay hash of piece G49: the dense decompositions and solves, run over a fixed corpus and
//! hashed with SHA-256, must give the same digest on every target (chapters 21 and 28.13; the
//! golden store of piece G19; the board's standard bar: a piece's cases join the replay corpus).
//!
//! The corpus: 256 matrices from a fixed linear congruential sequence, every shape from 1 × 1 to
//! 8 × 8, of the four kinds of `spectral_determinism.rs` (random entries in [−1, 1), small
//! integers, graded columns down to 2^−600, and exactly rank-deficient integer products). Each goes
//! through `PivotedQr::factor` (its permutation, the diagonal of R and its rank at 10^−10),
//! `Cod::factor` at 10^−10 (rank, permutation, least-squares solution of a seeded right-hand side
//! and null space) and `canonical_null_space`; each square one also through `Lu::factor` (factors,
//! permutation and a solve), `solve_lower` (unit and not) and `solve_upper` on its triangles,
//! `Cholesky::factor` of A^T·A + I (factor and a solve) and of A itself, and `solve_tridiagonal` on
//! its three middle diagonals. Each result is written in a canonical byte encoding (doubles as
//! their bits, shapes and permutations as 64-bit integers, each error as its name), so a target
//! that rounds one step differently, pivots on another row or orders one tie differently changes
//! the digest. Budgets too small to start pin the refusals too. The digest lives in the store; a
//! change is a bug, or a new definition with a golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_linalg::{
    Cholesky, Cod, LinalgError, Lu, Matrix, PivotedQr, Work, canonical_null_space, solve_lower,
    solve_tridiagonal, solve_upper,
};
use geom_num::real::Real;
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-linalg/dense";

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
    fn matrix(&mut self, m: &Matrix) {
        self.size(m.rows());
        self.size(m.cols());
        for i in 0..m.rows() {
            for j in 0..m.cols() {
                self.f64(m.get(i, j).unwrap());
            }
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
    fn int(&mut self, k: i64) -> i64 {
        ((self.next() >> 20) % (2 * k as u64 + 1)) as i64 - k
    }
}

/// The matrix of kind `kind` and shape m × n (as `spectral_determinism.rs` draws them).
fn matrix(rng: &mut Lcg, kind: usize, m: usize, n: usize) -> Matrix {
    let e: Vec<f64> = match kind {
        0 => (0..m * n).map(|_| rng.unit()).collect(),
        1 => (0..m * n).map(|_| rng.int(7) as f64).collect(),
        2 => (0..m * n)
            .map(|k| rng.unit() * f64::from_bits((1023 - 85 * (k % n) as u64) << 52))
            .collect(),
        _ => {
            let r = m.min(n).saturating_sub(1).max(1);
            let x: Vec<i64> = (0..m * r).map(|_| rng.int(4)).collect();
            let y: Vec<i64> = (0..n * r).map(|_| rng.int(4)).collect();
            (0..m * n)
                .map(|k| {
                    let (i, j) = (k / n, k % n);
                    (0..r).fold(0, |acc, t| acc + x[i * r + t] * y[j * r + t]) as f64
                })
                .collect()
        }
    };
    Matrix::from_rows(m, n, &e).unwrap()
}

/// A^T·A + I, each entry summed in index order: symmetric positive definite.
fn gram(a: &Matrix) -> Matrix {
    let n = a.cols();
    let mut e = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut s = if i == j { 1.0 } else { 0.0 };
            for k in 0..a.rows() {
                s += a.get(k, i).unwrap() * a.get(k, j).unwrap();
            }
            e[i * n + j] = s;
        }
    }
    Matrix::from_rows(n, n, &e).unwrap()
}

fn unlimited() -> Work {
    Work::new(u64::MAX)
}

fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    let mut rng = Lcg(0x6749);
    let tol = Real::new(1e-10).unwrap();
    for m in 1..=8 {
        for n in 1..=8 {
            for kind in 0..4 {
                let a = matrix(&mut rng, kind, m, n);
                let b: Vec<f64> = (0..m).map(|_| rng.unit()).collect();
                rec.result(PivotedQr::factor(&a, &mut unlimited()), |rec, qr| {
                    rec.sizes(qr.permutation());
                    for k in 0..m.min(n) {
                        rec.f64(qr.r_diagonal(k).unwrap());
                    }
                    rec.size(qr.rank(tol));
                });
                rec.result(Cod::factor(&a, tol, &mut unlimited()), |rec, cod| {
                    rec.size(cod.rank());
                    rec.sizes(cod.permutation());
                    rec.result(cod.least_squares(&b, &mut unlimited()), |rec, x| {
                        rec.vector(&x);
                    });
                    rec.result(cod.null_space(&mut unlimited()), |rec, z| rec.matrix(&z));
                });
                rec.result(canonical_null_space(&a, tol, &mut unlimited()), |rec, z| {
                    rec.matrix(&z);
                });
                if m == n {
                    rec.result(Lu::factor(&a, &mut unlimited()), |rec, lu| {
                        rec.matrix(lu.factors());
                        rec.sizes(lu.permutation());
                        rec.result(lu.solve(&b, &mut unlimited()), |rec, x| rec.vector(&x));
                    });
                    for unit in [false, true] {
                        rec.result(solve_lower(&a, &b, unit, &mut unlimited()), |rec, x| {
                            rec.vector(&x);
                        });
                    }
                    rec.result(solve_upper(&a, &b, &mut unlimited()), |rec, x| {
                        rec.vector(&x);
                    });
                    let g = gram(&a);
                    rec.result(Cholesky::factor(&g, &mut unlimited()), |rec, c| {
                        rec.matrix(c.factor_g());
                        rec.result(c.solve(&b, &mut unlimited()), |rec, x| rec.vector(&x));
                    });
                    rec.result(Cholesky::factor(&a, &mut unlimited()), |rec, c| {
                        rec.matrix(c.factor_g());
                    });
                    let diag: Vec<f64> = (0..n).map(|i| a.get(i, i).unwrap()).collect();
                    let sub: Vec<f64> = (1..n).map(|i| a.get(i, i - 1).unwrap()).collect();
                    let sup: Vec<f64> = (1..n).map(|i| a.get(i - 1, i).unwrap()).collect();
                    rec.result(
                        solve_tridiagonal(&sub, &diag, &sup, &b, &mut unlimited()),
                        |rec, x| rec.vector(&x),
                    );
                }
            }
        }
    }
    // Budgets too small to start, so the refusals are pinned too.
    let a = matrix(&mut rng, 0, 5, 5);
    rec.result(PivotedQr::factor(&a, &mut Work::new(1)), |_, _| {});
    rec.result(Cod::factor(&a, tol, &mut Work::new(1)), |_, _| {});
    rec.result(Lu::factor(&a, &mut Work::new(1)), |_, _| {});
    rec.result(Cholesky::factor(&gram(&a), &mut Work::new(1)), |_, _| {});
    rec
}

#[test]
fn dense_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-linalg dense corpus: {} results, {} bytes, golden hash {}",
        rec.calls,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert!(rec.calls >= 1000, "only {} results", rec.calls);
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different factor, pivot, \
             solution or step count (compare this log's digest with another target's), or a \
             definition changed in place, which chapter 26.2 forbids"
        )
    });
}
