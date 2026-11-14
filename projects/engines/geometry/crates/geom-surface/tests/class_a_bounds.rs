//! Piece G346's enclosures in `class_a`, beyond their values: where the centred form is used and
//! where it must not be, how tight each enclosure is, and how the surfaces' partials are read.
//!
//! - **A side's range.** [`Side::domain`] is its pcurve's domain.
//! - **Centred where smooth.** Over a span with no pcurve break strictly inside it and whose box
//!   touches no surface break, the centred form cuts the curvature's enclosure to well under half
//!   the direct one: next to a pcurve break at either end of the span, and between the surface's
//!   knots in u and in v.
//! - **Direct where not smooth, and still certified.** Over a span across a pcurve's corner, or
//!   whose box crosses a knot where the surface's second partials jump, on one side or both, the
//!   centred form would miss the values on the far side; the enclosure holds them.
//! - **The remainder's box.** A NURBS box is enclosed by Taylor's theorem with its remainder over
//!   the box widened to 1/128 of its smooth piece, never past the piece's knots: next to a knot
//!   beyond which the second partials are large, and on a piece whose higher partials grow away
//!   from the box, the gap's enclosure stays tight, and it holds the values.
//! - **Narrow spans.** Over 2⁻³⁰ the enclosures are tight on a knotted patch, for a pcurve
//!   moving in both directions, only in u, or only in v.
//! - **A knot line is read from both sides.** A pcurve along a knot line where the curvature jumps
//!   gets an enclosure holding the curvature on each side.
//! - **Pcurves.** One whose derivatives cannot be bounded still has its gap and normal angle
//!   bounded over a span; a curved one's centred enclosures hold the values inside.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests: oracles and generators, and a failure stops the test"
)]

use geom_curve::{Basis as CurveBasis, CurveKind, Line, Offset, OffsetLaw, Trimmed};
use geom_nurbs::{Basis, Curve, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::class_a::{self, Measure, Side};
use geom_surface::{Plane, SurfaceKind};

const ALL: [Measure; 4] = [
    Measure::Gap,
    Measure::NormalAngle,
    Measure::Curvature,
    Measure::CurvatureChange,
];

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The plane z = `lift` over x in [−1, 3], y in [0, 1], with u = x and v = y.
fn flat(lift: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, lift],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-1.0, 3.0), (0.0, 1.0)),
        )
        .unwrap(),
    )
}

/// The Greville abscissae of a clamped basis of degree `p` on `knots`: control points there make
/// the coordinate equal to the parameter.
fn greville(p: usize, knots: &[f64]) -> Vec<f64> {
    let n = knots.len() - p - 1;
    (0..n)
        .map(|i| knots[i + 1..=i + p].iter().sum::<f64>() / p as f64)
        .collect()
}

/// The graph z = Σ zᵢ·gⱼ·Bᵢ(u)·Bⱼ(v) over [0, 1]², with u = x and v = y: degree `p` on `ku` in
/// u, degree 1 on `kv` in v, heights `z` along u and factors `g` along v.
fn spline(p: usize, ku: &[f64], z: &[f64], kv: &[f64], g: &[f64]) -> SurfaceKind {
    let (xs, ys) = (greville(p, ku), greville(1, kv));
    assert_eq!((xs.len(), ys.len()), (z.len(), g.len()));
    let mut pts = Vec::new();
    for (x, zi) in xs.iter().zip(z) {
        for (y, gj) in ys.iter().zip(g) {
            pts.push([*x, *y, zi * gj]);
        }
    }
    SurfaceKind::Nurbs(
        Surface::new(
            Basis::new(p, ku.to_vec()).unwrap(),
            Basis::new(1, kv.to_vec()).unwrap(),
            &pts,
        )
        .unwrap(),
    )
}

/// A cubic in u with knots at 1/4, 1/2 and 3/4, and one knot at 0.8 in v.
fn knotted() -> SurfaceKind {
    spline(
        3,
        &[0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0, 1.0],
        &[0.1, -0.2, 0.4, 0.3, -0.1, 0.5, 0.2],
        &[0.0, 0.0, 0.8, 1.0, 1.0],
        &[1.0, 1.4, 1.1],
    )
}

/// A cubic in u with a double knot at 1/2, so its second partials jump there.
fn creased() -> SurfaceKind {
    spline(
        3,
        &[0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0],
        &[0.0, 0.1, 0.5, -0.3, 0.4, 0.2],
        &[0.0, 0.0, 1.0, 1.0],
        &[1.0, 1.3],
    )
}

/// The Bernstein coefficients on [a, a + l] of the cubic with Taylor coefficients `q` at a
/// (q₀ + q₁·(u − a) + q₂·(u − a)² + q₃·(u − a)³).
fn bezier(q: [f64; 4], l: f64) -> [f64; 4] {
    let s = [q[0], q[1] * l, q[2] * l * l, q[3] * l * l * l];
    [
        s[0],
        s[0] + s[1] / 3.0,
        s[0] + 2.0 * s[1] / 3.0 + s[2] / 3.0,
        s[0] + s[1] + s[2] + s[3],
    ]
}

/// Two cubic pieces meeting at a triple knot u = 1/2: on the side of the box, z = 1/2 +
/// k·(u − c)³, whose second partial is 0 at c and grows away from it; across the knot, a
/// parabola whose second partial is 2·10⁴. `left` says on which side of the knot c lies.
fn cliff(c: f64, left: bool) -> SurfaceKind {
    let k = 1e5;
    // Taylor coefficients of the cubic at a, and of the parabola at a, each continuous at 1/2.
    let cubic = |a: f64| {
        let d = a - c;
        [0.5 + k * d * d * d, 3.0 * k * d * d, 3.0 * k * d, k]
    };
    let z_knot = cubic(0.5)[0];
    let parabola = |a: f64| {
        let d = a - 0.5;
        [z_knot + 1e4 * d * d, 2e4 * d, 1e4, 0.0]
    };
    let (l, r) = if left {
        (bezier(cubic(0.0), 0.5), bezier(parabola(0.5), 0.5))
    } else {
        (bezier(parabola(0.0), 0.5), bezier(cubic(0.5), 0.5))
    };
    let z = [l[0], l[1], l[2], l[3], r[1], r[2], r[3]];
    spline(
        3,
        &[0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0],
        &z,
        &[0.0, 0.0, 1.0, 1.0],
        &[1.0, 1.0],
    )
}

/// The polyline through `pts` (u, v) on t in [0, 1], a degree-1 B-spline with knots `inner`.
fn polyline(inner: &[f64], pts: &[[f64; 2]]) -> CurveKind<2> {
    let mut knots = vec![0.0, 0.0];
    knots.extend_from_slice(inner);
    knots.extend([1.0, 1.0]);
    CurveKind::Nurbs(Curve::new(Basis::new(1, knots).unwrap(), pts).unwrap())
}

/// The segment from `from` along `d` on t in [0, 1].
fn segment(from: [f64; 2], d: [f64; 2]) -> CurveKind<2> {
    CurveKind::Line(Line::new(from, d, (0.0, 1.0)).unwrap())
}

fn sides<'a>(
    a: &'a SurfaceKind,
    p: &'a CurveKind<2>,
    b: &'a SurfaceKind,
    q: &'a CurveKind<2>,
) -> [Side<'a>; 2] {
    [
        Side {
            surface: a,
            pcurve: p,
            turned: false,
        },
        Side {
            surface: b,
            pcurve: q,
            turned: false,
        },
    ]
}

fn value_at(m: Measure, s: [Side<'_>; 2], t: f64, limit: Limit) -> (f64, f64) {
    let v = class_a::at(m, s, t, limit, &mut meter())
        .unwrap()
        .value()
        .unwrap();
    (v.lo(), v.hi())
}

/// The measure over [lo, hi], centred at its middle, or direct.
fn over(m: Measure, s: [Side<'_>; 2], lo: f64, hi: f64, centred: bool) -> (f64, f64) {
    let c = f64::midpoint(lo, hi);
    let at = class_a::at(m, s, c, Limit::FromRight, &mut meter()).unwrap();
    let v = class_a::over(m, s, (lo, hi), centred.then_some((c, &at)), &mut meter())
        .unwrap()
        .value()
        .unwrap();
    (v.lo(), v.hi())
}

fn width((lo, hi): (f64, f64)) -> f64 {
    hi - lo
}

/// How far the values at `n` + 1 even parameters of [lo, hi] (from both sides) fall outside `e`.
fn miss(m: Measure, s: [Side<'_>; 2], (lo, hi): (f64, f64), e: (f64, f64), n: u32) -> f64 {
    let mut worst = 0.0_f64;
    for k in 0..=n {
        let t = lo + (hi - lo) * f64::from(k) / f64::from(n);
        for limit in [Limit::FromLeft, Limit::FromRight] {
            let v = value_at(m, s, t, limit);
            let mid = f64::midpoint(v.0, v.1);
            worst = worst.max(e.0 - mid).max(mid - e.1);
        }
    }
    worst
}

#[test]
fn a_side_s_range_is_its_pcurve_s_domain() {
    let s = flat(0.0);
    let p = CurveKind::Line(Line::new([0.0, 0.5], [0.1, 0.0], (-0.5, 2.0)).unwrap());
    let side = Side {
        surface: &s,
        pcurve: &p,
        turned: false,
    };
    assert_eq!(side.domain(), (-0.5, 2.0));
    let k = polyline(&[0.25], &[[0.0, 0.0], [0.0, 0.25], [0.0, 1.0]]);
    let side = Side {
        surface: &s,
        pcurve: &k,
        turned: true,
    };
    assert_eq!(side.domain(), (0.0, 1.0));
    assert!(side.breaks().contains(&0.25), "{:?}", side.breaks());
}

#[test]
fn the_centred_form_is_used_wherever_the_parts_are_smooth() {
    let (a, b) = (flat(0.0), knotted());
    // Straight, u = 0.3 + 0.1·t, v = t, with a break at t = 1/2: its box lies between the knots
    // u = 1/4 and 1/2, and below or above v = 0.8.
    let p = polyline(&[0.5], &[[0.3, 0.0], [0.35, 0.5], [0.4, 1.0]]);
    let s = sides(&a, &p, &b, &p);
    let h = 1.0 / 64.0;
    for (lo, hi) in [
        (0.5, 0.5 + h),
        (0.5 - h, 0.5),
        (0.3, 0.3 + h),
        (0.6, 0.6 + h),
        (0.85, 0.85 + h),
    ] {
        let ratio = [Measure::Curvature, Measure::CurvatureChange]
            .map(|m| width(over(m, s, lo, hi, true)) / width(over(m, s, lo, hi, false)));
        assert!(ratio[0].min(ratio[1]) < 0.4, "[{lo}, {hi}]: {ratio:?}");
    }
}

#[test]
fn across_a_corner_or_a_crease_the_enclosure_holds_both_sides() {
    let a = flat(0.0);
    let straight = segment([0.45, 0.0], [0.1, 1.0]);
    // The surface's second partials jump across u = 1/2, crossed at t = 1/2.
    let crease = creased();
    // A pcurve whose direction turns at t = 1/2, on a surface smooth there.
    let smooth = knotted();
    let corner = polyline(&[0.5], &[[0.3, 0.0], [0.45, 0.5], [0.3, 1.0]]);
    let joins = [
        (
            "crease, both",
            sides(&crease, &straight, &crease, &straight),
        ),
        ("crease, one", sides(&a, &straight, &crease, &straight)),
        ("corner, both", sides(&smooth, &corner, &a, &corner)),
        ("corner, one", sides(&a, &straight, &smooth, &corner)),
    ];
    for (name, s) in joins {
        // A long span, and a short one where the centred form would be far narrower than the
        // jump (neither centre, nor any parameter sampled, is the crossing itself).
        for (span, n) in [((0.41, 0.6), 38), ((0.4999, 0.50005), 29)] {
            for m in ALL {
                let centred = over(m, s, span.0, span.1, true);
                let worst = miss(m, s, span, centred, n);
                assert!(
                    worst <= 0.0,
                    "{name} {m:?} {span:?}: {centred:?} misses by {worst}"
                );
            }
        }
    }
}

#[test]
fn the_remainder_s_box_stays_on_its_piece_and_holds_the_values() {
    let a = flat(0.0);
    let d = 0.001;
    for (c, left) in [(0.498, true), (0.502, false)] {
        let b = cliff(c, left);
        let p = segment([c - d, 0.5], [2.0 * d, 0.0]);
        let s = sides(&a, &p, &b, &p);
        let direct = over(Measure::Gap, s, 0.0, 1.0, false);
        let worst = miss(Measure::Gap, s, (0.0, 1.0), direct, 40);
        assert!(worst <= 0.0, "cliff {c}: {direct:?} misses by {worst}");
        assert!(width(direct) < 2e-3, "cliff {c}: {direct:?}");
    }
}

#[test]
fn narrow_spans_on_a_knotted_patch_are_tight() {
    let (a, b) = (flat(0.0), knotted());
    let narrow = 1.0 / 1_073_741_824.0;
    for (name, p) in [
        ("both", segment([0.3, 0.1], [0.1, 0.5])),
        ("v only", segment([0.3, 0.0], [0.0, 1.0])),
        ("u only", segment([0.3, 0.4], [0.1, 0.0])),
    ] {
        let s = sides(&a, &p, &b, &p);
        for m in ALL {
            let lo = 0.37;
            let centred = width(over(m, s, lo, lo + narrow, true));
            assert!(centred < 1e-6, "{name} {m:?}: {centred}");
        }
    }
}

#[test]
fn a_knot_line_is_read_from_both_sides() {
    let (a, b) = (flat(0.0), creased());
    let at = |u: f64| segment([u, 0.0], [0.0, 1.0]);
    let (on, left, right) = (at(0.5), at(0.5 - 1e-9), at(0.5 + 1e-9));
    for m in [Measure::Curvature, Measure::CurvatureChange] {
        for limit in [Limit::FromLeft, Limit::FromRight] {
            let v = value_at(m, sides(&a, &on, &b, &on), 0.4, limit);
            let l = value_at(m, sides(&a, &left, &b, &left), 0.4, limit);
            let r = value_at(m, sides(&a, &right, &b, &right), 0.4, limit);
            for side in [l, r] {
                let mid = f64::midpoint(side.0, side.1);
                assert!(
                    v.0 <= mid + 1e-5 && mid - 1e-5 <= v.1,
                    "{m:?} {limit:?}: {v:?} misses {side:?}"
                );
            }
        }
    }
}

#[test]
fn a_pcurve_whose_derivatives_are_unbounded_still_has_its_gap_and_angle_bounded() {
    let (a, b) = (flat(0.0), knotted());
    // A pcurve whose speed underflows: its offset's derivatives cannot be bounded, though its box
    // can (the basis's, widened by the distance).
    let still = Line::new([0.3, 0.5], [0.0, 1e-300], (0.0, 1.0)).unwrap();
    let off = CurveKind::Offset(
        Offset::new(
            Trimmed::whole(CurveBasis::Line(still)).unwrap(),
            OffsetLaw::Planar { distance: 0.01 },
        )
        .unwrap(),
    );
    let line = segment([0.3, 0.0], [0.0, 1.0]);
    let s = sides(&a, &line, &b, &off);
    for m in [Measure::Gap, Measure::NormalAngle] {
        let v = class_a::over(m, s, (0.2, 0.4), None, &mut meter())
            .unwrap()
            .value()
            .unwrap();
        assert!(0.0 <= v.lo() && v.hi() < 1.5, "{m:?}: {v:?}");
    }
    for m in [Measure::Curvature, Measure::CurvatureChange] {
        assert_eq!(
            class_a::over(m, s, (0.2, 0.4), None, &mut meter()),
            Err(geom_curve::CurveError::Unbounded),
            "{m:?}"
        );
    }
}

#[test]
fn a_curved_pcurve_s_centred_enclosures_hold_the_values() {
    let (a, b) = (flat(0.0), knotted());
    // u = 0.26 + 0.23·t², v = t, inside the knots u = 1/4 and 1/2: its second derivative is
    // (0.46, 0), which the tangent's derivative along t needs.
    let bend = CurveKind::Nurbs(
        Curve::new(
            Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap(),
            &[[0.26, 0.0], [0.26, 0.5], [0.49, 1.0]],
        )
        .unwrap(),
    );
    // A quarter turn in a short way, from (0.3, 0.3) to (0.45, 0.45): its tangent turns fast,
    // and the curvature change across a cubic depends on the direction.
    let turn = CurveKind::Nurbs(
        Curve::new(
            Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap(),
            &[[0.3, 0.3], [0.45, 0.3], [0.45, 0.45]],
        )
        .unwrap(),
    );
    // A quarter turn in a hundredth: the tangent turns far faster than the point moves, so over a
    // short span nearly all of each part's change along t comes through the tangent's derivative.
    let spin = CurveKind::Nurbs(
        Curve::new(
            Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap(),
            &[[0.35, 0.35], [0.36, 0.35], [0.36, 0.36]],
        )
        .unwrap(),
    );
    let short = 1.0 / 1_048_576.0;
    let quick = [0.3, 0.5, 0.7].map(|t| (t, t + short));
    for (p, spans) in [
        (
            &bend,
            &[
                (0.55, 0.7),
                (0.1, 0.4),
                (0.6, 0.61),
                (0.2, 0.21),
                (0.9, 0.905),
            ][..],
        ),
        (
            &turn,
            &[(0.45, 0.46), (0.5, 0.505), (0.1, 0.2), (0.7, 0.701)][..],
        ),
        (&spin, &quick[..]),
    ] {
        let s = sides(&a, p, &b, p);
        for &span in spans {
            for m in ALL {
                let centred = over(m, s, span.0, span.1, true);
                let worst = miss(m, s, span, centred, 30);
                assert!(
                    worst <= 0.0,
                    "{m:?} {span:?}: {centred:?} misses by {worst}"
                );
            }
        }
    }
}
