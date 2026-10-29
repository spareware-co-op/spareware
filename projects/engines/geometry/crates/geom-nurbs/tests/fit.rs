//! Fitting (piece G58) against the exact rational reference: every certified error holds for the
//! exact curve or surface of the returned doubles, interpolants pass through their data, least
//! squares finds the least-squares solution (compared with the exact normal equations), the
//! constraints of A9.6 are met, the fit to a stated error stays within it, and every documented
//! refusal.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::many_single_char_names,
    clippy::doc_markdown,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::range_plus_one,
    clippy::too_many_lines,
    clippy::expect_used,
    clippy::manual_midpoint,
    clippy::assigning_clones,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod common;

use common::{Lcg, Q, abs, cox_de_boor, derivatives, q, qi, zero};
use geom_nurbs::{
    Basis, Constraint, Curve, CurveFit, FitVersion, KnotMode, Limit, NurbsError, Parametrization,
    Surface, averaged_knots, grid_parameters, least_squares_knots, parameters,
};
use geom_run::{Budget, WorkMeter};

const V: FitVersion = FitVersion::V1;
const RULES: [Parametrization; 3] = [
    Parametrization::Uniform,
    Parametrization::ChordLength,
    Parametrization::Centripetal,
];

/// sin(πx), correctly rounded on every target.
fn sinpi(x: f64) -> f64 {
    geom_num::math::sinpi(x).unwrap().get()
}

/// cos(πx), correctly rounded on every target.
fn cospi(x: f64) -> f64 {
    geom_num::math::cospi(x).unwrap().get()
}

/// √x, correctly rounded.
fn sqrt(x: f64) -> f64 {
    geom_num::math::sqrt(x).unwrap().get()
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The exact k-th derivative of a nonrational curve at u, from the right (inside at the ends).
fn exact<const D: usize>(c: &Curve<D>, u: f64, k: usize) -> [Q; D] {
    let b = c.basis();
    let ders = derivatives(b.knots(), b.degree(), u, Limit::FromRight, k);
    let n = &ders[k];
    let mut out: [Q; D] = core::array::from_fn(|_| zero());
    for (i, p) in c.homogeneous().iter().enumerate() {
        for d in 0..D {
            out[d] += &n[i] * q(p[d]);
        }
    }
    out
}

/// The exact point of a nonrational surface at (u, v).
fn exact_surface<const D: usize>(s: &Surface<D>, u: f64, v: f64) -> [Q; D] {
    let (bu, bv) = (s.basis_u(), s.basis_v());
    let nu = cox_de_boor(bu.knots(), bu.degree(), u, Limit::FromRight);
    let nv = cox_de_boor(bv.knots(), bv.degree(), v, Limit::FromRight);
    let (nu, nv) = (&nu[bu.degree()], &nv[bv.degree()]);
    let cols = bv.count();
    let mut out: [Q; D] = core::array::from_fn(|_| zero());
    for (idx, p) in s.homogeneous().iter().enumerate() {
        let (i, j) = (idx / cols, idx % cols);
        let w = &nu[i] * &nv[j];
        if w == zero() {
            continue;
        }
        for d in 0..D {
            out[d] += &w * q(p[d]);
        }
    }
    out
}

/// |a − b|² ≤ bound², exactly.
fn certified<const D: usize>(a: [f64; D], b: &[Q; D], bound: f64) -> bool {
    let mut s = zero();
    for d in 0..D {
        let x = q(a[d]) - &b[d];
        s += &x * &x;
    }
    s <= q(bound) * q(bound)
}

/// A curve's certificate: every data point within `error` of the exact curve at its parameter.
fn check_fit<const D: usize>(fit: &CurveFit<D>, data: &[[f64; D]]) {
    assert_eq!(fit.parameters.len(), data.len());
    for (k, (p, &u)) in data.iter().zip(&fit.parameters).enumerate() {
        let c = exact(&fit.curve, u, 0);
        assert!(
            certified(*p, &c, fit.error),
            "point {k} beyond the certified error {}",
            fit.error
        );
    }
}

/// Random 3D points along a wandering polygon (no two neighbours coincide), scaled by `scale`.
fn polygon(rng: &mut Lcg, count: usize, scale: f64) -> Vec<[f64; 3]> {
    let mut p = [rng.unit(), rng.unit(), rng.unit()];
    let mut out = Vec::new();
    for _ in 0..count {
        out.push([p[0] * scale, p[1] * scale, p[2] * scale]);
        for x in &mut p {
            *x += 0.2 + rng.unit();
        }
        p[1] -= 0.7 * rng.unit();
    }
    out
}

/// Exact Gaussian elimination: the solution of a·x = b (a square and nonsingular).
fn solve_exact(mut a: Vec<Vec<Q>>, mut b: Vec<Vec<Q>>) -> Vec<Vec<Q>> {
    let n = a.len();
    for k in 0..n {
        let piv = (k..n).find(|&i| a[i][k] != zero()).expect("singular");
        a.swap(k, piv);
        b.swap(k, piv);
        for i in k + 1..n {
            if a[i][k] == zero() {
                continue;
            }
            let l = &a[i][k] / &a[k][k];
            for j in k..n {
                let t = &l * &a[k][j];
                a[i][j] -= t;
            }
            for c in 0..b[i].len() {
                let t = &l * &b[k][c];
                b[i][c] -= t;
            }
        }
    }
    let mut x = vec![vec![zero(); b[0].len()]; n];
    for i in (0..n).rev() {
        for c in 0..b[i].len() {
            let mut s = b[i][c].clone();
            for j in i + 1..n {
                s -= &a[i][j] * &x[j][c];
            }
            x[i][c] = s / &a[i][i];
        }
    }
    x
}

#[test]
fn versions_are_numbered() {
    assert_eq!(FitVersion::CURRENT, FitVersion::V1);
    assert_eq!(FitVersion::V1.number(), 1);
}

#[test]
fn parameters_follow_the_three_rules() {
    // Chords 3, 4 and 5: chord length gives 0, 3/12, 7/12, 1.
    let pts = [[0.0, 0.0], [3.0, 0.0], [3.0, 4.0], [0.0, 0.0]];
    let m = &mut meter();
    let u = parameters(&pts, Parametrization::ChordLength, m).unwrap();
    assert_eq!(u, vec![0.0, 3.0 / 12.0, 7.0 / 12.0, 1.0]);
    let u = parameters(&pts, Parametrization::Uniform, m).unwrap();
    assert_eq!(u, vec![0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]);
    let u = parameters(&pts, Parametrization::Centripetal, m).unwrap();
    let (a, b, c) = (sqrt(3.0), 2.0, sqrt(5.0));
    let total = a + b + c;
    assert_eq!(u, vec![0.0, a / total, (a + b) / total, 1.0]);
    // Coincident neighbours, and every point the same.
    let dup = [[0.0, 0.0], [1.0, 0.0], [1.0, 0.0], [2.0, 0.0]];
    for rule in [Parametrization::ChordLength, Parametrization::Centripetal] {
        assert_eq!(
            parameters(&dup, rule, m),
            Err(NurbsError::CoincidentData { index: 2 })
        );
        assert_eq!(
            parameters(&[[1.0, 1.0]; 3], rule, m),
            Err(NurbsError::CoincidentData { index: 1 })
        );
    }
    // Two points so close, relative to the polygon, that their parameters round to one value.
    let tiny = [[0.0, 0.0], [1.0, 0.0], [1.0 + 1e-17, 0.0]];
    assert_eq!(
        parameters(&tiny, Parametrization::ChordLength, m),
        Err(NurbsError::CoincidentData { index: 2 })
    );
    assert_eq!(
        parameters(&[[0.0, 0.0]], Parametrization::Uniform, m),
        Err(NurbsError::DataCount {
            points: 1,
            needed: 2
        })
    );
    assert_eq!(
        parameters(&[[0.0, 0.0], [f64::NAN, 1.0]], Parametrization::Uniform, m),
        Err(NurbsError::DataNotFinite { index: 1 })
    );
}

#[test]
fn grid_parameters_average_the_rows_and_columns() {
    // Two rows; columns 0..3 at x = 0, 1, 3, 6 in the first row and 0, 2, 3, 6 in the second.
    let grid = [
        [0.0, 0.0],
        [1.0, 0.0],
        [3.0, 0.0],
        [6.0, 0.0],
        [0.0, 1.0],
        [2.0, 1.0],
        [3.0, 1.0],
        [6.0, 1.0],
    ];
    let m = &mut meter();
    let (u, v) = grid_parameters(&grid, (2, 4), Parametrization::ChordLength, m).unwrap();
    assert_eq!(u, vec![0.0, 1.0]);
    assert_eq!(v, vec![0.0, (1.0 / 6.0 + 2.0 / 6.0) / 2.0, 0.5, 1.0]);
    // A degenerate row (a pole) is left out of the average.
    let pole = [
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 1.0],
        [1.0, 1.0],
        [3.0, 1.0],
    ];
    let (_, v) = grid_parameters(&pole, (2, 3), Parametrization::ChordLength, m).unwrap();
    assert_eq!(v, vec![0.0, 1.0 / 3.0, 1.0]);
    assert_eq!(
        grid_parameters(&pole, (3, 3), Parametrization::ChordLength, m),
        Err(NurbsError::PointCount {
            points: 6,
            expected: 9
        })
    );
}

#[test]
fn knots_follow_equations_9_8_9_68_and_9_69() {
    let m = &mut meter();
    let u = [0.0, 0.25, 0.5, 0.75, 1.0];
    let b = averaged_knots(&u, 2, m).unwrap();
    assert_eq!(b.knots(), &[0.0, 0.0, 0.0, 0.375, 0.625, 1.0, 1.0, 1.0]);
    let b = averaged_knots(&u, 1, m).unwrap();
    assert_eq!(b.knots(), &[0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0]);
    assert_eq!(
        averaged_knots(&u, 5, m),
        Err(NurbsError::DataCount {
            points: 5,
            needed: 6
        })
    );
    assert_eq!(
        averaged_knots(&[0.0, 0.5, 0.5, 1.0], 1, m),
        Err(NurbsError::ParameterOrder { index: 2 })
    );
    // Least squares: every span holds a parameter, and each knot is the book's interpolated value.
    let mut rng = Lcg(5);
    for trial in 0..200 {
        let count = 4 + rng.below(40);
        let mut u: Vec<f64> = (0..count).map(|_| rng.unit()).collect();
        u.sort_by(f64::total_cmp);
        u.dedup();
        u[0] = 0.0;
        let last = u.len() - 1;
        u[last] = 1.0;
        let p = 1 + rng.below(4);
        if u.len() < p + 1 {
            continue;
        }
        let n_ctrl = p + 1 + rng.below(u.len() - p);
        let b = least_squares_knots(&u, n_ctrl, p, m).unwrap();
        assert_eq!(b.count(), n_ctrl, "trial {trial}");
        let k = b.knots();
        let den = n_ctrl - p;
        for j in 1..den {
            let i = j * u.len() / den;
            let alpha = Q::from(dashu_int::IBig::from((j * u.len() % den) as i64)) / qi(den as i64);
            let exact = q(u[i - 1]) + alpha * (q(u[i]) - q(u[i - 1]));
            let got = k[p + j];
            assert!(
                abs(&(q(got) - exact)) <= q(4.0 * f64::EPSILON),
                "trial {trial} knot {j}"
            );
        }
        for s in p..b.count() {
            if k[s] < k[s + 1] {
                assert!(
                    u.iter().any(|&x| k[s] <= x && x <= k[s + 1]),
                    "trial {trial}: span {s} holds no parameter"
                );
            }
        }
    }
    assert_eq!(
        least_squares_knots(&u, 2, 2, m),
        Err(NurbsError::ControlCount {
            count: 2,
            least: 3,
            most: 5
        })
    );
}

#[test]
fn interpolants_pass_through_their_data_certified() {
    let mut rng = Lcg(11);
    for trial in 0..120 {
        let count = 2 + rng.below(30);
        let scale = [1.0, 1e-3, 1e4][trial % 3];
        let data = polygon(&mut rng, count, scale);
        let rule = RULES[trial % 3];
        for p in 1..=5.min(count - 1) {
            let fit = Curve::interpolate(&data, p, rule, V, &mut meter()).unwrap();
            assert_eq!(fit.curve.degree(), p);
            assert_eq!(fit.curve.homogeneous().len(), count);
            assert_eq!(
                fit.parameters,
                parameters(&data, rule, &mut meter()).unwrap()
            );
            assert_eq!(fit.constraint_error, 0.0);
            assert_eq!(fit.version, V);
            check_fit(&fit, &data);
            // Interpolation's error is rounding, not approximation.
            assert!(
                fit.error <= 1e-9 * scale * count as f64,
                "trial {trial} p {p}: {}",
                fit.error
            );
            // The ends are the end points exactly.
            assert_eq!(fit.curve.homogeneous()[0], data[0]);
            assert_eq!(fit.curve.homogeneous()[count - 1], data[count - 1]);
        }
    }
}

#[test]
fn interpolation_at_given_parameters_and_its_refusals() {
    let m = &mut meter();
    let data = [
        [0.0, 0.0],
        [1.0, 2.0],
        [2.0, -1.0],
        [3.0, 0.5],
        [4.0, 1.0],
        [5.0, 0.0],
    ];
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0]).unwrap();
    let u = [0.0, 0.1, 0.35, 0.6, 0.85, 1.0];
    let fit = Curve::interpolate_at(&data, &u, basis.clone(), V, m).unwrap();
    check_fit(&fit, &data);
    assert!(fit.error < 1e-13);
    // Schoenberg–Whitney broken: no parameter where N_4 lives.
    let crowded = [0.0, 0.05, 0.1, 0.15, 0.2, 1.0];
    assert_eq!(
        Curve::interpolate_at(&data, &crowded, basis.clone(), V, m),
        Err(NurbsError::Undetermined)
    );
    assert_eq!(
        Curve::interpolate_at(&data, &[0.0, 0.1, 0.1, 0.6, 0.85, 1.0], basis.clone(), V, m),
        Err(NurbsError::ParameterOrder { index: 2 })
    );
    assert_eq!(
        Curve::interpolate_at(
            &data,
            &[0.0, 0.1, 0.35, 0.6, 0.85, 1.5],
            basis.clone(),
            V,
            m
        ),
        Err(NurbsError::ParameterOutside)
    );
    assert_eq!(
        Curve::interpolate_at(&data, &u[..5], basis.clone(), V, m),
        Err(NurbsError::BufferLength {
            given: 5,
            needed: 6
        })
    );
    assert_eq!(
        Curve::interpolate_at(&data[..5], &u[..5], basis, V, m),
        Err(NurbsError::PointCount {
            points: 5,
            expected: 6
        })
    );
    // Degrees and counts.
    for p in [0, 29] {
        assert_eq!(
            Curve::interpolate(&data, p, Parametrization::ChordLength, V, m),
            Err(NurbsError::Degree { degree: p })
        );
    }
    assert_eq!(
        Curve::interpolate(&data, 6, Parametrization::ChordLength, V, m),
        Err(NurbsError::DataCount {
            points: 6,
            needed: 7
        })
    );
}

#[test]
fn end_derivatives_are_met_certified() {
    let mut rng = Lcg(23);
    for trial in 0..80 {
        let count = 2 + rng.below(20);
        let data = polygon(&mut rng, count, 10.0);
        let d0 = [rng.unit() * 20.0 - 10.0, rng.unit() * 20.0, 5.0];
        let dm = [rng.unit() * 20.0 - 10.0, -3.0, rng.unit() * 20.0];
        for p in 2..=(count + 1).min(5) {
            let rule = RULES[(trial + p) % 3];
            let fit =
                Curve::interpolate_with_ends(&data, [d0, dm], p, rule, V, &mut meter()).unwrap();
            assert_eq!(fit.curve.homogeneous().len(), count + 2);
            check_fit(&fit, &data);
            for (u, d) in [(0.0, d0), (1.0, dm)] {
                let c = exact(&fit.curve, u, 1);
                assert!(
                    certified(d, &c, fit.constraint_error),
                    "trial {trial} p {p}"
                );
            }
            assert!(
                fit.error < 1e-9 && fit.constraint_error < 1e-9,
                "trial {trial} p {p}"
            );
        }
    }
    let m = &mut meter();
    let data = [[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]];
    let ends = [[1.0, 0.0], [1.0, 0.0]];
    assert_eq!(
        Curve::interpolate_with_ends(&data, ends, 1, Parametrization::ChordLength, V, m),
        Err(NurbsError::Degree { degree: 1 })
    );
    assert_eq!(
        Curve::interpolate_with_ends(&data, ends, 5, Parametrization::ChordLength, V, m),
        Err(NurbsError::DataCount {
            points: 3,
            needed: 4
        })
    );
    assert_eq!(
        Curve::interpolate_with_ends(
            &data,
            [[1.0, 0.0], [f64::INFINITY, 0.0]],
            2,
            Parametrization::ChordLength,
            V,
            m
        ),
        Err(NurbsError::DataNotFinite { index: 4 })
    );
}

/// A smooth height field on a grid of `rows` × `cols`, with some noise.
fn grid(rng: &mut Lcg, rows: usize, cols: usize) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    for i in 0..rows {
        for j in 0..cols {
            let (x, y) = (
                i as f64 + 0.3 * rng.unit(),
                j as f64 * 1.5 + 0.3 * rng.unit(),
            );
            out.push([x, y, sinpi(x * 0.2) * cospi(y * 0.13) + 0.05 * rng.unit()]);
        }
    }
    out
}

#[test]
fn surface_interpolants_pass_through_the_grid_certified() {
    let mut rng = Lcg(31);
    for trial in 0..24 {
        let (rows, cols) = (2 + rng.below(8), 2 + rng.below(8));
        let data = grid(&mut rng, rows, cols);
        for p in 1..=3.min(rows - 1) {
            for qd in 1..=3.min(cols - 1) {
                let rule = RULES[(trial + p + qd) % 3];
                let fit = Surface::interpolate(&data, (rows, cols), (p, qd), rule, V, &mut meter())
                    .unwrap();
                assert_eq!(fit.u.len(), rows);
                assert_eq!(fit.v.len(), cols);
                for k in 0..rows {
                    for l in 0..cols {
                        let s = exact_surface(&fit.surface, fit.u[k], fit.v[l]);
                        assert!(
                            certified(data[k * cols + l], &s, fit.error),
                            "trial {trial} ({p}, {qd}) point ({k}, {l})"
                        );
                    }
                }
                assert!(fit.error < 1e-10, "trial {trial}: {}", fit.error);
            }
        }
    }
    let m = &mut meter();
    let data = grid(&mut Lcg(1), 3, 4);
    assert_eq!(
        Surface::interpolate(&data, (3, 4), (3, 1), Parametrization::ChordLength, V, m),
        Err(NurbsError::DataCount {
            points: 3,
            needed: 4
        })
    );
    assert_eq!(
        Surface::interpolate(&data, (4, 4), (1, 1), Parametrization::ChordLength, V, m),
        Err(NurbsError::PointCount {
            points: 12,
            expected: 16
        })
    );
}

/// The exact least-squares control points with fixed ends: the normal equations of the interior
/// points solved in rationals.
fn exact_fixed_ends(data: &[[f64; 3]], u: &[f64], basis: &Basis) -> Vec<[Q; 3]> {
    let (p, n) = (basis.degree(), basis.count() - 1);
    let m = data.len() - 1;
    let rows: Vec<Vec<Q>> = u
        .iter()
        .map(|&x| cox_de_boor(basis.knots(), p, x, Limit::FromRight)[p].clone())
        .collect();
    let mut ntn = vec![vec![zero(); n - 1]; n - 1];
    let mut rhs = vec![vec![zero(); 3]; n - 1];
    for k in 1..m {
        let r: Vec<Q> = (0..3)
            .map(|d| q(data[k][d]) - &rows[k][0] * q(data[0][d]) - &rows[k][n] * q(data[m][d]))
            .collect();
        for i in 1..n {
            for j in 1..n {
                let t = &rows[k][i] * &rows[k][j];
                ntn[i - 1][j - 1] += t;
            }
            for d in 0..3 {
                let t = &rows[k][i] * &r[d];
                rhs[i - 1][d] += t;
            }
        }
    }
    let x = solve_exact(ntn, rhs);
    let mut out = vec![core::array::from_fn(|d| q(data[0][d]))];
    for row in x {
        out.push([row[0].clone(), row[1].clone(), row[2].clone()]);
    }
    out.push(core::array::from_fn(|d| q(data[m][d])));
    out
}

#[test]
fn least_squares_finds_the_least_squares_curve() {
    let mut rng = Lcg(41);
    for trial in 0..60 {
        let count = 6 + rng.below(30);
        let data = polygon(&mut rng, count, 1.0);
        let p = 1 + rng.below(3);
        let n_ctrl = (p + 1 + rng.below(6)).min(count);
        let rule = RULES[trial % 3];
        let fit = Curve::least_squares(&data, n_ctrl, p, rule, V, &mut meter()).unwrap();
        check_fit(&fit, &data);
        assert_eq!(fit.curve.homogeneous().len(), n_ctrl);
        assert_eq!(fit.curve.homogeneous()[0], data[0]);
        assert_eq!(fit.curve.homogeneous()[n_ctrl - 1], data[count - 1]);
        let exact_pts = exact_fixed_ends(&data, &fit.parameters, fit.curve.basis());
        for (i, (got, want)) in fit.curve.homogeneous().iter().zip(&exact_pts).enumerate() {
            for d in 0..3 {
                assert!(
                    abs(&(q(got[d]) - &want[d])) <= q(1e-9),
                    "trial {trial} control point {i}"
                );
            }
        }
        // The certified error is at least the exact largest distance, and not far above it.
        let mut worst = zero();
        for (pt, &u) in data.iter().zip(&fit.parameters) {
            let c = exact(&fit.curve, u, 0);
            let mut s = zero();
            for d in 0..3 {
                let x = q(pt[d]) - &c[d];
                s += &x * &x;
            }
            if s > worst {
                worst = s;
            }
        }
        let e = q(fit.error);
        assert!(&e * &e >= worst);
        // Tight: within a relative 1e-9 and an absolute 1e-12 (the enclosures' width on data of
        // this size) of the exact worst distance.
        let looser = fit.error * (1.0 - 1e-9) - 1e-12;
        if looser > 0.0 {
            assert!(
                q(looser) * q(looser) <= worst,
                "trial {trial}: the bound is not tight"
            );
        }
    }
    let m = &mut meter();
    let data = polygon(&mut Lcg(2), 5, 1.0);
    assert_eq!(
        Curve::least_squares(&data, 6, 2, Parametrization::ChordLength, V, m),
        Err(NurbsError::ControlCount {
            count: 6,
            least: 3,
            most: 5
        })
    );
}

/// The exact solution of A9.6's problem by its KKT system.
fn exact_constrained(
    data: &[[f64; 3]],
    weights: &[f64],
    u: &[f64],
    basis: &Basis,
    cons: &[Constraint<3>],
) -> Vec<[Q; 3]> {
    let (p, count) = (basis.degree(), basis.count());
    let s = cons.len();
    let size = count + s;
    let mut a = vec![vec![zero(); size]; size];
    let mut b = vec![vec![zero(); 3]; size];
    for (k, &x) in u.iter().enumerate() {
        let n = &cox_de_boor(basis.knots(), p, x, Limit::FromRight)[p];
        let w = q(weights[k]);
        for i in 0..count {
            for j in 0..count {
                let t = &w * &n[i] * &n[j];
                a[i][j] += t;
            }
            for d in 0..3 {
                let t = &w * &n[i] * q(data[k][d]);
                b[i][d] += t;
            }
        }
    }
    for (r, c) in cons.iter().enumerate() {
        let row = &derivatives(basis.knots(), p, c.parameter, Limit::FromRight, c.order)[c.order];
        for i in 0..count {
            a[count + r][i] = row[i].clone();
            a[i][count + r] = row[i].clone();
        }
        for d in 0..3 {
            b[count + r][d] = q(c.value[d]);
        }
    }
    solve_exact(a, b)
        .into_iter()
        .take(count)
        .map(|r| [r[0].clone(), r[1].clone(), r[2].clone()])
        .collect()
}

#[test]
fn weighted_constrained_least_squares_meets_its_constraints() {
    let mut rng = Lcg(53);
    for trial in 0..40 {
        let count = 10 + rng.below(20);
        let data = polygon(&mut rng, count, 1.0);
        let m = &mut meter();
        let u = parameters(&data, Parametrization::ChordLength, m).unwrap();
        let p = 2 + rng.below(2);
        let n_ctrl = p + 3 + rng.below(4);
        let basis = least_squares_knots(&u, n_ctrl, p, m).unwrap();
        let weights: Vec<f64> = (0..count).map(|_| 0.5 + 1.5 * rng.unit()).collect();
        let cons = [
            Constraint {
                parameter: 0.0,
                order: 0,
                value: data[0],
            },
            Constraint {
                parameter: 0.0,
                order: 1,
                value: [2.0, 1.0, 1.5],
            },
            Constraint {
                parameter: 1.0,
                order: 0,
                value: data[count - 1],
            },
            Constraint {
                parameter: 0.5,
                order: 2,
                value: [0.0, 0.5, -1.0],
            },
        ];
        let used = &cons[..1 + trial % 4];
        let fit =
            Curve::least_squares_at(&data, Some(&weights), &u, basis.clone(), used, V, m).unwrap();
        check_fit(&fit, &data);
        for c in used {
            let e = exact(&fit.curve, c.parameter, c.order);
            assert!(
                certified(c.value, &e, fit.constraint_error),
                "trial {trial}"
            );
        }
        assert!(
            fit.constraint_error < 1e-8,
            "trial {trial}: {}",
            fit.constraint_error
        );
        let want = exact_constrained(&data, &weights, &u, &basis, used);
        for (i, (got, w)) in fit.curve.homogeneous().iter().zip(&want).enumerate() {
            for d in 0..3 {
                assert!(
                    abs(&(q(got[d]) - &w[d])) <= q(1e-8),
                    "trial {trial} control point {i}"
                );
            }
        }
        // No constraints and unit weights.
        let free = Curve::least_squares_at(&data, None, &u, basis.clone(), &[], V, m).unwrap();
        check_fit(&free, &data);
        let want = exact_constrained(&data, &vec![1.0; count], &u, &basis, &[]);
        for (got, w) in free.curve.homogeneous().iter().zip(&want) {
            for d in 0..3 {
                assert!(abs(&(q(got[d]) - &w[d])) <= q(1e-8), "trial {trial}");
            }
        }
    }
    // Refusals.
    let m = &mut meter();
    let data = polygon(&mut Lcg(3), 8, 1.0);
    let u = parameters(&data, Parametrization::ChordLength, m).unwrap();
    let basis = least_squares_knots(&u, 4, 2, m).unwrap();
    let pin = Constraint {
        parameter: 0.0,
        order: 0,
        value: data[0],
    };
    assert_eq!(
        Curve::least_squares_at(&data, None, &u, basis.clone(), &[pin; 5], V, m),
        Err(NurbsError::Constraints)
    );
    assert_eq!(
        Curve::least_squares_at(&data, None, &u, basis.clone(), &[pin, pin], V, m),
        Err(NurbsError::Constraints)
    );
    let mut bad = vec![1.0; 8];
    bad[3] = 0.0;
    assert_eq!(
        Curve::least_squares_at(&data, Some(&bad), &u, basis.clone(), &[], V, m),
        Err(NurbsError::DataWeight { index: 3 })
    );
    assert_eq!(
        Curve::least_squares_at(&data, Some(&bad[..7]), &u, basis.clone(), &[], V, m),
        Err(NurbsError::BufferLength {
            given: 7,
            needed: 8
        })
    );
    let nan = Constraint {
        parameter: 0.5,
        order: 1,
        value: [f64::NAN, 0.0, 0.0],
    };
    assert_eq!(
        Curve::least_squares_at(&data, None, &u, basis.clone(), &[nan], V, m),
        Err(NurbsError::DataNotFinite { index: 8 })
    );
    let outside = Constraint {
        parameter: 2.0,
        order: 0,
        value: data[0],
    };
    assert_eq!(
        Curve::least_squares_at(&data, None, &u, basis.clone(), &[outside], V, m),
        Err(NurbsError::ParameterOutside)
    );
    // All data in one span of a basis with many functions: not determined.
    let crowded: Vec<f64> = (0..8).map(|k| f64::from(k) / 100.0).collect();
    let wide = Basis::new(2, vec![0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0]).unwrap();
    assert_eq!(
        Curve::least_squares_at(&data, None, &crowded, wide, &[], V, m),
        Err(NurbsError::Undetermined)
    );
}

#[test]
fn surface_least_squares_is_certified_with_corners_kept() {
    let mut rng = Lcg(61);
    for trial in 0..16 {
        let (rows, cols) = (4 + rng.below(8), 4 + rng.below(8));
        let data = grid(&mut rng, rows, cols);
        let (p, qd) = (1 + rng.below(3), 1 + rng.below(3));
        let cu = (p + 1 + rng.below(3)).min(rows);
        let cv = (qd + 1 + rng.below(3)).min(cols);
        if cu <= p || cv <= qd {
            continue;
        }
        let rule = RULES[trial % 3];
        let fit = Surface::least_squares(
            &data,
            (rows, cols),
            (cu, cv),
            (p, qd),
            rule,
            V,
            &mut meter(),
        )
        .unwrap();
        assert_eq!(fit.surface.basis_u().count(), cu);
        assert_eq!(fit.surface.basis_v().count(), cv);
        let net = fit.surface.homogeneous();
        assert_eq!(net[0], data[0]);
        assert_eq!(net[cv - 1], data[cols - 1]);
        assert_eq!(net[(cu - 1) * cv], data[(rows - 1) * cols]);
        assert_eq!(net[cu * cv - 1], data[rows * cols - 1]);
        for k in 0..rows {
            for l in 0..cols {
                let s = exact_surface(&fit.surface, fit.u[k], fit.v[l]);
                assert!(
                    certified(data[k * cols + l], &s, fit.error),
                    "trial {trial} point ({k}, {l})"
                );
            }
        }
    }
}

/// Points along a helix, `count` of them.
fn helix(count: usize) -> Vec<[f64; 3]> {
    (0..count)
        .map(|k| {
            let t = k as f64 / (count - 1) as f64 * 1.3;
            [cospi(t) * 2.0, sinpi(t) * 2.0, 0.9 * t]
        })
        .collect()
}

#[test]
fn approximation_stays_within_its_tolerance() {
    for (count, tol, degree) in [(200, 1e-3, 3), (120, 1e-5, 3), (60, 1e-2, 2), (90, 1e-4, 4)] {
        let data = helix(count);
        for mode in [KnotMode::Single, KnotMode::Multiple] {
            let fit = Curve::approximate(&data, degree, tol, mode, V, &mut meter()).unwrap();
            assert_eq!(fit.curve.degree(), degree);
            assert!(fit.error <= tol, "{count} {tol}: {}", fit.error);
            check_fit(&fit, &data);
            assert_eq!(fit.parameters[0], 0.0);
            assert_eq!(fit.parameters[count - 1], 1.0);
            // Smooth data need far fewer control points than data points.
            assert!(
                fit.curve.homogeneous().len() < count / 2,
                "{count} {tol} {mode:?}: {} control points",
                fit.curve.homogeneous().len()
            );
        }
    }
    // A corner: multiple knots keep it within the tolerance.
    let mut corner: Vec<[f64; 2]> = (0..40).map(|k| [f64::from(k) / 39.0, 0.0]).collect();
    corner.extend((1..40).map(|k| [1.0, f64::from(k) / 39.0]));
    for mode in [KnotMode::Single, KnotMode::Multiple] {
        let fit = Curve::approximate(&corner, 3, 1e-4, mode, V, &mut meter()).unwrap();
        assert!(fit.error <= 1e-4);
        check_fit(&fit, &corner);
    }
    // Noisy data, loosely.
    let mut rng = Lcg(71);
    let noisy: Vec<[f64; 2]> = (0..150)
        .map(|k| {
            let x = f64::from(k) / 149.0;
            [x, sinpi(x * 2.0) + 0.01 * (rng.unit() - 0.5)]
        })
        .collect();
    let fit = Curve::approximate(&noisy, 3, 0.02, KnotMode::Single, V, &mut meter()).unwrap();
    assert!(fit.error <= 0.02);
    check_fit(&fit, &noisy);
    assert!(fit.curve.homogeneous().len() < 40);
    // Refusals.
    let m = &mut meter();
    let data = helix(10);
    for tol in [-1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            Curve::approximate(&data, 3, tol, KnotMode::Single, V, m),
            Err(NurbsError::Tolerance)
        );
    }
    assert_eq!(
        Curve::approximate(&data[..3], 3, 0.1, KnotMode::Single, V, m),
        Err(NurbsError::DataCount {
            points: 3,
            needed: 4
        })
    );
    // A tolerance below the interpolants' rounding cannot be met.
    let wild: Vec<[f64; 3]> = polygon(&mut Lcg(9), 30, 1e6);
    match Curve::approximate(&wild, 3, 0.0, KnotMode::Single, V, m) {
        Err(NurbsError::OutsideTolerance) => {}
        Ok(fit) => assert_eq!(fit.error, 0.0),
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn a_small_budget_stops_every_fit() {
    let data = helix(50);
    let tiny = || WorkMeter::new(Budget::of_work(100));
    assert!(matches!(
        Curve::interpolate(&data, 3, Parametrization::ChordLength, V, &mut tiny()),
        Err(NurbsError::Stopped(_))
    ));
    assert!(matches!(
        Curve::least_squares(&data, 10, 3, Parametrization::ChordLength, V, &mut tiny()),
        Err(NurbsError::Stopped(_))
    ));
    assert!(matches!(
        Curve::approximate(&data, 3, 1e-3, KnotMode::Single, V, &mut tiny()),
        Err(NurbsError::Stopped(_))
    ));
}
