//! How long the triangulation takes, by the standard library's clock (piece G153): a million
//! random points with 10,000 breaklines, the done-when's 2 seconds, and a cocircular grid, the
//! hard case of 17.3. Prints one line per run; the gate is the count benchmark in `geom-bench`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::print_stdout,
    reason = "a benchmark program; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::time::Instant;

use geom_mesh::cdt::{Site, triangulate};
use geom_num::grid::P2;
use geom_run::{Budget, WorkMeter};
use geom_test::{Seed, SplitMix64};

fn main() {
    let mut rng = SplitMix64::new(Seed(0x153));
    let n = 1_000_000_usize;
    let reach = NonZeroU64::new(1 << 32).unwrap();
    let mut seen = BTreeSet::new();
    while seen.len() < n {
        seen.insert((rng.below(reach) as i64, rng.below(reach) as i64));
    }
    let mut sites: Vec<Site<u32>> = seen
        .into_iter()
        .zip(0_u32..)
        .map(|((x, y), key)| Site {
            at: P2::new(x, y).unwrap(),
            key,
        })
        .collect();
    for i in (1..sites.len()).rev() {
        let j = rng.below(NonZeroU64::new(i as u64 + 1).unwrap()) as usize;
        sites.swap(i, j);
    }
    // Breaklines as a survey has them: each joins a site to the third next up its column of a
    // grid of 1,024 columns, a few site spacings long, and the sites are far apart in key.
    let mut sorted: Vec<u32> = (0..n as u32).collect();
    sorted.sort_by_key(|&i| {
        let at = sites[i as usize].at;
        (at.x.get() >> 22, at.y.get())
    });
    let short: Vec<[u32; 2]> = (0..10_000)
        .map(|i| {
            let at = (i * 97) as usize;
            [sorted[at], sorted[at + 3]]
        })
        .collect();
    // Breaklines across the model: sorted by x alone, each runs far in y through hundreds of
    // triangles, the heavy case of the constraint step.
    sorted.sort_by_key(|&i| sites[i as usize].at);
    let long: Vec<[u32; 2]> = (0..10_000)
        .map(|i| {
            let at = (i * 97) as usize;
            [sorted[at], sorted[at + 30]]
        })
        .collect();
    for (name, constraints) in [("short", &short), ("long", &long)] {
        let start = Instant::now();
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let c = triangulate(&sites, constraints, None, &mut meter).unwrap();
        println!(
            "scatter: {} points, {} {name} breaklines: {} triangles in {:?}",
            n,
            constraints.len(),
            c.triangles.len(),
            start.elapsed()
        );
    }
    let side = 1000_i64;
    let grid: Vec<Site<u32>> = (0..side * side)
        .zip(0_u32..)
        .map(|(i, key)| Site {
            at: P2::new((i % side) * 64, (i / side) * 64).unwrap(),
            key,
        })
        .collect();
    let start = Instant::now();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let c = triangulate(&grid, &[], None, &mut meter).unwrap();
    println!(
        "grid: {} points: {} triangles in {:?}",
        grid.len(),
        c.triangles.len(),
        start.elapsed()
    );
}
