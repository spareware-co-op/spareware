//! The generators reach every hard case of chapter 28.5 from one rule byte, and stay in range on
//! any bytes (G116).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "tests: native arithmetic, and a failure stops the test"
)]

use geom_fuzz::adversarial::{
    MILLIMETRE, PITCHES, base, beside, coordinate, length, nudge, point, repeats,
};
use geom_fuzz::robustness::{DESIGNS, RECORDED, render, tally};
use geom_num::P3;
use geom_num::grid::COORD_MAX;
use geom_test::ByteSource;

/// The coordinate of rule `rule` and word `word`.
fn value(rule: u8, word: u64) -> i64 {
    let mut bytes = vec![rule];
    bytes.extend_from_slice(&word.to_le_bytes());
    coordinate(&mut ByteSource::new(&bytes))
}

fn len(rule: u8, word: u64) -> u64 {
    let mut bytes = vec![rule];
    bytes.extend_from_slice(&word.to_le_bytes());
    length(&mut ByteSource::new(&bytes)).get()
}

#[test]
fn a_millimetre_is_6400_steps_and_the_pitches_are_what_people_draw_on() {
    assert_eq!(MILLIMETRE, 6_400);
    assert_eq!(PITCHES, [6_400, 16_000, 64_000]);
}

#[test]
fn every_rule_of_a_coordinate_lands_where_it_says() {
    assert!(value(0, 1 << 20).abs() < 64);
    for (rule, pitch) in [(1, PITCHES[0]), (13, PITCHES[1]), (19, PITCHES[2])] {
        let v = value(rule, 7);
        assert_eq!(v, 7 * pitch);
        assert_eq!(v.rem_euclid(pitch), 0);
    }
    assert_eq!(value(2, 7).rem_euclid(PITCHES[0]), 1);
    assert!(value(3, 5) <= COORD_MAX && value(3, 5) >= COORD_MAX - 8);
    assert!(value(4, 5) >= -COORD_MAX && value(4, 5) <= -COORD_MAX + 8);
    assert_eq!(value(5, 1 << 39), 1 << 39);
}

#[test]
fn every_rule_of_a_length_lands_where_it_says() {
    assert_eq!(len(0, 123), 1);
    assert_eq!(len(1, 20), 4);
    assert_eq!(len(2, 3), 3 * 6_400);
    assert_eq!(len(3, 1_000_001), 1);
    assert!(len(4, u64::MAX) <= 1 << 41);
    assert_eq!(len(5, u64::MAX), u64::MAX);
}

#[test]
fn any_bytes_give_a_point_in_range() {
    for fill in [0u8, 3, 4, 0x7f, 0xff] {
        let bytes = [fill; 27];
        let got = point(&mut ByteSource::new(&bytes));
        for c in [got.x.get(), got.y.get(), got.z.get()] {
            assert!(c.abs() <= COORD_MAX, "{fill}: {c}");
        }
    }
}

#[test]
fn a_base_of_three_or_six_corners_is_convex_by_construction() {
    let mut bytes = vec![3u8];
    bytes.extend_from_slice(&[0u8; 3 * 27]);
    let corners = base(&mut ByteSource::new(&bytes));
    assert_eq!(corners.len(), 3);
    let (a, b, c) = (corners[0], corners[1], corners[2]);
    let d = |p: P3, q: P3| (q.x.get() - p.x.get(), q.y.get() - p.y.get());
    let (u, v) = (d(a, b), d(a, c));
    assert!(u.0 * v.1 - u.1 * v.0 > 0);
    // Other counts keep what was written, flattened to one height.
    let mut bytes = vec![4u8];
    bytes.extend_from_slice(&[1u8; 4 * 27]);
    let corners = base(&mut ByteSource::new(&bytes));
    assert_eq!(corners.len(), 4);
    assert!(corners.iter().all(|c| c.z == corners[0].z));
}

#[test]
fn a_nudge_moves_each_axis_by_at_most_one_step() {
    assert_eq!(nudge(&mut ByteSource::new(&[0b00_11_00])), [-1, 1, -1]);
    assert_eq!(nudge(&mut ByteSource::new(&[0b01_10_01])), [0, 0, 0]);
    let at = P3::new(5, 6, 7).unwrap();
    assert_eq!(beside(at, [0, 0, 0]), at);
    assert_eq!(beside(at, [1, -1, 1]), P3::new(6, 5, 8).unwrap());
}

#[test]
fn a_nudge_past_the_grid_stays_where_it_is() {
    let edge = P3::new(COORD_MAX, 0, 0).unwrap();
    assert_eq!(beside(edge, [1, 1, 1]), edge);
}

#[test]
fn repeats_stand_in_a_row() {
    let wall = repeats(300, 16_000);
    assert_eq!(wall.len(), 300);
    assert_eq!(wall[0], [16_000, 0, 0]);
    assert_eq!(wall[299], [4_800_000, 0, 0]);
}

/// The counts of `src/robustness.txt`: a count that falls fails here, and one that rises is
/// recorded by replacing the file with what this prints.
#[test]
fn robustness_never_falls() {
    let counts = tally();
    let now = render(&counts);
    println!("{now}");
    assert_eq!(now, RECORDED, "the seeded designs' successes moved");
    let (made, designs) = counts["make"];
    assert_eq!(designs, DESIGNS);
    assert!(made * 10 >= designs * 9, "{made} of {designs}");
}
