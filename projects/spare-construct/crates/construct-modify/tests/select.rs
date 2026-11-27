//! A selection moved, copied and arrayed: what follows its host, what moves alone and how, the
//! copies' IDs, names and hosts, and the refusals, each naming the element.

// The crate denies these outside tests (Cargo.toml); a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod fixtures;

use construct_modify::array::{linear, linear_span, radial, radial_sweep};
use construct_modify::{Copies, ModifyError, Motion, Request, Turn, plan};
use construct_params::{Angle, Value};
use construct_regen::hosting;
use fixtures::*;
use kit_core::Id;

const NOT_FLIPPED: Fixed = Fixed { flipped: false };

fn ids(list: &[&str]) -> Vec<Id> {
    list.iter().map(|id| i(id)).collect()
}

fn move_by(dx: i64, dy: i64) -> Request {
    Request::Move(Motion::by([dx * MM, dy * MM]))
}

fn quarter() -> Motion {
    Motion::turn_about([0, 0], &Turn::quarters(1))
}

/// An angle of `tenths` tenths of a degree.
fn tenths(tenths: i64) -> Angle {
    Angle::from_units(tenths * Angle::UNITS_PER_DEGREE / 10)
}

fn copy(key: &str, motions: Vec<Motion>) -> Request {
    Request::Copy(Copies {
        key: key.to_owned(),
        motions,
    })
}

fn along(record: &construct_model::element::Element) -> i64 {
    match Value::from_stored(&record.props[hosting::ALONG_KEY]) {
        Some(Value::Length(l)) => l.steps(),
        other => panic!("{other:?}"),
    }
}

fn host_of(record: &construct_model::element::Element) -> Id {
    hosting::host_of(record).unwrap()
}

/// A wall along x with a box door, a placed door and a wall opening on it, and a wall of its
/// own beside it.
fn walls() -> std::collections::BTreeMap<Id, construct_model::element::Element> {
    document(vec![
        wall("W-1", [0, 0], [6000, 0]),
        door("D-1", "W-1", 500),
        swing_door("D-2", "W-1", 3000),
        wall_opening("O-1", "W-1", 4500, false),
        wall("W-2", [0, 3000], [6000, 3000]),
    ])
}

#[test]
fn what_a_wall_hosts_follows_it_unchanged() {
    let model = walls();
    let planned = plan(&model, &ids(&["W-1"]), &move_by(1000, 500), &NOT_FLIPPED).unwrap();
    let changed: Vec<&str> = planned.changed.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(changed, ["W-1"], "its doors' records are not rewritten");
    assert_eq!(planned.made.len(), 0);
    assert_eq!(
        planned.hosts.iter().map(Id::as_str).collect::<Vec<_>>(),
        ["W-1"]
    );
    // Naming the doors too changes nothing: they follow their host either way.
    let all = plan(
        &model,
        &ids(&["D-1", "W-1", "D-2"]),
        &move_by(1000, 500),
        &NOT_FLIPPED,
    )
    .unwrap();
    assert_eq!(all, planned);
}

#[test]
fn a_quarter_turn_changes_the_axis_its_box_door_and_opening_name() {
    let model = walls();
    let planned = plan(
        &model,
        &ids(&["W-1"]),
        &Request::Move(quarter()),
        &NOT_FLIPPED,
    )
    .unwrap();
    let changed: Vec<&str> = planned.changed.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(changed, ["D-1", "O-1", "W-1"]);
    assert_eq!(planned.changed[0].recipe.args["axis"], "y");
}

#[test]
fn a_mirror_changes_the_placed_doors_hand_and_the_walls_side() {
    let model = walls();
    let mirror = Motion::mirror([0, 0], [0, MM]).unwrap();
    let planned = plan(&model, &ids(&["W-1"]), &Request::Move(mirror), &NOT_FLIPPED).unwrap();
    let changed: Vec<&str> = planned.changed.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(changed, ["D-2", "W-1"]);
    assert_eq!(
        value_of(&planned.changed[0], "swing").as_deref(),
        Some("choice:right")
    );
    assert_eq!(
        planned.changed[1].props["wall.flipped"],
        Value::YesNo(true).stored()
    );
}

#[test]
fn a_hosted_element_alone_slides_along_its_host_or_is_refused_by_name() {
    let model = walls();
    let planned = plan(&model, &ids(&["D-1"]), &move_by(250, 0), &NOT_FLIPPED).unwrap();
    assert_eq!(planned.changed.len(), 1);
    assert_eq!(along(&planned.changed[0]), 750 * MM);
    assert_eq!(host_of(&planned.changed[0]), i("W-1"));
    assert_eq!(
        planned.hosts.iter().map(Id::as_str).collect::<Vec<_>>(),
        ["W-1"]
    );
    // A wall running toward -x slides it the other way along.
    let reversed = document(vec![
        wall("W-9", [6000, 0], [0, 0]),
        door("D-9", "W-9", 500),
    ]);
    let planned = plan(&reversed, &ids(&["D-9"]), &move_by(250, 0), &NOT_FLIPPED).unwrap();
    assert_eq!(along(&planned.changed[0]), 250 * MM);

    let refused = plan(&model, &ids(&["D-1"]), &move_by(0, 10), &NOT_FLIPPED).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "D-1 is hosted on W-1: alone it moves only along W-1, and this would take it off. Move \
         W-1 with it, or move it along W-1."
    );
    let refused = plan(
        &model,
        &ids(&["D-2"]),
        &Request::Move(quarter()),
        &NOT_FLIPPED,
    )
    .unwrap_err();
    assert_eq!(
        refused,
        ModifyError::TurnsWithHost {
            element: "D-2".to_owned(),
            host: "W-1".to_owned()
        }
    );
    let angled = document(vec![
        wall("W-3", [0, 0], [3000, 4000]),
        door("D-3", "W-3", 500),
    ]);
    let refused = plan(&angled, &ids(&["D-3"]), &move_by(30, 40), &NOT_FLIPPED).unwrap_err();
    assert_eq!(
        refused,
        ModifyError::HostAtAngle {
            element: "D-3".to_owned(),
            host: "W-3".to_owned()
        }
    );
}

#[test]
fn an_older_door_hosted_by_its_position_is_refused_by_name() {
    let model = document(vec![wall("W-1", [0, 0], [6000, 0]), old_door("D-0", "W-1")]);
    let refused = plan(&model, &ids(&["W-1"]), &move_by(10, 0), &NOT_FLIPPED).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "D-0 is hosted by where it was placed, as older doors were: move it along its host once \
         with hosted.move, and it moves with its host from then on."
    );
}

#[test]
fn a_floor_opening_moves_with_its_floor() {
    let model = document(vec![
        floor("F-1", &[[0, 0], [4000, 0], [4000, 3000], [0, 3000]]),
        floor_opening("O-1", "F-1", &[[100, 100], [600, 100], [600, 400]]),
    ]);
    let planned = plan(&model, &ids(&["F-1"]), &move_by(1000, 0), &NOT_FLIPPED).unwrap();
    let changed: Vec<&str> = planned.changed.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(changed, ["F-1", "O-1"]);
    assert_eq!(
        outline_corners(&planned.changed[1])[0],
        [1100 * MM, 100 * MM]
    );
    // Alone, it moves in its floor's plan and stays on it.
    let alone = plan(&model, &ids(&["O-1"]), &move_by(10, 10), &NOT_FLIPPED).unwrap();
    assert_eq!(alone.changed.len(), 1);
    assert_eq!(host_of(&alone.changed[0]), i("F-1"));
    assert_eq!(
        alone.hosts.iter().map(Id::as_str).collect::<Vec<_>>(),
        ["F-1"]
    );
}

#[test]
fn copies_take_new_ids_and_their_hosted_elements_the_copy_of_their_host() {
    let model = walls();
    let planned = plan(
        &model,
        &ids(&["W-1"]),
        &copy("C", vec![Motion::by([0, 1000 * MM])]),
        &NOT_FLIPPED,
    )
    .unwrap();
    assert_eq!(planned.changed.len(), 0);
    let made: Vec<(&str, &str)> = planned
        .made
        .iter()
        .map(|r| (r.id.as_str(), r.name.as_str()))
        .collect();
    // Counted in the order of the originals' IDs: D-1, D-2, O-1, W-1.
    assert_eq!(
        made,
        [
            ("C.1", "D-1"),
            ("C.2", "D-2"),
            ("C.3", "O-1"),
            ("C.4", "W-1")
        ]
    );
    for hosted in &planned.made[..3] {
        assert_eq!(host_of(hosted), i("C.4"));
    }
    let wall_copy = &planned.made[3];
    assert_eq!(point_of(wall_copy, "start"), [0, 1000 * MM]);
    assert_eq!(
        wall_copy.props,
        model[&i("W-1")].props,
        "same type and parameters"
    );
    assert_eq!(
        planned.hosts.iter().map(Id::as_str).collect::<Vec<_>>(),
        ["C.4"]
    );
    // A door copied alone stays on its own host, further along.
    let alone = plan(
        &model,
        &ids(&["D-1"]),
        &copy("K", vec![Motion::by([1000 * MM, 0])]),
        &NOT_FLIPPED,
    )
    .unwrap();
    assert_eq!(alone.made[0].id, i("K.1"));
    assert_eq!(host_of(&alone.made[0]), i("W-1"));
    assert_eq!(along(&alone.made[0]), 1500 * MM);
}

#[test]
fn an_arrays_copies_count_their_copy_and_their_original() {
    let model = walls();
    let motions = linear([0, 1000 * MM], 3).unwrap();
    let planned = plan(&model, &ids(&["W-2"]), &copy("A", motions), &NOT_FLIPPED).unwrap();
    let made: Vec<&str> = planned.made.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(made, ["A.1.1", "A.2.1"]);
    assert_eq!(point_of(&planned.made[1], "start"), [0, 5000 * MM]);
}

#[test]
fn copies_of_grids_and_rooms_take_the_next_names_and_numbers() {
    let model = document(vec![
        grid_line("G-A", "A", [0, 0], [0, 9000]),
        grid_line("G-B", "B", [6000, 0], [6000, 9000]),
        room("R-1", "101", [1000, 1000]),
        room("R-2", "102", [3000, 1000]),
    ]);
    let motions = linear([12_000 * MM, 0], 3).unwrap();
    let planned = plan(
        &model,
        &ids(&["G-A", "R-1"]),
        &copy("Z", motions),
        &NOT_FLIPPED,
    )
    .unwrap();
    let names: Vec<&str> = planned
        .made
        .iter()
        .filter(|r| r.kind == "grid")
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(names, ["C", "D"], "B is taken");
    let numbers: Vec<&str> = planned
        .made
        .iter()
        .filter_map(construct_rooms::number_of)
        .collect();
    assert_eq!(numbers, ["103", "104"]);
}

#[test]
fn copies_are_refused_in_words() {
    let model = walls();
    let refused = plan(
        &model,
        &ids(&["W-2"]),
        &copy("C", vec![Motion::identity()]),
        &NOT_FLIPPED,
    )
    .unwrap_err();
    assert_eq!(
        refused.to_string(),
        "That would put every copy on its original."
    );
    let taken = document(vec![
        wall("W-2", [0, 0], [1000, 0]),
        wall("C.1", [0, 5], [1, 5]),
    ]);
    let refused = plan(
        &taken,
        &ids(&["W-2"]),
        &copy("C", vec![Motion::by([0, MM])]),
        &NOT_FLIPPED,
    )
    .unwrap_err();
    assert_eq!(
        refused.to_string(),
        "An element C.1 is already in this model: give the copies another key."
    );
    let long = "k".repeat(64);
    let refused = plan(
        &model,
        &ids(&["W-2"]),
        &copy(&long, vec![Motion::by([0, MM])]),
        &NOT_FLIPPED,
    )
    .unwrap_err();
    assert_eq!(refused, ModifyError::CopyKey { key: long });
}

#[test]
fn a_selection_is_refused_by_name() {
    let model = walls();
    assert_eq!(
        plan(&model, &[], &move_by(1, 0), &NOT_FLIPPED).unwrap_err(),
        ModifyError::Nothing
    );
    assert_eq!(
        plan(&model, &ids(&["W-7"]), &move_by(1, 0), &NOT_FLIPPED)
            .unwrap_err()
            .to_string(),
        "There is no element W-7 in this model."
    );
}

/// A span split evenly is split exactly; one that does not split on the grid puts each copy at
/// the nearest step to its share, worked out from the originals: 64 steps in 7 are 9.14, 18.29,
/// ... steps, so the copies sit 9, 18, 27, 37, 46, 55 and 64 along, and the last is the span.
#[test]
fn a_linear_array_by_a_span_puts_each_copy_at_the_nearest_step_to_its_share() {
    let motions = linear_span([900 * MM, 0], 4).unwrap();
    assert_eq!(motions[0].translation(), Some([300 * MM, 0]));
    assert_eq!(motions[2].translation(), Some([900 * MM, 0]));
    let shares: Vec<[i64; 2]> = linear_span([64, -64], 8)
        .unwrap()
        .iter()
        .map(|m| m.translation().unwrap())
        .collect();
    let along: Vec<i64> = shares.iter().map(|[x, _]| *x).collect();
    assert_eq!(along, [9, 18, 27, 37, 46, 55, 64]);
    let across: Vec<i64> = shares.iter().map(|[_, y]| *y).collect();
    assert_eq!(
        across,
        [-9, -18, -27, -37, -46, -55, -64],
        "a half away from zero"
    );
    assert_eq!(linear([0, 0], 3).unwrap_err(), ModifyError::InPlace);
    assert_eq!(linear_span([0, 0], 3).unwrap_err(), ModifyError::InPlace);
    assert_eq!(linear([1, 0], 1).unwrap_err(), ModifyError::TooFew);
}

/// A radial array turns by any angle, each copy worked out from the originals by its own angle,
/// so a copy that comes to a quarter turn lands exactly however many small steps came before it;
/// one that would come round onto the originals is refused.
#[test]
fn a_radial_array_turns_by_any_angle_each_copy_from_the_originals() {
    let motions = radial([0, 0], degrees(90), 4).unwrap();
    assert_eq!(motions.len(), 3);
    assert_eq!(motions[1].apply([MM, 0]), Some([-MM, 0]));
    let refused = radial([0, 0], degrees(90), 5).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "5 at that angle come round onto the original: at most 4 fit."
    );
    // Eight at 45 degrees: the first at the nearest step to (1 mm / sqrt 2, 1 mm / sqrt 2), the
    // second a quarter turn exactly.
    let eighths = radial([0, 0], degrees(45), 8).unwrap();
    assert_eq!(eighths.len(), 7);
    assert_eq!(eighths[0].apply([MM, 0]), Some([4525, 4525]));
    assert_eq!(eighths[1].apply([MM, 0]), Some([0, MM]));
    assert_eq!(eighths[3].apply([MM, 0]), Some([-MM, 0]));
    assert_eq!(
        radial([0, 0], degrees(45), 9).unwrap_err(),
        ModifyError::RoundAgain { count: 9, most: 8 }
    );
    // 900 steps of 0.1 degrees: the last is a quarter turn, exactly, with nothing built up.
    let small = radial([0, 0], tenths(1), 901).unwrap();
    assert_eq!(small.len(), 900);
    assert_eq!(small[899].apply([10_000 * MM, 0]), Some([0, 10_000 * MM]));
    assert_eq!(
        small[449].apply([10_000 * MM, 0]),
        turned(degrees(45), [10_000 * MM, 0])
    );
}

/// A sweep splits into any number of turns, in whole angle units or not; a sweep of a full turn
/// spreads the count evenly round. The first of eight over 100 degrees (100/7 degrees) and the
/// first of seven round (360/7 degrees) take 10 m along x to the points pinned, worked out apart
/// from this crate.
#[test]
fn a_radial_array_by_a_sweep_splits_it_into_any_count() {
    let ten_m = [10_000 * MM, 0];
    let swept = radial_sweep([0, 0], degrees(180), 3).unwrap();
    assert_eq!(swept[0].apply([MM, 0]), Some([0, MM]));
    let fifths = radial_sweep([0, 0], degrees(180), 5).unwrap();
    assert_eq!(fifths[1].apply([MM, 0]), Some([0, MM]));
    assert_eq!(fifths[3].apply([MM, 0]), Some([-MM, 0]));
    let sevenths = radial_sweep([0, 0], degrees(100), 8).unwrap();
    assert_eq!(sevenths[0].apply(ten_m), Some([62_020_946, 15_792_473]));
    assert_eq!(sevenths[6].apply(ten_m), turned(degrees(100), ten_m));
    let round = radial_sweep([0, 0], degrees(360), 7).unwrap();
    assert_eq!(round.len(), 6);
    assert_eq!(round[0].apply(ten_m), Some([39_903_347, 50_037_215]));
    let back = radial_sweep([0, 0], degrees(-360), 4).unwrap();
    assert_eq!(back[0].apply([MM, 0]), Some([0, -MM]));
    // Seven angle units over two steps are three and a half each; the second is seven exactly.
    let halves = radial_sweep([0, 0], Angle::from_units(7), 3).unwrap();
    assert_eq!(
        halves[1],
        radial([0, 0], Angle::from_units(7), 2).unwrap()[0]
    );
    assert_eq!(
        radial_sweep([0, 0], degrees(0), 3).unwrap_err(),
        ModifyError::InPlace
    );
}

fn turned(angle: Angle, p: [i64; 2]) -> Option<[i64; 2]> {
    Motion::turn_about([0, 0], &Turn::of_angle(angle)).apply(p)
}

/// Three walls joined at their corners, turned together by 37.5 degrees about a point off the
/// millimetres: each shared end is rounded once, so they are still joined.
#[test]
fn joined_walls_turned_by_any_angle_stay_joined() {
    let model = document(vec![
        wall("W-1", [0, 0], [4000, 0]),
        wall("W-2", [4000, 0], [4000, 3000]),
        wall("W-3", [4000, 3000], [0, 3000]),
    ]);
    let centre = [1234 * MM + 17, 567 * MM - 3];
    let turn = Motion::turn_about(centre, &Turn::of_angle(tenths(375)));
    let planned = plan(
        &model,
        &ids(&["W-1", "W-2", "W-3"]),
        &Request::Move(turn),
        &NOT_FLIPPED,
    )
    .unwrap();
    let [w1, w2, w3] = &planned.changed[..] else {
        panic!("{:?}", planned.changed)
    };
    assert_eq!(point_of(w1, "end"), point_of(w2, "start"));
    assert_eq!(point_of(w2, "end"), point_of(w3, "start"));
    assert_ne!(point_of(w1, "end"), point_of(&model[&i("W-1")], "end"));
}

/// A wall turned by any angle takes what it hosts with it, their records unchanged: each sits at
/// its offsets along the wall, so it stays on it exactly. (Whether the builder hosts a door on a
/// wall off the axes is the server's check.)
#[test]
fn what_a_wall_hosts_follows_any_turn_unchanged() {
    let model = walls();
    let turn = Motion::turn_about([0, 0], &Turn::of_angle(tenths(375)));
    let planned = plan(&model, &ids(&["W-1"]), &Request::Move(turn), &NOT_FLIPPED).unwrap();
    let changed: Vec<&str> = planned.changed.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(changed, ["W-1"]);
    assert_eq!(
        planned.hosts.iter().map(Id::as_str).collect::<Vec<_>>(),
        ["W-1"]
    );
}
