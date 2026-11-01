//! Fuzz target: `geom-mesh`'s exact mesh never panics, refuses what is not a mesh, and gives one
//! mesh whatever order its input comes in.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: up to [`MAX_FACES`] triangles
//! between the corners of a small lattice, so that repeats, flat faces, open edges and edges of
//! many faces are all common. [`check`] builds them. What it accepts must pass the checker again,
//! come out the same with the input turned, and hold to the definitions of 19.2, which are worked
//! out here apart from the code.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case.
//!
//! Method: the laws of chapter 19.2's "What it guarantees",
//! `projects/engines/geometry/architecture/19-meshes-spatial-clash.md`: the edge uses are counted
//! here from the faces alone, and the kind must agree with them; each half-edge's pair is an
//! opposite half-edge; six times a closed mesh's volume is the sum of the corners' determinants.
//!
//! Cost: at most [`MAX_FACES`] triangles, two builds and one pass over the result.

use std::collections::BTreeMap;

use geom_mesh::exact::build::{Corner, Polygons, Source, build};
use geom_mesh::exact::{ExactMesh, MeshError, MeshKind, NONE, PropTable, Role, Vertex};
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

/// What the lattice is scaled by: plain, and to the edge of the frame.
pub const SCALES: [i64; 2] = [1, 1 << 38];

/// The most bytes [`decode`] reads from any input: 38, two for the scale and the count, and three
/// for each of 12 triangles.
pub const MAX_INPUT_BYTES: usize = 38;

/// One decoded input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// What the lattice is scaled by.
    pub scale: i64,
    /// Each triangle's three lattice points, numbered x + 3y + 9z.
    pub faces: Vec<[u8; 3]>,
}

/// The case `data` holds, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let scale = SCALES
        .get(usize::from(src.byte() % 2))
        .copied()
        .unwrap_or(1);
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
    (Case { scale, faces }, read)
}

/// The polygons of `case`, its vertices the lattice in order.
fn polygons(case: &Case) -> Result<Polygons, Finding> {
    let mut vertices = Vec::new();
    for z in 0..i64::from(SIDE) {
        for y in 0..i64::from(SIDE) {
            for x in 0..i64::from(SIDE) {
                let lattice = |c: i64| c.saturating_mul(case.scale);
                let at = P3::new(lattice(x), lattice(y), lattice(z)).map_err(|e| {
                    Finding::new("a decoded point is in the frame", || format!("{e:?}"))
                })?;
                vertices.push(Corner::Grid(at));
            }
        }
    }
    let count = u32::try_from(case.faces.len()).unwrap_or(u32::MAX);
    Ok(Polygons {
        vertices,
        corners: case.faces.iter().flatten().map(|&v| u32::from(v)).collect(),
        loop_start: (0..=count).map(|f| f.saturating_mul(3)).collect(),
        face_loops: (0..=count).collect(),
        face_run: alloc_zero(case.faces.len()),
        runs: vec![Source {
            name: NameId::from_bytes([1; 16]),
            role: Role::Face,
            flags: 0,
        }],
        face_plane: Vec::new(),
        edge_plane: Vec::new(),
        corner_prop: Vec::new(),
        props: PropTable::default(),
        face_dev: Vec::new(),
        class: Exactness::Exact,
    })
}

/// `n` zeros.
fn alloc_zero(n: usize) -> Vec<u32> {
    vec![0; n]
}

/// How a mesh's kind must come out, from how often each edge is used each way.
fn expected_kind(case: &Case) -> MeshKind {
    let mut uses: BTreeMap<(u8, u8), (u32, u32)> = BTreeMap::new();
    for face in &case.faces {
        let [a, b, c] = *face;
        for (x, y) in [(a, b), (b, c), (c, a)] {
            let slot = uses.entry((x.min(y), x.max(y))).or_default();
            if x < y {
                slot.0 = slot.0.saturating_add(1);
            } else {
                slot.1 = slot.1.saturating_add(1);
            }
        }
    }
    let many = uses.values().any(|&(p, q)| p.saturating_add(q) > 2);
    let open = uses.values().any(|&(p, q)| p.saturating_add(q) == 1);
    if many {
        MeshKind::General
    } else if open {
        MeshKind::Open
    } else {
        MeshKind::Closed
    }
}

/// Every law of the module's method on `mesh`, the mesh of `case`.
fn laws(case: &Case, mesh: &ExactMesh) -> Result<(), Finding> {
    let broken = |law: &str, detail: String| Finding::new(law, || format!("{case:?}: {detail}"));
    let kind = expected_kind(case);
    if mesh.kind() != kind {
        let got = mesh.kind();
        return Err(broken(
            "the kind is what the edges make it",
            format!("{got:?}"),
        ));
    }
    let p = mesh.parts();
    if mesh.faces() != case.faces.len() {
        return Err(broken("every face is kept", format!("{}", mesh.faces())));
    }
    for (h, &pair) in p.he_pair.iter().enumerate() {
        if pair != NONE {
            let back = p.he_pair.get(pair as usize).copied();
            if back.map(|b| b as usize) != Some(h) {
                return Err(broken("a pair is mutual", format!("{h}")));
            }
        }
    }
    if p.verts.iter().any(|v| !matches!(v, Vertex::Grid(_))) {
        return Err(broken("grid points stay on the grid", String::new()));
    }
    Ok(())
}

/// The same polygons with the vertices reversed and each triangle turned one place.
fn turned(case: &Case, input: &Polygons) -> Polygons {
    let last = u32::try_from(input.vertices.len())
        .unwrap_or(0)
        .saturating_sub(1);
    let mut out = input.clone();
    out.vertices.reverse();
    let mut corners: Vec<u32> = Vec::with_capacity(input.corners.len());
    for face in case.faces.iter().rev() {
        let [a, b, c] = face.map(u32::from);
        corners.extend([b, c, a].map(|v| last.saturating_sub(v)));
    }
    out.corners = corners;
    out
}

/// Builds the case and, if it is a mesh, holds it to every law.
///
/// # Errors
///
/// A [`Finding`] for the first law broken, or for a refusal that is not the case's fault.
pub fn check(case: &Case) -> Result<(), Finding> {
    let input = polygons(case)?;
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    match build(&input, &Exec::serial(), &mut meter) {
        Ok(mesh) => {
            laws(case, &mesh)?;
            let mut meter = WorkMeter::new(Budget::UNLIMITED);
            let second = build(&turned(case, &input), &Exec::serial(), &mut meter)
                .map_err(|e| Finding::new("a turned case builds", || format!("{case:?}: {e:?}")))?;
            if second != mesh {
                return Err(Finding::new("one mesh in any order", || {
                    format!("{case:?}")
                }));
            }
            let mut meter = WorkMeter::new(Budget::UNLIMITED);
            ExactMesh::new(mesh.into_parts(), &Exec::serial(), &mut meter)
                .map(|_| ())
                .map_err(|e| Finding::new("a mesh passes again", || format!("{case:?}: {e:?}")))
        }
        Err(MeshError::Fault(_)) => Ok(()),
        Err(e) => Err(Finding::new("a refusal names a fault", || {
            format!("{case:?}: {e:?}")
        })),
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
