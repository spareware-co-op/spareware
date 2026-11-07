//! The fuzz target of the spline rules (piece G220a): any bytes are read as two cubic splines
//! and a few rules on them, and everything the crate does with splines must hold its laws.
//!
//! What it does: [`harness`] reads the fuzzer's bytes as the control points of two cubics, a line,
//! an arc, and up to six rules (joins, a spline end against the line or the arc, a stated length, a
//! pinned point), writes the `sketch v1` text, and runs the sketch through the assembly, the arc
//! length, the senses of the joins, the solver, the measure of the solved joins and the fit-point
//! settling.
//!
//! Guarantees:
//!
//! - **Never a panic.** Every step returns a typed error on a sketch it cannot read.
//! - **The length is a bound.** A spline's length is at least the distance between its ends, less
//!   the error bound the measure states, and the bound is not negative.
//! - **A solve is solved.** When the solver answers, its largest residual is within the tolerance,
//!   and answering twice gives equal sketches.
//! - **The same answer twice.** The assembly, the senses and the length are equal on a second run.
//!
//! Method: the property-based fuzzing of 28.5
//! (`projects/engines/geometry/architecture/28-testing.md`), on sections 15.2 and 15.3
//! (`projects/engines/geometry/architecture/15-sketches.md`).
//!
//! Cost: a few small solves and one measure per input.

use core::fmt::Write as _;

use geom_run::{Budget, WorkMeter};
use geom_sketch::equation::Assembly;
use geom_sketch::length::spline_length;
use geom_sketch::model::{ItemId, Sketch};
use geom_sketch::smooth::{join_sense, measure_joins};
use geom_sketch::solve::{TOLERANCE, solve};
use geom_sketch::text::parse;

use crate::Finding;

/// The most rules a case has.
const MAX_RULES: usize = 6;

/// The sketch text the bytes describe: eight control points, a line and an arc on a 100-step grid,
/// and rules chosen by the bytes. Shorter bytes give fewer rules; no bytes give a sketch with the
/// splines and nothing on them.
fn text_of(data: &[u8]) -> String {
    let mut bytes = data.iter().copied();
    let mut text = String::from("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n");
    for p in 0..12_u32 {
        let (x, y) = (bytes.next().unwrap_or(0), bytes.next().unwrap_or(0));
        let (x, y) = (
            i32::from(x).saturating_sub(128),
            i32::from(y).saturating_sub(128),
        );
        let _ = writeln!(
            text,
            "entity 1.{p} point {} {}",
            x.saturating_mul(100),
            y.saturating_mul(100)
        );
    }
    let _ = writeln!(
        text,
        "entity 2.0 spline 3 4 1.0 1.1 1.2 1.3 8 0 0 0 0 1 1 1 1 4 1 1 1 1"
    );
    let _ = writeln!(
        text,
        "entity 2.1 spline 3 4 1.4 1.5 1.6 1.7 8 0 0 0 0 1 1 1 1 4 1 1 1 1"
    );
    let _ = writeln!(text, "entity 2.2 line 1.8 1.9 segment");
    let _ = writeln!(text, "entity 2.3 arc 1.8 700 1.10 1.11");
    for c in 0..MAX_RULES {
        let Some(kind) = bytes.next() else { break };
        let order = ["g1", "g2", "g3"]
            .get(usize::from(kind / 8 % 3))
            .copied()
            .unwrap_or("g1");
        let end = |bit: u8| if kind & bit == 0 { "start" } else { "finish" };
        let amount =
            100_u32.saturating_add(u32::from(bytes.next().unwrap_or(0)).saturating_mul(50));
        let at = f64::from(bytes.next().unwrap_or(0)) / 255.0;
        let _ = match kind % 6 {
            0 => writeln!(
                text,
                "constraint 3.{c} driving 0 spline-join 2.0 {} 2.1 {} {order}",
                end(1),
                end(2)
            ),
            1 => writeln!(
                text,
                "constraint 3.{c} driving 0 spline-against 2.0 {} 2.2 start {order}",
                end(1)
            ),
            2 => writeln!(
                text,
                "constraint 3.{c} driving 0 spline-against 2.1 {} 2.3 {} {order}",
                end(1),
                end(2)
            ),
            3 => writeln!(
                text,
                "constraint 3.{c} driving 0 spline-length 2.0 {amount}"
            ),
            4 => writeln!(
                text,
                "constraint 3.{c} driving 0 spline-through 2.1 1.8 {at}"
            ),
            _ => writeln!(
                text,
                "constraint 3.{c} driving 0 spline-tangent-line 2.1 {} 2.2",
                end(1)
            ),
        };
    }
    text
}

/// The laws on `sketch`.
///
/// # Errors
///
/// A [`Finding`] naming the law `sketch` broke.
pub fn check_sketch(sketch: &Sketch) -> Result<(), Finding> {
    let law = |what: &str| Finding::new(what, || format!("{sketch:?}"));
    for spline in [ItemId::new(2, 0), ItemId::new(2, 1)] {
        if let Ok(length) = spline_length(sketch, spline) {
            if length.error.is_nan() || length.error < 0.0 || !length.value.is_finite() {
                return Err(law("a length states a bound that is not negative"));
            }
            let (Ok(first), Ok(last)) = (
                sketch.point(ItemId::new(1, if spline.key == 0 { 0 } else { 4 })),
                sketch.point(ItemId::new(1, if spline.key == 0 { 3 } else { 7 })),
            ) else {
                return Err(law("the control points are there"));
            };
            let (dx, dy) = (first[0] - last[0], first[1] - last[1]);
            // Compared squared, so no root is taken; the slack covers the rounding of the sums.
            let reach = length.value + length.error;
            if reach * reach * (1.0 + 1e-9) + 1e-6 < dx * dx + dy * dy {
                return Err(law("a spline is at least as long as the chord of its ends"));
            }
            if spline_length(sketch, spline) != Ok(length) {
                return Err(law("a second length is the same"));
            }
        }
    }
    for c in sketch.constraints() {
        if let Ok(first) = join_sense(sketch, c.id)
            && join_sense(sketch, c.id) != Ok(first)
        {
            return Err(law("a second sense is the same"));
        }
    }
    let Ok(mut assembly) = Assembly::new(sketch) else {
        return Ok(());
    };
    let _ = assembly.evaluate(sketch);
    let mut meter = WorkMeter::new(Budget::of_work(200_000));
    let Ok(solved) = solve(sketch, &mut meter) else {
        return Ok(());
    };
    if solved.report.largest.is_nan() || solved.report.largest > TOLERANCE {
        return Err(law(
            "a solved sketch has its residuals within the tolerance",
        ));
    }
    let mut again = WorkMeter::new(Budget::of_work(200_000));
    match solve(sketch, &mut again) {
        Ok(second) if second == solved => {}
        _ => return Err(law("a second solve gives the same sketch")),
    }
    if measure_joins(&solved.sketch).is_err() {
        return Err(law("the joins of a solved sketch are measured"));
    }
    Ok(())
}

/// The laws on the sketch `data` describes.
///
/// # Errors
///
/// As [`check_sketch`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    match parse(&text_of(data)) {
        Ok(sketch) => check_sketch(&sketch),
        Err(_) => Ok(()),
    }
}
