//! Group 3 of chapter 8.3 on faces no closed form covers: a face against itself, and two faces
//! against each other, by `geom-intersect`'s searches over each face's part of its surface (G177).
//!
//! What it does: [`face_box`] bounds the part of its surface a face uses by its fins' pcurves.
//! [`alone`] runs `geom_intersect::selfcross::self_intersect_over` on that box. [`pair`] runs
//! `geom_intersect::known::subdivide_known_over` on the two faces' boxes, every edge the two share
//! given as a known curve read from its two fins' pcurves (12.11, G174), as a crossing and, where
//! that tube does not prove, as a tangency.
//!
//! Guarantees:
//!
//! - **Apart only when proven.** [`Meet::Apart`] only when the search leaves nothing: no branch, no
//!   settled pair, no flagged or unproven region. A face can cross itself, or two faces each
//!   other, only where the search finds something, by 12.8 and 12.13's guarantees, and a shared
//!   edge is left out only inside the tube proven about it.
//! - **Never called a crossing on a box.** A face is its loops' part of its box, so what the search
//!   finds in the box may lie outside the face: it is [`Meet::Unknown`], reported unchecked, and
//!   never [`Meet::Cross`] here.
//! - **Fails closed.** A fin without a pcurve, a pcurve on another span than its edge, or a kind's
//!   error leaves the pair [`Meet::Unknown`]; only the meter's stop ends the check.
//!
//! Method: chapter 8.3's group 3 (`projects/engines/geometry/architecture/08-topology.md`), on the
//! searches of chapter 12.8, 12.11 and 12.13
//! (`projects/engines/geometry/architecture/12-intersections.md`).
//!
//! Cost: one self-intersection search per curved face and one known-curve search per pair of
//! curved faces whose boxes meet in space, each as 12.8 costs them, and [`CHAIN`] + 1 pcurve reads
//! per shared edge. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the checker's: a the arenas, f and g the faces, e an edge, n and m fins, p and q pcurves, t a parameter"
)]

use alloc::vec::Vec;

use geom_curve::ParamCurve;
use geom_curve::tight::{larger, smaller};
use geom_intersect::gap;
use geom_intersect::known::{Known, KnownContact, subdivide_known_over};
use geom_intersect::selfcross::self_intersect_over;
use geom_run::{Stopped, WorkMeter};
use geom_topo::entity::{Edge, Face};
use geom_topo::{Arenas, Ix, SurfaceKind};

use crate::combinatorial::{edge_fins, face_fins, fin_edge, fin_face};
use crate::self_intersection::Meet;

/// The vertices a shared edge's known curve is read at, less one: its span cut in this many.
pub const CHAIN: u32 = 16;

/// A part of a surface's parameters, ((u₀, u₁), (v₀, v₁)).
type Region = ((f64, f64), (f64, f64));

/// Face `f`'s surface and the box its fins' pcurves span; `None` where
/// a fin has no pcurve or a pcurve cannot be bounded.
pub(crate) fn face_box<'a>(
    a: &'a Arenas,
    f: Ix<Face>,
    meter: &mut WorkMeter<'_>,
) -> Option<(&'a SurfaceKind, Region)> {
    let surface = a.faces.surface.get(f).and_then(|s| a.surfaces.get(*s))?;
    let mut hull: Option<Region> = None;
    for n in face_fins(a, f) {
        let pcurve = a
            .fins
            .pcurve
            .get(n)
            .copied()
            .flatten()
            .and_then(|p| a.pcurves.get(p))?;
        let b = pcurve.bound(pcurve.domain(), meter).ok()?;
        let [u0, v0] = b.lo;
        let [u1, v1] = b.hi;
        hull = Some(match hull {
            None => ((u0, u1), (v0, v1)),
            Some(((a0, a1), (b0, b1))) => (
                (smaller(a0, u0), larger(a1, u1)),
                (smaller(b0, v0), larger(b1, v1)),
            ),
        });
    }
    // The searches cut it to the surface's cells, so a pcurve's box a rounding past the domain
    // costs nothing.
    hull.map(|region| (surface, region))
}

/// What a search's error leaves: the meter's stop, if it stopped, and otherwise undecided.
fn undecided(meter: &WorkMeter<'_>) -> Result<Meet, Stopped> {
    meter.check()?;
    Ok(Meet::Unknown)
}

/// The floor and ceiling of a search for a body recorded to `limit` steps: a step at least, and a
/// ceiling four floors up.
fn tolerances(limit: f64) -> (f64, f64) {
    let floor = larger(limit, 1.0);
    (floor, 4.0 * floor)
}

/// Whether face `f` crosses itself (module documentation).
pub(crate) fn alone(
    a: &Arenas,
    f: Ix<Face>,
    limit: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Meet, Stopped> {
    let Some((surface, region)) = face_box(a, f, meter) else {
        return Ok(Meet::Unknown);
    };
    let (floor, ceiling) = tolerances(limit);
    match self_intersect_over(surface, region, floor, ceiling, meter) {
        Ok(found)
            if found.branches.is_empty()
                && found.flagged.is_empty()
                && found.unproven.is_empty() =>
        {
            Ok(Meet::Apart)
        }
        Ok(_) => Ok(Meet::Unknown),
        Err(_) => undecided(meter),
    }
}

/// The known curves of the edges faces `f` and `g` share, read from their fins' pcurves as
/// `contact`; `None` where a shared edge's fins have no pcurve on its span.
fn shared(
    a: &Arenas,
    (f, g): (Ix<Face>, Ix<Face>),
    (limit, contact): (f64, KnownContact),
    meter: &mut WorkMeter<'_>,
) -> Option<Vec<Known>> {
    let mut edges: Vec<Ix<Edge>> = Vec::new();
    for n in face_fins(a, f) {
        if let Some(e) = fin_edge(a, n)
            && !edges.contains(&e)
            && edge_fins(a, e).any(|m| fin_face(a, m) == Some(g))
        {
            edges.push(e);
        }
    }
    let mut out = Vec::with_capacity(edges.len());
    for e in edges {
        let on = |face: Ix<Face>| {
            edge_fins(a, e)
                .find(|&m| fin_face(a, m) == Some(face))
                .and_then(|m| a.fins.pcurve.get(m).copied().flatten())
                .and_then(|p| a.pcurves.get(p))
        };
        let (p, q) = (on(f)?, on(g)?);
        let span = p.domain();
        if q.domain() != span {
            return None;
        }
        let (lo, hi) = span;
        let mut vertices = Vec::with_capacity(usize::try_from(CHAIN).ok()?.saturating_add(1));
        for k in 0..=CHAIN {
            let t = if k == CHAIN {
                hi
            } else {
                lo + (hi - lo) * f64::from(k) / f64::from(CHAIN)
            };
            let ([u1, v1], [u2, v2]) = (
                gap::pcurve_at(p, t, meter).ok()?,
                gap::pcurve_at(q, t, meter).ok()?,
            );
            vertices.push([u1, v1, u2, v2]);
        }
        out.push(Known {
            vertices,
            bound: limit,
            contact,
        });
    }
    Some(out)
}

/// Whether faces `f` and `g` meet other than along the edges they share (module documentation).
pub(crate) fn pair(
    a: &Arenas,
    (f, g): (Ix<Face>, Ix<Face>),
    limit: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Meet, Stopped> {
    let (Some((first, one)), Some((second, two))) = (face_box(a, f, meter), face_box(a, g, meter))
    else {
        return Ok(Meet::Unknown);
    };
    let (floor, _) = tolerances(limit);
    for contact in [KnownContact::Crossing, KnownContact::Tangent] {
        let Some(known) = shared(a, (f, g), (limit, contact), meter) else {
            return Ok(Meet::Unknown);
        };
        match subdivide_known_over((first, second), (one, two), &known, floor, meter) {
            Ok(found) if found.settled.is_empty() && found.flagged.is_empty() => {
                return Ok(Meet::Apart);
            }
            Ok(_) => {}
            Err(_) => return undecided(meter),
        }
    }
    Ok(Meet::Unknown)
}
