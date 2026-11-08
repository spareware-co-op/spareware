//! The benchmark of piece G224 (chapters 15.7 and 15.8): the dependency groups of a conflict among
//! 256 unknowns, and the free motions of a sketch.
//!
//! Run with `cargo bench -p geom-sketch --bench diagnose`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18). Section 15.7's targets are under 20 ms for the groups of a conflict among up to 256
//! unknowns, and under 5 ms for the free motions of one entity in a 10,000-unknown outline. The
//! second is measured with the sparse per-entity form (`geom_sketch::motions`): the one-time
//! factorisation of the 10,000-unknown outline is timed apart from the query for one entity, which is
//! what the target bounds. The dense form is timed at 200 unknowns for comparison.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::fmt::Write;
use std::hint::black_box;
use std::time::Instant;

use geom_run::{Budget, WorkMeter};
use geom_sketch::diagnose::{diagnose, free_motions};
use geom_sketch::model::{ItemId, Sketch};
use geom_sketch::motions::MotionFinder;
use geom_sketch::text::parse;

const HEAD: &str = "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n";

/// A chain of `points` points, each 100 right and 50 up of the last, held at the first and
/// dimensioned along both axes on every link: exactly determined. With `conflict`, the last point
/// is also asked to be 100 from the first, which the chain contradicts.
fn chain(points: usize, conflict: bool, upward: bool) -> Sketch {
    let mut text = HEAD.to_owned();
    for i in 0..points {
        writeln!(
            text,
            "entity 1.{i} point {} {}",
            i as f64 * 100.0,
            i as f64 * 50.0
        )
        .unwrap();
    }
    writeln!(text, "constraint 2.0 driving 0 fix 1.0 0 0").unwrap();
    writeln!(text, "constraint 2.1 driving 0 fix 1.0 1 0").unwrap();
    for i in 1..points {
        writeln!(
            text,
            "constraint 3.{i} driving 0 across 1.{} 1.{i} 100",
            i - 1
        )
        .unwrap();
        if upward {
            writeln!(
                text,
                "constraint 4.{i} driving 0 upward 1.{} 1.{i} 50",
                i - 1
            )
            .unwrap();
        }
    }
    if conflict {
        writeln!(
            text,
            "constraint 5.0 driving 1 distance 1.0 1.{} 100",
            points - 1
        )
        .unwrap();
    }
    parse(&text).unwrap()
}

fn time(name: &str, runs: u32, mut f: impl FnMut()) {
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let ms = start.elapsed().as_secs_f64() * 1e3 / f64::from(runs);
    println!("{name:<56} {ms:>9.3} ms");
}

fn main() {
    let conflict = chain(128, true, true);
    time("groups of a conflict among 256 unknowns", 20, || {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let r = diagnose(black_box(&conflict), 10_000.0, &mut meter).unwrap();
        assert_eq!(r.conflicting.len(), 1);
    });
    let free = chain(500, false, true);
    time("diagnosis of a 1,000-unknown chain", 3, || {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let r = diagnose(black_box(&free), 10_000.0, &mut meter).unwrap();
        assert_eq!(r.dof_least, 0);
    });
    let under = chain(100, false, false);
    time(
        "free motions of a 200-unknown chain, y free (100 motions)",
        20,
        || {
            let mut meter = WorkMeter::new(Budget::UNLIMITED);
            black_box(free_motions(black_box(&under), &mut meter).unwrap());
        },
    );
    // The sparse form on a 10,000-unknown outline: 5,000 points held along x, every y free.
    let outline = chain(5000, false, false);
    time(
        "sparse factorisation of a 10,000-unknown outline",
        3,
        || {
            let mut meter = WorkMeter::new(Budget::UNLIMITED);
            black_box(MotionFinder::new(black_box(&outline), &mut meter).unwrap());
        },
    );
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let finder = MotionFinder::new(&outline, &mut meter).unwrap();
    assert_eq!(finder.dof(), 4999);
    time(
        "free motions of one entity, 10,000 unknowns (target < 5 ms)",
        200,
        || {
            let mut meter = WorkMeter::new(Budget::UNLIMITED);
            let m = finder
                .of_entity(black_box(ItemId::new(1, 2500)), &mut meter)
                .unwrap();
            assert_eq!(m.len(), 1);
        },
    );
}
