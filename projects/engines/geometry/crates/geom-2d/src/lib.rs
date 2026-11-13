//! Spare Kit geometry kernel: 2D drafting.
//!
//! One job: Drafting entities, the 2D arrangement, regions, hatches, 2D booleans and offsets,
//! edits, snaps and references, 2D distance and clearance, survey linework and COGO.
//!
//! Layer: 6, Arrangements (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Stored curves are never approximated; every cross, touch or contain question about lines, arcs
//!   and ellipses is answered exactly (14, 14.13).
//! - Never decides topology from chords, samples or floats; no absolute epsilon; points are never
//!   merged by rounding them into bins (14.13).
//! - Never lowers exactness without an acceptance, and never rounds a construction where that would
//!   flip a decision (14.13).
//! - Hatch lines, linetype dashes and dimension pictures are derived and deterministic, never
//!   stored as truth (14.13).
//! - The same grid, predicates, curve kinds and names as the 3D kernel (principle 8).
//!
//! Sections: 14 and 17.2; open questions OQ-12.1 and OQ-14.9, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Family: 2D.
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Status: carriers, points and commits (G192), tangent constructions (G193), the entity model
//! (G194), contacts read from the certified curve-curve intersection (G195), the arrangement of
//! lines, arcs and ellipses (G196, first part), snaps, tracking and the 2D index (G198), updates and
//! point location for lines (G199), region finding (G200), transform and length edits (G206) and
//! spline fitting and biarcs (G213); built on by pieces G197, G199 (arcs), G201, G202, G203, G204,
//! G205, G207, G208, G209, G210, G211, G212, G359, G360, G361.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod accept;
pub mod arrange;
pub mod carrier;
pub mod commit;
pub mod contact;
pub mod edit;
pub mod entity;
pub mod fit;
mod ideal;
pub mod index;
pub mod point;
pub mod region;
pub mod snap;
pub mod tangent;
pub mod update;
