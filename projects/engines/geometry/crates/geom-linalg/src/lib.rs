//! Spare Kit geometry kernel: deterministic linear algebra.
//!
//! One job: Deterministic dense and sparse linear algebra: QR, the complete orthogonal
//! decomposition, triangular solves, column ordering.
//!
//! Layer: 1, Numbers and the contract (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Bit-identical results on every target and thread count: plain scalar loops in a fixed order,
//!   no SIMD chosen at run time, no `mul_add`, and no threads inside one factorisation (15.6, 21).
//! - Least-squares steps come from QR or the complete orthogonal decomposition, never from the
//!   normal equations, which square the condition number (15.6).
//! - The least-change step is the limit of the damped steps as damping goes to zero, so an
//!   under-determined system never jumps to a pivot-order basic solution (15.6).
//! - Column ordering follows COLAMD (Davis, Gilbert, Larimore and Ng 2004), written in safe Rust,
//!   with ties broken by variable order (15.6).
//! - No platform maths: every maths function comes from `geom_num::math` (21).
//!
//! Sections: 15.6 and 16.6; open questions OQ-15.7, OQ-16.1 and OQ-29.16; chapter 27 ("Who owns
//! what"), in the kernel architecture (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Modules today (piece G49, dense decompositions):
//!
//! - [`matrix`]: [`Matrix`], dense and finite; [`LinalgError`]; and [`Work`], the budget every
//!   factorisation charges before it starts.
//! - [`solve`]: LU with partial pivoting, Cholesky, triangular and tridiagonal solves.
//! - [`qr`]: Householder QR with column pivoting, the complete orthogonal decomposition, the
//!   least-norm least-squares solution and the canonical null space.
//!
//! Status: dense decompositions built (G49); singular values and eigenvalues (G50) and the sparse
//! parts (G215) to come.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod matrix;
pub mod qr;
pub mod solve;

pub use matrix::{LinalgError, Matrix, Work};
pub use qr::{Cod, PivotedQr, canonical_null_space};
pub use solve::{Cholesky, Lu, solve_lower, solve_tridiagonal, solve_upper};
