//! Face splitting for flat carriers (G244, chapter 11.4 step 5): each face cut along its In and
//! section edges, its loops traced by exact orientation, holes given to the face that holds them,
//! and the split faces a partition of the face.

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

use geom_bool::carrier::{CarrierIx, PointGeom};
use geom_bool::interfere::{
    Arrangement, BuildError, EdgeIn, EdgeUse, FaceIn, Interfered, VertexIn, interfere,
};
use geom_bool::merge::default_remaining;
use geom_bool::split::{Piece, SplitFace, Splitting, Use, split};
use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_num::P3;
use geom_num::plane::Plane;
use geom_run::{Budget, Exec, WorkMeter};

/// The tests' unit, in steps: a millimetre on a grid of a micrometre.
const S: i64 = 1000;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 32))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x * S, y * S, z * S).unwrap()
}

fn coords(q: P3) -> [f64; 3] {
    [q.x.get() as f64, q.y.get() as f64, q.z.get() as f64]
}

/// A loop run the other way round.
fn reversed(ring: &[EdgeUse]) -> Vec<EdgeUse> {
    ring.iter()
        .rev()
        .map(|u| EdgeUse {
            edge: u.edge,
            forward: !u.forward,
        })
        .collect()
}

/// The faces of a box: x low, x high, y low, y high, z low, z high.
struct Cuboid {
    faces: [usize; 6],
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

    /// A face whose loops are each listed in order round it with the face on their left, seen
    /// from outside its carrier: for a flat one, from the side its plane's entry in the table
    /// faces ([`Build::runs`]).
    fn face(&mut self, surface: CarrierIx, loops: &[Vec<usize>], operand: u32) -> usize {
        let loops = loops.iter().map(|ring| self.runs(ring)).collect();
        self.face_with(surface, loops, operand)
    }

    /// A flat face on the plane with normal `n` and offset `d`, its loops each listed in order
    /// round it with the face on their left seen from where `n` points: turned round where the
    /// plane's entry in the table faces the other way (its normal's first part that is not zero
    /// is positive).
    fn flat(&mut self, (n, d): ([i128; 3], i128), loops: Vec<Vec<EdgeUse>>, operand: u32) -> usize {
        let plane = self.plane(n, d);
        let flipped = n.iter().find(|v| **v != 0).is_some_and(|v| *v < 0);
        let loops = if flipped {
            loops.into_iter().map(|ring| reversed(&ring)).collect()
        } else {
            loops
        };
        self.face_with(plane, loops, operand)
    }

    fn face_with(&mut self, surface: CarrierIx, loops: Vec<Vec<EdgeUse>>, operand: u32) -> usize {
        self.arr.faces.push(FaceIn {
            surface,
            loops,
            operand,
        });
        self.arr.faces.len() - 1
    }

    /// A loop listed in order round its face as uses: each edge run on from the end it shares
    /// with the edge before it (the first towards the edge after it), a closed edge forward.
    fn runs(&self, ring: &[usize]) -> Vec<EdgeUse> {
        let mut at = None;
        let mut uses = Vec::new();
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
            uses.push(EdgeUse { edge: e, forward });
        }
        uses
    }

    fn ring(&mut self, corners: &[usize], operand: u32) -> Vec<usize> {
        (0..corners.len())
            .map(|k| self.line(corners[k], corners[(k + 1) % corners.len()], operand))
            .collect()
    }

    /// A flat face on the plane (`n`, `d`), its corners counter-clockwise seen from where `n`
    /// points.
    fn polygon(&mut self, corners: &[usize], n: [i128; 3], d: i128, operand: u32) -> usize {
        let edges = self.ring(corners, operand);
        let uses = self.runs(&edges);
        self.flat((n, d), vec![uses], operand)
    }

    /// A box from `lo` to `hi`, in the tests' unit.
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
                faces[2 * axis + side] = self.face(plane, &[edges], operand);
            }
        }
        Cuboid { faces }
    }

    fn run(&self) -> (Interfered, Splitting) {
        let out = interfere(
            &self.arr,
            default_remaining(),
            &Exec::serial(),
            &mut meter(),
        )
        .unwrap();
        let splitting = split(&self.arr, &out, &mut meter()).unwrap();
        (out, splitting)
    }
}

/// The ends of a piece, as grid points, in the order a use runs it.
fn ends(b: &Build, out: &Interfered, one: &Use) -> [P3; 2] {
    let [from, to] = match one.piece {
        Piece::Split(k) => {
            let block = out.splits[k].block;
            [block.from.unwrap().vertex, block.to.unwrap().vertex]
        }
        Piece::Section(s) => [
            out.sections[s].from.unwrap().vertex,
            out.sections[s].to.unwrap().vertex,
        ],
    };
    let at = |v: usize| b.arr.carriers.commit(&out.vertices[v]).unwrap().0;
    if one.forward {
        [at(from), at(to)]
    } else {
        [at(to), at(from)]
    }
}

/// Twice the signed area of a loop of straight pieces in the picture (x, y) or the one given,
/// exactly, and a check that each piece starts where the one before it ends.
fn twice_area(b: &Build, out: &Interfered, uses: &[Use], keep: [usize; 2]) -> i128 {
    let pick = |q: P3| {
        let v = [q.x.get(), q.y.get(), q.z.get()];
        (i128::from(v[keep[0]]), i128::from(v[keep[1]]))
    };
    let mut total = 0_i128;
    for (k, one) in uses.iter().enumerate() {
        let [s, e] = ends(b, out, one);
        let next = ends(b, out, &uses[(k + 1) % uses.len()])[0];
        assert_eq!(e, next, "the loop runs on from where each piece ends");
        let ((x0, y0), (x1, y1)) = (pick(s), pick(e));
        total += x0 * y1 - x1 * y0;
    }
    total
}

/// Twice the area of a split face: its outer loop's less its holes'.
fn twice_face_area(b: &Build, out: &Interfered, face: &SplitFace, keep: [usize; 2]) -> i128 {
    let outer = twice_area(b, out, &face.outer, keep);
    assert!(outer > 0, "the outer loop runs counter-clockwise");
    let mut total = outer;
    for hole in &face.holes {
        let h = twice_area(b, out, hole, keep);
        assert!(h < 0, "a hole runs clockwise");
        total += h;
    }
    total
}

fn of_face(splitting: &Splitting, f: usize) -> Vec<&SplitFace> {
    splitting.faces.iter().filter(|s| s.face == f).collect()
}

/// A wall's long face, crossed by another wall, splits into the opening's rectangle and the rest
/// of the face round it; each side of the crossing wall splits into three strips; the areas add up
/// to each face's exactly.
#[test]
fn a_wall_s_face_is_split_round_a_crossing_wall() {
    let mut b = Build::default();
    let wall = b.cuboid([0, 0, 0], [1000, 100, 300], 0);
    let other = b.cuboid([400, -200, 50], [500, 300, 250], 1);
    let (out, splitting) = b.run();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    // The wall's faces y = 0 and y = 100, in the picture (x, z): their normal is largest in y.
    let keep = [0, 2];
    for f in [wall.faces[2], wall.faces[3]] {
        let parts = of_face(&splitting, f);
        assert_eq!(parts.len(), 2, "{parts:?}");
        let holes: Vec<usize> = parts.iter().map(|s| s.holes.len()).collect();
        assert!(holes.contains(&0) && holes.contains(&1), "{holes:?}");
        let sum: i128 = parts
            .iter()
            .map(|s| twice_face_area(&b, &out, s, keep))
            .sum();
        assert_eq!(sum.abs(), 2 * i128::from(1000 * S) * i128::from(300 * S));
        let opening = parts.iter().find(|s| s.holes.is_empty()).unwrap();
        assert_eq!(opening.outer.len(), 4);
        assert!(
            opening
                .outer
                .iter()
                .all(|u| matches!(u.piece, Piece::Section(_)))
        );
    }
    // A side of the crossing wall, x = 400: three strips, y below 0, between 0 and 100, above.
    let parts = of_face(&splitting, other.faces[0]);
    assert_eq!(parts.len(), 3, "{parts:?}");
    let sum: i128 = parts
        .iter()
        .map(|s| twice_face_area(&b, &out, s, [1, 2]))
        .sum();
    assert_eq!(sum.abs(), 2 * i128::from(500 * S) * i128::from(200 * S));
    // Faces nothing crosses stay whole.
    assert_eq!(of_face(&splitting, wall.faces[0]).len(), 1);
    assert_eq!(of_face(&splitting, wall.faces[0])[0].outer.len(), 4);
}

/// A slab's top, pierced by a column, splits into the column's square and the slab round it.
#[test]
fn a_slab_s_top_is_split_round_a_column() {
    let mut b = Build::default();
    let slab = b.cuboid([0, 0, 0], [1000, 1000, 200], 0);
    b.cuboid([400, 400, -300], [500, 500, 500], 1);
    let (out, splitting) = b.run();
    let parts = of_face(&splitting, slab.faces[5]);
    assert_eq!(parts.len(), 2);
    let ring = parts.iter().find(|s| s.holes.len() == 1).unwrap();
    assert_eq!(ring.outer.len(), 4);
    assert_eq!(ring.holes[0].len(), 4);
    let sum: i128 = parts
        .iter()
        .map(|s| twice_face_area(&b, &out, s, [0, 1]))
        .sum();
    assert_eq!(sum, 2 * i128::from(1000 * S) * i128::from(1000 * S));
}

/// A section through two corners of a face splits it into two triangles meeting along it.
#[test]
fn a_section_through_corners_splits_a_face_in_two() {
    let mut b = Build::default();
    let cube = b.cuboid([0, 0, 0], [100, 100, 100], 0);
    let tri = [(-50, -50), (150, 150), (150, -50)];
    let near: Vec<usize> = tri.iter().map(|&(y, z)| b.vertex(p(50, y, z), 1)).collect();
    let far: Vec<usize> = tri
        .iter()
        .map(|&(y, z)| b.vertex(p(150, y, z), 1))
        .collect();
    let s = i128::from(S);
    b.polygon(&[near[0], near[1], far[1], far[0]], [0, 1, -1], 0, 1);
    b.polygon(&[near[1], near[2], far[2], far[1]], [0, -1, 0], 150 * s, 1);
    b.polygon(&[near[2], near[0], far[0], far[2]], [0, 0, 1], 50 * s, 1);
    b.polygon(&[near[0], near[1], near[2]], [-1, 0, 0], 50 * s, 1);
    b.polygon(&[far[0], far[1], far[2]], [-1, 0, 0], 150 * s, 1);
    let (out, splitting) = b.run();
    let parts = of_face(&splitting, cube.faces[1]);
    assert_eq!(parts.len(), 2, "{parts:?}");
    for part in &parts {
        assert_eq!(part.outer.len(), 3);
        assert_eq!(part.holes.len(), 0);
        assert_eq!(
            twice_face_area(&b, &out, part, [1, 2]),
            i128::from(100 * S) * i128::from(100 * S)
        );
    }
}

/// A face with a round hole and nothing crossing it stays one face with its hole; the inside of
/// the hole is no face.
#[test]
fn a_face_with_a_round_hole_stays_whole() {
    let mut b = Build::default();
    let corners: Vec<usize> = [(0, 0), (1000, 0), (1000, 1000), (0, 1000)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
        .collect();
    let outer = b.ring(&corners, 0);
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let k = S as f64;
    let circle = Circle::new([500.0 * k, 500.0 * k, 0.0], 200.0 * k, frame, (0.0, 2.0)).unwrap();
    let hole = b.edge(CurveKind::Circle(circle), None, 0);
    let plane = b.plane([0, 0, 1], 0);
    // The hole's circle runs counter-clockwise seen from +z: the face is on its right.
    let outer = b.runs(&outer);
    let back = EdgeUse {
        edge: hole,
        forward: false,
    };
    let face = b.face_with(plane, vec![outer, vec![back]], 0);
    let (_, splitting) = b.run();
    let parts = of_face(&splitting, face);
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].outer.len(), 4);
    assert_eq!(parts[0].holes.len(), 1);
    assert_eq!(parts[0].holes[0].len(), 1);
}

/// A plate crossing a face with a round hole, through the hole: the face splits along the
/// sections either side of the hole, into the part left of the plate and the part right of it, each
/// with an arc of the hole's rim in its outer loop, and the plate's strip above and below the hole.
#[test]
fn a_plate_through_a_round_hole_splits_the_face_in_two() {
    let mut b = Build::default();
    let corners: Vec<usize> = [(0, 0), (1000, 0), (1000, 1000), (0, 1000)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
        .collect();
    let outer = b.ring(&corners, 0);
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let k = S as f64;
    let circle = Circle::new([500.0 * k, 500.0 * k, 0.0], 200.0 * k, frame, (0.0, 2.0)).unwrap();
    let hole = b.edge(CurveKind::Circle(circle), None, 0);
    let plane = b.plane([0, 0, 1], 0);
    // The hole's circle runs counter-clockwise seen from +z: the face is on its right.
    let outer = b.runs(&outer);
    let back = EdgeUse {
        edge: hole,
        forward: false,
    };
    let face = b.face_with(plane, vec![outer, vec![back]], 0);
    // A thin upright plate on x = 450, across the whole face.
    let slab = b.cuboid([450, -100, -100], [460, 1100, 100], 1);
    let (_, splitting) = b.run();
    let parts = of_face(&splitting, face);
    assert_eq!(parts.len(), 4, "{parts:?}");
    assert!(parts.iter().all(|s| s.holes.is_empty()), "{parts:?}");
    // The plate's own sides are cut, not split: each section ends inside them, at the hole's rim
    // and at the face's edges.
    for side in [slab.faces[0], slab.faces[1]] {
        let parts = of_face(&splitting, side);
        assert_eq!(parts.len(), 1, "{parts:?}");
        assert_eq!(parts[0].cuts.len(), 2, "{parts:?}");
        assert!(parts[0].cuts.iter().all(|c| c.len() == 2));
    }
}

/// Two faces on one plane, overlapping: each splits along the other's edges inside it, the overlap
/// a face of each.
#[test]
fn faces_on_one_plane_split_each_other() {
    let mut b = Build::default();
    let floor_corners: Vec<usize> = [(0, 0), (1000, 0), (1000, 1000), (0, 1000)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
        .collect();
    let floor = b.polygon(&floor_corners, [0, 0, 1], 0, 0);
    let mat_corners: Vec<usize> = [(800, 200), (1200, 200), (1200, 600), (800, 600)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 0), 1))
        .collect();
    let mat = b.polygon(&mat_corners, [0, 0, 1], 0, 1);
    let (out, splitting) = b.run();
    for (f, whole) in [(floor, 1000 * 1000), (mat, 400 * 400)] {
        let parts = of_face(&splitting, f);
        assert_eq!(parts.len(), 2, "{parts:?}");
        let sum: i128 = parts
            .iter()
            .map(|s| twice_face_area(&b, &out, s, [0, 1]))
            .sum();
        assert_eq!(sum, 2 * i128::from(whole) * i128::from(S) * i128::from(S));
        let overlap = 200 * 400;
        assert!(parts.iter().any(|s| twice_face_area(&b, &out, s, [0, 1])
            == 2 * i128::from(overlap) * i128::from(S) * i128::from(S)));
    }
}

/// A face on a torus waits for its own splitting.
#[test]
fn a_curved_face_waits() {
    let mut b = Build::default();
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let k = S as f64;
    // The upper half of the torus, on the left of its outer circle and the right of its inner.
    let mut rims = Vec::new();
    for radius in [70.0, 30.0] {
        let ring = Circle::new([0.0, 0.0, 0.0], radius * k, frame, (0.0, 2.0)).unwrap();
        rims.push(b.edge(CurveKind::Circle(ring), None, 0));
    }
    let torus = geom_surface::Torus::new(
        [0.0; 3],
        50.0 * k,
        20.0 * k,
        frame,
        ((0.0, 2.0), (0.0, 1.0)),
    )
    .unwrap();
    let ix = b
        .arr
        .carriers
        .add_surface(geom_topo::SurfaceKind::Torus(torus))
        .unwrap();
    let outer = EdgeUse {
        edge: rims[0],
        forward: true,
    };
    let inner = EdgeUse {
        edge: rims[1],
        forward: false,
    };
    let ring = b.face_with(CarrierIx::Surface(ix), vec![vec![outer], vec![inner]], 0);
    let (_, splitting) = b.run();
    assert_eq!(splitting.waiting, vec![ring]);
    assert_eq!(splitting.faces.len(), 0);
}

/// A flat face whose loop runs with the face on its right, round the outside of a square: no
/// face is that, and splitting refuses it. A loop whose edges do not run end to end is refused
/// too.
#[test]
fn a_flat_face_with_its_loop_the_wrong_way_is_refused() {
    let mut b = Build::default();
    let corners: Vec<usize> = [(0, 0), (1000, 0), (1000, 1000), (0, 1000)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
        .collect();
    let edges = b.ring(&corners, 0);
    let uses = reversed(&b.runs(&edges));
    let face = b.flat(([0, 0, 1], 0), vec![uses], 0);
    let out = interfere(&b.arr, default_remaining(), &Exec::serial(), &mut meter()).unwrap();
    assert_eq!(
        split(&b.arr, &out, &mut meter()),
        Err(BuildError::Unbounded { face })
    );
    // The same square with one edge run the wrong way: it no longer starts where the one before
    // it ends.
    let mut uses = b.runs(&edges);
    uses[1].forward = false;
    b.arr.faces[face].loops = vec![uses];
    assert_eq!(
        split(&b.arr, &out, &mut meter()),
        Err(BuildError::BrokenLoop { face, ring: 0 })
    );
}

/// A disc of radius 200 and, on the same plane, a disc of another operand of radius 100 inside it
/// that touches its rim from within, both circles starting at the point they touch. There the
/// four ends of the two circles leave along one tangent two by two, and the tangents give no
/// order: the circles are proven to touch exactly (their centres lie the difference of their
/// radii apart), and the smaller bends more, so the order is by curvature. The disc splits into
/// the small disc and the crescent round it, whose loop runs the rim and then the small circle
/// back; the small disc stays whole.
#[test]
fn circles_that_touch_are_ordered_by_curvature() {
    let k = S as f64;
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let disc = |b: &mut Build, centre: f64, radius: f64, operand: u32| {
        let at = b.vertex(p(700, 500, 0), operand);
        let circle = Circle::new([centre * k, 500.0 * k, 0.0], radius * k, frame, (0.0, 2.0));
        let rim = b.edge(CurveKind::Circle(circle.unwrap()), Some([at, at]), operand);
        let forward = EdgeUse {
            edge: rim,
            forward: true,
        };
        (b.flat(([0, 0, 1], 0), vec![vec![forward]], operand), rim)
    };
    let mut b = Build::default();
    let (big, rim) = disc(&mut b, 500.0, 200.0, 0);
    let (small, inner) = disc(&mut b, 600.0, 100.0, 1);
    let (out, splitting) = b.run();
    let whole = |e: usize| {
        let k = out.splits.iter().position(|s| s.source == e).unwrap();
        Piece::Split(k)
    };
    let (rim, inner) = (whole(rim), whole(inner));
    let use_of = |piece, forward| Use { piece, forward };
    let mut parts: Vec<Vec<Use>> = of_face(&splitting, big)
        .iter()
        .map(|s| s.outer.clone())
        .collect();
    parts.sort_by_key(Vec::len);
    assert_eq!(
        parts,
        vec![
            vec![use_of(inner, true)],
            vec![use_of(rim, true), use_of(inner, false)]
        ]
    );
    assert_eq!(of_face(&splitting, small).len(), 1);
}

/// A square and, on the same plane, a disc of another operand inside it that touches its lower
/// edge, the disc's circle starting at the point of touching, which splits that edge. There the
/// circle leaves along the edge's right half and arrives along its left: the line and the circle
/// are proven to touch exactly (the centre lies the radius from the edge's line), and the circle
/// bends to the left of each, so it lies counter-clockwise of the right half and clockwise of the
/// left. The square splits into the disc and the rest, whose loop runs the square's five pieces
/// and the circle back.
#[test]
fn a_circle_that_touches_a_line_is_ordered_by_curvature() {
    let mut b = Build::default();
    let corners: Vec<usize> = [(0, 0), (1000, 0), (1000, 1000), (0, 1000)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
        .collect();
    let square = b.polygon(&corners, [0, 0, 1], 0, 0);
    let k = S as f64;
    let frame = Frame::new([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let at = b.vertex(p(500, 0, 0), 1);
    let circle = Circle::new([500.0 * k, 200.0 * k, 0.0], 200.0 * k, frame, (0.0, 2.0)).unwrap();
    let rim = b.edge(CurveKind::Circle(circle), Some([at, at]), 1);
    let forward = EdgeUse {
        edge: rim,
        forward: true,
    };
    let disc = b.flat(([0, 0, 1], 0), vec![vec![forward]], 1);
    let (out, splitting) = b.run();
    let circle = Piece::Split(out.splits.iter().position(|s| s.source == rim).unwrap());
    let mut parts: Vec<Vec<Use>> = of_face(&splitting, square)
        .iter()
        .map(|s| s.outer.clone())
        .collect();
    parts.sort_by_key(Vec::len);
    assert_eq!(parts.len(), 2, "{parts:?}");
    assert_eq!(
        parts[0],
        vec![Use {
            piece: circle,
            forward: true
        }]
    );
    assert_eq!(parts[1].len(), 6, "{parts:?}");
    assert!(parts[1].contains(&Use {
        piece: circle,
        forward: false
    }));
    assert_eq!(of_face(&splitting, disc).len(), 1);
}
