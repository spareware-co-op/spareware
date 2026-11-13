//! Differential testing against outside oracles (chapter 28.10; piece G113): the same job through
//! another system, and an investigation of every difference.
//!
//! What it does: this crate holds the jobs and the comparison code; every oracle that is not a
//! permissive Rust crate runs out of process, in its own container, from the oracle repository.
//!
//! - [`catalogue`]: the maintained list of oracles, with their licences and pins, and the checks
//!   that nothing is linked or published.
//! - [`exact`]: the exact-input writer: whole grid steps for an exact oracle, doubles written so they
//!   read back bit for bit for a floating-point one, and the rounding a change of unit costs.
//! - [`compare`]: exact comparison, and comparison within an allowance of both sides' stated bounds
//!   plus the conversion rounding, capped at our bound plus the fit bound.
//! - [`triage`]: the signature of a difference, the register of triaged signatures, and the four
//!   outcomes, each written as a fixture.
//! - [`geomdl`]: the first oracle, geomdl, on NURBS: its jobs, and each answer compared, with any
//!   difference settled in exact rationals by [`crate::nurbs_reference`].
//!
//! - [`smlib`]: the second, `SMLib`'s `NLib` layer, on the same lines: evaluation, knot insertion and
//!   removal, and degree elevation.
//! - [`sketches`]: ezpz and fiksi, two sketch solvers, against the block solver on sketches drawn
//!   from a known truth (G222).
//!
//! The binary `oracle` lists the catalogue, checks it (`-- check`), writes a night's jobs
//! (`-- jobs`), compares the answers (`-- compare`) and records a triage (`-- triage`).
//!
//! Guarantees:
//!
//! - **Never trusted as truth, never linked, never shipped** (28.10): an oracle only ever raises a
//!   difference, ground truth settles it, and [`catalogue::linked`] fails a build that links one.
//! - **The oracle sees what the kernel saw**: [`exact`] writes each input so it reads back to the
//!   same bits.
//! - **The allowance never grows with the oracle's tolerance**: it is capped at our bound plus the
//!   fit bound ([`compare::Allowance::total`]).
//! - **Each signature is judged once**: a nightly run fails only on a new one.
//!
//! Method: chapter 28.10 and decision 32.6
//! (`projects/engines/geometry/architecture/28-testing.md`,
//! `projects/engines/geometry/architecture/32-decisions.md`).
//!
//! Cost: as each part's header states.

pub mod catalogue;
pub mod compare;
pub mod exact;
pub mod geomdl;
pub mod sketches;
pub mod smlib;
pub mod triage;
