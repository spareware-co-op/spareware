//! Fuzz target: the offset curve keeps the kind contract on any bytes.
//!
//! What it does: [`decode`] reads a fuzzer's bytes exactly as `crate::curves` does and makes the
//! kind an offset (`CurveKind::Offset`, tag [`crate::curves::OFFSET`]): an offset of an ellipse
//! arc, in the plane or in space with a reference direction, built from the case's doubles.
//! [`check`] builds it and holds it to `geom_curve::contract::check` under a meter of [`WORK`]
//! units, and when that check ends on the budget, to
//! `geom_curve::contract::check_without_conversions` under a meter of [`PARTIAL_WORK`], by the
//! `curves` target's own code. [`harness`] is `decode` then `check`. Offsets are 45% of the curves inputs measured on 2026-11-13 and 91.7% of their
//! instructions, so the `curves` target passes them and they are fuzzed here, where the other
//! fourteen kinds take none of their time and they take none of the others'. It reaches pieces
//! G60, G62, G67 and G68 (Gate 2, 32.46).
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** It is the same case the `curves` target decoded from them before
//!   the split, but for the kind: a file saved by either target replays here as the offset it would
//!   have built. The first byte is read and its value set aside.
//! - **What is a finding.** As `crate::curves` says: a law broken with no error behind it, or a
//!   call failing as `CurveError::Defect`, in the contract or in the pass after a budget stop.
//! - **No hang.** The contract runs under [`WORK`] units and the pass after a stop under
//!   [`PARTIAL_WORK`]; running out is a typed stop, which is legal.
//! - **A bounded input.** At most [`MAX_INPUT_BYTES`] bytes, `crate::curves`'s bound.
//!
//! Method: the kind contract of chapter 7 as `geom_curve::contract` states it, driven by
//! coverage-guided fuzzing (chapter 28.5), as `crate::curves`
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: one offset built, one contract check bounded by [`WORK`] work units and, after a budget
//! stop, one pass bounded by [`PARTIAL_WORK`]; at most [`MAX_INPUT_BYTES`] bytes read.

use geom_curve::ParamCurve;
use geom_math::TypedTransform;

use crate::Finding;
use crate::curves::{self, OFFSET, PARAMS};

pub use crate::curves::{Case, Reached};

/// The most bytes [`decode`] reads from any input: 223, `crate::curves`'s bound, as the bytes are
/// read the same way.
pub const MAX_INPUT_BYTES: usize = curves::MAX_INPUT_BYTES;

/// The work units the contract may spend on one offset: 7,000,000.
///
/// Set by the time rule (the fuzz-budgets design note): the slowest stored input, charged this
/// and the pass of [`PARTIAL_WORK`], must take at most 4 s on a hosted runner, a margin of 2.5
/// under the campaign's 10-second limit on one input. The rule could be applied only once the
/// offset path was charged what it costs: on the `curves` target's 2,000,000, which offsets had
/// before they were split out, five stored 3D offsets ran 1,843 to 4,262 instructions a unit
/// where the usual offset runs 316, and issues #852 and #853 priced that path.
///
/// Measured on a hosted runner with the campaign's libFuzzer build, each of the 3,856 stored
/// curves inputs run once as an offset (run 43564862778): at 8,000,000 the slowest took 4.31 s
/// (`36a4848b`) and the next 3.85 s (`44fc6556`), every other 2.87 s or less. Both spend the
/// whole budget and stop in the NURBS law, and their release instructions scale with it
/// (2.64 thousand million at 8,000,000, 2.31 at 7,000,000, for `36a4848b`), so 8,000,000 broke
/// the rule and 7,500,000 would put `36a4848b` at about 4.04 s. At 7,000,000 the five slowest,
/// each run three times, took at most MEASURED_CURVES.
///
/// What it buys: on the 1,626 stored offsets of 2026-11-13 (on the charges before #852), 4,000,000
/// took the offsets that stop from 553 at the old budget to 313 and 8,000,000 to 122; this budget
/// lies between.
pub const WORK: u64 = 7_000_000;

/// The work units of the pass without conversions after a budget stop: `crate::curves`'s,
/// derived there on the offsets.
pub const PARTIAL_WORK: u64 = curves::PARTIAL_WORK;

/// Any bytes as a case, the kind made an offset, and how many bytes were read.
#[must_use]
pub fn decode(bytes: &[u8]) -> (Case, usize) {
    let (case, read) = curves::decode(bytes);
    (
        Case {
            kind: OFFSET,
            ..case
        },
        read,
    )
}

/// The kind contract on `kind` at `params` (fractions k/255 of its domain) under `transform`,
/// and after a budget stop the pass without conversions, on this target's budgets: [`check`] on a
/// kind handed in, so tests can show it catches a broken one.
///
/// # Errors
///
/// As `crate::curves::check_with`.
pub fn check_with<K: ParamCurve<D> + core::fmt::Debug, const D: usize>(
    kind: &K,
    params: &[u8; PARAMS],
    transform: &TypedTransform,
) -> Result<(), Finding> {
    curves::contract_on(kind, *params, transform, (WORK, PARTIAL_WORK)).map(drop)
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
    curves::reached_on(
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
