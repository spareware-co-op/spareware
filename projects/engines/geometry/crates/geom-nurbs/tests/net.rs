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
