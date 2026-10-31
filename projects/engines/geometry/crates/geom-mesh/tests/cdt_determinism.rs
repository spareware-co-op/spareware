//! The replay hash of piece G153: triangulations of cocircular grids, of scattered points with
//! crossing constraints, and of refined outlines, their canonical bytes hashed with SHA-256, must
//! give the same digest on every target (chapters 17.3, 21 and 28.13; the golden store of piece
//! G19).
//!
//! The corpus, from a fixed `SplitMix64` stream: square grids of 2 to 32 points a side, where
//! every circle test is a tie, with keys in shuffled order, plain and with constraints that cross;
//! 16 scatters with constraints between random points; and 8 outlines refined to a quality. Every
//! vertex is written as its two steps and its key, every triangle and constrained edge as its
//! vertices, so a target that breaks a tie, commits a crossing or orders a triangle differently
//! changes the digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;
use std::num::NonZeroU64;

use geom_mesh::cdt::{Cdt, Quality, Site, triangulate};
use geom_num::grid::P2;
use geom_run::{Budget, WorkMeter};
use geom_test::golden::{self, hex, sha256};
use geom_test::{Seed, SplitMix64};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-mesh/cdt";

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

/// The keys 0 to `n` − 1 in a shuffled order.
fn keys(rng: &mut SplitMix64, n: usize) -> Vec<u32> {
    let mut k: Vec<u32> = (0..n as u32).collect();
    for i in (1..n).rev() {
        k.swap(i, below(rng, i as u64 + 1) as usize);
    }
    k
}

fn sites(rng: &mut SplitMix64, points: &[(i64, i64)]) -> Vec<Site<u32>> {
    let k = keys(rng, points.len());
    points
        .iter()
        .zip(k)
        .map(|(&(x, y), key)| Site {
            at: P2::new(x, y).unwrap(),
            key,
        })
        .collect()
}

fn write(out: &mut Vec<u8>, c: &Cdt<u32>) {
    out.extend((c.vertices.len() as u64).to_le_bytes());
    for v in &c.vertices {
        out.extend(v.at.x.get().to_le_bytes());
        out.extend(v.at.y.get().to_le_bytes());
        out.extend(v.key.unwrap_or(u32::MAX).to_le_bytes());
    }
    out.extend((c.triangles.len() as u64).to_le_bytes());
    for t in c.triangles.iter().flatten() {
        out.extend(t.to_le_bytes());
    }
    out.extend((c.constraints.len() as u64).to_le_bytes());
    for e in c.constraints.iter().flatten() {
        out.extend(e.to_le_bytes());
    }
}

/// The corpus's canonical bytes and how many triangulations went in.
fn corpus() -> (Vec<u8>, u32) {
    let mut rng = SplitMix64::new(Seed(0x0153));
    let mut out = Vec::new();
    let mut runs = 0;
    let mut run = |out: &mut Vec<u8>, s: &[Site<u32>], c: &[[u32; 2]], q: Option<&Quality>| {
        // Far above what the corpus needs: a triangulation that stops advancing fails, not hangs.
        let mut meter = WorkMeter::new(Budget::of_work(1 << 20));
        write(out, &triangulate(s, c, q, &mut meter).unwrap());
        runs += 1;
    };
    for side in [2_i64, 3, 5, 8, 13, 21, 32] {
        let points: Vec<(i64, i64)> = (0..side * side)
            .map(|i| ((i % side) * 7, (i / side) * 7))
            .collect();
        let s = sites(&mut rng, &points);
        run(&mut out, &s, &[], None);
        let n = points.len() as u32;
        let across = [[0, n - 1], [side as u32 - 1, n - side as u32]];
        run(&mut out, &s, &across, None);
    }
    for _ in 0..16 {
        let mut seen = BTreeSet::new();
        let count = 8 + below(&mut rng, 200) as usize;
        while seen.len() < count {
            seen.insert((
                below(&mut rng, 1 << 20) as i64 - (1 << 19),
                below(&mut rng, 1 << 20) as i64 - (1 << 19),
            ));
        }
        let points: Vec<(i64, i64)> = seen.into_iter().collect();
        let s = sites(&mut rng, &points);
        let mut constraints = Vec::new();
        for _ in 0..=below(&mut rng, 12) {
            let (a, b) = (
                below(&mut rng, count as u64) as u32,
                below(&mut rng, count as u64) as u32,
            );
            if a != b {
                constraints.push([a, b]);
            }
        }
        run(&mut out, &s, &constraints, None);
    }
    let quality = Quality {
        ratio_squared: 2.0,
        max_edge_squared: None,
        min_edge_squared: 16,
    };
    for _ in 0..8 {
        let reach = 2000 + below(&mut rng, 6000) as i64;
        let inner = 200 + below(&mut rng, 800) as i64;
        let points = [
            (0, 0),
            (reach, 0),
            (reach, inner),
            (inner, inner),
            (inner, reach),
            (0, reach),
        ];
        let outline: Vec<[u32; 2]> = (0..6).map(|i| [i, (i + 1) % 6]).collect();
        let s = sites(&mut rng, &points);
        run(&mut out, &s, &outline, Some(&quality));
    }
    (out, runs)
}

#[test]
fn cdt_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.0), rec)
    });
    let (bytes, runs) = &checked.value;
    println!(
        "geom-mesh cdt corpus: {runs} triangulations, {} bytes, golden hash {}",
        bytes.len(),
        hex(sha256(bytes))
    );
    assert_eq!(*runs, 38, "every triangulation ran");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target breaks a tie, commits a crossing or \
             orders a triangle differently (compare this log's digest with another target's), or \
             a definition changed in place, which chapter 26.2 forbids"
        )
    });
}
