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
use geom_bool::crossing::{Enclosed, Hull, crossings};
use geom_bool::distance::Carriers;
use geom_bool::inside::{FlatFace, Place};
use geom_bool::interfere::{
    Arrangement, BuildError, EdgeIn, FaceIn, Interfered, Interference, VertexIn, interfere,
};
use geom_bool::merge::default_remaining;
use geom_bool::rule::{ContactError, NearContact};
use geom_curve::{Circle, CurveKind, Frame, Helix, HelixAxes, Line, ParamCurve};
use geom_num::P3;
use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::{FlatSurface, SurfaceKind};

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
    let reused = first
        .interferences
        .iter()
        .filter(|i| matches!(i, Interference::EdgeFace { reused: true, .. }))
        .count();
    assert_eq!(
        reused, 2,
        "the two edges ending at the corner meet the face there"
    );
    assert_eq!(first.faces[0].in_vertices.len(), 2);
}

/// A U-shaped face across a large one: along the line where their planes meet, the stretch across
/// the U's notch lies inside the large face only, and is no section edge.
#[test]
fn a_section_is_kept_only_where_inside_both_faces() {
    let mut b = Build::default();
    // The large face on y = 0.
    let big: Vec<usize> = [
        (0, 0),
        (1_000_000, 0),
        (1_000_000, 1_000_000),
        (0, 1_000_000),
    ]
    .iter()
    .map(|&(x, z)| b.vertex(p(x, 0, z), 0))
    .collect();
    let edges = b.ring(&big, 0);
    let plane = b.plane([0, 1, 0], 0);
    let large = b.face(plane, vec![edges], 0);
    // The U on x = 500,000, open towards −y, its notch across y = 0 from z = 300,000 to 700,000.
    let u: Vec<usize> = [
        (100_000, 100_000),
        (100_000, 900_000),
        (-100_000, 900_000),
        (-100_000, 700_000),
        (50_000, 700_000),
        (50_000, 300_000),
        (-100_000, 300_002),
        (-100_000, 100_000),
    ]
    .iter()
    .map(|&(y, z)| b.vertex(p(500_000, y, z), 1))
    .collect();
    let edges = b.ring(&u, 1);
    let plane = b.plane([1, 0, 0], -500_000);
    let shape = b.face(plane, vec![edges], 1);
    let out = b.run().unwrap();
    let mut spans: Vec<(i64, i64)> = out
        .sections
        .iter()
        .map(|s| {
            assert_eq!(s.faces, [large, shape]);
            let z = |v: usize| b.arr.carriers.commit(&out.vertices[v]).unwrap().0.z.get();
            let (a, c) = (z(s.from.vertex), z(s.to.vertex));
            (a.min(c), a.max(c))
        })
        .collect();
    spans.sort_unstable();
    // The notch's lower side slopes a little, so it crosses y = 0 off the grid, near z = 300,000.67:
    // a vertex proven within its bound, which widens its paves.
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].0, 100_000);
    assert!((spans[0].1 - 300_000).abs() <= 1, "{spans:?}");
    assert_eq!(spans[1], (700_000, 900_000));
    // Each section's paves are widened by their vertices' bounds.
    for s in &out.sections {
        for pave in [s.from, s.to] {
            let (_, bound) = b.arr.carriers.commit(&out.vertices[pave.vertex]).unwrap();
            let width = pave.t.hi() - pave.t.lo();
            assert!(
                width >= 2.0 * bound.steps() as f64,
                "{pave:?} within {bound:?}"
            );
        }
    }
}

/// A circle lying in a square face and crossing its edge: cut at the two crossings into an arc
/// inside and one outside, the outside one running round the circle's seam. Only the inside arc is
/// In, whichever of them runs round the seam.
#[test]
fn a_closed_curve_lying_in_a_face_is_in_where_inside() {
    for (cx, inside_wraps) in [(1_000_000.0, false), (0.0, true)] {
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
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        // Centred on the face's right edge, x = 1,000,000 (its seam, t = 0, outside), or on its
        // left edge, x = 0 (its seam inside).
        let ring = Circle::new([cx, 500_000.0, 0.0], 200_000.0, frame, (0.0, 2.0)).unwrap();
        let e = b.edge(CurveKind::Circle(ring), None, 1);
        let out = b.run().unwrap();
        let pieces: Vec<usize> = (0..out.splits.len())
            .filter(|&k| out.splits[k].source == e)
            .collect();
        assert_eq!(pieces.len(), 2);
        let inside = &out.faces[face].in_blocks;
        assert_eq!(inside.len(), 1, "{inside:?}");
        assert_eq!(out.splits[inside[0]].block.wraps, inside_wraps);
    }
}

/// A region the crossing engine flags just outside a face, beyond its sloping side, on planes that
/// drop each axis in turn: proven outside by its centre and reach, so accepted; the same tangency
/// inside the face is refused.
#[test]
fn a_flagged_region_just_outside_a_face_is_proven_outside() {
    // The triangle (0, 0), (1,000,000, 0), (0, 1,000,000) in the plane's two kept coordinates, and
    // a circle of radius 1,000 touching the plane at (600,000, 500,000), beyond the hypotenuse by
    // about 70,000; at (300,000, 250,000) it is inside.
    for axis in 0..3 {
        for (at, refused) in [
            ((600_000.0, 500_000.0), false),
            ((300_000.0, 250_000.0), true),
        ] {
            let lay = |u: f64, v: f64, w: f64| -> [f64; 3] {
                let mut q = [0.0; 3];
                q[(axis + 1) % 3] = u;
                q[(axis + 2) % 3] = v;
                q[axis] = w;
                q
            };
            let mut b = Build::default();
            let corners: Vec<usize> = [(0.0, 0.0), (1_000_000.0, 0.0), (0.0, 1_000_000.0)]
                .iter()
                .map(|&(u, v)| {
                    let q = lay(u, v, 0.0);
                    b.vertex(p(q[0] as i64, q[1] as i64, q[2] as i64), 0)
                })
                .collect();
            let edges = b.ring(&corners, 0);
            let mut n = [0_i128; 3];
            n[axis] = 1;
            let plane = b.plane(n, 0);
            b.face(plane, vec![edges], 0);
            // The circle stands in the plane holding the dropped axis and the second kept one.
            let frame = Frame::new(lay(0.0, 0.0, 1.0), lay(1.0, 0.0, 0.0)).unwrap();
            let ring = Circle::new(lay(at.0, at.1, 1_000.0), 1_000.0, frame, (0.0, 2.0)).unwrap();
            b.edge(CurveKind::Circle(ring), None, 1);
            match (b.run(), refused) {
                (Err(BuildError::Undecided { .. }), true) | (Ok(_), false) => {}
                (other, _) => panic!("axis {axis} at {at:?}: {other:?}"),
            }
        }
    }
}

/// A face on a curved carrier whose box passes the frame, a plane surface of sides far past it:
/// its box is the whole frame, so every edge and face of another operand waits for it.
#[test]
fn an_unbounded_carrier_s_face_takes_the_whole_frame() {
    let mut b = Build::default();
    let (floor, _) = b.flat(&[(0, 0), (1_000, 0), (1_000, 1_000), (0, 1_000)], 0, 0);
    let surface = FlatSurface::new(
        [0.0, 0.0, 500_000_000.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        ((-1.0e13, 1.0e13), (-1.0e13, 1.0e13)),
    )
    .unwrap();
    let ix = b
        .arr
        .carriers
        .add_surface(SurfaceKind::Plane(surface))
        .unwrap();
    let corners: Vec<usize> = [(0, 0), (1, 0), (1, 1)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 500_000_000), 1))
        .collect();
    let edges = b.ring(&corners, 1);
    let far = b.face(CarrierIx::Surface(ix), vec![edges], 1);
    let out = b.run().unwrap();
    assert_eq!(out.faces[far].pending.faces, vec![floor]);
    assert_eq!(out.faces[far].pending.edges.len(), 4);
}

/// The quick box tests of a flat face: a point outside the box of its boundary in the picture by
/// more than its bound, and a box apart from it.
#[test]
fn the_box_tests_of_a_flat_face() {
    let mut b = Build::default();
    let (face, _) = b.flat(&[(0, 0), (1_000, 0), (1_000, 1_000), (0, 1_000)], 0, 0);
    let flat = FlatFace::new(&b.arr, face, &mut meter()).unwrap().unwrap();
    let w = |k: u64| Bound(geom_num::Steps(k));
    // Five steps left of the box, and five right of it.
    assert!(!flat.beyond(p(-5, 500, 0), w(5)));
    assert!(flat.beyond(p(-5, 500, 0), w(4)));
    assert!(!flat.beyond(p(1_005, 500, 0), w(5)));
    assert!(flat.beyond(p(1_005, 500, 0), w(4)));
    assert!(!flat.beyond(p(500, 1_005, 0), w(5)));
    assert!(flat.beyond(p(500, -5, 0), w(4)));
    assert!(
        !flat.beyond(p(500, 500, 99), w(0)),
        "the dropped axis does not count"
    );
    let iv = |lo: [f64; 3], hi: [f64; 3]| (lo, hi);
    let (lo, hi) = iv([1_001.0, 0.0, 0.0], [1_002.0, 1.0, 0.0]);
    assert!(flat.apart_from(lo, hi));
    let (lo, hi) = iv([1_000.0, 0.0, 0.0], [1_002.0, 1.0, 0.0]);
    assert!(!flat.apart_from(lo, hi));
    let (lo, hi) = iv([-5.0, 0.0, 0.0], [-1.0, 1.0, 0.0]);
    assert!(flat.apart_from(lo, hi));
    let (lo, hi) = iv([-5.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    assert!(!flat.apart_from(lo, hi));
    let (lo, hi) = iv([10.0, -9.0, 0.0], [20.0, -1.0, 0.0]);
    assert!(flat.apart_from(lo, hi));
    let (lo, hi) = iv([10.0, 1_001.0, -50.0], [20.0, 1_009.0, 50.0]);
    assert!(flat.apart_from(lo, hi));
}

/// Every error of the face stages says what went wrong.
#[test]
fn the_face_stages_errors_say_what() {
    let at = geom_num::P3::new(1, 2, 3).unwrap();
    let pave = geom_bool::pave::Pave {
        vertex: 4,
        t: Interval::point(0.5).unwrap(),
    };
    let near = ContactError::NearContact(NearContact {
        enclosure: Interval::new(0.0, 100.0).unwrap(),
        refinement: 8,
    });
    for (error, words) in [
        (
            BuildError::Undecided {
                edge: 2,
                face: 3,
                why: geom_intersect::solve::Reason::Singular,
                at: geom_topo::Box3::from_point(at),
            },
            "edge 2 meets the plane of face 3",
        ),
        (BuildError::Unclassified { face: 7, at }, "face 7"),
        (
            BuildError::SectionOrder {
                faces: [5, 6],
                first: pave,
                second: pave,
            },
            "faces 5 and 6",
        ),
        (BuildError::Contact(near), "refinement limit"),
    ] {
        let text = format!("{error}");
        assert!(text.contains(words), "{text}");
    }
}

fn segment(o: [f64; 3], d: [f64; 3]) -> CurveKind<3> {
    CurveKind::Line(Line::new(o, d, (0.0, 1.0)).unwrap())
}

fn floor_plane() -> Plane {
    Plane::new([0, 0, 1], 0).unwrap()
}

/// A segment meets a plane once, at its end, or not at all; lying in it, it is flagged whole.
#[test]
fn a_segment_meets_a_plane_once_or_not_at_all() {
    let m = &mut meter();
    let at = |c: &CurveKind<3>, m: &mut WorkMeter<'_>| crossings(c, &floor_plane(), m).unwrap();
    let found = at(&segment([0.0, 0.0, -5.0], [0.0, 0.0, 10.0]), m);
    assert_eq!(found.zeros.len(), 1);
    assert!(found.zeros[0].enclosure[0].contains(0.5));
    // Ending on the plane, and starting on it: the root at the end.
    let found = at(&segment([0.0, 0.0, -10.0], [0.0, 0.0, 10.0]), m);
    assert_eq!(found.zeros.len(), 1);
    assert!(found.zeros[0].enclosure[0].contains(1.0));
    let found = at(&segment([0.0, 0.0, 0.0], [0.0, 0.0, 10.0]), m);
    assert_eq!(found.zeros.len(), 1);
    assert!(found.zeros[0].enclosure[0].contains(0.0));
    // Running down onto it from above, and up to it from below.
    let found = at(&segment([0.0, 0.0, 10.0], [0.0, 0.0, -10.0]), m);
    assert_eq!(found.zeros.len(), 1);
    assert!(found.zeros[0].enclosure[0].contains(1.0));
    // Short of it, and parallel to it above.
    assert_eq!(
        at(&segment([0.0, 0.0, 1.0], [0.0, 0.0, 10.0]), m)
            .zeros
            .len(),
        0
    );
    assert_eq!(
        at(&segment([0.0, 0.0, -11.0], [0.0, 0.0, 10.0]), m)
            .zeros
            .len(),
        0
    );
    let found = at(&segment([0.0, 0.0, 1.0], [10.0, 0.0, 0.0]), m);
    assert!(found.zeros.is_empty() && found.flagged.is_empty());
    // In it: the whole domain flagged.
    let found = at(&segment([0.0, 0.0, 0.0], [10.0, 0.0, 0.0]), m);
    assert_eq!(found.zeros.len(), 0);
    assert_eq!(found.flagged.len(), 1);
    // A circle across the plane: two crossings by the engine.
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    let ring =
        CurveKind::Circle(Circle::new([0.0, 0.0, 300.0], 1_000.0, frame, (0.0, 2.0)).unwrap());
    assert_eq!(at(&ring, m).zeros.len(), 2);
}

/// A segment's hull is its two ends; a curve whose NURBS form is approximate has its spread
/// widened by the form's deviation; a hull says where a curve lies from a plane.
#[test]
fn hulls_hold_their_curves() {
    let m = &mut meter();
    let floor = Enclosed::of(&floor_plane()).unwrap();
    let hull = Hull::of(&segment([1.0, 2.0, 3.0], [4.0, 0.0, -6.0]), m).unwrap();
    let s = hull.spread(&floor).unwrap();
    assert!(
        s.lo() <= -3.0 && s.lo() > -3.0001 && s.hi() >= 3.0 && s.hi() < 3.0001,
        "{s:?}"
    );
    // A helix converts to NURBS within a stated deviation: its spread holds the control points'
    // heights widened by it, both ways.
    let helix = CurveKind::Helix(
        Helix::new(
            [0.0, 0.0, 0.0],
            HelixAxes::Frame {
                frame: Frame::standard().unwrap(),
                radius: 1_000.0,
                pitch: 400.0,
            },
            (0.0, 3.0),
        )
        .unwrap(),
    );
    let form = helix.to_nurbs(m).unwrap();
    assert!(form.deviation > 0.0, "{}", form.deviation);
    let nurbs = &form.value.curve;
    let heights: Vec<f64> = (0..nurbs.homogeneous().len())
        .map(|i| nurbs.control_point(i).unwrap()[2])
        .collect();
    let (low, high) = heights
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), z| {
            (l.min(*z), h.max(*z))
        });
    let s = Hull::of(&helix, m).unwrap().spread(&floor).unwrap();
    assert!(
        s.lo() <= low - form.deviation && s.lo() > low - form.deviation - 1e-6,
        "{s:?} {low}"
    );
    assert!(
        s.hi() >= high + form.deviation && s.hi() < high + form.deviation + 1e-6,
        "{s:?} {high}"
    );
}

/// A segment's distance from a grid point, beside it and past each end, in closed form.
#[test]
fn a_segment_s_distance_beside_it_and_past_each_end() {
    let m = &mut meter();
    let mut carriers = Carriers::new();
    let ix = carriers
        .add_curve(segment([0.0; 3], [10.0, 0.0, 0.0]))
        .unwrap();
    let mut d = |x: i64, y: i64, z: i64| {
        carriers
            .distance(p(x, y, z), CarrierIx::Curve(ix), 0, m)
            .unwrap()
    };
    for (x, y, z) in [
        (5, 3, 4),
        (-3, 4, 0),
        (13, 0, 4),
        (0, 3, 4),
        (10, 4, 3),
        (-4, 0, 3),
        (14, 3, 0),
    ] {
        let e = d(x, y, z);
        assert!(
            e.lo() > 4.999 && e.lo() <= 5.0 && e.hi() >= 5.0 && e.hi() < 5.001,
            "({x}, {y}, {z}): {e:?}"
        );
    }
    let on = d(7, 0, 0);
    assert_eq!(on.lo(), 0.0);
    assert!(on.hi() < 1e-9, "{on:?}");
}
