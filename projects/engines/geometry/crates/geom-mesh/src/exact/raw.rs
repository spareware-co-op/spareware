//! A raw mesh: what an import reads, before healing makes it an exact mesh (19.2, "Raw meshes").
//!
//! What it does: a [`RawMesh`] holds polygons on grid points with no promise about how they join.
//! They may be open, non-manifold or self-intersecting. Only healing, reconstruction, display and
//! clash accept one, because inside and outside are undefined for an open surface.
//!
//! Guarantees:
//!
//! - **On the grid.** Every vertex is a grid point: import rounds into the frame first (19.7).
//! - **Every index in range.** [`RawMesh::new`] tests that, and that every polygon has at least
//!   three corners. Nothing else about its topology is tested.
//!
//! Method: chapter 19.2's "Raw meshes",
//! `projects/engines/geometry/architecture/19-meshes-spatial-clash.md`.
//!
//! Cost: linear in the corners.

use alloc::vec::Vec;

use geom_num::grid::P3;

use super::{MeshFault, index};

/// Polygons on grid points, with only their indices checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawMesh {
    vertices: Vec<P3>,
    corners: Vec<u32>,
    face_start: Vec<u32>,
}

impl RawMesh {
    /// The polygons whose corners are `corners[face_start[f]..face_start[f + 1]]`, as vertices.
    ///
    /// # Errors
    ///
    /// [`MeshFault::Shape`] for offsets that do not rise from 0 to the corners' end or a corner
    /// that is no vertex, and [`MeshFault::ShortLoop`] for a polygon of fewer than three corners.
    pub fn new(
        vertices: Vec<P3>,
        corners: Vec<u32>,
        face_start: Vec<u32>,
    ) -> Result<Self, MeshFault> {
        let last = index(corners.len()).map_err(|_| MeshFault::Shape)?;
        if face_start.first() != Some(&0)
            || face_start.last() != Some(&last)
            || corners.iter().any(|&c| c as usize >= vertices.len())
        {
            return Err(MeshFault::Shape);
        }
        for (pair, looped) in face_start.windows(2).zip(0_u32..) {
            let &[start, end] = pair else {
                return Err(MeshFault::Shape);
            };
            if end < start {
                return Err(MeshFault::Shape);
            }
            if end.saturating_sub(start) < 3 {
                return Err(MeshFault::ShortLoop { looped });
            }
        }
        Ok(Self {
            vertices,
            corners,
            face_start,
        })
    }

    /// The vertices.
    #[must_use]
    pub fn vertices(&self) -> &[P3] {
        &self.vertices
    }

    /// Every polygon's corners, one polygon after another.
    #[must_use]
    pub fn corners(&self) -> &[u32] {
        &self.corners
    }

    /// Where each polygon starts in [`Self::corners`], with the end last.
    #[must_use]
    pub fn face_start(&self) -> &[u32] {
        &self.face_start
    }
}
