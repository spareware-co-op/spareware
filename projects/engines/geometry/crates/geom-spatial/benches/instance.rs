//! The benchmark of piece G295 (chapter 30): 1,000,000 instances placed and indexed in 1 s, at no
//! more than 160 bytes each with their record (the board's "done when").
//!
//! Run with `cargo bench -p geom-spatial`, on one core for the board's figure
//! (`taskset -c 2 cargo bench -p geom-spatial`). It uses the standard library's clock only
//! (wall-clock time on this machine; instruction counts are geom-bench's, piece G18). The scene
//! comes from a fixed linear congruential sequence, so every run measures the same work: a tower
//! of 100 storeys of 100 m by 100 m at q0, 10,000 instances a storey of 200 definitions (doors,
//! fittings and fixtures of 12 parts each, 20 of them parts in fine q1 frames), each placed by one
//! of the 48 signed permutations at a whole-step shift. It times making the 1,000,000 records
//! (chapter 19.12's `Placed`) and building their assembly (every record checked against its
//! definition, then the instance tree), and 1,000 room-sized queries through to the parts; then
//! the same tower as one storey definition, a nested assembly of 10,000 records, placed 100 times.
//! Bytes are counted from the capacities the allocator was asked for (the kernel has no counting
//! allocator, which would need `unsafe`), and the process's peak resident memory is read from
//! `/proc/self/status` where there is one: the peak is that of one build at a time, the records
//! and the assembly being built with every working copy the build makes. `INSTANCE_RUNS` sets the number of timed runs (5);
//! each run's times are printed, then their medians.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_bvh::{Aabb, BoxTree, Entry};
use geom_math::{Axis, Box3, CrsId, FrameRecord, GridTransform};
use geom_num::grid::{GridStep, P3};
use geom_run::{Budget, Exec, WorkMeter};
use geom_spatial::instance::{
    Assembly, Content, DefinitionKey, Definitions, InstanceError, Place, Record,
};
use geom_topo::placed::{Definition, Placed, Placement};

/// Chapter 19.12's record, wrapped so the instance tree reads it, as the facade's wrapper does.
#[derive(Clone, Copy)]
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

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 24
    }
    fn below(&mut self, n: u64) -> i64 {
        (self.next() % n) as i64
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

fn grid(p: usize, shift: [i64; 3]) -> GridTransform {
    GridTransform::from_parts(
        ARRANGEMENTS[(p >> 3) % 6],
        [p & 1 != 0, p & 2 != 0, p & 4 != 0],
        shift,
    )
    .unwrap()
}

/// Steps per metre at q0.
const METRE: i64 = 6_400_000;
/// Instances a storey, storeys, definitions.
const PER_STOREY: usize = 10_000;
const STOREYS: usize = 100;
const KINDS: u32 = 200;

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

/// 200 definitions of 12 parts each, up to a metre across; the last 20 in a fine q1 frame.
fn definitions(rng: &mut Lcg) -> Definitions<Inst> {
    let coarse = FrameRecord::map(CrsId::Local(1), GridStep::Q0, [0; 3]).unwrap();
    let fine = FrameRecord::map(CrsId::Local(1), GridStep::Q1, [0; 3]).unwrap();
    let mut defs = Definitions::new();
    for k in 0..KINDS {
        let (frame, scale) = if k < KINDS - 20 {
            (coarse, 1)
        } else {
            (fine, 25)
        };
        let entries: Vec<Entry<3>> = (0..12u64)
            .map(|i| {
                let lo: [i64; 3] = std::array::from_fn(|_| rng.below(METRE as u64) * scale);
                let hi: [i64; 3] =
                    std::array::from_fn(|a| lo[a] + rng.below((METRE / 4) as u64) * scale);
                Entry {
                    key: i,
                    bbox: Aabb::new(lo, hi).unwrap(),
                }
            })
            .collect();
        let tree = BoxTree::build(&entries, &Exec::serial(), &mut meter()).unwrap();
        defs.get_or_insert_with(key(k), |_| Ok(Content::Parts { frame, tree }))
            .unwrap();
    }
    defs
}

/// `count` records of the 200 definitions over storeys of 100 m by 100 m, 3.5 m apart.
fn records(rng: &mut Lcg, count: usize, storeys: usize) -> Vec<(u32, GridTransform)> {
    (0..count)
        .map(|i| {
            let storey = (i * storeys / count) as i64;
            let h = rng.below(u64::from(KINDS)) as u32;
            let shift = [
                rng.below((100 * METRE) as u64),
                rng.below((100 * METRE) as u64),
                storey * 7 * METRE / 2,
            ];
            (h, grid(rng.below(48) as usize, shift))
        })
        .collect()
}

fn place(placements: &[(u32, GridTransform)], shared: &[Box3]) -> Vec<Inst> {
    placements
        .iter()
        .map(|(h, t)| Inst(Placed::exact(Definition(*h), t, &shared[*h as usize]).unwrap()))
        .collect()
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// The process's resident memory and its peak, in MiB, where `/proc/self/status` has them.
fn resident() -> String {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return "n/a (no /proc/self/status)".to_owned();
    };
    let field = |name: &str| {
        status
            .lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|kb| kb.parse::<f64>().ok())
            .map_or(f64::NAN, |kb| kb / 1024.0)
    };
    format!(
        "resident {:.0} MiB, peak {:.0} MiB",
        field("VmRSS:"),
        field("VmHWM:")
    )
}

fn main() {
    let runs: usize = std::env::var("INSTANCE_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let mut rng = Lcg(295);
    let frame = FrameRecord::map(CrsId::Local(1), GridStep::Q0, [0; 3]).unwrap();
    let defs = definitions(&mut rng);
    let shared: Vec<Box3> = (0..KINDS)
        .map(|h| defs.bound_in(h, &frame).unwrap())
        .collect();
    let n = PER_STOREY * STOREYS;
    let placements = records(&mut rng, n, STOREYS);
    println!("before: {}", resident());

    let (mut make_t, mut build_t, mut total_t) = (Vec::new(), Vec::new(), Vec::new());
    let mut kept: Option<Assembly<Inst>> = None;
    for run in 0..runs {
        // One assembly at a time, so the peak below is one build's.
        drop(kept.take());
        let start = Instant::now();
        let recs = place(&placements, &shared);
        let made = start.elapsed().as_secs_f64();
        let start = Instant::now();
        let a = Assembly::build(frame, recs, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
        let built = start.elapsed().as_secs_f64();
        println!(
            "run {run}: make {n} records {:.1} ms, build the assembly {:.1} ms, total {:.1} ms",
            made * 1e3,
            built * 1e3,
            (made + built) * 1e3
        );
        make_t.push(made);
        build_t.push(built);
        total_t.push(made + built);
        kept = Some(black_box(a));
    }
    let a = kept.unwrap();
    let total = median(total_t);
    println!(
        "median of {runs}: make {:.1} ms, build {:.1} ms, total {:.1} ms; target 1,000 ms: {}",
        median(make_t) * 1e3,
        median(build_t) * 1e3,
        total * 1e3,
        if total <= 1.0 { "met" } else { "MISSED" }
    );
    let bytes = a.heap_bytes() as f64 / n as f64;
    let tree = a.tree().heap_bytes() as f64 / n as f64;
    println!(
        "bytes an instance: {bytes:.1} (record {}, tree {tree:.1}: a 4-byte leaf and {:.2} nodes of {} bytes); target 160: {}",
        size_of::<Inst>(),
        a.tree().nodes().len() as f64 / n as f64,
        size_of::<geom_bvh::Node<3>>(),
        if bytes <= 160.0 { "met" } else { "MISSED" }
    );
    println!("after: {}", resident());

    // 1,000 room-sized queries (5 m cubes) through to the parts.
    let mut q = Lcg(2);
    let regions: Vec<Box3> = (0..1000)
        .map(|_| {
            let lo = [
                q.below((100 * METRE) as u64),
                q.below((100 * METRE) as u64),
                q.below((STOREYS as i64 * 7 * METRE / 2) as u64),
            ];
            let p = |v: [i64; 3]| P3::new(v[0], v[1], v[2]).unwrap();
            Box3::new(p(lo), p(lo.map(|c| c + 5 * METRE))).unwrap()
        })
        .collect();
    let mut query_t = Vec::new();
    let mut hits = 0;
    for _ in 0..runs {
        let start = Instant::now();
        hits = 0;
        for r in &regions {
            hits += black_box(a.query(&defs, &[], r, &mut meter()).unwrap()).len();
        }
        query_t.push(start.elapsed().as_secs_f64());
    }
    println!(
        "1,000 room queries through to the parts: median {:.1} ms ({:.1} µs each), {hits} part hits",
        median(query_t) * 1e3,
        median_each(&regions, &a, &defs)
    );
    drop(a);

    // The same tower as one storey definition placed 100 times.
    let storey: Vec<(u32, GridTransform)> = records(&mut rng, PER_STOREY, 1);
    let mut nested = defs.clone();
    let floor = Assembly::build(
        frame,
        place(&storey, &shared),
        &nested,
        &[],
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    let floor_bytes = floor.heap_bytes();
    let fh = nested
        .get_or_insert_with(key(KINDS), |_| Ok(Content::Assembly(floor)))
        .unwrap();
    let floor_box = nested.bound_in(fh, &frame).unwrap();
    let mut nested_t = Vec::new();
    let mut tower = None;
    for _ in 0..runs {
        let start = Instant::now();
        let recs: Vec<Inst> = (0..STOREYS as i64)
            .map(|s| {
                let t = GridTransform::translation([0, 0, s * 7 * METRE / 2]).unwrap();
                Inst(Placed::exact(Definition(fh), &t, &floor_box).unwrap())
            })
            .collect();
        let t = Assembly::build(frame, recs, &nested, &[], &Exec::serial(), &mut meter()).unwrap();
        nested_t.push(start.elapsed().as_secs_f64());
        tower = Some(black_box(t));
    }
    let tower = tower.unwrap();
    let mut nq = Vec::new();
    let mut nested_hits = 0;
    for _ in 0..runs {
        let start = Instant::now();
        nested_hits = 0;
        for r in &regions {
            nested_hits += black_box(tower.query(&nested, &[], r, &mut meter()).unwrap()).len();
        }
        nq.push(start.elapsed().as_secs_f64());
    }
    println!(
        "nested: one storey of {PER_STOREY} records ({:.1} bytes each) placed {STOREYS} times: build {:.3} ms, {} bytes in all for {n} instances ({:.2} an instance); 1,000 room queries {:.1} ms, {nested_hits} part hits",
        floor_bytes as f64 / PER_STOREY as f64,
        median(nested_t) * 1e3,
        floor_bytes + tower.heap_bytes(),
        (floor_bytes + tower.heap_bytes()) as f64 / n as f64,
        median(nq) * 1e3,
    );
}

/// The median time of one query, in µs, over the regions, each timed alone.
fn median_each(regions: &[Box3], a: &Assembly<Inst>, defs: &Definitions<Inst>) -> f64 {
    let each: Vec<f64> = regions
        .iter()
        .map(|r| {
            let start = Instant::now();
            black_box(a.query(defs, &[], r, &mut meter()).unwrap());
            start.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    median(each)
}
