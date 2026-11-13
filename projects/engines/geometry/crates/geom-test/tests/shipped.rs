//! The shipped-checker leg (chapters 8.3, 28.4 and 28.13; decision 32.48): what an operation
//! answers never depends on how much of the checker the build runs.
//!
//! A shipped release build checks each result with the cheap groups only (8.3), and 28.4 asks for
//! a leg in that configuration whose hashes equal the full-checker legs'. Until decision 32.48
//! there was none: this crate turned `geom-check`'s `full` feature on for every build that links
//! it, and every kernel crate's tests link it. `kernel.yml` now runs this file and
//! `determinism.rs` in release without this crate's default feature `full-checker`, which is the
//! shipped checker; every other leg runs them with the full one. The assertions are the same in
//! both, so a build that answers differently fails one of them.
//!
//! What this file proves, in the build it runs in:
//!
//! - The build checks at the level its switches say: the cheap groups in release without
//!   `full-checker`, every group otherwise. A dependency that turns the full checker back on
//!   behind the leg's back fails here.
//! - `kernel.yml` runs the leg, in release, on every native target and on wasm32.
//! - Every body the replay corpus makes passes the full checker, and the corpus's digest is the
//!   store's `geom-test/replay-corpus`.
//! - Over every primitive, moved, mirrored, turned and scaled, exactly and with rounding, and
//!   each result transformed again: every body an operation returns passes the full checker
//!   (principle 4), no operation reports a defect, and the numbers of results returned and
//!   refused are the ones written here, which are the full-checker build's.
//! - The cases that were returned unchecked by a shipped build and refused by a test build are
//!   judged by the full set in every build: a box turned 35 degrees (the Workshop hinge), which
//!   the full set passes from G134 and every build returns; a cylinder turned the same way,
//!   refused; and a thin prism halved, whose rounded corners meet, refused.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    reason = "test code: a failure should stop loudly"
)]

use std::collections::BTreeMap;

use geom_check::{Level, check};
use geom_format::operation::Operation;
use geom_format::record::Record;
use geom_make::{
    BoxInput, Made, Moved, PrismInput, RoundInput, SphereInput, TorusInput, cone, cylinder,
    make_box, mirror, move_by, prism, rotate, scale, sphere, torus,
};
use geom_math::{Axis, Rotation, Scale};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{MakeDetail, OpContext, OpError, OpErrorDetail, OpId, OpOutput, OpPath};
use geom_run::{Budget, BudgetKind, WorkMeter};
use geom_test::faults::full_check;
use geom_test::fixture::kernel::Kernel;
use geom_test::fixture::replay::{FAMILIES, digest, seeded};
use geom_test::fixture::repo::{embedded, parse_all};
use geom_test::fixture::run::{Engine, Ran, replay};
use geom_test::fixture::{Fixture, Measures};
use geom_test::golden::Store;

/// Whether this build checks each result as a shipped build does: in release, with nothing
/// turning the full checker on.
const SHIPPED: bool = !cfg!(any(debug_assertions, feature = "full-checker"));

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([11; 16])), Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn s(num: u64, den: u64) -> Scale {
    Scale::new(num, den).unwrap()
}

/// `out` to transform, any bound accepted in its class.
fn moved(out: &OpOutput<Made>) -> Moved<'_> {
    let wide = Steps(1 << 40);
    let accept = match out.exactness {
        Exactness::Approximate { step, .. } => Exactness::Approximate {
            deviation: wide,
            step,
        },
        _ => Exactness::Certified {
            bound: wide,
            step: GridStep::Q0,
        },
    };
    Moved {
        body: &out.result.body,
        exactness: out.exactness,
        accept: Some(accept),
    }
}

/// 35 degrees to four places: cos 35° = 0.8192, sin 35° = 0.5736. Not a Pythagorean direction,
/// so the rotation is held in doubles and every corner rounds.
fn thirty_five_degrees() -> Rotation {
    Rotation::direction(8192, 5736).unwrap()
}

/// The piece a refusal waits for, if `e` is the typed refusal of a body the checker cannot
/// certify yet.
fn waits_for(e: &OpError) -> Option<&'static str> {
    match e.detail() {
        OpErrorDetail::Make(MakeDetail::NotCertifiedYet { piece }) => Some(piece),
        _ => None,
    }
}

/// The error `what` was refused with. A body returned instead is the failure this file is for.
fn refused(what: &str, result: Result<OpOutput<Made>, OpError>) -> OpError {
    match result {
        Ok(out) => panic!(
            "{what}: returned as {:?}; every build must refuse it",
            out.exactness
        ),
        Err(e) => e,
    }
}

#[test]
fn the_build_checks_at_the_level_its_switches_say() {
    let expected = if SHIPPED { Level::Cheap } else { Level::Full };
    assert_eq!(
        Level::of_build(),
        expected,
        "release without the full-checker feature is the shipped checker, and nothing else is"
    );
}

/// The leg's command, as `kernel.yml` must run it natively and for wasm32.
const LEG: &str = "cargo test --locked --release -p geom-test --no-default-features --test shipped --test determinism";

/// The workflow is read as YAML, as `kernel_quality_workflow.rs` reads its own, so the check sees
/// the steps GitHub sees. WebAssembly test runs read no files of the repository.
#[cfg(not(target_family = "wasm"))]
#[test]
fn kernel_yml_runs_the_shipped_checker_leg_natively_and_on_wasm32() {
    use yaml_rust2::YamlLoader;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap();
    let text = std::fs::read_to_string(root.join(".github/workflows/kernel.yml")).unwrap();
    let workflow = YamlLoader::load_from_str(&text).unwrap().remove(0);
    let mut legs = Vec::new();
    for (job, body) in workflow["jobs"].as_hash().unwrap() {
        let job = job.as_str().unwrap();
        for step in body["steps"].as_vec().into_iter().flatten() {
            let Some(run) = step["run"].as_str().filter(|run| run.starts_with(LEG)) else {
                continue;
            };
            // A shipped build sets no flags: overflow checks off, the target's own baseline.
            assert!(
                step["env"].is_badvalue() && body["env"]["RUSTFLAGS"].is_badvalue(),
                "{job}: the shipped-checker leg builds as shipped, with no environment of its own"
            );
            // It runs on every leg of its job's matrix that builds in release.
            let when = step["if"].as_str().unwrap_or_default();
            assert!(
                when.is_empty()
                    || when == "${{ !cancelled() }}"
                    || when == "${{ !cancelled() && matrix.mode == 'release' }}",
                "{job}: {when}"
            );
            legs.push((job.to_owned(), run.trim().to_owned()));
        }
    }
    assert_eq!(
        legs,
        [
            ("test".to_owned(), LEG.to_owned()),
            ("wasm".to_owned(), format!("{LEG} --target wasm32-wasip1")),
        ],
        "kernel.yml must run the shipped-checker leg in its jobs `test` and `wasm`"
    );
}

/// The kernel, with every body an operation returns checked in full as it comes back.
#[derive(Default)]
struct Watched {
    kernel: Kernel,
    returned: u64,
    failed: Vec<String>,
}

impl Engine for Watched {
    fn start(&mut self, heal: Steps, inputs: &[Record]) -> Result<(), String> {
        self.kernel.start(heal, inputs)
    }

    fn run(&mut self, operation: &Operation) -> Ran {
        let ran = self.kernel.run(operation);
        if let Ran::Done(step) = &ran {
            for body in self.kernel.last() {
                self.returned += 1;
                let report = full_check(body, step.exactness).unwrap();
                if !report.passed() {
                    self.failed
                        .push(format!("{}: {:?}", operation.name, report.faults().first()));
                }
            }
        }
        ran
    }

    fn measures(&mut self) -> Measures {
        self.kernel.measures()
    }
}

fn stored() -> Vec<Fixture> {
    parse_all(&embedded()).unwrap()
}

#[test]
fn every_body_the_replay_corpus_makes_passes_the_full_checker() {
    let fixtures = stored();
    let mut watched = Watched::default();
    for fixture in &fixtures {
        let _ = replay(&mut watched, &fixture.log);
    }
    for family in FAMILIES {
        for log in seeded(family).unwrap().unwrap() {
            let _ = replay(&mut watched, &log);
        }
    }
    assert!(watched.failed.is_empty(), "{:#?}", watched.failed);
    // Every primitive of the make family and the transform family's four bodies at least.
    assert!(watched.returned >= 28, "{}", watched.returned);
}

#[test]
fn the_replay_corpus_gives_the_stores_digest() {
    let got = digest(&stored(), &mut Kernel::new()).unwrap();
    let want = Store::embedded()
        .unwrap()
        .expected("geom-test/replay-corpus")
        .unwrap();
    assert_eq!(got, want.0);
}

/// Every primitive at three sizes and places, as the fault-injection catalogue's valid bodies
/// are built (`attack_g110.rs`).
fn primitives() -> Vec<(String, OpOutput<Made>)> {
    let c = ctx();
    let mut all = Vec::new();
    for (i, at) in [p(0, 0, 0), p(-40, 17, 3), p(1 << 30, -(1 << 30), 5)]
        .into_iter()
        .enumerate()
    {
        let [ax, ay, az] = [at.x.get(), at.y.get(), at.z.get()];
        let base = vec![
            p(ax, ay, az),
            p(ax + 10, ay, az),
            p(ax + 12, ay + 7, az),
            p(ax + 3, ay + 11, az),
        ];
        let round = RoundInput {
            base: at,
            radius: Steps(5 << i),
            height: Steps(10 << i),
            step: GridStep::Q0,
        };
        let ball = SphereInput {
            center: at,
            radius: Steps(5 << i),
            step: GridStep::Q0,
        };
        let ring = TorusInput {
            center: at,
            major: Steps(10 << i),
            minor: Steps(3 << i),
            step: GridStep::Q0,
        };
        let size = [Steps(4 << i), Steps(6 << i), Steps(8 << i)];
        let height = Steps(9 << i);
        all.push((
            format!("box {i}"),
            make_box(&c, &BoxInput { min: at, size }).unwrap(),
        ));
        all.push((
            format!("prism {i}"),
            prism(&c, &PrismInput { base, height }).unwrap(),
        ));
        all.push((format!("cylinder {i}"), cylinder(&c, &round).unwrap()));
        all.push((format!("cone {i}"), cone(&c, &round).unwrap()));
        all.push((format!("sphere {i}"), sphere(&c, &ball).unwrap()));
        all.push((format!("torus {i}"), torus(&c, &ring).unwrap()));
    }
    all
}

/// Every transform of the sweep on `out`: the exact ones of the fault-injection catalogue, then
/// those that round a corner or move a plane or a curve by a deviation.
fn transforms(out: &OpOutput<Made>) -> Vec<(&'static str, Result<OpOutput<Made>, OpError>)> {
    let c = ctx();
    let at = p(1, 2, 3);
    let input = moved(out);
    vec![
        ("moved", move_by(&c, input, [3, -4, 5])),
        ("mirrored", mirror(&c, input, Axis::X, at)),
        (
            "quarter turned",
            rotate(&c, input, Axis::Z, Rotation::quarter_turns(1), at),
        ),
        ("doubled", scale(&c, input, [s(2, 1); 3], at)),
        ("tripled", scale(&c, input, [s(3, 1); 3], at)),
        (
            "turned 35 degrees",
            rotate(&c, input, Axis::Z, thirty_five_degrees(), at),
        ),
        (
            "turned 3-4-5",
            rotate(&c, input, Axis::X, Rotation::direction(4, 3).unwrap(), at),
        ),
        ("by 3/2", scale(&c, input, [s(3, 2); 3], at)),
        ("halved", scale(&c, input, [s(1, 2); 3], at)),
        (
            "by 7/3 along y",
            scale(&c, input, [s(1, 1), s(7, 3), s(1, 1)], at),
        ),
    ]
}

/// What the sweep's operations answered, by answer.
#[derive(Debug, Default, PartialEq, Eq)]
struct Tally {
    /// Bodies returned, each of which passed the full checker.
    returned: u64,
    /// Refusals that wait for a piece, by the piece.
    waits: BTreeMap<&'static str, u64>,
    /// Refusals that are not the checker's, which no level touches: a scale along one axis of a
    /// curved body, for one.
    refused: u64,
}

impl Tally {
    /// Counts `result`, the answer to `what`, and returns the body if one came back.
    fn count(
        &mut self,
        what: &str,
        result: Result<OpOutput<Made>, OpError>,
    ) -> Option<OpOutput<Made>> {
        match result {
            Ok(out) => {
                let report = full_check(&out.result.body, out.exactness).unwrap();
                assert!(
                    report.passed(),
                    "{what}: returned, and the full checker does not pass it: {:?}",
                    report.faults().first()
                );
                self.returned += 1;
                Some(out)
            }
            Err(e) => {
                assert_ne!(
                    *e.detail(),
                    OpErrorDetail::Make(MakeDetail::ResultInvalid),
                    "{what}: a defect: {}",
                    e.message()
                );
                match waits_for(&e) {
                    Some(piece) => *self.waits.entry(piece).or_default() += 1,
                    None => self.refused += 1,
                }
                None
            }
        }
    }
}

#[test]
fn every_returned_body_passes_the_full_checker_and_the_answers_are_the_full_builds() {
    let mut tally = Tally::default();
    for (name, base) in primitives() {
        for (how, first) in transforms(&base) {
            let what = format!("{name} {how}");
            let Some(first) = tally.count(&what, first) else {
                continue;
            };
            for (then, second) in transforms(&first) {
                tally.count(&format!("{what}, then {then}"), second);
            }
        }
    }
    // The full-checker build's answers. A shipped build that gives others answers differently
    // by build, which principle 5 and 28.4 forbid; one that returns a body the full checker does
    // not pass has already failed above.
    // Before G134 the checker certified no plane off the integers: of 1590 operations 1263
    // bodies were returned and 123 waited for it. With it a rounded flat face stands for its
    // star of triangles and those 123 are returned. Fifteen of them are a primitive's first
    // transform, so each is transformed again ten ways: 150 operations more, every one returned.
    let expected = Tally {
        returned: 1536,
        waits: [("G177", 96)].into(),
        refused: 108,
    };
    assert_eq!(tally, expected);
}

/// The work units a check of `out` charges at `level` (23.10: one per row or pair visited).
fn units(out: &OpOutput<Made>, level: Level) -> u128 {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    check(&out.result.body, out.exactness, level, &mut meter).unwrap();
    meter.used().get(BudgetKind::Work)
}

/// What decision 32.48 costs a shipped build, in the checker's own work units, the same on every
/// machine: the full set in place of the cheap groups on a result that is less than exact. The
/// record cites these numbers, as they stand with G134's check of flat faces (its note "With
/// G134"); the ones it measured before that are in its table.
#[test]
fn the_full_set_on_a_result_less_than_exact_costs_what_the_record_says() {
    let c = ctx();
    let (mut bodies, mut cheap, mut full) = (0_u64, 0_u128, 0_u128);
    // The largest of each kind: its cheap groups' units and the full set's.
    let mut by_kind: BTreeMap<String, (u128, u128)> = BTreeMap::new();
    for (name, base) in primitives() {
        // Each primitive as made, and moved: a round primitive is less than exact as made, and a
        // flat one once a scale by 3/2 has rounded it.
        let input = moved(&base);
        let results = [
            move_by(&c, input, [3, -4, 5]),
            scale(&c, input, [s(3, 2); 3], p(1, 2, 3)),
        ];
        for out in results.into_iter().flatten() {
            if matches!(out.exactness, Exactness::Exact) {
                continue;
            }
            let (a, b) = (units(&out, Level::Cheap), units(&out, Level::Full));
            bodies += 1;
            cheap += a;
            full += b;
            let kind = name.split(' ').next().unwrap().to_owned();
            let entry = by_kind.entry(kind).or_default();
            *entry = (entry.0.max(a), entry.1.max(b));
        }
    }
    // Thirty bodies: twelve round primitives moved and all eighteen primitives scaled by 3/2.
    let expected: BTreeMap<String, (u128, u128)> = [
        ("box", (227, 522)),
        ("cone", (27, 46)),
        ("cylinder", (48, 80)),
        ("prism", (227, 522)),
        ("sphere", (6, 12)),
        ("torus", (40, 54)),
    ]
    .into_iter()
    .map(|(kind, units)| (kind.to_owned(), units))
    .collect();
    assert_eq!(by_kind, expected);
    assert_eq!((bodies, cheap, full), (30, 2088, 4275));
}

#[test]
fn what_a_shipped_build_used_to_return_unchecked_is_judged_in_full_in_every_build() {
    let c = ctx();
    let origin = p(0, 0, 0);
    // The Workshop hinge: a box turned 35 degrees. Its corners round and its planes leave the
    // integers. Before G134 the checker could not certify that, so a test build refused the
    // turn and a shipped build returned it unchecked. From G134 each of its faces stands for
    // its star of triangles, the full set passes it, and every build returns it on that
    // verdict, recorded within the two steps its corners rounded by.
    let leaf = make_box(
        &c,
        &BoxInput {
            min: origin,
            size: [Steps(6400), Steps(3200), Steps(1600)],
        },
    )
    .unwrap();
    let turned = rotate(&c, moved(&leaf), Axis::Z, thirty_five_degrees(), origin)
        .unwrap_or_else(|e| panic!("a box turned 35 degrees: {}", e.message()));
    let within_two = Exactness::Certified {
        bound: Steps(2),
        step: GridStep::Q0,
    };
    assert_eq!(turned.exactness, within_two);
    let report = full_check(&turned.result.body, turned.exactness).unwrap();
    assert!(report.passed(), "{:?}", report.faults().first());
    // The same turn of a cylinder waits for the curved checker, G177.
    let pin = cylinder(
        &c,
        &RoundInput {
            base: origin,
            radius: Steps(800),
            height: Steps(3200),
            step: GridStep::Q0,
        },
    )
    .unwrap();
    let turned = rotate(&c, moved(&pin), Axis::X, thirty_five_degrees(), origin);
    let e = refused("a cylinder turned 35 degrees", turned);
    assert_eq!(waits_for(&e), Some("G177"), "{}", e.message());
    // A thin prism halved: its corner at (0, 1) rounds onto the one at (0, 0). That body is not
    // only uncertified, it is wrong, and only groups 2 and 3 can see it.
    let base = vec![p(0, 0, 0), p(2, 0, 0), p(0, 1, 0)];
    let thin = prism(
        &c,
        &PrismInput {
            base,
            height: Steps(2),
        },
    )
    .unwrap();
    let halved = scale(&c, moved(&thin), [s(1, 2); 3], origin);
    let e = refused("a thin prism halved", halved);
    assert_eq!(waits_for(&e), Some("G417"), "{}", e.message());
    assert!(
        e.message().contains("two edges of the face meet"),
        "{}",
        e.message()
    );
}
