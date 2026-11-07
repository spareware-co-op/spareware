//! Fuzz target: `geom-mesh`'s reconstruction by winding number never panics or hangs, refuses what
//! has no inside, and gives one answer whatever order its faces come in.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: up to [`MAX_FACES`] triangles
//! between the corners of a small lattice, so that repeats, flat faces, open edges, shared planes
//! and edges of many faces are all common. [`check`] reconstructs them under a work budget. What
//! it returns must be a closed solid mesh whose class is Approximate, whose undecided cells
//! match its log, and which is the same when the faces come in the other order and each starts from
//! another corner; what it refuses must be one of its named refusals.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input and a bounded run.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes,
//!   and the run is held to [`WORK`] units of the meter, so a hang is a refusal, not a stall.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case.
//!
//! Method: the laws of chapter 19.7's reconstruction,
//! `projects/engines/geometry/architecture/19-meshes-spatial-clash.md`: the result is a closed
//! solid, its class says it is Approximate, the cells it left out are counted once, and nothing in
//! it depends on the order the faces came in.
//!
//! Cost: at most [`MAX_FACES`] triangles and two reconstructions.

use geom_mesh::exact::build::Source;
use geom_mesh::exact::raw::RawMesh;
use geom_mesh::exact::{MeshError, MeshKind, Role};
use geom_mesh::repair::{Reconstruct, ReconstructError, reconstruct};
use geom_num::Exactness;
use geom_num::grid::P3;
use geom_op::NameId;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most triangles a case holds.
pub const MAX_FACES: u8 = 12;

/// The lattice's points on each axis.
pub const SIDE: u8 = 3;

/// What the lattice is scaled by.
pub const SCALE: i64 = 40;

/// The most bytes [`decode`] reads from any input: 38, two for the unused scale and the count, and
/// three for each of 12 triangles.
pub const MAX_INPUT_BYTES: usize = 38;

/// The work a run may charge before it is stopped.
pub const WORK: u64 = 1 << 24;

/// One decoded input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// Each triangle's three lattice points, numbered x + 3y + 9z.
    pub faces: Vec<[u8; 3]>,
}

/// The case `data` holds, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    // The first byte is reserved, so that the corpora of the exact mesh target are useful here.
    let _ = src.byte();
    let count = src
        .byte()
        .checked_rem(MAX_FACES.saturating_add(1))
        .unwrap_or(0);
    let points = SIDE.saturating_mul(SIDE).saturating_mul(SIDE);
    let mut faces = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let mut corner = || src.byte().checked_rem(points).unwrap_or(0);
        faces.push([corner(), corner(), corner()]);
    }
    let read = data.len().saturating_sub(src.remaining());
    (Case { faces }, read)
}

/// The lattice point `n`, scaled.
fn point(n: u8) -> Result<P3, Finding> {
    let side = i64::from(SIDE);
    let n = i64::from(n);
    let at = |c: i64| c.saturating_mul(SCALE);
    let plane = side.saturating_mul(side);
    let part = |q: Option<i64>| at(q.unwrap_or(0));
    P3::new(
        part(n.checked_rem(side)),
        part(n.checked_div(side).and_then(|q| q.checked_rem(side))),
        part(n.checked_div(plane)),
    )
    .map_err(|e| Finding::new("a decoded point is in the frame", || format!("{e:?}")))
}

/// The raw mesh of `faces`, the lattice's points as its vertices in order.
fn raw(faces: &[[u8; 3]]) -> Result<RawMesh, Finding> {
    let mut vertices = Vec::new();
    for n in 0..SIDE.saturating_mul(SIDE).saturating_mul(SIDE) {
        vertices.push(point(n)?);
    }
    let count = u32::try_from(faces.len()).unwrap_or(u32::MAX);
    RawMesh::new(
        vertices,
        faces.iter().flatten().map(|&v| u32::from(v)).collect(),
        (0..=count).map(|f| f.saturating_mul(3)).collect(),
    )
    .map_err(|e| Finding::new("a decoded mesh is a raw mesh", || format!("{e:?}")))
}

/// Reconstructs `faces` within [`WORK`].
fn run(
    faces: &[[u8; 3]],
) -> Result<Result<geom_mesh::repair::Reconstructed, ReconstructError>, Finding> {
    let mesh = raw(faces)?;
    let mut meter = WorkMeter::new(Budget::of_work(WORK));
    Ok(reconstruct(
        &mesh,
        Reconstruct::default(),
        Source {
            name: NameId::from_bytes([2; 16]),
            role: Role::FaceSet,
            flags: 0,
        },
        &Exec::serial(),
        &mut meter,
    ))
}

/// The faces in the other order, each starting from another corner.
fn turned(case: &Case) -> Vec<[u8; 3]> {
    case.faces
        .iter()
        .rev()
        .map(|&[a, b, c]| [b, c, a])
        .collect()
}

/// Reconstructs the case and, if it gives a mesh, holds it to every law.
///
/// # Errors
///
/// A [`Finding`] for the first law broken, or for a refusal that is not one reconstruction names.
pub fn check(case: &Case) -> Result<(), Finding> {
    let broken = |law: &'static str, detail: String| {
        Finding::new(law, move || format!("{case:?}: {detail}"))
    };
    match run(&case.faces)? {
        Ok(done) => {
            if !done.mesh.is_solid() || done.mesh.kind() == MeshKind::Open {
                return Err(broken("the result is a closed solid", String::new()));
            }
            if !matches!(done.mesh.parts().class, Exactness::Approximate { .. }) {
                return Err(broken("the result is Approximate", String::new()));
            }
            if u32::try_from(done.undecided.len()) != Ok(done.log.cells.undecided) {
                return Err(broken(
                    "every undecided cell is listed once",
                    format!("{:?}", done.log.cells),
                ));
            }
            match run(&turned(case))? {
                Ok(second) if second.mesh == done.mesh && second.log == done.log => Ok(()),
                other => Err(broken(
                    "one answer in any order",
                    format!("{:?}", other.map(|r| r.log)),
                )),
            }
        }
        // Faces with no area, or no inside, or too little budget: refusals reconstruction names.
        Err(
            ReconstructError::NoSurface
            | ReconstructError::NothingKept { .. }
            | ReconstructError::Heal(_)
            | ReconstructError::Mesh(MeshError::Stopped(_) | MeshError::BudgetExceeded(_)),
        ) => Ok(()),
        Err(e) => Err(broken("a refusal names its cause", format!("{e:?}"))),
    }
}

/// The harness every fuzzing engine drives: [`decode`] then [`check`].
///
/// # Errors
///
/// As for [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}
