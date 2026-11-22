//! What the self-crossing search (12.13) leaves for surfaces chosen so that each part of its pair
//! test shows: a ramp pinched to a point at one end, whose crossing must be traced whole however
//! the pole line touches the pieces beside it; a cusped sweep, whose touching pieces are flagged
//! as undecided by the exclusion test, which is what a pair that touches is split for; a tall fan
//! round an apex, proven clean whole; and the exact work of surfaces whose regions are proven
//! clean, or split, along the way.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly"
)]

use geom_intersect::selfcross::{SelfCrossing, self_intersect};
use geom_intersect::{Test, Unresolved};
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::SurfaceKind;

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

/// The net of `rows` rows of `cols` points `at`, or with `t` its transpose, each point mapped by
/// `map`.
fn either_way(
    (rows, cols): (u32, u32),
    at: &dyn Fn(usize, usize) -> [f64; 3],
    t: bool,
    map: fn([f64; 3]) -> [f64; 3],
) -> SurfaceKind {
    let (m, n) = (
        usize::try_from(rows).unwrap(),
        usize::try_from(cols).unwrap(),
    );
    let mut net = Vec::new();
    if t {
        for j in 0..n {
            for i in 0..m {
                net.push(map(at(i, j)));
            }
        }
        linear_net(cols, rows, &net)
    } else {
        for i in 0..m {
            for j in 0..n {
                net.push(map(at(i, j)));
            }
        }
        linear_net(rows, cols, &net)
    }
}

const fn same(p: [f64; 3]) -> [f64; 3] {
    p
}

/// x ↦ M x for M = [[1, 2, 2], [2, 1, −2], [2, −2, 1]]: a rotation scaled by 3, exact on these
/// nets' coordinates, which turns the z axis to (2, −2, 1), off every coordinate plane.
const fn turned([x, y, z]: [f64; 3]) -> [f64; 3] {
    [
        x + 2.0 * y + 2.0 * z,
        2.0 * x + y - 2.0 * z,
        2.0 * x - 2.0 * y + z,
    ]
}

const SQUARE: [[f64; 2]; 5] = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0], [1.0, 0.0]];

/// The square ramp of `selfcross_mutants`: round the z axis over a turn and a quarter, rows u = 0
/// to 5 at the square's corners, columns v = 0 and 1 at radii 1 and 2, up from z = 0 to ½ over its
/// first quarter, level, then down to 0 over its last. With `pinch`, its first row (u = 0) is the
/// one point (3/2, 0, 0), or, with `at_end`, its last row (u = 5) is (0, 3/2, 0): a pole line.
fn ramp(pinch: bool, at_end: bool, t: bool) -> SurfaceKind {
    let z = [0.0, 0.5, 0.5, 0.5, 0.5, 0.0];
    either_way(
        (6, 2),
        &|i, j| {
            if pinch && !at_end && i == 0 {
                return [1.5, 0.0, 0.0];
            }
            if pinch && at_end && i == 5 {
                return [0.0, 1.5, 0.0];
            }
            let r = [1.0, 2.0][j];
            [r * SQUARE[i % 4][0], r * SQUARE[i % 4][1], z[i]]
        },
        t,
        same,
    )
}

/// A square spindle about the z axis, its first and last rows the tips (0, 0, ∓1): pinched at
/// both ends of u, or with `t` of v, and closed round the other way.
fn spindle(t: bool) -> SurfaceKind {
    either_way(
        (3, 5),
        &|i, j| match i {
            0 => [0.0, 0.0, -1.0],
            2 => [0.0, 0.0, 1.0],
            _ => [SQUARE[j][0], SQUARE[j][1], 0.0],
        },
        t,
        same,
    )
}

/// The sheet whose section at y = v is the cubic arch (−a, 0), (b, h), (−b, h), (a, 0) in x and z
/// of `rows`' first entry at v = −1 and its second at v = 1, its points mapped by `map`.
fn sweep(rows: [(f64, f64, f64); 2], map: fn([f64; 3]) -> [f64; 3]) -> SurfaceKind {
    let across = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let along = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let sections = rows.map(|(a, b, h)| [[-a, 0.0], [b, h], [-b, h], [a, 0.0]]);
    let mut net = Vec::new();
    for i in 0..4 {
        for (y, section) in [-1.0, 1.0].into_iter().zip(&sections) {
            let [x, z] = section[i];
            net.push(map([x, y, z]));
        }
    }
    SurfaceKind::Nurbs(Surface::new(across, along, &net).unwrap())
}

/// A bicubic graph z = f over [−1, 1]², its net's x and y bent so that its edges curve in the
/// plane.
fn bent_graph() -> SurfaceKind {
    let cubic = || Basis::new(3, vec![-1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for i in 0..4_usize {
        for j in 0..4_usize {
            let at = |k: usize| -1.0 + 2.0 * f64::from(u32::try_from(k).unwrap()) / 3.0;
            let h = f64::from(u32::try_from((i * 5 + j * 3) % 7).unwrap()) / 16.0 - 0.1875;
            let dx = 0.25 * [0.0, 1.0, -1.0, 0.0][j];
            let dy = 0.25 * [0.0, -1.0, 1.0, 0.0][i];
            net.push([at(i) + dx, at(j) + dy, h]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(cubic(), cubic(), &net).unwrap())
}

/// A tall fan: the triangles from the apex (0, 0, 4), the row u = 0 and so a pole line, to the
/// polyline `rim` at z = 0, the row u = 1; with `t`, transposed, the apex the column v = 0.
fn fan(rim: &'static [[f64; 2]], t: bool) -> SurfaceKind {
    let n = u32::try_from(rim.len()).unwrap();
    either_way(
        (2, n),
        &|i, j| {
            if i == 0 {
                [0.0, 0.0, 4.0]
            } else {
                [rim[j][0], rim[j][1], 0.0]
            }
        },
        t,
        same,
    )
}

/// A rim that runs most of a turn round the axis, (1, 0) to (0, 1), (−1, 0), (0, −1) and (1, −1).
const MOST_OF_A_TURN: [[f64; 2]; 5] = [
    [1.0, 0.0],
    [0.0, 1.0],
    [-1.0, 0.0],
    [0.0, -1.0],
    [1.0, -1.0],
];

/// A rim of three sides, (1, 0) to (0, 1), (−1, 0) and (0, −2).
const THREE_SIDES: [[f64; 2]; 4] = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -2.0]];

/// A plane U at z = 0 whose arms, x from ½ to a = 3/2 and from b to b + 1, are `gap` steps of the
/// doubles at 3/2 apart (b = a + gap · 2⁻⁵²): rows u = 0 to 3 down the left arm, across the bottom
/// and up the right, columns v = 0 on the inner side and 1 on the outer. It is one to one for any
/// gap above zero.
fn u_shape(gap: f64) -> SurfaceKind {
    let (a, b) = (1.5, 1.5 + gap * f64::EPSILON);
    let rows = [
        [[a, 2.0], [0.5, 2.0]],
        [[a, 1.0], [0.5, 0.0]],
        [[b, 1.0], [b + 1.0, 0.0]],
        [[b, 2.0], [b + 1.0, 2.0]],
    ];
    either_way(
        (4, 2),
        &|i, j| {
            let [x, y] = rows[i][j];
            [x, y, 0.0]
        },
        false,
        same,
    )
}

fn mid(x: geom_num::interval::Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// Checks that `found` is the crossing of a ramp's first quarter (u in [0, 1]) with its last
/// (u in [4, 5]), every vertex on `curve` (from the first sheet's (u, v), the second's), from both
/// sheets, and nothing else left.
fn crossing(found: &SelfCrossing, t: bool, curve: &dyn Fn(f64, f64) -> [f64; 2], what: &str) {
    assert!(found.unproven.is_empty(), "{what}: {:?}", found.unproven);
    assert!(found.flagged.is_empty(), "{what}: {:?}", found.flagged);
    let mut sides = [0, 0];
    for branch in &found.branches {
        let first = branch.vertices[0].map(mid);
        let [a1, _, a2, _] = if t {
            [first[1], first[0], first[3], first[2]]
        } else {
            first
        };
        sides[usize::from(a1 > a2)] += 1;
        for x in &branch.vertices {
            let x = x.map(mid);
            let [u1, v1, u2, v2] = if t { [x[1], x[0], x[3], x[2]] } else { x };
            let ((p, q), (r, s)) = if u1 < u2 {
                ((u1, v1), (u2, v2))
            } else {
                ((u2, v2), (u1, v1))
            };
            assert!(
                (0.0..=1.0).contains(&p) && (4.0..=5.0).contains(&r),
                "{what}: {x:?}"
            );
            let [want_r, want_s] = curve(p, q);
            assert!(
                (r - want_r).abs() < FLOOR && (s - want_s).abs() < FLOOR,
                "{what}: {x:?}"
            );
        }
    }
    assert!(sides[0] >= 1 && sides[0] == sides[1], "{what}: {sides:?}");
}

/// The ramp pinched at its start: its first quarter is the triangle from P = (3/2, 0, 0) to the
/// segment (0, r, ½), r = 1 + v from 1 to 2, the points (3/2·(1 − u), u·r, u/2); its last is
/// ((1 − t)·r′, t·r′, (1 − t)/2) for u = 4 + t, r′ = 1 + v′. They are one point where u = 1 − t
/// (z), 3/2·t = u·r′ (x) and u·r = t·r′ (y): with k = t / u, r′ = 3/2·k and r = 3/2·k², so
/// u = 1 / (1 + k), v = 3/2·k² − 1, u′ = 5 − u and v′ = 3/2·k − 1, for k from √(2/3) to √(4/3),
/// where v meets its ends. The pole line u = 0 is the point P, which the crossing keeps clear of
/// (u > 2/5), so nothing there is undecided.
#[test]
fn a_ramp_pinched_at_its_start_crosses_itself_where_its_ends_overlap() {
    for t in [false, true] {
        let found = self_intersect(&ramp(true, false, t), FLOOR, CEILING, &mut meter()).unwrap();
        let curve = |u: f64, v: f64| {
            let k = (1.0 - u) / u;
            assert!((v - (1.5 * k * k - 1.0)).abs() < FLOOR, "{t}: {u} {v}");
            [5.0 - u, 1.5 * k - 1.0]
        };
        crossing(&found, t, &curve, "pinched at its start");
    }
}

/// The ramp pinched at its end: its first quarter is ((1 − u)·r, u·r, u/2), its last the triangle
/// from the segment (r′, 0, ½) at t = 0 to Q = (0, 3/2, 0) at t = 1, the points
/// ((1 − t)·r′, 3/2·t, (1 − t)/2). They are one point where u = 1 − t (z), t·r = (1 − t)·r′ (x)
/// and (1 − t)·r = 3/2·t (y): with k = t / (1 − t), r = 3/2·k and r′ = 3/2·k², so u = 1 / (1 + k),
/// v = 3/2·k − 1, u′ = 5 − u and v′ = 3/2·k² − 1, for k from √(2/3) to √(4/3), where v′ meets its
/// ends. The pole line u = 5 is the point Q, which the crossing keeps clear of.
#[test]
fn a_ramp_pinched_at_its_end_crosses_itself_where_its_ends_overlap() {
    let found = self_intersect(&ramp(true, true, false), FLOOR, CEILING, &mut meter()).unwrap();
    let curve = |u: f64, v: f64| {
        let k = (1.0 - u) / u;
        assert!((v - (1.5 * k - 1.0)).abs() < FLOOR, "{u} {v}");
        [5.0 - u, 1.5 * k * k - 1.0]
    };
    crossing(&found, false, &curve, "pinched at its end");
}

/// The cusped sweep of `selfcross_mutants` (a = b: its section's loop shrunk to a cusp, so it does
/// not cross itself) leaves its cusp undecided. A pair of its pieces that touch, in parameters, is
/// never settled, only split (12.13), and is flagged at the floor as undecided by the exclusion
/// test: the test the pair is split for, not one of its normals.
#[test]
fn a_cusped_sweep_flags_its_touching_pieces_as_undecided_by_exclusion() {
    let floor = 1.0 / 32.0;
    let cusp = sweep([(9.0 / 8.0, 9.0 / 8.0, 2.0); 2], same);
    let found = self_intersect(&cusp, floor, CEILING, &mut meter()).unwrap();
    assert!(found.branches.is_empty() && found.unproven.is_empty());
    let meets = |x: (f64, f64), y: (f64, f64)| x.0 <= y.1 && y.0 <= x.1;
    let mut touching = 0;
    for f in &found.flagged {
        if meets(f.first.0, f.second.0) && meets(f.first.1, f.second.1) {
            touching += 1;
            assert_eq!(
                f.why,
                Unresolved::UndecidedAtFloor {
                    test: Test::Exclusion
                },
                "{f:?}"
            );
        }
    }
    assert!(touching > 0);
}

/// The fan over most of a turn is one to one: a point (1 − s)·A + s·R of it, A the apex and R on
/// the rim, has z = 4·(1 − s), which fixes s, and for s > 0 its x and y are s·R, which fix R, and
/// the rim is a simple polyline (its last side, at y = −1, meets no other). So it does not cross
/// itself, and the search, whose tangent boxes reach past ±2 along the tall sides, proves that
/// whole: nothing to report, either way round.
#[test]
fn a_tall_fan_round_most_of_a_turn_is_clean_either_way_round() {
    for t in [false, true] {
        let found = self_intersect(&fan(&MOST_OF_A_TURN, t), FLOOR, CEILING, &mut meter()).unwrap();
        assert_eq!(
            found,
            SelfCrossing {
                branches: Vec::new(),
                unproven: Vec::new(),
                flagged: Vec::new(),
            },
            "{t}"
        );
    }
}

/// The plane U whose arms lie 8 steps of the doubles apart is one to one, and the search proves
/// it whole: the boxes of its two inner sides end at one double, each bound widened past the
/// points it holds, so the sides they hold are apart. Nothing to report, and no pair left at the
/// floor.
#[test]
fn a_u_whose_arms_lie_8_steps_apart_is_clean() {
    let found = self_intersect(&u_shape(8.0), FLOOR, CEILING, &mut meter()).unwrap();
    assert_eq!(
        found,
        SelfCrossing {
            branches: Vec::new(),
            unproven: Vec::new(),
            flagged: Vec::new(),
        }
    );
}

/// The exact work each search takes, with how many branches, unproven pairs and flags it leaves,
/// for surfaces whose pairs are held clean, or split, by regions the other tests do not reach: the
/// pinched ramps and the fan, whose pole line touches pieces on one side or both; the spindles,
/// glued across their seam; a turned arch and a bent graph, whose axes or boundaries lie off
/// the coordinate planes; and a U whose arms lie 8 steps of the doubles apart, so that the boxes
/// of their sides, each bound rounded outward, end at one double: boxes that only share an end are
/// counted apart, and the U is held clean whole, with nothing left at the floor.
#[test]
fn each_surface_takes_its_recorded_work() {
    let surfaces = [
        ("pinched", ramp(true, false, false), FLOOR),
        ("pinched transposed", ramp(true, false, true), FLOOR),
        ("pinched at its end", ramp(true, true, false), FLOOR),
        (
            "pinched at its end transposed",
            ramp(true, true, true),
            FLOOR,
        ),
        ("fan transposed", fan(&THREE_SIDES, true), 1.0 / 32.0),
        ("spindle", spindle(false), FLOOR),
        ("spindle transposed", spindle(true), FLOOR),
        (
            "turned arch",
            sweep([(1.5, 0.25, 1.0), (1.25, 0.5, 1.375)], turned),
            FLOOR,
        ),
        ("bent graph", bent_graph(), FLOOR),
        ("arms 8 steps apart", u_shape(8.0), FLOOR),
    ];
    let want: [(&str, u128, usize, usize, usize); 10] = [
        ("pinched", 211_683, 2, 0, 0),
        ("pinched transposed", 235_636, 2, 0, 0),
        ("pinched at its end", 369_408, 2, 0, 0),
        ("pinched at its end transposed", 337_553, 2, 0, 0),
        ("fan transposed", 4_065_748, 0, 0, 4),
        ("spindle", 203_100, 0, 0, 0),
        ("spindle transposed", 202_328, 0, 0, 0),
        ("turned arch", 12_854, 0, 0, 0),
        ("bent graph", 1058, 0, 0, 0),
        ("arms 8 steps apart", 3832, 0, 0, 0),
    ];
    for ((name, surface, floor), want) in surfaces.iter().zip(want) {
        let mut m = meter();
        let found = self_intersect(surface, *floor, CEILING, &mut m).unwrap();
        let seen = (
            *name,
            m.used().work,
            found.branches.len(),
            found.unproven.len(),
            found.flagged.len(),
        );
        assert_eq!(seen, want);
    }
}
