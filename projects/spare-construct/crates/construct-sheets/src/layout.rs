//! Where each viewport of a sheet lies on the paper: its centre with its alignments worked out,
//! and its outline worked from the view's extent and scale.
//!
//! The extent of a view (how wide and how high what it shows is, in model lengths) is not stored
//! on a viewport: it changes whenever the model does, and an alignment must hold when it does. So
//! the caller gives each view's [`ViewFacts`] and [`Sheets::layout`] works the answer every time.
//!
//! Exactness. A viewport's centre is an exact [`Length`]. The outline's half width is the view's
//! extent over twice the scale's `n`, rounded up to a whole grid step (156 nm), so the outline
//! always holds the view's drawing and is never smaller than it. An aligned viewport's centre is
//! the followed viewport's anchor, less the anchor's offset in this one, each in whole steps. A
//! view with no extent (nothing of it to draw yet) is a point on the paper: it takes part in
//! alignments with no width and has no outline.

use crate::error::SheetError;
use crate::sheets::Sheets;
use crate::viewport::{Alignment, Anchor, Viewport};
use construct_params::Length;
use construct_styles::Scale;
use kit_core::Id;
use std::collections::BTreeMap;

/// What the layout needs to know of a view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewFacts {
    /// The view's scale, 1:n.
    pub scale: Scale,
    /// How wide and how high what the view shows is, in model lengths, or `None` for a view with
    /// no extent yet.
    pub extent: Option<(Length, Length)>,
}

/// A rectangle on the paper, lower left corner and upper right corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outline {
    /// Left edge.
    pub min_x: Length,
    /// Bottom edge.
    pub min_y: Length,
    /// Right edge.
    pub max_x: Length,
    /// Top edge.
    pub max_y: Length,
}

/// A viewport as it lies on the paper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    /// The viewport.
    pub viewport: Id,
    /// The view it shows.
    pub view: Id,
    /// The centre across the paper, alignments worked out.
    pub x: Length,
    /// The centre up the paper, alignments worked out.
    pub y: Length,
    /// The view's scale.
    pub scale: Scale,
    /// Whether its title shows.
    pub title: bool,
    /// The outline, or `None` when the view has no extent.
    pub outline: Option<Outline>,
    /// Whether the whole outline is on the paper (a viewport with no outline is on it when its
    /// centre is).
    pub on_paper: bool,
}

fn steps_of(length: Length) -> i128 {
    i128::from(length.steps())
}

fn length_of(steps: i128, what: &str) -> Result<Length, SheetError> {
    i64::try_from(steps)
        .map(Length::from_steps)
        .map_err(|_| SheetError::OutOfRange(format!("{what} does not fit")))
}

/// Half of `extent` on paper at 1:`n`, rounded up to a whole step.
fn half(extent: Length, scale: Scale) -> Result<i128, SheetError> {
    if extent.steps() < 0 {
        return Err(SheetError::BadExtent(format!(
            "a view cannot be {extent} wide"
        )));
    }
    let two_n = i128::from(scale.denominator())
        .checked_mul(2)
        .ok_or_else(|| SheetError::OutOfRange("a scale does not fit".to_owned()))?;
    let whole = steps_of(extent);
    let quotient = whole.checked_div(two_n).unwrap_or(0);
    let rest = whole.checked_rem(two_n).unwrap_or(0);
    // Rounded up: the outline is never smaller than the drawing it holds.
    quotient
        .checked_add(i128::from(rest > 0))
        .ok_or_else(|| SheetError::OutOfRange("an extent does not fit".to_owned()))
}

/// How far the anchor is from the centre: minus the half, nothing, or the half.
fn offset(anchor: Anchor, half: i128) -> i128 {
    match anchor {
        Anchor::Min => half.checked_neg().unwrap_or(0),
        Anchor::Centre => 0,
        Anchor::Max => half,
    }
}

impl Sheets {
    /// The viewports of `sheet` as they lie on its paper, by viewport ID, with every alignment
    /// worked out from the views' `facts`.
    ///
    /// # Errors
    /// [`SheetError::UnknownSheet`]; [`SheetError::UnknownView`] when `facts` knows no view of a
    /// viewport; [`SheetError::BadAlignment`] for alignments in a loop (written by nothing the
    /// commands accept); [`SheetError::BadExtent`] or [`SheetError::OutOfRange`].
    pub fn layout(
        &self,
        sheet: &Id,
        facts: impl Fn(&Id) -> Option<ViewFacts>,
    ) -> Result<Vec<Placed>, SheetError> {
        let found = self
            .sheet(sheet)
            .ok_or_else(|| SheetError::UnknownSheet(sheet.clone()))?;
        let viewports = self.viewports_of(sheet);
        let mut halves: BTreeMap<&Id, (i128, i128, ViewFacts)> = BTreeMap::new();
        for viewport in &viewports {
            let known = facts(viewport.view())
                .ok_or_else(|| SheetError::UnknownView(viewport.view().clone()))?;
            let (hw, hh) = match known.extent {
                Some((width, height)) => (half(width, known.scale)?, half(height, known.scale)?),
                None => (0, 0),
            };
            halves.insert(viewport.id(), (hw, hh, known));
        }
        let by_id: BTreeMap<&Id, &Viewport> = viewports.iter().map(|v| (v.id(), *v)).collect();
        let mut placed = Vec::new();
        for viewport in &viewports {
            let x = centre(viewport, &by_id, &halves, Axis::X, 0)?;
            let y = centre(viewport, &by_id, &halves, Axis::Y, 0)?;
            let (hw, hh, known) = halves
                .get(viewport.id())
                .copied()
                .ok_or_else(|| SheetError::UnknownViewport(viewport.id().clone()))?;
            let outline = match known.extent {
                None => None,
                Some(_) => Some(Outline {
                    min_x: length_of(sub(x, hw)?, "an outline")?,
                    min_y: length_of(sub(y, hh)?, "an outline")?,
                    max_x: length_of(add(x, hw)?, "an outline")?,
                    max_y: length_of(add(y, hh)?, "an outline")?,
                }),
            };
            let (paper_w, paper_h) = (
                steps_of(found.paper().width()),
                steps_of(found.paper().height()),
            );
            let inside = |low: i128, high: i128, size: i128| low >= 0 && high <= size;
            let on_paper = inside(sub(x, hw)?, add(x, hw)?, paper_w)
                && inside(sub(y, hh)?, add(y, hh)?, paper_h);
            placed.push(Placed {
                viewport: viewport.id().clone(),
                view: viewport.view().clone(),
                x: length_of(x, "a centre")?,
                y: length_of(y, "a centre")?,
                scale: known.scale,
                title: viewport.title(),
                outline,
                on_paper,
            });
        }
        Ok(placed)
    }
}

fn add(a: i128, b: i128) -> Result<i128, SheetError> {
    a.checked_add(b)
        .ok_or_else(|| SheetError::OutOfRange("a position does not fit".to_owned()))
}

fn sub(a: i128, b: i128) -> Result<i128, SheetError> {
    a.checked_sub(b)
        .ok_or_else(|| SheetError::OutOfRange("a position does not fit".to_owned()))
}

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

type Halves<'a> = BTreeMap<&'a Id, (i128, i128, ViewFacts)>;

/// The centre of a viewport on an axis, in steps: its own when it follows nothing there, else the
/// followed viewport's anchor less this one's offset.
fn centre(
    viewport: &Viewport,
    by_id: &BTreeMap<&Id, &Viewport>,
    halves: &Halves<'_>,
    axis: Axis,
    depth: usize,
) -> Result<i128, SheetError> {
    let (own, align): (Length, Option<&Alignment>) = match axis {
        Axis::X => (viewport.x(), viewport.align_x()),
        Axis::Y => (viewport.y(), viewport.align_y()),
    };
    let Some(align) = align else {
        return Ok(steps_of(own));
    };
    if depth > by_id.len() {
        return Err(SheetError::BadAlignment(
            "the alignments run in a loop".to_owned(),
        ));
    }
    let target = by_id
        .get(&align.to)
        .copied()
        .ok_or_else(|| SheetError::UnknownViewport(align.to.clone()))?;
    let half_of = |one: &Viewport| -> Result<i128, SheetError> {
        let (hw, hh, _) = halves
            .get(one.id())
            .copied()
            .ok_or_else(|| SheetError::UnknownViewport(one.id().clone()))?;
        Ok(match axis {
            Axis::X => hw,
            Axis::Y => hh,
        })
    };
    let followed = centre(target, by_id, halves, axis, depth.saturating_add(1))?;
    let anchor_at = add(followed, offset(align.anchor, half_of(target)?))?;
    sub(anchor_at, offset(align.anchor, half_of(viewport)?))
}
