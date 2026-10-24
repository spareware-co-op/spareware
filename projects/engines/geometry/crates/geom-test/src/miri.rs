//! The weekly Miri run's scope: every kernel crate, whose test suites Miri interprets.
//!
//! What it does: [`packages`] reads `cargo metadata` and returns the package of every kernel crate,
//! in name order, and [`cargo_args`] writes them as the `-p` arguments the workflow
//! (`.github/workflows/kernel-quality.yml`) gives `cargo miri nextest run` and `cargo miri test
//! --doc`. nextest runs each test in its own Miri process, splits the tests into partitions, one CI
//! job each, and stops a test that reaches its budget (the `default-miri` profile in
//! `.config/nextest.toml`): such a test passed every step Miri interpreted, and the job's report
//! names it.
//!
//! Guarantees:
//!
//! - **Every kernel test suite, with nothing to keep in step.** The scope is every kernel crate in
//!   the metadata, so a new crate or test file runs under Miri the week it lands, and no list of
//!   test names or files is kept anywhere. A test that cannot run under Miri at all (one that
//!   starts a process or reads the repository) says so where it is written, with
//!   `#[cfg_attr(miri, ignore = "<why>")]`.
//! - **The development crates are left out** (`geom-test`, `geom-fuzz`, `geom-gen`,
//!   `geom-bench`): they never ship, and their tests start processes and read files, which Miri's
//!   isolation refuses.
//! - **A plan that runs nothing does not pass.** Metadata without any kernel crate, or with a
//!   package that has no name or manifest path, is an error.
//!
//! Method: the `packages[]` of `cargo metadata --format-version 1`
//! (<https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>); a kernel crate is a package
//! whose manifest is in the kernel's crates folder, as [`crate::deps`] defines it. nextest's Miri
//! support and partitions are documented at <https://nexte.st/docs/integrations/miri/> and
//! <https://nexte.st/docs/ci-features/partitioning/>.
//!
//! Cost: linear in the size of the metadata.

use serde_json::Value;

use crate::deps::{DEVELOPMENT_CRATES, KERNEL_CRATES_DIR};

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
}
