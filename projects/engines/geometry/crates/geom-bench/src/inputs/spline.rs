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
