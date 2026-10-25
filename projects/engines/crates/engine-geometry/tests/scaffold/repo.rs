//! Reading the repository into what `check::workspace` compares with the table.
//!
//! What it does: runs `cargo metadata --format-version 1 --no-deps` through the `cargo_metadata`
//! crate with the cargo that runs this test (the `CARGO` environment variable), and for every
//! workspace crate records its folder, its
//! library's header and attributes, whether its manifest forbids unsafe code, its lint settings
//! (its manifest's `lints` keys and the clippy configuration clippy reads for it), its dependencies
//! and their sources, and its features. A library counts as `no_std` only if no Rust file in its
//! source folder declares `extern crate std`. It also lists the folders under
//! `projects/engines/geometry/crates`, so a folder cargo does not see is still reported, and reads
//! the toolchain file and the workflows for the toolchain check. For the merge gate (G12) it reads
//! every Rust source file of the crates that ship (`crate_sources`), for the ban on global state
//! and the rule on lint attributes, and finds every cargo configuration file on the way to a crate
//! of the family (`cargo_config_files`).
//!
//! What it guarantees: every path is taken relative to the workspace root that cargo reports, so
//! short and long Windows path forms never disagree; folders are walked in sorted order without
//! recursion; anything it cannot read stops the test with the reason. It changes nothing.
//!
//! Method: cargo's documented metadata format
//! (<https://doc.rust-lang.org/cargo/commands/cargo-metadata.html#json-format>), read into the
//! typed model of the `cargo_metadata` crate (<https://docs.rs/cargo_metadata/0.23.1/>), which
//! piece G12a put in place of a JSON reader written here (chapter 29.1); manifests and clippy
//! configurations by the `toml` crate, through `manifest`.
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

use cargo_metadata::{DependencyKind, MetadataCommand, TargetKind};

use crate::check::{Dep, DepKind, GEOMETRY_CRATES, Observed, Package};
use crate::globals::{NOT_CRATE_CODE, SourceFile, Sources};
use crate::header::Header;
use crate::lints::{CLIPPY_FILES, ClippyConfig, Lints};
use crate::manifest::{keys, lint_settings, manifest_forbids_unsafe};
use crate::posture::CARGO_CONFIG_FILES;
use crate::source::{Attributes, attributes, names_extern_crate_std};

/// The cargo that runs this test: the `CARGO` variable cargo sets for the test process, or, if a
/// runner dropped it, the one cargo set when it compiled the test.
pub fn cargo() -> OsString {
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
fn library(
    package: &cargo_metadata::Package,
    dir_path: &Path,
) -> (Option<Vec<String>>, Header, Attributes) {
    let lib_target = package.targets.iter().find(|target| {
        target.kind.iter().any(|k| {
            matches!(
                k,
                TargetKind::Lib
                    | TargetKind::RLib
                    | TargetKind::DyLib
                    | TargetKind::CDyLib
                    | TargetKind::StaticLib
                    | TargetKind::ProcMacro
            )
        })
    });
    let Some(target) = lib_target else {
        return (None, Header::default(), Attributes::default());
    };
    let src = target.src_path.clone().into_std_path_buf();
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
fn dependencies(package: &cargo_metadata::Package) -> Vec<Dep> {
    package
        .dependencies
        .iter()
        .map(|dep| Dep {
            name: dep.name.clone(),
            kind: match dep.kind {
                DependencyKind::Normal => DepKind::Normal,
                DependencyKind::Build => DepKind::Build,
                DependencyKind::Development => DepKind::Dev,
                other => panic!("cargo metadata: unknown dependency kind {other:?}"),
            },
            path: dep.path.is_some(),
            optional: dep.optional,
            source: dep.source.as_ref().map(|s| s.repr.clone()),
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
        settings: keys(&read(&file)),
    })
}

/// Reads the workspace.
pub fn observe() -> Observed {
    let metadata = MetadataCommand::new()
        .cargo_path(cargo())
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .no_deps()
        .exec()
        .unwrap_or_else(|e| panic!("cargo metadata failed: {e}"));
    let root = metadata.workspace_root.clone().into_std_path_buf();
    let workspace_manifest = read(&root.join("Cargo.toml"));

    let mut packages = BTreeMap::new();
    for package in &metadata.packages {
        let name = package.name.to_string();
        let manifest_path = package.manifest_path.clone().into_std_path_buf();
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
            features: package.features.clone(),
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

/// Every Rust source file of each crate in `crates` that `observed` holds, outside the folders
/// directly in its folder that `globals::NOT_CRATE_CODE` names, read for global state, in sorted
/// order (the merge gate's ban on global state, G12).
pub fn crate_sources(observed: &Observed, crates: &BTreeSet<&str>) -> Sources {
    let root = workspace_root();
    let mut sources = Sources::new();
    for (name, package) in observed
        .packages
        .iter()
        .filter(|(name, _)| crates.contains(name.as_str()))
    {
        let dir = package
            .dir
            .iter()
            .fold(root.clone(), |p, part| p.join(part));
        let mut files = Vec::new();
        for (path, is_dir) in entries(&dir) {
            let skipped = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| NOT_CRATE_CODE.contains(&n));
            if is_dir && !skipped {
                files.extend(rust_files(&path));
            } else if !is_dir && path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
        files.sort();
        let of_crate: Vec<SourceFile> = files
            .iter()
            .map(|file| SourceFile::read(relative(file, &root), &read(file)))
            .collect();
        sources.insert(name.clone(), of_crate);
    }
    sources
}

/// The files the compiler read for each crate (absolute paths, as `gate::compiled_sources` lists
/// them), read for the source rules: each path relative to the workspace root when it is under it,
/// else whole. A file that is not UTF-8 text (data a crate pastes in with `include_bytes!`) cannot
/// be read as Rust, so it counts as not lexing, which the rules report.
pub fn compiled_sources(compiled: &BTreeMap<String, BTreeSet<PathBuf>>) -> Sources {
    let canonical = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let root = canonical(&workspace_root());
    let mut sources = Sources::new();
    for (krate, files) in compiled {
        let of_crate = files
            .iter()
            .map(|file| {
                let full = canonical(file);
                let path: Vec<String> = match full.strip_prefix(&root) {
                    Ok(rest) => rest
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect(),
                    Err(_) => vec![file.display().to_string()],
                };
                match fs::read_to_string(&full) {
                    Ok(text) => SourceFile::read(path, &text),
                    Err(_) => SourceFile {
                        path,
                        uses: None,
                        lints: None,
                    },
                }
            })
            .collect();
        sources.insert(krate.clone(), of_crate);
    }
    sources
}

/// Every cargo configuration file, `.cargo/config.toml` or the older `.cargo/config`, in `root` or
/// in a folder on the way from it to one of `dirs` (each relative to `root`, one component per
/// entry), that folder included: the files cargo reads when it runs there
/// (<https://doc.rust-lang.org/cargo/reference/config.html#hierarchical-structure>). Each is
/// returned relative to `root`.
pub fn cargo_config_files(root: &Path, dirs: &[Vec<String>]) -> BTreeSet<Vec<String>> {
    let mut found = BTreeSet::new();
    for dir in dirs {
        for depth in 0..=dir.len() {
            for name in CARGO_CONFIG_FILES {
                let mut file: Vec<String> = dir[..depth].to_vec();
                file.push(".cargo".to_owned());
                file.push(name.to_owned());
                let path = file.iter().fold(root.to_path_buf(), |p, part| p.join(part));
                if path.is_file() {
                    found.insert(file);
                }
            }
        }
    }
    found
}

/// The workspace root, as cargo finds it from this crate.
pub fn workspace_root() -> PathBuf {
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
