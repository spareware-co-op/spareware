//! Attacks on G244's claims (chapter 11.4 step 5): face splitting where its tests decide. Tangents
//! of every half-plane round a vertex, arcs of a circle and an ellipse with the seam inside one;
//! holes given to the smallest face that holds them, beside sections and pieces of a boundary
//! line; edges running either way along a loop; a closed curve far from the origin with a domain
//! away from zero; a side whose direction is not whole; and two sides almost parallel at the
//! frame's size, which only exact arithmetic orders.

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
use geom_bool::interfere::{Arrangement, EdgeIn, EdgeUse, FaceIn, Interfered, VertexIn, interfere};
use geom_bool::merge::default_remaining;
use geom_bool::split::{Piece, SplitFace, Splitting, Use, split};
use geom_curve::{Axes, Circle, CurveKind, Ellipse, Frame, Line};
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
    /// points: turned round where the plane's entry in the table faces the other way (its
    /// normal's first part that is not zero is positive).
    fn polygon(&mut self, corners: &[usize], n: [i128; 3], d: i128, operand: u32) -> usize {
        let edges = self.ring(corners, operand);
        let plane = self.plane(n, d);
        let uses = self.runs(&edges);
        let flipped = n.iter().find(|v| **v != 0).is_some_and(|v| *v < 0);
        let uses = if flipped { reversed(&uses) } else { uses };
        self.face_with(plane, vec![uses], operand)
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

/// A closed edge run backward, as a loop of a face.
fn back(edge: usize) -> EdgeUse {
    EdgeUse {
        edge,
        forward: false,
    }
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

fn of_face(splitting: &Splitting, f: usize) -> Vec<&SplitFace> {
    splitting.faces.iter().filter(|s| s.face == f).collect()
}

/// The pieces of a face's split faces: how many faces have each count of uses in their outer loop.
fn outer_sizes(parts: &[&SplitFace]) -> Vec<usize> {
    let mut sizes: Vec<usize> = parts.iter().map(|s| s.outer.len()).collect();
    sizes.sort_unstable();
    sizes
}

/// A square cut by its two diagonals and by a circle or an ellipse round its centre, all lying in
/// it, each of its own operand: eight faces, four bounded by an arc and two half-diagonals, four by a side of the square,
/// two pieces of diagonals and an arc. The arcs leave each meeting with a diagonal in a tangent of
/// every half-plane, so the order round each vertex needs every case of the tangent test, and the
/// curve's seam lies inside one of the arcs.
#[test]
fn arcs_and_diagonals_split_a_square_into_eight() {
    let k = S as f64;
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let curves = [
        CurveKind::Circle(
            Circle::new([1000.0 * k, 1000.0 * k, 0.0], 500.0 * k, frame, (0.0, 2.0)).unwrap(),
        ),
        CurveKind::Ellipse(
            Ellipse::new(
                [1000.0 * k, 1000.0 * k, 0.0],
                Axes::Frame {
                    frame,
                    a: 600.0 * k,
                    b: 400.0 * k,
                },
                (0.0, 2.0),
            )
            .unwrap(),
        ),
    ];
    for curve in curves {
        let mut b = Build::default();
        let corners: Vec<usize> = [(0, 0), (2000, 0), (2000, 2000), (0, 2000)]
            .iter()
            .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
            .collect();
        let square = b.polygon(&corners, [0, 0, 1], 0, 0);
        // The diagonals are of two operands, so they meet at the centre in a vertex.
        let ends: Vec<usize> = [(0, 0, 1), (2000, 2000, 1), (2000, 0, 3), (0, 2000, 3)]
            .iter()
            .map(|&(x, y, operand)| b.vertex(p(x, y, 0), operand))
            .collect();
        b.line(ends[0], ends[1], 1);
        b.line(ends[2], ends[3], 3);
        b.edge(curve, None, 2);
        let (_, splitting) = b.run();
        assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
        let parts = of_face(&splitting, square);
        assert_eq!(
            outer_sizes(&parts),
            vec![3, 3, 3, 3, 4, 4, 4, 4],
            "{parts:?}"
        );
        assert!(
            parts
                .iter()
                .all(|s| s.holes.is_empty() && s.cuts.is_empty())
        );
    }
}

/// A slab's top with two squares lying in it, a large one and a small one, and a column through
/// the large one: the column's outline is a hole of the large square's face, the smallest that
/// holds it, not of the small square's nor of the slab round them, which has the two squares as
/// its holes.
#[test]
fn a_hole_goes_to_the_smallest_face_holding_it() {
    let mut b = Build::default();
    let slab = b.cuboid([0, 0, 0], [1000, 1000, 200], 0);
    for (lo, hi) in [(100, 600), (700, 900)] {
        let c: Vec<usize> = [(lo, lo), (hi, lo), (hi, hi), (lo, hi)]
            .iter()
            .map(|&(x, y)| b.vertex(p(x, y, 200), 1))
            .collect();
        b.ring(&c, 1);
    }
    b.cuboid([300, 300, -100], [400, 400, 400], 2);
    let (out, splitting) = b.run();
    let parts = of_face(&splitting, slab.faces[5]);
    assert_eq!(parts.len(), 4, "{parts:?}");
    let area = |s: &SplitFace| twice_area(&b, &out, &s.outer, [0, 1]);
    let s = i128::from(S);
    let square = |side: i128| 2 * side * side * s * s;
    let holes_of = |side: i128| {
        let face = parts.iter().find(|f| area(f) == square(side)).unwrap();
        face.holes.len()
    };
    assert_eq!(holes_of(1000), 2, "the slab round the squares");
    assert_eq!(holes_of(500), 1, "the large square, round the column");
    assert_eq!(holes_of(200), 0, "the small square");
    assert_eq!(holes_of(100), 0, "the column's section");
}

/// A square face whose boundary edges run either way along its loop, crossed by a plate: every
/// choice of directions splits it the same, into the two sides of the plate and the plate's strip,
/// with the same areas.
#[test]
fn a_face_splits_the_same_whichever_way_its_edges_run() {
    for reversed in 0..16_u32 {
        let mut b = Build::default();
        let c: Vec<usize> = [(0, 0), (1000, 0), (1000, 1000), (0, 1000)]
            .iter()
            .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
            .collect();
        let edges: Vec<usize> = (0..4)
            .map(|k| {
                let (u, v) = (c[k], c[(k + 1) % 4]);
                if reversed >> k & 1 == 1 {
                    b.line(v, u, 0)
                } else {
                    b.line(u, v, 0)
                }
            })
            .collect();
        let plane = b.plane([0, 0, 1], 0);
        let face = b.face(plane, &[edges], 0);
        b.cuboid([400, -100, -100], [450, 1100, 100], 1);
        let (out, splitting) = b.run();
        let parts = of_face(&splitting, face);
        assert_eq!(parts.len(), 3, "edges reversed {reversed:04b}: {parts:?}");
        let mut areas: Vec<i128> = parts
            .iter()
            .map(|s| twice_face_area(&b, &out, s, [0, 1]))
            .collect();
        areas.sort_unstable();
        let s = i128::from(S);
        assert_eq!(
            areas,
            vec![
                2 * 50 * 1000 * s * s,
                2 * 400 * 1000 * s * s,
                2 * 550 * 1000 * s * s
            ],
            "edges reversed {reversed:04b}"
        );
    }
}

/// A triangle cut by an upright plate into a large part, the plate's strip and a small part, with
/// a column through the large part, run either way along the hypotenuse. The ray placing the
/// column's outline crosses the hypotenuse's line beyond the small part's piece of it and the
/// plate's section: the column is a hole of the large part only.
#[test]
fn a_hole_beside_a_section_goes_to_its_own_part() {
    for flip in [false, true] {
        let mut b = Build::default();
        let c: Vec<usize> = [(0, 0), (2000, 0), (0, 2000)]
            .iter()
            .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
            .collect();
        let hypotenuse = if flip {
            b.line(c[2], c[1], 0)
        } else {
            b.line(c[1], c[2], 0)
        };
        let edges = vec![b.line(c[0], c[1], 0), hypotenuse, b.line(c[2], c[0], 0)];
        let plane = b.plane([0, 0, 1], 0);
        let face = b.face(plane, &[edges], 0);
        b.cuboid([1200, -100, -100], [1250, 2100, 100], 1);
        b.cuboid([200, 1300, -100], [300, 1400, 100], 2);
        let (out, splitting) = b.run();
        let parts = of_face(&splitting, face);
        assert_eq!(parts.len(), 4, "{parts:?}");
        let with_hole: Vec<&&SplitFace> = parts.iter().filter(|s| !s.holes.is_empty()).collect();
        assert_eq!(with_hole.len(), 1, "{parts:?}");
        // The large part: left of the plate, x from 0 to 1,200, under the hypotenuse.
        let s = i128::from(S);
        assert_eq!(
            twice_area(&b, &out, &with_hole[0].outer, [0, 1]),
            (2000 * 2000 - 800 * 800) * s * s,
            "flipped {flip}"
        );
    }
}

/// A face with a round hole far from the origin, the hole's circle running over the parameters 10
/// to 12: the hole is the face's, found from a point on the circle at its middle parameter.
#[test]
fn a_round_hole_far_from_the_origin_stays_the_face_s() {
    let mut b = Build::default();
    let corners: Vec<usize> = [(1000, 1000), (2000, 1000), (2000, 2000), (1000, 2000)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
        .collect();
    let outer = b.ring(&corners, 0);
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let k = S as f64;
    let circle = Circle::new(
        [1500.0 * k, 1500.0 * k, 0.0],
        200.0 * k,
        frame,
        (10.0, 12.0),
    )
    .unwrap();
    let hole = b.edge(CurveKind::Circle(circle), None, 0);
    let plane = b.plane([0, 0, 1], 0);
    // The circle runs counter-clockwise seen from +z: the face is on its right.
    let outer = b.runs(&outer);
    let face = b.face_with(plane, vec![outer, vec![back(hole)]], 0);
    let (_, splitting) = b.run();
    let parts = of_face(&splitting, face);
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].holes.len(), 1, "{parts:?}");
}

/// A wedge face whose side from its apex is a line of direction (0.5, 1.5), not whole, with a cut
/// lying in it from the apex between its two sides: the order round the apex, side, cut, side,
/// keeps the cut inside the wedge's one loop, run both ways.
#[test]
fn a_side_of_direction_not_whole_is_ordered_by_its_enclosure() {
    let mut b = Build::default();
    let apex = b.vertex(P3::new(0, 0, 0).unwrap(), 0);
    let far = b.vertex(P3::new(1000, 3000, 0).unwrap(), 0);
    let back = b.vertex(P3::new(-500, 3000, 0).unwrap(), 0);
    let slanted = Line::new([0.0; 3], [0.5, 1.5, 0.0], (0.0, 2000.0)).unwrap();
    let side = b.edge(CurveKind::Line(slanted), Some([apex, far]), 0);
    let top = b.line(far, back, 0);
    let other = b.line(back, apex, 0);
    let plane = b.plane([0, 0, 1], 0);
    let wedge = b.face(plane, &[vec![side, top, other]], 0);
    let start = b.vertex(P3::new(0, 0, 0).unwrap(), 1);
    let end = b.vertex(P3::new(200, 1200, 0).unwrap(), 1);
    b.line(start, end, 1);
    let (_, splitting) = b.run();
    let parts = of_face(&splitting, wedge);
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].outer.len(), 5, "{parts:?}");
}

/// A face at the frame's size whose two sides leave a corner along tangents whose cross product
/// is 1 against terms of 2^80: only exact whole-number arithmetic orders them.
#[test]
fn two_sides_almost_parallel_at_the_frame_s_size_are_ordered_exactly() {
    let n: i64 = 1 << 40;
    let mut b = Build::default();
    let c: Vec<usize> = [(0, 0), (n - 2, n - 3), (n - 1, 0), (n - 1, n - 2)]
        .iter()
        .map(|&(x, y)| b.vertex(P3::new(x, y, 0).unwrap(), 0))
        .collect();
    let face = b.polygon(&c, [0, 0, 1], 0, 0);
    let (_, splitting) = b.run();
    let parts = of_face(&splitting, face);
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].outer.len(), 4);
}

/// The same triangle, plate and column moved to negative coordinates, with the column low down,
/// where the ray placing its outline crosses the plate's section: each part's loop is closed across
/// the section by the straight chord between the section's two vertices, from (−1,800, −1,000) to
/// (−1,800, −200), so the column is a hole of the large part only.
#[test]
fn a_hole_whose_ray_crosses_a_section_goes_to_its_own_part() {
    let mut b = Build::default();
    let c: Vec<usize> = [(-3000, -1000), (-1000, -1000), (-3000, 1000)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
        .collect();
    let face = b.polygon(&c, [0, 0, 1], 0, 0);
    b.cuboid([-1800, -1100, -100], [-1750, 1100, 100], 1);
    b.cuboid([-2800, -700, -100], [-2700, -600, 100], 2);
    let (out, splitting) = b.run();
    let parts = of_face(&splitting, face);
    assert_eq!(parts.len(), 4, "{parts:?}");
    let with_hole: Vec<&&SplitFace> = parts.iter().filter(|s| !s.holes.is_empty()).collect();
    assert_eq!(with_hole.len(), 1, "{parts:?}");
    let s = i128::from(S);
    assert_eq!(
        twice_area(&b, &out, &with_hole[0].outer, [0, 1]),
        (2000 * 2000 - 800 * 800) * s * s
    );
}

/// A face with a round hole, cut by a plate whose side passes through the hole's top and bottom
/// points, where the circle leaves each vertex along the picture's first axis, its tangent's
/// enclosure holding that axis's line: the order round those vertices is counted from the next ray
/// direction, and the face splits into the part either side of the plate and the plate's strip
/// above and below the hole, whichever way the circle runs.
#[test]
fn a_plate_through_a_round_hole_s_top_and_bottom_splits_the_face() {
    for normal in [[0.0, 0.0, 1.0], [0.0, 0.0, -1.0]] {
        let mut b = Build::default();
        let corners: Vec<usize> = [(0, 0), (1000, 0), (1000, 1000), (0, 1000)]
            .iter()
            .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
            .collect();
        let outer = b.ring(&corners, 0);
        let frame = Frame::new([1.0, 0.0, 0.0], normal).unwrap();
        let k = S as f64;
        let circle =
            Circle::new([500.0 * k, 500.0 * k, 0.0], 200.0 * k, frame, (0.0, 2.0)).unwrap();
        let hole = b.edge(CurveKind::Circle(circle), None, 0);
        let plane = b.plane([0, 0, 1], 0);
        // Counter-clockwise about +z the circle has the face on its right; about −z on its left.
        let outer = b.runs(&outer);
        let hole = EdgeUse {
            edge: hole,
            forward: normal[2] < 0.0,
        };
        let face = b.face_with(plane, vec![outer, vec![hole]], 0);
        b.cuboid([500, -100, -100], [510, 1100, 100], 1);
        let (_, splitting) = b.run();
        let parts = of_face(&splitting, face);
        assert_eq!(
            outer_sizes(&parts),
            vec![4, 4, 6, 6],
            "{normal:?}: {parts:?}"
        );
        assert!(parts.iter().all(|s| s.holes.is_empty()), "{parts:?}");
    }
}

/// A face with a round hole, and a line lying in it from the hole's top point up to the face's
/// edge, its direction (0, 0.5) not whole: at the top point the circle leaves along the picture's
/// first axis both ways and the line along its second, each enclosure holding its axis's line, so
/// the order there is counted from the diagonal. The face stays one, its outer loop running round
/// the hole and up and down the line, whichever way the circle runs.
#[test]
fn a_hole_and_a_line_along_the_axes_are_ordered_from_the_diagonal() {
    for normal in [[0.0, 0.0, 1.0], [0.0, 0.0, -1.0]] {
        let mut b = Build::default();
        let corners: Vec<usize> = [(0, 0), (1000, 0), (1000, 1000), (0, 1000)]
            .iter()
            .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
            .collect();
        let outer = b.ring(&corners, 0);
        let frame = Frame::new([1.0, 0.0, 0.0], normal).unwrap();
        let k = S as f64;
        let circle =
            Circle::new([500.0 * k, 400.0 * k, 0.0], 200.0 * k, frame, (0.0, 2.0)).unwrap();
        let hole = b.edge(CurveKind::Circle(circle), None, 0);
        let plane = b.plane([0, 0, 1], 0);
        // Counter-clockwise about +z the circle has the face on its right; about −z on its left.
        let outer = b.runs(&outer);
        let hole = EdgeUse {
            edge: hole,
            forward: normal[2] < 0.0,
        };
        let face = b.face_with(plane, vec![outer, vec![hole]], 0);
        let (start, end) = (b.vertex(p(500, 600, 0), 1), b.vertex(p(500, 1000, 0), 1));
        let line = Line::new(
            [500.0 * k, 600.0 * k, 0.0],
            [0.0, 0.5, 0.0],
            (0.0, 800.0 * k),
        )
        .unwrap();
        b.edge(CurveKind::Line(line), Some([start, end]), 1);
        let (_, splitting) = b.run();
        let parts = of_face(&splitting, face);
        assert_eq!(outer_sizes(&parts), vec![8], "{normal:?}: {parts:?}");
        assert!(parts[0].holes.is_empty(), "{parts:?}");
    }
}

/// A triangle cut by a slanting plate along x + y = −2,400 (or, on a wall, x + z = −2,400), with a
/// column through the corner below the section and one through the strip above it: the ray placing
/// each column's outline runs into the straight chord between the section's two vertices, or
/// beside it, at negative coordinates off both axes, so each column is a hole of its own part.
#[test]
fn holes_beside_a_slanting_section_go_to_their_own_parts() {
    // The picture's second coordinate is y on the floor and z on the wall.
    for wall in [false, true] {
        let at = |u: i64, v: i64, w: i64| if wall { p(u, w, v) } else { p(u, v, w) };
        // Swapping the second and third coordinates mirrors every loop: on the wall each normal
        // is turned round with them, so that the loops still run counter-clockwise about it.
        let normal = |nu: i128, nv: i128, nw: i128| {
            if wall { [-nu, -nw, -nv] } else { [nu, nv, nw] }
        };
        let sense: i128 = if wall { -1 } else { 1 };
        let mut b = Build::default();
        let c: Vec<usize> = [(-3000, -1000), (-1000, -1000), (-3000, 1000)]
            .iter()
            .map(|&(u, v)| b.vertex(at(u, v, 0), 0))
            .collect();
        let face = b.polygon(&c, normal(0, 0, 1), 0, 0);
        let plate: Vec<usize> = [
            (-1300, -1100, -100),
            (-3100, 700, -100),
            (-3100, 700, 100),
            (-1300, -1100, 100),
        ]
        .iter()
        .map(|&(u, v, w)| b.vertex(at(u, v, w), 1))
        .collect();
        b.polygon(&plate, normal(1, 1, 0), sense * 2400 * i128::from(S), 1);
        // One column in the corner below the section, one in the strip above it.
        for (lo, hi, operand) in [
            ((-2800, -900), (-2700, -800), 2),
            ((-1900, -400), (-1800, -300), 3),
        ] {
            let (l, h) = (at(lo.0, lo.1, -100), at(hi.0, hi.1, 100));
            let unit = |q: P3| [q.x.get() / S, q.y.get() / S, q.z.get() / S];
            b.cuboid(unit(l), unit(h), operand);
        }
        let (_, splitting) = b.run();
        let parts = of_face(&splitting, face);
        // The corner, the strip and the columns' sections.
        assert_eq!(
            outer_sizes(&parts),
            vec![3, 4, 4, 4],
            "wall {wall}: {parts:?}"
        );
        let holding: Vec<&&SplitFace> = parts.iter().filter(|s| !s.holes.is_empty()).collect();
        assert_eq!(
            outer_sizes(&holding.iter().map(|s| **s).collect::<Vec<_>>()),
            vec![3, 4],
            "wall {wall}: {parts:?}"
        );
        assert!(holding.iter().all(|s| s.holes.len() == 1), "{parts:?}");
    }
}

/// A square lying in a face with its diagonal, of another operand, from corner to corner: one
/// group of pieces, whose two triangles' loops and the hole round it start at different pieces.
/// The hole is the face's, never placed in a triangle of its own group, though its point, a corner,
/// is a vertex of both.
#[test]
fn a_hole_is_never_placed_in_a_face_of_its_own_group() {
    let mut b = Build::default();
    let corners: Vec<usize> = [(0, 0), (1000, 0), (1000, 1000), (0, 1000)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
        .collect();
    let face = b.polygon(&corners, [0, 0, 1], 0, 0);
    let square: Vec<usize> = [(200, 200), (600, 200), (600, 600), (200, 600)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 0), 1))
        .collect();
    b.ring(&square, 1);
    let (from, to) = (b.vertex(p(200, 200, 0), 2), b.vertex(p(600, 600, 0), 2));
    b.line(from, to, 2);
    let (out, splitting) = b.run();
    let parts = of_face(&splitting, face);
    assert_eq!(outer_sizes(&parts), vec![3, 3, 4], "{parts:?}");
    let rest = parts.iter().find(|s| s.outer.len() == 4).unwrap();
    assert_eq!(rest.holes.len(), 1, "{rest:?}");
    let s = i128::from(S);
    assert_eq!(
        twice_face_area(&b, &out, rest, [0, 1]),
        2 * (1000 * 1000 - 400 * 400) * s * s
    );
}

/// A face whose right side is the line x = k·m (m the least double, 2^−1074), with a hole of its
/// own whose corner is the origin. The ray placing the hole runs along the picture's first axis and
/// meets the side k·m ahead; every other direction runs along the side or meets it within its
/// rounding of the point. At k = 4 the crossing's position along the ray is enclosed from exactly 0:
/// not proven ahead, so the direction is refused too and the hole is Unclassified; at k = 3 the
/// enclosure holds 0; from k = 5 it is proven ahead and the hole is the face's.
#[test]
fn a_crossing_enclosed_from_exactly_zero_is_not_ahead() {
    let tiny = |k: u64| f64::from_bits(k);
    for (k, placed) in [(3, false), (4, false), (5, true)] {
        let mut b = Build::default();
        let l = 1000.0 * S as f64;
        let c: Vec<usize> = [(-1000, -1000), (0, -1000), (0, 1000), (-1000, 1000)]
            .iter()
            .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
            .collect();
        let bottom = b.line(c[0], c[1], 0);
        let side = Line::new([tiny(k), -l, 0.0], [0.0, 2.0 * l, 0.0], (0.0, 1.0)).unwrap();
        let right = b.edge(CurveKind::Line(side), Some([c[1], c[2]]), 0);
        let top = b.line(c[2], c[3], 0);
        let left = b.line(c[3], c[0], 0);
        let h: Vec<usize> = [(0, 0), (0, -500), (-500, -500), (-500, 0)]
            .iter()
            .map(|&(x, y)| b.vertex(p(x, y, 0), 0))
            .collect();
        let hole = b.ring(&h, 0);
        let plane = b.plane([0, 0, 1], 0);
        let face = b.face(plane, &[vec![bottom, right, top, left], hole], 0);
        let out = interfere(&b.arr, default_remaining(), &Exec::serial(), &mut meter()).unwrap();
        match split(&b.arr, &out, &mut meter()) {
            Ok(splitting) if placed => {
                let parts = of_face(&splitting, face);
                assert_eq!(parts.len(), 1, "k = {k}: {parts:?}");
                assert_eq!(parts[0].holes.len(), 1, "k = {k}: {parts:?}");
            }
            Err(geom_bool::interfere::BuildError::Unclassified { face: f, at })
                if !placed && f == face && at == p(0, 0, 0) => {}
            other => panic!("k = {k}: {other:?}"),
        }
    }
}
