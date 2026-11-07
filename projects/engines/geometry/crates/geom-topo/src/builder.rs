//! The builder: the one way to make a body, by editing arenas and publishing them.
//!
//! What it does: a [`Builder`] holds [`Arenas`] to fill or edit: [`Builder::new`] starts empty,
//! [`Builder::edit`] starts from a published body and shares all of its chunks.
//! [`Builder::publish`] checks the arenas and returns them as an immutable [`Body`]: the columns
//! of each arena hold equally many rows, every link points inside the arena it names, and the
//! outside region is a void region. Piece G81 adds the Euler operators, persistent-name order and
//! attributes on top of this; piece G84's checker judges topology (loops close, shells close,
//! Euler's formula), which publishing does not.
//!
//! Guarantees:
//!
//! - **Bodies come only from here.** [`Body`] has no other public constructor, so every body has
//!   passed [`Builder::publish`]'s checks.
//! - **Every link inside the body.** A published body holds no index past the end of the arena it
//!   points into, so following any link of a body never fails.
//! - **One answer, the first fault.** The checks run in a fixed order: each arena's column lengths
//!   from region to vertex, then each link column in [`Link::ALL`]'s order, rows ascending, then the
//!   outside region. The error is the first fault in that order, the same on every machine.
//! - **Edits copy only what they touch.** A builder made by [`Builder::edit`] shares every chunk
//!   with the body; each write copies only the chunk it lands in, so a version made by editing ten
//!   faces of a million-face body shares all but at most ten chunks per column written (23.9).
//! - **Metered.** Publishing charges one work unit for every chunk of every link column it
//!   checks, to the caller's meter, which polls the caller's cancellation (23.10).
//!
//! Method: Parasolid's model structure, made immutable after building (chapter 8.2;
//! `projects/kit/notes/G01-evidence/02-occt-parasolid-acis.md`, section 8), with the validation
//! layer evidence report 03 recommends copying from Fornjot, every object checked as it enters
//! (`projects/kit/notes/G01-evidence/03-rust-kernels-crates.md`, section 2).
//!
//! Cost: [`Builder::new`] and [`Builder::edit`] are linear in the number of chunks (one pointer
//! each); [`Builder::publish`] is linear in the number of rows of the link columns, one comparison
//! each, and allocates nothing. Nothing recurses.

mod euler;
mod named;

pub use euler::{Euler, NewEdge, NewFace, NewVertex, Wire};

use geom_run::WorkMeter;

use crate::arenas::Arenas;
use crate::body::{Body, BodyKind};
use crate::column::{CHUNK_ROWS, Column};
use crate::entity::{Link, RegionIx, RegionKind};
use crate::error::TopoError;
use crate::ix::{Arena, Ix};

/// Arenas being filled or edited, to be published as a body.
#[derive(Clone, Debug, PartialEq)]
pub struct Builder {
    /// The arenas: fill and edit them freely; [`Builder::publish`] checks them.
    pub arenas: Arenas,
}

/// A value a link column holds: an index, or an index that may be absent.
trait Target {
    /// The row it points at, if any.
    fn target(&self) -> Option<u32>;
}

impl<T> Target for Ix<T> {
    fn target(&self) -> Option<u32> {
        Some(self.row())
    }
}

impl<T> Target for Option<Ix<T>> {
    fn target(&self) -> Option<u32> {
        self.map(Ix::row)
    }
}

/// [`TopoError::Ragged`] for `arena` unless every length in `lengths` is `first`.
fn even(arena: Arena, first: u32, lengths: &[u32]) -> Result<(), TopoError> {
    if lengths.iter().all(|&n| n == first) {
        Ok(())
    } else {
        Err(TopoError::Ragged(arena))
    }
}

/// Checks that every index `column` holds is a row of the arena `link` points into, charging one
/// unit to `meter` for each chunk.
fn links<K, V: Target>(
    column: &Column<K, V>,
    link: Link,
    arenas: &Arenas,
    meter: &mut WorkMeter<'_>,
) -> Result<(), TopoError> {
    let rows = arenas.rows(link.to());
    for (value, row) in column.iter().zip(0_u32..) {
        if row % CHUNK_ROWS == 0 {
            meter.charge(1)?;
        }
        if value.target().is_some_and(|target| target >= rows) {
            return Err(TopoError::Dangling { link, row });
        }
    }
    Ok(())
}

/// [`TopoError::Ragged`] for the first arena, from region to edge, whose columns hold different
/// numbers of rows. The vertex arena has one column, so it cannot be ragged.
pub(crate) fn even_columns(arenas: &Arenas) -> Result<(), TopoError> {
    let regions = &arenas.regions;
    even(
        Arena::Region,
        regions.kind.len(),
        &[regions.first_shell.len()],
    )?;
    let shells = &arenas.shells;
    even(
        Arena::Shell,
        shells.region.len(),
        &[shells.next.len(), shells.first_face.len()],
    )?;
    let faces = &arenas.faces;
    even(
        Arena::Face,
        faces.shell.len(),
        &[
            faces.next.len(),
            faces.surface.len(),
            faces.sense.len(),
            faces.first_loop.len(),
            faces.bound.len(),
        ],
    )?;
    let loops = &arenas.loops;
    even(
        Arena::Loop,
        loops.face.len(),
        &[loops.next.len(), loops.first_fin.len()],
    )?;
    let fins = &arenas.fins;
    even(
        Arena::Fin,
        fins.loop_.len(),
        &[
            fins.next.len(),
            fins.prev.len(),
            fins.edge.len(),
            fins.sense.len(),
            fins.radial.len(),
            fins.pcurve.len(),
        ],
    )?;
    let edges = &arenas.edges;
    even(
        Arena::Edge,
        edges.first_fin.len(),
        &[edges.start.len(), edges.end.len(), edges.curve.len()],
    )?;
    Ok(())
}

/// [`TopoError::Dangling`] for the first link, in [`Link::ALL`]'s order and then by row, that points
/// past the end of the arena it names.
pub(crate) fn inside(arenas: &Arenas, meter: &mut WorkMeter<'_>) -> Result<(), TopoError> {
    let (regions, shells, faces) = (&arenas.regions, &arenas.shells, &arenas.faces);
    let (loops, fins, edges) = (&arenas.loops, &arenas.fins, &arenas.edges);
    links(&regions.first_shell, Link::RegionFirstShell, arenas, meter)?;
    links(&shells.region, Link::ShellRegion, arenas, meter)?;
    links(&shells.next, Link::ShellNext, arenas, meter)?;
    links(&shells.first_face, Link::ShellFirstFace, arenas, meter)?;
    links(&faces.shell, Link::FaceShell, arenas, meter)?;
    links(&faces.next, Link::FaceNext, arenas, meter)?;
    links(&faces.surface, Link::FaceSurface, arenas, meter)?;
    links(&faces.first_loop, Link::FaceFirstLoop, arenas, meter)?;
    links(&loops.face, Link::LoopFace, arenas, meter)?;
    links(&loops.next, Link::LoopNext, arenas, meter)?;
    links(&loops.first_fin, Link::LoopFirstFin, arenas, meter)?;
    links(&fins.loop_, Link::FinLoop, arenas, meter)?;
    links(&fins.next, Link::FinNext, arenas, meter)?;
    links(&fins.prev, Link::FinPrev, arenas, meter)?;
    links(&fins.edge, Link::FinEdge, arenas, meter)?;
    links(&fins.radial, Link::FinRadial, arenas, meter)?;
    links(&fins.pcurve, Link::FinPcurve, arenas, meter)?;
    links(&edges.first_fin, Link::EdgeFirstFin, arenas, meter)?;
    links(&edges.start, Link::EdgeStart, arenas, meter)?;
    links(&edges.end, Link::EdgeEnd, arenas, meter)?;
    links(&edges.curve, Link::EdgeCurve, arenas, meter)?;
    links(&arenas.vertices.point, Link::VertexPoint, arenas, meter)?;

    Ok(())
}

impl Builder {
    /// A builder with empty arenas.
    #[must_use]
    pub const fn new() -> Self {
        Builder {
            arenas: Arenas::new(),
        }
    }

    /// A builder holding `body`'s arenas, sharing every chunk with it until written to.
    #[must_use]
    pub fn edit(body: &Body) -> Self {
        Builder {
            arenas: body.arenas().clone(),
        }
    }

    /// Checks the arenas and publishes them as an immutable body of `kind`, whose infinite void
    /// region is `outside`.
    ///
    /// # Errors
    ///
    /// The first fault, in the order of the module's guarantees: [`TopoError::Ragged`] if one
    /// arena's columns hold different numbers of rows; [`TopoError::Dangling`] if a link column
    /// holds an index past the end of the arena it points into; [`TopoError::Outside`] if
    /// `outside` is not a void region; [`TopoError::Stopped`] if the meter stops.
    pub fn publish(
        self,
        kind: BodyKind,
        outside: RegionIx,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Body, TopoError> {
        even_columns(&self.arenas)?;
        inside(&self.arenas, meter)?;
        if self.arenas.regions.kind.get(outside) != Some(&RegionKind::Void) {
            return Err(TopoError::Outside);
        }
        Ok(Body::checked(kind, outside, self.arenas))
    }
}

impl Default for Builder {
    fn default() -> Self {
        Builder::new()
    }
}
