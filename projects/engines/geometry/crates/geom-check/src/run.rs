//! One check of a body: the groups its level asks for, in chapter 8.3's order, each charged to the
//! caller's meter, into one report.
//!
//! What it does: [`check`] runs, for each group of [`Group::ALL`] the [`Level`] asks for, the
//! group's checks on the body and the exactness the body is recorded with, and reports every
//! fault in the order found. A group the level asks for that this checker does not have yet is
//! listed as missing in the report, never run and never counted as passed. An operation calls it
//! on its result before returning it, at [`Level::of_build`] (8.3; principle 4); an import, and any
//! caller who asks, call it at [`Level::Full`].
//!
//! Guarantees:
//!
//! - **Never changes the body.** The check reads the body through `&Body` only.
//! - **The same report everywhere.** The groups run in 8.3's order and each in a fixed order, so
//!   one body gives one report on every machine and thread count.
//! - **Stoppable.** Every row visited is charged to the caller's meter, so the caller's budget or
//!   cancellation stops a check like any operation (23.10), and a refused allocation stops it as
//!   the meter's `OutOfMemory`, never as an abort.
//!
//! Method: Parasolid's body check in gated groups, reporting each fault with its entities
//! (`projects/kit/notes/G01-evidence/02-occt-parasolid-acis.md`, section 5), as chapter 8.3 and
//! 28.4 run it (`projects/engines/geometry/architecture/08-topology.md`,
//! `projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: the sum of the groups run, each linear in the body's rows; it reserves
//! [`crate::MAX_FAULTS`] faults and one scratch buffer of a few words per row (at most four per
//! vertex and one per edge), shared by every check.

use alloc::vec::Vec;

use geom_num::Exactness;
use geom_run::{Stopped, WorkMeter};
use geom_topo::Body;

use crate::report::{Group, Level, Report};
use crate::sink::Sink;
use crate::{combinatorial, exactness};

/// Checks `body`, recorded with `exactness`, at `level`.
///
/// # Errors
///
/// The meter's stop, if its budget runs out, the caller cancels or an allocation is refused,
/// before the check is done; nothing is reported then.
pub fn check(
    body: &Body,
    exactness: Exactness,
    level: Level,
    meter: &mut WorkMeter<'_>,
) -> Result<Report, Stopped> {
    let mut sink = Sink::new(meter)?;
    let words = combinatorial::words(body.arenas());
    let mut scratch: Vec<u32> = Vec::new();
    if scratch.try_reserve_exact(words).is_err() {
        return Err(sink.out_of_memory());
    }
    scratch.resize(words, 0);
    let mut ran = [false; 4];
    let mut missing = [false; 4];
    for (at, group) in Group::ALL.into_iter().enumerate() {
        if !level.runs(group) {
            continue;
        }
        let done = match group {
            Group::Combinatorial => {
                combinatorial::check(body, &mut scratch, &mut sink)?;
                &mut ran
            }
            Group::Exactness => {
                exactness::check(body, exactness, &mut sink)?;
                &mut ran
            }
            Group::Geometric | Group::SelfIntersection => &mut missing,
        };
        done.get_mut(at).into_iter().for_each(|slot| *slot = true);
    }
    Ok(Report {
        faults: sink.faults,
        more: sink.more,
        ran,
        missing,
    })
}
