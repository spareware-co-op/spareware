//! Inputs for chapter 17.2's survey points and linework (piece G361, decision 32.44): figures for
//! `geom-curve`'s linework, datasets for `geom-2d`'s survey import, and IDs for a first import's
//! keys.
//!
//! - `linework`, class `typical`: a figure of eight points within ±2^30 steps: a line, an arc of
//!   radius 2^16 to 2^24 steps (about 10 mm to 2.6 m at the building step) through three points
//!   on its circle at random heights, a smooth figure through three more points, and a line.
//! - `import`, class `typical`: datasets of [`POINTS`] points within ±2^30 steps, with IDs `P` and
//!   a number, each unique in its dataset.
//! - `first_import`, class `typical`: IDs of 1 to 40 characters, digits, letters and a space.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a batch is
//! `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: one class per operation, as chapter 28.14 sets benchmarks out
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: a few dozen draws of [`SplitMix64`] per figure or dataset, once per batch, outside every
//! measured run.

use geom_curve::Segment;
use geom_num::grid::P3;
use geom_num::math;
use geom_nurbs::Parametrization;

use super::{Class, TYPICAL_REACH, batch_of, rng};
use crate::rng::SplitMix64;

/// The points of each dataset an import benchmark imports.
pub const POINTS: usize = 16;

/// A value in `0..n`, for `n` below 2^62.
fn below(g: &mut SplitMix64, n: u64) -> i64 {
    #[allow(
        clippy::cast_possible_wrap,
        reason = "every caller passes n below 2^62, so the remainder fits i64"
    )]
    let v = (g.next_u64() % n) as i64;
    v
}

/// A value in `[0, 1)`, from the top 53 bits of a draw.
fn unit(g: &mut SplitMix64) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "53 bits, exactly a double, over 2^53, exactly a double"
    )]
    let v = (g.next_u64() >> 11) as f64 / (1_u64 << 53) as f64;
    v
}

/// A typical grid coordinate, within ±2^30 steps.
fn coordinate(g: &mut SplitMix64) -> i64 {
    below(g, 2 * TYPICAL_REACH.unsigned_abs() + 1) - TYPICAL_REACH
}

/// The point `d` steps away from `p` in plan, at height `z`, each offset rounded to the grid, ties
/// to even, by the kernel's deterministic rounding.
fn offset(p: P3, d: (f64, f64), z: i64) -> Option<P3> {
    let whole = |v: f64| {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a whole double below 2^26 in magnitude, far inside i64"
        )]
        let w = math::round_ties_even(v).ok()?.get() as i64;
        Some(w)
    };
    P3::new(p.x.get() + whole(d.0)?, p.y.get() + whole(d.1)?, z).ok()
}

/// The point at `turn` half-turns on the circle of `radius` about (`cx`, `cy`), by the kernel's
/// `sinpi` and `cospi`, so every target draws the same figure.
fn on_circle(cx: f64, cy: f64, radius: f64, turn: f64) -> Option<(f64, f64)> {
    Some((
        cx + radius * math::cospi(turn).ok()?.get(),
        cy + radius * math::sinpi(turn).ok()?.get(),
    ))
}

/// Whether three points turn in plan, exactly: their cross product is not zero.
fn turns(a: P3, m: P3, b: P3) -> bool {
    let d = |p: P3, q: P3| {
        (
            i128::from(q.x.get() - p.x.get()),
            i128::from(q.y.get() - p.y.get()),
        )
    };
    let ((mx, my), (bx, by)) = (d(a, m), d(a, b));
    mx * by - my * bx != 0
}

/// One figure: a line, an arc, a smooth figure through three more points, and a line.
fn figure(g: &mut SplitMix64) -> Option<(Vec<P3>, Vec<Segment>)> {
    let height = |g: &mut SplitMix64| below(g, 1 << 20) - (1 << 19);
    let start = P3::new(coordinate(g), coordinate(g), height(g)).ok()?;
    // A radius from 2^16 to 2^24 steps, and a heading anywhere, in half-turns.
    let radius = 65_536.0 * (1.0 + 255.0 * unit(g));
    let heading = 2.0 * unit(g);
    let line_end = offset(
        start,
        on_circle(0.0, 0.0, 2.0 * radius, heading)?,
        height(g),
    )?;
    // The arc's circle has its center a radius to the left of the line's end, so the line's end
    // lies on it a quarter turn back from the heading, and the arc turns left from there.
    let (cx, cy) = on_circle(0.0, 0.0, radius, heading + 0.5)?;
    let from = heading - 0.5;
    let sweep = 0.1 + 0.8 * unit(g);
    let middle = offset(
        line_end,
        on_circle(cx, cy, radius, from + sweep * 0.5)?,
        height(g),
    )?;
    let arc_end = offset(
        line_end,
        on_circle(cx, cy, radius, from + sweep)?,
        height(g),
    )?;
    if !turns(line_end, middle, arc_end) {
        return None;
    }
    let mut points = vec![start, line_end, middle, arc_end];
    let mut last = arc_end;
    for _ in 0..3 {
        let step = ((unit(g) - 0.2) * radius, (unit(g) - 0.2) * radius);
        last = offset(last, step, height(g))?;
        points.push(last);
    }
    points.push(offset(last, (radius, -radius), height(g))?);
    if points.windows(2).any(|w| w.first() == w.get(1)) {
        return None;
    }
    let segments = vec![
        Segment::Line,
        Segment::Arc,
        Segment::Smooth {
            points: 3,
            rule: Parametrization::Centripetal,
        },
        Segment::Line,
    ];
    Some((points, segments))
}

/// Figures for `linework`.
#[must_use]
pub fn linework(class: Class) -> Vec<(Vec<P3>, Vec<Segment>)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("survey::linework", class);
    batch_of(|_| figure(&mut g))
}

/// Datasets of [`POINTS`] points with unique IDs, for `survey::import`.
#[must_use]
pub fn import(class: Class) -> Vec<Vec<(String, P3)>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("survey::import", class);
    batch_of(|at| {
        (0..POINTS)
            .map(|k| {
                let id = format!("P{}", at * 1_000 + k * 7 + 1);
                let p = P3::new(
                    coordinate(&mut g),
                    coordinate(&mut g),
                    below(&mut g, 1 << 24),
                );
                p.ok().map(|p| (id, p))
            })
            .collect()
    })
}

/// IDs for `PointKey::first_import`.
#[must_use]
pub fn first_import(class: Class) -> Vec<String> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("survey::first_import", class);
    const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz -";
    batch_of(|_| {
        let len = 1 + below(&mut g, 40).unsigned_abs();
        let id: String = (0..len)
            .filter_map(|_| {
                CHARS
                    .get(below(&mut g, CHARS.len() as u64).unsigned_abs() as usize)
                    .map(|&c| char::from(c))
            })
            .collect();
        Some(id)
    })
}
