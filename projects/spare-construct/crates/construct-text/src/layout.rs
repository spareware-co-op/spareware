//! Laying a note out: its lines as they break at the wrap width, the size of each and the box they
//! make. Worked from the note record, its style and font metrics every time it is asked; nothing
//! about the layout is stored, so a style change or a font change is never stale.
//!
//! Exact: every width is the metrics' thousandths of the size times the style's size on paper,
//! rounded up to a whole grid step (a box never holds less than the text); line heights likewise;
//! alignment offsets are whole steps, rounded down. Where a line may break is the Unicode line
//! breaking algorithm's (the `unicode-linebreak` crate); a word wider than the wrap width keeps a
//! line of its own and runs past the box's right edge, never split.

use crate::error::TextError;
use crate::metrics::{Face, FontMetrics};
use crate::note::{Align, ListKind, TextNote};
use crate::style::TextStyle;
use construct_params::Length;
use unicode_linebreak::linebreaks;

/// One laid-out line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaidLine {
    /// The list marker drawn at `x` on the first line of a list item (`•`, `2.`), or empty.
    pub marker: String,
    /// The text of the line, with no trailing space.
    pub text: String,
    /// Where the line's block starts, from the box's left edge: alignment worked in.
    pub x: Length,
    /// How far the text sits right of `x`: the marker's width on every line of a list item, so
    /// continuation lines hang under the first, and zero for a plain paragraph.
    pub indent: Length,
    /// The width of the block: `indent` and the text's width.
    pub width: Length,
    /// Where the line's top is, down from the box's top.
    pub top: Length,
}

/// A note laid out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Laid {
    /// The lines, top to bottom. A blank line of the note is a line with no text.
    pub lines: Vec<LaidLine>,
    /// How far a line takes: the font's line height at the style's size and spacing.
    pub line_height: Length,
    /// The box's width: the wrap width when the note wraps, else its widest line.
    pub width: Length,
    /// The box's height: every line's.
    pub height: Length,
}

impl Laid {
    /// The box in the view's coordinates before rotation, as left, bottom, right and top: its top
    /// left corner is the note's anchor and it runs right and down the page. Rotating it about the
    /// anchor by the note's rotation is the drawing's job.
    ///
    /// # Errors
    /// [`TextError::OutOfRange`] for a box that does not fit in a length.
    pub fn bounds(&self, note: &TextNote) -> Result<[Length; 4], TextError> {
        let out = || TextError::OutOfRange("a text note's box".to_owned());
        let right = note.x().checked_add(self.width).ok_or_else(out)?;
        let bottom = note.y().checked_sub(self.height).ok_or_else(out)?;
        Ok([note.x(), bottom, right, note.y()])
    }
}

/// `units` thousandths of `size`, rounded up to whole steps, times `percent` over 100 likewise.
fn scaled(units: u64, size: i64, percent: u16) -> Result<i64, TextError> {
    let out = || TextError::OutOfRange("a measured size".to_owned());
    let whole = i128::from(units)
        .checked_mul(i128::from(size))
        .and_then(|n| n.checked_mul(i128::from(percent)))
        .ok_or_else(out)?;
    let divisor = 1000_i128.checked_mul(100).ok_or_else(out)?;
    // Rounded up: a measured box never holds less than the text.
    let up = whole
        .checked_add(divisor.checked_sub(1).ok_or_else(out)?)
        .and_then(|n| n.checked_div(divisor))
        .ok_or_else(out)?;
    i64::try_from(up).map_err(|_| out())
}

struct Measure<'a> {
    metrics: &'a dyn FontMetrics,
    face: Face<'a>,
    size: i64,
}

impl Measure<'_> {
    fn width(&self, text: &str) -> Result<i64, TextError> {
        scaled(self.metrics.advance(&self.face, text)?, self.size, 100)
    }
}

/// A paragraph broken into lines no wider than `avail` where a break allows it, each with its
/// width. `None` is no wrapping: one line.
fn wrap(
    content: &str,
    avail: Option<i64>,
    measure: &Measure<'_>,
) -> Result<Vec<(String, i64)>, TextError> {
    let Some(avail) = avail else {
        let line = content.trim_end();
        return Ok(vec![(line.to_owned(), measure.width(line)?)]);
    };
    let mut lines = Vec::new();
    let mut start = 0_usize;
    let mut best: Option<(usize, i64)> = None;
    let part = |from: usize, to: usize| content.get(from..to).unwrap_or_default().trim_end();
    for (end, _) in linebreaks(content) {
        loop {
            let width = measure.width(part(start, end))?;
            if width <= avail || best.is_none() {
                best = Some((end, width));
                break;
            }
            // It does not fit, and an earlier break is there: the line ends at it.
            if let Some((until, until_width)) = best.take() {
                lines.push((part(start, until).to_owned(), until_width));
                start = until;
            }
        }
    }
    if let Some((end, width)) = best {
        lines.push((part(start, end).to_owned(), width));
    }
    Ok(lines)
}

/// Lays a note out in its style with `metrics`: its lines (each paragraph broken at the wrap
/// width, list markers set, each line placed by the alignment) and its box.
///
/// # Errors
/// [`TextError::Metrics`] when the metrics do not know the font; [`TextError::OutOfRange`] for a
/// size that does not fit.
pub fn lay_out(
    note: &TextNote,
    style: &TextStyle,
    metrics: &dyn FontMetrics,
) -> Result<Laid, TextError> {
    let out = || TextError::OutOfRange("a text note's layout".to_owned());
    let face = Face {
        font: style.font(),
        bold: style.bold(),
        italic: style.italic(),
    };
    let size = style.size().steps();
    let measure = Measure {
        metrics,
        face,
        size,
    };
    let line_height = scaled(metrics.line_height(&face)?, size, style.spacing())?;
    let wrap_width = note.width().map(Length::steps);

    // Each line before alignment: marker, text, indent, block width.
    let mut raw: Vec<(String, String, i64, i64)> = Vec::new();
    let mut item = 0_u32;
    for paragraph in note.lines() {
        if paragraph.trim().is_empty() {
            raw.push((String::new(), String::new(), 0, 0));
            continue;
        }
        item = item.checked_add(1).ok_or_else(out)?;
        let marker = match note.list() {
            ListKind::None => String::new(),
            ListKind::Bullet => "\u{2022}".to_owned(),
            ListKind::Number => format!("{item}."),
        };
        let indent = if marker.is_empty() {
            0
        } else {
            // The marker and one space's room: the gap is the width of a space.
            measure
                .width(&marker)?
                .checked_add(measure.width(" ")?)
                .ok_or_else(out)?
        };
        let avail = match wrap_width {
            Some(width) => Some(width.checked_sub(indent).ok_or_else(out)?),
            None => None,
        };
        for (position, (text, width)) in wrap(paragraph, avail, &measure)?.into_iter().enumerate() {
            let marker = if position == 0 {
                marker.clone()
            } else {
                String::new()
            };
            raw.push((
                marker,
                text,
                indent,
                indent.checked_add(width).ok_or_else(out)?,
            ));
        }
    }

    let widest = raw.iter().map(|line| line.3).max().unwrap_or(0);
    let box_width = wrap_width.unwrap_or(widest);
    let mut lines = Vec::with_capacity(raw.len());
    for (position, (marker, text, indent, block)) in raw.into_iter().enumerate() {
        let room = box_width.checked_sub(block).ok_or_else(out)?.max(0);
        let x = match note.align() {
            Align::Left => 0,
            Align::Centre => room.div_euclid(2),
            Align::Right => room,
        };
        let top = i64::try_from(position)
            .ok()
            .and_then(|n| n.checked_mul(line_height))
            .ok_or_else(out)?;
        lines.push(LaidLine {
            marker,
            text,
            x: Length::from_steps(x),
            indent: Length::from_steps(indent),
            width: Length::from_steps(block),
            top: Length::from_steps(top),
        });
    }
    let height = i64::try_from(lines.len())
        .ok()
        .and_then(|n| n.checked_mul(line_height))
        .ok_or_else(out)?;
    Ok(Laid {
        lines,
        line_height: Length::from_steps(line_height),
        width: Length::from_steps(box_width),
        height: Length::from_steps(height),
    })
}
