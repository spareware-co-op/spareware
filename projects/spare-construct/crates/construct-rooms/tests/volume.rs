//! A room's volume with the floors in its space taken out, every number worked by hand: a box of 40
//! by 30 with its base at 0 and its upper limit at 27 (grid steps; the geometry does not care what
//! a step is), under a flat floor, beside floors that take nothing, under a part floor with an
//! opening, and under a sloped floor.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_rooms::{Bound, Found, Outline, RoomError, Slab, find, net_volume};
use kit_core::Id;

fn room() -> Found {
    let line = |id: &str, a: [i64; 2], b: [i64; 2]| Bound {
        wall: Id::parse(id).unwrap(),
        outline: Outline::Line([a, b]),
    };
    let lines = [
        line("a", [0, 0], [40, 0]),
        line("b", [40, 0], [40, 30]),
        line("c", [40, 30], [0, 30]),
        line("d", [0, 30], [0, 0]),
    ];
    find(&lines, [20, 15]).unwrap().unwrap()
}

fn rectangle(x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<[i64; 2]> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

fn volume(slabs: &[Slab]) -> i128 {
    net_volume(&room(), 0, 27, slabs).unwrap().cubic_steps()
}

#[test]
fn a_room_with_no_floor_is_its_area_times_its_height() {
    assert_eq!(volume(&[]), 40 * 30 * 27);
}

#[test]
fn the_floor_of_the_level_above_takes_its_thickness_from_the_top() {
    // Its top at the upper limit (27) and 4 thick: its underside at 23, so 4 of each column.
    let above = Slab::flat(vec![rectangle(-5, -5, 45, 35)], 27, 4);
    assert_eq!(volume(&[above]), 40 * 30 * 27 - 40 * 30 * 4);
}

#[test]
fn a_floor_whose_top_is_at_the_base_takes_nothing_and_a_raised_one_what_stands_above_it() {
    let on_the_level = Slab::flat(vec![rectangle(0, 0, 40, 30)], 0, 5);
    assert_eq!(volume(&[on_the_level]), 40 * 30 * 27);
    // Top at 2, 3 thick: from -1 up to 2, of which 2 is above the base.
    let raised = Slab::flat(vec![rectangle(0, 0, 40, 30)], 2, 3);
    assert_eq!(volume(&[raised]), 40 * 30 * 27 - 40 * 30 * 2);
    // Wholly above the upper limit, or wholly below the base: nothing.
    let high = Slab::flat(vec![rectangle(0, 0, 40, 30)], 40, 4);
    let low = Slab::flat(vec![rectangle(0, 0, 40, 30)], -3, 4);
    assert_eq!(volume(&[high, low]), 40 * 30 * 27);
}

#[test]
fn a_floor_over_part_of_the_room_with_an_opening_takes_only_what_is_inside_both() {
    // x from 10 to 60 against the room's 0 to 40: 30 by 30 overlap, less a 5 by 10 opening wholly in it.
    let part = Slab::flat(
        vec![
            rectangle(10, 0, 60, 30),
            // The opening, clockwise as a hole.
            vec![[20, 10], [20, 20], [25, 20], [25, 10]],
        ],
        27,
        4,
    );
    assert_eq!(volume(&[part]), 40 * 30 * 27 - (30 * 30 - 5 * 10) * 4);
}

#[test]
fn two_floors_take_what_each_takes() {
    let a = Slab::flat(vec![rectangle(0, 0, 40, 30)], 27, 4);
    let b = Slab::flat(vec![rectangle(0, 0, 20, 30)], 2, 3);
    assert_eq!(volume(&[a, b]), 40 * 30 * 27 - 40 * 30 * 4 - 20 * 30 * 2);
}

#[test]
fn a_room_under_a_sloped_floor_is_cut_by_its_underside() {
    // The floor above rises 5 over the 40: its top is 27 at x = 0 and 32 at x = 40, through
    // (0,0,27), (40,0,32) and (0,30,27); 5 thick, so its underside is 22 at x = 0 and 27 at x = 40.
    // Each column loses 27 - underside = 5 - x/8, from 5 at x = 0 down to 0 at x = 40: over the
    // 30 depth, 30 x (5 x 40 - 40^2/16) = 30 x 100 = 3000.
    let sloped = Slab::sloped(
        vec![rectangle(-10, -10, 50, 40)],
        [[0, 0, 27], [40, 0, 32], [0, 30, 27]],
        5,
    )
    .unwrap();
    assert_eq!(volume(&[sloped]), 40 * 30 * 27 - 3000);
}

#[test]
fn a_sloped_floor_whose_underside_crosses_the_upper_limit_takes_the_wedge_it_cuts() {
    // A steep floor: its top is 10 at x = 0 and 50 at x = 40 (rise 1 per step along x), 4 thick,
    // so its underside is x + 6. The room reaches up to 27.
    // x in 0..17: the top (x + 10) is under 27, so the whole 4 thick stands in the room;
    // x in 17..21: the underside is under 27, so 27 - (x + 6) = 21 - x of it does;
    // beyond x = 21: nothing. Per unit of depth: 4 x 17 = 68, plus the integral of (21 - x) from
    // 17 to 21, which is 8: 76, and over the 30 depth, 2280.
    let steep = Slab::sloped(
        vec![rectangle(0, 0, 40, 30)],
        [[0, 0, 10], [40, 0, 50], [0, 30, 10]],
        4,
    )
    .unwrap();
    assert_eq!(volume(&[steep]), 40 * 30 * 27 - 2280);
}

#[test]
fn bad_slabs_and_heights_are_refused_by_name() {
    assert_eq!(
        Slab::sloped(vec![], [[0, 0, 0], [1, 1, 1], [2, 2, 2]], 4),
        Err(RoomError::NoPlane)
    );
    assert_eq!(net_volume(&room(), 5, 5, &[]), Err(RoomError::NoHeight));
}
