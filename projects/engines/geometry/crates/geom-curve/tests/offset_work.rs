//! What the meter charges an offset curve and the extents under it (issue #765): the counts are
//! the ones the modules state, an ordinary extent costs what it always did, and the offset that
//! ran for 77 thousand million instructions under the curves fuzz target's budget now stops on it.
//!
//! - An extent costs one unit, and `FAR_CORNER` more for each corner of its arctangent's box that
//!   the certified arctangent takes to its 192-bit path: none for ordinary coefficients, a zero
//!   coefficient included, nor for the corners near an axis that the arctangent answers without
//!   it; counted on point and on wide coefficients, for each corner of a box on its own, at each
//!   of the arctangent's own cuts and one double either side, among subnormal coordinates too;
//!   and a budget one unit short stops it.
//! - An offset's evaluation costs its basis's plus `Offset::jet_work`, at a point and over a span,
//!   in 2D and in 3D, at every order the cache uses.
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

use geom_curve::extent::{FAR_CORNER, Scalar, extent};
use geom_curve::offset::TERM_WORK;
use geom_curve::{
    Axes, Basis, CurveError, CurveKind, DerivativeBound, Ellipse, Offset, OffsetLaw, ParamCurve,
    Trimmed, contract, taylor,
};
use geom_math::{Axis, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::Limit;
use geom_run::{Budget, Stop, WorkMeter};

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
fn the_jets_work_is_their_terms() {
    // 3·D + 1 recurrences of (order + 1)(order + 2)/2 terms each, two units a term.
    assert_eq!(TERM_WORK, 2);
    assert_eq!(Offset::<2>::jet_work(0).unwrap(), 14);
    assert_eq!(Offset::<3>::jet_work(0).unwrap(), 20);
    assert_eq!(Offset::<2>::jet_work(9).unwrap(), 2 * 7 * 55);
    assert_eq!(Offset::<3>::jet_work(9).unwrap(), 1_100);
    assert_eq!(Offset::<3>::jet_work(10).unwrap(), 1_320);
    for order in 0..40 {
        let n = order as u64 + 1;
        for (d, recurrences) in [(2, 7), (3, 10)] {
            let got = if d == 2 {
                Offset::<2>::jet_work(order)
            } else {
                Offset::<3>::jet_work(order)
            };
            assert_eq!(got.unwrap(), recurrences * n * (n + 1));
        }
    }
    assert_eq!(Offset::<3>::jet_work(usize::MAX), Err(CurveError::Defect));
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
        // At a point: the basis's enclosure one order higher.
        let mut m = meter();
        offset
            .enclose(0.125, Limit::FromRight, order, &mut m)
            .unwrap();
        let mut b = meter();
        basis
            .enclose(0.125, Limit::FromRight, order + 1, &mut b)
            .unwrap();
        assert_eq!(m.used().work, b.used().work + jets, "order {order}");
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
            want += basis_over(&basis, (w[0], w[1]), order) + jets;
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
    // (532,800 units in the plane and 755,843 in space, of 2,000,000).
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
