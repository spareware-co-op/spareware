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

use super::{Class, arc, carrier, crossing, plane, predicate};

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

/// Inputs for `generated::plain::orient2d_value` (piece G42): the same class and generator as
/// `generated::orient2d::value`'s, drawn under the variant's own name.
#[must_use]
pub fn orient2d_value_plain(class: Class) -> Vec<[P2; 3]> {
    predicate::orient2d_from("generated::plain::orient2d_value", class)
}

/// Inputs for `generated::plain::orient3d_value` (piece G42): the same class and generator as
/// `generated::orient3d::value`'s, drawn under the variant's own name.
#[must_use]
pub fn orient3d_value_plain(class: Class) -> Vec<[P3; 4]> {
    predicate::orient3d_from("generated::plain::orient3d_value", class)
}

/// Inputs for `generated::plain::incircle_value` (piece G42): the same class and generator as
/// `generated::incircle::value`'s, drawn under the variant's own name.
#[must_use]
pub fn incircle_value_plain(class: Class) -> Vec<[P2; 4]> {
    predicate::incircle_from("generated::plain::incircle_value", class)
}

/// Inputs for `generated::plain::insphere_value` (piece G42): the same class and generator as
/// `generated::insphere::value`'s, drawn under the variant's own name.
#[must_use]
pub fn insphere_value_plain(class: Class) -> Vec<[P3; 5]> {
    predicate::insphere_from("generated::plain::insphere_value", class)
}

/// Inputs for `generated::plain::plane_side_value` (piece G42): the same class and generator as
/// `generated::plane_side::value`'s, drawn under the variant's own name.
#[must_use]
pub fn plane_side_value_plain(class: Class) -> Vec<([i128; 3], i128, P3)> {
    plane_side("generated::plain::plane_side_value", class)
}

/// Inputs for `generated::plain::tpi_weight_value` (piece G42): the same class and generator as
/// `generated::tpi_weight::value`'s, drawn under the variant's own name.
#[must_use]
pub fn tpi_weight_value_plain(class: Class) -> Vec<[[i128; 3]; 3]> {
    tpi_normals("generated::plain::tpi_weight_value", class)
}

/// Inputs for `generated::plain::tpi_side_value` (piece G42): the same class and generator as
/// `generated::tpi_side::value`'s, drawn under the variant's own name.
#[must_use]
pub fn tpi_side_value_plain(class: Class) -> Vec<[Coefficients; 4]> {
    tpi_four("generated::plain::tpi_side_value", class)
}

/// Inputs for `generated::plain::lpi_weight_value` (piece G42): the same class and generator as
/// `generated::lpi_weight::value`'s, drawn under the variant's own name.
#[must_use]
pub fn lpi_weight_value_plain(class: Class) -> Vec<([i128; 3], P3, P3)> {
    lpi_line("generated::plain::lpi_weight_value", class)
}

/// Inputs for `generated::plain::lpi_side_value` (piece G42): the same class and generator as
/// `generated::lpi_side::value`'s, drawn under the variant's own name.
#[must_use]
pub fn lpi_side_value_plain(class: Class) -> Vec<(Coefficients, Coefficients, P3, P3)> {
    lpi_two("generated::plain::lpi_side_value", class)
}

/// Inputs for `generated::plain::ssi_weight_value` (piece G42): the same class and generator as
/// `generated::ssi_weight::value`'s, drawn under the variant's own name.
#[must_use]
pub fn ssi_weight_value_plain(class: Class) -> Vec<[P2; 4]> {
    ssi_lines("generated::plain::ssi_weight_value", class)
}

/// Inputs for `generated::plain::ssi_side_value` (piece G42): the same class and generator as
/// `generated::ssi_side::value`'s, drawn under the variant's own name.
#[must_use]
pub fn ssi_side_value_plain(class: Class) -> Vec<([P2; 4], P2, P2)> {
    ssi_three("generated::plain::ssi_side_value", class)
}

/// Inputs for `generated::limbs::orient2d_value` (piece G42): the same class and generator as
/// `generated::orient2d::value`'s, drawn under the variant's own name.
#[must_use]
pub fn orient2d_value_limbs(class: Class) -> Vec<[P2; 3]> {
    predicate::orient2d_from("generated::limbs::orient2d_value", class)
}

/// Inputs for `generated::limbs::orient3d_value` (piece G42): the same class and generator as
/// `generated::orient3d::value`'s, drawn under the variant's own name.
#[must_use]
pub fn orient3d_value_limbs(class: Class) -> Vec<[P3; 4]> {
    predicate::orient3d_from("generated::limbs::orient3d_value", class)
}

/// Inputs for `generated::limbs::incircle_value` (piece G42): the same class and generator as
/// `generated::incircle::value`'s, drawn under the variant's own name.
#[must_use]
pub fn incircle_value_limbs(class: Class) -> Vec<[P2; 4]> {
    predicate::incircle_from("generated::limbs::incircle_value", class)
}

/// Inputs for `generated::limbs::insphere_value` (piece G42): the same class and generator as
/// `generated::insphere::value`'s, drawn under the variant's own name.
#[must_use]
pub fn insphere_value_limbs(class: Class) -> Vec<[P3; 5]> {
    predicate::insphere_from("generated::limbs::insphere_value", class)
}

/// Inputs for `generated::limbs::plane_side_value` (piece G42): the same class and generator as
/// `generated::plane_side::value`'s, drawn under the variant's own name.
#[must_use]
pub fn plane_side_value_limbs(class: Class) -> Vec<([i128; 3], i128, P3)> {
    plane_side("generated::limbs::plane_side_value", class)
}

/// Inputs for `generated::limbs::tpi_weight_value` (piece G42): the same class and generator as
/// `generated::tpi_weight::value`'s, drawn under the variant's own name.
#[must_use]
pub fn tpi_weight_value_limbs(class: Class) -> Vec<[[i128; 3]; 3]> {
    tpi_normals("generated::limbs::tpi_weight_value", class)
}

/// Inputs for `generated::limbs::tpi_side_value` (piece G42): the same class and generator as
/// `generated::tpi_side::value`'s, drawn under the variant's own name.
#[must_use]
pub fn tpi_side_value_limbs(class: Class) -> Vec<[Coefficients; 4]> {
    tpi_four("generated::limbs::tpi_side_value", class)
}

/// Inputs for `generated::limbs::lpi_weight_value` (piece G42): the same class and generator as
/// `generated::lpi_weight::value`'s, drawn under the variant's own name.
#[must_use]
pub fn lpi_weight_value_limbs(class: Class) -> Vec<([i128; 3], P3, P3)> {
    lpi_line("generated::limbs::lpi_weight_value", class)
}

/// Inputs for `generated::limbs::lpi_side_value` (piece G42): the same class and generator as
/// `generated::lpi_side::value`'s, drawn under the variant's own name.
#[must_use]
pub fn lpi_side_value_limbs(class: Class) -> Vec<(Coefficients, Coefficients, P3, P3)> {
    lpi_two("generated::limbs::lpi_side_value", class)
}

/// Inputs for `generated::limbs::ssi_weight_value` (piece G42): the same class and generator as
/// `generated::ssi_weight::value`'s, drawn under the variant's own name.
#[must_use]
pub fn ssi_weight_value_limbs(class: Class) -> Vec<[P2; 4]> {
    ssi_lines("generated::limbs::ssi_weight_value", class)
}

/// Inputs for `generated::limbs::ssi_side_value` (piece G42): the same class and generator as
/// `generated::ssi_side::value`'s, drawn under the variant's own name.
#[must_use]
pub fn ssi_side_value_limbs(class: Class) -> Vec<([P2; 4], P2, P2)> {
    ssi_three("generated::limbs::ssi_side_value", class)
}

/// Inputs for `avnaim::orient2d` (piece G42): the class and generator of
/// `generated::orient2d::value`'s, drawn under the method's own name.
#[must_use]
pub fn orient2d_avnaim(class: Class) -> Vec<[P2; 3]> {
    predicate::orient2d_from("avnaim::orient2d", class)
}

/// Inputs for `avnaim::orient3d` (piece G42): the class and generator of
/// `generated::orient3d::value`'s, drawn under the method's own name.
#[must_use]
pub fn orient3d_avnaim(class: Class) -> Vec<[P3; 4]> {
    predicate::orient3d_from("avnaim::orient3d", class)
}

/// A carrier's a, b and c.
fn line(l: &geom_num::carrier::Carrier) -> [i128; 3] {
    let ([a, b], c) = l.coefficients();
    [a, b, c]
}

/// A rational point's X, Y and W.
fn point(p: &geom_num::carrier::Rational2) -> [i128; 3] {
    let ([x, y], w) = p.coordinates();
    [x, y, w]
}

/// [`carrier::side_from`]'s batch as the predicate's integers.
fn carrier_side(op: &str, class: Class) -> Vec<([i128; 3], [i128; 3])> {
    carrier::side_from(op, class)
        .iter()
        .map(|(l, p)| (line(l), point(p)))
        .collect()
}

/// [`carrier::order_from`]'s batch as the predicate's integers.
fn carrier_order(op: &str, class: Class) -> Vec<([i128; 2], [i128; 3], [i128; 3])> {
    carrier::order_from(op, class)
        .iter()
        .map(|(l, p, q)| {
            let [a, b, _] = line(l);
            ([a, b], point(p), point(q))
        })
        .collect()
}

/// [`carrier::cmp_from`]'s batch as the predicate's integers: the two x coordinates and weights.
fn rational_cmp(op: &str, class: Class) -> Vec<[i128; 4]> {
    carrier::cmp_from(op, class)
        .iter()
        .map(|(p, q)| {
            let ([xp, _], wp) = p.coordinates();
            let ([xq, _], wq) = q.coordinates();
            [xp, xq, wp, wq]
        })
        .collect()
}

/// [`carrier::orient_from`]'s batch as the predicate's integers.
fn rational_orient(op: &str, class: Class) -> Vec<[[i128; 3]; 3]> {
    carrier::orient_from(op, class)
        .iter()
        .map(|t| t.each_ref().map(point))
        .collect()
}

/// Inputs for `generated::carrier_side::filter`.
#[must_use]
pub fn carrier_side_filter(class: Class) -> Vec<([i128; 3], [i128; 3])> {
    carrier_side("generated::carrier_side::filter", class)
}

/// Inputs for `generated::carrier_side::value`.
#[must_use]
pub fn carrier_side_value(class: Class) -> Vec<([i128; 3], [i128; 3])> {
    carrier_side("generated::carrier_side::value", class)
}

/// Inputs for `generated::plain::carrier_side_value`.
#[must_use]
pub fn carrier_side_value_plain(class: Class) -> Vec<([i128; 3], [i128; 3])> {
    carrier_side("generated::plain::carrier_side_value", class)
}

/// Inputs for `generated::limbs::carrier_side_value`.
#[must_use]
pub fn carrier_side_value_limbs(class: Class) -> Vec<([i128; 3], [i128; 3])> {
    carrier_side("generated::limbs::carrier_side_value", class)
}

/// Inputs for `generated::carrier_order::filter`.
#[must_use]
pub fn carrier_order_filter(class: Class) -> Vec<([i128; 2], [i128; 3], [i128; 3])> {
    carrier_order("generated::carrier_order::filter", class)
}

/// Inputs for `generated::carrier_order::value`.
#[must_use]
pub fn carrier_order_value(class: Class) -> Vec<([i128; 2], [i128; 3], [i128; 3])> {
    carrier_order("generated::carrier_order::value", class)
}

/// Inputs for `generated::plain::carrier_order_value`.
#[must_use]
pub fn carrier_order_value_plain(class: Class) -> Vec<([i128; 2], [i128; 3], [i128; 3])> {
    carrier_order("generated::plain::carrier_order_value", class)
}

/// Inputs for `generated::limbs::carrier_order_value`.
#[must_use]
pub fn carrier_order_value_limbs(class: Class) -> Vec<([i128; 2], [i128; 3], [i128; 3])> {
    carrier_order("generated::limbs::carrier_order_value", class)
}

/// Inputs for `generated::rational_cmp::filter`.
#[must_use]
pub fn rational_cmp_filter(class: Class) -> Vec<[i128; 4]> {
    rational_cmp("generated::rational_cmp::filter", class)
}

/// Inputs for `generated::rational_cmp::value`.
#[must_use]
pub fn rational_cmp_value(class: Class) -> Vec<[i128; 4]> {
    rational_cmp("generated::rational_cmp::value", class)
}

/// Inputs for `generated::plain::rational_cmp_value`.
#[must_use]
pub fn rational_cmp_value_plain(class: Class) -> Vec<[i128; 4]> {
    rational_cmp("generated::plain::rational_cmp_value", class)
}

/// Inputs for `generated::limbs::rational_cmp_value`.
#[must_use]
pub fn rational_cmp_value_limbs(class: Class) -> Vec<[i128; 4]> {
    rational_cmp("generated::limbs::rational_cmp_value", class)
}

/// Inputs for `generated::rational_orient::filter`.
#[must_use]
pub fn rational_orient_filter(class: Class) -> Vec<[[i128; 3]; 3]> {
    rational_orient("generated::rational_orient::filter", class)
}

/// Inputs for `generated::rational_orient::value`.
#[must_use]
pub fn rational_orient_value(class: Class) -> Vec<[[i128; 3]; 3]> {
    rational_orient("generated::rational_orient::value", class)
}

/// Inputs for `generated::plain::rational_orient_value`.
#[must_use]
pub fn rational_orient_value_plain(class: Class) -> Vec<[[i128; 3]; 3]> {
    rational_orient("generated::plain::rational_orient_value", class)
}

/// Inputs for `generated::limbs::rational_orient_value`.
#[must_use]
pub fn rational_orient_value_limbs(class: Class) -> Vec<[[i128; 3]; 3]> {
    rational_orient("generated::limbs::rational_orient_value", class)
}

/// A rational centre's X, Y and W.
fn centre(r: &geom_num::arc::Round) -> [i128; 3] {
    point(&r.centre())
}

/// [`arc::round_grid_from`]'s batch as the predicate's integers.
fn round_grid(op: &str, class: Class) -> Vec<(P2, P2, [i128; 3])> {
    arc::round_grid_from(op, class)
        .iter()
        .map(|(r, z)| (*z, r.on(), centre(r)))
        .collect()
}

/// [`arc::round_side_from`]'s batch as the predicate's integers.
fn round_rational(op: &str, class: Class) -> Vec<([i128; 3], P2, [i128; 3])> {
    arc::round_side_from(op, class)
        .iter()
        .map(|(r, z)| (point(z), r.on(), centre(r)))
        .collect()
}

/// [`arc::round_meets_from`]'s batch as the predicate's integers.
fn round_line(op: &str, class: Class) -> Vec<([i128; 3], P2, [i128; 3])> {
    arc::round_meets_from(op, class)
        .iter()
        .map(|(r, l)| (line(l), r.on(), centre(r)))
        .collect()
}

/// [`arc::circle_side_from`]'s batch as the predicate's integers.
fn circle_side(op: &str, class: Class) -> Vec<([i128; 3], P2, i128)> {
    arc::circle_side_from(op, class)
        .iter()
        .map(|(c, z)| (point(z), c.centre(), c.radius()))
        .collect()
}

/// [`arc::circle_meets_from`]'s batch as the predicate's integers.
fn circle_line(op: &str, class: Class) -> Vec<([i128; 3], P2, i128)> {
    arc::circle_meets_from(op, class)
        .iter()
        .map(|(c, l)| (line(l), c.centre(), c.radius()))
        .collect()
}

/// [`arc::gap_from`]'s batch.
fn circle_gap(op: &str, class: Class) -> Vec<(P2, P2, i128)> {
    arc::gap_from(op, class)
}

/// Inputs for `generated::round_grid::filter`.
#[must_use]
pub fn round_grid_filter(class: Class) -> Vec<(P2, P2, [i128; 3])> {
    round_grid("generated::round_grid::filter", class)
}

/// Inputs for `generated::round_grid::value`.
#[must_use]
pub fn round_grid_value(class: Class) -> Vec<(P2, P2, [i128; 3])> {
    round_grid("generated::round_grid::value", class)
}

/// Inputs for `generated::plain::round_grid_value`.
#[must_use]
pub fn round_grid_value_plain(class: Class) -> Vec<(P2, P2, [i128; 3])> {
    round_grid("generated::plain::round_grid_value", class)
}

/// Inputs for `generated::limbs::round_grid_value`.
#[must_use]
pub fn round_grid_value_limbs(class: Class) -> Vec<(P2, P2, [i128; 3])> {
    round_grid("generated::limbs::round_grid_value", class)
}

/// Inputs for `generated::round_rational::filter`.
#[must_use]
pub fn round_rational_filter(class: Class) -> Vec<([i128; 3], P2, [i128; 3])> {
    round_rational("generated::round_rational::filter", class)
}

/// Inputs for `generated::round_rational::value`.
#[must_use]
pub fn round_rational_value(class: Class) -> Vec<([i128; 3], P2, [i128; 3])> {
    round_rational("generated::round_rational::value", class)
}

/// Inputs for `generated::plain::round_rational_value`.
#[must_use]
pub fn round_rational_value_plain(class: Class) -> Vec<([i128; 3], P2, [i128; 3])> {
    round_rational("generated::plain::round_rational_value", class)
}

/// Inputs for `generated::limbs::round_rational_value`.
#[must_use]
pub fn round_rational_value_limbs(class: Class) -> Vec<([i128; 3], P2, [i128; 3])> {
    round_rational("generated::limbs::round_rational_value", class)
}

/// Inputs for `generated::round_line::filter`.
#[must_use]
pub fn round_line_filter(class: Class) -> Vec<([i128; 3], P2, [i128; 3])> {
    round_line("generated::round_line::filter", class)
}

/// Inputs for `generated::round_line::value`.
#[must_use]
pub fn round_line_value(class: Class) -> Vec<([i128; 3], P2, [i128; 3])> {
    round_line("generated::round_line::value", class)
}

/// Inputs for `generated::plain::round_line_value`.
#[must_use]
pub fn round_line_value_plain(class: Class) -> Vec<([i128; 3], P2, [i128; 3])> {
    round_line("generated::plain::round_line_value", class)
}

/// Inputs for `generated::limbs::round_line_value`.
#[must_use]
pub fn round_line_value_limbs(class: Class) -> Vec<([i128; 3], P2, [i128; 3])> {
    round_line("generated::limbs::round_line_value", class)
}

/// Inputs for `generated::circle_side::filter`.
#[must_use]
pub fn circle_side_filter(class: Class) -> Vec<([i128; 3], P2, i128)> {
    circle_side("generated::circle_side::filter", class)
}

/// Inputs for `generated::circle_side::value`.
#[must_use]
pub fn circle_side_value(class: Class) -> Vec<([i128; 3], P2, i128)> {
    circle_side("generated::circle_side::value", class)
}

/// Inputs for `generated::plain::circle_side_value`.
#[must_use]
pub fn circle_side_value_plain(class: Class) -> Vec<([i128; 3], P2, i128)> {
    circle_side("generated::plain::circle_side_value", class)
}

/// Inputs for `generated::limbs::circle_side_value`.
#[must_use]
pub fn circle_side_value_limbs(class: Class) -> Vec<([i128; 3], P2, i128)> {
    circle_side("generated::limbs::circle_side_value", class)
}

/// Inputs for `generated::circle_line::filter`.
#[must_use]
pub fn circle_line_filter(class: Class) -> Vec<([i128; 3], P2, i128)> {
    circle_line("generated::circle_line::filter", class)
}

/// Inputs for `generated::circle_line::value`.
#[must_use]
pub fn circle_line_value(class: Class) -> Vec<([i128; 3], P2, i128)> {
    circle_line("generated::circle_line::value", class)
}

/// Inputs for `generated::plain::circle_line_value`.
#[must_use]
pub fn circle_line_value_plain(class: Class) -> Vec<([i128; 3], P2, i128)> {
    circle_line("generated::plain::circle_line_value", class)
}

/// Inputs for `generated::limbs::circle_line_value`.
#[must_use]
pub fn circle_line_value_limbs(class: Class) -> Vec<([i128; 3], P2, i128)> {
    circle_line("generated::limbs::circle_line_value", class)
}

/// Inputs for `generated::circle_gap::filter`.
#[must_use]
pub fn circle_gap_filter(class: Class) -> Vec<(P2, P2, i128)> {
    circle_gap("generated::circle_gap::filter", class)
}

/// Inputs for `generated::circle_gap::value`.
#[must_use]
pub fn circle_gap_value(class: Class) -> Vec<(P2, P2, i128)> {
    circle_gap("generated::circle_gap::value", class)
}

/// Inputs for `generated::plain::circle_gap_value`.
#[must_use]
pub fn circle_gap_value_plain(class: Class) -> Vec<(P2, P2, i128)> {
    circle_gap("generated::plain::circle_gap_value", class)
}

/// Inputs for `generated::limbs::circle_gap_value`.
#[must_use]
pub fn circle_gap_value_limbs(class: Class) -> Vec<(P2, P2, i128)> {
    circle_gap("generated::limbs::circle_gap_value", class)
}
