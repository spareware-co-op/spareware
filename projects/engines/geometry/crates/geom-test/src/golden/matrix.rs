//! The determinism matrix's runner: every golden of the current edition, run by cargo in whatever
//! configuration a CI leg sets, with proof that each one really ran and agreed.
//!
//! What it does: [`plan`] groups the current edition's test-checked goldens by crate and test
//! target; [`Group::cargo_args`] gives the `cargo test` arguments that run exactly those test
//! functions (libtest's `--exact` filters) with the leg's own arguments (`--release`, `--target
//! wasm32-wasip1`, ...) and its environment (`RUSTFLAGS` with `-C target-cpu=native` or overflow
//! checks off, `CARGO_TARGET_DIR`), which cargo inherits; [`verify`] reads a run's output and
//! requires that libtest ran exactly the planned functions, all passed, and that every planned
//! golden printed its `golden <id> ... agrees with the store` line ([`super::check`]). The binary
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
//!   was ignored, and every planned id printed its agreeing line.
//!
//! Method: libtest's documented output (`test result: ok. N passed; M failed; I ignored; ...`) and
//! command-line filters (<https://doc.rust-lang.org/rustc/tests/index.html>), the golden matrix of
//! chapters 21 and 28.13 (`projects/engines/geometry/architecture/21-determinism.md`).
//!
//! Cost: linear in the store and in the output read.

use core::fmt;

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
                "libtest's summary is \"test result: {summary}\", not ok: the golden tests did                  not all pass"
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
        let prefix = format!("golden {id} (edition ");
        let agreed = output
            .lines()
            .any(|l| l.starts_with(&prefix) && l.contains("agrees with the store"));
        if !agreed {
            return Err(RunError::Silent(id.clone()));
        }
    }
    Ok(())
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

    #[test]
    fn a_run_that_ran_and_agreed_verifies() {
        let out = "running 1 test\ngolden geom-a/one (edition pre-edition): aa agrees with the \
                   store; 2 runs agree\ntest f1 ... ok\n\ntest result: ok. 1 passed; 0 failed; \
                   0 ignored; 0 measured; 9 filtered out; finished in 0.1s\n";
        assert_eq!(verify(&group(), out), Ok(()));
    }

    #[test]
    fn every_unproven_run_fails() {
        let line = "golden geom-a/one (edition pre-edition): aa agrees with the store\n";
        let cases = [
            (
                "nothing ran",
                format!(
                    "{line}test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out\n"
                ),
            ),
            ("no summary", line.to_owned()),
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
                "golden geom-a/one (edition pre-edition): aa differs from the store's bb\n\
                 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured\n"
                    .to_owned(),
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
