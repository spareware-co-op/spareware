//! The exactness report and the validity report as operations (22; piece G102): what each says of
//! a flat and a curved body, a stop as the context's error, and the record coming back with the
//! answer.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{certified, cube, round_cylinder};
use geom_check::Level;
use geom_check::query::{ExactnessReport, exactness, validity};
use geom_num::Exactness;
use geom_op::{ErrorKind, OpContext, OpId, OpPath};
use geom_run::Budget;
use geom_topo::BodyKind;

fn ctx(budget: Budget) -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), budget)
}

#[test]
fn a_flat_body_reports_its_record_and_its_counts() {
    let body = cube().body(BodyKind::Solid);
    let out = exactness(&ctx(Budget::UNLIMITED), &body, Exactness::Exact).unwrap();
    assert_eq!(out.exactness, Exactness::Exact);
    let ExactnessReport {
        recorded,
        flat_faces,
        curved_faces,
        straight_edges,
        curved_edges,
        report,
    } = out.result;
    assert_eq!(recorded, Exactness::Exact);
    assert_eq!((flat_faces, curved_faces), (6, 0));
    assert_eq!((straight_edges, curved_edges), (12, 0));
    assert!(report.passed());
}

#[test]
fn a_curved_body_counts_what_is_curved_and_an_exact_record_on_it_fails() {
    let body = round_cylinder().body(BodyKind::Solid);
    let good = exactness(&ctx(Budget::UNLIMITED), &body, certified()).unwrap();
    assert_eq!(good.exactness, certified());
    let good = good.result;
    assert_eq!((good.flat_faces, good.curved_faces), (2, 1));
    assert_eq!((good.straight_edges, good.curved_edges), (0, 2));
    assert!(good.report.passed());
    let bad = exactness(&ctx(Budget::UNLIMITED), &body, Exactness::Exact).unwrap();
    assert!(!bad.result.report.passed());
}

#[test]
fn the_validity_report_is_the_checks_at_the_level_asked() {
    let body = cube().body(BodyKind::Solid);
    let full = validity(
        &ctx(Budget::UNLIMITED),
        &body,
        Exactness::Exact,
        Level::Full,
    )
    .unwrap();
    assert_eq!(full.exactness, Exactness::Exact);
    let full = full.result;
    assert!(full.passed());
    assert_eq!(full.ran().count(), 4);
    let cheap = validity(&ctx(Budget::UNLIMITED), &body, certified(), Level::Cheap).unwrap();
    assert_eq!(cheap.exactness, certified());
    assert_eq!(cheap.result.ran().count(), 2);
}

#[test]
fn a_stop_is_the_contexts_error_and_gives_no_report() {
    let body = cube().body(BodyKind::Solid);
    let work = |w| ctx(Budget::of_work(w));
    for w in [0, 1, 6, 17, 18, 19] {
        let e = exactness(&work(w), &body, Exactness::Exact);
        if let Err(e) = e {
            assert_eq!(e.kind(), ErrorKind::BudgetExceeded);
        }
    }
    // Six faces, twelve edges, then the cheap groups: each pass can stop.
    assert_eq!(
        exactness(&work(3), &body, Exactness::Exact)
            .unwrap_err()
            .kind(),
        ErrorKind::BudgetExceeded
    );
    assert_eq!(
        exactness(&work(10), &body, Exactness::Exact)
            .unwrap_err()
            .kind(),
        ErrorKind::BudgetExceeded
    );
    assert_eq!(
        exactness(&work(19), &body, Exactness::Exact)
            .unwrap_err()
            .kind(),
        ErrorKind::BudgetExceeded
    );
    assert_eq!(
        validity(&work(3), &body, Exactness::Exact, Level::Full)
            .unwrap_err()
            .kind(),
        ErrorKind::BudgetExceeded
    );
}
