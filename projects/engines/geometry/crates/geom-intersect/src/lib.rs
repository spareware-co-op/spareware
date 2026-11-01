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
//! - [`project`]: nearest points and inversion (G162).
//! - [`level`]: the implicit-parametric tracer and level curves (G168).
//! - [`general`]: general surface with surface, subdivision and start points (G169).
//! - [`march`]: certified marching between start points (G170).
//! - [`known`]: intersections known by construction, taken out only where a tube is proven (G174).
//! - [`seam`]: seams, poles and knot lines, read exactly from each kind, and where a branch passes
//!   them (G173).
//! - [`selfcross`]: where a surface crosses itself, by Volino and Magnenat-Thalmann's test (G176).
//! - [`solve`] and [`curve`] (piece G163): the interval engine, and where two curves meet.
//! - [`curve_surface`] (piece G164): where a curve meets a surface.
//! - [`closed`] (piece G159): exact decisions for a line with a quadric or torus, and the special
//!   positions of two quadrics.
//!
//! Status: the result types and canonical order built by piece G158, projection and inversion by
//! G162, the implicit tracer and level curves by G168, subdivision by G169, marching by G170, seams,
//! poles and knot lines by G173, known curves by G174, self-intersection by G176; the other methods
//! come with G159, G163, G164, G165, G166, G167, G170, G171, G172, G175, G292,
//! G320, G321, G372, G390, G392, G393, G466, G467, G468.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod bound;
pub mod closed;
pub mod contact;
pub mod curve;
pub mod curve_surface;
pub mod error;
pub mod general;
pub mod known;
pub mod level;
pub mod march;
pub mod order;
pub mod project;
pub mod result;
pub mod seam;
pub mod selfcross;
pub mod solve;

pub use error::IntersectError;
pub use level::{
    Branch, Envelope, FlaggedBox, Isocline, Level, LevelCurves, OnImplicit, Perspective, Sample,
    Silhouette, trace,
};
pub use order::{CanonicalKey, Geom, Operand, PLAN, pair, pairs, share};
pub use project::{
    Continuum, Nearest, Place, Projection, invert_curve, invert_surface, project_curve,
    project_surface,
};
pub use result::{
    Coincidence, Coverage, CurveContact, CurveEnd, Degeneracy, Diag, Intersection, Item,
    OverlapRegion, ParamBox, PointContact, StepsInterval, Test, Unresolved,
};
pub use seam::{Charts, Pass, Passing, charts};
