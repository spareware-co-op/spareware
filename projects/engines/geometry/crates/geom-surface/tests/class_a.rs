//! Piece G346's Class-A maths in `geom-surface` (`class_a`): the four continuity measures across
//! constructed joins whose answers are known in closed form, and the enclosures' laws.
//!
//! - **Constructed joins.** A flat strip z = 0 for x ≤ 0 meets, along x = 0, a graph z = f(x) for
//!   x ≥ 0, each an exact Bezier patch: z = x² is a G1 join with a jump of 2 in curvature (G2 = 2,
//!   G3 = 0), z = x³ a G2 join with κ′ jumping by 6, z = x⁴ a G3 join; a strip lifted by 0.5 has a
//!   gap of 0.5; z = x meets at an angle of π/4; a cylinder of radius 4 rolling off the strip
//!   jumps in curvature by 1/4. Each value at a parameter, and over the whole edge, holds the
//!   closed form.
//! - **Senses.** Turning both faces changes no measure; turning one turns G1 by π.
//! - **Enclosures hold the point values.** Over random spans of random graphs (a printed seed;
//!   `SPAREKIT_SEED` replays one), the enclosure over the span, with and without its centre,
//!   holds the value at random parameters inside it; the centred enclosure over a short span is
//!   tight.
//! - **Undefined is refused.** A pcurve that stops (a zero tangent) refuses G2 and G3; a pcurve
//!   outside its surface's domain is refused; a centre outside its span is refused.
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

use core::f64::consts::PI;
use core::num::NonZeroU64;

use geom_curve::{CurveError, CurveKind, Frame, Line};
use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::class_a::{self, Measure, Side};
use geom_surface::{Cylinder, Plane, SurfaceKind};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

const ALL: [Measure; 4] = [
    Measure::Gap,
    Measure::NormalAngle,
    Measure::Curvature,
    Measure::CurvatureChange,
];

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The flat strip z = `lift` over x in [−1, 0], y in [0, 1].
fn strip(lift: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, lift],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-1.0, 0.0), (0.0, 1.0)),
        )
        .unwrap(),
    )
}

/// C(n, k) as a double.
fn choose(n: usize, k: usize) -> f64 {
    (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64)
}

/// The graph z = Σ cₖ·xᵏ over x in [0, 1], y in [0, 1], as an exact Bezier patch of degree
/// `c.len() − 1` in u = x and 1 in v = y: the Bernstein coefficients of the polynomial.
fn graph(c: &[f64]) -> SurfaceKind {
    let p = c.len() - 1;
    let z: Vec<f64> = (0..=p)
        .map(|i| (0..=i).map(|k| c[k] * choose(i, k) / choose(p, k)).sum())
        .collect();
    let mut knots = vec![0.0; p + 1];
    knots.extend(vec![1.0; p + 1]);
    let bu = Basis::new(p, knots).unwrap();
    let bv = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let mut pts = Vec::new();
    for (i, zi) in z.iter().enumerate() {
        for y in [0.0, 1.0] {
            pts.push([i as f64 / p as f64, y, *zi]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(bu, bv, &pts).unwrap())
}

/// The pcurve u = `u`, v = t over t in [0, 1].
fn along(u: f64) -> CurveKind<2> {
    CurveKind::Line(Line::new([u, 0.0], [0.0, 1.0], (0.0, 1.0)).unwrap())
}

/// The join of the strip (its edge at u = 0) and `other` (its edge at u = 0).
fn sides<'a>(
    strip: &'a SurfaceKind,
    other: &'a SurfaceKind,
    pcurve: &'a CurveKind<2>,
) -> [Side<'a>; 2] {
    [
        Side {
            surface: strip,
            pcurve,
            turned: false,
        },
        Side {
            surface: other,
            pcurve,
            turned: false,
        },
    ]
}

fn value_at(m: Measure, s: [Side<'_>; 2], t: f64) -> (f64, f64) {
    let v = class_a::at(m, s, t, Limit::FromRight, &mut meter())
        .unwrap()
        .value()
        .unwrap();
    (v.lo(), v.hi())
}

/// The measure over [lo, hi], centred at its middle.
fn value_over(m: Measure, s: [Side<'_>; 2], lo: f64, hi: f64) -> (f64, f64) {
    let c = 0.5 * lo + 0.5 * hi;
    let at = class_a::at(m, s, c, Limit::FromRight, &mut meter()).unwrap();
    let v = class_a::over(m, s, (lo, hi), Some((c, &at)), &mut meter())
        .unwrap()
        .value()
        .unwrap();
    (v.lo(), v.hi())
}

fn holds((lo, hi): (f64, f64), x: f64, slack: f64) -> bool {
    lo <= x + slack && x - slack <= hi
}

/// Each measure at t = 0.3 and over [0, 1], with its expected value.
fn expect(s: [Side<'_>; 2], want: [f64; 4]) {
    for (m, w) in ALL.into_iter().zip(want) {
        let at = value_at(m, s, 0.3);
        assert!(holds(at, w, 0.0), "{m:?} at 0.3: {at:?}, want {w}");
        assert!(at.1 - at.0 < 1e-9, "{m:?} at 0.3 is wide: {at:?}");
        let over = value_over(m, s, 0.0, 1.0);
        assert!(
            holds(over, w, 0.0),
            "{m:?} over the edge: {over:?}, want {w}"
        );
    }
}

#[test]
fn a_parabola_rolling_off_a_strip_is_g1_with_a_curvature_jump_of_two() {
    let (a, b, p) = (strip(0.0), graph(&[0.0, 0.0, 1.0]), along(0.0));
    expect(sides(&a, &b, &p), [0.0, 0.0, 2.0, 0.0]);
}

#[test]
fn a_cubic_rolling_off_a_strip_is_g2_with_a_curvature_change_of_six() {
    let (a, b, p) = (strip(0.0), graph(&[0.0, 0.0, 0.0, 1.0]), along(0.0));
    expect(sides(&a, &b, &p), [0.0, 0.0, 0.0, 6.0]);
}

#[test]
fn a_quartic_rolling_off_a_strip_is_g3() {
    let (a, b, p) = (strip(0.0), graph(&[0.0, 0.0, 0.0, 0.0, 1.0]), along(0.0));
    expect(sides(&a, &b, &p), [0.0, 0.0, 0.0, 0.0]);
}

#[test]
fn a_lifted_strip_has_its_lift_as_the_gap() {
    let (a, b, p) = (strip(0.5), graph(&[0.0, 0.0]), along(0.0));
    expect(sides(&a, &b, &p), [0.5, 0.0, 0.0, 0.0]);
}

#[test]
fn a_crease_of_a_slope_one_is_a_quarter_turn_of_normals() {
    let (a, b, p) = (strip(0.0), graph(&[0.0, 1.0]), along(0.0));
    let s = sides(&a, &b, &p);
    let at = value_at(Measure::NormalAngle, s, 0.7);
    assert!(holds(at, PI / 4.0, 1e-15), "{at:?}");
    assert!(at.1 - at.0 < 1e-9, "{at:?}");
    assert!(holds(
        value_over(Measure::NormalAngle, s, 0.0, 1.0),
        PI / 4.0,
        1e-15
    ));
    // The two sides' normals tilt apart, so their curvature frames differ but both are flat.
    assert!(holds(value_at(Measure::Curvature, s, 0.7), 0.0, 0.0));
}

/// A cylinder of radius 4 about the line x = 0, z = 4, touching the strip along x = 0 and
/// curving up for x > 0.
fn roll() -> SurfaceKind {
    // The frame's first axis points from the axis to the line of contact (−z), its normal is the
    // cylinder's axis (y); the second axis is then −x, so u runs negative into x > 0.
    let frame = Frame::new([0.0, 0.0, -1.0], [0.0, 1.0, 0.0]).unwrap();
    SurfaceKind::Cylinder(
        Cylinder::new([0.0, 0.0, 4.0], 4.0, frame, ((-0.25, 0.0), (0.0, 1.0))).unwrap(),
    )
}

#[test]
fn a_cylinder_rolling_off_a_strip_jumps_by_its_curvature() {
    let (a, b, p) = (strip(0.0), roll(), along(0.0));
    let s = sides(&a, &b, &p);
    // Which way the cylinder's own normal points depends on its frame: take the face's normal
    // upward (+z) as the strip's, turning the cylinder if its normal points down.
    let up = value_at(Measure::NormalAngle, s, 0.5);
    let s = if up.0 > 1.0 {
        [
            s[0],
            Side {
                turned: true,
                ..s[1]
            },
        ]
    } else {
        s
    };
    expect(s, [0.0, 0.0, 0.25, 0.0]);
}

#[test]
fn turning_both_faces_changes_no_measure_and_one_turns_the_angle_by_a_half_turn() {
    let (a, b, p) = (strip(0.0), graph(&[0.0, 0.3, 1.0, -2.0]), along(0.0));
    let s = sides(&a, &b, &p);
    let both = [
        Side {
            turned: true,
            ..s[0]
        },
        Side {
            turned: true,
            ..s[1]
        },
    ];
    for m in ALL {
        let (x, y) = (value_at(m, s, 0.4), value_at(m, both, 0.4));
        assert!(
            (x.0 - y.0).abs() < 1e-12 && (x.1 - y.1).abs() < 1e-12,
            "{m:?}: {x:?} and {y:?}"
        );
    }
    let one = [
        Side {
            turned: true,
            ..s[0]
        },
        s[1],
    ];
    let straight = value_at(Measure::NormalAngle, s, 0.4);
    let turned = value_at(Measure::NormalAngle, one, 0.4);
    assert!(
        holds(turned, PI - f64::midpoint(straight.0, straight.1), 1e-12),
        "{straight:?} {turned:?}"
    );
}

fn unit(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

#[test]
fn enclosures_over_a_span_hold_the_values_inside_it() {
    let seed = resolve(
        "G346 class_a enclosures hold point values",
        Seed(0x0346_C1A5),
    )
    .unwrap();
    let mut rng = SplitMix64::new(seed);
    let rounds = if cfg!(debug_assertions) { 20 } else { 200 };
    for _ in 0..rounds {
        let degree = 1 + rng.below(NonZeroU64::new(4).unwrap()) as usize;
        let c: Vec<f64> = (0..=degree).map(|_| unit(&mut rng) * 4.0 - 2.0).collect();
        let lift = if rng.below(NonZeroU64::new(2).unwrap()) == 0 {
            0.0
        } else {
            unit(&mut rng) - 0.5
        };
        let (a, b) = (strip(lift), graph(&c));
        // A pcurve that wanders a little in u on the graph's side keeps every measure varying.
        let wander = CurveKind::Line(
            Line::new(
                [0.1 * unit(&mut rng), 0.0],
                [0.2 * unit(&mut rng), 1.0],
                (0.0, 1.0),
            )
            .unwrap(),
        );
        let p = along(0.0);
        let s = [
            Side {
                surface: &a,
                pcurve: &p,
                turned: false,
            },
            Side {
                surface: &b,
                pcurve: &wander,
                turned: false,
            },
        ];
        let (x, y) = (unit(&mut rng), unit(&mut rng));
        let (lo, hi) = (x.min(y), x.max(y));
        for m in ALL {
            let direct = class_a::over(m, s, (lo, hi), None, &mut meter())
                .unwrap()
                .value()
                .unwrap();
            let centred = value_over(m, s, lo, hi);
            for _ in 0..5 {
                let t = lo + (hi - lo) * unit(&mut rng);
                let v = value_at(m, s, t);
                let mid = 0.5 * v.0 + 0.5 * v.1;
                assert!(
                    direct.lo() <= mid && mid <= direct.hi(),
                    "{m:?} {c:?} [{lo}, {hi}] t {t}: {v:?} outside the direct {direct:?}"
                );
                assert!(
                    holds(centred, mid, 0.0),
                    "{m:?} {c:?} [{lo}, {hi}] t {t}: {v:?} outside the centred {centred:?}"
                );
            }
            // Over a span of 2⁻³⁰ the centred enclosure is tight.
            let narrow = value_over(m, s, lo, lo + 1.0 / 1_073_741_824.0);
            assert!(
                narrow.1 - narrow.0 < 1e-7,
                "{m:?} {c:?} at {lo}: {narrow:?}"
            );
        }
    }
}

#[test]
fn undefined_measures_and_bad_arguments_are_refused() {
    let (a, b) = (strip(0.0), graph(&[0.0, 0.0, 1.0]));
    // A pcurve that does not move has no tangent: G0 and G1 are defined, G2 and G3 are not.
    let still = CurveKind::Line(Line::new([0.0, 0.5], [0.0, 1e-300], (0.0, 1.0)).unwrap());
    let s = sides(&a, &b, &still);
    assert!(class_a::at(Measure::Gap, s, 0.5, Limit::FromRight, &mut meter()).is_ok());
    for m in [Measure::Curvature, Measure::CurvatureChange] {
        assert_eq!(
            class_a::at(m, s, 0.5, Limit::FromRight, &mut meter()),
            Err(CurveError::Unbounded),
            "{m:?}"
        );
    }
    // A pcurve wholly outside its surface's domain.
    let far = along(3.0);
    let s = [
        Side {
            surface: &a,
            pcurve: &far,
            turned: false,
        },
        Side {
            surface: &b,
            pcurve: &far,
            turned: false,
        },
    ];
    assert_eq!(
        class_a::at(Measure::Gap, s, 0.5, Limit::FromRight, &mut meter()),
        Err(CurveError::SpanOutside)
    );
    // A centre outside its span, or of another measure.
    let p = along(0.0);
    let s = sides(&a, &b, &p);
    let at = class_a::at(Measure::Gap, s, 0.9, Limit::FromRight, &mut meter()).unwrap();
    assert_eq!(
        class_a::over(Measure::Gap, s, (0.0, 0.5), Some((0.9, &at)), &mut meter()),
        Err(CurveError::ParameterOutside)
    );
    assert_eq!(
        class_a::over(
            Measure::Curvature,
            s,
            (0.5, 1.0),
            Some((0.9, &at)),
            &mut meter()
        ),
        Err(CurveError::Defect)
    );
    assert_eq!(
        class_a::over(Measure::Gap, s, (0.5, 0.25), None, &mut meter()),
        Err(CurveError::SpanOutside)
    );
}
