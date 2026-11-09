//! The checks that end on any version of the checker: valid bodies and faults with no cycle in
//! their lists or rings, and no loop of one fin, each held to exactly the faults it must give.
//!
//! Why a file of its own, named to run first: cargo runs a crate's test binaries in name order and
//! stops at the first that fails or hangs (chapter 28.8). A mutant that stops the checker noticing
//! cycles (a mark never kept, a stage that never runs, a fault never recorded) makes the bodies of
//! `faults.rs` that hold cycles loop for ever, so it could only be timed out there. Here no body
//! can loop, so the same mutant fails a test and is caught.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{Rows, cube, faults, ix, report, square_sheet, wire};
use geom_check::{Entity, Fault, FaultKind};
use geom_num::Exactness;
use geom_topo::BodyKind;

/// A cube's faults after `break_it`, as a solid.
fn broken_cube(break_it: impl FnOnce(&mut Rows)) -> Vec<Fault> {
    let mut r = cube();
    break_it(&mut r);
    faults(&r, BodyKind::Solid)
}

#[test]
fn valid_bodies_without_one_fin_loops_pass() {
    assert!(report(&cube(), BodyKind::Solid, Exactness::Exact).passed());
    assert!(report(&square_sheet(), BodyKind::Sheet, Exactness::Exact).passed());
    assert!(report(&wire(), BodyKind::Wire, Exactness::Exact).passed());
}

#[test]
fn faults_of_the_first_stage_alone() {
    assert_eq!(
        broken_cube(|r| r.faces[4].next = None),
        [Fault::at(FaultKind::Orphan, Entity::Face(ix(5)))]
    );
    assert_eq!(
        broken_cube(|r| r.fins[2].prev = ix(3)),
        [Fault::at(FaultKind::FinPrev, Entity::Fin(ix(1)))]
    );
    assert_eq!(
        broken_cube(|r| {
            r.fins[0].radial = ix(0);
            r.fins[19].radial = ix(19);
        }),
        [Fault::at(FaultKind::OffRadialRing, Entity::Fin(ix(19)))]
    );
    // The broken edge leaves the loops it is in open; the second stage must not run to say so.
    assert_eq!(
        broken_cube(|r| r.edges[0].end = None),
        [Fault::at(FaultKind::EdgeEnds, Entity::Edge(ix(0)))]
    );
}

#[test]
fn faults_of_the_second_stage() {
    assert_eq!(
        broken_cube(|r| {
            r.edge(Some((0, 7)));
        }),
        [Fault::at(FaultKind::Misplaced, Entity::Edge(ix(12)))]
    );
    // Face 0's fins in the order 0, 2, 1, 3: three fins end where the next does not start.
    let open = broken_cube(|r| {
        r.fins[0].next = ix(2);
        r.fins[2].prev = ix(0);
        r.fins[2].next = ix(1);
        r.fins[1].prev = ix(2);
        r.fins[1].next = ix(3);
        r.fins[3].prev = ix(1);
    });
    let at = |fin| Fault::at(FaultKind::LoopOpen, Entity::Loop(ix(0))).with(Entity::Fin(ix(fin)));
    assert_eq!(open, [at(0), at(2), at(1)]);
}

#[test]
fn a_report_with_faults_does_not_pass() {
    let r = report(
        &{
            let mut r = cube();
            r.faces[4].next = None;
            r
        },
        BodyKind::Solid,
        Exactness::Exact,
    );
    assert_eq!((r.faults().len(), r.more()), (1, 0));
    assert!(!r.passed());
}
