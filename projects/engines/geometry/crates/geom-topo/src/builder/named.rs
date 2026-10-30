//! Publishing in name order (piece G81): a body's arena indices assigned in persistent-name order,
//! so its arenas depend only on what it contains, never on how it was built (8.2, 32.17).
//!
//! What it does: [`Builder::publish_named`] takes one name per region, face, edge and vertex,
//! checks the arenas as [`Builder::publish`] does, moves every named arena's rows into name order,
//! places the arenas without names (shells, loops, fins) and the geometry pools by a fixed rule
//! from those names, rewrites every link through the moves, and publishes the result, carrying its
//! names in name order.
//!
//! The fixed rule, one key per row, compared in the order listed (8.2: they "follow a fixed rule
//! derived from the names of the faces, edges and vertices they join"):
//!
//! - **Shell:** its region's place, then the least place among its faces.
//! - **Loop:** its face's place, then the least place among its fins' edges.
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
//! - **Checked as published.** The arenas pass [`Builder::publish`]'s checks before anything is
//!   read, and the moved arenas pass them again as they are published.
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
//! row once into new chunks. Nothing recurses.

use alloc::vec::Vec;

use geom_run::WorkMeter;

use super::{Builder, even_columns, inside};
use crate::arenas::{Arenas, Pool};
use crate::body::{Body, BodyKind};
use crate::column::CHUNK_ROWS;
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
        let arenas = moves.arenas(a, meter)?;
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
fn least(owners: u32, pairs: impl Iterator<Item = (Option<u32>, Option<u32>)>) -> Vec<Option<u32>> {
    let mut best: Vec<Option<u32>> = (0..owners).map(|_| None).collect();
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

/// The new place of `ix` (every link was checked to point inside its arena).
fn to<K: Kind>(moves: &Permutation, ix: Ix<K>) -> Result<Ix<K>, TopoError> {
    moves
        .new_of(ix.row())
        .and_then(Ix::new)
        .ok_or(TopoError::NoSuchRow {
            arena: K::ARENA,
            row: ix.row(),
        })
}

/// The new place of `ix`, if there is one.
fn to_some<K: Kind>(moves: &Permutation, ix: Option<Ix<K>>) -> Result<Option<Ix<K>>, TopoError> {
    ix.map(|i| to(moves, i)).transpose()
}

/// The rows of an arena moved by `moves`: row `new` is `get` of the row that moves there,
/// rewritten by `link`, charging `meter` one unit as each chunk begins.
fn moved<K: Kind, R>(
    moves: &Permutation,
    meter: &mut WorkMeter<'_>,
    get: impl Fn(Ix<K>) -> Option<R>,
    mut link: impl FnMut(R) -> Result<R, TopoError>,
) -> Result<Vec<R>, TopoError> {
    (0..moves.len())
        .map(|new| {
            if new % CHUNK_ROWS == 0 {
                meter.charge(1)?;
            }
            let old = moves.old_of(new).unwrap_or(new);
            let row = Ix::new(old).and_then(&get).ok_or(TopoError::NoSuchRow {
                arena: K::ARENA,
                row: old,
            })?;
            link(row)
        })
        .collect()
}

/// A geometry pool with its rows moved by `moves`.
fn moved_pool<T: Kind + Clone>(
    pool: &Pool<T>,
    moves: &Permutation,
    meter: &mut WorkMeter<'_>,
) -> Result<Pool<T>, TopoError> {
    let rows = moved(moves, meter, |ix| pool.get(ix).cloned(), Ok)?;
    let mut out = Pool::new();
    out.extend(rows)?;
    Ok(out)
}

/// Every arena's move.
struct Moves {
    regions: Permutation,
    shells: Permutation,
    faces: Permutation,
    loops: Permutation,
    fins: Permutation,
    edges: Permutation,
    vertices: Permutation,
    surfaces: Permutation,
    curves: Permutation,
    pcurves: Permutation,
    points: Permutation,
}

impl Moves {
    /// Every arena's move into name order, from `names` and the keys of the module's rule.
    fn new(a: &Arenas, names: &Names) -> Result<Moves, TopoError> {
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
            a.fins
                .loop_
                .iter()
                .map(|l| Some(l.row()))
                .zip(a.fins.edge.iter().map(|e| edges.new_of(e.row()))),
        );
        let loop_keys: Vec<(Option<u32>, Option<u32>)> = a
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

    /// `a` moved: every row at its new place, every link pointing at its target's new place.
    fn arenas(&self, a: &Arenas, meter: &mut WorkMeter<'_>) -> Result<Arenas, TopoError> {
        let mut out = Arenas::new();
        let regions = moved(
            &self.regions,
            meter,
            |ix| a.regions.get(ix),
            |r: Region| {
                Ok(Region {
                    first_shell: to_some(&self.shells, r.first_shell)?,
                    ..r
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
                    next: to_some(&self.shells, s.next)?,
                    first_face: to_some(&self.faces, s.first_face)?,
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
                    next: to_some(&self.faces, f.next)?,
                    surface: to(&self.surfaces, f.surface)?,
                    first_loop: to_some(&self.loops, f.first_loop)?,
                    ..f
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
                    next: to_some(&self.loops, l.next)?,
                    first_fin: to_some(&self.fins, l.first_fin)?,
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
                    first_fin: to_some(&self.fins, e.first_fin)?,
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
