// SPDX-License-Identifier: Apache-2.0
//! Tests of the spline rules with the solver (piece G220a; chapter 15.3, "Smoothness", and 15.2,
//! "Fit-point splines").
//!
//! The oracles are independent of the code under test: a cubic cut at its middle by de Casteljau's
//! rule joins with every order of smoothness, a perturbed copy is pulled back to it and measures
//! smooth, a circle's arc continues into a rational quarter that measures as the same circle, a
//! cusp is the join of a curve with its own reverse, and a fit-point spline's parameters at rest are
//! the chord-length parameters of its fit points, computed here.

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
    clippy::manual_midpoint,
    clippy::cast_possible_truncation,
    clippy::needless_range_loop,
    clippy::type_complexity,
    reason = "tests, written as the definitions read"
)]

use std::f64::consts::FRAC_1_SQRT_2;
use std::fmt::Write;

use geom_run::{Budget, WorkMeter};
use geom_sketch::equation::{Assembly, Sense};
use geom_sketch::fit::{MOST_ROUNDS, settle};
use geom_sketch::model::{ItemId, Smoothness};
use geom_sketch::smooth::{join_sense, measure_joins, solve_and_measure};
use geom_sketch::solve::{TOLERANCE, continue_to, solve};
use geom_sketch::text::parse;

const HEAD: &str = "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n";
const BEZIER: &str = "0 0 0 0 1 1 1 1";

fn id(sequence: u64, key: u64) -> ItemId {
    ItemId::new(sequence, key)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn point(text: &mut String, key: u64, x: f64, y: f64) {
    writeln!(text, "entity 1.{key} point {x} {y}").unwrap();
}

fn cubic(text: &mut String, index: u64, first: u64) {
    writeln!(
        text,
        "entity 2.{index} spline 3 4 1.{first} 1.{} 1.{} 1.{} 8 {BEZIER} 4 1 1 1 1",
        first + 1,
        first + 2,
        first + 3
    )
    .unwrap();
}

fn fix(text: &mut String, key: u64, point: u64, x: f64, y: f64) {
    writeln!(
        text,
        "constraint 3.{key} driving 0 fix 1.{point} 0 {x}\nconstraint 3.{} driving 0 fix 1.{point} 1 {y}",
        key + 1
    )
    .unwrap();
}

/// A cubic cut at its middle by de Casteljau's rule: the left half on points 1.0 to 1.3 and the
/// right on 1.4 to 1.7, both ends of the cut at 1.3 and 1.4.
fn halves() -> ([[f64; 2]; 4], [[f64; 2]; 4]) {
    let p = [[0.0, 0.0], [8.0, 16.0], [24.0, 20.0], [32.0, 0.0]];
    let mid = |a: [f64; 2], b: [f64; 2]| [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let (q0, q1, q2) = (mid(p[0], p[1]), mid(p[1], p[2]), mid(p[2], p[3]));
    let (r0, r1) = (mid(q0, q1), mid(q1, q2));
    let s = mid(r0, r1);
    ([p[0], q0, r0, s], [s, r1, q2, p[3]])
}

/// The two halves with the right one's inner points moved off the cut by `shift`, the left half and
/// the right half's far end held, the cut points made one and a join of `order` stated.
fn pulled(shift: f64, order: &str) -> String {
    let (l, r) = halves();
    let mut text = HEAD.to_owned();
    for (k, c) in l.iter().enumerate() {
        point(&mut text, k as u64, c[0], c[1]);
    }
    for (k, c) in r.iter().enumerate() {
        let (dx, dy) = if k == 1 || k == 2 {
            (shift, -shift)
        } else {
            (0.0, 0.0)
        };
        point(&mut text, 4 + k as u64, c[0] + dx, c[1] + dy);
    }
    cubic(&mut text, 0, 0);
    cubic(&mut text, 1, 4);
    let mut key = 0;
    for k in 0..4 {
        fix(&mut text, key, k, l[k as usize][0], l[k as usize][1]);
        key += 2;
    }
    fix(&mut text, key, 7, r[3][0], r[3][1]);
    writeln!(text, "constraint 4.0 driving 0 coincident 1.3 1.4").unwrap();
    writeln!(
        text,
        "constraint 4.1 driving 0 spline-join 2.0 finish 2.1 start {order}"
    )
    .unwrap();
    text
}

#[test]
fn a_solved_join_measures_smooth_to_every_order() {
    for (order, level) in [
        ("g1", Smoothness::Tangent),
        ("g2", Smoothness::Curvature),
        ("g3", Smoothness::CurvatureRate),
    ] {
        let s = parse(&pulled(2.0, order)).unwrap();
        let before = measure_joins(&s).unwrap();
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].order, level);
        let (solved, after) = solve_and_measure(&s, &mut meter()).unwrap();
        assert_eq!(after.len(), 1);
        let (b, a) = (before[0].continuity, after[0].continuity);
        assert!(
            a.tangent.abs() < b.tangent.abs() * 0.05,
            "{order}: {b:?} then {a:?}"
        );
        assert!(a.tangent.abs() < 2e-3, "{order}: {a:?}");
        if let (Some(kb), Some(ka)) = (b.curvature, a.curvature) {
            assert!(
                ka.abs() < kb.abs() * 0.1 + 1e-4,
                "{order}: {b:?} then {a:?}"
            );
        }
        if let (Some(rb), Some(ra)) = (b.rate, a.rate) {
            assert!(
                ra.abs() < rb.abs() * 0.1 + 1e-5,
                "{order}: {b:?} then {a:?}"
            );
        }
        assert!(solved.report.largest <= TOLERANCE, "{:?}", solved.report);
    }
}

#[test]
fn a_join_to_a_circle_and_a_line_measures_smooth_when_solved() {
    // The rational quarter circle of radius 5 about the origin from (0, 5) to (-5, 0), its start
    // pulled off the circle; the arc from (5, 0) to (0, 5); a vertical line at (-5, 0) going down.
    let w = FRAC_1_SQRT_2;
    let mut text = HEAD.to_owned();
    for (k, (x, y)) in [
        (0.0, 0.0),
        (5.0, 0.0),
        (0.0, 5.0),
        (0.3, 5.1),
        (-4.0, 5.4),
        (-5.0, 0.2),
        (-5.0, -4.0),
    ]
    .into_iter()
    .enumerate()
    {
        point(&mut text, k as u64, x, y);
    }
    writeln!(text, "entity 2.0 arc 1.0 5 1.1 1.2").unwrap();
    writeln!(
        text,
        "entity 2.1 spline 2 3 1.3 1.4 1.5 6 0 0 0 1 1 1 3 1 {w} 1"
    )
    .unwrap();
    writeln!(text, "entity 2.2 line 1.5 1.6 segment").unwrap();
    let mut key = 0;
    for (k, x, y) in [(0, 0.0, 0.0), (1, 5.0, 0.0), (2, 0.0, 5.0), (6, -5.0, -4.0)] {
        fix(&mut text, key, k, x, y);
        key += 2;
    }
    text.push_str("constraint 4.0 driving 0 coincident 1.2 1.3\n");
    text.push_str("constraint 4.1 driving 0 spline-against 2.1 start 2.0 finish g2\n");
    text.push_str("constraint 4.2 driving 0 spline-against 2.1 finish 2.2 start g1\n");
    let s = parse(&text).unwrap();
    let (solved, joins) = solve_and_measure(&s, &mut meter()).unwrap();
    assert_eq!(joins.len(), 2);
    for j in &joins {
        assert!(j.continuity.tangent.abs() < 2e-3, "{j:?}");
    }
    let against_arc = joins[0].continuity;
    assert!(
        against_arc.curvature.unwrap().abs() < 1e-3,
        "{against_arc:?}"
    );
    assert!(solved.report.largest <= TOLERANCE);
}

#[test]
fn a_join_and_its_reverse_are_told_apart_by_the_sense() {
    // The cut cubic continues; the same left half joined to its own reverse is a cusp. Both have a
    // zero G1 row.
    let s = parse(&pulled(0.0, "g1")).unwrap();
    assert_eq!(join_sense(&s, id(4, 1)).unwrap(), Some(Sense::Positive));
    let (l, _) = halves();
    let mut text = HEAD.to_owned();
    for (k, c) in l.iter().enumerate() {
        point(&mut text, k as u64, c[0], c[1]);
    }
    for (k, c) in l.iter().rev().enumerate() {
        point(&mut text, 4 + k as u64, c[0], c[1]);
    }
    cubic(&mut text, 0, 0);
    cubic(&mut text, 1, 4);
    text.push_str("constraint 4.1 driving 0 spline-join 2.0 finish 2.1 start g1\n");
    let cusp = parse(&text).unwrap();
    assert_eq!(join_sense(&cusp, id(4, 1)).unwrap(), Some(Sense::Negative));
    // Both are on the G1 row's zero set.
    for sketch in [&s, &cusp] {
        let mut a = Assembly::new(sketch).unwrap();
        a.evaluate(sketch).unwrap();
        assert!(
            a.residuals().iter().all(|r| r.abs() < 1e-9),
            "{:?}",
            a.residuals()
        );
    }
    // An id that is no rule of the sketch is unknown.
    assert!(join_sense(&s, id(9, 9)).is_err());
    // A constraint with one branch has no sense.
    assert_eq!(join_sense(&s, id(3, 0)).unwrap(), None);
}

/// Two cubics meeting at the origin, the first ending along -x and the second's free point leaving
/// by the join; the first's inner point is held at `(ax, ay)`.
fn dragged(ax: f64, ay: f64) -> String {
    let mut text = HEAD.to_owned();
    for (k, (x, y)) in [
        (-9.0, 1.0),
        (-6.0, 0.5),
        (ax, ay),
        (0.0, 0.0),
        (0.0, 0.0),
        (2.0, 0.1),
        (6.0, 4.0),
        (9.0, 1.0),
    ]
    .into_iter()
    .enumerate()
    {
        point(&mut text, k as u64, x, y);
    }
    cubic(&mut text, 0, 0);
    cubic(&mut text, 1, 4);
    let mut key = 0;
    for (k, x, y) in [
        (0, -9.0, 1.0),
        (1, -6.0, 0.5),
        (2, ax, ay),
        (3, 0.0, 0.0),
        (6, 6.0, 4.0),
        (7, 9.0, 1.0),
    ] {
        fix(&mut text, key, k, x, y);
        key += 2;
    }
    text.push_str("constraint 4.0 driving 0 coincident 1.3 1.4\n");
    text.push_str("constraint 4.1 driving 0 spline-join 2.0 finish 2.1 start g1\n");
    text
}

#[test]
fn a_path_keeps_the_continuation_and_never_ends_in_the_cusp() {
    let from = solve(&parse(&dragged(-3.0, 0.0)).unwrap(), &mut meter()).unwrap();
    assert_eq!(
        join_sense(&from.sketch, id(4, 1)).unwrap(),
        Some(Sense::Positive)
    );
    // The first curve's inner point is dragged a third of a turn about the join, so the second
    // curve's free point has to swing with it.
    let to = parse(&dragged(-1.5, -2.598)).unwrap();
    let reached = continue_to(&from.sketch, &to, &mut meter()).unwrap();
    assert_eq!(
        join_sense(&reached.sketch, id(4, 1)).unwrap(),
        Some(Sense::Positive)
    );
    // The free point is across the join from the first's inner point: on the line through both.
    let [fx, fy] = reached.sketch.point(id(1, 5)).unwrap();
    assert!(fx > 0.0 && fy > 0.0, "({fx}, {fy})");
    let cross = fx * -2.598 - fy * -1.5;
    assert!(cross.abs() < 0.2, "{cross}");
    let joins = measure_joins(&reached.sketch).unwrap();
    assert!(joins[0].continuity.tangent.abs() < 5e-3, "{joins:?}");
}

/// Fit points on a gentle curve, a fit-point spline through them by `author`, and the sketch that
/// holds the end fit points and the middle one at `(mx, my)`.
fn fitted(mx: f64, my: f64) -> (geom_sketch::model::Sketch, ItemId) {
    let data = [
        [0.0, 0.0],
        [10.0, 6.0],
        [22.0, 9.0],
        [31.0, 7.0],
        [40.0, 1.0],
        [52.0, -3.0],
    ];
    let mut text = HEAD.to_owned();
    for (k, p) in data.iter().enumerate() {
        point(&mut text, k as u64, p[0], p[1]);
    }
    let mut sketch = parse(&text).unwrap();
    let fit_points: Vec<ItemId> = (0..6).map(|k| id(1, k)).collect();
    let made = geom_sketch::fit::author(&sketch, &fit_points, 3, id(2, 0), 1, 100).unwrap();
    for e in made.entities {
        sketch.add_entity(e).unwrap();
    }
    for c in made.constraints {
        sketch.add_constraint(c).unwrap();
    }
    let mut holds = String::new();
    let mut key = 0;
    for (k, x, y) in [(0, 0.0, 0.0), (2, mx, my), (5, 52.0, -3.0)] {
        fix(&mut holds, key, k, x, y);
        key += 2;
    }
    let mut full = geom_sketch::text::write(&sketch).unwrap();
    full.push_str(&holds);
    (parse(&full).unwrap(), id(2, 0))
}

fn chord_parameters(points: &[[f64; 2]]) -> Vec<f64> {
    let mut sum = vec![0.0];
    for w in points.windows(2) {
        let last = *sum.last().unwrap();
        sum.push(last + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
    }
    let total = *sum.last().unwrap();
    sum.iter().map(|s| s / total).collect()
}

#[test]
fn a_dragged_fit_point_is_reinterpolated_until_it_settles() {
    let (sketch, spline) = fitted(26.0, 16.0);
    let settled = settle(&sketch, spline, &mut meter()).unwrap();
    assert!(settled.settled, "{settled:?}");
    assert!(settled.rounds >= 2, "{}", settled.rounds);
    assert!(settled.rounds <= MOST_ROUNDS);
    assert!(settled.moved < TOLERANCE, "{}", settled.moved);
    let s = &settled.solved.sketch;
    // The fit points pass through the curve: each through-constraint is met.
    let mut a = Assembly::new(s).unwrap();
    a.evaluate(s).unwrap();
    assert!(
        a.residuals().iter().all(|r| r.abs() <= TOLERANCE),
        "{:?}",
        a.residuals()
    );
    // The middle fit point went where it was held.
    let fits: Vec<[f64; 2]> = (0..6).map(|k| s.point(id(1, k)).unwrap()).collect();
    assert!(
        (fits[2][0] - 26.0).abs() < 2.0 * TOLERANCE && (fits[2][1] - 16.0).abs() < 2.0 * TOLERANCE
    );
    // The stored parametrisation is the chord-length one of the fit points as they ended, to the
    // distance the settling allows.
    let want = chord_parameters(&fits);
    for c in s.constraints() {
        if let geom_sketch::model::ConstraintKind::SplineThrough { point, at, .. } = c.kind {
            let i = (point.key) as usize;
            assert!(
                (at - want[i]).abs() < 2e-3,
                "fit point {i}: {at} against {}",
                want[i]
            );
        }
    }
}

#[test]
fn a_fit_point_spline_at_rest_settles_at_once() {
    let (sketch, spline) = fitted(22.0, 9.0);
    let settled = settle(&sketch, spline, &mut meter()).unwrap();
    assert!(settled.settled);
    assert_eq!(settled.rounds, 1);
    assert!(settled.moved < TOLERANCE);
}

/// Two quarter circles of radius 5 about the origin as rational quadratics, the arc of the first,
/// a plain quadratic and cubic, and lines up and down at (5, 0).
fn circle_scene() -> String {
    let w = FRAC_1_SQRT_2;
    let mut text = HEAD.to_owned();
    for (k, (x, y)) in [
        (5.0, 0.0),
        (5.0, 5.0),
        (0.0, 5.0),
        (0.0, 5.0),
        (-5.0, 5.0),
        (-5.0, 0.0),
        (0.0, 0.0),
        (5.0, -3.0),
        (5.0, 9.0),
    ]
    .into_iter()
    .enumerate()
    {
        point(&mut text, k as u64, x, y);
    }
    writeln!(
        text,
        "entity 2.0 spline 2 3 1.0 1.1 1.2 6 0 0 0 1 1 1 3 1 {w} 1"
    )
    .unwrap();
    writeln!(
        text,
        "entity 2.1 spline 2 3 1.3 1.4 1.5 6 0 0 0 1 1 1 3 1 {w} 1"
    )
    .unwrap();
    writeln!(text, "entity 2.2 arc 1.6 5 1.0 1.2").unwrap();
    writeln!(text, "entity 2.3 line 1.7 1.8 segment").unwrap();
    writeln!(text, "entity 2.4 line 1.8 1.7 segment").unwrap();
    // The first quarter run backwards.
    writeln!(
        text,
        "entity 2.5 spline 2 3 1.2 1.1 1.0 6 0 0 0 1 1 1 3 1 {w} 1"
    )
    .unwrap();
    text
}

#[test]
fn every_kind_of_spline_join_has_a_sense() {
    let scene = circle_scene();
    let sense = |rule: &str| {
        let s = parse(&format!("{scene}constraint 3.0 driving 0 {rule}\n")).unwrap();
        join_sense(&s, id(3, 0)).unwrap()
    };
    // The two quarters continue into each other, at every order.
    for order in ["g1", "g2", "g3"] {
        assert_eq!(
            sense(&format!("spline-join 2.0 finish 2.1 start {order}")),
            Some(Sense::Positive),
            "rational {order}"
        );
        assert_eq!(
            sense(&format!("spline-join 2.0 finish 2.5 start {order}")),
            Some(Sense::Negative),
            "rational {order}, a cusp"
        );
    }
    // The first quarter leaves (5, 0) upward: along the line going up, against the one going down.
    assert_eq!(
        sense("spline-tangent-line 2.0 start 2.3"),
        Some(Sense::Positive)
    );
    assert_eq!(
        sense("spline-tangent-line 2.0 start 2.4"),
        Some(Sense::Negative)
    );
    for order in ["g1", "g2", "g3"] {
        assert_eq!(
            sense(&format!("spline-against 2.0 start 2.3 start {order}")),
            Some(Sense::Positive)
        );
        assert_eq!(
            sense(&format!("spline-against 2.0 start 2.4 start {order}")),
            Some(Sense::Negative)
        );
        // The second quarter leaves (0, 5) anticlockwise about the origin; the first, from its
        // finish, clockwise.
        assert_eq!(
            sense(&format!("spline-against 2.1 start 2.2 finish {order}")),
            Some(Sense::Positive)
        );
        assert_eq!(
            sense(&format!("spline-against 2.0 finish 2.2 finish {order}")),
            Some(Sense::Negative)
        );
    }
}

#[test]
fn the_plain_layout_rules_have_a_sense_too() {
    // Plain cubics: the cut cubic at every order, and a tangent line.
    let (l, _) = halves();
    for order in ["g1", "g2", "g3"] {
        let s = parse(&pulled(0.0, order)).unwrap();
        assert_eq!(
            join_sense(&s, id(4, 1)).unwrap(),
            Some(Sense::Positive),
            "{order}"
        );
    }
    let mut text = pulled(0.0, "g1");
    writeln!(
        text,
        "entity 1.20 point {} {}\nentity 1.21 point {} {}\nentity 2.5 line 1.20 1.21 segment\n",
        l[3][0],
        l[3][1],
        l[3][0] - 3.0,
        l[3][1] + 1.0
    )
    .unwrap();
    text.push_str("constraint 4.2 driving 0 spline-tangent-line 2.0 finish 2.5\n");
    let s = parse(&text).unwrap();
    // The cubic leaves its finish towards its inner point, to the left and up: along the line.
    assert!(join_sense(&s, id(4, 2)).unwrap().is_some());
}

#[test]
fn the_measure_reads_a_tangent_line_and_both_ends_of_an_arc() {
    let scene = circle_scene();
    let mut text = scene.clone();
    // The first quarter leaves (5, 0) upward along the vertical line; its finish at (0, 5) runs
    // into the arc's finish; the second quarter leaves that point anticlockwise from the arc's.
    text.push_str("constraint 3.0 driving 0 spline-tangent-line 2.0 start 2.3\n");
    text.push_str("constraint 3.1 driving 0 spline-against 2.0 finish 2.2 finish g3\n");
    text.push_str("constraint 3.2 driving 0 spline-against 2.1 start 2.2 finish g3\n");
    text.push_str("constraint 3.3 driving 0 spline-against 2.0 start 2.2 start g3\n");
    let s = parse(&text).unwrap();
    let joins = measure_joins(&s).unwrap();
    assert_eq!(joins.len(), 4);
    assert_eq!(joins[0].order, Smoothness::Tangent);
    // The quarter circle and the arc are one circle: every gap is rounding, whichever way the pair
    // runs, except where both leave the same way round (the curvatures then add).
    assert!(joins[0].continuity.tangent.abs() < 1e-12, "{:?}", joins[0]);
    assert!(
        joins[2].continuity.curvature.unwrap().abs() < 1e-12,
        "{:?}",
        joins[2]
    );
    assert!(
        joins[2].continuity.rate.unwrap().abs() < 1e-10,
        "{:?}",
        joins[2]
    );
    // The first quarter and the arc leave their finishes the same way: against.
    assert!(
        joins[1].continuity.curvature.unwrap().abs() > 0.1,
        "{:?}",
        joins[1]
    );
    assert!(
        joins[3].continuity.curvature.unwrap().abs() > 0.1,
        "{:?}",
        joins[3]
    );
}

#[test]
fn a_line_at_any_slope_is_taken_against_the_spline_whichever_way_it_runs() {
    // The cut cubic's left half leaves (0, 0) towards (8, 16), the direction (1, 2).
    for (a, b) in [
        ("0 0", "3 6"),
        ("3 6", "0 0"),
        ("-1 -2", "5 10"),
        ("5 10", "-1 -2"),
    ] {
        let mut text = pulled(0.0, "g1");
        let (a, b) = (a.to_owned(), b.to_owned());
        writeln!(
            text,
            "entity 1.20 point {a}\nentity 1.21 point {b}\nentity 2.5 line 1.20 1.21 segment\nconstraint 4.2 driving 0 spline-tangent-line 2.0 start 2.5\nconstraint 4.3 driving 0 spline-against 2.0 start 2.5 start g2"
        )
        .unwrap();
        let joins = measure_joins(&parse(&text).unwrap()).unwrap();
        for j in joins
            .iter()
            .filter(|j| j.constraint.sequence == 4 && j.constraint.key >= 2)
        {
            assert!(j.continuity.tangent.abs() < 1e-12, "{a} to {b}: {j:?}");
        }
    }
}

#[test]
fn a_settled_sketch_reads_back_whatever_its_ids() {
    // Control points authored in a sequence above the spline's: the text writes them first.
    let data = [
        [0.0, 0.0],
        [10.0, 6.0],
        [22.0, 9.0],
        [31.0, 7.0],
        [40.0, 1.0],
        [52.0, -3.0],
    ];
    let mut text = HEAD.to_owned();
    for (k, p) in data.iter().enumerate() {
        point(&mut text, k as u64, p[0], p[1]);
    }
    let mut sketch = parse(&text).unwrap();
    let fit_points: Vec<ItemId> = (0..6).map(|k| id(1, k)).collect();
    let made = geom_sketch::fit::author(&sketch, &fit_points, 3, id(2, 0), 4, 0).unwrap();
    for e in made.entities {
        sketch.add_entity(e).unwrap();
    }
    for c in made.constraints {
        sketch.add_constraint(c).unwrap();
    }
    let written = geom_sketch::text::write(&sketch).unwrap();
    assert_eq!(parse(&written).unwrap(), sketch);
    let spline_at = written.find("entity 2.0 spline").unwrap();
    let last_point = written.find("entity 4.5 construction point").unwrap();
    assert!(
        last_point < spline_at,
        "the points are written before the spline"
    );
    let mut held = String::new();
    for (key, (k, x, y)) in [(0, 0.0, 0.0), (2, 26.0, 16.0), (5, 52.0, -3.0)]
        .into_iter()
        .enumerate()
    {
        fix(&mut held, 2 * key as u64, k, x, y);
    }
    let both = parse(&format!("{written}{held}")).unwrap();
    let settled = settle(&both, id(2, 0), &mut meter()).unwrap();
    assert!(settled.settled);
    let back = parse(&geom_sketch::text::write(&settled.solved.sketch).unwrap()).unwrap();
    assert_eq!(back, settled.solved.sketch);
}

#[test]
fn a_line_at_the_other_end_and_any_slope_is_taken_against_the_spline() {
    // The cut cubic's left half arrives at (16, 13.5) from the left; the finish leaves it back along
    // (-6, -0.5). Lines with a slope that tells a dot product from a sum or a quotient of terms.
    for (a, b) in [
        ("16 13.5", "20 12.5"),
        ("20 12.5", "16 13.5"),
        ("16 13.5", "12 14.5"),
        ("12 14.5", "16 13.5"),
        ("16 13.5", "15.5 23.5"),
        ("15.5 23.5", "16 13.5"),
    ] {
        let mut text = pulled(0.0, "g1");
        writeln!(
            text,
            "entity 1.20 point {a}\nentity 1.21 point {b}\nentity 2.5 line 1.20 1.21 segment\nconstraint 4.2 driving 0 spline-tangent-line 2.0 finish 2.5"
        )
        .unwrap();
        let joins = measure_joins(&parse(&text).unwrap()).unwrap();
        let last = joins.last().unwrap();
        // The line is not along the spline, so the angle is not zero, but it is the one of at most a
        // quarter turn (half a half-turn) whichever way the line runs: a line the wrong way round
        // would measure more.
        assert!(
            last.continuity.tangent.abs() <= 0.5 + 1e-9,
            "{a} to {b}: {last:?}"
        );
    }
}

#[test]
fn entities_written_after_the_ones_they_name_whatever_their_ids() {
    use geom_sketch::model::{Constraint, ConstraintKind, Entity, EntityKind, Extent, Standing};
    let mut sketch = parse(HEAD).unwrap();
    let add = |s: &mut geom_sketch::model::Sketch, id: ItemId, kind: EntityKind| {
        s.add_entity(Entity {
            id,
            kind,
            construction: false,
        })
        .unwrap();
    };
    // Points with the highest ids, and everything that names them below.
    for k in 0..3 {
        add(
            &mut sketch,
            id(9, k),
            EntityKind::Point {
                x: f64::from(k as u32),
                y: 1.0,
            },
        );
    }
    add(
        &mut sketch,
        id(1, 0),
        EntityKind::Line {
            start: id(9, 0),
            end: id(9, 1),
            extent: Extent::Segment,
        },
    );
    add(
        &mut sketch,
        id(1, 1),
        EntityKind::Circle {
            centre: id(9, 2),
            radius: 2.0,
        },
    );
    add(
        &mut sketch,
        id(1, 2),
        EntityKind::Ellipse {
            centre: id(9, 2),
            axis: [3.0, 0.0],
            minor: 1.0,
        },
    );
    add(
        &mut sketch,
        id(1, 3),
        EntityKind::Arc {
            centre: id(9, 2),
            radius: 2.0,
            start: id(9, 0),
            end: id(9, 1),
        },
    );
    add(
        &mut sketch,
        id(1, 4),
        EntityKind::Parameter {
            curve: id(1, 3),
            t: 0.5,
            speed: 1.0,
        },
    );
    sketch
        .add_constraint(Constraint {
            id: id(3, 0),
            kind: ConstraintKind::Coincident {
                a: id(9, 0),
                b: id(9, 1),
            },
            standing: Standing::Driving,
            priority: 0,
        })
        .unwrap();
    let written = geom_sketch::text::write(&sketch).unwrap();
    assert_eq!(parse(&written).unwrap(), sketch);
    // The parameter came after the arc it names, the arc after the points.
    let at = |needle: &str| written.find(needle).unwrap();
    assert!(at("entity 9.2 point") < at("entity 1.0 line"));
    assert!(at("entity 9.2 point") < at("entity 1.1 circle"));
    assert!(at("entity 9.2 point") < at("entity 1.2 ellipse"));
    assert!(at("entity 1.3 arc") < at("entity 1.4 parameter"));
}
