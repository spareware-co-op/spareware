//! A picture clipped to a crop: what a view's crop region keeps of what is drawn (job 7.7;
//! CON-A8-F1-P3).
//!
//! What it does: [`clip`] takes a [`Picture`] (a drawing made ready to print at one scale) and the
//! corners of a crop outline in the drawing's own coordinates, and returns the picture with every
//! stroke and every hatch line cut to the outline's inside. A line that crosses the outline is
//! cut where it crosses, at the exact point, which is a fraction of the grid where the outline is
//! not along an axis; a piece that lies along the outline stays (the closed region counts); a line
//! wholly outside goes. A line cut in several places by an outline that is not convex gives each
//! piece as its own stroke, in order along the line, each keeping the pen, colour, opacity and
//! source of the line it came from. Nothing is drawn along the cut: the crop's own outline is the
//! caller's to draw when the view shows it.
//!
//! Arcs: an arc is kept whole when its circle is clear of the outline and the arc is inside it,
//! dropped when the circle is clear and the arc is outside, and refused ([`DrawingError::Crop`])
//! when the circle meets the outline: where it crosses, the cut point has a square root in it and
//! is not a fraction, so there is no exact answer and none is rounded.
//!
//! Promises: exact (whole grid steps and exact fractions of them; no float, no tolerance); the
//! same picture and outline give the same bytes; every refusal is a [`DrawingError`] value.
//!
//! Depends on this crate's picture and its private overlay.
//!
//! Interim: the segment clip is the crate's own exact overlay (`src/overlay.rs`), which the
//! kernel's 2D booleans (`geom-2d`, pieces G196, G199, G200 and G202) replace with the same
//! answers.

use crate::DrawingError;
use crate::drawing::{Shape, ViewPoint};
use crate::overlay::Window;
use crate::picture::{Hatch, Picture, Stroke};

/// `picture` kept to the inside of the crop outline through `outline` (its corners in order, the
/// last joined to the first, either way round).
///
/// # Errors
///
/// [`DrawingError::BadShape`] for an outline of fewer than three corners, a corner twice in a row
/// or no area; [`DrawingError::Crop`] for an arc whose circle meets the outline;
/// [`DrawingError::TooBig`] for a number past 128 bits.
pub fn clip(picture: &Picture, outline: &[ViewPoint]) -> Result<Picture, DrawingError> {
    let window = Window::new(outline)?;
    let mut hatches = Vec::with_capacity(picture.hatches.len());
    for hatch in &picture.hatches {
        let mut lines = Vec::new();
        for [a, b] in &hatch.lines {
            lines.extend(window.pieces(*a, *b)?);
        }
        if !lines.is_empty() {
            hatches.push(Hatch {
                lines,
                ..hatch.clone()
            });
        }
    }
    let mut strokes = Vec::with_capacity(picture.strokes.len());
    for stroke in &picture.strokes {
        match stroke.shape {
            Shape::Segment { start, end } => {
                for [a, b] in window.pieces(start, end)? {
                    strokes.push(Stroke {
                        shape: Shape::Segment { start: a, end: b },
                        ..stroke.clone()
                    });
                }
            }
            Shape::Arc { centre, start, .. } => match window.arc(centre, start)? {
                Some(true) => strokes.push(stroke.clone()),
                Some(false) => {}
                None => {
                    return Err(DrawingError::Crop(
                        "an arc whose circle meets the crop outline cannot be cut exactly"
                            .to_owned(),
                    ));
                }
            },
        }
    }
    Ok(Picture {
        scale: picture.scale,
        hatches,
        strokes,
    })
}
