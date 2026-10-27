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
pub const GENERATORS: [Generator; 45] = [
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
    Generator {
        name: "predicate-tpi-weight",
        output: "projects/engines/geometry/crates/geom-num/src/generated/tpi_weight.rs",
        about: "the weight's sign of a three-plane point: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_tpi_weight),
    },
    Generator {
        name: "predicate-tpi-side",
        output: "projects/engines/geometry/crates/geom-num/src/generated/tpi_side.rs",
        about: "a three-plane point against a plane: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_tpi_side),
    },
    Generator {
        name: "predicate-lpi-weight",
        output: "projects/engines/geometry/crates/geom-num/src/generated/lpi_weight.rs",
        about: "the weight's sign of a line-plane point: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_lpi_weight),
    },
    Generator {
        name: "predicate-lpi-side",
        output: "projects/engines/geometry/crates/geom-num/src/generated/lpi_side.rs",
        about: "a line-plane point against a plane: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_lpi_side),
    },
    Generator {
        name: "predicate-ssi-weight",
        output: "projects/engines/geometry/crates/geom-num/src/generated/ssi_weight.rs",
        about: "the weight's sign of a 2D crossing point: its f64 filter, exact path and certificate",
        generate: Generate::Plain(predicates::generate_ssi_weight),
    },
    Generator {
        name: "predicate-ssi-side",
        output: "projects/engines/geometry/crates/geom-num/src/generated/ssi_side.rs",
        about: "orient2d with a 2D crossing point: its f64 filter, exact path and certificate",
        generate: Generate::Plain(predicates::generate_ssi_side),
    },
    Generator {
        name: "verus-orient2d",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/orient2d.rs",
        about: "orient2d's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_orient2d),
    },
    Generator {
        name: "verus-orient3d",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/orient3d.rs",
        about: "orient3d's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_orient3d),
    },
    Generator {
        name: "verus-incircle",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/incircle.rs",
        about: "incircle's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_incircle),
    },
    Generator {
        name: "verus-insphere",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/insphere.rs",
        about: "insphere's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_insphere),
    },
    Generator {
        name: "gappa-orient2d",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-orient2d.g",
        about: "orient2d's f64 filter for Gappa, whose proof of its error bound Rocq checks",
        generate: Generate::Plain(predicates::generate_gappa_orient2d),
    },
    Generator {
        name: "gappa-orient3d",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-orient3d.g",
        about: "orient3d's f64 filter for Gappa, whose proof of its error bound Rocq checks",
        generate: Generate::Plain(predicates::generate_gappa_orient3d),
    },
    Generator {
        name: "gappa-incircle-x",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-incircle-x.g",
        about: "incircle's f64 filter for Gappa where m_x is the largest maximum; Rocq checks the proof",
        generate: Generate::Plain(predicates::generate_gappa_incircle_x),
    },
    Generator {
        name: "gappa-incircle-y",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-incircle-y.g",
        about: "incircle's f64 filter for Gappa where m_y is the largest maximum; Rocq checks the proof",
        generate: Generate::Plain(predicates::generate_gappa_incircle_y),
    },
    Generator {
        name: "gappa-insphere-x",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-insphere-x.g",
        about: "insphere's f64 filter for Gappa where m_x is the largest maximum; Rocq checks the proof",
        generate: Generate::Plain(predicates::generate_gappa_insphere_x),
    },
    Generator {
        name: "gappa-insphere-y",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-insphere-y.g",
        about: "insphere's f64 filter for Gappa where m_y is the largest maximum; Rocq checks the proof",
        generate: Generate::Plain(predicates::generate_gappa_insphere_y),
    },
    Generator {
        name: "gappa-insphere-z",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-insphere-z.g",
        about: "insphere's f64 filter for Gappa where m_z is the largest maximum; Rocq checks the proof",
        generate: Generate::Plain(predicates::generate_gappa_insphere_z),
    },
    Generator {
        name: "gappa-plane-side-d",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-plane-side-d.g",
        about: "plane_side's f64 filter for Gappa where m_d is the largest term; Rocq checks the proof",
        generate: Generate::Plain(predicates::generate_gappa_plane_side_d),
    },
    Generator {
        name: "gappa-plane-side-z-n",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-plane-side-z-n.g",
        about: "plane_side's f64 filter for Gappa where m_z * m_n is the largest term; Rocq checks the proof",
        generate: Generate::Plain(predicates::generate_gappa_plane_side_z_n),
    },
    Generator {
        name: "gappa-plane-side-y-n",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-plane-side-y-n.g",
        about: "plane_side's f64 filter for Gappa where m_y * m_n is the largest term; Rocq checks the proof",
        generate: Generate::Plain(predicates::generate_gappa_plane_side_y_n),
    },
    Generator {
        name: "gappa-plane-side-x-n",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-plane-side-x-n.g",
        about: "plane_side's f64 filter for Gappa where m_x * m_n is the largest term; Rocq checks the proof",
        generate: Generate::Plain(predicates::generate_gappa_plane_side_x_n),
    },
    Generator {
        name: "gappa-tpi-weight",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-tpi-weight.g",
        about: "tpi_weight's f64 filter for Gappa, whose proof of its error bound Rocq checks",
        generate: Generate::Plain(predicates::generate_gappa_tpi_weight),
    },
    Generator {
        name: "gappa-ssi-weight",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-ssi-weight.g",
        about: "ssi_weight's f64 filter for Gappa, whose proof of its error bound Rocq checks",
        generate: Generate::Plain(predicates::generate_gappa_ssi_weight),
    },
    Generator {
        name: "gappa-ssi-side",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-ssi-side.g",
        about: "ssi_side's f64 filter for Gappa, whose proof of its error bound Rocq checks",
        generate: Generate::Plain(predicates::generate_gappa_ssi_side),
    },
    Generator {
        name: "gappa-lpi-weight-z-n",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-lpi-weight-z-n.g",
        about: "lpi_weight's f64 filter for Gappa where m_z * m_n is the largest term; Rocq checks the proof",
        generate: Generate::Plain(predicates::generate_gappa_lpi_weight_z_n),
    },
    Generator {
        name: "gappa-lpi-weight-y-n",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-lpi-weight-y-n.g",
        about: "lpi_weight's f64 filter for Gappa where m_y * m_n is the largest term; Rocq checks the proof",
        generate: Generate::Plain(predicates::generate_gappa_lpi_weight_y_n),
    },
    Generator {
        name: "gappa-lpi-weight-x-n",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-lpi-weight-x-n.g",
        about: "lpi_weight's f64 filter for Gappa where m_x * m_n is the largest term; Rocq checks the proof",
        generate: Generate::Plain(predicates::generate_gappa_lpi_weight_x_n),
    },
    Generator {
        name: "fptaylor-orient2d",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-orient2d.fpt",
        about: "orient2d's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_orient2d),
    },
    Generator {
        name: "fptaylor-orient3d",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-orient3d.fpt",
        about: "orient3d's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_orient3d),
    },
    Generator {
        name: "fptaylor-incircle",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-incircle.fpt",
        about: "incircle's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_incircle),
    },
    Generator {
        name: "fptaylor-insphere",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-insphere.fpt",
        about: "insphere's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_insphere),
    },
    Generator {
        name: "fptaylor-plane-side",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-plane-side.fpt",
        about: "plane_side's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_plane_side),
    },
    Generator {
        name: "fptaylor-tpi-weight",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-tpi-weight.fpt",
        about: "tpi_weight's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_tpi_weight),
    },
    Generator {
        name: "fptaylor-tpi-side",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-tpi-side.fpt",
        about: "tpi_side's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_tpi_side),
    },
    Generator {
        name: "fptaylor-lpi-weight",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-lpi-weight.fpt",
        about: "lpi_weight's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_lpi_weight),
    },
    Generator {
        name: "fptaylor-lpi-side",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-lpi-side.fpt",
        about: "lpi_side's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_lpi_side),
    },
    Generator {
        name: "fptaylor-ssi-weight",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-ssi-weight.fpt",
        about: "ssi_weight's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_ssi_weight),
    },
    Generator {
        name: "fptaylor-ssi-side",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-ssi-side.fpt",
        about: "ssi_side's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_ssi_side),
    },
];

/// Every folder only generators write into. A folder stays listed while any file in it remains,
/// so an orphan left by a removed generator is reported, not forgotten.
pub const FOLDERS: [Folder; 3] = [
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
    Folder {
        path: predicates::VERUS_DIR,
        // The same, for the predicates' Verus twins in geom-num's Verus crate.
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
