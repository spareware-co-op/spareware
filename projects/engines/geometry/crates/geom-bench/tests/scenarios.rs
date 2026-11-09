//! The flat boolean's scenarios (piece G140): what each one builds, against closed forms, and
//! three selections from one arrangement within 1.2 times the work units of one (11.17's gate, in
//! work units; in instructions it is not met yet).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests: a failure stops the test"
)]

use geom_bench::harness::Kernel;
use geom_bench::inputs::Class;
use geom_bench::inputs::scenario::{self, Host};
use geom_bench::kernels::scenario as kernel;
use geom_bool::combine::combine;
use geom_bool::operand::{BooleanInput, Selection};
use geom_op::{OpContext, OpError, OpId, OpPath};
use geom_poly::merge::Merge;
use geom_run::{Budget, Exec};

/// One run of `kernel` over `batch`.
fn ran<T>(kernel: Kernel<T, Result<[u32; 4], OpError>>, batch: &[T]) -> Vec<[u32; 4]> {
    let mut out = Vec::new();
    kernel(batch, &mut out);
    out.into_iter().map(Result::unwrap).collect()
}

/// A box with `n` box holes through it: 6 + 4n faces, 12 + 12n edges, 8 + 8n vertices.
fn holed(n: usize) -> [u32; 4] {
    let n = u32::try_from(n).unwrap();
    [6 + 4 * n, 12 + 12 * n, 8 + 8 * n, 1]
}

#[test]
fn every_scenario_builds_what_its_shape_says() {
    let size = scenario::reduced();
    let class = Class::Typical;
    // The doors and holes go through: each adds four faces, twelve edges and eight vertices.
    assert_eq!(
        ran(kernel::openings, &scenario::wall(class, size)),
        [holed(size.openings)]
    );
    assert_eq!(
        ran(kernel::openings, &scenario::slab(class, size)),
        [holed(size.holes)]
    );
    // A moved door is still a door through the wall, and the cache cuts it as a fresh cut does.
    let recut = scenario::moved_door(class, size);
    assert_eq!(ran(kernel::moved_door, &recut), [holed(size.openings)]);
    let fresh = Host {
        host: recut[0].host.host.clone(),
        tools: recut[0].host.tools.clone(),
    };
    assert_eq!(
        ran(kernel::moved_door, &recut),
        ran(kernel::openings, &[fresh])
    );
    // Every room is a box of its own.
    let rooms = u32::try_from(size.rooms * size.rooms).unwrap();
    assert_eq!(
        ran(kernel::storey, &scenario::storey(class, size)),
        [[6 * rooms, 12 * rooms, 8 * rooms, rooms]]
    );
    // The union is one body; three selections give three.
    assert_eq!(ran(kernel::union, &scenario::boxes(class, size))[0][3], 1);
    let one = scenario::picks(class, size, 1);
    assert_eq!(ran(kernel::selections_of, &one), [holed(size.openings)]);
    let three = scenario::picks(class, size, 3);
    assert_eq!(ran(kernel::selections_of, &three)[0][3], 3);
    // No other class has a scenario.
    assert!(scenario::wall(Class::Degenerate, size).is_empty());
}

/// The least work `select` takes on `host`: the smallest budget it finishes within.
fn work(host: &Host, select: &[Selection]) -> u64 {
    let (mut low, mut high) = (0u64, 1u64 << 20);
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        let ctx = OpContext::new(
            OpPath::new(OpId::from_bytes([4; 16])),
            Budget::new(middle, u64::MAX),
        );
        let cut = scenario::cut_of(host, None);
        let mut operands = vec![cut.host];
        operands.extend(cut.tools);
        let input = BooleanInput {
            operands,
            select: select.to_vec(),
            merge: Merge::RemoveRedundant,
            accept: None,
        };
        if combine(&ctx, &Exec::serial(), &input).is_ok() {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

#[test]
fn three_selections_charge_under_1_2_times_the_work_units_of_one() {
    // 11.17's gate asks that three selections from one arrangement cost under 1.2 times one. In
    // the work units every operation charges (23.10) it holds on the scenario's 30-door wall:
    // 166,493 against 140,251 (1.187) when written, and 1.25 on 10 doors. In instructions it does
    // not: 913 million against 428 million (2.13), as each selection's merge and naming cost about
    // 70% of one selection and the arrangement only 28% (Callgrind, G140). So this pins the work
    // units, and the gate in cost stays open until the rebuild and naming get cheaper or the work
    // units charge them what they cost.
    let host = scenario::wall(Class::Typical, scenario::reduced())
        .pop()
        .unwrap();
    let target = scenario::solid(0, &host.host).key;
    let one = work(&host, &[Selection::Subtract { target }]);
    let three = work(
        &host,
        &[
            Selection::Subtract { target },
            Selection::Unite,
            Selection::Intersect,
        ],
    );
    assert!(10 * three < 12 * one, "three {three}, one {one}");
}
