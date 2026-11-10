//! Many points per call (piece G104, chapter 23.6): every surface kind, through `SurfaceKind`,
//! evaluates N parameter pairs from and into flat arrays bit for bit as N single calls, a failing
//! pair fails only itself, and wrong array lengths are refused before anything is written.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    reason = "tests may use native arithmetic and casts"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;

use geom_curve::{Basis as Leaf, Circle, CurveError, Frame, Line, Trimmed};
use geom_math::{Transform, TypedTransform};
use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, Extrusion, Offset, ParamSurface, Plane, Revolution, Ruled, Sphere, SurfaceKind,
    Torus,
};
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

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

fn patch(rng: &mut SplitMix64, rational: bool) -> Surface<3> {
    let bu = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let bv = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut pts = Vec::new();
    let mut ws = Vec::new();
    for i in 0..5 {
        for j in 0..5 {
            pts.push([f64::from(i) * 0.75, f64::from(j), range(rng, -0.3, 0.3)]);
            ws.push(range(rng, 0.95, 1.05));
        }
    }
    if rational {
        Surface::new_rational(bu, bv, &pts, &ws).unwrap()
    } else {
        Surface::new(bu, bv, &pts).unwrap()
    }
}

/// One surface of every kind.
fn kinds(rng: &mut SplitMix64) -> Vec<(&'static str, SurfaceKind)> {
    let m = &mut meter();
    let f = Frame::standard().unwrap();
    let axis = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let arc = Trimmed::whole(Leaf::Circle(
        Circle::new([0.0; 3], 2.0, Frame::standard().unwrap(), (0.0, 1.0)).unwrap(),
    ))
    .unwrap();
    let line = Trimmed::whole(Leaf::Line(
        Line::new([2.0, 0.0, 1.0], [1.0, 0.5, 2.0], (0.0, 1.0)).unwrap(),
    ))
    .unwrap();
    let c = [
        range(rng, -9.0, 9.0),
        range(rng, -9.0, 9.0),
        range(rng, -9.0, 9.0),
    ];
    let cylinder = Cylinder::new(c, range(rng, 1.0, 5.0), f, ((0.0, 1.5), (0.0, 3.0))).unwrap();
    let squash = TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap());
    let rational = rng.coin();
    let affine = SurfaceKind::Cylinder(cylinder.clone())
        .transformed(&squash, m)
        .unwrap()
        .value;
    vec![
        ("nurbs", SurfaceKind::Nurbs(patch(rng, rational))),
        (
            "plane",
            SurfaceKind::Plane(
                Plane::new(
                    c,
                    [1.0, 0.5, 0.0],
                    [0.0, 1.0, 2.0],
                    ((0.0, 2.0), (-1.0, 1.0)),
                )
                .unwrap(),
            ),
        ),
        ("cylinder", SurfaceKind::Cylinder(cylinder.clone())),
        (
            "cone",
            SurfaceKind::Cone(
                Cone::new(c, range(rng, 1.0, 5.0), 0.25, f, ((-0.5, 0.5), (0.0, 2.0))).unwrap(),
            ),
        ),
        (
            "sphere",
            SurfaceKind::Sphere(
                Sphere::new(c, range(rng, 1.0, 5.0), f, ((0.0, 2.0), (-0.5, 0.25))).unwrap(),
            ),
        ),
        (
            "torus",
            SurfaceKind::Torus(
                Torus::new(c, 5.0, range(rng, 1.0, 3.0), f, ((0.0, 1.0), (-1.0, 1.0))).unwrap(),
            ),
        ),
        ("affine", affine),
        (
            "revolution",
            SurfaceKind::Revolution(
                Revolution::new(line.clone(), [0.0; 3], axis, (0.0, 1.5)).unwrap(),
            ),
        ),
        (
            "extrusion",
            SurfaceKind::Extrusion(
                Extrusion::new(arc.clone(), [0.5, 1.0, 2.0], (0.0, 2.0)).unwrap(),
            ),
        ),
        (
            "ruled",
            SurfaceKind::Ruled(Ruled::new(line, arc, (0.0, 1.0)).unwrap()),
        ),
        (
            "offset",
            SurfaceKind::Offset(
                Offset::new(SurfaceKind::Cylinder(cylinder), range(rng, -0.8, 0.8)).unwrap(),
            ),
        ),
    ]
}

/// One coordinate across and past its range, ends and NaN included.
fn coordinate(rng: &mut SplitMix64, (a, b): (f64, f64)) -> f64 {
    match below(rng, 14) {
        0 => a,
        1 => b,
        2 => a - 1.0 - unit(rng),
        3 => b + 1.0 + unit(rng),
        4 => f64::NAN,
        _ => range(rng, a, b),
    }
}

fn check_kind(name: &str, s: &SurfaceKind, rng: &mut SplitMix64) {
    let (du, dv) = s.domain();
    for n in [0usize, 1, 2, 7, 64, 129] {
        let mut uv = vec![0.0; 2 * n];
        for i in 0..n {
            uv[i] = coordinate(rng, du);
            uv[n + i] = coordinate(rng, dv);
        }
        for sides in [
            (Limit::FromRight, Limit::FromRight),
            (Limit::FromLeft, Limit::FromRight),
            (Limit::FromRight, Limit::FromLeft),
        ] {
            let mut out = vec![-7.0; 3 * n];
            let mut results = vec![Ok(()); n];
            s.points(&uv, sides, &mut out, &mut results).unwrap();
            for i in 0..n {
                let (u, v) = (uv[i], uv[n + i]);
                match s.point(u, v, sides) {
                    Ok(p) => {
                        assert_eq!(results[i], Ok(()), "{name} at ({u}, {v})");
                        for k in 0..3 {
                            assert_eq!(
                                out[k * n + i].to_bits(),
                                p[k].to_bits(),
                                "{name} coordinate {k} at ({u}, {v})"
                            );
                        }
                    }
                    Err(e) => {
                        assert_eq!(results[i], Err(e), "{name} at ({u}, {v})");
                        for k in 0..3 {
                            assert_eq!(out[k * n + i].to_bits(), 0.0f64.to_bits());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_surface_kind_evaluates_many_pairs_as_single_calls() {
    let seed = resolve("geom-surface many points", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut kinds_seen = std::collections::BTreeSet::new();
    for _ in 0..4 {
        for (name, s) in kinds(&mut rng) {
            check_kind(name, &s, &mut rng);
            kinds_seen.insert(name);
        }
    }
    let every = [
        "nurbs",
        "plane",
        "cylinder",
        "cone",
        "sphere",
        "torus",
        "affine",
        "revolution",
        "extrusion",
        "ruled",
        "offset",
    ];
    assert_eq!(kinds_seen.len(), every.len());
    assert!(every.iter().all(|k| kinds_seen.contains(k)));
}

#[test]
fn wrong_lengths_are_refused_before_anything_is_written() {
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    let sides = (Limit::FromRight, Limit::FromRight);
    let uv = [0.0, 0.5, 1.0, 0.25, 0.75, 1.0];
    let mut out = vec![9.0; 9];
    let mut results = vec![Err(CurveError::Defect); 3];
    assert_eq!(
        plane.points(&uv[..5], sides, &mut out, &mut results),
        Err(CurveError::ArrayLength {
            expected: 6,
            given: 5
        })
    );
    assert_eq!(
        plane.points(&uv, sides, &mut out[..8], &mut results),
        Err(CurveError::ArrayLength {
            expected: 9,
            given: 8
        })
    );
    assert_eq!(
        plane.points(&uv, sides, &mut out, &mut results[..2]),
        Err(CurveError::ArrayLength {
            expected: 4,
            given: 6
        })
    );
    assert!(out.iter().all(|&v| v == 9.0));
    assert!(results.iter().all(|r| *r == Err(CurveError::Defect)));
    plane.points(&uv, sides, &mut out, &mut results).unwrap();
    assert_eq!(out, [0.0, 0.5, 1.0, 0.25, 0.75, 1.0, 0.0, 0.0, 0.0]);
    assert!(results.iter().all(Result::is_ok));
}
