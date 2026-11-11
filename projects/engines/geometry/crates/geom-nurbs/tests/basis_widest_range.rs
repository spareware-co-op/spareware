//! The basis functions of `geom_nurbs::basis` on the widest knot ranges `Basis::new` accepts:
//! finite knots whose last and first differ by as much as the largest double.
//!
//! On such a range the two rounded differences that A2.2 and A2.3 add can sum past the largest
//! double though each is finite. The basis header guarantees the values keep their bound
//! γ_(6p) · N there, because the knot difference the sum stands for divides instead, and that a
//! sum that is finite is still the divisor, so no value that was right has changed. The exact
//! values come from the rational reference of `common`, and for degree 1 from the definition by
//! hand. The first two tests are those of an independent audit of the merged kernel (issue 751),
//! which found every value 0 and a line's point at the origin.
//!
//! The basis tests a sum only on a knot range of 2^1023 or more, below which none can overflow.
//! The last tests hold the values, the first derivatives and the divisor on the double below
//! that range, on it and on the double above.

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

mod common;

use common::{Lcg, cox_de_boor, derivatives, gamma, magnitudes_a23, q, within};
use geom_nurbs::{Basis, Curve, Limit};

const LIMITS: [Limit; 2] = [Limit::FromRight, Limit::FromLeft];

/// 2^k as a double, for k from −1074 to 1023, written as its bits: the exponent field for a
/// normal double, one bit of the fraction below the normal range.
fn pow2(k: i32) -> f64 {
    if k >= -1022 {
        f64::from_bits(u64::try_from(k + 1023).unwrap() << 52)
    } else {
        f64::from_bits(1_u64 << u32::try_from(k + 1074).unwrap())
    }
}

/// The ends of the widest range: a = −2^1023 and b = 2^1023 − 2^971, so b − a is exactly the
/// largest double.
fn ends() -> (f64, f64) {
    let a = -pow2(1023);
    let b = pow2(1023) - pow2(971);
    assert_eq!(b - a, f64::MAX);
    (a, b)
}

/// The basis of degree `p` on [a, b] with `interior` between its clamped ends.
fn clamped(a: f64, b: f64, p: usize, interior: &[f64]) -> Basis {
    let mut knots = vec![a; p + 1];
    knots.extend_from_slice(interior);
    knots.extend(vec![b; p + 1]);
    Basis::new(p, knots).unwrap()
}

/// The basis of degree `p` on the widest range with `interior` between its clamped ends.
fn wide(p: usize, interior: &[f64]) -> Basis {
    let (a, b) = ends();
    clamped(a, b, p, interior)
}

/// The degree-1 basis on [a, a, b, b].
fn wide_basis() -> Basis {
    wide(1, &[])
}

/// Whether the two rounded differences from `u` to the ends of the range sum past the largest
/// double: the case the knot difference is there for.
fn sum_overflows(u: f64) -> bool {
    let (a, b) = ends();
    !((b - u) + (u - a)).is_finite()
}

/// At u = 0.6·2^971 the two knot differences round to 2^1023 + 2^971 and 2^1023 − 3·2^970, whose
/// sum is the tie 2^1024 − 2^970 and rounds to infinity. The exact basis values are
/// N_0 = (b − u)/(b − a) and N_1 = (u − a)/(b − a), both within 2^−50 of one half.
#[test]
fn basis_values_on_a_knot_range_of_the_largest_double_sum_to_one() {
    let basis = wide_basis();
    let u = 0.6 * pow2(971);
    assert!(sum_overflows(u));
    let mut n = [0.0; 2];
    basis.eval(u, Limit::FromRight, &mut n).unwrap();
    // Each exact value is one half to within 2^−50; γ_6 is far below that.
    for (i, &v) in n.iter().enumerate() {
        assert!(
            (v - 0.5).abs() < 1e-9,
            "N_{i}({u:e}) = {v}, but the exact value is 0.5 within 2^-50"
        );
    }
    assert_eq!(n[0] + n[1], 1.0);
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

/// Bases on the widest range: one span of each degree, where every step divides by the whole
/// range, and bases with interior knots, where only the widest steps do.
fn wide_bases() -> Vec<Basis> {
    let (a, b) = ends();
    let mut out: Vec<Basis> = [1, 2, 3, 5, 7].iter().map(|&p| wide(p, &[])).collect();
    // The double next to each end, and the middle.
    out.push(wide(2, &[a + pow2(970)]));
    out.push(wide(2, &[b - pow2(970)]));
    out.push(wide(3, &[a + pow2(970), 0.0, b - pow2(970)]));
    out.push(wide(3, &[0.0, 0.0]));
    out
}

/// Parameters across the widest range: its ends, the audit's parameter, and random ones.
fn wide_parameters(rng: &mut Lcg) -> Vec<f64> {
    let (a, b) = ends();
    let mut out = vec![a, b, 0.0, 0.6 * pow2(971), -0.6 * pow2(971)];
    for _ in 0..40 {
        // a + (b − a)·t without overflow: t·b + (1 − t)·a, both terms inside the range.
        let t = rng.unit();
        let u = t * b + (1.0 - t) * a;
        if a <= u && u <= b {
            out.push(u);
        }
    }
    out
}

/// The header's bound for the values, γ_(6p) · N, against the exact recurrence, on every wide
/// basis; `eval_derivs` gives the same bits; and the parameters do reach the overflowing sum.
#[test]
fn values_on_the_widest_range_are_within_their_bound_of_the_recurrence() {
    let mut rng = Lcg(0x5eed_0751);
    let params = wide_parameters(&mut rng);
    let reached = params.iter().filter(|&&u| sum_overflows(u)).count();
    assert!(
        reached >= 3,
        "only {reached} parameters make the sum overflow"
    );
    for basis in wide_bases() {
        values_are_within_their_bound(&basis, &params);
    }
}

/// The values of `basis` at each parameter, from each side: in [0, 1], within γ_(6p) · N of the
/// exact recurrence, summing to one, and the same bits from `eval_derivs`.
fn values_are_within_their_bound(basis: &Basis, params: &[f64]) {
    let p = basis.degree();
    let bound_factor = gamma(6 * p);
    for &u in params {
        for limit in LIMITS {
            let mut n = vec![0.0; p + 1];
            let span = basis.eval(u, limit, &mut n).unwrap();
            let exact = cox_de_boor(basis.knots(), p, u, limit);
            let mut total = 0.0;
            for (r, &value) in n.iter().enumerate() {
                assert!((0.0..=1.0).contains(&value));
                let e = &exact[p][span - p + r];
                let bound = &bound_factor * e;
                assert!(
                    within(value, e, &bound),
                    "degree {p}, u {u:e}, function {r}: {value}"
                );
                total += value;
            }
            assert!((total - 1.0).abs() < 1e-12, "degree {p}, u {u:e}: {total}");
            let mut rows = vec![0.0; p + 1];
            let again = basis.eval_derivs(u, limit, 0, &mut rows).unwrap();
            assert_eq!((again, &rows), (span, &n), "degree {p}, u {u:e}");
        }
    }
}

/// The first derivatives on the widest range of degree 1 are −1 / (b − a) and 1 / (b − a), and
/// b − a is the largest double: both round to 2^−1024, a double below the normal range. Before
/// the fix the scheme divided by infinity and gave 0. The line from (1, 1) to (3, 3) has the
/// derivative 2 / (b − a) in each coordinate.
#[test]
fn derivatives_on_the_widest_range_divide_by_the_knot_difference() {
    let basis = wide_basis();
    let u = 0.6 * pow2(971);
    assert!(sum_overflows(u));
    let mut rows = [0.0; 4];
    basis
        .eval_derivs(u, Limit::FromRight, 1, &mut rows)
        .unwrap();
    assert_eq!(rows[2], -pow2(-1024));
    assert_eq!(rows[3], pow2(-1024));
    let mut n = [0.0; 2];
    basis.eval(u, Limit::FromRight, &mut n).unwrap();
    assert_eq!(rows[..2], n);

    let curve = Curve::<2>::new(basis, &[[1.0, 1.0], [3.0, 3.0]]).unwrap();
    let mut d = [[0.0; 2]; 2];
    curve.derivs(u, Limit::FromRight, &mut d).unwrap();
    assert_eq!(d[1], [pow2(-1023), pow2(-1023)]);
}

/// First derivatives of every wide basis against the exact derivative formula. The header's
/// bound γ_(6p+2) · M is relative, so it is for results that do not underflow, and on this range
/// a first derivative is near 2^−1024, below the normal doubles: there a quotient by a knot
/// difference and its product with a basis value each round to the doubles' spacing 2^−1074
/// (half of it at most), two such terms are added and the factor p multiplies them, so 4·p
/// spacings beside the relative bound hold every step's rounding.
#[test]
fn first_derivatives_on_the_widest_range_are_within_their_bound() {
    let mut rng = Lcg(0x5eed_0753);
    let params = wide_parameters(&mut rng);
    for basis in wide_bases() {
        first_derivatives_are_within_their_bound(&basis, &params);
    }
}

/// The first derivatives of `basis` at each parameter, from each side, within γ_(6p+2) · M and
/// 4·p spacings of the exact derivative formula.
fn first_derivatives_are_within_their_bound(basis: &Basis, params: &[f64]) {
    let p = basis.degree();
    let bound_factor = gamma(6 * p + 2);
    let underflow = q(pow2(-1074)) * q((4 * p) as f64);
    for &u in params {
        for limit in LIMITS {
            let mut rows = vec![0.0; 2 * (p + 1)];
            let span = basis.eval_derivs(u, limit, 1, &mut rows).unwrap();
            let exact = derivatives(basis.knots(), p, u, limit, 1);
            let mags = magnitudes_a23(basis.knots(), p, span, u, limit, 1);
            for r in 0..=p {
                let e = &exact[1][span - p + r];
                let bound = &bound_factor * &mags[1][r] + &underflow;
                assert!(
                    within(rows[p + 1 + r], e, &bound),
                    "degree {p}, u {u:e}, function {r}: {:e}",
                    rows[p + 1 + r]
                );
            }
        }
    }
}

/// Nothing changes where the sum is finite: the divisor is still the sum of the two rounded
/// differences, as A2.2 adds them, and not the knot difference. On [0.1, 1.3] at 0.2 the two
/// differ in their last bit, and so do the quotients, so this pins which one divides.
#[test]
fn a_finite_sum_still_divides() {
    let (a, b, u) = (0.1, 1.3, 0.2);
    let (r, le) = (b - u, u - a);
    let sum = r + le;
    assert_ne!(sum, b - a);
    assert_ne!(r / sum, r / (b - a));
    let basis = Basis::new(1, vec![a, a, b, b]).unwrap();
    let mut n = [0.0; 2];
    basis.eval(u, Limit::FromRight, &mut n).unwrap();
    assert_eq!(n, [r / sum, le / sum]);
    let mut rows = [0.0; 4];
    basis
        .eval_derivs(u, Limit::FromRight, 1, &mut rows)
        .unwrap();
    assert_eq!(rows, [r / sum, le / sum, -1.0 / sum, 1.0 / sum]);
}

/// The same on the widest range, where every sum is tested: one that is finite still divides,
/// and the knot difference does not take its place. At 2^970 the difference to the first knot
/// rounds down and the sum with it, to the double below the knot difference, so the first
/// quotient differs by which one divides.
#[test]
fn a_finite_sum_on_the_widest_range_still_divides() {
    let (a, b) = ends();
    let u = pow2(970);
    let (r, le) = (b - u, u - a);
    let sum = r + le;
    assert!(sum.is_finite());
    assert_ne!(sum, b - a);
    assert_ne!(r / sum, r / (b - a));
    let basis = wide_basis();
    let mut n = [0.0; 2];
    basis.eval(u, Limit::FromRight, &mut n).unwrap();
    assert_eq!(n, [r / sum, le / sum]);
    let mut rows = [0.0; 4];
    basis
        .eval_derivs(u, Limit::FromRight, 1, &mut rows)
        .unwrap();
    assert_eq!(rows, [r / sum, le / sum, -1.0 / sum, 1.0 / sum]);
}

/// The basis tests the sum of a step's two differences only on a knot range of 2^1023 or more,
/// because below that no sum can overflow. These are the ends [a, b] and a parameter on the
/// double below that range, on it and on the double above. At each parameter the sum of the two
/// rounded differences is finite and is not the rounded knot difference, so a degree-1 basis
/// tells which of the two divided.
fn about_the_cut() -> [(f64, f64, f64); 3] {
    let cut = pow2(1023);
    let cases = [
        (
            0xFFC0_0000_0000_0000_u64,
            0x7FD7_FFFF_FFFF_FFFF_u64,
            0x7FC8_0000_0000_0003_u64,
        ),
        (
            0xFFC0_0000_0000_0000,
            0x7FD8_0000_0000_0001,
            0xFFB0_0000_0000_0003,
        ),
        (
            0xFFC0_0000_0000_0001,
            0x7FD8_0000_0000_0001,
            0x7FC0_0000_0000_0004,
        ),
    ]
    .map(|(a, b, u)| (f64::from_bits(a), f64::from_bits(b), f64::from_bits(u)));
    let ranges = cases.map(|(a, b, _)| (b - a).to_bits());
    assert_eq!(
        ranges,
        [cut.to_bits() - 1, cut.to_bits(), cut.to_bits() + 1],
        "the double below 2^1023, 2^1023 and the double above"
    );
    cases
}

/// On each side of the range from which the sums are tested, and on it, a finite sum divides:
/// the values and first derivatives of degree 1 are the quotients by the sum, bit for bit, and
/// not those by the knot difference.
#[test]
fn a_finite_sum_divides_below_the_cut_on_it_and_above() {
    for (a, b, u) in about_the_cut() {
        let (r, le) = (b - u, u - a);
        let sum = r + le;
        assert!(sum.is_finite());
        assert_ne!(sum, b - a);
        assert_ne!(r / sum, r / (b - a));
        assert_ne!(le / sum, le / (b - a));
        let basis = clamped(a, b, 1, &[]);
        let mut n = [0.0; 2];
        basis.eval(u, Limit::FromRight, &mut n).unwrap();
        assert_eq!(n, [r / sum, le / sum], "range {:e}", b - a);
        let mut rows = [0.0; 4];
        basis
            .eval_derivs(u, Limit::FromRight, 1, &mut rows)
            .unwrap();
        assert_eq!(
            rows,
            [r / sum, le / sum, -1.0 / sum, 1.0 / sum],
            "range {:e}",
            b - a
        );
    }
}

/// The header's bounds on the same three ranges, for one span of degrees 1 to 5 and for bases
/// with interior knots: the values against the exact recurrence and the first derivatives
/// against the exact formula, at the ends, the middle, the pinned parameter and random ones.
#[test]
fn values_and_first_derivatives_about_the_cut_are_within_their_bounds() {
    let mut rng = Lcg(0x5eed_0758);
    for (a, b, pinned) in about_the_cut() {
        let mut params = vec![a, b, 0.0, pinned];
        for _ in 0..40 {
            let t = rng.unit();
            let u = t * b + (1.0 - t) * a;
            if a <= u && u <= b {
                params.push(u);
            }
        }
        assert!(params.len() > 30, "{} parameters", params.len());
        let bases = [
            clamped(a, b, 1, &[]),
            clamped(a, b, 2, &[]),
            clamped(a, b, 3, &[]),
            clamped(a, b, 5, &[]),
            clamped(a, b, 2, &[0.0]),
            clamped(a, b, 3, &[a + pow2(970), 0.0, 0.0, b - pow2(970)]),
        ];
        for basis in &bases {
            values_are_within_their_bound(basis, &params);
            first_derivatives_are_within_their_bound(basis, &params);
        }
    }
}
