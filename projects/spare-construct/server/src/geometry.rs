//! What builds this server's shapes: the one place the server picks a builder, by its `geometry`
//! setting (`STACK_GEOMETRY`).
//!
//! - `stand-in` (the default): `construct_regen::geometry()`, the builder the browser runs too, so
//!   a browser builds what this server builds and compares it with the server's checkpoints.
//! - `kernel`: `construct_regen::kernel::Kernel` over the geometry kernel's engine seam, with
//!   geom-solidcore's `SolidcoreEngine` behind it on the building grid. Only a server built with
//!   the `kernel` feature has it, since that links solidcore (a long build, decision 32.70). The
//!   browser builds a kernel-built document with the same builder over the same engine, its own
//!   package (`shapes-kernel`), which only such a server serves (`http.rs`); a browser without it
//!   is a viewer of the stored shapes (construct-model's FORMAT.md, "A document").
//!
//! Interim: solidcore answers behind the engine seam where the kernel's own exact operations do
//! not yet; `projects/engines/geometry/geom-solidcore/tests/interim.json` says which part answers
//! each operation and what replaces it. Moving to an exact engine of the kernel's own changes the
//! one constructor line below.

use clap::ValueEnum;
use construct_model::geometry::Geometry;
use std::sync::Arc;

/// Which builder makes this server's shapes.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builder {
    /// The default builder, the browser's too.
    StandIn,
    /// The geometry kernel's engine seam.
    Kernel,
}

/// The builder `builder` names.
///
/// # Errors
///
/// When this server was built without the kernel, or the kernel's builder can't run on its
/// engine's grid, in words.
pub fn geometry(builder: Builder) -> Result<Arc<dyn Geometry>, String> {
    match builder {
        Builder::StandIn => Ok(construct_regen::geometry()),
        Builder::Kernel => kernel(),
    }
}

#[cfg(feature = "kernel")]
fn kernel() -> Result<Arc<dyn Geometry>, String> {
    use construct_regen::kernel::{Kernel, STEP};
    use geom_solidcore::engine::SolidcoreEngine;
    let built = Kernel::new(SolidcoreEngine::new(STEP)).map_err(|e| e.to_string())?;
    Ok(Arc::new(built))
}

#[cfg(not(feature = "kernel"))]
fn kernel() -> Result<Arc<dyn Geometry>, String> {
    Err("this server was built without the kernel: build it with --features kernel".to_owned())
}

/// What draws a view's plan ([`crate::plan`]): the kernel's builder over solidcore's engine, on a
/// server built with the `kernel` feature (whatever builder makes its shapes: a plan is drawn
/// from the stored recipes, not from the stored shapes), and nothing on a server built without
/// it, which says so when a plan is asked for. Interim, as [`geometry`]'s kernel: the engine that
/// answers the cut is the one line below.
#[cfg(feature = "kernel")]
#[must_use]
pub fn drawer() -> Option<Arc<dyn crate::plan::Drawer>> {
    use construct_regen::kernel::{Kernel, STEP};
    use geom_solidcore::engine::SolidcoreEngine;
    let built = Kernel::new(SolidcoreEngine::new(STEP)).ok()?;
    Some(Arc::new(built))
}

/// What draws a view's plan: nothing, on a server built without the `kernel` feature.
#[cfg(not(feature = "kernel"))]
#[must_use]
pub fn drawer() -> Option<Arc<dyn crate::plan::Drawer>> {
    None
}
