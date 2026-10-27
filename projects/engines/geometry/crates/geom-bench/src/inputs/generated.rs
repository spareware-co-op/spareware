//! Inputs for the generated predicate modules of `geom_num::generated`: each predicate's `filter`
//! and `value`, on the classes of the explicit-point predicates of [`super::predicate`] and of
//! [`super::plane::side`], each benchmark on a batch of its own.
//!
//! What it does: every builder draws the batch its predicate's benchmark in
//! [`super::predicate`] (or [`super::plane::side`]) draws, from a seed named after the generated
//! function instead, so the classes mean the same and the batches differ. On typical inputs a
//! filter decides; on nearly degenerate ones it cannot, and returns `None`, leaving the exact value
//! to decide; on degenerate ones the exact value is zero; on extreme ones every intermediate is
//! near the bound its certificate states.
//!
//! Guarantees: every batch is full and the same on every target, and its class is what it claims
//! for the generated function as well (`tests/inputs.rs` checks each claim); every result is
//! checked against exact determinants by `tests/attack_g06.rs`.
//!
//! Method: the input classes of exact geometric predicates, after J. R. Shewchuk, "Adaptive
//! precision floating-point arithmetic and fast robust geometric predicates", *Discrete &
//! Computational Geometry* 18 (1997), section 6, <https://doi.org/10.1007/PL00009321>, as
//! [`super::predicate`] builds them.
//!
//! Cost: that of the builder it shares, once per batch, outside every measured run.

use geom_num::grid::{P2, P3};

use super::{Class, crossing, plane, predicate};

/// Point triples for `generated::orient2d::filter`.
#[must_use]
pub fn orient2d_filter(class: Class) -> Vec<[P2; 3]> {
    predicate::orient2d_from("generated::orient2d::filter", class)
}

/// Point triples for `generated::orient2d::value`.
#[must_use]
pub fn orient2d_value(class: Class) -> Vec<[P2; 3]> {
    predicate::orient2d_from("generated::orient2d::value", class)
}

/// Point quadruples for `generated::orient3d::filter`.
#[must_use]
pub fn orient3d_filter(class: Class) -> Vec<[P3; 4]> {
    predicate::orient3d_from("generated::orient3d::filter", class)
}

/// Point quadruples for `generated::orient3d::value`.
#[must_use]
pub fn orient3d_value(class: Class) -> Vec<[P3; 4]> {
    predicate::orient3d_from("generated::orient3d::value", class)
}

/// Point quadruples for `generated::incircle::filter`.
#[must_use]
pub fn incircle_filter(class: Class) -> Vec<[P2; 4]> {
    predicate::incircle_from("generated::incircle::filter", class)
}

/// Point quadruples for `generated::incircle::value`.
#[must_use]
pub fn incircle_value(class: Class) -> Vec<[P2; 4]> {
    predicate::incircle_from("generated::incircle::value", class)
}

/// Point quintuples for `generated::insphere::filter`.
#[must_use]
pub fn insphere_filter(class: Class) -> Vec<[P3; 5]> {
    predicate::insphere_from("generated::insphere::filter", class)
}

/// Point quintuples for `generated::insphere::value`.
#[must_use]
pub fn insphere_value(class: Class) -> Vec<[P3; 5]> {
    predicate::insphere_from("generated::insphere::value", class)
}

/// A plane's coefficients and a grid point, `([nx, ny, nz], d, p)`, from [`plane::side`]'s
/// classes drawn with the seed of `op`.
fn plane_side(op: &str, class: Class) -> Vec<([i128; 3], i128, P3)> {
    plane::side_from(op, class)
        .into_iter()
        .map(|(plane, p)| (plane.n(), plane.d(), p))
        .collect()
}

/// Coefficients and points for `generated::plane_side::filter`.
#[must_use]
pub fn plane_side_filter(class: Class) -> Vec<([i128; 3], i128, P3)> {
    plane_side("generated::plane_side::filter", class)
}

/// Coefficients and points for `generated::plane_side::value`.
#[must_use]
pub fn plane_side_value(class: Class) -> Vec<([i128; 3], i128, P3)> {
    plane_side("generated::plane_side::value", class)
}

/// A plane's coefficients: its normal and offset.
type Coefficients = ([i128; 3], i128);

/// The three normals of the meeting points in [`plane::side_of_tpi`]'s classes, drawn with the
/// seed of `op`.
fn tpi_normals(op: &str, class: Class) -> Vec<[[i128; 3]; 3]> {
    plane::side_of_tpi_from(op, class)
        .into_iter()
        .map(|(_, t)| t.planes().map(|p| p.n()))
        .collect()
}

/// The four planes' coefficients (the meeting point's three, then the classifying plane) of
/// [`plane::side_of_tpi`]'s classes, drawn with the seed of `op`.
fn tpi_four(op: &str, class: Class) -> Vec<[Coefficients; 4]> {
    plane::side_of_tpi_from(op, class)
        .into_iter()
        .map(|(s, t)| {
            let [p, q, r] = t.planes().map(|p| (p.n(), p.d()));
            [p, q, r, (s.n(), s.d())]
        })
        .collect()
}

/// The line's plane normal and its two points, from [`plane::side_of_lpi`]'s classes drawn with
/// the seed of `op`.
fn lpi_line(op: &str, class: Class) -> Vec<([i128; 3], P3, P3)> {
    plane::side_of_lpi_from(op, class)
        .into_iter()
        .map(|(_, l)| {
            let [p, q] = l.points();
            (l.plane().n(), p, q)
        })
        .collect()
}

/// Both planes' coefficients (the line's, then the classifying one) and the line's points, from
/// [`plane::side_of_lpi`]'s classes drawn with the seed of `op`.
fn lpi_two(op: &str, class: Class) -> Vec<(Coefficients, Coefficients, P3, P3)> {
    plane::side_of_lpi_from(op, class)
        .into_iter()
        .map(|(s, l)| {
            let [p, q] = l.points();
            ((l.plane().n(), l.plane().d()), (s.n(), s.d()), p, q)
        })
        .collect()
}

/// The two lines' points, from [`crossing::ssi_orient2d`]'s classes drawn with the seed of `op`.
fn ssi_lines(op: &str, class: Class) -> Vec<[P2; 4]> {
    crossing::ssi_orient2d_from(op, class)
        .into_iter()
        .map(|(s, _, _)| s.points())
        .collect()
}

/// The two lines' points and the classifying line's, from [`crossing::ssi_orient2d`]'s classes
/// drawn with the seed of `op`.
fn ssi_three(op: &str, class: Class) -> Vec<([P2; 4], P2, P2)> {
    crossing::ssi_orient2d_from(op, class)
        .into_iter()
        .map(|(s, p, q)| (s.points(), p, q))
        .collect()
}

/// Inputs for `generated::tpi_weight::filter`.
#[must_use]
pub fn tpi_weight_filter(class: Class) -> Vec<[[i128; 3]; 3]> {
    tpi_normals("generated::tpi_weight::filter", class)
}

/// Inputs for `generated::tpi_weight::value`.
#[must_use]
pub fn tpi_weight_value(class: Class) -> Vec<[[i128; 3]; 3]> {
    tpi_normals("generated::tpi_weight::value", class)
}

/// Inputs for `generated::tpi_side::filter`.
#[must_use]
pub fn tpi_side_filter(class: Class) -> Vec<[Coefficients; 4]> {
    tpi_four("generated::tpi_side::filter", class)
}

/// Inputs for `generated::tpi_side::value`.
#[must_use]
pub fn tpi_side_value(class: Class) -> Vec<[Coefficients; 4]> {
    tpi_four("generated::tpi_side::value", class)
}

/// Inputs for `generated::lpi_weight::filter`.
#[must_use]
pub fn lpi_weight_filter(class: Class) -> Vec<([i128; 3], P3, P3)> {
    lpi_line("generated::lpi_weight::filter", class)
}

/// Inputs for `generated::lpi_weight::value`.
#[must_use]
pub fn lpi_weight_value(class: Class) -> Vec<([i128; 3], P3, P3)> {
    lpi_line("generated::lpi_weight::value", class)
}

/// Inputs for `generated::lpi_side::filter`.
#[must_use]
pub fn lpi_side_filter(class: Class) -> Vec<(Coefficients, Coefficients, P3, P3)> {
    lpi_two("generated::lpi_side::filter", class)
}

/// Inputs for `generated::lpi_side::value`.
#[must_use]
pub fn lpi_side_value(class: Class) -> Vec<(Coefficients, Coefficients, P3, P3)> {
    lpi_two("generated::lpi_side::value", class)
}

/// Inputs for `generated::ssi_weight::filter`.
#[must_use]
pub fn ssi_weight_filter(class: Class) -> Vec<[P2; 4]> {
    ssi_lines("generated::ssi_weight::filter", class)
}

/// Inputs for `generated::ssi_weight::value`.
#[must_use]
pub fn ssi_weight_value(class: Class) -> Vec<[P2; 4]> {
    ssi_lines("generated::ssi_weight::value", class)
}

/// Inputs for `generated::ssi_side::filter`.
#[must_use]
pub fn ssi_side_filter(class: Class) -> Vec<([P2; 4], P2, P2)> {
    ssi_three("generated::ssi_side::filter", class)
}

/// Inputs for `generated::ssi_side::value`.
#[must_use]
pub fn ssi_side_value(class: Class) -> Vec<([P2; 4], P2, P2)> {
    ssi_three("generated::ssi_side::value", class)
}
