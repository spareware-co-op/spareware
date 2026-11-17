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
//! `kernel-quality miri-times` command). [`plan`] splits the finishing tests into the partitions
//! by their times, and [`Plan::filterset`] writes partition k's tests as the nextest filterset its
//! job runs; [`unlisted`] names every test of a nextest list ([`listed_tests`]) the table has no
//! row for, and the workflow's filter step fails on any. [`to_measure`] picks the tests a
//! measuring run on GitHub's runners measures (the workflow's measuring mode: the tests with no
//! row, or every test), each planned at its finishing row's seconds or at the stop, and
//! [`measure_plan`] packs them into the fewest parts that each end within the stop.
//!
//! Guarantees:
//!
//! - **Every test Miri finishes, and only those.** Each kernel test has a row. A test Miri finishes
//!   within the measuring run's stop has its measured seconds, always under [`BUDGET`]; a test
//!   measured over it is native-only, with its measured time or the stop it ran past, and its
//!   partition's filterset leaves it out, so it runs in every native suite and not under Miri. A
//!   test with no row fails the filter step ([`unlisted`]): nothing runs under Miri by default,
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
//! - **Balanced.** Longest processing time first (Graham): each test, the longest first, goes to
//!   the partition with the least time so far, so no partition passes the average by more than its
//!   longest test. With [`AT_ONCE`] tests at a time, [`Plan::longest`] is how long the slowest
//!   partition's tests take, which `kernel_quality_workflow.rs` holds under [`PART_SECONDS`] with
//!   the fewest partitions that do.
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
//! is read with roxmltree. The partitions by R. L. Graham, "Bounds on multiprocessing timing
//! anomalies", SIAM Journal on Applied Mathematics 17(2) (1969), 416–429,
//! <https://doi.org/10.1137/0117039> (the longest-processing-time rule, within 4/3 of the best).
//!
//! Cost: linear in the size of the metadata, the table, the list and the report; a plan is
//! O(t log t + t·p) for t tests and p partitions, and a filterset linear in its partition's tests.

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

/// The longest a partition's tests may take, in seconds (the owner's testing principle 8: no run
/// takes more than a few minutes).
pub const PART_SECONDS: u64 = 900;

/// The budget of one test under Miri, in seconds: the `default-miri` profile of
/// `.config/nextest.toml` stops a test still running then. A finishing row is always under it.
pub const BUDGET: u64 = 600;

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
/// has a finishing row, or belongs to `geom-pool`, whose tests Miri runs whole under Tree Borrows,
/// and that every such package of `Cargo.lock` is named.
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
/// test`. A test of the crate `whole` needs none: Miri runs every test of that crate, outside the
/// table (geom-pool, under Tree Borrows).
#[must_use]
pub fn undriven(rows: &[Row], libraries: &[Library], whole: &str) -> Vec<String> {
    let finishing: BTreeSet<(&str, &str)> = rows
        .iter()
        .filter(|r| matches!(r.state, State::Finishes(_)))
        .map(|r| (r.binary.as_str(), r.test.as_str()))
        .collect();
    let crate_of = |binary: &str| binary.split("::").next().unwrap_or(binary).to_owned();
    libraries
        .iter()
        .filter(|l| crate_of(l.binary) != whole && !finishing.contains(&(l.binary, l.test)))
        .map(|l| format!("{}: {} {}", l.name, l.binary, l.test))
        .collect()
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
    let (mut comments, rows, pending) = parse_old(old)?;
    if let Some(note) = note {
        comments.push(format!("# {note}"));
    }
    let rows = measured(&rows, report, scale, stop)?;
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
    let stop_ms = stop.checked_mul(1000).ok_or("stop overflows")?;
    let mut table: BTreeMap<(String, String), State> = rows
        .iter()
        .map(|r| ((r.binary.clone(), r.test.clone()), r.state))
        .collect();
    let mut failed = Vec::new();
    let mut seen = BTreeSet::new();
    for r in report {
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

/// The plan of `parts` partitions for `tests` (module documentation).
///
/// # Errors
///
/// For no partitions.
pub fn plan(tests: &[Timed], parts: usize) -> Result<Plan, String> {
    if parts == 0 {
        return Err("a plan of no partitions runs nothing".into());
    }
    let mut order = tests.to_vec();
    order.sort_by(|x, y| {
        y.seconds
            .cmp(&x.seconds)
            .then_with(|| (&x.binary, &x.test).cmp(&(&y.binary, &y.test)))
    });
    let mut out: Vec<(Vec<Timed>, u64)> = vec![(Vec::new(), 0); parts];
    for t in order {
        // The partition with the least time so far, the first of equals.
        let least = out
            .iter_mut()
            .enumerate()
            .min_by_key(|(p, part)| (part.1, *p))
            .map(|(_, part)| part)
            .ok_or("a plan of no partitions runs nothing")?;
        least.1 = least.1.saturating_add(t.seconds);
        least.0.push(t);
    }
    Ok(Plan { parts: out })
}

/// The fewest partitions whose plan keeps every partition within [`PART_SECONDS`], searched from
/// the least the total allows; `None` if a test alone takes longer.
#[must_use]
pub fn fewest(tests: &[Timed]) -> Option<usize> {
    fewest_within(tests, PART_SECONDS)
}

/// The fewest partitions whose plan keeps every partition within `bound` seconds at [`AT_ONCE`] a
/// time, searched from the least the total allows; `None` if a test alone takes longer, or for a
/// bound of zero.
#[must_use]
pub fn fewest_within(tests: &[Timed], bound: u64) -> Option<usize> {
    let total = tests.iter().fold(0_u64, |a, t| a.saturating_add(t.seconds));
    let per = AT_ONCE.checked_mul(bound).filter(|&p| p > 0)?;
    let least = usize::try_from(total.div_ceil(per).max(1)).ok()?;
    let most = tests.len().max(1);
    (least..=most).find(|&n| plan(tests, n).is_ok_and(|p| p.longest() <= bound))
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

/// The parts of a measuring run: the fewest whose tests each take at most [`BUDGET`] at
/// [`AT_ONCE`] a time, balanced as [`plan`] balances the partitions. No tests give one empty
/// part.
///
/// # Errors
///
/// For more parts than the workflow's part jobs hold ([`crate::mutants::MAX_SHARDS`]): the run is
/// to be narrowed by a filterset. Also for a test planned over the stop, which [`to_measure`]
/// never gives.
pub fn measure_plan(tests: &[Timed]) -> Result<Plan, String> {
    let n = fewest_within(tests, BUDGET).ok_or_else(|| {
        format!("a test is planned over the {BUDGET} s stop, which no part can hold")
    })?;
    let most = usize::try_from(crate::mutants::MAX_SHARDS).unwrap_or(usize::MAX);
    if n > most {
        return Err(format!(
            "{} tests need {n} parts of at most {BUDGET} s, more than the workflow's {most}: \
             narrow the run with a filterset (the input miri_measure_tests)",
            tests.len()
        ));
    }
    plan(tests, n)
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
    /// How many partitions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Whether the plan has no partitions (never: [`plan`] refuses that).
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

    /// The seconds its tests take, at [`AT_ONCE`] at a time, in the slowest partition: the longer
    /// of its total over [`AT_ONCE`], rounded up, and its longest test.
    #[must_use]
    pub fn longest(&self) -> u64 {
        self.parts
            .iter()
            .map(|(tests, total)| {
                let longest_test = tests.iter().map(|t| t.seconds).max().unwrap_or(0);
                total.div_ceil(AT_ONCE).max(longest_test)
            })
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
            // The pool's tests all run under Tree Borrows, outside the table.
            lib("pool", "pool", "geom-pool::attack", "absent"),
            lib("pool-unit", "pool", "geom-pool", "absent"),
        ];
        assert_eq!(
            undriven(&rows, &libs, "geom-pool"),
            ["stopped: a slow", "unmeasured: a absent"]
        );
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
        let p = plan(&tests, 3).unwrap();
        assert_eq!(p.len(), 3);
        // Longest first, each to the partition with the least so far, the first of equals: 590,
        // 590 and 500 one each; 100 to the third (500); 90 to the first (both at 590); 10 to the
        // second (590 against the first's 680).
        assert_eq!(p.tests_of(1), [("a", "slow"), ("c", "x")]);
        assert_eq!(p.tests_of(2), [("a", "slow2"), ("c", "y")]);
        assert_eq!(p.tests_of(3), [("b", "mid"), ("b", "quick")]);
        assert!(p.tests_of(0).is_empty() && p.tests_of(4).is_empty());
        // At four at once a partition takes at least its longest test.
        assert_eq!(p.longest(), 590);
        // Each test once, over all partitions' filtersets, and nothing else.
        let mut seen = Vec::new();
        for k in 1..=3 {
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
            p.filterset(3).unwrap(),
            "(binary_id(=b) & test(=mid)) | (binary_id(=b) & test(=quick))"
        );
        assert!(p.filterset(0).is_err() && p.filterset(4).is_err());
        assert!(plan(&tests, 0).is_err());
        // No tests: every partition runs nothing.
        assert_eq!(plan(&[], 2).unwrap().filterset(1).unwrap(), "none()");
        // The fewest partitions within the bound: 1,880 s is 470 s a partition at four at once.
        assert_eq!(fewest(&tests), Some(1));
        let many: Vec<Timed> = (0..40).map(|i| timed(500, "d", &format!("t{i}"))).collect();
        // 20,000 s over 3,600 s a partition: at least 6; LPT puts 7 or 6 tests of 500 s in each.
        let n = fewest(&many).unwrap();
        assert!(plan(&many, n).unwrap().longest() <= PART_SECONDS);
        assert!(plan(&many, n - 1).unwrap().longest() > PART_SECONDS);
        assert_eq!(fewest(&[timed(901, "e", "alone")]), None);
    }

    #[test]
    fn the_recorded_times_read_and_balance() {
        let rows = parse_table(TIMES).unwrap();
        assert!(rows.len() > 2000, "{}", rows.len());
        let tests = finishing(&rows);
        // With sixteen partitions, no partition passes the average by more than its longest test.
        let total: u64 = tests.iter().fold(0, |a, t| a + t.seconds);
        let p = plan(&tests, 16).unwrap();
        let average = total.div_ceil(16 * AT_ONCE);
        assert!(p.longest() <= average + BUDGET, "{} {average}", p.longest());
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
        // Quick tests share a part up to the stop at four at once: 100 of 24 s is 600 s.
        let quick: Vec<Timed> = (0..101).map(|i| timed(24, "d", &format!("t{i}"))).collect();
        assert_eq!(measure_plan(&quick[..100]).unwrap().len(), 1);
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
        assert_eq!(fewest_within(&at_stop, 0), None);
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
