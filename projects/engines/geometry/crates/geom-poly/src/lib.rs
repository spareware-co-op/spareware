//! Spare Kit geometry kernel: exact plane-based polyhedra.
//!
//! One job: Exact plane-based polyhedra and their arrangements; plane commit; indicators, early-out
//! derivation and piece selection; the maximal-face merge with T-junction insertion, shared by
//! `geom-bool` and `geom-mesh`.
//!
//! Layer: 6, Arrangements (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - The arrangement adds no error: every decision is an exact predicate on integer planes within
//!   section 6.2's bit budgets (11.3).
//! - Planes are reduced, with normals within 83 bits and offsets within 125, checked at
//!   construction (6.2, 24.7).
//! - A vertex is the meeting of three planes the body holds, never rounded (24.3).
//! - Subdivision is a pure function of its box, its polygons and its reference point; ties break by
//!   operand key, then name, and results join in tree order, never thread order (11.12).
//! - Plane commit records its proven deviation and keeps the input's exact grid symmetries (13.2).
//!
//! Sections: 6.2, 11.3, 11.12, 11.14, 13.2, 19.5 and 23.2; open questions OQ-11.2 and OQ-19.4, in
//! the kernel architecture (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-math merges (G03): geom-math.
//!
//! Status: scaffold only; built by pieces G123, G124, G125, G126, G127, G128, G142.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
