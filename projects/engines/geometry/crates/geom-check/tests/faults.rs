//! Fault injection (chapter 28.4): valid bodies broken on purpose, one fault at a time, and the
//! checker must report each with the right kind at the right entity. Every [`FaultKind`] is seeded
//! here at least once: a flipped face, an open loop, a fin off its ring, a wrongly placed shell,
//! a closed edge sharing a loop, an exact record on curved geometry, and the rest.
//!
//! The cube's rows, as `common::cube` builds them: region 0 is the outside void, region 1 the
//! solid with shell 0; faces 0 to 5 in `CUBE_FACES` order, face `i` with loop `i` of fins `4i` to
//! `4i + 3`; edge 0 joins vertices 0 and 2 and has fins 0 (face 0) and 19 (face 4).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly; rows and faults named as in the builder"
)]

mod common;

use common::{
    CUBE, CUBE_FACES, Rows, acorn, circle, cube, faces_into, faults, has, ix, kinds, pcircle,
    polyhedron, report, sphere, square_sheet, wire,
};
use geom_check::{Entity, Fault, FaultKind, Group, MAX_FAULTS};
use geom_num::Exactness;
use geom_topo::{BodyKind, Fin, RegionKind, Sense};

/// A cube's faults after `break_it`, as a solid.
fn broken_cube(break_it: impl FnOnce(&mut Rows)) -> Vec<Fault> {
    let mut r = cube();
    break_it(&mut r);
    faults(&r, BodyKind::Solid)
}

/// Whether every fault is one of the first stage's (lists, rings and edge ends).
fn first_stage_only(faults: &[Fault]) -> bool {
    faults.iter().all(|f| {
        matches!(
            f.kind,
            FaultKind::ChainCycle
                | FaultKind::ListedTwice
                | FaultKind::WrongParent
                | FaultKind::Orphan
                | FaultKind::EmptyLoop
                | FaultKind::FinRing
                | FaultKind::FinPrev
                | FaultKind::RadialRing
                | FaultKind::RadialEdge
                | FaultKind::OffRadialRing
                | FaultKind::EdgeEnds
        )
    })
}

#[test]
fn a_list_that_comes_back_on_itself() {
    let f = broken_cube(|r| r.shells[0].next = Some(ix(0)));
    assert_eq!(f, [Fault::at(FaultKind::ChainCycle, Entity::Region(ix(1)))]);
}

#[test]
fn a_child_listed_twice() {
    let f = broken_cube(|r| {
        let s = r.shell(1);
        r.shells[s as usize].first_face = Some(ix(3));
    });
    assert_eq!(
        f,
        [Fault::at(FaultKind::ListedTwice, Entity::Face(ix(3))).with(Entity::Shell(ix(1)))]
    );
}

#[test]
fn every_list_is_walked_the_same_way() {
    use geom_topo::Shell;
    // A region's shells.
    let f = broken_cube(|r| {
        let extra = r.region(RegionKind::Solid);
        r.regions[extra as usize].first_shell = Some(ix(0));
    });
    assert_eq!(
        f,
        [Fault::at(FaultKind::ListedTwice, Entity::Shell(ix(0))).with(Entity::Region(ix(2)))]
    );
    let f = broken_cube(|r| r.shells[0].region = ix(0));
    assert_eq!(
        f,
        [Fault::at(FaultKind::WrongParent, Entity::Shell(ix(0))).with(Entity::Region(ix(1)))]
    );
    let f = broken_cube(|r| {
        r.shells.push(Shell {
            region: ix(1),
            next: None,
            first_face: None,
        });
    });
    assert_eq!(f, [Fault::at(FaultKind::Orphan, Entity::Shell(ix(1)))]);
    // A shell's faces.
    let f = broken_cube(|r| r.faces[5].next = Some(ix(0)));
    assert_eq!(f, [Fault::at(FaultKind::ChainCycle, Entity::Shell(ix(0)))]);
    let f = broken_cube(|r| {
        let s = r.shell(1);
        r.faces[1].shell = ix(s);
    });
    assert_eq!(
        f,
        [Fault::at(FaultKind::WrongParent, Entity::Face(ix(1))).with(Entity::Shell(ix(0)))]
    );
    // A face's loops.
    let f = broken_cube(|r| r.loops[0].next = Some(ix(0)));
    assert_eq!(f, [Fault::at(FaultKind::ChainCycle, Entity::Face(ix(0)))]);
    let f = broken_cube(|r| r.faces[1].first_loop = Some(ix(0)));
    assert_eq!(
        f,
        [
            Fault::at(FaultKind::ListedTwice, Entity::Loop(ix(0))).with(Entity::Face(ix(1))),
            Fault::at(FaultKind::Orphan, Entity::Loop(ix(1)))
        ]
    );
    let f = broken_cube(|r| r.faces[0].first_loop = None);
    assert_eq!(f, [Fault::at(FaultKind::Orphan, Entity::Loop(ix(0)))]);
}

#[test]
fn a_child_naming_the_wrong_parent() {
    let f = broken_cube(|r| r.loops[2].face = ix(3));
    assert_eq!(
        f,
        [Fault::at(FaultKind::WrongParent, Entity::Loop(ix(2))).with(Entity::Face(ix(2)))]
    );
    // A fin naming the wrong loop.
    let f = broken_cube(|r| r.fins[1].loop_ = ix(1));
    assert_eq!(
        f,
        [Fault::at(FaultKind::WrongParent, Entity::Fin(ix(1))).with(Entity::Loop(ix(0)))]
    );
}

#[test]
fn a_child_no_parent_lists() {
    let f = broken_cube(|r| r.faces[4].next = None);
    assert_eq!(f, [Fault::at(FaultKind::Orphan, Entity::Face(ix(5)))]);
}

#[test]
fn a_loop_without_fins() {
    let f = broken_cube(|r| r.loops[0].first_fin = None);
    assert_eq!(f[0], Fault::at(FaultKind::EmptyLoop, Entity::Loop(ix(0))));
    for fin in 0..4 {
        assert!(has(&f, FaultKind::Orphan, Entity::Fin(ix(fin))), "{f:?}");
    }
    assert!(first_stage_only(&f));
}

#[test]
fn a_ring_of_fins_that_does_not_close() {
    let f = broken_cube(|r| r.fins[1].next = ix(1));
    assert!(has(&f, FaultKind::FinRing, Entity::Loop(ix(0))), "{f:?}");
    assert!(
        f.contains(&Fault::at(FaultKind::FinRing, Entity::Loop(ix(0))).with(Entity::Fin(ix(1))))
    );
    assert!(has(&f, FaultKind::FinPrev, Entity::Fin(ix(1))));
    assert!(first_stage_only(&f));
    // A fin's next fin in another loop: the other loop already holds it.
    let f = broken_cube(|r| r.fins[1].next = ix(4));
    assert!(
        f.contains(
            &Fault::at(FaultKind::ListedTwice, Entity::Fin(ix(4))).with(Entity::Loop(ix(1)))
        ),
        "{f:?}"
    );
}

#[test]
fn a_fin_that_is_not_its_next_fins_previous() {
    let f = broken_cube(|r| r.fins[2].prev = ix(3));
    assert_eq!(f, [Fault::at(FaultKind::FinPrev, Entity::Fin(ix(1)))]);
}

#[test]
fn a_radial_ring_that_does_not_close() {
    let f = broken_cube(|r| r.fins[19].radial = ix(19));
    assert_eq!(
        f,
        [Fault::at(FaultKind::RadialRing, Entity::Edge(ix(0))).with(Entity::Fin(ix(19)))]
    );
}

#[test]
fn a_fin_on_another_edges_ring() {
    let f = broken_cube(|r| r.fins[19].edge = ix(1));
    assert_eq!(
        f,
        [Fault::at(FaultKind::RadialEdge, Entity::Edge(ix(0))).with(Entity::Fin(ix(19)))]
    );
}

#[test]
fn a_fin_off_its_edges_ring() {
    let f = broken_cube(|r| {
        r.fins[0].radial = ix(0);
        r.fins[19].radial = ix(19);
    });
    assert_eq!(
        f,
        [Fault::at(FaultKind::OffRadialRing, Entity::Fin(ix(19)))]
    );
}

#[test]
fn an_edge_with_one_end() {
    let f = broken_cube(|r| r.edges[0].end = None);
    assert_eq!(f, [Fault::at(FaultKind::EdgeEnds, Entity::Edge(ix(0)))]);
    let f = broken_cube(|r| r.edges[0].start = None);
    assert_eq!(f, [Fault::at(FaultKind::EdgeEnds, Entity::Edge(ix(0)))]);
}

#[test]
fn the_second_stage_waits_for_sound_structure() {
    // A cube as a wire body has faces it cannot hold, but its broken edge comes first, alone.
    let mut r = cube();
    r.edges[0].end = None;
    assert_eq!(kinds(&faults(&r, BodyKind::Wire)), [FaultKind::EdgeEnds]);
}

#[test]
fn what_a_body_kind_cannot_hold() {
    // Faces in a wire, and its solid region.
    let f = faults(&cube(), BodyKind::Wire);
    for face in 0..6 {
        assert!(
            has(&f, FaultKind::Misplaced, Entity::Face(ix(face))),
            "{f:?}"
        );
    }
    assert!(has(&f, FaultKind::MaterialInKind, Entity::Region(ix(1))));
    // Edges in an acorn.
    let f = faults(&wire(), BodyKind::Acorn);
    assert_eq!(
        f,
        [
            Fault::at(FaultKind::Misplaced, Entity::Edge(ix(0))),
            Fault::at(FaultKind::Misplaced, Entity::Edge(ix(1)))
        ]
    );
    // An edge without fins in a solid.
    let f = broken_cube(|r| {
        r.edge(Some((0, 7)));
    });
    assert_eq!(f, [Fault::at(FaultKind::Misplaced, Entity::Edge(ix(12)))]);
    // A vertex on no edge in a solid, and in a wire.
    let f = broken_cube(|r| {
        r.vertex([5, 5, 5]);
    });
    assert_eq!(f, [Fault::at(FaultKind::Misplaced, Entity::Vertex(ix(8)))]);
    let f = faults(&acorn(), BodyKind::Wire);
    assert_eq!(f, [Fault::at(FaultKind::Misplaced, Entity::Vertex(ix(0)))]);
    // Faces and edges in an acorn.
    let f = faults(&cube(), BodyKind::Acorn);
    assert!(has(&f, FaultKind::Misplaced, Entity::Face(ix(0))));
    assert!(has(&f, FaultKind::Misplaced, Entity::Edge(ix(0))));
}

#[test]
fn a_loop_that_does_not_close() {
    // Face 0's fins in the order 0, 2, 1, 3: each ring link is sound, the loop is not.
    let f = broken_cube(|r| {
        r.fins[0].next = ix(2);
        r.fins[2].prev = ix(0);
        r.fins[2].next = ix(1);
        r.fins[1].prev = ix(2);
        r.fins[1].next = ix(3);
        r.fins[3].prev = ix(1);
    });
    assert!(has(&f, FaultKind::LoopOpen, Entity::Loop(ix(0))), "{f:?}");
    assert!(f.iter().all(|x| x.kind == FaultKind::LoopOpen));
}

#[test]
fn a_closed_edge_sharing_its_loop() {
    let mut r = Rows::empty();
    let (a, b) = (r.edge(None), r.edge(None));
    let s = r.shell(0);
    r.face(s, &[&[(a, Sense::Forward), (b, Sense::Forward)]]);
    let f = faults(&r, BodyKind::Sheet);
    assert!(f.contains(
        &Fault::at(FaultKind::RingEdgeInLoop, Entity::Loop(ix(0))).with(Entity::Fin(ix(0)))
    ));
    assert!(f.contains(
        &Fault::at(FaultKind::RingEdgeInLoop, Entity::Loop(ix(0))).with(Entity::Fin(ix(1)))
    ));
}

#[test]
fn edges_with_the_wrong_number_of_fins() {
    // A box without its lid: the lid's four edges have one fin each, and the open shell's
    // characteristic, 8 − 12 + 10 − 5 = 1, is odd.
    let open = polyhedron(&CUBE, &CUBE_FACES[..5]);
    let f = faults(&open, BodyKind::Solid);
    for e in [2, 5, 8, 11] {
        assert!(
            f.contains(&Fault::at(FaultKind::FinCount, Entity::Edge(ix(e))).value(1)),
            "{f:?}"
        );
    }
    assert!(f.contains(&Fault::at(FaultKind::Euler, Entity::Shell(ix(0))).value(1)));
    // Three faces of a sheet on one edge.
    let mut r = Rows::empty();
    for p in [[0, 0, 0], [10, 0, 0], [5, 10, 0], [5, -10, 0], [5, 0, 10]] {
        r.vertex(p);
    }
    let s = r.shell(0);
    faces_into(&mut r, s, &[&[0, 1, 2], &[1, 0, 3], &[0, 1, 4]]);
    let f = faults(&r, BodyKind::Sheet);
    assert!(
        f.contains(&Fault::at(FaultKind::FinCount, Entity::Edge(ix(0))).value(3)),
        "{f:?}"
    );
}

#[test]
fn partners_running_the_same_way() {
    // Face 0 turned over: its fins in the other order, each the other way along its edge.
    let f = broken_cube(|r| {
        for k in 0..4 {
            let fin: &mut Fin = &mut r.fins[k];
            std::mem::swap(&mut fin.next, &mut fin.prev);
            fin.sense = match fin.sense {
                Sense::Forward => Sense::Reversed,
                Sense::Reversed => Sense::Forward,
            };
        }
    });
    let expected: Vec<Fault> = [0, 1, 2, 3]
        .map(|e| Fault::at(FaultKind::PartnerSense, Entity::Edge(ix(e))))
        .to_vec();
    assert_eq!(f, expected);
}

#[test]
fn partners_in_different_shells() {
    // Faces 3 to 5 moved to a second shell of the solid.
    let f = broken_cube(|r| {
        let s = r.shell(1);
        r.faces[2].next = None;
        r.shells[s as usize].first_face = Some(ix(3));
        for face in 3..6 {
            r.faces[face].shell = ix(s);
        }
    });
    for e in [0, 1, 2, 5, 6, 7, 8, 9] {
        assert!(
            has(&f, FaultKind::PartnerShells, Entity::Edge(ix(e))),
            "{e}: {f:?}"
        );
    }
    assert!(!has(&f, FaultKind::PartnerShells, Entity::Edge(ix(3))));
}

#[test]
fn a_shell_in_pieces() {
    // Two cubes in one shell: not connected, and the characteristic is 4.
    let points: Vec<[i64; 3]> = CUBE
        .iter()
        .copied()
        .chain(CUBE.iter().map(|p| [p[0] + 100, p[1], p[2]]))
        .collect();
    let moved: Vec<Vec<u32>> = CUBE_FACES
        .iter()
        .map(|f| f.iter().map(|v| v + 8).collect())
        .collect();
    let faces: Vec<&[u32]> = CUBE_FACES
        .iter()
        .copied()
        .chain(moved.iter().map(Vec::as_slice))
        .collect();
    let f = faults(&polyhedron(&points, &faces), BodyKind::Solid);
    assert_eq!(
        f,
        [
            Fault::at(FaultKind::ShellDisconnected, Entity::Shell(ix(0))).with(Entity::Face(ix(6))),
            Fault::at(FaultKind::Euler, Entity::Shell(ix(0))).value(4)
        ]
    );
    // Two squares in one sheet: 8 − 8 + 4 − 2 = 2, plus two boundary cycles.
    let mut r = Rows::empty();
    for p in [
        [0, 0, 0],
        [10, 0, 0],
        [10, 10, 0],
        [0, 10, 0],
        [20, 0, 0],
        [30, 0, 0],
        [30, 10, 0],
        [20, 10, 0],
    ] {
        r.vertex(p);
    }
    let s = r.shell(0);
    faces_into(&mut r, s, &[&[0, 1, 2, 3], &[4, 5, 6, 7]]);
    let f = faults(&r, BodyKind::Sheet);
    assert!(
        f.contains(&Fault::at(FaultKind::Euler, Entity::Shell(ix(0))).value(4)),
        "{f:?}"
    );
}

#[test]
fn shells_and_regions_out_of_place() {
    // A shell with no faces.
    let f = broken_cube(|r| {
        r.shell(1);
    });
    assert_eq!(f, [Fault::at(FaultKind::EmptyShell, Entity::Shell(ix(1)))]);
    // A solid region with no shell.
    let f = broken_cube(|r| {
        r.region(RegionKind::Solid);
    });
    assert_eq!(
        f,
        [Fault::at(FaultKind::EmptyRegion, Entity::Region(ix(2)))]
    );
    // A solid body whose only region is void.
    let f = broken_cube(|r| r.regions[1].kind = RegionKind::Void);
    assert_eq!(
        f,
        [
            Fault::at(FaultKind::NoMaterial, Entity::Body),
            Fault::at(FaultKind::ShellOfVoid, Entity::Shell(ix(0)))
        ]
    );
    // A sheet with a solid region.
    let mut r = square_sheet();
    r.region(RegionKind::Solid);
    assert_eq!(
        faults(&r, BodyKind::Sheet),
        [
            Fault::at(FaultKind::EmptyRegion, Entity::Region(ix(1))),
            Fault::at(FaultKind::MaterialInKind, Entity::Region(ix(1)))
        ]
    );
}

#[test]
fn an_exact_record_on_curved_geometry() {
    let exact = |r: &Rows| {
        report(r, BodyKind::Solid, Exactness::Exact)
            .faults()
            .to_vec()
    };
    // Group 4 judges the record whatever group 2 finds first: the cube's corners and edges are
    // not on a unit sphere, nor its edge's ends at a unit circle's.
    let group = |faults: Vec<Fault>, g: Group| -> Vec<Fault> {
        faults.into_iter().filter(|f| f.kind.group() == g).collect()
    };
    let unchecked = |at: Entity, with: Entity| Fault::at(FaultKind::Unchecked, at).with(with);
    let mut r = cube();
    r.surfaces[0] = sphere();
    assert_eq!(
        group(exact(&r), Group::Exactness),
        [Fault::at(FaultKind::NotFlat, Entity::Face(ix(0)))]
    );
    assert!(group(exact(&r), Group::Geometric).iter().all(|f| matches!(
        f.kind,
        FaultKind::VertexOffSurface | FaultKind::EdgeOffSurface
    ) && f.with
        == Some(Entity::Face(ix(0)))));
    // Certified, the same geometry is the record's to bound, not group 4's to refuse.
    assert_eq!(group(faults(&r, BodyKind::Solid), Group::Exactness), []);
    let mut r = cube();
    r.curves[0] = circle();
    assert_eq!(
        group(exact(&r), Group::Exactness),
        [Fault::at(FaultKind::NotStraight, Entity::Edge(ix(0)))]
    );
    let mut r = cube();
    r.pcurves.push(pcircle());
    r.fins[0].pcurve = Some(ix(0));
    assert_eq!(
        exact(&r),
        [
            unchecked(Entity::Pcurve(ix(0)), Entity::Fin(ix(0))),
            Fault::at(FaultKind::CurvedPcurve, Entity::Fin(ix(0)))
        ]
    );
}

#[test]
fn faults_past_the_cap_are_counted() {
    // 300 fins in no loop and on no ring: 600 faults.
    let mut r = cube();
    for _ in 0..300 {
        let at = u32::try_from(r.fins.len()).unwrap();
        r.fins.push(Fin {
            loop_: ix(0),
            next: ix(at),
            prev: ix(at),
            edge: ix(0),
            sense: Sense::Forward,
            radial: ix(at),
            pcurve: None,
        });
    }
    let report = report(&r, BodyKind::Solid, Exactness::Exact);
    assert_eq!(report.faults().len(), MAX_FAULTS);
    assert_eq!(report.more(), 600 - MAX_FAULTS as u64);
    assert!(!report.passed());
    assert_eq!(
        report.faults()[0],
        Fault::at(FaultKind::Orphan, Entity::Fin(ix(24)))
    );
}

#[test]
fn every_fault_says_what_is_wrong_in_its_group() {
    let all = [
        FaultKind::ChainCycle,
        FaultKind::ListedTwice,
        FaultKind::WrongParent,
        FaultKind::Orphan,
        FaultKind::EmptyLoop,
        FaultKind::FinRing,
        FaultKind::FinPrev,
        FaultKind::RadialRing,
        FaultKind::RadialEdge,
        FaultKind::OffRadialRing,
        FaultKind::EdgeEnds,
        FaultKind::Misplaced,
        FaultKind::LoopOpen,
        FaultKind::RingEdgeInLoop,
        FaultKind::FinCount,
        FaultKind::PartnerSense,
        FaultKind::PartnerShells,
        FaultKind::ShellDisconnected,
        FaultKind::EmptyShell,
        FaultKind::EmptyRegion,
        FaultKind::NoMaterial,
        FaultKind::MaterialInKind,
        FaultKind::ShellOfVoid,
        FaultKind::Euler,
        FaultKind::NotFlat,
        FaultKind::NotStraight,
        FaultKind::CurvedPcurve,
        FaultKind::VertexOffSurface,
        FaultKind::VertexOffCurve,
        FaultKind::EdgeOffSurface,
        FaultKind::Unchecked,
        FaultKind::EdgesCross,
        FaultKind::FacesCross,
        FaultKind::PairUnchecked,
    ];
    let mut words: Vec<&str> = all.iter().map(|k| k.what()).collect();
    assert!(words.iter().all(|w| w.len() > 10));
    words.sort_unstable();
    words.dedup();
    assert_eq!(
        words.len(),
        all.len(),
        "each kind says something of its own"
    );
    for kind in all {
        let expected = match kind {
            FaultKind::NotFlat | FaultKind::NotStraight | FaultKind::CurvedPcurve => {
                Group::Exactness
            }
            FaultKind::VertexOffSurface
            | FaultKind::VertexOffCurve
            | FaultKind::EdgeOffSurface
            | FaultKind::Unchecked => Group::Geometric,
            FaultKind::EdgesCross | FaultKind::FacesCross | FaultKind::PairUnchecked => {
                Group::SelfIntersection
            }
            _ => Group::Combinatorial,
        };
        assert_eq!(kind.group(), expected, "{kind:?}");
    }
    assert_eq!(
        Fault::at(FaultKind::FinCount, Entity::Edge(ix(3)))
            .value(1)
            .to_string(),
        "group 1, edge 3: the edge has a number of fins its body kind does not allow (1)"
    );
    assert_eq!(
        Fault::at(FaultKind::ListedTwice, Entity::Face(ix(3)))
            .with(Entity::Shell(ix(1)))
            .to_string(),
        "group 1, face 3: it is listed by a second parent (shell 1)"
    );
    assert_eq!(
        Fault::at(FaultKind::NotFlat, Entity::Face(ix(0))).to_string(),
        "group 4, face 0: the body is recorded Exact, but the face is curved"
    );
    let entities = [
        (Entity::Body, "the body"),
        (Entity::Region(ix(1)), "region 1"),
        (Entity::Shell(ix(2)), "shell 2"),
        (Entity::Face(ix(3)), "face 3"),
        (Entity::Loop(ix(4)), "loop 4"),
        (Entity::Fin(ix(5)), "fin 5"),
        (Entity::Edge(ix(6)), "edge 6"),
        (Entity::Vertex(ix(7)), "vertex 7"),
        (Entity::Surface(ix(8)), "surface 8"),
        (Entity::Curve(ix(9)), "curve 9"),
        (Entity::Pcurve(ix(10)), "pcurve 10"),
    ];
    for (entity, text) in entities {
        assert_eq!(entity.to_string(), text);
    }
}

/// A writer that refuses every write after its first `left`.
struct FailAfter {
    left: usize,
}

impl std::fmt::Write for FailAfter {
    fn write_str(&mut self, _: &str) -> std::fmt::Result {
        if self.left == 0 {
            return Err(std::fmt::Error);
        }
        self.left = self.left.saturating_sub(1);
        Ok(())
    }
}

#[test]
fn a_fault_written_to_a_failing_writer_reports_the_failure() {
    use std::fmt::Write as _;
    let fault = Fault::at(FaultKind::FinCount, Entity::Edge(ix(3)))
        .with(Entity::Face(ix(1)))
        .value(2);
    let full = fault.to_string();
    // However many writes succeed first, a refused one ends the fault's writing with the error.
    let writes = (0..64)
        .find(|&n| write!(FailAfter { left: n }, "{fault}").is_ok())
        .unwrap();
    for n in 0..writes {
        assert!(write!(FailAfter { left: n }, "{fault}").is_err(), "{n}");
    }
    assert!(writes >= 3, "{full}");
}
