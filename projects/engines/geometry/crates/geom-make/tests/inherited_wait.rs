//! An exact transform of a body whose data were already less than exact waits for the piece that
//! widens the checker when all the checker says is that it could not certify the result, and is
//! a defect only when the checker finds something wrong (the `Inherited` arm of the rule
//! `not_certified.rs` holds).
//!
//! From G134 the checker certifies every flat body an operation here returns that an exact
//! transform would move, so the body is made by hand: the rounded box of `not_certified.rs` with a
//! parameter curve on one fin, which the checker has no certified test for and reports as
//! unchecked, never as wrong. Nothing else in it is less than certified, so its report is
//! uncertified and has no other fault.
//!
//! Covers: `transform.mirror`, `transform.move`, `transform.rotate`.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a failure should stop loudly, in a helper as in a test"
)]

use geom_check::{FaultKind, Level, check};
use geom_curve::{CurveKind, Line};
use geom_make::{BoxInput, Made, Moved, make_box, mirror, move_by, rotate, scale};
use geom_math::{Axis, Rotation, Scale};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{
    ErrorKind, MakeDetail, OpContext, OpError, OpErrorDetail, OpId, OpOutput, OpPath, Remedy,
};
use geom_run::{Budget, WorkMeter};
use geom_topo::{Body, Builder, Ix};

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn s(num: u64, den: u64) -> Scale {
    Scale::new(num, den).unwrap()
}

/// The full checker's report on `body`, recorded `exactness`.
fn report(body: &Body, exactness: Exactness) -> geom_check::Report {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    check(body, exactness, Level::Full, &mut meter).unwrap()
}

/// `body` to transform, recorded `exactness`, any bound accepted in its class.
fn moved(body: &Body, exactness: Exactness) -> Moved<'_> {
    let accept = match exactness {
        Exactness::Approximate { step, .. } => Exactness::Approximate {
            deviation: Steps(1 << 40),
            step,
        },
        _ => Exactness::Certified {
            bound: Steps(1 << 40),
            step: GridStep::Q0,
        },
    };
    Moved {
        body,
        exactness,
        accept: Some(accept),
    }
}

/// A unit cube scaled by 7/3 along y, which rounds its far corners from 7/3 to 2: a body the
/// checker passes, recorded Certified.
fn a_rounded_box() -> OpOutput<Made> {
    let c = ctx();
    let cube = make_box(
        &c,
        &BoxInput {
            min: p(0, 0, 0),
            size: [Steps(1); 3],
        },
    )
    .unwrap();
    let wide = scale(
        &c,
        moved(&cube.result.body, cube.exactness),
        [s(1, 1), s(7, 3), s(1, 1)],
        p(0, 0, 0),
    )
    .unwrap();
    assert!(report(&wide.result.body, wide.exactness).passed());
    assert!(matches!(wide.exactness, Exactness::Certified { .. }));
    wide
}

/// The rounded box with a parameter curve on its first fin, published again under the same
/// names, and its record.
fn a_rounded_box_with_a_pcurve() -> (Body, Exactness) {
    let wide = a_rounded_box();
    let body = &wide.result.body;
    let mut builder = Builder::edit(body);
    let line = Line::new([0.0; 2], [1.0, 0.0], (0.0, 1.0)).unwrap();
    let curve = builder.arenas.pcurves.push(CurveKind::Line(line)).unwrap();
    builder
        .arenas
        .fins
        .pcurve
        .set(Ix::new(0).unwrap(), Some(curve))
        .unwrap();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let body = builder
        .publish_named(
            body.names().unwrap(),
            body.kind(),
            body.outside(),
            &mut meter,
        )
        .unwrap();
    (body, wide.exactness)
}

/// Asserts `e` is the typed refusal that waits for `piece`, unsupported and with that remedy.
fn waits(what: &str, e: &OpError, piece: &'static str) {
    assert_eq!(
        *e.detail(),
        OpErrorDetail::Make(MakeDetail::NotCertifiedYet { piece }),
        "{what}: {}",
        e.message()
    );
    assert_eq!(e.kind(), ErrorKind::Unsupported, "{what}");
    assert_eq!(e.remedies(), [Remedy::AwaitPiece { piece }], "{what}");
}

/// The checker finds nothing wrong with the moved body and cannot certify its parameter curve, so
/// a move, a mirror and a quarter turn, each exact, wait for G417 rather than report a defect.
#[test]
fn an_exact_transform_the_checker_cannot_certify_waits_when_the_record_was_not_exact() {
    let (body, record) = a_rounded_box_with_a_pcurve();
    let found = report(&body, record);
    assert!(found.uncertified(), "{:?}", found.faults());
    assert!(
        found
            .faults()
            .iter()
            .all(|f| f.kind == FaultKind::Unchecked),
        "{:?}",
        found.faults()
    );
    let c = ctx();
    let input = moved(&body, record);
    waits("move", &move_by(&c, input, [1, 0, 0]).unwrap_err(), "G417");
    waits(
        "mirror",
        &mirror(&c, input, Axis::Y, p(0, 0, 0)).unwrap_err(),
        "G417",
    );
    let turn = Rotation::quarter_turns(1);
    waits(
        "quarter turn",
        &rotate(&c, input, Axis::X, turn, p(0, 0, 0)).unwrap_err(),
        "G417",
    );
}
