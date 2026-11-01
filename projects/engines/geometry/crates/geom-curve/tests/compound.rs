//! The bulge polyline, trimmed, composite, offset and curve-on-surface kinds (piece G62) under the
//! kind contract, and their own claims: a bulge arc lies on its circle and converts exactly; a
//! trimmed NURBS curve keeps its knots; a composite's joints are exact and its NURBS form one
//! curve; an offset of a circle is a circle; a curve on a surface carries its cache's error.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests: native arithmetic and indexing, the formulas' names, and a failure stops the test"
)]

use geom_curve::contract::check;
use geom_curve::{
    Basis, BulgePolyline, Circle, Composite, CurveError, CurveKind, Frame, Helix, HelixAxes,
    IntersectionCurve, Line, Marched, Offset, OffsetLaw, OnSurface, ParamCurve, ParamMap, Span,
    Station, SurfaceRef, Trimmed,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_num::math;
use geom_nurbs::{Basis as Knots, Curve, Limit};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Test inputs only: Knuth's MMIX linear congruential sequence.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
    fn below(&mut self, n: usize) -> usize {
        ((self.next() >> 33) % n as u64) as usize
    }
}

fn planar() -> Vec<TypedTransform> {
    vec![
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1)),
        TypedTransform::Exact(GridTransform::mirror(Axis::X)),
        TypedTransform::from(
            RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap(),
        ),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 1.0).unwrap()),
        TypedTransform::from(Transform::scale(1.5, 1.5, 1.0).unwrap()),
    ]
}

fn spatial() -> Vec<TypedTransform> {
    let mut t = planar();
    t.extend([
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1)),
        TypedTransform::from(Transform::rotation(Axis::Y, 0.71).unwrap()),
        TypedTransform::from(Transform::scale(1.5, 0.75, 3.0).unwrap()),
    ]);
    t
}

/// Parameters across a domain, its ends and every whole number inside.
fn params((a, b): (f64, f64)) -> Vec<f64> {
    let mut ps: Vec<f64> = (0..=12)
        .map(|i| (a + (b - a) * f64::from(i) / 12.0).clamp(a, b))
        .collect();
    let mut k = a.ceil();
    while k < b {
        ps.push(k);
        k += 1.0;
    }
    ps
}

fn polyline(rng: &mut Lcg) -> BulgePolyline<2> {
    let n = 2 + rng.below(4);
    let closed = rng.below(2) == 0;
    let vertices: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            // Around a circle, in half-turns.
            let a = 2.0 * i as f64 / n as f64;
            [
                10.0 * math::cospi(a).unwrap().get() + rng.range(-1.0, 1.0),
                10.0 * math::sinpi(a).unwrap().get() + rng.range(-1.0, 1.0),
            ]
        })
        .collect();
    let segments = if closed { n } else { n - 1 };
    let bulges: Vec<f64> = (0..segments)
        .map(|_| match rng.below(5) {
            0 => 0.0,
            1 => 1.0,
            2 => -1.0,
            3 => rng.range(-3.0, 3.0),
            _ => rng.range(-0.5, 0.5),
        })
        .collect();
    let widths = if rng.below(2) == 0 {
        Some(
            (0..segments)
                .map(|_| (rng.range(0.0, 2.0), rng.range(0.0, 2.0)))
                .collect(),
        )
    } else {
        None
    };
    BulgePolyline::new(vertices, bulges, widths, closed).unwrap()
}

fn nurbs3(rng: &mut Lcg) -> Curve<3> {
    let mut k = vec![0.0; 4];
    k.extend([1.0, 2.0]);
    k.extend([3.0; 4]);
    let basis = Knots::new(3, k).unwrap();
    let pts: Vec<[f64; 3]> = (0..basis.count())
        .map(|_| {
            [
                rng.range(-9.0, 9.0),
                rng.range(-9.0, 9.0),
                rng.range(-9.0, 9.0),
            ]
        })
        .collect();
    Curve::new(basis, &pts).unwrap()
}

#[test]
fn bulge_polylines_keep_the_contract() {
    let mut rng = Lcg(621);
    let all = planar();
    for trial in 0..60 {
        let p = polyline(&mut rng);
        let t = &all[trial % all.len()];
        check(&p, &params(p.domain()), t, &mut meter())
            .unwrap_or_else(|e| panic!("trial {trial}: {p:?} under {t:?}: {e}"));
    }
}

#[test]
fn a_bulge_arc_lies_on_its_circle_and_converts_exactly() {
    let m = &mut meter();
    // A semicircle anticlockwise from (−1, 0) to (1, 0) passes (0, −1).
    let p = BulgePolyline::new(vec![[-1.0, 0.0], [1.0, 0.0]], vec![1.0], None, false).unwrap();
    assert_eq!(p.point(0.5, Limit::FromRight).unwrap(), [0.0, -1.0]);
    let n = p.to_nurbs(m).unwrap();
    assert_eq!(
        n.deviation, 0.0,
        "halving and averaging whole numbers is exact"
    );
    assert_eq!(n.value.map, ParamMap::Same);
    // A quarter-ish arc: every enclosed point lies on its circle (centre and radius from the
    // bulge: r = c(1 + b²)/(4|b|), the centre on the chord's bisector).
    let b = 0.25_f64;
    let q = BulgePolyline::new(vec![[0.0, 0.0], [4.0, 0.0]], vec![b], None, false).unwrap();
    let r = 4.0 * (1.0 + b * b) / (4.0 * b);
    let centre = [2.0, r - (4.0 * b / 2.0)];
    for k in 0..=16 {
        let t = f64::from(k) / 16.0;
        let e = q.enclose(t, Limit::FromRight, 0, m).unwrap()[0];
        let (dx, dy) = (e[0].lo() - centre[0], e[1].lo() - centre[1]);
        let d = math::sqrt(dx * dx + dy * dy).unwrap().get();
        assert!((d - r).abs() < 1e-12, "t = {t}: {d} against {r}");
    }
    // The NURBS form passes the vertices at the same parameter.
    let poly = BulgePolyline::new(
        vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
        vec![0.0, 1.0, 0.0, -2.5],
        Some(vec![(0.0, 1.0); 4]),
        true,
    )
    .unwrap();
    let n = poly.to_nurbs(m).unwrap();
    for i in 0..=4 {
        let t = f64::from(i);
        let a = ParamCurve::point(&n.value.curve, t, Limit::FromLeft).unwrap();
        let v = poly.point(t, Limit::FromLeft).unwrap();
        assert!(
            (a[0] - v[0]).abs() < 1e-14 && (a[1] - v[1]).abs() < 1e-14,
            "vertex {i}"
        );
    }
    check(&poly, &params(poly.domain()), &planar()[3], m).unwrap();
    // Reversal swaps each segment's widths and negates the bulges.
    let CurveKind::Bulge(r) = poly.reversed(m).unwrap().value else {
        panic!("a reversed polyline is a polyline")
    };
    assert_eq!(r.bulges(), &[2.5, -0.0, -1.0, -0.0]);
    assert_eq!(r.widths().unwrap()[0], (1.0, 0.0));
    // A non-uniform scale turns its arcs elliptical: a NURBS curve, explicitly.
    let squash = TypedTransform::from(Transform::scale(2.0, 0.5, 1.0).unwrap());
    assert!(matches!(
        poly.transformed(&squash, m).unwrap().value,
        CurveKind::Nurbs(_)
    ));
    // A split trims it.
    assert!(matches!(
        poly.split(1.5, m).unwrap().0.value,
        CurveKind::Trimmed(_)
    ));
}

#[test]
fn trimmed_curves_keep_their_basis() {
    let mut rng = Lcg(622);
    let all = spatial();
    for trial in 0..30 {
        let c = nurbs3(&mut rng);
        let lo = rng.range(0.0, 1.4);
        let hi = rng.range(1.6, 3.0);
        let t = Trimmed::new(Basis::Nurbs(c.clone()), (lo, hi)).unwrap();
        check(&t, &params((lo, hi)), &all[trial % all.len()], &mut meter())
            .unwrap_or_else(|e| panic!("trial {trial}: {e}"));
        assert_eq!(t.basis(), &Basis::Nurbs(c));
    }
    let circle = Circle::new([0.0; 3], 2.0, Frame::standard().unwrap(), (0.0, 2.0)).unwrap();
    let arc = Trimmed::new(Basis::Circle(circle), (0.25, 1.25)).unwrap();
    check(&arc, &params((0.25, 1.25)), &all[0], &mut meter()).unwrap();
    assert_eq!(
        Trimmed::new(
            Basis::Circle(Circle::full([0.0; 3], 1.0, Frame::standard().unwrap()).unwrap()),
            (-0.5, 1.0)
        )
        .err(),
        Some(CurveError::SpanOutside)
    );
}

/// A line, an arc and a NURBS curve, joined exactly.
fn chain() -> Composite<3> {
    let line = Line::new([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
    // The circle of radius 1 about (1, 1, 0) is at (1, 0, 0) at t = 1.5 and (2, 1, 0) at t = 2.
    let circle = Circle::full([1.0, 1.0, 0.0], 1.0, Frame::standard().unwrap()).unwrap();
    let knots = Knots::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let spline = Curve::new(knots, &[[2.0, 1.0, 0.0], [2.0, 3.0, 1.0], [0.0, 3.0, 2.0]]).unwrap();
    Composite::new(vec![
        Trimmed::whole(Basis::Line(line)).unwrap(),
        Trimmed::new(Basis::Circle(circle), (1.5, 2.0)).unwrap(),
        Trimmed::whole(Basis::Nurbs(spline)).unwrap(),
    ])
    .unwrap()
}

#[test]
fn composites_join_exactly_and_convert_to_one_curve() {
    let m = &mut meter();
    let c = chain();
    // The line and arc meet at an exact point; the NURBS curve's enclosure of its start widens
    // by a few ulps, so that joint's certified gap is that small, not 0.
    assert_eq!(c.gaps()[0], 0.0);
    assert!(c.gaps()[1] < 1e-14, "{:?}", c.gaps());
    assert_eq!(c.starts(), &[0.0, 1.0, 1.5, 2.5]);
    for t in spatial() {
        check(&c, &params(c.domain()), &t, m).unwrap_or_else(|e| panic!("{t:?}: {e}"));
    }
    let n = c.to_nurbs(m).unwrap();
    assert!(n.deviation < 1e-14, "{}", n.deviation);
    assert!(matches!(n.value.map, ParamMap::Pieces(_)));
    assert_eq!(n.value.curve.degree(), 2);
    // Split at a joint and inside a segment, and reversed.
    let (l, r) = c.split(1.0, m).unwrap();
    assert_eq!((l.deviation, r.deviation), (0.0, 0.0));
    assert_eq!(l.value.domain(), (0.0, 1.0));
    let (l, _) = c.split(1.75, m).unwrap();
    assert_eq!(l.value.domain(), (0.0, 1.75));
    let back = c.reversed(m).unwrap();
    assert_eq!(back.value.domain(), (-2.5, 0.0));
    // Segments that do not meet are refused.
    let a = Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
    let b = Line::new([1.0, 1e-9, 0.0], [1.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
    assert_eq!(
        Composite::new(vec![
            Trimmed::whole(Basis::Line(a)).unwrap(),
            Trimmed::whole(Basis::Line(b)).unwrap()
        ])
        .err(),
        Some(CurveError::Disjoint)
    );
}

#[test]
fn an_offset_of_a_circle_is_a_circle() {
    let m = &mut meter();
    let circle = Circle::new([1.0, 2.0], 5.0, Frame::standard().unwrap(), (0.0, 1.5)).unwrap();
    let o = Offset::new(
        Trimmed::whole(Basis::Circle(circle)).unwrap(),
        OffsetLaw::Planar { distance: 1.5 },
    )
    .unwrap();
    // Anticlockwise, the left normal points inward: radius 5 − 1.5.
    for k in 0..=12 {
        let t = 1.5 * f64::from(k) / 12.0;
        let e = o.enclose(t, Limit::FromRight, 0, m).unwrap()[0];
        let (x, y) = (e[0].lo() - 1.0, e[1].lo() - 2.0);
        assert!(
            (math::sqrt(x * x + y * y).unwrap().get() - 3.5).abs() < 1e-12,
            "t = {t}"
        );
    }
    for t in planar() {
        check(&o, &params((0.0, 1.5)), &t, m).unwrap_or_else(|e| panic!("{t:?}: {e}"));
    }
    let cached = o.cached(m).unwrap();
    let (_, dev) = cached.cache().unwrap();
    assert!(dev < 1e-9, "{dev}");
    assert_eq!(cached.to_nurbs(m).unwrap().deviation, dev);
}

#[test]
fn offsets_in_space_and_by_distances_keep_the_contract() {
    let m = &mut meter();
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let helix = Helix::new(
        [0.0, 0.0, 0.0],
        HelixAxes::Frame {
            frame,
            radius: 3.0,
            pitch: 2.0,
        },
        (0.0, 3.0),
    )
    .unwrap();
    let reference = Offset::new(
        Trimmed::whole(Basis::Helix(helix.clone())).unwrap(),
        OffsetLaw::Reference {
            distance: 0.5,
            direction: [0.0, 0.0, 1.0],
        },
    )
    .unwrap();
    let distances = Offset::new(
        Trimmed::whole(Basis::Helix(helix)).unwrap(),
        OffsetLaw::Distances(vec![
            Station {
                at: 0.5,
                lateral: 1.0,
                vertical: 0.0,
            },
            Station {
                at: 2.0,
                lateral: -0.5,
                vertical: 0.75,
            },
        ]),
    )
    .unwrap();
    for t in spatial() {
        check(&reference, &params((0.0, 3.0)), &t, m)
            .unwrap_or_else(|e| panic!("reference {t:?}: {e}"));
        let mut ps = params((0.0, 3.0));
        ps.extend([0.5, 2.0]);
        check(&distances, &ps, &t, m).unwrap_or_else(|e| panic!("distances {t:?}: {e}"));
    }
    // The stations' offsets are where they say, from the side they hold.
    let at = distances.point(0.5, Limit::FromRight).unwrap();
    let base = Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
    let _ = base;
    let h = Helix::new(
        [0.0; 3],
        HelixAxes::Frame {
            frame,
            radius: 3.0,
            pitch: 2.0,
        },
        (0.0, 3.0),
    )
    .unwrap();
    let c = h.point(0.5, Limit::FromRight).unwrap();
    let d = math::sqrt((at[0] - c[0]) * (at[0] - c[0]) + (at[1] - c[1]) * (at[1] - c[1]))
        .unwrap()
        .get();
    assert!((d - 1.0).abs() < 1e-12 && (at[2] - c[2]).abs() < 1e-12);
}

#[test]
fn a_curve_on_a_surface_carries_its_cache() {
    let m = &mut meter();
    let uv = Trimmed::whole(Basis::Line(
        Line::new([0.0, 0.0], [1.0, 2.0], (0.0, 1.0)).unwrap(),
    ))
    .unwrap();
    let knots = Knots::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let cache = Curve::new(knots, &[[0.0, 0.0, 0.0], [1.0, 2.0, 0.0]]).unwrap();
    let on = OnSurface::new(uv, SurfaceRef(7), cache, 1e-9).unwrap();
    for t in spatial() {
        check(&on, &params((0.0, 1.0)), &t, m).unwrap_or_else(|e| panic!("{t:?}: {e}"));
    }
    let grow = TypedTransform::from(Transform::uniform_scale(2.0).unwrap());
    let CurveKind::OnSurface(g) = on.transformed(&grow, m).unwrap().value else {
        panic!("still on its surface")
    };
    assert!(g.error() >= 2e-9 && g.error() < 4e-9, "{}", g.error());
    assert_eq!(g.surface(), SurfaceRef(7));
    assert_eq!(
        OnSurface::<2>::new(
            on.pcurve().clone(),
            SurfaceRef(7),
            Curve::new(
                Knots::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap(),
                &[[0.0, 0.0], [1.0, 1.0]]
            )
            .unwrap(),
            0.0
        )
        .err(),
        Some(CurveError::Dimension { dimension: 2 })
    );
}

#[test]
fn an_intersection_curve_carries_its_caches() {
    let m = &mut meter();
    let uv = |a: [f64; 2], b: [f64; 2]| {
        Trimmed::whole(Basis::Line(Line::new(a, b, (0.0, 1.0)).unwrap())).unwrap()
    };
    let knots = || Knots::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let points = [
        [0.0, 0.0, 0.0],
        [1.0, 2.0, 0.5],
        [2.0, 1.0, 1.0],
        [3.0, 3.0, 0.0],
        [4.0, 2.0, 2.0],
    ];
    let cache = Curve::new(knots(), &points).unwrap();
    let span = |at: (f64, f64), e: f64| Span {
        at,
        curve: e,
        pcurves: [e / 2.0, e / 4.0],
    };
    let chain = |t: f64| Marched {
        at: [Interval::point(t).unwrap(); 4],
        t,
    };
    let make = |spans: Vec<Span>, marched: Vec<Marched>| {
        IntersectionCurve::new(
            [SurfaceRef(3), SurfaceRef(4)],
            marched,
            cache.clone(),
            [uv([0.0, 0.0], [1.0, 1.0]), uv([1.0, 0.0], [0.0, 1.0])],
            spans,
        )
    };
    let curve = make(
        vec![span((0.0, 0.5), 1e-9), span((0.5, 1.0), 2e-9)],
        vec![chain(0.0), chain(0.25), chain(0.75), chain(1.0)],
    )
    .unwrap();
    assert_eq!(curve.error(), 2e-9);
    for t in spatial() {
        check(&curve, &params((0.0, 1.0)), &t, m).unwrap_or_else(|e| panic!("{t:?}: {e}"));
    }
    // A split tiles both halves, cuts the span it falls in and parts the chain.
    let (left, right) = curve.split(0.25, m).unwrap();
    let (CurveKind::Intersection(l), CurveKind::Intersection(r)) = (left.value, right.value) else {
        panic!("still intersection curves")
    };
    let ats = |c: &IntersectionCurve<3>| c.spans().iter().map(|s| s.at).collect::<Vec<_>>();
    assert_eq!(ats(&l), vec![(0.0, 0.25)]);
    assert_eq!(ats(&r), vec![(0.25, 0.5), (0.5, 1.0)]);
    assert_eq!(l.chain().len(), 2);
    assert_eq!(r.chain().len(), 3);
    assert!(r.spans()[1].curve >= 2e-9 && l.spans()[0].pcurves[0] >= 5e-10);
    // A reversal mirrors the spans and the chain.
    let CurveKind::Intersection(b) = curve.reversed(m).unwrap().value else {
        panic!("still an intersection curve")
    };
    assert_eq!(ats(&b), vec![(-1.0, -0.5), (-0.5, 0.0)]);
    assert_eq!(b.spans()[0].curve, 2e-9);
    assert_eq!(b.chain()[0].t, -1.0);
    // A scale by two at least doubles every bound and leaves the pcurves.
    let grow = TypedTransform::from(Transform::uniform_scale(2.0).unwrap());
    let CurveKind::Intersection(g) = curve.transformed(&grow, m).unwrap().value else {
        panic!("still an intersection curve")
    };
    assert!(g.error() >= 4e-9 && g.error() < 8e-9, "{}", g.error());
    assert_eq!(g.pcurves(), curve.pcurves());
    assert_eq!(g.surfaces(), [SurfaceRef(3), SurfaceRef(4)]);
    // Refused: spans that leave a gap, a bound that is not finite, a chain out of order or
    // outside the domain.
    for (spans, marched, why) in [
        (
            vec![span((0.0, 0.4), 1e-9), span((0.5, 1.0), 1e-9)],
            vec![],
            CurveError::SpanOutside,
        ),
        (
            vec![span((0.0, 1.0), f64::NAN)],
            vec![],
            CurveError::NotFinite,
        ),
        (vec![span((0.0, 1.0), -1.0)], vec![], CurveError::NotFinite),
        (
            vec![span((0.0, 1.0), 1e-9)],
            vec![chain(0.5), chain(0.25)],
            CurveError::SpanOutside,
        ),
        (
            vec![span((0.0, 1.0), 1e-9)],
            vec![chain(1.5)],
            CurveError::SpanOutside,
        ),
        (vec![], vec![], CurveError::SpanOutside),
    ] {
        assert_eq!(make(spans, marched).err(), Some(why));
    }
}
