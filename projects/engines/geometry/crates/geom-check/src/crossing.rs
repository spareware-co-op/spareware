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
use geom_intersect::IntersectError;
use geom_intersect::gap;
use geom_intersect::known::{Known, KnownContact, subdivide_known_over};
use geom_intersect::selfcross::{SelfCrossing, self_intersect_over};
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
    let found = self_intersect_over(surface, region, floor, ceiling, meter);
    left_alone(found, meter)
}

/// What a face's search for where it crosses itself leaves: apart only when it found nothing, no
/// branch, flagged or unproven region; undecided when it found anything or failed.
fn left_alone(
    found: Result<SelfCrossing, IntersectError>,
    meter: &WorkMeter<'_>,
) -> Result<Meet, Stopped> {
    match found {
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        clippy::cast_possible_truncation,
        clippy::arithmetic_side_effects,
        clippy::unnecessary_wraps,
        reason = "test code: a failure should stop loudly, and the fixtures' rows are few"
    )]

    use geom_curve::{CurveKind, Line};
    use geom_intersect::general::{FlaggedPair, Settled};
    use geom_intersect::march::Branch;
    use geom_intersect::result::{CurveEnd, Unresolved};
    use geom_math::Box3;
    use geom_num::P3;
    use geom_num::interval::Interval;
    use geom_nurbs::{Basis, Surface};
    use geom_run::Budget;
    use geom_topo::{Builder, Fin, Loop, Sense};

    use super::*;

    fn meter() -> WorkMeter<'static> {
        WorkMeter::new(Budget::UNLIMITED)
    }

    fn ix<K>(row: u32) -> Ix<K> {
        Ix::new(row).unwrap()
    }

    /// S(u, v) = (u, v, 0) over [0, 10]², a bilinear NURBS patch.
    fn floor() -> SurfaceKind {
        let basis = || Basis::new(1, alloc::vec![0.0, 0.0, 10.0, 10.0]).unwrap();
        let net = [
            [0.0, 0.0, 0.0],
            [0.0, 10.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
        ];
        SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap())
    }

    /// The pcurve from `(x, y)` along `(dx, dy)` over `span`.
    fn pline((x, y): (f64, f64), (dx, dy): (f64, f64), span: (f64, f64)) -> CurveKind<2> {
        CurveKind::Line(Line::new([x, y], [dx, dy], span).unwrap())
    }

    /// Every fin's pcurve: the patch's diagonal from (1, 1) to (9, 9).
    fn diagonal(_: usize, _: usize) -> Option<CurveKind<2>> {
        Some(pline((1.0, 1.0), (8.0, 8.0), (0.0, 1.0)))
    }

    /// The rows group 3 reads, never published: face 0 with one loop of fins on `edges[0]`, face
    /// 1 with one on `edges[1]`, both on the floor patch, fin `k` of face `f` with the pcurve
    /// `pcurve(f, k)` gives, and every edge a line with no vertices. With `dangling`, face 0's
    /// loop starts at a fin past the table.
    fn rows(
        edges: [&[u32]; 2],
        pcurve: impl Fn(usize, usize) -> Option<CurveKind<2>>,
        dangling: bool,
    ) -> Builder {
        let count = edges.iter().flat_map(|e| e.iter()).copied().max().unwrap() + 1;
        let (mut faces, mut loops, mut fins, mut pcurves) =
            (Vec::new(), Vec::new(), Vec::<Fin>::new(), Vec::new());
        let bound = Box3::new(P3::new(0, 0, 0).unwrap(), P3::new(10, 10, 0).unwrap()).unwrap();
        for (f, ring) in edges.into_iter().enumerate() {
            let first = fins.len() as u32;
            let n = ring.len() as u32;
            let start = if dangling && f == 0 { 99 } else { first };
            loops.push(Loop {
                face: ix(f as u32),
                next: None,
                first_fin: Some(ix(start)),
            });
            faces.push(Face {
                shell: ix(0),
                next: None,
                surface: ix(f as u32),
                sense: Sense::Forward,
                first_loop: Some(ix(f as u32)),
                bound,
            });
            for (k, &e) in ring.iter().enumerate() {
                let p = pcurve(f, k).map(|c| {
                    pcurves.push(c);
                    ix(pcurves.len() as u32 - 1)
                });
                fins.push(Fin {
                    loop_: ix(f as u32),
                    next: ix(first + (k as u32 + 1) % n),
                    prev: ix(first + (k as u32 + n - 1) % n),
                    edge: ix(e),
                    sense: Sense::Forward,
                    radial: ix(0),
                    pcurve: p,
                });
            }
        }
        let mut rows = Vec::new();
        for e in 0..count {
            let on: Vec<u32> = (0..fins.len() as u32)
                .filter(|&k| fins[k as usize].edge.row() == e)
                .collect();
            for (i, &k) in on.iter().enumerate() {
                fins[k as usize].radial = ix(on[(i + 1) % on.len()]);
            }
            rows.push(Edge {
                first_fin: on.first().map(|&k| ix(k)),
                start: None,
                end: None,
                curve: ix(e),
            });
        }
        let line = || CurveKind::Line(Line::new([0.0; 3], [10.0, 0.0, 0.0], (0.0, 1.0)).unwrap());
        let mut b = Builder::new();
        let a = &mut b.arenas;
        a.faces.extend(&faces).unwrap();
        a.loops.extend(&loops).unwrap();
        a.fins.extend(&fins).unwrap();
        a.edges.extend(&rows).unwrap();
        a.surfaces.extend(alloc::vec![floor(), floor()]).unwrap();
        a.curves
            .extend((0..count).map(|_| line()).collect::<Vec<_>>())
            .unwrap();
        a.pcurves.extend(pcurves).unwrap();
        b
    }

    /// Faces 0 and 1 of [`rows`].
    fn faces() -> (Ix<Face>, Ix<Face>) {
        (ix(0), ix(1))
    }

    #[test]
    fn a_face_with_a_fin_without_its_pcurve_is_undecided() {
        let b = rows(
            [&[0, 1], &[0, 2]],
            |f, k| ((f, k) != (0, 1)).then(|| pline((1.0, 1.0), (8.0, 8.0), (0.0, 1.0))),
            false,
        );
        assert_eq!(
            alone(&b.arenas, ix(0), 1.0, &mut meter()).unwrap(),
            Meet::Unknown
        );
        assert_eq!(
            pair(&b.arenas, faces(), 1.0, &mut meter()).unwrap(),
            Meet::Unknown
        );
    }

    #[test]
    fn a_shared_edge_read_on_two_spans_leaves_the_pair_undecided() {
        let b = rows(
            [&[0, 1], &[0, 2]],
            |f, k| {
                Some(if (f, k) == (1, 0) {
                    pline((1.0, 1.0), (4.0, 4.0), (0.0, 2.0))
                } else {
                    pline((1.0, 1.0), (8.0, 8.0), (0.0, 1.0))
                })
            },
            false,
        );
        let read = (1.0, KnownContact::Crossing);
        assert!(shared(&b.arenas, faces(), read, &mut meter()).is_none());
        assert_eq!(
            pair(&b.arenas, faces(), 1.0, &mut meter()).unwrap(),
            Meet::Unknown
        );
    }

    #[test]
    fn an_edge_a_face_uses_twice_is_one_known_curve() {
        let b = rows([&[0, 0, 1], &[0, 2]], diagonal, false);
        let read = (1.0, KnownContact::Crossing);
        let known = shared(&b.arenas, faces(), read, &mut meter()).unwrap();
        assert_eq!(known.len(), 1);
        assert_eq!(known[0].vertices.len(), CHAIN as usize + 1);
    }

    #[test]
    fn a_fin_with_no_row_shares_no_edge() {
        let b = rows([&[0, 1], &[0, 2]], diagonal, true);
        let read = (1.0, KnownContact::Crossing);
        assert_eq!(
            shared(&b.arenas, faces(), read, &mut meter()).map(|k| k.len()),
            Some(0)
        );
    }

    #[test]
    fn a_bound_that_is_not_a_number_leaves_the_face_and_the_pair_undecided() {
        let b = rows([&[0, 1], &[0, 2]], diagonal, false);
        assert_eq!(
            alone(&b.arenas, ix(0), f64::NAN, &mut meter()).unwrap(),
            Meet::Unknown
        );
        assert_eq!(
            pair(&b.arenas, faces(), f64::NAN, &mut meter()).unwrap(),
            Meet::Unknown
        );
    }

    #[test]
    fn anything_a_search_leaves_keeps_a_face_from_being_apart() {
        let none = || SelfCrossing {
            branches: Vec::new(),
            unproven: Vec::new(),
            flagged: Vec::new(),
        };
        let patch = ((0.0, 1.0), (0.0, 1.0));
        let point = [Interval::point(0.5).unwrap(); 4];
        let branch = Branch {
            vertices: alloc::vec![point],
            ends: [CurveEnd::Flagged, CurveEnd::Flagged],
            deviation: 0.0,
            passes: Vec::new(),
        };
        let settled = Settled {
            first: patch,
            second: patch,
            starts: alloc::vec![point],
        };
        let flag = FlaggedPair {
            first: patch,
            second: patch,
            why: Unresolved::HigherOrderContact,
        };
        let m = meter();
        assert_eq!(left_alone(Ok(none()), &m).unwrap(), Meet::Apart);
        for found in [
            SelfCrossing {
                branches: alloc::vec![branch],
                ..none()
            },
            SelfCrossing {
                flagged: alloc::vec![flag],
                ..none()
            },
            SelfCrossing {
                unproven: alloc::vec![settled],
                ..none()
            },
        ] {
            assert_eq!(left_alone(Ok(found), &m).unwrap(), Meet::Unknown);
        }
        assert_eq!(
            left_alone(Err(IntersectError::InvalidInput), &m).unwrap(),
            Meet::Unknown
        );
        // Once the meter has stopped, a search's error is the stop.
        let mut stopped = WorkMeter::new(Budget::new(0, u64::MAX));
        assert!(stopped.charge(1).is_err());
        assert!(left_alone(Err(IntersectError::InvalidInput), &stopped).is_err());
    }
}
