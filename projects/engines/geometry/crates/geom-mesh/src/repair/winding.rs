//! Generalised winding numbers of a triangle soup, in intervals: libigl's `solid_angle` and
//! `winding_number`, converted (19.7, reconstruction by winding number).
//!
//! What it does: [`winding_number`] sums, over every triangle, the solid angle the triangle makes
//! at a box, over 4π, in outward-rounded interval arithmetic. [`solid_angle`] is one triangle's
//! term, `atan2(det, den)`, the form of libigl's `solid_angle` (which divides it by 2π at once).
//! A box that holds a point of a triangle has no angle to take; that is `None`, never a guess.
//!
//! Guarantees:
//!
//! - **An enclosure.** The interval holds the winding number at every point of the box, because
//!   every operation rounds outward and `atan2` is the proven one of `geom-num`.
//! - **The sign of the side.** A triangle counts positive from behind (inside, for a mesh that
//!   faces outward) and negative from in front, so a closed outward mesh gives 1 inside, 0 outside.
//! - **Refused, never guessed.** The origin of `atan2` (the box holds a point of the triangle) is
//!   `None`; any other refusal of the arithmetic is an error.
//! - **No recursion, no unsafe code.**
//!
//! Method: Jacobson, Kavan and Sorkine-Hornung, "Robust inside-outside segmentation using
//! generalized winding numbers", ACM Trans. Graph. 32(4), 2013,
//! <https://doi.org/10.1145/2461912.2461916>; the solid angle of a triangle at a point is Van
//! Oosterom and Strackee's, IEEE Trans. Biomed. Eng. 30(2), 1983,
//! <https://doi.org/10.1109/TBME.1983.325207>.
//! Source: PORT from libigl `include/igl/solid_angle.cpp` and `include/igl/winding_number.cpp` at
//!   commit `b41f1944a703653ae0da81e6588650ba14c270ea`, by Alec Jacobson and libigl's authors,
//!   MPL-2.0,
//!   <https://github.com/libigl/libigl/blob/b41f1944a703653ae0da81e6588650ba14c270ea/include/igl/solid_angle.cpp>
//!   <https://github.com/libigl/libigl/blob/b41f1944a703653ae0da81e6588650ba14c270ea/include/igl/winding_number.cpp>
//!   Copyright (C) 2015 Alec Jacobson <alecjacobson@gmail.com>
//!   Changed for Spare Kit: translated to safe `no_std` Rust over `geom-num` intervals instead of
//!   `Eigen` doubles; the grid triangle's corners and the box are exact or enclosed, so every sum
//!   and product rounds outward; the sum over triangles is a plain loop in the order given, charged
//!   to the work meter, without the bounding-volume hierarchy (`WindingNumberAABB`) and its
//!   parallel loop; `atan2` is the proven interval one; a box on a triangle is `None` where the
//!   C++ gives whatever `atan2(0, 0)` gives; the 2D (`signed_angle`) case is left out.
//! SPDX-License-Identifier: MPL-2.0
//!
//! The original notice of `winding_number.cpp`, reproduced as the Mozilla Public License
//! requires (`solid_angle.cpp` carries none):
//!
//! ```text
//! This file is part of libigl, a simple c++ geometry processing library.
//!
//! Copyright (C) 2015 Alec Jacobson <alecjacobson@gmail.com>
//!
//! This Source Code Form is subject to the terms of the Mozilla Public License
//! v. 2.0. If a copy of the MPL was not distributed with this file, You can
//! obtain one at http://mozilla.org/MPL/2.0/.
//! ```
//!
//! Cost: about 60 interval operations a triangle, so O(n) for n triangles. Charges the meter one
//! unit per triangle.

use geom_num::NumError;
use geom_num::grid::P3;
use geom_num::interval::Interval;
use geom_run::WorkMeter;

use super::reconstruct::ReconstructError;

/// A grid coordinate as a double: every coordinate is below 2^40, so this is exact.
#[allow(
    clippy::cast_precision_loss,
    reason = "a coordinate is below 2^40 in size, which a double holds exactly"
)]
fn real(v: i64) -> f64 {
    v as f64
}

/// A winding number over the box `at`: the sum of every triangle's solid angle, over 4π. `None`
/// where the arithmetic refused, which is where the box holds a point of a triangle.
///
/// # Errors
///
/// What the interval arithmetic refuses short of the origin of `atan2`, and a stopped meter.
pub(super) fn winding_number(
    triangles: &[[P3; 3]],
    at: [Interval; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Interval>, ReconstructError> {
    let two_pi = Interval::PI.mul(Interval::point(2.0)?)?;
    let mut sum = Interval::point(0.0)?;
    for t in triangles {
        meter.charge(1)?;
        let Some(angle) = solid_angle(*t, at)? else {
            return Ok(None);
        };
        sum = sum.add(angle.div(two_pi)?)?;
    }
    Ok(Some(sum))
}

/// libigl's `solid_angle`, without its final division by 2π: the angle `atan2(det, den)` that a
/// triangle with corners `t` makes at the box `p`, as an interval. `None` where the box holds a
/// point of the triangle, for which the angle is not defined.
///
/// # Errors
///
/// What the interval arithmetic refuses short of the origin of `atan2`.
pub(super) fn solid_angle(t: [P3; 3], p: [Interval; 3]) -> Result<Option<Interval>, NumError> {
    let vector = |q: P3| -> Result<[Interval; 3], NumError> {
        Ok([
            Interval::point(real(q.x.get()))?.sub(p[0])?,
            Interval::point(real(q.y.get()))?.sub(p[1])?,
            Interval::point(real(q.z.get()))?.sub(p[2])?,
        ])
    };
    let v = [vector(t[0])?, vector(t[1])?, vector(t[2])?];
    let dot3 = |a: [Interval; 3], b: [Interval; 3]| -> Result<Interval, NumError> {
        a[0].mul(b[0])?.add(a[1].mul(b[1])?)?.add(a[2].mul(b[2])?)
    };
    let length = |a: [Interval; 3]| -> Result<Interval, NumError> {
        a[0].square()?
            .add(a[1].square()?)?
            .add(a[2].square()?)?
            .sqrt()
    };
    let (l0, l1, l2) = (length(v[0])?, length(v[1])?, length(v[2])?);
    // The determinant of the three vectors, by the expansion along the first.
    let minor = |a: Interval, b: Interval, c: Interval, d: Interval| a.mul(d)?.sub(b.mul(c)?);
    let det = v[0][0]
        .mul(minor(v[1][1], v[1][2], v[2][1], v[2][2])?)?
        .sub(v[0][1].mul(minor(v[1][0], v[1][2], v[2][0], v[2][2])?)?)?
        .add(v[0][2].mul(minor(v[1][0], v[1][1], v[2][0], v[2][1])?)?)?;
    let den = l0
        .mul(l1)?
        .mul(l2)?
        .add(dot3(v[1], v[2])?.mul(l0)?)?
        .add(dot3(v[2], v[0])?.mul(l1)?)?
        .add(dot3(v[0], v[1])?.mul(l2)?)?;
    match det.atan2(den) {
        Ok(angle) => Ok(Some(angle)),
        Err(NumError::OutsideDomain) => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i64, y: i64, z: i64) -> P3 {
        P3::new(x, y, z).unwrap()
    }

    /// A triangle counts for the side it is seen from: behind it (inside, for an outward mesh) the
    /// angle is positive, in front of it negative, and either way it falls away with distance.
    #[test]
    fn a_triangles_angle_has_the_sign_of_the_side_and_falls_away_with_distance() {
        let t = [p(0, 0, 0), p(1000, 0, 0), p(0, 1000, 0)];
        let at = |z: f64| {
            [
                Interval::new(100.0, 100.0).unwrap(),
                Interval::new(100.0, 100.0).unwrap(),
                Interval::new(z, z).unwrap(),
            ]
        };
        let behind = solid_angle(t, at(-1.0)).unwrap().unwrap();
        let front = solid_angle(t, at(1.0)).unwrap().unwrap();
        let far = solid_angle(t, at(-1.0e6)).unwrap().unwrap();
        // atan2(det, den) is half the solid angle: nearly π/2 times 2 at the surface.
        assert!(behind.lo() > 3.0 && behind.hi() < 3.15, "{behind:?}");
        assert!(front.hi() < -3.0 && front.lo() > -3.15, "{front:?}");
        assert!(far.lo() > 0.0 && far.hi() < 1.0e-3, "{far:?}");
    }
}
