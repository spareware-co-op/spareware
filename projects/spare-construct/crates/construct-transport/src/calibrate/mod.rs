//! Calibration: matrix estimation against traffic counts, and the validation of a model against them.
//!
//! Port of calibrate.js (contract C3: the ME2-style matrix estimation and the validation summary against NZTA TMDG
//! 2019 with UK TAG M3.1 alongside) and of the calculation parts of the calibration panel, features/calibration.js:
//! the criteria library, count roles, counts and screenlines against a result, GEH and the pass and fail verdicts,
//! convergence and matrix-estimation checks, trip-end factors and the stress test. The panel's HTML, charts and
//! buttons are not ported; everything that turns a model's results and the counts into numbers or verdicts is.
//!
//! The count data comes from [`crate::datahub`]. The prototype's asynchronous runs are synchronous here and report
//! progress through a callback.

mod assess;
mod common;
mod criteria;
mod estimate;
mod exports;
mod fit;
mod jslog;
mod m1;
mod panel;
mod report;
mod roles;
mod rows;
mod screen;
mod sites;
mod stats;
mod store;
mod suggest;
mod turncounts;
mod turnrows;
mod workflow;

pub use assess::*;
pub use common::*;
pub use criteria::*;
pub use estimate::*;
pub use exports::*;
pub use fit::*;
pub use jslog::log as js_log;
pub use m1::*;
pub use panel::*;
pub use report::*;
pub use roles::*;
pub use rows::*;
pub use screen::*;
pub use sites::*;
pub use stats::*;
pub use store::*;
pub use suggest::*;
pub use turncounts::*;
pub use turnrows::*;
pub use workflow::*;
