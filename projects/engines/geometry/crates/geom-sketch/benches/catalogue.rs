//! The benchmark of piece G218 (chapter 30): each kind's residual and Jacobian, the board's "under
//! 200 ns".
//!
//! Run with `cargo bench -p geom-sketch --bench catalogue`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18). Each rule of the test scene is timed with the scene's entities, less the time of the
//! entities' own rows, so that what remains is one rule.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::format_collect,
    clippy::similar_names,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_sketch::equation::Assembly;
use geom_sketch::text::parse;

const SCENE: &str = include_str!("../tests/data/catalogue.sketch");
const RUNS: u32 = 200_000;

fn nanoseconds(a: &mut Assembly, x: &[f64]) -> f64 {
    let start = Instant::now();
    for _ in 0..RUNS {
        a.evaluate_at(black_box(x)).unwrap();
    }
    start.elapsed().as_secs_f64() * 1e9 / f64::from(RUNS)
}

fn main() {
    let bare: String = SCENE
        .lines()
        .filter(|line| !line.starts_with("constraint"))
        .map(|line| format!("{line}\n"))
        .collect();
    let mut plain = Assembly::new(&parse(&bare).unwrap()).unwrap();
    let x = plain.start().unwrap();
    let mut base = nanoseconds(&mut plain, &x);
    base = base.min(nanoseconds(&mut plain, &x));
    let mut slowest = (0.0, String::new());
    for line in SCENE.lines().filter(|line| line.starts_with("constraint")) {
        let mut a = Assembly::new(&parse(&format!("{bare}{line}\n")).unwrap()).unwrap();
        let x = a.start().unwrap();
        let own = nanoseconds(&mut a, &x).min(nanoseconds(&mut a, &x)) - base;
        let rule = line.splitn(5, ' ').nth(4).unwrap();
        println!("{rule:<52} {own:>8.1} ns");
        if own > slowest.0 {
            slowest = (own, rule.to_owned());
        }
    }
    println!("slowest: {} at {:.1} ns", slowest.1, slowest.0);
}
