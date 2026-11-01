//! Attacks on piece G173's claims: each kind's seams, poles and knot lines read exactly from its
//! data, and branches joined across a closing seam with the point kept once, in the march and in
//! the level-curve tracer, while a seam that does not close is never joined.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::contact::{Reading, analyse};
use geom_intersect::general::subdivide;
use geom_intersect::march::{Branch, Traced, march};
use geom_intersect::result::{CurveEnd, Degeneracy, PointContact, Unresolved};
use geom_intersect::seam::{Pass, Passing, charts, cuts};
use geom_intersect::{OnImplicit, Silhouette, trace};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Offset, ParamSurface, Plane, Sphere, SurfaceKind, Torus};

const FLOOR: f64 = 1e-3;
const CEILING: f64 = 1e-2;
const TURN: f64 = 2.0;

/// A meter above what any case here takes, so a march that loses its way fails, not hangs.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 22))
}

fn frame() -> Frame<3> {
    Frame::standard().unwrap()
}

fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

fn flat(height: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0625, 0.03125, height],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-9.0, 9.0), (-9.0, 9.0)),
        )
        .unwrap(),
    )
}

fn sphere(domain: ((f64, f64), (f64, f64))) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere::new([0.0; 3], 5.0, frame(), domain).unwrap())
}

fn traced(first: &SurfaceKind, second: &SurfaceKind) -> Traced {
    let found = subdivide(first, second, FLOOR, &mut meter()).unwrap();
    assert!(found.flagged.is_empty(), "{:?}", found.flagged);
    march(first, second, &found, FLOOR, CEILING, &mut meter()).unwrap()
}

/// Every vertex of a branch on both surfaces.
fn on_both(first: &SurfaceKind, second: &SurfaceKind, branch: &Branch) {
    let sides = (Limit::FromRight, Limit::FromRight);
    for [a, b, c, d] in &branch.vertices {
        let p = first.point(mid(*a), mid(*b), sides).unwrap();
        let q = second.point(mid(*c), mid(*d), sides).unwrap();
        let gap = (p[0] - q[0]).hypot(p[1] - q[1]).hypot(p[2] - q[2]);
        assert!(gap < 4.0 * FLOOR, "{gap}");
    }
}

/// One closed branch with one pass of a seam, whose vertex holds one end of the closing parameter
/// `k` of (u₁, v₁, u₂, v₂).
fn one_loop_across(t: &Traced, k: usize, (lo, hi): (f64, f64)) -> &Branch {
    assert_eq!(t.branches.len(), 1, "{t:?}");
    assert!(t.unproven.is_empty(), "{:?}", t.unproven);
    let branch = &t.branches[0];
    assert_eq!(
        branch.ends,
        [CurveEnd::Closed, CurveEnd::Closed],
        "{branch:?}"
    );
    assert_eq!(branch.passes.len(), 1, "{branch:?}");
    let Pass { at, passing } = branch.passes[0];
    assert_eq!(passing, Passing::Seam);
    let x = branch.vertices[at][k];
    assert!(x.contains(lo) || x.contains(hi), "{x:?}");
    branch
}

#[test]
fn round_kinds_close_only_over_one_whole_turn() {
    assert_eq!(
        charts(&sphere(((0.0, TURN), (-0.5, 0.5)))).closes,
        [true, false]
    );
    // Not a turn: a band short of one, and a span whose difference rounds to 2 but falls short.
    assert_eq!(
        charts(&sphere(((0.0, 1.5), (-0.5, 0.5)))).closes,
        [false, false]
    );
    let start = 2.0_f64.powi(-60);
    assert_eq!(
        TURN - start,
        TURN,
        "the test needs a difference that rounds to 2"
    );
    assert_eq!(
        charts(&sphere(((start, TURN), (-0.5, 0.5)))).closes,
        [false, false]
    );
    // Shifted by an exact amount, it still closes.
    assert_eq!(
        charts(&sphere(((0.25, 2.25), (-0.5, 0.5)))).closes,
        [true, false]
    );
    let can = Cylinder::new([0.0; 3], 2.0, frame(), ((-1.0, 1.0), (-3.0, 3.0))).unwrap();
    assert_eq!(charts(&SurfaceKind::Cylinder(can)).closes, [true, false]);
    let ring = Torus::new([0.0; 3], 5.0, 1.0, frame(), ((0.0, TURN), (-1.0, 1.0))).unwrap();
    assert_eq!(charts(&SurfaceKind::Torus(ring)).closes, [true, true]);
    assert_eq!(charts(&flat(0.0)).closes, [false, false]);
}

#[test]
fn poles_are_read_exactly_from_each_kind() {
    let whole = charts(&sphere(((0.0, TURN), (-0.5, 0.5))));
    assert_eq!(whole.poles, (vec![], vec![-0.5, 0.5]));
    let band = charts(&sphere(((0.0, TURN), (-0.25, 0.5))));
    assert_eq!(band.poles, (vec![], vec![0.5]));
    // A cone whose radius r₀ + k·v is zero at an end, inside, and nowhere.
    let tip = Cone::new([0.0; 3], 0.0, 0.5, frame(), ((0.0, TURN), (0.0, 4.0))).unwrap();
    assert_eq!(charts(&SurfaceKind::Cone(tip)).poles, (vec![], vec![0.0]));
    let both = Cone::new([0.0; 3], 1.0, 0.5, frame(), ((0.0, TURN), (-4.0, 4.0))).unwrap();
    let both = charts(&SurfaceKind::Cone(both));
    assert_eq!(both.poles, (vec![], vec![-2.0]));
    assert_eq!(cuts(&both, &cone_on((-4.0, 4.0))).1, vec![-2.0]);
    // −r₀/k is no double here: k·v is never exactly −r₀, so no apex is claimed.
    let third = Cone::new([0.0; 3], 1.0, 3.0, frame(), ((0.0, TURN), (-4.0, 4.0))).unwrap();
    assert_eq!(charts(&SurfaceKind::Cone(third)).poles, (vec![], vec![]));
    // A horn torus (R = r) closes on its centre at v = ±1; a ring torus has no pole.
    let horn = Torus::new([0.0; 3], 2.0, 2.0, frame(), ((0.0, TURN), (-1.0, 1.0))).unwrap();
    assert_eq!(
        charts(&SurfaceKind::Torus(horn)).poles,
        (vec![], vec![-1.0, 1.0])
    );
    let ring = Torus::new([0.0; 3], 5.0, 1.0, frame(), ((0.0, TURN), (-1.0, 1.0))).unwrap();
    assert_eq!(charts(&SurfaceKind::Torus(ring)).poles, (vec![], vec![]));
}

fn cone_on(v: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Cone(Cone::new([0.0; 3], 1.0, 0.5, frame(), ((0.0, TURN), v)).unwrap())
}

/// A rational quadratic circle of radius `r` in u (nine points, the first and last one), and a
/// line in v from z = −8 to z = 8.
fn nurbs_cylinder(r: f64, last: [f64; 2]) -> SurfaceKind {
    let w = core::f64::consts::FRAC_1_SQRT_2;
    let ku = Basis::new(
        2,
        vec![
            0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
        ],
    )
    .unwrap();
    let kv = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let ring = [
        [r, 0.0],
        [r, r],
        [0.0, r],
        [-r, r],
        [-r, 0.0],
        [-r, -r],
        [0.0, -r],
        [r, -r],
        last,
    ];
    let mut points = Vec::new();
    let mut weights = Vec::new();
    for (i, [x, y]) in ring.iter().enumerate() {
        for z in [-8.0, 8.0] {
            points.push([*x, *y, z]);
            weights.push(if i % 2 == 1 { w } else { 1.0 });
        }
    }
    SurfaceKind::Nurbs(Surface::new_rational(ku, kv, &points, &weights).unwrap())
}

#[test]
fn a_nurbs_closes_where_its_first_and_last_rows_are_one_row() {
    assert_eq!(
        charts(&nurbs_cylinder(5.0, [5.0, 0.0])).closes,
        [true, false]
    );
    // One coordinate off by the least amount: not one row, so not closed.
    let off = 5.0_f64.next_up();
    assert_eq!(
        charts(&nurbs_cylinder(5.0, [off, 0.0])).closes,
        [false, false]
    );
}

#[test]
fn a_nurbs_pole_and_a_nurbs_crease_are_read_from_its_net_and_knots() {
    // A cone of degree 1 in v: every point of the row at v = 0 is its apex.
    let ku = Basis::new(1, vec![0.0, 0.0, 0.5, 1.0, 1.0]).unwrap();
    let kv = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let net = [
        [0.0, 0.0, 0.0],
        [-4.0, -4.0, 4.0],
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 4.0],
        [0.0, 0.0, 0.0],
        [-4.0, 4.0, 4.0],
    ];
    let cone = charts(&SurfaceKind::Nurbs(Surface::new(ku, kv, &net).unwrap()));
    assert_eq!(cone.poles, (vec![], vec![0.0]));
    // Degree 1 at an inner knot: only C⁰, a crease.
    assert_eq!(cone.creases, (vec![0.5], vec![]));
    // Degree 3 at a simple inner knot: C², no crease.
    let smooth = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let line = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for i in 0..5 {
        for j in 0..2 {
            net.push([f64::from(i), f64::from(j), f64::from(i * i)]);
        }
    }
    let curved = charts(&SurfaceKind::Nurbs(
        Surface::new(smooth, line, &net).unwrap(),
    ));
    assert_eq!(curved.creases, (vec![], vec![]));
}

#[test]
fn an_offset_closes_where_its_round_base_does() {
    let base = sphere(((0.0, TURN), (-0.5, 0.5)));
    let off = SurfaceKind::Offset(Offset::new(base, 1.0).unwrap());
    let read = charts(&off);
    assert_eq!(read.closes, [true, false]);
    // An offset's poles are not claimed: its edge there is a point only where the base's normal
    // has one limit, which this piece does not read.
    assert_eq!(read.poles, (vec![], vec![]));
}

#[test]
fn a_plane_through_a_whole_sphere_is_one_loop_across_its_seam() {
    let ball = sphere(((0.0, TURN), (-0.5, 0.5)));
    let plane = flat(1.0);
    let t = traced(&ball, &plane);
    let branch = one_loop_across(&t, 0, (0.0, TURN));
    on_both(&ball, &plane, branch);
    // The circle z = 1 on a sphere of radius 5: v = asin(1/5)/π everywhere.
    let v = (0.2_f64).asin() / core::f64::consts::PI;
    for x in &branch.vertices {
        assert!((mid(x[1]) - v).abs() < 1e-6, "{x:?}");
    }
    // u turns once round: every step but the pass moves u the same way.
    let n = branch.vertices.len();
    let mut ups = 0;
    let mut downs = 0;
    for k in 0..n {
        let (a, b) = (branch.vertices[k][0], branch.vertices[(k + 1) % n][0]);
        if a.hi() < b.lo() {
            ups += 1;
        } else if b.hi() < a.lo() {
            downs += 1;
        }
    }
    assert!(ups == 1 || downs == 1, "{ups} {downs}");
    assert_eq!(ups + downs, n);
}

#[test]
fn the_seam_join_does_not_care_which_surface_is_first() {
    let ball = sphere(((0.0, TURN), (-0.5, 0.5)));
    let plane = flat(1.0);
    let swapped = traced(&plane, &ball);
    let branch = one_loop_across(&swapped, 2, (0.0, TURN));
    on_both(&plane, &ball, branch);
}

#[test]
fn a_tilted_plane_through_a_cylinder_is_one_loop_across_its_seam() {
    let can = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 3.0, frame(), ((0.0, TURN), (-20.0, 20.0))).unwrap(),
    );
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 1.0],
            ((-9.0, 9.0), (-9.0, 9.0)),
        )
        .unwrap(),
    );
    let t = traced(&can, &plane);
    let branch = one_loop_across(&t, 0, (0.0, TURN));
    on_both(&can, &plane, branch);
    for x in &branch.vertices {
        let (u, v) = (mid(x[0]), mid(x[1]));
        assert!(
            (v - 3.0 * (core::f64::consts::PI * u).sin()).abs() < 1e-6,
            "{u} {v}"
        );
    }
}

#[test]
fn a_plane_through_a_whole_cone_is_one_loop_across_its_seam() {
    let cone = cone_on((0.0, 4.0));
    let plane = flat(1.0);
    let t = traced(&cone, &plane);
    let branch = one_loop_across(&t, 0, (0.0, TURN));
    on_both(&cone, &plane, branch);
}

#[test]
fn a_plane_along_a_ring_torus_cuts_two_loops_across_its_v_seam() {
    let ring = SurfaceKind::Torus(
        Torus::new([0.0; 3], 5.0, 1.0, frame(), ((0.0625, 2.0625), (0.0, TURN))).unwrap(),
    );
    // y = 1/32: two small loops round the tube, near u = 0 and near u = 1, each crossing v's seam.
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.03125, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            ((-9.0, 9.0), (-9.0, 9.0)),
        )
        .unwrap(),
    );
    let t = traced(&ring, &plane);
    assert_eq!(t.branches.len(), 2, "{t:?}");
    for branch in &t.branches {
        assert_eq!(
            branch.ends,
            [CurveEnd::Closed, CurveEnd::Closed],
            "{branch:?}"
        );
        assert_eq!(branch.passes.len(), 1, "{branch:?}");
        assert_eq!(branch.passes[0].passing, Passing::Seam);
        let x = branch.vertices[branch.passes[0].at][1];
        assert!(x.contains(0.0) || x.contains(TURN), "{x:?}");
        on_both(&ring, &plane, branch);
    }
}

#[test]
fn a_rational_cylinder_closes_like_the_round_one() {
    let can = nurbs_cylinder(5.0, [5.0, 0.0]);
    let plane = flat(1.0);
    let t = traced(&can, &plane);
    let branch = one_loop_across(&t, 0, (0.0, 1.0));
    on_both(&can, &plane, branch);
}

#[test]
fn a_band_short_of_a_turn_is_never_joined() {
    let band = sphere(((0.0, 1.9375), (-0.5, 0.5)));
    let plane = flat(1.0);
    let t = traced(&band, &plane);
    assert_eq!(t.branches.len(), 1, "{t:?}");
    let branch = &t.branches[0];
    assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
    assert!(branch.passes.is_empty(), "{branch:?}");
}

#[test]
fn a_nurbs_whose_last_row_misses_the_first_is_never_joined() {
    // The rows differ by the least amount, so the domain's ends are not one curve, and the circle
    // ends at both (as it must: nothing proves the gap closed).
    let off = 5.0_f64.next_up();
    let can = nurbs_cylinder(5.0, [off, 0.0]);
    let plane = flat(1.0);
    let t = traced(&can, &plane);
    assert!(
        t.branches
            .iter()
            .all(|b| b.passes.is_empty() && b.ends != [CurveEnd::Closed, CurveEnd::Closed]),
        "{t:?}"
    );
}

#[test]
fn a_silhouette_across_a_seam_is_one_loop() {
    let ball = sphere(((0.0, TURN), (-0.4, 0.4)));
    let level = Silhouette {
        direction: [0.0, 0.0, 2.0],
    };
    let found = trace(&ball, &level, FLOOR, CEILING, &mut meter()).unwrap();
    assert!(found.flagged.is_empty(), "{:?}", found.flagged);
    assert_eq!(found.branches.len(), 1, "{found:?}");
    let branch = &found.branches[0];
    assert_eq!(branch.ends, [CurveEnd::Closed, CurveEnd::Closed]);
    assert_eq!(branch.passes.len(), 1, "{branch:?}");
    let Pass { at, passing } = branch.passes[0];
    assert_eq!(passing, Passing::Seam);
    let [u, v] = branch.vertices[at];
    assert!(u.contains(0.0) || u.contains(TURN), "{u:?}");
    assert!(v.contains(0.0), "{v:?}");
}

#[test]
fn a_level_curve_on_a_whole_torus_closes_in_both_parameters() {
    // The plane y = 1/32 on a whole ring torus: two loops round the tube, each across v's seam.
    let ring = SurfaceKind::Torus(
        Torus::new([0.0; 3], 5.0, 1.0, frame(), ((0.0625, 2.0625), (0.0, TURN))).unwrap(),
    );
    let plane = Plane::new(
        [0.0, 0.03125, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        ((-9.0, 9.0), (-9.0, 9.0)),
    )
    .unwrap();
    let found = trace(&ring, &OnImplicit(&plane), FLOOR, CEILING, &mut meter()).unwrap();
    assert!(found.flagged.is_empty(), "{:?}", found.flagged);
    assert_eq!(found.branches.len(), 2, "{found:?}");
    for branch in &found.branches {
        assert_eq!(
            branch.ends,
            [CurveEnd::Closed, CurveEnd::Closed],
            "{branch:?}"
        );
        assert_eq!(branch.passes.len(), 1, "{branch:?}");
        let [_, v] = branch.vertices[branch.passes[0].at];
        assert!(v.contains(0.0) || v.contains(TURN), "{v:?}");
    }
}

#[test]
fn a_level_curve_across_a_crease_names_the_knot_line() {
    // A tent, degree 1 in u with its ridge at the knot u = 1/2, and the plane y = 1/32 across the
    // ridge, traced as the level curve of the plane's equation on the tent: it crosses once.
    let ku = Basis::new(1, vec![0.0, 0.0, 0.5, 1.0, 1.0]).unwrap();
    let kv = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let net = [
        [-4.0, -4.0, 0.0],
        [-4.0, 4.0, 0.0],
        [0.0625, -4.0, 4.0],
        [0.0625, 4.0, 4.0],
        [4.0, -4.0, 0.0],
        [4.0, 4.0, 0.0],
    ];
    let tent = SurfaceKind::Nurbs(Surface::new(ku, kv, &net).unwrap());
    let across = Plane::new(
        [0.0, 0.03125, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        ((-9.0, 9.0), (-9.0, 9.0)),
    )
    .unwrap();
    let found = trace(&tent, &OnImplicit(&across), FLOOR, CEILING, &mut meter()).unwrap();
    assert!(found.flagged.is_empty(), "{:?}", found.flagged);
    assert_eq!(found.branches.len(), 1, "{found:?}");
    let branch = &found.branches[0];
    assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
    assert_eq!(branch.passes.len(), 1, "{branch:?}");
    let Pass { at, passing } = branch.passes[0];
    assert_eq!(passing, Passing::KnotLine);
    assert!(branch.vertices[at][0].contains(0.5), "{branch:?}");
}

/// The plane x = 0, holding the z axis: through a sphere's poles and a cone's apex.
fn upright() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        )
        .unwrap(),
    )
}

/// The passes of `branch` that are `passing`, and each one's vertex.
fn passes_of(branch: &Branch, passing: Passing) -> Vec<[Interval; 4]> {
    branch
        .passes
        .iter()
        .filter(|p| p.passing == passing)
        .map(|p| branch.vertices[p.at])
        .collect()
}

#[test]
fn a_plane_through_both_poles_is_one_loop_through_them() {
    let ball = sphere(((0.0, TURN), (-0.5, 0.5)));
    let plane = upright();
    let t = traced(&ball, &plane);
    assert_eq!(t.branches.len(), 1, "{t:?}");
    assert!(t.unproven.is_empty(), "{:?}", t.unproven);
    let branch = &t.branches[0];
    assert_eq!(branch.ends, [CurveEnd::Closed, CurveEnd::Closed]);
    on_both(&ball, &plane, branch);
    // The great circle in x = 0: the meridians u = 1/2 and u = 3/2, joined at both poles, each
    // point kept once and held on its pole line.
    let poles = passes_of(branch, Passing::Pole);
    assert_eq!(poles.len(), 2, "{branch:?}");
    let mut held: Vec<f64> = poles.iter().map(|x| x[1].lo()).collect();
    held.sort_by(f64::total_cmp);
    assert_eq!(held, vec![-0.5, 0.5]);
    for x in &poles {
        assert_eq!(x[1].lo(), x[1].hi());
    }
    for x in &branch.vertices {
        let u = mid(x[0]);
        let pole = x[1].lo().abs() == 0.5;
        assert!(
            pole || (u - 0.5).abs() < 1e-6 || (u - 1.5).abs() < 1e-6,
            "{x:?}"
        );
    }
}

#[test]
fn the_pole_join_does_not_care_which_surface_is_first() {
    let ball = sphere(((0.0, TURN), (-0.5, 0.5)));
    let plane = upright();
    let t = traced(&plane, &ball);
    assert_eq!(t.branches.len(), 1, "{t:?}");
    let branch = &t.branches[0];
    assert_eq!(branch.ends, [CurveEnd::Closed, CurveEnd::Closed]);
    assert_eq!(passes_of(branch, Passing::Pole).len(), 2, "{branch:?}");
    on_both(&plane, &ball, branch);
}

#[test]
fn a_plane_past_the_poles_crosses_the_seam_twice_and_no_pole() {
    // x = 1/4: a circle round the x axis, through the seam's half-plane at z = ±√(25 − 1/16).
    let ball = sphere(((0.0, TURN), (-0.5, 0.5)));
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.25, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        )
        .unwrap(),
    );
    let t = traced(&ball, &plane);
    assert_eq!(t.branches.len(), 1, "{t:?}");
    let branch = &t.branches[0];
    assert_eq!(branch.ends, [CurveEnd::Closed, CurveEnd::Closed]);
    assert!(passes_of(branch, Passing::Pole).is_empty(), "{branch:?}");
    let seams = passes_of(branch, Passing::Seam);
    assert_eq!(seams.len(), 2, "{branch:?}");
    for x in &seams {
        assert!(x[0].contains(0.0) || x[0].contains(TURN), "{x:?}");
    }
    on_both(&ball, &plane, branch);
}

#[test]
fn a_plane_through_a_cones_apex_is_one_branch_through_it() {
    // One nappe, r₀ = 0: the plane x = 0 holds its generators at u = 1/2 and 3/2, which meet only
    // at the apex. Two half-branches end there, so they are one branch, turning at the pole.
    let tip = Cone::new([0.0; 3], 0.0, 1.0, frame(), ((0.0, TURN), (0.0, 4.0))).unwrap();
    let cone = SurfaceKind::Cone(tip);
    let plane = upright();
    let t = traced(&cone, &plane);
    assert_eq!(t.branches.len(), 1, "{t:?}");
    assert!(t.unproven.is_empty(), "{:?}", t.unproven);
    let branch = &t.branches[0];
    assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
    let apex = passes_of(branch, Passing::Pole);
    assert_eq!(apex.len(), 1, "{branch:?}");
    assert_eq!((apex[0][1].lo(), apex[0][1].hi()), (0.0, 0.0));
    on_both(&cone, &plane, branch);
}

#[test]
fn a_plane_through_a_double_cones_apex_ends_four_half_branches_there() {
    // Both nappes: the plane holds two whole lines, crossing at the apex, a singular point of four
    // half-branches. Each ends there (12.8, step 8), none joined through it.
    let both = Cone::new([0.0; 3], 0.0, 1.0, frame(), ((0.0, TURN), (-4.0, 4.0))).unwrap();
    let cone = SurfaceKind::Cone(both);
    let plane = upright();
    let t = traced(&cone, &plane);
    assert_eq!(t.branches.len(), 4, "{t:?}");
    assert!(t.unproven.is_empty(), "{:?}", t.unproven);
    for branch in &t.branches {
        let mut ends = branch.ends;
        ends.sort_by_key(|e| *e == CurveEnd::Singular);
        assert_eq!(ends, [CurveEnd::Boundary, CurveEnd::Singular], "{branch:?}");
        assert!(branch.passes.is_empty(), "{branch:?}");
        on_both(&cone, &plane, branch);
    }
}

#[test]
fn a_plane_through_a_nurbs_apex_is_one_branch_through_it() {
    // Degree 1 in both: two flat facets from the apex at the origin (the row at v = 0 is one
    // point), meeting along the crease u = 1/2. The plane x = 0 cuts each facet along a ray from
    // the apex, at u = 1/4 and u = 3/4.
    let ku = Basis::new(1, vec![0.0, 0.0, 0.5, 1.0, 1.0]).unwrap();
    let kv = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let net = [
        [0.0, 0.0, 0.0],
        [-4.0, -4.0, 4.0],
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 4.0],
        [0.0, 0.0, 0.0],
        [-4.0, 4.0, 4.0],
    ];
    let pyramid = SurfaceKind::Nurbs(Surface::new(ku, kv, &net).unwrap());
    let plane = upright();
    let t = traced(&pyramid, &plane);
    assert_eq!(t.branches.len(), 1, "{t:?}");
    assert!(t.unproven.is_empty(), "{:?}", t.unproven);
    let branch = &t.branches[0];
    assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
    assert_eq!(passes_of(branch, Passing::Pole).len(), 1, "{branch:?}");
    on_both(&pyramid, &plane, branch);
}

#[test]
fn a_plane_touching_only_a_cones_apex_is_one_point() {
    // z = x/4 through the apex of a cone whose generators rise at 45°: the plane is shallower than
    // every generator, so it meets the cone at the apex alone. Nothing leaves the apex: subdivision
    // keeps it to the floor as a pole, and the analysis reads one singular point there with no
    // branches, never nothing.
    let tip = Cone::new([0.0; 3], 0.0, 1.0, frame(), ((0.0, TURN), (0.0, 4.0))).unwrap();
    let cone = SurfaceKind::Cone(tip);
    let shallow = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.25],
            [0.0, 1.0, 0.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        )
        .unwrap(),
    );
    let found = subdivide(&cone, &shallow, FLOOR, &mut meter()).unwrap();
    assert!(found.settled.is_empty(), "{found:?}");
    assert!(!found.flagged.is_empty(), "{found:?}");
    for pair in &found.flagged {
        assert_eq!(
            pair.why,
            Unresolved::NonSmooth {
                at: Degeneracy::Pole
            }
        );
    }
    let read = analyse(&cone, &shallow, &found, FLOOR, CEILING, &mut meter()).unwrap();
    assert_eq!(read.len(), 1, "{read:?}");
    let Reading::Point { at, gap, contact } = read[0] else {
        panic!("{read:?}");
    };
    assert_eq!(contact, PointContact::Singular { branches: 0 });
    assert!(gap.contains(0.0), "{gap:?}");
    assert_eq!((at[1].lo(), at[1].hi()), (0.0, 0.0));
    let t = march(&cone, &shallow, &found, FLOOR, CEILING, &mut meter()).unwrap();
    assert!(t.branches.is_empty(), "{t:?}");
}

#[test]
fn a_sphere_resting_on_a_plane_at_its_pole_touches_it_at_one_point() {
    // The standard frame's south pole rests on z = 0: the plane touches the sphere there and
    // nowhere else. In the sphere's own chart the pole has no tangent plane to read 12.9's forms
    // in, so the analysis reads it in a chart turned to put the pole on the equator.
    let ball = SurfaceKind::Sphere(
        Sphere::new(
            [0.0625, 0.03125, 5.0],
            5.0,
            frame(),
            ((0.0, TURN), (-0.5, 0.5)),
        )
        .unwrap(),
    );
    let ground = flat(0.0);
    // A floor of 1e-2, as a tangency flags every pair its normals agree on down to the floor.
    let (touch, room) = (1e-2, || WorkMeter::new(Budget::of_work(1 << 24)));
    for (one, two, swapped) in [(&ball, &ground, false), (&ground, &ball, true)] {
        let found = subdivide(one, two, touch, &mut room()).unwrap();
        assert!(found.settled.is_empty(), "{found:?}");
        let read = analyse(one, two, &found, touch, CEILING, &mut room()).unwrap();
        assert_eq!(read.len(), 1, "{read:?}");
        let Reading::Point { at, gap, contact } = read[0] else {
            panic!("{read:?}");
        };
        assert_eq!(contact, PointContact::IsolatedTangent, "{read:?}");
        assert!(gap.contains(0.0), "{gap:?}");
        let v = if swapped { at[3] } else { at[1] };
        assert_eq!((v.lo(), v.hi()), (-0.5, -0.5));
    }
}
