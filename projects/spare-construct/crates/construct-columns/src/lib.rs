//! Spare Construct: architectural columns, the data half (CON-A5-F1-P1, job 14.7).
//!
//! What a column is, as data: a vertical member at a point or where two grid lines cross, standing
//! from a base level (plus an offset) up to a top level (plus an offset) or for an unconnected
//! height, turned by any angle, of a type that holds its section (rectangular or round, the sizes
//! the type's values) and its material. Everything a column's shape depends on is read when it is
//! built, so moving a level or a grid moves the column and no column record changes. Map:
//! projects/spare-construct/ARCHITECTURE.md, "Levels and grids"; the stored form is in FORMAT.md
//! beside this crate; the choices behind it are in the decision note
//! `projects/spare-construct/notes/CON-A5-F1-P1-columns.md`.
//!
//! - [`section`]: [`Section`] (rectangular or round, sizes checked), [`Section::ring`] (the corners
//!   the builders extrude, turned about the centre, each rounded once) and [`Section::area`].
//! - [`stored`], [`column_type`]: [`ColumnSpec`], the text a type's system block holds, and
//!   [`new_column_type`], [`with_spec`], [`spec_of`].
//! - [`column`]: [`Column`], [`Position`] (a point, or two grid lines' crossing), [`Top`] (a level
//!   and an offset, or an unconnected height), the element record ([`column_record`],
//!   [`column_of`], [`with_column`]), the recipe, [`filled_by_type`] and [`finish`] (what the
//!   builders take), [`height_between`] and [`check_height`] (a top below its base is refused by
//!   name).
//! - [`measure`]: [`quantities`] and [`read_only`]: height, section area, volume and mass,
//!   worked out, never stored.
//! - [`defaults`]: [`default_columns`], [`default_column_types`]: three types of our own.
//! - [`ColumnError`]: every refusal, as a value.
//!
//! Promises: every length is exact in construct-params' unit and no float is stored; the same data
//! gives the same bytes; reading refuses anything writing would not give; nothing panics. Depends
//! on construct-model (the record), construct-categories, construct-levels (ties and heights),
//! construct-grids (crossings), construct-types, construct-params, construct-materials and
//! kit-core; `serde_json` and `libm` (a turn's sine and cosine).
//!
//! Interim, each with the piece that replaces it (rows of the interim register, geom-solidcore's
//! `tests/interim.json`: the places "construct-columns' Section, a round section" and
//! "construct-columns' `Section::ring`, a turned column"):
//! - A round section is built as a regular polygon of 48 corners, as the engine seam's extrusion
//!   takes polygons only; the kernel's exact sweep of arcs (geom-sweep, chapter 13.4) replaces it,
//!   and the same tests judge it.
//! - A turn is the true turn of each corner rounded once, through `libm`'s sine and cosine with a
//!   stated bound ([`section`]); construct-modify's exact rational turn (piece A80) replaces it
//!   once it is on main.
//!
//! Not built here, with their places kept:
//! - Joining a column into the walls it touches (a wall's layers wrap or stop at the column):
//!   train 10's wall joins (piece A60). The place: the layered wall builder reads the columns on
//!   its level whose rings it crosses, as it reads its joined walls, and cuts its layers back to
//!   the ring.
//! - Slanted columns, and columns attached to beams and floors (CON-A5-F1-P1's other words);
//!   structural columns of steel sections (CON-A5-F1-P4, section libraries).
//! - The page tool, plan and 3D drawing and the two-browser section: the second half, a later lane.

pub mod column;
pub mod column_type;
pub mod defaults;
pub mod error;
pub mod measure;
pub mod section;
pub mod stored;

pub use column::{
    AT_ARG, COLUMN_KIND, COLUMN_OP, Column, GRIDS_ARG, HEIGHT_ARG, MATERIAL_ARG, MOST_HEIGHT,
    OP_VERSION, Position, RING_ARG, SECTION_ARG, TURN_ARG, Top, check_height, column_of,
    column_recipe, column_record, filled_by_type, finish, grids_read, height_between, is_column,
    is_column_recipe, reads_section, section_arg, tie_arg, with_column,
};
pub use column_type::{new_column_type, spec_of, with_spec};
pub use defaults::{DefaultColumn, default_column_types, default_columns};
pub use error::ColumnError;
pub use measure::{AREA, HEIGHT, MASS, Quantities, VOLUME, quantities, read_only};
pub use section::{MOST_SIDE, ROUND_SIDES, Section, twice_area};
pub use stored::{ColumnSpec, FORMAT_VERSION, OWNER};
