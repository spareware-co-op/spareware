//! Spare Kit geometry kernel: NURBS algorithms.
//!
//! One job: NURBS algorithms: basis, evaluation, knot operations, degree change, fitting, Bernstein
//! bounds, Bezier extraction, symbolic products, skinning, Gordon and Coons combination.
//!
//! Layer: 2, Geometry (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Every algorithm's doc comment names its source: a NURBS Book algorithm number or a paper's DOI
//!   (7).
//! - Bounds are conservative, from Bernstein and interval arithmetic rounded outward, never from
//!   sampling (principle 2, 12.3).
//! - Sums run in a fixed written order (basis sums, Horner steps); no platform maths (21, 23.4).
//! - Well-formedness is checked at construction: knots non-decreasing, multiplicities at most
//!   degree plus one, weights positive (24.7).
//! - A fit states its certified error and is never accepted when it misses its bound; its
//!   least-squares solves go through `geom-linalg` (13.21, 15.6).
//!
//! Sections: 7, 12.3, 12.12, 13 (introduction), 14.1 and 15.6; 29.7, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Modules today (piece G56, The NURBS Book's chapters 2 to 5):
//!
//! - [`Basis`], in [`basis`]: a checked degree and knot vector, and its functions and their
//!   derivatives (A2.1 to A2.5).
//! - [`Curve`], in [`curve`]: curves of any dimension, their points and derivatives (A3.1 to
//!   A3.3, A4.1, A4.2).
//! - [`Surface`], in [`surface`]: surfaces, their points and partial derivatives (A3.5, A3.6,
//!   A4.3, A4.4).
//! - [`Point`], in [`point`]: the control points the algorithms take, `geom-math`'s and arrays.
//! - Knot insertion and refinement of curves, [`Curve::insert_knot`] and [`Curve::refine`]
//!   (A5.1, A5.4), each returning a [`Changed`] value with a certified deviation ([`ops`]).
//! - Bezier extraction and degree elevation of curves, [`Curve::to_bezier`] and
//!   [`Curve::elevate`] (A5.6, A5.9).
//! - Certified knot removal and degree reduction of curves, [`Curve::remove_knot`] and
//!   [`Curve::reduce_degree`] (A5.8, A5.11).
//! - Knot insertion, refinement, Bezier extraction and degree elevation of surfaces,
//!   [`Surface::insert_knot`], [`Surface::refine`], [`Surface::to_bezier`] and
//!   [`Surface::elevate`] (A5.3, A5.5, A5.7, A5.10).
//! - [`NurbsError`], in [`error`].
//!
//! Status: basis, evaluation and knot operations built (G56); interpolation and approximation
//! (G58), the Bernstein toolkit (G59) and the rest (G259) to come.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod arith;
pub mod basis;
mod bezier;
mod budget;
pub mod curve;
mod elevate;
pub mod error;
mod index;
mod insert;
mod net;
pub mod ops;
pub mod point;
mod reduce;
mod remove;
pub mod surface;

pub use basis::{Basis, Limit, MAX_DEGREE};
pub use curve::Curve;
pub use error::NurbsError;
pub use ops::{Changed, Direction};
pub use point::Point;
pub use surface::Surface;
