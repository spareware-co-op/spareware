//! Spare Kit geometry kernel: the one box tree.
//!
//! One job: The one tree of integer boxes, 2D and 3D.
//!
//! Layer: 2, Geometry (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Every box is rounded outward to whole grid steps and never misses what it should contain;
//!   boxes around certified or Tolerant geometry are padded by their bound (19.9, 23.7).
//! - Built top-down in one pass by the binned surface-area heuristic with integer costs; every
//!   split is a pure function of the node's contents, ties broken by entry key, so the tree is the
//!   same on any number of threads (19.9).
//! - Answers never depend on the tree's shape: every query returns its set sorted by key (19.9,
//!   23.7).
//! - Traversal uses an explicit stack with a depth limit, never recursion, and checks cancellation
//!   at fixed steps (19.9, principle 13).
//! - Parallel only through `geom-run`'s helper (23.4).
//!
//! Sections: 14.8, 19.9 and 23.7; open questions OQ-11.2 and OQ-19.1; chapter 27 ("Who owns what"),
//! in the kernel architecture (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//! Direct dependencies still to add when geom-math merges (G03): geom-math.
//!
//! Proposed in chapter 27, pending the owner's decision G08.
//!
//! Status: scaffold only; built by pieces G54, G55.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
