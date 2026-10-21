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
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//!
//! Status: scaffold only; built by pieces G84, G85, G102, G134, G177, G322.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
