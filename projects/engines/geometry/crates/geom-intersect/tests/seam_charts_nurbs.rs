//! What a NURBS surface's own data proves about its seams and poles (chapter 12.10): a row or
//! column of the net that is one point is a pole only where the basis is clamped, so that the
//! surface reaches that row at the domain's end; the first and last columns are one closing
//! curve only for a clamped basis too. An unclamped basis ends inside its net: the same net proves
//! nothing there. And a cone whose apex is exactly at an end of its domain has that one pole,
//! read once, not once as an end and again as a point inside.

#![allow(
    clippy::unwrap_used,
    clippy::many_single_char_names,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::charts;
use geom_nurbs::{Basis, Surface};
use geom_surface::{Cone, SurfaceKind};

/// Degree 1 on three control points: clamped on [0, 2], or unclamped on [1, 3].
fn linear(clamped: bool) -> Basis {
    if clamped {
        Basis::new(1, vec![0.0, 0.0, 1.0, 2.0, 2.0]).unwrap()
    } else {
        Basis::new(1, vec![0.0, 1.0, 2.0, 3.0, 4.0]).unwrap()
    }
}

/// A spindle: its first and last rows in u are each one point, the tips, and its middle row a
/// triangle about the axis. `transposed` takes the same net with u and v swapped.
fn spindle(clamped: bool, transposed: bool) -> SurfaceKind {
    let ring = [[1.0, 0.0], [-0.5, 0.9], [-0.5, -0.9]];
    let at = |i: usize, j: usize| -> [f64; 3] {
        match i {
            0 => [0.0, 0.0, -1.0],
            2 => [0.0, 0.0, 1.0],
            _ => [ring[j][0], ring[j][1], 0.0],
        }
    };
    let mut net = Vec::new();
    for i in 0..3 {
        for j in 0..3 {
            net.push(if transposed { at(j, i) } else { at(i, j) });
        }
    }
    SurfaceKind::Nurbs(Surface::new(linear(clamped), linear(clamped), &net).unwrap())
}

/// A square tube: its rows in u climb in z, and its first and last columns in v are one line.
/// `transposed` closes in u instead.
fn tube(clamped: bool, transposed: bool) -> SurfaceKind {
    let square = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [1.0, 0.0]];
    let around = if clamped {
        Basis::new(1, vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0]).unwrap()
    } else {
        Basis::new(1, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]).unwrap()
    };
    let at = |i: usize, j: usize| -> [f64; 3] {
        let [x, y] = square[j];
        [x, y, [-1.0, 0.0, 1.0][i]]
    };
    let mut net = Vec::new();
    if transposed {
        for j in 0..4 {
            for i in 0..3 {
                net.push(at(i, j));
            }
        }
        SurfaceKind::Nurbs(Surface::new(around, linear(clamped), &net).unwrap())
    } else {
        for i in 0..3 {
            for j in 0..4 {
                net.push(at(i, j));
            }
        }
        SurfaceKind::Nurbs(Surface::new(linear(clamped), around, &net).unwrap())
    }
}

#[test]
fn a_collapsed_row_is_a_pole_only_for_a_clamped_basis() {
    assert_eq!(
        charts(&spindle(true, false)).poles,
        (vec![0.0, 2.0], vec![])
    );
    assert_eq!(charts(&spindle(true, true)).poles, (vec![], vec![0.0, 2.0]));
    assert_eq!(charts(&spindle(false, false)).poles, (vec![], vec![]));
    assert_eq!(charts(&spindle(false, true)).poles, (vec![], vec![]));
}

#[test]
fn equal_end_columns_close_only_for_a_clamped_basis() {
    assert_eq!(charts(&tube(true, false)).closes, [false, true]);
    assert_eq!(charts(&tube(true, true)).closes, [true, false]);
    assert_eq!(charts(&tube(false, false)).closes, [false, false]);
    assert_eq!(charts(&tube(false, true)).closes, [false, false]);
    // A spindle's tip rows are each one point but not the same row: it does not close.
    assert_eq!(charts(&spindle(true, false)).closes, [false, false]);
}

#[test]
fn a_cone_apex_at_an_end_of_its_domain_is_one_pole() {
    // Radius 1 + v/2: zero at v = −2, at the start of the first domain and the end of the second.
    for (v, poles) in [
        ((-2.0, 4.0), vec![-2.0]),
        ((-4.0, -2.0), vec![-2.0]),
        ((-4.0, 4.0), vec![-2.0]),
    ] {
        let cone = Cone::new(
            [0.0; 3],
            1.0,
            0.5,
            Frame::standard().unwrap(),
            ((0.0, 2.0), v),
        )
        .unwrap();
        assert_eq!(
            charts(&SurfaceKind::Cone(cone)).poles,
            (vec![], poles),
            "{v:?}"
        );
    }
}
