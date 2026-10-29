//! The offset surface kind (piece G66) under the kind contract, and its own claims: its partials
//! of every order are the exact kinds' where an offset stays in kind (a sphere, cylinder or torus
//! moved by d is the same kind with its radius changed by d); offset by d and then by −d is the
//! base; its NURBS cache is certified, within a tolerance asked for or refused, and smaller after
//! knot removal; split, reversal and grid transforms keep the kind; every refusal is typed.

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

use geom_curve::{CurveError, DerivativeBound, Frame, ParamMap};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Direction, Limit, NurbsError, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::contract::check;
use geom_surface::{
    Cylinder, Offset, ParamBox, ParamSurface, Plane, Sphere, SurfaceKind, Torus, partial_index,
};

const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

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
    fn vector(&mut self, s: f64) -> [f64; 3] {
        [self.range(-s, s), self.range(-s, s), self.range(-s, s)]
    }
}

/// The dot product of two vectors (in a fixed order: the kernel's tests never call `sum`).
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The largest distance from q to a point of the box of enclosures e.
fn far(q: [f64; 3], e: &[Interval; 3]) -> f64 {
    let mut s = 0.0;
    for (x, i) in q.iter().zip(e) {
        let (a, b) = ((x - i.lo()).abs(), (x - i.hi()).abs());
        let m = if a > b { a } else { b };
        s += m * m;
    }
    s.sqrt()
}

fn frame(rng: &mut Lcg) -> Frame<3> {
    loop {
        if let Ok(f) = Frame::new(rng.vector(1.0), rng.vector(1.0)) {
            return f;
        }
    }
}

fn params(((a, b), (c, d)): ParamBox, n: u32) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for i in 0..=n {
        for j in 0..=n {
            out.push((
                (a + (b - a) * f64::from(i) / f64::from(n)).clamp(a, b),
                (c + (d - c) * f64::from(j) / f64::from(n)).clamp(c, d),
            ));
        }
    }
    out
}

fn transforms() -> Vec<TypedTransform> {
    vec![
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1)),
        TypedTransform::Exact(GridTransform::mirror(Axis::Y)),
        TypedTransform::from(
            RationalTransform::rotation(Axis::Y, Rotation::direction(3, 4).unwrap()).unwrap(),
        ),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap()),
    ]
}

/// A gently waved bicubic patch over [0, 1]², with one interior knot in u when `knot`.
fn wavy(rng: &mut Lcg, knot: bool) -> Surface<3> {
    let ku = if knot {
        vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]
    } else {
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]
    };
    let rows = ku.len() - 4;
    let bu = Basis::new(3, ku).unwrap();
    let bv = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut pts = Vec::new();
    for i in 0..rows {
        for j in 0..4 {
            pts.push([
                i as f64 / (rows - 1) as f64 * 4.0,
                f64::from(j) / 3.0 * 4.0,
                rng.range(-0.4, 0.4),
            ]);
        }
    }
    Surface::new(bu, bv, &pts).unwrap()
}

/// σ = ±1: whether `S_u` × `S_v` points away from the centre (or the axis) c at (u, v).
fn outward(s: &SurfaceKind, c: [f64; 3], (u, v): (f64, f64)) -> f64 {
    let [p, a, b] = s.partials(u, v, SIDES).unwrap();
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    dot(n, [p[0] - c[0], p[1] - c[1], p[2] - c[2]]).signum()
}

fn overlap(a: Interval, b: Interval, slack: f64) -> bool {
    a.lo() - slack <= b.hi() && b.lo() - slack <= a.hi()
}

/// Every partial to `order` of `a` and `b` at the pairs overlap, within `slack`.
fn same_partials(a: &SurfaceKind, b: &SurfaceKind, at: &[(f64, f64)], order: usize, slack: f64) {
    let m = &mut meter();
    for &(u, v) in at {
        let x = a.partials_over(((u, u), (v, v)), SIDES, order, m).unwrap();
        let y = b.partials_over(((u, u), (v, v)), SIDES, order, m).unwrap();
        assert_eq!(x.len(), partial_index(order + 1, 0).unwrap());
        for (k, (p, q)) in x.iter().zip(&y).enumerate() {
            for d in 0..3 {
                assert!(
                    overlap(p[d], q[d], slack),
                    "partial {k}, coordinate {d} at ({u}, {v}): {:?} and {:?}",
                    p[d],
                    q[d]
                );
            }
        }
    }
}

#[test]
fn offsets_of_round_kinds_are_the_round_kinds_moved() {
    let rng = &mut Lcg(0x6606);
    for _ in 0..6 {
        let c = rng.vector(20.0);
        let f = frame(rng);
        let r = rng.range(2.0, 6.0);
        let d = rng.range(-1.5, 1.5);
        let zone = ((-0.6, 0.7), (-0.35, 0.3));
        let sphere = SurfaceKind::Sphere(Sphere::new(c, r, f, zone).unwrap());
        let s = outward(&sphere, c, (0.0, 0.0));
        let moved = SurfaceKind::Sphere(Sphere::new(c, r + s * d, f, zone).unwrap());
        let off = SurfaceKind::Offset(Offset::new(sphere, d).unwrap());
        same_partials(&off, &moved, &params(zone, 3), 3, 0.0);

        let band = ((-0.8, 0.5), (-2.0, 3.0));
        let cyl = SurfaceKind::Cylinder(Cylinder::new(c, r, f, band).unwrap());
        let s = outward(&cyl, c, (0.0, 0.0));
        let moved = SurfaceKind::Cylinder(Cylinder::new(c, r + s * d, f, band).unwrap());
        let off = SurfaceKind::Offset(Offset::new(cyl, d).unwrap());
        same_partials(&off, &moved, &params(band, 3), 3, 0.0);

        let tube_box = ((-0.7, 0.9), (-0.8, 0.6));
        let big = r + 3.0;
        let torus = SurfaceKind::Torus(Torus::new(c, big, r, f, tube_box).unwrap());
        // A point inside the tube, between its centre circle and the surface, on the same
        // parameters: the tube's outward side is away from it.
        let probe = SurfaceKind::Torus(Torus::new(c, big, r * 0.5, f, tube_box).unwrap());
        let inside = probe.point(0.0, 0.0, SIDES).unwrap();
        let s = outward(&torus, inside, (0.0, 0.0));
        let moved = SurfaceKind::Torus(Torus::new(c, big, r + s * d, f, tube_box).unwrap());
        let off = SurfaceKind::Offset(Offset::new(torus, d).unwrap());
        same_partials(&off, &moved, &params(tube_box, 3), 3, 0.0);
    }
}

#[test]
fn an_offset_by_d_then_minus_d_is_the_base() {
    let rng = &mut Lcg(0x66d);
    let m = &mut meter();
    for k in 0..6 {
        let base = match k {
            0..=3 => SurfaceKind::Nurbs(wavy(rng, k % 2 == 1)),
            4 => SurfaceKind::Cylinder(
                Cylinder::new(rng.vector(5.0), 2.5, frame(rng), ((-0.5, 0.5), (0.0, 2.0))).unwrap(),
            ),
            _ => SurfaceKind::Torus(
                Torus::new(
                    rng.vector(5.0),
                    4.0,
                    1.5,
                    frame(rng),
                    ((0.0, 0.5), (0.0, 0.5)),
                )
                .unwrap(),
            ),
        };
        let d = rng.range(0.2, 0.6) * if k % 3 == 0 { -1.0 } else { 1.0 };
        let once = SurfaceKind::Offset(Offset::new(base.clone(), d).unwrap());
        let back = SurfaceKind::Offset(Offset::new(once, -d).unwrap());
        same_partials(&back, &base, &params(base.domain(), 4), 2, 0.0);
        // Within a bound: the enclosures of the round trip are narrow and hold the base's points.
        for (u, v) in params(base.domain(), 5) {
            let e = back.enclose(u, v, SIDES, m).unwrap()[0];
            let p = base.point(u, v, SIDES).unwrap();
            for d in 0..3 {
                assert!(e[d].hi() - e[d].lo() < 1e-12 * (1.0 + p[d].abs()));
                assert!(e[d].lo() - 1e-15 <= p[d] && p[d] <= e[d].hi() + 1e-15);
            }
        }
    }
}

#[test]
fn every_offset_keeps_the_contract() {
    let rng = &mut Lcg(0x6666);
    let m = &mut meter();
    let mut kinds = Vec::new();
    for knot in [false, true] {
        let base = SurfaceKind::Nurbs(wavy(rng, knot));
        kinds.push(Offset::new(base, rng.range(-0.5, 0.5)).unwrap());
    }
    let c = rng.vector(10.0);
    let f = frame(rng);
    let cyl = Cylinder::new(c, 3.0, f, ((-0.4, 0.3), (-1.0, 1.0))).unwrap();
    kinds.push(Offset::new(SurfaceKind::Cylinder(cyl), 0.75).unwrap());
    let inner = Offset::new(SurfaceKind::Nurbs(wavy(rng, false)), 0.3).unwrap();
    kinds.push(Offset::new(SurfaceKind::Offset(inner), -0.1).unwrap());
    // An exact grid map, a rational rotation and a non-uniform scale: the kind kept, and the
    // cache moved with a rational and a certified deviation.
    let ts = transforms();
    for k in kinds {
        let k = k.cached(m).unwrap();
        for t in [&ts[1], &ts[2], &ts[4]] {
            check(&k, &params(k.domain(), 2), t, m)
                .unwrap_or_else(|e| panic!("{e} for {:?} under {t:?}", k.base()));
        }
    }
}

#[test]
fn the_cache_is_within_the_tolerance_or_refused() {
    let rng = &mut Lcg(0xcac4e);
    let m = &mut meter();
    let base = SurfaceKind::Nurbs(wavy(rng, true));
    let off = Offset::new(base, 0.4).unwrap();
    for tol in [1e-1, 1e-2] {
        let form = off.to_nurbs_within(tol, m).unwrap();
        assert!(form.deviation <= tol && form.deviation > 0.0);
        assert_eq!(
            (form.value.u, form.value.v),
            (ParamMap::Same, ParamMap::Same)
        );
        // The base's break at u = 1/2 stays a knot of the cache.
        assert!(form.value.surface.basis_u().knots().contains(&0.5));
        for (u, v) in params(off.domain(), 9) {
            let e = off.enclose(u, v, SIDES, m).unwrap()[0];
            let q = ParamSurface::point(&form.value.surface, u, v, SIDES).unwrap();
            let dist = far(q, &e);
            assert!(dist <= form.deviation, "{dist} > {}", form.deviation);
        }
    }
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            off.to_nurbs_within(bad, m),
            Err(CurveError::Nurbs(NurbsError::Tolerance))
        );
    }
    // Below rounding no sampling certifies: refused, never stretched.
    let plane = Plane::new(
        [1.0, 2.0, 3.0],
        [2.0, 0.0, 1.0],
        [0.0, 3.0, 0.0],
        ((0.0, 1.0), (0.0, 1.0)),
    )
    .unwrap();
    let flat = Offset::new(SurfaceKind::Plane(plane), 0.5).unwrap();
    assert_eq!(
        flat.to_nurbs_within(1e-30, m),
        Err(CurveError::Nurbs(NurbsError::OutsideTolerance))
    );
    // Kept, the cache is returned as it is.
    let kept = off.cached(m).unwrap();
    let (form, dev) = kept.cache().unwrap();
    assert!(off.cache().is_none());
    assert!(dev > 0.0 && dev == off.to_nurbs(m).unwrap().deviation);
    assert_eq!(kept.to_nurbs(m).unwrap().value, *form);
}

#[test]
fn knot_removal_shrinks_a_flat_cache() {
    let m = &mut meter();
    let plane = Plane::new(
        [1.0, 2.0, 3.0],
        [2.0, 0.0, 0.0],
        [0.0, 3.0, 0.0],
        ((0.0, 1.0), (0.0, 1.0)),
    )
    .unwrap();
    let off = Offset::new(SurfaceKind::Plane(plane), 0.5).unwrap();
    let form = off.to_nurbs_within(1e-9, m).unwrap();
    let s = &form.value.surface;
    // A plane needs no interior knot: all were removed.
    assert_eq!((s.basis_u().count(), s.basis_v().count()), (4, 4));
    let p = ParamSurface::point(s, 0.25, 0.75, SIDES).unwrap();
    assert!((p[2] - 3.5).abs() <= form.deviation + 1e-15);
}

#[test]
fn split_reversal_and_grid_transforms_keep_the_kind() {
    let rng = &mut Lcg(0x5911);
    let m = &mut meter();
    let off = Offset::new(SurfaceKind::Nurbs(wavy(rng, true)), 0.3).unwrap();
    let (l, r) = off.split(Direction::U, 0.3, m).unwrap();
    assert_eq!((l.deviation, r.deviation), (0.0, 0.0));
    let SurfaceKind::Offset(left) = &l.value else {
        panic!("a split offset is an offset")
    };
    assert_eq!(left.domain(), ((0.0, 0.3), (0.0, 1.0)));
    assert_eq!(left.breaks(), (vec![], vec![]));
    assert_eq!(
        left.enclose(0.2, 0.4, SIDES, m),
        off.enclose(0.2, 0.4, SIDES, m)
    );
    let SurfaceKind::Offset(right) = &r.value else {
        panic!("a split offset is an offset")
    };
    assert_eq!(right.breaks(), (vec![0.5], vec![]));
    for dir in [Direction::U, Direction::V] {
        let rev = off.reversed(dir, m).unwrap();
        assert_eq!(rev.deviation, 0.0);
        let SurfaceKind::Offset(o) = &rev.value else {
            panic!("a reversed offset is an offset")
        };
        assert_eq!(o.distance(), -0.3);
        let (u, v) = (0.7, 0.2);
        let (ru, rv) = match dir {
            Direction::U => (-u, v),
            Direction::V => (u, -v),
        };
        let a = off.point(u, v, SIDES).unwrap();
        let b = o.point(ru, rv, SIDES).unwrap();
        for d in 0..3 {
            assert!((a[d] - b[d]).abs() < 1e-12);
        }
    }
    let mirrored = off
        .transformed(&TypedTransform::Exact(GridTransform::mirror(Axis::Z)), m)
        .unwrap();
    let SurfaceKind::Offset(o) = &mirrored.value else {
        panic!("a mirrored offset is an offset")
    };
    assert_eq!((o.distance(), mirrored.deviation), (-0.3, 0.0));
    let turned = off
        .transformed(
            &TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1)),
            m,
        )
        .unwrap();
    let SurfaceKind::Offset(o) = &turned.value else {
        panic!("a turned offset is an offset")
    };
    assert_eq!(o.distance(), 0.3);
    let scaled = off
        .transformed(
            &TypedTransform::from(Transform::uniform_scale(2.0).unwrap()),
            m,
        )
        .unwrap();
    assert!(matches!(scaled.value, SurfaceKind::Nurbs(_)) && scaled.deviation > 0.0);
    assert!(off.split(Direction::V, 1.0, m).is_err());
}

#[test]
fn refusals_are_typed_and_poles_honest() {
    let m = &mut meter();
    let base = SurfaceKind::Nurbs(wavy(&mut Lcg(1), false));
    for d in [f64::NAN, f64::INFINITY] {
        assert_eq!(Offset::new(base.clone(), d), Err(CurveError::NotFinite));
    }
    let off = Offset::new(base, 0.2).unwrap();
    assert_eq!(
        off.enclose(1.5, 0.5, SIDES, m),
        Err(CurveError::ParameterOutside)
    );
    assert_eq!(
        off.bound(((0.0, 1.5), (0.0, 1.0)), m),
        Err(CurveError::SpanOutside)
    );
    assert_eq!(
        off.partials_over(((0.0, 1.0), (-0.5, 1.0)), SIDES, 2, m),
        Err(CurveError::SpanOutside)
    );
    // An unbounded plane has no cache.
    let plane = Plane::new(
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        ((f64::NEG_INFINITY, 1.0), (0.0, 1.0)),
    )
    .unwrap();
    let flat = Offset::new(SurfaceKind::Plane(plane), 1.0).unwrap();
    assert_eq!(flat.to_nurbs(m), Err(CurveError::Unbounded));
    // At a sphere's pole the normal of the parameters vanishes: refused, bounded, undecided.
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    let cap = Sphere::new([0.0; 3], 2.0, f, ((0.0, 0.5), (0.25, 0.5))).unwrap();
    let pole = Offset::new(SurfaceKind::Sphere(cap), 0.5).unwrap();
    assert_eq!(pole.enclose(0.2, 0.5, SIDES, m), Err(CurveError::Unbounded));
    let whole = ((0.0, 0.5), (0.25, 0.5));
    assert_eq!(
        pole.derivative_bound(whole, Direction::U, m),
        Ok(DerivativeBound::Unbounded)
    );
    let b = pole.bound(whole, m).unwrap();
    // The pole of the offset sphere, on the axis at radius 2 ± 0.5, lies in the bound.
    let s = outward(pole.base(), [0.0; 3], (0.2, 0.3));
    let p = pole.base().point(0.2, 0.5, SIDES).unwrap();
    for ((&x, &lo), &hi) in p.iter().zip(&b.lo).zip(&b.hi) {
        let y = x * (1.0 + s * 0.25);
        assert!(lo <= y && y <= hi);
    }
    assert_eq!(pole.to_nurbs(m), Err(CurveError::Unbounded));
    let n = pole.normal_bound(whole, m).unwrap();
    assert_eq!((n.lo, n.hi), ([f64::NEG_INFINITY; 3], [f64::INFINITY; 3]));
    // Away from the pole the normal bound decides the sign along the axis (here y).
    let n = pole.normal_bound(((0.0, 0.5), (0.25, 0.4)), m).unwrap();
    assert!(n.lo[1] > 0.0 || n.hi[1] < 0.0, "{n:?}");
}
