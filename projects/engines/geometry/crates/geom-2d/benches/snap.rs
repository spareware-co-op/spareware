//! How long a snap query takes on a drawing of a million entities, by the standard library's clock
//! (piece G198): the done-when's "1M-entity snap query". Builds the drawing and its index once, then
//! asks 20,000 snaps at random cursors with an aperture of 64 steps for every kind that needs no
//! second point, plus intersections. `SNAP_ENTITIES` replaces the million. Prints one line per
//! stage; the gate is the count benchmark in `geom-bench`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::print_stdout,
    reason = "a benchmark program; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::hint::black_box;
use std::num::NonZeroU64;
use std::time::Instant;

use geom_2d::carrier::{Carriers, Circle2};
use geom_2d::entity::{Attr, Ends, Entity, Shape};
use geom_2d::index::SnapIndex;
use geom_2d::point::Pt2;
use geom_2d::snap::{Drawing, SnapKind};
use geom_num::arc::Circle2 as GridCircle;
use geom_num::carrier::Carrier;
use geom_num::grid::{P2, Steps};
use geom_op::NameId;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::{Seed, SplitMix64};

/// The side of the square the entities lie in, in steps.
const SIDE: u64 = 4_000_000;

fn p(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn name(n: u32) -> NameId {
    let mut b = [0_u8; 16];
    b[12..].copy_from_slice(&n.to_be_bytes());
    NameId::from_bytes(b)
}

fn main() {
    let count: u32 = std::env::var("SNAP_ENTITIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000);
    let mut rng = SplitMix64::new(Seed(0x198));
    let mut below = |n: u64| rng.below(NonZeroU64::new(n).unwrap()) as i64;
    let start = Instant::now();
    let mut items = Vec::with_capacity(count as usize);
    for n in 0..count {
        let (x, y) = (below(SIDE), below(SIDE));
        let shape = match below(8) {
            0 => Shape::Point(p(x, y)),
            1 => Shape::Circle(Circle2::Grid(
                GridCircle::new(p(x, y), i128::from(1 + below(200))).unwrap(),
            )),
            _ => {
                let (dx, dy) = (below(800) - 400, below(800) - 400);
                if dx == 0 && dy == 0 {
                    Shape::Point(p(x, y))
                } else {
                    Shape::Line {
                        carrier: Carrier::through(p(x, y), p(x + dx, y + dy)).unwrap(),
                        ends: Ends::Segment(Pt2::Grid(p(x, y)), Pt2::Grid(p(x + dx, y + dy))),
                    }
                }
            }
        };
        items.push((name(n), Entity::new(Attr(0), shape)));
    }
    println!(
        "{count} entities made{:>40.2} s",
        start.elapsed().as_secs_f64()
    );
    let start = Instant::now();
    let drawing = Drawing::new(items, Vec::new()).unwrap();
    println!(
        "the drawing, in name order{:>34.2} s",
        start.elapsed().as_secs_f64()
    );
    let start = Instant::now();
    let index = SnapIndex::build(
        &drawing,
        &Exec::serial(),
        &mut WorkMeter::new(Budget::UNLIMITED),
    )
    .unwrap();
    println!("the index built{:>45.2} s", start.elapsed().as_secs_f64());
    let kinds = [
        SnapKind::End,
        SnapKind::Start,
        SnapKind::Mid,
        SnapKind::Center,
        SnapKind::Node,
        SnapKind::Quadrant,
        SnapKind::Intersection,
    ];
    let queries = 20_000_u32;
    let mut carriers = Carriers::new();
    let mut found = 0_u32;
    let mut worst = 0.0_f64;
    let start = Instant::now();
    for _ in 0..queries {
        let cursor = p(below(SIDE), below(SIDE));
        let one = Instant::now();
        let got = index
            .snap(
                black_box(cursor),
                Steps(64),
                &kinds,
                &mut carriers,
                &mut WorkMeter::new(Budget::UNLIMITED),
            )
            .unwrap();
        worst = worst.max(one.elapsed().as_secs_f64());
        found += u32::from(got.is_some());
        carriers.truncate(0);
    }
    let mean = start.elapsed().as_secs_f64() * 1e6 / f64::from(queries);
    println!(
        "a snap of {} kinds, aperture 64 steps: mean {mean:.1} us, worst {:.1} us, {found} of {queries} found",
        kinds.len(),
        worst * 1e6
    );
}
