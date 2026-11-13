//! Face splitting on cylinders and cones (G244, chapter 11.4 step 5): each face seen from a point
//! inside its carrier, so that a cylinder's whole side has no seam and a cone's apex is an
//! ordinary point; faces of two operands on one carrier split each other, as two on one plane do;
//! and a face that something not yet met with it may cross waits.

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
use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_num::P3;
use geom_num::plane::Plane;
use geom_run::{Budget, Exec, WorkMeter};
use geom_surface::{Cone, Cylinder};
use geom_topo::SurfaceKind;

/// The tests' unit, in steps: a millimetre on a grid of a micrometre.
const S: i64 = 1000;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 28))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x * S, y * S, z * S).unwrap()
}

fn coords(q: P3) -> [f64; 3] {
    [q.x.get() as f64, q.y.get() as f64, q.z.get() as f64]
}

/// The unit vector along coordinate `k`, or against it.
fn unit(k: usize, sign: f64) -> [f64; 3] {
    let mut v = [0.0; 3];
    v[k] = sign;
    v
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

    /// A whole circle of radius `r` about `centre`, counter-clockwise about `frame`'s normal.
    fn circle(&mut self, centre: P3, r: i64, frame: Frame<3>, operand: u32) -> usize {
        let circle = Circle::new(coords(centre), (r * S) as f64, frame, (0.0, 2.0)).unwrap();
        self.edge(CurveKind::Circle(circle), None, operand)
    }

    fn surface(&mut self, surface: SurfaceKind) -> CarrierIx {
        CarrierIx::Surface(self.arr.carriers.add_surface(surface).unwrap())
    }

    fn face(&mut self, surface: CarrierIx, loops: Vec<Vec<usize>>, operand: u32) -> usize {
        self.arr.faces.push(FaceIn {
            surface,
            loops,
            operand,
        });
        self.arr.faces.len() - 1
    }

    /// A square on the plane z = 0, from (`lo`, `lo`) to (`hi`, `hi`).
    fn square(&mut self, lo: i64, hi: i64, operand: u32) -> usize {
        let corners: Vec<usize> = [(lo, lo), (hi, lo), (hi, hi), (lo, hi)]
            .iter()
            .map(|&(x, y)| self.vertex(p(x, y, 0), operand))
            .collect();
        let edges = (0..4)
            .map(|k| self.line(corners[k], corners[(k + 1) % 4], operand))
            .collect();
        let plane = CarrierIx::Plane(
            self.arr
                .carriers
                .add_plane(&Plane::new([0, 0, 1], 0).unwrap())
                .unwrap(),
        );
        self.face(plane, vec![edges], operand)
    }

    /// The side of a cylinder of radius `r` about the axis through `base` along coordinate `k`
    /// (against it for a negative `sign`), from the height `from` to the height `to` along the
    /// axis, each in the tests' unit: the face, its circle at `from` and its circle at `to`.
    fn tube(
        &mut self,
        (base, k, sign): (P3, usize, f64),
        r: i64,
        (from, to): (i64, i64),
        operand: u32,
    ) -> (usize, usize, usize) {
        let frame = Frame::new(unit((k + 1) % 3, 1.0), unit(k, sign)).unwrap();
        let along = |h: i64| {
            let mut c = [base.x.get(), base.y.get(), base.z.get()];
            c[k] += (sign as i64) * h * S;
            P3::new(c[0], c[1], c[2]).unwrap()
        };
        let first = self.circle(along(from), r, frame, operand);
        let second = self.circle(along(to), r, frame, operand);
        let cylinder = Cylinder::new(
            coords(base),
            (r * S) as f64,
            frame,
            ((0.0, 2.0), ((from * S) as f64, (to * S) as f64)),
        )
        .unwrap();
        let ix = self.surface(SurfaceKind::Cylinder(cylinder));
        let face = self.face(ix, vec![vec![first], vec![second]], operand);
        (face, first, second)
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

fn of_face(splitting: &Splitting, f: usize) -> Vec<&SplitFace> {
    splitting.faces.iter().filter(|s| s.face == f).collect()
}

/// The split edge of a whole closed edge.
fn whole_piece(out: &Interfered, edge: usize) -> Piece {
    let found: Vec<usize> = (0..out.splits.len())
        .filter(|k| out.splits[*k].source == edge)
        .collect();
    assert_eq!(found.len(), 1, "edge {edge} is one piece");
    Piece::Split(found[0])
}

fn run(piece: Piece, forward: bool) -> Use {
    Use { piece, forward }
}

/// A cylinder's whole side, between its two end circles, is one face with no seam: whichever
/// coordinate its axis runs along, and either way, its outer loop is the circle at the far end
/// of its axis and the other circle its hole, each run with the face on its left seen from
/// outside the cylinder. The circles run counter-clockwise about the axis, so the far one is run
/// backwards and the near one forwards.
#[test]
fn a_cylinder_s_side_is_one_face_with_no_seam() {
    for k in 0..3 {
        for sign in [1.0, -1.0] {
            let mut b = Build::default();
            let (side, near, far) = b.tube((p(10, -20, 30), k, sign), 50, (0, 100), 0);
            let (out, splitting) = b.run();
            assert!(splitting.waiting.is_empty(), "axis {k} {sign}");
            let parts = of_face(&splitting, side);
            assert_eq!(parts.len(), 1, "axis {k} {sign}: {parts:?}");
            assert_eq!(
                parts[0].outer,
                vec![run(whole_piece(&out, far), false)],
                "axis {k} {sign}"
            );
            assert_eq!(
                parts[0].holes,
                vec![vec![run(whole_piece(&out, near), true)]],
                "axis {k} {sign}"
            );
            assert!(
                parts[0].cuts.is_empty() && parts[0].inside.is_empty(),
                "{:?} {:?}",
                parts[0].cuts,
                parts[0].inside
            );
        }
    }
}

/// The same side given with its loops the other way round is the same face: the outer loop is
/// found in the picture, not taken from the order of the loops.
#[test]
fn a_cylinder_s_outer_loop_is_found_not_given() {
    let mut b = Build::default();
    let (side, near, far) = b.tube((p(0, 0, 0), 2, 1.0), 50, (0, 100), 0);
    b.arr.faces[side].loops.reverse();
    let (out, splitting) = b.run();
    let parts = of_face(&splitting, side);
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].outer, vec![run(whole_piece(&out, far), false)]);
    assert_eq!(
        parts[0].holes,
        vec![vec![run(whole_piece(&out, near), true)]]
    );
}

/// Vertices of another operand on a cylinder's side: one inside the face is In it and is given to
/// the split face; one on the carrier beyond the face's end, two on its boundary and one off the
/// carrier are not. Each rim with a vertex on it is one piece from that vertex round to it again,
/// at the circle's own seam or away from it, and the side is still one face.
#[test]
fn a_vertex_on_a_cylinder_s_side_is_placed() {
    let mut b = Build::default();
    let (side, near, far) = b.tube((p(0, 0, 0), 2, 1.0), 50, (0, 100), 0);
    let inside = b.vertex(p(0, 50, 40), 1);
    let beyond = b.vertex(p(0, 50, 140), 1);
    let on_seam = b.vertex(p(50, 0, 100), 1);
    let on_rim = b.vertex(p(0, 50, 0), 1);
    let off = b.vertex(p(0, 60, 40), 1);
    let (out, splitting) = b.run();
    assert_eq!(out.faces[side].in_vertices, vec![inside]);
    assert!(
        out.faces[side].pending.vertices.is_empty(),
        "{:?}",
        out.faces[side].pending.vertices
    );
    assert_eq!(out.paves[near].len(), 1, "{:?}", out.paves[near]);
    assert_eq!(out.paves[far].len(), 1, "{:?}", out.paves[far]);
    let parts = of_face(&splitting, side);
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].inside, vec![inside]);
    assert_eq!(parts[0].outer, vec![run(whole_piece(&out, far), false)]);
    assert_eq!(
        parts[0].holes,
        vec![vec![run(whole_piece(&out, near), true)]]
    );
    for v in [beyond, on_seam, on_rim, off] {
        assert!(!out.faces[side].in_vertices.contains(&v), "vertex {v}");
    }
}

/// A plane across a cylinder's side, its edges far away: only the two faces' boxes meet. The face
/// stages have no section for the pair yet, so neither face is split: both wait.
#[test]
fn a_cylinder_and_a_plane_across_it_wait() {
    let mut b = Build::default();
    let (side, _, _) = b.tube((p(0, 0, -50), 2, 1.0), 50, (0, 100), 0);
    let sheet = b.square(-1000, 1000, 1);
    let (out, splitting) = b.run();
    assert_eq!(out.faces[side].pending.faces, vec![sheet]);
    assert!(
        out.faces[side].pending.edges.is_empty(),
        "{:?}",
        out.faces[side].pending.edges
    );
    assert_eq!(out.faces[sheet].pending.faces, vec![side]);
    assert_eq!(splitting.waiting, vec![side, sheet]);
    assert!(splitting.faces.is_empty(), "{:?}", splitting.faces);
}

/// An edge of another operand through a cylinder's side, bounding no face: where it crosses the
/// side is a new vertex, In the side and a pave on the edge, and the side is one face that holds
/// it. A spoke beyond the side's end crosses the carrier outside the face: no vertex.
#[test]
fn an_edge_through_a_cylinder_s_side_is_a_vertex_in_it() {
    let mut b = Build::default();
    let (side, _, _) = b.tube((p(0, 0, 0), 2, 1.0), 50, (0, 100), 0);
    let (from, to) = (b.vertex(p(0, 0, 50), 1), b.vertex(p(100, 0, 50), 1));
    let spoke = b.line(from, to, 1);
    let (from, to) = (b.vertex(p(0, 0, 150), 1), b.vertex(p(0, 100, 150), 1));
    let beyond = b.line(from, to, 1);
    let (out, splitting) = b.run();
    let info = &out.faces[side];
    assert!(
        info.pending.edges.is_empty() && info.pending.faces.is_empty(),
        "{:?}",
        info.pending
    );
    assert_eq!(info.in_vertices.len(), 1, "{:?}", info.in_vertices);
    let crossing = info.in_vertices[0];
    assert!(crossing >= b.arr.vertices.len(), "a new vertex: {crossing}");
    match &out.vertices[crossing] {
        PointGeom::Certified { at, .. } => assert_eq!(*at, p(50, 0, 50)),
        other => panic!("{other:?}"),
    }
    assert!(out.paves[spoke].iter().any(|pave| pave.vertex == crossing));
    assert!(
        out.paves[beyond]
            .iter()
            .all(|pave| pave.vertex < b.arr.vertices.len()),
        "{:?}",
        out.paves[beyond]
    );
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    let parts = of_face(&splitting, side);
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].inside, vec![crossing]);
}

/// A window of another operand on a cylinder's side: half the way round, across the point where
/// the cylinder's own parameter starts again, between two rulings and two arcs. Its carrier is the
/// side's, found exactly, so its edges are In the side, which splits into the window and the rest:
/// the rest's outer loop is the far rim and its holes the near rim and the window, with no seam
/// met. A short ruling of a third operand, on the other side, is a cut of the rest.
#[test]
fn a_window_across_a_cylinder_s_seam_is_cut_out() {
    let mut b = Build::default();
    let (side, near, far) = b.tube((p(0, 0, 0), 2, 1.0), 50, (0, 100), 0);
    // The arcs run from (0, -50) through (50, 0), where the side's parameter starts, to (0, 50).
    let frame = Frame::new([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let corners = [
        b.vertex(p(0, -50, 30), 1),
        b.vertex(p(0, 50, 30), 1),
        b.vertex(p(0, 50, 70), 1),
        b.vertex(p(0, -50, 70), 1),
    ];
    let arc = |b: &mut Build, z: i64, ends: [usize; 2]| {
        let circle = Circle::new(coords(p(0, 0, z)), (50 * S) as f64, frame, (0.0, 1.0)).unwrap();
        b.edge(CurveKind::Circle(circle), Some(ends), 1)
    };
    let low = arc(&mut b, 30, [corners[0], corners[1]]);
    let up = b.line(corners[1], corners[2], 1);
    let high = arc(&mut b, 70, [corners[3], corners[2]]);
    let down = b.line(corners[3], corners[0], 1);
    let carrier = Cylinder::new(
        coords(p(0, 0, 0)),
        (50 * S) as f64,
        Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
        ((0.0, 2.0), (0.0, (100 * S) as f64)),
    )
    .unwrap();
    let ix = b.surface(SurfaceKind::Cylinder(carrier));
    let window = b.face(ix, vec![vec![low, up, high, down]], 1);
    let (from, to) = (b.vertex(p(-50, 0, 30), 2), b.vertex(p(-50, 0, 70), 2));
    let short = b.line(from, to, 2);
    let (out, splitting) = b.run();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    assert_eq!(out.faces[side].same_carrier, vec![window]);
    assert_eq!(out.faces[side].in_blocks.len(), 5, "{:?}", out.faces[side]);
    let source = |u: &Use| match u.piece {
        Piece::Split(k) => out.splits[k].source,
        Piece::Section(k) => panic!("a section, {k}"),
    };
    let window_edges = [low, up, high, down];
    // The window, a face of its own and of the side, by the same four pieces.
    for f in [window, side] {
        let parts = of_face(&splitting, f);
        let cut_out: Vec<_> = parts.iter().filter(|s| s.outer.len() == 4).collect();
        assert_eq!(cut_out.len(), 1, "{f}: {parts:?}");
        let mut sources: Vec<usize> = cut_out[0].outer.iter().map(source).collect();
        sources.sort_unstable();
        assert_eq!(sources, window_edges.to_vec(), "{f}: {parts:?}");
        assert!(cut_out[0].holes.is_empty() && cut_out[0].cuts.is_empty());
    }
    assert_eq!(of_face(&splitting, window).len(), 1);
    // The rest of the side: the far rim round it, the near rim and the window its holes, and the
    // short ruling a cut.
    let parts = of_face(&splitting, side);
    assert_eq!(parts.len(), 2, "{parts:?}");
    let rest: Vec<_> = parts.iter().filter(|s| s.outer.len() == 1).collect();
    assert_eq!(rest.len(), 1, "{parts:?}");
    assert_eq!(rest[0].outer, vec![run(whole_piece(&out, far), false)]);
    assert_eq!(rest[0].holes.len(), 2, "{parts:?}");
    assert!(
        rest[0]
            .holes
            .contains(&vec![run(whole_piece(&out, near), true)])
    );
    let hole = rest[0].holes.iter().find(|h| h.len() == 4).unwrap();
    let mut sources: Vec<usize> = hole.iter().map(source).collect();
    sources.sort_unstable();
    assert_eq!(sources, window_edges.to_vec());
    assert_eq!(rest[0].cuts.len(), 1, "{parts:?}");
    assert!(rest[0].cuts[0].iter().all(|u| source(u) == short));
}

/// Two cones of one shape about different points of one axis are not known to be one carrier
/// (only cones with the same stored data are): both wait.
#[test]
fn cones_about_different_centres_wait() {
    let mut b = Build::default();
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let mut faces = Vec::new();
    for (operand, apex) in [(0, 0), (1, 10)] {
        let rim = b.circle(p(0, 0, apex + 50), 50, frame, operand);
        let cone = Cone::new(
            coords(p(0, 0, apex)),
            0.0,
            1.0,
            frame,
            ((0.0, 2.0), (0.0, (50 * S) as f64)),
        )
        .unwrap();
        let ix = b.surface(SurfaceKind::Cone(cone));
        faces.push(b.face(ix, vec![vec![rim]], operand));
    }
    let (out, splitting) = b.run();
    assert!(
        out.faces[faces[0]].same_carrier.is_empty(),
        "{:?}",
        out.faces[faces[0]].same_carrier
    );
    assert_eq!(out.faces[faces[0]].pending.faces, vec![faces[1]]);
    assert_eq!(splitting.waiting, faces);
}

/// Coaxial cylinders of one radius, given about different points of the axis, are one carrier,
/// found exactly. Where their sides overlap, each side is split along the other's end circle
/// inside it: the overlap is a face of each, bounded by the same two pieces.
#[test]
fn coaxial_cylinders_of_one_radius_split_each_other() {
    let mut b = Build::default();
    let (lower, foot, lid) = b.tube((p(0, 0, 0), 2, 1.0), 50, (0, 100), 0);
    let (upper, joint, top) = b.tube((p(0, 0, 50), 2, 1.0), 50, (0, 100), 1);
    let (out, splitting) = b.run();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    assert_eq!(out.faces[lower].same_carrier, vec![upper]);
    assert_eq!(out.faces[upper].same_carrier, vec![lower]);
    let piece = |e: usize| whole_piece(&out, e);
    assert_eq!(out.faces[lower].in_blocks.len(), 1);
    assert_eq!(out.faces[upper].in_blocks.len(), 1);
    // The lower side: the band from 50 to 100 inside its top circle, and the band from 0 to 50.
    let parts = of_face(&splitting, lower);
    assert_eq!(parts.len(), 2, "{parts:?}");
    let band = |outer: usize, hole: usize| {
        (
            vec![run(piece(outer), false)],
            vec![vec![run(piece(hole), true)]],
        )
    };
    let got: Vec<_> = parts
        .iter()
        .map(|s| (s.outer.clone(), s.holes.clone()))
        .collect();
    assert!(got.contains(&band(lid, joint)), "{got:?}");
    assert!(got.contains(&band(joint, foot)), "{got:?}");
    // The upper side: from 100 to 150, and the overlap again, by the same two pieces.
    let parts = of_face(&splitting, upper);
    assert_eq!(parts.len(), 2, "{parts:?}");
    let got: Vec<_> = parts
        .iter()
        .map(|s| (s.outer.clone(), s.holes.clone()))
        .collect();
    assert!(got.contains(&band(top, lid)), "{got:?}");
    assert!(got.contains(&band(lid, joint)), "{got:?}");
}

/// A cylinder inside another, about the same axis but of another radius, is another carrier: the
/// two sides may cross for all the face stages know, so both wait and neither is split.
#[test]
fn coaxial_cylinders_of_two_radii_wait() {
    let mut b = Build::default();
    let (outer, _, _) = b.tube((p(0, 0, 0), 2, 1.0), 50, (0, 100), 0);
    let (inner, _, _) = b.tube((p(0, 0, 0), 2, 1.0), 30, (0, 100), 1);
    let (out, splitting) = b.run();
    assert_eq!(splitting.waiting, vec![outer, inner]);
    assert!(splitting.faces.is_empty(), "{:?}", splitting.faces);
    assert_eq!(out.faces[outer].pending.faces, vec![inner]);
    assert!(
        out.faces[outer].same_carrier.is_empty(),
        "{:?}",
        out.faces[outer].same_carrier
    );
}

/// A cone from its apex to a circle, and on the same cone two quarter sectors of other operands,
/// each two rulings from the apex and an arc: four rulings meet at the apex, an ordinary point of
/// the cone's picture, where they are put in order exactly. The cone splits into the two sectors
/// and the rest, whose hole is the one walk round both sectors through the apex.
#[test]
fn a_cone_is_split_through_its_apex() {
    let mut b = Build::default();
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let cone = |domain| SurfaceKind::Cone(Cone::new([0.0; 3], 0.0, 1.0, frame, domain).unwrap());
    // The whole cone, to the circle at height 100.
    let rim = b.circle(p(0, 0, 100), 100, frame, 0);
    let whole = b.surface(cone(((0.0, 2.0), (0.0, (100 * S) as f64))));
    let body = b.face(whole, vec![vec![rim]], 0);
    // Two sectors to height 40: over the first quarter turn, and over the third.
    let mut sectors = Vec::new();
    for (operand, (ax, ay), (bx, by), turn) in
        [(1, (40, 0), (0, 40), 0.0), (2, (-40, 0), (0, -40), 1.0)]
    {
        let apex = b.vertex(p(0, 0, 0), operand);
        let (first, second) = (
            b.vertex(p(ax, ay, 40), operand),
            b.vertex(p(bx, by, 40), operand),
        );
        let out_ruling = b.line(apex, first, operand);
        let arc = Circle::new(
            coords(p(0, 0, 40)),
            (40 * S) as f64,
            frame,
            (turn, turn + 0.5),
        )
        .unwrap();
        let arc = b.edge(CurveKind::Circle(arc), Some([first, second]), operand);
        let back_ruling = b.line(second, apex, operand);
        let patch = b.surface(cone(((turn, turn + 0.5), (0.0, (40 * S) as f64))));
        sectors.push(b.face(patch, vec![vec![out_ruling, arc, back_ruling]], operand));
    }
    let (out, splitting) = b.run();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    assert_eq!(out.faces[body].same_carrier, sectors);
    // Each sector stays whole: the other's edges lie outside it.
    for &sector in &sectors {
        let parts = of_face(&splitting, sector);
        assert_eq!(parts.len(), 1, "{parts:?}");
        assert_eq!(parts[0].outer.len(), 3);
        // Seen from outside the cone, which opens upwards, the loop as given runs clockwise.
        assert!(parts[0].outer.iter().all(|u| !u.forward), "{parts:?}");
        assert!(parts[0].holes.is_empty(), "{:?}", parts[0].holes);
    }
    // The cone: two sectors of three pieces each, and the rest.
    let parts = of_face(&splitting, body);
    assert_eq!(parts.len(), 3, "{parts:?}");
    let rest: Vec<_> = parts.iter().filter(|s| s.outer.len() == 1).collect();
    assert_eq!(rest.len(), 1, "{parts:?}");
    assert_eq!(rest[0].outer, vec![run(whole_piece(&out, rim), false)]);
    assert_eq!(rest[0].holes.len(), 1, "{parts:?}");
    assert_eq!(rest[0].holes[0].len(), 6, "{parts:?}");
    assert!(rest[0].holes[0].iter().all(|u| u.forward), "{parts:?}");
    let cut: Vec<_> = parts.iter().filter(|s| s.outer.len() == 3).collect();
    assert_eq!(cut.len(), 2, "{parts:?}");
    for sector in cut {
        assert!(sector.outer.iter().all(|u| !u.forward), "{parts:?}");
        assert!(
            sector.holes.is_empty() && sector.cuts.is_empty(),
            "{:?} {:?}",
            sector.holes,
            sector.cuts
        );
    }
}

/// Squares of three operands on one plane, each inside the last, and a vertex of a fourth inside
/// all three: every loop and the vertex go to the innermost face that holds them, whichever order
/// the squares come in. The holder is found by the ray test alone, loop inside loop.
#[test]
fn nested_loops_go_to_the_innermost_face() {
    for inner_first in [false, true] {
        let mut b = Build::default();
        let sizes = [(0, 1000), (200, 800), (400, 600)];
        let mut faces = [0; 3];
        let order: Vec<usize> = if inner_first {
            vec![2, 1, 0]
        } else {
            vec![0, 1, 2]
        };
        for &k in &order {
            faces[k] = b.square(sizes[k].0, sizes[k].1, k as u32);
        }
        let lone = b.vertex(p(500, 500, 0), 3);
        let (_, splitting) = b.run();
        assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
        // The floor: its ring round the middle square, the middle square's ring round the
        // small one, and the small square with the vertex.
        let parts = of_face(&splitting, faces[0]);
        assert_eq!(parts.len(), 3, "{inner_first}: {parts:?}");
        let with_vertex: Vec<_> = parts.iter().filter(|s| s.inside == vec![lone]).collect();
        assert_eq!(with_vertex.len(), 1, "{inner_first}: {parts:?}");
        assert!(with_vertex[0].holes.is_empty(), "{inner_first}: {parts:?}");
        let ringed: Vec<_> = parts.iter().filter(|s| s.holes.len() == 1).collect();
        assert_eq!(ringed.len(), 2, "{inner_first}: {parts:?}");
        assert!(ringed.iter().all(|s| s.inside.is_empty()));
        // No two of the three share an outer loop, and each hole is another's outer loop run back.
        for ring in &ringed {
            let hole: Vec<Piece> = ring.holes[0].iter().map(|u| u.piece).collect();
            let matched = parts.iter().filter(|s| {
                s.outer.len() == hole.len() && s.outer.iter().all(|u| hole.contains(&u.piece))
            });
            assert_eq!(matched.count(), 1, "{inner_first}: {parts:?}");
            assert!(ring.outer.iter().all(|u| !hole.contains(&u.piece)));
        }
        // The small square's own face holds the vertex and nothing else.
        let parts = of_face(&splitting, faces[2]);
        assert_eq!(parts.len(), 1, "{inner_first}: {parts:?}");
        assert_eq!(parts[0].inside, vec![lone]);
    }
}
