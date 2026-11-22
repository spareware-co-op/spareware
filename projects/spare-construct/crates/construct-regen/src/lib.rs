//! Spare Construct: regeneration.
//!
//! Dependency graph: only what changed is rebuilt, across every CPU core. Map: "Regeneration and
//! constraints". The order and caching wait on piece A03; for now every changed element is rebuilt
//! in log order.
//!
//! - [`stand_in`]: makes boxes until the geometry kernel can. The only file that knows.
//! - [`regenerator`]: reads the log and keeps a project's derived shapes up to date.
//! - [`parameters`]: how categories, parameters and types sit in a document's records, and how a
//!   recipe that names a parameter is filled before it is built; the server's commands read
//!   through it too, so the two cannot differ.
//!
//! [`geometry`] is the one line that picks what builds shapes. The server and the browser both
//! call it, so they can't differ, and it is the line that changes when the kernel arrives.

#[cfg(feature = "server")]
pub mod parameters;
#[cfg(feature = "server")]
pub mod regenerator;
pub mod stand_in;

use construct_model::geometry::Geometry;
use std::sync::Arc;

/// What builds shapes.
#[must_use]
pub fn geometry() -> Arc<dyn Geometry> {
    Arc::new(stand_in::Boxes)
}
