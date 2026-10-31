//! The benchmark of piece G87 (chapter 30): hashing names under `NameScheme v1`, interning them in
//! the name graph, and ordering colliding entities by 20.2's rule.
//!
//! Run with `cargo bench -p geom-name`. It uses the standard library's clock only (wall-clock time
//! on this machine, one thread; the reference machine and its nightly timings are piece G107's).
//! The inputs come from a fixed linear congruential sequence.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::cmp::Ordering;
use std::hint::black_box;
use std::time::Instant;

use geom_name::{NameGraph, WeakOrder, ordinals};
use geom_op::name::{NameList, NameSet, Ordinal, Role, RoleArg, TopoName};
use geom_op::{NameId, OpId, OpPath};

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn name(&mut self) -> NameId {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&self.next().to_le_bytes());
        bytes[8..].copy_from_slice(&self.next().to_le_bytes());
        NameId::from_bytes(bytes)
    }
}

/// Runs `f` once to warm up, then `runs` times, and prints the mean time per item.
fn time(label: &str, runs: u32, items: usize, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let per_run = start.elapsed().as_secs_f64() / f64::from(runs);
    println!(
        "{label:<48} {:>10.1} µs {:>10.0} ns/item",
        per_run * 1e6,
        per_run * 1e9 / items as f64
    );
}

struct Key(i64, i64);

impl WeakOrder for Key {
    fn least_point(&self, other: &Self) -> Option<Ordering> {
        Some(self.0.cmp(&other.0))
    }
    fn vertices(&self, _: &Self) -> Option<Ordering> {
        Some(Ordering::Equal)
    }
    fn direction(&self, other: &Self) -> Option<Ordering> {
        Some(self.1.cmp(&other.1))
    }
}

fn main() {
    let mut lcg = Lcg(0x5350_4152_454b_4954);
    let op = OpPath::new(OpId::from_bytes([7; 16]))
        .step("extrude")
        .unwrap();
    let names: Vec<TopoName> = (0..10_000)
        .map(|i| match i % 3 {
            0 => TopoName::Created {
                op: op.clone(),
                role: Role::new("side", [RoleArg::Name(lcg.name())]).unwrap(),
                from: NameList::new([lcg.name(), lcg.name()]),
            },
            1 => TopoName::Merged {
                op: op.clone(),
                of: NameSet::new([lcg.name(), lcg.name(), lcg.name()]).unwrap(),
            },
            _ => TopoName::Weak {
                base: lcg.name(),
                ord: Ordinal::new(1, 3).unwrap(),
            },
        })
        .collect();
    time("TopoName::id, 10,000 names", 20, names.len(), || {
        for name in &names {
            black_box(name.id().unwrap());
        }
    });
    time("NameGraph::intern, 10,000 names", 20, names.len(), || {
        let mut graph = NameGraph::new();
        for name in &names {
            black_box(graph.intern(name.clone()).unwrap());
        }
    });
    for n in [100_usize, 100_000] {
        let keys: Vec<Key> = (0..n)
            .map(|i| Key((lcg.next() % 1000) as i64, i as i64))
            .collect();
        let runs = if n > 1000 { 5 } else { 2000 };
        time(
            &format!("ordinals, {n} colliding entities"),
            runs,
            n,
            || {
                black_box(ordinals(&keys).unwrap());
            },
        );
    }
}
