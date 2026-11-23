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
use std::collections::BTreeSet;

use construct_materials::MaterialId;
use construct_styles::{Colour, Micrometres, PatternId, Scale};
use kit_core::Id;

use crate::DrawingError;
use crate::drawing::{Drawing, Line, LineKind, Shape, ViewPoint};
use crate::hatch::hatch;
use crate::merge::merge;
use crate::overlay::Overlay;
use crate::pens::{Styles, style};

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
        element: line.source.element.clone(),
        role: line.source.role.clone(),
    })
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
    let merged = merge(drawing)?;
    let mut hatches = Vec::new();
    for region in &merged {
        let Some(id) = &region.material else {
            continue;
        };
        let material = styles
            .materials
            .get(id)
            .ok_or_else(|| DrawingError::NoMaterial(id.as_str().to_owned()))?;
        let Some(fill) = &material.drawing.cut.fill else {
            continue;
        };
        let pattern = styles
            .fills
            .get(fill.as_str())
            .ok_or_else(|| DrawingError::NoFillPattern(fill.as_str().to_owned()))?;
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
            colour: material.drawing.cut.colour,
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
