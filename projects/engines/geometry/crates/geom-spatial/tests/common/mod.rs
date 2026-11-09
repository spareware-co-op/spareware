//! What the instance tree's test files share: chapter 19.12's record wrapped as the facade wraps
//! it, a test pool, generators, and the brute-force oracle that maps every part of every instance
//! up to the queried frame in exact rational arithmetic.

#![allow(
    dead_code,
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
    reason = "test helpers; each test file uses some of them, and the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;
use std::num::NonZeroU64;

use geom_bvh::{Aabb, BoxTree, Entry};
use geom_math::{Axis, Box3, CrsId, FrameRecord, GridTransform};
use geom_num::grid::{GridStep, P3};
use geom_run::{Budget, Exec, Pool, WorkMeter};
use geom_spatial::instance::{
    Assembly, Content, DefinitionKey, Definitions, Hit, InstanceError, Place, Record,
};
use geom_test::SplitMix64;
use geom_topo::placed::{Definition, Placed, Placement};

/// Chapter 19.12's record, wrapped so this crate's trait reads it, as the facade's wrapper does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inst(pub Placed);

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

/// A test pool: `threads` scoped threads, task i on thread i mod threads, each running its tasks
/// last first.
pub struct Threads(pub usize);

impl Pool for Threads {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        let mut groups: Vec<Vec<&mut (dyn FnMut() + Send)>> =
            (0..self.0).map(|_| Vec::new()).collect();
        for (i, t) in tasks.iter_mut().enumerate() {
            groups[i % self.0].push(&mut **t);
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

pub fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

pub fn int(rng: &mut SplitMix64, k: i64) -> i64 {
    rng.below(NonZeroU64::new(2 * k as u64 + 1).unwrap()) as i64 - k
}

pub fn pick(rng: &mut SplitMix64, n: usize) -> usize {
    rng.below(NonZeroU64::new(n as u64).unwrap()) as usize
}

pub fn p3(v: [i64; 3]) -> P3 {
    P3::new(v[0], v[1], v[2]).unwrap()
}

pub fn bx(lo: [i64; 3], hi: [i64; 3]) -> Box3 {
    Box3::new(p3(lo), p3(hi)).unwrap()
}

pub fn coords(p: P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

pub fn frame(step: GridStep, origin: [i64; 3]) -> FrameRecord {
    FrameRecord::map(CrsId::Local(1), step, origin).unwrap()
}

pub fn key(n: u32, edition: u32) -> DefinitionKey {
    let mut content = [0u8; 32];
    content[..4].copy_from_slice(&n.to_le_bytes());
    DefinitionKey { content, edition }
}

pub const ARRANGEMENTS: [[Axis; 3]; 6] = [
    [Axis::X, Axis::Y, Axis::Z],
    [Axis::X, Axis::Z, Axis::Y],
    [Axis::Y, Axis::X, Axis::Z],
    [Axis::Y, Axis::Z, Axis::X],
    [Axis::Z, Axis::X, Axis::Y],
    [Axis::Z, Axis::Y, Axis::X],
];

/// Signed permutation `p` of the 48, shifted by `shift`.
pub fn grid(p: usize, shift: [i64; 3]) -> GridTransform {
    let signs = [p & 1 != 0, p & 2 != 0, p & 4 != 0];
    GridTransform::from_parts(ARRANGEMENTS[(p >> 3) % 6], signs, shift).unwrap()
}

/// A parts tree of `n` boxes within `span` of `centre`, each up to `size` across, keyed from
/// `first` in steps of 7 (so keys are not positions).
pub fn parts(
    rng: &mut SplitMix64,
    n: usize,
    centre: [i64; 3],
    span: i64,
    size: i64,
    first: u64,
) -> BoxTree<3> {
    let entries: Vec<Entry<3>> = (0..n)
        .map(|i| {
            let lo: [i64; 3] = std::array::from_fn(|a| centre[a] + int(rng, span));
            let hi: [i64; 3] = std::array::from_fn(|a| lo[a] + int(rng, size).abs());
            Entry {
                key: first + 7 * i as u64,
                bbox: Aabb::new(lo, hi).unwrap(),
            }
        })
        .collect();
    BoxTree::build(&entries, &Exec::serial(), &mut meter()).unwrap()
}

pub fn add_parts(
    defs: &mut Definitions<Inst>,
    k: DefinitionKey,
    f: FrameRecord,
    tree: BoxTree<3>,
) -> u32 {
    defs.get_or_insert_with(k, |_| Ok(Content::Parts { frame: f, tree }))
        .unwrap()
}

/// An exact record of definition `h` in an assembly of frame `asm`.
pub fn exact(defs: &Definitions<Inst>, h: u32, asm: &FrameRecord, t: &GridTransform) -> Inst {
    let shared = defs.bound_in(h, asm).unwrap();
    Inst(Placed::exact(Definition(h), t, &shared).unwrap())
}

/// One step of a part's way up to the queried frame, for the oracle.
#[derive(Clone, Copy)]
pub enum Step {
    /// The definition's frame is finer: assembly = (definition − offset) / refine.
    Finer(i128, [i128; 3]),
    /// The definition's frame is coarser: assembly = refine·definition + offset.
    Coarser(i128, [i128; 3]),
    /// A placement.
    Grid(GridTransform),
}

/// The oracle's own frame step, from `geom-math`'s shifts directly.
pub fn frame_step(asm: &FrameRecord, def: &FrameRecord) -> Step {
    if let Ok(s) = asm.shift_to(def) {
        Step::Finer(i128::from(s.refine()), s.offset().map(i128::from))
    } else {
        let s = def.shift_to(asm).unwrap();
        Step::Coarser(i128::from(s.refine()), s.offset().map(i128::from))
    }
}

/// A rational box: numerators and their one denominator.
#[derive(Clone, Copy, Debug)]
pub struct Rat {
    pub lo: [i128; 3],
    pub hi: [i128; 3],
    pub den: i128,
}

pub fn up(b: Rat, step: Step) -> Rat {
    match step {
        Step::Finer(r, o) => Rat {
            lo: std::array::from_fn(|a| b.lo[a] - o[a] * b.den),
            hi: std::array::from_fn(|a| b.hi[a] - o[a] * b.den),
            den: b.den * r,
        },
        Step::Coarser(r, o) => Rat {
            lo: std::array::from_fn(|a| r * b.lo[a] + o[a] * b.den),
            hi: std::array::from_fn(|a| r * b.hi[a] + o[a] * b.den),
            den: b.den,
        },
        Step::Grid(t) => {
            let src = t.source();
            let neg = t.negate();
            let shift = t.translation_part();
            let mut lo = [0; 3];
            let mut hi = [0; 3];
            for i in 0..3 {
                let s = src[i].index() as usize;
                let (l, h) = if neg[i] {
                    (-b.hi[s], -b.lo[s])
                } else {
                    (b.lo[s], b.hi[s])
                };
                lo[i] = l + i128::from(shift[i]) * b.den;
                hi[i] = h + i128::from(shift[i]) * b.den;
            }
            Rat { lo, hi, den: b.den }
        }
    }
}

pub fn meets(b: &Rat, r: &Box3) -> bool {
    let (rl, rh) = (coords(r.min()), coords(r.max()));
    (0..3).all(|a| b.lo[a] <= i128::from(rh[a]) * b.den && i128::from(rl[a]) * b.den <= b.hi[a])
}

/// Every (path, part) brute force finds: each part of each instance, through every level, mapped
/// up to `root`'s frame exactly, tested against `region`.
pub fn oracle(
    root: &Assembly<Inst>,
    defs: &Definitions<Inst>,
    region: &Box3,
) -> BTreeSet<(Vec<u32>, u64)> {
    let mut out = BTreeSet::new();
    let mut stack: Vec<(&Assembly<Inst>, Vec<u32>, Vec<Step>)> =
        vec![(root, Vec::new(), Vec::new())];
    while let Some((a, path, chain)) = stack.pop() {
        for (i, rec) in a.records().iter().enumerate() {
            let d = defs.get(rec.definition()).unwrap();
            let t = rec.0.placement().grid().unwrap();
            let mut steps = vec![frame_step(a.frame(), d.frame()), Step::Grid(t)];
            steps.extend_from_slice(&chain);
            let mut p = path.clone();
            p.push(i as u32);
            match d.content() {
                Content::Parts { tree, .. } => {
                    for e in tree.entries() {
                        let mut b = Rat {
                            lo: e.bbox.lo().map(i128::from),
                            hi: e.bbox.hi().map(i128::from),
                            den: 1,
                        };
                        for s in &steps {
                            b = up(b, *s);
                        }
                        if meets(&b, region) {
                            out.insert((p.clone(), e.key));
                        }
                    }
                }
                Content::Assembly(inner) => stack.push((inner, p, steps)),
            }
        }
    }
    out
}

pub fn found(hits: &[Hit]) -> BTreeSet<(Vec<u32>, u64)> {
    hits.iter()
        .map(|h| (h.path.records().to_vec(), h.part))
        .collect()
}

pub fn sorted(hits: &[Hit]) -> bool {
    hits.windows(2).all(|w| w[0] < w[1])
}
