//! Group 1 of chapter 8.3, combinatorial: every loop closes, every fin has a partner or is a
//! boundary, shells are closed and consistently oriented, regions nest correctly, and Euler's
//! formula holds.
//!
//! What it does: the check runs in two stages, as Parasolid gates its body check. The first
//! follows every list and ring of the body once: each region's shells, each shell's faces and each
//! face's loops (a first child and a `next` chain, each child naming its parent), each loop's fins
//! (a ring both ways), each edge's fins (a radial ring), and each edge's two ends. It finds a list
//! that comes back on itself, a child listed twice or by the wrong parent or by none, a ring that
//! does not close, a fin off its edge's ring, and an edge with one end. Only a body whose lists
//! and rings are sound goes on to the second stage, which reads them:
//!
//! - **What the body kind allows:** no faces in a wire or an acorn, no edges in an acorn, no edge
//!   without fins in a solid or a sheet, no vertex on no edge outside an acorn or a general body.
//! - **Loops close:** each fin ends at the vertex where the next fin of its loop starts (a fin runs
//!   from its edge's start to its end when `Forward`, the other way when `Reversed`), and a closed
//!   edge without vertices is the only fin of its loop.
//! - **Fin partners:** in a solid every edge has exactly two fins; in a sheet one (a boundary) or
//!   two; two partners run opposite ways along their edge and lie in one shell, so each shell is
//!   closed (in a solid) and consistently oriented. A general body may be non-manifold: any number
//!   of fins, in the radial order its operation recorded.
//! - **Shells:** every shell has faces, and they are one set connected through edges shared within
//!   the shell.
//! - **Regions:** a solid region has a shell; a solid body has a solid region, and each of its
//!   shells belongs to one, as the body bounds material; a sheet, wire or acorn body has no solid
//!   region.
//! - **Euler's formula,** for each shell of a solid or a sheet: with V vertices (a closed edge
//!   without vertices counting one), E edges, F faces and L loops, the characteristic
//!   V − E + 2F − L equals 2 − 2G for a closed shell of genus G, and 2 − 2G − B for a sheet with B
//!   boundary cycles. So the check fails a closed shell whose characteristic is odd or above 2,
//!   and a sheet whose characteristic plus its boundary cycles is.
//!
//! What it does not do: whether one shell lies inside another, and whether a loop runs the right
//! way round its face, depend on geometry and belong to groups 2 and 3. A general body's Euler
//! formula needs the non-manifold form, which no piece has built yet.
//!
//! Guarantees:
//!
//! - **Finite on any body.** Every walk of the first stage marks each row it visits and stops at
//!   the first row it meets twice, so it visits each row at most once, however the links are
//!   broken; the second stage walks only rings and lists the first found sound.
//! - **The second stage reads only sound structure.** It runs only when the first found nothing,
//!   so a fault it reports is real, never an echo of a broken list.
//! - **Every fault, in a fixed order.** The checks run in the order above, rows ascending, so the
//!   same body gives the same faults in the same order on every machine.
//! - **Charged.** One work unit per row visited, to the caller's meter.
//!
//! Method: Parasolid's published body check, its loop, fin and shell checks and its gating
//! (`projects/kit/notes/G01-evidence/02-occt-parasolid-acis.md`, section 5, and the checking
//! chapter it links, <http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.09.html>);
//! the shell check of Open CASCADE, each edge used exactly twice in a closed shell
//! (`BRepCheck_Shell`, linked from the same section); and the Euler–Poincaré formula for boundary representations,
//! V − E + F − (L − F) = 2(S − G):
//!
//! - M. Mäntylä, *An Introduction to Solid Modeling*, Computer Science Press, 1988, chapter 3.
//! - A. Hatcher, *Algebraic Topology*, Cambridge University Press, 2002, section 2.2 (the Euler
//!   characteristic of a compact surface, 2 − 2g − b), <https://pi.math.cornell.edu/~hatcher/AT/AT.pdf>.
//! - R. E. Tarjan and J. van Leeuwen, "Worst-case analysis of set union algorithms", *Journal of
//!   the ACM* 31 (1984), 245–281, <https://doi.org/10.1145/62.2160>, for the connected sets (path
//!   halving).
//!
//! Cost: linear in the body's rows, plus each set operation's time with path halving, logarithmic
//! amortised at worst (Tarjan and van Leeuwen); memory: a few words per row of each arena,
//! reserved before use.

use core::iter::successors;

use geom_run::Stopped;
use geom_topo::entity::{
    Edge, EdgeIx, Face, FaceIx, Fin, FinIx, Loop, LoopIx, Region, RegionIx, Shell, ShellIx, Vertex,
    VertexIx,
};
use geom_topo::ix::Arena;
use geom_topo::{Arenas, Body, BodyKind, Column, Ix, RegionKind, Sense};

use crate::report::{Entity, Fault, FaultKind};
use crate::sink::Sink;

/// The words of scratch space group 1 needs for `a`: the most any one of its checks uses at once.
pub(crate) fn words(a: &Arenas) -> usize {
    let rows = |arena| usize::try_from(a.rows(arena)).unwrap_or(usize::MAX);
    let vertices = rows(Arena::Vertex);
    [
        rows(Arena::Shell),
        rows(Arena::Face),
        rows(Arena::Loop),
        rows(Arena::Fin),
        vertices.saturating_mul(3).saturating_add(rows(Arena::Edge)),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
}

/// Group 1 on `body`: the first stage, then the second if the first found nothing. `scratch`
/// holds at least [`words`] words.
pub(crate) fn check(
    body: &Body,
    scratch: &mut [u32],
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
    let before = sink.found();
    let a = body.arenas();
    lists(a, scratch, sink)?;
    fin_rings(a, scratch, sink)?;
    radial_rings(a, scratch, sink)?;
    edge_ends(a, sink)?;
    if sink.found() != before {
        return Ok(());
    }
    let kind = body.kind();
    contents(a, kind, scratch, sink)?;
    loops_close(a, sink)?;
    partners(a, kind, sink)?;
    shells(a, scratch, sink)?;
    regions(a, kind, sink)?;
    if matches!(kind, BodyKind::Solid | BodyKind::Sheet) {
        euler(a, kind, scratch, sink)?;
    }
    Ok(())
}

// Total accessors. Publishing guarantees every link of a body is a row of the arena it names
// (`geom_topo::Builder::publish`), so each lookup below finds its row; the fallback a lookup
// names is never taken, and keeps the check free of branches no body can reach.

/// Every row of an arena of `rows` rows, ascending.
fn rows<K>(rows: u32) -> impl Iterator<Item = Ix<K>> {
    (0..rows).filter_map(Ix::new)
}

/// The value of an optional link column at a row.
fn link<K, T: Copy>(column: &Column<K, Option<T>>, at: Ix<K>) -> Option<T> {
    column.get(at).copied().flatten()
}

/// A fin's next fin in its loop.
fn next(a: &Arenas, f: FinIx) -> FinIx {
    a.fins.next.get(f).copied().unwrap_or(f)
}

/// A fin's next fin around its edge.
fn radial(a: &Arenas, f: FinIx) -> FinIx {
    a.fins.radial.get(f).copied().unwrap_or(f)
}

/// A fin's edge.
fn fin_edge(a: &Arenas, f: FinIx) -> Option<EdgeIx> {
    a.fins.edge.get(f).copied()
}

/// The face a fin's loop belongs to.
fn fin_face(a: &Arenas, f: FinIx) -> Option<FaceIx> {
    a.fins
        .loop_
        .get(f)
        .copied()
        .and_then(|l| a.loops.face.get(l).copied())
}

/// The shell a fin's face lies in.
fn fin_shell(a: &Arenas, f: FinIx) -> Option<ShellIx> {
    fin_face(a, f).and_then(|x| a.faces.shell.get(x).copied())
}

/// An edge's start and end.
fn ends(a: &Arenas, e: Option<EdgeIx>) -> (Option<VertexIx>, Option<VertexIx>) {
    (
        e.and_then(|e| link(&a.edges.start, e)),
        e.and_then(|e| link(&a.edges.end, e)),
    )
}

/// Where a fin starts and ends: its edge's start and end, swapped when the fin is `Reversed`;
/// `None` for a closed edge without vertices.
fn fin_ends(a: &Arenas, f: FinIx) -> Option<(VertexIx, VertexIx)> {
    let (start, end) = ends(a, fin_edge(a, f));
    let reversed = a.fins.sense.get(f) == Some(&Sense::Reversed);
    start
        .zip(end)
        .map(|(s, t)| if reversed { (t, s) } else { (s, t) })
}

/// The fins of a ring from `first`, following `step` until it comes back (rings the first stage
/// found sound).
fn ring(
    a: &Arenas,
    first: FinIx,
    step: fn(&Arenas, FinIx) -> FinIx,
) -> impl Iterator<Item = FinIx> + '_ {
    successors(Some(first), move |&f| {
        let n = step(a, f);
        (n != first).then_some(n)
    })
}

/// The fins of a loop, from its first.
fn loop_fins(a: &Arenas, l: LoopIx) -> impl Iterator<Item = FinIx> + '_ {
    link(&a.loops.first_fin, l)
        .into_iter()
        .flat_map(move |first| ring(a, first, next))
}

/// The fins of an edge, in radial order from its first.
fn edge_fins(a: &Arenas, e: EdgeIx) -> impl Iterator<Item = FinIx> + '_ {
    link(&a.edges.first_fin, e)
        .into_iter()
        .flat_map(move |first| ring(a, first, radial))
}

/// Sets `v[i]` to `value` and returns what it held.
fn swap_in<T: Copy>(v: &mut [T], i: usize, value: T) -> Option<T> {
    v.get_mut(i).map(|slot| core::mem::replace(slot, value))
}

/// A row number as an index of a slice.
fn at(row: u32) -> usize {
    usize::try_from(row).unwrap_or(usize::MAX)
}

/// Marks `v[row]` with `stamp`; whether it held another mark before.
fn mark(v: &mut [u32], row: u32, stamp: u32) -> bool {
    swap_in(v, at(row), stamp).is_some_and(|was| was != stamp)
}

/// The first `len` words of `scratch`, zeroed, and the words after them.
fn take(scratch: &mut [u32], len: u32) -> (&mut [u32], &mut [u32]) {
    let (head, tail) = scratch.split_at_mut(at(len).min(scratch.len()));
    head.fill(0);
    (head, tail)
}

/// One parent-to-children list: each parent's first child and the children's `next` chain, each
/// child naming its parent.
struct List<P, C> {
    /// How many parents and children there are.
    parents: u32,
    children: u32,
    /// The parent's first child, a child's next sibling, and a child's parent.
    first: fn(&Arenas, Ix<P>) -> Option<Ix<C>>,
    next: fn(&Arenas, Ix<C>) -> Option<Ix<C>>,
    parent: fn(&Arenas, Ix<C>) -> Option<Ix<P>>,
    /// Each as an entity, for the faults.
    parent_entity: fn(Ix<P>) -> Entity,
    child_entity: fn(Ix<C>) -> Entity,
}

/// Walks one list kind: a cycle, a child listed twice or by the wrong parent, and children no
/// parent lists.
fn walk<P, C>(
    a: &Arenas,
    list: &List<P, C>,
    scratch: &mut [u32],
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
    // Each child's parent row plus one, 0 while no parent has listed it.
    let (owner, _) = take(scratch, list.children);
    for p in rows::<P>(list.parents) {
        let mark = p.row().saturating_add(1);
        let mut next = (list.first)(a, p);
        while let Some(child) = next {
            sink.charge()?;
            match swap_in(owner, child.to_usize(), mark).unwrap_or(0) {
                0 => {}
                o if o == mark => {
                    sink.push(Fault::at(FaultKind::ChainCycle, (list.parent_entity)(p)));
                    break;
                }
                _ => {
                    let fault = Fault::at(FaultKind::ListedTwice, (list.child_entity)(child));
                    sink.push(fault.with((list.parent_entity)(p)));
                    break;
                }
            }
            if (list.parent)(a, child) != Some(p) {
                let fault = Fault::at(FaultKind::WrongParent, (list.child_entity)(child));
                sink.push(fault.with((list.parent_entity)(p)));
            }
            next = (list.next)(a, child);
        }
    }
    for child in rows::<C>(list.children) {
        if owner.get(child.to_usize()) == Some(&0) {
            sink.push(Fault::at(FaultKind::Orphan, (list.child_entity)(child)));
        }
    }
    Ok(())
}

/// Each region's shells, each shell's faces and each face's loops.
fn lists(a: &Arenas, scratch: &mut [u32], sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    walk::<Region, Shell>(
        a,
        &List {
            parents: a.rows(Arena::Region),
            children: a.rows(Arena::Shell),
            first: |a, r| link(&a.regions.first_shell, r),
            next: |a, s| link(&a.shells.next, s),
            parent: |a, s| a.shells.region.get(s).copied(),
            parent_entity: Entity::Region,
            child_entity: Entity::Shell,
        },
        scratch,
        sink,
    )?;
    walk::<Shell, Face>(
        a,
        &List {
            parents: a.rows(Arena::Shell),
            children: a.rows(Arena::Face),
            first: |a, s| link(&a.shells.first_face, s),
            next: |a, f| link(&a.faces.next, f),
            parent: |a, f| a.faces.shell.get(f).copied(),
            parent_entity: Entity::Shell,
            child_entity: Entity::Face,
        },
        scratch,
        sink,
    )?;
    walk::<Face, Loop>(
        a,
        &List {
            parents: a.rows(Arena::Face),
            children: a.rows(Arena::Loop),
            first: |a, f| link(&a.faces.first_loop, f),
            next: |a, l| link(&a.loops.next, l),
            parent: |a, l| a.loops.face.get(l).copied(),
            parent_entity: Entity::Face,
            child_entity: Entity::Loop,
        },
        scratch,
        sink,
    )
}

/// Each loop's ring of fins: it comes back to its first fin, each fin names the loop and is its
/// next fin's previous one, and no fin is in two rings or in none.
fn fin_rings(a: &Arenas, scratch: &mut [u32], sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    // Each fin's loop row plus one, 0 while no loop's ring has reached it.
    let (owner, _) = take(scratch, a.rows(Arena::Fin));
    for l in rows::<Loop>(a.rows(Arena::Loop)) {
        let mark = l.row().saturating_add(1);
        let Some(first) = link(&a.loops.first_fin, l) else {
            sink.push(Fault::at(FaultKind::EmptyLoop, Entity::Loop(l)));
            continue;
        };
        let mut f = first;
        loop {
            sink.charge()?;
            match swap_in(owner, f.to_usize(), mark).unwrap_or(0) {
                0 => {}
                o if o == mark => {
                    sink.push(Fault::at(FaultKind::FinRing, Entity::Loop(l)).with(Entity::Fin(f)));
                    break;
                }
                _ => {
                    sink.push(
                        Fault::at(FaultKind::ListedTwice, Entity::Fin(f)).with(Entity::Loop(l)),
                    );
                    break;
                }
            }
            if a.fins.loop_.get(f).copied() != Some(l) {
                sink.push(Fault::at(FaultKind::WrongParent, Entity::Fin(f)).with(Entity::Loop(l)));
            }
            let n = next(a, f);
            if a.fins.prev.get(n).copied() != Some(f) {
                sink.push(Fault::at(FaultKind::FinPrev, Entity::Fin(f)));
            }
            if n == first {
                break;
            }
            f = n;
        }
    }
    for f in rows::<Fin>(a.rows(Arena::Fin)) {
        if owner.get(f.to_usize()) == Some(&0) {
            sink.push(Fault::at(FaultKind::Orphan, Entity::Fin(f)));
        }
    }
    Ok(())
}

/// Each edge's radial ring of fins: it comes back to the edge's first fin, every fin on it names
/// the edge, and every fin is on a ring.
fn radial_rings(a: &Arenas, scratch: &mut [u32], sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    // 1 for each fin some edge's ring has reached.
    let (on, _) = take(scratch, a.rows(Arena::Fin));
    for e in rows::<Edge>(a.rows(Arena::Edge)) {
        let Some(first) = link(&a.edges.first_fin, e) else {
            continue;
        };
        let mut f = first;
        loop {
            sink.charge()?;
            if swap_in(on, f.to_usize(), 1) == Some(1) {
                sink.push(Fault::at(FaultKind::RadialRing, Entity::Edge(e)).with(Entity::Fin(f)));
                break;
            }
            if fin_edge(a, f) != Some(e) {
                sink.push(Fault::at(FaultKind::RadialEdge, Entity::Edge(e)).with(Entity::Fin(f)));
            }
            let n = radial(a, f);
            if n == first {
                break;
            }
            f = n;
        }
    }
    for f in rows::<Fin>(a.rows(Arena::Fin)) {
        if on.get(f.to_usize()) == Some(&0) {
            sink.push(Fault::at(FaultKind::OffRadialRing, Entity::Fin(f)));
        }
    }
    Ok(())
}

/// Each edge has both its vertices or neither (a closed edge).
fn edge_ends(a: &Arenas, sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    for e in rows::<Edge>(a.rows(Arena::Edge)) {
        sink.charge()?;
        let (start, end) = ends(a, Some(e));
        if start.is_some() != end.is_some() {
            sink.push(Fault::at(FaultKind::EdgeEnds, Entity::Edge(e)));
        }
    }
    Ok(())
}

/// What the body kind allows: faces, edges, edges without fins, vertices on no edge.
fn contents(
    a: &Arenas,
    kind: BodyKind,
    scratch: &mut [u32],
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
    let faces_allowed = !matches!(kind, BodyKind::Wire | BodyKind::Acorn);
    let edges_allowed = kind != BodyKind::Acorn;
    let finless_allowed = matches!(kind, BodyKind::Wire | BodyKind::General);
    let lone_vertex_allowed = matches!(kind, BodyKind::Acorn | BodyKind::General);
    for f in rows::<Face>(a.rows(Arena::Face)) {
        sink.charge()?;
        if !faces_allowed {
            sink.push(Fault::at(FaultKind::Misplaced, Entity::Face(f)));
        }
    }
    // 1 for each vertex an edge uses.
    let (used, _) = take(scratch, a.rows(Arena::Vertex));
    for e in rows::<Edge>(a.rows(Arena::Edge)) {
        sink.charge()?;
        let finless = link(&a.edges.first_fin, e).is_none();
        if !edges_allowed || (finless && !finless_allowed) {
            sink.push(Fault::at(FaultKind::Misplaced, Entity::Edge(e)));
        }
        let (start, end) = ends(a, Some(e));
        for v in start.into_iter().chain(end) {
            mark(used, v.row(), 1);
        }
    }
    for v in rows::<Vertex>(a.rows(Arena::Vertex)) {
        sink.charge()?;
        if !lone_vertex_allowed && used.get(v.to_usize()) == Some(&0) {
            sink.push(Fault::at(FaultKind::Misplaced, Entity::Vertex(v)));
        }
    }
    Ok(())
}

/// Each loop closes: each fin ends where the next fin starts, and a closed edge without vertices
/// is alone in its loop.
fn loops_close(a: &Arenas, sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    for l in rows::<Loop>(a.rows(Arena::Loop)) {
        let alone = link(&a.loops.first_fin, l).is_some_and(|f| next(a, f) == f);
        for f in loop_fins(a, l) {
            sink.charge()?;
            match (fin_ends(a, f), fin_ends(a, next(a, f))) {
                (Some((_, head)), Some((tail, _))) if head != tail => {
                    sink.push(Fault::at(FaultKind::LoopOpen, Entity::Loop(l)).with(Entity::Fin(f)));
                }
                (None, _) if !alone => {
                    let fault = Fault::at(FaultKind::RingEdgeInLoop, Entity::Loop(l));
                    sink.push(fault.with(Entity::Fin(f)));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Fin partners: how many fins each edge has for its body kind, and two partners run opposite
/// ways and lie in one shell.
fn partners(a: &Arenas, kind: BodyKind, sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    let allowed: &[usize] = match kind {
        BodyKind::Solid => &[2],
        BodyKind::Sheet => &[1, 2],
        BodyKind::Wire | BodyKind::Acorn | BodyKind::General => return Ok(()),
    };
    for e in rows::<Edge>(a.rows(Arena::Edge)) {
        let mut count = 0usize;
        let mut pair: [Option<FinIx>; 2] = [None; 2];
        for f in edge_fins(a, e) {
            sink.charge()?;
            swap_in(&mut pair, count, Some(f));
            count = count.saturating_add(1);
        }
        if count == 0 {
            continue;
        }
        if !allowed.contains(&count) {
            let fins = i64::try_from(count).unwrap_or(i64::MAX);
            sink.push(Fault::at(FaultKind::FinCount, Entity::Edge(e)).value(fins));
            continue;
        }
        if let [Some(one), Some(two)] = pair {
            if a.fins.sense.get(one) == a.fins.sense.get(two) {
                sink.push(Fault::at(FaultKind::PartnerSense, Entity::Edge(e)));
            }
            if fin_shell(a, one) != fin_shell(a, two) {
                sink.push(Fault::at(FaultKind::PartnerShells, Entity::Edge(e)));
            }
        }
    }
    Ok(())
}

/// Disjoint sets of rows, with path halving (Tarjan and van Leeuwen), in one slice of scratch:
/// each row's parent. Linking puts the second set's root under the first's; which root goes under
/// which never changes the sets, only the trees' depth, which path halving keeps small.
struct Sets<'s> {
    parent: &'s mut [u32],
}

impl<'s> Sets<'s> {
    /// `n` rows, each its own set, in the first words of `scratch`; and the words after them.
    fn new(scratch: &'s mut [u32], n: u32) -> (Sets<'s>, &'s mut [u32]) {
        let (parent, rest) = take(scratch, n);
        for (row, p) in (0u32..).zip(parent.iter_mut()) {
            *p = row;
        }
        (Sets { parent }, rest)
    }

    /// The parent of `row` (itself at a root).
    fn up(&self, row: u32) -> u32 {
        self.parent.get(at(row)).copied().unwrap_or(row)
    }

    /// The set `row` is in, halving its path. A path is never longer than the rows, so the walk
    /// is bounded by them.
    fn find(&mut self, mut row: u32) -> u32 {
        for _ in 0..self.parent.len() {
            let up = self.up(row);
            if up == row {
                break;
            }
            let grand = self.up(up);
            swap_in(self.parent, at(row), grand);
            row = grand;
        }
        row
    }

    /// Joins the sets of `x` and `y`.
    fn join(&mut self, x: u32, y: u32) {
        let (x, y) = (self.find(x), self.find(y));
        swap_in(self.parent, at(y), x);
    }
}

/// Every shell has faces, and they are one set connected through edges shared within the shell.
fn shells(a: &Arenas, scratch: &mut [u32], sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    let (mut sets, _) = Sets::new(scratch, a.rows(Arena::Face));
    for f in rows::<Fin>(a.rows(Arena::Fin)) {
        sink.charge()?;
        let n = radial(a, f);
        if fin_shell(a, f) == fin_shell(a, n) {
            fin_face(a, f)
                .zip(fin_face(a, n))
                .into_iter()
                .for_each(|(x, y)| sets.join(x.row(), y.row()));
        }
    }
    for s in rows::<Shell>(a.rows(Arena::Shell)) {
        let Some(first) = link(&a.shells.first_face, s) else {
            sink.push(Fault::at(FaultKind::EmptyShell, Entity::Shell(s)));
            continue;
        };
        let root = sets.find(first.row());
        let mut at = link(&a.faces.next, first);
        while let Some(face) = at {
            sink.charge()?;
            if sets.find(face.row()) != root {
                let fault = Fault::at(FaultKind::ShellDisconnected, Entity::Shell(s));
                sink.push(fault.with(Entity::Face(face)));
                break;
            }
            at = link(&a.faces.next, face);
        }
    }
    Ok(())
}

/// Regions: a solid region has a shell; a solid body has a solid region and its shells belong to
/// solid regions; a sheet, wire or acorn body has no solid region.
fn regions(a: &Arenas, kind: BodyKind, sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    let solid = |r: RegionIx| a.regions.kind.get(r) == Some(&RegionKind::Solid);
    let mut material = false;
    for r in rows::<Region>(a.rows(Arena::Region)) {
        sink.charge()?;
        if !solid(r) {
            continue;
        }
        material = true;
        if link(&a.regions.first_shell, r).is_none() {
            sink.push(Fault::at(FaultKind::EmptyRegion, Entity::Region(r)));
        }
        if matches!(kind, BodyKind::Sheet | BodyKind::Wire | BodyKind::Acorn) {
            sink.push(Fault::at(FaultKind::MaterialInKind, Entity::Region(r)));
        }
    }
    if kind == BodyKind::Solid {
        if !material {
            sink.push(Fault::at(FaultKind::NoMaterial, Entity::Body));
        }
        for s in rows::<Shell>(a.rows(Arena::Shell)) {
            sink.charge()?;
            if !a.shells.region.get(s).copied().is_some_and(solid) {
                sink.push(Fault::at(FaultKind::ShellOfVoid, Entity::Shell(s)));
            }
        }
    }
    Ok(())
}

/// One shell's counts for Euler's formula.
#[derive(Default)]
struct Counts {
    vertices: i64,
    edges: i64,
    faces: i64,
    loops: i64,
    /// Boundary cycles (in a sheet): sets of boundary vertices, and boundary edges without
    /// vertices.
    boundary: i64,
}

/// Which shell last counted each vertex, edge and set of boundary vertices: the shell's row plus
/// one, 0 for none.
struct Marks<'s> {
    vertices: &'s mut [u32],
    edges: &'s mut [u32],
    roots: &'s mut [u32],
}

/// Euler's formula for each shell of a solid or a sheet.
fn euler(
    a: &Arenas,
    kind: BodyKind,
    scratch: &mut [u32],
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
    let (vertices, rest) = take(scratch, a.rows(Arena::Vertex));
    let (edges, rest) = take(rest, a.rows(Arena::Edge));
    let (roots, rest) = take(rest, a.rows(Arena::Vertex));
    let mut marks = Marks {
        vertices,
        edges,
        roots,
    };
    let (mut sets, _) = Sets::new(rest, a.rows(Arena::Vertex));
    // The boundary cycles: boundary vertices joined along the boundary edges (one fin each).
    for f in rows::<Fin>(a.rows(Arena::Fin)) {
        sink.charge()?;
        if radial(a, f) == f
            && let (Some(x), Some(y)) = ends(a, fin_edge(a, f))
        {
            sets.join(x.row(), y.row());
        }
    }
    for s in rows::<Shell>(a.rows(Arena::Shell)) {
        let stamp = s.row().saturating_add(1);
        let mut c = Counts::default();
        let mut face = link(&a.shells.first_face, s);
        while let Some(at) = face {
            c.faces = c.faces.saturating_add(1);
            let mut lp = link(&a.faces.first_loop, at);
            while let Some(l) = lp {
                c.loops = c.loops.saturating_add(1);
                for f in loop_fins(a, l) {
                    sink.charge()?;
                    count_fin(a, f, stamp, &mut c, &mut sets, &mut marks);
                }
                lp = link(&a.loops.next, l);
            }
            face = link(&a.faces.next, at);
        }
        let characteristic = c
            .vertices
            .saturating_sub(c.edges)
            .saturating_add(c.faces.saturating_mul(2))
            .saturating_sub(c.loops);
        let sum = if kind == BodyKind::Sheet {
            characteristic.saturating_add(c.boundary)
        } else {
            characteristic
        };
        if sum.rem_euclid(2) != 0 || sum > 2 {
            sink.push(Fault::at(FaultKind::Euler, Entity::Shell(s)).value(sum));
        }
    }
    Ok(())
}

/// Counts one fin's edge, its vertices and its boundary cycle for the shell stamped `stamp`, each
/// once.
fn count_fin(
    a: &Arenas,
    f: FinIx,
    stamp: u32,
    c: &mut Counts,
    sets: &mut Sets<'_>,
    marks: &mut Marks<'_>,
) {
    let e = fin_edge(a, f);
    if !e.is_some_and(|e| mark(marks.edges, e.row(), stamp)) {
        return;
    }
    c.edges = c.edges.saturating_add(1);
    let boundary = radial(a, f) == f;
    if let (Some(start), Some(end)) = ends(a, e) {
        for v in [start, end] {
            if mark(marks.vertices, v.row(), stamp) {
                c.vertices = c.vertices.saturating_add(1);
            }
        }
        if boundary && mark(marks.roots, sets.find(start.row()), stamp) {
            c.boundary = c.boundary.saturating_add(1);
        }
    } else {
        // A closed edge without vertices counts one vertex, and is a boundary cycle on its own.
        c.vertices = c.vertices.saturating_add(1);
        if boundary {
            c.boundary = c.boundary.saturating_add(1);
        }
    }
}
