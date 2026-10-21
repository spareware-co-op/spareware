//! Spare Kit geometry kernel: division, panels and unrolling.
//!
//! One job: Surface division, patterns, panels, mould grouping, unrolling.
//!
//! Layer: 10, Operations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Division runs in a parameterisation the caller chooses, and every node lies on the true
//!   surface within its evaluation bound (16.4).
//! - A flat map is checked with exact predicates before it is used; a failed check is an error with
//!   remedies (16.4).
//! - Results are per tile, so one failed tile does not fail the rest (16.4).
//! - Grid lines are named by their anchor and signed offset; a division set by count alone has weak
//!   names (16.4, 20.2).
//! - The material's allowed strain is the caller's input; exceeding it is an error naming the
//!   regions (16.4).
//!
//! Sections: 16.4 and 16.6; open question OQ-16.1, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//!
//! Proposed in chapter 27, pending the owner's decision G08.
//!
//! Status: scaffold only; built by pieces G342, G343, G344, G345.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
