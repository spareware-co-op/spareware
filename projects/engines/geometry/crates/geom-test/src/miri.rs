//! The Miri run's scope, every kernel crate, whose test suites Miri interprets, and its partitions,
//! balanced by each test's measured time.
//!
//! What it does: [`packages`] reads `cargo metadata` and returns the package of every kernel crate,
//! in name order, and [`cargo_args`] writes them as the `-p` arguments the workflow
//! (`.github/workflows/kernel-quality.yml`) gives `cargo miri nextest run` and `cargo miri test
//! --doc`. nextest runs each test in its own Miri process, one partition per CI job, and stops a
//! test that reaches its budget (the `default-miri` profile in `.config/nextest.toml`): such a
//! test passed every step Miri interpreted, and the job's report names it. [`plan`] splits the
//! tests into the partitions by their times under Miri in [`TIMES`] (`miri-times.tsv`, from a full
//! run's reports), and [`Plan::filterset`] writes partition k's tests as the nextest filterset
//! its job runs.
//!
//! Guarantees:
//!
//! - **Every kernel test suite, with nothing to keep in step.** The scope is every kernel crate in
//!   the metadata, so a new crate or test file runs under Miri the week it lands. A test that
//!   cannot run under Miri at all (one that starts a process or reads the repository) says so
//!   where it is written, with `#[cfg_attr(miri, ignore = "<why>")]`.
//! - **Every test in exactly one partition, the times list only balancing them.** A test in the
//!   list runs in the partition the plan gives it; a test the list does not name runs in the
//!   partition its binary is given (each binary named in the list goes to one partition, by its
//!   place in name order); a binary the list does not name runs in partition 1. So a test added
//!   or renamed since the list was written still runs, once, and a stale list makes the
//!   partitions less even, never a test skipped.
//! - **No crate of [`FINISHED`] has a test Miri stops.** A test still running at its [`BUDGET`]
//!   is stopped and gives no verdict, so Miri never checks the code it had not reached. Each crate
//!   of [`FINISHED`] runs its slow tests on a smaller case under Miri, the same functions reached;
//!   `kernel_quality_workflow.rs` refuses a row of such a crate that reads the budget and any test
//!   of it ignored or compiled out under Miri, so none can drop out of Miri unseen.
//! - **Balanced.** Longest processing time first (Graham): each test, the longest first, goes to
//!   the partition with the least time so far, so no partition passes the average by more than its
//!   longest test. With [`AT_ONCE`] tests at a time, [`Plan::longest`] is how long the slowest
//!   partition's tests take, which `kernel_quality_workflow.rs` holds under [`PART_SECONDS`] for
//!   the workflow's count.
//! - **The development crates are left out** (`geom-test`, `geom-fuzz`, `geom-gen`,
//!   `geom-bench`): they never ship, and their tests start processes and read files, which Miri's
//!   isolation refuses.
//! - **A plan that runs nothing does not pass.** Metadata without any kernel crate, or with a
//!   package that has no name or manifest path, is an error; so are a times list with a line that
//!   is not `seconds<TAB>binary<TAB>test`, a name a filterset could misread, a test listed twice,
//!   and a plan of no partitions.
//!
//! Method: the `packages[]` of `cargo metadata --format-version 1`
//! (<https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>); a kernel crate is a package
//! whose manifest is in the kernel's crates folder, as [`crate::deps`] defines it. nextest's Miri
//! support and filtersets are documented at <https://nexte.st/docs/integrations/miri/> and
//! <https://nexte.st/docs/filtersets/>. The partitions by R. L. Graham, "Bounds on multiprocessing
//! timing anomalies", SIAM Journal on Applied Mathematics 17(2) (1969), 416–429,
//! <https://doi.org/10.1137/0117039> (the longest-processing-time rule, within 4/3 of the best).
//!
//! Cost: linear in the size of the metadata; a plan is O(t log t + t·p) for t tests and p
//! partitions, and a filterset linear in its partition's tests and binaries.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::deps::{DEVELOPMENT_CRATES, KERNEL_CRATES_DIR};

/// Each kernel test's time under Miri: `seconds<TAB>binary id<TAB>test name` a line, from the
/// `junit.xml` reports of a full run, `#` lines comments.
pub const TIMES: &str = include_str!("miri-times.tsv");

/// The tests a partition runs at once: nextest's default of one per core, and a GitHub-hosted
/// Linux runner has four.
pub const AT_ONCE: u64 = 4;

/// The longest a partition's tests may take, in seconds (the owner's testing principle 8: no run
/// takes more than a few minutes). A test Miri stops at its budget takes 600.
pub const PART_SECONDS: u64 = 900;

/// A test's budget under Miri, in seconds: the `default-miri` profile's two periods of five
/// minutes (`.config/nextest.toml`). A test still running then is stopped, and its row reads this.
pub const BUDGET: u64 = 600;

/// The kernel crates each of whose tests Miri finishes within its [`BUDGET`]: their slow tests run
/// a smaller case under Miri (each crate's `tests/miri/mod.rs` says how), and no row of theirs in
/// [`TIMES`] reads the budget. A crate joins once its stopped tests are cut.
pub const FINISHED: [&str; 1] = ["geom-poly"];

/// One test's measured time under Miri.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timed {
    /// Whole seconds.
    pub seconds: u64,
    /// nextest's binary id: `crate` for its unit tests, `crate::file` for an integration test.
    pub binary: String,
    /// The test's name within its binary.
    pub test: String,
}

/// Whether `name` holds only what a test or binary name may: letters, digits, `_`, `-` and `:`,
/// none of which a filterset reads as anything but part of the name.
fn plain(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b':'))
}

/// The tests of a times list, in the order given.
///
/// # Errors
///
/// The line, if one is not three tab-separated fields, its seconds not a whole number, a name not
/// [`plain`], or a test listed twice.
pub fn parse_times(text: &str) -> Result<Vec<Timed>, String> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let fields: Vec<&str> = line.split('\t').collect();
        let [seconds, binary, name] = fields.as_slice() else {
            return Err(format!("not seconds, binary and test: {line:?}"));
        };
        let seconds = seconds
            .parse::<u64>()
            .map_err(|e| format!("{line:?}: {e}"))?;
        if !plain(binary) || !plain(name) {
            return Err(format!("a name a filterset could misread: {line:?}"));
        }
        if !seen.insert((*binary, *name)) {
            return Err(format!("listed twice: {line:?}"));
        }
        out.push(Timed {
            seconds,
            binary: (*binary).to_owned(),
            test: (*name).to_owned(),
        });
    }
    Ok(out)
}

/// The partitions: which listed tests each runs, and the listed binaries whose other tests it
/// runs.
#[derive(Clone, Debug)]
pub struct Plan {
    /// Every listed test.
    tests: Vec<Timed>,
    /// For each partition, its listed tests and their total seconds.
    parts: Vec<(Vec<Timed>, u64)>,
    /// Each listed binary and the partition (from 0) that runs its unlisted tests.
    binaries: BTreeMap<String, usize>,
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
    let names: BTreeSet<&str> = tests.iter().map(|t| t.binary.as_str()).collect();
    let binaries = names
        .into_iter()
        .zip((0..parts).cycle())
        .map(|(b, p)| (b.to_owned(), p))
        .collect();
    Ok(Plan {
        tests: tests.to_vec(),
        parts: out,
        binaries,
    })
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

    /// The listed tests of partition `k` (from 1), as (binary, test).
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

    /// The partition (from 1) that runs the unlisted tests of `binary`, if the list names it.
    #[must_use]
    pub fn partition_of_binary(&self, binary: &str) -> Option<usize> {
        self.binaries.get(binary).and_then(|p| p.checked_add(1))
    }

    /// The seconds its listed tests take, at [`AT_ONCE`] at a time, in the slowest partition: the
    /// longer of its total over [`AT_ONCE`], rounded up, and its longest test.
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

    /// The nextest filterset of partition `k` (from 1): its listed tests, the unlisted tests of
    /// the listed binaries it is given, and in partition 1 every binary the list does not name.
    ///
    /// # Errors
    ///
    /// For a `k` outside 1 ..= [`Plan::len`].
    pub fn filterset(&self, k: usize) -> Result<String, String> {
        let p = k
            .checked_sub(1)
            .filter(|&p| p < self.parts.len())
            .ok_or_else(|| format!("no partition {k} of {}", self.parts.len()))?;
        let mut terms: Vec<String> = self
            .tests_of(k)
            .into_iter()
            .map(|(b, t)| format!("(binary_id(={b}) & test(={t}))"))
            .collect();
        for (binary, _) in self.binaries.iter().filter(|(_, q)| **q == p) {
            let listed: Vec<String> = self
                .tests
                .iter()
                .filter(|t| &t.binary == binary)
                .map(|t| format!("test(={})", t.test))
                .collect();
            terms.push(format!(
                "(binary_id(={binary}) & not ({}))",
                listed.join(" | ")
            ));
        }
        if p == 0 {
            let known: Vec<String> = self
                .binaries
                .keys()
                .map(|b| format!("binary_id(={b})"))
                .collect();
            terms.push(if known.is_empty() {
                "all()".into()
            } else {
                format!("not ({})", known.join(" | "))
            });
        }
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

    fn timed(seconds: u64, binary: &str, test: &str) -> Timed {
        Timed {
            seconds,
            binary: binary.into(),
            test: test.into(),
        }
    }

    #[test]
    fn a_times_list_is_read_or_refused_by_the_line() {
        let text = "# a comment\n600\tgeom-num::wide\ti256::add\n\n3\tgeom-num\tdd::tests::x\n";
        assert_eq!(
            parse_times(text).unwrap(),
            [
                timed(600, "geom-num::wide", "i256::add"),
                timed(3, "geom-num", "dd::tests::x")
            ]
        );
        for bad in [
            "600\tgeom-num::wide\n",
            "600\tgeom-num::wide\tx\ty\n",
            "ten\tgeom-num\tx\n",
            "-1\tgeom-num\tx\n",
            "1\tgeom num\tx\n",
            "1\tgeom-num\tx)\n",
            "1\tgeom-num\tx | all()\n",
            "1\t\tx\n",
            "1\tgeom-num\tx\n2\tgeom-num\tx\n",
        ] {
            assert!(parse_times(bad).is_err(), "{bad:?}");
        }
    }

    /// The names a filterset gives, as (binary, test) for `binary_id(=b) & test(=t)` and
    /// (binary, "") for a binary's other tests.
    fn named(filter: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for term in filter.split(" | (") {
            let term = term.trim_start_matches('(');
            let Some(rest) = term.strip_prefix("binary_id(=") else {
                continue;
            };
            let (binary, rest) = rest.split_once(')').unwrap();
            let test = rest
                .strip_prefix(" & test(=")
                .and_then(|r| r.split_once(')'))
                .map_or("", |(t, _)| t);
            out.push((binary.to_owned(), test.to_owned()));
        }
        out
    }

    #[test]
    fn every_test_and_binary_is_in_one_partition_and_the_longest_go_first() {
        let tests = [
            timed(600, "a", "slow"),
            timed(600, "a", "slow2"),
            timed(500, "b", "mid"),
            timed(100, "b", "quick"),
            timed(90, "c", "x"),
            timed(10, "c", "y"),
        ];
        let p = plan(&tests, 3).unwrap();
        assert_eq!(p.len(), 3);
        // Longest first, each to the partition with the least so far, the first of equals: 600,
        // 600 and 500 one each; 100 to the third (500); 90 to the first (all at 600); 10 to the
        // second (600 against the first's 690).
        assert_eq!(p.tests_of(1), [("a", "slow"), ("c", "x")]);
        assert_eq!(p.tests_of(2), [("a", "slow2"), ("c", "y")]);
        assert_eq!(p.tests_of(3), [("b", "mid"), ("b", "quick")]);
        assert!(p.tests_of(0).is_empty() && p.tests_of(4).is_empty());
        // At four at once a partition takes at least its longest test.
        assert_eq!(p.longest(), 600);
        // Each listed test once, over all partitions' filtersets.
        let mut seen = Vec::new();
        let mut others = Vec::new();
        for k in 1..=3 {
            for (b, t) in named(&p.filterset(k).unwrap()) {
                if t.is_empty() {
                    others.push((b, k));
                } else {
                    seen.push((b, t));
                }
            }
        }
        seen.sort();
        let mut want: Vec<(String, String)> = tests
            .iter()
            .map(|t| (t.binary.clone(), t.test.clone()))
            .collect();
        want.sort();
        assert_eq!(seen, want);
        // Each binary's other tests in one partition, the one the plan says.
        others.sort();
        assert_eq!(others, [("a".into(), 1), ("b".into(), 2), ("c".into(), 3)]);
        for (b, k) in &others {
            assert_eq!(p.partition_of_binary(b), Some(*k));
        }
        assert_eq!(p.partition_of_binary("d"), None);
        // A binary's other tests are all of it but its listed ones; unknown binaries go to 1.
        let one = p.filterset(1).unwrap();
        assert!(
            one.contains("(binary_id(=a) & not (test(=slow) | test(=slow2)))"),
            "{one}"
        );
        assert!(
            one.ends_with("not (binary_id(=a) | binary_id(=b) | binary_id(=c))"),
            "{one}"
        );
        assert!(!p.filterset(2).unwrap().contains("not (binary_id"));
        assert!(p.filterset(0).is_err() && p.filterset(4).is_err());
        assert!(plan(&tests, 0).is_err());
        // No tests listed: partition 1 runs everything, the others nothing.
        let empty = plan(&[], 2).unwrap();
        assert_eq!(empty.filterset(1).unwrap(), "all()");
        assert_eq!(empty.filterset(2).unwrap(), "none()");
    }

    #[test]
    fn the_recorded_times_read_and_balance() {
        let tests = parse_times(TIMES).unwrap();
        assert!(tests.len() > 2000, "{}", tests.len());
        // With one partition per hour of tests at once, no partition passes the average by more
        // than its longest test.
        let total: u64 = tests.iter().fold(0, |a, t| a + t.seconds);
        let p = plan(&tests, 16).unwrap();
        let average = total.div_ceil(16 * AT_ONCE);
        assert!(p.longest() <= average + 600, "{} {average}", p.longest());
    }
}
