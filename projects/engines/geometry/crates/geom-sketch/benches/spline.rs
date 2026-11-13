// SPDX-License-Identifier: Apache-2.0
//! The benchmark of piece G220 (chapter 30): the residual and Jacobian of each spline rule, the
//! board's "under 200 ns" of the constraint catalogue.
//!
//! Run with `cargo bench -p geom-sketch --bench spline`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18). Each rule is timed on a cubic cut in two, with the splines' own rows (there are none:
//! a spline's control points are points) taken off by timing the drawing without the rule.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_sketch::equation::Assembly;
use geom_sketch::text::parse;

const RUNS: u32 = 200_000;

/// A cubic Bézier cut at its middle, as two splines, and a line along the join.
const DRAWING: &str = "sketch v1
plane lattice 0 0 0 1 0 0 0 1 0 q0
entity 1.0 point 0 0
entity 1.1 point 4 8
entity 1.2 point 10 13
entity 1.3 point 16 13.5
entity 1.4 point 16 13.5
entity 1.5 point 22 14
entity 1.6 point 28 10
entity 1.7 point 32 0
entity 1.8 point 16 13.5
entity 1.9 point 26 14
entity 2.0 spline 3 4 1.0 1.1 1.2 1.3 8 0 0 0 0 1 1 1 1 4 1 1 1 1
entity 2.1 spline 3 4 1.4 1.5 1.6 1.7 8 0 0 0 0 1 1 1 1 4 1 1 1 1
entity 2.2 line 1.8 1.9 segment
";

fn nanoseconds(a: &mut Assembly, x: &[f64]) -> f64 {
    let start = Instant::now();
    for _ in 0..RUNS {
        a.evaluate_at(black_box(x)).unwrap();
    }
    start.elapsed().as_secs_f64() * 1e9 / f64::from(RUNS)
}

fn main() {
    let mut plain = Assembly::new(&parse(DRAWING).unwrap()).unwrap();
    let x = plain.start().unwrap();
    let base = nanoseconds(&mut plain, &x).min(nanoseconds(&mut plain, &x));
    for rule in [
        "spline-join 2.0 finish 2.1 start g1",
        "spline-join 2.0 finish 2.1 start g2",
        "spline-join 2.0 finish 2.1 start g3",
        "spline-tangent-line 2.1 start 2.2",
    ] {
        let text = format!("{DRAWING}constraint 3.0 driving 0 {rule}\n");
        let mut a = Assembly::new(&parse(&text).unwrap()).unwrap();
        let x = a.start().unwrap();
        let own = nanoseconds(&mut a, &x).min(nanoseconds(&mut a, &x)) - base;
        println!("{rule:<52} {own:>8.1} ns");
    }
}
