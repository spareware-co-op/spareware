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

use geom_bool::carrier::{CarrierIx, CurveGeom, PointGeom};
use geom_bool::interfere::{Arrangement, EdgeIn, EdgeUse, FaceIn, Interfered, VertexIn, interfere};
use geom_bool::merge::default_remaining;
use geom_bool::split::{Piece, SplitFace, Splitting, Use, split};
use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_num::P3;
use geom_num::plane::Plane;
use geom_run::{Budget, Exec, WorkMeter};
use geom_surface::{Cone, Cylinder, Sphere, Torus};
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

    /// A square on the plane z = 0, from (`lo`, `lo`) to (`hi`, `hi`).
    fn square(&mut self, lo: i64, hi: i64, operand: u32) -> usize {
        self.sheet(2, 0, (lo, hi), operand)
    }

    /// A square on the plane where coordinate `k` is `at`, from `lo` to `hi` in the other two,
    /// counter-clockwise seen from where coordinate `k` is larger; each in the tests' unit.
    fn sheet(&mut self, k: usize, at: i64, (lo, hi): (i64, i64), operand: u32) -> usize {
        let point = |a: i64, b: i64| {
            let mut c = [0; 3];
            c[k] = at;
            c[(k + 1) % 3] = a;
            c[(k + 2) % 3] = b;
            p(c[0], c[1], c[2])
        };
        let corners: Vec<usize> = [(lo, lo), (hi, lo), (hi, hi), (lo, hi)]
            .iter()
            .map(|&(a, b)| self.vertex(point(a, b), operand))
            .collect();
        let edges = (0..4)
            .map(|k| self.line(corners[k], corners[(k + 1) % 4], operand))
            .collect();
        let mut n = [0; 3];
        n[k] = 1;
        let plane = CarrierIx::Plane(
            self.arr
                .carriers
                .add_plane(&Plane::new(n, -i128::from(at * S)).unwrap())
                .unwrap(),
        );
        self.face(plane, &[edges], operand)
    }

    /// A face on the plane through grid points `corners[0..3]`, bounded by the four corners in
    /// order, in steps.
    fn quad(&mut self, corners: [[i64; 3]; 4], operand: u32) -> usize {
        let at = |c: [i64; 3]| P3::new(c[0], c[1], c[2]).unwrap();
        let ids: Vec<usize> = corners
            .iter()
            .map(|&c| self.vertex(at(c), operand))
            .collect();
        let edges: Vec<usize> = (0..4)
            .map(|k| self.line(ids[k], ids[(k + 1) % 4], operand))
            .collect();
        let plane = Plane::through(at(corners[0]), at(corners[1]), at(corners[2])).unwrap();
        // The corners run counter-clockwise about the plane's normal as made; its entry in the
        // table faces the other way when that normal's first part that is not zero is negative.
        let flipped = plane.n().iter().find(|v| **v != 0).is_some_and(|v| *v < 0);
        let plane = CarrierIx::Plane(self.arr.carriers.add_plane(&plane).unwrap());
        let uses = self.runs(&edges);
        let uses = if flipped { reversed(&uses) } else { uses };
        self.face_with(plane, vec![uses], operand)
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
        // Seen from outside, the side lies above its first circle and below its second.
        let face = self.face_with(ix, vec![vec![ahead(first)], vec![back(second)]], operand);
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

/// A closed edge run forward, as a loop of a face.
fn ahead(edge: usize) -> EdgeUse {
    EdgeUse {
        edge,
        forward: true,
    }
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

/// The pieces of a split face's outer loop and holes, each list in order.
fn shape_of(s: &SplitFace) -> (Vec<Piece>, Vec<Vec<Piece>>) {
    let pieces = |uses: &[Use]| uses.iter().map(|u| u.piece).collect::<Vec<_>>();
    (
        pieces(&s.outer),
        s.holes.iter().map(|h| pieces(h)).collect(),
    )
}

/// A plane square to a cylinder's axis across its side, its edges far away: no vertex is on both
/// faces, and the carriers cross in a circle (G165's closed form), whole, inside both. It is one
/// section with no pave: the side splits into the band above it and the band below, and the
/// plane into the disc inside the circle and the rest, whose hole the circle is.
#[test]
fn a_plane_across_a_cylinder_cuts_it_in_a_circle() {
    let mut b = Build::default();
    let (side, near, far) = b.tube((p(0, 0, -50), 2, 1.0), 50, (0, 100), 0);
    let sheet = b.square(-1000, 1000, 1);
    let (out, splitting) = b.run();
    for f in [side, sheet] {
        let info = &out.faces[f];
        assert!(
            info.pending.faces.is_empty() && info.pending.edges.is_empty(),
            "{:?}",
            info.pending
        );
        assert_eq!(info.sections, vec![0]);
    }
    assert_eq!(out.sections.len(), 1, "{:?}", out.sections);
    let section = out.sections[0];
    assert_eq!(section.faces, [side, sheet]);
    assert_eq!(section.curve, CurveGeom::Section(0));
    assert!(section.from.is_none() && section.to.is_none() && !section.wraps);
    match &out.curves[0] {
        CurveKind::Circle(c) => {
            // The closed form's data are the midpoints of enclosures, within its class's steps.
            assert!(c.center().iter().all(|x| x.abs() < 1.0), "{c:?}");
            assert_eq!(c.radius(), (50 * S) as f64);
        }
        other => panic!("{other:?}"),
    }
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    let circle = Piece::Section(0);
    let (near, far) = (whole_piece(&out, near), whole_piece(&out, far));
    let mut bands: Vec<_> = of_face(&splitting, side)
        .into_iter()
        .map(shape_of)
        .collect();
    bands.sort();
    let mut want = vec![
        (vec![far], vec![vec![circle]]),
        (vec![circle], vec![vec![near]]),
    ];
    want.sort();
    assert_eq!(bands, want);
    let parts = of_face(&splitting, sheet);
    assert_eq!(parts.len(), 2, "{parts:?}");
    let disc: Vec<_> = parts.iter().filter(|s| s.outer.len() == 1).collect();
    assert_eq!(disc.len(), 1, "{parts:?}");
    assert_eq!(shape_of(disc[0]), (vec![circle], Vec::new()));
    let rest: Vec<_> = parts.iter().filter(|s| s.outer.len() == 4).collect();
    assert_eq!(rest.len(), 1, "{parts:?}");
    assert_eq!(
        rest[0].holes,
        vec![vec![run(circle, !disc[0].outer[0].forward)]]
    );
}

/// A plane along a cylinder's axis, 30 from it, across its side of radius 50: the carriers cross
/// in two rulings 40 either side of the axis (G165), each from the rim below to the rim above.
/// The rims cross the plane at four vertices, on the side's boundary and In the plane: each
/// ruling's paves, so each ruling is one section between two of them. The side splits into the
/// narrow part on the near side of the plane and the wide part, each between the two rulings and
/// an arc of each rim; on the plane the two rulings are cuts of the one face.
#[test]
fn a_plane_along_a_cylinder_cuts_it_in_two_rulings() {
    let mut b = Build::default();
    let (side, near, far) = b.tube((p(0, 0, -50), 2, 1.0), 50, (0, 100), 0);
    let wall = b.sheet(0, 30, (-1000, 1000), 1);
    let (out, splitting) = b.run();
    assert_eq!(out.sections.len(), 2, "{:?}", out.sections);
    for (k, section) in out.sections.iter().enumerate() {
        assert_eq!(section.faces, [side, wall]);
        let ends = [section.from.unwrap().vertex, section.to.unwrap().vertex];
        let at = |v: usize| b.arr.carriers.commit(&out.vertices[v]).unwrap().0;
        let (a, c) = (at(ends[0]), at(ends[1]));
        assert_eq!(
            (a.x.get(), a.y.get()),
            (c.x.get(), c.y.get()),
            "{k}: {a:?} {c:?}"
        );
        assert_eq!(a.x.get(), 30 * S);
        assert_eq!(a.y.get().abs(), 40 * S);
        assert_eq!((a.z.get() - c.z.get()).abs(), 100 * S);
        assert!(
            matches!(out.curves[k], CurveKind::Line(_)),
            "{:?}",
            out.curves[k]
        );
    }
    assert_eq!(
        out.faces[wall].in_vertices.len(),
        4,
        "{:?}",
        out.faces[wall]
    );
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    let rulings = [Piece::Section(0), Piece::Section(1)];
    let parts = of_face(&splitting, side);
    assert_eq!(parts.len(), 2, "{parts:?}");
    for part in &parts {
        assert_eq!(part.outer.len(), 4, "{parts:?}");
        assert!(part.holes.is_empty() && part.cuts.is_empty(), "{parts:?}");
        let pieces: Vec<Piece> = part.outer.iter().map(|u| u.piece).collect();
        assert!(rulings.iter().all(|r| pieces.contains(r)), "{parts:?}");
        assert_eq!(
            pieces
                .iter()
                .filter(|q| matches!(q, Piece::Split(_)))
                .count(),
            2,
            "{parts:?}"
        );
    }
    let sources = |part: &SplitFace| -> Vec<usize> {
        let mut s: Vec<usize> = part
            .outer
            .iter()
            .filter_map(|u| match u.piece {
                Piece::Split(k) => Some(out.splits[k].source),
                Piece::Section(_) => None,
            })
            .collect();
        s.sort_unstable();
        s
    };
    assert!(parts.iter().all(|part| sources(part) == vec![near, far]));
    let parts = of_face(&splitting, wall);
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].cuts.len(), 2, "{parts:?}");
}

/// A plane across a cone opening upwards from its apex, at height 25: a circle of radius 25,
/// inside both faces. The cone splits into the cap round its apex and the band to its rim, and
/// the plane into the disc and the rest.
#[test]
fn a_plane_across_a_cone_cuts_it_in_a_circle() {
    let mut b = Build::default();
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let rim = b.circle(p(0, 0, 100), 100, frame, 0);
    let cone = Cone::new(
        [0.0; 3],
        0.0,
        1.0,
        frame,
        ((0.0, 2.0), (0.0, (100 * S) as f64)),
    );
    let ix = b.surface(SurfaceKind::Cone(cone.unwrap()));
    // The cone lies below its rim: seen from outside, on the rim's right.
    let body = b.face_with(ix, vec![vec![back(rim)]], 0);
    let sheet = b.sheet(2, 25, (-1000, 1000), 1);
    let (out, splitting) = b.run();
    assert_eq!(out.sections.len(), 1, "{:?}", out.sections);
    match &out.curves[0] {
        CurveKind::Circle(c) => {
            assert_eq!(c.center(), coords(p(0, 0, 25)));
            assert_eq!(c.radius(), (25 * S) as f64);
        }
        other => panic!("{other:?}"),
    }
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    let circle = Piece::Section(0);
    let rim = whole_piece(&out, rim);
    let mut parts: Vec<_> = of_face(&splitting, body)
        .into_iter()
        .map(shape_of)
        .collect();
    parts.sort();
    let mut want = vec![(vec![circle], Vec::new()), (vec![rim], vec![vec![circle]])];
    want.sort();
    assert_eq!(parts, want);
    assert_eq!(of_face(&splitting, sheet).len(), 2);
}

/// A cone about the axis through `centre` (in the tests' unit) along coordinate `k`, given by its
/// radius `r0` at its centre and its slope, and its face between its rim, at the height `rim_at`,
/// and its apex.
fn apex_cone(
    b: &mut Build,
    (centre, k): ([i64; 3], usize),
    (r0, slope, rim_at): (i64, f64, i64),
    domain: (i64, i64),
) -> usize {
    let frame = Frame::new(unit((k + 1) % 3, 1.0), unit(k, 1.0)).unwrap();
    let mut middle = centre;
    middle[k] += rim_at;
    let rim_radius = r0 as f64 + slope * rim_at as f64;
    let rim = b.circle(
        p(middle[0], middle[1], middle[2]),
        rim_radius as i64,
        frame,
        0,
    );
    let cone = Cone::new(
        coords(p(centre[0], centre[1], centre[2])),
        (r0 * S) as f64,
        slope,
        frame,
        ((0.0, 2.0), ((domain.0 * S) as f64, (domain.1 * S) as f64)),
    );
    let ix = b.surface(SurfaceKind::Cone(cone.unwrap()));
    // Seen from outside, a cone opening up the axis lies on its upper rim's right, and one
    // narrowing up the axis on its lower rim's left.
    let ring = if slope > 0.0 { back(rim) } else { ahead(rim) };
    b.face_with(ix, vec![vec![ring]], 0)
}

/// A plane through a cone's apex and along its axis: the carriers cross in two rulings (12.7's
/// closed form), each from the apex to the rim. The apex lies inside both faces and is no vertex
/// of either, so a vertex is made there, In both, and each ruling is one section from it to where
/// the rim crosses the plane. The cone splits into its two halves, each two rulings and an arc of
/// the rim; on the plane the two rulings are one cut of the one face. So for a cone opening from
/// its apex at the origin up the third coordinate (x = 0, z = ±y), and for one narrowing to its
/// apex 50 from a rim of radius 100, about an axis through (30, 40, 10) along each coordinate.
#[test]
fn a_plane_through_a_cone_s_apex_cuts_it_in_two_rulings() {
    let off = [30, 40, 10];
    for (axis, shape, domain, tip) in [
        (([0; 3], 2), (0, 1.0, 100), (0, 100), p(0, 0, 0)),
        ((off, 2), (100, -2.0, 0), (0, 50), p(30, 40, 60)),
        ((off, 0), (100, -2.0, 0), (0, 50), p(80, 40, 10)),
        ((off, 1), (100, -2.0, 0), (0, 50), p(30, 90, 10)),
    ] {
        check_apex_cut(axis, shape, domain, tip);
    }
}

/// [`a_plane_through_a_cone_s_apex_cuts_it_in_two_rulings`] for one cone, cut by the plane through
/// its axis where the coordinate after the axis's is the centre's.
fn check_apex_cut(
    (centre, k): ([i64; 3], usize),
    shape: (i64, f64, i64),
    domain: (i64, i64),
    tip: P3,
) {
    let mut b = Build::default();
    let body = apex_cone(&mut b, (centre, k), shape, domain);
    let across = (k + 1) % 3;
    let sheet = b.sheet(across, centre[across], (-1000, 1000), 1);
    let (out, splitting) = b.run();
    assert!(
        splitting.waiting.is_empty(),
        "{tip:?}: {:?}",
        splitting.waiting
    );
    assert_eq!(out.sections.len(), 2, "{tip:?}: {:?}", out.sections);
    // The vertex made at the apex is In both faces and ends both sections.
    let apex = out.vertices.len() - 1;
    match &out.vertices[apex] {
        PointGeom::Certified { at, .. } => assert_eq!(*at, tip),
        other => panic!("{other:?}"),
    }
    for f in [body, sheet] {
        assert!(out.faces[f].in_vertices.contains(&apex), "{f}");
    }
    for s in &out.sections {
        let ends = [s.from.unwrap().vertex, s.to.unwrap().vertex];
        assert!(ends.contains(&apex), "{s:?}");
    }
    let parts = of_face(&splitting, body);
    assert_eq!(parts.len(), 2, "{parts:?}");
    for half in &parts {
        assert_eq!(half.outer.len(), 3, "{parts:?}");
        assert!(half.holes.is_empty(), "{parts:?}");
    }
    let parts = of_face(&splitting, sheet);
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].cuts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].cuts[0].len(), 4, "{parts:?}");
}

/// The same plane through the apex of the same cone, whose face is now the band between heights
/// 50 and 100: the apex lies outside the band, so no vertex is made there, and each ruling is one
/// section between the two rims' crossings of the plane. The band splits into its two halves,
/// each two rulings and an arc of each rim.
#[test]
fn a_plane_through_a_cone_s_apex_outside_its_face_cuts_it_between_its_rims() {
    let mut b = Build::default();
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let low = b.circle(p(0, 0, 50), 50, frame, 0);
    let high = b.circle(p(0, 0, 100), 100, frame, 0);
    let cone = Cone::new(
        [0.0; 3],
        0.0,
        1.0,
        frame,
        ((0.0, 2.0), (0.0, (100 * S) as f64)),
    );
    let ix = b.surface(SurfaceKind::Cone(cone.unwrap()));
    let band = b.face_with(ix, vec![vec![ahead(low)], vec![back(high)]], 0);
    let sheet = b.sheet(0, 0, (-1000, 1000), 1);
    let (out, splitting) = b.run();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    assert_eq!(out.sections.len(), 2, "{:?}", out.sections);
    let at = |v: usize| b.arr.carriers.commit(&out.vertices[v]).unwrap().0;
    assert!((0..out.vertices.len()).all(|v| at(v) != p(0, 0, 0)));
    for s in &out.sections {
        let mut ends = [at(s.from.unwrap().vertex), at(s.to.unwrap().vertex)];
        ends.sort_by_key(|q| q.z.get());
        assert_eq!(
            (ends[0].z.get(), ends[1].z.get()),
            (50 * S, 100 * S),
            "{ends:?}"
        );
    }
    let parts = of_face(&splitting, band);
    assert_eq!(parts.len(), 2, "{parts:?}");
    for half in &parts {
        assert_eq!(half.outer.len(), 4, "{parts:?}");
        assert!(half.holes.is_empty(), "{parts:?}");
    }
    assert_eq!(of_face(&splitting, sheet).len(), 1);
}

/// A strip of the plane x = 50, from z = 20 to 80, tangent to a cylinder of radius 50 about the
/// third coordinate: the carriers touch along the ruling x = 50, y = 0, which the closed form
/// finds from an exact sign of nothing (the plane stands its radius from the axis), so it is a
/// section. The strip's lower and upper edges touch the side where they cross the ruling: those
/// two points are vertices of both faces, and the ruling's section runs between them. The strip
/// splits into its two halves; on the side the ruling is a cut.
#[test]
fn a_plane_tangent_to_a_cylinder_meets_it_along_a_ruling() {
    let mut b = Build::default();
    let (side, _, _) = b.tube((p(0, 0, 0), 2, 1.0), 50, (0, 100), 0);
    let strip = b.quad(
        [
            [50 * S, -10 * S, 20 * S],
            [50 * S, 10 * S, 20 * S],
            [50 * S, 10 * S, 80 * S],
            [50 * S, -10 * S, 80 * S],
        ],
        1,
    );
    let (out, splitting) = b.run();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    assert_eq!(out.sections.len(), 1, "{:?}", out.sections);
    let section = &out.sections[0];
    let at = |v: usize| b.arr.carriers.commit(&out.vertices[v]).unwrap().0;
    let mut ends = [
        at(section.from.unwrap().vertex),
        at(section.to.unwrap().vertex),
    ];
    ends.sort_by_key(|q| q.z.get());
    assert_eq!(ends, [p(50, 0, 20), p(50, 0, 80)]);
    let parts = of_face(&splitting, strip);
    assert_eq!(parts.len(), 2, "{parts:?}");
    for half in &parts {
        assert_eq!(half.outer.len(), 4, "{parts:?}");
        assert!(half.outer.iter().any(|u| u.piece == Piece::Section(0)));
    }
    let parts = of_face(&splitting, side);
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].cuts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].cuts[0].len(), 2, "{parts:?}");
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
    let window = b.face(ix, &[vec![low, up, high, down]], 1);
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

/// A cone opening upwards from its apex to its rim at height 100, closed by its cap, and a plane
/// parallel to its axis, 30 from it: the plane cuts the cone in a hyperbola's branch from one
/// crossing of the rim to the other (G165), and the cap in a line between the same two. On the
/// plane the branch and the line bound the part inside the cone, its area measured in closed
/// form; the cone and the cap each split in two.
#[test]
fn a_plane_along_a_cone_cuts_it_in_a_hyperbola() {
    let mut b = Build::default();
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let rim = b.circle(p(0, 0, 100), 100, frame, 0);
    let cone = Cone::new(
        [0.0; 3],
        0.0,
        1.0,
        frame,
        ((0.0, 2.0), (0.0, (100 * S) as f64)),
    );
    let ix = b.surface(SurfaceKind::Cone(cone.unwrap()));
    // The cone lies below its rim: seen from outside, on the rim's right.
    let body = b.face_with(ix, vec![vec![back(rim)]], 0);
    let lid = CarrierIx::Plane(
        b.arr
            .carriers
            .add_plane(&Plane::new([0, 0, 1], -i128::from(100 * S)).unwrap())
            .unwrap(),
    );
    let cap = b.face(lid, &[vec![rim]], 0);
    let wall = b.sheet(1, 30, (-1000, 1000), 1);
    let (out, splitting) = b.run();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    let branch: Vec<usize> = (0..out.sections.len())
        .filter(|&s| out.sections[s].faces == [body, wall])
        .collect();
    assert_eq!(branch.len(), 1, "{:?}", out.sections);
    let CurveGeom::Section(ix) = out.sections[branch[0]].curve else {
        panic!("{:?}", out.sections[branch[0]]);
    };
    assert!(
        matches!(out.curves[ix], CurveKind::Hyperbola(_)),
        "{:?}",
        out.curves[ix]
    );
    let across: Vec<usize> = (0..out.sections.len())
        .filter(|&s| out.sections[s].faces == [cap, wall])
        .collect();
    assert_eq!(across.len(), 1, "{:?}", out.sections);
    let (branch, across) = (Piece::Section(branch[0]), Piece::Section(across[0]));
    for f in [body, cap] {
        assert_eq!(of_face(&splitting, f).len(), 2, "{f}");
    }
    let parts = of_face(&splitting, wall);
    assert_eq!(parts.len(), 2, "{parts:?}");
    let inside: Vec<_> = parts.iter().filter(|s| s.outer.len() == 2).collect();
    assert_eq!(inside.len(), 1, "{parts:?}");
    let mut pieces: Vec<Piece> = inside[0].outer.iter().map(|u| u.piece).collect();
    pieces.sort();
    let mut want = vec![branch, across];
    want.sort();
    assert_eq!(pieces, want);
    let rest: Vec<_> = parts.iter().filter(|s| s.outer.len() == 4).collect();
    assert_eq!(rest.len(), 1, "{parts:?}");
    assert_eq!(rest[0].holes.len(), 1, "{parts:?}");
}

/// The same cone and cap, and the plane z = x + 50, parallel to one of the cone's rulings: it
/// cuts the cone in a parabola from one crossing of the rim to the other, and the cap in a line
/// between the same two. On the plane the parabola and the line bound the part inside the cone,
/// its area measured in closed form; the cone and the cap each split in two.
#[test]
fn a_plane_along_a_ruling_of_a_cone_cuts_it_in_a_parabola() {
    let mut b = Build::default();
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let rim = b.circle(p(0, 0, 100), 100, frame, 0);
    let cone = Cone::new(
        [0.0; 3],
        0.0,
        1.0,
        frame,
        ((0.0, 2.0), (0.0, (100 * S) as f64)),
    );
    let ix = b.surface(SurfaceKind::Cone(cone.unwrap()));
    // The cone lies below its rim: seen from outside, on the rim's right.
    let body = b.face_with(ix, vec![vec![back(rim)]], 0);
    let lid = CarrierIx::Plane(
        b.arr
            .carriers
            .add_plane(&Plane::new([0, 0, 1], -i128::from(100 * S)).unwrap())
            .unwrap(),
    );
    let cap = b.face(lid, &[vec![rim]], 0);
    let wall = b.quad(
        [
            [-60 * S, -120 * S, -10 * S],
            [80 * S, -120 * S, 130 * S],
            [80 * S, 120 * S, 130 * S],
            [-60 * S, 120 * S, -10 * S],
        ],
        1,
    );
    let (out, splitting) = b.run();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    let curve: Vec<usize> = (0..out.sections.len())
        .filter(|&s| out.sections[s].faces == [body, wall])
        .collect();
    assert_eq!(curve.len(), 1, "{:?}", out.sections);
    let CurveGeom::Section(ix) = out.sections[curve[0]].curve else {
        panic!("{:?}", out.sections[curve[0]]);
    };
    assert!(
        matches!(out.curves[ix], CurveKind::Parabola(_)),
        "{:?}",
        out.curves[ix]
    );
    let across: Vec<usize> = (0..out.sections.len())
        .filter(|&s| out.sections[s].faces == [cap, wall])
        .collect();
    assert_eq!(across.len(), 1, "{:?}", out.sections);
    let (curve, across) = (Piece::Section(curve[0]), Piece::Section(across[0]));
    for f in [body, cap] {
        assert_eq!(of_face(&splitting, f).len(), 2, "{f}");
    }
    let parts = of_face(&splitting, wall);
    assert_eq!(parts.len(), 2, "{parts:?}");
    let inside: Vec<_> = parts.iter().filter(|s| s.outer.len() == 2).collect();
    assert_eq!(inside.len(), 1, "{parts:?}");
    let mut pieces: Vec<Piece> = inside[0].outer.iter().map(|u| u.piece).collect();
    pieces.sort();
    let mut want = vec![curve, across];
    want.sort();
    assert_eq!(pieces, want);
    let rest: Vec<_> = parts.iter().filter(|s| s.outer.len() == 4).collect();
    assert_eq!(rest.len(), 1, "{parts:?}");
    assert_eq!(rest[0].holes.len(), 1, "{parts:?}");
}

/// A plane through the axis of a cylinder at a slope of one in two, given by grid points far
/// apart, so its normal's numbers are large: it cuts the side in an ellipse, whole and inside
/// both faces. The side splits into the part above it and the part below, and the plane into the
/// ellipse's inside and the rest.
#[test]
fn a_tilted_plane_cuts_a_cylinder_in_an_ellipse() {
    let mut b = Build::default();
    let (side, _, _) = b.tube((p(0, 0, -50), 2, 1.0), 50, (0, 100), 0);
    let m = 1000 * S;
    let sheet = b.quad(
        [
            [-m, -m, -m / 2],
            [m, -m, m / 2],
            [m, m, m / 2],
            [-m, m, -m / 2],
        ],
        1,
    );
    let (out, splitting) = b.run();
    assert_eq!(out.sections.len(), 1, "{:?}", out.sections);
    assert!(out.sections[0].from.is_none());
    assert!(
        matches!(out.curves[0], CurveKind::Ellipse(_)),
        "{:?}",
        out.curves[0]
    );
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    let ellipse = Piece::Section(0);
    let parts = of_face(&splitting, side);
    assert_eq!(parts.len(), 2, "{parts:?}");
    assert!(
        parts.iter().all(|s| s.outer == vec![run(ellipse, true)]
            || s.holes == vec![vec![run(ellipse, false)]]
            || s.outer == vec![run(ellipse, false)]
            || s.holes == vec![vec![run(ellipse, true)]]),
        "{parts:?}"
    );
    let parts = of_face(&splitting, sheet);
    assert_eq!(parts.len(), 2, "{parts:?}");
    let inside: Vec<_> = parts.iter().filter(|s| s.outer.len() == 1).collect();
    assert_eq!(inside.len(), 1, "{parts:?}");
    assert_eq!(inside[0].outer[0].piece, ellipse);
}

/// A plane x + 2z = 1 step: its table form has no point on the z axis a double holds as a whole
/// number (the plane crosses it at half a step), so no surface holds it exactly and the closed
/// forms are not asked. The pair waits rather than meet a plane that is not the face's.
#[test]
fn a_plane_no_surface_holds_exactly_waits() {
    let mut b = Build::default();
    let (side, _, _) = b.tube((p(0, 0, -50), 2, 1.0), 50, (0, 100), 0);
    let (h, w) = (500 * S, 1000 * S);
    let sheet = b.quad(
        [
            [1 - 2 * h, -w, h],
            [1 + 2 * h, -w, -h],
            [1 + 2 * h, w, -h],
            [1 - 2 * h, w, h],
        ],
        1,
    );
    let (out, splitting) = b.run();
    assert!(out.sections.is_empty(), "{:?}", out.sections);
    assert_eq!(out.faces[side].pending.faces, vec![sheet]);
    assert_eq!(out.faces[sheet].pending.faces, vec![side]);
    assert_eq!(splitting.waiting, vec![side, sheet]);
}

/// Two cylinders of different radii whose axes cross square: in general position, where they
/// meet is a quartic the closed forms of 12.7 do not give as a conic (its curve is a procedural
/// one, G175's kind), so the pair waits.
#[test]
fn cylinders_in_general_position_wait() {
    let mut b = Build::default();
    let (upright, _, _) = b.tube((p(0, 0, -100), 2, 1.0), 50, (0, 200), 0);
    let (across, _, _) = b.tube((p(-100, 0, 0), 0, 1.0), 30, (0, 200), 1);
    let (out, splitting) = b.run();
    assert!(out.sections.is_empty(), "{:?}", out.sections);
    assert_eq!(out.faces[upright].pending.faces, vec![across]);
    assert_eq!(splitting.waiting, vec![upright, across]);
}

/// Two cones of one shape about different points of one axis are not known to be one carrier
/// (only cones with the same stored data are), and the closed forms prove they never meet: their
/// meridians are parallel. Neither waits, and each stays whole.
#[test]
fn cones_about_different_centres_never_meet() {
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
        faces.push(b.face_with(ix, vec![vec![back(rim)]], operand));
    }
    let (out, splitting) = b.run();
    assert!(
        out.faces[faces[0]].same_carrier.is_empty(),
        "{:?}",
        out.faces[faces[0]].same_carrier
    );
    assert!(
        out.faces[faces[0]].pending.faces.is_empty(),
        "{:?}",
        out.faces[faces[0]].pending
    );
    assert!(out.sections.is_empty(), "{:?}", out.sections);
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    for f in faces {
        assert_eq!(of_face(&splitting, f).len(), 1);
    }
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

/// A cylinder inside another, about the same axis but of another radius, is another carrier, and
/// the closed forms prove the two never meet: neither waits, and each side stays whole.
#[test]
fn coaxial_cylinders_of_two_radii_never_meet() {
    let mut b = Build::default();
    let (outer, _, _) = b.tube((p(0, 0, 0), 2, 1.0), 50, (0, 100), 0);
    let (inner, _, _) = b.tube((p(0, 0, 0), 2, 1.0), 30, (0, 100), 1);
    let (out, splitting) = b.run();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    assert_eq!(of_face(&splitting, outer).len(), 1);
    assert_eq!(of_face(&splitting, inner).len(), 1);
    assert_eq!(out.faces[outer].pending.faces, Vec::<usize>::new());
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
    let body = b.face_with(whole, vec![vec![back(rim)]], 0);
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
        // Seen from outside the cone, out along a ruling, round the arc and back runs clockwise:
        // the sector lies on the loop's left run the other way.
        let ring = reversed(&b.runs(&[out_ruling, arc, back_ruling]));
        sectors.push(b.face_with(patch, vec![ring], operand));
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

impl Build {
    /// A face on the sphere of radius 50 about the origin, in the tests' unit, bounded by the
    /// circle of radius `r` about the third coordinate at height `h`: run counter-clockwise about
    /// that coordinate (`forward`), the face is the cap above the circle; run back, the rest.
    /// The face, and the circle's edge.
    fn on_ball(&mut self, (h, r): (i64, i64), forward: bool, operand: u32) -> (usize, usize) {
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let rim = self.circle(p(0, 0, h), r, frame, operand);
        let ball =
            Sphere::new([0.0; 3], (50 * S) as f64, frame, ((0.0, 2.0), (-0.5, 0.5))).unwrap();
        let ix = self.surface(SurfaceKind::Sphere(ball));
        let use_ = EdgeUse { edge: rim, forward };
        (self.face_with(ix, vec![vec![use_]], operand), rim)
    }
}

/// A sphere's face is named by which way its loop runs: one circle at height 40 bounds the cap
/// above it, run counter-clockwise about the axis, and the rest of the sphere, run back. A vertex
/// of another operand at each pole is In the face that holds it, and each face is one face, its
/// outer loop the circle run with the face on its left.
#[test]
fn a_sphere_s_loop_says_which_side_is_the_face() {
    for forward in [true, false] {
        let mut b = Build::default();
        let (face, rim) = b.on_ball((40, 30), forward, 0);
        let north = b.vertex(p(0, 0, 50), 1);
        let south = b.vertex(p(0, 0, -50), 1);
        let (out, splitting) = b.run();
        assert!(
            splitting.waiting.is_empty(),
            "{forward}: {:?}",
            splitting.waiting
        );
        let held = if forward { north } else { south };
        assert_eq!(out.faces[face].in_vertices, vec![held], "{forward}");
        let parts = of_face(&splitting, face);
        assert_eq!(parts.len(), 1, "{forward}: {parts:?}");
        assert_eq!(parts[0].outer, vec![run(whole_piece(&out, rim), forward)]);
        assert!(parts[0].holes.is_empty(), "{forward}: {parts:?}");
        assert_eq!(parts[0].inside, vec![held], "{forward}");
    }
}

/// The upper half of a sphere and the plane at height 30 across it: the carriers cross in the
/// circle of radius 40 there (12.7's closed form), whole and inside both faces. The half splits
/// into the cap above the circle and the band below it, and the plane into the disc inside the
/// circle and the rest round it.
#[test]
fn a_plane_across_a_sphere_cuts_it_in_a_circle() {
    let mut b = Build::default();
    let (half, equator) = b.on_ball((0, 50), true, 0);
    let sheet = b.sheet(2, 30, (-1000, 1000), 1);
    let (out, splitting) = b.run();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    assert_eq!(out.sections.len(), 1, "{:?}", out.sections);
    match &out.curves[0] {
        // The closed form's centre and radius, to within its rounding.
        CurveKind::Circle(c) => {
            let [x, y, z] = c.center();
            assert!(x.abs() < 1e-6 && y.abs() < 1e-6, "{:?}", c.center());
            assert!((z - (30 * S) as f64).abs() < 1e-6, "{:?}", c.center());
            assert!(
                (c.radius() - (40 * S) as f64).abs() < 1e-6,
                "{}",
                c.radius()
            );
        }
        other => panic!("{other:?}"),
    }
    let parts = of_face(&splitting, half);
    assert_eq!(parts.len(), 2, "{parts:?}");
    let circle = Piece::Section(0);
    let cap: Vec<_> = parts.iter().filter(|s| s.holes.is_empty()).collect();
    assert_eq!(cap.len(), 1, "{parts:?}");
    assert_eq!(cap[0].outer.len(), 1, "{parts:?}");
    assert_eq!(cap[0].outer[0].piece, circle);
    let band: Vec<_> = parts.iter().filter(|s| s.holes.len() == 1).collect();
    assert_eq!(band.len(), 1, "{parts:?}");
    assert_eq!(band[0].outer, vec![run(whole_piece(&out, equator), true)]);
    assert_eq!(band[0].holes[0].len(), 1, "{parts:?}");
    assert_eq!(band[0].holes[0][0].piece, circle);
    // The cap runs the circle one way, the band's hole the other.
    assert_ne!(cap[0].outer[0].forward, band[0].holes[0][0].forward);
    let parts = of_face(&splitting, sheet);
    assert_eq!(parts.len(), 2, "{parts:?}");
}

/// Faces of two operands on one sphere, given as two surfaces with the same centre and radius,
/// are one carrier's: the cap above height 40 lies in the upper half, whose face splits along the
/// cap's circle into the cap and the band below it; the cap stays whole.
#[test]
fn faces_on_one_sphere_split_each_other() {
    let mut b = Build::default();
    let (half, equator) = b.on_ball((0, 50), true, 0);
    let (cap, rim) = b.on_ball((40, 30), true, 1);
    let (out, splitting) = b.run();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    assert_eq!(out.faces[half].same_carrier, vec![cap]);
    assert_eq!(out.faces[cap].same_carrier, vec![half]);
    let parts = of_face(&splitting, half);
    assert_eq!(parts.len(), 2, "{parts:?}");
    let got: Vec<_> = parts
        .iter()
        .map(|s| (s.outer.clone(), s.holes.clone()))
        .collect();
    let (circle, bottom) = (whole_piece(&out, rim), whole_piece(&out, equator));
    assert!(got.contains(&(vec![run(circle, true)], vec![])), "{got:?}");
    assert!(
        got.contains(&(vec![run(bottom, true)], vec![vec![run(circle, false)]])),
        "{got:?}"
    );
    assert_eq!(of_face(&splitting, cap).len(), 1);
}

impl Build {
    /// A face on the torus of radii 50 and 25 about the third coordinate through the origin, in
    /// the tests' unit, bounded by its two circles at height `h` (of radii 50 + √(625 − h²) and
    /// 50 − √(625 − h²), given as `radii`): run with the outer one counter-clockwise about the
    /// axis and the inner one back (`forward`), the face is the band of the torus beyond the
    /// height; the other way, the rest of the torus. The face, and the outer and inner circles.
    fn on_torus(
        &mut self,
        (h, radii): (i64, [i64; 2]),
        forward: bool,
        operand: u32,
    ) -> (usize, usize, usize) {
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let [outer, inner] = radii.map(|r| self.circle(p(0, 0, h), r, frame, operand));
        let torus = Torus::new(
            [0.0; 3],
            (50 * S) as f64,
            (25 * S) as f64,
            frame,
            ((0.0, 2.0), (0.0, 2.0)),
        )
        .unwrap();
        let ix = self.surface(SurfaceKind::Torus(torus));
        let (o, i) = if forward {
            (ahead(outer), back(inner))
        } else {
            (back(outer), ahead(inner))
        };
        (
            self.face_with(ix, vec![vec![o], vec![i]], operand),
            outer,
            inner,
        )
    }
}

/// A torus's face above its middle plane, the band above height 15 between its circles of radii
/// 70 and 30, is seen along its axis. Vertices of another operand on the torus's top circle are In
/// it; one on its bottom circle and one on its outer equator lie below the band's least height
/// and are not. The band is one face, the outer circle its outer loop and the inner its hole. The
/// same circles run the other way bound the rest of the torus, which holds the lower half: no
/// picture along the axis holds it, and it waits.
#[test]
fn a_torus_s_face_above_its_middle_plane_is_seen_along_its_axis() {
    let mut b = Build::default();
    let (band, outer, inner) = b.on_torus((15, [70, 30]), true, 0);
    let tops: Vec<usize> = [(50, 0), (0, 50), (-50, 0)]
        .iter()
        .map(|&(x, y)| b.vertex(p(x, y, 25), 1))
        .collect();
    b.vertex(p(50, 0, -25), 1);
    b.vertex(p(75, 0, 0), 1);
    // On the torus below the band's height, outside the band's circles; and above the top, off
    // the torus.
    b.vertex(p(74, 0, 7), 1);
    b.vertex(p(50, 0, 30), 1);
    let (out, splitting) = b.run();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    assert_eq!(out.faces[band].in_vertices, tops);
    let parts = of_face(&splitting, band);
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert_eq!(parts[0].outer, vec![run(whole_piece(&out, outer), true)]);
    assert_eq!(
        parts[0].holes,
        vec![vec![run(whole_piece(&out, inner), false)]]
    );
    assert_eq!(parts[0].inside, tops);
    // The rest of the torus.
    let mut b = Build::default();
    let (rest, _, _) = b.on_torus((15, [70, 30]), false, 0);
    let (_, splitting) = b.run();
    assert_eq!(splitting.waiting, vec![rest]);
}

/// A torus whose axis is tilted off every coordinate, and one whose tube reaches its axis, have
/// no picture along the axis: a band of either waits.
#[test]
fn a_torus_on_a_tilted_axis_or_reaching_its_axis_waits() {
    for (normal, minor) in [([0.0, 0.6, 0.8], 25.0), ([0.0, 0.0, 1.0], 50.0)] {
        let mut b = Build::default();
        let frame = Frame::new([1.0, 0.0, 0.0], normal).unwrap();
        let rims: Vec<usize> = [70, 30]
            .iter()
            .map(|&r| b.circle(p(0, 0, 0), r, frame, 0))
            .collect();
        let torus = Torus::new(
            [0.0; 3],
            (50 * S) as f64,
            minor * S as f64,
            frame,
            ((0.0, 2.0), (0.0, 2.0)),
        )
        .unwrap();
        let ix = b.surface(SurfaceKind::Torus(torus));
        let band = b.face_with(ix, vec![vec![ahead(rims[0])], vec![back(rims[1])]], 0);
        let (_, splitting) = b.run();
        assert_eq!(splitting.waiting, vec![band], "{normal:?} {minor}");
    }
}

/// The band of the torus of radii 50 and 25 above height 15, and a band of another operand above
/// height 20 between that torus's circles of radii 65 and 35 there, on a torus given again with
/// the same centre and radii, its axis pointing either way: one carrier. The first band splits
/// along the second's circles into the second and the two narrow bands either side of it. A band
/// on a torus of another tube about the same centre is another carrier.
#[test]
fn faces_on_one_torus_split_each_other() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    for (axis, minor, height, radii) in [
        (1.0, 25, 20, [65, 35]),
        (-1.0, 25, 20, [65, 35]),
        (1.0, 20, 12, [66, 34]),
    ] {
        let mut b = Build::default();
        let (band, _, _) = b.on_torus((15, [70, 30]), true, 0);
        let rims: Vec<usize> = radii
            .iter()
            .map(|&r| b.circle(p(0, 0, height), r, frame, 1))
            .collect();
        let along = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, axis]).unwrap();
        let torus = Torus::new(
            [0.0; 3],
            (50 * S) as f64,
            (minor * S) as f64,
            along,
            ((0.0, 2.0), (0.0, 2.0)),
        )
        .unwrap();
        let ix = b.surface(SurfaceKind::Torus(torus));
        let inner = b.face_with(ix, vec![vec![ahead(rims[0])], vec![back(rims[1])]], 1);
        let (out, splitting) = b.run();
        if minor != 25 {
            assert!(out.faces[band].same_carrier.is_empty(), "{minor}");
            continue;
        }
        assert_eq!(out.faces[band].same_carrier, vec![inner], "{axis}");
        assert_eq!(out.faces[inner].same_carrier, vec![band], "{axis}");
        assert!(
            splitting.waiting.is_empty(),
            "{axis}: {:?}",
            splitting.waiting
        );
        assert_eq!(of_face(&splitting, band).len(), 3, "{axis}");
        assert_eq!(of_face(&splitting, inner).len(), 1, "{axis}");
    }
}

/// A plane resting on a torus's top, at height 25, touches it along the circle of radius 50
/// there: the closed form for a plane and a torus gives that circle from its own search, not from
/// an exact sign, so the touch is not proven exact, and the pair waits.
#[test]
fn a_plane_resting_on_a_torus_waits() {
    let mut b = Build::default();
    let (band, _, _) = b.on_torus((15, [70, 30]), true, 0);
    let sheet = b.sheet(2, 25, (-1000, 1000), 1);
    let (out, splitting) = b.run();
    assert!(out.sections.is_empty(), "{:?}", out.sections);
    assert_eq!(splitting.waiting, vec![band, sheet]);
}

/// The band of a torus below height −15, seen along the axis from below, and the plane at height
/// −20 across it: the carriers cross in two circles, of radii 65 and 35 (12.7's closed form for a
/// plane square to a torus's axis), each whole and inside both faces. The band splits into three:
/// the band below the plane between the two circles, and the narrow bands outside the outer one
/// and inside the inner one; the plane into the disc inside the inner circle, the ring between
/// the two and the rest.
#[test]
fn a_plane_across_a_torus_s_band_cuts_it_in_two_circles() {
    let mut b = Build::default();
    // Below the middle plane the outside of the torus is seen from below: the band below −15 lies
    // on the right of its outer circle run counter-clockwise about the third coordinate.
    let (band, outer, inner) = b.on_torus((-15, [70, 30]), false, 0);
    let sheet = b.sheet(2, -20, (-1000, 1000), 1);
    let (out, splitting) = b.run();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    assert_eq!(out.sections.len(), 2, "{:?}", out.sections);
    let mut radii: Vec<f64> = out
        .curves
        .iter()
        .map(|c| match c {
            CurveKind::Circle(c) => {
                let [x, y, z] = c.center();
                assert!(x.abs() < 1e-6 && y.abs() < 1e-6, "{:?}", c.center());
                assert!((z + (20 * S) as f64).abs() < 1e-6, "{:?}", c.center());
                c.radius() / S as f64
            }
            other => panic!("{other:?}"),
        })
        .collect();
    radii.sort_by(f64::total_cmp);
    assert!(
        (radii[0] - 35.0).abs() < 1e-6 && (radii[1] - 65.0).abs() < 1e-6,
        "{radii:?}"
    );
    let parts: Vec<_> = of_face(&splitting, band)
        .into_iter()
        .map(shape_of)
        .collect();
    assert_eq!(parts.len(), 3, "{parts:?}");
    assert!(
        parts.iter().all(|(o, h)| o.len() == 1 && h.len() == 1),
        "{parts:?}"
    );
    let outers: Vec<Piece> = parts.iter().map(|(o, _)| o[0]).collect();
    let holes: Vec<Piece> = parts.iter().map(|(_, h)| h[0][0]).collect();
    assert!(outers.contains(&whole_piece(&out, outer)), "{parts:?}");
    assert!(holes.contains(&whole_piece(&out, inner)), "{parts:?}");
    for s in 0..2 {
        let section = Piece::Section(s);
        assert!(
            outers.contains(&section) && holes.contains(&section),
            "{parts:?}"
        );
    }
    assert_eq!(of_face(&splitting, sheet).len(), 3);
}
