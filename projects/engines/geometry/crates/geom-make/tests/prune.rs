//! `PruneHistory` on real histories (piece G109): a box moved, then mirrored, keeps each step in
//! its place and, of each step's records, only those of the names the program holds.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests: a failure stops the test"
)]

use geom_make::{BoxInput, Made, Moved, make_box, mirror, move_by};
use geom_math::Axis;
use geom_name::prune;
use geom_name::resolve::Step;
use geom_num::grid::P3;
use geom_num::{Exactness, Steps};
use geom_op::{NameId, NameSet, OpContext, OpId, OpOutput, OpPath};
use geom_run::Budget;

fn ctx(byte: u8) -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([byte; 16])), Budget::UNLIMITED)
}

fn moved(out: &OpOutput<Made>) -> Moved<'_> {
    Moved {
        body: &out.result.body,
        exactness: out.exactness,
        accept: None,
    }
}

fn faces(body: &geom_topo::Body) -> Vec<NameId> {
    let names = body.names().unwrap();
    names
        .faces
        .iter()
        .map(|k| NameId::from_bytes(k.0))
        .collect()
}

/// A box, moved, then mirrored, with each step's record.
fn built() -> (Vec<Step>, OpOutput<Made>) {
    let input = BoxInput {
        min: P3::new(0, 0, 0).unwrap(),
        size: [Steps(10), Steps(20), Steps(30)],
    };
    let first = make_box(&ctx(1), &input).unwrap();
    let second = move_by(&ctx(2), moved(&first), [5, 0, 0]).unwrap();
    let third = mirror(&ctx(3), moved(&second), Axis::Z, P3::new(0, 0, 0).unwrap()).unwrap();
    let steps = vec![
        Step::Recorded {
            by: ctx(2).path().clone(),
            history: second.result.history.clone(),
        },
        Step::Recorded {
            by: ctx(3).path().clone(),
            history: third.result.history.clone(),
        },
    ];
    (steps, third)
}

#[test]
fn a_transform_keeps_every_held_name_and_prunes_the_rest() {
    let (steps, last) = built();
    assert_eq!(last.exactness, Exactness::Exact);
    let names = faces(&last.result.body);
    let held = NameSet::new(names[..2].iter().copied()).unwrap();
    let by = ctx(9);
    let pruned = prune(&by, &steps, &held).unwrap().result;
    assert_eq!(pruned.len(), steps.len());
    for step in &pruned {
        let Step::Kept {
            held: kept,
            pruned_by,
            history,
            ..
        } = step
        else {
            panic!("{step:?}");
        };
        assert_eq!(kept, &held);
        assert_eq!(pruned_by, by.path());
        // A transform records every entity under its own name; two are held.
        let names: Vec<NameId> = history.records().iter().map(|r| r.output).collect();
        assert_eq!(names, held.names());
    }
}

#[test]
fn a_budget_stops_it_on_real_histories() {
    let (steps, last) = built();
    let held = NameSet::new(faces(&last.result.body)).unwrap();
    let small = OpContext::new(
        OpPath::new(OpId::from_bytes([9; 16])),
        Budget::new(3, u64::MAX),
    );
    assert!(prune(&small, &steps, &held).is_err());
}
