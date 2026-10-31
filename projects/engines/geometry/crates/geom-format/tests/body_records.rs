//! A body stored as records reads back index for index, its chunks are geom-topo's own runs so an
//! edit rewrites only the chunks it touches, the next operation on the body read back gives the
//! same bits, and records that are not the body's are refused (G94).

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

use std::collections::BTreeSet;

use geom_format::body::{
    BODY_TAG, BodyError, CHUNK_ROWS, CHUNK_TAG, SCHEMA, StoredBody, read_body, write_body,
};
use geom_format::cbor::{Decoder, Item};
use geom_format::content::ContentId;
use geom_format::kinds::{write_curve, write_surface};
use geom_format::record::{COPY_ALWAYS, REQUIRED, Record};
use geom_math::{Axis, Transform, TypedTransform};
use geom_op::cbor::Encoder;
use geom_topo::order::{NameKey, Names};
use geom_topo::{Body, BodyKind, Builder};

use topo::{ix, many_faces, meter, wall, wall_builder, wall_outside, wall_rows};

fn store(body: &Body) -> StoredBody {
    write_body(body, &mut meter()).unwrap()
}

fn ids(s: &StoredBody) -> BTreeSet<ContentId> {
    s.chunks.iter().map(Record::content_id).collect()
}

fn key(arena: u8, row: usize) -> NameKey {
    let mut k = [0u8; 16];
    k[0] = arena;
    k[8..].copy_from_slice(&(row as u64).to_be_bytes());
    NameKey(k)
}

fn named_wall() -> Body {
    let r = wall_rows();
    let names = Names {
        regions: (0..r.regions.len()).map(|i| key(1, i)).collect(),
        faces: (0..r.faces.len()).map(|i| key(3, i)).collect(),
        edges: (0..r.edges.len()).map(|i| key(6, i)).collect(),
        vertices: (0..r.vertices.len()).map(|i| key(7, i)).collect(),
    };
    wall_builder(true)
        .publish_named(&names, BodyKind::Solid, wall_outside(), &mut meter())
        .unwrap()
}

#[test]
fn a_body_reads_back_index_for_index() {
    for body in [wall(), named_wall(), many_faces(3)] {
        let stored = store(&body);
        assert_eq!(stored.index.tag, BODY_TAG);
        assert!(stored.chunks.iter().all(|c| c.tag == CHUNK_TAG));
        let back = read_body(&stored, &mut meter()).unwrap();
        assert_eq!(back, body);
        assert_eq!(back.names(), body.names());
        // Written again, the records are the same bytes: the content ID is a function of the body.
        assert_eq!(store(&back), stored);
        assert_eq!(store(&back).content_id(), stored.content_id());
        // The chunks may come in any order.
        let mut shuffled = stored.clone();
        shuffled.chunks.reverse();
        assert_eq!(read_body(&shuffled, &mut meter()).unwrap(), body);
    }
}

#[test]
fn a_named_body_lists_its_names_and_another_does_not() {
    assert_eq!(
        store(&named_wall()).chunks.len(),
        store(&wall()).chunks.len() + 4
    );
}

#[test]
fn tables_are_cut_into_geom_topos_runs() {
    let n = 2 * CHUNK_ROWS + 1808;
    let body = many_faces(n);
    let stored = store(&body);
    // Regions, shells, faces (three runs), surfaces; the other tables are empty and have none.
    assert_eq!(stored.chunks.len(), 1 + 1 + 3 + 1);
    assert_eq!(read_body(&stored, &mut meter()).unwrap(), body);
}

#[test]
fn a_one_percent_edit_rewrites_only_the_chunks_it_touches() {
    let n = 10_000_u32;
    let body = many_faces(n);
    let mut arenas = body.arenas().clone();
    // 1% of the faces, all in the second run, move to the other surface.
    for row in 5_000..5_100 {
        arenas.faces.surface.set(ix(row), ix(1)).unwrap();
    }
    let edited = Builder { arenas }
        .publish(body.kind(), body.outside(), &mut meter())
        .unwrap();
    let (before, after) = (store(&body), store(&edited));
    let changed: Vec<_> = ids(&after).difference(&ids(&before)).cloned().collect();
    assert_eq!(changed.len(), 1, "only the faces' second chunk is new");
    assert_eq!(ids(&before).difference(&ids(&after)).count(), 1);
    assert_ne!(after.content_id(), before.content_id());
    assert_eq!(read_body(&after, &mut meter()).unwrap(), edited);
}

#[test]
fn the_next_operation_on_a_body_read_back_gives_the_same_bits() {
    let ops = [
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap()),
    ];
    for body in [wall(), named_wall()] {
        let back = read_body(&store(&body), &mut meter()).unwrap();
        for t in &ops {
            let bytes = |b: &Body| {
                let mut out = Vec::new();
                for s in b.arenas().surfaces.iter() {
                    let mut e = Encoder::new();
                    write_surface(
                        &mut e,
                        &geom_surface::ParamSurface::transformed(s, t, &mut meter())
                            .unwrap()
                            .value,
                    )
                    .unwrap();
                    out.extend(e.finish().unwrap().into_bytes());
                }
                for c in b.arenas().curves.iter() {
                    let mut e = Encoder::new();
                    write_curve(
                        &mut e,
                        &geom_curve::ParamCurve::transformed(c, t, &mut meter())
                            .unwrap()
                            .value,
                    )
                    .unwrap();
                    out.extend(e.finish().unwrap().into_bytes());
                }
                out
            };
            assert_eq!(bytes(&back), bytes(&body));
        }
    }
}

#[test]
fn records_that_are_not_the_bodys_are_refused() {
    let stored = store(&wall());
    // A chunk's byte changed: its content ID is no longer the one listed.
    let mut s = stored.clone();
    let last = s.chunks[2].payload.len() - 1;
    s.chunks[2].payload[last] ^= 1;
    assert!(matches!(
        read_body(&s, &mut meter()),
        Err(BodyError::Chunk { .. })
    ));
    // A chunk missing.
    let mut s = stored.clone();
    s.chunks.remove(0);
    assert!(matches!(
        read_body(&s, &mut meter()),
        Err(BodyError::Chunk { .. })
    ));
    // A chunk the index does not list, and one given twice.
    let other = store(&many_faces(5));
    let mut s = stored.clone();
    s.chunks.push(other.chunks[2].clone());
    assert!(matches!(
        read_body(&s, &mut meter()),
        Err(BodyError::Shape(_))
    ));
    let mut s = stored.clone();
    s.chunks.push(stored.chunks[0].clone());
    assert!(matches!(
        read_body(&s, &mut meter()),
        Err(BodyError::Shape(_))
    ));
    // Dependencies other than the chunks.
    let mut s = stored.clone();
    s.index.depends.pop();
    assert!(matches!(
        read_body(&s, &mut meter()),
        Err(BodyError::Shape(_))
    ));
    // Another record type, schema or flags for the index or a chunk.
    for change in 0..3 {
        let mut s = stored.clone();
        match change {
            0 => s.index.tag = CHUNK_TAG,
            1 => s.index.schema = 2,
            _ => s.index.flags |= COPY_ALWAYS,
        }
        assert!(
            matches!(read_body(&s, &mut meter()), Err(BodyError::Shape(_))),
            "index {change}"
        );
        let mut s = stored.clone();
        match change {
            0 => s.chunks[1].tag = BODY_TAG,
            1 => s.chunks[1].schema = 2,
            _ => s.chunks[1].flags |= COPY_ALWAYS,
        }
        // The chunk's ID is of its payload alone, so it is still found, and refused as the wrong one.
        assert!(
            matches!(read_body(&s, &mut meter()), Err(BodyError::Chunk { .. })),
            "chunk {change}"
        );
    }
    // Another body's index over this body's chunks.
    let mut s = stored.clone();
    s.index = other.index.clone();
    assert!(read_body(&s, &mut meter()).is_err());
}

/// A chunk's table and place, as its payload names them.
fn place(r: &Record) -> (u64, u64) {
    let mut d = Decoder::new(&r.payload, 4);
    let mut m = meter();
    d.next(&mut m).unwrap();
    let mut uint = || match d.next(&mut m).unwrap() {
        Some(Item::Uint(n)) => n,
        other => panic!("{other:?}"),
    };
    (uint(), uint())
}

/// The chunks of table `code`, in order.
fn table(s: &StoredBody, code: u64) -> Vec<Record> {
    let mut v: Vec<Record> = s
        .chunks
        .iter()
        .filter(|r| place(r).0 == code)
        .cloned()
        .collect();
    v.sort_by_key(|r| place(r).1);
    v
}

/// An index written by hand over `tables` (code, rows, chunks), with `extra` items after it.
fn forge(
    kind: u64,
    outside: u64,
    tables: &[(u64, u64, Vec<Record>)],
    named: u64,
    extra: bool,
) -> StoredBody {
    let mut e = Encoder::new();
    e.array(5).unwrap();
    e.uint(kind).unwrap();
    e.uint(outside).unwrap();
    e.array(tables.len() as u64).unwrap();
    let mut chunks = Vec::new();
    let mut ids = Vec::new();
    for (code, rows, records) in tables {
        e.array(3).unwrap();
        e.uint(*code).unwrap();
        e.uint(*rows).unwrap();
        e.array(records.len() as u64).unwrap();
        for r in records {
            let id = r.content_id();
            e.item(&id.item().unwrap()).unwrap();
            ids.push(id);
            chunks.push(r.clone());
        }
    }
    e.uint(named).unwrap();
    e.map(0).unwrap();
    let mut payload = e.finish().unwrap().into_bytes();
    if extra {
        payload.push(0);
    }
    ids.sort_by_key(|id| id.item().unwrap().into_bytes());
    ids.dedup();
    StoredBody {
        index: Record {
            tag: BODY_TAG,
            flags: REQUIRED,
            schema: SCHEMA,
            depends: ids,
            payload,
        },
        chunks,
    }
}

/// The tables of a stored body of `rows` rows per table, as the writer lists them.
fn tables_of(s: &StoredBody, rows: &[(u64, u64)]) -> Vec<(u64, u64, Vec<Record>)> {
    rows.iter()
        .map(|&(code, n)| (code, n, table(s, code)))
        .collect()
}

/// `many_faces(n)`'s tables: one solid and one void region, one shell, n faces, two surfaces.
fn many(n: u64) -> [(u64, u64); 11] {
    [
        (1, 2),
        (2, 1),
        (3, n),
        (4, 0),
        (5, 0),
        (6, 0),
        (7, 0),
        (8, 0),
        (9, 2),
        (10, 0),
        (11, 0),
    ]
}

#[test]
fn an_index_written_by_hand_reads_as_the_writers() {
    let stored = store(&many_faces(10));
    assert_eq!(
        forge(1, 2, &tables_of(&stored, &many(10)), 0, false),
        stored
    );
}

#[test]
fn an_index_that_does_not_fit_its_chunks_is_refused() {
    let stored = store(&many_faces(10));
    let shape = |s: &StoredBody| matches!(read_body(s, &mut meter()), Err(BodyError::Shape(_)));
    // Rows other than the chunks hold.
    let mut t = tables_of(&stored, &many(10));
    t[2].1 = 11;
    assert!(shape(&forge(1, 2, &t, 0, false)));
    // A table missing, given twice, or out of order.
    let t = tables_of(&stored, &many(10));
    let mut missing = t.clone();
    missing.pop();
    assert!(shape(&forge(1, 2, &missing, 0, false)));
    let mut twice = t.clone();
    twice.push(t[10].clone());
    assert!(shape(&forge(1, 2, &twice, 0, false)));
    let mut swapped = t.clone();
    swapped.swap(3, 4);
    assert!(shape(&forge(1, 2, &swapped, 0, false)));
    // Names said and not given; a named flag, a kind or an outside region out of range.
    assert!(shape(&forge(1, 2, &t, 1, false)));
    assert!(shape(&forge(1, 2, &t, 2, false)));
    assert!(shape(&forge(5, 2, &t, 0, false)));
    assert!(shape(&forge(1, 0, &t, 0, false)));
    // Bytes after the index.
    assert!(shape(&forge(1, 2, &t, 0, true)));
    // Each body kind the index can name reads, as far as the builder allows.
    for kind in 0..5 {
        let s = forge(kind, 2, &t, 0, false);
        assert!(
            !matches!(read_body(&s, &mut meter()), Err(BodyError::Shape(_))),
            "{kind}"
        );
    }
}

#[test]
fn chunks_are_whole_runs_in_their_places() {
    let one = store(&many_faces(1));
    let big = store(&many_faces(2 * CHUNK_ROWS));
    // 4,097 faces as a run of 1 and a run of 4,096: the first run is not full.
    let mut t = tables_of(&one, &many(1));
    t[2] = (
        3,
        u64::from(CHUNK_ROWS) + 1,
        vec![table(&one, 3)[0].clone(), table(&big, 3)[1].clone()],
    );
    assert!(matches!(
        read_body(&forge(1, 2, &t, 0, false), &mut meter()),
        Err(BodyError::Chunk { table: 3, chunk: 0 })
    ));
    // Two runs listed in each other's places.
    let n = 2 * u64::from(CHUNK_ROWS);
    let mut t = tables_of(&big, &many(n));
    t[2].2.swap(0, 1);
    assert!(matches!(
        read_body(&forge(1, 2, &t, 0, false), &mut meter()),
        Err(BodyError::Chunk { table: 3, chunk: 0 })
    ));
    // A run listed under another table.
    let mut t = tables_of(&big, &many(n));
    t[0].2 = t[1].2.clone();
    assert!(matches!(
        read_body(&forge(1, 2, &t, 0, false), &mut meter()),
        Err(BodyError::Chunk { table: 1, chunk: 0 })
    ));
    // A chunk with a byte after it.
    let mut t = tables_of(&one, &many(1));
    let mut r = t[1].2[0].clone();
    r.payload.push(0);
    t[1].2 = vec![r];
    assert!(matches!(
        read_body(&forge(1, 2, &t, 0, false), &mut meter()),
        Err(BodyError::Shape(_))
    ));
}
