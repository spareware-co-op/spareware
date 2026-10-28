//! The replay hash of piece G54: box trees built over a fixed corpus, their bytes and their
//! answers hashed with SHA-256, must give the same digest on every target (chapters 21 and 28.13;
//! the golden store of piece G19).
//!
//! The corpus: from a fixed linear congruential sequence, 2D and 3D sets of 1 to 9,000 boxes
//! (sparse, dense, nested and all equal, the largest past the size at which subtrees are built in
//! parallel), each tree's canonical bytes, the keys meeting eight query boxes, its pairs and its
//! pairs with the next set, and one refit of each. Any target that splits one node differently,
//! orders one tie differently or answers one query differently changes the digest. The digest
//! lives in the store; a change is a bug, or a new definition with a golden of its own (chapter
//! 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_bvh::{Aabb, BoxTree, Entry, Refitted};
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-bvh/tree";

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

fn set<const D: usize>(rng: &mut Lcg, n: usize, span: i64, size: i64) -> Vec<Entry<D>> {
    (0..n)
        .map(|i| {
            let lo: [i64; D] = std::array::from_fn(|_| rng.int(span));
            let hi: [i64; D] = std::array::from_fn(|a| lo[a] + rng.int(size).abs());
            Entry {
                key: (i as u64)
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    .rotate_left(29),
                bbox: Aabb::new(lo, hi).unwrap(),
            }
        })
        .collect()
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

#[derive(Default)]
struct Recorder {
    bytes: Vec<u8>,
    trees: u32,
}

impl Recorder {
    fn keys(&mut self, keys: &[u64]) {
        self.bytes
            .extend_from_slice(&(keys.len() as u64).to_le_bytes());
        for k in keys {
            self.bytes.extend_from_slice(&k.to_le_bytes());
        }
    }
    fn pairs(&mut self, pairs: &[(u64, u64)]) {
        self.bytes
            .extend_from_slice(&(pairs.len() as u64).to_le_bytes());
        for (a, b) in pairs {
            self.bytes.extend_from_slice(&a.to_le_bytes());
            self.bytes.extend_from_slice(&b.to_le_bytes());
        }
    }
    fn run<const D: usize>(&mut self, rng: &mut Lcg, a: &[Entry<D>], b: &[Entry<D>]) {
        let mut t = BoxTree::build(a, &Exec::serial(), &mut meter()).unwrap();
        let u = BoxTree::build(b, &Exec::serial(), &mut meter()).unwrap();
        self.bytes.extend_from_slice(&t.to_bytes().unwrap());
        for _ in 0..8 {
            let q = set::<D>(rng, 1, 1 << 30, 1 << 28)[0].bbox;
            self.keys(&t.overlapping(&q, &mut meter()).unwrap());
        }
        if a.len() <= 3000 {
            self.pairs(&t.pairs(&mut meter()).unwrap());
        }
        self.pairs(&t.pairs_with(&u, &mut meter()).unwrap());
        let moved: Vec<Entry<D>> = a
            .iter()
            .step_by(3)
            .map(|e| Entry {
                key: e.key,
                bbox: b[e.key as usize % b.len()].bbox,
            })
            .collect();
        let how = t.refit(&moved, &Exec::serial(), &mut meter()).unwrap();
        self.bytes.push(u8::from(how == Refitted::Rebuilt));
        self.bytes.extend_from_slice(&t.to_bytes().unwrap());
        self.trees += 2;
    }
}

fn corpus() -> Recorder {
    let mut rng = Lcg(54);
    let mut rec = Recorder::default();
    for (n, span, size) in [
        (1, 100, 5),
        (7, 100, 5),
        (100, 1 << 20, 1 << 14),
        (1000, 1 << 10, 1 << 8),
        (9000, 1 << 40, 1 << 30),
    ] {
        let a = set::<2>(&mut rng, n, span, size);
        let b = set::<2>(&mut rng, n / 3 + 1, span, size);
        rec.run(&mut rng, &a, &b);
        let a = set::<3>(&mut rng, n, span, size);
        let b = set::<3>(&mut rng, n / 3 + 1, span, size);
        rec.run(&mut rng, &a, &b);
    }
    let nested: Vec<Entry<3>> = (0..40u64)
        .map(|i| Entry {
            key: i,
            bbox: Aabb::new([0; 3], [1 << i; 3]).unwrap(),
        })
        .collect();
    let equal: Vec<Entry<3>> = (0..300u64)
        .map(|i| Entry {
            key: 1000 - i,
            bbox: Aabb::new([3; 3], [4; 3]).unwrap(),
        })
        .collect();
    rec.run(&mut rng, &nested, &equal);
    rec
}

#[test]
fn tree_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-bvh tree corpus: {} trees, {} bytes, golden hash {}",
        rec.trees,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert!(rec.trees >= 22, "only {} trees", rec.trees);
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target splits a node, orders a tie or answers a \
             query differently (compare this log's digest with another target's), or a definition \
             changed in place, which chapter 26.2 forbids"
        )
    });
}
