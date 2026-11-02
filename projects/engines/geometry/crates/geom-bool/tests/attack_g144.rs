//! Piece G144's attack tests: the arrangement step of local operations (`geom_bool::local`). The
//! fast and full paths give one body; the fast path refuses what its proof does not cover; each
//! class of 13.14's taxonomy is found and recorded; and a change outside the call's set is
//! refused, naming its class and entities.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    clippy::too_many_lines,
    reason = "tests: native arithmetic, and a failure stops the test"
)]

use geom_bool::combine::intersect;
use geom_bool::half_space::half_space;
use geom_bool::local::{Local, LocalInput, Path, Plan, local_step};
use geom_bool::operand::{Operand, OperandKey, OperandKind, Selection};
use geom_check::{Level, check};
use geom_make::{BoxInput, make_box};
use geom_num::{Exactness, P3, Steps};
use geom_op::history::{ChangeClass, LoopChange, TopologyChange};
use geom_op::local::{AllowSet, LocalDetail};
use geom_op::{ErrorKind, OpContext, OpErrorDetail, OpId, OpPath};
use geom_poly::merge::Merge;
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::Body;
use geom_topo::query::volume;

const SERIAL: Exec<'static> = Exec::serial();

fn ctx(n: u8) -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([n; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// A box made by its own operation, so two boxes bring different names.
fn cuboid(n: u8, low: [i64; 3], size: [u64; 3]) -> Body {
    let input = BoxInput {
        min: p(low[0], low[1], low[2]),
        size: size.map(Steps),
    };
    make_box(&ctx(n), &input).unwrap().result.body
}

fn solid(n: u8, body: &Body) -> Operand<'_> {
    Operand {
        key: OperandKey([n; 16]),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

fn step<'b>(
    target: &'b Body,
    tools: &[&'b Body],
    select: Selection,
    plan: Plan,
    may: AllowSet,
) -> Result<Local, geom_op::OpError> {
    let input = LocalInput {
        target: solid(1, target),
        tools: tools
            .iter()
            .enumerate()
            .map(|(i, b)| solid(u8::try_from(i).unwrap() + 2, b))
            .collect(),
        select,
        merge: Merge::RemoveRedundant,
        accept: None,
        may,
        plan,
    };
    local_step(&ctx(9), &SERIAL, &input).map(|out| out.result)
}

fn subtract() -> Selection {
    Selection::Subtract {
        target: OperandKey([1; 16]),
    }
}

fn classes(local: &Local) -> Vec<ChangeClass> {
    local
        .history
        .changes()
        .iter()
        .map(TopologyChange::class)
        .collect()
}

fn checked(body: &Body) {
    let report = check(body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(report.passed(), "{report:?}");
}

fn sixfold(body: &Body) -> i128 {
    volume(body, &mut meter()).unwrap().to_i128().unwrap()
}

#[test]
fn the_fast_and_full_paths_give_one_body() {
    let target = cuboid(1, [0, 0, 0], [10, 20, 30]);
    // An extrusion of the whole top face, a box beside the target on a whole face, and one apart.
    let on_top = cuboid(2, [0, 0, 30], [10, 20, 10]);
    let beside = cuboid(3, [10, 0, 0], [5, 20, 30]);
    let apart = cuboid(4, [40, 0, 0], [3, 3, 3]);
    // A box whose top lies on the target's top plane, facing the same way, apart from it.
    let level = cuboid(5, [20, 0, 0], [5, 5, 30]);
    for (what, tool, select) in [
        ("a box level with it", &level, Selection::Unite),
        ("an extrusion", &on_top, Selection::Unite),
        ("a box beside", &beside, Selection::Unite),
        ("a box apart", &apart, Selection::Unite),
        ("a cut that only touches", &on_top, subtract()),
    ] {
        let full = step(
            &target,
            &[tool],
            select.clone(),
            Plan::Arrange,
            AllowSet::ALL,
        )
        .unwrap();
        let fast = step(&target, &[tool], select, Plan::Glued, AllowSet::ALL).unwrap();
        assert_eq!(
            (full.path, fast.path),
            (Path::Arranged, Path::Fast),
            "{what}"
        );
        assert_eq!(full.body, fast.body, "{what}");
        assert_eq!(full.history, fast.history, "{what}");
        checked(&fast.body);
    }
    let extruded = step(
        &target,
        &[&on_top],
        Selection::Unite,
        Plan::Glued,
        AllowSet::ALL,
    )
    .unwrap();
    assert_eq!(sixfold(&extruded.body), 6 * 10 * 20 * 40);
    assert_eq!(extruded.body.arenas().faces.len(), 6);
}

#[test]
fn the_fast_path_refuses_what_its_proof_does_not_cover() {
    let target = cuboid(1, [0, 0, 0], [10, 20, 30]);
    // A boss on part of the top face: the shared face is not a whole face of both.
    let boss = cuboid(2, [2, 2, 30], [4, 4, 5]);
    let e = step(
        &target,
        &[&boss],
        Selection::Unite,
        Plan::Glued,
        AllowSet::ALL,
    )
    .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidInput);
    // Arranged, the same boss is an ordinary union.
    let arranged = step(
        &target,
        &[&boss],
        Selection::Unite,
        Plan::Arrange,
        AllowSet::ALL,
    );
    assert!(arranged.is_ok());
}

#[test]
fn an_extrusion_adds_its_far_face_and_merges_its_sides() {
    let target = cuboid(1, [0, 0, 0], [10, 20, 30]);
    let on_top = cuboid(2, [0, 0, 30], [10, 20, 10]);
    let done = step(
        &target,
        &[&on_top],
        Selection::Unite,
        Plan::Glued,
        AllowSet::ALL,
    )
    .unwrap();
    let found = classes(&done);
    assert!(found.contains(&ChangeClass::FaceAdded), "{found:?}");
    assert_eq!(
        found
            .iter()
            .filter(|c| **c == ChangeClass::FacesMerged)
            .count(),
        4,
        "{found:?}"
    );
    // The shared face is replaced, no change of its own.
    assert!(!found.contains(&ChangeClass::FaceVanished), "{found:?}");
    assert!(!found.contains(&ChangeClass::OvershootRemoved), "{found:?}");
}

#[test]
fn a_change_outside_the_set_is_refused_with_its_class_and_entities() {
    let target = cuboid(1, [0, 0, 0], [10, 20, 30]);
    let on_top = cuboid(2, [0, 0, 30], [10, 20, 10]);
    let only_added = AllowSet::of(&[ChangeClass::FaceAdded]);
    let e = step(
        &target,
        &[&on_top],
        Selection::Unite,
        Plan::Glued,
        only_added,
    )
    .unwrap_err();
    assert_eq!(
        e.detail(),
        &OpErrorDetail::Local(LocalDetail::TopologyChangeNotAllowed {
            class: ChangeClass::FacesMerged
        })
    );
    assert_eq!(e.kind(), ErrorKind::NoSolution);
    assert_eq!(
        e.entities().len(),
        3,
        "two faces merged into one: {:?}",
        e.entities()
    );
    let none = AllowSet::ALL.without(ChangeClass::FaceAdded);
    let e = step(&target, &[&on_top], Selection::Unite, Plan::Arrange, none).unwrap_err();
    assert!(matches!(
        e.detail(),
        OpErrorDetail::Local(LocalDetail::TopologyChangeNotAllowed { .. })
    ));
}

#[test]
fn a_groove_splits_the_face_it_crosses_and_trims_it() {
    let target = cuboid(1, [0, 0, 0], [10, 20, 30]);
    let groove = cuboid(2, [-1, 8, 25], [12, 4, 10]);
    let done = step(
        &target,
        &[&groove],
        subtract(),
        Plan::Arrange,
        AllowSet::ALL,
    )
    .unwrap();
    checked(&done.body);
    let changes = done.history.changes();
    assert!(
        changes.iter().any(|c| matches!(
            c,
            TopologyChange::LoopChanged {
                change: LoopChange::FaceSplit,
                ..
            }
        )),
        "{changes:?}"
    );
    assert!(
        classes(&done).contains(&ChangeClass::ClashTrimmed),
        "{changes:?}"
    );
}

#[test]
fn a_slab_through_the_middle_splits_the_body() {
    let target = cuboid(1, [0, 0, 0], [10, 20, 30]);
    let slab = cuboid(2, [-1, -1, 10], [12, 22, 10]);
    let done = step(&target, &[&slab], subtract(), Plan::Arrange, AllowSet::ALL).unwrap();
    checked(&done.body);
    let found = classes(&done);
    assert!(found.contains(&ChangeClass::BodySplit), "{found:?}");
    assert!(found.contains(&ChangeClass::FaceAdded), "{found:?}");
}

#[test]
fn a_face_swallowed_by_a_tool_is_an_overshoot_and_its_neighbours_are_trimmed() {
    let target = cuboid(1, [0, 0, 0], [10, 20, 30]);
    let cap = cuboid(2, [-1, -1, 25], [12, 22, 15]);
    let done = step(
        &target,
        &[&cap],
        Selection::Unite,
        Plan::Arrange,
        AllowSet::ALL,
    )
    .unwrap();
    checked(&done.body);
    let changes = done.history.changes();
    let removed = changes.iter().find_map(|c| match c {
        TopologyChange::OvershootRemoved { faces } => Some(faces.names().len()),
        _ => None,
    });
    assert_eq!(removed, Some(1), "the target's top face: {changes:?}");
    assert!(
        classes(&done).contains(&ChangeClass::ClashTrimmed),
        "{changes:?}"
    );
}

#[test]
fn a_face_left_outside_the_selection_vanishes() {
    let target = cuboid(1, [0, 0, 0], [10, 20, 30]);
    let lower = cuboid(2, [-1, -1, -1], [12, 22, 16]);
    let done = step(
        &target,
        &[&lower],
        Selection::Intersect,
        Plan::Arrange,
        AllowSet::ALL,
    )
    .unwrap();
    checked(&done.body);
    let changes = done.history.changes();
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, TopologyChange::FaceVanished { .. })),
        "the target's top face: {changes:?}"
    );
}

#[test]
fn a_plug_closes_a_hole() {
    let block = cuboid(1, [0, 0, 0], [10, 20, 30]);
    let pillar = cuboid(5, [3, 3, -1], [4, 4, 32]);
    let holed = {
        let input = geom_bool::operand::BooleanInput {
            operands: vec![solid(1, &block), solid(2, &pillar)],
            select: vec![subtract()],
            merge: Merge::RemoveRedundant,
            accept: None,
        };
        geom_bool::combine::combine(&ctx(6), &SERIAL, &input)
            .unwrap()
            .result
            .remove(0)
            .remove(0)
            .body
    };
    let plug = cuboid(7, [3, 3, 0], [4, 4, 30]);
    let done = step(
        &holed,
        &[&plug],
        Selection::Unite,
        Plan::Arrange,
        AllowSet::ALL,
    )
    .unwrap();
    checked(&done.body);
    let changes = done.history.changes();
    assert!(
        changes.iter().any(|c| matches!(
            c,
            TopologyChange::LoopChanged {
                change: LoopChange::HoleClosed,
                ..
            }
        )),
        "{changes:?}"
    );
    assert_eq!(sixfold(&done.body), 6 * 10 * 20 * 30);
}

/// A square pyramid on [0, 20]² with its apex at (10, 10, 20): a box cut by four half-spaces.
fn pyramid() -> Body {
    let block = cuboid(1, [0, 0, 0], [20, 20, 20]);
    let apex = p(10, 10, 20);
    let sides = [
        [p(0, 0, 0), p(20, 0, 0), apex],
        [p(20, 0, 0), p(20, 20, 0), apex],
        [p(20, 20, 0), p(0, 20, 0), apex],
        [p(0, 20, 0), p(0, 0, 0), apex],
    ];
    let mut body = block;
    for (i, points) in sides.into_iter().enumerate() {
        let n = u8::try_from(i).unwrap() + 20;
        let half = half_space(
            &OpPath::new(OpId::from_bytes([n; 16])),
            points,
            &mut meter(),
        )
        .unwrap();
        let out = intersect(&ctx(n + 10), &SERIAL, &[solid(1, &body), solid(2, &half)]).unwrap();
        body = out.result.body;
    }
    assert_eq!(
        sixfold(&body),
        2 * 20 * 20 * 20,
        "six times a third of 20 by 20 by 20"
    );
    body
}

#[test]
fn cutting_off_an_apex_of_four_faces_splits_the_vertex() {
    let target = pyramid();
    let tip = cuboid(2, [8, 8, 18], [4, 4, 7]);
    let done = step(&target, &[&tip], subtract(), Plan::Arrange, AllowSet::ALL).unwrap();
    checked(&done.body);
    let changes = done.history.changes();
    let split = changes.iter().find_map(|c| match c {
        TopologyChange::VertexSplit { edges, .. } => Some(edges.names().len()),
        _ => None,
    });
    assert_eq!(split, Some(4), "the cut's four edges: {changes:?}");
}

#[test]
fn every_class_is_found_by_some_test_here() {
    // The classes each test above asserts, against 13.14's list; a self-overlap cut needs a
    // target crossing itself, which the checker refuses as an operand, so it is classified from
    // the history's split records and has no body to make it here (the PR names the gap).
    let asserted = [
        ChangeClass::VertexSplit,
        ChangeClass::FaceVanished,
        ChangeClass::ClashTrimmed,
        ChangeClass::OvershootRemoved,
        ChangeClass::FacesMerged,
        ChangeClass::FaceAdded,
        ChangeClass::LoopChanged,
        ChangeClass::BodySplit,
    ];
    let all: Vec<ChangeClass> = geom_op::local::CLASSES
        .into_iter()
        .filter(|c| *c != ChangeClass::SelfOverlapCut)
        .collect();
    assert_eq!(all, asserted);
}
