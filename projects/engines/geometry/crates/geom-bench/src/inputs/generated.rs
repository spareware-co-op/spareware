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

use super::{Class, plane, predicate};

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
