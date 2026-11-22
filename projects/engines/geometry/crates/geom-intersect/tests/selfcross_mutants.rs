//! What the self-crossing search (12.13) does, pinned closely enough that a change to any step of
//! it shows: a ceiling that is not positive is refused even for a kind with nothing to search, and
//! each surface here, closed on itself, pinched at a pole, crossing itself, cusped or clean, takes the same
//! work and leaves the same branches, unproven pairs and flags every time.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::IntersectError;
use geom_intersect::selfcross::self_intersect;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Sphere, SurfaceKind};

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

const SQUARE: [[f64; 2]; 5] = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0], [1.0, 0.0]];

/// A square tube about the z axis from z = −1 to 1, its first and last columns one line.
fn tube(t: bool) -> SurfaceKind {
    either_way(
        3,
        5,
        &|i, j| [SQUARE[j][0], SQUARE[j][1], [-1.0, 0.0, 1.0][i]],
        t,
    )
}

/// A square pyramid's sides: closed round, pinched to the apex at its first row.
fn pyramid(t: bool) -> SurfaceKind {
    either_way(
        3,
        5,
        &|i, j| {
            let r = [0.0, 0.5, 1.0][i];
            [r * SQUARE[j][0], r * SQUARE[j][1], r]
        },
        t,
    )
}

/// The sheet whose section at y = v is the cubic arch (−a, 0), (b, h), (−b, h), (a, 0) in x and z
/// of `rows`' first entry at v = −1 and its second at v = 1: it loops, crossing itself, when b > a.
fn sweep(rows: [(f64, f64, f64); 2]) -> SurfaceKind {
    let across = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let along = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let sections = rows.map(|(a, b, h)| [[-a, 0.0], [b, h], [-b, h], [a, 0.0]]);
    let mut net = Vec::new();
    for i in 0..4 {
        for (y, section) in [-1.0, 1.0].into_iter().zip(&sections) {
            let [x, z] = section[i];
            net.push([x, y, z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(across, along, &net).unwrap())
}

/// A bicubic graph z = f(x, y) over [−1, 1]², its heights fixed.
fn graph() -> SurfaceKind {
    let cubic = || Basis::new(3, vec![-1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for i in 0..4 {
        for j in 0..4 {
            let at = |k: i32| -1.0 + 2.0 * f64::from(k) / 3.0;
            let h = f64::from((i * 5 + j * 3) % 7) / 16.0 - 0.1875;
            net.push([at(i), at(j), h]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(cubic(), cubic(), &net).unwrap())
}

/// A square ramp round the z axis over a turn and a quarter, between radii 1 and 2: up from z = 0
/// to ½ over its first quarter, level, then down to 0 over its last, which lies over its first. Its
/// normals all lie in one hemisphere about z, so only its boundary, projected along z, shows that
/// it overlaps itself: its first and last quarters cross where both are at z = ¼, along u = ½ of
/// one sheet and u = 4½ of the other, at the same v.
fn ramp(t: bool) -> SurfaceKind {
    let z = [0.0, 0.5, 0.5, 0.5, 0.5, 0.0];
    either_way(
        6,
        2,
        &|i, j| {
            let r = [1.0, 2.0][j];
            [r * SQUARE[i % 4][0], r * SQUARE[i % 4][1], z[i]]
        },
        t,
    )
}

fn mid(x: geom_num::interval::Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

#[test]
fn a_ramp_over_more_than_a_turn_crosses_itself_where_its_ends_overlap() {
    for t in [false, true] {
        let found = self_intersect(&ramp(t), FLOOR, CEILING, &mut meter()).unwrap();
        let mut sides = [0, 0];
        for branch in &found.branches {
            let x = branch.vertices[0].map(mid);
            let [a1, b1, a2, b2] = if t { [x[1], x[0], x[3], x[2]] } else { x };
            sides[usize::from(a1 > a2)] += 1;
            for x in &branch.vertices {
                let x = x.map(mid);
                let [u1, v1, u2, v2] = if t { [x[1], x[0], x[3], x[2]] } else { x };
                let on = |p: f64, q: f64| (p - 0.5).abs() < FLOOR && (q - 4.5).abs() < FLOOR;
                assert!(on(u1, u2) || on(u2, u1), "{t}: {x:?}");
                assert!((v1 - v2).abs() < FLOOR, "{t}: {x:?}");
            }
            assert!((b1 - b2).abs() < FLOOR, "{t}: {x:?}");
        }
        // Each crossing from both sheets.
        assert!(sides[0] >= 1 && sides[0] == sides[1], "{t}: {sides:?}");
    }
}

#[test]
fn a_ceiling_that_is_not_positive_is_refused_even_with_nothing_to_search() {
    let sphere = SurfaceKind::Sphere(
        Sphere::new(
            [0.0; 3],
            5.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    );
    assert!(self_intersect(&sphere, FLOOR, CEILING, &mut meter()).is_ok());
    for ceiling in [0.0, -1.0, f64::NAN] {
        assert_eq!(
            self_intersect(&sphere, FLOOR, ceiling, &mut meter()),
            Err(IntersectError::InvalidInput),
            "{ceiling}"
        );
    }
}

/// The work each surface's search takes, and how many branches, unproven pairs and flags it
/// leaves.
#[test]
fn each_surface_takes_its_recorded_work() {
    let surfaces = [
        ("tube", tube(false)),
        ("tube transposed", tube(true)),
        ("pyramid", pyramid(false)),
        ("loop", sweep([(9.0 / 8.0, 13.0 / 8.0, 2.0); 2])),
        ("arch", sweep([(1.5, 0.25, 1.0), (1.25, 0.5, 1.375)])),
        ("cusp", sweep([(9.0 / 8.0, 9.0 / 8.0, 2.0); 2])),
        ("graph", graph()),
        ("ramp", ramp(false)),
        ("ramp transposed", ramp(true)),
    ];
    let want: [(&str, u128, usize, usize, usize); 9] = [
        ("tube", 95_536, 0, 0, 0),
        ("tube transposed", 95_536, 0, 0, 0),
        ("pyramid", 5_974_714, 0, 0, 0),
        ("loop", 467_011, 2, 0, 0),
        ("arch", 484, 0, 0, 0),
        ("cusp", 8_539_690, 0, 0, 2740),
        ("graph", 1058, 0, 0, 0),
        ("ramp", 171_111, 2, 0, 0),
        ("ramp transposed", 173_065, 2, 0, 0),
    ];
    for ((name, surface), want) in surfaces.iter().zip(want) {
        let mut m = meter();
        let found = self_intersect(surface, FLOOR, CEILING, &mut m).unwrap();
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
