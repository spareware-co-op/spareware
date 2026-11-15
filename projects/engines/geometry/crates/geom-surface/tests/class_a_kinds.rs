//! Piece G346's enclosures in `class_a` on the kinds and at the places the other tests leave out:
//! a domain that ends on its base's crease, a box next to a knot of a patch of high degree, the
//! analytic kinds' closed forms and the swept kinds' iso-lines.
//!
//! - **A domain's end is read from inside it.** An offset split at its base's crease ends there,
//!   and beyond the end its base is the other piece: at a parameter and over a span, a pcurve
//!   along that end gets the piece's own point and normal, from either side asked.
//! - **The remainder's box reaches away from a knot.** Over 2⁻³⁰ next to a knot of a patch of
//!   degree 7, on either side, the curvature and its change stay tight.
//! - **The analytic kinds' closed forms.** Over an eighth of a turn of a cylinder against a plane,
//!   the right angle between the normals is enclosed to the last bits and the curvature (1/r) and
//!   its change (0) within pinned widths, as on a sphere the gap and the angle: each narrower
//!   than the Taylor form about the box's centre would give.
//! - **The swept kinds' iso-lines.** Over 2⁻³⁰ along a profile's parameter, of an extrusion in u
//!   and a revolution in v, every measure stays tight.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "tests: oracles and generators, and a failure stops the test"
)]

use geom_curve::{Basis as Leaf, CurveKind, Frame, Line, Trimmed};
use geom_nurbs::{Basis, Curve, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::class_a::{self, Measure, Side};
use geom_surface::{Cylinder, Extrusion, Offset, Plane, Revolution, Sphere, SurfaceKind};

const ALL: [Measure; 4] = [
    Measure::Gap,
    Measure::NormalAngle,
    Measure::Curvature,
    Measure::CurvatureChange,
];

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The plane through `at` spanned by `du` and `dv`, over u in [−1, 3] and v in [0, 1].
fn plane(at: [f64; 3], du: [f64; 3], dv: [f64; 3]) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(at, du, dv, ((-1.0, 3.0), (0.0, 1.0))).unwrap())
}

/// The Greville abscissae of a clamped basis of degree `p` on `knots`.
fn greville(p: usize, knots: &[f64]) -> Vec<f64> {
    let n = knots.len() - p - 1;
    (0..n)
        .map(|i| knots[i + 1..=i + p].iter().sum::<f64>() / p as f64)
        .collect()
}

/// The graph z = Σ zᵢ·gⱼ·Bᵢ(u)·Bⱼ(v) over [0, 1]², u = x and v = y: degree `p` on `ku` in u,
/// degree 1 with no inner knot in v, heights `z` along u and factors `g` along v.
fn spline(p: usize, ku: &[f64], z: &[f64], g: [f64; 2]) -> SurfaceKind {
    let xs = greville(p, ku);
    assert_eq!(xs.len(), z.len());
    let mut pts = Vec::new();
    for (x, zi) in xs.iter().zip(z) {
        for (y, gj) in [0.0, 1.0].iter().zip(g) {
            pts.push([*x, *y, zi * gj]);
        }
    }
    SurfaceKind::Nurbs(
        Surface::new(
            Basis::new(p, ku.to_vec()).unwrap(),
            Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap(),
            &pts,
        )
        .unwrap(),
    )
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

#[test]
fn an_offset_split_at_its_base_s_crease_is_read_from_inside() {
    // A crease at u = 1/2 (a triple knot): z = 0 before it, z = 0.6·(u − 1/2) after it.
    let base = spline(
        3,
        &[0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0],
        &[0.0, 0.0, 0.0, 0.0, 0.1, 0.2, 0.3],
        [1.0, 1.0],
    );
    let whole = Offset::new(base, 0.1).unwrap();
    let left = SurfaceKind::Offset(whole.restricted(((0.0, 0.5), (0.0, 1.0))).unwrap());
    let right = SurfaceKind::Offset(whole.restricted(((0.5, 1.0), (0.0, 1.0))).unwrap());
    // Each piece's offset is a plane: z = 0.1, and the sloped plane moved 0.1 along its normal
    // (−0.6, 0, 1)/√1.36.
    let s = 1.36_f64.sqrt();
    let flat = plane([0.0, 0.0, 0.1], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let slope = plane(
        [0.5 - 0.06 / s, 0.0, 0.1 / s],
        [1.0, 0.0, 0.6],
        [0.0, 1.0, 0.0],
    );
    // The end u = 1/2, the left piece's upper end and the right piece's lower end; on the planes
    // the same line.
    let end = segment([0.5, 0.0], [0.0, 1.0]);
    let start = segment([0.0, 0.0], [0.0, 1.0]);
    for (name, s) in [
        ("left", sides(&left, &end, &flat, &end)),
        ("right", sides(&right, &end, &slope, &start)),
    ] {
        for m in [Measure::Gap, Measure::NormalAngle] {
            for limit in [Limit::FromLeft, Limit::FromRight] {
                for t in [0.3, 0.7] {
                    let v = value_at(m, s, t, limit);
                    assert!(v.1 < 1e-9, "{name} {m:?} {limit:?} at {t}: {v:?}");
                }
            }
            for (lo, hi) in [(0.2, 0.4), (0.6, 0.6 + 1.0 / 1_048_576.0)] {
                let v = over(m, s, lo, hi, true);
                assert!(v.1 < 1e-6, "{name} {m:?} [{lo}, {hi}]: {v:?}");
            }
        }
    }
}

#[test]
fn narrow_spans_next_to_a_knot_are_tight() {
    let a = plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    // Degree 7 in u with one knot at 1/2: its partials to the sixth order, which the curvature
    // change's remainder reads, are not zero.
    let mut ku = vec![0.0; 8];
    ku.push(0.5);
    ku.extend([1.0; 8]);
    let b = spline(
        7,
        &ku,
        &[0.1, -0.2, 0.4, 0.3, -0.1, 0.5, 0.2, -0.3, 0.1],
        [1.0, 1.3],
    );
    let narrow = 1.0 / 1_073_741_824.0;
    let near = 1.0 / 16_777_216.0;
    // u = 0.4 + 0.1·t up to the knot, and u = 0.5 + 0.1·t from it: the spans' boxes stay about
    // 6·10⁻⁹ clear of the knot, below it and above it.
    for (name, p, lo) in [
        (
            "below",
            segment([0.4, 0.4], [0.1, 0.0]),
            1.0 - near - narrow,
        ),
        ("above", segment([0.5, 0.4], [0.1, 0.0]), near),
    ] {
        let s = sides(&a, &p, &b, &p);
        for m in ALL {
            let centred = width(over(m, s, lo, lo + narrow, true));
            assert!(centred < 1e-6, "{name} {m:?}: {centred}");
        }
    }
}

#[test]
fn the_analytic_kinds_are_read_by_their_closed_forms() {
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let r = 2.0;
    let flat = plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let tube =
        SurfaceKind::Cylinder(Cylinder::new([0.0; 3], r, f, ((0.0, 1.5), (-1.0, 2.0))).unwrap());
    let ball = SurfaceKind::Sphere(Sphere::new([0.0; 3], r, f, ((0.0, 1.5), (-0.4, 0.4))).unwrap());
    // A quarter turn in u while v moves, on each; its quarter t in [1/4, 1/2] spans an eighth of
    // a turn.
    let p = segment([0.2, -0.3], [0.5, 0.6]);
    let q = segment([0.0, 0.2], [1.0, 0.3]);
    let direct = |m: Measure, b: &SurfaceKind| -> (f64, f64) {
        let v = class_a::over(m, sides(b, &p, &flat, &q), (0.25, 0.5), None, &mut meter())
            .unwrap()
            .value()
            .unwrap();
        (v.lo(), v.hi())
    };
    // The cylinder's normals are level and the plane's vertical, a right angle all along.
    let right = direct(Measure::NormalAngle, &tube);
    let quarter = core::f64::consts::FRAC_PI_2;
    assert!(
        right.0 <= quarter && quarter <= right.1 && width(right) < 1e-12,
        "{right:?}"
    );
    // Its curvature against the plane's is 1/r all along, and the change 0. The Taylor form
    // about the box's centre would give 0.96 and 0.076 here.
    let k = direct(Measure::Curvature, &tube);
    assert!(k.0 <= 1.0 / r && 1.0 / r <= k.1 && width(k) < 0.92, "{k:?}");
    let c = direct(Measure::CurvatureChange, &tube);
    assert!(c.0 == 0.0 && c.1 < 0.07, "{c:?}");
    // On the sphere, where the Taylor form would give 1.08 and 1.51.
    let gap = direct(Measure::Gap, &ball);
    assert!(width(gap) < 1.0, "{gap:?}");
    let angle = direct(Measure::NormalAngle, &ball);
    assert!(width(angle) < 1.2, "{angle:?}");
}

#[test]
fn the_swept_kinds_iso_lines_are_tight() {
    let flat = plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let profile = Trimmed::whole(Leaf::Nurbs(
        Curve::new(
            Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap(),
            &[
                [2.0, 0.0, 0.0],
                [3.0, 0.5, 1.0],
                [2.5, -0.5, 2.0],
                [4.0, 0.0, 3.0],
                [3.0, 1.0, 4.0],
            ],
        )
        .unwrap(),
    ))
    .unwrap();
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let extrusion = SurfaceKind::Extrusion(
        Extrusion::new(profile.clone(), [1.0, -1.0, 0.5], (0.0, 1.0)).unwrap(),
    );
    let revolution =
        SurfaceKind::Revolution(Revolution::new(profile, [0.0; 3], f, (0.0, 1.0)).unwrap());
    let narrow = 1.0 / 1_073_741_824.0;
    // Along the profile's parameter only: u on the extrusion, v on the revolution.
    for (name, b, p) in [
        ("extrusion", &extrusion, segment([0.1, 0.4], [0.2, 0.0])),
        ("revolution", &revolution, segment([0.4, 0.1], [0.0, 0.2])),
    ] {
        let s = sides(b, &p, &flat, &p);
        for m in ALL {
            let centred = width(over(m, s, 0.37, 0.37 + narrow, true));
            assert!(centred < 1e-6, "{name} {m:?}: {centred}");
        }
    }
}
