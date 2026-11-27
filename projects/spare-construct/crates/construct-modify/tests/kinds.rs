//! Each kind of element under each motion, on records made by the crates that own their forms:
//! what moves, what keeps its ID, what a mirror turns over, and the refusals, each naming the
//! element.

// The crate denies these outside tests (Cargo.toml); a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod fixtures;

use construct_grids::{Grid, PlaneShape, Point as GridPoint, ReferencePlane, Shape};
use construct_modify::kinds::{follower, moved};
use construct_modify::{ModifyError, Motion, Turn};
use construct_params::Value;
use fixtures::*;

fn quarter_about(centre: [i64; 2]) -> Motion {
    Motion::turn_about(centre.map(|c| c * MM), Turn::quarters(1)).unwrap()
}

fn shift(dx: i64, dy: i64) -> Motion {
    Motion::by([dx * MM, dy * MM])
}

/// The mirror about the vertical line x = `x` millimetres.
fn mirror_x(x: i64) -> Motion {
    Motion::mirror([x * MM, 0], [x * MM, MM]).unwrap()
}

const NOT_FLIPPED: Fixed = Fixed { flipped: false };

#[test]
fn a_layered_wall_moves_and_turns_its_line_and_keeps_the_rest() {
    let w = wall("W-1", [0, 0], [4000, 0]);
    let moved_by = moved(&w, &shift(1000, 2000), &NOT_FLIPPED).unwrap();
    assert_eq!(point_of(&moved_by, "start"), [1000 * MM, 2000 * MM]);
    assert_eq!(point_of(&moved_by, "end"), [5000 * MM, 2000 * MM]);
    assert_eq!(moved_by.props, w.props);
    assert_eq!(moved_by.recipe.args["height"], w.recipe.args["height"]);
    assert_eq!(moved_by.recipe.args["layers"], w.recipe.args["layers"]);

    let turned = moved(&w, &quarter_about([0, 0]), &NOT_FLIPPED).unwrap();
    assert_eq!(point_of(&turned, "start"), [0, 0]);
    assert_eq!(point_of(&turned, "end"), [0, 4000 * MM]);
    assert!(!turned.props.contains_key("wall.flipped"));
}

/// Walking a wall from its start to its end its exterior is on the right; mirrored, the line runs
/// the mirror's way and the exterior must stay where the mirror puts it, so the wall's side turns
/// over: an unflipped wall is written flipped, and a flipped one unflipped.
#[test]
fn a_mirrored_wall_turns_its_side_over() {
    let w = wall("W-1", [0, 0], [4000, 0]);
    let mirrored = moved(&w, &mirror_x(5000), &NOT_FLIPPED).unwrap();
    assert_eq!(point_of(&mirrored, "start"), [10_000 * MM, 0]);
    assert_eq!(point_of(&mirrored, "end"), [6000 * MM, 0]);
    assert_eq!(mirrored.props["wall.flipped"], Value::YesNo(true).stored());
    let back = moved(&w, &mirror_x(5000), &Fixed { flipped: true }).unwrap();
    assert_eq!(back.props["wall.flipped"], Value::YesNo(false).stored());
}

#[test]
fn a_point_off_the_grid_is_refused_naming_the_element() {
    // A 3:4:5 turn about the origin takes (1 mm, 0) to (0.6 mm, -0.8 mm): 3,840 and -5,120 steps,
    // on the grid. One step is not.
    let turn = Motion::turn_about([0, 0], Turn::between([3, 4], [1, 0]).unwrap()).unwrap();
    let w = wall("W-1", [0, 0], [1, 0]);
    let turned = moved(&w, &turn, &NOT_FLIPPED).unwrap();
    assert_eq!(point_of(&turned, "end"), [3840, -5120]);
    let mut off = w.clone();
    off.recipe.args.insert(
        "end".to_owned(),
        serde_json::json!(["length:q0:1", "length:q0:0"]),
    );
    let refused = moved(&off, &turn, &NOT_FLIPPED).unwrap_err();
    assert_eq!(
        refused,
        ModifyError::OffGrid {
            element: "W-1".to_owned()
        }
    );
    assert_eq!(
        refused.to_string(),
        "W-1 would not land on the length grid: a point of it would lie between two of its steps."
    );
}

#[test]
fn a_floor_keeps_its_edges_ids_through_every_motion() {
    let f = floor("F-1", &[[0, 0], [4000, 0], [4000, 3000], [0, 3000]]);
    let turned = moved(&f, &quarter_about([0, 0]), &NOT_FLIPPED).unwrap();
    assert_eq!(
        floor_corners(&turned),
        vec![
            ("e0".to_owned(), [0, 0]),
            ("e1".to_owned(), [0, 4000 * MM]),
            ("e2".to_owned(), [-3000 * MM, 4000 * MM]),
            ("e3".to_owned(), [-3000 * MM, 0]),
        ]
    );
    // A mirror runs the loop the other way round; construct-slabs takes either way, and each edge
    // keeps its ID.
    let mirrored = moved(&f, &mirror_x(0), &NOT_FLIPPED).unwrap();
    let ids: Vec<String> = floor_corners(&mirrored)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids, ["e0", "e1", "e2", "e3"]);
    assert_eq!(floor_corners(&mirrored)[1].1, [-4000 * MM, 0]);
    assert_eq!(mirrored.props, f.props);
}

#[test]
fn a_roof_keeps_what_each_edge_does() {
    let r = gable("R-1", 8000, 6000);
    let turned = moved(&r, &quarter_about([0, 0]), &NOT_FLIPPED).unwrap();
    let roof = construct_roofs::roof_of(&turned.recipe).unwrap();
    let before = construct_roofs::roof_of(&r.recipe).unwrap();
    assert_eq!(roof.edges(), before.edges());
    assert_eq!(roof.form(), before.form());
    let corners = roof
        .footprint()
        .corners(construct_slabs::Ring::Outer)
        .unwrap();
    assert_eq!([corners[1].1.x, corners[1].1.y], [0, 8000 * MM]);
}

#[test]
fn a_room_moves_its_point() {
    let r = room("R-1", "101", [2000, 1500]);
    let turned = moved(&r, &quarter_about([0, 0]), &NOT_FLIPPED).unwrap();
    assert_eq!(
        construct_rooms::at_of(&turned.recipe).unwrap(),
        [-1500 * MM, 2000 * MM]
    );
    assert_eq!(construct_rooms::number_of(&turned), Some("101"));
    assert_eq!(
        construct_rooms::limit_of(&turned),
        construct_rooms::limit_of(&r)
    );
}

#[test]
fn a_shaft_and_a_floor_opening_move_their_outlines() {
    let s = shaft("S-1", &[[0, 0], [1000, 0], [1000, 500], [0, 500]]);
    let mirrored = moved(&s, &mirror_x(2000), &NOT_FLIPPED).unwrap();
    assert_eq!(
        outline_corners(&mirrored),
        vec![
            [4000 * MM, 0],
            [3000 * MM, 0],
            [3000 * MM, 500 * MM],
            [4000 * MM, 500 * MM]
        ]
    );
    assert_eq!(mirrored.props, s.props);
    let o = floor_opening("O-1", "F-1", &[[100, 100], [600, 100], [600, 400]]);
    let by = moved(&o, &shift(50, -50), &NOT_FLIPPED).unwrap();
    assert_eq!(
        outline_corners(&by),
        vec![
            [150 * MM, 50 * MM],
            [650 * MM, 50 * MM],
            [650 * MM, 350 * MM]
        ]
    );
    assert_eq!(by.references, o.references);
}

#[test]
fn a_grid_line_and_an_arc_turn_and_mirror_exactly() {
    let g = grid_line("G-A", "A", [0, 0], [0, 9000]);
    let turned = moved(&g, &quarter_about([0, 0]), &NOT_FLIPPED).unwrap();
    let shape = *Grid::from_record(&turned).unwrap().shape();
    let p = |x: i64, y: i64| GridPoint::new(mm(x), mm(y));
    assert_eq!(shape, Shape::line(p(0, 0), p(-9000, 0)).unwrap());
    assert_eq!(turned.name, "A");

    let arc = grid_arc("G-1", "1", 10, 80);
    let turned = moved(&arc, &quarter_about([0, 0]), &NOT_FLIPPED).unwrap();
    assert_eq!(
        *Grid::from_record(&turned).unwrap().shape(),
        Shape::arc(p(0, 1000), mm(6000), degrees(100), degrees(170)).unwrap()
    );
    // About the x axis an arc from 10 to 80 degrees runs from 280 to 350, its ends changed over.
    let along_x = Motion::mirror([0, 0], [MM, 0]).unwrap();
    let mirrored = moved(&arc, &along_x, &NOT_FLIPPED).unwrap();
    assert_eq!(
        *Grid::from_record(&mirrored).unwrap().shape(),
        Shape::arc(p(1000, 0), mm(6000), degrees(280), degrees(350)).unwrap()
    );
    // Any other turn would put its angles off the angle units.
    let other = Motion::turn_about([0, 0], Turn::between([3, 4], [1, 0]).unwrap()).unwrap();
    assert_eq!(
        moved(&arc, &other, &NOT_FLIPPED).unwrap_err(),
        ModifyError::ArcTurns {
            element: "1".to_owned()
        }
    );
}

#[test]
fn a_reference_plane_moves_in_plan_and_a_horizontal_one_stays() {
    let p = |x: i64, y: i64| GridPoint::new(mm(x), mm(y));
    let vertical = plane(
        "P-1",
        Some("Mid"),
        PlaneShape::Vertical {
            from: p(0, 0),
            to: p(0, 1000),
        },
    );
    let by = moved(&vertical, &shift(500, 0), &NOT_FLIPPED).unwrap();
    assert_eq!(
        *ReferencePlane::from_record(&by).unwrap().shape(),
        PlaneShape::Vertical {
            from: p(500, 0),
            to: p(500, 1000)
        }
    );
    let level = plane("P-2", None, PlaneShape::Horizontal { elevation: mm(900) });
    assert_eq!(moved(&level, &shift(500, 0), &NOT_FLIPPED).unwrap(), level);
}

#[test]
fn a_block_turns_by_quarters_in_whole_millimetres() {
    let b = block("B-1", 1000, 0, 400, 200);
    let turned = moved(&b, &quarter_about([1000, 0]), &NOT_FLIPPED).unwrap();
    assert_eq!(
        [turned.placement.x, turned.placement.y],
        [800, 0],
        "the box runs from 800 to 1000 along x and 0 to 400 along y"
    );
    assert_eq!(turned.recipe.args["width"], 200);
    assert_eq!(turned.recipe.args["depth"], 400);
    // Half a millimetre is between two of its placement's millimetres.
    let half = Motion::by([MM / 2, 0]);
    assert_eq!(
        moved(&b, &half, &NOT_FLIPPED).unwrap_err(),
        ModifyError::NotWholeMm {
            element: "B-1".to_owned()
        }
    );
    let other = Motion::turn_about([0, 0], Turn::between([3, 4], [1, 0]).unwrap()).unwrap();
    assert_eq!(
        moved(&b, &other, &NOT_FLIPPED).unwrap_err(),
        ModifyError::BoxTurns {
            element: "B-1".to_owned()
        }
    );
}

#[test]
fn an_older_box_wall_moves_and_does_not_turn() {
    let w = old_wall("W-0", 100, 200, 4000);
    let by = moved(&w, &shift(1000, -200), &NOT_FLIPPED).unwrap();
    assert_eq!([by.placement.x, by.placement.y], [1100, 0]);
    assert_eq!(by.recipe, w.recipe);
    let refused = moved(&w, &quarter_about([0, 0]), &NOT_FLIPPED).unwrap_err();
    assert_eq!(
        refused,
        ModifyError::OlderBox {
            element: "W-0".to_owned()
        }
    );
    assert_eq!(
        refused.to_string(),
        "W-0 is an older box, drawn along x or y: it moves, and is drawn again to turn or mirror \
         it."
    );
}

#[test]
fn a_record_of_no_shape_is_not_moved() {
    let refused = moved(&a_type("T-1"), &shift(1, 0), &NOT_FLIPPED).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "T-1 is a type, which the modify tools do not move: change it with its own commands."
    );
}

/// What a wall's turn changes in what it hosts: a box door and a wall opening name their wall's
/// axis, so a quarter turn changes it and a half turn does not; a mirror changes a placed door's
/// hand, and nothing else.
#[test]
fn what_follows_a_wall_changes_only_what_names_its_wall() {
    let d = door("D-1", "W-1", 500);
    let turned = follower(&d, &quarter_about([0, 0]), &NOT_FLIPPED)
        .unwrap()
        .unwrap();
    assert_eq!(turned.recipe.args["axis"], "y");
    let back = follower(&turned, &quarter_about([0, 0]), &NOT_FLIPPED)
        .unwrap()
        .unwrap();
    assert!(!back.recipe.args.contains_key("axis"));
    let half = Motion::turn_about([0, 0], Turn::quarters(2)).unwrap();
    assert_eq!(follower(&d, &half, &NOT_FLIPPED).unwrap(), None);
    assert_eq!(follower(&d, &shift(5, 5), &NOT_FLIPPED).unwrap(), None);

    let o = wall_opening("O-1", "W-1", 1000, false);
    let turned = follower(&o, &quarter_about([0, 0]), &NOT_FLIPPED)
        .unwrap()
        .unwrap();
    let opening = construct_openings::WallOpening::of("O-1", &turned.recipe).unwrap();
    assert!(opening.along_y);

    // The library's single swing door is hinged on the left unless it holds its own hand.
    let f = swing_door("F-1", "W-1", 2000);
    assert_eq!(value_of(&f, "swing"), None);
    let mirrored = follower(&f, &mirror_x(0), &NOT_FLIPPED).unwrap().unwrap();
    assert_eq!(
        value_of(&mirrored, "swing").as_deref(),
        Some("choice:right")
    );
    let again = follower(&mirrored, &mirror_x(0), &NOT_FLIPPED)
        .unwrap()
        .unwrap();
    assert_eq!(value_of(&again, "swing").as_deref(), Some("choice:left"));
    assert_eq!(
        follower(&f, &quarter_about([0, 0]), &NOT_FLIPPED).unwrap(),
        None
    );
    assert_eq!(mirrored.references, f.references);
    assert_eq!(mirrored.props, f.props);
}
