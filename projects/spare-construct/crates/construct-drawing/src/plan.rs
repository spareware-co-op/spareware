//! A plan from built elements: what the view's plane cuts.
//!
//! What it does: [`plan`] takes elements built by the kernel builder (each its solid and the plane
//! of each named face, [`construct_regen::kernel::KernelSolid`]), with their categories, materials
//! and placements, and a view along a grid axis ([`View`]: a plan at a cut height, or a section).
//! For each element it cuts the solid with the view's plane, moved into the element's own frame
//! (placement sits outside the recipe, so every element of one type shares its solid), through the
//! engine seam's `section`. Every cut loop becomes [`LineKind::Cut`] lines, one per edge, each
//! named by the face it lies in: the element's named face whose plane is the edge's (its carrier,
//! as the seam names it). The loops together are the element's cut [`Region`], with its material.
//! An element the plane misses or only touches draws nothing.
//!
//! Not yet (note A12's build order): edges seen below the plane (step 5), hidden lines (step 6),
//! joins (step 7). Regions of one material are merged and filled when the drawing is printed
//! ([`crate::merge`], [`crate::hatch`], [`crate::picture`]), not here: the drawing keeps each
//! element's own region.
//!
//! Interim: the kernel's `draw` (geom-draw, pieces G387, G389 and G394) replaces this function
//! under the same tests; the cut is the seam's `section`, whose answer the interim register names.

use construct_categories::CategoryId;
use construct_materials::MaterialId;
use construct_model::element::Placement;
use construct_model::geometry::GeometryError;
use construct_regen::kernel::{Kernel, KernelSolid};
use engine_geometry::value::engine::{Engine, GridPlane};
use kit_core::Id;

use crate::DrawingError;
use crate::drawing::{Drawing, Entity, Line, LineKind, Region, Shape, Source, ViewPoint};
use crate::view::View;

/// One built element to draw.
#[derive(Debug)]
pub struct PlanElement<'a, S> {
    /// The element's permanent ID.
    pub id: Id,
    /// Its category, which its lines are styled by.
    pub category: CategoryId,
    /// Its material, for its cut region.
    pub material: Option<MaterialId>,
    /// Where its shape sits, in whole millimetres.
    pub placement: Placement,
    /// Its solid and the plane of each named face, from [`Kernel::solid`].
    pub shape: &'a KernelSolid<S>,
}

fn geometry(e: GeometryError) -> DrawingError {
    DrawingError::Geometry {
        code: e.code,
        message: e.message,
    }
}

/// `plane` moved by `-by`: the same plane in a frame whose origin is at `by`.
fn moved(plane: &GridPlane, by: [i64; 3]) -> Result<GridPlane, DrawingError> {
    let mut shift: i128 = 0;
    for (n, b) in plane.normal.into_iter().zip(by) {
        shift = n
            .checked_mul(i128::from(b))
            .and_then(|t| shift.checked_add(t))
            .ok_or(DrawingError::TooBig)?;
    }
    Ok(GridPlane {
        normal: plane.normal,
        offset: plane
            .offset
            .checked_sub(shift)
            .ok_or(DrawingError::TooBig)?,
    })
}

/// `ring` turned to start at its least point, by x, then y.
fn least_first(mut ring: Vec<ViewPoint>) -> Result<Vec<ViewPoint>, DrawingError> {
    let mut least = 0;
    for (i, p) in ring.iter().enumerate() {
        let best = ring.get(least).copied().ok_or(DrawingError::TooBig)?;
        if p.order(best)? == std::cmp::Ordering::Less {
            least = i;
        }
    }
    ring.rotate_left(least);
    Ok(ring)
}

/// The drawing of what `view`'s plane cuts of `elements`, built by `kernel`.
///
/// # Errors
///
/// [`DrawingError::Geometry`] for the engine's refusals (a cut it does not answer yet among
/// them), [`DrawingError::UnnamedFace`] for a cut edge on no named face of its element, and
/// [`DrawingError::TooBig`] for a placement past the grid.
pub fn plan<E: Engine + Send + 'static>(
    kernel: &Kernel<E>,
    view: View,
    elements: &[PlanElement<'_, E::Solid>],
) -> Result<Drawing, DrawingError> {
    let units = kernel.units();
    let plane = view.cut_plane()?;
    let mut lines = Vec::new();
    let mut regions = Vec::new();
    for element in elements {
        let Placement { x, y, z } = element.placement;
        let by = [
            units.steps(x).map_err(geometry)?,
            units.steps(y).map_err(geometry)?,
            units.steps(z).map_err(geometry)?,
        ];
        let cut = kernel
            .section(&element.shape.solid, &moved(&plane, by)?)
            .map_err(geometry)?;
        let mut loops = Vec::new();
        for ring in &cut.loops {
            let mut points = Vec::with_capacity(ring.edges.len());
            for edge in &ring.edges {
                let face = element
                    .shape
                    .faces
                    .iter()
                    .find(|f| f.plane == edge.face)
                    .ok_or_else(|| DrawingError::UnnamedFace(format!("{:?}", edge.face)))?;
                let start = view.project(&edge.start, by)?;
                points.push(start);
                lines.push(Line {
                    kind: LineKind::Cut,
                    source: Source {
                        element: element.id.clone(),
                        entity: Entity::Face,
                        name: face.name.name.clone(),
                        role: face.name.role.clone(),
                    },
                    category: element.category,
                    shape: Shape::Segment {
                        start,
                        end: view.project(&edge.end, by)?,
                    },
                });
            }
            loops.push(least_first(points)?);
        }
        if !loops.is_empty() {
            loops.sort();
            regions.push(Region {
                element: element.id.clone(),
                category: element.category,
                material: element.material.clone(),
                loops,
            });
        }
    }
    Drawing::new(view, lines, regions)
}
