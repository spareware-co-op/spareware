//! The registry: every generator whose output is committed, the folders only generators write, and
//! where the workspace is.
//!
//! What it does: [`REGISTRY`] lists every generator ([`GENERATORS`]) and every generated folder
//! ([`FOLDERS`]). Adding a generator means adding its module and one entry here, with the one file
//! it owns, inside a generated folder, and, if it uses randomness, its fixed seed; CI then checks
//! its output on every pull request. Removing or renaming one leaves its old output an orphan, which
//! `check` reports until it is deleted. [`workspace_root`] finds the workspace this crate was built
//! in, the default root for writing and checking.
//!
//! Guarantees: the registry keeps the rules [`crate::files::validate`] checks (unique kebab-case
//! names, portable paths, every output in a generated folder, no two outputs one file anywhere); a
//! unit test runs that check, so a broken entry fails the build's tests before CI runs a generator.
//!
//! Method: a static table of plain function pointers, so the set of generators is fixed at compile
//! time and visible in one place, in the order they run: the committed-output pattern of chapter
//! 27's proposed `geom-gen` rule
//! (`projects/engines/geometry/architecture/27-crates-folders.md`, "Generated code and data have
//! one home").
//!
//! Cost: none at run time beyond the generators themselves.

use std::path::{Path, PathBuf};

use crate::files::{Folder, Registry};
use crate::generator::{GenError, Generate, Generator};
use crate::grid_steps;
use crate::predicates;

/// Every generator, in the order they run.
pub const GENERATORS: [Generator; 7] = [
    Generator {
        name: grid_steps::NAME,
        output: grid_steps::OUTPUT,
        about: "the grid steps, common units on each step and the tolerance ladder, exactly, from geom-num",
        generate: Generate::Plain(grid_steps::generate),
    },
    Generator {
        name: predicates::MODULE_GENERATOR,
        output: predicates::MODULE_OUTPUT,
        about: "geom-num's generated predicate module: its declarations, shared helpers and proof",
        generate: Generate::Plain(predicates::generate_module),
    },
    Generator {
        name: "predicate-orient2d",
        output: "projects/engines/geometry/crates/geom-num/src/generated/orient2d.rs",
        about: "orient2d's f64 filter, exact path and bit-range certificate, from its description",
        generate: Generate::Plain(predicates::generate_orient2d),
    },
    Generator {
        name: "predicate-orient3d",
        output: "projects/engines/geometry/crates/geom-num/src/generated/orient3d.rs",
        about: "orient3d's f64 filter, exact path and bit-range certificate, from its description",
        generate: Generate::Plain(predicates::generate_orient3d),
    },
    Generator {
        name: "predicate-incircle",
        output: "projects/engines/geometry/crates/geom-num/src/generated/incircle.rs",
        about: "incircle's f64 filter, exact path and bit-range certificate, from its description",
        generate: Generate::Plain(predicates::generate_incircle),
    },
    Generator {
        name: "predicate-insphere",
        output: "projects/engines/geometry/crates/geom-num/src/generated/insphere.rs",
        about: "insphere's f64 filter, exact path and bit-range certificate, from its description",
        generate: Generate::Plain(predicates::generate_insphere),
    },
    Generator {
        name: "predicate-plane-side",
        output: "projects/engines/geometry/crates/geom-num/src/generated/plane_side.rs",
        about: "the side of a plane: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_plane_side),
    },
];

/// Every folder only generators write into. A folder stays listed while any file in it remains,
/// so an orphan left by a removed generator is reported, not forgotten.
pub const FOLDERS: [Folder; 2] = [
    Folder {
        path: "projects/engines/geometry/crates/geom-gen/generated",
        // Turns off git's line-ending conversion for the outputs, which `check` compares byte for
        // byte.
        keep: &[".gitattributes"],
    },
    Folder {
        path: predicates::OUTPUT_DIR,
        // The same, for geom-num's generated predicate modules.
        keep: &[".gitattributes"],
    },
];

/// The registry `cargo run -p geom-gen` works with.
pub const REGISTRY: Registry<'static> = Registry {
    generators: &GENERATORS,
    folders: &FOLDERS,
};

/// How many folders this crate's manifest sits below the workspace root:
/// `projects/engines/geometry/crates/geom-gen`.
const DEPTH: usize = 5;

/// The root of the workspace this crate was built in: five folders above its manifest.
///
/// # Errors
///
/// [`GenError::Io`] if that folder has no `Cargo.toml`, which would mean this crate moved
/// and `DEPTH` is wrong.
pub fn workspace_root() -> Result<PathBuf, GenError> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .ancestors()
        .nth(DEPTH)
        .ok_or_else(|| GenError::Io {
            path: manifest_dir.display().to_string(),
            message: format!("has fewer than {DEPTH} parent folders"),
        })?;
    if !root.join("Cargo.toml").is_file() {
        return Err(GenError::Io {
            path: root.display().to_string(),
            message: "has no Cargo.toml: not the workspace root".to_owned(),
        });
    }
    Ok(root.to_path_buf())
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
    use crate::files::validate;

    #[test]
    fn the_registry_keeps_its_rules() {
        validate(&REGISTRY).unwrap();
    }

    #[cfg_attr(
        target_family = "wasm",
        ignore = "reads the host's file system, which WASI cannot without a preopened folder"
    )]
    #[test]
    fn the_workspace_root_holds_this_crate() {
        let root = workspace_root().unwrap();
        assert!(
            root.join("projects/engines/geometry/crates/geom-gen/Cargo.toml")
                .is_file()
        );
    }
}
