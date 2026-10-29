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
//! Status: scaffold only; built by pieces G49, G50, G215.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
