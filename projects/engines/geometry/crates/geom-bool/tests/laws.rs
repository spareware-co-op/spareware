//! The flat boolean's laws over random grid boxes, against an exact count of unit cells: the
//! volume of every selection, inclusion and exclusion, A less A, and commutation with the exact
//! transforms, translation by a grid vector and a quarter turn (G140; 11.17's polyhedral
//! verification corpus, the exact-volume pattern of csgrs' hypermesh). Every result passes the
//! full checker and the second checker of 28.4, which agree (Gate 4, G156).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and indexing, and a failure stops the test"
)]

use geom_bool::combine::{Combined, intersect, subtract, unite};
use geom_bool::operand::{Operand, OperandKey, OperandKind};
use geom_check::{Level, check};
use geom_make::{BoxInput, Made, make_box};
use geom_num::{Exactness, P3, Steps};
use geom_op::{OpContext, OpError, OpId, OpOutput, OpPath};
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::agree::judge;
use geom_topo::query::volume;
use geom_topo::{Body, BodyKind, PointKind};
use proptest::prelude::*;

const SERIAL: Exec<'static> = Exec::serial();

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A box from `low`, of `size`.
type Block = ([i64; 3], [u64; 3]);

fn cuboid((low, size): Block) -> Body {
    let input = BoxInput {
        min: P3::new(low[0], low[1], low[2]).unwrap(),
        size: size.map(Steps),
    };
    let made: Made = make_box(&ctx(), &input).unwrap().result;
    made.body
}

fn solid(n: usize, body: &Body) -> Operand<'_> {
    Operand {
        key: OperandKey([n as u8 + 1; 16]),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

/// Whether the unit cell at `cell` lies in `block`.
fn holds((low, size): &Block, cell: [i64; 3]) -> bool {
    (0..3).all(|k| cell[k] >= low[k] && cell[k] < low[k] + size[k] as i64)
}

/// The unit cells of the boxes' span that `keep` keeps, given which boxes hold each.
fn count(blocks: &[Block], keep: impl Fn(&[bool]) -> bool) -> i128 {
    let (mut low, mut high) = ([i64::MAX; 3], [i64::MIN; 3]);
    for (l, s) in blocks {
        for k in 0..3 {
            low[k] = low[k].min(l[k]);
            high[k] = high[k].max(l[k] + s[k] as i64);
        }
    }
    let mut n = 0;
    for x in low[0]..high[0] {
        for y in low[1]..high[1] {
            for z in low[2]..high[2] {
                let held: Vec<bool> = blocks.iter().map(|b| holds(b, [x, y, z])).collect();
                n += i128::from(keep(&held));
            }
        }
    }
    n
}

/// Six times the volume of a valid result, or `None` for a general body with faces, whose
/// volume the topology does not give: those are held to the checker alone.
fn sixfold(out: &OpOutput<Combined>) -> Option<i128> {
    let body = &out.result.body;
    let report = check(body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(report.passed(), "{report:?}");
    // The second checker of 28.4 reads every result too, and agrees (Gate 4, G156).
    let both = judge(body, Exactness::Exact).unwrap();
    assert!(both.agree() && both.unjudged == 0, "{both:?}");
    match body.kind() {
        BodyKind::Solid => Some(volume(body, &mut meter()).unwrap().to_i128().unwrap()),
        _ if body.arenas().faces.is_empty() => Some(0),
        _ => None,
    }
}

/// The grid corners of a result, sorted, moved back by `by`.
fn corners(out: &OpOutput<Combined>, by: [i64; 3]) -> Vec<[i64; 3]> {
    let a = out.result.body.arenas();
    let mut at = Vec::new();
    for row in 0..a.points.len() {
        if let Some(PointKind::Grid(g)) = a.points.get(geom_topo::Ix::new(row).unwrap()) {
            at.push([g.x.get() - by[0], g.y.get() - by[1], g.z.get() - by[2]]);
        }
    }
    at.sort_unstable();
    at
}

fn any_block() -> impl Strategy<Value = Block> {
    ([-6i64..6, -6i64..6, -6i64..6], [1u64..7, 1u64..7, 1u64..7])
}

/// `block` turned a quarter about the z axis: (x, y, z) to (-y, x, z).
fn turned((low, size): &Block) -> Block {
    (
        [-(low[1] + size[1] as i64), low[0], low[2]],
        [size[1], size[0], size[2]],
    )
}

/// The three selections of `blocks`, the first the target, moved by `by`.
fn selections(blocks: &[Block], by: [i64; 3]) -> [Result<OpOutput<Combined>, OpError>; 3] {
    let moved: Vec<Block> = blocks
        .iter()
        .map(|(l, s)| ([l[0] + by[0], l[1] + by[1], l[2] + by[2]], *s))
        .collect();
    selected(&moved)
}

/// The three selections of `blocks`, the first the target.
fn selected(blocks: &[Block]) -> [Result<OpOutput<Combined>, OpError>; 3] {
    let moved: Vec<Body> = blocks.iter().map(|b| cuboid(*b)).collect();
    let operands: Vec<Operand<'_>> = moved.iter().enumerate().map(|(i, b)| solid(i, b)).collect();
    let target = operands[0].key;
    [
        unite(&ctx(), &SERIAL, &operands),
        intersect(&ctx(), &SERIAL, &operands),
        subtract(&ctx(), &SERIAL, target, &operands),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn every_selection_holds_the_cells_it_keeps_and_moves_with_its_operands(
        blocks in prop::collection::vec(any_block(), 2..6),
        by in [-1_000_000_000_000i64..1_000_000_000_000, -40i64..40, -40i64..40],
    ) {
        let wants = [
            count(&blocks, |held| held.iter().any(|h| *h)),
            count(&blocks, |held| held.iter().all(|h| *h)),
            count(&blocks, |held| held[0] && !held[1..].iter().any(|h| *h)),
        ];
        let here = selections(&blocks, [0; 3]);
        let there = selections(&blocks, by);
        for ((got, moved), want) in here.iter().zip(&there).zip(wants) {
            let (got, moved) = (got.as_ref().unwrap(), moved.as_ref().unwrap());
            if let Some(six) = sixfold(got) {
                prop_assert_eq!(six, 6 * want);
            }
            // Moved by a grid vector, the result is the same shape moved, and as valid.
            prop_assert_eq!(sixfold(moved), sixfold(got));
            prop_assert_eq!(corners(moved, by), corners(got, [0; 3]));
        }
    }

    #[test]
    fn every_selection_turns_with_its_operands(blocks in prop::collection::vec(any_block(), 2..5)) {
        // Turned a quarter about z, each selection is the same shape turned, and as valid.
        let here = selected(&blocks);
        let there = selected(&blocks.iter().map(turned).collect::<Vec<_>>());
        for (got, turn) in here.iter().zip(&there) {
            let (got, turn) = (got.as_ref().unwrap(), turn.as_ref().unwrap());
            prop_assert_eq!(sixfold(turn), sixfold(got));
            let mut back: Vec<[i64; 3]> = corners(turn, [0; 3])
                .into_iter()
                .map(|[x, y, z]| [y, -x, z])
                .collect();
            back.sort_unstable();
            prop_assert_eq!(back, corners(got, [0; 3]));
        }
    }

    #[test]
    fn a_union_and_an_intersection_sum_to_their_operands(a in any_block(), b in any_block()) {
        let [union, meet, _] = selections(&[a, b], [0; 3]);
        let one = |block: Block| {
            let body = cuboid(block);
            let size = block.1;
            let six = volume(&body, &mut meter()).unwrap().to_i128().unwrap();
            prop_assert_eq!(six, 6 * i128::from(size[0] * size[1] * size[2]));
            Ok(six)
        };
        if let (Some(u), Some(m)) = (sixfold(&union.unwrap()), sixfold(&meet.unwrap())) {
            prop_assert_eq!(u + m, one(a)? + one(b)?);
        }
    }

    #[test]
    fn a_body_less_itself_is_nothing(blocks in prop::collection::vec(any_block(), 1..4)) {
        let bodies: Vec<Body> = blocks.iter().map(|b| cuboid(*b)).collect();
        let union = unite(
            &ctx(),
            &SERIAL,
            &bodies.iter().enumerate().map(|(i, b)| solid(i, b)).collect::<Vec<_>>(),
        )
        .unwrap()
        .result
        .body;
        let less = subtract(
            &ctx(),
            &SERIAL,
            solid(0, &union).key,
            &[solid(0, &union), solid(1, &union)],
        )
        .unwrap();
        prop_assert_eq!(less.result.body.arenas().faces.len(), 0);
        prop_assert_eq!(sixfold(&less), Some(0));
    }
}
