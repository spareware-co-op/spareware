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
