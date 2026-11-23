//! Cut regions merged by material: touching regions of one material draw as one, with no line
//! between them (note A12's build-order step 4; chapter 18.4 of the geometry architecture leaves
//! the merge to the program).
//!
//! What it does: [`merge`] takes a drawing's cut regions and joins those that may merge into
//! [`MergedRegion`]s: the union of their loops, exactly, as separate areas. Two regions may merge
//! when they have the same category and the same material; a region with no material merges with
//! nothing. Regions that overlap or share an edge become one; regions that touch only at a point
//! stay apart, and a hole stays a hole unless a region of the same rule fills it. Each edge of a
//! merged outline is a cut [`Line`] that keeps the source of the cut line it is part of (the
//! element and its face, by persistent name): a shortened edge keeps its source, and two pieces
//! in one line with different sources stay two lines.
//!
//! The rule stands in for joins (note A12's step 7, job 9.2): until elements are joined, regions
//! that touch are taken as joined. Merged regions are derived when a drawing is printed, like its
//! hatch lines; the stored drawing keeps each element's own region and lines.
//!
//! Interim: the union is the crate's own exact overlay ([`crate::overlay`]), which the kernel's
//! 2D booleans (`geom-2d`, pieces G196, G199, G200 and G202) replace with the same answers.

use std::collections::{BTreeMap, BTreeSet};

use construct_categories::CategoryId;
use construct_materials::MaterialId;
use kit_core::Id;

use crate::DrawingError;
use crate::drawing::{Drawing, Line, LineKind, Region, Shape, ViewPoint};
use crate::overlay::Overlay;

/// One merged cut region: the regions of one category and material that touch, as one.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MergedRegion {
    /// The category of every region in it.
    pub category: CategoryId,
    /// Their material; `None` for a region with no material, which is never merged.
    pub material: Option<MaterialId>,
    /// The loops: the outline first, counter-clockwise, then its holes, clockwise, each from its
    /// least point (by x, then y, exactly).
    pub loops: Vec<Vec<ViewPoint>>,
    /// The elements merged into it, in order: those whose material it holds, an element wholly
    /// inside the others among them.
    pub elements: Vec<Id>,
    /// Its outline as cut lines, loop by loop, edge by edge from each loop's first point, each
    /// with the source of the cut line it is part of.
    pub outline: Vec<Line>,
}

/// Which regions may merge: the same category and material, or, with no material, the region
/// alone.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    Material(CategoryId, MaterialId),
    Alone(CategoryId, usize),
}

/// The edges of `region`'s loops, each with the index of the cut line it is.
fn labelled(
    region: &Region,
    lines: &BTreeMap<(&Id, ViewPoint, ViewPoint), usize>,
) -> Result<Vec<Vec<(ViewPoint, usize)>>, DrawingError> {
    let mut out = Vec::with_capacity(region.loops.len());
    for ring in &region.loops {
        let mut labelled = Vec::with_capacity(ring.len());
        for (i, &p) in ring.iter().enumerate() {
            let q = ring
                .get(i.saturating_add(1))
                .or_else(|| ring.first())
                .copied()
                .unwrap_or(p);
            let label = lines.get(&(&region.element, p, q)).ok_or_else(|| {
                DrawingError::BadShape(format!(
                    "an edge of {}'s region with no cut line of its own",
                    region.element.as_str()
                ))
            })?;
            labelled.push((p, *label));
        }
        out.push(labelled);
    }
    Ok(out)
}

/// The drawing's cut regions, merged: those of one category and material that touch become one.
/// In order of category, material, then loops.
///
/// # Errors
///
/// [`DrawingError::BadShape`] for a region whose edge is not one of its element's cut lines;
/// [`DrawingError::TooBig`] for a point past 128 bits where edges cross;
/// [`DrawingError::Overlay`] if the overlay breaks its own rule.
pub fn merge(drawing: &Drawing) -> Result<Vec<MergedRegion>, DrawingError> {
    let all = drawing.lines();
    let mut by_edge = BTreeMap::new();
    for (i, line) in all.iter().enumerate() {
        if let (LineKind::Cut, Shape::Segment { start, end }) = (line.kind, line.shape) {
            by_edge
                .entry((&line.source.element, start, end))
                .or_insert(i);
        }
    }
    let mut groups: BTreeMap<Group, Vec<&Region>> = BTreeMap::new();
    for (i, region) in drawing.regions().iter().enumerate() {
        let key = match &region.material {
            Some(m) => Group::Material(region.category, m.clone()),
            None => Group::Alone(region.category, i),
        };
        groups.entry(key).or_default().push(region);
    }
    let mut out = Vec::new();
    for (key, regions) in groups {
        let (category, material) = match key {
            Group::Material(c, m) => (c, Some(m)),
            Group::Alone(c, _) => (c, None),
        };
        let mut loops = Vec::new();
        for region in &regions {
            loops.extend(labelled(region, &by_edge)?);
        }
        for area in Overlay::union(&loops)? {
            // Every region whose material the area holds, a region wholly inside the others
            // among them.
            let mut elements = BTreeSet::new();
            for region in &regions {
                for ring in &region.loops {
                    if let (Some(&a), Some(&b)) = (ring.first(), ring.get(1))
                        && Overlay::holds(&area, a, b)?
                    {
                        elements.insert(region.element.clone());
                    }
                }
            }
            let mut outline = Vec::new();
            let mut points = Vec::with_capacity(area.loops.len());
            for ring in &area.loops {
                for (i, &(start, label)) in ring.iter().enumerate() {
                    let (end, _) = ring
                        .get(i.saturating_add(1))
                        .or_else(|| ring.first())
                        .copied()
                        .unwrap_or((start, label));
                    let line = all
                        .get(label)
                        .ok_or_else(|| DrawingError::Overlay("a label of no line".to_owned()))?;
                    outline.push(Line {
                        kind: LineKind::Cut,
                        source: line.source.clone(),
                        category: line.category,
                        shape: Shape::Segment { start, end },
                    });
                }
                points.push(ring.iter().map(|&(p, _)| p).collect());
            }
            out.push(MergedRegion {
                category,
                material: material.clone(),
                loops: points,
                elements: elements.into_iter().collect(),
                outline,
            });
        }
    }
    out.sort();
    Ok(out)
}
