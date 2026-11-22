//! The exact mesh: a surface of flat polygons, every one of them exact data on the grid (19.2).
//!
//! What it does: [`ExactMesh`] holds polygons with holes as parallel arrays, half-edge by half-edge.
//! Every face points to its supporting plane and every half-edge to an edge plane, a second plane
//! through that edge, in one sorted pool of canonical planes. A vertex is a grid point or the
//! meeting of three planes of the pool. [`build`] makes a mesh from polygons, [`check`] is the
//! mesh checker group, and [`mass`] gives its volume and areas. [`raw`] holds what an import reads
//! before it is healed.
//!
//! Guarantees:
//!
//! - **Only checked meshes exist.** [`ExactMesh::new`] is the one way to make one, and it runs the
//!   whole checker group: a value of the type has passed every test of [`check`].
//! - **A mesh's bytes depend only on its content.** The planes are sorted by coefficients, the
//!   vertices by key, the runs by source and role, the faces in a run by their keys; each loop
//!   starts at its lowest vertex, and the holes follow in key order. Nothing is ordered by a float,
//!   an address or a hash, and the checker refuses any other order.
//! - **One point, one form.** An implicit vertex whose coordinates are whole grid steps is stored
//!   as a grid point, and no two vertices are one point.
//! - **Properties decide nothing.** A property vertex carries display data only; no test here
//!   reads one, and [`Parts::props`] holds the only floats in a mesh.
//!
//! Method: chapter 19.2's data model
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`): parallel half-edge
//! arrays, property vertices and provenance runs after Manifold, with polygons, plane-based
//! corners after EMBER (<https://doi.org/10.1145/3528223.3530181>) and exact radial pairing added.
//!
//! Consulted: Manifold, Apache-2.0,
//!  <https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/src/shared.h>,
//!  for half-edges as parallel arrays, and
//!  <https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/include/manifold/mesh.h>,
//!  for property vertices and runs; its data model, not its code.
//!
//! Cost: [`Parts`] holds about 60 bytes per half-edge and 40 per plane. A mesh is immutable; an
//! operation makes a new one.

#![allow(
    clippy::many_single_char_names,
    reason = "a, b, c and d are spreading steps, as in the bit-interleaving formula"
)]

use alloc::vec::Vec;
use core::fmt;
use core::ops::Range;

use geom_bvh::BvhError;
use geom_num::grid::{P3, Steps};
use geom_num::plane::Plane;
use geom_num::{Exactness, NumError};
use geom_op::NameId;
use geom_poly::merge::MergeError;
use geom_run::{ChunkPlan, Exceeded, Exec, RunError, Stop, Stopped, WorkMeter};

pub mod build;
pub mod check;
pub mod mass;
pub mod raw;

/// What a half-edge's pair or property holds where it has none.
pub const NONE: u32 = u32::MAX;

/// Faces and planes to a chunk of the parallel helper: enough that a chunk outweighs its handling,
/// few enough that a million faces share out over any number of threads.
pub(crate) const PLAN: ChunkPlan = ChunkPlan::fixed::<1024>();

/// A vertex: a grid point, or the meeting of three planes of the pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vertex {
    /// A grid point.
    Grid(P3),
    /// The meeting of three planes, by their rows in the pool, rising.
    Planes([u32; 3]),
}

/// A plane as a face or an edge holds it: its row in the pool, and whether it looks the other way.
///
/// A face's outward normal is its plane's normal, turned if `flipped`. An edge plane faces into its
/// face: the face lies on the side the plane, turned if `flipped`, looks to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Facing {
    /// The row in the pool.
    pub row: u32,
    /// Whether it looks the other way from the pool's plane.
    pub flipped: bool,
}

/// What a mesh is as a surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MeshKind {
    /// Every edge has two half-edges, one each way.
    Closed,
    /// As `Closed`, but for boundary edges, which have one.
    Open,
    /// Some edge has more than two half-edges: a non-manifold body.
    General,
}

/// What a run's source was to the mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// One B-rep face.
    Face,
    /// One set of faces, such as an IFC face set.
    FaceSet,
    /// One operand of a boolean.
    Operand,
}

/// Where a contiguous range of faces came from (19.2, "Provenance runs").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    /// The faces.
    pub faces: Range<u32>,
    /// The source's persistent name.
    pub source: NameId,
    /// What the source was.
    pub role: Role,
    /// The caller's flags, kept and never read here.
    pub flags: u32,
}

/// Display data, row by row: normals, texture coordinates, colours.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropTable {
    /// The values in one row.
    pub width: u32,
    /// The rows, one after another.
    pub values: Vec<f32>,
}

impl PropTable {
    /// How many rows there are.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.values
            .len()
            .checked_div(self.width as usize)
            .unwrap_or(0)
    }
}

/// The arrays of a mesh, as 19.2 sets them out.
#[derive(Clone, Debug, PartialEq)]
pub struct Parts {
    /// The planes, canonical and sorted by coefficients.
    pub planes: Vec<Plane>,
    /// The vertices, sorted by key: grid points by Morton code, then three-plane points by rows.
    pub verts: Vec<Vertex>,
    /// Each face's supporting plane.
    pub face_plane: Vec<Facing>,
    /// The loops of face `f` are `face_loops[f]..face_loops[f + 1]`; the first is the outer loop.
    pub face_loops: Vec<u32>,
    /// The half-edges of loop `l` are `loop_start[l]..loop_start[l + 1]`, in order.
    pub loop_start: Vec<u32>,
    /// Each half-edge's start vertex; its end is the next one's start in its loop.
    pub he_vert: Vec<u32>,
    /// Each half-edge's edge plane.
    pub he_plane: Vec<Facing>,
    /// Each half-edge's opposite, [`NONE`] on a boundary; in exact radial order on an edge of more
    /// than two half-edges.
    pub he_pair: Vec<u32>,
    /// Each half-edge's row of [`Self::props`], or [`NONE`].
    pub he_prop: Vec<u32>,
    /// Display data only.
    pub props: PropTable,
    /// The sources, sorted by name and role, covering the faces in order.
    pub runs: Vec<Run>,
    /// Each face's proven distance from its source, in the steps of [`Self::class`]; 0 when exact.
    pub face_dev: Vec<Steps>,
    /// How far the mesh is from what it stands for.
    pub class: Exactness,
    /// What it is as a surface.
    pub kind: MeshKind,
}

/// An exact polygon mesh: every face flat, every vertex a grid point or a meeting of three planes.
/// Two meshes of the same arrays are equal, whether or not either is known to be a solid, and
/// print the same: the flag is what is known of a mesh, not part of it.
#[derive(Clone)]
pub struct ExactMesh {
    parts: Parts,
    /// Whether it is known to bound a solid ([`ExactMesh::is_solid`]).
    solid: bool,
}

impl PartialEq for ExactMesh {
    fn eq(&self, other: &Self) -> bool {
        self.parts == other.parts
    }
}

#[allow(
    clippy::missing_fields_in_debug,
    reason = "the solid flag is what is known of a mesh, not part of it, as PartialEq has it"
)]
impl fmt::Debug for ExactMesh {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExactMesh")
            .field("parts", &self.parts)
            .finish()
    }
}

impl ExactMesh {
    /// The mesh `parts` describe, once every test of [`check::check`] has passed.
    ///
    /// # Errors
    ///
    /// As for [`check::check`].
    pub fn new(
        parts: Parts,
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Self, MeshError> {
        check::check(&parts, exec, meter)?;
        Ok(Self {
            parts,
            solid: false,
        })
    }

    /// Whether the mesh is known to bound a solid: no two of its faces cross, and its winding
    /// number is 1 just behind every face and 0 just in front, EMBER's no-self-intersection and
    /// no-nested-parts flags (11.3 step 4), so a boolean need neither cut nor trace where only its
    /// faces are. Only what proves it says so: a boolean's result is one by construction, its
    /// winding the test's 0 or 1. A mesh made from arrays says `false`.
    #[must_use]
    pub const fn is_solid(&self) -> bool {
        self.solid
    }

    /// `self`, known to bound a solid: for the constructions that prove it.
    pub(crate) fn proven_solid(self) -> Self {
        Self {
            solid: true,
            ..self
        }
    }

    /// The arrays.
    #[must_use]
    pub const fn parts(&self) -> &Parts {
        &self.parts
    }

    /// The arrays, for a reader that means to change them and check them again.
    #[must_use]
    pub fn into_parts(self) -> Parts {
        self.parts
    }

    /// How many faces there are.
    #[must_use]
    pub fn faces(&self) -> usize {
        self.parts.face_plane.len()
    }

    /// What it is as a surface.
    #[must_use]
    pub const fn kind(&self) -> MeshKind {
        self.parts.kind
    }
}

/// What a mesh broke (19.2, "What it guarantees"). Each names the first place it was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshFault {
    /// The arrays do not fit together.
    Shape,
    /// This plane is not in canonical form.
    PlaneNotCanonical {
        /// Its row.
        row: u32,
    },
    /// This plane does not sort above the one before it.
    PlanesOutOfOrder {
        /// Its row.
        row: u32,
    },
    /// This vertex does not sort above the one before it.
    VerticesOutOfOrder {
        /// The vertex.
        vertex: u32,
    },
    /// This vertex's three planes do not meet in a point.
    PlanesDoNotMeet {
        /// The vertex.
        vertex: u32,
    },
    /// This three-plane vertex is a grid point, and belongs in that form.
    ImplicitOnTheGrid {
        /// The vertex.
        vertex: u32,
    },
    /// These two vertices are one point.
    RepeatedVertex {
        /// The lower.
        first: u32,
        /// The higher.
        second: u32,
    },
    /// This loop has fewer than three half-edges.
    ShortLoop {
        /// The loop.
        looped: u32,
    },
    /// This half-edge starts and ends at one vertex.
    ZeroEdge {
        /// The half-edge.
        half_edge: u32,
    },
    /// This loop does not start at its lowest vertex.
    LoopStart {
        /// The loop.
        looped: u32,
    },
    /// This face's holes are not in key order.
    HolesOutOfOrder {
        /// The face.
        face: u32,
    },
    /// This face does not sort above the one before it in its run.
    FacesOutOfOrder {
        /// The face.
        face: u32,
    },
    /// This run does not sort above the one before it.
    RunsOutOfOrder {
        /// The run.
        run: u32,
    },
    /// These two faces use the same vertices in the same order.
    RepeatedFace {
        /// The lower.
        first: u32,
        /// The higher.
        second: u32,
    },
    /// This face has no area.
    DegenerateFace {
        /// The face.
        face: u32,
    },
    /// This half-edge's start is not on its face's plane.
    OffFacePlane {
        /// The half-edge.
        half_edge: u32,
    },
    /// An end of this half-edge is not on its edge plane.
    OffEdgePlane {
        /// The half-edge.
        half_edge: u32,
    },
    /// This face's corners turn the other way from its plane.
    WrongWayRound {
        /// The face.
        face: u32,
    },
    /// This half-edge's edge plane is its face's plane, or looks away from its face.
    EdgePlaneFacing {
        /// The half-edge.
        half_edge: u32,
    },
    /// This half-edge's pair is not its opposite.
    BadPair {
        /// The half-edge.
        half_edge: u32,
    },
    /// This half-edge's edge is not used equally often each way.
    UnevenEdge {
        /// The half-edge.
        half_edge: u32,
    },
    /// The half-edges around this one's edge are not paired in radial order.
    RadialOrder {
        /// The half-edge.
        half_edge: u32,
    },
    /// The mesh is not the kind it says.
    WrongKind,
    /// This face's deviation is more than the mesh's class allows.
    Deviation {
        /// The face.
        face: u32,
    },
}

impl fmt::Display for MeshFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Why a mesh could not be made or measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshError {
    /// The mesh breaks a rule of the checker group.
    Fault(MeshFault),
    /// This face has a corner off the grid and came without its plane.
    FacePlaneNeeded {
        /// The face, as it was given.
        face: u32,
    },
    /// This corner starts or ends an edge off the grid and came without its edge plane.
    EdgePlaneNeeded {
        /// The corner, as it was given.
        corner: u32,
    },
    /// More items than 32-bit indices can name.
    Full,
    /// Inside and outside are undefined for an open mesh. Heal it first.
    NotClosed,
    /// A boolean's pieces would not merge, which closed inputs should never cause.
    Merge(MergeError),
    /// The outside passes this test of a boolean, so its result has no finite boundary. Intersect
    /// it with a box first.
    UnboundedResult {
        /// The test, by its place in the list given.
        test: u32,
    },
    /// Planes to slice by must be parallel and rise along their normal.
    SlicesOutOfOrder,
    /// The box tree refused.
    Bvh(BvhError),
    /// The numbers layer refused: a plane over budget, or a fault of its own.
    Num(NumError),
    /// The meter stopped: its budget ran out, the caller cancelled, or memory was refused.
    Stopped(Stop),
    /// A phase went over its budget, by `geom-run`'s phase rule.
    BudgetExceeded(Exceeded),
    /// An allocation was refused.
    OutOfMemory,
    /// A step the helper's own rules exclude failed: a defect, reported, never answered.
    Defect,
}

impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeshError::Fault(fault) => write!(f, "invalid mesh: {fault}"),
            MeshError::FacePlaneNeeded { face } => {
                write!(f, "face {face} needs its plane")
            }
            MeshError::EdgePlaneNeeded { corner } => {
                write!(f, "corner {corner} needs its edge plane")
            }
            MeshError::Full => f.write_str("more items than 32-bit indices can name"),
            MeshError::NotClosed => f.write_str("an open mesh has no inside: heal it first"),
            MeshError::Merge(e) => write!(f, "{e}"),
            MeshError::UnboundedResult { test } => write!(
                f,
                "test {test} passes outside everything, so its result has no boundary: intersect it with a box first"
            ),
            MeshError::SlicesOutOfOrder => {
                f.write_str("planes to slice by must be parallel and rise along their normal")
            }
            MeshError::Bvh(e) => write!(f, "{e}"),
            MeshError::Num(e) => write!(f, "{e}"),
            MeshError::Stopped(e) => write!(f, "the mesh stopped: {e}"),
            MeshError::BudgetExceeded(e) => write!(f, "the mesh: {e}"),
            MeshError::OutOfMemory => f.write_str("an allocation was refused"),
            MeshError::Defect => f.write_str("a step the helper's rules exclude failed (a defect)"),
        }
    }
}

impl core::error::Error for MeshError {}

impl From<MeshFault> for MeshError {
    fn from(e: MeshFault) -> MeshError {
        MeshError::Fault(e)
    }
}

impl From<MergeError> for MeshError {
    fn from(e: MergeError) -> Self {
        match e {
            MergeError::Num(e) => MeshError::Num(e),
            MergeError::Stopped(s) => s.into(),
            other => MeshError::Merge(other),
        }
    }
}

impl From<BvhError> for MeshError {
    fn from(e: BvhError) -> Self {
        MeshError::Bvh(e)
    }
}

impl From<NumError> for MeshError {
    fn from(e: NumError) -> MeshError {
        MeshError::Num(e)
    }
}

impl From<Stopped> for MeshError {
    fn from(e: Stopped) -> MeshError {
        MeshError::Stopped(e.reason())
    }
}

impl From<RunError<MeshError>> for MeshError {
    fn from(error: RunError<MeshError>) -> MeshError {
        match error {
            RunError::Failed(e) => e,
            RunError::BudgetExceeded(exceeded) => MeshError::BudgetExceeded(exceeded),
            RunError::Cancelled => MeshError::Stopped(Stop::Cancelled),
            RunError::OutOfMemory => MeshError::OutOfMemory,
            RunError::Internal(_) => MeshError::Defect,
        }
    }
}

/// The greatest common divisor of two magnitudes. Below 2^64, by Stein's binary algorithm, which
/// needs no division: a mesh's planes are this crate's hot path, and their coefficients are mostly
/// small. Above that, by `geom_math`'s.
fn gcd(a: u128, b: u128) -> u128 {
    let (Ok(mut x), Ok(mut y)) = (u64::try_from(a), u64::try_from(b)) else {
        return geom_math::gcd_u128(a, b);
    };
    if x == 0 || y == 0 {
        return u128::from(x | y);
    }
    let twos = (x | y).trailing_zeros();
    x >>= x.trailing_zeros();
    while y != 0 {
        y >>= y.trailing_zeros();
        (x, y) = (x.min(y), x.abs_diff(y));
    }
    u128::from(x << twos)
}

/// A count as a 32-bit index.
pub(crate) fn index(n: usize) -> Result<u32, MeshError> {
    u32::try_from(n).map_err(|_| MeshError::Full)
}

/// A slice's item at a 32-bit index; the arrays not fitting together if it is not there.
pub(crate) fn at<T>(items: &[T], i: u32) -> Result<&T, MeshFault> {
    items.get(i as usize).ok_or(MeshFault::Shape)
}

/// Every third group of `width` bits set, from bit 0: the masks that spread a coordinate.
const fn every_third(width: u32) -> u128 {
    let group = 1_u128.wrapping_shl(width).wrapping_sub(1);
    let mut mask = 0;
    let mut shift = 0;
    while shift < u128::BITS {
        mask |= group.wrapping_shl(shift);
        shift = shift.saturating_add(width.saturating_mul(3));
    }
    mask
}

/// The 41 bits of `v`, each moved to three times its place.
const fn spread(v: u64) -> u128 {
    let mut x = v as u128;
    x = (x | (x << 64)) & every_third(32);
    x = (x | (x << 32)) & every_third(16);
    x = (x | (x << 16)) & every_third(8);
    x = (x | (x << 8)) & every_third(4);
    x = (x | (x << 4)) & every_third(2);
    (x | (x << 2)) & every_third(1)
}

/// The Morton code of a grid point: its coordinates, shifted to be positive, their 41 bits
/// interleaved into one 123-bit number (19.2, "Canonical order"), so nearby points sort near each
/// other. Two grid points have one code exactly when they are one point.
#[must_use]
pub fn morton(p: P3) -> u128 {
    let shift = |c: i64| c.wrapping_add(1 << 40).cast_unsigned();
    spread(shift(p.x.get())) | (spread(shift(p.y.get())) << 1) | (spread(shift(p.z.get())) << 2)
}

/// A plane's sort key: its coefficients in order.
fn plane_key(plane: &Plane) -> ([i128; 3], i128) {
    (plane.n(), plane.d())
}
