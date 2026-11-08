//! The benchmark behind 32.45: should chunks be cut by name (content-defined buckets, a bucket
//! ending where a row's name meets a fixed condition) so that one inserted face leaves the other
//! chunks alone, as 31.7 asks of piece G105?
//!
//! It measures what any cut would have to rewrite, not one guess at a cut. A body is a strip of
//! quads published by name (32.17), one more quad is added with a name in the middle of the order,
//! and every row of the new version whose values no row of the old one has is marked changed. A
//! scheme can keep a run of rows only if none is changed, and a bucket scheme with a mean of
//! `CHUNK_ROWS` rows keeps its buckets above a quarter of that, the usual floor of a
//! content-defined cut. So the rows outside clean runs of at least that length are a lower bound
//! on what any such scheme rewrites, and the test compares it with what the fixed runs rewrite,
//! checked against the real chunks' content IDs. Counts, not time: the same on every machine.

#![allow(
    dead_code,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unwrap_used,
    clippy::panic,
    reason = "tests: native arithmetic and indexing, and a failure stops the test"
)]

#[path = "../../geom-topo/tests/common/mod.rs"]
mod topo;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Write};

use geom_format::body::{BodyVersion, CHUNK_ROWS, DEPTH, StoredBody, table, write_body};
use geom_format::cbor::{Decoder, Item};
use geom_format::content::ContentId;
use geom_format::record::Record;
use geom_math::Box3;
use geom_num::grid::P3;
use geom_op::hash::sha256;
use geom_topo::order::{NameKey, Names};
use geom_topo::{
    Body, BodyKind, Builder, Column, Edge, Face, Fin, Loop, PointKind, Region, RegionKind, Sense,
    Shell, Vertex,
};

use topo::{ix, line, meter, plane};

/// The quads of the strip: past the chunk size in every table, as a building-scale body is.
const QUADS: u32 = 20_000;
/// The shortest bucket a content-defined cut keeps: a quarter of the mean.
const SHORTEST: usize = (CHUNK_ROWS / 4) as usize;

fn name(kind: u8, row: u32) -> NameKey {
    let mut bytes = [0u8; 5];
    bytes[0] = kind;
    bytes[1..].copy_from_slice(&row.to_be_bytes());
    let digest = sha256(&bytes);
    let mut key = [0u8; 16];
    key.copy_from_slice(&digest[..16]);
    NameKey(key)
}

/// A strip of `n` unit quads along x, each on a plane of its own, with shared rungs and named by
/// hashes of their rows, so name order is unrelated to position, as a real body's is. Quad `n - 1`
/// takes the name `last`, when given.
fn strip(n: u32, last: Option<NameKey>) -> Body {
    let mut b = Builder::new();
    let a = &mut b.arenas;
    a.regions
        .extend(&[
            Region {
                kind: RegionKind::Solid,
                first_shell: Some(ix(0)),
            },
            Region {
                kind: RegionKind::Void,
                first_shell: None,
            },
        ])
        .unwrap();
    a.shells
        .push(Shell {
            region: ix(0),
            next: None,
            first_face: Some(ix(0)),
        })
        .unwrap();
    let bottom = |q: u32| 2 * q;
    let top = |q: u32| 2 * q + 1;
    let (bottom_edge, top_edge) = (|q: u32| 3 * q, |q: u32| 3 * q + 1);
    let rung = |q: u32| if q == n { 3 * n } else { 3 * q + 2 };
    let mut faces = Vec::new();
    let mut loops = Vec::new();
    let mut fins = Vec::new();
    for q in 0..n {
        let x = i64::from(q);
        a.surfaces
            .push(plane([f64::from(q), 0.0, 0.0], 0, 1))
            .unwrap();
        faces.push(Face {
            shell: ix(0),
            next: (q + 1 < n).then(|| ix(q + 1)),
            surface: ix(q),
            sense: Sense::Forward,
            first_loop: Some(ix(q)),
            bound: Box3::new(P3::new(x, 0, 0).unwrap(), P3::new(x + 1, 1, 0).unwrap()).unwrap(),
        });
        loops.push(Loop {
            face: ix(q),
            next: None,
            first_fin: Some(ix(4 * q)),
        });
        let ring = [
            (bottom_edge(q), Sense::Forward),
            (rung(q + 1), Sense::Forward),
            (top_edge(q), Sense::Reversed),
            (rung(q), Sense::Reversed),
        ];
        for (k, (edge, sense)) in ring.into_iter().enumerate() {
            let k = k as u32;
            let fin = 4 * q + k;
            let radial = match k {
                1 if q + 1 < n => 4 * (q + 1) + 3,
                3 if q > 0 => 4 * (q - 1) + 1,
                _ => fin,
            };
            fins.push(Fin {
                loop_: ix(q),
                next: ix(4 * q + (k + 1) % 4),
                prev: ix(4 * q + (k + 3) % 4),
                edge: ix(edge),
                sense,
                radial: ix(radial),
                pcurve: None,
            });
        }
    }
    a.faces.extend(&faces).unwrap();
    a.loops.extend(&loops).unwrap();
    a.fins.extend(&fins).unwrap();
    let mut edges = Vec::new();
    let mut edge = |from: u32, to: u32, fin: u32, row: u32| {
        edges.push((
            row,
            Edge {
                first_fin: Some(ix(fin)),
                start: Some(ix(from)),
                end: Some(ix(to)),
                curve: ix(row),
            },
        ));
    };
    for q in 0..n {
        edge(bottom(q), bottom(q + 1), 4 * q, bottom_edge(q));
        edge(top(q), top(q + 1), 4 * q + 2, top_edge(q));
        edge(bottom(q), top(q), 4 * q + 3, rung(q));
    }
    edge(bottom(n), top(n), 4 * (n - 1) + 1, rung(n));
    edges.sort_by_key(|(row, _)| *row);
    for (_, e) in &edges {
        let start = e.start.unwrap().row();
        let end = e.end.unwrap().row();
        let ends = [start, end].map(|v| i64::from(v / 2));
        let heights = [start, end].map(|v| i64::from(v % 2));
        a.curves
            .push(line([ends[0], heights[0], 0], [ends[1], heights[1], 0]))
            .unwrap();
        a.edges.push(*e).unwrap();
    }
    for v in 0..2 * (n + 1) {
        a.points
            .push(PointKind::Grid(
                P3::new(i64::from(v / 2), i64::from(v % 2), 0).unwrap(),
            ))
            .unwrap();
        a.vertices.push(Vertex { point: ix(v) }).unwrap();
    }
    let mut face_names: Vec<NameKey> = (0..n).map(|q| name(3, q)).collect();
    if let Some(last) = last {
        face_names[(n - 1) as usize] = last;
    }
    let names = Names {
        regions: (0..2).map(|r| name(1, r)).collect(),
        faces: face_names,
        edges: (0..=3 * n).map(|e| name(6, e)).collect(),
        vertices: (0..2 * (n + 1)).map(|v| name(7, v)).collect(),
    };
    b.publish_named(&names, BodyKind::Sheet, ix(1), &mut meter())
        .unwrap()
}

/// Adds a column's values to its table's rows.
fn add<K, V: Debug>(rows: &mut Vec<String>, column: &Column<K, V>) {
    for (row, value) in column.iter().enumerate() {
        if row == rows.len() {
            rows.push(String::new());
        }
        write!(rows[row], "{value:?},").unwrap();
    }
}

/// Every row of every table but the two small ones, by table, as the values the chunks encode.
fn tables(body: &Body) -> Vec<(&'static str, u64, Vec<String>)> {
    let a = body.arenas();
    let names = body.names().unwrap();
    let mut out = Vec::new();
    let mut table = |label, code, columns: &mut dyn FnMut(&mut Vec<String>)| {
        let mut rows = Vec::new();
        columns(&mut rows);
        out.push((label, code, rows));
    };
    table("faces", table::FACES, &mut |r| {
        add(r, &a.faces.shell);
        add(r, &a.faces.next);
        add(r, &a.faces.surface);
        add(r, &a.faces.sense);
        add(r, &a.faces.first_loop);
        add(r, &a.faces.bound);
    });
    table("loops", table::LOOPS, &mut |r| {
        add(r, &a.loops.face);
        add(r, &a.loops.next);
        add(r, &a.loops.first_fin);
    });
    table("fins", table::FINS, &mut |r| {
        add(r, &a.fins.loop_);
        add(r, &a.fins.next);
        add(r, &a.fins.prev);
        add(r, &a.fins.edge);
        add(r, &a.fins.sense);
        add(r, &a.fins.radial);
        add(r, &a.fins.pcurve);
    });
    table("edges", table::EDGES, &mut |r| {
        add(r, &a.edges.first_fin);
        add(r, &a.edges.start);
        add(r, &a.edges.end);
        add(r, &a.edges.curve);
    });
    table("vertices", table::VERTICES, &mut |r| {
        add(r, &a.vertices.point);
    });
    table("points", table::POINTS, &mut |r| add(r, &a.points));
    table("surfaces", table::SURFACES, &mut |r| add(r, &a.surfaces));
    table("curves", table::CURVES, &mut |r| add(r, &a.curves));
    table("face names", table::FACE_NAMES, &mut |r| {
        r.extend(names.faces.iter().map(|k| format!("{k:?}")));
    });
    table("edge names", table::EDGE_NAMES, &mut |r| {
        r.extend(names.edges.iter().map(|k| format!("{k:?}")));
    });
    table("vert names", table::VERTEX_NAMES, &mut |r| {
        r.extend(names.vertices.iter().map(|k| format!("{k:?}")));
    });
    out
}

/// Which rows of `new` have values no row of `old` has, a row of `old` answering once.
fn changed(new: &[String], old: &[String]) -> Vec<bool> {
    let mut left: BTreeMap<&str, usize> = BTreeMap::new();
    for row in old {
        *left.entry(row).or_default() += 1;
    }
    new.iter()
        .map(|row| match left.get_mut(row.as_str()) {
            Some(n) if *n > 0 => {
                *n -= 1;
                false
            }
            _ => true,
        })
        .collect()
}

/// The rows no bucket scheme with buckets of at least [`SHORTEST`] rows can keep: all but those
/// in runs of that many unchanged rows.
fn any_bucket_scheme_rewrites(changed: &[bool]) -> usize {
    let kept: usize = changed
        .split(|c| *c)
        .map(<[bool]>::len)
        .filter(|run| *run >= SHORTEST)
        .sum();
    changed.len() - kept
}

/// The rows the fixed runs rewrite: every row of a chunk whose rows differ from the chunk at its
/// place in `old`.
fn fixed_runs_rewrite(new: &[String], old: &[String]) -> usize {
    let size = CHUNK_ROWS as usize;
    new.chunks(size)
        .zip(old.chunks(size).map(Some).chain(std::iter::repeat(None)))
        .filter(|(n, o)| *o != Some(*n))
        .map(|(n, _)| n.len())
        .sum()
}

/// A chunk record's table, place and rows.
fn header_of(record: &Record) -> [u64; 3] {
    let mut d = Decoder::new(&record.payload, DEPTH);
    let mut m = meter();
    d.next(&mut m).unwrap();
    [0; 3].map(|_| match d.next(&mut m).unwrap() {
        Some(Item::Uint(n)) => n,
        other => panic!("{other:?}"),
    })
}

fn table_of(record: &Record) -> u64 {
    header_of(record)[0]
}

fn rows_of(record: &Record) -> usize {
    header_of(record)[2] as usize
}

fn ids(s: &StoredBody) -> BTreeSet<ContentId> {
    s.chunks.iter().map(Record::content_id).collect()
}

/// The bytes of the chunks of table `code`.
fn table_bytes(stored: &StoredBody, code: u64) -> usize {
    stored
        .chunks
        .iter()
        .filter(|r| table_of(r) == code)
        .map(|r| r.payload.len())
        .sum()
}

/// What the benchmark found, for one edit: rows in all and changed, then the rows any bucket
/// scheme must rewrite and the rows the fixed runs rewrite, and the last two in bytes, a table's
/// rows counted at its mean bytes.
#[derive(Default, Debug)]
struct Totals {
    rows: usize,
    changed: usize,
    bucket: usize,
    fixed: usize,
    bytes: usize,
    bucket_bytes: usize,
    fixed_bytes: usize,
}

fn measure(old: &Body, new: &Body) -> Totals {
    let stored = write_body(new, &mut meter()).unwrap();
    let mut totals = Totals::default();
    println!(
        "{:<10} {:>8} {:>8} {:>8} {:>8} {:>10} {:>10} {:>10}",
        "table", "rows", "changed", "bucket", "fixed", "bytes", "bucket", "fixed"
    );
    let before = tables(old);
    for ((label, code, new_rows), (_, _, old_rows)) in tables(new).iter().zip(before.iter()) {
        let marks = changed(new_rows, old_rows);
        let bytes = table_bytes(&stored, *code);
        let line = Totals {
            rows: new_rows.len(),
            changed: marks.iter().filter(|c| **c).count(),
            bucket: any_bucket_scheme_rewrites(&marks),
            fixed: fixed_runs_rewrite(new_rows, old_rows),
            bytes,
            ..Totals::default()
        };
        let share = |rows: usize| (rows as u128 * bytes as u128 / line.rows as u128) as usize;
        let (bucket_bytes, fixed_bytes) = (share(line.bucket), share(line.fixed));
        println!(
            "{label:<10} {:>8} {:>8} {:>8} {:>8} {bytes:>10} {bucket_bytes:>10} {fixed_bytes:>10}",
            line.rows, line.changed, line.bucket, line.fixed
        );
        totals.rows += line.rows;
        totals.changed += line.changed;
        totals.bucket += line.bucket;
        totals.fixed += line.fixed;
        totals.bytes += bytes;
        totals.bucket_bytes += bucket_bytes;
        totals.fixed_bytes += fixed_bytes;
    }
    println!("{totals:?}");
    totals
}

/// The name halfway through the order of the strip's faces: a new face there shifts half of them.
fn middle_name(body: &Body) -> NameKey {
    let names = body.names().unwrap();
    let mut key = names.faces[names.faces.len() / 2];
    key.0[15] ^= 1;
    key
}

#[test]
#[cfg_attr(
    miri,
    ignore = "20,000 quads: too slow under Miri, and it holds no unsafe"
)]
fn one_inserted_face() {
    let old = strip(QUADS, None);
    let new = strip(QUADS + 1, Some(middle_name(&old)));
    let totals = measure(&old, &new);

    // The harness's fixed count is what the real chunks do: count the rows of the chunks whose
    // content IDs the old version does not hold, which also counts the two small tables.
    let old_stored = write_body(&old, &mut meter()).unwrap();
    let new_stored = write_body(&new, &mut meter()).unwrap();
    let held = ids(&old_stored);
    let real: usize = new_stored
        .chunks
        .iter()
        .filter(|r| !held.contains(&r.content_id()))
        .map(rows_of)
        .sum();
    assert!(
        real >= totals.fixed && real <= totals.fixed + 3,
        "the real chunks rewrite {real} rows, the harness says {}",
        totals.fixed
    );

    // The numbers of 32.45. Row links change nearly every row of the faces, loops, fins and edges,
    // so about a third of the bytes go whatever the cut. Fixed runs also rewrite the pools and the
    // name tables, whose rows only shift: up to 59% of what they rewrite could stay. A format
    // change that moves these moves the decision.
    assert!(totals.fixed_bytes * 100 >= totals.bytes * 75, "{totals:?}");
    assert!(totals.fixed_bytes * 100 <= totals.bytes * 80, "{totals:?}");
    assert!(totals.bucket_bytes * 100 >= totals.bytes * 30, "{totals:?}");
    assert!(totals.bucket_bytes * 100 <= totals.bytes * 35, "{totals:?}");
}

/// An edit in place moves no row, and there the fixed runs are already as small as a cut can be.
#[test]
#[cfg_attr(
    miri,
    ignore = "20,000 quads: too slow under Miri, and it holds no unsafe"
)]
fn an_edit_in_place_rewrites_one_chunk() {
    let old = strip(QUADS, None);
    let mut arenas = old.arenas().clone();
    let far = Box3::new(P3::new(-9, -9, 0).unwrap(), P3::new(9, 9, 0).unwrap()).unwrap();
    arenas.faces.bound.set(ix(100), far).unwrap();
    let names = old.names().unwrap().clone();
    let new = Builder { arenas }
        .publish_named(&names, old.kind(), old.outside(), &mut meter())
        .unwrap();
    let totals = measure(&old, &new);
    assert_eq!(totals.changed, 1);
    assert_eq!(totals.fixed, CHUNK_ROWS as usize);
    // And a new version finds that out without reading the rest: only that chunk is hashed.
    let first = BodyVersion::first(&old, &mut meter()).unwrap();
    let next = first.next(&new, &mut meter()).unwrap();
    assert_eq!(next.written().len(), 1);
    assert_eq!(
        next.content_id(),
        write_body(&new, &mut meter()).unwrap().content_id()
    );
}
