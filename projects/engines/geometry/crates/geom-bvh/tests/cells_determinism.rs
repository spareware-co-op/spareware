//! The replay hash of piece G55: cell indexes built over a fixed corpus, their bytes and their
//! answers hashed with SHA-256, must give the same digest on every target (chapters 21 and 28.13;
//! the golden store of piece G19).
//!
//! The corpus: from a fixed linear congruential sequence, sets of 1 to 20,000 grid points (sparse
//! over the frame, dense in a small cube, clustered, at the corners, all on one point), each
//! index's canonical bytes, the keys within six radii of twelve centres and the work each search
//! charged, and every pair within two radii. Any target that orders one code, one tie or one
//! answer differently changes the digest. The digest lives in the store; a change is a bug, or a
//! new definition with a golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_bvh::{CellIndex, Point};
use geom_num::grid::{COORD_MAX, P3};
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-bvh/cells";

/// Knuth's MMIX linear congruential sequence.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 20
    }
    fn int(&mut self, k: i64) -> i64 {
        (self.next() % (2 * k as u64 + 1)) as i64 - k
    }
}

fn p3(c: [i64; 3]) -> P3 {
    P3::new(c[0], c[1], c[2]).unwrap()
}

fn set(rng: &mut Lcg, n: usize, centre: [i64; 3], span: i64) -> Vec<Point> {
    (0..n)
        .map(|i| Point {
            key: (i as u64)
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .rotate_left(29),
            at: p3(std::array::from_fn(|a| {
                (centre[a] + rng.int(span)).clamp(-COORD_MAX, COORD_MAX)
            })),
        })
        .collect()
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

#[derive(Default)]
struct Recorder {
    bytes: Vec<u8>,
    indexes: u32,
}

impl Recorder {
    fn run(&mut self, rng: &mut Lcg, pts: &[Point], radii: [u64; 6]) {
        let idx = CellIndex::build(pts, &Exec::serial(), &mut meter()).unwrap();
        self.bytes.extend_from_slice(&idx.to_bytes().unwrap());
        for i in 0..12 {
            let c = if i % 2 == 0 && !pts.is_empty() {
                pts[(i * 7919) % pts.len()].at
            } else {
                p3(std::array::from_fn(|_| rng.int(COORD_MAX)))
            };
            for r in radii {
                let mut m = meter();
                let keys = idx.within(c, r, &mut m).unwrap();
                self.bytes
                    .extend_from_slice(&(keys.len() as u64).to_le_bytes());
                for k in keys {
                    self.bytes.extend_from_slice(&k.to_le_bytes());
                }
                self.bytes.extend_from_slice(&m.used().work.to_le_bytes());
            }
        }
        for r in [radii[1], radii[2]] {
            let mut m = meter();
            let pairs = idx.pairs_within(r, &Exec::serial(), &mut m).unwrap();
            self.bytes
                .extend_from_slice(&(pairs.len() as u64).to_le_bytes());
            for (a, b) in pairs {
                self.bytes.extend_from_slice(&a.to_le_bytes());
                self.bytes.extend_from_slice(&b.to_le_bytes());
            }
            self.bytes.extend_from_slice(&m.used().work.to_le_bytes());
        }
        self.indexes += 1;
    }
}

fn corpus() -> Recorder {
    let mut rng = Lcg(55);
    let mut rec = Recorder::default();
    let m = COORD_MAX;
    for (n, centre, span, radii) in [
        (1, [0, 0, 0], 100, [0, 1, 10, 100, 1 << 20, 1 << 42]),
        (50, [3, -4, 5], 20, [0, 1, 3, 9, 30, 100]),
        (
            2000,
            [0, 0, 0],
            m,
            [0, 1 << 20, 1 << 36, 1 << 39, 1 << 40, 1 << 41],
        ),
        (5000, [-77, 77, -7], 30, [0, 1, 2, 5, 12, 60]),
        (
            20_000,
            [m - 1000, -m + 1000, 0],
            1000,
            [0, 3, 8, 40, 500, 1 << 12],
        ),
    ] {
        let pts = set(&mut rng, n, centre, span);
        rec.run(&mut rng, &pts, radii);
    }
    let corners: Vec<Point> = (0..8u64)
        .map(|i| Point {
            key: 1000 - i,
            at: p3([
                if i & 1 == 0 { -m } else { m },
                if i & 2 == 0 { -m } else { m },
                if i & 4 == 0 { -m } else { m },
            ]),
        })
        .collect();
    rec.run(
        &mut rng,
        &corners,
        [0, 1 << 40, 2 * m as u64, 1 << 41, 1 << 42, u64::MAX],
    );
    let same: Vec<Point> = (0..300u64)
        .map(|i| Point {
            key: 1000 - 3 * i,
            at: p3([5, -6, 7]),
        })
        .collect();
    rec.run(&mut rng, &same, [0, 1, 2, 3, 4, 5]);
    rec
}

#[test]
fn cells_golden_hash_is_the_same_on_every_target() {
    // A broken code would make every search scan every point: stop at once instead. The code of
    // the frame's lowest corner is 0b111, of its highest 2^123 − 1, and of the origin 7·2^120.
    let m = COORD_MAX;
    assert_eq!(geom_bvh::morton(p3([-m, -m, -m])).unwrap(), 0b111);
    assert_eq!(geom_bvh::morton(p3([m, m, m])).unwrap(), (1 << 123) - 1);
    assert_eq!(geom_bvh::morton(p3([0, 0, 0])).unwrap(), 7 << 120);
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-bvh cell corpus: {} indexes, {} bytes, golden hash {}",
        rec.indexes,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert_eq!(rec.indexes, 7);
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target orders a code, a tie or an answer \
             differently (compare this log's digest with another target's), or a definition \
             changed in place, which chapter 26.2 forbids"
        )
    });
}
