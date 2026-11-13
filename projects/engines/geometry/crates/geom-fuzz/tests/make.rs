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
//! - The target's first two findings, cut to their smallest cases, hold the laws (28.16): an exact
//!   scale of a body an earlier scale rounded, which the checker cannot certify, is refused with a
//!   type. Their inputs as the fuzzer wrote them are in the target's crash folder.

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
    Answers, Case, MAX_COPIES, MAX_CORNERS, MAX_INPUT_BYTES, MAX_STEPS, Motion, Shape, Step,
    answers, answers_hold, check, check_with, decode, harness,
};
use geom_fuzz::seeded;
use geom_make::{BoxInput, RoundInput};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::history::History;
use geom_op::{NameSet, OpContext, OpId, OpOutput, OpPath};
use geom_run::Budget;
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

/// A wrong query, as a change to the real queries' answers.
type Breaks = dyn Fn(&mut Answers);

fn query_context() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([4; 16])), Budget::UNLIMITED)
}

fn flat_box() -> OpOutput<geom_make::Made> {
    let input = BoxInput {
        min: p(-3, 1, 2),
        size: [Steps(3), Steps(4), Steps(5)],
    };
    geom_make::make_box(&query_context(), &input).unwrap()
}

fn round_body() -> OpOutput<geom_make::Made> {
    let input = RoundInput {
        base: p(0, 0, 0),
        radius: Steps(5),
        height: Steps(10),
        step: GridStep::Q0,
    };
    geom_make::cylinder(&query_context(), &input).unwrap()
}

/// A query error a valid body never gives.
fn malformed() -> geom_op::OpError {
    geom_op::OpError::new(
        OpPath::new(OpId::from_bytes([4; 16])),
        geom_op::detail::QueryDetail::Malformed,
    )
}

/// The law `edit` breaks in the answers on `out`.
fn broken_law(out: &OpOutput<geom_make::Made>, edit: &Breaks) -> String {
    let mut broken = answers(&query_context(), out);
    edit(&mut broken);
    answers_hold(out, &broken).unwrap_err().law
}

#[test]
fn a_curved_body_whose_volume_is_answered_is_caught() {
    let round = round_body();
    assert_eq!(
        answers_hold(&round, &answers(&query_context(), &round)),
        Ok(())
    );
    let answered = |a: &mut Answers| {
        let class = a.exactness.as_ref().unwrap().exactness;
        a.volume = Ok(OpOutput::new(geom_num::wide::I256::from_i64(6), class));
    };
    assert_eq!(
        broken_law(&round, &answered),
        "query.volume refuses a body with a curved face or edge"
    );
}

#[test]
fn a_broken_query_is_caught() {
    let flat = flat_box();
    assert_eq!(
        answers_hold(&flat, &answers(&query_context(), &flat)),
        Ok(())
    );
    let cases: [(&str, &Breaks); 8] = [
        ("a query states its body's class", &|a| {
            if let Ok(v) = &mut a.volume {
                v.exactness = Exactness::Approximate {
                    deviation: Steps(1),
                    step: GridStep::Q0,
                };
            }
        }),
        ("query.validity is the checker's report", &|a| {
            a.validity[1] = a.validity[0].clone();
        }),
        (
            "query.exactness reports the record, the geometry and the checker's judgement",
            &|a| {
                if let Ok(r) = &mut a.exactness {
                    r.result.flat_faces -= 1;
                    r.result.curved_faces += 1;
                }
            },
        ),
        (
            "query.faces lists the body's faces by name, in arena order",
            &|a| {
                if let Ok(f) = &mut a.faces {
                    f.result.swap(0, 1);
                }
            },
        ),
        (
            "the box holds every vertex, tight exactly when every face is flat",
            &|a| {
                if let Ok(o) = &mut a.bounding_box {
                    if let Some(b) = &mut o.result {
                        b.tight = false;
                    }
                }
            },
        ),
        ("a closed shell's area vectors sum to zero", &|a| {
            if let Some(Ok(f)) = a.areas.first_mut() {
                f.result[2] += 2;
            }
        }),
        (
            "query.volume is six times the volume its loops enclose",
            &|a| {
                if let Ok(v) = &mut a.volume {
                    v.result = geom_num::wide::I256::from_i64(1);
                }
            },
        ),
        (
            "a query refuses a valid body only for a curved face or edge",
            &|a| a.faces = Err(malformed()),
        ),
    ];
    for (name, edit) in cases {
        assert_eq!(broken_law(&flat, edit), name);
    }
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

/// A prism on `base`, `height` high, and `steps` on it.
fn prism_case(base: &[(i64, i64, i64)], height: u64, steps: Vec<Step>) -> Case {
    Case {
        shape: Shape::Prism(geom_make::PrismInput {
            base: base.iter().map(|&(x, y, z)| p(x, y, z)).collect(),
            height: Steps(height),
        }),
        steps,
    }
}

/// A scale by `factors` about `center`, any bound up to `accept` accepted.
fn scaled(factors: [(u64, u64); 3], center: P3, accept: u64, pattern: bool) -> Step {
    Step {
        motions: vec![Motion::Scale(factors, center)],
        pattern,
        accept: Some(accept),
        work: 787,
    }
}

/// The first finding (crash-dc639a8b7a2af6c90c4a6c108eb2ea4930e40338), cut to four corners and
/// three steps: two scales by 5/2 round the prism's corners and leave its planes off the
/// integers; a pattern's exact scale by 5 then gives a body whose two parallel sides the checker
/// cannot certify apart. That was reported as a defect.
#[test]
fn an_exact_scale_of_a_twice_rounded_prism_is_refused_with_a_type() {
    let base = [(-40, -11, 1), (-39, -10, 1), (-38, -7, 1), (-37, -2, 1)];
    let center = p(50_368_000, 50_368_000, 50_368_000);
    let half = |pattern| scaled([(5, 2); 3], center, 1 << 30, pattern);
    let five = |pattern| scaled([(5, 1); 3], p(1_241_601, 0, 0), 64, pattern);
    for pattern in [true, false] {
        let steps = vec![half(false), half(false), five(pattern)];
        holds(check(&prism_case(&base, 41_472_000, steps)));
    }
}

/// The second finding (crash-5c649c78ec4ac8d4244d360b9e70380a31d7fa56), cut to its two steps: a
/// scale by (2, 7/3, 1) rounds a triangle prism's corners; a scale by (3, 1, 1), which is exact,
/// then gives a body whose caps the checker cannot certify apart.
#[test]
fn an_exact_scale_of_a_rounded_triangle_is_refused_with_a_type() {
    let base = [
        (4094, 2048, 64_000),
        (4095, 2049, 64_000),
        (4096, 2052, 64_000),
    ];
    let steps = vec![
        scaled(
            [(4, 2), (7, 3), (2, 2)],
            p(-12_799, 4_518_401, -22),
            64,
            false,
        ),
        scaled([(6, 2), (2, 2), (1, 1)], p(2_880_001, 1, 0), 64, false),
    ];
    holds(check(&prism_case(&base, 41_472_000, steps)));
}
