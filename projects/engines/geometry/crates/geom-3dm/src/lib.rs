//! Spare Kit geometry kernel: 3dm exchange.
//!
//! One job: Geometry mapping to and from Rhino's 3dm format.
//!
//! Layer: 12, Format and exchange (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Blocked: nothing is built or ported until `McNeel` confirms the openNURBS licence in writing
//!   (owner action G407; 24.11).
//! - Then it ports only what it needs, keeping the licence and its notice: knot-vector conversion,
//!   tolerances imported as Tolerant bodies, and subdivision cages where Rhino's rules match ours
//!   (24.11, 29.20).
//! - Until then Rhino users exchange through STEP (24.11).
//!
//! Sections: 16.1, 24.10, 24.11 and 29.14, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! The kinds it maps, from sections 24.10 and 24.11: NURBS curves and surfaces, B-rep and Tolerant
//! bodies, meshes and subdivision cages.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//! Direct dependencies still to add when geom-math merges (G03): geom-math.
//!
//! Status: scaffold only; built by pieces G438.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
