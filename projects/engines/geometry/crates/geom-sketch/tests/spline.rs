// SPDX-License-Identifier: Apache-2.0
//! Tests of the spline constraints and the smoothness measure (piece G220; chapter 15.3).
//!
//! The oracles are independent of the code under test. The derivatives at a spline's ends are held
//! to a Cox-de Boor evaluator written here and to forward differences exact for a cubic. A cubic
//! cut in two by de Casteljau's rule at its middle gives two splines that join with every order of
//! smoothness; the rows of each join are zero there and read the value worked by hand when a control
//! point moves. Every Jacobian entry is a central difference of its residual.

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
    clippy::manual_midpoint,
    clippy::type_complexity,
    clippy::cast_possible_truncation,
    reason = "tests, written as the definitions read"
)]

use std::fmt::Write;

use geom_sketch::equation::Assembly;
use geom_sketch::model::{ItemId, Sketch, SketchError, Smoothness, SplineEnd};
use geom_sketch::smooth::{continuity, curvature_of, curvature_rate_of, end_jets};
use geom_sketch::text::{parse, write};

const HEAD: &str = "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n";

fn id(sequence: u64, key: u64) -> ItemId {
    ItemId::new(sequence, key)
}

/// The points `pts` as entities 1.k, from `first`.
fn points(text: &mut String, first: u64, pts: &[[f64; 2]]) {
    for (k, p) in pts.iter().enumerate() {
        writeln!(
            text,
            "entity 1.{} point {} {}",
            first + k as u64,
            p[0],
            p[1]
        )
        .unwrap();
    }
}

/// A spline entity `2.index` of `degree` on the points 1.first.. with `knots` and unit weights.
fn spline(text: &mut String, index: u64, degree: u32, first: u64, n: usize, knots: &[f64]) {
    write!(text, "entity 2.{index} spline {degree} {n}").unwrap();
    for k in 0..n {
        write!(text, " 1.{}", first + k as u64).unwrap();
    }
    write!(text, " {}", knots.len()).unwrap();
    for k in knots {
        write!(text, " {k}").unwrap();
    }
    write!(text, " {n}").unwrap();
    for _ in 0..n {
        text.push_str(" 1");
    }
    text.push('\n');
}

const BEZIER: [f64; 8] = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];

/// The cubic Bézier `p` cut at its middle: the left and right halves' control points.
fn halves(p: [[f64; 2]; 4]) -> ([[f64; 2]; 4], [[f64; 2]; 4]) {
    let mid = |a: [f64; 2], b: [f64; 2]| [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let (q0, q1, q2) = (mid(p[0], p[1]), mid(p[1], p[2]), mid(p[2], p[3]));
    let (r0, r1) = (mid(q0, q1), mid(q1, q2));
    let s = mid(r0, r1);
    ([p[0], q0, r0, s], [s, r1, q2, p[3]])
}

/// A cubic cut in two: splines 2.0 (the left half) and 2.1 (the right), the first on points 1.0 to
/// 1.3 and the second on 1.4 to 1.7.
fn cut_cubic() -> String {
    let (l, r) = halves([[0.0, 0.0], [8.0, 16.0], [24.0, 20.0], [32.0, 0.0]]);
    let mut text = HEAD.to_owned();
    points(&mut text, 0, &l);
    points(&mut text, 4, &r);
    spline(&mut text, 0, 3, 0, 4, &BEZIER);
    spline(&mut text, 1, 3, 4, 4, &BEZIER);
    text
}

fn with_rule(text: &str, rule: &str) -> Sketch {
    parse(&format!("{text}constraint 3.0 driving 0 {rule}\n")).unwrap()
}

fn residuals(s: &Sketch) -> Vec<f64> {
    let mut a = Assembly::new(s).unwrap();
    a.evaluate(s).unwrap();
    a.residuals().to_vec()
}

#[test]
fn a_cubic_cut_in_two_joins_with_every_order() {
    let text = cut_cubic();
    for (order, rows) in [("g1", 1), ("g2", 2), ("g3", 3)] {
        let s = with_rule(&text, &format!("spline-join 2.0 finish 2.1 start {order}"));
        let r = residuals(&s);
        assert_eq!(r.len(), rows);
        for (i, x) in r.iter().enumerate() {
            assert!(x.abs() < 1e-12, "{order} row {i}: {x}");
        }
    }
    let s = parse(&text).unwrap();
    for order in [
        Smoothness::Tangent,
        Smoothness::Curvature,
        Smoothness::CurvatureRate,
    ] {
        let c = continuity(
            &s,
            (id(2, 0), SplineEnd::Finish),
            (id(2, 1), SplineEnd::Start),
            order,
        )
        .unwrap();
        assert!(c.tangent.abs() < 1e-12, "{c:?}");
        assert!(c.curvature.is_none_or(|k| k.abs() < 1e-12), "{c:?}");
        assert!(c.rate.is_none_or(|k| k.abs() < 1e-9), "{c:?}");
    }
}

/// Bézier derivatives at the start of the cubic `p`, by the textbook formulas.
fn bezier_jets(p: [[f64; 2]; 4]) -> [[f64; 2]; 3] {
    let sub = |a: [f64; 2], b: [f64; 2]| [a[0] - b[0], a[1] - b[1]];
    let d1 = sub(p[1], p[0]);
    let d2 = sub(sub(p[2], p[1]), d1);
    let q = sub(sub(p[3], p[2]), sub(p[2], p[1]));
    let d3 = sub(q, d2);
    [
        [3.0 * d1[0], 3.0 * d1[1]],
        [6.0 * d2[0], 6.0 * d2[1]],
        [6.0 * d3[0], 6.0 * d3[1]],
    ]
}

fn cross(u: [f64; 2], v: [f64; 2]) -> f64 {
    u[0] * v[1] - u[1] * v[0]
}

fn dot(u: [f64; 2], v: [f64; 2]) -> f64 {
    u[0] * v[0] + u[1] * v[1]
}

#[test]
fn a_rule_reads_the_value_worked_by_hand_when_a_control_point_moves() {
    let text = cut_cubic();
    let (l, r) = halves([[0.0, 0.0], [8.0, 16.0], [24.0, 20.0], [32.0, 0.0]]);
    // Move the right half's second control point up by 3 and the third by 2.
    let mut r2 = r;
    r2[1][1] += 3.0;
    r2[2][1] += 2.0;
    // The scale a rule has is the power of two at or below the larger first leg of the two ends,
    // outward: both start at the join.
    let leg = |a: [f64; 2], b: [f64; 2]| (b[0] - a[0]).abs().max((b[1] - a[1]).abs());
    let scale = 2.0_f64.powf(leg(l[3], l[2]).max(leg(r[0], r[1])).log2().floor());
    // Build the rule on the original and read it at the moved points.
    let s = with_rule(&text, "spline-join 2.0 finish 2.1 start g3");
    let mut a = Assembly::new(&s).unwrap();
    let mut x = a.start().unwrap();
    for (k, p) in r2.iter().enumerate() {
        let (px, py) = (id(1, 4 + k as u64), 0);
        let ix = a
            .variables()
            .iter()
            .position(|v| v.owner == px && v.slot == py)
            .unwrap();
        x[ix] = p[0];
        x[ix + 1] = p[1];
    }
    a.evaluate_at(&x).unwrap();
    let got = a.residuals().to_vec();
    // The first end, outward from the left half's finish: its control points reversed.
    let out_a = bezier_jets([l[3], l[2], l[1], l[0]]);
    let out_b = bezier_jets(r2);
    let sine =
        cross(out_a[0], out_b[0]) / (dot(out_a[0], out_a[0]) * dot(out_b[0], out_b[0])).sqrt();
    let speed = |d: [f64; 2]| dot(d, d).sqrt();
    let k = |j: &[[f64; 2]; 3]| cross(j[0], j[1]) / speed(j[0]).powi(3);
    let kr = |j: &[[f64; 2]; 3]| {
        (cross(j[0], j[2]) * dot(j[0], j[0]) - 3.0 * cross(j[0], j[1]) * dot(j[0], j[1]))
            / dot(j[0], j[0]).powi(3)
    };
    let want = [
        sine * scale,
        (k(&out_a) + k(&out_b)) * scale * scale,
        (kr(&out_a) - kr(&out_b)) * scale.powi(3),
    ];
    for i in 0..3 {
        assert!(
            (got[i] - want[i]).abs() < 1e-9 * (1.0 + want[i].abs()),
            "row {i}: {got:?} for {want:?}"
        );
    }
    assert!(
        got[0].abs() > 0.1 && got[1].abs() > 1e-3 && got[2].abs() > 1e-8,
        "{got:?}"
    );
}

/// The Cox-de Boor value of the spline with `knots`, degree `p` and control points `pts` at `u`.
fn deboor(p: usize, knots: &[f64], pts: &[[f64; 2]], u: f64) -> [f64; 2] {
    // The span, with the last span closed at the end.
    let n = pts.len();
    let mut k = p;
    while k + 1 < n && u >= knots[k + 1] {
        k += 1;
    }
    let mut d: Vec<[f64; 2]> = (0..=p).map(|j| pts[j + k - p]).collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = j + k - p;
            let a = (u - knots[i]) / (knots[i + p + 1 - r] - knots[i]);
            d[j] = [
                (1.0 - a) * d[j - 1][0] + a * d[j][0],
                (1.0 - a) * d[j - 1][1] + a * d[j][1],
            ];
        }
    }
    d[p]
}

/// The first three derivatives at the start of a polynomial piece, from four equally spaced
/// samples `h` apart: forward differences, exact for a cubic.
fn forward_jets(f: [[f64; 2]; 4], h: f64) -> [[f64; 2]; 3] {
    let mut out = [[0.0; 2]; 3];
    for c in 0..2 {
        let [f0, f1, f2, f3] = [f[0][c], f[1][c], f[2][c], f[3][c]];
        out[0][c] = (-11.0 / 6.0 * f0 + 3.0 * f1 - 1.5 * f2 + f3 / 3.0) / h;
        out[1][c] = (2.0 * f0 - 5.0 * f1 + 4.0 * f2 - f3) / (h * h);
        out[2][c] = (-f0 + 3.0 * f1 - 3.0 * f2 + f3) / (h * h * h);
    }
    out
}

#[test]
fn the_ends_derivatives_are_those_of_the_curve() {
    // A cubic with one inner knot, a cubic with two, and a quadratic: the factors depend on the knots.
    // Each piece is a polynomial of the degree, so forward differences on it are exact.
    let five = [[0.0, 0.0], [3.0, 5.0], [7.0, 6.0], [10.0, 2.0], [12.0, 9.0]];
    let six = [
        [1.0, 1.0],
        [2.0, 6.0],
        [5.0, 7.0],
        [9.0, 3.0],
        [11.0, 8.0],
        [14.0, 2.0],
    ];
    let cases: [(usize, &[[f64; 2]], &[f64], usize); 3] = [
        (3, &five, &[0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0], 3),
        (
            3,
            &six,
            &[0.0, 0.0, 0.0, 0.0, 0.3, 0.7, 1.0, 1.0, 1.0, 1.0],
            3,
        ),
        (2, &five, &[0.0, 0.0, 0.0, 0.3, 0.6, 1.0, 1.0, 1.0], 2),
    ];
    for (p, pts, knots, level) in cases {
        let mut text = HEAD.to_owned();
        points(&mut text, 0, pts);
        spline(&mut text, 0, p as u32, 0, pts.len(), knots);
        let s = parse(&text).unwrap();
        let end = knots[knots.len() - 1];
        let h = 0.08;
        for (which, reverse) in [(SplineEnd::Start, false), (SplineEnd::Finish, true)] {
            // An outward parameter v runs from the end into the curve.
            let sample = |v: f64| deboor(p, knots, pts, if reverse { end - v } else { v });
            let f = [sample(0.0), sample(h), sample(2.0 * h), sample(3.0 * h)];
            let want = forward_jets(f, h);
            let got = end_jets(&s, id(2, 0), which, level).unwrap();
            for i in 0..level {
                let g = [got.first, got.second, got.third][i];
                for c in 0..2 {
                    assert!(
                        (g[c] - want[i][c]).abs() < 1e-6 * (1.0 + want[i][c].abs()),
                        "degree {p} {which:?} derivative {}: {g:?} for {:?}",
                        i + 1,
                        want[i]
                    );
                }
            }
        }
    }
}

#[test]
fn curvature_and_its_rate_are_those_of_the_curve() {
    // The curve (t, t^3 + 2t^2 + t): the curvature and its derivative along the arc at t = 0.3,
    // the rate checked against a central difference of the curvature over the speed.
    let at = |t: f64| {
        (
            [1.0, 3.0 * t * t + 4.0 * t + 1.0],
            [0.0, 6.0 * t + 4.0],
            [0.0, 6.0],
        )
    };
    let kappa = |t: f64| {
        let (d1, d2, _) = at(t);
        curvature_of(d1, d2).unwrap()
    };
    let t = 0.3;
    let (d1, d2, d3) = at(t);
    let h = 1e-5;
    let numeric = (kappa(t + h) - kappa(t - h)) / (2.0 * h) / dot(d1, d1).sqrt();
    let exact = curvature_rate_of(d1, d2, d3).unwrap();
    assert!((numeric - exact).abs() < 1e-7, "{numeric} {exact}");
    assert!((kappa(t) - cross(d1, d2) / dot(d1, d1).powf(1.5)).abs() < 1e-14);
}

#[test]
fn every_jacobian_entry_is_the_derivative_of_its_residual() {
    // A short quartic against a cubic, and the cubic against a line, jiggled off the join.
    let mut text = HEAD.to_owned();
    points(
        &mut text,
        0,
        &[
            [0.0, 0.0],
            [2.0, 3.0],
            [5.0, 4.0],
            [8.0, 2.0],
            [8.0, 2.0],
            [11.0, 1.0],
            [13.0, -2.0],
            [16.0, -1.0],
            [20.0, 5.0],
            [30.0, 6.0],
        ],
    );
    spline(&mut text, 0, 3, 0, 4, &BEZIER);
    spline(
        &mut text,
        1,
        3,
        4,
        4,
        &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    );
    writeln!(text, "entity 2.2 line 1.8 1.9 segment").unwrap();
    for rule in [
        "spline-join 2.0 finish 2.1 start g1",
        "spline-join 2.0 finish 2.1 start g2",
        "spline-join 2.0 finish 2.1 start g3",
        "spline-join 2.0 start 2.1 finish g3",
        "spline-tangent-line 2.1 finish 2.2",
        "spline-tangent-line 2.0 start 2.2",
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
fn a_spline_leaves_along_its_line() {
    // A cubic whose first leg points along (1, 1), against the line y = x and against y = 0.
    let mut text = HEAD.to_owned();
    points(
        &mut text,
        0,
        &[
            [0.0, 0.0],
            [2.0, 2.0],
            [5.0, 3.0],
            [8.0, 0.0],
            [1.0, 1.0],
            [9.0, 9.0],
            [0.0, 0.0],
            [9.0, 0.0],
        ],
    );
    spline(&mut text, 0, 3, 0, 4, &BEZIER);
    writeln!(text, "entity 2.1 line 1.4 1.5 segment").unwrap();
    writeln!(text, "entity 2.2 line 1.6 1.7 segment").unwrap();
    let along = residuals(&with_rule(&text, "spline-tangent-line 2.0 start 2.1"));
    assert!(along[0].abs() < 1e-12, "{along:?}");
    // Against the x axis the sine is -1/sqrt(2) (turning clockwise); the scale is the power of two
    // at or below the larger first leg, the line's 9.
    let across = residuals(&with_rule(&text, "spline-tangent-line 2.0 start 2.2"));
    assert!(
        (across[0] + 8.0 * (0.5_f64).sqrt()).abs() < 1e-12,
        "{across:?}"
    );
}

#[test]
fn the_text_form_reads_back_exactly() {
    let text = cut_cubic();
    let mut full = text;
    full.push_str("entity 2.2 line 1.0 1.7 segment\n");
    full.push_str("constraint 3.0 driving 0 spline-join 2.0 finish 2.1 start g3\n");
    full.push_str("constraint 3.1 driving 0 spline-join 2.1 finish 2.0 finish g2\n");
    full.push_str("constraint 3.2 driving 0 spline-tangent-line 2.0 finish 2.2\n");
    let s = parse(&full).unwrap();
    let written = write(&s).unwrap();
    assert_eq!(parse(&written).unwrap(), s);
    assert_eq!(write(&parse(&written).unwrap()).unwrap(), written);
    for bad in [
        "spline-join 2.0 middle 2.1 start g1",
        "spline-join 2.0 finish 2.1 start g4",
        "spline-tangent-line 2.0 start",
    ] {
        assert!(
            parse(&format!("{full}constraint 3.9 driving 0 {bad}\n")).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn only_a_spline_a_constraint_can_read_is_accepted() {
    let line = |k: u64| format!("entity 2.{k} line 1.0 1.1 segment\n");
    let base = |extra: &str, knots: &[f64], weights: &str| {
        let mut text = HEAD.to_owned();
        points(
            &mut text,
            0,
            &[[0.0, 0.0], [2.0, 1.0], [5.0, 3.0], [8.0, 0.0]],
        );
        write!(
            text,
            "entity 2.0 spline 3 4 1.0 1.1 1.2 1.3 {}",
            knots.len()
        )
        .unwrap();
        for k in knots {
            write!(text, " {k}").unwrap();
        }
        writeln!(text, " 4 {weights}").unwrap();
        text.push_str(&line(1));
        text.push_str(extra);
        text
    };
    let rule = "constraint 3.0 driving 0 spline-tangent-line 2.0 start 2.1\n";
    // The good one.
    assert!(parse(&base(rule, &BEZIER, "1 1 1 1")).is_ok());
    // Rational is read now (piece G220); not clamped, a wrong knot count, and a line where a
    // spline goes are not.
    assert!(parse(&base(rule, &BEZIER, "1 2 1 1")).is_ok());
    assert!(
        parse(&base(
            rule,
            &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 2.0],
            "1 1 1 1"
        ))
        .is_err()
    );
    assert!(parse(&base(rule, &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0], "1 1 1 1")).is_err());
    let join = "constraint 3.0 driving 0 spline-join 2.1 start 2.0 start g1\n";
    assert!(matches!(
        parse(&base(join, &BEZIER, "1 1 1 1")),
        Err(SketchError::WrongKind(_))
    ));
    // A quadratic has no third derivative.
    let mut quad = HEAD.to_owned();
    points(
        &mut quad,
        0,
        &[
            [0.0, 0.0],
            [2.0, 1.0],
            [5.0, 3.0],
            [0.0, 4.0],
            [3.0, 5.0],
            [6.0, 6.0],
        ],
    );
    quad.push_str("entity 2.0 spline 2 3 1.0 1.1 1.2 6 0 0 0 1 1 1 3 1 1 1\n");
    quad.push_str("entity 2.1 spline 2 3 1.3 1.4 1.5 6 0 0 0 1 1 1 3 1 1 1\n");
    // A quadratic on four points has the points for a third derivative and not the degree.
    let mut long = HEAD.to_owned();
    points(
        &mut long,
        0,
        &[
            [0.0, 0.0],
            [2.0, 1.0],
            [5.0, 3.0],
            [0.0, 4.0],
            [3.0, 5.0],
            [6.0, 6.0],
            [9.0, 7.0],
            [1.0, 9.0],
        ],
    );
    long.push_str(
        "entity 2.0 spline 2 4 1.0 1.1 1.2 1.3 7 0 0 0 0.5 1 1 1 4 1 1 1 1
",
    );
    long.push_str(
        "entity 2.1 spline 2 4 1.4 1.5 1.6 1.7 7 0 0 0 0.5 1 1 1 4 1 1 1 1
",
    );
    for (order, ok) in [("g1", true), ("g2", true), ("g3", false)] {
        let rule = format!(
            "constraint 3.0 driving 0 spline-join 2.0 finish 2.1 start {order}
"
        );
        assert_eq!(parse(&format!("{long}{rule}")).is_ok(), ok, "long {order}");
    }
    // Three points and a degree of three have no room for any derivative: all the knots are one.
    let mut flat = HEAD.to_owned();
    points(
        &mut flat,
        0,
        &[
            [0.0, 0.0],
            [2.0, 1.0],
            [5.0, 3.0],
            [0.0, 4.0],
            [3.0, 5.0],
            [6.0, 6.0],
        ],
    );
    flat.push_str(
        "entity 2.0 spline 3 3 1.0 1.1 1.2 7 0 0 0 0 0 0 0 3 1 1 1
",
    );
    flat.push_str(
        "entity 2.1 spline 3 3 1.3 1.4 1.5 7 0 0 0 0 0 0 0 3 1 1 1
",
    );
    for order in ["g1", "g2", "g3"] {
        let rule = format!(
            "constraint 3.0 driving 0 spline-join 2.0 finish 2.1 start {order}
"
        );
        assert!(parse(&format!("{flat}{rule}")).is_err(), "flat {order}");
    }
    for (order, ok) in [("g1", true), ("g2", true), ("g3", false)] {
        let rule = format!("constraint 3.0 driving 0 spline-join 2.0 finish 2.1 start {order}\n");
        assert_eq!(parse(&format!("{quad}{rule}")).is_ok(), ok, "{order}");
    }
}

/// The index of point `p`'s x among the variables.
fn place(a: &Assembly, p: ItemId) -> usize {
    a.variables().iter().position(|v| v.owner == p).unwrap()
}

/// Moves point `from` onto point `to` in `x`.
fn onto(a: &Assembly, x: &mut [f64], from: ItemId, to: ItemId) {
    let (f, t) = (place(a, from), place(a, to));
    x[f] = x[t];
    x[f + 1] = x[t + 1];
}

#[test]
fn a_spline_end_with_no_tangent_or_a_line_with_no_direction_is_degenerate() {
    let mut text = cut_cubic();
    text.push_str(
        "entity 1.8 point 3 3
entity 1.9 point 9 3
entity 2.2 line 1.8 1.9 segment
",
    );
    let p = |k: u64| id(1, k);
    // For each rule: the points whose collapse flattens an end (to, from), and the rule's id.
    let cases: [(&str, &[(u64, u64)]); 5] = [
        ("spline-join 2.0 finish 2.1 start g1", &[(3, 2), (4, 5)]),
        ("spline-join 2.0 finish 2.1 start g2", &[(3, 2), (4, 5)]),
        ("spline-join 2.0 finish 2.1 start g3", &[(3, 2), (4, 5)]),
        ("spline-join 2.0 start 2.1 finish g3", &[(0, 1), (7, 6)]),
        ("spline-tangent-line 2.1 finish 2.2", &[(7, 6), (8, 9)]),
    ];
    for (rule, flats) in cases {
        let s = with_rule(&text, rule);
        for &(to, from) in flats {
            let mut a = Assembly::new(&s).unwrap();
            let mut x = a.start().unwrap();
            onto(&a, &mut x, p(from), p(to));
            assert_eq!(
                a.evaluate_at(&x),
                Err(SketchError::Degenerate(id(3, 0))),
                "{rule}: {from} onto {to}"
            );
        }
        // Whole, and with a leg that is vertical or a line that is level: not degenerate.
        let mut a = Assembly::new(&s).unwrap();
        let mut x = a.start().unwrap();
        assert_eq!(a.evaluate_at(&x), Ok(()), "{rule}");
        for &(to, from) in flats {
            let (f, t) = (place(&a, p(from)), place(&a, p(to)));
            x[f] = x[t];
            x[f + 1] = x[t + 1] + 1.0;
            assert_eq!(a.evaluate_at(&x), Ok(()), "{rule}: {from} above {to}");
            x[f] = x[t] + 1.0;
            x[f + 1] = x[t + 1];
            assert_eq!(a.evaluate_at(&x), Ok(()), "{rule}: {from} beside {to}");
        }
    }
}
