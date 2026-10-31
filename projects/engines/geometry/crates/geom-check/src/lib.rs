//! Spare Kit geometry kernel: the validity checker.
//!
//! One job: The B-rep validity checker.
//!
//! Layer: 5, Checker (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Checks run in groups: combinatorial, geometric consistency, self-intersection and the
//!   exactness record (8.3).
//! - Geometric consistency is certified with bounds, never sampled (8.3).
//! - Self-intersection checks use the same certified intersections the operations do
//!   (ARCHITECTURE.md, layers).
//! - The cheap groups run after every operation in every build; the full set runs in debug builds,
//!   in continuous integration, on every import and on request (8.3).
//! - The checker reports what fails with the entities involved and never modifies a body (principle
//!   4).
//!
//! Sections: 8.3, 10.2, 16.1, 18.10, 19.2, 22, 24.7 and 28.4, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Modules today (piece G84, the framework and groups 1 and 4):
//!
//! - [`check`], in `run`: one check of a body at a [`Level`], returning a [`Report`].
//! - [`report`]: the [`Group`]s of 8.3, the [`Level`]s, the [`Entity`] a [`Fault`] is at, its
//!   [`FaultKind`], and the [`Report`].
//! - `combinatorial`: group 1, loops close, fin partners, shells closed and oriented, regions,
//!   Euler's formula.
//! - `exactness`: group 4, the exactness record against the geometry.
//! - `sink`: the faults found so far, capped at [`MAX_FAULTS`], and the caller's meter.
//! - [`VERSION`]: the checker's version, as validation stamps record it (G108).
//!
//! Status: the framework and groups 1 and 4 (G84) built; geometric consistency and
//! self-intersection to come in G85, G102, G134, G177 and G322.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod combinatorial;
mod exactness;
pub mod report;
mod run;
mod sink;

pub use report::{Entity, Fault, FaultKind, Group, Level, Report};
pub use run::check;
pub use sink::MAX_FAULTS;

/// The checker's version, which a validation stamp records (24.7): a stored body from a known
/// source whose stamp comes from a checker at least this new, and lists every group this one has,
/// needs only the cheap groups when it is opened. It rises with every group built and every check
/// made stricter: G84's framework with groups 1 and 4 is version 1.
pub const VERSION: u64 = 1;
