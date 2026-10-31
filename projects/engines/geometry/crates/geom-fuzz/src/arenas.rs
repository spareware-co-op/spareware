//! Fuzz target: `geom-topo`'s arenas publish exactly the bodies their rules allow, and a body's
//! versions never see each other's edits.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a row count for each of the
//! eleven arenas, a value for every link of every row (inside its arena, at its end, far past it,
//! or absent where the link may be), a column that is one row longer or shorter than its arena,
//! the outside region, and edits to make to a published body. [`check`] fills the arenas, publishes
//! them, and compares the answer with an independent reading of `geom-topo`'s rules (the length of
//! every column, then every link in the order of `Link::ALL`, then the outside region): the same
//! body or the same first fault. When the body publishes, it checks that every index it holds is
//! inside its arenas, that an edit starting from it publishes the same body, and that the edits
//! leave the first version unchanged. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case. Each
//!   link is read through a rule byte that makes the boundaries common: the last row, the first
//!   row past the end, `u32::MAX − 1`, or absent.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a disagreement is a [`Finding`]** naming the law and the case.
//!
//! Method: differential testing against an independent reading of chapter 8.2's rules as
//! `geom-topo`'s `builder` module states them (`projects/engines/geometry/architecture/08-topology.md`),
//! written here from the rules, not from the code: lengths per arena from region to edge, then
//! the links in their published order, rows ascending, then the outside region.
//!
//! Cost: at most [`MAX_ROWS`] rows per arena and [`MAX_EDITS`] edits: a few hundred column writes
//! and one publish per version; at most [`MAX_INPUT_BYTES`] bytes read.

use geom_curve::{CurveKind, Line};
use geom_math::Box3;
use geom_num::grid::P3;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};
use geom_test::{ByteSource, Entropy};
use geom_topo::{
    Arena, Arenas, Body, BodyKind, Builder, Column, Ix, Kind, Link, RegionKind, Sense, TopoError,
};

use crate::Finding;

/// The most rows an arena of a case holds.
pub const MAX_ROWS: u8 = 5;

/// The most edits a case makes to a published body.
pub const MAX_EDITS: u8 = 4;

/// The arenas' link columns, as (link, whether the link may be absent).
const LINKS: [(Link, bool); 22] = [
    (Link::RegionFirstShell, true),
    (Link::ShellRegion, false),
    (Link::ShellNext, true),
    (Link::ShellFirstFace, true),
    (Link::FaceShell, false),
    (Link::FaceNext, true),
    (Link::FaceSurface, false),
    (Link::FaceFirstLoop, true),
    (Link::LoopFace, false),
    (Link::LoopNext, true),
    (Link::LoopFirstFin, true),
    (Link::FinLoop, false),
    (Link::FinNext, false),
    (Link::FinPrev, false),
    (Link::FinEdge, false),
    (Link::FinRadial, false),
    (Link::FinPcurve, true),
    (Link::EdgeFirstFin, true),
    (Link::EdgeStart, true),
    (Link::EdgeEnd, true),
    (Link::EdgeCurve, false),
    (Link::VertexPoint, false),
];

/// The most bytes [`decode`] reads from any input: 1,063.
///
/// A mode byte. Eleven row counts, one byte each. Each link of each row: a rule byte, and for a row read from
/// the input eight bytes more, so at most 9 bytes for each of the 22 link columns' [`MAX_ROWS`]
/// rows. A byte for each of at most [`MAX_ROWS`] regions' kinds. Two bytes for the uneven column.
/// The outside region, read as a link: 9. The edit count: 1. Each of at most [`MAX_EDITS`] edits:
/// a link byte, a row byte and a link's 9. So 1 + 11 + 22 · 5 · 9 + 5 + 2 + 9 + 1 + 4 · 11 = 1,063.
/// The unit test `the_input_bound_is_reached_and_never_passed` builds that input.
pub const MAX_INPUT_BYTES: usize = 1 + 11 + 22 * 5 * 9 + 5 + 2 + 9 + 1 + 4 * 11;

const _: () = assert!(MAX_INPUT_BYTES == 1063);

/// A link's value in a case: absent, or the row it points at.
pub type Target = Option<u32>;

/// One edit to a published body: the link column, the row (modulo the column's rows) and the new
/// value, which may point past the end, so that publishing the edit can fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edit {
    /// The link column written.
    pub link: Link,
    /// The row written, before taking it modulo the column's rows.
    pub row: u8,
    /// The value written (an absent value is written as row 0 where the link may not be absent).
    pub value: Target,
}

/// One case: the arenas' rows, every link's values, the uneven column, the outside region and the
/// edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// Each arena's rows, in `Arena::ALL`'s order.
    pub rows: [u8; 11],
    /// Each link column's values, in [`LINKS`]'s order, one per row of the arena that holds it.
    pub links: Vec<Vec<Target>>,
    /// Each region's kind: void if true.
    pub void: Vec<bool>,
    /// A link column made one row longer (`Some((i, true))`) or shorter (`Some((i, false))`)
    /// than its arena, by its place in [`LINKS`].
    pub uneven: Option<(usize, bool)>,
    /// The outside region.
    pub outside: u32,
    /// The edits made to the body, if it publishes.
    pub edits: Vec<Edit>,
}

/// The index of `arena` in `Arena::ALL`.
fn place(arena: Arena) -> usize {
    Arena::ALL.iter().position(|&a| a == arena).unwrap_or(0)
}

/// A link's value through its rule byte: inside the arena, its last row, one past its end, far
/// past it, or absent; with `faults` false, only inside the arena or absent.
fn target(src: &mut ByteSource<'_>, rows: u32, optional: bool, faults: bool) -> Target {
    match src.byte() % 8 {
        0 if optional => None,
        1 => Some(rows.saturating_sub(1)),
        2 if faults => Some(rows),
        3 if faults => Some(u32::MAX - 1),
        4 if faults => src
            .word()
            .checked_rem(u64::from(rows.saturating_add(3)))
            .and_then(|v| u32::try_from(v).ok()),
        _ => Some(u32::from(src.byte()).checked_rem(rows.max(1)).unwrap_or(0)),
    }
}

/// Decodes `data` into a case, and returns it with the number of bytes read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    // Without faults (an even mode byte, and every input that runs out), every arena has a row
    // and every link is inside its arena, so bodies publish; with them, anything goes.
    let faults = src.byte() % 2 == 1;
    let mut rows = [0u8; 11];
    for r in &mut rows {
        *r = if faults {
            src.byte() % (MAX_ROWS + 1)
        } else {
            (src.byte() % MAX_ROWS).saturating_add(1)
        };
    }
    let count = |arena: Arena| u32::from(rows.get(place(arena)).copied().unwrap_or(0));
    let links = LINKS
        .iter()
        .map(|&(link, optional)| {
            (0..count(link.from()))
                .map(|_| target(&mut src, count(link.to()), optional, faults))
                .collect()
        })
        .collect();
    let void = (0..count(Arena::Region))
        .map(|_| src.byte() % 2 == 0)
        .collect();
    let pick = src.byte();
    let uneven = (faults && pick % 4 == 0).then(|| {
        (
            usize::from(src.byte())
                .checked_rem(LINKS.len())
                .unwrap_or(0),
            pick % 8 == 0,
        )
    });
    let outside = target(&mut src, count(Arena::Region), false, faults).unwrap_or(0);
    let edits = (0..src.byte() % (MAX_EDITS + 1))
        .map(|_| {
            let (link, optional) = LINKS
                .get(
                    usize::from(src.byte())
                        .checked_rem(LINKS.len())
                        .unwrap_or(0),
                )
                .copied()
                .unwrap_or((Link::FaceShell, false));
            let row = src.byte();
            Edit {
                link,
                row,
                value: target(&mut src, count(link.to()), optional, faults),
            }
        })
        .collect();
    let read = data.len().saturating_sub(src.remaining());
    (
        Case {
            rows,
            links,
            void,
            uneven,
            outside,
            edits,
        },
        read,
    )
}

/// The harness every fuzzing engine drives: decode `data` into a case, then [`check`].
///
/// # Errors
///
/// As for [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

/// Publishes the case's arenas with `geom-topo`'s builder and checks the laws.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    check_with(case, |b, kind, outside, meter| {
        b.publish(kind, outside, meter)
    })
}

/// [`check`] with `publish` in place of the builder's: how a test shows that a broken publish is
/// caught.
///
/// # Errors
///
/// As for [`check`].
pub fn check_with<P>(case: &Case, publish: P) -> Result<(), Finding>
where
    P: Fn(Builder, BodyKind, Ix<geom_topo::Region>, &mut WorkMeter<'_>) -> Result<Body, TopoError>,
{
    let arenas = fill(case);
    let Some(outside) = Ix::new(case.outside) else {
        return Ok(());
    };
    let want = expected(&arenas, case.outside);
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let got = publish(
        Builder {
            arenas: arenas.clone(),
        },
        BodyKind::General,
        outside,
        &mut meter,
    );
    let body = match (got, want) {
        (Ok(body), Ok(())) => body,
        (Err(e), Err(w)) if e == w => return Ok(()),
        (got, want) => {
            return Err(Finding::new(
                "publishing accepts exactly the arenas the rules allow, and names the first fault",
                || {
                    format!(
                        "got {:?}, expected {want:?}; case {case:?}",
                        got.map(|_| ())
                    )
                },
            ));
        }
    };
    if body.arenas() != &arenas || body.outside() != outside {
        return Err(Finding::new(
            "a published body is its arenas, unchanged",
            || format!("case {case:?}"),
        ));
    }
    if let Some((link, row)) = outside_links(body.arenas()) {
        return Err(Finding::new(
            "every index a published body holds is inside its arenas",
            || format!("{link} at row {row}; case {case:?}"),
        ));
    }
    let again = publish(Builder::edit(&body), BodyKind::General, outside, &mut meter);
    if again.as_ref().ok() != Some(&body) {
        return Err(Finding::new(
            "an edit that changes nothing publishes the same body",
            || format!("case {case:?}"),
        ));
    }
    let snapshot = body.clone();
    let mut edited = Builder::edit(&body);
    for edit in &case.edits {
        write(&mut edited.arenas, edit);
    }
    let want = expected(&edited.arenas, case.outside);
    let got = publish(edited, BodyKind::General, outside, &mut meter);
    if got.as_ref().map(|_| ()).map_err(|e| *e) != want {
        return Err(Finding::new(
            "an edited version publishes exactly when the rules allow",
            || format!("case {case:?}"),
        ));
    }
    if body != snapshot {
        return Err(Finding::new(
            "a new version never changes the old one",
            || format!("case {case:?}"),
        ));
    }
    Ok(())
}

/// The arenas a case describes.
fn fill(case: &Case) -> Arenas {
    let mut a = Arenas::new();
    let count = |arena: Arena| u32::from(case.rows.get(place(arena)).copied().unwrap_or(0));
    for &void in &case.void {
        let kind = if void {
            RegionKind::Void
        } else {
            RegionKind::Solid
        };
        let _ = a.regions.kind.push(kind);
    }
    for (i, &(link, _)) in LINKS.iter().enumerate() {
        let values = case.links.get(i).map_or(&[][..], Vec::as_slice);
        for value in values {
            push(&mut a, link, *value);
        }
    }
    for _ in 0..count(Arena::Face) {
        let _ = a.faces.sense.push(Sense::Forward);
        if let Ok(p) = P3::new(0, 0, 0)
            && let Ok(b) = Box3::new(p, p)
        {
            let _ = a.faces.bound.push(b);
        }
    }
    for _ in 0..count(Arena::Fin) {
        let _ = a.fins.sense.push(Sense::Reversed);
    }
    let domain = ((0.0, 1.0), (0.0, 1.0));
    for _ in 0..count(Arena::Surface) {
        if let Ok(plane) = Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], domain) {
            let _ = a.surfaces.push(SurfaceKind::Plane(plane));
        }
    }
    for _ in 0..count(Arena::Curve) {
        if let Ok(line) = Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 1.0)) {
            let _ = a.curves.push(CurveKind::Line(line));
        }
    }
    for _ in 0..count(Arena::Pcurve) {
        if let Ok(line) = Line::new([0.0; 2], [1.0, 0.0], (0.0, 1.0)) {
            let _ = a.pcurves.push(CurveKind::Line(line));
        }
    }
    for _ in 0..count(Arena::Point) {
        if let Ok(p) = P3::new(0, 0, 0) {
            let _ = a.points.push(p);
        }
    }
    if let Some((i, longer)) = case.uneven
        && let Some(&(link, _)) = LINKS.get(i)
    {
        if longer {
            push(&mut a, link, Some(0));
        } else {
            drop_last(&mut a, link);
        }
    }
    a
}

/// Adds `value` as a new last row of a column whose link may not be absent (row 0 if it is).
fn push_req<K: Kind, T>(c: &mut Column<K, Ix<T>>, value: Target) {
    if let Some(v) = Ix::new(value.unwrap_or(0)) {
        let _ = c.push(v);
    }
}

/// Adds `value` as a new last row of a column whose link may be absent.
fn push_opt<K: Kind, T>(c: &mut Column<K, Option<Ix<T>>>, value: Target) {
    let _ = c.push(value.and_then(Ix::new));
}

/// Adds `value` as a new last row of `link`'s column.
fn push(a: &mut Arenas, link: Link, value: Target) {
    match link {
        Link::RegionFirstShell => push_opt(&mut a.regions.first_shell, value),
        Link::ShellRegion => push_req(&mut a.shells.region, value),
        Link::ShellNext => push_opt(&mut a.shells.next, value),
        Link::ShellFirstFace => push_opt(&mut a.shells.first_face, value),
        Link::FaceShell => push_req(&mut a.faces.shell, value),
        Link::FaceNext => push_opt(&mut a.faces.next, value),
        Link::FaceSurface => push_req(&mut a.faces.surface, value),
        Link::FaceFirstLoop => push_opt(&mut a.faces.first_loop, value),
        Link::LoopFace => push_req(&mut a.loops.face, value),
        Link::LoopNext => push_opt(&mut a.loops.next, value),
        Link::LoopFirstFin => push_opt(&mut a.loops.first_fin, value),
        Link::FinLoop => push_req(&mut a.fins.loop_, value),
        Link::FinNext => push_req(&mut a.fins.next, value),
        Link::FinPrev => push_req(&mut a.fins.prev, value),
        Link::FinEdge => push_req(&mut a.fins.edge, value),
        Link::FinRadial => push_req(&mut a.fins.radial, value),
        Link::FinPcurve => push_opt(&mut a.fins.pcurve, value),
        Link::EdgeFirstFin => push_opt(&mut a.edges.first_fin, value),
        Link::EdgeStart => push_opt(&mut a.edges.start, value),
        Link::EdgeEnd => push_opt(&mut a.edges.end, value),
        Link::EdgeCurve => push_req(&mut a.edges.curve, value),
        Link::VertexPoint => push_req(&mut a.vertices.point, value),
    }
}

/// A column with its last row removed, by rebuilding it from the others.
fn shorter<K: Kind, V: Clone>(c: &Column<K, V>) -> Column<K, V> {
    let mut out = Column::new();
    let keep = usize::try_from(c.len().saturating_sub(1)).unwrap_or(0);
    let _ = out.extend(c.iter().take(keep).cloned().collect::<Vec<_>>());
    out
}

/// Removes the last row of `link`'s column.
fn drop_last(a: &mut Arenas, link: Link) {
    match link {
        Link::RegionFirstShell => a.regions.first_shell = shorter(&a.regions.first_shell),
        Link::ShellRegion => a.shells.region = shorter(&a.shells.region),
        Link::ShellNext => a.shells.next = shorter(&a.shells.next),
        Link::ShellFirstFace => a.shells.first_face = shorter(&a.shells.first_face),
        Link::FaceShell => a.faces.shell = shorter(&a.faces.shell),
        Link::FaceNext => a.faces.next = shorter(&a.faces.next),
        Link::FaceSurface => a.faces.surface = shorter(&a.faces.surface),
        Link::FaceFirstLoop => a.faces.first_loop = shorter(&a.faces.first_loop),
        Link::LoopFace => a.loops.face = shorter(&a.loops.face),
        Link::LoopNext => a.loops.next = shorter(&a.loops.next),
        Link::LoopFirstFin => a.loops.first_fin = shorter(&a.loops.first_fin),
        Link::FinLoop => a.fins.loop_ = shorter(&a.fins.loop_),
        Link::FinNext => a.fins.next = shorter(&a.fins.next),
        Link::FinPrev => a.fins.prev = shorter(&a.fins.prev),
        Link::FinEdge => a.fins.edge = shorter(&a.fins.edge),
        Link::FinRadial => a.fins.radial = shorter(&a.fins.radial),
        Link::FinPcurve => a.fins.pcurve = shorter(&a.fins.pcurve),
        Link::EdgeFirstFin => a.edges.first_fin = shorter(&a.edges.first_fin),
        Link::EdgeStart => a.edges.start = shorter(&a.edges.start),
        Link::EdgeEnd => a.edges.end = shorter(&a.edges.end),
        Link::EdgeCurve => a.edges.curve = shorter(&a.edges.curve),
        Link::VertexPoint => a.vertices.point = shorter(&a.vertices.point),
    }
}

/// Sets row `row` of a column whose link may not be absent to `value` (row 0 if it is).
fn set_req<K: Kind, T>(c: &mut Column<K, Ix<T>>, row: u32, value: Target) {
    if let (Some(r), Some(v)) = (Ix::new(row), Ix::new(value.unwrap_or(0))) {
        let _ = c.set(r, v);
    }
}

/// Sets row `row` of a column whose link may be absent to `value`.
fn set_opt<K: Kind, T>(c: &mut Column<K, Option<Ix<T>>>, row: u32, value: Target) {
    if let Some(r) = Ix::new(row) {
        let _ = c.set(r, value.and_then(Ix::new));
    }
}

/// Writes one edit: `edit.value` at `edit.row` modulo the column's rows, if it has any.
fn write(a: &mut Arenas, edit: &Edit) {
    let rows = a.rows(edit.link.from());
    let Some(row) = u32::from(edit.row).checked_rem(rows) else {
        return;
    };
    let v = edit.value;
    match edit.link {
        Link::RegionFirstShell => set_opt(&mut a.regions.first_shell, row, v),
        Link::ShellRegion => set_req(&mut a.shells.region, row, v),
        Link::ShellNext => set_opt(&mut a.shells.next, row, v),
        Link::ShellFirstFace => set_opt(&mut a.shells.first_face, row, v),
        Link::FaceShell => set_req(&mut a.faces.shell, row, v),
        Link::FaceNext => set_opt(&mut a.faces.next, row, v),
        Link::FaceSurface => set_req(&mut a.faces.surface, row, v),
        Link::FaceFirstLoop => set_opt(&mut a.faces.first_loop, row, v),
        Link::LoopFace => set_req(&mut a.loops.face, row, v),
        Link::LoopNext => set_opt(&mut a.loops.next, row, v),
        Link::LoopFirstFin => set_opt(&mut a.loops.first_fin, row, v),
        Link::FinLoop => set_req(&mut a.fins.loop_, row, v),
        Link::FinNext => set_req(&mut a.fins.next, row, v),
        Link::FinPrev => set_req(&mut a.fins.prev, row, v),
        Link::FinEdge => set_req(&mut a.fins.edge, row, v),
        Link::FinRadial => set_req(&mut a.fins.radial, row, v),
        Link::FinPcurve => set_opt(&mut a.fins.pcurve, row, v),
        Link::EdgeFirstFin => set_opt(&mut a.edges.first_fin, row, v),
        Link::EdgeStart => set_opt(&mut a.edges.start, row, v),
        Link::EdgeEnd => set_opt(&mut a.edges.end, row, v),
        Link::EdgeCurve => set_req(&mut a.edges.curve, row, v),
        Link::VertexPoint => set_req(&mut a.vertices.point, row, v),
    }
}

/// Every link column of `a`, as (link, each row's target).
fn columns(a: &Arenas) -> Vec<(Link, Vec<Target>)> {
    fn of<K, T>(c: &Column<K, Ix<T>>) -> Vec<Target> {
        c.iter().map(|i| Some(i.row())).collect()
    }
    fn opt<K, T>(c: &Column<K, Option<Ix<T>>>) -> Vec<Target> {
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

/// The first link of `a`, in order, holding an index outside its arena, with its row.
fn outside_links(a: &Arenas) -> Option<(Link, u32)> {
    columns(a).into_iter().find_map(|(link, values)| {
        let rows = a.rows(link.to());
        values
            .iter()
            .position(|v| v.is_some_and(|t| t >= rows))
            .and_then(|row| u32::try_from(row).ok())
            .map(|row| (link, row))
    })
}

/// The rules, read independently: every arena's columns from region to edge hold its rows, then
/// every link is inside its arena, then the outside region is a void region.
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
        if l.iter().any(|n| Some(n) != l.first()) {
            return Err(TopoError::Ragged(arena));
        }
    }
    if let Some((link, row)) = outside_links(a) {
        return Err(TopoError::Dangling { link, row });
    }
    let void = usize::try_from(outside)
        .ok()
        .and_then(|o| a.regions.kind.iter().nth(o))
        == Some(&RegionKind::Void);
    if void {
        Ok(())
    } else {
        Err(TopoError::Outside)
    }
}
