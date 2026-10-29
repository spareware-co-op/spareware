//! The surface knot operations: each result is the input surface within its reported deviation
//! (exact rational evaluation at the knots and between them), and each line of the net is the
//! curve operation on that line, bit for bit.

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
    clippy::type_complexity,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod common;

use common::{Lcg, Q, cox_de_boor, knots, q, qi, zero};
use geom_nurbs::{Basis, Curve, Direction, Limit, NurbsError, Surface};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn surfaces(seed: u64) -> Vec<Surface<3>> {
    let mut rng = Lcg(seed);
    let mut out = Vec::new();
    for (p, qd) in [(1, 2), (2, 3), (3, 1), (3, 3)] {
        for kind in 0..2 {
            let nu = p + 2 + rng.below(3);
            let bu = Basis::new(p, knots(&mut rng, p, nu, true, kind)).unwrap();
            let nv = qd + 2 + rng.below(3);
            let bv = Basis::new(qd, knots(&mut rng, qd, nv, true, kind + 1)).unwrap();
            let count = bu.count() * bv.count();
            let pts: Vec<[f64; 3]> = (0..count)
                .map(|_| [rng.unit() * 8.0 - 4.0, rng.unit() * 0.5, rng.unit() * 300.0])
                .collect();
            out.push(Surface::new(bu.clone(), bv.clone(), &pts).unwrap());
            let w: Vec<f64> = (0..count).map(|_| 0.4 + rng.unit() * 2.5).collect();
            out.push(Surface::new_rational(bu, bv, &pts, &w).unwrap());
        }
    }
    out
}

/// S(u, v) exactly, from the surface's stored doubles.
fn exact_point(s: &Surface<3>, u: f64, v: f64, limits: (Limit, Limit)) -> [Q; 3] {
    let (bu, bv) = (s.basis_u(), s.basis_v());
    let nu = cox_de_boor(bu.knots(), bu.degree(), u, limits.0);
    let nv = cox_de_boor(bv.knots(), bv.degree(), v, limits.1);
    let cols = bv.count();
    let mut a = [zero(), zero(), zero()];
    let mut w = zero();
    for (idx, row) in s.homogeneous().iter().enumerate() {
        let (i, j) = (idx / cols, idx % cols);
        let n = &nu[bu.degree()][i] * &nv[bv.degree()][j];
        if n == zero() {
            continue;
        }
        for d in 0..3 {
            a[d] += &n * q(row[d]);
        }
        w += &n * s.weights().map_or_else(|| qi(1), |ws| q(ws[idx]));
    }
    a.map(|x| x / &w)
}

fn params(b: &Basis, rng: &mut Lcg) -> Vec<f64> {
    let (lo, hi) = b.domain();
    let mut out: Vec<f64> = b
        .knots()
        .iter()
        .copied()
        .filter(|&k| lo <= k && k <= hi)
        .collect();
    out.dedup();
    out.push(lo + (hi - lo) * rng.unit());
    out
}

/// `new` is `old` within `deviation` on a grid of the old domains.
fn same_surface(old: &Surface<3>, new: &Surface<3>, deviation: f64, rng: &mut Lcg) {
    assert!(deviation.is_finite() && deviation >= 0.0);
    let dev2 = q(deviation) * q(deviation);
    for u in params(new.basis_u(), rng) {
        for v in params(new.basis_v(), rng) {
            let lim = (Limit::FromRight, Limit::FromRight);
            let a = exact_point(old, u, v, lim);
            let b = exact_point(new, u, v, lim);
            let mut d2 = zero();
            for d in 0..3 {
                let diff = &a[d] - &b[d];
                d2 += &diff * &diff;
            }
            assert!(
                d2 <= dev2,
                "({u}, {v}): the surfaces differ by more than {deviation}"
            );
        }
    }
}

/// Line `index` of `s` in direction `dir` as a curve.
fn line(s: &Surface<3>, dir: Direction, index: usize) -> Curve<3> {
    let cols = s.basis_v().count();
    let (basis, idx): (&Basis, Vec<usize>) = match dir {
        Direction::U => (
            s.basis_u(),
            (0..s.basis_u().count()).map(|i| i * cols + index).collect(),
        ),
        Direction::V => (s.basis_v(), (0..cols).map(|j| index * cols + j).collect()),
    };
    let pts: Vec<[f64; 3]> = idx.iter().map(|&k| s.homogeneous()[k]).collect();
    let w = s
        .weights()
        .map(|ws| idx.iter().map(|&k| ws[k]).collect::<Vec<f64>>());
    Curve::from_homogeneous(basis.clone(), pts, w).unwrap()
}

fn lines(s: &Surface<3>, dir: Direction) -> usize {
    match dir {
        Direction::U => s.basis_v().count(),
        Direction::V => s.basis_u().count(),
    }
}

#[test]
fn surface_operations_keep_the_surface_and_match_the_curves() {
    let mut rng = Lcg(71);
    for s in surfaces(0x0007_1a1a) {
        for dir in [Direction::U, Direction::V] {
            let basis = match dir {
                Direction::U => s.basis_u().clone(),
                Direction::V => s.basis_v().clone(),
            };
            let (lo, hi) = basis.domain();
            let u = lo + (hi - lo) * (0.1 + 0.8 * rng.unit());
            let x = vec![
                lo + (hi - lo) * 0.3,
                lo + (hi - lo) * 0.3,
                lo + (hi - lo) * 0.7,
            ];
            let ins = s.insert_knot(dir, u, 1, &mut meter()).unwrap();
            let refd = s.refine(dir, &x, &mut meter());
            let up = s.elevate(dir, 1, &mut meter());
            same_surface(&s, &ins.value, ins.deviation, &mut rng);
            if let Ok(up) = &up {
                same_surface(&s, &up.value, up.deviation, &mut rng);
            } else {
                // Only where a knot of this direction has p + 1 copies.
                assert_eq!(up.as_ref().unwrap_err(), &NurbsError::Discontinuous);
            }
            for k in 0..lines(&s, dir) {
                let c = line(&s, dir, k);
                let ci = c.insert_knot(u, 1, &mut meter()).unwrap().value;
                assert_eq!(line(&ins.value, dir, k), ci);
                if let Ok(up) = &up {
                    let ce = c.elevate(1, &mut meter()).unwrap().value;
                    assert_eq!(line(&up.value, dir, k), ce);
                }
            }
            match refd {
                Ok(r) => {
                    same_surface(&s, &r.value, r.deviation, &mut rng);
                    for k in 0..lines(&s, dir) {
                        let cr = line(&s, dir, k).refine(&x, &mut meter()).unwrap().value;
                        assert_eq!(line(&r.value, dir, k), cr);
                    }
                }
                Err(e) => assert_eq!(e, NurbsError::TooManyKnots),
            }
        }
        let patches = s.to_bezier(&mut meter()).unwrap();
        let mut du: Vec<f64> = s.basis_u().knots().to_vec();
        du.dedup();
        let mut dv: Vec<f64> = s.basis_v().knots().to_vec();
        dv.dedup();
        assert_eq!(patches.value.len(), (du.len() - 1) * (dv.len() - 1));
        let dev2 = q(patches.deviation) * q(patches.deviation);
        for (index, patch) in patches.value.iter().enumerate() {
            let (i, j) = (index / (dv.len() - 1), index % (dv.len() - 1));
            assert_eq!(patch.basis_u().domain(), (du[i], du[i + 1]));
            assert_eq!(patch.basis_v().domain(), (dv[j], dv[j + 1]));
            for (u, lu) in [
                (du[i], Limit::FromRight),
                (du[i + 1], Limit::FromLeft),
                (f64::midpoint(du[i], du[i + 1]), Limit::FromRight),
            ] {
                for (v, lv) in [
                    (dv[j], Limit::FromRight),
                    (dv[j + 1], Limit::FromLeft),
                    (f64::midpoint(dv[j], dv[j + 1]), Limit::FromRight),
                ] {
                    let a = exact_point(&s, u, v, (lu, lv));
                    let b = exact_point(patch, u, v, (Limit::FromRight, Limit::FromRight));
                    let mut d2 = zero();
                    for d in 0..3 {
                        let diff = &a[d] - &b[d];
                        d2 += &diff * &diff;
                    }
                    assert!(d2 <= dev2, "patch ({i}, {j}) at ({u}, {v})");
                }
            }
        }
    }
}

#[test]
fn surface_operations_refuse_as_the_curves_do() {
    let bu = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]).unwrap();
    let bv = Basis::new(1, vec![0.0, 1.0, 2.0, 3.0]).unwrap();
    let s = Surface::new(bu, bv, &[[0.0; 3]; 8]).unwrap();
    let mut m = meter();
    assert_eq!(
        s.insert_knot(Direction::U, 2.0, 1, &mut m).unwrap_err(),
        NurbsError::KnotOutside
    );
    assert_eq!(
        s.insert_knot(Direction::U, 1.0, 2, &mut m).unwrap_err(),
        NurbsError::TooManyKnots
    );
    assert_eq!(
        s.elevate(Direction::V, 1, &mut m).unwrap_err(),
        NurbsError::NotClamped
    );
    assert_eq!(s.to_bezier(&mut m).unwrap_err(), NurbsError::NotClamped);
    assert_eq!(
        s.elevate(Direction::U, 27, &mut m).unwrap_err(),
        NurbsError::Degree { degree: 29 }
    );
    assert!(s.insert_knot(Direction::V, 1.5, 1, &mut m).is_ok());
}

#[test]
fn derivative_surfaces_are_the_partial_derivatives() {
    let mut rng = Lcg(72);
    for s in surfaces(0x0007_2b2b)
        .into_iter()
        .filter(|s| !s.is_rational())
    {
        let (p, qd) = (s.basis_u().degree(), s.basis_v().degree());
        for (k, l) in [(1, 0), (0, 1), (1, 1), (2, 0), (2, 1)] {
            if (k > 0 && k >= p) || (l > 0 && l >= qd) {
                assert!(matches!(
                    s.derivative_surface((k, l), &mut meter()),
                    Err(NurbsError::Degree { .. })
                ));
                continue;
            }
            let der = match s.derivative_surface((k, l), &mut meter()) {
                Ok(d) => d,
                Err(NurbsError::Discontinuous) => continue,
                Err(e) => panic!("{e}"),
            };
            assert_eq!(
                (der.basis_u().degree(), der.basis_v().degree()),
                (p - k, qd - l)
            );
            // Exact differences of the exact net, and their magnitudes.
            let (net, mags) = exact_differences(&s, k, l);
            let g = common::gamma(4 * (k + l));
            for (idx, row) in der.homogeneous().iter().enumerate() {
                for d in 0..3 {
                    assert!(common::within(row[d], &net[idx][d], &(&g * &mags[idx][d])));
                }
            }
            // Evaluated exactly, the exact differences give the exact partial derivative.
            let cols = der.basis_v().count();
            for u in params(der.basis_u(), &mut rng) {
                for v in params(der.basis_v(), &mut rng) {
                    let lim = (Limit::FromRight, Limit::FromRight);
                    let nu = cox_de_boor(der.basis_u().knots(), p - k, u, lim.0);
                    let nv = cox_de_boor(der.basis_v().knots(), qd - l, v, lim.1);
                    let du = common::derivatives(s.basis_u().knots(), p, u, lim.0, k);
                    let dv = common::derivatives(s.basis_v().knots(), qd, v, lim.1, l);
                    let scols = s.basis_v().count();
                    for d in 0..3 {
                        let mut a = zero();
                        for (idx, row) in net.iter().enumerate() {
                            a += &nu[p - k][idx / cols] * &nv[qd - l][idx % cols] * &row[d];
                        }
                        let mut b = zero();
                        for (idx, row) in s.homogeneous().iter().enumerate() {
                            b += &du[k][idx / scols] * &dv[l][idx % scols] * q(row[d]);
                        }
                        assert_eq!(a, b, "({k}, {l}) at ({u}, {v})");
                    }
                }
            }
        }
    }
    let rational = surfaces(1).into_iter().find(Surface::is_rational).unwrap();
    assert_eq!(
        rational
            .derivative_surface((1, 0), &mut meter())
            .unwrap_err(),
        NurbsError::Rational
    );
}

/// A3.7's differences for the (k, l) derivative net, exactly and on magnitudes, row by row.
fn exact_differences(s: &Surface<3>, k: usize, l: usize) -> (Vec<[Q; 3]>, Vec<[Q; 3]>) {
    let (bu, bv) = (s.basis_u(), s.basis_v());
    let (p, qd) = (bu.degree(), bv.degree());
    let ku: Vec<Q> = bu.knots().iter().map(|&x| q(x)).collect();
    let kv: Vec<Q> = bv.knots().iter().map(|&x| q(x)).collect();
    let (mut rows, mut cols) = (bu.count(), bv.count());
    let mut net: Vec<[Q; 3]> = s
        .homogeneous()
        .iter()
        .map(|r| [q(r[0]), q(r[1]), q(r[2])])
        .collect();
    let mut mags: Vec<[Q; 3]> = net
        .iter()
        .map(|r| [common::abs(&r[0]), common::abs(&r[1]), common::abs(&r[2])])
        .collect();
    let diff =
        |line: &mut Vec<[Q; 3]>, mag: &mut Vec<[Q; 3]>, knots: &[Q], deg: usize, order: usize| {
            let n = line.len();
            for level in 1..=order {
                for i in 0..n - level {
                    let alf =
                        qi((deg - level + 1) as i64) / (&knots[i + deg + 1] - &knots[i + level]);
                    for d in 0..3 {
                        line[i][d] = &alf * (&line[i + 1][d] - &line[i][d]);
                        mag[i][d] = &alf * (&mag[i + 1][d] + &mag[i][d]);
                    }
                }
            }
            line.truncate(n - order);
            mag.truncate(n - order);
        };
    if k > 0 {
        let mut cols_out: Vec<(Vec<[Q; 3]>, Vec<[Q; 3]>)> = Vec::new();
        for j in 0..cols {
            let mut line: Vec<[Q; 3]> = (0..rows).map(|i| net[i * cols + j].clone()).collect();
            let mut mag: Vec<[Q; 3]> = (0..rows).map(|i| mags[i * cols + j].clone()).collect();
            diff(&mut line, &mut mag, &ku, p, k);
            cols_out.push((line, mag));
        }
        rows -= k;
        net = (0..rows)
            .flat_map(|i| {
                cols_out
                    .iter()
                    .map(move |(c, _)| c[i].clone())
                    .collect::<Vec<_>>()
            })
            .collect();
        mags = (0..rows)
            .flat_map(|i| {
                cols_out
                    .iter()
                    .map(move |(_, m)| m[i].clone())
                    .collect::<Vec<_>>()
            })
            .collect();
    }
    if l > 0 {
        let mut n2 = Vec::new();
        let mut m2 = Vec::new();
        for i in 0..rows {
            let mut line: Vec<[Q; 3]> = net[i * cols..(i + 1) * cols].to_vec();
            let mut mag: Vec<[Q; 3]> = mags[i * cols..(i + 1) * cols].to_vec();
            diff(&mut line, &mut mag, &kv, qd, l);
            n2.extend(line);
            m2.extend(mag);
        }
        cols -= l;
        net = n2;
        mags = m2;
    }
    let _ = cols;
    (net, mags)
}
