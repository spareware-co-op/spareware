//! The exact rational reference for the NURBS tests, until piece G57's reference lands in
//! `geom-test`: B-spline bases by their definition (the Cox–de Boor recurrence and the derivative
//! formula), in exact rational arithmetic, and the magnitudes that the error bounds of `basis.rs`
//! are stated against.
//!
//! Every double is a rational, so the reference evaluates exactly the knots, parameters and
//! control points the code is given; a test compares the code's doubles with these values against
//! the bounds the module headers state.

#![allow(
    dead_code,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::doc_markdown,
    clippy::needless_range_loop,
    clippy::range_plus_one,
    clippy::assigning_clones,
    clippy::similar_names,
    reason = "test reference code written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_nurbs::Limit;

pub type Q = RBig;

/// `x` exactly.
pub fn q(x: f64) -> Q {
    RBig::try_from(x).unwrap()
}

/// The integer `k` as a rational.
pub fn qi(k: i64) -> Q {
    RBig::from(IBig::from(k))
}

pub fn zero() -> Q {
    qi(0)
}

pub fn abs(x: &Q) -> Q {
    if *x < zero() { -x.clone() } else { x.clone() }
}

/// γ_n = n·u / (1 − n·u), u = 2^−53, exactly.
pub fn gamma(n: usize) -> Q {
    let u = RBig::from_parts(IBig::from(1), dashu_int::UBig::from(1u8) << 53);
    let nu = qi(n as i64) * u;
    nu.clone() / (qi(1) - nu)
}

/// A small deterministic generator (a 64-bit linear congruential sequence, Knuth's MMIX
/// constants); test inputs only.
pub struct Lcg(pub u64);

impl Lcg {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    /// A double in [0, 1) with 53 random bits.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// An integer in [0, n).
    pub fn below(&mut self, n: usize) -> usize {
        ((self.next_u64() >> 33) % n as u64) as usize
    }
}

/// A knot vector for degree `p` with `n + 1` functions: clamped or not, with interior knots that
/// are whole numbers, dyadic fractions or arbitrary doubles, some repeated up to `p + 1` times.
pub fn knots(rng: &mut Lcg, p: usize, n: usize, clamped: bool, kind: usize) -> Vec<f64> {
    let m = n + p + 1;
    loop {
        let mut values: Vec<f64> = Vec::new();
        let interior = if clamped { m + 1 - 2 * (p + 1) } else { m + 1 };
        let mut x = match kind % 3 {
            0 => -3.0,
            1 => 0.25,
            _ => rng.unit() * 7.0 - 2.0,
        };
        while values.len() < interior {
            let step = match kind % 3 {
                0 => (1 + rng.below(3)) as f64,
                1 => (1 + rng.below(8)) as f64 / 8.0,
                _ => rng.unit() * 2.0 + 1e-3,
            };
            x += step;
            let repeat = if rng.below(4) == 0 { 1 + rng.below(p + 1) } else { 1 };
            for _ in 0..repeat {
                if values.len() < interior {
                    values.push(x);
                }
            }
        }
        let mut u = Vec::new();
        if clamped {
            let a = values.first().copied().unwrap_or(x) - 1.0;
            let b = values.last().copied().unwrap_or(x) + 1.0;
            u.extend(std::iter::repeat_n(a, p + 1));
            u.extend(values);
            u.extend(std::iter::repeat_n(b, p + 1));
        } else {
            u = values;
        }
        // Keep vectors whose runs respect p + 1 and whose domain is more than one value.
        if geom_nurbs::Basis::new(p, u.clone()).is_ok() {
            return u;
        }
    }
}

/// The limit used at `u`: from the right at the start of the domain, from the left at its end.
pub fn effective(knots: &[f64], p: usize, u: f64, limit: Limit) -> Limit {
    let lo = knots[p];
    let hi = knots[knots.len() - 1 - p];
    if u == lo {
        Limit::FromRight
    } else if u == hi {
        Limit::FromLeft
    } else {
        limit
    }
}

/// Every basis function of every degree from 0 to `p` at `u`, by the Cox–de Boor recurrence:
/// `table[k][i]` is N_(i,k)(u), one-sided as `limit` says.
pub fn cox_de_boor(knots: &[f64], p: usize, u: f64, limit: Limit) -> Vec<Vec<Q>> {
    let limit = effective(knots, p, u, limit);
    let kq: Vec<Q> = knots.iter().map(|&k| q(k)).collect();
    let uq = q(u);
    let m = knots.len() - 1;
    let mut table = Vec::new();
    let zero_degree: Vec<Q> = (0..m)
        .map(|i| {
            let inside = match limit {
                Limit::FromRight => knots[i] <= u && u < knots[i + 1],
                Limit::FromLeft => knots[i] < u && u <= knots[i + 1],
            };
            if inside { qi(1) } else { zero() }
        })
        .collect();
    table.push(zero_degree);
    for k in 1..=p {
        let prev = &table[k - 1];
        let row: Vec<Q> = (0..m - k)
            .map(|i| {
                let mut v = zero();
                let d1 = &kq[i + k] - &kq[i];
                if d1 != zero() {
                    v += (&uq - &kq[i]) / &d1 * &prev[i];
                }
                let d2 = &kq[i + k + 1] - &kq[i + 1];
                if d2 != zero() {
                    v += (&kq[i + k + 1] - &uq) / &d2 * &prev[i + 1];
                }
                v
            })
            .collect();
        table.push(row);
    }
    table
}

/// The derivatives of every basis function of degree `p` at `u`, by the derivative formula:
/// `out[d][i]` is the d-th derivative of N_(i,p), d from 0 to `order`.
pub fn derivatives(knots: &[f64], p: usize, u: f64, limit: Limit, order: usize) -> Vec<Vec<Q>> {
    let kq: Vec<Q> = knots.iter().map(|&k| q(k)).collect();
    let values = cox_de_boor(knots, p, u, limit);
    // dtab[d][k][i]: the d-th derivative of N_(i,k).
    let mut dtab: Vec<Vec<Vec<Q>>> = vec![values];
    for d in 1..=order {
        let prev = &dtab[d - 1];
        let mut level = Vec::new();
        for k in 0..=p {
            let count = prev[k].len();
            let row: Vec<Q> = (0..count)
                .map(|i| {
                    if k == 0 {
                        return zero();
                    }
                    let mut v = zero();
                    let d1 = &kq[i + k] - &kq[i];
                    if d1 != zero() {
                        v += &prev[k - 1][i] / &d1;
                    }
                    let d2 = &kq[i + k + 1] - &kq[i + 1];
                    if d2 != zero() {
                        v -= &prev[k - 1][i + 1] / &d2;
                    }
                    v * qi(k as i64)
                })
                .collect();
            level.push(row);
        }
        dtab.push(level);
    }
    dtab.into_iter().map(|mut level| level.swap_remove(p)).collect()
}

/// The magnitudes of A2.3's derivative scheme at `span`: `out[k][r]` is p!/(p − k)! times the sum
/// over j of A_(k,j) · N_(span−p+r−k+j, p−k), with A_(k,j) the scheme's coefficients run on
/// absolute values (each difference replaced by a sum) and every value exact.
pub fn magnitudes_a23(knots: &[f64], p: usize, span: usize, u: f64, limit: Limit, order: usize)
    -> Vec<Vec<Q>> {
    let kq: Vec<Q> = knots.iter().map(|&k| q(k)).collect();
    let values = cox_de_boor(knots, p, u, limit);
    // ndu_val(j, deg): N_(span−deg+j, deg); delta(j, r) = U[span+r+1] − U[span+1−j+r].
    let val = |j: usize, deg: usize| -> Q { values[deg][span - deg + j].clone() };
    let delta = |j: usize, r: usize| -> Q { &kq[span + r + 1] - &kq[span + 1 + r - j] };
    let top = order.min(p);
    let mut out = vec![vec![zero(); p + 1]; order + 1];
    for r in 0..=p {
        out[0][r] = val(r, p);
    }
    for r in 0..=p {
        let mut a_prev: Vec<Q> = vec![qi(1)];
        for k in 1..=top {
            let pk = p - k;
            let mut a = vec![zero(); k + 1];
            let mut d = zero();
            if r >= k {
                a[0] = &a_prev[0] / delta(pk + 1, r - k);
                d += &a[0] * val(r - k, pk);
            }
            let j1 = if r + 1 >= k { 1 } else { k - r };
            let j2 = if r <= pk + 1 { k - 1 } else { p - r };
            for j in j1..=j2 {
                let rkj = r + j - k;
                let prev_j = a_prev.get(j).cloned().unwrap_or_else(zero);
                let prev_j1 = a_prev.get(j - 1).cloned().unwrap_or_else(zero);
                a[j] = (abs(&prev_j) + abs(&prev_j1)) / delta(pk + 1, rkj);
                d += &a[j] * val(rkj, pk);
            }
            if r <= pk {
                a[k] = abs(&a_prev[k - 1]) / delta(pk + 1, r);
                d += &a[k] * val(r, pk);
            }
            let mut factor = qi(1);
            for t in 0..k {
                factor *= qi((p - t) as i64);
            }
            out[k][r] = d * factor;
            a_prev = a;
        }
    }
    out
}

/// The magnitudes of A2.5's derivative scheme for N_i at `u`: `out[k]` is the scheme's k-th
/// derivative run on absolute values (each difference replaced by a sum), every value exact.
pub fn magnitudes_a25(knots: &[f64], p: usize, i: usize, u: f64, limit: Limit, order: usize)
    -> Vec<Q> {
    let kq: Vec<Q> = knots.iter().map(|&k| q(k)).collect();
    let values = cox_de_boor(knots, p, u, limit);
    // nt[j][k] = N_(i+j, k).
    let nt = |j: usize, k: usize| -> Q { values[k][i + j].clone() };
    let mut out = vec![zero(); order + 1];
    out[0] = nt(0, p);
    for k in 1..=order.min(p) {
        let mut nd: Vec<Q> = (0..=k).map(|j| nt(j, p - k)).collect();
        for l in 1..=k {
            let factor = qi((p - k + l) as i64);
            let d0 = &kq[i + p - k + l] - &kq[i];
            let mut saved = if nd[0] == zero() || d0 == zero() { zero() } else { abs(&nd[0]) / &d0 };
            for j in 0..(k - l + 1) {
                let ur = &kq[p - k + l + i + j + 1];
                let ul = &kq[i + j + 1];
                if nd[j + 1] == zero() {
                    nd[j] = &factor * &saved;
                    saved = zero();
                } else {
                    let temp = abs(&nd[j + 1]) / (ur - ul);
                    nd[j] = &factor * (&saved + &temp);
                    saved = temp;
                }
            }
        }
        out[k] = nd[0].clone();
    }
    out
}

/// |a − b| ≤ bound.
pub fn within(computed: f64, exact: &Q, bound: &Q) -> bool {
    abs(&(q(computed) - exact)) <= *bound
}
