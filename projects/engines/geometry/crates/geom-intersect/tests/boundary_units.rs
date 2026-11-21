//! An edge that lies in a surface whose two parameters are not one unit (G173a, chapter 12.8 step
//! 5). The proof of such an edge takes a box of the other surface's `(s, r)` about each piece of it,
//! and the box's radius is a length: a little more than the edge's travel over the piece. Each
//! parameter has to take that length in its own unit. A plane may be written with one vector a
//! step long and the other a unit long, and a cylinder's u is in half-turns while its v is in
//! steps; with one number for both, the box of the parameter that does not move was as wide, in
//! its unit, as the edge is long in the other's, and the edge was never proven.
//!
//! Every case has an answer known exactly, a straight line through whole numbers, and each vertex
//! of the branch is held against it: the line's own coordinates in both surfaces, and where the
//! pieces of the proof end. Run at the scale of the SSI corpus (1024 steps to the unit, floor 8,
//! ceiling 10.24), in both orders of the pair.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::general::subdivide;
use geom_intersect::march::{Branch, march};
use geom_intersect::result::CurveEnd;
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, Plane, SurfaceKind};

const UNIT: f64 = 1024.0;
const FLOOR: f64 = 8.0;
const CEILING: f64 = 10.24;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

/// The wall x = 0, u along y and v along z, both in units.
fn wall(u: (f64, f64), v: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new([0.0; 3], [0.0, UNIT, 0.0], [0.0, 0.0, UNIT], (u, v)).unwrap())
}

/// How a strip's two parameters are written.
#[derive(Clone, Copy, Debug)]
enum Written {
    /// Both in units.
    Units,
    /// u, across the strip, in units, and v, along it, in steps.
    AlongInSteps,
    /// u in steps and v in units.
    AcrossInSteps,
}

/// The strip of the plane z = 0 from x = −`half` to `half` units and y = 0 to 2 units, u along x
/// and v along y, and the steps to one of u and to one of v. The three writings are one patch.
fn strip(half: f64, written: Written) -> (SurfaceKind, [f64; 2]) {
    let [across, along] = match written {
        Written::Units => [UNIT, UNIT],
        Written::AlongInSteps => [UNIT, 1.0],
        Written::AcrossInSteps => [1.0, UNIT],
    };
    let domain = (
        (-half * UNIT / across, half * UNIT / across),
        (0.0, 2.0 * UNIT / along),
    );
    let plane = Plane::new([0.0; 3], [across, 0.0, 0.0], [0.0, along, 0.0], domain).unwrap();
    (SurfaceKind::Plane(plane), [across, along])
}

/// The cylinder of radius one unit about z from its quarter-turn to its three-quarter-turn, `tall`
/// units high from z = 0: u in half-turns, v in steps.
fn tube(tall: f64) -> SurfaceKind {
    let domain = ((0.25, 0.75), (0.0, tall * UNIT));
    SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], UNIT, Frame::standard().unwrap(), domain).unwrap(),
    )
}

/// The one branch of the pair: nothing flagged, every step proven, an edge of a domain at both
/// ends.
fn branch(a: &SurfaceKind, b: &SurfaceKind) -> Branch {
    let found = subdivide(a, b, FLOOR, &mut meter()).unwrap();
    assert!(found.flagged.is_empty(), "{:#?}", found.flagged);
    let traced = march(a, b, &found, FLOOR, CEILING, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{:#?}", traced.unproven);
    let [branch] = traced.branches.as_slice() else {
        panic!("{:#?}", traced.branches);
    };
    assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
    branch.clone()
}

/// Whether `x` holds `at`.
fn holds(x: Interval, at: f64) -> bool {
    x.lo() <= at && at <= x.hi()
}

/// Whether `a` times `j` meets `b` times `k`. Both factors are powers of two, so the products are
/// exact and the test is one of exact arithmetic.
fn meets((a, j): (Interval, f64), (b, k): (Interval, f64)) -> bool {
    a.lo() * j <= b.hi() * k && b.lo() * k <= a.hi() * j
}

/// The places along the edge where the branch has a vertex, each once: the middles of parameter
/// `index`, in order.
fn places(branch: &Branch, index: usize) -> Vec<f64> {
    let mut out: Vec<f64> = branch
        .vertices
        .iter()
        .map(|v| v[index].lo() / 2.0 + v[index].hi() / 2.0)
        .collect();
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() <= 1e-9);
    out
}

/// The places are the ends of `pieces` equal pieces of an edge `long` units long.
fn evenly(at: &[f64], long: f64, pieces: u32) {
    let exact: Vec<f64> = (0..=pieces)
        .map(|k| long * f64::from(k) / f64::from(pieces))
        .collect();
    assert_eq!(at.len(), exact.len(), "{at:?}");
    for (place, exact) in at.iter().zip(&exact) {
        assert!((place - exact).abs() <= 1e-9, "{at:?}");
    }
}

/// The wall's edge z = 0, the line x = 0 from y = 0 to 2 units, lies along the middle of the strip
/// and is the pair's one branch, proven in `pieces` pieces.
fn along_the_strip(half: f64, written: Written, strip_first: bool, pieces: u32) {
    let (strip, [across, along]) = strip(half, written);
    let wall = wall((0.0, 2.0), (0.0, 1.0));
    let (found, [x, y, t, z]) = if strip_first {
        (branch(&strip, &wall), [0, 1, 2, 3])
    } else {
        (branch(&wall, &strip), [2, 3, 0, 1])
    };
    for v in &found.vertices {
        // On the line x and z are 0, and y is the strip's v and the wall's u.
        assert!(holds(v[x], 0.0) && holds(v[z], 0.0), "{written:?} {v:?}");
        assert!(meets((v[y], along), (v[t], UNIT)), "{written:?} {v:?}");
        // Across the strip the line is a point: its box is far under a step wide, in space.
        assert!(
            (v[x].hi() - v[x].lo()) * across <= 1e-6,
            "{written:?} {v:?}"
        );
    }
    evenly(&places(&found, t), 2.0, pieces);
}

/// The half width, in units, of the narrowest strip whose edge is proven in 8 pieces, on a grid of
/// 2^−20 of a unit, and the next one down.
///
/// A piece `l` units long travels `l / 2` each way from its middle, and its box is 1.05 times
/// that, across the strip as well as along it. Across, the box has the strip's half width `w` and
/// the room past each end, a sixteenth of its width, to stand in: `1.05 · l / 2 < 9 · w / 8`. For a
/// quarter of a unit, 8 pieces, that is `w > 7 / 60`, and 7/60 lies between these two.
const FITS: f64 = 122_334.0 / 1_048_576.0;
const TIGHT: f64 = 122_333.0 / 1_048_576.0;

#[test]
fn a_strip_has_the_same_branch_however_its_parameters_are_written() {
    for (half, pieces) in [(FITS, 8), (TIGHT, 16)] {
        for written in [
            Written::Units,
            Written::AlongInSteps,
            Written::AcrossInSteps,
        ] {
            along_the_strip(half, written, true, pieces);
            along_the_strip(half, written, false, pieces);
        }
    }
}

/// The wall's edge u = 1, the line x = 0, y = one unit from z = 0 to `tall` units, is the line of
/// the cylinder at u = 1/2, and the pair's one branch, proven in `pieces` pieces.
fn along_the_tube(tall: f64, wall_first: bool, pieces: u32) {
    let wall = wall((1.0, 2.0), (0.0, tall));
    let tube = tube(tall);
    let (found, [y, t, turn, height]) = if wall_first {
        (branch(&wall, &tube), [0, 1, 2, 3])
    } else {
        (branch(&tube, &wall), [2, 3, 0, 1])
    };
    for v in &found.vertices {
        // On the line the wall's u is 1, the cylinder's u is 1/2, and z is the wall's v and the
        // cylinder's.
        assert!(holds(v[y], 1.0) && holds(v[turn], 0.5), "{v:?}");
        assert!(meets((v[t], UNIT), (v[height], 1.0)), "{v:?}");
        // The turn's box is a small part of the turn, not the edge's length read as half-turns.
        assert!((v[turn].hi() - v[turn].lo()) / 2.0 <= 0.003, "{v:?}");
    }
    evenly(&places(&found, t), tall, pieces);
}

/// Pieces of half a unit. The box of a piece of a whole unit, 1.05 × 512 steps each way, is 30
/// degrees of the turn each way at the cylinder's 1024π steps to the half-turn. Krawczyk's
/// operator brings that to a box of 4 degrees, whose ends stand 2.5 steps off the wall: over the
/// eighth of the floor, one step, the third equation is held to. At half a unit the box is 15
/// degrees, its image half a degree, 0.003 of a half-turn, and 0.04 steps.
#[test]
fn a_planes_edge_along_a_cylinders_line_is_proven_in_pieces_of_half_a_unit() {
    for (tall, pieces) in [(1.0, 2), (2.0, 4)] {
        along_the_tube(tall, true, pieces);
        along_the_tube(tall, false, pieces);
    }
}

/// The plane z = 0 written with its two vectors leaning on each other, u along x and v along
/// (1, 1, 0), both in units: the point at (u, v) is (u + v, v, 0) units. The wall's edge, the line
/// x = 0 from y = 0 to 2 units, is the line u = −v from v = 0 to 2, and the domain holds it with a
/// quarter of a unit to spare across.
fn leaning() -> SurfaceKind {
    let domain = ((-2.25, 0.25), (0.0, 2.0));
    SurfaceKind::Plane(Plane::new([0.0; 3], [UNIT, 0.0, 0.0], [UNIT, UNIT, 0.0], domain).unwrap())
}

/// Where each piece of the edge stands on the other surface is found by Gauss-Newton from the
/// middle of its domain, (−1, 1), and on a plane one step lands on the point exactly. The edge is
/// proven in its two halves, whose middles stand at (−1/2, 1/2) and (−3/2, 3/2). With vectors at an
/// eighth of a turn, a step that took the cross term of the normal equations' inverse with the
/// wrong sign would leave s off by twice as much, the other way, at each step: 2^11 units after
/// the twelve, far outside the domain, and neither half would be proven. The branch is the line,
/// held exactly, ending at the two halves' ends.
#[test]
fn an_edge_along_a_plane_whose_vectors_lean_is_proven_where_it_lies() {
    let wall = wall((0.0, 2.0), (0.0, 1.0));
    let strip = leaning();
    for strip_first in [true, false] {
        let (found, [u, v, t, z]) = if strip_first {
            (branch(&strip, &wall), [0, 1, 2, 3])
        } else {
            (branch(&wall, &strip), [2, 3, 0, 1])
        };
        for p in &found.vertices {
            // On the line the wall's u is y in units, the plane's v is the same, its u is −v, and
            // z, the wall's v, is 0.
            assert!(holds(p[z], 0.0), "{p:?}");
            assert!(meets((p[v], 1.0), (p[t], 1.0)), "{p:?}");
            let minus_v = Interval::new(-p[v].hi(), -p[v].lo()).unwrap();
            assert!(meets((p[u], 1.0), (minus_v, 1.0)), "{p:?}");
        }
        evenly(&places(&found, t), 2.0, 2);
    }
}
