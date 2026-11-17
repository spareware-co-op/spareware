//! The instruction-and-cache gate in parts: which part of the catalogue each runner measures, and
//! the judge's check that the parts together measured every benchmark exactly once.
//!
//! What it does: `kernel-bench.yml` measures the merge base and the change in parts, each part one
//! job with its share of the benchmark groups, so no job runs longer than 15 minutes; one judge per
//! target then gathers every part's counts and judges the whole catalogue as one run did before.
//!
//! - [`plan`] shares the groups among parts by their measured seconds (longest first, each to the
//!   part with the least so far), with as few parts as keep every part within [`PART_SECONDS`] or
//!   holding one group alone, and names the part that also measures any group the plan does not
//!   list (a group added since: [`Plan::rest`], the part with the least). The part holding the
//!   group `sign` also runs the planted slowdown (gate 1, `trip gungraun`), so that group is
//!   weighed with [`TRIP_SECONDS`] more.
//! - [`group_seconds`] sums each group's seconds from the durations Gungraun records in its
//!   summaries ([`read_durations`]): the times file `geom-bench-counts times` writes from a run's
//!   own report, and `geom-bench-counts plan` turns into `parts.tsv`, so the shares are written
//!   by the tool, never by hand (`tests/coverage.rs` checks the file is the plan of its seconds).
//! - [`measured`] reads which benchmarks one part measured from the list of its saved files, and
//!   [`check`] refuses the parts unless every part lists the same benchmarks and every benchmark
//!   listed was measured in exactly one part, with nothing measured that is not listed.
//!
//! `parts.tsv`, line by line (lines starting with `#` are comments):
//!
//! ```text
//! <group><TAB><part><TAB><seconds>     one per group, in name order
//! *<TAB><part><TAB>-                   the part that measures every group not listed
//! ```
//!
//! Guarantees:
//!
//! - **Every benchmark in exactly one part, or no verdict.** [`check`] names every benchmark
//!   measured in no part or in two, every part that lists other benchmarks than the first, and
//!   every benchmark measured that is not listed; a part that left no files is a missing file to
//!   the command line, which refuses it (`geom-bench-counts parts`).
//! - **Deterministic.** The plan depends only on the seconds and the limit: ties go to the lower
//!   part and the earlier name, and every map is a `BTreeMap`.
//! - **Strict reading.** [`Plan::parse`] refuses a malformed line, a group twice, no `*` line or
//!   two, a part numbered 0, and parts that are not numbered 1 to n without a gap.
//!
//! Method: longest processing time first (R. L. Graham, "Bounds on multiprocessing timing
//! anomalies", SIAM J. Appl. Math. 17(2), 1969, <https://doi.org/10.1137/0117039>), as
//! `geom_test::coverage::plan_parts` shares the coverage run's crates. Gungraun's summary model (`gungraun_runner::summary::model`, the
//! `summary` feature at exactly the version the gate runs) gives each tool run's `duration_ns`.
//!
//! Cost: [`plan`] is O(g · n) for g groups and n parts tried; [`check`] is O(b log b) for b
//! benchmarks over all parts.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use gungraun_runner::summary::model::BenchmarkSummary;

use crate::callgrind::benchmark_name;

/// The most seconds of measuring a part is planned for: 7 minutes. A part also sets up its runner
/// and builds its tree (4 minutes 19 seconds at most on the runs `parts.tsv` names), so a part of
/// this much measuring ends well within the 15 minutes a job may take.
pub const PART_SECONDS: u64 = 420;

/// The seconds the planted slowdown adds to the part that runs it: its rebuild of the benchmark
/// with the planted code and its one measured benchmark, 4 minutes 54 seconds on the slower leg of
/// main's run 43868715590 (aarch64).
pub const TRIP_SECONDS: u64 = 294;

/// The group whose part runs the planted slowdown: it compares `sign/of_i128` with the change's own
/// run of it, so it runs where that group was measured.
pub const TRIP_GROUP: &str = "sign";

/// Which part measures which group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Each group's part (1 to [`Plan::parts`]) and its measured seconds.
    pub groups: BTreeMap<String, (usize, u64)>,
    /// The part that also measures every group the plan does not list.
    pub rest: usize,
    /// How many parts.
    pub parts: usize,
}

/// A group's weight in the plan: its seconds, with [`TRIP_SECONDS`] for [`TRIP_GROUP`].
fn weight(group: &str, seconds: u64) -> u64 {
    if group == TRIP_GROUP {
        seconds.saturating_add(TRIP_SECONDS)
    } else {
        seconds
    }
}

/// Shares the groups among parts (the module documentation says how).
#[must_use]
pub fn plan(seconds: &BTreeMap<String, u64>, limit: u64) -> Plan {
    let mut order: Vec<(&String, u64)> = seconds
        .iter()
        .map(|(group, &s)| (group, weight(group, s)))
        .collect();
    order.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let most = order.len().max(1);
    let mut parts = 1;
    loop {
        let mut load = vec![0u64; parts];
        let mut count = vec![0usize; parts];
        let mut groups = BTreeMap::new();
        for &(group, w) in &order {
            let least = least(&load);
            if let (Some(l), Some(c)) = (load.get_mut(least), count.get_mut(least)) {
                *l = l.saturating_add(w);
                *c += 1;
            }
            groups.insert(
                group.clone(),
                (least + 1, seconds.get(group).copied().unwrap_or(0)),
            );
        }
        let fits = load.iter().zip(&count).all(|(&l, &c)| l <= limit || c == 1);
        if fits || parts >= most {
            return Plan {
                groups,
                rest: least(&load) + 1,
                parts,
            };
        }
        parts += 1;
    }
}

/// The index of the least of `load`, the first of equals.
fn least(load: &[u64]) -> usize {
    let mut at = 0;
    for (i, &l) in load.iter().enumerate() {
        if load.get(at).is_some_and(|&best| l < best) {
            at = i;
        }
    }
    at
}

impl Plan {
    /// Each part's planned seconds, with [`TRIP_SECONDS`] in the part of [`TRIP_GROUP`].
    #[must_use]
    pub fn load(&self) -> Vec<u64> {
        let mut load = vec![0u64; self.parts];
        for (group, &(part, s)) in &self.groups {
            if let Some(l) = load.get_mut(part.wrapping_sub(1)) {
                *l = l.saturating_add(weight(group, s));
            }
        }
        load
    }

    /// The part that measures `group`.
    #[must_use]
    pub fn part_of(&self, group: &str) -> usize {
        self.groups.get(group).map_or(self.rest, |&(part, _)| part)
    }

    /// The plan as `parts.tsv`, after the comment lines `header` (each given its `# `).
    #[must_use]
    pub fn text(&self, header: &[String]) -> String {
        let mut out = String::new();
        for line in header {
            let _ = writeln!(out, "# {line}");
        }
        for (group, (part, s)) in &self.groups {
            let _ = writeln!(out, "{group}\t{part}\t{s}");
        }
        let _ = writeln!(out, "*\t{}\t-", self.rest);
        out
    }

    /// Reads `parts.tsv` (the module documentation gives the format).
    ///
    /// # Errors
    ///
    /// A message naming the line, for anything [`Plan`]'s guarantees refuse.
    pub fn parse(text: &str) -> Result<Plan, String> {
        let mut groups = BTreeMap::new();
        let mut rest = None;
        for (at, line) in text.lines().enumerate() {
            let n = at + 1;
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            let [group, part, s] = fields.as_slice() else {
                return Err(format!(
                    "line {n}: not three tab-separated fields: {line:?}"
                ));
            };
            let part: usize = part
                .parse()
                .ok()
                .filter(|&p| p > 0)
                .ok_or_else(|| format!("line {n}: not a part number from 1: {part:?}"))?;
            if *group == "*" {
                if *s != "-" || rest.replace(part).is_some() {
                    return Err(format!("line {n}: a second `*` line, or one with seconds"));
                }
                continue;
            }
            let good = !group.is_empty()
                && group
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            let s: u64 = s
                .parse()
                .map_err(|_| format!("line {n}: not whole seconds: {s:?}"))?;
            if !good || groups.insert((*group).to_owned(), (part, s)).is_some() {
                return Err(format!(
                    "line {n}: a bad group name, or a group twice: {group:?}"
                ));
            }
        }
        let rest = rest.ok_or("no `*` line names the part for the groups not listed")?;
        let used: BTreeSet<usize> = groups.values().map(|&(p, _)| p).chain([rest]).collect();
        let parts = used.len();
        if used.iter().copied().ne(1..=parts) {
            return Err(format!("the parts are not numbered 1 to {parts}: {used:?}"));
        }
        Ok(Plan {
            groups,
            rest,
            parts,
        })
    }
}

/// Each group's whole seconds (rounded up) from each benchmark's nanoseconds, by the benchmark's
/// name `group/op/class`.
#[must_use]
pub fn group_seconds(durations: &[(String, u64)]) -> BTreeMap<String, u64> {
    let mut ns: BTreeMap<String, u64> = BTreeMap::new();
    for (name, d) in durations {
        let group = name.split('/').next().unwrap_or(name);
        let total = ns.entry(group.to_owned()).or_insert(0);
        *total = total.saturating_add(*d);
    }
    ns.into_iter()
        .map(|(group, d)| (group, d.div_ceil(1_000_000_000)))
        .collect()
}

/// Each benchmark's name and the nanoseconds of all its tool runs, from every `summary.json`
/// under `dir`, in path order.
///
/// # Errors
///
/// A message naming the folder or file that cannot be read, or the summary that cannot be parsed.
pub fn read_durations(dir: &Path) -> Result<Vec<(String, u64)>, String> {
    let mut found = Vec::new();
    for path in crate::callgrind::summary_files(dir)? {
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let summary: BenchmarkSummary = serde_json::from_str(&text)
            .map_err(|e| format!("{}: not a Gungraun summary: {e}", path.display()))?;
        let name = benchmark_name(&summary).map_err(|e| format!("{}: {e}", path.display()))?;
        let ns = summary
            .profiles
            .0
            .iter()
            .fold(0u64, |t, p| t.saturating_add(p.duration_ns));
        found.push((name, ns));
    }
    Ok(found)
}

/// The two sides a part measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The merge base: a part's saved files hold its base run alone, so a benchmark with any file
    /// was measured.
    Base,
    /// The change: a benchmark with a `summary.json` was measured (the gate saves one each).
    Head,
}

/// The bench targets the gate measures: the catalogue (`benches/instructions`) and EMBER's pair 827
/// (`benches/ember`, measured in the gate's last part).
pub const BENCHES: [&str; 2] = ["instructions", "ember"];

/// The benchmarks one part measured on `side`, from the list of its saved files (`tar -t` of its
/// Gungraun home: `geom-bench/<bench>/<group>/<op>.<class>/<file>`, `<bench>` one of
/// [`BENCHES`], with or without a leading `./`). Entries at other depths (the folders themselves)
/// are not files of a benchmark.
///
/// # Errors
///
/// A message naming an entry of a benchmark's folder that names no benchmark.
pub fn measured(listing: &str, side: Side) -> Result<BTreeSet<String>, String> {
    let mut names = BTreeSet::new();
    for entry in listing.lines() {
        let entry = entry.trim_end().trim_start_matches("./");
        let path: Vec<&str> = entry.split('/').collect();
        let [package, bench, group, folder, file] = path.as_slice() else {
            continue;
        };
        if *package != "geom-bench" || !BENCHES.contains(bench) || file.is_empty() {
            continue;
        }
        if side == Side::Head && *file != "summary.json" {
            continue;
        }
        let Some((op, class)) = folder.split_once('.') else {
            return Err(format!("{entry}: the folder names no benchmark"));
        };
        if group.is_empty() || op.is_empty() || class.is_empty() {
            return Err(format!("{entry}: the folder names no benchmark"));
        }
        names.insert(format!("{group}/{op}/{class}"));
    }
    Ok(names)
}

/// What is wrong with the parts: `listed` holds each part's list of the run's benchmarks (all the
/// same), `done` each part's measured benchmarks, part 1 first. Empty when every listed benchmark
/// was measured in exactly one part and nothing else was.
#[must_use]
pub fn check(listed: &[BTreeSet<String>], done: &[BTreeSet<String>]) -> Vec<String> {
    let mut found = Vec::new();
    let Some(first) = listed.first() else {
        return vec!["no part".to_owned()];
    };
    if listed.len() != done.len() {
        found.push(format!(
            "{} lists of benchmarks but {} parts measured",
            listed.len(),
            done.len()
        ));
    }
    for (k, names) in listed.iter().enumerate().skip(1) {
        if names != first {
            found.push(format!("part {} lists other benchmarks than part 1", k + 1));
        }
    }
    let mut parts: BTreeMap<&str, Vec<usize>> =
        first.iter().map(|n| (n.as_str(), vec![])).collect();
    for (k, names) in done.iter().enumerate() {
        for name in names {
            match parts.get_mut(name.as_str()) {
                Some(at) => at.push(k + 1),
                None => found.push(format!(
                    "part {} measured {name}, which the run does not list",
                    k + 1
                )),
            }
        }
    }
    for (name, at) in parts {
        match at.as_slice() {
            [_] => {}
            [] => found.push(format!("{name} was measured in no part")),
            more => found.push(format!("{name} was measured in parts {more:?}")),
        }
    }
    found
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|&n| n.to_owned()).collect()
    }

    fn secs(pairs: &[(&str, u64)]) -> BTreeMap<String, u64> {
        pairs.iter().map(|&(g, s)| (g.to_owned(), s)).collect()
    }

    #[test]
    fn the_plan_fits_the_limit_with_the_fewest_parts() {
        let s = secs(&[
            ("mesh", 420),
            ("a", 200),
            ("b", 200),
            ("c", 100),
            ("sign", 1),
        ]);
        let p = plan(&s, 420);
        // One part over 420 holds mesh alone; the others fit, sign weighed with the trip.
        assert_eq!(p.parts, 3, "{p:?}");
        assert_eq!(p.part_of("mesh"), 1);
        assert!(p.load().iter().all(|&l| l <= 420), "{:?}", p.load());
        // A group the plan does not list goes to the part with the least.
        let least = p.load().iter().copied().min().unwrap();
        assert_eq!(p.load()[p.rest - 1], least);
        assert_eq!(p.part_of("new_group"), p.rest);
        // A group too long for any part still gets a part of its own, and the plan ends.
        let alone = plan(&secs(&[("x", 5000), ("y", 5000)]), 420);
        assert_eq!(alone.parts, 2);
        assert_eq!(plan(&BTreeMap::new(), 420).parts, 1);
    }

    #[test]
    fn a_plan_reads_back_as_written_and_bad_files_are_refused() {
        let p = plan(&secs(&[("mesh", 400), ("a", 300), ("b", 200)]), 420);
        let text = p.text(&["from run 1".to_owned()]);
        assert!(text.starts_with("# from run 1\n"));
        assert_eq!(Plan::parse(&text).unwrap(), p);
        for bad in [
            "a\t1\t3\n",                   // no `*` line
            "a\t1\t3\n*\t1\t-\n*\t1\t-\n", // two
            "a\t1\t3\na\t1\t3\n*\t1\t-\n", // a group twice
            "a\t0\t3\n*\t1\t-\n",          // part 0
            "a\t1\t3\n*\t3\t-\n",          // a gap: no part 2
            "a\t1\tx\n*\t1\t-\n",          // not seconds
            "A b\t1\t3\n*\t1\t-\n",        // a bad name
            "a 1 3\n*\t1\t-\n",            // not tabs
        ] {
            assert!(Plan::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_committed_plan_is_the_tools_own() {
        // parts.tsv is `geom-bench-counts plan`'s output, never edited by hand: its parts are the
        // plan of its own seconds, at the limit the tool plans for.
        let committed = Plan::parse(include_str!("../parts.tsv")).unwrap();
        let seconds = committed
            .groups
            .iter()
            .map(|(g, &(_, s))| (g.clone(), s))
            .collect();
        assert_eq!(plan(&seconds, PART_SECONDS), committed);
        // Planted: the part of the groups not listed changed by hand, and a group moved to
        // another part by hand.
        let text = include_str!("../parts.tsv");
        let other = if committed.rest == 1 { 2 } else { 1 };
        let moved = text.replacen(
            &format!("*\t{}\t-", committed.rest),
            &format!("*\t{other}\t-"),
            1,
        );
        assert_ne!(plan(&seconds, PART_SECONDS), Plan::parse(&moved).unwrap());
        let sign = committed.part_of("sign");
        let to = if sign == committed.parts { 1 } else { sign + 1 };
        let moved = text.replacen(&format!("\nsign\t{sign}\t"), &format!("\nsign\t{to}\t"), 1);
        assert_ne!(moved, text);
        assert_ne!(plan(&seconds, PART_SECONDS), Plan::parse(&moved).unwrap());
    }

    #[test]
    fn seconds_are_summed_by_group_and_rounded_up() {
        let d = [
            ("mesh/envelope/typical".to_owned(), 1_500_000_000),
            ("mesh/hull/typical".to_owned(), 600_000_000),
            ("sign/of_i128/typical".to_owned(), 1),
        ];
        assert_eq!(group_seconds(&d), secs(&[("mesh", 3), ("sign", 1)]));
    }

    const LISTING: &str = "./\n./geom-bench/\n./geom-bench/instructions/\n\
        ./geom-bench/instructions/ball/\n./geom-bench/instructions/ball/cubic.typical/\n\
        ./geom-bench/instructions/ball/cubic.typical/callgrind.cubic.typical.out.base@base\n\
        ./geom-bench/instructions/ball/cubic.typical/summary.json\n\
        ./geom-bench/instructions/ball/cubic.extreme/dhat.cubic.extreme.out.base@base\n";

    #[test]
    fn a_part_measured_what_its_files_show() {
        assert_eq!(
            measured(LISTING, Side::Base).unwrap(),
            set(&["ball/cubic/extreme", "ball/cubic/typical"])
        );
        assert_eq!(
            measured(LISTING, Side::Head).unwrap(),
            set(&["ball/cubic/typical"])
        );
        assert!(
            measured(
                "geom-bench/instructions/ball/cubic/summary.json\n",
                Side::Head
            )
            .is_err()
        );
        // EMBER's pair, in the gate's last part, is a benchmark too; another bench target's files
        // are not.
        let ember = "./geom-bench/ember/mesh/ember.pair_827/summary.json\n\
            ./geom-bench/wall_clock/mesh/ember.pair_827/summary.json\n";
        assert_eq!(
            measured(ember, Side::Head).unwrap(),
            set(&["mesh/ember/pair_827"])
        );
    }

    #[test]
    fn every_benchmark_is_measured_in_exactly_one_part_or_the_parts_are_refused() {
        let all = set(&["a/x/t", "b/y/t", "c/z/t"]);
        let good = [set(&["a/x/t", "c/z/t"]), set(&["b/y/t"])];
        assert!(check(&[all.clone(), all.clone()], &good).is_empty());
        // Planted: a benchmark in no part (a part measured less).
        let found = check(
            &[all.clone(), all.clone()],
            &[set(&["a/x/t"]), set(&["b/y/t"])],
        );
        assert!(
            found.iter().any(|p| p == "c/z/t was measured in no part"),
            "{found:?}"
        );
        // Planted: a benchmark in two parts (a doubled count).
        let found = check(
            &[all.clone(), all.clone()],
            &[set(&["a/x/t", "c/z/t"]), set(&["b/y/t", "c/z/t"])],
        );
        assert!(
            found
                .iter()
                .any(|p| p == "c/z/t was measured in parts [1, 2]"),
            "{found:?}"
        );
        // Planted: a part that dropped out leaves its benchmarks in no part.
        let found = check(std::slice::from_ref(&all), &[set(&["a/x/t", "c/z/t"])]);
        assert!(
            found
                .iter()
                .any(|p| p.contains("b/y/t was measured in no part")),
            "{found:?}"
        );
        // Planted: a part that lists another catalogue, and a count of no listed benchmark.
        let found = check(&[all.clone(), set(&["a/x/t"])], &good);
        assert!(
            found.iter().any(|p| p.contains("part 2 lists other")),
            "{found:?}"
        );
        let found = check(
            &[all.clone(), all.clone()],
            &[good[0].clone(), set(&["b/y/t", "d/w/t"])],
        );
        assert!(
            found.iter().any(|p| p.contains("measured d/w/t, which")),
            "{found:?}"
        );
        assert!(!check(&[], &[]).is_empty());
    }
}
