//! Spare Kit geometry kernel: the numbers layer.
//!
//! Everything the kernel decides about shape rests on the numbers in this crate. Its one job is
//! numbers: where coordinates live, how "which side is this on" is answered exactly, how curved
//! computations carry proven bounds, where maths functions come from, and the tolerance ladder.
//!
//! Map: chapter 6 of the kernel architecture (`projects/engines/geometry/architecture/06-numbers.md`),
//! with the numeric parts of chapters 9 (exactness), 21 (determinism) and 28 (testing).
//!
//! Invariants this crate keeps:
//!
//! - Every grid coordinate is a whole number of grid steps with magnitude at most [`grid::COORD_MAX`]
//!   (2^40 − 1). Every exact predicate's bit budget assumes this, and each predicate's doc comment
//!   carries its derivation.
//! - Every predicate returns a [`sign::Sign`], never a number, and is exact: no rounding decides a sign.
//! - Arithmetic on fixed-width integers is checked. A result that does not fit is `None`, never a
//!   silently wrapped value.
//! - No `unsafe`, no panics outside tests, no platform maths: every maths function comes from
//!   [`math`], which uses the pure-Rust `libm` crate, so results are identical on every target.
//! - No floating-point value produced here is ever NaN.
#![cfg_attr(not(test), no_std)]

pub mod eft;
pub mod error;
pub mod exactness;
pub mod grid;
pub mod interval;
pub mod ladder;
pub mod math;
pub mod plane;
pub mod predicate;
pub mod real;
pub mod sign;
pub mod wide;

pub use error::NumError;
pub use exactness::Exactness;
pub use grid::{Coord, GridStep, P2, P3, Rounded, Steps};
pub use sign::Sign;
pub use wide::{I256, I320, I384, I512, Wide};
