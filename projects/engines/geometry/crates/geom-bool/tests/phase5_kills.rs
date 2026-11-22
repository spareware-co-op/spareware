//! Two answers of the boolean's arrangement held where the Phase 5 mutation judge found nothing
//! tested them: the contact rule on lines far from the origin, whose pieces' balls must be centred
//! on the pieces themselves (G243, chapter 11.4 step 3), and a plane through a cylinder's axis,
//! which meets it in rulings on the edges of the cylinder's box, so the plane made a surface for
//! the closed forms must reach past that box (G244, chapter 11.4 step 5).

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
use geom_bool::common::Proof;
use geom_bool::interfere::{Arrangement, EdgeIn, EdgeUse, FaceIn, Interfered, VertexIn, interfere};
use geom_bool::merge::default_remaining;
use geom_bool::split::split;
use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_num::P3;
use geom_num::plane::Plane;
use geom_run::{Budget, Exec, WorkMeter};
use geom_surface::Cylinder;
use geom_topo::SurfaceKind;

fn meter(work: u64) -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(work))
}

/// The height, in steps, the lines of the first test stand at: far from the origin, on the grid.
const HIGH: i64 = 4096;

/// The edge from (0, 0, `HIGH`) to (10,496, 0, `HIGH`) of operand 0, and an edge of operand 1
/// between the same two points along the line 20.5 steps above it in y: the case of
/// `common_blocks.rs`'s first test, moved up by a whole number of steps, so every piece's box,
/// its ball's grid-aligned centre and the distances are as they are there, exactly.
fn lines_high_up() -> Interfered {
    let length = 41 * 256;
    let mut arr = Arrangement::default();
    for (operand, y) in [(0_u32, 0.0), (1, 20.5)] {
        let mut end = |x: i64| {
            arr.vertices.push(VertexIn {
                point: PointGeom::Grid(P3::new(x, 0, HIGH).unwrap()),
                operand,
            });
            arr.vertices.len() - 1
        };
        let ends = [end(0), end(length)];
        let line = Line::new([0.0, y, HIGH as f64], [length as f64, 0.0, 0.0], (0.0, 1.0)).unwrap();
        let curve = arr.carriers.add_curve(CurveKind::Line(line)).unwrap();
        arr.edges.push(EdgeIn {
            curve,
            ends: Some(ends),
            operand,
        });
    }
    interfere(
        &arr,
        default_remaining(),
        &Exec::serial(),
        &mut meter(1 << 30),
    )
    .unwrap()
}

/// The two edges of `lines_high_up`, 4,096 steps from the origin: at the finest refinement each
/// piece is 41 steps long, its ball centred on the grid point a half step to one side of its
/// middle, of radius 21 steps, and the other line lies within 21 steps of every piece and within
/// 41.5 steps of every point of every ball, below the contact radius of 64. So the two are one
/// block whose proof reaches 41.5, as on the axis. A ball centred anywhere but on its piece, the
/// origin say, would stand 4,096 steps from the other line and prove the first piece apart at
/// once: no block, found in a moment.
#[test]
fn lines_far_from_the_origin_are_one_block_by_balls_on_their_pieces() {
    let out = lines_high_up();
    assert_eq!(out.common.len(), 1, "{:?}", out.common);
    assert_eq!(out.common[0].members.len(), 2, "{:?}", out.common);
    let Proof::Contact(d) = out.common[0].proof else {
        panic!("{:?}", out.common[0].proof);
    };
    assert!(41.4 < d.hi() && d.hi() < 41.6, "{d:?}");
}

/// The tests' unit, in steps: a millimetre on a grid of a micrometre.
const S: i64 = 1000;

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x * S, y * S, z * S).unwrap()
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

    fn face(&mut self, surface: CarrierIx, loops: Vec<Vec<EdgeUse>>, operand: u32) -> usize {
        self.arr.faces.push(FaceIn {
            surface,
            loops,
            operand,
        });
        self.arr.faces.len() - 1
    }

    /// The square on the plane x = `at` from −1000 to 1000 in y and z (the tests' unit),
    /// counter-clockwise seen from where x is larger.
    fn wall(&mut self, at: i64, operand: u32) -> usize {
        let corners: Vec<usize> = [(-1000, -1000), (1000, -1000), (1000, 1000), (-1000, 1000)]
            .iter()
            .map(|&(y, z)| self.vertex(p(at, y, z), operand))
            .collect();
        let edges: Vec<usize> = (0..4)
            .map(|k| self.line(corners[k], corners[(k + 1) % 4], operand))
            .collect();
        let plane = CarrierIx::Plane(
            self.arr
                .carriers
                .add_plane(&Plane::new([1, 0, 0], -i128::from(at * S)).unwrap())
                .unwrap(),
        );
        // Each edge runs on from the end it shares with the one before it: forward, as listed.
        let uses = edges
            .into_iter()
            .map(|edge| EdgeUse {
                edge,
                forward: true,
            })
            .collect();
        self.face(plane, vec![uses], operand)
    }

    /// The side of a cylinder of radius `r` about the z axis from z = 0 to z = `to` (the tests'
    /// unit): the face, between its whole circle at 0, run forward, and the one at `to`, run back.
    fn tube(&mut self, r: i64, to: i64, operand: u32) -> usize {
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let mut rim = |h: i64| {
            let circle =
                Circle::new(coords(p(0, 0, h)), (r * S) as f64, frame, (0.0, 2.0)).unwrap();
            self.edge(CurveKind::Circle(circle), None, operand)
        };
        let (first, second) = (rim(0), rim(to));
        let cylinder = Cylinder::new(
            [0.0; 3],
            (r * S) as f64,
            frame,
            ((0.0, 2.0), (0.0, (to * S) as f64)),
        )
        .unwrap();
        let ix = CarrierIx::Surface(
            self.arr
                .carriers
                .add_surface(SurfaceKind::Cylinder(cylinder))
                .unwrap(),
        );
        let ahead = EdgeUse {
            edge: first,
            forward: true,
        };
        let back = EdgeUse {
            edge: second,
            forward: false,
        };
        self.face(ix, vec![vec![ahead], vec![back]], operand)
    }
}

/// A cylinder's side of radius 50 about the z axis from z = 0 to 100, and the plane x = 0 through
/// its axis: they cross in the two rulings x = 0, y = ±50, each from the rim at z = 0 to the rim at
/// z = 100, where the rims cross the plane at four grid vertices In it. The rulings stand on the
/// edges of the cylinder's box in y, from −50 to 50 about the plane's own axis, so the plane made a
/// surface for the closed forms must reach past that box by a share of its width, on both sides;
/// one that reached short of it, by a share of the ratio of its ends, would hold neither ruling.
/// Each ruling is one section between two of those vertices, and nothing waits.
#[test]
fn a_plane_through_a_cylinder_s_axis_cuts_it_in_the_rulings_on_its_box() {
    let mut b = Build::default();
    let side = b.tube(50, 100, 0);
    let wall = b.wall(0, 1);
    let out = interfere(
        &b.arr,
        default_remaining(),
        &Exec::serial(),
        &mut meter(1 << 22),
    )
    .unwrap();
    assert_eq!(out.sections.len(), 2, "{:?}", out.sections);
    let mut ends = Vec::new();
    for (k, section) in out.sections.iter().enumerate() {
        assert_eq!(section.faces, [side, wall]);
        assert!(
            matches!(out.curves[k], CurveKind::Line(_)),
            "{:?}",
            out.curves[k]
        );
        let at = |v: usize| b.arr.carriers.commit(&out.vertices[v]).unwrap().0;
        let (a, c) = (
            at(section.from.unwrap().vertex),
            at(section.to.unwrap().vertex),
        );
        let mut zs = [a.z.get(), c.z.get()];
        zs.sort_unstable();
        assert_eq!(zs, [0, 100 * S], "{k}: {a:?} {c:?}");
        assert_eq!((a.x.get(), a.y.get()), (c.x.get(), c.y.get()), "{k}");
        ends.push((a.x.get(), a.y.get()));
    }
    ends.sort_unstable();
    assert_eq!(ends, [(0, -50 * S), (0, 50 * S)]);
    assert_eq!(
        out.faces[wall].in_vertices.len(),
        4,
        "{:?}",
        out.faces[wall]
    );
    let splitting = split(&b.arr, &out, &mut meter(1 << 22)).unwrap();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
}
