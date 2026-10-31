//! Group 4 of chapter 8.3, the exactness record: the body's exactness class is consistent with
//! its geometry.
//!
//! What it does: a body recorded [`Exactness::Exact`] must be one its geometry can make exact.
//! Chapter 9 makes a B-rep exact for flat faces and certified for curved ones, so every face of an
//! exact body lies on a plane, every edge on a line, and every pcurve it holds is a line; any
//! other kind of carrier is a fault at the entity that uses it. A body recorded
//! [`Exactness::Certified`] or [`Exactness::Approximate`] may use any carrier: its stated bound is
//! judged against the geometry by group 2, which certifies every vertex and edge against its
//! carriers at that bound (11.12), and the record's own arithmetic, the least exact class of the
//! inputs with their bounds added, is `geom_num::Exactness`'s, proven there.
//!
//! Where the record lives: an operation returns it with the body, in `geom_op::OpOutput`'s
//! `exactness`, and the checker takes it beside the body; the native format stores it in the body
//! index (24.2).
//!
//! Guarantees:
//!
//! - **An exact record is flat.** A body recorded exact passes only when every face's surface is
//!   a plane, every edge's curve a line and every pcurve a line.
//! - **Every fault, in a fixed order.** Faces, then edges, then fins, rows ascending.
//! - **Charged.** One work unit per row visited, to the caller's meter.
//!
//! Method: chapter 9's exactness classes and the kinds that keep a B-rep exact
//! (`projects/engines/geometry/architecture/09-representations.md`), checked as chapter 8.3's
//! fourth group (`projects/engines/geometry/architecture/08-topology.md`).
//!
//! Cost: linear in the body's faces, edges and fins; no allocation.

use geom_num::Exactness;
use geom_run::Stopped;
use geom_topo::entity::{Edge, Face, Fin, PcurveIx};
use geom_topo::ix::Arena;
use geom_topo::{Arenas, Body, CurveKind, Ix, SurfaceKind};

use crate::report::{Entity, Fault, FaultKind};
use crate::sink::Sink;

/// Every row of an arena of `rows` rows, ascending.
fn rows<K>(rows: u32) -> impl Iterator<Item = Ix<K>> {
    (0..rows).filter_map(Ix::new)
}

/// Group 4 on `body`, recorded `exactness`.
pub(crate) fn check(
    body: &Body,
    exactness: Exactness,
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
    if exactness != Exactness::Exact {
        return Ok(());
    }
    let a = body.arenas();
    for f in rows::<Face>(a.rows(Arena::Face)) {
        sink.charge()?;
        let flat = a
            .faces
            .surface
            .get(f)
            .and_then(|s| a.surfaces.get(*s))
            .is_some_and(|s| matches!(s, SurfaceKind::Plane(_)));
        if !flat {
            sink.push(Fault::at(FaultKind::NotFlat, Entity::Face(f)));
        }
    }
    for e in rows::<Edge>(a.rows(Arena::Edge)) {
        sink.charge()?;
        let straight = a
            .edges
            .curve
            .get(e)
            .and_then(|c| a.curves.get(*c))
            .is_some_and(|c| matches!(c, CurveKind::Line(_)));
        if !straight {
            sink.push(Fault::at(FaultKind::NotStraight, Entity::Edge(e)));
        }
    }
    for f in rows::<Fin>(a.rows(Arena::Fin)) {
        sink.charge()?;
        if !pcurve_straight(a, a.fins.pcurve.get(f).copied().flatten()) {
            sink.push(Fault::at(FaultKind::CurvedPcurve, Entity::Fin(f)));
        }
    }
    Ok(())
}

/// Whether a fin's pcurve, if it has one, is a line.
fn pcurve_straight(a: &Arenas, pcurve: Option<PcurveIx>) -> bool {
    pcurve.is_none_or(|p| {
        a.pcurves
            .get(p)
            .is_some_and(|c| matches!(c, CurveKind::Line(_)))
    })
}
