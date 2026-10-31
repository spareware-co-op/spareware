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
use crate::quadrature;

/// Every generator, in the order they run.
pub const GENERATORS: [Generator; 93] = [
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
        name: "predicate-carrier-side",
        output: "projects/engines/geometry/crates/geom-num/src/generated/carrier_side.rs",
        about: "a rational point against a 2D carrier: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_carrier_side),
    },
    Generator {
        name: "predicate-carrier-order",
        output: "projects/engines/geometry/crates/geom-num/src/generated/carrier_order.rs",
        about: "two rational points along a 2D carrier: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_carrier_order),
    },
    Generator {
        name: "predicate-rational-cmp",
        output: "projects/engines/geometry/crates/geom-num/src/generated/rational_cmp.rs",
        about: "two fractions of a rational point's coordinates: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_rational_cmp),
    },
    Generator {
        name: "predicate-rational-orient",
        output: "projects/engines/geometry/crates/geom-num/src/generated/rational_orient.rs",
        about: "the turn of three rational points: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_rational_orient),
    },
    Generator {
        name: "predicate-round-grid",
        output: "projects/engines/geometry/crates/geom-num/src/generated/round_grid.rs",
        about: "a grid point against a circle about a rational point: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_round_grid),
    },
    Generator {
        name: "predicate-round-rational",
        output: "projects/engines/geometry/crates/geom-num/src/generated/round_rational.rs",
        about: "a rational point against a circle about a rational point: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_round_rational),
    },
    Generator {
        name: "predicate-round-line",
        output: "projects/engines/geometry/crates/geom-num/src/generated/round_line.rs",
        about: "a carrier against a circle about a rational point: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_round_line),
    },
    Generator {
        name: "predicate-circle-side",
        output: "projects/engines/geometry/crates/geom-num/src/generated/circle_side.rs",
        about: "a rational point against a grid-centred circle: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_circle_side),
    },
    Generator {
        name: "predicate-circle-line",
        output: "projects/engines/geometry/crates/geom-num/src/generated/circle_line.rs",
        about: "a carrier against a grid-centred circle: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_circle_line),
    },
    Generator {
        name: "predicate-circle-gap",
        output: "projects/engines/geometry/crates/geom-num/src/generated/circle_gap.rs",
        about: "the distance between two grid points against a whole length: its f64 filter on rounded inputs, exact path and certificate",
        generate: Generate::Plain(predicates::generate_circle_gap),
    },
    Generator {
        name: predicates::PLAIN_GENERATOR,
        output: predicates::PLAIN_OUTPUT,
        about: "every predicate's exact path in plain i128, the baseline of chapter 23.3's variants",
        generate: Generate::Plain(predicates::generate_plain),
    },
    Generator {
        name: predicates::LIMBS_GENERATOR,
        output: predicates::LIMBS_OUTPUT,
        about: "every predicate's exact path in width-aware 32-bit limbs, with its limb proofs",
        generate: Generate::Plain(predicates::generate_limbs),
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
        name: "verus-ssi-weight",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/ssi_weight.rs",
        about: "ssi_weight's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_ssi_weight),
    },
    Generator {
        name: "verus-ssi-side",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/ssi_side.rs",
        about: "ssi_side's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_ssi_side),
    },
    Generator {
        name: "verus-carrier-side",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/carrier_side.rs",
        about: "carrier_side's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_carrier_side),
    },
    Generator {
        name: "verus-carrier-order",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/carrier_order.rs",
        about: "carrier_order's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_carrier_order),
    },
    Generator {
        name: "verus-rational-cmp",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/rational_cmp.rs",
        about: "rational_cmp's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_rational_cmp),
    },
    Generator {
        name: "verus-rational-orient",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/rational_orient.rs",
        about: "rational_orient's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_rational_orient),
    },
    Generator {
        name: "verus-round-grid",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/round_grid.rs",
        about: "round_grid's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_round_grid),
    },
    Generator {
        name: "verus-round-rational",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/round_rational.rs",
        about: "round_rational's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_round_rational),
    },
    Generator {
        name: "verus-round-line",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/round_line.rs",
        about: "round_line's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_round_line),
    },
    Generator {
        name: "verus-circle-side",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/circle_side.rs",
        about: "circle_side's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_circle_side),
    },
    Generator {
        name: "verus-circle-line",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/circle_line.rs",
        about: "circle_line's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_circle_line),
    },
    Generator {
        name: "verus-circle-gap",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/circle_gap.rs",
        about: "circle_gap's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_circle_gap),
    },
    Generator {
        name: "verus-plane-side",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/plane_side.rs",
        about: "plane_side's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_plane_side),
    },
    Generator {
        name: "verus-tpi-weight",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/tpi_weight.rs",
        about: "tpi_weight's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_tpi_weight),
    },
    Generator {
        name: "verus-tpi-side",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/tpi_side.rs",
        about: "tpi_side's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_tpi_side),
    },
    Generator {
        name: "verus-lpi-weight",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/lpi_weight.rs",
        about: "lpi_weight's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_lpi_weight),
    },
    Generator {
        name: "verus-lpi-side",
        output: "projects/engines/geometry/crates/geom-num/verus/generated/lpi_side.rs",
        about: "lpi_side's exact path as a Verus-annotated twin, which Verus verifies in CI",
        generate: Generate::Plain(predicates::generate_verus_lpi_side),
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
        name: "gappa-carrier-side-n-p",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-carrier-side-n-p.g",
        about: "carrier_side's f64 filter for Gappa, one case of two, whose proof Rocq checks",
        generate: Generate::Plain(predicates::generate_gappa_carrier_side_n_p),
    },
    Generator {
        name: "gappa-carrier-side-d-w",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-carrier-side-d-w.g",
        about: "carrier_side's f64 filter for Gappa, one case of two, whose proof Rocq checks",
        generate: Generate::Plain(predicates::generate_gappa_carrier_side_d_w),
    },
    Generator {
        name: "gappa-carrier-order",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-carrier-order.g",
        about: "carrier_order's f64 filter for Gappa, whose proof of its error bound Rocq checks",
        generate: Generate::Plain(predicates::generate_gappa_carrier_order),
    },
    Generator {
        name: "gappa-rational-cmp",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-rational-cmp.g",
        about: "rational_cmp's f64 filter for Gappa, whose proof of its error bound Rocq checks",
        generate: Generate::Plain(predicates::generate_gappa_rational_cmp),
    },
    Generator {
        name: "gappa-rational-orient",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-rational-orient.g",
        about: "rational_orient's f64 filter for Gappa, whose proof of its error bound Rocq checks",
        generate: Generate::Plain(predicates::generate_gappa_rational_orient),
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
        name: "gappa-circle-gap-x",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-circle-gap-x.g",
        about: "circle_gap's f64 filter for Gappa, one case of three, whose proof Rocq checks",
        generate: Generate::Plain(predicates::generate_gappa_circle_gap_x),
    },
    Generator {
        name: "gappa-circle-gap-y",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-circle-gap-y.g",
        about: "circle_gap's f64 filter for Gappa, one case of three, whose proof Rocq checks",
        generate: Generate::Plain(predicates::generate_gappa_circle_gap_y),
    },
    Generator {
        name: "gappa-circle-gap-t",
        output: "projects/engines/geometry/crates/geom-gen/generated/gappa-circle-gap-t.g",
        about: "circle_gap's f64 filter for Gappa, one case of three, whose proof Rocq checks",
        generate: Generate::Plain(predicates::generate_gappa_circle_gap_t),
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
    Generator {
        name: "fptaylor-carrier-side",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-carrier-side.fpt",
        about: "carrier_side's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_carrier_side),
    },
    Generator {
        name: "fptaylor-carrier-order",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-carrier-order.fpt",
        about: "carrier_order's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_carrier_order),
    },
    Generator {
        name: "fptaylor-rational-cmp",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-rational-cmp.fpt",
        about: "rational_cmp's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_rational_cmp),
    },
    Generator {
        name: "fptaylor-rational-orient",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-rational-orient.fpt",
        about: "rational_orient's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_rational_orient),
    },
    Generator {
        name: "fptaylor-round-grid",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-round-grid.fpt",
        about: "round_grid's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_round_grid),
    },
    Generator {
        name: "fptaylor-round-rational",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-round-rational.fpt",
        about: "round_rational's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_round_rational),
    },
    Generator {
        name: "fptaylor-round-line",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-round-line.fpt",
        about: "round_line's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_round_line),
    },
    Generator {
        name: "fptaylor-circle-side",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-circle-side.fpt",
        about: "circle_side's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_circle_side),
    },
    Generator {
        name: "fptaylor-circle-line",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-circle-line.fpt",
        about: "circle_line's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_circle_line),
    },
    Generator {
        name: "fptaylor-circle-gap",
        output: "projects/engines/geometry/crates/geom-gen/generated/fptaylor-circle-gap.fpt",
        about: "circle_gap's normalised f64 filter for FPTaylor, whose certificate HOL Light checks",
        generate: Generate::Plain(predicates::generate_fptaylor_circle_gap),
    },
    Generator {
        name: quadrature::NAME,
        output: quadrature::OUTPUT,
        about: "the Gauss-Legendre and Gauss-Kronrod rules on [-1, 1], every node and weight correctly rounded from an exact enclosure",
        generate: Generate::Plain(quadrature::generate),
    },
];

/// Every folder only generators write into. A folder stays listed while any file in it remains,
/// so an orphan left by a removed generator is reported, not forgotten.
pub const FOLDERS: [Folder; 4] = [
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
    Folder {
        path: quadrature::OUTPUT_DIR,
        // The same, for geom-num's quadrature rules.
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
