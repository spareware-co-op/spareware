//! Names and history of a boolean's result (11.9).
//!
//! What it does: [`name`] gives every region, face, edge and vertex of a merged result its
//! persistent name and its history record, from the names of the faces it came from. A target's
//! face that passes through keeps its name; a tool's is `Created { op, role: tool(key), from }`;
//! several kept pieces of one face are `Split { of, piece: ByTools }`; flush faces become
//! `Merged { of }`. An edge or vertex that was already there keeps its name and a new one is
//! `Bounded` by the faces around it. Names two operands both bring are qualified by the operand's
//! key, and those that would still collide take a weak ordinal.
//!
//! How it runs: [`Sources`] says which input face each polygon came from, so a face's sources are
//! the sources of its pieces. Edges and vertices are found again by their faces and their exact
//! points.
//!
//! Guarantees:
//!
//! - **Ordered by name, never by address, hash order or thread timing.** Every list follows the
//!   operands' keys and the names; the rows of a result are the shape's own (11.9, 11.3).
//! - **An unchanged entity keeps its name, and the history says so** (28.3).
//! - **A qualified face is Generated.** Its name changes, so its history is never Unchanged or
//!   Modified: it is generated from its source, which is listed as deleted (11.9).
//! - **Every input is accounted for.** It has descendants or it is listed as deleted (20.3).
//!
//! Method: chapter 11.9 (`projects/engines/geometry/architecture/11-booleans.md`) on chapter 20's
//! names and histories.
//!
//! Cost: `O(n log n)` in the entities of the result and the operands. Charges the meter nothing
//! of its own: the rows it writes are charged by the publication.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

use geom_name::{OrderError, WeakOrder, weak_names};
use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, P3, Sign};
use geom_op::{
    EdgeOrVertex, History, HistoryError, NameError, NameId, NameList, NameSet, OpPath, Origin,
    PieceKey, Record, Role, RoleArg, Side, TopoName,
};
use geom_poly::merge::{Fin as Fin_, Merged, Outward};
use geom_poly::plane::canonical;
use geom_topo::order::{NameKey, Names};
use geom_topo::{Arenas, Body, Edge, Face, Fin, Ix, PointKind, Vertex};

use crate::operand::OperandKey;

/// One operand, for naming.
#[derive(Clone, Copy, Debug)]
pub struct Named<'b> {
    /// Its key.
    pub key: OperandKey,
    /// Its body.
    pub body: &'b Body,
    /// Its entities' names.
    pub names: &'b Names,
    /// Whether its faces keep their names; if not it is a tool here.
    pub keeps: bool,
}

/// Where polygons came from.
#[derive(Clone, Copy, Debug)]
pub struct Sources<'a> {
    /// The operands, in key order.
    pub operands: &'a [Named<'a>],
    /// Each polygon's operand, a place in `operands`.
    pub owners: &'a [usize],
    /// Each polygon's face, a row of its operand's.
    pub rows: &'a [u32],
    /// Each polygon's plane, facing as its face does.
    pub planes: &'a [Plane],
    /// The input faces that other results of this selection hold pieces of.
    pub split: &'a BTreeSet<(usize, u32)>,
}

/// A result's names and history.
#[derive(Clone, Debug)]
pub struct Naming {
    /// The names, by row of the arenas [`Merged`] writes.
    pub names: Names,
    /// The names this call made that the result's names reach, in the order it made them, each
    /// after those it refers to; with the names the operands brought, they are what a name graph
    /// (`geom_name`) needs to resolve a reference to the result (20.4).
    pub made: Vec<TopoName>,
    /// The history.
    pub history: History,
    /// Whether names two operands brought were qualified by their keys.
    pub qualified: bool,
}

/// Why a result could not be named: a defect, or a name the scheme refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamingError {
    /// A row past the end of a list.
    Broken,
    /// The numbers layer refused.
    Num(NumError),
    /// A name could not be formed.
    Name(NameError),
    /// A history could not be formed.
    History(HistoryError),
    /// Two entities tied under 20.2's order.
    Order(OrderError),
}

impl fmt::Display for NamingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamingError::Broken => f.write_str("naming lost a row"),
            NamingError::Num(e) => write!(f, "naming met {e:?}"),
            NamingError::Name(e) => write!(f, "a name could not be formed: {e:?}"),
            NamingError::History(e) => write!(f, "a history could not be formed: {e:?}"),
            NamingError::Order(e) => write!(f, "names could not be ordered: {e:?}"),
        }
    }
}

impl core::error::Error for NamingError {}

impl From<NumError> for NamingError {
    fn from(e: NumError) -> Self {
        NamingError::Num(e)
    }
}

impl From<NameError> for NamingError {
    fn from(e: NameError) -> Self {
        NamingError::Name(e)
    }
}

impl From<HistoryError> for NamingError {
    fn from(e: HistoryError) -> Self {
        NamingError::History(e)
    }
}

impl From<OrderError> for NamingError {
    fn from(e: OrderError) -> Self {
        NamingError::Order(e)
    }
}

/// `key`'s name.
const fn id(key: NameKey) -> NameId {
    NameId::from_bytes(key.0)
}

/// `name`'s key.
const fn key_of(name: NameId) -> NameKey {
    NameKey(name.to_bytes())
}

/// The names one call makes, in the order it takes their ids: a name's id is taken from its
/// parents' ids, so each comes after those it refers to.
#[derive(Default)]
struct Mint {
    names: Vec<TopoName>,
    seen: BTreeSet<NameId>,
}

impl Mint {
    /// `name`'s id, `name` kept the first time it is made.
    fn id(&mut self, name: TopoName) -> Result<NameId, NameError> {
        let id = name.id()?;
        if self.seen.insert(id) {
            self.names.push(name);
        }
        Ok(id)
    }

    /// The names kept that `outputs` reach through names this call made, in the order made: a name
    /// made and then replaced (a colliding name told apart, a flush face qualified then merged)
    /// stays only if a kept name refers to it.
    fn reached(
        self,
        outputs: impl IntoIterator<Item = NameId>,
    ) -> Result<Vec<TopoName>, NameError> {
        let mut by_id = BTreeMap::new();
        for name in &self.names {
            by_id.insert(name.id()?, name);
        }
        let mut needed = BTreeSet::new();
        let mut stack: Vec<NameId> = outputs.into_iter().collect();
        while let Some(id) = stack.pop() {
            if let Some(name) = by_id.get(&id)
                && needed.insert(id)
            {
                stack.extend(geom_name::graph::parents(name));
            }
        }
        let mut out = Vec::with_capacity(needed.len());
        for name in self.names {
            if needed.contains(&name.id()?) {
                out.push(name);
            }
        }
        Ok(out)
    }
}

/// The role `name` of the operand with `key`.
fn keyed_role(name: &str, key: OperandKey) -> Result<Role, NameError> {
    Role::new(name, [RoleArg::Name(NameId::from_bytes(key.0))])
}

/// A rank for 20.2's order: an entity's vertex numbers, the lowest first, which follow the exact
/// lexicographic order of the points.
#[derive(Clone, Debug)]
struct Rank(Vec<usize>);

impl WeakOrder for Rank {
    fn least_point(&self, other: &Self) -> Option<Ordering> {
        Some(self.0.first()?.cmp(other.0.first()?))
    }

    fn vertices(&self, other: &Self) -> Option<Ordering> {
        Some(self.0.cmp(&other.0))
    }

    fn direction(&self, _: &Self) -> Option<Ordering> {
        Some(Ordering::Equal)
    }
}

/// `names`, every name that several rows share replaced by its weak names. A weak name can itself
/// be taken: a body comes back in holding the weak names of an earlier call, so this goes on until
/// no two rows share one.
fn told_apart(
    names: &[NameId],
    ranks: &[Rank],
    mint: &mut Mint,
) -> Result<Vec<NameId>, NamingError> {
    let mut out = names.to_vec();
    loop {
        let mut rows: BTreeMap<NameId, Vec<usize>> = BTreeMap::new();
        for (row, name) in out.iter().enumerate() {
            rows.entry(*name).or_default().push(row);
        }
        if rows.len() == out.len() {
            return Ok(out);
        }
        for (name, list) in rows.iter().filter(|(_, list)| list.len() > 1) {
            let mut held = Vec::with_capacity(list.len());
            for row in list {
                held.push(ranks.get(*row).ok_or(NamingError::Broken)?.clone());
            }
            for (row, weak) in list.iter().zip(weak_names(*name, &held)?) {
                let new = mint.id(weak)?;
                out.iter_mut()
                    .skip(*row)
                    .take(1)
                    .for_each(|slot| *slot = new);
            }
        }
    }
}

/// The names more than one of `lists` holds: each list is one operand's names of one arena.
fn shared_names<'a>(lists: impl Iterator<Item = &'a [NameKey]>) -> BTreeSet<NameId> {
    let mut seen = BTreeSet::new();
    let mut shared = BTreeSet::new();
    for list in lists {
        let own: BTreeSet<NameId> = list.iter().map(|k| NameId::from_bytes(k.0)).collect();
        for name in own {
            if !seen.insert(name) {
                shared.insert(name);
            }
        }
    }
    shared
}

/// An input edge a result's edge carries on: its operand and row, and whether its ends are the
/// input's own.
type EdgeSource = ((usize, u32), bool);

/// What is known of one input operand's edges and vertices.
struct Input<'a> {
    arenas: &'a Arenas,
    /// Each pair of face rows, its edges.
    between: BTreeMap<(u32, u32), Vec<u32>>,
    /// Each grid point near a vertex, its vertices.
    near: BTreeMap<(i64, i64, i64), Vec<u32>>,
}

/// The row of `ix`.
fn row_of<K>(ix: Ix<K>) -> u32 {
    ix.row()
}

/// The two ends of the edge at `row`.
fn ends(a: &Arenas, row: u32) -> Option<(Ix<Vertex>, Ix<Vertex>)> {
    let edge = a.edges.get(Ix::<Edge>::new(row)?)?;
    edge.start.zip(edge.end)
}

/// The point at `v`, as a meeting of planes or a grid point.
enum At {
    Grid(P3),
    Meet(Tpi),
}

/// Where `v` of `a` is.
#[allow(
    clippy::many_single_char_names,
    reason = "a is the arenas, v the vertex, g its grid point, and p, q and r its planes"
)]
fn at_vertex(a: &Arenas, v: Ix<Vertex>) -> Result<Option<At>, NumError> {
    let point = a
        .vertices
        .get(v)
        .and_then(|row| a.points.get(row.point).copied());
    Ok(match point {
        Some(PointKind::Grid(g)) => Some(At::Grid(g)),
        Some(PointKind::Meet([p, q, r])) => {
            let planes = [a.planes.get(p), a.planes.get(q), a.planes.get(r)];
            match planes {
                [Some(p), Some(q), Some(r)] => Some(At::Meet(Tpi::new(*p, *q, *r)?)),
                _ => None,
            }
        }
        None => None,
    })
}

/// Whether `point` is where `at` is, exactly.
fn same_point(point: &Tpi, at: &At) -> Result<bool, NumError> {
    Ok(match at {
        At::Grid(g) => {
            let near = point.to_grid()?;
            near.is_exact() && near.value() == *g
        }
        At::Meet(other) => {
            let mut all = true;
            for plane in other.planes() {
                all = all && plane.side_of_tpi(point)? == Sign::Zero;
            }
            all
        }
    })
}

/// A vertex's grid key.
fn grid_key(g: P3) -> (i64, i64, i64) {
    (g.x.get(), g.y.get(), g.z.get())
}

/// `a`, indexed.
fn input(a: &Arenas) -> Result<Input<'_>, NumError> {
    let mut between: BTreeMap<(u32, u32), Vec<u32>> = BTreeMap::new();
    for row in 0..a.edges.len() {
        let mut faces = BTreeSet::new();
        let first = Ix::<Edge>::new(row)
            .and_then(|ix| a.edges.get(ix))
            .and_then(|edge| edge.first_fin);
        let mut at = first;
        while let Some(fin) = at {
            let held: Option<Fin> = a.fins.get(fin);
            let face = held.and_then(|f| a.loops.get(f.loop_)).map(|l| l.face);
            faces.extend(face.map(row_of));
            at = held.map(|f| f.radial).filter(|next| Some(*next) != first);
        }
        let mut list = faces.into_iter();
        if let (Some(p), Some(q)) = (list.next(), list.next()) {
            between.entry((p, q)).or_default().push(row);
        }
    }
    let mut near: BTreeMap<(i64, i64, i64), Vec<u32>> = BTreeMap::new();
    for row in 0..a.vertices.len() {
        let place = match Ix::<Vertex>::new(row) {
            Some(ix) => at_vertex(a, ix)?,
            None => None,
        };
        let key = match place {
            Some(At::Grid(g)) => Some(grid_key(g)),
            Some(At::Meet(t)) => Some(grid_key(t.to_grid()?.value())),
            None => None,
        };
        if let Some(key) = key {
            near.entry(key).or_default().push(row);
        }
    }
    Ok(Input {
        arenas: a,
        between,
        near,
    })
}

/// The input vertex of `one` exactly at `point`.
fn found_vertex(one: &Input<'_>, point: &Tpi) -> Result<Option<u32>, NumError> {
    let key = grid_key(point.to_grid()?.value());
    for row in one.near.get(&key).into_iter().flatten() {
        let place = match Ix::<Vertex>::new(*row) {
            Some(ix) => at_vertex(one.arenas, ix)?,
            None => None,
        };
        if let Some(at) = place
            && same_point(point, &at)?
        {
            return Ok(Some(*row));
        }
    }
    Ok(None)
}

/// A name, or a defect.
fn name_at(list: &[NameKey], row: u32) -> Result<NameId, NamingError> {
    let at = usize::try_from(row).map_err(|_| NamingError::Broken)?;
    list.get(at).map(|key| id(*key)).ok_or(NamingError::Broken)
}

/// The faces of a result, named, with what is known of each.
struct FacesDone {
    names: Vec<NameId>,
    records: Vec<Record>,
    sources: Vec<Vec<(usize, u32)>>,
    inputs: Vec<Vec<NameId>>,
    deleted: BTreeSet<NameId>,
    qualified: bool,
}

/// Each plane any input face lies on: the operand, the face's name and which way it faces.
type OnPlanes = BTreeMap<(i128, [i128; 3]), Vec<(usize, NameId, bool)>>;

/// The input faces on each plane.
fn on_planes(sources: &Sources<'_>) -> Result<OnPlanes, NamingError> {
    let mut out: OnPlanes = BTreeMap::new();
    let polygons = sources.planes.iter().zip(sources.owners).zip(sources.rows);
    for ((plane, owner), row) in polygons {
        let operand = sources.operands.get(*owner).ok_or(NamingError::Broken)?;
        let held = canonical(plane)?;
        let name = name_at(&operand.names.faces, *row)?;
        let list = out.entry((held.plane.d(), held.plane.n())).or_default();
        list.push((*owner, name, held.flipped));
    }
    Ok(out)
}

/// The fins of `face`.
fn fins_of(merged: &Merged, face: usize) -> impl Iterator<Item = &Fin_> {
    let rings = merged.faces.get(face).map(|held| &held.rings);
    rings
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|fin| merged.fins.get(*fin))
}

/// The tools that bound `face`: the input faces of other operands on the planes its fins lie on,
/// and which side of the lowest it lies on.
fn tools_of(
    merged: &Merged,
    face: usize,
    owner: usize,
    planes: &OnPlanes,
) -> Result<(Vec<NameId>, Side), NamingError> {
    let mut found: BTreeMap<NameId, Side> = BTreeMap::new();
    for fin in fins_of(merged, face) {
        let held = canonical(&fin.bound)?;
        let on = planes.get(&(held.plane.d(), held.plane.n()));
        for (other, name, flipped) in on.into_iter().flatten() {
            if *other != owner {
                let side = if *flipped == held.flipped {
                    Side::Front
                } else {
                    Side::Back
                };
                found.insert(*name, side);
            }
        }
    }
    let side = found.values().next().copied().unwrap_or(Side::Front);
    Ok((found.into_keys().collect(), side))
}

/// The name and origin of a face with several sources, `names` by source of `list`, and the inputs
/// that qualifying them deleted.
fn merged_face(
    op: &OpPath,
    list: &[(usize, u32)],
    names: &[NameId],
    sources: &Sources<'_>,
    mint: &mut Mint,
) -> Result<(NameId, Origin, BTreeSet<NameId>), NamingError> {
    let inputs: BTreeSet<NameId> = names.iter().copied().collect();
    if inputs.len() > 1 {
        let of = NameSet::new(inputs)?;
        let name = TopoName::Merged {
            op: op.clone(),
            of: of.clone(),
        };
        return Ok((mint.id(name)?, Origin::Merged { of }, BTreeSet::new()));
    }
    // Flush faces of operands that brought one name: each qualified, then merged.
    let mut qualified = BTreeSet::new();
    for (owner, _) in list {
        let operand = sources.operands.get(*owner).ok_or(NamingError::Broken)?;
        let name = TopoName::Created {
            op: op.clone(),
            role: keyed_role("operand", operand.key)?,
            from: NameList::new(inputs.iter().copied()),
        };
        qualified.insert(mint.id(name)?);
    }
    let name = TopoName::Merged {
        op: op.clone(),
        of: NameSet::new(qualified)?,
    };
    let origin = Origin::Generated {
        from: NameSet::new(inputs.iter().copied())?,
        role: Role::new("merged", [])?,
    };
    Ok((mint.id(name)?, origin, inputs))
}

/// Names every face of `merged`, whose `pieces` are as given.
fn name_faces(
    op: &OpPath,
    merged: &Merged,
    pieces: &[Outward],
    sources: &Sources<'_>,
    mint: &mut Mint,
) -> Result<FacesDone, NamingError> {
    let mut of_face: Vec<BTreeSet<(usize, u32)>> = alloc::vec![BTreeSet::new(); merged.faces.len()];
    for (piece, face) in pieces.iter().zip(&merged.piece_face) {
        let owner = sources.owners.get(piece.polygon);
        let row = sources.rows.get(piece.polygon);
        let (owner, row) = owner.zip(row).ok_or(NamingError::Broken)?;
        of_face.iter_mut().skip(*face).take(1).for_each(|set| {
            set.insert((*owner, *row));
        });
    }
    let mut pieces_of: BTreeMap<(usize, u32), usize> = BTreeMap::new();
    for set in &of_face {
        for source in set {
            let slot = pieces_of.entry(*source).or_insert(0);
            *slot = slot.saturating_add(1);
        }
    }
    // Names two operands bring are qualified, each by its operand's key.
    let planes = on_planes(sources)?;
    let mut bringers: BTreeMap<NameId, BTreeSet<usize>> = BTreeMap::new();
    for (owner, row) in pieces_of.keys() {
        let operand = sources.operands.get(*owner).ok_or(NamingError::Broken)?;
        let name = name_at(&operand.names.faces, *row)?;
        bringers.entry(name).or_default().insert(*owner);
    }
    let mut done = FacesDone {
        names: Vec::with_capacity(merged.faces.len()),
        records: Vec::with_capacity(merged.faces.len()),
        sources: Vec::with_capacity(merged.faces.len()),
        inputs: Vec::with_capacity(merged.faces.len()),
        deleted: BTreeSet::new(),
        qualified: false,
    };
    for (face, set) in of_face.iter().enumerate() {
        let list: Vec<(usize, u32)> = set.iter().copied().collect();
        let mut names = Vec::with_capacity(list.len());
        for (owner, row) in &list {
            let operand = sources.operands.get(*owner).ok_or(NamingError::Broken)?;
            names.push(name_at(&operand.names.faces, *row)?);
        }
        let (name, origin) = if let ([(owner, row)], [own]) = (list.as_slice(), names.as_slice()) {
            {
                let operand = sources.operands.get(*owner).ok_or(NamingError::Broken)?;
                let shared = bringers.get(own).is_some_and(|set| set.len() > 1);
                let splits = pieces_of.get(&(*owner, *row)).copied().unwrap_or(1) > 1
                    || sources.split.contains(&(*owner, *row));
                if !operand.keeps || shared {
                    let word = if operand.keeps { "operand" } else { "tool" };
                    let role = keyed_role(word, operand.key)?;
                    let name = TopoName::Created {
                        op: op.clone(),
                        role: role.clone(),
                        from: NameList::new([*own]),
                    };
                    done.qualified = done.qualified || shared;
                    done.deleted.insert(*own);
                    (
                        mint.id(name)?,
                        Origin::Generated {
                            from: NameSet::new([*own])?,
                            role,
                        },
                    )
                } else if splits {
                    let (tools, side) = tools_of(merged, face, *owner, &planes)?;
                    let piece = PieceKey::ByTools {
                        tools: NameSet::new(tools)?,
                        side,
                    };
                    let name = TopoName::Split {
                        op: op.clone(),
                        of: *own,
                        piece: piece.clone(),
                    };
                    (mint.id(name)?, Origin::Split { of: *own, piece })
                } else {
                    (
                        *own,
                        Origin::Modified {
                            from: NameSet::new([*own])?,
                        },
                    )
                }
            }
        } else {
            let (name, origin, gone) = merged_face(op, &list, &names, sources, mint)?;
            done.qualified = done.qualified || !gone.is_empty();
            done.deleted.extend(gone);
            (name, origin)
        };
        done.names.push(name);
        done.records.push(Record {
            output: name,
            origin,
        });
        done.sources.push(list);
        done.inputs.push(names);
    }
    Ok(done)
}

/// A face's rank: its vertex numbers, the lowest first.
fn face_ranks(merged: &Merged) -> Vec<Rank> {
    (0..merged.faces.len())
        .map(|at| {
            let mut list: Vec<usize> = fins_of(merged, at).map(|fin| fin.from).collect();
            list.sort_unstable();
            Rank(list)
        })
        .collect()
}

/// Whether `face` is the input face it came from, its boundary untouched: as many fins, and every
/// fin starting at one of the input face's own vertices.
fn untouched(
    merged: &Merged,
    face: usize,
    source: (usize, u32),
    inputs: &[Input<'_>],
) -> Result<bool, NumError> {
    let (owner, row) = source;
    let Some(one) = inputs.get(owner) else {
        return Ok(false);
    };
    let a = one.arenas;
    let mut corners = BTreeSet::new();
    let mut at_loop = Ix::<Face>::new(row)
        .and_then(|ix| a.faces.get(ix))
        .and_then(|f| f.first_loop);
    while let Some(ix) = at_loop {
        let held = a.loops.get(ix);
        let start = held.and_then(|l| l.first_fin);
        let mut at = start;
        while let Some(fin) = at {
            let this = a.fins.get(fin);
            let way = this.and_then(|f| {
                let edge = a.edges.get(f.edge)?;
                match f.sense {
                    geom_topo::Sense::Forward => edge.start,
                    geom_topo::Sense::Reversed => edge.end,
                }
            });
            corners.extend(way.map(row_of));
            at = this.map(|f| f.next).filter(|next| Some(*next) != start);
        }
        at_loop = held.and_then(|l| l.next);
    }
    let fins: Vec<usize> = fins_of(merged, face).map(|fin| fin.from).collect();
    if fins.len() != corners.len() {
        return Ok(false);
    }
    for from in fins {
        let Some(point) = merged.vertices.get(from) else {
            return Ok(false);
        };
        match found_vertex(one, point)? {
            Some(row) if corners.contains(&row) => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}

/// One arena's names before ordinals, each with its origin and rank.
#[derive(Default)]
struct Rows {
    names: Vec<NameId>,
    origins: Vec<Origin>,
    ranks: Vec<Rank>,
}

impl Rows {
    /// The names told apart, with their records added to `records`.
    fn finished(
        self,
        records: &mut Vec<Record>,
        mint: &mut Mint,
    ) -> Result<Vec<NameId>, NamingError> {
        let names = told_apart(&self.names, &self.ranks, mint)?;
        for (name, origin) in names.iter().zip(self.origins) {
            records.push(Record {
                output: *name,
                origin,
            });
        }
        Ok(names)
    }
}

/// The input edge each edge of `merged` carries on, if it lies between two faces of one operand:
/// its operand and row, and whether its ends are the input's own.
fn edge_sources(
    merged: &Merged,
    faces: &FacesDone,
    inputs: &[Input<'_>],
) -> Result<Vec<Option<EdgeSource>>, NamingError> {
    let mut found = Vec::with_capacity(merged.edges.len());
    for edge in &merged.edges {
        let mut sides = BTreeSet::new();
        for fin in &edge.fins {
            let held = merged.fins.get(*fin).ok_or(NamingError::Broken)?;
            let list = faces.sources.get(held.face).ok_or(NamingError::Broken)?;
            if let [one] = list.as_slice() {
                sides.insert(*one);
            }
        }
        let mut hit = None;
        let held: Vec<(usize, u32)> = sides.into_iter().collect();
        if let [(owner, a), (other, b)] = held.as_slice()
            && owner == other
            && let Some(one) = inputs.get(*owner)
            && let Some(list) = one.between.get(&((*a).min(*b), (*a).max(*b)))
        {
            let low = merged.vertices.get(edge.low).ok_or(NamingError::Broken)?;
            let high = merged.vertices.get(edge.high).ok_or(NamingError::Broken)?;
            let ends_at = found_vertex(one, low)?.zip(found_vertex(one, high)?);
            for row in list {
                let same = ends(one.arenas, *row).is_some_and(|(s, e)| {
                    let (s, e) = (s.row(), e.row());
                    ends_at == Some((s, e)) || ends_at == Some((e, s))
                });
                if same {
                    hit = Some(((*owner, *row), true));
                }
            }
            if let (None, [row]) = (hit, list.as_slice()) {
                hit = Some(((*owner, *row), false));
            }
        }
        found.push(hit);
    }
    Ok(found)
}

/// Every edge of `merged`, kept or `Bounded`.
fn name_edges(
    merged: &Merged,
    faces: &FacesDone,
    inputs: &[Input<'_>],
    sources: &Sources<'_>,
    alive: &mut BTreeSet<NameId>,
    mint: &mut Mint,
) -> Result<Rows, NamingError> {
    let shared = shared_names(sources.operands.iter().map(|o| o.names.edges.as_slice()));
    let hits = edge_sources(merged, faces, inputs)?;
    let mut users: BTreeMap<(usize, u32), usize> = BTreeMap::new();
    for (source, _) in hits.iter().flatten() {
        let slot = users.entry(*source).or_insert(0);
        *slot = slot.saturating_add(1);
    }
    let of_row = |at: usize| faces.names.get(at).copied().ok_or(NamingError::Broken);
    let mut touching: Vec<BTreeSet<NameId>> = alloc::vec![BTreeSet::new(); merged.vertices.len()];
    for fin in &merged.fins {
        let held = of_row(fin.face)?;
        for at in [fin.from, fin.to] {
            touching.iter_mut().skip(at).take(1).for_each(|set| {
                set.insert(held);
            });
        }
    }
    let mut rows = Rows::default();
    for (edge, hit) in merged.edges.iter().zip(&hits) {
        let mut around = BTreeSet::new();
        let mut made_from = BTreeSet::new();
        for fin in &edge.fins {
            let held = merged.fins.get(*fin).ok_or(NamingError::Broken)?;
            around.insert(of_row(held.face)?);
            let names = faces.inputs.get(held.face).ok_or(NamingError::Broken)?;
            made_from.extend(names.iter().copied());
        }
        let by = NameSet::new(around.iter().copied())?;
        // Only a kept operand's edge keeps its name, and not a name two operands brought: the
        // rule is per name, so the wall's own edges keep theirs beside two doors of one type.
        let kept = |owner: usize, row: u32| {
            sources.operands.get(owner).is_some_and(|one| {
                one.keeps && name_at(&one.names.edges, row).is_ok_and(|n| !shared.contains(&n))
            })
        };
        let keeps = hit
            .filter(|((owner, row), _)| kept(*owner, *row))
            .filter(|(source, _)| users.get(source) == Some(&1));
        let (name, origin) = if let Some(((owner, row), exact)) = keeps {
            let operand = sources.operands.get(owner).ok_or(NamingError::Broken)?;
            let own = name_at(&operand.names.edges, row)?;
            alive.insert(own);
            let origin = if exact {
                Origin::Unchanged
            } else {
                Origin::Modified {
                    from: NameSet::new([own])?,
                }
            };
            (own, origin)
        } else {
            let at_ends = [edge.low, edge.high]
                .into_iter()
                .filter_map(|v| touching.get(v));
            let ending = at_ends.flatten().filter(|held| !around.contains(*held));
            let name = TopoName::Bounded {
                kind: EdgeOrVertex::Edge,
                by,
                ends: NameSet::new(ending.copied().collect::<BTreeSet<NameId>>())?,
            };
            let origin = Origin::Generated {
                from: NameSet::new(made_from)?,
                role: Role::new("edge", [])?,
            };
            (mint.id(name)?, origin)
        };
        rows.names.push(name);
        rows.origins.push(origin);
        rows.ranks.push(Rank(alloc::vec![edge.low, edge.high]));
    }
    Ok(rows)
}

/// Every vertex of `merged`: a kept operand's name where it had one at the same point, or
/// `Bounded` by the faces around it.
fn name_vertices(
    merged: &Merged,
    faces: &FacesDone,
    inputs: &[Input<'_>],
    sources: &Sources<'_>,
    alive: &mut BTreeSet<NameId>,
    mint: &mut Mint,
) -> Result<Rows, NamingError> {
    let shared = shared_names(sources.operands.iter().map(|o| o.names.vertices.as_slice()));
    let mut around: Vec<BTreeSet<NameId>> = alloc::vec![BTreeSet::new(); merged.vertices.len()];
    let mut made_from = around.clone();
    for fin in &merged.fins {
        let held = faces
            .names
            .get(fin.face)
            .copied()
            .ok_or(NamingError::Broken)?;
        let names = faces.inputs.get(fin.face).ok_or(NamingError::Broken)?;
        for at in [fin.from, fin.to] {
            around.iter_mut().skip(at).take(1).for_each(|set| {
                set.insert(held);
            });
            made_from.iter_mut().skip(at).take(1).for_each(|set| {
                set.extend(names.iter().copied());
            });
        }
    }
    let mut rows = Rows::default();
    let places = merged.vertices.iter().zip(&around).zip(&made_from);
    for (at, ((point, set), from)) in places.enumerate() {
        let mut own = None;
        for (place, one) in inputs.iter().enumerate() {
            let operand = sources.operands.get(place).ok_or(NamingError::Broken)?;
            if own.is_none()
                && operand.keeps
                && let Some(row) = found_vertex(one, point)?
            {
                // Not a name two operands brought (per name, as for edges).
                let name = name_at(&operand.names.vertices, row)?;
                if !shared.contains(&name) {
                    own = Some(name);
                }
            }
        }
        let (name, origin) = if let Some(own) = own {
            alive.insert(own);
            (own, Origin::Unchanged)
        } else {
            let name = TopoName::Bounded {
                kind: EdgeOrVertex::Vertex,
                by: NameSet::new(set.iter().copied())?,
                ends: NameSet::new([])?,
            };
            let origin = Origin::Generated {
                from: NameSet::new(from.iter().copied())?,
                role: Role::new("vertex", [])?,
            };
            (mint.id(name)?, origin)
        };
        rows.names.push(name);
        rows.origins.push(origin);
        rows.ranks.push(Rank(alloc::vec![at]));
    }
    Ok(rows)
}

/// The regions of `merged`: one per outer shell, by its least face name, then the outside.
fn name_regions(
    op: &OpPath,
    merged: &Merged,
    faces: &[NameId],
    records: &mut Vec<Record>,
    mint: &mut Mint,
) -> Result<Vec<NameId>, NamingError> {
    let mut regions = Vec::new();
    let mut add = |role: Role| -> Result<(), NamingError> {
        let name = mint.id(TopoName::Created {
            op: op.clone(),
            role: role.clone(),
            from: NameList::new([]),
        })?;
        regions.push(name);
        records.push(Record {
            output: name,
            origin: Origin::Generated {
                from: NameSet::new([])?,
                role,
            },
        });
        Ok(())
    };
    for shell in merged.shells.iter().filter(|shell| shell.outer) {
        // The least name, which depends on no coordinate: a face's number does.
        let names = shell.faces.iter().filter_map(|face| faces.get(*face));
        let least = names.min().ok_or(NamingError::Broken)?;
        add(Role::new("region.solid", [RoleArg::Name(*least)])?)?;
    }
    add(Role::new("region.outside", [])?)?;
    Ok(regions)
}

/// Every input name with no descendant, or qualified away: the deleted list.
fn deleted_names(
    sources: &Sources<'_>,
    records: &[Record],
    alive: &BTreeSet<NameId>,
    gone: &BTreeSet<NameId>,
) -> Result<NameSet, NamingError> {
    let mut named: BTreeSet<NameId> = BTreeSet::new();
    for operand in sources.operands {
        let lists = [
            &operand.names.regions,
            &operand.names.faces,
            &operand.names.edges,
            &operand.names.vertices,
        ];
        named.extend(
            lists
                .iter()
                .flat_map(|list| list.iter().map(|key| id(*key))),
        );
    }
    let carried: BTreeSet<NameId> = records
        .iter()
        .flat_map(|record| match &record.origin {
            Origin::Modified { from } | Origin::Merged { of: from } => from.names().to_vec(),
            Origin::Split { of, .. } => alloc::vec![*of],
            Origin::Unchanged | Origin::Generated { .. } => Vec::new(),
        })
        .collect();
    let deleted = named
        .into_iter()
        .filter(|name| gone.contains(name) || !(alive.contains(name) || carried.contains(name)));
    Ok(NameSet::new(deleted)?)
}

/// The names and history of `merged`, a result of `op` holding `pieces`.
///
/// # Errors
///
/// A [`NamingError`]: a defect, since every name of a result can be formed.
pub fn name(
    op: &OpPath,
    merged: &Merged,
    pieces: &[Outward],
    sources: &Sources<'_>,
) -> Result<Naming, NamingError> {
    let mut inputs = Vec::with_capacity(sources.operands.len());
    for operand in sources.operands {
        inputs.push(input(operand.body.arenas())?);
    }
    let mut mint = Mint::default();
    let mut faces = name_faces(op, merged, pieces, sources, &mut mint)?;
    faces.names = told_apart(&faces.names, &face_ranks(merged), &mut mint)?;
    for (at, (record, name)) in faces.records.iter_mut().zip(&faces.names).enumerate() {
        record.output = *name;
        if let (Origin::Modified { .. }, Some([source])) =
            (&record.origin, faces.sources.get(at).map(Vec::as_slice))
            && untouched(merged, at, *source, &inputs)?
        {
            record.origin = Origin::Unchanged;
        }
    }
    let mut records = core::mem::take(&mut faces.records);
    let mut alive: BTreeSet<NameId> = records
        .iter()
        .filter(|r| matches!(r.origin, Origin::Unchanged | Origin::Modified { .. }))
        .map(|r| r.output)
        .collect();
    let edges = name_edges(merged, &faces, &inputs, sources, &mut alive, &mut mint)?
        .finished(&mut records, &mut mint)?;
    let vertices = name_vertices(merged, &faces, &inputs, sources, &mut alive, &mut mint)?
        .finished(&mut records, &mut mint)?;
    let regions = name_regions(op, merged, &faces.names, &mut records, &mut mint)?;
    let deleted = deleted_names(sources, &records, &alive, &faces.deleted)?;
    let history = History::new(records, deleted, [])?;
    let names = Names {
        regions: regions.into_iter().map(key_of).collect(),
        faces: faces.names.iter().copied().map(key_of).collect(),
        edges: edges.into_iter().map(key_of).collect(),
        vertices: vertices.into_iter().map(key_of).collect(),
    };
    let made = mint.reached(
        [&names.regions, &names.faces, &names.edges, &names.vertices]
            .into_iter()
            .flatten()
            .map(|key| NameId::from_bytes(key.0)),
    )?;
    Ok(Naming {
        names,
        made,
        history,
        qualified: faces.qualified,
    })
}
