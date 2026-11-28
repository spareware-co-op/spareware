//! A section or an elevation drawn from the stored model (job 8.1): what the page shows when a
//! person opens one, as [`crate::plan`] draws a floor plan.
//!
//! What it does: [`draw`] takes a section's or an elevation's line (construct-views' `CutLine`:
//! its two ends, the side it looks to, how far it sees), whether it cuts, its scale, the elements
//! it cuts or sees (each its category, placement and filled recipe, as
//! [`Service::section`](crate::service::Service::section) reads them at the log's head) and the
//! document's levels and grids, and gives one SVG document, in model millimetres with the view's y
//! up the sheet:
//!
//! - each element's cut regions, merged by material and hatched, and its cut lines at its
//!   category's cut weight, where a section's plane crosses it (an elevation cuts nothing);
//! - the outline of what of each element lies between the plane and the view's depth, at its
//!   category's projection weight, less what a cut region or an element wholly nearer hides
//!   (construct-drawing's `section`);
//! - all of it kept to the view's width, between its line's two ends, cut exactly there with
//!   nothing drawn along the cut; then to the crop's outline when the crop is on, its outline drawn
//!   when it is shown, as a plan's crop is (the crop's corners are the view's own frame: the
//!   distance along the line from its first end, and the height);
//! - the levels and grids that cross the view, as lines with their names and a grid's bubble,
//!   over the frame the drawing fills (the crop's box when the crop is on), never clipped.
//!
//! A view's graphics rules reach it as a plan's do ([`crate::plan::ViewLooks`]). One element at a
//! time, as the plan: an element whose solid the builder cannot make, whose cut or slices the
//! engine does not answer, or whose lines the styles cannot pen, is left out and named in
//! [`Drawn::skipped`] with the reason; so is an arc grid, which a section does not show yet. A
//! line not along a grid axis is refused as a whole, in words.
//!
//! Not drawn yet, each a known fault of the drawing rather than of this file: the edges inside an
//! element's outline (job 8.2); hidden lines between elements whose depths overlap (job 9.1); a
//! the openings in the layers of a wall at an angle (a wall along an axis has the openings of the
//! doors and placed families hosted in it cut through every layer it reaches, as the regenerator
//! builds it: `Kernel::layers_hosting`; a plan still draws its walls whole); floors, roofs and
//! placed families, whose drawing solids the kernel builder does not give yet (named in `skipped`
//! by the builder's own words).
//!
//! Depends on construct-drawing, construct-regen's kernel builder and construct-api's view
//! reading. Interim: as the plan's.

use construct_api::cloud::views::{DatumLine, DatumPlace};
use construct_drawing::crop::clip;
use construct_drawing::datum::{Datum, DatumAt, frame, marks};
use construct_drawing::drawing::{Drawing, ViewPoint};
use construct_drawing::look::Looks;
use construct_drawing::pens::widths;
use construct_drawing::picture::picture;
use construct_drawing::plan::PlanElement;
use construct_drawing::section::{Look, Part, SectionView, part, section};
use construct_drawing::svg::Svg;
use construct_materials::MaterialId;
use construct_model::geometry::{Geometry, Hosted};
use construct_regen::hosting;
use construct_regen::kernel::Kernel;
use construct_regen::kernel::LayerSolid;
use construct_styles::Scale;
use construct_views::{CutLine, Side};
use engine_geometry::value::engine::Engine;

use crate::plan::{Defaults, Drawn, PlanCrop, Skipped, ToDraw, crop_strokes};

/// What a section or an elevation is drawn from.
#[derive(Clone, Copy, Debug)]
pub struct SectionAsk<'a> {
    /// Its line, the side it looks to and how far it sees.
    pub line: &'a CutLine,
    /// `true` for a section, which cuts what its plane crosses; `false` for an elevation.
    pub cuts: bool,
    /// The scale it is printed at.
    pub scale: Scale,
    /// The elements it cuts or sees.
    pub elements: &'a [ToDraw],
    /// The document's levels and grids.
    pub datums: &'a [DatumLine],
    /// How each category looks in it (`None`: as the project's object styles say).
    pub looks: Option<&'a dyn Looks>,
    /// Its crop (`None`: not cropped).
    pub crop: Option<PlanCrop<'a>>,
}

/// The drawing crate's view of `line`.
fn view_of(line: &CutLine, cuts: bool) -> Result<SectionView, String> {
    let (from, to) = (line.from(), line.to());
    let look = match line.look() {
        Side::Left => Look::Left,
        Side::Right => Look::Right,
    };
    SectionView::on_line([from.x, from.y], [to.x, to.y], look, line.far(), cuts)
        .map_err(|e| e.to_string())
}

/// What `view` gives of one element: each layer of it is its own solid, with its material, and the
/// element is checked to pen at `scale`.
fn one<E: Engine + Send + 'static>(
    kernel: &Kernel<E>,
    view: &SectionView,
    defaults: &Defaults,
    ask: &SectionAsk<'_>,
    element: &ToDraw,
) -> Result<Part, String> {
    let layers = layers_of(kernel, element)?;
    let solids: Vec<PlanElement<'_, E::Solid>> = layers
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
    let found = part(kernel, view, &solids).map_err(|e| e.to_string())?;
    let mut lines = found.cut_lines().to_vec();
    lines.extend(found.outline().iter().cloned());
    let alone =
        Drawing::new(view.view(), lines, found.regions().to_vec()).map_err(|e| e.to_string())?;
    widths(&alone, defaults.styles(ask.looks), ask.scale).map_err(|e| e.to_string())?;
    Ok(found)
}

/// The solids `element` is drawn from. A wall with doors or placed families hosted in it has their
/// openings cut through its layers, seated as the regenerator seats them (an embedded curtain wall
/// as the plain opening it cuts); anything else is [`Kernel::layers`].
fn layers_of<E: Engine + Send + 'static>(
    kernel: &Kernel<E>,
    element: &ToDraw,
) -> Result<Vec<LayerSolid<E::Solid>>, String> {
    let Some(opened) = &element.opened else {
        return kernel.layers(&element.recipe).map_err(|e| e.message);
    };
    let edition = kernel.edition();
    let mut seated = Vec::with_capacity(opened.held.len());
    for held in &opened.held {
        let at = match hosting::cut_of(
            &edition,
            &opened.host,
            &element.recipe,
            &held.record,
            &held.recipe,
            &held.hosting,
        ) {
            Ok(Some(cut)) => Ok(cut),
            Ok(None) => hosting::seat(
                &edition,
                &opened.host,
                &element.recipe,
                &held.record,
                &held.recipe,
                &held.hosting,
            ),
            Err(e) => Err(e),
        }
        .map_err(|e| e.message)?;
        seated.push((held.record.id.as_str(), at));
    }
    let hosted: Vec<Hosted<'_>> = seated
        .iter()
        .map(|(key, (at, recipe))| Hosted {
            key,
            recipe,
            at: *at,
        })
        .collect();
    kernel
        .layers_hosting(&element.recipe, &hosted)
        .map_err(|e| e.message)
}

fn datum_of(line: &DatumLine) -> Datum {
    Datum {
        id: line.id.clone(),
        category: line.category,
        name: line.name.clone(),
        at: match &line.at {
            DatumPlace::Level(height) => DatumAt::Level { height: *height },
            DatumPlace::Grid { from, to } => DatumAt::Grid {
                from: [from.x, from.y],
                to: [to.x, to.y],
            },
            DatumPlace::ArcGrid => DatumAt::ArcGrid,
        },
    }
}

const TOO_BIG: &str = "a number of the drawing is past 128 bits";

/// The point with `a`'s x and `b`'s y, over one denominator.
fn corner(a: ViewPoint, b: ViewPoint) -> Result<ViewPoint, String> {
    let den = a.den.checked_mul(b.den).ok_or(TOO_BIG)?;
    let x = a.x.checked_mul(b.den).ok_or(TOO_BIG)?;
    let y = b.y.checked_mul(a.den).ok_or(TOO_BIG)?;
    ViewPoint::new(x, y, den).map_err(|e| e.to_string())
}

/// Whether `a` is less than `b` along x (`up` false) or y (`up` true), exactly.
fn less(a: ViewPoint, b: ViewPoint, up: bool) -> Result<bool, String> {
    let (first, second) = if up { (a.y, b.y) } else { (a.x, b.x) };
    let left = first.checked_mul(b.den).ok_or(TOO_BIG)?;
    let right = second.checked_mul(a.den).ok_or(TOO_BIG)?;
    Ok(left < right)
}

/// The four corners of the box from `lo` to `hi`, counter-clockwise.
fn box_outline([lo, hi]: [ViewPoint; 2]) -> Result<Vec<ViewPoint>, String> {
    Ok(vec![lo, corner(hi, lo)?, hi, corner(lo, hi)?])
}

/// Whether the box from `lo` to `hi` has an area: some width and some height.
fn has_area([lo, hi]: [ViewPoint; 2]) -> Result<bool, String> {
    Ok(less(lo, hi, false)? && less(lo, hi, true)?)
}

/// The box round `outline`: its least and greatest corners.
fn bounds(outline: &[ViewPoint]) -> Result<Option<[ViewPoint; 2]>, String> {
    let mut points = outline.iter().copied();
    let Some(first) = points.next() else {
        return Ok(None);
    };
    let (mut left, mut right, mut low, mut high) = (first, first, first, first);
    for p in points {
        if less(p, left, false)? {
            left = p;
        }
        if less(right, p, false)? {
            right = p;
        }
        if less(p, low, true)? {
            low = p;
        }
        if less(high, p, true)? {
            high = p;
        }
    }
    Ok(Some([corner(left, low)?, corner(right, high)?]))
}

/// The section or elevation `ask` describes, built by `kernel`.
///
/// # Errors
///
/// In words: a line not along a grid axis, and anything the drawing as a whole cannot be made or
/// printed for.
pub(crate) fn draw<E: Engine + Send + 'static>(
    kernel: &Kernel<E>,
    ask: &SectionAsk<'_>,
) -> Result<Drawn, String> {
    let defaults = Defaults::new();
    let styles = defaults.styles(ask.looks);
    let view = view_of(ask.line, ask.cuts)?;
    let (mut parts, mut skipped) = (Vec::new(), Vec::new());
    for element in ask.elements {
        match one(kernel, &view, &defaults, ask, element) {
            Ok(found) => parts.push(found),
            Err(why) => skipped.push(Skipped {
                element: element.id.to_string(),
                why,
            }),
        }
    }
    let drawing = section(&view, &parts).map_err(|e| e.to_string())?;
    let datums: Vec<Datum> = ask.datums.iter().map(datum_of).collect();
    let mut pic = picture(&drawing, styles, ask.scale).map_err(|e| e.to_string())?;
    // The view's width: between its line's two ends, at every height the drawing reaches.
    let filled = frame(&view, &drawing, &datums).map_err(|e| e.to_string())?;
    if let Some(filled) = filled
        && has_area(filled)?
    {
        pic = clip(&pic, &box_outline(filled)?).map_err(|e| e.to_string())?;
    }
    let mut over = filled;
    if let Some(PlanCrop { view: id, crop }) = ask.crop {
        let outline = crop
            .region()
            .corners()
            .iter()
            .map(|p| view.point(p.x, p.y).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        if crop.is_on() {
            pic = clip(&pic, &outline).map_err(|e| e.to_string())?;
            over = bounds(&outline)?;
        }
        if crop.is_shown() {
            pic.strokes
                .extend(crop_strokes(&defaults, ask.scale, id, &outline)?);
        }
    }
    let mut labels = Vec::new();
    if let Some(over) = over {
        let marked = marks(&view, &datums, over, styles, ask.scale).map_err(|e| e.to_string())?;
        pic.strokes.extend(marked.strokes);
        labels = marked.labels;
        skipped.extend(marked.skipped.into_iter().map(|(id, why)| Skipped {
            element: id.to_string(),
            why,
        }));
    }
    let svg = Svg.labelled(&pic, &labels).map_err(|e| e.to_string())?;
    Ok(Drawn { svg, skipped })
}
