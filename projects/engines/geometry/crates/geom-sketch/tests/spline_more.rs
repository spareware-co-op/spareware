// SPDX-License-Identifier: Apache-2.0
//! Tests of rational splines, splines against lines and arcs, spline arc length, a point pinned to
//! a spline and fit-point splines (piece G220; chapter 15.3).
//!
//! The oracles are independent of the code under test: a rational quadratic Bézier with weights
//! 1, √2/2, 1 is an exact quarter circle, so its curvature is one over the radius, two quarters
//! join a circle's arc with every order of smoothness, and its length is r·π/2; the parabola
//! y = x² has a closed-form arc length; a straight cubic has the length of its chord; every
//! Jacobian entry is a central difference of its residual.

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
    clippy::needless_range_loop,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    clippy::cast_possible_truncation,
    reason = "tests, written as the definitions read"
)]

use std::f64::consts::{FRAC_1_SQRT_2, PI};
use std::fmt::Write;

use geom_sketch::equation::Assembly;
use geom_sketch::fit;
use geom_sketch::length::spline_length;
use geom_sketch::model::{ItemId, Sketch, SketchError, Smoothness, SplineEnd};
use geom_sketch::smooth::{continuity, curvature_of, end_jets};
use geom_sketch::text::{parse, write};

const HEAD: &str = "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n";
const QUADRATIC: &str = "0 0 0 1 1 1";

fn id(sequence: u64, key: u64) -> ItemId {
    ItemId::new(sequence, key)
}

fn point(text: &mut String, key: u64, x: f64, y: f64) {
    writeln!(text, "entity 1.{key} point {x} {y}").unwrap();
}

/// A spline entity `2.index` of `degree` on the points 1.first.. with `knots` (a text list) and
/// `weights`.
fn spline(text: &mut String, index: u64, degree: u32, first: u64, weights: &[f64], knots: &str) {
    let n = weights.len();
    write!(text, "entity 2.{index} spline {degree} {n}").unwrap();
    for k in 0..n {
        write!(text, " 1.{}", first + k as u64).unwrap();
    }
    write!(text, " {} {knots} {n}", knots.split(' ').count()).unwrap();
    for w in weights {
        write!(text, " {w}").unwrap();
    }
    text.push('\n');
}

/// Two quarter circles of radius 5 about the origin, as rational quadratics: spline 2.0 from (5, 0)
/// to (0, 5) and spline 2.1 on from there to (-5, 0); the arc 2.2 of the first quarter, on centre
/// 1.6.
fn quarters() -> String {
    let w = [1.0, FRAC_1_SQRT_2, 1.0];
    let mut text = HEAD.to_owned();
    for (k, (x, y)) in [
        (5.0, 0.0),
        (5.0, 5.0),
        (0.0, 5.0),
        (0.0, 5.0),
        (-5.0, 5.0),
        (-5.0, 0.0),
        (0.0, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        point(&mut text, k as u64, x, y);
    }
    spline(&mut text, 0, 2, 0, &w, QUADRATIC);
    spline(&mut text, 1, 2, 3, &w, QUADRATIC);
    writeln!(text, "entity 2.2 arc 1.6 5 1.0 1.2").unwrap();
    text
}

fn with_rule(text: &str, rule: &str) -> Sketch {
    parse(&format!("{text}constraint 3.0 driving 0 {rule}\n")).unwrap()
}

/// The residuals of the last `rows` equations, which are the rule's (an arc adds two before them).
fn tail(s: &Sketch, rows: usize) -> Vec<f64> {
    let mut a = Assembly::new(s).unwrap();
    a.evaluate(s).unwrap();
    let r = a.residuals();
    r[r.len() - rows..].to_vec()
}

#[test]
fn a_rational_quarter_circle_has_the_curvature_of_its_circle() {
    let s = parse(&quarters()).unwrap();
    for end in [SplineEnd::Start, SplineEnd::Finish] {
        let j = end_jets(&s, id(2, 0), end, 3).unwrap();
        let k = curvature_of(j.first, j.second).unwrap();
        // Outward from the end into the spline: counterclockwise from the start, clockwise from the
        // finish.
        let want = if end == SplineEnd::Start { 0.2 } else { -0.2 };
        assert!((k - want).abs() < 1e-12, "{end:?}: {k}");
    }
    // The first derivative at the start: p w1 / w0 (P1 - P0) over the knot span.
    let j = end_jets(&s, id(2, 0), SplineEnd::Start, 1).unwrap();
    assert!(j.first[0].abs() < 1e-12);
    assert!((j.first[1] - 2.0 * FRAC_1_SQRT_2 * 5.0).abs() < 1e-12);
}

#[test]
fn two_rational_quarters_join_with_every_order_and_match_their_arc() {
    let text = quarters();
    for (order, rows) in [("g1", 1), ("g2", 2), ("g3", 3)] {
        let s = with_rule(&text, &format!("spline-join 2.0 finish 2.1 start {order}"));
        for (i, x) in tail(&s, rows).iter().enumerate() {
            assert!(x.abs() < 1e-9, "{order} row {i}: {x}");
        }
        let s = with_rule(
            &text,
            &format!("spline-against 2.1 start 2.2 finish {order}"),
        );
        for (i, x) in tail(&s, rows).iter().enumerate() {
            assert!(x.abs() < 1e-9, "against an arc, {order} row {i}: {x}");
        }
    }
    // The measure agrees.
    let s = parse(&text).unwrap();
    let c = continuity(
        &s,
        (id(2, 0), SplineEnd::Finish),
        (id(2, 1), SplineEnd::Start),
        Smoothness::CurvatureRate,
    )
    .unwrap();
    assert!(
        c.tangent.abs() < 1e-12 && c.curvature.unwrap().abs() < 1e-12,
        "{c:?}"
    );
    assert!(c.rate.unwrap().abs() < 1e-10, "{c:?}");
}

#[test]
fn an_arc_laid_over_its_spline_is_tangent_but_not_a_continuation() {
    // Both run counterclockwise from (5, 0): the same side, so the curvatures add instead of
    // cancelling.
    let s = with_rule(&quarters(), "spline-against 2.0 start 2.2 start g2");
    let r = tail(&s, 2);
    assert!(r[0].abs() < 1e-9, "{r:?}");
    assert!(r[1].abs() > 1e-3, "{r:?}");
}

#[test]
fn rational_splines_that_bend_differently_do_not_join_at_g2() {
    let mut text = HEAD.to_owned();
    let w = [1.0, FRAC_1_SQRT_2, 1.0];
    for (k, (x, y)) in [
        (5.0, 0.0),
        (5.0, 5.0),
        (0.0, 5.0),
        (0.0, 5.0),
        (-10.0, 5.0),
        (-10.0, -5.0),
    ]
    .into_iter()
    .enumerate()
    {
        point(&mut text, k as u64, x, y);
    }
    spline(&mut text, 0, 2, 0, &w, QUADRATIC);
    spline(&mut text, 1, 2, 3, &w, QUADRATIC);
    let r = tail(&with_rule(&text, "spline-join 2.0 finish 2.1 start g2"), 2);
    assert!(r[0].abs() < 1e-9, "tangent: {r:?}");
    assert!(r[1].abs() > 1e-3, "curvature: {r:?}");
}

#[test]
fn a_cubic_leaves_a_line_with_the_curvature_of_a_line_and_not_its_rate() {
    // The first three control points are on the x axis: the end has no curvature, and the rate of
    // change is (d1 x d3) / |d1|^4 = 18 / 81 by hand.
    let mut text = HEAD.to_owned();
    for (k, (x, y)) in [
        (0.0, 0.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (3.0, 1.0),
        (-4.0, 0.0),
        (9.0, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        point(&mut text, k as u64, x, y);
    }
    spline(&mut text, 0, 3, 0, &[1.0; 4], "0 0 0 0 1 1 1 1");
    writeln!(text, "entity 2.1 line 1.4 1.5 segment").unwrap();
    for (order, rows) in [("g1", 1), ("g2", 2)] {
        let r = tail(
            &with_rule(
                &text,
                &format!("spline-against 2.0 start 2.1 start {order}"),
            ),
            rows,
        );
        for x in r {
            assert!(x.abs() < 1e-12, "{order}: {x}");
        }
    }
    let r = tail(
        &with_rule(&text, "spline-against 2.0 start 2.1 start g3"),
        3,
    );
    assert!(r[0].abs() < 1e-12 && r[1].abs() < 1e-12, "{r:?}");
    assert!(r[2].abs() > 1e-4, "{r:?}");
    let j = end_jets(&parse(&text).unwrap(), id(2, 0), SplineEnd::Start, 3).unwrap();
    let rate = geom_sketch::smooth::curvature_rate_of(j.first, j.second, j.third).unwrap();
    assert!((rate - 18.0 / 81.0).abs() < 1e-12, "{rate}");
}

#[test]
fn the_length_of_a_quarter_circle_is_a_quarter_of_its_circumference() {
    let s = parse(&quarters()).unwrap();
    let l = spline_length(&s, id(2, 0)).unwrap();
    let exact = 5.0 * PI / 2.0;
    assert!((l.value - exact).abs() <= l.error, "{l:?} against {exact}");
    assert!(l.error < 1e-9, "{l:?}");
}

#[test]
fn the_length_of_a_parabola_and_of_a_straight_cubic_are_their_closed_forms() {
    let mut text = HEAD.to_owned();
    for (k, (x, y)) in [
        (0.0, 0.0),
        (0.5, 0.0),
        (1.0, 1.0),
        (0.0, 0.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (3.0, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        point(&mut text, k as u64, x, y);
    }
    spline(&mut text, 0, 2, 0, &[1.0; 3], QUADRATIC);
    spline(&mut text, 1, 3, 3, &[1.0; 4], "0 0 0 0 1 1 1 1");
    let s = parse(&text).unwrap();
    let parabola = spline_length(&s, id(2, 0)).unwrap();
    let exact = (2.0 * 5.0_f64.sqrt() + 2.0_f64.asinh()) / 4.0;
    assert!(
        (parabola.value - exact).abs() <= parabola.error,
        "{parabola:?} against {exact}"
    );
    assert!(parabola.error < 1e-9, "{parabola:?}");
    let straight = spline_length(&s, id(2, 1)).unwrap();
    assert!(
        (straight.value - 3.0).abs() <= straight.error,
        "{straight:?}"
    );
    assert!(straight.error < 1e-9, "{straight:?}");
}

#[test]
fn a_spline_with_a_stopped_point_has_no_provable_length() {
    let mut text = HEAD.to_owned();
    for (k, (x, y)) in [(0.0, 0.0), (0.0, 0.0), (1.0, 1.0), (2.0, 0.0)]
        .into_iter()
        .enumerate()
    {
        point(&mut text, k as u64, x, y);
    }
    spline(&mut text, 0, 3, 0, &[1.0; 4], "0 0 0 0 1 1 1 1");
    let s = parse(&text).unwrap();
    assert_eq!(
        spline_length(&s, id(2, 0)),
        Err(SketchError::Degenerate(id(2, 0)))
    );
}

#[test]
fn a_length_rule_reads_the_error_of_the_stated_length() {
    let exact = 5.0 * PI / 2.0;
    let s = with_rule(&quarters(), &format!("spline-length 2.0 {exact}"));
    let r = tail(&s, 1);
    assert!(r[0].abs() < 1e-9, "{r:?}");
    let s = with_rule(&quarters(), "spline-length 2.0 8");
    let r = tail(&s, 1);
    assert!((r[0] - (exact - 8.0)).abs() < 1e-9, "{r:?}");
}

#[test]
fn a_point_pinned_to_a_spline_reads_the_miss() {
    // The cubic (0,0) (8,16) (24,20) (32,0) at 1/2 is (8 + 3*16 + 3*24 + 32 ... ) by Bernstein:
    // (P0 + 3 P1 + 3 P2 + P3) / 8 = (0 + 24 + 72 + 32, 0 + 48 + 60 + 0) / 8 = (16, 13.5).
    let mut text = HEAD.to_owned();
    for (k, (x, y)) in [
        (0.0, 0.0),
        (8.0, 16.0),
        (24.0, 20.0),
        (32.0, 0.0),
        (16.0, 13.5),
        (17.0, 12.0),
    ]
    .into_iter()
    .enumerate()
    {
        point(&mut text, k as u64, x, y);
    }
    spline(&mut text, 0, 3, 0, &[1.0; 4], "0 0 0 0 1 1 1 1");
    let r = tail(&with_rule(&text, "spline-through 2.0 1.4 0.5"), 2);
    assert!(r[0].abs() < 1e-12 && r[1].abs() < 1e-12, "{r:?}");
    let r = tail(&with_rule(&text, "spline-through 2.0 1.5 0.5"), 2);
    assert!(
        (r[0] + 1.0).abs() < 1e-12 && (r[1] - 1.5).abs() < 1e-12,
        "{r:?}"
    );
    // Outside the domain, the rule is refused.
    assert!(
        parse(&format!(
            "{text}constraint 3.0 driving 0 spline-through 2.0 1.4 1.5\n"
        ))
        .is_err()
    );
}

#[test]
fn every_jacobian_entry_is_the_derivative_of_its_residual() {
    let mut text = quarters();
    text.push_str("entity 1.7 point 0 0\nentity 1.8 point 6 1\nentity 2.3 line 1.7 1.8 segment\n");
    for rule in [
        "spline-join 2.0 finish 2.1 start g1",
        "spline-join 2.0 finish 2.1 start g2",
        "spline-join 2.0 finish 2.1 start g3",
        "spline-tangent-line 2.0 start 2.3",
        "spline-against 2.0 finish 2.2 finish g1",
        "spline-against 2.1 start 2.2 finish g2",
        "spline-against 2.1 start 2.2 finish g3",
        "spline-against 2.1 finish 2.3 start g1",
        "spline-against 2.1 finish 2.3 start g2",
        "spline-against 2.1 finish 2.3 start g3",
        "spline-length 2.1 7",
        "spline-through 2.0 1.7 0.3",
    ] {
        let s = with_rule(&text, rule);
        let mut a = Assembly::new(&s).unwrap();
        let mut x = a.start().unwrap();
        let mut state = 0x0220_u64;
        for value in &mut x {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            *value += ((state >> 40) as f64 / (1_u64 << 24) as f64 - 0.5) * 0.5;
        }
        a.rescale(&x).unwrap();
        a.evaluate_at(&x).unwrap();
        let jacobian = a.jacobian().to_dense().unwrap();
        let rows = a.residuals().len();
        let h = 1e-6;
        for j in 0..x.len() {
            let scale = a.variables()[j].scale;
            let mut at = |value: f64| {
                let mut y = x.clone();
                y[j] = value;
                a.evaluate_at(&y).unwrap();
                a.residuals().to_vec()
            };
            let (up, down) = (at(x[j] + h), at(x[j] - h));
            for i in 0..rows {
                let numeric = (up[i] - down[i]) / (2.0 * h) * scale;
                let exact = jacobian.get(i, j).unwrap();
                assert!(
                    (numeric - exact).abs() < 1e-5 * (1.0 + exact.abs()),
                    "{rule}: row {i}, variable {j}: {numeric} and {exact}"
                );
            }
        }
    }
}

#[test]
fn the_new_rules_round_trip_in_text() {
    let mut text = quarters();
    for rule in [
        "spline-against 2.0 finish 2.2 finish g3",
        "spline-length 2.0 7.5",
        "spline-through 2.0 1.6 0.25",
    ] {
        let key = text.len();
        writeln!(text, "constraint 3.{key} driving 0 {rule}").unwrap();
    }
    let s = parse(&text).unwrap();
    assert_eq!(parse(&write(&s).unwrap()).unwrap(), s);
}

#[test]
fn a_fit_point_spline_passes_through_its_fit_points() {
    let data = [
        [0.0, 0.0],
        [1.0, 2.0],
        [3.0, 3.0],
        [4.0, 1.0],
        [6.0, 0.0],
        [7.0, 2.5],
    ];
    let mut s = parse(HEAD).unwrap();
    let mut fit_points = Vec::new();
    for (k, p) in data.iter().enumerate() {
        let text = format!("entity 1.{k} point {} {}\n", p[0], p[1]);
        let one = parse(&format!("{HEAD}{text}")).unwrap();
        let entity = one.entities()[0].clone();
        s.add_entity(entity).unwrap();
        fit_points.push(id(1, k as u64));
    }
    let made = fit::author(&s, &fit_points, 3, id(2, 0), 4, 0).unwrap();
    assert!(made.fit.error < 1e-9, "{}", made.fit.error);
    assert_eq!(made.constraints.len(), data.len());
    for e in made.entities.clone() {
        s.add_entity(e).unwrap();
    }
    for c in made.constraints.clone() {
        s.add_constraint(c).unwrap();
    }
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate(&s).unwrap();
    for (i, r) in a.residuals().iter().enumerate() {
        assert!(r.abs() < 1e-9, "row {i}: {r}");
    }
    // Nothing moved: re-interpolating is a fixed point.
    let again = fit::reinterpolate(&s, id(2, 0)).unwrap();
    assert!(again.control_moved < 1e-9, "{}", again.control_moved);
    assert!(again.parameter_moved < 1e-12, "{}", again.parameter_moved);
    assert_eq!(again.fit_points, fit_points);
}

#[test]
fn moving_a_fit_point_moves_the_reinterpolation() {
    let data = [[0.0, 0.0], [1.0, 2.0], [3.0, 3.0], [4.0, 1.0], [6.0, 0.0]];
    let mut text = HEAD.to_owned();
    for (k, p) in data.iter().enumerate() {
        point(&mut text, k as u64, p[0], p[1]);
    }
    let s = parse(&text).unwrap();
    let ids: Vec<ItemId> = (0..data.len() as u64).map(|k| id(1, k)).collect();
    let made = fit::author(&s, &ids, 3, id(2, 0), 4, 0).unwrap();
    let mut moved_text = HEAD.to_owned();
    for (k, p) in data.iter().enumerate() {
        point(
            &mut moved_text,
            k as u64,
            p[0],
            p[1] + if k == 2 { 1.0 } else { 0.0 },
        );
    }
    let mut s = parse(&moved_text).unwrap();
    for e in made.entities {
        s.add_entity(e).unwrap();
    }
    for c in made.constraints {
        s.add_constraint(c).unwrap();
    }
    let r = fit::reinterpolate(&s, id(2, 0)).unwrap();
    assert!(r.control_moved > 0.1, "{}", r.control_moved);
    assert!(r.parameter_moved > 0.0);
    assert!(r.fit.error < 1e-9);
}

#[test]
fn fit_data_the_interpolation_refuses_is_refused() {
    let s = parse(&format!(
        "{HEAD}entity 1.0 point 0 0\nentity 1.1 point 1 1\n"
    ))
    .unwrap();
    assert!(fit::author(&s, &[id(1, 0), id(1, 1)], 3, id(2, 0), 4, 0).is_err());
    assert!(fit::author(&s, &[id(1, 0), id(1, 9)], 1, id(2, 0), 4, 0).is_err());
}

#[test]
fn the_new_rules_and_helpers_refuse_what_is_not_a_spline_or_not_a_curve() {
    let base = quarters();
    let s = parse(&base).unwrap();
    // A point is no spline.
    let point = id(1, 0);
    assert!(matches!(
        s.spline_is_rational(point),
        Err(SketchError::WrongKind(_))
    ));
    assert!(matches!(
        s.spline_curve(point),
        Err(SketchError::WrongKind(_))
    ));
    assert!(matches!(
        spline_length(&s, point),
        Err(SketchError::WrongKind(_))
    ));
    assert!(matches!(
        end_jets(&s, point, SplineEnd::Start, 1),
        Err(SketchError::WrongKind(_))
    ));
    // Orders the jets do not give.
    for level in [0, 4] {
        assert!(matches!(
            end_jets(&s, id(2, 0), SplineEnd::Start, level),
            Err(SketchError::WrongKind(_))
        ));
    }
    // Rules whose names are the wrong kind of thing.
    for rule in [
        "spline-against 2.0 start 1.0 start g1",
        "spline-against 1.0 start 2.2 start g1",
        "spline-length 1.0 5",
        "spline-length 2.0 0",
        "spline-length 2.0 -3",
        "spline-through 1.0 1.0 0.5",
        "spline-through 2.0 2.0 0.5",
        "spline-through 2.0 1.0 -0.5",
    ] {
        let text = format!("{base}constraint 3.0 driving 0 {rule}\n");
        assert!(parse(&text).is_err(), "{rule}");
    }
    // A fit-point spline's helpers refuse what is not one.
    assert!(matches!(
        fit::reinterpolate(&s, point),
        Err(SketchError::WrongKind(_))
    ));
    assert!(matches!(
        fit::reinterpolate(&s, id(2, 0)),
        Err(SketchError::WrongKind(_))
    ));
}

#[test]
fn a_rational_spline_leaves_along_the_line_it_is_tangent_to() {
    // The first quarter circle leaves (5, 0) straight up: along the vertical line x = 5, across the
    // horizontal one.
    let mut text = quarters();
    text.push_str("entity 1.7 point 5 -3\nentity 1.8 point 5 9\nentity 1.9 point -4 0\n");
    text.push_str("entity 2.3 line 1.7 1.8 segment\nentity 2.4 line 1.9 1.8 segment\n");
    let along = tail(&with_rule(&text, "spline-tangent-line 2.0 start 2.3"), 1);
    assert!(along[0].abs() < 1e-9, "{along:?}");
    let across = tail(&with_rule(&text, "spline-tangent-line 2.0 start 2.4"), 1);
    assert!(across[0].abs() > 0.1, "{across:?}");
}

/// The length of a cubic Bézier by summing 200,000 chords: below the true length by less than
/// 1e-7 here, from the curve's own formula and nothing of the crate under test.
fn chord_length(p: [[f64; 2]; 4]) -> f64 {
    let at = |t: f64| -> [f64; 2] {
        let u = 1.0 - t;
        let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
        [
            w[0] * p[0][0] + w[1] * p[1][0] + w[2] * p[2][0] + w[3] * p[3][0],
            w[0] * p[0][1] + w[1] * p[1][1] + w[2] * p[2][1] + w[3] * p[3][1],
        ]
    };
    let n = 200_000;
    let mut last = at(0.0);
    let mut total = 0.0;
    for i in 1..=n {
        let here = at(f64::from(i) / f64::from(n));
        total += (here[0] - last[0]).hypot(here[1] - last[1]);
        last = here;
    }
    total
}

#[test]
fn a_loop_needs_more_panels_and_still_has_its_length_within_the_bound() {
    let p = [[0.0, 0.0], [300.0, 10.0], [-300.0, 60.0], [0.0, 70.0]];
    let mut text = HEAD.to_owned();
    for (k, c) in p.iter().enumerate() {
        point(&mut text, k as u64, c[0], c[1]);
    }
    spline(&mut text, 0, 3, 0, &[1.0; 4], "0 0 0 0 1 1 1 1");
    let l = spline_length(&parse(&text).unwrap(), id(2, 0)).unwrap();
    let exact = chord_length(p);
    assert!(
        (l.value - exact).abs() <= l.error + 1e-6,
        "{l:?} against {exact}"
    );
    // The bound is honest, not tight, for a curve this tight in its turn: it shrinks with the panels
    // and is still below a thousandth here, while the value is within 1e-6 of the chords.
    assert!((l.value - exact).abs() < 1e-6, "{l:?} against {exact}");
    assert!(l.error < 1e-3 * l.value, "{l:?}");
}
