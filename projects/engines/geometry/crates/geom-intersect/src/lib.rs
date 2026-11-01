//! Spare Kit geometry kernel: certified intersections.
//!
//! One job: Certified curve-curve, curve-surface and surface-surface intersection; level curves,
//! isoclines and contour generators; closed-form distances.
//!
//! Layer: 4, Intersections (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Every part of an answer is returned with a proof, or as a flagged region that says why it
//!   could not be proven; nothing is dropped, guessed or silently trimmed (12).
//! - Every point and parameter returned is an enclosure, never a bare number; an "exactly zero"
//!   decision is made by exact arithmetic, by identity of the defining data, or not claimed (12).
//! - Canonical operand order, fixed traversal and per-pair budgets: intersect(A, B) and
//!   intersect(B, A) give the same items with the roles swapped, on any thread count (12.15).
//! - Derived caps are errors: never a best-so-far point, never success on a truncated trace
//!   (12.15).
//! - Thresholds are in grid steps from the tolerance ladder, never a fraction of a surface's size
//!   (12.15).
//!
//! Sections: 12, 16.1, 17.8, 18.5, 19.10, 19.14 and 23.7; open questions OQ-12.1 and OQ-19.7, in
//! the kernel architecture (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Modules (piece G158):
//!
//! - [`result`]: what an intersection returns, its items and contact types, in canonical order.
//! - [`order`]: the canonical operand order, and pairs run one by one or as a batch.
//! - [`error`]: why an intersection could not be computed.
//! - [`solve`] and [`curve`] (piece G163): the interval engine, and where two curves meet.
//! - [`curve_surface`] (piece G164): where a curve meets a surface.
//! - [`quadrics`] (piece G165): where two natural quadrics meet, in closed form.
//! - [`closed`] (piece G159): exact decisions for a line with a quadric or torus, and the special
//!   positions of two quadrics.
//!
//! Status: the result types and canonical order built by piece G158, projection and inversion by
//! G162; the other methods come with G159,
//! G162, G163, G164, G165, G166, G167, G168, G169, G170, G171, G172, G173, G174, G175, G176, G292,
//! G320, G321, G372, G390, G392, G393, G466, G467, G468.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod closed;
pub mod curve;
pub mod curve_surface;
pub mod error;
pub mod order;
pub mod project;
pub mod quadrics;
pub mod quartic;
mod reduced;
pub mod result;
pub mod solve;

pub use error::IntersectError;
pub use order::{CanonicalKey, Geom, Operand, PLAN, pair, pairs, share};
pub use project::{
    Continuum, Nearest, Place, Projection, invert_curve, invert_surface, project_curve,
    project_surface,
};
pub use result::{
    Coincidence, Coverage, CurveContact, CurveEnd, Degeneracy, Diag, Intersection, Item,
    OverlapRegion, ParamBox, PointContact, StepsInterval, Test, Unresolved,
};
