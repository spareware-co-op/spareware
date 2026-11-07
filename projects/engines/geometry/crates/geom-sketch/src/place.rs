// SPDX-License-Identifier: Apache-2.0
//! Placing a rigid cluster: the rigid motion that best carries its solved shape onto where it was
//! drawn, in any dimension.
//!
//! What it does: a cluster is solved in its own frame (`local` positions). [`kabsch`] finds the
//! rotation and translation that bring those positions closest, in the least-squares sense, to the
//! positions the person drew (`drawn`), so geometry the person did not constrain stays where it
//! was. [`place`] applies the ground rule first: a cluster that holds the ground is solved in the
//! sketch's own frame and is not moved.
//!
//! Guarantees:
//!
//! - **A proper rotation.** The result has determinant +1 for every N: when the best orthogonal fit
//!   is a reflection, the direction of the smallest singular value is flipped (Umeyama's
//!   correction), so a cluster is never mirrored.
//! - **Deterministic.** Sums run in a fixed order over the points as given, and the decomposition
//!   is `geom-linalg`'s one-sided Jacobi SVD, which is bit-identical on every target.
//! - **Honest about ambiguity.** The singular values of the correlation are returned; when more
//!   than one of them is zero (the points lie on a line in space, or on a point), the rotation
//!   about the missing directions is not determined by the data, and the caller decides.
//! - **No partial result.** Mismatched lengths, no points or a non-finite coordinate is an error and
//!   nothing is placed.
//!
//! Method: W. Kabsch, "A solution for the best rotation to relate two sets of vectors", Acta
//! Crystallographica A32 (1976), doi:10.1107/S0567739476001873, with the determinant correction of
//! S. Umeyama, "Least-squares estimation of transformation parameters between two point patterns",
//! IEEE TPAMI 13(4) (1991), doi:10.1109/34.88573: centre both sets, take the SVD of their
//! correlation H = U S Vᵀ and set R = V diag(1, …, 1, ±1) Uᵀ, the sign that makes det R = +1.
//!
//! Cost: O(kN² + N³) for k points in N dimensions; O(N²) memory. No recursion.

extern crate alloc;

use alloc::vec::Vec;

use geom_linalg::matrix::{Matrix, Work};
use geom_linalg::solve::Lu;
use geom_linalg::svd::Svd;

use crate::model::SketchError;

/// A rigid motion of N-space: `p ↦ rotation · p + translation`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rigid<const N: usize> {
    /// The rotation, by rows; a proper orthogonal matrix.
    pub rotation: [[f64; N]; N],
    /// The translation.
    pub translation: [f64; N],
    /// The singular values of the points' correlation, largest first: how well the data fixes each
    /// direction of the rotation.
    pub singular_values: [f64; N],
}

impl<const N: usize> Rigid<N> {
    /// The image of `p`.
    #[must_use]
    pub fn apply(&self, p: &[f64; N]) -> [f64; N] {
        let mut out = self.translation;
        for (o, row) in out.iter_mut().zip(&self.rotation) {
            let mut sum = 0.0;
            for (r, x) in row.iter().zip(p) {
                sum += r * x;
            }
            *o += sum;
        }
        out
    }
}

/// Where a cluster goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement<const N: usize> {
    /// Nowhere: the cluster holds the ground and was solved in the sketch's frame.
    Frame,
    /// Carried by this motion from its own frame to the sketch's.
    Moved(Rigid<N>),
}

fn get<T: Copy>(list: &[T], i: usize) -> Result<T, SketchError> {
    list.get(i).copied().ok_or(SketchError::Internal)
}

fn centroid<const N: usize>(points: &[[f64; N]]) -> Result<[f64; N], SketchError> {
    let count = f64::from(u32::try_from(points.len()).map_err(|_| SketchError::Internal)?);
    let mut sum = [0.0; N];
    for p in points {
        for (s, x) in sum.iter_mut().zip(p) {
            if !x.is_finite() {
                return Err(SketchError::NotFinite);
            }
            *s += x;
        }
    }
    for s in &mut sum {
        *s /= count;
    }
    Ok(sum)
}

/// The sign of the determinant of the N × N matrix `m` (row-major), by LU: the product of the
/// pivots' signs times the permutation's parity.
fn determinant_sign<const N: usize>(m: &[f64], work: &mut Work) -> Result<f64, SketchError> {
    let a = Matrix::from_rows(N, N, m)?;
    let lu = Lu::factor(&a, work)?;
    let mut negative = false;
    for i in 0..N {
        negative ^= lu.factors().get(i, i)? < 0.0;
    }
    // Parity of the row permutation: N minus its number of cycles.
    let perm = lu.permutation();
    let mut seen = alloc::vec![false; N];
    let mut cycles = 0_usize;
    for start in 0..N {
        if get(&seen, start)? {
            continue;
        }
        cycles = cycles.checked_add(1).ok_or(SketchError::Internal)?;
        let mut at = start;
        while !get(&seen, at)? {
            *seen.get_mut(at).ok_or(SketchError::Internal)? = true;
            at = get(perm, at)?;
        }
    }
    let swaps = N.checked_sub(cycles).ok_or(SketchError::Internal)?;
    negative ^= swaps.checked_rem(2) == Some(1);
    Ok(if negative { -1.0 } else { 1.0 })
}

/// The rigid motion that best carries `local` onto `drawn`, point for point.
///
/// # Errors
///
/// [`SketchError::Internal`] if the two lists differ in length or are empty;
/// [`SketchError::NotFinite`] for a coordinate that is NaN or infinite; [`SketchError::Linear`] if
/// the decomposition fails or runs out of `work`.
pub fn kabsch<const N: usize>(
    local: &[[f64; N]],
    drawn: &[[f64; N]],
    work: &mut Work,
) -> Result<Rigid<N>, SketchError> {
    if local.len() != drawn.len() || local.is_empty() || N == 0 {
        return Err(SketchError::Internal);
    }
    let (ca, cb) = (centroid(local)?, centroid(drawn)?);
    // H[i][j] = sum over points of (a - ca)_i (b - cb)_j.
    let cells = N.checked_mul(N).ok_or(SketchError::Internal)?;
    let mut h: Vec<f64> = alloc::vec![0.0; cells];
    for (a, b) in local.iter().zip(drawn) {
        let da: Vec<f64> = a.iter().zip(&ca).map(|(x, c)| x - c).collect();
        let db: Vec<f64> = b.iter().zip(&cb).map(|(x, c)| x - c).collect();
        for (i, x) in da.iter().enumerate() {
            for (j, y) in db.iter().enumerate() {
                let at = i.checked_mul(N).and_then(|r| r.checked_add(j));
                *at.and_then(|k| h.get_mut(k)).ok_or(SketchError::Internal)? += x * y;
            }
        }
    }
    let svd = Svd::jacobi(&Matrix::from_rows(N, N, &h)?, work)?;
    // R0 = V Uᵀ; its determinant decides the sign of the last column of V.
    let (u, v) = (svd.u(), svd.v());
    let product = |flip: f64| -> Result<Vec<f64>, SketchError> {
        let mut r = alloc::vec![0.0; cells];
        for i in 0..N {
            for j in 0..N {
                let mut sum = 0.0;
                for k in 0..N {
                    let scale = if k.checked_add(1) == Some(N) { flip } else { 1.0 };
                    sum += v.get(i, k)? * scale * u.get(j, k)?;
                }
                let at = i.checked_mul(N).and_then(|row| row.checked_add(j));
                *at.and_then(|k| r.get_mut(k)).ok_or(SketchError::Internal)? = sum;
            }
        }
        Ok(r)
    };
    let plain = product(1.0)?;
    let flip = determinant_sign::<N>(&plain, work)?;
    let r = if flip < 0.0 { product(-1.0)? } else { plain };
    let mut rotation = [[0.0; N]; N];
    for (i, row) in rotation.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            let at = i.checked_mul(N).and_then(|k| k.checked_add(j));
            *cell = at
                .and_then(|k| r.get(k).copied())
                .ok_or(SketchError::Internal)?;
        }
    }
    let mut translation = cb;
    for (t, row) in translation.iter_mut().zip(&rotation) {
        let mut sum = 0.0;
        for (m, x) in row.iter().zip(&ca) {
            sum += m * x;
        }
        *t -= sum;
    }
    let mut singular_values = [0.0; N];
    for (s, x) in singular_values.iter_mut().zip(svd.singular_values()) {
        *s = *x;
    }
    Ok(Rigid {
        rotation,
        translation,
        singular_values,
    })
}

/// Where a cluster goes: nowhere if it is `grounded`, else the best-fit motion of [`kabsch`].
///
/// # Errors
///
/// As [`kabsch`], for a cluster that is not grounded.
pub fn place<const N: usize>(
    grounded: bool,
    local: &[[f64; N]],
    drawn: &[[f64; N]],
    work: &mut Work,
) -> Result<Placement<N>, SketchError> {
    if grounded {
        return Ok(Placement::Frame);
    }
    kabsch(local, drawn, work).map(Placement::Moved)
}

#[cfg(test)]
mod tests {
    use geom_linalg::matrix::Work;

    use super::{Placement, Rigid, kabsch, place};

    fn work() -> Work {
        Work::new(u64::MAX)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn a_rotation_and_a_shift_in_the_plane_are_recovered() {
        // A quarter turn then (5, -2).
        let local = [[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [0.0, 3.0]];
        let drawn: Vec<[f64; 2]> = local.iter().map(|p| [5.0 - p[1], -2.0 + p[0]]).collect();
        let r: Rigid<2> = kabsch(&local, &drawn, &mut work()).unwrap();
        for (p, q) in local.iter().zip(&drawn) {
            let image = r.apply(p);
            assert!(close(image[0], q[0]) && close(image[1], q[1]), "{image:?}");
        }
    }

    #[test]
    fn a_mirror_image_is_fitted_by_a_rotation_not_a_reflection() {
        // The drawn points are the local ones mirrored: no rotation fits them, and the answer must
        // still be a rotation (det +1), at a larger residual.
        let local = [[0.0, 0.0], [3.0, 0.0], [0.0, 1.0]];
        let drawn = [[0.0, 0.0], [-3.0, 0.0], [0.0, 1.0]];
        let r = kabsch(&local, &drawn, &mut work()).unwrap();
        let det = r.rotation[0][0] * r.rotation[1][1] - r.rotation[0][1] * r.rotation[1][0];
        assert!(close(det, 1.0), "{det}");
    }

    #[test]
    fn three_dimensions_work_the_same_way() {
        // A half turn about z then a lift.
        let local = [
            [1.0, 0.0, 0.0],
            [0.0, 2.0, 0.0],
            [0.0, 0.0, 3.0],
            [1.0, 1.0, 1.0],
        ];
        let drawn: Vec<[f64; 3]> = local.iter().map(|p| [-p[0], -p[1], p[2] + 4.0]).collect();
        let r = kabsch(&local, &drawn, &mut work()).unwrap();
        for (p, q) in local.iter().zip(&drawn) {
            let image = r.apply(p);
            assert!(image.iter().zip(q).all(|(a, b)| close(*a, *b)), "{image:?}");
        }
    }

    #[test]
    fn the_ground_is_never_moved_and_bad_input_places_nothing() {
        let a = [[0.0, 0.0], [1.0, 0.0]];
        assert_eq!(place(true, &a, &a, &mut work()), Ok(Placement::Frame));
        assert!(place(false, &a, &a[..1], &mut work()).is_err());
        assert!(place::<2>(false, &[], &[], &mut work()).is_err());
        let bad = [[f64::NAN, 0.0], [1.0, 0.0]];
        assert!(place(false, &a, &bad, &mut work()).is_err());
    }
}
