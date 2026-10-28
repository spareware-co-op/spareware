//! Spare Kit geometry kernel: DXF exchange.
//!
//! One job: Geometry mapping to and from DXF.
//!
//! Layer: 12, Format and exchange (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Follows Autodesk's public 2018 DXF reference; no behaviour is taken from observing proprietary
//!   software (14.14, 24.11).
//! - Proprietary or opaque entities (3DSOLID, `ACAD_PROXY_ENTITY` and the like) are unsupported,
//!   their bytes kept as an attribute with a remedy (24.11).
//! - Where a file declares no unit, the caller names one; the importer never guesses (24.8).
//! - DWG is an owner's decision after legal advice, and `LibreDWG` is never used (24.11, 29.14).
//!
//! Sections: 14, 24.8, 24.10 and 24.11, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! The kinds it maps, from section 24.11's table: curve kinds and Acorn bodies, extrusion recipes
//! for thickness, 2D regions, hatches, text anchors and dimensions, placed instances, exact meshes
//! and subdivision cages.
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Status: scaffold only; built by pieces G430, G431, G432, G433.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
