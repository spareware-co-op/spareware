//! Spare Kit geometry kernel: subdivision bodies.
//!
//! One job: The editable subdivision body: validation, whole-net refinement, net edits, PCCM
//! patches.
//!
//! Layer: 8, Other representations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - The control net is exact: its points are on the grid and its polygons consistently oriented;
//!   the limit surface is certified (9, 16.1).
//! - One scheme, Catmull-Clark with semi-sharp creases (16.1).
//! - Conversion to NURBS is exact on regular regions and approximate with a stored deviation at
//!   extraordinary points, and never silent (9, 16.1).
//! - Routes that need trim-free T-splines or watertightCAD reparameterisation are never offered
//!   (16.3).
//!
//! Sections: 9, 16.1, 16.3 and 16.6; open question OQ-16.1, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Status: scaffold only; built by pieces G315, G316, G319, G323, G325.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
