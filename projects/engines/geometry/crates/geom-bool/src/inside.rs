//! Where a point lies in a flat face: on its boundary, inside it or outside it, certified (11.4
//! step 4, G243a).
//!
//! What it does: a [`FlatFace`] is a face whose carrier is a plane of the table, with its boundary
//! edges and the joins between them. [`FlatFace::place`] says where a grid point, known within a bound, lies.
//! **On** when it touches a boundary edge by the contact rule, measured in space from the edge's
//! curve ([`crate::distance::Carriers::distance`]), so the decision is the one vertex with edge
//! takes. Otherwise **In** or **Out** by the parity of a ray's crossings in the plane's own picture:
//! the coordinate the plane's normal is largest in is dropped, an affine bijection from the plane
//! onto the other two coordinates that keeps inside and outside. A ray runs from the point along a
//! fixed sequence of small whole directions in that picture. Lifted back to space, the ray's line
//! is the meeting of the face's plane with the plane through the point that holds the ray and the
//! dropped axis, a plane of whole numbers, so each boundary edge crosses the ray where it crosses
//! that plane ([`crate::crossing::crossings`]), on the ray's side of the point. A direction is
//! kept only when its answer is proven: every crossing a simple root ahead of the point or behind
//! it, no boundary edge lying in the ray's plane, no flagged region ahead, and no join within a step
//! of the ray ahead of the point: a join is the box round the end points of the curves that meet at
//! a vertex (or a closed curve's two ends), where a crossing could fall between two curves, or be
//! counted on both.
//!
//! Guarantees:
//!
//! - **On is the contact rule.** A point touches the boundary only when its distance from a
//!   boundary edge's curve, widened by the point's own bound, holds zero and lies within `ε_fit` at
//!   the finest refinement; a pair the rule cannot decide fails with [`crate::rule::NearContact`].
//! - **In and Out are proven, never sampled.** Each crossing counted is a zero of the edge's offset
//!   from the ray's plane that Krawczyk's operator proved alone in its box, where the edge passes
//!   from one side to the other, and whose position is proven ahead of the point. A direction with
//!   any undecided part is dropped whole, and the next is tried.
//! - **Joins cannot hide a crossing.** The boundary is its curves and the gaps where neighbouring
//!   curves meet, each gap inside its join's box, which holds the enclosures of the curves' end
//!   points there. A ray that stays more than a step from every join ahead of the point meets no
//!   gap and no curve at its end, so the crossings counted are those of the closed boundary, each
//!   once.
//! - **Refused, never guessed.** When no direction of [`DIRECTIONS`] qualifies, the point's
//!   picture lies on the boundary's picture while the point itself is proven apart from the
//!   boundary in space, and [`FlatFace::place`] fails with [`BuildError::Unclassified`].
//! - **Deterministic.** The directions are tried in their fixed order, and the edges in loop order.
//!
//! Method: the crossing-number test for a point in a polygon (E. Haines, "Point in Polygon
//! Strategies", *Graphics Gems IV*, Academic Press (1994), section 2,
//! <https://doi.org/10.1016/B978-0-12-336156-1.50013-6>), with every crossing proven by chapter
//! 12.3's engine (`projects/engines/geometry/architecture/12-intersections.md`) and a fixed
//! sequence of rays in place of a random one, as chapter 11.4's classification asks
//! (`projects/engines/geometry/architecture/11-booleans.md`). The order of tests, the point on the
//! carrier first and then near the boundary and then inside, is `SMLib`'s `TestAndImprintFace`
//! and `SmLoop::ContainsUVPoint`, read in solidcore's Rust port; their winding walk samples each
//! trim curve in steps and decides within a tolerance, which is not taken.
//!
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/c440e5f892452a9654aee8f3030de6095fdb1415/crates/solidcore/src/boolean/topology_intersector/p1.rs>
//!   and
//!   <https://github.com/tangency-labs/solidcore/blob/c440e5f892452a9654aee8f3030de6095fdb1415/crates/solidcore/src/query/loop.rs>,
//!   for the order of the vertex-in-face tests; no code copied.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: [`FlatFace::new`] converts each boundary edge to its NURBS form once. [`FlatFace::place`]
//! is one certified distance per boundary edge whose box is within reach, then per direction tried
//! an interval test per join and, per boundary edge not proven on one side of the ray's plane by
//! its hull, one run of the crossing engine; all charged to the meter. No recursion.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_num::{NumError, P3};
use geom_run::WorkMeter;
use geom_topo::{CurveIx, ParamCurve};

use crate::carrier::{Bound, CarrierIx};
use crate::crossing::{Enclosed, Hull, crossings};
use crate::distance::Carriers;
use crate::interfere::{Arrangement, BuildError, Entity, widened};
use crate::rule::{Contact, EPS_FIT, Verdict, decide, judge};

/// The ray directions tried, in order, in the plane's own picture: whole and primitive, no two
/// parallel.
pub const DIRECTIONS: [(i64, i64); 16] = [
    (1, 0),
    (0, 1),
    (1, 1),
    (1, -1),
    (2, 1),
    (1, 2),
    (2, -1),
    (1, -2),
    (3, 1),
    (1, 3),
    (3, -1),
    (1, -3),
    (3, 2),
    (2, 3),
    (3, -2),
    (2, -3),
];

/// Where a point lies in a face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// Outside it.
    Out,
    /// Inside it, off its boundary.
    In,
    /// On its boundary: the boundary edges it touches, in loop order.
    On(Vec<usize>),
}

/// A boundary edge, ready for the tests.
#[derive(Clone, Debug)]
struct Side {
    /// The edge, by its index in the arrangement.
    edge: usize,
    /// Its curve.
    curve: CurveIx,
    /// Its NURBS hull.
    hull: Hull,
    /// The box of its curve, lower and upper corners.
    reach: ([f64; 3], [f64; 3]),
}

/// A face whose carrier is a plane of the table, ready to place points in.
#[derive(Clone, Debug)]
pub struct FlatFace {
    /// The face, by its index in the arrangement.
    face: usize,
    /// Its plane.
    plane: Plane,
    /// The two coordinates of the plane's picture; the third is the one dropped.
    keep: [usize; 2],
    /// Its boundary edges, loop by loop.
    sides: Vec<Side>,
    /// Its joins: for each vertex of its boundary, the box round the end points of the curves that
    /// meet there, and for each closed edge the box round its two ends.
    joins: Vec<[Interval; 3]>,
    /// The box of its boundary in the plane's picture, lower and upper corners.
    area: ([f64; 2], [f64; 2]),
}

/// The box round two enclosed points.
fn hull(a: [Interval; 3], b: [Interval; 3]) -> Result<[Interval; 3], NumError> {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    let span = |x: Interval, y: Interval| {
        Interval::new(
            if x.lo() < y.lo() { x.lo() } else { y.lo() },
            if x.hi() > y.hi() { x.hi() } else { y.hi() },
        )
    };
    Ok([span(a0, b0)?, span(a1, b1)?, span(a2, b2)?])
}

/// The enclosure of a curve's point at `t`.
fn end_point(
    curve: &impl ParamCurve<3>,
    t: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<[Interval; 3], BuildError> {
    let at = curve
        .bound((t, t), meter)
        .map_err(geom_intersect::IntersectError::from)?;
    let [lx, ly, lz] = at.lo;
    let [hx, hy, hz] = at.hi;
    Ok([
        Interval::new(lx, hx)?,
        Interval::new(ly, hy)?,
        Interval::new(lz, hz)?,
    ])
}

/// A grid point's coordinates.
const fn coords(p: P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

/// A coordinate of three, or zero.
fn coord(v: [i64; 3], k: usize) -> i64 {
    v.get(k).copied().unwrap_or(0)
}

/// The lower bound of the distance from a grid point to a box, less nothing: zero inside it.
fn box_gap(point: P3, (lo, hi): &([f64; 3], [f64; 3])) -> Result<f64, NumError> {
    let mut gaps = [Interval::point(0.0)?; 3];
    for (k, gap) in gaps.iter_mut().enumerate() {
        #[allow(
            clippy::cast_precision_loss,
            reason = "a coordinate is at most 2^40 in size, which a double holds exactly"
        )]
        let here = Interval::point(coord(coords(point), k) as f64)?;
        let (low, high) = (
            Interval::point(lo.get(k).copied().unwrap_or(0.0))?,
            Interval::point(hi.get(k).copied().unwrap_or(0.0))?,
        );
        let below = low.sub(here)?;
        let above = here.sub(high)?;
        let least_gap = if below.lo() > 0.0 {
            below.lo()
        } else if above.lo() > 0.0 {
            above.lo()
        } else {
            0.0
        };
        *gap = Interval::point(least_gap)?;
    }
    Ok(Interval::norm(&gaps)?.lo())
}

/// The least magnitude in an enclosure.
fn least(x: Interval) -> f64 {
    if x.lo() > 0.0 {
        x.lo()
    } else if x.hi() < 0.0 {
        -x.hi()
    } else {
        0.0
    }
}

/// `a` times `b` plus `c` times `d`, in whole numbers.
fn dot2(a: i128, b: i128, c: i128, d: i128) -> Result<i128, NumError> {
    a.checked_mul(b)
        .and_then(|x| x.checked_add(c.checked_mul(d)?))
        .ok_or(NumError::BudgetExceeded)
}

impl FlatFace {
    /// The face `face` of `arr`, ready to place points in: `None` when its carrier is not a plane
    /// of the table.
    ///
    /// # Errors
    ///
    /// [`BuildError::Missing`] for a face, edge or plane that is not there; a boundary curve's
    /// bound's or conversion's errors.
    pub fn new(
        arr: &Arrangement,
        face: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<FlatFace>, BuildError> {
        let this = arr
            .faces
            .get(face)
            .ok_or(BuildError::Missing(Entity::Face(face)))?;
        let CarrierIx::Plane(ix) = this.surface else {
            return Ok(None);
        };
        let plane = arr
            .carriers
            .plane(ix)
            .ok_or(BuildError::Missing(Entity::Face(face)))?;
        // The coordinate the normal is largest in, the first of equals: the picture drops it.
        let normal = plane.n();
        let mut drop = 0_usize;
        for k in 1..3 {
            let (here, best) = (normal.get(k).copied(), normal.get(drop).copied());
            if let (Some(here), Some(best)) = (here, best)
                && here.unsigned_abs() > best.unsigned_abs()
            {
                drop = k;
            }
        }
        let keep = match drop {
            0 => [1, 2],
            1 => [0, 2],
            _ => [0, 1],
        };
        let mut sides = Vec::new();
        let mut at_vertex: BTreeMap<usize, [Interval; 3]> = BTreeMap::new();
        let mut joins = Vec::new();
        for &e in this.loops.iter().flatten() {
            let edge = arr
                .edges
                .get(e)
                .ok_or(BuildError::Missing(Entity::Edge(e)))?;
            let curve = arr
                .carriers
                .curve(edge.curve)
                .ok_or(BuildError::Missing(Entity::Edge(e)))?;
            let (t0, t1) = curve.domain();
            let whole = curve
                .bound((t0, t1), meter)
                .map_err(geom_intersect::IntersectError::from)?;
            let (start, end) = (end_point(curve, t0, meter)?, end_point(curve, t1, meter)?);
            match edge.ends {
                Some(pair) => {
                    for (v, point) in pair.into_iter().zip([start, end]) {
                        let joined = match at_vertex.get(&v) {
                            Some(&box_so_far) => hull(box_so_far, point)?,
                            None => point,
                        };
                        at_vertex.insert(v, joined);
                    }
                }
                None => joins.push(hull(start, end)?),
            }
            sides.push(Side {
                edge: e,
                curve: edge.curve,
                hull: Hull::of(curve, meter)?,
                reach: (whole.lo, whole.hi),
            });
        }
        joins.extend(at_vertex.into_values());
        // The face lies inside its boundary's box, and its picture inside that box's picture.
        let mut area = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for side in &sides {
            for (slot, k) in keep.iter().enumerate() {
                let (lo, hi) = (side.reach.0.get(*k), side.reach.1.get(*k));
                if let (Some(lo), Some(hi), Some(a), Some(b)) =
                    (lo, hi, area.0.get_mut(slot), area.1.get_mut(slot))
                {
                    *a = if *lo < *a { *lo } else { *a };
                    *b = if *hi > *b { *hi } else { *b };
                }
            }
        }
        Ok(Some(FlatFace {
            face,
            plane,
            keep,
            sides,
            joins,
            area,
        }))
    }

    /// Whether the grid point `point`, within `bound` of where its vertex is, lies outside the box
    /// of the face's boundary in the plane's picture by more than the bound: then it is Out, and in
    /// space further from every boundary edge than the bound, so not On either.
    #[must_use]
    pub fn beyond(&self, point: P3, bound: Bound) -> bool {
        #[allow(
            clippy::cast_precision_loss,
            reason = "bounds are counts of steps far below 2^53, and a coordinate at most 2^40"
        )]
        let (slack, at) = (bound.steps() as f64, coords(point).map(|c| c as f64));
        self.keep.iter().enumerate().any(|(slot, k)| {
            let here = at.get(*k).copied().unwrap_or(0.0);
            let (lo, hi) = (
                self.area.0.get(slot).copied().unwrap_or(f64::INFINITY),
                self.area.1.get(slot).copied().unwrap_or(f64::NEG_INFINITY),
            );
            here + slack < lo || here - slack > hi
        })
    }

    /// Whether the box `lo..hi` lies outside the box of the face's boundary in the plane's
    /// picture: then no point of it lies in the face or on its boundary.
    #[must_use]
    pub fn apart_from(&self, lo: [f64; 3], hi: [f64; 3]) -> bool {
        self.keep.iter().enumerate().any(|(slot, k)| {
            let (a, b) = (
                self.area.0.get(slot).copied().unwrap_or(f64::INFINITY),
                self.area.1.get(slot).copied().unwrap_or(f64::NEG_INFINITY),
            );
            let (low, high) = (
                lo.get(*k).copied().unwrap_or(0.0),
                hi.get(*k).copied().unwrap_or(0.0),
            );
            high < a || low > b
        })
    }

    /// The face's plane.
    #[must_use]
    pub fn plane(&self) -> &Plane {
        &self.plane
    }

    /// The boundary edges, in loop order.
    pub fn edges(&self) -> impl Iterator<Item = usize> + '_ {
        self.sides.iter().map(|s| s.edge)
    }

    /// Where the grid point `point`, within `bound` of where its vertex is, lies in the face.
    ///
    /// # Errors
    ///
    /// [`BuildError::Contact`] for a contact the rule cannot decide, or a stopped meter;
    /// [`BuildError::Unclassified`] when no ray direction qualifies.
    pub fn place(
        &self,
        carriers: &Carriers,
        point: P3,
        bound: Bound,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Place, BuildError> {
        #[allow(
            clippy::cast_precision_loss,
            reason = "bounds are counts of steps far below 2^53"
        )]
        let reach = (bound.steps().saturating_add(EPS_FIT.0)) as f64;
        if self.beyond(point, bound) {
            return Ok(Place::Out);
        }
        let mut touched = Vec::new();
        for side in &self.sides {
            if box_gap(point, &side.reach)? > reach {
                continue;
            }
            let carrier = CarrierIx::Curve(side.curve);
            let contact = decide(|level: u32| {
                let distance = carriers.distance(point, carrier, level, meter)?;
                Ok(widened(distance, bound)?)
            })?;
            if let Contact::Touch(_) = contact {
                touched.push(side.edge);
            }
        }
        if !touched.is_empty() {
            return Ok(Place::On(touched));
        }
        for direction in DIRECTIONS {
            if let Some(inside) = self.ray(carriers, point, direction, meter)? {
                return Ok(if inside { Place::In } else { Place::Out });
            }
        }
        Err(BuildError::Unclassified {
            face: self.face,
            at: point,
        })
    }

    /// The parity of the crossings of the ray from `point` along `direction` in the picture, or
    /// `None` when the direction does not qualify.
    fn ray(
        &self,
        carriers: &Carriers,
        point: P3,
        direction: (i64, i64),
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<bool>, BuildError> {
        let [first, second] = self.keep;
        let at = coords(point);
        let (from_first, from_second) =
            (i128::from(coord(at, first)), i128::from(coord(at, second)));
        let (run, rise) = (i128::from(direction.0), i128::from(direction.1));
        let fall = rise.checked_neg().ok_or(NumError::BudgetExceeded)?;
        // The ray's plane holds the dropped axis and the direction: its normal is the direction
        // turned a quarter, (−rise, run), in the picture's coordinates.
        let mut normal = [0_i128; 3];
        if let Some(slot) = normal.get_mut(first) {
            *slot = fall;
        }
        if let Some(slot) = normal.get_mut(second) {
            *slot = run;
        }
        let offset = dot2(fall, from_first, run, from_second)?
            .checked_neg()
            .ok_or(NumError::BudgetExceeded)?;
        let ray_plane = Plane::new(normal, offset)?;
        let enclosed = Enclosed::of(&ray_plane)?;

        #[allow(
            clippy::cast_precision_loss,
            reason = "the direction's parts are at most 3, and the point's coordinates at most 2^40"
        )]
        let (run_f, rise_f, first_f, second_f) = (
            run as f64,
            rise as f64,
            from_first as f64,
            from_second as f64,
        );
        let along = |lo: [f64; 3], hi: [f64; 3]| -> Result<Interval, NumError> {
            let part = |k: usize, scale: f64, origin: f64| -> Result<Interval, NumError> {
                let span = Interval::new(
                    lo.get(k).copied().unwrap_or(0.0),
                    hi.get(k).copied().unwrap_or(0.0),
                )?;
                Interval::point(scale)?.mul(span.sub(Interval::point(origin)?)?)
            };
            part(first, run_f, first_f)?.add(part(second, rise_f, second_f)?)
        };
        // A step, in the units of the ray plane's value and of the distance along the ray: both are
        // lengths times the direction's length.
        let step = Interval::point(run_f)?
            .square()?
            .add(Interval::point(rise_f)?.square()?)?
            .sqrt()?
            .hi();
        // A join within a step of the ray's line, ahead of the point or within a step behind it:
        // the direction fails.
        for join in &self.joins {
            let across = enclosed.value(join)?;
            let ahead = along(join.map(Interval::lo), join.map(Interval::hi))?;
            if least(across) <= step && ahead.hi() >= -step {
                return Ok(None);
            }
        }
        let mut count = 0_u32;
        for side in &self.sides {
            match judge(side.hull.spread(&enclosed)?) {
                // Proven on one side of the ray's plane: no crossing.
                Verdict::Decided(_) => continue,
                // Lying in the ray's plane: the ray runs along it.
                Verdict::Touch => return Ok(None),
                Verdict::Near => {}
            }
            let curve = carriers
                .curve(side.curve)
                .ok_or(BuildError::Missing(Entity::Edge(side.edge)))?;
            let found = crossings(curve, &ray_plane, meter)?;
            for zero in &found.zeros {
                let [param] = zero.enclosure;
                let at = curve
                    .bound((param.lo(), param.hi()), meter)
                    .map_err(geom_intersect::IntersectError::from)?;
                let ahead = along(at.lo, at.hi)?;
                if ahead.lo() > 0.0 {
                    count = count.checked_add(1).ok_or(NumError::BudgetExceeded)?;
                } else if ahead.hi() >= 0.0 {
                    return Ok(None);
                }
            }
            for flag in &found.flagged {
                let [param] = flag.region;
                let at = curve
                    .bound((param.lo(), param.hi()), meter)
                    .map_err(geom_intersect::IntersectError::from)?;
                if along(at.lo, at.hi)?.hi() >= 0.0 {
                    return Ok(None);
                }
            }
        }
        Ok(Some(count % 2 == 1))
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly"
)]
mod tests {
    use super::*;

    fn iv(lo: f64, hi: f64) -> Interval {
        Interval::new(lo, hi).unwrap()
    }

    #[test]
    fn the_hull_of_two_boxes() {
        let a = [iv(0.0, 1.0), iv(5.0, 6.0), iv(-2.0, -1.0)];
        let b = [iv(2.0, 3.0), iv(4.0, 5.5), iv(-1.5, 0.0)];
        let h = hull(a, b).unwrap();
        assert_eq!(
            h.map(|x| (x.lo(), x.hi())),
            [(0.0, 3.0), (4.0, 6.0), (-2.0, 0.0)]
        );
        assert_eq!(
            hull(b, a).unwrap().map(|x| (x.lo(), x.hi())),
            [(0.0, 3.0), (4.0, 6.0), (-2.0, 0.0)]
        );
    }

    #[test]
    fn the_gap_from_a_point_to_a_box() {
        let reach = ([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        assert_eq!(box_gap(P3::new(5, 5, 5).unwrap(), &reach).unwrap(), 0.0);
        let gap = box_gap(P3::new(-3, 14, 5).unwrap(), &reach).unwrap();
        assert!(gap <= 5.0 && gap > 4.999, "{gap}");
        let gap = box_gap(P3::new(13, -4, 5).unwrap(), &reach).unwrap();
        assert!(gap <= 5.0 && gap > 4.999, "{gap}");
    }

    #[test]
    fn the_least_magnitude() {
        assert_eq!(least(iv(2.0, 3.0)), 2.0);
        assert_eq!(least(iv(-3.0, -2.0)), 2.0);
        assert_eq!(least(iv(-1.0, 1.0)), 0.0);
    }
}
