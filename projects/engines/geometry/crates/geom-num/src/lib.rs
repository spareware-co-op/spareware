//! Spare Kit geometry kernel: the numbers layer.
//!
//! Everything the kernel decides about shape rests on the numbers in this crate. Its one job is
//! numbers: where coordinates live, how "which side is this on" is answered exactly, how curved
//! computations carry proven bounds, where maths functions come from, where a randomised method's
//! random numbers come from (a counter-based generator keyed by explicit seeds), and the tolerance
//! ladder.
//!
//! Layer: 1, Numbers and the contract (chapter 27's proposed layer table).
//!
//! Map: chapter 6 of the kernel architecture
//! (`projects/engines/geometry/architecture/06-numbers.md`), with the numeric parts of chapters 9
//! (exactness), 21 (determinism) and 28 (testing), and the random generator chapters 16
//! (free-form) and 17 (survey, terrain and civil) ask of this crate ([`rng`], the one generator of
//! the randomised-method rule in the kernel's `AGENTS.md`).
//!
//! Invariants this crate keeps:
//!
//! - Every grid coordinate is a whole number of grid steps with magnitude at most
//!   [`grid::COORD_MAX`] (2^40 − 1). Every exact predicate's bit budget assumes this, and its
//!   module carries the written derivation (for the explicit-point predicates, in [`predicate`]'s
//!   header), whose arithmetic is asserted when the crate compiles.
//! - Every predicate returns a [`sign::Sign`], never a number, and is exact: no rounding decides a
//!   sign. A computation that cannot complete is reported as [`NumError::BudgetExceeded`], never
//!   answered; no input can cause one, only a defect in this crate.
//! - A [`plane::Plane`] and a [`plane::Tpi`] check their bit budgets when they are made, so every
//!   computation on them fits its type.
//! - Committing a three-plane point to the grid ([`plane::Tpi::to_grid`]) rounds each coordinate
//!   to the nearest step, ties to even, moving it by at most half a step
//!   ([`ladder::COMMIT_PER_AXIS`]), and says whether it moved.
//! - Integer arithmetic on fixed-width values in kernel code is a checked method (`checked_*`, or
//!   in [`wide`] and [`wide32`] the `overflowing_*` limb steps, whose carry or borrow is kept), so
//!   nothing depends on overflow checks being on: a result that does not fit is `None` or an error,
//!   never a silently wrapped value. Three kinds of step compute modulo a power of two on purpose,
//!   and each cites what defines it so. In [`rng`], Philox's multiplications and key schedule,
//!   modulo 2^32 (its 32 × 32-bit product is formed modulo 2^64, as its reference does, and never
//!   reaches 2^64), and Lemire's threshold, a negation modulo 2^64. In the large accumulator inside
//!   [`exact_sum::ExactSum::add_slice`] and [`exact_sum::ExactSum::add_iter`], the sum of a
//!   double's whole bit pattern modulo 2^64, as Neal's method specifies, from which the exact sum
//!   of the significands is recovered. In those two a `wrapping_*` method is the specified
//!   operation, not a result that failed to fit. And in the CORE-MATH ports of [`math`], a
//!   `wrapping_*` method where the C source computes modulo 2^64 or 2^128 on purpose, each with a
//!   comment saying why the C does it (a two's complement negation, a sum of which only the low
//!   bits are read, or a limb sum whose carry is kept by comparison as [`wide`] keeps its own), so
//!   a wrap there is the computation the proof describes, never a lost value. Every other integer
//!   step in the ports is checked too: its `None`, which each step proves cannot come, ends the
//!   computation with the canonical NaN, which the public functions report as `NotFinite`. Only the
//!   compiler's evaluation of constants uses native operators, where an overflow stops the build.
//!   Counts of steps never saturate: a saturated bound would understate the truth.
//! - No floating-point value produced here from finite inputs is NaN. [`math`] returns a
//!   [`real::Real`] or an error; an [`interval::Interval`] has finite bounds or is refused as
//!   `NotFinite`; [`eft`] reports a pair that is not exact with an infinite second part; a
//!   [`dd::DD`] is finite by construction; [`exact_sum::ExactSum`] gives a `Real` or `NotFinite`;
//!   [`rng`] draws its doubles as `Real`s in [0, 1); [`brent`]'s searches and [`quadrature`]'s
//!   integrals return `Real`s, and stop at a caller's cap with an error, never a best guess; every
//!   operation of [`scalar::Scalar`] on a `Real`, and so of a [`dual::Dual`] over it, checks its
//!   result, so a NaN or an infinity in a value or a derivative is `NotFinite` (the NaN tripwire
//!   of chapter 15.4); [`newton`]'s operators return enclosures with finite bounds or an
//!   error; a [`ball::Ball`] has a finite double-double centre and a finite radius, or the
//!   operation is `NotFinite`; and [`isolate`] returns enclosures and regions with finite bounds,
//!   or an error. Inside [`math`], where the ports compute on
//!   raw doubles, any NaN is the canonical `0x7ff8_0000_0000_0000`, and no public function
//!   returns one.
//! - Every interval operation encloses the exact result of its operation on any reals in its
//!   operands, and an enclosure that needs a maths function without a proven error is refused
//!   ([`NumError::UnprovenMaths`]).
//! - A bound carries its grid step, and bounds add exactly in the finer step
//!   ([`exactness::Exactness`]).
//! - Every double-double operation ([`dd::DD`]) is within its stated error bound of the exact
//!   result: the published, machine-checked bound of its algorithm (the theorem is named on each
//!   operation), plus, from the range of binary64, 2^−398 relative for products, quotients and
//!   roots and 2^−1074 absolute for products and quotients. A `DD` is never NaN or infinite: an
//!   overflow is [`NumError::NotFinite`].
//! - A sum of doubles taken with [`exact_sum::ExactSum`] is exact until it is rounded once, to
//!   nearest with ties to even, so its bits depend only on the terms, never on their order, on how
//!   they were split into chunks, or on the order of merges.
//! - No `unsafe`, no panics outside tests, no platform maths: every maths function comes from
//!   [`math`]. `sinpi`, `cospi`, `exp`, `ln` and `atan2` are correctly rounded CORE-MATH ports
//!   with proven error bounds, so their bits are fixed by their specification. Everything else
//!   comes from the pure-Rust `libm` crate, the same code on every target; where `libm` uses a
//!   processor instruction instead (the square root, and the fused multiply-add where the
//!   processor has one), it is one IEEE 754 defines exactly, so results are identical on every
//!   target. The `libm` functions without a proven bound are kept apart in [`math::unproven`],
//!   and nothing in this crate uses them.
//!
//! Licences of the published crates this crate depends on (DEPEND, chapter 29.1), as each one's
//! manifest at the version in `Cargo.lock` states it:
//!
//! - MIT: `libm`.
#![cfg_attr(not(test), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

// The work list of the root-isolation engine ([`isolate`]) grows with the system, so it is a
// vector, reserved fallibly and charged to the caller's logical-memory budget (chapter 23.10).
extern crate alloc;

pub mod ball;
pub mod brent;
pub mod crossing;
pub mod dd;
pub mod dual;
pub mod eft;
pub mod error;
pub mod exact_sum;
pub mod exactness;
pub mod generated;
pub mod grid;
pub mod interval;
pub mod isolate;
pub mod ladder;
pub mod math;
pub mod newton;
pub mod perturb;
pub mod plane;
pub mod predicate;
pub mod quadrature;
pub mod quadrature_rules;
pub mod quadric;
pub mod real;
pub mod rng;
pub mod scalar;
pub mod sign;
pub mod sturm;
pub mod wide;
pub mod wide32;

pub use dd::DD;
pub use error::NumError;
pub use exact_sum::ExactSum;
pub use exactness::Exactness;
pub use grid::{Coord, GridStep, P2, P3, Rounded, Steps};
pub use real::Real;
pub use sign::Sign;
pub use wide::{I256, I320, I384, I512, Wide};
pub use wide32::Wide32;
