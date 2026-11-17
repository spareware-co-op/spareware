//! The determinism matrix's runner: every golden of the current edition, run by cargo in whatever
//! configuration a CI leg sets, with proof that each one really ran and agreed.
//!
//! What it does: [`plan`] groups the current edition's test-checked goldens by crate and test
//! target; [`Group::cargo_args`] gives the `cargo test` arguments that run exactly those test
//! functions (libtest's `--exact` filters) with the leg's own arguments (`--release`, `--target
//! wasm32-wasip1`, ...) and its environment (`RUSTFLAGS` with `-C target-cpu=native` or overflow
//! checks off, `CARGO_TARGET_DIR`), which cargo inherits; [`verify`] reads a run's output and
//! requires that libtest ran exactly the planned functions, all passed, and that every planned
//! golden printed its `golden <id> ... agrees with the store` line ([`super::check`]);
//! [`build_args`] gives the one `cargo test --no-run` that builds every group's binary side by side
//! before the groups run, and [`part`] shares the groups out among CI jobs, whole crates to each
//! part, balanced by measured seconds, each group in exactly one part. The binary
//! `golden run` drives it, and `.github/workflows/kernel.yml` runs it on every target and build
//! mode: debug, release with overflow checks on and as shipped, natively with
//! `-C target-cpu=native`, and on wasm32-wasip1 under wasmtime and Node in debug and release.
//!
//! Why a runner, rather than one `cargo test` per golden written into the workflow: the store is
//! the one list of goldens, so a golden added to it joins every leg at once, and a filter that
//! matched nothing (a renamed test, a moved file) cannot pass silently, which libtest's own
//! filters allow (zero tests run is a success to libtest).
//!
//! Guarantees:
//!
//! - **Exactly the store's goldens.** One group per crate and test target, in store order, each
//!   naming its functions exactly; a golden checked by a file has no test and is checked by
//!   `golden check` instead.
//! - **Nothing passes unrun.** [`verify`] fails unless the run has exactly one libtest summary,
//!   its verdict is `ok.`, it says exactly the group's number of tests passed, none failed and none
//!   was ignored, and every planned id printed its agreeing line: `golden <id> (edition
//!   <label>): <64 hexadecimal digits> agrees with the store`, read wherever it starts in an
//!   output line. libtest runs tests on one thread on wasm32-wasip1 (no threads) and whenever
//!   `RUST_TEST_THREADS=1`, and then prints `test <name> ... ` before the test runs, so the test's
//!   own first line follows it on the same output line; with several threads it starts its own.
//! - **Every group in exactly one part.** [`part`] places every crate of the plan in one part of
//!   `n`, so the parts of a leg run every group once between them, a golden added to the store
//!   included.
//!
//! Method: libtest's documented output (`test result: ok. N passed; M failed; I ignored; ...`) and
//! command-line filters (<https://doc.rust-lang.org/rustc/tests/index.html>), the golden matrix of
//! chapters 21 and 28.13 (`projects/engines/geometry/architecture/21-determinism.md`).
//!
//! Cost: linear in the store and in the output read.

use core::fmt;

use super::sha256::Digest;
use super::store::Store;

/// The goldens one `cargo test` invocation runs: one crate's test target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The crate.
    pub krate: String,
    /// The test target: an integration test's name, or `lib`.
    pub test: String,
    /// The test functions, in store order, each once.
    pub functions: Vec<String>,
    /// The golden ids those functions check, in store order.
    pub ids: Vec<String>,
}

impl Group {
    /// The arguments of `cargo` that run this group, with the leg's own `cargo test` arguments
    /// (`extra`) before libtest's.
    #[must_use]
    pub fn cargo_args(&self, extra: &[String]) -> Vec<String> {
        let mut args: Vec<String> = vec![
            "test".to_owned(),
            "--locked".to_owned(),
            "-p".to_owned(),
            self.krate.clone(),
        ];
        if self.test == "lib" {
            args.push("--lib".to_owned());
        } else {
            args.push("--test".to_owned());
            args.push(self.test.clone());
        }
        args.extend(extra.iter().cloned());
        args.push("--".to_owned());
        args.push("--exact".to_owned());
        args.push("--nocapture".to_owned());
        args.extend(self.functions.iter().cloned());
        args
    }
}

/// The `cargo` invocations that build every group's test target at once, before any group runs:
/// one for the integration tests (every group's crate and test name), one for the crates whose
/// unit tests hold goldens (`--lib`), each with the leg's own arguments. One cargo invocation
/// builds its targets side by side; a group's own `cargo test` then finds its binary built in the
/// same configuration and only runs it. Cargo builds a named test in every selected crate that has
/// one, so a test name two crates share is built in both: more than the plan runs, never less.
#[must_use]
pub fn build_args(groups: &[Group], extra: &[String]) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for lib in [false, true] {
        let mut crates: Vec<&str> = Vec::new();
        let mut tests: Vec<&str> = Vec::new();
        for g in groups.iter().filter(|g| (g.test == "lib") == lib) {
            if !crates.contains(&g.krate.as_str()) {
                crates.push(&g.krate);
            }
            if !lib && !tests.contains(&g.test.as_str()) {
                tests.push(&g.test);
            }
        }
        if crates.is_empty() {
            continue;
        }
        let mut args: Vec<String> = vec!["test".to_owned(), "--locked".to_owned()];
        args.push("--no-run".to_owned());
        for c in crates {
            args.push("-p".to_owned());
            args.push(c.to_owned());
        }
        if lib {
            args.push("--lib".to_owned());
        }
        for t in tests {
            args.push("--test".to_owned());
            args.push(t.to_owned());
        }
        args.extend(extra.iter().cloned());
        out.push(args);
    }
    out
}

/// Seconds each crate's golden groups took to run in release on macOS x86-64, the slowest native
/// leg, on its quicker runners (main's run 44052487998 of 17 November 2026, parts 1 and 2 of the
/// checked goldens, from each group's `golden run: cargo` line to the next). They only balance
/// [`part`]: a crate not listed counts [`UNMEASURED_SECONDS`] and still runs in exactly one part.
/// The six long ones (geom-run to geom-topo) each go to a part of their own when a leg has six.
const RELEASE_SECONDS: [(&str, u64); 16] = [
    ("geom-run", 141),
    ("geom-num", 130),
    ("geom-format", 110),
    ("geom-linalg", 100),
    ("geom-poly", 81),
    ("geom-topo", 78),
    ("geom-test", 30),
    ("geom-curve", 30),
    ("geom-math", 15),
    ("geom-surface", 12),
    ("geom-nurbs", 11),
    ("geom-mesh", 6),
    ("geom-op", 5),
    ("geom-bvh", 5),
    ("geom-name", 3),
    ("geom-make", 2),
];

/// The same in debug (the same run's `Test (macOS x86-64, debug)`), where geom-test's polyhedral
/// corpus is the longest and geom-run's golden is short.
const DEBUG_SECONDS: [(&str, u64); 16] = [
    ("geom-test", 200),
    ("geom-num", 80),
    ("geom-poly", 52),
    ("geom-curve", 50),
    ("geom-format", 46),
    ("geom-mesh", 35),
    ("geom-surface", 35),
    ("geom-math", 15),
    ("geom-run", 10),
    ("geom-topo", 8),
    ("geom-linalg", 8),
    ("geom-nurbs", 5),
    ("geom-bvh", 3),
    ("geom-op", 3),
    ("geom-name", 3),
    ("geom-make", 2),
];

/// What [`part`] counts for a crate its table does not list.
const UNMEASURED_SECONDS: u64 = 30;

/// The groups of part `k` of `n` (1 to `n`), so that `n` CI jobs side by side run every group
/// exactly once between them. Whole crates are shared out, so a crate's groups build their crate
/// once: the longest first ([`RELEASE_SECONDS`] when `release`, else [`DEBUG_SECONDS`], then the
/// name), each to the part with the fewest seconds so far (the lowest part on a tie). The groups
/// keep their plan order. `None` when `k`
/// is not in 1 to `n`.
#[must_use]
pub fn part(groups: &[Group], k: usize, n: usize, release: bool) -> Option<Vec<Group>> {
    if k == 0 || k > n {
        return None;
    }
    let table: &[(&str, u64)] = if release {
        &RELEASE_SECONDS
    } else {
        &DEBUG_SECONDS
    };
    let seconds = |c: &str| {
        table
            .iter()
            .find(|(name, _)| *name == c)
            .map_or(UNMEASURED_SECONDS, |(_, s)| *s)
    };
    let mut crates: Vec<&str> = Vec::new();
    for g in groups {
        if !crates.contains(&g.krate.as_str()) {
            crates.push(&g.krate);
        }
    }
    crates.sort_by(|a, b| seconds(b).cmp(&seconds(a)).then(a.cmp(b)));
    let mut load: Vec<u64> = vec![0; n];
    let mut home: Vec<(&str, usize)> = Vec::new();
    for c in crates {
        let (at, least) = load
            .iter()
            .enumerate()
            .min_by_key(|(i, l)| (**l, *i))
            .map_or((0, 0), |(i, l)| (i, *l));
        if let Some(slot) = load.get_mut(at) {
            *slot = least.saturating_add(seconds(c));
        }
        home.push((c, at));
    }
    Some(
        groups
            .iter()
            .filter(|g| {
                home.iter()
                    .any(|(c, at)| *c == g.krate && at.checked_add(1) == Some(k))
            })
            .cloned()
            .collect(),
    )
}

/// Groups the current edition's test-checked goldens by crate and test target, in store order.
#[must_use]
pub fn plan(store: &Store) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    let Ok(edition) = store.current() else {
        return groups;
    };
    for golden in &edition.goldens {
        let Some(check) = &golden.check else { continue };
        let position = groups
            .iter()
            .position(|g| g.krate == check.krate && g.test == check.test);
        let group = if let Some(at) = position {
            groups.get_mut(at)
        } else {
            groups.push(Group {
                krate: check.krate.clone(),
                test: check.test.clone(),
                functions: Vec::new(),
                ids: Vec::new(),
            });
            groups.last_mut()
        };
        if let Some(group) = group {
            if !group.functions.contains(&check.function) {
                group.functions.push(check.function.clone());
            }
            group.ids.push(golden.id.clone());
        }
    }
    groups
}

/// Why a run does not prove its group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunError {
    /// No libtest summary line: the binary did not run, or crashed.
    NoSummary,
    /// The summary's counts are not exactly the group's.
    Counts {
        /// Tests passed.
        passed: u64,
        /// Tests failed.
        failed: u64,
        /// Tests ignored.
        ignored: u64,
        /// Tests the group runs.
        planned: usize,
    },
    /// libtest's verdict is not `ok.` (the summary line, as printed).
    NotOk(String),
    /// A planned golden printed no agreeing line.
    Silent(String),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::NoSummary => write!(f, "libtest printed no summary: the tests did not run"),
            RunError::Counts {
                passed,
                failed,
                ignored,
                planned,
            } => write!(
                f,
                "libtest ran {passed} passed, {failed} failed, {ignored} ignored, but the group \
                 has {planned} golden test(s): each must run and pass"
            ),
            RunError::NotOk(summary) => write!(
                f,
                "libtest's summary is \"test result: {summary}\", not ok: the golden tests did \
                 not all pass"
            ),
            RunError::Silent(id) => write!(
                f,
                "golden {id} printed no line saying it agrees with the store: its test ran \
                 without checking it"
            ),
        }
    }
}

impl core::error::Error for RunError {}

/// The number before `word` in a summary such as `ok. 3 passed; 0 failed; ...`.
fn count_before(summary: &str, word: &str) -> Option<u64> {
    summary.split(';').find_map(|part| {
        let mut it = part.split_whitespace().rev();
        let (last, number) = (it.next()?, it.next()?);
        if last == word {
            number.parse().ok()
        } else {
            None
        }
    })
}

/// Checks a run's output (stdout) against its group.
///
/// # Errors
///
/// A [`RunError`] saying what the run fails to prove.
pub fn verify(group: &Group, output: &str) -> Result<(), RunError> {
    let summaries: Vec<&str> = output
        .lines()
        .filter_map(|l| l.trim().strip_prefix("test result: "))
        .collect();
    let [summary] = summaries.as_slice() else {
        return Err(RunError::NoSummary);
    };
    // libtest's verdict comes first: `ok.` or `FAILED.`; anything but `ok.` is no proof.
    let Some(summary) = summary.strip_prefix("ok.") else {
        return Err(RunError::NotOk((*summary).to_owned()));
    };
    let (Some(passed), Some(failed), Some(ignored)) = (
        count_before(summary, "passed"),
        count_before(summary, "failed"),
        count_before(summary, "ignored"),
    ) else {
        return Err(RunError::NoSummary);
    };
    let planned = group.functions.len();
    if usize::try_from(passed).ok() != Some(planned) || failed != 0 || ignored != 0 {
        return Err(RunError::Counts {
            passed,
            failed,
            ignored,
            planned,
        });
    }
    for id in &group.ids {
        if !output.lines().any(|line| agrees_in(line, id)) {
            return Err(RunError::Silent(id.clone()));
        }
    }
    Ok(())
}

/// Whether `line` holds the agreeing line [`super::check`] prints for golden `id`, `golden <id>
/// (edition <label>): <digest> agrees with the store`, starting anywhere in it (after libtest's
/// `test <name> ... ` when tests run on one thread). The label must be an edition label and the
/// digest 64 lowercase hexadecimal digits, so a line for another golden, a differing digest or
/// text that only mentions the id never counts.
fn agrees_in(line: &str, id: &str) -> bool {
    let prefix = format!("golden {id} (edition ");
    line.match_indices(prefix.as_str()).any(|(at, _)| {
        let Some(rest) = line.get(at..).and_then(|s| s.strip_prefix(prefix.as_str())) else {
            return false;
        };
        let Some((edition, rest)) = rest.split_once("): ") else {
            return false;
        };
        let label = !edition.is_empty()
            && edition
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        let (Some(digest), Some(verdict)) = (rest.get(..64), rest.get(64..)) else {
            return false;
        };
        label && Digest::is_canonical_hex(digest) && verdict.starts_with(" agrees with the store")
    })
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use super::*;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn golden(id: &str, test: &str, function: &str) -> String {
        format!(
            r#"{{"id": "{id}", "what": "w", "sha256": "{A}",
                "check": {{"crate": "{}", "test": "{test}", "function": "{function}"}}}}"#,
            id.split('/').next().unwrap()
        )
    }

    fn store(goldens: &[String]) -> Store {
        Store::parse(&format!(
            r#"{{"format": 1, "about": "t", "editions": [{{"edition": "pre-edition",
            "state": "open", "goldens": [{}, {{"id": "geom-z/f", "what": "w", "sha256": "{A}",
            "file": "a.txt"}}]}}]}}"#,
            goldens.join(",")
        ))
        .unwrap()
    }

    #[test]
    fn goldens_group_by_crate_and_target_in_store_order() {
        let s = store(&[
            golden("geom-a/one", "det", "f1"),
            golden("geom-b/two", "lib", "m::tests::f2"),
            golden("geom-a/three", "det", "f3"),
            golden("geom-a/four", "det", "f1"),
        ]);
        let p = plan(&s);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].functions, ["f1", "f3"]);
        assert_eq!(p[0].ids, ["geom-a/one", "geom-a/three", "geom-a/four"]);
        assert_eq!(
            p[0].cargo_args(&["--release".to_owned()]),
            [
                "test",
                "--locked",
                "-p",
                "geom-a",
                "--test",
                "det",
                "--release",
                "--",
                "--exact",
                "--nocapture",
                "f1",
                "f3"
            ]
        );
        assert_eq!(
            p[1].cargo_args(&[]),
            [
                "test",
                "--locked",
                "-p",
                "geom-b",
                "--lib",
                "--",
                "--exact",
                "--nocapture",
                "m::tests::f2"
            ]
        );
    }

    fn group() -> Group {
        plan(&store(&[golden("geom-a/one", "det", "f1")])).remove(0)
    }

    /// The real store's plan, shared out among 1 to 6 parts: every group lands in exactly one part,
    /// whole crates stay together, and a part outside 1 to `n` is refused. Planted: a golden of a
    /// crate no table lists still lands in exactly one part.
    #[test]
    fn every_group_runs_in_exactly_one_part() {
        let real = plan(&Store::embedded().unwrap());
        let mut planted = real.clone();
        planted.push(Group {
            krate: "geom-new".to_owned(),
            test: "det".to_owned(),
            functions: vec!["f".to_owned()],
            ids: vec!["geom-new/det".to_owned()],
        });
        for groups in [&real, &planted] {
            for (n, release) in (1..=6).flat_map(|n| [(n, true), (n, false)]) {
                let parts: Vec<Vec<Group>> = (1..=n)
                    .map(|k| part(groups, k, n, release).unwrap())
                    .collect();
                for g in groups {
                    let homes = parts.iter().filter(|p| p.contains(g)).count();
                    assert_eq!(homes, 1, "{}::{} in {homes} of {n} parts", g.krate, g.test);
                    let of_crate: Vec<usize> = (0..n)
                        .filter(|i| parts[*i].iter().any(|h| h.krate == g.krate))
                        .collect();
                    assert_eq!(
                        of_crate.len(),
                        1,
                        "{} split over parts {of_crate:?}",
                        g.krate
                    );
                }
                assert_eq!(parts.iter().fold(0, |all, p| all + p.len()), groups.len());
            }
            assert_eq!(part(groups, 0, 2, true), None);
            assert_eq!(part(groups, 3, 2, false), None);
        }
        // The two slowest crates go to different parts of two, by each mode's own table.
        let one = part(&real, 1, 2, true).unwrap();
        assert!(one.iter().any(|g| g.krate == "geom-run"));
        assert!(!one.iter().any(|g| g.krate == "geom-num"));
        let one = part(&real, 1, 2, false).unwrap();
        assert!(one.iter().any(|g| g.krate == "geom-test"));
        assert!(!one.iter().any(|g| g.krate == "geom-num"));
        // In release, six parts give each of the six long crates a part of its own.
        let long = [
            "geom-run",
            "geom-num",
            "geom-format",
            "geom-linalg",
            "geom-poly",
            "geom-topo",
        ];
        for k in 1..=6 {
            let p = part(&real, k, 6, true).unwrap();
            let held = long
                .iter()
                .filter(|c| p.iter().any(|g| g.krate == **c))
                .count();
            assert_eq!(held, 1, "release part {k} of 6 holds {held} long crates");
        }
    }

    /// The build before the runs names every group's crate and test once, unit tests apart, with
    /// the leg's arguments; a plan without unit-test goldens builds no `--lib`.
    #[test]
    fn one_build_names_every_group() {
        let s = store(&[
            golden("geom-a/one", "det", "f1"),
            golden("geom-b/two", "lib", "m::tests::f2"),
            golden("geom-c/three", "det", "f3"),
            golden("geom-c/four", "other", "f4"),
        ]);
        let args = build_args(&plan(&s), &["--release".to_owned()]);
        let words = |v: &[&str]| v.iter().map(|w| (*w).to_owned()).collect::<Vec<String>>();
        assert_eq!(
            args,
            [
                words(&[
                    "test",
                    "--locked",
                    "--no-run",
                    "-p",
                    "geom-a",
                    "-p",
                    "geom-c",
                    "--test",
                    "det",
                    "--test",
                    "other",
                    "--release"
                ]),
                words(&[
                    "test",
                    "--locked",
                    "--no-run",
                    "-p",
                    "geom-b",
                    "--lib",
                    "--release"
                ]),
            ]
        );
        let only = build_args(&plan(&store(&[golden("geom-a/one", "det", "f1")])), &[]);
        assert_eq!(only.len(), 1);
        assert!(!only[0].contains(&"--lib".to_owned()));
        assert_eq!(build_args(&[], &[]), Vec::<Vec<String>>::new());
    }

    #[test]
    fn a_run_that_ran_and_agreed_verifies() {
        let out = format!(
            "running 1 test\ngolden geom-a/one (edition pre-edition): {A} agrees with the \
             store; 2 runs agree\ntest f1 ... ok\n\ntest result: ok. 1 passed; 0 failed; \
             0 ignored; 0 measured; 9 filtered out; finished in 0.1s\n"
        );
        assert_eq!(verify(&group(), &out), Ok(()));
    }

    /// On one test thread (wasm32-wasip1, or `RUST_TEST_THREADS=1`) libtest prints `test <name>
    /// ... ` before the test runs, so the agreeing line follows it on the same output line; that
    /// run verifies too, and the same form with a differing digest does not.
    #[test]
    fn a_run_on_one_thread_verifies_with_its_line_after_the_test_name() {
        let inline = format!(
            "running 1 test\ntest f1 ... golden geom-a/one (edition pre-edition): {A} agrees with \
             the store; 2 runs agree on perturbed heaps\nsection digests\nok\n\ntest result: ok. \
             1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s\n"
        );
        assert_eq!(verify(&group(), &inline), Ok(()));
        let differs = format!(
            "running 1 test\ntest f1 ... golden geom-a/one (edition pre-edition): {A} differs \
             from the store's {A}\nok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored\n"
        );
        assert_eq!(
            verify(&group(), &differs),
            Err(RunError::Silent("geom-a/one".to_owned()))
        );
    }

    /// Only the exact agreeing line counts: not a short digest, a missing edition, another
    /// golden's id that begins with this one, or a line that only mentions the id.
    #[test]
    fn only_the_exact_agreeing_line_counts() {
        let good = format!("golden geom-a/one (edition pre-edition): {A} agrees with the store");
        assert!(agrees_in(&good, "geom-a/one"));
        assert!(agrees_in(
            &format!("test f1 ... {good}; 2 runs"),
            "geom-a/one"
        ));
        for bad in [
            "golden geom-a/one (edition pre-edition): aa agrees with the store".to_owned(),
            format!("golden geom-a/one (edition ): {A} agrees with the store"),
            format!("golden geom-a/one (edition Pre Edition): {A} agrees with the store"),
            format!("golden geom-a/one-two (edition pre-edition): {A} agrees with the store"),
            format!(
                "golden geom-a/one (edition pre-edition): {} agrees with the store",
                A.to_uppercase()
            ),
            format!("golden geom-a/one (edition pre-edition): {A} is not in the store"),
            "the store agrees with golden geom-a/one".to_owned(),
        ] {
            assert!(!agrees_in(&bad, "geom-a/one"), "{bad}");
        }
    }

    #[test]
    fn every_unproven_run_fails() {
        let line = format!("golden geom-a/one (edition pre-edition): {A} agrees with the store\n");
        let cases = [
            (
                "nothing ran",
                format!(
                    "{line}test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out\n"
                ),
            ),
            ("no summary", line.clone()),
            (
                "a failure",
                format!("{line}test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured\n"),
            ),
            (
                "ignored",
                format!("{line}test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured\n"),
            ),
            (
                "silent",
                "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured\n".to_owned(),
            ),
            (
                "disagreed",
                format!(
                    "golden geom-a/one (edition pre-edition): {A} differs from the store's {A}\n\
                     test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured\n"
                ),
            ),
            (
                "two binaries",
                format!(
                    "{line}test result: ok. 1 passed; 0 failed; 0 ignored\ntest result: ok. 1 passed; 0 failed; 0 ignored\n"
                ),
            ),
        ];
        for (name, out) in cases {
            assert!(verify(&group(), &out).is_err(), "{name} verified");
        }
    }
}
