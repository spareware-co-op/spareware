//! Spare Kit geometry kernel: point clouds as reference.
//!
//! One job: Point clouds as reference: the out-of-core index, snapping, primitive detection and
//! fitting.
//!
//! Layer: 8, Other representations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Reference only: a cloud is never part of a solid and never stored in the operation log (9,
//!   17.10).
//! - The kernel reads no files: a query names the chunks it needs and the program supplies them
//!   (17.10).
//! - Results are ordered by distance, then point index, the same on every machine (17.10).
//! - Random sampling uses the counter-based generator keyed by a seed in the operation's input
//!   (16.3, 17.10).
//! - Every fit is Approximate with a Certified deviation; an accepted fit becomes the inputs of a
//!   new operation and is never redone (17.10).
//!
//! Sections: 16.6, 17.10, 23.12 and 24.11; open questions OQ-16.1 and OQ-17.3, in the kernel
//! architecture (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-math merges (G03): geom-math.
//!
//! Status: scaffold only; built by pieces G350, G379, G380, G436.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
