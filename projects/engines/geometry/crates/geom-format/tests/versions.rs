//! A new version of a body hashes only the chunks it changed and gets the IDs a full write gets,
//! whichever column changed, and a body's geometry ID does not see its names (G105, 23.8, 24.2).

#![allow(
    dead_code,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    reason = "tests: native arithmetic and indexing, and a failure stops the test"
)]

#[path = "../../geom-topo/tests/common/mod.rs"]
mod topo;

use geom_format::body::{BodyVersion, CHUNK_ROWS, DEPTH, table, write_body};
use geom_format::cbor::{Decoder, Item};
use geom_format::record::Record;
use geom_math::Box3;
use geom_num::grid::P3;
use geom_topo::order::{NameKey, Names};
use geom_topo::{Arenas, Body, BodyKind, Builder, Ix, PointKind, RegionKind, Sense, Shell};
use proptest::prelude::*;

use topo::{ix, line, many_faces, meter, plane, wall, wall_builder, wall_outside, wall_rows};

fn first(body: &Body) -> BodyVersion {
    BodyVersion::first(body, &mut meter()).unwrap()
}

/// The next version, checked against a full write of the same body.
fn next(before: &BodyVersion, body: &Body) -> BodyVersion {
    let version = before.next(body, &mut meter()).unwrap();
    let full = write_body(body, &mut meter()).unwrap();
    assert_eq!(*version.index(), full.index);
    assert_eq!(version.content_id(), full.content_id());
    for record in version.written() {
        assert!(
            full.chunks.contains(record),
            "a chunk the body does not hold"
        );
    }
    version
}

fn published(body: &Body, arenas: Arenas) -> Body {
    Builder { arenas }
        .publish(body.kind(), body.outside(), &mut meter())
        .unwrap()
}

/// The table a chunk record belongs to.
fn table_of(record: &Record) -> u64 {
    let mut d = Decoder::new(&record.payload, DEPTH);
    let mut m = meter();
    d.next(&mut m).unwrap();
    match d.next(&mut m).unwrap() {
        Some(Item::Uint(n)) => n,
        other => panic!("{other:?}"),
    }
}

/// A wall with a second shell and a pcurve, so every column has another value to take.
fn every_column() -> Body {
    let body = wall();
    let mut arenas = body.arenas().clone();
    arenas
        .shells
        .push(Shell {
            region: ix(1),
            next: None,
            first_face: None,
        })
        .unwrap();
    let pcurve = geom_curve::Line::new([0.0, 0.0], [1.0, 0.5], (0.0, 1.0)).unwrap();
    let pcurve = arenas
        .pcurves
        .push(geom_curve::CurveKind::Line(pcurve))
        .unwrap();
    arenas.fins.pcurve.set(ix(1), Some(pcurve)).unwrap();
    published(&body, arenas)
}

/// Another row than `now`.
fn other<K: geom_topo::Kind>(now: Ix<K>) -> Ix<K> {
    if now.row() == 0 { ix(1) } else { ix(0) }
}

/// `Some` of a row for `None`, and `None` for `Some`.
fn flipped<K: geom_topo::Kind>(now: Option<Ix<K>>) -> Option<Ix<K>> {
    if now.is_some() { None } else { Some(ix(0)) }
}

fn reversed(now: Sense) -> Sense {
    match now {
        Sense::Forward => Sense::Reversed,
        Sense::Reversed => Sense::Forward,
    }
}

/// A table, a column of it, and an edit of that column.
type ColumnEdit = (u64, &'static str, fn(&mut Arenas));
/// An edit of a body's names.
type NameEdit = fn(&mut Names);

/// One edit of row 0 of each column of each table, with the table it is in.
const EDITS: [ColumnEdit; 30] = [
    (table::REGIONS, "kind", |a| {
        a.regions.kind.set(ix(0), RegionKind::Void).unwrap();
    }),
    (table::REGIONS, "first shell", |a| {
        let now = *a.regions.first_shell.get(ix(0)).unwrap();
        a.regions.first_shell.set(ix(0), flipped(now)).unwrap();
    }),
    (table::SHELLS, "region", |a| {
        let now = *a.shells.region.get(ix(0)).unwrap();
        a.shells.region.set(ix(0), other(now)).unwrap();
    }),
    (table::SHELLS, "next", |a| {
        let now = *a.shells.next.get(ix(0)).unwrap();
        a.shells.next.set(ix(0), flipped(now)).unwrap();
    }),
    (table::SHELLS, "first face", |a| {
        let now = *a.shells.first_face.get(ix(0)).unwrap();
        a.shells.first_face.set(ix(0), flipped(now)).unwrap();
    }),
    (table::FACES, "shell", |a| {
        let now = *a.faces.shell.get(ix(0)).unwrap();
        a.faces.shell.set(ix(0), other(now)).unwrap();
    }),
    (table::FACES, "next", |a| {
        let now = *a.faces.next.get(ix(0)).unwrap();
        a.faces.next.set(ix(0), flipped(now)).unwrap();
    }),
    (table::FACES, "surface", |a| {
        let now = *a.faces.surface.get(ix(0)).unwrap();
        a.faces.surface.set(ix(0), other(now)).unwrap();
    }),
    (table::FACES, "sense", |a| {
        let now = *a.faces.sense.get(ix(0)).unwrap();
        a.faces.sense.set(ix(0), reversed(now)).unwrap();
    }),
    (table::FACES, "first loop", |a| {
        let now = *a.faces.first_loop.get(ix(0)).unwrap();
        a.faces.first_loop.set(ix(0), flipped(now)).unwrap();
    }),
    (table::FACES, "bound", |a| {
        let far = Box3::new(P3::new(-9, -9, -9).unwrap(), P3::new(9, 9, 9).unwrap()).unwrap();
        a.faces.bound.set(ix(0), far).unwrap();
    }),
    (table::LOOPS, "face", |a| {
        let now = *a.loops.face.get(ix(0)).unwrap();
        a.loops.face.set(ix(0), other(now)).unwrap();
    }),
    (table::LOOPS, "next", |a| {
        let now = *a.loops.next.get(ix(0)).unwrap();
        a.loops.next.set(ix(0), flipped(now)).unwrap();
    }),
    (table::LOOPS, "first fin", |a| {
        let now = *a.loops.first_fin.get(ix(0)).unwrap();
        a.loops.first_fin.set(ix(0), flipped(now)).unwrap();
    }),
    (table::FINS, "loop", |a| {
        let now = *a.fins.loop_.get(ix(0)).unwrap();
        a.fins.loop_.set(ix(0), other(now)).unwrap();
    }),
    (table::FINS, "next", |a| {
        let now = *a.fins.next.get(ix(0)).unwrap();
        a.fins.next.set(ix(0), other(now)).unwrap();
    }),
    (table::FINS, "prev", |a| {
        let now = *a.fins.prev.get(ix(0)).unwrap();
        a.fins.prev.set(ix(0), other(now)).unwrap();
    }),
    (table::FINS, "edge", |a| {
        let now = *a.fins.edge.get(ix(0)).unwrap();
        a.fins.edge.set(ix(0), other(now)).unwrap();
    }),
    (table::FINS, "sense", |a| {
        let now = *a.fins.sense.get(ix(0)).unwrap();
        a.fins.sense.set(ix(0), reversed(now)).unwrap();
    }),
    (table::FINS, "radial", |a| {
        let now = *a.fins.radial.get(ix(0)).unwrap();
        a.fins.radial.set(ix(0), other(now)).unwrap();
    }),
    (table::FINS, "pcurve", |a| {
        let now = *a.fins.pcurve.get(ix(0)).unwrap();
        a.fins.pcurve.set(ix(0), flipped(now)).unwrap();
    }),
    (table::EDGES, "first fin", |a| {
        let now = *a.edges.first_fin.get(ix(0)).unwrap();
        a.edges.first_fin.set(ix(0), flipped(now)).unwrap();
    }),
    (table::EDGES, "start", |a| {
        let now = *a.edges.start.get(ix(0)).unwrap();
        a.edges.start.set(ix(0), flipped(now)).unwrap();
    }),
    (table::EDGES, "end", |a| {
        let now = *a.edges.end.get(ix(0)).unwrap();
        a.edges.end.set(ix(0), flipped(now)).unwrap();
    }),
    (table::EDGES, "curve", |a| {
        let now = *a.edges.curve.get(ix(0)).unwrap();
        a.edges.curve.set(ix(0), other(now)).unwrap();
    }),
    (table::VERTICES, "point", |a| {
        let now = *a.vertices.point.get(ix(0)).unwrap();
        a.vertices.point.set(ix(0), other(now)).unwrap();
    }),
    (table::POINTS, "point", |a| {
        a.points
            .set(ix(0), PointKind::Grid(P3::new(5, 6, 7).unwrap()))
            .unwrap();
    }),
    (table::SURFACES, "surface", |a| {
        a.surfaces.set(ix(0), plane([3.0, 4.0, 5.0], 0, 1)).unwrap();
    }),
    (table::CURVES, "curve", |a| {
        a.curves.set(ix(0), line([1, 2, 3], [4, 5, 6])).unwrap();
    }),
    (table::PCURVES, "pcurve", |a| {
        let moved = geom_curve::Line::new([2.0, 0.0], [1.0, 0.5], (0.0, 1.0)).unwrap();
        a.pcurves
            .set(ix(0), geom_curve::CurveKind::Line(moved))
            .unwrap();
    }),
];

#[test]
fn a_first_version_is_a_full_write() {
    for body in [wall(), every_column(), many_faces(9_000)] {
        let version = first(&body);
        let full = write_body(&body, &mut meter()).unwrap();
        assert_eq!(*version.index(), full.index);
        assert_eq!(version.written(), full.chunks);
        assert_eq!(version.content_id(), full.content_id());
    }
}

#[test]
fn an_edit_of_any_column_rewrites_its_chunk_and_no_other() {
    let body = every_column();
    let before = first(&body);
    for (code, column, edit) in EDITS {
        let mut arenas = body.arenas().clone();
        edit(&mut arenas);
        let edited = published(&body, arenas);
        let version = next(&before, &edited);
        assert_eq!(version.written().len(), 1, "table {code}, {column}");
        assert_eq!(table_of(&version.written()[0]), code, "{column}");
        assert_ne!(version.content_id(), before.content_id(), "{column}");
    }
}

#[test]
fn the_same_body_writes_nothing() {
    let body = every_column();
    let before = first(&body);
    let version = next(&before, &body);
    assert_eq!(version.written(), []);
    assert_eq!(version.content_id(), before.content_id());
}

#[test]
fn a_one_percent_edit_hashes_only_the_chunk_it_touches() {
    let body = many_faces(9_000);
    let before = first(&body);
    let mut arenas = body.arenas().clone();
    for row in 4_200..4_290 {
        arenas.faces.surface.set(ix(row), ix(1)).unwrap();
    }
    let version = next(&before, &published(&body, arenas));
    assert_eq!(version.written().len(), 1);
    assert_eq!(table_of(&version.written()[0]), table::FACES);
}

/// Equal rows in chunks of their own are not shared, so they are hashed again, to the same IDs.
#[test]
fn equal_rows_that_are_not_shared_are_hashed_again() {
    let before = first(&many_faces(9_000));
    let version = next(&before, &many_faces(9_000));
    assert_eq!(version.written().len(), before.written().len());
    assert_eq!(version.content_id(), before.content_id());
}

#[test]
fn a_table_that_grows_or_shrinks_writes_its_new_chunks() {
    let body = many_faces(CHUNK_ROWS + 10);
    let before = first(&body);
    let mut arenas = body.arenas().clone();
    let last = arenas.faces.shell.iter().count() as u32 - 1;
    let row = geom_topo::Face {
        shell: ix(0),
        next: None,
        surface: ix(0),
        sense: Sense::Forward,
        first_loop: None,
        bound: *arenas.faces.bound.get(ix(0)).unwrap(),
    };
    for _ in 0..CHUNK_ROWS {
        arenas.faces.push(row).unwrap();
    }
    arenas.faces.next.set(ix(last), Some(ix(last + 1))).unwrap();
    let grown = published(&body, arenas);
    let version = next(&before, &grown);
    assert!(version.written().len() >= 2);
    assert!(version.written().len() < before.written().len());
    let back = next(&version, &body);
    assert_eq!(back.content_id(), before.content_id());
}

fn key(arena: u8, row: usize) -> NameKey {
    let mut k = [0u8; 16];
    k[0] = arena;
    k[8..].copy_from_slice(&(row as u64).to_be_bytes());
    NameKey(k)
}

/// The wall, named from `base`.
fn named_wall(base: u8) -> Body {
    let r = wall_rows();
    let names = Names {
        regions: (0..r.regions.len()).map(|i| key(base, i)).collect(),
        faces: (0..r.faces.len()).map(|i| key(base + 1, i)).collect(),
        edges: (0..r.edges.len()).map(|i| key(base + 2, i)).collect(),
        vertices: (0..r.vertices.len()).map(|i| key(base + 3, i)).collect(),
    };
    wall_builder(true)
        .publish_named(&names, BodyKind::Solid, wall_outside(), &mut meter())
        .unwrap()
}

/// `body`'s rows published by the names `rename` gives.
fn renamed(body: &Body, rename: impl FnOnce(&mut Names)) -> Body {
    let mut names = body.names().unwrap().clone();
    rename(&mut names);
    Builder {
        arenas: body.arenas().clone(),
    }
    .publish_named(&names, body.kind(), body.outside(), &mut meter())
    .unwrap()
}

#[test]
fn name_tables_are_compared_by_their_rows() {
    let body = named_wall(1);
    let before = first(&body);
    let again = next(&before, &named_wall(1));
    assert!(
        again
            .written()
            .iter()
            .all(|r| table_of(r) < table::REGION_NAMES)
    );
    // A new last name, so the arenas keep their order and one name table's chunk changes.
    let edited = renamed(&body, |n| {
        let last = n.vertices.len() - 1;
        n.vertices[last] = key(200, 0);
    });
    let version = next(&before, &edited);
    let names: Vec<u64> = version
        .written()
        .iter()
        .map(table_of)
        .filter(|t| *t >= table::REGION_NAMES)
        .collect();
    assert_eq!(names, [table::VERTEX_NAMES]);
}

#[test]
fn every_name_table_is_compared_with_its_own() {
    let body = named_wall(1);
    let before = first(&body);
    let edits: [(u64, NameEdit); 4] = [
        (table::REGION_NAMES, |n| n.regions[1] = key(90, 0)),
        (table::FACE_NAMES, |n| n.faces[5] = key(91, 0)),
        (table::EDGE_NAMES, |n| n.edges[11] = key(92, 0)),
        (table::VERTEX_NAMES, |n| n.vertices[7] = key(93, 0)),
    ];
    for (code, edit) in edits {
        let version = next(&before, &renamed(&body, edit));
        let names: Vec<u64> = version
            .written()
            .iter()
            .map(table_of)
            .filter(|t| *t >= table::REGION_NAMES)
            .collect();
        assert_eq!(names, [code]);
    }
}

#[test]
fn names_come_and_go_between_versions() {
    let named = named_wall(1);
    let bare = published(&named, named.arenas().clone());
    let before = first(&bare);
    let version = next(&before, &named);
    let tables = version
        .written()
        .iter()
        .filter(|r| table_of(r) >= table::REGION_NAMES)
        .count();
    assert_eq!(tables, 4);
    let back = next(&version, &bare);
    assert_eq!(back.written(), []);
    assert_eq!(back.content_id(), before.content_id());
}

#[test]
fn a_bodys_geometry_id_does_not_see_its_names() {
    let named = named_wall(1);
    let bare = published(&named, named.arenas().clone());
    assert_eq!(first(&bare).geometry_id(), &first(&bare).content_id());
    assert_eq!(first(&named).geometry_id(), first(&bare).geometry_id());
    assert_ne!(first(&named).content_id(), first(&bare).content_id());
    // Other names in the same order: another body, the same geometry.
    let renamed_wall = named_wall(5);
    assert_eq!(
        first(&renamed_wall).geometry_id(),
        first(&named).geometry_id()
    );
    assert_ne!(
        first(&renamed_wall).content_id(),
        first(&named).content_id()
    );
}

#[test]
fn a_bodys_geometry_id_sees_its_geometry() {
    let body = named_wall(1);
    let mut arenas = body.arenas().clone();
    arenas
        .points
        .set(ix(0), PointKind::Grid(P3::new(5, 6, 7).unwrap()))
        .unwrap();
    let names = body.names().unwrap().clone();
    let moved = Builder { arenas }
        .publish_named(&names, body.kind(), body.outside(), &mut meter())
        .unwrap();
    assert_ne!(first(&moved).geometry_id(), first(&body).geometry_id());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Whatever the edits, a chain of versions, each compared only with the one before, has the
    /// IDs a full write of each body has; and the chunks each compares with stay alive though the
    /// bodies are gone.
    #[test]
    fn a_chain_of_versions_is_a_chain_of_full_writes(
        steps in prop::collection::vec(prop::collection::vec(0u32..9_000, 1..4), 1..6)
    ) {
        let mut body = many_faces(9_000);
        let mut version = first(&body);
        for rows in steps {
            let mut arenas = body.arenas().clone();
            for row in rows {
                let now = *arenas.faces.surface.get(ix(row)).unwrap();
                arenas.faces.surface.set(ix(row), other(now)).unwrap();
            }
            let edited = published(&body, arenas);
            drop(body);
            version = next(&version, &edited);
            body = edited;
        }
    }
}
