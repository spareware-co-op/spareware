//! Body records: a body stored as one index record and a chunk record for every 4,096-row run of
//! each of its tables, so a body's content ID is the root of a hash tree and a new version writes
//! only the chunks that changed (24.2, 24.3).
//!
//! What it does: [`write_body`] turns a `geom_topo::Body` into a [`StoredBody`]: one chunk record
//! (type 4) per run of [`CHUNK_ROWS`] rows of each table, in arena order, and the index record
//! (type 3) listing every table's row count and its chunks' content IDs in order. [`read_body`]
//! reads them back through `geom_topo::Builder`, which checks the topology again before it
//! publishes, so a stored body is never trusted further than a built one. The tables, by code in
//! [`table`], are the arenas of chapter 8.2: regions, shells, faces, loops, fins, edges and
//! vertices; the geometry pools (points, surfaces, curves and pcurves); and, for a body published
//! by name, the names of its regions, faces, edges and vertices.
//!
//! Guarantees:
//!
//! - **Index for index.** A body read back equals the one written (`PartialEq`), every arena in
//!   the same order, so the next operation on either gives the same bits (24.3).
//! - **Chunks are a pure function of their rows.** The runs are geom-topo's own chunks
//!   (`geom_topo::CHUNK_ROWS`, 4,096), so a chunk whose rows did not change encodes to the same
//!   bytes and the same content ID, whatever else changed: an edit to 1% of a body's rows rewrites
//!   only the chunks those rows are in.
//! - **References are checked.** Every reference is stored as its row plus one, 0 for none; a
//!   required reference of 0, a row count that disagrees with the chunks, a chunk of the wrong table
//!   or position, or a content ID that does not match its chunk is refused before anything is
//!   built, and the builder then checks the topology itself.
//!
//! Method: chapter 24.2 to 24.4 of the kernel architecture,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`, and the arenas of chapter
//! 8.2, `projects/engines/geometry/architecture/08-topology.md`; Merkle's hash tree,
//! <https://doi.org/10.1007/3-540-48184-2_32>, for the content ID of the index over its chunks.
//!
//! Cost: linear in the body: one pass over each table to write, one to read, one SHA-256 pass over
//! every chunk; memory for one chunk's payload at a time plus the body itself.

#![allow(
    clippy::many_single_char_names,
    reason = "e the encoder, d the decoder, a the arenas; f, l and n the face, loop and fin tables"
)]

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

use geom_math::Box3;
use geom_num::grid::P3;
use geom_op::cbor::{EncodeError, Encoder};
use geom_run::{Stopped, WorkMeter};
use geom_topo::order::{NameKey, Names};
use geom_topo::{
    Arenas, Body, BodyKind, Builder, Column, Edge, Face, Fin, Ix, Loop, Region, RegionKind, Sense,
    Shell, TopoError, Vertex,
};

use crate::cbor::{CborError, Decoder, Item, int_column};
use crate::content::ContentId;
use crate::kinds::{self, KindError};
use crate::record::{REQUIRED, Record};

/// The record type of a body's index.
pub const BODY_TAG: u64 = 3;
/// The record type of one chunk of one of a body's tables.
pub const CHUNK_TAG: u64 = 4;
/// The schema of both.
pub const SCHEMA: u64 = 1;
/// The rows of every chunk but a table's last: geom-topo's own chunks.
pub const CHUNK_ROWS: u32 = geom_topo::CHUNK_ROWS;

/// The table codes, in the order the index lists them.
pub mod table {
    /// Regions: kind, first shell.
    pub const REGIONS: u64 = 1;
    /// Shells: region, next, first face.
    pub const SHELLS: u64 = 2;
    /// Faces: shell, next, surface, sense, first loop, bound.
    pub const FACES: u64 = 3;
    /// Loops: face, next, first fin.
    pub const LOOPS: u64 = 4;
    /// Fins: loop, next, previous, edge, sense, radial, pcurve.
    pub const FINS: u64 = 5;
    /// Edges: first fin, start, end, curve.
    pub const EDGES: u64 = 6;
    /// Vertices: point.
    pub const VERTICES: u64 = 7;
    /// Points: x, y, z in grid steps.
    pub const POINTS: u64 = 8;
    /// Surfaces: one kind each.
    pub const SURFACES: u64 = 9;
    /// 3D curves: one kind each.
    pub const CURVES: u64 = 10;
    /// Parameter-space curves: one kind each.
    pub const PCURVES: u64 = 11;
    /// The names of regions, faces, edges and vertices, 16 bytes each.
    pub const REGION_NAMES: u64 = 12;
    /// Face names.
    pub const FACE_NAMES: u64 = 13;
    /// Edge names.
    pub const EDGE_NAMES: u64 = 14;
    /// Vertex names.
    pub const VERTEX_NAMES: u64 = 15;
}

/// A body as records: its index and its chunks, each chunk once, in the index's order.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredBody {
    /// The index record.
    pub index: Record,
    /// The chunk records.
    pub chunks: Vec<Record>,
}

impl StoredBody {
    /// The body's content ID: the index payload's, the root of the hash tree over its chunks.
    #[must_use]
    pub fn content_id(&self) -> ContentId {
        self.index.content_id()
    }
}

/// Why a body could not be stored or read back.
#[derive(Clone, Debug, PartialEq)]
pub enum BodyError {
    /// The encoder refused a value (writing).
    Encode(EncodeError),
    /// Bytes that break SKG's canonical CBOR.
    Cbor(CborError),
    /// A payload that is not what its table or the index needs.
    Shape(&'static str),
    /// A chunk the index does not list, or listed in the wrong place or with another content ID.
    Chunk {
        /// The table.
        table: u64,
        /// The chunk's position in its table.
        chunk: u64,
    },
    /// A geometry kind that could not be stored or read.
    Kind(KindError),
    /// The topology did not build back.
    Topo(TopoError),
    /// The meter stopped the work.
    Stopped(Stopped),
}

impl fmt::Display for BodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BodyError::Encode(e) => write!(f, "no encoding: {e}"),
            BodyError::Cbor(e) => write!(f, "not canonical CBOR: {e}"),
            BodyError::Shape(what) => write!(f, "not a body record: {what}"),
            BodyError::Chunk { table, chunk } => {
                write!(
                    f,
                    "chunk {chunk} of table {table} is missing or not the one listed"
                )
            }
            BodyError::Kind(e) => write!(f, "a geometry kind: {e}"),
            BodyError::Topo(e) => write!(f, "the topology did not build back: {e:?}"),
            BodyError::Stopped(s) => write!(f, "{s}"),
        }
    }
}

impl core::error::Error for BodyError {}

impl From<EncodeError> for BodyError {
    fn from(e: EncodeError) -> Self {
        BodyError::Encode(e)
    }
}

impl From<CborError> for BodyError {
    fn from(e: CborError) -> Self {
        BodyError::Cbor(e)
    }
}

impl From<KindError> for BodyError {
    fn from(e: KindError) -> Self {
        BodyError::Kind(e)
    }
}

impl From<TopoError> for BodyError {
    fn from(e: TopoError) -> Self {
        BodyError::Topo(e)
    }
}

impl From<Stopped> for BodyError {
    fn from(s: Stopped) -> Self {
        BodyError::Stopped(s)
    }
}

/// A reference as stored: its row plus one, 0 for none.
fn reference<K>(ix: Option<Ix<K>>) -> u64 {
    ix.map_or(0, |i| u64::from(i.row()).saturating_add(1))
}

/// A sense as stored: 0 forward, 1 reversed.
fn sense(s: Sense) -> u64 {
    match s {
        Sense::Forward => 0,
        Sense::Reversed => 1,
    }
}

/// Writes one column of one chunk as an array of unsigned integers.
fn uints<V>(e: &mut Encoder, rows: &[V], f: impl Fn(&V) -> u64) -> Result<(), EncodeError> {
    e.array(u64::try_from(rows.len()).unwrap_or(u64::MAX))?;
    for v in rows {
        e.uint(f(v))?;
    }
    Ok(())
}

/// The chunks of a column, as slices.
fn chunks_of<K, V>(column: &Column<K, V>) -> Vec<&[V]> {
    column.chunks().collect()
}

/// One chunk record of `table`, number `chunk`, with `rows` rows, its columns written by `columns`.
fn chunk_record(
    table: u64,
    chunk: usize,
    rows: usize,
    columns: u64,
    write: impl FnOnce(&mut Encoder) -> Result<(), BodyError>,
) -> Result<Record, BodyError> {
    let mut e = Encoder::new();
    e.array(4)?;
    e.uint(table)?;
    e.uint(u64::try_from(chunk).unwrap_or(u64::MAX))?;
    e.uint(u64::try_from(rows).unwrap_or(u64::MAX))?;
    e.array(columns)?;
    write(&mut e)?;
    Ok(Record {
        tag: CHUNK_TAG,
        flags: REQUIRED,
        schema: SCHEMA,
        depends: Vec::new(),
        payload: e.finish()?.into_bytes(),
    })
}

/// Every chunk of a table: `count` chunks of `width` columns, chunk `i` holding `rows_of(i)` rows,
/// its columns written by `write`; pushed onto `out` with the table's row count. Each chunk costs
/// the meter one unit.
fn table_chunks(
    table: u64,
    count: usize,
    width: u64,
    rows_of: impl Fn(usize) -> usize,
    mut write: impl FnMut(&mut Encoder, usize) -> Result<(), BodyError>,
    out: &mut Vec<(u64, u64, Vec<Record>)>,
    meter: &mut WorkMeter<'_>,
) -> Result<(), BodyError> {
    let mut records = Vec::with_capacity(count);
    let mut total: u64 = 0;
    for i in 0..count {
        meter.charge(1)?;
        let rows = rows_of(i);
        total = total.saturating_add(u64::try_from(rows).unwrap_or(u64::MAX));
        records.push(chunk_record(table, i, rows, width, |e| write(e, i))?);
    }
    out.push((table, total, records));
    Ok(())
}

/// The body as records.
///
/// # Errors
///
/// [`BodyError::Kind`] for a geometry kind that has no stored form, and the encoder's errors.
#[allow(
    clippy::too_many_lines,
    reason = "one short block per table, in the index's order"
)]
pub fn write_body(body: &Body, meter: &mut WorkMeter<'_>) -> Result<StoredBody, BodyError> {
    let a = body.arenas();
    let mut tables: Vec<(u64, u64, Vec<Record>)> = Vec::new();

    let kind = chunks_of(&a.regions.kind);
    let first_shell = chunks_of(&a.regions.first_shell);
    table_chunks(
        table::REGIONS,
        kind.len(),
        2,
        |i| kind.get(i).map_or(0, |c| c.len()),
        |e, i| {
            uints(e, kind.get(i).copied().unwrap_or_default(), |k| match k {
                RegionKind::Solid => 0,
                RegionKind::Void => 1,
            })?;
            uints(e, first_shell.get(i).copied().unwrap_or_default(), |r| {
                reference(*r)
            })?;
            Ok(())
        },
        &mut tables,
        meter,
    )?;

    let (region, next, first_face) = (
        chunks_of(&a.shells.region),
        chunks_of(&a.shells.next),
        chunks_of(&a.shells.first_face),
    );
    table_chunks(
        table::SHELLS,
        region.len(),
        3,
        |i| region.get(i).map_or(0, |c| c.len()),
        |e, i| {
            uints(e, region.get(i).copied().unwrap_or_default(), |r| {
                reference(Some(*r))
            })?;
            uints(e, next.get(i).copied().unwrap_or_default(), |r| {
                reference(*r)
            })?;
            uints(e, first_face.get(i).copied().unwrap_or_default(), |r| {
                reference(*r)
            })?;
            Ok(())
        },
        &mut tables,
        meter,
    )?;

    let f = &a.faces;
    let (shell, next, surface, fsense, first_loop, bound) = (
        chunks_of(&f.shell),
        chunks_of(&f.next),
        chunks_of(&f.surface),
        chunks_of(&f.sense),
        chunks_of(&f.first_loop),
        chunks_of(&f.bound),
    );
    table_chunks(
        table::FACES,
        shell.len(),
        6,
        |i| shell.get(i).map_or(0, |c| c.len()),
        |e, i| {
            uints(e, shell.get(i).copied().unwrap_or_default(), |r| {
                reference(Some(*r))
            })?;
            uints(e, next.get(i).copied().unwrap_or_default(), |r| {
                reference(*r)
            })?;
            uints(e, surface.get(i).copied().unwrap_or_default(), |r| {
                reference(Some(*r))
            })?;
            uints(e, fsense.get(i).copied().unwrap_or_default(), |s| sense(*s))?;
            uints(e, first_loop.get(i).copied().unwrap_or_default(), |r| {
                reference(*r)
            })?;
            let boxes: Vec<i64> = bound
                .get(i)
                .copied()
                .unwrap_or_default()
                .iter()
                .flat_map(|b| {
                    let (lo, hi) = (b.min(), b.max());
                    [
                        lo.x.get(),
                        lo.y.get(),
                        lo.z.get(),
                        hi.x.get(),
                        hi.y.get(),
                        hi.z.get(),
                    ]
                })
                .collect();
            e.i64_column(&boxes)?;
            Ok(())
        },
        &mut tables,
        meter,
    )?;

    let l = &a.loops;
    let (face, next, first_fin) = (
        chunks_of(&l.face),
        chunks_of(&l.next),
        chunks_of(&l.first_fin),
    );
    table_chunks(
        table::LOOPS,
        face.len(),
        3,
        |i| face.get(i).map_or(0, |c| c.len()),
        |e, i| {
            uints(e, face.get(i).copied().unwrap_or_default(), |r| {
                reference(Some(*r))
            })?;
            uints(e, next.get(i).copied().unwrap_or_default(), |r| {
                reference(*r)
            })?;
            uints(e, first_fin.get(i).copied().unwrap_or_default(), |r| {
                reference(*r)
            })?;
            Ok(())
        },
        &mut tables,
        meter,
    )?;

    let n = &a.fins;
    let (lp, next, prev, edge, nsense, radial, pcurve) = (
        chunks_of(&n.loop_),
        chunks_of(&n.next),
        chunks_of(&n.prev),
        chunks_of(&n.edge),
        chunks_of(&n.sense),
        chunks_of(&n.radial),
        chunks_of(&n.pcurve),
    );
    table_chunks(
        table::FINS,
        lp.len(),
        7,
        |i| lp.get(i).map_or(0, |c| c.len()),
        |e, i| {
            uints(e, lp.get(i).copied().unwrap_or_default(), |r| {
                reference(Some(*r))
            })?;
            uints(e, next.get(i).copied().unwrap_or_default(), |r| {
                reference(Some(*r))
            })?;
            uints(e, prev.get(i).copied().unwrap_or_default(), |r| {
                reference(Some(*r))
            })?;
            uints(e, edge.get(i).copied().unwrap_or_default(), |r| {
                reference(Some(*r))
            })?;
            uints(e, nsense.get(i).copied().unwrap_or_default(), |s| sense(*s))?;
            uints(e, radial.get(i).copied().unwrap_or_default(), |r| {
                reference(Some(*r))
            })?;
            uints(e, pcurve.get(i).copied().unwrap_or_default(), |r| {
                reference(*r)
            })?;
            Ok(())
        },
        &mut tables,
        meter,
    )?;

    let d = &a.edges;
    let (first_fin, start, end, curve) = (
        chunks_of(&d.first_fin),
        chunks_of(&d.start),
        chunks_of(&d.end),
        chunks_of(&d.curve),
    );
    table_chunks(
        table::EDGES,
        curve.len(),
        4,
        |i| curve.get(i).map_or(0, |c| c.len()),
        |e, i| {
            uints(e, first_fin.get(i).copied().unwrap_or_default(), |r| {
                reference(*r)
            })?;
            uints(e, start.get(i).copied().unwrap_or_default(), |r| {
                reference(*r)
            })?;
            uints(e, end.get(i).copied().unwrap_or_default(), |r| {
                reference(*r)
            })?;
            uints(e, curve.get(i).copied().unwrap_or_default(), |r| {
                reference(Some(*r))
            })?;
            Ok(())
        },
        &mut tables,
        meter,
    )?;

    let point = chunks_of(&a.vertices.point);
    table_chunks(
        table::VERTICES,
        point.len(),
        1,
        |i| point.get(i).map_or(0, |c| c.len()),
        |e, i| {
            uints(e, point.get(i).copied().unwrap_or_default(), |r| {
                reference(Some(*r))
            })?;
            Ok(())
        },
        &mut tables,
        meter,
    )?;

    let points = chunks_of(&a.points);
    table_chunks(
        table::POINTS,
        points.len(),
        1,
        |i| points.get(i).map_or(0, |c| c.len()),
        |e, i| {
            let xyz: Vec<i64> = points
                .get(i)
                .copied()
                .unwrap_or_default()
                .iter()
                .flat_map(|p| [p.x.get(), p.y.get(), p.z.get()])
                .collect();
            e.i64_column(&xyz)?;
            Ok(())
        },
        &mut tables,
        meter,
    )?;

    let surfaces = chunks_of(&a.surfaces);
    table_chunks(
        table::SURFACES,
        surfaces.len(),
        1,
        |i| surfaces.get(i).map_or(0, |c| c.len()),
        |e, i| {
            let rows = surfaces.get(i).copied().unwrap_or_default();
            e.array(u64::try_from(rows.len()).unwrap_or(u64::MAX))?;
            for s in rows {
                kinds::write_surface(e, s)?;
            }
            Ok(())
        },
        &mut tables,
        meter,
    )?;
    let curves = chunks_of(&a.curves);
    table_chunks(
        table::CURVES,
        curves.len(),
        1,
        |i| curves.get(i).map_or(0, |c| c.len()),
        |e, i| {
            let rows = curves.get(i).copied().unwrap_or_default();
            e.array(u64::try_from(rows.len()).unwrap_or(u64::MAX))?;
            for c in rows {
                kinds::write_curve(e, c)?;
            }
            Ok(())
        },
        &mut tables,
        meter,
    )?;
    let pcurves = chunks_of(&a.pcurves);
    table_chunks(
        table::PCURVES,
        pcurves.len(),
        1,
        |i| pcurves.get(i).map_or(0, |c| c.len()),
        |e, i| {
            let rows = pcurves.get(i).copied().unwrap_or_default();
            e.array(u64::try_from(rows.len()).unwrap_or(u64::MAX))?;
            for c in rows {
                kinds::write_curve(e, c)?;
            }
            Ok(())
        },
        &mut tables,
        meter,
    )?;

    if let Some(names) = body.names() {
        for (code, keys) in [
            (table::REGION_NAMES, &names.regions),
            (table::FACE_NAMES, &names.faces),
            (table::EDGE_NAMES, &names.edges),
            (table::VERTEX_NAMES, &names.vertices),
        ] {
            let runs: Vec<&[NameKey]> = keys.chunks(CHUNK_ROWS as usize).collect();
            table_chunks(
                code,
                runs.len(),
                1,
                |i| runs.get(i).map_or(0, |c| c.len()),
                |e, i| {
                    let bytes: Vec<u8> = runs
                        .get(i)
                        .copied()
                        .unwrap_or_default()
                        .iter()
                        .flat_map(|k| k.0)
                        .collect();
                    e.bytes(&bytes)?;
                    Ok(())
                },
                &mut tables,
                meter,
            )?;
        }
    }

    index(body, tables)
}

/// The index record over the chunks, and the chunks in its order.
fn index(body: &Body, tables: Vec<(u64, u64, Vec<Record>)>) -> Result<StoredBody, BodyError> {
    let mut e = Encoder::new();
    e.array(5)?;
    e.uint(match body.kind() {
        BodyKind::Solid => 0,
        BodyKind::Sheet => 1,
        BodyKind::Wire => 2,
        BodyKind::Acorn => 3,
        BodyKind::General => 4,
    })?;
    e.uint(reference(Some(body.outside())))?;
    e.array(u64::try_from(tables.len()).unwrap_or(u64::MAX))?;
    let mut chunks = Vec::new();
    let mut ids = Vec::new();
    for (code, rows, records) in tables {
        e.array(3)?;
        e.uint(code)?;
        e.uint(rows)?;
        e.array(u64::try_from(records.len()).unwrap_or(u64::MAX))?;
        for r in records {
            let id = r.content_id();
            e.item(&id.item()?)?;
            ids.push(id);
            chunks.push(r);
        }
    }
    e.uint(u64::from(body.names().is_some()))?;
    e.map(0)?;
    let payload = e.finish()?.into_bytes();
    ids.sort_by_key(|id| {
        id.item()
            .map(geom_op::cbor::Item::into_bytes)
            .unwrap_or_default()
    });
    ids.dedup();
    Ok(StoredBody {
        index: Record {
            tag: BODY_TAG,
            flags: REQUIRED,
            schema: SCHEMA,
            depends: ids,
            payload,
        },
        chunks,
    })
}

/// The containers and tags a body record's payload nests at most: a chunk, its columns, a column
/// of kinds and one kind ([`kinds::DEPTH`]); an index nests five (the index, its tables, a table,
/// its IDs, an ID).
pub const DEPTH: usize = 3 + kinds::DEPTH;

/// The columns of each table, by code.
fn width(code: u64) -> Option<u64> {
    match code {
        table::REGIONS => Some(2),
        table::SHELLS | table::LOOPS => Some(3),
        table::FACES => Some(6),
        table::FINS => Some(7),
        table::EDGES => Some(4),
        table::VERTICES..=table::VERTEX_NAMES => Some(1),
        _ => None,
    }
}

/// The next head, which must be an unsigned integer.
fn uint(
    d: &mut Decoder<'_>,
    meter: &mut WorkMeter<'_>,
    what: &'static str,
) -> Result<u64, BodyError> {
    match d.next(meter)? {
        Some(Item::Uint(n)) => Ok(n),
        _ => Err(BodyError::Shape(what)),
    }
}

/// The next head, which must be an array; its length.
fn array(
    d: &mut Decoder<'_>,
    meter: &mut WorkMeter<'_>,
    what: &'static str,
) -> Result<u64, BodyError> {
    match d.next(meter)? {
        Some(Item::Array(n)) => Ok(n),
        _ => Err(BodyError::Shape(what)),
    }
}

/// The next head, which must be a byte string.
fn bytes<'a>(
    d: &mut Decoder<'a>,
    meter: &mut WorkMeter<'_>,
    what: &'static str,
) -> Result<&'a [u8], BodyError> {
    match d.next(meter)? {
        Some(Item::Bytes(b)) => Ok(b),
        _ => Err(BodyError::Shape(what)),
    }
}

/// An `i64` column (tag 79).
fn i64s(
    d: &mut Decoder<'_>,
    meter: &mut WorkMeter<'_>,
    what: &'static str,
) -> Result<Vec<i64>, BodyError> {
    if d.next(meter)? != Some(Item::Tag(79)) {
        return Err(BodyError::Shape(what));
    }
    let mut out = Vec::new();
    int_column(bytes(d, meter, what)?, &mut out, meter)?;
    Ok(out)
}

/// A stored reference: `None` for 0, else the row `n − 1`.
fn optional<K>(n: u64) -> Result<Option<Ix<K>>, BodyError> {
    match n.checked_sub(1) {
        None => Ok(None),
        Some(row) => u32::try_from(row)
            .ok()
            .and_then(Ix::new)
            .map(Some)
            .ok_or(BodyError::Shape("a reference past an arena's limit")),
    }
}

/// A stored reference that must name a row.
fn required<K>(n: u64) -> Result<Ix<K>, BodyError> {
    optional(n)?.ok_or(BodyError::Shape("a required reference of 0"))
}

/// A stored sense.
fn read_sense(n: u64) -> Result<Sense, BodyError> {
    match n {
        0 => Ok(Sense::Forward),
        1 => Ok(Sense::Reversed),
        _ => Err(BodyError::Shape("a sense other than 0 or 1")),
    }
}

/// One column of `rows` unsigned integers, each mapped by `f`.
fn column<T>(
    d: &mut Decoder<'_>,
    meter: &mut WorkMeter<'_>,
    rows: u64,
    f: impl Fn(u64) -> Result<T, BodyError>,
) -> Result<Vec<T>, BodyError> {
    if array(d, meter, "a column")? != rows {
        return Err(BodyError::Shape(
            "a column whose length is not the chunk's rows",
        ));
    }
    let mut out = Vec::new();
    for _ in 0..rows {
        out.push(f(uint(d, meter, "a column value")?)?);
    }
    Ok(out)
}

/// The value at `k` of a column read for this chunk (always there: every column holds the chunk's
/// rows).
fn at<T: Copy>(v: &[T], k: usize, wrong: &BodyError) -> Result<T, BodyError> {
    v.get(k).copied().ok_or_else(|| wrong.clone())
}

/// A grid point from three coordinates.
fn grid(x: i64, y: i64, z: i64) -> Result<P3, BodyError> {
    P3::new(x, y, z).map_err(|_| BodyError::Shape("a point outside the grid"))
}

/// Reads a body back from its index record and its chunks, given in any order.
///
/// # Errors
///
/// [`BodyError::Chunk`] for a chunk the index lists that is missing, or is another table's or
/// another position's; [`BodyError::Shape`] for anything else that is not this body's records
/// (a chunk the index does not list among them included); [`BodyError::Kind`] for a geometry kind
/// that does not read back; [`BodyError::Topo`] for topology the builder refuses; the decoder's and
/// the meter's errors.
#[allow(
    clippy::too_many_lines,
    reason = "the index's fields in order, then the checks that tie it to its chunks"
)]
pub fn read_body(stored: &StoredBody, meter: &mut WorkMeter<'_>) -> Result<Body, BodyError> {
    let index = &stored.index;
    if index.tag != BODY_TAG || index.schema != SCHEMA || index.flags != REQUIRED {
        return Err(BodyError::Shape("not a body index of schema 1"));
    }
    let mut by_id: BTreeMap<ContentId, &Record> = BTreeMap::new();
    for r in &stored.chunks {
        by_id.insert(r.content_id(), r);
    }
    let mut d = Decoder::new(&index.payload, DEPTH);
    if array(&mut d, meter, "the index")? != 5 {
        return Err(BodyError::Shape("an index of five fields"));
    }
    let kind = match uint(&mut d, meter, "the body kind")? {
        0 => BodyKind::Solid,
        1 => BodyKind::Sheet,
        2 => BodyKind::Wire,
        3 => BodyKind::Acorn,
        4 => BodyKind::General,
        _ => return Err(BodyError::Shape("a body kind from 0 to 4")),
    };
    let outside = required(uint(&mut d, meter, "the outside region")?)?;
    let count = array(&mut d, meter, "the tables")?;
    let mut arenas = Arenas::new();
    let mut names = Names::default();
    let mut codes = Vec::new();
    let mut listed = Vec::new();
    for _ in 0..count {
        if array(&mut d, meter, "a table")? != 3 {
            return Err(BodyError::Shape("a table of three fields"));
        }
        let code = uint(&mut d, meter, "a table code")?;
        let rows = uint(&mut d, meter, "a table's rows")?;
        let chunks = array(&mut d, meter, "a table's chunks")?;
        codes.push(code);
        // geom-topo's own runs: every chunk full but the last, and none for an empty table.
        if chunks != rows.div_ceil(u64::from(CHUNK_ROWS)) {
            return Err(BodyError::Shape(
                "a table cut into other chunks than its rows need",
            ));
        }
        for position in 0..chunks {
            if array(&mut d, meter, "a content ID")? != 2 {
                return Err(BodyError::Shape("a content ID of two fields"));
            }
            let alg = uint(&mut d, meter, "an algorithm")?;
            let digest = bytes(&mut d, meter, "a digest")?;
            let id = ContentId::new(alg, digest).ok_or(BodyError::Shape("a content ID"))?;
            let wrong = BodyError::Chunk {
                table: code,
                chunk: position,
            };
            let record = by_id.get(&id).ok_or_else(|| wrong.clone())?;
            if record.tag != CHUNK_TAG || record.schema != SCHEMA || record.flags != REQUIRED {
                return Err(wrong);
            }
            let got = read_chunk(
                &record.payload,
                code,
                position,
                &mut arenas,
                &mut names,
                meter,
            )?;
            let full = position.saturating_add(1) < chunks;
            if (full && got != u64::from(CHUNK_ROWS)) || got == 0 {
                return Err(wrong);
            }
            listed.push(id);
        }
    }
    let by_name = match uint(&mut d, meter, "the named flag")? {
        0 => false,
        1 => true,
        _ => return Err(BodyError::Shape("a named flag of 0 or 1")),
    };
    if d.next(meter)? != Some(Item::Map(0)) {
        return Err(BodyError::Shape("an empty extension map"));
    }
    if d.next(meter)?.is_some() || d.end()? != index.payload.len() {
        return Err(BodyError::Shape("bytes after the index"));
    }
    let last = if by_name {
        table::VERTEX_NAMES
    } else {
        table::PCURVES
    };
    if !codes.iter().copied().eq(table::REGIONS..=last) {
        return Err(BodyError::Shape(
            "tables other than a body's, or out of order",
        ));
    }
    if listed.len() != by_id.len() || listed.len() != stored.chunks.len() {
        return Err(BodyError::Shape(
            "a chunk the index does not list, or one given twice",
        ));
    }
    let mut depends = listed;
    depends.sort_by_key(|id| {
        id.item()
            .map(geom_op::cbor::Item::into_bytes)
            .unwrap_or_default()
    });
    depends.dedup();
    if depends != index.depends {
        return Err(BodyError::Shape(
            "dependencies other than the index's chunks",
        ));
    }
    let builder = Builder { arenas };
    Ok(if by_name {
        builder.publish_named(&names, kind, outside, meter)?
    } else {
        builder.publish(kind, outside, meter)?
    })
}

/// Reads chunk `position` of table `code` into the arenas; its rows.
#[allow(clippy::too_many_lines, reason = "one short arm per table")]
fn read_chunk(
    payload: &[u8],
    code: u64,
    position: u64,
    a: &mut Arenas,
    names: &mut Names,
    meter: &mut WorkMeter<'_>,
) -> Result<u64, BodyError> {
    let wrong = BodyError::Chunk {
        table: code,
        chunk: position,
    };
    let mut decoder = Decoder::new(payload, DEPTH);
    let d = &mut decoder;
    if array(d, meter, "a chunk")? != 4
        || uint(d, meter, "a chunk's table")? != code
        || uint(d, meter, "a chunk's position")? != position
    {
        return Err(wrong);
    }
    let rows = uint(d, meter, "a chunk's rows")?;
    if Some(array(d, meter, "a chunk's columns")?) != width(code) {
        return Err(BodyError::Shape("a chunk with another number of columns"));
    }
    match code {
        table::REGIONS => {
            let kind = column(d, meter, rows, |n| match n {
                0 => Ok(RegionKind::Solid),
                1 => Ok(RegionKind::Void),
                _ => Err(BodyError::Shape("a region kind of 0 or 1")),
            })?;
            let first_shell = column(d, meter, rows, optional)?;
            for (kind, first_shell) in kind.into_iter().zip(first_shell) {
                a.regions.push(Region { kind, first_shell })?;
            }
        }
        table::SHELLS => {
            let region = column(d, meter, rows, required)?;
            let next = column(d, meter, rows, optional)?;
            let first_face = column(d, meter, rows, optional)?;
            for ((region, next), first_face) in region.into_iter().zip(next).zip(first_face) {
                a.shells.push(Shell {
                    region,
                    next,
                    first_face,
                })?;
            }
        }
        table::FACES => {
            let shell = column(d, meter, rows, required)?;
            let next = column(d, meter, rows, optional)?;
            let surface = column(d, meter, rows, required)?;
            let sense = column(d, meter, rows, read_sense)?;
            let first_loop = column(d, meter, rows, optional)?;
            let bounds = i64s(d, meter, "the faces' bounds")?;
            if u64::try_from(bounds.len()).ok() != rows.checked_mul(6) {
                return Err(BodyError::Shape("six bound coordinates per face"));
            }
            for (k, &[x0, y0, z0, x1, y1, z1]) in bounds.as_chunks::<6>().0.iter().enumerate() {
                let bound = Box3::new(grid(x0, y0, z0)?, grid(x1, y1, z1)?)
                    .map_err(|_| BodyError::Shape("a bound whose corners are not in order"))?;
                a.faces.push(Face {
                    shell: at(&shell, k, &wrong)?,
                    next: at(&next, k, &wrong)?,
                    surface: at(&surface, k, &wrong)?,
                    sense: at(&sense, k, &wrong)?,
                    first_loop: at(&first_loop, k, &wrong)?,
                    bound,
                })?;
            }
        }
        table::LOOPS => {
            let face = column(d, meter, rows, required)?;
            let next = column(d, meter, rows, optional)?;
            let first_fin = column(d, meter, rows, optional)?;
            for ((face, next), first_fin) in face.into_iter().zip(next).zip(first_fin) {
                a.loops.push(Loop {
                    face,
                    next,
                    first_fin,
                })?;
            }
        }
        table::FINS => {
            let lp = column(d, meter, rows, required)?;
            let next = column(d, meter, rows, required)?;
            let prev = column(d, meter, rows, required)?;
            let edge = column(d, meter, rows, required)?;
            let sense = column(d, meter, rows, read_sense)?;
            let radial = column(d, meter, rows, required)?;
            let pcurve = column(d, meter, rows, optional)?;
            for k in 0..lp.len() {
                a.fins.push(Fin {
                    loop_: at(&lp, k, &wrong)?,
                    next: at(&next, k, &wrong)?,
                    prev: at(&prev, k, &wrong)?,
                    edge: at(&edge, k, &wrong)?,
                    sense: at(&sense, k, &wrong)?,
                    radial: at(&radial, k, &wrong)?,
                    pcurve: at(&pcurve, k, &wrong)?,
                })?;
            }
        }
        table::EDGES => {
            let first_fin = column(d, meter, rows, optional)?;
            let start = column(d, meter, rows, optional)?;
            let end = column(d, meter, rows, optional)?;
            let curve = column(d, meter, rows, required)?;
            for k in 0..curve.len() {
                a.edges.push(Edge {
                    first_fin: at(&first_fin, k, &wrong)?,
                    start: at(&start, k, &wrong)?,
                    end: at(&end, k, &wrong)?,
                    curve: at(&curve, k, &wrong)?,
                })?;
            }
        }
        table::VERTICES => {
            for point in column(d, meter, rows, required)? {
                a.vertices.push(Vertex { point })?;
            }
        }
        table::POINTS => {
            let xyz = i64s(d, meter, "the points")?;
            if u64::try_from(xyz.len()).ok() != rows.checked_mul(3) {
                return Err(BodyError::Shape("three coordinates per point"));
            }
            for &[x, y, z] in xyz.as_chunks::<3>().0 {
                a.points.push(grid(x, y, z)?)?;
            }
        }
        table::SURFACES => {
            if array(d, meter, "the surfaces")? != rows {
                return Err(BodyError::Shape("a surface per row"));
            }
            for _ in 0..rows {
                a.surfaces.push(kinds::read_surface(d, meter)?)?;
            }
        }
        table::CURVES => {
            if array(d, meter, "the curves")? != rows {
                return Err(BodyError::Shape("a curve per row"));
            }
            for _ in 0..rows {
                a.curves.push(kinds::read_curve::<3>(d, meter)?)?;
            }
        }
        table::PCURVES => {
            if array(d, meter, "the pcurves")? != rows {
                return Err(BodyError::Shape("a pcurve per row"));
            }
            for _ in 0..rows {
                a.pcurves.push(kinds::read_curve::<2>(d, meter)?)?;
            }
        }
        _ => {
            let raw = bytes(d, meter, "a column of names")?;
            if u64::try_from(raw.len()).ok() != rows.checked_mul(16) {
                return Err(BodyError::Shape("sixteen bytes per name"));
            }
            let keys = raw.as_chunks::<16>().0.iter().map(|k| NameKey(*k));
            match code {
                table::REGION_NAMES => names.regions.extend(keys),
                table::FACE_NAMES => names.faces.extend(keys),
                table::EDGE_NAMES => names.edges.extend(keys),
                _ => names.vertices.extend(keys),
            }
        }
    }
    if d.next(meter)?.is_some() || d.end()? != payload.len() {
        return Err(BodyError::Shape("bytes after a chunk"));
    }
    Ok(rows)
}
