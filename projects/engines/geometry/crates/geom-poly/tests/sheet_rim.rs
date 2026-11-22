//! A sheet's rim along a line two polygons' planes share, named the same whatever order the
//! polygons are cut in and however the boxes fell: by the plane of the polygon that lies along the
//! line and holds the point, not by whichever cut the piece first (G129, 11.3, #1182).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "tests: native arithmetic, and a failure stops the test"
)]

use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, P3, Sign};
use geom_poly::convex::{Piece, convex_pieces};
use geom_poly::merge::{Merge, Merged, Rules, as_kept, merge_with};
use geom_poly::plane::canonical;
use geom_poly::subdivide::{around, arrange};
use geom_poly::trace::Polygon;
use geom_poly::winding::{Winding, WindingTest};
use geom_run::{Budget, Exec, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn plane(n: [i128; 3], d: i128) -> Plane {
    Plane::new(n, d).unwrap()
}

fn reduced(plane: &Plane) -> Plane {
    canonical(plane).unwrap().plane
}

fn opposite(p: &Plane) -> Result<Plane, NumError> {
    Plane::new(p.n().map(|v| -v), -p.d())
}

/// A tetrahedron's faces, each the plane through three corners, outward; face i is the one away
/// from corner i.
fn tetrahedron(corner: [[i64; 3]; 4], entry: Option<u32>) -> Vec<Polygon> {
    let at = |p: [i64; 3]| P3::new(p[0], p[1], p[2]).unwrap();
    let mut planes = Vec::new();
    for i in 0..4 {
        let [a, b, c] = [(i + 1) % 4, (i + 2) % 4, (i + 3) % 4].map(|k| corner[k]);
        let plane = Plane::through(at(a), at(b), at(c)).unwrap();
        planes.push(match plane.side(at(corner[i])).unwrap() {
            Sign::Negative => plane,
            _ => opposite(&plane).unwrap(),
        });
    }
    let mut polygons = Vec::new();
    for i in 0..4 {
        let sides: Vec<Plane> = (1..4)
            .map(|k| opposite(&planes[(i + k) % 4]).unwrap())
            .collect();
        let mut pieces = convex_pieces(&planes[i], &[sides], &mut meter()).unwrap();
        polygons.push(Polygon {
            piece: pieces.pop().unwrap(),
            entry,
        });
    }
    polygons
}

/// The sheet: the square z = 0, x and y from 0 to 10, of no entry.
fn sheet() -> Polygon {
    Polygon {
        piece: Piece {
            face: plane([0, 0, 1], 0),
            bounds: vec![
                plane([0, 1, 0], 0),
                plane([-1, 0, 0], 10),
                plane([0, -1, 0], 10),
                plane([1, 0, 0], 0),
            ],
        },
        entry: None,
    }
}

/// What a union keeps of the sheet, merged as a sheet and given the other polygons' pieces, with
/// the triangle cut before the solid or after it, at leaf size `leaf`.
fn rim(triangle_first: bool, leaf: usize) -> Merged {
    let solid = tetrahedron(
        [[-20, -20, 81], [60, -20, -159], [-20, 60, 1], [40, 40, 0]],
        Some(0),
    );
    // The triangle is face 3 of a tetrahedron of no entry, the face away from its fourth corner.
    let triangle =
        tetrahedron([[1, 1, 3], [0, 0, -1], [-1, 2, -2], [1, 1, 1003]], None).swap_remove(3);
    let mut polygons = vec![sheet()];
    if triangle_first {
        polygons.push(triangle);
        polygons.extend(solid);
    } else {
        polygons.extend(solid);
        polygons.push(triangle);
    }
    let cell = around(&polygons).unwrap();
    let found = arrange(
        &Exec::serial(),
        cell,
        Winding::new(),
        &polygons,
        leaf,
        &[],
        &mut meter(),
    )
    .unwrap();
    let test = WindingTest::unite();
    let kept: Vec<_> = found
        .iter()
        .filter(|c| c.polygon == 0 && test.holds(&c.front))
        .map(as_kept)
        .collect();
    let crossing: Vec<Piece> = found
        .iter()
        .filter(|c| c.polygon != 0)
        .map(|c| c.piece.clone())
        .collect();
    let rules = Rules {
        cuts: &[],
        open: true,
        crossing: &crossing,
    };
    merge_with(&kept, Merge::RemoveRedundant, rules, &mut meter()).unwrap()
}

#[test]
fn a_sheet_s_rim_is_named_by_the_polygon_along_it_that_holds_the_point_not_the_first_cut() {
    // The solid's face 3x + y + z = 1 crosses the sheet along the line 3x + y = 1, from
    // V = (1/3, 0, 0) on the sheet's edge y = 0 to (0, 1, 0), and the union keeps the sheet beyond
    // it. The triangle's plane 3x + y - z = 1 meets the sheet along the same line, but the triangle
    // does so only for x from -1/5 to 1/4, short of V. Cut before the solid, the triangle's plane
    // ran along the line across the whole piece, and the rim's fin there, and so V's name, bore its
    // plane; cut after, the solid's. V is named by the sheet, its edge and the solid's face, which
    // holds V, in either order and at either leaf size.
    let face = reduced(&plane([0, 0, 1], 0));
    let edge = reduced(&plane([0, 1, 0], 0));
    let solid = reduced(&plane([3, 1, 1], -1));
    let on = |plane: &Plane, v: &Tpi| plane.side_of_tpi(v) == Ok(Sign::Zero);
    let whole = rim(false, 1000);
    let at_v: Vec<&Tpi> = whole
        .vertices
        .iter()
        .filter(|v| [face, edge, solid].iter().all(|p| on(p, v)))
        .collect();
    assert_eq!(
        at_v.len(),
        1,
        "V, where the rim along the line meets the edge"
    );
    assert_eq!(*at_v[0], Tpi::meeting(face, edge, solid).unwrap());
    for triangle_first in [false, true] {
        for leaf in [1, 1000] {
            let m = rim(triangle_first, leaf);
            let case = format!("triangle first {triangle_first}, leaf {leaf}");
            assert_eq!(m.vertices, whole.vertices, "{case}");
            assert_eq!(m.edges, whole.edges, "{case}");
        }
    }
}
