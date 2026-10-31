//! Attacks on piece G67's claims in `geom-surface`: closed-form extents and derivative bounds for
//! every analytic surface kind.
//!
//! - **Containment on a million seeded inputs per kind** (in release builds; a fiftieth in debug
//!   builds): plane, cylinder, cone, sphere, ring torus, spindle torus (apple and lemon parts) and
//!   the affine images of each that are not similarities. For random surfaces, random parameter
//!   boxes and random points in them, the certified enclosures of S, `S_u` and `S_v` at the point
//!   must meet the bound and the two derivative bounds over the box; at every hundredth point the
//!   partials to order 2 at the point must meet `partials_over` the box.
//! - **The second partials checked independently**: against central differences of the first
//!   partials in doubles, so a sign or a factor of π in a derivative's closed form is caught.
//! - **Tightness against dense samples**: the boxes lie within a small slack of the extremes of a
//!   401 × 401 grid of samples (an interval extension over the box would miss by a few per cent).
//! - **Edges**: whole spheres and tori bounded by their radii, a cone through its apex, boxes of
//!   one point and of one line, and the affine torus's pieces route.
//! - **Only angles are cut into pieces**: a cylinder's and a cone's second parameter is a length,
//!   so their normal bounds, and their affine images', cost the same over a long span of it as
//!   over a short one.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::needless_range_loop,
    clippy::disallowed_methods,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

use geom_curve::{DerivativeBound, Frame};
use geom_math::{Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Direction, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, ParamBox, ParamSurface, Plane, Sphere, SurfaceKind, Torus, partial_index,
};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

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

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * ((self.next() >> 11) as f64 / (1u64 << 53) as f64)
    }

    fn pick(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn vector(&mut self, size: f64) -> [f64; 3] {
        std::array::from_fn(|_| self.range(-size, size))
    }
}

/// All the work in release builds, a fiftieth in debug builds and under Miri a little.
fn scale(n: usize) -> usize {
    if cfg!(miri) {
        (n / 5000).max(1)
    } else if cfg!(debug_assertions) {
        (n / 50).max(1)
    } else {
        n
    }
}

fn meets(e: &[Interval; 3], b: &Bounds<3>) -> bool {
    e.iter()
        .zip(b.lo.iter().zip(&b.hi))
        .all(|(x, (&lo, &hi))| x.lo() <= hi && lo <= x.hi())
}

fn overlaps(a: &[Interval; 3], b: &[Interval; 3]) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| x.lo() <= y.hi() && y.lo() <= x.hi())
}

fn bounded(d: DerivativeBound<3>) -> Bounds<3> {
    match d {
        DerivativeBound::Bounded(b) => b,
        DerivativeBound::Unbounded => panic!("an analytic kind's partials are bounded"),
    }
}

fn frame(rng: &mut Rng) -> Frame<3> {
    loop {
        if let Ok(f) = Frame::new(rng.vector(1.0), rng.vector(1.0)) {
            return f;
        }
    }
}

/// A random interval of [a, b]: all of it, a point, or two random ends.
fn interval(rng: &mut Rng, (a, b): (f64, f64)) -> (f64, f64) {
    match rng.pick(12) {
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

fn point_in(rng: &mut Rng, (lo, hi): (f64, f64)) -> f64 {
    match rng.pick(24) {
        0 => lo,
        1 => hi,
        _ => rng.range(lo, hi).clamp(lo, hi),
    }
}

/// A random one-turn-or-less angular domain.
fn turn(rng: &mut Rng) -> (f64, f64) {
    let a = rng.range(-2.0, 2.0);
    (a, a + rng.range(0.01, 2.0))
}

/// A random surface of kind `k`: 0 plane, 1 cylinder, 2 cone, 3 sphere, 4 ring torus, 5 spindle
/// torus (its apple or lemon part, or both), 6 an affine image of one of 1 to 5 that is not a
/// similarity.
fn surface(k: usize, rng: &mut Rng) -> Option<SurfaceKind> {
    let c = rng.vector(50.0);
    let f = frame(rng);
    let v = |rng: &mut Rng, lo: f64, hi: f64| {
        let a = rng.range(lo, hi);
        (a, a + rng.range(0.01, hi - lo))
    };
    Some(match k {
        0 => SurfaceKind::Plane(
            Plane::new(
                c,
                rng.vector(5.0),
                rng.vector(5.0),
                (v(rng, -5.0, 5.0), v(rng, -5.0, 5.0)),
            )
            .ok()?,
        ),
        1 => SurfaceKind::Cylinder(
            Cylinder::new(c, rng.range(0.1, 20.0), f, (turn(rng), v(rng, -10.0, 10.0))).ok()?,
        ),
        2 => SurfaceKind::Cone(
            Cone::new(
                c,
                rng.range(-5.0, 5.0),
                rng.range(-2.0, 2.0),
                f,
                (turn(rng), v(rng, -10.0, 10.0)),
            )
            .ok()?,
        ),
        3 => {
            let (a, b) = (rng.range(-0.5, 0.5), rng.range(-0.5, 0.5));
            SurfaceKind::Sphere(
                Sphere::new(
                    c,
                    rng.range(0.1, 20.0),
                    f,
                    (turn(rng), (a.min(b), a.max(b))),
                )
                .ok()?,
            )
        }
        4 => {
            let major = rng.range(1.0, 20.0);
            SurfaceKind::Torus(
                Torus::new(
                    c,
                    major,
                    rng.range(0.05, 0.95) * major,
                    f,
                    (turn(rng), turn(rng)),
                )
                .ok()?,
            )
        }
        5 => {
            // A spindle: the minor radius above the major. Its apple is where R + r·cos πv ≥ 0.
            let major = rng.range(1.0, 10.0);
            let minor = major * rng.range(1.05, 3.0);
            let edge = (-major / minor).acos() / std::f64::consts::PI;
            let domain = match rng.pick(3) {
                0 => (-edge * rng.range(0.1, 1.0), edge * rng.range(0.1, 1.0)),
                1 => (
                    edge + rng.range(0.0, 0.1),
                    edge + rng.range(0.15, 2.0 - 2.0 * edge),
                ),
                _ => turn(rng),
            };
            SurfaceKind::Torus(Torus::new(c, major, minor, f, (turn(rng), domain)).ok()?)
        }
        _ => {
            let base = surface(1 + rng.pick(5) as usize, rng)?;
            let t = TypedTransform::from(
                Transform::scale(
                    rng.range(0.3, 3.0),
                    rng.range(0.3, 3.0),
                    rng.range(0.3, 3.0),
                )
                .unwrap(),
            );
            let image = base.transformed(&t, &mut meter()).ok()?.value;
            if !matches!(image, SurfaceKind::Affine(_)) {
                return None;
            }
            image
        }
    })
}

const KINDS: [&str; 7] = [
    "plane",
    "cylinder",
    "cone",
    "sphere",
    "ring torus",
    "spindle torus",
    "affine image",
];

/// A random box of the domain.
fn region(rng: &mut Rng, (us, vs): ParamBox) -> ParamBox {
    (interval(rng, us), interval(rng, vs))
}

/// `points` checks on each of `regions` boxes.
fn contain(name: &str, s: &SurfaceKind, regions: usize, points: usize, rng: &mut Rng) -> usize {
    let m = &mut meter();
    let mut checked = 0;
    for _ in 0..regions {
        let r = region(rng, s.domain());
        let b = s.bound(r, m).unwrap();
        let bu = bounded(s.derivative_bound(r, Direction::U, m).unwrap());
        let bv = bounded(s.derivative_bound(r, Direction::V, m).unwrap());
        let over = s.partials_over(r, SIDES, 2, m).unwrap();
        for i in 0..points {
            let (u, v) = (point_in(rng, r.0), point_in(rng, r.1));
            let e = s.enclose(u, v, SIDES, m).unwrap();
            assert!(
                meets(&e[0], &b) && meets(&e[1], &bu) && meets(&e[2], &bv),
                "{name}: ({u}, {v}) outside the bounds over {r:?}: {e:?} {b:?} {bu:?} {bv:?}"
            );
            if i % 100 == 0 {
                let at = s.partials_over(((u, u), (v, v)), SIDES, 2, m).unwrap();
                for (k, (x, y)) in at.iter().zip(&over).enumerate() {
                    assert!(
                        overlaps(x, y),
                        "{name}: partial {k} at ({u}, {v}) outside {r:?}"
                    );
                }
                // The point and first partials agree with the enclosures.
                for (k, (i, j)) in [(0, 0), (1, 0), (0, 1)].into_iter().enumerate() {
                    let x = &at[partial_index(i, j).unwrap()];
                    assert!(
                        overlaps(x, &e[k]),
                        "{name}: partial ({i}, {j}) at ({u}, {v})"
                    );
                }
            }
            checked += 1;
        }
    }
    checked
}

/// Claim: every analytic surface kind's bound and derivative bounds over any box hold the surface
/// at every point of it, and its partials to order 2 over the box hold each point's: a million
/// seeded points per kind.
#[test]
fn every_analytic_surface_kind_holds_a_million_seeded_points() {
    for (k, name) in KINDS.iter().enumerate() {
        let mut rng = Rng(0x6765 + k as u64);
        let (surfaces, regions, points) = (scale(100), 10, 1000);
        let (mut checked, mut built) = (0, 0);
        while built < surfaces {
            let Some(s) = surface(k, &mut rng) else {
                continue;
            };
            built += 1;
            checked += contain(name, &s, regions, points, &mut rng);
        }
        assert_eq!(checked, surfaces * regions * points);
        println!("{name}: {checked} points held");
    }
}

/// Claim: the second partials' closed forms are the derivatives of the first: at random points,
/// within 10⁻⁶ (relative) of central differences of `S_u` and `S_v` in doubles.
#[test]
fn second_partials_match_differences_of_the_first() {
    let mut rng = Rng(0x6771);
    let m = &mut meter();
    for round in 0..scale(3500).max(70) {
        let Some(s) = surface(round % 7, &mut rng) else {
            continue;
        };
        let ((u0, u1), (v0, v1)) = s.domain();
        let h = 1e-6;
        let u = rng.range(u0 + 2.0 * h, u1 - 2.0 * h);
        let v = rng.range(v0 + 2.0 * h, v1 - 2.0 * h);
        if !(u0 + h < u && u < u1 - h && v0 + h < v && v < v1 - h) {
            continue;
        }
        let at = s.partials_over(((u, u), (v, v)), SIDES, 2, m).unwrap();
        let p = |u: f64, v: f64| s.partials(u, v, SIDES).unwrap();
        let (pu, mu, pv, mv) = (p(u + h, v), p(u - h, v), p(u, v + h), p(u, v - h));
        let size = 1.0
            + p(u, v)[1..]
                .iter()
                .flatten()
                .fold(0.0f64, |a, x| a.max(x.abs()));
        for d in 0..3 {
            let suu = (pu[1][d] - mu[1][d]) / (2.0 * h);
            let suv = (pv[1][d] - mv[1][d]) / (2.0 * h);
            let svv = (pv[2][d] - mv[2][d]) / (2.0 * h);
            for ((i, j), want) in [((2, 0), suu), ((1, 1), suv), ((0, 2), svv)] {
                let e = at[partial_index(i, j).unwrap()][d];
                let mid = f64::midpoint(e.lo(), e.hi());
                assert!(
                    (mid - want).abs() <= 1e-6 * size * 10.0,
                    "{} ({i}, {j}) coordinate {d} at ({u}, {v}): {e:?} against {want}",
                    KINDS[round % 7]
                );
            }
        }
    }
}

/// The extremes of the point (`which` 0) or a first partial (1 in u, 2 in v) over a grid of
/// (n + 1)² samples.
fn sampled(
    s: &SurfaceKind,
    ((u0, u1), (v0, v1)): ParamBox,
    which: usize,
    n: usize,
) -> ([f64; 3], [f64; 3]) {
    let (mut min, mut max) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for i in 0..=n {
        let u = (u0 + (u1 - u0) * (i as f64 / n as f64)).min(u1);
        for j in 0..=n {
            let v = (v0 + (v1 - v0) * (j as f64 / n as f64)).min(v1);
            let p = s.partials(u, v, SIDES).unwrap()[which];
            for d in 0..3 {
                min[d] = min[d].min(p[d]);
                max[d] = max[d].max(p[d]);
            }
        }
    }
    (min, max)
}

/// Claim: the boxes are the kinds' own extents: within 3·10⁻³ (relative to the surface's size) of
/// a 201 × 201 grid's extremes, which come within about 5·10⁻⁴ of the true ones, also for an
/// affine image of a torus, whose last route is second order.
#[test]
fn surface_bounds_are_the_kinds_own_extents() {
    let mut rng = Rng(0x6772);
    let m = &mut meter();
    for round in 0..scale(280).max(14) {
        let k = round % 7;
        let Some(s) = surface(k, &mut rng) else {
            continue;
        };
        let r = region(&mut rng, s.domain());
        let boxes = [
            s.bound(r, m).unwrap(),
            bounded(s.derivative_bound(r, Direction::U, m).unwrap()),
            bounded(s.derivative_bound(r, Direction::V, m).unwrap()),
        ];
        let slack = 3e-3;
        for (which, b) in boxes.iter().enumerate() {
            let (min, max) = sampled(&s, r, which, 200);
            let size = 1.0 + (0..3).fold(0.0f64, |a, d| a.max(max[d].abs()).max(min[d].abs()));
            for d in 0..3 {
                assert!(
                    b.lo[d] <= min[d] + 1e-12 * size && max[d] - 1e-12 * size <= b.hi[d],
                    "{} {which}: {b:?} misses [{min:?}, {max:?}]",
                    KINDS[k]
                );
                assert!(
                    min[d] - b.lo[d] <= slack * size && b.hi[d] - max[d] <= slack * size,
                    "{} {which} coordinate {d} over {r:?}: {b:?} against [{min:?}, {max:?}]",
                    KINDS[k]
                );
            }
        }
    }
}

/// Claim: a whole sphere and a whole ring torus are bounded by their radii, to a few rounding
/// steps, on any frame.
#[test]
fn whole_spheres_and_tori_are_bounded_by_their_radii() {
    let m = &mut meter();
    let f = Frame::new([3.0, 4.0, 12.0], [4.0, -3.0, 0.0]).unwrap();
    let s = Sphere::new([1.0, -2.0, 3.0], 5.0, f, ((0.0, 2.0), (-0.5, 0.5))).unwrap();
    let b = s.bound(((0.0, 2.0), (-0.5, 0.5)), m).unwrap();
    for (d, c) in [1.0, -2.0, 3.0].into_iter().enumerate() {
        assert!(b.lo[d] <= c - 5.0 && c - 5.0 - b.lo[d] < 1e-13, "{b:?}");
        assert!(b.hi[d] >= c + 5.0 && b.hi[d] - (c + 5.0) < 1e-13, "{b:?}");
    }
    let z = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let t = Torus::new([0.0; 3], 5.0, 2.0, z, ((0.0, 2.0), (0.0, 2.0))).unwrap();
    let b = t.bound(((0.0, 2.0), (0.0, 2.0)), m).unwrap();
    for d in 0..2 {
        assert!(b.lo[d] <= -7.0 && b.lo[d] > -7.0 - 1e-13, "{b:?}");
        assert!(b.hi[d] >= 7.0 && b.hi[d] < 7.0 + 1e-13, "{b:?}");
    }
    assert!(b.lo[2] <= -2.0 && b.lo[2] > -2.0 - 1e-13 && b.hi[2] >= 2.0 && b.hi[2] < 2.0 + 1e-13);
    // A quarter of the tube at u = 0: x = R + r·cos πv from R + r down to R.
    let b = t.bound(((0.0, 0.0), (0.0, 0.5)), m).unwrap();
    assert!(b.lo[0] <= 5.0 && 5.0 - b.lo[0] < 1e-13 && b.hi[0] >= 7.0 && b.hi[0] - 7.0 < 1e-13);
    // A cone through its apex: radius |r₀ + k·v| changes sign at v = −r₀/k.
    let c = Cone::new([0.0; 3], 1.0, -1.0, z, ((0.0, 2.0), (0.0, 3.0))).unwrap();
    let b = c.bound(((0.0, 2.0), (0.0, 3.0)), m).unwrap();
    for d in 0..2 {
        assert!(
            b.lo[d] <= -2.0 && b.lo[d] > -2.0 - 1e-13 && b.hi[d] >= 2.0 && b.hi[d] < 2.0 + 1e-13
        );
    }
    assert_eq!((b.lo[2], b.hi[2]), (0.0, 3.0));
}

/// Claim: a plane's box is its region's corners exactly, and a box of one point or one line is
/// that point or line's own extent.
#[test]
fn planes_points_and_lines_are_exact() {
    let m = &mut meter();
    let p = Plane::new(
        [1.0, 2.0, 3.0],
        [1.0, -2.0, 0.5],
        [0.0, 1.0, 2.0],
        ((-10.0, 10.0), (-10.0, 10.0)),
    )
    .unwrap();
    let b = p.bound(((-1.0, 2.0), (0.5, 3.0)), m).unwrap();
    let corners = [(-1.0, 0.5), (-1.0, 3.0), (2.0, 0.5), (2.0, 3.0)];
    for d in 0..3 {
        let values: Vec<f64> = corners
            .iter()
            .map(|&(u, v)| p.point(u, v, SIDES).unwrap()[d])
            .collect();
        let lo = values.iter().fold(f64::INFINITY, |a, &x| a.min(x));
        let hi = values.iter().fold(f64::NEG_INFINITY, |a, &x| a.max(x));
        assert_eq!((b.lo[d], b.hi[d]), (lo, hi));
    }
    let z = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let s = Sphere::new([0.0; 3], 2.0, z, ((0.0, 2.0), (-0.5, 0.5))).unwrap();
    // The equator: a circle of radius 2 in the plane z = 0.
    let b = s.bound(((0.0, 2.0), (0.0, 0.0)), m).unwrap();
    assert_eq!((b.lo[2], b.hi[2]), (0.0, 0.0));
    for d in 0..2 {
        assert!(
            b.lo[d] <= -2.0 && b.lo[d] > -2.0 - 1e-14 && b.hi[d] >= 2.0 && b.hi[d] < 2.0 + 1e-14
        );
    }
    // One point: a few rounding steps about it.
    let b = s.bound(((0.3, 0.3), (0.2, 0.2)), m).unwrap();
    let e = s.enclose(0.3, 0.2, SIDES, m).unwrap();
    assert!(meets(&e[0], &b));
    for d in 0..3 {
        assert!(b.hi[d] - b.lo[d] < 1e-14, "{b:?}");
    }
}

/// Claim: regions outside the domain are refused.
#[test]
fn regions_outside_are_refused() {
    let m = &mut meter();
    let z = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let t = Torus::new([0.0; 3], 5.0, 2.0, z, ((0.0, 1.0), (0.0, 1.0))).unwrap();
    for r in [
        ((-0.5, 0.5), (0.0, 1.0)),
        ((0.0, 1.0), (0.5, 1.5)),
        ((0.5, 0.25), (0.0, 1.0)),
        ((f64::NAN, 0.5), (0.0, 1.0)),
    ] {
        assert!(t.bound(r, m).is_err(), "{r:?}");
        assert!(t.derivative_bound(r, Direction::U, m).is_err(), "{r:?}");
        assert!(t.partials_over(r, SIDES, 2, m).is_err(), "{r:?}");
    }
}

/// Claim: the last route (an affine image of a torus) cuts the v-span into ⌈32·w⌉ even pieces from
/// its lower end, at most 64, each costing the same work: the work grows by one step a piece. And
/// on pieces of 1/32 of a half-turn the box is within r²·|`S_vv`| of the samples' extremes, r the
/// half-width 1/64 and |`S_vv`| at most π² times the minor radius times the map's largest factor.
#[test]
fn the_last_route_cuts_the_v_span_into_even_pieces() {
    let f = Frame::new([1.0, 0.3, 0.2], [0.1, -0.4, 1.0]).unwrap();
    let torus = Torus::new([0.0; 3], 3.0, 1.0, f, ((0.0, 2.0), (0.0, 2.0))).unwrap();
    let t = TypedTransform::from(Transform::scale(1.0, 2.0, 0.5).unwrap());
    let s = SurfaceKind::Torus(torus)
        .transformed(&t, &mut meter())
        .unwrap()
        .value;
    assert!(matches!(s, SurfaceKind::Affine(_)));
    let us = (0.1, 0.4);
    let work = |vs: (f64, f64)| {
        let mut m = meter();
        s.bound((us, vs), &mut m).unwrap();
        m.used().work
    };
    let one = work((0.25, 0.281_25));
    let step = work((0.25, 0.312_5)) - one;
    assert!(step > 0);
    for (vs, pieces) in [
        ((0.25, 0.5), 8),
        ((1.0, 1.25), 8),
        ((0.1, 1.9), 58),
        ((0.0, 2.0), 64),
    ] {
        assert_eq!(work(vs), one + (pieces - 1) * step, "{vs:?}");
    }
    let r = (us, (0.25, 0.75));
    let b = s.bound(r, &mut meter()).unwrap();
    let (min, max) = sampled(&s, r, 0, 200);
    let half = 1.0 / 64.0;
    let slack = half * half * std::f64::consts::PI.powi(2) * 2.0 + 1e-4;
    for d in 0..3 {
        assert!(
            min[d] - b.lo[d] <= slack && b.hi[d] - max[d] <= slack,
            "coordinate {d}: {b:?} against [{min:?}, {max:?}]"
        );
    }
}

#[test]
fn only_angles_are_cut_into_pieces() {
    // A cylinder's and a cone's second parameter is a length: their normal bounds, and their
    // affine images', cost the same work over 4 of it as over a tenth. A sphere's is an angle,
    // cut into more pieces over a longer span.
    let f = Frame::new([1.0, 0.3, 0.2], [0.1, -0.4, 1.0]).unwrap();
    let t = TypedTransform::from(Transform::scale(1.0, 2.0, 0.5).unwrap());
    let us = (0.1, 0.4);
    let work = |s: &SurfaceKind, vs: (f64, f64)| {
        let mut m = meter();
        s.normal_bound((us, vs), &mut m).unwrap();
        m.used().work
    };
    for s in [
        SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 2.0, f, ((0.0, 2.0), (0.0, 4.0))).unwrap()),
        SurfaceKind::Cone(Cone::new([0.0; 3], 1.0, 0.5, f, ((0.0, 2.0), (0.0, 4.0))).unwrap()),
    ] {
        let affine = s.transformed(&t, &mut meter()).unwrap().value;
        assert!(matches!(affine, SurfaceKind::Affine(_)));
        for k in [&s, &affine] {
            assert_eq!(work(k, (0.0, 0.1)), work(k, (0.0, 4.0)), "{k:?}");
        }
    }
    let sphere =
        SurfaceKind::Sphere(Sphere::new([0.0; 3], 2.0, f, ((0.0, 2.0), (-0.5, 0.5))).unwrap());
    assert!(work(&sphere, (-0.5, -0.4)) < work(&sphere, (-0.5, 0.5)));
}
