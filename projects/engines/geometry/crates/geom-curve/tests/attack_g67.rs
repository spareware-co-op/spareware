//! Attacks on piece G67's claims in `geom-curve`: closed-form extents and derivative bounds for
//! every analytic kind, and Fresnel and Gauss–Legendre enclosures with remainder bounds for the
//! transitions.
//!
//! - **Containment on a million seeded inputs per kind** (in release builds; a fiftieth in debug
//!   builds, where overflow checks are on): for each kind, random curves, random spans and random
//!   stations in them; the certified enclosure of the point and its first two derivatives at each
//!   station must meet the bound and the two derivative bounds over the span, which they cannot
//!   if a bound misses the exact value by more than an enclosure's width. Every transition law
//!   the same way, with `Transition::bound` and `Transition::derivative_bound`.
//! - **Tightness against dense samples**, the oracle of a closed form: the box's ends lie within a
//!   small slack of the extremes of 4,001 evenly spaced samples (an interval extension over the
//!   span would miss by a few per cent).
//! - **Fresnel against a 256-bit truth**: the fixed-point integral of e^{iπt²/2} by Taylor series
//!   on short pieces (`series`), not the kernel's series, expansion or quadrature; and the
//!   clothoid's Fresnel route against the same truth for its heading, and against the quadrature.
//! - **Edges**: spans ending on a critical point, spans of one station, helices of a thousand
//!   turns, a helix past 2⁵⁰, a lift at the edge of monotone, coefficients whose enclosures hold
//!   zero or end at it, tiny and huge Fresnel arguments (a second truth past 1000: DLMF's
//!   expansion in 256-bit fixed point with the angle reduced exactly), and refusals.
//! - **Routes, by their work**: every closed form costs one unit of work, and every undecided
//!   case takes the pieces' route, which costs more, so a closed form that falls back, or a
//!   fallback that is skipped, shows in a meter's budget.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::needless_range_loop,
    clippy::disallowed_methods,
    dead_code,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod series;

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_curve::extent::{Scalar, extent};
use geom_curve::transition::fresnel::fresnel;
use geom_curve::{
    Axes, Circle, CurvatureLaw, CurveError, DerivativeBound, Ellipse, Frame, Helix, HelixAxes,
    Hyperbola, Involute, Line, Parabola, ParamCurve, Transition,
};
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Limit};
use geom_run::{Budget, WorkMeter};
use series::{F, Truth, abs, cos_sin, div, frac, fx, mul, one, pi};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// `SplitMix64`: a fixed, seeded stream.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [lo, hi).
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * ((self.next() >> 11) as f64 / (1u64 << 53) as f64)
    }

    fn pick(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn vector<const D: usize>(&mut self, size: f64) -> [f64; D] {
        std::array::from_fn(|_| self.range(-size, size))
    }
}

/// The work a test does: all of it in release builds, a fiftieth in debug builds and under Miri.
fn scale(n: usize) -> usize {
    if cfg!(miri) {
        (n / 5000).max(1)
    } else if cfg!(debug_assertions) {
        (n / 50).max(1)
    } else {
        n
    }
}

/// Whether an enclosure meets a box.
fn meets<const D: usize>(e: &[Interval; D], b: &Bounds<D>) -> bool {
    e.iter()
        .zip(b.lo.iter().zip(&b.hi))
        .all(|(x, (&lo, &hi))| x.lo() <= hi && lo <= x.hi())
}

fn bounded<const D: usize>(d: DerivativeBound<D>) -> Bounds<D> {
    match d {
        DerivativeBound::Bounded(b) => b,
        DerivativeBound::Unbounded => panic!("an analytic kind's derivative is bounded"),
    }
}

/// A random span of the domain: the whole domain, one station, or two random stations.
fn span(rng: &mut Rng, (a, b): (f64, f64)) -> (f64, f64) {
    match rng.pick(16) {
        0 => (a, b),
        1 => {
            let t = rng.range(a, b);
            (t, t)
        }
        _ => {
            let (s, t) = (rng.range(a, b), rng.range(a, b));
            (s.min(t), s.max(t))
        }
    }
}

/// A random station of a span, its ends included.
fn station(rng: &mut Rng, (lo, hi): (f64, f64)) -> f64 {
    match rng.pick(32) {
        0 => lo,
        1 => hi,
        _ => rng.range(lo, hi).clamp(lo, hi),
    }
}

/// `stations` checks on each of `spans` spans of a kind: the point and two derivatives at a
/// station meet the bound and derivative bounds over the span.
fn contain<K: ParamCurve<D>, const D: usize>(
    name: &str,
    kind: &K,
    spans: usize,
    stations: usize,
    rng: &mut Rng,
) -> usize {
    let m = &mut meter();
    let mut checked = 0;
    for _ in 0..spans {
        let s = span(rng, kind.domain());
        let b0 = kind.bound(s, m).unwrap();
        let b1 = bounded(kind.derivative_bound(s, 1, m).unwrap());
        let b2 = bounded(kind.derivative_bound(s, 2, m).unwrap());
        for _ in 0..stations {
            let t = station(rng, s);
            let e = kind.enclose(t, Limit::FromRight, 2, m).unwrap();
            assert!(
                meets(&e[0], &b0) && meets(&e[1], &b1) && meets(&e[2], &b2),
                "{name}: station {t} outside the bounds over {s:?}: {e:?} {b0:?} {b1:?} {b2:?}"
            );
            checked += 1;
        }
    }
    checked
}

/// A frame of random direction in 3D.
fn frame3(rng: &mut Rng) -> Frame<3> {
    loop {
        if let Ok(f) = Frame::new(rng.vector(1.0), rng.vector(1.0)) {
            return f;
        }
    }
}

/// A random domain of at most one turn.
fn turn(rng: &mut Rng) -> (f64, f64) {
    let a = rng.range(-2.0, 2.0);
    (a, a + rng.range(0.01, 2.0))
}

/// A random analytic curve of kind `k` (0 line, 1 circle, 2 ellipse, 3 hyperbola, 4 parabola,
/// 5 helix, 6 involute), checked on `spans` × `stations`.
fn attack_kind(k: usize, spans: usize, stations: usize, rng: &mut Rng) -> usize {
    let c: [f64; 3] = rng.vector(100.0);
    match k {
        0 => {
            let a = rng.range(-50.0, 50.0);
            let line = Line::new(c, rng.vector(10.0), (a, a + rng.range(0.1, 100.0))).unwrap();
            contain("line", &line, spans, stations, rng)
        }
        1 => {
            let circle = Circle::new(c, rng.range(0.01, 50.0), frame3(rng), turn(rng)).unwrap();
            contain("circle", &circle, spans, stations, rng)
        }
        2 => {
            let axes = Axes::Vectors {
                u: rng.vector(20.0),
                v: rng.vector(20.0),
            };
            let Ok(e) = Ellipse::new(c, axes, turn(rng)) else {
                return 0;
            };
            contain("ellipse", &e, spans, stations, rng)
        }
        3 => {
            let axes = Axes::Vectors {
                u: rng.vector(5.0),
                v: rng.vector(5.0),
            };
            let a = rng.range(-3.0, 2.0);
            let Ok(h) = Hyperbola::new(c, axes, (a, a + rng.range(0.01, 3.0))) else {
                return 0;
            };
            contain("hyperbola", &h, spans, stations, rng)
        }
        4 => {
            let axes = Axes::Vectors {
                u: rng.vector(3.0),
                v: rng.vector(10.0),
            };
            let a = rng.range(-10.0, 5.0);
            let Ok(p) = Parabola::new(c, axes, (a, a + rng.range(0.01, 10.0))) else {
                return 0;
            };
            contain("parabola", &p, spans, stations, rng)
        }
        5 => {
            // The lift per half-turn, from far below to far above π times the radius, so both the
            // monotone coordinates and the critical ones are met.
            let lift = 10f64.powf(rng.range(-3.0, 2.0));
            let axes = HelixAxes::Vectors {
                u: rng.vector(5.0),
                v: rng.vector(5.0),
                w: rng.vector(lift),
            };
            let a = rng.range(-30.0, 30.0);
            let Ok(h) = Helix::new(c, axes, (a, a + rng.range(0.01, 40.0))) else {
                return 0;
            };
            contain("helix", &h, spans, stations, rng)
        }
        _ => {
            let axes = Axes::Vectors {
                u: rng.vector(5.0),
                v: rng.vector(5.0),
            };
            let a = rng.range(-3.0, 4.0);
            let Ok(i) = Involute::with_axes(c, axes, (a, a + rng.range(0.01, 4.0))) else {
                return 0;
            };
            contain("involute", &i, spans, stations, rng)
        }
    }
}

const KINDS: [&str; 7] = [
    "line",
    "circle",
    "ellipse",
    "hyperbola",
    "parabola",
    "helix",
    "involute",
];

/// Claim: every analytic kind's bound and first two derivative bounds over any span hold the
/// curve at every station of it: a million seeded stations per kind.
#[test]
fn every_analytic_kind_holds_a_million_seeded_stations() {
    for (k, name) in KINDS.iter().enumerate() {
        let mut rng = Rng(0x6700 + k as u64);
        let (curves, spans, stations) = (scale(100), 10, 1000);
        let mut checked = 0;
        for _ in 0..curves {
            checked += attack_kind(k, spans, stations, &mut rng);
        }
        assert!(
            checked * 10 >= curves * spans * stations * 9,
            "{name}: {checked}"
        );
        println!("{name}: {checked} stations held");
    }
}

/// The extremes of `samples` evenly spaced values of each coordinate of the derivative of
/// `order`, in doubles.
fn sampled<K: ParamCurve<D>, const D: usize>(
    kind: &K,
    (lo, hi): (f64, f64),
    order: usize,
    samples: usize,
) -> ([f64; D], [f64; D]) {
    let (mut min, mut max) = ([f64::INFINITY; D], [f64::NEG_INFINITY; D]);
    let mut out = vec![[0.0; D]; order + 1];
    for i in 0..=samples {
        let t = if i == samples {
            hi
        } else {
            lo + (hi - lo) * (i as f64 / samples as f64)
        };
        kind.derivatives(t, Limit::FromRight, &mut out).unwrap();
        for d in 0..D {
            min[d] = min[d].min(out[order][d]);
            max[d] = max[d].max(out[order][d]);
        }
    }
    (min, max)
}

/// The box of order `order` lies within `slack` (relative to the samples' size) of the samples'
/// extremes, and holds them.
fn tight<K: ParamCurve<D>, const D: usize>(
    name: &str,
    kind: &K,
    s: (f64, f64),
    order: usize,
    slack: f64,
) {
    let m = &mut meter();
    let b = if order == 0 {
        kind.bound(s, m).unwrap()
    } else {
        bounded(kind.derivative_bound(s, order, m).unwrap())
    };
    let (min, max) = sampled(kind, s, order, 4000);
    for d in 0..D {
        let size = 1.0 + max[d].abs().max(min[d].abs());
        let rounding = 1e-12 * size;
        assert!(
            b.lo[d] <= min[d] + rounding && max[d] - rounding <= b.hi[d],
            "{name} order {order}: the box {b:?} misses samples [{min:?}, {max:?}]"
        );
        assert!(
            min[d] - b.lo[d] <= slack * size && b.hi[d] - max[d] <= slack * size,
            "{name} order {order} coordinate {d}: the box {b:?} is {} and {} wider than samples \
             [{min:?}, {max:?}]",
            min[d] - b.lo[d],
            b.hi[d] - max[d]
        );
    }
}

/// Claim: the boxes are the kinds' own extents: within a sampling error of dense samples' extremes
/// (closed forms), or a little more for the involute's derivatives (the pieces' route).
#[test]
fn bounds_are_the_kinds_own_extents() {
    let mut rng = Rng(0x6767);
    for round in 0..scale(400).max(8) {
        let c: [f64; 3] = rng.vector(10.0);
        let k = round % 7;
        let check = |kind: &dyn Fn(usize, f64)| {
            for order in 0..=2 {
                kind(order, 1e-5);
            }
        };
        match k {
            0 => {
                let l = Line::new(c, rng.vector(3.0), (-2.0, 3.0)).unwrap();
                let s = span(&mut rng, l.domain());
                check(&|o, sl| tight("line", &l, s, o, sl));
            }
            1 => {
                let l =
                    Circle::new(c, rng.range(0.5, 5.0), frame3(&mut rng), turn(&mut rng)).unwrap();
                let s = span(&mut rng, l.domain());
                check(&|o, sl| tight("circle", &l, s, o, sl));
            }
            2 => {
                let axes = Axes::Vectors {
                    u: rng.vector(5.0),
                    v: rng.vector(5.0),
                };
                let l = Ellipse::new(c, axes, turn(&mut rng)).unwrap();
                let s = span(&mut rng, l.domain());
                check(&|o, sl| tight("ellipse", &l, s, o, sl));
            }
            3 => {
                let axes = Axes::Vectors {
                    u: rng.vector(2.0),
                    v: rng.vector(2.0),
                };
                let l = Hyperbola::new(c, axes, (-2.0, 2.0)).unwrap();
                let s = span(&mut rng, l.domain());
                check(&|o, sl| tight("hyperbola", &l, s, o, sl));
            }
            4 => {
                let axes = Axes::Vectors {
                    u: rng.vector(2.0),
                    v: rng.vector(4.0),
                };
                let l = Parabola::new(c, axes, (-3.0, 3.0)).unwrap();
                let s = span(&mut rng, l.domain());
                check(&|o, sl| tight("parabola", &l, s, o, sl));
            }
            5 => {
                let lift = 10f64.powf(rng.range(-2.0, 1.0));
                let axes = HelixAxes::Vectors {
                    u: rng.vector(2.0),
                    v: rng.vector(2.0),
                    w: rng.vector(lift),
                };
                let l = Helix::new(c, axes, (-3.0, 5.0)).unwrap();
                let s = span(&mut rng, l.domain());
                check(&|o, sl| tight("helix", &l, s, o, sl));
            }
            _ => {
                let axes = Axes::Vectors {
                    u: rng.vector(2.0),
                    v: rng.vector(2.0),
                };
                let l = Involute::with_axes(c, axes, (-1.0, 3.0)).unwrap();
                let s = span(&mut rng, l.domain());
                tight("involute", &l, s, 0, 1e-5);
                // The derivatives have no closed-form critical points: monotone pieces.
                tight("involute", &l, s, 1, 1e-2);
                tight("involute", &l, s, 2, 1e-2);
            }
        }
    }
}

/// Claim: a circle on a rotated frame is bounded by its own extent, ±r about its centre in each
/// axis over a whole turn, to a few rounding steps: no interval extension's overshoot.
#[test]
fn a_whole_circle_on_a_rotated_frame_is_bounded_by_its_radius() {
    let m = &mut meter();
    let frame = Frame::new([3.0, 4.0, 12.0], [4.0, -3.0, 0.0]).unwrap();
    let c = Circle::new([1.0, 2.0, 3.0], 2.0, frame, (0.25, 2.25)).unwrap();
    let b = c.bound((0.25, 2.25), m).unwrap();
    // The extent along axis i is the centre ± r·√(x̂ᵢ² + ŷᵢ²), with x̂ = (3, 4, 12)/13 and
    // ŷ = n̂ × x̂ for n̂ = (4, −3, 0)/5.
    let x: [f64; 3] = [3.0 / 13.0, 4.0 / 13.0, 12.0 / 13.0];
    let n: [f64; 3] = [0.8, -0.6, 0.0];
    let y = [
        n[1] * x[2] - n[2] * x[1],
        n[2] * x[0] - n[0] * x[2],
        n[0] * x[1] - n[1] * x[0],
    ];
    for i in 0..3 {
        let r = 2.0 * (x[i] * x[i] + y[i] * y[i]).sqrt();
        let c = [1.0, 2.0, 3.0][i];
        assert!((b.lo[i] - (c - r)).abs() < 1e-14, "{i}: {b:?}");
        assert!((b.hi[i] - (c + r)).abs() < 1e-14, "{i}: {b:?}");
    }
    // A quarter turn from x̂ to ŷ: each coordinate's extent from its ends and its critical points.
    let q = c.bound((0.25, 0.75), m).unwrap();
    let (min, max) = sampled(&c, (0.25, 0.75), 0, 100_000);
    for i in 0..3 {
        assert!(q.lo[i] <= min[i] && max[i] <= q.hi[i]);
        // 10⁵ samples come within R·(πh)²/2 ≈ 1.5·10⁻¹⁰ of an extreme between them.
        assert!(
            min[i] - q.lo[i] < 1e-9 && q.hi[i] - max[i] < 1e-9,
            "{i}: {q:?}"
        );
    }
}

/// Claim: a span that ends on a critical point, or holds one station, is bounded exactly there.
#[test]
fn spans_ending_on_critical_points_and_single_stations_are_exact() {
    let m = &mut meter();
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let c = Circle::new([0.0; 3], 1.0, f, (0.0, 2.0)).unwrap();
    // cos πt peaks at 0, sin πt at ½: both ends exact.
    let b = c.bound((0.0, 0.5), m).unwrap();
    assert_eq!((b.lo[0], b.hi[0]), (0.0, 1.0));
    assert_eq!(b.lo[1], 0.0);
    assert!(b.hi[1] >= 1.0 && b.hi[1] - 1.0 < 1e-15);
    assert_eq!((b.lo[2], b.hi[2]), (0.0, 0.0));
    // Over a whole turn the box is the unit square.
    let w = c.bound((0.0, 2.0), m).unwrap();
    for i in 0..2 {
        assert!(w.lo[i] <= -1.0 && w.lo[i] > -1.0 - 1e-15);
        assert!(w.hi[i] >= 1.0 && w.hi[i] < 1.0 + 1e-15);
    }
    // One station: the box is a few rounding steps about the point.
    for t in [0.0, 0.3, 1.0, 1.7, 2.0] {
        let b = c.bound((t, t), m).unwrap();
        let e = c.enclose(t, Limit::FromRight, 0, m).unwrap();
        assert!(meets(&e[0], &b), "{t}: {b:?} {e:?}");
        for i in 0..3 {
            assert!(b.hi[i] - b.lo[i] < 1e-15, "{t}: {b:?}");
        }
    }
}

/// Claim: a helix of a thousand turns is bounded by its radius across and its lift along, with a
/// few evaluations (two families of critical points, each at most four members).
#[test]
fn a_helix_of_a_thousand_turns_is_bounded_by_its_radius_and_lift() {
    let m = &mut meter();
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let axes = HelixAxes::Frame {
        frame: f,
        radius: 3.0,
        pitch: 0.5,
    };
    let h = Helix::new([0.0; 3], axes, (0.0, 2000.0)).unwrap();
    let b = h.bound((0.0, 2000.0), m).unwrap();
    for i in 0..2 {
        assert!(b.lo[i] <= -3.0 && b.lo[i] > -3.0 - 1e-12, "{b:?}");
        assert!(b.hi[i] >= 3.0 && b.hi[i] < 3.0 + 1e-12, "{b:?}");
    }
    assert_eq!((b.lo[2], b.hi[2]), (0.0, 500.0));
    // Inside, between two stations far apart.
    let s = (17.3, 1789.6);
    let b = h.bound(s, m).unwrap();
    let (min, max) = sampled(&h, s, 0, 400_000);
    for i in 0..3 {
        assert!(b.lo[i] <= min[i] && max[i] <= b.hi[i]);
        assert!(
            min[i] - b.lo[i] < 1e-9 && b.hi[i] - max[i] < 1e-9,
            "{i}: {b:?} {min:?} {max:?}"
        );
    }
}

/// Claim: a helix whose lift is near π times its radius, where the closed form's critical points
/// are undecided, and one past 2⁵⁰, where a period is too fine to place them, are bounded by the
/// pieces' route, and still hold every station.
#[test]
fn undecided_helices_take_the_pieces_route_and_hold() {
    let mut rng = Rng(0x6750);
    // u = (1, 0, 0), v = (0, 1, 0) and a lift along x of π·1 to within a rounding: the x
    // coordinate cos πt + w·t has g′ = π·(w/π − sin πt), touching zero.
    let w = std::f64::consts::PI;
    let axes = HelixAxes::Vectors {
        u: [1.0, 0.0, 0.0],
        v: [0.0, 1.0, 0.0],
        w: [w, 0.0, 1.0],
    };
    let h = Helix::new([0.0; 3], axes, (-4.0, 4.0)).unwrap();
    assert!(contain("lift π", &h, scale(200), 100, &mut rng) > 0);
    tight("lift π", &h, (-4.0, 4.0), 0, 1e-3);
    let far = 1_125_899_906_842_624.0 * 1.5;
    let axes = HelixAxes::Vectors {
        u: [2.0, 0.0, 0.0],
        v: [0.0, 2.0, 0.0],
        w: [0.0, 0.0, 1.0],
    };
    let h = Helix::new([0.0; 3], axes, (far, far + 3.0)).unwrap();
    assert!(contain("far", &h, scale(200), 100, &mut rng) > 0);
}

/// Claim: the hyperbola's and parabola's single critical points are found, in and out of the span.
#[test]
fn single_critical_points_are_found() {
    let m = &mut meter();
    // x = cosh t + ½·sinh t has its least value √(1 − ¼) at tanh t = −½.
    let axes = Axes::Vectors {
        u: [1.0, 0.0],
        v: [0.5, 1.0],
    };
    let h = Hyperbola::new([0.0, 0.0], axes, (-2.0, 2.0)).unwrap();
    let b = h.bound((-2.0, 2.0), m).unwrap();
    let least = 0.75f64.sqrt();
    assert!(b.lo[0] <= least && least - b.lo[0] < 1e-14, "{b:?}");
    // Past the critical point: the ends alone.
    let b = h.bound((0.0, 2.0), m).unwrap();
    assert_eq!(b.lo[0], 1.0);
    // y = t² − 2t + 3 has its least value 2 at t = 1.
    let axes = Axes::Vectors {
        u: [0.0, 1.0],
        v: [1.0, -2.0],
    };
    let p = Parabola::new([0.0, 3.0], axes, (-5.0, 5.0)).unwrap();
    let b = p.bound((-1.0, 4.0), m).unwrap();
    assert_eq!((b.lo[1], b.hi[1]), (2.0, 11.0));
    assert_eq!((b.lo[0], b.hi[0]), (-1.0, 4.0));
}

/// Claim: `extent` holds every value of its scalar for every coefficient of its enclosures, also
/// where they hold zero and the closed form is undecided: sampled at 2,001 stations and at
/// coefficients drawn from each enclosure.
#[test]
fn extents_hold_with_coefficients_that_hold_zero() {
    let mut rng = Rng(0x6777);
    let m = &mut meter();
    for round in 0..scale(2000).max(40) {
        // A coefficient: a point, a small interval holding zero, or zero exactly.
        let coefficient = |rng: &mut Rng| {
            let x = rng.range(-3.0, 3.0);
            match rng.pick(4) {
                0 => Interval::point(0.0).unwrap(),
                1 => {
                    let w = rng.range(0.0, 1e-3);
                    Interval::new(-w, w * 0.5).unwrap()
                }
                2 => Interval::new(x, x + rng.range(0.0, 1e-2)).unwrap(),
                _ => Interval::point(x).unwrap(),
            }
        };
        let p = coefficient(&mut rng);
        let q = coefficient(&mut rng);
        let a = coefficient(&mut rng);
        let b = coefficient(&mut rng);
        let c = coefficient(&mut rng);
        let d = coefficient(&mut rng);
        let g = match round % 5 {
            0 => Scalar::Wave { p, q, a, b },
            1 => Scalar::Quadratic { p, q, r: a },
            2 => Scalar::Hyperbolic { p, a, b },
            3 => Scalar::Involute { p, a, b },
            _ => Scalar::Modulated { p, a, b, c, d },
        };
        let lo = rng.range(-3.0, 3.0);
        let s = (lo, lo + rng.range(0.0, 3.0));
        let e = extent(&g, s, m).unwrap();
        // Each coefficient at an end or the middle of its enclosure, as a point scalar.
        let ends = |x: Interval, k: u64| match k % 3 {
            0 => Interval::point(x.lo()).unwrap(),
            1 => Interval::point(x.hi()).unwrap(),
            _ => Interval::point(f64::midpoint(x.lo(), x.hi())).unwrap(),
        };
        for k in 0..9u64 {
            let h = match g {
                Scalar::Wave { p, q, a, b } => Scalar::Wave {
                    p: ends(p, k),
                    q: ends(q, k / 3),
                    a: ends(a, k + 1),
                    b: ends(b, k / 3 + 2),
                },
                Scalar::Quadratic { p, q, r } => Scalar::Quadratic {
                    p: ends(p, k),
                    q: ends(q, k / 3),
                    r: ends(r, k + 1),
                },
                Scalar::Hyperbolic { p, a, b } => Scalar::Hyperbolic {
                    p: ends(p, k),
                    a: ends(a, k / 3),
                    b: ends(b, k + 1),
                },
                Scalar::Involute { p, a, b } => Scalar::Involute {
                    p: ends(p, k),
                    a: ends(a, k / 3),
                    b: ends(b, k + 1),
                },
                Scalar::Modulated { p, a, b, c, d } => Scalar::Modulated {
                    p: ends(p, k),
                    a: ends(a, k / 3),
                    b: ends(b, k + 1),
                    c: ends(c, k / 3 + 1),
                    d: ends(d, k + 2),
                },
            };
            for i in 0..=200 {
                let t = s.0 + (s.1 - s.0) * (f64::from(i) / 200.0);
                let v = h.over(Interval::point(t.min(s.1)).unwrap()).unwrap();
                assert!(
                    e.lo() <= v.hi() && v.lo() <= e.hi(),
                    "{g:?} over {s:?}: {e:?} misses {v:?} at {t}"
                );
            }
        }
    }
    // Refusals: a span reversed, not finite.
    let g = Scalar::constant(Interval::point(1.0).unwrap()).unwrap();
    assert_eq!(extent(&g, (1.0, 0.0), m), Err(CurveError::SpanOutside));
    assert_eq!(extent(&g, (0.0, f64::NAN), m), Err(CurveError::SpanOutside));
    assert_eq!(
        extent(&g, (f64::NEG_INFINITY, 0.0), m),
        Err(CurveError::SpanOutside)
    );
}

// ---- Transitions -------------------------------------------------------------------------

/// A random law of kind `k` (decision 32.33's nine).
fn law(k: usize, rng: &mut Rng) -> CurvatureLaw {
    let kappa = |rng: &mut Rng| rng.range(-0.02, 0.02);
    let signed = |rng: &mut Rng, lo: f64, hi: f64| {
        let x = rng.range(lo, hi);
        if rng.pick(2) == 0 { x } else { -x }
    };
    match k {
        0 => CurvatureLaw::Clothoid {
            start: kappa(rng),
            end: kappa(rng),
        },
        1 => CurvatureLaw::Helmert {
            start: kappa(rng),
            end: kappa(rng),
        },
        2 => CurvatureLaw::Bloss {
            start: kappa(rng),
            end: kappa(rng),
        },
        3 => CurvatureLaw::Cosine {
            start: kappa(rng),
            end: kappa(rng),
        },
        4 => CurvatureLaw::Sine {
            start: kappa(rng),
            end: kappa(rng),
        },
        5 => CurvatureLaw::CantCoupled {
            start: kappa(rng),
            end: kappa(rng),
            height: rng.range(1.0, 2.5),
            cant: rng.range(-0.1, 0.1),
        },
        6 => {
            let mut terms = [None; 8];
            let present = 1 + rng.pick(7) as usize;
            for (i, t) in terms.iter_mut().enumerate().take(present + 1) {
                if i == 0 || rng.pick(2) == 0 {
                    *t = Some(signed(rng, 60.0, 600.0));
                }
            }
            CurvatureLaw::Polynomial {
                terms,
                from: rng.range(0.0, 40.0),
            }
        }
        7 => CurvatureLaw::CosineSpiral {
            constant: (rng.pick(2) == 0).then(|| signed(rng, 100.0, 1000.0)),
            cosine: signed(rng, 100.0, 1000.0),
            length: rng.range(50.0, 300.0),
            from: rng.range(0.0, 100.0),
        },
        _ => CurvatureLaw::SineSpiral {
            constant: (rng.pick(2) == 0).then(|| signed(rng, 100.0, 1000.0)),
            linear: (rng.pick(2) == 0).then(|| signed(rng, 100.0, 300.0)),
            sine: signed(rng, 100.0, 1000.0),
            length: rng.range(50.0, 300.0),
            from: rng.range(0.0, 100.0),
        },
    }
}

const LAWS: [&str; 9] = [
    "clothoid",
    "Helmert",
    "Bloss",
    "cosine",
    "sine",
    "cant-coupled",
    "polynomial spiral",
    "cosine spiral",
    "sine spiral",
];

fn transition(k: usize, rng: &mut Rng) -> Option<Transition<2>> {
    let start: [f64; 2] = rng.vector(1000.0);
    let direction: [f64; 2] = rng.vector(1.0);
    let length = rng.range(1.0, 200.0);
    Transition::new(start, direction, length, law(k, rng), &mut meter()).ok()
}

/// Claim: a transition's bound and first two derivative bounds over any span hold the curve at
/// every station of it: a million seeded stations for the kind, about 111,000 for each law.
#[test]
fn every_transition_law_holds_a_million_seeded_stations() {
    let mut total = 0;
    for (k, name) in LAWS.iter().enumerate() {
        let mut rng = Rng(0x6763 + k as u64);
        let m = &mut meter();
        let (curves, spans, stations) = (scale(12), 10, 930);
        let (mut checked, mut built) = (0, 0);
        for _ in 0..curves {
            let Some(c) = transition(k, &mut rng) else {
                continue;
            };
            built += 1;
            for _ in 0..spans {
                let s = span(&mut rng, c.domain());
                let b0 = c.bound(s, m).unwrap();
                let b1 = bounded(c.derivative_bound(s, 1, m).unwrap());
                let b2 = bounded(c.derivative_bound(s, 2, m).unwrap());
                for _ in 0..stations {
                    let t = station(&mut rng, s);
                    let e = c.enclose(t, Limit::FromRight, 2, m).unwrap();
                    assert!(
                        meets(&e[0], &b0) && meets(&e[1], &b1) && meets(&e[2], &b2),
                        "{name}: station {t} outside the bounds over {s:?} of {c:?}"
                    );
                    checked += 1;
                }
            }
        }
        assert!(
            built * 10 >= curves * 9,
            "{name}: {built} of {curves} built"
        );
        println!("{name}: {checked} stations held");
        total += checked;
    }
    assert!(total * 10 >= scale(1_000_000) * 9, "{total}");
}

/// Claim: a transition's box is close to its extent: within a few thousandths of the span's
/// length of dense samples' extremes (pieces turning at most a sixteenth of a half-turn).
#[test]
fn transition_bounds_are_close_to_their_extents() {
    let mut rng = Rng(0x6768);
    let m = &mut meter();
    for round in 0..scale(900).max(18) {
        let k = round % 9;
        let Some(c) = transition(k, &mut rng) else {
            continue;
        };
        let s = span(&mut rng, c.domain());
        let b = c.bound(s, m).unwrap();
        let (mut min, mut max) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for i in 0..=2000 {
            let t = (s.0 + (s.1 - s.0) * (f64::from(i) / 2000.0)).min(s.1);
            let p = c.point(t, Limit::FromRight).unwrap();
            for d in 0..2 {
                min[d] = min[d].min(p[d]);
                max[d] = max[d].max(p[d]);
            }
        }
        let slack = 3e-3 * (s.1 - s.0) + 1e-9 * (1.0 + max[0].abs() + max[1].abs());
        for d in 0..2 {
            assert!(
                b.lo[d] <= min[d] + 1e-9 && max[d] - 1e-9 <= b.hi[d],
                "{}: {b:?}",
                LAWS[k]
            );
            assert!(
                min[d] - b.lo[d] <= slack && b.hi[d] - max[d] <= slack,
                "{}: {b:?} against [{min:?}, {max:?}] over {s:?}",
                LAWS[k]
            );
        }
    }
}

/// Claim: spans outside a transition, reversed or not finite are refused.
#[test]
fn transition_spans_outside_are_refused() {
    let m = &mut meter();
    let law = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 0.01,
    };
    let c = Transition::<2>::new([0.0, 0.0], [1.0, 0.0], 100.0, law, m).unwrap();
    for s in [
        (-1.0, 10.0),
        (10.0, 101.0),
        (20.0, 10.0),
        (f64::NAN, 1.0),
        (0.0, f64::INFINITY),
    ] {
        assert_eq!(c.bound(s, m), Err(CurveError::SpanOutside), "{s:?}");
        assert_eq!(
            c.derivative_bound(s, 1, m),
            Err(CurveError::SpanOutside),
            "{s:?}"
        );
    }
    for t in [-1.0, 100.5, f64::NAN] {
        assert_eq!(c.enclose_fresnel(t, m), Err(CurveError::ParameterOutside));
    }
}

// ---- Fresnel integrals -------------------------------------------------------------------

/// The truth's slack: its own truncation, far below a double's step.
fn slack() -> IBig {
    one() >> 180
}

fn holds(i: Interval, v: &IBig) -> bool {
    fx(i.lo()) - slack() <= *v && *v <= fx(i.hi()) + slack()
}

/// C(x) and S(x) in 256-bit fixed point: ∫₀ˣ e^{iπt²/2} by the truth's Taylor pieces, its
/// heading split every half unit so each piece's series converges fast.
fn fresnel_truth(x: f64) -> [IBig; 2] {
    let half_pi = pi() >> 1;
    let poly = vec![IBig::ZERO, IBig::ZERO, half_pi];
    let ends = (x.abs() * 2.0).ceil() as i64;
    let parts = (0..ends.max(1))
        .map(|k| (frac(k, 2), poly.clone()))
        .collect();
    let truth = Truth {
        parts,
        wave: None,
        start: [IBig::ZERO, IBig::ZERO],
        unit: [one(), IBig::ZERO],
    };
    let [c, s] = truth.local(&fx(x.abs()));
    if x < 0.0 { [-c, -s] } else { [c, s] }
}

/// Claim: the Fresnel integrals' enclosures hold the 256-bit truth, on the series' range, between
/// the ranges and on the expansion's, at the ranges' edges, and for negative arguments; and they
/// are a few times 10⁻¹⁵ wide, or 10⁻¹⁴ between the ranges.
#[test]
fn fresnel_enclosures_hold_the_truth() {
    let mut points = vec![
        0.0,
        1e-300,
        1e-8,
        0.3,
        0.7,
        1.0,
        1.0f64.next_up(),
        1.37,
        2.0,
        2.5,
        3.3,
        4.2,
        5.0f64.next_down(),
        5.0,
        5.01,
        6.5,
        8.0,
        11.5,
    ];
    if cfg!(debug_assertions) {
        points = vec![
            0.3,
            1.0,
            1.0f64.next_up(),
            2.5,
            5.0f64.next_down(),
            5.0,
            6.5,
        ];
    }
    for &x in &points {
        let [tc, ts] = fresnel_truth(x);
        for (x, tc, ts) in [(x, tc.clone(), ts.clone()), (-x, -tc, -ts)] {
            let [c, s] = fresnel(Interval::point(x).unwrap()).unwrap();
            assert!(holds(c, &tc) && holds(s, &ts), "{x}: {c:?} {s:?}");
            let width = (c.hi() - c.lo()).max(s.hi() - s.lo());
            // Measured over [−12, 12] in steps of 10⁻³: at most 3.9·10⁻¹⁵ on the series' range,
            // 2.5·10⁻¹⁴ between and 10⁻¹⁵ on the expansion's.
            let limit = if x.abs() <= 1.0 {
                1e-14
            } else if x.abs() < 5.0 {
                5e-14
            } else {
                2e-15
            };
            assert!(width < limit, "{x}: {width}");
        }
    }
}

/// Claim: Fresnel enclosures over an interval of arguments hold the values at each argument in it,
/// and their limits at huge arguments, where x² leaves the doubles, hold ½.
#[test]
fn fresnel_over_intervals_and_huge_arguments() {
    let mut rng = Rng(0x6766);
    for _ in 0..scale(2000).max(40) {
        let a = rng.range(-12.0, 12.0);
        let w = 10f64.powf(rng.range(-12.0, -2.0));
        let range = fresnel(Interval::new(a, a + w).unwrap()).unwrap();
        for i in 0..=8 {
            let x = (a + w * (f64::from(i) / 8.0)).min(a + w);
            let at = fresnel(Interval::point(x).unwrap()).unwrap();
            for (r, v) in range.iter().zip(&at) {
                assert!(
                    r.lo() <= v.hi() && v.lo() <= r.hi(),
                    "{a} + {w}: {range:?} misses {at:?}"
                );
            }
        }
        let width = range[0].hi() - range[0].lo();
        assert!(width <= 2.0 * w + 1e-13, "{a} {w} {width}");
    }
    for x in [1e3, 1e8, 1.234e15, 1e100, 1e200, f64::MAX] {
        let [c, s] = fresnel(Interval::point(x).unwrap()).unwrap();
        let reach = 2.0 / (std::f64::consts::PI * x);
        for v in [c, s] {
            assert!(v.lo() <= 0.5 + reach && 0.5 - reach <= v.hi(), "{x}: {v:?}");
            assert!(v.hi() - v.lo() <= 4.0 * reach + 1e-15, "{x}: {v:?}");
        }
        let [nc, ns] = fresnel(Interval::point(-x).unwrap()).unwrap();
        assert_eq!((nc.lo(), nc.hi()), (-c.hi(), -c.lo()));
        assert_eq!((ns.lo(), ns.hi()), (-s.hi(), -s.lo()));
    }
    assert_eq!(
        fresnel(Interval::new(f64::MAX, f64::MAX).unwrap()).map(|_| ()),
        Ok(())
    );
}

/// The truth of a clothoid from curvature `k1` changing at `rate` per unit length (fixed point),
/// placed at the origin heading along x.
fn clothoid_truth(k1: &IBig, rate: &IBig, t: f64) -> [IBig; 2] {
    let poly = vec![IBig::ZERO, k1.clone(), rate >> 1];
    let truth = Truth {
        parts: vec![(IBig::ZERO, poly)],
        wave: None,
        start: [IBig::ZERO, IBig::ZERO],
        unit: [one(), IBig::ZERO],
    };
    truth.local(&fx(t))
}

/// Claim: the clothoid's Fresnel route holds the truth and meets the quadrature's enclosure, for
/// the clothoid law and IFC's `IfcClothoid` (with and without a constant term, both signs), and
/// declines every other law.
#[test]
fn the_fresnel_route_holds_the_truth_and_meets_the_quadrature() {
    let m = &mut meter();
    // (law, length): the truth's start curvature and rate worked exactly from the stored doubles.
    let cases = [
        (
            CurvatureLaw::Clothoid {
                start: 0.0,
                end: 1.0 / 300.0,
            },
            80.0,
        ),
        (
            CurvatureLaw::Clothoid {
                start: 1.0 / 500.0,
                end: -1.0 / 250.0,
            },
            120.0,
        ),
        (
            CurvatureLaw::Clothoid {
                start: -0.04,
                end: 0.03,
            },
            64.0,
        ),
        (
            CurvatureLaw::Polynomial {
                terms: [None, Some(120.0), None, None, None, None, None, None],
                from: 0.0,
            },
            90.0,
        ),
        (
            CurvatureLaw::Polynomial {
                terms: [
                    Some(400.0),
                    Some(-150.0),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
                from: 30.0,
            },
            50.0,
        ),
        (
            CurvatureLaw::Polynomial {
                terms: [Some(-900.0), Some(75.5), None, None, None, None, None, None],
                from: 12.25,
            },
            40.0,
        ),
    ];
    for (law, length) in cases {
        let (k1, rate) = match law {
            CurvatureLaw::Clothoid { start, end } => {
                (fx(start), div(&(fx(end) - fx(start)), &fx(length)))
            }
            CurvatureLaw::Polynomial { terms, from } => {
                // κ(σ) = 1/A₀ + sign(A₁)·σ/A₁², σ = from + t.
                let a1 = terms[1].unwrap();
                let rate = div(&one(), &mul(&fx(a1), &fx(a1)));
                let rate = if a1 < 0.0 { -rate } else { rate };
                let k0 = terms[0].map_or(IBig::ZERO, |a0| div(&one(), &fx(a0)));
                (k0 + mul(&rate, &fx(from)), rate)
            }
            _ => panic!("only the clothoid laws here"),
        };
        let c = Transition::<2>::new([0.0, 0.0], [1.0, 0.0], length, law, m).unwrap();
        for i in 0..=12 {
            let t = length * (f64::from(i) / 12.0);
            let f = c.enclose_fresnel(t, m).unwrap().unwrap();
            let g = c.enclose(t, Limit::FromRight, 0, m).unwrap()[0];
            let truth = clothoid_truth(&k1, &rate, t);
            for d in 0..2 {
                assert!(holds(f[d], &truth[d]), "{law:?} at {t}: {f:?}");
                assert!(
                    f[d].lo() <= g[d].hi() && g[d].lo() <= f[d].hi(),
                    "{law:?} at {t}: {f:?} {g:?}"
                );
                assert!(f[d].hi() - f[d].lo() < 1e-9, "{law:?} at {t}: {f:?}");
            }
        }
    }
    // Placed and turned: the start and direction carried.
    let law = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 0.01,
    };
    let c = Transition::<2>::new([5.0, -7.0], [3.0, 4.0], 60.0, law, m).unwrap();
    for t in [0.0, 13.0, 60.0] {
        let f = c.enclose_fresnel(t, m).unwrap().unwrap();
        let g = c.enclose(t, Limit::FromRight, 0, m).unwrap()[0];
        for d in 0..2 {
            assert!(
                f[d].lo() <= g[d].hi() && g[d].lo() <= f[d].hi(),
                "{t}: {f:?} {g:?}"
            );
        }
    }
    // Every other law, and a clothoid of constant curvature, has no Fresnel form.
    let mut rng = Rng(0x6769);
    for k in 1..9 {
        if k == 6 {
            continue;
        }
        let Some(c) = transition(k, &mut rng) else {
            continue;
        };
        assert_eq!(
            c.enclose_fresnel(c.length() * 0.5, m),
            Ok(None),
            "{}",
            LAWS[k]
        );
    }
    let arc = Transition::<2>::new(
        [0.0, 0.0],
        [1.0, 0.0],
        10.0,
        CurvatureLaw::Clothoid {
            start: 0.01,
            end: 0.01,
        },
        m,
    )
    .unwrap();
    assert_eq!(arc.enclose_fresnel(5.0, m), Ok(None));
    let cubic = Transition::<2>::new(
        [0.0, 0.0],
        [1.0, 0.0],
        10.0,
        CurvatureLaw::Polynomial {
            terms: [None, Some(100.0), Some(200.0), None, None, None, None, None],
            from: 0.0,
        },
        m,
    )
    .unwrap();
    assert_eq!(cubic.enclose_fresnel(5.0, m), Ok(None));
}

// ---- Routes, by the work they cost ------------------------------------------------------------

fn pt(x: f64) -> Interval {
    Interval::point(x).unwrap()
}

/// A meter allowing `n` units of work.
fn budget(n: u64) -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(n))
}

/// Claim: every closed form is one unit of work, however long the span, monotone or not: no
/// fallback to pieces where the closed form decides.
#[test]
fn closed_forms_cost_one_unit_of_work() {
    let wave = |q: f64| Scalar::Wave {
        p: pt(1.0),
        q: pt(q),
        a: pt(0.6),
        b: pt(-0.8),
    };
    let cases = [
        // A circle's coordinate, and a helix's with a slope below and above π·R.
        (wave(0.0), (-0.7, 2.9)),
        (wave(0.3), (-0.7, 2.9)),
        (wave(0.3), (17.3, 1789.6)),
        (wave(-0.3), (17.3, 1789.6)),
        (wave(4.0), (-0.7, 2.9)),
        (wave(-4.0), (-0.7, 2.9)),
        // A line.
        (
            Scalar::Wave {
                p: pt(1.0),
                q: pt(2.0),
                a: pt(0.0),
                b: pt(0.0),
            },
            (-3.0, 5.0),
        ),
        (
            Scalar::Quadratic {
                p: pt(1.0),
                q: pt(-2.0),
                r: pt(0.7),
            },
            (-3.0, 5.0),
        ),
        // Interior extremum, none (a + b = 0, so a·e⁻ᵗ), and monotone (b > a).
        (
            Scalar::Hyperbolic {
                p: pt(0.0),
                a: pt(1.0),
                b: pt(0.5),
            },
            (-2.0, 2.0),
        ),
        (
            Scalar::Hyperbolic {
                p: pt(0.0),
                a: pt(1.0),
                b: pt(-1.0),
            },
            (-2.0, 2.0),
        ),
        (
            Scalar::Hyperbolic {
                p: pt(0.0),
                a: pt(0.5),
                b: pt(1.0),
            },
            (-2.0, 2.0),
        ),
        (
            Scalar::Involute {
                p: pt(0.0),
                a: pt(0.6),
                b: pt(-0.8),
            },
            (-0.7, 2.9),
        ),
        // A modulated scalar with no t in its coefficients is a wave.
        (
            Scalar::Modulated {
                p: pt(0.0),
                a: pt(0.6),
                b: pt(-0.8),
                c: pt(0.0),
                d: pt(0.0),
            },
            (-0.7, 2.9),
        ),
    ];
    for (g, s) in cases {
        let one = extent(&g, s, &mut budget(1));
        let any = extent(&g, s, &mut meter()).unwrap();
        assert_eq!(one, Ok(any), "{g:?} over {s:?}");
    }
    // The modulated scalar with no t in its coefficients is exactly the wave.
    let modulated = Scalar::Modulated {
        p: pt(0.25),
        a: pt(0.6),
        b: pt(-0.8),
        c: pt(0.0),
        d: pt(0.0),
    };
    let same = Scalar::Wave {
        p: pt(0.25),
        q: pt(0.0),
        a: pt(0.6),
        b: pt(-0.8),
    };
    assert_eq!(
        extent(&modulated, (-0.7, 2.9), &mut meter()),
        extent(&same, (-0.7, 2.9), &mut meter())
    );
}

/// Claim: where the closed form is undecided (no closed form, coefficients holding the origin, a
/// critical point that cannot be placed, a span reaching 2⁵⁰), the pieces' route bounds the scalar
/// at more than one unit of work, and still holds it.
#[test]
fn undecided_scalars_take_the_pieces_route() {
    let far = 1_125_899_906_842_624.0;
    let cases = [
        (
            Scalar::Modulated {
                p: pt(0.0),
                a: pt(0.6),
                b: pt(-0.8),
                c: pt(1.9),
                d: pt(2.5),
            },
            (-0.7, 2.9),
        ),
        (
            Scalar::Wave {
                p: pt(0.0),
                q: pt(0.0),
                a: Interval::new(-0.5, 1.0).unwrap(),
                b: Interval::new(-0.5, 1.0).unwrap(),
            },
            (-0.7, 2.9),
        ),
        (
            Scalar::Quadratic {
                p: pt(0.0),
                q: pt(1.0),
                r: Interval::new(0.0, 1.0).unwrap(),
            },
            (-3.0, 2.0),
        ),
        (
            Scalar::Wave {
                p: pt(0.0),
                q: pt(0.0),
                a: pt(0.6),
                b: pt(-0.8),
            },
            (0.0, far),
        ),
        (
            Scalar::Wave {
                p: pt(0.0),
                q: pt(0.0),
                a: pt(0.6),
                b: pt(-0.8),
            },
            (-far, 0.0),
        ),
    ];
    for (g, s) in cases {
        assert!(
            matches!(extent(&g, s, &mut budget(1)), Err(CurveError::Stopped(_))),
            "{g:?} over {s:?}"
        );
        assert!(extent(&g, s, &mut meter()).is_ok(), "{g:?} over {s:?}");
    }
}

/// Claim: coefficients whose enclosures have zero at one end, not zero itself, are not taken for
/// zero or for certainly signed: the extent holds the scalar for every coefficient in them. So is a
/// hyperbolic quotient (a − b)/(a + b) whose enclosure ends at exactly zero, where ½·ln is −∞.
#[test]
fn coefficients_with_zero_at_an_end_are_held() {
    let m = &mut meter();
    // a·cos πt for a in [0, 1] reaches 1 at t = 0.
    let g = Scalar::Wave {
        p: pt(0.0),
        q: pt(0.0),
        a: Interval::new(0.0, 1.0).unwrap(),
        b: pt(0.0),
    };
    let e = extent(&g, (-0.5, 0.5), m).unwrap();
    assert!(e.lo() <= 0.0 && e.hi() >= 1.0, "{e:?}");
    // q·t + r·t² for r in [0, 1] and in [−1, 0]: the vertex −q/2r may be anywhere.
    for (r, extreme) in [((0.0, 1.0), -0.25), ((-1.0, 0.0), 0.25)] {
        let g = Scalar::Quadratic {
            p: pt(0.0),
            q: pt(1.0),
            r: Interval::new(r.0, r.1).unwrap(),
        };
        let e = extent(&g, (-3.0, 2.0), m).unwrap();
        assert!(e.lo() <= extreme && extreme <= e.hi(), "{r:?}: {e:?}");
    }
    // a·cosh t + sinh t for a in [1, 1.001]: (a − b)/(a + b) holds 0 at its lower end.
    let g = Scalar::Hyperbolic {
        p: pt(0.0),
        a: Interval::new(1.0, 1.001).unwrap(),
        b: pt(1.0),
    };
    let e = extent(&g, (-3.0, 1.0), m).unwrap();
    for a in [1.0, 1.0005, 1.001] {
        let h = Scalar::Hyperbolic {
            p: pt(0.0),
            a: pt(a),
            b: pt(1.0),
        };
        for i in 0..=100 {
            let t = -3.0 + 4.0 * f64::from(i) / 100.0;
            let v = h.over(pt(t)).unwrap();
            assert!(
                e.lo() <= v.hi() && v.lo() <= e.hi(),
                "{a} at {t}: {e:?} {v:?}"
            );
        }
    }
    // a in [10⁻³⁰⁸, 1] and b two steps below 10⁻³⁰⁸, where the doubles are a step of 2⁻¹⁰⁷⁴ apart:
    // a − b's lower end is two steps, widened to one, and its quotient by a + b (at most just
    // over 1) rounds to one step, widened to zero. So the quotient's enclosure starts at exactly
    // zero: undecided, and the extent holds the scalar for every a.
    let low = 1e-308f64;
    let below = low.next_down().next_down();
    let g = Scalar::Hyperbolic {
        p: pt(0.0),
        a: Interval::new(low, 1.0).unwrap(),
        b: pt(below),
    };
    let e = extent(&g, (-2.0, 2.0), m).unwrap();
    for a in [low, 0.5, 1.0] {
        let h = Scalar::Hyperbolic {
            p: pt(0.0),
            a: pt(a),
            b: pt(below),
        };
        for i in 0..=100 {
            let t = -2.0 + 4.0 * f64::from(i) / 100.0;
            let v = h.over(pt(t)).unwrap();
            assert!(
                e.lo() <= v.hi() && v.lo() <= e.hi(),
                "{a} at {t}: {e:?} {v:?}"
            );
        }
    }
}

/// Claim: a gentle transition's box over a span costs three units of work (its one piece and the
/// two certified points at its ends): no piece is cut in eight where no coordinate turns back.
#[test]
fn a_gentle_transition_is_one_piece() {
    let law = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 0.001,
    };
    let c = Transition::<2>::new([3.0, -2.0], [3.0, 4.0], 100.0, law, &mut meter()).unwrap();
    let b = c.bound((10.0, 70.0), &mut budget(3)).unwrap();
    let p = |t: f64| c.point(t, Limit::FromRight).unwrap();
    let (a, z) = (p(10.0), p(70.0));
    for d in 0..2 {
        assert!(
            b.lo[d] <= a[d].min(z[d]) && a[d].max(z[d]) <= b.hi[d],
            "{b:?}"
        );
    }
    assert!(matches!(
        c.bound((10.0, 70.0), &mut budget(2)),
        Err(CurveError::Stopped(_))
    ));
}

/// Claim: a clothoid whose rate of curvature underflows, so that α's enclosure has zero at one
/// end, has no Fresnel form: the route declines, and refuses nothing.
#[test]
fn a_clothoid_of_an_underflowing_rate_has_no_fresnel_form() {
    let m = &mut meter();
    for end in [5e-324, -5e-324] {
        let law = CurvatureLaw::Clothoid { start: 0.0, end };
        let c = Transition::<2>::new([0.0, 0.0], [1.0, 0.0], 1.0, law, m).unwrap();
        assert_eq!(c.enclose_fresnel(0.5, m), Ok(None), "{end}");
    }
}

/// Claim: a polynomial spiral with no terms is straight, the α of its heading exactly zero (no
/// rounding enters it, unlike a clothoid of equal curvatures), and it has no Fresnel form: the
/// route declines, and refuses nothing.
#[test]
fn a_spiral_of_no_terms_has_no_fresnel_form() {
    let m = &mut meter();
    for from in [0.0, 3.0] {
        let law = CurvatureLaw::Polynomial {
            terms: [None; 8],
            from,
        };
        let c = Transition::<2>::new([0.0, 0.0], [1.0, 0.0], 10.0, law, m).unwrap();
        assert_eq!(c.enclose_fresnel(5.0, m), Ok(None), "{from}");
    }
}

/// Claim: below 2⁻³⁰⁰ the Fresnel integrals' enclosures hold x³-sized S, never a point at zero,
/// and at 2⁻³⁰⁰ itself the series gives S strictly positive.
#[test]
fn tiny_fresnel_arguments_keep_s_positive() {
    for x in [1e-300, 1e-320, 5e-324] {
        let [c, s] = fresnel(pt(x)).unwrap();
        assert!(c.lo() <= x && x <= c.hi(), "{x}: {c:?}");
        assert!(s.lo() >= 0.0 && s.hi() > 0.0, "{x}: {s:?}");
    }
    let x = 4.909_093_465_297_727e-91;
    let [_, s] = fresnel(pt(x)).unwrap();
    assert!(s.lo() > 0.0, "{s:?}");
}

/// C(x) and S(x) for x ≥ 1000 in 256-bit fixed point: DLMF 7.5.3 and 7.5.4 with the expansions
/// 7.12.2 and 7.12.3 summed until a term is below 2⁻²⁴⁰ (where the least term is far smaller), and
/// the angle πx²/2 taken from x² exactly: x²/2 reduced modulo 2 as a rational.
fn fresnel_far_truth(x: f64) -> [IBig; 2] {
    let q = RBig::try_from(x).unwrap();
    let num = q.numerator().clone();
    let den = IBig::from(q.denominator().clone());
    let (n2, d2) = (&num * &num, &den * &den);
    // x²/2 = n2/(2·d2); modulo 2 its numerator is n2 modulo 4·d2.
    let modulus = &d2 * IBig::from(4);
    let r = (n2.clone() % &modulus + &modulus) % &modulus;
    let half_turns = (r << F) / (&d2 * IBig::from(2));
    let (cs, sn) = cos_sin(&mul(&pi(), &half_turns));
    let y = mul(&pi(), &((n2 << F) / &d2)) >> 1;
    let (mut f, mut g) = (IBig::ZERO, IBig::ZERO);
    let mut a = one();
    for k in 0..400usize {
        match k % 4 {
            0 => f += &a,
            1 => g += &a,
            2 => f -= &a,
            _ => g -= &a,
        }
        a = div(&mul(&a, &frac(2 * k as i64 + 1, 2)), &y);
        if abs(&a) < (one() >> 240) {
            break;
        }
    }
    let scale = div(&one(), &mul(&pi(), &fx(x)));
    let (f, g) = (mul(&scale, &f), mul(&scale, &g));
    let half = one() >> 1;
    [
        half.clone() + mul(&f, &sn) - mul(&g, &cs),
        half - mul(&f, &cs) - mul(&g, &sn),
    ]
}

/// Claim: at large arguments, where x² is not a double, the Fresnel integrals' enclosures hold the
/// truth: the angle's rounding error e of x² = p + e is taken exactly, not dropped or scaled.
#[test]
fn far_fresnel_arguments_hold_the_truth() {
    for x in [
        1000.5,
        12_345.678,
        333_333.333_333,
        10_000_000.1,
        98_765_432.123_456_7,
        1.5e15 + 2.0,
    ] {
        let [c, s] = fresnel(pt(x)).unwrap();
        let [tc, ts] = fresnel_far_truth(x);
        assert!(holds(c, &tc) && holds(s, &ts), "{x}: {c:?} {s:?}");
        let [nc, ns] = fresnel(pt(-x)).unwrap();
        assert!(holds(nc, &-tc) && holds(ns, &-ts), "{x}");
    }
}

/// Claim: along a family of critical points whose values step evenly, the extreme over a span is
/// at its first or its last member inside it, and both are found: cos πt − t/10 over [1, 9] peaks
/// at its first maximum, cos πt + t/10 over [0.5, 8.5] at its last.
#[test]
fn the_first_and_last_members_of_a_family_are_found() {
    for (q, s) in [(-0.1, (1.0, 9.0)), (0.1, (0.5, 8.5))] {
        let g = Scalar::Wave {
            p: pt(0.0),
            q: pt(q),
            a: pt(1.0),
            b: pt(0.0),
        };
        let e = extent(&g, s, &mut budget(1)).unwrap();
        let mut max = f64::NEG_INFINITY;
        let mut min = f64::INFINITY;
        for i in 0..=100_000 {
            let t = s.0 + (s.1 - s.0) * (f64::from(i) / 100_000.0);
            let v = (std::f64::consts::PI * t).cos() + q * t;
            max = max.max(v);
            min = min.min(v);
        }
        assert!(
            e.hi() >= max - 1e-12 && e.hi() - max < 1e-6,
            "{q}: {e:?} against {max}"
        );
        assert!(
            e.lo() <= min + 1e-12 && min - e.lo() < 1e-6,
            "{q}: {e:?} against {min}"
        );
    }
}

/// Claim: a helix's derivative bounds are closed forms in every coordinate, the axial one, which
/// does not turn, included: one unit of work per coordinate.
#[test]
fn a_helix_derivative_bound_is_one_unit_per_coordinate() {
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let axes = HelixAxes::Frame {
        frame: f,
        radius: 3.0,
        pitch: 0.5,
    };
    let h = Helix::new([0.0; 3], axes, (0.0, 2000.0)).unwrap();
    for order in 0..4 {
        assert!(
            h.derivative_bound((17.3, 1789.6), order, &mut budget(3))
                .is_ok(),
            "{order}"
        );
    }
}
