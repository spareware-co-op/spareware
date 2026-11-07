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
use geom_curve::{Circle, CurveKind, Frame, Helix, HelixAxes, Line, ParamCurve, Polynomial};
use geom_nurbs::{Basis, Curve, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::frame_rule::{
    Closing, FrameError, FrameRule, Law, Laws, ProfileAxis, StationPlacement, Stations, arc_length,
    closed_form_frame, frenet_defined, placement, reference_defined,
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
                        at: 0.5 * (a + b),
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
        // The stations' estimate is per step; over n steps the errors add.
        let allowed = 2.0 * s.frames().len() as f64 * s.measured_twist() * PI + 1e-11;
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
    assert!((p.origin[2] - 0.5 * (2.0 * 4.0 + 0.25 * 16.0)).abs() < 1e-12);
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
        let c: Vec<f64> = (0..1 + (rng.0.next_u64() % 5) as usize)
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
