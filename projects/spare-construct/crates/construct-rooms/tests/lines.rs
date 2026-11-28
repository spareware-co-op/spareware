//! Room separation lines and area boundary lines in the search, every number worked by hand: an
//! open plan split by a line into two rooms, the line moved, deleted, crossing nothing, with a loose
//! end, tying a column to a wall, a ring of lines inside a room, and a line at an angle whose
//! corners are rounded once; and areas, found from lines alone, with the gross area of an L.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_params::Length;
use construct_rooms::{
    Bound, Found, Neighbour, Outcome, Outline, RoomError, assess, assess_flat, find, measure_flat,
    nearest_edge,
};
use kit_core::Id;

const MM: i64 = 6400;
/// Square steps in a square millimetre.
const SQ: i128 = (MM as i128) * (MM as i128);

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(at: [i64; 2]) -> [i64; 2] {
    at.map(|v| v * MM)
}

fn rect(id: &str, low: [i64; 2], high: [i64; 2]) -> Bound {
    let [x0, y0] = mm(low);
    let [x1, y1] = mm(high);
    Bound {
        wall: i(id),
        outline: Outline::Polygon(vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]),
    }
}

/// A 200 mm wall along an axis, its centre line from `from` to `to` in millimetres.
fn wall(id: &str, from: [i64; 2], to: [i64; 2]) -> Bound {
    let low = [from[0].min(to[0]), from[1].min(to[1])];
    let high = [from[0].max(to[0]), from[1].max(to[1])];
    if from[1] == to[1] {
        rect(id, [low[0], low[1] - 100], [high[0], high[1] + 100])
    } else {
        rect(id, [low[0] - 100, low[1]], [high[0] + 100, high[1]])
    }
}

/// A line from `from` to `to`, in millimetres.
fn line(id: &str, from: [i64; 2], to: [i64; 2]) -> Bound {
    Bound {
        wall: i(id),
        outline: Outline::Line([mm(from), mm(to)]),
    }
}

/// Four walls drawn centre line to centre line round 4000 by 3000 mm: the open space inside is
/// 3800 by 2800 mm, from (100, 100) to (3900, 2900).
fn box_of_walls() -> Vec<Bound> {
    vec![
        wall("w-s", [0, 0], [4000, 0]),
        wall("w-e", [4000, 0], [4000, 3000]),
        wall("w-n", [4000, 3000], [0, 3000]),
        wall("w-w", [0, 3000], [0, 0]),
    ]
}

fn found(bounds: &[Bound], at: [i64; 2]) -> Found {
    find(bounds, mm(at)).unwrap().unwrap()
}

fn area_mm2(found: &Found) -> i128 {
    let flat = measure_flat(found).unwrap();
    assert_eq!(
        flat.area.square_steps() % SQ,
        0,
        "a whole square millimetre"
    );
    flat.area.square_steps() / SQ
}

#[test]
fn an_open_plan_is_split_into_two_rooms_by_a_line() {
    let mut bounds = box_of_walls();
    // The one room, before the line: 3800 by 2800 mm.
    assert_eq!(area_mm2(&found(&bounds, [700, 1500])), 10_640_000);
    // A separation line at x = 1500, drawn from the middle of the south wall to the north's.
    bounds.push(line("l-1", [1500, 0], [1500, 3000]));
    let kitchen = found(&bounds, [700, 1500]);
    let living = found(&bounds, [3000, 1500]);
    // 100 to 1500 is 1400 mm and 1500 to 3900 is 2400 mm, each 2800 mm deep.
    assert_eq!(area_mm2(&kitchen), 3_920_000);
    assert_eq!(area_mm2(&living), 6_720_000);
    assert_eq!(area_mm2(&kitchen) + area_mm2(&living), 10_640_000);
    // Each is bounded by the line; the rooms are four-cornered and cut the line at the walls' faces.
    assert!(kitchen.walls.contains(&i("l-1")) && living.walls.contains(&i("l-1")));
    assert_eq!(kitchen.outer.len(), 4);
    assert_eq!(living.outer.len(), 4);
    assert_eq!(kitchen.outer[0].at, mm([100, 100]));
    assert_eq!(living.outer[0].at, mm([1500, 100]));
    // Two rooms in two loops are neither redundant: both are enclosed.
    let others = [
        Neighbour {
            id: i("r-a"),
            at: mm([700, 1500]),
        },
        Neighbour {
            id: i("r-b"),
            at: mm([3000, 1500]),
        },
    ];
    let height = Length::from_mm(2700);
    for (id, at, area) in [
        ("r-a", [700, 1500], 3_920_000),
        ("r-b", [3000, 1500], 6_720_000),
    ] {
        let mine: Vec<Neighbour> = others.iter().filter(|o| o.id != i(id)).cloned().collect();
        match assess(&i(id), mm(at), height, &bounds, &mine).unwrap() {
            Outcome::Enclosed { measures, .. } => {
                assert_eq!(measures.area.square_steps(), area * SQ);
                assert_eq!(
                    measures.volume.cubic_steps(),
                    area * SQ * 2700 * i128::from(MM)
                );
            }
            other => panic!("{id}: {other:?}"),
        }
    }
}

#[test]
fn moving_a_line_finds_the_rooms_again_and_deleting_it_joins_them() {
    let mut bounds = box_of_walls();
    bounds.push(line("l-1", [2500, 0], [2500, 3000]));
    // Moved from 1500 to 2500: 100 to 2500 is 2400 mm, 2500 to 3900 is 1400 mm.
    assert_eq!(area_mm2(&found(&bounds, [700, 1500])), 6_720_000);
    assert_eq!(area_mm2(&found(&bounds, [3000, 1500])), 3_920_000);
    // Deleted: one loop, and the later room (by ID) is redundant beside the first.
    bounds.pop();
    assert_eq!(area_mm2(&found(&bounds, [3000, 1500])), 10_640_000);
    let first = Neighbour {
        id: i("r-a"),
        at: mm([700, 1500]),
    };
    let second = assess(
        &i("r-b"),
        mm([3000, 1500]),
        Length::from_mm(2700),
        &bounds,
        &[first],
    )
    .unwrap();
    assert_eq!(second, Outcome::Redundant { first: i("r-a") });
}

#[test]
fn a_line_that_bounds_nothing_changes_nothing_and_is_not_refused() {
    let mut bounds = box_of_walls();
    // Far outside the walls, crossing nothing.
    bounds.push(line("l-far", [9000, 0], [9000, 3000]));
    // Along the south wall's inside face and inside the north wall: the walls stand there.
    bounds.push(line("l-face", [100, 100], [3900, 100]));
    bounds.push(line("l-in", [0, 3000], [4000, 3000]));
    let room = found(&bounds, [700, 1500]);
    assert_eq!(area_mm2(&room), 10_640_000);
    assert_eq!(room.outer.len(), 4);
    assert!(room.walls.iter().all(|w| w.as_str().starts_with("w-")));
}

#[test]
fn a_line_with_a_loose_end_is_no_part_of_the_boundary() {
    let mut bounds = box_of_walls();
    // From inside the north wall to a loose end in the room.
    bounds.push(line("l-spur", [1500, 3000], [1500, 1000]));
    let room = found(&bounds, [700, 1500]);
    assert_eq!(area_mm2(&room), 10_640_000);
    assert_eq!(room.outer.len(), 4);
    assert!(!room.walls.contains(&i("l-spur")));
    let flat = measure_flat(&room).unwrap();
    assert_eq!(flat.perimeter.steps(), (3800 + 2800 + 3800 + 2800) * MM);
}

#[test]
fn a_line_tying_a_column_to_a_wall_leaves_the_column_a_hole() {
    let mut bounds = box_of_walls();
    bounds.push(rect("c-1", [1000, 1000], [1500, 1500]));
    bounds.push(line("l-tie", [1500, 1250], [4000, 1250]));
    let room = found(&bounds, [700, 500]);
    // 3800 by 2800 less the 500 by 500 column.
    assert_eq!(area_mm2(&room), 10_640_000 - 250_000);
    assert_eq!(room.outer.len(), 4);
    assert_eq!(room.holes.len(), 1);
    assert_eq!(room.holes[0].len(), 4);
    assert!(!room.walls.contains(&i("l-tie")));
}

#[test]
fn a_ring_of_lines_makes_a_room_and_a_hole_in_the_room_round_it() {
    let mut bounds = box_of_walls();
    bounds.push(line("l-a", [1000, 1000], [2000, 1000]));
    bounds.push(line("l-b", [2000, 1000], [2000, 2000]));
    bounds.push(line("l-c", [2000, 2000], [1000, 2000]));
    bounds.push(line("l-d", [1000, 2000], [1000, 1000]));
    let inner = found(&bounds, [1500, 1500]);
    assert_eq!(area_mm2(&inner), 1_000_000);
    assert_eq!(inner.holes.len(), 0);
    let outer = found(&bounds, [500, 500]);
    assert_eq!(area_mm2(&outer), 10_640_000 - 1_000_000);
    assert_eq!(outer.holes.len(), 1);
}

#[test]
fn a_point_on_a_line_is_refused_naming_the_line() {
    let mut bounds = box_of_walls();
    bounds.push(line("l-1", [1500, 0], [1500, 3000]));
    assert_eq!(
        find(&bounds, mm([1500, 1500])),
        Err(RoomError::PointOnLine { line: i("l-1") })
    );
    // A line of no length is refused too.
    let zero = [line("l-0", [10, 10], [10, 10])];
    assert_eq!(
        find(&zero, mm([0, 0])),
        Err(RoomError::NoOutline { wall: i("l-0") })
    );
}

#[test]
fn a_line_at_an_angle_has_its_corners_rounded_once_and_both_rooms_agree() {
    let mut bounds = box_of_walls();
    // The diagonal of the walls' centres, y = 3x/4. It meets the inside faces at x = 400/3 mm on
    // the south face and x = 11600/3 mm on the north one, off the grid.
    bounds.push(line("l-d", [0, 0], [4000, 3000]));
    let below = found(&bounds, [3000, 500]);
    let above = found(&bounds, [700, 2000]);
    assert!(below.rounded && above.rounded);
    // The diagonal splits the 3800 by 2800 room into two equal trapezoids of 5,320,000 mm2 each,
    // the corners moved by at most half a step each way.
    let half = 5_320_000_i128 * SQ;
    for room in [&below, &above] {
        let area = measure_flat(room).unwrap().area.square_steps();
        assert!((area - half).abs() < 20_000_000, "{area} against {half}");
    }
    // The meeting points are the same two grid points in both rooms.
    let shared: Vec<[i64; 2]> = below
        .outer
        .iter()
        .map(|c| c.at)
        .filter(|at| above.outer.iter().any(|c| c.at == *at))
        .collect();
    assert_eq!(shared.len(), 2);
    // 400/3 mm is 853,333.33 steps: rounded to 853,333; 11600/3 mm is 24,746,666.67: to 24,746,667.
    assert!(shared.contains(&[853_333, 100 * MM]));
    assert!(shared.contains(&[24_746_667, 2900 * MM]));
}

#[test]
fn an_l_shaped_gross_area_is_found_from_area_lines_alone() {
    // The L: 10000 by 4000 mm with a 4000 by 4000 mm arm, six lines drawn round it.
    let corners = [
        [0, 0],
        [10_000, 0],
        [10_000, 4000],
        [4000, 4000],
        [4000, 8000],
        [0, 8000],
    ];
    let lines: Vec<Bound> = (0..corners.len())
        .map(|k| {
            line(
                &format!("a-{k}"),
                corners[k],
                corners[(k + 1) % corners.len()],
            )
        })
        .collect();
    let area = found(&lines, [2000, 2000]);
    assert_eq!(area.outer.len(), 6);
    // 10000 x 4000 + 4000 x 4000 = 56 square metres.
    assert_eq!(area_mm2(&area), 56_000_000);
    // Its perimeter: 10000 + 4000 + 6000 + 4000 + 4000 + 8000 = 36 metres.
    assert_eq!(measure_flat(&area).unwrap().perimeter.steps(), 36_000 * MM);
    match assess_flat(&i("ar-1"), mm([2000, 2000]), &lines, &[]).unwrap() {
        Outcome::Enclosed { measures, .. } => {
            assert_eq!(measures.area.square_steps(), 56_000_000 * SQ);
        }
        other => panic!("{other:?}"),
    }
    // A second area in the same loop is redundant beside the first by ID.
    let second = assess_flat(
        &i("ar-2"),
        mm([8000, 2000]),
        &lines,
        &[Neighbour {
            id: i("ar-1"),
            at: mm([2000, 2000]),
        }],
    )
    .unwrap();
    assert_eq!(second, Outcome::Redundant { first: i("ar-1") });
    // A gap of one step in the loop leaves it not enclosed.
    let mut open = lines.clone();
    open[5] = Bound {
        wall: i("a-5"),
        outline: Outline::Line([mm([0, 8000]), [0, 1]]),
    };
    assert_eq!(
        assess_flat(&i("ar-1"), mm([2000, 2000]), &open, &[]).unwrap(),
        Outcome::NotEnclosed
    );
}

#[test]
fn picking_a_wall_takes_the_face_nearest_the_click() {
    // A 200 mm wall from (0, 0) to (4000, 0) centre line, as its outline of outer faces.
    let outline = [
        mm([0, -100]),
        mm([4000, -100]),
        mm([4000, 100]),
        mm([0, 100]),
    ];
    // A click on the room's side (north, y above 100) takes the north face, from east to west as
    // the outline runs; one on the other side takes the south face.
    assert_eq!(
        nearest_edge(&outline, mm([2000, 600])),
        Some([mm([4000, 100]), mm([0, 100])])
    );
    assert_eq!(
        nearest_edge(&outline, mm([2000, -600])),
        Some([mm([0, -100]), mm([4000, -100])])
    );
    // A click past the end takes the end face; a point on the first of two equal faces, the first.
    assert_eq!(
        nearest_edge(&outline, mm([5000, 0])),
        Some([mm([4000, -100]), mm([4000, 100])])
    );
    assert_eq!(nearest_edge(&[mm([0, 0])], mm([1, 1])), None);
}
