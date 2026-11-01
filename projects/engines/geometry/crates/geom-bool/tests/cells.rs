//! Cells, rooms and what lies around them: a small storey against hand-counted boundaries, a leak
//! found at its width, and clash volumes (G137, 11.7).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic, and a failure stops the test"
)]

use geom_bool::combine::{Beyond, Combined, combine};
use geom_bool::operand::{
    BooleanInput, CellSeed, Operand, OperandKey, OperandKind, Selection, Term,
};
use geom_check::{Level, check};
use geom_make::{BoxInput, make_box};
use geom_num::{Exactness, P3, Steps};
use geom_op::{CombineDetail, OpContext, OpErrorDetail, OpId, OpOutput, OpPath};
use geom_poly::merge::Merge;
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::Body;
use geom_topo::query::volume;

const SERIAL: Exec<'static> = Exec::serial();

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn cuboid(n: u8, low: [i64; 3], size: [u64; 3]) -> Body {
    let input = BoxInput {
        min: p(low[0], low[1], low[2]),
        size: size.map(Steps),
    };
    let context = OpContext::new(OpPath::new(OpId::from_bytes([n; 16])), Budget::UNLIMITED);
    make_box(&context, &input).unwrap().result.body
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

fn sixfold(body: &Body) -> i128 {
    volume(body, &mut meter()).unwrap().to_i128().unwrap()
}

fn checked(body: &Body) {
    let report = check(body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(report.passed(), "{report:?}");
}

fn picked(operands: Vec<Operand<'_>>, selection: Selection) -> OpOutput<Vec<Vec<Combined>>> {
    let input = BooleanInput {
        operands,
        select: vec![selection],
        merge: Merge::RemoveRedundant,
        accept: None,
    };
    combine(&ctx(), &SERIAL, &input).unwrap()
}

/// A room is inside the envelope (1) and outside every wall (2 and on).
fn room_test(walls: &[u8]) -> Vec<Term> {
    let mut terms = vec![Term::Inside(key(1))];
    for wall in walls {
        terms.push(Term::Inside(key(*wall)));
        terms.push(Term::Not);
        terms.push(Term::And);
    }
    terms
}

fn seed(n: u8, point: P3) -> CellSeed {
    CellSeed { point, key: key(n) }
}

#[test]
fn a_wall_makes_two_rooms_each_named_by_its_seed() {
    let envelope = cuboid(1, [0, 0, 0], [100, 100, 30]);
    let wall = cuboid(2, [48, 0, 0], [4, 100, 30]);
    let selection = Selection::Cells {
        test: room_test(&[2]),
        seeds: vec![seed(50, p(20, 50, 15)), seed(51, p(80, 50, 15))],
    };
    let out = picked(vec![solid(1, &envelope), solid(2, &wall)], selection);
    let rooms = &out.result[0];
    assert_eq!(rooms.len(), 2);
    for room in rooms {
        assert_eq!(sixfold(&room.body), 6 * 48 * 100 * 30);
        checked(&room.body);
        let report = room.cell.as_ref().unwrap();
        assert_eq!(report.keys.len(), 1);
        // Five faces outside, and the wall beyond the sixth.
        let beyond: Vec<Beyond> = report.boundaries.iter().map(|(_, b)| *b).collect();
        assert_eq!(beyond.iter().filter(|b| **b == Beyond::Outside).count(), 5);
        assert_eq!(
            beyond
                .iter()
                .filter(|b| **b == Beyond::Element(key(2)))
                .count(),
            1
        );
    }
    let keys: Vec<OperandKey> = rooms
        .iter()
        .map(|r| r.cell.as_ref().unwrap().keys[0])
        .collect();
    assert!(keys.contains(&key(50)) && keys.contains(&key(51)));
    // A room's region is named by its seed, whatever else changes.
    let name = |room: &Combined| room.body.names().unwrap().regions.clone();
    assert_ne!(name(&rooms[0]), name(&rooms[1]));
    let thicker = cuboid(2, [47, 0, 0], [6, 100, 30]);
    let again = Selection::Cells {
        test: room_test(&[2]),
        seeds: vec![seed(50, p(20, 50, 15)), seed(51, p(80, 50, 15))],
    };
    let moved = picked(vec![solid(1, &envelope), solid(2, &thicker)], again);
    for (room, other) in rooms.iter().zip(&moved.result[0]) {
        assert_eq!(
            name(room),
            name(other),
            "the wall moved and the room kept its name"
        );
    }
}

#[test]
fn a_cutter_makes_rooms_that_see_what_it_cut_off() {
    let envelope = cuboid(1, [0, 0, 0], [100, 100, 30]);
    let stud = cuboid(2, [50, -10, -10], [1, 120, 50]);
    let cutter = Operand {
        kind: OperandKind::Cutter,
        ..solid(2, &stud)
    };
    let selection = Selection::Cells {
        test: vec![Term::Inside(key(1))],
        seeds: vec![seed(50, p(20, 50, 15)), seed(51, p(80, 50, 15))],
    };
    let out = picked(vec![solid(1, &envelope), cutter], selection);
    let rooms = &out.result[0];
    assert_eq!(rooms.len(), 2);
    let report = rooms[0].cell.as_ref().unwrap();
    let cells = report
        .boundaries
        .iter()
        .filter(|(_, b)| matches!(b, Beyond::Cell(_)))
        .count();
    assert_eq!(cells, 1);
}

#[test]
fn a_leak_is_reported_where_it_is_narrowest() {
    let envelope = cuboid(1, [0, 0, 0], [100, 100, 30]);
    let lower = cuboid(2, [48, 0, 0], [4, 50, 30]);
    let upper = cuboid(3, [48, 51, 0], [4, 49, 30]);
    let selection = Selection::Cells {
        test: room_test(&[2, 3]),
        seeds: vec![seed(50, p(20, 50, 15)), seed(51, p(80, 50, 15))],
    };
    let out = picked(
        vec![solid(1, &envelope), solid(2, &lower), solid(3, &upper)],
        selection,
    );
    assert_eq!(out.result[0].len(), 1);
    let report = out.result[0][0].cell.as_ref().unwrap();
    assert_eq!(report.keys.len(), 2);
    let note = out
        .diagnostics
        .iter()
        .find(|d| d.code == "bool.seeds_share_cell")
        .unwrap();
    assert!(
        note.message.contains("narrowest 1 steps"),
        "{}",
        note.message
    );
}

#[test]
fn seeds_must_lie_in_a_cell_and_on_no_face() {
    let envelope = cuboid(1, [0, 0, 0], [100, 100, 30]);
    let wall = cuboid(2, [48, 0, 0], [4, 100, 30]);
    let refused = |point| {
        let input = BooleanInput {
            operands: vec![solid(1, &envelope), solid(2, &wall)],
            select: vec![Selection::Cells {
                test: room_test(&[2]),
                seeds: vec![seed(50, point)],
            }],
            merge: Merge::RemoveRedundant,
            accept: None,
        };
        *combine(&ctx(), &SERIAL, &input).unwrap_err().detail()
    };
    assert_eq!(
        refused(p(48, 50, 15)),
        OpErrorDetail::Combine(CombineDetail::SeedOnBoundary)
    );
    assert_eq!(
        refused(p(50, 50, 15)),
        OpErrorDetail::Combine(CombineDetail::SeedOutside),
        "inside the wall"
    );
    assert_eq!(
        refused(p(500, 50, 15)),
        OpErrorDetail::Combine(CombineDetail::SeedOutside),
        "past the box"
    );
}

#[test]
fn clash_volumes_are_cells_labelled_by_what_overlaps() {
    let a = cuboid(1, [0, 0, 0], [40, 40, 40]);
    let b = cuboid(2, [30, 0, 0], [40, 40, 40]);
    let c = cuboid(3, [60, 0, 0], [40, 40, 40]);
    let selection = Selection::Cells {
        test: vec![Term::AtLeast(2)],
        seeds: Vec::new(),
    };
    let out = picked(vec![solid(1, &a), solid(2, &b), solid(3, &c)], selection);
    let clashes = &out.result[0];
    assert_eq!(clashes.len(), 2);
    let mut labels: Vec<(Vec<OperandKey>, i128)> = clashes
        .iter()
        .map(|r| (r.cell.as_ref().unwrap().elements.clone(), sixfold(&r.body)))
        .collect();
    labels.sort();
    assert_eq!(labels[0].0, [key(1), key(2)]);
    assert_eq!(labels[1].0, [key(2), key(3)]);
    assert_eq!(labels[0].1, 6 * 10 * 40 * 40);
    assert_eq!(labels[1].1, 6 * 10 * 40 * 40);
}

#[test]
fn a_small_storey_has_the_boundaries_counted_by_hand() {
    // Two rooms either side of a wall, a column in one of them, under a slab.
    let envelope = cuboid(1, [0, 0, 0], [200, 100, 30]);
    let wall = cuboid(2, [98, 0, 0], [4, 100, 30]);
    let column = cuboid(3, [40, 40, 0], [10, 10, 30]);
    let selection = Selection::Cells {
        test: room_test(&[2, 3]),
        seeds: vec![seed(50, p(20, 20, 15)), seed(51, p(150, 50, 15))],
    };
    let out = picked(
        vec![solid(1, &envelope), solid(2, &wall), solid(3, &column)],
        selection,
    );
    let rooms = &out.result[0];
    assert_eq!(rooms.len(), 2);
    let mut volumes: Vec<i128> = rooms.iter().map(|r| sixfold(&r.body)).collect();
    volumes.sort_unstable();
    assert_eq!(
        volumes,
        [6 * (98 * 100 * 30 - 10 * 10 * 30), 6 * 98 * 100 * 30]
    );
    let by_column = rooms
        .iter()
        .find(|r| r.cell.as_ref().unwrap().keys == [key(50)])
        .unwrap();
    let report = by_column.cell.as_ref().unwrap();
    let on = |what: Beyond| report.boundaries.iter().filter(|(_, b)| *b == what).count();
    assert_eq!(on(Beyond::Element(key(3))), 4, "the column's four sides");
    assert_eq!(on(Beyond::Element(key(2))), 1);
    assert_eq!(on(Beyond::Outside), 5);
}

#[test]
fn a_general_fuse_keeps_every_cell() {
    let a = cuboid(1, [0, 0, 0], [40, 40, 40]);
    let b = cuboid(2, [20, 0, 0], [40, 40, 40]);
    let out = picked(vec![solid(1, &a), solid(2, &b)], Selection::GeneralFuse);
    let fused = &out.result[0][0].body;
    let arenas = fused.arenas();
    assert_eq!(
        arenas.regions.len(),
        3 + 1,
        "left, both, right, and outside"
    );
    assert_eq!(arenas.shells.len(), 3);
    checked(fused);
}
