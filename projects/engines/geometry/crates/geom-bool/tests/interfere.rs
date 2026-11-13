//! Interferences, paves and common blocks (G243, chapter 11.4 steps 2 and 3): vertices merged
//! first, vertices put on the edges they touch, edges met with the vertices already found reused,
//! paves in a proven order, edges split with their history, and common blocks exact within one
//! analytic kind and by the contact rule across kinds.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    reason = "test code: a failure should stop loudly"
)]

use geom_bool::carrier::{Bound, PointGeom};
use geom_bool::common::Proof;
use geom_bool::interfere::{
    Arrangement, BuildError, EdgeIn, Interfered, Interference, VertexIn, interfere,
};
use geom_bool::merge::default_remaining;
use geom_bool::pave::{History, OrderError, Pave, blocks, order, split};
use geom_curve::{Circle, CurveKind, Frame, Line, ParamCurve};
use geom_num::P3;
use geom_num::interval::Interval;
use geom_run::{Budget, Exec, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 30))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn coords(q: P3) -> [f64; 3] {
    [q.x.get() as f64, q.y.get() as f64, q.z.get() as f64]
}

/// Builds an arrangement edge by edge.
struct Build {
    arr: Arrangement,
}

impl Build {
    fn new() -> Self {
        Build {
            arr: Arrangement::default(),
        }
    }

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

    /// A straight edge from `a` to `b`, parameter 0 at `a` and 1 at `b`, with its two vertices.
    fn segment(&mut self, a: P3, b: P3, operand: u32) -> usize {
        let (o, e) = (coords(a), coords(b));
        let d = [e[0] - o[0], e[1] - o[1], e[2] - o[2]];
        let line = Line::new(o, d, (0.0, 1.0)).unwrap();
        let (va, vb) = (self.vertex(a, operand), self.vertex(b, operand));
        self.edge(CurveKind::Line(line), Some([va, vb]), operand)
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

/// The merged vertex at grid point `at`.
fn vertex_at(out: &Interfered, at: P3) -> usize {
    out.vertices
        .iter()
        .position(|v| match v {
            PointGeom::Grid(q) | PointGeom::Certified { at: q, .. } => *q == at,
            PointGeom::Planes(_) => false,
        })
        .unwrap_or_else(|| panic!("no vertex at {at:?}: {:?}", out.vertices))
}

fn pave_vertices(out: &Interfered, edge: usize) -> Vec<usize> {
    out.paves[edge].iter().map(|p| p.vertex).collect()
}

#[test]
fn collinear_wall_edges_form_one_exact_common_block() {
    // Two walls' bottom edges on the x axis, overlapping from 400 to 1000.
    let mut b = Build::new();
    let a = b.segment(p(0, 0, 0), p(1000, 0, 0), 0);
    let c = b.segment(p(400, 0, 0), p(1400, 0, 0), 1);
    let out = b.run().unwrap();
    assert_eq!(out.vertices.len(), 4, "{:?}", out.vertices);
    let (v0, v1, v2, v3) = (
        vertex_at(&out, p(0, 0, 0)),
        vertex_at(&out, p(1000, 0, 0)),
        vertex_at(&out, p(400, 0, 0)),
        vertex_at(&out, p(1400, 0, 0)),
    );
    // Each edge carries the other's end that lies on it, in order.
    assert_eq!(pave_vertices(&out, a), [v0, v2, v1]);
    assert_eq!(pave_vertices(&out, c), [v2, v1, v3]);
    assert!(out.interferences.contains(&Interference::VertexEdge {
        vertex: v2,
        edge: a
    }));
    assert!(out.interferences.contains(&Interference::VertexEdge {
        vertex: v1,
        edge: c
    }));
    // Both edges are split in two, and the shared stretch from 400 to 1000 is one common block,
    // proven exactly.
    assert_eq!(out.splits.len(), 4);
    assert!(out.splits.iter().all(|s| s.history == History::Split));
    assert_eq!(out.common.len(), 1, "{:?}", out.common);
    let block = &out.common[0];
    assert_eq!(block.proof, Proof::Exact);
    let mut sources: Vec<usize> = block
        .members
        .iter()
        .map(|m| out.splits[*m].source)
        .collect();
    sources.sort_unstable();
    assert_eq!(sources, [a, c]);
    for m in &block.members {
        assert_eq!(out.splits[*m].block.ends(), Some((v1.min(v2), v1.max(v2))));
    }
}

#[test]
fn cylinders_sharing_a_line_form_one_common_block() {
    // Two cylinders' straight boundary edges on the line x = 100, y = 0: one runs up from z = 0 to
    // 500 at unit speed, the other down from z = 800 to 200 at twice that speed.
    let mut b = Build::new();
    let (s0, s1) = (b.vertex(p(100, 0, 0), 0), b.vertex(p(100, 0, 500), 0));
    let up = b.edge(
        CurveKind::Line(Line::new([100.0, 0.0, 0.0], [0.0, 0.0, 1.0], (0.0, 500.0)).unwrap()),
        Some([s0, s1]),
        0,
    );
    let (t0, t1) = (b.vertex(p(100, 0, 800), 1), b.vertex(p(100, 0, 200), 1));
    let down = b.edge(
        CurveKind::Line(Line::new([100.0, 0.0, 800.0], [0.0, 0.0, -2.0], (0.0, 300.0)).unwrap()),
        Some([t0, t1]),
        1,
    );
    let out = b.run().unwrap();
    let (z200, z500) = (
        vertex_at(&out, p(100, 0, 200)),
        vertex_at(&out, p(100, 0, 500)),
    );
    assert_eq!(pave_vertices(&out, up)[1], z200);
    assert_eq!(pave_vertices(&out, down)[1], z500);
    // The second pave of the falling edge is at its parameter 150, where it reaches z = 500.
    let t = out.paves[down][1].t;
    assert!(t.lo() <= 150.0 && 150.0 <= t.hi(), "{t:?}");
    assert_eq!(out.common.len(), 1, "{:?}", out.common);
    assert_eq!(out.common[0].proof, Proof::Exact);
}

#[test]
fn an_arc_and_a_nurbs_arc_form_one_common_block_by_the_contact_rule() {
    // A quarter circle of radius 1000, and the same quarter as a rational NURBS curve, from two
    // operands with their own vertices at its ends.
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let arc = Circle::new([0.0, 0.0, 0.0], 1000.0, frame, (0.0, 0.5)).unwrap();
    let nurbs = CurveKind::Circle(arc.clone())
        .to_nurbs(&mut meter())
        .unwrap()
        .value
        .curve;
    let mut b = Build::new();
    let (a0, a1) = (b.vertex(p(1000, 0, 0), 0), b.vertex(p(0, 1000, 0), 0));
    let first = b.edge(CurveKind::Circle(arc), Some([a0, a1]), 0);
    let (b0, b1) = (b.vertex(p(1000, 0, 0), 1), b.vertex(p(0, 1000, 0), 1));
    let second = b.edge(CurveKind::Nurbs(nurbs), Some([b0, b1]), 1);
    let out = b.run().unwrap();
    // The ends merge, each edge stays whole, and the two are one block by the contact rule.
    assert_eq!(out.vertices.len(), 2, "{:?}", out.vertices);
    assert_eq!(out.merged[a0], out.merged[b0]);
    assert_eq!(out.merged[a1], out.merged[b1]);
    assert!(out.splits.iter().all(|s| s.history == History::Unchanged));
    assert_eq!(out.common.len(), 1, "{:?}", out.common);
    let Proof::Contact(d) = out.common[0].proof else {
        panic!("{:?}", out.common[0].proof);
    };
    assert!(d.hi() < 64.0, "{d:?}");
    let mut sources: Vec<usize> = out.common[0]
        .members
        .iter()
        .map(|m| out.splits[*m].source)
        .collect();
    sources.sort_unstable();
    assert_eq!(sources, [first, second]);
}

#[test]
fn an_arc_and_a_chord_between_the_same_vertices_are_not_one() {
    // The quarter circle and the straight chord between its ends share both vertices and nothing
    // else: two blocks, no common block.
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let arc = Circle::new([0.0, 0.0, 0.0], 1000.0, frame, (0.0, 0.5)).unwrap();
    let mut b = Build::new();
    let (a0, a1) = (b.vertex(p(1000, 0, 0), 0), b.vertex(p(0, 1000, 0), 0));
    b.edge(CurveKind::Circle(arc), Some([a0, a1]), 0);
    b.segment(p(1000, 0, 0), p(0, 1000, 0), 1);
    let out = b.run().unwrap();
    assert_eq!(out.vertices.len(), 2);
    assert!(out.common.is_empty(), "{:?}", out.common);
}

#[test]
fn a_meeting_through_an_existing_corner_ends_at_that_corner() {
    // Operand 0 has a corner at (500, 500, 0) between two edges; operand 1's diagonal passes
    // through it. The diagonal takes the corner as a pave, and both crossings at the corner reuse
    // it: no second vertex is made nearby.
    let mut b = Build::new();
    let corner = b.vertex(p(500, 500, 0), 0);
    let left = b.vertex(p(0, 500, 0), 0);
    let top = b.vertex(p(500, 1000, 0), 0);
    let line = |a: P3, c: P3| {
        let (o, e) = (coords(a), coords(c));
        CurveKind::Line(Line::new(o, [e[0] - o[0], e[1] - o[1], e[2] - o[2]], (0.0, 1.0)).unwrap())
    };
    let e1 = b.edge(line(p(0, 500, 0), p(500, 500, 0)), Some([left, corner]), 0);
    let e2 = b.edge(
        line(p(500, 500, 0), p(500, 1000, 0)),
        Some([corner, top]),
        0,
    );
    let diagonal = b.segment(p(0, 0, 0), p(1000, 1000, 0), 1);
    let out = b.run().unwrap();
    assert_eq!(out.vertices.len(), 5, "{:?}", out.vertices);
    let c = vertex_at(&out, p(500, 500, 0));
    assert_eq!(pave_vertices(&out, diagonal)[1], c);
    assert_eq!(pave_vertices(&out, diagonal).len(), 3);
    for edge in [e1, e2] {
        assert!(
            out.interferences.iter().any(|i| matches!(
                i,
                Interference::EdgeEdge { first, second, vertex, reused: true }
                    if *first == edge && *second == diagonal && *vertex == c
            )),
            "{:?}",
            out.interferences
        );
    }
    // The corner's own edges are not cut: it is their end.
    assert_eq!(pave_vertices(&out, e1).len(), 2);
    assert_eq!(pave_vertices(&out, e2).len(), 2);
}

#[test]
fn crossing_edges_make_one_certified_vertex() {
    let mut b = Build::new();
    let a = b.segment(p(0, 0, 0), p(1000, 0, 0), 0);
    let c = b.segment(p(500, -500, 0), p(500, 500, 0), 1);
    let out = b.run().unwrap();
    assert_eq!(out.vertices.len(), 5);
    let x = vertex_at(&out, p(500, 0, 0));
    let PointGeom::Certified { within, on, .. } = &out.vertices[x] else {
        panic!("{:?}", out.vertices[x]);
    };
    assert_eq!(on.len(), 2);
    assert!(*within <= Bound(geom_num::Steps(1)), "{within:?}");
    assert_eq!(pave_vertices(&out, a)[1], x);
    assert_eq!(pave_vertices(&out, c)[1], x);
    assert_eq!(out.splits.len(), 4);
    assert_eq!(out.common, []);
}

#[test]
fn edges_proven_apart_do_not_meet() {
    // Parallel edges one step apart are different edges, however close.
    let mut b = Build::new();
    b.segment(p(0, 0, 0), p(1000, 0, 0), 0);
    b.segment(p(0, 1, 0), p(1000, 1, 0), 1);
    let out = b.run();
    // One step is inside the contact radius and cannot be proven zero either way only if the
    // enclosures are wide; a line's distance is exact, so the vertices of one are proven apart
    // from the other's edge.
    let out = out.unwrap();
    assert_eq!(out.vertices.len(), 4);
    assert_eq!(out.common, []);
    assert!(out.splits.iter().all(|s| s.history == History::Unchanged));
}

#[test]
fn paves_of_two_vertices_whose_parameters_meet_are_refused() {
    let iv = |a: f64, b: f64| Interval::new(a, b).unwrap();
    let got = order(
        7,
        vec![
            Pave {
                vertex: 1,
                t: iv(0.5, 0.6),
            },
            Pave {
                vertex: 2,
                t: iv(0.59, 0.7),
            },
        ],
    );
    let Err(OrderError::Undecided(e)) = got else {
        panic!("{got:?}");
    };
    assert_eq!((e.edge, e.first.vertex, e.second.vertex), (7, 1, 2));
    // One vertex met twice at enclosures that meet is one pave, at their meet.
    let one = order(
        0,
        vec![
            Pave {
                vertex: 3,
                t: iv(0.2, 0.4),
            },
            Pave {
                vertex: 3,
                t: iv(0.3, 0.5),
            },
        ],
    )
    .unwrap();
    assert_eq!(
        one,
        [Pave {
            vertex: 3,
            t: iv(0.3, 0.4)
        }]
    );
    // The order does not depend on the order given.
    let a = order(
        0,
        vec![
            Pave {
                vertex: 5,
                t: iv(0.9, 0.9),
            },
            Pave {
                vertex: 4,
                t: iv(0.1, 0.1),
            },
        ],
    );
    let b = order(
        0,
        vec![
            Pave {
                vertex: 4,
                t: iv(0.1, 0.1),
            },
            Pave {
                vertex: 5,
                t: iv(0.9, 0.9),
            },
        ],
    );
    assert_eq!(a.unwrap(), b.unwrap());
}

#[test]
fn a_closed_edge_wraps_round_and_keeps_its_history() {
    let iv = |a: f64| Interval::point(a).unwrap();
    let paves = [
        Pave {
            vertex: 0,
            t: iv(0.2),
        },
        Pave {
            vertex: 1,
            t: iv(0.9),
        },
    ];
    let cut = blocks(3, &paves, true);
    assert_eq!(cut.len(), 2);
    assert!(!cut[0].wraps && cut[1].wraps);
    assert_eq!(
        (cut[1].from.unwrap().vertex, cut[1].to.unwrap().vertex),
        (1, 0)
    );
    // A closed edge with no pave is one block with no ends, unchanged.
    let whole = split(3, &[], true);
    assert_eq!(whole.len(), 1);
    assert_eq!(whole[0].history, History::Unchanged);
    assert_eq!(whole[0].block.ends(), None);
    // With one pave it is one block round from it to it, and split.
    let once = split(3, &paves[..1], true);
    assert_eq!(once.len(), 1);
    assert_eq!(once[0].history, History::Split);
    assert!(once[0].block.wraps);
}
