//! Surface extension (piece G253; chapter 13.9), as laws: every analytic kind extends to the same
//! kind on the wider domain and stops at its turn, pole and apex; NURBS extend by the natural
//! continuation geom-nurbs builds; swept kinds and offsets by their definitions; a fold at a cusp
//! is refused with the length reached, or stopped there when asked; random spans extend, keep
//! their source and pass the normal certificate.
//!
//! Seeds: each seeded law prints `seed: <hex>` and replays with `SPAREKIT_SEED`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    reason = "tests: native arithmetic and indexing, the formulas' names, and a failure stops the test"
)]

use geom_curve::{Basis as Leaf, CurveError, Frame, Line, Trimmed};
use geom_math::{Transform, TypedTransform};
use geom_nurbs::{Basis, Curve, Direction, End, Limit, NurbsError, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::extend::extend;
use geom_surface::{
    Cone, Cylinder, ExtendError, Extrusion, Offset, ParamBox, ParamSurface, Plane, Reach,
    Revolution, Side, Sphere, SurfaceKind, Torus,
};
use geom_test::{Seed, SplitMix64};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

struct Rng(SplitMix64);

impl Rng {
    fn new(label: &str, default: u64) -> Rng {
        Rng(SplitMix64::new(
            geom_test::seed::resolve(label, Seed(default)).unwrap(),
        ))
    }

    fn real(&mut self, lo: f64, hi: f64) -> f64 {
        let u = (self.0.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        lo + (hi - lo) * u
    }

    fn direction(&mut self) -> [f64; 3] {
        loop {
            let v = [
                self.real(-1.0, 1.0),
                self.real(-1.0, 1.0),
                self.real(-1.0, 1.0),
            ];
            let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if n > 0.2 {
                return [v[0] / n, v[1] / n, v[2] / n];
            }
        }
    }
}

const SIDES: [Side; 4] = [Side::UMin, Side::UMax, Side::VMin, Side::VMax];

fn grown(d: ParamBox, side: Side, len: f64) -> ParamBox {
    let ((u0, u1), (v0, v1)) = d;
    match side {
        Side::UMin => ((u0 - len, u1), (v0, v1)),
        Side::UMax => ((u0, u1 + len), (v0, v1)),
        Side::VMin => ((u0, u1), (v0 - len, v1)),
        Side::VMax => ((u0, u1), (v0, v1 + len)),
    }
}

fn exact(s: &SurfaceKind, side: Side, by: f64) -> geom_surface::Extended {
    extend(s, side, by, Reach::Exactly, &mut meter())
        .unwrap_or_else(|e| panic!("{side:?} by {by}: {e}"))
}

#[test]
fn analytic_kinds_extend_to_the_same_kind_on_the_wider_domain() {
    let mut rng = Rng::new("extend: analytic", 0x2530_0001);
    for _ in 0..12 {
        let c = [
            rng.real(-3.0, 3.0),
            rng.real(-3.0, 3.0),
            rng.real(-3.0, 3.0),
        ];
        let frame = Frame::new(rng.direction(), rng.direction()).unwrap();
        let (u0, v0) = (rng.real(-1.0, 1.0), rng.real(-0.2, 0.0));
        let u = (u0, u0 + rng.real(0.2, 1.0));
        let by = rng.real(0.05, 0.3);
        let (r, k) = (rng.real(1.0, 2.0), rng.real(0.1, 0.5));
        let makers: Vec<Box<dyn Fn(ParamBox) -> SurfaceKind>> = vec![
            Box::new(move |d| {
                SurfaceKind::Plane(Plane::new(c, frame.x(), frame.normal(), d).unwrap())
            }),
            Box::new(move |d| SurfaceKind::Cylinder(Cylinder::new(c, r, frame, d).unwrap())),
            Box::new(move |d| SurfaceKind::Cone(Cone::new(c, r, k, frame, d).unwrap())),
            Box::new(move |d| SurfaceKind::Sphere(Sphere::new(c, r, frame, d).unwrap())),
            Box::new(move |d| SurfaceKind::Torus(Torus::new(c, 2.0 * r, r, frame, d).unwrap())),
        ];
        let domain = (u, (v0, v0 + 0.15));
        for make in &makers {
            let s = make(domain);
            for side in SIDES {
                let e = exact(&s, side, by);
                assert_eq!(e.surface, make(grown(domain, side, by)), "{side:?}");
                assert_eq!((e.reached, e.deviation), (by, 0.0));
            }
        }
    }
}

#[test]
fn analytic_kinds_stop_at_a_turn_a_pole_and_an_apex() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    // A cylinder 1.5 half-turns wide reaches half a turn more in u, no further.
    let cyl = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 1.0, frame, ((0.0, 1.5), (0.0, 1.0))).unwrap(),
    );
    match extend(&cyl, Side::UMax, 1.0, Reach::Exactly, &mut meter()) {
        Err(ExtendError::ExtensionFailed {
            side: Side::UMax,
            reached,
        }) => {
            assert!((reached - 0.5).abs() < 1e-12, "{reached}");
        }
        other => panic!("{other:?}"),
    }
    let far = extend(&cyl, Side::UMax, 1.0, Reach::AsFarAsValid, &mut meter()).unwrap();
    assert!((far.reached - 0.5).abs() < 1e-12);
    let ((a, b), _) = far.surface.domain();
    assert!(b - a <= 2.0 && b - a > 2.0 - 1e-12);
    // A sphere reaches its pole, no further.
    let sph = SurfaceKind::Sphere(
        Sphere::new([0.0; 3], 1.0, frame, ((0.0, 1.0), (-0.25, 0.25))).unwrap(),
    );
    let pole = extend(&sph, Side::VMax, 1.0, Reach::AsFarAsValid, &mut meter()).unwrap();
    assert!((pole.reached - 0.25).abs() < 1e-12);
    assert!(pole.surface.domain().1.1 <= 0.5);
    // A cone r = 1 + v reaches its apex at v = −1, never past it.
    let cone =
        SurfaceKind::Cone(Cone::new([0.0; 3], 1.0, 1.0, frame, ((0.0, 1.0), (0.0, 1.0))).unwrap());
    match extend(&cone, Side::VMin, 2.0, Reach::Exactly, &mut meter()) {
        Err(ExtendError::ExtensionFailed { reached, .. }) => {
            assert!(reached <= 1.0 && reached > 1.0 - 1e-12, "{reached}");
        }
        other => panic!("{other:?}"),
    }
    let tip = extend(&cone, Side::VMin, 2.0, Reach::AsFarAsValid, &mut meter()).unwrap();
    let v_new = tip.surface.domain().1.0;
    assert!(
        v_new >= -1.0 && 1.0 + v_new >= 0.0,
        "past the apex: {v_new}"
    );
    // Away from the apex the cone extends freely.
    exact(&cone, Side::VMax, 5.0);
    // A spindle torus (minor radius above the major) stops where R + r·cos πv vanishes.
    let spindle = SurfaceKind::Torus(
        Torus::new([0.0; 3], 1.0, 2.0, frame, ((0.0, 1.0), (-0.1, 0.1))).unwrap(),
    );
    let stop = extend(&spindle, Side::VMax, 1.0, Reach::AsFarAsValid, &mut meter()).unwrap();
    // cos πv = −1/2 at v = 2/3.
    let v_end = stop.surface.domain().1.1;
    assert!(v_end <= 2.0 / 3.0 && v_end > 2.0 / 3.0 - 1e-9, "{v_end}");
}

/// A clamped bicubic NURBS graph z = f(x, y): its normal's z component never vanishes, near
/// its domain or past it.
fn graph(rng: &mut Rng, spans: usize, rational: bool) -> Surface<3> {
    let knots = |n: usize| -> Vec<f64> {
        let mut k = vec![0.0; 4];
        for i in 1..n {
            k.push(i as f64);
        }
        k.extend([n as f64; 4]);
        k
    };
    let bu = Basis::new(3, knots(spans)).unwrap();
    let bv = Basis::new(3, knots(2)).unwrap();
    let (rows, cols) = (bu.count(), bv.count());
    let mut points = Vec::new();
    for i in 0..rows {
        for j in 0..cols {
            points.push([i as f64, j as f64, rng.real(-0.1, 0.1)]);
        }
    }
    if rational {
        let w: Vec<f64> = (0..rows * cols).map(|_| rng.real(0.95, 1.05)).collect();
        Surface::new_rational(bu, bv, &points, &w).unwrap()
    } else {
        Surface::new(bu, bv, &points).unwrap()
    }
}

#[test]
fn nurbs_extend_by_the_natural_continuation_and_keep_their_source() {
    let mut rng = Rng::new("extend: nurbs", 0x2530_0002);
    for round in 0..16 {
        let s = graph(&mut rng, 1 + round % 4, round % 2 == 1);
        let kind = SurfaceKind::Nurbs(s.clone());
        for side in SIDES {
            let by = rng.real(0.05, if round % 2 == 1 { 0.2 } else { 1.0 });
            let e = exact(&kind, side, by);
            let want = s
                .extend(side.direction(), side.end(), by, &mut meter())
                .unwrap();
            assert_eq!(e.surface, SurfaceKind::Nurbs(want.value));
            assert_eq!(e.deviation, want.deviation);
            // Over the source's domain, the same points.
            let ((u0, u1), (v0, v1)) = s.domain();
            for i in 0..=4 {
                for j in 0..=4 {
                    let (u, v) = (
                        u0 + (u1 - u0) * f64::from(i) / 4.0,
                        v0 + (v1 - v0) * f64::from(j) / 4.0,
                    );
                    let sides = (
                        if u == u1 {
                            Limit::FromLeft
                        } else {
                            Limit::FromRight
                        },
                        if v == v1 {
                            Limit::FromLeft
                        } else {
                            Limit::FromRight
                        },
                    );
                    assert_eq!(
                        e.surface.point(u, v, sides).unwrap().map(f64::to_bits),
                        kind.point(u, v, sides).unwrap().map(f64::to_bits)
                    );
                }
            }
        }
    }
}

/// The surface (3t², 3t³, z) over t in [1/2, 1] and z in [0, 1]: a cubic Bezier in t times a
/// line, whose profile has a cusp at t = 0.
fn cusped() -> SurfaceKind {
    let bu = Basis::new(3, vec![0.5, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let bv = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let profile = [[0.75, 0.375], [1.25, 0.75], [2.0, 1.5], [3.0, 3.0]];
    let mut points = Vec::new();
    for p in profile {
        for z in [0.0, 1.0] {
            points.push([p[0], p[1], z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(bu, bv, &points).unwrap())
}

#[test]
fn a_fold_at_a_cusp_is_refused_or_stopped_short_of_it() {
    let s = cusped();
    // The cusp is 0.5 past the low side in u.
    match extend(&s, Side::UMin, 0.8, Reach::Exactly, &mut meter()) {
        Err(ExtendError::ExtensionFailed {
            side: Side::UMin,
            reached,
        }) => {
            assert!(reached <= 0.5 && reached > 0.5 - 1e-3, "{reached}");
        }
        other => panic!("{other:?}"),
    }
    let far = extend(&s, Side::UMin, 0.8, Reach::AsFarAsValid, &mut meter()).unwrap();
    assert!(
        far.reached <= 0.5 && far.reached > 0.5 - 1e-3,
        "{}",
        far.reached
    );
    let ((u0, _), _) = far.surface.domain();
    assert!(u0 >= 0.0, "past the cusp: {u0}");
    // Short of the cusp, the extension is fine.
    let near = exact(&s, Side::UMin, 0.3);
    assert_eq!(near.reached, 0.3);
}

#[test]
fn swept_kinds_and_offsets_extend_by_their_definitions() {
    // An extrusion of a line segment: the line widened in u, the extrusion in v.
    let line = Line::new([0.0, 0.0, 0.0], [1.0, 2.0, 0.0], (0.0, 1.0)).unwrap();
    let make = |u: (f64, f64), v: (f64, f64)| {
        let profile = Trimmed::new(
            Leaf::Line(
                Line::new(
                    [0.0, 0.0, 0.0],
                    [1.0, 2.0, 0.0],
                    (u.0.min(0.0), u.1.max(1.0)),
                )
                .unwrap(),
            ),
            u,
        )
        .unwrap();
        SurfaceKind::Extrusion(Extrusion::new(profile, [0.0, 0.0, 1.0], v).unwrap())
    };
    let ext = SurfaceKind::Extrusion(
        Extrusion::new(
            Trimmed::new(Leaf::Line(line), (0.0, 1.0)).unwrap(),
            [0.0, 0.0, 1.0],
            (0.0, 1.0),
        )
        .unwrap(),
    );
    let e = exact(&ext, Side::UMax, 0.5);
    assert_eq!(e.surface, make((0.0, 1.5), (0.0, 1.0)));
    let e = exact(&ext, Side::VMin, 2.0);
    assert_eq!(e.surface, make((0.0, 1.0), (-2.0, 1.0)));
    // A revolution of a NURBS profile continued past its end.
    let profile = Curve::new(
        Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap(),
        &[[1.0, 0.0, 0.0], [1.5, 0.0, 0.5], [1.2, 0.0, 1.0]],
    )
    .unwrap();
    let rev = SurfaceKind::Revolution(
        Revolution::new(
            Trimmed::whole(Leaf::Nurbs(profile.clone())).unwrap(),
            [0.0; 3],
            Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            (0.0, 1.0),
        )
        .unwrap(),
    );
    let e = exact(&rev, Side::VMax, 0.25);
    assert_eq!(e.surface.domain(), ((0.0, 1.0), (0.0, 1.25)));
    let grown_profile = profile.extend(End::High, 0.25, &mut meter()).unwrap();
    assert!(e.deviation >= grown_profile.deviation);
    // An offset of a graph is the offset of its base extended.
    let mut rng = Rng::new("extend: offset", 0x2530_0003);
    let base = graph(&mut rng, 2, false);
    let off = SurfaceKind::Offset(Offset::new(SurfaceKind::Nurbs(base.clone()), 0.05).unwrap());
    let e = exact(&off, Side::VMax, 0.5);
    let want = base
        .extend(Direction::V, End::High, 0.5, &mut meter())
        .unwrap();
    assert_eq!(
        e.surface,
        SurfaceKind::Offset(Offset::new(SurfaceKind::Nurbs(want.value), 0.05).unwrap())
    );
}

#[test]
fn random_spans_extend_keep_their_source_and_pass_the_certificate() {
    let mut rng = Rng::new("extend: random spans", 0x2530_0004);
    for round in 0..24 {
        let s = graph(&mut rng, 1 + round % 6, round % 3 == 0);
        let kind = SurfaceKind::Nurbs(s);
        for side in SIDES {
            let by = rng.real(0.01, 0.15);
            for reach in [Reach::Exactly, Reach::AsFarAsValid] {
                let e = extend(&kind, side, by, reach, &mut meter())
                    .unwrap_or_else(|err| panic!("round {round}, {side:?}: {err}"));
                assert_eq!(e.reached, by);
                assert!(e.deviation.is_finite() && e.deviation >= 0.0);
                let wider = e.surface.domain();
                let (d0, d1) = (kind.domain(), grown(kind.domain(), side, by));
                assert_eq!(wider, d1, "{d0:?}");
            }
        }
    }
}

#[test]
fn every_documented_refusal() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let cyl = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 1.0, frame, ((0.0, 1.0), (0.0, 1.0))).unwrap(),
    );
    for by in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            extend(&cyl, Side::VMax, by, Reach::Exactly, &mut meter()),
            Err(ExtendError::Curve(CurveError::SpanOutside))
        ));
    }
    // A NURBS form that is not clamped at the side.
    let open = Surface::new(
        Basis::new(1, vec![0.0, 1.0, 2.0, 3.0]).unwrap(),
        Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap(),
        &[
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
        ],
    )
    .unwrap();
    assert!(matches!(
        extend(
            &SurfaceKind::Nurbs(open),
            Side::UMax,
            0.5,
            Reach::Exactly,
            &mut meter()
        ),
        Err(ExtendError::Curve(CurveError::Nurbs(
            NurbsError::NotClamped
        )))
    ));
    // The meter stops it.
    let mut tight = WorkMeter::new(Budget::new(1, u64::MAX));
    assert!(matches!(
        extend(&cusped(), Side::UMin, 0.3, Reach::Exactly, &mut tight),
        Err(ExtendError::Stopped(_))
    ));
}

/// A NURBS surface (P(t), v): the cubic Bezier `profile` in the xy plane over the parameter range
/// `t`, carried straight up z from height v0 to v1, so v is the height and t runs across the side
/// of a v extension and along a u one.
fn prism(profile: [[f64; 2]; 4], t: (f64, f64), v: (f64, f64)) -> SurfaceKind {
    let bu = Basis::new(3, vec![t.0, t.0, t.0, t.0, t.1, t.1, t.1, t.1]).unwrap();
    let bv = Basis::new(1, vec![v.0, v.0, v.1, v.1]).unwrap();
    let mut points = Vec::new();
    for p in profile {
        for z in [v.0, v.1] {
            points.push([p[0], p[1], z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(bu, bv, &points).unwrap())
}

/// (3t², 3t³) over t in [1/2, 1]: its cusp at t = 0 lies 1/2 past its low end.
const CUSP_BELOW: [[f64; 2]; 4] = [[0.75, 0.375], [1.25, 0.75], [2.0, 1.5], [3.0, 3.0]];

/// (3t², 3t³) over t in [−1, −1/2]: its cusp at t = 0 lies 1/2 past its high end.
const CUSP_ABOVE: [[f64; 2]; 4] = [[3.0, -3.0], [2.0, -1.5], [1.25, -0.75], [0.75, -0.375]];

/// An arch in the xy plane over t in [0, 1]: its tangent turns half a turn along it, so the
/// prism's normal turns half a turn across a v side.
const ARCH: [[f64; 2]; 4] = [[1.0, 0.0], [1.0, 1.5], [-1.0, 1.5], [-1.0, 0.0]];

#[test]
fn a_cusp_past_either_side_is_found_to_within_the_narrowest_slab() {
    // The certificate cuts the new strip into slabs from the old side, halving a slab that fails,
    // and a slab still failing at 2⁻²⁴ of the strip's length ends the reach at its near edge. The
    // cusp is 1/2 from the old side, so the reach is a whole number of narrowest slabs out, the
    // next of which holds the cusp, whichever the side and wherever the range across it.
    let by = 0.8;
    for (profile, t, side) in [
        (CUSP_BELOW, (0.5, 1.0), Side::UMin),
        (CUSP_ABOVE, (-1.0, -0.5), Side::UMax),
    ] {
        for across in [(0.0, 1.0), (-2.0, 1.0), (1.0, 2.0)] {
            let s = prism(profile, t, across);
            let (old, far) = match side {
                Side::UMin => (t.0, t.0 - by),
                _ => (t.1, t.1 + by),
            };
            let slab = (far - old).abs() / 16_777_216.0;
            match extend(&s, side, by, Reach::Exactly, &mut meter()) {
                Err(ExtendError::ExtensionFailed {
                    side: failed,
                    reached,
                }) => {
                    assert_eq!(failed, side);
                    assert!(
                        reached <= 0.5 && 0.5 <= reached + slab,
                        "{side:?}, across {across:?}: {reached}"
                    );
                    let slabs = reached / slab;
                    assert_eq!(
                        slabs,
                        slabs.trunc(),
                        "{side:?}, across {across:?}: {reached}"
                    );
                }
                other => panic!("{side:?}, across {across:?}: {other:?}"),
            }
            let short = extend(&s, side, by, Reach::AsFarAsValid, &mut meter()).unwrap();
            assert!(
                short.reached <= 0.5 && 0.5 <= short.reached + 2.0 * slab,
                "{side:?}, across {across:?}: {}",
                short.reached
            );
            assert_eq!(
                short.surface.domain(),
                grown(s.domain(), side, short.reached)
            );
        }
    }
}

#[test]
fn a_swept_cusp_is_refused_though_its_normal_has_a_zero_component() {
    // An extrusion along z of a planar profile: its normal's z component is 0 everywhere, so a
    // box passes only on its x or y component, never on a bound of exactly 0.
    let profile = Curve::new(
        Basis::new(3, vec![0.5, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap(),
        &CUSP_BELOW.map(|[x, y]| [x, y, 0.0])[..],
    )
    .unwrap();
    let ext = SurfaceKind::Extrusion(
        Extrusion::new(
            Trimmed::whole(Leaf::Nurbs(profile)).unwrap(),
            [0.0, 0.0, 1.0],
            (0.0, 1.0),
        )
        .unwrap(),
    );
    match extend(&ext, Side::UMin, 0.8, Reach::Exactly, &mut meter()) {
        Err(ExtendError::ExtensionFailed {
            side: Side::UMin,
            reached,
        }) => assert!(reached <= 0.5 && reached > 0.5 - 1e-3, "{reached}"),
        other => panic!("{other:?}"),
    }
    let near = exact(&ext, Side::UMin, 0.3);
    assert_eq!(near.reached, 0.3);
}

#[test]
fn a_degenerate_sweep_has_no_normal_to_certify() {
    // A line extruded along itself: the surface is that line, its normal exactly 0 everywhere,
    // and its normal bounds are exactly 0 too, so no extension of it is certified.
    let profile = Trimmed::new(
        Leaf::Line(Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 1.0)).unwrap()),
        (0.0, 1.0),
    )
    .unwrap();
    let flat =
        SurfaceKind::Extrusion(Extrusion::new(profile, [2.0, 0.0, 0.0], (0.0, 1.0)).unwrap());
    for side in SIDES {
        let refused = extend(&flat, side, 0.5, Reach::Exactly, &mut meter());
        assert!(
            matches!(refused, Err(ExtendError::ExtensionFailed { reached, .. }) if reached == 0.0),
            "{side:?}: {refused:?}"
        );
        let e = extend(&flat, side, 0.5, Reach::AsFarAsValid, &mut meter()).unwrap();
        assert_eq!((e.reached, &e.surface), (0.0, &flat), "{side:?}");
    }
}

#[test]
fn a_normal_that_turns_across_the_side_is_certified_box_by_box() {
    // No one box across the arch's whole range holds a normal bound clear of zero: the
    // certificate halves the boxes across the side until each passes.
    for t in [(0.0, 1.0), (1.0, 2.0)] {
        let s = prism(ARCH, t, (0.0, 1.0));
        for side in [Side::VMin, Side::VMax] {
            let e = exact(&s, side, 0.5);
            assert_eq!(e.reached, 0.5);
            assert_eq!(e.surface.domain(), grown(s.domain(), side, 0.5), "{t:?}");
        }
    }
}

#[test]
fn an_affine_image_extends_to_the_same_terms_on_the_wider_domain() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let squash = TypedTransform::from(Transform::scale(2.0, 0.5, 1.0).unwrap());
    let domain = ((0.0, 1.0), (0.0, 1.0));
    let image = |d: ParamBox| {
        Cylinder::new([0.0; 3], 1.0, frame, d)
            .unwrap()
            .transformed(&squash, &mut meter())
            .unwrap()
            .value
    };
    let s = image(domain);
    assert!(matches!(s, SurfaceKind::Affine(_)), "{s:?}");
    for side in SIDES {
        let e = exact(&s, side, 0.25);
        assert_eq!(e.surface.domain(), grown(domain, side, 0.25), "{side:?}");
        assert_eq!(e.surface, image(grown(domain, side, 0.25)), "{side:?}");
        assert_eq!((e.reached, e.deviation), (0.25, 0.0));
    }
}

#[test]
fn swept_kinds_extend_past_either_end_of_a_profile_or_a_straight_direction() {
    // An extrusion of a line segment, trimmed to `u` on a line over `line`.
    let lined = |u: (f64, f64), v: (f64, f64), line: (f64, f64)| {
        let profile = Trimmed::new(
            Leaf::Line(Line::new([0.0, 0.0, 0.0], [1.0, 2.0, 0.0], line).unwrap()),
            u,
        )
        .unwrap();
        SurfaceKind::Extrusion(Extrusion::new(profile, [0.0, 0.0, 1.0], v).unwrap())
    };
    let ext = lined((0.0, 1.0), (0.0, 1.0), (0.0, 1.0));
    // Past the line's start, the line itself on a wider domain; up the extrusion, a taller one.
    let e = exact(&ext, Side::UMin, 0.5);
    assert_eq!(e.surface, lined((-0.5, 1.0), (0.0, 1.0), (-0.5, 1.0)));
    let e = exact(&ext, Side::VMax, 2.0);
    assert_eq!(e.surface, lined((0.0, 1.0), (0.0, 3.0), (0.0, 1.0)));
    // A trim inside the line's domain only moves.
    let inner = lined((0.25, 0.75), (0.0, 1.0), (0.0, 1.0));
    let e = exact(&inner, Side::UMin, 0.125);
    assert_eq!(e.surface, lined((0.125, 0.75), (0.0, 1.0), (0.0, 1.0)));
    // A revolution of a NURBS profile: continued past either end of its basis, trimmed further
    // inside it, and turned further about its axis.
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let profile = Curve::new(
        Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap(),
        &[[1.0, 0.0, 0.0], [1.5, 0.0, 0.5], [1.2, 0.0, 1.0]],
    )
    .unwrap();
    let rev = |p: Trimmed<3>, u: (f64, f64)| {
        SurfaceKind::Revolution(Revolution::new(p, [0.0; 3], frame, u).unwrap())
    };
    let whole = |c: Curve<3>| Trimmed::whole(Leaf::Nurbs(c)).unwrap();
    let s = rev(whole(profile.clone()), (0.0, 1.0));
    for (end, side) in [(End::Low, Side::VMin), (End::High, Side::VMax)] {
        let e = exact(&s, side, 0.25);
        let grown_profile = profile.extend(end, 0.25, &mut meter()).unwrap().value;
        assert_eq!(e.surface, rev(whole(grown_profile), (0.0, 1.0)), "{end:?}");
    }
    let e = exact(&s, Side::UMax, 0.5);
    assert_eq!(e.surface, rev(whole(profile.clone()), (0.0, 1.5)));
    let trimmed = rev(
        Trimmed::new(Leaf::Nurbs(profile.clone()), (0.25, 0.75)).unwrap(),
        (0.0, 1.0),
    );
    let e = exact(&trimmed, Side::VMin, 0.125);
    assert_eq!(
        e.surface,
        rev(
            Trimmed::new(Leaf::Nurbs(profile), (0.125, 0.75)).unwrap(),
            (0.0, 1.0)
        )
    );
    assert_eq!(e.deviation, 0.0);
}

#[test]
fn every_error_says_what_went_wrong() {
    let failed = ExtendError::ExtensionFailed {
        side: Side::VMax,
        reached: 0.25,
    }
    .to_string();
    assert!(
        failed.contains("VMax") && failed.contains("0.25"),
        "{failed}"
    );
    let refused = ExtendError::Curve(CurveError::SpanOutside).to_string();
    assert!(
        refused.contains("refused") && refused.contains("SpanOutside"),
        "{refused}"
    );
    let mut tight = WorkMeter::new(Budget::new(1, u64::MAX));
    let stopped = extend(&cusped(), Side::UMin, 0.3, Reach::Exactly, &mut tight)
        .unwrap_err()
        .to_string();
    assert!(stopped.starts_with("stopped"), "{stopped}");
}

#[test]
fn a_straight_extension_moves_nothing_whatever_the_placement() {
    // Two exact scales by 2^340 place this extrusion by L = 2^680·I, whose norm overflows a
    // double, though the surface's points and normals do not: its profile and direction are
    // 2^-700 long. Extending its straight direction moves no profile, so the deviation is 0 and
    // the placement's size never enters it.
    let pow2 = |biased: u64| f64::from_bits(biased << 52);
    let (tiny, big) = (pow2(1023 - 700), pow2(1023 + 340));
    let profile = Trimmed::new(
        Leaf::Line(Line::new([0.0; 3], [tiny, 2.0 * tiny, 0.0], (0.0, 1.0)).unwrap()),
        (0.0, 1.0),
    )
    .unwrap();
    let mut s =
        SurfaceKind::Extrusion(Extrusion::new(profile, [0.0, 0.0, tiny], (0.0, 1.0)).unwrap());
    let scale = TypedTransform::from(Transform::uniform_scale(big).unwrap());
    for _ in 0..2 {
        s = s.transformed(&scale, &mut meter()).unwrap().value;
    }
    assert!(matches!(s, SurfaceKind::Extrusion(_)), "{s:?}");
    for side in [Side::VMin, Side::VMax] {
        let e = exact(&s, side, 1.0);
        assert_eq!((e.reached, e.deviation), (1.0, 0.0), "{side:?}");
        assert_eq!(e.surface.domain(), grown(s.domain(), side, 1.0));
    }
}
