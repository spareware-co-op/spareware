//! The analytic surface kinds (piece G65) under the kind contract, and their own claims: every
//! plane, cylinder, cone, sphere and torus keeps every law under exact, rational, certified,
//! non-uniform and mirroring transforms; a whole cylinder converts exactly; a non-uniform scale
//! changes a kind to NURBS explicitly; a sphere's pole keeps its normal; a torus knows its form.

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

use geom_curve::{CurveError, Frame, ParamMap};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Direction, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::contract::check;
use geom_surface::{
    Cone, Cylinder, ImplicitBound, ParamBox, ParamSurface, Plane, Sphere, SurfaceKind, Torus,
    TorusForm,
};

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

fn transforms() -> Vec<TypedTransform> {
    vec![
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1)),
        TypedTransform::Exact(GridTransform::mirror(Axis::Y)),
        TypedTransform::from(
            RationalTransform::rotation(Axis::Y, Rotation::direction(3, 4).unwrap()).unwrap(),
        ),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap()),
        TypedTransform::from(Transform::uniform_scale(1.5).unwrap()),
    ]
}

fn frame(rng: &mut Lcg) -> Frame<3> {
    loop {
        if let Ok(f) = Frame::new(rng.vector(1.0), rng.vector(1.0)) {
            return f;
        }
    }
}

fn params(((a, b), (c, d)): ParamBox) -> Vec<(f64, f64)> {
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

fn span(rng: &mut Lcg, lo: f64, hi: f64, min: f64) -> (f64, f64) {
    let a = rng.range(lo, hi - min);
    (a, rng.range(a + min, hi))
}

fn kinds(rng: &mut Lcg) -> Vec<SurfaceKind> {
    let c = rng.vector(30.0);
    let plane = Plane::new(
        c,
        rng.vector(5.0),
        rng.vector(5.0),
        (span(rng, -3.0, 3.0, 0.5), span(rng, -3.0, 3.0, 0.5)),
    )
    .unwrap();
    let cylinder = Cylinder::new(
        c,
        rng.range(0.5, 10.0),
        frame(rng),
        (span(rng, -1.0, 1.0, 0.2), span(rng, -5.0, 5.0, 0.5)),
    )
    .unwrap();
    let cone = Cone::new(
        c,
        rng.range(0.5, 5.0),
        rng.range(-0.5, 0.5),
        frame(rng),
        (span(rng, -1.0, 1.0, 0.2), span(rng, -2.0, 2.0, 0.5)),
    )
    .unwrap();
    let sphere = Sphere::new(
        c,
        rng.range(0.5, 10.0),
        frame(rng),
        (span(rng, -1.0, 1.0, 0.2), span(rng, -0.5, 0.5, 0.1)),
    )
    .unwrap();
    let torus = Torus::new(
        c,
        rng.range(3.0, 10.0),
        rng.range(0.5, 2.5),
        frame(rng),
        (span(rng, -1.0, 1.0, 0.2), span(rng, -1.0, 1.0, 0.2)),
    )
    .unwrap();
    vec![
        SurfaceKind::Plane(plane),
        SurfaceKind::Cylinder(cylinder),
        SurfaceKind::Cone(cone),
        SurfaceKind::Sphere(sphere),
        SurfaceKind::Torus(torus),
    ]
}

#[test]
fn every_analytic_kind_keeps_the_contract() {
    let mut rng = Lcg(65);
    let all = transforms();
    for trial in 0..24 {
        for (i, k) in kinds(&mut rng).into_iter().enumerate() {
            let t = &all[(trial + i) % all.len()];
            check(&k, &params(k.domain()), t, &mut meter())
                .unwrap_or_else(|e| panic!("trial {trial}: {k:?} under {t:?}: {e}"));
        }
    }
}

#[test]
fn a_whole_cylinder_converts_exactly() {
    let m = &mut meter();
    let f = Frame::standard().unwrap();
    let c = Cylinder::new([1.0, 2.0, 3.0], 4.0, f, ((0.0, 2.0), (0.0, 5.0))).unwrap();
    let n = c.to_nurbs(m).unwrap();
    assert_eq!(n.deviation, 0.0);
    assert!(matches!(n.value.u, ParamMap::Circular(_)));
    assert_eq!(n.value.v, ParamMap::Same);
    let s = Sphere::new([0.0; 3], 2.0, f, ((0.0, 2.0), (-0.5, 0.5))).unwrap();
    let n = s.to_nurbs(m).unwrap();
    assert!(n.deviation < 1e-13, "{}", n.deviation);
    check(&s, &params(s.domain()), &transforms()[0], m).unwrap();
}

#[test]
fn transforms_change_kinds_explicitly() {
    let m = &mut meter();
    let f = Frame::standard().unwrap();
    let c = Cylinder::new([0.0; 3], 1.0, f, ((0.0, 1.0), (0.0, 1.0))).unwrap();
    let squash = TypedTransform::from(Transform::scale(2.0, 0.5, 1.0).unwrap());
    assert!(matches!(
        c.transformed(&squash, m).unwrap().value,
        SurfaceKind::Affine(_)
    ));
    let grow = TypedTransform::from(Transform::uniform_scale(2.0).unwrap());
    let SurfaceKind::Cylinder(g) = c.transformed(&grow, m).unwrap().value else {
        panic!("a similarity keeps a cylinder")
    };
    assert_eq!(g.radius(), 2.0);
    let mirror = TypedTransform::Exact(GridTransform::mirror(Axis::X));
    let moved = c.transformed(&mirror, m).unwrap();
    assert_eq!(moved.deviation, 0.0);
    let SurfaceKind::Cylinder(r) = moved.value else {
        panic!("a mirror keeps a cylinder")
    };
    assert_eq!(r.axial_sign(), -1.0);
}

#[test]
fn spheres_keep_their_normals_at_the_poles_and_tori_their_forms() {
    let m = &mut meter();
    let f = Frame::standard().unwrap();
    let s = Sphere::new([0.0; 3], 1.0, f, ((0.0, 2.0), (0.25, 0.5))).unwrap();
    let n = s.normal_bound(((0.0, 2.0), (0.45, 0.5)), m).unwrap();
    assert!(n.lo[2] > 0.0, "{n:?}");
    let torus = |big: f64, small: f64, v: (f64, f64)| {
        Torus::new([0.0; 3], big, small, f, ((0.0, 2.0), v))
            .unwrap()
            .form()
            .unwrap()
    };
    assert_eq!(torus(3.0, 1.0, (0.0, 2.0)), TorusForm::Ring);
    assert_eq!(torus(1.0, 1.0, (0.0, 2.0)), TorusForm::Horn);
    // R = 1, r = 2: R + r·cos πv ≥ 0 for |v| ≤ 2/3.
    assert_eq!(torus(1.0, 2.0, (-0.5, 0.5)), TorusForm::Apple);
    assert_eq!(torus(1.0, 2.0, (0.8, 1.2)), TorusForm::Lemon);
    assert_eq!(torus(1.0, 2.0, (0.0, 1.0)), TorusForm::Spindle);
    let lemon = Torus::new([0.0; 3], 1.0, 2.0, f, ((0.0, 2.0), (0.8, 1.2))).unwrap();
    check(&lemon, &params(lemon.domain()), &transforms()[3], m).unwrap();
}

/// A patch that reaches a cone's apex only at its edge keeps its nappe's normals there, the limit
/// from inside (12.10); one across the apex holds both nappes' and so both signs.
#[test]
fn a_cone_keeps_its_normals_up_to_its_apex() {
    let m = &mut meter();
    let f = Frame::standard().unwrap();
    // r₀ = 0, k = 1: the normal's direction is e(u) − n̂, so its z is −1 on the nappe v > 0.
    let tip = Cone::new([0.0; 3], 0.0, 1.0, f, ((0.0, 2.0), (-4.0, 4.0))).unwrap();
    let up = tip.normal_bound(((0.25, 0.5), (0.0, 0.25)), m).unwrap();
    assert!(up.hi[2] < 0.0, "{up:?}");
    let down = tip.normal_bound(((0.25, 0.5), (-0.25, 0.0)), m).unwrap();
    assert!(down.lo[2] > 0.0, "{down:?}");
    let across = tip.normal_bound(((0.25, 0.5), (-0.25, 0.25)), m).unwrap();
    assert!(across.lo[2] < 0.0 && across.hi[2] > 0.0, "{across:?}");
}

#[test]
fn split_and_reverse_are_exact_and_refusals_typed() {
    let m = &mut meter();
    let mut rng = Lcg(66);
    let sides = (Limit::FromRight, Limit::FromRight);
    for k in kinds(&mut rng) {
        for dir in [Direction::U, Direction::V] {
            let r = k.reversed(dir, m).unwrap();
            assert_eq!(r.deviation, 0.0);
            for (u, v) in params(k.domain()) {
                let p = k.point(u, v, sides).unwrap();
                let (ru, rv) = if dir == Direction::U {
                    (-u, v)
                } else {
                    (u, -v)
                };
                let q = r.value.point(ru, rv, sides).unwrap();
                for i in 0..3 {
                    assert!(
                        (p[i] - q[i]).abs() <= 1e-12 * (1.0 + p[i].abs()),
                        "{k:?} {dir:?}"
                    );
                }
            }
        }
    }
    let f = Frame::standard().unwrap();
    let unit = ((0.0, 1.0), (0.0, 1.0));
    assert_eq!(
        Cylinder::new([0.0; 3], 0.0, f, unit).err(),
        Some(CurveError::Degenerate)
    );
    assert_eq!(
        Cylinder::new([0.0; 3], 1.0, f, ((0.0, 2.5), (0.0, 1.0))).err(),
        Some(CurveError::SpanOutside)
    );
    assert_eq!(
        Sphere::new([0.0; 3], 1.0, f, ((0.0, 1.0), (0.0, 0.75))).err(),
        Some(CurveError::SpanOutside)
    );
    assert_eq!(
        Plane::new([0.0; 3], [1.0, 2.0, 3.0], [2.0, 4.0, 6.0], unit).err(),
        Some(CurveError::Degenerate)
    );
    let infinite = Plane::new(
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        ((f64::NEG_INFINITY, f64::INFINITY), (0.0, 1.0)),
    )
    .unwrap();
    assert_eq!(infinite.to_nurbs(m).err(), Some(CurveError::Unbounded));
}

/// A cone's equation vanishes on it, which a slope taken from anything but its own would not.
#[test]
fn a_cones_equation_vanishes_on_it() {
    let domain = ((0.25, 1.75), (-0.5, 0.5));
    let cone = Cone::new(
        [1.0, 2.0, 3.0],
        5.0,
        0.25,
        Frame::standard().unwrap(),
        domain,
    )
    .unwrap();
    let surface = SurfaceKind::Cone(cone.clone());
    let sides = (Limit::FromRight, Limit::FromRight);
    for (u, v) in [(0.5, 0.25), (1.0, -0.25), (1.5, 0.5)] {
        let [x, y, z] = surface.point(u, v, sides).unwrap();
        let at = [x, y, z].map(|c| Interval::point(c).unwrap());
        let value = cone.enclose_box(at).unwrap().value;
        assert!(
            value.lo().abs() < 1e-9 && value.hi().abs() < 1e-9,
            "{value:?}"
        );
    }
}
