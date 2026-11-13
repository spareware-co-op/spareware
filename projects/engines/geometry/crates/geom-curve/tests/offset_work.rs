//! What the meter charges an offset curve and the extents under it (issues #765 and #852): the
//! counts are the ones the modules state, an ordinary extent costs what it always did, every
//! evaluation costs about the curve kinds' rate at every order, and the offsets that ran for
//! thousands of millions of instructions under the curves fuzz target's budget now stop on it
//! early.
//!
//! - An extent costs one unit, and `FAR_CORNER` more for each corner of its arctangent's box that
//!   the certified arctangent takes to its 192-bit path: none for ordinary coefficients, a zero
//!   coefficient included, nor for the corners near an axis that the arctangent answers without
//!   it; counted on point and on wide coefficients, for each corner of a box on its own, at each
//!   of the arctangent's own cuts and one double either side, among subnormal coordinates too;
//!   and a budget one unit short stops it.
//! - An offset's evaluation costs its basis's plus `Offset::jet_work` and `Offset::basis_work`,
//!   at a point and over a span, in 2D and in 3D, at every order the cache uses; an order its
//!   jets cannot hold is refused before any work.
//! - Each such evaluation, measured, costs 235 to 350 instructions for each unit it is charged
//!   (issue #852: at order 0 over a span it was 2,389).
//! - The seven stored inputs of issue #852, whose conversions ran 3.5 to 11.9 thousand million
//!   instructions inside the fuzz target's budget, are stopped by it within 100,000 charges
//!   (about 81,000; the first of them took 555,008).
//! - The offset of issue #765's smallest input, under that input's transform and the fuzz
//!   target's budget of 2,000,000 units, is a typed stop, and an ordinary offset's whole kind
//!   contract still fits half that budget, in 2D and in 3D.
//!
//! Covers: `CurveKind::Offset`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "tests: native arithmetic and indexing, the formulas' names, and a failure stops the test"
)]

use std::sync::Mutex;

use geom_curve::extent::{FAR_CORNER, Scalar, extent};
use geom_curve::offset::{BOUND_WORK, JET_WORK, POINT_WORK, TERM_WORK};
use geom_curve::{
    Axes, Basis, CurveError, CurveKind, DerivativeBound, Ellipse, Offset, OffsetLaw, ParamCurve,
    Trimmed, contract, taylor,
};
use geom_math::{Axis, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::Limit;
use geom_run::{Budget, Cancel, POLL_EVERY, Stop, WorkMeter};

/// The curves fuzz target's budget (`geom_fuzz::curves::WORK`), which this crate cannot name.
const FUZZ_WORK: u64 = 2_000_000;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn point(x: f64) -> Interval {
    Interval::point(x).unwrap()
}

fn iv(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

/// a·cos πt + b·sin πt, with a slope q.
fn wave(q: Interval, a: Interval, b: Interval) -> Scalar {
    Scalar::Wave {
        p: point(0.0),
        q,
        a,
        b,
    }
}

/// The units one extent of `g` over [0, ½] is charged.
fn extent_work(g: &Scalar) -> u128 {
    let mut m = meter();
    extent(g, (0.0, 0.5), &mut m).unwrap();
    m.used().work
}

/// The units one extent of a·cos πt + b·sin πt is charged: its phase is atan2(y = b, x = a), and
/// for point coefficients its box's four corners are one point, asked four times.
fn phase_work(a: f64, b: f64) -> u128 {
    extent_work(&wave(point(0.0), point(a), point(b)))
}

/// 2^k for a normal double, exact: the exponent written into the bits.
fn two_to(k: i32) -> f64 {
    f64::from_bits(u64::try_from(k + 1023).unwrap() << 52)
}

/// The least subnormal double, 2⁻¹⁰⁷⁴.
const LEAST: f64 = f64::from_bits(1);

#[test]
fn an_ordinary_extent_costs_one_unit() {
    let zero = point(0.0);
    for (a, b) in [
        (3.0, 4.0),
        (-3.0, 4.0),
        // One double short of 2⁵³ apart, |y| the larger.
        (1.0, two_to(53).next_down()),
        (two_to(-53).next_up(), 1.0),
        // A zero coefficient is on an axis, not near one: its angle is exact or a quarter turn.
        (0.0, 5.0),
        (5.0, 0.0),
        (-5.0, 0.0),
    ] {
        assert_eq!(phase_work(a, b), 1, "{a} {b}");
    }
    // A line has no arctangent at all.
    assert_eq!(extent_work(&wave(point(two_to(-80)), zero, zero)), 1);
    // The other closed forms take none either.
    let quadratic = Scalar::Quadratic {
        p: zero,
        q: point(two_to(-80)),
        r: point(two_to(80)),
    };
    assert_eq!(extent_work(&quadratic), 1);
}

#[test]
fn an_extent_is_charged_for_each_dear_corner() {
    let zero = point(0.0);
    assert_eq!(FAR_CORNER, 40);
    let all = 1 + 4 * u128::from(FAR_CORNER);
    // Point coefficients: near the positive x-axis 2⁴⁹·⁸ apart, the quotient −10⁻¹⁵ not a double
    // (the fast paths cannot decide it); and past the first gate near +y, near −x and near −y.
    for (a, b) in [
        (1e15, -1.0),
        (two_to(-60), two_to(60)),
        (-two_to(60), two_to(-60)),
        (two_to(-1000), -1.0),
    ] {
        assert_eq!(phase_work(a, b), all, "{a} {b}");
    }
    // A wide coefficient: only the corners at its small end are dear (x from 2⁻⁵⁴ against y = 1,
    // past the first gate; y from 2⁻⁶⁰ against x = 3, near the positive x-axis).
    let two = 1 + 2 * u128::from(FAR_CORNER);
    assert_eq!(
        extent_work(&wave(zero, iv(two_to(-54), 0.5), point(1.0))),
        two
    );
    assert_eq!(
        extent_work(&wave(zero, point(3.0), iv(two_to(-60), 0.5))),
        two
    );
    // A slope adds a second arctangent, of ρ = q/(πR) against √(1 − ρ²) ≈ 1: a tiny slope's is
    // near the positive x-axis though the wave's own phase is not,
    assert_eq!(
        extent_work(&wave(point(two_to(-80)), point(3.0), point(4.0))),
        all
    );
    assert_eq!(
        extent_work(&wave(point(two_to(-30)), point(1.0), point(two_to(60)))),
        all + 4 * u128::from(FAR_CORNER)
    );
    // unless ρ is below 2⁻⁹⁶ (here about 2⁻¹⁴²), whose arctangent rounds ρ itself, cheaply.
    assert_eq!(
        extent_work(&wave(point(two_to(-80)), point(1.0), point(two_to(60)))),
        all
    );
    // An involute's phase is the same arctangent.
    let involute = Scalar::Involute {
        p: zero,
        a: point(two_to(-60)),
        b: point(1.0),
    };
    assert_eq!(extent_work(&involute), all);
}

#[test]
fn the_arctangents_own_cuts_set_the_charge() {
    // Each cut is the arctangent's (`geom_num::math`'s atan2): what decides whether a corner goes
    // to its 192-bit path, met exactly, and one double either side.
    let all = 1 + 4 * u128::from(FAR_CORNER);
    for (a, b, charged) in [
        // The first gate: bit patterns 53 binades apart, sent straight to the accurate path; with
        // |y| the larger, only past 53. Near −x, at the cut and one double short of it,
        (-two_to(53), 1.0, true),
        (-two_to(53).next_down(), 1.0, false),
        // and near +y, at the cut and one double past it.
        (1.0, two_to(53), false),
        (1.0, two_to(53).next_up(), true),
        // Among subnormal coordinates the bit patterns set the cut, not the values: 2⁻⁹⁷⁰ and the
        // least subnormal are 2¹⁰⁴ apart, but their patterns are one short of 53 binades, and the
        // fast paths (which scale such a corner up) decide it.
        (-two_to(-970), LEAST, false),
        (-two_to(-970).next_up(), LEAST, true),
        (LEAST, two_to(-970).next_up(), false),
        (LEAST, two_to(-970).next_up().next_up(), true),
        // Near the positive x-axis the fast paths decide nothing from 2⁴⁷ apart. At exactly 2⁴⁷
        // the quotient is a power of two, a double, which the accurate path returns as it is;
        // one double further it is not.
        (3.0 * two_to(47), 3.0, false),
        ((3.0 * two_to(47)).next_up(), 3.0, true),
        // From 2⁹⁶ apart the quotient is below 2⁻⁹⁶, and the accurate path returns its rounding.
        (two_to(96).next_down(), 1.0, true),
        (two_to(96), 1.0, false),
        (two_to(96).next_up(), 1.0, false),
        // An exact quotient in between is cheap too, here 10⁻¹⁵, 2⁻⁵³ and 2⁻⁵³ among subnormals
        // (but 2⁻⁵³/3 is not exact).
        (1.0, 1e-15, false),
        (two_to(53), 1.0, false),
        (two_to(-1021), LEAST, false),
        (3.0 * two_to(-1021), LEAST, true),
    ] {
        let want = if charged { all } else { 1 };
        assert_eq!(phase_work(a, b), want, "{a:e} {b:e}");
    }
}

#[test]
fn each_corner_of_the_box_is_charged_on_its_own() {
    // Four boxes, each with one dear corner, one at each place `Interval::atan2` takes a corner
    // from: (y.lo, x.hi) near the positive x-axis, (y.hi, x.lo) past the first gate near +y,
    // (y.lo, x.lo) past it near −x, and (y.hi, x.hi) near the positive x-axis from below.
    let one = 1 + u128::from(FAR_CORNER);
    let zero = point(0.0);
    let x_right = iv(3.0 * two_to(-30), 3.0);
    assert_eq!(extent_work(&wave(zero, x_right, iv(two_to(-60), 1.0))), one);
    assert_eq!(
        extent_work(&wave(
            zero,
            iv(two_to(-60), two_to(-30)),
            iv(two_to(-40), 1.0)
        )),
        one
    );
    assert_eq!(
        extent_work(&wave(
            zero,
            iv(-1.0, -two_to(-30)),
            iv(two_to(-60), two_to(-20))
        )),
        one
    );
    assert_eq!(
        extent_work(&wave(zero, x_right, iv(-1.0, -two_to(-60)))),
        one
    );
}

#[test]
fn a_box_around_the_negative_axis_is_not_charged_for_its_corners() {
    // `Interval::atan2` answers a box that holds the negative x-axis and points below it with
    // every angle, without taking a corner's: nothing dear runs, so nothing more is charged,
    // though all four corners are past the first gate.
    //
    // Such a wave's extent may go by pieces, a unit each and far fewer than one corner's charge, so
    // the corners charged are the whole multiples of `FAR_CORNER` in its work.
    let zero = point(0.0);
    let tiny = two_to(-60);
    let corners = |a: Interval, b: Interval| {
        let work = extent_work(&wave(zero, a, b));
        assert!(work % u128::from(FAR_CORNER) < 20, "{work}");
        work / u128::from(FAR_CORNER)
    };
    let left = iv(-1.0, -0.5);
    let across = iv(-tiny, tiny);
    assert_eq!(corners(left, across), 0);
    // Down to the axis itself.
    let below = iv(-tiny, 0.0);
    assert_eq!(corners(left, below), 0);
    // One condition short, the corners are taken and charged: a box right of the origin (its x
    // not a power of two, so that no quotient is exact),
    let right = iv(0.6, 1.2);
    assert_eq!(corners(right, across), 4);
    // one wholly below the axis,
    let under = iv(-2.0 * tiny, -tiny);
    assert_eq!(corners(left, under), 4);
    // one above it with its lower edge on the axis (those two corners are on it, not near it),
    let above = iv(0.0, tiny);
    assert_eq!(corners(left, above), 2);
    // and one whose left edge is on the y-axis.
    let from_axis = iv(0.0, 1.2);
    assert_eq!(corners(from_axis, across), 2);
}

#[test]
fn a_budget_one_unit_short_stops_an_extent() {
    let g = wave(point(0.0), point(two_to(-60)), point(two_to(60)));
    let needs = 1 + 4 * FAR_CORNER;
    let mut m = WorkMeter::new(Budget::of_work(needs));
    assert!(extent(&g, (0.0, 0.5), &mut m).is_ok());
    let mut m = WorkMeter::new(Budget::of_work(needs - 1));
    let Err(CurveError::Stopped(stop)) = extent(&g, (0.0, 0.5), &mut m) else {
        panic!("an extent the budget cannot pay for must stop");
    };
    assert!(matches!(stop.reason(), Stop::OverBudget(_)));
}

#[test]
fn the_jets_work_is_their_terms_and_their_recurrences() {
    // 3·D + 1 recurrences, each (order + 1)(order + 2)/2 terms at a unit a term and five units
    // whatever the order.
    assert_eq!(TERM_WORK, 1);
    assert_eq!(JET_WORK, 5);
    assert_eq!(Offset::<2>::jet_work(0).unwrap(), 7 * 6);
    assert_eq!(Offset::<3>::jet_work(0).unwrap(), 60);
    assert_eq!(Offset::<2>::jet_work(9).unwrap(), 7 * 60);
    assert_eq!(Offset::<3>::jet_work(9).unwrap(), 600);
    assert_eq!(Offset::<3>::jet_work(10).unwrap(), 710);
    for order in 0..40 {
        let n = order as u64 + 1;
        for (d, recurrences) in [(2, 7), (3, 10)] {
            let got = if d == 2 {
                Offset::<2>::jet_work(order)
            } else {
                Offset::<3>::jet_work(order)
            };
            assert_eq!(got.unwrap(), recurrences * (n * (n + 1) / 2 + 5));
        }
    }
    assert_eq!(Offset::<3>::jet_work(usize::MAX), Err(CurveError::Defect));
}

#[test]
fn the_basis_work_is_the_bounds_and_the_orders_asked() {
    // The basis is asked to one order higher: order + 2 bounds over a span, 19 units a
    // coordinate each, or an enclosure of order + 2 orders at a point, four units a coordinate an
    // order.
    assert_eq!(BOUND_WORK, 19);
    assert_eq!(POINT_WORK, 4);
    assert_eq!(Offset::<2>::basis_work(0, true).unwrap(), 2 * 2 * 19);
    assert_eq!(Offset::<3>::basis_work(0, true).unwrap(), 114);
    assert_eq!(Offset::<3>::basis_work(10, true).unwrap(), 684);
    assert_eq!(Offset::<2>::basis_work(0, false).unwrap(), 16);
    assert_eq!(Offset::<3>::basis_work(0, false).unwrap(), 24);
    assert_eq!(Offset::<3>::basis_work(9, false).unwrap(), 132);
    for order in 0..40 {
        let orders = order as u64 + 2;
        assert_eq!(Offset::<2>::basis_work(order, true), Ok(2 * orders * 19));
        assert_eq!(Offset::<3>::basis_work(order, true), Ok(3 * orders * 19));
        assert_eq!(Offset::<2>::basis_work(order, false), Ok(2 * orders * 4));
        assert_eq!(Offset::<3>::basis_work(order, false), Ok(3 * orders * 4));
    }
    for over_span in [false, true] {
        assert_eq!(
            Offset::<3>::basis_work(usize::MAX, over_span),
            Err(CurveError::Defect)
        );
    }
}

#[test]
fn an_order_the_jets_cannot_hold_is_refused_before_any_work() {
    // From order 170 the basis's jets would need 171!, past the doubles, so the jets would be
    // NotFinite whatever the basis gave: refused so before the basis is asked or anything is
    // charged, and never the Defect of a count past u64 (from about 1.9·10⁹).
    let offset = Offset::new(
        ellipse::<3>(),
        OffsetLaw::Reference {
            distance: 0.25,
            direction: [0.0, 0.0, 1.0],
        },
    )
    .unwrap();
    for order in [170, 171, 1_000_000, 1_400_000_000, usize::MAX] {
        let mut m = meter();
        let got = offset.enclose(0.125, Limit::FromRight, order, &mut m);
        assert_eq!(got.err(), Some(CurveError::NotFinite), "order {order}");
        assert_eq!(m.used().work, 0, "order {order}");
        let mut m = meter();
        let got = offset.derivative_bound((0.0, 0.0625), order, &mut m);
        assert_eq!(got.err(), Some(CurveError::NotFinite), "order {order}");
        assert_eq!(m.used().work, 0, "order {order}");
    }
    // One order lower the jets are built, and charged for.
    let mut m = meter();
    let _ = offset.enclose(0.125, Limit::FromRight, 169, &mut m);
    assert!(m.used().work >= u128::from(Offset::<3>::jet_work(169).unwrap()));
}

/// An ordinary ellipse in the plane z = 0, in D dimensions, trimmed to [−½, ½].
fn ellipse<const D: usize>() -> Trimmed<D> {
    let mut u = [0.0; D];
    let mut v = [0.0; D];
    (u[0], u[1]) = (3.0, 0.5);
    (v[0], v[1]) = (-0.25, 2.0);
    let e = Ellipse::new([0.0; D], Axes::Vectors { u, v }, (-1.0, 1.0)).unwrap();
    Trimmed::new(Basis::Ellipse(e), (-0.5, 0.5)).unwrap()
}

/// What the basis charges for what one evaluation of an offset over `span` asks it: its bound and
/// its derivative bounds to `order` + 1.
fn basis_over<const D: usize>(basis: &Trimmed<D>, span: (f64, f64), order: usize) -> u128 {
    let mut m = meter();
    basis.bound(span, &mut m).unwrap();
    for j in 1..=order + 1 {
        basis.derivative_bound(span, j, &mut m).unwrap();
    }
    m.used().work
}

fn evaluations_cost_the_basis_and_the_jets<const D: usize>(law: OffsetLaw) {
    let basis = ellipse::<D>();
    let offset = Offset::new(basis.clone(), law).unwrap();
    for order in 0..=10 {
        let jets = u128::from(Offset::<D>::jet_work(order).unwrap());
        let asked = |over_span| u128::from(Offset::<D>::basis_work(order, over_span).unwrap());
        // At a point: the basis's enclosure one order higher.
        let mut m = meter();
        offset
            .enclose(0.125, Limit::FromRight, order, &mut m)
            .unwrap();
        let mut b = meter();
        basis
            .enclose(0.125, Limit::FromRight, order + 1, &mut b)
            .unwrap();
        assert_eq!(
            m.used().work,
            b.used().work + jets + asked(false),
            "order {order}"
        );
        // Over a span: four pieces, each the basis's bounds and one evaluation of the jets.
        let span = (0.0, 0.0625);
        let mut m = meter();
        let bound = if order == 0 {
            offset.bound(span, &mut m).map(|_| ())
        } else {
            offset.derivative_bound(span, order, &mut m).map(|b| {
                assert!(matches!(b, DerivativeBound::Bounded(_)), "order {order}");
            })
        };
        bound.unwrap();
        let cuts = taylor::cuts_between(span, &[], 4).unwrap();
        let mut want: u128 = 0;
        for w in cuts.windows(2) {
            want += basis_over(&basis, (w[0], w[1]), order) + jets + asked(true);
        }
        assert_eq!(cuts.len(), 5);
        assert_eq!(m.used().work, want, "order {order}");
    }
}

#[test]
fn an_offsets_evaluation_costs_its_basis_and_its_jets() {
    evaluations_cost_the_basis_and_the_jets::<2>(OffsetLaw::Planar { distance: 0.25 });
    evaluations_cost_the_basis_and_the_jets::<3>(OffsetLaw::Reference {
        distance: 0.25,
        direction: [0.0, 0.0, 1.0],
    });
}

/// An ellipse with no zero coefficient, in D dimensions, trimmed to [−½, ½]: the one the
/// instructions of [`MEASURED`] were counted on.
fn full_ellipse<const D: usize>() -> Trimmed<D> {
    let mut u = [0.0; D];
    let mut v = [0.0; D];
    (u[0], u[1]) = (3.0, 0.5);
    (v[0], v[1]) = (-0.25, 2.0);
    if D == 3 {
        (u[2], v[2]) = (0.125, 0.75);
    }
    let e = Ellipse::new([0.0; D], Axes::Vectors { u, v }, (-1.0, 1.0)).unwrap();
    Trimmed::new(Basis::Ellipse(e), (-0.5, 0.5)).unwrap()
}

/// The instructions one evaluation of an offset costs at each order 0 to 10, its basis's share
/// included: (dimension, whether over a span, the instructions by order). Counted with callgrind
/// on a release build for x86-64 (issue #852): 100 evaluations at each order of the offset of
/// [`full_ellipse`] by 0.25 (in 3D along the reference direction (0.25, 0.5, 1)), at the
/// parameter 0.125 and over each quarter of the span [0, 0.0625].
const MEASURED: [(usize, bool, [u64; 11]); 4] = [
    (
        2,
        true,
        [
            40_517, 59_358, 79_577, 100_935, 123_885, 148_208, 174_049, 202_815, 231_128, 261_296,
            292_912,
        ],
    ),
    (
        2,
        false,
        [
            16_645, 23_512, 31_635, 40_674, 51_392, 63_423, 76_833, 93_103, 108_731, 126_239,
            145_026,
        ],
    ),
    (
        3,
        true,
        [
            62_123, 90_772, 121_707, 154_454, 189_117, 225_867, 264_773, 308_851, 351_796, 397_003,
            443_863,
        ],
    ),
    (
        3,
        false,
        [
            24_834, 34_756, 46_740, 60_216, 75_587, 92_932, 112_245, 136_381, 159_262, 184_396,
            210_927,
        ],
    ),
];

/// The units one evaluation of `offset` to `order` is charged in all, its basis's own charges
/// included: at the parameter 0.125, or over one quarter of the span [0, 0.0625] (a bound over the
/// span is four evaluations, one a quarter, each charged the same).
fn evaluation_work<const D: usize>(offset: &Offset<D>, order: usize, over_span: bool) -> u64 {
    let mut m = meter();
    if over_span {
        let bound = offset
            .derivative_bound((0.0, 0.0625), order, &mut m)
            .unwrap();
        assert!(
            matches!(bound, DerivativeBound::Bounded(_)),
            "order {order}"
        );
        let work = u64::try_from(m.used().work).unwrap();
        assert_eq!(work % 4, 0, "order {order}");
        work / 4
    } else {
        offset
            .enclose(0.125, Limit::FromRight, order, &mut m)
            .unwrap();
        u64::try_from(m.used().work).unwrap()
    }
}

#[test]
fn every_evaluation_costs_about_the_curve_kinds_rate() {
    // The curve kinds' rate is about 285 instructions a unit (`geom_curve::offset`, "What the work
    // costs"). Every evaluation the measurement covers, at every order, costs 235 to 350
    // instructions for each unit it is charged. Before issue #852 an evaluation of order 0 over a
    // span was charged 26 units in 3D for its 62,115 instructions, 2,389 a unit, and one of order
    // 10 at a point 1,332 for 210,920, 158 a unit.
    let planar = Offset::new(full_ellipse::<2>(), OffsetLaw::Planar { distance: 0.25 }).unwrap();
    let space = Offset::new(
        full_ellipse::<3>(),
        OffsetLaw::Reference {
            distance: 0.25,
            direction: [0.25, 0.5, 1.0],
        },
    )
    .unwrap();
    for (dimension, over_span, instructions) in MEASURED {
        for (order, cost) in instructions.into_iter().enumerate() {
            let charged = if dimension == 2 {
                evaluation_work(&planar, order, over_span)
            } else {
                evaluation_work(&space, order, over_span)
            };
            let at = format!("{dimension}D, over a span {over_span}, order {order}: {charged}");
            assert!(charged * 235 <= cost, "{at} units for {cost}: overcharged");
            assert!(cost <= charged * 350, "{at} units for {cost}: undercharged");
        }
    }
    // The charges themselves, at the orders the conversion's halving and its pieces use.
    assert_eq!(evaluation_work(&space, 0, true), 180);
    assert_eq!(evaluation_work(&space, 10, true), 1_430);
    assert_eq!(evaluation_work(&space, 0, false), 86);
    assert_eq!(evaluation_work(&space, 9, false), 743);
    assert_eq!(evaluation_work(&planar, 0, true), 122);
    assert_eq!(evaluation_work(&planar, 10, true), 977);
    assert_eq!(evaluation_work(&planar, 0, false), 60);
    assert_eq!(evaluation_work(&planar, 9, false), 519);
}

/// A cancellation that never asks to stop and counts how often it is read (behind a lock: the
/// crate bans atomic counters, tests included). A meter reads it once every `POLL_EVERY` charges,
/// so the reads count the charges a call made before it ended.
#[derive(Default)]
struct Reads(Mutex<usize>);

impl Cancel for Reads {
    fn requested(&self) -> bool {
        *self.0.lock().unwrap() += 1;
        false
    }
}

/// The offsets of issue #852's seven stored curves inputs, as `geom_fuzz::curves` builds them, each
/// under the first eight digits of its input's SHA-1: an ellipse trimmed to [−½, ½], offset along
/// its tangent crossed with a reference direction that the tangent is parallel to somewhere in
/// the domain (ddc2b788, 1dabb018 and edf80e0c, at t = 0) or nearly parallel to. fd977595 is the
/// input a five-hour fuzzing campaign timed out on, past its limit of 10 s an input.
fn stored_offsets() -> Vec<(&'static str, Offset<3>)> {
    let build = |c: [f64; 3], u: [f64; 3], v: [f64; 3], distance: f64, direction: [f64; 3]| {
        let e = Ellipse::new(c, Axes::Vectors { u, v }, (-1.0, 1.0)).unwrap();
        let basis = Trimmed::new(Basis::Ellipse(e), (-0.5, 0.5)).unwrap();
        let law = OffsetLaw::Reference {
            distance,
            direction,
        };
        Offset::new(basis, law).unwrap()
    };
    let (far, large) = (two_to(48), two_to(40));
    let faint = f64::from_bits(0x2548_0832_05ED_02FF);
    vec![
        (
            "d881371f",
            build(
                [far, 3.0, 0.5],
                [0.5, 3.0, 65_536.0],
                [-0.5, far, far],
                -0.0,
                [0.0, -1.0, -1.0],
            ),
        ),
        (
            "8c111277",
            build(
                [1.0, large, large],
                [large, large, 65_536.0],
                [0.5, 65_536.0, 65_536.0],
                faint,
                [65_536.0, 65_536.0, f64::from_bits(0xA300_0000_0000_0007)],
            ),
        ),
        (
            "191381a4",
            build(
                [1.0, large, large],
                [large, large, 65_536.0],
                [0.5, 65_536.0, 65_536.0],
                faint,
                [65_536.0, 65_536.0, 9.0],
            ),
        ),
        (
            "ddc2b788",
            build(
                [1.0, 0.0, 0.0],
                [0.0, two_to(-12), 0.0],
                [5.0, 5.0, 5.0],
                5.0,
                [5.0, 5.0, 5.0],
            ),
        ),
        (
            "1dabb018",
            build(
                [
                    two_to(-56),
                    f64::from_bits(0x48FE_0600_EDE7_4800),
                    two_to(-56),
                ],
                [0.0, 0.0, 0.5],
                [65_536.0, 65_536.0, 0.0],
                0.5,
                [65_536.0, 65_536.0, 0.0],
            ),
        ),
        (
            "fd977595",
            build(
                [far, f64::from_bits(0xA2C8_4E33_184E_3118), 1.0],
                [f64::from_bits(0x1414_F314_1414_03F8), 2.5, -3.0],
                [0.0, far, far],
                -1.0,
                [1.0, 4_096.0, 4_096.0],
            ),
        ),
        (
            "edf80e0c",
            build(
                [0.0, -3.5, two_to(60)],
                [0.5, 0.0, 0.0],
                [65_536.0, 0.5, 0.0],
                0.5,
                [65_536.0, 0.5, 0.0],
            ),
        ),
    ]
}

#[test]
fn the_stored_offsets_of_issue_852_stop_early_on_the_fuzz_budget() {
    // Each conversion halves its pieces until the normal is apart from zero, which it never is,
    // or not for long: evaluations of order 0 over a span, 180 units each, so the budget of
    // 2,000,000 units pays for about 11,000 of them, each seven charges or more (its own and one
    // for each of the six extents it asks of its basis). Before issue #852 such an evaluation was
    // charged 26 units, the budget paid for 75,000 of them, and the conversions ran 3.5 to 11.9
    // thousand million instructions before it stopped them: the first made 555,008 charges, where
    // each now makes 81,024 to 81,728.
    for (name, offset) in stored_offsets() {
        let reads = Reads::default();
        let mut m = WorkMeter::new(Budget::of_work(FUZZ_WORK)).with_cancel(&reads);
        let Err(CurveError::Stopped(stop)) = offset.to_nurbs(&mut m) else {
            panic!("{name}: the conversion must stop on the budget");
        };
        assert!(matches!(stop.reason(), Stop::OverBudget(_)), "{name}");
        let charges = *reads.0.lock().unwrap() * POLL_EVERY;
        assert!(charges <= 100_000, "{name}: {charges} charges");
    }
}

/// The offset of issue #765's 24-byte curves input, as `geom_fuzz::curves` builds it: an ellipse
/// whose axes mix 2⁻⁵⁶, 1.1·10⁻⁷² and 65,536, offset by nothing along −x.
fn slow_offset() -> (Trimmed<3>, Offset<3>) {
    let e = Ellipse::new(
        [two_to(-56), 0.0, 0.0],
        Axes::Vectors {
            u: [two_to(-56), 1.0, 65_536.0],
            v: [0.5, f64::from_bits(0x3100_1037_3100_4848), two_to(-12)],
        },
        (-1.0, 1.0),
    )
    .unwrap();
    let basis = Trimmed::new(Basis::Ellipse(e), (-0.5, 0.5)).unwrap();
    let law = OffsetLaw::Reference {
        distance: 0.0,
        direction: [-1.0, 0.0, 0.0],
    };
    (basis.clone(), Offset::new(basis, law).unwrap())
}

#[test]
fn the_slow_offsets_basis_is_charged_for_its_arctangents() {
    // The derivative's phase in each coordinate is atan2(−π·u, π·v), π an enclosure. x: 2⁻⁵⁶
    // against ½, near the positive x-axis 2⁵⁵ apart, where the two corners that take both ends
    // of π from the same side have the exact quotient −2⁻⁵⁵ (cheap) and the two mixed ones are
    // dear; y: 1 against 1.1·10⁻⁷², past the first gate, all four dear; z: 65,536 against 2⁻¹²
    // (2²⁸ apart: ordinary).
    let (basis, _) = slow_offset();
    let mut m = meter();
    basis.derivative_bound((0.0, 0.125), 1, &mut m).unwrap();
    assert_eq!(m.used().work, 3 + 6 * u128::from(FAR_CORNER));
}

#[test]
fn the_slow_offset_of_issue_765_stops_on_the_fuzz_budget() {
    let (_, offset) = slow_offset();
    let kind = CurveKind::Offset(offset);
    let scale = TypedTransform::from(Transform::scale(1.0, 65_536.0, 1.0).unwrap());
    let mut m = WorkMeter::new(Budget::of_work(FUZZ_WORK));
    let Err(CurveError::Stopped(stop)) = kind.transformed(&scale, &mut m) else {
        panic!("the conversion must stop on the budget");
    };
    assert!(matches!(stop.reason(), Stop::OverBudget(_)));
    assert!(m.used().work > u128::from(FUZZ_WORK));
}

/// The units the whole kind contract of `offset` takes at five parameters under a turn that is not
/// exact, so the transform law converts it; on the fuzz target's budget, which it must fit.
fn contract_work<const D: usize>(offset: Offset<D>) -> u128 {
    let kind = CurveKind::Offset(offset);
    let turn = TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap());
    let mut m = WorkMeter::new(Budget::of_work(FUZZ_WORK));
    let params = [-0.5, -0.3, 0.0, 0.125, 0.5];
    assert_eq!(contract::check(&kind, &params, &turn, &mut m), Ok(()));
    m.used().work
}

#[test]
fn an_ordinary_offsets_whole_contract_fits_the_fuzz_budget() {
    // The price must not starve the fuzz target of the kind: an ordinary offset still goes through
    // every law of the contract, its conversions included, within half of the target's budget
    // (570,799 units in the plane and 822,794 in space, of 2,000,000; 569,760 and 805,123 before
    // issue #852).
    let planar =
        contract_work(Offset::new(ellipse::<2>(), OffsetLaw::Planar { distance: 0.25 }).unwrap());
    let space = contract_work(
        Offset::new(
            ellipse::<3>(),
            OffsetLaw::Reference {
                distance: 0.25,
                direction: [0.0, 0.0, 1.0],
            },
        )
        .unwrap(),
    );
    assert!(planar * 2 < u128::from(FUZZ_WORK), "{planar}");
    assert!(space * 2 < u128::from(FUZZ_WORK), "{space}");
}
