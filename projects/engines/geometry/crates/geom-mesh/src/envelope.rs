//! The polyhedral envelope: a body that proves "within ε of these triangles" exactly (19.8).
//!
//! What it does: [`TriangleEnvelope`] is a convex polyhedron around one triangle, its corners on
//! the grid, inscribed in the triangle's ε-offset. [`Envelope`] holds one for each triangle of a
//! surface, behind a box tree. A point inside any of them is within ε of the surface.
//!
//! Guarantees:
//!
//! - **Inside proves within ε.** Each polyhedron is the triangle swept by a cube of half side h, the
//!   largest whole number with 3h² ≤ ε². Its corners are a triangle's vertices moved by (±h, ±h, ±h),
//!   each within ε of the triangle, and the ε-offset of a triangle is convex, so the whole polyhedron
//!   is inside it. A point farther than ε is never inside.
//! - **Within h proves inside.** A point within h of the triangle is inside, since a ball of radius
//!   h fits in the cube. So the envelope is never more than a factor √3 narrower than ε.
//! - **Exact, within 384 bits.** Every test is the sign of a plane at a point: a grid point in `i128`,
//!   a line-plane or three-plane point in [`Plane::side_of_lpi`] and [`Plane::side_of_tpi`], chapter
//!   6.2's deepest tests. The width is absolute, in grid steps, never a fraction of a model's size.
//! - **Refused, never rounded.** A plane over 6.2's budget is an error. That takes an envelope
//!   reaching past the frame on a triangle whose normal is near its largest.
//!
//! Method: the polyhedral envelope of Wang et al. (2020), <https://doi.org/10.1145/3386569.3392426>,
//! as 19.8 asks, its corners on the grid
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`). The paper bounds a triangle
//! by a prism with caps; here the same inscribed body is a Minkowski sum, the triangle and a cube,
//! whose facets have closed-form normals: the cube's, the triangle's, and each edge crossed with
//! each axis, both ways. A body with normal m reaches m · v + h·|m|₁ at its furthest, so each
//! facet's plane is known without a hull.
//!
//! Cost: 26 planes a triangle at most, each one or two greatest common divisors. A containment test
//! is a tree query and up to 26 signs for each triangle whose box holds the point.

use alloc::vec::Vec;

use geom_bvh::{Aabb, BoxTree, BvhError, Entry};
use geom_math::gcd_u128;
use geom_num::grid::P3;
use geom_num::plane::{Lpi, Plane, Tpi};
use geom_num::{NumError, Sign};
use geom_run::{Exec, Stopped, WorkMeter};

/// Why an envelope could not be made or asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeError {
    /// A plane is over the budget of 6.2, or the numbers layer refused.
    Num(NumError),
    /// The box tree refused.
    Bvh(BvhError),
    /// The work budget ran out, or the run was cancelled.
    Stopped(Stopped),
    /// More triangles than a tree indexes.
    Full,
}

impl core::fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EnvelopeError::Num(e) => write!(f, "{e}"),
            EnvelopeError::Bvh(e) => write!(f, "{e}"),
            EnvelopeError::Stopped(e) => write!(f, "{e}"),
            EnvelopeError::Full => f.write_str("more triangles than a tree indexes"),
        }
    }
}

impl core::error::Error for EnvelopeError {}

impl From<NumError> for EnvelopeError {
    fn from(e: NumError) -> Self {
        EnvelopeError::Num(e)
    }
}

impl From<BvhError> for EnvelopeError {
    fn from(e: BvhError) -> Self {
        EnvelopeError::Bvh(e)
    }
}

impl From<Stopped> for EnvelopeError {
    fn from(e: Stopped) -> Self {
        EnvelopeError::Stopped(e)
    }
}

/// The half side of the cube that fits an envelope of width `epsilon`: the largest h with
/// 3h² ≤ ε², so that (±h, ±h, ±h) is within ε of the origin.
#[must_use]
pub fn half_side(epsilon: u64) -> u64 {
    let e = u128::from(epsilon);
    let h = e.saturating_mul(e).checked_div(3).unwrap_or(0).isqrt();
    u64::try_from(h).unwrap_or(0)
}

/// Coordinate `k` of a point.
fn coordinate(p: P3, k: usize) -> i128 {
    i128::from(match k {
        0 => p.x.get(),
        1 => p.y.get(),
        _ => p.z.get(),
    })
}

/// `b − a`.
fn run(a: P3, b: P3) -> [i128; 3] {
    let part = |k: usize| coordinate(b, k).wrapping_sub(coordinate(a, k));
    [part(0), part(1), part(2)]
}

/// u × v of differences: each component below 2^83, so wrapping steps are exact.
fn cross(u: [i128; 3], v: [i128; 3]) -> [i128; 3] {
    let [a, b, c] = u;
    let [x, y, z] = v;
    let term =
        |p: i128, q: i128, r: i128, s: i128| p.wrapping_mul(q).wrapping_sub(r.wrapping_mul(s));
    [term(b, z, c, y), term(c, x, a, z), term(a, y, b, x)]
}

/// `m` over its components' common divisor, or `None` for a zero vector.
fn primitive(m: [i128; 3]) -> Option<[i128; 3]> {
    let [x, y, z] = m.map(i128::unsigned_abs);
    let g = gcd_u128(gcd_u128(x, y), z);
    if g == 0 {
        return None;
    }
    let by = i128::try_from(g).ok()?;
    Some(m.map(|v| v.checked_div(by).unwrap_or(0)))
}

/// The plane of the facet with outward normal `m`: n · x + d = 0 with d = −(max m·v + h·|m|₁).
/// A point is on or under it when its side is not `Positive`.
fn facet(m: [i128; 3], t: [P3; 3], h: u64) -> Result<Plane, NumError> {
    let reach = |v: P3| -> Option<i128> {
        let [a, b, c] = m;
        let dot = a
            .checked_mul(coordinate(v, 0))?
            .checked_add(b.checked_mul(coordinate(v, 1))?)?
            .checked_add(c.checked_mul(coordinate(v, 2))?)?;
        Some(dot)
    };
    let [p, q, r] = t;
    let furthest = reach(p)
        .zip(reach(q))
        .zip(reach(r))
        .map(|((x, y), z)| x.max(y).max(z))
        .ok_or(NumError::BudgetExceeded)?;
    let [a, b, c] = m.map(i128::unsigned_abs);
    let width = a
        .checked_add(b)
        .and_then(|s| s.checked_add(c))
        .and_then(|s| i128::try_from(s).ok())
        .and_then(|s| s.checked_mul(i128::from(h)))
        .ok_or(NumError::BudgetExceeded)?;
    let offset = furthest
        .checked_add(width)
        .and_then(i128::checked_neg)
        .ok_or(NumError::BudgetExceeded)?;
    Plane::new(m, offset)
}

/// A convex polyhedron with grid corners around one triangle, inscribed in its ε-offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriangleEnvelope {
    planes: Vec<Plane>,
}

impl TriangleEnvelope {
    /// The triangle `t` swept by the cube of half side `h`. A triangle with no area, or a repeated
    /// corner, gives the same body with the facets of what remains.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] for a plane over 6.2's budget.
    pub fn new(t: [P3; 3], h: u64) -> Result<Self, NumError> {
        let [a, b, c] = t;
        let axes: [[i128; 3]; 3] = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];
        let edges = [run(a, b), run(b, c), run(c, a)];
        let mut normals: Vec<[i128; 3]> = Vec::with_capacity(13);
        normals.extend(axes);
        normals.push(cross(run(a, b), run(a, c)));
        for edge in edges {
            for axis in axes {
                normals.push(cross(edge, axis));
            }
        }
        let mut planes = Vec::with_capacity(26);
        for n in normals {
            if let Some(m) = primitive(n) {
                planes.push(facet(m, t, h)?);
                planes.push(facet(m.map(i128::wrapping_neg), t, h)?);
            }
        }
        Ok(Self { planes })
    }

    /// The facets' planes, each looking outward.
    #[must_use]
    pub fn planes(&self) -> &[Plane] {
        &self.planes
    }

    /// Whether grid point `p` is inside or on the polyhedron.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`], which planes and points within budget cannot reach.
    pub fn contains(&self, p: P3) -> Result<bool, NumError> {
        for plane in &self.planes {
            if plane.side(p)? == Sign::Positive {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Whether a three-plane point is inside or on the polyhedron.
    ///
    /// # Errors
    ///
    /// As for [`Plane::side_of_tpi`].
    pub fn contains_tpi(&self, p: &Tpi) -> Result<bool, NumError> {
        for plane in &self.planes {
            if plane.side_of_tpi(p)? == Sign::Positive {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Whether a line-plane point is inside or on the polyhedron.
    ///
    /// # Errors
    ///
    /// As for [`Plane::side_of_lpi`].
    pub fn contains_lpi(&self, p: &Lpi) -> Result<bool, NumError> {
        for plane in &self.planes {
            if plane.side_of_lpi(p)? == Sign::Positive {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// The envelope of a surface: one polyhedron to a triangle, and a tree of their boxes.
#[derive(Debug)]
pub struct Envelope {
    triangles: Vec<TriangleEnvelope>,
    tree: BoxTree<3>,
    /// How far a box reaches past its triangle.
    h: u64,
}

impl Envelope {
    /// The envelope of width `epsilon`, in grid steps, around `triangles`.
    ///
    /// # Errors
    ///
    /// [`EnvelopeError::Num`] for a plane over budget, [`EnvelopeError::Full`] and
    /// [`EnvelopeError::Stopped`] as they say, and [`EnvelopeError::Bvh`] for a box the tree
    /// refuses.
    pub fn new(
        triangles: &[[P3; 3]],
        epsilon: u64,
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Self, EnvelopeError> {
        let h = half_side(epsilon);
        let pad = i64::try_from(h).map_err(|_| NumError::BudgetExceeded)?;
        let mut bodies = Vec::with_capacity(triangles.len());
        let mut entries = Vec::with_capacity(triangles.len());
        for (t, key) in triangles.iter().zip(0_u64..) {
            meter.charge(1)?;
            bodies.push(TriangleEnvelope::new(*t, h)?);
            let [a, b, c] = *t;
            let low = |f: fn(P3) -> i64| f(a).min(f(b)).min(f(c)).saturating_sub(pad);
            let high = |f: fn(P3) -> i64| f(a).max(f(b)).max(f(c)).saturating_add(pad);
            let (x, y, z): (fn(P3) -> i64, fn(P3) -> i64, fn(P3) -> i64) =
                (|p| p.x.get(), |p| p.y.get(), |p| p.z.get());
            entries.push(Entry {
                key,
                bbox: Aabb::new([low(x), low(y), low(z)], [high(x), high(y), high(z)])?,
            });
        }
        let tree = BoxTree::build(&entries, exec, meter)?;
        Ok(Self {
            triangles: bodies,
            tree,
            h,
        })
    }

    /// The half side of the envelope's cube: every point this close to the surface is inside.
    #[must_use]
    pub const fn half_side(&self) -> u64 {
        self.h
    }

    /// The bodies whose boxes hold the box from `low` to `high`.
    fn near(
        &self,
        low: [i64; 3],
        high: [i64; 3],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<&TriangleEnvelope>, EnvelopeError> {
        let keys = self.tree.overlapping(&Aabb::new(low, high)?, meter)?;
        let mut out = Vec::with_capacity(keys.len());
        for key in keys {
            let i = usize::try_from(key).map_err(|_| EnvelopeError::Full)?;
            out.extend(self.triangles.get(i));
        }
        Ok(out)
    }

    /// Whether grid point `p` is inside the envelope, so within ε of the surface.
    ///
    /// # Errors
    ///
    /// As for [`TriangleEnvelope::contains`] and the tree's query.
    pub fn contains(&self, p: P3, meter: &mut WorkMeter<'_>) -> Result<bool, EnvelopeError> {
        let at = [p.x.get(), p.y.get(), p.z.get()];
        for body in self.near(at, at, meter)? {
            meter.charge(1)?;
            if body.contains(p)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Whether a three-plane point is inside the envelope.
    ///
    /// # Errors
    ///
    /// As for [`TriangleEnvelope::contains_tpi`] and the tree's query.
    pub fn contains_tpi(&self, p: &Tpi, meter: &mut WorkMeter<'_>) -> Result<bool, EnvelopeError> {
        // Rounding moves a coordinate at most half a step, so one step's padding holds the point.
        let c = p.to_grid()?.value();
        let at = [c.x.get(), c.y.get(), c.z.get()];
        let (low, high) = (
            at.map(|v| v.saturating_sub(1)),
            at.map(|v| v.saturating_add(1)),
        );
        for body in self.near(low, high, meter)? {
            meter.charge(1)?;
            if body.contains_tpi(p)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
