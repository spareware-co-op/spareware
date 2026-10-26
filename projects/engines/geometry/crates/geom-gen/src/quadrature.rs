//! Generator `quadrature-rules`: the Gauss–Legendre and Gauss–Kronrod rules on [−1, 1], computed
//! exactly and each node and weight rounded once to the nearest `f64` (piece G51; chapters 12 and
//! 29.4).
//!
//! What it does: writes `geom-num`'s `src/quadrature_rules/mod.rs`, the tables
//! `geom_num::quadrature` integrates with: the Gauss–Legendre rules of 1 to [`GAUSS_MAX`] points,
//! and the Gauss–Kronrod rules of 2n + 1 points that extend the n-point Gauss rule for each n of
//! [`KRONROD_N`] (QUADPACK's six). Every number is computed in exact rational arithmetic and
//! rounded to the nearest `f64`, ties to even, only once an exact enclosure of it lies inside one
//! rounding cell, so each entry is the correctly rounded value of the true node or weight.
//!
//! - **Gauss nodes and weights, by Golub and Welsch.** The n Gauss nodes are the eigenvalues of
//!   the Jacobi matrix of the Legendre weight, whose leading principal minors are the monic
//!   Legendre polynomials P₀, …, Pₙ (Pₖ₊₁ = x·Pₖ − bₖ·Pₖ₋₁, bₖ = k²/(4k² − 1)). They are isolated
//!   and narrowed by bisection on the Sturm count of that sequence, the number of eigenvalues
//!   above a point (Golub and Van Loan, section 8.4.1). The weight of a node x is μ₀ times the
//!   squared first component of its normalised eigenvector, 1/Σₖ Pₖ(x)²/(μ₀·b₁⋯bₖ) with μ₀ = 2:
//!   that eigenvector's components are the orthonormal polynomials at x (Golub and Welsch 1969).
//! - **Kronrod nodes and weights, from their definition.** The n + 1 nodes Kronrod adds are the
//!   zeros of the Stieltjes polynomial Eₙ₊₁, the monic polynomial of degree n + 1 orthogonal to
//!   every polynomial of degree at most n against the weight Pₙ on [−1, 1]: its coefficients
//!   solve those orthogonality conditions exactly. They interlace with the Gauss nodes (Szegő
//!   1935), which isolates each one. The 2n + 1 weights are the interpolatory ones,
//!   w(t) = R(t)/Q′(t) with Q = Pₙ·Eₙ₊₁ and R(t) = ∫ (Q(x) − Q(t))/(x − t) dx, a polynomial in t.
//!   (Laurie's construction of the Jacobi–Kronrod matrix, which would let Golub and Welsch give
//!   these too, was not available to read; the interpolatory weights are the definition of the
//!   rule, exact here, and the tests check that they give the Gauss weights from Golub and
//!   Welsch's formula when Q = Pₙ.)
//! - **Enclosures.** A node is kept as a bracket of two dyadic rationals with a sign change (or as
//!   an exact zero). A weight is enclosed over the node's bracket from its value at one end and a
//!   bound on its derivative there (the sum of |j·cⱼ| bounds a polynomial's derivative on
//!   [−1, 1]). The bracket is halved until both the node's and the weight's enclosures round to one
//!   `f64`.
//!
//! Guarantees:
//!
//! - **Correctly rounded.** Every node and weight is the `f64` nearest to the exact value, ties to
//!   even (dashu-ratio's `to_f64`, which its documentation guarantees correctly rounded, and the
//!   unit tests check against both neighbours), decided from an exact enclosure, never from an
//!   approximation.
//! - **Exact or nothing.** Every step is exact rational arithmetic; a bracket that does not round
//!   to one value within [`MAX_HALVINGS`] halvings, or an isolation that fails, is an error, never
//!   an approximate table.
//! - **Pure.** No input but the constants here; the same bytes on every target.
//!
//! Method: G. H. Golub and J. H. Welsch, *Calculation of Gauss quadrature rules*, Mathematics of
//! Computation 23 (1969), <https://doi.org/10.1090/S0025-5718-69-99647-1>; G. H. Golub and C. F.
//! Van Loan, *Matrix Computations*, 4th ed., Johns Hopkins (2013), section 8.4.1 (bisection by
//! Sturm count); A. S. Kronrod, *Nodes and Weights of Quadrature Formulas*, Consultants Bureau
//! (1965); G. Szegő, *Über gewisse orthogonale Polynome, die zu einer oszillierenden
//! Belegungsfunktion gehören*, Mathematische Annalen 110 (1935),
//! <https://doi.org/10.1007/BF01448041> (the interlacing); the rule set is QUADPACK's
//! (R. Piessens et al., *QUADPACK*, Springer (1983), <https://doi.org/10.1007/978-3-642-61786-7>).
//!
//! Cost: a few seconds natively: for every node, a few hundred halvings, each an exact
//! evaluation of a polynomial of degree at most 2n + 1 ≤ 61 at a dyadic point.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "exact rationals and big integers (dashu) cannot overflow; every index and size here is               a rule's point count, at most 61, or a position below it by construction; the               single-letter names are the formulas' (Golub and Welsch, Kronrod, interval bounds)"
)]

use core::fmt::Write as _;

use dashu_int::ops::Gcd as _;
use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;

use crate::generator::GenError;

/// The generator's name.
pub const NAME: &str = "quadrature-rules";

/// The file it writes: a generated module of `geom-num`.
pub const OUTPUT: &str = "projects/engines/geometry/crates/geom-num/src/quadrature_rules/mod.rs";

/// The folder it owns.
pub const OUTPUT_DIR: &str = "projects/engines/geometry/crates/geom-num/src/quadrature_rules";

/// The Gauss–Legendre rules written: 1 to this many points.
pub const GAUSS_MAX: usize = 20;

/// The Gauss rules extended by Kronrod's: QUADPACK's six (its G7K15 to G30K61).
pub const KRONROD_N: [usize; 6] = [7, 10, 15, 20, 25, 30];

/// The most halvings of a node's bracket before its enclosures must round to one `f64`; far more
/// than the 53-bit significand and the weights' conditioning need.
pub const MAX_HALVINGS: u32 = 400;

fn fail<T>(message: impl Into<String>) -> Result<T, GenError> {
    Err(GenError::generator(NAME, message.into()))
}

/// A polynomial with rational coefficients, constant term first.
#[derive(Clone, Debug, PartialEq)]
struct Poly(Vec<RBig>);

impl Poly {
    fn constant(c: RBig) -> Poly {
        Poly(vec![c])
    }

    /// x · self.
    fn times_x(&self) -> Poly {
        let mut c = vec![RBig::ZERO];
        c.extend(self.0.iter().cloned());
        Poly(c)
    }

    fn scaled(&self, k: &RBig) -> Poly {
        Poly(self.0.iter().map(|c| c * k).collect())
    }

    fn sub(&self, other: &Poly) -> Poly {
        let n = self.0.len().max(other.0.len());
        let at = |p: &Poly, i: usize| p.0.get(i).cloned().unwrap_or(RBig::ZERO);
        Poly((0..n).map(|i| at(self, i) - at(other, i)).collect())
    }

    fn add(&self, other: &Poly) -> Poly {
        let n = self.0.len().max(other.0.len());
        let at = |p: &Poly, i: usize| p.0.get(i).cloned().unwrap_or(RBig::ZERO);
        Poly((0..n).map(|i| at(self, i) + at(other, i)).collect())
    }

    fn mul(&self, other: &Poly) -> Poly {
        let mut c = vec![RBig::ZERO; self.0.len() + other.0.len() - 1];
        for (i, a) in self.0.iter().enumerate() {
            for (j, b) in other.0.iter().enumerate() {
                if let Some(slot) = c.get_mut(i + j) {
                    *slot = &*slot + a * b;
                }
            }
        }
        Poly(c)
    }

    fn derivative(&self) -> Poly {
        let c: Vec<RBig> = self
            .0
            .iter()
            .enumerate()
            .skip(1)
            .map(|(j, c)| c * RBig::from(j as u64))
            .collect();
        if c.is_empty() {
            Poly::constant(RBig::ZERO)
        } else {
            Poly(c)
        }
    }

    /// Σ j·|cⱼ|: a bound on |p′| over [−1, 1].
    fn slope_bound(&self) -> RBig {
        self.0
            .iter()
            .enumerate()
            .skip(1)
            .fold(RBig::ZERO, |acc, (j, c)| {
                acc + abs(c) * RBig::from(j as u64)
            })
    }
}

/// A polynomial as integer coefficients over one positive denominator, for exact evaluation at
/// dyadic points with integers only.
#[derive(Clone, Debug)]
struct IntPoly {
    c: Vec<IBig>,
    den: UBig,
}

impl IntPoly {
    fn new(p: &Poly) -> IntPoly {
        let den = p.0.iter().fold(UBig::ONE, |l, c| {
            let d = c.denominator();
            let g = (&l).gcd(d);
            &l / &g * d
        });
        let c =
            p.0.iter()
                .map(|c| c.numerator() * IBig::from(&den / c.denominator()))
                .collect();
        IntPoly { c, den }
    }

    /// The numerator V and the power of two k with p(x) = V / (den · 2^k), for a dyadic x.
    fn scaled_at(&self, x: &RBig) -> (IBig, usize) {
        let m = x.numerator();
        let s = x.denominator().trailing_zeros().unwrap_or(0);
        let d = self.c.len().saturating_sub(1);
        // Horner on p(m/2^s)·2^(s·d) = Σ cⱼ mʲ 2^(s(d−j)).
        let acc = self
            .c
            .iter()
            .rev()
            .enumerate()
            .fold(IBig::ZERO, |acc, (i, c)| acc * m + (c << (s * i)));
        (acc, s * d)
    }

    fn sign_at(&self, x: &RBig) -> i8 {
        let (v, _) = self.scaled_at(x);
        sign_int(&v)
    }

    fn at(&self, x: &RBig) -> RBig {
        let (v, k) = self.scaled_at(x);
        RBig::from_parts(v, &self.den << k)
    }
}

fn sign_int(x: &IBig) -> i8 {
    match x.cmp(&IBig::ZERO) {
        core::cmp::Ordering::Less => -1,
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Greater => 1,
    }
}

fn abs(x: &RBig) -> RBig {
    if *x < RBig::ZERO {
        -x.clone()
    } else {
        x.clone()
    }
}

fn ratio(num: i64, den: u64) -> RBig {
    RBig::from_parts(IBig::from(num), UBig::from(den))
}

/// ∫₋₁¹ xⁱ dx: 2/(i + 1) for even i, 0 for odd.
fn moment(i: usize) -> RBig {
    if i % 2 == 0 {
        ratio(2, i as u64 + 1)
    } else {
        RBig::ZERO
    }
}

/// bₖ = k²/(4k² − 1), the recurrence coefficient of the monic Legendre polynomials.
fn b(k: usize) -> RBig {
    let k = k as u64;
    RBig::from_parts(IBig::from(k * k), UBig::from(4 * k * k - 1))
}

/// The monic Legendre polynomials P₀, …, Pₙ.
fn legendre(n: usize) -> Vec<Poly> {
    let mut p = vec![Poly::constant(RBig::ONE), Poly(vec![RBig::ZERO, RBig::ONE])];
    for k in 1..n {
        let next = p[k].times_x().sub(&p[k - 1].scaled(&b(k)));
        p.push(next);
    }
    p.truncate(n + 1);
    p
}

/// The number of zeros of Pₙ above x: the sign changes of P₀(x), …, Pₙ(x), zeros dropped (the
/// Sturm count of the Jacobi matrix, Golub and Van Loan 8.4.1), evaluated by the recurrence.
fn zeros_above(n: usize, x: &RBig) -> usize {
    let mut prev = RBig::ONE;
    let mut cur = x.clone();
    let mut changes = 0;
    let mut last_sign = 1i8;
    for k in 1..=n {
        if k > 1 {
            let next = x * &cur - b(k - 1) * &prev;
            prev = cur;
            cur = next;
        }
        let s = sign(&cur);
        if s != 0 {
            if s != last_sign {
                changes += 1;
            }
            last_sign = s;
        }
    }
    changes
}

fn sign(x: &RBig) -> i8 {
    match x.cmp(&RBig::ZERO) {
        core::cmp::Ordering::Less => -1,
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Greater => 1,
    }
}

fn half(x: &RBig, y: &RBig) -> RBig {
    (x + y) * ratio(1, 2)
}

/// A zero of a polynomial: a bracket with a sign change, or an exact value (`lo == hi`).
#[derive(Clone, Debug)]
struct Bracket {
    lo: RBig,
    hi: RBig,
}

impl Bracket {
    /// Halves the bracket around the zero of `p`; an exact zero at the midpoint closes it.
    fn halve(&mut self, p: &IntPoly) {
        if self.lo == self.hi {
            return;
        }
        // A symmetric rule has a zero at 0 exactly when its degree is odd; halving a bracket that
        // straddles 0 would only approach it, so 0 is tried first.
        if self.lo < RBig::ZERO && self.hi > RBig::ZERO && p.sign_at(&RBig::ZERO) == 0 {
            self.lo = RBig::ZERO;
            self.hi = RBig::ZERO;
            return;
        }
        let mid = half(&self.lo, &self.hi);
        let sm = p.sign_at(&mid);
        // An end may itself be a zero (the exact zero at 0, next to this bracket's): then the
        // other end's sign decides.
        let lo_sign = p.sign_at(&self.lo);
        let toward_lo = if lo_sign == 0 {
            sm == p.sign_at(&self.hi)
        } else {
            sm != lo_sign
        };
        if sm == 0 {
            self.lo.clone_from(&mid);
            self.hi = mid;
        } else if toward_lo {
            self.hi = mid;
        } else {
            self.lo = mid;
        }
    }

    fn width(&self) -> RBig {
        &self.hi - &self.lo
    }
}

/// The f64 nearest to x, ties to even.
fn nearest(x: &RBig) -> f64 {
    x.to_f64().value()
}

/// The common value of an enclosure's ends rounded, or `None` if they round apart.
fn rounded(lo: &RBig, hi: &RBig) -> Option<f64> {
    let (a, b) = (nearest(lo), nearest(hi));
    (a.to_bits() == b.to_bits()).then_some(a)
}

/// The enclosure of f over the bracket, from its value at `lo` and its slope bound.
fn enclose(f: &IntPoly, slope: &RBig, br: &Bracket) -> (RBig, RBig) {
    let v = f.at(&br.lo);
    let d = slope * br.width();
    (&v - &d, &v + &d)
}

/// One correctly rounded node and weight: halves the bracket until the node and `weight`'s
/// enclosure each round to one `f64`.
fn settle(
    p: &IntPoly,
    mut br: Bracket,
    weight: &dyn Fn(&Bracket) -> Option<(RBig, RBig)>,
) -> Result<(f64, f64), GenError> {
    let mut since_weight = 0u32;
    for _ in 0..=MAX_HALVINGS {
        // The weight is enclosed only once the node rounds, and then every eighth halving: it is
        // the costlier enclosure.
        if since_weight == 0 {
            if let Some(x) = rounded(&br.lo, &br.hi) {
                if let Some(w) = weight(&br).and_then(|(lo, hi)| rounded(&lo, &hi)) {
                    return Ok((x, w));
                }
                since_weight = 8;
            }
        }
        since_weight = since_weight.saturating_sub(1);
        br.halve(p);
    }
    fail(format!(
        "a node near {} did not round to one f64 within {MAX_HALVINGS} halvings",
        nearest(&br.lo)
    ))
}

/// The quotient enclosure [a, b]/[c, d] for c > 0 or d < 0, or `None` if the divisor holds 0.
fn quotient(num: (RBig, RBig), den: (RBig, RBig)) -> Option<(RBig, RBig)> {
    let (a, b) = num;
    let (c, d) = den;
    if c <= RBig::ZERO && d >= RBig::ZERO {
        return None;
    }
    let q = [&a / &c, &a / &d, &b / &c, &b / &d];
    let lo = q.iter().min()?.clone();
    let hi = q.iter().max()?.clone();
    Some((lo, hi))
}

/// The brackets of the n zeros of Pₙ, ascending, isolated by Sturm counts.
fn gauss_brackets(n: usize) -> Result<Vec<Bracket>, GenError> {
    let pn = IntPoly::new(&legendre(n)[n]);
    let mut out = Vec::new();
    // Each work item: an interval and the number of zeros above each end.
    let mut work = vec![(RBig::from(-1), RBig::ONE, n, 0usize)];
    while let Some((lo, hi, above_lo, above_hi)) = work.pop() {
        let inside = above_lo - above_hi;
        match inside {
            0 => {}
            1 => out.push(Bracket { lo, hi }),
            _ => {
                let mid = half(&lo, &hi);
                let above_mid = zeros_above(n, &mid);
                if pn.sign_at(&mid) == 0 {
                    // An exact zero at the midpoint: its own bracket, and the rest either side.
                    out.push(Bracket {
                        lo: mid.clone(),
                        hi: mid.clone(),
                    });
                    work.push((lo, mid.clone(), above_lo, above_mid + 1));
                    work.push((mid, hi, above_mid, above_hi));
                } else {
                    work.push((lo, mid.clone(), above_lo, above_mid));
                    work.push((mid, hi, above_mid, above_hi));
                }
            }
        }
        if out.len() > n {
            return fail(format!("more than {n} zeros isolated for P{n}"));
        }
    }
    out.sort_by(|a, b| a.lo.cmp(&b.lo));
    if out.len() != n {
        return fail(format!("{} zeros isolated for P{n}", out.len()));
    }
    Ok(out)
}

/// The Gauss weight function 1/S(x), S(x) = Σₖ Pₖ(x)²/(μ₀·b₁⋯bₖ) for k < n, as S's polynomial.
fn christoffel(p: &[Poly], n: usize) -> Poly {
    let mut s = Poly::constant(RBig::ZERO);
    let mut h = ratio(2, 1);
    for (k, pk) in p.iter().enumerate().take(n) {
        if k > 0 {
            h *= b(k);
        }
        s = s.add(&pk.mul(pk).scaled(&(RBig::ONE / &h)));
    }
    s
}

/// An n-point Gauss–Legendre rule: nodes and weights, ascending, correctly rounded.
fn gauss(n: usize) -> Result<Vec<(f64, f64)>, GenError> {
    let p = legendre(n);
    let s = christoffel(&p, n);
    let slope = s.slope_bound();
    let s = IntPoly::new(&s);
    let pn = IntPoly::new(&p[n]);
    let weight = |br: &Bracket| {
        let (lo, hi) = enclose(&s, &slope, br);
        quotient((RBig::ONE, RBig::ONE), (lo, hi))
    };
    gauss_brackets(n)?
        .into_iter()
        .map(|br| settle(&pn, br, &weight))
        .collect()
}

/// The Stieltjes polynomial Eₙ₊₁: monic, degree n + 1, orthogonal to xᵏ·Pₙ for k = 0, …, n.
fn stieltjes(pn: &Poly, n: usize) -> Result<Poly, GenError> {
    // E = x^(n+1) + Σ cⱼ x^(n+1−2j), j = 1..=m, by parity; the conditions with odd k + parity
    // vanish, leaving m = ⌊(n + 1)/2⌋ equations, k = 1, 3, ... or 0, 2, ... as parity needs.
    let m = n.div_ceil(2);
    let integral = |e: usize, k: usize| -> RBig {
        // ∫ x^e · Pₙ(x) · x^k dx.
        pn.0.iter()
            .enumerate()
            .fold(RBig::ZERO, |acc, (i, c)| acc + c * moment(e + i + k))
    };
    let ks: Vec<usize> = (0..=n).filter(|k| (n + 1 + n + k) % 2 == 0).collect();
    if ks.len() != m {
        return fail(format!(
            "{} conditions for {m} unknowns of E{}",
            ks.len(),
            n + 1
        ));
    }
    // Rows: Σⱼ cⱼ ∫x^(n+1−2j) Pₙ x^k = −∫x^(n+1) Pₙ x^k.
    let mut a: Vec<Vec<RBig>> = ks
        .iter()
        .map(|&k| {
            let mut row: Vec<RBig> = (1..=m).map(|j| integral(n + 1 - 2 * j, k)).collect();
            row.push(-integral(n + 1, k));
            row
        })
        .collect();
    // Gaussian elimination, exact.
    for col in 0..m {
        let Some(piv) = (col..m).find(|&r| a[r][col] != RBig::ZERO) else {
            return fail(format!("the system for E{} is singular", n + 1));
        };
        a.swap(col, piv);
        for r in 0..m {
            if r != col && a[r][col] != RBig::ZERO {
                let f = &a[r][col] / &a[col][col];
                let pivot_row = a[col].clone();
                for (x, y) in a[r].iter_mut().zip(pivot_row.iter()) {
                    *x = &*x - &f * y;
                }
            }
        }
    }
    let mut coeffs = vec![RBig::ZERO; n + 2];
    coeffs[n + 1] = RBig::ONE;
    for j in 1..=m {
        coeffs[n + 1 - 2 * j] = &a[j - 1][m] / &a[j - 1][j - 1];
    }
    Ok(Poly(coeffs))
}

/// A Gauss–Kronrod rule: its 2n + 1 nodes ascending, with the Kronrod weight of each and the
/// Gauss weight of each Gauss node (the odd positions), correctly rounded.
struct KronrodTable {
    nodes: Vec<f64>,
    kronrod: Vec<f64>,
    gauss: Vec<f64>,
}

fn kronrod(n: usize) -> Result<KronrodTable, GenError> {
    let p = legendre(n);
    let pn = p[n].clone();
    let e = stieltjes(&pn, n)?;
    let q = pn.mul(&e);
    let dq = q.derivative();
    // R(t) = Σⱼ rⱼ tʲ, rⱼ = Σ_{k > j} q_k · m_{k−1−j}.
    let r = Poly(
        (0..q.0.len().saturating_sub(1))
            .map(|j| {
                q.0.iter()
                    .enumerate()
                    .skip(j + 1)
                    .fold(RBig::ZERO, |acc, (k, c)| acc + c * moment(k - 1 - j))
            })
            .collect(),
    );
    let (slope_r, slope_dq) = (r.slope_bound(), dq.slope_bound());
    let (r, dq) = (IntPoly::new(&r), IntPoly::new(&dq));
    let (e_int, pn_int) = (IntPoly::new(&e), IntPoly::new(&pn));
    let weight = |br: &Bracket| quotient(enclose(&r, &slope_r, br), enclose(&dq, &slope_dq, br));
    let gauss_brackets = gauss_brackets(n)?;
    // Separators between the Gauss zeros: each Gauss bracket's midpoint, with −1 and 1 outside.
    let mut cuts = vec![RBig::from(-1)];
    cuts.extend(gauss_brackets.iter().map(|g| half(&g.lo, &g.hi)));
    cuts.push(RBig::ONE);
    let mut all: Vec<(f64, f64)> = Vec::new();
    for w in cuts.windows(2) {
        let (lo, hi) = (w[0].clone(), w[1].clone());
        let (sl, sh) = (e_int.sign_at(&lo), e_int.sign_at(&hi));
        if sl == 0 || sh == 0 || sl == sh {
            return fail(format!(
                "E{} does not change sign between two Gauss zeros",
                n + 1
            ));
        }
        all.push(settle(&e_int, Bracket { lo, hi }, &weight)?);
    }
    let gauss_rule = gauss(n)?;
    for br in gauss_brackets {
        all.push(settle(&pn_int, br, &weight)?);
    }
    all.sort_by(|a, b| a.0.total_cmp(&b.0));
    let nodes: Vec<f64> = all.iter().map(|(x, _)| *x).collect();
    // The Gauss nodes are at the odd positions; they must be the Gauss rule's own.
    for (i, (x, _)) in gauss_rule.iter().enumerate() {
        if nodes.get(2 * i + 1).map(|v| v.to_bits()) != Some(x.to_bits()) {
            return fail(format!(
                "the Gauss nodes of G{n} K{} do not interlace",
                2 * n + 1
            ));
        }
    }
    Ok(KronrodTable {
        nodes,
        kronrod: all.iter().map(|(_, w)| *w).collect(),
        gauss: gauss_rule.iter().map(|(_, w)| *w).collect(),
    })
}

/// A double as Rust source, exactly: `f64::from_bits(0x…)` with its shortest decimal in a comment.
fn bits(v: f64) -> String {
    let h = format!("{:016x}", v.to_bits());
    format!(
        "f64::from_bits(0x{}_{}_{}_{})",
        &h[0..4],
        &h[4..8],
        &h[8..12],
        &h[12..16]
    )
}

fn list(out: &mut String, name: &str, values: &[f64]) {
    let _ = writeln!(out, "        {name}: &[");
    for v in values {
        let _ = writeln!(out, "            {}, // {v:?}", bits(*v));
    }
    let _ = writeln!(out, "        ],");
}

/// The module's header.
const HEADER: &str = "\
//! Generated by geom-gen, generator `quadrature-rules` (seed: none, it uses no randomness), by
//! `projects/engines/geometry/crates/geom-gen/src/quadrature.rs`. Do not edit: change the generator
//! and run `cargo run -p geom-gen -- write`. CI runs `cargo run -p geom-gen -- check` and fails on
//! any difference.
//!
//! The quadrature rules [`crate::quadrature`] integrates with, on [−1, 1]: the Gauss–Legendre rules
//! of 1 to 20 points, and the Gauss–Kronrod rules of 2n + 1 points that extend the n-point Gauss
//! rule for n = 7, 10, 15, 20, 25 and 30 (QUADPACK's G7K15 to G30K61).
//!
//! Guarantees: every node and weight is the `f64` nearest to its exact value, ties to even, decided
//! by geom-gen from an exact rational enclosure; nodes are ascending and symmetric about 0; in a
//! Kronrod rule the Gauss nodes are the odd positions, the same bits as the Gauss rule's own. The
//! n-point Gauss rule integrates every polynomial of degree at most 2n − 1 exactly, and a Kronrod
//! rule every one of degree at most 3n + 1 (n even) or 3n + 2 (n odd), up to the rounding of its
//! entries (`tests/quadrature.rs`).
//!
//! Method: Golub and Welsch's eigenvalue method for the Gauss rules, with Sturm-count bisection
//! (G. H. Golub and J. H. Welsch, Mathematics of Computation 23 (1969),
//! doi:10.1090/S0025-5718-69-99647-1), and Kronrod's extension from its definition, the zeros of
//! the Stieltjes polynomial with interpolatory weights (A. S. Kronrod 1965; G. Szegő 1935,
//! doi:10.1007/BF01448041); the generator's header gives the details.
//!
//! Cost: none at run time: constant tables.

use crate::quadrature::{GaussRule, KronrodRule};
";

/// Generator `quadrature-rules`.
///
/// # Errors
///
/// [`GenError::Generator`] if a rule cannot be computed exactly (module documentation).
pub fn generate() -> Result<Vec<u8>, GenError> {
    let mut out = String::from(HEADER);
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "/// The Gauss–Legendre rules: entry n − 1 has n points, for n = 1 to {GAUSS_MAX}."
    );
    let _ = writeln!(
        out,
        "pub const GAUSS_LEGENDRE: [GaussRule; {GAUSS_MAX}] = ["
    );
    for n in 1..=GAUSS_MAX {
        let rule = gauss(n)?;
        let _ = writeln!(out, "    // {n} point(s).");
        let _ = writeln!(out, "    GaussRule {{");
        list(
            &mut out,
            "nodes",
            &rule.iter().map(|(x, _)| *x).collect::<Vec<_>>(),
        );
        list(
            &mut out,
            "weights",
            &rule.iter().map(|(_, w)| *w).collect::<Vec<_>>(),
        );
        let _ = writeln!(out, "    }},");
    }
    let _ = writeln!(out, "];");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "/// The Gauss–Kronrod rules, for n = 7, 10, 15, 20, 25 and 30 Gauss points: 2n + 1 nodes each."
    );
    let _ = writeln!(
        out,
        "pub const GAUSS_KRONROD: [KronrodRule; {}] = [",
        KRONROD_N.len()
    );
    for n in KRONROD_N {
        let rule = kronrod(n)?;
        let _ = writeln!(out, "    // G{n} K{}.", 2 * n + 1);
        let _ = writeln!(out, "    KronrodRule {{");
        list(&mut out, "nodes", &rule.nodes);
        list(&mut out, "kronrod", &rule.kronrod);
        list(&mut out, "gauss", &rule.gauss);
        let _ = writeln!(out, "    }},");
    }
    let _ = writeln!(out, "];");
    Ok(out.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed module is what the generator writes, on every target CI runs.
    #[test]
    fn the_committed_rules_are_current() {
        let committed = include_str!("../../geom-num/src/quadrature_rules/mod.rs");
        let generated = String::from_utf8(generate().unwrap()).unwrap();
        assert!(
            committed == generated,
            "run `cargo run -p geom-gen -- write`"
        );
    }

    /// `nearest` is the nearest double: no neighbour is closer, exactly.
    #[test]
    fn nearest_is_correctly_rounded() {
        let cases = [
            ratio(1, 3),
            ratio(2, 3),
            ratio(1, 10),
            ratio(-7, 9),
            ratio(355, 113),
        ];
        for q in cases {
            let f = nearest(&q);
            let exact = |v: f64| RBig::try_from(v).unwrap();
            let d = abs(&(&q - exact(f)));
            assert!(d <= abs(&(&q - exact(f.next_up()))), "{q}");
            assert!(d <= abs(&(&q - exact(f.next_down()))), "{q}");
        }
    }

    /// The interpolatory weights, which the Kronrod rules use, give Golub and Welsch's Gauss
    /// weights when the nodes are the Gauss nodes: two formulas, one rule.
    #[test]
    fn interpolatory_weights_are_golub_welsch_weights() {
        for n in 1..=10 {
            let p = legendre(n);
            let pn = p[n].clone();
            let dq = pn.derivative();
            let r = Poly(
                (0..pn.0.len() - 1)
                    .map(|j| {
                        pn.0.iter()
                            .enumerate()
                            .skip(j + 1)
                            .fold(RBig::ZERO, |acc, (k, c)| acc + c * moment(k - 1 - j))
                    })
                    .collect(),
            );
            let (slope_r, slope_dq) = (r.slope_bound(), dq.slope_bound());
            let (r, dq) = (IntPoly::new(&r), IntPoly::new(&dq));
            let weight =
                |br: &Bracket| quotient(enclose(&r, &slope_r, br), enclose(&dq, &slope_dq, br));
            let pn_int = IntPoly::new(&pn);
            let interpolatory: Vec<(f64, f64)> = gauss_brackets(n)
                .unwrap()
                .into_iter()
                .map(|br| settle(&pn_int, br, &weight).unwrap())
                .collect();
            assert_eq!(interpolatory, gauss(n).unwrap(), "n = {n}");
        }
    }

    /// Two known values: the 2-point rule's nodes ±1/√3 with weights 1, and the 3-point rule's
    /// middle weight 8/9.
    #[test]
    fn small_rules_have_their_closed_forms() {
        let two = gauss(2).unwrap();
        // The node is the double nearest 1/√3: the midpoints to its neighbours square to either
        // side of 1/3.
        let x = two[1].0;
        let exact = |v: f64| RBig::try_from(v).unwrap();
        let (below, above) = (
            half(&exact(x), &exact(x.next_down())),
            half(&exact(x), &exact(x.next_up())),
        );
        assert!(&below * &below < ratio(1, 3) && &above * &above > ratio(1, 3));
        assert_eq!(two[0].0, -x);
        assert_eq!((two[0].1, two[1].1), (1.0, 1.0));
        assert_eq!(gauss(3).unwrap()[1], (0.0, nearest(&ratio(8, 9))));
    }
}
