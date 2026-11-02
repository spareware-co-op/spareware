//! The flat boolean's scenarios (piece G140): what each one builds, against closed forms, and
//! 11.17's benchmark gate, three selections from one arrangement within 1.2 times the work of one.

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
fn three_selections_from_one_arrangement_cost_under_1_2_times_one() {
    // 11.17's gate on the scenario's own wall of 30 doors, in the work units every operation
    // charges (23.10), which do not depend on the machine: 166,493 against 140,251 (1.187) when
    // written. Each selection adds its own rebuild, so the ratio falls as the arrangement grows:
    // on 10 doors it is 45,583 against 36,453 (1.25). Timed, the 30 doors took about 2.2 times
    // as long for three, as the rebuild takes more of the time than of the work units (23.10).
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
