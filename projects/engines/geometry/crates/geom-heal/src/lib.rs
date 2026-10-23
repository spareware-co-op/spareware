//! Spare Kit geometry kernel: healing and sewing.
//!
//! One job: Healing and sewing Tolerant B-rep bodies; rounding imported polyhedra.
//!
//! Layer: 12, Format and exchange (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Replaces geometry and never raises a tolerance; never heals beyond the recorded healing
//!   tolerance (24.9, 24.13).
//! - Each step is a recorded action in one operation (24.9).
//! - Free edges are sewn only when their curves lie within the healing tolerance along their whole
//!   length, by a certified bound, and the match is unique; otherwise nothing in the group is sewn
//!   and the report gives `Ambiguous` with the candidates (24.9).
//! - Rounding imported polyhedra calls mesh heal's quantise and weld, never a second copy (24.8).
//! - No rounded body is published invalid (24.8).
//!
//! Sections: 24.8 and 24.9; open question OQ-24.6; 30.5, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its dependency on `geom-exchange`, for the numbers-to-grid rules, is section 30.5's amendment.
//! Steps 1 and 2 of 24.9 use certified intersection; the edge to `geom-intersect` is added through
//! chapter 27's route when this crate's design note shows it.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Proposed in chapter 27, pending the owner's decision G08.
//!
//! Status: scaffold only; built by pieces G417, G419, G420.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
