// SPDX-License-Identifier: Apache-2.0
//! The recursion bound of `geom-bool`'s module headers, run at its bound (chapter 28.8; the
//! recursion check of piece G13): a batch of hosts cut on a pool, and one host alone on a pool,
//! with a pool that counts how many of its parallel phases are open at once.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic, a counter that never decides a result, and a failure stops the test"
)]

use geom_bool::batch::{BatchRule, Cut, cut};
use geom_bool::operand::{Operand, OperandKey, OperandKind};
use geom_make::{BoxInput, make_box};
use geom_num::{Exactness, P3, Steps};
use geom_op::{OpContext, OpId, OpPath};
#[cfg(not(target_family = "wasm"))]
use geom_run::Pool;
use geom_run::{Budget, Exec};
use geom_topo::Body;
#[cfg(not(target_family = "wasm"))]
use std::sync::atomic::{AtomicUsize, Ordering};

/// A pool that runs its tasks one after another and counts the parallel phases open at once.
#[cfg(not(target_family = "wasm"))]
struct Counting {
    open: AtomicUsize,
    deepest: AtomicUsize,
}

#[cfg(not(target_family = "wasm"))]
impl Pool for Counting {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        let now = self.open.fetch_add(1, Ordering::SeqCst) + 1;
        self.deepest.fetch_max(now, Ordering::SeqCst);
        for task in tasks.iter_mut() {
            task();
        }
        self.open.fetch_sub(1, Ordering::SeqCst);
    }
}

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn cuboid(low: [i64; 3], size: [u64; 3]) -> Body {
    let input = BoxInput {
        min: P3::new(low[0], low[1], low[2]).unwrap(),
        size: size.map(Steps),
    };
    make_box(&ctx(), &input).unwrap().result.body
}

fn solid(n: u16, body: &Body) -> Operand<'_> {
    let mut bytes = [0; 16];
    bytes[..2].copy_from_slice(&n.to_be_bytes());
    Operand {
        key: OperandKey(bytes),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

/// Cuts `hosts` walls, each with one door, on the counting pool, and returns the most parallel
/// phases that were open at once.
#[cfg(not(target_family = "wasm"))]
fn deepest_with(hosts: i64) -> usize {
    let walls: Vec<Body> = (0..hosts)
        .map(|i| cuboid([0, 100 * i, 0], [100, 10, 60]))
        .collect();
    let doors: Vec<Body> = (0..hosts)
        .map(|i| cuboid([40, 100 * i - 5, 0], [20, 20, 40]))
        .collect();
    let cuts: Vec<Cut<'_>> = walls
        .iter()
        .zip(&doors)
        .enumerate()
        .map(|(i, (wall, door))| Cut {
            host: solid(2 * i as u16, wall),
            tools: vec![solid(2 * i as u16 + 1, door)],
            reuse: None,
        })
        .collect();
    let pool = Counting {
        open: AtomicUsize::new(0),
        deepest: AtomicUsize::new(0),
    };
    let on_pool = cut(&ctx(), &Exec::with_pool(&pool), &cuts, BatchRule::Atomic).unwrap();
    let serial = cut(&ctx(), &Exec::serial(), &cuts, BatchRule::Atomic).unwrap();
    assert_eq!(on_pool.result, serial.result);
    pool.deepest.load(Ordering::SeqCst)
}

#[test]
fn parallel_phases_nest_at_most_twice() {
    #[cfg(not(target_family = "wasm"))]
    {
        // Many hosts: the hosts' phase, and nothing open inside it.
        assert!(deepest_with(6) <= 2);
        // One host: the arrangement's phases, one after another.
        assert!(deepest_with(1) <= 2);
    }
    // Without a pool every phase is inline, so a batch runs the cycle with no threads at all.
    let wall = cuboid([0, 0, 0], [100, 10, 60]);
    let door = cuboid([40, -5, 0], [20, 20, 40]);
    let cuts = [Cut {
        host: solid(0, &wall),
        tools: vec![solid(1, &door)],
        reuse: None,
    }];
    assert!(cut(&ctx(), &Exec::serial(), &cuts, BatchRule::Atomic).is_ok());
}
