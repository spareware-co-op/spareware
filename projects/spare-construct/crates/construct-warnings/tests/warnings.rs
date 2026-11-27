//! The warnings of a model: each kind appearing, its identity staying the same through an edit
//! that does not touch it, and leaving when its cause is fixed; the order of the list; a model
//! with no problems; and the exact test of two walls, pinned on the cases that decide it (end to
//! end, side by side, crossing, joining, on another level, at an angle).

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_warnings::{
    Model, RULES, Room, RoomState, Severity, Wall, WallError, Warning, overlap_along_length,
    warnings,
};
use kit_core::Id;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// A wall along x or y as the rectangle from `low` to `high`, on `level`.
fn rect(id: &str, level: &str, low: [i64; 2], high: [i64; 2]) -> Wall {
    let direction = if high[0] - low[0] >= high[1] - low[1] {
        [1, 0]
    } else {
        [0, 1]
    };
    Wall::new(
        i(id),
        i(level),
        direction,
        &[low, [high[0], low[1]], high, [low[0], high[1]]],
    )
    .unwrap()
}

/// A wall at a slope of 3 in 8, a parallelogram with sides (16, 12) and (-6, 8), moved by `by`.
fn slanted(id: &str, by: [i64; 2]) -> Wall {
    let at = |x: i64, y: i64| [x + by[0], y + by[1]];
    Wall::new(
        i(id),
        i("L1"),
        [16, 12],
        &[at(0, 0), at(16, 12), at(10, 20), at(-6, 8)],
    )
    .unwrap()
}

fn room(id: &str, state: RoomState) -> Room {
    Room {
        id: i(id),
        who: format!("room {id}"),
        state,
    }
}

fn model(walls: Vec<Wall>, rooms: Vec<Room>) -> Model {
    Model { walls, rooms }
}

fn identities(model: &Model) -> Vec<String> {
    warnings(model).iter().map(Warning::identity).collect()
}

#[test]
fn a_model_with_no_problems_gives_an_empty_list() {
    assert_eq!(warnings(&Model::default()), []);
    let sound = model(
        vec![
            rect("W-1", "L1", [0, 0], [100, 10]),
            rect("W-2", "L1", [0, 90], [100, 100]),
        ],
        vec![
            room("R-1", RoomState::Enclosed),
            room("R-2", RoomState::Redundant(i("R-1"))),
            room("R-3", RoomState::Refused("a point in a wall".to_owned())),
        ],
    );
    assert_eq!(warnings(&sound), []);
}

#[test]
fn a_room_that_is_not_enclosed_is_a_warning_naming_it() {
    let m = model(Vec::new(), vec![room("R-7", RoomState::NotEnclosed)]);
    let list = warnings(&m);
    assert_eq!(list.len(), 1);
    let w = &list[0];
    assert_eq!(w.kind, "room.not_enclosed");
    assert_eq!(w.severity, Severity::Warning);
    assert_eq!(w.elements, [i("R-7")]);
    assert_eq!(
        w.words,
        "Room R-7 is not enclosed: the walls round its point do not close"
    );
    assert_eq!(w.identity(), "room.not_enclosed|R-7");
}

#[test]
fn a_warning_keeps_its_identity_through_an_unrelated_edit_and_leaves_when_fixed() {
    let open = model(
        vec![rect("W-1", "L1", [0, 0], [100, 10])],
        vec![
            room("R-1", RoomState::NotEnclosed),
            room("R-2", RoomState::Enclosed),
        ],
    );
    let before = identities(&open);
    assert_eq!(before, ["room.not_enclosed|R-1"]);
    // An edit that does not touch it: another wall, another room, the wall moved.
    let edited = model(
        vec![
            rect("W-1", "L1", [0, 5], [100, 15]),
            rect("W-9", "L1", [0, 500], [100, 510]),
        ],
        vec![
            room("R-1", RoomState::NotEnclosed),
            room("R-2", RoomState::Enclosed),
            room("R-3", RoomState::Enclosed),
        ],
    );
    assert_eq!(identities(&edited), before);
    // The cause fixed: the room closes, the warning is gone.
    let fixed = model(edited.walls.clone(), vec![room("R-1", RoomState::Enclosed)]);
    assert_eq!(warnings(&fixed), []);
}

#[test]
fn overlapping_walls_are_one_warning_naming_both_lower_id_first() {
    let m = model(
        vec![
            rect("W-2", "L1", [50, 0], [150, 10]),
            rect("W-1", "L1", [0, 0], [100, 10]),
        ],
        Vec::new(),
    );
    let list = warnings(&m);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].kind, "wall.overlap");
    assert_eq!(list[0].elements, [i("W-1"), i("W-2")]);
    assert_eq!(
        list[0].words,
        "Walls W-1 and W-2 overlap along their length"
    );
}

#[test]
fn an_overlap_keeps_its_identity_and_leaves_when_a_wall_is_moved_off() {
    let walls = |second: [i64; 2]| {
        vec![
            rect("W-1", "L1", [0, 0], [100, 10]),
            rect("W-2", "L1", second, [second[0] + 100, second[1] + 10]),
            rect("W-3", "L1", [0, 400], [100, 410]),
        ]
    };
    let overlapping = identities(&model(walls([60, 5]), Vec::new()));
    assert_eq!(overlapping, ["wall.overlap|W-1|W-2"]);
    // Unrelated edit: W-3 moves, W-2 slides along but still overlaps.
    let edited = model(
        vec![
            rect("W-1", "L1", [0, 0], [100, 10]),
            rect("W-2", "L1", [90, 0], [190, 10]),
            rect("W-3", "L1", [0, 300], [100, 310]),
        ],
        Vec::new(),
    );
    assert_eq!(identities(&edited), overlapping);
    // Fixed: W-2 moved clear.
    assert_eq!(warnings(&model(walls([100, 0]), Vec::new())), []);
}

#[test]
fn what_counts_as_an_overlap_and_what_does_not() {
    let base = rect("W-1", "L1", [0, 0], [100, 10]);
    let over = |other: &Wall| overlap_along_length(&base, other);
    // Counts: any shared length with any shared width; a thin wall inside a thick one; the same
    // place twice.
    assert!(over(&rect("W-2", "L1", [60, 5], [160, 15])));
    assert!(over(&rect("W-2", "L1", [20, 0], [40, 10])));
    assert!(over(&rect("W-2", "L1", [20, 4], [40, 6])));
    assert!(over(&rect("W-2", "L1", [0, 0], [100, 10])));
    assert!(over(&rect("W-2", "L1", [99, 9], [200, 19])));
    // Does not: end to end, a gap, side by side with the faces touching, side by side apart,
    // a corner only.
    assert!(!over(&rect("W-2", "L1", [100, 0], [200, 10])));
    assert!(!over(&rect("W-2", "L1", [101, 0], [200, 10])));
    assert!(!over(&rect("W-2", "L1", [0, 10], [100, 20])));
    assert!(!over(&rect("W-2", "L1", [0, 11], [100, 21])));
    assert!(!over(&rect("W-2", "L1", [100, 10], [200, 20])));
    // Does not: a join (a wall ending on this one, a wall across it, an L corner), which are not
    // parallel.
    assert!(!over(&rect("W-2", "L1", [45, 10], [55, 100])));
    assert!(!over(&rect("W-2", "L1", [45, -50], [55, 50])));
    assert!(!over(&rect("W-2", "L1", [0, 0], [10, 100])));
    // Does not: on another level.
    assert!(!over(&rect("W-2", "L2", [20, 0], [40, 10])));
}

#[test]
fn walls_at_an_angle_are_judged_exactly_too() {
    let base = slanted("W-1", [0, 0]);
    let over = |by: [i64; 2]| overlap_along_length(&base, &slanted("W-2", by));
    // Along its own line: end to end touches, a little less overlaps.
    assert!(!over([16, 12]));
    assert!(over([12, 9]));
    // Across it: sharing a face touches (the side is (-6, 8)), half a width overlaps, a step more
    // than the width is clear.
    assert!(!over([-6, 8]));
    assert!(over([-3, 4]));
    assert!(!over([-7, 9]));
    // The same wall, and a wall that is not parallel at all.
    assert!(over([0, 0]));
    let across = Wall::new(
        i("W-3"),
        i("L1"),
        [-12, 16],
        &[[0, 0], [-12, 16], [-18, 12], [-6, -4]],
    )
    .unwrap();
    assert!(!overlap_along_length(&base, &across));
}

#[test]
fn the_list_is_ordered_by_kind_then_elements_whatever_order_it_was_given() {
    let walls = vec![
        rect("W-5", "L1", [0, 0], [100, 10]),
        rect("W-3", "L1", [0, 0], [100, 10]),
        rect("W-4", "L1", [0, 0], [100, 10]),
        rect("W-8", "L1", [0, 300], [100, 310]),
        rect("W-7", "L1", [50, 300], [150, 310]),
    ];
    let rooms = vec![
        room("R-2", RoomState::NotEnclosed),
        room("R-1", RoomState::NotEnclosed),
    ];
    let expected = [
        "room.not_enclosed|R-1",
        "room.not_enclosed|R-2",
        "wall.overlap|W-3|W-4",
        "wall.overlap|W-3|W-5",
        "wall.overlap|W-4|W-5",
        "wall.overlap|W-7|W-8",
    ];
    assert_eq!(identities(&model(walls.clone(), rooms.clone())), expected);
    let (mut walls, mut rooms) = (walls, rooms);
    walls.reverse();
    rooms.reverse();
    assert_eq!(identities(&model(walls, rooms)), expected);
}

#[test]
fn every_kind_is_one_entry_with_its_own_name() {
    let names: Vec<&str> = RULES.iter().map(|r| r.kind).collect();
    assert_eq!(names, ["room.not_enclosed", "wall.overlap"]);
    assert_eq!(Severity::Warning.word(), "warning");
    assert!(Severity::Notice < Severity::Warning && Severity::Warning < Severity::Error);
}

#[test]
fn an_outline_that_is_not_a_convex_polygon_on_the_grid_is_refused() {
    let l = i("L1");
    let sq = [[0, 0], [10, 0], [10, 10], [0, 10]];
    assert_eq!(
        Wall::new(i("W-1"), l.clone(), [1, 0], &sq[..2]).unwrap_err(),
        WallError::Corners
    );
    assert_eq!(
        Wall::new(i("W-1"), l.clone(), [1, 0], &[[0, 0], [5, 0], [10, 0]]).unwrap_err(),
        WallError::NoArea
    );
    assert_eq!(
        Wall::new(i("W-1"), l.clone(), [0, 0], &sq).unwrap_err(),
        WallError::NoDirection
    );
    assert_eq!(
        Wall::new(
            i("W-1"),
            l.clone(),
            [1, 0],
            &[[0, 0], [2_000_000_000_000, 0], [0, 5]]
        )
        .unwrap_err(),
        WallError::TooBig
    );
    let dent = [[0, 0], [10, 0], [5, 5], [10, 10], [0, 10]];
    assert_eq!(
        Wall::new(i("W-1"), l.clone(), [1, 0], &dent).unwrap_err(),
        WallError::NotConvex
    );
    // Either way round is taken.
    let cw = [[0, 0], [0, 10], [10, 10], [10, 0]];
    let wall = Wall::new(i("W-1"), l, [1, 0], &cw).unwrap();
    assert_eq!(wall.outline(), [[10, 0], [10, 10], [0, 10], [0, 0]]);
}
