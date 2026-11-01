//! Euler operators (piece G81): edits that change a body's topology one step at a time, each
//! keeping Euler's formula, on an editor whose rows are removed by marking them dead.
//!
//! What it does: an [`Euler`] editor holds arenas, one name per region, face, edge and vertex, and
//! which rows are live. It starts empty ([`Euler::new`]) or from a body published by name
//! ([`Euler::edit`]). The operators come in pairs, each undoing the other:
//!
//! | Make | Kill | Counts it changes (V, E, F, rings R, shells S, holes H) |
//! | --- | --- | --- |
//! | [`Euler::make_wire`] | [`Euler::kill_wire`] | V + 2, E + 1, F + 1, S + 1: a shell of one face whose one loop runs along one edge and back |
//! | [`Euler::mev`] | [`Euler::kev`] | V + 1, E + 1: an edge to a new vertex, out from a loop and back |
//! | [`Euler::mef`] | [`Euler::kef`] | E + 1, F + 1: an edge across a loop, splitting off a new face |
//! | [`Euler::mekr`] | [`Euler::kemr`] | E + 1, R − 1: an edge joining two loops of one face into one |
//! | [`Euler::mfkrh`] | [`Euler::kfmrh`] | F + 1, R − 1, H − 1: a ring made the loop of a new face |
//!
//! Regions and cells: [`Euler::make_region`] and [`Euler::kill_region`] add and remove regions,
//! and [`Euler::move_shell`] puts a shell in another region, as a void shell inside a solid makes a
//! cavity. [`Euler::publish`] drops the dead rows and publishes the rest by name
//! ([`Builder::publish_named`]).
//!
//! Guarantees:
//!
//! - **Euler's formula is kept.** Each operator changes the counts by its row of the table above,
//!   so V − E + F − R = 2 (S − H) holds after it when it held before; `R` is loops less faces, and
//!   `H` is the count the model of `tests/euler_model.rs` keeps. `verus/euler.rs` proves this, and
//!   the next guarantee, on a model of these operators (piece G111); `tests/euler_model.rs` ties
//!   the model to this code.
//! - **Loops stay closed and rings stay paired.** Every operator leaves each loop's fins one cycle
//!   by `next` and `prev`, each the inverse of the other, each fin's start the end of the fin before
//!   it, and each edge it makes with two fins, each the other's radial neighbour.
//! - **A refusal changes nothing.** Every operator checks its arguments before it changes a row;
//!   one it cannot apply is a [`TopoError::Euler`] with the [`Refusal`]. An arena that is full or an
//!   allocation refused part-way leaves the editor part-way, to be dropped.
//! - **Rows only grow while editing.** A killed row is marked dead, so every handle the editor
//!   gave stays the row it named; [`Euler::publish`] drops dead rows and the geometry no live
//!   entity uses, then assigns every index in name order.
//! - **Lists are made when publishing.** The editor keeps no list of a region's shells, a shell's
//!   faces or a face's loops ([`Builder::publish_named`] rebuilds them from the owner links), only
//!   each loop's and each edge's first fin.
//!
//! Bounded proofs, and what is only tested: Kani proves (piece G112, chapter 28.7), with its
//! overflow, bounds and panic checks on, these sequences from a lone wire, every argument a
//! constant: MEV then KEV, MEF then KEF, MEF from a fin to itself, KEMR then MEKR, MFKRH then KFMRH
//! and a wire's removal each keep every rule above and Euler's formula, and three refusals change
//! nothing.
//! A symbolic fin took CBMC past 12 GB, so these are bounded in their arguments as well as their
//! length; the geometry pools, which no operator reads, are stood in for. That the operators keep
//! the rules on every body is Verus's proof on a model (`verus/euler.rs`, piece G111), and
//! `tests/euler_model.rs`'s 100,000 random steps tie the model to this code.
//!
//! Method: Mäntylä and Sulonen's GWB operators on half-edges (MVFS, MEV, MEF, KEMR, KFMRH and their
//! inverses; "GWB: a solid modeler with Euler operators", IEEE Computer Graphics and Applications
//! 2(7), 17-31, 1982, <https://doi.org/10.1109/MCG.1982.1674396>), with fins as half-edges. A fin
//! needs an edge here (8.1), so the first operator makes a wire of one edge and two vertices,
//! Mäntylä's MVFS followed by MEV, and no operator leaves a loop without a fin.
//!
//! Cost: each operator is constant time but for the fins it moves to another loop, which it
//! visits once; [`Euler::publish`] is linear in the rows, then [`Builder::publish_named`]'s cost.
//! Nothing recurses.

use alloc::vec::Vec;
use core::iter::successors;

use geom_curve::CurveKind;
use geom_math::Box3;
use geom_num::grid::P3;
use geom_run::WorkMeter;
use geom_surface::SurfaceKind;

use super::Builder;
use super::named::{Keep, Moves, to};
use crate::arenas::Arenas;
use crate::body::{Body, BodyKind};
use crate::entity::{
    CurveIx, Edge, EdgeIx, Face, FaceIx, Fin, FinIx, Loop, LoopIx, PointKind, Region, RegionIx,
    RegionKind, Sense, Shell, ShellIx, SurfaceIx, Vertex, VertexIx,
};
use crate::error::{Refusal, TopoError};
use crate::ix::{Arena, Ix, Kind};
use crate::order::{NameKey, Names};

/// A face an operator makes: its name, the surface it lies on, its sense and a box holding it.
#[derive(Clone, Debug, PartialEq)]
pub struct NewFace {
    /// Its persistent name.
    pub name: NameKey,
    /// The surface it lies on.
    pub surface: SurfaceKind,
    /// Which side of the surface is material.
    pub sense: Sense,
    /// A box holding it.
    pub bound: Box3,
}

/// An edge an operator makes: its name and the curve it lies on.
#[derive(Clone, Debug, PartialEq)]
pub struct NewEdge {
    /// Its persistent name.
    pub name: NameKey,
    /// The curve it lies on.
    pub curve: CurveKind<3>,
}

/// A vertex an operator makes: its name and its point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewVertex {
    /// Its persistent name.
    pub name: NameKey,
    /// Its point.
    pub point: P3,
}

/// What [`Euler::make_wire`] made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wire {
    /// The new shell.
    pub shell: ShellIx,
    /// Its one face.
    pub face: FaceIx,
    /// The face's one loop.
    pub loop_: LoopIx,
    /// The one edge.
    pub edge: EdgeIx,
    /// The loop's two fins: along the edge, then back.
    pub fins: [FinIx; 2],
    /// The edge's start and end.
    pub vertices: [VertexIx; 2],
}

/// Which rows of each entity arena are live.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Live {
    regions: Vec<bool>,
    shells: Vec<bool>,
    faces: Vec<bool>,
    loops: Vec<bool>,
    fins: Vec<bool>,
    edges: Vec<bool>,
    vertices: Vec<bool>,
}

/// An editor of a body's topology by Euler operators.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Euler {
    /// The rows, live and dead.
    arenas: Arenas,
    /// One name per region, face, edge and vertex row.
    names: Names,
    /// Which rows are live.
    live: Live,
    /// Live shells per region row.
    shells_of_region: Vec<u32>,
    /// Live faces per shell row.
    faces_of_shell: Vec<u32>,
    /// Live loops per face row.
    loops_of_face: Vec<u32>,
}

/// The refusal of a handle to a row that is not live.
fn dead<K: Kind>(ix: Ix<K>) -> TopoError {
    TopoError::Euler(Refusal::Dead {
        arena: K::ARENA,
        row: ix.row(),
    })
}

/// `row`, if `ix`'s flag in `mask` says it is live.
fn alive<K: Kind, R>(mask: &[bool], ix: Ix<K>, row: Option<R>) -> Result<R, TopoError> {
    row.filter(|_| mask.get(ix.to_usize()) == Some(&true))
        .ok_or(dead(ix))
}

/// The index `k` rows past the end of an arena of `len` rows.
fn after<K: Kind>(len: u32, k: u32) -> Result<Ix<K>, TopoError> {
    len.checked_add(k)
        .and_then(Ix::new)
        .ok_or(TopoError::Full(K::ARENA))
}

/// The count at `row` of `counts`.
fn count(counts: &[u32], row: u32) -> u32 {
    usize::try_from(row)
        .ok()
        .and_then(|r| counts.get(r))
        .copied()
        .unwrap_or(0)
}

/// Sets the count at `row` of `counts` to what `f` makes of it.
fn recount(counts: &mut [u32], row: u32, f: impl Fn(u32) -> u32) {
    usize::try_from(row)
        .ok()
        .and_then(|r| counts.get_mut(r))
        .into_iter()
        .for_each(|c| *c = f(*c));
}

/// Marks `row` of `mask` dead.
fn kill(mask: &mut [bool], row: u32) {
    usize::try_from(row)
        .ok()
        .and_then(|r| mask.get_mut(r))
        .into_iter()
        .for_each(|l| *l = false);
}

/// For each of `count` rows, how many of `owners` name it.
fn tally(owners: impl Iterator<Item = u32>, count: u32) -> Vec<u32> {
    let mut counts: Vec<u32> = (0..count).map(|_| 0_u32).collect();
    for owner in owners {
        recount(&mut counts, owner, |c| c.saturating_add(1));
    }
    counts
}

/// Which of `rows` pool rows a live user names: `users` holds each user's row in the pool, if
/// any, and whether the user is live.
fn used(rows: u32, users: impl Iterator<Item = (Option<u32>, bool)>) -> Vec<bool> {
    let mut used: Vec<bool> = (0..rows).map(|_| false).collect();
    for (row, live) in users {
        usize::try_from(row.filter(|_| live).unwrap_or(u32::MAX))
            .ok()
            .and_then(|r| used.get_mut(r))
            .into_iter()
            .for_each(|u| *u = true);
    }
    used
}

impl Euler {
    /// An editor with no rows.
    #[must_use]
    pub fn new() -> Euler {
        Euler::default()
    }

    /// An editor holding `body`, every row live.
    ///
    /// # Errors
    ///
    /// [`Refusal::Unnamed`] if the body was published without names.
    pub fn edit(body: &Body) -> Result<Euler, TopoError> {
        let names = body
            .names()
            .ok_or(TopoError::Euler(Refusal::Unnamed))?
            .clone();
        let a = body.arenas().clone();
        let all = |rows: u32| -> Vec<bool> { (0..rows).map(|_| true).collect() };
        Ok(Euler {
            live: Live {
                regions: all(a.regions.len()),
                shells: all(a.shells.len()),
                faces: all(a.faces.len()),
                loops: all(a.loops.len()),
                fins: all(a.fins.len()),
                edges: all(a.edges.len()),
                vertices: all(a.vertices.len()),
            },
            shells_of_region: tally(a.shells.region.iter().map(|r| r.row()), a.regions.len()),
            faces_of_shell: tally(a.faces.shell.iter().map(|s| s.row()), a.shells.len()),
            loops_of_face: tally(a.loops.face.iter().map(|f| f.row()), a.faces.len()),
            arenas: a,
            names,
        })
    }

    /// The rows, live and dead, to read. The lists of a region's shells, a shell's faces and a
    /// face's loops are not kept while editing; each loop's and each edge's first fin is.
    #[must_use]
    pub const fn arenas(&self) -> &Arenas {
        &self.arenas
    }

    /// One name per region, face, edge and vertex row, live and dead.
    #[must_use]
    pub const fn names(&self) -> &Names {
        &self.names
    }

    /// Whether `row` of the entity arena `arena` is live; a geometry pool's rows never are (they
    /// live while a live entity uses them).
    #[must_use]
    pub fn is_live(&self, arena: Arena, row: u32) -> bool {
        let l = &self.live;
        let mask: &[bool] = match arena {
            Arena::Region => &l.regions,
            Arena::Shell => &l.shells,
            Arena::Face => &l.faces,
            Arena::Loop => &l.loops,
            Arena::Fin => &l.fins,
            Arena::Edge => &l.edges,
            Arena::Vertex => &l.vertices,
            Arena::Surface | Arena::Curve | Arena::Pcurve | Arena::Point | Arena::Plane => &[],
        };
        usize::try_from(row).ok().and_then(|r| mask.get(r)) == Some(&true)
    }

    fn region(&self, r: RegionIx) -> Result<Region, TopoError> {
        alive(&self.live.regions, r, self.arenas.regions.get(r))
    }

    fn shell(&self, s: ShellIx) -> Result<Shell, TopoError> {
        alive(&self.live.shells, s, self.arenas.shells.get(s))
    }

    fn face(&self, f: FaceIx) -> Result<Face, TopoError> {
        alive(&self.live.faces, f, self.arenas.faces.get(f))
    }

    fn loop_(&self, l: LoopIx) -> Result<Loop, TopoError> {
        alive(&self.live.loops, l, self.arenas.loops.get(l))
    }

    fn fin(&self, f: FinIx) -> Result<Fin, TopoError> {
        alive(&self.live.fins, f, self.arenas.fins.get(f))
    }

    fn edge(&self, e: EdgeIx) -> Result<Edge, TopoError> {
        alive(&self.live.edges, e, self.arenas.edges.get(e))
    }

    /// The vertex `fin` starts at; [`Refusal::Shape`] if its edge has none there.
    fn start(&self, fin: &Fin) -> Result<VertexIx, TopoError> {
        let e = self.edge(fin.edge)?;
        let v = match fin.sense {
            Sense::Forward => e.start,
            Sense::Reversed => e.end,
        };
        v.ok_or(TopoError::Euler(Refusal::Shape))
    }

    /// The two fins of `edge`, its first and that one's radial neighbour; [`Refusal::Ring`] unless
    /// they are two, each the other's neighbour.
    fn pair(&self, edge: &Edge) -> Result<(FinIx, FinIx), TopoError> {
        let ring = TopoError::Euler(Refusal::Ring);
        let x = edge.first_fin.ok_or(ring)?;
        let y = self.fin(x)?.radial;
        if x == y || self.fin(y)?.radial != x {
            return Err(ring);
        }
        Ok((x, y))
    }

    /// The fins from `from` along the loop up to, not including, `to`; the whole loop when `to` is
    /// `from`.
    fn segment(&self, from: FinIx, to: FinIx) -> Vec<FinIx> {
        let fins = &self.arenas.fins;
        successors(Some(from), |&f| {
            fins.next.get(f).copied().filter(|&n| n != to)
        })
        .take(usize::try_from(fins.len()).unwrap_or(usize::MAX))
        .collect()
    }

    /// Joins `a` to `b`: `b` follows `a` in their loop.
    fn link(&mut self, a: FinIx, b: FinIx) -> Result<(), TopoError> {
        self.arenas.fins.next.set(a, b)?;
        self.arenas.fins.prev.set(b, a)
    }

    /// Puts every fin of `fins` in loop `l`.
    fn move_fins(&mut self, fins: &[FinIx], l: LoopIx) -> Result<(), TopoError> {
        fins.iter()
            .try_for_each(|&f| self.arenas.fins.loop_.set(f, l))
    }

    fn add_vertex(&mut self, v: NewVertex) -> Result<VertexIx, TopoError> {
        let point = self.arenas.points.push(PointKind::Grid(v.point))?;
        let ix = self.arenas.vertices.push(Vertex { point })?;
        self.names.vertices.push(v.name);
        self.live.vertices.push(true);
        Ok(ix)
    }

    fn add_curve(&mut self, curve: CurveKind<3>) -> Result<CurveIx, TopoError> {
        self.arenas.curves.push(curve)
    }

    fn add_surface(&mut self, surface: SurfaceKind) -> Result<SurfaceIx, TopoError> {
        self.arenas.surfaces.push(surface)
    }

    fn add_edge(
        &mut self,
        e: NewEdge,
        start: VertexIx,
        end: VertexIx,
        first_fin: FinIx,
    ) -> Result<EdgeIx, TopoError> {
        let curve = self.add_curve(e.curve)?;
        let ix = self.arenas.edges.push(Edge {
            first_fin: Some(first_fin),
            start: Some(start),
            end: Some(end),
            curve,
        })?;
        self.names.edges.push(e.name);
        self.live.edges.push(true);
        Ok(ix)
    }

    fn add_shell(&mut self, region: RegionIx) -> Result<ShellIx, TopoError> {
        let ix = self.arenas.shells.push(Shell {
            region,
            next: None,
            first_face: None,
        })?;
        self.live.shells.push(true);
        self.faces_of_shell.push(0);
        recount(&mut self.shells_of_region, region.row(), |c| {
            c.saturating_add(1)
        });
        Ok(ix)
    }

    fn add_face(&mut self, f: NewFace, shell: ShellIx) -> Result<FaceIx, TopoError> {
        let surface = self.add_surface(f.surface)?;
        let ix = self.arenas.faces.push(Face {
            shell,
            next: None,
            surface,
            sense: f.sense,
            first_loop: None,
            bound: f.bound,
        })?;
        self.names.faces.push(f.name);
        self.live.faces.push(true);
        self.loops_of_face.push(0);
        recount(&mut self.faces_of_shell, shell.row(), |c| {
            c.saturating_add(1)
        });
        Ok(ix)
    }

    fn add_loop(&mut self, face: FaceIx, first_fin: FinIx) -> Result<LoopIx, TopoError> {
        let ix = self.arenas.loops.push(Loop {
            face,
            next: None,
            first_fin: Some(first_fin),
        })?;
        self.live.loops.push(true);
        recount(&mut self.loops_of_face, face.row(), |c| c.saturating_add(1));
        Ok(ix)
    }

    fn add_fin(&mut self, fin: Fin) -> Result<FinIx, TopoError> {
        let ix = self.arenas.fins.push(fin)?;
        self.live.fins.push(true);
        Ok(ix)
    }

    /// A new fin of `edge` in `loop_`, with no pcurve.
    const fn fin_row(
        loop_: LoopIx,
        next: FinIx,
        prev: FinIx,
        edge: EdgeIx,
        sense: Sense,
        radial: FinIx,
    ) -> Fin {
        Fin {
            loop_,
            next,
            prev,
            edge,
            sense,
            radial,
            pcurve: None,
        }
    }

    /// Makes a region of `kind`, with no shells.
    ///
    /// # Errors
    ///
    /// [`TopoError::Full`] or [`TopoError::OutOfMemory`] from the arena.
    pub fn make_region(&mut self, kind: RegionKind, name: NameKey) -> Result<RegionIx, TopoError> {
        let ix = self.arenas.regions.push(Region {
            kind,
            first_shell: None,
        })?;
        self.names.regions.push(name);
        self.live.regions.push(true);
        self.shells_of_region.push(0);
        Ok(ix)
    }

    /// Removes `region`, which must hold no shell.
    ///
    /// # Errors
    ///
    /// [`Refusal::Dead`]; [`Refusal::InUse`] if it holds a shell.
    pub fn kill_region(&mut self, region: RegionIx) -> Result<(), TopoError> {
        self.region(region)?;
        if count(&self.shells_of_region, region.row()) != 0 {
            return Err(TopoError::Euler(Refusal::InUse));
        }
        kill(&mut self.live.regions, region.row());
        Ok(())
    }

    /// Puts `shell` in region `to`, as a void shell inside a solid makes a cavity.
    ///
    /// # Errors
    ///
    /// [`Refusal::Dead`] if either is not live.
    pub fn move_shell(&mut self, shell: ShellIx, to: RegionIx) -> Result<(), TopoError> {
        let s = self.shell(shell)?;
        self.region(to)?;
        recount(&mut self.shells_of_region, s.region.row(), |c| {
            c.saturating_sub(1)
        });
        recount(&mut self.shells_of_region, to.row(), |c| {
            c.saturating_add(1)
        });
        self.arenas.shells.region.set(shell, to)
    }

    /// Makes a new shell in `region` holding one face, whose one loop runs along one new edge from
    /// `start` to `end` and back.
    ///
    /// # Errors
    ///
    /// [`Refusal::Dead`] if the region is not live; [`TopoError::Full`] or
    /// [`TopoError::OutOfMemory`] from an arena.
    pub fn make_wire(
        &mut self,
        region: RegionIx,
        face: NewFace,
        edge: NewEdge,
        start: NewVertex,
        end: NewVertex,
    ) -> Result<Wire, TopoError> {
        self.region(region)?;
        let a = &self.arenas;
        let (fins, loops, edges) = (a.fins.len(), a.loops.len(), a.edges.len());
        let (f0, f1): (FinIx, FinIx) = (after(fins, 0)?, after(fins, 1)?);
        let (l, e): (LoopIx, EdgeIx) = (after(loops, 0)?, after(edges, 0)?);
        let v0 = self.add_vertex(start)?;
        let v1 = self.add_vertex(end)?;
        self.add_edge(edge, v0, v1, f0)?;
        let shell = self.add_shell(region)?;
        let face = self.add_face(face, shell)?;
        self.add_loop(face, f0)?;
        self.add_fin(Euler::fin_row(l, f1, f1, e, Sense::Forward, f1))?;
        self.add_fin(Euler::fin_row(l, f0, f0, e, Sense::Reversed, f0))?;
        Ok(Wire {
            shell,
            face,
            loop_: l,
            edge: e,
            fins: [f0, f1],
            vertices: [v0, v1],
        })
    }

    /// Removes a lone wire: `edge`, its two fins, which must be the whole of their loop, the loop,
    /// its face, which must be its shell's only face, the shell and the edge's vertices.
    ///
    /// # Errors
    ///
    /// [`Refusal::Dead`]; [`Refusal::Ring`]; [`Refusal::Shape`] if the fins are not a loop of their
    /// own; [`Refusal::InUse`] if the face has another loop or the shell another face.
    pub fn kill_wire(&mut self, edge: EdgeIx) -> Result<(), TopoError> {
        let e = self.edge(edge)?;
        let (x, y) = self.pair(&e)?;
        let fx = self.fin(x)?;
        if fx.next != y || fx.prev != y {
            return Err(TopoError::Euler(Refusal::Shape));
        }
        let l = self.loop_(fx.loop_)?;
        let face = self.face(l.face)?;
        if count(&self.loops_of_face, l.face.row()) != 1
            || count(&self.faces_of_shell, face.shell.row()) != 1
        {
            return Err(TopoError::Euler(Refusal::InUse));
        }
        let shell = self.shell(face.shell)?;
        for v in [e.start, e.end].into_iter().flatten() {
            kill(&mut self.live.vertices, v.row());
        }
        kill(&mut self.live.edges, edge.row());
        kill(&mut self.live.fins, x.row());
        kill(&mut self.live.fins, y.row());
        kill(&mut self.live.loops, fx.loop_.row());
        kill(&mut self.live.faces, l.face.row());
        kill(&mut self.live.shells, face.shell.row());
        recount(&mut self.loops_of_face, l.face.row(), |_| 0);
        recount(&mut self.faces_of_shell, face.shell.row(), |_| 0);
        recount(&mut self.shells_of_region, shell.region.row(), |c| {
            c.saturating_sub(1)
        });
        Ok(())
    }

    /// Makes an edge from the vertex `at` starts at to a new vertex `vertex`, and back, in `at`'s
    /// loop just before `at`. Returns the edge and the vertex.
    ///
    /// # Errors
    ///
    /// [`Refusal::Dead`]; [`Refusal::Shape`] if `at` starts at no vertex; [`TopoError::Full`] or
    /// [`TopoError::OutOfMemory`] from an arena.
    pub fn mev(
        &mut self,
        at: FinIx,
        edge: NewEdge,
        vertex: NewVertex,
    ) -> Result<(EdgeIx, VertexIx), TopoError> {
        let fa = self.fin(at)?;
        let near = self.start(&fa)?;
        let fins = self.arenas.fins.len();
        let (g, h): (FinIx, FinIx) = (after(fins, 0)?, after(fins, 1)?);
        let e: EdgeIx = after(self.arenas.edges.len(), 0)?;
        let far = self.add_vertex(vertex)?;
        self.add_edge(edge, near, far, g)?;
        self.add_fin(Euler::fin_row(fa.loop_, h, fa.prev, e, Sense::Forward, h))?;
        self.add_fin(Euler::fin_row(fa.loop_, at, g, e, Sense::Reversed, g))?;
        self.arenas.fins.next.set(fa.prev, g)?;
        self.arenas.fins.prev.set(at, h)?;
        Ok((e, far))
    }

    /// Removes a spur: `edge`, whose two fins follow each other in one loop, out to its far vertex
    /// and back, and that vertex.
    ///
    /// # Errors
    ///
    /// [`Refusal::Dead`]; [`Refusal::Ring`]; [`Refusal::Shape`] if the fins are not next to each
    /// other, are the whole of their loop, or the edge is closed.
    pub fn kev(&mut self, edge: EdgeIx) -> Result<(), TopoError> {
        let e = self.edge(edge)?;
        let (x, y) = self.pair(&e)?;
        let (fx, fy) = (self.fin(x)?, self.fin(y)?);
        let shape = TopoError::Euler(Refusal::Shape);
        let (into, out) = match (e.start == e.end, fx.next == y, fy.next == x) {
            (false, true, false) => (fx, fy),
            (false, false, true) => (fy, fx),
            _ => return Err(shape),
        };
        let far = self.start(&out)?;
        self.link(into.prev, out.next)?;
        self.arenas
            .loops
            .first_fin
            .set(into.loop_, Some(out.next))?;
        kill(&mut self.live.vertices, far.row());
        kill(&mut self.live.edges, edge.row());
        kill(&mut self.live.fins, x.row());
        kill(&mut self.live.fins, y.row());
        Ok(())
    }

    /// Makes an edge from the vertex `from` starts at to the vertex `to` starts at, across their
    /// one loop, and a new face `face` on the same shell. The fins from `from` up to `to` (the
    /// whole loop when `to` is `from`), closed by the new edge's reversed fin, become the new
    /// face's loop; the rest, closed by its forward fin, stay. Returns the edge and the face.
    ///
    /// # Errors
    ///
    /// [`Refusal::Dead`]; [`Refusal::Loops`] if the fins are in two loops; [`Refusal::Shape`] if
    /// one starts at no vertex; [`TopoError::Full`] or [`TopoError::OutOfMemory`] from an arena.
    pub fn mef(
        &mut self,
        from: FinIx,
        to: FinIx,
        edge: NewEdge,
        face: NewFace,
    ) -> Result<(EdgeIx, FaceIx), TopoError> {
        let (ff, ft) = (self.fin(from)?, self.fin(to)?);
        if ff.loop_ != ft.loop_ {
            return Err(TopoError::Euler(Refusal::Loops));
        }
        let (v1, v2) = (self.start(&ff)?, self.start(&ft)?);
        let old = self.loop_(ff.loop_)?;
        let shell = self.face(old.face)?.shell;
        let moved = self.segment(from, to);
        let arenas = &self.arenas;
        let (fins, edges, loops) = (arenas.fins.len(), arenas.edges.len(), arenas.loops.len());
        let (g, h): (FinIx, FinIx) = (after(fins, 0)?, after(fins, 1)?);
        let (e, split): (EdgeIx, LoopIx) = (after(edges, 0)?, after(loops, 0)?);
        self.add_edge(edge, v1, v2, g)?;
        if from == to {
            // The new edge closes on itself: its forward fin is the old loop's only fin.
            self.add_fin(Euler::fin_row(ff.loop_, g, g, e, Sense::Forward, h))?;
        } else {
            self.add_fin(Euler::fin_row(ff.loop_, to, ff.prev, e, Sense::Forward, h))?;
            self.arenas.fins.next.set(ff.prev, g)?;
            self.arenas.fins.prev.set(to, g)?;
        }
        self.add_fin(Euler::fin_row(split, from, ft.prev, e, Sense::Reversed, g))?;
        self.arenas.fins.next.set(ft.prev, h)?;
        self.arenas.fins.prev.set(from, h)?;
        self.move_fins(&moved, split)?;
        self.arenas.loops.first_fin.set(ff.loop_, Some(g))?;
        let new = self.add_face(face, shell)?;
        self.add_loop(new, h)?;
        Ok((e, new))
    }

    /// Removes the edge of `gone` and the face of `gone`'s loop, which must be that face's only
    /// loop; the loop's other fins join the loop of the edge's other fin, whose face takes in the
    /// removed face's box.
    ///
    /// # Errors
    ///
    /// [`Refusal::Dead`]; [`Refusal::Ring`]; [`Refusal::Faces`] if both fins are in one face or
    /// the face to remove has another loop; [`Refusal::Shape`] if both loops are the edge's fin
    /// alone.
    pub fn kef(&mut self, gone: FinIx) -> Result<(), TopoError> {
        let fg = self.fin(gone)?;
        let edge = fg.edge;
        let (x, y) = self.pair(&self.edge(edge)?)?;
        let stay = if x == gone { y } else { x };
        let fs = self.fin(stay)?;
        let (lg, ls) = (self.loop_(fg.loop_)?, self.loop_(fs.loop_)?);
        if lg.face == ls.face || count(&self.loops_of_face, lg.face.row()) != 1 {
            return Err(TopoError::Euler(Refusal::Faces));
        }
        let (alone_g, alone_s) = (fg.next == gone, fs.next == stay);
        if alone_g && alone_s {
            return Err(TopoError::Euler(Refusal::Shape));
        }
        let (face_g, face_s) = (self.face(lg.face)?, self.face(ls.face)?);
        let moved = self.segment(fg.next, gone);
        if alone_g {
            self.link(fs.prev, fs.next)?;
        } else if alone_s {
            self.link(fg.prev, fg.next)?;
        } else {
            self.link(fs.prev, fg.next)?;
            self.link(fg.prev, fs.next)?;
        }
        self.move_fins(&moved, fs.loop_)?;
        let first = if alone_g { fs.next } else { fg.next };
        self.arenas.loops.first_fin.set(fs.loop_, Some(first))?;
        self.arenas
            .faces
            .bound
            .set(ls.face, face_s.bound.union(&face_g.bound))?;
        kill(&mut self.live.edges, edge.row());
        kill(&mut self.live.fins, gone.row());
        kill(&mut self.live.fins, stay.row());
        kill(&mut self.live.loops, fg.loop_.row());
        kill(&mut self.live.faces, lg.face.row());
        recount(&mut self.loops_of_face, lg.face.row(), |_| 0);
        recount(&mut self.faces_of_shell, face_g.shell.row(), |c| {
            c.saturating_sub(1)
        });
        Ok(())
    }

    /// Makes an edge from the vertex `from` starts at to the vertex `to` starts at, joining their
    /// two loops of one face into `from`'s loop. Returns the edge.
    ///
    /// # Errors
    ///
    /// [`Refusal::Dead`]; [`Refusal::Loops`] unless the fins are in two loops of one face;
    /// [`Refusal::Shape`] if one starts at no vertex; [`TopoError::Full`] or
    /// [`TopoError::OutOfMemory`] from an arena.
    pub fn mekr(&mut self, from: FinIx, to: FinIx, edge: NewEdge) -> Result<EdgeIx, TopoError> {
        let (ff, ft) = (self.fin(from)?, self.fin(to)?);
        let (l1, l2) = (self.loop_(ff.loop_)?, self.loop_(ft.loop_)?);
        if ff.loop_ == ft.loop_ || l1.face != l2.face {
            return Err(TopoError::Euler(Refusal::Loops));
        }
        let (v1, v2) = (self.start(&ff)?, self.start(&ft)?);
        let moved = self.segment(to, to);
        let fins = self.arenas.fins.len();
        let (g, h): (FinIx, FinIx) = (after(fins, 0)?, after(fins, 1)?);
        let e: EdgeIx = after(self.arenas.edges.len(), 0)?;
        self.add_edge(edge, v1, v2, g)?;
        self.add_fin(Euler::fin_row(ff.loop_, to, ff.prev, e, Sense::Forward, h))?;
        self.add_fin(Euler::fin_row(
            ff.loop_,
            from,
            ft.prev,
            e,
            Sense::Reversed,
            g,
        ))?;
        self.arenas.fins.next.set(ff.prev, g)?;
        self.arenas.fins.prev.set(to, g)?;
        self.arenas.fins.next.set(ft.prev, h)?;
        self.arenas.fins.prev.set(from, h)?;
        self.move_fins(&moved, ff.loop_)?;
        kill(&mut self.live.loops, ft.loop_.row());
        recount(&mut self.loops_of_face, l1.face.row(), |c| {
            c.saturating_sub(1)
        });
        Ok(e)
    }

    /// Removes the edge of `x`, whose two fins must be in one loop and not next to each other,
    /// splitting the loop in two: the fins after `x` up to the edge's other fin become a new loop
    /// (a ring) of the same face. Returns the ring.
    ///
    /// # Errors
    ///
    /// [`Refusal::Dead`]; [`Refusal::Ring`]; [`Refusal::Loops`] if the fins are in two loops;
    /// [`Refusal::Shape`] if they are next to each other; [`TopoError::Full`] or
    /// [`TopoError::OutOfMemory`] from an arena.
    pub fn kemr(&mut self, x: FinIx) -> Result<LoopIx, TopoError> {
        let fx = self.fin(x)?;
        let (p, q) = self.pair(&self.edge(fx.edge)?)?;
        let y = if p == x { q } else { p };
        let fy = self.fin(y)?;
        if fy.loop_ != fx.loop_ {
            return Err(TopoError::Euler(Refusal::Loops));
        }
        if fx.next == y || fy.next == x {
            return Err(TopoError::Euler(Refusal::Shape));
        }
        let face = self.loop_(fx.loop_)?.face;
        let moved = self.segment(fx.next, y);
        let ring: LoopIx = after(self.arenas.loops.len(), 0)?;
        self.link(fy.prev, fx.next)?;
        self.link(fx.prev, fy.next)?;
        self.add_loop(face, fx.next)?;
        self.move_fins(&moved, ring)?;
        self.arenas.loops.first_fin.set(fx.loop_, Some(fy.next))?;
        kill(&mut self.live.edges, fx.edge.row());
        kill(&mut self.live.fins, x.row());
        kill(&mut self.live.fins, y.row());
        Ok(ring)
    }

    /// Removes the face of `ring`, which must be that face's only loop, making `ring` a loop of
    /// `keep`, another face of the same shell, which takes in the removed face's box.
    ///
    /// # Errors
    ///
    /// [`Refusal::Dead`]; [`Refusal::Faces`] if the faces are one, are in two shells, or the face
    /// to remove has another loop.
    pub fn kfmrh(&mut self, keep: FaceIx, ring: LoopIx) -> Result<(), TopoError> {
        let fk = self.face(keep)?;
        let gone = self.loop_(ring)?.face;
        let fg = self.face(gone)?;
        if gone == keep || fg.shell != fk.shell || count(&self.loops_of_face, gone.row()) != 1 {
            return Err(TopoError::Euler(Refusal::Faces));
        }
        self.arenas.loops.face.set(ring, keep)?;
        self.arenas
            .faces
            .bound
            .set(keep, fk.bound.union(&fg.bound))?;
        recount(&mut self.loops_of_face, keep.row(), |c| c.saturating_add(1));
        recount(&mut self.loops_of_face, gone.row(), |_| 0);
        recount(&mut self.faces_of_shell, fg.shell.row(), |c| {
            c.saturating_sub(1)
        });
        kill(&mut self.live.faces, gone.row());
        Ok(())
    }

    /// Makes `ring`, a loop of a face that keeps another loop, the one loop of a new face `face` on
    /// the same shell. Returns the face.
    ///
    /// # Errors
    ///
    /// [`Refusal::Dead`]; [`Refusal::Faces`] if `ring` is its face's only loop;
    /// [`TopoError::Full`] or [`TopoError::OutOfMemory`] from an arena.
    pub fn mfkrh(&mut self, ring: LoopIx, face: NewFace) -> Result<FaceIx, TopoError> {
        let old = self.loop_(ring)?.face;
        let shell = self.face(old)?.shell;
        if count(&self.loops_of_face, old.row()) < 2 {
            return Err(TopoError::Euler(Refusal::Faces));
        }
        let new = self.add_face(face, shell)?;
        self.arenas.loops.face.set(ring, new)?;
        recount(&mut self.loops_of_face, old.row(), |c| c.saturating_sub(1));
        recount(&mut self.loops_of_face, new.row(), |c| c.saturating_add(1));
        Ok(new)
    }

    /// Drops the dead rows and the geometry no live entity uses, and publishes the rest by name
    /// ([`Builder::publish_named`], which rebuilds the lists the editor does not keep) as a body of `kind` whose infinite void region is `outside`.
    ///
    /// # Errors
    ///
    /// [`TopoError::NoSuchRow`] if `outside` is not a live region; otherwise any of
    /// [`Builder::publish_named`]'s errors.
    pub fn publish(
        self,
        kind: BodyKind,
        outside: RegionIx,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Body, TopoError> {
        let Euler {
            arenas,
            names,
            live,
            ..
        } = self;
        let a = &arenas;
        let surfaces = used(
            a.surfaces.len(),
            a.faces
                .surface
                .iter()
                .map(|s| Some(s.row()))
                .zip(live.faces.iter().copied()),
        );
        let curves = used(
            a.curves.len(),
            a.edges
                .curve
                .iter()
                .map(|c| Some(c.row()))
                .zip(live.edges.iter().copied()),
        );
        let pcurves = used(
            a.pcurves.len(),
            a.fins
                .pcurve
                .iter()
                .map(|p| p.map(Ix::row))
                .zip(live.fins.iter().copied()),
        );
        let points = used(
            a.points.len(),
            a.vertices
                .point
                .iter()
                .map(|p| Some(p.row()))
                .zip(live.vertices.iter().copied()),
        );
        // A plane is kept whichever vertices went: the pool is the body's, and its order the moves'.
        let planes: Vec<bool> = a.planes.iter().map(|_| true).collect();
        let keep = |mask: &[bool]| Keep::new(mask.iter().copied());
        let moves = Moves {
            regions: keep(&live.regions),
            shells: keep(&live.shells),
            faces: keep(&live.faces),
            loops: keep(&live.loops),
            fins: keep(&live.fins),
            edges: keep(&live.edges),
            vertices: keep(&live.vertices),
            surfaces: keep(&surfaces),
            curves: keep(&curves),
            pcurves: keep(&pcurves),
            points: keep(&points),
            planes: keep(&planes),
        };
        let kept = moves.arenas(&arenas, meter)?;
        let names = Names {
            regions: moves.regions.apply(&names.regions),
            faces: moves.faces.apply(&names.faces),
            edges: moves.edges.apply(&names.edges),
            vertices: moves.vertices.apply(&names.vertices),
        };
        let outside = to(&moves.regions, outside)?;
        Builder { arenas: kept }.publish_named(&names, kind, outside, meter)
    }
}

/// Bounded proofs (chapter 28.7; piece G112), proven by `kernel-fuzz.yml`'s `kani-topo` job.
#[cfg(kani)]
// cargo-mutants reads this attribute from the source and skips the module (its harnesses are
// proven by Kani, and no test build compiles them); no build evaluates it, since the module
// exists only under Kani, where `test` is off.
#[cfg_attr(test, mutants::skip)]
mod proofs {
    use geom_curve::Line;
    use geom_surface::Plane;

    use super::*;

    /// The arguments every operator takes: one name per call, and shared geometry, which no
    /// operator reads.
    struct Stock {
        names: u8,
        curve: CurveKind<3>,
        surface: SurfaceKind,
        bound: Box3,
        point: P3,
    }

    impl Stock {
        fn new() -> Stock {
            let point = P3::new(0, 0, 0).ok().unwrap();
            Stock {
                names: 0,
                curve: CurveKind::Line(Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 1.0)).unwrap()),
                surface: SurfaceKind::Plane(
                    Plane::new(
                        [0.0; 3],
                        [1.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        ((0.0, 1.0), (0.0, 1.0)),
                    )
                    .unwrap(),
                ),
                bound: Box3::new(point, point).ok().unwrap(),
                point,
            }
        }

        fn name(&mut self) -> NameKey {
            self.names += 1;
            NameKey([self.names; 16])
        }

        fn edge(&mut self) -> NewEdge {
            NewEdge {
                name: self.name(),
                curve: self.curve.clone(),
            }
        }

        fn face(&mut self) -> NewFace {
            NewFace {
                name: self.name(),
                surface: self.surface.clone(),
                sense: Sense::Forward,
                bound: self.bound,
            }
        }

        fn vertex(&mut self) -> NewVertex {
            NewVertex {
                name: self.name(),
                point: self.point,
            }
        }
    }

    /// An editor holding one region and one wire.
    fn wire(stock: &mut Stock) -> Euler {
        let mut e = Euler::new();
        let name = stock.name();
        let region = e.make_region(RegionKind::Solid, name).unwrap();
        let (face, edge) = (stock.face(), stock.edge());
        let (start, end) = (stock.vertex(), stock.vertex());
        e.make_wire(region, face, edge, start, end).unwrap();
        e
    }

    /// The live rows of arena `arena`, of `rows` rows.
    fn live_rows(e: &Euler, arena: Arena, rows: u32) -> i32 {
        let mut count = 0;
        for row in 0..rows {
            if e.is_live(arena, row) {
                count += 1;
            }
        }
        count
    }

    /// Every combinatorial rule the operators promise: each live fin's links, partner and loop;
    /// each live loop's ring holding all its fins; each live edge's first fin; Euler's formula.
    fn assert_rules(e: &Euler, holes: i32, rows: u32) {
        let a = &e.arenas;
        assert!(a.fins.len() == rows);
        for row in 0..rows {
            if !e.is_live(Arena::Fin, row) {
                continue;
            }
            let x: FinIx = Ix::new(row).unwrap();
            let fin = e.fin(x).unwrap();
            let next = e.fin(fin.next).unwrap();
            let prev = e.fin(fin.prev).unwrap();
            let partner = e.fin(fin.radial).unwrap();
            assert!(next.prev == x && prev.next == x);
            assert!(next.loop_ == fin.loop_);
            assert!(fin.radial != x && partner.radial == x);
            assert!(partner.edge == fin.edge && partner.sense != fin.sense);
            assert!(e.start(&partner).unwrap() == e.start(&next).unwrap());
            assert!(e.loop_(fin.loop_).is_ok());
        }
        for row in 0..a.loops.len() {
            if !e.is_live(Arena::Loop, row) {
                continue;
            }
            let l: LoopIx = Ix::new(row).unwrap();
            let first = e.loop_(l).unwrap().first_fin.unwrap();
            let ring = e.segment(first, first);
            assert!(
                ring.iter()
                    .all(|&f| e.fin(f).is_ok_and(|fin| fin.loop_ == l))
            );
            let mut fins = 0;
            for f in 0..rows {
                let x: FinIx = Ix::new(f).unwrap();
                if e.fin(x).is_ok_and(|fin| fin.loop_ == l) {
                    fins += 1;
                }
            }
            assert!(fins == ring.len());
        }
        for row in 0..a.edges.len() {
            if !e.is_live(Arena::Edge, row) {
                continue;
            }
            let x: EdgeIx = Ix::new(row).unwrap();
            let first = e.edge(x).unwrap().first_fin.unwrap();
            assert!(e.fin(first).is_ok_and(|fin| fin.edge == x));
        }
        let v = live_rows(e, Arena::Vertex, a.vertices.len());
        let edges = live_rows(e, Arena::Edge, a.edges.len());
        let f = live_rows(e, Arena::Face, a.faces.len());
        let l = live_rows(e, Arena::Loop, a.loops.len());
        let s = live_rows(e, Arena::Shell, a.shells.len());
        assert!(v - edges + f - (l - f) == 2 * (s - holes));
    }

    /// Stands in for [`Euler::add_curve`]: no operator reads a curve.
    fn no_curve(_: &mut Euler, _: CurveKind<3>) -> Result<CurveIx, TopoError> {
        Ok(Ix::new(0).unwrap())
    }

    /// Stands in for [`Euler::add_surface`]: no operator reads a surface.
    fn no_surface(_: &mut Euler, _: SurfaceKind) -> Result<SurfaceIx, TopoError> {
        Ok(Ix::new(0).unwrap())
    }

    /// Stands in for `geom_curve::frame::not_parallel`, an exact sum whose loops CBMC cannot bound,
    /// which the stock's one plane meets, on axes that are not parallel.
    fn apart(_: [f64; 3], _: [f64; 3]) -> bool {
        true
    }

    /// MEV at the wire's first fin, then KEV of the new edge: every rule after each.
    #[kani::proof]
    #[kani::unwind(14)]
    #[kani::stub(geom_curve::frame::not_parallel, apart)]
    #[kani::stub(Euler::add_curve, no_curve)]
    #[kani::stub(Euler::add_surface, no_surface)]
    fn mev_and_kev() {
        let mut stock = Stock::new();
        let mut e = wire(&mut stock);
        let at: FinIx = Ix::new(0).unwrap();
        let (edge, vertex) = (stock.edge(), stock.vertex());
        let (new, _) = e.mev(at, edge, vertex).unwrap();
        assert_rules(&e, 0, 4);
        e.kev(new).unwrap();
        assert_rules(&e, 0, 4);
    }

    /// Row `row` of the fins.
    fn fin(row: u32) -> FinIx {
        Ix::new(row).unwrap()
    }

    /// MEF across a wire's two fins, then KEF of the new face: every rule after each.
    #[kani::proof]
    #[kani::unwind(14)]
    #[kani::stub(geom_curve::frame::not_parallel, apart)]
    #[kani::stub(Euler::add_curve, no_curve)]
    #[kani::stub(Euler::add_surface, no_surface)]
    fn mef_and_kef() {
        let mut stock = Stock::new();
        let mut e = wire(&mut stock);
        let (edge, face) = (stock.edge(), stock.face());
        e.mef(fin(0), fin(1), edge, face).unwrap();
        assert_rules(&e, 0, 4);
        e.kef(fin(3)).unwrap();
        assert_rules(&e, 0, 4);
    }

    /// MEF from a fin to itself, a closed edge whose forward fin is a loop of its own.
    #[kani::proof]
    #[kani::unwind(14)]
    #[kani::stub(geom_curve::frame::not_parallel, apart)]
    #[kani::stub(Euler::add_curve, no_curve)]
    #[kani::stub(Euler::add_surface, no_surface)]
    fn mef_closes_an_edge() {
        let mut stock = Stock::new();
        let mut e = wire(&mut stock);
        let (edge, face) = (stock.edge(), stock.face());
        e.mef(fin(0), fin(0), edge, face).unwrap();
        assert_rules(&e, 0, 4);
    }

    /// A wire with a spur on each side, so its two fins are not next to each other: [`Euler::kemr`]
    /// applies to it. Six fins.
    fn spurred(stock: &mut Stock) -> Euler {
        let mut e = wire(stock);
        let (edge, vertex) = (stock.edge(), stock.vertex());
        e.mev(fin(1), edge, vertex).unwrap();
        let (edge, vertex) = (stock.edge(), stock.vertex());
        e.mev(fin(0), edge, vertex).unwrap();
        e
    }

    /// KEMR on a spurred wire, then MEKR joining the ring back: every rule after each.
    #[kani::proof]
    #[kani::unwind(14)]
    #[kani::stub(geom_curve::frame::not_parallel, apart)]
    #[kani::stub(Euler::add_curve, no_curve)]
    #[kani::stub(Euler::add_surface, no_surface)]
    fn kemr_and_mekr() {
        let mut stock = Stock::new();
        let mut e = spurred(&mut stock);
        e.kemr(fin(0)).unwrap();
        assert_rules(&e, 0, 6);
        let edge = stock.edge();
        e.mekr(fin(2), fin(4), edge).unwrap();
        assert_rules(&e, 0, 8);
    }

    /// MFKRH makes a ring a face of its own, KFMRH makes it a ring again: every rule after each,
    /// with one hole fewer between them.
    #[kani::proof]
    #[kani::unwind(14)]
    #[kani::stub(geom_curve::frame::not_parallel, apart)]
    #[kani::stub(Euler::add_curve, no_curve)]
    #[kani::stub(Euler::add_surface, no_surface)]
    fn mfkrh_and_kfmrh() {
        let mut stock = Stock::new();
        let mut e = spurred(&mut stock);
        let ring = e.kemr(fin(0)).unwrap();
        let face = stock.face();
        e.mfkrh(ring, face).unwrap();
        assert_rules(&e, -1, 6);
        e.kfmrh(Ix::new(0).unwrap(), ring).unwrap();
        assert_rules(&e, 0, 6);
    }

    /// A lone wire removed: nothing is left live.
    #[kani::proof]
    #[kani::unwind(14)]
    #[kani::stub(geom_curve::frame::not_parallel, apart)]
    #[kani::stub(Euler::add_curve, no_curve)]
    #[kani::stub(Euler::add_surface, no_surface)]
    fn kill_wire_leaves_nothing() {
        let mut stock = Stock::new();
        let mut e = wire(&mut stock);
        e.kill_wire(Ix::new(0).unwrap()).unwrap();
        assert_rules(&e, 0, 2);
        assert!(live_rows(&e, Arena::Vertex, 2) == 0 && live_rows(&e, Arena::Fin, 2) == 0);
    }

    /// What a lone wire refuses changes nothing: KEV (its fins are the whole of their loop), KEF
    /// (both fins are in one face) and KEMR (they are next to each other).
    #[kani::proof]
    #[kani::unwind(14)]
    #[kani::stub(geom_curve::frame::not_parallel, apart)]
    #[kani::stub(Euler::add_curve, no_curve)]
    #[kani::stub(Euler::add_surface, no_surface)]
    fn refusals_change_nothing() {
        let mut stock = Stock::new();
        let mut e = wire(&mut stock);
        let rows = [e.fin(fin(0)).unwrap(), e.fin(fin(1)).unwrap()];
        assert!(e.kev(Ix::new(0).unwrap()).is_err());
        assert!(e.kef(fin(0)).is_err());
        assert!(e.kemr(fin(0)).is_err());
        assert!([e.fin(fin(0)).unwrap(), e.fin(fin(1)).unwrap()] == rows);
        assert!(e.arenas.fins.len() == 2 && e.arenas.loops.len() == 1);
        assert_rules(&e, 0, 2);
    }
}
