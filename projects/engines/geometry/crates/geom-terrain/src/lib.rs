//! Spare Kit geometry kernel: terrain.
//!
//! One job: TINs from points and breaklines, contours, volumes, terrain solids, grading, flow and
//! sight, `OnSurface` references.
//!
//! Layer: 10, Operations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - A TIN is an exact mesh with one height at each plan position, every vertex a grid point
//!   (17.3).
//! - Built by constrained Delaunay triangulation on exact predicates; points on one circle are
//!   separated by symbolic perturbation ordered by names, never by moving a point, so the surface
//!   is one answer however it is built (17.3).
//! - Crossing breaklines follow the operation's explicit policy; breaklines that would make a
//!   vertical face are an error with remedies (17.3).
//! - `OnSurface` references resolve in the current version or fail with a reason (17.12).
//! - Each result carries the class section 17.13 gives it, checked by a property test.
//!
//! Sections: 17.3 to 17.7, 17.11, 17.12 and 17.13; open question OQ-17.3, in the kernel
//! architecture (`projects/engines/geometry/architecture/`).
//!
//! Family: Ground.
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Status: scaffold only; built by pieces G362, G363, G364, G365, G366, G367, G368, G369, G370,
//! G374, G377, G380.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
