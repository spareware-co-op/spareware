//! The Euler operators case by case through the public interface: a square built and taken apart
//! again, each refusal with the arguments that earn it, bodies the operators cannot make (an edge
//! with no fin, a closed edge with no vertex, a ring of three fins) edited by name, and the
//! editor's own reads.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    clippy::many_single_char_names
)]

mod common;

use common::{ix, line, meter, plane};
use geom_math::Box3;
use geom_num::grid::P3;
use geom_topo::order::{NameKey, Names};
use geom_topo::{
    Arena, Body, BodyKind, Builder, Edge, EdgeIx, Euler, Face, Fin, FinIx, Loop, NewEdge, NewFace,
    NewVertex, Refusal, Region, RegionKind, Sense, Shell, TopoError,
};

fn name(n: u8) -> NameKey {
    NameKey([n; 16])
}

fn face(n: u8) -> NewFace {
    NewFace {
        name: name(n),
        surface: plane([0.0; 3], 0, 1),
        sense: Sense::Forward,
        bound: Box3::new(P3::new(0, 0, 0).unwrap(), P3::new(1, 1, 0).unwrap()).unwrap(),
    }
}

fn edge(n: u8) -> NewEdge {
    NewEdge {
        name: name(n),
        curve: line([0, 0, 0], [1, 0, 0]),
    }
}

fn vertex(n: u8) -> NewVertex {
    NewVertex {
        name: name(n),
        point: P3::new(i64::from(n), 0, 0).unwrap(),
    }
}

fn refused(r: Refusal) -> TopoError {
    TopoError::Euler(r)
}

/// The live fin of the edge named `e` running along it (`forward`) or back.
fn fin(eu: &Euler, e: u8, forward: bool) -> FinIx {
    let a = eu.arenas();
    (0..a.fins.len())
        .filter(|&f| eu.is_live(Arena::Fin, f))
        .map(ix)
        .find(|&f| {
            let x = a.fins.get(f).unwrap();
            eu.names().edges[x.edge.to_usize()] == name(e) && (x.sense == Sense::Forward) == forward
        })
        .unwrap()
}

/// The live edge named `e`.
fn edge_ix(eu: &Euler, e: u8) -> EdgeIx {
    let row = eu.names().edges.iter().position(|&n| n == name(e)).unwrap();
    ix(row as u32)
}

/// Counts of live vertices, edges, faces, loops and shells.
fn counts(eu: &Euler) -> [u32; 5] {
    [
        Arena::Vertex,
        Arena::Edge,
        Arena::Face,
        Arena::Loop,
        Arena::Shell,
    ]
    .map(|arena| {
        (0..eu.arenas().rows(arena))
            .filter(|&r| eu.is_live(arena, r))
            .count() as u32
    })
}

/// A square lamina in a void region named 1: vertices 10 to 13, edges 20 to 23 (2k from vertex
/// 1k to 1k+1, the last back to 10), face 30 on one side and face 31 on the other.
fn square() -> Euler {
    let mut eu = Euler::new();
    let outside = eu.make_region(RegionKind::Void, name(1)).unwrap();
    let w = eu
        .make_wire(outside, face(30), edge(20), vertex(10), vertex(11))
        .unwrap();
    assert_eq!(
        eu.arenas().fins.get(w.fins[0]).unwrap().sense,
        Sense::Forward
    );
    let (e21, v12) = eu.mev(w.fins[1], edge(21), vertex(12)).unwrap();
    assert_eq!(eu.arenas().edges.get(e21).unwrap().end, Some(v12));
    eu.mev(fin(&eu, 21, false), edge(22), vertex(13)).unwrap();
    let (_, f31) = eu
        .mef(fin(&eu, 22, false), fin(&eu, 20, true), edge(23), face(31))
        .unwrap();
    assert_eq!(eu.names().faces[f31.to_usize()], name(31));
    eu
}

fn publish(eu: Euler) -> Body {
    let outside = (0..eu.arenas().regions.len())
        .find(|&r| eu.is_live(Arena::Region, r) && eu.names().regions[r as usize] == name(1))
        .unwrap();
    eu.publish(BodyKind::Sheet, ix(outside), &mut meter())
        .unwrap()
}

#[test]
fn a_square_is_built_and_taken_apart() {
    let mut eu = square();
    assert_eq!(counts(&eu), [4, 4, 2, 2, 1]);
    let body = publish(eu.clone());
    assert_eq!(body.arenas().faces.len(), 2);
    assert_eq!(body.arenas().edges.len(), 4);
    // Edited again by name, the body publishes as itself.
    assert_eq!(publish(Euler::edit(&body).unwrap()), body);
    // Taken apart in the reverse order.
    eu.kef(fin(&eu, 23, false)).unwrap();
    assert_eq!(counts(&eu), [4, 3, 1, 1, 1]);
    eu.kev(edge_ix(&eu, 22)).unwrap();
    eu.kev(edge_ix(&eu, 21)).unwrap();
    assert_eq!(counts(&eu), [2, 1, 1, 1, 1]);
    eu.kill_wire(edge_ix(&eu, 20)).unwrap();
    assert_eq!(counts(&eu), [0, 0, 0, 0, 0]);
    let empty = publish(eu);
    assert_eq!(empty.arenas().regions.len(), 1);
    assert_eq!(empty.arenas().surfaces.len(), 0);
}

#[test]
fn the_same_square_whatever_the_handles() {
    // Built twice, once with a spare wire made first and killed at the end, so every row the
    // square uses sits at another index; both publish the same body.
    let a = publish(square());
    let mut eu = Euler::new();
    let outside = eu.make_region(RegionKind::Void, name(1)).unwrap();
    let spare = eu
        .make_wire(outside, face(90), edge(91), vertex(92), vertex(93))
        .unwrap();
    let w = eu
        .make_wire(outside, face(30), edge(20), vertex(10), vertex(11))
        .unwrap();
    eu.mev(w.fins[1], edge(21), vertex(12)).unwrap();
    eu.mev(fin(&eu, 21, false), edge(22), vertex(13)).unwrap();
    eu.mef(fin(&eu, 22, false), fin(&eu, 20, true), edge(23), face(31))
        .unwrap();
    eu.kill_wire(spare.edge).unwrap();
    assert_eq!(publish(eu), a);
}

#[test]
fn rings_and_holes() {
    let mut eu = square();
    // An edge from vertex 10 to a new vertex 14 and on to 15, then across to 14 closing a face.
    eu.mev(fin(&eu, 20, true), edge(24), vertex(14)).unwrap();
    eu.mev(fin(&eu, 24, false), edge(25), vertex(15)).unwrap();
    // Removing edge 24 cuts the spur 14-15 off as a ring of face 30.
    let ring = eu.kemr(fin(&eu, 24, true)).unwrap();
    assert_eq!(counts(&eu), [6, 5, 2, 3, 1]);
    let lp = eu.arenas().loops.get(ring).unwrap();
    assert_eq!(eu.names().faces[lp.face.to_usize()], name(30));
    // The ring becomes a face of its own, and back.
    let f = eu.mfkrh(ring, face(32)).unwrap();
    assert_eq!(counts(&eu), [6, 5, 3, 3, 1]);
    assert_eq!(
        eu.mfkrh(ring, face(33)),
        Err(refused(Refusal::Faces)),
        "the ring is now its face's only loop"
    );
    eu.kfmrh(ix(0), ring).unwrap();
    assert!(!eu.is_live(Arena::Face, f.row()));
    assert_eq!(counts(&eu), [6, 5, 2, 3, 1]);
    // Joined back by an edge from 10 to 14.
    let joined = eu
        .mekr(fin(&eu, 20, true), fin(&eu, 25, true), edge(26))
        .unwrap();
    assert_eq!(counts(&eu), [6, 6, 2, 2, 1]);
    assert_eq!(eu.names().edges[joined.to_usize()], name(26));
    let body = publish(eu.clone());
    assert_eq!(publish(Euler::edit(&body).unwrap()), body);
}

#[test]
fn a_face_bounded_by_one_closed_edge() {
    let mut eu = square();
    let at = fin(&eu, 21, true);
    let (e, f) = eu.mef(at, at, edge(27), face(34)).unwrap();
    let edge_row = eu.arenas().edges.get(e).unwrap();
    assert_eq!(edge_row.start, edge_row.end);
    assert_eq!(counts(&eu), [4, 5, 3, 3, 1]);
    // The old loop keeps the new edge's forward fin alone; the new face takes the old cycle.
    let g = fin(&eu, 27, true);
    let fg = eu.arenas().fins.get(g).unwrap();
    assert_eq!((fg.next, fg.prev), (g, g));
    let h = fin(&eu, 27, false);
    let lh = eu.arenas().fins.get(h).unwrap().loop_;
    assert_eq!(eu.arenas().loops.get(lh).unwrap().face, f);
    // A closed edge is no spur; nor can both its loops lose their fin.
    assert_eq!(eu.kev(e), Err(refused(Refusal::Shape)));
    eu.kef(h).unwrap();
    assert_eq!(counts(&eu), [4, 4, 2, 2, 1]);
    let body = publish(eu);
    assert_eq!(body.arenas().faces.len(), 2);
}

#[test]
fn kef_joins_a_lone_fin_either_way() {
    // The face made by a closed edge is removed through the closed edge's other fin: the old
    // loop is the edge's fin alone, so the new face's fins take its place.
    let mut eu = square();
    let at = fin(&eu, 21, true);
    let (_, f) = eu.mef(at, at, edge(27), face(34)).unwrap();
    let g = fin(&eu, 27, true);
    eu.kef(g).unwrap();
    assert!(eu.is_live(Arena::Face, f.row()));
    assert_eq!(counts(&eu), [4, 4, 2, 2, 1]);
    let body = publish(eu);
    assert_eq!(body.arenas().fins.len(), 8);
}

#[test]
fn each_refusal_and_its_arguments() {
    let mut eu = square();
    let before = eu.clone();
    let (f20, f20r) = (fin(&eu, 20, true), fin(&eu, 20, false));
    let (f22, f22r) = (fin(&eu, 22, true), fin(&eu, 22, false));
    // mef and mekr need their fins in one loop and in two loops of one face.
    assert_eq!(
        eu.mef(f20, f20r, edge(40), face(41)),
        Err(refused(Refusal::Loops))
    );
    assert_eq!(eu.mekr(f20, f22, edge(40)), Err(refused(Refusal::Loops)));
    assert_eq!(eu.mekr(f20, f20r, edge(40)), Err(refused(Refusal::Loops)));
    // kemr needs the fins in one loop, and not next to each other.
    assert_eq!(eu.kemr(f22), Err(refused(Refusal::Loops)));
    // kef needs fins of two faces, and the face it removes must have only that loop.
    eu.mev(f20, edge(42), vertex(43)).unwrap();
    let spur = fin(&eu, 42, true);
    assert_eq!(eu.kef(spur), Err(refused(Refusal::Faces)));
    assert_eq!(eu.kemr(spur), Err(refused(Refusal::Shape)));
    assert_eq!(eu.kemr(fin(&eu, 42, false)), Err(refused(Refusal::Shape)));
    // kev needs a spur, and kill_wire a lone wire.
    assert_eq!(eu.kev(edge_ix(&eu, 20)), Err(refused(Refusal::Shape)));
    assert_eq!(eu.kill_wire(edge_ix(&eu, 20)), Err(refused(Refusal::Shape)));
    // kfmrh needs two faces of one shell; mfkrh a face that keeps a loop.
    let loop0 = eu.arenas().fins.get(f20).unwrap().loop_;
    let face0 = eu.arenas().loops.get(loop0).unwrap().face;
    assert_eq!(eu.kfmrh(face0, loop0), Err(refused(Refusal::Faces)));
    assert_eq!(eu.mfkrh(loop0, face(44)), Err(refused(Refusal::Faces)));
    eu.kev(edge_ix(&eu, 42)).unwrap();
    assert_eq!(eu, {
        let mut b = before.clone();
        b.mev(f20, edge(42), vertex(43)).unwrap();
        b.kev(edge_ix(&b, 42)).unwrap();
        b
    });
    // A second shell: kfmrh across shells, kill_wire on a wire whose face has a ring.
    let outside = ix(0);
    let w = eu
        .make_wire(outside, face(50), edge(51), vertex(52), vertex(53))
        .unwrap();
    let other_loop = eu.arenas().fins.get(f22r).unwrap().loop_;
    let other_face = eu.arenas().loops.get(other_loop).unwrap().face;
    assert_eq!(eu.kfmrh(other_face, w.loop_), Err(refused(Refusal::Faces)));
    // Regions: in use, dead, moved.
    assert_eq!(eu.kill_region(outside), Err(refused(Refusal::InUse)));
    let solid = eu.make_region(RegionKind::Solid, name(60)).unwrap();
    eu.move_shell(w.shell, solid).unwrap();
    assert_eq!(eu.kill_region(solid), Err(refused(Refusal::InUse)));
    eu.move_shell(w.shell, outside).unwrap();
    eu.kill_region(solid).unwrap();
    let dead = Refusal::Dead {
        arena: Arena::Region,
        row: solid.row(),
    };
    assert_eq!(eu.kill_region(solid), Err(refused(dead)));
    assert_eq!(eu.move_shell(w.shell, solid), Err(refused(dead)));
    assert_eq!(
        eu.make_wire(solid, face(70), edge(71), vertex(72), vertex(73)),
        Err(refused(dead))
    );
    // A face with a ring and a wire of its own: kill_wire refuses while the face has two loops.
    eu.mev(w.fins[0], edge(54), vertex(55)).unwrap();
    eu.mev(fin(&eu, 54, false), edge(56), vertex(57)).unwrap();
    let ring = eu.kemr(fin(&eu, 54, true)).unwrap();
    assert_eq!(eu.kill_wire(edge_ix(&eu, 51)), Err(refused(Refusal::InUse)));
    // Every handle to a dead row is refused, naming it.
    let gone = edge_ix(&eu, 54);
    assert_eq!(
        eu.kev(gone),
        Err(refused(Refusal::Dead {
            arena: Arena::Edge,
            row: gone.row()
        }))
    );
    let new = eu.mfkrh(ring, face(58)).unwrap();
    // The wire's face has one loop again, but its shell now has another face.
    assert_eq!(eu.kill_wire(edge_ix(&eu, 51)), Err(refused(Refusal::InUse)));
    let wire_face = eu.arenas().loops.get(w.loop_).unwrap().face;
    eu.kfmrh(wire_face, ring).unwrap();
    assert_eq!(
        eu.kfmrh(new, ring),
        Err(refused(Refusal::Dead {
            arena: Arena::Face,
            row: new.row()
        }))
    );
    eu.mfkrh(ring, face(59)).unwrap();
}

/// A body the operators cannot make, built by hand: one void region, one shell, one face with four
/// loops. Edge 1 is closed with no vertex, its two fins a loop; edge 2 has no fin; edge 3 one fin
/// alone in a loop, its own radial neighbour; edge 4 three fins in two loops, in one ring.
fn odd_body() -> Body {
    let mut b = Builder::new();
    let a = &mut b.arenas;
    a.regions
        .push(Region {
            kind: RegionKind::Void,
            first_shell: Some(ix(0)),
        })
        .unwrap();
    a.shells
        .push(Shell {
            region: ix(0),
            next: None,
            first_face: Some(ix(0)),
        })
        .unwrap();
    a.surfaces.push(plane([0.0; 3], 0, 1)).unwrap();
    a.faces
        .push(Face {
            shell: ix(0),
            next: None,
            surface: ix(0),
            sense: Sense::Forward,
            first_loop: Some(ix(0)),
            bound: Box3::new(P3::new(0, 0, 0).unwrap(), P3::new(1, 1, 0).unwrap()).unwrap(),
        })
        .unwrap();
    for first in [0, 2, 3, 5] {
        a.loops
            .push(Loop {
                face: ix(0),
                next: None,
                first_fin: Some(ix(first)),
            })
            .unwrap();
    }
    let fin = |l: u32, next: u32, prev: u32, e: u32, s: Sense, radial: u32| Fin {
        loop_: ix(l),
        next: ix(next),
        prev: ix(prev),
        edge: ix(e),
        sense: s,
        radial: ix(radial),
        pcurve: None,
    };
    a.fins
        .extend(&[
            fin(0, 1, 1, 0, Sense::Forward, 1),
            fin(0, 0, 0, 0, Sense::Reversed, 0),
            fin(1, 2, 2, 2, Sense::Forward, 2),
            fin(2, 4, 4, 3, Sense::Forward, 4),
            fin(2, 3, 3, 3, Sense::Reversed, 5),
            fin(3, 5, 5, 3, Sense::Reversed, 3),
        ])
        .unwrap();
    for (first, curve) in [(Some(0), 0), (None, 1), (Some(2), 2), (Some(3), 3)] {
        a.curves.push(line([0, 0, 0], [1, 0, 0])).unwrap();
        a.edges
            .push(Edge {
                first_fin: first.map(ix),
                start: None,
                end: None,
                curve: ix(curve),
            })
            .unwrap();
    }
    let names = Names {
        regions: vec![name(1)],
        faces: vec![name(2)],
        edges: vec![name(3), name(4), name(5), name(6)],
        vertices: vec![],
    };
    b.publish_named(&names, BodyKind::General, ix(0), &mut meter())
        .unwrap()
}

#[test]
fn bodies_the_operators_cannot_make_are_refused_or_edited() {
    let body = odd_body();
    let mut eu = Euler::edit(&body).unwrap();
    let e = |eu: &Euler, n: u8| edge_ix(eu, n);
    // An edge with no fin, one alone, and a ring of three have no pair of fins.
    for n in [4, 5, 6] {
        assert_eq!(eu.kev(e(&eu, n)), Err(refused(Refusal::Ring)), "edge {n}");
    }
    // A closed edge with no vertex: nothing starts there.
    let closed = eu.arenas().edges.get(e(&eu, 3)).unwrap().first_fin.unwrap();
    assert_eq!(
        eu.mev(closed, edge(10), vertex(11)),
        Err(refused(Refusal::Shape))
    );
    assert_eq!(eu.kill_wire(e(&eu, 3)), Err(refused(Refusal::InUse)));
    // Unnamed bodies cannot be edited by name.
    let unnamed = Builder::edit(&body)
        .publish(BodyKind::General, body.outside(), &mut meter())
        .unwrap();
    assert_eq!(Euler::edit(&unnamed), Err(refused(Refusal::Unnamed)));
    // Publishing with an outside region that is not live.
    let outside = body.outside();
    let mut gone = Euler::edit(&body).unwrap();
    let r = gone.make_region(RegionKind::Void, name(9)).unwrap();
    gone.kill_region(r).unwrap();
    assert!(matches!(
        gone.publish(BodyKind::General, r, &mut meter()),
        Err(TopoError::NoSuchRow {
            arena: Arena::Region,
            ..
        })
    ));
    // The odd body round-trips.
    assert_eq!(
        Euler::edit(&body)
            .unwrap()
            .publish(BodyKind::General, outside, &mut meter())
            .unwrap(),
        body
    );
    // A pool row is never live in the editor's sense; nor is a row past the end.
    for arena in [Arena::Surface, Arena::Curve, Arena::Pcurve, Arena::Point] {
        assert!(!eu.is_live(arena, 0));
    }
    assert!(!eu.is_live(Arena::Fin, 99));
    assert_eq!(eu.names(), body.names().unwrap());
    eu.make_region(RegionKind::Solid, name(8)).unwrap();
}

#[test]
fn refusals_read_plainly() {
    let cases = [
        (
            Refusal::Dead {
                arena: Arena::Fin,
                row: 3,
            },
            "fin 3 is not a live row",
        ),
        (
            Refusal::Loops,
            "the fins are not in the loops the operator needs",
        ),
        (Refusal::Ring, "the edge does not have exactly two fins"),
        (
            Refusal::Shape,
            "the topology is not the shape the operator works on",
        ),
        (Refusal::Faces, "the faces are not what the operator needs"),
        (
            Refusal::InUse,
            "the region or shell still holds other entities",
        ),
        (Refusal::Unnamed, "the body has no names to edit it by"),
    ];
    for (r, text) in cases {
        assert_eq!(r.to_string(), text);
        assert_eq!(
            TopoError::Euler(r).to_string(),
            format!("the Euler operator refused: {text}")
        );
    }
}
