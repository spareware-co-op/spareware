//! Attacks on piece G66's claims: the higher partials of every kind and of the offset against an
//! independent finite-difference oracle (rational bases, swept kinds, offsets of offsets); the
//! cache's certificate against dense sampling, on smooth bases and on a kinked one whose offset
//! tears; a folded offset (moved past its radius) under the full contract, its normals turned
//! over; and a kind with faulty partials, which the contract's new laws must catch.

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

use geom_curve::{
    Basis as Leaf, Circle, ContractError, CurveError, DerivativeBound, Frame, Law, Line, Trimmed,
};
use geom_math::{Axis, GridTransform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Bounds, Changed, Direction, Limit, NurbsError, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::contract::check;
use geom_surface::{
    Extrusion, Offset, ParamBox, ParamSurface, Revolution, Ruled, Sphere, SurfaceForm, SurfaceKind,
    partial_index,
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
}

/// A random bicubic patch over [0, 1]², rational when `rational`, with the u knots given.
fn patch(rng: &mut Lcg, ku: Vec<f64>, rational: bool) -> Surface<3> {
    let rows = ku.len() - 4;
    let bu = Basis::new(3, ku).unwrap();
    let bv = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut pts = Vec::new();
    let mut ws = Vec::new();
    for i in 0..rows {
        for j in 0..4 {
            pts.push([
                i as f64 / (rows - 1) as f64 * 3.0,
                f64::from(j),
                rng.range(-0.3, 0.3),
            ]);
            ws.push(rng.range(0.7, 1.4));
        }
    }
    if rational {
        Surface::new_rational(bu, bv, &pts, &ws).unwrap()
    } else {
        Surface::new(bu, bv, &pts).unwrap()
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

fn mid(i: Interval) -> f64 {
    i.lo() * 0.5 + i.hi() * 0.5
}

/// Each partial of order k + 1 at (u, v) against the central difference of the order-k partials
/// at u ± h (or v ± h), for every k below `order`.
fn against_differences(k: &SurfaceKind, (u, v): (f64, f64), order: usize, h: f64, rel: f64) {
    let m = &mut meter();
    let at = |u: f64, v: f64, m: &mut WorkMeter<'_>| {
        k.partials_over(((u, u), (v, v)), SIDES, order, m)
            .unwrap_or_else(|e| panic!("{e:?} at ({u}, {v}) for {k:?}"))
    };
    let here = at(u, v, m);
    let (up, um) = (at(u + h, v, m), at(u - h, v, m));
    let (vp, vm) = (at(u, v + h, m), at(u, v - h, m));
    let mut scale: f64 = 1.0;
    for x in here.iter().flatten() {
        let a = mid(*x).abs();
        if a > scale {
            scale = a;
        }
    }
    for n in 0..order {
        for j in 0..=n {
            let i = n - j;
            let (a, b) = (
                partial_index(i, j).unwrap(),
                partial_index(i + 1, j).unwrap(),
            );
            let c = partial_index(i, j + 1).unwrap();
            for d in 0..3 {
                let du = (mid(up[a][d]) - mid(um[a][d])) / (2.0 * h);
                let dv = (mid(vp[a][d]) - mid(vm[a][d])) / (2.0 * h);
                assert!(
                    (du - mid(here[b][d])).abs() <= rel * scale,
                    "∂u of ({i},{j}) in {d}: {du} against {:?}",
                    here[b][d]
                );
                assert!(
                    (dv - mid(here[c][d])).abs() <= rel * scale,
                    "∂v of ({i},{j}) in {d}: {dv} against {:?}",
                    here[c][d]
                );
            }
        }
    }
}

#[test]
fn every_kinds_higher_partials_are_the_derivatives_of_its_lower_ones() {
    let rng = &mut Lcg(0xa66);
    let at = [(0.31, 0.47), (0.62, 0.21), (0.5, 0.73)];
    for rational in [false, true] {
        let s = patch(rng, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0], rational);
        let base = SurfaceKind::Nurbs(s);
        for &p in &at {
            against_differences(&base, p, 4, 1e-4, 1e-5);
        }
        let off = SurfaceKind::Offset(Offset::new(base.clone(), 0.35).unwrap());
        for &p in &at {
            against_differences(&off, p, 3, 1e-4, 1e-5);
        }
        let twice = SurfaceKind::Offset(Offset::new(off, -0.2).unwrap());
        for &p in &at {
            against_differences(&twice, p, 2, 1e-4, 1e-5);
        }
    }
    let line = Trimmed::whole(Leaf::Line(
        Line::new([2.0, 0.0, 1.0], [1.0, 0.5, 2.0], (0.0, 1.0)).unwrap(),
    ))
    .unwrap();
    let arc = Trimmed::whole(Leaf::Circle(
        Circle::new([0.0; 3], 2.0, Frame::standard().unwrap(), (0.0, 1.0)).unwrap(),
    ))
    .unwrap();
    // The arc lies in the plane z = 0; it turns about the line through (0, 0, 3) along y.
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    let swept = [
        SurfaceKind::Revolution(
            Revolution::new(arc.clone(), [0.0, 0.0, 3.0], f, (0.0, 1.5)).unwrap(),
        ),
        SurfaceKind::Extrusion(Extrusion::new(arc.clone(), [0.0, 1.0, 4.0], (0.0, 2.0)).unwrap()),
        SurfaceKind::Ruled(Ruled::new(line, arc, (0.0, 1.0)).unwrap()),
    ];
    for k in &swept {
        let ((a, b), (c, d)) = k.domain();
        for t in [0.3, 0.55, 0.8] {
            let p = (a + (b - a) * t, c + (d - c) * (1.0 - t));
            against_differences(k, p, 4, 1e-4, 1e-5);
            let off = SurfaceKind::Offset(Offset::new(k.clone(), 0.25).unwrap());
            against_differences(&off, p, 3, 1e-4, 1e-5);
        }
        // Linear in v: nothing above the first order in v, exactly.
        if !matches!(k, SurfaceKind::Revolution(_)) {
            let ((a, _), (c, _)) = k.domain();
            let x = k
                .partials_over(((a, a), (c, c)), SIDES, 3, &mut meter())
                .unwrap();
            for (i, j) in [(0, 2), (1, 2), (0, 3)] {
                assert!(
                    x[partial_index(i, j).unwrap()]
                        .iter()
                        .all(|e| e.lo() == 0.0 && e.hi() == 0.0)
                );
            }
        }
    }
    // The index lists partials by total order, then by order in v; and refuses to overflow.
    let order: Vec<usize> = [(0, 0), (1, 0), (0, 1), (2, 0), (1, 1), (0, 2), (3, 0)]
        .iter()
        .map(|&(i, j)| partial_index(i, j).unwrap())
        .collect();
    assert_eq!(order, vec![0, 1, 2, 3, 4, 5, 6]);
    assert_eq!(partial_index(usize::MAX, 1), Err(CurveError::Defect));
    // i + j fits but n(n + 1) does not, on 32- and 64-bit targets alike.
    let half = usize::MAX / 2;
    assert_eq!(partial_index(half, half), Err(CurveError::Defect));
    // Orders whose factorials are no longer exact doubles are refused by name.
    let rational = SurfaceKind::Nurbs(patch(
        rng,
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        true,
    ));
    assert_eq!(
        rational.partials_over(((0.5, 0.5), (0.5, 0.5)), SIDES, 19, &mut meter()),
        Err(CurveError::Nurbs(NurbsError::DerivativeOrder { order: 19 }))
    );
    assert!(
        rational
            .partials_over(((0.5, 0.5), (0.5, 0.5)), SIDES, 18, &mut meter())
            .is_ok()
    );
}

#[test]
fn the_cache_certificate_holds_under_dense_sampling() {
    let rng = &mut Lcg(0x005a_3b1e);
    let m = &mut meter();
    for (k, rational) in [(0, false), (1, true), (2, false)] {
        let ku = if k == 2 {
            vec![0.0, 0.0, 0.0, 0.0, 0.3, 0.7, 1.0, 1.0, 1.0, 1.0]
        } else {
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]
        };
        let base = SurfaceKind::Nurbs(patch(rng, ku, rational));
        let off = Offset::new(base, rng.range(-0.4, 0.4)).unwrap();
        let tol = [3e-2, 1e-2, 5e-2][k];
        let form = off.to_nurbs_within(tol, m).unwrap_or_else(|e| {
            panic!(
                "{e:?} for {k}: best {:?}",
                off.to_nurbs(m).map(|f| f.deviation)
            )
        });
        let s = &form.value.surface;
        let mut worst: f64 = 0.0;
        for i in 0..=24 {
            for j in 0..=24 {
                let (u, v) = (f64::from(i) / 24.0, f64::from(j) / 24.0);
                let e = off.enclose(u, v, SIDES, m).unwrap()[0];
                let q = ParamSurface::point(s, u, v, SIDES).unwrap();
                let dist = far(q, &e);
                if dist > worst {
                    worst = dist;
                }
            }
        }
        assert!(worst <= form.deviation, "{worst} > {}", form.deviation);
        // Every break of the base is a knot of the cache's u basis.
        for b in off.breaks().0 {
            assert!(s.basis_u().knots().contains(&b), "break {b} kept");
        }
    }
}

#[test]
fn a_torn_offset_is_certified_honestly() {
    // A crease along u = 1/2 (the knot three times): the normal jumps there, so the offset
    // tears into a gap of about 2·d·sin(θ/2) for the crease's angle θ.
    let ku = vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0];
    let bu = Basis::new(3, ku).unwrap();
    let bv = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut pts = Vec::new();
    for i in 0..7 {
        for j in 0..4 {
            let x = f64::from(i) / 6.0 * 2.0;
            let z = if i <= 3 { 0.0 } else { (x - 1.0) * 0.8 };
            pts.push([x, f64::from(j), z]);
        }
    }
    let base = SurfaceKind::Nurbs(Surface::new(bu, bv, &pts).unwrap());
    let off = Offset::new(base, 0.3).unwrap();
    assert_eq!(off.breaks(), (vec![0.5], vec![]));
    let left = off
        .point(0.5, 0.5, (Limit::FromLeft, Limit::FromRight))
        .unwrap();
    let right = off
        .point(0.5, 0.5, (Limit::FromRight, Limit::FromRight))
        .unwrap();
    let step = [left[0] - right[0], left[1] - right[1], left[2] - right[2]];
    let gap = dot(step, step).sqrt();
    assert!(gap > 0.1);
    let m = &mut meter();
    let form = off.to_nurbs(m).unwrap();
    // The cache is continuous, so it misses one side of the tear by at least half the gap: the
    // certificate says so instead of hiding it.
    assert!(form.deviation >= gap * 0.5, "{} < {gap}", form.deviation);
    assert_eq!(
        off.to_nurbs_within(gap * 0.25, m),
        Err(CurveError::Nurbs(NurbsError::OutsideTolerance))
    );
}

/// Whether `S_u` × `S_v` points away from the axis or centre through c (+1) or towards it (−1).
fn away(s: &SurfaceKind, c: [f64; 3], (u, v): (f64, f64)) -> f64 {
    let [p, a, b] = s.partials(u, v, SIDES).unwrap();
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    dot(n, [p[0] - c[0], p[1] - c[1], p[2] - c[2]]).signum()
}

#[test]
fn a_folded_offset_keeps_the_contract_with_its_normals_turned_over() {
    let m = &mut meter();
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    let grid = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    // A cylinder (axis y) moved 3 towards its axis, past it: radius 1 on the far side. One
    // principal radius is passed, 1 − 2Hd + Kd² < 0, and the normals turn over.
    let band = ((0.0, 0.4), (-1.0, 1.0));
    let cyl = SurfaceKind::Cylinder(geom_surface::Cylinder::new([0.0; 3], 2.0, f, band).unwrap());
    let s = away(&cyl, [0.0, 0.3, 0.0], (0.2, 0.3));
    let folded = Offset::new(cyl.clone(), -3.0 * s)
        .unwrap()
        .cached(m)
        .unwrap();
    check(&folded, &[(0.1, -0.5), (0.2, 0.0), (0.35, 0.75)], &grid, m).unwrap();
    let base = cyl.normal_bound(band, m).unwrap();
    let turned = folded.normal_bound(band, m).unwrap();
    let mut decided = 0;
    for d in 0..3 {
        if base.lo[d] > 0.0 {
            assert!(turned.hi[d] < 0.0);
            decided += 1;
        }
        if base.hi[d] < 0.0 {
            assert!(turned.lo[d] > 0.0);
            decided += 1;
        }
    }
    assert!(decided > 0);
    // A sphere moved past its centre passes both radii: the factor is (1 + d/r)² > 0, and the
    // normals keep their side although the points went through the centre.
    let zone = ((0.0, 0.4), (-0.2, 0.2));
    let sphere = SurfaceKind::Sphere(Sphere::new([0.0; 3], 2.0, f, zone).unwrap());
    let s = away(&sphere, [0.0; 3], (0.2, 0.0));
    let through = Offset::new(sphere.clone(), -3.0 * s)
        .unwrap()
        .cached(m)
        .unwrap();
    check(&through, &[(0.1, -0.1), (0.2, 0.0), (0.35, 0.15)], &grid, m).unwrap();
    let base = sphere.normal_bound(zone, m).unwrap();
    let kept = through.normal_bound(zone, m).unwrap();
    for d in 0..3 {
        if base.lo[d] > 0.0 {
            assert!(kept.lo[d] > 0.0);
        }
        if base.hi[d] < 0.0 {
            assert!(kept.hi[d] < 0.0);
        }
    }
    let q = through.point(0.2, 0.0, SIDES).unwrap();
    let r = dot(q, q).sqrt();
    assert!((r - 1.0).abs() < 1e-12);
}

/// A kind with a fault in its partials of every order.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Fault {
    /// The first partials moved off the enclosure's.
    Shifted,
    /// One partial short.
    Short,
    /// A box's partials that miss the point's.
    Narrow,
    /// A box's partials that fail.
    Failing,
    /// A box's partials that cannot be bounded: allowed.
    Unbounded,
    /// Regions outside the domain accepted.
    Outside,
}

struct Faulty {
    inner: Surface<3>,
    fault: Fault,
}

impl ParamSurface for Faulty {
    fn domain(&self) -> ParamBox {
        ParamSurface::domain(&self.inner)
    }
    fn point(&self, u: f64, v: f64, s: (Limit, Limit)) -> Result<[f64; 3], CurveError> {
        ParamSurface::point(&self.inner, u, v, s)
    }
    fn partials(&self, u: f64, v: f64, s: (Limit, Limit)) -> Result<[[f64; 3]; 3], CurveError> {
        self.inner.partials(u, v, s)
    }
    fn enclose(
        &self,
        u: f64,
        v: f64,
        s: (Limit, Limit),
        m: &mut WorkMeter<'_>,
    ) -> Result<[[Interval; 3]; 3], CurveError> {
        self.inner.enclose(u, v, s, m)
    }
    fn bound(&self, r: ParamBox, m: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        ParamSurface::bound(&self.inner, r, m)
    }
    fn derivative_bound(
        &self,
        r: ParamBox,
        d: Direction,
        m: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<3>, CurveError> {
        ParamSurface::derivative_bound(&self.inner, r, d, m)
    }
    fn partials_over(
        &self,
        r: ParamBox,
        s: (Limit, Limit),
        k: usize,
        m: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 3]>, CurveError> {
        let ((u0, u1), (v0, v1)) = r;
        let point = u0 == u1 && v0 == v1;
        if self.fault == Fault::Outside && u0 < 0.0 {
            return self.inner.partials_over(((0.0, u1), (v0, v1)), s, k, m);
        }
        let mut p = self.inner.partials_over(r, s, k, m)?;
        match (self.fault, point) {
            (Fault::Shifted, true) => p[1][0] = Interval::point(p[1][0].hi() + 1.0)?,
            (Fault::Short, true) => {
                p.pop();
            }
            (Fault::Narrow, false) => p[3][2] = Interval::point(1e9)?,
            (Fault::Failing, false) => return Err(CurveError::NotFinite),
            (Fault::Unbounded, false) => return Err(CurveError::Unbounded),
            _ => {}
        }
        Ok(p)
    }
    fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
        self.inner.breaks()
    }
    fn normal_bound(&self, r: ParamBox, m: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        self.inner.normal_bound(r, m)
    }
    fn to_nurbs(&self, m: &mut WorkMeter<'_>) -> Result<Changed<SurfaceForm>, CurveError> {
        self.inner.to_nurbs(m)
    }
    fn split(
        &self,
        d: Direction,
        t: f64,
        m: &mut WorkMeter<'_>,
    ) -> Result<(Changed<SurfaceKind>, Changed<SurfaceKind>), CurveError> {
        ParamSurface::split(&self.inner, d, t, m)
    }
    fn reversed(
        &self,
        d: Direction,
        m: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        self.inner.reversed(d, m)
    }
    fn transformed(
        &self,
        t: &TypedTransform,
        m: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        self.inner.transformed(t, m)
    }
}

#[test]
fn the_contract_catches_faulty_partials() {
    let rng = &mut Lcg(0xfa17);
    let m = &mut meter();
    let inner = patch(rng, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0], false);
    let grid = TypedTransform::Exact(GridTransform::mirror(Axis::X));
    let at = [(0.3, 0.6), (0.7, 0.2)];
    let law = |fault: Fault, m: &mut WorkMeter<'_>| {
        let k = Faulty {
            inner: inner.clone(),
            fault,
        };
        check(&k, &at, &grid, m).err()
    };
    let broken = |l: Law, error: Option<CurveError>| {
        Some(ContractError {
            law: l,
            at: 0.3,
            error,
        })
    };
    assert_eq!(
        law(Fault::Shifted, m),
        broken(Law::EvaluationIsEnclosed, None)
    );
    assert_eq!(
        law(Fault::Short, m),
        broken(Law::EvaluationIsEnclosed, None)
    );
    assert_eq!(
        law(Fault::Narrow, m),
        broken(Law::DerivativeBoundHoldsDerivatives, None)
    );
    assert_eq!(
        law(Fault::Failing, m),
        broken(
            Law::DerivativeBoundHoldsDerivatives,
            Some(CurveError::NotFinite)
        )
    );
    assert_eq!(law(Fault::Unbounded, m), None);
    let outside = law(Fault::Outside, m).unwrap();
    assert_eq!((outside.law, outside.error), (Law::RefusesOutside, None));
}

/// A gently curved bicubic patch over [0, 1]² with a knot at 1/2 in u and in v.
fn knotted() -> Surface<3> {
    let k = vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];
    let (bu, bv) = (Basis::new(3, k.clone()).unwrap(), Basis::new(3, k).unwrap());
    let mut pts = Vec::new();
    for i in 0..5_u8 {
        for j in 0..5_u8 {
            let (x, y) = (f64::from(i), f64::from(j));
            pts.push([x * 0.5, y * 0.5, 0.04 * (x - 2.0) * (y - 1.5)]);
        }
    }
    Surface::new(bu, bv, &pts).unwrap()
}

/// A cubic NURBS profile over [0, 1] with a knot at 1/2, off the axis of the revolution below.
fn profile() -> Trimmed<3> {
    let b = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let pts = [
        [2.0, 0.0, 0.0],
        [2.5, 0.0, 0.5],
        [2.2, 0.0, 1.0],
        [2.8, 0.0, 1.5],
        [2.4, 0.0, 2.0],
    ];
    Trimmed::whole(Leaf::Nurbs(geom_nurbs::Curve::new(b, &pts).unwrap())).unwrap()
}

/// The creased patch of `creased` with u and v swapped: the crease along v = 1/2.
fn creased_in_v() -> SurfaceKind {
    let kv = vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0];
    let bu = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let bv = Basis::new(3, kv).unwrap();
    let mut pts = Vec::new();
    for i in 0..4 {
        for j in 0..7 {
            let y = f64::from(j) / 6.0 * 2.0;
            let z = if j <= 3 { 0.0 } else { (y - 1.0) * 0.8 };
            pts.push([f64::from(i), y, z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(bu, bv, &pts).unwrap())
}

/// A creased bicubic patch: the knot at u = 1/2 three times, flat on one side and sloped on the
/// other, scaled by `size`.
fn creased(size: f64) -> SurfaceKind {
    let ku = vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0];
    let bu = Basis::new(3, ku).unwrap();
    let bv = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut pts = Vec::new();
    for i in 0..7 {
        for j in 0..4 {
            let x = f64::from(i) / 6.0 * 2.0;
            let z = if i <= 3 { 0.0 } else { (x - 1.0) * 0.8 };
            pts.push([x * size, f64::from(j) * size, z * size]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(bu, bv, &pts).unwrap())
}

/// Whether every enclosure of `a` meets the matching one of `b`.
fn meet_all(a: &[[Interval; 3]], b: &[[Interval; 3]]) -> bool {
    a.iter().zip(b).all(|(x, y)| {
        x.iter()
            .zip(y)
            .all(|(p, q)| q.lo() <= p.hi() && p.lo() <= q.hi())
    })
}

#[test]
fn breaks_are_strictly_inside_and_follow_splits() {
    let m = &mut meter();
    let s = SurfaceKind::Nurbs(knotted());
    assert_eq!(s.breaks(), (vec![0.5], vec![0.5]));
    let line = Trimmed::whole(Leaf::Line(
        Line::new([0.0, 1.0, 0.0], [0.0, 1.0, 2.0], (0.0, 1.0)).unwrap(),
    ))
    .unwrap();
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let swept = [
        (
            SurfaceKind::Extrusion(Extrusion::new(profile(), [0.0, 1.0, 0.0], (0.0, 2.0)).unwrap()),
            (vec![0.5], vec![]),
        ),
        (
            SurfaceKind::Revolution(Revolution::new(profile(), [0.0; 3], f, (0.0, 1.0)).unwrap()),
            (vec![], vec![0.5]),
        ),
        (
            SurfaceKind::Ruled(Ruled::new(profile(), line, (0.0, 1.0)).unwrap()),
            (vec![0.5], vec![]),
        ),
    ];
    for (k, b) in &swept {
        assert_eq!(k.breaks(), *b);
    }
    // An offset keeps the breaks strictly inside its own domain, whatever the split.
    let off = Offset::new(s, 0.2).unwrap();
    for (dir, t, left, right) in [
        (Direction::U, 0.5, (vec![], vec![0.5]), (vec![], vec![0.5])),
        (Direction::V, 0.5, (vec![0.5], vec![]), (vec![0.5], vec![])),
        (
            Direction::U,
            0.3,
            (vec![], vec![0.5]),
            (vec![0.5], vec![0.5]),
        ),
        (
            Direction::V,
            0.7,
            (vec![0.5], vec![0.5]),
            (vec![0.5], vec![]),
        ),
    ] {
        let (l, r) = off.split(dir, t, m).unwrap();
        assert_eq!((l.value.breaks(), r.value.breaks()), (left, right));
    }
}

#[test]
fn a_split_offset_refuses_what_lies_outside_its_own_part() {
    let m = &mut meter();
    let off = Offset::new(SurfaceKind::Nurbs(knotted()), 0.2).unwrap();
    let nan = f64::NAN;
    for (dir, t, keep_right) in [
        (Direction::U, 0.4, false),
        (Direction::U, 0.4, true),
        (Direction::V, 0.6, false),
        (Direction::V, 0.6, true),
    ] {
        let (l, r) = off.split(dir, t, m).unwrap();
        let part = if keep_right { r.value } else { l.value };
        let ((a, b), (c, d)) = part.domain();
        // Past the part's own ends (some inside the base's domain), unordered, or not finite.
        let bad = [
            ((a, b), (c, d + 0.05)),
            ((a, b), (c - 0.05, d)),
            ((a, b + 0.05), (c, d)),
            ((a - 0.05, b), (c, d)),
            ((b, a), (c, d)),
            ((a, b), (d, c)),
            ((nan, b), (c, d)),
            ((a, nan), (c, d)),
            ((a, b), (nan, d)),
            ((a, b), (c, nan)),
        ];
        for region in bad {
            assert_eq!(
                part.bound(region, m),
                Err(CurveError::SpanOutside),
                "{region:?}"
            );
            assert_eq!(
                part.partials_over(region, SIDES, 1, m),
                Err(CurveError::SpanOutside),
                "{region:?}"
            );
        }
    }
    // Reversal keeps a part's own domain, negated: never the base's.
    let (_, r) = off.split(Direction::V, 0.6, m).unwrap();
    assert_eq!(
        r.value.reversed(Direction::V, m).unwrap().value.domain(),
        ((0.0, 1.0), (-1.0, -0.6))
    );
    let (_, r) = off.split(Direction::U, 0.4, m).unwrap();
    assert_eq!(
        r.value.reversed(Direction::U, m).unwrap().value.domain(),
        ((-1.0, -0.4), (0.0, 1.0))
    );
}

#[test]
fn a_regions_pieces_stay_inside_it() {
    let m = &mut meter();
    let off = Offset::new(SurfaceKind::Nurbs(knotted()), 0.2).unwrap();
    // x grows with u, so a bound over u in [0, 0.3] stays short of the points at u = 0.45,
    // though the base's break at 1/2 lies past the region.
    let b = off.bound(((0.0, 0.3), (0.0, 1.0)), m).unwrap();
    let p = off.point(0.45, 0.5, SIDES).unwrap();
    assert!(b.hi[0] < p[0], "{} >= {}", b.hi[0], p[0]);
}

#[test]
fn nurbs_partials_along_a_line_hold_its_points() {
    let m = &mut meter();
    let mut rng = Lcg(0x11e5);
    let kinds = [
        SurfaceKind::Nurbs(knotted()),
        SurfaceKind::Nurbs(patch(
            &mut rng,
            vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
            true,
        )),
    ];
    for s in &kinds {
        // Lines at the start, inside a patch, at a knot and at the end, in each direction.
        for t in [0.0, 0.25, 0.5, 1.0] {
            for (line, at) in [
                (((t, t), (0.1, 0.4)), [(t, 0.1), (t, 0.25), (t, 0.4)]),
                (((0.1, 0.4), (t, t)), [(0.1, t), (0.25, t), (0.4, t)]),
            ] {
                // A line takes its one value from `sides`, as its points do.
                for sides in [SIDES, (Limit::FromLeft, Limit::FromLeft)] {
                    let over = s.partials_over(line, sides, 3, m).unwrap();
                    for (u, v) in at {
                        let p = s.partials_over(((u, u), (v, v)), sides, 3, m).unwrap();
                        assert!(meet_all(&p, &over), "{line:?} at ({u}, {v}) from {sides:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn a_box_across_a_tear_holds_both_sides() {
    let m = &mut meter();
    let off = Offset::new(creased(1.0), 0.6).unwrap();
    for (u0, u1) in [(0.49, 0.51), (0.4, 0.5001)] {
        let over = off
            .partials_over(((u0, u1), (0.3, 0.4)), SIDES, 1, m)
            .unwrap();
        for (u, sides) in [
            (u0, SIDES),
            (0.5, (Limit::FromLeft, Limit::FromRight)),
            (0.5, SIDES),
            (u1, SIDES),
        ] {
            let p = off
                .partials_over(((u, u), (0.35, 0.35)), sides, 1, m)
                .unwrap();
            assert!(meet_all(&p, &over), "({u0}, {u1}) at {u}");
        }
    }
    // The same across a tear in v.
    let off = Offset::new(creased_in_v(), 0.6).unwrap();
    for (v0, v1) in [(0.49, 0.51), (0.4, 0.5001)] {
        let over = off
            .partials_over(((0.3, 0.4), (v0, v1)), SIDES, 1, m)
            .unwrap();
        for (v, sides) in [
            (v0, SIDES),
            (0.5, (Limit::FromRight, Limit::FromLeft)),
            (0.5, SIDES),
            (v1, SIDES),
        ] {
            let p = off
                .partials_over(((0.35, 0.35), (v, v)), sides, 1, m)
                .unwrap();
            assert!(meet_all(&p, &over), "({v0}, {v1}) at {v}");
        }
    }
    // Refining a cache whose certificate cannot fall stops at once: a tear wider than the fit
    // bound keeps the second sampling (8 steps a cell: 2·8 + 1 points across u, 8 + 1 along v),
    // and no knot is removed from it.
    let wide = Offset::new(creased(100.0), 200.0).unwrap();
    let form = wide.to_nurbs(m).unwrap();
    let s = &form.value.surface;
    assert!(form.deviation > 64.0, "{}", form.deviation);
    assert_eq!((s.basis_u().count(), s.basis_v().count()), (17, 9));
}

#[test]
fn an_offset_onto_a_cylinders_axis_decides_nothing_and_fails_nothing() {
    let m = &mut meter();
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    let band = ((0.0, 0.4), (-1.0, 1.0));
    let cyl = SurfaceKind::Cylinder(geom_surface::Cylinder::new([0.0; 3], 2.0, f, band).unwrap());
    let s = away(&cyl, [0.0, 0.3, 0.0], (0.2, 0.3));
    // Moved by the radius towards the axis, the band collapses onto it: the fold factor is 0
    // everywhere, no piece decides a sign, and the cross product of the partials (0 across the
    // turn) bounds the normals.
    let flat = Offset::new(cyl, -2.0 * s).unwrap();
    let n = flat.normal_bound(band, m).unwrap();
    for d in 0..3 {
        assert!(n.lo[d] <= 0.0 && 0.0 <= n.hi[d], "{n:?}");
    }
    let p = flat.point(0.2, 0.5, SIDES).unwrap();
    assert!(p[0].abs() < 1e-12 && p[2].abs() < 1e-12, "{p:?}");
}
