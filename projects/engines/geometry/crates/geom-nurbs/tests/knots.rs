//! The knot operations against the exact rational reference: every result is the input curve
//! within the deviation it reports, its control points are within the a priori bound of the
//! module headers, its knots are what the operation promises, and every documented error.

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
    clippy::range_plus_one,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod common;

use common::{Lcg, Q, abs, cox_de_boor, gamma, knots, q, qi, zero};
use geom_nurbs::{Basis, Curve, Limit, NurbsError};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Curves of dimension 3, clamped and not, rational and not.
fn curves(seed: u64, clamped_only: bool) -> Vec<Curve<3>> {
    let mut rng = Lcg(seed);
    let mut out = Vec::new();
    for p in [1, 2, 3, 4, 5] {
        for kind in 0..3 {
            for clamped in [true, false] {
                if clamped_only && !clamped {
                    continue;
                }
                let n = p + 2 + rng.below(5);
                let basis = Basis::new(p, knots(&mut rng, p, n, clamped, kind)).unwrap();
                let points: Vec<[f64; 3]> = (0..basis.count())
                    .map(|_| [rng.unit() * 20.0 - 10.0, rng.unit() * 1e-3, rng.unit() * 500.0])
                    .collect();
                out.push(Curve::new(basis.clone(), &points).unwrap());
                let w: Vec<f64> = (0..basis.count()).map(|_| 0.3 + rng.unit() * 3.0).collect();
                out.push(Curve::new_rational(basis, &points, &w).unwrap());
            }
        }
    }
    out
}

/// C(u) exactly, from the curve's stored doubles.
fn exact_point(curve: &Curve<3>, u: f64) -> [Q; 3] {
    let basis = curve.basis();
    let p = basis.degree();
    let table = cox_de_boor(basis.knots(), p, u, Limit::FromRight);
    let mut a = [zero(), zero(), zero()];
    let mut w = zero();
    for (i, row) in curve.homogeneous().iter().enumerate() {
        let n = &table[p][i];
        if *n == zero() {
            continue;
        }
        for d in 0..3 {
            a[d] += n * q(row[d]);
        }
        w += n * curve.weights().map_or_else(|| qi(1), |ws| q(ws[i]));
    }
    a.map(|x| x / &w)
}

/// Checks that `new` is `old` within `deviation` at the knots and between them.
fn same_curve(old: &Curve<3>, new: &Curve<3>, deviation: f64, rng: &mut Lcg) {
    assert!(deviation.is_finite() && deviation >= 0.0);
    let (lo, hi) = old.basis().domain();
    assert_eq!(new.basis().domain(), (lo, hi));
    let mut us: Vec<f64> = new.basis().knots().iter().copied().filter(|&k| lo <= k && k <= hi).collect();
    for _ in 0..6 {
        us.push(lo + (hi - lo) * rng.unit());
    }
    let dev2 = q(deviation) * q(deviation);
    for u in us {
        let a = exact_point(old, u);
        let b = exact_point(new, u);
        let mut d2 = zero();
        for d in 0..3 {
            let diff = &a[d] - &b[d];
            d2 += &diff * &diff;
        }
        assert!(d2 <= dev2, "u {u}: the curves differ by more than {deviation}");
    }
}

/// Boehm's insertion of `u` once, in exact arithmetic, on homogeneous points (w·P, w).
fn insert_exact(knots: &mut Vec<Q>, p: usize, pts: &mut Vec<[Q; 4]>, u: &Q) {
    let k = (0..knots.len() - 1).rfind(|&i| knots[i] <= *u).unwrap();
    let mut out = Vec::new();
    for i in 0..=pts.len() {
        if i + p <= k {
            out.push(pts[i].clone());
        } else if i > k {
            out.push(pts[i - 1].clone());
        } else {
            let alpha = (u - &knots[i]) / (&knots[i + p] - &knots[i]);
            let beta = qi(1) - &alpha;
            let row: [Q; 4] = std::array::from_fn(|d| &alpha * &pts[i][d] + &beta * &pts[i - 1][d]);
            out.push(row);
        }
    }
    *pts = out;
    knots.insert(k + 1, u.clone());
}

/// The exact homogeneous points of `curve` with `x` inserted one value at a time.
fn refine_exact(curve: &Curve<3>, x: &[f64]) -> (Vec<Q>, Vec<[Q; 4]>) {
    let p = curve.degree();
    let mut kq: Vec<Q> = curve.basis().knots().iter().map(|&k| q(k)).collect();
    let mut pts: Vec<[Q; 4]> = curve
        .homogeneous()
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let w = curve.weights().map_or(1.0, |ws| ws[i]);
            [q(r[0]), q(r[1]), q(r[2]), q(w)]
        })
        .collect();
    for &v in x {
        insert_exact(&mut kq, p, &mut pts, &q(v));
    }
    (kq, pts)
}

/// Each homogeneous coordinate of `new` within γ_(10t)·M of the exact refinement.
fn a_priori(curve: &Curve<3>, new: &Curve<3>, x: &[f64]) {
    let (kq, pts) = refine_exact(curve, x);
    let new_knots: Vec<Q> = new.basis().knots().iter().map(|&k| q(k)).collect();
    assert_eq!(new_knots, kq);
    let factor = gamma(10 * x.len());
    for d in 0..4 {
        let m = curve
            .homogeneous()
            .iter()
            .enumerate()
            .map(|(i, r)| {
                if d < 3 {
                    abs(&q(r[d]))
                } else {
                    q(curve.weights().map_or(1.0, |ws| ws[i]))
                }
            })
            .fold(zero(), |a, b| if b > a { b } else { a });
        let bound = &factor * &m;
        for (i, row) in new.homogeneous().iter().enumerate() {
            let value = if d < 3 {
                row[d]
            } else if let Some(ws) = new.weights() {
                ws[i]
            } else {
                continue;
            };
            assert!(abs(&(q(value) - &pts[i][d])) <= bound, "point {i}, coordinate {d}");
        }
    }
}

#[test]
fn insertion_keeps_the_curve() {
    let mut rng = Lcg(31);
    for curve in curves(0x0001_5e47, false) {
        let p = curve.degree();
        let (lo, hi) = curve.basis().domain();
        // A new value, and an existing interior knot when there is one.
        let mut values = vec![lo + (hi - lo) * (0.1 + 0.8 * rng.unit())];
        if let Some(&k) = curve.basis().knots().iter().find(|&&k| lo < k && k < hi) {
            values.push(k);
        }
        for u in values {
            let s = curve.basis().multiplicity(u);
            let room = p.saturating_sub(s);
            for times in 1..=room {
                let out = curve.insert_knot(u, times, &mut meter()).unwrap();
                assert_eq!(out.value.basis().multiplicity(u), s + times);
                same_curve(&curve, &out.value, out.deviation, &mut rng);
                a_priori(&curve, &out.value, &vec![u; times]);
            }
            assert_eq!(
                curve.insert_knot(u, room + 1, &mut meter()).unwrap_err(),
                NurbsError::TooManyKnots
            );
        }
    }
}

#[test]
fn refinement_keeps_the_curve() {
    let mut rng = Lcg(32);
    for curve in curves(0x002e_f14e, false) {
        let p = curve.degree();
        let (lo, hi) = curve.basis().domain();
        let mut x: Vec<f64> = (0..1 + rng.below(6)).map(|_| lo + (hi - lo) * (0.01 + 0.98 * rng.unit())).collect();
        // A repeated new value and the existing interior knots (where they leave room).
        let extra = x[0];
        x.push(extra);
        for &k in curve.basis().knots() {
            if lo < k && k < hi && curve.basis().multiplicity(k) < p && !x.contains(&k) {
                x.push(k);
            }
        }
        x.sort_by(f64::total_cmp);
        let mut kept: Vec<f64> = Vec::new();
        for v in x {
            let used = kept.iter().filter(|&&s| s == v).count();
            if used + curve.basis().multiplicity(v) < p {
                kept.push(v);
            }
        }
        let x = kept;
        let out = curve.refine(&x, &mut meter()).unwrap();
        same_curve(&curve, &out.value, out.deviation, &mut rng);
        a_priori(&curve, &out.value, &x);
        // One knot at a time gives the same knots, and a curve within both deviations.
        let mut one = curve.clone();
        let mut total = 0.0;
        for &v in &x {
            let step = one.insert_knot(v, 1, &mut meter()).unwrap();
            total += step.deviation;
            one = step.value;
        }
        assert_eq!(one.basis().knots(), out.value.basis().knots());
        same_curve(&out.value, &one, out.deviation + total * 1.000_001, &mut rng);
    }
}

#[test]
fn deviations_are_rounding_sized() {
    // An exact operation's deviation is a few units in the last place of the coordinates.
    for curve in curves(0x0d0e, false) {
        let (lo, hi) = curve.basis().domain();
        let out = curve.insert_knot(lo + (hi - lo) * 0.37, 1, &mut meter()).unwrap();
        let scale = curve
            .homogeneous()
            .iter()
            .flat_map(|r| r.iter().map(|x| x.abs()))
            .fold(0.0_f64, |a, b| if b > a { b } else { a });
        let weights = curve.weights().map_or(1.0, |ws| {
            ws.iter().fold(f64::INFINITY, |a, &b| if b < a { b } else { a })
        });
        assert!(out.deviation <= 1e-12 * (1.0 + scale) / weights, "{}", out.deviation);
    }
}

#[test]
fn knots_outside_or_out_of_order_are_refused() {
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]).unwrap();
    let curve = Curve::new(basis, &[[0.0, 0.0], [1.0, 2.0], [3.0, 1.0], [4.0, 0.0]]).unwrap();
    let mut m = meter();
    for u in [0.0, 2.0, -1.0, 3.0, f64::NAN] {
        assert_eq!(curve.insert_knot(u, 1, &mut m).unwrap_err(), NurbsError::KnotOutside);
        assert_eq!(curve.refine(&[u], &mut m).unwrap_err(), NurbsError::KnotOutside);
    }
    assert_eq!(curve.refine(&[1.5, 0.5], &mut m).unwrap_err(), NurbsError::KnotOutside);
    assert_eq!(curve.refine(&[1.0, 1.0], &mut m).unwrap_err(), NurbsError::TooManyKnots);
    assert_eq!(curve.refine(&[0.5, 0.5, 0.5], &mut m).unwrap_err(), NurbsError::TooManyKnots);
    assert!(curve.refine(&[0.5, 0.5, 1.0], &mut m).is_ok());
    assert_eq!(curve.refine(&[], &mut m).unwrap().value, curve);
    let mut empty = WorkMeter::new(Budget::new(3, u64::MAX));
    assert!(matches!(curve.insert_knot(0.5, 1, &mut empty), Err(NurbsError::Stopped(_))));
}

/// Checks that `piece` is `old` on its own domain within `deviation`.
fn same_on_piece(old: &Curve<3>, piece: &Curve<3>, deviation: f64, rng: &mut Lcg) {
    let (a, b) = piece.basis().domain();
    let dev2 = q(deviation) * q(deviation);
    for u in [a, b, a + (b - a) * rng.unit(), a + (b - a) * rng.unit()] {
        // At the piece's right end, the old curve is taken from the left.
        let table_limit = if u == b { Limit::FromLeft } else { Limit::FromRight };
        let x = exact_point_limit(old, u, table_limit);
        let y = exact_point(piece, u);
        let mut d2 = zero();
        for d in 0..3 {
            let diff = &x[d] - &y[d];
            d2 += &diff * &diff;
        }
        assert!(d2 <= dev2, "u {u}: the piece differs by more than {deviation}");
    }
}

fn exact_point_limit(curve: &Curve<3>, u: f64, limit: Limit) -> [Q; 3] {
    let basis = curve.basis();
    let p = basis.degree();
    let table = cox_de_boor(basis.knots(), p, u, limit);
    let mut a = [zero(), zero(), zero()];
    let mut w = zero();
    for (i, row) in curve.homogeneous().iter().enumerate() {
        let n = &table[p][i];
        for d in 0..3 {
            a[d] += n * q(row[d]);
        }
        w += n * curve.weights().map_or_else(|| qi(1), |ws| q(ws[i]));
    }
    a.map(|x| x / &w)
}

#[test]
fn bezier_pieces_are_the_curve() {
    let mut rng = Lcg(41);
    for curve in curves(0x00be_21e5, true) {
        let p = curve.degree();
        let out = curve.to_bezier(&mut meter()).unwrap();
        let mut distinct: Vec<f64> = curve.basis().knots().to_vec();
        distinct.dedup();
        assert_eq!(out.value.len(), distinct.len() - 1);
        for (piece, w) in out.value.iter().zip(distinct.windows(2)) {
            assert_eq!(piece.degree(), p);
            assert_eq!(piece.basis().domain(), (w[0], w[1]));
            assert!(piece.basis().is_clamped() && piece.homogeneous().len() == p + 1);
            same_on_piece(&curve, piece, out.deviation, &mut rng);
        }
        // Without interior knots of multiplicity p + 1, the pieces are the exact refinement's
        // points, p apart, within γ_(10t)·M.
        let (lo, hi) = curve.basis().domain();
        let interior: Vec<f64> = distinct.iter().copied().filter(|&k| lo < k && k < hi).collect();
        if interior.iter().all(|&k| curve.basis().multiplicity(k) <= p) {
            let mut x = Vec::new();
            for &k in &interior {
                for _ in curve.basis().multiplicity(k)..p {
                    x.push(k);
                }
            }
            let (_, pts) = refine_exact(&curve, &x);
            let factor = gamma(10 * x.len().max(1));
            for (k, piece) in out.value.iter().enumerate() {
                for (j, row) in piece.homogeneous().iter().enumerate() {
                    for d in 0..3 {
                        let m = curve
                            .homogeneous()
                            .iter()
                            .map(|r| abs(&q(r[d])))
                            .fold(zero(), |a, b| if b > a { b } else { a });
                        assert!(abs(&(q(row[d]) - &pts[k * p + j][d])) <= &factor * &m);
                    }
                }
            }
        }
    }
    let unclamped = curves(7, false).into_iter().find(|c| !c.basis().is_clamped()).unwrap();
    assert_eq!(unclamped.to_bezier(&mut meter()).unwrap_err(), NurbsError::NotClamped);
}

#[test]
fn elevation_keeps_the_curve() {
    let mut rng = Lcg(42);
    for curve in curves(0x0e1e_0a7e, true) {
        let p = curve.degree();
        let (lo, hi) = curve.basis().domain();
        let full = curve
            .basis()
            .knots()
            .iter()
            .any(|&k| lo < k && k < hi && curve.basis().multiplicity(k) > p);
        for t in [1, 2, 3] {
            let out = curve.elevate(t, &mut meter());
            if full {
                assert_eq!(out.unwrap_err(), NurbsError::Discontinuous);
                continue;
            }
            let out = out.unwrap();
            assert_eq!(out.value.degree(), p + t);
            for &k in curve.basis().knots() {
                assert_eq!(
                    out.value.basis().multiplicity(k),
                    curve.basis().multiplicity(k) + t,
                    "knot {k}"
                );
            }
            same_curve(&curve, &out.value, out.deviation, &mut rng);
            let scale = curve
                .homogeneous()
                .iter()
                .flat_map(|r| r.iter().map(|x| x.abs()))
                .fold(1.0_f64, |a, b| if b > a { b } else { a });
            assert!(out.deviation < 1e-9 * scale, "deviation {}", out.deviation);
        }
    }
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let c = Curve::new(basis, &[[0.0; 2], [1.0, 1.0], [2.0, 1.0], [3.0, 0.0]]).unwrap();
    assert_eq!(c.elevate(26, &mut meter()).unwrap_err(), NurbsError::Degree { degree: 29 });
    assert_eq!(c.elevate(0, &mut meter()).unwrap().value, c);
    let unclamped = curves(7, false).into_iter().find(|c| !c.basis().is_clamped()).unwrap();
    assert_eq!(unclamped.elevate(1, &mut meter()).unwrap_err(), NurbsError::NotClamped);
}
