//! The intersection acceptance harness: the ssi-corpus run out of process, and section 12's gate
//! (chapter 12.17; piece G160).
//!
//! What it does: the ssi-corpus holds 119 surface/surface cases with exact truth, and a scorer that
//! grades any engine that keeps its runner contract. [`case`] reads a case. [`place`] puts it on the
//! grid, at the smallest scale that keeps its truth. A runner ([`output`]'s contract, the
//! `ssi-runner` binary) answers it. The scorer, which is the corpus's own Python and runs only on
//! CI in a container pinned by digest, gives each answer a verdict. [`gate`] then holds the verdicts
//! and the flagged regions to 12.17, per case and per class. [`catalogue`] rebuilds the published
//! failure lists with inputs of our own, in the corpus's schema, so they run the same way.
//!
//! The kernel has no surface/surface method yet: that comes with G162 onwards. Until it does, the
//! runner answers with [`standin`]s, which echo the truth and perturb it, and those runs prove that
//! the harness passes a right answer and fails a wrong one. The gate in CI passing today says
//! nothing about the kernel. A method plugs in through [`output::Output::of_intersection`], which
//! reads the kernel's own [`geom_intersect::Intersection`].
//!
//! Guarantees:
//!
//! - **Two readings.** The scorer's verdict is checked against [`cover`], this crate's own reading
//!   of coverage, so a scorer that is wrong or misread shows up as a failure of its own.
//! - **Attribution travels.** Every report carries the corpus's credit and each case's source: 25
//!   cases derive from Yang, Jia and Yan 2023, CC BY 4.0.
//! - **Nothing runs on trust of the placement.** A case that cannot be placed with its truth kept
//!   is an error naming what failed, never a case quietly left out.
//!
//! Method: chapter 12.17's oracle list and acceptance gate
//! (`projects/engines/geometry/architecture/12-intersections.md`), and the corpus's runner contract
//! and scorer, <https://github.com/jyviko/ssi-corpus/tree/b04c8a6093a8b3505d865d1dd874ec3879ccb1f9>.
//!
//! Cost: a case's placement is linear in its truth samples, once per scale tried; judging is
//! quadratic in its samples (see [`cover`]).
//!
//! Consulted: ssi-corpus, MIT OR Apache-2.0,
//!   <https://github.com/jyviko/ssi-corpus/tree/b04c8a6093a8b3505d865d1dd874ec3879ccb1f9>, for its
//!   case schema, its runner contract and its scoring rules.

pub mod case;
pub mod catalogue;
pub mod cover;
pub mod gate;
pub mod output;
pub mod place;
pub mod standin;

use core::fmt;

use geom_num::{Real, math};

/// Why the harness could not go on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SsiError {
    /// A file could not be read or written.
    File {
        /// The file.
        path: String,
        /// Why.
        why: String,
    },
    /// A file does not say what it should.
    Malformed {
        /// What was being read.
        what: String,
        /// Why it failed.
        why: String,
    },
    /// A case cannot be placed on the grid with its truth kept.
    Unplaced {
        /// The case.
        id: String,
        /// The check that failed at the last scale tried.
        why: String,
    },
    /// The kernel refused what the adapter built.
    Kernel {
        /// The case or item.
        id: String,
        /// The kernel's error.
        why: String,
    },
}

impl fmt::Display for SsiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SsiError::File { path, why } => write!(f, "{path}: {why}"),
            SsiError::Malformed { what, why } => write!(f, "{what}: {why}"),
            SsiError::Unplaced { id, why } => write!(f, "{id} cannot be placed: {why}"),
            SsiError::Kernel { id, why } => write!(f, "{id}: the kernel refused it: {why}"),
        }
    }
}

impl std::error::Error for SsiError {}

/// The sine of `half` half-turns, by `geom-num`'s `sinpi`, the same on every target.
pub(crate) fn sin_half(half: f64) -> f64 {
    math::sinpi(half).map_or(f64::NAN, Real::get)
}

/// The cosine of `half` half-turns, likewise.
pub(crate) fn cos_half(half: f64) -> f64 {
    math::cospi(half).map_or(f64::NAN, Real::get)
}

/// The sine of `deg` degrees.
pub(crate) fn sin_deg(deg: f64) -> f64 {
    sin_half(deg / 180.0)
}

/// The cosine of `deg` degrees.
pub(crate) fn cos_deg(deg: f64) -> f64 {
    cos_half(deg / 180.0)
}

/// The larger of two lengths; `b` if they cannot be ordered.
pub(crate) fn larger(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

/// The smaller of two lengths; `b` if they cannot be ordered.
pub(crate) fn smaller(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}
