//! A room's loop found from the walls around its point, exactly: a closed box of walls drawn to
//! their centre lines, walls that only touch at a corner (and a hair's gap that opens the loop),
//! T-meetings (two rooms, with a partition run into the walls or stopped at their faces), a wall
//! ending inside the room (a nib), an island wall and a column (holes), two rooms in one loop,
//! an L's corner that points into the room (where joined walls' faces meet, along the axes and
//! turned), and the refusals: a point in a wall, a curved wall, a corner off the grid, an area
//! that is not whole square steps. Every number is pinned exactly.
//!
//! A doorway does not break a loop because a wall's outline is read whole, openings and all: the
//! search is given no opening. construct-regen's tests put a door in a wall of the loop and find
//! the same room.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_params::{Area, Length};
use construct_rooms::{
    Bound, Corner, Neighbour, Outcome, Outline, RoomError, assess, find, measure,
};
use kit_core::Id;

const MM: i64 = 6400;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// A wall as the outline of its outer faces: the rectangle from `low` to `high`, in millimetres.
fn rect(id: &str, low: [i64; 2], high: [i64; 2]) -> Bound {
    let [x0, y0] = low.map(|v| v * MM);
    let [x1, y1] = high.map(|v| v * MM);
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

/// Four walls drawn centre line to centre line round a 4000 by 3000 mm box.
fn box_of_walls() -> Vec<Bound> {
    vec![
        wall("w-s", [0, 0], [4000, 0]),
        wall("w-e", [4000, 0], [4000, 3000]),
        wall("w-n", [4000, 3000], [0, 3000]),
        wall("w-w", [0, 3000], [0, 0]),
    ]
}

fn at(x: i64, y: i64) -> [i64; 2] {
    [x * MM, y * MM]
}

fn corners(found: &[Corner]) -> Vec<([i64; 2], &str)> {
    found
        .iter()
        .map(|c| ([c.at[0] / MM, c.at[1] / MM], c.wall.as_str()))
        .collect()
}

fn ids(walls: &[&str]) -> Vec<Id> {
    walls.iter().map(|w| i(w)).collect()
}

fn height() -> Length {
    Length::from_mm(2700)
}

#[test]
fn a_box_of_walls_bounds_a_room_by_their_inner_faces() {
    let found = find(&box_of_walls(), at(1000, 1000)).unwrap().unwrap();
    assert_eq!(
        corners(&found.outer),
        [
            ([100, 100], "w-s"),
            ([3900, 100], "w-e"),
            ([3900, 2900], "w-n"),
            ([100, 2900], "w-w"),
        ]
    );
    assert_eq!(found.holes.len(), 0);
    assert_eq!(
        found.walls.iter().cloned().collect::<Vec<_>>(),
        ids(&["w-e", "w-n", "w-s", "w-w"])
    );
    let m = measure(&found, height()).unwrap();
    // 3800 by 2800 mm.
    assert_eq!(m.area, Area::from_mm2(10_640_000));
    assert_eq!(m.area.square_steps(), 435_814_400_000_000);
    assert_eq!(m.perimeter, Length::from_mm(13_200));
    assert_eq!(
        m.volume,
        Area::from_mm2(10_640_000)
            .checked_mul(Length::from_mm(2700))
            .unwrap()
    );
    assert_eq!(m.volume.cubic_steps(), 435_814_400_000_000 * 2700 * 6400);
    // The same walls in another order give the same loop.
    let mut turned = box_of_walls();
    turned.reverse();
    assert_eq!(find(&turned, at(1000, 1000)).unwrap().unwrap(), found);
    // Outside the box there is no room.
    assert_eq!(find(&box_of_walls(), at(5000, 1000)).unwrap(), None);
}

/// Walls whose outlines meet at one point only close the loop there: their faces meet and leave
/// no gap. Moved one grid step apart, they do not.
#[test]
fn walls_that_only_touch_at_a_corner_close_the_loop() {
    let touching = vec![
        rect("w-s", [0, -200], [4000, 0]),
        rect("w-e", [4000, 0], [4200, 3000]),
        rect("w-n", [0, 3000], [4000, 3200]),
        rect("w-w", [-200, 0], [0, 3000]),
    ];
    let found = find(&touching, at(2000, 1500)).unwrap().unwrap();
    assert_eq!(
        corners(&found.outer),
        [
            ([0, 0], "w-s"),
            ([4000, 0], "w-e"),
            ([4000, 3000], "w-n"),
            ([0, 3000], "w-w"),
        ]
    );
    let m = measure(&found, height()).unwrap();
    assert_eq!(m.area, Area::from_mm2(12_000_000));
    assert_eq!(m.perimeter, Length::from_mm(14_000));

    // The east wall one grid step to the east: a gap of 156.25 nm at both its corners.
    let mut apart = touching;
    apart[1] = Bound {
        wall: i("w-e"),
        outline: Outline::Polygon(vec![
            [4000 * MM + 1, 0],
            [4200 * MM + 1, 0],
            [4200 * MM + 1, 3000 * MM],
            [4000 * MM + 1, 3000 * MM],
        ]),
    };
    assert_eq!(find(&apart, at(2000, 1500)).unwrap(), None);
    assert_eq!(
        assess(&i("r-1"), at(2000, 1500), height(), &apart, &[]).unwrap(),
        Outcome::NotEnclosed
    );
}

/// A partition meeting the box's walls in two T's makes two rooms; it is the same whether it is
/// drawn into the walls' centre lines or stopped at their faces.
#[test]
fn t_meetings_split_a_box_into_two_rooms() {
    let mut walls = vec![
        wall("w-s", [0, 0], [6000, 0]),
        wall("w-e", [6000, 0], [6000, 3000]),
        wall("w-n", [6000, 3000], [0, 3000]),
        wall("w-w", [0, 3000], [0, 0]),
    ];
    for partition in [
        wall("w-p", [3000, 0], [3000, 3000]),
        wall("w-p", [3000, 100], [3000, 2900]),
    ] {
        walls.truncate(4);
        walls.push(partition);
        let left = find(&walls, at(1000, 1000)).unwrap().unwrap();
        assert_eq!(
            corners(&left.outer),
            [
                ([100, 100], "w-s"),
                ([2900, 100], "w-p"),
                ([2900, 2900], "w-n"),
                ([100, 2900], "w-w"),
            ]
        );
        let right = find(&walls, at(5000, 1000)).unwrap().unwrap();
        assert_eq!(
            corners(&right.outer),
            [
                ([3100, 100], "w-s"),
                ([5900, 100], "w-e"),
                ([5900, 2900], "w-n"),
                ([3100, 2900], "w-p"),
            ]
        );
        for found in [&left, &right] {
            let m = measure(found, height()).unwrap();
            assert_eq!(m.area, Area::from_mm2(7_840_000));
            assert_eq!(m.perimeter, Length::from_mm(11_200));
        }
        assert_eq!(
            right.walls.iter().cloned().collect::<Vec<_>>(),
            ids(&["w-e", "w-n", "w-p", "w-s"])
        );
        // A room in the right half is not redundant beside one in the left.
        let left_room = Neighbour {
            id: i("r-1"),
            at: at(1000, 1000),
        };
        let outcome = assess(&i("r-2"), at(5000, 1000), height(), &walls, &[left_room]).unwrap();
        assert_eq!(outcome.word(), "enclosed");
    }
}

/// A wall that ends inside the room (a nib) is walked round: the room loses the nib's footprint.
#[test]
fn a_nib_is_walked_round() {
    let mut walls = box_of_walls();
    walls.push(wall("w-nib", [2000, 0], [2000, 1000]));
    let found = find(&walls, at(1000, 2000)).unwrap().unwrap();
    assert_eq!(
        corners(&found.outer),
        [
            ([100, 100], "w-s"),
            ([1900, 100], "w-nib"),
            ([1900, 1000], "w-nib"),
            ([2100, 1000], "w-nib"),
            ([2100, 100], "w-s"),
            ([3900, 100], "w-e"),
            ([3900, 2900], "w-n"),
            ([100, 2900], "w-w"),
        ]
    );
    let m = measure(&found, height()).unwrap();
    // 3800 by 2800 mm less the nib's 200 by 900 mm inside it.
    assert_eq!(m.area, Area::from_mm2(10_640_000 - 180_000));
    assert_eq!(m.perimeter, Length::from_mm(13_200 + 1800));
    assert!(found.walls.contains(&i("w-nib")));
}

/// An island wall and a column inside the room are holes in its boundary, each clockwise.
#[test]
fn an_island_wall_and_a_column_are_holes() {
    let mut walls = box_of_walls();
    walls.push(wall("w-island", [1000, 1500], [3000, 1500]));
    walls.push(rect("c-1", [500, 500], [800, 800]));
    let found = find(&walls, at(2000, 500)).unwrap().unwrap();
    assert_eq!(found.holes.len(), 2);
    assert_eq!(
        corners(&found.holes[0]),
        [
            ([500, 500], "c-1"),
            ([500, 800], "c-1"),
            ([800, 800], "c-1"),
            ([800, 500], "c-1"),
        ]
    );
    assert_eq!(
        corners(&found.holes[1]),
        [
            ([1000, 1400], "w-island"),
            ([1000, 1600], "w-island"),
            ([3000, 1600], "w-island"),
            ([3000, 1400], "w-island"),
        ]
    );
    let m = measure(&found, height()).unwrap();
    assert_eq!(m.area, Area::from_mm2(10_640_000 - 400_000 - 90_000));
    assert_eq!(m.perimeter, Length::from_mm(13_200 + 4400 + 1200));
    assert!(found.holds(at(2000, 2000)));
    assert!(!found.holds(at(2000, 1500)), "in the island wall");
    assert!(!found.holds(at(600, 600)), "in the column");
    // A point in the island wall is refused, naming it.
    assert_eq!(
        find(&walls, at(2000, 1500)).unwrap_err(),
        RoomError::PointInWall {
            wall: i("w-island")
        }
    );
}

/// Two rooms in one loop: the one whose ID comes first keeps it; the other is redundant, as a
/// value, and keeps no area.
#[test]
fn a_second_room_in_one_loop_is_redundant() {
    let walls = box_of_walls();
    let first = Neighbour {
        id: i("r-1"),
        at: at(1000, 1000),
    };
    let second = Neighbour {
        id: i("r-2"),
        at: at(3000, 2000),
    };
    let kept = assess(
        &first.id,
        first.at,
        height(),
        &walls,
        std::slice::from_ref(&second),
    )
    .unwrap();
    let Outcome::Enclosed { measures, .. } = kept else {
        panic!("{kept:?}")
    };
    assert_eq!(measures.area, Area::from_mm2(10_640_000));
    let redundant = assess(&second.id, second.at, height(), &walls, &[first]).unwrap();
    assert_eq!(redundant, Outcome::Redundant { first: i("r-1") });
    assert_eq!(redundant.word(), "redundant");
}

/// A parallelogram wall along `x + y = n` steps, on its side away from the origin.
fn slant(id: &str, n: i64) -> Bound {
    Bound {
        wall: i(id),
        outline: Outline::Polygon(vec![
            [n + 10, -10],
            [n + 30, 10],
            [n - 180, 220],
            [n - 200, 200],
        ]),
    }
}

/// The two walls along the axes, in grid steps: the floor's face `y = 0`, the side's `x = 0`.
fn axes() -> Vec<Bound> {
    vec![
        Bound {
            wall: i("w-x"),
            outline: Outline::Polygon(vec![[-50, -30], [250, -30], [250, 0], [-50, 0]]),
        },
        Bound {
            wall: i("w-y"),
            outline: Outline::Polygon(vec![[-30, -50], [0, -50], [0, 250], [-30, 250]]),
        },
    ]
}

/// A slanted wall: the corners where it meets the axes' walls are grid points, its edge is
/// measured to the nearest step, and the area is exact.
#[test]
fn a_slanted_wall_bounds_a_triangle_exactly() {
    let mut walls = axes();
    walls.push(slant("w-slant", 100));
    let found = find(&walls, [10, 10]).unwrap().unwrap();
    let raw: Vec<([i64; 2], &str)> = found
        .outer
        .iter()
        .map(|c| (c.at, c.wall.as_str()))
        .collect();
    assert_eq!(
        raw,
        [([0, 0], "w-x"), ([100, 0], "w-slant"), ([0, 100], "w-y")]
    );
    let m = measure(&found, Length::from_steps(7)).unwrap();
    assert_eq!(m.area, Area::from_square_steps(5000));
    // 100 + 100 + 141.42..., the slanted edge to the nearest step.
    assert_eq!(m.perimeter, Length::from_steps(341));
    assert_eq!(m.volume.cubic_steps(), 35_000);
}

#[test]
fn what_cannot_be_held_exactly_is_refused_by_name() {
    // A triangle whose twice-area is odd: 101 by 101 steps.
    let mut walls = axes();
    walls.push(slant("w-slant", 101));
    let found = find(&walls, [10, 10]).unwrap().unwrap();
    assert_eq!(
        measure(&found, Length::from_steps(7)).unwrap_err(),
        RoomError::AreaNotExact
    );

    // Two slanted faces crossing at (50.5, 50.5) steps: off the grid.
    let a = Bound {
        wall: i("w-a"),
        outline: Outline::Polygon(vec![[-10, -10], [200, 200], [190, 210], [-20, 0]]),
    };
    let walls = vec![axes()[0].clone(), a, slant("w-b", 101)];
    assert_eq!(
        find(&walls, [50, 10]).unwrap_err(),
        RoomError::OffGrid {
            walls: ids(&["w-a", "w-b"])
        }
    );
}

#[test]
fn a_curved_wall_a_point_in_a_wall_and_no_height_are_refused() {
    let mut walls = box_of_walls();
    walls.push(Bound {
        wall: i("w-arc"),
        outline: Outline::Curved,
    });
    let refused = find(&walls, at(1000, 1000)).unwrap_err();
    assert_eq!(refused, RoomError::CurvedWall { wall: i("w-arc") });
    assert_eq!(refused.code(), "CurvedWall");
    assert!(refused.to_string().contains("w-arc"), "{refused}");
    assert_eq!(
        find(&box_of_walls(), at(0, 1000)).unwrap_err(),
        RoomError::PointInWall { wall: i("w-w") }
    );
    let found = find(&box_of_walls(), at(1000, 1000)).unwrap().unwrap();
    assert_eq!(
        measure(&found, Length::from_steps(0)).unwrap_err(),
        RoomError::NoHeight
    );
    let flat = vec![Bound {
        wall: i("w-flat"),
        outline: Outline::Polygon(vec![[0, 0], [10, 0], [20, 0]]),
    }];
    assert_eq!(
        find(&flat, [5, 5]).unwrap_err(),
        RoomError::NoOutline { wall: i("w-flat") }
    );
}

/// A polygon of millimetre corners in steps, each turned by `turn`.
fn turned(id: &str, corners: &[[i64; 2]], turn: fn([i64; 2]) -> [i64; 2]) -> Bound {
    Bound {
        wall: i(id),
        outline: Outline::Polygon(corners.iter().map(|&[x, y]| turn(at(x, y))).collect()),
    }
}

/// The plan as it is.
fn as_drawn(p: [i64; 2]) -> [i64; 2] {
    p
}

/// The plan turned by the angle whose cosine is 4/5 and sine 3/5 (36.87 degrees) about the
/// origin: exact on the grid for a point whose steps are multiples of 5, as every whole
/// millimetre is (6400 steps). It keeps every length and area.
fn three_four_five(p: [i64; 2]) -> [i64; 2] {
    let [x, y] = p;
    assert_eq!((x % 5, y % 5), (0, 0));
    [(4 * x - 3 * y) / 5, (3 * x + 4 * y) / 5]
}

/// The six walls of an L, 250 mm thick on their centre lines from (0,0) to (8000,0), (8000,3000),
/// (4000,3000), (4000,6000), (0,6000) and back, as the outlines the search is handed, in
/// millimetres. `inward` is how the two walls at (4000,3000), the corner that points into the
/// room, are shaped there: `Plain`, each its own rectangle to its line's end, unjoined; `Butt`,
/// the second running through the corner's 250 mm square and the first stopping at it; `Mitre`,
/// the square split on its diagonal, so the two outlines touch at the room's corner only. The
/// other corners are butt joints, the wall drawn second running through.
fn l_walls(inward: Inward, turn: fn([i64; 2]) -> [i64; 2]) -> Vec<Bound> {
    let rect = |id: &str, [x0, y0]: [i64; 2], [x1, y1]: [i64; 2]| {
        turned(id, &[[x0, y0], [x1, y0], [x1, y1], [x0, y1]], turn)
    };
    let (w3, w4) = match inward {
        Inward::Plain => (
            rect("w-3", [4000, 2875], [8125, 3125]),
            rect("w-4", [3875, 3000], [4125, 5875]),
        ),
        Inward::Butt => (
            rect("w-3", [4125, 2875], [8125, 3125]),
            rect("w-4", [3875, 2875], [4125, 5875]),
        ),
        Inward::Mitre => (
            turned(
                "w-3",
                &[[3875, 2875], [8125, 2875], [8125, 3125], [4125, 3125]],
                turn,
            ),
            turned(
                "w-4",
                &[[3875, 2875], [4125, 3125], [4125, 5875], [3875, 5875]],
                turn,
            ),
        ),
    };
    vec![
        rect("w-1", [-125, -125], [7875, 125]),
        rect("w-2", [7875, -125], [8125, 2875]),
        w3,
        w4,
        rect("w-5", [125, 5875], [4125, 6125]),
        rect("w-6", [-125, 125], [125, 6125]),
    ]
}

#[derive(Clone, Copy)]
enum Inward {
    Plain,
    Butt,
    Mitre,
}

/// Issue 1329, worked by hand. The L of [`l_walls`] with a room at (2000,2000): each inner face
/// is 125 mm in from its line, so the room's outline is (125,125) (7875,125) (7875,2875)
/// (3875,2875) (3875,5875) (125,5875), and at the corner that points into the room the two inner
/// faces meet at the one point (3875,2875).
/// - Area: 7750 x 2750 + 3750 x 3000 = 32,562,500 mm2 (the centre lines' 36,000,000, less the
///   perimeter 28,000 x 125, plus 5 outward corners and less 1 inward corner of 125 x 125).
/// - Perimeter: 7750 + 2750 + 4000 + 3000 + 3750 + 5750 = 27,000 mm.
///
/// Whether the walls meet there in a butt joint or a mitre (their outlines touching at the room's
/// corner only), the room is that outline, exactly. Handed the walls' plain rectangles instead,
/// unjoined at that corner, the search finds what they leave: a 125 by 125 mm notch,
/// (4000,2875) (4000,3000) (3875,3000), and 32,578,125 mm2, one 15,625 mm2 square more. The
/// search is right in each case; a room's faces are the walls' as joined only if the outlines it
/// is handed are the joined ones. All the same turned by 36.87 degrees: nothing here leans on the
/// plan's axes.
#[test]
fn an_inward_corner_is_where_the_joined_walls_faces_meet_at_any_angle() {
    let joined = [
        [125, 125],
        [7875, 125],
        [7875, 2875],
        [3875, 2875],
        [3875, 5875],
        [125, 5875],
    ];
    let notched = [
        [125, 125],
        [7875, 125],
        [7875, 2875],
        [4000, 2875],
        [4000, 3000],
        [3875, 3000],
        [3875, 5875],
        [125, 5875],
    ];
    for turn in [as_drawn as fn([i64; 2]) -> [i64; 2], three_four_five] {
        for (inward, outline, area) in [
            (Inward::Butt, &joined[..], 32_562_500),
            (Inward::Mitre, &joined[..], 32_562_500),
            (Inward::Plain, &notched[..], 32_578_125),
        ] {
            let walls = l_walls(inward, turn);
            let found = find(&walls, turn(at(2000, 2000))).unwrap().unwrap();
            let mut want: Vec<[i64; 2]> = outline.iter().map(|&[x, y]| turn(at(x, y))).collect();
            // The loop starts at its least corner, by x then y.
            let least = (0..want.len()).min_by_key(|&k| want[k]).unwrap();
            want.rotate_left(least);
            let got: Vec<[i64; 2]> = found.outer.iter().map(|c| c.at).collect();
            assert_eq!(got, want);
            assert_eq!(found.walls.len(), 6);
            let m = measure(&found, Length::from_mm(3000)).unwrap();
            assert_eq!(m.area, Area::from_mm2(area));
            assert_eq!(m.perimeter, Length::from_mm(27_000));
            assert_eq!(
                m.volume,
                Area::from_mm2(area)
                    .checked_mul(Length::from_mm(3000))
                    .unwrap()
            );
        }
    }
}
