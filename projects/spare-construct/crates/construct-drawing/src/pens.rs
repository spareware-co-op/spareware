//! The printed width of every line of a drawing at a view scale.
//!
//! What it does: [`widths`] looks each line up in construct-styles' object styles
//! (`ObjectStyles::lookup`) by its category, as a cut edge for a [`LineKind::Cut`] line and a
//! projected edge otherwise, at the scale given, and returns its pen's printed width in whole
//! micrometres, one per line in the drawing's order. The drawing itself holds no width: weights
//! are paper sizes, applied at display and export, so one drawing prints at any scale (chapter
//! 18.2 of the geometry architecture; note A12, "From line to pen").
//!
//! Not yet: bands, hidden lines, phases and per-view overrides (note A12's build-order step 10 and
//! CON-A8-F3-P2); a line's pattern and colour are in the same lookup and are read by [`style`].

use construct_categories::CategoryTable;
use construct_materials::Materials;
use construct_styles::{Edge, EdgeStyle, LineStyles, Micrometres, ObjectStyles, Scale};

use crate::DrawingError;
use crate::drawing::{Drawing, Line, LineKind};
use crate::hatch::FillPatterns;

/// The styles a drawing is printed with: the category tree, the pens and patterns, the object
/// styles, the materials (for a cut region's fill and its colour) and the fill patterns.
#[derive(Clone, Copy, Debug)]
pub struct Styles<'a> {
    /// The categories, built-in and the project's own.
    pub categories: &'a CategoryTable,
    /// The pens and line patterns.
    pub lines: &'a LineStyles,
    /// The object styles.
    pub objects: &'a ObjectStyles,
    /// The materials: a cut region's material gives its fill pattern, by name, and its colour.
    pub materials: &'a Materials,
    /// The fill patterns, by name.
    pub fills: &'a FillPatterns,
}

/// The pen, colour and pattern `line` prints with at `scale`.
///
/// # Errors
///
/// [`DrawingError::Style`] for what the object styles refuse: an unknown category, or a cut line
/// of a category that cannot be cut.
pub fn style(styles: Styles<'_>, line: &Line, scale: Scale) -> Result<EdgeStyle, DrawingError> {
    let edge = match line.kind {
        LineKind::Cut => Edge::Cut,
        LineKind::Projected => Edge::Projected,
    };
    styles
        .objects
        .lookup(styles.categories, styles.lines, line.category, edge, scale)
        .map_err(|why| DrawingError::Style(format!("{why:?}")))
}

/// The printed width of each line of `drawing` at `scale`, in the drawing's order.
///
/// # Errors
///
/// As [`style`], for the first line refused.
pub fn widths(
    drawing: &Drawing,
    styles: Styles<'_>,
    scale: Scale,
) -> Result<Vec<Micrometres>, DrawingError> {
    drawing
        .lines()
        .iter()
        .map(|line| style(styles, line, scale).map(|s| s.width))
        .collect()
}
