//! Spare Kit geometry kernel: the conversion graph.
//!
//! One job: The conversion graph between representations, primitive recognition, the route query.
//!
//! Layer: 10, Operations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Every edge returns its class and bound; a route's class is the least exact of its edges, and
//!   bounds add (9, 16.3).
//! - An edge is Exact only if its output is the same shape as its input (16.3).
//! - Randomised methods take their seed as an explicit input and use the counter-based generator
//!   and the shared fixed-chunk helper (16.3).
//! - A point cloud never becomes part of a solid: converting one makes a new, separate body, marked
//!   Approximate (16.3).
//! - The route query says which routes exist and what class each would give before anything is
//!   computed (16.3).
//!
//! Sections: 9, 13.17, 16.3, 16.6 and 24.10; open question OQ-16.1, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Family: Free-form.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//!
//! Status: scaffold only; built by pieces G322, G323, G333, G334, G335, G340, G350, G351, G425.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
