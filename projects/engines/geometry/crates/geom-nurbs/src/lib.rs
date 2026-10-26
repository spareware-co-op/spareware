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
//! Modules today (pieces G56, G58 and G59, The NURBS Book's chapters 2 to 5 and 9, and chapter 12's
//! Bernstein toolkit):
//!
//! - [`Basis`], in [`basis`]: a checked degree and knot vector, and its functions and their
//!   derivatives (A2.1 to A2.5).
//! - [`Curve`], in [`curve`]: curves of any dimension, their points and derivatives (A3.1 to
//!   A3.3, A4.1, A4.2), and points by corner cutting, [`Curve::point_by_corner_cutting`] (A5.2).
//! - [`Surface`], in [`surface`]: surfaces, their points and partial derivatives (A3.5, A3.6,
//!   A4.3, A4.4), and derivative surfaces, [`Surface::derivative_surface`] (A3.7).
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
//! - Fitting (G58), in [`fit`]: the parameters of data points and grids, [`parameters`] and
//!   [`grid_parameters`] (equations 9.4 to 9.6, A9.3); knot vectors for interpolation and least
//!   squares, [`averaged_knots`] and [`least_squares_knots`] (equations 9.8, 9.68 and 9.69); and
//!   what every fit returns, [`CurveFit`] and [`SurfaceFit`], each with its certified error and
//!   the [`FitVersion`] that made it.
//! - Interpolation: [`Curve::interpolate`], [`Curve::interpolate_at`],
//!   [`Curve::interpolate_with_ends`] and [`Surface::interpolate`] (A9.1, section 9.2.2, A9.4).
//! - Least squares, through `geom-linalg`'s complete orthogonal decomposition:
//!   [`Curve::least_squares`] (section 9.4.1), [`Curve::least_squares_at`] with weights and
//!   constraints (A9.6) and [`Surface::least_squares`] (A9.7).
//! - Approximation to a stated error: [`Curve::approximate`] (section 9.4.4, A9.8).
//! - The Bernstein toolkit (G59), in [`bernstein`], [`product`] and [`exact`]: Bezier pieces as
//!   interval enclosures of their exact Bernstein coefficients ([`Curve::bernstein`],
//!   [`Surface::bernstein`]), their boxes, hodographs and derivative boxes, slabs, fat lines and
//!   fat planes, and Bezier clipping ([`BernsteinCurve`], [`BernsteinPatch`]); symbolic products
//!   and the normal patch with collapsed edges divided out ([`Surface::normal_patches`]); and the
//!   exact integer paths for grid pieces ([`exact::plane_coefficients`], [`exact::normal_patch`]).
//! - [`NurbsError`], in [`error`].
//!
//! Status: basis, evaluation and knot operations (G56), fitting (G58) and the Bernstein toolkit
//! (G59) built; the rest (G259) to come.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod approx;
mod arith;
mod band;
pub mod basis;
pub mod bernstein;
mod bezier;
mod budget;
pub mod curve;
mod deboor;
mod elevate;
mod enclose;
pub mod error;
pub mod exact;
pub mod fit;
mod index;
mod insert;
mod interp;
mod lsq;
mod net;
pub mod ops;
pub mod point;
pub mod product;
mod reduce;
mod remove;
pub mod surface;

pub use approx::NEWTON_STEPS;
pub use basis::{Basis, Limit, MAX_DEGREE};
pub use bernstein::{BernsteinCurve, BernsteinPatch, Bounds, Slab};
pub use curve::Curve;
pub use error::NurbsError;
pub use fit::{
    Constraint, CurveFit, FitVersion, KnotMode, Parametrization, SurfaceFit, averaged_knots,
    grid_parameters, least_squares_knots, parameters,
};
pub use ops::{Changed, Direction};
pub use point::Point;
pub use product::Edge;
pub use surface::Surface;
