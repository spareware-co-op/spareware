//! Piece G361's surveyed 3D linework (chapter 17.2): lines, arcs in plan on a steady grade and
//! smooth figures, joined into one composite curve within `ε_fit` of every surveyed point.
//!
//! Oracles: each arc's circumcenter and radius in exact rationals (`dashu-ratio`), against the
//! stored helix or level arc; each point against the curve evaluated at its parameter, in doubles;
//! the grade of each helix piece against its points' rise over its turn; and the kinds a figure
//! becomes against chapter 17.2's rule. Seeded arcs on random circles print their seed
//! (`SPAREKIT_SEED` replays one).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests: oracles and generators, and a failure stops the test"
)]

use dashu_ratio::RBig;
use geom_curve::{
    Basis, Composite, HelixAxes, Linework, LineworkError, ParamCurve, Segment, linework,
};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_nurbs::{Limit, Parametrization};
use geom_run::{Budget, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn xyz(q: P3) -> [f64; 3] {
    [q.x.get() as f64, q.y.get() as f64, q.z.get() as f64]
}

/// The name of each piece's kind, in order.
fn kinds(l: &Linework) -> Vec<&'static str> {
    l.curve()
        .segments()
        .iter()
        .map(|s| match s.basis() {
            Basis::Line(_) => "line",
            Basis::Circle(_) => "level arc",
            Basis::Helix(_) => "helix",
            Basis::Nurbs(_) => "nurbs",
            _ => "other",
        })
        .collect()
}

/// Every point lies within the bound (and rounding) of the curve evaluated at its parameter in
/// doubles, from the side of the piece it starts or ends; the parameters increase.
fn check_through(points: &[P3], l: &Linework) {
    assert_eq!(l.parameters().len(), points.len());
    assert!(l.bound() <= 64.0, "bound {}", l.bound());
    assert!(l.parameters().windows(2).all(|w| w[0] < w[1]));
    let c: &Composite<3> = l.curve();
    let reach = points
        .iter()
        .map(|q| xyz(*q).iter().map(|v| v.abs()).fold(1.0, f64::max))
        .fold(1.0, f64::max);
    for (i, (&q, &s)) in points.iter().zip(l.parameters()).enumerate() {
        let sides: &[Limit] = if i == 0 {
            &[Limit::FromRight]
        } else if i + 1 == points.len() {
            &[Limit::FromLeft]
        } else {
            &[Limit::FromLeft, Limit::FromRight]
        };
        for &side in sides {
            let at = c.point(s, side).unwrap();
            let d = dist(at, xyz(q));
            // The evaluation in doubles rounds: a few units in the last place of the coordinates.
            assert!(
                d <= l.bound() + 1e-13 * reach,
                "point {i} at {s} from {side:?}: {d} past the bound {}",
                l.bound()
            );
        }
    }
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn q(v: i64) -> RBig {
    RBig::from(v)
}

/// The circumcenter in plan of three grid points, exactly.
fn exact_center(a: P3, m: P3, b: P3) -> (RBig, RBig) {
    let (ax, ay) = (q(a.x.get()), q(a.y.get()));
    let (bx, by) = (q(m.x.get()) - &ax, q(m.y.get()) - &ay);
    let (cx, cy) = (q(b.x.get()) - &ax, q(b.y.get()) - &ay);
    let b2 = &bx * &bx + &by * &by;
    let c2 = &cx * &cx + &cy * &cy;
    let d = (&bx * &cy - &by * &cx) * RBig::from(2);
    let ux = (&cy * &b2 - &by * &c2) / &d;
    let uy = (&bx * &c2 - &cx * &b2) / &d;
    (ax + ux, ay + uy)
}

fn f(r: &RBig) -> f64 {
    r.to_f64().value()
}

/// Each arc piece's stored center and radius against the exact circumcenter and radius of `a`,
/// `m` and `b`.
fn check_circle(l: &Linework, a: P3, m: P3, b: P3) {
    let (cx, cy) = exact_center(a, m, b);
    let r2 = {
        let dx = q(a.x.get()) - &cx;
        let dy = q(a.y.get()) - &cy;
        &dx * &dx + &dy * &dy
    };
    let radius = f(&r2).sqrt();
    let (cx, cy) = (f(&cx), f(&cy));
    for s in l.curve().segments() {
        let (center, r) = match s.basis() {
            Basis::Helix(h) => match h.axes() {
                HelixAxes::Frame { radius, .. } => (h.center(), radius),
                HelixAxes::Vectors { .. } => panic!("a survey helix is stored on its frame"),
            },
            Basis::Circle(c) => (c.center(), c.radius()),
            _ => continue,
        };
        let scale = radius.max(cx.abs()).max(cy.abs());
        assert!((center[0] - cx).abs() <= 1e-14 * scale, "{center:?} {cx}");
        assert!((center[1] - cy).abs() <= 1e-14 * scale, "{center:?} {cy}");
        assert!((r - radius).abs() <= 1e-13 * scale, "{r} {radius}");
    }
}

/// A kerb return of radius 5k about (x0, y0): three points on the circle, equal chords, turning
/// left.
fn kerb(k: i64, z: [i64; 3]) -> [P3; 3] {
    let (x0, y0) = (1_234_567_000, -987_654_000);
    [
        p(x0 + 4 * k, y0 - 3 * k, z[0]),
        p(x0 + 5 * k, y0, z[1]),
        p(x0 + 4 * k, y0 + 3 * k, z[2]),
    ]
}

#[test]
fn lines_alone_are_exact() {
    let pts = [
        p(0, 0, 0),
        p(64_000, 0, 6_400),
        p(64_000, 128_000, 6_400),
        p(0, 128_000, -12_800),
        p(0, 128_000, 0),
    ];
    let l = linework(&pts, &[Segment::Line; 4], &mut meter()).unwrap();
    assert_eq!(kinds(&l), ["line"; 4]);
    assert_eq!(l.bound(), 0.0);
    assert_eq!(l.exactness(GridStep::Q0), Exactness::Exact);
    assert_eq!(l.parameters(), [0.0, 1.0, 2.0, 3.0, 4.0]);
    assert!(l.curve().gaps().iter().all(|&g| g == 0.0));
    check_through(&pts, &l);
    // A vertical line, one plan position at two heights, is a line like any other.
    let l = linework(&pts[3..], &[Segment::Line], &mut meter()).unwrap();
    assert_eq!(l.bound(), 0.0);
}

#[test]
fn a_level_arc_is_one_circle() {
    let pts = kerb(6_400_000, [100_000; 3]);
    let l = linework(&pts, &[Segment::Arc], &mut meter()).unwrap();
    assert_eq!(kinds(&l), ["level arc"]);
    check_circle(&l, pts[0], pts[1], pts[2]);
    check_through(&pts, &l);
    assert!(matches!(
        l.exactness(GridStep::Q0),
        Exactness::Certified {
            bound: Steps(1),
            ..
        }
    ));
}

#[test]
fn an_arc_on_one_grade_is_one_helix() {
    // Equal plan chords and equal rises.
    let pts = kerb(6_400_000, [100_000, 164_000, 228_000]);
    let l = linework(&pts, &[Segment::Arc], &mut meter()).unwrap();
    assert_eq!(kinds(&l), ["helix"]);
    check_circle(&l, pts[0], pts[1], pts[2]);
    check_through(&pts, &l);
    // The middle point sits halfway along the helix's turn.
    let s = l.parameters();
    assert!((s[1] - f64::midpoint(s[0], s[2])).abs() <= 1e-15, "{s:?}");
}

#[test]
fn an_arc_off_one_grade_is_two_helices_meeting_at_its_middle_point() {
    for z in [
        [100_000, 164_000, 260_000],
        [100_000, 164_000, 100_000],
        [100_000, 100_000, 228_000],
        [100_000, 36_000, 228_000],
    ] {
        let pts = kerb(6_400_000, z);
        let l = linework(&pts, &[Segment::Arc], &mut meter()).unwrap();
        let expected: Vec<&str> = [(z[0], z[1]), (z[1], z[2])]
            .iter()
            .map(|(a, b)| if a == b { "level arc" } else { "helix" })
            .collect();
        assert_eq!(kinds(&l), expected, "{z:?}");
        check_circle(&l, pts[0], pts[1], pts[2]);
        check_through(&pts, &l);
        // Each piece's grade is its own rise over its own turn.
        for (seg, (a, b)) in l.curve().segments().iter().zip([(0, 1), (1, 2)]) {
            if let Basis::Helix(h) = seg.basis() {
                let HelixAxes::Frame { pitch, frame, .. } = h.axes() else {
                    panic!()
                };
                let turn = seg.interval().1 - seg.interval().0;
                let rise = (pts[b].z.get() - pts[a].z.get()) as f64;
                let along = frame.normal()[2] * pitch / 2.0 * turn;
                assert!((along - rise).abs() <= 1e-9 * rise.abs(), "{along} {rise}");
            }
        }
    }
}

#[test]
fn an_unequal_arc_keeps_every_height_turning_either_way() {
    // Unequal chords on the circle of radius 5k: the grades differ in general, so two pieces.
    let k = 1_280_000;
    let (x0, y0) = (-55_000_000, 77_000_000);
    let left = [
        p(x0 + 5 * k, y0, 0),
        p(x0 + 3 * k, y0 + 4 * k, 6_400),
        p(x0 - 4 * k, y0 + 3 * k, 25_600),
    ];
    for pts in [left, [left[2], left[1], left[0]]] {
        let l = linework(&pts, &[Segment::Arc], &mut meter()).unwrap();
        assert_eq!(kinds(&l), ["helix", "helix"]);
        check_circle(&l, pts[0], pts[1], pts[2]);
        check_through(&pts, &l);
    }
    // A major arc: more than half a turn.
    let major = [
        p(x0 + 5 * k, y0, 0),
        p(x0 - 5 * k, y0, 6_400),
        p(x0 + 3 * k, y0 - 4 * k, 12_800),
    ];
    let l = linework(&major, &[Segment::Arc], &mut meter()).unwrap();
    check_through(&major, &l);
    let turned: f64 = l
        .curve()
        .segments()
        .iter()
        .map(|s| s.interval().1 - s.interval().0)
        .sum();
    // From 0 through π to −0.295π (the point (3, −4)), anticlockwise: 1.705 half-turns.
    let expected = 2.0 - (4.0_f64).atan2(3.0) / core::f64::consts::PI;
    assert!((turned - expected).abs() <= 1e-12, "{turned} {expected}");
}

#[test]
fn a_smooth_figure_interpolates_its_points() {
    let pts: Vec<P3> = (0..9)
        .map(|i| {
            let x = i * 3_200_000;
            let y = (i * i) * 150_000 - i * 900_000;
            let z = 640_000 + i * 12_345;
            p(x, y, z)
        })
        .collect();
    for rule in [
        Parametrization::ChordLength,
        Parametrization::Centripetal,
        Parametrization::Uniform,
    ] {
        let l = linework(&pts, &[Segment::Smooth { points: 8, rule }], &mut meter()).unwrap();
        assert_eq!(kinds(&l), ["nurbs"]);
        let Basis::Nurbs(c) = l.curve().segments()[0].basis() else {
            panic!()
        };
        assert_eq!(c.degree(), 3);
        check_through(&pts, &l);
        assert!(l.bound() > 0.0 && l.bound() < 1.0, "{}", l.bound());
    }
    // Through three points, the figure is of degree two.
    let l = linework(
        &pts[..3],
        &[Segment::Smooth {
            points: 2,
            rule: Parametrization::ChordLength,
        }],
        &mut meter(),
    )
    .unwrap();
    let Basis::Nurbs(c) = l.curve().segments()[0].basis() else {
        panic!()
    };
    assert_eq!(c.degree(), 2);
    check_through(&pts[..3], &l);
}

#[test]
fn a_figure_of_every_kind_is_one_curve() {
    let k = 1_280_000;
    let pts = [
        p(0, 0, 0),
        p(10 * k, 0, 6_400),
        // An arc turning left about (10k, 5k), radius 5k, rising.
        p(15 * k, 5 * k, 12_800),
        p(10 * k, 10 * k, 19_200),
        // A smooth figure on.
        p(6 * k, 11 * k, 20_000),
        p(2 * k, 13 * k, 21_000),
        p(-k, 17 * k, 21_500),
        // And a line to finish.
        p(-k, 25 * k, 21_500),
    ];
    let segments = [
        Segment::Line,
        Segment::Arc,
        Segment::Smooth {
            points: 3,
            rule: Parametrization::Centripetal,
        },
        Segment::Line,
    ];
    let l = linework(&pts, &segments, &mut meter()).unwrap();
    assert_eq!(kinds(&l), ["line", "helix", "nurbs", "line"]);
    check_through(&pts, &l);
    let Exactness::Certified { bound, step } = l.exactness(GridStep::Q0) else {
        panic!()
    };
    assert_eq!(step, GridStep::Q0);
    assert!(bound.0 >= 1 && bound.0 <= 64);
    assert!((bound.0 as f64) >= l.bound() && ((bound.0 - 1) as f64) < l.bound());
    // The joints are certified, each within twice the bound.
    assert_eq!(l.curve().gaps().len(), 3);
    assert!(l.curve().gaps().iter().all(|&g| g <= 2.0 * l.bound()));
    // The arc's center is the exact circumcenter.
    check_circle(&l, pts[1], pts[2], pts[3]);
}

#[test]
fn what_cannot_be_linework_is_refused_by_name() {
    let pts = [
        p(0, 0, 0),
        p(64_000, 0, 0),
        p(128_000, 0, 0),
        p(128_000, 64_000, 0),
    ];
    let m = &mut meter();
    assert_eq!(
        linework(&pts, &[], m),
        Err(LineworkError::Count {
            points: 4,
            needed: 1
        })
    );
    assert_eq!(
        linework(&pts, &[Segment::Line, Segment::Line], m),
        Err(LineworkError::Count {
            points: 4,
            needed: 3
        })
    );
    assert_eq!(
        linework(&pts[..1], &[Segment::Line], m),
        Err(LineworkError::Count {
            points: 1,
            needed: 2
        })
    );
    assert_eq!(
        linework(&pts, &[Segment::Arc, Segment::Line], m),
        Err(LineworkError::Collinear { segment: 0 })
    );
    // Two points at one plan position: no circle in plan through them.
    let stacked = [p(0, 0, 0), p(0, 0, 6_400), p(64_000, 0, 0)];
    assert_eq!(
        linework(&stacked, &[Segment::Arc], m),
        Err(LineworkError::Collinear { segment: 0 })
    );
    let repeated = [p(0, 0, 0), p(64_000, 0, 0), p(64_000, 0, 0)];
    assert_eq!(
        linework(&repeated, &[Segment::Line, Segment::Line], m),
        Err(LineworkError::Repeated { index: 2 })
    );
    assert_eq!(
        linework(
            &pts[..2],
            &[Segment::Smooth {
                points: 1,
                rule: Parametrization::ChordLength
            }],
            m
        ),
        Err(LineworkError::Short { segment: 0 })
    );
    for e in [
        LineworkError::Count {
            points: 4,
            needed: 3,
        },
        LineworkError::Short { segment: 2 },
        LineworkError::Repeated { index: 5 },
        LineworkError::Collinear { segment: 1 },
        LineworkError::OverFit {
            bound: geom_num::Real::new(70.5).unwrap(),
        },
    ] {
        assert_ne!(e.to_string(), "");
    }
}

#[test]
fn a_stopped_meter_stops_linework() {
    let pts = kerb(6_400_000, [0, 6_400, 12_800]);
    let mut m = WorkMeter::new(Budget::of_work(0));
    assert!(matches!(
        linework(&pts, &[Segment::Arc], &mut m),
        Err(LineworkError::Curve(geom_curve::CurveError::Stopped(_)))
    ));
}

#[test]
fn the_same_figure_gives_the_same_bits() {
    let pts = kerb(3_333_333, [1, 77_777, 99_999]);
    let a = linework(&pts, &[Segment::Arc], &mut meter()).unwrap();
    let b = linework(&pts, &[Segment::Arc], &mut meter()).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.bound().to_bits(), b.bound().to_bits());
}

/// Seeded arcs: three points rounded to the grid from a random circle, at random heights, in every
/// quadrant and size from 2^12 to 2^34 steps of radius, turning either way.
#[test]
fn seeded_arcs_pass_through_their_points() {
    let seed = resolve("geom-curve linework arcs", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut unit = || (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
    let rounds = if cfg!(debug_assertions) { 60 } else { 600 };
    let mut checked = 0;
    for round in 0..rounds {
        let r = 2.0_f64.powf(12.0 + 22.0 * unit());
        let cx = (unit() * 2.0 - 1.0) * 2.0_f64.powi(38);
        let cy = (unit() * 2.0 - 1.0) * 2.0_f64.powi(38);
        let start = unit() * 2.0 * core::f64::consts::PI;
        let spread = 0.05 + 1.85 * unit();
        let split = 0.1 + 0.8 * unit();
        let angles = [
            start,
            start + spread * split * core::f64::consts::PI,
            start + spread * core::f64::consts::PI,
        ];
        let mut pts: Vec<P3> = angles
            .iter()
            .map(|t| {
                let z = ((unit() * 2.0 - 1.0) * 2.0_f64.powi(30)).round() as i64;
                p(
                    (cx + r * t.cos()).round() as i64,
                    (cy + r * t.sin()).round() as i64,
                    z,
                )
            })
            .collect();
        if unit() < 0.5 {
            pts.reverse();
        }
        match linework(&pts, &[Segment::Arc], &mut meter()) {
            Ok(l) => {
                check_circle(&l, pts[0], pts[1], pts[2]);
                check_through(&pts, &l);
                checked += 1;
            }
            Err(LineworkError::Collinear { .. }) => {}
            Err(e) => panic!("round {round}, seed {seed}: {e} for {pts:?}"),
        }
    }
    assert!(checked * 10 >= rounds * 9, "{checked} of {rounds}");
}

#[test]
fn segments_say_how_many_points_they_use() {
    assert_eq!(Segment::Line.uses(), 1);
    assert_eq!(Segment::Arc.uses(), 2);
    assert_eq!(
        Segment::Smooth {
            points: 7,
            rule: Parametrization::Uniform
        }
        .uses(),
        7
    );
}
