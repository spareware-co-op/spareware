//! Spare Construct: dimensions bound to references (CON-A9-F1-P1): linear, ordinate, angular,
//! radial, diameter and arc length; dimension styles; overrides of the shown text; equality.
//!
//! A dimension is an element of the dimensions category, in one view. A linear one is an aligned,
//! horizontal or vertical measure between a string of references (a wall's faces or reference
//! line, a grid line, a level, a door's or window's centre or edges), with its offset line and a
//! witness line from each reference to it; an ordinate one reads each reference's distance from
//! the first along such a line; an angular one the angle between two lines; a radial, diameter
//! or arc length one a curve's radius, diameter or length. It has no shape: nothing builds it.
//! Map: projects/spare-construct/ARCHITECTURE.md; the stored form is in FORMAT.md beside this
//! crate and the choices are in notes/CON-A9-F1-P1-dimensions.md.
//!
//! - [`Dimension`], [`Kind`], [`Layout`], [`Direction`], [`Ray`], [`Reference`], [`Feature`]:
//!   the data. Its value is never stored: [`measure`] works each value out, exactly where it can
//!   be, from where [`Locator::locate`] says each reference is, every time it is asked, so a
//!   dimension follows its references when they move. A [`Length`](construct_params::Length) in
//!   grid steps or an [`Angle`](construct_params::Angle) in angle units comes out: whole numbers
//!   of any size all the way, the nearest step or unit only at the very end, and each
//!   [`Segment`] says whether its value is the true one (`exact`). An angle that is not a multiple
//!   of 45 degrees, and every arc length, is irrational: it is the nearest whole unit or step,
//!   within a bound [`trig`] proves (half a unit and 2^-80 of one; half a step and 2^-70 of one).
//! - [`Locator`], [`Locus`], [`Curve`], [`Sweep`], [`Missing`], [`Located`]: the one question a
//!   dimension asks of the model, behind a trait of ours. construct-api's `cloud::dimensions`
//!   answers it from the document's records; the tests answer it from a table. A curve is a type
//!   of ours, so a curved wall (job 14.4) answers with one and its dimensions need nothing new.
//! - [`Measured`], [`Segment`], [`Witness`], [`Figure`], [`EqualState`]: the answer. A reference
//!   that is gone, has no place in the view, runs along the offset line or is not the shape its
//!   dimension needs gives a [`DimensionWarning`] naming it (the shape of construct-warnings'
//!   warnings) and no value for what depends on it: never a wrong number.
//! - [`placed`], [`Placing`]: the layout a person's click means for an angle (which of the four
//!   angles, and the arc's radius), a radius or diameter (its direction) and an arc length (its
//!   offset), from where the references are now.
//! - Equality ("EQ"): [`Dimension::with_equalities`] stores which segments are to be equal, as
//!   data, and [`Measured::equalities`] says whether each holds now. [`equalize`] works out how
//!   far each element must move so that they are equal ([`Move`]); construct-api's
//!   `dimension.equalize` moves them through the edit log, one undo step.
//! - [`DimensionStyle`], [`Look`], [`Marker`], [`TextPlace`], [`Styles`]: dimension styles as
//!   project data: two built-in ones and a project's own, each a record with no shape.
//! - [`Overrides`]: a text in place of the value, a prefix and a suffix; the true value is always
//!   worked out and kept beside them.
//! - [`display`], [`display_angle`], [`labels`], [`Label`], [`TextSize`], [`PlainText`],
//!   [`Unmeasured`]: the value as the person's units (or the style's) show it, and the label's
//!   place. Drawing the text waits for job 7.4's text (piece A78): the size of a piece of text is
//!   asked behind [`TextSize`], and [`PlainText`] answers until then (interim; the piece that
//!   adds the text engine replaces it).
//! - [`Dimensions`]: a document's dimensions, with the lookups a command needs.
//!
//! Promises: records are the truth (main's ruling 7), and this crate is the types and pure
//! functions over them; wiring into the edit log and the server is construct-api's
//! `cloud::dimensions`. The same data gives the same bytes and the same answer on every machine
//! and run (no clock, no random number, no hash-map order, no float); reading refuses anything
//! writing would not give; no length is rounded to whole millimetres; nothing panics. Depends on
//! construct-model (the record), construct-categories (the category), construct-params (lengths,
//! angles, units, fractions), construct-grids (points in plan) and kit-core (IDs), and on
//! `num-bigint` (MIT OR Apache-2.0, version pinned) for exact whole numbers of any size.
//!
//! Not built here, with their places kept:
//! - Spot levels, coordinates and slopes (CON-A9-F1-P2, job 14.12) and auto dimensions (P3, job
//!   14.13): their references are [`Reference`]s and their styles [`DimensionStyle`]s, so each
//!   adds a [`Layout`] or a [`Kind`] and nothing else changes.
//! - Equality kept while a reference moves later: setting it moves the references once; after,
//!   [`EqualState`] reports whether it still holds. Keeping it on every move needs the solver on
//!   elements, a later piece.
//! - Dimensions in sections and elevations: a level is a place in those views only, whose frame
//!   (the cut line's) is job 8.1's. The locator is the one place that learns it.

mod dimension;
mod dimensions;
mod equal;
mod error;
mod exact;
mod layout;
mod locate;
mod measure;
mod overrides;
mod place;
mod reference;
mod style;
mod text;
pub mod trig;
mod warning;

pub use dimension::{
    CATEGORY_KEY, DIMENSION_KIND, DIMENSION_NAME, DIRECTION_X_KEY, DIRECTION_Y_KEY, Dimension,
    EQUAL_KEY, Equality, FIRST_KEY, Kind, LAYOUT_KEY, MAX_REFERENCES, OFFSET_KEY, ORDINATE_KEY,
    PREFIX_KEY, RADIUS_KEY, SECOND_KEY, STYLE_KEY, SUFFIX_KEY, TEXT_KEY, THROUGH_X_KEY,
    THROUGH_Y_KEY, TOWARD_X_KEY, TOWARD_Y_KEY, VIEW_KEY, X_KEY, Y_KEY, dimension_recipe,
};
pub use dimensions::Dimensions;
pub use equal::{Move, equalize};
pub use error::DimensionError;
pub use layout::{Direction, Layout, Ray};
pub use locate::{Curve, Located, Locator, Locus, Missing, Sweep, Why};
pub use measure::{EqualState, Figure, Measured, Segment, Witness, measure};
pub use overrides::{MAX_OVERRIDE, Overrides};
pub use place::{Placing, placed};
pub use reference::{Edge, Feature, REFERENCE_KIND, Reference, Side};
pub use style::{
    ANGLE_PLACES_KEY, ARROWS_ID, CENTRE_KEY, DimensionStyle, EXTENSION_KEY, GAP_KEY, HEIGHT_KEY,
    LENGTH_PLACES_KEY, LENGTH_UNIT_KEY, Look, MARKER_KEY, MAX_NAME, MOST_SIZE, Marker, STYLE_KIND,
    Styles, TEXT_PLACE_KEY, TICKS_ID, TextPlace, is_builtin, style_recipe,
};
pub use text::{Label, PlainText, TextSize, Unmeasured, display, display_angle, labels};
pub use warning::{
    DimensionWarning, OUT_OF_RANGE, REFERENCE_MISSING, REFERENCE_PARALLEL, Severity,
};
