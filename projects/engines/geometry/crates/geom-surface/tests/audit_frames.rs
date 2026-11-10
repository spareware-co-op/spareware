//! Audit of the frame rules (piece G254) against what each rule is by definition, on a helix, an
//! arc and a twisted cubic: every placement is a right-handed orthonormal frame at the path's
//! point with its z axis along the tangent; a rotation-minimising frame does not spin about the
//! tangent; a Frenet frame's x axis is the principal normal; a fixed reference steers the axis it
//! names. The references are the path's own derivatives and finite differences of the frames.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests: native arithmetic and indexing, the formulas' names, and a failure stops the test"
)]

use geom_curve::{Circle, CurveKind, Frame, Helix, HelixAxes, ParamCurve};
use geom_nurbs::{Basis, Curve, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::frame_rule::placement;
use geom_surface::{Closing, FrameRule, Laws, ProfileAxis, Stations};

type V = [f64; 3];

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V, b: V) -> V {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(k: f64, a: V) -> V {
    a.map(|x| k * x)
}

fn norm(a: V) -> f64 {
    dot(a, a).sqrt()
}

fn unit(a: V) -> V {
    scale(1.0 / norm(a), a)
}

fn paths() -> Vec<(&'static str, CurveKind<3>)> {
    let tilted = Frame::new([1.0, 0.5, 0.0], [0.25, -0.5, 1.0]).unwrap();
    let helix = Helix::new(
        [1.0, -2.0, 0.5],
        HelixAxes::Frame { frame: tilted, radius: 2.0, pitch: 1.5 },
        (0.0, 3.0),
    )
    .unwrap();
    let arc = Circle::new([0.0, 1.0, 2.0], 3.0, tilted, (0.25, 1.75)).unwrap();
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0]).unwrap();
    let cubic = Curve::new(
        basis,
        &[[0.0, 0.0, 0.0], [2.0, 1.0, 0.0], [3.0, 3.0, 2.0], [1.0, 5.0, 4.0], [-1.0, 4.0, 6.0]],
    )
    .unwrap();
    vec![
        ("helix", CurveKind::Helix(helix)),
        ("arc", CurveKind::Circle(arc)),
        ("cubic", CurveKind::Nurbs(cubic)),
    ]
}

fn derivatives(path: &CurveKind<3>, t: f64) -> [V; 3] {
    let mut d = [[0.0; 3]; 3];
    path.derivatives(t, Limit::FromRight, &mut d).unwrap();
    d
}

fn samples(path: &CurveKind<3>, n: usize) -> Vec<f64> {
    let (a, b) = path.domain();
    (0..=n).map(|i| (a + (b - a) * i as f64 / n as f64).clamp(a, b)).collect()
}

/// The frame of every rule is orthonormal and right-handed, sits at the path's point and has its
/// z axis along the tangent (but for `Translate`, whose frame is the world's).
#[test]
fn every_placement_is_a_right_handed_frame_on_the_path() {
    let mut failures = Vec::new();
    for (name, path) in paths() {
        let start = cross(unit(derivatives(&path, path.domain().0)[1]), [0.3, -0.2, 0.9]);
        let stations = Stations::build(&path, start, 1.0, 1e-9, Closing::Open, &mut meter()).unwrap();
        let rules = [
            ("translate", FrameRule::Translate),
            ("rotation-minimising", FrameRule::RotationMinimising(stations)),
            ("frenet", FrameRule::Frenet),
            ("fixed x", FrameRule::FixedReference { reference: [0.2, 0.1, 1.0], axis: ProfileAxis::X }),
            ("fixed y", FrameRule::FixedReference { reference: [0.2, 0.1, 1.0], axis: ProfileAxis::Y }),
        ];
        for (rule_name, rule) in &rules {
            for t in samples(&path, 60) {
                let p = match placement(rule, &Laws::default(), &path, t) {
                    Ok(p) => p,
                    Err(e) => {
                        failures.push(format!("{name}, {rule_name} at {t}: {e:?}"));
                        break;
                    }
                };
                let d = derivatives(&path, t);
                let tangent = unit(d[1]);
                let ortho = (norm(p.x) - 1.0).abs()
                    + (norm(p.y) - 1.0).abs()
                    + (norm(p.z) - 1.0).abs()
                    + dot(p.x, p.y).abs()
                    + dot(p.y, p.z).abs()
                    + dot(p.z, p.x).abs()
                    + norm(sub(cross(p.x, p.y), p.z));
                if ortho > 1e-9 {
                    failures.push(format!("{name}, {rule_name} at {t}: not a right-handed frame ({ortho:e})"));
                    break;
                }
                if norm(sub(p.origin, d[0])) > 1e-9 * (1.0 + norm(d[0])) {
                    failures.push(format!("{name}, {rule_name} at {t}: origin {:?} off the path {:?}", p.origin, d[0]));
                    break;
                }
                if *rule_name != "translate" && norm(sub(p.z, tangent)) > 1e-9 {
                    failures.push(format!("{name}, {rule_name} at {t}: z {:?} is not the tangent {tangent:?}", p.z));
                    break;
                }
                if p.scale != 1.0 {
                    failures.push(format!("{name}, {rule_name} at {t}: scale {}", p.scale));
                    break;
                }
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// A rotation-minimising frame's x axis turns only with the tangent: x′ has no part along y. The
/// total spin about the tangent over the path, ∫ x′·y, is the twist error, which the stations
/// were built to keep within the tolerance over the profile's radius.
#[test]
fn a_rotation_minimising_frame_does_not_spin_about_the_tangent() {
    for (name, path) in paths() {
        let start = cross(unit(derivatives(&path, path.domain().0)[1]), [0.3, -0.2, 0.9]);
        let tolerance = 1e-7;
        let stations = Stations::build(&path, start, 1.0, tolerance, Closing::Open, &mut meter()).unwrap();
        let rule = FrameRule::RotationMinimising(stations);
        let (a, b) = path.domain();
        let n = 4000;
        let h = (b - a) / n as f64;
        let frame = |t: f64| placement(&rule, &Laws::default(), &path, t.clamp(a, b)).unwrap();
        // The spin by the midpoint rule on central differences of x.
        let mut spin = 0.0;
        let mut worst: f64 = 0.0;
        for i in 0..n {
            let (t0, t1) = (a + h * i as f64, a + h * (i + 1) as f64);
            let (f0, f1) = (frame(t0), frame(t1));
            let mid = frame(0.5 * (t0 + t1));
            let rate = dot(sub(f1.x, f0.x), mid.y) / h;
            worst = worst.max(rate.abs());
            spin += rate * h;
        }
        // Radians of twist over the whole path, against the tolerance over a radius of 1, with
        // room for the differences' own error.
        assert!(
            spin.abs() <= 10.0 * tolerance + 1e-6,
            "{name}: the frame spins {spin:e} radians about the tangent over the path (largest rate {worst:e})"
        );
    }
}

/// A Frenet frame's x axis is the principal normal: across the tangent, in the plane of C′ and
/// C″, on C″'s side.
#[test]
fn a_frenet_frame_has_the_principal_normal_for_its_x_axis() {
    for (name, path) in paths() {
        for t in samples(&path, 40) {
            let Ok(p) = placement(&FrameRule::Frenet, &Laws::default(), &path, t) else { continue };
            let d = derivatives(&path, t);
            let tangent = unit(d[1]);
            let across = sub(d[2], scale(dot(d[2], tangent), tangent));
            if norm(across) < 1e-9 * norm(d[2]) {
                continue;
            }
            let normal = unit(across);
            assert!(
                norm(sub(p.x, normal)) < 1e-9,
                "{name} at {t}: x {:?} is not the principal normal {normal:?}",
                p.x
            );
            let binormal = cross(tangent, normal);
            assert!(norm(sub(p.y, binormal)) < 1e-9, "{name} at {t}: y is not the binormal");
        }
    }
}

/// A fixed reference steers the profile's x axis (IFC's fixed reference sweep) or its y axis (an
/// up-vector) towards the reference's part across the tangent.
#[test]
fn a_fixed_reference_steers_the_axis_it_names() {
    let reference = [0.2, 0.1, 1.0];
    for (name, path) in paths() {
        for (axis, which) in [(ProfileAxis::X, "x"), (ProfileAxis::Y, "y")] {
            let rule = FrameRule::FixedReference { reference, axis };
            for t in samples(&path, 40) {
                let Ok(p) = placement(&rule, &Laws::default(), &path, t) else { continue };
                let tangent = unit(derivatives(&path, t)[1]);
                let across = unit(sub(reference, scale(dot(reference, tangent), tangent)));
                let steered = if axis == ProfileAxis::X { p.x } else { p.y };
                assert!(
                    norm(sub(steered, across)) < 1e-9,
                    "{name} at {t}: the {which} axis {steered:?} is not the reference across the tangent {across:?}"
                );
            }
        }
    }
}
