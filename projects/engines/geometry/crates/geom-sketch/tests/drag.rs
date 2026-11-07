//! Tests of `geom_sketch::drag` (piece G228): dragging.
//!
//! The board's done-when: "Held-target projection, `AtLimit` and `DragLimit`, logged waypoints
//! replayed by continuation; drag scripts replay bit-identically; 4 ms frame at 1,000 entities."
//! The benchmark is `benches/drag.rs`.
//!
//! What this file proves: a dragged corner of a rectangle takes the cursor and its neighbours follow
//! with every constraint held; a cursor past what a stated length allows is `AtLimit`, names the
//! length, and leaves the point at the nearest allowed place; a cursor that would turn a parallel
//! line half a turn is `DragLimit` and the frame is the last good one; a frame with no budget shows
//! the previous frame; a script of frames replays to the same bits as it ran, twice over; a log
//! keeps at most 256 targets, the last among them; and a release commits and reports no drift for
//! an ordinary drag.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests, written as the definitions read; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fmt::Write;

use geom_run::{Budget, WorkMeter};
use geom_sketch::drag::{Dragger, MOST_WAYPOINTS, Status, replay};
use geom_sketch::model::{ItemId, Sketch};
use geom_sketch::text::parse;

const HEAD: &str = "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n";

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Points 10.0 to 10.3 are the corners of a rectangle going round from the origin; lines 11.0 to
/// 11.3 its sides: 11.0 bottom, 11.1 right, 11.2 top, 11.3 left. The origin is fixed.
fn rectangle() -> Sketch {
    let mut text = HEAD.to_owned();
    for (key, (x, y)) in [(0.0, 0.0), (400.0, 0.0), (400.0, 300.0), (0.0, 300.0)]
        .iter()
        .enumerate()
    {
        writeln!(text, "entity 10.{key} point {x} {y}\n").unwrap();
    }
    for (key, (p, q)) in [(0, 1), (1, 2), (3, 2), (0, 3)].iter().enumerate() {
        writeln!(text, "entity 11.{key} line 10.{p} 10.{q} segment\n").unwrap();
    }
    text.push_str("constraint 12.0 driving 0 horizontal 11.0\n");
    text.push_str("constraint 12.1 driving 0 vertical 11.1\n");
    text.push_str("constraint 12.2 driving 0 horizontal 11.2\n");
    text.push_str("constraint 12.3 driving 0 vertical 11.3\n");
    text.push_str("constraint 12.4 driving 0 fix 10.0 0 0\n");
    parse(&text).unwrap()
}

fn point(s: &Sketch, key: u64) -> [f64; 2] {
    s.point(ItemId::new(10, key)).unwrap()
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 0.02 && (a[1] - b[1]).abs() < 0.02
}

#[test]
fn a_dragged_corner_takes_the_cursor_and_its_neighbours_follow() {
    let s = rectangle();
    let mut drag = Dragger::begin(&s, &[ItemId::new(10, 2)], &mut meter()).unwrap();
    let frame = drag.frame(&[[500.0, 350.0]], &mut meter()).unwrap();
    assert_eq!(frame.status, Status::Held);
    assert!(near(point(&frame.sketch, 2), [500.0, 350.0]));
    assert!(near(point(&frame.sketch, 1), [500.0, 0.0]));
    assert!(near(point(&frame.sketch, 3), [0.0, 350.0]));
    assert!(near(point(&frame.sketch, 0), [0.0, 0.0]));
}

#[test]
fn each_frame_starts_from_the_last() {
    let s = rectangle();
    let mut drag = Dragger::begin(&s, &[ItemId::new(10, 2)], &mut meter()).unwrap();
    for i in 1..=20 {
        let at = f64::from(i) * 10.0;
        let frame = drag
            .frame(&[[400.0 + at, 300.0 + at]], &mut meter())
            .unwrap();
        assert_eq!(frame.status, Status::Held);
        assert!(near(point(&frame.sketch, 2), [400.0 + at, 300.0 + at]));
    }
}

#[test]
fn a_cursor_past_a_stated_length_is_at_limit_and_names_the_length() {
    // Pin the bottom side's length: the right corners cannot leave x = 400 while the origin is fixed.
    let mut s = rectangle();
    let kind = geom_sketch::model::ConstraintKind::Distance {
        a: ItemId::new(10, 0),
        b: ItemId::new(10, 1),
        distance: geom_sketch::model::Amount::Value(400.0),
    };
    s.add_constraint(geom_sketch::model::Constraint {
        id: ItemId::new(12, 6),
        kind,
        standing: geom_sketch::model::Standing::Driving,
        priority: 0,
    })
    .unwrap();
    let mut drag = Dragger::begin(&s, &[ItemId::new(10, 2)], &mut meter()).unwrap();
    let frame = drag.frame(&[[900.0, 300.0]], &mut meter()).unwrap();
    match frame.status {
        Status::AtLimit { blocking } => {
            assert!(blocking.contains(&ItemId::new(12, 6)), "{blocking:?}");
        }
        other => panic!("not at limit: {other:?}"),
    }
    // The right side stays at x = 400, the nearest the constraints allow.
    assert!((point(&frame.sketch, 2)[0] - 400.0).abs() < 0.02);
    assert!((point(&frame.sketch, 1)[0] - 400.0).abs() < 0.02);
    // A cursor back inside the limit holds again.
    let again = drag.frame(&[[400.0, 340.0]], &mut meter()).unwrap();
    assert_eq!(again.status, Status::Held);
    assert!(near(point(&again.sketch, 2), [400.0, 340.0]));
}

#[test]
fn a_turn_to_the_other_branch_is_drag_limit_and_the_frame_is_the_last_good_one() {
    let mut text = HEAD.to_owned();
    for (key, (x, y)) in [(0.0, 0.0), (100.0, 0.0), (0.0, 50.0), (100.0, 50.0)]
        .iter()
        .enumerate()
    {
        writeln!(text, "entity 10.{key} point {x} {y}\n").unwrap();
    }
    text.push_str("entity 11.0 line 10.0 10.1 segment\n");
    text.push_str("entity 11.1 line 10.2 10.3 segment\n");
    for (key, (entity, slot, value)) in [
        (0, 0, 0.0),
        (0, 1, 0.0),
        (1, 0, 100.0),
        (1, 1, 0.0),
        (2, 0, 0.0),
        (2, 1, 50.0),
    ]
    .iter()
    .enumerate()
    {
        writeln!(
            text,
            "constraint 12.{key} driving 0 fix 10.{entity} {slot} {value}"
        )
        .unwrap();
    }
    text.push_str(
        "constraint 12.9 driving 0 parallel 11.0 11.1
",
    );
    let s = parse(&text).unwrap();
    let mut drag = Dragger::begin(&s, &[ItemId::new(10, 3)], &mut meter()).unwrap();
    let before = drag.frame(&[[100.0, 50.0]], &mut meter()).unwrap();
    assert_eq!(before.status, Status::Held);
    let frame = drag.frame(&[[-100.0, 50.0]], &mut meter()).unwrap();
    match frame.status {
        Status::DragLimit { blocking } => assert_ne!(blocking.len(), 0),
        other => panic!("not a drag limit: {other:?}"),
    }
    assert_eq!(frame.sketch, before.sketch);
}

#[test]
fn a_frame_with_no_budget_shows_the_previous_frame() {
    let s = rectangle();
    let mut drag = Dragger::begin(&s, &[ItemId::new(10, 2)], &mut meter()).unwrap();
    let first = drag.frame(&[[450.0, 320.0]], &mut meter()).unwrap();
    let mut none = WorkMeter::new(Budget::of_work(1));
    let behind = drag.frame(&[[600.0, 600.0]], &mut none).unwrap();
    assert_eq!(behind.status, Status::Behind);
    assert_eq!(behind.sketch, first.sketch);
    // The drag goes on from where it was.
    let next = drag.frame(&[[600.0, 600.0]], &mut meter()).unwrap();
    assert_eq!(next.status, Status::Held);
    assert!(near(point(&next.sketch, 2), [600.0, 600.0]));
}

fn script(drag: &mut Dragger, frames: usize) -> geom_sketch::drag::Frame {
    let mut last = None;
    for i in 0..frames {
        let t = i as f64;
        let cursor = [[400.0 + 3.0 * t, 300.0 + 2.0 * t + 0.37 * (t % 7.0)]];
        last = Some(drag.frame(&cursor, &mut meter()).unwrap());
    }
    last.unwrap()
}

#[test]
fn a_drag_script_replays_to_the_same_bits_as_it_ran_and_twice_over() {
    let s = rectangle();
    let mut drag = Dragger::begin(&s, &[ItemId::new(10, 2)], &mut meter()).unwrap();
    let live = script(&mut drag, 60);
    let log = drag.log_of().unwrap();
    assert_eq!(log.path.len(), 60);
    assert_eq!(log.every, 1);
    let one = replay(&log, &mut meter()).unwrap();
    let two = replay(&log, &mut meter()).unwrap();
    assert_eq!(one, live.sketch);
    assert_eq!(one, two);
}

#[test]
fn a_log_keeps_at_most_256_targets_and_the_last() {
    let s = rectangle();
    let mut drag = Dragger::begin(&s, &[ItemId::new(10, 2)], &mut meter()).unwrap();
    let live = script(&mut drag, 600);
    let log = drag.log_of().unwrap();
    assert!(log.path.len() <= MOST_WAYPOINTS);
    assert_eq!(log.frames, 600);
    assert!(log.every > 1);
    let last = [[
        400.0 + 3.0 * 599.0,
        300.0 + 2.0 * 599.0 + 0.37 * (599.0_f64 % 7.0),
    ]];
    assert_eq!(log.path.last().unwrap().as_slice(), last.as_slice());
    // The thinned path still ends where the drag did, to the solver's tolerance.
    let end = replay(&log, &mut meter()).unwrap();
    assert!(near(point(&end, 2), point(&live.sketch, 2)));
}

#[test]
fn a_release_commits_the_replay_and_reports_no_drift_for_an_ordinary_drag() {
    let s = rectangle();
    let mut drag = Dragger::begin(&s, &[ItemId::new(10, 2)], &mut meter()).unwrap();
    let live = script(&mut drag, 40);
    let released = drag.release(&mut meter()).unwrap();
    assert_eq!(released.sketch, live.sketch);
    assert_eq!(released.moved, Vec::new());
    assert!(released.largest < 1.0, "{}", released.largest);
    assert_eq!(released.committed.points.len(), 4);
}

#[test]
fn a_point_that_is_not_one_cannot_be_dragged() {
    let s = rectangle();
    assert!(Dragger::begin(&s, &[ItemId::new(11, 0)], &mut meter()).is_err());
}
