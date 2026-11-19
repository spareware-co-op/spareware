//! A known tangent curve carried by a round surface's implicit form (G174, chapter 12.11): a
//! cylinder lying on a flat NURBS sheet touches it along a line of the cylinder's axis direction.
//! The sheet has no implicit form, so only the cylinder can carry the tube: across the line the
//! cylinder's equation read on the sheet is y², which bends one way, so every cell is proven and
//! nothing is left to search. The line runs along the cylinder's v (its axis), not its u, so the
//! cell is built along the other parameter to `attack_g174`'s sheets. Either surface first.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::general::subdivide;
use geom_intersect::known::{Known, KnownContact, subdivide_known};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, SurfaceKind};

const FLOOR: f64 = 1.0 / 128.0;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 23))
}

/// z = 0 over x and y in [−1/2, 1/2], as a NURBS of degree 1.
fn flat() -> SurfaceKind {
    let line = || Basis::new(1, vec![-0.5, -0.5, 0.5, 0.5]).unwrap();
    let mut net = Vec::new();
    for x in [-0.5, 0.5] {
        for y in [-0.5, 0.5] {
            net.push([x, y, 0.0]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(line(), line(), &net).unwrap())
}

/// The cylinder of radius 1/2 about the line y = 0, z = 1/2, along x from −1 to 1, past the
/// sheet's ends: u = 0 points up, so it rests on z = 0 at u = 1, v = x.
fn roller() -> SurfaceKind {
    SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0, 0.0, 0.5],
            0.5,
            Frame::new([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]).unwrap(),
            ((0.0, 2.0), (-1.0, 1.0)),
        )
        .unwrap(),
    )
}

/// The line of contact, (1, x) on the cylinder and (x, 0) on the sheet, from x = −1/2 to 1/2.
fn contact(cylinder_first: bool) -> Known {
    let point = |x: f64| Interval::point(x).unwrap();
    Known {
        vertices: (-4..=4)
            .map(|k| {
                let x = f64::from(k) / 8.0;
                let (c, s) = ([point(1.0), point(x)], [point(x), point(0.0)]);
                if cylinder_first {
                    [c[0], c[1], s[0], s[1]]
                } else {
                    [s[0], s[1], c[0], c[1]]
                }
            })
            .collect(),
        bound: 0.0,
        contact: KnownContact::Tangent,
    }
}

#[test]
fn a_cylinder_lying_on_a_sheet_is_taken_out_whole_by_its_own_equation() {
    let (sheet, cylinder) = (flat(), roller());
    for cylinder_first in [true, false] {
        let (a, b) = if cylinder_first {
            (&cylinder, &sheet)
        } else {
            (&sheet, &cylinder)
        };
        // Alone, the search cannot settle a tangent contact: it flags it at the floor.
        let plain = subdivide(a, b, FLOOR, &mut meter()).unwrap();
        assert!(!plain.flagged.is_empty(), "{plain:?}");
        let known = subdivide_known(a, b, &[contact(cylinder_first)], FLOOR, &mut meter()).unwrap();
        assert!(
            known.flagged.is_empty(),
            "{cylinder_first}: {:?}",
            known.flagged
        );
        assert!(
            known.settled.is_empty(),
            "{cylinder_first}: {:?}",
            known.settled
        );
    }
}
