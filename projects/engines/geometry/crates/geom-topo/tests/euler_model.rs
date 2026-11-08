//! The Euler operators against a model, in lockstep (G81's done-when): a random sequence of
//! operators, regions and publishes, 100,000 steps, drives the editor and a model of plain lists
//! keyed by name. At every step both agree on whether the operator applies, a refusal leaves the
//! editor unchanged, both describe the same topology, every loop is closed and every ring paired,
//! and Euler's formula holds; every few steps the editor is published and edited again, and the
//! body it gives is the body it came from.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "test models and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

use common::{ix, line, meter, plane};
use geom_math::Box3;
use geom_num::grid::P3;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};
use geom_topo::order::NameKey;
use geom_topo::{
    Arena, BodyKind, Euler, FinIx, NewEdge, NewFace, NewVertex, Refusal, RegionIx, RegionKind,
    Sense, TopoError,
};

/// The steps the run takes.
const STEPS: u32 = 100_000;

/// A fin as the model knows it: its edge's name and whether it runs along the edge.
type Use = (NameKey, bool);

/// What both sides describe: regions with their kinds and shells (each the set of its faces'
/// names), each face's loops (each turned to start at its least fin), edges and vertices.
#[derive(Debug, PartialEq, Eq)]
struct State {
    regions: BTreeMap<NameKey, (RegionKind, BTreeSet<BTreeSet<NameKey>>)>,
    faces: BTreeMap<NameKey, Vec<Vec<Use>>>,
    edges: BTreeMap<NameKey, (NameKey, NameKey)>,
    vertices: BTreeSet<NameKey>,
}

/// `l` turned to start at index `i`.
fn rot(l: &[Use], i: usize) -> Vec<Use> {
    l[i..].iter().chain(&l[..i]).copied().collect()
}

/// `l` turned to start where it is least.
fn norm(l: &[Use]) -> Vec<Use> {
    (0..l.len()).map(|i| rot(l, i)).min().unwrap()
}

/// The model: plain lists keyed by name.
#[derive(Clone, Debug, Default)]
struct Model {
    regions: BTreeMap<NameKey, RegionKind>,
    /// Shell token to region.
    shells: BTreeMap<u64, NameKey>,
    /// Face to its shell token and loops.
    faces: BTreeMap<NameKey, (u64, Vec<Vec<Use>>)>,
    edges: BTreeMap<NameKey, (NameKey, NameKey)>,
    vertices: BTreeSet<NameKey>,
    holes: i64,
    tokens: u64,
}

impl Model {
    /// Where `u` is: its face, loop and place in the loop.
    fn find(&self, u: Use) -> (NameKey, usize, usize) {
        for (name, (_, loops)) in &self.faces {
            for (li, l) in loops.iter().enumerate() {
                if let Some(p) = l.iter().position(|&x| x == u) {
                    return (*name, li, p);
                }
            }
        }
        panic!("no fin {u:?}");
    }

    fn start(&self, u: Use) -> NameKey {
        let (s, e) = self.edges[&u.0];
        if u.1 { s } else { e }
    }

    fn loop_mut(&mut self, f: NameKey, li: usize) -> &mut Vec<Use> {
        &mut self.faces.get_mut(&f).unwrap().1[li]
    }

    fn make_wire(
        &mut self,
        region: NameKey,
        face: NameKey,
        edge: NameKey,
        v: [NameKey; 2],
    ) -> bool {
        if !self.regions.contains_key(&region) {
            return false;
        }
        self.tokens += 1;
        self.shells.insert(self.tokens, region);
        self.faces
            .insert(face, (self.tokens, vec![vec![(edge, true), (edge, false)]]));
        self.edges.insert(edge, (v[0], v[1]));
        self.vertices.extend(v);
        true
    }

    fn kill_wire(&mut self, edge: NameKey) -> bool {
        let (f, li, _) = self.find((edge, true));
        let (f2, li2, _) = self.find((edge, false));
        let (token, loops) = self.faces[&f].clone();
        let faces_in_shell = self.faces.values().filter(|x| x.0 == token).count();
        if (f, li) != (f2, li2) || loops[li].len() != 2 || loops.len() != 1 || faces_in_shell != 1 {
            return false;
        }
        let (s, e) = self.edges.remove(&edge).unwrap();
        self.vertices.remove(&s);
        self.vertices.remove(&e);
        self.faces.remove(&f);
        self.shells.remove(&token);
        true
    }

    fn mev(&mut self, at: Use, edge: NameKey, vertex: NameKey) -> bool {
        let (f, li, p) = self.find(at);
        let v = self.start(at);
        let l = self.loop_mut(f, li);
        l.insert(p, (edge, true));
        l.insert(p + 1, (edge, false));
        self.edges.insert(edge, (v, vertex));
        self.vertices.insert(vertex);
        true
    }

    fn kev(&mut self, edge: NameKey) -> bool {
        let (f, li, p1) = self.find((edge, true));
        let (f2, li2, p2) = self.find((edge, false));
        let (s, e) = self.edges[&edge];
        let n = self.faces[&f].1[li].len();
        let (a1, a2) = ((p1 + 1) % n == p2, (p2 + 1) % n == p1);
        if (f, li) != (f2, li2) || s == e || a1 == a2 {
            return false;
        }
        // The far vertex is where the fin out ends and the fin back starts.
        let far = if a1 { e } else { s };
        self.loop_mut(f, li).retain(|x| x.0 != edge);
        self.edges.remove(&edge);
        self.vertices.remove(&far);
        true
    }

    fn mef(&mut self, from: Use, to: Use, edge: NameKey, face: NameKey) -> bool {
        let (f, li, i) = self.find(from);
        let (f2, li2, j) = self.find(to);
        if (f, li) != (f2, li2) {
            return false;
        }
        let (v1, v2) = (self.start(from), self.start(to));
        let l = self.faces[&f].1[li].clone();
        let r = rot(&l, i);
        let k = if i == j {
            l.len()
        } else {
            (j + l.len() - i) % l.len()
        };
        let mut moved: Vec<Use> = r[..k].to_vec();
        let mut kept: Vec<Use> = r[k..].to_vec();
        moved.push((edge, false));
        kept.push((edge, true));
        *self.loop_mut(f, li) = kept;
        let token = self.faces[&f].0;
        self.faces.insert(face, (token, vec![moved]));
        self.edges.insert(edge, (v1, v2));
        true
    }

    fn kef(&mut self, gone: Use) -> bool {
        let stay = (gone.0, !gone.1);
        let (fg, lg, pg) = self.find(gone);
        let (fs, ls, ps) = self.find(stay);
        let g = self.faces[&fg].1[lg].clone();
        let s = self.faces[&fs].1[ls].clone();
        if fg == fs || self.faces[&fg].1.len() != 1 || (g.len() == 1 && s.len() == 1) {
            return false;
        }
        let mut merged: Vec<Use> = rot(&s, ps)[1..].to_vec();
        merged.extend_from_slice(&rot(&g, pg)[1..]);
        *self.loop_mut(fs, ls) = merged;
        self.faces.remove(&fg);
        self.edges.remove(&gone.0);
        true
    }

    fn mekr(&mut self, from: Use, to: Use, edge: NameKey) -> bool {
        let (f1, l1, p1) = self.find(from);
        let (f2, l2, p2) = self.find(to);
        if f1 != f2 || l1 == l2 {
            return false;
        }
        let (v1, v2) = (self.start(from), self.start(to));
        let loops = &mut self.faces.get_mut(&f1).unwrap().1;
        let mut joined = rot(&loops[l1], p1);
        joined.push((edge, true));
        joined.extend(rot(&loops[l2], p2));
        joined.push((edge, false));
        loops[l1] = joined;
        loops.remove(l2);
        self.edges.insert(edge, (v1, v2));
        true
    }

    fn kemr(&mut self, x: Use) -> bool {
        let y = (x.0, !x.1);
        let (f, li, p) = self.find(x);
        let (f2, li2, q) = self.find(y);
        if (f, li) != (f2, li2) {
            return false;
        }
        let n = self.faces[&f].1[li].len();
        if (p + 1) % n == q || (q + 1) % n == p {
            return false;
        }
        let r = rot(&self.faces[&f].1[li], p);
        let at = r.iter().position(|&u| u == y).unwrap();
        let ring = r[1..at].to_vec();
        let rest = r[at + 1..].to_vec();
        let loops = &mut self.faces.get_mut(&f).unwrap().1;
        loops[li] = rest;
        loops.push(ring);
        self.edges.remove(&x.0);
        true
    }

    fn kfmrh(&mut self, keep: NameKey, member: Use) -> bool {
        let (gone, li, _) = self.find(member);
        let (tg, loops) = self.faces[&gone].clone();
        if gone == keep || tg != self.faces[&keep].0 || loops.len() != 1 {
            return false;
        }
        self.faces.remove(&gone);
        self.faces.get_mut(&keep).unwrap().1.push(loops[li].clone());
        self.holes += 1;
        true
    }

    fn mfkrh(&mut self, member: Use, face: NameKey) -> bool {
        let (f, li, _) = self.find(member);
        if self.faces[&f].1.len() < 2 {
            return false;
        }
        let token = self.faces[&f].0;
        let ring = self.faces.get_mut(&f).unwrap().1.remove(li);
        self.faces.insert(face, (token, vec![ring]));
        self.holes -= 1;
        true
    }

    fn kill_region(&mut self, r: NameKey) -> bool {
        if self.shells.values().any(|&x| x == r) {
            return false;
        }
        self.regions.remove(&r);
        true
    }

    fn move_shell(&mut self, face: NameKey, to: NameKey) -> bool {
        let token = self.faces[&face].0;
        self.shells.insert(token, to);
        true
    }

    fn state(&self) -> State {
        let mut regions = BTreeMap::new();
        for (name, kind) in &self.regions {
            let shells = self
                .shells
                .iter()
                .filter(|(_, r)| *r == name)
                .map(|(t, _)| {
                    self.faces
                        .iter()
                        .filter(|(_, x)| x.0 == *t)
                        .map(|(f, _)| *f)
                        .collect()
                })
                .collect();
            regions.insert(*name, (*kind, shells));
        }
        let faces = self
            .faces
            .iter()
            .map(|(f, (_, loops))| {
                let mut l: Vec<Vec<Use>> = loops.iter().map(|x| norm(x)).collect();
                l.sort();
                (*f, l)
            })
            .collect();
        State {
            regions,
            faces,
            edges: self.edges.clone(),
            vertices: self.vertices.clone(),
        }
    }

    /// V − E + F − R − 2 (S − H), zero while Euler's formula holds.
    fn euler(&self) -> i64 {
        let f = self.faces.len() as i64;
        let l: i64 = self.faces.values().fold(0, |n, x| n + x.1.len() as i64);
        self.vertices.len() as i64 - self.edges.len() as i64 + f
            - (l - f)
            - 2 * (self.shells.len() as i64 - self.holes)
    }
}

/// The fin at row `f` as the model knows it.
fn use_of(e: &Euler, f: FinIx) -> Use {
    let fin = e.arenas().fins.get(f).unwrap();
    (
        e.names().edges[fin.edge.to_usize()],
        fin.sense == Sense::Forward,
    )
}

/// The editor's live rows of `arena`.
fn live(e: &Euler, arena: Arena) -> Vec<u32> {
    (0..e.arenas().rows(arena))
        .filter(|&r| e.is_live(arena, r))
        .collect()
}

/// What the editor describes, checking as it reads that every live loop is a closed cycle of live
/// fins, each the next of the one before and starting where it ends, that every edge's fins are a
/// radial pair, and that every live fin is in exactly one loop.
fn state_of(e: &Euler) -> State {
    let a = e.arenas();
    let n = e.names();
    let fin = |f: FinIx| a.fins.get(f).unwrap();
    let start = |f: FinIx| {
        let x = fin(f);
        let edge = a.edges.get(x.edge).unwrap();
        match x.sense {
            Sense::Forward => edge.start.unwrap(),
            Sense::Reversed => edge.end.unwrap(),
        }
    };
    let mut regions = BTreeMap::new();
    for r in live(e, Arena::Region) {
        regions.insert(
            n.regions[r as usize],
            (a.regions.get(ix(r)).unwrap().kind, BTreeSet::new()),
        );
    }
    let mut shells: BTreeMap<u32, BTreeSet<NameKey>> = BTreeMap::new();
    for s in live(e, Arena::Shell) {
        shells.insert(s, BTreeSet::new());
    }
    let mut faces: BTreeMap<NameKey, Vec<Vec<Use>>> = BTreeMap::new();
    for f in live(e, Arena::Face) {
        let face = a.faces.get(ix(f)).unwrap();
        assert!(e.is_live(Arena::Shell, face.shell.row()));
        shells
            .get_mut(&face.shell.row())
            .unwrap()
            .insert(n.faces[f as usize]);
        faces.insert(n.faces[f as usize], Vec::new());
    }
    for (s, set) in shells {
        let region = a.shells.get(ix(s)).unwrap().region;
        assert!(e.is_live(Arena::Region, region.row()));
        regions
            .get_mut(&n.regions[region.to_usize()])
            .unwrap()
            .1
            .insert(set);
    }
    let mut walked = 0;
    for l in live(e, Arena::Loop) {
        let lp = a.loops.get(ix(l)).unwrap();
        assert!(e.is_live(Arena::Face, lp.face.row()));
        let first = lp.first_fin.unwrap();
        let mut uses = Vec::new();
        let mut f = first;
        loop {
            assert!(e.is_live(Arena::Fin, f.row()));
            let x = fin(f);
            assert_eq!(x.loop_, ix(l));
            assert_eq!(fin(x.next).prev, f);
            assert_eq!(start(x.next), {
                let edge = a.edges.get(x.edge).unwrap();
                match x.sense {
                    Sense::Forward => edge.end.unwrap(),
                    Sense::Reversed => edge.start.unwrap(),
                }
            });
            let r = fin(x.radial);
            assert_eq!((r.edge, r.radial), (x.edge, f));
            assert_ne!(r.sense, x.sense);
            uses.push(use_of(e, f));
            walked += 1;
            f = x.next;
            if f == first {
                break;
            }
            assert!(uses.len() <= a.fins.len() as usize);
        }
        faces
            .get_mut(&n.faces[lp.face.to_usize()])
            .unwrap()
            .push(norm(&uses));
    }
    for loops in faces.values_mut() {
        loops.sort();
    }
    assert_eq!(walked, live(e, Arena::Fin).len());
    let mut edges = BTreeMap::new();
    for x in live(e, Arena::Edge) {
        let edge = a.edges.get(ix(x)).unwrap();
        let first = edge.first_fin.unwrap();
        assert!(e.is_live(Arena::Fin, first.row()));
        assert_eq!(fin(first).edge, ix(x));
        let name = |v: Option<geom_topo::VertexIx>| {
            let v = v.unwrap();
            assert!(e.is_live(Arena::Vertex, v.row()));
            n.vertices[v.to_usize()]
        };
        edges.insert(n.edges[x as usize], (name(edge.start), name(edge.end)));
    }
    let vertices = live(e, Arena::Vertex)
        .into_iter()
        .map(|v| n.vertices[v as usize])
        .collect();
    State {
        regions,
        faces,
        edges,
        vertices,
    }
}

/// The driver: the editor, the model, and names to hand out.
struct Run {
    euler: Euler,
    model: Model,
    rng: SplitMix64,
    names: u64,
    outside: NameKey,
    /// The model the step works on, kept when the step applies.
    trial: Model,
}

fn below(rng: &mut SplitMix64, n: usize) -> usize {
    rng.below(NonZeroU64::new(n as u64).unwrap()) as usize
}

impl Run {
    fn name(&mut self) -> NameKey {
        self.names += 1;
        let mut k = [0u8; 16];
        k[..8].copy_from_slice(&self.rng.next_u64().to_be_bytes());
        k[8..].copy_from_slice(&self.names.to_be_bytes());
        NameKey(k)
    }

    fn face(&mut self) -> NewFace {
        NewFace {
            name: self.name(),
            surface: plane([0.0; 3], 0, 1),
            sense: Sense::Forward,
            bound: Box3::new(P3::new(0, 0, 0).unwrap(), P3::new(1, 1, 1).unwrap()).unwrap(),
        }
    }

    fn edge(&mut self) -> NewEdge {
        NewEdge {
            name: self.name(),
            curve: line([0, 0, 0], [1, 0, 0]),
        }
    }

    fn vertex(&mut self) -> NewVertex {
        NewVertex {
            name: self.name(),
            point: P3::new(0, 0, 0).unwrap(),
        }
    }

    /// A row of `arena` to act on: usually a live one, at times a dead one.
    fn pick(&mut self, arena: Arena) -> Option<u32> {
        let rows = self.euler.arenas().rows(arena);
        let all: Vec<u32> = live(&self.euler, arena);
        if rows > 0 && below(&mut self.rng, 20) == 0 {
            let dead: Vec<u32> = (0..rows).filter(|r| !all.contains(r)).collect();
            if !dead.is_empty() {
                return Some(dead[below(&mut self.rng, dead.len())]);
            }
        }
        if all.is_empty() {
            return None;
        }
        Some(all[below(&mut self.rng, all.len())])
    }

    /// A fin in another loop of `f`'s face, or in `f`'s own loop, if there is one.
    fn partner(&mut self, f: FinIx, same_loop: bool) -> Option<FinIx> {
        let a = self.euler.arenas();
        let me = a.fins.get(f)?;
        let face = a.loops.get(me.loop_)?.face;
        let fins: Vec<u32> = live(&self.euler, Arena::Fin)
            .into_iter()
            .filter(|&g| {
                let x = a.fins.get(ix(g)).unwrap();
                let l = a.loops.get(x.loop_).unwrap();
                l.face == face && (x.loop_ == me.loop_) == same_loop
            })
            .collect();
        (!fins.is_empty()).then(|| ix(fins[below(&mut self.rng, fins.len())]))
    }

    fn is_dead(&self, arena: Arena, row: u32) -> bool {
        !self.euler.is_live(arena, row)
    }

    /// `act` on a live `row` of `arena`; on a dead one, the refusal it must give.
    fn checked(
        &mut self,
        arena: Arena,
        row: u32,
        act: impl FnOnce(&mut Run) -> (bool, Result<(), TopoError>),
    ) -> (bool, Result<(), TopoError>) {
        if self.is_dead(arena, row) {
            (false, self.euler_dead(arena, row))
        } else {
            act(self)
        }
    }

    /// One random step; returns what the model said and what the editor did.
    fn step(&mut self) -> (bool, Result<(), TopoError>) {
        let size = live(&self.euler, Arena::Fin).len();
        let grow = size < 60;
        let op = below(&mut self.rng, 13);
        self.trial = self.model.clone();
        let result: (bool, Result<(), TopoError>) = match op {
            0 if grow || live(&self.euler, Arena::Shell).is_empty() => {
                let region = self.pick(Arena::Region).unwrap();
                let (face, edge, v0, v1) = (self.face(), self.edge(), self.vertex(), self.vertex());
                let ok = !self.is_dead(Arena::Region, region)
                    && self.trial.make_wire(
                        self.euler.names().regions[region as usize],
                        face.name,
                        edge.name,
                        [v0.name, v1.name],
                    );
                let r = self
                    .euler
                    .make_wire(ix(region), face, edge, v0, v1)
                    .map(|_| ());
                (ok, r)
            }
            1 | 2 if grow => match self.pick(Arena::Fin) {
                None => return (true, Ok(())),
                Some(f) => {
                    let (edge, vertex) = (self.edge(), self.vertex());
                    self.checked(Arena::Fin, f, |run| {
                        let ok = run
                            .trial
                            .mev(use_of(&run.euler, ix(f)), edge.name, vertex.name);
                        (ok, run.euler.mev(ix(f), edge, vertex).map(|_| ()))
                    })
                }
            },
            3 | 4 => match self.pick(Arena::Edge) {
                None => return (true, Ok(())),
                Some(x) => self.checked(Arena::Edge, x, |run| {
                    let name = run.euler.names().edges[x as usize];
                    (run.trial.kev(name), run.euler.kev(ix(x)))
                }),
            },
            5 if grow => match self.pick(Arena::Fin) {
                None => return (true, Ok(())),
                Some(f) => {
                    let to = match below(&mut self.rng, 16) {
                        0..=3 => Some(ix(f)),
                        4..=6 => self.pick(Arena::Fin).map(ix),
                        _ => self.partner(ix(f), true),
                    };
                    let (edge, face) = (self.edge(), self.face());
                    match to {
                        None => return (true, Ok(())),
                        Some(t)
                            if self.is_dead(Arena::Fin, f) || self.is_dead(Arena::Fin, t.row()) =>
                        {
                            let r = self.euler.mef(ix(f), t, edge, face).map(|_| ());
                            assert!(matches!(r, Err(TopoError::Euler(Refusal::Dead { .. }))));
                            (false, r)
                        }
                        Some(t) => {
                            let ok = self.trial.mef(
                                use_of(&self.euler, ix(f)),
                                use_of(&self.euler, t),
                                edge.name,
                                face.name,
                            );
                            (ok, self.euler.mef(ix(f), t, edge, face).map(|_| ()))
                        }
                    }
                }
            },
            6 => match self.pick(Arena::Fin) {
                None => return (true, Ok(())),
                Some(f) => self.checked(Arena::Fin, f, |run| {
                    (
                        run.trial.kef(use_of(&run.euler, ix(f))),
                        run.euler.kef(ix(f)),
                    )
                }),
            },
            7 if grow => match self.pick(Arena::Fin) {
                None => return (true, Ok(())),
                Some(f) => {
                    let to = if below(&mut self.rng, 3) == 0 {
                        self.pick(Arena::Fin).map(ix)
                    } else {
                        self.partner(ix(f), false)
                    };
                    let edge = self.edge();
                    match to {
                        None => return (true, Ok(())),
                        Some(t)
                            if self.is_dead(Arena::Fin, f) || self.is_dead(Arena::Fin, t.row()) =>
                        {
                            let r = self.euler.mekr(ix(f), t, edge).map(|_| ());
                            assert!(matches!(r, Err(TopoError::Euler(Refusal::Dead { .. }))));
                            (false, r)
                        }
                        Some(t) => {
                            let ok = self.trial.mekr(
                                use_of(&self.euler, ix(f)),
                                use_of(&self.euler, t),
                                edge.name,
                            );
                            (ok, self.euler.mekr(ix(f), t, edge).map(|_| ()))
                        }
                    }
                }
            },
            8 => match self.pick(Arena::Fin) {
                None => return (true, Ok(())),
                Some(f) => self.checked(Arena::Fin, f, |run| {
                    (
                        run.trial.kemr(use_of(&run.euler, ix(f))),
                        run.euler.kemr(ix(f)).map(|_| ()),
                    )
                }),
            },
            9 => match (self.pick(Arena::Face), self.pick(Arena::Loop)) {
                (Some(k), Some(l)) => {
                    if self.is_dead(Arena::Face, k) || self.is_dead(Arena::Loop, l) {
                        let r = self.euler.kfmrh(ix(k), ix(l));
                        assert!(matches!(r, Err(TopoError::Euler(Refusal::Dead { .. }))));
                        (false, r)
                    } else {
                        let first = self
                            .euler
                            .arenas()
                            .loops
                            .get(ix(l))
                            .unwrap()
                            .first_fin
                            .unwrap();
                        let ok = self.trial.kfmrh(
                            self.euler.names().faces[k as usize],
                            use_of(&self.euler, first),
                        );
                        (ok, self.euler.kfmrh(ix(k), ix(l)))
                    }
                }
                _ => return (true, Ok(())),
            },
            10 => match self.pick(Arena::Loop) {
                None => return (true, Ok(())),
                Some(l) => {
                    let face = self.face();
                    self.checked(Arena::Loop, l, |run| {
                        let first = run
                            .euler
                            .arenas()
                            .loops
                            .get(ix(l))
                            .unwrap()
                            .first_fin
                            .unwrap();
                        let ok = run.trial.mfkrh(use_of(&run.euler, first), face.name);
                        (ok, run.euler.mfkrh(ix(l), face).map(|_| ()))
                    })
                }
            },
            11 => match self.pick(Arena::Edge) {
                None => return (true, Ok(())),
                Some(x) => self.checked(Arena::Edge, x, |run| {
                    let name = run.euler.names().edges[x as usize];
                    (run.trial.kill_wire(name), run.euler.kill_wire(ix(x)))
                }),
            },
            12 => self.regions(),
            _ => return (true, Ok(())),
        };
        if result.0 {
            self.model = self.trial.clone();
        }
        result
    }

    /// A refusal for a dead row of `arena`, from the operator that takes one row of it.
    fn euler_dead(&mut self, arena: Arena, row: u32) -> Result<(), TopoError> {
        let (edge, vertex, face) = (self.edge(), self.vertex(), self.face());
        let r = match arena {
            Arena::Fin => match below(&mut self.rng, 4) {
                0 => self.euler.mev(ix(row), edge, vertex).map(|_| ()),
                1 => self.euler.kef(ix(row)),
                2 => self.euler.kemr(ix(row)).map(|_| ()),
                _ => self.euler.mef(ix(row), ix(row), edge, face).map(|_| ()),
            },
            Arena::Edge => match below(&mut self.rng, 2) {
                0 => self.euler.kev(ix(row)),
                _ => self.euler.kill_wire(ix(row)),
            },
            Arena::Loop => self.euler.mfkrh(ix(row), face).map(|_| ()),
            _ => panic!("no operator here takes a dead {arena}"),
        };
        assert_eq!(
            r,
            Err(TopoError::Euler(Refusal::Dead { arena, row })),
            "a dead {arena} {row}"
        );
        r
    }

    /// A step on regions: make one, kill one, or move a shell.
    fn regions(&mut self) -> (bool, Result<(), TopoError>) {
        match below(&mut self.rng, 3) {
            0 => {
                let (name, kind) = (
                    self.name(),
                    if self.rng.coin() {
                        RegionKind::Solid
                    } else {
                        RegionKind::Void
                    },
                );
                self.trial.regions.insert(name, kind);
                (true, self.euler.make_region(kind, name).map(|_| ()))
            }
            1 => {
                let Some(r) = self.pick(Arena::Region) else {
                    return (true, Ok(()));
                };
                let name = self.euler.names().regions[r as usize];
                if name == self.outside {
                    return (true, Ok(()));
                }
                if self.is_dead(Arena::Region, r) {
                    let got = self.euler.kill_region(ix(r));
                    assert!(matches!(got, Err(TopoError::Euler(Refusal::Dead { .. }))));
                    return (false, got);
                }
                (self.trial.kill_region(name), self.euler.kill_region(ix(r)))
            }
            _ => match (self.pick(Arena::Shell), self.pick(Arena::Region)) {
                (Some(s), Some(r)) => {
                    if self.is_dead(Arena::Shell, s) || self.is_dead(Arena::Region, r) {
                        let got = self.euler.move_shell(ix(s), ix(r));
                        assert!(matches!(got, Err(TopoError::Euler(Refusal::Dead { .. }))));
                        return (false, got);
                    }
                    let a = self.euler.arenas();
                    let face = live(&self.euler, Arena::Face)
                        .into_iter()
                        .find(|&f| a.faces.get(ix(f)).unwrap().shell == ix(s))
                        .unwrap();
                    let ok = self.trial.move_shell(
                        self.euler.names().faces[face as usize],
                        self.euler.names().regions[r as usize],
                    );
                    (ok, self.euler.move_shell(ix(s), ix(r)))
                }
                _ => (true, Ok(())),
            },
        }
    }

    /// Publishes the editor, edits the body again, and checks that publishing that gives the same
    /// body.
    fn round_trip(&mut self) {
        let outside: RegionIx = ix(live(&self.euler, Arena::Region)
            .into_iter()
            .find(|&r| self.euler.names().regions[r as usize] == self.outside)
            .unwrap());
        let body = self
            .euler
            .clone()
            .publish(BodyKind::General, outside, &mut meter())
            .unwrap();
        let again = Euler::edit(&body).unwrap();
        let state = state_of(&again);
        assert_eq!(state, state_of(&self.euler));
        let republished = again
            .clone()
            .publish(BodyKind::General, body.outside(), &mut meter())
            .unwrap();
        assert_eq!(republished, body);
        self.euler = again;
    }
}

#[test]
fn the_editor_and_the_model_agree_for_100_000_steps() {
    let seed = resolve("geom-topo Euler model", Seed::DEFAULT).unwrap();
    let mut run = Run {
        euler: Euler::new(),
        model: Model::default(),
        rng: SplitMix64::new(seed),
        names: 0,
        outside: NameKey([0; 16]),
        trial: Model::default(),
    };
    run.outside = run.name();
    run.euler
        .make_region(RegionKind::Void, run.outside)
        .unwrap();
    run.model.regions.insert(run.outside, RegionKind::Void);
    let mut applied = [0u32; 2];
    for step in 0..STEPS {
        let before = run.euler.clone();
        let (ok, got) = run.step();
        match (ok, &got) {
            (true, Ok(())) => applied[1] += 1,
            (false, Err(TopoError::Euler(_))) => {
                assert_eq!(
                    run.euler, before,
                    "step {step}: a refusal changed the editor"
                );
                applied[0] += 1;
            }
            _ => panic!("step {step}: the model says {ok}, the editor {got:?}"),
        }
        assert_eq!(state_of(&run.euler), run.model.state(), "step {step}");
        assert_eq!(run.model.euler(), 0, "step {step}: Euler's formula");
        if step % 97 == 96 {
            run.round_trip();
        }
    }
    // Both kinds of step happened many times.
    assert!(applied.iter().all(|&n| n > STEPS / 10), "{applied:?}");
}
