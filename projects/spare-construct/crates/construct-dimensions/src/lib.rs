//! Spare Construct: linear dimensions bound to references (CON-A9-F1-P1; the data half).
//!
//! A dimension is an element of the dimensions category, in one view: an aligned, horizontal or
//! vertical measure between a string of references (a wall's faces or reference line, a grid line, a
//! level, a door's or window's centre or edges), with its offset line and a witness line from each
//! reference to it. It has no shape: nothing builds it. Map:
//! projects/spare-construct/ARCHITECTURE.md; the stored form is in FORMAT.md beside this crate and
//! the choices are in notes/CON-A9-F1-P1-dimensions.md.
//!
//! - [`Dimension`], [`Layout`], [`Direction`], [`Reference`], [`Feature`]: the data. Its value is
//!   never stored: [`measure`] works each segment out, exactly, from where [`Locator::locate`]
//!   says each reference is, every time it is asked, so a dimension follows its references when
//!   they move. A [`Length`](construct_params::Length) in grid steps comes out: whole numbers
//!   of any size all the way, the nearest step only at the very end, and each [`Segment`] says
//!   whether its value is the true one (`exact`).
//! - [`Locator`], [`Locus`], [`Missing`], [`Located`]: the one question a dimension asks of the
//!   model, behind a trait of ours. construct-api's `cloud::dimensions` answers it from the
//!   document's records; the tests answer it from a table.
//! - [`Measured`], [`Segment`], [`Witness`], [`EqualState`]: the answer. A reference that is gone,
//!   or has no place in the view, or runs along the offset line, gives a [`DimensionWarning`]
//!   naming it (the shape of construct-warnings' warnings) and no value for the segments it ends:
//!   never a wrong number.
//! - Equality ("EQ"): [`Dimension::with_equalities`] stores which segments are to be equal, as
//!   data. It is not enforced here: nothing moves a reference or an element to make them so (that
//!   needs the solver on elements, a later job). [`Measured::equalities`] says whether each holds
//!   now, to the exact value.
//! - [`display`], [`labels`], [`Label`], [`TextSize`], [`Unmeasured`]: the value as the person's
//!   units show it (construct-params), and the label's place. Drawing the text waits for job 7.4's
//!   text (piece A78): the size of a piece of text is asked behind [`TextSize`], and [`Unmeasured`]
//!   answers until then (interim; the piece that adds the text engine replaces it).
//! - [`Dimensions`]: a document's dimensions, with the lookups a command needs.
//!
//! Promises: records are the truth (main's ruling 7), and this crate is the types and pure
//! functions over them; wiring into the edit log and the server is construct-api's
//! `cloud::dimensions`. The same data gives the same bytes and the same answer on every machine
//! and run (no clock, no random number, no hash-map order); reading refuses anything writing
//! would not give; no length is rounded to whole millimetres or held as a float; nothing panics.
//! Depends on construct-model (the record), construct-categories (the category), construct-params
//! (lengths, units, fractions), construct-grids (points in plan) and kit-core (IDs), and on
//! `num-bigint` (MIT OR Apache-2.0, version pinned) for exact whole numbers of any size.
//!
//! Not built here, with their places kept:
//! - Angular, radial, arc and ordinate dimensions, spot levels and slopes (CON-A9-F1-P2) and auto
//!   dimensions (P3): other kinds of annotation, each its own piece.
//! - Dimension styles (arrows, text size, units overrides) and text overrides: they come with the
//!   text of job 7.4, which owns fonts and sizes.
//! - Enforcing equality (above), and dimensions in sections and elevations: a level is a place in
//!   those views only, whose frame (the cut line's) is job 8.1's. The locator is the one place
//!   that learns it.
//! - The page tool, the drawing in the plan and the two-browser section: the second half.

mod dimension;
mod dimensions;
mod error;
mod exact;
mod layout;
mod locate;
mod measure;
mod reference;
mod text;
mod warning;

pub use dimension::{
    CATEGORY_KEY, DIMENSION_KIND, DIMENSION_NAME, DIRECTION_X_KEY, DIRECTION_Y_KEY, Dimension,
    EQUAL_KEY, Equality, LAYOUT_KEY, MAX_REFERENCES, THROUGH_X_KEY, THROUGH_Y_KEY, VIEW_KEY, X_KEY,
    Y_KEY, dimension_recipe,
};
pub use dimensions::Dimensions;
pub use error::DimensionError;
pub use layout::{Direction, Layout};
pub use locate::{Located, Locator, Locus, Missing, Why};
pub use measure::{EqualState, Measured, Segment, Witness, measure};
pub use reference::{Edge, Feature, REFERENCE_KIND, Reference, Side};
pub use text::{Label, TextSize, Unmeasured, display, labels};
pub use warning::{
    DimensionWarning, OUT_OF_RANGE, REFERENCE_MISSING, REFERENCE_PARALLEL, Severity,
};
