//! Attack tests from SMLib's confirmed defects, batch 2: `geom-nurbs`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: the right result (compared with a direct evaluation of
//! the curve or surface), or a defined refusal, and never a panic, a hang, a NaN or a wrong value.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::needless_range_loop,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::manual_midpoint,
    clippy::type_complexity,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::many_single_char_names,
    clippy::range_minus_one,
    reason = "tests: native arithmetic, casts and indexing, platform maths on an independent oracle (the formula as it is written), prose that names defect files, and a failure stops the test"
)]

use geom_nurbs::{
    Basis, Curve, CurveFit, FitVersion, Limit, MAX_DEGREE, NurbsError, Parametrization,
};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 36))
}

const RIGHT: Limit = Limit::FromRight;
const LEFT: Limit = Limit::FromLeft;

/// A Bezier basis of degree p: p + 1 zeros and p + 1 ones.
fn bezier_basis(p: usize) -> Basis {
    let mut knots = vec![0.0; p + 1];
    knots.extend(vec![1.0; p + 1]);
    Basis::new(p, knots).unwrap()
}

/// A Bezier curve of degree p with control points on a wavy line, in the plane.
fn bezier_curve(p: usize) -> Curve<2> {
    let pts: Vec<[f64; 2]> = (0..=p)
        .map(|i| {
            let t = i as f64;
            [t, 3.0 * (t * 0.7).sin() + 0.25 * t]
        })
        .collect();
    Curve::new(bezier_basis(p), &pts).unwrap()
}

fn close2(a: [f64; 2], b: [f64; 2], tol: f64) -> bool {
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= tol)
}

/// solidcore plan/upstream-issues/WR-24--n-knotvectorisvalid-accepts-degenerate-vectors.md, with
/// UB-8--span-and-basis-routines-read-outside-short-knot-vectors.md and
/// UB-30--reparametrisation-writes-at-negative-indices.md (b4de2149): the validity test accepted
/// p + 1 equal knots (`[2,2,2,2]` with p = 3) and one knot with p = 0, and later routines indexed
/// such vectors as if they had a span; short vectors (p = 3 with `[0,0,0,1,1]`, or p = 20) were
/// read past their end. The inputs are the defects': ours refuses each at construction with the
/// rule it breaks, and a vector with the right count and a real span is accepted.
#[test]
fn wr_24_ub_8_ub_30_degenerate_and_short_knot_vectors_are_refused() {
    // p + 1 equal knots, p = 3: too few knots for a basis, and an empty domain if padded out.
    assert!(matches!(
        Basis::new(3, vec![2.0, 2.0, 2.0, 2.0]),
        Err(NurbsError::KnotCount {
            knots: 4,
            needed: 8
        })
    ));
    for knots in [vec![2.0; 8], vec![2.0; 9], vec![2.0; 20]] {
        assert!(
            Basis::new(3, knots.clone()).is_err(),
            "p = 3 on {} equal knots must be refused",
            knots.len()
        );
    }
    // One knot with p = 0, and p = 0 at all: there is no degree-0 basis.
    assert_eq!(
        Basis::new(0, vec![1.0]),
        Err(NurbsError::Degree { degree: 0 })
    );
    assert_eq!(
        Basis::new(0, vec![0.0, 1.0]),
        Err(NurbsError::Degree { degree: 0 })
    );
    // The short vectors: p = 3 with five knots, p = 20 with 21, and a record with fewer knots than
    // its degree (UB-30's m < p).
    assert!(matches!(
        Basis::new(3, vec![0.0, 0.0, 0.0, 1.0, 1.0]),
        Err(NurbsError::KnotCount {
            knots: 5,
            needed: 8
        })
    ));
    assert!(matches!(
        Basis::new(20, vec![0.0; 21]),
        Err(NurbsError::KnotCount { .. })
    ));
    assert!(matches!(
        Basis::new(3, vec![0.0, 1.0]),
        Err(NurbsError::KnotCount { .. })
    ));
    assert!(matches!(
        Basis::new(3, Vec::new()),
        Err(NurbsError::KnotCount { knots: 0, .. })
    ));
    // A knot repeated more than p + 1 times, inside or at the end.
    assert!(matches!(
        Basis::new(2, vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]),
        Err(NurbsError::Multiplicity {
            index: 0,
            multiplicity: 4
        })
    ));
    // p + 1 equal knots in the middle are allowed (a jump), p + 2 are not.
    assert!(Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0]).is_ok());
    assert!(matches!(
        Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0]),
        Err(NurbsError::Multiplicity { .. })
    ));
    // The smallest valid vector has a real span.
    let b = Basis::new(3, vec![2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0]).unwrap();
    assert_eq!(b.domain(), (2.0, 3.0));
    // The degree limit: p above the maximum is refused, never indexed.
    assert!(matches!(
        Basis::new(MAX_DEGREE + 1, vec![0.0; 2 * MAX_DEGREE + 4]),
        Err(NurbsError::Degree { .. })
    ));
    assert!(matches!(
        Basis::new(usize::MAX, vec![0.0, 1.0]),
        Err(NurbsError::Degree { .. })
    ));
}

/// solidcore plan/upstream-issues/UB-7--n-basiseval-uses-an-uninitialised-span-for-an-unknown-side.md
/// with UB-10--n-basisderivsarray-walks-spans-with-no-bound.md (b4de2149): a basis evaluation with
/// the side left unknown used a span it had not set, and the derivative walk ran off a clamped
/// vector for the parameter `U[m-p]` (the end of the domain) or any parameter beyond it. The inputs
/// are the defects': an inner parameter with each side, the end of a clamped vector, and
/// parameters past either end. Ours gives the partition of unity at every parameter in the domain
/// from both sides, the one-sided values at the ends, and a refusal outside, for the degrees
/// 1, 3, 13, 20 and the limit.
#[test]
fn ub_7_ub_10_basis_at_inner_end_and_outside_parameters() {
    for p in [1usize, 3, 13, 20, MAX_DEGREE] {
        let basis = bezier_basis(p);
        let mut out = vec![0.0; p + 1];
        for u in [0.0, 1e-300, 0.25, 0.37, 0.5, 0.999_999_999, 1.0] {
            for side in [RIGHT, LEFT] {
                let span = basis.eval(u, side, &mut out).unwrap();
                assert_eq!(span, p, "one span: p = {p}, u = {u}");
                let sum: f64 = out.iter().sum();
                assert!((sum - 1.0).abs() < 1e-12, "p = {p}, u = {u}: sum {sum}");
                assert!(out.iter().all(|&n| (0.0..=1.0 + 1e-15).contains(&n)));
            }
        }
        // The end of the domain, the defect's U[m-p]: the last function is 1, the rest 0.
        basis.eval(1.0, RIGHT, &mut out).unwrap();
        assert_eq!(out[p], 1.0);
        assert!(out[..p].iter().all(|&n| n == 0.0));
        basis.eval(0.0, LEFT, &mut out).unwrap();
        assert_eq!(out[0], 1.0);
        // Outside the domain, either side, and NaN: refused.
        for bad in [-1e-12, 1.0 + 1e-12, 2.0, -1e300, f64::NAN, f64::INFINITY] {
            for side in [RIGHT, LEFT] {
                assert_eq!(
                    basis.eval(bad, side, &mut out),
                    Err(NurbsError::ParameterOutside),
                    "p = {p}, u = {bad}"
                );
                assert_eq!(basis.span(bad, side), Err(NurbsError::ParameterOutside));
            }
        }
        // A buffer of the wrong length is refused, not written past.
        let mut short = vec![0.0; p];
        assert!(matches!(
            basis.eval(0.5, RIGHT, &mut short),
            Err(NurbsError::BufferLength { .. })
        ));
    }
    // A multi-span clamped vector: at each interior knot the two sides pick the two spans, at the
    // end the last real span from both sides.
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0]).unwrap();
    assert_eq!(basis.span(1.0, RIGHT), Ok(4));
    assert_eq!(basis.span(1.0, LEFT), Ok(3));
    assert_eq!(basis.span(3.0, RIGHT), Ok(5));
    assert_eq!(basis.span(3.0, LEFT), Ok(5));
    assert_eq!(basis.span(0.0, RIGHT), Ok(3));
    assert_eq!(basis.span(0.0, LEFT), Ok(3));
    // Derivatives at the domain's end are finite and sum to zero, from both sides.
    let mut d = vec![0.0; 4 * 4];
    for side in [RIGHT, LEFT] {
        basis.eval_derivs(3.0, side, 3, &mut d).unwrap();
        assert!(d.iter().all(|v| v.is_finite()));
        for k in 1..4 {
            let s: f64 = d[k * 4..(k + 1) * 4].iter().sum();
            assert!(s.abs() < 1e-9, "derivative {k} sums to {s}");
        }
    }
}

/// solidcore plan/upstream-issues/UB-9--n-basisderivs-overflows-an-int.md (b4de2149): the
/// derivatives of the basis functions overflowed an integer with p >= 13 and derivative order 10
/// or more. The input is the defect's: degree 13 and orders 10 to 14. On a Bezier basis the k-th
/// derivative is known exactly: the p-th derivative of Bernstein function i is
/// (-1)^(p-i) C(p,i) p!, every derivative above order p is zero, and for k >= 1 the derivatives
/// of the basis functions sum to zero.
#[test]
fn ub_9_basis_derivatives_of_high_degree_and_order_are_exact_or_refused() {
    let p = 13usize;
    let basis = bezier_basis(p);
    let fact = (1..=p).fold(1.0_f64, |a, k| a * k as f64);
    // Pascal row p, exact in a double.
    let mut c = vec![1.0_f64; p + 1];
    for i in 1..=p {
        c[i] = c[i - 1] * (p + 1 - i) as f64 / i as f64;
    }
    for order in [10usize, 11, 12, 13, 14] {
        for u in [0.0, 0.37, 0.9, 1.0] {
            let mut out = vec![0.0; (order + 1) * (p + 1)];
            let r = basis.eval_derivs(u, RIGHT, order, &mut out);
            match r {
                Err(NurbsError::NotFinite) => continue,
                other => assert_eq!(other, Ok(p), "order {order}, u {u}"),
            }
            assert!(out.iter().all(|v| v.is_finite()));
            for k in 1..=order {
                let row = &out[k * (p + 1)..(k + 1) * (p + 1)];
                let scale: f64 = row.iter().map(|v| v.abs()).sum();
                let sum: f64 = row.iter().sum();
                assert!(
                    sum.abs() <= 1e-9 * scale.max(1.0),
                    "order {order}, k {k}, u {u}: sum {sum} of scale {scale}"
                );
                if k > p {
                    assert!(row.iter().all(|&v| v == 0.0), "k {k} above p is zero");
                }
            }
            if order >= p {
                let row = &out[p * (p + 1)..(p + 1) * (p + 1)];
                for (i, v) in row.iter().enumerate() {
                    let sign = if (p - i).is_multiple_of(2) { 1.0 } else { -1.0 };
                    let want = sign * c[i] * fact;
                    assert!(
                        (v - want).abs() <= 1e-7 * want.abs(),
                        "p-th derivative of function {i}: {v} against {want}"
                    );
                }
            }
        }
    }
}

/// solidcore plan/upstream-issues/UB-14--nl-pascaltri-18-18-is-indexed-past-its-end.md
/// (b4de2149): elevating a degree-10 Bezier by 10 indexed Pascal's table (18 rows) past its end.
/// The input is the defect's. Ours computes the triangle to the degree limit: the elevated curve
/// is the same curve within its certified deviation, and an elevation beyond the limit is refused.
#[test]
fn ub_14_elevating_a_degree_10_bezier_by_10_keeps_the_curve() {
    let c = bezier_curve(10);
    let up = c.elevate(10, &mut meter()).unwrap();
    assert_eq!(up.value.degree(), 20);
    assert_eq!(up.value.homogeneous().len(), 21);
    assert!(
        up.deviation >= 0.0 && up.deviation < 1e-6,
        "{}",
        up.deviation
    );
    for k in 0..=40 {
        let u = f64::from(k) / 40.0;
        let (a, b) = (
            c.point(u, RIGHT).unwrap(),
            up.value.point(u, RIGHT).unwrap(),
        );
        assert!(
            close2(a, b, 1e-9 + up.deviation),
            "u = {u}: {a:?} against {b:?}"
        );
    }
    // To the limit exactly, and one past it.
    let to_max = c.elevate(MAX_DEGREE - 10, &mut meter()).unwrap();
    assert_eq!(to_max.value.degree(), MAX_DEGREE);
    assert!(close2(
        c.point(0.4, RIGHT).unwrap(),
        to_max.value.point(0.4, RIGHT).unwrap(),
        1e-7 + to_max.deviation
    ));
    assert!(matches!(
        c.elevate(MAX_DEGREE - 10 + 1, &mut meter())
            .map(|r| r.value),
        Err(NurbsError::Degree { .. })
    ));
}

/// solidcore plan/upstream-issues/UB-14--nl-pascaltri-18-18-is-indexed-past-its-end.md and
/// UB-235--nlib-fitting-and-approximation-overflow-nl-index-on-huge.md (b4de2149): an elevation
/// count past the degree limit. `Curve::elevate` documents `Degree` when p + t exceeds the limit,
/// so every too-large count that does not overflow the sum is a `Degree` refusal.
#[test]
fn ub_14_elevating_by_a_huge_count_is_a_degree_refusal() {
    let c = bezier_curve(10);
    for t in [MAX_DEGREE, 1 << 40, usize::MAX - 10] {
        assert!(
            matches!(
                c.elevate(t, &mut meter()).map(|r| r.value),
                Err(NurbsError::Degree { .. })
            ),
            "t = {t}"
        );
    }
}

/// solidcore plan/upstream-issues/UB-14--nl-pascaltri-18-18-is-indexed-past-its-end.md and
/// UB-235--nlib-fitting-and-approximation-overflow-nl-index-on-huge.md (b4de2149): an elevation
/// count whose sum with the degree overflows `usize`. `Curve::elevate` documents `Degree` when
/// p + t exceeds the limit, so a count of `usize::MAX` must be a `Degree` refusal like any other
/// too-large count. It used to return `Defect`, the variant for a bug in the kernel, because the
/// sum `p + t` was a checked addition whose failure maps to `Defect`; the sum now saturates, so
/// the refusal is `Degree` naming `usize::MAX`.
#[test]
fn ub_14_elevating_by_a_count_that_overflows_the_degree_is_a_degree_refusal() {
    let c = bezier_curve(10);
    for t in [usize::MAX - 9, usize::MAX - 1, usize::MAX] {
        assert!(
            matches!(
                c.elevate(t, &mut meter()).map(|r| r.value),
                Err(NurbsError::Degree { degree: usize::MAX })
            ),
            "t = {t}"
        );
    }
}

/// solidcore plan/upstream-issues/UB-15--n-bezreducedegree-with-p-1-reads-unwritten-negative-indices.md
/// (b4de2149): reducing the degree of a degree-1 segment read indices below the array. The input
/// is the defect's. Ours refuses degree 1 with a typed error, refuses a bad tolerance, and reduces
/// a line that was elevated to degree 2 back to degree 1 within its deviation.
#[test]
fn ub_15_reducing_a_degree_1_curve_is_refused_and_an_elevated_line_comes_back() {
    let line = bezier_curve(1);
    assert!(matches!(
        line.reduce_degree(1e-6, &mut meter()).map(|r| r.value),
        Err(NurbsError::Degree { .. })
    ));
    assert_eq!(
        line.reduce_degree(-1.0, &mut meter()).map(|r| r.value),
        Err(NurbsError::Tolerance)
    );
    assert_eq!(
        line.reduce_degree(f64::NAN, &mut meter()).map(|r| r.value),
        Err(NurbsError::Tolerance)
    );
    let up = line.elevate(1, &mut meter()).unwrap().value;
    assert_eq!(up.degree(), 2);
    let down = up.reduce_degree(1e-9, &mut meter()).unwrap();
    assert_eq!(down.value.degree(), 1);
    assert!(down.deviation <= 1e-9);
    for k in 0..=20 {
        let u = f64::from(k) / 20.0;
        assert!(close2(
            line.point(u, RIGHT).unwrap(),
            down.value.point(u, RIGHT).unwrap(),
            1e-9
        ));
    }
    // A true quadratic cannot go to a line within a tiny tolerance: refused, not forced.
    let quad = bezier_curve(2);
    assert_eq!(
        quad.reduce_degree(1e-9, &mut meter()).map(|r| r.value),
        Err(NurbsError::OutsideTolerance)
    );
}

/// solidcore plan/upstream-issues/UB-29--curve-tools-trust-their-inputs.md (b4de2149): knot
/// insertion on a curve with an inner knot of multiplicity p + 1 trusted its input and walked off
/// the knot vector. The input is the defect's: a quadratic with an inner knot repeated three
/// times, a curve that jumps at that knot. Ours inserts nothing there, refuses elevation and
/// reduction with `Discontinuous`, still evaluates from each side, and cuts it into two Bezier
/// pieces.
#[test]
fn ub_29_a_curve_with_a_full_multiplicity_inner_knot_is_handled_not_trusted() {
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0]).unwrap();
    let pts = [
        [0.0, 0.0],
        [1.0, 1.0],
        [2.0, 0.0],
        [5.0, 5.0],
        [6.0, 7.0],
        [7.0, 5.0],
    ];
    let c = Curve::<2>::new(basis, &pts).unwrap();
    // The jump: from the left the curve ends at point 2, from the right it starts at point 3.
    assert!(close2(c.point(1.0, LEFT).unwrap(), pts[2], 1e-12));
    assert!(close2(c.point(1.0, RIGHT).unwrap(), pts[3], 1e-12));
    // Insertion at the knot, and anywhere else beyond the multiplicity, is refused.
    assert!(c.insert_knot(1.0, 1, &mut meter()).is_err());
    assert!(c.refine(&[1.0], &mut meter()).is_err());
    assert_eq!(
        c.insert_knot(0.5, 3, &mut meter()).map(|r| r.value),
        Err(NurbsError::TooManyKnots)
    );
    // Elevation and reduction need a continuous curve.
    assert_eq!(
        c.elevate(1, &mut meter()).map(|r| r.value),
        Err(NurbsError::Discontinuous)
    );
    assert_eq!(
        c.reduce_degree(1e-3, &mut meter()).map(|r| r.value),
        Err(NurbsError::Discontinuous)
    );
    // Insertion elsewhere is still exact.
    let fine = c.insert_knot(0.5, 1, &mut meter()).unwrap();
    for k in 0..=10 {
        let u = f64::from(k) / 10.0;
        assert!(close2(
            c.point(u, RIGHT).unwrap(),
            fine.value.point(u, RIGHT).unwrap(),
            1e-12 + fine.deviation
        ));
    }
    // Two pieces, each the Bezier of its three points.
    let pieces = c.to_bezier(&mut meter()).unwrap();
    assert_eq!(pieces.value.len(), 2);
    let end0 = pieces.value[0].basis().domain().1;
    let start1 = pieces.value[1].basis().domain().0;
    assert!(close2(
        pieces.value[0].point(end0, LEFT).unwrap(),
        pts[2],
        1e-12
    ));
    assert!(close2(
        pieces.value[1].point(start1, RIGHT).unwrap(),
        pts[3],
        1e-12
    ));
}

/// solidcore plan/upstream-issues/WR-23--sm-crvtrim-returns-an-error-flag-that-callers-read-as.md
/// (b4de2149): trimming a curve with the two ends equal returned an error flag its callers read as
/// success. The input is the defect's: `ur == ul`. Ours cuts a Bernstein piece to a sub-span and
/// refuses an empty, reversed, outside, NaN or infinite one with `KnotOutside`; a real sub-span
/// encloses the original curve's points.
#[test]
fn wr_23_cutting_a_piece_to_an_empty_or_reversed_span_is_refused() {
    let c = bezier_curve(3);
    let piece = c.bernstein(&mut meter()).unwrap().remove(0);
    for span in [
        (0.5, 0.5),
        (0.0, 0.0),
        (1.0, 1.0),
        (0.7, 0.3),
        (-0.1, 0.5),
        (0.5, 1.1),
        (f64::NAN, 0.5),
        (0.2, f64::NAN),
        (0.0, f64::INFINITY),
    ] {
        assert_eq!(
            piece.restrict(span).map(|_| ()),
            Err(NurbsError::KnotOutside),
            "{span:?}"
        );
    }
    // The whole span and a real sub-span are accepted and hold the curve's points.
    let whole = piece.restrict((0.0, 1.0)).unwrap();
    assert_eq!(whole.domain(), (0.0, 1.0));
    let cut = piece.restrict((0.25, 0.75)).unwrap();
    assert_eq!(cut.domain(), (0.25, 0.75));
    for t in [0.25, 0.4, 0.5, 0.75] {
        let want = c.point(t, RIGHT).unwrap();
        let (v, w) = cut.value_at(t).unwrap();
        assert!(w.lo() <= 1.0 && w.hi() >= 1.0, "a non-rational weight is 1");
        for i in 0..2 {
            assert!(
                v[i].lo() - 1e-12 <= want[i] && want[i] <= v[i].hi() + 1e-12,
                "t = {t}, axis {i}"
            );
        }
    }
    // A parameter outside the sub-span is refused.
    assert!(cut.value_at(0.1).is_err());
}

/// solidcore plan/upstream-issues/UB-28--curve-fitting-routines-index-outside-their-inputs.md with
/// UB-235--nlib-fitting-and-approximation-overflow-nl-index-on-huge.md (b4de2149): the fit with end
/// derivatives indexed outside its inputs for 3 points and zero derivatives, and the fits
/// overflowed on a degree of 2^63 - 1. The inputs are the defects': three points with zero end
/// derivatives, too few points for the degree, and a degree of `i64::MAX`. Ours returns a curve
/// through the data for the first, and typed refusals for the others.
#[test]
fn ub_28_ub_235_fits_with_few_points_zero_derivatives_and_huge_degrees() {
    let pts: [[f64; 2]; 3] = [[0.0, 0.0], [1.0, 2.0], [3.0, 1.0]];
    let v = FitVersion::CURRENT;
    // Three points with zero end derivatives, cubic: five control points, passes through the data.
    let fit: CurveFit<2> = Curve::interpolate_with_ends(
        &pts,
        [[0.0, 0.0], [0.0, 0.0]],
        3,
        Parametrization::ChordLength,
        v,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(fit.curve.homogeneous().len(), 5);
    assert!(fit.error < 1e-9 && fit.constraint_error < 1e-9);
    for (q, u) in pts.iter().zip(&fit.parameters) {
        assert!(close2(fit.curve.point(*u, RIGHT).unwrap(), *q, 1e-9));
    }
    // Too few points for the degree.
    assert_eq!(
        Curve::<2>::interpolate(&pts, 3, Parametrization::Uniform, v, &mut meter()),
        Err(NurbsError::DataCount {
            points: 3,
            needed: 4
        })
    );
    // Absurd degrees: refused as degrees, before any index is built from them.
    for degree in [MAX_DEGREE + 1, 1 << 40, usize::MAX, i64::MAX as usize, 0] {
        assert!(
            matches!(
                Curve::<2>::interpolate(&pts, degree, Parametrization::Uniform, v, &mut meter()),
                Err(NurbsError::Degree { .. })
            ),
            "degree {degree}"
        );
        assert!(
            matches!(
                Curve::<2>::interpolate_with_ends(
                    &pts,
                    [[0.0, 0.0], [0.0, 0.0]],
                    degree,
                    Parametrization::Uniform,
                    v,
                    &mut meter()
                ),
                Err(NurbsError::Degree { .. })
            ),
            "ends, degree {degree}"
        );
    }
    // Non-finite end derivatives and data are refused, naming the item.
    assert!(matches!(
        Curve::<2>::interpolate_with_ends(
            &pts,
            [[f64::NAN, 0.0], [0.0, 0.0]],
            3,
            Parametrization::Uniform,
            v,
            &mut meter()
        ),
        Err(NurbsError::DataNotFinite { .. })
    ));
}
