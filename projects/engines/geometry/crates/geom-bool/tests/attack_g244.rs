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
use geom_bool::interfere::{Arrangement, EdgeIn, FaceIn, Interfered, VertexIn, interfere};
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

    fn polygon(&mut self, corners: &[usize], n: [i128; 3], d: i128, operand: u32) -> usize {
        let edges = self.ring(corners, operand);
        let plane = self.plane(n, d);
        self.face(plane, vec![edges], operand)
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
                faces[2 * axis + side] = self.face(plane, vec![edges], operand);
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
        Piece::Section(s) => [out.sections[s].from.vertex, out.sections[s].to.vertex],
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
        let face = b.face(plane, vec![edges], 0);
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
        let face = b.face(plane, vec![edges], 0);
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
    let face = b.face(plane, vec![outer, vec![hole]], 0);
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
    let wedge = b.face(plane, vec![vec![side, top, other]], 0);
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
