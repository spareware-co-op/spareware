//! Reading the repository into what `check::workspace` compares with the table.
//!
//! What it does: runs `cargo metadata --format-version 1 --no-deps` with the cargo that runs this
//! test (the `CARGO` environment variable), and for every workspace crate records its folder, its
//! library's header and attributes, whether its manifest forbids unsafe code, its lint settings
//! (its manifest's `lints` keys and the clippy configuration clippy reads for it), its dependencies
//! and their sources, and its features. A library counts as `no_std` only if no Rust file in its
//! source folder declares `extern crate std`. It also lists the folders under
//! `projects/engines/geometry/crates`, so a folder cargo does not see is still reported, and reads
//! the toolchain file and the workflows for the toolchain check.
//!
//! What it guarantees: every path is taken relative to the workspace root that cargo reports, so
//! short and long Windows path forms never disagree; folders are walked in sorted order without
//! recursion; anything it cannot read stops the test with the reason. It changes nothing.
//!
//! Method: cargo's documented metadata format
//! (<https://doc.rust-lang.org/cargo/commands/cargo-metadata.html#json-format>), read by the
//! strict parser in `json`.
//!
//! Cost: one `cargo metadata` run without dependency resolution, one read of each crate's
//! `Cargo.toml`, of its clippy configuration and of each Rust file in its library's source folder.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::check::{Dep, DepKind, GEOMETRY_CRATES, Observed, Package};
use crate::header::Header;
use crate::json::{self, Json};
use crate::lints::{CLIPPY_FILES, ClippyConfig, Lints};
use crate::manifest::{lint_settings, manifest_forbids_unsafe, settings};
use crate::source::{Attributes, attributes, names_extern_crate_std};

/// The cargo that runs this test: the `CARGO` variable cargo sets for the test process, or, if a
/// runner dropped it, the one cargo set when it compiled the test.
fn cargo() -> OsString {
    std::env::var_os("CARGO")
        .or_else(|| option_env!("CARGO").map(OsString::from))
        .expect("CARGO is not set: run this test through cargo")
}

/// The components of `path` below `base`, as text.
fn relative(path: &Path, base: &Path) -> Vec<String> {
    let rest = path
        .strip_prefix(base)
        .unwrap_or_else(|_| panic!("{} is not under {}", path.display(), base.display()));
    rest.components()
        .map(|c| match c {
            Component::Normal(part) => part
                .to_str()
                .unwrap_or_else(|| panic!("{} is not UTF-8", path.display()))
                .to_owned(),
            other => panic!("unexpected path component {other:?} in {}", path.display()),
        })
        .collect()
}

fn text(value: &Json, key: &str) -> String {
    value
        .get(key)
        .and_then(Json::as_str)
        .unwrap_or_else(|| panic!("cargo metadata: missing text field {key}"))
        .to_owned()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The entries of one folder, sorted, as (path, is a folder).
fn entries(dir: &Path) -> Vec<(PathBuf, bool)> {
    let mut found: Vec<(PathBuf, bool)> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()))
        .map(|entry| {
            let entry = entry.expect("cannot read a folder entry");
            let is_dir = entry.file_type().expect("cannot read a file type").is_dir();
            (entry.path(), is_dir)
        })
        .collect();
    found.sort();
    found
}

/// Every `.rs` file under `dir`, in sorted order, walked with an explicit stack.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(folder) = stack.pop() {
        for (path, is_dir) in entries(&folder) {
            if is_dir {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// A library's source as `relative` gives it, its header and its attributes; for a package without
/// a library, none of them. A library counts as `no_std` only if no Rust file in its source folder
/// declares `extern crate std`.
fn library(package: &Json, dir_path: &Path) -> (Option<Vec<String>>, Header, Attributes) {
    let lib_target = package
        .get("targets")
        .and_then(Json::as_array)
        .expect("cargo metadata: no targets")
        .iter()
        .find(|target| {
            target
                .get("kind")
                .and_then(Json::as_array)
                .is_some_and(|kinds| {
                    kinds.iter().any(|k| {
                        matches!(
                            k.as_str(),
                            Some("lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro")
                        )
                    })
                })
        });
    let Some(target) = lib_target else {
        return (None, Header::default(), Attributes::default());
    };
    let src = PathBuf::from(text(target, "src_path"));
    let source = read(&src);
    let mut attrs = attributes(&source);
    // `extern crate std` in any module links std into the whole crate.
    let src_dir = src.parent().expect("a library source has a folder");
    if rust_files(src_dir)
        .iter()
        .any(|file| names_extern_crate_std(&read(file)))
    {
        attrs.no_std = false;
    }
    (
        Some(relative(&src, dir_path)),
        Header::parse(&source),
        attrs,
    )
}

/// A package's dependencies of every kind, as `cargo metadata` lists them.
fn dependencies(package: &Json) -> Vec<Dep> {
    package
        .get("dependencies")
        .and_then(Json::as_array)
        .expect("cargo metadata: no dependencies list")
        .iter()
        .map(|dep| Dep {
            name: text(dep, "name"),
            kind: match dep.get("kind").and_then(Json::as_str) {
                None => DepKind::Normal,
                Some("build") => DepKind::Build,
                Some("dev") => DepKind::Dev,
                Some(other) => panic!("cargo metadata: unknown dependency kind {other}"),
            },
            path: dep.get("path").and_then(Json::as_str).is_some(),
            optional: dep.get("optional").and_then(Json::as_bool).unwrap_or(false),
            source: dep.get("source").and_then(Json::as_str).map(str::to_owned),
        })
        .collect()
}

/// A package's features and what each switches on, as `cargo metadata` lists them.
fn features(package: &Json) -> BTreeMap<String, Vec<String>> {
    package
        .get("features")
        .and_then(Json::as_object)
        .expect("cargo metadata: no features map")
        .iter()
        .map(|(feature, enables)| {
            let enables = enables
                .as_array()
                .expect("cargo metadata: a feature is not a list")
                .iter()
                .map(|f| {
                    f.as_str()
                        .expect("cargo metadata: a feature entry is not text")
                        .to_owned()
                })
                .collect();
            (feature.clone(), enables)
        })
        .collect()
}

/// The clippy configuration clippy reads for the crate in `dir`: the first of `CLIPPY_FILES`, in
/// that order, in `dir` or the nearest folder above it, up to the workspace root `root`.
pub fn clippy_config(dir: &Path, root: &Path) -> Option<ClippyConfig> {
    let file = dir
        .ancestors()
        .take_while(|folder| folder.starts_with(root))
        .flat_map(|folder| CLIPPY_FILES.map(|name| folder.join(name)))
        .find(|path| path.is_file())?;
    Some(ClippyConfig {
        file: relative(&file, root),
        settings: settings(&read(&file)),
    })
}

/// Reads the workspace.
pub fn observe() -> Observed {
    let output = Command::new(cargo())
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cannot run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("cargo metadata wrote non-UTF-8");
    let metadata =
        json::parse(&stdout).unwrap_or_else(|e| panic!("cargo metadata wrote invalid JSON: {e:?}"));
    let root = PathBuf::from(text(&metadata, "workspace_root"));
    let workspace_manifest = read(&root.join("Cargo.toml"));

    let mut packages = BTreeMap::new();
    let listed = metadata
        .get("packages")
        .and_then(Json::as_array)
        .expect("cargo metadata: no packages");
    for package in listed {
        let name = text(package, "name");
        let manifest_path = PathBuf::from(text(package, "manifest_path"));
        let dir_path = manifest_path
            .parent()
            .expect("a manifest has a folder")
            .to_path_buf();
        let manifest = read(&manifest_path);
        let (lib, header, attributes) = library(package, &dir_path);
        let observed = Package {
            dir: relative(&dir_path, &root),
            lib,
            header,
            attributes,
            manifest_forbids_unsafe: manifest_forbids_unsafe(&manifest, &workspace_manifest),
            lints: Lints {
                tables: lint_settings(&manifest),
                clippy: clippy_config(&dir_path, &root),
            },
            deps: dependencies(package),
            features: features(package),
        };
        assert!(
            packages.insert(name.clone(), observed).is_none(),
            "cargo metadata lists {name} twice"
        );
    }

    let crates_dir = GEOMETRY_CRATES
        .iter()
        .fold(root.clone(), |p, part| p.join(part));
    let mut geometry_dirs = BTreeSet::new();
    for (path, is_dir) in entries(&crates_dir) {
        if is_dir {
            let name = path.file_name().expect("a folder has a name");
            let name = name.to_str().expect("a crate folder name is not UTF-8");
            geometry_dirs.insert(name.to_owned());
        }
    }

    Observed {
        packages,
        geometry_dirs,
    }
}

/// The workspace root, as cargo finds it from this crate.
fn workspace_root() -> PathBuf {
    let output = Command::new(cargo())
        .args(["locate-project", "--workspace", "--message-format", "plain"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cannot run cargo locate-project");
    assert!(
        output.status.success(),
        "cargo locate-project failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest = String::from_utf8(output.stdout).expect("cargo wrote non-UTF-8");
    Path::new(manifest.trim())
        .parent()
        .expect("the workspace manifest has a folder")
        .to_path_buf()
}

/// The text of the toolchain file named `toolchain_file` at the workspace root, and every
/// workflow's file name and text, in sorted order.
pub fn toolchain_files(toolchain_file: &str) -> (String, Vec<(String, String)>) {
    let root = workspace_root();
    let toolchain = read(&root.join(toolchain_file));
    let workflows = entries(&root.join(".github").join("workflows"))
        .into_iter()
        .filter(|(path, is_dir)| {
            !is_dir && path.extension().is_some_and(|e| e == "yml" || e == "yaml")
        })
        .map(|(path, _)| {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .expect("a workflow name is UTF-8")
                .to_owned();
            (name, read(&path))
        })
        .collect();
    (toolchain, workflows)
}
