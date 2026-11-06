//! Inputs for `geom-linalg`'s hot paths (pieces G49 and G50, decision 32.44): small dense
//! matrices for the column-pivoted QR factorisation and the one-sided Jacobi singular value
//! decomposition.
//!
//! Class `typical` only: random entries in [−1, 1) at 2^−52 resolution, 6 × 6 for the QR and
//! 6 × 4 for the SVD, so a batch of 64 matrices and their factors fits the simulated first-level
//! cache. The factorisations' refusals (a budget too small) are tested in `geom-linalg` itself and
//! pinned by its goldens.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the corpus shape of `geom-linalg`'s `tests/dense_determinism.rs` and
//! `tests/spectral_determinism.rs` (random entries, the factorisation run whole under an unlimited
//! budget), at one size.
//! Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: 36 or 24 draws of [`SplitMix64`] per input, once per batch, outside every measured run.

use geom_linalg::Matrix;

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// An entry in [−1, 1) at 2^−52 resolution.
fn entry(g: &mut SplitMix64) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "the value is below 2^53, so the conversion is exact"
    )]
    let v = (g.next_u64() >> 11) as f64;
    v / (1u64 << 52) as f64 - 1.0
}

/// Random `rows` × `cols` matrices for the operation named `op`.
fn matrices(op: &str, class: Class, rows: usize, cols: usize) -> Vec<Matrix> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(op, class);
    batch_of(|_| {
        let e: Vec<f64> = (0..rows * cols).map(|_| entry(&mut g)).collect();
        Matrix::from_rows(rows, cols, &e).ok()
    })
}

/// 6 × 6 matrices for `PivotedQr::factor`.
#[must_use]
pub fn pivoted_qr(class: Class) -> Vec<Matrix> {
    matrices("linalg::pivoted_qr", class, 6, 6)
}

/// 6 × 4 matrices for `Svd::jacobi`.
#[must_use]
pub fn svd_jacobi(class: Class) -> Vec<Matrix> {
    matrices("linalg::svd_jacobi", class, 6, 4)
}
