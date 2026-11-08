//! The frame rules and laws (piece G254; chapter 13.3), as laws over seeded paths: every rule's
//! frame is orthonormal and right-handed and travels along its path; rotation-minimising stations
//! meet the closed forms on lines, arcs and helices, meet their stations bit for bit, are C1
//! between them, rebuild bit for bit and spread or refuse a closing twist; Frenet and
//! fixed-reference frames are certified where they exist and refused, with the span, where they
//! may not; placements are met and the turn between them shared evenly; a helical rule is a screw
//! motion; twist and scale laws act on top of any rule; and laws are exact where given exactly.
//!
//! Seeds: each seeded law prints `seed: <hex>` and replays with `SPAREKIT_SEED`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    clippy::unreachable,
    reason = "tests: native arithmetic and indexing, the formulas' names, and a failure stops the test"
)]

use core::f64::consts::PI;

use dashu_ratio::RBig;
use geom_curve::{
    Circle, CurveError, CurveKind, Frame, Helix, HelixAxes, Line, ParamCurve, Polynomial, Trimmed,
};
use geom_num::NumError;
use geom_nurbs::{Basis, Curve, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::frame_rule::{
    Closing, FrameError, FrameRule, Law, Laws, MAX_SPANS, ProfileAxis, StationFrame,
    StationPlacement, Stations, arc_length, closed_form_frame, frenet_defined, placement,
    reference_defined,
};
use geom_test::{Seed, SplitMix64};

type V = [f64; 3];

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn norm(a: V) -> f64 {
    dot(a, a).sqrt()
}

fn unit(a: V) -> V {
    let n = norm(a);
    [a[0] / n, a[1] / n, a[2] / n]
}

/// The angle between two unit vectors, in radians.
fn angle(a: V, b: V) -> f64 {
    norm(cross(a, b)).atan2(dot(a, b))
}

struct Rng(SplitMix64);

impl Rng {
    fn new(label: &str, default: u64) -> Rng {
        Rng(SplitMix64::new(
            geom_test::seed::resolve(label, Seed(default)).unwrap(),
        ))
    }

    /// A double in [lo, hi), from 53 bits.
    fn real(&mut self, lo: f64, hi: f64) -> f64 {
        let u = (self.0.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        lo + (hi - lo) * u
    }

    fn vector(&mut self, r: f64) -> V {
        [self.real(-r, r), self.real(-r, r), self.real(-r, r)]
    }

    fn direction(&mut self) -> V {
        loop {
            let v = self.vector(1.0);
            if norm(v) > 0.2 {
                return unit(v);
            }
        }
    }
}

fn circle(rng: &mut Rng) -> CurveKind<3> {
    let n = rng.direction();
    let x = rng.direction();
    let frame = Frame::new(x, n).unwrap();
    let a = rng.real(-1.0, 0.5);
    CurveKind::Circle(
        Circle::new(
            rng.vector(5.0),
            rng.real(0.5, 4.0),
            frame,
            (a, a + rng.real(0.3, 1.5)),
        )
        .unwrap(),
    )
}

fn helix(rng: &mut Rng) -> CurveKind<3> {
    let frame = Frame::new(rng.direction(), rng.direction()).unwrap();
    let pitch = if rng.real(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 } * rng.real(0.5, 6.0);
    let axes = HelixAxes::Frame {
        frame,
        radius: rng.real(0.5, 3.0),
        pitch,
    };
    let a = rng.real(-2.0, 1.0);
    CurveKind::Helix(Helix::new(rng.vector(5.0), axes, (a, a + rng.real(1.0, 5.0))).unwrap())
}

fn line(rng: &mut Rng) -> CurveKind<3> {
    let a = rng.real(-1.0, 0.0);
    CurveKind::Line(
        Line::new(
            rng.vector(5.0),
            rng.vector(3.0),
            (a, a + rng.real(0.5, 3.0)),
        )
        .unwrap(),
    )
}

/// A twisted cubic space curve: curvature and torsion both nonzero on its domain.
fn cubic(rng: &mut Rng) -> CurveKind<3> {
    let k = rng.real(0.5, 2.0);
    CurveKind::Polynomial(
        Polynomial::new(
            vec![
                rng.vector(3.0),
                [1.0, 0.0, 0.0],
                [0.0, k, 0.0],
                [0.0, 0.0, k * 0.5],
            ],
            (0.1, rng.real(1.0, 2.0)),
        )
        .unwrap(),
    )
}

fn start_normal(path: &CurveKind<3>, rng: &mut Rng) -> V {
    let (a, _) = path.domain();
    let mut d = [[0.0; 3]; 2];
    path.derivatives(a, Limit::FromRight, &mut d).unwrap();
    loop {
        let v = rng.direction();
        if norm(cross(v, unit(d[1]))) > 0.3 {
            return v;
        }
    }
}

fn tangent(path: &CurveKind<3>, t: f64) -> V {
    let side = if t >= path.domain().1 {
        Limit::FromLeft
    } else {
        Limit::FromRight
    };
    let mut d = [[0.0; 3]; 2];
    path.derivatives(t, side, &mut d).unwrap();
    unit(d[1])
}

fn samples(path: &CurveKind<3>, n: usize) -> Vec<f64> {
    let (a, b) = path.domain();
    (0..=n)
        .map(|k| {
            if k == n {
                b
            } else {
                a + (b - a) * k as f64 / n as f64
            }
        })
        .collect()
}

fn stations(path: &CurveKind<3>, rng: &mut Rng) -> Stations {
    Stations::build(
        path,
        start_normal(path, rng),
        1.0,
        1e-9,
        Closing::Open,
        &mut meter(),
    )
    .unwrap()
}

#[test]
fn every_rules_frame_is_orthonormal_right_handed_and_travels_along_its_path() {
    let mut rng = Rng::new("frames: orthonormal", 0x2540_0001);
    for round in 0..24 {
        let path = match round % 4 {
            0 => circle(&mut rng),
            1 => helix(&mut rng),
            2 => line(&mut rng),
            _ => cubic(&mut rng),
        };
        let (a, b) = path.domain();
        let mut rules = vec![
            (FrameRule::Translate, false),
            (
                FrameRule::RotationMinimising(stations(&path, &mut rng)),
                true,
            ),
            (
                FrameRule::Placements(vec![
                    StationPlacement {
                        at: a,
                        x: rng.direction(),
                        z: rng.direction(),
                    },
                    StationPlacement {
                        at: f64::midpoint(a, b),
                        x: rng.direction(),
                        z: rng.direction(),
                    },
                ]),
                false,
            ),
            (
                FrameRule::Helical {
                    point: rng.vector(3.0),
                    axis: rng.direction(),
                    reference: rng.direction(),
                    pitch: Law::Linear {
                        start: 1.0,
                        rate: 0.25,
                    },
                    radius: Law::Constant(2.0),
                },
                false,
            ),
        ];
        if round % 4 != 2 {
            rules.push((FrameRule::Frenet, true));
        }
        if round % 4 == 1 {
            // A helix's tangent never runs along its axis.
            let CurveKind::Helix(h) = &path else {
                unreachable!()
            };
            let HelixAxes::Frame { frame, .. } = h.axes() else {
                unreachable!()
            };
            for axis in [ProfileAxis::X, ProfileAxis::Y] {
                rules.push((
                    FrameRule::FixedReference {
                        reference: frame.normal(),
                        axis,
                    },
                    true,
                ));
            }
        }
        for (rule, follows) in &rules {
            for t in samples(&path, 17) {
                let p = placement(rule, &Laws::default(), &path, t)
                    .unwrap_or_else(|e| panic!("{rule:?} at {t}: {e}"));
                for v in [p.x, p.y, p.z] {
                    assert!((norm(v) - 1.0).abs() < 1e-12, "{rule:?}: |{v:?}|");
                }
                assert!(dot(p.x, p.y).abs() < 1e-12 && dot(p.y, p.z).abs() < 1e-12);
                assert!(dot(p.z, p.x).abs() < 1e-12, "{rule:?}");
                let handed = sub(cross(p.x, p.y), p.z);
                assert!(norm(handed) < 1e-12, "{rule:?} is not right-handed at {t}");
                if *follows {
                    let along = sub(p.z, tangent(&path, t));
                    assert!(norm(along) < 1e-12, "{rule:?} leaves the tangent at {t}");
                    let on = sub(
                        p.origin,
                        path.point(
                            t,
                            if t >= b {
                                Limit::FromLeft
                            } else {
                                Limit::FromRight
                            },
                        )
                        .unwrap(),
                    );
                    assert!(norm(on) == 0.0, "{rule:?} leaves the path at {t}");
                }
            }
        }
    }
}

/// How far the frame of stations `s` may be from the exact rotation-minimising frame on a line, an
/// arc or a circular helix, in radians: derived, not fitted.
///
/// - **One step's error.** Wang et al.'s step is time-symmetric (the step back from x to xᵢ undoes
///   it), so on a helix its twist error is odd in the step's length Δs: e = E·(1 + ρ + O(Θ⁴)) with
///   E = −κ²τ³Δs⁵/720 and ρ = (κ²/42 − τ²/28)·Δs², for a step that turns Θ = Δs·√(κ² + τ²)
///   radians about the axis, so |ρ| ≤ Θ²/28. That is the series of the step on the helix, checked
///   against the step in 80-digit arithmetic, where the O(Θ⁴) term is 6·10⁻⁴·Θ⁴. On a line or an
///   arc τ = 0, and the step is exact.
/// - **The estimate.** Every point of a helix is alike, so the rule's estimate of a step,
///   (16/15)·(e(Δs) − 2·e(Δs/2)), is E·(1 + 21ρ/20 + O(Θ⁴)), so |E| is at most m/(1 − 21|ρ|/20)
///   for the largest estimate m.
/// - **Adding up.** Each step is an orthogonal map from one station's plane across the tangent to
///   the next one's, so the stations' errors add. Between stations the C1 correction makes the
///   error E·((u⁵ − 5u³ + 5u²) + ρ·(u⁷ − 7u³ + 7u²)) a fraction u of the way along, at most
///   E·(1 + 1.12·|ρ|). So with n stations the frame is everywhere within
///   (n − 1)·m·(1 + 2.3·|ρ| + O(Θ⁴)), which is at most (n − 1)·m·(1 + Θ²/8) for the widest
///   step's Θ up to 3/4, the O(Θ⁴) terms being below 10⁻³·Θ² there.
/// - **Rounding.** The chord x − xᵢ carries the points' rounding, a few units in the last place of
///   X, the largest coordinate, and the step's twist moves with the chord's direction by at most
///   the tangent's turn κ·Δs ≈ κ·|x − xᵢ| per radian (when the tangent does not turn, the two
///   reflections are one and the step is the identity), so by a few times κ·X units; the step's
///   hundred other operations round unit vectors. So each station's normal is within
///   R = 2⁷·(1 + κ·X) units of 2⁻⁵³, the estimate within (16/15)·2R, and the frame, with the
///   closed form's own rounding, within 4·(n + 1)·R in all.
fn closed_form_allowance(path: &CurveKind<3>, s: &Stations) -> f64 {
    let bound = path.bound(path.domain(), &mut meter()).unwrap();
    let reach = bound
        .lo
        .iter()
        .chain(bound.hi.iter())
        .fold(0.0_f64, |m, x| m.max(x.abs()));
    let widest = s
        .frames()
        .windows(2)
        .map(|w| w[1].at - w[0].at)
        .fold(0.0, f64::max);
    let (curvature, turn) = match path {
        CurveKind::Line(_) => (0.0, 0.0),
        // τ = 0: the step is exact on an arc whatever its turn.
        CurveKind::Circle(c) => (1.0 / c.radius(), 0.0),
        CurveKind::Helix(h) => {
            let HelixAxes::Frame { radius, pitch, .. } = h.axes() else {
                unreachable!()
            };
            // The helix's parameter is its turn about the axis in half-turns.
            let rise = pitch / (2.0 * PI);
            (radius / (radius * radius + rise * rise), PI * widest)
        }
        _ => unreachable!(),
    };
    assert!(
        turn <= 0.75,
        "a step turns {turn} radians, beyond the series' range"
    );
    let steps = (s.frames().len() - 1) as f64;
    let units = 4.0 * (steps + 2.0) * 128.0 * (1.0 + curvature * reach);
    steps * s.measured_twist() * PI * (1.0 + turn * turn / 8.0) + units * f64::EPSILON / 2.0
}

#[test]
fn rotation_minimising_stations_meet_the_closed_forms_on_lines_arcs_and_helices() {
    let mut rng = Rng::new("frames: closed forms", 0x2540_0002);
    for round in 0..30 {
        let path = match round % 3 {
            0 => line(&mut rng),
            1 => circle(&mut rng),
            _ => helix(&mut rng),
        };
        let start = start_normal(&path, &mut rng);
        let tolerance = [1e-6, 1e-8, 1e-10][round % 3];
        let s = Stations::build(&path, start, 1.0, tolerance, Closing::Open, &mut meter()).unwrap();
        let rule = FrameRule::RotationMinimising(s.clone());
        let allowed = closed_form_allowance(&path, &s);
        for t in samples(&path, 40) {
            let exact = closed_form_frame(&path, start, t).unwrap().unwrap();
            let p = placement(&rule, &Laws::default(), &path, t).unwrap();
            let off = angle(p.x, exact);
            assert!(
                off <= allowed,
                "round {round}: {off} radians from the closed form at {t}, {} stations, measured {}",
                s.frames().len(),
                s.measured_twist()
            );
        }
        // On a line and an arc the double reflection is the closed form up to rounding.
        if round % 3 != 2 {
            assert!(
                s.measured_twist() < 1e-12,
                "round {round}: {}",
                s.measured_twist()
            );
        }
    }
}

#[test]
fn the_frame_meets_its_stations_bit_for_bit_and_is_c1_between_them() {
    let mut rng = Rng::new("frames: stations and C1", 0x2540_0003);
    for round in 0..16 {
        let path = if round % 2 == 0 {
            helix(&mut rng)
        } else {
            cubic(&mut rng)
        };
        let s = Stations::build(
            &path,
            start_normal(&path, &mut rng),
            1.0,
            1e-7,
            Closing::Open,
            &mut meter(),
        )
        .unwrap();
        let rule = FrameRule::RotationMinimising(s.clone());
        for f in s.frames() {
            let p = placement(&rule, &Laws::default(), &path, f.at).unwrap();
            assert_eq!(
                p.x.map(f64::to_bits),
                f.normal.map(f64::to_bits),
                "round {round} at {}",
                f.at
            );
        }
        // At each interior station the spin about the tangent is (nearly) zero from both sides,
        // and the frame's derivative is the same from both sides.
        for w in s.frames().windows(3) {
            let (before, here, after) = (w[0].at, w[1].at, w[2].at);
            let h = 1e-6 * (after - before).min(here - before).min(after - here);
            let x = |t: f64| placement(&rule, &Laws::default(), &path, t).unwrap().x;
            let (l, c, r) = (x(here - h), x(here), x(here + h));
            let left = [(c[0] - l[0]) / h, (c[1] - l[1]) / h, (c[2] - l[2]) / h];
            let right = [(r[0] - c[0]) / h, (r[1] - c[1]) / h, (r[2] - c[2]) / h];
            let size = norm(left).max(norm(right)).max(1.0);
            assert!(
                norm(sub(left, right)) <= 1e-3 * size,
                "round {round}: the frame is not C1 at {here}: {left:?} and {right:?}"
            );
            let z = tangent(&path, here);
            let spin = |d: V| dot(d, cross(z, c));
            assert!(spin(left).abs() <= 1e-3 * size && spin(right).abs() <= 1e-3 * size);
        }
    }
}

#[test]
fn stations_rebuild_bit_for_bit_and_read_back_the_same() {
    let mut rng = Rng::new("frames: bit for bit", 0x2540_0004);
    for _ in 0..12 {
        let path = cubic(&mut rng);
        let start = start_normal(&path, &mut rng);
        let one = Stations::build(&path, start, 2.0, 1e-8, Closing::Open, &mut meter()).unwrap();
        let two = Stations::build(&path, start, 2.0, 1e-8, Closing::Open, &mut meter()).unwrap();
        let bits = |s: &Stations| -> Vec<u64> {
            s.frames()
                .iter()
                .flat_map(|f| {
                    [f.at.to_bits()]
                        .into_iter()
                        .chain(f.tangent.map(f64::to_bits))
                        .chain(f.normal.map(f64::to_bits))
                })
                .collect()
        };
        assert_eq!(bits(&one), bits(&two));
        let read = Stations::from_frames(one.frames().to_vec(), one.closing()).unwrap();
        for t in samples(&path, 23) {
            let p = placement(
                &FrameRule::RotationMinimising(one.clone()),
                &Laws::default(),
                &path,
                t,
            )
            .unwrap();
            let q = placement(
                &FrameRule::RotationMinimising(read.clone()),
                &Laws::default(),
                &path,
                t,
            )
            .unwrap();
            assert_eq!(p.x.map(f64::to_bits), q.x.map(f64::to_bits));
        }
        // Stored frames that are not stations are refused.
        let mut swapped = one.frames().to_vec();
        swapped.reverse();
        assert!(matches!(
            Stations::from_frames(swapped, 0.0),
            Err(FrameError::InvalidRule(_))
        ));
        assert!(Stations::from_frames(Vec::new(), 0.0).is_err());
    }
}

/// A closed cubic B-spline, tangent-continuous at its join, that does not lie in a plane.
fn closed_loop() -> CurveKind<3> {
    let basis = Basis::new(
        3,
        vec![0.0, 0.0, 0.0, 0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let points: [[f64; 3]; 6] = [
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, -1.0],
        [1.0, 0.0, 0.0],
    ];
    CurveKind::Nurbs(Curve::new(basis, &points).unwrap())
}

#[test]
fn a_closed_paths_twist_is_spread_or_refused() {
    let path = closed_loop();
    let (a, b) = path.domain();
    let start = [0.0, 0.0, 1.0];
    let open = Stations::build(&path, start, 1.0, 1e-9, Closing::Open, &mut meter()).unwrap();
    let x = |s: &Stations, t: f64| {
        placement(
            &FrameRule::RotationMinimising(s.clone()),
            &Laws::default(),
            &path,
            t,
        )
        .unwrap()
        .x
    };
    let twist = angle(x(&open, a), x(&open, b));
    assert!(
        twist > 1e-3,
        "the loop's frame closes by itself ({twist}); pick another loop"
    );
    let spread = Stations::build(&path, start, 1.0, 1e-9, Closing::Spread, &mut meter()).unwrap();
    assert!(angle(x(&spread, a), x(&spread, b)) < 1e-12);
    assert!((spread.closing().abs() * PI - twist).abs() < 1e-9);
    assert!(matches!(
        Stations::build(&path, start, 1.0, 1e-9, Closing::Refuse, &mut meter()),
        Err(FrameError::ClosingTwist { .. })
    ));
    // A full circle closes within the tolerance, so asking for no twist is met.
    let mut rng = Rng::new("frames: full circle", 0x2540_0005);
    let full = CurveKind::Circle(
        Circle::full(
            rng.vector(3.0),
            2.0,
            Frame::new(rng.direction(), rng.direction()).unwrap(),
        )
        .unwrap(),
    );
    let s = Stations::build(
        &full,
        start_normal(&full, &mut rng),
        1.0,
        1e-9,
        Closing::Refuse,
        &mut meter(),
    )
    .unwrap();
    let (a, b) = full.domain();
    assert!(angle(x_on(&full, &s, a), x_on(&full, &s, b)) < 1e-12);
    // A closing rule on a path that is not closed is refused: an open arc, and a loop whose end is
    // its start but whose tangent there turns a corner.
    let open_arc = circle(&mut rng);
    let start = start_normal(&open_arc, &mut rng);
    let corner = CurveKind::Nurbs(
        Curve::new(
            Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap(),
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
            ],
        )
        .unwrap(),
    );
    for closing in [Closing::Spread, Closing::Refuse] {
        assert!(matches!(
            Stations::build(&open_arc, start, 1.0, 1e-9, closing, &mut meter()),
            Err(FrameError::InvalidRule(_))
        ));
        assert!(matches!(
            Stations::build(&corner, [0.0, 0.0, 1.0], 1.0, 1e-9, closing, &mut meter()),
            Err(FrameError::InvalidRule(_))
        ));
    }
    // Left open, the same loop is fine.
    Stations::build(
        &corner,
        [0.0, 0.0, 1.0],
        1.0,
        1e-9,
        Closing::Open,
        &mut meter(),
    )
    .unwrap();
}

fn x_on(path: &CurveKind<3>, s: &Stations, t: f64) -> V {
    placement(
        &FrameRule::RotationMinimising(s.clone()),
        &Laws::default(),
        path,
        t,
    )
    .unwrap()
    .x
}

#[test]
fn frenet_frames_are_certified_or_refused_where_the_curvature_may_vanish() {
    let mut rng = Rng::new("frames: Frenet", 0x2540_0006);
    for _ in 0..8 {
        let h = helix(&mut rng);
        frenet_defined(&h, &mut meter()).unwrap();
        // On a helix the principal normal points at the axis.
        let CurveKind::Helix(k) = &h else {
            unreachable!()
        };
        let HelixAxes::Frame { frame, .. } = k.axes() else {
            unreachable!()
        };
        let ([fx, fy, _], _) = frame.unit_vectors().unwrap();
        for t in samples(&h, 9) {
            let p = placement(&FrameRule::Frenet, &Laws::default(), &h, t).unwrap();
            let (c, s) = ((PI * t).cos(), (PI * t).sin());
            let toward = [
                -(c * fx[0] + s * fy[0]),
                -(c * fx[1] + s * fy[1]),
                -(c * fx[2] + s * fy[2]),
            ];
            assert!(angle(p.x, toward) < 1e-12);
        }
        frenet_defined(&cubic(&mut rng), &mut meter()).unwrap();
        let l = line(&mut rng);
        assert!(matches!(
            frenet_defined(&l, &mut meter()),
            Err(FrameError::FrameUndefined { .. })
        ));
    }
    // A planar cubic (t, t³, 0) has an inflection at 0: the span reported holds it, and is narrow.
    let inflected = CurveKind::Polynomial(
        Polynomial::new(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0; 3], [0.0, 1.0, 0.0]],
            (-1.0, 2.0),
        )
        .unwrap(),
    );
    match frenet_defined(&inflected, &mut meter()) {
        Err(FrameError::FrameUndefined { from, to }) => {
            assert!(from <= 0.0 && 0.0 <= to, "[{from}, {to}]");
            assert!(to - from <= 3.0 / 16_777_216.0 * 2.0, "[{from}, {to}]");
        }
        other => panic!("{other:?}"),
    }
    // A parabola never inflects.
    let parabola = CurveKind::Polynomial(
        Polynomial::new(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            (-1.0, 2.0),
        )
        .unwrap(),
    );
    frenet_defined(&parabola, &mut meter()).unwrap();
}

#[test]
fn fixed_references_steer_their_axis_and_refuse_a_tangent_along_them() {
    let mut rng = Rng::new("frames: fixed reference", 0x2540_0007);
    for _ in 0..8 {
        let path = helix(&mut rng);
        let CurveKind::Helix(h) = &path else {
            unreachable!()
        };
        let HelixAxes::Frame { frame, .. } = h.axes() else {
            unreachable!()
        };
        let up = unit(frame.normal());
        reference_defined(&path, up, &mut meter()).unwrap();
        for t in samples(&path, 9) {
            let z = tangent(&path, t);
            let across = unit(sub(
                up,
                [z[0] * dot(up, z), z[1] * dot(up, z), z[2] * dot(up, z)],
            ));
            let x = placement(
                &FrameRule::FixedReference {
                    reference: up,
                    axis: ProfileAxis::X,
                },
                &Laws::default(),
                &path,
                t,
            )
            .unwrap();
            assert!(angle(x.x, across) < 1e-12);
            let y = placement(
                &FrameRule::FixedReference {
                    reference: up,
                    axis: ProfileAxis::Y,
                },
                &Laws::default(),
                &path,
                t,
            )
            .unwrap();
            assert!(angle(y.y, across) < 1e-12);
        }
    }
    // (t, t², 0) runs along x at 0.
    let parabola = CurveKind::Polynomial(
        Polynomial::new(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            (-1.0, 2.0),
        )
        .unwrap(),
    );
    match reference_defined(&parabola, [1.0, 0.0, 0.0], &mut meter()) {
        Err(FrameError::FrameUndefined { from, to }) => assert!(from <= 0.0 && 0.0 <= to),
        other => panic!("{other:?}"),
    }
    let along = CurveKind::Line(Line::new([0.0; 3], [0.0, 0.0, 2.0], (0.0, 1.0)).unwrap());
    assert!(matches!(
        reference_defined(&along, [0.0, 0.0, 1.0], &mut meter()),
        Err(FrameError::FrameUndefined { .. })
    ));
    assert!(
        placement(
            &FrameRule::FixedReference {
                reference: [0.0, 0.0, 1.0],
                axis: ProfileAxis::X
            },
            &Laws::default(),
            &along,
            0.5
        )
        .is_err()
    );
}

#[test]
fn placements_are_met_and_the_turn_between_them_is_shared_evenly() {
    let path = CurveKind::Line(Line::new([0.0; 3], [0.0, 0.0, 1.0], (0.0, 4.0)).unwrap());
    let turned = |half_turns: f64| [(PI * half_turns).cos(), (PI * half_turns).sin(), 0.0];
    let list = vec![
        StationPlacement {
            at: 0.0,
            x: turned(0.0),
            z: [0.0, 0.0, 1.0],
        },
        StationPlacement {
            at: 2.0,
            x: turned(0.3),
            z: [0.0, 0.0, 1.0],
        },
        StationPlacement {
            at: 4.0,
            x: turned(0.7),
            z: [0.0, 0.0, 1.0],
        },
    ];
    let rule = FrameRule::Placements(list.clone());
    for p in &list {
        let got = placement(&rule, &Laws::default(), &path, p.at).unwrap();
        assert!(angle(got.x, p.x) < 1e-15);
    }
    for (t, expect) in [(1.0, 0.15), (0.5, 0.075), (3.0, 0.5), (3.5, 0.6)] {
        let got = placement(&rule, &Laws::default(), &path, t).unwrap();
        assert!(angle(got.x, turned(expect)) < 1e-12, "at {t}");
    }
    // Just short of a half turn, about an axis off every coordinate axis, the turn's axis is still
    // found to full accuracy (Shepperd's method; the antisymmetric part alone would lose it in
    // sin θ ≈ 3·10⁻⁶); a half turn exactly has no shorter way round, and is refused.
    let mut rng = Rng::new("frames: near a half turn", 0x2540_000b);
    let nearly = 1.0 - 1.0 / 1_048_576.0;
    for _ in 0..16 {
        let k = rng.direction();
        let z0 = rng.direction();
        let x0 = unit(cross(z0, rng.direction()));
        let rule = FrameRule::Placements(vec![
            StationPlacement {
                at: 0.0,
                x: x0,
                z: z0,
            },
            StationPlacement {
                at: 4.0,
                x: rodrigues(x0, k, nearly),
                z: rodrigues(z0, k, nearly),
            },
        ]);
        for (t, share) in [(1.0, 0.25), (2.0, 0.5), (3.0, 0.75)] {
            let got = placement(&rule, &Laws::default(), &path, t).unwrap();
            let (x, z) = (
                rodrigues(x0, k, nearly * share),
                rodrigues(z0, k, nearly * share),
            );
            assert!(
                angle(got.x, x) < 1e-14 && angle(got.z, z) < 1e-14,
                "at {t}: {} and {}",
                angle(got.x, x),
                angle(got.z, z)
            );
        }
    }
    let half = FrameRule::Placements(vec![
        StationPlacement {
            at: 0.0,
            x: [1.0, 0.0, 0.0],
            z: [0.0, 0.0, 1.0],
        },
        StationPlacement {
            at: 4.0,
            x: [-1.0, 0.0, 0.0],
            z: [0.0, 0.0, 1.0],
        },
    ]);
    assert!(matches!(
        placement(&half, &Laws::default(), &path, 2.0),
        Err(FrameError::InvalidRule(_))
    ));
}

/// `v` turned about the unit `k` by `half_turns` (Rodrigues' formula).
fn rodrigues(v: V, k: V, half_turns: f64) -> V {
    let (c, s) = ((PI * half_turns).cos(), (PI * half_turns).sin());
    let kv = cross(k, v);
    let along = dot(k, v) * (1.0 - c);
    [
        c * v[0] + s * kv[0] + along * k[0],
        c * v[1] + s * kv[1] + along * k[1],
        c * v[2] + s * kv[2] + along * k[2],
    ]
}

#[test]
fn a_helical_rule_is_a_screw_motion() {
    let mut rng = Rng::new("frames: helical", 0x2540_0008);
    let path = line(&mut rng);
    for _ in 0..16 {
        let axis = rng.direction();
        let pitch = rng.real(-5.0, 5.0);
        let rule = FrameRule::Helical {
            point: rng.vector(3.0),
            axis,
            reference: rng.direction(),
            pitch: Law::Constant(pitch),
            radius: Law::Constant(rng.real(0.5, 3.0)),
        };
        for k in 0..8 {
            let theta = -1.0 + 0.25 * f64::from(k);
            let p = placement(&rule, &Laws::default(), &path, theta).unwrap();
            let q = placement(&rule, &Laws::default(), &path, theta + 2.0).unwrap();
            // One full turn later: the same axes, bit for bit, and the origin a pitch along.
            assert_eq!(
                [p.x, p.y, p.z].map(|v| v.map(f64::to_bits)),
                [q.x, q.y, q.z].map(|v| v.map(f64::to_bits))
            );
            let moved = sub(q.origin, p.origin);
            let n = unit(axis);
            assert!(norm(sub(moved, [pitch * n[0], pitch * n[1], pitch * n[2]])) < 1e-12);
        }
    }
    // A linear pitch law rises by its integral: pitch p₀ + k·θ per turn, rise ∫ (p₀ + kθ)/2 dθ.
    let rule = FrameRule::Helical {
        point: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        reference: [1.0, 0.0, 0.0],
        pitch: Law::Linear {
            start: 2.0,
            rate: 0.5,
        },
        radius: Law::Constant(1.0),
    };
    let p = placement(&rule, &Laws::default(), &path, 4.0).unwrap();
    let integral = 2.0 * 4.0 + 0.25 * 16.0;
    assert!((p.origin[2] - integral / 2.0).abs() < 1e-12);
}

#[test]
fn twist_and_scale_laws_act_on_top_of_any_rule() {
    let mut rng = Rng::new("frames: laws", 0x2540_0009);
    for round in 0..12 {
        let path = if round % 2 == 0 {
            helix(&mut rng)
        } else {
            circle(&mut rng)
        };
        let rule = FrameRule::RotationMinimising(stations(&path, &mut rng));
        let rate = rng.real(-0.5, 0.5);
        let laws = Laws {
            twist: Some(Law::Linear { start: 0.1, rate }),
            scale: Some(Law::Linear {
                start: 1.0,
                rate: 0.01,
            }),
        };
        let (a, _) = path.domain();
        for t in samples(&path, 11) {
            let plain = placement(&rule, &Laws::default(), &path, t).unwrap();
            let lawful = placement(&rule, &laws, &path, t).unwrap();
            let s = arc_length(&path, t).unwrap();
            // On an arc or a helix the arc length is the speed times the parameter.
            let mut d = [[0.0; 3]; 2];
            path.derivatives(a, Limit::FromRight, &mut d).unwrap();
            assert!((s - norm(d[1]) * (t - a)).abs() <= 1e-12 * s.max(1.0));
            let twist = 0.1 + rate * s;
            let (c, sn) = ((PI * twist).cos(), (PI * twist).sin());
            let want = [
                c * plain.x[0] + sn * plain.y[0],
                c * plain.x[1] + sn * plain.y[1],
                c * plain.x[2] + sn * plain.y[2],
            ];
            assert!(angle(lawful.x, want) < 1e-12);
            assert!((lawful.scale - (1.0 + 0.01 * s)).abs() < 1e-12);
            assert_eq!(lawful.z, plain.z);
            // Each law alone reads the same arc length.
            let twisted = Laws {
                twist: laws.twist.clone(),
                scale: None,
            };
            let twisted = placement(&rule, &twisted, &path, t).unwrap();
            assert!(angle(twisted.x, want) < 1e-12 && twisted.scale == 1.0);
            let scaled = Laws {
                twist: None,
                scale: laws.scale.clone(),
            };
            let scaled = placement(&rule, &scaled, &path, t).unwrap();
            assert!((scaled.scale - (1.0 + 0.01 * s)).abs() < 1e-12 && scaled.x == plain.x);
        }
    }
    // A scale that is not positive is refused.
    let path = line(&mut rng);
    let shrink = Laws {
        twist: None,
        scale: Some(Law::Constant(0.0)),
    };
    assert!(matches!(
        placement(&FrameRule::Translate, &shrink, &path, path.domain().0),
        Err(FrameError::InvalidRule(_))
    ));
    let nan = Laws {
        twist: Some(Law::Constant(f64::NAN)),
        scale: None,
    };
    assert!(placement(&FrameRule::Translate, &nan, &path, path.domain().0).is_err());
}

#[test]
fn laws_are_exact_where_given_exactly() {
    let mut rng = Rng::new("frames: exact laws", 0x2540_000a);
    let q = |x: f64| RBig::try_from(x).unwrap();
    for _ in 0..64 {
        // Dyadic coefficients and points, so every product and sum is exact in doubles.
        let c: Vec<f64> = (0..=(rng.0.next_u64() % 5) as usize)
            .map(|_| (rng.real(-8.0, 8.0) * 16.0).round() / 16.0)
            .collect();
        let s = (rng.real(-2.0, 2.0) * 8.0).round() / 8.0;
        let law = Law::Polynomial(c.clone());
        let mut value = RBig::ZERO;
        let mut area = RBig::ZERO;
        let mut power = RBig::ONE;
        for (k, ck) in c.iter().enumerate() {
            value += q(*ck) * &power;
            power *= q(s);
            area += q(*ck) * &power / RBig::from(k as u64 + 1);
        }
        assert_eq!(q(law.value(s)), value);
        // The integral and derivative round, so they are held within a few units in the last place.
        let a = law.integral(s);
        let want = area.to_f64().value();
        assert!(
            (a - want).abs() <= 1e-12 * want.abs().max(1.0),
            "{c:?} at {s}: {a} against {want}"
        );
    }
    assert_eq!(
        Law::Linear {
            start: 1.5,
            rate: -0.25
        }
        .value(4.0),
        0.5
    );
    assert_eq!(
        Law::Linear {
            start: 1.5,
            rate: -0.25
        }
        .integral(4.0),
        4.0
    );
    assert_eq!(Law::Polynomial(vec![0.0, 0.0, 3.0]).derivative(2.0), 12.0);
}

#[test]
fn every_error_says_what_went_wrong_and_where() {
    let line = CurveKind::Line(Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 1.0)).unwrap());
    let stopped = match Stations::build(
        &line,
        [0.0, 0.0, 1.0],
        1.0,
        1e-9,
        Closing::Open,
        &mut WorkMeter::new(Budget::new(0, u64::MAX)),
    ) {
        Err(e @ FrameError::Stopped(_)) => e,
        other => panic!("a meter with no work left: {other:?}"),
    };
    let cases = [
        (
            FrameError::Curve(CurveError::Degenerate),
            vec!["path", "Degenerate"],
        ),
        (
            FrameError::Num(NumError::NotFinite),
            vec!["numbers", "NotFinite"],
        ),
        (
            FrameError::FrameUndefined {
                from: 0.25,
                to: 0.75,
            },
            vec!["undefined", "0.25", "0.75"],
        ),
        (
            FrameError::ZeroTangent { at: 0.375 },
            vec!["tangent", "0.375"],
        ),
        (
            FrameError::InvalidRule("a reason of its own"),
            vec!["a reason of its own"],
        ),
        (
            FrameError::ClosingTwist { half_turns: 0.125 },
            vec!["does not close", "0.125"],
        ),
        (
            FrameError::Undecided { from: 1.5, to: 2.5 },
            vec!["could not decide", "1.5", "2.5"],
        ),
        (FrameError::TooManyStations, vec!["too many stations"]),
        (FrameError::Quadrature, vec!["arc length"]),
        (stopped, vec!["stopped"]),
    ];
    for (error, words) in cases {
        let text = error.to_string();
        for w in words {
            assert!(text.contains(w), "{error:?} says {text:?}, without {w:?}");
        }
    }
}

#[test]
fn laws_that_are_not_finite_or_empty_are_refused() {
    let path = CurveKind::Line(Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 4.0)).unwrap());
    let screw = |pitch: Law, radius: Law| FrameRule::Helical {
        point: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        reference: [1.0, 0.0, 0.0],
        pitch,
        radius,
    };
    let refused = |rule: &FrameRule, laws: &Laws| {
        matches!(
            placement(rule, laws, &path, 1.0),
            Err(FrameError::InvalidRule("a law that is not finite"))
        )
    };
    // Unrefused, each of these would place the profile at an infinite origin, or (empty) give a
    // rise, radius or twist of 0, with no other error.
    let bad = [
        Law::Constant(f64::INFINITY),
        Law::Linear {
            start: 1.0,
            rate: f64::INFINITY,
        },
        Law::Linear {
            start: f64::INFINITY,
            rate: 1.0,
        },
        Law::Polynomial(Vec::new()),
        Law::Polynomial(vec![1.0, f64::INFINITY]),
        Law::Polynomial(vec![f64::NAN]),
    ];
    for law in &bad {
        let one = Law::Constant(1.0);
        assert!(
            refused(&screw(law.clone(), one.clone()), &Laws::default()),
            "pitch {law:?}"
        );
        assert!(
            refused(&screw(one, law.clone()), &Laws::default()),
            "radius {law:?}"
        );
        let twist = Laws {
            twist: Some(law.clone()),
            scale: None,
        };
        assert!(refused(&FrameRule::Translate, &twist), "twist {law:?}");
    }
    // Finite laws of the same forms are used.
    for law in [
        Law::Constant(2.0),
        Law::Linear {
            start: 1.0,
            rate: 0.5,
        },
        Law::Polynomial(vec![1.0, 0.5]),
    ] {
        let laws = Laws {
            twist: Some(law.clone()),
            scale: Some(law.clone()),
        };
        placement(&screw(law.clone(), law), &laws, &path, 1.0).unwrap();
    }
}

#[test]
fn stored_frames_are_refused_unless_they_are_stations() {
    let mut rng = Rng::new("frames: stored frames", 0x2540_000c);
    let path = cubic(&mut rng);
    let s = stations(&path, &mut rng);
    for f in s.frames() {
        assert_eq!(f.binormal(), cross(f.tangent, f.normal));
        assert!((norm(f.binormal()) - 1.0).abs() < 1e-15);
    }
    let frames = s.frames().to_vec();
    Stations::from_frames(frames.clone(), 0.25).unwrap();
    let spoilt = |change: &dyn Fn(&mut StationFrame)| {
        let mut f = frames.clone();
        change(&mut f[1]);
        Stations::from_frames(f, 0.0)
    };
    let first = frames[0].at;
    for (what, result) in [
        ("a tangent", spoilt(&|f| f.tangent[1] = f64::NAN)),
        ("a normal", spoilt(&|f| f.normal[2] = f64::INFINITY)),
        ("a parameter", spoilt(&|f| f.at = f64::NAN)),
        ("a repeated parameter", spoilt(&|f| f.at = first)),
        (
            "a closing angle",
            Stations::from_frames(frames.clone(), f64::NAN),
        ),
    ] {
        assert!(
            matches!(result, Err(FrameError::InvalidRule(_))),
            "{what}: {result:?}"
        );
    }
}

#[test]
fn a_path_is_read_from_the_left_at_its_end_and_from_the_right_inside() {
    // A polyline with a corner at 1: (0, 0, 0) to (1, 0, 0) to (1, 1, 0).
    let corner = Curve::new(
        Basis::new(1, vec![0.0, 0.0, 1.0, 2.0, 2.0]).unwrap(),
        &[[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
    )
    .unwrap();
    let fixed = FrameRule::FixedReference {
        reference: [0.0, 0.0, 1.0],
        axis: ProfileAxis::X,
    };
    let z = |path: &CurveKind<3>, t: f64| placement(&fixed, &Laws::default(), path, t).unwrap().z;
    // Inside the domain the corner is read from the right: the next leg's direction.
    let whole = CurveKind::Nurbs(corner.clone());
    assert_eq!(z(&whole, 1.0), [0.0, 1.0, 0.0]);
    // Trimmed to end at the corner, the path's end is read from the left: its own last direction,
    // not the leg beyond the trim.
    let first_leg =
        CurveKind::Trimmed(Trimmed::new(geom_curve::Basis::Nurbs(corner), (0.0, 1.0)).unwrap());
    assert_eq!(z(&first_leg, 1.0), [1.0, 0.0, 0.0]);
}

#[test]
fn stations_refuse_a_radius_tolerance_or_domain_they_cannot_meet() {
    let line = CurveKind::Line(Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 1.0)).unwrap());
    let build = |path: &CurveKind<3>, radius: f64, tolerance: f64| {
        Stations::build(
            path,
            [0.0, 0.0, 1.0],
            radius,
            tolerance,
            Closing::Open,
            &mut meter(),
        )
    };
    for (radius, tolerance) in [
        (1.0, 0.0),
        (1.0, -1e-9),
        (1.0, f64::INFINITY),
        (1.0, f64::NAN),
        (0.0, 1e-9),
        (-1.0, 1e-9),
        (f64::INFINITY, 1e-9),
        (f64::NAN, 1e-9),
    ] {
        assert!(
            matches!(
                build(&line, radius, tolerance),
                Err(FrameError::InvalidRule(
                    "a radius or tolerance that is not positive"
                ))
            ),
            "radius {radius}, tolerance {tolerance}"
        );
    }
    // A ray, or a line infinite at its start, has no finite domain to place stations on.
    for domain in [
        (0.0, f64::INFINITY),
        (f64::NEG_INFINITY, 0.0),
        (f64::NEG_INFINITY, f64::INFINITY),
    ] {
        let ray = CurveKind::Line(Line::new([0.0; 3], [1.0, 0.0, 0.0], domain).unwrap());
        assert!(
            matches!(
                build(&ray, 1.0, 1e-9),
                Err(FrameError::InvalidRule("a path with no finite domain"))
            ),
            "{domain:?}"
        );
    }
}

/// A closed cubic B-spline like [`closed_loop`], on the domain [1, 4], its control points' heights
/// `lift` (1 in [`closed_loop`]) and its fifth control point moved by `lean` along x, so that its
/// end tangent turns from its start tangent by atan(lean / √(1 + lift²)).
fn leaning_loop(lean: f64, lift: f64) -> CurveKind<3> {
    let basis = Basis::new(3, vec![1.0, 1.0, 1.0, 1.0, 2.0, 3.0, 4.0, 4.0, 4.0, 4.0]).unwrap();
    let points: [[f64; 3]; 6] = [
        [1.0, 0.0, 0.0],
        [1.0, 1.0, lift],
        [-1.0, 1.0, -lift],
        [-1.0, -1.0, lift],
        [1.0 + lean, -1.0, -lift],
        [1.0, 0.0, 0.0],
    ];
    CurveKind::Nurbs(Curve::new(basis, &points).unwrap())
}

#[test]
fn a_closing_rule_needs_the_end_tangent_within_the_tolerance() {
    // With a radius r, a tangent that turns by θ radians at the join twists the profile's edge by
    // r·θ, so the join is closed when r·θ is within the tolerance.
    let tolerance = 1e-3;
    let lean = |radians: f64| radians.tan() * 2.0_f64.sqrt();
    for (radius, radians, closed) in [
        (1.0, 0.5 * tolerance, true),
        (1.0, 2.0 * tolerance, false),
        (2.0, 0.4 * tolerance, true),
        (2.0, 0.6 * tolerance, false),
    ] {
        let path = leaning_loop(lean(radians), 1.0);
        let built = Stations::build(
            &path,
            [0.0, 0.0, 1.0],
            radius,
            tolerance,
            Closing::Spread,
            &mut meter(),
        );
        let refused = matches!(
            built,
            Err(FrameError::InvalidRule(
                "a closing rule on a path that is not closed"
            ))
        );
        assert_eq!(
            refused, !closed,
            "radius {radius}, {radians} radians: {built:?}"
        );
    }
}

#[test]
fn a_spread_closing_angle_turns_the_frame_in_proportion_to_the_parameter() {
    let path = leaning_loop(0.0, 1.0);
    let (a, b) = path.domain();
    let build = |closing: Closing| {
        Stations::build(&path, [0.0, 0.0, 1.0], 1.0, 1e-9, closing, &mut meter()).unwrap()
    };
    let (open, spread) = (build(Closing::Open), build(Closing::Spread));
    assert_eq!(open.frames(), spread.frames());
    assert_eq!(open.closing(), 0.0);
    assert!(spread.closing().abs() > 1e-3, "{}", spread.closing());
    for t in samples(&path, 12) {
        let at = |s: &Stations| {
            placement(
                &FrameRule::RotationMinimising(s.clone()),
                &Laws::default(),
                &path,
                t,
            )
            .unwrap()
        };
        let (p, q) = (at(&open), at(&spread));
        let turned = dot(cross(p.x, q.x), p.z).atan2(dot(p.x, q.x));
        let want = -spread.closing() * PI * (t - a) / (b - a);
        assert!(
            (turned - want).abs() < 1e-12,
            "at {t}: {turned} against {want}"
        );
    }
}

#[test]
fn a_placement_puts_a_profile_point_at_its_origin_plus_its_scaled_axes() {
    let mut rng = Rng::new("frames: place", 0x2540_000d);
    let path = helix(&mut rng);
    let rule = FrameRule::RotationMinimising(stations(&path, &mut rng));
    let laws = Laws {
        twist: Some(Law::Constant(0.25)),
        scale: Some(Law::Linear {
            start: 2.0,
            rate: 0.125,
        }),
    };
    for t in samples(&path, 7) {
        let p = placement(&rule, &laws, &path, t).unwrap();
        assert_eq!(p.place([0.0, 0.0]), p.origin);
        for _ in 0..4 {
            let (u, v) = (rng.real(-2.0, 2.0), rng.real(-2.0, 2.0));
            let want: V =
                core::array::from_fn(|i| p.origin[i] + p.scale * (u * p.x[i] + v * p.y[i]));
            let got = p.place([u, v]);
            assert!(
                norm(sub(got, want)) <= 1e-14 * (1.0 + norm(want)),
                "{got:?} against {want:?}"
            );
        }
    }
}

#[test]
fn arc_length_is_exact_in_closed_form_and_by_quadrature_elsewhere() {
    let mut rng = Rng::new("frames: arc length", 0x2540_000e);
    for round in 0..48 {
        let path = match round % 3 {
            0 => line(&mut rng),
            1 => circle(&mut rng),
            _ => helix(&mut rng),
        };
        let speed = match &path {
            CurveKind::Line(l) => norm(l.direction()),
            CurveKind::Circle(c) => PI * c.radius(),
            CurveKind::Helix(h) => {
                let HelixAxes::Frame { radius, pitch, .. } = h.axes() else {
                    unreachable!()
                };
                let (round, rise) = (PI * radius, 0.5 * pitch);
                (round * round + rise * rise).sqrt()
            }
            _ => unreachable!(),
        };
        let (a, _) = path.domain();
        for t in samples(&path, 9) {
            assert_eq!(
                arc_length(&path, t).unwrap(),
                speed * (t - a),
                "round {round} at {t}"
            );
        }
    }
    // A Pythagorean-hodograph cubic, (t − t³/3, t², 0), whose speed is 1 + t², on a domain mostly
    // left of 0: its arc length from −2 to t is (t + 2) + (t³ + 8)/3.
    let ph = CurveKind::Polynomial(
        Polynomial::new(
            vec![
                [0.0; 3],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [-1.0 / 3.0, 0.0, 0.0],
            ],
            (-2.0, 1.0),
        )
        .unwrap(),
    );
    for t in [-2.0, -1.5, -1.0, -0.25, 0.0, 0.5, 1.0] {
        let want = (t + 2.0) + (t * t * t + 8.0) / 3.0;
        let got = arc_length(&ph, t).unwrap();
        assert!(
            (got - want).abs() <= 1e-12 * want.max(1.0),
            "at {t}: {got} against {want}"
        );
    }
}

#[test]
fn a_lines_frenet_frame_is_undefined_on_its_whole_domain() {
    let mut rng = Rng::new("frames: Frenet on a line", 0x2540_000f);
    for _ in 0..8 {
        let l = line(&mut rng);
        let (a, b) = l.domain();
        assert_eq!(
            frenet_defined(&l, &mut meter()),
            Err(FrameError::FrameUndefined { from: a, to: b })
        );
    }
}

#[test]
fn the_turn_between_placements_is_shared_out_about_any_axis_by_any_angle() {
    let path = CurveKind::Line(Line::new([0.0; 3], [0.0, 0.0, 1.0], (0.0, 4.0)).unwrap());
    let mut rng = Rng::new("frames: placement turns", 0x2540_0010);
    // The coordinate axes and axes in each coordinate plane put each of Shepperd's four cases
    // where another would lose accuracy: small turns, where w is the largest component, and
    // nearly half turns, where one of q's is.
    let mut axes: Vec<V> = vec![
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.6, 0.8, 0.0],
        [0.0, 0.6, 0.8],
        [0.8, 0.0, 0.6],
    ];
    for _ in 0..6 {
        axes.push(rng.direction());
    }
    let tiny = 1.0 / 1_048_576.0;
    for k in axes {
        for turn in [tiny, 0.3, 1.0 - tiny] {
            let z0 = rng.direction();
            let x0 = unit(cross(z0, rng.direction()));
            let rule = FrameRule::Placements(vec![
                StationPlacement {
                    at: 0.0,
                    x: x0,
                    z: z0,
                },
                StationPlacement {
                    at: 4.0,
                    x: rodrigues(x0, k, turn),
                    z: rodrigues(z0, k, turn),
                },
            ]);
            for (t, share) in [(1.0, 0.25), (2.0, 0.5), (3.0, 0.75)] {
                let got = placement(&rule, &Laws::default(), &path, t).unwrap();
                let (x, z) = (
                    rodrigues(x0, k, turn * share),
                    rodrigues(z0, k, turn * share),
                );
                assert!(
                    angle(got.x, x) < 1e-14 && angle(got.z, z) < 1e-14,
                    "{k:?} by {turn} at {t}: {} and {}",
                    angle(got.x, x),
                    angle(got.z, z)
                );
            }
        }
    }
}

#[test]
fn placements_a_half_turn_apart_are_met_at_their_parameters_and_refused_between() {
    let path = CurveKind::Line(Line::new([0.0; 3], [0.0, 0.0, 1.0], (0.0, 4.0)).unwrap());
    let half = FrameRule::Placements(vec![
        StationPlacement {
            at: 0.0,
            x: [1.0, 0.0, 0.0],
            z: [0.0, 0.0, 1.0],
        },
        StationPlacement {
            at: 4.0,
            x: [-1.0, 0.0, 0.0],
            z: [0.0, 0.0, 1.0],
        },
    ]);
    let x = |t: f64| placement(&half, &Laws::default(), &path, t).map(|p| p.x);
    assert_eq!(x(0.0), Ok([1.0, 0.0, 0.0]));
    assert_eq!(x(4.0), Ok([-1.0, 0.0, 0.0]));
    for t in [1e-12, 2.0, 4.0 - 1e-12] {
        assert!(matches!(x(t), Err(FrameError::InvalidRule(_))), "at {t}");
    }
}

#[test]
fn every_placed_step_meets_the_tolerance_on_any_domain() {
    let mut rng = Rng::new("frames: steps within the tolerance", 0x2540_0011);
    for round in 0..12 {
        // Domains left of 0, about 0 and right of it.
        let shift = [-7.0, -0.5, 6.0][round % 3];
        let a = shift + rng.real(0.0, 0.5);
        let path = if round % 2 == 0 {
            let frame = Frame::new(rng.direction(), rng.direction()).unwrap();
            let axes = HelixAxes::Frame {
                frame,
                radius: rng.real(0.5, 3.0),
                pitch: rng.real(0.5, 6.0),
            };
            let domain = (a, a + rng.real(1.0, 3.0));
            CurveKind::Helix(Helix::new(rng.vector(5.0), axes, domain).unwrap())
        } else {
            let k = rng.real(0.5, 2.0);
            let coefficients = vec![
                rng.vector(3.0),
                [1.0, 0.0, 0.0],
                [0.0, k, 0.0],
                [0.0, 0.0, k * 0.5],
            ];
            let domain = (a, a + rng.real(1.0, 1.5));
            CurveKind::Polynomial(Polynomial::new(coefficients, domain).unwrap())
        };
        let radius = rng.real(0.5, 2.0);
        let tolerance = [1e-6, 1e-8][round % 2];
        let s = Stations::build(
            &path,
            start_normal(&path, &mut rng),
            radius,
            tolerance,
            Closing::Open,
            &mut meter(),
        )
        .unwrap();
        assert!(
            s.measured_twist() <= tolerance / radius / PI,
            "round {round}: {} stations, measured {}",
            s.frames().len(),
            s.measured_twist()
        );
    }
}

#[test]
fn asking_for_no_twist_refuses_a_closing_angle_only_beyond_the_tolerance() {
    // A loop lifted a fifth as far as `closed_loop`: its quarters' steps err less than a fifth of
    // its closing angle, so for any tolerance near that angle the stations are the quarters and
    // the angle stays the same. Of a few such loops, take one whose angle is exactly the limit,
    // tolerance / radius / π half-turns, of some tolerance.
    let start = [0.0, 0.0, 1.0];
    let build = |path: &CurveKind<3>, tolerance: f64, closing: Closing| {
        Stations::build(path, start, 1.0, tolerance, closing, &mut meter())
    };
    let limit = |tolerance: f64| tolerance / 1.0 / PI;
    let (path, angle, tolerance) = [0.2, 0.21, 0.22, 0.23, 0.24, 0.25]
        .into_iter()
        .find_map(|lift| {
            let path = leaning_loop(0.0, lift);
            let quarters = build(&path, 1.0, Closing::Spread).unwrap();
            let angle = quarters.closing().abs();
            assert!(quarters.frames().len() == 5 && quarters.measured_twist() < 0.2 * angle);
            let mut tolerance = angle * PI;
            for _ in 0..8 {
                if limit(tolerance) < angle {
                    tolerance = tolerance.next_up();
                } else if limit(tolerance) > angle {
                    tolerance = tolerance.next_down();
                }
            }
            (limit(tolerance) == angle).then_some((path, angle, tolerance))
        })
        .unwrap();
    // An angle equal to the limit is within the tolerance: it is spread.
    let met = build(&path, tolerance, Closing::Refuse).unwrap();
    assert_eq!(met.closing().abs(), angle);
    // The next tolerance down whose limit is below the angle refuses it.
    let mut below = tolerance;
    while limit(below) >= angle {
        below = below.next_down();
    }
    assert!(matches!(
        build(&path, below, Closing::Refuse),
        Err(FrameError::ClosingTwist { half_turns }) if half_turns.abs() == angle
    ));
}

#[test]
fn a_certified_test_examines_exactly_its_budget_of_spans() {
    // (t³, t³ + εt², 0) on [1, 2]: C' × C'' = (0, 0, −6εt²) is small beside the two products it is
    // the difference of (each about 18t³), so its bounds decide a span only once the span is about
    // ε/9 wide or less: with ε = 0.00078 every span of 2⁻¹⁴ of the domain is decided and none of
    // 2⁻¹³. Depth first, the whole domain is the first span examined and the first half's
    // 2¹⁴ − 1 spans the next, which spends MAX_SPANS; the second half is the first span past it.
    assert_eq!(MAX_SPANS, 1 << 14);
    let eps = 0.00078;
    let path = CurveKind::Polynomial(
        Polynomial::new(
            vec![[0.0; 3], [0.0; 3], [0.0, eps, 0.0], [1.0, 1.0, 0.0]],
            (1.0, 2.0),
        )
        .unwrap(),
    );
    assert_eq!(
        frenet_defined(&path, &mut meter()),
        Err(FrameError::Undecided { from: 1.5, to: 2.0 })
    );
}
