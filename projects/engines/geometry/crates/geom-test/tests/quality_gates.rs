//! The mutation and coverage gates (piece G21, section 28.8) fail when they should and pass only
//! when they should.
//!
//! Each test builds a small repository in a temporary folder (kernel crates, a development crate,
//! sources with known lines), writes cargo-mutants output directories and LCOV files the way
//! cargo-mutants 27.1 and cargo-llvm-cov 0.9.1 write them, and judges them with `geom_test`'s gates.
//!
//! - **Seeded fixtures.** `random_mutation_runs_are_judged_as_the_rules_say` and
//!   `random_coverage_is_judged_as_the_bars_say` draw hundreds of runs from one printed seed
//!   (`SPAREKIT_SEED` replays any) and compare every verdict with an oracle written here from 28.8's
//!   rules alone, sharing no code with the gates.
//! - **Every way to fail.** The other tests seed one fault each, which the gate must report: an
//!   unexplained survivor, a kind the foundation set may not keep, a stale entry, a missing shard,
//!   a truncated or failed run, an unviable mutant that never compiled, a line below the bar, a
//!   branch not taken, a run without branch data, a mark without a proof, and more.
//! - **The binary.** `kernel-quality` itself is run on fixtures: exit 0 to pass, 1 to fail, 2 for
//!   input it cannot use.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// Fixtures are written for reading side by side with the rules they test: short local names
// (`a`, `b`, `l`), slices of one run directory, string building and long fixture tables.
#![allow(
    clippy::similar_names,
    clippy::format_collect,
    clippy::format_push_string,
    clippy::needless_pass_by_value,
    clippy::match_same_arms,
    clippy::cloned_ref_to_slice_refs,
    clippy::type_complexity,
    clippy::too_many_lines,
    clippy::assert_is_empty,
    reason = "test fixtures; the rules are for kernel code"
)]

use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use geom_test::coverage::{self, check_log, check_marks, excuse_awaiting, measure};
use geom_test::foundation::{Place, Set, place};
use geom_test::mutants::{self, Mode, judge, read_run};
use geom_test::quality_ledger::{Kind, Ledger, SCHEMA};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};
use serde_json::{Value, json};

const K: &str = "projects/engines/geometry/crates";
const NUM: &str = "projects/engines/geometry/crates/geom-num/src/a.rs";
const BOOL: &str = "projects/engines/geometry/crates/geom-bool/src/lib.rs";
const REASON: &str =
    "No input through the public interface can tell this mutant from the original.";

// ---------------------------------------------------------------------------------------------
// A repository in a temporary folder
// ---------------------------------------------------------------------------------------------

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A temporary repository, removed when dropped.
struct Repo {
    root: PathBuf,
}

impl Repo {
    /// An empty repository with three kernel crates (geom-num, geom-bool, geom-topo) and a
    /// development crate (geom-test), each with a manifest.
    fn new(label: &str) -> Repo {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "geom-test-quality-{label}-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = Repo {
            root: std::fs::canonicalize(&dir).unwrap(),
        };
        for krate in ["geom-num", "geom-bool", "geom-topo", "geom-test"] {
            repo.write(&format!("{K}/{krate}/Cargo.toml"), "[package]\n");
        }
        repo
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.path(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Rust source whose line `i` (from 1) is `pub const X{i}: u32 = {i};`, so every line's text is
/// unique and the file parses.
fn numbered_source(lines: usize) -> String {
    (1..=lines)
        .map(|i| format!("pub const X{i}: u32 = {i};\n"))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// cargo-mutants output, as version 27.1 writes it
// ---------------------------------------------------------------------------------------------

/// One mutant in cargo-mutants' JSON form.
fn mutant(file: &str, line: usize, column: usize, description: &str) -> Value {
    json!({
        "name": format!("{file}:{line}:{column}: {description}"),
        "package": "geom-num",
        "file": file,
        "function": null,
        "span": {"start": {"line": line, "column": column}, "end": {"line": line, "column": column + 1}},
        "replacement": "x",
        "genre": "BinaryOperator",
    })
}

/// What happened to a mutant, as cargo-mutants' `summary` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Summary {
    Caught,
    Missed,
    Timeout,
    Unviable,
}

impl Summary {
    fn text(self) -> &'static str {
        match self {
            Summary::Caught => "CaughtMutant",
            Summary::Missed => "MissedMutant",
            Summary::Timeout => "Timeout",
            Summary::Unviable => "Unviable",
        }
    }
}

const RUSTC_ERROR: &str =
    "*** cargo test --no-run\nerror[E0308]: mismatched types\n  --> src/a.rs:3:5\n   |\n";

/// Writes a whole cargo-mutants output directory `<root>/<name>/mutants.out`.
fn write_run(repo: &Repo, name: &str, mutants: &[(Value, Summary)]) -> PathBuf {
    let dir = repo.path(&format!("out/{name}/mutants.out"));
    std::fs::create_dir_all(dir.join("log")).unwrap();
    let list: Vec<Value> = mutants.iter().map(|(m, _)| m.clone()).collect();
    std::fs::write(
        dir.join("mutants.json"),
        serde_json::to_string(&list).unwrap(),
    )
    .unwrap();
    if mutants.is_empty() {
        return dir;
    }
    let mut outcomes =
        vec![json!({"scenario": "Baseline", "summary": "Success", "log_path": "log/baseline.log"})];
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    for (i, (m, s)) in mutants.iter().enumerate() {
        let log = format!("log/m{i}.log");
        let text = if *s == Summary::Unviable {
            RUSTC_ERROR
        } else {
            "*** test\nok\n"
        };
        std::fs::write(dir.join(&log), text).unwrap();
        outcomes.push(json!({"scenario": {"Mutant": m}, "summary": s.text(), "log_path": log}));
        *counts.entry(s.text()).or_default() += 1;
    }
    let get = |k: &str| counts.get(k).copied().unwrap_or(0);
    let value = json!({
        "outcomes": outcomes,
        "total_mutants": mutants.len(),
        "missed": get("MissedMutant"),
        "caught": get("CaughtMutant"),
        "timeout": get("Timeout"),
        "unviable": get("Unviable"),
        "success": 0,
        "start_time": "2026-10-22T00:00:00Z",
        "end_time": "2026-10-22T01:00:00Z",
        "cargo_mutants_version": "27.1.0",
    });
    std::fs::write(
        dir.join("outcomes.json"),
        serde_json::to_string(&value).unwrap(),
    )
    .unwrap();
    dir
}

/// Edits `outcomes.json` of a written run.
fn edit_outcomes(dir: &Path, edit: impl FnOnce(&mut Value)) {
    let path = dir.join("outcomes.json");
    let mut value: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    edit(&mut value);
    std::fs::write(&path, serde_json::to_string(&value).unwrap()).unwrap();
}

/// A ledger survivor entry.
fn entry(file: &str, line: usize, description: &str, occurrences: u64, kind: &str) -> Value {
    json!({
        "file": file,
        "mutant": description,
        "line_text": format!("pub const X{line}: u32 = {line};"),
        "occurrences": occurrences,
        "kind": kind,
        "reason": REASON,
    })
}

fn ledger(survivors: Vec<Value>) -> Ledger {
    let text = json!({"schema": SCHEMA, "survivors": survivors, "coverage_off": []}).to_string();
    Ledger::parse(&text).unwrap()
}

/// A repository with two numbered source files, one in the foundation set and one elsewhere.
fn mutation_repo(label: &str) -> Repo {
    let repo = Repo::new(label);
    repo.write(NUM, &numbered_source(40));
    repo.write(BOOL, &numbered_source(40));
    repo
}

/// Judges one written run against `survivors`, with a base branch whose ledger is the same (so
/// every `known` entry is an old one; `known_entries_must_come_from_the_base_branch` tests the
/// other case).
fn verdict(
    repo: &Repo,
    dirs: &[PathBuf],
    survivors: Vec<Value>,
    mode: Mode,
    expected: Option<&[Value]>,
) -> mutants::Verdict {
    let l = ledger(survivors);
    verdict_with_base(repo, dirs, &l, mode, expected, Some(&l))
}

/// Judges one written run against `ledger`, with `base` as the base branch's ledger.
fn verdict_with_base(
    repo: &Repo,
    dirs: &[PathBuf],
    ledger: &Ledger,
    mode: Mode,
    expected: Option<&[Value]>,
    base: Option<&Ledger>,
) -> mutants::Verdict {
    let runs: Vec<_> = dirs.iter().map(|d| read_run(d).unwrap()).collect();
    let expected: Option<Vec<mutants::Mutant>> = expected.map(|e| {
        e.iter()
            .map(|m| mutants::mutant_from_json(m).unwrap())
            .collect()
    });
    judge(&runs, ledger, &repo.root, mode, expected.as_deref(), base)
}

// ---------------------------------------------------------------------------------------------
// The mutation gate, seeded
// ---------------------------------------------------------------------------------------------

/// What the oracle needs to know about one drawn mutant.
struct Drawn {
    file: &'static str,
    foundation: bool,
    line: usize,
    description: &'static str,
    summary: Summary,
}

/// 28.8's rules, written from scratch: whether a set of mutants and ledger entries passes.
fn oracle(
    drawn: &[Drawn],
    entries: &[(usize, &'static str, &'static str, u64, &'static str)],
    mode: Mode,
) -> bool {
    // An entry is keyed by (file, line, description).
    let mut ok = true;
    for &(_, file, _, _, kind) in entries {
        if file == NUM && (kind == "harmless" || kind == "known") {
            ok = false;
        }
    }
    let find = |d: &Drawn| {
        entries
            .iter()
            .find(|e| e.1 == d.file && e.0 == d.line && e.2 == d.description)
    };
    let mut used: BTreeMap<(usize, &str, &str), u64> = BTreeMap::new();
    for d in drawn {
        let e = find(d);
        match (d.summary, e) {
            (Summary::Caught | Summary::Unviable, Some(_)) => ok = false,
            (Summary::Caught | Summary::Unviable, None) => {}
            (Summary::Missed, Some(&(line, file, desc, _, kind))) => {
                let allowed = kind == "equivalent"
                    || (!d.foundation && (kind == "harmless" || kind == "known"));
                if allowed {
                    *used.entry((line, file, desc)).or_default() += 1;
                } else {
                    ok = false;
                }
            }
            (Summary::Timeout, Some(&(line, file, desc, _, kind))) => {
                if kind == "hangs" {
                    *used.entry((line, file, desc)).or_default() += 1;
                } else {
                    ok = false;
                }
            }
            (Summary::Missed | Summary::Timeout, None) => ok = false,
        }
    }
    for &(line, file, desc, occurrences, _) in entries {
        let n = used.get(&(line, file, desc)).copied().unwrap_or(0);
        let fine = match mode {
            Mode::Full => n == occurrences,
            Mode::Diff => n <= occurrences,
        };
        ok &= fine;
    }
    ok
}

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn random_mutation_runs_are_judged_as_the_rules_say() {
    let seed = resolve(
        "quality gates: random mutation runs",
        Seed(0x6732_315f_6d75_7461),
    )
    .unwrap();
    let mut rng = SplitMix64::new(seed);
    let repo = mutation_repo("random-mutants");
    let descriptions = [
        "replace < with <= in f",
        "replace + with - in f",
        "delete ! in g",
    ];
    let kinds = ["equivalent", "harmless", "hangs", "known"];
    let summaries = [
        Summary::Caught,
        Summary::Missed,
        Summary::Timeout,
        Summary::Unviable,
    ];
    let mut passes = 0;
    let mut fails = 0;
    for case in 0..300 {
        let mode = if rng.coin() { Mode::Full } else { Mode::Diff };
        // Half the cases are clean: every survivor gets an entry of an allowed kind with its exact
        // count, and every mutant of one key has one outcome; the rest are drawn freely.
        let clean = rng.coin();
        let count = 1 + below(&mut rng, 8) as usize;
        let mut drawn = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut outcome_of: BTreeMap<(&str, usize, &str), Summary> = BTreeMap::new();
        for _ in 0..count {
            let foundation = rng.coin();
            let file = if foundation { NUM } else { BOOL };
            let line = 1 + below(&mut rng, 6) as usize;
            let description = descriptions[below(&mut rng, 3) as usize];
            // Two columns per key at most, so a key can occur twice.
            let column = 1 + below(&mut rng, 2) as usize;
            if !seen.insert((file, line, column, description)) {
                continue;
            }
            // Mostly survivors, so the ledger has work to do.
            let mut summary = summaries[[1, 1, 1, 0, 2, 3][below(&mut rng, 6) as usize]];
            if clean {
                summary = *outcome_of
                    .entry((file, line, description))
                    .or_insert(summary);
            }
            drawn.push((
                Drawn {
                    file,
                    foundation,
                    line,
                    description,
                    summary,
                },
                column,
            ));
        }
        // Ledger entries: for most surviving keys, a random kind and usually the right count; and
        // sometimes an entry for a key no mutant has, or one that was caught.
        let mut entries: Vec<(usize, &str, &str, u64, &str)> = Vec::new();
        for (d, _) in &drawn {
            let key_taken = entries
                .iter()
                .any(|e| e.0 == d.line && e.1 == d.file && e.2 == d.description);
            if key_taken {
                continue;
            }
            if clean {
                let n = drawn
                    .iter()
                    .filter(|(o, _)| {
                        o.file == d.file && o.line == d.line && o.description == d.description
                    })
                    .count() as u64;
                let kind = match d.summary {
                    Summary::Caught | Summary::Unviable => continue,
                    Summary::Timeout => "hangs",
                    Summary::Missed if d.foundation => "equivalent",
                    Summary::Missed => {
                        ["equivalent", "harmless", "known"][below(&mut rng, 3) as usize]
                    }
                };
                entries.push((d.line, d.file, d.description, n, kind));
                continue;
            }
            if below(&mut rng, 10) < 2 {
                continue;
            }
            let same = drawn
                .iter()
                .filter(|(o, _)| {
                    o.file == d.file && o.line == d.line && o.description == d.description
                })
                .count() as u64;
            let occurrences = match below(&mut rng, 10) {
                0 => same + 1,
                1 if same > 1 => same - 1,
                _ => same,
            };
            let kind = if d.summary == Summary::Timeout && below(&mut rng, 4) != 0 {
                "hangs"
            } else if d.summary == Summary::Missed && below(&mut rng, 3) == 0 {
                if d.foundation {
                    "equivalent"
                } else {
                    kinds[below(&mut rng, 4) as usize]
                }
            } else {
                kinds[below(&mut rng, 4) as usize]
            };
            entries.push((d.line, d.file, d.description, occurrences, kind));
        }
        if !clean && below(&mut rng, 8) == 0 {
            entries.push((30, BOOL, "replace < with <= in f", 1, "known"));
        }
        let values: Vec<(Value, Summary)> = drawn
            .iter()
            .map(|(d, column)| (mutant(d.file, d.line, *column, d.description), d.summary))
            .collect();
        let dir = write_run(&repo, &format!("case{case}"), &values);
        let survivors: Vec<Value> = entries
            .iter()
            .map(|&(line, file, desc, n, kind)| entry(file, line, desc, n, kind))
            .collect();
        let all: Vec<Value> = values.iter().map(|(m, _)| m.clone()).collect();
        let expected = (mode == Mode::Full).then_some(all.as_slice());
        let v = verdict(&repo, &[dir], survivors, mode, expected);
        let drawn_only: Vec<Drawn> = drawn.into_iter().map(|(d, _)| d).collect();
        let want = oracle(&drawn_only, &entries, mode);
        assert_eq!(
            v.passes(),
            want,
            "case {case} ({mode:?}): the gate says {}, the rules say {want}\n{}",
            v.passes(),
            mutants::report(&v)
        );
        assert!(!clean || want, "case {case}: a clean case must pass");
        if want { passes += 1 } else { fails += 1 }
    }
    // Both verdicts are exercised many times, so neither half of the oracle is idle.
    assert!(
        passes >= 30 && fails >= 30,
        "{passes} passes, {fails} fails"
    );
}

// ---------------------------------------------------------------------------------------------
// The mutation gate, one seeded fault at a time
// ---------------------------------------------------------------------------------------------

fn missed(file: &str, line: usize) -> (Value, Summary) {
    (
        mutant(file, line, 5, "replace < with <= in f"),
        Summary::Missed,
    )
}

fn caught(file: &str, line: usize) -> (Value, Summary) {
    (
        mutant(file, line, 5, "replace < with <= in f"),
        Summary::Caught,
    )
}

fn fails_with(v: &mutants::Verdict, needle: &str) {
    assert!(!v.passes(), "the gate passed:\n{}", mutants::report(v));
    assert!(
        v.failures.iter().any(|f| f.contains(needle)),
        "no failure mentions {needle:?}:\n{}",
        mutants::report(v)
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn an_explained_survivor_passes_and_an_unexplained_one_fails() {
    let repo = mutation_repo("explained");
    let dir = write_run(&repo, "a", &[missed(NUM, 3), caught(NUM, 4)]);
    let keep = vec![entry(NUM, 3, "replace < with <= in f", 1, "equivalent")];
    assert!(verdict(&repo, &[dir.clone()], keep, Mode::Diff, None).passes());
    fails_with(
        &verdict(&repo, &[dir], vec![], Mode::Diff, None),
        "no ledger entry explains it",
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn the_foundation_set_keeps_only_equivalent_mutants() {
    let repo = mutation_repo("foundation-kinds");
    let dir = write_run(&repo, "a", &[missed(NUM, 3)]);
    for kind in ["harmless", "known"] {
        let keep = vec![entry(NUM, 3, "replace < with <= in f", 1, kind)];
        let v = verdict(&repo, &[dir.clone()], keep, Mode::Diff, None);
        fails_with(&v, "keeps only equivalent mutants");
    }
    // Elsewhere, a known survivor is no new one.
    let dir = write_run(&repo, "b", &[missed(BOOL, 3)]);
    for kind in ["equivalent", "harmless", "known"] {
        let keep = vec![entry(BOOL, 3, "replace < with <= in f", 1, kind)];
        assert!(
            verdict(&repo, &[dir.clone()], keep, Mode::Diff, None).passes(),
            "{kind}"
        );
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn known_entries_must_come_from_the_base_branch() {
    let repo = mutation_repo("base-known");
    let dir = write_run(&repo, "a", &[missed(BOOL, 3)]);
    let known = ledger(vec![entry(BOOL, 3, "replace < with <= in f", 1, "known")]);
    let empty = ledger(vec![]);
    // A pull request that adds the known entry for its own survivor merges a new unexplained one.
    for base in [None, Some(&empty)] {
        let v = verdict_with_base(&repo, &[dir.clone()], &known, Mode::Diff, None, base);
        fails_with(&v, "the base branch's ledger does not");
    }
    // The base holds the same mutant under another kind: still not an old known survivor.
    let harmless = ledger(vec![entry(
        BOOL,
        3,
        "replace < with <= in f",
        1,
        "harmless",
    )]);
    let v = verdict_with_base(
        &repo,
        &[dir.clone()],
        &known,
        Mode::Diff,
        None,
        Some(&harmless),
    );
    fails_with(&v, "the base branch's ledger does not");
    // An old known survivor passes; so does one the pull request explains as harmless.
    let v = verdict_with_base(
        &repo,
        &[dir.clone()],
        &known,
        Mode::Diff,
        None,
        Some(&known),
    );
    assert!(v.passes(), "{}", mutants::report(&v));
    let v = verdict_with_base(&repo, &[dir.clone()], &harmless, Mode::Diff, None, None);
    assert!(v.passes(), "{}", mutants::report(&v));
    // A ledger-only pull request may add known entries no mutant of its diff matches.
    let other = write_run(&repo, "b", &[caught(BOOL, 9)]);
    let v = verdict_with_base(&repo, &[other], &known, Mode::Diff, None, None);
    assert!(v.passes(), "{}", mutants::report(&v));
    // The weekly run judges the ledger as it is: the base plays no part.
    let list = [mutant(BOOL, 3, 5, "replace < with <= in f")];
    let v = verdict_with_base(&repo, &[dir], &known, Mode::Full, Some(&list), None);
    assert!(v.passes(), "{}", mutants::report(&v));
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn shard_plans_fit_the_budget_and_a_matrix() {
    use geom_test::mutants::{
        MAX_SHARDS, MINUTES_PER_MUTANT, SHARD_MINUTES, shard_count, shard_matrix,
    };
    assert_eq!(shard_count(0), Ok(0));
    assert_eq!(shard_count(1), Ok(1));
    let per_shard = usize::try_from(SHARD_MINUTES / MINUTES_PER_MUTANT).unwrap();
    assert_eq!(shard_count(per_shard), Ok(1));
    assert_eq!(shard_count(per_shard + 1), Ok(2));
    for n in [1, 7, 60, 61, 6190, 15_000] {
        let shards = shard_count(n).unwrap();
        // Every shard plans for at most SHARD_MINUTES, and one fewer shard would not do.
        assert!(
            n as u64 * MINUTES_PER_MUTANT <= shards * SHARD_MINUTES,
            "{n}"
        );
        assert!(
            n as u64 * MINUTES_PER_MUTANT > (shards - 1) * SHARD_MINUTES,
            "{n}"
        );
    }
    let too_many = usize::try_from(MAX_SHARDS * SHARD_MINUTES / MINUTES_PER_MUTANT).unwrap() + 1;
    assert!(shard_count(too_many).is_err());
    let plan: Value = serde_json::from_str(&shard_matrix(3)).unwrap();
    assert_eq!(plan, json!({"count": 3, "shards": [0, 1, 2]}));
    // A matrix may not be empty: a plan of no shard still lists shard 0, and count 0 skips it.
    let none: Value = serde_json::from_str(&shard_matrix(0)).unwrap();
    assert_eq!(none, json!({"count": 0, "shards": [0]}));
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn timeouts_need_a_hangs_entry_and_survivors_cannot_use_one() {
    let repo = mutation_repo("timeouts");
    let t = (
        mutant(NUM, 3, 5, "replace < with <= in f"),
        Summary::Timeout,
    );
    let dir = write_run(&repo, "a", &[t]);
    fails_with(
        &verdict(&repo, &[dir.clone()], vec![], Mode::Diff, None),
        "timed out",
    );
    let eq = vec![entry(NUM, 3, "replace < with <= in f", 1, "equivalent")];
    fails_with(
        &verdict(&repo, &[dir.clone()], eq, Mode::Diff, None),
        "does not allow",
    );
    let hangs = vec![entry(NUM, 3, "replace < with <= in f", 1, "hangs")];
    assert!(verdict(&repo, &[dir], hangs.clone(), Mode::Diff, None).passes());
    let dir = write_run(&repo, "b", &[missed(NUM, 3)]);
    fails_with(
        &verdict(&repo, &[dir], hangs, Mode::Diff, None),
        "does not allow",
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn stale_entries_fail() {
    let repo = mutation_repo("stale");
    let keep = vec![entry(NUM, 3, "replace < with <= in f", 1, "equivalent")];
    // Now caught: the entry must go, in either mode.
    let dir = write_run(&repo, "a", &[caught(NUM, 3)]);
    fails_with(
        &verdict(&repo, &[dir], keep.clone(), Mode::Diff, None),
        "remove the stale entry",
    );
    // Not generated at all: stale in the full run, outside the diff in a diff run.
    let dir = write_run(&repo, "b", &[caught(NUM, 4)]);
    let all = [mutant(NUM, 4, 5, "replace < with <= in f")];
    fails_with(
        &verdict(&repo, &[dir.clone()], keep.clone(), Mode::Full, Some(&all)),
        "the entry is stale",
    );
    assert!(verdict(&repo, &[dir], keep, Mode::Diff, None).passes());
    // A line that no longer exists fails in both modes.
    let gone = vec![json!({
        "file": NUM, "mutant": "replace < with <= in f", "line_text": "let gone = 0;",
        "occurrences": 1, "kind": "equivalent", "reason": REASON,
    })];
    let dir = write_run(&repo, "c", &[caught(NUM, 4)]);
    fails_with(
        &verdict(&repo, &[dir], gone, Mode::Diff, None),
        "the code changed",
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn occurrences_must_match() {
    let repo = mutation_repo("occurrences");
    let two = [
        (mutant(NUM, 3, 5, "replace < with <= in f"), Summary::Missed),
        (mutant(NUM, 3, 9, "replace < with <= in f"), Summary::Missed),
    ];
    let dir = write_run(&repo, "a", &two);
    let all: Vec<Value> = two.iter().map(|(m, _)| m.clone()).collect();
    let one = vec![entry(NUM, 3, "replace < with <= in f", 1, "equivalent")];
    fails_with(
        &verdict(&repo, &[dir.clone()], one, Mode::Diff, None),
        "counts 1 occurrence",
    );
    let two_entry = vec![entry(NUM, 3, "replace < with <= in f", 2, "equivalent")];
    assert!(verdict(&repo, &[dir.clone()], two_entry, Mode::Full, Some(&all)).passes());
    let three = vec![entry(NUM, 3, "replace < with <= in f", 3, "equivalent")];
    fails_with(
        &verdict(&repo, &[dir], three, Mode::Full, Some(&all)),
        "counts 3 occurrence",
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn shards_must_cover_the_list_exactly_once() {
    let repo = mutation_repo("shards");
    let a = write_run(&repo, "a", &[caught(NUM, 3)]);
    let b = write_run(&repo, "b", &[caught(NUM, 4)]);
    let all = [
        mutant(NUM, 3, 5, "replace < with <= in f"),
        mutant(NUM, 4, 5, "replace < with <= in f"),
    ];
    assert!(
        verdict(
            &repo,
            &[a.clone(), b.clone()],
            vec![],
            Mode::Full,
            Some(&all)
        )
        .passes()
    );
    fails_with(
        &verdict(&repo, &[a.clone()], vec![], Mode::Full, Some(&all)),
        "no run tested it",
    );
    fails_with(
        &verdict(&repo, &[a.clone(), a.clone()], vec![], Mode::Diff, None),
        "shards must not overlap",
    );
    fails_with(
        &verdict(&repo, &[a.clone(), b], vec![], Mode::Full, Some(&all[..1])),
        "not in the list",
    );
    fails_with(
        &verdict(&repo, &[a], vec![], Mode::Full, None),
        "needs the list of every mutant",
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn a_run_that_is_not_whole_cannot_be_read() {
    let repo = mutation_repo("whole");
    let ok = write_run(&repo, "ok", &[caught(NUM, 3), missed(NUM, 4)]);
    assert!(read_run(&ok).is_ok());
    let cases: [(&str, Box<dyn Fn(&mut Value)>); 6] = [
        ("unfinished", Box::new(|v| v["end_time"] = Value::Null)),
        (
            "baseline failed",
            Box::new(|v| v["outcomes"][0]["summary"] = json!("Failure")),
        ),
        (
            "no baseline",
            Box::new(|v| {
                v["outcomes"].as_array_mut().unwrap().remove(0);
            }),
        ),
        ("count", Box::new(|v| v["missed"] = json!(0))),
        ("total", Box::new(|v| v["total_mutants"] = json!(3))),
        (
            "lost outcome",
            Box::new(|v| {
                v["outcomes"].as_array_mut().unwrap().pop();
                v["total_mutants"] = json!(1);
                v["missed"] = json!(0);
            }),
        ),
    ];
    for (name, edit) in cases {
        let dir = write_run(&repo, name, &[caught(NUM, 3), missed(NUM, 4)]);
        edit_outcomes(&dir, |v| edit(v));
        assert!(read_run(&dir).is_err(), "{name}: read as whole");
    }
    // Mutants listed but no outcomes at all.
    let dir = write_run(&repo, "no outcomes", &[caught(NUM, 3)]);
    std::fs::remove_file(dir.join("outcomes.json")).unwrap();
    assert!(read_run(&dir).is_err());
    // An unclassified outcome reads, but fails the gate.
    let dir = write_run(&repo, "unclassified", &[caught(NUM, 3)]);
    edit_outcomes(&dir, |v| {
        v["outcomes"][1]["summary"] = json!("Failure");
        v["caught"] = json!(0);
    });
    fails_with(
        &verdict(&repo, &[dir], vec![], Mode::Diff, None),
        "could not classify",
    );
    // No mutants and no outcomes is a run of nothing, which passes.
    let empty = write_run(&repo, "empty", &[]);
    assert!(verdict(&repo, &[empty], vec![], Mode::Diff, Some(&[])).passes());
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn an_unviable_mutant_needs_a_compiler_error_in_its_log() {
    let repo = mutation_repo("unviable");
    let u = (
        mutant(NUM, 3, 5, "replace < with <= in f"),
        Summary::Unviable,
    );
    let dir = write_run(&repo, "a", &[u.clone()]);
    assert!(read_run(&dir).is_ok());
    // The second and third are what cargo-mutants 27.1 logged, as "Unviable", for three mutants
    // of geom-num built while this machine's disk was full (2026-10-22): mutants that compile and
    // were never tested.
    for log in [
        "error: linking with `cc` failed: exit status: 1\n",
        "error: failed to write file \\\\?\\C:\\t\\target\\release\\incremental\\plane\\dep-graph.part.bin: \
         There is not enough space on the disk. (os error 112)\n\nerror: could not compile \
         `geom-num` (test \"plane\") due to 1 previous error\n",
        "  = note: ld: final link failed: No space left on device\n\nerror: could not compile \
         `geom-num` (lib test); 1 warning emitted\n",
        "error: could not compile `geom-num`\n\nCaused by:\n  process didn't exit successfully\n",
        "",
    ] {
        let dir = write_run(&repo, "b", &[u.clone()]);
        std::fs::write(dir.join("log/m0.log"), log).unwrap();
        assert!(read_run(&dir).is_err(), "{log:?}");
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn mutants_outside_kernel_sources_mean_a_wrong_configuration() {
    let repo = mutation_repo("outside");
    let file = "projects/engines/geometry/crates/geom-test/src/lib.rs";
    repo.write(file, &numbered_source(5));
    let dir = write_run(
        &repo,
        "a",
        &[(
            mutant(file, 2, 1, "replace < with <= in f"),
            Summary::Caught,
        )],
    );
    fails_with(
        &verdict(&repo, &[dir], vec![], Mode::Diff, None),
        "configuration is wrong",
    );
}

// ---------------------------------------------------------------------------------------------
// The coverage gate
// ---------------------------------------------------------------------------------------------

/// An LCOV record for `file` under `repo`, with its lines and branch outcomes; `brf` defaults to
/// the number of branch outcomes.
fn lcov_record(
    repo: &Repo,
    rel: &str,
    lines: &[(usize, u64)],
    branches: &[(usize, Option<u64>)],
    brf: Option<usize>,
) -> String {
    let mut out = format!("SF:{}\n", repo.path(rel).display());
    for (i, (line, taken)) in branches.iter().enumerate() {
        let t = taken.map_or("-".to_owned(), |t| t.to_string());
        out.push_str(&format!("BRDA:{line},{},{},{t}\n", i / 2, i % 2));
    }
    out.push_str(&format!("BRF:{}\n", brf.unwrap_or(branches.len())));
    for (line, count) in lines {
        out.push_str(&format!("DA:{line},{count}\n"));
    }
    out.push_str("end_of_record\n");
    out
}

/// A kernel source file: `n` lines of code in one function, then a test module of `t` lines.
fn kernel_source(n: usize, t: usize) -> String {
    let mut s = String::from("pub fn f(x: i64) -> i64 {\n");
    for i in 0..n {
        s.push_str(&format!("    if x > {i} {{ return {i}; }}\n"));
    }
    s.push_str("    0\n}\n");
    if t > 0 {
        s.push_str("#[cfg(test)]\nmod tests {\n");
        for i in 0..t {
            s.push_str(&format!("    fn t{i}() {{ if true {{}} }}\n"));
        }
        s.push_str("}\n");
    }
    s
}

fn judged(repo: &Repo, krate: &str, lcov: &str) -> Vec<String> {
    let report = measure(&repo.root, krate, lcov).unwrap();
    coverage::judge(&[report])
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn random_coverage_is_judged_as_the_bars_say() {
    let seed = resolve(
        "quality gates: random coverage",
        Seed(0x6732_315f_636f_7665),
    )
    .unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut passes = 0;
    let mut fails = 0;
    for case in 0..200 {
        let repo = Repo::new("random-coverage");
        let foundation = rng.coin();
        let krate = if foundation { "geom-num" } else { "geom-bool" };
        let rel = format!("{K}/{krate}/src/lib.rs");
        let n = 1 + below(&mut rng, 30) as usize;
        let t = below(&mut rng, 4) as usize;
        repo.write(&rel, &kernel_source(n, t));
        // Lines 1..=n+3 are the function (n + 3 lines); a test module follows.
        let code_lines: Vec<usize> = (1..=n + 3).collect();
        let test_lines: Vec<usize> = (n + 6..n + 6 + t).collect();
        // Mostly covered, sometimes not.
        let miss_rate = [0, 0, 1, 5, 20][below(&mut rng, 5) as usize];
        let mut lines = Vec::new();
        for &l in &code_lines {
            let hit = miss_rate == 0 || below(&mut rng, 100) >= miss_rate;
            lines.push((l, if hit { 1 + below(&mut rng, 9) } else { 0 }));
        }
        for &l in &test_lines {
            lines.push((l, below(&mut rng, 2)));
        }
        let mut branches = Vec::new();
        for i in 0..n {
            let l = 2 + i;
            for _ in 0..2 {
                let taken = if miss_rate == 0 || below(&mut rng, 100) >= miss_rate {
                    Some(1)
                } else if rng.coin() {
                    Some(0)
                } else {
                    None
                };
                branches.push((l, taken));
            }
        }
        for &l in &test_lines {
            branches.push((l, Some(0)));
            branches.push((l, None));
        }
        let lcov = lcov_record(&repo, &rel, &lines, &branches, None);
        let failures = judged(&repo, krate, &lcov);
        // The oracle: only code lines count; the foundation needs every line and branch outcome,
        // elsewhere 90% of lines.
        let found = code_lines.len() as u64;
        let hit = lines
            .iter()
            .filter(|(l, c)| code_lines.contains(l) && *c > 0)
            .count() as u64;
        let branch_found = (2 * n) as u64;
        let branch_hit = branches
            .iter()
            .filter(|(l, t)| code_lines.contains(l) && t.is_some_and(|t| t > 0))
            .count() as u64;
        let want = if foundation {
            hit == found && branch_hit == branch_found
        } else {
            100 * hit >= 90 * found
        };
        assert_eq!(failures.is_empty(), want, "case {case}: {failures:?}");
        let report = measure(&repo.root, krate, &lcov).unwrap();
        let c = report.files[0].counts;
        assert_eq!((c.lines_found, c.lines_hit), (found, hit), "case {case}");
        assert_eq!(
            (c.branches_found, c.branches_hit),
            (branch_found, branch_hit),
            "case {case}"
        );
        if want { passes += 1 } else { fails += 1 }
    }
    assert!(
        passes >= 30 && fails >= 30,
        "{passes} passes, {fails} fails"
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn the_ninety_percent_bar_is_exact() {
    let repo = Repo::new("ninety");
    let rel = format!("{K}/geom-bool/src/lib.rs");
    repo.write(&rel, &kernel_source(7, 0));
    // Ten lines: nine run is 90%, eight is 80%.
    for (missed, pass) in [(1, true), (2, false)] {
        let lines: Vec<(usize, u64)> = (1..=10).map(|l| (l, u64::from(l > missed))).collect();
        let f = judged(
            &repo,
            "geom-bool",
            &lcov_record(&repo, &rel, &lines, &[], None),
        );
        assert_eq!(f.is_empty(), pass, "{missed} missed: {f:?}");
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn foundation_misses_are_named() {
    let repo = Repo::new("foundation-misses");
    let rel = format!("{K}/geom-num/src/lib.rs");
    repo.write(&rel, &kernel_source(2, 0));
    let lines = [(1, 1), (2, 1), (3, 0), (4, 1), (5, 1)];
    let f = judged(
        &repo,
        "geom-num",
        &lcov_record(
            &repo,
            &rel,
            &lines,
            &[(2, Some(1)), (2, Some(1)), (3, Some(1)), (3, Some(1))],
            None,
        ),
    );
    assert!(f.iter().any(|x| x.contains("never run: 3")), "{f:?}");
    let all = [(1, 1), (2, 1), (3, 1), (4, 1), (5, 1)];
    let f = judged(
        &repo,
        "geom-num",
        &lcov_record(
            &repo,
            &rel,
            &all,
            &[(2, Some(1)), (2, Some(0)), (3, Some(1)), (3, None)],
            None,
        ),
    );
    assert!(
        f.iter()
            .any(|x| x.contains("not taken both ways on lines: 2, 3")),
        "{f:?}"
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn foundation_code_with_branches_needs_branch_data() {
    let repo = Repo::new("no-branch-data");
    let rel = format!("{K}/geom-num/src/lib.rs");
    repo.write(&rel, &kernel_source(2, 0));
    let lines = [(1, 1), (2, 1), (3, 1), (4, 1), (5, 1)];
    let f = judged(
        &repo,
        "geom-num",
        &lcov_record(&repo, &rel, &lines, &[], None),
    );
    assert!(f.iter().any(|x| x.contains("no branch record")), "{f:?}");
    // Elsewhere only lines count, so no branch data is needed.
    let other = format!("{K}/geom-bool/src/lib.rs");
    repo.write(&other, &kernel_source(2, 0));
    assert!(
        judged(
            &repo,
            "geom-bool",
            &lcov_record(&repo, &other, &lines, &[], None)
        )
        .is_empty()
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn a_file_with_functions_but_no_data_fails() {
    let repo = Repo::new("no-data");
    let a = format!("{K}/geom-bool/src/lib.rs");
    let b = format!("{K}/geom-bool/src/more.rs");
    let types = format!("{K}/geom-bool/src/types.rs");
    repo.write(&a, &kernel_source(1, 0));
    repo.write(&b, &kernel_source(1, 0));
    repo.write(
        &types,
        "pub struct S { pub f: fn(u32) -> u32 }\npub const X: u32 = 1;\n",
    );
    let lines = [(1, 1), (2, 1), (3, 1), (4, 1)];
    let f = judged(
        &repo,
        "geom-bool",
        &lcov_record(&repo, &a, &lines, &[], None),
    );
    assert!(
        f.iter()
            .any(|x| x.contains("more.rs defines functions but has no coverage data")),
        "{f:?}"
    );
    assert!(!f.iter().any(|x| x.contains("types.rs")), "{f:?}");
    // An out-of-line test module is test code as a whole, like a `mod tests { ... }` block, and
    // cargo-llvm-cov leaves files named tests.rs out of its report; its functions need no data.
    repo.write(&b, "pub const Y: u32 = 2;\n#[cfg(test)]\nmod tests;\n");
    repo.write(&format!("{K}/geom-bool/src/more/tests.rs"), "fn t() {}\n");
    let f = judged(
        &repo,
        "geom-bool",
        &lcov_record(&repo, &a, &lines, &[], None),
    );
    assert!(f.is_empty(), "{f:?}");
    assert!(geom_test::coverage::has_code(&repo.root, "geom-bool").unwrap());
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn a_crate_with_code_but_no_foundation_module_fails() {
    let repo = Repo::new("module");
    let lib = format!("{K}/geom-topo/src/lib.rs");
    repo.write(&lib, &kernel_source(1, 0));
    let lines = [(1, 1), (2, 1), (3, 1), (4, 1)];
    let f = judged(
        &repo,
        "geom-topo",
        &lcov_record(&repo, &lib, &lines, &[], None),
    );
    assert!(f.iter().any(|x| x.contains("foundation module")), "{f:?}");
    // With src/builder.rs, the builder is held to the foundation bar and the rest to 90%.
    let builder = format!("{K}/geom-topo/src/builder.rs");
    repo.write(&builder, &kernel_source(1, 0));
    let lcov = lcov_record(&repo, &lib, &lines, &[], None)
        + &lcov_record(
            &repo,
            &builder,
            &[(1, 1), (2, 0), (3, 1), (4, 1)],
            &[(2, Some(1)), (2, Some(1))],
            None,
        );
    let f = judged(&repo, "geom-topo", &lcov);
    assert!(
        f.iter().any(|x| x.contains("builder.rs: lines 3/4")),
        "{f:?}"
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn folded_branches_are_left_out_only_when_they_can_be_placed() {
    let repo = Repo::new("folded");
    let rel = format!("{K}/geom-num/src/lib.rs");
    repo.write(&rel, &kernel_source(2, 1));
    let lines = [(1, 1), (2, 1), (3, 1), (4, 1), (5, 1)];
    // One folded outcome (BRF one below the BRDA lines), and the only outcome of 0: it is the
    // folded one, so the file is fully covered.
    let one_zero = [(2, Some(1)), (2, Some(1)), (3, Some(1)), (3, Some(0))];
    assert!(
        judged(
            &repo,
            "geom-num",
            &lcov_record(&repo, &rel, &lines, &one_zero, Some(3))
        )
        .is_empty()
    );
    // Without the folded outcome, the same 0 is a miss.
    assert!(
        !judged(
            &repo,
            "geom-num",
            &lcov_record(&repo, &rel, &lines, &one_zero, None)
        )
        .is_empty()
    );
    // Two outcomes of 0 in kernel code, one folded: one is a real miss.
    let two_zero = [(2, Some(1)), (2, Some(0)), (3, Some(1)), (3, Some(0))];
    assert!(
        !judged(
            &repo,
            "geom-num",
            &lcov_record(&repo, &rel, &lines, &two_zero, Some(3))
        )
        .is_empty()
    );
    // Outcomes of 0 in both test and kernel code, with a folded one somewhere: it cannot be
    // placed, so the file fails rather than guess.
    let mixed = [
        (2, Some(1)),
        (2, Some(0)),
        (3, Some(1)),
        (3, Some(1)),
        (9, Some(0)),
        (9, Some(1)),
    ];
    let f = judged(
        &repo,
        "geom-num",
        &lcov_record(&repo, &rel, &lines, &mixed, Some(5)),
    );
    assert!(
        f.iter().any(|x| x.contains("cannot be told apart")),
        "{f:?}"
    );
    // More folded outcomes than branch records is not data the gate can use.
    assert!(
        measure(
            &repo.root,
            "geom-num",
            &lcov_record(&repo, &rel, &lines, &one_zero, Some(9))
        )
        .is_err()
    );
}

#[test]
fn a_run_that_warned_about_mismatched_data_fails() {
    assert!(check_log("geom-num", "running 3 tests\ntest result: ok\n").is_none());
    let warned = "warning: 135 functions have mismatched data\n";
    assert!(check_log("geom-num", warned).is_some_and(|w| w.contains("cannot be trusted")));
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn a_foundation_line_waits_for_its_proof_only_with_an_exact_entry() {
    let repo = Repo::new("awaiting");
    let rel = format!("{K}/geom-num/src/lib.rs");
    // Lines 2, 3 and 4 hold `if x > 0`, `if x > 1` and `if x > 2`; line 3 never runs.
    repo.write(&rel, &kernel_source(3, 0));
    let lines = [(1, 1), (2, 1), (3, 0), (4, 1), (5, 1), (6, 1)];
    let branches = [
        (2, Some(1)),
        (2, Some(1)),
        (3, None),
        (3, None),
        (4, Some(1)),
        (4, Some(1)),
    ];
    let plain = lcov_record(&repo, &rel, &lines, &branches, None);
    let judge_with = |lcov: &str, entries: Vec<Value>| -> Vec<String> {
        let text = json!({"schema": SCHEMA, "survivors": [], "coverage_off": [], "awaiting_proof": entries})
            .to_string();
        let ledger = Ledger::parse(&text).unwrap();
        let mut reports = vec![measure(&repo.root, "geom-num", lcov).unwrap()];
        let mut failures = excuse_awaiting(&repo.root, &mut reports, &ledger).unwrap();
        failures.extend(coverage::judge(&reports));
        failures
    };
    let run = |entries: Vec<Value>| judge_with(&plain, entries);
    let entry = |text: &str, n: u64| json!({"file": rel, "line_text": text, "occurrences": n, "piece": "G41", "reason": REASON});
    let missed = "if x > 1 { return 1; }";
    // No entry: the line and its branch fail the foundation bar.
    assert_eq!(run(vec![]).len(), 2, "{:?}", run(vec![]));
    // The exact entry: it passes.
    assert_eq!(run(vec![entry(missed, 1)]), Vec::<String>::new());
    // An entry for a line the tests run is stale.
    let f = run(vec![entry(missed, 1), entry("if x > 2 { return 2; }", 1)]);
    assert!(f.len() == 1 && f[0].contains("the run has 0"), "{f:?}");
    // A wrong count excuses nothing.
    let f = run(vec![entry(missed, 2)]);
    assert!(f.iter().any(|x| x.contains("the run has 1")), "{f:?}");
    assert!(f.len() == 3, "{f:?}");
    // An entry for a crate the run did not measure waits for the run that does.
    let other = json!({"file": format!("{K}/geom-bool/src/lib.rs"), "line_text": "x", "occurrences": 1, "piece": "G41", "reason": REASON});
    assert!(run(vec![entry(missed, 1), other]).is_empty());
    // Folded outcomes among zeros in test and kernel code both: the folding note fails the file,
    // unless every kernel outcome with a count of 0 waits for its proof.
    repo.write(&rel, &kernel_source(3, 1));
    let mut with_test = branches.to_vec();
    with_test.extend([(9, Some(0)), (9, None)]);
    let folded = lcov_record(&repo, &rel, &lines, &with_test, Some(with_test.len() - 1));
    let f = judge_with(&folded, vec![]);
    assert!(f.iter().any(|x| x.contains("folded")), "{f:?}");
    assert_eq!(
        judge_with(&folded, vec![entry(missed, 1)]),
        Vec::<String>::new()
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn a_line_with_no_code_is_not_counted() {
    let repo = Repo::new("no-code");
    let rel = format!("{K}/geom-num/src/lib.rs");
    repo.write(
        &rel,
        "#[must_use]
pub fn f(x: i64) -> i64 {
    // a comment
    if x > 0 { return 1; }

    0
}
",
    );
    // llvm-cov's zero counts on the attribute, the comment and the blank line are not misses.
    let lines = [(1, 0), (2, 3), (3, 0), (4, 3), (5, 0), (6, 2), (7, 3)];
    let branches = [(4, Some(1)), (4, Some(2))];
    let lcov = lcov_record(&repo, &rel, &lines, &branches, None);
    let report = measure(&repo.root, "geom-num", &lcov).unwrap();
    assert_eq!(report.files[0].counts.lines_found, 4);
    assert_eq!(report.files[0].missed_lines, Vec::<usize>::new());
    assert_eq!(coverage::judge(&[report]), Vec::<String>::new());
    // A line of code with a count of 0 is still a miss.
    let lines = [(1, 0), (2, 3), (3, 0), (4, 3), (5, 0), (6, 0), (7, 3)];
    let lcov = lcov_record(&repo, &rel, &lines, &branches, None);
    let report = measure(&repo.root, "geom-num", &lcov).unwrap();
    assert_eq!(report.files[0].missed_lines, vec![6]);
}

// ---------------------------------------------------------------------------------------------
// coverage(off) marks and their proofs
// ---------------------------------------------------------------------------------------------

const PROOF_FILE: &str = "projects/engines/geometry/crates/geom-num/proofs/rungs.rs";

fn marked_source(cite: &str) -> String {
    format!(
        "pub fn f() -> u32 {{ 1 }}\n\n// The rung difference is at most 2.\n// proof: {cite}\n\
         #[cfg_attr(coverage_nightly, coverage(off))]\nfn other() -> u32 {{ 0 }}\n"
    )
}

fn mark_entry(proof: &str, tool: &str) -> Value {
    json!({"file": format!("{K}/geom-num/src/lib.rs"), "item": "fn other() -> u32 { 0 }", "proof": proof, "tool": tool, "reason": REASON})
}

fn marks(repo: &Repo, entries: Vec<Value>) -> Vec<String> {
    let text = json!({"schema": SCHEMA, "survivors": [], "coverage_off": entries}).to_string();
    check_marks(&repo.root, &Ledger::parse(&text).unwrap()).unwrap()
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn every_coverage_off_mark_cites_a_real_proof() {
    let repo = Repo::new("marks");
    let cite = format!("{PROOF_FILE}::rungs_are_at_most_two");
    repo.write(&format!("{K}/geom-num/src/lib.rs"), &marked_source(&cite));
    repo.write(
        PROOF_FILE,
        "#[cfg(kani)]\nmod proofs {\n    #[kani::proof]\n    fn rungs_are_at_most_two() {}\n}\n",
    );
    assert_eq!(
        marks(&repo, vec![mark_entry(&cite, "kani")]),
        Vec::<String>::new()
    );
    // No ledger entry.
    assert!(
        marks(&repo, vec![])
            .iter()
            .any(|f| f.contains("has no ledger entry"))
    );
    // The ledger cites another proof than the comment.
    let other = format!("{PROOF_FILE}::other_harness");
    assert!(
        marks(&repo, vec![mark_entry(&other, "kani")])
            .iter()
            .any(|f| f.contains("must cite"))
    );
    // The cited function is not a proof of that tool, or does not exist.
    assert!(
        marks(&repo, vec![mark_entry(&cite, "verus")])
            .iter()
            .any(|f| f.contains("is not a verus proof"))
    );
    repo.write(PROOF_FILE, "fn rungs_are_at_most_two() {}\n");
    assert!(
        marks(&repo, vec![mark_entry(&cite, "kani")])
            .iter()
            .any(|f| f.contains("is not a kani proof"))
    );
    std::fs::remove_file(repo.path(PROOF_FILE)).unwrap();
    assert!(
        marks(&repo, vec![mark_entry(&cite, "kani")])
            .iter()
            .any(|f| f.contains("is not a kani proof"))
    );
    // A Verus proof function counts for Verus.
    repo.write(
        PROOF_FILE,
        "verus! {\n    proof fn rungs_are_at_most_two() {}\n}\n",
    );
    assert_eq!(
        marks(&repo, vec![mark_entry(&cite, "verus")]),
        Vec::<String>::new()
    );
    // An entry whose mark is gone is stale.
    repo.write(
        &format!("{K}/geom-num/src/lib.rs"),
        "pub fn f() -> u32 { 1 }\n",
    );
    assert!(
        marks(&repo, vec![mark_entry(&cite, "verus")])
            .iter()
            .any(|f| f.contains("stale"))
    );
    // A mark without any citation fails even before the ledger is read.
    repo.write(
        &format!("{K}/geom-num/src/lib.rs"),
        "#[coverage(off)]\nfn bare() {}\n",
    );
    assert!(!marks(&repo, vec![]).is_empty());
}

// ---------------------------------------------------------------------------------------------
// Which crates a pull request measures
// ---------------------------------------------------------------------------------------------

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads and writes files in a temporary folder, which a WebAssembly test run cannot"
)]
#[test]
fn a_change_reaches_its_crate_and_every_dependent() {
    let repo = Repo::new("affected");
    let metadata = json!({"packages": [
        {"name": "geom-num", "dependencies": []},
        {"name": "geom-topo", "dependencies": [{"name": "geom-num"}]},
        {"name": "geom-bool", "dependencies": [{"name": "geom-topo"}, {"name": "proptest"}]},
        {"name": "geom-test", "dependencies": [{"name": "geom-bool"}]},
    ]});
    let affected = |changed: &[&str]| {
        let changed: Vec<String> = changed.iter().map(|s| (*s).to_owned()).collect();
        coverage::affected_crates(&repo.root, &changed, &metadata).unwrap()
    };
    assert_eq!(
        affected(&[&format!("{K}/geom-num/src/a.rs")]),
        ["geom-bool", "geom-num", "geom-topo"]
    );
    assert_eq!(
        affected(&[&format!("{K}/geom-topo/tests/t.rs")]),
        ["geom-bool", "geom-topo"]
    );
    assert_eq!(
        affected(&[&format!("{K}/geom-bool/Cargo.toml")]),
        ["geom-bool"]
    );
    // Files outside the kernel's crates, and the gates' own crate, reach everything.
    let all = ["geom-bool", "geom-num", "geom-topo"];
    assert_eq!(affected(&["Cargo.lock"]), all);
    assert_eq!(
        affected(&["projects/engines/geometry/quality-ledger.json"]),
        all
    );
    assert_eq!(affected(&[&format!("{K}/geom-test/src/lib.rs")]), all);
    assert_eq!(affected(&[]), Vec::<String>::new());
}

// ---------------------------------------------------------------------------------------------
// The binary
// ---------------------------------------------------------------------------------------------

fn kernel_quality(repo: &Repo, args: &[&str]) -> (i32, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_kernel-quality"));
    cmd.args(args);
    // mutants-plan reads only its list; every other command reads the repository.
    if args.first() != Some(&"mutants-plan") {
        cmd.arg("--repo").arg(&repo.root);
    }
    let out = cmd.output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap(), text)
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "runs the binary as a child process and writes files, which a WebAssembly test run cannot"
)]
#[test]
fn the_binary_exits_0_to_pass_1_to_fail_and_2_for_bad_input() {
    let repo = mutation_repo("binary");
    let ledger_path = repo.path("ledger.json");
    std::fs::write(
        &ledger_path,
        json!({"schema": SCHEMA, "survivors": [], "coverage_off": []}).to_string(),
    )
    .unwrap();
    let l = ledger_path.to_str().unwrap();
    let pass = write_run(&repo, "pass", &[caught(NUM, 3)]);
    let fail = write_run(&repo, "fail", &[missed(NUM, 3)]);
    let run = |dir: &Path| dir.to_str().unwrap().to_owned();
    let list = |dir: &Path| dir.join("mutants.json").to_str().unwrap().to_owned();
    let (code, text) = kernel_quality(
        &repo,
        &[
            "mutants",
            "--mode",
            "diff",
            "--expected",
            &list(&pass),
            "--ledger",
            l,
            &run(&pass),
        ],
    );
    assert_eq!(code, 0, "{text}");
    let (code, text) = kernel_quality(
        &repo,
        &[
            "mutants",
            "--mode",
            "diff",
            "--expected",
            &list(&fail),
            "--ledger",
            l,
            &run(&fail),
        ],
    );
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("no ledger entry explains it"), "{text}");
    // The full run with a list that holds one mutant more than the shards tested.
    let longer = repo.path("longer.json");
    std::fs::write(
        &longer,
        json!([
            mutant(NUM, 3, 5, "replace < with <= in f"),
            mutant(NUM, 7, 5, "replace < with <= in f")
        ])
        .to_string(),
    )
    .unwrap();
    let (code, text) = kernel_quality(
        &repo,
        &[
            "mutants",
            "--mode",
            "full",
            "--expected",
            longer.to_str().unwrap(),
            "--ledger",
            l,
            &run(&pass),
        ],
    );
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("listed, but no run tested it"), "{text}");
    // No mutant in the diff: an empty list (cargo-mutants prints none) and no shard pass.
    let empty = repo.path("empty.json");
    std::fs::write(&empty, "").unwrap();
    let (code, text) = kernel_quality(
        &repo,
        &[
            "mutants",
            "--mode",
            "diff",
            "--expected",
            empty.to_str().unwrap(),
            "--ledger",
            l,
        ],
    );
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("(no mutants)"), "{text}");
    let (code, text) = kernel_quality(&repo, &["mutants-plan", "--list", empty.to_str().unwrap()]);
    assert_eq!(code, 0, "{text}");
    assert_eq!(text.trim(), r#"{"count":0,"shards":[0]}"#);
    let (code, text) = kernel_quality(&repo, &["mutants-plan", "--list", longer.to_str().unwrap()]);
    assert_eq!(code, 0, "{text}");
    assert_eq!(text.trim(), r#"{"count":1,"shards":[0]}"#);
    // Bad input: an unknown mode, a missing list, a run that is not whole.
    let (code, _) = kernel_quality(
        &repo,
        &[
            "mutants",
            "--mode",
            "sometimes",
            "--expected",
            &list(&pass),
            "--ledger",
            l,
            &run(&pass),
        ],
    );
    assert_eq!(code, 2);
    let (code, _) = kernel_quality(
        &repo,
        &["mutants", "--mode", "diff", "--ledger", l, &run(&pass)],
    );
    assert_eq!(code, 2);
    std::fs::remove_file(pass.join("outcomes.json")).unwrap();
    let (code, _) = kernel_quality(
        &repo,
        &[
            "mutants",
            "--mode",
            "diff",
            "--expected",
            &list(&pass),
            "--ledger",
            l,
            &run(&pass),
        ],
    );
    assert_eq!(code, 2);

    // Coverage: every crate with code needs its LCOV file and the log of its run; a crate with no
    // function (geom-topo here) has nothing to measure and is named as skipped.
    for krate in ["geom-num", "geom-bool"] {
        repo.write(
            &format!("{K}/{krate}/src/lib.rs"),
            "pub fn f() -> u32 {\n    1\n}\n",
        );
    }
    repo.write(
        &format!("{K}/geom-topo/src/lib.rs"),
        "pub const X: u32 = 1;\n",
    );
    let cov = repo.path("cov");
    std::fs::create_dir_all(&cov).unwrap();
    let c = cov.to_str().unwrap();
    let (code, text) = kernel_quality(&repo, &["affected", "--all"]);
    assert_eq!(code, 0, "{text}");
    assert!(text.starts_with("geom-bool\ngeom-num\n"), "{text}");
    assert!(
        text.contains("geom-topo: no function outside test code"),
        "{text}"
    );
    let (code, text) = kernel_quality(
        &repo,
        &["coverage", "--all", "--lcov-dir", c, "--ledger", l],
    );
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("no coverage data"), "{text}");
    for krate in ["geom-num", "geom-bool"] {
        let rel = format!("{K}/{krate}/src/lib.rs");
        let lcov = lcov_record(&repo, &rel, &[(1, 1), (2, 1), (3, 1)], &[], None);
        std::fs::write(cov.join(format!("{krate}.lcov")), lcov).unwrap();
        std::fs::write(cov.join(format!("{krate}.log")), "test result: ok\n").unwrap();
    }
    let (code, text) = kernel_quality(
        &repo,
        &["coverage", "--all", "--lcov-dir", c, "--ledger", l],
    );
    assert_eq!(code, 0, "{text}");
    std::fs::write(
        cov.join("geom-num.log"),
        "warning: 2 functions have mismatched data\n",
    )
    .unwrap();
    let (code, text) = kernel_quality(
        &repo,
        &[
            "coverage",
            "--crates",
            "geom-num",
            "--lcov-dir",
            c,
            "--ledger",
            l,
        ],
    );
    assert_eq!(code, 1, "{text}");
    let (code, _) = kernel_quality(
        &repo,
        &[
            "coverage",
            "--crates",
            "geom-nope",
            "--lcov-dir",
            c,
            "--ledger",
            l,
        ],
    );
    assert_eq!(code, 2);
}

// ---------------------------------------------------------------------------------------------
// The repository's own ledger
// ---------------------------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .to_path_buf()
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_repository_ledger_reads_and_every_mark_in_the_kernel_has_its_proof() {
    let root = std::fs::canonicalize(repo_root()).unwrap();
    let ledger = Ledger::read(&root.join(geom_test::quality_ledger::LEDGER_PATH)).unwrap();
    for s in &ledger.survivors {
        // The foundation set keeps only equivalent mutants (and hangs, which are caught by time).
        if let Place::Kernel {
            set: Set::Foundation,
            ..
        } = place(&s.file)
        {
            assert!(matches!(s.kind, Kind::Equivalent | Kind::Hangs), "{s:?}");
        }
        let text = std::fs::read_to_string(root.join(&s.file)).unwrap();
        assert!(text.lines().any(|l| l.trim() == s.line_text), "{s:?}");
    }
    assert_eq!(check_marks(&root, &ledger).unwrap(), Vec::<String>::new());
}
