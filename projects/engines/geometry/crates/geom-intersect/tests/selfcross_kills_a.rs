//! What the self-crossing search (12.13) leaves where two pieces touch across a closing seam or at
//! a pole, pinned by true answers. A square ramp round twice, closed, whose two passes cross along a
//! curve over its seam, is traced whole from both sheets with nothing left undecided, either way
//! round; a ramp pinched to a point past its end still has its crossing traced. A teardrop sweep
//! cusped at its seam, and the cone over it, cross nowhere: no branch, and every flagged pair whose
//! pieces share a point (in parameters, across the seam or at the apex) is flagged as one the
//! exclusion test cannot decide, since pieces that share a point can never be proven apart.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::tight::{larger, smaller};
use geom_intersect::result::{Test, Unresolved};
use geom_intersect::selfcross::{SelfCrossing, self_intersect};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{ParamSurface, SurfaceKind};

const FLOOR: f64 = 1.0 / 128.0;
const CEILING: f64 = 1.0 / 16.0;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

/// The surface of degree 1 both ways through `net`, `rows` rows of `cols`, on clamped knots.
fn linear_net(rows: u32, cols: u32, net: &[[f64; 3]]) -> SurfaceKind {
    let knots = |n: u32| {
        let mut k = vec![0.0];
        for i in 0..n {
            k.push(f64::from(i));
        }
        k.push(f64::from(n - 1));
        Basis::new(1, k).unwrap()
    };
    SurfaceKind::Nurbs(Surface::new(knots(rows), knots(cols), net).unwrap())
}

/// The net of `rows` rows of `cols` points `at`, or with `t` its transpose.
fn either_way(rows: u32, cols: u32, at: &dyn Fn(usize, usize) -> [f64; 3], t: bool) -> SurfaceKind {
    let (m, n) = (
        usize::try_from(rows).unwrap(),
        usize::try_from(cols).unwrap(),
    );
    let mut net = Vec::new();
    if t {
        for j in 0..n {
            for i in 0..m {
                net.push(at(i, j));
            }
        }
        linear_net(cols, rows, &net)
    } else {
        for i in 0..m {
            for j in 0..n {
                net.push(at(i, j));
            }
        }
        linear_net(rows, cols, &net)
    }
}

const SQUARE: [[f64; 2]; 4] = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];

/// A square ramp round the z axis twice between radii 1 and 2 (v = r − 1), closed: its rows run
/// from M, the middle of S₀S₁, by S₁, S₂, S₃ to S₀ (u = 4), round again to S₀ (u = 8) and back to
/// M (u = 9), where it closes. The first pass is at height ½ at M and 1 from S₁ to S₀, the second
/// falls from 1 + (v − ½)/2 at S₀ to (v − ½)/2 at S₁ and is at 0 from S₂ on, rising to ½ at M.
/// Over S₀S₁, at s along it, the planform is the same for both passes and the first is at height s,
/// the second at 1 − s + (v − ½)/2, so they cross where s = 3/8 + v/4: on the first sheet at
/// u = 35/4 + v/2 for v up to ½ and at u = v/2 − ¼ past it (the curve crosses the seam at v = ½), on
/// the second at u = 35/8 + v/4, at the same v on both. Over the other sides the passes are at least
/// ¾ apart in height. With `t`, u and v trade places.
fn double_ramp(t: bool) -> SurfaceKind {
    let m = [0.5, 0.5];
    let [s0, s1, s2, s3] = SQUARE;
    let plan = [m, s1, s2, s3, s0, s1, s2, s3, s0, m];
    let z = [0.5, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5];
    either_way(
        10,
        2,
        &|i, j| {
            let r = [1.0, 2.0][j];
            let lean = if i == 4 || i == 5 {
                (r - 1.5) / 2.0
            } else {
                0.0
            };
            [r * plan[i][0], r * plan[i][1], z[i] + lean]
        },
        t,
    )
}

/// A square ramp round the z axis over a turn and a quarter between radii 1 and 2 (v = r − 1), up
/// from z = 0 to ½ over its first quarter, level, then down to 0 over its last, which lies over its
/// first, and on past its end (u = 5) to a point below, (0, 3/2, −3), its last row (u = 6), so its
/// domain's last edge in u is a pole. The triangle to the point lies in the plane x = 0 below z = 0
/// and meets the ramp only along its own first row. The first and last quarters cross where both
/// are at z = ¼, along u = ½ of one sheet and u = 9/2 of the other, at the same v. With `t`, u and
/// v trade places.
fn pinched_ramp(t: bool) -> SurfaceKind {
    let z = [0.0, 0.5, 0.5, 0.5, 0.5, 0.0];
    either_way(
        7,
        2,
        &|i, j| {
            if i == 6 {
                return [0.0, 1.5, -3.0];
            }
            let r = [1.0, 2.0][j];
            [r * SQUARE[i % 4][0], r * SQUARE[i % 4][1], z[i]]
        },
        t,
    )
}

/// A teardrop in x and z: the cusped cubic arch (−9/8, 0), (9/8, 2), (−9/8, 2), (9/8, 0) cut at its
/// cusp (0, 3/2), its right half first, then a cubic bulge below z = 0 from (9/8, 0) to (−9/8, 0),
/// tangent to the halves at their feet, then its left half back to the cusp. The right half has
/// x ≥ 0 and the left x ≤ 0, each 0 only at the cusp, both z ≥ 0, each 0 only at its foot; the bulge
/// has z = −3t(1 − t), so a crossing of it with itself would sit at t and 1 − t, where its x are
/// opposite: only its middle, x = 0. So it is a simple closed curve, cusped where it starts and ends.
const DROP: [[f64; 2]; 10] = [
    [0.0, 1.5],
    [0.0, 1.5],
    [0.0, 1.0],
    [1.125, 0.0],
    [2.25, -1.0],
    [-2.25, -1.0],
    [-1.125, 0.0],
    [0.0, 1.0],
    [0.0, 1.5],
    [0.0, 1.5],
];

/// The teardrop swept along y from −¼ to ¼ (u along the teardrop, from 0 to 3, closing at the cusp,
/// and v from −1 to 1), or with `apex` the cone over its section at y = ¼ from (0, −¼, ¾), which
/// lies inside the teardrop's planform, so its first edge in v is a pole. A cylinder or cone over a
/// simple closed curve crosses itself nowhere. With `t`, u and v trade places.
fn teardrop(apex: bool, t: bool) -> SurfaceKind {
    let across = Basis::new(
        3,
        vec![
            0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0,
        ],
    )
    .unwrap();
    let along = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let at = |i: usize, j: usize| {
        if apex && j == 0 {
            [0.0, -0.25, 0.75]
        } else {
            [DROP[i][0], [-0.25, 0.25][j], DROP[i][1]]
        }
    };
    let mut net = Vec::new();
    if t {
        for j in 0..2 {
            for i in 0..10 {
                net.push(at(i, j));
            }
        }
        SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap())
    } else {
        for i in 0..10 {
            for j in 0..2 {
                net.push(at(i, j));
            }
        }
        SurfaceKind::Nurbs(Surface::new(across, along, &net).unwrap())
    }
}

/// How far a branch's vertex, a double, may be read from the exact point it stands for, in each
/// parameter: far more than its rounding and far less than any piece the search cuts.
const ROUNDING: f64 = 1.0 / 1_099_511_627_776.0;

/// One straight stretch of a crossing on one sheet: the closing parameter c = `at` + `slope`·a over
/// `range` of the parameter a across.
#[derive(Clone, Copy)]
struct Line {
    range: (f64, f64),
    at: f64,
    slope: f64,
}

/// The common part of two ranges, if any.
fn cap(x: (f64, f64), y: (f64, f64)) -> Option<(f64, f64)> {
    let (lo, hi) = (larger(x.0, y.0), smaller(x.1, y.1));
    (lo <= hi).then_some((lo, hi))
}

/// The values of a for which `line` meets `c`, enclosed.
fn preimage(c: Interval, line: Line) -> Option<(f64, f64)> {
    if line.slope == 0.0 {
        return c
            .contains(line.at)
            .then_some((f64::NEG_INFINITY, f64::INFINITY));
    }
    let a = c
        .sub(Interval::point(line.at).unwrap())
        .unwrap()
        .div(Interval::point(line.slope).unwrap())
        .unwrap();
    Some((a.lo(), a.hi()))
}

/// Whether the box (c₁, a₁) of one sheet and (c₂, a₂) of the other holds a point of the crossing,
/// `first` its stretches on the first sheet and `second` on the second, at one a on both.
fn holds_crossing(x: [Interval; 4], first: &[Line], second: &[Line]) -> bool {
    let [c1, a1, c2, a2] = x;
    let Some(a) = cap((a1.lo(), a1.hi()), (a2.lo(), a2.hi())) else {
        return false;
    };
    first.iter().any(|p| {
        second.iter().any(|q| {
            let (Some(x), Some(y)) = (preimage(c1, *p), preimage(c2, *q)) else {
                return false;
            };
            [p.range, q.range, x, y]
                .into_iter()
                .try_fold(a, cap)
                .is_some()
        })
    })
}

/// Checks that `found` is the crossing whose stretches are `one` on one sheet and `other` on the
/// other, across a from 0 to 1, and nothing else: two branches, one from each sheet, every vertex
/// holding a point of it, each running from one edge of a to the other, nothing unproven and
/// nothing flagged.
fn traced_whole(found: &SelfCrossing, t: bool, one: &[Line], other: &[Line]) {
    assert!(found.unproven.is_empty(), "{t}: {:?}", found.unproven);
    assert!(found.flagged.is_empty(), "{t}: {:?}", found.flagged);
    assert_eq!(found.branches.len(), 2, "{t}: {:?}", found.branches);
    let mut sides = [0, 0];
    for branch in &found.branches {
        // A vertex is a double, so it holds the exact point of the crossing only to its rounding:
        // each parameter is read within ROUNDING of it.
        let read = |x: &[Interval; 4]| {
            let x = x.map(|p| Interval::new(p.lo() - ROUNDING, p.hi() + ROUNDING).unwrap());
            if t { [x[1], x[0], x[3], x[2]] } else { x }
        };
        let first = read(&branch.vertices[0]);
        let side = usize::from(!holds_crossing(first, one, other));
        sides[side] += 1;
        let (p, q) = if side == 0 {
            (one, other)
        } else {
            (other, one)
        };
        for x in &branch.vertices {
            assert!(holds_crossing(read(x), p, q), "{t}: {x:?}");
        }
        let last = read(&branch.vertices[branch.vertices.len() - 1]);
        let ends = [first[1], last[1]];
        assert!(
            (ends[0].contains(0.0) && ends[1].contains(1.0))
                || (ends[0].contains(1.0) && ends[1].contains(0.0)),
            "{t}: {ends:?}"
        );
    }
    assert_eq!(sides, [1, 1], "{t}");
}

type Region = ((f64, f64), (f64, f64));

/// Where two pieces share a point, if they do: 0 in parameters, 1 only across the seam where the
/// parameter `closes` (0 for u) closes, 2 only on the `pole` line of v, which is one point.
fn shared(
    surface: &SurfaceKind,
    closes: usize,
    pole: Option<f64>,
    a: Region,
    b: Region,
) -> Option<u8> {
    let domain = surface.domain();
    let meets = |x: (f64, f64), y: (f64, f64)| x.0 <= y.1 && y.0 <= x.1;
    let side = |r: Region, k: usize| if k == 0 { r.0 } else { r.1 };
    let (u, v) = (meets(a.0, b.0), meets(a.1, b.1));
    if u && v {
        return Some(0);
    }
    let (lo, hi) = side(domain, closes);
    let (x, y) = (side(a, closes), side(b, closes));
    let other = if closes == 0 { v } else { u };
    if other && ((x.0 == lo && y.1 == hi) || (y.0 == lo && x.1 == hi)) {
        return Some(1);
    }
    let at = |p: f64, r: (f64, f64)| r.0 <= p && p <= r.1;
    pole.filter(|&p| at(p, a.1) && at(p, b.1)).map(|_| 2)
}

/// The floor the teardrops are searched to: their cusp is never decided, and a coarser floor
/// leaves fewer flags along it.
const CUSP_FLOOR: f64 = 1.0 / 16.0;

/// Checks that `surface`, which crosses itself nowhere, has no branch and no unproven pair, and
/// that each flagged pair whose pieces share a point is flagged undecided by the exclusion test.
/// Returns how many flagged pairs share a point in each way `shared` names.
fn touching_flags(surface: &SurfaceKind, closes: usize, pole: Option<f64>) -> [usize; 3] {
    let found = self_intersect(surface, CUSP_FLOOR, CEILING, &mut meter()).unwrap();
    assert!(found.branches.is_empty(), "{:?}", found.branches);
    assert!(found.unproven.is_empty(), "{:?}", found.unproven);
    let mut count = [0; 3];
    for pair in &found.flagged {
        if let Some(way) = shared(surface, closes, pole, pair.first, pair.second) {
            count[usize::from(way)] += 1;
            assert_eq!(
                pair.why,
                Unresolved::UndecidedAtFloor {
                    test: Test::Exclusion
                },
                "{pair:?}"
            );
        }
    }
    count
}

#[test]
fn a_crossing_over_a_closing_seam_is_traced_whole_from_both_sheets() {
    let sheet = |range, at, slope| Line { range, at, slope };
    let one = [
        sheet((0.0, 0.5), 35.0 / 4.0, 0.5),
        sheet((0.5, 1.0), -0.25, 0.5),
    ];
    let other = [sheet((0.0, 1.0), 35.0 / 8.0, 0.25)];
    for t in [false, true] {
        let found = self_intersect(&double_ramp(t), FLOOR, CEILING, &mut meter()).unwrap();
        traced_whole(&found, t, &one, &other);
    }
}

#[test]
fn a_crossing_beside_a_pole_on_the_domain_s_last_edge_is_traced() {
    let one = [Line {
        range: (0.0, 1.0),
        at: 0.5,
        slope: 0.0,
    }];
    let other = [Line {
        range: (0.0, 1.0),
        at: 4.5,
        slope: 0.0,
    }];
    for t in [false, true] {
        let found = self_intersect(&pinched_ramp(t), FLOOR, CEILING, &mut meter()).unwrap();
        traced_whole(&found, t, &one, &other);
    }
}

#[test]
fn pieces_that_meet_across_a_cusped_seam_are_flagged_as_touching() {
    for t in [false, true] {
        let count = touching_flags(&teardrop(false, t), usize::from(t), None);
        assert!(count[1] > 0, "{t}: {count:?}");
    }
}

#[test]
fn pieces_that_meet_only_at_an_apex_are_flagged_as_touching() {
    let count = touching_flags(&teardrop(true, false), 0, Some(-1.0));
    assert!(count[2] > 0, "{count:?}");
}
