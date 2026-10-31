//! Publishing in name order (piece G81): a body's arena indices assigned in persistent-name order,
//! so its arenas depend only on what it contains, never on how it was built (8.2, 32.17).
//!
//! What it does: [`Builder::publish_named`] takes one name per region, face, edge and vertex,
//! checks the arenas as [`Builder::publish`] does, moves every named arena's rows into name order,
//! places the arenas without names (shells, loops, fins) and the geometry pools by a fixed rule
//! from those names, rewrites every link through the moves, rebuilds every list from the
//! back-links, and publishes the result, carrying its names in name order.
//!
//! The fixed rule, one key per row, compared in the order listed (8.2: they "follow a fixed rule
//! derived from the names of the faces, edges and vertices they join"):
//!
//! - **Shell:** its region's place, then the least place among its faces.
//! - **Loop:** its face's place, then the least of its fins' edge places, each with its fin's sense
//!   (forward first), so two loops of one face that share an edge differ.
//! - **Fin:** its edge's place, then its loop's place, then its sense (forward first).
//! - **Surface, curve, pcurve, point:** the least place among the faces, edges, fins or vertices
//!   that lie on it.
//!
//! A place is a row's index once its arena is in order, so every key comes from names alone. A
//! shell with no face or a loop with no fin has no place from its members; two such rows of one
//! region or face, or any two rows with equal keys, are refused rather than placed by a guess, and
//! so is a pool row nothing uses.
//!
//! Guarantees:
//!
//! - **Index order is name order.** Every named arena's rows are in name order, and every other
//!   arena's rows are in the order of their keys above; so any order of the same rows, with their
//!   links moved to match, publishes the same body (32.17).
//! - **Every link follows its row.** A link that pointed at a row points at that row's new place,
//!   so the body joins the same entities as the arenas given.
//! - **Lists follow the back-links, in name order.** Each region's shells, each shell's faces and
//!   each face's loops are chained from the least row up, and each loop's and each edge's first fin
//!   is its least fin, all read from the links each entity holds to its owner (a shell's region, a
//!   face's shell, a loop's face, a fin's loop and edge). What the lists given held is replaced, so
//!   they never depend on the order a body was built in either. A loop's cycle of fins and an
//!   edge's radial ring are kept as given: their order is the shape's.
//! - **Checked as published.** The arenas pass [`Builder::publish`]'s checks before anything is
//!   read, and the moved arenas pass them again as they are published.
//! - **What did not change stays shared.** A chunk that comes out holding what the arena given
//!   held at that place is that arena's own chunk, so a new version shares it with the version it
//!   was made from (23.8, 23.9), and republishing a body shares them all. Integers are compared; a
//!   pool of doubles shares a chunk only if none of its rows moved, since equal doubles can differ
//!   in bits.
//! - **One answer, the first fault.** Faults are reported in a fixed order: [`Builder::publish`]'s
//!   checks; then, for regions, faces, edges and vertices in turn, a count of names other than one
//!   per row or two rows of one name; then equal keys among shells, loops and fins in turn; then an
//!   unused row of the surface, curve, pcurve and point pools in turn.
//! - **Metered.** Besides what [`Builder::publish`] charges, twice (on the arenas given and on the
//!   moved ones), publishing by name charges one unit for every chunk of rows it moves.
//!
//! Method: the boolean rebuild's renumbering in name order (`projects/engines/geometry/architecture/
//! 11-booleans.md`, 11.3), done for every body by the builder, as decided in
//! `projects/engines/geometry/architecture/32-decisions.md`, 32.17, and laid out in
//! `projects/engines/geometry/architecture/08-topology.md`, 8.2.
//!
//! Cost: one sort of each arena's keys, `O(n log n)` in its rows, and one pass that copies every
//! row once into new chunks, and one more pass over the owner links to rebuild the lists. Nothing
//! recurses.

use alloc::vec::Vec;

use geom_run::WorkMeter;

use super::{Builder, even_columns, inside};
use crate::arenas::{Arenas, Pool};
use crate::body::{Body, BodyKind};
use crate::column::{CHUNK_ROWS, Column};
use crate::entity::{Edge, Face, Fin, Loop, Region, RegionIx, RegionKind, Sense, Shell, Vertex};
use crate::error::TopoError;
use crate::ix::{Arena, Ix, Kind};
use crate::order::{NameKey, Names, Permutation};

impl Builder {
    /// Checks the arenas, assigns every index in name order and publishes them as an immutable body
    /// of `kind`, whose infinite void region is `outside`, carrying `names` in name order.
    ///
    /// `names` holds one name per region, face, edge and vertex, in the arenas' row order.
    ///
    /// # Errors
    ///
    /// The first fault, in the module's order: any of [`Builder::publish`]'s errors;
    /// [`TopoError::Names`] if a named arena's names are not one per row;
    /// [`TopoError::DuplicateName`] if two rows of a named arena share a name;
    /// [`TopoError::Unordered`] if two shells, loops or fins get equal keys;
    /// [`TopoError::Unused`] if no entity uses a row of a geometry pool.
    pub fn publish_named(
        self,
        names: &Names,
        kind: BodyKind,
        outside: RegionIx,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Body, TopoError> {
        let a = &self.arenas;
        even_columns(a)?;
        inside(a, meter)?;
        if a.regions.kind.get(outside) != Some(&RegionKind::Void) {
            return Err(TopoError::Outside);
        }
        let moves = Moves::new(a, names)?;
        let mut arenas = rechain(moves.arenas(a, meter)?)?;
        share(&mut arenas, a, &moves);
        let sorted = Names {
            regions: in_order(&moves.regions, &names.regions),
            faces: in_order(&moves.faces, &names.faces),
            edges: in_order(&moves.edges, &names.edges),
            vertices: in_order(&moves.vertices, &names.vertices),
        };
        let outside = to(&moves.regions, outside)?;
        let body = Builder { arenas }.publish(kind, outside, meter)?;
        Ok(body.named(sorted))
    }
}

/// Whether two chunks of a column of integers hold the same rows.
fn equal<V: PartialEq>(_place: usize, mine: &[V], theirs: &[V]) -> bool {
    mine == theirs
}

/// Whether no row of chunk `place`, of `rows` rows, moved.
fn unmoved(moves: &Permutation, place: usize, rows: usize) -> bool {
    let first = u32::try_from(place).map_or(u32::MAX, |p| p.saturating_mul(CHUNK_ROWS));
    (first..)
        .take(rows)
        .all(|row| moves.source(row) == Some(row))
}

/// Gives `new`, the arenas `moves` made from `old`, each of `old`'s chunks it holds unchanged.
fn share(new: &mut Arenas, old: &Arenas, moves: &Moves<Permutation>) {
    new.regions.kind.share_with(&old.regions.kind, equal);
    new.regions
        .first_shell
        .share_with(&old.regions.first_shell, equal);
    new.shells.region.share_with(&old.shells.region, equal);
    new.shells.next.share_with(&old.shells.next, equal);
    new.shells
        .first_face
        .share_with(&old.shells.first_face, equal);
    new.faces.shell.share_with(&old.faces.shell, equal);
    new.faces.next.share_with(&old.faces.next, equal);
    new.faces.surface.share_with(&old.faces.surface, equal);
    new.faces.sense.share_with(&old.faces.sense, equal);
    new.faces
        .first_loop
        .share_with(&old.faces.first_loop, equal);
    new.faces.bound.share_with(&old.faces.bound, equal);
    new.loops.face.share_with(&old.loops.face, equal);
    new.loops.next.share_with(&old.loops.next, equal);
    new.loops.first_fin.share_with(&old.loops.first_fin, equal);
    new.fins.loop_.share_with(&old.fins.loop_, equal);
    new.fins.next.share_with(&old.fins.next, equal);
    new.fins.prev.share_with(&old.fins.prev, equal);
    new.fins.edge.share_with(&old.fins.edge, equal);
    new.fins.sense.share_with(&old.fins.sense, equal);
    new.fins.radial.share_with(&old.fins.radial, equal);
    new.fins.pcurve.share_with(&old.fins.pcurve, equal);
    new.edges.first_fin.share_with(&old.edges.first_fin, equal);
    new.edges.start.share_with(&old.edges.start, equal);
    new.edges.end.share_with(&old.edges.end, equal);
    new.edges.curve.share_with(&old.edges.curve, equal);
    new.vertices.point.share_with(&old.vertices.point, equal);
    new.points.share_with(&old.points, equal);
    new.surfaces
        .share_with(&old.surfaces, |place, mine, theirs| {
            mine.len() == theirs.len() && unmoved(&moves.surfaces, place, mine.len())
        });
    new.curves.share_with(&old.curves, |place, mine, theirs| {
        mine.len() == theirs.len() && unmoved(&moves.curves, place, mine.len())
    });
    new.pcurves.share_with(&old.pcurves, |place, mine, theirs| {
        mine.len() == theirs.len() && unmoved(&moves.pcurves, place, mine.len())
    });
}

/// The move into name order of a named arena of `rows` rows.
fn by_name(names: &[NameKey], rows: u32, arena: Arena) -> Result<Permutation, TopoError> {
    if u32::try_from(names.len()).ok() != Some(rows) {
        return Err(TopoError::Names(arena));
    }
    order(names, arena, true)
}

/// The move into the order of `keys`; equal keys are refused, as [`TopoError::DuplicateName`]
/// in a `named` arena and as [`TopoError::Unordered`] in any other.
fn order<T: Ord>(keys: &[T], arena: Arena, named: bool) -> Result<Permutation, TopoError> {
    Permutation::by_key(keys).map_err(|e| e.in_arena(arena, named))
}

/// For each of `owners` rows, the least place among its members, or `None` with none. Each pair
/// is one member's owner and place; a member with no owner (a fin without a pcurve) is skipped.
fn least<T: Ord + Copy>(
    owners: u32,
    pairs: impl Iterator<Item = (Option<u32>, Option<T>)>,
) -> Vec<Option<T>> {
    let mut best: Vec<Option<T>> = (0..owners).map(|_| None).collect();
    for pair in pairs {
        let (Some(owner), Some(place)) = pair else {
            continue;
        };
        // Every owner is a row of its arena, which `inside` checked.
        usize::try_from(owner)
            .ok()
            .and_then(|o| best.get_mut(o))
            .into_iter()
            .for_each(|slot| *slot = Some(slot.map_or(place, |b| b.min(place))));
    }
    best
}

/// The move into key order of a geometry pool of `rows` rows, each row keyed by the least place of
/// what lies on it; a row nothing uses is [`TopoError::Unused`].
fn pool(
    rows: u32,
    pairs: impl Iterator<Item = (Option<u32>, Option<u32>)>,
    arena: Arena,
) -> Result<Permutation, TopoError> {
    let keys = least(rows, pairs);
    if let Some(row) = (0_u32..)
        .zip(&keys)
        .find_map(|(r, k)| k.is_none().then_some(r))
    {
        return Err(TopoError::Unused { arena, row });
    }
    order(&keys, arena, false)
}

/// A fin's place among a loop's fins: its edge's place, then its sense.
type FinKey = (u32, u8);

/// A sense's place in a fin's key: forward first.
const fn sense(s: Sense) -> u8 {
    match s {
        Sense::Forward => 0,
        Sense::Reversed => 1,
    }
}

/// `names`, one per row, in the order `moves` puts the rows in (as many rows: checked).
fn in_order(moves: &Permutation, names: &[NameKey]) -> Vec<NameKey> {
    moves.apply(names).unwrap_or_default()
}

/// Where an arena's rows go: a [`Permutation`] moves every row, a [`Keep`] drops some.
pub(crate) trait Place {
    /// The rows after the move.
    fn rows(&self) -> u32;
    /// The new row of the row `old`, or `None` if it is dropped.
    fn place(&self, old: u32) -> Option<u32>;
    /// The row that moves to `new`.
    fn source(&self, new: u32) -> Option<u32>;
}

impl Place for Permutation {
    fn rows(&self) -> u32 {
        self.len()
    }

    fn place(&self, old: u32) -> Option<u32> {
        self.new_of(old)
    }

    fn source(&self, new: u32) -> Option<u32> {
        self.old_of(new)
    }
}

/// The live rows of an arena, kept in their order, the others dropped.
pub(crate) struct Keep {
    /// The new row of each row, `None` for a dropped one.
    new_of_old: Vec<Option<u32>>,
    /// The row each new row comes from.
    old_of_new: Vec<u32>,
}

impl Keep {
    /// Keeps the rows whose flag in `live` is set.
    pub(crate) fn new(live: impl Iterator<Item = bool>) -> Keep {
        let mut keep = Keep {
            new_of_old: Vec::new(),
            old_of_new: Vec::new(),
        };
        for (old, alive) in (0_u32..).zip(live) {
            if alive {
                keep.new_of_old.push(Some(keep.rows()));
                keep.old_of_new.push(old);
            } else {
                keep.new_of_old.push(None);
            }
        }
        keep
    }

    /// `rows`, one per row before, with the dropped ones left out.
    pub(crate) fn apply<T: Clone>(&self, rows: &[T]) -> Vec<T> {
        self.old_of_new
            .iter()
            .filter_map(|&old| usize::try_from(old).ok().and_then(|o| rows.get(o)).cloned())
            .collect()
    }
}

impl Place for Keep {
    fn rows(&self) -> u32 {
        // At most one new row per row of a 32-bit arena.
        u32::try_from(self.old_of_new.len()).unwrap_or(u32::MAX)
    }

    fn place(&self, old: u32) -> Option<u32> {
        usize::try_from(old)
            .ok()
            .and_then(|o| self.new_of_old.get(o))
            .copied()
            .flatten()
    }

    fn source(&self, new: u32) -> Option<u32> {
        usize::try_from(new)
            .ok()
            .and_then(|n| self.old_of_new.get(n))
            .copied()
    }
}

/// The new place of `ix`; a link to a dropped row, or past the end, is [`TopoError::NoSuchRow`].
pub(crate) fn to<K: Kind, P: Place>(moves: &P, ix: Ix<K>) -> Result<Ix<K>, TopoError> {
    moves
        .place(ix.row())
        .and_then(Ix::new)
        .ok_or(TopoError::NoSuchRow {
            arena: K::ARENA,
            row: ix.row(),
        })
}

/// The new place of `ix`, if there is one.
fn to_some<K: Kind, P: Place>(moves: &P, ix: Option<Ix<K>>) -> Result<Option<Ix<K>>, TopoError> {
    ix.map(|i| to(moves, i)).transpose()
}

/// The rows of an arena moved by `moves`: row `new` is `get` of the row that moves there,
/// rewritten by `link`, charging `meter` one unit as each chunk begins.
fn moved<K: Kind, R, P: Place>(
    moves: &P,
    meter: &mut WorkMeter<'_>,
    get: impl Fn(Ix<K>) -> Option<R>,
    mut link: impl FnMut(R) -> Result<R, TopoError>,
) -> Result<Vec<R>, TopoError> {
    (0..moves.rows())
        .map(|new| {
            if new % CHUNK_ROWS == 0 {
                meter.charge(1)?;
            }
            let old = moves.source(new).unwrap_or(new);
            let row = Ix::new(old).and_then(&get).ok_or(TopoError::NoSuchRow {
                arena: K::ARENA,
                row: old,
            })?;
            link(row)
        })
        .collect()
}

/// A geometry pool with its rows moved by `moves`.
fn moved_pool<T: Kind + Clone, P: Place>(
    pool: &Pool<T>,
    moves: &P,
    meter: &mut WorkMeter<'_>,
) -> Result<Pool<T>, TopoError> {
    let rows = moved(moves, meter, |ix| pool.get(ix).cloned(), Ok)?;
    let mut out = Pool::new();
    out.extend(rows)?;
    Ok(out)
}

/// Every arena's move.
pub(crate) struct Moves<P> {
    pub(crate) regions: P,
    pub(crate) shells: P,
    pub(crate) faces: P,
    pub(crate) loops: P,
    pub(crate) fins: P,
    pub(crate) edges: P,
    pub(crate) vertices: P,
    pub(crate) surfaces: P,
    pub(crate) curves: P,
    pub(crate) pcurves: P,
    pub(crate) points: P,
}

impl Moves<Permutation> {
    /// Every arena's move into name order, from `names` and the keys of the module's rule.
    fn new(a: &Arenas, names: &Names) -> Result<Moves<Permutation>, TopoError> {
        let regions = by_name(&names.regions, a.regions.len(), Arena::Region)?;
        let faces = by_name(&names.faces, a.faces.len(), Arena::Face)?;
        let edges = by_name(&names.edges, a.edges.len(), Arena::Edge)?;
        let vertices = by_name(&names.vertices, a.vertices.len(), Arena::Vertex)?;

        let shell_faces = least(
            a.shells.len(),
            a.faces
                .shell
                .iter()
                .map(|s| Some(s.row()))
                .zip(faces.iter().map(Some)),
        );
        let shell_keys: Vec<(Option<u32>, Option<u32>)> = a
            .shells
            .region
            .iter()
            .map(|r| regions.new_of(r.row()))
            .zip(shell_faces)
            .collect();
        let shells = order(&shell_keys, Arena::Shell, false)?;

        let loop_edges = least(
            a.loops.len(),
            a.fins.loop_.iter().map(|l| Some(l.row())).zip(
                a.fins
                    .edge
                    .iter()
                    .zip(a.fins.sense.iter())
                    .map(|(e, s)| edges.new_of(e.row()).map(|p| (p, sense(*s)))),
            ),
        );
        let loop_keys: Vec<(Option<u32>, Option<FinKey>)> = a
            .loops
            .face
            .iter()
            .map(|f| faces.new_of(f.row()))
            .zip(loop_edges)
            .collect();
        let loops = order(&loop_keys, Arena::Loop, false)?;

        let fin_keys: Vec<(Option<u32>, Option<u32>, u8)> = a
            .fins
            .edge
            .iter()
            .zip(a.fins.loop_.iter())
            .zip(a.fins.sense.iter())
            .map(|((e, l), s)| (edges.new_of(e.row()), loops.new_of(l.row()), sense(*s)))
            .collect();
        let fins = order(&fin_keys, Arena::Fin, false)?;

        let surfaces = pool(
            a.surfaces.len(),
            a.faces
                .surface
                .iter()
                .map(|s| Some(s.row()))
                .zip(faces.iter().map(Some)),
            Arena::Surface,
        )?;
        let curves = pool(
            a.curves.len(),
            a.edges
                .curve
                .iter()
                .map(|c| Some(c.row()))
                .zip(edges.iter().map(Some)),
            Arena::Curve,
        )?;
        let pcurves = pool(
            a.pcurves.len(),
            a.fins
                .pcurve
                .iter()
                .map(|p| p.map(Ix::row))
                .zip(fins.iter().map(Some)),
            Arena::Pcurve,
        )?;
        let points = pool(
            a.points.len(),
            a.vertices
                .point
                .iter()
                .map(|p| Some(p.row()))
                .zip(vertices.iter().map(Some)),
            Arena::Point,
        )?;

        Ok(Moves {
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
        })
    }
}

impl<P: Place> Moves<P> {
    /// `a` moved: every row at its new place, every link pointing at its target's new place, and
    /// every list left empty for [`rechain`] to rebuild (a region's first shell, a shell's next
    /// and first face, a face's next and first loop, a loop's next and first fin, an edge's first
    /// fin).
    pub(crate) fn arenas(
        &self,
        a: &Arenas,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Arenas, TopoError> {
        let mut out = Arenas::new();
        let regions = moved(
            &self.regions,
            meter,
            |ix| a.regions.get(ix),
            |r: Region| {
                Ok(Region {
                    kind: r.kind,
                    first_shell: None,
                })
            },
        )?;
        out.regions.extend(&regions)?;
        let shells = moved(
            &self.shells,
            meter,
            |ix| a.shells.get(ix),
            |s: Shell| {
                Ok(Shell {
                    region: to(&self.regions, s.region)?,
                    next: None,
                    first_face: None,
                })
            },
        )?;
        out.shells.extend(&shells)?;
        let faces = moved(
            &self.faces,
            meter,
            |ix| a.faces.get(ix),
            |f: Face| {
                Ok(Face {
                    shell: to(&self.shells, f.shell)?,
                    next: None,
                    surface: to(&self.surfaces, f.surface)?,
                    sense: f.sense,
                    first_loop: None,
                    bound: f.bound,
                })
            },
        )?;
        out.faces.extend(&faces)?;
        let loops = moved(
            &self.loops,
            meter,
            |ix| a.loops.get(ix),
            |l: Loop| {
                Ok(Loop {
                    face: to(&self.faces, l.face)?,
                    next: None,
                    first_fin: None,
                })
            },
        )?;
        out.loops.extend(&loops)?;
        let fins = moved(
            &self.fins,
            meter,
            |ix| a.fins.get(ix),
            |f: Fin| {
                Ok(Fin {
                    loop_: to(&self.loops, f.loop_)?,
                    next: to(&self.fins, f.next)?,
                    prev: to(&self.fins, f.prev)?,
                    edge: to(&self.edges, f.edge)?,
                    radial: to(&self.fins, f.radial)?,
                    pcurve: to_some(&self.pcurves, f.pcurve)?,
                    ..f
                })
            },
        )?;
        out.fins.extend(&fins)?;
        let edges = moved(
            &self.edges,
            meter,
            |ix| a.edges.get(ix),
            |e: Edge| {
                Ok(Edge {
                    first_fin: None,
                    start: to_some(&self.vertices, e.start)?,
                    end: to_some(&self.vertices, e.end)?,
                    curve: to(&self.curves, e.curve)?,
                })
            },
        )?;
        out.edges.extend(&edges)?;
        let vertices = moved(
            &self.vertices,
            meter,
            |ix| a.vertices.get(ix),
            |v: Vertex| {
                Ok(Vertex {
                    point: to(&self.points, v.point)?,
                })
            },
        )?;
        out.vertices.extend(&vertices)?;
        self.pools(a, out, meter)
    }

    /// `out` with `a`'s geometry pools moved into it.
    fn pools(
        &self,
        a: &Arenas,
        out: Arenas,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Arenas, TopoError> {
        Ok(Arenas {
            surfaces: moved_pool(&a.surfaces, &self.surfaces, meter)?,
            curves: moved_pool(&a.curves, &self.curves, meter)?,
            pcurves: moved_pool(&a.pcurves, &self.pcurves, meter)?,
            points: moved_pool(&a.points, &self.points, meter)?,
            ..out
        })
    }
}

/// `a` with every list rebuilt from the back-links, in row order, so in name order: each region's
/// shells, each shell's faces and each face's loops chained from the least row up, and each loop's
/// and each edge's first fin its least fin. What the lists held before is replaced.
fn rechain(mut a: Arenas) -> Result<Arenas, TopoError> {
    let (first_shell, shell_next) = chain(a.shells.region.iter().map(|r| r.row()), a.regions.len());
    let (first_face, face_next) = chain(a.faces.shell.iter().map(|s| s.row()), a.shells.len());
    let (first_loop, loop_next) = chain(a.loops.face.iter().map(|f| f.row()), a.faces.len());
    let (loop_fin, _) = chain(a.fins.loop_.iter().map(|l| l.row()), a.loops.len());
    let (edge_fin, _) = chain(a.fins.edge.iter().map(|e| e.row()), a.edges.len());
    a.regions.first_shell = column(first_shell)?;
    a.shells.next = column(shell_next)?;
    a.shells.first_face = column(first_face)?;
    a.faces.next = column(face_next)?;
    a.faces.first_loop = column(first_loop)?;
    a.loops.next = column(loop_next)?;
    a.loops.first_fin = column(loop_fin)?;
    a.edges.first_fin = column(edge_fin)?;
    Ok(a)
}

/// For children whose parents are `parents`, in child row order: each of `count` parents' least
/// child, and each child's next sibling in row order.
fn chain(parents: impl Iterator<Item = u32>, count: u32) -> (Vec<Option<u32>>, Vec<Option<u32>>) {
    let parents: Vec<u32> = parents.collect();
    let mut first: Vec<Option<u32>> = (0..count).map(|_| None).collect();
    let mut next: Vec<Option<u32>> = Vec::with_capacity(parents.len());
    let children = u32::try_from(parents.len()).unwrap_or(u32::MAX);
    for (child, &parent) in (0..children).zip(&parents).rev() {
        // Every parent is a row of its arena, which `inside` checked.
        let head = usize::try_from(parent).ok().and_then(|p| first.get_mut(p));
        next.push(head.and_then(|h| h.replace(child)));
    }
    next.reverse();
    (first, next)
}

/// A link column holding `rows`.
fn column<K: Kind, T>(rows: Vec<Option<u32>>) -> Result<Column<K, Option<Ix<T>>>, TopoError> {
    let mut out = Column::new();
    out.extend(rows.into_iter().map(|r| r.and_then(Ix::new)))?;
    Ok(out)
}
