//! Curves of `geom_nurbs::curve` against the exact rational reference: points and derivatives
//! within the bounds the module states, the end points of clamped curves exact, derivative curves
//! that are the derivatives, and every documented error.

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
use geom_math::{F2, F3};
use geom_nurbs::{Basis, Curve, Limit, NurbsError, Point};
use geom_run::{Budget, WorkMeter};

const LIMITS: [Limit; 2] = [Limit::FromRight, Limit::FromLeft];

/// A curve of dimension 3 for each basis, nonrational and rational, with coordinates of mixed
/// sizes.
fn curves() -> Vec<Curve<3>> {
    let mut rng = Lcg(0x0c0f_fee5);
    let mut out = Vec::new();
    for p in [1, 2, 3, 4, 5] {
        for kind in 0..3 {
            for clamped in [true, false] {
                let n = p + 1 + rng.below(5);
                let basis = Basis::new(p, knots(&mut rng, p, n, clamped, kind)).unwrap();
                let points: Vec<[f64; 3]> = (0..basis.count())
                    .map(|_| {
                        let scale = [1.0, 100.0, 1e-3][rng.below(3)];
                        [
                            (rng.unit() - 0.5) * scale,
                            (rng.unit() - 0.5) * scale,
                            (rng.unit() - 0.5) * scale,
                        ]
                    })
                    .collect();
                out.push(Curve::new(basis.clone(), &points).unwrap());
                let weights: Vec<f64> = (0..basis.count())
                    .map(|_| 0.25 + rng.unit() * 4.0)
                    .collect();
                out.push(Curve::new_rational(basis, &points, &weights).unwrap());
            }
        }
    }
    out
}

fn parameters(curve: &Curve<3>, rng: &mut Lcg, between: usize) -> Vec<f64> {
    let (lo, hi) = curve.basis().domain();
    let mut out: Vec<f64> = curve
        .basis()
        .knots()
        .iter()
        .copied()
        .filter(|&k| lo <= k && k <= hi)
        .collect();
    out.dedup();
    for _ in 0..between {
        out.push(lo + (hi - lo) * rng.unit());
    }
    out
}

/// The exact derivatives C^(k)(u), k from 0 to `order`, coordinate by coordinate, and the
/// magnitudes the module's bounds use: for a nonrational curve Σ M_(k,j)·|P_j|, for a rational
/// one MC_k.
fn exact(curve: &Curve<3>, u: f64, limit: Limit, order: usize) -> (Vec<[Q; 3]>, Vec<[Q; 3]>) {
    let basis = curve.basis();
    let p = basis.degree();
    let span = basis.span(u, limit).unwrap();
    let ders = derivatives(basis.knots(), p, u, limit, order);
    let mags = magnitudes_a23(basis.knots(), p, span, u, limit, order);
    let first = span - p;
    let pts = curve.homogeneous();
    let w: Vec<Q> = match curve.weights() {
        Some(ws) => ws.iter().map(|&x| q(x)).collect(),
        None => vec![qi(1); pts.len()],
    };
    let mut a = vec![[zero(), zero(), zero()]; order + 1];
    let mut sa = vec![[zero(), zero(), zero()]; order + 1];
    let mut wd = vec![zero(); order + 1];
    let mut sw = vec![zero(); order + 1];
    for k in 0..=order {
        for j in 0..=p {
            let nk = &ders[k][first + j];
            let mk = if k <= p { mags[k][j].clone() } else { zero() };
            for d in 0..3 {
                a[k][d] += nk * q(pts[first + j][d]);
                sa[k][d] += &mk * abs(&q(pts[first + j][d]));
            }
            wd[k] += nk * &w[first + j];
            sw[k] += &mk * &w[first + j];
        }
    }
    if curve.weights().is_none() {
        return (a, sa);
    }
    // Leibniz: C^(k) = (A^(k) − Σ C(k, i)·w^(i)·C^(k−i)) / w, and its magnitude recurrence.
    let mut c: Vec<[Q; 3]> = Vec::new();
    let mut mc: Vec<[Q; 3]> = Vec::new();
    for k in 0..=order {
        let mut v = a[k].clone();
        let mut m = sa[k].clone();
        for i in 1..=k {
            let b = qi(binomial(k, i) as i64);
            for d in 0..3 {
                v[d] -= &b * &wd[i] * &c[k - i][d];
                m[d] += &b * &sw[i] * &mc[k - i][d];
            }
        }
        for d in 0..3 {
            v[d] = &v[d] / &wd[0];
            m[d] = &m[d] / &wd[0];
        }
        c.push(v);
        mc.push(m);
    }
    (c, mc)
}

fn binomial(n: usize, k: usize) -> u64 {
    let mut r = 1u64;
    for i in 0..k {
        r = r * (n - i) as u64 / (i + 1) as u64;
    }
    r
}

#[test]
fn points_are_within_their_bound() {
    let mut rng = Lcg(11);
    for curve in curves() {
        let p = curve.degree();
        let factor = if curve.is_rational() {
            gamma(14 * p + 3)
        } else {
            gamma(7 * p + 1)
        };
        for u in parameters(&curve, &mut rng, 10) {
            for limit in LIMITS {
                let c = curve.point(u, limit).unwrap();
                let (e, m) = exact(&curve, u, limit, 0);
                for d in 0..3 {
                    let bound = &factor * &m[0][d];
                    assert!(
                        within(c[d], &e[0][d], &bound),
                        "degree {p}, u {u}, coordinate {d}"
                    );
                }
            }
        }
    }
}

#[test]
fn derivatives_are_within_their_bounds() {
    let mut rng = Lcg(12);
    for curve in curves() {
        let p = curve.degree();
        let order = p + 1;
        for u in parameters(&curve, &mut rng, 5) {
            for limit in LIMITS {
                let mut out = vec![[0.0; 3]; order + 1];
                curve.derivs(u, limit, &mut out).unwrap();
                let point = curve.point(u, limit).unwrap();
                for d in 0..3 {
                    assert_eq!(out[0][d].to_bits(), point[d].to_bits());
                }
                let (e, m) = exact(&curve, u, limit, order);
                for k in 0..=order {
                    let factor = if curve.is_rational() {
                        gamma((k + 1) * (14 * p + 8 + k))
                    } else if k == 0 {
                        gamma(7 * p + 1)
                    } else {
                        gamma(7 * p + 3)
                    };
                    for d in 0..3 {
                        let bound = &factor * &m[k][d];
                        assert!(
                            within(out[k][d], &e[k][d], &bound),
                            "degree {p}, rational {}, u {u}, derivative {k}, coordinate {d}",
                            curve.is_rational()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn clamped_ends_are_the_end_points() {
    for curve in curves().into_iter().filter(|c| c.basis().is_clamped()) {
        let (lo, hi) = curve.basis().domain();
        let last = curve.homogeneous().len() - 1;
        for limit in LIMITS {
            assert_eq!(
                curve.point(lo, limit).unwrap(),
                curve.control_point(0).unwrap()
            );
            assert_eq!(
                curve.point(hi, limit).unwrap(),
                curve.control_point(last).unwrap()
            );
        }
    }
}

#[test]
fn derivative_curves_are_the_derivatives() {
    let mut rng = Lcg(13);
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    for curve in curves()
        .into_iter()
        .filter(|c| !c.is_rational() && c.degree() >= 2)
    {
        let p = curve.degree();
        for k in 1..p {
            let der = match curve.derivative_curve(k, &mut meter) {
                Ok(der) => der,
                Err(NurbsError::Discontinuous) => {
                    // Only where some knot is repeated more than p − k + 1 times.
                    let u_ = curve.basis().knots();
                    assert!(u_.windows(p - k + 2).any(|w| w[0] == w[p - k + 1]));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            assert_eq!(der.degree(), p - k);
            assert_eq!(der.basis().domain(), curve.basis().domain());
            // The control points, against the differences run exactly and on magnitudes.
            let (pts, mags) = difference_points(&curve, k);
            let factor = gamma(4 * k);
            for (i, row) in der.homogeneous().iter().enumerate() {
                for d in 0..3 {
                    let bound = &factor * &mags[i][d];
                    assert!(within(row[d], &pts[i][d], &bound), "k {k}, point {i}");
                }
            }
            // Evaluated exactly, the exact differences give the exact k-th derivative.
            let dq: Vec<Vec<Q>> = pts.iter().map(|r| r.to_vec()).collect();
            for u in parameters(&curve, &mut rng, 3) {
                let (e, _) = exact(&curve, u, Limit::FromRight, k);
                let n = common::cox_de_boor(der.basis().knots(), p - k, u, Limit::FromRight);
                for d in 0..3 {
                    let mut v = zero();
                    for (i, row) in dq.iter().enumerate() {
                        v += &n[p - k][i] * &row[d];
                    }
                    assert_eq!(v, e[k][d], "k {k}, u {u}");
                }
            }
        }
    }
}

/// A3.3's differences for curve's k-th derivative, exactly and on magnitudes.
fn difference_points(curve: &Curve<3>, k: usize) -> (Vec<[Q; 3]>, Vec<[Q; 3]>) {
    let u_: Vec<Q> = curve.basis().knots().iter().map(|&x| q(x)).collect();
    let p = curve.degree();
    let mut pts: Vec<[Q; 3]> = curve
        .homogeneous()
        .iter()
        .map(|r| [q(r[0]), q(r[1]), q(r[2])])
        .collect();
    let mut mags: Vec<[Q; 3]> = pts
        .iter()
        .map(|r| [abs(&r[0]), abs(&r[1]), abs(&r[2])])
        .collect();
    let n = pts.len();
    for level in 1..=k {
        for i in 0..n - level {
            let alf = qi((p - level + 1) as i64) / (&u_[i + p + 1] - &u_[i + level]);
            for d in 0..3 {
                pts[i][d] = &alf * (&pts[i + 1][d] - &pts[i][d]);
                mags[i][d] = &alf * (&mags[i + 1][d] + &mags[i][d]);
            }
        }
    }
    pts.truncate(n - k);
    mags.truncate(n - k);
    (pts, mags)
}

#[test]
fn construction_refuses_bad_points_and_weights() {
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]).unwrap();
    let pts = [[0.0, 0.0], [1.0, 2.0], [3.0, 1.0], [4.0, 0.0]];
    assert!(Curve::new(basis.clone(), &pts).is_ok());
    assert_eq!(
        Curve::new(basis.clone(), &pts[..3]).unwrap_err(),
        NurbsError::PointCount {
            points: 3,
            expected: 4
        }
    );
    let mut bad = pts;
    bad[2][1] = f64::NAN;
    assert_eq!(
        Curve::new(basis.clone(), &bad).unwrap_err(),
        NurbsError::PointNotFinite { index: 2 }
    );
    assert_eq!(
        Curve::new_rational(basis.clone(), &pts, &[1.0, 1.0, 0.0, 1.0]).unwrap_err(),
        NurbsError::Weight { index: 2 }
    );
    assert_eq!(
        Curve::new_rational(basis.clone(), &pts, &[1.0, -1.0, 1.0, 1.0]).unwrap_err(),
        NurbsError::Weight { index: 1 }
    );
    assert_eq!(
        Curve::new_rational(basis.clone(), &pts, &[1.0, 1.0, 1.0]).unwrap_err(),
        NurbsError::PointCount {
            points: 3,
            expected: 4
        }
    );
    let huge = [[0.0, 0.0], [1e308, 0.0], [3.0, 1.0], [4.0, 0.0]];
    assert_eq!(
        Curve::new_rational(basis.clone(), &huge, &[1.0, 4.0, 1.0, 1.0]).unwrap_err(),
        NurbsError::PointNotFinite { index: 1 }
    );
    assert_eq!(
        Curve::from_homogeneous(
            basis,
            pts.to_vec(),
            Some(vec![1.0, 1.0, f64::INFINITY, 1.0])
        )
        .unwrap_err(),
        NurbsError::Weight { index: 2 }
    );
}

#[test]
fn geom_math_points_go_in_and_come_out() {
    let basis = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let a = F3::new(1.0, 2.0, 3.0).unwrap();
    let b = F3::new(3.0, 2.0, 1.0).unwrap();
    let curve = Curve::new(basis.clone(), &[a, b]).unwrap();
    let mid = F3::from_coords(curve.point(0.5, Limit::FromRight).unwrap()).unwrap();
    assert_eq!(mid, F3::new(2.0, 2.0, 2.0).unwrap());
    let flat = Curve::new(
        basis,
        &[F2::new(0.0, 1.0).unwrap(), F2::new(2.0, 3.0).unwrap()],
    )
    .unwrap();
    assert_eq!(flat.point(0.25, Limit::FromLeft).unwrap(), [0.5, 1.5]);
}

#[test]
fn evaluation_errors_are_typed() {
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]).unwrap();
    let pts = [[0.0, 0.0], [1.0, 2.0], [3.0, 1.0], [4.0, 0.0]];
    let curve = Curve::new(basis.clone(), &pts).unwrap();
    assert_eq!(
        curve.point(2.5, Limit::FromRight),
        Err(NurbsError::ParameterOutside)
    );
    assert_eq!(
        curve.derivs(0.5, Limit::FromRight, &mut []),
        Err(NurbsError::BufferLength {
            given: 0,
            needed: 1
        })
    );
    let mut long = vec![[0.0; 2]; 30];
    assert_eq!(
        curve.derivs(0.5, Limit::FromRight, &mut long),
        Err(NurbsError::DerivativeOrder { order: 29 })
    );
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    assert_eq!(
        curve.derivative_curve(2, &mut meter).unwrap_err(),
        NurbsError::Degree { degree: 0 }
    );
    assert_eq!(curve.derivative_curve(0, &mut meter).unwrap(), curve);
    let rational = Curve::new_rational(basis, &pts, &[1.0, 2.0, 1.0, 1.0]).unwrap();
    assert_eq!(
        rational.derivative_curve(1, &mut meter).unwrap_err(),
        NurbsError::Rational
    );
    // A derivative curve across a jump of its lower derivative does not exist.
    let c0 = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0]).unwrap();
    let pts5 = [
        [0.0, 0.0],
        [1.0, 2.0],
        [3.0, 1.0],
        [4.0, 0.0],
        [5.0, 5.0],
        [6.0, 1.0],
    ];
    let jump = Curve::new(c0, &pts5).unwrap();
    assert_eq!(
        jump.derivative_curve(1, &mut meter).unwrap_err(),
        NurbsError::Discontinuous
    );
    // A meter with no work left stops it before it allocates.
    let mut empty = WorkMeter::new(Budget::new(0, u64::MAX));
    assert!(matches!(
        curve.derivative_curve(1, &mut empty),
        Err(NurbsError::Stopped(_))
    ));
}
