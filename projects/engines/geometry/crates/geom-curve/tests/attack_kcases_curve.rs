//! Attack tests from solidcore's kernel-case corpus: curves, `geom-curve`.
//!
//! The corpus is `crates/solidcore/tests/kernel_cases/` in solidcore (tangency-labs/solidcore,
//! Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70). The entries here are the ones
//! that reduce to evaluating a curve: a point a given fraction of the way along by length, the
//! tightest radius of curvature of an ellipse, a curvature comb, a centre, a point past the end.
//! The length of a curve is integrated from the kernel's own derivatives (a composite five-point
//! Gauss-Legendre rule, in plain doubles); curvature is |C' x C''| / |C'|^3 from the kernel's
//! own first and second derivatives. Each test cites its entry as `<area>/<case>.json`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::many_single_char_names,
    clippy::suboptimal_flops,
    reason = "tests: native arithmetic, platform maths on an independent oracle, prose that names corpus files, and a failure stops the test"
)]

use geom_curve::{Axes, Basis as Leaf, Circle, Ellipse, Frame, Line, ParamCurve};
use geom_nurbs::{Basis, Curve, Limit};

const R: Limit = Limit::FromRight;

const NODES: [f64; 5] = [
    -0.906_179_845_938_664,
    -0.538_469_310_105_683_1,
    0.0,
    0.538_469_310_105_683_1,
    0.906_179_845_938_664,
];
const WEIGHTS: [f64; 5] = [
    0.236_926_885_056_189_1,
    0.478_628_670_499_366_5,
    0.568_888_888_888_888_9,
    0.478_628_670_499_366_5,
    0.236_926_885_056_189_1,
];

fn norm(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn derivs(c: &impl ParamCurve<3>, t: f64) -> [[f64; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    c.derivatives(t, R, &mut out).unwrap();
    out
}

/// The length of `c` over [a, b], 64 panels of five points.
fn length(c: &impl ParamCurve<3>, a: f64, b: f64) -> f64 {
    let h = (b - a) / 64.0;
    let mut total = 0.0;
    for i in 0..64 {
        for (x, w) in NODES.iter().zip(WEIGHTS) {
            let t = a + h * (f64::from(i) + 0.5 + 0.5 * x);
            total += w * 0.5 * h * norm(derivs(c, t)[1]);
        }
    }
    total
}

/// The parameter at which the length from the start is `fraction` of the whole, by bisection.
fn at_fraction(c: &impl ParamCurve<3>, fraction: f64) -> f64 {
    let (lo0, hi0) = c.domain();
    let whole = length(c, lo0, hi0);
    let (mut lo, mut hi) = (lo0, hi0);
    for _ in 0..60 {
        let mid = f64::midpoint(lo, hi);
        if length(c, lo0, mid) < fraction * whole {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    f64::midpoint(lo, hi)
}

fn curvature(c: &impl ParamCurve<3>, t: f64) -> f64 {
    let [_, d1, d2] = derivs(c, t);
    let cr = [
        d1[1] * d2[2] - d1[2] * d2[1],
        d1[2] * d2[0] - d1[0] * d2[2],
        d1[0] * d2[1] - d1[1] * d2[0],
    ];
    norm(cr) / norm(d1).powi(3)
}

fn near(got: [f64; 3], want: [f64; 3], tol: f64) {
    for k in 0..3 {
        assert!((got[k] - want[k]).abs() <= tol, "{got:?} vs {want:?}");
    }
}

/// construction/cov-point-along-path-a-third-of-an-arc.json: the arc r 10 about the origin from
/// (10, 0) to (0, 10): a third of its length is 30 degrees round, (10 cos 30, 10 sin 30, 0) =
/// (8.660254037844386, 5, 0), graded to 1e-9.
#[test]
fn construction_a_third_of_the_way_along_a_quarter_arc_by_length() {
    let arc = Circle::<3>::new([0.0; 3], 10.0, Frame::standard().unwrap(), (0.0, 0.5)).unwrap();
    let t = at_fraction(&arc, 1.0 / 3.0);
    near(
        arc.point(t, R).unwrap(),
        [8.660_254_037_844_386, 5.0, 0.0],
        1e-9,
    );
}

/// construction/cov-point-along-path-middle-of-a-symmetric-spline.json: the Bezier (0,0), (10,10),
/// (30,10), (40,0) is its own mirror image in x = 20, so half its length is reached at its middle:
/// (20, 7.5, 0), graded to 1e-9. Found here from the length, not from the parameter.
#[test]
fn construction_half_way_along_a_symmetric_cubic_by_length() {
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let pts = [
        [0.0, 0.0, 0.0],
        [10.0, 10.0, 0.0],
        [30.0, 10.0, 0.0],
        [40.0, 0.0, 0.0],
    ];
    let curve = Leaf::Nurbs(Curve::<3>::new(basis, &pts).unwrap());
    let t = at_fraction(&curve, 0.5);
    assert!((t - 0.5).abs() < 1e-9, "t = {t}");
    near(curve.point(t, R).unwrap(), [20.0, 7.5, 0.0], 1e-9);
}

/// construction/cov-point-along-path-quarter-of-a-line.json and ...-past-the-end.json: a quarter
/// along the line from (0,0,0) to (40,0,0) is (10,0,0); a parameter of 1.5 is past its end and is
/// refused, not moved back to the end.
#[test]
fn construction_a_quarter_along_a_line_and_a_parameter_past_its_end() {
    let line = Line::<3>::segment([0.0; 3], [40.0, 0.0, 0.0])
        .unwrap()
        .value;
    near(line.point(0.25, R).unwrap(), [10.0, 0.0, 0.0], 1e-12);
    assert!(line.point(1.5, R).is_err());
    assert!(line.point(-0.1, R).is_err());
}

/// inspect/cov-minimum-radius-of-an-ellipse.json: the ellipse a 10, b 4 is tightest at the ends of
/// the major axis, where the radius of curvature is b^2 / a = 1.6, at (+-10, 0).
#[test]
fn inspect_the_smallest_radius_of_an_ellipse_is_b_squared_over_a() {
    let axes = Axes::Frame {
        frame: Frame::standard().unwrap(),
        a: 10.0,
        b: 4.0,
    };
    let e = Ellipse::<3>::new([0.0; 3], axes, (0.0, 2.0)).unwrap();
    let mut smallest = f64::INFINITY;
    for i in 0..=4000 {
        let t = 2.0 * f64::from(i) / 4000.0;
        smallest = smallest.min(1.0 / curvature(&e, t));
    }
    assert!((smallest - 1.6).abs() < 1e-9, "{smallest}");
    for t in [0.0, 1.0] {
        assert!((1.0 / curvature(&e, t) - 1.6).abs() < 1e-12);
        assert!((e.point(t, R).unwrap()[0].abs() - 10.0).abs() < 1e-12);
    }
}

/// inspect/cov-curvature-comb-circle-and-line.json: a circle r 7 has curvature 1/7 everywhere
/// (radius 7); a line is straight (curvature 0, no radius).
#[test]
fn inspect_a_circle_has_curvature_one_over_r_and_a_line_has_none() {
    let c = Circle::<3>::new([0.0; 3], 7.0, Frame::standard().unwrap(), (0.0, 2.0)).unwrap();
    for i in 0..=50 {
        let t = 2.0 * f64::from(i) / 50.0;
        assert!((curvature(&c, t) - 1.0 / 7.0).abs() < 1e-13, "t = {t}");
    }
    let line = Line::<3>::segment([0.0; 3], [20.0, 5.0, 0.0])
        .unwrap()
        .value;
    for i in 0..=10 {
        let k = curvature(&line, f64::from(i) / 10.0);
        assert!(k == 0.0, "{k}");
    }
}

/// construction/cov-point-center-of-a-sketch-arc.json: the centre of the arc r 10 about the origin
/// is the origin, and its radius 10.
#[test]
fn construction_the_centre_of_an_arc() {
    let arc = Circle::<3>::new([0.0; 3], 10.0, Frame::standard().unwrap(), (0.0, 0.5)).unwrap();
    assert_eq!(arc.center(), [0.0; 3]);
    assert!((arc.radius() - 10.0).abs() < 1e-15);
}
