//! The entities of a body, region to vertex, each stored as a table of columns.
//!
//! What it does: for each entity of chapter 8.1 a row type ([`Region`], [`Shell`], [`Face`],
//! [`Loop`], [`Fin`], [`Edge`], [`Vertex`]) and its arena ([`Regions`], [`Shells`], [`Faces`],
//! [`Loops`], [`Fins`], [`Edges`], [`Vertices`]): one [`Column`] per field, so a scan reads only
//! the fields it needs (chapter 23.9). Entities refer to each other, and to the geometry pools, by
//! typed index only. [`Link`] names every column that holds an index, with the arena it points
//! from and the arena it points into.
//!
//! The links: a region lists its shells, a shell its faces and a face its loops, each as a first
//! child and a `next` chain; each child names its parent. A loop's fins form a ring both ways
//! (`next` and `prev`), and the fins of one edge form a ring in radial order (`radial`), so a
//! manifold edge's two fins are each other's partner. An edge names one of its fins and its two
//! vertices (none for a closed edge), a face its surface and sense, a fin its pcurve where its face
//! is curved, an edge its 3D curve and a vertex its point. This is the structure Parasolid
//! documents publicly, with explicit fins and regions (8.1).
//!
//! Guarantees:
//!
//! - **Columns, not records.** Every field is a column of its own; a row exists only as the same
//!   index in each column.
//! - **Typed links.** Every link is an [`Ix`] of the arena it points into, and an absent link is
//!   `None` at no extra size.
//! - **Rows are values.** Two arenas are equal when their columns' values are equal in row order,
//!   whatever memory they share.
//! - **Whole rows or an error.** [`Faces::push`] and the others add one value to each column, in
//!   field order. Only a refused allocation can stop them part way, and the arenas are then
//!   ragged, which publishing refuses ([`TopoError::Ragged`]).
//!
//! Method: Parasolid's published model structure: body, region, shell, face, loop, fin, edge and
//! vertex, with fins in radial order around their edge
//! (`projects/kit/notes/G01-evidence/02-occt-parasolid-acis.md`, section 8; chapter 8.1), stored as
//! structure of arrays as Manifold stores its halfedges
//! (`projects/kit/notes/G01-evidence/01-manifold-mesh-booleans.md`, section 2).
//!
//! Recursion:
//!
//! - Types: `Edge`, `Face`, `Fin`, `Loop`, `Region`, `Shell`.
//!   Bound: 1.
//!   Why: they name each other only through `Ix`, a 32-bit row number whose marker
//!   `PhantomData<fn() -> K>` holds no value of its kind, so no row holds another and their drop
//!   glue and derived implementations go one level deep.
//!   Test: `rows_clone_compare_hash_and_print_one_level_deep`.
//!
//! Cost: as [`Column`]'s operations, once per field: a push is amortised constant time per field,
//! a row read is one lookup per field.

use geom_curve::CurveKind;
use geom_math::Box3;
use geom_num::grid::P3;
use geom_num::plane::Plane;
use geom_surface::SurfaceKind;

use crate::column::{Column, Memory};
use crate::error::TopoError;
use crate::ix::{Arena, Ix, Kind};

/// The index of a region.
pub type RegionIx = Ix<Region>;
/// The index of a shell.
pub type ShellIx = Ix<Shell>;
/// The index of a face.
pub type FaceIx = Ix<Face>;
/// The index of a loop.
pub type LoopIx = Ix<Loop>;
/// The index of a fin.
pub type FinIx = Ix<Fin>;
/// The index of an edge.
pub type EdgeIx = Ix<Edge>;
/// The index of a vertex.
pub type VertexIx = Ix<Vertex>;
/// The index of a surface in the surface pool.
pub type SurfaceIx = Ix<SurfaceKind>;
/// The index of a 3D curve in the curve pool.
pub type CurveIx = Ix<CurveKind<3>>;
/// The index of a pcurve, a 2D curve in a face's parameter space, in the pcurve pool.
pub type PcurveIx = Ix<CurveKind<2>>;
/// The index of a place in the point pool.
pub type PointIx = Ix<PointKind>;
/// The index of a plane in the plane pool.
pub type PlaneIx = Ix<Plane>;

/// Where a vertex lies (6.3, 19.2, 24.3): a grid point, or where three of the body's planes meet,
/// kept exactly and never rounded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointKind {
    /// A grid point.
    Grid(P3),
    /// The meeting of three planes of the plane pool.
    Meet([PlaneIx; 3]),
}

impl From<P3> for PointKind {
    fn from(p: P3) -> Self {
        PointKind::Grid(p)
    }
}

impl PointKind {
    /// The grid point, for a vertex that has one.
    #[must_use]
    pub const fn grid(self) -> Option<P3> {
        match self {
            PointKind::Grid(p) => Some(p),
            PointKind::Meet(_) => None,
        }
    }
}

/// Whether a region is material or empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegionKind {
    /// Material: inside the body.
    Solid,
    /// Empty: a void inside the body, or the infinite region outside it.
    Void,
}

/// Which way an entity runs against the geometry it lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sense {
    /// With it: a face's material is on the side its surface's normal points away from; a fin
    /// runs in its edge's direction.
    Forward,
    /// Against it.
    Reversed,
}

/// A region: a connected volume of a body, solid or void (8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Region {
    /// Solid or void.
    pub kind: RegionKind,
    /// The first of the shells bounding it.
    pub first_shell: Option<ShellIx>,
}

/// A shell: a connected set of faces bounding a region (8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Shell {
    /// The region it bounds.
    pub region: RegionIx,
    /// The next shell of the same region.
    pub next: Option<ShellIx>,
    /// The first of its faces.
    pub first_face: Option<FaceIx>,
}

/// A face: a bounded piece of one surface, with a sense (8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Face {
    /// The shell it belongs to.
    pub shell: ShellIx,
    /// The next face of the same shell.
    pub next: Option<FaceIx>,
    /// The surface it lies on.
    pub surface: SurfaceIx,
    /// Which side of the surface is material.
    pub sense: Sense,
    /// The first of its loops.
    pub first_loop: Option<LoopIx>,
    /// A box of grid points that contains the face, for broad phases (19.9, 23.9).
    pub bound: Box3,
}

/// A loop: a closed boundary of a face (8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Loop {
    /// The face it bounds.
    pub face: FaceIx,
    /// The next loop of the same face.
    pub next: Option<LoopIx>,
    /// The first of its fins.
    pub first_fin: Option<FinIx>,
}

/// A fin: the use of an edge by a loop, in one direction (8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fin {
    /// The loop it belongs to.
    pub loop_: LoopIx,
    /// The next fin around the loop.
    pub next: FinIx,
    /// The previous fin around the loop.
    pub prev: FinIx,
    /// The edge it uses.
    pub edge: EdgeIx,
    /// Whether it runs with its edge or against it.
    pub sense: Sense,
    /// The next fin around its edge, in radial order (itself when alone).
    pub radial: FinIx,
    /// Its curve in its face's parameter space, where the face is curved.
    pub pcurve: Option<PcurveIx>,
}

/// An edge: a bounded piece of one curve, between two vertices or closed (8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Edge {
    /// One of its fins: the radial ring starts here.
    pub first_fin: Option<FinIx>,
    /// The vertex it starts at; `None` for a closed edge.
    pub start: Option<VertexIx>,
    /// The vertex it ends at; `None` for a closed edge.
    pub end: Option<VertexIx>,
    /// Its primary geometry, the 3D curve.
    pub curve: CurveIx,
}

/// A vertex: a point of the topology (8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Vertex {
    /// Where it lies.
    pub point: PointIx,
}

impl Kind for Region {
    const ARENA: Arena = Arena::Region;
}
impl Kind for Shell {
    const ARENA: Arena = Arena::Shell;
}
impl Kind for Face {
    const ARENA: Arena = Arena::Face;
}
impl Kind for Loop {
    const ARENA: Arena = Arena::Loop;
}
impl Kind for Fin {
    const ARENA: Arena = Arena::Fin;
}
impl Kind for Edge {
    const ARENA: Arena = Arena::Edge;
}
impl Kind for Vertex {
    const ARENA: Arena = Arena::Vertex;
}
impl Kind for SurfaceKind {
    const ARENA: Arena = Arena::Surface;
}
impl Kind for CurveKind<3> {
    const ARENA: Arena = Arena::Curve;
}
impl Kind for CurveKind<2> {
    const ARENA: Arena = Arena::Pcurve;
}
impl Kind for PointKind {
    const ARENA: Arena = Arena::Point;
}
impl Kind for Plane {
    const ARENA: Arena = Arena::Plane;
}

/// The region arena: one column per field of [`Region`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Regions {
    /// Each region's kind.
    pub kind: Column<Region, RegionKind>,
    /// Each region's first shell.
    pub first_shell: Column<Region, Option<ShellIx>>,
}

impl Regions {
    /// An empty arena.
    #[must_use]
    pub const fn new() -> Self {
        Regions {
            kind: Column::new(),
            first_shell: Column::new(),
        }
    }

    /// The rows of the first column.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.kind.len()
    }

    /// Whether the first column is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.kind.is_empty()
    }

    /// Adds `row` to every column and returns its index in the first.
    ///
    /// # Errors
    ///
    /// As [`Column::push`].
    pub fn push(&mut self, row: Region) -> Result<RegionIx, TopoError> {
        let ix = self.kind.push(row.kind)?;
        self.first_shell.push(row.first_shell)?;
        Ok(ix)
    }

    /// Adds `rows` to every column, reserving exactly.
    ///
    /// # Errors
    ///
    /// As [`Column::extend`].
    pub fn extend(&mut self, rows: &[Region]) -> Result<(), TopoError> {
        self.kind.extend(rows.iter().map(|r| r.kind))?;
        self.first_shell.extend(rows.iter().map(|r| r.first_shell))
    }

    /// The row at `ix`, or `None` past the end of a column.
    #[must_use]
    pub fn get(&self, ix: RegionIx) -> Option<Region> {
        Some(Region {
            kind: *self.kind.get(ix)?,
            first_shell: *self.first_shell.get(ix)?,
        })
    }

    /// What the columns hold in memory.
    #[must_use]
    pub fn memory(&self) -> Memory {
        self.kind.memory().plus(self.first_shell.memory())
    }
}

/// The shell arena: one column per field of [`Shell`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shells {
    /// Each shell's region.
    pub region: Column<Shell, RegionIx>,
    /// Each shell's next shell of the same region.
    pub next: Column<Shell, Option<ShellIx>>,
    /// Each shell's first face.
    pub first_face: Column<Shell, Option<FaceIx>>,
}

impl Shells {
    /// An empty arena.
    #[must_use]
    pub const fn new() -> Self {
        Shells {
            region: Column::new(),
            next: Column::new(),
            first_face: Column::new(),
        }
    }

    /// The rows of the first column.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.region.len()
    }

    /// Whether the first column is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.region.is_empty()
    }

    /// Adds `row` to every column and returns its index in the first.
    ///
    /// # Errors
    ///
    /// As [`Column::push`].
    pub fn push(&mut self, row: Shell) -> Result<ShellIx, TopoError> {
        let ix = self.region.push(row.region)?;
        self.next.push(row.next)?;
        self.first_face.push(row.first_face)?;
        Ok(ix)
    }

    /// Adds `rows` to every column, reserving exactly.
    ///
    /// # Errors
    ///
    /// As [`Column::extend`].
    pub fn extend(&mut self, rows: &[Shell]) -> Result<(), TopoError> {
        self.region.extend(rows.iter().map(|r| r.region))?;
        self.next.extend(rows.iter().map(|r| r.next))?;
        self.first_face.extend(rows.iter().map(|r| r.first_face))
    }

    /// The row at `ix`, or `None` past the end of a column.
    #[must_use]
    pub fn get(&self, ix: ShellIx) -> Option<Shell> {
        Some(Shell {
            region: *self.region.get(ix)?,
            next: *self.next.get(ix)?,
            first_face: *self.first_face.get(ix)?,
        })
    }

    /// What the columns hold in memory.
    #[must_use]
    pub fn memory(&self) -> Memory {
        self.region
            .memory()
            .plus(self.next.memory())
            .plus(self.first_face.memory())
    }
}

/// The face arena: one column per field of [`Face`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Faces {
    /// Each face's shell.
    pub shell: Column<Face, ShellIx>,
    /// Each face's next face of the same shell.
    pub next: Column<Face, Option<FaceIx>>,
    /// Each face's surface.
    pub surface: Column<Face, SurfaceIx>,
    /// Each face's sense.
    pub sense: Column<Face, Sense>,
    /// Each face's first loop.
    pub first_loop: Column<Face, Option<LoopIx>>,
    /// Each face's box.
    pub bound: Column<Face, Box3>,
}

impl Faces {
    /// An empty arena.
    #[must_use]
    pub const fn new() -> Self {
        Faces {
            shell: Column::new(),
            next: Column::new(),
            surface: Column::new(),
            sense: Column::new(),
            first_loop: Column::new(),
            bound: Column::new(),
        }
    }

    /// The rows of the first column.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.shell.len()
    }

    /// Whether the first column is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.shell.is_empty()
    }

    /// Adds `row` to every column and returns its index in the first.
    ///
    /// # Errors
    ///
    /// As [`Column::push`].
    pub fn push(&mut self, row: Face) -> Result<FaceIx, TopoError> {
        let ix = self.shell.push(row.shell)?;
        self.next.push(row.next)?;
        self.surface.push(row.surface)?;
        self.sense.push(row.sense)?;
        self.first_loop.push(row.first_loop)?;
        self.bound.push(row.bound)?;
        Ok(ix)
    }

    /// Adds `rows` to every column, reserving exactly.
    ///
    /// # Errors
    ///
    /// As [`Column::extend`].
    pub fn extend(&mut self, rows: &[Face]) -> Result<(), TopoError> {
        self.shell.extend(rows.iter().map(|r| r.shell))?;
        self.next.extend(rows.iter().map(|r| r.next))?;
        self.surface.extend(rows.iter().map(|r| r.surface))?;
        self.sense.extend(rows.iter().map(|r| r.sense))?;
        self.first_loop.extend(rows.iter().map(|r| r.first_loop))?;
        self.bound.extend(rows.iter().map(|r| r.bound))
    }

    /// The row at `ix`, or `None` past the end of a column.
    #[must_use]
    pub fn get(&self, ix: FaceIx) -> Option<Face> {
        Some(Face {
            shell: *self.shell.get(ix)?,
            next: *self.next.get(ix)?,
            surface: *self.surface.get(ix)?,
            sense: *self.sense.get(ix)?,
            first_loop: *self.first_loop.get(ix)?,
            bound: *self.bound.get(ix)?,
        })
    }

    /// What the columns hold in memory.
    #[must_use]
    pub fn memory(&self) -> Memory {
        self.shell
            .memory()
            .plus(self.next.memory())
            .plus(self.surface.memory())
            .plus(self.sense.memory())
            .plus(self.first_loop.memory())
            .plus(self.bound.memory())
    }
}

/// The loop arena: one column per field of [`Loop`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loops {
    /// Each loop's face.
    pub face: Column<Loop, FaceIx>,
    /// Each loop's next loop of the same face.
    pub next: Column<Loop, Option<LoopIx>>,
    /// Each loop's first fin.
    pub first_fin: Column<Loop, Option<FinIx>>,
}

impl Loops {
    /// An empty arena.
    #[must_use]
    pub const fn new() -> Self {
        Loops {
            face: Column::new(),
            next: Column::new(),
            first_fin: Column::new(),
        }
    }

    /// The rows of the first column.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.face.len()
    }

    /// Whether the first column is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.face.is_empty()
    }

    /// Adds `row` to every column and returns its index in the first.
    ///
    /// # Errors
    ///
    /// As [`Column::push`].
    pub fn push(&mut self, row: Loop) -> Result<LoopIx, TopoError> {
        let ix = self.face.push(row.face)?;
        self.next.push(row.next)?;
        self.first_fin.push(row.first_fin)?;
        Ok(ix)
    }

    /// Adds `rows` to every column, reserving exactly.
    ///
    /// # Errors
    ///
    /// As [`Column::extend`].
    pub fn extend(&mut self, rows: &[Loop]) -> Result<(), TopoError> {
        self.face.extend(rows.iter().map(|r| r.face))?;
        self.next.extend(rows.iter().map(|r| r.next))?;
        self.first_fin.extend(rows.iter().map(|r| r.first_fin))
    }

    /// The row at `ix`, or `None` past the end of a column.
    #[must_use]
    pub fn get(&self, ix: LoopIx) -> Option<Loop> {
        Some(Loop {
            face: *self.face.get(ix)?,
            next: *self.next.get(ix)?,
            first_fin: *self.first_fin.get(ix)?,
        })
    }

    /// What the columns hold in memory.
    #[must_use]
    pub fn memory(&self) -> Memory {
        self.face
            .memory()
            .plus(self.next.memory())
            .plus(self.first_fin.memory())
    }
}

/// The fin arena: one column per field of [`Fin`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fins {
    /// Each fin's loop.
    pub loop_: Column<Fin, LoopIx>,
    /// Each fin's next fin around its loop.
    pub next: Column<Fin, FinIx>,
    /// Each fin's previous fin around its loop.
    pub prev: Column<Fin, FinIx>,
    /// Each fin's edge.
    pub edge: Column<Fin, EdgeIx>,
    /// Each fin's sense along its edge.
    pub sense: Column<Fin, Sense>,
    /// Each fin's next fin around its edge, in radial order.
    pub radial: Column<Fin, FinIx>,
    /// Each fin's pcurve.
    pub pcurve: Column<Fin, Option<PcurveIx>>,
}

impl Fins {
    /// An empty arena.
    #[must_use]
    pub const fn new() -> Self {
        Fins {
            loop_: Column::new(),
            next: Column::new(),
            prev: Column::new(),
            edge: Column::new(),
            sense: Column::new(),
            radial: Column::new(),
            pcurve: Column::new(),
        }
    }

    /// The rows of the first column.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.loop_.len()
    }

    /// Whether the first column is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.loop_.is_empty()
    }

    /// Adds `row` to every column and returns its index in the first.
    ///
    /// # Errors
    ///
    /// As [`Column::push`].
    pub fn push(&mut self, row: Fin) -> Result<FinIx, TopoError> {
        let ix = self.loop_.push(row.loop_)?;
        self.next.push(row.next)?;
        self.prev.push(row.prev)?;
        self.edge.push(row.edge)?;
        self.sense.push(row.sense)?;
        self.radial.push(row.radial)?;
        self.pcurve.push(row.pcurve)?;
        Ok(ix)
    }

    /// Adds `rows` to every column, reserving exactly.
    ///
    /// # Errors
    ///
    /// As [`Column::extend`].
    pub fn extend(&mut self, rows: &[Fin]) -> Result<(), TopoError> {
        self.loop_.extend(rows.iter().map(|r| r.loop_))?;
        self.next.extend(rows.iter().map(|r| r.next))?;
        self.prev.extend(rows.iter().map(|r| r.prev))?;
        self.edge.extend(rows.iter().map(|r| r.edge))?;
        self.sense.extend(rows.iter().map(|r| r.sense))?;
        self.radial.extend(rows.iter().map(|r| r.radial))?;
        self.pcurve.extend(rows.iter().map(|r| r.pcurve))
    }

    /// The row at `ix`, or `None` past the end of a column.
    #[must_use]
    pub fn get(&self, ix: FinIx) -> Option<Fin> {
        Some(Fin {
            loop_: *self.loop_.get(ix)?,
            next: *self.next.get(ix)?,
            prev: *self.prev.get(ix)?,
            edge: *self.edge.get(ix)?,
            sense: *self.sense.get(ix)?,
            radial: *self.radial.get(ix)?,
            pcurve: *self.pcurve.get(ix)?,
        })
    }

    /// What the columns hold in memory.
    #[must_use]
    pub fn memory(&self) -> Memory {
        self.loop_
            .memory()
            .plus(self.next.memory())
            .plus(self.prev.memory())
            .plus(self.edge.memory())
            .plus(self.sense.memory())
            .plus(self.radial.memory())
            .plus(self.pcurve.memory())
    }
}

/// The edge arena: one column per field of [`Edge`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edges {
    /// Each edge's first fin.
    pub first_fin: Column<Edge, Option<FinIx>>,
    /// Each edge's start vertex.
    pub start: Column<Edge, Option<VertexIx>>,
    /// Each edge's end vertex.
    pub end: Column<Edge, Option<VertexIx>>,
    /// Each edge's 3D curve.
    pub curve: Column<Edge, CurveIx>,
}

impl Edges {
    /// An empty arena.
    #[must_use]
    pub const fn new() -> Self {
        Edges {
            first_fin: Column::new(),
            start: Column::new(),
            end: Column::new(),
            curve: Column::new(),
        }
    }

    /// The rows of the first column.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.first_fin.len()
    }

    /// Whether the first column is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.first_fin.is_empty()
    }

    /// Adds `row` to every column and returns its index in the first.
    ///
    /// # Errors
    ///
    /// As [`Column::push`].
    pub fn push(&mut self, row: Edge) -> Result<EdgeIx, TopoError> {
        let ix = self.first_fin.push(row.first_fin)?;
        self.start.push(row.start)?;
        self.end.push(row.end)?;
        self.curve.push(row.curve)?;
        Ok(ix)
    }

    /// Adds `rows` to every column, reserving exactly.
    ///
    /// # Errors
    ///
    /// As [`Column::extend`].
    pub fn extend(&mut self, rows: &[Edge]) -> Result<(), TopoError> {
        self.first_fin.extend(rows.iter().map(|r| r.first_fin))?;
        self.start.extend(rows.iter().map(|r| r.start))?;
        self.end.extend(rows.iter().map(|r| r.end))?;
        self.curve.extend(rows.iter().map(|r| r.curve))
    }

    /// The row at `ix`, or `None` past the end of a column.
    #[must_use]
    pub fn get(&self, ix: EdgeIx) -> Option<Edge> {
        Some(Edge {
            first_fin: *self.first_fin.get(ix)?,
            start: *self.start.get(ix)?,
            end: *self.end.get(ix)?,
            curve: *self.curve.get(ix)?,
        })
    }

    /// What the columns hold in memory.
    #[must_use]
    pub fn memory(&self) -> Memory {
        self.first_fin
            .memory()
            .plus(self.start.memory())
            .plus(self.end.memory())
            .plus(self.curve.memory())
    }
}

/// The vertex arena: one column per field of [`Vertex`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vertices {
    /// Each vertex's point.
    pub point: Column<Vertex, PointIx>,
}

impl Vertices {
    /// An empty arena.
    #[must_use]
    pub const fn new() -> Self {
        Vertices {
            point: Column::new(),
        }
    }

    /// The rows of the first column.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.point.len()
    }

    /// Whether the first column is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.point.is_empty()
    }

    /// Adds `row` to every column and returns its index in the first.
    ///
    /// # Errors
    ///
    /// As [`Column::push`].
    pub fn push(&mut self, row: Vertex) -> Result<VertexIx, TopoError> {
        self.point.push(row.point)
    }

    /// Adds `rows` to every column, reserving exactly.
    ///
    /// # Errors
    ///
    /// As [`Column::extend`].
    pub fn extend(&mut self, rows: &[Vertex]) -> Result<(), TopoError> {
        self.point.extend(rows.iter().map(|r| r.point))
    }

    /// The row at `ix`, or `None` past the end of the column.
    #[must_use]
    pub fn get(&self, ix: VertexIx) -> Option<Vertex> {
        Some(Vertex {
            point: *self.point.get(ix)?,
        })
    }

    /// What the column holds in memory.
    #[must_use]
    pub fn memory(&self) -> Memory {
        self.point.memory()
    }
}

/// A column that holds indices into an arena: the links between entities, and from entities into
/// the geometry pools.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Link {
    /// [`Region::first_shell`].
    RegionFirstShell,
    /// [`Shell::region`].
    ShellRegion,
    /// [`Shell::next`].
    ShellNext,
    /// [`Shell::first_face`].
    ShellFirstFace,
    /// [`Face::shell`].
    FaceShell,
    /// [`Face::next`].
    FaceNext,
    /// [`Face::surface`].
    FaceSurface,
    /// [`Face::first_loop`].
    FaceFirstLoop,
    /// [`Loop::face`].
    LoopFace,
    /// [`Loop::next`].
    LoopNext,
    /// [`Loop::first_fin`].
    LoopFirstFin,
    /// [`Fin::loop_`].
    FinLoop,
    /// [`Fin::next`].
    FinNext,
    /// [`Fin::prev`].
    FinPrev,
    /// [`Fin::edge`].
    FinEdge,
    /// [`Fin::radial`].
    FinRadial,
    /// [`Fin::pcurve`].
    FinPcurve,
    /// [`Edge::first_fin`].
    EdgeFirstFin,
    /// [`Edge::start`].
    EdgeStart,
    /// [`Edge::end`].
    EdgeEnd,
    /// [`Edge::curve`].
    EdgeCurve,
    /// [`Vertex::point`].
    VertexPoint,
}

impl Link {
    /// Every link column, in the order publishing checks them: by arena, then by field.
    pub const ALL: [Link; 22] = [
        Link::RegionFirstShell,
        Link::ShellRegion,
        Link::ShellNext,
        Link::ShellFirstFace,
        Link::FaceShell,
        Link::FaceNext,
        Link::FaceSurface,
        Link::FaceFirstLoop,
        Link::LoopFace,
        Link::LoopNext,
        Link::LoopFirstFin,
        Link::FinLoop,
        Link::FinNext,
        Link::FinPrev,
        Link::FinEdge,
        Link::FinRadial,
        Link::FinPcurve,
        Link::EdgeFirstFin,
        Link::EdgeStart,
        Link::EdgeEnd,
        Link::EdgeCurve,
        Link::VertexPoint,
    ];

    /// The arena whose rows hold the link.
    #[must_use]
    pub const fn from(self) -> Arena {
        match self {
            Link::RegionFirstShell => Arena::Region,
            Link::ShellRegion | Link::ShellNext | Link::ShellFirstFace => Arena::Shell,
            Link::FaceShell | Link::FaceNext | Link::FaceSurface | Link::FaceFirstLoop => {
                Arena::Face
            }
            Link::LoopFace | Link::LoopNext | Link::LoopFirstFin => Arena::Loop,
            Link::FinLoop
            | Link::FinNext
            | Link::FinPrev
            | Link::FinEdge
            | Link::FinRadial
            | Link::FinPcurve => Arena::Fin,
            Link::EdgeFirstFin | Link::EdgeStart | Link::EdgeEnd | Link::EdgeCurve => Arena::Edge,
            Link::VertexPoint => Arena::Vertex,
        }
    }

    /// The arena the link points into.
    #[must_use]
    pub const fn to(self) -> Arena {
        match self {
            Link::ShellRegion => Arena::Region,
            Link::RegionFirstShell | Link::ShellNext | Link::FaceShell => Arena::Shell,
            Link::ShellFirstFace | Link::FaceNext | Link::LoopFace => Arena::Face,
            Link::FaceFirstLoop | Link::LoopNext | Link::FinLoop => Arena::Loop,
            Link::LoopFirstFin
            | Link::FinNext
            | Link::FinPrev
            | Link::FinRadial
            | Link::EdgeFirstFin => Arena::Fin,
            Link::FinEdge => Arena::Edge,
            Link::EdgeStart | Link::EdgeEnd => Arena::Vertex,
            Link::FaceSurface => Arena::Surface,
            Link::EdgeCurve => Arena::Curve,
            Link::FinPcurve => Arena::Pcurve,
            Link::VertexPoint => Arena::Point,
        }
    }

    /// The column's name, `<entity>.<field>`: `face.surface`, `fin.radial`, and so on.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Link::RegionFirstShell => "region.first_shell",
            Link::ShellRegion => "shell.region",
            Link::ShellNext => "shell.next",
            Link::ShellFirstFace => "shell.first_face",
            Link::FaceShell => "face.shell",
            Link::FaceNext => "face.next",
            Link::FaceSurface => "face.surface",
            Link::FaceFirstLoop => "face.first_loop",
            Link::LoopFace => "loop.face",
            Link::LoopNext => "loop.next",
            Link::LoopFirstFin => "loop.first_fin",
            Link::FinLoop => "fin.loop",
            Link::FinNext => "fin.next",
            Link::FinPrev => "fin.prev",
            Link::FinEdge => "fin.edge",
            Link::FinRadial => "fin.radial",
            Link::FinPcurve => "fin.pcurve",
            Link::EdgeFirstFin => "edge.first_fin",
            Link::EdgeStart => "edge.start",
            Link::EdgeEnd => "edge.end",
            Link::EdgeCurve => "edge.curve",
            Link::VertexPoint => "vertex.point",
        }
    }
}

impl core::fmt::Display for Link {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

impl Default for Regions {
    fn default() -> Self {
        Regions::new()
    }
}

impl Default for Shells {
    fn default() -> Self {
        Shells::new()
    }
}

impl Default for Faces {
    fn default() -> Self {
        Faces::new()
    }
}

impl Default for Loops {
    fn default() -> Self {
        Loops::new()
    }
}

impl Default for Fins {
    fn default() -> Self {
        Fins::new()
    }
}

impl Default for Edges {
    fn default() -> Self {
        Edges::new()
    }
}

impl Default for Vertices {
    fn default() -> Self {
        Vertices::new()
    }
}
