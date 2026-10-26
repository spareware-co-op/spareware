//! Surfaces of `geom_nurbs::surface` against the exact rational reference: points and partial
//! derivatives within the bounds the module states, and the documented errors.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::many_single_char_names,
    clippy::doc_markdown,
    clippy::needless_range_loop,
    clippy::similar_names,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod common;

use common::{Lcg, Q, abs, derivatives, gamma, knots, magnitudes_a23, q, qi, within, zero};
use geom_nurbs::{Basis, Limit, NurbsError, Surface};

const LIMITS: [(Limit, Limit); 2] = [
    (Limit::FromRight, Limit::FromLeft),
    (Limit::FromLeft, Limit::FromRight),
];

fn surfaces() -> Vec<Surface<3>> {
    let mut rng = Lcg(0x5afe_5afe);
    let mut out = Vec::new();
    for (p, qd) in [(1, 1), (2, 3), (3, 2), (4, 1)] {
        for kind in 0..3 {
            let nu = p + 1 + rng.below(3);
            let bu = Basis::new(p, knots(&mut rng, p, nu, kind != 1, kind)).unwrap();
            let nv = qd + 1 + rng.below(3);
            let bv = Basis::new(qd, knots(&mut rng, qd, nv, kind != 2, kind + 1)).unwrap();
            let count = bu.count() * bv.count();
            let pts: Vec<[f64; 3]> = (0..count)
                .map(|_| [rng.unit() * 10.0 - 5.0, rng.unit() * 0.1, rng.unit() * 1000.0])
                .collect();
            out.push(Surface::new(bu.clone(), bv.clone(), &pts).unwrap());
            let ws: Vec<f64> = (0..count).map(|_| 0.5 + rng.unit() * 2.0).collect();
            out.push(Surface::new_rational(bu, bv, &pts, &ws).unwrap());
        }
    }
    out
}

fn params(b: &Basis, rng: &mut Lcg) -> Vec<f64> {
    let (lo, hi) = b.domain();
    let mut out: Vec<f64> = b.knots().iter().copied().filter(|&k| lo <= k && k <= hi).collect();
    out.dedup();
    out.truncate(3);
    for _ in 0..2 {
        out.push(lo + (hi - lo) * rng.unit());
    }
    out
}

fn binomial(n: usize, k: usize) -> i64 {
    let mut r = 1i64;
    for i in 0..k {
        r = r * (n - i) as i64 / (i + 1) as i64;
    }
    r
}

/// The exact partial derivatives S_(k,l), k ≤ du, l ≤ dv, and the magnitudes of the module's
/// bounds, as tables indexed [k][l][coordinate].
#[allow(clippy::type_complexity, reason = "a test table")]
fn exact(
    s: &Surface<3>,
    u: f64,
    v: f64,
    limits: (Limit, Limit),
    du: usize,
    dv: usize,
) -> (Vec<Vec<[Q; 3]>>, Vec<Vec<[Q; 3]>>) {
    let (bu, bv) = (s.basis_u(), s.basis_v());
    let (p, qd) = (bu.degree(), bv.degree());
    let su = bu.span(u, limits.0).unwrap();
    let sv = bv.span(v, limits.1).unwrap();
    let nu = derivatives(bu.knots(), p, u, limits.0, du);
    let nv = derivatives(bv.knots(), qd, v, limits.1, dv);
    let mu = magnitudes_a23(bu.knots(), p, su, u, limits.0, du);
    let mv = magnitudes_a23(bv.knots(), qd, sv, v, limits.1, dv);
    let cols = bv.count();
    let pts = s.homogeneous();
    let w: Vec<Q> = match s.weights() {
        Some(ws) => ws.iter().map(|&x| q(x)).collect(),
        None => vec![qi(1); pts.len()],
    };
    let z3 = || [zero(), zero(), zero()];
    let mut a = vec![vec![z3(); dv + 1]; du + 1];
    let mut sa = vec![vec![z3(); dv + 1]; du + 1];
    let mut wd = vec![vec![zero(); dv + 1]; du + 1];
    let mut sw = vec![vec![zero(); dv + 1]; du + 1];
    for k in 0..=du {
        for l in 0..=dv {
            for i in 0..=p {
                for j in 0..=qd {
                    let gi = su - p + i;
                    let gj = sv - qd + j;
                    let n = &nu[k][gi] * &nv[l][gj];
                    let mk = if k <= p { mu[k][i].clone() } else { zero() };
                    let ml = if l <= qd { mv[l][j].clone() } else { zero() };
                    let m = mk * ml;
                    let idx = gi * cols + gj;
                    for d in 0..3 {
                        a[k][l][d] += &n * q(pts[idx][d]);
                        sa[k][l][d] += &m * abs(&q(pts[idx][d]));
                    }
                    wd[k][l] += &n * &w[idx];
                    sw[k][l] += &m * &w[idx];
                }
            }
        }
    }
    if s.weights().is_none() {
        return (a, sa);
    }
    let mut out = vec![vec![z3(); dv + 1]; du + 1];
    let mut ms = vec![vec![z3(); dv + 1]; du + 1];
    for k in 0..=du {
        for l in 0..=dv {
            let mut val = a[k][l].clone();
            let mut mag = sa[k][l].clone();
            for j in 1..=l {
                let b = qi(binomial(l, j));
                for d in 0..3 {
                    val[d] -= &b * &wd[0][j] * &out[k][l - j][d];
                    mag[d] += &b * &sw[0][j] * &ms[k][l - j][d];
                }
            }
            for i in 1..=k {
                let b = qi(binomial(k, i));
                for d in 0..3 {
                    val[d] -= &b * &wd[i][0] * &out[k - i][l][d];
                    mag[d] += &b * &sw[i][0] * &ms[k - i][l][d];
                    let mut v2 = zero();
                    let mut m2 = zero();
                    for j in 1..=l {
                        let c = qi(binomial(l, j));
                        v2 += &c * &wd[i][j] * &out[k - i][l - j][d];
                        m2 += &c * &sw[i][j] * &ms[k - i][l - j][d];
                    }
                    val[d] -= &b * v2;
                    mag[d] += &b * m2;
                }
            }
            for d in 0..3 {
                val[d] = &val[d] / &wd[0][0];
                mag[d] = &mag[d] / &wd[0][0];
            }
            out[k][l] = val;
            ms[k][l] = mag;
        }
    }
    (out, ms)
}

#[test]
fn points_and_derivatives_are_within_their_bounds() {
    let mut rng = Lcg(21);
    for s in surfaces() {
        let (p, qd) = (s.basis_u().degree(), s.basis_v().degree());
        let (du, dv) = (p.min(2) + 1, qd.min(2));
        for u in params(s.basis_u(), &mut rng) {
            for v in params(s.basis_v(), &mut rng) {
                for limits in LIMITS {
                    let point = s.point(u, v, limits).unwrap();
                    let mut out = vec![[0.0; 3]; (du + 1) * (dv + 1)];
                    s.derivs(u, v, limits, (du, dv), &mut out).unwrap();
                    for d in 0..3 {
                        assert_eq!(out[0][d].to_bits(), point[d].to_bits());
                    }
                    let (e, m) = exact(&s, u, v, limits, du, dv);
                    for k in 0..=du {
                        for l in 0..=dv {
                            let order = k + l;
                            let factor = if s.is_rational() {
                                gamma((order + 1) * (14 * (p + qd) + 2 * order + 12))
                            } else if order == 0 {
                                gamma(7 * (p + qd) + 2)
                            } else {
                                gamma(7 * (p + qd) + 6)
                            };
                            for d in 0..3 {
                                let bound = &factor * &m[k][l][d];
                                assert!(
                                    within(out[k * (dv + 1) + l][d], &e[k][l][d], &bound),
                                    "degrees ({p}, {qd}), rational {}, ({u}, {v}), order ({k}, {l})",
                                    s.is_rational()
                                );
                            }
                        }
                    }
                    if s.is_rational() {
                        let factor = gamma(14 * (p + qd) + 5);
                        for d in 0..3 {
                            assert!(within(point[d], &e[0][0][d], &(&factor * &m[0][0][d])));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn errors_are_typed() {
    let bu = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let bv = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let pts = [[0.0; 2]; 6];
    assert_eq!(
        Surface::new(bu.clone(), bv.clone(), &pts[..5]).unwrap_err(),
        NurbsError::PointCount { points: 5, expected: 6 }
    );
    let s = Surface::new(bu.clone(), bv.clone(), &pts).unwrap();
    let lim = (Limit::FromRight, Limit::FromRight);
    assert_eq!(s.point(0.5, 1.5, lim), Err(NurbsError::ParameterOutside));
    assert_eq!(s.point(f64::NAN, 0.5, lim), Err(NurbsError::ParameterOutside));
    let mut out = vec![[0.0; 2]; 5];
    assert_eq!(
        s.derivs(0.5, 0.5, lim, (1, 1), &mut out),
        Err(NurbsError::BufferLength { given: 5, needed: 4 })
    );
    let mut big = vec![[0.0; 2]; 30];
    assert_eq!(
        s.derivs(0.5, 0.5, lim, (0, 29), &mut big),
        Err(NurbsError::DerivativeOrder { order: 29 })
    );
    assert_eq!(
        Surface::new_rational(bu, bv, &pts, &[1.0, 1.0, 1.0, 1.0, 1.0, 0.0]).unwrap_err(),
        NurbsError::Weight { index: 5 }
    );
}
