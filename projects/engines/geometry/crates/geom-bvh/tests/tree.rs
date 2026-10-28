//! Tests of the box tree (piece G54) through its public interface.
//!
//! What this file proves: every query equals brute force (boxes meeting a box, pairs within a
//! tree, pairs between two trees) on seeded 2D and 3D boxes, small and large, sparse and dense,
//! with trees big enough to run the parallel subtree phase; a tree's bytes are the same for the
//! serial executor, pools of 1 to 8 threads running their tasks in any order, and any input order
//! of the same entries; every node holds what is below it, children follow parents, leaves hold at
//! most `LEAF_MAX` entries, and no tree is deeper than `MAX_DEPTH`, even on nested boxes that make
//! every surface-area split lopsided; a refit keeps the answers right, and past its limit
//! rebuilds into exactly a fresh build; refusals leave the tree as it was; a cancelled or
//! budget-limited build or query stops with no answer; and boxes convert, round and pad as their
//! classes say.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::needless_range_loop,
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

use geom_bvh::{
    Aabb, BoxTree, BvhError, Entry, Inflation, LEAF_MAX, LIMIT, MAX_DEPTH, NodeKind, Refitted,
};
use geom_math::{Box2, Box3, F2, F3, FBox2, FBox3};
use geom_num::grid::{P2, P3};
use geom_num::real::Real;
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

/// `n` entries with distinct keys (scrambled, not in key order), boxes of size up to `size` in a
/// cube of side `span`, a few of them much larger.
fn entries<const D: usize>(rng: &mut SplitMix64, n: usize, span: i64, size: i64) -> Vec<Entry<D>> {
    (0..n)
        .map(|i| {
            let big = below(rng, 50) == 0;
            let mut lo = [0i64; D];
            let mut hi = [0i64; D];
            for a in 0..D {
                lo[a] = below(rng, 2 * span as u64 + 1) - span;
                let s = if big {
                    below(rng, span as u64 + 1)
                } else {
                    below(rng, size as u64 + 1)
                };
                hi[a] = lo[a] + s;
            }
            Entry {
                key: (i as u64)
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    .rotate_left(17),
                bbox: Aabb::new(lo, hi).unwrap(),
            }
        })
        .collect()
}

fn brute_overlapping<const D: usize>(es: &[Entry<D>], q: &Aabb<D>) -> Vec<u64> {
    let mut v: Vec<u64> = es
        .iter()
        .filter(|e| e.bbox.overlaps(q))
        .map(|e| e.key)
        .collect();
    v.sort_unstable();
    v
}

fn brute_pairs<const D: usize>(es: &[Entry<D>]) -> Vec<(u64, u64)> {
    let mut v = Vec::new();
    for i in 0..es.len() {
        for j in i + 1..es.len() {
            if es[i].bbox.overlaps(&es[j].bbox) {
                let (a, b) = (es[i].key, es[j].key);
                v.push((a.min(b), a.max(b)));
            }
        }
    }
    v.sort_unstable();
    v
}

fn brute_pairs_with<const D: usize>(a: &[Entry<D>], b: &[Entry<D>]) -> Vec<(u64, u64)> {
    let mut v = Vec::new();
    for x in a {
        for y in b {
            if x.bbox.overlaps(&y.bbox) {
                v.push((x.key, y.key));
            }
        }
    }
    v.sort_unstable();
    v
}

/// Every structural promise: boxes hold what is below, children after parents, leaves small and
/// covering every entry once, depth within the limit.
fn check_structure<const D: usize>(t: &BoxTree<D>) {
    let nodes = t.nodes();
    if t.is_empty() {
        assert_eq!(nodes.len(), 0);
        return;
    }
    let mut covered = vec![false; t.len()];
    for (i, n) in nodes.iter().enumerate() {
        match n.kind() {
            NodeKind::Leaf { start, len } => {
                assert!(len as usize <= LEAF_MAX && len > 0, "leaf of {len}");
                for p in start as usize..(start + len) as usize {
                    assert!(n.bbox().contains_box(&t.entries()[p].bbox));
                    assert!(!covered[p]);
                    covered[p] = true;
                }
            }
            NodeKind::Inner { left, right } => {
                for c in [left, right] {
                    assert!(c as usize > i, "child {c} of {i}");
                    assert!(n.bbox().contains_box(&nodes[c as usize].bbox()));
                }
            }
        }
    }
    assert!(covered.iter().all(|&c| c));
    assert!(t.depth().unwrap() <= MAX_DEPTH);
}

fn agree<const D: usize>(label: &str, n: usize, span: i64, size: i64) {
    let seed = resolve(label, Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let es = entries::<D>(&mut rng, n, span, size);
    let others = entries::<D>(&mut rng, n / 2 + 1, span, size);
    let t = BoxTree::build(&es, &Exec::serial(), &mut meter()).unwrap();
    let u = BoxTree::build(&others, &Exec::serial(), &mut meter()).unwrap();
    check_structure(&t);
    assert_eq!(t.len(), n);
    for _ in 0..20 {
        let q = entries::<D>(&mut rng, 1, span, 4 * size)[0].bbox;
        assert_eq!(
            t.overlapping(&q, &mut meter()).unwrap(),
            brute_overlapping(&es, &q)
        );
    }
    if n <= 3000 {
        assert_eq!(t.pairs(&mut meter()).unwrap(), brute_pairs(&es), "{label}");
        assert_eq!(
            t.pairs_with(&u, &mut meter()).unwrap(),
            brute_pairs_with(&es, &others),
            "{label}"
        );
        assert_eq!(
            u.pairs_with(&t, &mut meter()).unwrap(),
            brute_pairs_with(&others, &es),
            "{label}"
        );
    }
}

#[test]
fn queries_equal_brute_force_in_2d_and_3d() {
    for (n, span, size) in [
        (0, 10, 2),
        (1, 10, 2),
        (2, 10, 2),
        (5, 10, 3),
        (17, 50, 10),
        (300, 1000, 40),
    ] {
        agree::<2>(&format!("box tree 2D, {n} boxes"), n, span, size);
        agree::<3>(&format!("box tree 3D, {n} boxes"), n, span, size);
    }
    agree::<3>("box tree 3D, dense", 2000, 100, 30);
    agree::<2>("box tree 2D, past the grain", 9000, 1 << 30, 1 << 22);
    agree::<3>("box tree 3D, past the grain", 9000, 1 << 30, 1 << 24);
}

#[test]
fn the_tree_is_the_same_for_every_pool_and_input_order() {
    let seed = resolve("box tree bytes", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let es = entries::<3>(&mut rng, 20_000, 1 << 35, 1 << 25);
    let base = BoxTree::build(&es, &Exec::serial(), &mut meter()).unwrap();
    let bytes = base.to_bytes().unwrap();
    for threads in 1..=8 {
        let pool = Threads(threads);
        let t = BoxTree::build(&es, &Exec::with_pool(&pool), &mut meter()).unwrap();
        assert_eq!(t.to_bytes().unwrap(), bytes, "{threads} threads");
    }
    let mut reversed = es.clone();
    reversed.reverse();
    assert_eq!(
        BoxTree::build(&reversed, &Exec::serial(), &mut meter())
            .unwrap()
            .to_bytes()
            .unwrap(),
        bytes
    );
    let es2 = entries::<2>(&mut rng, 10_000, 1 << 35, 1 << 25);
    let b2 = BoxTree::build(&es2, &Exec::serial(), &mut meter())
        .unwrap()
        .to_bytes()
        .unwrap();
    let pool = Threads(3);
    assert_eq!(
        BoxTree::build(&es2, &Exec::with_pool(&pool), &mut meter())
            .unwrap()
            .to_bytes()
            .unwrap(),
        b2
    );
}

#[test]
fn nested_and_identical_boxes_stay_within_the_depth_limit() {
    // Boxes nested at every power of two: the surface-area split peels one box at a time, so only
    // the switch to median splits keeps the depth bounded.
    let nested: Vec<Entry<3>> = (0..40u64)
        .flat_map(|i| {
            (0..4u64).map(move |j| Entry {
                key: i * 4 + j,
                bbox: Aabb::new([0, 0, 0], [1 << i, 1 << i, (1 << i) + j as i64]).unwrap(),
            })
        })
        .collect();
    let t = BoxTree::build(&nested, &Exec::serial(), &mut meter()).unwrap();
    check_structure(&t);
    let q = Aabb::new([1 << 20, 0, 0], [1 << 20, 0, 0]).unwrap();
    assert_eq!(
        t.overlapping(&q, &mut meter()).unwrap(),
        brute_overlapping(&nested, &q)
    );
    assert_eq!(t.pairs(&mut meter()).unwrap(), brute_pairs(&nested));
    // Every box the same: no centre separates them, so every split is the median in key order.
    let same: Vec<Entry<2>> = (0..1000u64)
        .map(|k| Entry {
            key: 999 - k,
            bbox: Aabb::new([5, 5], [6, 7]).unwrap(),
        })
        .collect();
    let t = BoxTree::build(&same, &Exec::serial(), &mut meter()).unwrap();
    check_structure(&t);
    let keys: Vec<u64> = t.entries().iter().map(|e| e.key).collect();
    assert_eq!(keys, (0..1000).collect::<Vec<_>>());
    assert_eq!(t.depth().unwrap(), 8);
    assert_eq!(t.pairs(&mut meter()).unwrap().len(), 1000 * 999 / 2);
}

#[test]
fn a_refit_keeps_answers_right_and_rebuilds_past_its_limit() {
    let seed = resolve("box tree refit", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut es = entries::<3>(&mut rng, 600, 1000, 30);
    let mut t = BoxTree::build(&es, &Exec::serial(), &mut meter()).unwrap();
    let built = t.built_cost();
    assert_eq!(built, t.cost());
    // A small move: refit only.
    let moved: Vec<Entry<3>> = es[..10]
        .iter()
        .map(|e| {
            let (lo, hi) = (e.bbox.lo(), e.bbox.hi());
            Entry {
                key: e.key,
                bbox: Aabb::new([lo[0] + 1, lo[1], lo[2]], [hi[0] + 1, hi[1], hi[2]]).unwrap(),
            }
        })
        .collect();
    assert_eq!(
        t.refit(&moved, &Exec::serial(), &mut meter()),
        Ok(Refitted::Refit)
    );
    for m in &moved {
        es.iter_mut().find(|e| e.key == m.key).unwrap().bbox = m.bbox;
    }
    check_structure(&t);
    assert_eq!(t.pairs(&mut meter()).unwrap(), brute_pairs(&es));
    assert_eq!(t.built_cost(), built);
    // Every box sent far apart: the cost passes twice the built cost, so the tree is rebuilt, and
    // equals a fresh build of the same entries.
    let far: Vec<Entry<3>> = es
        .iter()
        .enumerate()
        .map(|(i, e)| Entry {
            key: e.key,
            bbox: Aabb::new([i as i64 * 1_000_000; 3], [i as i64 * 1_000_000 + 5; 3]).unwrap(),
        })
        .collect();
    assert_eq!(
        t.refit(&far, &Exec::serial(), &mut meter()),
        Ok(Refitted::Rebuilt)
    );
    let fresh = BoxTree::build(&far, &Exec::serial(), &mut meter()).unwrap();
    assert_eq!(t, fresh);
    assert_eq!(t.pairs(&mut meter()).unwrap().len(), 0);
    // Refusals leave the tree as it was.
    let before = t.clone();
    let unknown = Entry {
        key: u64::MAX,
        bbox: Aabb::new([0; 3], [1; 3]).unwrap(),
    };
    assert_eq!(
        t.refit(&[far[0], unknown], &Exec::serial(), &mut meter()),
        Err(BvhError::UnknownKey(u64::MAX))
    );
    assert_eq!(
        t.refit(&[far[3], far[3]], &Exec::serial(), &mut meter()),
        Err(BvhError::DuplicateKey(far[3].key))
    );
    assert_eq!(t, before);
}

#[test]
fn builds_refuse_repeated_keys_and_handle_tiny_trees() {
    let b = Aabb::new([0, 0], [1, 1]).unwrap();
    let es = [
        Entry { key: 9, bbox: b },
        Entry { key: 3, bbox: b },
        Entry { key: 9, bbox: b },
        Entry { key: 3, bbox: b },
    ];
    assert_eq!(
        BoxTree::build(&es, &Exec::serial(), &mut meter()),
        Err(BvhError::DuplicateKey(3))
    );
    let empty = BoxTree::<2>::build(&[], &Exec::serial(), &mut meter()).unwrap();
    assert!(empty.is_empty() && empty.nodes().is_empty());
    assert_eq!((empty.depth(), empty.cost()), (Ok(0), 0));
    assert_eq!(empty.overlapping(&b, &mut meter()).unwrap().len(), 0);
    assert_eq!(empty.pairs(&mut meter()).unwrap().len(), 0);
    let one = BoxTree::build(&es[..1], &Exec::serial(), &mut meter()).unwrap();
    assert_eq!(empty.pairs_with(&one, &mut meter()).unwrap().len(), 0);
    assert_eq!(one.pairs_with(&empty, &mut meter()).unwrap().len(), 0);
    assert_eq!(one.overlapping(&b, &mut meter()).unwrap(), vec![9]);
    assert_eq!((one.len(), one.depth()), (1, Ok(0)));
    // One leaf of one 2 × 2 box: half the perimeter in grid points is 2 + 2.
    assert_eq!(one.cost(), 4);
}

#[test]
fn the_bytes_are_the_documented_encoding() {
    let es = [Entry {
        key: 0x0102_0304_0506_0708,
        bbox: Aabb::new([-1, 2], [3, 4]).unwrap(),
    }];
    let t = BoxTree::build(&es, &Exec::serial(), &mut meter()).unwrap();
    let mut want = b"SKBVH1".to_vec();
    want.push(2);
    want.extend_from_slice(&1u32.to_le_bytes());
    want.extend_from_slice(&1u32.to_le_bytes());
    want.extend_from_slice(&0x0102_0304_0506_0708u64.to_le_bytes());
    for c in [-1i64, 2, 3, 4] {
        want.extend_from_slice(&c.to_le_bytes());
    }
    for c in [-1i64, 2, 3, 4] {
        want.extend_from_slice(&c.to_le_bytes());
    }
    want.push(0);
    want.extend_from_slice(&0u32.to_le_bytes());
    want.extend_from_slice(&1u32.to_le_bytes());
    assert_eq!(t.to_bytes().unwrap(), want);
}

#[test]
fn cancelled_or_short_budgets_stop_with_no_answer() {
    let seed = resolve("box tree stops", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let es = entries::<3>(&mut rng, 5000, 1 << 20, 1 << 12);
    let token = CancelToken::new();
    token.cancel();
    let mut cancelled = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    assert_eq!(
        BoxTree::build(&es, &Exec::serial(), &mut cancelled),
        Err(BvhError::Stopped(Stop::Cancelled))
    );
    let t = BoxTree::build(&es, &Exec::serial(), &mut meter()).unwrap();
    let mut cancelled = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    assert_eq!(
        t.pairs(&mut cancelled),
        Err(BvhError::Stopped(Stop::Cancelled))
    );
    let mut short = WorkMeter::new(Budget::of_work(10));
    assert!(matches!(
        t.overlapping(
            &es[0]
                .bbox
                .inflated(Inflation::Tolerant(Real::new(1e6).unwrap()))
                .unwrap(),
            &mut short
        ),
        Err(BvhError::Stopped(Stop::OverBudget(_)))
    ));
    let mut short = WorkMeter::new(Budget::of_work(100));
    assert!(matches!(
        BoxTree::build(&es, &Exec::serial(), &mut short),
        Err(BvhError::Stopped(Stop::OverBudget(_)) | BvhError::BudgetExceeded(_))
    ));
}

#[test]
fn boxes_convert_round_and_pad_by_class() {
    let p2 = |x, y| P2::new(x, y).unwrap();
    let p3 = |x, y, z| P3::new(x, y, z).unwrap();
    let b2: Aabb<2> = Box2::new(p2(-3, 4), p2(5, 6)).unwrap().into();
    assert_eq!((b2.lo(), b2.hi()), ([-3, 4], [5, 6]));
    let b3: Aabb<3> = Box3::new(p3(1, 2, 3), p3(4, 5, 6)).unwrap().into();
    assert_eq!((b3.lo(), b3.hi()), ([1, 2, 3], [4, 5, 6]));
    // Rounded outward, then padded by the bound rounded up.
    let f = FBox3::new(
        F3::new(0.5, -0.5, 2.0).unwrap(),
        F3::new(1.25, 0.0, 2.0).unwrap(),
    )
    .unwrap();
    let exact = Aabb::from_fbox3(&f, Inflation::Exact).unwrap();
    assert_eq!((exact.lo(), exact.hi()), ([0, -1, 2], [2, 0, 2]));
    let certified = Aabb::from_fbox3(&f, Inflation::Certified(Real::new(0.25).unwrap())).unwrap();
    assert_eq!((certified.lo(), certified.hi()), ([-1, -2, 1], [3, 1, 3]));
    let tolerant = exact
        .inflated(Inflation::Tolerant(Real::new(2.0).unwrap()))
        .unwrap();
    assert_eq!((tolerant.lo(), tolerant.hi()), ([-2, -3, 0], [4, 2, 4]));
    let g = FBox2::new(F2::new(-2.5, 7.0).unwrap(), F2::new(-2.0, 7.5).unwrap()).unwrap();
    let flat = Aabb::from_fbox2(&g, Inflation::Exact).unwrap();
    assert_eq!((flat.lo(), flat.hi()), ([-3, 7], [-2, 8]));
    // Refusals.
    let neg = Inflation::Certified(Real::new(-0.5).unwrap());
    assert_eq!(exact.inflated(neg), Err(BvhError::BadBound));
    assert_eq!(Aabb::<1>::new([0], [1]), Err(BvhError::Dimension));
    assert_eq!(Aabb::<4>::new([0; 4], [1; 4]), Err(BvhError::Dimension));
    assert_eq!(Aabb::new([2, 0], [1, 0]), Err(BvhError::Empty));
    assert_eq!(
        Aabb::new([0, -LIMIT - 1], [0, 0]),
        Err(BvhError::OutOfRange)
    );
    assert_eq!(Aabb::new([0, 0], [0, LIMIT + 1]), Err(BvhError::OutOfRange));
    assert!(Aabb::new([-LIMIT, -LIMIT], [LIMIT, LIMIT]).is_ok());
    let edge = Aabb::new([0, 0], [LIMIT - 1, 0]).unwrap();
    assert_eq!(
        edge.inflated(Inflation::Tolerant(Real::new(1.5).unwrap())),
        Err(BvhError::OutOfRange)
    );
    assert_eq!(
        Aabb::from_float([0.0, f64::NAN], [1.0, 1.0], Inflation::Exact),
        Err(BvhError::BadBound)
    );
    assert_eq!(
        Aabb::from_float([0.0, 2.0], [1.0, 1.0], Inflation::Exact),
        Err(BvhError::Empty)
    );
    assert_eq!(
        Aabb::from_float([0.0, 0.0], [1.0, 1e300], Inflation::Exact),
        Err(BvhError::OutOfRange)
    );
    // Touching boxes overlap; the measure counts grid points.
    let a = Aabb::new([0, 0, 0], [1, 1, 1]).unwrap();
    let b = Aabb::new([1, 1, 1], [2, 3, 4]).unwrap();
    assert!(a.overlaps(&b) && b.overlaps(&a));
    assert!(!a.overlaps(&Aabb::new([2, 0, 0], [3, 1, 1]).unwrap()));
    assert_eq!(a.area(), 12);
    assert_eq!(b.area(), 2 * 3 + 3 * 4 + 4 * 2);
    assert_eq!(a.union(&b), Aabb::new([0, 0, 0], [2, 3, 4]).unwrap());
    assert!(a.union(&b).contains_box(&b) && !a.contains_box(&b));
}

#[test]
fn errors_read_as_they_say() {
    let mut texts = Vec::new();
    for e in [
        BvhError::Stopped(Stop::Cancelled),
        BvhError::DuplicateKey(4),
        BvhError::UnknownKey(5),
        BvhError::TooManyEntries,
        BvhError::Dimension,
        BvhError::Empty,
        BvhError::OutOfRange,
        BvhError::BadBound,
        BvhError::OutOfMemory,
        BvhError::Defect,
    ] {
        let text = e.to_string();
        assert_ne!(text, "");
        texts.push(text);
    }
    assert!(texts[1].contains('4') && texts[2].contains('5'));
    let mut unique = texts.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), texts.len());
}
