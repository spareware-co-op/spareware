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
//! Status: scaffold only; built by pieces G56, G58, G59, G259.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
