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
//!   checks, and of the meetings of planes in the point pool, which are links too, to the caller's
//!   meter, which polls the caller's cancellation (23.10).
//!
//! Bounded proofs, and what is only tested: Kani proves (piece G112, chapter 28.7) the two checks
//! publishing makes: the link check accepts a column exactly when every index it holds, present
//! or absent, is below the rows of the arena it points into, and otherwise names the first row
//! that is not (every column of two links, any targets, any arena of up to three rows); and the
//! length check accepts an arena exactly when its columns are even (any lengths). That publishing
//! runs each check on the right column against the right arena, for all 22 links and every arena,
//! is tested (`tests/body.rs`, at each arena's boundary), and so are whole bodies, the order of the
//! faults and the meter (`tests/model.rs`): a whole body's geometry pools hold kinds whose
//! constructors Kani cannot get through within a CI budget.
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
use crate::entity::{Link, PointKind, RegionIx, RegionKind};
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
    below(column, link, arenas.rows(link.to()), meter)
}

/// [`TopoError::Dangling`] for the first row of `column` whose index is `rows` or more, the rows of
/// the arena `link` points into; one unit charged to `meter` for each chunk.
fn below<K, V: Target>(
    column: &Column<K, V>,
    link: Link,
    rows: u32,
    meter: &mut WorkMeter<'_>,
) -> Result<(), TopoError> {
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
    meets(arenas, meter)
}

/// [`TopoError::NoSuchRow`] for the first meeting of planes, by row, that names a plane past the
/// end of the plane pool, charging one unit to `meter` for each chunk's worth of meetings, so a
/// body of grid points costs what it did (26.2).
fn meets(arenas: &Arenas, meter: &mut WorkMeter<'_>) -> Result<(), TopoError> {
    let rows = arenas.rows(Arena::Plane);
    let mut seen = 0_u32;
    for point in arenas.points.iter() {
        let PointKind::Meet(planes) = point else {
            continue;
        };
        if seen.is_multiple_of(CHUNK_ROWS) {
            meter.charge(1)?;
        }
        seen = seen.saturating_add(1);
        if let Some(far) = planes.iter().find(|p| p.row() >= rows) {
            return Err(TopoError::NoSuchRow {
                arena: Arena::Plane,
                row: far.row(),
            });
        }
    }
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

#[cfg(kani)]
// cargo-mutants reads this attribute from the source and skips the module (its harnesses are
// proven by Kani, and no test build compiles them); no build evaluates it, since the module
// exists only under Kani, where `test` is off.
#[cfg_attr(test, mutants::skip)]
mod proofs {
    use geom_run::Budget;

    use super::*;
    use crate::entity::{Face, Shell};

    /// The rows of a proof's link column. The count is fixed and the values symbolic: a whole
    /// body's arenas hold geometry whose constructors Kani cannot get through in a CI budget
    /// (their exact sums walk thousands of chunks), so the proofs take the checks themselves.
    const ROWS: u32 = 2;

    /// A row number of 0 to 3: inside an arena of up to three rows, or past it.
    fn small() -> u32 {
        let r: u32 = kani::any();
        kani::assume(r <= 3);
        r
    }

    /// The link check accepts a column exactly when every index it holds, present or absent, is
    /// below the rows of the arena it points into, and otherwise names the first row that is not,
    /// for every column of two links of either kind, any targets and any arena of up to three rows.
    #[kani::proof]
    #[kani::unwind(4)]
    fn the_link_check_accepts_exactly_the_links_inside_their_arena() {
        let rows = small();
        let mut required: Column<Face, Ix<Shell>> = Column::new();
        let mut optional: Column<Face, Option<Ix<Shell>>> = Column::new();
        let mut targets = [0_u32; ROWS as usize];
        let mut present = [false; ROWS as usize];
        for (t, p) in targets.iter_mut().zip(present.iter_mut()) {
            *t = small();
            *p = kani::any();
            let ix = Ix::new(*t);
            assert!(required.push(ix.unwrap_or_else(|| unreachable!())).is_ok());
            assert!(optional.push(if *p { ix } else { None }).is_ok());
        }
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let first = targets.iter().position(|&t| t >= rows);
        assert_eq!(
            below(&required, Link::FaceShell, rows, &mut meter),
            match first {
                Some(row) => Err(TopoError::Dangling {
                    link: Link::FaceShell,
                    row: row as u32,
                }),
                None => Ok(()),
            }
        );
        let first = targets
            .iter()
            .zip(present)
            .position(|(&t, p)| p && t >= rows);
        assert_eq!(
            below(&optional, Link::FaceNext, rows, &mut meter),
            match first {
                Some(row) => Err(TopoError::Dangling {
                    link: Link::FaceNext,
                    row: row as u32,
                }),
                None => Ok(()),
            }
        );
    }

    /// The length check accepts an arena exactly when every column holds as many rows as its first.
    #[kani::proof]
    fn the_length_check_accepts_exactly_even_columns() {
        let first: u32 = kani::any();
        let lengths: [u32; 3] = kani::any();
        let even_all = lengths.iter().all(|&n| n == first);
        assert_eq!(
            even(Arena::Fin, first, &lengths),
            if even_all {
                Ok(())
            } else {
                Err(TopoError::Ragged(Arena::Fin))
            }
        );
    }
}
