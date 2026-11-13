//! Attacks on piece G137's claim for group 3: two faces with the same edges, each run the opposite
//! way, are the two sides of one wall in a general body and do not cross; the same two faces run
//! the same way do, and so do the two sides in a body that is not general.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{Rows, report};
use geom_check::FaultKind;
use geom_num::Exactness;
use geom_topo::{BodyKind, Sense};

/// Two square faces on the same four edges, the second run the other way round if `turned`.
fn pillow(turned: bool) -> Rows {
    let mut r = Rows::empty();
    for p in [[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]] {
        r.vertex(p);
    }
    let edges: Vec<u32> = (0..4).map(|i| r.edge(Some((i, (i + 1) % 4)))).collect();
    let s = r.shell(0);
    let one: Vec<(u32, Sense)> = edges.iter().map(|e| (*e, Sense::Forward)).collect();
    let mut two = one.clone();
    if turned {
        two = edges.iter().rev().map(|e| (*e, Sense::Reversed)).collect();
    }
    r.face(s, &[one.as_slice()]);
    r.face(s, &[two.as_slice()]);
    r
}

fn crosses(rows: &Rows, kind: BodyKind) -> bool {
    report(rows, kind, Exactness::Exact)
        .faults()
        .iter()
        .any(|fault| fault.kind == FaultKind::FacesCross)
}

#[test]
fn a_wall_of_a_general_body_is_two_faces_that_do_not_cross() {
    assert!(!crosses(&pillow(true), BodyKind::General));
}

#[test]
fn two_faces_run_the_same_way_cross() {
    assert!(crosses(&pillow(false), BodyKind::General));
}

#[test]
fn only_a_general_body_has_walls() {
    assert!(crosses(&pillow(true), BodyKind::Sheet));
}
