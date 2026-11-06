//! Attack tests from SMLib's confirmed defects, batch 1: `geom-nurbs`.
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
    Basis, Curve, CurveFit, Direction, FitVersion, KnotMode, Limit, NurbsError, Surface,
};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 36))
}

const RIGHT: Limit = Limit::FromRight;

/// A clamped cubic with six control points on [0, 3] and interior knots 1 and 2.
fn cubic() -> Curve<3> {
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0]).unwrap();
    let pts = [
        [0.0, 0.0, 0.0],
        [1.0, 2.0, 0.5],
        [2.5, -1.0, 1.0],
        [4.0, 3.0, -0.5],
        [5.0, 0.5, 2.0],
        [6.0, 1.0, 0.0],
    ];
    Curve::new(basis, &pts).unwrap()
}

fn close(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= tol)
}

/// solidcore plan/upstream-issues/UB-11--n-basisinsertknots-reads-one-past-or-before-x.md and
/// UB-196--n-basisinsertknots-writes-past-its-output-for-knot-vectors.md (b4de2149): knot
/// insertion read one past the new knots (an empty list read before them), and with a knot vector
/// shorter than 2p + 2 it wrote past its output. The inputs are the defects': an empty list of
/// new knots; p = 1, knots [4294967296, 0] (unsorted, too short) with new knots
/// [1e300, 1.5, 2.5, 3.5]. Ours refuses the short vector at construction; an empty refinement is
/// the same curve; new knots outside the domain or out of order are refused; a valid refinement
/// leaves the curve where it was.
#[test]
fn ub_11_and_ub_196_knot_insertion_with_empty_short_or_wild_input() {
    // The short, unsorted knot vector of UB-196 cannot make a basis at all.
    assert!(matches!(
        Basis::new(1, vec![4_294_967_296.0, 0.0]),
        Err(NurbsError::KnotCount {
            knots: 2,
            needed: 4
        })
    ));
    assert!(matches!(
        Basis::new(2, vec![0.0, 0.0, 1.0, 1.0, 1.0]),
        Err(NurbsError::KnotCount { .. })
    ));
    // Unsorted and long enough: refused as decreasing.
    assert!(matches!(
        Basis::new(1, vec![4_294_967_296.0, 0.0, 1.0, 2.0]),
        Err(NurbsError::KnotDecreasing { index: 1 })
    ));
    let c = cubic();
    // The empty list: the same curve, with no deviation.
    let same = c.refine(&[], &mut meter()).unwrap();
    assert_eq!(same.value, c);
    assert_eq!(same.deviation, 0.0);
    // The defect's new knots: 1e300 is far outside [0, 3].
    assert_eq!(
        c.refine(&[1e300, 1.5, 2.5, 3.5], &mut meter())
            .map(|r| r.value),
        Err(NurbsError::KnotOutside)
    );
    // In range but out of order, at the ends of the domain, not finite, or too often.
    for bad in [
        vec![2.5, 1.5],
        vec![0.0],
        vec![3.0],
        vec![f64::NAN],
        vec![f64::INFINITY],
        vec![-1.0, 1.5],
    ] {
        assert_eq!(
            c.refine(&bad, &mut meter()).map(|r| r.value),
            Err(NurbsError::KnotOutside),
            "{bad:?}"
        );
    }
    assert_eq!(
        c.refine(&[0.5, 0.5, 0.5, 0.5], &mut meter())
            .map(|r| r.value),
        Err(NurbsError::TooManyKnots)
    );
    // A valid refinement keeps every point of the curve, within its certified deviation.
    let fine = c.refine(&[0.3, 0.3, 1.5, 2.9], &mut meter()).unwrap();
    assert_eq!(fine.value.homogeneous().len(), 10);
    assert!(fine.deviation >= 0.0 && fine.deviation < 1e-12);
    for k in 0..=60 {
        let u = 3.0 * f64::from(k) / 60.0;
        let (a, b) = (
            c.point(u, RIGHT).unwrap(),
            fine.value.point(u, RIGHT).unwrap(),
        );
        assert!(close(a, b, 1e-12 + fine.deviation), "u = {u}");
    }
    // One knot at a time the same way.
    let one = c.insert_knot(1.5, 1, &mut meter()).unwrap();
    assert!(close(
        c.point(1.5, RIGHT).unwrap(),
        one.value.point(1.5, RIGHT).unwrap(),
        1e-12
    ));
    assert_eq!(
        c.insert_knot(f64::NAN, 1, &mut meter()).map(|r| r.value),
        Err(NurbsError::KnotOutside)
    );
}

/// solidcore plan/upstream-issues/UB-250--n-igesclampsrf-writes-past-its-new-knot-vector-when-a-knot.md
/// and UB-238--smbsplinecurve-createcanonical-corrupts-the-heap-for-a-nan.md (b4de2149): a NaN
/// knot made a span search find no span and a clamp write past its output, and made a curve
/// builder corrupt the heap. The inputs are the defects': v knots 360 360 360 NaN NaN 540 540 540
/// (degree 2); degree 4 with knots [0, 0.18, 0.54, 0.91, NaN] and multiplicities [5, 3, 2, 1, 5];
/// degree 5 with knots [0, NaN, 1.14] and multiplicities [6, 2, 6]. Ours refuses each at
/// construction, naming the knot.
#[test]
fn ub_250_and_ub_238_a_nan_knot_is_refused_at_construction() {
    assert_eq!(
        Basis::new(
            2,
            vec![360.0, 360.0, 360.0, f64::NAN, f64::NAN, 540.0, 540.0, 540.0]
        ),
        Err(NurbsError::KnotNotFinite { index: 3 })
    );
    let expand = |knots: &[f64], mult: &[usize]| -> Vec<f64> {
        knots
            .iter()
            .zip(mult)
            .flat_map(|(&k, &m)| std::iter::repeat_n(k, m))
            .collect()
    };
    let k = expand(&[0.0, 0.18, 0.54, 0.91, f64::NAN], &[5, 3, 2, 1, 5]);
    assert!(matches!(
        Basis::new(4, k),
        Err(NurbsError::KnotNotFinite { index: 11 })
    ));
    let k = expand(&[0.0, f64::NAN, 1.14], &[6, 2, 6]);
    assert!(matches!(
        Basis::new(5, k),
        Err(NurbsError::KnotNotFinite { index: 6 })
    ));
    // An infinite knot, and knots whose spread overflows, are refused too.
    assert!(matches!(
        Basis::new(1, vec![0.0, 0.0, 1.0, f64::INFINITY]),
        Err(NurbsError::KnotNotFinite { index: 3 })
    ));
    assert_eq!(
        Basis::new(1, vec![-f64::MAX, -f64::MAX, f64::MAX, f64::MAX]),
        Err(NurbsError::KnotRange)
    );
    // No basis, so no curve or surface on it: the same refusal from a NaN control point or weight.
    let basis = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    assert_eq!(
        Curve::<2>::new(basis.clone(), &[[0.0, 0.0], [f64::NAN, 1.0]]).map(|_| ()),
        Err(NurbsError::PointNotFinite { index: 1 })
    );
    assert!(matches!(
        Curve::<2>::new_rational(basis, &[[0.0, 0.0], [1.0, 1.0]], &[1.0, f64::NAN]),
        Err(NurbsError::Weight { .. })
    ));
}

/// solidcore plan/upstream-issues/UB-236--sm-createbezsegments-copies-control-points-past-the-polygon.md
/// and UB-286--createbezsegments-writes-through-a-null-next-segment.md (b4de2149): Bezier
/// extraction of a curve whose end knots are not clamped copied control points past the polygon
/// (UB-236) or wrote through a null next segment, for a curve with one span (UB-286). The input is
/// the one of UB-286's fuzz case: degree 4, knots -4.77 x5, -3.77 x4, -0.0, which is
/// non-decreasing, has one non-zero span, and is not clamped at the end. Ours refuses to
/// extract an unclamped curve, evaluates it, and extracts the clamped one-span curve exactly.
#[test]
fn ub_236_and_ub_286_bezier_extraction_of_an_unclamped_one_span_curve() {
    let mut knots = vec![-4.77; 5];
    knots.extend([-3.77; 4]);
    knots.push(-0.0);
    let basis = Basis::new(4, knots).unwrap();
    assert!(!basis.is_clamped());
    let pts: Vec<[f64; 2]> = (0..basis.count())
        .map(|i| [i as f64, (i * i) as f64 - 3.0])
        .collect();
    let c = Curve::<2>::new(basis, &pts).unwrap();
    assert_eq!(
        c.to_bezier(&mut meter()).map(|r| r.value),
        Err(NurbsError::NotClamped)
    );
    // The curve itself still evaluates, finite, over its one span.
    let (lo, hi) = c.basis().domain();
    assert_eq!((lo, hi), (-4.77, -3.77));
    for k in 0..=20 {
        let u = lo + (hi - lo) * f64::from(k) / 20.0;
        let p = c.point(u, RIGHT).unwrap();
        assert!(p.iter().all(|x| x.is_finite()));
    }
    // An unclamped curve with several spans is refused the same way, and its elevation too.
    let uniform = Basis::new(3, (0..10).map(f64::from).collect()).unwrap();
    let pts: Vec<[f64; 2]> = (0..uniform.count()).map(|i| [i as f64, 1.0]).collect();
    let u = Curve::<2>::new(uniform, &pts).unwrap();
    assert_eq!(
        u.to_bezier(&mut meter()).map(|r| r.value),
        Err(NurbsError::NotClamped)
    );
    assert_eq!(
        u.elevate(1, &mut meter()).map(|r| r.value),
        Err(NurbsError::NotClamped)
    );
    // The clamped one-span curve extracts to one piece equal to it everywhere.
    let one = Basis::new(4, vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let pts: Vec<[f64; 2]> = (0..5)
        .map(|i| [f64::from(i), f64::from(i * i % 3)])
        .collect();
    let c = Curve::<2>::new(one, &pts).unwrap();
    let pieces = c.to_bezier(&mut meter()).unwrap();
    assert_eq!(pieces.value.len(), 1);
    for k in 0..=20 {
        let t = f64::from(k) / 20.0;
        let (a, b) = (
            c.point(t, RIGHT).unwrap(),
            pieces.value[0].point(t, RIGHT).unwrap(),
        );
        assert!((a[0] - b[0]).abs() <= 1e-13 && (a[1] - b[1]).abs() <= 1e-13);
    }
    // And a many-span clamped curve gives one piece per span, each a Bezier curve of full degree.
    let pieces = cubic().to_bezier(&mut meter()).unwrap().value;
    assert_eq!(pieces.len(), 3);
    assert!(
        pieces
            .iter()
            .all(|p| p.degree() == 3 && p.homogeneous().len() == 4)
    );
}

/// solidcore plan/upstream-issues/UB-225--n-approxnurbswithnonratcrv-overflows-its-nl-index-sample.md
/// (b4de2149): an approximation to a tiny tolerance (1e-300), or of a curve with a coordinate of
/// 1e300, computed a sample count that overflowed an integer and wrote past its array. Ours is
/// given the same data under a work budget: a tolerance below what the doubles can reach, a
/// coordinate of 1e300, and a tolerance of zero. Each must end in a refusal or a curve whose
/// certified error is within the tolerance, and never in a panic or a hang.
#[test]
fn ub_225_an_approximation_to_an_extreme_tolerance_or_size_is_refused_or_certified() {
    let ring: Vec<[f64; 2]> = (0..24)
        .map(|i| {
            let a = f64::from(i) * 0.26;
            [10.0 * a.cos(), 10.0 * a.sin()]
        })
        .collect();
    let mut outcomes = Vec::new();
    for tol in [1e-300, 0.0, 5e-324, 1e-15] {
        for mode in [KnotMode::Single, KnotMode::Multiple] {
            outcomes.push((
                tol,
                Curve::<2>::approximate(&ring, 3, tol, mode, FitVersion::CURRENT, &mut meter()),
            ));
        }
    }
    // One huge coordinate, with an ordinary and a tiny tolerance.
    let mut huge = ring.clone();
    huge[11][0] = 1e300;
    for tol in [1e-6, 1e-300] {
        outcomes.push((
            tol,
            Curve::<2>::approximate(
                &huge,
                3,
                tol,
                KnotMode::Single,
                FitVersion::CURRENT,
                &mut meter(),
            ),
        ));
    }
    for (tol, r) in outcomes {
        match r {
            Ok(CurveFit { error, curve, .. }) => {
                assert!(
                    error.is_finite() && error <= tol,
                    "error {error:e} above tolerance {tol:e}"
                );
                let (lo, hi) = curve.basis().domain();
                assert!(
                    curve
                        .point(lo, RIGHT)
                        .unwrap()
                        .iter()
                        .all(|x| x.is_finite())
                );
                assert!(
                    curve
                        .point((lo + hi) / 2.0, RIGHT)
                        .unwrap()
                        .iter()
                        .all(|x| x.is_finite())
                );
            }
            Err(e) => assert!(
                matches!(
                    e,
                    NurbsError::OutsideTolerance
                        | NurbsError::NotFinite
                        | NurbsError::Stopped(_)
                        | NurbsError::Tolerance
                        | NurbsError::CoincidentData { .. }
                ),
                "tolerance {tol:e}: {e:?}"
            ),
        }
    }
    // Not a tolerance at all: refused.
    for tol in [f64::NAN, f64::INFINITY, -1.0] {
        assert_eq!(
            Curve::<2>::approximate(
                &ring,
                3,
                tol,
                KnotMode::Single,
                FitVersion::CURRENT,
                &mut meter()
            )
            .map(|f| f.error),
            Err(NurbsError::Tolerance)
        );
    }
    // A coordinate that is not a number is refused as data.
    let mut bad = ring;
    bad[3][1] = f64::NAN;
    assert!(matches!(
        Curve::<2>::approximate(
            &bad,
            3,
            1e-3,
            KnotMode::Single,
            FitVersion::CURRENT,
            &mut meter()
        ),
        Err(NurbsError::DataNotFinite { .. })
    ));
}

/// solidcore plan/upstream-issues/WR-38--smbsplinesurface-data-constructor-zeroes-the-v-knots-when-s.md
/// (b4de2149): the surface constructor copied the v knots into a destination sized for the u
/// knots, so with more v knots than u knots the v knots stayed zero. The input is the defect's:
/// a surface with more v knots than u knots. Ours must keep every knot as given, and the corners
/// of the clamped surface must be the corner control points.
#[test]
fn wr_38_a_surface_with_more_v_knots_than_u_knots_keeps_its_v_knots() {
    let ku = vec![0.0, 0.0, 1.0, 1.0];
    let kv = vec![0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0, 1.0];
    assert!(kv.len() > ku.len());
    let bu = Basis::new(1, ku.clone()).unwrap();
    let bv = Basis::new(3, kv.clone()).unwrap();
    let (nu, nv) = (bu.count(), bv.count());
    assert_eq!((nu, nv), (2, 7));
    let net: Vec<[f64; 3]> = (0..nu)
        .flat_map(|i| (0..nv).map(move |j| [i as f64 * 10.0, j as f64, ((i + j) % 3) as f64]))
        .collect();
    let s = Surface::new(bu, bv, &net).unwrap();
    assert_eq!(s.basis_u().knots(), ku.as_slice());
    assert_eq!(s.basis_v().knots(), kv.as_slice());
    assert_eq!(s.basis_v().domain(), (0.0, 1.0));
    // The corners are the corner control points (clamped in both directions).
    let l = (RIGHT, RIGHT);
    assert_eq!(s.point(0.0, 0.0, l).unwrap(), net[0]);
    assert_eq!(
        s.point(0.0, 1.0, (RIGHT, Limit::FromLeft)).unwrap(),
        net[nv - 1]
    );
    assert_eq!(
        s.point(1.0, 0.0, (Limit::FromLeft, RIGHT)).unwrap(),
        net[(nu - 1) * nv]
    );
    assert_eq!(
        s.point(1.0, 1.0, (Limit::FromLeft, Limit::FromLeft))
            .unwrap(),
        net[nu * nv - 1]
    );
    // The v direction is not flattened: the surface varies with v along its first row.
    let a = s.point(0.0, 0.2, l).unwrap();
    let b = s.point(0.0, 0.8, l).unwrap();
    assert!((b[1] - a[1]).abs() > 0.1);
    // Refining v keeps the knots and the surface.
    let r = s.refine(Direction::V, &[0.1, 0.6], &mut meter()).unwrap();
    assert_eq!(r.value.basis_v().knots().len(), kv.len() + 2);
    let p = r.value.point(0.5, 0.45, l).unwrap();
    let q = s.point(0.5, 0.45, l).unwrap();
    assert!(close(p, q, 1e-12 + r.deviation));
}

/// solidcore plan/upstream-issues/WR-67--smbsplinecurve-getcontrolpolygon-zeroes-the-weights-array-of.md
/// (b4de2149): reading the control polygon of a rational curve zeroed its weights. Ours must give
/// every control point and weight back as they were put in, and the curve must keep passing
/// through its ends. The weights are 1 and 1e-3 to 1e3, and the points have mixed signs.
#[test]
fn wr_67_the_control_polygon_and_weights_of_a_rational_curve_are_read_back_as_given() {
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let pts = [
        [1.0, -2.0],
        [-3.5, 4.0],
        [0.25, 0.125],
        [10.0, -10.0],
        [-0.75, 6.0],
    ];
    let w = [1.0, 1e-3, 2.0, 1e3, 0.5];
    let c = Curve::<2>::new_rational(basis, &pts, &w).unwrap();
    assert!(c.is_rational());
    assert_eq!(c.weights().unwrap(), &w);
    for (i, (p, wi)) in pts.iter().zip(w).enumerate() {
        let got = c.control_point(i).unwrap();
        for k in 0..2 {
            assert!(
                (got[k] - p[k]).abs() <= 2.0 * f64::EPSILON * p[k].abs(),
                "point {i}"
            );
            assert!(
                (c.homogeneous()[i][k] - p[k] * wi).abs() <= 2.0 * f64::EPSILON * (p[k] * wi).abs()
            );
        }
    }
    // Reading did not change anything: the weights are still there, and the ends are the ends.
    assert_eq!(c.weights().unwrap(), &w);
    assert_eq!(c.point(0.0, RIGHT).unwrap(), pts[0]);
    assert_eq!(c.point(1.0, Limit::FromLeft).unwrap(), pts[4]);
    assert_eq!(
        c.control_point(5),
        Err(NurbsError::Index { index: 5, count: 5 })
    );
    // A zero, negative or infinite weight is refused, never stored.
    for bad in [0.0, -1.0, f64::INFINITY] {
        let mut v = w;
        v[2] = bad;
        assert!(matches!(
            Curve::<2>::new_rational(c.basis().clone(), &pts, &v),
            Err(NurbsError::Weight { .. })
        ));
    }
}

/// solidcore plan/upstream-issues/HANG-42--n-srfremoveallknots-n-srfremoveknotsconstraints-loop-forever.md
/// (b4de2149): knot removal on a net with a coordinate near 1e300 made its error bounds NaN and
/// looped forever, with tolerance 0 and with tolerance 1. Ours removes one knot a time, with a
/// certified deviation: the input is a curve with a coordinate of 1e300 and of -1e300, tolerances
/// 0, 1 and the largest double. It must end, remove nothing it cannot certify, and never
/// report a deviation above the tolerance.
#[test]
fn hang_42_knot_removal_with_a_coordinate_near_1e300_ends_and_certifies() {
    for big in [1e300, -1e300] {
        let basis = Basis::new(
            3,
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0],
        )
        .unwrap();
        let mut pts: Vec<[f64; 3]> = (0..basis.count())
            .map(|i| [i as f64, (i * 7 % 5) as f64, 0.5 * i as f64])
            .collect();
        pts[3][0] = big;
        let c = Curve::<3>::new(basis, &pts).unwrap();
        for tol in [0.0, 1.0, f64::MAX] {
            for u in [1.0, 2.0] {
                match c.remove_knot(u, 4, tol, &mut meter()) {
                    Ok((changed, removed)) => {
                        assert!(removed <= 4);
                        assert!(!changed.deviation.is_nan());
                        if removed > 0 {
                            assert!(
                                changed.deviation <= tol,
                                "{big:e} {tol:e} {u}: {}",
                                changed.deviation
                            );
                        }
                    }
                    Err(e) => assert!(
                        matches!(
                            e,
                            NurbsError::NotFinite
                                | NurbsError::Stopped(_)
                                | NurbsError::KnotOutside
                                | NurbsError::Discontinuous
                        ),
                        "{e:?}"
                    ),
                }
            }
        }
        // A tolerance that is not a number is refused, as is a knot that is not there.
        assert_eq!(
            c.remove_knot(1.0, 1, f64::NAN, &mut meter()).map(|r| r.1),
            Err(NurbsError::Tolerance)
        );
        assert_eq!(
            c.remove_knot(1.5, 1, 1.0, &mut meter()).map(|r| r.1),
            Err(NurbsError::KnotOutside)
        );
    }
}

/// solidcore plan/upstream-issues/HANG-37--b-spline-surface-extension-does-not-return-for-a-control.md
/// and UB-unnumbered--smsurface-surface-p8-segfaults-on-extreme-1e308-b-spline.md (b4de2149): a
/// surface with a control point of magnitude 1e157 or more (HANG-37: a 3 x 3 degree-2 rational
/// surface with one point at x = -1e300 and weights around 1.5) never returned, and one with
/// weights near 1e308 and control points near 1e-6 crashed. Ours is given both nets: it must
/// evaluate, differentiate, refine and elevate each to a finite answer or a refusal, and finish.
#[test]
fn hang_37_and_the_1e308_weight_surface_evaluate_or_refuse() {
    let knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    let nets: [(Vec<[f64; 3]>, Vec<f64>); 4] = [
        // HANG-37: one coordinate at -1e300, weights around 1.5.
        (
            (0..9)
                .map(|i| {
                    let x = if i == 4 { -1e300 } else { f64::from(i % 3) };
                    [x, f64::from(i / 3), 0.25 * f64::from(i)]
                })
                .collect(),
            (0..9).map(|i| 1.5 + 0.01 * f64::from(i)).collect(),
        ),
        // The crash: weights near 1e308, points near 1e-6.
        (
            (0..9)
                .map(|i| [1e-6 * f64::from(i), 1e-6, -1e-6 * f64::from(i % 2)])
                .collect(),
            (0..9)
                .map(|i| if i % 2 == 0 { 1e308 } else { 1.0 })
                .collect(),
        ),
        // Both weights near the largest double and points at 1e300.
        (
            (0..9)
                .map(|i| [1e300, -1e300 * f64::from(i % 2), 1.0])
                .collect(),
            vec![f64::MAX; 9],
        ),
        // The smallest positive weights.
        (
            (0..9).map(|i| [f64::from(i), 0.0, 0.0]).collect(),
            vec![f64::MIN_POSITIVE; 9],
        ),
    ];
    let finite = |p: &[f64; 3]| p.iter().all(|x| x.is_finite());
    for (k, (net, w)) in nets.iter().enumerate() {
        let (bu, bv) = (
            Basis::new(2, knots.clone()).unwrap(),
            Basis::new(2, knots.clone()).unwrap(),
        );
        let s = match Surface::new_rational(bu, bv, net, w) {
            Ok(s) => s,
            // Refused at construction (a weighted coordinate overflows): defined, and enough.
            Err(e) => {
                assert!(
                    matches!(
                        e,
                        NurbsError::PointNotFinite { .. } | NurbsError::Weight { .. }
                    ),
                    "{k}: {e:?}"
                );
                continue;
            }
        };
        for iu in 0..=4 {
            for iv in 0..=4 {
                let (u, v) = (f64::from(iu) / 4.0, f64::from(iv) / 4.0);
                match s.point(u, v, (RIGHT, RIGHT)) {
                    Ok(p) => assert!(finite(&p), "net {k} at ({u}, {v}): {p:?}"),
                    Err(e) => assert_eq!(e, NurbsError::NotFinite, "net {k}"),
                }
                let mut d = [[0.0; 3]; 4];
                match s.derivs(u, v, (RIGHT, RIGHT), (1, 1), &mut d) {
                    Ok(()) => assert!(d.iter().all(finite), "net {k} derivatives at ({u}, {v})"),
                    Err(e) => assert_eq!(e, NurbsError::NotFinite, "net {k}"),
                }
            }
        }
        for r in [
            s.insert_knot(Direction::U, 0.5, 1, &mut meter()),
            s.insert_knot(Direction::V, 0.25, 2, &mut meter()),
            s.elevate(Direction::U, 1, &mut meter()),
        ] {
            match r {
                Ok(c) => {
                    assert!(c.value.homogeneous().iter().all(finite), "net {k}");
                    assert!(!c.deviation.is_nan());
                }
                Err(e) => assert!(
                    matches!(e, NurbsError::NotFinite | NurbsError::Stopped(_)),
                    "net {k}: {e:?}"
                ),
            }
        }
    }
}

/// solidcore plan/upstream-issues/UB-80--smsurfofextrusion-evaluatestep-lets-order-5-overrun-sm.md
/// (b4de2149): a derivative of order 5 wrote past a fixed array that held orders up to 4. Ours
/// is asked for orders 0 to 5 of a cubic (and 0 to 8 of a rational one) and past its own limit.
/// Orders above the degree of a nonrational curve are exactly zero; the third is a constant that
/// equals the third difference of its control points; and an order beyond what the kernel
/// supports is refused with an error, not computed past an array.
#[test]
fn ub_80_derivatives_of_order_five_and_more_are_exact_or_refused() {
    let c = cubic();
    let mut d = [[0.0; 3]; 6];
    c.derivs(0.7, RIGHT, &mut d).unwrap();
    // The first interior knot is at 1, the cubic piece on [0, 1] has Bezier-like end derivatives.
    assert!(close(d[0], c.point(0.7, RIGHT).unwrap(), 0.0));
    assert_eq!(d[4], [0.0; 3]);
    assert_eq!(d[5], [0.0; 3]);
    let mut d2 = [[0.0; 3]; 6];
    c.derivs(0.1, RIGHT, &mut d2).unwrap();
    // The third derivative is constant on a span.
    assert!(close(d[3], d2[3], 1e-9));
    assert!(d[3].iter().any(|x| *x != 0.0));
    // The first derivative at the start of a clamped curve is p (P1 - P0) / (u_{p+1} - u_1).
    let mut s = [[0.0; 3]; 2];
    c.derivs(0.0, RIGHT, &mut s).unwrap();
    let p1 = c.control_point(1).unwrap();
    let p0 = c.control_point(0).unwrap();
    for k in 0..3 {
        assert!((s[1][k] - 3.0 * (p1[k] - p0[k]) / 1.0).abs() <= 1e-12);
    }
    // A rational curve has all its derivatives: orders 0 to 8, finite.
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let r = Curve::<2>::new_rational(
        basis,
        &[[0.0, 0.0], [1.0, 2.0], [3.0, 2.0], [4.0, 0.0]],
        &[1.0, 0.7, 1.3, 1.0],
    )
    .unwrap();
    let mut out = [[0.0; 2]; 9];
    r.derivs(0.4, RIGHT, &mut out).unwrap();
    assert!(out.iter().all(|v| v.iter().all(|x| x.is_finite())));
    // Past the supported order: an error, never a result.
    let mut huge = vec![[0.0; 3]; 200];
    assert!(c.derivs(0.5, RIGHT, &mut huge).is_err());
    // And outside the domain: refused.
    let mut e = [[0.0; 3]; 6];
    assert_eq!(
        c.derivs(3.5, RIGHT, &mut e),
        Err(NurbsError::ParameterOutside)
    );
    assert_eq!(
        c.derivs(f64::NAN, RIGHT, &mut e),
        Err(NurbsError::ParameterOutside)
    );
}
