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
//! - [`waiver`]: the night's report, the ratchet against the last one, and the waivers: a reviewed
//!   exception for a cell that went missing, and never for a difference.
//! - [`geomdl`]: the first oracle, geomdl, on NURBS: its jobs, and each answer compared, with any
//!   difference settled in exact rationals by [`crate::nurbs_reference`].
//!
//! The binary `oracle` lists the catalogue, checks it (`-- check`), writes a night's jobs
//! (`-- jobs`), compares the answers (`-- compare`) and records a triage (`-- triage`).
//!
//! Decision (piece S2, solidcore's difftest): the runner stays this one tool, and difftest's waiver
//! file, its evidence rule for a defect blamed on the reference, its report-to-report ratchet and
//! its perturbation check are re-derived in it in our terms, none of its code copied. What it
//! measures does not carry over: difftest holds a port equal to the C++ it was ported from, so the
//! reference is the truth and "green" is agreement; here a reference is never the truth, a
//! difference is settled by exact arithmetic, and a waiver therefore reaches coverage only, never a
//! difference. Left out: its prebuilt C++ libraries (the oracle builds from pinned source, 32.6),
//! its 1e-9 relative tolerance (comparisons are exact or within stated bounds), and its fuzzing of
//! the reference's guards and its heap-padding rerun, which probe a C++ library's undefined
//! behaviour, and no oracle here is one yet.
//!
//! The split with the oracle repository stays as it is: the out-of-process references live there,
//! and its nightly calls this tool's `jobs` and `compare` with the arguments it passes today. Every
//! flag added here is optional, so that workflow is unchanged; `--report` and `--baseline` are for
//! a night that keeps the last night's report, and wiring them into that workflow is its owner's.
//!
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/b4de2149aadbd737268091c51d8390a7d3cdda70/crates/difftest/README.md>,
//!   for the whole of the above. No code copied.
//!
//! Guarantees:
//!
//! - **Never trusted as truth, never linked, never shipped** (28.10): an oracle only ever raises a
//!   difference, ground truth settles it, and [`catalogue::linked`] fails a build that links one.
//! - **The oracle sees what the kernel saw**: [`exact`] writes each input so it reads back to the
//!   same bits.
//! - **The allowance never grows with the oracle's tolerance**: it is capped at our bound plus the
//!   fit bound ([`compare::Allowance::total`]).
//! - **Each signature is judged once**: a nightly run fails only on a new one, or on a job its
//!   record cannot cover.
//! - **Nothing hides a wrong answer of ours**: a register entry rests on what the exact reference
//!   found, a "theirs" carries a reproducing input and the pinned oracle it was reproduced on, a
//!   job found ours fails the night under any record, and the only waiver is for lost coverage
//!   ([`triage`], [`waiver`]).
//! - **The comparison is shown to see**: `compare --perturb` moves each agreeing answer and fails
//!   unless every one is caught ([`compare::perturbed`]).
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
pub mod triage;
pub mod waiver;
