//! The true point and the fit on inputs whose answers are known exactly: planes that meet at their
//! own origin under bounds of zero, a wall with a level shelf beside the floor, parallel to it and
//! apart, in the box about the one point where the wall comes down through the floor, and an arc on
//! a cone whose domain runs down to just above its apex.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly; the names are chapter 12.12's"
)]

mod miri;

use geom_curve::{Basis as Leaf, Frame, IntersectionCurve, ParamCurve, Span, SurfaceRef, Trimmed};
use geom_intersect::fit::{fit, true_point};
use geom_intersect::march::Branch;
use geom_intersect::result::CurveEnd;
use geom_num::interval::Interval;
use geom_num::math::{cospi, sinpi};
use geom_nurbs::{Basis, Curve, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Extrusion, Plane, SurfaceKind};

const REFS: [SurfaceRef; 2] = [SurfaceRef(1), SurfaceRef(2)];

/// Room for the search many times over: the points below are proven within a few thousand units.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 16))
}

fn plane(
    origin: [f64; 3],
    u: [f64; 3],
    v: [f64; 3],
    domain: ((f64, f64), (f64, f64)),
) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(origin, u, v, domain).unwrap())
}

fn branch(vertices: Vec<[f64; 4]>) -> Branch {
    Branch {
        vertices: vertices
            .into_iter()
            .map(|x| x.map(|q| Interval::point(q).unwrap()))
            .collect(),
        ends: [CurveEnd::Boundary, CurveEnd::Boundary],
        deviation: 0.0,
        passes: Vec::new(),
    }
}

/// 2^e exactly, from its bits, for e from -1022 to 1023.
fn pow2(e: i32) -> f64 {
    f64::from_bits(u64::try_from(e + 1023).unwrap() << 52)
}

/// A curve made by hand over `(a, b)`: a degree-1 cache from `c0` to `c1`, each pcurve from its
/// point in `p0` to its point in `p1`, and one span with the bounds `curve` and `pcurves`.
fn made(
    (a, b): (f64, f64),
    (c0, c1): ([f64; 3], [f64; 3]),
    (p0, p1): ([[f64; 2]; 2], [[f64; 2]; 2]),
    (curve, pcurves): (f64, [f64; 2]),
) -> IntersectionCurve<3> {
    let basis = || Basis::new(1, vec![a, a, b, b]).unwrap();
    let cache = Curve::new(basis(), &[c0, c1]).unwrap();
    let pcurve = |i: usize| {
        let line = Curve::new(basis(), &[p0[i], p1[i]]).unwrap();
        Trimmed::new(Leaf::Nurbs(line), (a, b)).unwrap()
    };
    let spans = vec![Span {
        at: (a, b),
        curve,
        pcurves,
    }];
    IntersectionCurve::new(REFS, Vec::new(), cache, [pcurve(0), pcurve(1)], spans).unwrap()
}

/// Whether `x` holds the double `q`.
fn holds(x: Interval, q: f64) -> bool {
    x.lo() <= q && q <= x.hi()
}

#[test]
fn a_true_point_at_the_planes_own_origin_is_proven_under_bounds_of_zero() {
    // The planes z = 0 and y = 0, both with their origin at (1, 0, 0) and u along x, meet in the x
    // axis. The cache is the axis from 1/2 to 3/2 and both pcurves are (t, 0), exact in doubles, so
    // every bound is truly 0, and the true point in the plane across the cache at t = 0, x = 1, is
    // the origin of both planes: (0, 0) on each. A parameter of 0 has no size of its own, while the
    // point it stands for is at 1, where a double's step is 2^-52: the box about the pcurves'
    // points must still be a few such steps wide for the point to be proven in it.
    let domain = ((-4.0, 4.0), (-4.0, 4.0));
    let first = plane([1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], domain);
    let second = plane([1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], domain);
    let curve = made(
        (-0.5, 0.5),
        ([0.5, 0.0, 0.0], [1.5, 0.0, 0.0]),
        ([[-0.5, 0.0]; 2], [[0.5, 0.0]; 2]),
        (0.0, [0.0; 2]),
    );
    let x = true_point(&first, &second, &curve, 0.0, &mut meter())
        .unwrap()
        .unwrap_or_else(|| panic!("no true point at the origin"));
    for i in x {
        assert!(holds(i, 0.0), "{x:?}");
    }
}

#[test]
fn a_true_point_beside_a_shelf_parallel_to_the_floor_is_still_proven() {
    // The wall's profile runs level along z = 0 from x = -1 to x = 0, then down at 45 degrees to
    // (1, -1); swept along y it meets the floor z = -1/16 in the line x = 1/16, at u = 17/16 on the
    // wall. The cache is that line, exact, so a bound of 1/4 on the curve is true. Under it the box
    // about the pcurves' points at t = 0 reaches back over the shelf, where the wall is parallel to
    // the floor and 1/16 above it. No point of the curve is there, and nothing but the distance
    // between the two says so: the system's slope is singular all over the shelf. The one zero in
    // the box is (17/16, 0) on the wall and (1/16, 0) on the floor.
    let h = 0.0625;
    let basis = Basis::new(1, vec![0.0, 0.0, 1.0, 2.0, 2.0]).unwrap();
    let points = [[-1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, -1.0]];
    let profile = Curve::new(basis, &points).unwrap();
    let profile = Trimmed::new(Leaf::Nurbs(profile), (0.0, 2.0)).unwrap();
    let wall =
        SurfaceKind::Extrusion(Extrusion::new(profile, [0.0, 1.0, 0.0], (-1.0, 1.0)).unwrap());
    let floor = plane(
        [0.0, 0.0, -h],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        ((-4.0, 4.0), (-4.0, 4.0)),
    );
    let y = 3.0 / 16.0;
    let curve = made(
        (-y, y),
        ([h, -y, -h], [h, y, -h]),
        ([[1.0 + h, -y], [h, -y]], [[1.0 + h, y], [h, y]]),
        (0.25, [0.0; 2]),
    );
    let z = true_point(&wall, &floor, &curve, 0.0, &mut meter())
        .unwrap()
        .unwrap_or_else(|| panic!("no true point"));
    for (i, want) in z.into_iter().zip([1.0 + h, 0.0, h, 0.0]) {
        assert!(holds(i, want), "{z:?}");
    }
}

#[test]
fn an_arc_on_a_cone_whose_domain_ends_just_above_the_apex_is_still_fitted() {
    // The cone v·(cos πu, sin πu, 1) meets the plane z = 1/4 in the circle of radius 1/4 at that
    // height, at 45 degrees all the way round. The branch is its arc from u = 1/8 to 3/8, three
    // vertices, nowhere near the apex, and the fitted curve is that arc within the ceiling. The
    // cone's domain runs down to v = 13·2^-512, though, and the first bound is read over the whole
    // arc, where the cache is still coarse: the box wide enough to hold every point within its
    // reach runs down the cone to that edge. There the cone's circle has a radius of 13·2^-512, so
    // the cone moves far too slowly with u for that box to hold the reach, and the bound on how
    // fast its normal turns, which grows as 1/v² towards the apex, is more than half the largest
    // double. Such a piece has no bound of its own: it is halved until its boxes stay by the arc,
    // which they soon do. Under Miri, where the fit is minutes, the arc is the middle half of that
    // one, from u = 3/16 to 5/16: its first piece's wide box still runs down to the edge, and the
    // piece is still halved, three times where the longer arc's is four.
    let low = 13.0 * pow2(-512);
    let domain = ((0.0625, 0.4375), (low, 1.0));
    let cone = Cone::new([0.0; 3], 0.0, 1.0, Frame::standard().unwrap(), domain).unwrap();
    let cone = SurfaceKind::Cone(cone);
    let flat = plane(
        [0.0, 0.0, 0.25],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        ((-4.0, 4.0), (-4.0, 4.0)),
    );
    let at = |u: f64| {
        let (cos, sin) = (cospi(u).unwrap().get(), sinpi(u).unwrap().get());
        [u, 0.25, 0.25 * cos, 0.25 * sin]
    };
    let (u0, u1) = miri::case((0.125, 0.375), (0.1875, 0.3125));
    let arc = branch(vec![at(u0), at(u0 / 2.0 + u1 / 2.0), at(u1)]);
    let ceiling = 0.0625;
    let mut meter = WorkMeter::new(Budget::of_work(1 << 20));
    let curve = fit((&cone, &flat), REFS, &arc, ceiling, &mut meter)
        .unwrap_or_else(|e| panic!("{e:?} for an arc the surfaces cross cleanly"));
    let e = curve.error();
    assert!(e <= ceiling, "{e}");
    // Along the arc x and y both stay between the cosines of its ends, a quarter of each: between
    // cos(3π/8)/4 and cos(π/8)/4 for the longer arc.
    let (least, most) = (at(u1)[2], at(u0)[2]);
    let (lo, hi) = ParamCurve::domain(&curve);
    for k in 0..=64 {
        let t = lo + (hi - lo) * f64::from(k) / 64.0;
        let t = if k == 64 { hi } else { t };
        let [x, y, z] = ParamCurve::point(&curve, t, Limit::FromLeft).unwrap();
        let (out, up) = ((x * x + y * y).sqrt() - 0.25, z - 0.25);
        assert!((out * out + up * up).sqrt() <= e, "{t}: ({x}, {y}, {z})");
        assert!(least - e <= x && x <= most + e, "{t}: ({x}, {y}, {z})");
        assert!(least - e <= y && y <= most + e, "{t}: ({x}, {y}, {z})");
    }
}
