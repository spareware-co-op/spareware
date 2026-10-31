//! A body's faces with their names and roles (22; piece G102): roles read from history, through
//! every kind of origin, never from anything else about the face.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a failure should stop loudly"
)]

use geom_name::{faces, roles_of};
use geom_num::Exactness;
use geom_op::history::{History, Origin, Record};
use geom_op::{ErrorKind, NameId, NameSet, OpContext, OpId, OpPath, Role};
use geom_run::{Budget, Stop, WorkMeter};
use geom_topo::order::{NameKey, Names};
use geom_topo::{
    BodyKind, Builder, Face, Ix, Region, RegionKind, Sense, Shell, SurfaceKind,
};

fn id(n: u8) -> NameId {
    NameId::from_bytes([n; 16])
}

fn role(name: &str) -> Role {
    Role::new(name, []).unwrap()
}

fn set(names: &[NameId]) -> NameSet {
    NameSet::new(names.iter().copied()).unwrap()
}

fn generated(n: u8, name: &str) -> Record {
    Record {
        output: id(n),
        origin: Origin::Generated {
            from: set(&[]),
            role: role(name),
        },
    }
}

fn history(records: Vec<Record>) -> History {
    History::new(records, set(&[]), []).unwrap()
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn roles(name: u8, histories: &[&History]) -> Vec<String> {
    roles_of(id(name), histories, &mut meter())
        .unwrap()
        .iter()
        .map(|r| r.name().to_string())
        .collect()
}

#[test]
fn a_generated_entity_has_its_role() {
    let made = history(vec![generated(1, "top")]);
    assert_eq!(roles(1, &[&made]), ["top"]);
    // No history covers another name.
    assert!(roles(2, &[&made]).is_empty());
    assert!(roles(1, &[]).is_empty());
}

#[test]
fn a_modified_entity_carries_the_roles_of_what_it_carries_on() {
    let made = history(vec![generated(1, "top")]);
    let moved = history(vec![Record {
        output: id(1),
        origin: Origin::Modified { from: set(&[id(1)]) },
    }]);
    assert_eq!(roles(1, &[&made, &moved]), ["top"]);
    // And an entity that passed through unchanged: the walk looks further back.
    let passed = history(vec![Record {
        output: id(1),
        origin: Origin::Unchanged,
    }]);
    assert_eq!(roles(1, &[&made, &passed, &moved]), ["top"]);
    // A modification with a new name goes back to the old one.
    let renamed = history(vec![Record {
        output: id(9),
        origin: Origin::Modified { from: set(&[id(1)]) },
    }]);
    assert_eq!(roles(9, &[&made, &renamed]), ["top"]);
}

#[test]
fn a_generated_entity_later_on_stops_the_walk_at_its_own_role() {
    let first = history(vec![generated(1, "old")]);
    let second = history(vec![generated(1, "new")]);
    assert_eq!(roles(1, &[&first, &second]), ["new"]);
    // The earlier history alone is asked what happened before a step, not after it.
    let moved = history(vec![Record {
        output: id(1),
        origin: Origin::Modified { from: set(&[id(1)]) },
    }]);
    assert_eq!(roles(1, &[&first, &moved, &second]), ["new"]);
    assert_eq!(roles(1, &[&second, &moved, &first]), ["old"]);
}

#[test]
fn a_merged_entity_has_every_role_sorted_and_without_repeats() {
    let made = history(vec![
        generated(1, "side"),
        generated(2, "cap"),
        generated(3, "side"),
    ]);
    let merged = history(vec![Record {
        output: id(4),
        origin: Origin::Merged {
            of: set(&[id(1), id(2), id(3)]),
        },
    }]);
    assert_eq!(roles(4, &[&made, &merged]), ["cap", "side"]);
}

#[test]
fn every_entry_is_charged_and_visited_once() {
    let made = history(vec![generated(1, "side"), generated(2, "cap")]);
    let merged = history(vec![Record {
        output: id(4),
        origin: Origin::Merged {
            of: set(&[id(1), id(2)]),
        },
    }]);
    let mut m = meter();
    roles_of(id(4), &[&made, &merged], &mut m).unwrap();
    assert_eq!(m.used().work, 3);
    // Two paths to one name visit it once.
    let again = history(vec![
        Record {
            output: id(5),
            origin: Origin::Merged {
                of: set(&[id(1), id(2)]),
            },
        },
        Record {
            output: id(6),
            origin: Origin::Modified { from: set(&[id(1)]) },
        },
    ]);
    let last = history(vec![Record {
        output: id(7),
        origin: Origin::Merged {
            of: set(&[id(5), id(6)]),
        },
    }]);
    let mut m = meter();
    roles_of(id(7), &[&made, &again, &last], &mut m).unwrap();
    assert_eq!(m.used().work, 5);
    let mut tight = WorkMeter::new(Budget::of_work(2));
    let stop = roles_of(id(7), &[&made, &again, &last], &mut tight).unwrap_err();
    assert!(matches!(stop.reason(), Stop::OverBudget(_)));
}

/// A body of two faces with no loops, named 1 and 2.
fn named() -> geom_topo::Body {
    let ix = |r: u32| Ix::new(r).unwrap();
    let mut b = Builder::new();
    let a = &mut b.arenas;
    a.regions
        .extend(&[
            Region {
                kind: RegionKind::Solid,
                first_shell: Some(ix(0)),
            },
            Region {
                kind: RegionKind::Void,
                first_shell: None,
            },
        ])
        .unwrap();
    a.shells
        .extend(&[Shell {
            region: ix(0),
            next: None,
            first_face: Some(ix(0)),
        }])
        .unwrap();
    let zero = geom_num::P3::new(0, 0, 0).unwrap();
    let bound = geom_math::Box3::new(zero, zero).unwrap();
    a.faces
        .extend(&[
            Face {
                shell: ix(0),
                next: Some(ix(1)),
                surface: ix(0),
                sense: Sense::Forward,
                first_loop: None,
                bound,
            },
            Face {
                shell: ix(0),
                next: None,
                surface: ix(1),
                sense: Sense::Reversed,
                first_loop: None,
                bound,
            },
        ])
        .unwrap();
    let plane = |z: f64| {
        SurfaceKind::Plane(
            geom_surface::Plane::new(
                [0.0, 0.0, z],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                ((0.0, 1.0), (0.0, 1.0)),
            )
            .unwrap(),
        )
    };
    a.surfaces.extend([plane(0.0), plane(1.0)]).unwrap();
    let key = |n: u8| NameKey([n; 16]);
    b.publish_named(
        &Names {
            regions: vec![key(10), key(11)],
            faces: vec![key(1), key(2)],
            edges: Vec::new(),
            vertices: Vec::new(),
        },
        BodyKind::Sheet,
        ix(1),
        &mut meter(),
    )
    .unwrap()
}

fn ctx(budget: Budget) -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([3; 16])), budget)
}

#[test]
fn faces_are_listed_with_their_names_and_roles() {
    let body = named();
    let made = history(vec![generated(1, "bottom"), generated(2, "top")]);
    let out = faces(
        &ctx(Budget::UNLIMITED),
        &body,
        &[&made],
        Exactness::Exact,
    )
    .unwrap();
    assert_eq!(out.exactness, Exactness::Exact);
    let rows = out.result;
    let listed: Vec<(Option<NameId>, Vec<String>)> = rows
        .iter()
        .map(|f| {
            (
                f.name,
                f.roles.iter().map(|r| r.name().to_string()).collect(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            (Some(id(1)), vec!["bottom".to_string()]),
            (Some(id(2)), vec!["top".to_string()]),
        ]
    );
    assert_eq!(rows[0].row.face.row(), 0);
    assert_eq!(rows[1].row.sense, Sense::Reversed);
}

#[test]
fn a_body_without_names_lists_no_names_and_no_roles() {
    let mut b = Builder::new();
    b.arenas
        .regions
        .push(Region {
            kind: RegionKind::Void,
            first_shell: None,
        })
        .unwrap();
    let empty = b
        .publish(BodyKind::General, Ix::new(0).unwrap(), &mut meter())
        .unwrap();
    let out = faces(&ctx(Budget::UNLIMITED), &empty, &[], Exactness::Exact).unwrap();
    assert!(out.result.is_empty());
}

#[test]
fn a_stop_is_the_contexts_error() {
    let body = named();
    let made = history(vec![generated(1, "bottom"), generated(2, "top")]);
    for work in 0..4 {
        let e = faces(&ctx(Budget::of_work(work)), &body, &[&made], Exactness::Exact).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::BudgetExceeded);
    }
    assert!(faces(&ctx(Budget::of_work(4)), &body, &[&made], Exactness::Exact).is_ok());
}
