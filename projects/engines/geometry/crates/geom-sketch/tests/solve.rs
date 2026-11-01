//! Tests of `geom_sketch`'s block solver and continuation (piece G222).
//!
//! Each sketch here has an answer worked by hand. A solve meets every rule within a 64th of a step and
//! moves what is free as little as it can; a sketch that cannot be met is refused, naming a rule; a
//! singular start is nudged by its id; a budget stops it. A path keeps an angle's branch where a jump
//! would leave it, and is lost, naming the rule and the place, where the solution ends. ezpz's problem
//! files (MIT, kept in `data/ezpz` with their notice, at 1024 steps to their unit) all run, and their
//! solved values are held to a golden hash.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::float_arithmetic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    reason = "tests, written as the definitions read"
)]

use std::fs;
use std::path::Path;

use geom_run::{Budget, WorkMeter};
use geom_sketch::model::{ItemId, Sketch, SketchError};
use geom_sketch::solve::{MOST_HALVINGS, Solved, TOLERANCE, continue_to, solve};
use geom_sketch::text::parse;
use geom_test::golden::{self, hex, sha256};

const HEAD: &str = "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n";

fn id(sequence: u64, key: u64) -> ItemId {
    ItemId::new(sequence, key)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn sketch(lines: &str) -> Sketch {
    parse(&format!("{HEAD}{lines}")).unwrap()
}

fn solved(lines: &str) -> Solved {
    solve(&sketch(lines), &mut meter()).unwrap()
}

fn near(s: &Sketch, point: ItemId, x: f64, y: f64) {
    let [px, py] = s.point(point).unwrap();
    assert!(
        (px - x).abs() < 2.0 * TOLERANCE && (py - y).abs() < 2.0 * TOLERANCE,
        "{point:?} is at ({px}, {py}), not ({x}, {y})"
    );
}

const TRIANGLE: &str = "entity 1.0 point 0 0
entity 1.1 point 2900 100
entity 1.2 point 3100 3900
entity 2.0 line 1.0 1.1 segment
constraint 3.0 driving 0 fix 1.0 0 0
constraint 3.1 driving 0 fix 1.0 1 0
constraint 3.2 driving 0 horizontal 2.0
constraint 3.3 driving 0 distance 1.0 1.1 3000
constraint 3.4 driving 0 distance 1.1 1.2 4000
constraint 3.5 driving 0 distance 1.0 1.2 5000
";

#[test]
fn a_triangle_from_its_three_sides_keeps_the_side_it_was_drawn_on() {
    let done = solved(TRIANGLE);
    near(&done.sketch, id(1, 0), 0.0, 0.0);
    near(&done.sketch, id(1, 1), 3000.0, 0.0);
    near(&done.sketch, id(1, 2), 3000.0, 4000.0);
    assert!(done.report.largest <= TOLERANCE);
    assert!(done.report.iterations > 1);
    assert_eq!(done.report.steps, 1);
    assert_eq!(done.report.freedoms, 0);
    assert_eq!(done.report.nudged, []);
    assert!(done.report.stages >= 2);
}

#[test]
fn what_is_free_moves_as_little_as_it_can() {
    // Both ends of a line 800 long may move to make it 1000: each takes half, along it.
    let done = solved(
        "entity 1.0 point 0 0
entity 1.1 point 800 0
entity 1.2 point 5 6
constraint 2.0 driving 0 distance 1.0 1.1 1000
",
    );
    near(&done.sketch, id(1, 0), -100.0, 0.0);
    near(&done.sketch, id(1, 1), 900.0, 0.0);
    // A point no rule names has not moved at all.
    assert_eq!(done.sketch.point(id(1, 2)), Ok([5.0, 6.0]));
    assert_eq!(done.report.freedoms, 5);
    // With one end held, the other goes straight out along the line.
    let done = solved(
        "entity 1.0 point 0 0
entity 1.1 point 300 400
constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 distance 1.0 1.1 1000
",
    );
    near(&done.sketch, id(1, 1), 600.0, 800.0);
}

#[test]
fn a_rule_that_is_met_moves_nothing() {
    let lines = "entity 1.0 point 3 4
entity 1.1 point 3 9
constraint 2.0 driving 0 distance 1.0 1.1 5
";
    let done = solved(lines);
    assert_eq!(done.sketch, sketch(lines));
    assert_eq!(done.report.iterations, 0);
    assert_eq!(done.report.largest, 0.0);
}

#[test]
fn merged_variables_take_the_older_value() {
    let done = solved(
        "entity 1.0 point 10 20
entity 1.1 point 30 40
constraint 2.0 driving 0 coincident 1.0 1.1
",
    );
    assert_eq!(done.sketch.point(id(1, 1)), Ok([10.0, 20.0]));
    assert_eq!(done.report.stages, 0);
    assert_eq!(done.report.iterations, 0);
}

#[test]
fn what_cannot_be_met_is_refused_naming_a_rule() {
    let lines = "entity 1.0 point 0 0
constraint 2.0 driving 0 fix 1.0 0 1
constraint 2.1 driving 0 fix 1.0 0 2
";
    let Err(SketchError::Unsolved(rule)) = solve(&sketch(lines), &mut meter()) else {
        panic!("two values for one variable");
    };
    assert!(rule == id(2, 0) || rule == id(2, 1));
    // A triangle with one side longer than the other two together.
    let lines = "entity 1.0 point 0 0
entity 1.1 point 1000 0
entity 1.2 point 500 100
constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 fix 1.1 0 1000
constraint 2.3 driving 0 fix 1.1 1 0
constraint 2.4 driving 0 distance 1.0 1.2 200
constraint 2.5 driving 0 distance 1.1 1.2 200
";
    assert!(matches!(
        solve(&sketch(lines), &mut meter()),
        Err(SketchError::Unsolved(_))
    ));
}

#[test]
fn a_rule_said_twice_is_met_twice() {
    let done = solved(
        "entity 1.0 point 5 0
constraint 2.0 driving 0 fix 1.0 0 7
constraint 2.1 driving 0 fix 1.0 0 7
",
    );
    near(&done.sketch, id(1, 0), 7.0, 0.0);
}

#[test]
fn a_radius_cannot_end_below_zero() {
    let lines = "entity 1.0 point 0 0
entity 2.0 circle 1.0 10
constraint 3.0 driving 0 fix 2.0 0 -5
";
    assert_eq!(
        solve(&sketch(lines), &mut meter()),
        Err(SketchError::NotPositive)
    );
}

#[test]
fn a_budget_stops_it_with_no_answer() {
    let s = sketch(TRIANGLE);
    let mut none = WorkMeter::new(Budget::of_work(1));
    assert_eq!(solve(&s, &mut none), Err(SketchError::Stopped));
    // What the whole solve costs, and what its plan alone does: a sketch already at its answer
    // takes no iteration. A budget that holds the plan and not the iterations stops it.
    let cost = |s: &Sketch| {
        let mut m = meter();
        let done = solve(s, &mut m).unwrap();
        (u64::try_from(m.used().work).unwrap(), done)
    };
    let (whole, done) = cost(&s);
    let (plan, again) = cost(&done.sketch);
    assert_eq!(again.report.iterations, 0);
    assert!(plan < whole, "{plan} {whole}");
    let mut plan_only = WorkMeter::new(Budget::of_work(plan));
    assert_eq!(solve(&s, &mut plan_only), Err(SketchError::Stopped));
    let mut short = WorkMeter::new(Budget::of_work(whole - 1));
    assert_eq!(solve(&s, &mut short), Err(SketchError::Stopped));
    let mut enough = WorkMeter::new(Budget::of_work(whole));
    assert_eq!(solve(&s, &mut enough), Ok(done));
}

const FLAT_START: &str = "entity 1.0 point 100 200
entity 1.1 point 100 200
entity 2.0 line 1.0 1.1 segment
constraint 3.0 driving 0 heading 2.0 0.25
constraint 3.1 driving 0 length 2.0 100
";

#[test]
fn a_singular_start_is_nudged_by_its_id() {
    let done = solved(FLAT_START);
    assert_eq!(done.report.nudged, [id(3, 0)]);
    assert!(done.report.largest <= TOLERANCE);
    let [ax, ay] = done.sketch.point(id(1, 0)).unwrap();
    let [bx, by] = done.sketch.point(id(1, 1)).unwrap();
    assert!((ax - bx).abs() > 1.0, "{ax} {bx}");
    assert!(((bx - ax).abs() - (by - ay).abs()).abs() < 2.0 * TOLERANCE);
    // Nudged by the id alone: the same again, to the bit, and another id goes another way.
    assert_eq!(solved(FLAT_START), done);
    let other = solved(&FLAT_START.replace("3.0 driving", "3.7 driving"));
    assert_ne!(other.sketch, done.sketch);
}

#[test]
fn a_rule_that_stays_singular_is_degenerate() {
    // The line's ends are fixed one on the other, in a stage before the point's.
    let lines = "entity 1.0 point 7 7
entity 1.1 point 7 7
entity 1.2 point 1 1
entity 2.0 line 1.0 1.1 segment
constraint 3.0 driving 0 fix 1.0 0 7
constraint 3.1 driving 0 fix 1.0 1 7
constraint 3.2 driving 0 fix 1.1 0 7
constraint 3.3 driving 0 fix 1.1 1 7
constraint 3.4 driving 0 on-line 1.2 2.0
";
    assert_eq!(
        solve(&sketch(lines), &mut meter()),
        Err(SketchError::Degenerate(id(3, 4)))
    );
}

/// A line 1000 long from a fixed corner, at `angle` half-turns to a fixed base.
fn hand(angle: f64) -> Sketch {
    let (s, c) = (
        (angle * core::f64::consts::PI).sin() * 1000.0,
        (angle * core::f64::consts::PI).cos() * 1000.0,
    );
    sketch(&format!(
        "entity 1.0 point 0 0
entity 1.1 point 1000 0
entity 1.2 point {c} {s}
entity 2.0 line 1.0 1.1 segment
entity 2.1 line 1.0 1.2 segment
constraint 3.0 driving 0 fix 1.0 0 0
constraint 3.1 driving 0 fix 1.0 1 0
constraint 3.2 driving 0 fix 1.1 0 1000
constraint 3.3 driving 0 fix 1.1 1 0
constraint 3.4 driving 0 turned 2.0 2.1 {angle}
constraint 3.5 driving 0 length 2.1 1000
"
    ))
}

fn with_angle(from: &Sketch, angle: f64) -> Sketch {
    let text = geom_sketch::text::write(from).unwrap();
    let line = text
        .lines()
        .find(|l| l.contains("turned"))
        .unwrap()
        .to_owned();
    let head = line.rsplit_once(' ').unwrap().0;
    parse(&text.replace(&line, &format!("{head} {angle}"))).unwrap()
}

#[test]
fn a_path_ends_exactly_on_the_new_value() {
    let from = solve(&hand(0.25), &mut meter()).unwrap().sketch;
    let done = continue_to(&from, &with_angle(&from, 0.5), &mut meter()).unwrap();
    near(&done.sketch, id(1, 2), 0.0, 1000.0);
    assert!(done.report.steps >= 1);
    assert!(done.report.largest <= TOLERANCE);
    assert_eq!(
        done.sketch.constraints(),
        with_angle(&from, 0.5).constraints()
    );
}

#[test]
fn a_path_that_changes_nothing_takes_one_step_and_no_iterations() {
    let from = solve(&hand(0.25), &mut meter()).unwrap().sketch;
    let done = continue_to(&from, &from, &mut meter()).unwrap();
    assert_eq!(done.report.steps, 1);
    assert_eq!(done.report.iterations, 0);
    assert_eq!(done.sketch, from);
}

#[test]
fn a_long_jump_keeps_its_branch() {
    // In one jump, Newton's step for 0.1 to 0.9 lands near -0.1, half a turn from where it should
    // be: the residual is zero there too, and only the sense tells them apart.
    let from = solve(&hand(0.1), &mut meter()).unwrap().sketch;
    let to = with_angle(&from, 0.9);
    let jumped = solve(&to, &mut meter()).unwrap();
    let [x, y] = jumped.sketch.point(id(1, 2)).unwrap();
    let done = continue_to(&from, &to, &mut meter()).unwrap();
    assert!(done.report.steps > 1);
    let angle = 0.9 * core::f64::consts::PI;
    near(
        &done.sketch,
        id(1, 2),
        angle.cos() * 1000.0,
        angle.sin() * 1000.0,
    );
    assert!(y < 0.0, "the plain solve went to ({x}, {y})");
}

const SPREAD: &str = "entity 1.0 point 0 0
entity 1.1 point 3000 0
entity 1.2 point 1500 1322.875655532295
constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 fix 1.1 0 3000
constraint 2.3 driving 0 fix 1.1 1 0
constraint 2.4 driving 0 distance 1.0 1.2 2000
constraint 2.5 driving 0 distance 1.1 1.2 2000
";

#[test]
fn a_path_past_the_end_of_its_solutions_is_lost_where_they_end() {
    // Two sides of 2000 close to 1400 each, across a base of 3000: the apex reaches the base when
    // they are 1500, five sixths of the way, and the block's pivot falls as it gets there.
    let from = sketch(SPREAD);
    let to = sketch(&SPREAD.replace("1.2 2000", "1.2 1400"));
    let Err(SketchError::BranchLost { constraint, along }) = continue_to(&from, &to, &mut meter())
    else {
        panic!("the path has no end");
    };
    assert!(
        constraint == id(2, 4) || constraint == id(2, 5),
        "{constraint:?}"
    );
    assert!((0.5..0.84).contains(&along), "{along}");
    // Short of the end, it gets there.
    let near_end = sketch(&SPREAD.replace("1.2 2000", "1.2 1800"));
    let done = continue_to(&from, &near_end, &mut meter()).unwrap();
    let height = (1800.0_f64 * 1800.0 - 1500.0 * 1500.0).sqrt();
    near(&done.sketch, id(1, 2), 1500.0, height);
    assert_eq!(MOST_HALVINGS, 10);
}

#[test]
fn a_reference_can_move_along_a_path() {
    let lines = |x: f64| {
        format!(
            "entity 1.0 reference {x} 0
entity 1.1 point 500 400
constraint 2.0 driving 0 distance 1.0 1.1 500
constraint 2.1 driving 0 fix 1.1 1 400
"
        )
    };
    let from = sketch(&lines(200.0));
    let solved_start = solve(&from, &mut meter()).unwrap().sketch;
    near(&solved_start, id(1, 1), 500.0, 400.0);
    let done = continue_to(&solved_start, &sketch(&lines(1000.0)), &mut meter()).unwrap();
    near(&done.sketch, id(1, 1), 1300.0, 400.0);
    assert_eq!(done.sketch.point(id(1, 0)), Ok([1000.0, 0.0]));
}

#[test]
fn a_path_runs_only_between_one_sketch_and_its_values() {
    let from = sketch(TRIANGLE);
    for other in [
        // A rule more, a rule of another kind, another name, another side.
        format!("{TRIANGLE}constraint 3.6 driving 0 fix 1.2 0 3000\n"),
        TRIANGLE.replace("horizontal 2.0", "vertical 2.0"),
        TRIANGLE.replace("constraint 3.5", "constraint 3.6"),
        TRIANGLE.replace("entity 1.2 point", "entity 1.2 reference"),
    ] {
        assert_eq!(
            continue_to(&from, &sketch(&other), &mut meter()),
            Err(SketchError::NotTheSameSketch),
            "{other}"
        );
    }
    let sided = "entity 1.0 point 0 0
entity 1.1 point 10 0
entity 1.2 point 5 5
entity 2.0 line 1.0 1.1 segment
constraint 3.0 driving 0 point-line 1.2 2.0 5 left
";
    assert_eq!(
        continue_to(
            &sketch(sided),
            &sketch(&sided.replace("left", "right")),
            &mut meter()
        ),
        Err(SketchError::NotTheSameSketch)
    );
}

#[test]
fn a_block_that_starts_singular_is_not_watched() {
    // Two equal distances from a point on the line between two fixed points: its Jacobian is
    // singular as drawn, so no pivot can fall below where it began, and the path runs.
    let lines = |d: f64| {
        format!(
            "entity 1.0 point 0 0
entity 1.1 point 3000 0
entity 1.2 point 1500 0
constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 fix 1.1 0 3000
constraint 2.3 driving 0 fix 1.1 1 0
constraint 2.4 driving 0 distance 1.0 1.2 1500
constraint 2.5 driving 0 distance 1.1 1.2 {d}
"
        )
    };
    let done = continue_to(
        &sketch(&lines(1500.0)),
        &sketch(&lines(1500.0)),
        &mut meter(),
    );
    assert!(done.is_ok(), "{done:?}");
}

#[test]
fn with_values_needs_a_value_for_each_variable() {
    let s = sketch("entity 1.0 point 1 2\nentity 2.0 circle 1.0 3\n");
    let moved = s.with_values(&[4.0, 5.0, 6.0]).unwrap();
    assert_eq!(moved.point(id(1, 0)), Ok([4.0, 5.0]));
    assert_eq!(moved.radius(id(2, 0)), Ok(6.0));
    assert_eq!(s.with_values(&[4.0, 5.0]), Err(SketchError::Internal));
    assert_eq!(
        s.with_values(&[4.0, 5.0, -6.0]),
        Err(SketchError::NotPositive)
    );
}

#[test]
fn with_values_fills_every_kind_of_variable() {
    let s = sketch(
        "entity 1.0 point 0 0
entity 2.0 ellipse 1.0 3 0 1
entity 2.1 parameter 2.0 0.5 1
entity 2.2 scalar 7 1
entity 2.3 placement 1 2 3 4
entity 2.4 arc 1.0 1 1.0 1.0
",
    );
    let values: Vec<f64> = (1..=s.variables().unwrap().len())
        .map(|i| i as f64)
        .collect();
    let moved = s.with_values(&values).unwrap();
    let held: Vec<f64> = moved.variables().unwrap().iter().map(|v| v.value).collect();
    assert_eq!(held, values);
}

fn cases() -> Vec<(String, Sketch)> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/ezpz");
    let mut names: Vec<String> = fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".sketch"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let text = fs::read_to_string(folder.join(&name)).unwrap();
            (name, parse(&text).unwrap())
        })
        .collect()
}

#[test]
fn every_ezpz_problem_runs() {
    let list = cases();
    assert_eq!(list.len(), 27);
    for (name, s) in &list {
        let result = solve(s, &mut meter());
        if name == "inconsistent.sketch" {
            assert!(matches!(result, Err(SketchError::Unsolved(_))), "{name}");
        } else {
            let done = result.unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert!(done.report.largest <= TOLERANCE, "{name}");
        }
    }
}

#[test]
fn ezpz_problems_reach_the_answers_worked_by_hand() {
    let list = cases();
    let answer = |wanted: &str| {
        let (_, s) = list
            .iter()
            .find(|(name, _)| name == &format!("{wanted}.sketch"))
            .unwrap();
        solve(s, &mut meter()).unwrap().sketch
    };
    // 1024 steps to ezpz's unit.
    let u = 1024.0;
    let s = answer("midpoint");
    near(&s, id(1, 2), u, 1.5 * u);
    let s = answer("perpendicular");
    near(&s, id(1, 3), 4.0 * u, 0.0);
    let s = answer("square");
    near(&s, id(1, 1), 4.0 * u, 0.0);
    near(&s, id(1, 3), 0.0, 4.0 * u);
    let s = answer("circle_tangent");
    near(&s, id(1, 2), 2.5 * u, 1.5 * u);
    let s = answer("circle_tangent_other_dir");
    near(&s, id(1, 2), 2.5 * u, 1.5 * u);
    let s = answer("angle_parallel");
    near(&s, id(1, 1), 4.0 * u, 4.0 * u);
    near(&s, id(1, 3), 4.0 * u, 4.0 * u);
    let s = answer("symmetric");
    near(&s, id(1, 3), 0.4 * u, 0.5 * u);
    let s = answer("coincident");
    near(&s, id(1, 1), 3.0 * u, 3.0 * u);
}

#[test]
fn solved_values_are_the_same_on_every_target() {
    let bytes = {
        let mut bytes = Vec::new();
        for (name, s) in cases() {
            bytes.extend_from_slice(name.as_bytes());
            if let Ok(done) = solve(&s, &mut meter()) {
                for variable in done.sketch.variables().unwrap() {
                    bytes.extend_from_slice(&variable.value.to_bits().to_le_bytes());
                }
                bytes.extend_from_slice(&done.report.iterations.to_le_bytes());
            }
        }
        bytes
    };
    let checked = golden::check("geom-sketch/solve", || (sha256(&bytes), bytes.clone()));
    println!(
        "solve corpus: {} bytes, golden hash {}",
        checked.value.len(),
        hex(sha256(&checked.value))
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!("{e}. A changed digest is a bug, or a definition changed in place (chapter 26.2)")
    });
}
