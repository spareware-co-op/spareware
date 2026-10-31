//! The replay hash of piece G146: exact meshes, their canonical arrays hashed with SHA-256, must
//! give the same digest on every target (chapters 19.2, 21 and 28.13; the golden store of piece
//! G19).
//!
//! The corpus, from a fixed `SplitMix64` stream: boxes of every scale to the edge of the frame,
//! closed height fields, and a cut pyramid with corners off the grid. Every array of a mesh is
//! written out, so a target that orders a plane, a vertex, a face or a pair differently changes
//! the digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;

use geom_mesh::exact::build::{Polygons, build};
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::{ExactMesh, Facing, Vertex};
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::golden::{self, hex, sha256};
use geom_test::{Seed, SplitMix64};

mod shapes;

use shapes::{cuboid, frustum, prism};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-mesh/exact";

fn words(out: &mut Vec<u8>, items: &[u32]) {
    out.extend((items.len() as u64).to_le_bytes());
    for i in items {
        out.extend(i.to_le_bytes());
    }
}

fn facings(out: &mut Vec<u8>, items: &[Facing]) {
    out.extend((items.len() as u64).to_le_bytes());
    for f in items {
        out.extend(f.row.to_le_bytes());
        out.push(u8::from(f.flipped));
    }
}

fn write(out: &mut Vec<u8>, m: &ExactMesh) {
    let p = m.parts();
    out.extend((p.planes.len() as u64).to_le_bytes());
    for plane in &p.planes {
        for c in plane.n() {
            out.extend(c.to_le_bytes());
        }
        out.extend(plane.d().to_le_bytes());
    }
    out.extend((p.verts.len() as u64).to_le_bytes());
    for v in &p.verts {
        match v {
            Vertex::Grid(c) => {
                out.push(0);
                for x in [c.x.get(), c.y.get(), c.z.get()] {
                    out.extend(x.to_le_bytes());
                }
            }
            Vertex::Planes(rows) => {
                out.push(1);
                for r in rows {
                    out.extend(r.to_le_bytes());
                }
            }
        }
    }
    facings(out, &p.face_plane);
    words(out, &p.face_loops);
    words(out, &p.loop_start);
    words(out, &p.he_vert);
    facings(out, &p.he_plane);
    words(out, &p.he_pair);
    match m
        .volume(&mut WorkMeter::new(Budget::of_work(1 << 20)))
        .unwrap()
    {
        Volume::Exact(v) => {
            for limb in v.to_le_limbs() {
                out.extend(limb.to_le_bytes());
            }
        }
        Volume::Certified(v) => {
            out.extend(v.lo().to_bits().to_le_bytes());
            out.extend(v.hi().to_bits().to_le_bytes());
        }
    }
}

/// The corpus's canonical bytes and how many meshes went in.
fn corpus() -> (Vec<u8>, u32) {
    let mut rng = SplitMix64::new(Seed(0x0146));
    let mut out = Vec::new();
    let mut runs = 0;
    let mut run = |out: &mut Vec<u8>, input: &Polygons| {
        let mut meter = WorkMeter::new(Budget::of_work(1 << 20));
        write(out, &build(input, &Exec::serial(), &mut meter).unwrap());
        runs += 1;
    };
    for bits in [1, 3, 8, 20, 40] {
        let top = (1_i64 << bits) - 1;
        run(&mut out, &cuboid(top, top.max(2) - 1, 1 + top / 2));
    }
    let below = |rng: &mut SplitMix64, n: u64| rng.below(NonZeroU64::new(n).unwrap());
    for _ in 0..16 {
        let n = 2 + below(&mut rng, 12) as usize;
        let step = 1 + below(&mut rng, 1 << 20) as i64;
        let heights: Vec<Vec<i64>> = (0..n)
            .map(|_| {
                (0..n)
                    .map(|_| 1 + below(&mut rng, 1 << 24) as i64)
                    .collect()
            })
            .collect();
        run(&mut out, &prism(&heights, step).0);
    }
    run(&mut out, &frustum());
    (out, runs)
}

#[test]
fn exact_mesh_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.0), rec)
    });
    let (bytes, runs) = &checked.value;
    println!(
        "geom-mesh exact corpus: {runs} meshes, {} bytes, golden hash {}",
        bytes.len(),
        hex(sha256(bytes))
    );
    assert_eq!(*runs, 22, "every mesh ran");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target orders a plane, a vertex, a face or a \
             pair differently (compare this log's digest with another target's), or a definition \
             changed in place, which chapter 26.2 forbids"
        )
    });
}
