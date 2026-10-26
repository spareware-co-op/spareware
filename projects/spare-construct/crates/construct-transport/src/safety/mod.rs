//! Crash data and safety analysis near junctions and links (port of data.js crash analysis, CAS windows and fire-and-emergency conflation).
//!
//! Crashes come from the NZTA crash analysis system (CAS) as one record each, kept in a project as compact tuples
//! ([`cas_tuple`]). [`cas_window`] says which years to ask for. [`SafetyModel`] assigns the crashes to a scenario's
//! links and junctions and gives link and junction risk with KiwiRAP-style bands and a study-area summary
//! ([`SafetySummary`]); [`crashes_near`] lists the crashes near one link or junction; [`conflate_fc`] puts Auckland
//! Transport Future Connect road classes on links.

mod assign;
mod cas;
mod conflate;
mod risk;

pub use assign::*;
pub use cas::*;
pub use conflate::*;
pub use risk::*;
