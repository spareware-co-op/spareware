//! Names and history of booleans: the table of 11.9, chapter 20.3's laws, and names that hold still
//! (G132).

#![allow(
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: a failure stops the test"
)]

use std::collections::BTreeSet;

use geom_bool::combine::{Combined, combine, subtract, unite};
use geom_bool::operand::{BooleanInput, Operand, OperandKey, OperandKind, Selection};
use geom_check::{Level, check};
use geom_make::{BoxInput, make_box};
use geom_num::{Exactness, P3, Steps};
use geom_op::{
    History, NameId, NameSet, OpContext, OpId, OpOutput, OpPath, Origin, PieceKey, Record, RoleArg,
    TopoName,
};
use geom_poly::merge::Merge;
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::Body;

const SERIAL: Exec<'static> = Exec::serial();

fn ctx_of(n: u8) -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([n; 16])), Budget::UNLIMITED)
}

fn ctx() -> OpContext<'static> {
    ctx_of(7)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A box made by operation `op`, so boxes of one operation share their names.
fn cuboid(op: u8, low: [i64; 3], size: [u64; 3]) -> Body {
    let input = BoxInput {
        min: P3::new(low[0], low[1], low[2]).unwrap(),
        size: size.map(Steps),
    };
    make_box(&ctx_of(op), &input).unwrap().result.body
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

/// The names of `body`'s faces, sorted.
fn faces(body: &Body) -> Vec<NameId> {
    let names = body.names().unwrap();
    let mut list: Vec<NameId> = names
        .faces
        .iter()
        .map(|k| NameId::from_bytes(k.0))
        .collect();
    list.sort_unstable();
    list
}

/// Every name `body` holds.
fn all_names(body: &Body) -> BTreeSet<NameId> {
    let n = body.names().unwrap();
    [&n.regions, &n.faces, &n.edges, &n.vertices]
        .iter()
        .flat_map(|list| list.iter().map(|k| NameId::from_bytes(k.0)))
        .collect()
}

/// The history of `result`, holding the laws of 20.3 over `operands`.
fn lawful<'c>(result: &'c Combined, operands: &[&Body]) -> &'c History {
    let history = result.history.as_ref().unwrap();
    let inputs: BTreeSet<NameId> = operands.iter().flat_map(|b| all_names(b)).collect();
    history
        .check(&NameSet::new(inputs.iter().copied()).unwrap())
        .unwrap();
    // Every output has a record, and every input a descendant or a place on the deleted list.
    let outputs = all_names(&result.body);
    let recorded: BTreeSet<NameId> = history.records().iter().map(|r| r.output).collect();
    assert_eq!(outputs, recorded);
    let deleted: BTreeSet<NameId> = history.deleted().names().iter().copied().collect();
    let carried: BTreeSet<NameId> = history
        .records()
        .iter()
        .flat_map(|r| match &r.origin {
            Origin::Unchanged => vec![r.output],
            Origin::Modified { from } | Origin::Merged { of: from } => from.names().to_vec(),
            Origin::Split { of, .. } => vec![*of],
            Origin::Generated { .. } => Vec::new(),
        })
        .collect();
    for input in &inputs {
        assert!(
            deleted.contains(input) || carried.contains(input),
            "input {input:?} has no descendant and is not deleted"
        );
    }
    let report = check(&result.body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(report.passed(), "{report:?}");
    history
}

fn origin_of<'h>(history: &'h History, name: NameId) -> &'h Origin {
    &history
        .records()
        .iter()
        .find(|r: &&Record| r.output == name)
        .unwrap()
        .origin
}

#[test]
fn a_window_leaves_untouched_faces_unchanged_and_names_its_reveals_by_the_tool() {
    let wall = cuboid(1, [0, 0, 0], [100, 10, 60]);
    let window = cuboid(2, [40, -5, 20], [20, 20, 20]);
    let out = subtract(
        &ctx(),
        &SERIAL,
        key(1),
        &[solid(1, &wall), solid(2, &window)],
    )
    .unwrap();
    let result = out.result;
    let history = lawful(&result, &[&wall, &window]);
    let kept = faces(&wall);
    let names = faces(&result.body);
    assert_eq!(names.len(), 10);
    let mut unchanged = 0;
    let mut modified = 0;
    for name in &kept {
        assert!(names.contains(name), "a wall face lost its name");
        match origin_of(history, *name) {
            Origin::Unchanged => unchanged += 1,
            Origin::Modified { .. } => modified += 1,
            other => panic!("{other:?}"),
        }
    }
    // The two faces the window goes through changed; the other four did not.
    assert_eq!((unchanged, modified), (4, 2));
    let tool_faces = faces(&window);
    let generated: Vec<&Record> = history
        .records()
        .iter()
        .filter(|r| names.contains(&r.output))
        .filter(|r| matches!(&r.origin, Origin::Generated { .. }))
        .collect();
    assert_eq!(generated.len(), 4, "the four reveals");
    for record in generated {
        let Origin::Generated { from, role } = &record.origin else {
            panic!()
        };
        assert_eq!(role.name(), "tool");
        assert_eq!(role.args(), [RoleArg::Name(NameId::from_bytes(key(2).0))]);
        assert!(tool_faces.contains(&from.names()[0]));
    }
    for name in &tool_faces {
        assert!(
            history.deleted().names().contains(name),
            "the tool's face is gone"
        );
    }
}

#[test]
fn two_doors_of_one_type_differ_and_a_third_renames_nothing() {
    let wall = cuboid(1, [0, 0, 0], [300, 10, 60]);
    let door = |x: i64| cuboid(2, [x, -5, 0], [20, 20, 40]);
    let (one, two, three) = (door(20), door(120), door(220));
    let pair = [solid(1, &wall), solid(2, &one), solid(3, &two)];
    let less = subtract(&ctx(), &SERIAL, key(1), &pair).unwrap().result;
    let names = faces(&less.body);
    assert_eq!(
        names.len(),
        names.iter().copied().collect::<BTreeSet<_>>().len(),
        "reveals of one door type collide"
    );
    let trio = [
        solid(1, &wall),
        solid(2, &one),
        solid(3, &two),
        solid(4, &three),
    ];
    let more = subtract(&ctx(), &SERIAL, key(1), &trio).unwrap().result;
    let after = faces(&more.body);
    let stayed = names.iter().filter(|n| after.contains(n)).count();
    // Everything but the pieces of the two long faces and the head that the third door cuts.
    let split_again = names.iter().filter(|n| !after.contains(n));
    for name in split_again {
        assert!(
            matches!(
                origin_of(less.history.as_ref().unwrap(), *name),
                Origin::Split { .. } | Origin::Modified { .. }
            ),
            "a reveal was renamed"
        );
    }
    assert!(stayed >= names.len() - 3, "{stayed} of {}", names.len());
}

#[test]
fn a_split_face_names_its_pieces_by_the_tools_and_the_side() {
    let wall = cuboid(1, [0, 0, 0], [100, 10, 60]);
    let door = cuboid(2, [40, -5, -5], [20, 20, 70]);
    let out = subtract(&ctx(), &SERIAL, key(1), &[solid(1, &wall), solid(2, &door)]).unwrap();
    let result = out.result;
    let history = lawful(&result, &[&wall, &door]);
    let split: Vec<&Record> = history
        .records()
        .iter()
        .filter(|r| matches!(r.origin, Origin::Split { .. }))
        .collect();
    // Front, back, top and bottom each in two.
    assert_eq!(split.len(), 8);
    for record in split {
        let Origin::Split { of, piece } = &record.origin else {
            panic!()
        };
        let PieceKey::ByTools { tools, .. } = piece;
        assert!(faces(&wall).contains(of));
        assert!(!tools.names().is_empty(), "no cutting face");
        let name = TopoName::Split {
            op: ctx().path().clone(),
            of: *of,
            piece: piece.clone(),
        };
        // Two pieces of one face on one side take a weak ordinal; these have different tools.
        assert_eq!(name.id().unwrap(), record.output);
    }
}

#[test]
fn flush_faces_merge_their_names() {
    let left = cuboid(1, [0, 0, 0], [40, 40, 40]);
    let right = cuboid(2, [40, 0, 0], [40, 40, 40]);
    let out = unite(&ctx(), &SERIAL, &[solid(1, &left), solid(2, &right)]).unwrap();
    let result = out.result;
    let history = lawful(&result, &[&left, &right]);
    let merged = history
        .records()
        .iter()
        .filter(|r| matches!(r.origin, Origin::Merged { .. }))
        .count();
    assert_eq!(merged, 4);
}

#[test]
fn a_body_united_with_its_own_copy_qualifies_the_names_and_says_so() {
    let one = cuboid(1, [0, 0, 0], [40, 40, 40]);
    let two = cuboid(1, [100, 0, 0], [40, 40, 40]);
    let out = unite(&ctx(), &SERIAL, &[solid(1, &one), solid(2, &two)]).unwrap();
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.code == "bool.names_qualified")
    );
    let result = out.result;
    let history = lawful(&result, &[&one, &two]);
    let names = faces(&result.body);
    assert_eq!(names.iter().copied().collect::<BTreeSet<_>>().len(), 12);
    for name in &names {
        let Origin::Generated { role, .. } = origin_of(history, *name) else {
            panic!("a qualified face is generated")
        };
        assert_eq!(role.name(), "operand");
    }
    for name in faces(&one) {
        assert!(history.deleted().names().contains(&name));
    }
}

#[test]
fn names_do_not_care_about_the_order_of_the_operands_or_where_the_shape_is() {
    let run = |dx: i64, turned: bool| {
        let a = cuboid(1, [dx, 0, 0], [40, 40, 40]);
        let b = cuboid(2, [dx + 20, 20, 20], [40, 40, 40]);
        let (one, two) = (solid(1, &a), solid(2, &b));
        let operands = if turned { [two, one] } else { [one, two] };
        let out: OpOutput<Combined> = subtract(&ctx(), &SERIAL, key(1), &operands).unwrap();
        out.result.body.names().unwrap().clone()
    };
    let first = run(0, false);
    assert_eq!(first, run(0, true));
    assert_eq!(first, run(1000, false));
}

#[test]
fn every_selection_of_one_call_has_its_history() {
    let a = cuboid(1, [0, 0, 0], [40, 40, 40]);
    let b = cuboid(2, [20, 20, 20], [40, 40, 40]);
    let input = BooleanInput {
        operands: vec![solid(1, &a), solid(2, &b)],
        select: vec![
            Selection::Unite,
            Selection::Intersect,
            Selection::Subtract { target: key(2) },
            Selection::Split { target: key(1) },
            Selection::Imprint { target: key(1) },
        ],
        merge: Merge::RemoveRedundant,
        accept: None,
    };
    let out = combine(&ctx(), &SERIAL, &input).unwrap();
    for list in &out.result {
        for result in list {
            lawful(result, &[&a, &b]);
        }
    }
}

#[test]
fn an_operand_without_names_gives_a_body_without_history() {
    let a = cuboid(1, [0, 0, 0], [40, 40, 40]);
    let b = geom_bool::half_space::half_space(
        [
            P3::new(20, 0, 0).unwrap(),
            P3::new(20, 1, 0).unwrap(),
            P3::new(20, 0, 1).unwrap(),
        ],
        &mut meter(),
    )
    .unwrap();
    let out = unite(&ctx(), &SERIAL, &[solid(1, &a), solid(2, &b)]).unwrap();
    assert!(out.result.history.is_none());
}
