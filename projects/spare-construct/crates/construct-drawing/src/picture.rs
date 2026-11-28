//! A drawing made ready to print at one scale, for any output: merged cut regions with their
//! hatches and outlines, every line with its printed width and colour.
//!
//! What it does: [`picture`] merges the drawing's cut regions ([`crate::merge`]), fills each
//! merged region that has a material with its material's cut pattern ([`crate::hatch`], printed
//! with the pattern's pen at the scale, in the material's cut colour), and outlines it with its
//! cut lines at the category's cut weight for the scale (construct-styles' object styles and pens,
//! [`crate::pens::style`]). Where outlines of two merged regions lie on one another, as between
//! regions of different materials, the line is drawn once: the heavier pen wins, then the line
//! first in the drawing's order. Lines that bound no region, such as edges seen in projection, are
//! drawn as they are.
//!
//! An [`Output`] writes a picture: [`crate::svg::Svg`] today; a PDF or another writer is one more
//! type, and the picture does not change.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use construct_categories::CategoryId;
use construct_materials::MaterialId;
use construct_styles::{Colour, Micrometres, PatternId, Scale};
use kit_core::Id;

use crate::DrawingError;
use crate::drawing::{Drawing, Line, LineKind, Shape, ViewPoint};
use crate::hatch::hatch;
use crate::look::{Look, opacity};
use crate::merge::merge;
use crate::overlay::Overlay;
use crate::pens::{Styles, look_of, style};

/// One printed line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stroke {
    /// Its shape, in view coordinates.
    pub shape: Shape,
    /// Cut or seen in projection.
    pub kind: LineKind,
    /// Its printed width.
    pub width: Micrometres,
    /// Its colour.
    pub colour: Colour,
    /// Its line pattern.
    pub pattern: PatternId,
    /// How much of full strength it prints at, in whole percent (100 unless its view says halftone
    /// or transparency).
    pub opacity: u8,
    /// The element it came from.
    pub element: Id,
    /// What the element's operation calls the face or edge it came from.
    pub role: String,
}

/// The hatch of one merged region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hatch {
    /// The region's material.
    pub material: MaterialId,
    /// The printed width of its lines.
    pub width: Micrometres,
    /// Their colour: the material's cut colour.
    pub colour: Colour,
    /// How much of full strength it prints at, in whole percent.
    pub opacity: u8,
    /// The lines, each from its first point to its second, in view coordinates.
    pub lines: Vec<[ViewPoint; 2]>,
}

/// A drawing ready to print at one scale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    /// The scale it prints at.
    pub scale: Scale,
    /// The hatches, drawn first, in the order of the merged regions.
    pub hatches: Vec<Hatch>,
    /// The lines, drawn over the hatches: the merged outlines, then the other lines in the
    /// drawing's order.
    pub strokes: Vec<Stroke>,
}

/// A writer of pictures: SVG today, and any other output a later job adds.
pub trait Output {
    /// The bytes of `picture` in this output's format: the same picture always gives the same
    /// bytes.
    ///
    /// # Errors
    ///
    /// The writer's refusals, such as [`DrawingError::TooBig`] for a number it cannot write.
    fn write(&self, picture: &Picture) -> Result<Vec<u8>, DrawingError>;
}

fn stroke(
    line: &Line,
    shape: Shape,
    styles: Styles<'_>,
    scale: Scale,
) -> Result<Stroke, DrawingError> {
    let pen = style(styles, line, scale)?;
    Ok(Stroke {
        shape,
        kind: line.kind,
        width: pen.width,
        colour: pen.colour,
        pattern: pen.pattern,
        opacity: look_of(styles, line)?.as_ref().map_or(100, opacity),
        element: line.source.element.clone(),
        role: line.source.role.clone(),
    })
}

/// What the view draws of `drawing`: with view rules, the lines and regions whose category its
/// rules show; with none, `drawing` as it is.
fn shown(drawing: &Drawing, styles: Styles<'_>) -> Result<Option<Drawing>, DrawingError> {
    let Some(looks) = styles.looks else {
        return Ok(None);
    };
    let mut lines = Vec::new();
    for line in drawing.lines() {
        if looks
            .look_for(&line.source.element, line.category, line.kind)
            .map_err(DrawingError::Style)?
            .shown
        {
            lines.push(line.clone());
        }
    }
    let mut regions = Vec::new();
    for region in drawing.regions() {
        if looks
            .look_for(&region.element, region.category, LineKind::Cut)
            .map_err(DrawingError::Style)?
            .shown
        {
            regions.push(region.clone());
        }
    }
    Drawing::new(drawing.view(), lines, regions).map(Some)
}

/// An element's looks, region by region: a category and the look the view gives its cut.
type Looked = Vec<(CategoryId, Look)>;

/// The look the view gives the cut of a merged region, from its first element (all of a merged
/// region's elements look alike); `None` when the view has no looks.
fn cut_look(
    styles: Styles<'_>,
    region: &crate::merge::MergedRegion,
) -> Result<Option<Look>, DrawingError> {
    styles
        .looks
        .map(|looks| match region.elements.first() {
            Some(element) => looks.look_for(element, region.category, LineKind::Cut),
            None => looks.look(region.category, LineKind::Cut),
        })
        .transpose()
        .map_err(DrawingError::Style)
}

/// The drawing's cut regions merged. When the view's looks differ from one element to the next
/// ([`crate::look::Looks::per_element`]), only elements that look alike are merged, so a
/// halftone element beside a solid one of the same material stays two regions; otherwise every
/// element of a category and material merges, as it always did.
fn merged_regions(
    drawing: &Drawing,
    styles: Styles<'_>,
) -> Result<Vec<crate::merge::MergedRegion>, DrawingError> {
    let Some(looks) = styles.looks.filter(|looks| looks.per_element()) else {
        return merge(drawing);
    };
    // Each element's looks, region by region: elements with the same ones go together.
    let mut by_element: BTreeMap<Id, Looked> = BTreeMap::new();
    for region in drawing.regions() {
        let look = looks
            .look_for(&region.element, region.category, LineKind::Cut)
            .map_err(DrawingError::Style)?;
        by_element
            .entry(region.element.clone())
            .or_default()
            .push((region.category, look));
    }
    let mut groups: Vec<(Looked, BTreeSet<Id>)> = Vec::new();
    for (element, key) in by_element {
        match groups.iter_mut().find(|(seen, _)| *seen == key) {
            Some((_, elements)) => {
                elements.insert(element);
            }
            None => groups.push((key, BTreeSet::from([element]))),
        }
    }
    let mut merged = Vec::new();
    for (_, elements) in groups {
        let lines = drawing
            .lines()
            .iter()
            .filter(|line| elements.contains(&line.source.element))
            .cloned()
            .collect();
        let regions = drawing
            .regions()
            .iter()
            .filter(|region| elements.contains(&region.element))
            .cloned()
            .collect();
        merged.extend(merge(&Drawing::new(drawing.view(), lines, regions)?)?);
    }
    Ok(merged)
}

/// `drawing` ready to print at `scale` with `styles`.
///
/// # Errors
///
/// [`DrawingError::NoMaterial`] for a region whose material `styles` does not hold,
/// [`DrawingError::NoFillPattern`] for a material whose cut fill names no pattern there, what
/// [`crate::pens::style`] refuses, and the errors of [`merge`] and [`hatch`].
pub fn picture(
    drawing: &Drawing,
    styles: Styles<'_>,
    scale: Scale,
) -> Result<Picture, DrawingError> {
    let left = shown(drawing, styles)?;
    let drawing = left.as_ref().unwrap_or(drawing);
    let merged = merged_regions(drawing, styles)?;
    let mut hatches = Vec::new();
    for region in &merged {
        let Some(id) = &region.material else {
            continue;
        };
        let material = styles
            .materials
            .get(id)
            .ok_or_else(|| DrawingError::NoMaterial(id.as_str().to_owned()))?;
        // The view's fill for the category, where it has one, in place of the material's.
        let cut = cut_look(styles, region)?;
        let over = cut.as_ref().and_then(|look| look.fill.as_ref());
        let Some(fill) = over.map(|f| f.pattern.as_str()).or_else(|| {
            material
                .drawing
                .cut
                .fill
                .as_ref()
                .map(construct_materials::RefName::as_str)
        }) else {
            continue;
        };
        let pattern = styles
            .fills
            .get(fill)
            .ok_or_else(|| DrawingError::NoFillPattern(fill.to_owned()))?;
        let lines = hatch(&region.loops, pattern, scale)?;
        if lines.is_empty() {
            continue;
        }
        hatches.push(Hatch {
            material: id.clone(),
            width: styles
                .lines
                .model_pens
                .width(pattern.pen, scale)
                .map_err(|why| DrawingError::Style(format!("{why:?}")))?,
            colour: over.map_or(material.drawing.cut.colour, |f| f.colour),
            opacity: cut.as_ref().map_or(100, opacity),
            lines,
        });
    }
    // The outlines, heaviest pen first, then in the drawing's order: where two lie on one another
    // the first is drawn.
    let mut outlines = Vec::new();
    for region in &merged {
        for line in &region.outline {
            let pen = style(styles, line, scale)?;
            outlines.push((Reverse(pen.width), line));
        }
    }
    outlines.sort();
    let mut pieces = Vec::with_capacity(outlines.len());
    for (i, (_, line)) in outlines.iter().enumerate() {
        if let Shape::Segment { start, end } = line.shape {
            pieces.push((start, end, i));
        }
    }
    let mut strokes = Vec::new();
    for (start, end, i) in Overlay::once(&pieces)? {
        let (_, line) = outlines
            .get(i)
            .ok_or_else(|| DrawingError::Overlay("a label of no line".to_owned()))?;
        strokes.push(stroke(line, Shape::Segment { start, end }, styles, scale)?);
    }
    // Every other line: those that bound no region.
    let mut bounding = BTreeSet::new();
    for region in drawing.regions() {
        for ring in &region.loops {
            for (i, &p) in ring.iter().enumerate() {
                let q = ring
                    .get(i.saturating_add(1))
                    .or_else(|| ring.first())
                    .copied()
                    .unwrap_or(p);
                bounding.insert((&region.element, p, q));
            }
        }
    }
    for line in drawing.lines() {
        let bounds = match (line.kind, line.shape) {
            (LineKind::Cut, Shape::Segment { start, end }) => {
                bounding.contains(&(&line.source.element, start, end))
            }
            _ => false,
        };
        if !bounds {
            strokes.push(stroke(line, line.shape, styles, scale)?);
        }
    }
    Ok(Picture {
        scale,
        hatches,
        strokes,
    })
}
