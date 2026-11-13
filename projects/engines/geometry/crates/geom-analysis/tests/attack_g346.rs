//! Attacks on piece G346's claims (`continuity`, with `geom_surface::class_a`).
//!
//! - **Certified, even when stopped short.** A cap of one piece, or of none, still returns ends
//!   that hold the true maximum, only not as tight as asked; a run past its work budget stops
//!   with the budget's error.
//! - **Breaks.** A bicubic patch with interior knots in both directions, its edge crossing the
//!   knot lines, keeps every value along the edge under the upper end, as tight as asked.
//! - **Undefined spans reported.** An edge that runs into a sphere's pole, where the normal
//!   vanishes, has an unresolved span ending at the pole, never a bound.
//! - **Far from the origin.** The same join moved 10⁶ away has the same maximum.
//! - **Deterministic, sense-blind.** Two runs give the same bits; turning both faces gives the
//!   same maximum.
//! - **Refusals.** A tightness that is negative, infinite or NaN; pcurves over different ranges.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "tests: oracles and generators, and a failure stops the test"
)]

mod common;

use common::{along, graph, meter, strip};
use geom_analysis::continuity::{self, Ask, ContinuityError, Maximum, Upper};
use geom_curve::{CurveKind, Frame, Line};
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::class_a::{self, Limit, Measure, Side};
use geom_surface::{Plane, Sphere, SurfaceKind};

fn pair<'a>(a: &'a SurfaceKind, b: &'a SurfaceKind, p: &'a CurveKind<2>) -> [Side<'a>; 2] {
    [
        Side {
            surface: a,
            pcurve: p,
            turned: false,
        },
        Side {
            surface: b,
            pcurve: p,
            turned: false,
        },
    ]
}

fn upper(m: &Maximum) -> f64 {
    match m.upper {
        Upper::Bounded(u) => u,
        Upper::Unresolved { from, to } => panic!("unresolved over [{from}, {to}]"),
    }
}

const H: [f64; 3] = [1.0, 1.5, 1.0];

#[test]
fn a_capped_run_is_still_certified() {
    let (a, b, p) = (strip(0.0), graph(&[0.0, 0.0, 1.0], &H), along(0.0));
    for cap in [0, 1, 3] {
        let ask = Ask::new(Measure::Curvature, 1e-9).unwrap().with_pieces(cap);
        assert_eq!(ask.pieces(), cap);
        let m = continuity::join(pair(&a, &b, &p), ask, &mut meter()).unwrap();
        assert!(m.lower() <= 2.5 && 2.5 <= upper(&m), "cap {cap}: {m:?}");
        assert!(!m.as_asked && m.pieces <= cap.max(1), "cap {cap}: {m:?}");
    }
}

#[test]
fn a_run_past_its_budget_stops() {
    let (a, b, p) = (strip(0.0), graph(&[0.0, 0.0, 1.0], &H), along(0.0));
    let ask = Ask::new(Measure::Curvature, 1e-9).unwrap();
    for work in [0, 5, 50] {
        let mut m = WorkMeter::new(Budget::new(work, u64::MAX));
        assert!(
            matches!(
                continuity::join(pair(&a, &b, &p), ask, &mut m),
                Err(ContinuityError::Stopped(_))
            ),
            "work {work}"
        );
    }
}

/// A bicubic patch with one interior knot in u and three in v, meeting the strip G1 along u = 0
/// with a curvature jump that changes across each knot of v.
fn knotted() -> SurfaceKind {
    let bu = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let bv = Basis::new(
        3,
        vec![0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let heights = [0.3, 0.8, 0.5, 1.1, 0.4, 0.9, 0.6];
    let mut pts = Vec::new();
    for i in 0..5 {
        for (j, h) in heights.iter().enumerate() {
            let z = if i < 2 { 0.0 } else { *h * (i as f64 - 1.0) };
            pts.push([i as f64 / 4.0, j as f64 / 6.0, z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(bu, bv, &pts).unwrap())
}

#[test]
fn an_edge_across_knot_lines_keeps_every_value_under_its_maximum() {
    let (a, b, p) = (strip(0.0), knotted(), along(0.0));
    let s = pair(&a, &b, &p);
    for m in [Measure::Gap, Measure::Curvature, Measure::CurvatureChange] {
        let x = continuity::join(s, Ask::new(m, 1e-6).unwrap(), &mut meter()).unwrap();
        let u = upper(&x);
        assert!(x.as_asked, "{m:?}: {x:?}");
        for k in 0..=400 {
            let t = f64::from(k) / 400.0;
            for side in [Limit::FromLeft, Limit::FromRight] {
                if (t == 0.0 && side == Limit::FromLeft) || (t == 1.0 && side == Limit::FromRight) {
                    continue;
                }
                let v = class_a::at(m, s, t, side, &mut meter())
                    .unwrap()
                    .value()
                    .unwrap();
                assert!(v.lo() <= u, "{m:?} at {t} {side:?}: {v:?} above {x:?}");
            }
        }
    }
}

#[test]
fn an_edge_into_a_pole_is_unresolved_there() {
    let sphere = SurfaceKind::Sphere(
        Sphere::new(
            [0.0; 3],
            2.0,
            Frame::standard().unwrap(),
            ((0.0, 1.0), (0.0, 0.5)),
        )
        .unwrap(),
    );
    // The meridian u = 1/2 from the equator (v = 0) to the north pole (v = 1/2).
    let p = CurveKind::Line(Line::new([0.5, 0.0], [0.0, 0.5], (0.0, 1.0)).unwrap());
    let m = continuity::join(
        pair(&sphere, &sphere, &p),
        Ask::new(Measure::NormalAngle, 1e-6).unwrap(),
        &mut meter(),
    )
    .unwrap();
    match m.upper {
        Upper::Unresolved { from, to } => {
            assert!(from < to && to == 1.0 && from > 0.99, "{m:?}");
        }
        Upper::Bounded(_) => panic!("a pole bounded: {m:?}"),
    }
    assert!(!m.as_asked && m.witness.is_some(), "{m:?}");
    // The gap needs no normal, so it is bounded right up to the pole.
    let g = continuity::join(
        pair(&sphere, &sphere, &p),
        Ask::new(Measure::Gap, 1e-6).unwrap(),
        &mut meter(),
    )
    .unwrap();
    assert!(upper(&g) <= 1e-6 && g.as_asked, "{g:?}");
}

/// The strip and the graph z = x²·h(y) moved by `d`.
fn moved(d: f64) -> (SurfaceKind, SurfaceKind) {
    let a = SurfaceKind::Plane(
        Plane::new(
            [d, d, d],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-1.0, 0.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    let k = [1.0, 1.5, 1.0];
    let mut pts = Vec::new();
    for i in 0..3 {
        for (j, h) in k.iter().enumerate() {
            let z = if i == 2 { *h } else { 0.0 };
            pts.push([d + i as f64 / 2.0, d + j as f64 / 2.0, d + z]);
        }
    }
    let basis = || Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    (
        a,
        SurfaceKind::Nurbs(Surface::new(basis(), basis(), &pts).unwrap()),
    )
}

#[test]
fn far_from_the_origin_the_maximum_is_the_same() {
    let p = along(0.0);
    for d in [0.0, 1e3, 1e6, -1e6] {
        let (a, b) = moved(d);
        let m = continuity::join(
            pair(&a, &b, &p),
            Ask::new(Measure::Curvature, 1e-5).unwrap(),
            &mut meter(),
        )
        .unwrap();
        assert!(
            m.lower() <= 2.5 && 2.5 <= upper(&m) && m.as_asked,
            "at {d}: {m:?}"
        );
        assert!((m.witness.unwrap().at - 0.5).abs() < 0.05, "at {d}: {m:?}");
    }
}

#[test]
fn runs_repeat_bit_for_bit_and_ignore_turning_both_faces() {
    let (a, b, p) = (strip(0.0), graph(&[0.0, 0.0, 1.0], &H), along(0.0));
    let s = pair(&a, &b, &p);
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
    for m in [
        Measure::Gap,
        Measure::NormalAngle,
        Measure::Curvature,
        Measure::CurvatureChange,
    ] {
        let ask = Ask::new(m, 1e-6).unwrap();
        let x = continuity::join(s, ask, &mut meter()).unwrap();
        assert_eq!(x, continuity::join(s, ask, &mut meter()).unwrap(), "{m:?}");
        let y = continuity::join(both, ask, &mut meter()).unwrap();
        assert!(
            (x.lower() - y.lower()).abs() < 1e-9 && (upper(&x) - upper(&y)).abs() < 1e-9,
            "{m:?}: {x:?} {y:?}"
        );
    }
}

#[test]
fn bad_asks_and_mismatched_pcurves_are_refused() {
    for t in [-1e-9, f64::INFINITY, f64::NAN] {
        assert_eq!(
            Ask::new(Measure::Gap, t),
            Err(ContinuityError::Tight),
            "{t}"
        );
    }
    let ask = Ask::new(Measure::Gap, 0.0).unwrap();
    assert_eq!((ask.measure(), ask.tight()), (Measure::Gap, 0.0));
    let (a, b) = (strip(0.0), graph(&[0.0, 0.0, 1.0], &H));
    let (p, q) = (
        along(0.0),
        CurveKind::Line(Line::new([0.0, 0.0], [0.0, 0.5], (0.0, 2.0)).unwrap()),
    );
    let s = [
        Side {
            surface: &a,
            pcurve: &p,
            turned: false,
        },
        Side {
            surface: &b,
            pcurve: &q,
            turned: false,
        },
    ];
    assert_eq!(
        continuity::join(s, ask, &mut meter()),
        Err(ContinuityError::Mismatch)
    );
    for e in [
        ContinuityError::Tight,
        ContinuityError::Mismatch,
        ContinuityError::NoSuchRow,
    ] {
        assert!(!e.to_string().is_empty());
    }
}
