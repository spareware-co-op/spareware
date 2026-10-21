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
//! everything under `src/<module>/`. Those modules do not exist yet (both crates are scaffolds), so
//! the coverage gate refuses a crate that has code but no file of its foundation module
//! ([`missing_foundation_module`]): if the module is built under another path, the gate fails until
//! [`FOUNDATION_MODULES`] names it, rather than silently treating it as ordinary code.
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

/// The foundation modules of crates not wholly in the set: (crate, module under `src/`).
pub const FOUNDATION_MODULES: [(&str, &str); 2] =
    [("geom-topo", "builder"), ("geom-2d", "arrangement")];

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

/// Whether `src` (the components after `src/`) is in `krate`'s foundation module.
fn in_foundation_module(krate: &str, src: &[&str]) -> bool {
    FOUNDATION_MODULES
        .iter()
        .filter(|(c, _)| *c == krate)
        .any(|(_, module)| match src {
            [file] => file.strip_suffix(".rs") == Some(*module),
            [dir, _, ..] => dir == module,
            [] => false,
        })
}

/// The foundation module `krate` must have, if any.
#[must_use]
pub fn foundation_module(krate: &str) -> Option<&'static str> {
    FOUNDATION_MODULES
        .iter()
        .find(|(c, _)| *c == krate)
        .map(|(_, m)| *m)
}

/// For a crate with a foundation module: the module's name if the crate has code (`has_code`) but
/// none of `files` (paths relative to the repository root) is in that module.
#[must_use]
pub fn missing_foundation_module(
    krate: &str,
    has_code: bool,
    files: &[String],
) -> Option<&'static str> {
    let module = foundation_module(krate)?;
    let found = files.iter().any(|f| {
        matches!(
            place(f),
            Place::Kernel {
                set: Set::Foundation,
                ..
            }
        )
    });
    (has_code && !found).then_some(module)
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
            place(&format!("{K}/geom-2d/src/arrangement/mod.rs")),
            kernel("geom-2d", Set::Foundation)
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

    #[test]
    fn a_crate_with_code_but_no_foundation_module_is_reported() {
        let lib = vec![format!("{K}/geom-topo/src/lib.rs")];
        assert_eq!(
            missing_foundation_module("geom-topo", true, &lib),
            Some("builder")
        );
        assert_eq!(missing_foundation_module("geom-topo", false, &lib), None);
        let with = vec![lib[0].clone(), format!("{K}/geom-topo/src/builder.rs")];
        assert_eq!(missing_foundation_module("geom-topo", true, &with), None);
        assert_eq!(missing_foundation_module("geom-bool", true, &lib), None);
    }
}
