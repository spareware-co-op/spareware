//! What a later push to a pull request must judge again, after an earlier push's mutation or
//! coverage run finished clean (the owner's testing principle 5: "a later push judges only the
//! lines changed since the last judged head"), so the gates do not repeat on code whose verdict
//! cannot have changed, and never skip code whose verdict can.
//!
//! What it does: [`mutants`] takes the change's files (its base against its head), what changed
//! since the last head whose mutants were judged clean ([`Since`]), and the crate graph, and says
//! which of the change's Rust files the run must mutate again, every changed line of each, with
//! why. [`coverage`] says which changed files the coverage gate must measure the crates of.
//! `kernel-quality mutants-increment` and `coverage-increment` are the command line;
//! `kernel-quality.yml` finds the last clean head through GitHub's record of the pull request's
//! runs.
//!
//! The mutation rule. A mutant of a line in crate X is judged by X's own tests (`.cargo/mutants.toml`),
//! so its verdict can change only when something those tests build or read changes: exactly what
//! `test_scope` selects X's tests on. So, of what changed since the last clean head (on the pull
//! request's side, and on its base's side since that head), every crate whose tests that reaches
//! has its change's lines judged again, whole, with two exceptions that can only kill more mutants
//! or excuse more survivors, never fewer:
//!
//! - a file added under a crate's `tests/`, `benches/` or `examples/` (a new test binary; the old
//!   ones are unchanged) reaches nothing;
//! - the quality ledger reaches the files of the survivors it no longer holds (an entry removed or
//!   changed): an entry added only excuses more. A ledger the base side changed, or one that cannot
//!   be read, reaches every file.
//!
//! A change's line that changed since the last clean head is in a file of its crate, so its crate
//! is reached and the line is judged with it. A crate not reached has the same code, tests and
//! inputs as when its lines were judged clean, so their verdicts stand.
//!
//! The coverage rule is `coverage::affected_crates` on what changed since the last clean coverage
//! run, with the quality ledger left out when only its survivors changed (coverage reads its
//! `coverage_off` and `awaiting_proof` lists, never the survivors).
//!
//! Guarantees:
//!
//! - **Over, never under.** Whatever cannot be placed (a file in no crate, a ledger that cannot be
//!   read) judges every file again; `tests/kernel_quality_workflow.rs` plants a changed line, a
//!   changed test, a removed ledger entry and a base-side change, each of which must be judged.
//! - **No last clean head, everything.** The workflow judges the whole change when it finds no
//!   clean run, or cannot fetch its head.
//!
//! Method: the crate graph's selection of `test_scope` (cargo's development-dependency rule,
//! <https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#development-dependencies>),
//! and the monotonicity of cargo-mutants' verdicts (<https://mutants.rs/mutants.html>): a mutant is
//! caught when some test fails, so more tests over unchanged code catch at least what fewer did.
//!
//! Cost: one selection over the changed files, and one comparison of the ledger's survivors.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::fuzz_scope::TEST_FOLDERS;
use crate::quality_ledger::LEDGER_PATH;
use crate::shared::Changes;
use crate::test_scope::Scope;

/// What changed since the last clean head, all paths from the repository root.
#[derive(Clone, Debug, Default)]
pub struct Since {
    /// The files that differ between that head and this one (the pull request's side).
    pub head: Vec<String>,
    /// Of those, the files this head added.
    pub added: BTreeSet<String>,
    /// The files the base's commits since that head touch (the base's side).
    pub base: Vec<String>,
    /// The quality ledger's text at that head, and at this one.
    pub ledger: (Option<String>, Option<String>),
}

/// The files a run judges again, each with why.
pub type Plan = BTreeMap<String, String>;

/// Whether a path names a Rust file.
fn is_rust(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|e| e == "rs")
}

/// A path as `git diff` writes it.
fn normal(file: &str) -> String {
    let f = file.trim().replace('\\', "/");
    f.strip_prefix("./")
        .map_or_else(|| f.clone(), str::to_owned)
}

/// Whether `file`, added by the head, is a test target of its own: `tests/<name>.rs`,
/// `benches/<name>.rs` or `examples/<name>.rs` directly in a crate's folder, which cargo builds as
/// a binary of its own (a module below it, or a data file, is read by other targets).
fn new_test_file(scope: &Scope, file: &str) -> bool {
    scope.folders().values().any(|folder| {
        file.strip_prefix(folder.as_str()).is_some_and(|inside| {
            TEST_FOLDERS.iter().any(|t| {
                inside
                    .strip_prefix(t)
                    .is_some_and(|name| is_rust(name) && !name.contains('/'))
            })
        })
    })
}

/// The survivors' files of a ledger the other holds no more, or `None` when either cannot be read.
fn dropped_survivors(then: Option<&str>, now: Option<&str>) -> Option<BTreeSet<String>> {
    let survivors = |text: Option<&str>| -> Option<Vec<Value>> {
        let v: Value = serde_json::from_str(text?).ok()?;
        v.get("survivors")?.as_array().cloned()
    };
    let (then, now) = (survivors(then)?, survivors(now)?);
    let mut kept: BTreeMap<String, usize> = BTreeMap::new();
    for s in &now {
        let n = kept.entry(s.to_string()).or_default();
        *n = n.checked_add(1)?;
    }
    let mut out = BTreeSet::new();
    for s in &then {
        match kept.get_mut(&s.to_string()) {
            Some(n) if *n > 0 => *n = n.checked_sub(1)?,
            _ => {
                out.insert(normal(s.get("file")?.as_str()?));
            }
        }
    }
    Some(out)
}

/// Which of the change's Rust files (`changed`: every file of the change, base against head) a
/// later push mutates again, given what changed `since` the last clean head and `shared` what the
/// shared files among those changes reach (`crate::shared::changes` against that head; the ledger
/// is read here, by its survivors).
#[must_use]
pub fn mutants(scope: &Scope, changed: &[String], since: &Since, shared: &Changes) -> Plan {
    let rust: Vec<String> = changed
        .iter()
        .map(|f| normal(f))
        .filter(|f| is_rust(f))
        .collect();
    let every = |why: &str| -> Plan { rust.iter().map(|f| (f.clone(), why.to_owned())).collect() };
    let base: Vec<String> = since.base.iter().map(|f| normal(f)).collect();
    if base.iter().any(|f| f == LEDGER_PATH) {
        return every("the base's quality ledger changed since the last clean head");
    }
    let mut reach: Vec<String> = base;
    let mut plan = Plan::new();
    for f in since.head.iter().map(|f| normal(f)) {
        if f == LEDGER_PATH {
            let (then, now) = (&since.ledger.0, &since.ledger.1);
            let Some(dropped) = dropped_survivors(then.as_deref(), now.as_deref()) else {
                return every("the quality ledger cannot be read on both sides");
            };
            for file in dropped {
                if rust.contains(&file) {
                    plan.insert(
                        file,
                        "the quality ledger no longer holds one of its survivors".to_owned(),
                    );
                }
            }
        } else if !(since.added.contains(&f) && new_test_file(scope, &f)) {
            reach.push(f);
        }
    }
    let selection = scope.select_with(Some(&reach), shared);
    let running: BTreeMap<&str, &str> = selection
        .crates
        .iter()
        .filter_map(|e| Some((e.name.as_str(), e.why.as_deref()?)))
        .collect();
    for f in &rust {
        let owner = scope
            .folders()
            .iter()
            .filter(|(_, folder)| f.starts_with(folder.as_str()))
            .max_by_key(|(_, folder)| folder.len());
        match owner {
            None => {
                plan.insert(f.clone(), format!("{f} is in no crate"));
            }
            Some((krate, _)) => {
                if let Some(why) = running.get(krate.as_str()) {
                    plan.entry(f.clone()).or_insert_with(|| {
                        format!("{krate}'s tests may judge it otherwise now: {why}")
                    });
                }
            }
        }
    }
    plan
}

/// The files whose crates a later push's coverage run measures (for `coverage::affected_crates`):
/// what changed since the last clean coverage run on either side, the quality ledger left out when
/// only its survivors changed.
#[must_use]
pub fn coverage(since: &Since) -> Vec<String> {
    let lists = |text: Option<&str>| -> Option<(Value, Value)> {
        let v: Value = serde_json::from_str(text?).ok()?;
        let lists = [
            v.get("coverage_off").cloned().unwrap_or(Value::Null),
            v.get("awaiting_proof").cloned().unwrap_or(Value::Null),
        ];
        let [a, b] = lists;
        Some((a, b))
    };
    let (then, now) = (&since.ledger.0, &since.ledger.1);
    let same_lists = match (lists(then.as_deref()), lists(now.as_deref())) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    };
    let mut out: BTreeSet<String> = since.base.iter().map(|f| normal(f)).collect();
    for f in since.head.iter().map(|f| normal(f)) {
        if f != LEDGER_PATH || !same_lists {
            out.insert(f);
        }
    }
    if since.base.iter().any(|f| normal(f) == LEDGER_PATH) {
        out.insert(LEDGER_PATH.to_owned());
    }
    out.into_iter().collect()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests may unwrap and index"
)]
mod tests {
    use super::*;

    #[test]
    fn a_survivor_the_ledger_no_longer_holds_is_found() {
        let then = r#"{"survivors": [{"file": "a/x.rs", "m": 1}, {"file": "b/y.rs", "m": 2}]}"#;
        let now = r#"{"survivors": [{"file": "b/y.rs", "m": 2}, {"file": "c/z.rs", "m": 3}]}"#;
        assert_eq!(
            dropped_survivors(Some(then), Some(now)).unwrap(),
            BTreeSet::from(["a/x.rs".to_owned()])
        );
        assert!(dropped_survivors(Some(then), Some("{}")).is_none());
        assert!(dropped_survivors(None, Some(now)).is_none());
    }

    #[test]
    fn coverage_leaves_out_a_ledger_whose_coverage_lists_did_not_change() {
        let since = |then: &str, now: &str| Since {
            head: vec![LEDGER_PATH.to_owned(), "a/src/x.rs".to_owned()],
            ledger: (Some(then.to_owned()), Some(now.to_owned())),
            ..Since::default()
        };
        let a = r#"{"survivors": [1], "coverage_off": [], "awaiting_proof": []}"#;
        let b = r#"{"survivors": [2], "coverage_off": [], "awaiting_proof": []}"#;
        let c = r#"{"survivors": [2], "coverage_off": [{"file": "x"}], "awaiting_proof": []}"#;
        assert_eq!(coverage(&since(a, b)), ["a/src/x.rs"]);
        assert_eq!(coverage(&since(a, c)), ["a/src/x.rs", LEDGER_PATH]);
        assert_eq!(coverage(&since(a, "broken")), ["a/src/x.rs", LEDGER_PATH]);
    }
}
