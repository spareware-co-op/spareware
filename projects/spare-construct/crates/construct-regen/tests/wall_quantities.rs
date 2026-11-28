//! What a layered wall reads back: its length, one-face area, volume and each layer's volume, with
//! the openings taken out and the joints counted, every number worked by hand in the comments and
//! exact in grid steps (construct-regen's `wall_measure`).

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_model::geometry::Recipe;
use construct_openings::wall_layer;
use construct_params::{Area, Length, Value, Volume};
use construct_regen::inserts::Insert;
use construct_regen::joins;
use construct_regen::wall_measure::{AREA, LENGTH, MeasureError, VOLUME, quantities, read_only};
use construct_regen::wall_plan::{Joints, Layered, OP, Strip};
use kit_core::Id;
use serde_json::json;

fn q0(mm: i64) -> String {
    Value::Length(Length::checked_from_mm(mm).unwrap()).stored()
}

fn steps(mm: i64) -> i64 {
    Length::checked_from_mm(mm).unwrap().steps()
}

/// A layered wall's filled recipe from `start` to `end` (mm), `height` mm tall, its layers as
/// (function, exterior offset, interior offset) in mm from the line, positive to the exterior.
fn wall(start: [i64; 2], end: [i64; 2], height: i64, layers: &[(&str, i64, i64)]) -> Recipe {
    let layers: Vec<_> = layers
        .iter()
        .map(|(f, a, b)| json!({ "function": f, "material": "m", "exterior": q0(*a), "interior": q0(*b) }))
        .collect();
    Recipe::construct(
        OP,
        serde_json::from_value(json!({
            "start": [q0(start[0]), q0(start[1])],
            "end": [q0(end[0]), q0(end[1])],
            "height": q0(height),
            "layers": layers,
        }))
        .unwrap(),
    )
}

/// An opening on a wall along `axis`, from `low` to `high` mm in the model's plan, its foot and
/// head up the wall, in mm.
fn insert(who: &str, axis: usize, low: i64, high: i64, foot: i64, head: i64) -> Insert {
    Insert {
        id: Id::parse(who).unwrap(),
        who: format!("window {who}"),
        host: Id::parse("W-1").unwrap(),
        level: None,
        axis,
        low: steps(low),
        high: steps(high),
        foot: steps(foot),
        head: steps(head),
    }
}

fn mm2(v: i64) -> Area {
    Area::from_square_steps(i128::from(v) * 6400 * 6400)
}

fn mm3(v: i64) -> Volume {
    Volume::from_cubic_steps(i128::from(v) * 6400 * 6400 * 6400)
}

#[test]
fn the_issues_wall_with_a_door_and_a_window_cut_out() {
    // A 3000 mm high wall of 250 mm, layers 110, 40, 0, 90 and 10 mm, 8000 mm long on its line,
    // placed at (10000, 5000) mm in the model, with a 900 x 2100 mm door (1000 to 1900 mm along
    // the wall, on the floor) and a 1200 x 1200 mm window (4000 to 5200 mm along, sill 900 mm).
    // Offsets from the line, from the exterior: +125, +15, -25, -25, -115, -125 mm.
    let layers = [
        ("finish", 125, 15),
        ("thermal", 15, -25),
        ("membrane", -25, -25),
        ("structure", -25, -115),
        ("finish", -115, -125),
    ];
    let recipe = wall([0, 0], [8000, 0], 3000, &layers);
    let layered = Layered::of(&recipe).unwrap();
    let origin = [steps(10_000), steps(5_000)];
    let inserts = [
        insert("D-1", 0, 11_000, 11_900, 0, 2100),
        insert("WIN-1", 0, 14_000, 15_200, 900, 2100),
    ];
    let q = quantities(&layered, origin, &inserts).unwrap();
    // Length: 8000 mm.
    assert_eq!(q.length, Length::checked_from_mm(8000).unwrap());
    // Face area: 8000 x 3000 - 900 x 2100 - 1200 x 1200 = 24,000,000 - 1,890,000 - 1,440,000
    // = 20,670,000 mm2.
    assert_eq!(q.area, mm2(20_670_000));
    // Volume: 20,670,000 x 250 = 5,167,500,000 mm3; per layer area x thickness: 110 mm
    // 2,273,700,000; 40 mm 826,800,000; 0 mm none; 90 mm 1,860,300,000; 10 mm 206,700,000.
    assert_eq!(q.volume, mm3(5_167_500_000));
    let each: Vec<Volume> = q.layers.iter().map(|l| l.volume).collect();
    assert_eq!(
        each,
        [
            mm3(2_273_700_000),
            mm3(826_800_000),
            mm3(0),
            mm3(1_860_300_000),
            mm3(206_700_000)
        ]
    );
    // Each layer is what construct-openings' rule for a layer of a plain wall gives: the face
    // less the openings, times the thickness.
    let (door, window) = (
        (Length::from_mm(900), Length::from_mm(2100)),
        (Length::from_mm(1200), Length::from_mm(1200)),
    );
    for (layer, mm) in q.layers.iter().zip([110, 40, 0, 90, 10]) {
        let rule = wall_layer(
            q.length,
            Length::from_mm(3000),
            Length::from_mm(mm),
            &[door, window],
        )
        .unwrap();
        assert_eq!(layer.volume, rule.volume);
        assert_eq!(q.area, rule.face);
    }
    // The same wall with no openings: 8000 x 3000 = 24,000,000 mm2; x 250 = 6,000,000,000 mm3.
    let bare = quantities(&layered, origin, &[]).unwrap();
    assert_eq!(bare.area, mm2(24_000_000));
    assert_eq!(bare.volume, mm3(6_000_000_000));
}

#[test]
fn the_quantities_are_read_only_parameters_with_fixed_ids_and_exact_values() {
    let recipe = wall([0, 0], [8000, 0], 3000, &[("structure", 125, -125)]);
    let layered = Layered::of(&recipe).unwrap();
    let q = quantities(&layered, [0, 0], &[]).unwrap();
    let rows = read_only(&q);
    let ids: Vec<String> = rows.iter().map(|(d, _)| d.id().to_string()).collect();
    assert_eq!(ids, [LENGTH, AREA, VOLUME, "wall.layer-1.volume"]);
    let names: Vec<&str> = rows.iter().map(|(d, _)| d.name()).collect();
    assert_eq!(
        names,
        ["Length", "Area", "Volume", "Layer 1 (structure, m) volume"]
    );
    assert_eq!(
        rows[0].1,
        Some(Value::Length(Length::checked_from_mm(8000).unwrap()))
    );
    assert_eq!(rows[1].1, Some(Value::Area(mm2(24_000_000))));
    // 24,000,000 mm2 x 250 mm = 6,000,000,000 mm3.
    assert_eq!(rows[2].1, Some(Value::Volume(mm3(6_000_000_000))));
}

#[test]
fn a_wall_along_y_and_one_at_an_angle_are_exact_too() {
    // Along +y from (0, 0) to (0, 6000), 2700 high, one layer of 200 mm (+100 to -100); an opening
    // 1000 mm wide at 2000 to 3000 mm along, from the floor to 2100 mm.
    // Area: 6000 x 2700 - 1000 x 2100 = 16,200,000 - 2,100,000 = 14,100,000 mm2; volume x 200 =
    // 2,820,000,000 mm3.
    let along_y = Layered::of(&wall([0, 0], [0, 6000], 2700, &[("structure", 100, -100)])).unwrap();
    let q = quantities(&along_y, [0, 0], &[insert("D-1", 1, 2000, 3000, 0, 2100)]).unwrap();
    assert_eq!(q.length, Length::checked_from_mm(6000).unwrap());
    assert_eq!(q.area, mm2(14_100_000));
    assert_eq!(q.volume, mm3(2_820_000_000));
    // A 3-4-5 wall: (0, 0) to (3000, 4000) is exactly 5000 mm; 2500 high, one layer of 100 mm.
    // Area 5000 x 2500 = 12,500,000 mm2; the strip 5000 x 100 = 500,000 mm2, x 2500 =
    // 1,250,000,000 mm3.
    let slanted =
        Layered::of(&wall([0, 0], [3000, 4000], 2500, &[("structure", 50, -50)])).unwrap();
    let q = quantities(&slanted, [0, 0], &[]).unwrap();
    assert_eq!(q.length, Length::checked_from_mm(5000).unwrap());
    assert_eq!(q.area, mm2(12_500_000));
    assert_eq!(q.volume, mm3(1_250_000_000));
    // An opening in a wall that does not run along an axis is refused, not guessed.
    let refused = quantities(&slanted, [0, 0], &[insert("D-1", 0, 100, 1000, 0, 2100)]);
    assert!(matches!(refused, Err(MeasureError::OpeningNotAlongAxis(_))));
}

#[test]
fn a_joint_is_counted_the_abutting_wall_stops_at_the_through_wall() {
    // W-1 runs 0 to 8000 mm along x, 250 mm thick (+125 to -125), 2700 high; W-2, 120 mm thick
    // (+60 to -60), runs from (4000, 0), a point on W-1's line, to (4000, 5000). Both are one structure layer, so W-2 stops at W-1's
    // near face, which is 125 mm from the line: W-2's piece is 120 mm wide and 5000 - 125 = 4875
    // mm long = 585,000 mm2, and its volume is x 2700 = 1,579,500,000 mm3. Its line is still 5000 mm
    // long and its face area 5000 x 2700 = 13,500,000 mm2. W-1 is whole: 8000 x 250 = 2,000,000
    // mm2, x 2700 = 5,400,000,000 mm3.
    let first = wall([0, 0], [8000, 0], 2700, &[("structure", 125, -125)]);
    let second = wall([4000, 0], [4000, 5000], 2700, &[("structure", 60, -60)]);
    let joined = joins::among(&[("W-1", &first), ("W-2", &second)]);
    let w1 = Layered::of(&joined[0]).unwrap();
    let w2 = Layered::of(&joined[1]).unwrap();
    assert!(w2.joined.is_some());
    let q2 = quantities(&w2, [0, 0], &[]).unwrap();
    assert_eq!(q2.length, Length::checked_from_mm(5000).unwrap());
    assert_eq!(q2.area, mm2(13_500_000));
    assert_eq!(q2.volume, mm3(1_579_500_000));
    let q1 = quantities(&w1, [0, 0], &[]).unwrap();
    assert_eq!(q1.area, mm2(8000 * 2700));
    assert_eq!(q1.volume, mm3(5_400_000_000));
}

#[test]
fn an_opening_that_reaches_into_a_joint_is_refused_not_guessed() {
    // A layer whose one piece is a trapezoid, its end cut on a mitre: (0,0) (3000,0) (2900,100)
    // (100,100) mm, so a box from 0 to 500 mm along, across the whole 0 to 100 mm, has its
    // corners (0,100) and (500,100)... (0,100) is outside the piece.
    let mut wall = Layered::of(&wall([0, 0], [3000, 0], 2700, &[("structure", 0, -100)])).unwrap();
    let p = |x: i64, y: i64| [steps(x), steps(y)];
    wall.joined = Some(Joints {
        layers: vec![vec![vec![p(0, 0), p(3000, 0), p(2900, 100), p(100, 100)]]],
        outline: vec![p(0, 0), p(3000, 0), p(2900, 100), p(100, 100)],
    });
    let strip = Strip {
        function: "structure".to_owned(),
        material: "m".to_owned(),
        exterior: 0,
        interior: -steps(100),
    };
    assert_eq!(wall.layers, [strip]);
    let refused = quantities(&wall, [0, 0], &[insert("WIN-1", 0, 0, 500, 900, 2100)]).unwrap_err();
    assert_eq!(
        refused,
        MeasureError::OpeningInJoint {
            who: "window WIN-1".to_owned(),
            layer: 1
        }
    );
    // A box well inside the piece is counted: 1000 to 2000 mm along, 1000 x 1200 mm.
    // Piece area: the trapezoid, (3000 + 2800) / 2 x 100 = 290,000 mm2, x 2700 = 783,000,000 mm3;
    // less 1000 x 100 x 1200 = 120,000,000 gives 663,000,000 mm3.
    let q = quantities(&wall, [0, 0], &[insert("WIN-1", 0, 1000, 2000, 900, 2100)]).unwrap();
    assert_eq!(q.volume, mm3(663_000_000));
}
