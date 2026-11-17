//! The flat boolean's scenarios (piece G140): what each one builds, against closed forms, and
//! three selections from one arrangement within 1.2 times the work units of one (11.17's gate, in
//! work units; in instructions it is not met yet); and every input and result through both
//! checkers of 28.4, which agree (Gate 4, G156).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests: a failure stops the test"
)]

use geom_bench::harness::Kernel;
use geom_bench::inputs::Class;
use geom_bench::inputs::scenario::{self, Host};
use geom_bench::kernels::scenario as kernel;
use geom_bool::batch::{BatchRule, CutResult, cut};
use geom_bool::combine::{combine, unite};
use geom_bool::operand::{BooleanInput, Selection, Term};
use geom_num::Exactness;
use geom_op::{OpContext, OpError, OpId, OpOutput, OpPath};
use geom_poly::merge::Merge;
use geom_run::{Budget, Exec};
use geom_test::agree::judge;
use geom_topo::Body;

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

/// Both checkers on every body, each named by `what` and its place: they agree, both pass, and the
/// second checker judges every face and edge (28.4).
fn both_pass<'b>(what: &str, bodies: impl IntoIterator<Item = &'b Body>, unjudged: &mut usize) {
    for (k, body) in bodies.into_iter().enumerate() {
        let verdict = judge(body, Exactness::Exact).unwrap();
        assert!(
            verdict.agree() && verdict.passed(),
            "{what} {k}: {verdict:?}"
        );
        *unjudged += verdict.unjudged;
    }
}

#[test]
fn every_scenario_result_passes_both_checkers() {
    // Gate 4 (G156): the scenarios at the size the count gates run, every input and every result
    // through `geom-check` and the second checker of 28.4, which must agree.
    let (class, size) = (Class::Typical, scenario::reduced());
    let ctx = OpContext::new(OpPath::new(OpId::from_bytes([4; 16])), Budget::UNLIMITED);
    let serial = Exec::serial();
    let mut unjudged = 0;
    let cut_bodies = |out: OpOutput<Vec<CutResult>>| -> Vec<Body> {
        out.result.into_iter().map(|one| one.result.body).collect()
    };
    for (name, hosts) in [
        ("openings", scenario::wall(class, size)),
        ("slab_holes", scenario::slab(class, size)),
    ] {
        for host in &hosts {
            both_pass(
                name,
                core::iter::once(&host.host).chain(&host.tools),
                &mut unjudged,
            );
            let out = cut(
                &ctx,
                &serial,
                &[scenario::cut_of(host, None)],
                BatchRule::Atomic,
            );
            both_pass(name, &cut_bodies(out.unwrap()), &mut unjudged);
        }
    }
    for recut in &scenario::moved_door(class, size) {
        let out = cut(
            &ctx,
            &serial,
            &[scenario::cut_of(&recut.host, Some(&recut.cache))],
            BatchRule::Atomic,
        );
        both_pass("moved_door", &cut_bodies(out.unwrap()), &mut unjudged);
    }
    for many in &scenario::boxes(class, size) {
        let operands: Vec<_> = (0..)
            .zip(&many.bodies)
            .map(|(k, body)| scenario::solid(k, body))
            .collect();
        let out = unite(&ctx, &serial, &operands).unwrap();
        both_pass("union", [&out.result.body], &mut unjudged);
    }
    for count in [1, 3] {
        for picks in &scenario::picks(class, size, count) {
            let cut = scenario::cut_of(&picks.host, None);
            let mut operands = vec![cut.host];
            operands.extend(cut.tools);
            let input = BooleanInput {
                operands,
                select: scenario::selections(picks),
                merge: Merge::RemoveRedundant,
                accept: None,
            };
            let out = combine(&ctx, &serial, &input).unwrap();
            both_pass(
                "selections",
                out.result.iter().flatten().map(|one| &one.body),
                &mut unjudged,
            );
        }
    }
    for storey in &scenario::storey(class, size) {
        let envelope = scenario::solid(0, &storey.envelope);
        let mut test = vec![Term::Inside(envelope.key)];
        let mut operands = vec![envelope];
        for (k, wall) in (1..).zip(&storey.walls) {
            let one = scenario::solid(k, wall);
            test.extend([Term::Inside(one.key), Term::Not, Term::And]);
            operands.push(one);
        }
        let input = BooleanInput {
            operands,
            select: vec![Selection::Cells {
                test,
                seeds: scenario::seeds(storey),
            }],
            merge: Merge::RemoveRedundant,
            accept: None,
        };
        let out = combine(&ctx, &serial, &input).unwrap();
        both_pass(
            "storey",
            out.result.iter().flatten().map(|one| &one.body),
            &mut unjudged,
        );
    }
    // Every face and edge here is flat or straight, so the second checker reads them all.
    assert_eq!(unjudged, 0);
}
