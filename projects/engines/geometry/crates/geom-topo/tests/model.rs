//! Model-based tests and no-panic harnesses for the arenas (chapter 28.2 and the standard bar of
//! chapter 30.1), driven by seeded streams: the default seed on pull requests, a fresh one nightly
//! through `SPAREKIT_SEED`, and every seed printed so a failure replays.
//!
//! - A column against a plain vector: random pushes, extends, sets (in range and past the end),
//!   clones kept as old versions and dropped, over many chunks. After every step the column equals
//!   its model, its chunks keep the fixed rule, and every kept version still equals the model it
//!   was cloned with.
//! - Publishing arbitrary arenas: every link column filled with arbitrary rows (in range, past the
//!   end, absent), some columns longer than others, an arbitrary outside region. Publishing never
//!   panics, refuses only with the errors it documents, accepts exactly when an independent check
//!   of the same rules passes, and on success every link of the body resolves.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    reason = "test models and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::num::NonZeroU64;

use common::{ix, meter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};
use geom_topo::{
    Arena, Arenas, BodyKind, Builder, CHUNK_ROWS, Column, Face, Ix, Kind, Link, RegionKind, Sense,
    TopoError,
};

fn below(rng: &mut SplitMix64, n: u32) -> u32 {
    rng.below(NonZeroU64::new(u64::from(n)).unwrap()) as u32
}

/// The chunk rule: every chunk but the last full, none empty, as many as the rows need.
fn chunk_rule<K, V>(c: &Column<K, V>) {
    let lengths: Vec<usize> = c.chunks().map(<[V]>::len).collect();
    assert_eq!(lengths.len() as u32, c.len().div_ceil(CHUNK_ROWS));
    assert_eq!(c.chunk_count() as usize, lengths.len());
    if let Some((_, full)) = lengths.split_last() {
        assert!(full.iter().all(|&n| n == CHUNK_ROWS as usize));
    }
    assert!(lengths.iter().all(|&n| n > 0));
    let mut total = 0;
    for n in &lengths {
        total += n;
    }
    assert_eq!(total, c.len() as usize);
}

fn equal(c: &Column<Face, u32>, model: &[u32]) {
    assert_eq!(c.len() as usize, model.len());
    assert!(c.iter().eq(model.iter()));
}

#[test]
fn a_column_behaves_as_a_vector_through_every_edit_and_version() {
    let seed = resolve("geom-topo column model", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut c: Column<Face, u32> = Column::new();
    let mut model: Vec<u32> = Vec::new();
    let mut versions: Vec<(Column<Face, u32>, Vec<u32>)> = Vec::new();
    for step in 0..3000u32 {
        match below(&mut rng, 10) {
            0..=2 => {
                let v = rng.next_u64() as u32;
                let i = c.push(v).unwrap();
                assert_eq!(i.row() as usize, model.len());
                model.push(v);
            }
            3 => {
                let n = below(&mut rng, 2 * CHUNK_ROWS);
                let start = rng.next_u64() as u32;
                c.extend((0..n).map(|k| start.wrapping_add(k))).unwrap();
                model.extend((0..n).map(|k| start.wrapping_add(k)));
            }
            4..=6 => {
                let len = model.len() as u32;
                let row = below(&mut rng, len + len / 8 + 2);
                let v = rng.next_u64() as u32;
                let result = c.set(ix(row), v);
                if row < len {
                    result.unwrap();
                    model[row as usize] = v;
                } else {
                    assert_eq!(
                        result,
                        Err(TopoError::NoSuchRow {
                            arena: Arena::Face,
                            row
                        })
                    );
                }
            }
            7 => {
                if versions.len() < 6 {
                    versions.push((c.clone(), model.clone()));
                }
            }
            8 => {
                if !versions.is_empty() {
                    let k = below(&mut rng, versions.len() as u32) as usize;
                    let (old, old_model) = versions.swap_remove(k);
                    equal(&old, &old_model);
                }
            }
            _ => {
                // Read a few rows directly, and one past the end.
                for _ in 0..4 {
                    let len = model.len() as u32;
                    let row = below(&mut rng, len + 1);
                    assert_eq!(c.get(ix(row)), model.get(row as usize), "step {step}");
                }
            }
        }
        chunk_rule(&c);
        if step % 16 == 0 {
            equal(&c, &model);
            for (old, old_model) in &versions {
                equal(old, old_model);
                chunk_rule(old);
            }
        }
    }
    equal(&c, &model);
    for (old, old_model) in &versions {
        equal(old, old_model);
    }
}

/// An arbitrary row for a link into an arena of `rows` rows: mostly in range, sometimes just
/// past it, sometimes far past it.
fn any_row(rng: &mut SplitMix64, rows: u32, dangling: bool) -> u32 {
    if !dangling {
        return below(rng, rows.max(1));
    }
    match below(rng, 16) {
        0 => rows,
        1 => rows + below(rng, 1000),
        2 => u32::MAX - 1 - below(rng, 4),
        _ => below(rng, rows.max(1)),
    }
}

/// A column of `n` arbitrary links into an arena of `rows` rows.
fn links<K: Kind, T>(rng: &mut SplitMix64, n: u32, rows: u32, dangling: bool) -> Column<K, Ix<T>> {
    let mut c = Column::new();
    for _ in 0..n {
        c.push(ix(any_row(rng, rows, dangling))).unwrap();
    }
    c
}

/// A column of `n` arbitrary links into an arena of `rows` rows, absent one time in four.
fn options<K: Kind, T>(
    rng: &mut SplitMix64,
    n: u32,
    rows: u32,
    dangling: bool,
) -> Column<K, Option<Ix<T>>> {
    let mut c = Column::new();
    for _ in 0..n {
        let v = (below(rng, 4) != 0).then(|| ix(any_row(rng, rows, dangling)));
        c.push(v).unwrap();
    }
    c
}

/// Arbitrary arenas: arbitrary sizes and links, with links past their arenas if `dangling` and
/// columns one row off if `uneven`.
fn any_arenas(rng: &mut SplitMix64, dangling: bool, uneven: bool) -> Arenas {
    let mut a = Arenas::new();
    // Without dangling links every arena has a row, so every required link has a target.
    let rows: Vec<u32> = (0..11)
        .map(|_| below(rng, 6) + u32::from(!dangling))
        .collect();
    let r = |arena: Arena| rows[Arena::ALL.iter().position(|&x| x == arena).unwrap()];
    // A length that is the arena's, or now and then one off if `uneven`.
    let len = |rng: &mut SplitMix64, n: u32| match below(rng, 16) {
        0 if uneven => n + 1,
        1 if uneven && n > 0 => n - 1,
        _ => n,
    };
    let (rg, sh, fa, lo, fi, ed, ve) = (
        r(Arena::Region),
        r(Arena::Shell),
        r(Arena::Face),
        r(Arena::Loop),
        r(Arena::Fin),
        r(Arena::Edge),
        r(Arena::Vertex),
    );
    for _ in 0..len(rng, rg) {
        let kind = if rng.coin() {
            RegionKind::Void
        } else {
            RegionKind::Solid
        };
        a.regions.kind.push(kind).unwrap();
    }
    let n = len(rng, rg);
    a.regions.first_shell = options(rng, n, sh, dangling);
    let n = len(rng, sh);
    a.shells.region = links(rng, n, rg, dangling);
    let n = len(rng, sh);
    a.shells.next = options(rng, n, sh, dangling);
    let n = len(rng, sh);
    a.shells.first_face = options(rng, n, fa, dangling);
    let n = len(rng, fa);
    a.faces.shell = links(rng, n, sh, dangling);
    let n = len(rng, fa);
    a.faces.next = options(rng, n, fa, dangling);
    let n = len(rng, fa);
    a.faces.surface = links(rng, n, r(Arena::Surface), dangling);
    a.faces.sense = Column::filled(Sense::Forward, len(rng, fa)).unwrap();
    let n = len(rng, fa);
    a.faces.first_loop = options(rng, n, lo, dangling);
    let b = geom_math::Box3::new(
        geom_num::grid::P3::new(0, 0, 0).unwrap(),
        geom_num::grid::P3::new(1, 1, 1).unwrap(),
    )
    .unwrap();
    a.faces.bound = Column::filled(b, len(rng, fa)).unwrap();
    let n = len(rng, lo);
    a.loops.face = links(rng, n, fa, dangling);
    let n = len(rng, lo);
    a.loops.next = options(rng, n, lo, dangling);
    let n = len(rng, lo);
    a.loops.first_fin = options(rng, n, fi, dangling);
    let n = len(rng, fi);
    a.fins.loop_ = links(rng, n, lo, dangling);
    let n = len(rng, fi);
    a.fins.next = links(rng, n, fi, dangling);
    let n = len(rng, fi);
    a.fins.prev = links(rng, n, fi, dangling);
    let n = len(rng, fi);
    a.fins.edge = links(rng, n, ed, dangling);
    a.fins.sense = Column::filled(Sense::Reversed, len(rng, fi)).unwrap();
    let n = len(rng, fi);
    a.fins.radial = links(rng, n, fi, dangling);
    let n = len(rng, fi);
    a.fins.pcurve = options(rng, n, r(Arena::Pcurve), dangling);
    let n = len(rng, ed);
    a.edges.first_fin = options(rng, n, fi, dangling);
    let n = len(rng, ed);
    a.edges.start = options(rng, n, ve, dangling);
    let n = len(rng, ed);
    a.edges.end = options(rng, n, ve, dangling);
    let n = len(rng, ed);
    a.edges.curve = links(rng, n, r(Arena::Curve), dangling);
    a.vertices.point = links(rng, ve, r(Arena::Point), dangling);
    for _ in 0..r(Arena::Surface) {
        a.surfaces.push(common::plane([0.0; 3], 0, 1)).unwrap();
    }
    for _ in 0..r(Arena::Curve) {
        a.curves.push(common::line([0, 0, 0], [1, 0, 0])).unwrap();
    }
    for _ in 0..r(Arena::Pcurve) {
        let pcurve = geom_curve::Line::new([0.0, 0.0], [1.0, 0.0], (0.0, 1.0)).unwrap();
        a.pcurves.push(geom_curve::CurveKind::Line(pcurve)).unwrap();
    }
    for _ in 0..r(Arena::Point) {
        a.points
            .push(geom_num::grid::P3::new(0, 0, 0).unwrap().into())
            .unwrap();
    }
    a
}

/// Every link column of `a` as (link, values or `None` for absent).
fn columns(a: &Arenas) -> Vec<(Link, Vec<Option<u32>>)> {
    fn of<K, T>(c: &Column<K, Ix<T>>) -> Vec<Option<u32>> {
        c.iter().map(|i| Some(i.row())).collect()
    }
    fn opt<K, T>(c: &Column<K, Option<Ix<T>>>) -> Vec<Option<u32>> {
        c.iter().map(|i| i.map(Ix::row)).collect()
    }
    vec![
        (Link::RegionFirstShell, opt(&a.regions.first_shell)),
        (Link::ShellRegion, of(&a.shells.region)),
        (Link::ShellNext, opt(&a.shells.next)),
        (Link::ShellFirstFace, opt(&a.shells.first_face)),
        (Link::FaceShell, of(&a.faces.shell)),
        (Link::FaceNext, opt(&a.faces.next)),
        (Link::FaceSurface, of(&a.faces.surface)),
        (Link::FaceFirstLoop, opt(&a.faces.first_loop)),
        (Link::LoopFace, of(&a.loops.face)),
        (Link::LoopNext, opt(&a.loops.next)),
        (Link::LoopFirstFin, opt(&a.loops.first_fin)),
        (Link::FinLoop, of(&a.fins.loop_)),
        (Link::FinNext, of(&a.fins.next)),
        (Link::FinPrev, of(&a.fins.prev)),
        (Link::FinEdge, of(&a.fins.edge)),
        (Link::FinRadial, of(&a.fins.radial)),
        (Link::FinPcurve, opt(&a.fins.pcurve)),
        (Link::EdgeFirstFin, opt(&a.edges.first_fin)),
        (Link::EdgeStart, opt(&a.edges.start)),
        (Link::EdgeEnd, opt(&a.edges.end)),
        (Link::EdgeCurve, of(&a.edges.curve)),
        (Link::VertexPoint, of(&a.vertices.point)),
    ]
}

/// The same rules as publishing, written independently: lengths per arena, then links in order,
/// then the outside region.
fn expected(a: &Arenas, outside: u32) -> Result<(), TopoError> {
    let lengths: [(Arena, Vec<u32>); 6] = [
        (
            Arena::Region,
            vec![a.regions.kind.len(), a.regions.first_shell.len()],
        ),
        (
            Arena::Shell,
            vec![
                a.shells.region.len(),
                a.shells.next.len(),
                a.shells.first_face.len(),
            ],
        ),
        (
            Arena::Face,
            vec![
                a.faces.shell.len(),
                a.faces.next.len(),
                a.faces.surface.len(),
                a.faces.sense.len(),
                a.faces.first_loop.len(),
                a.faces.bound.len(),
            ],
        ),
        (
            Arena::Loop,
            vec![
                a.loops.face.len(),
                a.loops.next.len(),
                a.loops.first_fin.len(),
            ],
        ),
        (
            Arena::Fin,
            vec![
                a.fins.loop_.len(),
                a.fins.next.len(),
                a.fins.prev.len(),
                a.fins.edge.len(),
                a.fins.sense.len(),
                a.fins.radial.len(),
                a.fins.pcurve.len(),
            ],
        ),
        (
            Arena::Edge,
            vec![
                a.edges.first_fin.len(),
                a.edges.start.len(),
                a.edges.end.len(),
                a.edges.curve.len(),
            ],
        ),
    ];
    for (arena, l) in lengths {
        if l.iter().any(|&n| n != l[0]) {
            return Err(TopoError::Ragged(arena));
        }
    }
    for (link, values) in columns(a) {
        let rows = a.rows(link.to());
        if let Some(row) = values.iter().position(|v| v.is_some_and(|t| t >= rows)) {
            return Err(TopoError::Dangling {
                link,
                row: row as u32,
            });
        }
    }
    let void = a.regions.kind.iter().nth(outside as usize) == Some(&RegionKind::Void);
    if void {
        Ok(())
    } else {
        Err(TopoError::Outside)
    }
}

#[test]
fn publishing_arbitrary_arenas_never_panics_and_agrees_with_an_independent_check() {
    let seed = resolve("geom-topo publish harness", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut outcomes = [0u32; 4];
    for case in 0..4000 {
        let shape = below(&mut rng, 3);
        let a = any_arenas(&mut rng, shape == 1, shape == 2);
        let wild = below(&mut rng, 4) == 0;
        let outside = any_row(&mut rng, a.rows(Arena::Region), wild);
        let want = expected(&a, outside);
        let b = Builder { arenas: a };
        let got = b.publish(BodyKind::General, ix(outside), &mut meter());
        match (&got, &want) {
            (Ok(body), Ok(())) => {
                outcomes[0] += 1;
                for (link, values) in columns(body.arenas()) {
                    let rows = body.arenas().rows(link.to());
                    assert!(
                        values.iter().flatten().all(|&t| t < rows),
                        "case {case}: {link}"
                    );
                }
            }
            (Err(e), Err(w)) => {
                assert_eq!(e, w, "case {case}");
                outcomes[match e {
                    TopoError::Ragged(_) => 1,
                    TopoError::Dangling { .. } => 2,
                    _ => 3,
                }] += 1;
            }
            _ => panic!("case {case}: published {got:?}, expected {want:?}"),
        }
    }
    println!(
        "published {}, ragged {}, dangling {}, outside {}",
        outcomes[0], outcomes[1], outcomes[2], outcomes[3]
    );
    assert!(
        outcomes.iter().all(|&n| n > 100),
        "every outcome is drawn often: {outcomes:?}"
    );
}
