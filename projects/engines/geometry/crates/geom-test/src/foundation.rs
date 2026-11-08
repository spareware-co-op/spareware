//! The foundation set of section 28.8, the kernel crates the quality gates cover, and their bars.
//!
//! What it does: [`place`] says, for a path relative to the repository root, which kernel crate it
//! belongs to and whether it is in the foundation set, a development crate, or outside the kernel.
//! [`kernel_crates`] lists the kernel crates in the kernel's folder. The mutation gate
//! ([`crate::mutants`]) and the coverage gate ([`crate::coverage`]) both classify with these, so the
//! two gates can never disagree about what the foundation set is.
//!
//! The rules (28.8, "Mutation testing, coverage and the merge gate"):
//!
//! - **The foundation set** is `geom-num`, `geom-poly`, `geom-check`, `geom-name`, the builder in
//!   `geom-topo` and the arrangement module of `geom-2d`: the code where "exact" is promised and
//!   everything else rests. There, no mutant may survive unless it is marked equivalent with a
//!   written reason, judged by tests through the crate's public interface, and the tests must run
//!   [`FOUNDATION_LINES`] percent of lines and [`FOUNDATION_BRANCHES`] percent of branches.
//! - **Elsewhere** in the kernel, no new unexplained survivor may merge, and every crate needs at
//!   least [`ELSEWHERE_LINES`] percent of lines.
//! - **The development crates** (`geom-test`, `geom-fuzz`, `geom-gen`, `geom-bench`) never ship
//!   (chapter 27), so neither gate covers them; [`crate::deps`] keeps them out of every kernel
//!   build.
//!
//! A foundation module of a crate that is not wholly in the set is `src/<module>.rs` together with
//! everything under `src/<module>/`. A crate can gain other code before its foundation module is
//! built (`geom-2d`'s COGO before its arrangement), so each entry of [`FOUNDATION_MODULES`] also
//! classifies the crate's modules that the chapters put outside the foundation module. The coverage
//! gate refuses a crate that has code but no file of its foundation module
//! ([`missing_foundation_module`]) unless every module it has is classified outside and its root
//! file defines no function: if the module is built under another path, or a new module arrives
//! that no list classifies, the gate fails until [`FOUNDATION_MODULES`] names it, rather than
//! silently treating it as ordinary code.
//!
//! Guarantees:
//!
//! - **One classification for both gates.** The mutation gate and the coverage gate place every
//!   path with [`place`], so they cannot disagree about which code is in the foundation set.
//! - **Nothing leaves the foundation bar silently.** A crate of [`FOUNDATION_MODULES`] with code
//!   but no file of its module is reported ([`missing_foundation_module`]), unless every module it
//!   has is classified outside the foundation module by name; and the bars are the integers of
//!   28.8, compared exactly by the gates.
//! - **Development crates are never judged as kernel code**, and a path outside the kernel's
//!   crates folder is [`Place::Outside`].
//!
//! Method: chapter 27's rule "one folder per crate, folder name = crate name"
//! (`projects/engines/geometry/architecture/27-crates-folders.md`) makes a crate's name its
//! folder's name, so a path classifies by its components alone; the set and the bars are 28.8's
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: linear in the length of the path; [`kernel_crates`] reads one directory.

use std::path::Path;

use crate::deps::{DEVELOPMENT_CRATES, KERNEL_CRATES_DIR};

/// The crates wholly in the foundation set.
pub const FOUNDATION_CRATES: [&str; 4] = ["geom-num", "geom-poly", "geom-check", "geom-name"];

/// A crate only part of which is in the foundation set: its foundation module, and the modules it
/// may hold before that module exists, each classified outside the foundation set by the chapter
/// that gives it its job.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FoundationModule {
    /// The crate (its folder's name).
    pub krate: &'static str,
    /// The foundation module: `src/<module>.rs` and everything under `src/<module>/`.
    pub module: &'static str,
    /// Modules under `src/`, by the same naming, that hold work outside the foundation module.
    pub outside: &'static [&'static str],
}

/// The foundation modules of crates not wholly in the set (28.8: "the builder in `geom-topo` and
/// the arrangement module of `geom-2d`"), each with the modules classified outside it.
///
/// - `geom-topo`: none yet.
/// - `geom-2d`: the arrangement is built as `arrange` (piece G196 and those after it). Chapter 27's crate
///   table gives the crate survey linework and COGO as jobs of their own beside the 2D arrangement,
///   so `cogo` (G359, angle units, bearings and core COGO) and `survey` (G361, survey points and
///   3D linework) are outside it.
pub const FOUNDATION_MODULES: [FoundationModule; 2] = [
    FoundationModule {
        krate: "geom-topo",
        module: "builder",
        outside: &[],
    },
    FoundationModule {
        krate: "geom-2d",
        module: "arrange",
        outside: &["cogo", "survey"],
    },
];

/// Percent of lines the foundation set's own tests must run.
pub const FOUNDATION_LINES: u64 = 100;

/// Percent of branches the foundation set's own tests must take both ways.
pub const FOUNDATION_BRANCHES: u64 = 100;

/// Percent of lines every other kernel crate's own tests must run.
pub const ELSEWHERE_LINES: u64 = 90;

/// Which bar a piece of kernel code is held to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Set {
    /// The foundation set of 28.8.
    Foundation,
    /// Every other kernel crate, and the parts of `geom-topo` and `geom-2d` outside their
    /// foundation modules.
    Elsewhere,
}

impl Set {
    /// `foundation` or `elsewhere`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Set::Foundation => "foundation",
            Set::Elsewhere => "elsewhere",
        }
    }
}

/// Where a repository path belongs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// A file under a kernel crate's `src/`.
    Kernel {
        /// The crate (its folder's name).
        krate: String,
        /// The bar it is held to.
        set: Set,
    },
    /// Any other file of a kernel crate: its tests, manifest or configuration.
    KernelOther {
        /// The crate (its folder's name).
        krate: String,
    },
    /// A file of a development crate.
    Development {
        /// The crate (its folder's name).
        krate: String,
    },
    /// Anything outside the kernel's crates folder.
    Outside,
}

/// The components of a path written with `/` or `\` separators, empty ones dropped.
fn components(path: &str) -> Vec<&str> {
    path.split(['/', '\\']).filter(|c| !c.is_empty()).collect()
}

/// Where `path`, relative to the repository root, belongs.
#[must_use]
pub fn place(path: &str) -> Place {
    let parts = components(path);
    let Some(rest) = parts.strip_prefix(KERNEL_CRATES_DIR.as_slice()) else {
        return Place::Outside;
    };
    let Some((krate, inside)) = rest.split_first() else {
        return Place::Outside;
    };
    let krate = (*krate).to_owned();
    if DEVELOPMENT_CRATES.contains(&krate.as_str()) {
        return Place::Development { krate };
    }
    let Some(src) = inside.strip_prefix(["src"].as_slice()) else {
        return Place::KernelOther { krate };
    };
    if src.is_empty() {
        return Place::KernelOther { krate };
    }
    let set = if FOUNDATION_CRATES.contains(&krate.as_str()) || in_foundation_module(&krate, src) {
        Set::Foundation
    } else {
        Set::Elsewhere
    };
    Place::Kernel { krate, set }
}

/// The module of `src` (the components after `src/`): `src/<module>.rs` or the first folder of
/// `src/<module>/...`. `None` for an empty path.
fn module_of<'a>(src: &[&'a str]) -> Option<&'a str> {
    match src {
        [file] => file.strip_suffix(".rs"),
        [dir, _, ..] => Some(dir),
        [] => None,
    }
}

/// Whether `src` (the components after `src/`) is in `krate`'s foundation module.
fn in_foundation_module(krate: &str, src: &[&str]) -> bool {
    foundation_entry(krate).is_some_and(|f| module_of(src) == Some(f.module))
}

/// The entry of [`FOUNDATION_MODULES`] for `krate`, if it has one.
fn foundation_entry(krate: &str) -> Option<&'static FoundationModule> {
    FOUNDATION_MODULES.iter().find(|f| f.krate == krate)
}

/// The foundation module `krate` must have, if any.
#[must_use]
pub fn foundation_module(krate: &str) -> Option<&'static str> {
    foundation_entry(krate).map(|f| f.module)
}

/// A crate with a foundation module that has code but no file of that module, and code that is not
/// classified outside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingModule {
    /// The foundation module that has no file.
    pub module: &'static str,
    /// What is neither in it nor classified outside it, in path order: each unclassified module
    /// (`src/<name>.rs` or `src/<name>/`), and `src/lib.rs` or `src/main.rs` when it defines
    /// functions itself.
    pub unclassified: Vec<String>,
}

/// For a crate with a foundation module and none of its `files` in that module: what is missing,
/// unless every piece of the crate's code is classified outside the module. `files` holds paths
/// relative to the repository root, each with whether it defines functions.
///
/// The crate passes without its foundation module only while it has no code at all, or every
/// module it has is in the entry's `outside` list and its root file (`src/lib.rs` or `src/main.rs`)
/// defines no function. A module no list names, or code in the root file, keeps the foundation
/// module missing: the module may be the foundation module built under another name, so it is
/// never treated as ordinary code until someone classifies it.
#[must_use]
pub fn missing_foundation_module(krate: &str, files: &[(String, bool)]) -> Option<MissingModule> {
    let entry = foundation_entry(krate)?;
    let in_module = files.iter().any(|(f, _)| {
        matches!(
            place(f),
            Place::Kernel {
                set: Set::Foundation,
                ..
            }
        )
    });
    let has_code = files.iter().any(|(_, code)| *code);
    if in_module || !has_code {
        return None;
    }
    let mut unclassified = Vec::new();
    for (file, code) in files {
        let parts = components(file);
        let Some(src) = parts
            .strip_prefix(KERNEL_CRATES_DIR.as_slice())
            .and_then(|rest| rest.strip_prefix([krate].as_slice()))
            .and_then(|rest| rest.strip_prefix(["src"].as_slice()))
        else {
            continue;
        };
        let Some(module) = module_of(src) else {
            continue;
        };
        let root = src.len() == 1 && (module == "lib" || module == "main");
        let name = if root || src.len() == 1 {
            format!("src/{}", src.join("/"))
        } else {
            format!("src/{module}/")
        };
        let classified = if root {
            !code
        } else {
            entry.outside.contains(&module)
        };
        if !classified && !unclassified.contains(&name) {
            unclassified.push(name);
        }
    }
    (!unclassified.is_empty()).then_some(MissingModule {
        module: entry.module,
        unclassified,
    })
}

/// The kernel crates in the kernel's folder under `repo`, in name order: every folder there with a
/// `Cargo.toml`, except the development crates.
///
/// # Errors
///
/// An I/O error if the folder cannot be read, or a folder name is not UTF-8.
pub fn kernel_crates(repo: &Path) -> std::io::Result<Vec<String>> {
    let dir = KERNEL_CRATES_DIR
        .iter()
        .fold(repo.to_path_buf(), |p, c| p.join(c));
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if !path.join("Cargo.toml").is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| std::io::Error::other(format!("{} is not UTF-8", path.display())))?
            .to_owned();
        if !DEVELOPMENT_CRATES.contains(&name.as_str()) {
            out.push(name);
        }
    }
    out.sort();
    Ok(out)
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

    const K: &str = "projects/engines/geometry/crates";

    fn kernel(krate: &str, set: Set) -> Place {
        Place::Kernel {
            krate: krate.to_owned(),
            set,
        }
    }

    #[test]
    fn the_foundation_set_is_28_8s() {
        assert_eq!(
            place(&format!("{K}/geom-num/src/grid.rs")),
            kernel("geom-num", Set::Foundation)
        );
        assert_eq!(
            place(&format!("{K}\\geom-poly\\src\\a\\b.rs")),
            kernel("geom-poly", Set::Foundation)
        );
        assert_eq!(
            place(&format!("{K}/geom-topo/src/builder.rs")),
            kernel("geom-topo", Set::Foundation)
        );
        assert_eq!(
            place(&format!("{K}/geom-topo/src/builder/euler.rs")),
            kernel("geom-topo", Set::Foundation)
        );
        assert_eq!(
            place(&format!("{K}/geom-topo/src/builders.rs")),
            kernel("geom-topo", Set::Elsewhere)
        );
        assert_eq!(
            place(&format!("{K}/geom-topo/src/lib.rs")),
            kernel("geom-topo", Set::Elsewhere)
        );
        assert_eq!(
            place(&format!("{K}/geom-2d/src/arrange/mod.rs")),
            kernel("geom-2d", Set::Foundation)
        );
        assert_eq!(
            place(&format!("{K}/geom-2d/src/arrange.rs")),
            kernel("geom-2d", Set::Foundation)
        );
        assert_eq!(
            place(&format!("{K}/geom-2d/src/arrangement.rs")),
            kernel("geom-2d", Set::Elsewhere)
        );
        assert_eq!(
            place(&format!("{K}/geom-2d/src/cogo.rs")),
            kernel("geom-2d", Set::Elsewhere)
        );
        assert_eq!(
            place(&format!("{K}/geom-bool/src/lib.rs")),
            kernel("geom-bool", Set::Elsewhere)
        );
    }

    #[test]
    fn tests_development_crates_and_other_folders_are_not_kernel_code() {
        assert_eq!(
            place(&format!("{K}/geom-num/tests/wide.rs")),
            Place::KernelOther {
                krate: "geom-num".to_owned()
            }
        );
        assert_eq!(
            place(&format!("{K}/geom-test/src/lib.rs")),
            Place::Development {
                krate: "geom-test".to_owned()
            }
        );
        assert_eq!(
            place("projects/engines/crates/engine-geometry/src/lib.rs"),
            Place::Outside
        );
        assert_eq!(place("Cargo.toml"), Place::Outside);
    }

    /// `(path, defines functions)` for each of `files` under `krate`'s folder.
    fn files(krate: &str, files: &[(&str, bool)]) -> Vec<(String, bool)> {
        files
            .iter()
            .map(|(f, code)| (format!("{K}/{krate}/{f}"), *code))
            .collect()
    }

    fn missing(module: &'static str, unclassified: &[&str]) -> Option<MissingModule> {
        Some(MissingModule {
            module,
            unclassified: unclassified.iter().map(|u| (*u).to_owned()).collect(),
        })
    }

    #[test]
    fn a_crate_with_code_but_no_foundation_module_is_reported() {
        let lib = files("geom-topo", &[("src/lib.rs", true)]);
        assert_eq!(
            missing_foundation_module("geom-topo", &lib),
            missing("builder", &["src/lib.rs"])
        );
        let scaffold = files("geom-topo", &[("src/lib.rs", false)]);
        assert_eq!(missing_foundation_module("geom-topo", &scaffold), None);
        let with = files(
            "geom-topo",
            &[("src/lib.rs", true), ("src/builder.rs", true)],
        );
        assert_eq!(missing_foundation_module("geom-topo", &with), None);
        let inside = files(
            "geom-topo",
            &[("src/lib.rs", false), ("src/builder/euler.rs", true)],
        );
        assert_eq!(missing_foundation_module("geom-topo", &inside), None);
        assert_eq!(missing_foundation_module("geom-bool", &lib), None);
    }

    /// The classified modules carry code without the foundation module, and nothing else does:
    /// a module no list names, code in the root file, or the module under its old name each keep
    /// it missing, and a module classified for one crate is not classified for another.
    #[test]
    fn only_modules_classified_outside_pass_without_the_foundation_module() {
        let classified = files(
            "geom-2d",
            &[
                ("src/lib.rs", false),
                ("src/cogo.rs", true),
                ("src/survey.rs", true),
                ("src/survey/linework.rs", true),
            ],
        );
        assert_eq!(missing_foundation_module("geom-2d", &classified), None);

        // A new module no list classifies: the foundation module is still missing, and the new
        // module is named, whether or not it defines functions itself.
        for code in [true, false] {
            let mut new = classified.clone();
            new.push((format!("{K}/geom-2d/src/regions.rs"), code));
            assert_eq!(
                missing_foundation_module("geom-2d", &new),
                missing("arrange", &["src/regions.rs"])
            );
        }
        let mut folder = classified.clone();
        folder.push((format!("{K}/geom-2d/src/hatch/fill.rs"), true));
        folder.push((format!("{K}/geom-2d/src/hatch/mod.rs"), true));
        assert_eq!(
            missing_foundation_module("geom-2d", &folder),
            missing("arrange", &["src/hatch/"])
        );

        // The arrangement under the name the list used to give it is not the foundation module,
        // so it is reported, not passed as ordinary code.
        let mut old_name = classified.clone();
        old_name.push((format!("{K}/geom-2d/src/arrangement.rs"), true));
        assert_eq!(
            missing_foundation_module("geom-2d", &old_name),
            missing("arrange", &["src/arrangement.rs"])
        );

        // Code in the root file is not classified.
        let mut root = classified.clone();
        root[0].1 = true;
        assert_eq!(
            missing_foundation_module("geom-2d", &root),
            missing("arrange", &["src/lib.rs"])
        );

        // With the foundation module present, the gate holds it to the foundation bar instead.
        let mut built = old_name.clone();
        built.push((format!("{K}/geom-2d/src/arrange.rs"), true));
        assert_eq!(missing_foundation_module("geom-2d", &built), None);

        // geom-2d's classification is geom-2d's alone.
        let topo = files("geom-topo", &[("src/lib.rs", false), ("src/cogo.rs", true)]);
        assert_eq!(
            missing_foundation_module("geom-topo", &topo),
            missing("builder", &["src/cogo.rs"])
        );
    }

    /// Every entry names a module of its own, outside no list of its own, and every classified
    /// module once.
    #[test]
    fn every_entry_is_consistent() {
        for (i, f) in FOUNDATION_MODULES.iter().enumerate() {
            assert!(!FOUNDATION_CRATES.contains(&f.krate), "{}", f.krate);
            assert!(!f.outside.contains(&f.module), "{}", f.krate);
            assert!(!f.outside.contains(&"lib") && !f.outside.contains(&"main"));
            for (j, m) in f.outside.iter().enumerate() {
                assert!(!f.outside[..j].contains(m), "{}: {m} twice", f.krate);
            }
            for g in &FOUNDATION_MODULES[..i] {
                assert_ne!(g.krate, f.krate);
            }
        }
    }
}
