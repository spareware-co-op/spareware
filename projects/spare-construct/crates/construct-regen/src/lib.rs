//! Spare Construct: regeneration.
//!
//! Dependency graph: only what changed is rebuilt, across every CPU core. Map: "Regeneration and
//! constraints". The order and caching wait on piece A03; for now every changed element is rebuilt
//! in log order.
//!
//! - [`stand_in`]: makes boxes until the geometry kernel can. The only file that knows. Interim:
//!   [`kernel`] replaces it once the browser can run the kernel's engine too.
//! - [`kernel`]: builds through the geometry kernel's engine seam, over whichever engine the
//!   program hands it. It names no engine.
//! - [`regenerator`]: reads the log and keeps a project's derived shapes up to date.
//! - [`parameters`]: how categories, parameters and types sit in a document's records, and how a
//!   recipe that names a parameter is filled before it is built; the server's commands read
//!   through it too, so the two cannot differ.
//! - [`levels`]: how an element's place and shape read its levels (its base, and a wall's top
//!   tied to the level above), and a document's levels as a page shows them.
//! - [`catalog`]: what a page shows to manage them: the parameter kinds, the categories, the
//!   project's parameters with where they are bound, and the types with their values in units.
//!
//! [`geometry`] is the default builder, the stand-in, which the browser and the server both run
//! today. A program that links an engine builds a [`kernel::Kernel`] over it in its own one place
//! instead: the server picks between the two by a setting.

#[cfg(feature = "server")]
pub mod catalog;
pub mod kernel;
#[cfg(feature = "server")]
pub mod levels;
#[cfg(feature = "server")]
pub mod parameters;
#[cfg(feature = "server")]
pub mod regenerator;
pub mod stand_in;

use construct_model::geometry::Geometry;
use std::sync::Arc;

/// What builds shapes by default.
#[must_use]
pub fn geometry() -> Arc<dyn Geometry> {
    Arc::new(stand_in::Boxes)
}
