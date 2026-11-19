//! Inputs for `geom-sketch`'s spline rules (piece G220, decision 32.44): small sketches in the
//! `sketch v1` text form.
//!
//! Class `typical` only: cubic splines whose control points advance in x, so their speed is never
//! zero, for the arc length; and two rational quadratic splines with a G2 join, a G3 end against a
//! line, a pinned point and a stated length, for the assembly of the rules. The text is the input,
//! so the count includes reading it. The refusals are tested in `geom-sketch` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the splines of `geom-sketch`'s `tests/spline_more.rs`, with the points drawn from
//! [`SplitMix64`]. Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few draws of [`SplitMix64`] and a short text per input, once per batch, outside every
//! measured run.

use core::fmt::Write as _;

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// A cubic Bezier spline `2.0` on four points, each 100 to 160 right of the last.
fn cubic(g: &mut SplitMix64) -> String {
    let mut text = String::from("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n");
    for i in 0..4_i64 {
        let _ = writeln!(
            text,
            "entity 1.{i} point {} {}",
            i * 130 + g.range(0, 30),
            g.range(-90, 90)
        );
    }
    text.push_str("entity 2.0 spline 3 4 1.0 1.1 1.2 1.3 8 0 0 0 0 1 1 1 1 4 1 1 1 1\n");
    text
}

/// Two rational quadratics joined end to start, a line and a pinned point, with the rules on them.
fn rational(g: &mut SplitMix64) -> String {
    let mut text = String::from("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n");
    let corner = |g: &mut SplitMix64| (g.range(80, 140), g.range(-60, 60));
    let mut at = (0_i64, 0_i64);
    let mut points = Vec::new();
    for i in 0..6 {
        if i == 3 {
            // The second spline starts where the first ended.
            points.push(at);
            continue;
        }
        let (dx, dy) = corner(g);
        at = (at.0 + dx, at.1 + dy);
        points.push(at);
    }
    for (i, (x, y)) in points.iter().enumerate() {
        let _ = writeln!(text, "entity 1.{i} point {x} {y}");
    }
    let (first, second) = (
        points.first().copied().unwrap_or((0, 0)),
        points.get(1).copied().unwrap_or((0, 0)),
    );
    let _ = writeln!(text, "entity 1.6 point {} {}", first.0, first.1);
    let _ = writeln!(
        text,
        "entity 1.7 point {} {}",
        first.0 - 90,
        first.1 + g.range(-20, 20)
    );
    let _ = writeln!(text, "entity 1.8 point {} {}", second.0, second.1);
    let weight = |g: &mut SplitMix64| 0.5 + g.range(0, 150) as f64 / 100.0;
    let (w1, w2) = (weight(g), weight(g));
    let _ = writeln!(
        text,
        "entity 2.0 spline 2 3 1.0 1.1 1.2 6 0 0 0 1 1 1 3 1 {w1} 1"
    );
    let _ = writeln!(
        text,
        "entity 2.1 spline 2 3 1.3 1.4 1.5 6 0 0 0 1 1 1 3 1 {w2} 1"
    );
    text.push_str("entity 2.2 line 1.6 1.7 segment\n");
    text.push_str("constraint 3.0 driving 0 spline-join 2.0 finish 2.1 start g2\n");
    text.push_str("constraint 3.1 driving 0 spline-against 2.0 start 2.2 start g3\n");
    text.push_str("constraint 3.2 driving 0 spline-through 2.1 1.8 0.4\n");
    text.push_str("constraint 3.3 driving 0 spline-length 2.1 400\n");
    text
}

/// Sketches for `spline_length`: cubic splines of no zero speed.
#[must_use]
pub fn lengths(class: Class) -> Vec<String> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("spline::lengths", class);
    batch_of(|_| Some(cubic(&mut g)))
}

/// Sketches for the assembly of the spline rules: rational splines with the new rules.
#[must_use]
pub fn rational_rules(class: Class) -> Vec<String> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("spline::rational_rules", class);
    batch_of(|_| Some(rational(&mut g)))
}

/// A cubic cut at its middle, the right half pulled off the join by a few steps, with a join of
/// the stated order: the points of both halves held but the right half's inner two.
fn pulled(g: &mut SplitMix64, order: &str) -> String {
    let p = [[0.0, 0.0], [8.0, 16.0], [24.0, 20.0], [32.0, 0.0]];
    let mid = |a: [f64; 2], b: [f64; 2]| [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let (q0, q1, q2) = (mid(p[0], p[1]), mid(p[1], p[2]), mid(p[2], p[3]));
    let (r0, r1) = (mid(q0, q1), mid(q1, q2));
    let s = mid(r0, r1);
    let left = [p[0], q0, r0, s];
    let right = [s, r1, q2, p[3]];
    let shift = 1.0 + f64::from(i32::try_from(g.range(0, 30)).unwrap_or(0)) / 10.0;
    let mut text = String::from("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n");
    for (k, c) in left.iter().enumerate() {
        let _ = writeln!(text, "entity 1.{k} point {} {}", c[0], c[1]);
    }
    for (k, c) in right.iter().enumerate() {
        let (dx, dy) = if k == 1 || k == 2 {
            (shift, -shift)
        } else {
            (0.0, 0.0)
        };
        let _ = writeln!(text, "entity 1.{} point {} {}", 4 + k, c[0] + dx, c[1] + dy);
    }
    for (index, first) in [(0, 0), (1, 4)] {
        let _ = writeln!(
            text,
            "entity 2.{index} spline 3 4 1.{first} 1.{} 1.{} 1.{} 8 0 0 0 0 1 1 1 1 4 1 1 1 1",
            first + 1,
            first + 2,
            first + 3
        );
    }
    let mut key = 0;
    for (k, c) in left.iter().enumerate() {
        let _ = writeln!(
            text,
            "constraint 3.{key} driving 0 fix 1.{k} 0 {}\nconstraint 3.{} driving 0 fix 1.{k} 1 {}",
            c[0],
            key + 1,
            c[1]
        );
        key += 2;
    }
    let _ = writeln!(
        text,
        "constraint 3.{key} driving 0 fix 1.7 0 {}\nconstraint 3.{} driving 0 fix 1.7 1 {}",
        right[3][0],
        key + 1,
        right[3][1]
    );
    text.push_str("constraint 4.0 driving 0 coincident 1.3 1.4\n");
    let _ = writeln!(
        text,
        "constraint 4.1 driving 0 spline-join 2.0 finish 2.1 start {order}"
    );
    text
}

/// A fit-point spline through six points, the middle one held away from where the spline had it.
fn dragged(g: &mut SplitMix64) -> String {
    use geom_sketch::model::ItemId;
    let data = [
        [0.0, 0.0],
        [10.0, 6.0],
        [22.0, 9.0],
        [31.0, 7.0],
        [40.0, 1.0],
        [52.0, -3.0],
    ];
    let mut text = String::from("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n");
    for (k, p) in data.iter().enumerate() {
        let _ = writeln!(text, "entity 1.{k} point {} {}", p[0], p[1]);
    }
    let Ok(mut sketch) = geom_sketch::text::parse(&text) else {
        return text;
    };
    let fit_points: Vec<ItemId> = (0..6).map(|k| ItemId::new(1, k)).collect();
    if let Ok(made) = geom_sketch::fit::author(&sketch, &fit_points, 3, ItemId::new(2, 0), 1, 100) {
        for e in made.entities {
            let _ = sketch.add_entity(e);
        }
        for c in made.constraints {
            let _ = sketch.add_constraint(c);
        }
    }
    let mut out = geom_sketch::text::write(&sketch).unwrap_or(text);
    let tenth = |v: i64| f64::from(i32::try_from(v).unwrap_or(0)) / 10.0;
    let (mx, my) = (22.0 + tenth(g.range(0, 80)), 9.0 + tenth(g.range(0, 80)));
    let _ = writeln!(
        out,
        "constraint 3.0 driving 0 fix 1.0 0 0\nconstraint 3.1 driving 0 fix 1.0 1 0\nconstraint 3.2 driving 0 fix 1.2 0 {mx}\nconstraint 3.3 driving 0 fix 1.2 1 {my}\nconstraint 3.4 driving 0 fix 1.5 0 52\nconstraint 3.5 driving 0 fix 1.5 1 -3"
    );
    out
}

/// Sketches for the solver on a pulled join: cut cubics with a join of each order.
#[must_use]
pub fn pulled_joins(class: Class) -> Vec<String> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("spline::pulled_joins", class);
    batch_of(|i| {
        let order = ["g1", "g2", "g3"].get(i % 3).copied().unwrap_or("g1");
        Some(pulled(&mut g, order))
    })
}

/// Sketches for the settling of a fit-point spline.
#[must_use]
pub fn dragged_fits(class: Class) -> Vec<String> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("spline::dragged_fits", class);
    batch_of(|_| Some(dragged(&mut g)))
}
