//! The queries chapter 22 asks of the checker: a body's exactness report and its validity report,
//! each as an operation (22: "exactness class and bounds, validity report"; piece G102).
//!
//! What it does: [`validity`] runs [`check`] at the level asked and returns its [`Report`].
//! [`exactness`] says what the body's record claims and what its geometry allows: the record, how
//! many of its faces lie on planes and how many on other surfaces, how many of its edges on lines
//! and how many on other curves, and how group 4 judged the record against them. A body does not
//! hold its own exactness (24.4 stores it beside the body), so each query is given it.
//!
//! Guarantees:
//!
//! - **The contract.** Each runs inside its context's budget and cancellation, and a stop is that
//!   error, never a partial report; the record comes back with the answer.
//! - **Never changes the body.** Each reads it through `&Body` only.
//! - **Counts in one fixed order.** One unit of work per face and edge counted, on top of what the
//!   checker charges, so the report is the same everywhere.
//!
//! Method: chapter 22's list of queries and chapter 8.3's groups
//! (`projects/engines/geometry/architecture/22-errors-queries-ai.md`,
//! `projects/engines/geometry/architecture/08-topology.md`).
//!
//! Cost: [`check`]'s at the level asked; [`exactness`] adds a pass over the faces and edges and
//! runs the cheap groups. Recursion: none.

use geom_num::Exactness;
use geom_op::{OpContext, OpError, OpOutput};
use geom_topo::entity::{Edge, Face};
use geom_topo::ix::Arena;
use geom_topo::query_ops::PHASE;
use geom_topo::{Body, CurveKind, SurfaceKind};

use crate::combinatorial::rows;
use crate::report::{Level, Report};
use crate::run::check;

/// What [`exactness`] reports about a body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactnessReport {
    /// The class and bound the body is recorded with.
    pub recorded: Exactness,
    /// How many faces lie on planes.
    pub flat_faces: u32,
    /// How many faces lie on other surfaces.
    pub curved_faces: u32,
    /// How many edges lie on lines.
    pub straight_edges: u32,
    /// How many edges lie on other curves.
    pub curved_edges: u32,
    /// The cheap groups' report: group 4 judges the record against the geometry.
    pub report: Report,
}

/// `query.validity`: the report of a check of `body`, recorded `exactness`, at `level`.
///
/// # Errors
///
/// The budget and cancellation errors of the context.
pub fn validity(
    ctx: &OpContext<'_>,
    body: &Body,
    exactness: Exactness,
    level: Level,
) -> Result<OpOutput<Report>, OpError> {
    ctx.run(|meter| {
        let report =
            check(body, exactness, level, meter).map_err(|s| ctx.stopped(meter, s, PHASE))?;
        Ok(OpOutput::new(report, exactness))
    })
}

/// `query.exactness`: what `body`'s record, `recorded`, claims and what its geometry allows.
///
/// # Errors
///
/// The budget and cancellation errors of the context.
pub fn exactness(
    ctx: &OpContext<'_>,
    body: &Body,
    recorded: Exactness,
) -> Result<OpOutput<ExactnessReport>, OpError> {
    ctx.run(|meter| {
        let a = body.arenas();
        let (mut flat_faces, mut curved_faces) = (0_u32, 0_u32);
        for f in rows::<Face>(a.rows(Arena::Face)) {
            meter.charge(1).map_err(|s| ctx.stopped(meter, s, PHASE))?;
            let surface = a.faces.surface.get(f).and_then(|s| a.surfaces.get(*s));
            if matches!(surface, Some(SurfaceKind::Plane(_))) {
                flat_faces = flat_faces.saturating_add(1);
            } else {
                curved_faces = curved_faces.saturating_add(1);
            }
        }
        let (mut straight_edges, mut curved_edges) = (0_u32, 0_u32);
        for e in rows::<Edge>(a.rows(Arena::Edge)) {
            meter.charge(1).map_err(|s| ctx.stopped(meter, s, PHASE))?;
            let curve = a.edges.curve.get(e).and_then(|c| a.curves.get(*c));
            if matches!(curve, Some(CurveKind::Line(_))) {
                straight_edges = straight_edges.saturating_add(1);
            } else {
                curved_edges = curved_edges.saturating_add(1);
            }
        }
        let report =
            check(body, recorded, Level::Cheap, meter).map_err(|s| ctx.stopped(meter, s, PHASE))?;
        Ok(OpOutput::new(
            ExactnessReport {
                recorded,
                flat_faces,
                curved_faces,
                straight_edges,
                curved_edges,
                report,
            },
            recorded,
        ))
    })
}
