//! Spare Kit geometry kernel: alignments and corridors.
//!
//! One job: Alignments, stationing, the corridor model (ranges, stations, names), `Station`
//! references. Spiral kinds are `geom-curve`'s and corridor solids are `geom-sweep`'s.
//!
//! Layer: 10, Operations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Lengths are primary and stored as whole steps, so every station is an exact integer (17.8).
//! - Each segment's end lies within half a step plus its evaluation bound of the next segment's
//!   start, and declared tangent joins agree within the angular tolerance (17.8).
//! - Projection returns every candidate whose certified interval overlaps the smallest, in station
//!   order, never a guess (17.8).
//! - Never converts a spiral to NURBS behind the caller's back, derives a station from coordinates
//!   when a length is given, or integrates without a bound (17.8).
//! - `Station` references resolve on the current alignment or fail with a reason (17.12, 20.4).
//!
//! Sections: 17.8, 17.9 and 17.12; open question OQ-17.3, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Family: Ground.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//! Direct dependencies still to add when geom-math merges (G03): geom-math.
//!
//! Status: scaffold only; built by pieces G373, G374, G376.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
