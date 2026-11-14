//! Face information (G243a, chapter 11.4 steps 2 and 4): vertices, edges and faces met with faces
//! lowest dimension first, the vertices found before reused, a certified point-in-face test, and
//! each face's On, In and section groups.

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
    Arrangement, BuildError, EdgeIn, EdgeUse, FaceIn, Interfered, Interference, VertexIn, interfere,
};
use geom_bool::merge::default_remaining;
use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_num::P3;
use geom_num::plane::Plane;
use geom_run::{Budget, Exec, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 32))
}

/// The tests' unit, in steps: a millimetre on a grid of a micrometre, so the contact radius,
/// `ε_fit` = 64 steps, is small beside every length here.
const S: i64 = 1000;

/// A grid point given in the tests' unit.
fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x * S, y * S, z * S).unwrap()
}

/// A grid point given in steps.
fn raw(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn coords(q: P3) -> [f64; 3] {
    [q.x.get() as f64, q.y.get() as f64, q.z.get() as f64]
}

/// The corners and faces of a box: faces in the order x low, x high, y low, y high, z low, z high.
struct Cuboid {
    corners: [usize; 8],
    faces: [usize; 6],
}

/// Builds an arrangement entity by entity.
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

    /// A straight edge from vertex `a` to vertex `b`, parameter 0 at `a` and 1 at `b`.
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

    /// A face whose loops are each listed in order round it with the face on their left, seen
    /// from outside its carrier: each edge run on from the end it shares with the edge before it
    /// (the first towards the edge after it), a closed edge forward.
    fn face(&mut self, surface: CarrierIx, loops: Vec<Vec<usize>>, operand: u32) -> usize {
        let mut uses = Vec::new();
        for ring in &loops {
            let mut at = None;
            let mut run = Vec::new();
            for (i, &e) in ring.iter().enumerate() {
                let ends = self.arr.edges[e].ends;
                let forward = match (ends, at) {
                    (None, _) => true,
                    (Some([s, _]), Some(v)) => s == v,
                    (Some([_, t]), None) => self.arr.edges[ring[(i + 1) % ring.len()]]
                        .ends
                        .is_none_or(|[a, b]| a == t || b == t),
                };
                at = ends.map(|[s, t]| if forward { t } else { s });
                run.push(EdgeUse { edge: e, forward });
            }
            uses.push(run);
        }
        self.arr.faces.push(FaceIn {
            surface,
            loops: uses,
            operand,
        });
        self.arr.faces.len() - 1
    }

    /// A flat face through `corners` in order, with a new edge between each and the next.
    fn polygon(&mut self, corners: &[usize], n: [i128; 3], d: i128, operand: u32) -> usize {
        let edges = (0..corners.len())
            .map(|k| self.line(corners[k], corners[(k + 1) % corners.len()], operand))
            .collect();
        let plane = self.plane(n, d);
        self.face(plane, vec![edges], operand)
    }

    /// A box from `lo` to `hi`: its 8 corners (bit k of the index picks the high side of axis k),
    /// its 12 edges and its 6 faces.
    fn cuboid(&mut self, lo: [i64; 3], hi: [i64; 3], operand: u32) -> Cuboid {
        let mut corners = [0; 8];
        for (k, slot) in corners.iter_mut().enumerate() {
            let pick = |axis: usize| {
                if k >> axis & 1 == 1 {
                    hi[axis]
                } else {
                    lo[axis]
                }
            };
            *slot = self.vertex(p(pick(0), pick(1), pick(2)), operand);
        }
        let mut edge_of = std::collections::BTreeMap::new();
        for k in 0..8_usize {
            for axis in 0..3 {
                if k >> axis & 1 == 0 {
                    let j = k | 1 << axis;
                    let e = self.line(corners[k], corners[j], operand);
                    edge_of.insert((k, j), e);
                }
            }
        }
        let mut faces = [0; 6];
        for axis in 0..3 {
            let (b, c) = ((axis + 1) % 3, (axis + 2) % 3);
            for side in 0..2_usize {
                let ring: Vec<usize> = [(0, 0), (1, 0), (1, 1), (0, 1)]
                    .iter()
                    .map(|&(sb, sc)| side << axis | sb << b | sc << c)
                    .collect();
                let edges = (0..4)
                    .map(|k| {
                        let (u, v) = (ring[k], ring[(k + 1) % 4]);
                        edge_of[&(u.min(v), u.max(v))]
                    })
                    .collect();
                let mut n = [0_i128; 3];
                n[axis] = 1;
                let at = if side == 1 { hi[axis] } else { lo[axis] };
                let plane = self.plane(n, -i128::from(at * S));
                faces[2 * axis + side] = self.face(plane, vec![edges], operand);
            }
        }
        Cuboid { corners, faces }
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

/// The grid point a vertex of the result is committed to.
fn where_is(build: &Build, out: &Interfered, v: usize) -> P3 {
    build.arr.carriers.commit(&out.vertices[v]).unwrap().0
}

fn points(build: &Build, out: &Interfered, list: &[usize]) -> Vec<[i64; 3]> {
    let mut all: Vec<[i64; 3]> = list
        .iter()
        .map(|&v| {
            let q = where_is(build, out, v);
            [q.x.get() / S, q.y.get() / S, q.z.get() / S]
        })
        .collect();
    all.sort_unstable();
    all
}

/// A wall's face crossed by another wall's edges: the second wall passes through the first, so
/// each of its four edges across the first wall crosses both of its long faces inside them.
#[test]
fn a_walls_face_is_crossed_by_another_walls_edges() {
    let mut b = Build::default();
    let wall = b.cuboid([0, 0, 0], [1000, 100, 300], 0);
    let other = b.cuboid([400, -200, 50], [500, 300, 250], 1);
    let out = b.run().unwrap();
    for (face, y) in [(wall.faces[2], 0), (wall.faces[3], 100)] {
        let info = &out.faces[face];
        assert_eq!(
            points(&b, &out, &info.in_vertices),
            vec![[400, y, 50], [400, y, 250], [500, y, 50], [500, y, 250]],
            "the vertices inside the face at y = {y}"
        );
        assert_eq!(
            info.sections.len(),
            4,
            "a section edge for each face across it"
        );
        assert_eq!(info.on_blocks.len(), 4, "its four boundary edges, uncut");
    }
    // The wall's ends and top and bottom meet nothing.
    for face in [0, 1, 4, 5] {
        let info = &out.faces[wall.faces[face]];
        assert!(info.in_vertices.is_empty() && info.sections.is_empty());
    }
    // Each side of the other wall along the first's thickness has two section edges.
    for face in [0, 1, 4, 5] {
        assert_eq!(out.faces[other.faces[face]].sections.len(), 2);
    }
    assert_eq!(out.sections.len(), 8);
    let made = out
        .interferences
        .iter()
        .filter(|i| matches!(i, Interference::EdgeFace { reused: false, .. }))
        .count();
    assert_eq!(made, 8, "a new vertex for each crossing");
    // Every section edge runs between two of the crossings on one of the wall's long faces: up a
    // side of the other wall, 200 steps, or along its top or bottom, 100 steps.
    for s in &out.sections {
        let (a, c) = (
            where_is(&b, &out, s.from.unwrap().vertex),
            where_is(&b, &out, s.to.unwrap().vertex),
        );
        assert_eq!(a.y.get(), c.y.get(), "{a:?} to {c:?}");
        let run = ((c.x.get() - a.x.get()).abs(), (c.z.get() - a.z.get()).abs());
        assert!(run == (0, 200 * S) || run == (100 * S, 0), "{a:?} to {c:?}");
    }
    // The other wall's edges along y are each cut twice.
    let cut = out
        .splits
        .iter()
        .filter(|s| {
            let e = s.source;
            b.arr.edges[e].operand == 1 && out.paves[e].len() == 4
        })
        .count();
    assert_eq!(cut, 12, "four edges, three pieces each");
}

/// A column through a slab: the column's four upright edges cross the slab's top and bottom
/// inside them, and each of its four sides meets the top and the bottom in a section edge.
#[test]
fn a_column_passes_through_a_slab() {
    let mut b = Build::default();
    let slab = b.cuboid([0, 0, 0], [1000, 1000, 200], 0);
    let column = b.cuboid([400, 400, -300], [500, 500, 500], 1);
    let out = b.run().unwrap();
    for (face, z) in [(slab.faces[4], 0), (slab.faces[5], 200)] {
        let info = &out.faces[face];
        assert_eq!(
            points(&b, &out, &info.in_vertices),
            vec![[400, 400, z], [400, 500, z], [500, 400, z], [500, 500, z]]
        );
        assert_eq!(info.sections.len(), 4);
        assert_eq!(info.in_blocks, Vec::<usize>::new());
    }
    for face in 0..4 {
        assert_eq!(out.faces[column.faces[face]].sections.len(), 2);
    }
    assert_eq!(out.sections.len(), 8);
    // The slab's sides meet nothing: the column stands well inside it.
    for face in 0..4 {
        let info = &out.faces[slab.faces[face]];
        assert!(info.in_vertices.is_empty() && info.sections.is_empty());
    }
}

/// A section curve through an existing corner ends at that corner. A prism's sloping face, on the
/// plane y = z, meets a box's face x = 100 along its diagonal, from the box's corner (100, 0, 0) to
/// its corner (100, 100, 100): the section edge's paves are those two corners, and no vertex is
/// made near either.
#[test]
fn a_section_through_an_existing_corner_ends_at_the_corner() {
    let mut b = Build::default();
    let cube = b.cuboid([0, 0, 0], [100, 100, 100], 0);
    // The prism along x from 50 to 150, its section the triangle (-50, -50), (150, 150), (150, -50)
    // in (y, z).
    let tri = [(-50, -50), (150, 150), (150, -50)];
    let near: Vec<usize> = tri.iter().map(|&(y, z)| b.vertex(p(50, y, z), 1)).collect();
    let far: Vec<usize> = tri
        .iter()
        .map(|&(y, z)| b.vertex(p(150, y, z), 1))
        .collect();
    let slope = b.polygon(&[near[0], near[1], far[1], far[0]], [0, 1, -1], 0, 1);
    let s = i128::from(S);
    b.polygon(&[near[1], near[2], far[2], far[1]], [0, 1, 0], -150 * s, 1);
    b.polygon(&[near[2], near[0], far[0], far[2]], [0, 0, 1], 50 * s, 1);
    b.polygon(&[near[0], near[1], near[2]], [1, 0, 0], -50 * s, 1);
    b.polygon(&[far[0], far[1], far[2]], [1, 0, 0], -150 * s, 1);
    let out = b.run().unwrap();
    let face = cube.faces[1];
    let corner = |k: usize| out.merged[cube.corners[k]];
    // Corner 1 is (100, 0, 0); corner 7 is (100, 100, 100).
    let found: Vec<_> = out
        .sections
        .iter()
        .filter(|s| s.faces == [face, slope] || s.faces == [slope, face])
        .collect();
    assert_eq!(found.len(), 1, "{:?}", out.sections);
    let mut ends = [found[0].from.unwrap().vertex, found[0].to.unwrap().vertex];
    ends.sort_unstable();
    let mut want = [corner(1), corner(7)];
    want.sort_unstable();
    assert_eq!(ends, want, "the section ends at the box's own corners");
    // No vertex was made within the contact radius of either corner.
    for &c in &want {
        let at = where_is(&b, &out, c);
        let crowd = (0..out.vertices.len())
            .filter(|&v| {
                let q = where_is(&b, &out, v);
                (q.x.get() - at.x.get()).abs() <= 64
                    && (q.y.get() - at.y.get()).abs() <= 64
                    && (q.z.get() - at.z.get()).abs() <= 64
            })
            .count();
        assert_eq!(crowd, 1, "one vertex near {at:?}");
    }
    // Both corners lie inside the sloping face.
    for c in want {
        assert!(out.faces[slope].in_vertices.contains(&c));
    }
    // The box's two edges on the plane y = z are partly inside the sloping face: the piece of each
    // from x = 50 to x = 100.
    let inside: Vec<_> = out.faces[slope]
        .in_blocks
        .iter()
        .map(|&k| out.splits[k].source)
        .collect();
    assert_eq!(inside.len(), 2, "{inside:?}");
}

/// A flagged region where an edge meets a face's plane is accepted only where a vertex the two
/// share covers it. A circle touching the top of a slab from above, with no vertex there, is
/// refused; an arc that ends at the touching point, its end lying inside the face, is accepted.
#[test]
fn a_flagged_face_region_needs_a_shared_vertex() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    // c + r cos(πt) x̂ + r sin(πt) ŷ with ŷ = n̂ × x̂ = −ẑ: lowest at t = 1/2, at (500, 500, 0).
    let k = S as f64;
    let circle = Circle::new(
        [500.0 * k, 500.0 * k, 100.0 * k],
        100.0 * k,
        frame,
        (0.0, 2.0),
    )
    .unwrap();
    let mut b = Build::default();
    let slab = b.cuboid([0, 0, -200], [1000, 1000, 0], 0);
    b.edge(CurveKind::Circle(circle), None, 1);
    match b.run() {
        Err(BuildError::Undecided { face, .. }) => assert_eq!(face, slab.faces[5]),
        other => panic!("refused, not {other:?}"),
    }

    let arc = Circle::new(
        [500.0 * k, 500.0 * k, 100.0 * k],
        100.0 * k,
        frame,
        (0.0, 0.5),
    )
    .unwrap();
    let mut b = Build::default();
    let slab = b.cuboid([0, 0, -200], [1000, 1000, 0], 0);
    let start = b.vertex(p(600, 500, 100), 1);
    let touch = b.vertex(p(500, 500, 0), 1);
    b.edge(CurveKind::Circle(arc), Some([start, touch]), 1);
    let out = b.run().unwrap();
    let top = &out.faces[slab.faces[5]];
    assert_eq!(top.in_vertices, vec![out.merged[touch]]);
    assert_eq!(out.sections.len(), 0);
}

/// The point-in-face test on a square with a square hole, flat on z = 0, and on a sloping square:
/// inside, in the hole, outside, on the boundary, and on lines through corners, where the first
/// rays are refused and later ones decide.
#[test]
fn points_are_placed_in_a_face_with_a_hole() {
    let mut b = Build::default();
    let outer: Vec<usize> = [(0, 0), (1000, 0), (1000, 1000), (0, 1000)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
        .collect();
    let hole: Vec<usize> = [(400, 400), (400, 600), (600, 600), (600, 400)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
        .collect();
    let ring = |b: &mut Build, c: &[usize]| -> Vec<usize> {
        (0..c.len())
            .map(|k| b.line(c[k], c[(k + 1) % c.len()], 0))
            .collect()
    };
    let (o, h) = (ring(&mut b, &outer), ring(&mut b, &hole));
    let carrier = b.plane([0, 0, 1], 0);
    let face = b.face(carrier, vec![o.clone(), h.clone()], 0);
    let m = &mut meter();
    let flat = FlatFace::new(&b.arr, face, m).unwrap().unwrap();
    let place =
        |q: P3, m: &mut WorkMeter<'_>| flat.place(&b.arr.carriers, q, Bound::ZERO, m).unwrap();
    assert_eq!(place(p(100, 100, 0), m), Place::In);
    assert_eq!(place(p(500, 500, 0), m), Place::Out, "in the hole");
    assert_eq!(place(p(2000, 0, 0), m), Place::Out);
    assert_eq!(place(p(0, 500, 0), m), Place::On(vec![o[3]]));
    assert_eq!(place(p(400, 450, 0), m), Place::On(vec![h[0]]));
    // A step off an edge is proven apart from it, however near: in or out by the rays.
    assert_eq!(place(raw(-1, 500 * S, 0), m), Place::Out);
    assert_eq!(place(raw(1, 500 * S, 0), m), Place::In);
    assert_eq!(place(raw(400 * S - 1, 450 * S, 0), m), Place::In);
    assert_eq!(place(raw(400 * S + 1, 450 * S, 0), m), Place::Out);
    // A step from a corner, on its far side.
    assert_eq!(place(raw(400 * S - 1, 400 * S - 1, 0), m), Place::In);
    assert_eq!(place(raw(400 * S + 1, 400 * S + 1, 0), m), Place::Out);
    // Level with the hole's corners and the outer corners: rays along the axes are refused.
    assert_eq!(place(p(100, 400, 0), m), Place::In);
    assert_eq!(place(p(400, 100, 0), m), Place::In);
    assert_eq!(
        place(p(700, 700, 0), m),
        Place::In,
        "on the diagonal through four corners"
    );
    assert_eq!(place(p(1100, 1100, 0), m), Place::Out);
    // Off the plane, the point's picture decides.
    assert_eq!(place(raw(100 * S, 100 * S, 30), m), Place::In);
    assert_eq!(place(raw(500 * S, 500 * S, 30), m), Place::Out);

    // A square on the sloping plane z = x: the normal (1, 0, −1) is largest in x and z alike, and
    // the first of them, x, is dropped.
    let mut b = Build::default();
    let c: Vec<usize> = [(0, 0, 0), (100, 0, 100), (100, 100, 100), (0, 100, 0)]
        .iter()
        .map(|&(x, y, z)| b.vertex(p(x, y, z), 0))
        .collect();
    let face = b.polygon(&c, [1, 0, -1], 0, 0);
    let flat = FlatFace::new(&b.arr, face, m).unwrap().unwrap();
    let place =
        |q: P3, m: &mut WorkMeter<'_>| flat.place(&b.arr.carriers, q, Bound::ZERO, m).unwrap();
    assert_eq!(place(p(50, 50, 50), m), Place::In);
    assert_eq!(place(p(150, 50, 150), m), Place::Out);
    assert_eq!(place(p(50, 150, 50), m), Place::Out);
    assert!(matches!(place(p(50, 0, 50), m), Place::On(_)));
}

/// A face on a torus is not placed in: what meets its box waits for face splitting.
#[test]
fn a_curved_face_keeps_what_meets_it_pending() {
    let mut b = Build::default();
    let slab = b.cuboid([0, 0, 0], [1000, 1000, 200], 0);
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let k = S as f64;
    // The upper half of a torus of radii 60 and 30 about (500, 500, 180), between its inner
    // and outer circles at that height: seen from outside, on the outer circle's left and the
    // inner one's right.
    let mut rims = Vec::new();
    for radius in [90.0, 30.0] {
        let ring = Circle::new(
            [500.0 * k, 500.0 * k, 180.0 * k],
            radius * k,
            frame,
            (0.0, 2.0),
        )
        .unwrap();
        rims.push(b.edge(CurveKind::Circle(ring), None, 1));
    }
    let torus = geom_surface::Torus::new(
        [500.0 * k, 500.0 * k, 180.0 * k],
        60.0 * k,
        30.0 * k,
        frame,
        ((0.0, 2.0), (0.0, 1.0)),
    )
    .unwrap();
    let ix = b
        .arr
        .carriers
        .add_surface(geom_topo::SurfaceKind::Torus(torus))
        .unwrap();
    b.arr.faces.push(FaceIn {
        surface: CarrierIx::Surface(ix),
        loops: vec![
            vec![EdgeUse {
                edge: rims[0],
                forward: true,
            }],
            vec![EdgeUse {
                edge: rims[1],
                forward: false,
            }],
        ],
        operand: 1,
    });
    let ring = b.arr.faces.len() - 1;
    let out = b.run().unwrap();
    let info = &out.faces[ring];
    assert!(info.in_vertices.is_empty() && info.sections.is_empty());
    // The ring rises through the slab's top: that pair waits; the bottom is far below it.
    assert_eq!(info.pending.faces, vec![slab.faces[5]]);
    assert_eq!(out.faces[slab.faces[5]].pending.faces, vec![ring]);
    assert_eq!(out.faces[slab.faces[4]].pending.faces, Vec::<usize>::new());
}
