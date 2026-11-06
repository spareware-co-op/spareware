//! Attacks on piece G68's claims in `geom-curve`: first and second derivative bounds of rational
//! curves with weights from 10⁻³ to 10³, never below the exact values.
//!
//! - **Never below the exact values.** Seeded NURBS curves of degree 1 to 5 in two and three
//!   dimensions, most rational with weights 2^e·(1 + f), e a whole
//!   number from −10 to 9 and f in [0, 1) (about 10⁻³ to 2·10³), with 10⁻³ and 10³ themselves, random spans of their domains, and a dense row of parameters in each: at each,
//!   the exact C, C′ and C″ come from `geom_test::nurbs_reference` in exact rationals (the
//!   homogeneous derivatives from blossoms, then C′ = (A′ − W′·C)/W and
//!   C″ = (A″ − 2W′·C′ − W″·C)/W exactly), and must lie in the bounds of order 0, 1 and 2 over the
//!   span.
//! - **The curve bounded by its control points.** Leibniz's rule for a rational piece starts from
//!   the hull of its Euclidean control points (chapter 19.3), not from the boxes of A and W: with
//!   a weight of 10⁻³ beside one of 10³, the second derivative's bound over each tenth of the domain
//!   stays under 1,000 times its sampled size, where the boxes alone give over 3,000 times.
//! - **Intersecting enclosures.** `tight::meet` keeps the common part of two enclosures, and the
//!   first when they miss.
//!
//! Every curve is drawn from a printed seed (`SPAREKIT_SEED` replays one).
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    clippy::cast_lossless,
    reason = "tests: oracles and generators, and a failure stops the test"
)]

use core::num::NonZeroU64;

use dashu_ratio::RBig;
use geom_curve::tight;
use geom_curve::{DerivativeBound, ParamCurve};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Bounds, Curve};
use geom_run::{Budget, WorkMeter};
use geom_test::nurbs_reference::Spline;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// All the work in release builds, a tenth in debug builds.
fn scale(n: usize) -> usize {
    if cfg!(debug_assertions) {
        (n / 10).max(1)
    } else {
        n
    }
}

fn q(x: f64) -> RBig {
    RBig::try_from(x).unwrap()
}

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

fn unit(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

/// 2^e, exactly.
fn pow2(e: i64) -> f64 {
    let mut x = 1.0;
    for _ in 0..e.unsigned_abs() {
        x *= 2.0;
    }
    if e < 0 { 1.0 / x } else { x }
}

/// A seeded curve: degree 1 to 5, a clamped knot vector with up to three interior knots, points
/// in [−10, 10]^D, and (for three in four) weights 2^e·(1 + f), e a whole
/// number from −10 to 9 and f in [0, 1), with 10⁻³ and 10³ themselves.
fn curve<const D: usize>(rng: &mut SplitMix64) -> Curve<D> {
    let p = 1 + below(rng, 5) as usize;
    let mut k = vec![0.0; p + 1];
    let mut x = 0.0;
    for _ in 0..below(rng, 4) {
        x += 0.25 + unit(rng);
        k.push(x);
    }
    x += 0.5 + unit(rng);
    k.extend(std::iter::repeat_n(x, p + 1));
    let basis = Basis::new(p, k).unwrap();
    let n = basis.count();
    let pts: Vec<[f64; D]> = (0..n)
        .map(|_| [0.0; D].map(|_| unit(rng) * 20.0 - 10.0))
        .collect();
    if below(rng, 4) == 0 {
        return Curve::new(basis, &pts).unwrap();
    }
    let mut w: Vec<f64> = (0..n)
        .map(|_| pow2(below(rng, 20).cast_signed() - 10) * (1.0 + unit(rng)))
        .collect();
    let (i, j) = (below(rng, n as u64) as usize, below(rng, n as u64) as usize);
    w[i] = 1e-3;
    w[j] = 1e3;
    Curve::new_rational(basis, &pts, &w).unwrap()
}

/// The reference curve of a kernel curve: its stored homogeneous points and weights, exactly.
fn reference<const D: usize>(c: &Curve<D>) -> Spline {
    let b = c.basis();
    let pts: Vec<&[f64]> = c.homogeneous().iter().map(|p| &p[..]).collect();
    match c.weights() {
        Some(w) => Spline::new_rational(b.degree(), b.knots(), &pts, w).unwrap(),
        None => Spline::new(b.degree(), b.knots(), &pts).unwrap(),
    }
}

/// The exact C, C′ and C″ at u.
fn exact<const D: usize>(s: &Spline, u: &RBig) -> [Vec<RBig>; 3] {
    let h: [Vec<RBig>; 3] = [0, 1, 2].map(|k| s.homogeneous_derivative(u, k).unwrap());
    if !s.is_rational() {
        return h;
    }
    let (a, w) = (|k: usize| &h[k][..D], |k: usize| &h[k][D]);
    let c: Vec<RBig> = a(0).iter().map(|x| x / w(0)).collect();
    let c1: Vec<RBig> = (0..D).map(|d| (&a(1)[d] - w(1) * &c[d]) / w(0)).collect();
    let two = RBig::from(2u8);
    let c2: Vec<RBig> = (0..D)
        .map(|d| (&a(2)[d] - &two * w(1) * &c1[d] - w(2) * &c[d]) / w(0))
        .collect();
    [c, c1, c2]
}

fn holds<const D: usize>(b: &Bounds<D>, x: &[RBig]) -> bool {
    (0..D).all(|d| q(b.lo[d]) <= x[d] && x[d] <= q(b.hi[d]))
}

fn check<const D: usize>(label: &str) {
    let seed = resolve(label, Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut checked = 0usize;
    for case in 0..scale(150) {
        let c = curve::<D>(&mut rng);
        let s = reference(&c);
        let (a, b) = ParamCurve::domain(&c);
        let m = &mut meter();
        for _ in 0..3 {
            let span = match below(&mut rng, 3) {
                0 => (a, b),
                1 => {
                    let t = a + (b - a) * unit(&mut rng);
                    (t, (t + (b - a) * 0.01).min(b))
                }
                _ => {
                    let (s, t) = (a + (b - a) * unit(&mut rng), a + (b - a) * unit(&mut rng));
                    (s.min(t), s.max(t))
                }
            };
            let bounds: Vec<Bounds<D>> = (0..3)
                .map(|k| match c.derivative_bound(span, k, m).unwrap() {
                    DerivativeBound::Bounded(b) => b,
                    DerivativeBound::Unbounded => panic!("case {case}: order {k} unbounded"),
                })
                .collect();
            let g = if cfg!(debug_assertions) { 16 } else { 64 };
            for i in 0..=g {
                let t = (span.0 + (span.1 - span.0) * (i as f64 / g as f64)).min(span.1);
                let e = exact::<D>(&s, &q(t));
                for k in 0..3 {
                    assert!(
                        holds(&bounds[k], &e[k]),
                        "case {case}, order {k} at {t} over {span:?}: {:?} outside {:?}: {c:?}",
                        e[k],
                        bounds[k]
                    );
                }
                checked += 1;
            }
        }
    }
    println!("{label}: {checked} parameters checked");
    assert!(checked > scale(150) * 3 * 16);
}

#[test]
fn plane_curves_are_never_below_their_derivative_bounds() {
    check::<2>("G68 rational plane curves against the exact reference");
}

#[test]
fn space_curves_are_never_below_their_derivative_bounds() {
    check::<3>("G68 rational space curves against the exact reference");
}

#[test]
fn spread_weights_keep_the_second_derivative_bound_modest() {
    // A cubic Bezier arc whose weights are 10⁻³ and 10³ beside 1. From the boxes of A and W alone,
    // Leibniz's rule bounds C″ over the first tenth of the domain by over 3,000 times the largest
    // |C″| sampled densely there (measured off CI); from the hull, by about 110 times, and by at
    // most about 460 times over any tenth: under 1,000 times on each.
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let pts = [[0.0, 0.0], [1.0, 2.0], [3.0, 2.0], [4.0, 0.0]];
    let c = Curve::new_rational(basis, &pts, &[1.0, 1e-3, 1e3, 1.0]).unwrap();
    let s = reference(&c);
    let m = &mut meter();
    for i in 0..10 {
        let span = (f64::from(i) / 10.0, f64::from(i + 1) / 10.0);
        let DerivativeBound::Bounded(b) = c.derivative_bound(span, 2, m).unwrap() else {
            panic!("bounded");
        };
        let mut most = 0.0f64;
        for j in 0..=200 {
            let t = span.0 + (span.1 - span.0) * f64::from(j) / 200.0;
            for x in &exact::<2>(&s, &q(t))[2] {
                most = most.max(x.to_f64().value().abs());
            }
        }
        let size = (0..2).fold(0.0f64, |a, d| a.max(b.lo[d].abs()).max(b.hi[d].abs()));
        assert!(size <= 1000.0 * most, "{span:?}: {b:?} against {most}");
    }
}

#[test]
fn meet_keeps_the_common_part() {
    let iv = |a: f64, b: f64| Interval::new(a, b).unwrap();
    assert_eq!(tight::meet(iv(0.0, 2.0), iv(1.0, 3.0)), Ok(iv(1.0, 2.0)));
    assert_eq!(tight::meet(iv(0.0, 1.0), iv(1.0, 2.0)), Ok(iv(1.0, 1.0)));
    assert_eq!(tight::meet(iv(-1.0, 4.0), iv(0.0, 1.0)), Ok(iv(0.0, 1.0)));
    assert_eq!(tight::meet(iv(0.0, 1.0), iv(2.0, 3.0)), Ok(iv(0.0, 1.0)));
}
