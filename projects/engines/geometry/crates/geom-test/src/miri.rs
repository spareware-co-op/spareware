//! The weekly Miri run's plan: which kernel test suites run under Miri, one CI job each.
//!
//! What it does: [`plan`] reads `cargo metadata` and returns one [`Job`] per test target (the
//! library's unit tests and each integration test) of every kernel crate that has integration
//! tests, and one more job for the unit tests of every other kernel crate together. Miri
//! interprets a program hundreds of times slower than it runs, so the suites with real work run in
//! parallel jobs, and the crates that are still scaffolds share one. The workflow
//! (`.github/workflows/kernel-quality.yml`) runs each job's `cargo miri test` arguments, adding the
//! filters it keeps for tests too slow to interpret, each with its reason.
//!
//! Guarantees: every test target of every kernel crate is in exactly one job, so a new test file
//! runs under Miri the week it lands, unfiltered until a filter is written for it. The development
//! crates (`geom-test`, `geom-fuzz`, `geom-gen`, `geom-bench`) are left out: they never ship, and
//! they spawn processes and write files, which Miri's isolation refuses. A metadata file without
//! any kernel crate is an error, because a plan that runs nothing must not pass.
//!
//! Method: the `packages[].targets[]` of `cargo metadata --format-version 1`
//! (<https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>); a kernel crate is a package
//! whose manifest is in the kernel's crates folder, as [`crate::deps`] defines it.
//!
//! Cost: linear in the size of the metadata.

use serde_json::Value;

use crate::deps::{DEVELOPMENT_CRATES, KERNEL_CRATES_DIR};

/// One Miri job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    /// Its name, for the job's title and for the workflow's filter table: `<crate> <target>`, with
    /// `lib` for the unit tests, or `other kernel crates`.
    pub name: String,
    /// The arguments to `cargo miri test` that select its tests.
    pub args: Vec<String>,
}

/// The name of the job that runs the unit tests of the kernel crates without integration tests.
pub const OTHERS: &str = "other kernel crates";

/// Whether a manifest path is a kernel crate's (in the kernel's crates folder, not a development
/// crate), and its folder name if so.
fn kernel_crate(manifest: &str) -> Option<String> {
    let parts: Vec<&str> = manifest
        .split(['/', '\\'])
        .filter(|p| !p.is_empty())
        .collect();
    let (file, rest) = parts.split_last()?;
    let (folder, parent) = rest.split_last()?;
    let dir = KERNEL_CRATES_DIR.as_slice();
    let in_kernel = *file == "Cargo.toml" && parent.ends_with(dir);
    (in_kernel && !DEVELOPMENT_CRATES.contains(folder)).then(|| (*folder).to_owned())
}

/// The Miri plan for the workspace `cargo metadata` describes.
///
/// # Errors
///
/// A description of what is missing, if the metadata has no package list, a package without a
/// name, manifest path or targets, or no kernel crate at all.
pub fn plan(metadata: &Value) -> Result<Vec<Job>, String> {
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("cargo metadata has no packages")?;
    let mut jobs = Vec::new();
    let mut others = Vec::new();
    let mut kernel = 0usize;
    for p in packages {
        let manifest = p
            .get("manifest_path")
            .and_then(Value::as_str)
            .ok_or("a package without a manifest path")?;
        let Some(folder) = kernel_crate(manifest) else {
            continue;
        };
        kernel = kernel.saturating_add(1);
        let name = p
            .get("name")
            .and_then(Value::as_str)
            .ok_or("a package without a name")?;
        let targets = p
            .get("targets")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{name} has no targets"))?;
        let kind = |t: &Value, k: &str| {
            t.get("kind")
                .and_then(Value::as_array)
                .is_some_and(|ks| ks.iter().any(|x| x.as_str() == Some(k)))
        };
        let tests: Vec<String> = targets
            .iter()
            .filter(|t| kind(t, "test"))
            .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_owned))
            .collect();
        if tests.is_empty() {
            others.push(name.to_owned());
            continue;
        }
        if targets.iter().any(|t| kind(t, "lib")) {
            jobs.push(Job {
                name: format!("{folder} lib"),
                args: vec!["-p".to_owned(), name.to_owned(), "--lib".to_owned()],
            });
        }
        for t in tests {
            jobs.push(Job {
                name: format!("{folder} {t}"),
                args: vec![
                    "-p".to_owned(),
                    name.to_owned(),
                    "--test".to_owned(),
                    t.clone(),
                ],
            });
        }
    }
    if kernel == 0 {
        return Err(
            "no kernel crate in the metadata: a plan that runs nothing must not pass".into(),
        );
    }
    others.sort();
    if !others.is_empty() {
        jobs.push(Job {
            name: OTHERS.to_owned(),
            args: others
                .iter()
                .flat_map(|o| ["-p".to_owned(), o.clone()])
                .collect(),
        });
    }
    jobs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(jobs)
}

/// The plan as the JSON list a GitHub Actions matrix takes: `[{"name": ..., "args": "..."}]`, with
/// the arguments joined by spaces (no argument has one).
#[must_use]
pub fn matrix(jobs: &[Job]) -> String {
    let list: Vec<Value> = jobs
        .iter()
        .map(|j| serde_json::json!({"name": j.name, "args": j.args.join(" ")}))
        .collect();
    Value::Array(list).to_string()
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

    fn package(name: &str, folder: &str, kinds: &[(&str, &str)]) -> Value {
        let targets: Vec<Value> = kinds
            .iter()
            .map(|(k, n)| serde_json::json!({"kind": [k], "name": n}))
            .collect();
        serde_json::json!({
            "name": name,
            "manifest_path": format!("/r/projects/engines/geometry/crates/{folder}/Cargo.toml"),
            "targets": targets,
        })
    }

    #[test]
    fn every_test_target_has_exactly_one_job() {
        let metadata = serde_json::json!({"packages": [
            package("geom-num", "geom-num", &[("lib", "geom_num"), ("test", "wide"), ("test", "plane")]),
            package("geom-poly", "geom-poly", &[("lib", "geom_poly")]),
            package("geom-2d", "geom-2d", &[("lib", "geom_2d")]),
            package("geom-test", "geom-test", &[("lib", "geom_test"), ("test", "deps")]),
            {"name": "kit-core", "manifest_path": "/r/projects/kit/crates/kit-core/Cargo.toml", "targets": []},
        ]});
        let jobs = plan(&metadata).unwrap();
        let names: Vec<&str> = jobs.iter().map(|j| j.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "geom-num lib",
                "geom-num plane",
                "geom-num wide",
                "other kernel crates"
            ]
        );
        assert_eq!(jobs[3].args, ["-p", "geom-2d", "-p", "geom-poly"]);
        assert_eq!(jobs[2].args, ["-p", "geom-num", "--test", "wide"]);
        assert!(matrix(&jobs).contains("\"args\":\"-p geom-num --lib\""));
        assert!(plan(&serde_json::json!({"packages": []})).is_err());
        assert!(plan(&serde_json::json!({})).is_err());
    }
}
