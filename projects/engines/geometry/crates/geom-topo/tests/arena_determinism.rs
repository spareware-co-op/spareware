//! The replay hash of piece G80: arenas filled, published, edited and refused over a fixed
//! corpus, their bytes hashed with SHA-256, must give the same digest on every target (chapters 21
//! and 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed `SplitMix64` stream: 48 bodies of random sizes (some past three
//! chunks of faces and fins), every link random and in range, filled row by row or each arena at
//! once; each published body's columns in row order with each column's chunk count and the work
//! publishing charged; an edit of random rows and the new version beside the unchanged old one;
//! and the error of each body with one link pointed one or more rows past its arena, and of each
//! with one column made one row longer. Every index is written as its row (absent as
//! `u32::MAX`), every kind as a byte, every box and point as its six or three steps, and every
//! error as its text, so a target that cuts a chunk, orders a row, charges a unit or finds a fault
//! differently changes the digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::num::NonZeroU64;

use common::{ix, line, meter, plane};
use geom_curve::{CurveKind, Line};
use geom_math::Box3;
use geom_num::grid::P3;
use geom_test::golden::{self, hex, sha256};
use geom_test::{Seed, SplitMix64};
use geom_topo::{
    Arena, Arenas, Body, BodyKind, Builder, Column, Edge, Face, Fin, Ix, Link, Loop, PointKind,
    Region, RegionIx, RegionKind, Sense, Shell, Vertex,
};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-topo/arenas";

fn below(rng: &mut SplitMix64, n: u32) -> u32 {
    rng.below(NonZeroU64::new(u64::from(n)).unwrap()) as u32
}

/// A link into an arena of `rows` rows (at least one).
fn to<K>(rng: &mut SplitMix64, rows: u32) -> Ix<K> {
    ix(below(rng, rows))
}

/// A link that is absent one time in four.
fn maybe<K>(rng: &mut SplitMix64, rows: u32) -> Option<Ix<K>> {
    (below(rng, 4) != 0).then(|| to(rng, rows))
}

fn sense(rng: &mut SplitMix64) -> Sense {
    if rng.coin() {
        Sense::Forward
    } else {
        Sense::Reversed
    }
}

fn bound(rng: &mut SplitMix64) -> Box3 {
    let lo = [0; 3].map(|_| i64::from(below(rng, 1 << 20)) - (1 << 19));
    let hi = lo.map(|x| x + i64::from(below(rng, 1000)));
    Box3::new(
        P3::new(lo[0], lo[1], lo[2]).unwrap(),
        P3::new(hi[0], hi[1], hi[2]).unwrap(),
    )
    .unwrap()
}

/// Random arenas with every link in range, and the outside region (a void).
fn arenas(rng: &mut SplitMix64, big: bool) -> (Builder, RegionIx) {
    let n = |rng: &mut SplitMix64, small: u32, large: u32| {
        1 + below(rng, if big { large } else { small })
    };
    let rows = [
        n(rng, 3, 4),
        n(rng, 4, 8),
        n(rng, 30, 2 * 4096 + 500),
        n(rng, 30, 2 * 4096 + 500),
        n(rng, 120, 3 * 4096 + 900),
        n(rng, 60, 4096 + 300),
        n(rng, 40, 4096),
        n(rng, 4, 6),
        n(rng, 4, 6),
        n(rng, 3, 4),
        n(rng, 40, 4096),
    ];
    let [
        regions,
        shells,
        faces,
        loops,
        fins,
        edges,
        vertices,
        surfaces,
        curves,
        pcurves,
        points,
    ] = rows;
    let whole = rng.coin();
    let mut b = Builder::new();
    let a = &mut b.arenas;
    let outside = below(rng, regions);
    let region_rows: Vec<Region> = (0..regions)
        .map(|r| Region {
            kind: if r == outside || rng.coin() {
                RegionKind::Void
            } else {
                RegionKind::Solid
            },
            first_shell: maybe(rng, shells),
        })
        .collect();
    let shell_rows: Vec<Shell> = (0..shells)
        .map(|_| Shell {
            region: to(rng, regions),
            next: maybe(rng, shells),
            first_face: maybe(rng, faces),
        })
        .collect();
    let face_rows: Vec<Face> = (0..faces)
        .map(|_| Face {
            shell: to(rng, shells),
            next: maybe(rng, faces),
            surface: to(rng, surfaces),
            sense: sense(rng),
            first_loop: maybe(rng, loops),
            bound: bound(rng),
        })
        .collect();
    let loop_rows: Vec<Loop> = (0..loops)
        .map(|_| Loop {
            face: to(rng, faces),
            next: maybe(rng, loops),
            first_fin: maybe(rng, fins),
        })
        .collect();
    let fin_rows: Vec<Fin> = (0..fins)
        .map(|_| Fin {
            loop_: to(rng, loops),
            next: to(rng, fins),
            prev: to(rng, fins),
            edge: to(rng, edges),
            sense: sense(rng),
            radial: to(rng, fins),
            pcurve: maybe(rng, pcurves),
        })
        .collect();
    let edge_rows: Vec<Edge> = (0..edges)
        .map(|_| Edge {
            first_fin: maybe(rng, fins),
            start: maybe(rng, vertices),
            end: maybe(rng, vertices),
            curve: to(rng, curves),
        })
        .collect();
    let vertex_rows: Vec<Vertex> = (0..vertices)
        .map(|_| Vertex {
            point: to(rng, points),
        })
        .collect();
    let surface_rows: Vec<_> = (0..surfaces)
        .map(|z| plane([0.0, 0.0, f64::from(z)], 0, 1))
        .collect();
    let curve_rows: Vec<_> = (0..curves)
        .map(|c| line([0, 0, i64::from(c)], [1, 2, i64::from(c)]))
        .collect();
    let pcurve_rows: Vec<CurveKind<2>> = (0..pcurves)
        .map(|c| CurveKind::Line(Line::new([0.0, f64::from(c)], [1.0, 1.0], (0.0, 1.0)).unwrap()))
        .collect();
    let point_rows: Vec<P3> = (0..points)
        .map(|_| {
            let c = [0; 3].map(|_| i64::from(below(rng, 1 << 30)) - (1 << 29));
            P3::new(c[0], c[1], c[2]).unwrap()
        })
        .collect();
    if whole {
        a.regions.extend(&region_rows).unwrap();
        a.shells.extend(&shell_rows).unwrap();
        a.faces.extend(&face_rows).unwrap();
        a.loops.extend(&loop_rows).unwrap();
        a.fins.extend(&fin_rows).unwrap();
        a.edges.extend(&edge_rows).unwrap();
        a.vertices.extend(&vertex_rows).unwrap();
        a.surfaces.extend(surface_rows).unwrap();
        a.curves.extend(curve_rows).unwrap();
        a.pcurves.extend(pcurve_rows).unwrap();
        a.points
            .extend(point_rows.into_iter().map(PointKind::Grid))
            .unwrap();
    } else {
        for r in region_rows {
            a.regions.push(r).unwrap();
        }
        for r in shell_rows {
            a.shells.push(r).unwrap();
        }
        for r in face_rows {
            a.faces.push(r).unwrap();
        }
        for r in loop_rows {
            a.loops.push(r).unwrap();
        }
        for r in fin_rows {
            a.fins.push(r).unwrap();
        }
        for r in edge_rows {
            a.edges.push(r).unwrap();
        }
        for r in vertex_rows {
            a.vertices.push(r).unwrap();
        }
        for r in surface_rows {
            a.surfaces.push(r).unwrap();
        }
        for r in curve_rows {
            a.curves.push(r).unwrap();
        }
        for r in pcurve_rows {
            a.pcurves.push(r).unwrap();
        }
        for r in point_rows {
            a.points.push(PointKind::Grid(r)).unwrap();
        }
    }
    (b, ix(outside))
}

/// The corpus's bytes and how many bodies it published.
struct Record {
    bytes: Vec<u8>,
    bodies: u32,
}

impl Record {
    fn u32(&mut self, v: u32) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    fn i64(&mut self, v: i64) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    fn text(&mut self, t: &str) {
        self.bytes.extend_from_slice(t.as_bytes());
        self.bytes.push(0);
    }
    fn column<K, V>(&mut self, c: &Column<K, V>, mut each: impl FnMut(&mut Self, &V)) {
        self.u32(c.len());
        self.u32(c.chunk_count());
        for v in c.iter() {
            each(self, v);
        }
    }
    fn ix<K>(&mut self, i: Ix<K>) {
        self.u32(i.row());
    }
    fn opt<K>(&mut self, i: Option<Ix<K>>) {
        self.u32(i.map_or(u32::MAX, Ix::row));
    }
    fn sense(&mut self, s: Sense) {
        self.bytes.push(u8::from(s == Sense::Reversed));
    }
    fn p3(&mut self, p: P3) {
        self.i64(p.x.get());
        self.i64(p.y.get());
        self.i64(p.z.get());
    }

    fn arenas(&mut self, a: &Arenas) {
        self.column(&a.regions.kind, |r, k| {
            r.bytes.push(u8::from(*k == RegionKind::Void));
        });
        self.column(&a.regions.first_shell, |r, v| r.opt(*v));
        self.column(&a.shells.region, |r, v| r.ix(*v));
        self.column(&a.shells.next, |r, v| r.opt(*v));
        self.column(&a.shells.first_face, |r, v| r.opt(*v));
        self.column(&a.faces.shell, |r, v| r.ix(*v));
        self.column(&a.faces.next, |r, v| r.opt(*v));
        self.column(&a.faces.surface, |r, v| r.ix(*v));
        self.column(&a.faces.sense, |r, v| r.sense(*v));
        self.column(&a.faces.first_loop, |r, v| r.opt(*v));
        self.column(&a.faces.bound, |r, b| {
            r.p3(b.min());
            r.p3(b.max());
        });
        self.column(&a.loops.face, |r, v| r.ix(*v));
        self.column(&a.loops.next, |r, v| r.opt(*v));
        self.column(&a.loops.first_fin, |r, v| r.opt(*v));
        self.column(&a.fins.loop_, |r, v| r.ix(*v));
        self.column(&a.fins.next, |r, v| r.ix(*v));
        self.column(&a.fins.prev, |r, v| r.ix(*v));
        self.column(&a.fins.edge, |r, v| r.ix(*v));
        self.column(&a.fins.sense, |r, v| r.sense(*v));
        self.column(&a.fins.radial, |r, v| r.ix(*v));
        self.column(&a.fins.pcurve, |r, v| r.opt(*v));
        self.column(&a.edges.first_fin, |r, v| r.opt(*v));
        self.column(&a.edges.start, |r, v| r.opt(*v));
        self.column(&a.edges.end, |r, v| r.opt(*v));
        self.column(&a.edges.curve, |r, v| r.ix(*v));
        self.column(&a.vertices.point, |r, v| r.ix(*v));
        // The pools' geometry is other pieces' (their goldens hash it); their sizes are this one's.
        for arena in [Arena::Surface, Arena::Curve, Arena::Pcurve] {
            self.u32(a.rows(arena));
        }
        self.u32(a.surfaces.chunk_count());
        self.column(&a.points, |r, p| r.p3(p.grid().unwrap()));
    }

    fn body(&mut self, body: &Body) {
        self.bytes.push(body.kind() as u8);
        self.ix(body.outside());
        self.arenas(body.arenas());
    }

    fn publish(&mut self, b: Builder, kind: BodyKind, outside: RegionIx) -> Option<Body> {
        let mut m = meter();
        let result = b.publish(kind, outside, &mut m);
        self.bytes.extend_from_slice(&m.used().work.to_le_bytes());
        match result {
            Ok(body) => {
                self.bodies += 1;
                self.body(&body);
                Some(body)
            }
            Err(e) => {
                self.text(&e.to_string());
                None
            }
        }
    }
}

/// Points `link` at row `row` of its column to `target`, if the column has that row.
fn point(a: &mut Arenas, link: Link, row: u32, target: u32) {
    let r = row;
    let t = target;
    let _ = match link {
        Link::RegionFirstShell => a.regions.first_shell.set(ix(r), Some(ix(t))),
        Link::ShellRegion => a.shells.region.set(ix(r), ix(t)),
        Link::ShellNext => a.shells.next.set(ix(r), Some(ix(t))),
        Link::ShellFirstFace => a.shells.first_face.set(ix(r), Some(ix(t))),
        Link::FaceShell => a.faces.shell.set(ix(r), ix(t)),
        Link::FaceNext => a.faces.next.set(ix(r), Some(ix(t))),
        Link::FaceSurface => a.faces.surface.set(ix(r), ix(t)),
        Link::FaceFirstLoop => a.faces.first_loop.set(ix(r), Some(ix(t))),
        Link::LoopFace => a.loops.face.set(ix(r), ix(t)),
        Link::LoopNext => a.loops.next.set(ix(r), Some(ix(t))),
        Link::LoopFirstFin => a.loops.first_fin.set(ix(r), Some(ix(t))),
        Link::FinLoop => a.fins.loop_.set(ix(r), ix(t)),
        Link::FinNext => a.fins.next.set(ix(r), ix(t)),
        Link::FinPrev => a.fins.prev.set(ix(r), ix(t)),
        Link::FinEdge => a.fins.edge.set(ix(r), ix(t)),
        Link::FinRadial => a.fins.radial.set(ix(r), ix(t)),
        Link::FinPcurve => a.fins.pcurve.set(ix(r), Some(ix(t))),
        Link::EdgeFirstFin => a.edges.first_fin.set(ix(r), Some(ix(t))),
        Link::EdgeStart => a.edges.start.set(ix(r), Some(ix(t))),
        Link::EdgeEnd => a.edges.end.set(ix(r), Some(ix(t))),
        Link::EdgeCurve => a.edges.curve.set(ix(r), ix(t)),
        Link::VertexPoint => a.vertices.point.set(ix(r), ix(t)),
    };
}

fn corpus() -> Record {
    let mut rng = SplitMix64::new(geom_test::seed::fixed(
        "geom-topo arenas golden",
        Seed::DEFAULT,
    ));
    let mut rec = Record {
        bytes: Vec::new(),
        bodies: 0,
    };
    let kinds = [
        BodyKind::Solid,
        BodyKind::Sheet,
        BodyKind::Wire,
        BodyKind::Acorn,
        BodyKind::General,
    ];
    for round in 0..48u32 {
        let big = round % 8 == 7;
        let kind = kinds[(round % 5) as usize];
        let (b, outside) = arenas(&mut rng, big);
        let spare = b.clone();
        let Some(v1) = rec.publish(b, kind, outside) else {
            panic!("a body with every link in range was refused");
        };

        // An edit of random rows: the new version, then the old one, unchanged.
        let mut e = Builder::edit(&v1);
        let a = &mut e.arenas;
        for _ in 0..=below(&mut rng, 12) {
            let link = Link::ALL[below(&mut rng, 22) as usize];
            let from = a.rows(link.from());
            let rows = a.rows(link.to());
            let (row, target) = (below(&mut rng, from), below(&mut rng, rows));
            point(a, link, row, target);
        }
        let face = below(&mut rng, a.faces.len());
        a.faces.bound.set(ix(face), bound(&mut rng)).unwrap();
        a.faces.sense.set(ix(face), sense(&mut rng)).unwrap();
        rec.publish(e, kind, outside).unwrap();
        rec.body(&v1);

        // One link pointed past its arena.
        let mut bad = spare.clone();
        let link = Link::ALL[below(&mut rng, 22) as usize];
        let from = bad.arenas.rows(link.from());
        let rows = bad.arenas.rows(link.to());
        point(
            &mut bad.arenas,
            link,
            below(&mut rng, from),
            rows + below(&mut rng, 3),
        );
        assert!(rec.publish(bad, kind, outside).is_none());

        // One column one row longer.
        let mut ragged = spare;
        match below(&mut rng, 6) {
            0 => {
                ragged.arenas.regions.first_shell.push(None).unwrap();
            }
            1 => {
                ragged.arenas.shells.next.push(None).unwrap();
            }
            2 => {
                ragged.arenas.faces.bound.push(bound(&mut rng)).unwrap();
            }
            3 => {
                ragged.arenas.loops.first_fin.push(None).unwrap();
            }
            4 => {
                ragged.arenas.fins.pcurve.push(None).unwrap();
            }
            _ => {
                ragged.arenas.edges.end.push(None).unwrap();
            }
        }
        assert!(rec.publish(ragged, kind, outside).is_none());
    }
    rec
}

#[test]
fn arenas_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-topo arenas corpus: {} bodies, {} bytes, golden hash {}",
        rec.bodies,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert_eq!(rec.bodies, 96, "every valid body and every edit published");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target cuts a chunk, orders a row, charges a \
             unit or finds a fault differently (compare this log's digest with another target's), \
             or a definition changed in place, which chapter 26.2 forbids"
        )
    });
}
