//! The end of a section's curve, named the same however the boxes fell: by the two faces that meet
//! there and the outline the curve leaves a face across, not by a plane a cut ran through it from a
//! polygon that stops short of it (G129, 11.3, #1177).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "tests: native arithmetic, and a failure stops the test"
)]

use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, P3, Sign};
use geom_poly::convex::{Piece, convex_pieces};
use geom_poly::merge::{Curves, section};
use geom_poly::plane::canonical;
use geom_poly::subdivide::{around, arrange};
use geom_poly::trace::Polygon;
use geom_poly::winding::Winding;
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

fn up(axis: usize, at: i64) -> Plane {
    let mut n = [0i128; 3];
    n[axis] = 1;
    plane(n, -i128::from(at))
}

fn down(axis: usize, at: i64) -> Plane {
    let mut n = [0i128; 3];
    n[axis] = -1;
    plane(n, i128::from(at))
}

/// The six faces of the block from `low` to `high`, outward, each bounded by the planes of the
/// faces beside it: for each axis, the high face, then the low.
fn block(low: [i64; 3], high: [i64; 3], polygons: &mut Vec<Polygon>) {
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let (ul, uh, vl, vh) = (low[u], high[u], low[v], high[v]);
        let high_face = Piece {
            face: up(axis, high[axis]),
            bounds: vec![up(v, vl), down(u, uh), down(v, vh), up(u, ul)],
        };
        let low_face = Piece {
            face: down(axis, low[axis]),
            bounds: vec![up(u, ul), down(v, vh), down(u, uh), up(v, vl)],
        };
        for piece in [high_face, low_face] {
            polygons.push(Polygon {
                piece,
                entry: Some(0),
            });
        }
    }
}

/// A tetrahedron's faces, each the plane through three corners, outward; face i is the one away
/// from corner i.
fn tetrahedron(corner: [[i64; 3]; 4], entry: u32, polygons: &mut Vec<Polygon>) {
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
    for i in 0..4 {
        let sides: Vec<Plane> = (1..4)
            .map(|k| opposite(&planes[(i + k) % 4]).unwrap())
            .collect();
        let mut pieces = convex_pieces(&planes[i], &[sides], &mut meter()).unwrap();
        polygons.push(Polygon {
            piece: pieces.pop().unwrap(),
            entry: Some(entry),
        });
    }
}

/// The section of the block's top against the whole of the tilted solid, with the far solid in the
/// arrangement too, at leaf size `leaf`.
/// The block's top, among its polygons.
const TOP: usize = 4;

fn cut(polygons: &[Polygon], leaf: usize) -> Curves {
    let cell = around(polygons).unwrap();
    let found = arrange(
        &Exec::serial(),
        cell,
        Winding::new(),
        polygons,
        leaf,
        &[],
        &mut meter(),
    )
    .unwrap();
    let pieces: Vec<(Piece, usize)> = found
        .iter()
        .filter(|c| c.polygon == TOP || (6..10).contains(&c.polygon))
        .map(|c| (c.piece.clone(), usize::from(c.polygon != TOP)))
        .collect();
    section(&pieces, &mut meter()).unwrap()
}

#[test]
fn the_end_of_a_section_s_curve_is_not_named_by_a_plane_whose_polygon_stops_short_of_it() {
    // The tilted solid's face -3x + 3y + 2z + 29 = 0 crosses the block's top, z = 0, along the
    // line y = x - 29/3, which leaves the top across its edge x = 10 at (10, 1/3, 0), inside the
    // tilted face's triangle: an end of the section's curve, with the two faces on it and the
    // top's edge crossing it there. The far solid's face 3y + z = 1 runs through that point too,
    // but its triangle crosses the top only between x = 1/3 and x = 7/3. In one box that face's
    // plane cuts the top's piece right across, through (10, 1/3, 0), and comes before x = 10 in
    // plane order; in smaller boxes the far solid is not beside the point and the plane does not
    // reach it. The point is named by the two faces and the top's edge in every box.
    let mut polygons = Vec::new();
    block([0, 0, -10], [10, 10, 0], &mut polygons);
    // Each solid's first corner is the one away from the face that matters, so that face is its
    // first polygon and cuts the top before the solid's other faces do.
    tetrahedron(
        [[12, -4, 1], [6, -3, -1], [14, 5, -1], [10, -3, 5]],
        1,
        &mut polygons,
    );
    tetrahedron(
        [[1, 0, -2], [0, 0, 1], [3, 0, 1], [1, 1, -2]],
        2,
        &mut polygons,
    );
    let top = reduced(&plane([0, 0, 1], 0));
    let tilted = reduced(&plane([-3, 3, 2], 29));
    let edge = reduced(&plane([1, 0, 0], -10));
    let stray = reduced(&plane([0, 3, 1], -1));
    assert_eq!(reduced(&polygons[6].piece.face), tilted);
    assert_eq!(reduced(&polygons[10].piece.face), stray);
    let on = |plane: &Plane, v: &Tpi| plane.side_of_tpi(v) == Ok(Sign::Zero);
    let at_the_end = |v: &&Tpi| [top, tilted, edge, stray].iter().all(|p| on(p, v));
    let whole = cut(&polygons, 1000);
    let ends: Vec<&Tpi> = whole.vertices.iter().filter(at_the_end).collect();
    assert_eq!(ends.len(), 1, "the curve's end on the top's edge");
    for leaf in 1..10 {
        let curves = cut(&polygons, leaf);
        assert_eq!(curves.vertices, whole.vertices, "leaf {leaf}");
        assert_eq!(curves.edges, whole.edges, "leaf {leaf}");
    }
    assert_eq!(*ends[0], Tpi::meeting(top, tilted, edge).unwrap());
}
