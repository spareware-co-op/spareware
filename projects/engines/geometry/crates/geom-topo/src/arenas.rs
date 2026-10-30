//! Every arena of one body: the seven entity arenas and the four geometry pools.
//!
//! What it does: [`Arenas`] holds a body's entities ([`crate::entity`]) and the pools their
//! geometry is kept in: surfaces ([`SurfaceKind`]), 3D curves ([`CurveKind<3>`]), pcurves
//! ([`CurveKind<2>`]) and points ([`P3`]), each a [`Pool`], a column whose rows are the geometry
//! itself. Faces, edges, fins and vertices attach to geometry by index into these pools (8.1).
//! [`crate::Builder`] edits arenas and publishes them as an immutable [`crate::Body`].
//!
//! Guarantees:
//!
//! - **Vectors only.** Every container is a column in row order; there is no map, so nothing can
//!   be iterated in an order that depends on hashing or memory (8.2, 21).
//! - **Cheap versions.** Cloning arenas copies one pointer per chunk; later writes to either copy
//!   only the chunks they land in ([`Column`]).
//! - **Values are all that count.** Two arena sets are equal when every column's values are equal
//!   in row order.
//!
//! Method: one arena per entity kind and one pool per kind of geometry, addressed by typed 32-bit
//! indices, as chapter 8.2 and evidence report 03 lay out
//! (`projects/kit/notes/G01-evidence/03-rust-kernels-crates.md`, section 5).
//!
//! Cost: [`Arenas::rows`] is constant time; [`Arenas::memory`] is linear in the number of chunks.

use geom_curve::CurveKind;
use geom_num::grid::P3;
use geom_surface::SurfaceKind;

use crate::column::{Column, Memory};
use crate::entity::{Edges, Faces, Fins, Loops, Regions, Shells, Vertices};
use crate::ix::Arena;

/// A geometry pool: a column whose rows are the geometry of kind `T` itself.
pub type Pool<T> = Column<T, T>;

/// Every arena of one body.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Arenas {
    /// The regions.
    pub regions: Regions,
    /// The shells.
    pub shells: Shells,
    /// The faces.
    pub faces: Faces,
    /// The loops.
    pub loops: Loops,
    /// The fins.
    pub fins: Fins,
    /// The edges.
    pub edges: Edges,
    /// The vertices.
    pub vertices: Vertices,
    /// The surfaces faces lie on.
    pub surfaces: Pool<SurfaceKind>,
    /// The 3D curves edges lie on.
    pub curves: Pool<CurveKind<3>>,
    /// The pcurves fins lie on, each in its face's parameter space.
    pub pcurves: Pool<CurveKind<2>>,
    /// The points vertices lie at.
    pub points: Pool<P3>,
}

impl Arenas {
    /// Empty arenas.
    #[must_use]
    pub fn new() -> Self {
        Arenas::default()
    }

    /// The rows of `arena` (for an entity arena, the rows of its first column).
    #[must_use]
    pub const fn rows(&self, arena: Arena) -> u32 {
        match arena {
            Arena::Region => self.regions.len(),
            Arena::Shell => self.shells.len(),
            Arena::Face => self.faces.len(),
            Arena::Loop => self.loops.len(),
            Arena::Fin => self.fins.len(),
            Arena::Edge => self.edges.len(),
            Arena::Vertex => self.vertices.len(),
            Arena::Surface => self.surfaces.len(),
            Arena::Curve => self.curves.len(),
            Arena::Pcurve => self.pcurves.len(),
            Arena::Point => self.points.len(),
        }
    }

    /// What every column holds in memory; see [`Memory`].
    #[must_use]
    pub fn memory(&self) -> Memory {
        self.regions
            .memory()
            .plus(self.shells.memory())
            .plus(self.faces.memory())
            .plus(self.loops.memory())
            .plus(self.fins.memory())
            .plus(self.edges.memory())
            .plus(self.vertices.memory())
            .plus(self.surfaces.memory())
            .plus(self.curves.memory())
            .plus(self.pcurves.memory())
            .plus(self.points.memory())
    }
}
