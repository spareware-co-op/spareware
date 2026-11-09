//! Attacks on G243a's claims (chapter 11.4 steps 2 and 4): the face stages at the frame's edge,
//! rays through many corners and along edges, round holes with a seam, faces on one plane, an edge
//! lying in a face, a flagged region far from any vertex, and the same answer every time.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "test code: a failure should stop loudly"
)]

use geom_bool::carrier::{Bound, CarrierIx, PointGeom};
use geom_bool::inside::{FlatFace, Place};
use geom_bool::interfere::{
    Arrangement, BuildError, EdgeIn, FaceIn, Interfered, Interference, VertexIn, interfere,
};
use geom_bool::merge::default_remaining;
use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_num::P3;
use geom_num::plane::Plane;
use geom_run::{Budget, Exec, WorkMeter};

/// The largest coordinate of the frame, in steps: 2^40 − 1.
const EDGE: i64 = (1 << 40) - 1;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 32))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn coords(q: P3) -> [f64; 3] {
    [q.x.get() as f64, q.y.get() as f64, q.z.get() as f64]
}

#[derive(Default)]
struct Build {
    arr: Arrangement,
}

impl Build {
    fn vertex(&mut self, at: P3, operand: u32) -> usize {
        self.arr.vertices.push(VertexIn {
            point: PointGeom::Grid(at),
            operand,
        });
        self.arr.vertices.len() - 1
    }

    fn edge(&mut self, curve: CurveKind<3>, ends: Option<[usize; 2]>, operand: u32) -> usize {
        let curve = self.arr.carriers.add_curve(curve).unwrap();
        self.arr.edges.push(EdgeIn {
            curve,
            ends,
            operand,
        });
        self.arr.edges.len() - 1
    }

    fn at(&self, v: usize) -> P3 {
        match self.arr.vertices[v].point {
            PointGeom::Grid(q) => q,
            _ => panic!("a grid vertex"),
        }
    }

    fn line(&mut self, a: usize, b: usize, operand: u32) -> usize {
        let (o, e) = (coords(self.at(a)), coords(self.at(b)));
        let d = [e[0] - o[0], e[1] - o[1], e[2] - o[2]];
        let line = Line::new(o, d, (0.0, 1.0)).unwrap();
        self.edge(CurveKind::Line(line), Some([a, b]), operand)
    }

    fn plane(&mut self, n: [i128; 3], d: i128) -> CarrierIx {
        CarrierIx::Plane(
            self.arr
                .carriers
                .add_plane(&Plane::new(n, d).unwrap())
                .unwrap(),
        )
    }

    fn face(&mut self, surface: CarrierIx, loops: Vec<Vec<usize>>, operand: u32) -> usize {
        self.arr.faces.push(FaceIn {
            surface,
            loops,
            operand,
        });
        self.arr.faces.len() - 1
    }

    fn ring(&mut self, corners: &[usize], operand: u32) -> Vec<usize> {
        (0..corners.len())
            .map(|k| self.line(corners[k], corners[(k + 1) % corners.len()], operand))
            .collect()
    }

    /// A flat face on z = `z` through the given (x, y) corners in order.
    fn flat(&mut self, xy: &[(i64, i64)], z: i64, operand: u32) -> (usize, Vec<usize>) {
        let corners: Vec<usize> = xy
            .iter()
            .map(|&(x, y)| self.vertex(p(x, y, z), operand))
            .collect();
        let edges = self.ring(&corners, operand);
        let plane = self.plane([0, 0, 1], -i128::from(z));
        (self.face(plane, vec![edges], operand), corners)
    }

    fn run(&self) -> Result<Interfered, BuildError> {
        interfere(
            &self.arr,
            default_remaining(),
            &Exec::serial(),
            &mut meter(),
        )
    }
}

/// A square face across the whole frame, and a vertical edge of another operand through it near
/// a corner of the frame: every product stays in its budget, and the crossing is In.
#[test]
fn the_face_stages_hold_at_the_frame_s_edge() {
    let mut b = Build::default();
    let (face, _) = b.flat(
        &[(-EDGE, -EDGE), (EDGE, -EDGE), (EDGE, EDGE), (-EDGE, EDGE)],
        0,
        0,
    );
    let (lo, hi) = (
        b.vertex(p(EDGE - 5, EDGE - 7, -EDGE), 1),
        b.vertex(p(EDGE - 5, EDGE - 7, EDGE), 1),
    );
    b.line(lo, hi, 1);
    let out = b.run().unwrap();
    let inside = &out.faces[face].in_vertices;
    assert_eq!(inside.len(), 1);
    let at = b.arr.carriers.commit(&out.vertices[inside[0]]).unwrap().0;
    assert_eq!(at, p(EDGE - 5, EDGE - 7, 0));
}

/// A star of many corners: points level with corners, on the diagonals through them, and a step
/// from them are all placed, the refused rays giving way to later ones; none is unclassified.
#[test]
fn rays_through_corners_give_way_to_others() {
    let mut b = Build::default();
    // An eight-pointed star about (0, 0): points at radius 1,000,000, notches at 400,000, on the
    // whole numbers of a regular octagon's directions rounded to the grid.
    let mut xy = Vec::new();
    for k in 0..16 {
        let angle = core::f64::consts::PI * f64::from(k) / 8.0;
        let r = if k % 2 == 0 { 1_000_000.0 } else { 400_000.0 };
        xy.push((
            (r * angle.cos()).round() as i64,
            (r * angle.sin()).round() as i64,
        ));
    }
    let (face, _) = b.flat(&xy, 0, 0);
    let m = &mut meter();
    let flat = FlatFace::new(&b.arr, face, m).unwrap().unwrap();
    let place =
        |q: P3, m: &mut WorkMeter<'_>| flat.place(&b.arr.carriers, q, Bound::ZERO, m).unwrap();
    assert_eq!(
        place(p(0, 0, 0), m),
        Place::In,
        "the centre, level with two points"
    );
    for &(x, y) in &xy {
        // Halfway to each corner, and just past each.
        assert_eq!(
            place(p(x / 2, y / 2, 0), m),
            Place::In,
            "halfway to ({x}, {y})"
        );
        let (dx, dy) = (x.signum(), y.signum());
        assert_eq!(
            place(p(x + 3 * dx, y + 3 * dy, 0), m),
            Place::Out,
            "past ({x}, {y})"
        );
    }
    // Level with a point, outside the star, in all four directions.
    assert_eq!(place(p(2_000_000, 0, 0), m), Place::Out);
    assert_eq!(place(p(0, -2_000_000, 0), m), Place::Out);
}

/// A face with a round hole: a closed circle edge, whose seam is a join. Points level with the seam,
/// in the hole and beside it, are placed.
#[test]
fn a_round_hole_s_seam_is_a_join() {
    let mut b = Build::default();
    let corners: Vec<usize> = [
        (0, 0),
        (1_000_000, 0),
        (1_000_000, 1_000_000),
        (0, 1_000_000),
    ]
    .iter()
    .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
    .collect();
    let outer = b.ring(&corners, 0);
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    // Its seam, t = 0, at (700000, 500000).
    let circle = Circle::new([500_000.0, 500_000.0, 0.0], 200_000.0, frame, (0.0, 2.0)).unwrap();
    let hole = b.edge(CurveKind::Circle(circle), None, 0);
    let carrier = b.plane([0, 0, 1], 0);
    let face = b.face(carrier, vec![outer, vec![hole]], 0);
    let m = &mut meter();
    let flat = FlatFace::new(&b.arr, face, m).unwrap().unwrap();
    let place =
        |q: P3, m: &mut WorkMeter<'_>| flat.place(&b.arr.carriers, q, Bound::ZERO, m).unwrap();
    assert_eq!(
        place(p(500_000, 500_000, 0), m),
        Place::Out,
        "the hole's centre"
    );
    assert_eq!(
        place(p(600_000, 500_000, 0), m),
        Place::Out,
        "in the hole, level with the seam"
    );
    assert_eq!(place(p(800_000, 500_000, 0), m), Place::In, "past the seam");
    assert_eq!(
        place(p(100_000, 500_000, 0), m),
        Place::In,
        "before the hole, level with the seam"
    );
    assert_eq!(
        place(p(700_000, 500_000, 0), m),
        Place::On(vec![4]),
        "on the seam itself"
    );
}

/// Faces of two operands on one plane are the same carrier, not a section; parallel faces apart
/// make nothing; an edge of one lying in the other's face is In piece by piece.
#[test]
fn faces_on_one_plane_and_edges_lying_in_a_face() {
    let mut b = Build::default();
    let (floor, _) = b.flat(
        &[
            (0, 0),
            (1_000_000, 0),
            (1_000_000, 1_000_000),
            (0, 1_000_000),
        ],
        0,
        0,
    );
    // A smaller square on the same plane, half over the edge of the floor.
    let (mat, corners) = b.flat(
        &[
            (800_000, 200_000),
            (1_200_000, 200_000),
            (1_200_000, 600_000),
            (800_000, 600_000),
        ],
        0,
        1,
    );
    // A square above, parallel.
    let (roof, _) = b.flat(
        &[(0, 0), (500_000, 0), (500_000, 500_000), (0, 500_000)],
        300_000,
        2,
    );
    let out = b.run().unwrap();
    assert_eq!(out.faces[floor].same_carrier, vec![mat]);
    assert_eq!(out.faces[mat].same_carrier, vec![floor]);
    assert!(out.sections.is_empty(), "{:?}", out.sections);
    assert!(out.faces[roof].same_carrier.is_empty() && out.faces[roof].in_blocks.is_empty());
    // The mat's two corners on the floor are In it; its edges are cut where they cross the floor's
    // edge x = 1,000,000, and the pieces over the floor are In.
    let inside: Vec<usize> = out.faces[floor].in_vertices.clone();
    assert_eq!(inside, vec![out.merged[corners[0]], out.merged[corners[3]]]);
    let pieces = out.faces[floor].in_blocks.len();
    assert_eq!(
        pieces, 3,
        "the mat's left side, and the inner piece of its top and bottom"
    );
}

/// A circle crossing a face's plane twice, inside the face: two new vertices, each a pave on the
/// circle, and both In; a closed edge has no ends, and its pieces run round its seam.
#[test]
fn a_closed_curve_crossing_a_face_twice() {
    let mut b = Build::default();
    let (face, _) = b.flat(
        &[
            (0, 0),
            (1_000_000, 0),
            (1_000_000, 1_000_000),
            (0, 1_000_000),
        ],
        0,
        0,
    );
    // x̂ = ẑ puts the seam, t = 0, at the top, away from the plane: the crossings are at t = 1/2
    // and t = 3/2, where ŷ = n̂ × x̂ = x̂ of space.
    let frame = Frame::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0]).unwrap();
    let ring = Circle::new([500_000.0, 500_000.0, 0.0], 300_000.0, frame, (0.0, 2.0)).unwrap();
    let e = b.edge(CurveKind::Circle(ring), None, 1);
    let out = b.run().unwrap();
    let mut at: Vec<[i64; 3]> = out.faces[face]
        .in_vertices
        .iter()
        .map(|&v| {
            let q = b.arr.carriers.commit(&out.vertices[v]).unwrap().0;
            [q.x.get(), q.y.get(), q.z.get()]
        })
        .collect();
    at.sort_unstable();
    assert_eq!(at, vec![[200_000, 500_000, 0], [800_000, 500_000, 0]]);
    assert_eq!(out.paves[e].len(), 2);
    assert_eq!(out.splits.iter().filter(|s| s.source == e).count(), 2);
}

/// A circle touching a face's plane from above far from any vertex: the tangency is flagged, and
/// the boundary of the face does not save it, wherever the touching point lies inside it.
#[test]
fn a_tangency_inside_a_face_is_refused_without_a_vertex() {
    for (cx, cy) in [(500_000.0, 500_000.0), (5_000.0, 900_000.0)] {
        let mut b = Build::default();
        let (face, _) = b.flat(
            &[
                (0, 0),
                (1_000_000, 0),
                (1_000_000, 1_000_000),
                (0, 1_000_000),
            ],
            0,
            0,
        );
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
        let ring = Circle::new([cx, cy, 1_000.0], 1_000.0, frame, (0.0, 2.0)).unwrap();
        b.edge(CurveKind::Circle(ring), None, 1);
        match b.run() {
            Err(BuildError::Undecided { face: f, .. }) => assert_eq!(f, face),
            other => panic!("refused, not {other:?}"),
        }
    }
    // The same circle touching the plane outside a triangular face but inside its box, far from its
    // edges: the flagged region is proven outside.
    let mut b = Build::default();
    b.flat(&[(0, 0), (1_000_000, 0), (0, 1_000_000)], 0, 0);
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    let ring = Circle::new([800_000.0, 800_000.0, 1_000.0], 1_000.0, frame, (0.0, 2.0)).unwrap();
    b.edge(CurveKind::Circle(ring), None, 1);
    let out = b.run().unwrap();
    assert_eq!(out.faces[0].in_vertices, Vec::<usize>::new());
}

/// The same arrangement gives the same face information every time, and the vertex with face and
/// edge with face meetings are listed lowest dimension first.
#[test]
fn the_face_stages_are_deterministic_and_lowest_dimension_first() {
    let build = || {
        let mut b = Build::default();
        b.flat(
            &[
                (0, 0),
                (1_000_000, 0),
                (1_000_000, 1_000_000),
                (0, 1_000_000),
            ],
            0,
            0,
        );
        // A tetrahedron of another operand with one corner on the face and an edge through it.
        let a = b.vertex(p(500_000, 500_000, 0), 1);
        let c = b.vertex(p(200_000, 300_000, -400_000), 1);
        let d = b.vertex(p(300_000, 200_000, 400_000), 1);
        b.line(a, c, 1);
        b.line(c, d, 1);
        b.line(d, a, 1);
        b
    };
    let (first, second) = (build().run().unwrap(), build().run().unwrap());
    assert_eq!(first, second);
    let rank = |i: &Interference| match i {
        Interference::VertexVertex { .. } => 0,
        Interference::VertexEdge { .. } => 1,
        Interference::EdgeEdge { .. } => 2,
        Interference::VertexFace { .. } => 3,
        Interference::EdgeFace { .. } => 4,
        Interference::FaceFace { .. } => 5,
    };
    let ranks: Vec<u8> = first.interferences.iter().map(rank).collect();
    assert!(ranks.windows(2).all(|w| w[0] <= w[1]), "{ranks:?}");
    // The corner on the face is In it, and the edge from c to d crosses it at a new vertex; the two
    // edges ending at the corner meet the face there, at the corner, reused.
    let made = first
        .interferences
        .iter()
        .filter(|i| matches!(i, Interference::EdgeFace { reused: false, .. }))
        .count();
    assert_eq!(made, 1);
    assert_eq!(first.faces[0].in_vertices.len(), 2);
}
