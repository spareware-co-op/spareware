//! A check charges the caller's meter for every row it visits, so a budget can stop it at any of
//! them (23.10): for each body, every budget below what the whole check costs stops it, with the
//! meter's own stop, and the whole cost lets it finish with the same report as an unlimited meter.
//!
//! The bodies between them reach every group and every check of each: a closed solid recorded
//! exact, a sheet with a hole, a body with first-stage faults, and one with second-stage faults.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{Rows, annulus_sheet, certified, cube, ix, meter, torus, wire};
use geom_check::{Level, check};
use geom_num::Exactness;
use geom_run::{Budget, BudgetKind, Stop, WorkMeter};
use geom_topo::BodyKind;

/// Sweeps every budget below the whole check's cost on `rows` as `kind`.
fn sweep(name: &str, rows: &Rows, kind: BodyKind, exactness: Exactness) {
    let body = rows.body(kind);
    let mut unlimited = meter();
    let whole = check(&body, exactness, Level::Full, &mut unlimited).unwrap();
    let cost = u64::try_from(unlimited.used().work).unwrap();
    assert!(cost > 0, "{name}");
    for budget in 0..cost {
        let mut m = WorkMeter::new(Budget::new(budget, u64::MAX));
        let stopped = check(&body, exactness, Level::Full, &mut m).unwrap_err();
        assert_eq!(
            stopped.reason(),
            Stop::OverBudget(BudgetKind::Work),
            "{name}, budget {budget}"
        );
    }
    let mut m = WorkMeter::new(Budget::new(cost, u64::MAX));
    assert_eq!(
        check(&body, exactness, Level::Full, &mut m).unwrap(),
        whole,
        "{name}"
    );
}

#[test]
fn every_charge_can_stop_the_check() {
    sweep("cube", &cube(), BodyKind::Solid, Exactness::Exact);
    sweep("annulus", &annulus_sheet(), BodyKind::Sheet, certified());
    sweep("torus", &torus(), BodyKind::Solid, certified());
    sweep("wire", &wire(), BodyKind::Wire, Exactness::Exact);
    // First-stage faults: the walks stop at a cycle.
    let mut broken = cube();
    broken.shells[0].next = Some(ix(0));
    broken.fins[1].next = ix(1);
    sweep("broken lists", &broken, BodyKind::Solid, certified());
    // Second-stage faults, and an edge without fins.
    let mut odd = cube();
    odd.edge(Some((0, 7)));
    odd.vertex([5, 5, 5]);
    sweep("misplaced", &odd, BodyKind::Solid, certified());
}

#[test]
fn a_cheap_check_costs_no_more_than_a_full_one() {
    let body = cube().body(BodyKind::Solid);
    let mut full = meter();
    check(&body, Exactness::Exact, Level::Full, &mut full).unwrap();
    let mut cheap = meter();
    check(&body, Exactness::Exact, Level::Cheap, &mut cheap).unwrap();
    // Groups 2 and 3 are not built, so the two levels do the same work today.
    assert_eq!(cheap.used(), full.used());
}
