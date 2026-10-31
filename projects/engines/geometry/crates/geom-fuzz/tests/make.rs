//! The make fuzz target (G101), run as tests on stable Rust: bolero's test mode, the seeded leg, the
//! input bound, and a broken operation caught.
//!
//! - `make_fuzz` wraps `geom_fuzz::make::harness` in `bolero::check!`: the crate's corpus and crash
//!   files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`. Under `cargo bolero test make_fuzz --package geom-fuzz` it is a
//!   libFuzzer target.
//! - `make_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly; both a primitive built and each kind of step are reached;
//!   and an operation that loses a name, gives no history or raises a class is caught.

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

use geom_fuzz::make::{
    Case, MAX_COPIES, MAX_CORNERS, MAX_INPUT_BYTES, MAX_STEPS, Motion, Shape, Step, check,
    check_with, decode, harness,
};
use geom_fuzz::seeded;
use geom_make::{BoxInput, RoundInput};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::history::History;
use geom_op::{NameSet, OpOutput};
use geom_test::{Seed, SplitMix64};
use geom_topo::Builder;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 500;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 1_000;

/// The longest byte string the seeded leg draws: the harness reads at most [`MAX_INPUT_BYTES`];
/// the rest must change nothing.
const MAX_LEN: u16 = 800;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn make_fuzz() {
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
fn make_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "make fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// The input that reads the most: a prism of [`MAX_CORNERS`] corners and [`MAX_STEPS`] patterns of
/// [`MAX_COPIES`] copies.
#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = vec![1]; // a prism
    data.extend_from_slice(&[0; 27 + 27]); // its point and lengths
    data.push(MAX_CORNERS);
    data.extend_from_slice(&[0; 8 * 27]);
    data.push(MAX_STEPS);
    for _ in 0..MAX_STEPS {
        data.push(4); // a pattern
        data.extend_from_slice(&[0; 8]);
        data.push(MAX_COPIES - 1);
        data.extend_from_slice(&[0; 3 * 29]);
    }
    assert_eq!(data.len(), MAX_INPUT_BYTES);
    let (_, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    data.extend_from_slice(&[7; 100]);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    holds(check(&case));
    // The empty input is a case too.
    let (empty, read) = decode(&[]);
    assert_eq!(read, 0);
    holds(check(&empty));
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// A box moved twice and mirrored: every law applies.
fn a_case() -> Case {
    let step = |motion: Motion, pattern: bool| Step {
        motions: vec![motion],
        pattern,
        accept: None,
        work: 1999,
    };
    Case {
        shape: Shape::Box(BoxInput {
            min: p(0, 0, 0),
            size: [Steps(3), Steps(4), Steps(5)],
        }),
        steps: vec![
            step(Motion::Move([1, 2, 3]), false),
            step(
                geom_fuzz::make::Motion::Mirror(geom_math::Axis::X, p(1, 0, 0)),
                false,
            ),
            step(Motion::Move([9, 0, 0]), true),
        ],
    }
}

#[test]
fn a_broken_operation_is_caught() {
    holds(check(&a_case()));
    // Names lost.
    let nameless = |out: OpOutput<geom_make::Made>| {
        let mut made = out.result;
        let mut meter = geom_run::WorkMeter::new(geom_run::Budget::UNLIMITED);
        made.body = Builder::edit(&made.body)
            .publish(made.body.kind(), made.body.outside(), &mut meter)
            .unwrap();
        OpOutput::new(made, out.exactness)
    };
    let finding = check_with(&a_case(), nameless).unwrap_err();
    assert_eq!(finding.law, "a result carries its names");
    // A history with nothing in it.
    let forgotten = |out: OpOutput<geom_make::Made>| {
        let mut made = out.result;
        made.history = History::new([], NameSet::new([]).unwrap(), []).unwrap();
        OpOutput::new(made, out.exactness)
    };
    let finding = check_with(&a_case(), forgotten).unwrap_err();
    assert_eq!(finding.law, "a transform records every entity");
    // A class that rises: a certified cylinder reported Exact after a move.
    let raised = |out: OpOutput<geom_make::Made>| OpOutput::new(out.result, Exactness::Exact);
    let round = Case {
        shape: Shape::Cylinder(RoundInput {
            base: p(0, 0, 0),
            radius: Steps(5),
            height: Steps(10),
            step: GridStep::Q0,
        }),
        steps: vec![Step {
            motions: vec![Motion::Move([1, 0, 0])],
            pattern: false,
            accept: None,
            work: 1999,
        }],
    };
    assert!(check_with(&round, raised).is_err());
}

#[test]
fn most_cases_reach_a_body_and_every_kind_of_step_runs() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("make outcomes", Seed::DEFAULT));
    let mut built = 0;
    let mut stepped = 0;
    for _ in 0..1000 {
        let len = rng.below(std::num::NonZeroU64::new(700).unwrap()) as usize;
        let mut data = vec![0u8; len];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        built += usize::from(check_with(&case, |out| out).is_ok());
        stepped += usize::from(!case.steps.is_empty());
    }
    println!("{built} of 1000 clean; {stepped} with steps");
    assert_eq!(built, 1000);
    assert!(stepped > 500);
}
