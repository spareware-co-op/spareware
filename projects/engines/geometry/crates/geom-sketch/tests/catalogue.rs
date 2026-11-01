//! Tests of `geom_sketch`'s constraint catalogue on points, lines, circles and arcs (piece G218).
//!
//! The scene, `data/catalogue.sketch`, is one drawing on which every line of it holds exactly: a
//! rectangle 40 by 30 with its diagonal, circles and arcs about (20, 50), scalars, and a reference
//! line along its base. The tests hold each kind to chapter 15.3 and 15.4's rules: it is zero where
//! it holds; it reads the value worked by hand here when something moves, which a derivative test
//! cannot see, since a wrong residual has a matching wrong derivative; every Jacobian entry is a
//! central difference of its residual, shared corners included; each singular set is refused, naming
//! the constraint; every name and value it holds is checked; and its text reads back exactly.

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
    clippy::format_collect,
    reason = "tests, written as the definitions read"
)]

use core::f64::consts::PI;

use geom_sketch::author::{fix_entity, offset_chain};
use geom_sketch::equation::{Assembly, measure};
use geom_sketch::model::{
    Amount, CommitClass, Constraint, ConstraintKind, Entity, EntityKind, ItemId, Side, Sketch,
    SketchError, Standing,
};
use geom_sketch::structure::substitute;
use geom_sketch::text::{parse, write};
use geom_test::golden::{self, hex, sha256};

const SCENE: &str = include_str!("data/catalogue.sketch");

fn id(sequence: u64, key: u64) -> ItemId {
    ItemId::new(sequence, key)
}

fn scene() -> Sketch {
    parse(SCENE).unwrap()
}

/// The scene's text before its first constraint.
fn bare() -> String {
    SCENE
        .lines()
        .filter(|line| !line.starts_with("constraint"))
        .map(|line| format!("{line}\n"))
        .collect()
}

/// The scene's rules, as they are written after their standing and priority.
fn rules() -> Vec<String> {
    SCENE
        .lines()
        .filter(|line| line.starts_with("constraint"))
        .map(|line| line.splitn(5, ' ').nth(4).unwrap().to_owned())
        .collect()
}

/// The drawing, with `rule` as its only constraint.
fn alone(rule: &str) -> Result<Sketch, SketchError> {
    parse(&format!("{}constraint 20.0 driving 0 {rule}\n", bare()))
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
fn the_scene_is_canonical_and_every_rule_reads_back() {
    let s = scene();
    assert_eq!(write(&s).unwrap(), SCENE);
    assert_eq!(parse(&write(&s).unwrap()).unwrap(), s);
}

#[test]
fn every_kind_is_zero_where_it_holds() {
    let s = scene();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate(&s).unwrap();
    assert!(a.residuals().len() > 75);
    for (i, r) in a.residuals().iter().enumerate() {
        assert!(r.abs() < 1e-12, "row {i}: {r}");
    }
}

/// What the rule's own rows read when its items move, each worked by hand. The rules of an arc's own
/// ends come first, so the rule's rows are the last.
fn holds_at(rule: &str, moves: &[(ItemId, u8, f64)], expected: &[f64]) {
    let s = alone(rule).unwrap();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate_at(&moved(&a, moves)).unwrap();
    let rows = a.residuals();
    let own = &rows[rows.len() - expected.len()..];
    for (got, want) in own.iter().zip(expected) {
        assert!(
            (got - want).abs() < 1e-12,
            "{rule}: {own:?} for {expected:?}"
        );
    }
}

#[test]
fn each_kind_reads_what_it_should_off_its_solution() {
    let (a, b, c, d, m, e) = (id(1, 0), id(1, 1), id(1, 2), id(1, 3), id(1, 4), id(1, 5));
    let (o2, s, n, u, inner) = (id(1, 8), id(1, 9), id(1, 10), id(1, 12), id(1, 13));
    // Incidence. A line 40 long is scaled by 32, the power of two at or below it.
    holds_at("coincident 1.0 1.6", &[(id(1, 6), 0, 3.0)], &[-3.0, 0.0]);
    holds_at("concentric 3.0 3.1", &[(o2, 1, 4.0)], &[0.0, -4.0]);
    holds_at("on-line 1.4 2.0", &[(m, 1, 3.0)], &[120.0 / 32.0]);
    holds_at("on-circle 1.9 3.0", &[(s, 0, 1.0)], &[21.0 / 20.0]);
    holds_at("midpoint 1.4 1.0 1.1", &[(m, 0, 2.0)], &[-2.0, 0.0]);
    holds_at("collinear 2.0 2.5", &[(e, 1, 8.0)], &[0.0, 10.0]);
    holds_at("collinear 2.0 2.5", &[(m, 1, 4.0)], &[5.0, 0.0]);
    // Direction. An angle's row is a cross product over the lines' scale.
    holds_at("horizontal 2.0", &[(b, 1, 3.0)], &[3.0]);
    holds_at("vertical 2.1", &[(c, 0, 2.0)], &[2.0]);
    holds_at("parallel 2.0 2.2", &[(c, 1, 3.0)], &[3.75]);
    holds_at("perpendicular 2.0 2.1", &[(c, 0, 3.0)], &[-3.75]);
    holds_at("turned 2.0 2.1 0.5", &[(c, 0, 3.0)], &[-3.75]);
    holds_at("heading 2.1 0.5", &[(c, 0, 3.0)], &[-3.0]);
    holds_at("heading 2.0 0", &[(b, 1, 3.0)], &[3.0]);
    // Size.
    holds_at("distance 1.0 1.2 50", &[(c, 0, 1.0)], &[81.0 / 100.0]);
    holds_at("point-line 1.3 2.0 30 left", &[(d, 1, 2.0)], &[2.0]);
    holds_at(
        "point-line 1.17 2.0 5 right",
        &[(id(1, 17), 1, -2.0)],
        &[-2.0],
    );
    holds_at(
        "point-circle 1.12 3.0 10 outside",
        &[(u, 0, 2.0)],
        &[84.0 / 40.0],
    );
    holds_at(
        "point-circle 1.13 3.0 5 inside",
        &[(inner, 0, 1.0)],
        &[11.0 / 10.0],
    );
    holds_at("lines-apart 2.0 2.2 30 left", &[(d, 1, 2.0)], &[2.0]);
    holds_at("lines-apart 2.2 2.0 30 right", &[(a, 1, -2.0)], &[-2.0]);
    holds_at("across 1.0 1.1 40", &[(b, 0, 3.0)], &[3.0]);
    holds_at("upward 1.1 1.2 30", &[(c, 1, 3.0)], &[3.0]);
    holds_at("length 2.4 50", &[(c, 0, 1.0)], &[81.0 / 100.0]);
    holds_at("equal-length 2.0 2.2", &[(b, 0, 2.0)], &[164.0 / 64.0]);
    holds_at("radius 3.0 10", &[(id(3, 0), 0, 2.0)], &[2.0]);
    holds_at("diameter 3.0 20", &[(id(3, 0), 0, 2.0)], &[2.0]);
    holds_at("equal-radius 3.0 3.1", &[(id(3, 1), 0, 2.0)], &[2.0]);
    holds_at("offset 3.0 3.2 4 grows", &[(id(3, 2), 0, 1.0)], &[1.0]);
    holds_at("offset 3.2 3.0 4 shrinks", &[(id(3, 0), 0, 1.0)], &[1.0]);
    holds_at("sweep 4.0 0.5", &[(n, 0, 3.0)], &[-3.75]);
    for (arc, length, half_turns) in [(0, 5.0, 0.5), (1, 15.0, 1.5), (2, 10.0, 1.0)] {
        let rule = format!("arc-length 4.{arc} {}", length * PI);
        holds_at(&rule, &[(id(4, arc), 0, 1.0)], &[half_turns * PI]);
    }
    // Smoothness.
    holds_at("tangent-line 2.6 3.0 left", &[(id(3, 0), 0, 1.0)], &[-1.0]);
    holds_at("tangent-line 2.6 3.5 right", &[(id(3, 5), 0, 1.0)], &[1.0]);
    holds_at(
        "tangent-circles 3.0 3.3 outside",
        &[(id(3, 3), 0, 1.0)],
        &[-41.0 / 42.0],
    );
    holds_at(
        "tangent-circles 3.4 3.0 inside",
        &[(id(3, 4), 0, 1.0)],
        &[9.0 / 8.0],
    );
    holds_at(
        "tangent-at-end 2.7 4.0 1.9",
        &[(id(1, 14), 0, 1.0)],
        &[10.0 / 16.0],
    );
    holds_at(
        "arcs-tangent-at-end 4.0 4.3 1.9",
        &[(u, 1, 2.0)],
        &[-20.0 / 8.0],
    );
    // Symmetry and pattern.
    holds_at(
        "symmetric 1.0 1.1 2.8",
        &[(b, 0, 2.0), (b, 1, 2.0)],
        &[-75.0 / 64.0, 150.0 / 64.0],
    );
    holds_at("symmetric-about 1.4 1.0 1.1", &[(m, 0, 2.0)], &[-2.0, 0.0]);
    holds_at("spaced 1.0 1.4 1.1", &[(m, 0, 2.0)], &[-2.0, 0.0]);
    holds_at(
        "linear-pattern 1.0 1.1 1.3 1.2",
        &[(c, 0, 3.0), (c, 1, 1.0)],
        &[-3.0, -1.0],
    );
    holds_at(
        "circular-pattern 1.7 1.9 1.10 0.5",
        &[(n, 0, 1.0), (n, 1, 2.0)],
        &[-1.0, -2.0],
    );
    holds_at(
        "circular-pattern 1.7 1.9 1.10 0.5",
        &[(s, 1, 3.0)],
        &[-3.0, 0.0],
    );
    // Fixing and relations.
    holds_at("fix 1.0 0 0", &[(a, 0, 2.0)], &[2.0]);
    holds_at("fix 3.0 0 10", &[(id(3, 0), 0, 1.0)], &[1.0]);
    holds_at("linear 5.4 5.3 2 1", &[(id(5, 4), 0, 1.0)], &[1.0]);
    holds_at("linear 5.4 5.3 2 1", &[(id(5, 3), 0, 1.0)], &[-2.0]);
    holds_at("product 5.5 5.3 5.4", &[(id(5, 3), 0, 1.0)], &[-7.0]);
    // A named amount moves with its scalar.
    holds_at(
        "distance 1.0 1.2 @5.0",
        &[(id(5, 0), 0, 1.0)],
        &[-101.0 / 102.0],
    );
    holds_at("across 1.0 1.1 @5.7", &[(id(5, 7), 0, 1.0)], &[-1.0]);
}

/// A mixed drift off the solution, the same on every platform: a sum of coordinates would do as
/// well, but this leaves no two moves alike.
fn jiggled(a: &Assembly) -> Vec<f64> {
    let mut x = a.start().unwrap();
    let mut state = 0x0218_u64;
    for value in &mut x {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *value += ((state >> 40) as f64 / (1_u64 << 24) as f64 - 0.5) * 2.0;
    }
    x
}

fn assert_derivatives(s: &Sketch) {
    let mut a = Assembly::new(s).unwrap();
    let x = jiggled(&a);
    a.rescale(&x).unwrap();
    a.evaluate_at(&x).unwrap();
    let jacobian = a.jacobian().to_dense().unwrap();
    let rows = a.residuals().len();
    assert_eq!(jacobian.rows(), rows);
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
fn every_jacobian_entry_is_the_derivative_of_its_residual() {
    assert_derivatives(&scene());
}

#[test]
fn a_variable_twice_in_a_rule_adds_up() {
    // The point is an end of its own line, and the two lines share a corner: each variable has two
    // partial derivatives in one row, and the entry is their sum.
    for rule in [
        "on-line 1.0 2.0",
        "point-line 1.0 2.3 1 left",
        "perpendicular 2.0 2.1",
        "turned 2.0 2.3 0.25",
        "equal-length 2.0 2.1",
        "symmetric 1.0 1.2 2.0",
        "sweep 4.0 0.25",
    ] {
        assert_derivatives(&alone(rule).unwrap());
    }
    // On its own line a point's row is zero whatever moves, and so is its gradient.
    let s = alone("on-line 1.0 2.0").unwrap();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate_at(&jiggled(&a)).unwrap();
    let last = a.residuals().len() - 1;
    let dense = a.jacobian().to_dense().unwrap();
    for j in 0..a.variables().len() {
        assert!(dense.get(last, j).unwrap().abs() < 1e-12);
    }
}

#[test]
fn a_row_acts_as_a_length_at_its_solution() {
    // Section 15.4: one tolerance serves every constraint only if each row's gradient is near one.
    let s = scene();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate(&s).unwrap();
    let dense = a.jacobian().to_dense().unwrap();
    let keep = a.residuals().len() - 2 - 5;
    for i in 0..keep {
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
    // Zero-length lines: B on A, and C on D for a line's far end.
    let (a, b) = (id(1, 0), id(1, 1));
    for rule in [
        "on-line 1.4 2.0",
        "collinear 2.0 2.5",
        "parallel 2.0 2.2",
        "parallel 2.2 2.0",
        "perpendicular 2.0 2.1",
        "turned 2.0 2.1 0.5",
        "heading 2.0 0",
        "point-line 1.3 2.0 30 left",
        "lines-apart 2.0 2.2 30 left",
        "tangent-line 2.0 3.0 left",
        "symmetric 1.2 1.3 2.0",
    ] {
        let s = alone(rule).unwrap();
        let mut a0 = Assembly::new(&s).unwrap();
        let x = moved(&a0, &[(b, 0, -40.0)]);
        assert_eq!(
            a0.evaluate_at(&x),
            Err(SketchError::Degenerate(id(20, 0))),
            "{rule}"
        );
    }
    // A distance or a radius of nothing, by way of a scalar, and two that cancel.
    for (rule, owner, by) in [
        ("distance 1.0 1.2 @5.0", id(5, 0), -50.0),
        ("point-circle 1.12 3.0 @5.2 outside", id(5, 2), -20.0),
        ("point-circle 1.12 3.0 @5.2 inside", id(5, 2), 0.0),
        ("tangent-circles 3.0 3.1 inside", id(3, 0), 0.0),
    ] {
        let s = alone(rule).unwrap();
        let mut a0 = Assembly::new(&s).unwrap();
        let x = moved(&a0, &[(owner, 0, by)]);
        assert_eq!(
            a0.evaluate_at(&x),
            Err(SketchError::Degenerate(id(20, 0))),
            "{rule}"
        );
    }
    // An arc's sweep from a start or to an end at its centre.
    for point in [id(1, 9), id(1, 10)] {
        for rule in ["sweep 4.0 0.5", "arc-length 4.0 15"] {
            let s = alone(rule).unwrap();
            let mut a0 = Assembly::new(&s).unwrap();
            let shift = if point == id(1, 9) {
                [(point, 0, -10.0), (point, 1, 0.0)]
            } else {
                [(point, 0, 0.0), (point, 1, -10.0)]
            };
            let x = moved(&a0, &shift);
            assert_eq!(
                a0.evaluate_at(&x),
                Err(SketchError::Degenerate(id(20, 0))),
                "{rule}"
            );
        }
    }
    // A kind with no singular set takes a line of no length.
    let s = alone("equal-length 2.0 2.2").unwrap();
    let mut a0 = Assembly::new(&s).unwrap();
    let x = moved(&a0, &[(b, 0, -40.0)]);
    assert_eq!(a0.evaluate_at(&x), Ok(()));
    let _ = a;
}

#[test]
fn lengths_of_nothing_scale_by_one() {
    // Three points at one place give no scale, and the rule still reads its value.
    let text = format!(
        "{}entity 7.0 point 30 50\nentity 7.1 arc 1.7 0 1.9 1.9\n",
        bare()
    );
    assert_eq!(parse(&text), Err(SketchError::NotPositive));
    let text = format!(
        "{}entity 7.0 point 30 50\nentity 7.1 circle 7.0 1\nentity 7.2 point 30 50\nentity 7.3 arc 7.0 1 7.2 7.2\nentity 7.4 arc 7.0 1 7.2 7.2\nconstraint 20.0 driving 0 arcs-tangent-at-end 7.3 7.4 7.2\n",
        bare()
    );
    let s = parse(&text).unwrap();
    let mut a = Assembly::new(&s).unwrap();
    let x = moved(&a, &[(id(7, 2), 0, 3.0)]);
    a.evaluate_at(&x).unwrap();
    assert_eq!(*a.residuals().last().unwrap(), 0.0);
    let x = moved(&a, &[(id(7, 2), 0, 3.0), (id(7, 0), 1, 2.0)]);
    a.evaluate_at(&x).unwrap();
    assert_eq!(*a.residuals().last().unwrap(), 0.0);
}

#[test]
fn a_half_circle_has_a_smooth_length() {
    // The arctangent's branch sits half a turn from the start, so nothing jumps as the end passes
    // the half-turn, in either direction.
    let rule = format!("arc-length 4.2 {}", 10.0 * PI);
    let s = alone(&rule).unwrap();
    let mut a = Assembly::new(&s).unwrap();
    let mut read = |by: f64| {
        let x = moved(&a, &[(id(1, 11), 1, by)]);
        a.evaluate_at(&x).unwrap();
        *a.residuals().last().unwrap()
    };
    // W is at (10, 50), and y up turns it back: the arc grows as the end drops below the axis.
    let (down, up) = (read(-1e-6), read(1e-6));
    assert!((down - 1e-6).abs() < 1e-9, "{down}");
    assert!((up + 1e-6).abs() < 1e-9, "{up}");
    // Past half a turn at the start, the reading keeps its sweep.
    let rule = format!("arc-length 4.1 {}", 15.0 * PI);
    let s = alone(&rule).unwrap();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate(&s).unwrap();
    assert!(a.residuals().last().unwrap().abs() < 1e-12);
    // Rescaling at a new start moves the branch with it.
    let x = moved(&a, &[(id(1, 9), 0, -20.0)]);
    a.rescale(&x).unwrap();
    a.evaluate_at(&x).unwrap();
    assert!((a.residuals().last().unwrap() + 10.0 * PI).abs() < 1e-12);
}

#[test]
fn rescale_reads_the_lengths_at_the_values_it_is_given() {
    let s = alone("on-line 1.4 2.0").unwrap();
    let mut a = Assembly::new(&s).unwrap();
    // The line is now 88 long, so the scale is 64.
    let x = moved(&a, &[(id(1, 1), 0, 48.0), (id(1, 4), 1, 2.0)]);
    a.evaluate_at(&x).unwrap();
    let before = *a.residuals().last().unwrap();
    a.rescale(&x).unwrap();
    a.evaluate_at(&x).unwrap();
    let after = *a.residuals().last().unwrap();
    assert_eq!(before, 88.0 * 2.0 / 32.0);
    assert_eq!(after, 88.0 * 2.0 / 64.0);
    // The wrong length of x is refused, not read past.
    assert_eq!(a.rescale(&[0.0]), Err(SketchError::Internal));
    assert_eq!(a.evaluate_at(&[0.0]), Err(SketchError::Internal));
}

#[test]
fn a_reference_is_a_constant_and_never_a_column() {
    let s = alone("coincident 1.0 6.0").unwrap();
    assert!(s.variables().unwrap().iter().all(|v| v.owner != id(6, 0)));
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate_at(&moved(&a, &[(id(1, 0), 0, 3.0), (id(1, 0), 1, 4.0)]))
        .unwrap();
    let rows = a.residuals().len();
    assert_eq!(&a.residuals()[rows - 2..], [3.0, 4.0]);
    let (_, values) = (0..a.variables().len())
        .map(|j| a.jacobian().column(j).unwrap())
        .fold((0, 0), |(_, n), (_, v)| (0, n + v.len()));
    // Five entries in each of the arcs' eight rows, and the moving point's two in each of its own.
    assert_eq!(values, 8 * 5 + 2 * 2);
    assert_eq!(s.point(id(6, 1)), Ok([40.0, 0.0]));
    // A rule wholly on references is a row of no variables.
    let s = alone("horizontal 6.2").unwrap();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate(&s).unwrap();
    assert_eq!(*a.residuals().last().unwrap(), 0.0);
}

#[test]
fn structure_reads_the_catalogue() {
    // Eight rows from the four arcs' two ends each, all at their arcs' ids.
    let bare_sketch = parse(&bare()).unwrap();
    let sub = substitute(&bare_sketch).unwrap();
    assert_eq!(sub.graph.equations(), 8);
    assert_eq!(sub.owners[0], id(4, 0));
    assert_eq!(sub.owners[7], id(4, 3));
    // A concentric pair merges two pairs of variables, and leaves no equation.
    let plain = sub.graph.variables();
    for (rule, merged) in [
        ("concentric 3.0 3.1", 2),
        ("coincident 1.0 1.6", 2),
        ("equal-radius 3.0 3.1", 1),
        ("horizontal 2.0", 1),
        ("vertical 2.1", 1),
    ] {
        let sub = substitute(&alone(rule).unwrap()).unwrap();
        assert_eq!(sub.graph.equations(), 8, "{rule}");
        assert_eq!(sub.graph.variables(), plain - merged, "{rule}");
    }
    // Against a reference there is nothing to merge with: it stays an equation.
    let sub = substitute(&alone("coincident 1.0 6.0").unwrap()).unwrap();
    assert_eq!(sub.graph.equations(), 10);
    assert_eq!(sub.graph.variables(), plain);
    assert_eq!(sub.owners[9], id(20, 0));
    // A kind's equations are counted, and a shared name in a row comes once.
    let sub = substitute(&alone("collinear 2.0 2.5").unwrap()).unwrap();
    assert_eq!(sub.graph.equations(), 10);
    assert_eq!(sub.graph.row(8).unwrap().len(), 8);
    let sub = substitute(&alone("sweep 4.0 0.5").unwrap()).unwrap();
    assert_eq!(sub.graph.row(8).unwrap().len(), 6);
}

fn reads(rule: &str, want: f64) {
    let s = alone(rule).unwrap();
    let got = measure(&s, s.constraints().last().unwrap()).unwrap();
    assert!((got - want).abs() < 1e-12, "{rule}: {got}");
}

#[test]
fn every_dimension_measures() {
    reads("distance 1.0 1.2 1", 50.0);
    reads("distance 1.2 1.7 1", 800.0_f64.sqrt());
    reads("length 2.4 1", 50.0);
    reads("point-line 1.3 2.0 1 left", 30.0);
    reads("point-line 1.3 2.0 1 right", -30.0);
    reads("point-line 1.17 2.0 1 right", 5.0);
    reads("lines-apart 2.0 2.2 1 left", 30.0);
    reads("point-circle 1.12 3.0 1 outside", 10.0);
    reads("point-circle 1.13 3.0 1 inside", 5.0);
    reads("across 1.1 1.0 1", -40.0);
    reads("upward 1.1 1.2 1", 30.0);
    reads("radius 3.2 1", 14.0);
    reads("diameter 3.2 1", 28.0);
    reads("turned 2.0 2.1 1", 0.5);
    reads("turned 2.1 2.0 1", -0.5);
    reads("heading 2.1 1", 0.5);
    reads("sweep 4.0 1", 0.5);
    reads("sweep 4.1 1", 1.5);
    reads("arc-length 4.0 1", 5.0 * PI);
    reads("arc-length 4.1 1", 15.0 * PI);
    // Off the origin: a line at y = 30 and one at x = 40, a rise from (0, 30), and two slanted
    // lines, so no coordinate of the measure is zero.
    reads("point-line 1.12 2.2 1 left", 20.0);
    reads("point-line 1.3 2.1 1 left", 40.0);
    reads("upward 1.3 1.12 1", 20.0);
    // atan(4/3)/π: the angle from (40, 30) to (0, 30), in half-turns.
    reads("turned 2.4 2.1 1", 0.295_167_235_300_866_5);
    let s = alone("horizontal 2.0").unwrap();
    assert_eq!(
        measure(&s, s.constraints().last().unwrap()),
        Err(SketchError::NotADimension(id(20, 0)))
    );
}

#[test]
fn a_rule_on_lines_of_no_length_is_scaled_by_one() {
    // Both lines start with no length, so the parallel rule's length scale is 1, not the smallest
    // double: off that start its residual is the cross product of the two directions itself.
    let s = parse(
        "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\nentity 7.0 point 0 0\nentity 7.1 point 0 0\n\
         entity 7.2 point 5 5\nentity 7.3 point 5 5\nentity 8.0 line 7.0 7.1 segment\n\
         entity 8.1 line 7.2 7.3 segment\nconstraint 20.0 driving 0 parallel 8.0 8.1\n",
    )
    .unwrap();
    let mut a = Assembly::new(&s).unwrap();
    let x = moved(&a, &[(id(7, 1), 0, 1.0), (id(7, 3), 1, 1.0)]);
    a.evaluate_at(&x).unwrap();
    assert_eq!(a.residuals(), &[1.0]);
}

#[test]
fn an_arc_whose_ends_meet_sweeps_nothing() {
    // Sweeps run from 0 up to 2 half-turns, 2 not included: ends at one place sweep 0.
    let text = format!(
        "{}entity 7.0 point 30 50\nentity 7.1 arc 1.7 10 1.9 7.0\nconstraint 20.0 driving 0 sweep 7.1 1\n",
        bare()
    );
    let s = parse(&text).unwrap();
    assert_eq!(measure(&s, s.constraints().last().unwrap()), Ok(0.0));
}

#[test]
fn a_dimension_with_no_direction_is_degenerate() {
    let text = format!(
        "{}entity 7.0 line 1.0 1.6 segment\nentity 7.1 point 20 50\nentity 7.2 arc 1.7 10 7.1 1.9\nentity 7.3 arc 1.7 10 1.9 7.1\n",
        bare()
    );
    for rule in [
        "length 7.0 1",
        "point-line 1.2 7.0 1 left",
        "lines-apart 7.0 2.0 1 left",
        "turned 7.0 2.0 1",
        "turned 2.0 7.0 1",
        "heading 7.0 1",
        "sweep 7.2 1",
        "sweep 7.3 1",
        "arc-length 7.2 1",
    ] {
        let s = parse(&format!("{text}constraint 20.0 driving 0 {rule}\n")).unwrap();
        assert_eq!(
            measure(&s, s.constraints().last().unwrap()),
            Err(SketchError::Degenerate(id(20, 0))),
            "{rule}"
        );
    }
}

/// Whether `word` names an item of the scene, with or without its `@`.
fn item(s: &Sketch, word: &str) -> Option<ItemId> {
    let (sequence, key) = word.trim_start_matches('@').split_once('.')?;
    let name = id(sequence.parse().ok()?, key.parse().ok()?);
    s.entity(name).ok().map(|_| name)
}

#[test]
fn every_name_in_every_kind_is_checked() {
    let s = scene();
    let scalar = "5.0";
    let point = "1.0";
    for rule in rules() {
        let words: Vec<&str> = rule.split(' ').collect();
        for (place, word) in words.iter().enumerate().skip(1) {
            // The sweep's 1.5 is a number that reads as a point's name.
            let Some(name) = item(&s, word).filter(|_| rule != "sweep 4.1 1.5" || place == 1)
            else {
                continue;
            };
            let with = |other: &str| {
                let mut changed = words.clone();
                let replaced = format!("{}{other}", if word.starts_with('@') { "@" } else { "" });
                changed[place] = &replaced;
                alone(&changed.join(" "))
            };
            assert_eq!(
                with("99.99"),
                Err(SketchError::Unknown(id(99, 99))),
                "{rule}"
            );
            if words[0] == "fix" {
                continue;
            }
            let wrong = if s.point(name).is_ok() { scalar } else { point };
            assert!(
                matches!(with(wrong), Err(SketchError::WrongKind(_))),
                "{rule}: {place}"
            );
        }
    }
    assert_eq!(
        alone("fix 1.0 2 0").unwrap_err(),
        SketchError::WrongKind(id(1, 0))
    );
    assert_eq!(
        alone("fix 3.0 1 0").unwrap_err(),
        SketchError::WrongKind(id(3, 0))
    );
    assert!(alone("fix 1.0 1 0").is_ok());
}

#[test]
fn a_positive_value_must_be_positive_and_a_signed_one_finite() {
    for rule in [
        "distance 1.0 1.2",
        "point-line 1.3 2.0",
        "point-circle 1.12 3.0",
        "lines-apart 2.0 2.2",
        "length 2.4",
        "radius 3.0",
        "diameter 3.0",
        "arc-length 4.0",
    ] {
        let tail = match rule.split(' ').next().unwrap() {
            "point-line" | "lines-apart" => " left",
            "point-circle" => " outside",
            _ => "",
        };
        for value in ["0", "-1", "-0"] {
            assert_eq!(
                alone(&format!("{rule} {value}{tail}")).unwrap_err(),
                SketchError::NotPositive,
                "{rule} {value}"
            );
        }
        assert!(alone(&format!("{rule} 1{tail}")).is_ok());
    }
    for rule in [
        "across 1.0 1.1 -4",
        "across 1.0 1.1 0",
        "upward 1.0 1.1 -4",
        "offset 3.0 3.1 -1 grows",
        "offset 3.0 3.1 0 shrinks",
        "turned 2.0 2.1 -0.5",
        "turned 2.0 2.1 0",
        "heading 2.0 -0.5",
        "sweep 4.0 0",
        "circular-pattern 1.7 1.9 1.10 -1",
    ] {
        assert!(alone(rule).is_ok(), "{rule}");
    }
    let mut s = parse(&bare()).unwrap();
    let nan = Amount::Value(f64::NAN);
    let (a, b, arc) = (id(1, 0), id(1, 1), id(4, 0));
    for kind in [
        ConstraintKind::Distance {
            a,
            b,
            distance: nan,
        },
        ConstraintKind::Across {
            a,
            b,
            distance: nan,
        },
        ConstraintKind::Upward {
            a,
            b,
            distance: nan,
        },
        ConstraintKind::Sweep { arc, angle: nan },
        ConstraintKind::Heading {
            line: id(2, 0),
            angle: nan,
        },
        ConstraintKind::Turned {
            a: id(2, 0),
            b: id(2, 1),
            angle: nan,
        },
        ConstraintKind::CircularPattern {
            centre: a,
            a: b,
            b: id(1, 2),
            angle: nan,
        },
        ConstraintKind::Offset {
            from: id(3, 0),
            to: id(3, 1),
            by: nan,
            grows: true,
        },
        ConstraintKind::Linear {
            a: id(5, 0),
            b: id(5, 1),
            ratio: f64::NAN,
            plus: 0.0,
        },
        ConstraintKind::Linear {
            a: id(5, 0),
            b: id(5, 1),
            ratio: 0.0,
            plus: f64::INFINITY,
        },
        ConstraintKind::Fix {
            entity: a,
            slot: 0,
            value: f64::NAN,
        },
    ] {
        let c = Constraint {
            id: id(20, 0),
            kind,
            standing: Standing::Driving,
            priority: 0,
        };
        assert_eq!(s.add_constraint(c), Err(SketchError::NotFinite), "{kind:?}");
    }
}

#[test]
fn only_a_dimension_can_be_a_reference() {
    let dimensions = [
        "distance",
        "point-line",
        "point-circle",
        "lines-apart",
        "across",
        "upward",
        "length",
        "radius",
        "diameter",
        "turned",
        "heading",
        "sweep",
        "arc-length",
    ];
    for rule in rules() {
        let text = format!("{}constraint 20.0 reference 0 {rule}\n", bare());
        let tag = rule.split(' ').next().unwrap();
        if dimensions.contains(&tag) {
            let s = parse(&text).unwrap();
            let c = s.constraints().last().unwrap();
            assert!(c.kind.is_dimension());
            // Measured, and never a row.
            let plain = Assembly::new(&parse(&bare()).unwrap()).unwrap();
            let held = Assembly::new(&s).unwrap();
            assert_eq!(held.jacobian().rows(), plain.jacobian().rows());
        } else {
            assert_eq!(
                parse(&text).unwrap_err(),
                SketchError::NotADimension(id(20, 0)),
                "{rule}"
            );
        }
    }
}

#[test]
fn text_refuses_what_is_not_an_amount_a_side_or_a_tangency() {
    for rule in [
        "distance 1.0 1.2 @",
        "distance 1.0 1.2 @5",
        "distance 1.0 1.2 @x.0",
        "distance 1.0 1.2 @5.x",
        "distance 1.0 1.2 inf",
        "distance 1.0 1.2 NaN",
        "distance 1.0 1.2 five",
        "point-line 1.3 2.0 1 up",
        "point-circle 1.12 3.0 1 around",
        "tangent-circles 3.0 3.1 across",
        "offset 3.0 3.1 1 stays",
        "lines-apart 2.0 2.2 1 down",
        "tangent-line 2.6 3.0 middle",
        "radius 3.0",
        "horizontal 2.0 2.1",
        "no-such-kind 1.0",
    ] {
        assert!(
            matches!(alone(rule), Err(SketchError::Text { .. })),
            "{rule}"
        );
    }
}

#[test]
fn each_kind_has_its_commit_class() {
    let class = |rule: &str| alone(rule).unwrap().constraints()[0].kind.commit();
    for rule in [
        "coincident 1.0 1.6",
        "concentric 3.0 3.1",
        "on-line 1.4 2.0",
        "collinear 2.0 2.5",
        "horizontal 2.0",
        "vertical 2.1",
        "parallel 2.0 2.2",
        "perpendicular 2.0 2.1",
        "across 1.0 1.1 40",
        "upward 1.1 1.2 30",
        "equal-radius 3.0 3.1",
        "tangent-at-end 2.7 4.0 1.9",
        "fix 1.0 0 0",
        "turned 2.0 2.1 0.5",
        "turned 2.0 2.1 -1",
        "heading 2.1 0.5",
        "heading 2.1 0",
    ] {
        assert_eq!(class(rule), CommitClass::Exact, "{rule}");
    }
    for rule in [
        "distance 1.0 1.2 50",
        "length 2.4 50",
        "on-circle 1.9 3.0",
        "midpoint 1.4 1.0 1.1",
        "turned 2.0 2.1 0.25",
        "turned 2.0 2.1 @5.1",
        "heading 2.1 0.25",
        "heading 2.1 @5.1",
        "arcs-tangent-at-end 4.0 4.3 1.9",
        "tangent-circles 3.0 3.3 outside",
        "symmetric 1.0 1.1 2.8",
        "radius 3.0 10",
    ] {
        assert_eq!(class(rule), CommitClass::Bounded, "{rule}");
    }
}

fn circle_sketch(centre: [f64; 2], radius: f64, far: [f64; 2], other: f64) -> Sketch {
    parse(&format!(
        "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\nentity 1.0 point 0 0\nentity 1.1 point {} {}\nentity 1.2 point {} {}\nentity 2.0 circle 1.0 {radius}\nentity 2.1 circle 1.1 {other}\n",
        centre[0], centre[1], far[0], far[1]
    ))
    .unwrap()
}

#[test]
fn circles_read_their_root_choices_off_the_drawing() {
    let (p, c) = (id(1, 2), id(2, 0));
    let s = circle_sketch([9.0, 0.0], 5.0, [13.0, 0.0], 1.0);
    assert_eq!(s.drawn_outside(p, c), Ok(true));
    let s = circle_sketch([9.0, 0.0], 5.0, [3.0, 0.0], 1.0);
    assert_eq!(s.drawn_outside(p, c), Ok(false));
    let undecided = Err(SketchError::RootChoiceUndetermined(p));
    let s = circle_sketch([9.0, 0.0], 5.0, [3.0, 4.0], 1.0);
    assert_eq!(s.drawn_outside(p, c), undecided);
    let s = circle_sketch([9.0, 0.0], 5.0, [3.5, 4.0], 1.0);
    assert_eq!(s.drawn_outside(p, c), undecided);
    let s = circle_sketch([9.0, 0.0], 2.5, [3.0, 4.0], 1.0);
    assert_eq!(s.drawn_outside(p, c), undecided);
    assert_eq!(s.drawn_outside(c, c), Err(SketchError::WrongKind(c)));
    assert_eq!(s.drawn_outside(p, p), Err(SketchError::WrongKind(p)));

    let (a, b) = (id(2, 0), id(2, 1));
    let s = circle_sketch([10.0, 0.0], 4.0, [0.0, 0.0], 5.0);
    assert_eq!(s.drawn_touching_outside(a, b), Ok(true));
    let s = circle_sketch([1.0, 0.0], 4.0, [0.0, 0.0], 5.0);
    assert_eq!(s.drawn_touching_outside(a, b), Ok(false));
    let undecided = Err(SketchError::RootChoiceUndetermined(a));
    let s = circle_sketch([5.0, 0.0], 3.0, [0.0, 0.0], 4.0);
    assert_eq!(s.drawn_touching_outside(a, b), undecided);
    let s = circle_sketch([5.0, 0.5], 3.0, [0.0, 0.0], 4.0);
    assert_eq!(s.drawn_touching_outside(a, b), undecided);
    let s = circle_sketch([5.0, 0.0], 2.5, [0.0, 0.0], 4.0);
    assert_eq!(s.drawn_touching_outside(a, b), undecided);
    let s = circle_sketch([5.0, 0.0], 3.0, [0.0, 0.0], 4.5);
    assert_eq!(s.drawn_touching_outside(a, b), undecided);
    assert_eq!(
        s.drawn_touching_outside(p, b),
        Err(SketchError::WrongKind(p))
    );
    assert_eq!(
        s.drawn_touching_outside(a, p),
        Err(SketchError::WrongKind(p))
    );
}

#[test]
fn fixing_a_whole_entity_fixes_its_points() {
    let s = parse(&bare()).unwrap();
    let names = |list: &[Constraint]| -> Vec<(ItemId, u8, f64)> {
        list.iter()
            .map(|c| match c.kind {
                ConstraintKind::Fix {
                    entity,
                    slot,
                    value,
                } => (entity, slot, value),
                _ => panic!("a fix"),
            })
            .collect()
    };
    let line = fix_entity(&s, id(2, 0), 30, 4).unwrap();
    assert_eq!(
        names(&line),
        [
            (id(1, 0), 0, 0.0),
            (id(1, 0), 1, 0.0),
            (id(1, 1), 0, 40.0),
            (id(1, 1), 1, 0.0)
        ]
    );
    assert_eq!(line[3].id, id(30, 7));
    assert_eq!(line[3].standing, Standing::Driving);
    assert_eq!(fix_entity(&s, id(3, 0), 30, 0).unwrap().len(), 3);
    assert_eq!(fix_entity(&s, id(4, 0), 30, 0).unwrap().len(), 7);
    assert_eq!(fix_entity(&s, id(1, 0), 30, 0).unwrap().len(), 2);
    assert_eq!(fix_entity(&s, id(5, 0), 30, 0).unwrap().len(), 1);
    assert_eq!(fix_entity(&s, id(6, 0), 30, 0).unwrap().len(), 0);
    assert_eq!(
        fix_entity(&s, id(98, 0), 30, 0).unwrap_err(),
        SketchError::Unknown(id(98, 0))
    );
    assert_eq!(
        fix_entity(&s, id(2, 0), 30, u64::MAX).unwrap_err(),
        SketchError::Internal
    );
    let mut fixed = s.clone();
    for c in line {
        fixed.add_constraint(c).unwrap();
    }
    let mut a = Assembly::new(&fixed).unwrap();
    a.evaluate(&fixed).unwrap();
    assert!(a.residuals().iter().all(|r| *r == 0.0));

    let curves = [
        (
            EntityKind::Ellipse {
                centre: id(1, 0),
                axis: [3.0, 0.0],
                minor: 1.0,
            },
            5,
        ),
        (
            EntityKind::Spline {
                degree: 1,
                points: vec![id(1, 0), id(1, 1)],
                knots: vec![0.0, 0.0, 1.0, 1.0],
                weights: vec![1.0, 1.0],
            },
            4,
        ),
        (
            EntityKind::Parameter {
                curve: id(1, 0),
                t: 0.5,
                speed: 1.0,
            },
            3,
        ),
    ];
    for (kind, count) in curves {
        let mut held = s.clone();
        held.add_entity(Entity {
            id: id(9, 0),
            kind,
            construction: false,
        })
        .unwrap();
        assert_eq!(fix_entity(&held, id(9, 0), 30, 0).unwrap().len(), count);
    }
}

#[test]
fn an_offset_chain_ties_each_piece_to_its_copy() {
    let s = parse(&bare()).unwrap();
    let by = Amount::Named(id(5, 2));
    let list = offset_chain(
        &s,
        &[
            (id(2, 0), id(2, 2)),
            (id(4, 0), id(4, 1)),
            (id(3, 0), id(3, 2)),
        ],
        by,
        Side::Left,
        30,
        2,
    )
    .unwrap();
    let kinds: Vec<ConstraintKind> = list.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        [
            ConstraintKind::Parallel {
                a: id(2, 0),
                b: id(2, 2)
            },
            ConstraintKind::LinesApart {
                a: id(2, 0),
                b: id(2, 2),
                distance: by,
                side: Side::Left
            },
            ConstraintKind::Concentric {
                a: id(4, 0),
                b: id(4, 1)
            },
            ConstraintKind::Offset {
                from: id(4, 0),
                to: id(4, 1),
                by,
                grows: false
            },
            ConstraintKind::Concentric {
                a: id(3, 0),
                b: id(3, 2)
            },
            ConstraintKind::Offset {
                from: id(3, 0),
                to: id(3, 2),
                by,
                grows: false
            },
        ]
    );
    assert_eq!(list[5].id, id(30, 7));
    let right = offset_chain(&s, &[(id(3, 0), id(3, 2))], by, Side::Right, 30, 0).unwrap();
    assert_eq!(
        right[1].kind,
        ConstraintKind::Offset {
            from: id(3, 0),
            to: id(3, 2),
            by,
            grows: true
        }
    );
    for piece in [
        (id(2, 0), id(3, 0)),
        (id(3, 0), id(2, 0)),
        (id(1, 0), id(2, 0)),
    ] {
        assert!(matches!(
            offset_chain(&s, &[piece], by, Side::Left, 30, 0),
            Err(SketchError::WrongKind(_))
        ));
    }
    // A changed distance is a change of value: the rows do not change.
    let mut held = s.clone();
    for c in list {
        held.add_constraint(c).unwrap();
    }
    assert!(Assembly::new(&held).is_ok());
}

#[test]
fn catalogue_golden_hash_is_the_same_on_every_target() {
    let bytes = {
        let s = scene();
        let mut bytes = write(&s).unwrap().into_bytes();
        let mut a = Assembly::new(&s).unwrap();
        let x = jiggled(&a);
        a.rescale(&x).unwrap();
        a.evaluate_at(&x).unwrap();
        for value in a.residuals() {
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        let jacobian = a.jacobian();
        for j in 0..jacobian.cols() {
            let (rows, values) = jacobian.column(j).unwrap();
            for (row, value) in rows.iter().zip(values) {
                bytes.extend_from_slice(&(*row as u64).to_le_bytes());
                bytes.extend_from_slice(&value.to_bits().to_le_bytes());
            }
        }
        bytes
    };
    let checked = golden::check("geom-sketch/catalogue", || (sha256(&bytes), bytes.clone()));
    println!(
        "catalogue corpus: {} bytes, golden hash {}",
        checked.value.len(),
        hex(sha256(&checked.value))
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!("{e}. A changed digest is a bug, or a definition changed in place (chapter 26.2)")
    });
}

#[test]
fn a_scale_past_the_doubles_is_refused() {
    // One over the smallest double is an infinity, which no variable can be scaled by.
    for entity in [
        "parameter 1.0 0 5e-324",
        "scalar 0 5e-324",
        "placement 0 0 0 5e-324",
    ] {
        assert_eq!(
            parse(&format!("{}entity 7.0 {entity}\n", bare())),
            Err(SketchError::NotFinite),
            "{entity}"
        );
    }
}
