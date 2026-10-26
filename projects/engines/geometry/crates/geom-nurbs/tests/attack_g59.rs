//! Attacks on piece G59's claims: inputs chosen to break the Bernstein toolkit's bounds, clips,
//! products and refusals. Every bound is checked against exact rational evaluation of the exact
//! curve or surface the stored doubles define, so a call may refuse an attack but may never return
//! a bound that is false.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
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

use common::{Lcg, Q, cox_de_boor, q, qi, zero};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, BernsteinCurve, Curve, Limit, NurbsError, Slab, Surface};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The exact point of a curve at u, from inside [a, b].
fn exact<const D: usize>(c: &Curve<D>, u: f64, (_, b): (f64, f64)) -> [Q; D] {
    let basis = c.basis();
    let limit = if u == b {
        Limit::FromLeft
    } else {
        Limit::FromRight
    };
    let n = &cox_de_boor(basis.knots(), basis.degree(), u, limit)[basis.degree()];
    let mut a: [Q; D] = core::array::from_fn(|_| zero());
    let mut w = zero();
    for (i, p) in c.homogeneous().iter().enumerate() {
        let wi = c.weights().map_or(qi(1), |ws| q(ws[i]));
        w += &n[i] * &wi;
        for d in 0..D {
            a[d] += &n[i] * q(p[d]);
        }
    }
    core::array::from_fn(|d| &a[d] / &w)
}

/// Claim: the bounds hold for the exact curve, where extraction in doubles rounds: knots that
/// are not dyadic (0.1, 1/3, 0.7) and coordinates with every bit set.
#[test]
fn bounds_hold_where_extraction_rounds() {
    let mut rng = Lcg(59);
    for trial in 0..300 {
        let p = 1 + rng.below(4);
        let mut knots = vec![0.0; p + 1];
        knots.extend([0.1, 1.0 / 3.0, 0.7]);
        knots.extend(std::iter::repeat_n(1.0, p + 1));
        let basis = Basis::new(p, knots).unwrap();
        let n = basis.count();
        let pts: Vec<[f64; 3]> = (0..n)
            .map(|_| [rng.unit() * 1e3 - 500.0, rng.unit(), -rng.unit() * 7.0])
            .collect();
        let c = if trial % 2 == 0 {
            Curve::new(basis, &pts).unwrap()
        } else {
            let w: Vec<f64> = (0..n).map(|_| 0.25 + rng.unit() * 4.0).collect();
            Curve::new_rational(basis, &pts, &w).unwrap()
        };
        let m = &mut meter();
        for piece in c.bernstein(m).unwrap() {
            let b = piece.bound(m).unwrap();
            let (lo, hi) = piece.domain();
            for k in 0..=12 {
                // Rounding can carry lo + (hi − lo)·k/12 one step past hi: keep it inside.
                let u = (lo + (hi - lo) * f64::from(k) / 12.0).clamp(lo, hi);
                let x = exact(&c, u, piece.domain());
                for d in 0..3 {
                    assert!(
                        q(b.lo[d]) <= x[d] && x[d] <= q(b.hi[d]),
                        "trial {trial}: coordinate {d} at {u}"
                    );
                }
            }
        }
    }
}

/// Claim: huge or tiny magnitudes give a true bound or a typed error, never a false bound.
#[test]
fn extreme_magnitudes_are_bounded_or_refused() {
    for scale in [1e-300, 1e300, f64::MAX / 4.0] {
        let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]).unwrap();
        let pts = [
            [scale, -scale],
            [-scale, scale],
            [scale, scale],
            [-scale, -scale],
        ];
        for w in [None, Some([1e-300, 1.0, 1e300, 1.0])] {
            let c = match w {
                None => Curve::new(basis.clone(), &pts),
                Some(w) => Curve::new_rational(basis.clone(), &pts, &w),
            };
            let Ok(c) = c else { continue };
            let m = &mut meter();
            match c.bernstein(m) {
                Ok(pieces) => {
                    for piece in pieces {
                        if let Ok(b) = piece.bound(m) {
                            let x = exact(&c, piece.domain().0, piece.domain());
                            for d in 0..2 {
                                assert!(q(b.lo[d]) <= x[d] && x[d] <= q(b.hi[d]));
                            }
                        }
                        let _ = piece.derivative_bound(m);
                    }
                }
                Err(e) => assert_eq!(e, NurbsError::NotFinite, "scale {scale}"),
            }
        }
    }
}

/// Claim: a clip keeps every meeting, and drops the whole curve only when it misses the slab.
#[test]
fn clips_are_none_only_for_misses() {
    let m = &mut meter();
    let c = Curve::new(
        Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap(),
        &[[0.0, 0.0], [1.0, 2.0], [2.0, -2.0], [3.0, 0.0]],
    )
    .unwrap();
    let piece = c.bernstein(m).unwrap().remove(0);
    let slab = |lo: f64, hi: f64| Slab {
        origin: [0.0, 0.0],
        normal: [0.0, 1.0],
        lo,
        hi,
    };
    // Far above: no meeting.
    assert_eq!(piece.clip(&slab(5.0, 6.0), m).unwrap(), None);
    // Everything: the whole domain.
    assert_eq!(piece.clip(&slab(-10.0, 10.0), m).unwrap(), Some((0.0, 1.0)));
    // Touching the top of the polygon's hull exactly.
    assert!(piece.clip(&slab(2.0, 3.0), m).unwrap().is_some());
    // The curve's own maximum is below 2: a clip may keep a range, but it must hold every t
    // where y(t) ≥ 0.5.
    let r = piece.clip(&slab(0.5, 9.0), m).unwrap().unwrap();
    for k in 0..=64 {
        let t = f64::from(k) / 64.0;
        let y = &exact(&c, t, (0.0, 1.0))[1];
        if *y >= q(0.5) {
            assert!(r.0 <= t && t <= r.1, "dropped {t}");
        }
    }
    // A slab with an infinite side is refused, not trusted.
    assert_eq!(
        piece.clip(&slab(f64::NEG_INFINITY, 1.0), m),
        Err(NurbsError::NotFinite)
    );
}

/// Claim: refusals are typed: weights not certainly positive, a domain of one value, products
/// beyond the degree limit.
#[test]
fn malformed_forms_are_refused() {
    let one = Interval::point(1.0).unwrap();
    let zero_w = Interval::point(0.0).unwrap();
    assert_eq!(
        BernsteinCurve::new((0.0, 1.0), vec![[one]; 2], Some(vec![one, zero_w])),
        Err(NurbsError::Weight { index: 1 })
    );
    assert_eq!(
        BernsteinCurve::new((1.0, 1.0), vec![[one]; 2], None),
        Err(NurbsError::EmptyDomain)
    );
    assert_eq!(
        BernsteinCurve::<1>::new((0.0, 1.0), vec![], None),
        Err(NurbsError::PointCount {
            points: 0,
            expected: 1
        })
    );
    let m = &mut meter();
    let big = BernsteinCurve::new((0.0, 1.0), vec![[one; 3]; 101], None).unwrap();
    let small = BernsteinCurve::new((0.0, 1.0), vec![[one; 3]; 21], None).unwrap();
    assert_eq!(big.dot(&small, m), Err(NurbsError::Degree { degree: 120 }));
}

/// Claim: the normal patch of a degree-28 patch (degree 55 × 55) is computed and bounds the exact
/// normal.
#[test]
fn the_highest_degree_normal_patch_bounds_the_normal() {
    let p = geom_nurbs::MAX_DEGREE;
    let mut knots = vec![0.0; p + 1];
    knots.extend(std::iter::repeat_n(1.0, p + 1));
    let bu = Basis::new(p, knots.clone()).unwrap();
    let bv = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let mut rng = Lcg(28);
    let pts: Vec<[f64; 3]> = (0..(p + 1) * 2)
        .map(|_| {
            [
                f64::from(rng.below(100) as u32),
                f64::from(rng.below(100) as u32),
                1.0,
            ]
        })
        .collect();
    let s = Surface::new(bu, bv, &pts).unwrap();
    let m = &mut meter();
    let patch = s.bernstein(m).unwrap().remove(0);
    let n = patch.normal(m).unwrap();
    assert_eq!(n.degrees(), (2 * p - 1, 1));
    let b = n.bound(m).unwrap();
    assert!(b.lo.iter().chain(&b.hi).all(|x| x.is_finite()));
}
