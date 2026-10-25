//! Generators `predicate-module`, `predicate-orient2d`, `predicate-orient3d`, `predicate-incircle`,
//! `predicate-insphere` and `predicate-plane-side`: the kernel's predicates, from their
//! descriptions.
//!
//! What it does: holds the description of each predicate on explicit grid points (chapter 6.2:
//! orientation in 2D and 3D, in-circle and in-sphere), and of the side of a plane, whose
//! coefficients are wider integers whose `f64` copies are rounded (chapter 23.2), in the language
//! of [`crate::predicate_lang`], and the generators that derive each one
//! ([`crate::predicate_derive`]) and write its module into `geom-num`
//! ([`crate::predicate_emit`]): [`OUTPUT_DIR`]`/<name>.rs` for each predicate and
//! [`OUTPUT_DIR`]`/mod.rs` for the module that holds them. `geom-num`'s `predicate.rs` calls the
//! four explicit-point ones; `plane.rs`, outside this generator's piece, does not call
//! `plane_side` yet.
//!
//! Guarantees:
//!
//! - **The same formulas as `predicate.rs`.** Each description evaluates the determinant that
//!   `geom-num`'s hand-written fast path evaluates, term for term and with the same signs, so the
//!   generated exact value equals the hand-written one (tested in `geom-num`), and the generated
//!   filter's sign, when it gives one, is that value's sign. In-sphere shares the three cross
//!   products its four 3 × 3 determinants need instead of recomputing them; the values are the
//!   same. `plane_side` evaluates n · p + d as `geom-num`'s `Plane::side` does, term for term.
//! - **Pure.** No input but the descriptions and `geom-num`'s constants; the same bytes on every
//!   target.
//!
//! Method: one description per predicate, as Geogram's PCK writes one `.pck` file per predicate
//! and derives its filter and its exact path from it (Lévy 2016,
//! <https://doi.org/10.1016/j.cad.2015.10.004>; for example
//! <https://github.com/BrunoLevy/geogram/blob/16843b4804de35f14ef54667950ab4c8acdd0062/src/lib/geogram/numerics/predicates/orient3d.pck>,
//! BSD-3-Clause, read for method). The determinant forms are Shewchuk's (*Adaptive Precision
//! Floating-Point Arithmetic and Fast Robust Geometric Predicates*, 1997,
//! <https://doi.org/10.1007/PL00009321>), relative to the last point for in-circle and
//! in-sphere, with `geom-num`'s sign conventions (orient3d and insphere have the opposite sign to
//! `predicates.c`).
//! Consulted: Geogram PCK, BSD-3-Clause,
//!   <https://github.com/BrunoLevy/geogram/blob/16843b4804de35f14ef54667950ab4c8acdd0062/src/lib/geogram/numerics/predicates/orient3d.pck>,
//!   for its one description per predicate. No code copied.
//!
//! Cost: each generator derives one predicate, or all five for the parent module: a few hundred
//! exact operations each.

use crate::generator::GenError;
use crate::predicate_derive::{Derivation, derive};
use crate::predicate_emit::{Comparison, parent_module, predicate_module_serving, verus_twin};

/// The folder the generated modules are written into, relative to the workspace root.
pub const OUTPUT_DIR: &str = "projects/engines/geometry/crates/geom-num/src/generated";

/// orient2d: which way a, b, c turn, the sign of det[b − a; c − a].
pub const ORIENT2D: &str = "\
# orient2d: which way the points a, b, c turn (geom_num::predicate::orient2d).
# The sign of the determinant with rows b - a and c - a, as predicate.rs's fast path computes it.
predicate orient2d
input a b c: point2
let ba_x = b.x - a.x
let ba_y = b.y - a.y
let ca_x = c.x - a.x
let ca_y = c.y - a.y
let det = ba_x * ca_y - ba_y * ca_x
sign det
";

/// orient3d: which side of the plane through a, b, c the point d lies on.
pub const ORIENT3D: &str = "\
# orient3d: which side of the plane through a, b, c the point d lies on
# (geom_num::predicate::orient3d). The triple product (b - a) . ((c - a) x (d - a)),
# as predicate.rs's det3 computes it.
predicate orient3d
input a b c d: point3
let ba_x = b.x - a.x
let ba_y = b.y - a.y
let ba_z = b.z - a.z
let ca_x = c.x - a.x
let ca_y = c.y - a.y
let ca_z = c.z - a.z
let da_x = d.x - a.x
let da_y = d.y - a.y
let da_z = d.z - a.z
# (c - a) x (d - a)
let n_x = ca_y * da_z - ca_z * da_y
let n_y = ca_z * da_x - ca_x * da_z
let n_z = ca_x * da_y - ca_y * da_x
let det = ba_x * n_x + ba_y * n_y + ba_z * n_z
sign det
";

/// incircle: whether d lies inside the circle through a, b, c.
pub const INCIRCLE: &str = "\
# incircle: whether d lies inside the circle through a, b, c (geom_num::predicate::incircle).
# The lifted determinant relative to d: |a'|^2 (b' x c') + |b'|^2 (c' x a') + |c'|^2 (a' x b')
# with p' = p - d, as predicate.rs's fast path sums it.
predicate incircle
input a b c d: point2
let ad_x = a.x - d.x
let ad_y = a.y - d.y
let bd_x = b.x - d.x
let bd_y = b.y - d.y
let cd_x = c.x - d.x
let cd_y = c.y - d.y
let a_lift = ad_x * ad_x + ad_y * ad_y
let b_lift = bd_x * bd_x + bd_y * bd_y
let c_lift = cd_x * cd_x + cd_y * cd_y
let bc = bd_x * cd_y - bd_y * cd_x
let ca = cd_x * ad_y - cd_y * ad_x
let ab = ad_x * bd_y - ad_y * bd_x
let det = a_lift * bc + b_lift * ca + c_lift * ab
sign det
";

/// insphere: whether e lies inside the sphere through a, b, c, d.
pub const INSPHERE: &str = "\
# insphere: whether e lies inside the sphere through a, b, c, d
# (geom_num::predicate::insphere). The lifted determinant relative to e, with p' = p - e:
# |a'|^2 [b' c' d'] - |b'|^2 [a' c' d'] + |c'|^2 [a' b' d'] - |d'|^2 [a' b' c'],
# where [p q r] = p . (q x r), as predicate.rs's fast path sums it.
predicate insphere
input a b c d e: point3
let ae_x = a.x - e.x
let ae_y = a.y - e.y
let ae_z = a.z - e.z
let be_x = b.x - e.x
let be_y = b.y - e.y
let be_z = b.z - e.z
let ce_x = c.x - e.x
let ce_y = c.y - e.y
let ce_z = c.z - e.z
let de_x = d.x - e.x
let de_y = d.y - e.y
let de_z = d.z - e.z
let a_lift = ae_x * ae_x + ae_y * ae_y + ae_z * ae_z
let b_lift = be_x * be_x + be_y * be_y + be_z * be_z
let c_lift = ce_x * ce_x + ce_y * ce_y + ce_z * ce_z
let d_lift = de_x * de_x + de_y * de_y + de_z * de_z
# c' x d', b' x d' and b' x c'
let cd_x = ce_y * de_z - ce_z * de_y
let cd_y = ce_z * de_x - ce_x * de_z
let cd_z = ce_x * de_y - ce_y * de_x
let bd_x = be_y * de_z - be_z * de_y
let bd_y = be_z * de_x - be_x * de_z
let bd_z = be_x * de_y - be_y * de_x
let bc_x = be_y * ce_z - be_z * ce_y
let bc_y = be_z * ce_x - be_x * ce_z
let bc_z = be_x * ce_y - be_y * ce_x
let bcd = be_x * cd_x + be_y * cd_y + be_z * cd_z
let acd = ae_x * cd_x + ae_y * cd_y + ae_z * cd_z
let abd = ae_x * bd_x + ae_y * bd_y + ae_z * bd_z
let abc = ae_x * bc_x + ae_y * bc_y + ae_z * bc_z
let det = a_lift * bcd - b_lift * acd + c_lift * abd - d_lift * abc
sign det
";

/// `plane_side`: which side of the plane n · p + d = 0 the grid point p lies on.
pub const PLANE_SIDE: &str = "\
# plane_side: which side of the plane n . p + d = 0 the grid point p lies on: the sign of
# n . p + d, as geom_num::plane::Plane::side computes it, for a plane's integer normal n
# (|n_i| < 2^83) and offset d (|d| < 2^125). Their f64 copies are rounded (chapter 23.2).
predicate plane_side
input nx ny nz: rounded 83 in n
input d: rounded 125 in d
input p: point3
let s = nx * p.x + ny * p.y + nz * p.z + d
sign s
";

/// One predicate: its generator's name, the description's constant name, the description, its
/// output file, and the comparison row of the parent module.
pub struct Predicate {
    /// The generator's name.
    pub generator: &'static str,
    /// The name of the constant holding the description.
    pub constant: &'static str,
    /// The description.
    pub text: &'static str,
    /// The file it owns, relative to the workspace root.
    pub output: &'static str,
    /// Shewchuk's and FPG's published bounds for the same predicate.
    pub comparison: Comparison,
}

/// The predicates, in the order the registry runs them.
pub const PREDICATES: [Predicate; 5] = [
    Predicate {
        generator: "predicate-orient2d",
        constant: "ORIENT2D",
        text: ORIENT2D,
        output: "projects/engines/geometry/crates/geom-num/src/generated/orient2d.rs",
        comparison: Comparison {
            name: "orient2d",
            serves: "[`crate::predicate::orient2d`], which calls its exact path, and its filter \
                     first only on the targets where the filter pays (the routing in \
                     [`crate::predicate`]'s documentation), and whose documentation states the \
                     predicate, its sign convention and its degenerate cases",
            shewchuk: "(3 + 16ε)ε",
            fpg: "8.88720573725927976811e-16 · max₁ · max₂",
        },
    },
    Predicate {
        generator: "predicate-orient3d",
        constant: "ORIENT3D",
        text: ORIENT3D,
        output: "projects/engines/geometry/crates/geom-num/src/generated/orient3d.rs",
        comparison: Comparison {
            name: "orient3d",
            serves: "[`crate::predicate::orient3d`], which calls it, and whose documentation states \
                     the predicate, its sign convention and its degenerate cases",
            shewchuk: "(7 + 56ε)ε",
            fpg: "5.11071278299732992696e-15 · max₁ · max₂ · max₃",
        },
    },
    Predicate {
        generator: "predicate-incircle",
        constant: "INCIRCLE",
        text: INCIRCLE,
        output: "projects/engines/geometry/crates/geom-num/src/generated/incircle.rs",
        comparison: Comparison {
            name: "incircle",
            serves: "[`crate::predicate::incircle`], which calls it, and whose documentation states \
                     the predicate, its sign convention and its degenerate cases",
            shewchuk: "(10 + 96ε)ε",
            fpg: "— (Geogram lifts in-circle with heights passed in)",
        },
    },
    Predicate {
        generator: "predicate-insphere",
        constant: "INSPHERE",
        text: INSPHERE,
        output: "projects/engines/geometry/crates/geom-num/src/generated/insphere.rs",
        comparison: Comparison {
            name: "insphere",
            serves: "[`crate::predicate::insphere`], which calls it, and whose documentation states \
                     the predicate, its sign convention and its degenerate cases",
            shewchuk: "(16 + 224ε)ε",
            fpg: "— (Geogram lifts in-sphere with heights passed in)",
        },
    },
    Predicate {
        generator: "predicate-plane-side",
        constant: "PLANE_SIDE",
        text: PLANE_SIDE,
        output: "projects/engines/geometry/crates/geom-num/src/generated/plane_side.rs",
        comparison: Comparison {
            name: "plane_side",
            serves: "the side of a plane for a grid point, the sign of n · p + d for the normal n and \
                     offset d of a [`crate::plane::Plane`], integers wider than an `f64` holds, so \
                     their `f64` copies are rounded (chapter 23.2). [`crate::plane::Plane::side`] \
                     computes the same sign in `i128` and does not call this module yet; it is \
                     generated and tested so that rounded inputs are carried into an emitted bound",
            shewchuk: "— (not in predicates.c)",
            fpg: "— (Geogram has no side of an integer plane)",
        },
    },
];

/// The parent module's generator.
pub const MODULE_GENERATOR: &str = "predicate-module";

/// The parent module's file.
pub const MODULE_OUTPUT: &str = "projects/engines/geometry/crates/geom-num/src/generated/mod.rs";

/// The derivation of one predicate, as a generator error on failure.
///
/// # Errors
///
/// [`GenError::Generator`] naming the predicate's generator and the reason.
pub fn derivation(p: &Predicate) -> Result<Derivation, GenError> {
    derive(p.text).map_err(|e| GenError::generator(p.generator, e.to_string()))
}

/// One predicate's module, as bytes.
fn module_of(p: &Predicate) -> Result<Vec<u8>, GenError> {
    let d = derivation(p)?;
    predicate_module_serving(&d, p.generator, p.constant, p.comparison.serves)
        .map(String::into_bytes)
        .map_err(|e| GenError::generator(p.generator, e.0))
}

/// The predicate called `name`.
fn named(name: &str) -> Result<&'static Predicate, GenError> {
    PREDICATES
        .iter()
        .find(|p| p.comparison.name == name)
        .ok_or_else(|| GenError::generator(MODULE_GENERATOR, format!("no predicate `{name}`")))
}

/// Generator `predicate-orient2d`.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_orient2d() -> Result<Vec<u8>, GenError> {
    module_of(named("orient2d")?)
}

/// Generator `predicate-orient3d`.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_orient3d() -> Result<Vec<u8>, GenError> {
    module_of(named("orient3d")?)
}

/// Generator `predicate-incircle`.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_incircle() -> Result<Vec<u8>, GenError> {
    module_of(named("incircle")?)
}

/// Generator `predicate-insphere`.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_insphere() -> Result<Vec<u8>, GenError> {
    module_of(named("insphere")?)
}

/// Generator `predicate-plane-side`.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_plane_side() -> Result<Vec<u8>, GenError> {
    module_of(named("plane_side")?)
}

/// The folder of `geom-num`'s Verus crate that the predicate twins are generated into: modules of
/// `verus/lib.rs`, outside every crate's sources, so Cargo never builds them;
/// `kernel-dev.yml`'s `verus` job verifies the crate.
pub const VERUS_DIR: &str = "projects/engines/geometry/crates/geom-num/verus/generated";

/// The predicates with a Verus twin (pieces G36 and G38), by name.
pub const VERUS_TWINS: [&str; 4] = ["orient2d", "orient3d", "incircle", "insphere"];

/// The twin of the predicate called `name`, as bytes.
fn twin_of(name: &str) -> Result<Vec<u8>, GenError> {
    let generator = format!("verus-{name}");
    let d = derivation(named(name)?)?;
    verus_twin(&d, &generator, &format!("{OUTPUT_DIR}/{name}.rs"))
        .map(String::into_bytes)
        .map_err(|e| GenError::generator(&generator, e.0))
}

/// Generator `verus-orient2d`: orient2d's exact path as a Verus-annotated twin (piece G36).
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_verus_orient2d() -> Result<Vec<u8>, GenError> {
    twin_of("orient2d")
}

/// Generator `verus-orient3d`: orient3d's exact path as a Verus-annotated twin (piece G38).
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_verus_orient3d() -> Result<Vec<u8>, GenError> {
    twin_of("orient3d")
}

/// Generator `verus-incircle`: incircle's exact path as a Verus-annotated twin (piece G38).
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_verus_incircle() -> Result<Vec<u8>, GenError> {
    twin_of("incircle")
}

/// Generator `verus-insphere`: insphere's exact path as a Verus-annotated twin (piece G38).
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_verus_insphere() -> Result<Vec<u8>, GenError> {
    twin_of("insphere")
}

/// Generator `predicate-module`: the module that declares the predicates and holds their helpers.
///
/// # Errors
///
/// [`GenError::Generator`] if a derivation or the emission fails.
pub fn generate_module() -> Result<Vec<u8>, GenError> {
    let mut derivations = Vec::new();
    for p in &PREDICATES {
        derivations.push(derivation(p)?);
    }
    let rows: Vec<(&Derivation, &Comparison)> = derivations
        .iter()
        .zip(PREDICATES.iter().map(|p| &p.comparison))
        .collect();
    parent_module(&rows, MODULE_GENERATOR)
        .map(String::into_bytes)
        .map_err(|e| GenError::generator(MODULE_GENERATOR, e.0))
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
    use crate::files::first_difference_line;

    type Generate = fn() -> Result<Vec<u8>, GenError>;

    /// Each generator with its committed output, built into the test.
    const COMMITTED: [(&str, Generate, &[u8]); 6] = [
        (
            "mod.rs",
            generate_module,
            include_bytes!("../../geom-num/src/generated/mod.rs"),
        ),
        (
            "orient2d.rs",
            generate_orient2d,
            include_bytes!("../../geom-num/src/generated/orient2d.rs"),
        ),
        (
            "orient3d.rs",
            generate_orient3d,
            include_bytes!("../../geom-num/src/generated/orient3d.rs"),
        ),
        (
            "incircle.rs",
            generate_incircle,
            include_bytes!("../../geom-num/src/generated/incircle.rs"),
        ),
        (
            "insphere.rs",
            generate_insphere,
            include_bytes!("../../geom-num/src/generated/insphere.rs"),
        ),
        (
            "plane_side.rs",
            generate_plane_side,
            include_bytes!("../../geom-num/src/generated/plane_side.rs"),
        ),
    ];

    /// The committed files listed here are exactly the parent module and one per predicate, in
    /// the order of [`PREDICATES`], so a predicate added to the list cannot go unchecked.
    #[test]
    fn the_committed_list_is_every_predicate() {
        let listed: Vec<&str> = COMMITTED.iter().skip(1).map(|(f, _, _)| *f).collect();
        let predicates: Vec<String> = PREDICATES
            .iter()
            .map(|p| format!("{}.rs", p.comparison.name))
            .collect();
        assert_eq!(listed, predicates);
        assert_eq!(COMMITTED[0].0, "mod.rs");
        for p in &PREDICATES {
            assert!(p.output.ends_with(&format!("/{}.rs", p.comparison.name)));
        }
    }

    /// The committed files, built into this test, are exactly what the generators give on the
    /// target the test runs on: natively on every platform CI runs, and under wasm32-wasip1.
    #[test]
    fn the_committed_outputs_are_what_this_target_generates() {
        for (file, generate, committed) in COMMITTED {
            let generated = generate().unwrap();
            assert!(
                committed == generated.as_slice(),
                "the committed {file} differs from its generator's output on this target, first \
                 on line {}; run `cargo run -p geom-gen -- write`",
                first_difference_line(committed, &generated)
            );
        }
    }

    #[test]
    fn the_same_bytes_every_time_with_unix_line_ends() {
        for (_, generate, _) in COMMITTED {
            let a = generate().unwrap();
            assert_eq!(a, generate().unwrap());
            assert!(!a.contains(&b'\r'));
            assert!(a.ends_with(b"\n"));
            assert!(!a.ends_with(b"\n\n"));
        }
    }

    /// The registry runs exactly these generators, with these outputs, and its predicate folder
    /// is [`OUTPUT_DIR`], which holds every output here.
    #[test]
    fn the_registry_runs_every_predicate_generator() {
        use crate::registry::REGISTRY;
        let mut expected: Vec<(&str, &str)> =
            PREDICATES.iter().map(|p| (p.generator, p.output)).collect();
        expected.push((MODULE_GENERATOR, MODULE_OUTPUT));
        for (name, output) in &expected {
            let g = REGISTRY.generators.iter().find(|g| g.name == *name);
            assert_eq!(g.map(|g| g.output), Some(*output), "{name}");
            assert!(output.starts_with(&format!("{OUTPUT_DIR}/")), "{output}");
        }
        let in_folder = REGISTRY
            .generators
            .iter()
            .filter(|g| g.output.starts_with(&format!("{OUTPUT_DIR}/")))
            .count();
        assert_eq!(in_folder, expected.len());
        assert!(REGISTRY.folders.iter().any(|f| f.path == OUTPUT_DIR));
    }

    /// The bit budgets `predicate.rs` derives by hand (and asserts when `geom-num` compiles) are
    /// what the certificate derives: 83 bits for orient2d, 126 for orient3d, 168 for incircle's
    /// sum and 212 for insphere's, with the factors of the wide products in `i128` and the
    /// differences, 41 bits, in `i64`; and `plane.rs`'s 126 bits for a plane's side.
    #[test]
    fn the_certificates_match_predicate_rs_budgets() {
        let bits = |p: &Predicate, step: &str| {
            let d = derivation(p).unwrap();
            let i = d.steps.iter().position(|s| s.name == step).unwrap();
            (d.certs[i].bits, d.certs[i].width.rust())
        };
        assert_eq!(bits(&PREDICATES[0], "det"), (83, "i128"));
        assert_eq!(bits(&PREDICATES[0], "ba_x"), (41, "i64"));
        assert_eq!(bits(&PREDICATES[0], "det_1"), (82, "i128"));
        assert_eq!(bits(&PREDICATES[1], "n_x"), (83, "i128"));
        assert_eq!(bits(&PREDICATES[1], "det"), (126, "i128"));
        assert_eq!(bits(&PREDICATES[2], "a_lift"), (83, "i128"));
        assert_eq!(bits(&PREDICATES[2], "bc"), (83, "i128"));
        assert_eq!(bits(&PREDICATES[2], "det_1"), (166, "I256"));
        assert_eq!(bits(&PREDICATES[2], "det"), (168, "I256"));
        assert_eq!(bits(&PREDICATES[3], "a_lift"), (84, "i128"));
        assert_eq!(bits(&PREDICATES[3], "bcd"), (126, "i128"));
        assert_eq!(bits(&PREDICATES[3], "det_1"), (210, "I256"));
        assert_eq!(bits(&PREDICATES[3], "det"), (212, "I256"));
        assert_eq!(bits(&PREDICATES[4], "nx"), (83, "i128"));
        assert_eq!(bits(&PREDICATES[4], "d"), (125, "i128"));
        assert_eq!(bits(&PREDICATES[4], "s"), (126, "i128"));
    }
}
