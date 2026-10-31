//! The core queries as operations: each in the contract chapter 10.1 gives every operation, with
//! its context's budget and cancellation, its result stating its class, and a typed error (22:
//! "Queries are first-class operations"; piece G102).
//!
//! What it does: [`bounding_box`], [`twice_area`] and [`six_volume`] run `crate::query`'s functions
//! inside [`OpContext::run`]. Each takes the exactness its body is recorded with, since a body does
//! not hold its own (24.4 stores it beside the body), and returns it with the answer: the number is
//! exact on the body's vertices, and the class says how far those vertices may lie from the true
//! shape. [`failed`] turns a [`QueryError`] into the contract's error, for every crate that offers
//! a query.
//!
//! Guarantees:
//!
//! - **The contract.** A stop is the context's budget or cancellation error, never a partial
//!   answer; a body the query cannot read is a typed error with its base kind (22), naming the
//!   edge or face and the piece that adds the route.
//! - **The class is the body's.** Principle 3: a result is as exact as its inputs, so a query never
//!   claims more than the record it was given.
//! - **Charged by row.** One work unit per row visited in [`PHASE`], a schedule fixed by each
//!   operation's version and listed in its schema (23.10).
//!
//! Method: chapter 10.1's contract and chapter 22's rule that queries are operations
//! (`projects/engines/geometry/architecture/10-operations.md`,
//! `projects/engines/geometry/architecture/22-errors-queries-ai.md`), by the methods
//! `crate::query` names.
//!
//! Cost: `crate::query`'s, plus one meter. Recursion: none.

use geom_math::Box3;
use geom_num::Exactness;
use geom_num::wide::I256;
use geom_op::detail::QueryDetail;
use geom_op::{OpContext, OpError, OpOutput};
use geom_run::{Phase, WorkMeter};

use crate::body::Body;
use crate::entity::FaceIx;
use crate::query::{self, QueryError};

/// The phase every query charges in.
pub const PHASE: Phase = Phase::new("query");

/// A body's bounding box, as [`bounding_box`] gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundingBox {
    /// The box.
    pub bound: Box3,
    /// Whether it is the box of the body's vertices alone: every face a polygon on a plane.
    pub tight: bool,
}

/// The contract's error for `error`, at `ctx`'s path, with `meter`'s record of a stop.
#[must_use]
pub fn failed(ctx: &OpContext<'_>, meter: &WorkMeter<'_>, error: QueryError) -> OpError {
    let detail = match error {
        QueryError::Stopped(stop) => return ctx.stopped(meter, stop, PHASE),
        QueryError::NoSuchRow | QueryError::Ring => QueryDetail::Malformed,
        QueryError::Curved(edge) => QueryDetail::CurvedEdge { edge: edge.row() },
        QueryError::NotPlanar(face) => QueryDetail::CurvedFace { face: face.row() },
        QueryError::Loopless(face) => QueryDetail::NoLoops { face: face.row() },
        QueryError::NotSolid => QueryDetail::NotSolid,
        QueryError::Overflow => QueryDetail::Overflow,
    };
    OpError::new(ctx.path().clone(), detail)
}

/// `query.bounding_box`: the box of `body`, recorded `exactness`; `None` for a body with nothing
/// in it.
///
/// # Errors
///
/// [`QueryDetail::Malformed`], and the budget and cancellation errors of the context.
pub fn bounding_box(
    ctx: &OpContext<'_>,
    body: &Body,
    exactness: Exactness,
) -> Result<OpOutput<Option<BoundingBox>>, OpError> {
    ctx.run(|meter| {
        let boxed = query::bounding_box(body, meter).map_err(|e| failed(ctx, meter, e))?;
        let answer = boxed.map(|(bound, tight)| BoundingBox { bound, tight });
        Ok(OpOutput::new(answer, exactness))
    })
}

/// `query.area`: twice the area vector of flat face `face` of `body`, recorded `exactness`.
///
/// # Errors
///
/// [`QueryDetail::CurvedEdge`], [`QueryDetail::CurvedFace`], [`QueryDetail::NoLoops`],
/// [`QueryDetail::Malformed`], and the budget and cancellation errors of the context.
pub fn twice_area(
    ctx: &OpContext<'_>,
    body: &Body,
    face: FaceIx,
    exactness: Exactness,
) -> Result<OpOutput<[i128; 3]>, OpError> {
    ctx.run(|meter| {
        let area = query::face_area(body, face, meter).map_err(|e| failed(ctx, meter, e))?;
        Ok(OpOutput::new(area, exactness))
    })
}

/// `query.volume`: six times the volume of flat solid `body`, recorded `exactness`.
///
/// # Errors
///
/// [`QueryDetail::NotSolid`], the details of [`twice_area`], and the budget and cancellation errors
/// of the context.
pub fn six_volume(
    ctx: &OpContext<'_>,
    body: &Body,
    exactness: Exactness,
) -> Result<OpOutput<I256>, OpError> {
    ctx.run(|meter| {
        let volume = query::volume(body, meter).map_err(|e| failed(ctx, meter, e))?;
        Ok(OpOutput::new(volume, exactness))
    })
}
