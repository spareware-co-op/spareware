//! The fuzz target of a sketch's free motions (piece G224): any bytes are read as a small sketch,
//! and the motions the sparse form finds must be motions.
//!
//! What it does: [`harness`] reads the fuzzer's bytes as a few points and a few dimensions on them,
//! writes the `sketch v1` text, and hands the sketch to `geom_sketch::motions::MotionFinder`.
//!
//! Guarantees:
//!
//! - **Never a panic.** A finder is made or refused with a typed error on every sketch.
//! - **A motion is a motion.** Each motion moves the Jacobian's equations by no more than the rank
//!   tolerance lets through: `|J·x|` is at most σ_ind times the largest column, with the leading
//!   variable at 1, plus rounding (15.8).
//! - **One motion for each degree of freedom,** in leading-variable order, and a motion of one
//!   entity is one of the motions of all.
//! - **Deterministic.** A second finder gives equal motions.
//!
//! Method: the property-based fuzzing of 28.5
//! (`projects/engines/geometry/architecture/28-testing.md`), on section 15.8's free motions
//! (`projects/engines/geometry/architecture/15-sketches.md`).
//!
//! Cost: a few small sparse factorisations per input.

use core::fmt::Write as _;

use geom_run::{Budget, WorkMeter};
use geom_sketch::equation::Assembly;
use geom_sketch::model::Sketch;
use geom_sketch::motions::MotionFinder;
use geom_sketch::solve::INDEPENDENT;
use geom_sketch::text::parse;

use crate::Finding;

/// The most points a case has.
const MAX_POINTS: usize = 6;

/// The most dimensions a case has.
const MAX_CONSTRAINTS: usize = 10;

/// The sketch text the bytes describe: points on a 100-step grid and dimensions that hold some of
/// their coordinates. Shorter bytes give fewer items; no bytes give an empty sketch.
fn text_of(data: &[u8]) -> String {
    let mut bytes = data.iter().copied();
    let mut text = String::from("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n");
    let points = bytes.next().map_or(0, |b| usize::from(b) % MAX_POINTS) + 1;
    for p in 0..points {
        let (x, y) = (bytes.next().unwrap_or(0), bytes.next().unwrap_or(0));
        let _ = writeln!(
            text,
            "entity 1.{p} point {} {}",
            u32::from(x) * 100,
            u32::from(y) * 100
        );
    }
    for c in 0..MAX_CONSTRAINTS {
        let Some(kind) = bytes.next() else { break };
        let a = usize::from(bytes.next().unwrap_or(0)) % points;
        let b = usize::from(bytes.next().unwrap_or(1)) % points;
        let d = 100 + u32::from(bytes.next().unwrap_or(0)) * 4;
        let _ = match kind % 4 {
            0 => writeln!(
                text,
                "constraint 2.{c} driving 0 fix 1.{a} {} {d}",
                kind / 4 % 2
            ),
            1 if a != b => writeln!(text, "constraint 2.{c} driving 0 across 1.{a} 1.{b} {d}"),
            2 if a != b => writeln!(text, "constraint 2.{c} driving 0 upward 1.{a} 1.{b} {d}"),
            _ if a != b => writeln!(text, "constraint 2.{c} driving 0 distance 1.{a} 1.{b} {d}"),
            _ => Ok(()),
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
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let Ok(finder) = MotionFinder::new(sketch, &mut meter) else {
        return Ok(());
    };
    let law = |what: &str| Finding::new(what, || format!("{sketch:?}"));
    let Ok(all) = finder.all(&mut meter) else {
        return Err(law("a finder that was made gives its motions"));
    };
    if all.len() != finder.dof() {
        return Err(law("one motion for each degree of freedom"));
    }
    let Ok(again) = MotionFinder::new(sketch, &mut meter).and_then(|f| f.all(&mut meter)) else {
        return Err(law("a second finder is made"));
    };
    if again != all {
        return Err(law("the motions are the same every time"));
    }
    let mut assembly = Assembly::new(sketch).map_err(|_| law("an assembly is made"))?;
    assembly
        .evaluate(sketch)
        .map_err(|_| law("an assembly evaluates"))?;
    let variables = assembly.variables().to_vec();
    let jacobian = assembly.jacobian();
    let mut largest = 0.0_f64;
    for j in 0..jacobian.cols() {
        let (_, values) = jacobian.column(j).map_err(|_| law("a column is read"))?;
        // The 1-norm, which is at least the 2-norm: a looser bound is a sound one.
        let norm = values.iter().fold(0.0_f64, |a, b| a + b.abs());
        if norm > largest {
            largest = norm;
        }
    }
    for motion in &all {
        let mut x = vec![0.0; variables.len()];
        for &(owner, slot, moved) in &motion.moves {
            let Some(at) = variables
                .iter()
                .position(|v| v.owner == owner && v.slot == slot)
            else {
                return Err(law("a motion moves only the sketch's variables"));
            };
            let (Some(held), Some(var)) = (x.get_mut(at), variables.get(at)) else {
                return Err(law("a motion moves only the sketch's variables"));
            };
            *held = moved / var.scale;
        }
        let Ok(product) = jacobian.multiply(&x) else {
            return Err(law("the Jacobian multiplies a motion"));
        };
        let size = x
            .iter()
            .fold(0.0_f64, |a, b| if b.abs() > a { b.abs() } else { a });
        let bound = 1.01 * INDEPENDENT * largest + 1e-9 * (1.0 + largest * size);
        let worst = product
            .iter()
            .fold(0.0_f64, |a, b| if b.abs() > a { b.abs() } else { a });
        if worst > bound {
            return Err(Finding::new(
                "a motion changes no equation past the rank tolerance",
                || format!("{worst} > {bound} on {sketch:?}"),
            ));
        }
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
