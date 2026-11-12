//! What the meter charges a transition (issue #817): the counts are the ones the module states, and
//! the clothoid that ran for 38 seconds in the curves fuzz target's build under that target's
//! budget now stops on it, while an ordinary transition's whole kind contract still fits.
//!
//! - The unit tangent's derivatives to order k cost k·(k + 1)/2 terms of `TERM_WORK` units.
//! - An enclosure to order k costs its point's integral, eight nodes of `NODE_WORK`, and its
//!   derivatives' terms; a derivative bound over one station costs its order and one more, its
//!   piece's unit and the piece's terms; a parent costs each panel it tries, the ellipses of its
//!   remainder at `ELLIPSE_WORK` each, and each panel it keeps, an integral; for every law.
//! - The clothoid of the curves fuzz target's timeout input of 12 November 2026 (curvature from
//!   3·10⁻²⁹³ to 3 over 4,096, so 1,956 half-turns of heading) under that input's scale by 2⁴⁸,
//!   under the target's budget of 2,000,000 units, is a typed stop.
//! - The whole kind contract of an ordinary transition of each law, under a quarter turn, a scale
//!   and a turn that is not exact, fits half that budget.
//!
//! Covers: `CurveKind::Transition`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "tests: native arithmetic, the domain's names, and a failure stops the test"
)]

use geom_curve::transition::{ELLIPSE_WORK, NODE_WORK, NODES, TERM_WORK, units_work};
use geom_curve::{CurvatureLaw, CurveError, ParamCurve, Transition, contract};
use geom_math::{Axis, GridTransform, Transform, TypedTransform};
use geom_nurbs::Limit;
use geom_run::{Budget, Stop, WorkMeter};

/// The curves fuzz target's budget (`geom_fuzz::curves::WORK`), which this crate cannot name.
const FUZZ_WORK: u64 = 2_000_000;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// One transition of each law, of ordinary size.
fn laws() -> [(&'static str, CurvatureLaw, f64); 7] {
    [
        (
            "clothoid",
            CurvatureLaw::Clothoid {
                start: 0.0,
                end: 0.01,
            },
            100.0,
        ),
        (
            "Helmert",
            CurvatureLaw::Helmert {
                start: 0.001,
                end: 0.01,
            },
            100.0,
        ),
        (
            "Bloss",
            CurvatureLaw::Bloss {
                start: 0.0,
                end: 0.02,
            },
            80.0,
        ),
        (
            "cosine",
            CurvatureLaw::Cosine {
                start: 0.0,
                end: 0.02,
            },
            80.0,
        ),
        (
            "sine",
            CurvatureLaw::Sine {
                start: 0.001,
                end: 0.01,
            },
            100.0,
        ),
        (
            "cant-coupled",
            CurvatureLaw::CantCoupled {
                start: 0.001,
                end: 0.01,
                height: 1.5,
                cant: 0.1,
            },
            100.0,
        ),
        (
            "strong clothoid",
            CurvatureLaw::Clothoid {
                start: 3.0,
                end: 8.0,
            },
            5.0,
        ),
    ]
}

fn build(law: CurvatureLaw, length: f64, m: &mut WorkMeter<'_>) -> Transition<2> {
    Transition::new([1.0, 2.0], [0.6, 0.8], length, law, m).unwrap()
}

#[test]
fn the_derivatives_work_is_their_terms() {
    assert_eq!(units_work(0), Ok(0));
    assert_eq!(units_work(1), Ok(TERM_WORK));
    assert_eq!(units_work(2), Ok(3 * TERM_WORK));
    assert_eq!(units_work(10), Ok(55 * TERM_WORK));
    for k in 0..=40u64 {
        assert_eq!(
            units_work(k as usize),
            Ok(k * (k + 1) / 2 * TERM_WORK),
            "{k}"
        );
    }
}

#[test]
fn a_parent_costs_the_panels_it_tries_and_keeps() {
    let tried = 12 * ELLIPSE_WORK;
    let kept = NODES as u64 * NODE_WORK;
    for (name, law, length) in laws() {
        let mut m = meter();
        let k = build(law, length, &mut m);
        let p = k.panels() as u64;
        // The counts tried double from 1, or from 2 for Helmert's two halves, up to the one kept.
        let first = if matches!(law, CurvatureLaw::Helmert { .. }) {
            2
        } else {
            1
        };
        let mut all = 0;
        let mut c = first;
        while c <= p {
            all += c;
            c *= 2;
        }
        assert_eq!(m.used().work, u128::from(all * tried + p * kept), "{name}");
    }
}

#[test]
fn an_enclosure_costs_its_integral_and_its_terms() {
    for (name, law, length) in laws() {
        let k = build(law, length, &mut meter());
        let (a, b) = ParamCurve::<2>::domain(&k);
        for order in 0..=10 {
            for t in [a, a + (b - a) * 0.3, b] {
                let side = if t == b {
                    Limit::FromLeft
                } else {
                    Limit::FromRight
                };
                let mut m = meter();
                ParamCurve::<2>::enclose(&k, t, side, order, &mut m).unwrap();
                let want = NODES as u64 * NODE_WORK + units_work(order).unwrap();
                assert_eq!(
                    m.used().work,
                    u128::from(want),
                    "{name} order {order} at {t}"
                );
            }
        }
    }
}

#[test]
fn a_derivative_bound_over_one_station_costs_its_terms() {
    for (name, law, length) in laws() {
        let k = build(law, length, &mut meter());
        let (a, b) = ParamCurve::<2>::domain(&k);
        // Not at Helmert's middle, where the station is a piece of each half.
        let t = a + (b - a) * 0.3;
        for order in 1..=10 {
            let mut m = meter();
            ParamCurve::<2>::derivative_bound(&k, (t, t), order, &mut m).unwrap();
            let want = order as u64 + 1 + 1 + units_work(order).unwrap();
            assert_eq!(m.used().work, u128::from(want), "{name} order {order}");
        }
    }
}

/// The clothoid of the curves fuzz target's timeout input (`timeout-c49ae896…` in its crashes
/// folder), as `geom_fuzz::curves` builds it, and that input's transform, a scale by 2⁴⁸.
fn campaign_clothoid() -> (Transition<2>, TypedTransform) {
    let law = CurvatureLaw::Clothoid {
        // About 3·10⁻²⁹³.
        start: f64::from_bits(0x0333_2533_c84e_3318),
        end: 3.0,
    };
    let k = Transition::new(
        [1.25, 281_474_976_710_656.0],
        // About −8.2·10¹²⁴ and 7.1·10⁻³⁸, read from the input's bytes.
        [
            f64::from_bits(0xd9df_093a_6469_7353),
            f64::from_bits(0x3838_3838_3526_d537),
        ],
        4096.0,
        law,
        &mut WorkMeter::new(Budget::of_work(FUZZ_WORK)),
    )
    .unwrap();
    let s = 281_474_976_710_656.0;
    (
        k,
        TypedTransform::from(Transform::scale(s, s, 1.0).unwrap()),
    )
}

#[test]
fn the_campaign_clothoid_stops_on_the_fuzz_budget() {
    let (k, scale) = campaign_clothoid();
    assert_eq!(k.panels(), 4096);
    let mut m = WorkMeter::new(Budget::of_work(FUZZ_WORK));
    let Err(e) = contract::check(&k, &[0.0; 4], &scale, &mut m) else {
        panic!("the kind contract must stop on the budget");
    };
    let Some(CurveError::Stopped(stop)) = e.error else {
        panic!("not a budget stop: {e}");
    };
    assert!(matches!(stop.reason(), Stop::OverBudget(_)));
    assert!(m.used().work > u128::from(FUZZ_WORK));
}

#[test]
fn an_ordinary_transitions_whole_contract_fits_the_fuzz_budget() {
    // The price must not starve the fuzz target of the kind: an ordinary transition of each law
    // goes through every law of the contract, its conversion included, within half the target's
    // budget, under each kind of transform the target uses.
    let transforms = [
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1)),
        TypedTransform::from(Transform::scale(2.0, 2.0, 1.0).unwrap()),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
    ];
    for (name, law, length) in laws() {
        let k = build(law, length, &mut meter());
        let (a, b) = ParamCurve::<2>::domain(&k);
        let params = [a, a + (b - a) / 3.0, a + (b - a) * 2.0 / 3.0, b];
        for t in &transforms {
            let mut m = WorkMeter::new(Budget::of_work(FUZZ_WORK));
            assert_eq!(contract::check(&k, &params, t, &mut m), Ok(()), "{name}");
            let used = m.used().work;
            assert!(used * 2 < u128::from(FUZZ_WORK), "{name}: {used}");
        }
    }
}
