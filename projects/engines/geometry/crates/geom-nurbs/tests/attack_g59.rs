//! Attacks on piece G59's claims: inputs chosen to break the Bernstein toolkit's bounds, clips,
//! products and refusals. Every bound is checked against exact rational evaluation of the exact
//! curve or surface the stored doubles define, so a call may refuse an attack but may never return
//! a bound that is false.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::doc_markdown,
    clippy::needless_range_loop,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod common;

use common::{Lcg, Q, cox_de_boor, q, qi, zero};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, BernsteinCurve, Curve, Edge, Limit, NurbsError, Slab, Surface};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The exact point of a curve at u, from inside [a, b].
fn exact<const D: usize>(c: &Curve<D>, u: f64, (_, b): (f64, f64)) -> [Q; D] {
    let basis = c.basis();
    let limit = if u == b {
        Limit::FromLeft
    } else {
        Limit::FromRight
    };
    let n = &cox_de_boor(basis.knots(), basis.degree(), u, limit)[basis.degree()];
    let mut a: [Q; D] = core::array::from_fn(|_| zero());
    let mut w = zero();
    for (i, p) in c.homogeneous().iter().enumerate() {
        let wi = c.weights().map_or(qi(1), |ws| q(ws[i]));
        w += &n[i] * &wi;
        for d in 0..D {
            a[d] += &n[i] * q(p[d]);
        }
    }
    core::array::from_fn(|d| &a[d] / &w)
}

/// Claim: the bounds hold for the exact curve, where extraction in doubles rounds: knots that
/// are not dyadic (0.1, 1/3, 0.7) and coordinates with every bit set.
#[test]
fn bounds_hold_where_extraction_rounds() {
    let mut rng = Lcg(59);
    for trial in 0..300 {
        let p = 1 + rng.below(4);
        let mut knots = vec![0.0; p + 1];
        knots.extend([0.1, 1.0 / 3.0, 0.7]);
        knots.extend(std::iter::repeat_n(1.0, p + 1));
        let basis = Basis::new(p, knots).unwrap();
        let n = basis.count();
        let pts: Vec<[f64; 3]> = (0..n)
            .map(|_| [rng.unit() * 1e3 - 500.0, rng.unit(), -rng.unit() * 7.0])
            .collect();
        let c = if trial % 2 == 0 {
            Curve::new(basis, &pts).unwrap()
        } else {
            let w: Vec<f64> = (0..n).map(|_| 0.25 + rng.unit() * 4.0).collect();
            Curve::new_rational(basis, &pts, &w).unwrap()
        };
        let m = &mut meter();
        for piece in c.bernstein(m).unwrap() {
            let b = piece.bound(m).unwrap();
            let (lo, hi) = piece.domain();
            for k in 0..=12 {
                // Rounding can carry lo + (hi − lo)·k/12 one step past hi: keep it inside.
                let u = (lo + (hi - lo) * f64::from(k) / 12.0).clamp(lo, hi);
                let x = exact(&c, u, piece.domain());
                for d in 0..3 {
                    assert!(
                        q(b.lo[d]) <= x[d] && x[d] <= q(b.hi[d]),
                        "trial {trial}: coordinate {d} at {u}"
                    );
                }
            }
        }
    }
}

/// Claim: huge or tiny magnitudes give a true bound or a typed error, never a false bound.
#[test]
fn extreme_magnitudes_are_bounded_or_refused() {
    for scale in [1e-300, 1e300, f64::MAX / 4.0] {
        let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]).unwrap();
        let pts = [
            [scale, -scale],
            [-scale, scale],
            [scale, scale],
            [-scale, -scale],
        ];
        for w in [None, Some([1e-300, 1.0, 1e300, 1.0])] {
            let c = match w {
                None => Curve::new(basis.clone(), &pts),
                Some(w) => Curve::new_rational(basis.clone(), &pts, &w),
            };
            let Ok(c) = c else { continue };
            let m = &mut meter();
            match c.bernstein(m) {
                Ok(pieces) => {
                    for piece in pieces {
                        if let Ok(b) = piece.bound(m) {
                            let x = exact(&c, piece.domain().0, piece.domain());
                            for d in 0..2 {
                                assert!(q(b.lo[d]) <= x[d] && x[d] <= q(b.hi[d]));
                            }
                        }
                        let _ = piece.derivative_bound(m);
                    }
                }
                Err(e) => assert_eq!(e, NurbsError::NotFinite, "scale {scale}"),
            }
        }
    }
}

/// Claim: a clip keeps every meeting, and drops the whole curve only when it misses the slab.
#[test]
fn clips_are_none_only_for_misses() {
    let m = &mut meter();
    let c = Curve::new(
        Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap(),
        &[[0.0, 0.0], [1.0, 2.0], [2.0, -2.0], [3.0, 0.0]],
    )
    .unwrap();
    let piece = c.bernstein(m).unwrap().remove(0);
    let slab = |lo: f64, hi: f64| Slab {
        origin: [0.0, 0.0],
        normal: [0.0, 1.0],
        lo,
        hi,
    };
    // Far above: no meeting.
    assert_eq!(piece.clip(&slab(5.0, 6.0), m).unwrap(), None);
    // Everything: the whole domain.
    assert_eq!(piece.clip(&slab(-10.0, 10.0), m).unwrap(), Some((0.0, 1.0)));
    // Touching the top of the polygon's hull exactly.
    assert!(piece.clip(&slab(2.0, 3.0), m).unwrap().is_some());
    // The curve's own maximum is below 2: a clip may keep a range, but it must hold every t
    // where y(t) ≥ 0.5.
    let r = piece.clip(&slab(0.5, 9.0), m).unwrap().unwrap();
    for k in 0..=64 {
        let t = f64::from(k) / 64.0;
        let y = &exact(&c, t, (0.0, 1.0))[1];
        if *y >= q(0.5) {
            assert!(r.0 <= t && t <= r.1, "dropped {t}");
        }
    }
    // A slab with an infinite side is refused, not trusted.
    assert_eq!(
        piece.clip(&slab(f64::NEG_INFINITY, 1.0), m),
        Err(NurbsError::NotFinite)
    );
}

/// Claim: refusals are typed: weights not certainly positive, a domain of one value, products
/// beyond the degree limit.
#[test]
fn malformed_forms_are_refused() {
    let one = Interval::point(1.0).unwrap();
    let zero_w = Interval::point(0.0).unwrap();
    assert_eq!(
        BernsteinCurve::new((0.0, 1.0), vec![[one]; 2], Some(vec![one, zero_w])),
        Err(NurbsError::Weight { index: 1 })
    );
    assert_eq!(
        BernsteinCurve::new((1.0, 1.0), vec![[one]; 2], None),
        Err(NurbsError::EmptyDomain)
    );
    assert_eq!(
        BernsteinCurve::<1>::new((0.0, 1.0), vec![], None),
        Err(NurbsError::PointCount {
            points: 0,
            expected: 1
        })
    );
    let m = &mut meter();
    let big = BernsteinCurve::new((0.0, 1.0), vec![[one; 3]; 101], None).unwrap();
    let small = BernsteinCurve::new((0.0, 1.0), vec![[one; 3]; 21], None).unwrap();
    assert_eq!(big.dot(&small, m), Err(NurbsError::Degree { degree: 120 }));
}

/// Claim: the normal patch of a degree-28 patch (degree 55 × 55) is computed and bounds the exact
/// normal.
#[test]
fn the_highest_degree_normal_patch_bounds_the_normal() {
    let p = geom_nurbs::MAX_DEGREE;
    let mut knots = vec![0.0; p + 1];
    knots.extend(std::iter::repeat_n(1.0, p + 1));
    let bu = Basis::new(p, knots.clone()).unwrap();
    let bv = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let mut rng = Lcg(28);
    let pts: Vec<[f64; 3]> = (0..(p + 1) * 2)
        .map(|_| {
            [
                f64::from(rng.below(100) as u32),
                f64::from(rng.below(100) as u32),
                1.0,
            ]
        })
        .collect();
    let s = Surface::new(bu, bv, &pts).unwrap();
    let m = &mut meter();
    let patch = s.bernstein(m).unwrap().remove(0);
    let n = patch.normal(m).unwrap();
    assert_eq!(n.degrees(), (2 * p - 1, 1));
    let b = n.bound(m).unwrap();
    assert!(b.lo.iter().chain(&b.hi).all(|x| x.is_finite()));
}

/// The interval holding exactly `x`.
fn pt(x: f64) -> Interval {
    Interval::point(x).unwrap()
}

/// Claim: a constant piece (degree 0) inside a slab is inside it over its whole domain, and one
/// outside is outside everywhere; a curve of degree 1 standing for the same constant clips the
/// same way.
#[test]
fn a_constant_piece_clips_to_its_whole_domain_or_nothing() {
    let m = &mut meter();
    let slab = Slab {
        origin: [0.0, 0.0],
        normal: [0.0, 1.0],
        lo: 0.0,
        hi: 1.0,
    };
    for (y, kept) in [
        (0.5, true),
        (0.0, true),
        (1.0, true),
        (1.5, false),
        (-0.5, false),
    ] {
        let c = [pt(2.0), pt(y)];
        let constant = BernsteinCurve::new((0.25, 0.75), vec![c], None).unwrap();
        let line = BernsteinCurve::new((0.25, 0.75), vec![c, c], None).unwrap();
        let expected = kept.then_some((0.25, 0.75));
        assert_eq!(
            constant.clip(&slab, m).unwrap(),
            expected,
            "degree 0 at {y}"
        );
        assert_eq!(line.clip(&slab, m).unwrap(), expected, "degree 1 at {y}");
    }
}

/// Claim: a slab whose sides are crossed (lo above hi) holds nothing, so a curve that reaches both
/// of its sides is still clipped away entirely.
#[test]
fn a_crossed_slab_clips_everything_away() {
    let m = &mut meter();
    let piece = BernsteinCurve::new(
        (0.0, 1.0),
        vec![[pt(0.0), pt(0.0)], [pt(1.0), pt(3.0)]],
        None,
    )
    .unwrap();
    let crossed = Slab {
        origin: [0.0, 0.0],
        normal: [0.0, 1.0],
        lo: 2.0,
        hi: 1.0,
    };
    assert_eq!(piece.clip(&crossed, m).unwrap(), None);
}

/// Claim: a patch of degree 0 in one direction is constant along it: its partial there is zero
/// (degree 0 again), and its partial in the other direction is the usual scaled difference.
#[test]
fn a_patch_flat_in_one_direction_has_a_zero_partial_there() {
    let m = &mut meter();
    // Degrees (0, 2) on [0, 1] × [0, 2]: one row of three points.
    let row = vec![[pt(0.0), pt(1.0)], [pt(1.0), pt(3.0)], [pt(4.0), pt(4.0)]];
    let patch =
        geom_nurbs::BernsteinPatch::new(((0.0, 1.0), (0.0, 2.0)), (0, 2), row, None).unwrap();
    let du = patch.partial(geom_nurbs::Direction::U, m).unwrap();
    assert_eq!(du.degrees(), (0, 2));
    assert!(
        du.points()
            .iter()
            .flatten()
            .all(|x| x.lo() <= 0.0 && x.hi() >= 0.0 && x.hi() - x.lo() < 1e-300)
    );
    let dv = patch.partial(geom_nurbs::Direction::V, m).unwrap();
    assert_eq!(dv.degrees(), (0, 1));
    // q / (v1 − v0) = 1 times the differences of neighbouring points.
    for (got, want) in dv.points().iter().zip([[1.0, 2.0], [3.0, 1.0]]) {
        for (g, w) in got.iter().zip(want) {
            assert!(
                g.contains(w) && g.hi() - g.lo() < 1e-14,
                "{g:?} against {w}"
            );
        }
    }
    // Degrees (2, 0): the same column; now v is the flat direction.
    let column = vec![[pt(0.0), pt(1.0)], [pt(1.0), pt(3.0)], [pt(4.0), pt(4.0)]];
    let patch =
        geom_nurbs::BernsteinPatch::new(((0.0, 2.0), (0.0, 1.0)), (2, 0), column, None).unwrap();
    let dv = patch.partial(geom_nurbs::Direction::V, m).unwrap();
    assert_eq!(dv.degrees(), (2, 0));
    assert!(
        dv.points()
            .iter()
            .flatten()
            .all(|x| x.lo() <= 0.0 && x.hi() >= 0.0 && x.hi() - x.lo() < 1e-300)
    );
    let du = patch.partial(geom_nurbs::Direction::U, m).unwrap();
    assert_eq!(du.degrees(), (1, 0));
    for (got, want) in du.points().iter().zip([[1.0, 2.0], [3.0, 1.0]]) {
        for (g, w) in got.iter().zip(want) {
            assert!(
                g.contains(w) && g.hi() - g.lo() < 1e-14,
                "{g:?} against {w}"
            );
        }
    }
}

/// Claim: a rational patch says so, and its partial derivatives are refused (differentiate the
/// numerator and the weights instead).
#[test]
fn a_rational_patch_refuses_its_partials() {
    let m = &mut meter();
    let points = vec![
        [pt(0.0), pt(0.0)],
        [pt(2.0), pt(0.0)],
        [pt(0.0), pt(1.0)],
        [pt(2.0), pt(2.0)],
    ];
    let weights = vec![pt(1.0), pt(2.0), pt(1.0), pt(2.0)];
    let patch =
        geom_nurbs::BernsteinPatch::new(((0.0, 1.0), (0.0, 1.0)), (1, 1), points, Some(weights))
            .unwrap();
    assert!(patch.is_rational());
    assert!(!patch.numerator().is_rational());
    for dir in [geom_nurbs::Direction::U, geom_nurbs::Direction::V] {
        assert_eq!(patch.partial(dir, m), Err(NurbsError::Rational));
    }
}

/// Claim: a coefficient that is exactly 0 once rounded outward is on the slab, not a crossing: a
/// line that touches the slab's lower side at its start is kept there, and no zero is divided by.
/// The lower side sits 6 subnormal steps above the start, which the outward rounding of the
/// distance's four operations (one step each, and one for the side) brings to exactly 0.
#[test]
fn a_coefficient_of_exactly_zero_touches_the_slab() {
    let m = &mut meter();
    let tiny = f64::from_bits(1);
    for end in [-1.0, 1.0] {
        let line = BernsteinCurve::new((0.0, 1.0), vec![[pt(0.0)], [pt(end)]], None).unwrap();
        let slab = Slab {
            origin: [0.0],
            normal: [1.0],
            lo: 6.0 * tiny,
            hi: 6.0 * tiny,
        };
        let (start, stop) = line.clip(&slab, m).unwrap().unwrap();
        assert_eq!(start, 0.0);
        assert!(stop < 1e-15, "{stop}");
    }
}

/// Claim: products reach exactly 4·MAX_DEGREE in each direction and refuse one more, naming the
/// degree.
#[test]
fn products_reach_four_times_the_degree_limit_and_no_further() {
    let m = &mut meter();
    let top = 4 * geom_nurbs::MAX_DEGREE;
    let curve =
        |degree: usize| BernsteinCurve::new((0.0, 1.0), vec![[pt(1.0)]; degree + 1], None).unwrap();
    let half = top / 2;
    let product = curve(half).scaled_by(&curve(half), m).unwrap();
    assert_eq!(product.degree(), top);
    assert_eq!(
        curve(half + 1).scaled_by(&curve(half), m),
        Err(NurbsError::Degree { degree: top + 1 })
    );
    let patch = |(p, q): (usize, usize)| {
        geom_nurbs::BernsteinPatch::new(
            ((0.0, 1.0), (0.0, 1.0)),
            (p, q),
            vec![[pt(1.0)]; (p + 1) * (q + 1)],
            None,
        )
        .unwrap()
    };
    for (a, b, ok) in [
        ((half, 0), (half, 0), true),
        ((0, half), (0, half), true),
        ((half + 1, 0), (half, 0), false),
        ((0, half + 1), (0, half), false),
    ] {
        let r = patch(a).scaled_by(&patch(b), m);
        if ok {
            let d = r.unwrap().degrees();
            assert_eq!(d, (a.0 + b.0, a.1 + b.1));
        } else {
            assert_eq!(r, Err(NurbsError::Degree { degree: top + 1 }));
        }
    }
}

/// Claim: a product with one rational factor is refused as rational, whichever factor it is, for
/// curves and patches; patches on different domains are refused for their domains.
#[test]
fn one_rational_factor_is_enough_to_refuse_a_product() {
    let m = &mut meter();
    let plain = BernsteinCurve::new((0.0, 1.0), vec![[pt(1.0)], [pt(2.0)]], None).unwrap();
    let rational = BernsteinCurve::new(
        (0.0, 1.0),
        vec![[pt(1.0)], [pt(2.0)]],
        Some(vec![pt(1.0), pt(2.0)]),
    )
    .unwrap();
    assert_eq!(plain.scaled_by(&rational, m), Err(NurbsError::Rational));
    assert_eq!(rational.scaled_by(&plain, m), Err(NurbsError::Rational));
    let patch = |rational: bool, v: (f64, f64)| {
        geom_nurbs::BernsteinPatch::new(
            ((0.0, 1.0), v),
            (1, 0),
            vec![[pt(1.0)], [pt(2.0)]],
            rational.then(|| vec![pt(1.0), pt(2.0)]),
        )
        .unwrap()
    };
    let (a, r) = (patch(false, (0.0, 1.0)), patch(true, (0.0, 1.0)));
    assert_eq!(a.scaled_by(&r, m), Err(NurbsError::Rational));
    assert_eq!(r.scaled_by(&a, m), Err(NurbsError::Rational));
    assert_eq!(
        a.scaled_by(&patch(false, (0.0, 2.0)), m),
        Err(NurbsError::Domains)
    );
}

/// Claim: a patch with no direction (all points one point) or one whose diagonals overflow has no
/// fat plane: both are refused as degenerate, not given a slab of zero or infinite direction.
#[test]
fn a_patch_without_a_direction_has_no_fat_plane() {
    let m = &mut meter();
    let patch = |pts: Vec<[Interval; 3]>| {
        geom_nurbs::BernsteinPatch::new(((0.0, 1.0), (0.0, 1.0)), (1, 1), pts, None).unwrap()
    };
    let p = [pt(1.0), pt(2.0), pt(3.0)];
    assert_eq!(patch(vec![p; 4]).fat_plane(m), Err(NurbsError::Degenerate));
    let big = 1e300;
    let corners = vec![
        [pt(-big), pt(-big), pt(0.0)],
        [pt(big), pt(-big), pt(0.0)],
        [pt(-big), pt(big), pt(0.0)],
        [pt(big), pt(big), pt(big)],
    ];
    assert_eq!(patch(corners).fat_plane(m), Err(NurbsError::Degenerate));
}

/// Claim: a polynomial's coefficients exclude a zero only when none is zero and all have one sign.
#[test]
fn signs_exclude_zero_only_with_one_strict_sign() {
    use geom_nurbs::exact::Signs;
    let s = |positive, negative, zero| Signs {
        positive,
        negative,
        zero,
    };
    for (signs, excludes) in [
        (s(3, 0, 0), true),
        (s(0, 3, 0), true),
        (s(2, 1, 0), false),
        (s(3, 0, 1), false),
        (s(0, 3, 1), false),
        (s(0, 0, 2), false),
    ] {
        assert_eq!(signs.excludes_zero(), excludes, "{signs:?}");
    }
}

/// Claim: an exact normal patch counts each component's signs from its exact coefficients, and
/// there are three components.
#[test]
fn exact_normal_signs_count_each_component() {
    use geom_num::grid::P3;
    use geom_nurbs::exact::{Signs, normal_patch};
    let m = &mut meter();
    let p3 = |x, y, z| P3::new(x, y, z).unwrap();
    // A bilinear saddle over the unit square, scaled to the grid.
    let pts = [p3(0, 0, 0), p3(0, 8, 8), p3(8, 0, 8), p3(8, 8, 0)];
    let n = normal_patch(&pts, (1, 1), m).unwrap();
    for k in 0..3 {
        let mut want = Signs::default();
        for c in n.coefficients() {
            if c[k].is_zero() {
                want.zero += 1;
            } else if c[k].is_negative() {
                want.negative += 1;
            } else {
                want.positive += 1;
            }
        }
        assert_eq!(n.signs(k).unwrap(), want, "component {k}");
        assert_eq!(
            want.positive + want.negative + want.zero,
            n.coefficients().len()
        );
    }
    // The saddle's normal turns: x and y change sign, z does not.
    assert!(!n.signs(0).unwrap().excludes_zero());
    assert!(n.signs(2).unwrap().excludes_zero());
    assert_eq!(n.signs(3), Err(NurbsError::Index { index: 3, count: 3 }));
}

/// The cone-like patch of degrees (1, 2) whose `edge` is one point, the apex.
fn apex_on(edge: Edge) -> Surface<3> {
    let apex = [0.0, 0.0, 8.0];
    let ring = [[8.0, 0.0, 0.0], [8.0, 8.0, 0.0], [0.0, 8.0, 0.0]];
    let line = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let arc = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let (bu, bv, pts) = match edge {
        Edge::U0 => (line, arc, [apex, apex, apex, ring[0], ring[1], ring[2]]),
        Edge::U1 => (line, arc, [ring[0], ring[1], ring[2], apex, apex, apex]),
        Edge::V0 => (arc, line, [apex, ring[0], apex, ring[1], apex, ring[2]]),
        Edge::V1 => (arc, line, [ring[0], apex, ring[1], apex, ring[2], apex]),
    };
    Surface::new(bu, bv, &pts).unwrap()
}

/// Claim: a collapsed edge is found on each of the four sides, and only the normal patch on that
/// side is divided, by one degree in the direction across the edge.
#[test]
fn a_collapsed_edge_on_any_side_divides_its_patch() {
    let m = &mut meter();
    for edge in [Edge::U0, Edge::U1, Edge::V0, Edge::V1] {
        let s = apex_on(edge);
        assert_eq!(s.collapsed_edges(), vec![edge]);
        let full = s
            .bernstein(m)
            .unwrap()
            .remove(0)
            .normal(m)
            .unwrap()
            .degrees();
        let reduced = s.normal_patches(m).unwrap().remove(0).degrees();
        let want = match edge {
            Edge::U0 | Edge::U1 => (full.0 - 1, full.1),
            Edge::V0 | Edge::V1 => (full.0, full.1 - 1),
        };
        assert_eq!(reduced, want, "{edge:?}");
    }
}

/// Claim: weighted pole points whose cross products a·w′ could lose their rounding error to
/// underflow (below 2^−968) are not trusted as one point, however equal they look; the same pole
/// at a normal scale is.
#[test]
fn a_pole_near_underflow_is_not_trusted_as_one_point() {
    let line = || Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    for (scale, collapsed) in [(1e-292, false), (1e-200, true)] {
        let apex = [scale, scale, scale];
        let s = Surface::new_rational(
            line(),
            line(),
            &[apex, apex, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[1.0, 2.0, 1.0, 1.0],
        )
        .unwrap();
        let want = if collapsed {
            vec![Edge::U0]
        } else {
            Vec::new()
        };
        assert_eq!(s.collapsed_edges(), want, "scale {scale:e}");
    }
}
