//! Spare Kit geometry kernel: points, vectors, transforms, boxes and frames.
//!
//! This crate sits directly on the numbers layer (`geom-num`) and gives every geometry crate above
//! it the same small vocabulary:
//!
//! - [`vector`]: exact grid vectors [`V2`] and [`V3`] (differences of grid points) with exact dot,
//!   cross and triple products in `i128`, each with its bit budget.
//! - [`float`]: floating-point points and vectors [`F2`] and [`F3`], finite by construction, with
//!   arithmetic in a fixed order and square roots from `geom_num::math` only.
//! - [`typed`]: [`TypedTransform`], the typed transform enum (chapter 7): every transform is
//!   [`GridTransform`] (exact), [`RationalTransform`] (rational) or [`Transform`] (certified), and
//!   applying one to a grid point rounds once and says whether it was exact.
//! - [`grid_transform`]: [`GridTransform`], the exact transforms of the grid (whole-step
//!   translations, quarter turns about the axes, mirrors) as a signed permutation plus an integer
//!   translation.
//! - [`rational`]: [`RationalTransform`], exact rational affine transforms (Pythagorean rotations,
//!   rational scales), applied, composed, inverted and classified exactly.
//! - [`transform`]: [`Transform`], a general affine transform in `f64`, classified so a caller
//!   knows when a kind changes (a circle becomes an ellipse under a non-uniform scale; a mirror
//!   reverses arcs).
//! - [`bbox`]: exact grid boxes [`Box2`] and [`Box3`], and [`FBox3`] in `f64` with outward
//!   rounding.
//! - [`dd`]: double-double arithmetic [`DD`] on the error-free transformations of `geom_num::eft`.
//! - [`frame`]: [`FrameRecord`], the link between a frame and a map projection, with conversion of
//!   map coordinates into frame grid coordinates and back (each rounded once, correctly), exact
//!   shifts between frames on a shared lattice, and the regional lattice of frames.
//! - [`rounded`]: [`Rounded`], the report every rounding to the grid returns: the value and whether
//!   it was exact, made only by this crate's rounding functions.
//!
//! Map: chapter 6.1 (frames), chapter 7 (transforms that change kinds), chapter 17.1 (frames and
//! map projections) and chapter 21 (determinism) of the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Invariants this crate keeps:
//!
//! - **Exact where it decides.** Grid vectors, grid and rational transforms and grid boxes use
//!   integers only, and every product's bit budget is derived in its doc comment. Arithmetic that
//!   could leave its range is checked and returns [`NumError::OutOfRange`](geom_num::NumError)
//!   instead of wrapping.
//! - **The same result in every build profile; defects reported, never answered.** As in the
//!   numbers layer, every integer addition, subtraction, multiplication, division and negation in
//!   this crate's code is a checked method, never a native operator, and no cast loses a value
//!   silently (each cast's bound is written beside it), so nothing depends on overflow checks
//!   being on. A step that its bit budget proves fits can fail only through a defect, and is then
//!   reported as [`NumError::BudgetExceeded`](geom_num::NumError), never replaced by a guessed
//!   value: that is why exact products and classifications return a `Result`. For valid inputs
//!   every such result is `Ok`. Clippy enforces this (`arithmetic_side_effects` and the lossy-cast
//!   lints are denied); tests and the benchmark may use native arithmetic.
//! - **Certified elsewhere.** Floating-point boxes round outward; transform classifications and
//!   orientations are decided with interval arithmetic, never with a guessed tolerance (the only
//!   tolerance used is the ladder's angular bound); conversions to the grid round exactly once and
//!   say whether they were exact.
//! - **Never NaN, never infinite.** Every floating-point value this crate makes is finite; an
//!   operation whose result would not be returns an error.
//! - **Deterministic.** Only IEEE 754 basic operations (which Rust never fuses into a fused
//!   multiply-add), integer arithmetic and the maths of `geom_num::math`. No hash maps, no
//!   randomness, no platform maths, no recursion; every loop is bounded, by a fixed or proven count
//!   or by the length of the caller's own input (`from_points`, [`FrameRecord::home_of`]).
//! - **Safe.** No `unsafe`, no panics, no unchecked indexing.
//!
//! Method: each module names the published method it follows, with its source; together they
//! implement the contracts of chapters 6.1 and 17.1
//! (<https://github.com/spareware-co-op/spareware/blob/piece/G01/projects/engines/geometry/architecture/06-numbers.md>,
//! <https://github.com/spareware-co-op/spareware/blob/piece/G01/projects/engines/geometry/architecture/17-survey-terrain-civil.md>),
//! chapter 7
//! (<https://github.com/spareware-co-op/spareware/blob/piece/G01/projects/engines/geometry/architecture/07-geometry-types.md>)
//! and chapter 21
//! (<https://github.com/spareware-co-op/spareware/blob/piece/G01/projects/engines/geometry/architecture/21-determinism.md>).
//!
//! Cost: every operation runs in constant time and allocates nothing (the crate uses neither `std`
//! nor `alloc`), except `Box2::from_points` and `Box3::from_points`, linear in the points, and
//! [`FrameRecord::home_of`], linear in the operands. Each module states its own costs, including
//! the exact paths that run only when a cheaper floating-point filter cannot decide.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod bbox;
pub mod dd;
pub mod error;
mod exact;
pub mod float;
pub mod frame;
pub mod grid_transform;
pub mod rational;
pub mod rounded;
pub mod transform;
pub mod typed;
pub mod vector;

pub use bbox::{Box2, Box3, FBox3};
pub use dd::DD;
pub use error::{FrameMismatch, MathError};
pub use float::{F2, F3};
pub use frame::{
    CrsId, FrameKind, FrameRecord, FrameShift, LatticeCell, MapPoint, Rotation, Scale,
};
pub use grid_transform::{Axis, GridTransform};
pub use rational::RationalTransform;
pub use rounded::Rounded;
pub use transform::{Classification, Transform, TransformKind};
pub use typed::TypedTransform;
pub use vector::{V2, V3, W3};
