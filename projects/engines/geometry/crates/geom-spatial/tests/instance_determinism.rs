//! The replay hash of piece G295: instance trees built over a fixed corpus of scenes, their nodes
//! and leaves and every answer hashed with SHA-256, must give the same digest on every target
//! (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus: from a fixed linear congruential sequence, eight scenes of parts definitions in
//! coarse and fine frames, nested assemblies two and three deep, and root assemblies of 1 to 4,000
//! records placed by the 48 signed permutations, one of them built on a pool of three threads; a
//! scene of rational and certified general placements around a nested assembly; each
//! assembly's tree (every node's box and kind, every leaf), and the hits of twelve regions per
//! scene (each hit's path, part, definition and exactness) with the work each query charged. Any
//! target that splits a node differently, rounds an enclosure differently or orders a hit
//! differently changes the digest. The digest lives in the store; a change is a bug, or a new
//! definition with a golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::range_plus_one,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    reason = "the corpus generator and a test pool of scoped threads; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_bvh::{Aabb, BoxTree, Entry, NodeKind};
use geom_math::{Axis, Box3, CrsId, FrameRecord, GridTransform, Rotation, TypedTransform};
use geom_num::grid::{GridStep, P3};
use geom_run::{Budget, Exec, Pool, WorkMeter};
use geom_spatial::instance::{
    Assembly, Content, DefinitionKey, Definitions, Hit, InstanceError, Place, Record,
};
use geom_test::golden::{self, hex, sha256};
use geom_topo::placed::{Definition, Placed, Placement};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-spatial/instance";

#[derive(Clone, Copy, Debug)]
struct Inst(Placed);

impl Record for Inst {
    fn definition(&self) -> u32 {
        self.0.definition().0
    }
    fn place(&self) -> Result<Place, InstanceError> {
        match self.0.placement() {
            Placement::General(i) => Ok(Place::General(i)),
            exact @ Placement::Exact { .. } => exact
                .grid()
                .map(Place::Exact)
                .map_err(|_| InstanceError::Placement),
        }
    }
    fn bound(&self) -> &Box3 {
        self.0.bound()
    }
}

/// Three scoped threads, task i on thread i mod 3, each running its tasks last first.
struct Three;

impl Pool for Three {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        let mut groups: Vec<Vec<&mut (dyn FnMut() + Send)>> = (0..3).map(|_| Vec::new()).collect();
        for (i, t) in tasks.iter_mut().enumerate() {
            groups[i % 3].push(&mut **t);
        }
        let run = |group: Vec<&mut (dyn FnMut() + Send)>| {
            for t in group.into_iter().rev() {
                t();
            }
        };
        std::thread::scope(|s| {
            // wasm32-wasip1 has no threads: where none can start, every group runs here, in turn.
            if std::thread::Builder::new().spawn_scoped(s, || {}).is_err() {
                groups.into_iter().for_each(run);
                return;
            }
            for group in groups {
                s.spawn(move || run(group));
            }
        });
    }
}

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
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn key(n: u32) -> DefinitionKey {
    let mut content = [0u8; 32];
    content[..4].copy_from_slice(&n.to_le_bytes());
    DefinitionKey {
        content,
        edition: 1,
    }
}

const ARRANGEMENTS: [[Axis; 3]; 6] = [
    [Axis::X, Axis::Y, Axis::Z],
    [Axis::X, Axis::Z, Axis::Y],
    [Axis::Y, Axis::X, Axis::Z],
    [Axis::Y, Axis::Z, Axis::X],
    [Axis::Z, Axis::X, Axis::Y],
    [Axis::Z, Axis::Y, Axis::X],
];

fn grid(rng: &mut Lcg, span: i64) -> GridTransform {
    let p = rng.below(48);
    let shift = [rng.int(span), rng.int(span), rng.int(span)];
    GridTransform::from_parts(
        ARRANGEMENTS[p >> 3],
        [p & 1 != 0, p & 2 != 0, p & 4 != 0],
        shift,
    )
    .unwrap()
}

fn frames() -> [FrameRecord; 3] {
    [
        FrameRecord::map(CrsId::Local(1), GridStep::Q0, [0; 3]).unwrap(),
        FrameRecord::map(CrsId::Local(1), GridStep::Q1, [3, -7, 11]).unwrap(),
        FrameRecord::map(CrsId::Local(1), GridStep::Q0, [40, 0, -9]).unwrap(),
    ]
}

fn parts(rng: &mut Lcg, n: usize) -> BoxTree<3> {
    let entries: Vec<Entry<3>> = (0..n)
        .map(|i| {
            let lo: [i64; 3] = std::array::from_fn(|_| rng.int(1 << 12));
            let hi: [i64; 3] = std::array::from_fn(|a| lo[a] + rng.int(1 << 10).abs());
            Entry {
                key: (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
                bbox: Aabb::new(lo, hi).unwrap(),
            }
        })
        .collect();
    BoxTree::build(&entries, &Exec::serial(), &mut meter()).unwrap()
}

fn exact(defs: &Definitions<Inst>, h: u32, frame: &FrameRecord, t: &GridTransform) -> Inst {
    let shared = defs.bound_in(h, frame).unwrap();
    Inst(Placed::exact(Definition(h), t, &shared).unwrap())
}

#[derive(Default)]
struct Recorder {
    bytes: Vec<u8>,
    scenes: u32,
}

impl Recorder {
    fn put_box(&mut self, lo: [i64; 3], hi: [i64; 3]) {
        for c in lo.iter().chain(&hi) {
            self.bytes.extend_from_slice(&c.to_le_bytes());
        }
    }
    fn tree(&mut self, a: &Assembly<Inst>) {
        let t = a.tree();
        self.bytes
            .extend_from_slice(&(t.nodes().len() as u64).to_le_bytes());
        for n in t.nodes() {
            self.put_box(n.bbox().lo(), n.bbox().hi());
            let (tag, x, y) = match n.kind() {
                NodeKind::Leaf { start, len } => (0u8, start, len),
                NodeKind::Inner { left, right } => (1u8, left, right),
            };
            self.bytes.push(tag);
            self.bytes.extend_from_slice(&x.to_le_bytes());
            self.bytes.extend_from_slice(&y.to_le_bytes());
        }
        self.bytes
            .extend_from_slice(&(t.leaves().len() as u64).to_le_bytes());
        for l in t.leaves() {
            self.bytes.extend_from_slice(&l.to_le_bytes());
        }
    }
    fn hits(&mut self, hits: &[Hit], m: &WorkMeter<'_>) {
        self.bytes
            .extend_from_slice(&(hits.len() as u64).to_le_bytes());
        for h in hits {
            self.bytes.push(h.path.records().len() as u8);
            for r in h.path.records() {
                self.bytes.extend_from_slice(&r.to_le_bytes());
            }
            self.bytes.extend_from_slice(&h.part.to_le_bytes());
            self.bytes.extend_from_slice(&h.definition.to_le_bytes());
            self.bytes.push(u8::from(h.exact));
        }
        self.bytes.extend_from_slice(&m.used().work.to_le_bytes());
    }
    fn queries(
        &mut self,
        rng: &mut Lcg,
        root: &Assembly<Inst>,
        defs: &Definitions<Inst>,
        side: &[TypedTransform],
    ) {
        let whole = root.bound().unwrap().unwrap();
        let (lo, hi) = (whole.min(), whole.max());
        for q in 0..12 {
            let region = if q == 0 {
                whole
            } else {
                let c =
                    |l: i64, h: i64, rng: &mut Lcg| l + (rng.next() % ((h - l + 1) as u64)) as i64;
                let x = c(lo.x.get(), hi.x.get(), rng);
                let y = c(lo.y.get(), hi.y.get(), rng);
                let z = c(lo.z.get(), hi.z.get(), rng);
                let s = 1i64 << (4 + q);
                Box3::new(
                    P3::new(x, y, z).unwrap(),
                    P3::new(x + s, y + s, z + s).unwrap(),
                )
                .unwrap()
            };
            let mut m = meter();
            let hits = root.query(defs, side, &region, &mut m).unwrap();
            self.hits(&hits, &m);
        }
    }
}

fn corpus() -> Recorder {
    let mut rng = Lcg(295);
    let mut rec = Recorder::default();
    let fs = frames();
    for (scene, count) in [1usize, 4, 5, 60, 300, 1000, 2500, 4000]
        .into_iter()
        .enumerate()
    {
        let mut defs = Definitions::new();
        let mut n = 0;
        for _ in 0..4 {
            n += 1;
            let count = 1 + rng.below(20);
            let tree = parts(&mut rng, count);
            let frame = fs[rng.below(3)];
            defs.get_or_insert_with(key(n), |_| Ok(Content::Parts { frame, tree }))
                .unwrap();
        }
        for _ in 0..1 + scene % 3 {
            n += 1;
            let frame = fs[rng.below(3)];
            let placeable = defs.len();
            let records: Vec<Inst> = (0..1 + rng.below(9))
                .map(|_| {
                    let h = rng.below(placeable) as u32;
                    exact(&defs, h, &frame, &grid(&mut rng, 1 << 16))
                })
                .collect();
            let a =
                Assembly::build(frame, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
            rec.tree(&a);
            defs.get_or_insert_with(key(n), |_| Ok(Content::Assembly(a)))
                .unwrap();
        }
        let records: Vec<Inst> = (0..count)
            .map(|_| {
                let h = rng.below(defs.len()) as u32;
                exact(&defs, h, &fs[0], &grid(&mut rng, 1 << 24))
            })
            .collect();
        let exec = if scene == 7 {
            Exec::with_pool(&Three)
        } else {
            Exec::serial()
        };
        let root = Assembly::build(fs[0], records, &defs, &[], &exec, &mut meter()).unwrap();
        rec.tree(&root);
        rec.queries(&mut rng, &root, &defs, &[]);
        rec.scenes += 1;
    }
    // General placements: a rational and a certified turn, alone and around a nested assembly.
    let f = fs[0];
    let mut defs = Definitions::new();
    let tree = parts(&mut rng, 30);
    defs.get_or_insert_with(key(1), |_| Ok(Content::Parts { frame: f, tree }))
        .unwrap();
    let wing: Vec<Inst> = (0..6)
        .map(|_| exact(&defs, 0, &f, &grid(&mut rng, 1 << 14)))
        .collect();
    let wing = Assembly::build(f, wing, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
    defs.get_or_insert_with(key(2), |_| Ok(Content::Assembly(wing)))
        .unwrap();
    let turn = |c: i32, s: i32, shift: [i64; 3]| {
        TypedTransform::from(GridTransform::translation(shift).unwrap())
            .compose(
                &TypedTransform::rotation(Axis::Z, Rotation::direction(c, s).unwrap()).unwrap(),
            )
            .unwrap()
    };
    let side = vec![turn(3, 4, [1 << 18, 0, 0]), turn(2, 5, [0, 1 << 18, 0])];
    let records: Vec<Inst> = [(0, 0), (0, 1), (1, 0), (1, 1)]
        .into_iter()
        .map(|(h, i)| {
            let shared = defs.bound_in(h, &f).unwrap();
            Inst(Placed::general(Definition(h), i, &side[i as usize], &shared).unwrap())
        })
        .collect();
    let root = Assembly::build(f, records, &defs, &side, &Exec::serial(), &mut meter()).unwrap();
    rec.tree(&root);
    rec.queries(&mut rng, &root, &defs, &side);
    rec.scenes += 1;
    rec
}

#[test]
fn instance_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-spatial instance corpus: {} scenes, {} bytes, golden hash {}",
        rec.scenes,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert_eq!(rec.scenes, 9);
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target splits a node, rounds an enclosure or \
             orders a hit differently (compare this log's digest with another target's), or a \
             definition changed in place, which chapter 26.2 forbids"
        )
    });
}
