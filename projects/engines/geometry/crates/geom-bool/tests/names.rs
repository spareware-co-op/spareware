//! Names and history of booleans: the table of 11.9, chapter 20.3's laws, and names that hold still
//! (G132).

#![allow(
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: a failure stops the test"
)]

use std::collections::{BTreeMap, BTreeSet};

use geom_bool::combine::{Combined, combine, subtract, unite};
use geom_bool::half_space::half_space;
use geom_bool::names::NamingError;
use geom_bool::operand::{BooleanInput, Operand, OperandKey, OperandKind, Selection};
use geom_check::{Level, check};
use geom_make::{BoxInput, make_box};
use geom_name::OrderError;
use geom_num::{Exactness, NumError, P3, Steps};
use geom_op::{
    EdgeOrVertex, History, HistoryError, NameError, NameId, NameSet, OpContext, OpId, OpOutput,
    OpPath, Origin, PieceKey, Record, RoleArg, Side, TopoName,
};
use geom_poly::merge::Merge;
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::{Body, Ix, PointKind};

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

fn origin_of(history: &History, name: NameId) -> &Origin {
    &history
        .records()
        .iter()
        .find(|r: &&Record| r.output == name)
        .unwrap()
        .origin
}

/// Each vertex of `body`, by row: where it is and its name.
fn vertices(body: &Body) -> Vec<(PointKind, NameId)> {
    let a = body.arenas();
    let names = body.names().unwrap();
    (0..a.vertices.len())
        .map(|row| {
            let vertex = a.vertices.get(Ix::new(row).unwrap()).unwrap();
            let point = *a.points.get(vertex.point).unwrap();
            (point, NameId::from_bytes(names.vertices[row as usize].0))
        })
        .collect()
}

/// The name of `body`'s vertex at the grid point `at`, if it has one there.
fn vertex_at(body: &Body, at: [i64; 3]) -> Option<NameId> {
    let at = P3::new(at[0], at[1], at[2]).unwrap();
    vertices(body)
        .into_iter()
        .find(|(point, _)| *point == PointKind::Grid(at))
        .map(|(_, name)| name)
}

/// The names of `body`'s vertices that lie where three planes meet, off the grid.
fn meetings(body: &Body) -> Vec<NameId> {
    vertices(body)
        .into_iter()
        .filter(|(point, _)| matches!(point, PointKind::Meet(_)))
        .map(|(_, name)| name)
        .collect()
}

/// Each edge of `body`, by row: its two ends' grid points, the lower first, and its name.
fn edges(body: &Body) -> Vec<([[i64; 3]; 2], NameId)> {
    let a = body.arenas();
    let names = body.names().unwrap();
    let grid = |v| {
        let vertex = a.vertices.get(v).unwrap();
        let p = a.points.get(vertex.point).unwrap().grid().unwrap();
        [p.x.get(), p.y.get(), p.z.get()]
    };
    (0..a.edges.len())
        .map(|row| {
            let edge = a.edges.get(Ix::new(row).unwrap()).unwrap();
            let (s, e) = (grid(edge.start.unwrap()), grid(edge.end.unwrap()));
            let name = NameId::from_bytes(names.edges[row as usize].0);
            ([s.min(e), s.max(e)], name)
        })
        .collect()
}

/// The name 11.9 gives each edge of `body` that is new: `Bounded` by the faces around it, and
/// ended by the other faces at its two ends.
fn bounded_edges(body: &Body) -> Vec<NameId> {
    let a = body.arenas();
    let names = body.names().unwrap();
    let face_name = |row: u32| NameId::from_bytes(names.faces[row as usize].0);
    let mut around: BTreeMap<u32, BTreeSet<NameId>> = BTreeMap::new();
    let mut touching: BTreeMap<u32, BTreeSet<NameId>> = BTreeMap::new();
    for row in 0..a.fins.len() {
        let fin = a.fins.get(Ix::new(row).unwrap()).unwrap();
        let face = face_name(a.loops.get(fin.loop_).unwrap().face.row());
        around.entry(fin.edge.row()).or_default().insert(face);
        let edge = a.edges.get(fin.edge).unwrap();
        for end in [edge.start.unwrap(), edge.end.unwrap()] {
            touching.entry(end.row()).or_default().insert(face);
        }
    }
    (0..a.edges.len())
        .map(|row| {
            let edge = a.edges.get(Ix::new(row).unwrap()).unwrap();
            let by = &around[&row];
            let ends: BTreeSet<NameId> = [edge.start.unwrap(), edge.end.unwrap()]
                .iter()
                .flat_map(|v| touching[&v.row()].iter().copied())
                .filter(|face| !by.contains(face))
                .collect();
            TopoName::Bounded {
                kind: EdgeOrVertex::Edge,
                by: NameSet::new(by.iter().copied()).unwrap(),
                ends: NameSet::new(ends).unwrap(),
            }
            .id()
            .unwrap()
        })
        .collect()
}

/// The half-space behind the plane through `points`, made by operation `op`.
fn behind(op: u8, points: [[i64; 3]; 3]) -> Body {
    let points = points.map(|p| P3::new(p[0], p[1], p[2]).unwrap());
    half_space(ctx_of(op).path(), points, &mut meter()).unwrap()
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
    let door_faces = faces(&door);
    for record in split {
        let Origin::Split { of, piece } = &record.origin else {
            panic!()
        };
        let PieceKey::ByTools { tools, side } = piece;
        assert!(faces(&wall).contains(of));
        assert!(!tools.names().is_empty(), "no cutting face");
        // The tools are the door's faces, never the wall's own, and every piece the door leaves
        // lies in front of the door face that cuts it.
        for tool in tools.names() {
            assert!(door_faces.contains(tool), "a wall face named as a tool");
        }
        assert_eq!(*side, Side::Front);
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
    // Faces of two names merge under both: nothing is qualified, so the far corners keep theirs.
    assert!(
        !out.diagnostics
            .iter()
            .any(|d| d.code == "bool.names_qualified")
    );
    for (body, corner) in [(&left, [0, 0, 0]), (&right, [80, 40, 40])] {
        let name = vertex_at(body, corner).unwrap();
        assert_eq!(vertex_at(&result.body, corner), Some(name));
        assert_eq!(origin_of(history, name), &Origin::Unchanged);
    }
}

#[test]
fn a_body_united_with_its_copy_alongside_qualifies_the_faces_they_both_bring() {
    // Bottom, top, front and back each merge two faces of one name, one from each operand; the
    // two ends each come from one operand alone.
    let one = cuboid(1, [0, 0, 0], [40, 40, 40]);
    let two = cuboid(1, [40, 0, 0], [40, 40, 40]);
    let out = unite(&ctx(), &SERIAL, &[solid(1, &one), solid(2, &two)]).unwrap();
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.code == "bool.names_qualified")
    );
    let result = out.result;
    let history = lawful(&result, &[&one, &two]);
    let after = faces(&result.body);
    let kept = faces(&one)
        .into_iter()
        .filter(|n| after.contains(n))
        .count();
    assert_eq!(kept, 2, "the two ends");
    let merged: Vec<&Record> = history
        .records()
        .iter()
        .filter(|r| after.contains(&r.output) && matches!(r.origin, Origin::Generated { .. }))
        .collect();
    assert_eq!(merged.len(), 4);
    for record in merged {
        let Origin::Generated { from, role } = &record.origin else {
            panic!()
        };
        assert_eq!(role.name(), "merged");
        assert!(history.deleted().names().contains(&from.names()[0]));
    }
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
fn a_half_space_has_names_and_a_section_has_a_history() {
    let a = cuboid(1, [0, 0, 0], [40, 40, 40]);
    let b = cuboid(2, [20, 20, 20], [40, 40, 40]);
    let out = combine(
        &ctx(),
        &SERIAL,
        &BooleanInput {
            operands: vec![solid(1, &a), solid(2, &b)],
            select: vec![Selection::Section],
            merge: Merge::RemoveRedundant,
            accept: None,
        },
    )
    .unwrap();
    let curves = &out.result[0][0];
    let history = curves.history.as_ref().unwrap();
    assert_eq!(history.records().len(), 6 + 6 + 1);
    let inputs: BTreeSet<NameId> = all_names(&a).union(&all_names(&b)).copied().collect();
    history.check(&NameSet::new(inputs).unwrap()).unwrap();
}

#[test]
fn an_operand_without_names_gives_a_body_without_history() {
    let a = cuboid(1, [0, 0, 0], [40, 40, 40]);
    let plain = geom_bool::half_space::half_space(
        ctx().path(),
        [
            P3::new(20, 0, 0).unwrap(),
            P3::new(20, 1, 0).unwrap(),
            P3::new(20, 0, 1).unwrap(),
        ],
        &mut meter(),
    )
    .unwrap();
    let named = plain;
    let out = unite(&ctx(), &SERIAL, &[solid(1, &a), solid(2, &named)]).unwrap();
    assert!(
        out.result.history.is_some(),
        "a half-space has names of its own"
    );
    let bare = geom_topo::Builder::edit(&named);
    let bare = bare
        .publish(named.kind(), named.outside(), &mut meter())
        .unwrap();
    let out = unite(&ctx(), &SERIAL, &[solid(1, &a), solid(2, &bare)]).unwrap();
    assert!(out.result.history.is_none());
}

#[test]
fn a_naming_error_says_what_failed() {
    assert_eq!(NamingError::Broken.to_string(), "naming lost a row");
    let output = NameId::from_bytes([3; 16]);
    let cases = [
        (NamingError::Num(NumError::Degenerate), "naming met "),
        (
            NamingError::Name(NameError::EmptyRole),
            "a name could not be formed: ",
        ),
        (
            NamingError::History(HistoryError::RepeatedOutput { output }),
            "a history could not be formed: ",
        ),
        (
            NamingError::Order(OrderError::TooMany),
            "names could not be ordered: ",
        ),
    ];
    for (error, opening) in cases {
        let text = error.to_string();
        assert!(text.starts_with(opening), "{text}");
        assert!(text.len() > opening.len(), "{text}: the cause is missing");
    }
}

#[test]
fn edges_a_cut_leaves_alone_keep_their_names_and_one_it_shortens_is_modified() {
    // The notch takes the wall's top right edge, and shortens the four edges that end there; the
    // window, in the middle of the wall, touches no edge of it.
    let wall = cuboid(1, [0, 0, 0], [100, 10, 60]);
    let notch = cuboid(2, [90, -5, 50], [20, 20, 20]);
    let window = cuboid(3, [40, -5, 20], [20, 20, 20]);
    let operands = [solid(1, &wall), solid(2, &notch), solid(3, &window)];
    let result = subtract(&ctx(), &SERIAL, key(1), &operands).unwrap().result;
    let history = lawful(&result, &[&wall, &notch, &window]);
    let after: BTreeSet<NameId> = edges(&result.body).iter().map(|(_, n)| *n).collect();
    let (mut unchanged, mut modified, mut gone) = (0, 0, 0);
    for ([low, high], name) in edges(&wall) {
        let at_notch = |p: [i64; 3]| p[0] == 100 && p[2] == 60;
        match (at_notch(low), at_notch(high)) {
            (true, true) => {
                assert!(!after.contains(&name));
                assert!(history.deleted().names().contains(&name));
                gone += 1;
            }
            (true, false) | (false, true) => {
                assert!(after.contains(&name), "a shortened edge lost its name");
                let Origin::Modified { from } = origin_of(history, name) else {
                    panic!("a shortened edge is {:?}", origin_of(history, name))
                };
                assert_eq!(from.names(), [name]);
                modified += 1;
            }
            (false, false) => {
                assert!(
                    after.contains(&name),
                    "an edge the cut left alone lost its name"
                );
                assert_eq!(origin_of(history, name), &Origin::Unchanged);
                unchanged += 1;
            }
        }
    }
    assert_eq!((unchanged, modified, gone), (7, 4, 1));
}

#[test]
fn a_new_edge_is_named_by_the_faces_around_it_and_the_faces_that_end_it() {
    let wall = cuboid(1, [0, 0, 0], [100, 10, 60]);
    let window = cuboid(2, [40, -5, 20], [20, 20, 20]);
    let operands = [solid(1, &wall), solid(2, &window)];
    let result = subtract(&ctx(), &SERIAL, key(1), &operands).unwrap().result;
    let history = lawful(&result, &[&wall, &window]);
    let expected = bounded_edges(&result.body);
    let mut new = 0;
    for (row, (_, name)) in edges(&result.body).into_iter().enumerate() {
        if let Origin::Generated { role, .. } = origin_of(history, name) {
            assert_eq!(role.name(), "edge");
            assert_eq!(name, expected[row], "edge {row}");
            new += 1;
        }
    }
    // Four where the reveals meet, and four where each meets the front and the back.
    assert_eq!(new, 12);
}

#[test]
fn a_vertex_where_three_planes_meet_keeps_its_name_through_a_later_cut() {
    // x + y + 3z = 149 takes the corner (30, 30, 30) off, and meets the vertical edge under it at
    // z = 89/3, off the grid.
    let block = cuboid(1, [0, 0, 0], [30, 30, 30]);
    let corner = behind(5, [[29, 30, 30], [30, 29, 30], [2, 0, 49]]);
    let operands = [solid(1, &block), solid(2, &corner)];
    let cut = subtract(&ctx(), &SERIAL, key(1), &operands).unwrap().result;
    lawful(&cut, &[&block, &corner]);
    let met = meetings(&cut.body);
    assert_eq!(met.len(), 1);
    // A hole far from that corner.
    let hole = cuboid(6, [5, -5, 5], [5, 40, 5]);
    let operands = [solid(1, &cut.body), solid(2, &hole)];
    let again = subtract(&ctx_of(8), &SERIAL, key(1), &operands)
        .unwrap()
        .result;
    let history = lawful(&again, &[&cut.body, &hole]);
    assert_eq!(meetings(&again.body), met);
    for (_, name) in vertices(&cut.body) {
        assert!(vertices(&again.body).iter().any(|(_, n)| *n == name));
        assert_eq!(origin_of(history, name), &Origin::Unchanged);
    }
}

#[test]
fn a_new_vertex_half_a_step_from_an_old_one_takes_a_name_of_its_own() {
    // z = 89/3 rounds to the corner (30, 30, 30) that the cut takes away, but is not it.
    let block = cuboid(1, [0, 0, 0], [30, 30, 30]);
    let corner = behind(5, [[29, 30, 30], [30, 29, 30], [2, 0, 49]]);
    let operands = [solid(1, &block), solid(2, &corner)];
    let cut = subtract(&ctx(), &SERIAL, key(1), &operands).unwrap().result;
    let history = lawful(&cut, &[&block, &corner]);
    let gone = vertex_at(&block, [30, 30, 30]).unwrap();
    assert!(vertices(&cut.body).iter().all(|(_, n)| *n != gone));
    assert!(history.deleted().names().contains(&gone));
    let met = meetings(&cut.body);
    assert_eq!(met.len(), 1);
    assert!(matches!(
        origin_of(history, met[0]),
        Origin::Generated { .. }
    ));
    // x + y + 7z = 267 takes that point off in turn, and meets the same edge at z = 207/7, which
    // rounds to the same grid point but lies on another plane.
    let more = behind(7, [[27, 30, 30], [30, 27, 30], [0, 1, 38]]);
    let operands = [solid(1, &cut.body), solid(2, &more)];
    let again = subtract(&ctx_of(8), &SERIAL, key(1), &operands)
        .unwrap()
        .result;
    let history = lawful(&again, &[&cut.body, &more]);
    assert!(vertices(&again.body).iter().all(|(_, n)| *n != met[0]));
    assert!(history.deleted().names().contains(&met[0]));
    let now = meetings(&again.body);
    assert_eq!(now.len(), 1);
    assert!(matches!(
        origin_of(history, now[0]),
        Origin::Generated { .. }
    ));
}
