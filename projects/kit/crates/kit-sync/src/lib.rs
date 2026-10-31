//! Spare Kit: sync and history.
//!
//! Multi-user sync of operations, version history, branches and offline work. Map: root
//! ARCHITECTURE.md; note K08; note A05 for the method (Replicache's commands up, changes down,
//! with a server-held order, as Figma describes it).
//!
//! - [`orderer`]: one log for each scope, one batch at a time, each batch one transaction.
//! - [`claims`]: light, automatic, per element.
//! - [`replay`]: reads a log back and proves the stored pieces follow from it.
//! - [`live`]: what a browser connection sees.
//!
//! Nothing here knows a database or a program. It runs on `kit-store`'s traits, and each program
//! brings its commands through [`orderer::Commands`], so every failure below can be staged in a
//! plain test. See `tests/failures.rs`, and `tests/fuzz_orderer.rs` for faults in random order.

pub mod claims;
pub mod live;
pub mod orderer;
pub mod replay;

pub use orderer::{
    Applied, Clock, Commands, Context, Elsewhere, Orderer, Orderers, Outcome, Reply, Submit,
    SystemClock,
};
pub use replay::{LogCheck, read_log, verify_log};
