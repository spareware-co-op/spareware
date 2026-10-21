//! Spare Kit geometry kernel: mesh exchange.
//!
//! One job: Geometry mapping to and from glTF 2.0, OBJ, STL, PLY and 3MF 1.4.
//!
//! Layer: 12, Format and exchange (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Formats without a unit (STL, OBJ, PLY) take the caller's unit; the importer never guesses
//!   (24.8).
//! - STL is welded only on bit-identical source values, and every rounding step is recorded (24.8).
//! - `f32` formats (glTF, STL) are Certified by their stated bound; exact metric grid meshes
//!   round-trip exactly through 3MF (24.10, 24.11).
//! - glTF and 3MF wait for a terms check that gives them rows in section 29 (24.11, 29.19).
//! - Imported meshes are repaired by `geom-mesh`, each step recorded (24.11).
//!
//! Sections: 16.1, 24.8, 24.10, 24.11 and 29.19, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! The kinds it maps, from section 24.11: exact meshes, placements and OBJ's subdivision cages.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//! Direct dependencies still to add when geom-math merges (G03): geom-math.
//!
//! Status: scaffold only; built by pieces G354, G434, G435.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
