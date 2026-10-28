//! Attacks on piece G58's claims: inputs chosen to break the fits' certificates, their refusals and
//! their determinism. Each test names the claim it attacks. The certificate is always checked
//! against the exact rational value of the returned curve or surface, so a fit may refuse an
//! attack, but it may never return a bound that is false.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::doc_markdown,
    clippy::needless_range_loop,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod common;

use common::{Lcg, Q, derivatives, q, zero};
use geom_nurbs::{
    Constraint, Curve, CurveFit, FitVersion, KnotMode, Limit, MAX_DEGREE, NurbsError,
    Parametrization, Surface, least_squares_knots, parameters,
};
use geom_run::{Budget, WorkMeter};

const V: FitVersion = FitVersion::V1;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The exact k-th derivative of a nonrational curve at u.
fn exact<const D: usize>(c: &Curve<D>, u: f64, k: usize) -> [Q; D] {
    let b = c.basis();
    let ders = derivatives(b.knots(), b.degree(), u, Limit::FromRight, k);
    let mut out: [Q; D] = core::array::from_fn(|_| zero());
    for (i, p) in c.homogeneous().iter().enumerate() {
        for d in 0..D {
            out[d] += &ders[k][i] * q(p[d]);
        }
    }
    out
}

/// |a − b| ≤ bound, exactly.
fn within<const D: usize>(a: [f64; D], b: &[Q; D], bound: f64) -> bool {
    let mut s = zero();
    for d in 0..D {
        let x = q(a[d]) - &b[d];
        s += &x * &x;
    }
    s <= q(bound) * q(bound)
}

/// Every data point within the fit's certified error, exactly.
fn holds<const D: usize>(fit: &CurveFit<D>, data: &[[f64; D]]) {
    for (k, (p, &u)) in data.iter().zip(&fit.parameters).enumerate() {
        assert!(
            within(*p, &exact(&fit.curve, u, 0), fit.error),
            "point {k}: the certified error {} is false",
            fit.error
        );
    }
}

/// Claim: chords neither overflow nor underflow while the differences are finite; a difference
/// that overflows is a typed error.
#[test]
fn extreme_magnitudes_are_certified_or_refused() {
    for scale in [1e-300, 1e-150, 1e150, 1e300] {
        let data: Vec<[f64; 2]> = (0..8)
            .map(|k| {
                let x = f64::from(k);
                [x * scale, (x * x * 0.1) * scale]
            })
            .collect();
        match Curve::interpolate(&data, 3, Parametrization::ChordLength, V, &mut meter()) {
            Ok(fit) => holds(&fit, &data),
            Err(e) => assert_eq!(e, NurbsError::NotFinite, "scale {scale}"),
        }
    }
    let wide = [[-f64::MAX, 0.0], [f64::MAX, 0.0], [f64::MAX, 1.0]];
    assert_eq!(
        parameters(&wide, Parametrization::ChordLength, &mut meter()),
        Err(NurbsError::NotFinite)
    );
}

/// Claim: degrees up to MAX_DEGREE interpolate, certified.
#[test]
fn the_highest_degree_interpolates() {
    let data: Vec<[f64; 3]> = (0..=MAX_DEGREE + 2)
        .map(|k| {
            let x = k as f64;
            [x, (x * 0.37).fract(), (x * 0.11).fract()]
        })
        .collect();
    for p in [MAX_DEGREE - 1, MAX_DEGREE] {
        match Curve::interpolate(&data, p, Parametrization::ChordLength, V, &mut meter()) {
            Ok(fit) => {
                assert_eq!(fit.curve.degree(), p);
                holds(&fit, &data);
            }
            // A degree this high may meet a pivot that rounding has made not positive.
            Err(e) => assert_eq!(e, NurbsError::Undetermined, "degree {p}"),
        }
    }
}

/// Claim: a nearly singular interpolation is refused or certified, never certified falsely.
#[test]
fn clustered_points_are_certified_or_refused() {
    let mut rng = Lcg(97);
    for trial in 0..60 {
        let mut data: Vec<[f64; 2]> = Vec::new();
        let mut x = 0.0;
        for k in 0..12 {
            // Runs of points 2^−40 (bits 0x3D70…) apart, then a long jump.
            x += if k % 4 == 0 {
                1.0 + rng.unit()
            } else {
                f64::from_bits(0x3D70_0000_0000_0000)
            };
            data.push([x, rng.unit() * 1e-12]);
        }
        for p in 1..=5 {
            match Curve::interpolate(&data, p, Parametrization::ChordLength, V, &mut meter()) {
                Ok(fit) => holds(&fit, &data),
                Err(e) => assert!(
                    matches!(
                        e,
                        NurbsError::Undetermined
                            | NurbsError::NotFinite
                            | NurbsError::CoincidentData { .. }
                    ),
                    "trial {trial} p {p}: {e}"
                ),
            }
        }
    }
}

/// Claim: a fit to a stated error is within it however loose the tolerance, down to p + 1 control
/// points, and the certificate stays true.
#[test]
fn a_loose_tolerance_leaves_the_fewest_control_points() {
    let data: Vec<[f64; 2]> = (0..50)
        .map(|k| {
            let x = f64::from(k) / 49.0;
            [x, x * x]
        })
        .collect();
    for mode in [KnotMode::Single, KnotMode::Multiple] {
        let fit = Curve::approximate(&data, 3, 10.0, mode, V, &mut meter()).unwrap();
        assert_eq!(fit.curve.homogeneous().len(), 4, "{mode:?}");
        assert!(fit.error <= 10.0);
        holds(&fit, &data);
    }
    // Collinear data: a line, whatever degree is asked for.
    let line: Vec<[f64; 3]> = (0..40)
        .map(|k| [f64::from(k), 2.0 * f64::from(k), -1.0])
        .collect();
    let fit = Curve::approximate(&line, 2, 1e-9, KnotMode::Single, V, &mut meter()).unwrap();
    assert_eq!(fit.curve.homogeneous().len(), 3);
    holds(&fit, &line);
}

/// Claim: a fit to a stated error never returns a curve beyond it. At zero tolerance even an exact
/// interpolant cannot be certified through outward-rounded intervals, so a fit is either certified
/// at zero or refused, never returned with a larger error.
#[test]
fn zero_tolerance_is_met_exactly_or_refused() {
    let polyline: Vec<[f64; 2]> = (0..9).map(|k| [f64::from(k), f64::from(k % 2)]).collect();
    let curved: Vec<[f64; 2]> = (0..9)
        .map(|k| {
            let x = f64::from(k) / 3.0;
            [x, x * x * x / 7.0]
        })
        .collect();
    for (data, degree, mode) in [
        (&polyline, 1, KnotMode::Multiple),
        (&curved, 3, KnotMode::Single),
    ] {
        match Curve::approximate(data, degree, 0.0, mode, V, &mut meter()) {
            Ok(fit) => {
                assert_eq!(fit.error, 0.0);
                holds(&fit, data);
            }
            Err(e) => assert_eq!(e, NurbsError::OutsideTolerance),
        }
    }
    // A tolerance just above the rounding of the polyline's interpolant is met.
    let fit = Curve::approximate(&polyline, 1, 1e-12, KnotMode::Multiple, V, &mut meter()).unwrap();
    assert!(fit.error <= 1e-12);
    holds(&fit, &polyline);
}

/// Claim: constraints that contradict or repeat are refused; a derivative above the degree is
/// identically zero, so asking for it is a constraint the basis cannot meet.
#[test]
fn impossible_constraints_are_refused() {
    let data: Vec<[f64; 2]> = (0..20)
        .map(|k| {
            let x = f64::from(k) / 19.0;
            [x, x * (1.0 - x)]
        })
        .collect();
    let m = &mut meter();
    let u = parameters(&data, Parametrization::ChordLength, m).unwrap();
    let basis = least_squares_knots(&u, 6, 2, m).unwrap();
    let third = Constraint {
        parameter: 0.5,
        order: 3,
        value: [1.0, 0.0],
    };
    assert_eq!(
        Curve::least_squares_at(&data, None, &u, basis.clone(), &[third], V, m),
        Err(NurbsError::Constraints)
    );
    // Two derivatives at one point that a quadratic must make equal through one control point
    // pair are fine; two contradicting points at one parameter are not.
    let a = Constraint {
        parameter: 0.3,
        order: 0,
        value: [0.3, 0.2],
    };
    let b = Constraint {
        parameter: 0.3,
        order: 0,
        value: [0.3, 0.25],
    };
    assert_eq!(
        Curve::least_squares_at(&data, None, &u, basis, &[a, b], V, m),
        Err(NurbsError::Constraints)
    );
}

/// Claim: a grid with a degenerate row (a pole) still has its parameters and its certificate.
#[test]
fn a_pole_in_the_grid_is_certified() {
    let (rows, cols) = (5, 6);
    let mut data = Vec::new();
    for i in 0..rows {
        for j in 0..cols {
            let r = i as f64 / 4.0;
            let t = j as f64 / 5.0;
            // Row 0 is a single point: the pole of a cone.
            data.push([r * (1.0 - t), r * t, 1.0 - r]);
        }
    }
    let fit = Surface::interpolate(
        &data,
        (rows, cols),
        (2, 3),
        Parametrization::ChordLength,
        V,
        &mut meter(),
    )
    .unwrap();
    assert!(fit.error < 1e-12);
    let fit = Surface::least_squares(
        &data,
        (rows, cols),
        (4, 5),
        (2, 3),
        Parametrization::Centripetal,
        V,
        &mut meter(),
    )
    .unwrap();
    assert!(fit.error.is_finite());
}

/// Claim: the same inputs give the same bits, every time, in one process.
#[test]
fn fits_repeat_to_the_bit() {
    let data: Vec<[f64; 3]> = (0..120)
        .map(|k| {
            let x = f64::from(k) / 119.0;
            [x, x * x * 3.0 - x, (x * 7.0).fract()]
        })
        .collect();
    let first = Curve::approximate(&data, 3, 1e-3, KnotMode::Single, V, &mut meter()).unwrap();
    for _ in 0..3 {
        let again = Curve::approximate(&data, 3, 1e-3, KnotMode::Single, V, &mut meter()).unwrap();
        assert_eq!(again, first);
    }
}

/// The zigzag of `n` points with integer coordinates: every interior point a corner, so no knot of
/// its interpolant of degree 1 can be removed within a small tolerance.
fn zigzag(n: u32) -> Vec<[f64; 2]> {
    (0..n).map(|k| [f64::from(k), f64::from(k % 2)]).collect()
}

/// Claim: a fit whose error equals the tolerance is within it. The interpolant of a zigzag keeps
/// every knot, so its error is the certified bound of the interpolant itself; asking again with
/// exactly that bound as the tolerance must give the same fit, not a refusal.
#[test]
fn an_error_equal_to_the_tolerance_is_within_it() {
    let data = zigzag(12);
    for mode in [KnotMode::Single, KnotMode::Multiple] {
        let first = Curve::approximate(&data, 1, 1e-9, mode, V, &mut meter()).unwrap();
        assert_eq!(first.curve.homogeneous().len(), 12, "no knot is removable");
        assert!(first.error > 0.0);
        holds(&first, &data);
        let again = Curve::approximate(&data, 1, first.error, mode, V, &mut meter()).unwrap();
        assert_eq!(again, first);
    }
}

/// The polyline of 9 points with unit sides and corners at points 2, 4 and 6; its chord-length
/// parameters are k/8, exact.
fn three_corners() -> Vec<[f64; 2]> {
    vec![
        [0.0, 0.0],
        [1.0, 0.0],
        [2.0, 0.0],
        [2.0, 1.0],
        [2.0, 2.0],
        [3.0, 2.0],
        [4.0, 2.0],
        [4.0, 3.0],
        [4.0, 4.0],
    ]
}

/// Claim: repeated knots keep corners. Of the 9 points' interpolant of degree 1 the 4 knots on
/// straight sides are removed; raising the degree doubles the 3 corner knots, which gives exactly
/// 9 control points, as many as points, and the least-squares step then interpolates. The fit
/// keeps each corner as a double knot, and it holds at a tolerance equal to its own error.
#[test]
fn double_knots_keep_corners_with_as_many_control_points_as_points() {
    let data = three_corners();
    let fit = Curve::approximate(&data, 2, 1e-6, KnotMode::Multiple, V, &mut meter()).unwrap();
    assert_eq!(
        fit.curve.basis().knots(),
        &[
            0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0
        ]
    );
    assert_eq!(fit.curve.homogeneous().len(), data.len());
    holds(&fit, &data);
    let again =
        Curve::approximate(&data, 2, fit.error, KnotMode::Multiple, V, &mut meter()).unwrap();
    assert_eq!(again, fit);
}

/// Claim: an approximation needs p + 1 points (and at least two); with exactly that many it is the
/// interpolant, with one fewer it is refused by count.
#[test]
fn the_fewest_points_are_enough_and_one_fewer_is_refused() {
    let data = zigzag(4);
    let fit = Curve::approximate(&data, 3, 1e-9, KnotMode::Single, V, &mut meter()).unwrap();
    assert_eq!(fit.curve.degree(), 3);
    holds(&fit, &data);
    assert_eq!(
        Curve::approximate(&data[..3], 3, 1e-9, KnotMode::Single, V, &mut meter()),
        Err(NurbsError::DataCount {
            points: 3,
            needed: 4
        })
    );
    assert_eq!(
        Curve::approximate(&data[..1], 1, 1e-9, KnotMode::Multiple, V, &mut meter()),
        Err(NurbsError::DataCount {
            points: 1,
            needed: 2
        })
    );
}

/// Claim: raising the degree over corners whose double knots have no data between them is not
/// determined, and the attempt starts again one degree higher instead of fitting a curve the data
/// do not fix. Every knot of the result is then single: it comes from that next attempt's
/// interpolant.
#[test]
fn double_knots_with_no_data_between_them_start_the_next_attempt() {
    let mut data = zigzag(6);
    data.extend((1..=20).map(|k| [5.0 + f64::from(k), 1.0]));
    let fit = Curve::approximate(&data, 2, 1e-6, KnotMode::Multiple, V, &mut meter()).unwrap();
    holds(&fit, &data);
    let knots = fit.curve.basis().knots();
    let interior = &knots[3..knots.len() - 3];
    assert!(
        interior.windows(2).all(|w| w[0] < w[1]),
        "a double knot: the undetermined raise was kept, knots {knots:?}"
    );
}

/// Claim: parameters must strictly increase; a decrease is refused at the first parameter below
/// its predecessor, as an equal one is.
#[test]
fn a_decreasing_parameter_is_refused_where_it_falls() {
    let m = &mut meter();
    for (u, index) in [
        (&[0.0, 0.5, 0.4, 1.0][..], 2),
        (&[0.0, 0.25, 0.5, 0.75, 0.7, 1.0][..], 4),
        (&[0.0, 0.5, 0.5, 1.0][..], 2),
    ] {
        assert_eq!(
            geom_nurbs::averaged_knots(u, 1, m),
            Err(NurbsError::ParameterOrder { index })
        );
    }
}

/// Claim: a grid's shape is checked before its degrees: a grid that does not hold rows × columns
/// points is refused by count even when a direction also has too few rows for its degree; one
/// with a single row is refused for its rows.
#[test]
fn a_grid_of_the_wrong_size_is_refused_by_its_size_first() {
    let data: Vec<[f64; 3]> = (0..12)
        .map(|k| [f64::from(k % 4), f64::from(k / 4), f64::from(k % 3)])
        .collect();
    let r = Parametrization::ChordLength;
    assert_eq!(
        Surface::interpolate(&data, (4, 4), (4, 1), r, V, &mut meter()),
        Err(NurbsError::PointCount {
            points: 12,
            expected: 16
        })
    );
    assert_eq!(
        Surface::least_squares(&data[..4], (1, 4), (1, 2), (1, 1), r, V, &mut meter()),
        Err(NurbsError::DataCount {
            points: 1,
            needed: 2
        })
    );
}

/// Claim: interpolation at parameters where the last basis function never lives is not
/// determined (its pivot is zero), and is refused as such rather than divided by zero.
#[test]
fn a_last_function_without_data_is_undetermined() {
    let data = zigzag(6);
    let basis =
        geom_nurbs::Basis::new(2, vec![0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0]).unwrap();
    assert_eq!(
        Curve::interpolate_at(
            &data,
            &[0.0, 0.1, 0.3, 0.5, 0.6, 0.7],
            basis,
            V,
            &mut meter()
        ),
        Err(NurbsError::Undetermined)
    );
}

/// Claim: constraints may fix every control point (then the data only measure the error), and one
/// constraint more than there are control points is refused before any work on them.
#[test]
fn as_many_constraints_as_control_points_fix_the_curve() {
    let data: Vec<[f64; 2]> = (0..5).map(|k| [f64::from(k) / 4.0, 0.5]).collect();
    let u = [0.0, 0.25, 0.5, 0.75, 1.0];
    let line = geom_nurbs::Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let at = |parameter: f64, value: [f64; 2]| Constraint {
        parameter,
        order: 0,
        value,
    };
    let ends = [at(0.0, [0.0, 0.0]), at(1.0, [1.0, 1.0])];
    let fit =
        Curve::least_squares_at(&data, None, &u, line.clone(), &ends, V, &mut meter()).unwrap();
    assert_eq!(fit.curve.homogeneous(), &[[0.0, 0.0], [1.0, 1.0]]);
    assert!(fit.constraint_error < 1e-12, "{}", fit.constraint_error);
    let three = [ends[0], at(0.5, [0.5, 0.5]), ends[1]];
    let mut m = meter();
    assert_eq!(
        Curve::least_squares_at(&data, None, &u, line, &three, V, &mut m),
        Err(NurbsError::Constraints)
    );
    assert_eq!(m.used().work, REFUSED_WORK, "work spent before the refusal");
}

/// The work `least_squares_at` charges before it refuses too many constraints: copying the data.
const REFUSED_WORK: u128 = 5;

/// Claim: with constraints, data that leave a free control point untouched do not determine the
/// curve. Every parameter lies below 0.5, where the last function of the basis vanishes, and the
/// one constraint pins the first point only.
#[test]
fn constraints_with_a_free_point_the_data_miss_are_undetermined() {
    let data: Vec<[f64; 2]> = (0..4).map(|k| [f64::from(k), 1.0]).collect();
    let u = [0.0, 0.1, 0.2, 0.4];
    let basis = geom_nurbs::Basis::new(1, vec![0.0, 0.0, 0.5, 1.0, 1.0]).unwrap();
    let pin = Constraint {
        parameter: 0.0,
        order: 0,
        value: [0.0, 1.0],
    };
    assert_eq!(
        Curve::least_squares_at(&data, None, &u, basis, &[pin], V, &mut meter()),
        Err(NurbsError::Undetermined)
    );
}

/// Claim: the rank tolerance is max(m, n)·2^−53 relative to R's first diagonal entry (chapter
/// 15). One data point at 0.5 + k·2^−53 is the only one where the last function of the basis
/// lives, with value 2k·2^−53; the smallest k whose fit is determined is pinned, so a tolerance
/// twice or four times as large moves it.
#[test]
fn the_rank_tolerance_is_the_stated_one() {
    let basis = geom_nurbs::Basis::new(1, vec![0.0, 0.0, 0.5, 1.0, 1.0]).unwrap();
    let step = f64::EPSILON / 2.0;
    let first = (1..64)
        .find(|&k| {
            let t = 0.5 + f64::from(k) * step;
            let u = [0.0, 0.25, 0.5, t];
            let data = [[0.0], [0.25], [0.5], [t]];
            Curve::least_squares_at(&data, None, &u, basis.clone(), &[], V, &mut meter()).is_ok()
        })
        .unwrap();
    assert_eq!(first, FIRST_DETERMINED);
}

/// The smallest k of [`the_rank_tolerance_is_the_stated_one`] whose fit is determined.
const FIRST_DETERMINED: u32 = 5;

/// Claim: parameters whose range overflows are refused for that range. The knot between two of
/// them whose difference overflows is kept at the larger, never infinite, so the basis refuses
/// the range, not a knot.
#[test]
fn parameters_whose_range_overflows_are_refused_for_it() {
    let u = [-1.7e308, -1.6e308, 1.6e308, 1.7e308, 1.75e308];
    assert_eq!(
        least_squares_knots(&u, 4, 2, &mut meter()),
        Err(NurbsError::KnotRange)
    );
}

/// sin(πx), correctly rounded on every target.
fn sinpi(x: f64) -> f64 {
    geom_num::math::sinpi(x).unwrap().get()
}

/// cos(πx), correctly rounded on every target.
fn cospi(x: f64) -> f64 {
    geom_num::math::cospi(x).unwrap().get()
}

/// Claim: every fit charges the work it does; the totals are pinned, so a step that stops
/// charging, or does more than it should, changes them.
#[test]
fn the_work_each_fit_charges_is_pinned() {
    let mut used = Vec::new();
    let helix: Vec<[f64; 3]> = (0..60)
        .map(|k| {
            let t = f64::from(k) / 59.0 * 1.3;
            [cospi(t), sinpi(t), 0.9 * t]
        })
        .collect();
    let noisy: Vec<[f64; 2]> = (0..50)
        .map(|k| {
            let x = f64::from(k) / 49.0;
            [x, sinpi(x * 3.0) + 0.01 * f64::from(k % 7)]
        })
        .collect();
    for (degree, tol, mode) in [(3, 1e-3, KnotMode::Single), (2, 1e-4, KnotMode::Multiple)] {
        let mut m = meter();
        Curve::approximate(&helix, degree, tol, mode, V, &mut m).unwrap();
        used.push(m.used().work);
        let mut m = meter();
        Curve::approximate(&noisy, degree, tol * 10.0, mode, V, &mut m).unwrap();
        used.push(m.used().work);
    }
    let grid: Vec<[f64; 3]> = (0..30)
        .map(|k| {
            let (x, y) = (f64::from(k / 6), f64::from(k % 6));
            [x, y, x * 0.1 * y - 0.05 * x * x]
        })
        .collect();
    let mut m = meter();
    Surface::least_squares(
        &grid,
        (5, 6),
        (4, 4),
        (2, 2),
        Parametrization::ChordLength,
        V,
        &mut m,
    )
    .unwrap();
    used.push(m.used().work);
    let mut m = meter();
    Surface::interpolate(
        &grid,
        (5, 6),
        (2, 3),
        Parametrization::Centripetal,
        V,
        &mut m,
    )
    .unwrap();
    used.push(m.used().work);
    assert_eq!(used, USED);
}

/// The work of each fit in [`the_work_each_fit_charges_is_pinned`].
const USED: [u128; 6] = [63751, 99120, 12293, 7517, 1590, 2672];

/// Claim: least squares works down to its fewest data: two points and two control points give the
/// line through them, for a curve and for a grid of 2 × 2.
#[test]
fn two_points_give_the_line_through_them() {
    let data = [[0.0, 0.0], [3.0, 4.0]];
    let fit =
        Curve::least_squares(&data, 2, 1, Parametrization::ChordLength, V, &mut meter()).unwrap();
    assert_eq!(fit.curve.homogeneous(), &data);
    assert_eq!(fit.parameters, vec![0.0, 1.0]);
    let grid = [
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 1.0],
    ];
    let fit = Surface::least_squares(
        &grid,
        (2, 2),
        (2, 2),
        (1, 1),
        Parametrization::ChordLength,
        V,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(fit.surface.homogeneous(), &grid);
}
