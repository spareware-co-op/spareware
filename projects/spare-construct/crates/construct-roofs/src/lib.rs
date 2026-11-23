//! Spare Construct: a simple roof, the data and shape half (CON-A4-F4-P3, first half; job 5.3).
//!
//! What a roof is, as data: a footprint (a floor's boundary, one loop of straight edges in plan),
//! what each edge does (its overhang, and its pitch if the roof rises from it), the form that
//! makes (flat, mono-pitch or gable), a roof type's layers from the top face down, and its base
//! on a level; then where every layer is, as a solid on the kernel's grid, and what a roof reads
//! back exactly. Map: projects/spare-construct/ARCHITECTURE.md, "Building elements"; the stored
//! form is in FORMAT.md beside this crate, the decisions in
//! notes/CON-A4-F4-P3-simple-roof.md.
//!
//! - [`pitch`]: [`Pitch`] (a slope or an angle, held as given), [`MOST_SLOPE`], [`ANGLE_RUN`].
//! - [`roof`]: [`Roof`], [`EdgeSpec`], [`Form`]: every check and refusal of a footprint and its
//!   edges, the stored and the plain (command) forms.
//! - [`shape`]: [`shape`] gives a roof's [`Shape`] for its layers: the eaves outline, each face's
//!   height, each layer's [`Solid`] ([`LayerSolid`]), a gable's [`Ridge`], the exact areas.
//! - [`measure`]: [`quantities`], [`read_only`], [`layer_read_only`].
//! - [`roof_type`]: [`new_roof_type`], [`with_layers`], [`layers_of`], under the owner tag
//!   [`OWNER`].
//! - [`record`]: the element record and its recipe ([`roof_record`], [`roof_recipe`],
//!   [`roof_of`], [`with_roof`], [`filled`]).
//! - [`defaults`]: [`default_roofs`], [`default_roof_types`].
//! - [`error`]: [`RoofError`], every refusal as a value.
//!
//! Shared, not copied: a roof's footprint is construct-slabs' [`construct_slabs::Boundary`] and
//! its type's layers construct-slabs' [`construct_slabs::Layers`], read and checked by that crate's
//! own code, and a roof type's block is spelt as a floor type's.
//!
//! Promises: every [`Roof`] has passed every check, so its shape is made for any layers; every
//! length is a whole number of construct-params' grid steps and every corner a point of the
//! kernel's grid, each rounded once by one rule ([`shape`]'s header); areas and volumes are exact;
//! the same data gives the same bytes; reading refuses anything writing would not give; nothing
//! panics. No float is stored; the one float computed is the tangent of a pitch given as an angle
//! ([`pitch`]'s header says why it is exact enough). Depends on construct-slabs, construct-solver,
//! construct-types, construct-params, construct-materials, construct-levels, construct-model,
//! construct-categories, kit-core, libm and num-bigint.
//!
//! Not here, each refused in words naming the job that brings it: hips, valleys, an uneven gable
//! and a sloped roof on any footprint but a rectangle along the plan's axes (job 6.2, roof
//! solving), an overhang on a flat footprint with an edge off the axes (job 6.2), and an opening
//! through a roof (job 4.2). The commands that make roofs are construct-api's
//! `cloud::roofs`, the server's view of them reads this crate's quantities, and the page's roof
//! tool is the app's. Nothing here is interim.

pub mod defaults;
pub mod error;
mod exact;
pub mod measure;
pub mod pitch;
pub mod record;
pub mod roof;
pub mod roof_type;
pub mod shape;

pub use defaults::{DefaultRoof, default_roof_types, default_roofs};
pub use error::RoofError;
pub use exact::GRID_LIMIT;
pub use measure::{
    AREA, LayerQuantity, MASS, PITCH, Quantities, RIDGE_HEIGHT, SLOPE_AREA, THICKNESS, VOLUME,
    layer_read_only, quantities, read_only,
};
pub use pitch::{ANGLE_RUN, MOST_SLOPE, Pitch};
pub use record::{
    EDGES_ARG, FOOTPRINT_ARG, FORM_ARG, LAYERS_ARG, OP_VERSION, OUTLINE_ARG, ROOF_KIND, ROOF_OP,
    SLOPES_ARG, filled, is_roof, roof_of, roof_recipe, roof_record, with_roof,
};
pub use roof::{EdgeSpec, Form, Roof};
pub use roof_type::{OWNER, layers_of, new_roof_type, with_layers};
pub use shape::{LayerSolid, Ridge, Shape, Solid, shape};
