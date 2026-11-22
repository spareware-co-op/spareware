//! Spare Construct: pens and line patterns (CON-A8-F3-P1, first half).
//!
//! What a drawing's lines are made with, as project data: pens that print thinner on a
//! small-scale drawing, and line patterns of dashes, gaps and dots. Everything is exact whole
//! micrometres of printed length; no float is stored or compared.
//!
//! - [`pens`]: [`pens::PenTable`] for model lines (a width per view scale, with the lookup rule
//!   written on the type) and [`pens::AnnotationPens`] for annotation and perspective lines (one
//!   width).
//! - [`patterns`]: [`patterns::PatternTable`], six built-in patterns that cannot be removed and
//!   the project's own, each checked.
//! - [`default`]: our default metric set, built on the ISO 128-20 ladder of line widths, and the
//!   rule for how a pen steps down with scale.
//! - [`stored`]: [`stored::LineStyles`], the pens and patterns together, and its stored form:
//!   the same styles are always the same bytes.
//! - [`units`] and [`error`]: the exact values, and every refusal as a value.
//!
//! Changes are pure functions that return a new table (`with_pen_widths`, `with_pattern`, and
//! so on). Putting them through the edit log is a later job.
//!
//! Object styles (CON-A8-F3-P1, second half) sit beside [`stored::LineStyles`] in their own
//! table, naming a pen by its number and a pattern by its [`patterns::PatternId`]:
//!
//! - [`object_styles`]: [`object_styles::ObjectStyles`], for each category and subcategory the
//!   pen in projection, the pen where a view cuts it (none if it cannot be cut), a colour and a
//!   pattern; the inheritance rule for a subcategory with no entry; the lookup the drawing code
//!   calls; pure changes; and the stored form.
//! - [`object_default`]: our default object styles for every built-in category, and why.
//!
//! Wiring the changes into the edit log, and the per-view graphics rules (CON-A8-F3-P2), are
//! later jobs.

pub mod default;
pub mod error;
pub mod object_default;
pub mod object_styles;
pub mod patterns;
pub mod pens;
pub mod stored;
pub mod units;

pub use error::StyleError;
pub use object_styles::{Colour, Edge, EdgeStyle, ObjectStyle, ObjectStyleError, ObjectStyles};
pub use patterns::{LinePattern, PatternId, PatternTable, Segment};
pub use pens::{AnnotationPen, AnnotationPens, Pen, PenTable};
pub use stored::LineStyles;
pub use units::{Micrometres, Scale};
