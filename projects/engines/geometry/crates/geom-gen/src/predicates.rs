//! Generators `predicate-module`, `predicate-orient2d`, `predicate-orient3d`, `predicate-incircle`,
//! `predicate-insphere`, `predicate-plane-side`, `predicate-tpi-weight`, `predicate-tpi-side`,
//! `predicate-lpi-weight`, `predicate-lpi-side`, `predicate-ssi-weight`, `predicate-ssi-side`,
//! `predicate-variant-plain` and `predicate-variant-limbs`: the kernel's predicates, from their
//! descriptions; `verus-orient2d`, `verus-orient3d`, `verus-incircle`, `verus-insphere`,
//! `verus-plane-side`, `verus-tpi-weight`, `verus-tpi-side`, `verus-lpi-weight`, `verus-lpi-side`,
//! `verus-ssi-weight` and `verus-ssi-side`, the Verus twins of the predicates' exact paths; and
//! `gappa-orient2d`, `gappa-orient3d`, `gappa-incircle-x`, `gappa-incircle-y`, `gappa-insphere-x`,
//! `gappa-insphere-y`, `gappa-insphere-z`, `gappa-plane-side-d`, `gappa-plane-side-z-n`,
//! `gappa-plane-side-y-n`, `gappa-plane-side-x-n`, `gappa-tpi-weight`, `gappa-ssi-weight`,
//! `gappa-ssi-side`, `gappa-lpi-weight-z-n`, `gappa-lpi-weight-y-n`, `gappa-lpi-weight-x-n`,
//! `fptaylor-orient2d`, `fptaylor-orient3d`, `fptaylor-incircle`, `fptaylor-insphere`,
//! `fptaylor-plane-side`, `fptaylor-tpi-weight`, `fptaylor-tpi-side`, `fptaylor-lpi-weight`,
//! `fptaylor-lpi-side`, `fptaylor-ssi-weight` and `fptaylor-ssi-side`, the problems that
//! machine-check the filters' error bounds ([`crate::predicate_certificates`]).
//!
//! What it does: holds the description of each predicate on explicit grid points (chapter 6.2:
//! orientation in 2D and 3D, in-circle and in-sphere), of the side of a plane, whose
//! coefficients are wider integers whose `f64` copies are rounded (chapter 23.2), and of the
//! predicates on implicit points (chapter 6.3, piece G29): the sign of the weight of a
//! three-plane point, of a line-plane point and of a 2D crossing point, and which side of a plane
//! (or, in 2D, of a line) each lies on, in the language of [`crate::predicate_lang`], and the
//! generators that derive each one ([`crate::predicate_derive`]) and write its module into
//! `geom-num` ([`crate::predicate_emit`]): [`OUTPUT_DIR`]`/<name>.rs` for each predicate and
//! [`OUTPUT_DIR`]`/mod.rs` for the module that holds them, and [`PLAIN_OUTPUT`] and
//! [`LIMBS_OUTPUT`] for every exact path in plain `i128` and in width-aware 32-bit limbs, two of
//! chapter 23.3's variants (piece G42), which the benchmarks compare with the shipped paths. `geom-num`'s `predicate.rs` calls the
//! four explicit-point ones, `plane.rs` the side of a plane and the three-plane and line-plane
//! ones, and `crossing.rs` the 2D crossing ones.
//!
//! Guarantees:
//!
//! - **The same formulas as `predicate.rs`, `plane.rs` and `crossing.rs`.** Each description
//!   evaluates the determinant that `geom-num`'s hand-written fast path evaluates, term for term
//!   and with the same signs, so the generated exact value equals the hand-written one (tested in
//!   `geom-num`), and the generated filter's sign, when it gives one, is that value's sign.
//!   In-sphere shares the three cross products its four 3 × 3 determinants need instead of
//!   recomputing them; the values are the same. `plane_side` evaluates n · p + d as `geom-num`'s
//!   `Plane::side` does, term for term; `tpi_weight` and `tpi_side` evaluate the three-plane
//!   point's weight and its classification as `Tpi::homogeneous` and `Plane::side_of_tpi` do,
//!   except that the classification subtracts n · (−X) where `plane.rs` adds n · X, the same
//!   value; and the line-plane and crossing descriptions evaluate what `plane.rs` and
//!   `crossing.rs` document for them, step for step.
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
//! Cost: each generator derives one predicate, or all eleven for the parent module and each
//! variant: a few hundred exact operations each.

use crate::generator::GenError;
use crate::predicate_certificates::{fptaylor_problem, gappa_case_problem, gappa_problem};
use crate::predicate_derive::{Derivation, derive};
use crate::predicate_emit::{
    Comparison, EmitError, Style, parent_module, predicate_module_serving, variant_module,
    verus_twin,
};

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

/// `tpi_weight`: the sign of the weight w of the point where three planes meet.
pub const TPI_WEIGHT: &str = "\
# tpi_weight: the sign of the weight w = n1 . (n2 x n3) of the point where the planes
# (n1, d1), (n2, d2), (n3, d3) meet, the determinant of their normals, as
# geom_num::plane::Tpi::homogeneous computes it. Its sign corrects the sign of every
# classification of the point (geom_num::plane::Plane::side_of_tpi). Each normal is a group of
# its own: w is homogeneous of degree 1 in each.
predicate tpi_weight
input n1x n1y n1z: rounded 83 in n1
input n2x n2y n2z: rounded 83 in n2
input n3x n3y n3z: rounded 83 in n3
# n2 x n3
let c23_x = n2y * n3z - n2z * n3y
let c23_y = n2z * n3x - n2x * n3z
let c23_z = n2x * n3y - n2y * n3x
let w = n1x * c23_x + n1y * c23_y + n1z * c23_z
sign w
";

/// `tpi_side`: which side of a fourth plane the point where three planes meet lies on, before the
/// weight's sign corrects it.
pub const TPI_SIDE: &str = "\
# tpi_side: the sign of n4 . X + d4 w for the point X / w where the planes (n1, d1), (n2, d2) and
# (n3, d3) meet, against the plane (n4, d4): the 4 x 4 determinant of the four planes'
# coefficients, as geom_num::plane::Plane::side_of_tpi computes it from Tpi::homogeneous, with
# X = -mx, mx = d1 (n2 x n3) + d2 (n3 x n1) + d3 (n1 x n2), and w = n1 . (n2 x n3). Times the sign
# of w (tpi_weight) it is the side the point lies on. Each plane's normal and offset are groups
# of their own: every term is the product of one offset and three normals of different planes.
predicate tpi_side
input n1x n1y n1z: rounded 83 in n1
input d1: rounded 125 in d1
input n2x n2y n2z: rounded 83 in n2
input d2: rounded 125 in d2
input n3x n3y n3z: rounded 83 in n3
input d3: rounded 125 in d3
input n4x n4y n4z: rounded 83 in n4
input d4: rounded 125 in d4
# n2 x n3, n3 x n1 and n1 x n2
let c23_x = n2y * n3z - n2z * n3y
let c23_y = n2z * n3x - n2x * n3z
let c23_z = n2x * n3y - n2y * n3x
let c31_x = n3y * n1z - n3z * n1y
let c31_y = n3z * n1x - n3x * n1z
let c31_z = n3x * n1y - n3y * n1x
let c12_x = n1y * n2z - n1z * n2y
let c12_y = n1z * n2x - n1x * n2z
let c12_z = n1x * n2y - n1y * n2x
let w = n1x * c23_x + n1y * c23_y + n1z * c23_z
# minus the numerators X
let mx_x = d1 * c23_x + d2 * c31_x + d3 * c12_x
let mx_y = d1 * c23_y + d2 * c31_y + d3 * c12_y
let mx_z = d1 * c23_z + d2 * c31_z + d3 * c12_z
let s = d4 * w - n4x * mx_x - n4y * mx_y - n4z * mx_z
sign s
";

/// `lpi_weight`: the sign of the weight w of the point where the line through two grid points
/// meets a plane.
pub const LPI_WEIGHT: &str = "\
# lpi_weight: the sign of the weight w = n . (q - p) of the point where the line from p to q
# meets the plane (n, d), as geom_num::plane::Lpi::homogeneous computes it: zero when the line is
# parallel to the plane. Its sign corrects the sign of every classification of the point
# (geom_num::plane::Plane::side_of_lpi).
predicate lpi_weight
input nx ny nz: rounded 83 in n
input p q: point3
let qp_x = q.x - p.x
let qp_y = q.y - p.y
let qp_z = q.z - p.z
let w = nx * qp_x + ny * qp_y + nz * qp_z
sign w
";

/// `lpi_side`: which side of a second plane the point where a line meets a plane lies on, before
/// the weight's sign corrects it.
pub const LPI_SIDE: &str = "\
# lpi_side: the sign of w1 s2p - w2 s1p for the point where the line from p to q meets the plane
# (n1, d1), against the plane (n2, d2), with wk = nk . (q - p) and skp = nk . p + dk, as
# geom_num::plane::Plane::side_of_lpi computes it. It equals n2 . X + d2 w1 for the point's
# homogeneous coordinates X = s1q p - s1p q, w1 (s1q = s1p + w1); times the sign of w1
# (lpi_weight) it is the side the point lies on. Each plane's normal and offset are groups of
# their own.
predicate lpi_side
input n1x n1y n1z: rounded 83 in n1
input d1: rounded 125 in d1
input n2x n2y n2z: rounded 83 in n2
input d2: rounded 125 in d2
input p q: point3
let qp_x = q.x - p.x
let qp_y = q.y - p.y
let qp_z = q.z - p.z
let w1 = n1x * qp_x + n1y * qp_y + n1z * qp_z
let w2 = n2x * qp_x + n2y * qp_y + n2z * qp_z
let s1p = n1x * p.x + n1y * p.y + n1z * p.z + d1
let s2p = n2x * p.x + n2y * p.y + n2z * p.z + d2
let s = w1 * s2p - w2 * s1p
sign s
";

/// `ssi_weight`: the sign of the weight w of the point where the lines through two pairs of grid
/// points cross.
pub const SSI_WEIGHT: &str = "\
# ssi_weight: the sign of the weight w = (b - a) x (d - c) of the point where the line from a to b
# crosses the line from c to d, as geom_num::crossing::Ssi::homogeneous computes it: zero when
# the lines are parallel. Its sign corrects the sign of every classification of the point
# (geom_num::crossing::Ssi::orient2d).
predicate ssi_weight
input a b c d: point2
let ba_x = b.x - a.x
let ba_y = b.y - a.y
let dc_x = d.x - c.x
let dc_y = d.y - c.y
let w = ba_x * dc_y - ba_y * dc_x
sign w
";

/// `ssi_side`: which way two grid points and the point where two lines cross turn, before the
/// weight's sign corrects it.
pub const SSI_SIDE: &str = "\
# ssi_side: the sign of oa ob2 - ob oa2 for the point where the line from a to b crosses the line
# from c to d, against the line from p to q, with oa = orient2d(c, d, a), ob = orient2d(c, d, b),
# oa2 = orient2d(p, q, a) and ob2 = orient2d(p, q, b), as geom_num::crossing::Ssi::orient2d
# computes it. It equals (q - p) x (X - w p) for the point's homogeneous coordinates
# X = oa b - ob a, w = oa - ob; times the sign of w (ssi_weight) it is orient2d(p, q, x).
predicate ssi_side
input a b c d p q: point2
let dc_x = d.x - c.x
let dc_y = d.y - c.y
let ac_x = a.x - c.x
let ac_y = a.y - c.y
let bc_x = b.x - c.x
let bc_y = b.y - c.y
let qp_x = q.x - p.x
let qp_y = q.y - p.y
let ap_x = a.x - p.x
let ap_y = a.y - p.y
let bp_x = b.x - p.x
let bp_y = b.y - p.y
let oa = dc_x * ac_y - dc_y * ac_x
let ob = dc_x * bc_y - dc_y * bc_x
let oa2 = qp_x * ap_y - qp_y * ap_x
let ob2 = qp_x * bp_y - qp_y * bp_x
let s = oa * ob2 - ob * oa2
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
pub const PREDICATES: [Predicate; 11] = [
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
    Predicate {
        generator: "predicate-tpi-weight",
        constant: "TPI_WEIGHT",
        text: TPI_WEIGHT,
        output: "projects/engines/geometry/crates/geom-num/src/generated/tpi_weight.rs",
        comparison: Comparison {
            name: "tpi_weight",
            serves: "the sign of the weight w of a three-plane point, the determinant of its planes' normals, \
                     integers wider than an `f64` holds, so their `f64` copies are rounded \
                     (chapter 23.2). [`crate::plane::Plane::side_of_tpi`] calls it, filter first, \
                     to correct the sign of `tpi_side`; [`crate::plane::Tpi`] documents the point \
                     and its degenerate cases",
            shewchuk: "— (not in predicates.c)",
            fpg: "— (Geogram has no three-plane point)",
        },
    },
    Predicate {
        generator: "predicate-tpi-side",
        constant: "TPI_SIDE",
        text: TPI_SIDE,
        output: "projects/engines/geometry/crates/geom-num/src/generated/tpi_side.rs",
        comparison: Comparison {
            name: "tpi_side",
            serves: "the classification of a three-plane point against a fourth plane, the determinant of the \
                     four planes' coefficients, which are rounded as `f64`s (chapter 23.2), up to \
                     the sign of the point's weight. [`crate::plane::Plane::side_of_tpi`] calls it, \
                     filter first, and multiplies its sign by `tpi_weight`'s; its documentation \
                     states the predicate and its bit budget",
            shewchuk: "— (not in predicates.c)",
            fpg: "— (Geogram has no three-plane point)",
        },
    },
    Predicate {
        generator: "predicate-lpi-weight",
        constant: "LPI_WEIGHT",
        text: LPI_WEIGHT,
        output: "projects/engines/geometry/crates/geom-num/src/generated/lpi_weight.rs",
        comparison: Comparison {
            name: "lpi_weight",
            serves: "the sign of the weight w = n · (q − p) of a line-plane point, zero when the line is \
                     parallel to the plane, with the plane's normal rounded as `f64`s (chapter \
                     23.2). [`crate::plane::Plane::side_of_lpi`] calls it to correct the sign of \
                     `lpi_side`, filter first where the filter pays (its documentation, routing); \
                     [`crate::plane::Lpi`] documents the point and its degenerate cases",
            shewchuk: "— (not in predicates.c)",
            fpg: "— (Geogram has no line-plane point)",
        },
    },
    Predicate {
        generator: "predicate-lpi-side",
        constant: "LPI_SIDE",
        text: LPI_SIDE,
        output: "projects/engines/geometry/crates/geom-num/src/generated/lpi_side.rs",
        comparison: Comparison {
            name: "lpi_side",
            serves: "the classification of a line-plane point against a second plane, with both planes' \
                     coefficients rounded as `f64`s (chapter 23.2), up to the sign of the point's \
                     weight. [`crate::plane::Plane::side_of_lpi`] calls it, filter first, and \
                     multiplies its sign by `lpi_weight`'s; its documentation states the \
                     predicate and its bit budget",
            shewchuk: "— (not in predicates.c)",
            fpg: "— (Geogram has no line-plane point)",
        },
    },
    Predicate {
        generator: "predicate-ssi-weight",
        constant: "SSI_WEIGHT",
        text: SSI_WEIGHT,
        output: "projects/engines/geometry/crates/geom-num/src/generated/ssi_weight.rs",
        comparison: Comparison {
            name: "ssi_weight",
            serves: "the sign of the weight w = (b − a) × (d − c) of the point where two lines through grid \
                     points cross, zero when they are parallel. \
                     [`crate::crossing::Ssi::orient2d`] calls it to correct the sign of \
                     `ssi_side`, filter first where the filter pays (its documentation, routing); \
                     [`crate::crossing::Ssi`] documents the point and its degenerate cases",
            shewchuk: "— (not in predicates.c)",
            fpg: "— (Geogram has no implicit crossing point)",
        },
    },
    Predicate {
        generator: "predicate-ssi-side",
        constant: "SSI_SIDE",
        text: SSI_SIDE,
        output: "projects/engines/geometry/crates/geom-num/src/generated/ssi_side.rs",
        comparison: Comparison {
            name: "ssi_side",
            serves: "orient2d of two grid points and the point where two lines through grid points cross, up \
                     to the sign of the point's weight. [`crate::crossing::Ssi::orient2d`] calls \
                     it, filter first, and multiplies its sign by `ssi_weight`'s; its \
                     documentation states the predicate and its bit budget",
            shewchuk: "— (not in predicates.c)",
            fpg: "— (Geogram has no implicit crossing point)",
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

/// Generator `predicate-tpi-weight`.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_tpi_weight() -> Result<Vec<u8>, GenError> {
    module_of(named("tpi_weight")?)
}

/// Generator `predicate-tpi-side`.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_tpi_side() -> Result<Vec<u8>, GenError> {
    module_of(named("tpi_side")?)
}

/// Generator `predicate-lpi-weight`.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_lpi_weight() -> Result<Vec<u8>, GenError> {
    module_of(named("lpi_weight")?)
}

/// Generator `predicate-lpi-side`.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_lpi_side() -> Result<Vec<u8>, GenError> {
    module_of(named("lpi_side")?)
}

/// Generator `predicate-ssi-weight`.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_ssi_weight() -> Result<Vec<u8>, GenError> {
    module_of(named("ssi_weight")?)
}

/// Generator `predicate-ssi-side`.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_ssi_side() -> Result<Vec<u8>, GenError> {
    module_of(named("ssi_side")?)
}

/// The folder of `geom-num`'s Verus crate that the predicate twins are generated into: modules of
/// `verus/lib.rs`, outside every crate's sources, so Cargo never builds them;
/// `kernel-dev.yml`'s `verus` job verifies the crate.
pub const VERUS_DIR: &str = "projects/engines/geometry/crates/geom-num/verus/generated";

/// The predicates with a Verus twin (pieces G36, G38 and G39), by name.
pub const VERUS_TWINS: [&str; 11] = [
    "orient2d",
    "orient3d",
    "incircle",
    "insphere",
    "ssi_weight",
    "ssi_side",
    "plane_side",
    "tpi_weight",
    "tpi_side",
    "lpi_weight",
    "lpi_side",
];

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

/// The name of the generator of the plain-`i128` variant (chapter 23.3; piece G42).
pub const PLAIN_GENERATOR: &str = "predicate-variant-plain";

/// Where the plain-`i128` variant is written: beside the predicate modules, whose helpers it
/// shares.
pub const PLAIN_OUTPUT: &str = "projects/engines/geometry/crates/geom-num/src/generated/plain.rs";

/// Generator `predicate-variant-plain`: every predicate's exact path in plain `i128`, the
/// baseline of chapter 23.3's comparison of exact-path variants.
///
/// # Errors
///
/// [`GenError::Generator`] if a derivation or the emission fails.
pub fn generate_plain() -> Result<Vec<u8>, GenError> {
    let mut derivations = Vec::new();
    for p in &PREDICATES {
        derivations.push(derivation(p)?);
    }
    let refs: Vec<&Derivation> = derivations.iter().collect();
    variant_module(&refs, PLAIN_GENERATOR, Style::Plain)
        .map(String::into_bytes)
        .map_err(|e| GenError::generator(PLAIN_GENERATOR, e.0))
}

/// The name of the generator of the width-aware limb variant (chapter 23.3; piece G42).
pub const LIMBS_GENERATOR: &str = "predicate-variant-limbs";

/// Where the limb variant is written: beside the predicate modules, whose helpers it shares.
pub const LIMBS_OUTPUT: &str = "projects/engines/geometry/crates/geom-num/src/generated/limbs.rs";

/// Generator `predicate-variant-limbs`: every predicate's exact path in width-aware 32-bit limbs,
/// with the bounded proofs of each kind of limb step.
///
/// # Errors
///
/// [`GenError::Generator`] if a derivation or the emission fails.
pub fn generate_limbs() -> Result<Vec<u8>, GenError> {
    let mut derivations = Vec::new();
    for p in &PREDICATES {
        derivations.push(derivation(p)?);
    }
    let refs: Vec<&Derivation> = derivations.iter().collect();
    variant_module(&refs, LIMBS_GENERATOR, Style::Limbs)
        .map(String::into_bytes)
        .map_err(|e| GenError::generator(LIMBS_GENERATOR, e.0))
}

/// Generator `verus-ssi-weight`: `ssi_weight`'s exact path as a Verus-annotated twin (piece
/// G39).
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_verus_ssi_weight() -> Result<Vec<u8>, GenError> {
    twin_of("ssi_weight")
}

/// Generator `verus-ssi-side`: `ssi_side`'s exact path as a Verus-annotated twin (piece G39).
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_verus_ssi_side() -> Result<Vec<u8>, GenError> {
    twin_of("ssi_side")
}

/// Generator `verus-plane-side`: `plane_side`'s exact path as a Verus-annotated twin
/// (piece G39).
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_verus_plane_side() -> Result<Vec<u8>, GenError> {
    twin_of("plane_side")
}

/// Generator `verus-tpi-weight`: `tpi_weight`'s exact path as a Verus-annotated twin
/// (piece G39).
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_verus_tpi_weight() -> Result<Vec<u8>, GenError> {
    twin_of("tpi_weight")
}

/// Generator `verus-tpi-side`: `tpi_side`'s exact path as a Verus-annotated twin
/// (piece G39).
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_verus_tpi_side() -> Result<Vec<u8>, GenError> {
    twin_of("tpi_side")
}

/// Generator `verus-lpi-weight`: `lpi_weight`'s exact path as a Verus-annotated twin
/// (piece G39).
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_verus_lpi_weight() -> Result<Vec<u8>, GenError> {
    twin_of("lpi_weight")
}

/// Generator `verus-lpi-side`: `lpi_side`'s exact path as a Verus-annotated twin
/// (piece G39).
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_verus_lpi_side() -> Result<Vec<u8>, GenError> {
    twin_of("lpi_side")
}

/// The folder the certificates' problems are written into: geom-gen's generated folder, outside
/// every crate's sources; `kernel-certificates.yml` runs the tools on them.
pub const CERTIFICATES_DIR: &str = crate::predicate_certificates::PROBLEMS_DIR;

/// One problem of a filter certificate (chapter 28.6; piece G35), as bytes.
fn certificate(
    predicate: &str,
    generator: &'static str,
    emit: fn(&Derivation, &str) -> Result<String, EmitError>,
) -> Result<Vec<u8>, GenError> {
    let d = derivation(named(predicate)?)?;
    emit(&d, generator)
        .map(String::into_bytes)
        .map_err(|e| GenError::generator(generator, e.0))
}

/// Generator `gappa-orient2d`: orient2d's filter for Gappa, whose proof Rocq checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_orient2d() -> Result<Vec<u8>, GenError> {
    certificate("orient2d", "gappa-orient2d", gappa_problem)
}

/// Generator `gappa-orient3d`: orient3d's filter for Gappa, whose proof Rocq checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_orient3d() -> Result<Vec<u8>, GenError> {
    certificate("orient3d", "gappa-orient3d", gappa_problem)
}

/// One case of a filter certificate whose scale has several monomials (piece G40), as bytes: the
/// case `case` names, one of [`crate::predicate_certificates::gappa_cases`]'s.
fn case_certificate(
    predicate: &str,
    generator: &'static str,
    case: &str,
) -> Result<Vec<u8>, GenError> {
    let d = derivation(named(predicate)?)?;
    gappa_case_problem(&d, generator, case)
        .map(String::into_bytes)
        .map_err(|e| GenError::generator(generator, e.0))
}

/// Generator `gappa-incircle-x`: incircle's filter for Gappa in the case where `m_x` is the
/// largest maximum.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_incircle_x() -> Result<Vec<u8>, GenError> {
    case_certificate("incircle", "gappa-incircle-x", "x")
}

/// Generator `gappa-incircle-y`: incircle's filter for Gappa in the case where `m_y` is the
/// largest maximum.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_incircle_y() -> Result<Vec<u8>, GenError> {
    case_certificate("incircle", "gappa-incircle-y", "y")
}

/// Generator `gappa-insphere-x`: insphere's filter for Gappa in the case where `m_x` is the
/// largest maximum.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_insphere_x() -> Result<Vec<u8>, GenError> {
    case_certificate("insphere", "gappa-insphere-x", "x")
}

/// Generator `gappa-insphere-y`: insphere's filter for Gappa in the case where `m_y` is the
/// largest maximum.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_insphere_y() -> Result<Vec<u8>, GenError> {
    case_certificate("insphere", "gappa-insphere-y", "y")
}

/// Generator `gappa-insphere-z`: insphere's filter for Gappa in the case where `m_z` is the
/// largest maximum.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_insphere_z() -> Result<Vec<u8>, GenError> {
    case_certificate("insphere", "gappa-insphere-z", "z")
}

/// Generator `gappa-plane-side-d`: `plane_side`'s filter for Gappa in the case where
/// `m_d` is the largest term of the scale.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_plane_side_d() -> Result<Vec<u8>, GenError> {
    case_certificate("plane_side", "gappa-plane-side-d", "d")
}

/// Generator `gappa-plane-side-z-n`: `plane_side`'s filter for Gappa in the case where
/// `m_z * m_n` is the largest term of the scale.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_plane_side_z_n() -> Result<Vec<u8>, GenError> {
    case_certificate("plane_side", "gappa-plane-side-z-n", "z-n")
}

/// Generator `gappa-plane-side-y-n`: `plane_side`'s filter for Gappa in the case where
/// `m_y * m_n` is the largest term of the scale.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_plane_side_y_n() -> Result<Vec<u8>, GenError> {
    case_certificate("plane_side", "gappa-plane-side-y-n", "y-n")
}

/// Generator `gappa-plane-side-x-n`: `plane_side`'s filter for Gappa in the case where
/// `m_x * m_n` is the largest term of the scale.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_plane_side_x_n() -> Result<Vec<u8>, GenError> {
    case_certificate("plane_side", "gappa-plane-side-x-n", "x-n")
}

/// Generator `gappa-tpi-weight`: `tpi_weight`'s filter for Gappa, whose proof Rocq checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_tpi_weight() -> Result<Vec<u8>, GenError> {
    certificate("tpi_weight", "gappa-tpi-weight", gappa_problem)
}

/// Generator `gappa-ssi-weight`: `ssi_weight`'s filter for Gappa, whose proof Rocq checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_ssi_weight() -> Result<Vec<u8>, GenError> {
    certificate("ssi_weight", "gappa-ssi-weight", gappa_problem)
}

/// Generator `gappa-ssi-side`: `ssi_side`'s filter for Gappa, whose proof Rocq checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_ssi_side() -> Result<Vec<u8>, GenError> {
    certificate("ssi_side", "gappa-ssi-side", gappa_problem)
}

/// Generator `gappa-lpi-weight-z-n`: `lpi_weight`'s filter for Gappa in the case where `m_z * m_n` is
/// the largest term of its scale.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_lpi_weight_z_n() -> Result<Vec<u8>, GenError> {
    case_certificate("lpi_weight", "gappa-lpi-weight-z-n", "z-n")
}

/// Generator `gappa-lpi-weight-y-n`: `lpi_weight`'s filter for Gappa in the case where `m_y * m_n` is
/// the largest term of its scale.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_lpi_weight_y_n() -> Result<Vec<u8>, GenError> {
    case_certificate("lpi_weight", "gappa-lpi-weight-y-n", "y-n")
}

/// Generator `gappa-lpi-weight-x-n`: `lpi_weight`'s filter for Gappa in the case where `m_x * m_n` is
/// the largest term of its scale.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_gappa_lpi_weight_x_n() -> Result<Vec<u8>, GenError> {
    case_certificate("lpi_weight", "gappa-lpi-weight-x-n", "x-n")
}

/// Generator `fptaylor-orient2d`: orient2d's normalised filter for `FPTaylor`, whose certificate
/// HOL Light checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_fptaylor_orient2d() -> Result<Vec<u8>, GenError> {
    certificate("orient2d", "fptaylor-orient2d", fptaylor_problem)
}

/// Generator `fptaylor-orient3d`: orient3d's normalised filter for `FPTaylor`, whose certificate
/// HOL Light checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_fptaylor_orient3d() -> Result<Vec<u8>, GenError> {
    certificate("orient3d", "fptaylor-orient3d", fptaylor_problem)
}

/// Generator `fptaylor-incircle`: incircle's normalised filter for `FPTaylor`, whose certificate
/// HOL Light checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_fptaylor_incircle() -> Result<Vec<u8>, GenError> {
    certificate("incircle", "fptaylor-incircle", fptaylor_problem)
}

/// Generator `fptaylor-insphere`: insphere's normalised filter for `FPTaylor`, whose certificate
/// HOL Light checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_fptaylor_insphere() -> Result<Vec<u8>, GenError> {
    certificate("insphere", "fptaylor-insphere", fptaylor_problem)
}

/// Generator `fptaylor-plane-side`: `plane_side`'s normalised filter for `FPTaylor`, whose certificate
/// HOL Light checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_fptaylor_plane_side() -> Result<Vec<u8>, GenError> {
    certificate("plane_side", "fptaylor-plane-side", fptaylor_problem)
}

/// Generator `fptaylor-tpi-weight`: `tpi_weight`'s normalised filter for `FPTaylor`, whose certificate
/// HOL Light checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_fptaylor_tpi_weight() -> Result<Vec<u8>, GenError> {
    certificate("tpi_weight", "fptaylor-tpi-weight", fptaylor_problem)
}

/// Generator `fptaylor-tpi-side`: `tpi_side`'s normalised filter for `FPTaylor`, whose certificate
/// HOL Light checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_fptaylor_tpi_side() -> Result<Vec<u8>, GenError> {
    certificate("tpi_side", "fptaylor-tpi-side", fptaylor_problem)
}

/// Generator `fptaylor-lpi-weight`: `lpi_weight`'s normalised filter for `FPTaylor`, whose certificate
/// HOL Light checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_fptaylor_lpi_weight() -> Result<Vec<u8>, GenError> {
    certificate("lpi_weight", "fptaylor-lpi-weight", fptaylor_problem)
}

/// Generator `fptaylor-lpi-side`: `lpi_side`'s normalised filter for `FPTaylor`, whose certificate
/// HOL Light checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_fptaylor_lpi_side() -> Result<Vec<u8>, GenError> {
    certificate("lpi_side", "fptaylor-lpi-side", fptaylor_problem)
}

/// Generator `fptaylor-ssi-weight`: `ssi_weight`'s normalised filter for `FPTaylor`, whose certificate
/// HOL Light checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_fptaylor_ssi_weight() -> Result<Vec<u8>, GenError> {
    certificate("ssi_weight", "fptaylor-ssi-weight", fptaylor_problem)
}

/// Generator `fptaylor-ssi-side`: `ssi_side`'s normalised filter for `FPTaylor`, whose certificate
/// HOL Light checks.
///
/// # Errors
///
/// [`GenError::Generator`] if the derivation or the emission fails.
pub fn generate_fptaylor_ssi_side() -> Result<Vec<u8>, GenError> {
    certificate("ssi_side", "fptaylor-ssi-side", fptaylor_problem)
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
    const COMMITTED: [(&str, Generate, &[u8]); 12] = [
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
        (
            "tpi_weight.rs",
            generate_tpi_weight,
            include_bytes!("../../geom-num/src/generated/tpi_weight.rs"),
        ),
        (
            "tpi_side.rs",
            generate_tpi_side,
            include_bytes!("../../geom-num/src/generated/tpi_side.rs"),
        ),
        (
            "lpi_weight.rs",
            generate_lpi_weight,
            include_bytes!("../../geom-num/src/generated/lpi_weight.rs"),
        ),
        (
            "lpi_side.rs",
            generate_lpi_side,
            include_bytes!("../../geom-num/src/generated/lpi_side.rs"),
        ),
        (
            "ssi_weight.rs",
            generate_ssi_weight,
            include_bytes!("../../geom-num/src/generated/ssi_weight.rs"),
        ),
        (
            "ssi_side.rs",
            generate_ssi_side,
            include_bytes!("../../geom-num/src/generated/ssi_side.rs"),
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
        expected.push((PLAIN_GENERATOR, PLAIN_OUTPUT));
        expected.push((LIMBS_GENERATOR, LIMBS_OUTPUT));
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
