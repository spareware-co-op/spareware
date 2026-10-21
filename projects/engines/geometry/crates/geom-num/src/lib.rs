//! Spare Kit geometry kernel: the numbers layer.
//!
//! Everything the kernel decides about shape rests on the numbers in this crate. Its one job is
//! numbers: where coordinates live, how "which side is this on" is answered exactly, how curved
//! computations carry proven bounds, where maths functions come from, and the tolerance ladder.
//!
//! Map: chapter 6 of the kernel architecture
//! (`projects/engines/geometry/architecture/06-numbers.md`), with the numeric parts of chapters 9
//! (exactness), 21 (determinism) and 28 (testing).
//!
//! Invariants this crate keeps:
//!
//! - Every grid coordinate is a whole number of grid steps with magnitude at most
//!   [`grid::COORD_MAX`] (2^40 − 1). Every exact predicate's bit budget assumes this, and each
//!   predicate's doc comment carries its derivation, whose arithmetic is asserted when the crate
//!   compiles.
//! - Every predicate returns a [`sign::Sign`], never a number, and is exact: no rounding decides a
//!   sign. A computation that cannot complete is reported as [`NumError::BudgetExceeded`], never
//!   answered; no input can cause one, only a defect in this crate.
//! - A [`plane::Plane`] and a [`plane::Tpi`] check their bit budgets when they are made, so every
//!   computation on them fits its type.
//! - Committing a three-plane point to the grid ([`plane::Tpi::to_grid`]) rounds each coordinate
//!   to the nearest step, ties to even, moving it by at most half a step
//!   ([`ladder::COMMIT_PER_AXIS`]), and says whether it moved.
//! - Integer arithmetic on fixed-width values in kernel code is a checked method (`checked_*`, or
//!   in [`wide`] the `overflowing_*` limb steps, whose carry or borrow is kept), so nothing depends
//!   on overflow checks being on: a result that does not fit is `None` or an error, never a
//!   silently wrapped value. Only the compiler's evaluation of constants uses native operators,
//!   where an overflow stops the build. Counts of steps never saturate: a saturated bound would
//!   understate the truth.
//! - No floating-point value produced here from finite inputs is NaN. [`math`] returns a
//!   [`real::Real`] or an error; an [`interval::Interval`] has finite bounds or is refused as
//!   `NotFinite`; [`eft`] reports a pair that is not exact with an infinite second part.
//! - Every interval operation encloses the exact result of its operation on any reals in its
//!   operands, and an enclosure that needs a maths function without a proven error is refused
//!   ([`NumError::UnprovenMaths`]).
//! - A bound carries its grid step, and bounds add exactly in the finer step
//!   ([`exactness::Exactness`]).
//! - No `unsafe`, no panics outside tests, no platform maths: every maths function comes from
//!   [`math`], which uses the pure-Rust `libm` crate; where `libm` uses a processor instruction
//!   instead (the square root, and the fused multiply-add where the processor has one), it is one
//!   IEEE 754 defines exactly, so results are identical on every target.
//!
//! Licences of the published crates this crate depends on (DEPEND, chapter 29.1), as each one's
//! manifest at the version in `Cargo.lock` states it:
//!
//! - MIT: `libm`.
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
pub use real::Real;
pub use sign::Sign;
pub use wide::{I256, I320, I384, I512, Wide};
