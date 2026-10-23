//! Spare Kit geometry kernel: drawings.
//!
//! One job: Sections, projections, hidden lines, drawing curves with provenance, the drawing
//! checker.
//!
//! Layer: 10, Operations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Drawing never changes the model; a section drawing is a query (18).
//! - Nothing is rounded to make a drawing: points keep their exact form, and rounding happens only
//!   in the explicit explode and export operations, once (18.3).
//! - Every drawing passes the drawing checker, and each view succeeds or fails whole, never as a
//!   partial drawing (18.10, 18.11).
//! - Drawing is a versioned operation: within an edition the same model and view give the same
//!   drawing, bit for bit (18.10).
//! - Events order by exact signs, then by names; output sorts by band, class, source name and piece
//!   key (18.11).
//!
//! Sections: 18 and 23.8; open question OQ-18.2, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-math merges (G03): geom-math.
//!
//! Status: scaffold only; built by pieces G387, G389, G390, G391, G394, G395, G396, G397, G398,
//! G399, G400, G401, G402, G403, G404, G405.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
