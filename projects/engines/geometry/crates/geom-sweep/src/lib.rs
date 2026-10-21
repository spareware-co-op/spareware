//! Spare Kit geometry kernel: sweeps and lofts.
//!
//! One job: Extrude, revolve, sweep, loft, network surfaces, sectioned solids, corridor solids,
//! swept volumes of moving tools.
//!
//! Layer: 9, Shared operation services (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Never returns a body with a hole, a missing face or a face without geometry; never changes its
//!   inputs or any other body; a failed or cancelled operation publishes nothing (13.21, 10.1).
//! - Never ignores a cusp, loop or self-intersection, never grows a tolerance, and never retries
//!   with nudged inputs or stitches a result afterwards so it passes the checker (13.21).
//! - Never turns an analytic face into NURBS behind the caller's back, or lowers a result's class
//!   unasked (13.21).
//! - Faces are named by role (`cap.start`, `cap.end`, `side(source edge)`), so changing a dimension
//!   changes geometry, not names (15.14, 20.2).
//! - Corridor solids take sections already placed along a directrix curve, so this crate never
//!   depends on `geom-align` (30.6).
//!
//! Sections: 13.1 to 13.7, 13.17 and 17.9; open questions OQ-13.3 and OQ-17.3, in the kernel
//! architecture (`projects/engines/geometry/architecture/`).
//!
//! Family: Sweep and Manufacture.
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//!
//! Status: scaffold only; built by pieces G255, G256, G257, G259, G260, G261, G280, G375.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
