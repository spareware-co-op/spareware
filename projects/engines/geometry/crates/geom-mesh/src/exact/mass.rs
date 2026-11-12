//! Volume and area of an exact mesh (19.2, "Mass properties").
//!
//! What it does: [`ExactMesh::volume`] gives six times the volume, as an exact integer while every
//! vertex is a grid point and as a certified interval otherwise. [`ExactMesh::twice_area_vector`]
//! gives a grid face's exact doubled area vector, and [`ExactMesh::area`] its area as a certified
//! interval, a square root having no exact value.
//!
//! Guarantees:
//!
//! - **Exact on the grid.** Six times the volume is a sum of integer determinants, each below 2^123
//!   relative to the frame's origin, so at most 2^32 of them stay inside [`I256`], and every step
//!   is checked.
//! - **Rounded outward off it.** A three-plane vertex's coordinates are exact fractions, enclosed
//!   in intervals rounded outward, and so is every sum and product of them, so the true volume is
//!   inside the interval.
//! - **The sign says which way.** A closed mesh with outward normals has a positive volume.
//!
//! Method: the divergence theorem: the volume inside a closed surface is the sum over its faces of
//! the signed tetrahedra from the origin, fanned from each loop's first vertex; holes run the other
//! way and take away what they should. 19.2's "Mass properties",
//! `projects/engines/geometry/architecture/19-meshes-spatial-clash.md`.
//!
//! Cost: linear in the half-edges; on grid points three products of `i128` per half-edge.

#![allow(
    clippy::many_single_char_names,
    reason = "a, b, c and d are points and x, y and z coordinates, as in the formulas"
)]

use geom_num::NumError;
use geom_num::grid::P3;
use geom_num::interval::Interval;
use geom_num::plane::Tpi;
use geom_num::wide::{I256, Wide};
use geom_run::WorkMeter;

use super::{ExactMesh, MeshError, MeshFault, Vertex, at};

/// Six times a mesh's volume.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Volume {
    /// Exactly this: every vertex is on the grid.
    Exact(I256),
    /// Certainly inside this, rounded outward.
    Certified(Interval),
}

/// det[a; b; c] of grid points: each term below 2^120, the sum below 2^123.
fn determinant(a: P3, b: P3, c: P3) -> i128 {
    let [ax, ay, az] = [a.x.get(), a.y.get(), a.z.get()].map(i128::from);
    let [bx, by, bz] = [b.x.get(), b.y.get(), b.z.get()].map(i128::from);
    let [cx, cy, cz] = [c.x.get(), c.y.get(), c.z.get()].map(i128::from);
    let minor = |p: i128, q: i128, r: i128, s: i128| {
        p.saturating_mul(q).saturating_sub(r.saturating_mul(s))
    };
    ax.saturating_mul(minor(by, cz, bz, cy))
        .saturating_add(ay.saturating_mul(minor(bz, cx, bx, cz)))
        .saturating_add(az.saturating_mul(minor(bx, cy, by, cx)))
}

/// The interval around a wide integer, built from its 32-bit digits, each exact in a double.
fn enclose<const L: usize>(v: &Wide<L>) -> Result<Interval, MeshError> {
    let negative = v.is_negative();
    let magnitude = if negative {
        v.checked_neg().ok_or(NumError::BudgetExceeded)?
    } else {
        *v
    };
    let radix = Interval::point(4_294_967_296.0)?;
    let mut sum = Interval::point(0.0)?;
    for limb in magnitude.to_le_limbs().iter().rev() {
        let (high, low) = (limb >> 32, limb & 0xffff_ffff);
        #[allow(
            clippy::cast_precision_loss,
            reason = "each is below 2^32, exact in a double"
        )]
        for digit in [high as f64, low as f64] {
            sum = sum.mul(radix)?.add(Interval::point(digit)?)?;
        }
    }
    Ok(if negative { sum.neg() } else { sum })
}

/// A vertex's three coordinates as intervals.
fn coordinates(mesh: &ExactMesh, vertex: Vertex) -> Result<[Interval; 3], MeshError> {
    let parts = mesh.parts();
    match vertex {
        Vertex::Grid(c) => {
            let exact = |x: i64| -> Result<Interval, MeshError> {
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "a coordinate is below 2^40, exact in a double"
                )]
                Ok(Interval::point(x as f64)?)
            };
            Ok([exact(c.x.get())?, exact(c.y.get())?, exact(c.z.get())?])
        }
        Vertex::Planes(rows) => {
            let [a, b, d] = rows.map(|row| at(&parts.planes, row).copied());
            let (x, w) = Tpi::meeting(a?, b?, d?)?.homogeneous()?;
            let w = enclose(&w)?;
            let [p, q, r] = x;
            Ok([
                enclose(&p)?.div(w)?,
                enclose(&q)?.div(w)?,
                enclose(&r)?.div(w)?,
            ])
        }
    }
}

/// det[a; b; c] of three interval points.
fn interval_determinant(
    a: [Interval; 3],
    b: [Interval; 3],
    c: [Interval; 3],
) -> Result<Interval, MeshError> {
    let [ax, ay, az] = a;
    let [bx, by, bz] = b;
    let [cx, cy, cz] = c;
    let minor = |p: Interval, q: Interval, r: Interval, s: Interval| p.mul(q)?.sub(r.mul(s)?);
    Ok(ax
        .mul(minor(by, cz, bz, cy)?)?
        .add(ay.mul(minor(bz, cx, bx, cz)?)?)?
        .add(az.mul(minor(bx, cy, by, cx)?)?)?)
}

impl ExactMesh {
    /// Six times the volume this surface bounds: exact while every vertex is a grid point.
    ///
    /// # Errors
    ///
    /// [`MeshError::Num`] for a sum that does not fit, which 2^32 determinants below 2^123 cannot
    /// reach, and [`MeshError::Stopped`] when the budget runs out.
    pub fn volume(&self, meter: &mut WorkMeter<'_>) -> Result<Volume, MeshError> {
        let p = self.parts();
        let grid = p.verts.iter().all(|v| matches!(v, Vertex::Grid(_)));
        let mut exact = I256::from_i128(0);
        let mut enclosed = Interval::point(0.0)?;
        for pair in p.loop_start.windows(2) {
            let &[start, end] = pair else {
                return Err(MeshFault::Shape.into());
            };
            let run = p
                .he_vert
                .get(start as usize..end as usize)
                .ok_or(MeshFault::Shape)?;
            let (&first, rest) = run.split_first().ok_or(MeshFault::Shape)?;
            let base = *at(&p.verts, first)?;
            let mut last = None;
            for &v in rest {
                meter.charge(1)?;
                let vertex = *at(&p.verts, v)?;
                if let Some(before) = last {
                    if grid {
                        if let (Vertex::Grid(a), Vertex::Grid(b), Vertex::Grid(c)) =
                            (base, before, vertex)
                        {
                            exact = exact
                                .checked_add(&I256::from_i128(determinant(a, b, c)))
                                .ok_or(NumError::BudgetExceeded)?;
                        }
                    } else {
                        enclosed = enclosed.add(interval_determinant(
                            coordinates(self, base)?,
                            coordinates(self, before)?,
                            coordinates(self, vertex)?,
                        )?)?;
                    }
                }
                last = Some(vertex);
            }
        }
        Ok(if grid {
            Volume::Exact(exact)
        } else {
            Volume::Certified(enclosed)
        })
    }

    /// Twice face `face`'s area vector: exact, each component below 2^116, its holes taken away.
    /// `None` if the face has a corner off the grid.
    ///
    /// # Errors
    ///
    /// [`MeshError::Fault`] for a face that is not there.
    pub fn twice_area_vector(&self, face: u32) -> Result<Option<[i128; 3]>, MeshError> {
        let p = self.parts();
        let first = *at(&p.face_loops, face)?;
        let last = *at(&p.face_loops, face.saturating_add(1))?;
        let mut sum = [0_i128; 3];
        for looped in first..last {
            let start = *at(&p.loop_start, looped)?;
            let end = *at(&p.loop_start, looped.saturating_add(1))?;
            let run = p
                .he_vert
                .get(start as usize..end as usize)
                .ok_or(MeshFault::Shape)?;
            let mut last_point: Option<[i128; 3]> = None;
            let (&head, rest) = run.split_first().ok_or(MeshFault::Shape)?;
            let Vertex::Grid(base) = *at(&p.verts, head)? else {
                return Ok(None);
            };
            for &v in rest {
                let Vertex::Grid(c) = *at(&p.verts, v)? else {
                    return Ok(None);
                };
                let u = [
                    i128::from(c.x.get()).saturating_sub(i128::from(base.x.get())),
                    i128::from(c.y.get()).saturating_sub(i128::from(base.y.get())),
                    i128::from(c.z.get()).saturating_sub(i128::from(base.z.get())),
                ];
                if let Some([a, b, d]) = last_point {
                    let [x, y, z] = u;
                    let term = |p: i128, q: i128, r: i128, s: i128| {
                        p.saturating_mul(q).saturating_sub(r.saturating_mul(s))
                    };
                    let [sx, sy, sz] = sum;
                    sum = [
                        sx.saturating_add(term(b, z, d, y)),
                        sy.saturating_add(term(d, x, a, z)),
                        sz.saturating_add(term(a, y, b, x)),
                    ];
                }
                last_point = Some(u);
            }
        }
        Ok(Some(sum))
    }

    /// Face `face`'s area, as an interval rounded outward. `None` if it has a corner off the grid.
    ///
    /// # Errors
    ///
    /// [`MeshError::Fault`] for a face that is not there.
    pub fn area(&self, face: u32) -> Result<Option<Interval>, MeshError> {
        let Some([x, y, z]) = self.twice_area_vector(face)? else {
            return Ok(None);
        };
        let [x, y, z] = [
            enclose(&I256::from_i128(x))?,
            enclose(&I256::from_i128(y))?,
            enclose(&I256::from_i128(z))?,
        ];
        let half = Interval::point(0.5)?;
        Ok(Some(Interval::norm(&[x, y, z])?.mul(half)?))
    }
}
