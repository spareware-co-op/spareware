//! Piece G371's cubic spiral (decision 32.33, the kernel's definition): the transition law whose
//! offset from its start tangent is y = κ₁·l²/2 + Δκ·l³/(6L) along its arc length l, so the sine
//! of its heading is the clothoid's heading q(l) = κ₁·l + Δκ·l²/(2L).
//!
//! Oracles: y in exact rationals (`dashu-ratio`), the polynomial's integral; x = ∫₀ˡ √(1 − q²) by
//! the binomial series √(1 − z) = Σ cₖ·zᵏ, every term's integral exact, the tail bounded by
//! l·m^(2K+2)/(1 − m²) for |q| ≤ m < 1; the heading's turn against asin q through `sinpi` of
//! its enclosure; the curvature against q′/√(1 − q²) worked here; and every law of the kind
//! contract under rigid, mirrored and scaling transforms, whole and in part, either way.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    clippy::cast_lossless,
    clippy::assign_op_pattern,
    clippy::manual_midpoint,
    clippy::unnecessary_cast,
    reason = "tests written as the formulas read, with std's maths for the oracles, and a failure stops the test"
)]

use dashu_ratio::RBig;
use geom_curve::contract;
use geom_curve::{CurvatureLaw, CurveError, CurveKind, ParamCurve, Transition};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn r(x: f64) -> RBig {
    RBig::try_from(x).unwrap()
}

fn f(x: &RBig) -> f64 {
    x.to_f64().value()
}

/// A polynomial in l with rational coefficients, lowest power first.
type Poly = Vec<RBig>;

fn times(a: &Poly, b: &Poly) -> Poly {
    let mut out = vec![RBig::ZERO; a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            out[i + j] = &out[i + j] + x * y;
        }
    }
    out
}

/// ∫₀ˡ p, exactly.
fn integral_to(p: &Poly, l: &RBig) -> RBig {
    let mut sum = RBig::ZERO;
    let mut power = l.clone();
    for (k, c) in p.iter().enumerate() {
        sum = sum + c * &power / RBig::from(k as u64 + 1);
        power = &power * l;
    }
    sum
}

/// The cubic spiral's truth at arc length l in its own frame (start at the origin, leaving along
/// +x): (x, y, the bound on x's series tail), from κ₁, κ₂ and L, exactly as doubles give them.
fn truth(k1: f64, k2: f64, length: f64, l: f64, m: f64) -> (RBig, RBig, f64) {
    let (k1, k2, big, l) = (r(k1), r(k2), r(length), r(l));
    // q(u) = κ₁u + (κ₂ − κ₁)u²/(2L).
    let q: Poly = vec![RBig::ZERO, k1.clone(), (&k2 - &k1) / (RBig::from(2) * &big)];
    let y = integral_to(&q, &l);
    let q2 = times(&q, &q);
    // √(1 − z) = Σ cₖ zᵏ, c₀ = 1, cₖ₊₁ = cₖ·(k − ½)/(k + 1).
    let terms = 40;
    let mut c = RBig::ONE;
    let mut power: Poly = vec![RBig::ONE];
    let mut x = RBig::ZERO;
    for k in 0..terms {
        x = x + &c * integral_to(&power, &l);
        power = times(&power, &q2);
        c = c * (RBig::from(k as i64) - RBig::from(1) / RBig::from(2)) / RBig::from(k as i64 + 1);
    }
    // |cₖ| ≤ 1 for every k, so the tail is at most l·Σₖ≥K m^(2k) = l·m^(2K)/(1 − m²).
    let tail = f(&l) * m.powi(2 * terms as i32) / (1.0 - m * m) * 2.0;
    (x, y, tail)
}

/// The cases: from a straight to a radius, between two radii, and turning right.
fn cases() -> Vec<(&'static str, f64, f64, f64)> {
    vec![
        ("straight to R = 500 over 100", 0.0, 1.0 / 512.0, 100.0),
        (
            "R = 2000 to R = 400 over 150",
            1.0 / 2048.0,
            1.0 / 400.0,
            150.0,
        ),
        (
            "straight to R = 300, turning right, over 200",
            0.0,
            -1.0 / 300.0,
            200.0,
        ),
    ]
}

fn spiral(k1: f64, k2: f64, length: f64) -> Transition<2> {
    Transition::new(
        [3.0, -2.0],
        [1.0, 0.0],
        length,
        CurvatureLaw::CubicSpiral { start: k1, end: k2 },
        &mut meter(),
    )
    .unwrap()
}

/// Whether `i` and the exact value `v`, known within `tail`, can be one number.
fn holds(i: Interval, v: &RBig, tail: f64) -> bool {
    let v = f(v);
    let slack = tail + 4.0 * f64::EPSILON * v.abs().max(1.0);
    i.lo() - slack <= v && v <= i.hi() + slack
}

#[test]
fn points_enclose_the_exact_offset_and_the_series_abscissa() {
    for (name, k1, k2, length) in cases() {
        let t = spiral(k1, k2, length);
        let m = (k1.abs() + (k2 - k1).abs() / 2.0) * length;
        assert!(m < 1.0, "{name}");
        for frac in [0.0, 1.0 / 7.0, 1.0 / 3.0, 0.5, 0.9, 1.0] {
            let l = length * frac;
            let (x, y, tail) = truth(k1, k2, length, l, m);
            let e = t.enclose(l, Limit::FromRight, 0, &mut meter()).unwrap()[0];
            // The start (3, −2), leaving along +x.
            let (ex, ey) = (
                e[0].sub(Interval::point(3.0).unwrap()).unwrap(),
                e[1].add(Interval::point(2.0).unwrap()).unwrap(),
            );
            assert!(
                holds(ex, &x, tail),
                "{name} at {l}: x {ex:?} against {}",
                f(&x)
            );
            assert!(
                holds(ey, &y, 0.0),
                "{name} at {l}: y {ey:?} against {}",
                f(&y)
            );
            assert!(
                ex.hi() - ex.lo() < 1e-9 && ey.hi() - ey.lo() < 1e-9,
                "{name}: wide"
            );
            let p = t.point(l, Limit::FromRight).unwrap();
            assert!((p[0] - 3.0 - f(&x)).abs() < 1e-12 * length, "{name} at {l}");
            assert!((p[1] + 2.0 - f(&y)).abs() < 1e-12 * length, "{name} at {l}");
        }
    }
}

#[test]
fn from_a_straight_the_offset_is_the_textbook_cubic() {
    let (radius, length) = (512.0, 100.0);
    let t = spiral(0.0, 1.0 / radius, length);
    for frac in [0.25, 0.5, 1.0] {
        let l = length * frac;
        let e = t.enclose(l, Limit::FromRight, 0, &mut meter()).unwrap()[0];
        let y = r(l) * r(l) * r(l) / (RBig::from(6) * r(radius) * r(length));
        assert!(
            holds(e[1].add(Interval::point(2.0).unwrap()).unwrap(), &y, 0.0),
            "{l}"
        );
    }
}

#[test]
fn the_heading_and_curvature_are_the_sine_forms() {
    for (name, k1, k2, length) in cases() {
        let t = spiral(k1, k2, length);
        for frac in [0.0, 0.2, 0.6, 1.0] {
            let l = length * frac;
            let q = k1 * l + (k2 - k1) * l * l / (2.0 * length);
            // sin(π·turn) holds q.
            let turn = t.turn(l).unwrap();
            let s = turn.sinpi().unwrap();
            assert!(
                s.lo() - 1e-15 <= q && q <= s.hi() + 1e-15,
                "{name} at {l}: {s:?} {q}"
            );
            assert!(turn.hi() - turn.lo() < 1e-12, "{name}");
            let dq = k1 + (k2 - k1) * l / length;
            let kappa = dq / (1.0 - q * q).sqrt();
            let c = t.curvature(l).unwrap();
            assert!(
                c.lo() - 1e-15 <= kappa && kappa <= c.hi() + 1e-15,
                "{name} at {l}: {c:?} {kappa}"
            );
        }
        // The first derivative is the unit tangent (√(1 − q²), q).
        let e = t.enclose(length, Limit::FromLeft, 1, &mut meter()).unwrap();
        let q = (k1 + k2) / 2.0 * length;
        assert!(
            (e[1][1].lo() - q).abs() < 1e-12 && (e[1][0].lo() - (1.0 - q * q).sqrt()).abs() < 1e-12
        );
    }
}

#[test]
fn a_spiral_whose_offset_outruns_its_length_is_refused() {
    for (k2, length) in [
        (1.0 / 40.0, 100.0),
        (1.0 / 50.0, 100.0),
        (-1.0 / 50.0, 100.0),
    ] {
        let law = CurvatureLaw::CubicSpiral {
            start: 0.0,
            end: k2,
        };
        assert_eq!(
            Transition::new([0.0, 0.0], [1.0, 0.0], length, law, &mut meter()),
            Err(CurveError::Degenerate),
            "{k2}"
        );
    }
}

#[test]
fn its_mirror_turns_the_other_way() {
    let law = CurvatureLaw::CubicSpiral {
        start: 0.001,
        end: -0.002,
    };
    assert_eq!(
        law.mirrored(),
        CurvatureLaw::CubicSpiral {
            start: -0.001,
            end: 0.002,
        }
    );
    let t = spiral(0.0, 1.0 / 512.0, 100.0);
    let mirror = TypedTransform::Exact(GridTransform::mirror(Axis::Y));
    let image = t.transformed(&mirror, &mut meter()).unwrap();
    let CurveKind::Transition(m) = image.value else {
        panic!("a mirrored transition is a transition")
    };
    assert_eq!(
        m.law(),
        CurvatureLaw::CubicSpiral {
            start: 0.0,
            end: -1.0 / 512.0,
        }
    );
    let (p, q) = (
        t.point(60.0, Limit::FromRight).unwrap(),
        m.point(60.0, Limit::FromRight).unwrap(),
    );
    assert!(
        (p[0] - q[0]).abs() < 1e-12 && (p[1] + q[1]).abs() < 1e-12,
        "{p:?} {q:?}"
    );
}

#[test]
fn every_part_passes_the_kind_contract_under_every_transform() {
    let transforms = [
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1)),
        TypedTransform::Exact(GridTransform::mirror(Axis::Y)),
        TypedTransform::from(
            RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap(),
        ),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.37).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 1.0).unwrap()),
    ];
    for (i, (name, k1, k2, length)) in cases().into_iter().enumerate() {
        let t = spiral(k1, k2, length);
        let window = (length / 7.0, 0.8 * length);
        let parts = [
            t.clone(),
            t.segment((0.0, length), true).unwrap(),
            t.segment(window, false).unwrap(),
            t.segment(window, true).unwrap(),
        ];
        for (j, part) in parts.iter().enumerate() {
            let (a, b) = part.domain();
            let params: Vec<f64> = [0.0, 0.13, 0.5, 0.77, 1.0]
                .iter()
                .map(|q| (a + (b - a) * q).clamp(a, b))
                .collect();
            let tr = &transforms[(i + j) % transforms.len()];
            contract::check(part, &params, tr, &mut meter())
                .unwrap_or_else(|e| panic!("{name}, part {j}, under {tr:?}: {e}"));
        }
    }
}

#[test]
fn its_nurbs_form_is_certified_close() {
    for (name, k1, k2, length) in cases() {
        let t = spiral(k1, k2, length);
        let n = t.to_nurbs(&mut meter()).unwrap();
        assert!(n.deviation < 1e-9 * length, "{name}: {}", n.deviation);
        for frac in [0.1, 0.45, 0.85] {
            let l = length * frac;
            let p = t.point(l, Limit::FromRight).unwrap();
            let q = n.value.curve.point(l, Limit::FromRight).unwrap();
            assert!(
                (p[0] - q[0]).hypot(p[1] - q[1]) <= n.deviation + 1e-12 * length,
                "{name} at {l}"
            );
        }
    }
}

/// The cubic parabola y = x³/(6RL) is a polynomial curve in x (decision 32.33): its points are the
/// formula's, exactly up to its stated deviation, turning left for a positive radius and right for
/// a negative one, and it passes the kind contract.
#[test]
fn the_cubic_parabola_is_the_polynomial_in_x() {
    use geom_curve::Polynomial;
    for (radius, direction) in [
        (512.0, [1.0, 0.0]),
        (-300.0, [0.0, 2.0]),
        (250.0, [3.0, 4.0]),
    ] {
        let length = 80.0;
        let c = Polynomial::cubic_parabola([10.0, 20.0], direction, radius, length).unwrap();
        let exact_axes = direction[0] == 0.0 || direction[1] == 0.0;
        assert!(c.deviation < 1e-12 * length, "{radius}: {}", c.deviation);
        assert_eq!(c.value.degree(), 3);
        let norm = (direction[0] * direction[0] + direction[1] * direction[1]).sqrt();
        let (ux, uy) = (direction[0] / norm, direction[1] / norm);
        for x in [0.0, 20.0, 40.0, 80.0] {
            let p = c.value.point(x, Limit::FromRight).unwrap();
            let y = x * x * x / (6.0 * radius * length);
            let (ex, ey) = (10.0 + x * ux - y * uy, 20.0 + x * uy + y * ux);
            assert!(
                (p[0] - ex).abs() < 1e-11 && (p[1] - ey).abs() < 1e-11,
                "{radius} at {x}: {p:?} against ({ex}, {ey})"
            );
            if exact_axes && x == 0.0 {
                assert_eq!(p, [10.0, 20.0]);
            }
        }
        let t = TypedTransform::from(Transform::rotation(Axis::Z, 0.37).unwrap());
        contract::check(&c.value, &[0.0, 13.0, 40.0, 79.0, 80.0], &t, &mut meter())
            .unwrap_or_else(|e| panic!("{radius}: {e}"));
    }
    for (direction, radius, length) in [
        ([0.0, 0.0], 1.0, 1.0),
        ([1.0, 0.0], 0.0, 1.0),
        ([1.0, 0.0], 1.0, 0.0),
    ] {
        assert_eq!(
            Polynomial::cubic_parabola([0.0, 0.0], direction, radius, length),
            Err(CurveError::Degenerate)
        );
    }
}
