//! Our default object styles: one row for every built-in category and subcategory.
//!
//! **How the weights were chosen.** Construction drawings are conventionally weighted by what the
//! line says about the building, from heavy to light:
//!
//! 1. Where a view cuts through the structure and enclosure of the building (walls, floors,
//!    roofs, columns, beams, foundations), the cut edge is the heaviest line on the sheet: pen 6.
//!    The reader's eye is led to the shape of the section first. The ground line in a section is
//!    one step lighter (pen 5), because it is a datum and not a part of the building.
//! 2. Where a view cuts through secondary building elements (doors, windows, curtain walling,
//!    stairs, ramps, trusses, braces) the cut edge is the next step down: pen 5. Their finer
//!    parts (a door's glazing, a window's sash, balusters, infill panels) are lighter again, pens
//!    2 to 4.
//! 3. Fittings and equipment (furniture, casework, fixtures, services) are cut at pen 3 or 4: they
//!    are drawn to be read, never to compete with the structure.
//! 4. The finish skin of a wall, floor, roof or ceiling is cut at pen 3, so the structural core
//!    reads as the heavy line and the finish as a thin one beside it.
//! 5. Edges seen beyond the cut, in projection, are always lighter than the cut: pen 4 for walls,
//!    roofs and structure, pen 3 for floors and secondary elements, and pen 2 for fittings.
//! 6. The lightest pen, 1, draws what is only a surface or a symbol of the model: glazing,
//!    ceiling grids, room fills and boundaries, door swings, datums (levels, grids, reference
//!    planes), planting, filled regions (hatching), masking regions, views and sheets.
//! 7. Annotation uses pens 1 to 4 and is printed at one width whatever the scale (the annotation
//!    pens of [`crate::pens::AnnotationPens`]): text, tags, dimensions and leaders at pen 2,
//!    witness lines at pen 1, markers and detail lines at pen 3, and the title block at pen 4.
//!
//! Pens are numbered by the ladder of [`crate::default`] (ISO 128-20 widths), so pen 6 is 0.7 mm
//! at the largest scales and prints 0.5 mm at 1:50 and 1:100, and the cut wall at 1:100 is
//! therefore a 0.5 mm line, the weight most plan sets use.
//!
//! **Colour and pattern.** Almost everything is black and continuous (the built-in solid
//! pattern), so a print is clean in black and white. The few exceptions follow convention: dash
//! and chain patterns for datums, property lines, areas and matchlines; a dashed line for the
//! hidden or imaginary (a window swing, an opening, a mass, a scope box, an analysis member); a
//! dotted line for energy surfaces; and a muted colour only where a colour carries meaning
//! (ground, planting, analysis, imported or linked material).
//!
//! The values are ours, designed from the conventions above and the public ladder of ISO 128-20.
//! No other product's table was consulted. `tests/object_default.rs` pins them.

use crate::object_styles::{Colour, Entry, ObjectStyle};
use crate::patterns::{DASH_DOT, DASH_DOT_DOT, DASHED, DOTTED, PatternId, SOLID};
use construct_categories::CategoryId;

/// One row of the default table: the category's id, its projection pen, its cut pen (0 for none),
/// its colour and its pattern.
struct Row {
    id: u32,
    projection: u16,
    cut: u16,
    colour: Colour,
    pattern: PatternId,
}

const BLACK: Colour = Colour(0, 0, 0);
const GREY: Colour = Colour(110, 110, 110);
const GROUND: Colour = Colour(120, 90, 60);
const GREEN: Colour = Colour(40, 110, 40);
const BLUE: Colour = Colour(30, 80, 170);
const RED: Colour = Colour(170, 30, 30);

const fn row(id: u32, projection: u16, cut: u16) -> Row {
    Row {
        id,
        projection,
        cut,
        colour: BLACK,
        pattern: SOLID,
    }
}

const fn with(id: u32, projection: u16, cut: u16, colour: Colour, pattern: PatternId) -> Row {
    Row {
        id,
        projection,
        cut,
        colour,
        pattern,
    }
}

/// The rows, in the order of the built-in tree. A test fails if a built-in category has no row,
/// or if a row's cut pen disagrees with the category's "can be cut" flag.
const ROWS: &[Row] = &[
    // Walls: structure cut at pen 6, the finish skin at pen 3.
    row(100, 4, 6),
    row(101, 4, 6),
    row(102, 2, 3),
    // Curtain walls.
    row(200, 3, 5),
    row(201, 3, 5),
    row(202, 2, 3),
    // Doors.
    row(300, 3, 5),
    row(301, 3, 5),
    row(302, 3, 5),
    row(303, 1, 2),
    row(304, 1, 0),
    // Windows.
    row(400, 3, 5),
    row(401, 3, 5),
    row(402, 3, 4),
    row(403, 1, 2),
    row(404, 3, 5),
    with(405, 1, 0, BLACK, DASHED),
    // Floors.
    row(500, 3, 6),
    row(501, 3, 6),
    row(502, 2, 3),
    // Roofs.
    row(600, 4, 6),
    row(601, 4, 6),
    row(602, 2, 3),
    // Ceilings.
    row(700, 2, 4),
    row(701, 1, 3),
    row(702, 1, 0),
    // Stairs.
    row(800, 3, 5),
    row(801, 2, 4),
    row(802, 2, 4),
    row(803, 3, 5),
    row(804, 3, 5),
    row(805, 1, 0),
    // Ramps.
    row(900, 3, 5),
    // Railings.
    row(1000, 2, 4),
    row(1001, 2, 4),
    row(1002, 2, 4),
    row(1003, 1, 3),
    row(1004, 1, 2),
    // Structure.
    row(1100, 4, 6),
    row(1200, 4, 6),
    row(1300, 3, 5),
    row(1400, 3, 5),
    row(1500, 4, 6),
    // Openings cannot be cut: they are drawn as an outline.
    with(1600, 2, 0, BLACK, DASHED),
    // Fittings and equipment.
    row(1700, 2, 4),
    row(1800, 3, 4),
    row(1900, 2, 4),
    row(2000, 2, 3),
    row(2100, 2, 3),
    row(2200, 1, 2),
    row(2300, 2, 4),
    row(2400, 2, 3),
    row(2500, 2, 3),
    row(2600, 2, 4),
    row(2700, 2, 4),
    with(2800, 1, 3, GREEN, SOLID),
    // Site.
    row(2900, 2, 4),
    with(3000, 2, 5, GROUND, SOLID),
    with(3100, 3, 0, RED, DASH_DOT),
    row(3200, 2, 4),
    // Rooms, areas, masses.
    row(3300, 1, 0),
    row(3301, 1, 0),
    with(3302, 1, 0, BLACK, DASHED),
    with(3400, 1, 0, BLACK, DASH_DOT_DOT),
    with(3500, 1, 0, GREY, DASHED),
    // Datums: the lightest line, in a muted colour, as a chain or dashed line.
    with(3600, 1, 0, GREY, DASH_DOT),
    with(3700, 1, 0, GREY, DASH_DOT),
    with(3800, 1, 0, GREY, DASHED),
    with(3900, 1, 0, GREY, DASHED),
    // Annotation.
    row(5000, 2, 0),
    row(5001, 2, 0),
    row(5002, 1, 0),
    row(5100, 1, 0),
    row(5200, 2, 0),
    row(5300, 2, 0),
    row(5400, 2, 0),
    row(5500, 3, 0),
    row(5600, 3, 0),
    row(5700, 3, 0),
    row(5800, 3, 0),
    row(5900, 2, 0),
    row(6000, 1, 0),
    row(6100, 1, 0),
    row(6200, 4, 0),
    row(6300, 2, 0),
    with(6400, 3, 0, BLACK, DASH_DOT_DOT),
    row(6500, 4, 0),
    row(6600, 2, 0),
    // Views.
    row(7000, 1, 0),
    row(7100, 1, 0),
    row(7200, 1, 0),
    row(7300, 1, 0),
    row(7400, 2, 0),
    row(7500, 1, 0),
    // Analysis.
    with(8000, 2, 0, BLUE, DASHED),
    with(8100, 1, 0, BLUE, DASHED),
    with(8200, 2, 0, BLUE, SOLID),
    with(8300, 1, 0, BLUE, DOTTED),
    // Imports: muted, so brought-in material reads as background.
    with(9000, 1, 0, GREY, SOLID),
    with(9100, 2, 3, GREY, SOLID),
    with(9200, 1, 0, GREY, SOLID),
];

/// The default entries, one for every built-in category, in ascending category id.
pub(crate) fn default_entries() -> Vec<Entry> {
    let mut entries: Vec<Entry> = ROWS
        .iter()
        .map(|row| Entry {
            category: CategoryId(row.id),
            style: ObjectStyle {
                projection_pen: row.projection,
                cut_pen: (row.cut != 0).then_some(row.cut),
                colour: row.colour,
                pattern: row.pattern,
            },
        })
        .collect();
    entries.sort_by_key(|entry| entry.category);
    entries
}
