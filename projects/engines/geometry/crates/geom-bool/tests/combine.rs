//! Bodies in, bodies out: the boolean's entry against closed forms and the checker (G130, G131).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic, and a failure stops the test"
)]

use geom_bool::combine::{COMBINE, combine, intersect, subtract, unite};
use geom_bool::operand::{BooleanInput, Operand, OperandKey, OperandKind, Selection, Term};
use geom_bool::rebuild::Merge;
use geom_check::{Level, check};
use geom_make::{BoxInput, Made, cylinder, make_box, prism};
use geom_num::{Exactness, P3, Steps};
use geom_op::{CombineDetail, ErrorKind, OpContext, OpErrorDetail, OpId, OpPath};
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::Body;
use geom_topo::query::volume;
use proptest::prelude::*;

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn cuboid(low: [i64; 3], size: [u64; 3]) -> Body {
    let input = BoxInput {
        min: p(low[0], low[1], low[2]),
        size: size.map(Steps),
    };
    let made: Made = make_box(&ctx(), &input).unwrap().result;
    made.body
}

fn key(n: u8) -> OperandKey {
    OperandKey([n; 16])
}

fn solid(n: u8, body: &Body) -> Operand<'_> {
    Operand {
        key: key(n),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

/// Six times `body`'s volume.
fn sixfold(body: &Body) -> i128 {
    volume(body, &mut meter()).unwrap().to_i128().unwrap()
}

fn checked(body: &Body) {
    let report = check(body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(report.passed(), "{report:?}");
}

const SERIAL: Exec<'static> = Exec::serial();

#[test]
fn the_schema_says_what_the_entry_does() {
    assert_eq!(COMBINE.name, "combine.boolean");
    assert_eq!(COMBINE.inputs.len(), 3);
    assert_eq!(COMBINE.outputs.len(), 1);
    assert!(
        COMBINE
            .errors
            .iter()
            .any(|e| e.detail == "DuplicateOperand")
    );
}

#[test]
fn two_boxes_unite_cut_and_meet() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([20, 20, 20], [40, 40, 40]);
    let both = [solid(1, &a), solid(2, &b)];
    let u = unite(&ctx(), &SERIAL, &both).unwrap();
    assert_eq!(u.exactness, Exactness::Exact);
    assert_eq!(sixfold(&u.result), 6 * (2 * 64_000 - 8_000));
    checked(&u.result);
    let s = subtract(&ctx(), &SERIAL, key(1), &both).unwrap();
    assert_eq!(sixfold(&s.result), 6 * (64_000 - 8_000));
    checked(&s.result);
    let i = intersect(&ctx(), &SERIAL, &both).unwrap();
    assert_eq!(sixfold(&i.result), 6 * 8_000);
    checked(&i.result);
}

#[test]
fn one_arrangement_gives_every_selection() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([20, 20, 20], [40, 40, 40]);
    let input = BooleanInput {
        operands: vec![solid(1, &a), solid(2, &b)],
        select: vec![
            Selection::Unite,
            Selection::Intersect,
            Selection::Subtract { target: key(2) },
            Selection::Where(vec![
                Term::Inside(key(1)),
                Term::Inside(key(2)),
                Term::Not,
                Term::And,
            ]),
        ],
        merge: Merge::RemoveRedundant,
    };
    let out = combine(&ctx(), &SERIAL, &input).unwrap();
    let volumes: Vec<i128> = out.result.iter().map(sixfold).collect();
    assert_eq!(volumes, [6 * 120_000, 6 * 8_000, 6 * 56_000, 6 * 56_000]);
}

#[test]
fn a_door_through_a_wall_leaves_an_opening() {
    let wall = cuboid([0, 0, 0], [100, 10, 60]);
    let door = cuboid([40, -5, 0], [20, 20, 40]);
    let out = subtract(&ctx(), &SERIAL, key(1), &[solid(1, &wall), solid(2, &door)]).unwrap();
    assert_eq!(sixfold(&out.result), 6 * (100 * 10 * 60 - 20 * 10 * 40));
    checked(&out.result);
}

#[test]
fn many_tools_cut_in_one_call() {
    let wall = cuboid([0, 0, 0], [300, 10, 60]);
    let tools: Vec<Body> = (0..6)
        .map(|i| cuboid([20 + 45 * i, -5, 10], [20, 20, 30]))
        .collect();
    let mut operands = vec![solid(0, &wall)];
    for (i, tool) in tools.iter().enumerate() {
        operands.push(solid(i as u8 + 1, tool));
    }
    let out = subtract(&ctx(), &SERIAL, key(0), &operands).unwrap();
    assert_eq!(sixfold(&out.result), 6 * (300 * 10 * 60 - 6 * 20 * 10 * 30));
    checked(&out.result);
}

#[test]
fn a_result_goes_straight_back_in() {
    let mut body = cuboid([0, 0, 0], [200, 40, 40]);
    let whole = 200 * 40 * 40;
    let mut left = whole;
    for i in 0..10 {
        let tool = cuboid([5 + 19 * i, -5, 10], [10, 50, 20]);
        let out = subtract(&ctx(), &SERIAL, key(1), &[solid(1, &body), solid(2, &tool)]).unwrap();
        left -= 10 * 40 * 20;
        assert_eq!(sixfold(&out.result), 6 * left, "cut {i}");
        checked(&out.result);
        body = out.result;
    }
}

#[test]
fn a_tilted_prism_cuts_and_comes_back() {
    let block = cuboid([0, 0, 0], [60, 60, 60]);
    let base = vec![p(10, 5, -5), p(50, 15, -5), p(30, 45, -5)];
    let input = geom_make::PrismInput {
        base,
        height: Steps(80),
    };
    let tool = prism(&ctx(), &input).unwrap().result.body;
    let out = subtract(
        &ctx(),
        &SERIAL,
        key(1),
        &[solid(1, &block), solid(2, &tool)],
    )
    .unwrap();
    // The triangle's area is 800, through all 60 of the block.
    assert_eq!(sixfold(&out.result), 6 * (216_000 - 800 * 60));
    checked(&out.result);
    let again = cuboid([20, 20, 20], [60, 60, 60]);
    let more = unite(&ctx(), &SERIAL, &[solid(1, &out.result), solid(2, &again)]).unwrap();
    checked(&more.result);
}

#[test]
fn the_order_of_the_operands_does_not_count() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([20, 20, 20], [40, 40, 40]);
    let forward = unite(&ctx(), &SERIAL, &[solid(1, &a), solid(2, &b)]).unwrap();
    let backward = unite(&ctx(), &SERIAL, &[solid(2, &b), solid(1, &a)]).unwrap();
    assert_eq!(forward.result, backward.result);
}

#[test]
fn seams_stay_and_come_back_in() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([40, 0, 0], [40, 40, 40]);
    let input = BooleanInput {
        operands: vec![solid(1, &a), solid(2, &b)],
        select: vec![Selection::Unite],
        merge: Merge::KeepSeams,
    };
    let seamed = combine(&ctx(), &SERIAL, &input).unwrap().result.remove(0);
    checked(&seamed);
    let tool = cuboid([30, 10, 10], [20, 20, 40]);
    let out = subtract(
        &ctx(),
        &SERIAL,
        key(1),
        &[solid(1, &seamed), solid(2, &tool)],
    )
    .unwrap();
    assert_eq!(sixfold(&out.result), 6 * (128_000 - 20 * 20 * 30));
    checked(&out.result);
}

#[test]
fn a_cutter_cuts_and_counts_nothing() {
    let block = cuboid([0, 0, 0], [40, 40, 40]);
    let other = cuboid([20, 20, 20], [40, 40, 40]);
    let cutter = Operand {
        kind: OperandKind::Cutter,
        ..solid(2, &other)
    };
    let out = unite(&ctx(), &SERIAL, &[solid(1, &block), cutter]).unwrap();
    assert_eq!(sixfold(&out.result), 6 * 64_000);
}

#[test]
fn refusals_name_what_is_wrong() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let detail = |e: geom_op::OpError| *e.detail();
    let twice = unite(&ctx(), &SERIAL, &[solid(1, &a), solid(1, &a)]).unwrap_err();
    assert_eq!(
        detail(twice),
        OpErrorDetail::Combine(CombineDetail::DuplicateOperand)
    );
    let missing = subtract(&ctx(), &SERIAL, key(9), &[solid(1, &a)]).unwrap_err();
    assert_eq!(
        detail(missing),
        OpErrorDetail::Combine(CombineDetail::UnknownOperand)
    );
    let input = BooleanInput {
        operands: vec![solid(1, &a)],
        select: vec![Selection::Where(vec![Term::And])],
        merge: Merge::RemoveRedundant,
    };
    let bad = combine(&ctx(), &SERIAL, &input).unwrap_err();
    assert_eq!(bad.kind(), ErrorKind::InvalidInput);
    let outside = BooleanInput {
        operands: vec![solid(1, &a)],
        select: vec![Selection::Where(vec![Term::Inside(key(1)), Term::Not])],
        merge: Merge::RemoveRedundant,
    };
    let unbounded = combine(&ctx(), &SERIAL, &outside).unwrap_err();
    assert_eq!(
        detail(unbounded),
        OpErrorDetail::Combine(CombineDetail::UnboundedResult)
    );
}

#[test]
fn a_curved_operand_waits_for_the_curved_path() {
    let round = cylinder(
        &ctx(),
        &geom_make::RoundInput {
            base: p(0, 0, 0),
            radius: Steps(16),
            height: Steps(16),
            step: geom_num::GridStep::Q0,
        },
    )
    .unwrap()
    .result
    .body;
    let err = unite(&ctx(), &SERIAL, &[solid(1, &round)]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Unsupported);
}

#[test]
fn a_small_budget_stops_the_call() {
    let a = cuboid([0, 0, 0], [40, 40, 40]);
    let b = cuboid([20, 20, 20], [40, 40, 40]);
    let small = OpContext::new(
        OpPath::new(OpId::from_bytes([7; 16])),
        Budget::new(30, u64::MAX),
    );
    let got = unite(&small, &SERIAL, &[solid(1, &a), solid(2, &b)]);
    assert!(got.is_err());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn any_boxes_keep_their_volumes(
        boxes in prop::collection::vec(
            ([-6i64..6, -6i64..6, -6i64..6], [1u64..6, 1u64..6, 1u64..6]),
            2..4,
        )
    ) {
        let bodies: Vec<Body> = boxes.iter().map(|(low, size)| cuboid(*low, *size)).collect();
        let operands: Vec<Operand<'_>> = bodies
            .iter()
            .enumerate()
            .map(|(i, b)| solid(i as u8, b))
            .collect();
        let inside = |x: i64, y: i64, z: i64, k: &([i64; 3], [u64; 3])| {
            let (l, s) = k;
            x >= l[0] && x < l[0] + s[0] as i64
                && y >= l[1] && y < l[1] + s[1] as i64
                && z >= l[2] && z < l[2] + s[2] as i64
        };
        let mut count = 0i128;
        for x in -7..12 {
            for y in -7..12 {
                for z in -7..12 {
                    count += i128::from(boxes.iter().any(|k| inside(x, y, z, k)));
                }
            }
        }
        let out = unite(&ctx(), &SERIAL, &operands).unwrap();
        if out.result.kind() == geom_topo::BodyKind::Solid {
            prop_assert_eq!(sixfold(&out.result), 6 * count);
            checked(&out.result);
        }
    }
}
