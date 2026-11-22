//! The built-in categories: a fixed tree, in code, that no project can change.
//!
//! How the list was chosen. A building model holds three kinds of thing that a drawing or a
//! schedule has to tell apart: the parts the building is made of, the marks drawn over them, and
//! the pages that show them. So the top level has one entry per part a plan, section or schedule
//! names on its own (walls, doors, floors, rooms, furniture, a plumbing fixture), one per mark a
//! sheet carries (dimensions, tags, text, section markers), and one per page kind (views, sheets,
//! schedules). A subcategory exists only where a drawing has to draw one part of an element
//! differently from the rest: a door's panel, frame and swing arc, a wall's core and finish cut
//! lines, a stair's tread, riser and stringer. Coverage was cross-checked against the entity
//! classes of ISO 16739-1 (IFC), so every class a building model exports has a home here.
//!
//! Each id is permanent. Top-level ids are multiples of 100 and a subcategory's id is its parent's
//! plus a small number, so the room between them is for later built-in subcategories. A new
//! built-in takes a new id and an old id is never reused or moved; `tests/pinned.rs` fails on any
//! change to an existing one. Ids from [`FIRST_USER_ID`] up belong to project subcategories.

use crate::{CategoryId, CategoryKind};

/// One entry of the built-in tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuiltIn {
    /// Its permanent id.
    pub id: CategoryId,
    /// The name people see. Unique among its siblings, compared without regard to case.
    pub name: &'static str,
    /// What sort of thing it holds.
    pub kind: CategoryKind,
    /// The category it is a subcategory of; `None` for a top-level category.
    pub parent: Option<CategoryId>,
    /// Whether a view can cut elements of it (so they show cut edges, not only projected ones).
    pub cuttable: bool,
}

/// The first id a project subcategory can take. Every built-in id is below it.
pub const FIRST_USER_ID: u32 = 1_000_000;

const fn top(id: CategoryId, name: &'static str, kind: CategoryKind, cuttable: bool) -> BuiltIn {
    BuiltIn {
        id,
        name,
        kind,
        parent: None,
        cuttable,
    }
}

const fn sub(
    id: u32,
    name: &'static str,
    parent: CategoryId,
    kind: CategoryKind,
    cuttable: bool,
) -> BuiltIn {
    BuiltIn {
        id: CategoryId(id),
        name,
        kind,
        parent: Some(parent),
        cuttable,
    }
}

const M: CategoryKind = CategoryKind::Model;
const D: CategoryKind = CategoryKind::Datum;
const A: CategoryKind = CategoryKind::Annotation;
const V: CategoryKind = CategoryKind::View;
const AN: CategoryKind = CategoryKind::Analytical;
const I: CategoryKind = CategoryKind::Import;

macro_rules! top_level_ids {
    ($($(#[$doc:meta])* $name:ident = $id:expr;)*) => {
        $($(#[$doc])* pub const $name: CategoryId = CategoryId($id);)*
    };
}

top_level_ids! {
    /// Walls.
    WALLS = 100;
    /// Curtain walls.
    CURTAIN_WALLS = 200;
    /// Doors.
    DOORS = 300;
    /// Windows.
    WINDOWS = 400;
    /// Floors.
    FLOORS = 500;
    /// Roofs.
    ROOFS = 600;
    /// Ceilings.
    CEILINGS = 700;
    /// Stairs.
    STAIRS = 800;
    /// Ramps.
    RAMPS = 900;
    /// Railings.
    RAILINGS = 1000;
    /// Columns.
    COLUMNS = 1100;
    /// Beams.
    BEAMS = 1200;
    /// Braces.
    BRACES = 1300;
    /// Trusses.
    TRUSSES = 1400;
    /// Foundations.
    FOUNDATIONS = 1500;
    /// Openings cut through a host: shafts and wall, floor and roof openings.
    OPENINGS = 1600;
    /// Furniture.
    FURNITURE = 1700;
    /// Casework: built-in cupboards, counters and joinery.
    CASEWORK = 1800;
    /// Plumbing fixtures.
    PLUMBING_FIXTURES = 1900;
    /// Lighting fixtures.
    LIGHTING_FIXTURES = 2000;
    /// Electrical equipment: boards and panels.
    ELECTRICAL_EQUIPMENT = 2100;
    /// Electrical fixtures: sockets, switches and outlets.
    ELECTRICAL_FIXTURES = 2200;
    /// Mechanical equipment.
    MECHANICAL_EQUIPMENT = 2300;
    /// Ducts.
    DUCTS = 2400;
    /// Pipes.
    PIPES = 2500;
    /// Generic models: anything that fits no other category.
    GENERIC_MODELS = 2600;
    /// Specialty equipment.
    SPECIALTY_EQUIPMENT = 2700;
    /// Planting.
    PLANTING = 2800;
    /// Site components: paving, fences, parking and the like.
    SITE = 2900;
    /// Topography: the ground surface.
    TOPOGRAPHY = 3000;
    /// Property lines.
    PROPERTY_LINES = 3100;
    /// Building pads cut into the ground surface.
    BUILDING_PADS = 3200;
    /// Rooms.
    ROOMS = 3300;
    /// Areas.
    AREAS = 3400;
    /// Masses: volumes for early design.
    MASSES = 3500;
    /// Levels.
    LEVELS = 3600;
    /// Grids.
    GRIDS = 3700;
    /// Reference planes.
    REFERENCE_PLANES = 3800;
    /// Scope boxes.
    SCOPE_BOXES = 3900;
    /// Dimensions.
    DIMENSIONS = 5000;
    /// Tags.
    TAGS = 5100;
    /// Text notes.
    TEXT_NOTES = 5200;
    /// Keynotes.
    KEYNOTES = 5300;
    /// Spot dimensions: elevations, coordinates and slopes.
    SPOT_DIMENSIONS = 5400;
    /// Section markers.
    SECTION_MARKERS = 5500;
    /// Elevation markers.
    ELEVATION_MARKERS = 5600;
    /// Callout markers.
    CALLOUT_MARKERS = 5700;
    /// Detail lines.
    DETAIL_LINES = 5800;
    /// Detail components.
    DETAIL_COMPONENTS = 5900;
    /// Filled regions.
    FILLED_REGIONS = 6000;
    /// Masking regions.
    MASKING_REGIONS = 6100;
    /// Revision clouds.
    REVISION_CLOUDS = 6200;
    /// Symbols: drawing symbols with no model behind them, such as a north arrow.
    SYMBOLS = 6300;
    /// Matchlines.
    MATCHLINES = 6400;
    /// Title blocks.
    TITLE_BLOCKS = 6500;
    /// Legend components.
    LEGEND_COMPONENTS = 6600;
    /// Views: plans, sections, elevations, 3D, drafting views.
    VIEWS = 7000;
    /// Sheets.
    SHEETS = 7100;
    /// Schedules.
    SCHEDULES = 7200;
    /// Legends.
    LEGENDS = 7300;
    /// Viewports: a view placed on a sheet.
    VIEWPORTS = 7400;
    /// Cameras.
    CAMERAS = 7500;
    /// Analytical members: lines in an analysis model of beams, columns and braces.
    ANALYTICAL_MEMBERS = 8000;
    /// Analytical panels: surfaces in an analysis model of floors, walls and slabs.
    ANALYTICAL_PANELS = 8100;
    /// Analytical nodes.
    ANALYTICAL_NODES = 8200;
    /// Energy surfaces: the bounding surfaces of a room or space for energy analysis.
    ENERGY_SURFACES = 8300;
    /// Imported drawings.
    IMPORTED_DRAWINGS = 9000;
    /// Linked models.
    LINKED_MODELS = 9100;
    /// Point clouds.
    POINT_CLOUDS = 9200;
}

/// The whole tree, in id order: each top-level category, then its subcategories.
pub const BUILT_IN: &[BuiltIn] = &[
    top(WALLS, "Walls", M, true),
    sub(101, "Core cut line", WALLS, M, true),
    sub(102, "Finish cut line", WALLS, M, true),
    top(CURTAIN_WALLS, "Curtain walls", M, true),
    sub(201, "Mullions", CURTAIN_WALLS, M, true),
    sub(202, "Panels", CURTAIN_WALLS, M, true),
    top(DOORS, "Doors", M, true),
    sub(301, "Panel", DOORS, M, true),
    sub(302, "Frame", DOORS, M, true),
    sub(303, "Glazing", DOORS, M, true),
    sub(304, "Swing", DOORS, M, false),
    top(WINDOWS, "Windows", M, true),
    sub(401, "Frame", WINDOWS, M, true),
    sub(402, "Sash", WINDOWS, M, true),
    sub(403, "Glazing", WINDOWS, M, true),
    sub(404, "Sill", WINDOWS, M, true),
    sub(405, "Swing", WINDOWS, M, false),
    top(FLOORS, "Floors", M, true),
    sub(501, "Core cut line", FLOORS, M, true),
    sub(502, "Finish cut line", FLOORS, M, true),
    top(ROOFS, "Roofs", M, true),
    sub(601, "Core cut line", ROOFS, M, true),
    sub(602, "Finish cut line", ROOFS, M, true),
    top(CEILINGS, "Ceilings", M, true),
    sub(701, "Finish cut line", CEILINGS, M, true),
    sub(702, "Grid lines", CEILINGS, M, false),
    top(STAIRS, "Stairs", M, true),
    sub(801, "Tread", STAIRS, M, true),
    sub(802, "Riser", STAIRS, M, true),
    sub(803, "Stringer", STAIRS, M, true),
    sub(804, "Landing", STAIRS, M, true),
    sub(805, "Up direction line", STAIRS, M, false),
    top(RAMPS, "Ramps", M, true),
    top(RAILINGS, "Railings", M, true),
    sub(1001, "Top rail", RAILINGS, M, true),
    sub(1002, "Handrail", RAILINGS, M, true),
    sub(1003, "Balusters", RAILINGS, M, true),
    sub(1004, "Infill panel", RAILINGS, M, true),
    top(COLUMNS, "Columns", M, true),
    top(BEAMS, "Beams", M, true),
    top(BRACES, "Braces", M, true),
    top(TRUSSES, "Trusses", M, true),
    top(FOUNDATIONS, "Foundations", M, true),
    top(OPENINGS, "Openings", M, false),
    top(FURNITURE, "Furniture", M, true),
    top(CASEWORK, "Casework", M, true),
    top(PLUMBING_FIXTURES, "Plumbing fixtures", M, true),
    top(LIGHTING_FIXTURES, "Lighting fixtures", M, true),
    top(ELECTRICAL_EQUIPMENT, "Electrical equipment", M, true),
    top(ELECTRICAL_FIXTURES, "Electrical fixtures", M, true),
    top(MECHANICAL_EQUIPMENT, "Mechanical equipment", M, true),
    top(DUCTS, "Ducts", M, true),
    top(PIPES, "Pipes", M, true),
    top(GENERIC_MODELS, "Generic models", M, true),
    top(SPECIALTY_EQUIPMENT, "Specialty equipment", M, true),
    top(PLANTING, "Planting", M, true),
    top(SITE, "Site", M, true),
    top(TOPOGRAPHY, "Topography", M, true),
    top(PROPERTY_LINES, "Property lines", M, false),
    top(BUILDING_PADS, "Building pads", M, true),
    top(ROOMS, "Rooms", M, false),
    sub(3301, "Interior fill", ROOMS, M, false),
    sub(3302, "Boundary lines", ROOMS, M, false),
    top(AREAS, "Areas", M, false),
    top(MASSES, "Masses", M, false),
    top(LEVELS, "Levels", D, false),
    top(GRIDS, "Grids", D, false),
    top(REFERENCE_PLANES, "Reference planes", D, false),
    top(SCOPE_BOXES, "Scope boxes", D, false),
    top(DIMENSIONS, "Dimensions", A, false),
    sub(5001, "Dimension lines", DIMENSIONS, A, false),
    sub(5002, "Witness lines", DIMENSIONS, A, false),
    top(TAGS, "Tags", A, false),
    top(TEXT_NOTES, "Text notes", A, false),
    top(KEYNOTES, "Keynotes", A, false),
    top(SPOT_DIMENSIONS, "Spot dimensions", A, false),
    top(SECTION_MARKERS, "Section markers", A, false),
    top(ELEVATION_MARKERS, "Elevation markers", A, false),
    top(CALLOUT_MARKERS, "Callout markers", A, false),
    top(DETAIL_LINES, "Detail lines", A, false),
    top(DETAIL_COMPONENTS, "Detail components", A, false),
    top(FILLED_REGIONS, "Filled regions", A, false),
    top(MASKING_REGIONS, "Masking regions", A, false),
    top(REVISION_CLOUDS, "Revision clouds", A, false),
    top(SYMBOLS, "Symbols", A, false),
    top(MATCHLINES, "Matchlines", A, false),
    top(TITLE_BLOCKS, "Title blocks", A, false),
    top(LEGEND_COMPONENTS, "Legend components", A, false),
    top(VIEWS, "Views", V, false),
    top(SHEETS, "Sheets", V, false),
    top(SCHEDULES, "Schedules", V, false),
    top(LEGENDS, "Legends", V, false),
    top(VIEWPORTS, "Viewports", V, false),
    top(CAMERAS, "Cameras", V, false),
    top(ANALYTICAL_MEMBERS, "Analytical members", AN, false),
    top(ANALYTICAL_PANELS, "Analytical panels", AN, false),
    top(ANALYTICAL_NODES, "Analytical nodes", AN, false),
    top(ENERGY_SURFACES, "Energy surfaces", AN, false),
    top(IMPORTED_DRAWINGS, "Imported drawings", I, false),
    top(LINKED_MODELS, "Linked models", I, true),
    top(POINT_CLOUDS, "Point clouds", I, false),
];
