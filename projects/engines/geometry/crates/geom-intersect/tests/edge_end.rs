//! An edge that meets the other surface exactly on the end of its domain (G173a, chapter 12.10).
//! The SSI corpus' `c7_angle_planes` is a plane against a NURBS plane tilted by a small angle, both
//! over x from -1 to 1, so the branch they share, the line y = 0, z = 0, ends at the corners of both
//! patches. The zero of the plane's edge x = -1 against the NURBS patch lies on the NURBS patch's end
//! (its first parameter is zero there), where Krawczyk's operator cannot prove it: the interior
//! system flags the pair at once. The same edge system on the end itself, with two of its three
//! equations solved and the third checked, proves it. Run at the corpus' scale (8192 steps to the
//! unit, floor 8).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_intersect::general::subdivide;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};

const UNIT: f64 = 8192.0;
const FLOOR: f64 = 8.0;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

/// The plane z = 0 over x and y from -1 to 1.
fn flat() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [UNIT, 0.0, 0.0],
            [0.0, UNIT, 0.0],
            ((-1.0, 1.0), (-1.0, 1.0)),
        )
        .unwrap(),
    )
}

/// The bilinear patch through (±1, ±1, ±`rise`), a plane z = `rise`·y over the same square, as a
/// NURBS surface on the parameters [0, 1]².
fn tilted(rise: f64) -> SurfaceKind {
    let basis = || Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for x in [-1.0, 1.0] {
        for y in [-1.0, 1.0] {
            net.push([x * UNIT, y * UNIT, y * rise * UNIT]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap())
}

#[test]
fn a_zero_on_the_end_of_a_nurbs_patch_is_proven_there() {
    for rise in [1.0 / 1024.0, 1.0 / 65_536.0] {
        let (a, b) = (flat(), tilted(rise));
        for (first, second) in [(&a, &b), (&b, &a)] {
            let found = subdivide(first, second, FLOOR, &mut meter()).unwrap();
            assert!(found.flagged.is_empty(), "{:?}", found.flagged);
            let starts: usize = found.settled.iter().map(|pair| pair.starts.len()).sum();
            assert_eq!(starts % 2, 0, "{found:?}");
            // The line's two ends, at x = -1 and x = 1, on the corners of both patches.
            assert!(starts >= 2, "{found:?}");
        }
    }
}

#[test]
fn a_zero_inside_the_other_patch_is_proven_as_before() {
    // The same patches with the NURBS patch narrowed to x from -1/2 to 1/2: the line now meets the
    // NURBS patch's own edges inside the plane, and the plane's edges not at all. Nothing lies on
    // an end of the plane, and the pair is decided by the interior system.
    let basis = || Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for x in [-1.0, 1.0] {
        for y in [-1.0, 1.0] {
            net.push([x * UNIT * 0.5, y * UNIT, y * UNIT / 1024.0]);
        }
    }
    let narrow = SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap());
    let found = subdivide(&flat(), &narrow, FLOOR, &mut meter()).unwrap();
    assert!(found.flagged.is_empty(), "{:?}", found.flagged);
    let starts: usize = found.settled.iter().map(|pair| pair.starts.len()).sum();
    assert_eq!(starts % 2, 0, "{found:?}");
}
