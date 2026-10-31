//! Tests of `geom_linalg`'s singular value decompositions and symmetric eigensolver (piece G50)
//! against an exact rational oracle (dashu).
//!
//! What this file proves: on random, integer, graded and rank-deficient matrices of every shape
//! up to 7 × 7, both SVDs leave a residual A − U·Σ·Vᵀ and orthogonality defects UᵀU − I and
//! VᵀV − I within small multiples of the unit roundoff, checked exactly; every computed singular
//! value σ̂ᵢ brackets the exact i-th singular value, decided by exact inertia counts of Aᵀ·A − x·I
//! (Sylvester's law of inertia): within a small multiple of u·‖A‖ for both methods, and, on graded
//! matrices A = B·D with B well conditioned, within a small relative error for Jacobi however
//! small the value; exact rank deficiency gives singular values at rounding level and the exact
//! rank; and every method charges its work and refuses a budget that runs out.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::needless_range_loop,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    reason = "oracles and generators, indexed as the formulas are; the rules, chapter 21's bans included, are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;
use geom_linalg::{LinalgError, Matrix, Svd, SymmetricEigen, Work};
use geom_num::real::Real;

fn plenty() -> Work {
    Work::new(u64::MAX)
}

/// u = 2^−53, exactly.
fn unit() -> RBig {
    RBig::from_parts(IBig::ONE, UBig::ONE << 53)
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
    /// A double in [−1, 1).
    fn unit(&mut self) -> f64 {
        (self.next() as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
    fn int(&mut self, k: i64) -> i64 {
        ((self.next() >> 20) % (2 * k as u64 + 1)) as i64 - k
    }
}

fn exact(x: f64) -> RBig {
    RBig::try_from(x).unwrap()
}

fn abs(x: &RBig) -> RBig {
    if *x < RBig::ZERO {
        -x.clone()
    } else {
        x.clone()
    }
}

fn ex(m: &Matrix) -> Vec<Vec<RBig>> {
    (0..m.rows())
        .map(|i| (0..m.cols()).map(|j| exact(m.get(i, j).unwrap())).collect())
        .collect()
}

/// The test matrices: (name, matrix, graded) with graded the column scales of A = B·D if any.
fn matrices() -> Vec<(String, Matrix, Option<Vec<f64>>)> {
    let mut rng = Lcg(50);
    let mut out = Vec::new();
    let shapes = [
        (1, 1),
        (2, 2),
        (3, 2),
        (2, 3),
        (4, 4),
        (5, 3),
        (3, 5),
        (6, 6),
        (7, 4),
        (4, 7),
    ];
    for &(m, n) in &shapes {
        let e: Vec<f64> = (0..m * n).map(|_| rng.unit()).collect();
        out.push((
            format!("random {m}x{n}"),
            Matrix::from_rows(m, n, &e).unwrap(),
            None,
        ));
        let e: Vec<f64> = (0..m * n).map(|_| rng.int(9) as f64).collect();
        out.push((
            format!("integer {m}x{n}"),
            Matrix::from_rows(m, n, &e).unwrap(),
            None,
        ));
        // Rank-deficient: X·Yᵀ with X m × r and Y n × r, integer, r < min(m, n).
        let k = m.min(n);
        if k >= 2 {
            let r = k - 1;
            let x: Vec<i64> = (0..m * r).map(|_| rng.int(5)).collect();
            let y: Vec<i64> = (0..n * r).map(|_| rng.int(5)).collect();
            let e: Vec<f64> = (0..m)
                .flat_map(|i| (0..n).map(move |j| (i, j)))
                .map(|(i, j)| (0..r).map(|t| x[i * r + t] * y[j * r + t]).sum::<i64>() as f64)
                .collect();
            out.push((
                format!("rank {r} {m}x{n}"),
                Matrix::from_rows(m, n, &e).unwrap(),
                None,
            ));
        }
        // Graded: B (well conditioned: identity plus a small perturbation) times column scales
        // from 1 down to 2^−600.
        if m >= n {
            let d: Vec<f64> = (0..n)
                .map(|j| f64::from_bits((1023 - 120 * j as u64) << 52))
                .collect();
            let e: Vec<f64> = (0..m)
                .flat_map(|i| (0..n).map(move |j| (i, j)))
                .map(|(i, j)| {
                    let b = if i == j { 1.0 } else { 0.0 } + 0.25 * rng.unit();
                    b * d[j]
                })
                .collect();
            out.push((
                format!("graded {m}x{n}"),
                Matrix::from_rows(m, n, &e).unwrap(),
                Some(d),
            ));
            // The same grading reversed, and rows graded too: A = D₁·B·D₂.
            let d2: Vec<f64> = (0..n)
                .map(|j| f64::from_bits((1023 - 120 * (n - 1 - j) as u64) << 52))
                .collect();
            let d1: Vec<f64> = (0..m)
                .map(|i| f64::from_bits((1023 - 40 * i as u64) << 52))
                .collect();
            let e: Vec<f64> = (0..m)
                .flat_map(|i| (0..n).map(move |j| (i, j)))
                .map(|(i, j)| {
                    let b = if i == j { 1.0 } else { 0.0 } + 0.25 * rng.unit();
                    d1[i] * b * d2[j]
                })
                .collect();
            out.push((
                format!("two-sided graded {m}x{n}"),
                Matrix::from_rows(m, n, &e).unwrap(),
                None,
            ));
            let e: Vec<f64> = (0..m)
                .flat_map(|i| (0..n).map(move |j| (i, j)))
                .map(|(i, j)| {
                    let b = if i == j { 1.0 } else { 0.0 } + 0.25 * rng.unit();
                    b * d2[j]
                })
                .collect();
            out.push((
                format!("reverse graded {m}x{n}"),
                Matrix::from_rows(m, n, &e).unwrap(),
                Some(d2),
            ));
        }
    }
    out
}

/// The number of eigenvalues of the symmetric rational matrix s below x, by Sylvester's law of
/// inertia: the negative pivots of an LDLᵀ factorisation of s − x·I. A zero pivot moves x down by
/// a hair (2^−300 of its scale), which no test bracket is near.
fn below(s: &[Vec<RBig>], x: &RBig) -> usize {
    let n = s.len();
    let mut shift = x.clone();
    'retry: loop {
        let mut a: Vec<Vec<RBig>> = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| {
                        if i == j {
                            &s[i][j] - &shift
                        } else {
                            s[i][j].clone()
                        }
                    })
                    .collect()
            })
            .collect();
        let mut count = 0;
        for k in 0..n {
            let p = a[k][k].clone();
            if p == RBig::ZERO {
                let hair = RBig::from_parts(IBig::ONE, UBig::ONE << 300);
                shift = &shift - (abs(&shift) + RBig::ONE) * hair;
                continue 'retry;
            }
            if p < RBig::ZERO {
                count += 1;
            }
            for i in k + 1..n {
                let f = &a[i][k] / &p;
                for j in k + 1..n {
                    let v = &a[i][j] - &f * &a[k][j];
                    a[i][j] = v;
                }
            }
        }
        return count;
    }
}

/// Aᵀ·A (n × n) or A·Aᵀ, whichever is smaller, exactly: its eigenvalues are the squared singular
/// values (and zeros).
fn gram(a: &[Vec<RBig>]) -> Vec<Vec<RBig>> {
    let (m, n) = (a.len(), a[0].len());
    if n <= m {
        (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| (0..m).fold(RBig::ZERO, |acc, k| acc + &a[k][i] * &a[k][j]))
                    .collect()
            })
            .collect()
    } else {
        (0..m)
            .map(|i| {
                (0..m)
                    .map(|j| (0..n).fold(RBig::ZERO, |acc, k| acc + &a[i][k] * &a[j][k]))
                    .collect()
            })
            .collect()
    }
}

/// The i-th largest exact singular value lies in [lo, hi] (lo ≥ 0), decided by inertia counts.
fn brackets(g: &[Vec<RBig>], i: usize, lo: &RBig, hi: &RBig) -> bool {
    let k = g.len();
    // #σ ≥ lo is k − #eig below lo²; #σ > hi is k − #eig below or at hi² (counted just above).
    let at_least_lo = k - below(g, &(lo * lo));
    let above_hi = k - below(
        g,
        &(hi * hi + RBig::from_parts(IBig::ONE, UBig::ONE << 400)),
    );
    at_least_lo > i && above_hi <= i
}

/// ‖A − U·Σ·Vᵀ‖_max and the largest |UᵀU − I| and |VᵀV − I|, exactly.
fn defects(a: &[Vec<RBig>], svd: &Svd) -> (RBig, RBig) {
    let (u, v) = (ex(svd.u()), ex(svd.v()));
    let s: Vec<RBig> = svd.singular_values().iter().map(|&x| exact(x)).collect();
    let (m, n, k) = (a.len(), a[0].len(), s.len());
    let mut res = RBig::ZERO;
    for i in 0..m {
        for j in 0..n {
            let mut r = a[i][j].clone();
            for t in 0..k {
                r -= &u[i][t] * &s[t] * &v[j][t];
            }
            if abs(&r) > res {
                res = abs(&r);
            }
        }
    }
    let mut orth = RBig::ZERO;
    for f in [&u, &v] {
        let rows = f.len();
        for p in 0..k {
            for q in 0..k {
                let mut d = (0..rows).fold(RBig::ZERO, |acc, i| acc + &f[i][p] * &f[i][q]);
                if p == q {
                    d -= RBig::ONE;
                }
                if abs(&d) > orth {
                    orth = abs(&d);
                }
            }
        }
    }
    (res, orth)
}

fn max_abs(a: &[Vec<RBig>]) -> RBig {
    a.iter()
        .flatten()
        .fold(RBig::ZERO, |m, x| if abs(x) > m { abs(x) } else { m })
}

type Method = fn(&Matrix, &mut Work) -> Result<Svd, LinalgError>;

const METHODS: [(&str, Method); 2] = [("Jacobi", Svd::jacobi), ("Golub-Kahan", Svd::golub_kahan)];

#[test]
fn both_svds_are_backward_stable_and_bracket_the_exact_values() {
    for (name, a, _) in matrices() {
        let ae = ex(&a);
        let (m, n) = (a.rows(), a.cols());
        let scale = max_abs(&ae);
        let g = gram(&ae);
        let p = RBig::from((m + n) as i64);
        for (method, f) in METHODS {
            let svd = f(&a, &mut plenty()).unwrap();
            let s = svd.singular_values();
            assert_eq!(s.len(), m.min(n));
            assert!(
                s.windows(2).all(|w| w[0] >= w[1]),
                "{method} {name}: not sorted"
            );
            assert!(s.iter().all(|&x| x >= 0.0));
            let (res, orth) = defects(&ae, &svd);
            assert!(
                res <= &p * &p * unit() * &scale * RBig::from(8),
                "{method} {name}: residual {res}"
            );
            assert!(
                orth <= &p * &p * unit() * RBig::from(8),
                "{method} {name}: orthogonality {orth}"
            );
            // Each singular value within 8(m + n)²·u·‖A‖_max·√(mn) of the exact one.
            let delta = &p * &p * unit() * &scale * RBig::from(8 * (m * n) as i64);
            for (i, &x) in s.iter().enumerate() {
                let lo = exact(x) - &delta;
                let lo = if lo < RBig::ZERO { RBig::ZERO } else { lo };
                assert!(
                    brackets(&g, i, &lo, &(exact(x) + &delta)),
                    "{method} {name}: σ{i} = {x}"
                );
            }
        }
    }
}

#[test]
fn jacobi_keeps_relative_accuracy_on_graded_matrices() {
    for (name, a, graded) in matrices() {
        if graded.is_none() {
            continue;
        }
        let g = gram(&ex(&a));
        let svd = Svd::jacobi(&a, &mut plenty()).unwrap();
        for (i, &x) in svd.singular_values().iter().enumerate() {
            // Within a relative 2^−40 however small: the values span 2^−600 to 1.
            let rel = exact(x) * RBig::from_parts(IBig::ONE, UBig::ONE << 40);
            assert!(
                brackets(&g, i, &(exact(x) - &rel), &(exact(x) + &rel)),
                "Jacobi {name}: σ{i} = {x}"
            );
        }
    }
}

#[test]
fn exact_rank_deficiency_shows() {
    for (name, a, _) in matrices()
        .into_iter()
        .filter(|(n, _, _)| n.starts_with("rank"))
    {
        let r: usize = name.split(' ').nth(1).unwrap().parse().unwrap();
        for (method, f) in METHODS {
            let svd = f(&a, &mut plenty()).unwrap();
            assert_eq!(svd.rank(Real::new(1e-12).unwrap()), r, "{method} {name}");
            let first = svd.singular_values()[0];
            for &x in &svd.singular_values()[r..] {
                assert!(x <= 1e-13 * first, "{method} {name}: {x}");
            }
            let verdict = svd
                .classify(
                    Real::new(1e-6 * first).unwrap(),
                    Real::new(1e-12 * first).unwrap(),
                )
                .unwrap();
            assert_eq!(
                (
                    verdict.independent,
                    verdict.near_singular,
                    verdict.dependent
                ),
                (r, 0, svd.singular_values().len() - r)
            );
        }
    }
}

#[test]
fn zero_and_tiny_matrices() {
    for (m, n) in [(1, 1), (3, 2), (2, 3), (4, 4)] {
        let a = Matrix::zeros(m, n).unwrap();
        for (_, f) in METHODS {
            let svd = f(&a, &mut plenty()).unwrap();
            assert!(svd.singular_values().iter().all(|&x| x == 0.0));
            let (_, orth) = defects(&ex(&a), &svd);
            assert!(orth <= RBig::from_parts(IBig::ONE, UBig::ONE << 48));
        }
    }
}

#[test]
fn budgets_are_charged() {
    let a = Matrix::from_rows(3, 3, &[4.0, 1.0, 2.0, 1.0, 3.0, 0.5, 2.0, 0.5, 5.0]).unwrap();
    for (_, f) in METHODS {
        assert_eq!(f(&a, &mut Work::new(10)), Err(LinalgError::OutOfWork));
    }
    assert!(
        Svd::jacobi(&a, &mut plenty())
            .unwrap()
            .classify(Real::new(1.0).unwrap(), Real::new(2.0).unwrap())
            .is_err()
    );
}

#[test]
fn the_oracle_refuses_wrong_values() {
    // diag(3, 2, 1): the brackets must hold the true values and refuse values one part in 10^9 off.
    let a = Matrix::from_rows(3, 3, &[3.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 1.0]).unwrap();
    let g = gram(&ex(&a));
    let tight = RBig::from_parts(IBig::ONE, UBig::from(1_000_000_000_000u64));
    for (i, x) in [3, 2, 1].into_iter().enumerate() {
        let x = RBig::from(x);
        assert!(brackets(&g, i, &(&x - &tight), &(&x + &tight)));
        let off = &x * RBig::from_parts(IBig::from(1_000_000_001), UBig::from(1_000_000_000u64));
        assert!(!brackets(&g, i, &(&off - &tight), &(&off + &tight)));
    }
}

#[test]
fn golub_kahan_is_only_absolutely_accurate_on_graded_matrices() {
    // The reason Jacobi is here: on the graded matrices, Golub–Kahan's smallest singular values
    // are accurate to u·‖A‖ but not to a relative 2^−40 (Demmel and Veselić, section 2).
    let mut misses = 0;
    for (_, a, graded) in matrices() {
        if graded.is_none() || a.cols() < 3 {
            continue;
        }
        let g = gram(&ex(&a));
        let svd = Svd::golub_kahan(&a, &mut plenty()).unwrap();
        for (i, &x) in svd.singular_values().iter().enumerate() {
            let rel = exact(x) * RBig::from_parts(IBig::ONE, UBig::ONE << 40);
            if !brackets(&g, i, &(exact(x) - &rel), &(exact(x) + &rel)) {
                misses += 1;
            }
        }
    }
    assert!(
        misses > 0,
        "Golub-Kahan was relatively accurate on every graded matrix"
    );
}

/// Symmetric test matrices: random, integer, with repeated eigenvalues, graded and singular.
fn symmetric() -> Vec<(String, Matrix)> {
    let mut rng = Lcg(51);
    let mut out = Vec::new();
    for n in 1..=7 {
        let mut e = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..=i {
                let v = rng.unit();
                e[i * n + j] = v;
                e[j * n + i] = v;
            }
        }
        out.push((format!("random {n}"), Matrix::from_rows(n, n, &e).unwrap()));
        let mut e = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..=i {
                let v = rng.int(6) as f64;
                e[i * n + j] = v;
                e[j * n + i] = v;
            }
        }
        out.push((format!("integer {n}"), Matrix::from_rows(n, n, &e).unwrap()));
        // The all-ones matrix: eigenvalues n (once) and 0 (n − 1 times).
        out.push((
            format!("ones {n}"),
            Matrix::from_rows(n, n, &vec![1.0; n * n]).unwrap(),
        ));
        // Graded: D·B·D with D = diag(1, 2^−20, 2^−40, …).
        let d: Vec<f64> = (0..n)
            .map(|i| f64::from_bits((1023 - 20 * i as u64) << 52))
            .collect();
        let mut e = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..=i {
                let b = if i == j { 2.0 } else { 0.0 } + 0.5 * rng.unit();
                let v = d[i] * b * d[j];
                e[i * n + j] = v;
                e[j * n + i] = v;
            }
        }
        out.push((format!("graded {n}"), Matrix::from_rows(n, n, &e).unwrap()));
    }
    out
}

#[test]
fn the_eigensolver_is_backward_stable_and_brackets_the_exact_values() {
    for (name, a) in symmetric() {
        let n = a.rows();
        let ae = ex(&a);
        let scale = max_abs(&ae);
        let eig = SymmetricEigen::factor(&a, &mut plenty()).unwrap();
        let (vals, q) = (eig.values(), ex(eig.vectors()));
        assert!(vals.windows(2).all(|w| w[0] <= w[1]), "{name}: not sorted");
        let p = RBig::from((2 * n) as i64);
        let tol = &p * &p * unit() * RBig::from(8);
        // A·Q − Q·Λ and QᵀQ − I, exactly.
        let mut res = RBig::ZERO;
        let mut orth = RBig::ZERO;
        for i in 0..n {
            for k in 0..n {
                let mut r = -(&q[i][k] * exact(vals[k]));
                for j in 0..n {
                    r += &ae[i][j] * &q[j][k];
                }
                if abs(&r) > res {
                    res = abs(&r);
                }
                let mut d = (0..n).fold(RBig::ZERO, |acc, t| acc + &q[t][i] * &q[t][k]);
                if i == k {
                    d -= RBig::ONE;
                }
                if abs(&d) > orth {
                    orth = abs(&d);
                }
            }
        }
        assert!(res <= &tol * &scale, "{name}: residual {res}");
        assert!(orth <= tol, "{name}: orthogonality {orth}");
        // Eigenvalue i of A lies within δ of λ̂ᵢ, by inertia counts of A − x·I.
        let delta = &tol * &scale * RBig::from(n as i64);
        for (i, &x) in vals.iter().enumerate() {
            let lo = exact(x) - &delta;
            let hi = exact(x) + &delta + RBig::from_parts(IBig::ONE, UBig::ONE << 400);
            assert!(
                below(&ae, &hi) > i && below(&ae, &lo) <= i,
                "{name}: λ{i} = {x}"
            );
        }
    }
}

#[test]
fn the_eigensolver_refuses_what_is_not_symmetric() {
    let a = Matrix::from_rows(2, 2, &[1.0, 2.0, 2.000_000_000_000_000_4, 1.0]).unwrap();
    assert_eq!(
        SymmetricEigen::factor(&a, &mut plenty()),
        Err(LinalgError::NotSymmetric)
    );
    let b = Matrix::from_rows(2, 3, &[1.0; 6]).unwrap();
    assert_eq!(
        SymmetricEigen::factor(&b, &mut plenty()),
        Err(LinalgError::Shape)
    );
    let c = Matrix::from_rows(3, 3, &[2.0, 1.0, 0.0, 1.0, 2.0, 1.0, 0.0, 1.0, 2.0]).unwrap();
    assert_eq!(
        SymmetricEigen::factor(&c, &mut Work::new(10)),
        Err(LinalgError::OutOfWork)
    );
}

/// A fuzz finding (Gate 2's `linalg` target): the QR step scales its block to magnitude 1, which
/// here takes a pair of entries near 1e−127, beside 1e184, into the subnormal range. Rotations built
/// there were no unit pairs, and Q drifted from orthogonal by over 3,000 units of roundoff.
#[test]
fn rotations_stay_unit_where_the_step_scale_makes_a_pair_subnormal() {
    let a: [f64; 25] = [
        2.887_395_615_532_085_7e-143,
        0.000_122_070_312_5,
        -1.0,
        -9.399_359_901_127_183e183,
        524_288.0,
        0.000_122_070_312_5,
        0.0,
        -1.214_611_945_415_162_5e-127,
        -0.5,
        0.0,
        -1.0,
        -1.214_611_945_415_162_5e-127,
        0.0,
        0.0,
        0.0,
        -9.399_359_901_127_183e183,
        -0.5,
        0.0,
        0.0,
        0.0,
        524_288.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ];
    let m = Matrix::from_rows(5, 5, &a).unwrap();
    let eig = SymmetricEigen::factor(&m, &mut Work::new(1 << 40)).unwrap();
    let q = eig.vectors();
    for i in 0..5 {
        for j in 0..5 {
            let dot: f64 = (0..5)
                .map(|k| q.get(k, i).unwrap() * q.get(k, j).unwrap())
                .sum();
            let want = if i == j { 1.0 } else { 0.0 };
            assert!((dot - want).abs() < 2e-15, "{i} {j}: {dot}");
        }
    }
}
