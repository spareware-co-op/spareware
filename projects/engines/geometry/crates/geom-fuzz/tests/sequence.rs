//! The sequence fuzz target (G116), run as tests on stable Rust: bolero's test mode, the seeded
//! leg, the input bound, every kind of operation reached, the extreme wall, and broken laws caught.
//!
//! - `sequence_fuzz` wraps `geom_fuzz::sequence::harness` in `bolero::check!`; under
//!   `cargo bolero test sequence_fuzz --package geom-fuzz` it is a libFuzzer target.
//! - `sequence_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::holds;

use geom_format::body::write_body;
use geom_fuzz::make::{Motion, Shape, Step};
use geom_fuzz::seeded;
use geom_fuzz::sequence::{
    Case, Entry, Item, MANY_COPIES, MAX_INPUT_BYTES, MAX_OPERATIONS, check, decode, harness, judge,
    reached, replay_line,
};
use geom_make::BoxInput;
use geom_num::{Exactness, P3, Steps};
use geom_run::{Budget, WorkMeter};
use geom_test::{Seed, SplitMix64};
use geom_topo::Builder;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 200;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 300;

/// The longest byte string the seeded leg draws.
const MAX_LEN: u16 = 4_000;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn sequence_fuzz() {
    println!(
        "bolero test mode: corpus and crash files, then {ITERATIONS} random inputs whose seeds \
         bolero draws from the operating system; a failing input's seed is printed as \
         BOLERO_RANDOM_SEED=<seed>, which replays it"
    );
    bolero::check!()
        .with_iterations(ITERATIONS)
        .with_max_len(usize::from(MAX_LEN))
        .for_each(|data: &[u8]| holds(harness(data)));
}

#[test]
fn sequence_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "sequence fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// A fuzzer's bytes must get past the first operation, or the laws of a whole log are never
/// reached: of the seeded inputs, a fair share runs three operations or more.
#[test]
fn seeded_cases_reach_whole_logs() {
    let mut g = SplitMix64::new(Seed::DEFAULT);
    let (mut some, mut long) = (0, 0);
    for _ in 0..200 {
        let data: Vec<u8> = (0..MAX_LEN).map(|_| g.next_u64() as u8).collect();
        let count = reached(&decode(&data).0).unwrap();
        some += usize::from(count >= 1);
        long += usize::from(count >= 3);
    }
    println!("{some} of 200 ran an operation, {long} ran three or more");
    assert!(some >= 100, "{some}");
    assert!(long >= 40, "{long}");
}

#[test]
fn any_bytes_are_a_case() {
    assert_eq!(decode(&[]).0.entries, []);
    holds(harness(&[]));
    holds(harness(&[0xff; 64]));
}

/// The input that reads the most: twelve prisms of eight corners.
fn longest() -> Vec<u8> {
    let mut data = vec![0u8; MAX_INPUT_BYTES + 100];
    data[0] = MAX_OPERATIONS;
    // After the count, the place and the word of the cut, each operation's head is four bytes, its
    // first the rule (0: a primitive), then the choice (1: a prism), and 54 bytes on is its corner
    // count.
    let mut at = 1 + 1 + 8;
    for _ in 0..MAX_OPERATIONS {
        data[at + 4] = 1;
        let corners = at + 4 + 1 + 27 + 27;
        data[corners] = 8;
        at = corners + 1 + 8 * 27;
    }
    data
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    assert_eq!(decode(&longest()).1, MAX_INPUT_BYTES);
    let mut more = longest();
    more.extend_from_slice(&[0xab; 512]);
    assert_eq!(decode(&more).1, MAX_INPUT_BYTES);
}

fn a_box(at: P3) -> Shape {
    Shape::Box(BoxInput {
        min: at,
        size: [Steps(100), Steps(200), Steps(300)],
    })
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn transform(motions: Vec<Motion>, pattern: bool) -> Item {
    Item::Transform(Step {
        motions,
        pattern,
        accept: None,
        work: 0,
    })
}

fn case(items: Vec<(Item, u8)>) -> Case {
    Case {
        entries: items
            .into_iter()
            .map(|(item, on)| Entry { item, on })
            .collect(),
        cut_at: 0,
        cut_work: 40,
    }
}

#[test]
fn a_tree_of_operations_keeps_every_law() {
    let c = case(vec![
        (
            Item::Make {
                shape: a_box(p(0, 0, 0)),
                at: None,
            },
            0,
        ),
        (transform(vec![Motion::Move([5, 0, 0])], false), 0),
        // Starts exactly on a corner of the first box, and one step from another.
        (
            Item::Make {
                shape: a_box(p(9, 9, 9)),
                at: Some((1, [0, 0, 0])),
            },
            0,
        ),
        (
            Item::Make {
                shape: a_box(p(9, 9, 9)),
                at: Some((2, [1, -1, 0])),
            },
            1,
        ),
        // A pattern on the second result, then a transform of one of its copies.
        (
            transform(
                vec![Motion::Move([500, 0, 0]), Motion::Move([900, 0, 0])],
                true,
            ),
            1,
        ),
        (transform(vec![Motion::Move([0, 0, 7])], false), 5),
    ]);
    holds(check(&c));
}

#[test]
fn a_transform_with_nothing_to_take_is_left_out() {
    holds(check(&case(vec![(
        transform(vec![Motion::Move([5, 0, 0])], false),
        0,
    )])));
}

#[test]
fn a_refused_operation_leaves_the_log_alone() {
    let flat = Shape::Box(BoxInput {
        min: p(0, 0, 0),
        size: [Steps(0), Steps(1), Steps(1)],
    });
    holds(check(&case(vec![
        (
            Item::Make {
                shape: a_box(p(0, 0, 0)),
                at: None,
            },
            0,
        ),
        (
            Item::Make {
                shape: flat,
                at: None,
            },
            0,
        ),
        (transform(vec![Motion::Move([5, 0, 0])], false), 0),
    ])));
}

#[test]
fn the_extreme_wall_is_reached() {
    // The second operation's rule: a transform, on the rule that names the wall.
    let mut data = vec![0u8; 2_000];
    data[0] = 2;
    let at = 1 + 1 + 8;
    // A primitive: a box of one-step sizes (each length's rule byte 0).
    data[at + 4] = 0;
    let second = at + 4 + 1 + 27 + 27;
    data[second] = 1;
    let (c, _) = decode(&data);
    let Item::Transform(step) = &c.entries[1].item else {
        panic!("{:?}", c.entries[1]);
    };
    assert!(step.pattern);
    assert_eq!(step.motions.len(), usize::from(MANY_COPIES));
    holds(check(&c));
}

#[test]
fn a_body_the_checker_fails_is_a_finding() {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let made = geom_make::make_box(
        &geom_op::OpContext::new(
            geom_op::OpPath::new(geom_op::OpId::from_bytes([1; 16])),
            Budget::UNLIMITED,
        ),
        &BoxInput {
            min: p(0, 0, 0),
            size: [Steps(1), Steps(1), Steps(1)],
        },
    )
    .unwrap();
    let body = made.result.body;
    assert!(judge(&body, Exactness::Exact).is_ok());
    assert!(write_body(&body, &mut meter).is_ok());
    // Without its names: published plainly from the same arenas.
    let bare = Builder {
        arenas: body.arenas().clone(),
    }
    .publish(body.kind(), body.outside(), &mut meter)
    .unwrap();
    assert_eq!(
        judge(&bare, Exactness::Exact).unwrap_err().to_string(),
        "broken law: a result carries its names\n"
    );
    // A moved point: the faces no longer pass through it.
    let mut arenas = body.arenas().clone();
    arenas
        .points
        .set(geom_topo::Ix::new(0).unwrap(), p(5, 5, 5).into())
        .unwrap();
    let names = body.names().unwrap().clone();
    let broken = Builder { arenas }
        .publish_named(&names, body.kind(), body.outside(), &mut meter)
        .unwrap();
    let got = judge(&broken, Exactness::Exact).unwrap_err().to_string();
    assert!(
        got.contains("every result passes the full checker"),
        "{got}"
    );
}

/// A replay line says what the log made: reference mode's comparison is only as strong as this.
#[test]
fn a_replay_line_follows_the_results() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("replay lines", Seed::DEFAULT));
    let mut lines = std::collections::BTreeSet::new();
    let mut made = 0;
    for _ in 0..200 {
        let mut data = vec![0u8; 300];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        // The same case gives the same line, and a line for every case that made something.
        let line = replay_line(&case);
        assert_eq!(line, replay_line(&case));
        if let Some(line) = line {
            assert!(!line.starts_with("broken"), "{line}");
            lines.insert(line);
            made += 1;
        }
    }
    assert!(made > 100, "{made}");
    assert!(lines.len() > 50, "{}", lines.len());
}
