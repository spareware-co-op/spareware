//! A curtain wall as both builders read it (job 14.1): its plan on the kernel's grid, along an
//! axis exactly and at an angle rounded once along and once across; the stand-in's one box with
//! every panel and mullion drawn; its faces' names on both editions; and an embedded curtain wall
//! seated on its host's line and handed to its host as the plain opening it cuts, fitted by the
//! kernel builder's own rules for openings. The kernel's own solids are the kernel suite's
//! (`tests/kernel_suite`).

#![cfg(feature = "server")]
// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code, and the kernel suite it shares; the rules are for the crate's own code"
)]

mod kernel_suite;

use construct_curtain::{CurtainSpec, Edits, embedded_recipe, filled};
use construct_model::element::{Element, Placement};
use construct_model::geometry::{Geometry, Hosted, Recipe};
use construct_params::Length;
use construct_regen::curtain_plan::CurtainPlan;
use construct_regen::hosting;
use construct_regen::stand_in::Boxes;
use kernel_suite::{curtain, curtain_spec, layered_wall};
use kit_core::Id;
use std::collections::BTreeMap;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// An element of `recipe`, at the origin, with no props.
fn element(id: &str, kind: &str, recipe: Recipe) -> Element {
    Element {
        id: i(id),
        document: i("doc"),
        kind: kind.to_owned(),
        name: id.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe,
        props: BTreeMap::new(),
        references: Vec::new(),
    }
}

/// Along x, every corner is the frame's point exactly: the first panel of the 6 m by 3 m wall with
/// 50 mm mullions is from 50 to 1475 mm along and 12 mm either side of the line, and the vertical
/// mullion beside it and the sill below share their corners exactly where they meet.
#[test]
fn a_curtain_wall_along_an_axis_is_its_frame_exactly() {
    let wall = curtain([6000, 0], 3000, &curtain_spec(1500, 1000, true), None);
    let plan = CurtainPlan::of(&wall).unwrap();
    assert_eq!(plan.solids.len(), 43);
    let mm = |v: i64| v * 6400;
    let first = &plan.solids[0];
    assert_eq!(first.function, "panel.glazed");
    assert_eq!(
        first.corners,
        [
            [mm(50), mm(12)],
            [mm(50), mm(-12)],
            [mm(1475), mm(-12)],
            [mm(1475), mm(12)]
        ]
    );
    assert_eq!(first.z, [mm(50), mm(975)]);
    let jamb = plan
        .solids
        .iter()
        .find(|s| s.key == format!("mullion.v.{}.0", mm(1500)))
        .unwrap();
    let sill = plan
        .solids
        .iter()
        .find(|s| s.key == format!("mullion.h.0.{}", mm(1500)))
        .unwrap();
    assert!(jamb.corners.contains(&[mm(1525), mm(75)]));
    assert!(sill.corners.contains(&[mm(1525), mm(75)]));
    assert_eq!(
        plan.outline,
        [
            [mm(0), mm(75)],
            [mm(0), mm(-75)],
            [mm(6000), mm(-75)],
            [mm(6000), mm(75)]
        ]
    );
}

/// At an angle (a line 3 m by 4 m, 5 m long), each corner is the frame's point rounded once along
/// and once across: within one grid step of the true point on each axis, and two solids that
/// share a place along the wall and a face across it share the corner bit for bit. The stand-in
/// draws it at the nearest millimetres and says it rounded.
#[test]
fn a_curtain_wall_at_an_angle_is_rounded_once_along_and_once_across() {
    let wall = curtain([3000, 4000], 3000, &curtain_spec(1500, 1000, true), None);
    let plan = CurtainPlan::of(&wall).unwrap();
    assert_eq!(plan.grid.length, Length::from_mm(5000));
    let true_point = |u: f64, w: f64| -> [f64; 2] {
        let (ux, uy) = (0.6, 0.8);
        [u * ux + w * uy, u * uy - w * ux]
    };
    let mm = |v: i64| (v * 6400) as f64;
    let first = &plan.solids[0];
    // The quadrilateral's corners turn anticlockwise seen from above, from its start's corner on
    // the interior side (the frame's across, toward the exterior, is the line's right).
    let frame_points = [(50, -12), (50, 12), (1475, 12), (1475, -12)];
    for (corner, (u, w)) in first.corners.iter().zip(frame_points) {
        let [x, y] = true_point(mm(u), mm(w));
        assert!((corner[0] as f64 - x).abs() <= 1.0, "{corner:?} {x}");
        assert!((corner[1] as f64 - y).abs() <= 1.0, "{corner:?} {y}");
    }
    let jamb = plan
        .solids
        .iter()
        .find(|s| s.key.starts_with("mullion.v.9600000."))
        .unwrap();
    let sill = plan
        .solids
        .iter()
        .find(|s| s.key == "mullion.h.0.9600000")
        .unwrap();
    let shared = jamb
        .corners
        .iter()
        .filter(|c| sill.corners.contains(c))
        .count();
    assert_eq!(shared, 2, "{:?} {:?}", jamb.corners, sill.corners);
    let built = Boxes.build(&wall).unwrap();
    assert_eq!(built.displays[0].deviation, 1);
    assert_eq!(built.displays[0].parts.len(), 43);
}

/// The stand-in draws every panel and mullion as a part, the glass first; its box is the wall's
/// whole extent; its faces are named as a box's, and on the kernel's edition as a wall's six
/// roles, the same names however its grid changes.
#[test]
fn the_stand_in_draws_every_part_and_both_editions_name_its_faces() {
    let wall = curtain([6000, 0], 3000, &curtain_spec(1500, 1000, true), None);
    let built = Boxes.build(&wall).unwrap();
    let parts = &built.displays[0].parts;
    assert_eq!(
        parts
            .iter()
            .filter(|p| p.function == "panel.glazed")
            .count(),
        12
    );
    assert_eq!(parts.iter().filter(|p| p.function == "mullion").count(), 31);
    assert_eq!(built.bounds.min, [0, -75, 0]);
    assert_eq!(built.bounds.max, [6000, 75, 3000]);
    assert_eq!(built.displays[0].deviation, 0);
    let names = construct_regen::face_names(&Boxes.edition(), &wall).unwrap();
    assert_eq!(names, built.names);
    let kernel = construct_regen::kernel::names(&wall).unwrap();
    let coarser = curtain([6000, 0], 3000, &curtain_spec(2000, 0, false), None);
    assert_eq!(construct_regen::kernel::names(&coarser).unwrap(), kernel);
    let roles: Vec<&str> = kernel.iter().map(|n| n.role.as_str()).collect();
    assert_eq!(roles.len(), 6);
    assert_eq!(Boxes.build(&wall).unwrap().body, built.body);
}

/// A curtain wall that does not lay out is refused by both builders in construct-curtain's words:
/// a type not read in, and mullions that leave a panel no room.
#[test]
fn a_curtain_wall_that_does_not_lay_out_says_why() {
    let unread = curtain([6000, 0], 3000, "{}", None);
    let failed = Boxes.build(&unread).unwrap_err();
    assert_eq!(failed.code, "BadCurtain");
    let tight = curtain([6000, 0], 3000, &curtain_spec(50, 0, true), None);
    let failed = Boxes.build(&tight).unwrap_err();
    assert_eq!(failed.code, "BadCurtain");
    assert!(
        failed.message.contains("no room between its mullions"),
        "{}",
        failed.message
    );
}

/// A curtain wall 4 m long and 2.4 m high embedded 1 m along a 6 m wall 3 m high: its own builder
/// is handed it on the wall's reference line from 1 to 5 m along, at the wall's base; the wall is
/// handed the plain opening it cuts, 4 m by 2.4 m along x, 1 m along. One that reaches past the
/// wall's end, or as high as the wall, is refused by the kernel builder's own rules for openings,
/// naming it.
#[test]
fn an_embedded_curtain_wall_stands_on_its_hosts_line_and_cuts_its_opening() {
    let host_recipe = layered_wall([0, 0], [6000, 0], 3000, &kernel_suite::PARTITION);
    let host = element("W-1", "wall", host_recipe.clone());
    let spec = CurtainSpec::from_text(&curtain_spec(1000, 0, true)).unwrap();
    let embedded = |length: i64, height: i64, along: i64| {
        let recipe = embedded_recipe(
            Length::from_mm(length as i32),
            Length::from_mm(height as i32),
            &Edits::new(),
        )
        .unwrap();
        let mut cw = element("CW-1", "curtain", filled(&recipe, &spec).unwrap());
        let edition = Boxes.edition();
        let face = hosting::side_by_role(&edition, &host_recipe, "exterior").unwrap();
        hosting::write(
            &mut cw,
            &host.id,
            &face,
            Length::from_mm(along as i32),
            Length::from_mm(0),
        );
        cw
    };
    let cw = embedded(4000, 2400, 1000);
    let edition = Boxes.edition();
    let hosting = hosting::read(&cw).unwrap().unwrap();
    let (at, seated) =
        hosting::seat(&edition, &host, &host_recipe, &cw, &cw.recipe, &hosting).unwrap();
    assert_eq!(at, Placement { x: 0, y: 0, z: 0 });
    let plan = CurtainPlan::of(&seated).unwrap();
    assert_eq!(plan.line.start(), [1000 * 6400, 0]);
    assert_eq!(plan.line.end(), [5000 * 6400, 0]);
    let (cut_at, cut) = hosting::cut_of(&edition, &host, &host_recipe, &cw, &cw.recipe, &hosting)
        .unwrap()
        .unwrap();
    let opening = construct_openings::WallOpening::of("CW-1", &cut).unwrap();
    assert_eq!(
        (opening.width, opening.height, opening.along_y),
        (Length::from_mm(4000), Length::from_mm(2400), false)
    );
    assert_eq!(cut_at.x, 1000);
    let fits = |cw: &Element| {
        let hosting = hosting::read(cw).unwrap().unwrap();
        let (at, cut) = hosting::cut_of(&edition, &host, &host_recipe, cw, &cw.recipe, &hosting)
            .unwrap()
            .unwrap();
        construct_regen::kernel::fits(
            &edition,
            &host_recipe,
            &[Hosted {
                key: "CW-1",
                recipe: &cut,
                at,
            }],
        )
    };
    fits(&cw).unwrap();
    let past = fits(&embedded(5500, 2400, 1000)).unwrap_err();
    assert!(past.message.contains("CW-1"), "{}", past.message);
    let tall = fits(&embedded(4000, 3000, 1000)).unwrap_err();
    assert!(tall.message.contains("CW-1"), "{}", tall.message);
    // A curtain wall is no host, and the noun a refusal names it by is its own.
    assert!(!hosting::is_host(&cw.recipe));
    assert_eq!(hosting::noun(&cw.recipe), "curtain wall");
}

/// A page is told the grid it edits: every line (both borders and the interior ones, with where
/// each stands as the exact typed text a command takes and in the document's units, and whether
/// the wall added it) and every panel (the corner its cell starts at, its size and what it holds),
/// worked out from the filled recipe, so the page works out no grid of its own.
#[test]
fn a_page_is_told_the_grid_it_edits() {
    use construct_regen::curtains::CurtainView;
    use serde_json::json;
    let units = construct_params::Units::default();
    let q0 = |mm: i64| format!("length:q0:{}", mm * 6400);
    let (content, _) = construct_regen::families::Library::baked()
        .family("library.doors-windows.single-swing-door")
        .unwrap();
    let grid = json!({
        "added": { "vertical": [q0(750)], "horizontal": [] },
        "removed": { "vertical": [], "horizontal": [] },
        "omitted": [],
        "panels": [{ "at": [q0(750), q0(0)],
            "panel": { "kind": "door", "family": content.as_str(), "type": "d900x2100" } }],
    });
    let wall = curtain([6000, 0], 3000, &curtain_spec(1500, 1000, true), Some(grid));
    let view = CurtainView::of(&wall, &units).unwrap();
    // Vertical: both borders and lines at 750 (added), 1500, 3000 and 4500 mm; horizontal: both
    // borders and lines at 1 m and 2 m.
    assert_eq!(view.lines.len(), 6 + 4);
    let at: Vec<(&str, &str, bool, bool)> = view
        .lines
        .iter()
        .map(|l| (l.direction, l.at.as_str(), l.border, l.added))
        .collect();
    assert_eq!(
        at,
        [
            ("vertical", "0 mm", true, false),
            ("vertical", "750 mm", false, true),
            ("vertical", "1500 mm", false, false),
            ("vertical", "3000 mm", false, false),
            ("vertical", "4500 mm", false, false),
            ("vertical", "6000 mm", true, false),
            ("horizontal", "0 mm", true, false),
            ("horizontal", "1000 mm", false, false),
            ("horizontal", "2000 mm", false, false),
            ("horizontal", "3000 mm", true, false),
        ]
    );
    assert_eq!(view.lines[2].shown, "1500 mm");
    assert_eq!(view.lines[2].segments.len(), 3);
    assert_eq!(view.lines[2].segments[0].from, "0 mm");
    assert!(!view.lines[2].segments[0].omitted);
    // 5 columns by 3 rows.
    assert_eq!(view.panels.len(), 15);
    let door = view
        .panels
        .iter()
        .find(|p| p.kind == "door")
        .expect("the swapped panel");
    assert_eq!(door.at, ["750 mm", "0 mm"]);
    assert!(door.swapped);
    assert_eq!(door.door.as_ref().unwrap()[0], content.as_str());
    assert_eq!(
        view.panels.iter().filter(|p| p.kind == "glazed").count(),
        14
    );
    assert_eq!(view.unused, Vec::<String>::new());
    // What is not a curtain wall has no grid.
    assert!(
        CurtainView::of(
            &layered_wall([0, 0], [1000, 0], 2700, &kernel_suite::PARTITION),
            &units
        )
        .is_none()
    );
}
