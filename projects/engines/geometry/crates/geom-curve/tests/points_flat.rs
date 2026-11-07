//! Many points per call (piece G104, chapter 23.6): every curve kind, through `CurveKind`,
//! evaluates N parameters into flat arrays bit for bit as N single calls, a failing parameter fails
//! only itself, and wrong array lengths are refused before anything is written.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    reason = "tests may use native arithmetic and casts"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;

use geom_curve::transition::CurvatureLaw;
use geom_curve::{
    Axes, Basis, BulgePolyline, Circle, Composite, CurveError, CurveKind, Ellipse, Frame, Helix,
    HelixAxes, Hyperbola, Involute, Line, Offset, OffsetLaw, OnSurface, Parabola, ParamCurve,
    Polynomial, SurfaceRef, Transition, Trimmed,
};
use geom_nurbs::{Basis as Knots, Curve, Limit};
use geom_run::{Budget, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn unit(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

fn range(rng: &mut SplitMix64, lo: f64, hi: f64) -> f64 {
    lo + (hi - lo) * unit(rng)
}

fn vector<const D: usize>(rng: &mut SplitMix64, s: f64) -> [f64; D] {
    core::array::from_fn(|_| range(rng, -s, s))
}

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

fn frame<const D: usize>(rng: &mut SplitMix64) -> Frame<D> {
    for _ in 0..1000 {
        let x = vector::<D>(rng, 1.0);
        let n = if D == 2 {
            [0.0, 0.0, 1.0]
        } else {
            vector::<3>(rng, 1.0)
        };
        if let Ok(f) = Frame::new(x, n) {
            return f;
        }
    }
    panic!("no frame in 1000 draws")
}

fn axes<const D: usize>(rng: &mut SplitMix64) -> Axes<D> {
    Axes::Frame {
        frame: frame(rng),
        a: range(rng, 0.5, 20.0),
        b: range(rng, 0.5, 20.0),
    }
}

fn nurbs<const D: usize>(rng: &mut SplitMix64) -> Curve<D> {
    let knots = Knots::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0]).unwrap();
    let pts: Vec<[f64; D]> = (0..knots.count()).map(|_| vector(rng, 9.0)).collect();
    Curve::new(knots, &pts).unwrap()
}

/// One curve of every kind of dimension D that the kind allows there.
fn kinds<const D: usize>(rng: &mut SplitMix64) -> Vec<(&'static str, CurveKind<D>)> {
    let c = vector::<D>(rng, 50.0);
    let circle = Circle::new(c, range(rng, 0.5, 30.0), frame(rng), (0.0, 2.0)).unwrap();
    let mut out = vec![
        ("nurbs", CurveKind::Nurbs(nurbs(rng))),
        (
            "line",
            CurveKind::Line(Line::new(c, vector(rng, 10.0), (-1.0, 2.0)).unwrap()),
        ),
        ("circle", CurveKind::Circle(circle.clone())),
        (
            "ellipse",
            CurveKind::Ellipse(Ellipse::new(c, axes(rng), (0.0, 2.0)).unwrap()),
        ),
        (
            "hyperbola",
            CurveKind::Hyperbola(Hyperbola::new(c, axes(rng), (-2.0, 1.5)).unwrap()),
        ),
        (
            "parabola",
            CurveKind::Parabola(Parabola::new(c, axes(rng), (-3.0, 3.0)).unwrap()),
        ),
        (
            "polynomial",
            CurveKind::Polynomial(
                Polynomial::new((0..4).map(|_| vector(rng, 5.0)).collect(), (-1.0, 1.5)).unwrap(),
            ),
        ),
        (
            "involute",
            CurveKind::Involute(Involute::with_axes(c, axes(rng), (-1.0, 2.0)).unwrap()),
        ),
        (
            "trimmed",
            CurveKind::Trimmed(Trimmed::new(Basis::Circle(circle.clone()), (0.25, 1.25)).unwrap()),
        ),
        (
            "composite",
            CurveKind::Composite(
                Composite::new(vec![
                    Trimmed::new(Basis::Circle(circle.clone()), (0.0, 0.5)).unwrap(),
                    Trimmed::new(Basis::Circle(circle.clone()), (0.5, 1.5)).unwrap(),
                ])
                .unwrap(),
            ),
        ),
    ];
    if D == 3 {
        let ax = HelixAxes::Frame {
            frame: frame(rng),
            radius: range(rng, 0.5, 10.0),
            pitch: range(rng, -8.0, 8.0),
        };
        out.push((
            "helix",
            CurveKind::Helix(Helix::new(c, ax, (-2.0, 4.0)).unwrap()),
        ));
    }
    out
}

/// The kinds only a plane allows: the bulge polyline, the offset and the transition.
fn planar_kinds() -> Vec<(&'static str, CurveKind<2>)> {
    let m = &mut meter();
    let circle = Circle::new([1.0, 2.0], 5.0, Frame::standard().unwrap(), (0.0, 1.5)).unwrap();
    let law = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 0.01,
    };
    vec![
        (
            "bulge",
            CurveKind::Bulge(
                BulgePolyline::new(
                    vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [1.0, 5.0]],
                    vec![0.0, 0.5, -0.25],
                    None,
                    false,
                )
                .unwrap(),
            ),
        ),
        (
            "offset",
            CurveKind::Offset(
                Offset::new(
                    Trimmed::whole(Basis::Circle(circle)).unwrap(),
                    OffsetLaw::Planar { distance: 1.5 },
                )
                .unwrap(),
            ),
        ),
        (
            "transition",
            CurveKind::Transition(Transition::new([0.0, 0.0], [1.0, 0.0], 10.0, law, m).unwrap()),
        ),
    ]
}

/// Parameters across and past the domain, ends and NaN included.
fn parameters(rng: &mut SplitMix64, domain: (f64, f64), n: usize) -> Vec<f64> {
    let (a, b) = domain;
    let (a, b) = (
        if a < -1e6 { -1e6 } else { a },
        if b > 1e6 { 1e6 } else { b },
    );
    (0..n)
        .map(|i| match below(rng, 12) {
            0 => a,
            1 => b,
            2 => a - 1.0 - unit(rng),
            3 => b + 1.0 + unit(rng),
            4 if i % 2 == 0 => f64::NAN,
            _ => range(rng, a, b),
        })
        .collect()
}

fn check_kind<const D: usize>(name: &str, curve: &CurveKind<D>, rng: &mut SplitMix64) {
    for n in [0usize, 1, 2, 7, 64, 129] {
        let t = parameters(rng, curve.domain(), n);
        for side in [Limit::FromRight, Limit::FromLeft] {
            let mut out = vec![-7.0; D * n];
            let mut results = vec![Ok(()); n];
            curve.points(&t, side, &mut out, &mut results).unwrap();
            for (i, &ti) in t.iter().enumerate() {
                let single = curve.point(ti, side);
                match single {
                    Ok(p) => {
                        assert_eq!(results[i], Ok(()), "{name} at {ti}");
                        for k in 0..D {
                            assert_eq!(
                                out[k * n + i].to_bits(),
                                p[k].to_bits(),
                                "{name} coordinate {k} at {ti}"
                            );
                        }
                    }
                    Err(e) => {
                        assert_eq!(results[i], Err(e), "{name} at {ti}");
                        for k in 0..D {
                            assert_eq!(out[k * n + i].to_bits(), 0.0f64.to_bits());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_curve_kind_evaluates_many_parameters_as_single_calls() {
    let seed = resolve("geom-curve many points", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut kinds_seen = std::collections::BTreeSet::new();
    for _ in 0..4 {
        for (name, c) in kinds::<2>(&mut rng) {
            check_kind(name, &c, &mut rng);
            kinds_seen.insert(name);
        }
        for (name, c) in kinds::<3>(&mut rng) {
            check_kind(name, &c, &mut rng);
            kinds_seen.insert(name);
        }
        for (name, c) in planar_kinds() {
            check_kind(name, &c, &mut rng);
            kinds_seen.insert(name);
        }
        check_kind("on surface", &on_surface(), &mut rng);
        kinds_seen.insert("on surface");
    }
    // Every variant of CurveKind is exercised.
    let every = [
        "nurbs",
        "line",
        "circle",
        "ellipse",
        "hyperbola",
        "parabola",
        "helix",
        "polynomial",
        "involute",
        "bulge",
        "trimmed",
        "composite",
        "offset",
        "on surface",
        "transition",
    ];
    assert_eq!(kinds_seen.len(), every.len());
    assert!(every.iter().all(|k| kinds_seen.contains(k)));
}

#[test]
fn wrong_lengths_are_refused_before_anything_is_written() {
    let line = CurveKind::Line(Line::new([0.0; 3], [1.0, 2.0, 3.0], (0.0, 1.0)).unwrap());
    let t = [0.0, 0.5, 1.0];
    let mut out = vec![9.0; 9];
    let mut results = vec![Err(CurveError::Defect); 3];
    let short = &mut [Ok(()), Ok(())];
    assert_eq!(
        line.points(&t, Limit::FromRight, &mut out, short),
        Err(CurveError::ArrayLength {
            expected: 3,
            given: 2
        })
    );
    assert_eq!(
        line.points(&t, Limit::FromRight, &mut out[..8], &mut results),
        Err(CurveError::ArrayLength {
            expected: 9,
            given: 8
        })
    );
    let mut long = vec![9.0; 10];
    assert_eq!(
        line.points(&t, Limit::FromRight, &mut long, &mut results),
        Err(CurveError::ArrayLength {
            expected: 9,
            given: 10
        })
    );
    assert!(out.iter().all(|&v| v == 9.0));
    assert!(results.iter().all(|r| *r == Err(CurveError::Defect)));
    line.points(&t, Limit::FromRight, &mut out, &mut results)
        .unwrap();
    assert_eq!(out, [0.0, 0.5, 1.0, 0.0, 1.0, 2.0, 0.0, 1.5, 3.0]);
    assert_eq!(
        CurveError::ArrayLength {
            expected: 9,
            given: 8
        }
        .to_string(),
        "a flat array holds 8 values where the call needs 9"
    );
}

/// A curve on a surface: a 2D parameter curve with its 3D cache.
fn on_surface() -> CurveKind<3> {
    let knots = Knots::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let cache = Curve::new(knots, &[[0.0, 0.0, 0.0], [1.0, 2.0, 0.0]]).unwrap();
    let uv = Trimmed::whole(Basis::Line(
        Line::new([0.0, 0.0], [1.0, 2.0], (0.0, 1.0)).unwrap(),
    ))
    .unwrap();
    CurveKind::OnSurface(OnSurface::new(uv, SurfaceRef(7), cache, 1e-9).unwrap())
}
