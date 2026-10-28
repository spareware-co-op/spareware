//! Tests of the Morton-sorted cell index (piece G55) through its public interface.
//!
//! What this file proves: every search equals brute force (the points within a radius of a point,
//! and every pair within a radius) on seeded point sets that are sparse, dense, clustered, all on
//! one point, and at the frame's corners, for radii from zero to beyond the frame; the index's
//! bytes and every answer are the same for the serial executor, pools of 1 to 8 threads running
//! their tasks in any order, and any input order of the same points; the codes are the interleaved
//! bits of the offset coordinates and every aligned cell is one run of them; a repeated key is
//! refused; and a cancelled or budget-limited build or search stops with no answer.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    reason = "generators, brute-force oracles and a test pool of scoped threads (what geom-pool does for the facade); the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::num::NonZeroU64;

use geom_bvh::{AXIS_BITS, BvhError, CODE_BITS, CellIndex, Point, morton};
use geom_num::grid::{COORD_MAX, P3};
use geom_run::{Budget, CancelToken, Exec, Pool, Stop, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// A test pool: `threads` scoped threads, task i on thread i mod threads, each running its tasks
/// last first, so the tasks run in an order unlike the serial one.
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
                    for t in group.into_iter().rev() {
                        t();
                    }
                });
            }
        });
    }
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn below(rng: &mut SplitMix64, n: u64) -> i64 {
    rng.below(NonZeroU64::new(n).unwrap()) as i64
}

fn p3(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// `n` points with distinct scrambled keys, each coordinate within ±`span` of `centre`.
fn points(rng: &mut SplitMix64, n: usize, centre: [i64; 3], span: i64) -> Vec<Point> {
    (0..n)
        .map(|i| {
            let c: [i64; 3] = std::array::from_fn(|a| {
                (centre[a] + below(rng, 2 * span as u64 + 1) - span).clamp(-COORD_MAX, COORD_MAX)
            });
            Point {
                key: (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 8,
                at: p3(c[0], c[1], c[2]),
            }
        })
        .collect()
}

fn dist2(a: P3, b: P3) -> u128 {
    let d = [
        a.x.get() - b.x.get(),
        a.y.get() - b.y.get(),
        a.z.get() - b.z.get(),
    ];
    d.iter()
        .map(|v| (i128::from(*v) * i128::from(*v)) as u128)
        .sum()
}

fn brute_within(pts: &[Point], c: P3, r: u64) -> Vec<u64> {
    let mut out: Vec<u64> = pts
        .iter()
        .filter(|p| dist2(p.at, c) <= u128::from(r) * u128::from(r))
        .map(|p| p.key)
        .collect();
    out.sort_unstable();
    out
}

fn brute_pairs(pts: &[Point], r: u64) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    for (i, a) in pts.iter().enumerate() {
        for b in &pts[i + 1..] {
            if dist2(a.at, b.at) <= u128::from(r) * u128::from(r) {
                out.push((a.key.min(b.key), a.key.max(b.key)));
            }
        }
    }
    out.sort_unstable();
    out
}

/// The naive code: bit i of x, y and z to bits 3i, 3i + 1 and 3i + 2.
fn naive(p: P3) -> u128 {
    let o = [p.x.get(), p.y.get(), p.z.get()].map(|v| (v + (1 << 40)) as u128);
    let mut code = 0u128;
    for i in 0..41 {
        for (a, v) in o.iter().enumerate() {
            code |= ((v >> i) & 1) << (3 * i + a);
        }
    }
    code
}

/// The sets: sparse over the frame, dense in a small cube, clustered, at the corners, all equal.
fn sets(label: &str) -> Vec<(Vec<Point>, Vec<u64>)> {
    let seed = resolve(label, Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let m = COORD_MAX;
    let mut out = Vec::new();
    out.push((
        points(&mut rng, 600, [0, 0, 0], m),
        vec![0, 1 << 30, 1 << 38, 1 << 41],
    ));
    out.push((
        points(&mut rng, 900, [5, -7, 11], 40),
        vec![0, 1, 3, 7, 20, 100],
    ));
    out.push((
        points(&mut rng, 700, [m - 30, -m + 30, 12_345], 30),
        vec![0, 2, 9, 31, 64],
    ));
    let mut clusters = Vec::new();
    for k in 0..12 {
        let c = [
            below(&mut rng, 1 << 33),
            below(&mut rng, 1 << 33),
            below(&mut rng, 1 << 20),
        ];
        let mut more = points(&mut rng, 60, c, 1 << (4 + k % 6));
        for (i, p) in more.iter_mut().enumerate() {
            p.key = (k * 1000 + i) as u64;
        }
        clusters.extend(more);
    }
    out.push((clusters, vec![0, 15, 100, 1 << 10, 1 << 25]));
    let corners: Vec<Point> = (0..8u64)
        .map(|i| Point {
            key: 100 - i,
            at: p3(
                if i & 1 == 0 { -m } else { m },
                if i & 2 == 0 { -m } else { m },
                if i & 4 == 0 { -m } else { m },
            ),
        })
        .collect();
    out.push((
        corners,
        vec![0, 1 << 40, (1 << 41) - 2, 1 << 41, 1 << 42, u64::MAX],
    ));
    let same: Vec<Point> = (0..200u64)
        .map(|i| Point {
            key: 7 * i + 3,
            at: p3(-123, 456, -789),
        })
        .collect();
    out.push((same, vec![0, 1]));
    out
}

#[test]
fn codes_interleave_the_offset_bits() {
    assert_eq!((AXIS_BITS, CODE_BITS), (41, 123));
    let seed = resolve("cell codes", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let m = COORD_MAX;
    let mut cases = vec![
        p3(-m, -m, -m),
        p3(m, m, m),
        p3(0, 0, 0),
        p3(-1, 0, 1),
        p3(m, -m, 0),
    ];
    for _ in 0..5000 {
        cases.push(p3(
            below(&mut rng, 2 * m as u64 + 1) - m,
            below(&mut rng, 2 * m as u64 + 1) - m,
            below(&mut rng, 2 * m as u64 + 1) - m,
        ));
    }
    for p in cases {
        let code = morton(p).unwrap();
        assert_eq!(code, naive(p), "{p:?}");
        assert!(code < 1 << CODE_BITS);
    }
    assert_eq!(morton(p3(-m, -m, -m)).unwrap(), 0b111);
    // Every aligned cell is one run: codes inside a cell share their top bits, and sort between
    // its corners'.
    for level in [0u32, 1, 5, 20, 40] {
        let side = 1i64 << level;
        // Aligned corners in offset coordinates (k·2^level − 2^40), inside the frame.
        let corner = [-(1i64 << 40) + side, -(1i64 << 40), (1i64 << 40) - 2 * side];
        let lo = morton(p3(corner[0].clamp(-m, m), corner[1].clamp(-m, m), corner[2].clamp(-m, m))).unwrap();
        let top = morton(p3(
            (corner[0] + side - 1).clamp(-m, m),
            (corner[1] + side - 1).clamp(-m, m),
            (corner[2] + side - 1).clamp(-m, m),
        ))
        .unwrap();
        for _ in 0..100 {
            let q: [i64; 3] =
                std::array::from_fn(|a| (corner[a] + below(&mut rng, side as u64)).clamp(-m, m));
            let c = morton(p3(q[0], q[1], q[2])).unwrap();
            assert!(lo <= c && c <= top);
            assert_eq!(c >> (3 * level), lo >> (3 * level));
        }
    }
}

#[test]
fn searches_equal_brute_force() {
    let mut checked = 0;
    for (pts, radii) in sets("cell searches") {
        let idx = CellIndex::build(&pts, &Exec::serial(), &mut meter()).unwrap();
        assert_eq!(idx.len(), pts.len());
        assert!(!idx.is_empty());
        assert!(idx.codes().windows(2).all(|w| w[0] <= w[1]));
        for (w, p) in idx.points().windows(2).zip(idx.codes().windows(2)) {
            assert!(p[0] < p[1] || w[0].key < w[1].key, "ties in key order");
        }
        for (c, p) in idx.codes().iter().zip(idx.points()) {
            assert_eq!(*c, morton(p.at).unwrap());
        }
        let seed = resolve("cell search centres", Seed::DEFAULT).unwrap();
        let mut rng = SplitMix64::new(seed);
        for &r in &radii {
            // Centres at the points themselves, near them, and anywhere.
            for i in 0..40 {
                let base = pts[(i * 37) % pts.len()].at;
                let c = if i % 3 == 0 {
                    base
                } else {
                    let jitter = |v: i64, rng: &mut SplitMix64| {
                        (v + below(rng, 2 * r.min(1 << 20) + 3) - r.min(1 << 20) as i64 - 1)
                            .clamp(-COORD_MAX, COORD_MAX)
                    };
                    p3(
                        jitter(base.x.get(), &mut rng),
                        jitter(base.y.get(), &mut rng),
                        jitter(base.z.get(), &mut rng),
                    )
                };
                assert_eq!(
                    idx.within(c, r, &mut meter()).unwrap(),
                    brute_within(&pts, c, r),
                    "radius {r} at {c:?}"
                );
                checked += 1;
            }
            if pts.len() <= 1000 {
                assert_eq!(
                    idx.pairs_within(r, &Exec::serial(), &mut meter()).unwrap(),
                    brute_pairs(&pts, r),
                    "pairs within {r}"
                );
            }
        }
    }
    assert!(checked > 1000);
    // The empty index answers nothing.
    let empty = CellIndex::build(&[], &Exec::serial(), &mut meter()).unwrap();
    assert!(empty.is_empty());
    assert_eq!(
        empty.within(p3(0, 0, 0), 1 << 41, &mut meter()).unwrap(),
        Vec::<u64>::new()
    );
    assert_eq!(
        empty
            .pairs_within(5, &Exec::serial(), &mut meter())
            .unwrap(),
        Vec::<(u64, u64)>::new()
    );
}

#[test]
fn the_same_index_for_any_order_and_any_pool() {
    let seed = resolve("cell bytes", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    // Enough points that the code phase and the sorts run in many chunks.
    let mut pts = points(&mut rng, 30_000, [0, 0, 0], 1 << 12);
    pts.extend(points(&mut rng, 5_000, [77, 77, 77], 3));
    for (i, p) in pts.iter_mut().enumerate() {
        p.key = (i as u64).wrapping_mul(0xD1B5_4A32_D192_ED03);
    }
    let reference = CellIndex::build(&pts, &Exec::serial(), &mut meter()).unwrap();
    let bytes = reference.to_bytes().unwrap();
    let pairs = reference
        .pairs_within(2, &Exec::serial(), &mut meter())
        .unwrap();
    assert!(pairs.len() > 1000);
    let mut sorted = pts.clone();
    sorted.sort_by_key(|p| p.key);
    let mut reversed = pts.clone();
    reversed.reverse();
    for input in [&pts, &sorted, &reversed] {
        for threads in 1..=8 {
            let pool = Threads(threads);
            let exec = Exec::with_pool(&pool);
            let idx = CellIndex::build(input, &exec, &mut meter()).unwrap();
            assert_eq!(idx.to_bytes().unwrap(), bytes, "{threads} threads");
            assert_eq!(idx, reference);
            if threads % 3 == 1 {
                assert_eq!(idx.pairs_within(2, &exec, &mut meter()).unwrap(), pairs);
            }
        }
    }
    // The documented bytes.
    let one = CellIndex::build(
        &[Point {
            key: 9,
            at: p3(1, -2, 3),
        }],
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    let mut want = b"SKCELL1".to_vec();
    want.extend_from_slice(&1u64.to_le_bytes());
    want.extend_from_slice(&morton(p3(1, -2, 3)).unwrap().to_le_bytes());
    want.extend_from_slice(&9u64.to_le_bytes());
    for v in [1i64, -2, 3] {
        want.extend_from_slice(&v.to_le_bytes());
    }
    assert_eq!(one.to_bytes().unwrap(), want);
}

#[test]
fn a_repeated_key_is_refused() {
    let pts = [
        Point {
            key: 5,
            at: p3(0, 0, 0),
        },
        Point {
            key: 3,
            at: p3(1, 0, 0),
        },
        Point {
            key: 5,
            at: p3(2, 0, 0),
        },
        Point {
            key: 3,
            at: p3(3, 0, 0),
        },
    ];
    assert_eq!(
        CellIndex::build(&pts, &Exec::serial(), &mut meter()),
        Err(BvhError::DuplicateKey(3))
    );
    let sorted = [
        Point {
            key: 1,
            at: p3(0, 0, 0),
        },
        Point {
            key: 1,
            at: p3(0, 0, 0),
        },
    ];
    assert_eq!(
        CellIndex::build(&sorted, &Exec::serial(), &mut meter()),
        Err(BvhError::DuplicateKey(1))
    );
}

#[test]
fn cancelled_or_short_budgets_stop_with_no_answer() {
    let seed = resolve("cell stops", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let pts = points(&mut rng, 20_000, [0, 0, 0], 1 << 10);
    let token = CancelToken::new();
    token.cancel();
    let mut cancelled = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    assert_eq!(
        CellIndex::build(&pts, &Exec::serial(), &mut cancelled),
        Err(BvhError::Stopped(Stop::Cancelled))
    );
    let idx = CellIndex::build(&pts, &Exec::serial(), &mut meter()).unwrap();
    let mut cancelled = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    assert_eq!(
        idx.pairs_within(3, &Exec::serial(), &mut cancelled),
        Err(BvhError::Stopped(Stop::Cancelled))
    );
    let mut short = WorkMeter::new(Budget::of_work(10));
    assert!(matches!(
        idx.within(p3(0, 0, 0), 1 << 9, &mut short),
        Err(BvhError::Stopped(Stop::OverBudget(_)))
    ));
    let mut short = WorkMeter::new(Budget::of_work(100));
    assert!(matches!(
        idx.pairs_within(1 << 8, &Exec::serial(), &mut short),
        Err(BvhError::Stopped(Stop::OverBudget(_)) | BvhError::BudgetExceeded(_))
    ));
    // A search charges one unit per point it scans: the same count on every run.
    let mut a = meter();
    let mut b = meter();
    idx.within(p3(5, 5, 5), 300, &mut a).unwrap();
    idx.within(p3(5, 5, 5), 300, &mut b).unwrap();
    assert_eq!(a.used(), b.used());
    assert!(a.used().work > 0);
}
