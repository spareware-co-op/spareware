//! Batch cutting against the chapter's list: openings sharing jambs, tools that miss or only touch,
//! many hosts in input order, the same on any threads, and skipped tools listed (G138, 11.6).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic, and a failure stops the test"
)]

use geom_bool::batch::{BatchRule, Cut, Recut, ToolReport, cut};
use geom_bool::operand::{Operand, OperandKey, OperandKind};
use geom_check::{Level, check};
use geom_make::{BoxInput, RoundInput, cylinder, make_box};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{ErrorKind, OpContext, OpId, OpPath};
#[cfg(not(target_family = "wasm"))]
use geom_run::Pool;
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::Body;
use geom_topo::query::volume;
use proptest::prelude::*;

const SERIAL: Exec<'static> = Exec::serial();

/// A pool that runs each task on its own thread, last first.
#[cfg(not(target_family = "wasm"))]
struct Threads;

#[cfg(not(target_family = "wasm"))]
impl Pool for Threads {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        std::thread::scope(|scope| {
            for task in tasks.iter_mut().rev() {
                scope.spawn(task);
            }
        });
    }
}

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn cuboid(low: [i64; 3], size: [u64; 3]) -> Body {
    let input = BoxInput {
        min: P3::new(low[0], low[1], low[2]).unwrap(),
        size: size.map(Steps),
    };
    make_box(&ctx(), &input).unwrap().result.body
}

fn key(n: u16) -> OperandKey {
    let mut bytes = [0; 16];
    bytes[..2].copy_from_slice(&n.to_be_bytes());
    OperandKey(bytes)
}

fn solid(n: u16, body: &Body) -> Operand<'_> {
    Operand {
        key: key(n),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

fn sixfold(body: &Body) -> i128 {
    volume(body, &mut meter()).unwrap().to_i128().unwrap()
}

fn checked(body: &Body) {
    let report = check(body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(report.passed(), "{report:?}");
}

#[test]
fn a_wall_takes_its_openings_in_one_call() {
    let wall = cuboid([0, 0, 0], [1000, 10, 60]);
    let doors: Vec<Body> = (0..30)
        .map(|i| cuboid([10 + 30 * i, -5, 10], [20, 20, 30]))
        .collect();
    let tools: Vec<Operand<'_>> = doors
        .iter()
        .enumerate()
        .map(|(i, door)| solid(1 + i as u16, door))
        .collect();
    let one = Cut {
        host: solid(0, &wall),
        tools,
        reuse: None,
    };
    let out = cut(&ctx(), &SERIAL, &[one], BatchRule::Atomic).unwrap();
    let result = &out.result[0];
    assert_eq!(
        sixfold(&result.result.body),
        6 * (1000 * 10 * 60 - 30 * 20 * 10 * 30)
    );
    checked(&result.result.body);
    assert_eq!(result.tools.len(), 30);
    assert!(result.tools.iter().all(|tool| tool.cut));
    assert_eq!(result.skipped, &[] as &[OperandKey]);
}

#[test]
fn openings_that_share_a_jamb_or_overlap_need_no_union() {
    let wall = cuboid([0, 0, 0], [100, 10, 60]);
    let left = cuboid([10, -5, 10], [20, 20, 30]);
    let right = cuboid([30, -5, 10], [20, 20, 30]);
    let over = cuboid([40, -5, 10], [20, 20, 30]);
    let one = Cut {
        host: solid(0, &wall),
        tools: vec![solid(1, &left), solid(2, &right), solid(3, &over)],
        reuse: None,
    };
    let out = cut(&ctx(), &SERIAL, &[one], BatchRule::Atomic).unwrap();
    assert_eq!(
        sixfold(&out.result[0].result.body),
        6 * (100 * 10 * 60 - 50 * 10 * 30)
    );
    checked(&out.result[0].result.body);
}

#[test]
fn a_tool_that_misses_or_only_touches_is_named() {
    let wall = cuboid([0, 0, 0], [100, 10, 60]);
    let door = cuboid([40, -5, 0], [20, 20, 40]);
    let far = cuboid([500, 0, 0], [10, 10, 10]);
    let touching = cuboid([100, 0, 0], [10, 10, 10]);
    let one = Cut {
        host: solid(0, &wall),
        tools: vec![solid(1, &door), solid(2, &far), solid(3, &touching)],
        reuse: None,
    };
    let out = cut(&ctx(), &SERIAL, &[one], BatchRule::Atomic).unwrap();
    assert_eq!(
        out.result[0].tools,
        [
            ToolReport {
                key: key(1),
                cut: true
            },
            ToolReport {
                key: key(2),
                cut: false
            },
            ToolReport {
                key: key(3),
                cut: false
            },
        ]
    );
    let misses = out
        .diagnostics
        .iter()
        .filter(|d| d.code == "bool.tool_misses_host")
        .count();
    assert_eq!(misses, 2);
}

fn round() -> Body {
    let input = RoundInput {
        base: P3::new(50, 5, 10).unwrap(),
        radius: Steps(4),
        height: Steps(10),
        step: GridStep::Q0,
    };
    cylinder(&ctx(), &input).unwrap().result.body
}

#[test]
fn a_tool_that_cannot_enter_fails_the_call_or_is_listed() {
    let wall = cuboid([0, 0, 0], [100, 10, 60]);
    let door = cuboid([10, -5, 0], [20, 20, 40]);
    let drill = round();
    let cuts = || {
        [Cut {
            host: solid(0, &wall),
            tools: vec![solid(1, &door), solid(2, &drill)],
            reuse: None,
        }]
    };
    let refused = cut(&ctx(), &SERIAL, &cuts(), BatchRule::Atomic).unwrap_err();
    assert_eq!(refused.kind(), ErrorKind::Unsupported);
    let out = cut(&ctx(), &SERIAL, &cuts(), BatchRule::SkipFailedTools).unwrap();
    assert_eq!(out.result[0].skipped, [key(2)]);
    assert_eq!(out.result[0].tools.len(), 1);
    assert_eq!(
        sixfold(&out.result[0].result.body),
        6 * (100 * 10 * 60 - 20 * 10 * 40)
    );
    let host_fails = [Cut {
        host: solid(2, &drill),
        tools: vec![solid(1, &door)],
        reuse: None,
    }];
    let refused = cut(&ctx(), &SERIAL, &host_fails, BatchRule::SkipFailedTools).unwrap_err();
    assert_eq!(refused.kind(), ErrorKind::Unsupported);
}

#[test]
fn many_hosts_come_back_in_input_order_on_any_threads() {
    let walls: Vec<Body> = (0..8)
        .map(|i| cuboid([0, 100 * i, 0], [100 + 10 * i.cast_unsigned(), 10, 60]))
        .collect();
    let doors: Vec<Body> = (0..8)
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
    let out = cut(&ctx(), &SERIAL, &cuts, BatchRule::Atomic).unwrap();
    for (i, result) in out.result.iter().enumerate() {
        let long = 100 + 10 * i as i128;
        assert_eq!(
            sixfold(&result.result.body),
            6 * (long * 10 * 60 - 20 * 10 * 40)
        );
    }
    #[cfg(not(target_family = "wasm"))]
    {
        let exec = Exec::with_pool(&Threads);
        let again = cut(&ctx(), &exec, &cuts, BatchRule::Atomic).unwrap();
        assert_eq!(again.result, out.result);
    }
}

#[test]
fn a_winding_vector_stays_two_entries_long() {
    // Three hundred overlapping tools would make vectors of three hundred entries.
    let wall = cuboid([0, 0, 0], [400, 10, 60]);
    let doors: Vec<Body> = (0..12)
        .map(|i| cuboid([20 + 20 * i, -5, 10], [30, 20, 30]))
        .collect();
    let tools: Vec<Operand<'_>> = doors
        .iter()
        .enumerate()
        .map(|(i, door)| solid(1 + i as u16, door))
        .collect();
    let one = Cut {
        host: solid(0, &wall),
        tools,
        reuse: None,
    };
    let out = cut(&ctx(), &SERIAL, &[one], BatchRule::Atomic).unwrap();
    assert_eq!(
        sixfold(&out.result[0].result.body),
        6 * (400 * 10 * 60 - 250 * 10 * 30)
    );
}

/// A wall of `openings` doors, and its doors.
fn wall_of(openings: i64) -> (Body, Vec<Body>) {
    let wall = cuboid([0, 0, 0], [(10 + 30 * openings).cast_unsigned(), 10, 60]);
    let doors = (0..openings)
        .map(|i| cuboid([10 + 30 * i, -5, 10], [20, 20, 30]))
        .collect();
    (wall, doors)
}

fn cut_of<'b>(wall: &'b Body, doors: &'b [Body]) -> Cut<'b> {
    Cut {
        host: solid(0, wall),
        tools: doors
            .iter()
            .enumerate()
            .map(|(i, door)| solid(1 + i as u16, door))
            .collect(),
        reuse: None,
    }
}

/// The least budget a wall of `openings` needs, which is the work it takes.
fn work_of(openings: i64) -> u64 {
    let (wall, doors) = wall_of(openings);
    least(&cut_of(&wall, &doors))
}

/// The least budget `one` needs, which is the work it takes.
fn least(one: &Cut<'_>) -> u64 {
    let (mut low, mut high) = (0u64, 1 << 32);
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        let small = OpContext::new(
            OpPath::new(OpId::from_bytes([7; 16])),
            Budget::new(middle, u64::MAX),
        );
        if cut(
            &small,
            &SERIAL,
            std::slice::from_ref(one),
            BatchRule::Atomic,
        )
        .is_ok()
        {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

/// Whether every chunk of `a` is `b`'s very chunk, not a copy.
fn all_shared<K, V>(a: &geom_topo::Column<K, V>, b: &geom_topo::Column<K, V>) -> bool {
    a.chunks()
        .zip(b.chunks())
        .all(|(x, y)| x.as_ptr() == y.as_ptr())
}

#[test]
fn a_moved_door_is_cut_again_from_the_last_cut_as_a_fresh_cut_would_be() {
    // A wall of twenty doors, and one moved along it: the boxes about its old and new places are
    // solved again, the rest come from the last cut, and the body, the report, the cache and the
    // work are those of a fresh cut (11.6).
    let (wall, mut doors) = wall_of(20);
    let first = cut(&ctx(), &SERIAL, &[cut_of(&wall, &doors)], BatchRule::Atomic).unwrap();
    let last = &first.result[0].cache;
    doors[9] = cuboid([10 + 30 * 9 + 5, -5, 10], [20, 20, 30]);
    let fresh = cut(&ctx(), &SERIAL, &[cut_of(&wall, &doors)], BatchRule::Atomic).unwrap();
    let again = Cut {
        reuse: Some(last),
        ..cut_of(&wall, &doors)
    };
    let local = cut(
        &ctx(),
        &SERIAL,
        std::slice::from_ref(&again),
        BatchRule::Atomic,
    )
    .unwrap();
    assert_eq!(local.result, fresh.result);
    assert_eq!(local.diagnostics, fresh.diagnostics);
    let (kept, all) = (
        last.boxes().shared_with(local.result[0].cache.boxes()),
        last.boxes().len(),
    );
    assert!(kept * 5 >= all * 4, "{kept} of {all} boxes reused");
    assert!(kept < all);
    let merge = &local.result[0].cache.merge();
    let (kept, all) = (merge.shared_with(last.merge()), merge.len());
    assert!(kept * 5 >= all * 4, "{kept} of {all} merged pieces reused");
    assert!(kept < all);
    let names = local.result[0].cache.names();
    assert!(names.len() > 1 && !names.is_empty());
    assert!(Recut::default().names().is_empty());
    assert!(local.result[0].cache.order().is_some() && Recut::default().order().is_none());
    // The door kept every name and count, so the re-cut patched the last body: its links are the
    // last body's very chunks, and only the moved door's values were written.
    let (now, before) = (&local.result[0].result.body, last.body().unwrap());
    assert_eq!(local.result[0].cache.body(), Some(now));
    assert!(all_shared(
        &now.arenas().fins.edge,
        &before.arenas().fins.edge
    ));
    assert!(all_shared(
        &now.arenas().edges.start,
        &before.arenas().edges.start
    ));
    assert!(!all_shared(&now.arenas().points, &before.arenas().points));
    assert!(Recut::default().body().is_none());
    assert_eq!(least(&again), least(&cut_of(&wall, &doors)));
}

/// A door's change: moved by steps along the wall and up it, made wider or narrower, taken out, or
/// a door put in.
#[derive(Clone, Copy, Debug)]
enum Change {
    Moved(usize, i64, i64, i64),
    Out(usize),
    In(i64, i64),
}

fn any_change() -> impl Strategy<Value = Change> {
    prop_oneof![
        4 => (0usize..8, -40i64..40, -15i64..15, -10i64..10)
            .prop_map(|(at, along, up, wider)| Change::Moved(at, along, up, wider)),
        1 => (0usize..8).prop_map(Change::Out),
        1 => (-10i64..200, 0i64..40).prop_map(|(along, up)| Change::In(along, up)),
    ]
}

/// The doors after `change`, as their corners and sizes.
fn changed(doors: &mut Vec<([i64; 3], [u64; 3])>, change: Change) {
    match change {
        Change::Moved(at, along, up, wider) if !doors.is_empty() => {
            let count = doors.len();
            let (low, size) = &mut doors[at % count];
            low[0] += along;
            low[2] += up;
            size[0] = size[0].saturating_add_signed(wider).max(1);
        }
        Change::Out(at) if doors.len() > 1 => {
            doors.remove(at % doors.len());
        }
        Change::In(along, up) => doors.push(([along, -5, up], [20, 20, 30])),
        Change::Moved(..) | Change::Out(_) => {}
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(12))]

    #[test]
    fn any_run_of_changes_cuts_again_as_fresh_cuts_would(
        changes in prop::collection::vec(any_change(), 1..4),
    ) {
        // G139's law: each cut from the last is the fresh cut, bit for bit, cache and all.
        let wall = cuboid([0, 0, 0], [190, 10, 60]);
        let mut doors: Vec<([i64; 3], [u64; 3])> =
            (0..6).map(|i| ([10 + 30 * i, -5, 10], [20, 20, 30])).collect();
        let bodies = |doors: &[([i64; 3], [u64; 3])]| -> Vec<Body> {
            doors.iter().map(|(low, size)| cuboid(*low, *size)).collect()
        };
        let first = bodies(&doors);
        let mut last = cut(&ctx(), &SERIAL, &[cut_of(&wall, &first)], BatchRule::Atomic).unwrap();
        for change in changes {
            changed(&mut doors, change);
            let now = bodies(&doors);
            let fresh = cut(&ctx(), &SERIAL, &[cut_of(&wall, &now)], BatchRule::Atomic);
            let again = Cut {
                reuse: Some(&last.result[0].cache),
                ..cut_of(&wall, &now)
            };
            let local = cut(&ctx(), &SERIAL, &[again], BatchRule::Atomic);
            prop_assert_eq!(&local, &fresh, "after {:?}", change);
            let Ok(local) = local else { break };
            last = local;
        }
    }
}

#[test]
fn the_work_grows_in_step_with_the_openings() {
    let (few, many) = (work_of(10), work_of(40));
    assert!(many < few * 5, "{few} then {many}");
    assert!(many > few * 3, "{few} then {many}");
}

/// The 300-opening wall of G138, timed by hand; G140's gate counts its instructions.
#[test]
#[ignore = "a timing to read, not a check"]
#[allow(clippy::print_stdout, reason = "the timing is the output")]
fn a_wall_of_three_hundred_openings_timed() {
    let (wall, doors) = wall_of(300);
    for (label, exec) in [
        ("one thread", SERIAL),
        ("threads", Exec::with_pool(&Threads)),
    ] {
        let started = std::time::Instant::now();
        let out = cut(&ctx(), &exec, &[cut_of(&wall, &doors)], BatchRule::Atomic).unwrap();
        println!("300 openings, {label}: {:?}", started.elapsed());
        assert_eq!(out.result[0].tools.len(), 300);
    }
}

/// A door of the 300-opening wall moved and the wall cut again from its last cut, timed by hand;
/// G140's gate counts its instructions.
#[test]
#[ignore = "a timing to read, not a check"]
#[allow(clippy::print_stdout, reason = "the timing is the output")]
fn a_door_moved_in_a_wall_of_three_hundred_openings_timed() {
    let (wall, mut doors) = wall_of(300);
    for (label, exec) in [
        ("one thread", SERIAL),
        ("threads", Exec::with_pool(&Threads)),
    ] {
        let first = cut(&ctx(), &exec, &[cut_of(&wall, &doors)], BatchRule::Atomic).unwrap();
        doors[150] = cuboid([10 + 30 * 150 + 5, -5, 10], [20, 20, 30]);
        let again = Cut {
            reuse: Some(&first.result[0].cache),
            ..cut_of(&wall, &doors)
        };
        let started = std::time::Instant::now();
        let out = cut(&ctx(), &exec, &[again], BatchRule::Atomic).unwrap();
        println!(
            "a door moved in 300 openings, {label}: {:?}, {} of {} boxes reused",
            started.elapsed(),
            first.result[0]
                .cache
                .boxes()
                .shared_with(out.result[0].cache.boxes()),
            out.result[0].cache.boxes().len()
        );
        doors[150] = cuboid([10 + 30 * 150, -5, 10], [20, 20, 30]);
    }
}
