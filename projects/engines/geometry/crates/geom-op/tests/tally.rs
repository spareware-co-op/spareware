//! Tests of a context's tally (piece G98), through the public interface.
//!
//! What this file proves: a run reports exactly what it charged, on success and on failure, every
//! run adds to the same tally, and a tally changes nothing about the run.

#![allow(
    clippy::unwrap_used,
    reason = "test cases; the rules are for kernel code"
)]

use std::sync::Mutex;

use geom_num::NumError;
use geom_op::{ErrorKind, OpContext, OpError, OpId, OpPath, Tally};
use geom_run::{Budget, BudgetKind, Usage};

#[derive(Default)]
struct Sum(Mutex<Vec<Usage>>);

impl Tally for Sum {
    fn add(&self, used: Usage) {
        self.0.lock().unwrap().push(used);
    }
}

fn path() -> OpPath {
    OpPath::new(OpId::from_bytes([0x98; 16]))
}

#[test]
fn a_run_reports_what_it_charged_and_nothing_more() {
    let sum = Sum::default();
    let ctx = OpContext::new(path(), Budget::new(100, 1000)).with_tally(&sum);
    assert!(ctx.tally().is_some());
    let got = ctx.run(|meter| {
        meter.charge(7).unwrap();
        Ok(1)
    });
    assert_eq!(got.unwrap(), 1);
    // A body that fails has still charged.
    let failed: Result<u8, OpError> = ctx.run(|meter| {
        meter.charge(3).unwrap();
        Err(OpError::new(path(), NumError::Degenerate))
    });
    assert_eq!(failed.unwrap_err().kind(), ErrorKind::Degenerate);
    let seen = sum.0.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].get(BudgetKind::Work), 7);
    assert_eq!(seen[1].get(BudgetKind::Work), 3);
}

#[test]
fn a_tally_changes_nothing() {
    let quiet = OpContext::new(path(), Budget::new(5, 1000));
    assert!(quiet.tally().is_none());
    let sum = Sum::default();
    let heard = quiet.clone().with_tally(&sum);
    let charge = |ctx: &OpContext<'_>| {
        ctx.run(|meter| {
            meter
                .charge(9)
                .map_err(|s| ctx.stopped(meter, s, geom_op::RUN))?;
            Ok(())
        })
    };
    let (a, b) = (charge(&quiet), charge(&heard));
    assert_eq!(a.unwrap_err().kind(), ErrorKind::BudgetExceeded);
    assert_eq!(b.unwrap_err().kind(), ErrorKind::BudgetExceeded);
    assert_eq!(sum.0.lock().unwrap().len(), 1);
}

struct Stopped;

impl geom_run::Cancel for Stopped {
    fn requested(&self) -> bool {
        true
    }
}

#[test]
fn an_unbudgeted_meter_has_no_limit_and_still_hears_a_stop() {
    let ctx = OpContext::new(path(), Budget::new(1, 1));
    let mut meter = ctx.unbudgeted_meter();
    assert!(meter.charge(1 << 40).is_ok());
    assert!(meter.poll().is_ok());
    let stop = Stopped;
    let stopped = OpContext::new(path(), Budget::new(1, 1)).with_cancel(&stop);
    let mut meter = stopped.unbudgeted_meter();
    assert!(meter.poll().is_err());
}
