//! The basis functions of `geom_nurbs::basis` against the exact rational reference: spans by
//! their definition, values within γ_(6p) of the Cox–de Boor recurrence, derivatives within
//! γ_(6p+2) of the scheme's magnitudes, and every error the module documents.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::many_single_char_names,
    clippy::doc_markdown,
    clippy::needless_range_loop,
    clippy::range_plus_one,
    clippy::assigning_clones,
    clippy::similar_names,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod common;

use common::{
    Lcg, Q, cox_de_boor, derivatives, gamma, knots, magnitudes_a23, magnitudes_a25, q, within, zero,
};
use geom_nurbs::{Basis, Limit, NurbsError};

const LIMITS: [Limit; 2] = [Limit::FromRight, Limit::FromLeft];

/// Bases of several degrees, clamped and not, with every kind of interior knot.
fn bases() -> Vec<Basis> {
    let mut rng = Lcg(0x5eed_0056);
    let mut out = Vec::new();
    for p in [1, 2, 3, 4, 5, 7] {
        for kind in 0..3 {
            for clamped in [true, false] {
                let n = p + 1 + rng.below(6);
                out.push(Basis::new(p, knots(&mut rng, p, n, clamped, kind)).unwrap());
            }
        }
    }
    for p in [12, 28] {
        out.push(Basis::new(p, knots(&mut rng, p, p + 2, true, 1)).unwrap());
    }
    out
}

/// Parameters to test: every distinct knot in the domain and a few points between.
fn parameters(basis: &Basis, rng: &mut Lcg, between: usize) -> Vec<f64> {
    let (lo, hi) = basis.domain();
    let mut out: Vec<f64> = basis
        .knots()
        .iter()
        .copied()
        .filter(|&k| lo <= k && k <= hi)
        .collect();
    out.dedup();
    for _ in 0..between {
        out.push(lo + (hi - lo) * rng.unit());
    }
    out
}

fn effective(basis: &Basis, u: f64, limit: Limit) -> Limit {
    common::effective(basis.knots(), basis.degree(), u, limit)
}

#[test]
fn spans_meet_their_definition() {
    let mut rng = Lcg(1);
    for basis in bases() {
        let u_ = basis.knots();
        for u in parameters(&basis, &mut rng, 20) {
            for limit in LIMITS {
                let j = basis.span(u, limit).unwrap();
                assert!(u_[j] < u_[j + 1], "span {j} is one value");
                assert!(j >= basis.degree() && j < basis.count());
                match effective(&basis, u, limit) {
                    Limit::FromRight => assert!(u_[j] <= u && u < u_[j + 1]),
                    Limit::FromLeft => assert!(u_[j] < u && u <= u_[j + 1]),
                }
            }
        }
    }
}

#[test]
fn values_are_within_their_bound_of_the_recurrence() {
    let mut rng = Lcg(2);
    for basis in bases() {
        let p = basis.degree();
        let between = if p > 10 { 2 } else { 12 };
        let bound_factor = gamma(6 * p);
        for u in parameters(&basis, &mut rng, between) {
            for limit in LIMITS {
                let mut n = vec![0.0; p + 1];
                let span = basis.eval(u, limit, &mut n).unwrap();
                let exact = cox_de_boor(basis.knots(), p, u, limit);
                for (r, &value) in n.iter().enumerate() {
                    assert!((0.0..=1.0).contains(&value));
                    let e = &exact[p][span - p + r];
                    let bound = &bound_factor * e;
                    assert!(within(value, e, &bound), "degree {p}, u {u}, function {r}");
                }
                // Every function outside the span vanishes exactly in the reference too.
                let outside: Q = exact[p]
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i + p < span || *i > span)
                    .fold(zero(), |acc, (_, v)| acc + v);
                assert_eq!(outside, zero());
            }
        }
    }
}

#[test]
fn clamped_ends_are_exact() {
    for basis in bases().into_iter().filter(Basis::is_clamped) {
        let p = basis.degree();
        let (lo, hi) = basis.domain();
        let last = basis.count() - 1;
        for limit in LIMITS {
            let mut n = vec![0.0; p + 1];
            basis.eval(lo, limit, &mut n).unwrap();
            assert_eq!(n[0], 1.0);
            assert!(n[1..].iter().all(|&v| v == 0.0));
            basis.eval(hi, limit, &mut n).unwrap();
            assert_eq!(n[p], 1.0);
            assert!(n[..p].iter().all(|&v| v == 0.0));
            assert_eq!(basis.eval_one(0, lo, limit).unwrap(), 1.0);
            assert_eq!(basis.eval_one(last, hi, limit).unwrap(), 1.0);
            assert_eq!(basis.eval_one(1, lo, limit).unwrap(), 0.0);
        }
    }
}

#[test]
fn derivatives_are_within_their_bound() {
    let mut rng = Lcg(3);
    for basis in bases() {
        let p = basis.degree();
        let between = if p > 10 { 1 } else { 8 };
        let order = p + 1;
        let bound_factor = gamma(6 * p + 2);
        for u in parameters(&basis, &mut rng, between) {
            for limit in LIMITS {
                let mut ders = vec![0.0; (order + 1) * (p + 1)];
                let span = basis.eval_derivs(u, limit, order, &mut ders).unwrap();
                let mut n = vec![0.0; p + 1];
                basis.eval(u, limit, &mut n).unwrap();
                // The values have the same bits as eval's.
                for r in 0..=p {
                    assert_eq!(ders[r].to_bits(), n[r].to_bits());
                }
                let exact = derivatives(basis.knots(), p, u, limit, order);
                let mags = magnitudes_a23(basis.knots(), p, span, u, limit, order);
                for k in 1..=order {
                    for r in 0..=p {
                        let value = ders[k * (p + 1) + r];
                        let e = &exact[k][span - p + r];
                        if k > p {
                            assert_eq!(value, 0.0);
                            assert_eq!(*e, zero());
                            continue;
                        }
                        let bound = &bound_factor * &mags[k][r];
                        assert!(
                            within(value, e, &bound),
                            "degree {p}, u {u}, derivative {k} of function {r}: {value}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn single_functions_are_within_their_bounds() {
    let mut rng = Lcg(4);
    for basis in bases().into_iter().filter(|b| b.degree() <= 7) {
        let p = basis.degree();
        let order = p + 1;
        let value_bound = gamma(6 * p);
        let der_bound = gamma(6 * p + 2);
        for u in parameters(&basis, &mut rng, 4) {
            for limit in LIMITS {
                let table = cox_de_boor(basis.knots(), p, u, limit);
                let exact = derivatives(basis.knots(), p, u, limit, order);
                for i in 0..basis.count() {
                    let value = basis.eval_one(i, u, limit).unwrap();
                    let e = &table[p][i];
                    assert!(within(value, e, &(&value_bound * e)), "N_{i} at {u}");
                    let mut out = vec![0.0; order + 1];
                    basis.eval_one_derivs(i, u, limit, &mut out).unwrap();
                    assert!(within(out[0], e, &(&value_bound * e)));
                    let mags = magnitudes_a25(basis.knots(), p, i, u, limit, order);
                    for k in 1..=order {
                        if k > p {
                            assert_eq!(out[k], 0.0);
                            continue;
                        }
                        let bound = &der_bound * &mags[k];
                        assert!(
                            within(out[k], &exact[k][i], &bound),
                            "degree {p}, N_{i} at {u}, derivative {k}: {}",
                            out[k]
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn construction_refuses_each_rule_in_order() {
    let ok = vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0];
    assert!(Basis::new(2, ok.clone()).is_ok());
    assert_eq!(
        Basis::new(0, ok.clone()),
        Err(NurbsError::Degree { degree: 0 })
    );
    assert_eq!(
        Basis::new(29, ok.clone()),
        Err(NurbsError::Degree { degree: 29 })
    );
    assert_eq!(
        Basis::new(3, ok.clone()),
        Err(NurbsError::KnotCount {
            knots: 7,
            needed: 8
        })
    );
    assert_eq!(
        Basis::new(2, vec![0.0, 0.0, f64::NAN, 1.0, 2.0, 2.0]),
        Err(NurbsError::KnotNotFinite { index: 2 })
    );
    assert_eq!(
        Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 2.0, f64::INFINITY]),
        Err(NurbsError::KnotNotFinite { index: 5 })
    );
    assert_eq!(
        Basis::new(2, vec![0.0, 0.0, 1.0, 0.5, 2.0, 2.0]),
        Err(NurbsError::KnotDecreasing { index: 3 })
    );
    assert_eq!(
        Basis::new(1, vec![0.0, 0.0, 1.0, 1.0, 1.0, 2.0]),
        Err(NurbsError::Multiplicity {
            index: 2,
            multiplicity: 3
        })
    );
    assert_eq!(
        Basis::new(1, vec![-1e308, -1e308, 1e308, 1e308]),
        Err(NurbsError::KnotRange)
    );
    assert_eq!(
        Basis::new(1, vec![0.0, 1.0, 1.0, 2.0]),
        Err(NurbsError::EmptyDomain)
    );
}

#[test]
fn parameters_outside_the_domain_are_refused() {
    let basis = Basis::new(2, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
    assert_eq!(basis.domain(), (2.0, 4.0));
    let mut n = [0.0; 3];
    for u in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1.999_999,
        4.000_001,
        0.0,
    ] {
        assert_eq!(
            basis.span(u, Limit::FromRight),
            Err(NurbsError::ParameterOutside)
        );
        assert_eq!(
            basis.eval(u, Limit::FromLeft, &mut n),
            Err(NurbsError::ParameterOutside)
        );
        assert_eq!(
            basis.eval_one(0, u, Limit::FromRight),
            Err(NurbsError::ParameterOutside)
        );
    }
}

#[test]
fn buffers_and_indices_are_checked() {
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]).unwrap();
    assert_eq!(basis.count(), 4);
    let mut short = [0.0; 2];
    assert_eq!(
        basis.eval(0.5, Limit::FromRight, &mut short),
        Err(NurbsError::BufferLength {
            given: 2,
            needed: 3
        })
    );
    let mut ders = [0.0; 8];
    assert_eq!(
        basis.eval_derivs(0.5, Limit::FromRight, 2, &mut ders),
        Err(NurbsError::BufferLength {
            given: 8,
            needed: 9
        })
    );
    assert_eq!(
        basis.eval_one(4, 0.5, Limit::FromRight),
        Err(NurbsError::Index { index: 4, count: 4 })
    );
    assert_eq!(
        basis.eval_one_derivs(0, 0.5, Limit::FromRight, &mut []),
        Err(NurbsError::BufferLength {
            given: 0,
            needed: 1
        })
    );
}

#[test]
fn overflowing_derivatives_are_refused() {
    // A span of 10^−300 beside spans of 1: the second derivative is about 10^600.
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1e-300, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut ders = vec![0.0; 3 * 4];
    assert_eq!(
        basis.eval_derivs(5e-301, Limit::FromRight, 2, &mut ders),
        Err(NurbsError::NotFinite)
    );
    let mut one = [0.0; 3];
    assert_eq!(
        basis.eval_one_derivs(0, 5e-301, Limit::FromRight, &mut one),
        Err(NurbsError::NotFinite)
    );
    // The values themselves never overflow.
    let mut n = [0.0; 4];
    basis.eval(5e-301, Limit::FromRight, &mut n).unwrap();
    assert!(n.iter().all(|v| (0.0..=1.0).contains(v)));
}

#[test]
fn derivatives_jump_at_a_knot_from_each_side() {
    // A quadratic with a double interior knot at 1: C0 there, so the first derivative jumps.
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0]).unwrap();
    let mut right = vec![0.0; 2 * 3];
    let mut left = vec![0.0; 2 * 3];
    let sr = basis
        .eval_derivs(1.0, Limit::FromRight, 1, &mut right)
        .unwrap();
    let sl = basis
        .eval_derivs(1.0, Limit::FromLeft, 1, &mut left)
        .unwrap();
    assert_eq!((sl, sr), (2, 4));
    let exact_right = derivatives(basis.knots(), 2, 1.0, Limit::FromRight, 1);
    let exact_left = derivatives(basis.knots(), 2, 1.0, Limit::FromLeft, 1);
    for r in 0..3 {
        assert_eq!(q(right[3 + r]), exact_right[1][sr - 2 + r]);
        assert_eq!(q(left[3 + r]), exact_left[1][sl - 2 + r]);
    }
}
