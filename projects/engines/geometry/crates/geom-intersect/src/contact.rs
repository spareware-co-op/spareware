//! Tangent, singular and coincident analysis: what the flagged pairs of
//! [`crate::general::subdivide`] are (12.9).
//!
//! What it does: [`analyse`] joins the flagged pairs that touch into clusters. Near a contact most
//! of them are flagged for starts that could not be proven, so a cluster is taken whole, and one
//! with no pair at the floor is left as it was flagged. Each other cluster is read:
//!
//! 1. Two surfaces with the same data are one, whatever was flagged.
//! 2. A cluster that covers its patches from both sides, reaches an edge of one of them, and has no
//!    pair whose surfaces are proven to bend apart is an overlap, or near-coincident if what its
//!    boxes reach passes the ceiling.
//! 3. Otherwise the collinear-normal system is solved over the cluster (12.8's step 4). Each proven
//!    zero is read by its gap along the normal, then by the difference of the surfaces' second
//!    fundamental forms in the frame they share: definite is an isolated tangent point, indefinite a
//!    crossing of four half-branches, anything else a contact of higher order. A gap clear of zero
//!    is a near miss. No zero at all proves the cluster free of loops, so each pair settles as
//!    subdivision would have settled it, if all their starts can be proven. Proven points must
//!    account for every branch that ends on the cluster, or none of them stands.
//! 4. Where no zero stands alone, the branches that end on the cluster decide. With none, and the
//!    surfaces proven to bend apart, the contact is a tangent curve in a band of 12.9's width. With
//!    an even number, on two analytic surfaces, it is a singular contact of that many half-branches.
//!    Anything else is a contact of higher order.
//!
//! Guarantees:
//!
//! - **Every flagged pair is accounted for.** Each one is in a cluster with at least one [`Reading`],
//!   or is proven to hold nothing, or is read as it was flagged, and a cluster's points explain all
//!   the branches that reach it.
//! - **Nothing is called tangent on a gap not proven small.** A zero's gap is an enclosure, and only
//!   one that contains zero goes on to the second-order test.
//! - **The form's sign is proven.** The determinant of the forms' difference is an interval. One that
//!   spans zero is a contact of higher order, whatever its middle says.
//! - **A band is an upper bound.** Its width comes from what the cluster's boxes reach and the least
//!   its pairs' forms can differ by, and a band past the ceiling is a [`Unresolved::TangentBand`].
//! - **Overlap is by identity or by a covered area.** Two surfaces are identical only if their data
//!   are equal, and a cluster overlaps only if patches of both surfaces cover its hulls, so that
//!   every point of each has a box of the other within reach, and its hull ends at an edge: a
//!   contact in the open, where the surfaces part on every side, is not an overlap however flat.
//!
//! Method: chapter 12.9 (`projects/engines/geometry/architecture/12-intersections.md`). The forms
//! are Dupin's comparison of two indicatrices, re-derived there, and written here as the difference
//! H₁ − H₂ of each surface's form in one orthonormal frame of the tangent plane: H = A⁻ᵀ·II·A⁻¹, for A
//! the first partials in that frame. Two surfaces at a contact are graphs z = ½xᵀHx over that plane,
//! so H₁ − H₂ is the Hessian of their gap. The chapter's third-order test asks whether branches
//! leave a contact whose form is degenerate. Here that is read directly, from the proven starts of
//! the settled pairs around the cluster, which needs no frame at a contact that cannot be isolated
//! and also reaches the contacts of fourth order and past. The section's limit to analytic pairs
//! stands, because only their data can make a form exactly degenerate; on other pairs a degenerate
//! form is as likely a pair of near crossings, so it stays flagged.
//!
//! What this piece leaves: a tangent curve is reported as a band over its cluster, and its curve is
//! G175's. A contact on a domain's edge is G173's. The half-branches at a crossing are what
//! [`crate::march`] traces to the cluster's edge.
//!
//! Cost: each cluster is one run of [`crate::solve`] on the collinear-normal system, boxes in four
//! unknowns, each a read of both surfaces to the second order; joining and covering are quadratic in
//! the flagged pairs. All of it is charged to the meter. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are chapter 12.9's: S and T the surfaces, n the normal, H the forms"
)]

use alloc::vec::Vec;

use geom_curve::tight::{larger, smaller};
use geom_num::NumError;
use geom_num::grid::Steps;
use geom_num::interval::Interval;
use geom_run::WorkMeter;
use geom_surface::{ParamBox as Region, ParamSurface, SurfaceKind};

use crate::IntersectError;
use crate::curve::exact_steps;
use crate::general::{
    FlaggedPair, Settled, Subdivided, around, cross, ends, may_be_parallel, minus_box, normals,
    partials, point, same, span, speeds, starts_of,
};
use crate::project::{dot, image, negative, positive};
use crate::result::{Coincidence, CurveContact, PointContact, StepsInterval, Test, Unresolved};
use crate::solve::{Field, Solved, Zero, refined, solve};

/// Why a pair is flagged when subdivision ran out of room with its normals still unparted.
const NO_ROOM: Unresolved = Unresolved::UndecidedAtFloor {
    test: Test::LoopFreedom,
};

/// What one cluster of flagged pairs turned out to be.
#[derive(Clone, Debug, PartialEq)]
pub enum Reading {
    /// A contact at one point or in one small box.
    Point {
        /// (u₁, v₁, u₂, v₂).
        at: [Interval; 4],
        /// The gap along the normal, which may be zero.
        gap: Interval,
        /// How the surfaces meet there.
        contact: PointContact,
    },
    /// No contact: arcs cross this pair at a shallow angle, settled for [`crate::march`] to trace.
    Crossing(Settled),
    /// A tangent contact along a curve over these patches.
    Curve {
        /// The patch of the first surface.
        first: Region,
        /// The patch of the second.
        second: Region,
        /// The band and gap.
        contact: CurveContact,
    },
    /// The surfaces coincide over these patches.
    Overlap {
        /// The patch of the first surface.
        first: Region,
        /// The patch of the second.
        second: Region,
        /// By what test.
        coincidence: Coincidence,
    },
    /// What the tests could not decide.
    Undecided {
        /// The patch of the first surface.
        first: Region,
        /// The patch of the second.
        second: Region,
        /// Why.
        why: Unresolved,
    },
}

/// The collinear-normal system: `n₂ ⟂ S₁ᵤ`, `n₂ ⟂ S₁ᵥ`, and the same for `S₁ − S₂` (12.8, step 4).
struct Collinear<'a> {
    first: &'a SurfaceKind,
    second: &'a SurfaceKind,
    floor: f64,
    /// Each parameter's speed: a surface's run in a unit's worth of parameter, a plane's in steps.
    weights: [f64; 4],
}

/// Both surfaces read to the second order.
struct Second {
    gap: [Interval; 3],
    su: [Interval; 3],
    sv: [Interval; 3],
    suu: [Interval; 3],
    suv: [Interval; 3],
    svv: [Interval; 3],
    tu: [Interval; 3],
    tv: [Interval; 3],
    tuu: [Interval; 3],
    tuv: [Interval; 3],
    tvv: [Interval; 3],
    n: [Interval; 3],
    nu: [Interval; 3],
    nv: [Interval; 3],
}

/// `a + b`.
fn plus(a: &[Interval; 3], b: &[Interval; 3]) -> Result<[Interval; 3], NumError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    Ok([a0.add(b0)?, a1.add(b1)?, a2.add(b2)?])
}

impl Collinear<'_> {
    /// Reads both surfaces over `x`, or `None` where they cannot be bounded.
    fn read(
        &self,
        x: &[Interval; 4],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Second>, IntersectError> {
        let [u1, v1, u2, v2] = *x;
        let a = partials(self.first, (ends(u1), ends(v1)), 2, meter)?;
        let b = partials(self.second, (ends(u2), ends(v2)), 2, meter)?;
        let (Some(a), Some(b)) = (a, b) else {
            return Ok(None);
        };
        let ([s, su, sv, suu, suv, svv, ..], [t, tu, tv, tuu, tuv, tvv, ..]) =
            (a.as_slice(), b.as_slice())
        else {
            return Err(IntersectError::Internal);
        };
        Ok(Some(Second {
            gap: minus_box(s, t)?,
            su: *su,
            sv: *sv,
            suu: *suu,
            suv: *suv,
            svv: *svv,
            tu: *tu,
            tv: *tv,
            tuu: *tuu,
            tuv: *tuv,
            tvv: *tvv,
            n: cross(tu, tv)?,
            nu: plus(&cross(tuu, tv)?, &cross(tu, tuv)?)?,
            nv: plus(&cross(tuv, tv)?, &cross(tu, tvv)?)?,
        }))
    }

    /// The four residuals.
    fn residuals(r: &Second) -> Result<[Interval; 4], NumError> {
        Ok([
            dot(&r.n, &r.su)?,
            dot(&r.n, &r.sv)?,
            dot(&r.gap, &r.su)?,
            dot(&r.gap, &r.sv)?,
        ])
    }
}

impl Field<4> for Collinear<'_> {
    fn weights(&self) -> [f64; 4] {
        self.weights
    }

    fn at(&self, x: [f64; 4], meter: &mut WorkMeter<'_>) -> Result<[Interval; 4], IntersectError> {
        let [a, b, c, d] = x;
        let boxed = [point(a)?, point(b)?, point(c)?, point(d)?];
        match self.read(&boxed, meter)? {
            Some(r) => Ok(Collinear::residuals(&r)?),
            None => Err(IntersectError::Internal),
        }
    }

    fn over(
        &self,
        x: &[Interval; 4],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 4]>, IntersectError> {
        match self.read(x, meter)? {
            Some(r) => Ok(Some(Collinear::residuals(&r)?)),
            None => Ok(None),
        }
    }

    fn slope(
        &self,
        x: &[Interval; 4],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[[Interval; 4]; 4]>, IntersectError> {
        let Some(r) = self.read(x, meter)? else {
            return Ok(None);
        };
        Ok(Some([
            [
                dot(&r.n, &r.suu)?,
                dot(&r.n, &r.suv)?,
                dot(&r.nu, &r.su)?,
                dot(&r.nv, &r.su)?,
            ],
            [
                dot(&r.n, &r.suv)?,
                dot(&r.n, &r.svv)?,
                dot(&r.nu, &r.sv)?,
                dot(&r.nv, &r.sv)?,
            ],
            [
                dot(&r.su, &r.su)?.add(dot(&r.gap, &r.suu)?)?,
                dot(&r.sv, &r.su)?.add(dot(&r.gap, &r.suv)?)?,
                dot(&r.tu, &r.su)?.neg(),
                dot(&r.tv, &r.su)?.neg(),
            ],
            [
                dot(&r.su, &r.sv)?.add(dot(&r.gap, &r.suv)?)?,
                dot(&r.sv, &r.sv)?.add(dot(&r.gap, &r.svv)?)?,
                dot(&r.tu, &r.sv)?.neg(),
                dot(&r.tv, &r.sv)?.neg(),
            ],
        ]))
    }

    fn reach(&self, x: &[Interval; 4], meter: &mut WorkMeter<'_>) -> Result<f64, IntersectError> {
        let [u1, v1, u2, v2] = *x;
        let a = around(self.first, (ends(u1), ends(v1)), meter)?;
        let b = around(self.second, (ends(u2), ends(v2)), meter)?;
        Ok(larger(image(&a), image(&b)) / self.floor)
    }
}

/// A unit-length copy of `a`.
fn unit(a: &[Interval; 3]) -> Result<[Interval; 3], NumError> {
    let length = Interval::norm(a)?;
    let [x, y, z] = *a;
    Ok([x.div(length)?, y.div(length)?, z.div(length)?])
}

/// The second fundamental form of a surface in the frame (e₁, e₂, n), as (p, q, r) of the symmetric
/// [[p, q], [q, r]]: H = A⁻ᵀ·II·A⁻¹, for A the first partials' coordinates in that frame.
fn form(
    (su, sv): (&[Interval; 3], &[Interval; 3]),
    (suu, suv, svv): (&[Interval; 3], &[Interval; 3], &[Interval; 3]),
    (e1, e2, n): (&[Interval; 3], &[Interval; 3], &[Interval; 3]),
) -> Result<[Interval; 3], NumError> {
    // x = a·u + c·v, y = b·u + d·v.
    let (a, b) = (dot(su, e1)?, dot(su, e2)?);
    let (c, d) = (dot(sv, e1)?, dot(sv, e2)?);
    let det = a.mul(d)?.sub(b.mul(c)?)?;
    // u = ia·x + ib·y, v = ic·x + id·y.
    let (ia, ib) = (d.div(det)?, c.neg().div(det)?);
    let (ic, id) = (b.neg().div(det)?, a.div(det)?);
    let (e, f, g) = (dot(suu, n)?, dot(suv, n)?, dot(svv, n)?);
    let two = Interval::point(2.0)?;
    Ok([
        ia.square()?
            .mul(e)?
            .add(two.mul(ia)?.mul(ic)?.mul(f)?)?
            .add(ic.square()?.mul(g)?)?,
        ia.mul(ib)?
            .mul(e)?
            .add(ia.mul(id)?.add(ib.mul(ic)?)?.mul(f)?)?
            .add(ic.mul(id)?.mul(g)?)?,
        ib.square()?
            .mul(e)?
            .add(two.mul(ib)?.mul(id)?.mul(f)?)?
            .add(id.square()?.mul(g)?)?,
    ])
}

/// The difference H₁ − H₂ of the two surfaces' forms at `r`, in the frame they share. An error for a
/// box with no tangent plane of its own, at a pole or a crease.
fn difference(r: &Second) -> Result<[Interval; 3], NumError> {
    let n = unit(&cross(&r.su, &r.sv)?)?;
    let e1 = unit(&r.su)?;
    let e2 = cross(&n, &e1)?;
    let frame = (&e1, &e2, &n);
    let one = form((&r.su, &r.sv), (&r.suu, &r.suv, &r.svv), frame)?;
    let two = form((&r.tu, &r.tv), (&r.tuu, &r.tuv, &r.tvv), frame)?;
    let [p, q, s] = one;
    let [t, w, x] = two;
    Ok([p.sub(t)?, q.sub(w)?, s.sub(x)?])
}

/// A length in space, rounded up to whole steps.
fn size(v: f64) -> Steps {
    Steps(exact_steps(v))
}

/// The steps from nothing up to `v`.
fn up_to(v: f64) -> Result<StepsInterval, IntersectError> {
    StepsInterval::new(Steps::ZERO, size(v)).ok_or(IntersectError::Internal)
}

/// A box the floor's size around `zero`, within the box that proves it alone.
fn floor_box(zero: &Zero<4>, system: &Collinear<'_>) -> Result<[Interval; 4], NumError> {
    let fat = |x: Interval, proof: Interval, weight: f64| {
        let room = system.floor / weight;
        Interval::new(
            larger(x.lo() - room, proof.lo()),
            smaller(x.hi() + room, proof.hi()),
        )
    };
    let [a, b, c, d] = zero.enclosure;
    let [p, q, r, s] = zero.proof;
    let [w, x, y, z] = system.weights;
    Ok([fat(a, p, w)?, fat(b, q, x)?, fat(c, r, y)?, fat(d, s, z)?])
}

/// How a proven collinear-normal point reads.
fn read_zero(
    system: &Collinear<'_>,
    zero: &Zero<4>,
    (first, second): (Region, Region),
    meter: &mut WorkMeter<'_>,
) -> Result<Reading, IntersectError> {
    // The gap wants the box as tight as it will go. The forms don't: second partials read over a
    // box that thin are rounding, divided by its width twice, so they read one the floor's size.
    let at = refined(system, zero.enclosure, 0.0, meter)?;
    let higher = Reading::Undecided {
        first,
        second,
        why: Unresolved::HigherOrderContact,
    };
    let (Some(tight), Some(wider)) = (
        system.read(&at, meter)?,
        system.read(&floor_box(zero, system)?, meter)?,
    ) else {
        return Ok(higher);
    };
    let (Ok(n), Ok([p, q, s])) = (unit(&cross(&tight.su, &tight.sv)?), difference(&wider)) else {
        return Ok(higher);
    };
    let gap = dot(&tight.gap, &n)?;
    if positive(gap) || negative(gap) {
        return Ok(Reading::Undecided {
            first,
            second,
            why: Unresolved::NearMiss {
                gap: up_to(larger(gap.lo().abs(), gap.hi().abs()))?,
            },
        });
    }
    let det = p.mul(s)?.sub(q.square()?)?;
    Ok(if positive(det) {
        Reading::Point {
            at,
            gap,
            contact: PointContact::IsolatedTangent,
        }
    } else if negative(det) {
        Reading::Point {
            at,
            gap,
            contact: PointContact::Singular { branches: 4 },
        }
    } else {
        higher
    })
}

/// Whether two regions touch.
fn touch(a: Region, b: Region) -> bool {
    let ((p, q), (r, s)) = a;
    let ((t, w), (x, y)) = b;
    p <= w && t <= q && r <= y && x <= s
}

/// Whether `outer` holds all of `inner`.
fn holds(outer: Region, inner: Region) -> bool {
    let ((p, q), (r, s)) = outer;
    let ((t, w), (x, y)) = inner;
    p <= t && w <= q && r <= x && y <= s
}

/// The hull of two regions.
fn hull(a: Region, b: Region) -> Region {
    let ((p, q), (r, s)) = a;
    let ((t, w), (x, y)) = b;
    ((smaller(p, t), larger(q, w)), (smaller(r, x), larger(s, y)))
}

/// Whether `a` and `b`, two pairs of regions, touch on both surfaces.
fn meets(a: &FlaggedPair, b: &FlaggedPair) -> bool {
    touch(a.first, b.first) && touch(a.second, b.second)
}

/// Whether a pair of `a` touches a pair of `b`.
fn joins(a: &[FlaggedPair], b: &[FlaggedPair]) -> bool {
    a.iter().any(|x| b.iter().any(|y| meets(x, y)))
}

/// The clusters of `flagged` pairs that touch.
fn clusters(
    flagged: &[FlaggedPair],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Vec<FlaggedPair>>, IntersectError> {
    let mut out: Vec<Vec<FlaggedPair>> = Vec::new();
    for pair in flagged {
        let mut mine = alloc::vec![*pair];
        // A pair can join clusters that did not touch before it, so each that it reaches folds in.
        let mut k = 0;
        while let Some(cluster) = out.get(k) {
            meter.charge(1)?;
            if joins(cluster, &mine) {
                mine.extend(out.swap_remove(k));
                k = 0;
            } else {
                k = k.saturating_add(1);
            }
        }
        out.push(mine);
    }
    Ok(out)
}

/// The area of a region in parameters.
fn area(((a, b), (c, d)): Region) -> Result<Interval, NumError> {
    let (a, b) = (Interval::point(a)?, Interval::point(b)?);
    let (c, d) = (Interval::point(c)?, Interval::point(d)?);
    b.sub(a)?.mul(d.sub(c)?)
}

/// Whether `patches` cover `whole`. Subdivision's patches are nested or apart, so the area of those
/// that no other holds is the area of them all, and short of `whole`'s by a hole of at least one
/// patch, far past the rounding that an enclosure of the sum allows for.
fn covered(
    patches: impl Iterator<Item = Region>,
    whole: Region,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, IntersectError> {
    let mut outer: Vec<Region> = Vec::new();
    for patch in patches {
        meter.charge(1)?;
        if !outer.iter().any(|o| holds(*o, patch)) {
            outer.retain(|o| !holds(patch, *o));
            outer.push(patch);
        }
    }
    let mut sum = Interval::point(0.0)?;
    for patch in outer {
        sum = sum.add(area(patch)?)?;
    }
    Ok(sum.hi() >= area(whole)?.lo())
}

/// Whether `region` reaches a side of `domain`.
fn at_edge(region: Region, domain: Region) -> bool {
    let ((a, b), (c, d)) = region;
    let ((p, q), (r, s)) = domain;
    a <= p || b >= q || c <= r || d >= s
}

/// Whether `start` lies on one of the pairs of `cluster`.
fn on(cluster: &[FlaggedPair], start: &[Interval; 4]) -> bool {
    let [u1, v1, u2, v2] = *start;
    let (a, b) = ((ends(u1), ends(v1)), (ends(u2), ends(v2)));
    cluster
        .iter()
        .any(|p| touch(p.first, a) && touch(p.second, b))
}

/// Whether the branch that leaves the pair `k` by way of `start` comes back to `cluster`, walking
/// through the settled pairs of two starts. A pair with more can't be walked, and counts as gone.
fn comes_back(
    settled: &[Settled],
    cluster: &[FlaggedPair],
    (k, start): (usize, [Interval; 4]),
) -> bool {
    let (mut here, mut from) = (k, start);
    for _ in settled {
        let Some([a, b]) = settled.get(here).map(|p| p.starts.as_slice()) else {
            return false;
        };
        let next = if same(a, &from) { *b } else { *a };
        let onward = settled
            .iter()
            .enumerate()
            .find(|(j, q)| *j != here && q.starts.iter().any(|s| same(s, &next)));
        match onward {
            Some((j, _)) => (here, from) = (j, next),
            None => return on(cluster, &next),
        }
    }
    false
}

/// How many branches end on the cluster `pairs`. A start that two settled pairs list is where a
/// branch goes from one to the other, even beside the cluster, so an end is one that a single pair
/// lists. A cluster need not be convex, so a branch can leave it and come back, and that is no end.
fn branches(
    settled: &[Settled],
    pairs: &[FlaggedPair],
    meter: &mut WorkMeter<'_>,
) -> Result<usize, IntersectError> {
    let mut ending: usize = 0;
    for (k, pair) in settled.iter().enumerate() {
        for start in pair.starts.iter().filter(|s| on(pairs, s)) {
            meter.charge(1)?;
            let shared = settled
                .iter()
                .enumerate()
                .any(|(j, q)| j != k && q.starts.iter().any(|s| same(s, start)));
            if !shared && !comes_back(settled, pairs, (k, *start)) {
                ending = ending.saturating_add(1);
            }
        }
    }
    Ok(ending)
}

/// Whether a surface is one of the analytic kinds.
fn analytic(surface: &SurfaceKind) -> bool {
    matches!(
        surface,
        SurfaceKind::Plane(_)
            | SurfaceKind::Sphere(_)
            | SurfaceKind::Cylinder(_)
            | SurfaceKind::Cone(_)
            | SurfaceKind::Torus(_)
    )
}

/// One cluster, with the patches it spans and what its pairs show.
struct Cluster {
    first: Region,
    second: Region,
    /// The largest distance between points of a pair's two patches.
    within: f64,
    /// The least that any pair at the floor has its forms differ by, in their largest term: 0 where
    /// they may not differ.
    bend: f64,
}

/// What `x` keeps away from zero.
fn clear(x: Interval) -> f64 {
    larger(larger(x.lo(), -x.hi()), 0.0)
}

/// The cluster of `pairs`.
fn gather(
    system: &Collinear<'_>,
    pairs: &[FlaggedPair],
    meter: &mut WorkMeter<'_>,
) -> Result<Cluster, IntersectError> {
    let mut members = pairs.iter();
    let lead = members.next().ok_or(IntersectError::Internal)?;
    let (mut first, mut second) = (lead.first, lead.second);
    for pair in members {
        first = hull(first, pair.first);
        second = hull(second, pair.second);
    }
    let (mut within, mut bend) = (0.0, f64::INFINITY);
    for pair in pairs {
        let (a, b) = (
            around(system.first, pair.first, meter)?,
            around(system.second, pair.second, meter)?,
        );
        within = larger(within, image(&a) + image(&b));
        if pair.why != NO_ROOM {
            continue;
        }
        let x = [
            span(pair.first.0)?,
            span(pair.first.1)?,
            span(pair.second.0)?,
            span(pair.second.1)?,
        ];
        let mut most = 0.0;
        if let Some(r) = system.read(&x, meter)?
            && let Ok(d) = difference(&r)
        {
            most = d.into_iter().map(clear).fold(0.0, larger);
        }
        bend = smaller(bend, most);
    }
    Ok(Cluster {
        first,
        second,
        within,
        bend,
    })
}

/// How a cluster with no isolated zero reads, with its branches counted.
fn read_contact(
    system: &Collinear<'_>,
    cluster: &Cluster,
    ending: usize,
    ceiling: f64,
) -> Result<Reading, IntersectError> {
    let (first, second, within) = (cluster.first, cluster.second, cluster.within);
    let undecided = |why| Reading::Undecided { first, second, why };
    if ending == 0 && cluster.bend > 0.0 {
        let quotient = Interval::point(2.0 * within)?.div(Interval::point(cluster.bend)?)?;
        let width = quotient.sqrt()?.hi();
        return Ok(if width <= ceiling {
            Reading::Curve {
                first,
                second,
                contact: CurveContact::Tangent {
                    band: size(width),
                    gap: up_to(within)?,
                },
            }
        } else {
            undecided(Unresolved::TangentBand { width: size(width) })
        });
    }
    let singular = ending.is_multiple_of(2) && analytic(system.first) && analytic(system.second);
    Ok(match u16::try_from(ending) {
        Ok(branches) if ending > 0 && singular => Reading::Point {
            at: [
                span(first.0)?,
                span(first.1)?,
                span(second.0)?,
                span(second.1)?,
            ],
            gap: Interval::new(-within, within)?,
            contact: PointContact::Singular { branches },
        },
        _ => undecided(Unresolved::HigherOrderContact),
    })
}

/// How many half-branches a point contact has.
fn half_branches(contact: PointContact) -> usize {
    match contact {
        PointContact::Singular { branches } => usize::from(branches),
        _ => 0,
    }
}

/// What a first look at a cluster shows.
enum Look {
    /// All there is to say.
    Read(Reading),
    /// Arcs and nothing else, settled, in those of its pairs that any cross.
    Settled(Vec<Settled>),
    /// A contact, for when the clusters around it have settled what they can.
    Contact(Cluster, Solved<4>),
}

/// A first look at a cluster.
fn look(
    system: &Collinear<'_>,
    pairs: &[FlaggedPair],
    ceiling: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Look, IntersectError> {
    let cluster = gather(system, pairs, meter)?;
    let (a, b) = (cluster.first, cluster.second);
    let (first, second) = (system.first, system.second);
    let flat = cluster.bend <= 0.0
        && (at_edge(a, first.domain()) || at_edge(b, second.domain()))
        && covered(pairs.iter().map(|p| p.first), a, meter)?
        && covered(pairs.iter().map(|p| p.second), b, meter)?;
    if flat {
        return Ok(Look::Read(if cluster.within <= ceiling {
            Reading::Overlap {
                first: a,
                second: b,
                coincidence: Coincidence::Within(size(cluster.within)),
            }
        } else {
            Reading::Undecided {
                first: a,
                second: b,
                why: Unresolved::NearCoincident {
                    gap: up_to(cluster.within)?,
                },
            }
        }));
    }
    let domain = [span(a.0)?, span(a.1)?, span(b.0)?, span(b.1)?];
    let keep = |x: &[Interval; 4], meter: &mut WorkMeter<'_>| {
        let [u1, v1, u2, v2] = *x;
        Ok(may_be_parallel(
            &normals(first, (ends(u1), ends(v1)), meter)?,
            &normals(second, (ends(u2), ends(v2)), meter)?,
        )?)
    };
    let solved = solve(system, domain, keep, meter)?;
    if !solved.flagged.is_empty() || !solved.zeros.is_empty() {
        return Ok(Look::Contact(cluster, solved));
    }
    // No two normals agree, so no loop is here, only what crosses the pairs' edges.
    let mut pieces = Vec::new();
    for pair in pairs {
        match starts_of(
            first,
            second,
            (pair.first, pair.second),
            system.floor,
            meter,
        )? {
            Some(starts) if starts.len().is_multiple_of(2) => {
                if !starts.is_empty() {
                    pieces.push(Settled {
                        first: pair.first,
                        second: pair.second,
                        starts,
                    });
                }
            }
            _ => {
                return Ok(Look::Read(Reading::Undecided {
                    first: a,
                    second: b,
                    why: NO_ROOM,
                }));
            }
        }
    }
    Ok(Look::Settled(pieces))
}

/// How a contact reads, given all that is settled around it.
fn read_cluster(
    system: &Collinear<'_>,
    (cluster, solved): (&Cluster, &Solved<4>),
    ending: usize,
    ceiling: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Reading>, IntersectError> {
    if !solved.flagged.is_empty() {
        return Ok(alloc::vec![read_contact(system, cluster, ending, ceiling)?]);
    }
    let (a, b) = (cluster.first, cluster.second);
    let readings = solved
        .zeros
        .iter()
        .map(|zero| read_zero(system, zero, (a, b), meter))
        .collect::<Result<Vec<_>, _>>()?;
    let explained = readings.iter().try_fold(0, |sum: usize, r| match r {
        Reading::Point { contact, .. } => Some(sum.saturating_add(half_branches(*contact))),
        _ => None,
    });
    Ok(match explained {
        Some(count) if count != ending => alloc::vec![Reading::Undecided {
            first: a,
            second: b,
            why: Unresolved::HigherOrderContact,
        }],
        _ => readings,
    })
}

/// What the flagged pairs of `found` are, to a floor of `floor` and a ceiling of `ceiling` in space.
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] for a floor or ceiling that is not positive;
/// [`IntersectError::BudgetExceeded`] and [`IntersectError::Cancelled`];
/// [`IntersectError::UnprovenMaths`] where a kind's bound needs one.
pub fn analyse(
    first: &SurfaceKind,
    second: &SurfaceKind,
    found: &Subdivided,
    floor: f64,
    ceiling: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Reading>, IntersectError> {
    if floor.is_nan() || floor <= 0.0 || ceiling.is_nan() || ceiling <= 0.0 {
        return Err(IntersectError::InvalidInput);
    }
    if first == second {
        return Ok(alloc::vec![Reading::Overlap {
            first: first.domain(),
            second: second.domain(),
            coincidence: Coincidence::Identical,
        }]);
    }
    let ([a, b], [c, d]) = (
        speeds(first, first.domain(), meter)?,
        speeds(second, second.domain(), meter)?,
    );
    let system = Collinear {
        first,
        second,
        floor,
        weights: [a, b, c, d],
    };
    let mut out = Vec::new();
    let mut settled = found.settled.clone();
    let mut contacts = Vec::new();
    for cluster in clusters(&found.flagged, meter)? {
        if !cluster.iter().any(|p| p.why == NO_ROOM) {
            out.extend(cluster.iter().map(|p| Reading::Undecided {
                first: p.first,
                second: p.second,
                why: p.why,
            }));
            continue;
        }
        match look(&system, &cluster, ceiling, meter)? {
            Look::Read(reading) => out.push(reading),
            Look::Settled(pieces) => {
                out.extend(pieces.iter().cloned().map(Reading::Crossing));
                settled.extend(pieces);
            }
            Look::Contact(read, solved) => contacts.push((cluster, read, solved)),
        }
    }
    for (pairs, cluster, solved) in &contacts {
        let ending = branches(&settled, pairs, meter)?;
        out.extend(read_cluster(
            &system,
            (cluster, solved),
            ending,
            ceiling,
            meter,
        )?);
    }
    Ok(out)
}
