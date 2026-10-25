//! The instruction-and-cache gate's verdict, read from Gungraun's machine-readable summaries:
//! instructions and estimated cycles are gated at the gate's percentage on every benchmark, and
//! each L1 and last-level miss count is gated at that percentage only where its merge-base count
//! can resolve it; a smaller miss count is listed and judged through estimated cycles.
//!
//! The rule (chapter 23.13, "Miss counts too small to resolve 5%"). Counts are whole events, so the
//! smallest change a count of b can show is one event, 100/b percent of it. One event is within a
//! limit of p percent exactly when 100 ≤ p · b, so a count resolves p% only from its floor,
//! ⌈100 / p⌉, up ([`floor`]): 20 for the gate's 5%. Below the floor a limit of p% fails on any
//! single event, and code layout alone moves miss counts by an event or two: a change that did not
//! touch the code a benchmark runs can still shift which cache lines that code falls on (PR #212's
//! aarch64 leg failed with I1mr and ILmr 2 → 3 on `exact_sum` benchmarks it did not touch, while
//! estimated cycles held). So a miss count below the floor is not gated on its own: every change in
//! it is listed, and it is judged through estimated cycles, which Gungraun computes as
//! L1 hits + 5 · last-level hits + 35 · RAM hits, that is instructions and data accesses plus 4 per
//! L1 miss and 30 more per last-level miss, and which stays gated at p% on every benchmark. A miss
//! count at or above its floor stays gated at p%, as before. Instructions and estimated cycles are
//! gated at p% whatever their size: they are what every miss is judged through. With p = 0 no count
//! resolves the limit, so no miss count is gated on its own.
//!
//! Guarantees:
//!
//! - **Exact, for every count.** Every limit is tested in integers ([`Change::rose_over`]):
//!   100 · head > (100 + p) · base, and a count resolves p% exactly when it is at least
//!   ⌈100 / p⌉. Percentages are computed, as floating point, for display only.
//! - **Every benchmark judged, or no verdict.** Every benchmark the run lists has exactly one
//!   summary, every summary is a benchmark the run lists and of the schema version the gate's
//!   Gungraun writes, and every summary carries a whole count
//!   for each of the eight judged measures, from the pull request and, for a benchmark the merge
//!   base has, from the merge base; anything else is an error, never a pass.
//! - **Deterministic reports.** Benchmarks are judged and reported in name order, and measures in
//!   the order of [`JUDGED`], the same on every run and every platform.
//! - **Totality.** No input makes [`parse_summary`], [`judge`] or [`read_summaries`] panic.
//!
//! Method: the summary files Gungraun writes with `--save-summary`, one per benchmark in its output
//! folder (<https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/crates/gungraun-runner/src/runner/args.rs#L1325-L1358>),
//! read through Gungraun's own typed summary model (`gungraun_runner::summary::model`, schema
//! version 7, <https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/crates/gungraun-runner/src/summary/model.rs#L29>;
//! the `summary` feature of the `gungraun-runner` crate at exactly the version the gate runs) and
//! `serde_json`; estimated cycles as Gungraun computes them
//! (<https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/crates/gungraun-runner/src/runner/callgrind/mod.rs#L110-L126>);
//! the percentage limit of Gungraun's soft limits
//! (<https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/README.md>),
//! applied exactly; the resolution floor is ours, derived above, because no source sets a floor
//! for a percentage limit on small counts.
//! Consulted: Gungraun, Apache-2.0 OR MIT,
//!   <https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/README.md>,
//!   for its summary files, its default output layout and its estimated-cycles formula. No code
//!   copied; its summary model is used as a dependency.
//!
//! Cost: reading and parsing every summary, linear in their size; then O(n log n) for n
//! benchmarks, which are kept in an ordered map.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use gungraun_runner::api::{EventKind, Tool};
use gungraun_runner::metrics::model::Metric;
use gungraun_runner::summary::model::{BenchmarkSummary, SCHEMA_VERSION, ToolMetricResults};

use crate::counts::Change;

/// The miss counts, by Gungraun's event kinds and names: L1 and last-level misses of instruction
/// reads, data reads and data writes. Each is gated only where its merge-base count resolves the
/// gate's percentage ([`floor`]).
pub const MISSES: [(EventKind, &str); 6] = [
    (EventKind::I1mr, "I1mr"),
    (EventKind::D1mr, "D1mr"),
    (EventKind::D1mw, "D1mw"),
    (EventKind::ILmr, "ILmr"),
    (EventKind::DLmr, "DLmr"),
    (EventKind::DLmw, "DLmw"),
];

/// Every Callgrind measure the gate judges, in report order: instructions and estimated cycles,
/// gated on every benchmark, then the [`MISSES`]. The names are [`crate::GATED_METRICS`] (a test
/// checks).
pub const JUDGED: [(EventKind, &str); 8] = [
    (EventKind::Ir, "Ir"),
    (EventKind::EstimatedCycles, "EstimatedCycles"),
    MISSES[0],
    MISSES[1],
    MISSES[2],
    MISSES[3],
    MISSES[4],
    MISSES[5],
];

/// The smallest count that resolves a limit of `gate` percent: the least b with 100 ≤ gate · b,
/// on which one event is within the limit, so ⌈100 / gate⌉; `None` for a limit of 0, which no
/// count resolves. 20 for the gate's 5%.
#[must_use]
pub fn floor(gate: u64) -> Option<u64> {
    (gate != 0).then(|| 100u64.div_ceil(gate))
}

/// What the rule says of one measure's change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Within the limit, or a miss count below its floor that did not change.
    Within,
    /// A rise over the limit on a count that resolves it: it fails the gate.
    Rose,
    /// A miss count below its floor that changed: listed, and judged through estimated cycles.
    Unresolved,
}

/// The rule for the measure `kind` changing by `change` under a limit of `gate` percent.
#[must_use]
pub fn verdict(kind: EventKind, change: &Change, gate: u64) -> Verdict {
    let miss = MISSES.iter().any(|&(k, _)| k == kind);
    let resolved = !miss || floor(gate).is_some_and(|f| change.base >= f);
    if !resolved {
        if change.base == change.head {
            Verdict::Within
        } else {
            Verdict::Unresolved
        }
    } else if change.rose_over(gate) {
        Verdict::Rose
    } else {
        Verdict::Within
    }
}

/// One benchmark's judged counts, read from its summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// The benchmark, as the gate names it: `group/op/class`.
    pub name: String,
    /// Each measure of [`JUDGED`], in that order: its count in the pull request, and at the merge
    /// base; `None` for every measure when the merge base has no such benchmark.
    pub counts: [(u64, Option<u64>); 8],
}

/// A whole count, or a message naming the measure.
fn whole(metric: &Metric, name: &str, side: &str) -> Result<u64, String> {
    match *metric {
        Metric::Int(count) => Ok(count),
        Metric::Float(value) => Err(format!(
            "{name} at the {side} is {value}, not a whole count"
        )),
    }
}

/// Reads one benchmark's summary: its name from its module path and id (the path's first part,
/// the benchmark file, dropped), and the counts of every [`JUDGED`] measure from its Callgrind run.
///
/// # Errors
///
/// A message, if the text is not a Gungraun summary of the schema version this reader is built for
/// (`gungraun_runner::summary::model::SCHEMA_VERSION`), names no benchmark, has no single Callgrind
/// run, lacks a judged measure, holds a count that is not whole, or has a merge-base count for
/// some measures but not others.
pub fn parse_summary(text: &str) -> Result<Summary, String> {
    let summary: BenchmarkSummary =
        serde_json::from_str(text).map_err(|e| format!("not a Gungraun summary: {e}"))?;
    if summary.version != SCHEMA_VERSION {
        return Err(format!(
            "summary schema version {:?}, but this reader is for version {SCHEMA_VERSION:?}",
            summary.version
        ));
    }
    let mut parts: Vec<&str> = summary.module_path.split("::").skip(1).collect();
    if let Some(id) = summary.id.as_deref() {
        parts.push(id);
    }
    if parts.is_empty() || parts.iter().any(|p| p.is_empty() || p.contains(['/', ' '])) {
        return Err(format!(
            "no benchmark name in module path {:?} and id {:?}",
            summary.module_path, summary.id
        ));
    }
    let name = parts.join("/");
    let mut runs = summary
        .profiles
        .0
        .iter()
        .filter(|p| p.tool == Tool::Callgrind);
    let (Some(run), None) = (runs.next(), runs.next()) else {
        return Err(format!(
            "{name}: not exactly one Callgrind run in its summary"
        ));
    };
    let ToolMetricResults::Callgrind(results) = &run.data.total.metrics else {
        return Err(format!("{name}: its Callgrind run has no Callgrind counts"));
    };
    let mut counts = [(0, None); 8];
    for (slot, &(kind, metric)) in counts.iter_mut().zip(JUDGED.iter()) {
        let result = results
            .0
            .get(&kind)
            .ok_or_else(|| format!("{name}: no {metric} count in its summary"))?;
        let values = result.values.as_ref();
        let head = values
            .left()
            .ok_or_else(|| format!("{name}: no {metric} count from the pull request"))?;
        let head = whole(head, metric, "pull request").map_err(|e| format!("{name}: {e}"))?;
        let base = values
            .right()
            .map(|base| whole(base, metric, "merge base"))
            .transpose()
            .map_err(|e| format!("{name}: {e}"))?;
        *slot = (head, base);
    }
    if counts.iter().any(|c| c.1.is_some()) != counts.iter().all(|c| c.1.is_some()) {
        return Err(format!(
            "{name}: merge-base counts for some measures but not all"
        ));
    }
    Ok(Summary { name, counts })
}

/// Every `summary.json` under `dir`, at any depth, parsed, in path order. Gungraun writes one per
/// benchmark, in the benchmark's own folder of its output directory.
///
/// # Errors
///
/// A message naming the folder or file that cannot be read, or the summary that cannot be parsed.
pub fn read_summaries(dir: &Path) -> Result<Vec<Summary>, String> {
    let mut files = Vec::new();
    let mut folders = vec![dir.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let entries =
            std::fs::read_dir(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("{}: {e}", folder.display()))?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|e| format!("{}: {e}", path.display()))?;
            if kind.is_dir() {
                folders.push(path);
            } else if kind.is_file() && entry.file_name() == "summary.json" {
                files.push(path);
            }
        }
    }
    files.sort();
    files
        .iter()
        .map(|path: &PathBuf| {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
            parse_summary(&text).map_err(|e| format!("{}: {e}", path.display()))
        })
        .collect()
}

/// One measure of one benchmark that the report lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measure {
    /// The measure's Gungraun name.
    pub metric: &'static str,
    /// Its change; the change's name is the benchmark's.
    pub change: Change,
    /// The same benchmark's estimated cycles, through which a miss count below its floor is
    /// judged.
    pub cycles: Change,
}

/// What [`judge`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Judgement {
    /// The limit, in percent.
    pub gate: u64,
    /// The floor of the miss counts ([`floor`]).
    pub floor: Option<u64>,
    /// Rises over the limit on counts that resolve it: each one fails the gate.
    pub regressions: Vec<Measure>,
    /// Miss counts below their floor that changed: listed, judged through estimated cycles.
    pub unresolved: Vec<Measure>,
    /// Benchmarks the merge base lacks: recorded, not compared.
    pub new: Vec<String>,
    /// How many benchmarks were compared with the merge base.
    pub compared: usize,
}

/// Judges every benchmark the run lists (`names`) from its summary, under a limit of `gate`
/// percent.
///
/// # Errors
///
/// A message, if two summaries name one benchmark, a listed benchmark has no summary, or a summary
/// names a benchmark the run does not list (a stale or foreign file).
pub fn judge(
    summaries: &[Summary],
    names: &BTreeSet<String>,
    gate: u64,
) -> Result<Judgement, String> {
    let mut by_name: BTreeMap<&str, &Summary> = BTreeMap::new();
    for s in summaries {
        if by_name.insert(&s.name, s).is_some() {
            return Err(format!("{}: two summaries", s.name));
        }
        if !names.contains(&s.name) {
            return Err(format!(
                "{}: a summary of a benchmark this run does not list (clear Gungraun's output \
                 folder of old runs)",
                s.name
            ));
        }
    }
    if let Some(missing) = names.iter().find(|n| !by_name.contains_key(n.as_str())) {
        return Err(format!("{missing}: no summary"));
    }
    let mut out = Judgement {
        gate,
        floor: floor(gate),
        regressions: Vec::new(),
        unresolved: Vec::new(),
        new: Vec::new(),
        compared: 0,
    };
    for (&name, s) in &by_name {
        let mut changes = Vec::with_capacity(JUDGED.len());
        for (&(head, base), &(kind, metric)) in s.counts.iter().zip(JUDGED.iter()) {
            if let Some(base) = base {
                let change = Change {
                    name: name.to_owned(),
                    base,
                    head,
                };
                changes.push((kind, metric, change));
            }
        }
        let Some(cycles) = changes
            .iter()
            .find(|c| c.0 == EventKind::EstimatedCycles)
            .map(|c| c.2.clone())
        else {
            out.new.push(name.to_owned());
            continue;
        };
        out.compared += 1;
        for (kind, metric, change) in changes {
            let measure = |change| Measure {
                metric,
                change,
                cycles: cycles.clone(),
            };
            match verdict(kind, &change, gate) {
                Verdict::Within => {}
                Verdict::Rose => out.regressions.push(measure(change)),
                Verdict::Unresolved => out.unresolved.push(measure(change)),
            }
        }
    }
    Ok(out)
}

impl Judgement {
    /// Whether the gate passes: no rise over the limit on a count that resolves it.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.regressions.is_empty()
    }

    /// A Markdown report of everything found, under the heading `title`.
    #[must_use]
    pub fn report(&self, title: &str) -> String {
        let gate = self.gate;
        let mut out = String::new();
        let _ = writeln!(out, "### {title}\n");
        if self.passed() {
            let _ = writeln!(
                out,
                "**Passed.** {} benchmark(s) compared with the merge base; none rose by more than \
                 {gate}% on a count that resolves it.\n",
                self.compared
            );
        } else {
            let _ = writeln!(
                out,
                "**Failed:** {} rise(s) over {gate}%.\n",
                self.regressions.len()
            );
        }
        let rule = match self.floor {
            Some(floor) => format!(
                "Instructions and estimated cycles are gated at {gate}% on every benchmark. A miss \
                 count is gated at {gate}% when its merge-base count is {floor} or more; below \
                 {floor}, one event is more than {gate}%, so the count cannot resolve the limit: \
                 its changes are listed and judged through estimated cycles, which weight every \
                 miss (chapter 23.13)."
            ),
            None => format!(
                "Instructions and estimated cycles are gated at {gate}% on every benchmark. No \
                 count resolves a limit of {gate}%, so every miss count's changes are listed and \
                 judged through estimated cycles (chapter 23.13)."
            ),
        };
        let _ = writeln!(out, "{rule}\n");
        measures(
            &mut out,
            &format!("Rises over {gate}% (they block the merge)"),
            &self.regressions,
        );
        let below = self
            .floor
            .map_or_else(|| "every miss count".to_owned(), |f| format!("under {f}"));
        measures(
            &mut out,
            &format!("Miss counts {below} that changed (judged through estimated cycles)"),
            &self.unresolved,
        );
        let _ = writeln!(out, "#### New (recorded, not compared)\n");
        if self.new.is_empty() {
            let _ = writeln!(out, "None.\n");
        } else {
            for name in &self.new {
                let _ = writeln!(out, "- {name}");
            }
            out.push('\n');
        }
        out
    }
}

/// Appends a table of measures, or "None."
fn measures(out: &mut String, heading: &str, list: &[Measure]) {
    let _ = writeln!(out, "#### {heading}\n");
    if list.is_empty() {
        let _ = writeln!(out, "None.\n");
        return;
    }
    let _ = writeln!(
        out,
        "| Benchmark | Measure | Merge base | Pull request | Change | Estimated cycles |"
    );
    let _ = writeln!(out, "| --- | --- | ---: | ---: | ---: | ---: |");
    for m in list {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {:+.2}% | {:+.2}% |",
            m.change.name,
            m.metric,
            m.change.base,
            m.change.head,
            m.change.percent(),
            m.cycles.percent()
        );
    }
    out.push('\n');
}
