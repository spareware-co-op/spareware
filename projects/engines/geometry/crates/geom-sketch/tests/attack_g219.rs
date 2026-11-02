//! Piece G219's attack tests: the constraint catalogue on ellipses (chapter 15.3).
//!
//! The scene, `data/ellipses.sketch`, is one drawing on which every constraint holds exactly: an ellipse of major
//! radius 10 along x and minor radius 5 about the origin, with (6, 4) on it, its top (0, 5) at the
//! parameter ½ and a line y = 5 touching it there; and a second ellipse about (100, 0), its major
//! axis (6, 8) turned off the frame, with its point (106, 8) at the parameter 0. The tests hold each
//! kind to chapter 15.4's rules: zero where it holds; the value worked by hand here when something
//! moves; every Jacobian entry a central difference of its residual; each row acting as a length;
//! each singular set refused, naming the constraint; every name it holds checked for its kind; the
//! radii measured as reference dimensions; and the text reading back.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::float_arithmetic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::needless_range_loop,
    clippy::many_single_char_names,
    clippy::format_push_string,
    clippy::format_collect,
    clippy::disallowed_methods,
    reason = "tests, written as the definitions read; the Simpson oracle uses the platform's sine and               cosine so that it shares nothing with the kernel's quadrature"
)]

use geom_sketch::equation::{Assembly, measure};
use geom_sketch::model::{ItemId, Sketch, SketchError};
use geom_sketch::text::{parse, write};

/// The scene: its entities, then one line for each rule, `20.k` the k-th.
const FILE: &str = include_str!("data/ellipses.sketch");

/// The scene's text before its first constraint, with `\n` line ends.
fn bare() -> String {
    FILE.lines()
        .filter(|line| !line.starts_with("constraint"))
        .map(|line| format!("{line}\n"))
        .collect()
}

/// The scene's rules, as they are written after their standing and priority.
fn rules() -> Vec<String> {
    FILE.lines()
        .filter(|line| line.starts_with("constraint"))
        .map(|line| line.splitn(5, ' ').nth(4).unwrap().to_owned())
        .collect()
}

fn id(sequence: u64, key: u64) -> ItemId {
    ItemId::new(sequence, key)
}

/// The scene with `rules` as its constraints.
fn with(rules: &[&str]) -> Result<Sketch, SketchError> {
    let mut text = bare();
    for (k, rule) in rules.iter().enumerate() {
        text.push_str(&format!("constraint 20.{k} driving 0 {rule}\n"));
    }
    parse(&text)
}

fn scene() -> Sketch {
    parse(FILE).unwrap()
}

/// `a`'s start, with each of `moves` added to the variable it names.
fn moved(a: &Assembly, moves: &[(ItemId, u8, f64)]) -> Vec<f64> {
    let mut x = a.start().unwrap();
    for &(owner, slot, by) in moves {
        let place = a
            .variables()
            .iter()
            .position(|v| v.owner == owner && v.slot == slot)
            .unwrap();
        x[place] += by;
    }
    x
}

#[test]
fn every_kind_is_zero_where_it_holds_and_reads_back() {
    let s = scene();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate(&s).unwrap();
    // One row each, but two for each point at a parameter and for the point's distance, and two
    // for each end of the arc.
    assert_eq!(a.residuals().len(), rules().len() + 3 + 4);
    for (i, r) in a.residuals().iter().enumerate() {
        assert!(r.abs() < 1e-12, "row {i}: {r}");
    }
    assert_eq!(write(&s).unwrap(), FILE.replace("\r\n", "\n"));
    assert_eq!(parse(&write(&s).unwrap()).unwrap(), s);
}

/// What `rule`'s rows read when its items move, each worked by hand. The rules of the arc's own ends
/// come first, so the rule's rows are the last.
fn holds_at(rule: &str, moves: &[(ItemId, u8, f64)], expected: &[f64]) {
    let s = with(&[rule]).unwrap();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate_at(&moved(&a, moves)).unwrap();
    let rows = a.residuals();
    let rows = &rows[rows.len() - expected.len()..];
    for (got, want) in rows.iter().zip(expected) {
        assert!(
            (got - want).abs() < 1e-12,
            "{rule}: {rows:?} for {expected:?}"
        );
    }
}

#[test]
fn each_kind_reads_what_it_should_off_its_solution() {
    // The top moved one along x and two up: off its parameter by exactly that.
    holds_at(
        "at-parameter 1.2 6.0",
        &[(id(1, 2), 0, 1.0), (id(1, 2), 1, 2.0)],
        &[1.0, 2.0],
    );
    // The turned ellipse's point at 0 is c + a: moving the axis by (1, −1) moves it with it.
    holds_at(
        "at-parameter 1.6 6.1",
        &[(id(5, 1), 0, 1.0), (id(5, 1), 1, -1.0)],
        &[-1.0, 1.0],
    );
    // (13, 0) raised one: its foot at the vertex (10, 0), where the tangent is upright, is one off
    // across it, and (10 − 9) / 6 off the distance of 3.
    holds_at(
        "point-ellipse 1.13 6.2 3",
        &[(id(1, 13), 1, 1.0)],
        &[1.0, 1.0 / 6.0],
    );
    // An axis of 11: (121 − 100) / 20.
    holds_at("major-radius 5.0 10", &[(id(5, 0), 0, 1.0)], &[1.05]);
    // A minor radius of 6.
    holds_at("minor-radius 5.0 5", &[(id(5, 0), 2, 1.0)], &[1.0]);
    // The line raised one: the centre is 6 below it and the ellipse reaches 5, a gap of −1 on its
    // right; lowered one, the line cuts it, a gap of +1.
    holds_at(
        "tangent-ellipse 2.0 5.0 right",
        &[(id(1, 3), 1, 1.0), (id(1, 4), 1, 1.0)],
        &[-1.0],
    );
    holds_at(
        "tangent-ellipse 2.0 5.0 right",
        &[(id(1, 3), 1, -1.0), (id(1, 4), 1, -1.0)],
        &[1.0],
    );
    // The minor radius grown to 6 with the line still at 5: the ellipse reaches 6, a gap of +1.
    holds_at(
        "tangent-ellipse 2.0 5.0 right",
        &[(id(5, 0), 2, 1.0)],
        &[1.0],
    );
}

#[test]
fn an_elliptical_arc_keeps_each_end_at_its_parameter() {
    // The arc about (200, 0), axis (10, 0) and minor radius 5, from its parameter 0 to ½: its ends
    // are (210, 0) and (200, 5), each two rows, first in the assembly.
    let s = with(&[]).unwrap();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate(&s).unwrap();
    assert_eq!(a.residuals(), &[0.0; 4]);
    // The start moved by (1, 2): off its parameter by that.
    a.evaluate_at(&moved(&a, &[(id(1, 8), 0, 1.0), (id(1, 8), 1, 2.0)]))
        .unwrap();
    assert_eq!(a.residuals(), &[1.0, 2.0, 0.0, 0.0]);
    // The axis grown to 11: the start, at c + a, is one short of it; the end, at the top, stays.
    a.evaluate_at(&moved(&a, &[(id(7, 0), 0, 1.0)])).unwrap();
    for (got, want) in a.residuals().iter().zip([-1.0, 0.0, 0.0, 0.0]) {
        assert!((got - want).abs() < 1e-12, "{:?}", a.residuals());
    }
    // An arc is an ellipse to every ellipse rule.
    holds_at("major-radius 7.0 10", &[(id(7, 0), 0, 1.0)], &[1.05]);
    holds_at("on-ellipse 1.9 7.0", &[], &[0.0]);
    // Its ends must be points, and a parameter it names must be there by the assembly.
    let bad_end = bare().replace(
        "elliptical-arc 1.7 10 0 5 1.8 1.9",
        "elliptical-arc 1.7 10 0 5 2.0 1.9",
    );
    assert!(matches!(parse(&bad_end), Err(SketchError::WrongKind(_))));
    let missing = bare().replace(
        "entity 8.1 parameter 7.0 0.5 31.5
",
        "",
    );
    let s = parse(&missing).unwrap();
    assert!(matches!(Assembly::new(&s), Err(SketchError::Unknown(_))));
}

#[test]
fn curves_tangent_at_a_point_read_how_far_their_normals_turn() {
    // At (0, 5) the circle of radius 3 about (0, 8), the line y = 5 and the ellipse all have
    // vertical normals; at (10, 0) the two ellipses horizontal ones. All hold.
    for rule in [
        "tangent-at-point 5.0 3.0 1.2",
        "tangent-at-point 2.0 5.0 1.2",
        "tangent-at-point 5.0 5.2 1.12",
    ] {
        holds_at(rule, &[], &[0.0]);
    }
    // The circle moved one to the right: its normal at (0, 5) is (−1, −3), the ellipse's (0, 50000),
    // and the row turns positive, as the cross product does.
    let s = with(&["tangent-at-point 5.0 3.0 1.2"]).unwrap();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate_at(&moved(&a, &[(id(1, 10), 0, 1.0)])).unwrap();
    assert!(*a.residuals().last().unwrap() > 0.0, "{:?}", a.residuals());
    a.evaluate_at(&moved(&a, &[(id(1, 10), 0, -1.0)])).unwrap();
    assert!(*a.residuals().last().unwrap() < 0.0, "{:?}", a.residuals());
    // At the circle's centre, or the ellipse's, a curve has no normal: refused, naming the rule.
    for moves in [vec![(id(1, 2), 1, 3.0)], vec![(id(1, 2), 1, -5.0)]] {
        let mut a = Assembly::new(&s).unwrap();
        assert_eq!(
            a.evaluate_at(&moved(&a, &moves)),
            Err(SketchError::Degenerate(id(20, 0))),
            "{moves:?}"
        );
    }
    // Only curves can be tangent.
    for rule in [
        "tangent-at-point 1.0 5.0 1.2",
        "tangent-at-point 5.0 6.0 1.2",
        "tangent-at-point 5.0 3.0 3.0",
    ] {
        assert!(
            matches!(with(&[rule]), Err(SketchError::WrongKind(_))),
            "{rule}"
        );
    }
}

#[test]
fn a_point_inside_or_outside_an_ellipse_reads_with_the_sign_of_the_side() {
    // (6, 4) moved out along y, then in: the residual is positive outside, negative inside.
    let s = with(&["on-ellipse 1.1 5.0"]).unwrap();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate_at(&moved(&a, &[(id(1, 1), 1, 0.5)])).unwrap();
    assert!(*a.residuals().last().unwrap() > 0.0);
    a.evaluate_at(&moved(&a, &[(id(1, 1), 1, -0.5)])).unwrap();
    assert!(*a.residuals().last().unwrap() < 0.0);
}

/// The start of `a`, each variable moved by a fixed pseudo-random amount within ±1.
fn jiggled(a: &Assembly) -> Vec<f64> {
    let mut x = a.start().unwrap();
    let mut state = 0x0219_u64;
    for value in &mut x {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *value += ((state >> 40) as f64 / (1_u64 << 24) as f64 - 0.5) * 2.0;
    }
    x
}

#[test]
fn every_jacobian_entry_is_the_derivative_of_its_residual() {
    let s = scene();
    let mut a = Assembly::new(&s).unwrap();
    let x = jiggled(&a);
    a.rescale(&x).unwrap();
    a.evaluate_at(&x).unwrap();
    let jacobian = a.jacobian().to_dense().unwrap();
    let rows = a.residuals().len();
    let h = 1e-5;
    for j in 0..x.len() {
        let scale = a.variables()[j].scale;
        let at = |value: f64, a: &mut Assembly| {
            let mut y = x.clone();
            y[j] = value;
            a.evaluate_at(&y).unwrap();
            a.residuals().to_vec()
        };
        let up = at(x[j] + h, &mut a);
        let down = at(x[j] - h, &mut a);
        for i in 0..rows {
            let numeric = (up[i] - down[i]) / (2.0 * h) * scale;
            let exact = jacobian.get(i, j).unwrap();
            assert!(
                (numeric - exact).abs() < 2e-6 * (1.0 + exact.abs()),
                "row {i}, variable {j}: {numeric} and {exact}"
            );
        }
    }
}

#[test]
fn each_row_acts_as_a_length_at_its_solution() {
    // Section 15.4: one tolerance serves every constraint only if each row's gradient is near one.
    let s = scene();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate(&s).unwrap();
    let dense = a.jacobian().to_dense().unwrap();
    for i in 0..a.residuals().len() {
        let mut sum = 0.0;
        for j in 0..a.variables().len() {
            let entry = dense.get(i, j).unwrap();
            sum += entry * entry;
        }
        let norm = sum.sqrt();
        assert!((0.25..8.0).contains(&norm), "row {i}: {norm}");
    }
}

#[test]
fn every_singular_set_is_refused_naming_the_constraint() {
    for (rule, moves) in [
        // No minor radius.
        ("on-ellipse 1.1 5.0", vec![(id(5, 0), 2, -5.0)]),
        // Equal radii: the axis shrunk to 5, a circle with no axis direction.
        ("on-ellipse 1.1 5.0", vec![(id(5, 0), 0, -5.0)]),
        // No axis.
        ("at-parameter 1.2 6.0", vec![(id(5, 0), 0, -10.0)]),
        ("at-parameter 1.2 6.0", vec![(id(5, 0), 0, -5.0)]),
        // A line of no length, and an ellipse of no minor radius.
        ("tangent-ellipse 2.0 5.0 right", vec![(id(1, 3), 0, 40.0)]),
        ("tangent-ellipse 2.0 5.0 right", vec![(id(5, 0), 2, -5.0)]),
    ] {
        let s = with(&[rule]).unwrap();
        let mut a = Assembly::new(&s).unwrap();
        assert_eq!(
            a.evaluate_at(&moved(&a, &moves)),
            Err(SketchError::Degenerate(id(20, 0))),
            "{rule}: {moves:?}"
        );
    }
}

#[test]
fn every_name_is_checked_for_its_kind() {
    // An ellipse's rules name an ellipse; a parameter must be on one.
    for rule in [
        "on-ellipse 1.1 2.0",
        "on-ellipse 2.0 5.0",
        "at-parameter 1.2 5.0",
        "at-parameter 1.2 1.0",
        "major-radius 2.0 10",
        "minor-radius 1.0 5",
        "tangent-ellipse 5.0 5.0 left",
        "tangent-ellipse 2.0 2.0 left",
    ] {
        assert!(
            matches!(with(&[rule]), Err(SketchError::WrongKind(_))),
            "{rule}: {:?}",
            with(&[rule])
        );
    }
    // A radius must be above zero.
    assert!(with(&["major-radius 5.0 0"]).is_err());
    assert!(with(&["minor-radius 5.0 -1"]).is_err());
}

#[test]
fn the_radii_are_dimensions_and_measure() {
    let mut text = bare();
    text.push_str("constraint 20.0 reference 0 major-radius 5.1 7\n");
    text.push_str("constraint 20.1 reference 0 minor-radius 5.1 7\n");
    let s = parse(&text).unwrap();
    let find = |k: u64| s.constraints().iter().find(|c| c.id == id(20, k)).unwrap();
    let [major, minor] = [find(0), find(1)];
    assert_eq!(measure(&s, major).unwrap(), 10.0);
    assert_eq!(measure(&s, minor).unwrap(), 5.0);
    // The point's distance, 3 from its foot at the vertex.
    let apart = parse(&format!(
        "{}constraint 20.0 reference 0 point-ellipse 1.13 6.2 7\n",
        bare()
    ))
    .unwrap();
    let last = apart.constraints().last().unwrap();
    assert!((measure(&apart, last).unwrap() - 3.0).abs() < 1e-12);
    // A reference is measured, never solved: no rows but the arc's own four.
    assert_eq!(Assembly::new(&s).unwrap().residuals().len(), 4);
    // A point on an ellipse is no dimension.
    let point = parse(&format!(
        "{}constraint 20.0 reference 0 on-ellipse 1.1 5.0\n",
        bare()
    ));
    assert!(matches!(point, Err(SketchError::NotADimension(_))));
}

/// The length of the ellipse of axis (a, 0) and minor radius b from the parameter t0 to t1, by
/// Simpson's rule on 200,000 panels: an oracle of its own, independent of the kernel's quadrature.
fn simpson(a: f64, b: f64, t0: f64, t1: f64) -> f64 {
    let n = 200_000;
    let h = (t1 - t0) / f64::from(n);
    let speed = |t: f64| {
        let s = t * std::f64::consts::PI;
        std::f64::consts::PI * (a * a * s.sin() * s.sin() + b * b * s.cos() * s.cos()).sqrt()
    };
    let mut sum = speed(t0) + speed(t1);
    for k in 1..n {
        let w = if k % 2 == 1 { 4.0 } else { 2.0 };
        sum += w * speed(t0 + f64::from(k) * h);
    }
    sum * h / 3.0
}

#[test]
fn an_elliptical_arcs_length_is_its_quadrature() {
    // The arc from the parameter 0 to ½ on the ellipse 10 by 5: a quarter of its perimeter.
    let quarter = simpson(10.0, 5.0, 0.0, 0.5);
    let s = with(&["elliptical-arc-length 7.0 12"]).unwrap();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate(&s).unwrap();
    let row = *a.residuals().last().unwrap();
    assert!(
        (row - (quarter - 12.0)).abs() < 1e-9,
        "{row} for {}",
        quarter - 12.0
    );
    // Measured as a reference.
    let r = parse(&format!(
        "{}constraint 20.0 reference 0 elliptical-arc-length 7.0 12\n",
        bare()
    ))
    .unwrap();
    let got = measure(&r, r.constraints().last().unwrap()).unwrap();
    assert!((got - quarter).abs() < 1e-9, "{got} for {quarter}");
    // Every derivative, in the axis, the minor radius and both ends' parameters, is the central
    // difference of the row.
    let x = jiggled(&a);
    a.rescale(&x).unwrap();
    a.evaluate_at(&x).unwrap();
    let jacobian = a.jacobian().to_dense().unwrap();
    let last = a.residuals().len() - 1;
    let h = 1e-6;
    for j in 0..x.len() {
        let scale = a.variables()[j].scale;
        let mut at = |value: f64| {
            let mut y = x.clone();
            y[j] = value;
            a.evaluate_at(&y).unwrap();
            a.residuals()[last]
        };
        let numeric = (at(x[j] + h) - at(x[j] - h)) / (2.0 * h) * scale;
        let exact = jacobian.get(last, j).unwrap();
        assert!(
            (numeric - exact).abs() < 1e-5 * (1.0 + exact.abs()),
            "variable {j}: {numeric} and {exact}"
        );
    }
    // Only an elliptical arc has this length.
    assert!(matches!(
        with(&["elliptical-arc-length 5.0 12"]),
        Err(SketchError::WrongKind(_))
    ));
}
