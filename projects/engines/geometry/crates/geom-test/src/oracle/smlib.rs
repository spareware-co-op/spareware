//! The second NURBS oracle: `SMLib`'s `NLib` layer, out of process (chapter 28.10; decision 32.23; piece
//! G161).
//!
//! What it does: [`jobs`] draws a night's seeded curves and what to do with each: evaluate a point,
//! insert a knot, remove one, or raise the degree, the layers decision 32.23 allows. A job is the
//! same line [`super::geomdl`] writes, and an answer is judged the same way
//! ([`super::geomdl::judge_for`]). A knot removal needs a knot that can go, so its curve is drawn
//! and then refined by that knot.
//!
//! Guarantees:
//!
//! - **Only what 32.23 allows.** No job reaches fitting, booleans, validity or the intersector, and
//!   the harness in the oracle repository compiles none of the routines 29.6 bars.
//! - **Truth settles it.** A point, an insertion and an elevation are settled in exact rationals;
//!   a removal has no exact answer to settle against, so it is held to the kernel's stated bound.
//! - **Repeated runs agree.** The nightly runs the oracle twice in fresh processes and refuses any
//!   answer that differs bit for bit, since 32.23 counts a class only while its runs agree.
//!
//! Method: chapter 28.10's oracle list and decision 32.23
//! (`projects/engines/geometry/architecture/28-testing.md`,
//! `projects/engines/geometry/architecture/32-decisions.md`).
//!
//! Cost: linear in the jobs; a removal job costs one knot insertion to draw.

use super::geomdl::{Job, Op, drawn_jobs, refined};
use crate::seed::Seed;

/// The oracle's id in the catalogue.
pub const ID: &str = "smlib-nlib";

/// The night's `count` jobs from `seed`.
#[must_use]
pub fn jobs(seed: Seed, count: u64) -> Vec<Job> {
    let ops = [Op::Point, Op::Insert, Op::Remove, Op::Elevate];
    drawn_jobs(seed, count, &ops)
        .into_iter()
        .map(|job| {
            if job.op == Op::Remove {
                // A knot that cannot be inserted stays a job both sides must refuse.
                refined(&job).unwrap_or(job)
            } else {
                job
            }
        })
        .collect()
}
