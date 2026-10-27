//! The toolchain and dependency guard's proof (chapter 28.13, "The toolchain guard"; piece G20):
//! a deliberate change to the numeric code the kernel depends on must fail a golden.
//!
//! What it does: the guard itself is `kernel.yml`, which runs every golden on every target and
//! build mode whenever `rust-toolchain.toml`, `Cargo.lock`, the workspace manifest or anything
//! under the kernel changes, so a toolchain or dependency change that moves a result fails there.
//! This module proves that the guard trips. [`perturb_sin`] rewrites `libm`'s `sin` so that every
//! result has its last bit flipped, a one-ulp change no test of a tolerance would see; the binary
//! `guard` builds `geom-num`'s determinism golden against that copy of `libm` (Cargo's
//! `[patch.crates-io]`, given with `--config`) and [`verdict`] reads the run: the golden
//! `geom-num/determinism`, which hashes every function of `geom_num::math::unproven` and so
//! `libm::sin`, must report a digest that differs from the store's.
//!
//! Guarantees:
//!
//! - **The change is one bit, in one function.** [`perturb_sin`] renames `libm`'s one
//!   `pub fn sin(x: f64) -> f64` and puts a function of that name beside it that returns the same
//!   result with bit 0 flipped. It refuses a source with no such function or with two, so a new
//!   `libm` that moves the function fails the proof loudly instead of proving nothing.
//! - **A trip counts only for the right reason.** [`verdict`] accepts a run only when it failed,
//!   Cargo used the patch (it warns when a patch is not used, and that warning is refused), and
//!   the golden printed its line saying its digest differs from the store's. A run that failed to
//!   build, or failed for any other reason, proves nothing and is refused.
//! - **Deterministic.** Both functions are pure: the same text gives the same answer.
//!
//! Method: Cargo's documented `[patch]` table for overriding a dependency with a local copy
//! (<https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html>), set from the
//! command line (<https://doc.rust-lang.org/cargo/reference/config.html#command-line-overrides>),
//! and the seeded-violation pattern of the kernel's merge gate (G12): each gate is shown to fail
//! on a violation planted on purpose.
//!
//! Cost: linear in the length of the source and of the run's output.

use core::fmt;

/// The golden the deliberate change must trip: `geom-num`'s determinism corpus, which hashes every
/// function of `geom_num::math::unproven`, `libm::sin` among them.
pub const TRIPPED_GOLDEN: &str = "geom-num/determinism";

/// The file of `libm`'s crate that holds `sin`, relative to the crate's folder.
pub const SIN_FILE: &str = "src/math/sin.rs";

/// The signature [`perturb_sin`] finds exactly once.
const SIGNATURE: &str = "pub fn sin(x: f64) -> f64 {";

/// The name the original function takes.
const ORIGINAL: &str = "sin_before_the_guard";

/// What Cargo prints when a `[patch]` entry is not used in the build.
const UNUSED_PATCH: &str = "was not used in the crate graph";

/// Why the proof cannot be made, or why a run does not prove the guard trips.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuardError {
    /// `libm`'s source does not hold exactly one `pub fn sin(x: f64) -> f64`: how many it holds.
    Signature(usize),
    /// The run passed: the deliberate change moved no golden.
    NotTripped,
    /// Cargo reports the patch unused, so the run built the real `libm`.
    PatchUnused,
    /// The run failed, but not with [`TRIPPED_GOLDEN`] reporting a digest that differs.
    WrongFailure,
}

impl fmt::Display for GuardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Signature(n) => write!(
                f,
                "libm's {SIN_FILE} holds `{SIGNATURE}` {n} times, not once: update the guard's \
                 probe for this libm"
            ),
            Self::NotTripped => write!(
                f,
                "the run passed with libm's sin changed by one bit: no golden saw the change"
            ),
            Self::PatchUnused => write!(
                f,
                "Cargo did not use the patched libm, so the run proves nothing"
            ),
            Self::WrongFailure => write!(
                f,
                "the run failed, but not with golden {TRIPPED_GOLDEN} reporting a digest that \
                 differs from the store's"
            ),
        }
    }
}

impl std::error::Error for GuardError {}

/// `libm`'s `sin.rs` with the deliberate change: the original `sin` renamed, and a `sin` beside it
/// that returns the original's result with its last bit flipped.
///
/// # Errors
///
/// [`GuardError::Signature`] unless the source holds `pub fn sin(x: f64) -> f64 {` exactly once.
pub fn perturb_sin(source: &str) -> Result<String, GuardError> {
    let found = source.matches(SIGNATURE).count();
    if found != 1 {
        return Err(GuardError::Signature(found));
    }
    let renamed = source.replacen(SIGNATURE, &format!("fn {ORIGINAL}(x: f64) -> f64 {{"), 1);
    Ok(format!(
        "{renamed}\n\
         /// The toolchain guard's deliberate change (geom-test's `guard`, piece G20): the result of\n\
         /// the original `sin` with its last bit flipped.\n\
         pub fn sin(x: f64) -> f64 {{\n    \
         f64::from_bits({ORIGINAL}(x).to_bits() ^ 1)\n\
         }}\n"
    ))
}

/// Whether a run of the golden against the changed `libm` proves the guard trips: `passed` is
/// whether the run succeeded, and `output` its standard output and error together.
///
/// # Errors
///
/// [`GuardError::NotTripped`] if the run passed, [`GuardError::PatchUnused`] if Cargo reports the
/// patch unused, and [`GuardError::WrongFailure`] if the run failed without [`TRIPPED_GOLDEN`]'s
/// line saying its digest differs from the store's.
pub fn verdict(passed: bool, output: &str) -> Result<(), GuardError> {
    if output.contains(UNUSED_PATCH) {
        return Err(GuardError::PatchUnused);
    }
    if passed {
        return Err(GuardError::NotTripped);
    }
    let opening = format!("golden {TRIPPED_GOLDEN} (edition ");
    let tripped = output.lines().any(|line| {
        line.split_once(&opening)
            .is_some_and(|(_, rest)| rest.contains(" differs from the store's "))
    });
    if tripped {
        Ok(())
    } else {
        Err(GuardError::WrongFailure)
    }
}
