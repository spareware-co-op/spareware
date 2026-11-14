//! Fuzz target: the offset surface keeps the kind contract on any bytes.
//!
//! What it does: [`decode`] reads a fuzzer's bytes exactly as `crate::surfaces` does and makes
//! the kind an offset (`SurfaceKind::Offset`, tag [`crate::surfaces::OFFSET`]): an offset of a
//! cylinder, built from the case's doubles. [`check`] builds it and holds it to
//! `geom_surface::contract::check` under a meter of [`WORK`] units, and when that check ends on
//! the budget, to `geom_surface::contract::check_without_conversions` under a meter of
//! [`PARTIAL_WORK`], by the `surfaces` target's own code. [`harness`] is `decode` then `check`.
//! Offsets are 56% of the surfaces inputs measured on 2026-11-13 and 99.2% of their
//! instructions, so the `surfaces` target passes them and they are fuzzed here, where the other
//! ten kinds take none of their time and they take none of the others'. It reaches pieces G60
//! and G66 to G68 (Gate 2, 32.46).
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** It is the same case the `surfaces` target decoded from them before
//!   the split, but for the kind: a file saved by either target replays here as the offset it
//!   would have built. The first byte is read and its value set aside.
//! - **What is a finding.** As `crate::surfaces` says: a law broken with no error behind it, or a
//!   call failing as `CurveError::Defect`, in the contract or in the pass after a budget stop.
//! - **No hang.** The contract runs under [`WORK`] units and the pass after a stop under
//!   [`PARTIAL_WORK`]; running out is a typed stop, which is legal.
//! - **A bounded input.** At most [`MAX_INPUT_BYTES`] bytes, `crate::surfaces`'s bound.
//!
//! Method: the kind contract of chapter 7 as `geom_surface::contract` states it, driven by
//! coverage-guided fuzzing (chapter 28.5), as `crate::surfaces`
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: one offset built, one contract check bounded by [`WORK`] work units and, after a budget
//! stop, one pass bounded by [`PARTIAL_WORK`]; at most [`MAX_INPUT_BYTES`] bytes read.

use geom_math::TypedTransform;
use geom_surface::ParamSurface;

use crate::Finding;
use crate::surfaces::{self, OFFSET, PARAMS};

pub use crate::surfaces::{Case, Reached};

/// The most bytes [`decode`] reads from any input: 298, `crate::surfaces`'s bound, as the bytes
/// are read the same way.
pub const MAX_INPUT_BYTES: usize = surfaces::MAX_INPUT_BYTES;

/// The work units the contract may spend on one offset: 4,000,000.
///
/// The `surfaces` target's budget, which offsets had there before they were split out, and kept.
/// The contract builds an offset's NURBS cache twice (for the transform law and for the NURBS
/// law), so an offset whose contract completes charges 2,246,820 units at the median and
/// 3,955,108 at the most measured: this budget is 1.8 times an ordinary offset, and a quarter of
/// the offsets stop on it. The pass of [`PARTIAL_WORK`] then runs every law that needs no
/// conversion. A stop costs 1.0 to 1.96 s on a hosted runner (the 2026-11-12 campaign).
pub const WORK: u64 = surfaces::WORK;

/// The work units of the pass without conversions after a budget stop: `crate::surfaces`'s,
/// derived there on the offsets.
pub const PARTIAL_WORK: u64 = surfaces::PARTIAL_WORK;

/// Any bytes as a case, the kind made an offset, and how many bytes were read.
#[must_use]
pub fn decode(bytes: &[u8]) -> (Case, usize) {
    let (case, read) = surfaces::decode(bytes);
    (
        Case {
            kind: OFFSET,
            ..case
        },
        read,
    )
}

/// The kind contract on `kind` at `params` (fractions k/255 of each side of its domain) under
/// `transform`, and after a budget stop the pass without conversions, on this target's budgets:
/// [`check`] on a kind handed in, so tests can show it catches a broken one.
///
/// # Errors
///
/// As `crate::surfaces::check_with`.
pub fn check_with<K: ParamSurface + core::fmt::Debug>(
    kind: &K,
    params: &[(u8, u8); PARAMS],
    transform: &TypedTransform,
) -> Result<(), Finding> {
    surfaces::contract_on(kind, *params, transform, (WORK, PARTIAL_WORK)).map(drop)
}

/// Builds the case's offset, whatever its kind tag, and holds it to the kind contract (module
/// documentation).
///
/// # Errors
///
/// As [`check_with`].
pub fn check(case: &Case) -> Result<(), Finding> {
    reached(case).map(drop)
}

/// [`check`], and how far the contract got.
///
/// # Errors
///
/// As [`check_with`].
pub fn reached(case: &Case) -> Result<Reached, Finding> {
    surfaces::reached_on(
        &Case {
            kind: OFFSET,
            ..case.clone()
        },
        (WORK, PARTIAL_WORK),
    )
}

/// [`decode`], then [`check`].
///
/// # Errors
///
/// As [`check`].
pub fn harness(bytes: &[u8]) -> Result<(), Finding> {
    check(&decode(bytes).0)
}
