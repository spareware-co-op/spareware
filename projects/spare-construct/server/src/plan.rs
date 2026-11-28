//! A floor plan drawn from the stored model: what the page shows when a person opens a view.
//!
//! What it does: [`Drawer::draw`] takes the elements a plan cuts (each its category, placement and
//! filled recipe, as [`Service::plan`](crate::service::Service::plan) reads them from the model at
//! the log's head) and the height of the cut plane, and gives one SVG document, in model
//! millimetres with the view's y up the sheet, as construct-drawing prints a plan: the cut edges
//! of each layer of each wall at the pen of its category for the view's scale, and each region of
//! one material merged and hatched. It never writes anything: the drawing is a pure function of
//! the stored model, the view and the project's styles, so every browser asks and gets the same
//! bytes, and the page draws no geometry of its own.
//!
//! A view's graphics rules reach the drawing through [`ViewLooks`]: for each line and fill the
//! drawing asks it how its category looks on its edge, and it answers with construct-views'
//! `resolve` (through construct-api's `look_in`) and nothing of its own: weight, pattern, colour,
//! a cut's fill, halftone, transparency, and a hidden category left out. A view with no rules
//! resolves to the project's object styles, so its drawing is the bytes it was before rules.
//!
//! The engine behind it is whatever [`Kernel`] was made over: the geometry kernel's seam, whose
//! `section` only a server built with the `kernel` feature answers today (solidcore, behind the
//! seam), so a server without it has no drawer and says so (`crate::geometry::drawer`).
//!
//! One element at a time: an element whose solid the builder cannot make, or whose cut the engine
//! does not answer, or whose lines the styles cannot pen, is left out and named in
//! [`Drawn::skipped`] with the reason, so one element never blanks the plan.
//!
//! Not drawn yet, each a known fault of the plan rather than of this file: edges seen below the cut
//! (job 7.2's next step), a door's opening in its wall (the wall is cut whole until doors are
//! families, job 6.4), floors and datums (they are not cut by a plan's plane), and the styles are
//! the default metric set, not a project's own, until construct-styles holds them as project data.
//!
//! Depends on construct-drawing and construct-regen's kernel builder. Interim: the drawing
//! (construct-drawing's note) and the cut (the interim register) are both stand-ins for the
//! kernel's own, behind the same types.

use construct_api::cloud::views::ElementStyle;
use construct_api::cloud::views::{World, look_for as resolve_for, look_in};
use construct_categories::{CategoryId, CategoryTable};
use construct_drawing::drawing::Drawing;
use construct_drawing::drawing::LineKind;
use construct_drawing::hatch::FillPatterns;
use construct_drawing::look::{FillLook, Look, Looks};
use construct_drawing::pens::{Styles, widths};
use construct_drawing::plan::{PlanElement, plan};
use construct_drawing::svg::to_svg;
use construct_drawing::view::View;
use construct_materials::{MaterialId, Materials};
use construct_model::element::Placement;
use construct_model::geometry::Recipe;
use construct_params::Length;
use construct_regen::kernel::Kernel;
use construct_styles::{Edge, EdgeStyle, LineStyles, ObjectStyles, Scale};
use engine_geometry::value::engine::Engine;
use kit_core::Id;
use serde::Serialize;
use std::collections::BTreeMap;

/// One element a plan cuts, as the model holds it.
#[derive(Clone, Debug)]
pub struct ToDraw {
    /// Its permanent ID.
    pub id: Id,
    /// Its category, which its lines are penned by.
    pub category: CategoryId,
    /// Where its shape sits, in whole millimetres, base height included.
    pub placement: Placement,
    /// Its recipe as the regenerator fills it.
    pub recipe: Recipe,
}

/// An element left out of the drawing, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Skipped {
    /// The element's ID.
    pub element: String,
    /// The reason, in words.
    pub why: String,
}

/// A drawn plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drawn {
    /// The SVG document.
    pub svg: String,
    /// What was left out, in the order of the elements given.
    pub skipped: Vec<Skipped>,
}

/// Draws plans. One per server, chosen with its builder ([`crate::geometry::drawer`]).
pub trait Drawer: Send + Sync {
    /// The plan cut at the height `cut` above the project's zero, of `elements`, printed at
    /// `scale`, each category looking as `looks` says (`None`: as the project's object styles
    /// say).
    ///
    /// # Errors
    ///
    /// In words, when the drawing as a whole cannot be made or printed; an element that cannot be
    /// drawn is in [`Drawn::skipped`] instead.
    fn draw(
        &self,
        cut: Length,
        scale: Scale,
        elements: &[ToDraw],
        looks: Option<&dyn Looks>,
    ) -> Result<Drawn, String>;
}

/// How a view's rules make each category look: every answer is `resolve`'s, through the document
/// as it is at the log's head.
pub struct ViewLooks<'a> {
    world: &'a World,
    view: &'a Id,
    own: &'a BTreeMap<Id, ElementStyle>,
}

impl<'a> ViewLooks<'a> {
    /// The looks of `view` in `world`, each element in `own` looking as its filters and its
    /// overrides in the view say over what its category does.
    #[must_use]
    pub const fn new(world: &'a World, view: &'a Id, own: &'a BTreeMap<Id, ElementStyle>) -> Self {
        Self { world, view, own }
    }

    fn answer(
        &self,
        own: Option<&ElementStyle>,
        category: CategoryId,
        kind: LineKind,
    ) -> Result<Look, String> {
        let edge = match kind {
            LineKind::Cut => Edge::Cut,
            LineKind::Projected => Edge::Projected,
        };
        let found = if own.is_some() {
            resolve_for(self.world, self.view, category, edge, own)?
        } else {
            look_in(self.world, self.view, category, edge)?
        };
        let rule = found.style;
        Ok(Look {
            shown: rule.shown,
            edge: EdgeStyle {
                width: found.width,
                colour: rule.colour,
                pattern: rule.pattern,
            },
            fill: rule.fill.map(|fill| FillLook {
                pattern: fill.pattern,
                colour: fill.colour,
            }),
            halftone: rule.halftone,
            transparency: rule.transparency,
        })
    }
}

impl std::fmt::Debug for ViewLooks<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewLooks")
            .field("view", &self.view)
            .finish_non_exhaustive()
    }
}

impl Looks for ViewLooks<'_> {
    fn look(&self, category: CategoryId, kind: LineKind) -> Result<Look, String> {
        self.answer(None, category, kind)
    }

    fn look_for(&self, element: &Id, category: CategoryId, kind: LineKind) -> Result<Look, String> {
        self.answer(self.own.get(element), category, kind)
    }

    fn per_element(&self) -> bool {
        !self.own.is_empty()
    }
}

/// The default metric styles a plan is penned with, until a project keeps its own.
struct Defaults {
    categories: CategoryTable,
    lines: LineStyles,
    objects: ObjectStyles,
    materials: Materials,
    fills: FillPatterns,
}

impl Defaults {
    fn new() -> Self {
        Self {
            categories: CategoryTable::new(),
            lines: LineStyles::default_metric(),
            objects: ObjectStyles::default_metric(),
            materials: Materials::default_metric(),
            fills: FillPatterns::default_metric(),
        }
    }

    fn styles<'a>(&'a self, looks: Option<&'a dyn Looks>) -> Styles<'a> {
        Styles {
            looks,
            categories: &self.categories,
            lines: &self.lines,
            objects: &self.objects,
            materials: &self.materials,
            fills: &self.fills,
        }
    }
}

/// The drawing of what `view`'s plane cuts of one element: each layer of it is its own solid, with
/// its material, and the element is checked to pen at `scale`.
fn one<E: Engine + Send + 'static>(
    kernel: &Kernel<E>,
    view: View,
    defaults: &Defaults,
    scale: Scale,
    looks: Option<&dyn Looks>,
    element: &ToDraw,
) -> Result<Drawing, String> {
    let layers = kernel.layers(&element.recipe).map_err(|e| e.message)?;
    let parts: Vec<PlanElement<'_, E::Solid>> = layers
        .iter()
        .map(|layer| PlanElement {
            id: element.id.clone(),
            category: element.category,
            // A material the project's library does not hold draws no fill, not an error.
            material: layer
                .material
                .as_deref()
                .and_then(|m| MaterialId::parse(m).ok())
                .filter(|m| defaults.materials.get(m).is_some()),
            placement: element.placement,
            shape: &layer.solid,
        })
        .collect();
    let drawing = plan(kernel, view, &parts).map_err(|e| e.to_string())?;
    widths(&drawing, defaults.styles(looks), scale).map_err(|e| e.to_string())?;
    Ok(drawing)
}

impl<E: Engine + Send + 'static> Drawer for Kernel<E> {
    fn draw(
        &self,
        cut: Length,
        scale: Scale,
        elements: &[ToDraw],
        looks: Option<&dyn Looks>,
    ) -> Result<Drawn, String> {
        let defaults = Defaults::new();
        let view = View::plan(cut);
        let (mut lines, mut regions, mut skipped) = (Vec::new(), Vec::new(), Vec::new());
        for element in elements {
            match one(self, view, &defaults, scale, looks, element) {
                Ok(drawing) => {
                    lines.extend(drawing.lines().iter().cloned());
                    regions.extend(drawing.regions().iter().cloned());
                }
                Err(why) => skipped.push(Skipped {
                    element: element.id.to_string(),
                    why,
                }),
            }
        }
        let drawing = Drawing::new(view, lines, regions).map_err(|e| e.to_string())?;
        let svg = to_svg(&drawing, defaults.styles(looks), scale).map_err(|e| e.to_string())?;
        Ok(Drawn { svg, skipped })
    }
}
