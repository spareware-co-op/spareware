//! Heal (19.7): the five steps in order, and step 5, the resolve, at the end.
//!
//! What it does: [`heal()`] welds the raw mesh, stitches its cracks and orients it, steps 2 to 4
//! (step 1, [`super::quantise()`], made the raw mesh). If any boundary is left open, or any edge
//! cannot be paired, it fails and says which. Otherwise the faces are built into an
//! [`ExactMesh`], checked, and handed to the mesh boolean, which keeps where the winding number is
//! above zero: self-intersections and overlaps are resolved, and the result is a solid with
//! maximal faces. The result takes the class of what moved, and each of its faces that bound.
//!
//! Guarantees:
//!
//! - **A healed mesh is closed and a solid** ([`ExactMesh::is_solid`]), passed by the mesh checker
//!   on the way into the boolean and on the way out.
//! - **Its class is what moved** (see [`crate::repair`]): Exact when nothing did, else Certified
//!   by the weld's largest move, at most ε, which every face carries as its deviation: the faces
//!   the boolean merges mix moved and unmoved parts.
//! - **ε is never over the cap**, [`HEAL_CAP`]: a larger one is refused before anything runs.
//! - **One run.** The input's faces are one run of `source`, so the result's history names it.
//!
//! Method: 19.7's steps 2 to 5 (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`);
//! 19.5's boolean with one operand and the test "winding above zero".
//!
//! Cost: the steps', then the build's O(n log n) and the boolean's arrangement and merge.

use alloc::vec;
use alloc::vec::Vec;

use geom_num::Exactness;
use geom_num::grid::{GridStep, Steps};
use geom_num::ladder::{HEAL_CAP, HEAL_DEFAULT};
use geom_op::{History, OpPath};
use geom_poly::winding::WindingTest;
use geom_run::{Exec, WorkMeter};

use super::holes::{self, HoleLog};
use super::loops::{Face, point};
use super::orient::{self, OrientLog};
use super::weld::{WeldLog, weld};
use super::{HealError, RawMesh};
use crate::boolean::boolean;
use crate::exact::build::{Corner, Polygons, Source, build};
use crate::exact::{ExactMesh, MeshError, PropTable, index};

/// How to heal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Heal {
    /// The farthest a vertex may move, and so the widest hole the weld closes: `ε_heal`, at most
    /// [`HEAL_CAP`].
    pub epsilon: Steps,
    /// The frame's grid step, which ε and the result's bound count.
    pub step: GridStep,
}

impl Default for Heal {
    /// [`HEAL_DEFAULT`], 640 steps, on `Q0`'s grid: 0.1 mm.
    fn default() -> Self {
        Heal {
            epsilon: HEAL_DEFAULT,
            step: GridStep::Q0,
        }
    }
}

/// What each step did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HealLog {
    /// Step 2.
    pub weld: WeldLog,
    /// Step 3.
    pub holes: HoleLog,
    /// Step 4.
    pub orient: OrientLog,
}

/// A healed mesh, its history and what each step did.
#[derive(Clone, Debug, PartialEq)]
pub struct Healed {
    /// The mesh: closed, a solid, its class what healing moved.
    pub mesh: ExactMesh,
    /// What became of the input's run (20.3).
    pub history: History,
    /// What each step did.
    pub log: HealLog,
}

/// `raw` healed as `settings` say, its faces one run of `source`; `op` names this step in the
/// names of new runs.
///
/// # Errors
///
/// [`HealError::AboveCap`] for an ε over the cap; [`HealError::NotClosed`] for boundaries left
/// open that a larger ε might close, [`HealError::ToleranceExceeded`] when any is wider than the
/// cap; [`HealError::Unpaired`] for edges that cannot be paired; and [`HealError::Mesh`] for the
/// mesh checker's faults, a stopped meter or a refused allocation.
pub fn heal(
    raw: &RawMesh,
    settings: Heal,
    source: Source,
    op: &OpPath,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Healed, HealError> {
    if settings.epsilon.get() > HEAL_CAP.get() {
        return Err(HealError::AboveCap {
            asked: settings.epsilon,
        });
    }
    let welded = weld(raw, settings.epsilon, exec, meter)?;
    let points = welded.mesh.vertices();
    let mut faces = Vec::with_capacity(welded.source.len());
    for (f, (&input, &moved)) in welded.source.iter().zip(&welded.moved).enumerate() {
        let start = *welded.mesh.face_start().get(f).ok_or(MeshError::Defect)?;
        let end = *welded
            .mesh
            .face_start()
            .get(f.saturating_add(1))
            .ok_or(MeshError::Defect)?;
        let corners = welded
            .mesh
            .corners()
            .get(start as usize..end as usize)
            .ok_or(MeshError::Defect)?;
        faces.push(Face {
            corners: corners.to_vec(),
            input,
            moved,
        });
    }
    let closed = holes::close(points, faces, exec, meter)?;
    if !closed.open.is_empty() {
        let over: Vec<_> = closed
            .open
            .iter()
            .copied()
            .filter(|b| b.width.get() > HEAL_CAP.get())
            .collect();
        return Err(if over.is_empty() {
            HealError::NotClosed {
                boundaries: closed.open,
            }
        } else {
            HealError::ToleranceExceeded { boundaries: over }
        });
    }
    let oriented = orient::orient(points, closed.faces, meter)?;
    if !oriented.unpaired.is_empty() {
        return Err(HealError::Unpaired {
            edges: oriented.unpaired,
            non_orientable: oriented.log.non_orientable,
        });
    }
    let bound = welded.log.movement;
    let class = if bound.get() == 0 {
        Exactness::Exact
    } else {
        Exactness::Certified {
            bound,
            step: settings.step,
        }
    };
    let polygons = polygons(points, &oriented.faces, source, (class, bound))?;
    let mesh = build(&polygons, exec, meter)?;
    let outcome = boolean(&[&mesh], &[WindingTest::unite()], op, exec, meter)?
        .into_iter()
        .next()
        .ok_or(MeshError::Defect)?;
    Ok(Healed {
        mesh: classed(outcome.mesh, (class, bound), exec, meter)?,
        history: outcome.history,
        log: HealLog {
            weld: welded.log,
            holes: closed.log,
            orient: oriented.log,
        },
    })
}

/// `mesh` with the class `class` and every face `bound` from its source: the boolean gives its
/// result the class Exact whatever its operands' were, so healing states its own (19.13: "the
/// least exact input"). Checked again, and still a solid.
fn classed(
    mesh: ExactMesh,
    (class, bound): (Exactness, Steps),
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactMesh, HealError> {
    if class == mesh.parts().class {
        return Ok(mesh);
    }
    let solid = mesh.is_solid();
    let mut parts = mesh.into_parts();
    parts.face_dev = vec![bound; parts.face_plane.len()];
    parts.class = class;
    let mesh = ExactMesh::new(parts, exec, meter)?;
    Ok(if solid { mesh.proven_solid() } else { mesh })
}

/// The builder's input for `faces` on `points`: one run, and each face that moved `bound` from
/// its source.
fn polygons(
    points: &[geom_num::grid::P3],
    faces: &[Face],
    source: Source,
    (class, bound): (Exactness, Steps),
) -> Result<Polygons, HealError> {
    let mut vertices = Vec::with_capacity(points.len());
    for v in 0..index(points.len())? {
        vertices.push(Corner::Grid(point(points, v)?));
    }
    let mut corners = Vec::new();
    let mut loop_start = vec![0_u32];
    let mut face_dev = Vec::with_capacity(faces.len());
    for face in faces {
        corners.extend_from_slice(&face.corners);
        loop_start.push(index(corners.len())?);
        face_dev.push(if face.moved { bound } else { Steps::ZERO });
    }
    let count = index(faces.len())?;
    Ok(Polygons {
        vertices,
        corners,
        loop_start,
        face_loops: (0..=count).collect(),
        face_run: vec![0; faces.len()],
        runs: vec![source],
        face_plane: Vec::new(),
        edge_plane: Vec::new(),
        corner_prop: Vec::new(),
        props: PropTable::default(),
        face_dev,
        class,
    })
}
