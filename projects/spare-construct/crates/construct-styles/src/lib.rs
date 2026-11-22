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
//! The next job (CON-A8-F3-P1, second half: object styles) adds the line weight, colour and
//! pattern each category and subcategory draws with. It belongs beside [`stored::LineStyles`] as
//! its own module and its own table, naming a pen by its number and a pattern by its
//! [`patterns::PatternId`]; nothing here has to change for it.

pub mod default;
pub mod error;
pub mod patterns;
pub mod pens;
pub mod stored;
pub mod units;

pub use error::StyleError;
pub use patterns::{LinePattern, PatternId, PatternTable, Segment};
pub use pens::{AnnotationPen, AnnotationPens, Pen, PenTable};
pub use stored::LineStyles;
pub use units::{Micrometres, Scale};
