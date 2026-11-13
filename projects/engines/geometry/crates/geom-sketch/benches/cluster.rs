//! The benchmark of piece G223 (chapter 30): the rigid clusters of a 1,000-entity family, the
//! board's "in 10 ms".
//!
//! Run with `cargo bench -p geom-sketch --bench cluster`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18). The family is 250 rectangles of four points and five bars, each a rigid body, as the
//! G222 family sketch is; a second line joins 25 of them in a floppy chain of single bars, the
//! shape that makes the search look further than one body.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_run::{Budget, WorkMeter};
use geom_sketch::cluster::{Link, Node, Problem, SIZE_LIMIT, plan};

fn family(bodies: usize, chain: bool) -> Problem {
    let nodes = (0..4 * bodies)
        .map(|i| Node {
            key: 100 + i as u64,
            weight: 2,
            fixed: false,
        })
        .collect();
    let mut links = Vec::new();
    for b in 0..bodies {
        let o = 4 * b;
        for (i, (x, y)) in [(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)]
            .into_iter()
            .enumerate()
        {
            links.push(Link {
                key: 10 * b as u64 + i as u64,
                members: vec![o + x, o + y],
                weight: 1,
            });
        }
        if chain && b > 0 {
            links.push(Link {
                key: 1_000_000 + b as u64,
                members: vec![o - 1, o],
                weight: 1,
            });
        }
    }
    Problem {
        motions: 3,
        nodes,
        links,
    }
}

fn time(name: &str, runs: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let ms = start.elapsed().as_secs_f64() * 1e3 / f64::from(runs);
    println!("{name:<52} {ms:>8.2} ms");
}

fn main() {
    let apart = family(250, false);
    time("clusters of a 1,000-entity family", 20, || {
        let p = plan(
            black_box(&apart),
            SIZE_LIMIT,
            &mut WorkMeter::new(Budget::UNLIMITED),
        )
        .unwrap();
        assert_eq!(p.top.len(), 250);
        black_box(p);
    });
    let chained = family(25, true);
    time("clusters of 100 entities in a floppy chain", 20, || {
        let p = plan(
            black_box(&chained),
            SIZE_LIMIT,
            &mut WorkMeter::new(Budget::UNLIMITED),
        )
        .unwrap();
        assert_eq!(p.top.len(), 25);
        black_box(p);
    });
}
