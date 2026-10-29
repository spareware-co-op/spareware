//! The replay hash of piece G50: both singular value decompositions and the symmetric eigensolver,
//! run over a fixed corpus and hashed with SHA-256, must give the same digest on every target
//! (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus: 256 matrices from a fixed linear congruential sequence, every shape from 1 × 1 to
//! 8 × 8, of four kinds (random entries in [−1, 1), small integers, graded columns down to
//! 2^−600, and exactly rank-deficient integer products), each through `Svd::jacobi` and
//! `Svd::golub_kahan`, and the symmetric part of every square one through
//! `SymmetricEigen::factor`. Each result is written in a canonical byte encoding (doubles as their
//! bits, each error as its name), so a target that rounds one step differently, takes one more
//! sweep or QR step, or orders one tie differently changes the digest. The digest lives in the
//! store; a change is a bug, or a new definition with a golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::manual_midpoint,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_linalg::{LinalgError, Matrix, Svd, SymmetricEigen, Work};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-linalg/spectral";

#[derive(Default)]
struct Recorder {
    bytes: Vec<u8>,
    calls: u32,
}

impl Recorder {
    fn f64(&mut self, v: f64) {
        self.bytes.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    fn matrix(&mut self, m: &Matrix) {
        for i in 0..m.rows() {
            for j in 0..m.cols() {
                self.f64(m.get(i, j).unwrap());
            }
        }
    }
    fn error(&mut self, e: LinalgError) {
        self.bytes.extend_from_slice(format!("{e:?}").as_bytes());
    }
    fn svd(&mut self, r: Result<Svd, LinalgError>) {
        self.calls += 1;
        match r {
            Ok(s) => {
                self.bytes.push(1);
                for &x in s.singular_values() {
                    self.f64(x);
                }
                self.matrix(s.u());
                self.matrix(s.v());
            }
            Err(e) => {
                self.bytes.push(0);
                self.error(e);
            }
        }
    }
    fn eigen(&mut self, r: Result<SymmetricEigen, LinalgError>) {
        self.calls += 1;
        match r {
            Ok(s) => {
                self.bytes.push(1);
                for &x in s.values() {
                    self.f64(x);
                }
                self.matrix(s.vectors());
            }
            Err(e) => {
                self.bytes.push(0);
                self.error(e);
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

/// The matrix of kind `kind` and shape m × n.
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

fn corpus() -> Recorder {
    let mut rec = Recorder::default();
    let mut rng = Lcg(0x6750);
    for m in 1..=8 {
        for n in 1..=8 {
            for kind in 0..4 {
                let a = matrix(&mut rng, kind, m, n);
                rec.svd(Svd::jacobi(&a, &mut Work::new(u64::MAX)));
                rec.svd(Svd::golub_kahan(&a, &mut Work::new(u64::MAX)));
                if m == n {
                    // The symmetric part, (A + Aᵀ)/2, exactly symmetric.
                    let mut e = vec![0.0; n * n];
                    for i in 0..n {
                        for j in 0..=i {
                            let v = (a.get(i, j).unwrap() + a.get(j, i).unwrap()) / 2.0;
                            e[i * n + j] = v;
                            e[j * n + i] = v;
                        }
                    }
                    let s = Matrix::from_rows(n, n, &e).unwrap();
                    rec.eigen(SymmetricEigen::factor(&s, &mut Work::new(u64::MAX)));
                }
            }
        }
    }
    // Budgets that run out, so the refusals are pinned too.
    let a = matrix(&mut rng, 0, 5, 4);
    rec.svd(Svd::jacobi(&a, &mut Work::new(100)));
    rec.svd(Svd::golub_kahan(&a, &mut Work::new(100)));
    rec
}

#[test]
fn spectral_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-linalg spectral corpus: {} results, {} bytes, golden hash {}",
        rec.calls,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert!(rec.calls >= 290, "only {} results", rec.calls);
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes a different singular value, \
             vector or step count (compare this log's digest with another target's), or a \
             definition changed in place, which chapter 26.2 forbids"
        )
    });
}
