//! Spare Kit geometry kernel: spatial queries, distance and clash.
//!
//! One job: Spatial queries on the box tree, distance, clearance, clash, contact over time, the
//! instance tree.
//!
//! Layer: 7, Meshes and spatial (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Every answer is sorted by persistent key and depends neither on the tree's shape nor on the
//!   thread count or target (19.9).
//! - Distance and clearance are an exact decision or a proven interval; clash gives one `Contact`
//!   per pair (19.13).
//! - Time of first contact is a certified interval that never misses a contact (10.2, 19.13).
//! - Below the geometry's certified bound, tangent contact is reported as `NearLimit` with that
//!   bound, never a guess (19.13).
//! - Every query on an instance equals the same query on its materialisation (23.9).
//!
//! Sections: 10.2, 19.9 to 19.12 and 23.9; open questions OQ-19.7 and OQ-19.17, in the kernel
//! architecture (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Status: scaffold only; built by pieces G293, G294, G295, G296, G297.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
