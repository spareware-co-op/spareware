//! Certified distances of a grid point from a carrier, and between points that carry a bound
//! (11.4, G242).
//!
//! What it does: [`Carriers`] holds the builder's carriers: a table of planes and pools of surfaces
//! and curves, named by [`CarrierIx`]. [`Carriers::distance`] encloses how far a grid point lies
//! from one, computed from the carrier's own definition: a plane's from its integer coefficients
//! in one exact step, a line's in closed form, every other kind's by `geom_intersect::project`,
//! the certified nearest-point search of chapter 12.4. [`separation`] encloses how far apart two
//! points are, each point's own bound added: a certified point may lie anywhere within its bound of
//! where its carriers meet.
//!
//! Guarantees:
//!
//! - **An enclosure, never a number.** Each distance holds the true distance. A refinement only
//!   narrows it: the floor of the search at refinement k is [`FIRST_FLOOR`] over 2^k steps.
//! - **From the definition.** A distance is measured from the carrier's definition, never from an
//!   earlier representative or a cached fit, so repeated operations cannot walk a point away from
//!   its carriers.
//! - **A plane's distance is exact up to one rounding.** n · p + d is computed in `i128`, checked,
//!   and enclosed by the nearest doubles around it; |n| by an interval square root.
//! - **A line's distance is closed form.** For o + t·v over [a, b], a segment, a ray (one end
//!   infinite) or a whole line, the nearest parameter (p − o) · v / |v|² is enclosed and held to
//!   the domain; the distance is at most |p − o − t·v| over that enclosure, and at least the
//!   distance from the whole line, |(p − o) × v| / |v|, or from the end the nearest parameter is
//!   proven past, which is then a finite end. It does not change with the refinement.
//! - **Undecided is wide.** A search that reaches its floor with nearest points that cannot be told
//!   apart (`NonIsolated`) gives the widest enclosure, so the contact rule reads it as near.
//!
//! Method: the point-to-surface projection of Ma and Hewitt, certified, in
//! `geom_intersect::project` (<https://doi.org/10.1016/S0167-8396(03)00021-9>), and the plane's
//! signed distance n · p + d over |n| (`geom_num::plane`).
//!
//! Cost: a plane or a line, a few interval operations; any other carrier, one projection search at
//! the floor asked, charged to the meter. No recursion.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the chapter's: p a point, c a carrier, n a normal, d an offset"
)]

use alloc::vec::Vec;

use geom_intersect::project::{Projection, project_curve, project_surface};
use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_num::wide::I256;
use geom_num::{NumError, P3, Steps};
use geom_poly::plane::{PlaneIx, PlaneTable};
use geom_run::WorkMeter;
use geom_topo::{CurveIx, CurveKind, ParamCurve, SurfaceIx, SurfaceKind};

use crate::carrier::{Bound, CarrierIx, PointGeom};
use crate::rule::{ContactError, greater, lesser};

/// The floor of a projection at refinement 0, in steps; refinement k halves it k times.
pub const FIRST_FLOOR: f64 = 4.0;

/// The builder's carriers.
#[derive(Debug, Default)]
pub struct Carriers {
    planes: PlaneTable,
    surfaces: Vec<SurfaceKind>,
    curves: Vec<CurveKind<3>>,
}

/// The nearest doubles around a whole number held in 128 bits.
pub(crate) fn enclose_i128(v: i128) -> Result<Interval, NumError> {
    if v == 0 {
        return Interval::point(0.0);
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "the enclosure is widened by a double's step each way, which holds the value"
    )]
    let near = v as f64;
    Interval::new(near.next_down(), near.next_up())
}

/// A point's coordinates as doubles, which hold them exactly (at most 2^40).
fn coordinates(p: P3) -> [f64; 3] {
    #[allow(
        clippy::cast_precision_loss,
        reason = "a coordinate is at most 2^40 in size, which a double holds exactly"
    )]
    [p.x.get() as f64, p.y.get() as f64, p.z.get() as f64]
}

/// The smaller of two intervals' ends, so the enclosure of the nearer of two distances.
fn nearer(a: Interval, b: Interval) -> Result<Interval, NumError> {
    Interval::new(lesser(a.lo(), b.lo()), lesser(a.hi(), b.hi()))
}

/// An interval of distances with its lower end at least zero.
fn non_negative(d: Interval) -> Result<Interval, NumError> {
    Interval::new(greater(d.lo(), 0.0), greater(d.hi(), 0.0))
}

impl Carriers {
    /// No carriers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a plane, once: the same plane in another scale or facing gives the same entry.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if the plane cannot be reduced within its budget or the table
    /// is full.
    pub fn add_plane(&mut self, plane: &Plane) -> Result<PlaneIx, NumError> {
        self.planes
            .insert(plane)
            .map(|facing| facing.plane)
            .map_err(|_| NumError::BudgetExceeded)
    }

    /// Adds a surface.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if the pool is full.
    pub fn add_surface(&mut self, surface: SurfaceKind) -> Result<SurfaceIx, NumError> {
        let ix = SurfaceIx::from_usize(self.surfaces.len()).ok_or(NumError::BudgetExceeded)?;
        self.surfaces.push(surface);
        Ok(ix)
    }

    /// Adds a curve.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if the pool is full.
    pub fn add_curve(&mut self, curve: CurveKind<3>) -> Result<CurveIx, NumError> {
        let ix = CurveIx::from_usize(self.curves.len()).ok_or(NumError::BudgetExceeded)?;
        self.curves.push(curve);
        Ok(ix)
    }

    /// The plane of a table entry.
    #[must_use]
    pub fn plane(&self, ix: PlaneIx) -> Option<Plane> {
        self.planes.plane(ix)
    }

    /// A curve of the pool.
    #[must_use]
    pub fn curve(&self, ix: CurveIx) -> Option<&CurveKind<3>> {
        self.curves.get(usize::try_from(ix.row()).ok()?)
    }

    /// A surface of the pool.
    #[must_use]
    pub fn surface(&self, ix: SurfaceIx) -> Option<&SurfaceKind> {
        self.surfaces.get(usize::try_from(ix.row()).ok()?)
    }

    /// The signed distance of a grid point from a plane, as an enclosure: (n · p + d) / |n|.
    fn plane_distance(plane: &Plane, p: P3) -> Result<Interval, NumError> {
        let [n0, n1, n2] = plane.n();
        let [x, y, z] = [p.x.get(), p.y.get(), p.z.get()].map(i128::from);
        let value = n0
            .checked_mul(x)
            .and_then(|s| s.checked_add(n1.checked_mul(y)?))
            .and_then(|s| s.checked_add(n2.checked_mul(z)?))
            .and_then(|s| s.checked_add(plane.d()))
            .ok_or(NumError::BudgetExceeded)?;
        let norm = Interval::norm(&[enclose_i128(n0)?, enclose_i128(n1)?, enclose_i128(n2)?])?;
        if value == 0 {
            return Interval::point(0.0);
        }
        enclose_i128(value)?.div(norm)
    }

    /// The least whole number of steps b with b² |n|² ≥ (n · p + d)²: how far a grid point is from
    /// a plane, rounded up, decided exactly in wide integers with no rounding of its own.
    fn plane_bound(plane: &Plane, p: P3) -> Result<Bound, NumError> {
        let [n0, n1, n2] = plane.n();
        let [x, y, z] = [p.x.get(), p.y.get(), p.z.get()].map(i128::from);
        let value = n0
            .checked_mul(x)
            .and_then(|s| s.checked_add(n1.checked_mul(y)?))
            .and_then(|s| s.checked_add(n2.checked_mul(z)?))
            .and_then(|s| s.checked_add(plane.d()))
            .ok_or(NumError::BudgetExceeded)?;
        let wide = |v: i128| I256::from_i128(v);
        let times = |a: &I256, b: &I256| a.checked_mul(b).ok_or(NumError::BudgetExceeded);
        let norm_sq = [n0, n1, n2].iter().try_fold(I256::from_i128(0), |sum, n| {
            sum.checked_add(&times(&wide(*n), &wide(*n))?)
                .ok_or(NumError::BudgetExceeded)
        })?;
        let target = times(&wide(value), &wide(value))?;
        // The smallest b in 0..=2^62 with b² |n|² >= target, by bisection; none is a distance past
        // what a count of steps holds.
        let covers = |b: u64| -> Result<bool, NumError> {
            let b = I256::from_i128(i128::from(b));
            Ok(times(&times(&b, &b)?, &norm_sq)? >= target)
        };
        let (mut lo, mut hi) = (0_u64, 1_u64 << 62);
        if !covers(hi)? {
            return Err(NumError::BudgetExceeded);
        }
        while lo < hi {
            let mid = lo.saturating_add(hi.saturating_sub(lo) / 2);
            if covers(mid)? {
                hi = mid;
            } else {
                lo = mid.saturating_add(1);
            }
        }
        Ok(Bound(Steps(lo)))
    }

    /// An upper bound, in whole steps, on how far the grid point `p` lies from the carrier `c`: a
    /// plane's exactly, any other carrier's the enclosure's upper end rounded up. With the
    /// enclosure it is the bound of.
    ///
    /// # Errors
    ///
    /// As [`Carriers::distance`]; [`NumError::BudgetExceeded`] for a distance past what a count of
    /// steps holds.
    pub fn upper_bound(
        &self,
        p: P3,
        c: CarrierIx,
        refinement: u32,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Bound, Interval), ContactError> {
        let d = self.distance(p, c, refinement, meter)?;
        if let CarrierIx::Plane(ix) = c {
            let plane = self.planes.plane(ix).ok_or(NumError::Degenerate)?;
            return Ok((Self::plane_bound(&plane, p)?, d));
        }
        let bound = Bound::of(d.hi()).ok_or(NumError::BudgetExceeded)?;
        Ok((bound, d))
    }

    /// How far the grid point `p` lies from the carrier `c`, enclosed, refined `refinement` times:
    /// the floor of a projection is [`FIRST_FLOOR`] over 2^`refinement` steps. A plane's distance
    /// is exact and does not change with the refinement.
    ///
    /// # Errors
    ///
    /// [`ContactError::Num`] for a carrier that is not in the pools; whatever the projection
    /// stops with.
    pub fn distance(
        &self,
        p: P3,
        c: CarrierIx,
        refinement: u32,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Interval, ContactError> {
        let scale = (0..refinement.min(30)).fold(1.0_f64, |s, _| s * 2.0);
        let floor = FIRST_FLOOR / scale;
        let at = coordinates(p);
        let widest = || Interval::new(0.0, f64::MAX).map_err(ContactError::from);
        match c {
            CarrierIx::Plane(ix) => {
                let plane = self.planes.plane(ix).ok_or(NumError::Degenerate)?;
                let signed = Self::plane_distance(&plane, p)?;
                let (lo, hi) = (signed.lo().abs(), signed.hi().abs());
                let straddles = signed.lo() <= 0.0 && signed.hi() >= 0.0;
                Ok(Interval::new(
                    if straddles { 0.0 } else { lesser(lo, hi) },
                    greater(lo, hi),
                )?)
            }
            CarrierIx::Surface(ix) => {
                let surface = self
                    .surfaces
                    .get(usize::try_from(ix.row()).map_err(|_| NumError::Degenerate)?)
                    .ok_or(NumError::Degenerate)?;
                match project_surface(surface, &at, floor, meter)? {
                    Projection::Points(found) => {
                        let mut best: Option<Interval> = None;
                        for n in &found {
                            best = Some(match best {
                                Some(b) => nearer(b, n.distance)?,
                                None => n.distance,
                            });
                        }
                        best.map(non_negative).transpose()?.map_or_else(widest, Ok)
                    }
                    Projection::Set { distance, .. } => Ok(non_negative(distance)?),
                    Projection::NonIsolated { .. } => widest(),
                }
            }
            CarrierIx::Curve(ix) => {
                let curve = self
                    .curves
                    .get(usize::try_from(ix.row()).map_err(|_| NumError::Degenerate)?)
                    .ok_or(NumError::Degenerate)?;
                if let CurveKind::Line(line) = curve {
                    return Ok(segment_distance(
                        at,
                        (line.origin(), line.direction()),
                        curve.domain(),
                    )?);
                }
                match project_curve(curve, &at, floor, meter)? {
                    Projection::Points(found) => {
                        let mut best: Option<Interval> = None;
                        for n in &found {
                            best = Some(match best {
                                Some(b) => nearer(b, n.distance)?,
                                None => n.distance,
                            });
                        }
                        best.map(non_negative).transpose()?.map_or_else(widest, Ok)
                    }
                    Projection::Set { distance, .. } => Ok(non_negative(distance)?),
                    Projection::NonIsolated { .. } => widest(),
                }
            }
        }
    }

    /// The grid point a point is committed to, and how far that can be from where the point is: a
    /// grid point exactly, a plane meeting rounded to the grid (at most one step, as chapter 6.5
    /// derives), a certified point as itself within its bound.
    ///
    /// # Errors
    ///
    /// [`ContactError::Num`] for planes that do not meet in one point or are not in the table.
    pub fn commit(&self, point: &PointGeom) -> Result<(P3, Bound), ContactError> {
        match point {
            PointGeom::Grid(p) => Ok((*p, Bound::ZERO)),
            PointGeom::Certified { at, within, .. } => Ok((*at, *within)),
            PointGeom::Planes([a, b, c]) => {
                let get = |ix: PlaneIx| self.planes.plane(ix).ok_or(NumError::Degenerate);
                let meeting = geom_num::plane::Tpi::new(get(*a)?, get(*b)?, get(*c)?)?;
                let rounded = meeting.to_grid()?;
                let bound = if rounded.is_exact() {
                    Bound::ZERO
                } else {
                    Bound(geom_num::ladder::COMMIT_DISTANCE_3D)
                };
                Ok((rounded.value(), bound))
            }
        }
    }
}

/// Three doubles, each enclosed as itself.
fn exact3(v: [f64; 3]) -> Result<[Interval; 3], NumError> {
    let [x, y, z] = v;
    Ok([
        Interval::point(x)?,
        Interval::point(y)?,
        Interval::point(z)?,
    ])
}

/// The enclosure of the distance from the point `at` to the line o + t·v, t in `span`, whose ends
/// may be infinite: an infinite end is never used as a parameter, since the nearest parameter, a
/// finite enclosure, is never past it.
fn segment_distance(
    at: [f64; 3],
    (origin, direction): ([f64; 3], [f64; 3]),
    (a, b): (f64, f64),
) -> Result<Interval, NumError> {
    let [px, py, pz] = exact3(at)?;
    let [ox, oy, oz] = exact3(origin)?;
    let [vx, vy, vz] = exact3(direction)?;
    let off = [px.sub(ox)?, py.sub(oy)?, pz.sub(oz)?];
    let [wx, wy, wz] = off;
    let dot = |l: [Interval; 3], r: [Interval; 3]| -> Result<Interval, NumError> {
        let [l0, l1, l2] = l;
        let [r0, r1, r2] = r;
        l0.mul(r0)?.add(l1.mul(r1)?)?.add(l2.mul(r2)?)
    };
    let along = dot(off, [vx, vy, vz])?;
    let length_sq = dot([vx, vy, vz], [vx, vy, vz])?;
    let nearest = along.div(length_sq)?;
    // The parameter held to the domain, and the point there, enclosed.
    let held = Interval::new(
        if nearest.lo() < a {
            a
        } else if nearest.lo() > b {
            b
        } else {
            nearest.lo()
        },
        if nearest.hi() > b {
            b
        } else if nearest.hi() < a {
            a
        } else {
            nearest.hi()
        },
    )?;
    let gap = |t: Interval| -> Result<Interval, NumError> {
        Interval::norm(&[
            wx.sub(t.mul(vx)?)?,
            wy.sub(t.mul(vy)?)?,
            wz.sub(t.mul(vz)?)?,
        ])
    };
    let upper = gap(held)?.hi();
    let lower = if nearest.hi() < a {
        gap(Interval::point(a)?)?.lo()
    } else if nearest.lo() > b {
        gap(Interval::point(b)?)?.lo()
    } else {
        let cross = [
            wy.mul(vz)?.sub(wz.mul(vy)?)?,
            wz.mul(vx)?.sub(wx.mul(vz)?)?,
            wx.mul(vy)?.sub(wy.mul(vx)?)?,
        ];
        Interval::norm(&cross)?.div(length_sq.sqrt()?)?.lo()
    };
    Interval::new(greater(lower, 0.0), greater(upper, greater(lower, 0.0)))
}

/// How far apart two points are, enclosed, each point's own bound added: the distance between
/// their committed grid points, widened by both bounds each way, and never below zero.
///
/// # Errors
///
/// [`ContactError::Num`] when a point cannot be committed.
pub fn separation(
    carriers: &Carriers,
    a: &PointGeom,
    b: &PointGeom,
) -> Result<Interval, ContactError> {
    let ((pa, wa), (pb, wb)) = (carriers.commit(a)?, carriers.commit(b)?);
    let diff = |u: i64, v: i64| -> Result<Interval, NumError> {
        let d = u.checked_sub(v).ok_or(NumError::BudgetExceeded)?;
        enclose_i128(i128::from(d))
    };
    let parts = [
        diff(pa.x.get(), pb.x.get())?,
        diff(pa.y.get(), pb.y.get())?,
        diff(pa.z.get(), pb.z.get())?,
    ];
    let apart = Interval::norm(&parts)?;
    #[allow(
        clippy::cast_precision_loss,
        reason = "bounds are counts of steps far below 2^53"
    )]
    let slack = (wa.steps() as f64) + (wb.steps() as f64);
    Ok(Interval::new(
        greater(apart.lo() - slack, 0.0),
        apart.hi() + slack,
    )?)
}
