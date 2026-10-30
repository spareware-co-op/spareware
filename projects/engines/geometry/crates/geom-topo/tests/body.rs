//! Publishing arenas as a body, through the public interface: every check `builder.rs` promises,
//! in its order, on every column, at the boundary of each arena; the meter; and edits.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    reason = "tests may use native arithmetic and casts"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::{ix, many_faces, meter, wall, wall_builder, wall_outside, wall_rows};
use geom_math::Box3;
use geom_num::grid::P3;
use geom_run::{Budget, BudgetKind, CancelToken, Stop, WorkMeter};
use geom_topo::{Arena, Arenas, BodyKind, Builder, Link, RegionKind, Sense, TopoError};

/// Makes `link`'s column at `row` point at row `target` of the arena it points into.
fn point(a: &mut Arenas, link: Link, row: u32, target: u32) {
    let t = target;
    match link {
        Link::RegionFirstShell => a.regions.first_shell.set(ix(row), Some(ix(t))),
        Link::ShellRegion => a.shells.region.set(ix(row), ix(t)),
        Link::ShellNext => a.shells.next.set(ix(row), Some(ix(t))),
        Link::ShellFirstFace => a.shells.first_face.set(ix(row), Some(ix(t))),
        Link::FaceShell => a.faces.shell.set(ix(row), ix(t)),
        Link::FaceNext => a.faces.next.set(ix(row), Some(ix(t))),
        Link::FaceSurface => a.faces.surface.set(ix(row), ix(t)),
        Link::FaceFirstLoop => a.faces.first_loop.set(ix(row), Some(ix(t))),
        Link::LoopFace => a.loops.face.set(ix(row), ix(t)),
        Link::LoopNext => a.loops.next.set(ix(row), Some(ix(t))),
        Link::LoopFirstFin => a.loops.first_fin.set(ix(row), Some(ix(t))),
        Link::FinLoop => a.fins.loop_.set(ix(row), ix(t)),
        Link::FinNext => a.fins.next.set(ix(row), ix(t)),
        Link::FinPrev => a.fins.prev.set(ix(row), ix(t)),
        Link::FinEdge => a.fins.edge.set(ix(row), ix(t)),
        Link::FinRadial => a.fins.radial.set(ix(row), ix(t)),
        Link::FinPcurve => a.fins.pcurve.set(ix(row), Some(ix(t))),
        Link::EdgeFirstFin => a.edges.first_fin.set(ix(row), Some(ix(t))),
        Link::EdgeStart => a.edges.start.set(ix(row), Some(ix(t))),
        Link::EdgeEnd => a.edges.end.set(ix(row), Some(ix(t))),
        Link::EdgeCurve => a.edges.curve.set(ix(row), ix(t)),
        Link::VertexPoint => a.vertices.point.set(ix(row), ix(t)),
    }
    .unwrap();
}

/// Gives one column one more row.
type Grow = fn(&mut Arenas);

/// Every column of an arena with more than one, each with a way to give it one more row than the
/// others.
fn growers() -> Vec<(Arena, Grow)> {
    vec![
        (Arena::Region, |a| {
            a.regions.kind.push(RegionKind::Void).unwrap();
        }),
        (Arena::Region, |a| {
            a.regions.first_shell.push(None).unwrap();
        }),
        (Arena::Shell, |a| {
            a.shells.region.push(ix(0)).unwrap();
        }),
        (Arena::Shell, |a| {
            a.shells.next.push(None).unwrap();
        }),
        (Arena::Shell, |a| {
            a.shells.first_face.push(None).unwrap();
        }),
        (Arena::Face, |a| {
            a.faces.shell.push(ix(0)).unwrap();
        }),
        (Arena::Face, |a| {
            a.faces.next.push(None).unwrap();
        }),
        (Arena::Face, |a| {
            a.faces.surface.push(ix(0)).unwrap();
        }),
        (Arena::Face, |a| {
            a.faces.sense.push(Sense::Forward).unwrap();
        }),
        (Arena::Face, |a| {
            a.faces.first_loop.push(None).unwrap();
        }),
        (Arena::Face, |a| {
            let b = Box3::new(P3::new(0, 0, 0).unwrap(), P3::new(0, 0, 0).unwrap()).unwrap();
            a.faces.bound.push(b).unwrap();
        }),
        (Arena::Loop, |a| {
            a.loops.face.push(ix(0)).unwrap();
        }),
        (Arena::Loop, |a| {
            a.loops.next.push(None).unwrap();
        }),
        (Arena::Loop, |a| {
            a.loops.first_fin.push(None).unwrap();
        }),
        (Arena::Fin, |a| {
            a.fins.loop_.push(ix(0)).unwrap();
        }),
        (Arena::Fin, |a| {
            a.fins.next.push(ix(0)).unwrap();
        }),
        (Arena::Fin, |a| {
            a.fins.prev.push(ix(0)).unwrap();
        }),
        (Arena::Fin, |a| {
            a.fins.edge.push(ix(0)).unwrap();
        }),
        (Arena::Fin, |a| {
            a.fins.sense.push(Sense::Forward).unwrap();
        }),
        (Arena::Fin, |a| {
            a.fins.radial.push(ix(0)).unwrap();
        }),
        (Arena::Fin, |a| {
            a.fins.pcurve.push(None).unwrap();
        }),
        (Arena::Edge, |a| {
            a.edges.first_fin.push(None).unwrap();
        }),
        (Arena::Edge, |a| {
            a.edges.start.push(None).unwrap();
        }),
        (Arena::Edge, |a| {
            a.edges.end.push(None).unwrap();
        }),
        (Arena::Edge, |a| {
            a.edges.curve.push(ix(0)).unwrap();
        }),
    ]
}

fn publish(b: Builder) -> Result<geom_topo::Body, TopoError> {
    b.publish(BodyKind::Solid, wall_outside(), &mut meter())
}

#[test]
fn the_wall_publishes_with_every_row_as_built() {
    let body = wall();
    let w = wall_rows();
    assert_eq!(body.kind(), BodyKind::Solid);
    assert_eq!(body.outside(), wall_outside());
    let a = body.arenas();
    let counts = [2, 1, 6, 6, 24, 12, 8, 6, 12, 0, 8];
    for (arena, n) in Arena::ALL.iter().zip(counts) {
        assert_eq!(a.rows(*arena), n, "{arena}");
    }
    for (r, row) in w.regions.iter().enumerate() {
        assert_eq!(a.regions.get(ix(r as u32)), Some(*row));
    }
    for (r, row) in w.shells.iter().enumerate() {
        assert_eq!(a.shells.get(ix(r as u32)), Some(*row));
    }
    for (r, row) in w.faces.iter().enumerate() {
        assert_eq!(a.faces.get(ix(r as u32)), Some(*row));
    }
    for (r, row) in w.loops.iter().enumerate() {
        assert_eq!(a.loops.get(ix(r as u32)), Some(*row));
    }
    for (r, row) in w.fins.iter().enumerate() {
        assert_eq!(a.fins.get(ix(r as u32)), Some(*row));
    }
    for (r, row) in w.edges.iter().enumerate() {
        assert_eq!(a.edges.get(ix(r as u32)), Some(*row));
    }
    for (r, row) in w.vertices.iter().enumerate() {
        assert_eq!(a.vertices.get(ix(r as u32)), Some(*row));
    }
    assert!(a.surfaces.iter().eq(w.surfaces.iter()));
    assert!(a.curves.iter().eq(w.curves.iter()));
    assert!(a.points.iter().eq(w.points.iter()));
    // One past the end of each arena is no row.
    assert_eq!(a.regions.get(ix(2)), None);
    assert_eq!(a.shells.get(ix(1)), None);
    assert_eq!(a.faces.get(ix(6)), None);
    assert_eq!(a.loops.get(ix(6)), None);
    assert_eq!(a.fins.get(ix(24)), None);
    assert_eq!(a.edges.get(ix(12)), None);
    assert_eq!(a.vertices.get(ix(8)), None);
    // The lengths each arena reports, and emptiness.
    assert_eq!(
        [
            a.regions.len(),
            a.shells.len(),
            a.faces.len(),
            a.loops.len(),
            a.fins.len(),
            a.edges.len(),
            a.vertices.len()
        ],
        [2, 1, 6, 6, 24, 12, 8]
    );
    assert!(
        ![
            a.regions.is_empty(),
            a.shells.is_empty(),
            a.faces.is_empty(),
            a.loops.is_empty(),
            a.fins.is_empty(),
            a.edges.is_empty(),
            a.vertices.is_empty()
        ]
        .contains(&true)
    );
    let empty = Arenas::new();
    assert!(
        empty.regions.is_empty()
            && empty.shells.is_empty()
            && empty.faces.is_empty()
            && empty.loops.is_empty()
            && empty.fins.is_empty()
            && empty.edges.is_empty()
            && empty.vertices.is_empty()
    );
}

#[test]
fn push_and_extend_build_the_same_body() {
    let pushed = publish(wall_builder(false)).unwrap();
    let extended = publish(wall_builder(true)).unwrap();
    assert_eq!(pushed, extended);
    assert_eq!(pushed.arenas(), extended.arenas());
}

#[test]
fn every_body_kind_publishes() {
    for kind in [
        BodyKind::Solid,
        BodyKind::Sheet,
        BodyKind::Wire,
        BodyKind::Acorn,
        BodyKind::General,
    ] {
        let body = wall_builder(true)
            .publish(kind, wall_outside(), &mut meter())
            .unwrap();
        assert_eq!(body.kind(), kind);
    }
}

#[test]
fn a_ragged_arena_is_refused_whichever_column_is_long() {
    for (arena, grow) in growers() {
        let mut b = wall_builder(true);
        grow(&mut b.arenas);
        assert_eq!(publish(b), Err(TopoError::Ragged(arena)), "{arena}");
    }
}

#[test]
fn a_link_one_past_its_arena_is_refused_and_the_last_row_is_not() {
    let w = wall_builder(true);
    for link in Link::ALL {
        let rows = w.arenas.rows(link.to());
        let from = w.arenas.rows(link.from());
        let row = from - 1;
        let mut b = w.clone();
        if rows > 0 {
            point(&mut b.arenas, link, row, rows - 1);
            assert!(publish(b).is_ok(), "{link} at the last row");
        }
        let mut b = w.clone();
        point(&mut b.arenas, link, row, rows);
        assert_eq!(publish(b), Err(TopoError::Dangling { link, row }), "{link}");
        let mut b = w.clone();
        point(&mut b.arenas, link, 0, u32::MAX - 1);
        assert_eq!(
            publish(b),
            Err(TopoError::Dangling { link, row: 0 }),
            "{link}"
        );
    }
}

#[test]
fn links_name_their_arenas_and_columns() {
    use Arena::{Curve, Edge, Face, Fin, Loop, Pcurve, Point, Region, Shell, Surface, Vertex};
    let names: Vec<&str> = Link::ALL.iter().map(|l| l.name()).collect();
    assert_eq!(
        names,
        [
            "region.first_shell",
            "shell.region",
            "shell.next",
            "shell.first_face",
            "face.shell",
            "face.next",
            "face.surface",
            "face.first_loop",
            "loop.face",
            "loop.next",
            "loop.first_fin",
            "fin.loop",
            "fin.next",
            "fin.prev",
            "fin.edge",
            "fin.radial",
            "fin.pcurve",
            "edge.first_fin",
            "edge.start",
            "edge.end",
            "edge.curve",
            "vertex.point",
        ]
    );
    for link in Link::ALL {
        assert_eq!(link.to_string(), link.name());
        let (entity, _) = link.name().split_once('.').unwrap();
        assert_eq!(link.from().name(), entity);
    }
    let to: Vec<Arena> = Link::ALL.iter().map(|l| l.to()).collect();
    assert_eq!(
        to,
        [
            Shell, Region, Shell, Face, Shell, Face, Surface, Loop, Face, Loop, Fin, Loop, Fin,
            Fin, Edge, Fin, Pcurve, Fin, Vertex, Vertex, Curve, Point
        ]
    );
    let mut sorted = Link::ALL;
    sorted.sort();
    assert_eq!(sorted, Link::ALL, "ALL is in declaration order");
}

#[test]
fn the_first_fault_is_the_one_reported() {
    // Ragged comes before any dangling link, and an earlier arena before a later one.
    let mut b = wall_builder(true);
    point(&mut b.arenas, Link::RegionFirstShell, 0, 9);
    b.arenas.edges.curve.push(ix(0)).unwrap();
    b.arenas.faces.sense.push(Sense::Forward).unwrap();
    assert_eq!(publish(b), Err(TopoError::Ragged(Arena::Face)));
    // Links in Link::ALL's order, then rows ascending.
    let mut b = wall_builder(true);
    point(&mut b.arenas, Link::VertexPoint, 0, 99);
    point(&mut b.arenas, Link::FinRadial, 20, 99);
    point(&mut b.arenas, Link::FinRadial, 3, 99);
    point(&mut b.arenas, Link::EdgeStart, 0, 99);
    assert_eq!(
        publish(b),
        Err(TopoError::Dangling {
            link: Link::FinRadial,
            row: 3
        })
    );
    // A dangling link comes before a bad outside region.
    let mut b = wall_builder(true);
    point(&mut b.arenas, Link::VertexPoint, 7, 8);
    assert_eq!(
        b.publish(BodyKind::Solid, ix(0), &mut meter()),
        Err(TopoError::Dangling {
            link: Link::VertexPoint,
            row: 7
        })
    );
}

#[test]
fn absent_links_are_never_dangling() {
    let mut b = wall_builder(true);
    let a = &mut b.arenas;
    a.regions.first_shell.set(ix(0), None).unwrap();
    a.shells.next.set(ix(0), None).unwrap();
    a.faces.first_loop.set(ix(2), None).unwrap();
    a.loops.first_fin.set(ix(1), None).unwrap();
    a.edges.start.set(ix(4), None).unwrap();
    a.edges.end.set(ix(4), None).unwrap();
    a.edges.first_fin.set(ix(4), None).unwrap();
    assert!(publish(b).is_ok());
}

#[test]
fn the_outside_must_be_a_void_region_of_the_body() {
    for outside in [0, 2, 3, u32::MAX - 1] {
        assert_eq!(
            wall_builder(true).publish(BodyKind::Solid, ix(outside), &mut meter()),
            Err(TopoError::Outside),
            "region {outside}"
        );
    }
    // With no regions at all, nothing is the outside.
    assert_eq!(
        Builder::new().publish(BodyKind::Acorn, ix(0), &mut meter()),
        Err(TopoError::Outside)
    );
    // A body with only the outside region publishes.
    let mut b = Builder::new();
    b.arenas
        .regions
        .push(geom_topo::Region {
            kind: RegionKind::Void,
            first_shell: None,
        })
        .unwrap();
    let body = b.publish(BodyKind::Acorn, ix(0), &mut meter()).unwrap();
    assert_eq!(body.outside(), ix(0));
}

/// The chunks of every link column of `a`: what publishing charges.
fn link_chunks(a: &Arenas) -> u128 {
    let c = |n: u32| u128::from(n.div_ceil(geom_topo::CHUNK_ROWS));
    c(a.regions.len())
        + 3 * c(a.shells.len())
        + 4 * c(a.faces.len())
        + 3 * c(a.loops.len())
        + 6 * c(a.fins.len())
        + 4 * c(a.edges.len())
        + c(a.vertices.len())
}

#[test]
fn publishing_charges_one_unit_per_chunk_of_every_link_column() {
    let b = wall_builder(true);
    let mut m = meter();
    let body = b.publish(BodyKind::Solid, wall_outside(), &mut m).unwrap();
    assert_eq!(m.used().work, 22);
    assert_eq!(link_chunks(body.arenas()), 22);
    let big = many_faces(3 * geom_topo::CHUNK_ROWS + 1);
    let mut m = meter();
    let again = Builder::edit(&big)
        .publish(BodyKind::Sheet, ix(1), &mut m)
        .unwrap();
    assert_eq!(again, big);
    // Region: 1; shells: 3; faces: four link columns of four chunks each.
    assert_eq!(m.used().work, 1 + 3 + 4 * 4);
    assert_eq!(m.used().work, link_chunks(big.arenas()));
    assert_eq!(m.used().logical_bytes, 0);
}

#[test]
fn a_meter_that_stops_stops_publishing() {
    let mut m = WorkMeter::new(Budget::new(21, u64::MAX));
    assert_eq!(
        wall_builder(true).publish(BodyKind::Solid, wall_outside(), &mut m),
        Err(TopoError::Stopped(Stop::OverBudget(BudgetKind::Work)))
    );
    let mut m = WorkMeter::new(Budget::new(22, u64::MAX));
    assert!(
        wall_builder(true)
            .publish(BodyKind::Solid, wall_outside(), &mut m)
            .is_ok()
    );
    // Cancellation is read every 64 charges: a body of 300,000 faces charges 300 times.
    let big = many_faces(300_000);
    let cancel = CancelToken::new();
    cancel.cancel();
    let mut m = WorkMeter::new(Budget::UNLIMITED).with_cancel(&cancel);
    assert_eq!(
        Builder::edit(&big).publish(BodyKind::Sheet, ix(1), &mut m),
        Err(TopoError::Stopped(Stop::Cancelled))
    );
}

#[test]
fn an_edit_shares_every_chunk_until_written() {
    let body = wall();
    let b = Builder::edit(&body);
    assert_eq!(b.arenas, *body.arenas());
    assert_eq!(body.memory().unique_chunks, 0);
    assert_eq!(b.arenas.memory().unique_chunks, 0);
    let mut b = b;
    b.arenas.faces.sense.set(ix(0), Sense::Forward).unwrap();
    let v2 = b
        .publish(BodyKind::Solid, wall_outside(), &mut meter())
        .unwrap();
    assert_eq!(v2.memory().unique_chunks, 1);
    assert_eq!(body.memory().unique_chunks, 1);
    assert_eq!(body, wall(), "the original is unchanged");
    assert_ne!(v2, body);
    assert_eq!(v2.arenas().faces.sense.get(ix(0)), Some(&Sense::Forward));
}

#[test]
fn errors_say_what_and_where() {
    let cases = [
        (
            TopoError::Full(Arena::Fin),
            "the fin arena already holds the most rows an index addresses",
        ),
        (
            TopoError::NoSuchRow {
                arena: Arena::Edge,
                row: 12,
            },
            "the edge arena has no row 12",
        ),
        (
            TopoError::Ragged(Arena::Loop),
            "the columns of the loop arena hold different numbers of rows",
        ),
        (
            TopoError::Dangling {
                link: Link::FaceSurface,
                row: 3,
            },
            "face.surface of face 3 points past the end of the surface arena",
        ),
        (
            TopoError::Outside,
            "the outside region is not a void region of the body",
        ),
        (
            TopoError::Stopped(Stop::Cancelled),
            "publishing the body stopped: cancelled by the caller",
        ),
        (TopoError::OutOfMemory, "an allocation was refused"),
    ];
    for (e, text) in cases {
        assert_eq!(e.to_string(), text);
    }
    let mut v: Vec<u8> = Vec::new();
    let refused = v.try_reserve(usize::MAX).unwrap_err();
    assert_eq!(TopoError::from(refused), TopoError::OutOfMemory);
}
