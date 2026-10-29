//! Spare Kit geometry kernel: every curve kind.
//!
//! One job: Every curve kind in 2D and 3D, spirals included: evaluation, bounds, projection,
//! conversion.
//!
//! Layer: 2, Geometry (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Each kind is stored as itself, exactly, in a closed enum: arcs stay arcs, never converted to
//!   NURBS behind the caller's back (7).
//! - Conversion to NURBS is on demand and returns the NURBS, whether it is exact, and its maximum
//!   error (7).
//! - Every kind gives evaluation, derivatives and conservative bounds on value and derivatives over
//!   any parameter interval; derivatives are never taken by finite differences (7, 12.1, 13.21).
//! - Projection and inversion return enclosures with proofs, never bare numbers (12.4).
//! - Spirals evaluate Certified, by quadrature with a proven bound whose panel count depends only
//!   on the segment's parameters (17.8).
//! - A transform that changes a kind, such as a non-uniform scale turning a circle into an ellipse,
//!   does so explicitly (7).
//!
//! Sections: 7, 12.1, 12.4, 17.8, 19.3 and 26.1, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Modules today (piece G60): [`ParamCurve`], the behaviour every kind has, and [`CurveKind`],
//! the closed enum that stores every kind, in [`param`]; the NURBS kind (`nurbs`); typed transforms
//! on control points with a certified bound ([`affine`]); the kind contract any new kind must pass
//! ([`contract`]); and [`CurveError`] in [`error`].
//!
//! Status: the traits, the enum, the NURBS kind and the contract built (G60); the other kinds to
//! come in G61, G62, G63, G64, G67, G68, G175, G361 and G371.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod affine;
pub mod contract;
pub mod error;
mod nurbs;
pub mod param;

pub use error::{ContractError, CurveError, Law};
pub use param::{CurveKind, DerivativeBound, ParamCurve};
