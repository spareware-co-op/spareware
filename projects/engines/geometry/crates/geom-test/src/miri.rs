//! The Miri run's scope, every kernel crate, whose tests Miri interprets; its table, which says of
//! each test whether Miri finishes it and how long it took; and its partitions, balanced by the
//! times of the tests Miri finishes.
//!
//! What it does: [`packages`] reads `cargo metadata` and returns the package of every kernel crate,
//! in name order, and [`cargo_args`] writes them as the `-p` arguments the workflow
//! (`.github/workflows/kernel-quality.yml`) gives `cargo miri nextest run` and `cargo miri test
//! --doc`. [`parse_table`] reads [`TIMES`] (`miri-times.tsv`): one row a test, either the seconds
//! Miri took to finish it or a native-only mark with what was measured (32.58). [`write_table`]
//! is the only writer of those rows: it reads a measuring run's nextest report (`junit.xml`)
//! ([`read_junit`]) and gives each test it names the state the report shows (the
//! `kernel-quality miri-times` command). [`pack`] packs tests into parts by their times,
//! [`gate_plan`] gives the gate its partitions and geom-pool's tests their own part, and
//! [`Plan::filterset`] writes partition k's tests as the nextest filterset its job runs;
//! [`unlisted`] names every test of a nextest list ([`listed_tests`]) the table has no row for,
//! and the workflow's plan job (`kernel-quality miri-plan`) fails on any. [`to_measure`] picks
//! the tests a measuring run on GitHub's runners measures (the workflow's measuring mode: the tests with no
//! row, or every test), each planned at its finishing row's seconds or at the stop, and
//! [`measure_plan`] packs them into the fewest parts that each end within the stop.
//!
//! Guarantees:
//!
//! - **Every test Miri finishes, and only those.** Each kernel test has a row. A test Miri finishes
//!   within the measuring run's stop has its measured seconds, always under [`BUDGET`]; a test
//!   measured over it is native-only, with its measured time or the stop it ran past, and its
//!   partition's filterset leaves it out, so it runs in every native suite and not under Miri. A
//!   test with no row fails the plan job ([`unlisted`]): nothing runs under Miri by default,
//!   and nothing is left out without a measurement. A test that cannot run under Miri at all (one
//!   that reads the repository, or allocates tens of mebibytes) says so where it is written, with
//!   `#[cfg_attr(miri, ignore = "<why>")]`, and needs no row. This rests on every kernel crate
//!   forbidding unsafe code in its manifest (engine-geometry's scaffold check, rule `UnsafeNotForbidden`):
//!   Miri cannot find a fault in the kernel's own code, only in the unsafe code of the libraries
//!   it drives and of the standard library, so [`UNSAFE_LIBRARIES`] names, for each library with
//!   unsafe code the kernel uses, a finishing test that drives it.
//! - **The table is measured, never edited.** [`parse_table`] refuses a row at or over the budget
//!   (such a test was stopped with no verdict, or never timed), a native-only mark with no measured
//!   time, a native-only mark of a time under the budget (Miri finished it), a test in both states
//!   and a test listed twice; `kernel_quality_workflow.rs` holds the file to the form
//!   [`write_table`] writes. A report that shows a test failing under Miri, or killed, is refused
//!   with every such test named, so a fault Miri found is read, never written down as slow.
//! - **Every finishing test in exactly one partition.** A partition's filterset names its tests
//!   one by one, so every finishing test runs once, and native-only tests in none.
//! - **Each Miri job within 15 minutes, on a cold cache and a runner 1.6 times slower.** A gate
//!   job plans each finishing test at [`planned`] seconds: its row's seconds on a runner
//!   [`SLOWER`], and never over [`BUDGET`], where nextest stops it. With [`AT_ONCE`] tests at a
//!   time, [`Plan::part_seconds`] is the most a partition's tests can take in whatever order
//!   nextest starts them, and nextest lists each of its binaries under Miri first,
//!   [`LIST_SECONDS`] a binary, on that runner [`SLOWER`] too. [`gate_plan`] packs the finishing
//!   tests first fit decreasing into partitions whose listing and tests each end within
//!   [`PART_SECONDS`], what a job has left after [`FIXED_SECONDS`]; any finishing test, under
//!   [`BUDGET`], fits one alone with its listing, so none is left out for want of a partition.
//!   geom-pool's finishing tests, which run under Tree
//!   Borrows, are one part of their own, held to the same rule. `kernel_quality_workflow.rs` holds
//!   the table's plan to all of it.
//! - **A test nobody saw finish is not a pass (32.68).** The gate's profiles count a test nextest
//!   stops at [`BUDGET`] as passing, so [`stops`] reads each gate job's report for the tests
//!   stopped there and the job names them in its summary as stopped with no verdict. Only a test
//!   whose row is over [`SURE_WITHIN`], which a runner [`SLOWER`] can take to the stop, may be:
//!   [`MAY_STOP`] lists them, written from the table ([`render_may_stop`]) and held to at most
//!   [`MAY_STOP_MOST`]; any other test stopped fails its job ([`allowed_stops`]).
//! - **A measuring part ends within the stop.** Each test of a measuring run is planned at its
//!   finishing row's seconds, or at [`BUDGET`], the stop of a measuring run on a CI runner, when
//!   its row is native-only or it has none, so a part's tests take at most [`BUDGET`] at
//!   [`AT_ONCE`] a time; a run of more parts than the workflow's two part jobs hold
//!   ([`crate::mutants::MAX_SHARDS`]) is refused, to be narrowed by a filterset. Every test picked
//!   is in exactly one part.
//! - **The development crates are left out** (`geom-test`, `geom-fuzz`, `geom-gen`,
//!   `geom-bench`): they never ship, and their tests start processes and read files, which Miri's
//!   isolation refuses.
//! - **A plan that runs nothing does not pass.** Metadata without any kernel crate, or with a
//!   package that has no name or manifest path, is an error; so are a table row that is not
//!   `state<TAB>binary<TAB>test`, a name a filterset could misread, and a plan of no partitions.
//!
//! Method: the `packages[]` of `cargo metadata --format-version 1`
//! (<https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>); a kernel crate is a package
//! whose manifest is in the kernel's crates folder, as [`crate::deps`] defines it. nextest's Miri
//! support, filtersets, list output and `junit.xml` report are documented at
//! <https://nexte.st/docs/integrations/miri/>, <https://nexte.st/docs/filtersets/>,
//! <https://nexte.st/docs/listing/> and <https://nexte.st/docs/machine-readable/junit/>; the report
//! is read with roxmltree. A partition's bound is that of any list schedule, from R. L. Graham,
//! "Bounds for certain multiprocessing anomalies", Bell System Technical Journal 45(9) (1966),
//! 1563–1581: the last test to end started no later than the others' total over the slots, so a
//! partition ends within that and its longest test. The parts are packed first fit decreasing
//! (D. S. Johnson, A. Demers, J. D. Ullman, M. R. Garey and R. L. Graham, "Worst-case performance
//! bounds for simple one-dimensional packing algorithms", SIAM Journal on Computing 3(4) (1974),
//! 299–325, <https://doi.org/10.1137/0203025>), each test, the longest first, into the first part
//! it fits.
//!
//! Cost: linear in the size of the metadata, the table, the list and the report; a plan is
//! O(t log t + t·p log b) for t tests, p parts and b binaries a part, and a filterset linear in its partition's tests.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::deps::{DEVELOPMENT_CRATES, KERNEL_CRATES_DIR};

/// Miri's table: `state<TAB>binary id<TAB>test name` a line, `#` lines comments (32.58).
pub const TIMES: &str = include_str!("miri-times.tsv");

/// The table's path from the repository root, which `kernel-quality miri-times` rewrites.
pub const TABLE_PATH: &str = "projects/engines/geometry/crates/geom-test/src/miri-times.tsv";

/// The tests a partition runs at once: nextest's default of one per core, and a GitHub-hosted
/// Linux runner has four.
pub const AT_ONCE: u64 = 4;

/// The longest a Miri job may take, in seconds: 15 minutes (the owner's testing principle 8), its
/// set-up counted.
pub const JOB_SECONDS: u64 = 900;

/// How much slower than the table's runners a runner may be, as a fraction (num, den): 1.6 times.
/// A partition is sized so that its job ends within [`JOB_SECONDS`] on such a runner. Run
/// 44075531453 ran its partitions' tests at 0.84 to 1.59 times the table's seconds, the same
/// tests at the same concurrency: GitHub's runners vary that much.
pub const SLOWER: (u64, u64) = (8, 5);

/// What a partition's job costs before nextest lists its tests, in seconds, with a cold cache:
/// checkout, both toolchains, Miri's sysroot and the cache's restore (44 to 60 s, main's runs
/// 43973938568 and 44000736866), the kernel built under Miri (44 to 47 s), the plan's hand-over
/// downloaded and its cargo-nextest put in place (a few seconds), and the upload and the cache's
/// post step (about 6 s). The plan job (`miri-plan`) lists every test under Miri once, and builds
/// cargo-nextest and this tool once, so no partition does. Most of it is downloads, which a slow
/// runner does not stretch: run 44075531453's partition 64, whose tests ran 1.59 times the
/// table's seconds, spent 53 s before its test step and 2 s on its upload.
pub const FIXED_SECONDS: u64 = 120;

/// What nextest's listing of a partition's tests costs for each test binary it runs tests of, in
/// seconds: each binary is run under Miri to list its tests before the first test starts (main's
/// run 44000736866: 20 to 50 s for 43 to 55 binaries, at most 0.97 s a binary).
pub const LIST_SECONDS: u64 = 1;

/// The longest a partition's listing and tests may take, in seconds, on a runner [`SLOWER`]: what
/// is left of [`JOB_SECONDS`] after [`FIXED_SECONDS`], so `120 + 780 = 900`; its tests take at
/// most [`Plan::part_seconds`] of their [`planned`] seconds at [`AT_ONCE`] at a time, its listing
/// [`LIST_SECONDS`] a binary on that runner. It is not [`BUDGET`], the longest a test may take
/// and still finish under Miri: the two are two constants, and [`BUDGET`] with one binary's
/// listing is within this, so a test that finishes always has a partition that holds it and never
/// leaves Miri for want of one (`kernel_quality_workflow.rs` and this module's tests hold the
/// arithmetic).
pub const PART_SECONDS: u64 = 780;

/// The budget of one test under Miri, in seconds: the `default-miri` profile of
/// `.config/nextest.toml` stops a test still running then. A finishing row is always under it.
pub const BUDGET: u64 = 600;

/// The limit of a partition's tests' step, in minutes: over [`PART_SECONDS`], so a partition the
/// plan holds is never stopped by it, and under the job's 30 minutes by [`FIXED_SECONDS`] and the
/// first try of its report's upload, so a partition that overruns fails in that step with its
/// log, the summary of its stopped tests and its report kept (run 44168015168's partition 65 ran
/// out the job's limit inside the step, and nothing of it was kept).
pub const TESTS_STEP_MINUTES: u64 = 24;

/// The seconds after which a Miri gate job's tests' step stops nextest with SIGTERM (coreutils'
/// `timeout`), a minute before [`TESTS_STEP_MINUTES`]: nextest then ends its tests and writes its
/// report. GitHub's own stop at the step's limit ends nextest before it writes one (trial run
/// 44290072805 found no report), so without this an overrun kept its log and lost its report.
pub const TESTS_RUN_SECONDS: u64 = TESTS_STEP_MINUTES * 60 - 60;

/// `seconds` on a runner [`SLOWER`], rounded up.
fn on_a_slower_runner(seconds: u64) -> u64 {
    seconds.saturating_mul(SLOWER.0).div_ceil(SLOWER.1)
}

/// The seconds a gate job plans a finishing test of `seconds` at: on a runner [`SLOWER`], rounded
/// up, and never over [`BUDGET`], since nextest stops a test there whatever the runner.
#[must_use]
pub fn planned(seconds: u64) -> u64 {
    on_a_slower_runner(seconds).min(BUDGET)
}

/// The most seconds a finishing row may hold and its test still be planned to end within
/// [`BUDGET`] on a runner [`SLOWER`]: 600 s over 1.6, 375 s (32.68). A test whose row is over it
/// can reach the stop on a slow runner, so only such a test may be stopped there with no verdict,
/// and only while it is on [`MAY_STOP`].
pub const SURE_WITHIN: u64 = 375;

/// The yardstick (32.78): one fixed small test every Miri job runs alone under Miri before its
/// own tests, as (binary id, test). Its row in [`TIMES`] is its time on the runner the table's
/// seconds are counted in, so the job's [`job_scale`] is the speed of the runner it got, and each
/// test's stop ([`stop_ms`]) is counted in that runner's own seconds. GitHub's runners come in
/// two speeds (run 44248913106's partition 33 ran 1.4 times its proving run's seconds; a row
/// measured on the fast one stopped its test on the slow one), and a stop fixed in one runner's
/// seconds goes red by the luck of the runner.
pub const YARDSTICK: (&str, &str) = (
    "geom-num",
    "plane::tests::the_largest_denominators_of_both_widths",
);

/// The least a job's scale is held at, as a fraction (num, den): 0.5. A faster runner is judged
/// as one at 0.5, so a stop never falls under half its row's planned time.
pub const SCALE_LEAST: (u64, u64) = (1, 2);

/// The most a job's scale may be, as a fraction (num, den): 3. Over it the job fails as a runner
/// too slow to judge, the runner's red and not a test's.
pub const SCALE_MOST: (u64, u64) = (3, 1);

/// Whether `a` is less than `b`, two fractions (num, den) with nonzero denominators.
fn less((an, ad): (u64, u64), (bn, bd): (u64, u64)) -> bool {
    // Two u64s' product always fits a u128, so neither side saturates.
    u128::from(an).saturating_mul(u128::from(bd)) < u128::from(bn).saturating_mul(u128::from(ad))
}

/// A job's scale from its yardstick's run (32.78): the yardstick's seconds in `report` (its run's
/// nextest report) over its row in `rows`, as a fraction (num, den), held at [`SCALE_LEAST`] from
/// below.
///
/// # Errors
///
/// When the yardstick has no finishing row in `rows`, or `report` does not show it passing: a
/// missing time is an error, never a default. When the scale is over [`SCALE_MOST`]: the runner
/// is too slow to judge.
pub fn job_scale(rows: &[Row], report: &[Reported]) -> Result<(u64, u64), String> {
    let (binary, test) = YARDSTICK;
    let row = rows
        .iter()
        .find(|r| r.binary == binary && r.test == test)
        .ok_or_else(|| format!("the yardstick {binary} {test} has no row in Miri's table"))?;
    let State::Finishes(seconds) = row.state else {
        return Err(format!(
            "the yardstick {binary} {test} is native-only: it has no time to scale by"
        ));
    };
    if seconds == 0 {
        return Err(format!(
            "the yardstick {binary} {test}'s row is 0 s: it has no time to scale by"
        ));
    }
    let ran = report
        .iter()
        .find(|r| r.binary == binary && r.test == test)
        .ok_or_else(|| format!("the yardstick {binary} {test} is not in its report"))?;
    let ms = match &ran.outcome {
        Outcome::Passed(ms) => *ms,
        Outcome::TimedOut(ms) => {
            return Err(format!(
                "the yardstick {binary} {test} was stopped after {ms} ms: no time to scale by"
            ));
        }
        Outcome::Failed(words) => {
            return Err(format!("the yardstick {binary} {test} failed: {words}"));
        }
    };
    let den = seconds
        .checked_mul(1000)
        .ok_or("the yardstick's row overflows")?;
    let scale = (ms, den);
    if less(SCALE_MOST, scale) {
        return Err(format!(
            "runner too slow to judge: the yardstick took {} times its row (over {}), the \
             runner's red and not a test's",
            scale_text(scale),
            scale_text(SCALE_MOST)
        ));
    }
    Ok(if less(scale, SCALE_LEAST) {
        SCALE_LEAST
    } else {
        scale
    })
}

/// A scale as a decimal of three places, rounded up (`1.400`).
#[must_use]
pub fn scale_text((num, den): (u64, u64)) -> String {
    let milli = (u128::from(num) * 1000).div_ceil(u128::from(den.max(1)));
    format!("{}.{:03}", milli / 1000, milli % 1000)
}

/// A finishing test's stop in a job at `scale`, in milliseconds (32.78): its row's `seconds`
/// times 1.6 ([`SLOWER`]) times the scale, rounded up; a stop past `u64::MAX` milliseconds is
/// `u64::MAX`.
#[must_use]
pub fn stop_ms(seconds: u64, (num, den): (u64, u64)) -> u64 {
    let top = u128::from(seconds)
        .checked_mul(1000)
        .and_then(|t| t.checked_mul(u128::from(SLOWER.0)))
        .and_then(|t| t.checked_mul(u128::from(num)));
    // Two u64s' product always fits a u128.
    let bottom = u128::from(SLOWER.1).saturating_mul(u128::from(den.max(1)));
    top.and_then(|t| u64::try_from(t.div_ceil(bottom)).ok())
        .unwrap_or(u64::MAX)
}

/// What a gate job's report says of its tests' stops at its scale ([`over_stops`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Judged {
    /// Each test that reached its own stop, as `binary test: why`: each fails the job.
    pub over: Vec<String>,
    /// Each test a short run's stop (the `miri-smoke` profile's, under [`BUDGET`]) ended before
    /// its own: no verdict and not a pass, named, as `binary test: why`.
    pub cut: Vec<String>,
}

/// Every test of a gate job's `report` that ran to its stop at `scale` (32.78). A test's own stop
/// is [`stop_ms`] of its row; `ceiling` is the profile's stop in seconds, where nextest ends any
/// test. A test that passed at or after its own stop, or that nextest stopped with its own stop
/// at or under `ceiling`, is over: a test stopped at its budget is a failure, never a pass. So is
/// a test that ran with no finishing row. At [`BUDGET`] a test nextest stopped is over whatever
/// its own stop, since no stop is widened past the budget; under it (a short run) a test ended
/// before its own stop is cut, named and not passed.
#[must_use]
pub fn over_stops(report: &[Reported], rows: &[Row], scale: (u64, u64), ceiling: u64) -> Judged {
    let ceiling_ms = ceiling.saturating_mul(1000);
    let mut out = Judged::default();
    for r in report {
        let row = rows
            .iter()
            .find(|x| x.binary == r.binary && x.test == r.test);
        let Some(State::Finishes(seconds)) = row.map(|x| x.state) else {
            if !matches!(r.outcome, Outcome::Failed(_)) {
                out.over
                    .push(format!("{} {}: no finishing row", r.binary, r.test));
            }
            continue;
        };
        let own = stop_ms(seconds, scale);
        let (ms, by_nextest) = match r.outcome {
            Outcome::TimedOut(ms) => (ms, true),
            Outcome::Passed(ms) => (ms, ms >= ceiling_ms),
            Outcome::Failed(_) => continue,
        };
        if ms < own && !by_nextest {
            continue;
        }
        let why = format!(
            "{} {}: {ms} ms, its stop {own} ms (row {seconds} s x 1.6 x {})",
            r.binary,
            r.test,
            scale_text(scale)
        );
        if ms < own && ceiling < BUDGET {
            out.cut
                .push(format!("{why}, ended at the short run's {ceiling} s"));
        } else {
            out.over.push(why);
        }
    }
    out
}

/// The tests a gate job may see stopped at [`BUDGET`] with no verdict (32.68): every finishing row
/// over [`SURE_WITHIN`], `seconds<TAB>binary id<TAB>test name` a line, `#` lines comments, written
/// by `kernel-quality miri-may-stop` (and `miri-times`) from [`TIMES`] with
/// [`render_may_stop`], never by hand.
pub const MAY_STOP: &str = include_str!("miri-may-stop.tsv");

/// [`MAY_STOP`]'s path from the repository root.
pub const MAY_STOP_PATH: &str = "projects/engines/geometry/crates/geom-test/src/miri-may-stop.tsv";

/// How many tests [`MAY_STOP`] may hold. It may only be lowered: `kernel_quality_workflow.rs`
/// fails when the list holds more (a test newly measured over [`SURE_WITHIN`] is resized under
/// `cfg(miri)` before it lands) and when it holds fewer (the change that shrank it lowers this
/// too, so the list can never grow back). The aim is none.
pub const MAY_STOP_MOST: usize = 109;

/// The one crate Miri runs under Tree Borrows (the workflow's job `miri-pool` says why): its
/// finishing tests run in that job, and no partition holds them.
pub const TREE_BORROWS_CRATE: &str = "geom-pool";

/// The mark that opens a native-only row's state.
pub const NATIVE_ONLY: &str = "native-only";

/// The library a test drives under Miri: a library with unsafe code that the kernel uses, and a
/// test that runs it and that Miri finishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Library {
    /// The package name in `Cargo.lock`.
    pub name: &'static str,
    /// The package the kernel names that brings it in (itself, when the kernel names it).
    pub through: &'static str,
    /// The test's nextest binary id.
    pub binary: &'static str,
    /// The test's name.
    pub test: &'static str,
}

/// Each library with unsafe code that the kernel uses, with a finishing test that drives it (32.58,
/// point 3): every registry package whose own library sources hold `unsafe`, among the kernel
/// crates' normal dependencies on Linux (what ships, and what each brings in) and the registry
/// crates the kernel's tests name directly. `kernel_quality_workflow.rs` holds that each named test
/// has a finishing row, and that every such package of `Cargo.lock` is named.
pub const UNSAFE_LIBRARIES: &[Library] = &[
    // The hashing crates: every golden digest and content hash (geom-op's `hash`).
    lib("sha2", "sha2", HASH.0, HASH.1),
    lib("block-buffer", "sha2", HASH.0, HASH.1),
    lib("hybrid-array", "sha2", HASH.0, HASH.1),
    lib("cpufeatures", "sha2", HASH.0, HASH.1),
    // The thread pool: geom-pool alone may name rayon (29.4).
    lib("rayon", "rayon", POOL.0, POOL.1),
    lib("rayon-core", "rayon", POOL.0, POOL.1),
    lib("crossbeam-deque", "rayon", POOL.0, POOL.1),
    lib("crossbeam-epoch", "rayon", POOL.0, POOL.1),
    lib("crossbeam-utils", "rayon", POOL.0, POOL.1),
    lib("either", "rayon", POOL.0, POOL.1),
    // The maths geom-num's ports and enclosures call (`math::support`, `math::unproven`).
    lib(
        "libm",
        "libm",
        "geom-num",
        "math::unproven::tests::other_angles_are_close",
    ),
    // A role name's normal form (geom-op's `name`).
    lib(
        "unicode-normalization",
        "unicode-normalization",
        "geom-op::attack_g87",
        "role_names_must_be_nfc_and_not_empty",
    ),
    // The big-integer and rational oracles of the tests.
    lib("dashu-int", "dashu-int", EXACT.0, EXACT.1),
    lib("dashu-ratio", "dashu-ratio", EXACT.0, EXACT.1),
    lib(
        "num-bigint",
        "num-bigint",
        "geom-num",
        "predicate::tests::i256_holds_every_single_product",
    ),
    lib(
        "num-traits",
        "num-traits",
        "geom-num::attack2_eft",
        "split_documented_values_and_bounds",
    ),
    // The property tests' generator and runner.
    lib(
        "proptest",
        "proptest",
        "geom-op::attack_g82",
        "ids_and_names_order_as_their_bytes",
    ),
    // The counting allocator of the memory tests.
    lib(
        "cap",
        "cap",
        "geom-make::memory",
        "a_refused_allocation_is_out_of_memory",
    ),
    // The CBOR oracles' half-precision floats and JSON.
    lib(
        "half",
        "half",
        "geom-op::cbor",
        "every_binary16_value_is_three_bytes",
    ),
    lib("serde_json", "serde_json", JSON.0, JSON.1),
];

/// The test that drives the hashing crates.
const HASH: (&str, &str) = ("geom-op::attack_g86", "the_cut_is_the_digest_prefix");
/// The test that drives the thread pool.
const POOL: (&str, &str) = (
    "geom-pool::attack_g33",
    "attack_every_task_runs_exactly_once",
);
/// The test that drives the exact integers and rationals.
const EXACT: (&str, &str) = ("geom-curve::attack_g61", "long_helices_and_far_involutes");
/// The test that drives `serde_json`.
const JSON: (&str, &str) = (
    "geom-op::cbor2_oracle",
    "the_json_form_reads_back_as_text_for_every_kind",
);

/// A [`Library`] row.
const fn lib(
    name: &'static str,
    through: &'static str,
    binary: &'static str,
    test: &'static str,
) -> Library {
    Library {
        name,
        through,
        binary,
        test,
    }
}

/// Each library of `libraries` whose named test has no finishing row in `rows`, as `name: binary
/// test`: nothing drives it under Miri.
#[must_use]
pub fn undriven(rows: &[Row], libraries: &[Library]) -> Vec<String> {
    let finishing: BTreeSet<(&str, &str)> = rows
        .iter()
        .filter(|r| matches!(r.state, State::Finishes(_)))
        .map(|r| (r.binary.as_str(), r.test.as_str()))
        .collect();
    libraries
        .iter()
        .filter(|l| !finishing.contains(&(l.binary, l.test)))
        .map(|l| format!("{}: {} {}", l.name, l.binary, l.test))
        .collect()
}

/// The crate of a nextest binary id: `crate` for its unit tests, `crate::file` for an integration
/// test.
#[must_use]
pub fn crate_of(binary: &str) -> &str {
    binary.split("::").next().unwrap_or(binary)
}

/// One test's measured time under Miri, the plan's input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timed {
    /// Whole seconds.
    pub seconds: u64,
    /// nextest's binary id: `crate` for its unit tests, `crate::file` for an integration test.
    pub binary: String,
    /// The test's name within its binary.
    pub test: String,
}

/// What was measured of a test Miri does not finish within the budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Measured {
    /// Stopped after this many seconds, with no verdict: `native-only over N`.
    Over(u64),
    /// Finished in this many seconds, at or over the budget: `native-only N`.
    Took(u64),
}

/// A test's state in the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    /// Miri finishes it in this many seconds, under [`BUDGET`].
    Finishes(u64),
    /// It runs natively only: Miri was measured not to finish it within the budget.
    NativeOnly(Measured),
}

/// One row of the table.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Row {
    /// nextest's binary id.
    pub binary: String,
    /// The test's name within its binary.
    pub test: String,
    /// Whether Miri finishes it, and what was measured.
    pub state: State,
}

/// Whether `name` holds only what a test or binary name may: letters, digits, `_`, `-` and `:`,
/// none of which a filterset reads as anything but part of the name.
fn plain(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b':'))
}

/// A row's state from its first field.
fn parse_state(field: &str) -> Result<State, String> {
    let seconds = |s: &str| s.parse::<u64>().map_err(|e| format!("{s:?}: {e}"));
    let Some(mark) = field.strip_prefix(NATIVE_ONLY) else {
        let n = seconds(field)?;
        return if n < BUDGET {
            Ok(State::Finishes(n))
        } else {
            Err(format!(
                "{n} s is at or over the {BUDGET} s budget: a test Miri does not finish in it is \
                 native-only, with what was measured"
            ))
        };
    };
    let mark = mark.trim();
    if mark.is_empty() || mark == "over" {
        return Err("a native-only mark with no measured time".into());
    }
    if let Some(n) = mark.strip_prefix("over ") {
        return Ok(State::NativeOnly(Measured::Over(seconds(n)?)));
    }
    let n = seconds(mark)?;
    if n < BUDGET {
        return Err(format!(
            "native-only at {n} s, under the {BUDGET} s budget: Miri finishes it"
        ));
    }
    Ok(State::NativeOnly(Measured::Took(n)))
}

/// A state as its row's first field.
fn state_field(state: State) -> String {
    match state {
        State::Finishes(n) => n.to_string(),
        State::NativeOnly(Measured::Over(n)) => format!("{NATIVE_ONLY} over {n}"),
        State::NativeOnly(Measured::Took(n)) => format!("{NATIVE_ONLY} {n}"),
    }
}

/// The rows of a table, in the order given.
///
/// # Errors
///
/// The line, if one is not three tab-separated fields, its state not one [`parse_state`] reads, a
/// name not [`plain`], or a test listed twice or in both states.
pub fn parse_table(text: &str) -> Result<Vec<Row>, String> {
    let mut out = Vec::new();
    let mut seen: BTreeMap<(&str, &str), State> = BTreeMap::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let fields: Vec<&str> = line.split('\t').collect();
        let [state, binary, name] = fields.as_slice() else {
            return Err(format!("not state, binary and test: {line:?}"));
        };
        let state = parse_state(state).map_err(|e| format!("{line:?}: {e}"))?;
        if !plain(binary) || !plain(name) {
            return Err(format!("a name a filterset could misread: {line:?}"));
        }
        if let Some(before) = seen.insert((*binary, *name), state) {
            let finishes = |s: State| matches!(s, State::Finishes(_));
            return Err(if finishes(before) == finishes(state) {
                format!("listed twice: {line:?}")
            } else {
                format!("in both states, finishing and native-only: {line:?}")
            });
        }
        out.push(Row {
            binary: (*binary).to_owned(),
            test: (*name).to_owned(),
            state,
        });
    }
    Ok(out)
}

/// The finishing rows, as the plan's input.
#[must_use]
pub fn finishing(rows: &[Row]) -> Vec<Timed> {
    rows.iter()
        .filter_map(|r| match r.state {
            State::Finishes(seconds) => Some(Timed {
                seconds,
                binary: r.binary.clone(),
                test: r.test.clone(),
            }),
            State::NativeOnly(_) => None,
        })
        .collect()
}

/// The table's text: its comment lines as they were, then every row in byte order of binary and
/// test, one a line.
#[must_use]
pub fn render(comments: &[&str], rows: &[Row]) -> String {
    let mut sorted: Vec<&Row> = rows.iter().collect();
    sorted.sort_by(|x, y| (&x.binary, &x.test).cmp(&(&y.binary, &y.test)));
    let mut out = String::new();
    for c in comments {
        out.push_str(c);
        out.push('\n');
    }
    for r in sorted {
        out.push_str(&state_field(r.state));
        out.push('\t');
        out.push_str(&r.binary);
        out.push('\t');
        out.push_str(&r.test);
        out.push('\n');
    }
    out
}

/// Each finishing row whose test may reach `stop` on a runner [`SLOWER`]: its seconds on that
/// runner, rounded up, over `stop`. At [`BUDGET`] these are the rows over [`SURE_WITHIN`]
/// ([`MAY_STOP`]); at a shorter stop (the `miri-smoke` profile's) more. Longest first, then by
/// binary and name.
#[must_use]
pub fn may_stop_at(rows: &[Row], stop: u64) -> Vec<Timed> {
    let mut out: Vec<Timed> = finishing(rows)
        .into_iter()
        .filter(|t| on_a_slower_runner(t.seconds) > stop)
        .collect();
    out.sort_by(|x, y| {
        y.seconds
            .cmp(&x.seconds)
            .then_with(|| (&x.binary, &x.test).cmp(&(&y.binary, &y.test)))
    });
    out
}

/// [`MAY_STOP`]'s text from the table's rows: its header, then each row [`may_stop_at`] gives at
/// [`BUDGET`], one a line.
#[must_use]
pub fn render_may_stop(rows: &[Row]) -> String {
    let mut out = String::from(
        "# The Miri tests a gate job may see stopped at the 600 s budget with no verdict (decision \
         32.68):\n\
         # every finishing row of Miri's table (miri-times.tsv) over 375 s, 600 s over the 1.6 \
         times slower\n\
         # runner the plan allows, whose test such a runner can take to the stop. Written by\n\
         # `kernel-quality miri-may-stop` (and `miri-times`) from the table, never by hand. A \
         test on it that\n\
         # is stopped is named in its job's summary as stopped, not passed; any other test \
         stopped fails its\n\
         # job. It may only shrink (`geom_test::miri::MAY_STOP_MOST`): each test on it is to be \
         resized under\n\
         # cfg(miri), the same law on a smaller input, until Miri finishes it within 375 s.\n\
         # `seconds<TAB>binary<TAB>test`, the longest first.\n",
    );
    for t in may_stop_at(rows, BUDGET) {
        out.push_str(&t.seconds.to_string());
        out.push('\t');
        out.push_str(&t.binary);
        out.push('\t');
        out.push_str(&t.test);
        out.push('\n');
    }
    out
}

/// The tests of a [`MAY_STOP`] text, as (binary, test).
///
/// # Errors
///
/// As [`parse_table`]; also for a row that is not finishing over [`SURE_WITHIN`], which never
/// belongs on the list.
pub fn parse_may_stop(text: &str) -> Result<Vec<(String, String)>, String> {
    parse_table(text)?
        .into_iter()
        .map(|r| match r.state {
            State::Finishes(s) if s > SURE_WITHIN => Ok((r.binary, r.test)),
            _ => Err(format!(
                "{} {}: not a finishing row over {SURE_WITHIN} s",
                r.binary, r.test
            )),
        })
        .collect()
}

/// The tests a gate job at `stop` seconds may see stopped with no verdict, as (binary, test): at
/// [`BUDGET`], those of `list` (the text of [`MAY_STOP`]); at a shorter stop, a short run's (the
/// `miri-smoke` profile's), each [`may_stop_at`] gives.
///
/// # Errors
///
/// For a stop past [`BUDGET`], and at [`BUDGET`] for a `list` that [`parse_may_stop`] refuses or
/// that is not what [`render_may_stop`] writes from `rows`.
pub fn allowed_stops(
    rows: &[Row],
    list: &str,
    stop: u64,
) -> Result<BTreeSet<(String, String)>, String> {
    if stop > BUDGET {
        return Err(format!("a stop of {stop} s is past the {BUDGET} s budget"));
    }
    if stop < BUDGET {
        return Ok(may_stop_at(rows, stop)
            .into_iter()
            .map(|t| (t.binary, t.test))
            .collect());
    }
    let listed = parse_may_stop(list)?;
    if list != render_may_stop(rows) {
        return Err(format!(
            "{MAY_STOP_PATH} is not what `kernel-quality miri-may-stop` writes from Miri's table"
        ));
    }
    Ok(listed.into_iter().collect())
}

/// What a gate job's reports say of the tests nextest stopped ([`stops`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stops {
    /// The tests the reports name.
    pub reported: usize,
    /// The stopped tests that may be, as `binary test`.
    pub allowed: Vec<String>,
    /// The stopped tests that may not be, as `binary test`: each fails the job.
    pub refused: Vec<String>,
}

/// Which of a gate job's reported tests nextest stopped at `stop` seconds with no verdict, split
/// by whether `may` (as (binary, test)) holds them. A stopped test is one timed out, or one passed
/// at `stop` or later: the gate's profiles count a test stopped as passing (`on-timeout =
/// "pass"`), and nothing passes later than the stop that ends it, as [`measured`] reads a
/// measuring run.
#[must_use]
pub fn stops(report: &[Reported], stop: u64, may: &BTreeSet<(String, String)>) -> Stops {
    let stop_ms = stop.saturating_mul(1000);
    let mut out = Stops {
        reported: report.len(),
        ..Stops::default()
    };
    for r in report {
        let stopped = match r.outcome {
            Outcome::Passed(ms) => ms >= stop_ms,
            Outcome::TimedOut(_) => true,
            Outcome::Failed(_) => false,
        };
        if stopped {
            let name = format!("{} {}", r.binary, r.test);
            if may.contains(&(r.binary.clone(), r.test.clone())) {
                out.allowed.push(name);
            } else {
                out.refused.push(name);
            }
        }
    }
    out
}

/// One test's outcome in a nextest report (`junit.xml`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It passed, after this many milliseconds.
    Passed(u64),
    /// nextest stopped it at its slow timeout, after this many milliseconds.
    TimedOut(u64),
    /// Anything else: a failure, a crash, a kill; the report's words for it.
    Failed(String),
}

/// One test of a report: its binary id, name and outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reported {
    /// nextest's binary id (the test case's class name).
    pub binary: String,
    /// The test's name.
    pub test: String,
    /// What happened to it.
    pub outcome: Outcome,
}

/// Milliseconds from a `junit.xml` `time` attribute in seconds (`48.929`), rounded up.
fn millis(time: &str) -> Result<u64, String> {
    let (whole, frac) = time.split_once('.').unwrap_or((time, ""));
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(whole) || !(frac.is_empty() || digits(frac)) {
        return Err(format!("not a time in seconds: {time:?}"));
    }
    let whole: u64 = whole.parse().map_err(|e| format!("{time:?}: {e}"))?;
    let mut ms: u64 = 0;
    let mut rest = false;
    for (i, b) in frac.bytes().enumerate() {
        let d = u64::from(b.saturating_sub(b'0'));
        if i < 3 {
            ms = ms
                .checked_mul(10)
                .and_then(|m| m.checked_add(d))
                .ok_or("time overflows")?;
        } else if d > 0 {
            rest = true;
        }
    }
    for _ in frac.len()..3 {
        ms = ms.checked_mul(10).ok_or("time overflows")?;
    }
    whole
        .checked_mul(1000)
        .and_then(|w| w.checked_add(ms))
        .and_then(|t| t.checked_add(u64::from(rest)))
        .ok_or_else(|| format!("time overflows: {time:?}"))
}

/// Every test case of a nextest report (`target/nextest/<profile>/junit.xml`), in the
/// report's order. A case with a `failure` or `error` child failed, unless nextest's words for it
/// end in "timeout" (its slow timeout stopped it); with neither it passed.
///
/// # Errors
///
/// For text that is not XML, or a test case without a name, class name or time.
pub fn read_junit(xml: &str) -> Result<Vec<Reported>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("not a JUnit report: {e}"))?;
    let mut out = Vec::new();
    for case in doc.descendants().filter(|n| n.has_tag_name("testcase")) {
        let attr = |a: &str| {
            case.attribute(a)
                .ok_or_else(|| format!("a test case without its {a}"))
        };
        let (binary, test, time) = (attr("classname")?, attr("name")?, millis(attr("time")?)?);
        let bad = case
            .children()
            .find(|c| c.has_tag_name("failure") || c.has_tag_name("error"));
        let outcome = match bad {
            None => Outcome::Passed(time),
            Some(b) => {
                let words = b
                    .attribute("type")
                    .or_else(|| b.attribute("message"))
                    .unwrap_or(b.tag_name().name());
                if b.has_tag_name("failure") && words.ends_with("timeout") {
                    Outcome::TimedOut(time)
                } else {
                    Outcome::Failed(words.to_owned())
                }
            }
        };
        out.push(Reported {
            binary: binary.to_owned(),
            test: test.to_owned(),
            outcome,
        });
    }
    Ok(out)
}

/// A measuring run's scale: the seconds a CI runner takes for one of the run's seconds, as a
/// decimal (`1` for a CI run's own report, `2.3` for a development machine measured against one),
/// read as a fraction so no float rounds it.
///
/// # Errors
///
/// For anything but digits with at most one point, or a scale of zero.
pub fn parse_scale(text: &str) -> Result<(u64, u64), String> {
    let (whole, frac) = text.split_once('.').unwrap_or((text, ""));
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if whole.is_empty() || !digits(whole) || !digits(frac) || frac.len() > 6 {
        return Err(format!("not a scale: {text:?}"));
    }
    let num: u64 = format!("{whole}{frac}")
        .parse()
        .map_err(|e| format!("{text:?}: {e}"))?;
    let den = (0..frac.len()).try_fold(1_u64, |d, _| d.checked_mul(10));
    match den {
        Some(den) if num > 0 => Ok((num, den)),
        _ => Err(format!("not a scale: {text:?}")),
    }
}

/// `ms` milliseconds of the run as whole CI seconds at `scale`, rounded up.
fn scaled(ms: u64, (num, den): (u64, u64)) -> Result<u64, String> {
    let top = u128::from(ms)
        .checked_mul(u128::from(num))
        .ok_or("time overflows")?;
    let bottom = u128::from(den).checked_mul(1000).ok_or("scale overflows")?;
    let s = top.div_ceil(bottom);
    u64::try_from(s).map_err(|e| e.to_string())
}

/// A table read for rewriting: its comment lines, its rows, and the (binary, test) of each row at the
/// budget.
pub type OldTable = (Vec<String>, Vec<Row>, Vec<(String, String)>);

/// A table's text read for rewriting: its comment lines, its rows, and the tests whose rows sit at
/// or over the budget (the form tables had before 32.58), which the next measuring run must
/// replace.
///
/// # Errors
///
/// As [`parse_table`], but for rows at or over the budget.
pub fn parse_old(text: &str) -> Result<OldTable, String> {
    let comments = text
        .lines()
        .filter(|l| l.starts_with('#'))
        .map(str::to_owned)
        .collect();
    let mut kept = String::new();
    let mut pending = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let mut fields = line.split('\t');
        let first = fields.next().and_then(|s| s.parse::<u64>().ok());
        match (first, fields.next(), fields.next(), fields.next()) {
            (Some(n), Some(b), Some(t), None) if n >= BUDGET => {
                pending.push((b.to_owned(), t.to_owned()));
            }
            _ => {
                kept.push_str(line);
                kept.push('\n');
            }
        }
    }
    let rows = parse_table(&kept)?;
    let all: BTreeSet<(&str, &str)> = rows
        .iter()
        .map(|r| (r.binary.as_str(), r.test.as_str()))
        .chain(pending.iter().map(|(b, t)| (b.as_str(), t.as_str())))
        .collect();
    if all.len() != rows.len().saturating_add(pending.len()) {
        return Err("a test listed twice".into());
    }
    Ok((comments, rows, pending))
}

/// The table's text after a measuring run (`kernel-quality miri-times`). Each test the report
/// names gets the state it measured, at `scale` (see [`parse_scale`]); the run's `stop`, in its
/// own seconds, is where its slow timeout stops a test, so a pass that took that long was stopped
/// too (a run whose profile counts a stopped test as passing). A pass under the stop finishes,
/// unless its scaled time reaches the budget; a test stopped is native-only, over its scaled time.
/// Rows the report does not name stay as they were. With `listed` (every test of the kernel's
/// nextest listing, [`listed_tests`]), a row naming a test the listing lacks, renamed or removed,
/// is dropped. `note`, if given, is added to the header's comments.
///
/// # Errors
///
/// Every test the report shows failing or killed under Miri, named, and no table: a fault Miri
/// found, or a test that ran out of memory, is looked at, never recorded as slow. Also a row left at
/// the budget, a test the report names twice, and a name [`plain`] refuses.
pub fn write_table(
    old: &str,
    report: &[Reported],
    scale: (u64, u64),
    stop: u64,
    listed: Option<&[(String, String)]>,
    note: Option<&str>,
) -> Result<String, String> {
    write_table_parts(old, &[(report, scale)], stop, listed, note)
}

/// The scale that turns a measuring job's seconds into yardstick seconds (32.78): the inverse of
/// the job's [`job_scale`] from its yardstick run's report, `yardstick`, against the yardstick's
/// row in `rows`. A test of `s` seconds in such a job is stored as `s` over the job's scale.
///
/// # Errors
///
/// As [`job_scale`]: a missing yardstick time is an error, never a default, and a runner over
/// [`SCALE_MOST`] is too slow to judge.
pub fn to_yardstick_seconds(rows: &[Row], yardstick: &[Reported]) -> Result<(u64, u64), String> {
    let (num, den) = job_scale(rows, yardstick)?;
    Ok((den, num))
}

/// One measuring report made ready for [`write_table_parts`] in yardstick seconds (32.78,
/// `kernel-quality miri-times --yardstick`): its tests, with the scale of its own job's yardstick
/// report `yardstick` ([`to_yardstick_seconds`] against `rows`, the table before the run), so the
/// same work measured on a slow runner and on a fast one gives the same row. The yardstick's own
/// row is the unit and stays as it is: the yardstick in `report` is left out.
///
/// # Errors
///
/// As [`to_yardstick_seconds`].
pub fn in_yardstick_seconds(
    rows: &[Row],
    report: &[Reported],
    yardstick: &[Reported],
) -> Result<(Vec<Reported>, (u64, u64)), String> {
    let scale = to_yardstick_seconds(rows, yardstick)?;
    let (yb, yt) = YARDSTICK;
    let tests = report
        .iter()
        .filter(|r| !(r.binary == yb && r.test == yt))
        .cloned()
        .collect();
    Ok((tests, scale))
}

/// [`write_table`] of several reports, each at its own scale.
///
/// # Errors
///
/// As [`write_table`].
pub fn write_table_parts(
    old: &str,
    parts: &[(&[Reported], (u64, u64))],
    stop: u64,
    listed: Option<&[(String, String)]>,
    note: Option<&str>,
) -> Result<String, String> {
    let (mut comments, rows, pending) = parse_old(old)?;
    if let Some(note) = note {
        comments.push(format!("# {note}"));
    }
    let rows = measured_parts(&rows, parts, stop)?;
    let known: BTreeSet<(&str, &str)> = rows
        .iter()
        .map(|r| (r.binary.as_str(), r.test.as_str()))
        .collect();
    let keep =
        |b: &str, t: &str| listed.is_none_or(|l| l.iter().any(|(lb, lt)| lb == b && lt == t));
    let left: Vec<String> = pending
        .iter()
        .filter(|(b, t)| !known.contains(&(b.as_str(), t.as_str())) && keep(b, t))
        .map(|(b, t)| format!("{b} {t}"))
        .collect();
    if !left.is_empty() {
        return Err(format!(
            "{} rows still at the budget, which no report measured:\n{}",
            left.len(),
            left.join("\n")
        ));
    }
    let rows: Vec<Row> = rows
        .into_iter()
        .filter(|r| keep(&r.binary, &r.test))
        .collect();
    let comments: Vec<&str> = comments.iter().map(String::as_str).collect();
    Ok(render(&comments, &rows))
}

/// `rows` with each test of `report` in the state it measured ([`write_table`]).
///
/// # Errors
///
/// As [`write_table`].
pub fn measured(
    rows: &[Row],
    report: &[Reported],
    scale: (u64, u64),
    stop: u64,
) -> Result<Vec<Row>, String> {
    measured_parts(rows, &[(report, scale)], stop)
}

/// [`measured`] of several reports, each at its own scale; a test two reports name is an error.
///
/// # Errors
///
/// As [`write_table`].
pub fn measured_parts(
    rows: &[Row],
    parts: &[(&[Reported], (u64, u64))],
    stop: u64,
) -> Result<Vec<Row>, String> {
    let stop_ms = stop.checked_mul(1000).ok_or("stop overflows")?;
    let mut table: BTreeMap<(String, String), State> = rows
        .iter()
        .map(|r| ((r.binary.clone(), r.test.clone()), r.state))
        .collect();
    let mut failed = Vec::new();
    let mut seen = BTreeSet::new();
    for (r, scale) in parts
        .iter()
        .flat_map(|(report, scale)| report.iter().map(move |r| (r, *scale)))
    {
        if !plain(&r.binary) || !plain(&r.test) {
            return Err(format!("a name a filterset could misread: {r:?}"));
        }
        if !seen.insert((r.binary.as_str(), r.test.as_str())) {
            return Err(format!("the report names {} {} twice", r.binary, r.test));
        }
        let state = match &r.outcome {
            Outcome::Passed(ms) if *ms < stop_ms => {
                let s = scaled(*ms, scale)?;
                if s < BUDGET {
                    State::Finishes(s)
                } else {
                    State::NativeOnly(Measured::Took(s))
                }
            }
            Outcome::Passed(ms) | Outcome::TimedOut(ms) => {
                State::NativeOnly(Measured::Over(scaled(*ms, scale)?))
            }
            Outcome::Failed(words) => {
                failed.push(format!("{} {}: {words}", r.binary, r.test));
                continue;
            }
        };
        table.insert((r.binary.clone(), r.test.clone()), state);
    }
    if !failed.is_empty() {
        return Err(format!(
            "{} tests did not pass under Miri; nothing is written:\n{}",
            failed.len(),
            failed.join("\n")
        ));
    }
    Ok(table
        .into_iter()
        .map(|((binary, test), state)| Row {
            binary,
            test,
            state,
        })
        .collect())
}

/// The tests of a `cargo nextest list --message-format json` listing that the run would run: each
/// (binary id, test) whose filter matches and that is not ignored, in the listing's order.
///
/// # Errors
///
/// For a listing without `rust-suites`, or a suite without its test cases.
pub fn listed_tests(listing: &Value) -> Result<Vec<(String, String)>, String> {
    let suites = listing
        .get("rust-suites")
        .and_then(Value::as_object)
        .ok_or("a nextest listing without rust-suites")?;
    let mut out = Vec::new();
    for (binary, suite) in suites {
        let cases = suite
            .get("testcases")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("{binary}: no testcases"))?;
        for (name, case) in cases {
            let ignored = case.get("ignored").and_then(Value::as_bool) == Some(true);
            let matches = case
                .get("filter-match")
                .and_then(|m| m.get("status"))
                .and_then(Value::as_str)
                .is_none_or(|s| s == "matches");
            if matches && !ignored {
                out.push((binary.clone(), name.clone()));
            }
        }
    }
    Ok(out)
}

/// Each listed test the table has no row for, as `binary test`: a test nobody has measured under
/// Miri, which must be before it lands (`kernel-quality miri-times`).
#[must_use]
pub fn unlisted(rows: &[Row], listed: &[(String, String)]) -> Vec<String> {
    let known: BTreeSet<(&str, &str)> = rows
        .iter()
        .map(|r| (r.binary.as_str(), r.test.as_str()))
        .collect();
    listed
        .iter()
        .filter(|(b, t)| !known.contains(&(b.as_str(), t.as_str())))
        .map(|(b, t)| format!("{b} {t}"))
        .collect()
}

/// The partitions: which finishing tests each runs.
#[derive(Clone, Debug)]
pub struct Plan {
    /// For each partition, its tests and their total seconds.
    parts: Vec<(Vec<Timed>, u64)>,
}

/// What a part must end within ([`pack`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// A Miri gate job (`miri`, `miri-pool`): its tests at their [`planned`] seconds and its
    /// listing of their binaries on a runner [`SLOWER`] within [`PART_SECONDS`], so the job ends
    /// within [`JOB_SECONDS`] on such a runner after [`FIXED_SECONDS`].
    Job,
    /// A measuring part (`miri-measure`): its tests within [`BUDGET`], the measuring stop.
    Stop,
}

/// The seconds `count` tests of `total` seconds, the longest `longest`, can take at [`AT_ONCE`] at
/// a time in whatever order nextest starts them ([`Plan::part_seconds`]).
fn any_order(total: u64, longest: u64, count: usize) -> u64 {
    if u64::try_from(count).unwrap_or(u64::MAX) <= AT_ONCE {
        return longest;
    }
    total
        .saturating_sub(longest)
        .div_ceil(AT_ONCE)
        .saturating_add(longest)
}

/// Whether a part of `count` tests of `total` seconds, the longest `longest`, from `binaries`
/// test binaries, ends within `rule`. For [`Rule::Job`] the seconds are [`planned`] ones.
fn within(rule: Rule, total: u64, longest: u64, count: usize, binaries: usize) -> bool {
    let tests = any_order(total, longest, count);
    match rule {
        Rule::Stop => tests <= BUDGET,
        Rule::Job => {
            let listing = on_a_slower_runner(
                u64::try_from(binaries)
                    .unwrap_or(u64::MAX)
                    .saturating_mul(LIST_SECONDS),
            );
            listing.saturating_add(tests) <= PART_SECONDS
        }
    }
}

/// `tests` packed into the fewest parts this packs them into that each end within `rule`: first
/// fit decreasing, each test, the longest first (then by binary and name), into the first part it
/// fits, or a new part. No tests give one empty part. `None` if a test alone does not fit. For
/// [`Rule::Job`] the tests are finishing ones, each under [`BUDGET`] (`None` for one that is
/// not), and the plan holds them at their [`planned`] seconds.
#[must_use]
pub fn pack(tests: &[Timed], rule: Rule) -> Option<Plan> {
    let mut order = tests.to_vec();
    if rule == Rule::Job {
        for t in &mut order {
            if t.seconds >= BUDGET {
                return None;
            }
            t.seconds = planned(t.seconds);
        }
    }
    order.sort_by(|x, y| {
        y.seconds
            .cmp(&x.seconds)
            .then_with(|| (&x.binary, &x.test).cmp(&(&y.binary, &y.test)))
    });
    // Each part: its tests, total, longest test and binaries.
    let mut parts: Vec<(Vec<Timed>, u64, u64, BTreeSet<String>)> = Vec::new();
    for t in order {
        let fits = |p: &(Vec<Timed>, u64, u64, BTreeSet<String>)| {
            let binaries =
                p.3.len()
                    .saturating_add(usize::from(!p.3.contains(&t.binary)));
            within(
                rule,
                p.1.saturating_add(t.seconds),
                p.2.max(t.seconds),
                p.0.len().saturating_add(1),
                binaries,
            )
        };
        if let Some(p) = parts.iter_mut().find(|p| fits(p)) {
            p.1 = p.1.saturating_add(t.seconds);
            p.2 = p.2.max(t.seconds);
            p.3.insert(t.binary.clone());
            p.0.push(t);
        } else if within(rule, t.seconds, t.seconds, 1, 1) {
            let binaries = BTreeSet::from([t.binary.clone()]);
            parts.push((Vec::new(), t.seconds, t.seconds, binaries));
            parts.last_mut()?.0.push(t);
        } else {
            return None;
        }
    }
    let mut out: Vec<(Vec<Timed>, u64)> = parts.into_iter().map(|p| (p.0, p.1)).collect();
    if out.is_empty() {
        out.push((Vec::new(), 0));
    }
    Some(Plan { parts: out })
}

/// The Miri gate's plan (`kernel-quality miri-plan`): the partitions of every finishing test but
/// [`TREE_BORROWS_CRATE`]'s, packed by [`Rule::Job`], and that crate's finishing tests as one part
/// of their own, held to the same rule, for the job that runs them under Tree Borrows.
#[derive(Clone, Debug)]
pub struct GatePlan {
    /// The partitions, 1 to n.
    pub parts: Plan,
    /// [`TREE_BORROWS_CRATE`]'s finishing tests, as one part.
    pub pool: Plan,
}

/// The gate's plan from the table's rows ([`GatePlan`]).
///
/// # Errors
///
/// When a test fits no partition (a finishing row at or over [`BUDGET`], which [`parse_table`]
/// refuses), when the partitions are more than one matrix of GitHub Actions holds
/// ([`crate::mutants::MATRIX_JOBS`]), or when [`TREE_BORROWS_CRATE`]'s finishing tests do not fit
/// their one job.
pub fn gate_plan(rows: &[Row]) -> Result<GatePlan, String> {
    let (pool, rest): (Vec<Timed>, Vec<Timed>) = finishing(rows)
        .into_iter()
        .partition(|t| crate_of(&t.binary) == TREE_BORROWS_CRATE);
    let parts = pack(&rest, Rule::Job).ok_or_else(|| {
        format!("a finishing test takes {BUDGET} s or longer, which no partition can hold")
    })?;
    let most = usize::try_from(crate::mutants::MATRIX_JOBS).unwrap_or(usize::MAX);
    if parts.len() > most {
        return Err(format!(
            "{} partitions, more than the {most} one matrix holds",
            parts.len()
        ));
    }
    let pool = pack(&pool, Rule::Job)
        .filter(|p| p.len() == 1)
        .ok_or_else(|| {
            format!(
                "{TREE_BORROWS_CRATE}'s finishing tests do not fit the one job that runs them \
                 within {PART_SECONDS} s"
            )
        })?;
    Ok(GatePlan { parts, pool })
}

/// Which tests a measuring run measures (`kernel-quality miri-measure`, the workflow's dispatch
/// input `miri_measure`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measuring {
    /// The tests the table has no row for: what a change added.
    New,
    /// Every test, to measure the whole table again.
    All,
}

/// The tests of `listed` (a run's nextest listing, [`listed_tests`]) a measuring run measures,
/// in the listing's order, each planned at the seconds it is expected to take: its finishing
/// row's, or [`BUDGET`], a CI measuring run's stop, for a test whose row is native-only or that
/// has none.
#[must_use]
pub fn to_measure(rows: &[Row], listed: &[(String, String)], which: Measuring) -> Vec<Timed> {
    let table: BTreeMap<(&str, &str), State> = rows
        .iter()
        .map(|r| ((r.binary.as_str(), r.test.as_str()), r.state))
        .collect();
    listed
        .iter()
        .filter_map(|(binary, test)| {
            let state = table.get(&(binary.as_str(), test.as_str()));
            if which == Measuring::New && state.is_some() {
                return None;
            }
            let seconds = match state {
                Some(State::Finishes(s)) => *s,
                Some(State::NativeOnly(_)) | None => BUDGET,
            };
            Some(Timed {
                seconds,
                binary: binary.clone(),
                test: test.clone(),
            })
        })
        .collect()
}

/// The parts of a measuring run: its tests packed ([`pack`]) into parts whose tests each end
/// within [`BUDGET`] at [`AT_ONCE`] a time ([`Rule::Stop`]). No tests give one empty part.
///
/// # Errors
///
/// For more parts than the workflow's part jobs hold ([`crate::mutants::MAX_SHARDS`]): the run is
/// to be narrowed by a filterset. Also for a test planned over the stop, which [`to_measure`]
/// never gives.
pub fn measure_plan(tests: &[Timed]) -> Result<Plan, String> {
    let plan = pack(tests, Rule::Stop).ok_or_else(|| {
        format!("a test is planned over the {BUDGET} s stop, which no part can hold")
    })?;
    let n = plan.len();
    let most = usize::try_from(crate::mutants::MAX_SHARDS).unwrap_or(usize::MAX);
    if n > most {
        return Err(format!(
            "{} tests need {n} parts of at most {BUDGET} s, more than the workflow's {most}:              narrow the run with a filterset (the input miri_measure_tests)",
            tests.len()
        ));
    }
    Ok(plan)
}

/// A measuring run's parts as JSON for the workflow: `{"tests": t, "count": n, "parts": [...],
/// "count_2": m, "parts_2": [...]}`, numbered from 1. `parts` lists the first part job's, 1 to
/// `min(n, 256)`, and `parts_2` the second's, 257 to `n`, `count_2` of them. A GitHub Actions
/// matrix may not be empty, so a job with no part still lists its first (1 or 257); the workflow
/// runs a job only when its count is not 0. `count` is 0 when there is no test to measure.
#[must_use]
pub fn measure_matrix(tests: usize, count: usize) -> String {
    let jobs = usize::try_from(crate::mutants::MATRIX_JOBS).unwrap_or(usize::MAX);
    let first: Vec<usize> = (1..=count.clamp(1, jobs)).collect();
    let count_2 = count.saturating_sub(jobs);
    let second: Vec<usize> =
        (jobs.saturating_add(1)..=jobs.saturating_add(count_2.max(1))).collect();
    serde_json::json!({
        "tests": tests,
        "count": count,
        "parts": first,
        "count_2": count_2,
        "parts_2": second,
    })
    .to_string()
}

impl Plan {
    /// How many partitions. A gate job's plan ([`Rule::Job`]) holds each test at its [`planned`]
    /// seconds, so its [`Plan::part_seconds`] and [`Plan::longest`] are on a runner [`SLOWER`].
    #[must_use]
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Whether the plan has no partitions (never: [`pack`] gives one at least).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// The tests of partition `k` (from 1), as (binary, test).
    #[must_use]
    pub fn tests_of(&self, k: usize) -> Vec<(&str, &str)> {
        k.checked_sub(1)
            .and_then(|p| self.parts.get(p))
            .map(|(tests, _)| {
                tests
                    .iter()
                    .map(|t| (t.binary.as_str(), t.test.as_str()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The most seconds partition `k`'s tests (from 1) can take at [`AT_ONCE`] at a time, in
    /// whatever order nextest starts them: with [`AT_ONCE`] tests or fewer, all start at once and
    /// it is the longest test; with more, nextest starts the next test whenever one ends, so the
    /// last to end started once every slot had run the rest (Graham's bound for list scheduling,
    /// module documentation), the rest over [`AT_ONCE`], rounded up, and then the longest test.
    /// Zero for no such partition.
    #[must_use]
    pub fn part_seconds(&self, k: usize) -> u64 {
        let Some((tests, total)) = k.checked_sub(1).and_then(|p| self.parts.get(p)) else {
            return 0;
        };
        let longest = tests.iter().map(|t| t.seconds).max().unwrap_or(0);
        any_order(*total, longest, tests.len())
    }

    /// How many test binaries partition `k` (from 1) runs tests of: nextest lists each under Miri
    /// before the first test starts.
    #[must_use]
    pub fn binaries(&self, k: usize) -> usize {
        k.checked_sub(1)
            .and_then(|p| self.parts.get(p))
            .map_or(0, |(tests, _)| {
                tests
                    .iter()
                    .map(|t| t.binary.as_str())
                    .collect::<BTreeSet<_>>()
                    .len()
            })
    }

    /// Whether partition `k` (from 1) ends within `rule` ([`pack`]).
    #[must_use]
    pub fn ends_within(&self, k: usize, rule: Rule) -> bool {
        let Some((tests, total)) = k.checked_sub(1).and_then(|p| self.parts.get(p)) else {
            return false;
        };
        let longest = tests.iter().map(|t| t.seconds).max().unwrap_or(0);
        within(rule, *total, longest, tests.len(), self.binaries(k))
    }

    /// The seconds its tests can take in the slowest partition ([`Plan::part_seconds`]).
    #[must_use]
    pub fn longest(&self) -> u64 {
        (1..=self.parts.len())
            .map(|k| self.part_seconds(k))
            .max()
            .unwrap_or(0)
    }

    /// The nextest filterset of partition `k` (from 1): its tests, one by one.
    ///
    /// # Errors
    ///
    /// For a `k` outside 1 ..= [`Plan::len`].
    pub fn filterset(&self, k: usize) -> Result<String, String> {
        let p = k
            .checked_sub(1)
            .filter(|&p| p < self.parts.len())
            .ok_or_else(|| format!("no partition {k} of {}", self.parts.len()))?;
        let terms: Vec<String> = self
            .tests_of(p.saturating_add(1))
            .into_iter()
            .map(|(b, t)| format!("(binary_id(={b}) & test(={t}))"))
            .collect();
        Ok(if terms.is_empty() {
            "none()".into()
        } else {
            terms.join(" | ")
        })
    }
}

/// Whether a manifest path is a kernel crate's (in the kernel's crates folder, not a development
/// crate).
fn is_kernel_manifest(manifest: &str) -> bool {
    let parts: Vec<&str> = manifest
        .split(['/', '\\'])
        .filter(|p| !p.is_empty())
        .collect();
    let Some((file, rest)) = parts.split_last() else {
        return false;
    };
    let Some((folder, parent)) = rest.split_last() else {
        return false;
    };
    *file == "Cargo.toml"
        && parent.ends_with(KERNEL_CRATES_DIR.as_slice())
        && !DEVELOPMENT_CRATES.contains(folder)
}

/// The package names of every kernel crate in the workspace `cargo metadata` describes, in name
/// order.
///
/// # Errors
///
/// A description of what is missing, if the metadata has no package list, a package has no name or
/// manifest path, or no package is a kernel crate.
pub fn packages(metadata: &Value) -> Result<Vec<String>, String> {
    let list = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("cargo metadata has no packages")?;
    let mut out = Vec::new();
    for p in list {
        let manifest = p
            .get("manifest_path")
            .and_then(Value::as_str)
            .ok_or("a package without a manifest path")?;
        if !is_kernel_manifest(manifest) {
            continue;
        }
        let name = p
            .get("name")
            .and_then(Value::as_str)
            .ok_or("a package without a name")?;
        out.push(name.to_owned());
    }
    if out.is_empty() {
        return Err(
            "no kernel crate in the metadata: a Miri run that runs nothing must not pass".into(),
        );
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// The `-p <package>` arguments for `packages`, joined by spaces (no package name has one).
#[must_use]
pub fn cargo_args(packages: &[String]) -> String {
    packages
        .iter()
        .map(|p| format!("-p {p}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;

    fn package(name: &str, manifest: &str) -> Value {
        serde_json::json!({"name": name, "manifest_path": manifest, "targets": []})
    }

    #[test]
    fn every_kernel_crate_and_nothing_else() {
        let k = "/r/projects/engines/geometry/crates";
        let metadata = serde_json::json!({"packages": [
            package("geom-poly", &format!("{k}/geom-poly/Cargo.toml")),
            package("geom-num", &format!("{k}\\geom-num\\Cargo.toml")),
            package("geom-test", &format!("{k}/geom-test/Cargo.toml")),
            package("geom-bench", &format!("{k}/geom-bench/Cargo.toml")),
            package("engine-geometry", "/r/projects/engines/crates/engine-geometry/Cargo.toml"),
            package("kit-core", "/r/projects/kit/crates/kit-core/Cargo.toml"),
            package("deep", &format!("{k}/geom-num/fuzz/Cargo.toml")),
        ]});
        let names = packages(&metadata).unwrap();
        assert_eq!(names, ["geom-num", "geom-poly"]);
        assert_eq!(cargo_args(&names), "-p geom-num -p geom-poly");
        assert!(packages(&serde_json::json!({"packages": []})).is_err());
        assert!(packages(&serde_json::json!({})).is_err());
        assert!(packages(&serde_json::json!({"packages": [{"name": "x"}]})).is_err());
    }

    fn row(binary: &str, test: &str, state: State) -> Row {
        Row {
            binary: binary.into(),
            test: test.into(),
            state,
        }
    }

    fn timed(seconds: u64, binary: &str, test: &str) -> Timed {
        Timed {
            seconds,
            binary: binary.into(),
            test: test.into(),
        }
    }

    #[test]
    fn a_table_is_read_or_refused_by_the_line() {
        let text = "# a comment\n599\tgeom-num::wide\ti256::add\n\nnative-only over 276\tgeom-num\t\
                    dd::tests::x\nnative-only 734\tgeom-num\ty\n";
        assert_eq!(
            parse_table(text).unwrap(),
            [
                row("geom-num::wide", "i256::add", State::Finishes(599)),
                row(
                    "geom-num",
                    "dd::tests::x",
                    State::NativeOnly(Measured::Over(276))
                ),
                row("geom-num", "y", State::NativeOnly(Measured::Took(734))),
            ]
        );
        for (bad, why) in [
            ("600\tgeom-num::wide\tx\n", "at or over"),
            ("601\tgeom-num::wide\tx\n", "at or over"),
            ("600\tgeom-num::wide\n", "not state"),
            ("6\tgeom-num::wide\tx\ty\n", "not state"),
            ("ten\tgeom-num\tx\n", "invalid digit"),
            ("-1\tgeom-num\tx\n", "invalid digit"),
            ("1\tgeom num\tx\n", "misread"),
            ("1\tgeom-num\tx)\n", "misread"),
            ("1\tgeom-num\tx | all()\n", "misread"),
            ("1\t\tx\n", "misread"),
            // A native-only mark with no measured time.
            ("native-only\tgeom-num\tx\n", "no measured time"),
            ("native-only over\tgeom-num\tx\n", "no measured time"),
            ("native-only over \tgeom-num\tx\n", "no measured time"),
            ("native-only soon\tgeom-num\tx\n", "invalid digit"),
            // Miri finished it under the budget: it is not native-only.
            ("native-only 120\tgeom-num\tx\n", "Miri finishes it"),
            ("1\tgeom-num\tx\n2\tgeom-num\tx\n", "listed twice"),
            // A test in both states.
            (
                "1\tgeom-num\tx\nnative-only over 9\tgeom-num\tx\n",
                "both states",
            ),
            (
                "native-only 700\tgeom-num\tx\n3\tgeom-num\tx\n",
                "both states",
            ),
        ] {
            let got = parse_table(bad);
            assert!(
                got.as_ref().is_err_and(|e| e.contains(why)),
                "{bad:?}: {got:?}"
            );
        }
    }

    #[test]
    fn a_table_renders_sorted_with_its_comments_and_reads_back() {
        let rows = [
            row("b", "y", State::Finishes(3)),
            row("a", "z", State::NativeOnly(Measured::Over(276))),
            row("a", "x", State::NativeOnly(Measured::Took(700))),
        ];
        let text = render(&["# one", "# two"], &rows);
        assert_eq!(
            text,
            "# one\n# two\nnative-only 700\ta\tx\nnative-only over 276\ta\tz\n3\tb\ty\n"
        );
        let mut back = parse_table(&text).unwrap();
        let mut want = rows.to_vec();
        back.sort();
        want.sort();
        assert_eq!(back, want);
        assert_eq!(finishing(&rows), [timed(3, "b", "y")]);
    }

    #[test]
    fn a_junit_report_reads_each_outcome() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="4" failures="2" errors="1">
  <testsuite name="geom-num::wide" tests="4">
    <testcase name="quick" classname="geom-num::wide" time="48.929"></testcase>
    <testcase name="slow" classname="geom-num::wide" time="120.004">
      <failure type="test timeout">slow</failure>
    </testcase>
    <testcase name="bad" classname="geom-num::wide" time="3.2">
      <failure type="test failure with exit code 1">ub</failure>
    </testcase>
    <testcase name="gone" classname="geom-num::wide" time="7">
      <error type="execution failure"/>
    </testcase>
  </testsuite>
</testsuites>"#;
        assert_eq!(
            read_junit(xml).unwrap(),
            [
                Reported {
                    binary: "geom-num::wide".into(),
                    test: "quick".into(),
                    outcome: Outcome::Passed(48_929),
                },
                Reported {
                    binary: "geom-num::wide".into(),
                    test: "slow".into(),
                    outcome: Outcome::TimedOut(120_004),
                },
                Reported {
                    binary: "geom-num::wide".into(),
                    test: "bad".into(),
                    outcome: Outcome::Failed("test failure with exit code 1".into()),
                },
                Reported {
                    binary: "geom-num::wide".into(),
                    test: "gone".into(),
                    outcome: Outcome::Failed("execution failure".into()),
                },
            ]
        );
        assert!(read_junit("<testsuites><testcase name=\"x\"/></testsuites>").is_err());
        assert!(read_junit("not xml").is_err());
        assert_eq!(millis("0.0001"), Ok(1));
        assert_eq!(millis("12"), Ok(12_000));
        assert!(millis("1.2.3").is_err() && millis("").is_err() && millis("-1").is_err());
    }

    fn reported(binary: &str, test: &str, outcome: Outcome) -> Reported {
        Reported {
            binary: binary.into(),
            test: test.into(),
            outcome,
        }
    }

    #[test]
    fn a_measuring_run_writes_each_state_and_refuses_a_failure() {
        assert_eq!(parse_scale("2.3"), Ok((23, 10)));
        assert_eq!(parse_scale("1"), Ok((1, 1)));
        for bad in ["", "0", "0.0", "-1", "2,3", "1.2.3", ".5", "1.0000001"] {
            assert!(parse_scale(bad).is_err(), "{bad:?}");
        }
        let before = [
            row("a", "kept", State::Finishes(5)),
            row("a", "measured", State::Finishes(500)),
        ];
        let report = [
            // 48.929 s x 2.3 = 112.54 s, rounded up.
            reported("a", "measured", Outcome::Passed(48_929)),
            reported("a", "stopped", Outcome::TimedOut(120_004)),
            // A pass at the stop was stopped there (a profile that counts that as a pass).
            reported("a", "at_the_stop", Outcome::Passed(120_000)),
            // Under the stop, but 300 s x 2.3 is over the budget.
            reported("b", "long", Outcome::Passed(119_000)),
        ];
        let after = measured(&before, &report, (23, 10), 120).unwrap();
        assert_eq!(
            after,
            [
                row("a", "at_the_stop", State::NativeOnly(Measured::Over(276))),
                row("a", "kept", State::Finishes(5)),
                row("a", "measured", State::Finishes(113)),
                row("a", "stopped", State::NativeOnly(Measured::Over(277))),
                row("b", "long", State::Finishes(274)),
            ]
        );
        let tight = measured(&[], &report[3..], (6, 1), 300).unwrap();
        assert_eq!(
            tight,
            [row("b", "long", State::NativeOnly(Measured::Took(714)))]
        );
        // A failure or a kill is named, and nothing is written.
        let bad = [
            reported("a", "ub", Outcome::Failed("test failure".into())),
            reported("a", "measured", Outcome::Passed(1)),
            reported("c", "killed", Outcome::Failed("test abort".into())),
        ];
        let e = measured(&before, &bad, (1, 1), 120).unwrap_err();
        assert!(
            e.starts_with("2 tests") && e.contains("a ub: test failure") && e.contains("c killed"),
            "{e}"
        );
        let twice = [report[0].clone(), report[0].clone()];
        assert!(measured(&before, &twice, (1, 1), 120).is_err());
    }

    #[test]
    fn the_table_is_rewritten_from_a_report_and_no_row_stays_at_the_budget() {
        // The form before 32.58: a row at the budget, whether stopped there or never timed.
        let old = "# head\n600\ta\tstopped\n12\ta\tquick\n600\ta\tgone\n600\tb\tnever\n";
        let report = [
            reported("a", "stopped", Outcome::TimedOut(120_000)),
            reported("b", "never", Outcome::Passed(10_000)),
        ];
        // `a gone` was renamed: no report names it, so it stays at the budget and is refused...
        let e = write_table(old, &report, (1, 1), 120, None, None).unwrap_err();
        assert!(
            e.starts_with("1 rows still at the budget") && e.contains("a gone"),
            "{e}"
        );
        // ...unless the listing shows it no longer exists, when its row goes.
        let listed: Vec<(String, String)> = [("a", "stopped"), ("a", "quick"), ("b", "never")]
            .iter()
            .map(|(b, t)| ((*b).to_owned(), (*t).to_owned()))
            .collect();
        let text = write_table(old, &report, (1, 1), 120, Some(&listed), Some("run 1")).unwrap();
        assert_eq!(
            text,
            "# head\n# run 1\n12\ta\tquick\nnative-only over 120\ta\tstopped\n10\tb\tnever\n"
        );
        // The tool's own output reads back, and rewriting it with no report changes nothing.
        assert!(parse_table(&text).is_ok());
        assert_eq!(write_table(&text, &[], (1, 1), 120, None, None), Ok(text));
        assert!(parse_old("600\ta\tx\n3\ta\tx\n").is_err());
    }

    #[test]
    fn a_library_needs_a_finishing_test_that_drives_it() {
        let rows = [
            row("a", "quick", State::Finishes(3)),
            row("a", "slow", State::NativeOnly(Measured::Over(276))),
        ];
        let libs = [
            lib("driven", "driven", "a", "quick"),
            // Its test is native-only, or has no row: nothing drives it under Miri.
            lib("stopped", "stopped", "a", "slow"),
            lib("unmeasured", "unmeasured", "a", "absent"),
            // The pool's tests have rows too: one with none drives nothing.
            lib("pool", "pool", "geom-pool::attack", "absent"),
        ];
        assert_eq!(
            undriven(&rows, &libs),
            [
                "stopped: a slow",
                "unmeasured: a absent",
                "pool: geom-pool::attack absent"
            ]
        );
        assert_eq!(crate_of("geom-pool::attack_g33"), "geom-pool");
        assert_eq!(crate_of("geom-num"), "geom-num");
    }

    #[test]
    fn every_listed_test_needs_a_row() {
        let listing = serde_json::json!({"rust-suites": {
            "a": {"testcases": {
                "x": {"ignored": false, "filter-match": {"status": "matches"}},
                "y": {"ignored": true, "filter-match": {"status": "matches"}},
                "z": {"ignored": false, "filter-match": {"status": "mismatch"}},
            }},
            "b": {"testcases": {"w": {"ignored": false}}},
        }});
        let listed = listed_tests(&listing).unwrap();
        assert_eq!(
            listed,
            [("a".to_owned(), "x".to_owned()), ("b".into(), "w".into())]
        );
        let rows = [row("a", "x", State::NativeOnly(Measured::Over(276)))];
        // A test with no row at all.
        assert_eq!(unlisted(&rows, &listed), ["b w"]);
        assert!(listed_tests(&serde_json::json!({})).is_err());
        assert!(listed_tests(&serde_json::json!({"rust-suites": {"a": {}}})).is_err());
    }

    /// The names a filterset gives, as (binary, test).
    fn named(filter: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for term in filter.split(" | ") {
            let term = term.trim_start_matches('(').trim_end_matches(')');
            let Some(rest) = term.strip_prefix("binary_id(=") else {
                continue;
            };
            let (binary, rest) = rest.split_once(')').unwrap();
            let test = rest.strip_prefix(" & test(=").unwrap();
            out.push((binary.to_owned(), test.to_owned()));
        }
        out
    }

    #[test]
    fn every_finishing_test_is_in_one_partition_and_the_longest_go_first() {
        let tests = [
            timed(590, "a", "slow"),
            timed(590, "a", "slow2"),
            timed(500, "b", "mid"),
            timed(100, "b", "quick"),
            timed(90, "c", "x"),
            timed(10, "c", "y"),
        ];
        // Within the stop alone, four at once: the first four, the longest first, start together
        // and end by 590 s; with a fifth the last could start after 590 + 590 + 500 + 100 over
        // four slots, so the last two go to a second part.
        let p = pack(&tests, Rule::Stop).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(
            p.tests_of(1),
            [("a", "slow"), ("a", "slow2"), ("b", "mid"), ("b", "quick")]
        );
        assert_eq!(p.tests_of(2), [("c", "x"), ("c", "y")]);
        assert!(p.tests_of(0).is_empty() && p.tests_of(3).is_empty());
        assert_eq!(p.longest(), 590);
        assert_eq!(p.binaries(1), 2);
        assert!(p.ends_within(1, Rule::Stop) && p.ends_within(2, Rule::Stop));
        assert!(!p.ends_within(3, Rule::Stop));
        // Each test once, over all partitions' filtersets, and nothing else.
        let mut seen = Vec::new();
        for k in 1..=2 {
            seen.extend(named(&p.filterset(k).unwrap()));
        }
        seen.sort();
        let mut want: Vec<(String, String)> = tests
            .iter()
            .map(|t| (t.binary.clone(), t.test.clone()))
            .collect();
        want.sort();
        assert_eq!(seen, want);
        assert_eq!(
            p.filterset(2).unwrap(),
            "(binary_id(=c) & test(=x)) | (binary_id(=c) & test(=y))"
        );
        assert!(p.filterset(0).is_err() && p.filterset(3).is_err());
        // No tests: one partition, which runs nothing.
        let none = pack(&[], Rule::Job).unwrap();
        assert_eq!(none.len(), 1);
        assert_eq!(none.filterset(1).unwrap(), "none()");
        // A job's partitions: the same split, each test planned on a runner 1.6 times slower and
        // never over the stop (600, 600, 600, 160 s, then 144 and 16 s), with its binaries'
        // listing within PART_SECONDS.
        let p = pack(&tests, Rule::Job).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p.tests_of(2), [("c", "x"), ("c", "y")]);
        assert!((1..=2).all(|k| p.ends_within(k, Rule::Job)));
        assert_eq!(p.longest(), BUDGET);
        assert_eq!(p.part_seconds(2), 144);
        // Many quick tests of many binaries: the listing fills the job first. 370 tests of 1 s
        // (2 s planned) in 370 binaries: 187 s of tests at most and 592 s of listing fit; 371 do
        // not.
        let wide: Vec<Timed> = (0..371).map(|i| timed(1, &format!("w{i}"), "t")).collect();
        assert_eq!(pack(&wide[..370], Rule::Job).unwrap().len(), 1);
        assert_eq!(pack(&wide, Rule::Job).unwrap().len(), 2);
        // Four tests of 500 s, planned at the stop, start at once and end by 600 s; a fifth may
        // start only then.
        let many: Vec<Timed> = (0..40).map(|i| timed(500, "d", &format!("t{i}"))).collect();
        let p = pack(&many, Rule::Job).unwrap();
        assert_eq!(p.len(), 10);
        assert!((1..=10).all(|k| p.tests_of(k).len() == 4));
        // A test that does not finish fits no job; one that finishes fits alone, planned at the
        // stop, with its binary's listing.
        assert!(pack(&[timed(901, "e", "alone")], Rule::Job).is_none());
        assert!(pack(&[timed(BUDGET, "e", "alone")], Rule::Job).is_none());
        let alone = pack(&[timed(BUDGET - 1, "e", "alone")], Rule::Job).unwrap();
        assert_eq!(alone.len(), 1);
        assert_eq!(alone.part_seconds(1), BUDGET);
    }

    #[test]
    fn a_partition_ends_within_its_bound_in_any_order() {
        // Five tests: 4 x 100 s and 300 s. Whatever order nextest starts them in, the last starts
        // once the slots have run the other 400 s at most, 100 s each, and ends 300 s later.
        let tests: Vec<Timed> = (0..4)
            .map(|i| timed(100, "a", &format!("t{i}")))
            .chain([timed(300, "a", "long")])
            .collect();
        assert_eq!(any_order(700, 300, 5), 400);
        // The other order's worst case, started by hand: the four short ones fill the slots and
        // end at 100 s, then the long one runs alone to 400 s.
        let worst = 100 + 300;
        assert!(worst <= any_order(700, 300, 5));
        // Four tests or fewer all start at once.
        assert_eq!(any_order(400, 100, 4), 100);
        let p = pack(&tests, Rule::Stop).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p.part_seconds(1), 400);
        assert_eq!(p.longest(), 400);
        assert_eq!(p.part_seconds(0), 0);
        assert_eq!(p.part_seconds(2), 0);
        assert_eq!(p.binaries(2), 0);
        // A job's listing counts, on the slower runner: 780 s of tests and listing in all; 575 s
        // of tests here leave 205 s, 128 binaries' listing at 1.6 s each, rounded up.
        assert_eq!(any_order(2_000, 100, 20), 575);
        assert!(within(Rule::Job, 2_000, 100, 20, 128));
        assert!(!within(Rule::Job, 2_000, 100, 20, 129));
        // The longest a test is planned at, the stop, fits alone, with its binary's listing.
        assert!(within(Rule::Job, BUDGET, BUDGET, 1, 1));
        assert!(within(Rule::Stop, 600, 600, 1, 1));
        assert!(!within(Rule::Stop, 601, 601, 1, 1));
        // Planned: 1.6 times, rounded up, and never over the stop, which nextest holds.
        assert_eq!(SLOWER, (8, 5));
        assert_eq!(planned(0), 0);
        assert_eq!(planned(1), 2);
        assert_eq!(planned(5), 8);
        assert_eq!(planned(375), BUDGET);
        assert_eq!(planned(374), 599);
        assert_eq!(planned(BUDGET - 1), BUDGET);
        // The bound is what a job has left after its set-up, and it holds any finishing test
        // alone, planned at the stop, so none leaves Miri for want of a partition.
        assert_eq!(FIXED_SECONDS + PART_SECONDS, JOB_SECONDS);
        assert!(planned(BUDGET - 1) + on_a_slower_runner(LIST_SECONDS) <= PART_SECONDS);
    }

    #[test]
    fn the_gate_gives_geom_pool_its_own_part_and_the_rest_the_partitions() {
        let rows = [
            row("geom-pool::attack_g33", "quick", State::Finishes(20)),
            row(
                "geom-pool::attack_g33",
                "slow",
                State::NativeOnly(Measured::Over(601)),
            ),
            row("geom-pool", "unit", State::Finishes(3)),
            row("geom-num", "a", State::Finishes(590)),
            row("geom-num", "b", State::Finishes(10)),
            row("geom-op::x", "c", State::NativeOnly(Measured::Over(601))),
        ];
        let g = gate_plan(&rows).unwrap();
        // geom-pool's finishing tests in its part, its native-only one in none.
        let mut pool = g.pool.tests_of(1);
        pool.sort_unstable();
        assert_eq!(
            pool,
            [("geom-pool", "unit"), ("geom-pool::attack_g33", "quick")]
        );
        assert_eq!(g.pool.len(), 1);
        // The rest: 590 s and 10 s start together, with one binary to list.
        assert_eq!(g.parts.len(), 1);
        assert_eq!(g.parts.tests_of(1), [("geom-num", "a"), ("geom-num", "b")]);
        // No geom-pool row: an empty part, which runs nothing.
        let g = gate_plan(&rows[3..]).unwrap();
        assert_eq!(g.pool.filterset(1).unwrap(), "none()");
        // geom-pool's tests over its one job's bound are refused.
        let crowded: Vec<Row> = (0..5)
            .map(|i| row("geom-pool", &format!("t{i}"), State::Finishes(500)))
            .collect();
        assert!(gate_plan(&crowded).is_err_and(|e| e.contains("geom-pool")));
        assert!(gate_plan(&crowded[..4]).is_ok());
        // More partitions than one matrix holds are refused: tests of 599 s, four a partition.
        let most = usize::try_from(crate::mutants::MATRIX_JOBS).unwrap();
        let wide: Vec<Row> = (0..=4 * most)
            .map(|i| row("geom-num", &format!("t{i}"), State::Finishes(599)))
            .collect();
        assert!(gate_plan(&wide).is_err_and(|e| e.contains("matrix")));
        assert_eq!(gate_plan(&wide[..4 * most]).unwrap().parts.len(), most);
    }

    #[test]
    fn the_recorded_times_pack_into_jobs_that_end_in_time() {
        let rows = parse_table(TIMES).unwrap();
        assert!(rows.len() > 2000, "{}", rows.len());
        let g = gate_plan(&rows).unwrap();
        // Every finishing test once, geom-pool's in its own part.
        let mut seen: Vec<(&str, &str)> = (1..=g.parts.len())
            .flat_map(|k| g.parts.tests_of(k))
            .chain(g.pool.tests_of(1))
            .collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), before, "a test in two parts");
        assert_eq!(seen.len(), finishing(&rows).len());
        assert!(
            (1..=g.parts.len()).all(|k| g.parts.ends_within(k, Rule::Job)),
            "a partition past its job"
        );
        assert!(g.pool.ends_within(1, Rule::Job));
        assert!(
            g.pool
                .tests_of(1)
                .iter()
                .all(|(b, _)| crate_of(b) == TREE_BORROWS_CRATE)
        );
    }

    fn listed(names: &[(&str, &str)]) -> Vec<(String, String)> {
        names
            .iter()
            .map(|(b, t)| ((*b).to_owned(), (*t).to_owned()))
            .collect()
    }

    #[test]
    fn a_measuring_run_picks_its_tests_and_plans_them_within_the_stop() {
        let rows = [
            row("a", "quick", State::Finishes(30)),
            row("a", "slow", State::NativeOnly(Measured::Over(276))),
            row("a", "long", State::NativeOnly(Measured::Took(700))),
            row("a", "gone", State::Finishes(5)),
        ];
        let run = listed(&[("a", "quick"), ("a", "slow"), ("a", "long"), ("b", "new")]);
        // New: only the test with no row, planned at the stop.
        assert_eq!(
            to_measure(&rows, &run, Measuring::New),
            [timed(BUDGET, "b", "new")]
        );
        // All: every listed test, a finishing one at its seconds, the rest at the stop; a row the
        // listing lacks is not measured.
        assert_eq!(
            to_measure(&rows, &run, Measuring::All),
            [
                timed(30, "a", "quick"),
                timed(BUDGET, "a", "slow"),
                timed(BUDGET, "a", "long"),
                timed(BUDGET, "b", "new"),
            ]
        );
        // Four tests at the stop fill a part; a fifth needs a second.
        let at_stop: Vec<Timed> = (0..5)
            .map(|i| timed(BUDGET, "c", &format!("t{i}")))
            .collect();
        assert_eq!(measure_plan(&at_stop[..4]).unwrap().len(), 1);
        let p = measure_plan(&at_stop).unwrap();
        assert_eq!(p.len(), 2);
        assert!(p.longest() <= BUDGET);
        let mut seen: Vec<(&str, &str)> = (1..=2).flat_map(|k| p.tests_of(k)).collect();
        seen.sort_unstable();
        assert_eq!(seen.len(), 5, "every test in exactly one part");
        seen.dedup();
        assert_eq!(seen.len(), 5);
        // Quick tests share a part up to the stop at four at once, in any order: of 97 tests of
        // 24 s the last starts by 96 x 24 / 4 = 576 s and ends by 600 s; a 98th needs a second.
        let quick: Vec<Timed> = (0..98).map(|i| timed(24, "d", &format!("t{i}"))).collect();
        assert_eq!(measure_plan(&quick[..97]).unwrap().len(), 1);
        assert_eq!(measure_plan(&quick).unwrap().len(), 2);
        // No test: one empty part, which runs nothing.
        assert_eq!(measure_plan(&[]).unwrap().filterset(1).unwrap(), "none()");
        // Too many parts for the two part jobs: refused, to be narrowed.
        let most = usize::try_from(crate::mutants::MAX_SHARDS).unwrap();
        let fits: Vec<Timed> = (0..4 * most)
            .map(|i| timed(BUDGET, "e", &format!("t{i}")))
            .collect();
        assert_eq!(measure_plan(&fits).unwrap().len(), most);
        let over: Vec<Timed> = (0..=4 * most)
            .map(|i| timed(BUDGET, "e", &format!("t{i}")))
            .collect();
        assert!(
            measure_plan(&over).is_err_and(|e| e.contains("narrow the run with a filterset")),
            "more parts than the jobs hold"
        );
        assert!(measure_plan(&[timed(BUDGET + 1, "f", "x")]).is_err());
    }

    #[test]
    fn a_measuring_matrix_numbers_its_parts_from_one_in_two_jobs() {
        let read = |s: String| serde_json::from_str::<Value>(&s).unwrap();
        let m = read(measure_matrix(0, 0));
        assert_eq!(m["count"], 0);
        assert_eq!(m["parts"], serde_json::json!([1]));
        assert_eq!(m["count_2"], 0);
        assert_eq!(m["parts_2"], serde_json::json!([257]));
        let m = read(measure_matrix(9, 3));
        assert_eq!(m["tests"], 9);
        assert_eq!(m["parts"], serde_json::json!([1, 2, 3]));
        let m = read(measure_matrix(2000, 300));
        let first: Vec<u64> = (1..=256).collect();
        let second: Vec<u64> = (257..=300).collect();
        assert_eq!(m["parts"], serde_json::json!(first));
        assert_eq!(m["count_2"], 44);
        assert_eq!(m["parts_2"], serde_json::json!(second));
    }
}
