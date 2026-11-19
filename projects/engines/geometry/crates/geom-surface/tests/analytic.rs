//! The analytic surface kinds (piece G65) under the kind contract, and their own claims: every
//! plane, cylinder, cone, sphere and torus keeps every law under exact, rational, certified,
//! non-uniform and mirroring transforms; a whole cylinder converts exactly; a non-uniform scale
//! changes a kind to NURBS explicitly; a uniform whole-number scale moves every round kind exactly,
//! a cylinder's and a cone's axial term with it; a sphere's pole keeps its normal; a torus knows
//! its form.
//!
//! Covers: `SurfaceKind::Plane`, `SurfaceKind::Cylinder`, `SurfaceKind::Cone`,
//! `SurfaceKind::Sphere`, `SurfaceKind::Torus`.

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
fn a_uniform_whole_scale_moves_every_round_kind_exactly() {
    // m·p + t for whole m and t: twice and three times about points off the origin, and three times
    // with a mirror. The image of each (u, v) is the moved kind's point there, so the deviation is
    // the map's, 0, and not the distance a kept parameter would make: (m − 1)·v along the axis of a
    // cylinder or a cone, and for a cone the slope too.
    let m = &mut meter();
    // On the standard frame every double is exact, and so is the image: 0. On a turned one the
    // frame's own doubles round, a few ulps of the lengths.
    for (f, within) in [
        (Frame::standard().unwrap(), 0.0),
        (Frame::new([0.6, 0.8, 0.0], [0.0, 0.0, 1.0]).unwrap(), 1e-12),
    ] {
        let grid = |k: i64, mirrored: bool, t: [i128; 3]| {
            let x = if mirrored { -k } else { k };
            TypedTransform::from(
                RationalTransform::new([[x, 0, 0], [0, k, 0], [0, 0, k]], t, 1).unwrap(),
            )
        };
        let scales = [
            (grid(2, false, [5, -7, 11]), 2.0),
            (grid(3, false, [0, 0, 0]), 3.0),
            (grid(3, true, [-4, 9, 2]), -3.0),
        ];
        let kinds: Vec<SurfaceKind> = vec![
            SurfaceKind::Cylinder(
                Cylinder::new([1.0, 2.0, 3.0], 5.0, f, ((0.0, 2.0), (-1.0, 10.0))).unwrap(),
            ),
            SurfaceKind::Cone(
                Cone::new([1.0, 2.0, 3.0], 6.0, -0.5, f, ((0.0, 2.0), (0.0, 12.0))).unwrap(),
            ),
            SurfaceKind::Sphere(
                Sphere::new([1.0, 2.0, 3.0], 5.0, f, ((0.0, 2.0), (-0.5, 0.5))).unwrap(),
            ),
            SurfaceKind::Torus(
                Torus::new([1.0, 2.0, 3.0], 7.0, 2.0, f, ((0.0, 2.0), (-1.0, 1.0))).unwrap(),
            ),
        ];
        for k in &kinds {
            for (t, factor) in scales.iter().map(|(t, f): &(TypedTransform, f64)| (t, *f)) {
                let moved = k.transformed(t, m).unwrap();
                assert!(
                    moved.deviation <= within,
                    "{k:?} under {t:?}: {}",
                    moved.deviation
                );
                check(k, &params(k.domain()), t, m)
                    .unwrap_or_else(|e| panic!("{k:?} under {t:?}: {e}"));
                // The axial factor takes the scale where the axial term is a length.
                let axial = match (&moved.value, k) {
                    (SurfaceKind::Cylinder(c), SurfaceKind::Cylinder(_)) => c.axial_sign(),
                    (SurfaceKind::Cone(c), SurfaceKind::Cone(o)) => {
                        let ((r, slope), (r0, k0)) = (c.radius_and_slope(), o.radius_and_slope());
                        assert_eq!((r, slope), (factor.abs() * r0, factor.abs() * k0));
                        c.axial_sign()
                    }
                    // A sphere's and a torus's is r·sin πv: the radius carries the scale, a mirror
                    // the sign.
                    (SurfaceKind::Sphere(c), SurfaceKind::Sphere(_)) => {
                        c.axial_sign() * factor.abs()
                    }
                    (SurfaceKind::Torus(c), SurfaceKind::Torus(_)) => c.axial_sign() * factor.abs(),
                    other => panic!("a similarity keeps the kind: {other:?}"),
                };
                assert_eq!(axial, factor, "{k:?} under {t:?}");
            }
        }
        // Moved twice, a cylinder's and a cone's axial factor is the product of the scales: after
        // the first it is no longer ±1, so the second multiplies it, and the twice-moved kind's
        // points are the second map's images of the once-moved one's.
        for k in &kinds[..2] {
            let once = k.transformed(&scales[0].0, m).unwrap().value;
            let twice = once.transformed(&scales[2].0, m).unwrap().value;
            let axial = match &twice {
                SurfaceKind::Cylinder(c) => c.axial_sign(),
                SurfaceKind::Cone(c) => c.axial_sign(),
                other => panic!("a similarity keeps the kind: {other:?}"),
            };
            assert_eq!(axial, -6.0, "{k:?}");
            check(&once, &params(once.domain()), &scales[2].0, m)
                .unwrap_or_else(|e| panic!("{once:?} moved again: {e}"));
        }
        // A scale that is not uniform still gives the moved NURBS form, certified.
        let squash = TypedTransform::from(
            RationalTransform::new([[2, 0, 0], [0, 1, 0], [0, 0, 1]], [0; 3], 1).unwrap(),
        );
        for k in &kinds {
            let moved = k.transformed(&squash, m).unwrap();
            assert!(matches!(moved.value, SurfaceKind::Affine(_)), "{k:?}");
            check(k, &params(k.domain()), &squash, m).unwrap_or_else(|e| panic!("{k:?}: {e}"));
        }
    }
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

/// The same with the radius falling to the apex, r₀ = 0 and k = −1: the normal's direction is
/// sign(−v)·(e(u) + n̂), so its z is +1 on the nappe v < 0, which reaches the apex at its upper
/// edge, and −1 on v > 0, which reaches it at its lower edge. A patch that is only the apex's
/// line, v = 0, has a radius that is zero throughout and so no sign of its own: the cone goes on
/// past it on both sides, so its bound holds both nappes' normals, z = +1 and z = −1, for a rising
/// cone as for a falling one.
#[test]
fn a_falling_cone_keeps_its_normals_up_to_its_apex() {
    let m = &mut meter();
    let f = Frame::standard().unwrap();
    let fall = Cone::new([0.0; 3], 0.0, -1.0, f, ((0.0, 2.0), (-4.0, 4.0))).unwrap();
    let below = fall.normal_bound(((0.25, 0.5), (-0.25, 0.0)), m).unwrap();
    assert!(below.lo[2] > 0.0, "{below:?}");
    let above = fall.normal_bound(((0.25, 0.5), (0.0, 0.25)), m).unwrap();
    assert!(above.hi[2] < 0.0, "{above:?}");
    let rise = Cone::new([0.0; 3], 0.0, 1.0, f, ((0.0, 2.0), (-4.0, 4.0))).unwrap();
    for cone in [&fall, &rise] {
        let apex = cone.normal_bound(((0.25, 0.5), (0.0, 0.0)), m).unwrap();
        assert!(apex.lo[2] <= -1.0 && apex.hi[2] >= 1.0, "{apex:?}");
    }
}

/// A horn torus, R = r = 1, pinches to its centre at v = 1, where R + r·cos πv is exactly 0. A
/// patch that is only that pinch, (u, v) = (0, 1), has a radius that is zero throughout, which
/// gives no sign, so its normal bound holds both directions there, ±s·cos πv·e(u): x = ±1.
#[test]
fn a_horn_torus_at_its_pinch_holds_both_normals() {
    let m = &mut meter();
    let f = Frame::standard().unwrap();
    let horn = Torus::new([0.0; 3], 1.0, 1.0, f, ((0.0, 2.0), (0.0, 2.0))).unwrap();
    let pinch = horn.normal_bound(((0.0, 0.0), (1.0, 1.0)), m).unwrap();
    assert!(pinch.lo[0] <= -1.0 && pinch.hi[0] >= 1.0, "{pinch:?}");
}

/// A similarity multiplies a cone's axial factor s by its scale, so the cone climbs |s| along n̂
/// for each unit of v and its radius grows by σ = k/s for each unit of height: its equation
/// |w|² − (r₀ + σ·h)² vanishes on it and its Hessian is 2·I − 2·(1 + σ²)·n̂n̂ᵀ. A cone scaled by 2
/// (r₀ = 2, k = 2, s = 2, so σ = 1) and one built with s = 2 (r₀ = 1, k = 1, σ = 1/2), on the
/// standard frame, where every one of these is exact. The slope read as k·s instead missed both.
#[test]
fn a_scaled_cones_equation_takes_its_slope_per_unit_of_height() {
    let m = &mut meter();
    let f = Frame::standard().unwrap();
    let cone = Cone::new([0.0; 3], 1.0, 1.0, f, ((0.0, 2.0), (-0.5, 2.0))).unwrap();
    let twice = TypedTransform::from(
        RationalTransform::new([[2, 0, 0], [0, 2, 0], [0, 0, 2]], [0; 3], 1).unwrap(),
    );
    let SurfaceKind::Cone(scaled) = SurfaceKind::Cone(cone.clone())
        .transformed(&twice, m)
        .unwrap()
        .value
    else {
        panic!("a similarity keeps the kind")
    };
    assert_eq!(scaled.axial_sign(), 2.0);
    let stretched = cone.with_axial_sign(2.0).unwrap();
    let sides = (Limit::FromRight, Limit::FromRight);
    for (c, sigma) in [(&scaled, 1.0), (&stretched, 0.5)] {
        let surface = SurfaceKind::Cone(c.clone());
        for (u, v) in [(0.0, 1.0), (0.5, 2.0), (1.0, -0.5), (1.5, 0.25)] {
            let at = surface
                .point(u, v, sides)
                .unwrap()
                .map(|x| Interval::point(x).unwrap());
            let value = c.enclose_box(at).unwrap().value;
            assert!(
                value.lo().abs() < 1e-9 && value.hi().abs() < 1e-9,
                "{c:?} at ({u}, {v}): {value:?}"
            );
        }
        let region = [Interval::new(-1.0, 1.0).unwrap(); 3];
        let h = c.hessian_box(region).unwrap().expect("a cone has one");
        let along = 2.0 - 2.0 * (1.0 + sigma * sigma);
        for (i, row) in h.iter().enumerate() {
            for (j, entry) in row.iter().enumerate() {
                let want = match (i, j) {
                    (2, 2) => along,
                    (0, 0) | (1, 1) => 2.0,
                    _ => 0.0,
                };
                assert!(
                    entry.contains(want) && entry.hi() - entry.lo() < 1e-12,
                    "{c:?} [{i}][{j}]: {entry:?} against {want}"
                );
            }
        }
    }
}

/// A sphere's and a torus's routines read them as round, so their axial factor is 1 or −1 only:
/// 2 would be a spheroid or a stretched torus, and -3 one turned over too. A cylinder and a cone
/// honour any finite nonzero factor (their axial term is a length), so they take both.
#[test]
fn a_sphere_or_torus_takes_an_axial_factor_of_one_or_minus_one_only() {
    let f = Frame::standard().unwrap();
    let sphere = Sphere::new([1.0, 0.0, 0.0], 3.0, f, ((0.0, 2.0), (-0.5, 0.5))).unwrap();
    let torus = Torus::new([0.0; 3], 3.0, 1.0, f, ((0.0, 2.0), (0.0, 2.0))).unwrap();
    for bad in [2.0, -3.0] {
        assert_eq!(
            sphere.with_axial_sign(bad).map(|_| ()),
            Err(CurveError::AxialFactorNotUnit),
            "{bad}"
        );
        assert_eq!(
            torus.with_axial_sign(bad).map(|_| ()),
            Err(CurveError::AxialFactorNotUnit),
            "{bad}"
        );
    }
    for good in [1.0, -1.0] {
        assert_eq!(sphere.with_axial_sign(good).unwrap().axial_sign(), good);
        assert_eq!(torus.with_axial_sign(good).unwrap().axial_sign(), good);
    }
    let cylinder = Cylinder::new([0.0; 3], 2.0, f, ((0.0, 2.0), (-1.0, 1.0))).unwrap();
    let cone = Cone::new([0.0; 3], 2.0, 0.5, f, ((0.0, 2.0), (-1.0, 1.0))).unwrap();
    for factor in [2.0, -3.0] {
        assert_eq!(
            cylinder.with_axial_sign(factor).unwrap().axial_sign(),
            factor
        );
        assert_eq!(cone.with_axial_sign(factor).unwrap().axial_sign(), factor);
    }
    let why = CurveError::AxialFactorNotUnit.to_string();
    assert!(why.contains("1 or -1") && why.contains("round"), "{why}");
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

/// Each analytic kind's ∇²F holds the change of its gradient across a box: ∇F(y) − ∇F(x) lies in
/// ∇²F(box)·(y − x), the mean value theorem, so the two enclosures of that change must meet. At
/// random points, on a tilted frame, for every kind (G174's tangent tubes read these).
#[test]
fn each_kinds_hessian_holds_its_gradients_change() {
    let f = Frame::new([1.0, 0.5, 0.25], [0.25, -0.5, 1.0]).unwrap();
    let turn = (0.0, 2.0);
    let kinds: Vec<Box<dyn ImplicitBound>> = vec![
        Box::new(
            Plane::new(
                [0.5, -1.0, 2.0],
                [1.0, 0.25, 0.0],
                [0.0, 1.0, 0.5],
                ((-9.0, 9.0), (-9.0, 9.0)),
            )
            .unwrap(),
        ),
        Box::new(Sphere::new([0.5, -1.0, 2.0], 3.0, f, (turn, (-0.5, 0.5))).unwrap()),
        Box::new(Cylinder::new([0.5, -1.0, 2.0], 3.0, f, (turn, (-4.0, 4.0))).unwrap()),
        Box::new(Cone::new([0.5, -1.0, 2.0], 1.0, 0.75, f, (turn, (-2.0, 2.0))).unwrap()),
        Box::new(Torus::new([0.5, -1.0, 2.0], 5.0, 1.5, f, (turn, turn)).unwrap()),
    ];
    let mut rng = Lcg(0x5eed_0174);
    let point = |x: [f64; 3]| x.map(|t| Interval::point(t).unwrap());
    for kind in &kinds {
        for _ in 0..200 {
            let x = [
                rng.range(-6.0, 6.0),
                rng.range(-6.0, 6.0),
                rng.range(-6.0, 6.0),
            ];
            let d = [
                rng.range(-0.25, 0.25),
                rng.range(-0.25, 0.25),
                rng.range(-0.25, 0.25),
            ];
            let y = [x[0] + d[0], x[1] + d[1], x[2] + d[2]];
            let step = [0, 1, 2].map(|k| y[k] - x[k]);
            let region = [0, 1, 2].map(|k| {
                let (lo, hi) = if x[k] < y[k] {
                    (x[k], y[k])
                } else {
                    (y[k], x[k])
                };
                Interval::new(lo, hi).unwrap()
            });
            let hessian = kind
                .hessian_box(region)
                .unwrap()
                .expect("every analytic kind has one");
            let (gx, gy) = (
                kind.enclose_box(point(x)).unwrap().gradient,
                kind.enclose_box(point(y)).unwrap().gradient,
            );
            for i in 0..3 {
                let change = gy[i].sub(gx[i]).unwrap();
                let mut bound = Interval::point(0.0).unwrap();
                for j in 0..3 {
                    let term = hessian[i][j]
                        .mul(Interval::point(step[j]).unwrap())
                        .unwrap();
                    bound = bound.add(term).unwrap();
                }
                assert!(
                    change.lo() <= bound.hi() && bound.lo() <= change.hi(),
                    "{i}: {change:?} against {bound:?} at {x:?} + {d:?}"
                );
            }
        }
    }
}
