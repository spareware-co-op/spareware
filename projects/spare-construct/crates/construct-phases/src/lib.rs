//! Spare Construct: phases and design options.
//!
//! Which stage of the work an element belongs to, and which design option, as plain values a
//! record can hold, plus the one rule a view uses to decide whether an element is shown and in
//! which state. Map: projects/spare-construct/ARCHITECTURE.md, "Phases"; parts CON-A2-F8-P1 and
//! CON-A2-F8-P2; the field spellings are in FORMAT.md.
//!
//! - [`phases`]: a project's ordered list of phases, and an element's created and demolished phase.
//! - [`options`]: option sets, their options and primary option, and an element's membership.
//! - [`filter`]: a view's phase and phase filter, and the rule that decides what it shows.
//! - [`error`]: why a change or a question is refused.
//!
//! Every change is a pure function that returns the new value, or an error the caller can read.
//! Nothing here reads a clock or makes an ID: the caller supplies each permanent ID, so the same
//! data always gives the same bytes. Wiring these values into element records and the edit log is
//! a later job.

pub mod error;
pub mod filter;
pub mod options;
pub mod phases;

pub use error::PhaseError;
pub use filter::{ElementState, OptionChoice, PhaseFilter, ViewSettings, Visibility, visibility};
pub use options::{DesignOption, DesignOptions, OptionRef, OptionSet};
pub use phases::{ElementStaging, Phase, PhaseList};
