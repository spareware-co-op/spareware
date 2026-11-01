//! Minkowski sums of flat solids (19.6): every a + b for a in one solid and b in the other.
//!
//! What it does: [`minkowski`] gives the sum of two closed meshes as one closed mesh of one run.
//! It is the volume a tool translating over a shape sweeps.
//!
//! Guarantees:
//!
//! - **Exact on grid vertices.** Every sum of grid points is a grid point, every hull is exact, and
//!   the union is exact. A vertex off the grid is committed to it first, which makes the result
//!   [`Sum::class`] `Certified` by that half step, on each solid.
//! - **The size is the product of the solids', and stays inside its budget.** One hull for each
//!   pair of triangles, and one n-ary union of them all, charged to the meter. Two convex solids
//!   take one hull of all their sums.
//! - **Refused past the frame.** A sum outside the frame is [`NumError::OutOfRange`].
//!
//! Method: 19.6's table. Both solids split into convex pieces, each pair's sum is the exact convex
//! hull of the sums of their corners, and the sums are united in one boolean. The pieces here are
//! the triangles of the boundary, whose corners are all on the grid, not solid cells: the boundary
//! of a sum lies in the sums of the boundaries, and one copy of each solid, moved by a point of
//! the other, fills what is inside. A pair of parallel triangles has a flat sum, which its
//! neighbours' sums cover, so it is left out.
//!
//! Cost: O(t² log t) for t triangles a side for the hulls, then the union of up to t² hulls.

#![allow(
    clippy::many_single_char_names,
    reason = "a solid's points, and a triangle's corners, as chapter 19 names them"
)]

use alloc::vec::Vec;

use geom_num::grid::{GridStep, P3, Steps};
use geom_num::predicate::orient3d;
use geom_num::{Exactness, NumError, Sign};
use geom_op::NameId;
use geom_poly::winding::WindingTest;
use geom_run::{Exec, WorkMeter};

use crate::boolean::boolean_into;
use crate::exact::build::{Polygons, Source, build};
use crate::exact::{ExactMesh, MeshError, MeshFault, MeshKind, Role, at};
use crate::export::{ExportError, Triangles, triangles};
use crate::hull::{Hull, HullError};

/// A sum.
#[derive(Clone, Debug, PartialEq)]
pub struct Sum {
    /// The mesh.
    pub mesh: ExactMesh,
    /// `Exact` when both solids were on the grid.
    pub class: Exactness,
}

/// An export's errors as the mesh's: only committing to the grid can reach here.
fn from_export(e: &ExportError) -> MeshError {
    match *e {
        ExportError::Num(e) => MeshError::Num(e),
        ExportError::Stopped(s) => s.into(),
        ExportError::Fault(f) => MeshError::Fault(f),
        _ => MeshError::Defect,
    }
}

/// A hull's errors as the mesh's.
fn from_hull(e: HullError) -> MeshError {
    match e {
        HullError::Num(e) => MeshError::Num(e),
        HullError::Stopped(s) => s.into(),
        HullError::Full => MeshError::Full,
        HullError::Defect => MeshError::Defect,
    }
}

/// `a` + `b`, or an error if it is outside the frame.
fn plus(a: P3, b: P3) -> Result<P3, NumError> {
    let sum = |p: i64, q: i64| p.saturating_add(q);
    P3::new(
        sum(a.x.get(), b.x.get()),
        sum(a.y.get(), b.y.get()),
        sum(a.z.get(), b.z.get()),
    )
}

/// The corners of a triangle.
fn corners_of(t: &Triangles, corner: [u32; 3]) -> Result<[P3; 3], MeshFault> {
    let [p, q, r] = corner;
    Ok([
        *at(&t.vertices, p)?,
        *at(&t.vertices, q)?,
        *at(&t.vertices, r)?,
    ])
}

/// Whether the mesh is its own convex hull: no corner of the hull is above any of its triangles.
fn convex(t: &Triangles, hull: &Hull) -> bool {
    hull.is_solid()
        && t.corners.iter().all(|c| {
            corners_of(t, *c).is_ok_and(|[p, q, r]| {
                hull.points()
                    .iter()
                    .all(|s| orient3d(p, q, r, *s).is_ok_and(|side| side != Sign::Positive))
            })
        })
}

/// A run of its own for the `n`th operand of the union, whose names are the union's own affair.
fn numbered(n: u64) -> Source {
    let mut bytes = [0_u8; 16];
    let (low, _) = bytes.split_at_mut(8);
    low.copy_from_slice(&n.to_le_bytes());
    Source {
        name: NameId::from_bytes(bytes),
        role: Role::Face,
        flags: 0,
    }
}

/// `input`, moved by `by`.
fn moved(input: &Triangles, by: P3, source: Source) -> Result<Polygons, MeshError> {
    let mut vertices = Vec::with_capacity(input.vertices.len());
    for p in &input.vertices {
        vertices.push(crate::exact::build::Corner::Grid(plus(*p, by)?));
    }
    let mut corners = Vec::with_capacity(input.corners.len().saturating_mul(3));
    for &[a, b, c] in &input.corners {
        corners.push(a);
        corners.push(b);
        corners.push(c);
    }
    let count = crate::exact::index(input.corners.len())?;
    Ok(Polygons {
        vertices,
        corners,
        loop_start: (0..=count).map(|f| f.saturating_mul(3)).collect(),
        face_loops: (0..=count).collect(),
        face_run: alloc::vec![0; input.corners.len()],
        runs: alloc::vec![source],
        face_plane: Vec::new(),
        edge_plane: Vec::new(),
        corner_prop: Vec::new(),
        props: crate::exact::PropTable::default(),
        face_dev: Vec::new(),
        class: Exactness::Exact,
    })
}

/// The sum of `a` and `b` as a mesh of one run, `source`. `step` is the grid's, for the class of a
/// sum of solids off it.
///
/// # Errors
///
/// [`MeshError::NotClosed`] for a mesh with boundary edges, [`MeshError::Num`] for a sum outside
/// the frame, and a stopped meter.
pub fn minkowski(
    a: &ExactMesh,
    b: &ExactMesh,
    (source, step): (Source, GridStep),
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Sum, MeshError> {
    if a.kind() == MeshKind::Open || b.kind() == MeshKind::Open {
        return Err(MeshError::NotClosed);
    }
    let (ta, tb) = (
        triangles(a, meter).map_err(|e| from_export(&e))?,
        triangles(b, meter).map_err(|e| from_export(&e))?,
    );
    let rounded = u64::from(ta.rounded).saturating_add(u64::from(tb.rounded));
    let class = if rounded == 0 {
        Exactness::Exact
    } else {
        Exactness::Certified {
            bound: Steps(rounded),
            step,
        }
    };
    let (hull_a, hull_b) = (
        Hull::of(&ta.vertices, meter).map_err(from_hull)?,
        Hull::of(&tb.vertices, meter).map_err(from_hull)?,
    );
    if convex(&ta, &hull_a) && convex(&tb, &hull_b) {
        let mut sums = Vec::new();
        for p in &ta.vertices {
            for q in &tb.vertices {
                sums.push(plus(*p, *q)?);
            }
        }
        let hull = Hull::of(&sums, meter).map_err(from_hull)?;
        return Ok(Sum {
            mesh: hull.mesh(source, exec, meter)?,
            class,
        });
    }
    let mut operands: Vec<ExactMesh> = Vec::new();
    let mut next = 0_u64;
    for x in &ta.corners {
        let [p, q, r] = corners_of(&ta, *x)?;
        for y in &tb.corners {
            let [s, t, u] = corners_of(&tb, *y)?;
            let mut sums = Vec::with_capacity(9);
            for from in [p, q, r] {
                for to in [s, t, u] {
                    sums.push(plus(from, to)?);
                }
            }
            let hull = Hull::of(&sums, meter).map_err(from_hull)?;
            if hull.is_solid() {
                operands.push(hull.mesh(numbered(next), exec, meter)?);
                next = next.saturating_add(1);
            }
        }
    }
    let lowest = |t: &Triangles| t.vertices.first().copied();
    let (Some(from_a), Some(from_b)) = (lowest(&ta), lowest(&tb)) else {
        return Err(MeshError::NotClosed);
    };
    for (solid, by) in [(&ta, from_b), (&tb, from_a)] {
        let polygons = moved(solid, by, numbered(next))?;
        operands.push(build(&polygons, exec, meter)?);
        next = next.saturating_add(1);
    }
    let refs: Vec<&ExactMesh> = operands.iter().collect();
    let test = WindingTest::unite();
    let mesh = boolean_into(&refs, &test, source, exec, meter)?;
    Ok(Sum { mesh, class })
}
