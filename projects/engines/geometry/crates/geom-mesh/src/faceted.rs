//! Exact meshes as faceted B-reps (19.5): an edge of the conversion graph.
//!
//! What it does: [`faceted`] makes a closed mesh a body: adjacent faces on one plane merge into one
//! face, and what is left of their edges merges into maximal edges.
//!
//! Guarantees:
//!
//! - **Planes are matched exactly.** Faces are on one plane when their reduced coefficients are
//!   equal, found by sorting those keys, never by comparing pairs of faces, which is what stalls on
//!   large slabs (19.5).
//! - **The body is the mesh.** Its vertices are the mesh's points, grid points or three planes, and
//!   nothing moves.
//! - **Closed meshes only.** An open mesh bounds nothing: [`MeshError::NotClosed`].
//!
//! Method: 19.5's "Mesh to faceted B-rep"
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`): each face's convex pieces
//! go through `geom-poly`'s merge and publication, the same two steps a boolean's result takes. A
//! surface's `f64` frame is drawn from the plane's integer coefficients; the body's corners and
//! edges stand on the planes, so the frame only shows where the face is for evaluation.
//!
//! Cost: the merge's O(n log n) exact comparisons for n sides of pieces, then the publication.

use alloc::vec::Vec;

use geom_num::plane::{Plane, Tpi};
use geom_poly::merge::{Merge, Outward, merge};
use geom_poly::publish::{Carrier, publish};
use geom_poly::winding::Winding;
use geom_run::WorkMeter;
use geom_surface::Plane as Flat;
use geom_topo::{Body, Sense, SurfaceKind};

use crate::classify::pieces_of;
use crate::exact::{ExactMesh, Facing, MeshError, MeshFault, MeshKind, Parts, Vertex, at, index};
use crate::hull::plan_axes;

/// `facing`'s normal, the way it faces.
fn outward(plane: &Plane, facing: Facing) -> [i128; 3] {
    if facing.flipped {
        plane.n().map(i128::wrapping_neg)
    } else {
        plane.n()
    }
}

/// A value of a plane's coefficients as a double, for a frame that needs no more.
fn double(v: i128) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "a surface's frame shows where a face is; its corners and edges decide it exactly"
    )]
    let x = v as f64;
    x
}

/// A vertex of the face, at the grid point nearest it, as doubles.
fn anchor(parts: &Parts, face: u32) -> Result<[f64; 3], MeshError> {
    let looped = *at(&parts.face_loops, face)?;
    let corner = *at(&parts.loop_start, looped)?;
    let v = *at(&parts.he_vert, corner)?;
    let near = match *at(&parts.verts, v)? {
        Vertex::Grid(p) => p,
        Vertex::Planes([a, b, c]) => {
            let row = |r: u32| at(&parts.planes, r).copied();
            Tpi::new(row(a)?, row(b)?, row(c)?)?.to_grid()?.value()
        }
    };
    let part = |c: i64| double(i128::from(c));
    Ok([part(near.x.get()), part(near.y.get()), part(near.z.get())])
}

/// The surface of face `face`: its plane, by two directions in it from the plane's coefficients.
fn carrier(parts: &Parts, face: u32) -> Result<Carrier, MeshError> {
    let facing = *at(&parts.face_plane, face)?;
    let n = outward(at(&parts.planes, facing.row)?, facing);
    let ([i, j], up) = plan_axes(n);
    let k = i.saturating_add(2).checked_rem(3).unwrap_or(0);
    let part = |axis: usize| n.get(axis).copied().map_or(0.0, double);
    // In the axes i, j, k, a right-handed turn: (n_k, 0, −n_i) and (0, n_k, −n_j) lie in the plane,
    // and their cross product is n_k · n.
    let mut first = [0.0; 3];
    let mut second = [0.0; 3];
    let (on_i, on_j, on_k) = (part(i), part(j), part(k));
    let put = |v: &mut [f64; 3], axis: usize, value: f64| {
        if let Some(slot) = v.get_mut(axis) {
            *slot = value;
        }
    };
    put(&mut first, i, on_k);
    put(&mut first, k, -on_i);
    put(&mut second, j, on_k);
    put(&mut second, k, -on_j);
    let wide = (f64::NEG_INFINITY, f64::INFINITY);
    let plane = Flat::new(anchor(parts, face)?, first, second, (wide, wide))
        .map_err(|_| MeshError::Defect)?;
    Ok(Carrier {
        surface: SurfaceKind::Plane(plane),
        sense: if up { Sense::Forward } else { Sense::Reversed },
    })
}

/// The body `mesh` bounds, with adjacent faces on one plane merged.
///
/// # Errors
///
/// [`MeshError::NotClosed`] for a mesh with boundary edges, and a stopped meter.
pub fn faceted(mesh: &ExactMesh, meter: &mut WorkMeter<'_>) -> Result<Body, MeshError> {
    if mesh.kind() == MeshKind::Open {
        return Err(MeshError::NotClosed);
    }
    let parts = mesh.parts();
    let mut pieces = Vec::new();
    let mut carriers = Vec::new();
    for face in 0..index(mesh.faces())? {
        let surface = carrier(parts, face)?;
        let source = usize::try_from(face).map_err(|_| MeshFault::Shape)?;
        for piece in pieces_of(parts, face, meter)? {
            pieces.push(Outward {
                piece,
                polygon: carriers.len(),
                turned: false,
                source,
                front: Winding::new(),
                back: Winding::new(),
            });
            carriers.push(surface.clone());
        }
    }
    let merged = merge(&pieces, Merge::RemoveRedundant, meter)?;
    publish(&merged, &carriers, meter).map_err(|_| MeshFault::Shape.into())
}
