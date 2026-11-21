//! The fit and the true point on inputs whose answers are known exactly: a profile that jumps at
//! the end of its trimmed domain, exact planes far from the origin with bounds of zero, and a wide box
//! about a point of a circle that holds the bowl's bottom, where the two surfaces are parallel but
//! apart, and a parabola that touches the edge of a trimmed plane.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly; the names are chapter 12.12's"
)]

use geom_curve::{Basis as Leaf, IntersectionCurve, ParamCurve, Span, SurfaceRef, Trimmed};
use geom_intersect::IntersectError;
use geom_intersect::fit::{fit, true_point};
use geom_intersect::march::Branch;
use geom_intersect::result::CurveEnd;
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Curve, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Extrusion, ParamSurface, Plane, SurfaceKind};

const REFS: [SurfaceRef; 2] = [SurfaceRef(1), SurfaceRef(2)];
const CEILING: f64 = 1.0 / 256.0;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
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
fn a_profile_that_jumps_at_the_end_of_its_trimmed_domain_is_read_from_inside() {
    // The profile is degree 1 with a double knot at 1: (u, 0, 0) up to u = 1, then a jump to
    // (5, 5, 5). Trimmed to [0, 1] and swept along z it is the wall x = u, y = 0, and its points at
    // u = 1 are the limit from inside, (1, 0, 0, then z). Read from the right there the profile gives
    // (5, 5, 5) instead. The wall meets the floor z = 0 in the segment from (1/4, 0, 0) to (1, 0, 0),
    // the branch's two vertices, which is the answer.
    let basis = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0]).unwrap();
    let points = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [5.0, 5.0, 5.0],
        [6.0, 5.0, 5.0],
    ];
    let profile = Curve::new(basis, &points).unwrap();
    let profile = Trimmed::new(Leaf::Nurbs(profile), (0.0, 1.0)).unwrap();
    let wall =
        SurfaceKind::Extrusion(Extrusion::new(profile, [0.0, 0.0, 1.0], (-1.0, 1.0)).unwrap());
    let floor = plane(
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        ((-4.0, 4.0), (-4.0, 4.0)),
    );
    let two = branch(vec![[0.25, 0.0, 0.25, 0.0], [1.0, 0.0, 1.0, 0.0]]);
    let curve = fit((&wall, &floor), REFS, &two, CEILING, &mut meter()).unwrap();
    let e = curve.error();
    assert!(e <= CEILING, "{e}");
    let (lo, hi) = ParamCurve::domain(&curve);
    for k in 0..=64 {
        let t = lo + (hi - lo) * f64::from(k) / 64.0;
        let t = if k == 64 { hi } else { t };
        let [x, y, z] = ParamCurve::point(&curve, t, Limit::FromLeft).unwrap();
        assert!(y.abs() <= e && z.abs() <= e, "{t}: ({x}, {y}, {z})");
        assert!(0.25 - e <= x && x <= 1.0 + e, "{t}: ({x}, {y}, {z})");
    }
    let [x, ..] = ParamCurve::point(&curve, hi, Limit::FromLeft).unwrap();
    assert!((x - 1.0).abs() <= e, "{x}");
}

#[test]
fn a_true_point_far_out_on_exact_planes_is_proven_under_bounds_of_zero() {
    // The planes z = 0 and y = 0 meet in the x axis. Around x = 2^20 the cache and both pcurves are
    // the axis itself, exact in doubles, so every bound is truly 0, and the true point in the plane
    // across the cache at t = 2^20 is (2^20, 0) on both. The box about the pcurves' points is then
    // only the few steps of the double at 2^20 that stand in for a bound of zero.
    let far = 1_048_576.0;
    let domain = ((-4.0 * far, 4.0 * far), (-4.0, 4.0));
    let first = plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], domain);
    let second = plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], domain);
    let curve = made(
        (far - 1.0, far + 1.0),
        ([far - 1.0, 0.0, 0.0], [far + 1.0, 0.0, 0.0]),
        ([[far - 1.0, 0.0]; 2], [[far + 1.0, 0.0]; 2]),
        (0.0, [0.0; 2]),
    );
    let x = true_point(&first, &second, &curve, far, &mut meter())
        .unwrap()
        .unwrap_or_else(|| panic!("no true point at {far}"));
    for (i, want) in x.into_iter().zip([far, 0.0, far, 0.0]) {
        assert!(holds(i, want), "{x:?}");
    }
}

#[test]
fn a_true_point_in_a_box_over_the_bowls_bottom_is_still_proven() {
    // The plane z = c cuts the bowl z = x² + y² − 1/4 in the circle of radius r, r² = c + 1/4. The
    // cache is the chord at y = ±3/16, x = √(r² − 9/256), which the arc leaves by r − x < 0.03, so a
    // bound of 1/4 on the curve is true; the bowl's points over it are at most 9/256 < 0.05 off the
    // chord in z, r² − x², the plane's not at all. Under those wide bounds the box about the pcurves'
    // points at t = 0 reaches past the bowl's bottom, where the surfaces are parallel and 3/8 apart,
    // but not to the far side of the circle: the one zero in it is (r, 0) on both.
    let along = || Basis::new(2, vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for (x, sx) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
        for (y, sy) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
            net.push([x, y, sx + sy - 0.25]);
        }
    }
    let bowl = SurfaceKind::Nurbs(Surface::new(along(), along(), &net).unwrap());
    let c = 0.125;
    let flat = plane(
        [0.0, 0.0, c],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        ((-4.0, 4.0), (-4.0, 4.0)),
    );
    let r = (c + 0.25_f64).sqrt();
    let y = 3.0 / 16.0;
    let x = (c + 0.25 - y * y).sqrt();
    assert!(r - x < 0.03);
    let curve = made(
        (-y, y),
        ([x, -y, c], [x, y, c]),
        ([[x, -y]; 2], [[x, y]; 2]),
        (0.25, [0.05, 0.0]),
    );
    let z = true_point(&bowl, &flat, &curve, 0.0, &mut meter())
        .unwrap()
        .unwrap_or_else(|| panic!("no true point"));
    for u in [z[0], z[2]] {
        // r² = 3/8 lies inside the square of u: u holds r, not −r.
        assert!(u.lo() > 0.0, "{z:?}");
        assert!(
            u.lo() * u.lo() <= 0.375 && 0.375 <= u.hi() * u.hi(),
            "{z:?}"
        );
    }
    assert!(holds(z[1], 0.0) && holds(z[3], 0.0), "{z:?}");
}

#[test]
fn a_curve_that_touches_the_edge_of_a_trimmed_plane_never_leaves_it() {
    // The sheet z = y − x²/4 (quadratic in x, linear in y over [−1, 1]²) meets the plane z = c in
    // the parabola y = c + x²/4. The plane is read as (y, x) and trimmed to y ≥ c, so the parabola
    // lies on it everywhere and touches its edge y = c at x = 0, the middle vertex. Every point of
    // the true curve is in both domains, so a fitted curve's pcurves must be too: a pcurve point
    // with y < c is off the trimmed plane. Where the fit cannot bound a span within both domains its
    // answer is the refusal its contract gives, BudgetExceeded, never a curve.
    let c = 0.125;
    let along = Basis::new(2, vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]).unwrap();
    let across = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for (x, square) in [(-1.0, 1.0), (0.0, -1.0), (1.0, 1.0)] {
        for y in [-1.0, 1.0] {
            net.push([x, y, y - square / 4.0]);
        }
    }
    let sheet = SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap());
    let cut = plane(
        [0.0, 0.0, c],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
        ((c, 4.0), (-4.0, 4.0)),
    );
    let at = |x: f64| {
        let y = c + x * x / 4.0;
        [y, x, x, y]
    };
    let three = branch(vec![at(-0.6), at(0.0), at(0.7)]);
    match fit((&cut, &sheet), REFS, &three, 1.0 / 64.0, &mut meter()) {
        Ok(curve) => {
            for (p, s) in curve.pcurves().iter().zip([&cut, &sheet]) {
                let ((u0, u1), (v0, v1)) = ParamSurface::domain(s);
                let (lo, hi) = ParamCurve::domain(p);
                for k in 0..=65_536 {
                    let t = lo + (hi - lo) * f64::from(k) / 65_536.0;
                    let [u, v] = ParamCurve::point(p, t, Limit::FromRight).unwrap();
                    assert!(u0 <= u && u <= u1 && v0 <= v && v <= v1, "{t}: ({u}, {v})");
                }
            }
        }
        Err(e) => assert_eq!(e, IntersectError::BudgetExceeded),
    }
}
