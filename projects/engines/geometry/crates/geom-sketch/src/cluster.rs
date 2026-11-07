//! Rigid clusters: the Hoffmann–Lomonosov–Sitharam decomposition of a constraint graph, for any
//! number of rigid motions D.
//!
//! What it does: [`Problem`] is a geometric constraint graph. Its [`Node`]s are entities weighted by
//! their freedoms, its [`Link`]s are constraints weighted by their equations (a link may join any
//! number of nodes), and fixed nodes form the ground. [`plan`] finds clusters that are rigid by
//! themselves, replaces each by one node of weight D and repeats, which is the Modified Frontier
//! method, and returns the [`Plan`]: the clusters in the order to solve them, each over its own
//! children, and what is left at the top.
//!
//! D is the number of freedoms of a rigid body: 3 in the plane, 6 in space. The code never reads the
//! dimension; a set of nodes is **dense** when its constraints leave it at most D freedoms
//! (`w(S) - w(E[S]) <= D`, `S` having two nodes or more and a constraint among them), and a cluster
//! is a **minimal** dense set: one with no smaller dense set inside it.
//!
//! Guarantees:
//!
//! - **Canonical.** Nodes and links are ordered by their persistent keys before anything else, and
//!   every tie goes to the lower position in that order, so shuffling the input (the order of
//!   `nodes` and `links`, and the order of a link's members) gives the same plan, written in keys.
//! - **Exact.** Weights are whole numbers, every sum is checked, and density is an integer
//!   comparison; no value is read, so the plan is the same on every machine.
//! - **Complete.** Each node belongs to at most one cluster of a level; a cluster is minimal; when
//!   [`plan`] returns, no dense set of two nodes or more with a constraint among them is left in a
//!   component within the size limit, so the top-level nodes are what no rigid body explains.
//! - **The ground.** The fixed nodes form one ground cluster of weight D, made first. A cluster that
//!   absorbs it is `grounded`: it is placed in the sketch's frame and keeps no freedom. Any other
//!   cluster keeps D freedoms.
//! - **Iterative.** Every search keeps an explicit stack or queue charged to the meter, so no
//!   sketch can overflow the machine's stack (principle 13).
//! - **Size limit.** A component of more than `limit` nodes is not searched and is reported in
//!   [`Plan::too_large`], by its lowest key (15.5; until measured the limit is [`SIZE_LIMIT`]).
//!
//! Method: Hoffmann, Lomonosov and Sitharam, "Finding solvable subsets of constraint graphs"
//! (CP'97, doi:10.1007/BFb0017460) and "Decomposition plans for geometric constraint systems, Part
//! I: Performance measures for CAD" (J. Symbolic Computation 31, 2001, doi:10.1006/jsco.2000.0402),
//! Part II: New algorithms (doi:10.1006/jsco.2000.0403). Dense subsets are found as the heaviest
//! closure of a project-selection network (profit for each constraint, cost for each node, a
//! constraint needing all its nodes), by a minimum cut (J.-C. Picard, "Maximal closure of a graph
//! and applications to combinatorial problems", Management Science 22(11), 1976,
//! doi:10.1287/mnsc.22.11.1268, and the flow of `flow`), the construction the authors use; a dense
//! set is reduced to a minimal one by removing a node and searching what is left, O(n^2 (m + n))
//! in all. The search grows from each constraint outwards, a ring of neighbours at a time, so the
//! small clusters of an interactive sketch cost little.
//! Consulted: fiksi, Apache-2.0 OR MIT,
//!   <https://github.com/endoli/fiksi/blob/ff4850546738fb82e95ff27c4db63e924d938473/fiksi/src/analyze/graph/recursive_assembly.rs>,
//!   for what the decomposition must return and where its exhaustive search for a minimum subgraph
//!   is slow. No code copied, and its randomly seeded hash sets are not used here.
//!
//! Cost: one cluster costs a few maximum flows on the neighbourhood it grows through; a family of
//! small rigid bodies costs time linear in its size; the worst case is the authors' O(n^2 (m + n)).
//! No recursion.

extern crate alloc;

use alloc::vec::Vec;

use geom_run::WorkMeter;

use crate::flow::{INFINITE, Network};
use crate::model::SketchError;

/// The component size above which the cluster search is skipped until it is measured (15.5).
pub const SIZE_LIMIT: usize = 2_000;

/// An entity of the geometric graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    /// The entity's persistent key; every tie in the search is broken by it.
    pub key: u64,
    /// Its freedoms: 2 for a point in the plane, 1 for a scalar, and so on.
    pub weight: u32,
    /// Whether it is fixed in the sketch's frame: fixed geometry, a reference, the frame itself.
    pub fixed: bool,
}

/// A constraint of the geometric graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// The constraint's persistent key.
    pub key: u64,
    /// The nodes it relates, as positions in [`Problem::nodes`], in any order.
    pub members: Vec<usize>,
    /// Its equations: 1 for a distance, 2 for a coincidence of points in the plane.
    pub weight: u32,
}

/// A geometric constraint graph and the freedoms of a rigid body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// D: the freedoms of a rigid body, 3 in the plane and 6 in space.
    pub motions: u32,
    /// The entities.
    pub nodes: Vec<Node>,
    /// The constraints.
    pub links: Vec<Link>,
}

/// What a cluster is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Child {
    /// An entity, by its key.
    Node(u64),
    /// An earlier cluster of the plan, by its position in [`Plan::clusters`].
    Cluster(usize),
}

/// A rigid cluster: a minimal dense set at the moment it was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cluster {
    /// What it joins, ordered by the lowest key each holds.
    pub children: Vec<Child>,
    /// The keys of the constraints among its children, which it consumes, ascending.
    pub links: Vec<u64>,
    /// Whether it holds the ground, and so has no freedom.
    pub grounded: bool,
    /// How many of its equations are redundant: D less its freedoms, 0 when it is exactly rigid.
    pub redundant: u64,
}

/// The decomposition: clusters in the order to solve them, and what no cluster explains.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The clusters in creation order; a cluster's children come earlier. The ground cluster, if
    /// the problem has fixed nodes, is first.
    pub clusters: Vec<Cluster>,
    /// What is left at the top, ordered by lowest key: entities and clusters no larger cluster
    /// holds.
    pub top: Vec<Child>,
    /// The lowest key of every component past the size limit, whose clusters were not searched.
    pub too_large: Vec<u64>,
}

fn get<T: Copy>(list: &[T], i: usize) -> Result<T, SketchError> {
    list.get(i).copied().ok_or(SketchError::Internal)
}

fn slot<T>(list: &mut [T], i: usize) -> Result<&mut T, SketchError> {
    list.get_mut(i).ok_or(SketchError::Internal)
}

fn push<T>(list: &mut Vec<T>, value: T) -> Result<(), SketchError> {
    list.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
    list.push(value);
    Ok(())
}

fn add(a: u64, b: u64) -> Result<u64, SketchError> {
    a.checked_add(b).ok_or(SketchError::Internal)
}

fn next(i: usize) -> Result<usize, SketchError> {
    i.checked_add(1).ok_or(SketchError::Internal)
}

fn cost(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// The working graph: nodes and links by position, with the clusters made so far already
/// contracted into single nodes.
struct Work {
    motions: u64,
    weight: Vec<u64>,
    key: Vec<u64>,
    child: Vec<Child>,
    ground: Vec<bool>,
    blocked: Vec<bool>,
    alive: Vec<bool>,
    members: Vec<Vec<usize>>,
    link_weight: Vec<u64>,
    link_key: Vec<u64>,
    link_alive: Vec<bool>,
    incident: Vec<Vec<usize>>,
    /// Which nodes the last `mark` named: `stamp[v] == epoch`.
    stamp: Vec<u64>,
    epoch: u64,
}

impl Work {
    fn mark(&mut self, set: &[usize]) -> Result<(), SketchError> {
        self.epoch = add(self.epoch, 1)?;
        for &v in set {
            *slot(&mut self.stamp, v)? = self.epoch;
        }
        Ok(())
    }

    fn marked(&self, v: usize) -> Result<bool, SketchError> {
        Ok(get(&self.stamp, v)? == self.epoch)
    }

    fn inside(&self, l: usize) -> Result<bool, SketchError> {
        for &v in self.members.get(l).ok_or(SketchError::Internal)? {
            if !self.marked(v)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// The links wholly inside the marked set, by their lowest member, each once.
    fn links_inside(
        &self,
        set: &[usize],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<usize>, SketchError> {
        let mut found = Vec::new();
        for &v in set {
            let list = self.incident.get(v).ok_or(SketchError::Internal)?;
            meter.charge(cost(list.len()))?;
            for &l in list {
                let first = self
                    .members
                    .get(l)
                    .and_then(|m| m.first())
                    .copied()
                    .ok_or(SketchError::Internal)?;
                if get(&self.link_alive, l)? && first == v && self.inside(l)? {
                    push(&mut found, l)?;
                }
            }
        }
        found.sort_unstable();
        Ok(found)
    }

    /// Freedoms of the set `w(S) - w(E[S])`, which can be negative: an over-constrained set.
    fn freedoms(&mut self, set: &[usize], meter: &mut WorkMeter<'_>) -> Result<i128, SketchError> {
        self.mark(set)?;
        let mut nodes = 0_u64;
        for &v in set {
            nodes = add(nodes, get(&self.weight, v)?)?;
        }
        let mut equations = 0_u64;
        for l in self.links_inside(set, meter)? {
            equations = add(equations, get(&self.link_weight, l)?)?;
        }
        i128::from(nodes)
            .checked_sub(i128::from(equations))
            .ok_or(SketchError::Internal)
    }

    /// A dense set of the nodes `set` that holds the link `seed`, if one exists: the heaviest
    /// closure of the project-selection network on `set`, with `seed` forced in.
    fn dense_within(
        &mut self,
        set: &[usize],
        seed: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Vec<usize>>, SketchError> {
        self.mark(set)?;
        let links = self.links_inside(set, meter)?;
        // Nodes: 0 source, 1 sink, then each link, then each node of `set`.
        let base = links.len().checked_add(2).ok_or(SketchError::Internal)?;
        let total = base.checked_add(set.len()).ok_or(SketchError::Internal)?;
        let mut net = Network::new(total)?;
        for (i, &l) in links.iter().enumerate() {
            let at = next(next(i)?)?;
            let profit = if l == seed {
                INFINITE
            } else {
                get(&self.link_weight, l)?
            };
            net.add_arc(0, at, profit)?;
            for &v in self.members.get(l).ok_or(SketchError::Internal)? {
                let p = set.binary_search(&v).map_err(|_| SketchError::Internal)?;
                net.add_arc(
                    at,
                    base.checked_add(p).ok_or(SketchError::Internal)?,
                    INFINITE,
                )?;
            }
        }
        for (p, &v) in set.iter().enumerate() {
            net.add_arc(
                base.checked_add(p).ok_or(SketchError::Internal)?,
                1,
                get(&self.weight, v)?,
            )?;
        }
        net.max_flow(0, 1, meter)?;
        let side = net.source_side(0, meter)?;
        let mut chosen = Vec::new();
        for (p, &v) in set.iter().enumerate() {
            if get(&side, base.checked_add(p).ok_or(SketchError::Internal)?)? {
                push(&mut chosen, v)?;
            }
        }
        if chosen.len() < 2 {
            return Ok(None);
        }
        let free = self.freedoms(&chosen, meter)?;
        let has_link = !self.links_inside(&chosen, meter)?.is_empty();
        Ok((has_link && free <= i128::from(self.motions)).then_some(chosen))
    }

    /// Any dense set inside `set` (sorted), trying each link in turn as the seed.
    fn dense_inside(
        &mut self,
        set: &[usize],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Vec<usize>>, SketchError> {
        self.mark(set)?;
        for l in self.links_inside(set, meter)? {
            if let Some(found) = self.dense_within(set, l, meter)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    /// A minimal dense set inside the dense `set`: while removing some node leaves a dense set,
    /// take that one.
    fn minimal(
        &mut self,
        mut set: Vec<usize>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<usize>, SketchError> {
        'again: loop {
            for at in 0..set.len() {
                if set.len() < 3 {
                    return Ok(set);
                }
                let mut rest = set.clone();
                rest.remove(at);
                if let Some(smaller) = self.dense_inside(&rest, meter)? {
                    set = smaller;
                    continue 'again;
                }
            }
            return Ok(set);
        }
    }

    /// The nodes within `hops` links of `from`, ascending.
    fn region(
        &mut self,
        from: &[usize],
        hops: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<usize>, SketchError> {
        self.mark(from)?;
        let mut all: Vec<usize> = from.to_vec();
        let mut frontier: Vec<usize> = from.to_vec();
        for _ in 0..hops {
            let mut fresh = Vec::new();
            for &v in &frontier {
                let list = self.incident.get(v).ok_or(SketchError::Internal)?;
                meter.charge(cost(list.len()))?;
                for &l in list {
                    if !get(&self.link_alive, l)? {
                        continue;
                    }
                    for &u in self.members.get(l).ok_or(SketchError::Internal)? {
                        push(&mut fresh, u)?;
                    }
                }
            }
            fresh.sort_unstable();
            fresh.dedup();
            let mut kept = Vec::new();
            for &u in &fresh {
                if !self.marked(u)? {
                    *slot(&mut self.stamp, u)? = self.epoch;
                    push(&mut kept, u)?;
                    push(&mut all, u)?;
                }
            }
            if kept.is_empty() {
                break;
            }
            frontier = kept;
        }
        all.sort_unstable();
        Ok(all)
    }

    /// A cluster that holds the link `seed`, if one is found: its two nodes when they are rigid by
    /// themselves, else the heaviest dense closure in a growing neighbourhood, made minimal.
    fn find(
        &mut self,
        seed: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Vec<usize>>, SketchError> {
        let members = self.members.get(seed).ok_or(SketchError::Internal)?.clone();
        if members.len() == 2 {
            let free = self.freedoms(&members, meter)?;
            if free <= i128::from(self.motions) {
                return Ok(Some(members));
            }
        }
        let mut hops = 1_usize;
        let mut before = 0_usize;
        loop {
            let region = self.region(&members, hops, meter)?;
            if let Some(dense) = self.dense_within(&region, seed, meter)? {
                return self.minimal(dense, meter).map(Some);
            }
            if region.len() == before {
                return Ok(None);
            }
            before = region.len();
            hops = hops.checked_mul(2).ok_or(SketchError::Internal)?;
        }
    }

    /// Replaces `set` by one node of weight D, consuming the links among it and moving the rest to
    /// the new node. The new node's position.
    fn contract(
        &mut self,
        set: &[usize],
        grounded: bool,
        clusters: &mut Vec<Cluster>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<usize, SketchError> {
        let free = self.freedoms(set, meter)?;
        let id = clusters.len();
        let z = self.weight.len();
        let mut touched = Vec::new();
        let mut children = Vec::new();
        let mut any_ground = grounded;
        let mut any_blocked = false;
        let mut lowest = u64::MAX;
        for &v in set {
            any_ground |= get(&self.ground, v)?;
            any_blocked |= get(&self.blocked, v)?;
            lowest = lowest.min(get(&self.key, v)?);
            push(&mut children, (get(&self.key, v)?, get(&self.child, v)?))?;
            for &l in self.incident.get(v).ok_or(SketchError::Internal)? {
                if get(&self.link_alive, l)? {
                    push(&mut touched, l)?;
                }
            }
        }
        children.sort_unstable();
        touched.sort_unstable();
        touched.dedup();
        meter.charge(cost(touched.len()))?;
        let mut consumed = Vec::new();
        let mut kept = Vec::new();
        for &l in &touched {
            if self.inside(l)? {
                push(&mut consumed, get(&self.link_key, l)?)?;
                *slot(&mut self.link_alive, l)? = false;
            } else {
                let mut now = Vec::new();
                for &m in self.members.get(l).ok_or(SketchError::Internal)? {
                    if !self.marked(m)? {
                        push(&mut now, m)?;
                    }
                }
                push(&mut now, z)?;
                now.sort_unstable();
                *self.members.get_mut(l).ok_or(SketchError::Internal)? = now;
                push(&mut kept, l)?;
            }
        }
        consumed.sort_unstable();
        for &v in set {
            *slot(&mut self.alive, v)? = false;
        }
        push(&mut self.weight, self.motions)?;
        push(&mut self.key, lowest)?;
        push(&mut self.child, Child::Cluster(id))?;
        push(&mut self.ground, any_ground)?;
        push(&mut self.blocked, any_blocked)?;
        push(&mut self.alive, true)?;
        push(&mut self.incident, kept)?;
        push(&mut self.stamp, 0)?;
        let redundant = if any_ground && grounded {
            0
        } else {
            i128::from(self.motions)
                .checked_sub(free)
                .and_then(|r| u64::try_from(r).ok())
                .unwrap_or(0)
        };
        push(
            clusters,
            Cluster {
                children: children.into_iter().map(|(_, c)| c).collect(),
                links: consumed,
                grounded: any_ground,
                redundant,
            },
        )?;
        Ok(z)
    }
}

/// For every node, the lowest position of its component, by union–find over the links, and the
/// size of that component.
fn components(
    nodes: usize,
    members: &[Vec<usize>],
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<usize>, Vec<usize>), SketchError> {
    let mut parent: Vec<usize> = (0..nodes).collect();
    for m in members {
        meter.charge(cost(m.len()))?;
        let Some(&head) = m.first() else { continue };
        for &other in m {
            let (mut a, mut b) = (head, other);
            while get(&parent, a)? != a {
                let up = get(&parent, get(&parent, a)?)?;
                *slot(&mut parent, a)? = up;
                a = up;
            }
            while get(&parent, b)? != b {
                let up = get(&parent, get(&parent, b)?)?;
                *slot(&mut parent, b)? = up;
                b = up;
            }
            if a != b {
                let (low, high) = (a.min(b), a.max(b));
                *slot(&mut parent, high)? = low;
            }
        }
    }
    let mut size = alloc::vec![0_usize; nodes];
    let mut root_of = alloc::vec![0_usize; nodes];
    for v in 0..nodes {
        let mut r = v;
        while get(&parent, r)? != r {
            r = get(&parent, r)?;
        }
        *slot(&mut root_of, v)? = r;
        let count = slot(&mut size, r)?;
        *count = next(*count)?;
    }
    Ok((root_of, size))
}

/// The working graph of `problem` in canonical order, and the positions of its fixed nodes.
fn build(problem: &Problem) -> Result<(Work, Vec<usize>), SketchError> {
    // Canonical order: nodes and links by key.
    let mut order: Vec<usize> = (0..problem.nodes.len()).collect();
    order.sort_unstable_by_key(|&i| problem.nodes.get(i).map(|n| n.key));
    let mut rank = alloc::vec![0_usize; order.len()];
    for (new, &old) in order.iter().enumerate() {
        *slot(&mut rank, old)? = new;
    }
    let mut link_order: Vec<usize> = (0..problem.links.len()).collect();
    link_order.sort_unstable_by_key(|&i| problem.links.get(i).map(|l| l.key));
    let mut w = Work {
        motions: u64::from(problem.motions),
        weight: Vec::new(),
        key: Vec::new(),
        child: Vec::new(),
        ground: Vec::new(),
        blocked: Vec::new(),
        alive: Vec::new(),
        members: Vec::new(),
        link_weight: Vec::new(),
        link_key: Vec::new(),
        link_alive: Vec::new(),
        incident: Vec::new(),
        stamp: Vec::new(),
        epoch: 0,
    };
    let mut fixed = Vec::new();
    for (new, &old) in order.iter().enumerate() {
        let n = problem.nodes.get(old).ok_or(SketchError::Internal)?;
        if let Some(before) = new.checked_sub(1)
            && get(&w.key, before)? == n.key
        {
            return Err(SketchError::Internal);
        }
        push(&mut w.weight, u64::from(n.weight))?;
        push(&mut w.key, n.key)?;
        push(&mut w.child, Child::Node(n.key))?;
        push(&mut w.ground, false)?;
        push(&mut w.blocked, false)?;
        push(&mut w.alive, true)?;
        push(&mut w.incident, Vec::new())?;
        push(&mut w.stamp, 0)?;
        if n.fixed {
            push(&mut fixed, new)?;
        }
    }
    for (at, &old) in link_order.iter().enumerate() {
        let l = problem.links.get(old).ok_or(SketchError::Internal)?;
        let mut m = Vec::new();
        for &v in &l.members {
            push(&mut m, get(&rank, v)?)?;
        }
        m.sort_unstable();
        m.dedup();
        if m.is_empty() {
            return Err(SketchError::Internal);
        }
        if let Some(before) = at.checked_sub(1)
            && get(&w.link_key, before)? == l.key
        {
            return Err(SketchError::Internal);
        }
        for &v in &m {
            push(slot(&mut w.incident, v)?, at)?;
        }
        push(&mut w.members, m)?;
        push(&mut w.link_weight, u64::from(l.weight))?;
        push(&mut w.link_key, l.key)?;
        push(&mut w.link_alive, true)?;
    }
    Ok((w, fixed))
}

/// Decomposes `problem` into rigid clusters.
///
/// # Errors
///
/// [`SketchError::Internal`] for a member past the last node, a link with no member, two nodes or
/// two links with one key, or a weight sum that overflows; [`SketchError::OutOfMemory`];
/// [`SketchError::Stopped`] when the meter stops it.
pub fn plan(
    problem: &Problem,
    limit: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Plan, SketchError> {
    let (mut w, fixed) = build(problem)?;
    let (root_of, size) = components(problem.nodes.len(), &w.members, meter)?;
    let mut too_large = Vec::new();
    for v in 0..problem.nodes.len() {
        let root = get(&root_of, v)?;
        if get(&size, root)? > limit {
            *slot(&mut w.blocked, v)? = true;
            if root == v {
                push(&mut too_large, get(&w.key, v)?)?;
            }
        }
    }
    let mut clusters = Vec::new();
    if !fixed.is_empty() {
        w.contract(&fixed, true, &mut clusters, meter)?;
    }
    // The queue of links to try as seeds, in key order, refilled with the links of each new cluster.
    let mut queue: Vec<usize> = (0..w.members.len()).collect();
    let mut head = 0;
    while let Some(&seed) = queue.get(head) {
        head = next(head)?;
        meter.charge(1)?;
        if !get(&w.link_alive, seed)? {
            continue;
        }
        let first = w
            .members
            .get(seed)
            .and_then(|m| m.first())
            .copied()
            .ok_or(SketchError::Internal)?;
        if get(&w.blocked, first)? {
            continue;
        }
        if let Some(set) = w.find(seed, meter)? {
            let z = w.contract(&set, false, &mut clusters, meter)?;
            let touched = w.incident.get(z).ok_or(SketchError::Internal)?.clone();
            for l in touched {
                push(&mut queue, l)?;
            }
        }
    }
    let mut top = Vec::new();
    let mut tops: Vec<(u64, Child)> = Vec::new();
    for v in 0..w.weight.len() {
        if get(&w.alive, v)? {
            push(&mut tops, (get(&w.key, v)?, get(&w.child, v)?))?;
        }
    }
    tops.sort_unstable();
    for (_, c) in tops {
        push(&mut top, c)?;
    }
    too_large.sort_unstable();
    Ok(Plan {
        clusters,
        top,
        too_large,
    })
}
