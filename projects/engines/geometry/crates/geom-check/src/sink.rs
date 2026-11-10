//! Where a check puts what it finds, and how it pays for its work: the faults found so far, the
//! count of those past the cap, and the caller's meter.
//!
//! What it does: [`Sink`] holds the faults of one check and the caller's [`WorkMeter`]. It
//! reserves room for [`MAX_FAULTS`] faults before the check starts, as Parasolid's body check
//! takes a largest number of faults to return, so recording a fault never allocates; a fault past
//! the cap is counted, not kept. Each check charges one work unit per row it visits
//! ([`Sink::charge`]), so a check of a huge body can be stopped by the caller's budget or
//! cancellation like any operation (23.10). The checks' scratch space is reserved once, by
//! [`crate::check`], so a refused allocation stops the check through the meter instead of
//! aborting ([`Sink::out_of_memory`]).
//!
//! Guarantees:
//!
//! - **The first faults kept, in order, and the rest counted.** [`Sink::push`] keeps a fault
//!   while fewer than [`MAX_FAULTS`] are kept, and counts it otherwise; it never allocates.
//! - **Never aborts on memory.** A refused reservation is the meter's
//!   [`geom_run::Stop::OutOfMemory`].
//!
//! Method: the work meter of chapter 23.10 (`geom_run::WorkMeter`), charged per row as
//! `geom_topo::Builder::publish` charges per chunk
//! (`projects/engines/geometry/architecture/23-performance.md`, section 23.10), and the cap on
//! returned faults of Parasolid's body check
//! (`projects/kit/notes/G01-evidence/02-occt-parasolid-acis.md`, section 5).
//!
//! Cost: constant time per charge and per fault; [`Sink::new`] reserves [`MAX_FAULTS`] faults.

use alloc::vec::Vec;

use geom_run::{Stopped, WorkMeter};

use crate::report::Fault;

/// The most faults one check keeps; later ones are counted ([`crate::Report::more`]).
pub const MAX_FAULTS: usize = 256;

/// The faults of one check, and the caller's meter.
pub(crate) struct Sink<'m, 'c> {
    /// The first [`MAX_FAULTS`] faults found, in the order found.
    pub(crate) faults: Vec<Fault>,
    /// How many more were found.
    pub(crate) more: u64,
    /// How many faces and shells were left for the pieces that certify curved ones.
    pub(crate) deferred: u64,
    /// The caller's meter.
    meter: &'m mut WorkMeter<'c>,
}

impl<'m, 'c> Sink<'m, 'c> {
    /// An empty sink charging `meter`, with room for [`MAX_FAULTS`] faults.
    pub(crate) fn new(meter: &'m mut WorkMeter<'c>) -> Result<Self, Stopped> {
        let mut faults = Vec::new();
        if faults.try_reserve_exact(MAX_FAULTS).is_err() {
            return Err(meter.out_of_memory());
        }
        Ok(Sink {
            faults,
            more: 0,
            deferred: 0,
            meter,
        })
    }

    /// Charges one work unit, for one row visited.
    pub(crate) fn charge(&mut self) -> Result<(), Stopped> {
        self.meter.charge(1)
    }

    /// The caller's meter, for what charges its own rows.
    pub(crate) fn meter(&mut self) -> &mut WorkMeter<'c> {
        self.meter
    }

    /// Counts one face or shell left for the pieces that certify curved ones.
    pub(crate) fn defer(&mut self) {
        self.deferred = self.deferred.saturating_add(1);
    }

    /// Records a fault: kept while fewer than [`MAX_FAULTS`] are, counted otherwise.
    pub(crate) fn push(&mut self, fault: Fault) {
        if self.faults.len() < MAX_FAULTS {
            self.faults.push(fault);
        } else {
            self.more = self.more.saturating_add(1);
        }
    }

    /// How many faults have been found so far, kept or counted.
    pub(crate) fn found(&self) -> u64 {
        u64::try_from(self.faults.len())
            .unwrap_or(u64::MAX)
            .saturating_add(self.more)
    }

    /// The meter's stop for a refused allocation.
    pub(crate) fn out_of_memory(&mut self) -> Stopped {
        self.meter.out_of_memory()
    }
}
