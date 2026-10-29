//! Tests of `geom_num::quadrature` and its generated rules (piece G51).
//!
//! What this file proves: every Gauss–Legendre rule integrates every monomial of degree at most
//! 2n − 1, and every Gauss–Kronrod rule every one of degree at most 3n + 1 (n even) or 3n + 2
//! (n odd), with both its weight sets, to within the rounding of its entries, checked in exact
//! rationals from the committed bits; the rules' nodes are ascending and symmetric, their weights
//! positive and summing to 2, and each Kronrod rule's Gauss nodes are the Gauss rule's own bits;
//! the fixed rules and the adaptive integrator meet their tolerance on smooth integrals with
//! known values (an exponential, Runge's function, a sine) and polynomials; the integrator's
//! errors (an empty interval, no such rule, the cap, a narrow panel, a failing function) are
//! reported, never answered.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_num::NumError;
use geom_num::math;
use geom_num::quadrature::{MAX_PANELS, QuadError, gauss_legendre, integrate};
use geom_num::quadrature_rules::{GAUSS_KRONROD, GAUSS_LEGENDRE};
use geom_num::real::Real;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, Zero};

fn exact(x: f64) -> BigRational {
    BigRational::from_float(x).unwrap()
}

fn r(x: f64) -> Real {
    Real::new(x).unwrap()
}

/// ∫₋₁¹ xᵏ dx, exactly.
fn moment(k: u32) -> BigRational {
    if k % 2 == 0 {
        BigRational::new(BigInt::from(2), BigInt::from(k + 1))
    } else {
        BigRational::zero()
    }
}

/// |Σ wᵢ xᵢᵏ − ∫ xᵏ| in exact rationals, against (k + 2)·2^−52: the rounding of the nodes and
/// weights moves the sum by at most (k + 1)·2^−53·Σ|wᵢ| = (k + 1)·2^−52.
fn check_exact(nodes: &[f64], weights: &[f64], degree: u32, what: &str) {
    // Σ wᵢ xᵢᵏ for every k at once, each node's powers built up one multiplication at a time.
    let mut sums = vec![BigRational::zero(); degree as usize + 1];
    for (x, w) in nodes.iter().zip(weights) {
        let x = exact(*x);
        let mut term = exact(*w);
        for sum in &mut sums {
            *sum += &term;
            term *= &x;
        }
    }
    for (k, sum) in sums.into_iter().enumerate() {
        let k = k as u32;
        let err = (sum - moment(k)).abs();
        let bound = BigRational::new(BigInt::from(k + 2), BigInt::from(1u64 << 52));
        assert!(err <= bound, "{what}: x^{k} off by {err}");
    }
}

fn check_shape(nodes: &[f64], weights: &[f64], what: &str) {
    assert!(nodes.windows(2).all(|p| p[0] < p[1]), "{what}: ascending");
    for (a, b) in nodes.iter().zip(nodes.iter().rev()) {
        assert_eq!(*a, -*b, "{what}: symmetric");
    }
    assert!(weights.iter().all(|w| *w > 0.0), "{what}: positive");
    let total = weights
        .iter()
        .fold(BigRational::zero(), |acc, w| acc + exact(*w));
    let off = (total - BigRational::from_integer(BigInt::from(2))).abs();
    assert!(
        off <= BigRational::new(BigInt::from(1), BigInt::from(1u64 << 50)),
        "{what}: Σw"
    );
}

#[test]
fn gauss_rules_are_exact_to_their_degree() {
    for (i, rule) in GAUSS_LEGENDRE.iter().enumerate() {
        let n = i as u32 + 1;
        assert_eq!(rule.nodes.len(), n as usize);
        check_shape(rule.nodes, rule.weights, &format!("G{n}"));
        check_exact(rule.nodes, rule.weights, 2 * n - 1, &format!("G{n}"));
    }
}

#[test]
fn kronrod_rules_are_exact_to_their_degree() {
    for (rule, n) in GAUSS_KRONROD.iter().zip([7u32, 10, 15, 20, 25, 30]) {
        let what = format!("G{n}K{}", 2 * n + 1);
        assert_eq!(rule.nodes.len(), 2 * n as usize + 1);
        assert_eq!(rule.gauss.len(), n as usize);
        check_shape(rule.nodes, rule.kronrod, &what);
        let degree = if n % 2 == 0 { 3 * n + 1 } else { 3 * n + 2 };
        check_exact(rule.nodes, rule.kronrod, degree, &what);
        // The Gauss nodes, at the odd positions, with the Gauss weights.
        let gauss_nodes: Vec<f64> = rule.nodes.iter().skip(1).step_by(2).copied().collect();
        check_exact(
            &gauss_nodes,
            rule.gauss,
            2 * n - 1,
            &format!("{what}, Gauss part"),
        );
        if let Some(g) = GAUSS_LEGENDRE.get(n as usize - 1) {
            assert_eq!(
                g.nodes,
                gauss_nodes.as_slice(),
                "{what}: the Gauss rule's own nodes"
            );
            assert_eq!(
                g.weights, rule.gauss,
                "{what}: the Gauss rule's own weights"
            );
        }
    }
}

/// ∫₀¹ eˣ dx = e − 1, ∫₋₁¹ 1/(1 + 25x²) dx = (2/5)·atan 5, ∫₀¹ sin(πt) dt = 2/π.
/// A named integrand on [a, b] with its integral.
type Case = (
    &'static str,
    fn(Real) -> Result<Real, NumError>,
    f64,
    f64,
    f64,
);

fn smooth_cases() -> Vec<Case> {
    fn e(x: Real) -> Result<Real, NumError> {
        math::exp(x.get())
    }
    fn runge(x: Real) -> Result<Real, NumError> {
        Real::new(1.0 / (1.0 + 25.0 * x.get() * x.get()))
    }
    fn sine(x: Real) -> Result<Real, NumError> {
        math::sinpi(x.get())
    }
    let e1 = math::exp(1.0).unwrap().get() - 1.0;
    let atan5 = math::atan2(5.0, 1.0).unwrap().get();
    let pi = 4.0 * math::atan2(1.0, 1.0).unwrap().get();
    vec![
        ("exp", e, 0.0, 1.0, e1),
        ("runge", runge, -1.0, 1.0, 0.4 * atan5),
        ("sine", sine, 0.0, 1.0, 2.0 / pi),
    ]
}

#[test]
fn the_integrator_meets_its_tolerance_on_known_integrals() {
    for (name, f, a, b, want) in smooth_cases() {
        for (rule, kronrod) in GAUSS_KRONROD.iter().enumerate() {
            let got = integrate(f, r(a), r(b), r(1e-13), r(0.0), rule, 200).unwrap();
            assert!(got.error.get() <= 1e-13, "{name}, rule {rule}: {got:?}");
            assert!(
                (got.value.get() - want).abs() <= 4e-15,
                "{name}, rule {rule}: {got:?} against {want}"
            );
            assert_eq!(
                got.evaluations as usize,
                (2 * got.panels as usize - 1) * kronrod.nodes.len()
            );
        }
    }
    // Runge's function needs splitting with the smallest rule; the exponential does not.
    let runge = smooth_cases()[1].1;
    assert!(
        integrate(runge, r(-1.0), r(1.0), r(1e-13), r(0.0), 0, 200)
            .unwrap()
            .panels
            > 1
    );
    let e = smooth_cases()[0].1;
    assert_eq!(
        integrate(e, r(0.0), r(1.0), r(1e-13), r(0.0), 0, 200)
            .unwrap()
            .panels,
        1
    );
}

#[test]
fn fixed_rules_integrate_polynomials_and_smooth_functions() {
    // x^5 − 3x^2 + 1 on [−2, 3]: 3^6/6 − 2^6/6 − (27 + 8) + 5 = 665/6 − 30.
    let p = |x: Real| -> Result<Real, NumError> {
        let x = x.get();
        Real::new(x * x * x * x * x - 3.0 * x * x + 1.0)
    };
    for n in 3..=20 {
        let got = gauss_legendre(p, r(-2.0), r(3.0), n).unwrap().get();
        assert!((got - (665.0 / 6.0 - 30.0)).abs() < 1e-12, "n = {n}: {got}");
    }
    let (_, e, a, b, want) = smooth_cases()[0];
    assert!((gauss_legendre(e, r(a), r(b), 20).unwrap().get() - want).abs() <= 4e-16);
}

#[test]
fn errors_are_reported_never_answered() {
    let one = |_: Real| Real::new(1.0);
    assert_eq!(
        gauss_legendre(one, r(1.0), r(1.0), 5),
        Err(QuadError::BadInterval)
    );
    assert_eq!(
        gauss_legendre(one, r(0.0), r(1.0), 0),
        Err(QuadError::NoSuchRule)
    );
    assert_eq!(
        gauss_legendre(one, r(0.0), r(1.0), 21),
        Err(QuadError::NoSuchRule)
    );
    assert_eq!(gauss_legendre(one, r(0.0), r(1.0), 1), Ok(r(1.0)));
    assert_eq!(
        integrate(one, r(2.0), r(1.0), r(0.0), r(0.0), 0, 10),
        Err(QuadError::BadInterval)
    );
    assert_eq!(
        integrate(one, r(0.0), r(1.0), r(-1.0), r(0.0), 0, 10),
        Err(QuadError::BadInterval)
    );
    assert_eq!(
        integrate(one, r(0.0), r(1.0), r(0.0), r(0.0), 6, 10),
        Err(QuadError::NoSuchRule)
    );
    // √x has an unbounded derivative at 0: a cap of 3 bisections cannot reach 1e-15.
    let root = |x: Real| math::sqrt(x.get());
    assert_eq!(
        integrate(root, r(0.0), r(1.0), r(1e-15), r(0.0), 0, 3),
        Err(QuadError::CapReached)
    );
    // With the largest cap the panels run out first.
    assert_eq!(
        integrate(root, r(0.0), r(1.0), r(0.0), r(0.0), 0, u32::MAX),
        Err(QuadError::CapReached)
    );
    const { assert!(MAX_PANELS >= 64) };
    // A failing function's error comes back as it is.
    let fails = |_: Real| -> Result<Real, NumError> { Err(NumError::OutsideDomain) };
    assert_eq!(
        integrate(fails, r(0.0), r(1.0), r(1.0), r(0.0), 0, 10),
        Err(QuadError::Num(NumError::OutsideDomain))
    );
    // A panel one step wide, just below 1, where the steps halve: every node rounds to one end or
    // the other, so the function is constant on the nodes the rule sees and the integral is
    // 1e300 · (b − a), exactly what is returned, with no split.
    let b = 1.0f64;
    let a = b.next_down();
    let jump = move |x: Real| Real::new(if x.get() > a { 1e300 } else { 0.0 });
    let got = integrate(jump, r(a), r(b), r(0.0), r(0.0), 0, 10).unwrap();
    assert_eq!((got.value.get(), got.panels), (1e300 * (b - a), 1));
}

#[test]
fn a_relative_tolerance_is_honoured() {
    let (_, e, a, b, want) = smooth_cases()[0];
    let got = integrate(e, r(a), r(b), r(0.0), r(1e-12), 1, 50).unwrap();
    assert!(got.error.get() <= 1e-12 * got.value.get().abs());
    assert!((got.value.get() - want).abs() <= 1e-14);
}
