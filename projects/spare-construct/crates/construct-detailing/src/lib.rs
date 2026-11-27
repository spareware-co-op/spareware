//! Spare Construct: detail drawing in a view (CON-A9, job 14.8; the data and commands half).
//!
//! What a real architect's drawing holds that is not in the model: lines, filled regions, 2D
//! components and spot slopes that belong to one view. Each is an element record of its own kind
//! with no shape (nothing builds it), carries the ID of the one view it is in, and is drawn over
//! the model in that view and in no other. Map: projects/spare-construct/ARCHITECTURE.md; the
//! stored forms are in FORMAT.md beside this crate and the choices in
//! notes/CON-A9-F4-P1-detail-drawing.md.
//!
//! - [`DetailLine`], [`Curve`]: a straight line or an arc with a line style. An arc is the sketch
//!   solver's (centre, start, end, anticlockwise) and is checked by its own `check_drawn`.
//! - [`StyleRef`], [`BuiltIn`], [`OwnStyle`], [`OwnStyles`], [`LineStyle`]: six built-in line
//!   styles (a model pen of construct-styles, a line pattern, a colour) and a person's own, stored
//!   in the document as records (`detail.linestyle`).
//! - [`Region`], [`Fill`], [`RegionArea`], [`check_outline`]: filled regions (a fill pattern by
//!   name and a colour, a boundary line style) and masking regions (opaque, no pattern); the
//!   outline an exactly simple loop; the area exact.
//! - [`ComponentRecipe`], [`Library`], [`DetailComponent`], [`DetailRepeat`], [`place`]: 2D
//!   components from recipes of a kind of their own (lines and fills, our own designs), placed at
//!   a position turned by any angle (the modify tools' exact rule) and repeated along a line.
//! - [`SpotSlope`], [`Faces`], [`Plane`], [`Reading`], [`read`]: the slope of a floor or roof face
//!   at a point, never stored, worked out each time; refused by name ([`SlopeRefusal`]) when the
//!   face is missing, flat or upright, or the point is over none.
//! - [`Detailing`], [`ViewItems`], [`ItemKind`]: a document's items and the lookups a command
//!   needs; [`render`], [`Settings`], [`Rendered`]: a view's items as what construct-drawing lays
//!   over the model.
//!
//! Promises: records are the truth (main's ruling 7); this crate is the types and pure functions
//! over them, and construct-api's `cloud::detailing` puts them through the one ordered edit log.
//! Every length is a construct-params `Length` in grid steps, every angle an `Angle` in whole
//! angle units; no float is stored and the one float (a slope's angle) is display only; the same
//! data gives the same bytes and the same answer on every machine and run; reading refuses
//! anything writing would not give; nothing panics. Depends on construct-model, -categories,
//! -params, -grids, -styles, -drawing, -solver and -modify, kit-core, and on `num-bigint`,
//! `num-traits` (MIT OR Apache-2.0), `libm` (MIT), `serde` and `serde_json` (MIT OR Apache-2.0),
//! each version pinned.
//!
//! Interim: fill patterns are construct-drawing's table (construct-styles holds pens and line
//! patterns only; its own fill patterns are a later job, and this crate then reads them there);
//! line patterns print solid, as every line does. Not here, each with its place kept: text beside
//! a spot slope (job 7.4), mirroring a placed component, a component with parameters or a 3D
//! part, detail items in a sheet's legend, and drafting views (a view with no model), which are
//! job 8's.

mod component;
mod detailing;
mod error;
mod line;
mod num;
mod placed;
mod record;
mod region;
mod render;
mod slope;
mod style;

pub use component::{
    ComponentFill, ComponentLine, ComponentRecipe, FORMAT_VERSION, Library, MOST_PARTS, Placed,
    PlacedFill, PlacedLine, REACH_STEPS, SCHEMA, place,
};
pub use detailing::{Detailing, ItemKind, KIND_PREFIX, ViewItems, is_detail_kind};
pub use error::{DetailError, SlopeRefusal};
pub use line::{Curve, DetailLine, LINE_KIND, LINE_NAME};
pub use placed::{
    COMPONENT_KIND, DetailComponent, DetailRepeat, MOST_REPEATS, REPEAT_KIND, normalised,
};
pub use record::{CATEGORY_KEY, VIEW_KEY};
pub use region::{Fill, MASKING_KIND, MAX_CORNERS, REGION_KIND, Region, RegionArea, check_outline};
pub use render::{ARROW_MM, PAPER, Rendered, Settings, Skipped, render};
pub use slope::{
    Faces, Plane, Reading, SLOPE_KIND, SpotSlope, polygon_covers, polygon_covers_inside, read,
};
pub use style::{
    BuiltIn, LineStyle, OwnStyle, OwnStyles, Printed, STYLE_KIND, StyleRef, colour_of,
    colour_spelled,
};
