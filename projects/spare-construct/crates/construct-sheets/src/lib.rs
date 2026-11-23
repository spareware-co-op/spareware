//! Spare Construct: sheets and the viewports placed on them (CON-A11-F1-P1, CON-A11-F1-P2; the
//! data half).
//!
//! A sheet is an element of the sheets category: a number, a name and a paper. A viewport is an
//! element of the viewports category: a view placed on a sheet, with a position on the paper, a
//! title on or off, and alignments to other viewports. Neither has a shape. Map:
//! projects/spare-construct/ARCHITECTURE.md; the stored form is in FORMAT.md beside this crate
//! and the choices are in notes/CON-A11-F1-P1-sheets.md.
//!
//! - [`Sheet`], [`Paper`], [`PaperSize`], [`Orientation`]: a sheet's number (unique in the
//!   document), name and paper, a standard ISO 216 size or a custom size, every length exact.
//! - [`Viewport`], [`Alignment`], [`Anchor`]: a view on a sheet, its centre on the paper, its
//!   title, and the viewports it follows across and up the paper.
//! - [`Sheets`]: a document's sheets and viewports, and the rules a command checks: unique
//!   numbers, a view on one sheet only, a centre on the paper, alignments on one sheet and never
//!   in a loop, a followed viewport kept.
//! - [`Sheets::layout`], [`ViewFacts`], [`Placed`], [`Outline`]: for a sheet, each viewport's
//!   centre (alignments worked out) and outline on the paper, from each view's extent and scale.
//! - [`with_standard`], [`STANDARD`]: the sheet parameters (drawn by, checked by, issue date,
//!   revision...) as construct-params definitions bound to the sheets category.
//! - [`SheetError`]: every refusal, as a value.
//!
//! Promises: records are the truth (main's ruling 7), and this crate is the types and pure
//! functions over them; wiring into the edit log and the server is construct-api's
//! `cloud::sheets`. The same data gives the same bytes and the same answer on every machine and
//! run; reading refuses anything writing would not give; no length is rounded to whole
//! millimetres or held as a float; nothing panics. Depends on construct-model (the record),
//! construct-categories (the two categories), construct-params (lengths and parameters),
//! construct-styles (the scale) and kit-core (IDs). Nothing here is interim.
//!
//! Not built here, with their places kept: drawing a sheet, title blocks (job 8.6), the page,
//! printing (job 8.7), guide grids, sheet collections and revisions (A11-F1-P4, A11-F3-P1), and a
//! view's extent for every kind of view (it is given to the layout; crops are a later job).

mod error;
mod layout;
mod paper;
mod parameters;
mod sheet;
mod sheets;
mod viewport;

pub use error::SheetError;
pub use layout::{Outline, Placed, ViewFacts};
pub use paper::{MAX_CUSTOM, MIN_CUSTOM, Orientation, Paper, PaperSize};
pub use parameters::{STANDARD, standard_definitions, with_standard};
pub use sheet::{
    CATEGORY_KEY, HEIGHT_KEY, MAX_NUMBER_LEN, NUMBER_KEY, ORIENTATION_KEY, PAPER_KEY, SHEET_KIND,
    Sheet, WIDTH_KEY, sheet_recipe,
};
pub use sheets::Sheets;
pub use viewport::{
    ALIGN_X_ANCHOR_KEY, ALIGN_X_KEY, ALIGN_Y_ANCHOR_KEY, ALIGN_Y_KEY, Alignment, Anchor, SHEET_KEY,
    TITLE_KEY, VIEW_KEY, VIEWPORT_KIND, VIEWPORT_NAME, Viewport, X_KEY, Y_KEY, viewport_recipe,
};
