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
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Modules today: [`ParamCurve`], the behaviour every kind has, and [`CurveKind`], the closed enum
//! that stores every kind, in [`param`]; the NURBS kind (`nurbs`); typed transforms on control
//! points with a certified bound ([`affine`]); the kind contract any new kind must pass
//! ([`contract`]); and [`CurveError`] in [`error`] (piece G60). The analytic kinds (piece G61):
//! [`Line`] (segment, ray, infinite line) in [`line`](mod@line); [`Circle`], [`Ellipse`],
//! [`Hyperbola`] and [`Parabola`] in [`conic`]; [`Helix`] in [`helix`]; [`Polynomial`] in
//! [`polynomial`]; [`Involute`] in [`involute`]; exact frames ([`frame`]) and their axes
//! ([`axes`]); a kind's NURBS form and parameter map ([`map`]). Underneath: the shared certified
//! evaluation (`form`), transforms for the analytic kinds (`moved`), the exact rational conics
//! (`rational`), Taylor conversion with a certified remainder (`taylor`) and interval arithmetic
//! that stays exact where it can (`tight`).
//!
//! The kinds that hold curves (piece G62): [`BulgePolyline`] in [`bulge`]; [`Trimmed`] in
//! [`trimmed`]; [`Composite`] in [`composite`]; [`Offset`] in [`offset`], with interval Taylor
//! jets (`jet`); [`OnSurface`] in [`on_surface`]; all built on the leaves of [`Basis`] in
//! [`basis`], so no kind contains itself.
//!
//! Status: the traits, the enum, the NURBS kind and the contract (G60), the analytic kinds (G61)
//! and the bulge, trimmed, composite, offset and curve-on-surface kinds (G62) built; the other
//! kinds to come in G63, G64, G67, G68, G175, G361 and G371.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod affine;
pub mod axes;
pub mod basis;
pub mod bulge;
pub mod composite;
pub mod conic;
pub mod contract;
pub mod error;
mod form;
pub mod frame;
pub mod helix;
pub mod involute;
mod jet;
pub mod line;
pub mod map;
pub mod moved;
mod nurbs;
pub mod offset;
pub mod on_surface;
pub mod param;
pub mod polynomial;
mod rational;
pub mod taylor;
pub mod tight;
pub mod trimmed;

pub use axes::{Axes, HelixAxes};
pub use basis::Basis;
pub use bulge::BulgePolyline;
pub use composite::Composite;
pub use conic::{Circle, Ellipse, HYPERBOLA_REACH, Hyperbola, Parabola};
pub use error::{ContractError, CurveError, Law};
pub use frame::Frame;
pub use helix::Helix;
pub use involute::Involute;
pub use line::{Extent, Line};
pub use map::{LocalMap, MapPiece, NurbsForm, ParamMap};
pub use offset::{Offset, OffsetLaw, Station};
pub use on_surface::{OnSurface, SurfaceRef};
pub use param::{CurveKind, DerivativeBound, ParamCurve};
pub use polynomial::Polynomial;
pub use trimmed::Trimmed;
