//! Spare Kit geometry kernel: IFC exchange.
//!
//! One job: Geometry mapping to and from IFC 4.3, IFC4 and `IFC2x3`, with `IFC4x1` and 4x2
//! alignments as far as they map.
//!
//! Layer: 12, Format and exchange (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Maps every representation type of `IfcShapeRepresentation` both ways; an unmapped entity
//!   becomes `Unsupported` with its source ID and a remedy, never dropped (24.8, 24.11).
//! - Keeps every source attribute in the foreign recipe; profiles stay foreign recipes and write
//!   back as the same profile (24.8, 24.11).
//! - No cutter is dropped for being thin or touching (24.11).
//! - Every import runs the full checker before publishing (24.7).
//! - Rust types are generated, with attribution, from the official pinned IFC 4.3 EXPRESS release
//!   (24.11).
//!
//! Sections: 13.7, 17.13, 24.8, 24.10, 24.11 and 24.12, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! The kinds it maps, from section 24.11's table: curves, surfaces, B-rep and plane-based bodies,
//! face sets and meshes, 2D regions and text anchors, sweep and boolean recipes, alignment curves
//! and placements, terrain networks and point-cloud references. Generated EXPRESS tables (G416) are
//! committed here by `geom-gen`.
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Status: scaffold only; built by pieces G421, G422, G423, G424, G426.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
