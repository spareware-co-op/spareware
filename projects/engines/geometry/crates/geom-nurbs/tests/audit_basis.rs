//! Audit of `geom_nurbs::basis` and `geom_nurbs::curve` at the edge of the doubles: knot vectors
//! that `Basis::new` accepts (finite knots, a finite difference between the last and the first)
//! but whose span sums round past the largest double.
//!
//! The basis header guarantees that the values of `Basis::eval` lie in [0, 1] and are each within
//! γ_(6p)·N of the exact value N, and that the functions "never return a value that is not
//! finite"; the curve header guarantees a point within γ_(7p+1)·Σ N_j·|P_j| of the exact point.
//! The exact values here are computed by hand from the definition (degree 1, one span).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::many_single_char_names,
    clippy::doc_markdown,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

use geom_nurbs::{Basis, Curve, Limit};

/// 2^k as a double.
fn pow2(k: i32) -> f64 {
    2.0_f64.powi(k)
}

/// The degree-1 basis on [a, a, b, b] with a = −2^1023 and b = 2^1023 − 2^971: b − a is exactly
/// the largest double, so `Basis::new` accepts it.
fn wide_basis() -> Basis {
    let a = -pow2(1023);
    let b = pow2(1023) - pow2(971);
    assert_eq!(b - a, f64::MAX);
    Basis::new(1, vec![a, a, b, b]).unwrap()
}

/// At u = 0.6·2^971 the two knot differences round to 2^1023 + 2^971 and 2^1023 − 3·2^970, whose
/// sum is the tie 2^1024 − 2^970 and rounds to infinity. The exact basis values are
/// N_0 = (b − u)/(b − a) and N_1 = (u − a)/(b − a), both within 2^−50 of one half.
#[test]
fn basis_values_on_a_knot_range_of_the_largest_double_sum_to_one() {
    let basis = wide_basis();
    let u = 0.6 * pow2(971);
    let mut n = [0.0; 2];
    basis.eval(u, Limit::FromRight, &mut n).unwrap();
    // Each exact value is one half to within 2^−50; γ_6 is far below that.
    for (i, &v) in n.iter().enumerate() {
        assert!(
            (v - 0.5).abs() < 1e-9,
            "N_{i}({u:e}) = {v}, but the exact value is 0.5 within 2^-50"
        );
    }
}

/// The same parameter on the line from (1, 1) to (3, 3): the exact point is (2, 2) within 2^−49,
/// and every weight in the sum is positive, so the point cannot be the origin.
#[test]
fn a_line_on_a_knot_range_of_the_largest_double_evaluates_between_its_ends() {
    let curve = Curve::<2>::new(wide_basis(), &[[1.0, 1.0], [3.0, 3.0]]).unwrap();
    let u = 0.6 * pow2(971);
    let p = curve.point(u, Limit::FromRight).unwrap();
    for (d, &x) in p.iter().enumerate() {
        assert!(
            (x - 2.0).abs() < 1e-9,
            "coordinate {d} of C({u:e}) is {x}, but the exact point is (2, 2) within 2^-49"
        );
    }
}
