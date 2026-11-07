//! The replay hash of piece G254: the frame rules run over a fixed corpus and hashed with SHA-256
//! must give the same digest on every target (chapters 21 and 28.13; the golden store of piece
//! G19).
//!
//! The corpus: a circular helix, a twisted cubic, a circular arc and a closed cubic B-spline; on
//! each, rotation-minimising stations at two tolerances (every station's parameter, tangent and
//! normal, the closing angle and the measured twist), and placements at 13 parameters under every
//! rule (translate, rotation-minimising, Frenet, fixed reference and up-vector, placements,
//! helical), plain and with twist and scale laws; the certified Frenet and fixed-reference tests'
//! answers; and the refusals. Every double is written as its bits.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::unwrap_used,
    clippy::panic,
    reason = "the corpus generator; the rules are for kernel code"
)]

use geom_curve::{Circle, CurveKind, Frame, Helix, HelixAxes, ParamCurve, Polynomial};
use geom_nurbs::{Basis, Curve};
use geom_run::{Budget, WorkMeter};
use geom_surface::frame_rule::{
    Closing, FrameError, FrameRule, Law, Laws, ProfileAxis, StationPlacement, Stations,
    frenet_defined, placement, reference_defined,
};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-surface/frames";

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn put(out: &mut Vec<u8>, x: f64) {
    out.extend(x.to_bits().to_le_bytes());
}

fn put_v(out: &mut Vec<u8>, v: [f64; 3]) {
    for x in v {
        put(out, x);
    }
}

/// An error as a tag and, where it has one, its span.
fn put_err(out: &mut Vec<u8>, e: &FrameError) {
    let (tag, a, b) = match e {
        FrameError::Curve(_) => (1u8, 0.0, 0.0),
        FrameError::Num(_) => (2, 0.0, 0.0),
        FrameError::FrameUndefined { from, to } => (3, *from, *to),
        FrameError::ZeroTangent { at } => (4, *at, 0.0),
        FrameError::InvalidRule(_) => (5, 0.0, 0.0),
        FrameError::ClosingTwist { half_turns } => (6, *half_turns, 0.0),
        FrameError::Undecided { from, to } => (7, *from, *to),
        FrameError::TooManyStations => (8, 0.0, 0.0),
        FrameError::Quadrature => (9, 0.0, 0.0),
        FrameError::Stopped(_) => (10, 0.0, 0.0),
    };
    out.push(tag);
    put(out, a);
    put(out, b);
}

fn paths() -> Vec<CurveKind<3>> {
    let basis = Basis::new(
        3,
        vec![0.0, 0.0, 0.0, 0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let loop_points: [[f64; 3]; 6] = [
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, -1.0],
        [1.0, 0.0, 0.0],
    ];
    vec![
        CurveKind::Helix(
            Helix::new(
                [0.5, -1.0, 2.0],
                HelixAxes::Frame {
                    frame: Frame::new([0.6, 0.8, 0.0], [0.0, 0.6, 0.8]).unwrap(),
                    radius: 1.75,
                    pitch: -2.5,
                },
                (-1.0, 3.5),
            )
            .unwrap(),
        ),
        CurveKind::Polynomial(
            Polynomial::new(
                vec![
                    [0.0, 1.0, 0.0],
                    [1.0, 0.0, 0.25],
                    [0.0, 1.5, 0.0],
                    [0.0, 0.0, 0.75],
                ],
                (0.1, 1.9),
            )
            .unwrap(),
        ),
        CurveKind::Circle(
            Circle::new(
                [1.0, 2.0, 3.0],
                2.5,
                Frame::new([1.0, 1.0, 0.0], [1.0, -1.0, 1.0]).unwrap(),
                (0.2, 1.4),
            )
            .unwrap(),
        ),
        CurveKind::Nurbs(Curve::new(basis, &loop_points).unwrap()),
    ]
}

fn corpus() -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut placements = 0usize;
    for path in paths() {
        let (a, b) = path.domain();
        let params: Vec<f64> = (0..=12)
            .map(|k| {
                if k == 12 {
                    b
                } else {
                    a + (b - a) * f64::from(k) / 12.0
                }
            })
            .collect();
        let start = [0.3, -0.4, 0.85];
        let mut rules = vec![
            FrameRule::Translate,
            FrameRule::Frenet,
            FrameRule::FixedReference {
                reference: [0.0, 0.0, 1.0],
                axis: ProfileAxis::X,
            },
            FrameRule::FixedReference {
                reference: [0.0, 0.0, 1.0],
                axis: ProfileAxis::Y,
            },
            FrameRule::Placements(vec![
                StationPlacement {
                    at: a,
                    x: [1.0, 0.0, 0.0],
                    z: [0.0, 0.0, 1.0],
                },
                StationPlacement {
                    at: 0.5 * a + 0.5 * b,
                    x: [0.0, 1.0, 0.0],
                    z: [0.6, 0.0, 0.8],
                },
                StationPlacement {
                    at: b,
                    x: [0.0, 0.0, 1.0],
                    z: [1.0, 0.0, 0.0],
                },
            ]),
            FrameRule::Helical {
                point: [0.0, 0.0, 0.0],
                axis: [0.0, 1.0, 1.0],
                reference: [1.0, 0.0, 0.0],
                pitch: Law::Linear {
                    start: 1.0,
                    rate: 0.125,
                },
                radius: Law::Polynomial(vec![2.0, 0.0, 0.0625]),
            },
        ];
        for (tolerance, closing) in [(1e-6, Closing::Open), (1e-10, Closing::Spread)] {
            match Stations::build(&path, start, 1.5, tolerance, closing, &mut meter()) {
                Ok(s) => {
                    out.push(1);
                    for f in s.frames() {
                        put(&mut out, f.at);
                        put_v(&mut out, f.tangent);
                        put_v(&mut out, f.normal);
                    }
                    put(&mut out, s.closing());
                    put(&mut out, s.measured_twist());
                    rules.push(FrameRule::RotationMinimising(s));
                }
                Err(e) => put_err(&mut out, &e),
            }
        }
        let laws = Laws {
            twist: Some(Law::Linear {
                start: 0.25,
                rate: -0.0625,
            }),
            scale: Some(Law::Polynomial(vec![1.0, 0.125, -0.015625])),
        };
        for rule in &rules {
            for laws in [Laws::default(), laws.clone()] {
                for t in &params {
                    match placement(rule, &laws, &path, *t) {
                        Ok(p) => {
                            placements += 1;
                            out.push(1);
                            put_v(&mut out, p.origin);
                            put_v(&mut out, p.x);
                            put_v(&mut out, p.y);
                            put_v(&mut out, p.z);
                            put(&mut out, p.scale);
                        }
                        Err(e) => put_err(&mut out, &e),
                    }
                }
            }
        }
        for answer in [
            frenet_defined(&path, &mut meter()),
            reference_defined(&path, [0.0, 0.0, 1.0], &mut meter()),
        ] {
            match answer {
                Ok(()) => out.push(1),
                Err(e) => put_err(&mut out, &e),
            }
        }
    }
    // Refusals.
    let path = paths().swap_remove(1);
    for e in [
        Stations::build(
            &path,
            [1.0, 0.0, 0.25],
            1.0,
            1e-9,
            Closing::Open,
            &mut meter(),
        )
        .err(),
        Stations::build(
            &path,
            [0.0, 1.0, 0.0],
            0.0,
            1e-9,
            Closing::Open,
            &mut meter(),
        )
        .err(),
        Stations::from_frames(Vec::new(), 0.0).err(),
        placement(
            &FrameRule::Translate,
            &Laws {
                twist: None,
                scale: Some(Law::Constant(-1.0)),
            },
            &path,
            1.0,
        )
        .err(),
    ]
    .into_iter()
    .flatten()
    {
        put_err(&mut out, &e);
    }
    (out, placements)
}

#[test]
fn frames_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let (bytes, placements) = corpus();
        (sha256(&bytes), (bytes, placements))
    });
    let (bytes, placements) = &checked.value;
    println!(
        "geom-surface frames corpus: {placements} placements, {} bytes, golden hash {}",
        bytes.len(),
        hex(sha256(bytes))
    );
    assert!(*placements > 600, "only {placements} placements");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target rounds a reflection, a square root or an \
             angle differently (compare this log's digest with another target's), or a rule's \
             definition changed in place, which chapter 26.2 forbids"
        )
    });
}
