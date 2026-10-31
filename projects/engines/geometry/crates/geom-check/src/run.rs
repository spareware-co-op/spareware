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
//! Cost: the sum of the groups run, each linear in the body's rows except group 3, quadratic in
//! faces and in shells; it reserves [`crate::MAX_FAULTS`] faults and one scratch buffer of a few
//! words per row (at most four per vertex and one per edge), shared by every check.

use alloc::vec::Vec;

use geom_num::Exactness;
use geom_run::{Stopped, WorkMeter};
use geom_topo::Body;

use crate::report::{Group, Level, Report};
use crate::sink::Sink;
use crate::{combinatorial, exactness, flat, orientation};
use crate::{geometric, self_intersection};

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
    let missing = [false; 4];
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
            // Group 2 walks the rings group 1 proves finite, so it runs only on a body group 1
            // passed; otherwise it is skipped, neither run nor missing.
            Group::Geometric => {
                if sink.found() > 0 {
                    continue;
                }
                geometric::check(body, exactness, &mut sink)?;
                flat::boxes(body.arenas(), &mut sink)?;
                &mut ran
            }
            // Group 3 compares the carriers group 2 certified, so it runs only on a body groups 1
            // and 2 passed.
            Group::SelfIntersection => {
                if sink.found() > 0 {
                    continue;
                }
                self_intersection::check(body, exactness, &mut sink)?;
                // Which way a loop runs means something only for a loop that does not cross
                // itself, and which way a shell faces and where it lies only for shells that do
                // not cross.
                if sink.found() == 0 {
                    flat::sense(body, &mut sink)?;
                    orientation::check(body, &mut scratch, &mut sink)?;
                }
                &mut ran
            }
        };
        done.get_mut(at).into_iter().for_each(|slot| *slot = true);
    }
    Ok(Report {
        faults: sink.faults,
        more: sink.more,
        deferred: sink.deferred,
        ran,
        missing,
    })
}
