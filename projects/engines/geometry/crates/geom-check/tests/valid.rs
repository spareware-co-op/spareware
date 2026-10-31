//! Valid bodies pass: every kind of body, closed shells of genus 0 and 1, sheets with and without
//! holes, faces bounded by closed edges, at both levels and with every exactness record their
//! geometry allows; and the report says which groups ran and which are not built yet.
//!
//! The other half of chapter 28.4's fault injection: a checker that rejects a valid body is as
//! wrong as one that passes an invalid one.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{
    acorn, annulus_sheet, certified, cube, cylinder, disc_sheet, meter, report, square_sheet,
    torus, wire,
};
use geom_check::{Entity, Fault, FaultKind, Group, Level, check};
use geom_num::{Exactness, GridStep, Steps};
use geom_topo::BodyKind;

#[test]
fn every_kind_of_valid_body_passes() {
    let cases = [
        ("cube", cube(), BodyKind::Solid),
        ("cube as a general body", cube(), BodyKind::General),
        ("torus of planes and lines", torus(), BodyKind::Solid),
        ("square sheet", square_sheet(), BodyKind::Sheet),
        ("wire", wire(), BodyKind::Wire),
        ("wire as a general body", wire(), BodyKind::General),
        ("acorn", acorn(), BodyKind::Acorn),
        ("acorn as a general body", acorn(), BodyKind::General),
    ];
    for (name, rows, kind) in cases {
        for exactness in [
            certified(),
            Exactness::Approximate {
                deviation: Steps(3),
                step: GridStep::Q1,
            },
        ] {
            let r = report(&rows, kind, exactness);
            assert!(r.passed(), "{name}: {:?}", r.faults());
            assert_eq!(r.more(), 0);
        }
    }
}

#[test]
fn curved_carriers_are_reported_unchecked_never_passed() {
    // Group 2 has no certified check of circles yet: each is reported, and nothing else is.
    for (name, rows, kind) in [
        ("cylinder", cylinder(), BodyKind::Solid),
        ("disc sheet", disc_sheet(), BodyKind::Sheet),
        ("annulus sheet", annulus_sheet(), BodyKind::Sheet),
    ] {
        let r = report(&rows, kind, certified());
        assert!(!r.passed(), "{name}");
        assert!(
            r.faults()
                .iter()
                .all(|f| f.kind == FaultKind::Unchecked && matches!(f.at, Entity::Curve(_))),
            "{name}: {:?}",
            r.faults()
        );
    }
}

#[test]
fn flat_bodies_pass_recorded_exact() {
    for (name, rows, kind) in [
        ("cube", cube(), BodyKind::Solid),
        ("square sheet", square_sheet(), BodyKind::Sheet),
        ("wire", wire(), BodyKind::Wire),
        ("acorn", acorn(), BodyKind::Acorn),
    ] {
        let r = report(&rows, kind, Exactness::Exact);
        assert!(r.passed(), "{name}: {:?}", r.faults());
    }
    // A pcurve that is a line keeps an exact body exact (group 4); group 2 reports it unchecked
    // until pcurves have a certified check (G177), never passes it.
    let mut rows = cube();
    rows.pcurves.push(common::pline());
    rows.fins[0].pcurve = Some(common::ix(0));
    assert_eq!(
        report(&rows, BodyKind::Solid, Exactness::Exact).faults(),
        [
            Fault::at(FaultKind::Unchecked, Entity::Pcurve(common::ix(0)))
                .with(Entity::Fin(common::ix(0)))
        ]
    );
}

#[test]
fn a_cheap_check_runs_groups_1_and_4_and_a_full_one_names_what_is_missing() {
    let body = cube().body(BodyKind::Solid);
    let cheap = check(&body, Exactness::Exact, Level::Cheap, &mut meter()).unwrap();
    assert_eq!(
        cheap.ran().collect::<Vec<_>>(),
        [Group::Combinatorial, Group::Exactness]
    );
    assert_eq!(cheap.missing().count(), 0);
    assert!(cheap.passed());
    let full = check(&body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert_eq!(
        full.ran().collect::<Vec<_>>(),
        [Group::Combinatorial, Group::Geometric, Group::Exactness]
    );
    assert_eq!(
        full.missing().collect::<Vec<_>>(),
        [Group::SelfIntersection]
    );
    assert!(full.passed());
    // The same body gives the same report, every time.
    assert_eq!(
        check(&body, Exactness::Exact, Level::Full, &mut meter()).unwrap(),
        full
    );
}

#[test]
fn the_levels_and_groups_are_chapter_8_3s() {
    assert_eq!(Group::ALL.map(Group::number), [1, 2, 3, 4]);
    assert_eq!(Group::ALL.map(Group::cheap), [true, false, false, true]);
    assert_eq!(Group::ALL.map(Group::built), [true, true, false, true]);
    assert_eq!(
        Group::ALL.map(|g| Level::Cheap.runs(g)),
        [true, false, false, true]
    );
    assert_eq!(Group::ALL.map(|g| Level::Full.runs(g)), [true; 4]);
    // Debug builds, and builds with the `full` feature, run every group after each operation;
    // shipped release builds the cheap ones.
    let expected = if cfg!(any(debug_assertions, feature = "full")) {
        Level::Full
    } else {
        Level::Cheap
    };
    assert_eq!(Level::of_build(), expected);
}
