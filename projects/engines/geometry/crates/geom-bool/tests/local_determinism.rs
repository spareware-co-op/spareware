//! The replay hash of the arrangement step of local operations (piece G144): `local_step` on both
//! of its paths over seeded targets and tools, every result body, its names, its history with the
//! topology changes it records, and every refusal written out and hashed with SHA-256. The digest
//! must be the same on every target and on every thread count (chapters 13.1, 13.14, 21 and 28.13;
//! the golden store of piece G19).
//!
//! The corpus, from a fixed `SplitMix64` stream, integers only, so the inputs are the same on
//! every target by construction: boxes of sides 4 to 40 steps near the origin as targets, and for
//! each of eleven kinds three seeded tools, each axis and side drawn:
//!
//! - **On both paths** (`Plan::Arrange`, then `Plan::Glued`): an extrusion on a whole face, the
//!   same box subtracted (a cut that only touches), a box apart, a box level with a face and apart,
//!   a boss on part of a face (the fast path refuses it), and extrusions on both ends of an axis.
//! - **Arranged:** a groove across a face, a slab through the middle (the body splits), a cap over
//!   one end (the end face is an overshoot), a box over the other end intersected (the end face
//!   vanishes), and a corner cut off by a half-space.
//! - **Fixed:** a square pyramid's apex cut off by a box, off centre, and by a half-space (the apex
//!   splits), and a block with a hole through it plugged blind and through (holes close).
//!
//! One case in three narrows the step's allowed changes by one class drawn from 13.14's list, so
//! some steps are refused as `TopologyChangeNotAllowed`.
//!
//! Written per step: the path it took, then its canonical output (`geom_test::canonical`): the
//! result body by the SHA-256 of its native format (every array, its geometry and its names), the
//! names the step made, its history with every topology change, its exactness and its diagnostics;
//! or the refusal's kind, entities and evidence, and its debug text. A target or a pool that
//! arranges, orders, names or classifies differently, or refuses differently, changes the digest.
//! The thread-count test runs the same corpus through `geom_run::Exec::with_pool` on pools of 1, 2,
//! 3 and 8 threads, where the arrangement runs in chunks, and each must give the serial digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "the corpus generator, and threads for chapter 21's thread counts; the rules are for \
              kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;

use geom_bool::combine::{combine, intersect};
use geom_bool::half_space::half_space;
use geom_bool::local::{Local, LocalInput, Path, Plan, local_step};
use geom_bool::operand::{BooleanInput, Operand, OperandKey, OperandKind, Selection};
use geom_make::{BoxInput, make_box};
use geom_num::{Exactness, P3, Steps};
use geom_op::history::ChangeClass;
use geom_op::local::{AllowSet, CLASSES};
use geom_op::{NameSet, OpContext, OpError, OpId, OpOutput, OpPath};
use geom_poly::merge::Merge;
#[cfg(not(target_family = "wasm"))]
use geom_run::Pool;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::canonical::{Outcome, canonical_bytes};
use geom_test::fixture::kernel::id_of;
use geom_test::golden::{self, hex, sha256};
use geom_test::{Seed, SplitMix64};
use geom_topo::Body;

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-bool/local";

/// The steps that give a body, the steps refused, and those of each path.
const HELD: u32 = 50;
const REFUSED: u32 = 6;
const FAST: u32 = 15;

/// The seeded cases of each kind.
const EACH: usize = 3;

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

/// A whole number in `low..=high`.
fn between(rng: &mut SplitMix64, low: i64, high: i64) -> i64 {
    low + below(rng, (high - low + 1) as u64) as i64
}

/// The path of operation `n`.
fn op(n: u32) -> OpPath {
    let mut bytes = [0x44; 16];
    bytes[..4].copy_from_slice(&n.to_le_bytes());
    OpPath::new(OpId::from_bytes(bytes))
}

/// Operation `n`'s context, its budget far above what the corpus needs: a step that stops
/// advancing fails, not hangs.
fn ctx(n: u32) -> OpContext<'static> {
    OpContext::new(op(n), Budget::of_work(1 << 31))
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// The target's key.
const TARGET: OperandKey = OperandKey([1; 16]);

fn solid(key: OperandKey, body: &Body) -> Operand<'_> {
    Operand {
        key,
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

fn subtract() -> Selection {
    Selection::Subtract { target: TARGET }
}

/// The bodies' maker: each body by its own operation, so no two bring the same names.
struct Maker {
    next: u32,
}

impl Maker {
    fn path(&mut self) -> OpPath {
        self.next += 1;
        op(self.next)
    }

    /// The box from `low`, of sides `size`.
    fn cuboid(&mut self, low: [i64; 3], size: [i64; 3]) -> Body {
        let input = BoxInput {
            min: p(low[0], low[1], low[2]),
            size: size.map(|s| Steps(s as u64)),
        };
        let path = self.path();
        make_box(&OpContext::new(path, Budget::UNLIMITED), &input)
            .unwrap()
            .result
            .body
    }

    /// The half-space behind the plane through `points`, counter-clockwise from in front.
    fn half(&mut self, points: [P3; 3]) -> Body {
        let path = self.path();
        half_space(&path, points, &mut meter()).unwrap()
    }
}

/// One step's inputs.
struct Case {
    target: Body,
    tools: Vec<Body>,
    select: Selection,
    may: AllowSet,
    /// Whether the fast path runs on it too.
    both: bool,
}

/// A seeded target box: its low corner and its sides.
fn target(rng: &mut SplitMix64) -> ([i64; 3], [i64; 3]) {
    (
        [0, 1, 2].map(|_| between(rng, -50, 50)),
        [0, 1, 2].map(|_| between(rng, 4, 40)),
    )
}

/// The three axes, `a` drawn and the other two after it.
fn axes(rng: &mut SplitMix64) -> [usize; 3] {
    let a = below(rng, 3) as usize;
    [a, (a + 1) % 3, (a + 2) % 3]
}

/// The seeded cases of kind `kind` (the module documentation's order).
fn seeded(rng: &mut SplitMix64, make: &mut Maker, kind: u32) -> Case {
    let (low, size) = target(rng);
    let [a, b, c] = axes(rng);
    let high = |i: usize| low[i] + size[i];
    // A tool's extent on an axis it crosses whole, one step past both ends.
    let (mut lo, mut sz) = (low, size);
    let across = |lo: &mut [i64; 3], sz: &mut [i64; 3], i: usize| {
        lo[i] = low[i] - 1;
        sz[i] = size[i] + 2;
    };
    let mut both = true;
    let mut tools = Vec::new();
    let select = match kind {
        // An extrusion on a whole face, on either side; or the same box subtracted.
        0 | 1 => {
            let d = between(rng, 1, 20);
            lo[a] = if rng.coin() { high(a) } else { low[a] - d };
            sz[a] = d;
            tools.push(make.cuboid(lo, sz));
            if kind == 0 {
                Selection::Unite
            } else {
                subtract()
            }
        }
        // A box apart.
        2 => {
            lo[a] = high(a) + between(rng, 1, 10);
            for i in [a, b, c] {
                sz[i] = between(rng, 1, 20);
            }
            for i in [b, c] {
                lo[i] = low[i] + between(rng, -5, 5);
            }
            tools.push(make.cuboid(lo, sz));
            Selection::Unite
        }
        // A box whose face lies on the target's high face on `a`, facing the same way, apart on
        // `b`.
        3 => {
            sz[a] = between(rng, 1, size[a]);
            lo[a] = high(a) - sz[a];
            lo[b] = high(b) + between(rng, 1, 10);
            sz[b] = between(rng, 1, 20);
            tools.push(make.cuboid(lo, sz));
            Selection::Unite
        }
        // A boss on part of the high face on `a`.
        4 => {
            for i in [b, c] {
                lo[i] = low[i] + 1 + below(rng, (size[i] - 2) as u64) as i64;
                sz[i] = between(rng, 1, high(i) - 1 - lo[i]);
            }
            lo[a] = high(a);
            sz[a] = between(rng, 1, 10);
            tools.push(make.cuboid(lo, sz));
            Selection::Unite
        }
        // Extrusions on both ends of `a`.
        5 => {
            let (d, e) = (between(rng, 1, 20), between(rng, 1, 20));
            lo[a] = high(a);
            sz[a] = d;
            tools.push(make.cuboid(lo, sz));
            lo[a] = low[a] - e;
            sz[a] = e;
            tools.push(make.cuboid(lo, sz));
            Selection::Unite
        }
        // A groove across the high face on `a`, whole along `b`, inside on `c`.
        6 => {
            both = false;
            let depth = between(rng, 1, size[a] - 1);
            lo[a] = high(a) - depth;
            sz[a] = depth + 5;
            across(&mut lo, &mut sz, b);
            lo[c] = low[c] + between(rng, 1, size[c] - 2);
            sz[c] = between(rng, 1, high(c) - 1 - lo[c]);
            tools.push(make.cuboid(lo, sz));
            subtract()
        }
        // A slab through the middle on `a`, whole on `b` and `c`.
        7 => {
            both = false;
            let k = between(rng, 1, size[a] - 2);
            lo[a] = low[a] + k;
            sz[a] = between(rng, 1, size[a] - 1 - k);
            across(&mut lo, &mut sz, b);
            across(&mut lo, &mut sz, c);
            tools.push(make.cuboid(lo, sz));
            subtract()
        }
        // A cap over the high end on `a`, united.
        8 => {
            both = false;
            let depth = between(rng, 1, size[a] - 1);
            lo[a] = high(a) - depth;
            sz[a] = depth + between(rng, 1, 5);
            across(&mut lo, &mut sz, b);
            across(&mut lo, &mut sz, c);
            tools.push(make.cuboid(lo, sz));
            Selection::Unite
        }
        // A box over the low end on `a`, intersected.
        9 => {
            both = false;
            lo[a] = low[a] - 1;
            sz[a] = 1 + between(rng, 1, size[a] - 1);
            across(&mut lo, &mut sz, b);
            across(&mut lo, &mut sz, c);
            tools.push(make.cuboid(lo, sz));
            Selection::Intersect
        }
        // The high corner cut off by a half-space, the low side kept.
        _ => {
            both = false;
            let cut = between(rng, 1, size.into_iter().min().unwrap() - 1);
            let (x, y, z) = (high(0), high(1), high(2));
            tools.push(make.half([p(x - cut, y, z), p(x, y - cut, z), p(x, y, z - cut)]));
            Selection::Intersect
        }
    };
    let target = make.cuboid(low, size);
    Case {
        target,
        tools,
        select,
        may: may(rng),
        both,
    }
}

/// Every change, or one in three times every change but one class drawn.
fn may(rng: &mut SplitMix64) -> AllowSet {
    if below(rng, 3) == 0 {
        AllowSet::ALL.without(CLASSES[below(rng, CLASSES.len() as u64) as usize])
    } else {
        AllowSet::ALL
    }
}

/// A square pyramid on [0, 20]² with its apex at (10, 10, 20): a box cut by four half-spaces.
fn pyramid(make: &mut Maker, exec: &Exec<'_>) -> Body {
    let apex = p(10, 10, 20);
    let sides = [
        [p(0, 0, 0), p(20, 0, 0), apex],
        [p(20, 0, 0), p(20, 20, 0), apex],
        [p(20, 20, 0), p(0, 20, 0), apex],
        [p(0, 20, 0), p(0, 0, 0), apex],
    ];
    let mut body = make.cuboid([0, 0, 0], [20, 20, 20]);
    for points in sides {
        let half = make.half(points);
        let path = make.path();
        let both = [solid(TARGET, &body), solid(OperandKey([2; 16]), &half)];
        body = intersect(&OpContext::new(path, Budget::UNLIMITED), exec, &both)
            .unwrap()
            .result
            .body;
    }
    body
}

/// The block of 10 by 20 by 30 with a 4 by 4 hole through it.
fn holed(make: &mut Maker, exec: &Exec<'_>) -> Body {
    let block = make.cuboid([0, 0, 0], [10, 20, 30]);
    let pillar = make.cuboid([3, 3, -1], [4, 4, 32]);
    let input = BooleanInput {
        operands: vec![solid(TARGET, &block), solid(OperandKey([2; 16]), &pillar)],
        select: vec![subtract()],
        merge: Merge::RemoveRedundant,
        accept: None,
    };
    let path = make.path();
    combine(&OpContext::new(path, Budget::UNLIMITED), exec, &input)
        .unwrap()
        .result
        .remove(0)
        .remove(0)
        .body
}

/// The fixed cases (the module documentation's list).
fn fixed(make: &mut Maker, exec: &Exec<'_>) -> Vec<Case> {
    let case = |target: &Body, tool: Body, select: Selection| Case {
        target: target.clone(),
        tools: vec![tool],
        select,
        may: AllowSet::ALL,
        both: false,
    };
    let top = pyramid(make, exec);
    let block = holed(make, exec);
    let tip = make.cuboid([8, 8, 18], [4, 4, 7]);
    let off = make.cuboid([6, 9, 16], [8, 5, 9]);
    let below = make.half([p(0, 0, 18), p(1, 0, 18), p(0, 1, 18)]);
    let blind = make.cuboid([3, 3, 20], [4, 4, 10]);
    let through = make.cuboid([3, 3, 0], [4, 4, 30]);
    vec![
        case(&top, tip, subtract()),
        case(&top, off, subtract()),
        case(&top, below, Selection::Intersect),
        case(&block, blind, Selection::Unite),
        case(&block, through, Selection::Unite),
    ]
}

/// Every case, in order.
fn cases(exec: &Exec<'_>) -> Vec<Case> {
    let mut rng = SplitMix64::new(Seed(0x0144));
    let mut make = Maker { next: 1000 };
    let mut out = Vec::new();
    for kind in 0..11 {
        for _ in 0..EACH {
            out.push(seeded(&mut rng, &mut make, kind));
        }
    }
    out.extend(fixed(&mut make, exec));
    out
}

/// Case `case` on `plan`, as operation `n`.
fn step(exec: &Exec<'_>, n: u32, case: &Case, plan: Plan) -> Result<OpOutput<Local>, OpError> {
    let input = LocalInput {
        target: solid(TARGET, &case.target),
        tools: case
            .tools
            .iter()
            .enumerate()
            .map(|(i, b)| solid(OperandKey([i as u8 + 2; 16]), b))
            .collect(),
        select: case.select.clone(),
        merge: Merge::RemoveRedundant,
        accept: None,
        may: case.may,
        plan,
    };
    local_step(&ctx(n), exec, &input)
}

fn text(out: &mut Vec<u8>, t: &[u8]) {
    out.extend((t.len() as u64).to_le_bytes());
    out.extend(t);
}

/// What the corpus gave.
struct Record {
    bytes: Vec<u8>,
    held: u32,
    refused: u32,
    fast: u32,
    /// Every class of change some step recorded.
    found: Vec<ChangeClass>,
}

impl Record {
    /// Step `n`'s outcome: its path and its canonical output, or its refusal.
    fn write(&mut self, n: u32, made: &Result<OpOutput<Local>, OpError>) {
        match made {
            Ok(out) => {
                let local = &out.result;
                self.held += 1;
                self.bytes.push(0);
                self.bytes.push(u8::from(local.path == Path::Fast));
                if local.path == Path::Fast {
                    self.fast += 1;
                }
                for change in local.history.changes() {
                    let class = change.class();
                    if !self.found.contains(&class) {
                        self.found.push(class);
                    }
                }
                let digest = match id_of(&local.body) {
                    Ok(id) => <[u8; 32]>::try_from(id.digest()).unwrap(),
                    Err(why) => panic!("step {n}: a body the native format refuses: {why}"),
                };
                let names =
                    NameSet::new(local.names.iter().map(|name| name.id().unwrap())).unwrap();
                let path = op(n);
                let output = Outcome::Output {
                    bodies: &[digest],
                    names: &names,
                    history: Some((&path, &local.history)),
                    exactness: &out.exactness,
                    diagnostics: &out.diagnostics,
                };
                text(&mut self.bytes, &canonical_bytes(&output).unwrap());
            }
            Err(e) => {
                self.refused += 1;
                self.bytes.push(1);
                text(
                    &mut self.bytes,
                    &canonical_bytes(&Outcome::Error(e)).unwrap(),
                );
                text(&mut self.bytes, format!("{e:?}").as_bytes());
            }
        }
    }
}

/// The corpus's record on `exec`.
fn corpus(exec: &Exec<'_>) -> Record {
    let mut rec = Record {
        bytes: Vec::new(),
        held: 0,
        refused: 0,
        fast: 0,
        found: Vec::new(),
    };
    let mut n = 0;
    for case in cases(exec) {
        n += 1;
        let full = step(exec, n, &case, Plan::Arrange);
        rec.write(n, &full);
        if case.both {
            let fast = step(exec, n, &case, Plan::Glued);
            rec.write(n, &fast);
            // One body and one history either way, where both paths give one.
            if let (Ok(full), Ok(fast)) = (&full, &fast) {
                assert_eq!(full.result.body, fast.result.body, "case {n}");
                assert_eq!(full.result.history, fast.result.history, "case {n}");
            }
        }
    }
    rec
}

#[test]
fn local_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus(&Exec::serial());
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-bool local corpus: {} held ({} on the fast path), {} refused, {} bytes, golden hash \
         {}",
        rec.held,
        rec.fast,
        rec.refused,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    let mut found = rec.found.clone();
    found.sort_unstable();
    let mut every: Vec<ChangeClass> = CLASSES
        .into_iter()
        .filter(|c| *c != ChangeClass::SelfOverlapCut)
        .collect();
    every.sort_unstable();
    assert_eq!(
        found, every,
        "every class but a self-overlap cut, which a planar face cannot make"
    );
    assert_eq!(
        (rec.held, rec.fast, rec.refused),
        (HELD, FAST, REFUSED),
        "every step ran"
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target arranges, orders, names or classifies \
             differently, or refuses differently (compare this log's digest with another \
             target's), or a definition changed in place, which chapter 26.2 forbids"
        )
    });
}

/// A pool of `n` threads: each takes one run of the tasks, in order. Each call starts its own
/// threads, so a task that calls the helper again makes progress.
#[cfg(not(target_family = "wasm"))]
struct Threads(usize);

#[cfg(not(target_family = "wasm"))]
impl Pool for Threads {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        let run = tasks.len().div_ceil(self.0.max(1)).max(1);
        std::thread::scope(|scope| {
            for lane in tasks.chunks_mut(run) {
                scope.spawn(move || {
                    for task in lane {
                        task();
                    }
                });
            }
        });
    }
}

/// Chapter 21's thread-count test: the same digest on pools of 1, 2, 3 and 8 threads as on one
/// thread. WebAssembly has no threads.
#[cfg(not(target_family = "wasm"))]
#[test]
fn local_golden_hash_is_the_same_on_every_thread_count() {
    let one = hex(sha256(&corpus(&Exec::serial()).bytes));
    for threads in golden::THREAD_COUNTS {
        let got = hex(sha256(&corpus(&Exec::with_pool(&Threads(threads))).bytes));
        assert_eq!(got, one, "{threads} threads");
    }
}
