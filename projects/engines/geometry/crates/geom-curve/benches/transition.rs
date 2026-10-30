//! The benchmark of piece G63 (chapter 30): a million station evaluations per transition law, which
//! the board asks to take at most one second, and the certified enclosure and construction beside
//! them.
//!
//! Run with `cargo bench -p geom-curve --bench transition`. It uses the standard library's clock
//! only (wall-clock time on this machine; the reference machine and instruction counts are
//! geom-bench's, piece G18). Each law's stations are a million evenly spaced points along a
//! transition of road or rail proportions, evaluated in turn by [`Transition::point`], the fast
//! path; the line printed for each says whether the million took at most a second.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_curve::{CurvatureLaw, Transition};
use geom_run::{Budget, WorkMeter};

/// Stations per law.
const STATIONS: u32 = 1_000_000;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn main() {
    let r = 1.0 / 650.0;
    let mut seventh = [None; 8];
    seventh[0] = Some(-9000.0);
    seventh[1] = Some(1400.0);
    seventh[3] = Some(-750.0);
    seventh[7] = Some(520.0);
    let mut clothoid = [None; 8];
    clothoid[1] = Some(250.0);
    let laws = [
        ("clothoid", CurvatureLaw::Clothoid { start: 0.0, end: r }),
        ("Helmert", CurvatureLaw::Helmert { start: 0.0, end: r }),
        ("Bloss", CurvatureLaw::Bloss { start: 0.0, end: r }),
        ("cosine", CurvatureLaw::Cosine { start: 0.0, end: r }),
        ("sine", CurvatureLaw::Sine { start: 0.0, end: r }),
        (
            "cant-coupled",
            CurvatureLaw::CantCoupled {
                start: 0.0,
                end: r,
                height: 1.8,
                cant: 0.11,
            },
        ),
        (
            "IfcClothoid from a station",
            CurvatureLaw::Polynomial {
                terms: clothoid,
                from: 40.0,
            },
        ),
        (
            "seventh-order polynomial spiral",
            CurvatureLaw::Polynomial {
                terms: seventh,
                from: 10.0,
            },
        ),
        (
            "IfcCosineSpiral",
            CurvatureLaw::CosineSpiral {
                constant: Some(1500.0),
                cosine: 900.0,
                length: 120.0,
                from: 15.0,
            },
        ),
        (
            "IfcSineSpiral",
            CurvatureLaw::SineSpiral {
                constant: Some(-2000.0),
                linear: Some(350.0),
                sine: 700.0,
                length: 150.0,
                from: 20.0,
            },
        ),
    ];
    let length = 120.0;
    let step = length / f64::from(STATIONS);
    let mut worst = 0.0f64;
    for (name, law) in laws {
        let start = Instant::now();
        let t = Transition::new([1000.0, 2000.0], [3.0, 4.0], length, law, &mut meter()).unwrap();
        let built = start.elapsed().as_secs_f64() * 1e6;
        black_box(t.point(0.0).unwrap());
        let start = Instant::now();
        for i in 0..STATIONS {
            black_box(t.point(black_box(f64::from(i) * step)).unwrap());
        }
        let seconds = start.elapsed().as_secs_f64();
        worst = if seconds > worst { seconds } else { worst };
        let start = Instant::now();
        let runs = 100_000;
        for i in 0..runs {
            black_box(
                t.enclose(
                    black_box(f64::from(i) * length / f64::from(runs)),
                    2,
                    &mut meter(),
                )
                .unwrap(),
            );
        }
        let enclose = start.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
        println!(
            "{name:<34} {} panel(s), built in {built:>8.1} µs; 1M points in {seconds:.3} s ({}); \
             enclose with two derivatives {enclose:.3} µs",
            t.panels(),
            if seconds <= 1.0 {
                "within 1 s"
            } else {
                "OVER 1 s"
            },
        );
    }
    println!("slowest law: {worst:.3} s for a million stations");
}
