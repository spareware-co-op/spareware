//! The analytic curve kinds (piece G61) under the kind contract, and their own claims: every line,
//! ray, circle, arc, ellipse, hyperbola, parabola, helix, polynomial and involute keeps every law
//! under exact, rational, certified, non-uniform and mirroring transforms, in 2D and 3D; a circle
//! converts to NURBS exactly; a non-uniform scale turns a circle into an ellipse, explicitly; a
//! ray cannot drift; and every refusal is typed.

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
    Axes, Circle, CurveError, CurveKind, Ellipse, Extent, Frame, Helix, HelixAxes, Hyperbola,
    Involute, Line, Parabola, ParamCurve, ParamMap, Polynomial,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_nurbs::Limit;
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
    fn vector<const D: usize>(&mut self, scale: f64) -> [f64; D] {
        core::array::from_fn(|_| self.range(-scale, scale))
    }
}

/// Transforms acting in the plane z = 0, for 2D kinds; the same and more for 3D.
fn transforms(planar: bool) -> Vec<TypedTransform> {
    let mut out = vec![
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1)),
        TypedTransform::Exact(GridTransform::translation([3, -7, 0]).unwrap()),
        TypedTransform::Exact(GridTransform::mirror(Axis::X)),
        TypedTransform::from(
            RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap(),
        ),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 1.0).unwrap()),
        TypedTransform::from(Transform::scale(1.75, 1.75, 1.0).unwrap()),
    ];
    if !planar {
        out.extend([
            TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1)),
            TypedTransform::from(
                RationalTransform::rotation(Axis::Y, Rotation::direction(5, 12).unwrap()).unwrap(),
            ),
            TypedTransform::from(Transform::rotation(Axis::Y, 0.71).unwrap()),
            TypedTransform::from(Transform::scale(1.5, 0.75, 3.0).unwrap()),
            TypedTransform::from(Transform::uniform_scale(1.75).unwrap()),
        ]);
    }
    out
}

/// Nine parameters across [a, b] and its ends.
fn params((a, b): (f64, f64)) -> Vec<f64> {
    (0..=8)
        .map(|i| (a + (b - a) * f64::from(i) / 8.0).clamp(a, b))
        .collect()
}

/// A random frame in D dimensions.
fn frame<const D: usize>(rng: &mut Lcg) -> Frame<D> {
    loop {
        let x = rng.vector::<D>(1.0);
        let n = if D == 2 {
            [0.0, 0.0, if rng.below(2) == 0 { 1.0 } else { -2.0 }]
        } else {
            rng.vector::<3>(1.0)
        };
        if let Ok(f) = Frame::new(x, n) {
            return f;
        }
    }
}

fn axes<const D: usize>(rng: &mut Lcg) -> Axes<D> {
    if rng.below(2) == 0 {
        Axes::Frame {
            frame: frame(rng),
            a: rng.range(0.5, 20.0),
            b: rng.range(0.5, 20.0),
        }
    } else {
        Axes::Vectors {
            u: rng.vector(20.0),
            v: rng.vector(20.0),
        }
    }
}

/// A random domain inside [lo, hi] at least `min` long.
fn domain(rng: &mut Lcg, lo: f64, hi: f64, min: f64) -> (f64, f64) {
    let a = rng.range(lo, hi - min);
    let b = rng.range(a + min, hi);
    (a, b)
}

/// One random kind of each analytic sort, in D dimensions.
fn kinds<const D: usize>(rng: &mut Lcg) -> Vec<CurveKind<D>> {
    let c = rng.vector::<D>(50.0);
    let mut out = vec![
        CurveKind::Line(Line::new(c, rng.vector(10.0), domain(rng, -3.0, 3.0, 0.5)).unwrap()),
        CurveKind::Circle(
            Circle::new(
                c,
                rng.range(0.1, 30.0),
                frame(rng),
                domain(rng, -1.0, 1.0, 0.1),
            )
            .unwrap(),
        ),
        CurveKind::Ellipse(Ellipse::new(c, axes(rng), domain(rng, -1.0, 1.0, 0.1)).unwrap()),
        CurveKind::Hyperbola(Hyperbola::new(c, axes(rng), domain(rng, -3.0, 3.0, 0.2)).unwrap()),
        CurveKind::Parabola(Parabola::new(c, axes(rng), domain(rng, -4.0, 4.0, 0.2)).unwrap()),
        CurveKind::Polynomial(
            Polynomial::new(
                (0..2 + rng.below(6)).map(|_| rng.vector(5.0)).collect(),
                domain(rng, -2.0, 2.0, 0.2),
            )
            .unwrap(),
        ),
        CurveKind::Involute(
            Involute::with_axes(c, axes(rng), domain(rng, -2.0, 3.0, 0.2)).unwrap(),
        ),
    ];
    if D == 3 {
        let ax = if rng.below(2) == 0 {
            HelixAxes::Frame {
                frame: frame(rng),
                radius: rng.range(0.5, 10.0),
                pitch: rng.range(-8.0, 8.0),
            }
        } else {
            HelixAxes::Vectors {
                u: rng.vector(10.0),
                v: rng.vector(10.0),
                w: rng.vector(3.0),
            }
        };
        out.push(CurveKind::Helix(
            Helix::new(c, ax, domain(rng, -4.0, 6.0, 0.2)).unwrap(),
        ));
    }
    out
}

fn contract_in<const D: usize>(seed: u64, trials: usize) {
    let mut rng = Lcg(seed);
    let all = transforms(D == 2);
    for trial in 0..trials {
        for (i, kind) in kinds::<D>(&mut rng).into_iter().enumerate() {
            let t = &all[(trial + i) % all.len()];
            let ps = params(kind.domain());
            check(&kind, &ps, t, &mut meter())
                .unwrap_or_else(|e| panic!("{D}D trial {trial}, {kind:?} under {t:?}: {e}"));
        }
    }
}

#[test]
fn every_kind_keeps_the_contract_in_2d() {
    contract_in::<2>(61, 30);
}

#[test]
fn every_kind_keeps_the_contract_in_3d() {
    contract_in::<3>(62, 30);
}

#[test]
fn rays_and_infinite_lines_keep_the_contract() {
    let ray = Line::ray([1.0, 2.0, 3.0], [0.5, -1.0, 2.0]).unwrap();
    let back = Line::new([1.0, 2.0], [3.0, 1.0], (f64::NEG_INFINITY, 4.0)).unwrap();
    let line = Line::infinite([0.0, 0.0, 0.0], [1.0, 1.0, 0.0]).unwrap();
    assert_eq!(ray.extent(), Extent::Ray);
    assert_eq!(back.extent(), Extent::Ray);
    assert_eq!(line.extent(), Extent::Infinite);
    let grid = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    let ps = [0.0, 0.5, 3.0, 100.0];
    check(&ray, &ps, &grid, &mut meter()).unwrap();
    check(&back, &[-50.0, 0.0, 4.0], &grid, &mut meter()).unwrap();
    check(&line, &[-10.0, 0.0, 7.5], &grid, &mut meter()).unwrap();
    let m = &mut meter();
    assert_eq!(
        ray.to_nurbs(m).err(),
        Some(CurveError::Unbounded),
        "a ray has no NURBS form"
    );
    // A rotation whose image of the direction rounds would drift without bound: refused.
    let turn = TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap());
    assert_eq!(ray.transformed(&turn, m).err(), Some(CurveError::Unbounded));
    // An exact one moves it exactly.
    let moved = ray.transformed(&grid, m).unwrap();
    assert_eq!(moved.deviation, 0.0);
    // Splitting an infinite line gives two rays.
    let (l, r) = line.split(2.0, m).unwrap();
    for (half, dom) in [(l, (f64::NEG_INFINITY, 2.0)), (r, (2.0, f64::INFINITY))] {
        let CurveKind::Line(h) = half.value else {
            panic!("a line splits into lines")
        };
        assert_eq!(h.extent(), Extent::Ray);
        assert_eq!(ParamCurve::domain(&h), dom);
    }
    assert_eq!(
        ray.bound((0.0, f64::INFINITY), m),
        Err(CurveError::SpanOutside)
    );
}

#[test]
fn a_full_circle_converts_exactly() {
    let m = &mut meter();
    for (r, c) in [(1.0, [0.0, 0.0]), (7.0, [-3.0, 12.0]), (0.5, [1.5, -2.25])] {
        let circle = Circle::full(c, r, Frame::standard().unwrap()).unwrap();
        let n = circle.to_nurbs(m).unwrap();
        assert_eq!(n.deviation, 0.0, "radius {r}");
        let curve = &n.value.curve;
        assert_eq!(
            curve.weights().unwrap(),
            [1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0]
        );
        assert_eq!(
            n.value.map,
            ParamMap::Circular(vec![0.0, 0.5, 1.0, 1.5, 2.0])
        );
        // The quarter points are control points, and the NURBS curve passes through them.
        for (k, t) in [0.0, 0.5, 1.0, 1.5, 2.0].into_iter().enumerate() {
            let u = n.value.map.enclose(t).unwrap();
            assert_eq!((u.lo(), u.hi()), (k as f64, k as f64));
            let p = ParamCurve::point(curve, k as f64, Limit::FromRight).unwrap();
            let q = circle.point(t, Limit::FromRight).unwrap();
            assert_eq!(p, q, "t = {t}");
        }
        check(&circle, &params((0.0, 2.0)), &transforms(true)[0], m).unwrap();
    }
    // A full circle on a 3-4-5 frame in space: exact data, a conversion within rounding.
    let tilted = Circle::full(
        [1.0, 2.0, 3.0],
        5.0,
        Frame::new([3.0, 4.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
    )
    .unwrap();
    let n = tilted.to_nurbs(m).unwrap();
    assert!(n.deviation < 1e-13, "{}", n.deviation);
}

#[test]
fn a_non_uniform_scale_turns_a_circle_into_an_ellipse() {
    let m = &mut meter();
    let circle = Circle::new([1.0, 2.0], 3.0, Frame::standard().unwrap(), (0.0, 1.5)).unwrap();
    let squash = TypedTransform::from(Transform::scale(2.0, 0.5, 1.0).unwrap());
    let moved = circle.transformed(&squash, m).unwrap();
    let CurveKind::Ellipse(e) = &moved.value else {
        panic!("a non-uniform scale makes an ellipse: {:?}", moved.value)
    };
    assert_eq!(
        e.axes(),
        Axes::Vectors {
            u: [6.0, 0.0],
            v: [0.0, 1.5]
        }
    );
    assert_eq!(e.center(), [2.0, 1.0]);
    assert_eq!(moved.deviation, 0.0, "every product is exact");
    assert_eq!(ParamCurve::domain(e), (0.0, 1.5), "the same parameter");
    // A similarity keeps it a circle: radius times the scale, the frame carried.
    let grow = TypedTransform::from(Transform::scale(2.5, 2.5, 1.0).unwrap());
    let CurveKind::Circle(c) = circle.transformed(&grow, m).unwrap().value else {
        panic!("a uniform scale keeps a circle")
    };
    assert_eq!(c.radius(), 7.5);
    let quarter = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    let turned = circle.transformed(&quarter, m).unwrap();
    assert_eq!(turned.deviation, 0.0);
    let CurveKind::Circle(c) = turned.value else {
        panic!("a quarter turn keeps a circle")
    };
    assert_eq!(c.radius(), 3.0);
    // A mirror keeps it a circle and turns it the other way: the image at t is the mirror image.
    let mirror = TypedTransform::Exact(GridTransform::mirror(Axis::X));
    let flipped = circle.transformed(&mirror, m).unwrap();
    assert_eq!(flipped.deviation, 0.0);
    let CurveKind::Circle(c) = flipped.value else {
        panic!("a mirror keeps a circle")
    };
    assert_eq!(c.frame().normal(), [0.0, 0.0, -1.0]);
    for t in [0.0, 0.25, 0.5, 1.0] {
        let p = circle.point(t, Limit::FromRight).unwrap();
        let q = c.point(t, Limit::FromRight).unwrap();
        assert!((q[0] + p[0]).abs() < 1e-15 && (q[1] - p[1]).abs() < 1e-15);
    }
}

#[test]
fn a_helix_keeps_its_hand() {
    let m = &mut meter();
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let axes = HelixAxes::Frame {
        frame,
        radius: 2.0,
        pitch: 1.5,
    };
    let h = Helix::new([0.0, 0.0, 0.0], axes, (0.0, 6.0)).unwrap();
    // Reversal keeps the hand; a mirror changes it.
    let CurveKind::Helix(r) = h.reversed(m).unwrap().value else {
        panic!("a reversed helix is a helix")
    };
    let HelixAxes::Frame { pitch, .. } = r.axes() else {
        panic!("still on a frame")
    };
    assert_eq!(pitch, 1.5);
    let mirror = TypedTransform::Exact(GridTransform::mirror(Axis::Z));
    let CurveKind::Helix(mh) = h.transformed(&mirror, m).unwrap().value else {
        panic!("a mirrored helix is a helix")
    };
    let HelixAxes::Frame { pitch, .. } = mh.axes() else {
        panic!("still on a frame")
    };
    assert_eq!(pitch, -1.5);
    // Three turns rise three pitches.
    let top = h.point(6.0, Limit::FromLeft).unwrap();
    assert_eq!(top, [2.0, 0.0, 4.5]);
    // Its NURBS form is within a certified error near rounding.
    let n = h.to_nurbs(m).unwrap();
    assert!(n.deviation < 1e-12, "{}", n.deviation);
    assert_eq!(n.value.map, ParamMap::Same);
    // A helix is a 3D kind.
    assert_eq!(
        Helix::<2>::new(
            [0.0, 0.0],
            HelixAxes::Vectors {
                u: [1.0, 0.0],
                v: [0.0, 1.0],
                w: [0.0, 0.0]
            },
            (0.0, 1.0)
        )
        .err(),
        Some(CurveError::Dimension { dimension: 2 })
    );
}

#[test]
fn an_involute_starts_on_its_circle_with_a_cusp() {
    let m = &mut meter();
    let inv = Involute::new([0.0, 0.0], 4.0, Frame::standard().unwrap(), (-1.0, 2.0)).unwrap();
    assert_eq!(inv.point(0.0, Limit::FromRight).unwrap(), [4.0, 0.0]);
    let mut d = [[0.0; 2]; 2];
    inv.derivatives(0.0, Limit::FromRight, &mut d).unwrap();
    assert_eq!(d[1], [0.0, 0.0], "the cusp");
    // At half a turn the string has unwound half the circumference: the point (−4, 4π).
    let p = inv.point(1.0, Limit::FromRight).unwrap();
    assert!((p[0] + 4.0).abs() < 1e-14 && (p[1] - 4.0 * core::f64::consts::PI).abs() < 1e-13);
    let n = inv.to_nurbs(m).unwrap();
    assert!(n.deviation < 1e-11, "{}", n.deviation);
}

#[test]
fn conversions_report_their_error() {
    let m = &mut meter();
    let mut rng = Lcg(17);
    for _ in 0..20 {
        for kind in kinds::<3>(&mut rng) {
            let n = kind.to_nurbs(m).unwrap();
            let scale = {
                let b = kind.bound(kind.domain(), m).unwrap();
                b.lo.iter()
                    .chain(&b.hi)
                    .fold(1.0_f64, |s, x| if x.abs() > s { x.abs() } else { s })
            };
            assert!(
                n.deviation <= scale * 1e-11,
                "{kind:?}: deviation {} at scale {scale}",
                n.deviation
            );
        }
    }
    // A segment between grid points converts exactly; so does a parabola of whole numbers.
    let seg = Line::segment([0.0, 1.0, 2.0], [5.0, -3.0, 2.0]).unwrap();
    assert_eq!(seg.deviation, 0.0);
    assert_eq!(seg.value.to_nurbs(m).unwrap().deviation, 0.0);
    let par = Parabola::focal([0.0, 0.0], 2.0, Frame::standard().unwrap(), (-2.0, 2.0)).unwrap();
    assert_eq!(par.to_nurbs(m).unwrap().deviation, 0.0);
    // On [0, 3] the cubic's Bezier points are whole (on [0, 2] they would be thirds).
    let poly = Polynomial::new(
        vec![[1.0, 0.0], [0.0, 2.0], [3.0, 0.0], [0.0, -1.0]],
        (0.0, 3.0),
    )
    .unwrap();
    assert_eq!(poly.to_nurbs(m).unwrap().deviation, 0.0);
}

#[test]
fn split_and_reverse_are_exact() {
    let m = &mut meter();
    let mut rng = Lcg(23);
    for kind in kinds::<3>(&mut rng) {
        let (a, b) = kind.domain();
        let mid = f64::midpoint(a, b);
        let (l, r) = kind.split(mid, m).unwrap();
        assert_eq!((l.deviation, r.deviation), (0.0, 0.0));
        let back = kind.reversed(m).unwrap();
        assert_eq!(back.deviation, 0.0);
        for t in params((a, b)) {
            let p = kind.point(t, Limit::FromRight).unwrap();
            let q = back.value.point(-t, Limit::FromRight).unwrap();
            assert_eq!(p, q, "{kind:?} at {t}");
        }
    }
}

#[test]
fn refusals_are_typed() {
    let standard = Frame::<3>::standard().unwrap();
    assert_eq!(
        Frame::<3>::new([2.0, 4.0, 6.0], [1.0, 2.0, 3.0]).err(),
        Some(CurveError::Degenerate),
        "parallel, decided exactly"
    );
    assert!(
        Frame::<3>::new(
            [1.0, 0.0, 0.0],
            [1.0, f64::from_bits(0x3C30_0000_0000_0000), 0.0]
        )
        .is_ok()
    );
    assert_eq!(
        Frame::<2>::new([1.0, 0.0], [0.0, 1.0, 1.0]).err(),
        Some(CurveError::NotInPlane)
    );
    assert_eq!(
        Frame::<4>::standard().err(),
        Some(CurveError::Dimension { dimension: 4 })
    );
    let c = [0.0; 3];
    assert_eq!(
        Circle::new(c, 0.0, standard, (0.0, 1.0)).err(),
        Some(CurveError::Degenerate)
    );
    assert_eq!(
        Circle::new(c, 1.0, standard, (0.0, 2.0 + f64::EPSILON * 2.0)).err(),
        Some(CurveError::SpanOutside),
        "more than one turn"
    );
    assert!(Circle::new(c, 1.0, standard, (-1.0, 1.0)).is_ok());
    assert_eq!(
        Ellipse::new(
            c,
            Axes::Vectors {
                u: [1.0, 2.0, 3.0],
                v: [-2.0, -4.0, -6.0]
            },
            (0.0, 1.0)
        )
        .err(),
        Some(CurveError::Degenerate)
    );
    assert_eq!(
        Hyperbola::new(
            c,
            Axes::Frame {
                frame: standard,
                a: 1.0,
                b: 1.0
            },
            (0.0, 600.0)
        )
        .err(),
        Some(CurveError::SpanOutside)
    );
    assert!(Polynomial::<3>::new(vec![[1.0; 3]], (0.0, 1.0)).is_err());
    assert!(Polynomial::<3>::new(vec![[1.0; 3]; 30], (0.0, 1.0)).is_err());
    assert_eq!(
        Line::new(c, [0.0; 3], (0.0, 1.0)).err(),
        Some(CurveError::Degenerate)
    );
    let circle = Circle::new(c, 1.0, standard, (0.0, 1.0)).unwrap();
    let m = &mut meter();
    assert_eq!(
        circle.point(f64::NAN, Limit::FromRight),
        Err(CurveError::ParameterOutside)
    );
    assert_eq!(circle.bound((0.5, 0.25), m), Err(CurveError::SpanOutside));
    let flat = TypedTransform::from(Transform::scale(1.0, 1.0, 0.0).unwrap_or_else(|_| {
        Transform::from_rows([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
        ])
        .unwrap()
    }));
    assert_eq!(
        circle.transformed(&flat, m).err(),
        Some(CurveError::Degenerate)
    );
    let planar = Circle::new([0.0, 0.0], 1.0, Frame::standard().unwrap(), (0.0, 1.0)).unwrap();
    let tilt = TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1));
    assert_eq!(
        planar.transformed(&tilt, m).err(),
        Some(CurveError::NotInPlane)
    );
}
