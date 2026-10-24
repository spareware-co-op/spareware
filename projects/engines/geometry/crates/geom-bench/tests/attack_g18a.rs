//! Attacks on G18a (issue #215): the instruction-and-cache gate judges a cache-miss count too small
//! to resolve 5% through estimated cycles, keeps 5% on counts of 20 or more, reads Gungraun's
//! machine-readable summaries with a real parser, and applies one rule on every leg.
//!
//! Each test tries to break one claim of the done-when or of `geom_bench::callgrind`'s guarantees:
//! the seeded cases the issue names (a +1 miss on a count of 2 passes, a real rise in estimated
//! cycles fails, a rise over 5% on a count of 200 fails), the floor's derivation at and around its
//! boundary for every percentage, the parser against every malformed summary, the judgement's
//! completeness, the command line's exit statuses, and the workflow's wiring on both legs.
//!
//! The summaries are written here in Gungraun's schema, and one test proves they are exactly what
//! Gungraun's own serializer writes for the same data, so the seeded cases are summaries the gate
//! can meet.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use geom_bench::callgrind::{
    JUDGED, MISSES, Verdict, floor, judge, parse_summary, read_summaries, verdict,
};
use geom_bench::counts::Change;
use gungraun_runner::api::EventKind;
use gungraun_runner::summary::model::BenchmarkSummary;
use serde_json::{Value, json};

const WORKFLOW: &str = include_str!("../../../../../../.github/workflows/kernel-bench.yml");
const CHAPTER: &str = include_str!("../../../architecture/23-performance.md");
const GATE: &str = include_str!("../benches/instructions/gate.rs");

/// One measure's change, as `compare` and the judge make them.
fn change(base: u64, head: u64) -> Change {
    Change {
        name: "x".into(),
        base,
        head,
    }
}

/// A count in a summary: the pull request's (`new`) and, if the merge base has the benchmark, the
/// merge base's (`old`), with the change Gungraun records when both are present.
fn result(new: u64, old: Option<u64>) -> Value {
    match old {
        None => json!({ "values": { "new": new } }),
        Some(old) => {
            let (n, o) = (new as f64, old as f64);
            let (diff_pct, factor) = if new == old {
                (0.0, 1.0)
            } else if old == 0 {
                (f64::INFINITY, f64::INFINITY)
            } else {
                ((n - o) / o * 100.0, n / o)
            };
            json!({
                "change": { "diff_pct": diff_pct.to_string(), "factor": factor.to_string() },
                "values": { "new": new, "old": old },
            })
        }
    }
}

/// The counts of one benchmark: every judged measure, pull request and merge base.
#[derive(Clone)]
struct Counts {
    ir: (u64, Option<u64>),
    cycles: (u64, Option<u64>),
    misses: [(u64, Option<u64>); 6],
}

impl Counts {
    /// A benchmark that did not change: 10,000 instructions, 14,000 estimated cycles, and
    /// `misses` of each kind, at both.
    fn steady(misses: u64) -> Counts {
        Counts {
            ir: (10_000, Some(10_000)),
            cycles: (14_000, Some(14_000)),
            misses: [(misses, Some(misses)); 6],
        }
    }

    /// A benchmark the merge base lacks.
    fn new_only() -> Counts {
        Counts {
            ir: (10_000, None),
            cycles: (14_000, None),
            misses: [(2, None); 6],
        }
    }

    /// The same with miss kind `kind` (an index into `MISSES`) moved from `base` to `head`.
    fn miss(mut self, kind: usize, base: u64, head: u64) -> Counts {
        self.misses[kind] = (head, Some(base));
        self
    }

    /// The same with estimated cycles moved from `base` to `head`.
    fn cycles(mut self, base: u64, head: u64) -> Counts {
        self.cycles = (head, Some(base));
        self
    }

    /// The same with instructions moved from `base` to `head`.
    fn ir(mut self, base: u64, head: u64) -> Counts {
        self.ir = (head, Some(base));
        self
    }

    /// The Callgrind metrics object, with the other events Gungraun records alongside.
    fn metrics(&self) -> Value {
        let mut m = serde_json::Map::new();
        let both = |(n, o): (u64, Option<u64>)| result(n, o);
        m.insert("Ir".into(), both(self.ir));
        // Events the gate does not judge: present in real summaries, ignored by the judge.
        m.insert("Dr".into(), result(3_000, self.ir.1.map(|_| 3_000)));
        m.insert("Dw".into(), result(1_000, self.ir.1.map(|_| 1_000)));
        for (&(_, name), &counts) in MISSES.iter().zip(self.misses.iter()) {
            m.insert(name.into(), both(counts));
        }
        m.insert("L1hits".into(), result(13_990, self.ir.1.map(|_| 13_990)));
        m.insert("EstimatedCycles".into(), both(self.cycles));
        Value::Object(m)
    }
}

/// A summary of benchmark `group/op/class` in Gungraun's schema (version 7), with a Callgrind run
/// holding `counts` and a DHAT run, as the gate's runs write it.
fn summary(group: &str, op: &str, class: &str, counts: &Counts) -> Value {
    let metrics = counts.metrics();
    let run = |tool: &str, metrics: Value| {
        json!({
            "data": {
                "parts": [{
                    "metrics": metrics,
                    "tool_run": {
                        "new": { "command": "instructions", "part": 1, "pid": 4242 },
                        "old": { "command": "instructions", "part": 1, "pid": 4141 }
                    }
                }],
                "total": { "metrics": metrics, "regressions": [] }
            },
            "duration_ns": 1_000_000,
            "process_ns": 900_000,
            "started_at": "2026-10-25T00:00:00.000000000Z",
            "tool": tool
        })
    };
    let dhat = json!({
        "TotalBytes": result(0, Some(0)),
        "TotalBlocks": result(0, Some(0)),
        "AtTGmaxBytes": result(0, Some(0)),
        "MaximumBytes": result(0, Some(0)),
    });
    json!({
        "baselines": ["head", "base"],
        "benchmark_exe": "/work/target/release/deps/instructions",
        "benchmark_file": "/work/benches/instructions/main.rs",
        "duration_ns": 2_000_000,
        "function_name": op,
        "group": group,
        "id": class,
        "kind": "LibraryBenchmark",
        "module_path": format!("instructions::{group}::{op}"),
        "output_dir": format!("/work/target/gungraun/geom-bench/instructions/{group}/{op}.{class}"),
        "package_dir": "/work",
        "profiles": [run("Callgrind", metrics), run("DHAT", dhat)],
        "project_root": "/work",
        "started_at": "2026-10-25T00:00:00.000000000Z",
        "version": "7"
    })
}

/// Parses seeded summaries and judges them as the gate does, at 5%, against the names they carry.
fn judge_5(seeded: &[(&str, &str, &str, Counts)]) -> geom_bench::callgrind::Judgement {
    let summaries: Vec<_> = seeded
        .iter()
        .map(|(g, o, c, counts)| parse_summary(&summary(g, o, c, counts).to_string()).unwrap())
        .collect();
    let names: BTreeSet<String> = summaries.iter().map(|s| s.name.clone()).collect();
    judge(&summaries, &names, 5).unwrap()
}

// ------------------------------------------------------------------------------------------------
// The seeded cases of the done-when.
// ------------------------------------------------------------------------------------------------

#[test]
fn a_one_event_rise_on_a_count_of_2_passes_and_is_listed() {
    // PR #212's aarch64 leg (run 40136795440): I1mr and ILmr 2 -> 3 on exact_sum benchmarks it did
    // not touch, estimated cycles held. One event is +50%; the old gate failed on it.
    let counts = Counts::steady(2).miss(0, 2, 3).miss(3, 2, 3);
    let j = judge_5(&[("exact_sum", "sum", "typical", counts)]);
    assert!(j.passed(), "{}", j.report("t"));
    assert!(j.regressions.is_empty());
    let listed: Vec<(&str, u64, u64)> = j
        .unresolved
        .iter()
        .map(|m| (m.metric, m.change.base, m.change.head))
        .collect();
    assert_eq!(listed, [("I1mr", 2, 3), ("ILmr", 2, 3)]);
    let report = j.report("Instructions, estimated cycles and cache misses on aarch64");
    assert!(report.contains("**Passed.**"), "{report}");
    assert!(
        report.contains("| exact_sum/sum/typical | I1mr | 2 | 3 | +50.00% | +0.00% |"),
        "{report}"
    );
    assert!(
        report.contains("Miss counts under 20 that changed (judged through estimated cycles)"),
        "{report}"
    );
}

#[test]
fn a_real_rise_in_estimated_cycles_fails() {
    // Estimated cycles over 5% fail, with every miss count unchanged ...
    let j = judge_5(&[(
        "predicate",
        "orient2d",
        "typical",
        Counts::steady(2).cycles(14_000, 14_701),
    )]);
    assert!(!j.passed());
    assert_eq!(j.regressions.len(), 1);
    assert_eq!(j.regressions[0].metric, "EstimatedCycles");
    // ... and exactly 5% passes (14,000 · 1.05 = 14,700).
    let j = judge_5(&[(
        "predicate",
        "orient2d",
        "typical",
        Counts::steady(2).cycles(14_000, 14_700),
    )]);
    assert!(j.passed(), "{}", j.report("t"));
    // A small miss count that rises by hundreds of events is caught through estimated cycles:
    // 598 more misses to RAM cost 598 · (4 + 30) cycles more (Gungraun: L1 hits + 5 · LL hits +
    // 35 · RAM hits), 20,332 on 14,000. The miss count itself is listed, the cycles fail.
    let ram = 2u64 + 598;
    let cycles = 14_000 + 598 * 34;
    let counts = Counts::steady(2)
        .miss(1, 2, ram)
        .miss(4, 2, ram)
        .cycles(14_000, cycles);
    let j = judge_5(&[("exact_sum", "sum", "typical", counts)]);
    assert!(!j.passed());
    assert_eq!(
        j.regressions.iter().map(|m| m.metric).collect::<Vec<_>>(),
        ["EstimatedCycles"]
    );
    assert_eq!(
        j.unresolved.iter().map(|m| m.metric).collect::<Vec<_>>(),
        ["D1mr", "DLmr"]
    );
}

#[test]
fn a_rise_over_5_percent_on_a_count_of_200_fails() {
    // 5% of 200 is 10 events: 210 is at the limit and passes, 211 is over it and fails, on every
    // miss kind, while estimated cycles hold.
    for (kind, &(_, name)) in MISSES.iter().enumerate() {
        let j = judge_5(&[("wide", "add", "w4", Counts::steady(200).miss(kind, 200, 211))]);
        assert!(!j.passed(), "{name}");
        assert_eq!(j.regressions.len(), 1);
        assert_eq!(j.regressions[0].metric, name);
        assert_eq!(
            (j.regressions[0].change.base, j.regressions[0].change.head),
            (200, 211)
        );
        let j = judge_5(&[("wide", "add", "w4", Counts::steady(200).miss(kind, 200, 210))]);
        assert!(j.passed(), "{name}: {}", j.report("t"));
        assert!(j.unresolved.is_empty(), "a resolved count is not listed as unresolved");
    }
}

#[test]
fn instructions_are_gated_at_every_size() {
    // Instructions and estimated cycles are what every miss is judged through: no floor.
    let j = judge_5(&[("sign", "sign", "typical", Counts::steady(0).ir(10_000, 10_501))]);
    assert_eq!(
        j.regressions.iter().map(|m| m.metric).collect::<Vec<_>>(),
        ["Ir"]
    );
    for (base, head, fails) in [(10, 11, true), (1, 2, true), (0, 1, true), (19, 19, false)] {
        assert_eq!(
            verdict(EventKind::Ir, &change(base, head), 5) == Verdict::Rose,
            fails
        );
        assert_eq!(
            verdict(EventKind::EstimatedCycles, &change(base, head), 5) == Verdict::Rose,
            fails
        );
    }
}

// ------------------------------------------------------------------------------------------------
// The floor: derived, exact, at its boundary.
// ------------------------------------------------------------------------------------------------

#[test]
fn the_floor_is_the_least_count_on_which_one_event_is_within_the_limit() {
    assert_eq!(floor(5), Some(20));
    assert_eq!(floor(0), None);
    assert_eq!(floor(1), Some(100));
    assert_eq!(floor(3), Some(34));
    assert_eq!(floor(100), Some(1));
    assert_eq!(floor(u64::MAX), Some(1));
    for p in 1..=1000u64 {
        let f = floor(p).unwrap();
        // One event within p% of f: 100 ≤ p · f; and not of f − 1.
        assert!(100 <= p * f, "{p}");
        assert!(f == 1 || 100 > p * (f - 1), "{p}");
        // The judge agrees with the derivation: a one-event rise on a count of f passes, one on
        // f − 1 is not gated (listed instead).
        assert_eq!(
            verdict(EventKind::D1mr, &change(f, f + 1), p),
            Verdict::Within,
            "{p}"
        );
        if f > 1 {
            assert_eq!(
                verdict(EventKind::D1mr, &change(f - 1, f), p),
                Verdict::Unresolved,
                "{p}"
            );
        }
    }
}

#[test]
fn every_count_at_the_floor_or_above_still_resolves_5_percent() {
    // From 20 up, the old rule stands exactly: a miss count fails when it rises over 5%, and one
    // event never does. Below 20, no change fails on its own and every change is listed.
    for &(kind, name) in &MISSES {
        for base in 0..2_000u64 {
            for head in base.saturating_sub(3)..=base + base / 10 + 3 {
                let v = verdict(kind, &change(base, head), 5);
                if base >= 20 {
                    let over = 100 * head > 105 * base;
                    assert_eq!(v == Verdict::Rose, over, "{name} {base} -> {head}");
                    assert_ne!(v, Verdict::Unresolved);
                    if head == base + 1 {
                        assert_eq!(v, Verdict::Within, "{name}: one event on {base}");
                    }
                } else {
                    let expected = if head == base {
                        Verdict::Within
                    } else {
                        Verdict::Unresolved
                    };
                    assert_eq!(v, expected, "{name} {base} -> {head}");
                }
            }
        }
    }
    // The boundary the issue names: 19 + 1 is listed, 20 + 1 (exactly 5%) passes, 20 + 2 fails,
    // 39 + 2 (5.1%) fails, 40 + 2 (5%) passes.
    let d1mr = |b, h| verdict(EventKind::D1mr, &change(b, h), 5);
    assert_eq!(d1mr(19, 20), Verdict::Unresolved);
    assert_eq!(d1mr(19, 1_000), Verdict::Unresolved);
    assert_eq!(d1mr(20, 21), Verdict::Within);
    assert_eq!(d1mr(20, 22), Verdict::Rose);
    assert_eq!(d1mr(39, 41), Verdict::Rose);
    assert_eq!(d1mr(40, 42), Verdict::Within);
    // From zero: listed, never gated on its own.
    assert_eq!(d1mr(0, 1), Verdict::Unresolved);
    assert_eq!(d1mr(0, 0), Verdict::Within);
    // Falls below the floor are listed too.
    assert_eq!(d1mr(3, 2), Verdict::Unresolved);
}

#[test]
fn the_rule_is_exact_at_the_extremes_of_its_types() {
    let max = u64::MAX;
    for p in [0, 1, 5, 99, 100, 101, max - 1, max] {
        for &(kind, _) in &JUDGED {
            // No change never fails; a count that cannot hold more never wraps.
            assert_eq!(verdict(kind, &change(max, max), p), Verdict::Within);
            assert_ne!(verdict(kind, &change(max, 0), p), Verdict::Rose);
        }
    }
    // With a limit of 0, no count resolves it: misses are never gated on their own, instructions
    // fail on one event.
    assert_eq!(
        verdict(EventKind::I1mr, &change(1_000_000, 1_000_001), 0),
        Verdict::Unresolved
    );
    assert_eq!(
        verdict(EventKind::Ir, &change(1_000_000, 1_000_001), 0),
        Verdict::Rose
    );
    let j = {
        let s = parse_summary(
            &summary("a", "b", "c", &Counts::steady(500).miss(2, 500, 900)).to_string(),
        )
        .unwrap();
        let names: BTreeSet<String> = [s.name.clone()].into();
        judge(&[s], &names, 0).unwrap()
    };
    assert!(j.passed());
    assert!(
        j.report("t")
            .contains("No count resolves a limit of 0%, so every miss count's changes are listed")
    );
}

// ------------------------------------------------------------------------------------------------
// The parser: Gungraun's own format, and nothing else.
// ------------------------------------------------------------------------------------------------

#[test]
fn the_seeded_summaries_are_what_gungraun_writes() {
    // Read with Gungraun's own model, then written again by its own serializer: the same JSON. So
    // the seeded cases above are files the gate's runs can produce, not a format of our own.
    for counts in [
        Counts::steady(2).miss(0, 2, 3),
        Counts::steady(0).miss(5, 0, 7).cycles(14_000, 15_000),
        Counts::new_only(),
    ] {
        let written = summary("exact_sum", "sum", "typical", &counts);
        let model: BenchmarkSummary = serde_json::from_value(written.clone()).unwrap();
        assert_eq!(serde_json::to_value(&model).unwrap(), written);
    }
}

#[test]
fn a_summary_gives_the_gates_name_and_every_count() {
    let counts = Counts::steady(7).miss(4, 7, 9).cycles(14_000, 14_068);
    let s = parse_summary(&summary("exact_sum", "sum", "nearly_cancelling", &counts).to_string())
        .unwrap();
    assert_eq!(s.name, "exact_sum/sum/nearly_cancelling");
    assert_eq!(s.counts[0], (10_000, Some(10_000)));
    assert_eq!(s.counts[1], (14_068, Some(14_000)));
    assert_eq!(s.counts[2 + 4], (9, Some(7)));
    // Pretty-printed or compact, the same.
    let pretty = serde_json::to_string_pretty(&summary("exact_sum", "sum", "x", &counts)).unwrap();
    assert_eq!(parse_summary(&pretty).unwrap().counts, s.counts);
    // A benchmark the merge base lacks has no merge-base counts.
    let s = parse_summary(&summary("g", "o", "c", &Counts::new_only()).to_string()).unwrap();
    assert!(s.counts.iter().all(|c| c.1.is_none()));
}

/// The seeded summary of a steady benchmark, edited by `edit`, then parsed.
fn parse_edited(edit: impl FnOnce(&mut Value)) -> Result<geom_bench::callgrind::Summary, String> {
    let mut v = summary("g", "o", "c", &Counts::steady(5));
    edit(&mut v);
    parse_summary(&v.to_string())
}

/// The Callgrind totals of a seeded summary.
fn callgrind(v: &mut Value) -> &mut Value {
    &mut v["profiles"][0]["data"]["total"]["metrics"]
}

#[test]
fn every_malformed_summary_is_an_error() {
    let cases: Vec<(&str, Result<_, String>)> = vec![
        ("not JSON", parse_summary("{ not json")),
        ("empty", parse_summary("")),
        ("a list", parse_summary("[]")),
        (
            "another schema version",
            parse_edited(|v| v["version"] = json!("8")),
        ),
        (
            "no Callgrind run",
            parse_edited(|v| {
                v["profiles"].as_array_mut().unwrap().remove(0);
            }),
        ),
        (
            "two Callgrind runs",
            parse_edited(|v| {
                let run = v["profiles"][0].clone();
                v["profiles"].as_array_mut().unwrap().push(run);
            }),
        ),
        (
            "a missing miss count (cache simulation off)",
            parse_edited(|v| {
                callgrind(v).as_object_mut().unwrap().remove("DLmw");
            }),
        ),
        (
            "no estimated cycles",
            parse_edited(|v| {
                callgrind(v).as_object_mut().unwrap().remove("EstimatedCycles");
            }),
        ),
        (
            "a count that is not whole",
            parse_edited(|v| callgrind(v)["I1mr"]["values"]["new"] = json!(2.5)),
        ),
        (
            "a merge-base count that is not whole",
            parse_edited(|v| callgrind(v)["I1mr"]["values"]["old"] = json!(2.5)),
        ),
        (
            "a negative count",
            parse_edited(|v| callgrind(v)["I1mr"]["values"]["new"] = json!(-1)),
        ),
        (
            "no pull-request count",
            parse_edited(|v| callgrind(v)["Ir"]["values"] = json!({ "old": 10_000 })),
        ),
        (
            "merge-base counts for some measures only",
            parse_edited(|v| callgrind(v)["Ir"]["values"] = json!({ "new": 10_000 })),
        ),
        (
            "a count as a string",
            parse_edited(|v| callgrind(v)["Ir"]["values"]["new"] = json!("10000")),
        ),
        (
            "no module path",
            parse_edited(|v| {
                v.as_object_mut().unwrap().remove("module_path");
            }),
        ),
        (
            "a module path with no benchmark in it",
            parse_edited(|v| {
                v["module_path"] = json!("instructions");
                v.as_object_mut().unwrap().remove("id");
            }),
        ),
        (
            "an empty part in the name",
            parse_edited(|v| v["module_path"] = json!("instructions::::o")),
        ),
        (
            "a name with a space",
            parse_edited(|v| v["id"] = json!("two words")),
        ),
        (
            "a name with a slash",
            parse_edited(|v| v["id"] = json!("a/b")),
        ),
    ];
    for (what, parsed) in cases {
        assert!(parsed.is_err(), "{what}: parsed as {parsed:?}");
    }
    // The unedited summary parses, so each error above is the edit's.
    assert!(parse_edited(|_| {}).is_ok());
}

#[test]
fn every_benchmark_is_judged_or_there_is_no_verdict() {
    let s = |g: &str, counts: &Counts| parse_summary(&summary(g, "o", "c", counts).to_string());
    let a = s("a", &Counts::steady(2)).unwrap();
    let b = s("b", &Counts::steady(2)).unwrap();
    let names = |list: &[&str]| -> BTreeSet<String> { list.iter().map(|n| (*n).to_owned()).collect() };
    // A listed benchmark without a summary, a summary of a benchmark not listed, two summaries of
    // one benchmark, and no summaries at all for a listed run: each an error, never a pass.
    assert!(judge(&[a.clone()], &names(&["a/o/c", "b/o/c"]), 5).is_err());
    assert!(judge(&[a.clone(), b.clone()], &names(&["a/o/c"]), 5).is_err());
    assert!(judge(&[a.clone(), a.clone()], &names(&["a/o/c"]), 5).is_err());
    assert!(judge(&[], &names(&["a/o/c"]), 5).is_err());
    // Exactly the listed set: judged, in name order.
    let j = judge(&[b, a], &names(&["a/o/c", "b/o/c"]), 5).unwrap();
    assert_eq!(j.compared, 2);
    assert!(j.passed());
    // A benchmark the merge base lacks is recorded, not compared, and cannot fail.
    let n = s("n", &Counts::new_only()).unwrap();
    let j = judge(&[n], &names(&["n/o/c"]), 5).unwrap();
    assert_eq!((j.compared, j.new.clone()), (0, vec!["n/o/c".to_owned()]));
    assert!(j.passed());
    assert!(j.report("t").contains("- n/o/c"));
}

#[test]
fn reports_are_in_name_order_and_measure_order() {
    let j = judge_5(&[
        ("z", "o", "c", Counts::steady(100).miss(5, 100, 200).ir(10_000, 11_000)),
        ("a", "o", "c", Counts::steady(3).miss(2, 3, 4).miss(0, 3, 5)),
    ]);
    let rises: Vec<(&str, &str)> = j
        .regressions
        .iter()
        .map(|m| (m.change.name.as_str(), m.metric))
        .collect();
    assert_eq!(rises, [("z/o/c", "Ir"), ("z/o/c", "DLmw")]);
    let listed: Vec<(&str, &str)> = j
        .unresolved
        .iter()
        .map(|m| (m.change.name.as_str(), m.metric))
        .collect();
    assert_eq!(listed, [("a/o/c", "I1mr"), ("a/o/c", "D1mw")]);
    let report = j.report("t");
    assert!(report.contains("**Failed:** 2 rise(s) over 5%."), "{report}");
    assert!(report.contains("| z/o/c | DLmw | 100 | 200 | +100.00% | +0.00% |"));
    assert!(report.contains("its merge-base count is 20 or more"), "{report}");
}

// ------------------------------------------------------------------------------------------------
// The command line, end to end, on files in a folder laid out as Gungraun lays out its output.
// ------------------------------------------------------------------------------------------------

/// A fresh folder for one test, under Cargo's temporary folder for integration tests.
fn folder(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("attack_g18a").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes the seeded summaries as Gungraun does, `<group>/<op>.<class>/summary.json` under
/// `instructions/`, beside the other files of a run, and the names file; returns both paths.
fn lay_out(name: &str, seeded: &[(&str, &str, &str, Counts)]) -> (PathBuf, PathBuf) {
    let root = folder(name);
    let out = root.join("gungraun/geom-bench/instructions");
    let mut names = String::new();
    for (g, o, c, counts) in seeded {
        let dir = out.join(g).join(format!("{o}.{c}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("summary.json"),
            summary(g, o, c, counts).to_string(),
        )
        .unwrap();
        // The other files of a run, which are not summaries.
        std::fs::write(dir.join("callgrind.instructions.out"), "events: Ir\n").unwrap();
        std::fs::write(dir.join("summary.json.old"), "not read").unwrap();
        names.push_str(&format!("{g}/{o}/{c}\n"));
    }
    let names_file = root.join("names.txt");
    std::fs::write(&names_file, names).unwrap();
    (out, names_file)
}

/// Runs `geom-bench-counts callgrind` at 5%; its exit status and standard output.
fn run_counts(out: &Path, names: &Path) -> (Option<i32>, String) {
    let result = Command::new(env!("CARGO_BIN_EXE_geom-bench-counts"))
        .arg("callgrind")
        .arg(out)
        .arg(names)
        .arg("5")
        .arg("Instructions, estimated cycles and cache misses on x86-64")
        .output()
        .unwrap();
    (
        result.status.code(),
        String::from_utf8(result.stdout).unwrap(),
    )
}

#[test]
fn the_command_line_passes_fails_and_refuses_as_the_rule_says() {
    // The seeded cases, end to end: 0 for a pass, 1 for a rise, 2 when nothing can be judged.
    let (out, names) = lay_out(
        "pass",
        &[
            ("exact_sum", "sum", "typical", Counts::steady(2).miss(0, 2, 3).miss(3, 2, 3)),
            ("predicate", "orient2d", "typical", Counts::steady(0)),
        ],
    );
    let (status, report) = run_counts(&out, &names);
    assert_eq!(status, Some(0), "{report}");
    assert!(report.contains("**Passed.** 2 benchmark(s) compared"), "{report}");
    assert!(report.contains("| exact_sum/sum/typical | ILmr | 2 | 3 | +50.00% |"));

    let (out, names) = lay_out(
        "cycles",
        &[("predicate", "orient2d", "typical", Counts::steady(2).cycles(14_000, 15_000))],
    );
    assert_eq!(run_counts(&out, &names).0, Some(1));

    let (out, names) = lay_out(
        "two_hundred",
        &[("wide", "add", "w4", Counts::steady(200).miss(1, 200, 211))],
    );
    let (status, report) = run_counts(&out, &names);
    assert_eq!(status, Some(1));
    assert!(report.contains("| wide/add/w4 | D1mr | 200 | 211 | +5.50% |"), "{report}");

    // A summary missing for a listed benchmark, an unreadable folder, a malformed summary, and a
    // percentage that is not a whole number: 2, never a pass.
    let (out, names) = lay_out("missing", &[("a", "o", "c", Counts::steady(2))]);
    std::fs::write(&names, "a/o/c\nb/o/c\n").unwrap();
    assert_eq!(run_counts(&out, &names).0, Some(2));
    assert_eq!(run_counts(&out.join("nowhere"), &names).0, Some(2));
    let (out, names) = lay_out("malformed", &[("a", "o", "c", Counts::steady(2))]);
    std::fs::write(out.join("a/o.c/summary.json"), "{").unwrap();
    assert_eq!(run_counts(&out, &names).0, Some(2));
    let status = Command::new(env!("CARGO_BIN_EXE_geom-bench-counts"))
        .args(["callgrind", "x", "y", "5%", "t"])
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(2));
}

#[test]
fn read_summaries_finds_every_summary_at_any_depth_and_nothing_else() {
    let (out, _) = lay_out(
        "depth",
        &[
            ("b", "o", "c", Counts::steady(2)),
            ("a", "o", "c", Counts::steady(2)),
        ],
    );
    let found = read_summaries(&out).unwrap();
    assert_eq!(
        found.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["a/o/c", "b/o/c"]
    );
    // An empty folder has no summaries; a missing one is an error.
    assert!(read_summaries(&folder("empty")).unwrap().is_empty());
    assert!(read_summaries(&out.join("nowhere")).is_err());
}

// ------------------------------------------------------------------------------------------------
// The wiring: one rule, on every leg, from the summaries, written down where the issue says.
// ------------------------------------------------------------------------------------------------

/// The text of the workflow step named `name`, up to the next step.
fn workflow_step(name: &str) -> &'static str {
    let start = WORKFLOW.find(&format!("- name: {name}")).unwrap();
    let rest = &WORKFLOW[start + 1..];
    let end = rest.find("\n      - name:").map_or(rest.len(), |e| e + 1);
    &WORKFLOW[start..start + end]
}

#[test]
fn both_legs_judge_the_summaries_by_the_same_rule() {
    // One job measures both processors (its matrix), and the judgement is a step of that job with
    // no condition on the leg, so both legs run it, with the workflow's own percentage.
    let job = &WORKFLOW[WORKFLOW.find("\n  instructions:\n").unwrap()..WORKFLOW
        .find("\n  fuel:\n")
        .unwrap()];
    assert!(job.contains("- target: x86-64") && job.contains("- target: aarch64"));
    let judge_step = workflow_step("Judge instructions, cycles and cache misses from the summaries");
    assert!(job.contains(judge_step));
    assert!(!judge_step.contains("matrix."), "{judge_step}");
    assert!(judge_step.contains(
        "if: always() && needs.scope.outputs.base_gate == 'true' && steps.gate.outcome != 'skipped'"
    ));
    assert!(judge_step.contains("\"$GATE_PERCENT\""));
    assert!(judge_step.contains("\"$CARGO_TARGET_DIR/release/geom-bench-counts\" callgrind"));
    // A rise (1) and a judgement that cannot be made (2) both fail the step.
    assert!(judge_step.contains("|| status=$?"));
    let fails = judge_step
        .lines()
        .map(str::trim)
        .filter(|l| *l == "exit 1")
        .count();
    assert_eq!(fails, 2, "{judge_step}");
    // The gate saves the summaries the judgement reads, and Gungraun's own limits no longer gate a
    // miss count at a plain 5%: neither the workflow's nor the tree's.
    let gate = workflow_step("Benchmark the pull request against the merge base");
    assert!(gate.contains("--save-summary=json"));
    assert!(gate.contains("--callgrind-limits=\"$CALLGRIND_LIMITS\""));
    assert!(WORKFLOW.contains("  CALLGRIND_LIMITS: \"ir=5%,estimatedcycles=5%\"\n"));
    for miss in ["i1mr=5", "d1mr=5", "d1mw=5", "ilmr=5", "dlmr=5", "dlmw=5"] {
        assert!(!gate.contains(miss));
        assert!(!WORKFLOW.contains(&format!("CALLGRIND_LIMITS: \"{miss}")));
    }
    for &(_, miss) in &MISSES {
        assert!(!GATE.contains(&format!("EventKind::{miss}")), "{miss}");
    }
    // The summary reports the judgement.
    let summary = workflow_step("Write the summary");
    assert!(summary.contains("JUDGE_OUTCOME: ${{ steps.judge.outcome }}"));
    assert!(summary.contains("cat \"$t/callgrind.md\""));
}

#[test]
fn the_rule_is_written_in_chapter_23_13_and_the_gate_header() {
    let at = CHAPTER.find("## 23.13").unwrap();
    let section = &CHAPTER[at..CHAPTER[at..].find("## 23.14").map_or(CHAPTER.len(), |e| at + e)];
    assert!(
        section.contains("**Miss counts too small to resolve 5%.**"),
        "chapter 23.13 does not derive the rule"
    );
    for needle in ["below 20", "100 / p", "estimated cycles", "every leg", "40136795440"] {
        assert!(section.contains(needle), "23.13 lacks {needle:?}");
    }
    let header = include_str!("../src/callgrind.rs");
    for needle in ["⌈100 / p⌉", "20 for the gate's 5%", "35 · RAM hits", "#212"] {
        assert!(header.contains(needle), "the gate's header lacks {needle:?}");
    }
    assert!(GATE.contains("a count below 20"));
}
