//! A NURBS that closes on itself, or pinches to a point, is no self-crossing (G176, chapter 12.10):
//! patches on the two sides of its seam meet along the seam, and patches round a pole meet at the
//! pole, and that is the surface's own continuity, not a crossing. A tube closed in u or in v, a
//! spindle pinched at both ends of u or of v, and a cone closed round and pinched at one end are
//! all clean, with nothing to report.

#![allow(
    clippy::unwrap_used,
    clippy::many_single_char_names,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code: a failure should stop loudly"
)]

use geom_intersect::selfcross::{SelfCrossing, self_intersect};
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

/// A square tube about the z axis from z = −1 to 1, its first and last columns one line; with
/// `transposed`, its first and last rows instead.
fn tube(transposed: bool) -> SurfaceKind {
    let square = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0], [1.0, 0.0]];
    let at = |i: usize, j: usize| {
        let [x, y] = square[j];
        [x, y, [-1.0, 0.0, 1.0][i]]
    };
    let mut net = Vec::new();
    if transposed {
        for j in 0..5 {
            for i in 0..3 {
                net.push(at(i, j));
            }
        }
        linear_net(5, 3, &net)
    } else {
        for i in 0..3 {
            for j in 0..5 {
                net.push(at(i, j));
            }
        }
        linear_net(3, 5, &net)
    }
}

/// A double pyramid on a square about the z axis: its first and last rows each one point, the
/// tips; with `transposed`, its first and last columns.
fn spindle(transposed: bool) -> SurfaceKind {
    let square = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];
    let at = |i: usize, j: usize| match i {
        0 => [0.0, 0.0, -1.0],
        2 => [0.0, 0.0, 1.0],
        _ => [square[j][0], square[j][1], 0.0],
    };
    let mut net = Vec::new();
    if transposed {
        for j in 0..4 {
            for i in 0..3 {
                net.push(at(i, j));
            }
        }
        linear_net(4, 3, &net)
    } else {
        for i in 0..3 {
            for j in 0..4 {
                net.push(at(i, j));
            }
        }
        linear_net(3, 4, &net)
    }
}

/// A square pyramid's sides: closed round in v, pinched to the apex at u = 0.
fn pyramid() -> SurfaceKind {
    let square = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0], [1.0, 0.0]];
    let mut net = Vec::new();
    for i in 0..3 {
        for [x, y] in square {
            let r = [0.0, 0.5, 1.0][i];
            net.push([r * x, r * y, r]);
        }
    }
    linear_net(3, 5, &net)
}

fn clean(surface: &SurfaceKind) {
    let found = self_intersect(surface, FLOOR, CEILING, &mut meter()).unwrap();
    assert_eq!(
        found,
        SelfCrossing {
            branches: Vec::new(),
            unproven: Vec::new(),
            flagged: Vec::new(),
        },
        "{surface:?}"
    );
}

#[test]
fn a_tube_closed_on_itself_is_clean_either_way_round() {
    clean(&tube(false));
    clean(&tube(true));
}

#[test]
fn a_spindle_pinched_at_its_tips_is_clean_either_way_round() {
    clean(&spindle(false));
    clean(&spindle(true));
}

#[test]
fn a_pyramid_closed_round_and_pinched_at_its_apex_is_clean() {
    clean(&pyramid());
}
