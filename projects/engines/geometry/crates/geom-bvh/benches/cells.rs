//! The benchmarks of piece G55 (chapter 30): building the cell index over 10,000,000 points on 8
//! threads (the board's "10M points in under 1 s on 8 threads") and on one, and a welding-like
//! search, every pair within a few steps among 1,000,000 points.
//!
//! Run with `cargo bench -p geom-bvh --bench cells`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18), and a pool of scoped threads like the tests' (what geom-pool does for the facade).
//! The points come from a fixed linear congruential sequence, so every run measures the same work:
//! the ten million are spread over a cube of side 2^30 grid steps, and the million lie on a
//! 100 × 100 × 100 lattice of spacing 16, nudged by up to 3 steps, so each has a few neighbours
//! within 6 steps, as the vertices of a mesh to be welded do.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_bvh::{CellIndex, Point};
use geom_num::grid::P3;
use geom_run::{Budget, Exec, Pool, WorkMeter};

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 24
    }
}

/// `threads` scoped threads, task i on thread i mod threads.
struct Threads(usize);

impl Pool for Threads {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        let mut groups: Vec<Vec<&mut (dyn FnMut() + Send)>> =
            (0..self.0).map(|_| Vec::new()).collect();
        for (i, t) in tasks.iter_mut().enumerate() {
            groups[i % self.0].push(&mut **t);
        }
        std::thread::scope(|s| {
            for group in groups {
                s.spawn(move || {
                    for t in group {
                        t();
                    }
                });
            }
        });
    }
}

fn spread(n: usize) -> Vec<Point> {
    let mut rng = Lcg(55);
    (0..n)
        .map(|i| {
            let c: [i64; 3] = std::array::from_fn(|_| (rng.next() % (1 << 30)) as i64);
            Point {
                key: i as u64,
                at: P3::new(c[0], c[1], c[2]).unwrap(),
            }
        })
        .collect()
}

fn lattice() -> Vec<Point> {
    let mut rng = Lcg(56);
    (0..1_000_000_i64)
        .map(|i| {
            let (x, y, z) = (i % 100, (i / 100) % 100, i / 10_000);
            let nudge = |rng: &mut Lcg| (rng.next() % 7) as i64 - 3;
            Point {
                key: i as u64,
                at: P3::new(
                    x * 16 + nudge(&mut rng),
                    y * 16 + nudge(&mut rng),
                    z * 16 + nudge(&mut rng),
                )
                .unwrap(),
            }
        })
        .collect()
}

fn time<T>(name: &str, runs: u32, mut f: impl FnMut() -> T) -> f64 {
    black_box(f());
    let start = Instant::now();
    for _ in 0..runs {
        black_box(f());
    }
    let s = start.elapsed().as_secs_f64() / f64::from(runs);
    println!("{name:<56} {:>10.1} ms", s * 1e3);
    s
}

fn main() {
    let ten_million = spread(10_000_000);
    let eight = Threads(8);
    let pooled = Exec::with_pool(&eight);
    time("build, 10,000,000 points, 8 threads", 3, || {
        CellIndex::build(
            &ten_million,
            &pooled,
            &mut WorkMeter::new(Budget::UNLIMITED),
        )
        .unwrap()
    });
    time("build, 10,000,000 points, one thread", 1, || {
        CellIndex::build(
            &ten_million,
            &Exec::serial(),
            &mut WorkMeter::new(Budget::UNLIMITED),
        )
        .unwrap()
    });
    drop(ten_million);

    let mesh = lattice();
    let idx = CellIndex::build(&mesh, &pooled, &mut WorkMeter::new(Budget::UNLIMITED)).unwrap();
    let found = idx
        .pairs_within(6, &pooled, &mut WorkMeter::new(Budget::UNLIMITED))
        .unwrap()
        .len();
    time(
        &format!("pairs within 6 steps, 1,000,000 points ({found} pairs), 8 threads"),
        3,
        || {
            idx.pairs_within(6, &pooled, &mut WorkMeter::new(Budget::UNLIMITED))
                .unwrap()
        },
    );
    time(
        "points within 40 steps of 100,000 centres, one thread",
        3,
        || {
            let mut n = 0;
            for p in mesh.iter().step_by(10) {
                n += idx
                    .within(p.at, 40, &mut WorkMeter::new(Budget::UNLIMITED))
                    .unwrap()
                    .len();
            }
            n
        },
    );
}
