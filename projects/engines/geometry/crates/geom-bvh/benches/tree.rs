//! The benchmarks of piece G54 (chapter 30): building a tree of 1,000,000 boxes on one thread (the
//! board's "in 0.5 s"), and the pairs among 100,000 faces' boxes found by the tree against testing
//! every pair (the board's "beats all pairs").
//!
//! Run with `cargo bench -p geom-bvh`. It uses the standard library's clock only (wall-clock time
//! on this machine; the reference machine and instruction counts are geom-bench's, piece G18). The
//! boxes come from a fixed linear congruential sequence, so every run measures the same work: the
//! million are spread over a cube of side 2^30 grid steps with sides up to 2^20, and the 100,000
//! faces are small boxes along a 1,000 × 100 sheet, so each meets a handful of neighbours, as the
//! faces of a mesh do.

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

use geom_bvh::{Aabb, BoxTree, Entry};
use geom_run::{Budget, Exec, WorkMeter};

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

fn spread(n: usize) -> Vec<Entry<3>> {
    let mut rng = Lcg(1);
    (0..n)
        .map(|i| {
            let lo: [i64; 3] = std::array::from_fn(|_| (rng.next() % (1 << 30)) as i64);
            let hi: [i64; 3] = std::array::from_fn(|a| lo[a] + (rng.next() % (1 << 20)) as i64);
            Entry {
                key: i as u64,
                bbox: Aabb::new(lo, hi).unwrap(),
            }
        })
        .collect()
}

/// 100,000 face boxes on a 1,000 × 100 sheet of 1,024-step cells, each a little larger than its
/// cell and nudged, so it meets its neighbours.
fn faces() -> Vec<Entry<3>> {
    let mut rng = Lcg(2);
    (0..100_000_i64)
        .map(|i| {
            let (x, y) = ((i % 1000) * 1024, (i / 1000) * 1024);
            let z = (rng.next() % 256) as i64;
            let lo = [
                x - (rng.next() % 64) as i64,
                y - (rng.next() % 64) as i64,
                z,
            ];
            let hi = [x + 1024 + (rng.next() % 64) as i64, y + 1024, z + 64];
            Entry {
                key: i as u64,
                bbox: Aabb::new(lo, hi).unwrap(),
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
    let each = start.elapsed().as_secs_f64() / f64::from(runs);
    println!("{name}: {:.3} ms", each * 1e3);
    each
}

fn main() {
    let meter = || WorkMeter::new(Budget::UNLIMITED);
    let million = spread(1_000_000);
    let build = time("build, 1,000,000 boxes, one thread", 3, || {
        BoxTree::build(&million, &Exec::serial(), &mut meter()).unwrap()
    });
    println!(
        "  target 500 ms: {}",
        if build <= 0.5 { "met" } else { "MISSED" }
    );

    let faces = faces();
    let tree = BoxTree::build(&faces, &Exec::serial(), &mut meter()).unwrap();
    let mut found = 0;
    let tree_time = time("pairs among 100,000 faces, by the tree (built)", 3, || {
        let pairs = tree.pairs(&mut meter()).unwrap();
        found = pairs.len();
        pairs
    });
    let with_build = time("pairs among 100,000 faces, build and query", 3, || {
        let t = BoxTree::build(&faces, &Exec::serial(), &mut meter()).unwrap();
        t.pairs(&mut meter()).unwrap()
    });
    let mut brute = 0usize;
    let all_pairs = time("pairs among 100,000 faces, every pair tested", 1, || {
        brute = 0;
        for i in 0..faces.len() {
            for j in i + 1..faces.len() {
                if faces[i].bbox.overlaps(&faces[j].bbox) {
                    brute += 1;
                }
            }
        }
        brute
    });
    assert_eq!(found, brute, "the tree and every pair disagree");
    println!(
        "  {found} pairs; the tree (with its build) is {:.0}x faster than all pairs ({:.0}x without)",
        all_pairs / with_build,
        all_pairs / tree_time
    );
}
