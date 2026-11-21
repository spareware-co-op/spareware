//! The replay hash of Minkowski sums (piece G150): sums of boxes, prisms and tetrahedra by both of
//! `minkowski`'s methods, one hull for two convex solids and the union of the pairs' hulls
//! otherwise, a sum with corners off the grid, sums that reach the frame's last step, and the sums
//! it refuses. Every array of each sum's mesh, its runs, its volume and its class, and each
//! refusal, are written out and hashed with SHA-256: the digest must be the same on every target
//! and on every thread count (chapters 19.6, 21 and 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed `SplitMix64` stream, integers only, so the inputs are the same on
//! every target by construction:
//!
//! - **One hull:** 6 pairs of boxes of sides up to 2^20 steps, moved up to 2^38 steps each way; a
//!   triangular prism and a box; a box and a tetrahedron.
//! - **The union of hulls:** 2 L-shaped prisms, each with a tetrahedron (an L and a box, the
//!   smallest pair after these, takes over ten seconds unoptimised, and `tests/minkowski.rs` holds
//!   it to its counted cells).
//! - **Off the grid:** the cut pyramid of `shapes::frustum`, whose top corners are thirds, with a
//!   box, `Certified` by the half step it is committed by (with an L, its union runs past a
//!   budget of 2^28 units).
//! - **The frame's ends:** two boxes whose sum reaches 2^40 − 1 steps on every axis, and two whose
//!   sum reaches −(2^40 − 1).
//! - **Refused:** the first pair of the frame's ends moved one step further (past the frame), and
//!   an open mesh with a solid, in both orders.
//!
//! A target that orders a plane, a vertex, a face or a run differently, rounds a corner
//! differently or refuses differently changes the digest. The thread-count test sums the same
//! corpus through `geom_run::Exec::with_pool` on pools of 1, 2, 3 and 8 threads (`miri::pool`
//! says which pool), where the builder, the hulls' meshes and the union run in chunks, and each
//! must give the serial digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;

use geom_mesh::exact::build::{Corner, Polygons, build};
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::{ExactMesh, Facing, Vertex};
use geom_mesh::minkowski::minkowski;
use geom_num::grid::GridStep;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::golden::{self, hex, sha256};
use geom_test::{Seed, SplitMix64};

mod miri;
mod shapes;

use shapes::{cuboid, frustum, g, soup, source};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-mesh/minkowski";

/// The sums the corpus makes and the refusals it meets.
const SUMS: u32 = 13;
const REFUSALS: u32 = 3;

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

/// A whole number in `low..=high`.
fn between(rng: &mut SplitMix64, low: i64, high: i64) -> i64 {
    low + below(rng, (high - low + 1) as u64) as i64
}

/// `input`, moved by (`x`, `y`, `z`).
fn moved(mut input: Polygons, [x, y, z]: [i64; 3]) -> Polygons {
    for v in &mut input.vertices {
        let Corner::Grid(p) = *v else {
            panic!("these shapes are on the grid");
        };
        *v = g(p.x.get() + x, p.y.get() + y, p.z.get() + z);
    }
    input
}

/// A prism of `outline`, counter-clockwise from above, from z = 0 to `height`.
fn prism(outline: &[(i64, i64)], height: i64) -> Polygons {
    let n = outline.len() as u32;
    let mut vertices = Vec::new();
    for z in [0, height] {
        for &(x, y) in outline {
            vertices.push(g(x, y, z));
        }
    }
    let mut faces: Vec<Vec<u32>> = vec![(0..n).rev().collect(), (n..2 * n).collect()];
    for i in 0..n {
        let j = (i + 1) % n;
        faces.push(vec![i, j, j + n, i + n]);
    }
    let refs: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
    soup(vertices, &refs)
}

/// The tetrahedron with corners at the origin and at `a`, `b` and `c` on the axes, outward.
fn tetrahedron(a: i64, b: i64, c: i64) -> Polygons {
    soup(
        vec![g(0, 0, 0), g(a, 0, 0), g(0, b, 0), g(0, 0, c)],
        &[&[0, 2, 1], &[0, 1, 3], &[0, 3, 2], &[1, 2, 3]],
    )
}

/// An L of arms `w` and `h` and width `t`, from z = 0 to `height`.
fn ell(w: i64, h: i64, t: i64, height: i64) -> Polygons {
    prism(&[(0, 0), (w, 0), (w, t), (t, t), (t, h), (0, h)], height)
}

/// A seeded L, its arms 3 to 9 steps, its width under both, its height 1 to 5.
fn seeded_ell(rng: &mut SplitMix64) -> Polygons {
    let (w, h) = (between(rng, 3, 9), between(rng, 3, 9));
    let t = between(rng, 1, w.min(h) - 1);
    ell(w, h, t, between(rng, 1, 5))
}

/// A shift of up to `reach` steps each way on every axis.
fn shift(rng: &mut SplitMix64, reach: i64) -> [i64; 3] {
    [0, 1, 2].map(|_| between(rng, -reach, reach))
}

/// The corpus's pairs to sum, in order: the last three are refused.
fn pairs() -> Vec<(Polygons, Polygons)> {
    let mut rng = SplitMix64::new(Seed(0x0150));
    let mut pairs = Vec::new();
    for _ in 0..6 {
        let mut side = || between(&mut rng, 1, 1 << 20);
        let (a, b) = (
            cuboid(side(), side(), side()),
            cuboid(side(), side(), side()),
        );
        pairs.push((
            moved(a, shift(&mut rng, 1 << 38)),
            moved(b, shift(&mut rng, 1 << 38)),
        ));
    }
    pairs.push((
        prism(&[(0, 0), (4, 0), (0, 4)], 3),
        moved(cuboid(2, 2, 5), [-1, 2, 0]),
    ));
    pairs.push((cuboid(3, 1, 2), tetrahedron(2, 3, 4)));
    for _ in 0..2 {
        let a = seeded_ell(&mut rng);
        let (p, q, r) = (
            between(&mut rng, 1, 4),
            between(&mut rng, 1, 4),
            between(&mut rng, 1, 4),
        );
        pairs.push((
            moved(a, shift(&mut rng, 1 << 30)),
            moved(tetrahedron(p, q, r), shift(&mut rng, 1 << 30)),
        ));
    }
    pairs.push((frustum(), cuboid(2, 2, 2)));
    let top = 1_i64 << 39;
    pairs.push((
        moved(cuboid(2, 2, 2), [top - 2; 3]),
        moved(cuboid(3, 3, 3), [top - 4; 3]),
    ));
    pairs.push((
        moved(cuboid(2, 2, 2), [-top; 3]),
        moved(cuboid(3, 3, 3), [1 - top; 3]),
    ));
    // Refused: one step past the frame, and an open mesh in both orders.
    pairs.push((
        moved(cuboid(2, 2, 2), [top - 2; 3]),
        moved(cuboid(3, 3, 3), [top - 3; 3]),
    ));
    let open = soup(vec![g(0, 0, 0), g(4, 0, 0), g(0, 4, 0)], &[&[0, 1, 2]]);
    pairs.push((ell(4, 4, 1, 2), open.clone()));
    pairs.push((open, cuboid(1, 1, 1)));
    pairs
}

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

fn text(out: &mut Vec<u8>, t: &str) {
    out.extend((t.len() as u64).to_le_bytes());
    out.extend(t.as_bytes());
}

/// Every array of `m`, its runs and its volume, in canonical order.
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
    words(out, &p.he_prop);
    // The runs, the names the mesh publishes, as their debug text: names, roles, flags and the
    // faces each covers.
    text(out, &format!("{:?}", p.runs));
    match m
        .volume(&mut WorkMeter::new(Budget::of_work(1 << 24)))
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

/// The corpus's canonical bytes, how many sums went in, and how many refusals.
fn corpus(exec: &Exec<'_>) -> (Vec<u8>, u32, u32) {
    let mut out = Vec::new();
    let (mut sums, mut refusals) = (0, 0);
    for (a, b) in pairs() {
        let mesh = |input: &Polygons| build(input, exec, &mut WorkMeter::new(Budget::UNLIMITED));
        let (a, b) = (mesh(&a).unwrap(), mesh(&b).unwrap());
        // Far above what the corpus needs: a sum that stops advancing fails, not hangs.
        let mut meter = WorkMeter::new(Budget::of_work(1 << 28));
        match minkowski(&a, &b, (source(9), GridStep::Q0), exec, &mut meter) {
            Ok(sum) => {
                out.push(0);
                write(&mut out, &sum.mesh);
                text(&mut out, &format!("{:?}", sum.class));
                sums += 1;
            }
            Err(e) => {
                out.push(1);
                text(&mut out, &format!("{e:?}"));
                refusals += 1;
            }
        }
    }
    (out, sums, refusals)
}

#[test]
fn minkowski_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus(&Exec::serial());
        (sha256(&rec.0), rec)
    });
    let (bytes, sums, refusals) = &checked.value;
    println!(
        "geom-mesh minkowski corpus: {sums} sums, {refusals} refusals, {} bytes, golden hash {}",
        bytes.len(),
        hex(sha256(bytes))
    );
    assert_eq!((*sums, *refusals), (SUMS, REFUSALS), "every pair ran");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target orders a plane, a vertex, a face or a \
             run differently, rounds a corner differently or refuses differently (compare this \
             log's digest with another target's), or a definition changed in place, which \
             chapter 26.2 forbids"
        )
    });
}

/// Chapter 21's thread-count test: the same digest on pools of 1, 2, 3 and 8 threads as on one
/// thread. WebAssembly has no threads.
#[cfg(not(target_family = "wasm"))]
#[test]
fn minkowski_golden_hash_is_the_same_on_every_thread_count() {
    let one = hex(sha256(&corpus(&Exec::serial()).0));
    for threads in golden::THREAD_COUNTS {
        let pool = miri::pool(threads);
        let got = hex(sha256(&corpus(&Exec::with_pool(&*pool)).0));
        assert_eq!(got, one, "{threads} threads");
    }
}
