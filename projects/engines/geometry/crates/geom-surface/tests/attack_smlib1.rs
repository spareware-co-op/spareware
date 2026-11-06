//! Attack tests from SMLib's confirmed defects, batch 1: `geom-surface`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: the right point or derivative, from an independent
//! formula or a finite difference, or a defined refusal, and never a panic, a hang, a NaN or a
//! wrong value.

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
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::manual_midpoint,
    clippy::type_complexity,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::needless_range_loop,
    clippy::range_minus_one,
    reason = "tests: native arithmetic, casts and indexing, platform maths on an independent oracle (the formula as it is written), prose that names defect files, and a failure stops the test"
)]

use geom_curve::{Basis as Leaf, Circle, CurveError, Frame, ParamCurve, Trimmed};
use geom_nurbs::{Basis, Curve, Direction, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Extrusion, Offset, ParamBox, ParamSurface, Plane, Revolution, Sphere, SurfaceKind, Torus,
    partial_index,
};

const RIGHT: Limit = Limit::FromRight;
const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 38))
}

fn frame() -> Frame<3> {
    Frame::standard().unwrap()
}

/// The length of `p - c`, scaled by `1 / scale` first so a radius of 1e300 or of 1e-12 does not
/// overflow or underflow the sum of squares.
fn distance_over(p: [f64; 3], c: [f64; 3], scale: f64) -> f64 {
    let s: f64 = p
        .iter()
        .zip(&c)
        .map(|(a, b)| ((a - b) / scale).powi(2))
        .sum();
    s.sqrt()
}

/// A twisted bicubic patch on [0, 1]^2, a fixed net with every coordinate moved off a plane.
fn twisted() -> Surface<3> {
    let k = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    let bu = Basis::new(3, k.clone()).unwrap();
    let bv = Basis::new(3, k).unwrap();
    let z = [
        [0.0, 0.6, -0.4, 0.2],
        [0.5, -0.7, 0.9, -0.3],
        [-0.8, 0.4, -0.2, 0.7],
        [0.3, -0.5, 0.6, -0.9],
    ];
    let mut net = Vec::new();
    for (i, row) in z.iter().enumerate() {
        for (j, h) in row.iter().enumerate() {
            net.push([i as f64, j as f64 + 0.15 * (i as f64 - 1.5), *h]);
        }
    }
    Surface::new(bu, bv, &net).unwrap()
}

/// The midpoint of each coordinate's enclosure of one partial of `k` at one parameter pair.
fn partial(k: &SurfaceKind, (u, v): (f64, f64), order: usize, i: usize, j: usize) -> [f64; 3] {
    let all = k
        .partials_over(((u, u), (v, v)), SIDES, order, &mut meter())
        .unwrap();
    let e = all[partial_index(i, j).unwrap()];
    [
        e[0].lo() * 0.5 + e[0].hi() * 0.5,
        e[1].lo() * 0.5 + e[1].hi() * 0.5,
        e[2].lo() * 0.5 + e[2].hi() * 0.5,
    ]
}

fn agree(a: [f64; 3], b: [f64; 3], rel: f64, what: &str) {
    let scale = 1.0 + b.iter().map(|x| x.abs()).fold(0.0, f64::max);
    for d in 0..3 {
        assert!(
            (a[d] - b[d]).abs() <= rel * scale,
            "{what}: {a:?} against {b:?}"
        );
    }
}

/// solidcore plan/upstream-issues/WR-160--smoffsetsurface-third-derivatives-use-wrong-coefficients-in.md,
/// WR-161--smoffsetsurface-outputs-buuv-in-place-of-buvv-and-bvvv.md and
/// WR-162--smoffsetsurface-writes-duv-over-duu-for-an-upper-half.md (b4de2149): the offset's third
/// derivatives Duuv and Duvv had wrong coefficients (11 percent off a central difference of Duu
/// on a twisted surface), Duvv and Dvvv took the base's Buuv in place of Buvv and Bvvv, and the
/// mixed derivative was written over Duu. The input is the defects': a twisted surface offset by
/// a distance that is not zero, every third and second partial against a central difference of
/// the one below, taken in the other direction too, so each slot is checked against two routes.
#[test]
fn wr_160_161_162_the_offset_of_a_twisted_surface_has_the_right_higher_partials() {
    let off = SurfaceKind::Offset(Offset::new(SurfaceKind::Nurbs(twisted()), 0.45).unwrap());
    let h = 1e-4;
    for (u, v) in [(0.31, 0.47), (0.62, 0.21), (0.5, 0.73)] {
        // The order-2 partials against differences of the first partials.
        // Duu (2,0) is d/du of Du; Duv (1,1) is d/dv of Du and d/du of Dv; Dvv (0,2) is d/dv of Dv.
        let d = |i, j, du: f64, dv: f64| {
            let (a, b) = (
                partial(&off, (u + du, v + dv), 3, i, j),
                partial(&off, (u - du, v - dv), 3, i, j),
            );
            [
                (a[0] - b[0]) / (2.0 * h),
                (a[1] - b[1]) / (2.0 * h),
                (a[2] - b[2]) / (2.0 * h),
            ]
        };
        let at = |i, j| partial(&off, (u, v), 3, i, j);
        // WR-162: Duu is not overwritten by Duv, and the two differ here.
        agree(at(2, 0), d(1, 0, h, 0.0), 1e-5, "Duu as d/du Du");
        agree(at(1, 1), d(1, 0, 0.0, h), 1e-5, "Duv as d/dv Du");
        agree(at(1, 1), d(0, 1, h, 0.0), 1e-5, "Duv as d/du Dv");
        agree(at(0, 2), d(0, 1, 0.0, h), 1e-5, "Dvv as d/dv Dv");
        let (duu, duv) = (at(2, 0), at(1, 1));
        assert!(
            (0..3).any(|k| (duu[k] - duv[k]).abs() > 1e-3),
            "Duu and Duv coincide at ({u}, {v})"
        );
        // WR-160: Duuv and Duvv (and Duuu), each by two routes.
        agree(at(3, 0), d(2, 0, h, 0.0), 1e-5, "Duuu as d/du Duu");
        agree(at(2, 1), d(2, 0, 0.0, h), 1e-5, "Duuv as d/dv Duu");
        agree(at(2, 1), d(1, 1, h, 0.0), 1e-5, "Duuv as d/du Duv");
        agree(at(1, 2), d(1, 1, 0.0, h), 1e-5, "Duvv as d/dv Duv");
        agree(at(1, 2), d(0, 2, h, 0.0), 1e-5, "Duvv as d/du Dvv");
        // WR-161: Duvv and Dvvv are not the base's Buuv, and Dvvv is d/dv of Dvv.
        agree(at(0, 3), d(0, 2, 0.0, h), 1e-5, "Dvvv as d/dv Dvv");
        let base = SurfaceKind::Nurbs(twisted());
        let buuv = partial(&base, (u, v), 3, 2, 1);
        let dvvv = at(0, 3);
        assert!((0..3).any(|k| (dvvv[k] - buuv[k]).abs() > 1e-3));
    }
}

/// solidcore plan/upstream-issues/WR-164--smoffsetsurface-swapuv-with-an-extended-surface-negates-the.md
/// (b4de2149): swapping the parameters of an offset flips the normal, so its distance is
/// negated, and with an extended surface it was negated twice, leaving the offset on the wrong
/// side. Ours has no swap, but reversing a direction flips the normal in the same way, and the
/// input is the same shape: an offset of a surface that has been reversed, in u, in v and in both.
/// The offset of the reversed surface at the reversed parameters must be the same point of
/// space as the offset of the surface at the original parameters on the side the normal is on.
#[test]
fn wr_164_the_offset_of_a_reversed_surface_stays_on_the_side_its_normal_says() {
    let base = SurfaceKind::Nurbs(twisted());
    let d = 0.4;
    let plain = Offset::new(base.clone(), d).unwrap();
    let flipped = Offset::new(base.clone(), -d).unwrap();
    for dir in [Direction::U, Direction::V] {
        let rev = base.reversed(dir, &mut meter()).unwrap().value;
        let off_rev = Offset::new(rev, d).unwrap();
        let ((a, b), (c, e)) = off_rev.domain();
        for (s, t) in [(0.2, 0.3), (0.5, 0.5), (0.9, 0.1), (0.0, 1.0)] {
            // The same point of the surface: the parameter in the reversed direction is negated.
            let (u, v) = (s, t);
            let (ru, rv) = match dir {
                Direction::U => (-u, v),
                Direction::V => (u, -v),
            };
            assert!(
                a <= ru && ru <= b && c <= rv && rv <= e,
                "{dir:?} ({ru}, {rv})"
            );
            let got = off_rev.point(ru, rv, SIDES).unwrap();
            // One direction reversed turns the normal over: the offset is on the other side.
            let want = flipped.point(u, v, SIDES).unwrap();
            agree(got, want, 1e-12, "reversed offset");
            // And it is not the offset on the original side.
            let wrong = plain.point(u, v, SIDES).unwrap();
            assert!((0..3).any(|k| (got[k] - wrong[k]).abs() > 1e-3));
        }
    }
    // Both directions reversed turn the normal over twice: the same side as the original.
    let both = base
        .reversed(Direction::U, &mut meter())
        .unwrap()
        .value
        .reversed(Direction::V, &mut meter())
        .unwrap()
        .value;
    let off_both = Offset::new(both, d).unwrap();
    for (u, v) in [(0.2, 0.3), (0.7, 0.6)] {
        agree(
            off_both.point(-u, -v, SIDES).unwrap(),
            plain.point(u, v, SIDES).unwrap(),
            1e-12,
            "reversed in both",
        );
    }
}

/// solidcore plan/upstream-issues/UB-199--smapicreateplane-plane-overload-with-a-nan-length-or-width.md,
/// UB-198--smapicreatetorus-surface-overload-crashes-on-a-non-finite.md,
/// UB-200--smapicreatesphere-crashes-for-radii-from-about-7e9-and-for-a.md and
/// UB-201--smapicreatespherenopole-crashes-or-hangs-for-small-and-large.md (b4de2149): the
/// primitive constructors read outside their arrays or crashed for a NaN size, an infinite or
/// overflowing radius or centre, a sphere of radius 7e9 and over, and one of radius below 1e-4.
/// The inputs are the defects': a plane with a NaN or infinite length or width; a torus with an
/// infinite or overflowing radius or a non-finite origin; a sphere of radius 7e9, 1e10, 1e15,
/// 1e18, -6.5e9, 4e8, 1e-12, 1e-9, 1e-6, 2e-5, 5e-5 and 1e-4 and one about a non-finite centre.
/// Ours must refuse them, or build a surface whose points lie on it, and every conversion
/// to a NURBS must give a finite deviation or a refusal.
#[test]
fn ub_198_199_200_201_the_primitives_refuse_or_are_exact_at_extreme_sizes() {
    let f = frame();
    let turn: ParamBox = ((0.0, 2.0), (-0.5, 0.5));
    // A plane with a NaN length or width is refused; an infinite one is the unbounded plane.
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for (a, b) in [
            ([bad, 0.0, 0.0], [0.0, 3.0, 0.0]),
            ([3.0, 0.0, 0.0], [0.0, bad, 0.0]),
        ] {
            assert_eq!(
                Plane::new([0.0; 3], a, b, ((0.0, 1.0), (0.0, 1.0))).err(),
                Some(CurveError::NotFinite)
            );
        }
        assert_eq!(
            Plane::new(
                [bad, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                ((0.0, 1.0), (0.0, 1.0))
            )
            .err(),
            Some(CurveError::NotFinite)
        );
    }
    for dom in [
        ((f64::NAN, 1.0), (0.0, 1.0)),
        ((0.0, 1.0), (0.0, f64::NAN)),
        ((1.0, 0.0), (0.0, 1.0)),
    ] {
        assert_eq!(
            Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], dom).err(),
            Some(CurveError::SpanOutside)
        );
    }
    // The torus: non-finite origin or radius is refused; overflowing radii build or are refused.
    for bad in [f64::NAN, f64::INFINITY] {
        assert_eq!(
            Torus::new([bad, 0.0, 0.0], 3.0, 1.0, f, ((0.0, 2.0), (0.0, 2.0))).err(),
            Some(CurveError::NotFinite)
        );
        assert_eq!(
            Torus::new([0.0, bad, 0.0], 3.0, 1.0, f, ((0.0, 2.0), (0.0, 2.0))).err(),
            Some(CurveError::NotFinite)
        );
        for (major, minor) in [(bad, 1.0), (3.0, bad)] {
            assert!(Torus::new([0.0; 3], major, minor, f, ((0.0, 2.0), (0.0, 2.0))).is_err());
        }
    }
    for (major, minor) in [
        (1e308, 1e308),
        (f64::MAX, 1e293),
        (5.0, f64::MAX),
        (f64::MAX / 2.0, f64::MAX / 2.0),
        (3.0, 1.0),
    ] {
        match Torus::new([0.0; 3], major, minor, f, ((0.0, 2.0), (0.0, 2.0))) {
            Err(e) => assert!(
                matches!(e, CurveError::NotFinite | CurveError::Degenerate),
                "{major:e} {minor:e}: {e:?}"
            ),
            Ok(t) => {
                match t.point(0.3, 0.7, SIDES) {
                    Ok(p) => assert!(
                        p.iter().all(|x| x.is_finite()),
                        "{major:e} {minor:e}: {p:?}"
                    ),
                    Err(e) => assert_eq!(e, CurveError::NotFinite, "{major:e} {minor:e}"),
                }
                match t.to_nurbs(&mut meter()) {
                    Ok(n) => assert!(!n.deviation.is_nan()),
                    Err(e) => assert!(
                        matches!(
                            e,
                            CurveError::NotFinite
                                | CurveError::Nurbs(_)
                                | CurveError::Stopped(_)
                                | CurveError::Degenerate
                        ),
                        "{e:?}"
                    ),
                }
            }
        }
    }
    // The sphere, at every size of the defects, with a point on it checked against the radius.
    for radius in [
        7e9,
        1e10,
        1e15,
        1e18,
        4e8,
        1e300,
        1e-12,
        1.000_000_1e-12,
        1e-9,
        1e-6,
        2e-5,
        5e-5,
        1e-4,
        3e-4,
    ] {
        let s = Sphere::new([0.0; 3], radius, f, turn).expect("a positive finite radius");
        for (u, v) in [(0.0, 0.0), (0.3, 0.2), (1.1, -0.4), (1.9, 0.49)] {
            let p = s.point(u, v, SIDES).unwrap();
            assert!(
                (distance_over(p, [0.0; 3], radius) - 1.0).abs() <= 1e-12,
                "{radius:e} at ({u}, {v})"
            );
        }
        match s.to_nurbs(&mut meter()) {
            Ok(n) => assert!(
                n.deviation.is_finite() && n.deviation <= radius * 1e-9,
                "{radius:e}: {}",
                n.deviation
            ),
            Err(e) => assert!(
                matches!(
                    e,
                    CurveError::NotFinite | CurveError::Nurbs(_) | CurveError::Stopped(_)
                ),
                "{radius:e}: {e:?}"
            ),
        }
    }
    // A negative or zero radius, and a non-finite radius or centre, are refused.
    for radius in [-6.5e9, -6e9, 0.0, f64::NAN, f64::INFINITY] {
        assert!(
            Sphere::new([0.0; 3], radius, f, turn).is_err(),
            "{radius:e}"
        );
    }
    for c in [
        [f64::NAN, 0.0, 0.0],
        [0.0, f64::INFINITY, 0.0],
        [0.0, 0.0, f64::NEG_INFINITY],
    ] {
        assert_eq!(
            Sphere::new(c, 1.0, f, turn).err(),
            Some(CurveError::NotFinite)
        );
    }
}

/// solidcore plan/upstream-issues/UB-230--an-unbounded-smcone-whose-apex-height-overflows-recurses.md
/// (b4de2149): a cone with no height whose apex height (radius over the tangent of the
/// semi-angle) overflowed made a NaN interval and recursed until the stack ended. The inputs are
/// the defect's: bottom radius 1e300 with a semi-angle of 6e-11 or 1e-9 degrees, and -1e300 with 5.7e-11
/// and 6e-11. As a radius and a slope (tangent of the semi-angle), over a domain of height
/// 1e300 and the widest the doubles give, ours must build and evaluate, or refuse; the apex
/// must not be a NaN.
#[test]
fn ub_230_a_cone_whose_apex_is_beyond_the_doubles_builds_or_is_refused() {
    let f = frame();
    let to_slope = |deg: f64| (deg * std::f64::consts::PI / 180.0).tan();
    for (radius, deg) in [
        (1e300, 6e-11),
        (1e300, 1e-9),
        (-1e300, 5.7e-11),
        (-1e300, 6e-11),
        (1e300, 0.0),
        (1.0, 1e-300),
    ] {
        for v in [(-1.0, 1.0), (0.0, 1e300), (-1e300, 1e300), (0.0, f64::MAX)] {
            let cone = Cone::new([0.0; 3], radius, to_slope(deg), f, ((0.0, 2.0), v));
            let Ok(c) = cone else {
                let e = cone.err().unwrap();
                assert!(
                    matches!(
                        e,
                        CurveError::NotFinite | CurveError::Degenerate | CurveError::SpanOutside
                    ),
                    "{radius:e} {deg:e} {v:?}: {e:?}"
                );
                continue;
            };
            let ((u0, _), (v0, v1)) = c.domain();
            for t in [0.0, 0.5, 1.0] {
                let vv = v0 + (v1 - v0) * t;
                match c.point(u0, vv, SIDES) {
                    Ok(p) => assert!(
                        p.iter().all(|x| x.is_finite()),
                        "{radius:e} {deg:e} {v:?} at {vv:e}"
                    ),
                    Err(e) => assert!(
                        matches!(e, CurveError::NotFinite | CurveError::ParameterOutside),
                        "{e:?}"
                    ),
                }
            }
            match c.to_nurbs(&mut meter()) {
                Ok(n) => assert!(!n.deviation.is_nan()),
                Err(e) => assert!(
                    matches!(
                        e,
                        CurveError::NotFinite
                            | CurveError::Nurbs(_)
                            | CurveError::Stopped(_)
                            | CurveError::Degenerate
                            | CurveError::Unbounded
                    ),
                    "{radius:e} {deg:e} {v:?}: {e:?}"
                ),
            }
        }
    }
    // A cone with no radius and no slope is nothing, and is refused.
    assert!(Cone::new([0.0; 3], 0.0, 0.0, f, ((0.0, 2.0), (0.0, 1.0))).is_err());
}

/// solidcore plan/upstream-issues/UB-231--smsurface-createnaturaluvtrimcurves-segfaults-on-a-domain-1e.md
/// and UB-282--extrusion-nurb-with-empty-v-interval-faults-in-isclosed.md (b4de2149): a plane
/// with a domain 1e-300 wide (and one of width zero) made the natural trim curves crash; an
/// extrusion with an empty v interval had a NURB that could not be evaluated. The inputs are the
/// defects': a plane over [0, 0] x [1e-300, 1e-9] and a domain [-1e-300, 0] x [0, 10], and an
/// extrusion with an empty v interval, and one empty to 1e-9 with the ends swapped. Ours must
/// refuse an empty or reversed domain, and evaluate a very thin one.
#[test]
fn ub_231_and_ub_282_empty_and_hair_thin_domains() {
    let o = [0.0; 3];
    let (a, b) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    // Empty in u (the defect's [0, 0]), and reversed: refused.
    for dom in [
        ((0.0, 0.0), (1e-300, 1e-9)),
        ((1e-9, 0.0), (0.0, 1.0)),
        ((0.0, 1.0), (1.0, 1.0)),
    ] {
        assert_eq!(
            Plane::new(o, a, b, dom).err(),
            Some(CurveError::SpanOutside),
            "{dom:?}"
        );
    }
    // Hair thin: a plane over [-1e-300, 0] x [0, 10], and the defect's other, built and exact.
    for dom in [
        ((-1e-300, 0.0), (0.0, 10.0)),
        ((0.0, 1.0), (1e-300, 1e-9)),
        ((0.0, 5e-324), (0.0, 1.0)),
    ] {
        let p = Plane::new(o, a, b, dom).unwrap();
        let ((u0, u1), (v0, v1)) = p.domain();
        let q = p.point(u1, v1, SIDES).unwrap();
        assert_eq!(q, [u1, v1, 0.0]);
        assert_eq!(p.point(u0, v0, SIDES).unwrap(), [u0, v0, 0.0]);
        let bx = p.bound(dom, &mut meter()).unwrap();
        assert!(bx.lo[0] <= u0 && bx.hi[0] >= u1 && bx.lo[1] <= v0 && bx.hi[1] >= v1);
        assert!(p.to_nurbs(&mut meter()).is_ok());
    }
    // An extrusion with an empty or reversed v interval: refused; a hair-thin one is evaluated.
    let arc = Trimmed::whole(Leaf::Circle(
        Circle::new([0.0; 3], 2.0, frame(), (0.0, 1.0)).unwrap(),
    ))
    .unwrap();
    for along in [(1.0, 1.0), (0.0, 0.0), (1e-9, 0.0), (1e-9, -1e-9)] {
        assert_eq!(
            Extrusion::new(arc.clone(), [0.0, 0.0, 1.0], along).err(),
            Some(CurveError::SpanOutside),
            "{along:?}"
        );
    }
    let thin = Extrusion::new(arc.clone(), [0.0, 0.0, 1.0], (0.0, 1e-9)).unwrap();
    let p = thin.point(0.5, 1e-9, SIDES).unwrap();
    let q = arc.basis().point(0.5, RIGHT).unwrap();
    assert!(
        (p[0] - q[0]).abs() <= 1e-15
            && (p[1] - q[1]).abs() <= 1e-15
            && (p[2] - 1e-9).abs() <= 1e-15
    );
    assert!(thin.to_nurbs(&mut meter()).is_ok());
    assert_eq!(
        Extrusion::new(arc, [0.0; 3], (0.0, 1.0)).err(),
        Some(CurveError::Degenerate)
    );
}

/// solidcore plan/upstream-issues/UB-84--smsurface-calculateboundingbox-of-a-polar-box-recurses.md
/// and UB-173--the-base-class-box-of-an-extended-smoffsetsurface-recurses.md (b4de2149): the box
/// of a surface over a region outside what the surface's own cache covers asked the cache for its
/// box, which asked the surface again, until the stack overflowed; for an offset that allows
/// extension, and for an offset of an offset. Ours has no extension, so the same inputs are: the
/// box of an offset of an offset (to the nesting limit) over its whole domain, over a part,
/// and over a region that sticks out of the domain. Each must return a box that holds the
/// surface or refuse, and the nesting limit must refuse before the stack is in danger.
#[test]
fn ub_84_and_ub_173_nested_offsets_bound_without_recursing_forever() {
    let mut kind = SurfaceKind::Nurbs(twisted());
    let mut depth = 0;
    loop {
        match Offset::new(kind.clone(), 0.05) {
            Ok(o) => {
                depth += 1;
                kind = SurfaceKind::Offset(o);
            }
            Err(e) => {
                assert!(matches!(e, CurveError::TooDeep { .. }), "{e:?}");
                break;
            }
        }
        assert!(depth <= 64, "no nesting limit");
    }
    assert!(depth >= 1);
    let dom = kind.domain();
    let ((u0, u1), (v0, v1)) = dom;
    let whole = kind.bound(dom, &mut meter()).unwrap();
    let part_region = (
        (u0 + 0.2 * (u1 - u0), u0 + 0.6 * (u1 - u0)),
        (v0 + 0.1 * (v1 - v0), v0 + 0.9 * (v1 - v0)),
    );
    let part = kind.bound(part_region, &mut meter()).unwrap();
    // Both hold the sampled surface over the region, and the part lies inside the whole.
    for iu in 0..=8 {
        for iv in 0..=8 {
            let (u, v) = (
                u0 + (u1 - u0) * f64::from(iu) / 8.0,
                v0 + (v1 - v0) * f64::from(iv) / 8.0,
            );
            let p = kind.point(u, v, SIDES).unwrap();
            for d in 0..3 {
                assert!(
                    whole.lo[d] <= p[d] && p[d] <= whole.hi[d],
                    "whole at ({u}, {v})"
                );
            }
            if part_region.0.0 <= u
                && u <= part_region.0.1
                && part_region.1.0 <= v
                && v <= part_region.1.1
            {
                for d in 0..3 {
                    assert!(
                        part.lo[d] <= p[d] && p[d] <= part.hi[d],
                        "part at ({u}, {v})"
                    );
                }
            }
        }
    }
    for d in 0..3 {
        assert!(whole.lo[d] <= part.lo[d] + 1e-9 && part.hi[d] <= whole.hi[d] + 1e-9);
    }
    // A region outside the domain is refused, not answered by asking the cache again.
    let outside = ((u0 - 1.0, u1), (v0, v1));
    assert_eq!(
        kind.bound(outside, &mut meter()).err(),
        Some(CurveError::SpanOutside)
    );
    assert!(kind.point(u1 + 0.5, v0, SIDES).is_err());
    // The cached form of a one-level offset is a NURBS within its certified deviation.
    let one = Offset::new(SurfaceKind::Nurbs(twisted()), 0.3).unwrap();
    if let Ok(cached) = one.cached(&mut meter()) {
        let (form, dev) = cached.cache().unwrap();
        assert!(dev.is_finite());
        let p = one.point(0.4, 0.6, SIDES).unwrap();
        let q = form.surface.point(0.4, 0.6, (RIGHT, RIGHT)).unwrap();
        for d in 0..3 {
            assert!((p[d] - q[d]).abs() <= dev + 1e-12);
        }
    }
}

/// solidcore plan/upstream-issues/UB-80--smsurfofextrusion-evaluatestep-lets-order-5-overrun-sm.md
/// (b4de2149): a surface of extrusion evaluated to order 5 where its B-spline generator's
/// evaluator held 4, and wrote past a fixed array. The input is the defect's: an extrusion of a
/// B-spline curve (degrees 1 to 3) evaluated to orders 4, 5 and more. Ours returns every
/// partial, exactly: the u partials are the curve's derivatives, those in v past the first are
/// exactly zero, and the mixed ones are zero.
#[test]
fn ub_80_an_extrusion_of_a_b_spline_evaluates_to_order_five_exactly() {
    for degree in 1..=3usize {
        let n = degree + 3;
        let mut knots = vec![0.0; degree + 1];
        knots.extend((1..=(n - degree - 1)).map(|k| k as f64));
        let last = (n - degree) as f64;
        knots.extend(vec![last; degree + 1]);
        let basis = Basis::new(degree, knots).unwrap();
        let pts: Vec<[f64; 3]> = (0..basis.count())
            .map(|i| [i as f64, ((i * 5) % 3) as f64, 0.25 * (i * i) as f64])
            .collect();
        let curve = Curve::<3>::new(basis, &pts).unwrap();
        let (lo, hi) = curve.basis().domain();
        let profile = Trimmed::whole(Leaf::Nurbs(curve.clone())).unwrap();
        let s =
            SurfaceKind::Extrusion(Extrusion::new(profile, [0.5, 1.0, 2.0], (0.0, 3.0)).unwrap());
        let (u, v) = (lo + 0.37 * (hi - lo), 1.25);
        for order in [4, 5, 6, 8] {
            let all = s
                .partials_over(((u, u), (v, v)), SIDES, order, &mut meter())
                .unwrap();
            assert_eq!(all.len(), (order + 1) * (order + 2) / 2);
            let mut want = vec![[0.0; 3]; order + 1];
            curve.derivs(u, RIGHT, &mut want).unwrap();
            for i in 1..=order {
                let e = all[partial_index(i, 0).unwrap()];
                for d in 0..3 {
                    // The enclosure holds the curve's derivative (to the rounding of the sum).
                    let slack = 1e-9 * (1.0 + want[i][d].abs());
                    assert!(
                        e[d].lo() - slack <= want[i][d] && want[i][d] <= e[d].hi() + slack,
                        "degree {degree} order {i} axis {d}"
                    );
                    if i > degree {
                        assert!(
                            e[d].lo() == 0.0 && e[d].hi() == 0.0,
                            "order {i} past degree {degree}"
                        );
                    }
                }
            }
            // Mixed partials, and v past the first, are zero; v once is the direction.
            for (i, j) in [(1, 1), (0, 2), (2, 1), (0, order.min(5))] {
                if i + j <= order && !(i == 0 && j == 1) {
                    let e = all[partial_index(i, j).unwrap()];
                    assert!(
                        e.iter().all(|x| x.lo() == 0.0 && x.hi() == 0.0),
                        "({i}, {j})"
                    );
                }
            }
            let dv = all[partial_index(0, 1).unwrap()];
            for (d, want) in dv.iter().zip([0.5, 1.0, 2.0]) {
                assert!(d.lo() <= want && want <= d.hi());
            }
        }
    }
}

/// solidcore plan/upstream-issues/WR-471--revolved-arc-centred-within-tolerance-of-the-axis-has-no-pole.md
/// (b4de2149): a quarter disc's arc from (20, 0) through (14.142136, 14.142136) to (0, 20),
/// whose centre was 1.2847e-6 off the axis, turned about the axis: the sphere that was built from
/// it missed both ends of the arc and had no pole where the arc meets the axis. Ours turns the
/// arc itself: every point of the surface is the arc's point turned, so its radius from the
/// axis and height are the arc's, and at the end on the axis it is on the axis to within the
/// rounding of the arc, in every direction, and the surface does not pass through the "sphere".
#[test]
fn wr_471_a_revolved_arc_centred_next_to_the_axis_keeps_its_ends_and_its_pole() {
    let c: f64 = 1.2847e-6;
    let radius = ((20.0 - c).powi(2) + c * c).sqrt();
    // The arc in the plane y = 0, centred (c, 0, c), in half-turns from the frame's x axis (x) to
    // its y axis, which the frame maps to z: frame x = (1, 0, 0), normal (0, -1, 0) gives y = z.
    let f = Frame::<3>::new([1.0, 0.0, 0.0], [0.0, -1.0, 0.0]).unwrap();
    let a0 = (-c).atan2(20.0 - c) / std::f64::consts::PI;
    let a1 = (20.0 - c).atan2(-c) / std::f64::consts::PI;
    let arc = Circle::<3>::new([c, 0.0, c], radius, f, (a0, a1)).unwrap();
    let start = arc.point(a0, RIGHT).unwrap();
    let end = arc.point(a1, Limit::FromLeft).unwrap();
    assert!(
        (start[0] - 20.0).abs() < 1e-9 && start[2].abs() < 1e-9,
        "{start:?}"
    );
    assert!(
        end[0].abs() < 1e-9 && (end[2] - 20.0).abs() < 1e-9,
        "{end:?}"
    );
    let profile = Trimmed::whole(Leaf::Circle(arc.clone())).unwrap();
    let axis = Frame::<3>::standard().unwrap();
    let s = Revolution::new(profile, [0.0, 0.0, 0.0], axis, (0.0, 2.0)).unwrap();
    for k in 0..=8 {
        let t = a0 + (a1 - a0) * f64::from(k) / 8.0;
        let p = arc.point(t, RIGHT).unwrap();
        let radius_from_axis = p[0].hypot(p[1]);
        for iu in 0..=8 {
            let u = 2.0 * f64::from(iu) / 8.0;
            let q = s.point(u, t, SIDES).unwrap();
            // The turned point: same height, same distance from the axis.
            assert!((q[2] - p[2]).abs() <= 1e-12, "t = {t}");
            assert!(
                (q[0].hypot(q[1]) - radius_from_axis).abs() <= 1e-12,
                "t = {t} u = {u}"
            );
        }
    }
    // The ends: the start has the arc's own radius (20) in every direction; the end is a pole.
    for iu in 0..=8 {
        let u = 2.0 * f64::from(iu) / 8.0;
        let q0 = s.point(u, a0, SIDES).unwrap();
        assert!((q0[0].hypot(q0[1]) - start[0]).abs() <= 1e-9 && q0[2].abs() <= 1e-9);
        let q1 = s.point(u, a1, SIDES).unwrap();
        assert!(q1[0].hypot(q1[1]) <= 1e-9, "u = {u}: {q1:?}");
        assert!((q1[2] - 20.0).abs() <= 1e-9);
    }
    // The surface is not the sphere of the arc's radius about the origin: the midpoint of the arc
    // turned is the torus's, off that sphere by about the centre's offset.
    let mid = s.point(0.0, (a0 + a1) / 2.0, SIDES).unwrap();
    let on_axis_sphere = (mid[0] * mid[0] + mid[1] * mid[1] + mid[2] * mid[2]).sqrt();
    assert!((on_axis_sphere - radius).abs() > 1e-7 && (on_axis_sphere - radius).abs() < 1e-5);
}
