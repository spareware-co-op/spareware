//! The surface kind contract (piece G60) on the NURBS kind, a pole, a kind broken on purpose, and
//! the `ImplicitBound` of an exact plane: every random NURBS surface keeps every law under exact,
//! rational and certified transforms; a cone's apex keeps its normals' direction; a wrong normal
//! bound is caught; and the plane's enclosure holds the exact value at every corner.
//!
//! Covers: `SurfaceKind::Nurbs`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::many_single_char_names,
    clippy::cast_possible_wrap,
    clippy::needless_range_loop,
    reason = "tests: a failure stops the test"
)]

use geom_curve::affine::int_interval;
use geom_curve::{CurveError, DerivativeBound, Law};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_num::grid::P3;
use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_nurbs::{Basis, Bounds, Changed, Direction, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::contract::check;
use geom_surface::{ImplicitBound, ParamBox, ParamSurface, SurfaceForm, SurfaceKind};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

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
    fn below(&mut self, n: usize) -> usize {
        ((self.next() >> 33) % n as u64) as usize
    }
}

fn knots(rng: &mut Lcg, p: usize) -> Vec<f64> {
    let mut k = vec![0.0; p + 1];
    let mut x = 0.0;
    for _ in 0..rng.below(3) {
        x += 0.25 + rng.unit();
        k.push(x);
    }
    x += 0.5 + rng.unit();
    k.extend(std::iter::repeat_n(x, p + 1));
    k
}

fn surface(rng: &mut Lcg) -> Surface<3> {
    let (p, q) = (1 + rng.below(3), 1 + rng.below(3));
    let bu = Basis::new(p, knots(rng, p)).unwrap();
    let bv = Basis::new(q, knots(rng, q)).unwrap();
    let n = bu.count() * bv.count();
    let pts: Vec<[f64; 3]> = (0..n)
        .map(|_| [0, 1, 2].map(|_| rng.unit() * 100.0 - 50.0))
        .collect();
    if rng.below(2) == 0 {
        Surface::new(bu, bv, &pts).unwrap()
    } else {
        let w: Vec<f64> = (0..n).map(|_| 0.5 + rng.unit() * 2.0).collect();
        Surface::new_rational(bu, bv, &pts, &w).unwrap()
    }
}

fn transforms() -> Vec<TypedTransform> {
    vec![
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1)),
        TypedTransform::from(
            RationalTransform::rotation(Axis::Y, Rotation::direction(3, 4).unwrap()).unwrap(),
        ),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap()),
    ]
}

fn params(s: &Surface<3>) -> Vec<(f64, f64)> {
    let ((a, b), (c, d)) = ParamSurface::domain(s);
    let mut out = Vec::new();
    for i in 0..=4 {
        for j in 0..=4 {
            out.push((
                (a + (b - a) * f64::from(i) / 4.0).clamp(a, b),
                (c + (d - c) * f64::from(j) / 4.0).clamp(c, d),
            ));
        }
    }
    out
}

#[test]
fn every_nurbs_surface_keeps_the_contract() {
    let mut rng = Lcg(65);
    let all = transforms();
    for trial in 0..40 {
        let s = surface(&mut rng);
        let t = &all[trial % all.len()];
        check(&s, &params(&s), t, &mut meter()).unwrap_or_else(|e| panic!("trial {trial}: {e}"));
        let kind = SurfaceKind::Nurbs(s.clone());
        check(&kind, &params(&s), t, &mut meter())
            .unwrap_or_else(|e| panic!("trial {trial} (enum): {e}"));
    }
}

/// A cone patch whose u = 0 edge is its apex, rational or not.
fn apex(rational: bool) -> Surface<3> {
    let bu = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let bv = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let a = [0.0, 0.0, 8.0];
    let pts = [a, a, a, [8.0, 0.0, 0.0], [8.0, 8.0, 0.0], [0.0, 8.0, 0.0]];
    if rational {
        Surface::new_rational(bu, bv, &pts, &[1.0, 2.0, 4.0, 1.0, 0.5, 1.0]).unwrap()
    } else {
        Surface::new(bu, bv, &pts).unwrap()
    }
}

#[test]
fn a_pole_keeps_its_normals_direction() {
    for rational in [false, true] {
        let s = apex(rational);
        let m = &mut meter();
        let n = s.normal_bound(ParamSurface::domain(&s), m).unwrap();
        assert!(n.lo[2] > 0.0, "rational {rational}: {n:?}");
        check(&s, &params(&s), &transforms()[0], m).unwrap();
    }
    let s = apex(false);
    let m = &mut meter();
    let (left, right) = ParamSurface::split(&s, Direction::V, 0.5, m).unwrap();
    assert_eq!(left.value.domain().1, (0.0, 0.5));
    assert_eq!(right.value.domain().1, (0.5, 1.0));
    let r = s.reversed(Direction::V, m).unwrap();
    assert_eq!(r.deviation, 0.0);
    assert_eq!(r.value.domain().1, (-1.0, 0.0));
    assert_eq!(
        ParamSurface::bound(&s, ((0.5, 0.2), (0.0, 1.0)), m),
        Err(CurveError::SpanOutside)
    );
}

/// A NURBS surface whose normal bound has the wrong sign.
struct FlippedNormals(Surface<3>);

impl ParamSurface for FlippedNormals {
    fn domain(&self) -> ParamBox {
        ParamSurface::domain(&self.0)
    }
    fn point(&self, u: f64, v: f64, s: (Limit, Limit)) -> Result<[f64; 3], CurveError> {
        ParamSurface::point(&self.0, u, v, s)
    }
    fn partials(&self, u: f64, v: f64, s: (Limit, Limit)) -> Result<[[f64; 3]; 3], CurveError> {
        self.0.partials(u, v, s)
    }
    fn enclose(
        &self,
        u: f64,
        v: f64,
        s: (Limit, Limit),
        m: &mut WorkMeter<'_>,
    ) -> Result<[[Interval; 3]; 3], CurveError> {
        self.0.enclose(u, v, s, m)
    }
    fn bound(&self, r: ParamBox, m: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        ParamSurface::bound(&self.0, r, m)
    }
    fn derivative_bound(
        &self,
        r: ParamBox,
        d: Direction,
        m: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<3>, CurveError> {
        ParamSurface::derivative_bound(&self.0, r, d, m)
    }
    fn partials_over(
        &self,
        r: ParamBox,
        s: (Limit, Limit),
        k: usize,
        m: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 3]>, CurveError> {
        self.0.partials_over(r, s, k, m)
    }
    fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
        self.0.breaks()
    }
    fn normal_bound(&self, r: ParamBox, m: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        let b = self.0.normal_bound(r, m)?;
        Ok(Bounds {
            lo: b.hi.map(|x| -x),
            hi: b.lo.map(|x| -x),
        })
    }
    fn to_nurbs(&self, m: &mut WorkMeter<'_>) -> Result<Changed<SurfaceForm>, CurveError> {
        self.0.to_nurbs(m)
    }
    fn split(
        &self,
        d: Direction,
        t: f64,
        m: &mut WorkMeter<'_>,
    ) -> Result<(Changed<SurfaceKind>, Changed<SurfaceKind>), CurveError> {
        ParamSurface::split(&self.0, d, t, m)
    }
    fn reversed(
        &self,
        d: Direction,
        m: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        self.0.reversed(d, m)
    }
    fn transformed(
        &self,
        t: &TypedTransform,
        m: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        self.0.transformed(t, m)
    }
}

#[test]
fn a_wrong_normal_bound_is_caught() {
    let s = apex(false);
    let e = check(
        &FlippedNormals(s.clone()),
        &params(&s),
        &transforms()[0],
        &mut meter(),
    )
    .unwrap_err();
    assert_eq!(e.law, Law::NormalBoundHoldsDirections, "{e}");
}

#[test]
fn a_plane_bounds_its_equation_over_any_box() {
    const M: i64 = (1 << 40) - 1;
    let p3 = |x, y, z| P3::new(x, y, z).unwrap();
    let mut rng = Lcg(12);
    for trial in 0..200 {
        let mut r = || {
            if trial < 20 {
                if rng.below(2) == 0 { M } else { -M }
            } else {
                rng.below(2001) as i64 - 1000
            }
        };
        let (a, b, c) = (p3(r(), r(), r()), p3(r(), r(), r()), p3(r(), r(), r()));
        let Ok(plane) = Plane::through(a, b, c) else {
            continue;
        };
        let lo = [r(), r(), r()];
        let hi = lo.map(|x| x.saturating_add(rng.below(500) as i64).min(M));
        let region = [0, 1, 2].map(|d| Interval::new(lo[d] as f64, hi[d] as f64).unwrap());
        let e = plane.enclose_box(region).unwrap();
        assert!(e.smooth);
        for corner in 0..8 {
            let x = [0, 1, 2].map(|d| if corner >> d & 1 == 0 { lo[d] } else { hi[d] });
            let n = plane.n();
            let exact: i128 = n[0] * i128::from(x[0])
                + n[1] * i128::from(x[1])
                + n[2] * i128::from(x[2])
                + plane.d();
            // The exact value is an integer below 2^126, held exactly by an interval.
            let enc = int_interval(exact).unwrap();
            assert!(
                enc.lo() <= e.value.hi() && e.value.lo() <= enc.hi(),
                "trial {trial} corner {corner}"
            );
            for d in 0..3 {
                assert!(e.gradient[d].lo() <= n[d] as f64 && n[d] as f64 <= e.gradient[d].hi());
            }
        }
    }
}
