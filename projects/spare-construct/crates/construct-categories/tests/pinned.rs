//! The built-in ids are permanent: a stored model names a category by id and must keep meaning the
//! same thing for ever. This file lists every built-in as `id name parent` (`-` for none). A new
//! built-in is a new line at the right place with a new id. Editing or deleting an existing line
//! means a stored model changes meaning: do not.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use construct_categories::BUILT_IN;

const PINNED: &[&str] = &[
    "100 Walls -",
    "101 Core cut line 100",
    "102 Finish cut line 100",
    "200 Curtain walls -",
    "201 Mullions 200",
    "202 Panels 200",
    "300 Doors -",
    "301 Panel 300",
    "302 Frame 300",
    "303 Glazing 300",
    "304 Swing 300",
    "400 Windows -",
    "401 Frame 400",
    "402 Sash 400",
    "403 Glazing 400",
    "404 Sill 400",
    "405 Swing 400",
    "500 Floors -",
    "501 Core cut line 500",
    "502 Finish cut line 500",
    "600 Roofs -",
    "601 Core cut line 600",
    "602 Finish cut line 600",
    "700 Ceilings -",
    "701 Finish cut line 700",
    "702 Grid lines 700",
    "800 Stairs -",
    "801 Tread 800",
    "802 Riser 800",
    "803 Stringer 800",
    "804 Landing 800",
    "805 Up direction line 800",
    "900 Ramps -",
    "1000 Railings -",
    "1001 Top rail 1000",
    "1002 Handrail 1000",
    "1003 Balusters 1000",
    "1004 Infill panel 1000",
    "1100 Columns -",
    "1200 Beams -",
    "1300 Braces -",
    "1400 Trusses -",
    "1500 Foundations -",
    "1600 Openings -",
    "1700 Furniture -",
    "1800 Casework -",
    "1900 Plumbing fixtures -",
    "2000 Lighting fixtures -",
    "2100 Electrical equipment -",
    "2200 Electrical fixtures -",
    "2300 Mechanical equipment -",
    "2400 Ducts -",
    "2500 Pipes -",
    "2600 Generic models -",
    "2700 Specialty equipment -",
    "2800 Planting -",
    "2900 Site -",
    "3000 Topography -",
    "3100 Property lines -",
    "3200 Building pads -",
    "3300 Rooms -",
    "3301 Interior fill 3300",
    "3302 Boundary lines 3300",
    "3400 Areas -",
    "3500 Masses -",
    "3600 Levels -",
    "3700 Grids -",
    "3800 Reference planes -",
    "3900 Scope boxes -",
    "5000 Dimensions -",
    "5001 Dimension lines 5000",
    "5002 Witness lines 5000",
    "5100 Tags -",
    "5200 Text notes -",
    "5300 Keynotes -",
    "5400 Spot dimensions -",
    "5500 Section markers -",
    "5600 Elevation markers -",
    "5700 Callout markers -",
    "5800 Detail lines -",
    "5900 Detail components -",
    "6000 Filled regions -",
    "6100 Masking regions -",
    "6200 Revision clouds -",
    "6300 Symbols -",
    "6400 Matchlines -",
    "6500 Title blocks -",
    "6600 Legend components -",
    "7000 Views -",
    "7100 Sheets -",
    "7200 Schedules -",
    "7300 Legends -",
    "7400 Viewports -",
    "7500 Cameras -",
    "8000 Analytical members -",
    "8100 Analytical panels -",
    "8200 Analytical nodes -",
    "8300 Energy surfaces -",
    "9000 Imported drawings -",
    "9100 Linked models -",
    "9200 Point clouds -",
];

/// Every built-in id, name and parent is exactly the pinned list, in order, and nothing is added
/// without being pinned here.
#[test]
fn built_in_ids_are_pinned() {
    let now: Vec<String> = BUILT_IN
        .iter()
        .map(|c| {
            let parent = c
                .parent
                .map_or_else(|| "-".to_string(), |p| p.0.to_string());
            format!("{} {} {}", c.id.0, c.name, parent)
        })
        .collect();
    let pinned: Vec<String> = PINNED.iter().map(ToString::to_string).collect();
    assert_eq!(now, pinned);
}
