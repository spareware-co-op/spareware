//! What the meter charges an offset surface (issue #765): the count is the one the module states,
//! and the offset that ran for 16 thousand million instructions a conversion under the surfaces
//! fuzz target's budget, without reaching it, now stops on it.
//!
//! - `Offset::jet_work` is thirteen recurrences of C(order + 4, 4) terms, a unit a term.
//! - An offset's partials cost its base's one order higher plus the jets': one evaluation at a
//!   point, two over a box (the box's and its centre's, for the centred form), at every order
//!   the cache uses.
//! - The offset of issue #765's smallest input, under the fuzz target's budget of 4,000,000
//!   units, is a typed stop, and an ordinary offset's whole kind contract still fits that budget.
//!
//! Covers: `SurfaceKind::Offset`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "tests: native arithmetic and indexing, the formulas' names, and a failure stops the test"
)]

use geom_curve::{CurveError, Frame};
use geom_math::{Axis, Transform, TypedTransform};
use geom_nurbs::Limit;
use geom_run::{Budget, Stop, WorkMeter};
use geom_surface::{Cylinder, Offset, ParamBox, ParamSurface, SurfaceKind, contract};

/// The surfaces fuzz target's budget (`geom_fuzz::surfaces::WORK`), which this crate cannot name.
const FUZZ_WORK: u64 = 4_000_000;

const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// 2^k for a normal double, exact: the exponent written into the bits.
fn two_to(k: i32) -> f64 {
    f64::from_bits(u64::try_from(k + 1023).unwrap() << 52)
}

/// C(n, 4).
fn choose_four(n: u64) -> u64 {
    n * (n - 1) * (n - 2) * (n - 3) / 24
}

#[test]
fn the_jets_work_is_their_terms() {
    // One term at order 0, in thirteen recurrences, at a unit a term.
    assert_eq!(Offset::jet_work(0).unwrap(), 13);
    assert_eq!(Offset::jet_work(4).unwrap(), 13 * 70);
    assert_eq!(Offset::jet_work(8).unwrap(), 13 * 495);
    for order in 0..=18 {
        assert_eq!(
            Offset::jet_work(order).unwrap(),
            13 * choose_four(order as u64 + 4),
            "order {order}"
        );
    }
    // The count of terms is the count the recurrences run: each coefficient (k, l) of a product
    // sums (k + 1)·(l + 1) terms.
    for order in 0..=8_u64 {
        let mut terms = 0;
        for n in 0..=order {
            for l in 0..=n {
                terms += (n - l + 1) * (l + 1);
            }
        }
        assert_eq!(terms, choose_four(order + 4), "order {order}");
    }
    assert_eq!(Offset::jet_work(usize::MAX), Err(CurveError::Defect));
}

/// An ordinary cylinder and its offset.
fn ordinary() -> (SurfaceKind, Offset) {
    let frame = Frame::new([1.0, 0.25, 0.0], [0.0, 0.5, 2.0]).unwrap();
    let band: ParamBox = ((-0.5, 0.75), (-2.0, 3.0));
    let base = SurfaceKind::Cylinder(Cylinder::new([1.0, -2.0, 0.5], 3.0, frame, band).unwrap());
    let offset = Offset::new(base.clone(), 0.5).unwrap();
    (base, offset)
}

/// The units the base charges for its partials to `order` over `region`.
fn base_work(base: &SurfaceKind, region: ParamBox, order: usize) -> u128 {
    let mut m = meter();
    base.partials_over(region, SIDES, order, &mut m).unwrap();
    m.used().work
}

#[test]
fn an_offsets_partials_cost_its_bases_and_its_jets() {
    let (base, offset) = ordinary();
    let jets = |order: usize| u128::from(Offset::jet_work(order).unwrap());
    for order in 0..=7 {
        // At a point: one evaluation, from the base's partials one order higher.
        let at = ((0.125, 0.125), (0.5, 0.5));
        let mut m = meter();
        offset.partials_over(at, SIDES, order, &mut m).unwrap();
        assert_eq!(
            m.used().work,
            base_work(&base, at, order + 1) + jets(order),
            "order {order}"
        );
        // Over a box: the box's evaluation one order higher and its centre's, for the centred form.
        let region = ((0.0, 0.25), (0.0, 1.0));
        let centre = ((0.125, 0.125), (0.5, 0.5));
        let mut m = meter();
        offset.partials_over(region, SIDES, order, &mut m).unwrap();
        assert_eq!(
            m.used().work,
            base_work(&base, region, order + 2)
                + jets(order + 1)
                + base_work(&base, centre, order + 1)
                + jets(order),
            "order {order}"
        );
    }
}

#[test]
fn a_budget_one_unit_short_stops_an_evaluation() {
    let (base, offset) = ordinary();
    let at = ((0.125, 0.125), (0.5, 0.5));
    let needs = u64::try_from(base_work(&base, at, 5)).unwrap() + Offset::jet_work(4).unwrap();
    let mut m = WorkMeter::new(Budget::of_work(needs));
    assert!(offset.partials_over(at, SIDES, 4, &mut m).is_ok());
    let mut m = WorkMeter::new(Budget::of_work(needs - 1));
    let Err(CurveError::Stopped(stop)) = offset.partials_over(at, SIDES, 4, &mut m) else {
        panic!("an evaluation the budget cannot pay for must stop");
    };
    assert!(matches!(stop.reason(), Stop::OverBudget(_)));
}

#[test]
fn the_slow_offset_of_issue_765_stops_on_the_fuzz_budget() {
    // The 15-byte surfaces input, as `geom_fuzz::surfaces` builds it: a cylinder of radius 2⁴⁸
    // about a skew axis, from v = 1 to v = 2⁴⁸, offset by nothing, turned by −10 radians.
    let big = two_to(48);
    let frame = Frame::new([-8.0, big, big], [1.0, big, big]).unwrap();
    let cylinder = Cylinder::new([big, -8.0, 1.0], big, frame, ((0.0, 2.0), (1.0, big))).unwrap();
    let kind = SurfaceKind::Offset(Offset::new(SurfaceKind::Cylinder(cylinder), 0.0).unwrap());
    let turn = TypedTransform::from(Transform::rotation(Axis::Z, -10.0).unwrap());
    let mut m = WorkMeter::new(Budget::of_work(FUZZ_WORK));
    let Err(CurveError::Stopped(stop)) = kind.to_nurbs(&mut m) else {
        panic!("the conversion must stop on the budget");
    };
    assert!(matches!(stop.reason(), Stop::OverBudget(_)));
    assert!(m.used().work > u128::from(FUZZ_WORK));
    // And so the kind contract, which the fuzz target runs, ends at its NURBS law with that stop.
    let params = [(0.0, 1.0), (1.0, 2.0), (2.0, big)];
    let mut m = WorkMeter::new(Budget::of_work(FUZZ_WORK));
    let broken = contract::check(&kind, &params, &turn, &mut m).unwrap_err();
    assert!(
        matches!(broken.error, Some(CurveError::Stopped(_))),
        "{broken:?}"
    );
}

#[test]
fn an_ordinary_offsets_whole_contract_fits_the_fuzz_budget() {
    // The price must not starve the fuzz target of the kind: an ordinary offset still goes through
    // every law of the contract, its two conversions included, within the target's budget
    // (2,751,519 units of 4,000,000; one conversion is 1,337,370, two samplings of its cache). At
    // two units a term it would not, which is why a term is charged one.
    let (_, offset) = ordinary();
    let kind = SurfaceKind::Offset(offset);
    let turn = TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap());
    let params = [
        (-0.5, -2.0),
        (-0.25, 0.0),
        (0.0, 0.5),
        (0.125, 1.0),
        (0.75, 3.0),
    ];
    let mut m = WorkMeter::new(Budget::of_work(FUZZ_WORK));
    assert_eq!(contract::check(&kind, &params, &turn, &mut m), Ok(()));
    // Not by a hair: a quarter of the budget is left.
    assert!(m.used().work * 4 < u128::from(FUZZ_WORK) * 3);
}
