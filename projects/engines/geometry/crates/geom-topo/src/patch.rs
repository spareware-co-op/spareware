//! A body with some rows' values replaced, every link and name kept (piece G139a).
//!
//! What it does: [`Body::patched`] takes a [`Patch`], new values for faces' boxes and for rows of
//! the surface, curve, pcurve and point pools, and returns the body with those values in place,
//! sharing every chunk the patch does not write with the body it came from (8.2, 23.9). A local
//! re-cut whose topology and names are the last cut's changes only such values: a moved door's
//! planes, lines, corners and the boxes of the faces it passes through.
//!
//! Guarantees:
//!
//! - **Only values change.** A patch holds no link of the topology and no name, so every row keeps
//!   its place and its links. Pool rows are placed by the least place of the entities that lie on
//!   them, which links decide, so the patched body is in the order a publication in name order
//!   gives it (`crate::builder::named`), and its names are the body's own, shared.
//! - **No dangling index.** A patched row past the end of its arena is [`TopoError::NoSuchRow`],
//!   as is a patched point that meets a plane past the end of the plane pool, as publishing
//!   refuses it; the body is unchanged by a patch that fails.
//! - **Writes copy only their chunks.** Each row written copies the chunk it lands in, once, when
//!   the body still shares it; every other chunk stays shared.
//!
//! Method: copy on write over the arenas' `Arc` chunks ([`crate::Column::set`],
//! `Arc::make_mut`, <https://doc.rust-lang.org/alloc/sync/struct.Arc.html#method.make_mut>), as an
//! edit through [`crate::Builder::edit`] does, without the publication's re-placing of rows, which
//! a patch of values cannot change; the design in `projects/kit/notes/G139a-shared-bodies.md`.
//!
//! Cost: linear in the patch's rows, plus one chunk copy for each chunk it writes; nothing is
//! linear in the body.

use alloc::vec::Vec;

use geom_curve::CurveKind;
use geom_math::Box3;
use geom_surface::SurfaceKind;

use crate::body::Body;
use crate::entity::{CurveIx, FaceIx, PcurveIx, PointIx, PointKind, SurfaceIx};
use crate::error::TopoError;
use crate::ix::Arena;

/// New values for rows of a body, by row: faces' boxes, and rows of the surface, curve, pcurve
/// and point pools.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Patch {
    /// Faces' boxes.
    pub bounds: Vec<(FaceIx, Box3)>,
    /// Surfaces.
    pub surfaces: Vec<(SurfaceIx, SurfaceKind)>,
    /// Curves.
    pub curves: Vec<(CurveIx, CurveKind<3>)>,
    /// Curves on surfaces.
    pub pcurves: Vec<(PcurveIx, CurveKind<2>)>,
    /// Points.
    pub points: Vec<(PointIx, PointKind)>,
}

impl Patch {
    /// Whether it changes nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bounds.is_empty()
            && self.surfaces.is_empty()
            && self.curves.is_empty()
            && self.pcurves.is_empty()
            && self.points.is_empty()
    }
}

impl Body {
    /// This body with `patch`'s values in place, sharing every chunk the patch does not write,
    /// with this body's kind, outside region and names.
    ///
    /// # Errors
    ///
    /// [`TopoError::NoSuchRow`] for a patched row past the end of its arena, or for a patched
    /// point meeting a plane past the end of the plane pool; [`TopoError::OutOfMemory`] if a chunk
    /// copy is refused.
    pub fn patched(&self, patch: &Patch) -> Result<Body, TopoError> {
        let mut arenas = self.arenas().clone();
        for (_, point) in &patch.points {
            if let PointKind::Meet(planes) = point
                && let Some(far) = planes.iter().find(|p| arenas.planes.get(**p).is_none())
            {
                return Err(TopoError::NoSuchRow {
                    arena: Arena::Plane,
                    row: far.row(),
                });
            }
        }
        for (face, bound) in &patch.bounds {
            arenas.faces.bound.set(*face, *bound)?;
        }
        for (surface, value) in &patch.surfaces {
            arenas.surfaces.set(*surface, value.clone())?;
        }
        for (curve, value) in &patch.curves {
            arenas.curves.set(*curve, value.clone())?;
        }
        for (pcurve, value) in &patch.pcurves {
            arenas.pcurves.set(*pcurve, value.clone())?;
        }
        for (point, value) in &patch.points {
            arenas.points.set(*point, *value)?;
        }
        Ok(self.with_arenas(arenas))
    }
}
