//! Attacks on every claim of piece G80, "Topology arenas with typed indices" (chapter 30): body,
//! region, shell, face, loop, fin, edge and vertex in column-chunked arenas with 32-bit typed
//! indices, geometry pools, ordered containers only; editing 10 faces of a 1M-face body copies
//! only touched chunks; bytes per wall body recorded (chapters 8.2 and 23.9).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    reason = "tests may use native arithmetic and casts"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::mem::size_of;

use common::{ix, many_faces, meter, wall, wall_builder, wall_outside};
use geom_math::Box3;
use geom_num::grid::P3;
use geom_topo::{
    Arena, Body, BodyKind, Builder, CHUNK_ROWS, CurveIx, EdgeIx, FaceIx, FinIx, Link, LoopIx,
    Memory, PcurveIx, PointIx, RegionIx, ShellIx, SurfaceIx, VertexIx,
};

/// Every index a column of `body` holds, as (link, row, target row).
fn every_link(body: &Body) -> Vec<(Link, u32, u32)> {
    fn rows<V: Copy>(
        out: &mut Vec<(Link, u32, u32)>,
        link: Link,
        values: impl Iterator<Item = V>,
        target: impl Fn(V) -> Option<u32>,
    ) {
        for (row, v) in values.enumerate() {
            if let Some(t) = target(v) {
                out.push((link, row as u32, t));
            }
        }
    }
    let a = body.arenas();
    let mut out = Vec::new();
    let o = &mut out;
    rows(
        o,
        Link::RegionFirstShell,
        a.regions.first_shell.iter().copied(),
        |v| v.map(geom_topo::Ix::row),
    );
    rows(o, Link::ShellRegion, a.shells.region.iter().copied(), |v| {
        Some(v.row())
    });
    rows(o, Link::ShellNext, a.shells.next.iter().copied(), |v| {
        v.map(geom_topo::Ix::row)
    });
    rows(
        o,
        Link::ShellFirstFace,
        a.shells.first_face.iter().copied(),
        |v| v.map(geom_topo::Ix::row),
    );
    rows(o, Link::FaceShell, a.faces.shell.iter().copied(), |v| {
        Some(v.row())
    });
    rows(o, Link::FaceNext, a.faces.next.iter().copied(), |v| {
        v.map(geom_topo::Ix::row)
    });
    rows(o, Link::FaceSurface, a.faces.surface.iter().copied(), |v| {
        Some(v.row())
    });
    rows(
        o,
        Link::FaceFirstLoop,
        a.faces.first_loop.iter().copied(),
        |v| v.map(geom_topo::Ix::row),
    );
    rows(o, Link::LoopFace, a.loops.face.iter().copied(), |v| {
        Some(v.row())
    });
    rows(o, Link::LoopNext, a.loops.next.iter().copied(), |v| {
        v.map(geom_topo::Ix::row)
    });
    rows(
        o,
        Link::LoopFirstFin,
        a.loops.first_fin.iter().copied(),
        |v| v.map(geom_topo::Ix::row),
    );
    rows(o, Link::FinLoop, a.fins.loop_.iter().copied(), |v| {
        Some(v.row())
    });
    rows(o, Link::FinNext, a.fins.next.iter().copied(), |v| {
        Some(v.row())
    });
    rows(o, Link::FinPrev, a.fins.prev.iter().copied(), |v| {
        Some(v.row())
    });
    rows(o, Link::FinEdge, a.fins.edge.iter().copied(), |v| {
        Some(v.row())
    });
    rows(o, Link::FinRadial, a.fins.radial.iter().copied(), |v| {
        Some(v.row())
    });
    rows(o, Link::FinPcurve, a.fins.pcurve.iter().copied(), |v| {
        v.map(geom_topo::Ix::row)
    });
    rows(
        o,
        Link::EdgeFirstFin,
        a.edges.first_fin.iter().copied(),
        |v| v.map(geom_topo::Ix::row),
    );
    rows(o, Link::EdgeStart, a.edges.start.iter().copied(), |v| {
        v.map(geom_topo::Ix::row)
    });
    rows(o, Link::EdgeEnd, a.edges.end.iter().copied(), |v| {
        v.map(geom_topo::Ix::row)
    });
    rows(o, Link::EdgeCurve, a.edges.curve.iter().copied(), |v| {
        Some(v.row())
    });
    rows(
        o,
        Link::VertexPoint,
        a.vertices.point.iter().copied(),
        |v| Some(v.row()),
    );
    out
}

#[test]
fn every_entity_and_pool_index_is_32_bits_and_free_when_absent() {
    assert_eq!(
        [
            size_of::<RegionIx>(),
            size_of::<ShellIx>(),
            size_of::<FaceIx>(),
            size_of::<LoopIx>(),
            size_of::<FinIx>(),
            size_of::<EdgeIx>(),
            size_of::<VertexIx>(),
            size_of::<SurfaceIx>(),
            size_of::<CurveIx>(),
            size_of::<PcurveIx>(),
            size_of::<PointIx>(),
        ],
        [4; 11]
    );
    assert_eq!(
        [
            size_of::<Option<RegionIx>>(),
            size_of::<Option<ShellIx>>(),
            size_of::<Option<FaceIx>>(),
            size_of::<Option<LoopIx>>(),
            size_of::<Option<FinIx>>(),
            size_of::<Option<EdgeIx>>(),
            size_of::<Option<VertexIx>>(),
            size_of::<Option<SurfaceIx>>(),
            size_of::<Option<CurveIx>>(),
            size_of::<Option<PcurveIx>>(),
            size_of::<Option<PointIx>>(),
        ],
        [4; 11]
    );
}

#[test]
fn every_link_of_a_published_body_resolves() {
    for body in [wall(), many_faces(2 * CHUNK_ROWS + 3)] {
        let links = every_link(&body);
        assert_ne!(links.len(), 0);
        let a = body.arenas();
        for (link, row, target) in links {
            assert!(target < a.rows(link.to()), "{link} of row {row}");
        }
    }
    // The wall uses every link column but the pcurves (its faces are planes) and the next shell
    // and next loop (it has one shell, and one loop on each face).
    let used: std::collections::BTreeSet<Link> =
        every_link(&wall()).into_iter().map(|(l, _, _)| l).collect();
    let missing: Vec<Link> = Link::ALL
        .into_iter()
        .filter(|l| !used.contains(l))
        .collect();
    assert_eq!(missing, [Link::ShellNext, Link::LoopNext, Link::FinPcurve]);
}

#[test]
fn editing_ten_faces_of_a_million_face_body_copies_only_touched_chunks() {
    let v1 = many_faces(1_000_000);
    let chunks_per_column = 1_000_000_u32.div_ceil(CHUNK_ROWS);
    assert_eq!(chunks_per_column, 245);
    let before = v1.memory();
    assert_eq!(before.unique_chunks, before.chunks, "nothing shared yet");

    // Ten faces in ten different chunks: one field each.
    let touched: Vec<u32> = (0..10).map(|k| k * 100_003).collect();
    let mut b = Builder::edit(&v1);
    for &f in &touched {
        b.arenas.faces.surface.set(ix(f), ix(1)).unwrap();
    }
    let v2 = b.publish(BodyKind::Sheet, ix(1), &mut meter()).unwrap();
    assert_eq!(v2.memory().chunks, before.chunks);
    assert_eq!(v2.memory().unique_chunks, 10);
    assert_eq!(v1.memory().unique_chunks, 10);
    // Its own bytes are its lists of chunk pointers, which every version has, and the ten copies.
    let lists = Builder::edit(&v1).arenas.memory().unique_bytes;
    let header = (2 * size_of::<usize>() + size_of::<Vec<u8>>()) as u64;
    let chunk_bytes = u64::from(CHUNK_ROWS) * size_of::<SurfaceIx>() as u64 + header;
    assert_eq!(v2.memory().unique_bytes, lists + 10 * chunk_bytes);

    // The values: the ten moved, nothing else did, and the old version is as it was.
    for f in [0, 1, 99_999, 100_003, 500_000, 999_999] {
        let moved = touched.contains(&f);
        let s = v2.arenas().faces.surface.get(ix(f)).unwrap().row();
        assert_eq!(s, u32::from(moved), "face {f}");
        assert_eq!(v1.arenas().faces.surface.get(ix(f)).unwrap().row(), 0);
    }
    assert_eq!(v2.arenas().faces.shell, v1.arenas().faces.shell);
    assert_eq!(v2.arenas().faces.bound, v1.arenas().faces.bound);

    // Two fields of the same ten faces: twenty chunks, ten per column written.
    let mut b = Builder::edit(&v1);
    for &f in &touched {
        b.arenas.faces.surface.set(ix(f), ix(1)).unwrap();
        let bound = Box3::new(P3::new(0, 0, 0).unwrap(), P3::new(9, 9, 9).unwrap()).unwrap();
        b.arenas.faces.bound.set(ix(f), bound).unwrap();
    }
    let v3 = b.publish(BodyKind::Sheet, ix(1), &mut meter()).unwrap();
    assert_eq!(v3.memory().unique_chunks, 20);
    // Faces in one chunk: one copy.
    let mut b = Builder::edit(&v1);
    for f in 4096..4106 {
        b.arenas
            .faces
            .sense
            .set(ix(f), geom_topo::Sense::Reversed)
            .unwrap();
    }
    let v4 = b.publish(BodyKind::Sheet, ix(1), &mut meter()).unwrap();
    assert_eq!(v4.memory().unique_chunks, 1);
    drop((v2, v3, v4));
    assert_eq!(
        v1.memory(),
        before,
        "every copy gone, the original holds its chunks alone"
    );
}

/// Heap bytes of the wall, from the sizes of what each column holds: the record of chapter 23.9's
/// "bytes per wall body", derived independently of `Memory`'s own sum.
fn wall_bytes_by_hand() -> (u64, u64) {
    let header = (2 * size_of::<usize>() + size_of::<Vec<u8>>()) as u64;
    let list = 4 * size_of::<usize>() as u64;
    let col = |rows: u64, size: usize| rows * size as u64 + header + list;
    let ix = size_of::<u32>();
    let columns = [
        // Regions: kind (one byte), first shell.
        col(2, 1),
        col(2, ix),
        // Shells: region, next, first face.
        col(1, ix),
        col(1, ix),
        col(1, ix),
        // Faces: shell, next, surface, sense, first loop, bound.
        col(6, ix),
        col(6, ix),
        col(6, ix),
        col(6, 1),
        col(6, ix),
        col(6, size_of::<Box3>()),
        // Loops: face, next, first fin.
        col(6, ix),
        col(6, ix),
        col(6, ix),
        // Fins: loop, next, prev, edge, sense, radial, pcurve.
        col(24, ix),
        col(24, ix),
        col(24, ix),
        col(24, ix),
        col(24, 1),
        col(24, ix),
        col(24, ix),
        // Edges: first fin, start, end, curve.
        col(12, ix),
        col(12, ix),
        col(12, ix),
        col(12, ix),
        // Vertices: point.
        col(8, ix),
        // Pools: surfaces, curves, points (no pcurves: an empty column holds nothing).
        col(6, size_of::<geom_surface::SurfaceKind>()),
        col(12, size_of::<geom_curve::CurveKind<3>>()),
        col(8, size_of::<P3>()),
    ];
    let mut total = 0;
    for c in columns {
        total += c;
    }
    (total, columns.len() as u64)
}

#[test]
fn bytes_per_wall_body_are_recorded() {
    let body = wall();
    let m = body.memory();
    let (bytes, chunks) = wall_bytes_by_hand();
    println!(
        "wall body: {} heap bytes in {} chunks, beside the body's own {} bytes; surfaces {} bytes \
         a row, 3D curves {}, pcurves {}, face bound {}",
        m.bytes,
        m.chunks,
        size_of::<Body>(),
        size_of::<geom_surface::SurfaceKind>(),
        size_of::<geom_curve::CurveKind<3>>(),
        size_of::<geom_curve::CurveKind<2>>(),
        size_of::<Box3>(),
    );
    assert_eq!(m.bytes, bytes);
    assert_eq!(m.chunks, chunks);
    assert_eq!(m.unique_chunks, chunks);
    assert_eq!(m.unique_bytes, bytes);
    // The record in body.rs's header, per pointer width.
    #[cfg(target_pointer_width = "64")]
    assert_eq!((m.bytes, m.chunks, size_of::<Body>()), (16_740, 29, 976));
    #[cfg(target_pointer_width = "32")]
    assert_eq!((m.bytes, m.chunks, size_of::<Body>()), (15_024, 29, 492));
    // Built row by row instead, the same body holds more: each column grew by doubling.
    let pushed = wall_builder(false)
        .publish(BodyKind::Solid, wall_outside(), &mut meter())
        .unwrap();
    assert_eq!(pushed, body);
    assert!(pushed.memory().bytes > m.bytes);
}

#[test]
fn chunk_boundaries_do_not_depend_on_how_a_body_was_built() {
    // The same million faces by one extend, and by an edit of every chunk of another body.
    let n = 3 * CHUNK_ROWS + 77;
    let direct = many_faces(n);
    let mut b = Builder::edit(&many_faces(n));
    for f in (0..n).step_by(CHUNK_ROWS as usize / 2) {
        let s = *b.arenas.faces.surface.get(ix(f)).unwrap();
        b.arenas.faces.surface.set(ix(f), s).unwrap();
    }
    let edited = b.publish(BodyKind::Sheet, ix(1), &mut meter()).unwrap();
    assert_eq!(edited, direct);
    let lengths = |body: &Body| -> Vec<usize> {
        body.arenas()
            .faces
            .bound
            .chunks()
            .map(<[Box3]>::len)
            .collect()
    };
    assert_eq!(lengths(&edited), lengths(&direct));
    assert_eq!(lengths(&direct), [4096, 4096, 4096, 77]);
}

#[test]
fn a_new_version_never_changes_the_old_one_whichever_column_is_written() {
    let original = wall();
    let snapshot = wall();
    for link in Link::ALL {
        let mut b = Builder::edit(&original);
        let a = &mut b.arenas;
        match link {
            Link::RegionFirstShell => a.regions.first_shell.set(ix(0), None),
            Link::ShellRegion => a.shells.region.set(ix(0), ix(1)),
            Link::ShellNext => a.shells.next.set(ix(0), Some(ix(0))),
            Link::ShellFirstFace => a.shells.first_face.set(ix(0), Some(ix(5))),
            Link::FaceShell => a.faces.shell.set(ix(0), ix(0)),
            Link::FaceNext => a.faces.next.set(ix(0), None),
            Link::FaceSurface => a.faces.surface.set(ix(0), ix(5)),
            Link::FaceFirstLoop => a.faces.first_loop.set(ix(0), None),
            Link::LoopFace => a.loops.face.set(ix(0), ix(5)),
            Link::LoopNext => a.loops.next.set(ix(0), Some(ix(1))),
            Link::LoopFirstFin => a.loops.first_fin.set(ix(0), None),
            Link::FinLoop => a.fins.loop_.set(ix(0), ix(5)),
            Link::FinNext => a.fins.next.set(ix(0), ix(0)),
            Link::FinPrev => a.fins.prev.set(ix(0), ix(0)),
            Link::FinEdge => a.fins.edge.set(ix(0), ix(11)),
            Link::FinRadial => a.fins.radial.set(ix(0), ix(0)),
            Link::FinPcurve => a.fins.pcurve.set(ix(0), None),
            Link::EdgeFirstFin => a.edges.first_fin.set(ix(0), None),
            Link::EdgeStart => a.edges.start.set(ix(0), None),
            Link::EdgeEnd => a.edges.end.set(ix(0), None),
            Link::EdgeCurve => a.edges.curve.set(ix(0), ix(11)),
            Link::VertexPoint => a.vertices.point.set(ix(0), ix(7)),
        }
        .unwrap();
        let next = b
            .publish(BodyKind::Solid, wall_outside(), &mut meter())
            .unwrap();
        assert_eq!(original, snapshot, "{link}");
        drop(next);
    }
}

#[test]
fn no_container_iterates_by_hash_or_address() {
    // Ordered containers only (8.2): the crate's source names no hashed container and compares
    // no addresses. The one pointer comparison a chunk's sharing could tempt is never written.
    let sources = [
        ("arenas.rs", include_str!("../src/arenas.rs")),
        ("body.rs", include_str!("../src/body.rs")),
        ("builder.rs", include_str!("../src/builder.rs")),
        ("column.rs", include_str!("../src/column.rs")),
        ("entity.rs", include_str!("../src/entity.rs")),
        ("error.rs", include_str!("../src/error.rs")),
        ("ix.rs", include_str!("../src/ix.rs")),
        ("lib.rs", include_str!("../src/lib.rs")),
    ];
    for (name, text) in sources {
        let code: String = text
            .replace("\r\n", "\n")
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for banned in [
            "HashMap",
            "HashSet",
            "hashbrown",
            "IndexMap",
            "RandomState",
            "ptr_eq",
            "as_ptr",
            "addr(",
            "thread_local",
        ] {
            assert!(!code.contains(banned), "{name} names {banned}");
        }
        for line in code.lines().map(str::trim_start) {
            let item = line
                .strip_prefix("pub(crate) ")
                .or_else(|| line.strip_prefix("pub "))
                .unwrap_or(line);
            assert!(!item.starts_with("static "), "{name} declares a static");
        }
    }
}

#[test]
fn memory_is_the_sum_of_every_column() {
    let body = wall();
    let a = body.arenas();
    let sum = [
        a.regions.memory(),
        a.shells.memory(),
        a.faces.memory(),
        a.loops.memory(),
        a.fins.memory(),
        a.edges.memory(),
        a.vertices.memory(),
        a.surfaces.memory(),
        a.curves.memory(),
        a.pcurves.memory(),
        a.points.memory(),
    ]
    .into_iter()
    .fold(Memory::default(), Memory::plus);
    assert_eq!(body.memory(), sum);
    assert_eq!(a.memory(), sum);
    let parts = [
        a.regions.kind.memory().plus(a.regions.first_shell.memory()),
        a.vertices.point.memory(),
    ];
    assert_eq!(parts[0], a.regions.memory());
    assert_eq!(parts[1], a.vertices.memory());
    assert_eq!(a.rows(Arena::Pcurve), 0);
}
