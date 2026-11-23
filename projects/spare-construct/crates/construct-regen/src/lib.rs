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
//! - [`datum`]: the display shape of grids and reference planes, which have no solid; both
//!   builders hand a datum's recipe to it first.
//! - [`regenerator`]: reads the log and keeps a project's derived shapes up to date.
//! - [`parameters`]: how categories, parameters and types sit in a document's records, and how a
//!   recipe that names a parameter is filled before it is built; the server's commands read
//!   through it too, so the two cannot differ.
//! - [`levels`]: how an element's place and shape read its levels (its base, and a wall's top
//!   tied to the level above), and a document's levels as a page shows them.
//! - [`catalog`]: what a page shows to manage them: the parameter kinds, the categories, the
//!   project's parameters with where they are bound, and the types with their values in units.
//! - [`floor_plan`]: a floor (`floor`) as both builders read it: its filled recipe's loops and
//!   layers in grid steps (job 4.3). The kernel builds one solid per layer, the stand-in one box.
//! - [`slabs`]: how a floor's recipe reads its layers from its floor type and has its boundary
//!   checked (construct-slabs), filled with the parameters by [`parameters::recipe_at`].
//!
//! - [`wall_plan`]: a layered wall (`wall.layered`) in plan: where each layer face lies on the
//!   kernel's grid, rounded once per face, read from its filled recipe (job 4.1). Both builders
//!   build it: the kernel one solid per layer, the stand-in one box of its whole thickness.
//! - [`walls`]: how a layered wall's recipe reads its layers from its wall type, filled with the
//!   parameters by [`parameters::recipe_at`].
//! - [`joins`]: how a layered wall's recipe is filled with its joints with the walls it meets
//!   (job 5.1): the walls are found from the stored walls of its level, the rule is
//!   construct-walls', and the regenerator and the server's `/filled` both fill through it.
//!
//! - [`hosting`]: how a hosted element (a door) sits in its record, on a face of its host (a wall)
//!   at an offset along and up it, and where that puts it; the regenerator builds a wall with its
//!   doors through it, and the server's commands check through it, so the two cannot differ.
//!
//! [`geometry`] is the default builder, the stand-in, which the browser and the server both run
//! today. A program that links an engine builds a [`kernel::Kernel`] over it in its own one place
//! instead: the server picks between the two by a setting. [`face_names`] names a recipe's faces
//! as the builder of an edition names them, without building it.

#[cfg(feature = "server")]
pub mod catalog;
pub mod datum;
pub mod floor_plan;
#[cfg(feature = "server")]
pub mod hosting;
#[cfg(feature = "server")]
pub mod joins;
pub mod kernel;
#[cfg(feature = "server")]
pub mod levels;
#[cfg(feature = "server")]
pub mod parameters;
#[cfg(feature = "server")]
pub mod regenerator;
#[cfg(feature = "server")]
pub mod slabs;
pub mod stand_in;
pub mod wall_plan;
#[cfg(feature = "server")]
pub mod walls;

use construct_model::geometry::{Edition, FaceName, Geometry, GeometryError, Recipe};
use std::sync::Arc;

/// What builds shapes by default.
#[must_use]
pub fn geometry() -> Arc<dyn Geometry> {
    Arc::new(stand_in::Boxes)
}

/// The persistent names of the faces `recipe` builds to under `edition`, without building it: the
/// names the builder of that edition writes from the recipe ([`stand_in::names`],
/// [`kernel::names`]). A hosted element names its host's face by one of these.
///
/// # Errors
///
/// `EditionUnavailable` for an edition neither builder here is; the builder's own error for a
/// recipe it cannot build.
pub fn face_names(edition: &Edition, recipe: &Recipe) -> Result<Vec<FaceName>, GeometryError> {
    if *edition == stand_in::Boxes.edition() {
        stand_in::names(recipe)
    } else if edition.engine == kernel::ENGINE && edition.name_scheme == kernel::NAME_SCHEME {
        kernel::names(recipe)
    } else {
        Err(GeometryError {
            code: "EditionUnavailable".to_owned(),
            message: format!(
                "faces built by {} {} are not named here",
                edition.engine, edition.version
            ),
        })
    }
}
