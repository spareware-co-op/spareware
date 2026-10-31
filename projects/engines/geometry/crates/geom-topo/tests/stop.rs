//! What a builder's error means to the meter that ran it: a refused allocation is out of memory on
//! that meter, a stop is the meter's own, and anything else is no stop at all.

#![allow(
    clippy::unwrap_used,
    reason = "test code: a failure should stop loudly"
)]

use geom_run::{Budget, Stop, WorkMeter};
use geom_topo::TopoError;

#[test]
fn a_refused_allocation_is_out_of_memory_on_the_meter() {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let stop = TopoError::OutOfMemory.stop(&mut meter).unwrap();
    assert_eq!(stop.reason(), Stop::OutOfMemory);
    assert_eq!(meter.stopped(), Some(Stop::OutOfMemory));
}

#[test]
fn a_stop_is_the_meters_own() {
    let mut meter = WorkMeter::new(Budget::new(1, u64::MAX));
    let stopped = meter.charge(2).unwrap_err();
    let error = TopoError::from(stopped);
    assert_eq!(error.stop(&mut meter).unwrap().reason(), stopped.reason());
    // A stop from another run says nothing of a meter that has not stopped.
    let mut other = WorkMeter::new(Budget::UNLIMITED);
    assert!(error.stop(&mut other).is_none());
}

#[test]
fn any_other_error_is_no_stop() {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    assert!(TopoError::Outside.stop(&mut meter).is_none());
    assert_eq!(meter.stopped(), None);
}
