//! Maximum flow and the cheapest cut, for the rigid-cluster search.
//!
//! What it does: [`Network`] is a directed graph with whole-number capacities. [`Network::max_flow`]
//! pushes the most flow from a source to a sink by Dinic's method, and [`Network::source_side`] then
//! names the nodes the source can still reach, which is the smallest set on the source's side of a
//! minimum cut. The cluster search (`cluster`) reads a densest subgraph off that set.
//!
//! Guarantees:
//!
//! - **Exact.** Capacities and flows are `u64`; every sum is checked, and a capacity that would
//!   overflow is an error, never a wrapped value.
//! - **Deterministic.** Arcs are visited in the order they were added; there is no hashing and no
//!   address-dependent order, so the same network gives the same flow and the same cut on every
//!   machine.
//! - **Iterative.** The level search and the blocking-flow search each keep an explicit stack
//!   counted against the meter, so no network of any depth can overflow the machine's stack
//!   (principle 13).
//!
//! Method: Dinic's blocking-flow algorithm (E. A. Dinic, "Algorithm for solution of a problem of
//! maximum flow in networks with power estimation", Soviet Math. Doklady 11, 1970; see also
//! <https://en.wikipedia.org/wiki/Dinic%27s_algorithm>), with the current-arc pointer that makes
//! each blocking flow O(VE). It is written here rather than taken from a crate because the kernel
//! crates admit no new dependency without the owner's decision (`deny-no-std.toml`), and the rule
//! of the crate is a fixed arc order and a metered, recursion-free search that a general graph
//! library does not promise.
//!
//! Cost: O(V²E) for a flow in the worst case, far less on the shallow networks the cluster search
//! builds; O(V + E) memory. No recursion.

extern crate alloc;

use alloc::vec::Vec;

use geom_run::WorkMeter;

use crate::model::SketchError;

/// The arc list ends.
const NONE: usize = usize::MAX;

/// A capacity no real flow reaches, for arcs that must never be the cut.
pub const INFINITE: u64 = u64::MAX / 4;

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

fn cost(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// A flow network: nodes `0..nodes` and arcs with capacities, each with its reverse arc.
#[derive(Clone, Debug, Default)]
pub struct Network {
    /// The first arc leaving each node, or `NONE`.
    first: Vec<usize>,
    /// The next arc leaving the same node, or `NONE`.
    next: Vec<usize>,
    /// Where each arc goes.
    to: Vec<usize>,
    /// What each arc can still carry.
    left: Vec<u64>,
}

impl Network {
    /// A network of `nodes` nodes and no arcs.
    ///
    /// # Errors
    ///
    /// [`SketchError::OutOfMemory`].
    pub fn new(nodes: usize) -> Result<Network, SketchError> {
        let mut first = Vec::new();
        first
            .try_reserve_exact(nodes)
            .map_err(|_| SketchError::OutOfMemory)?;
        first.resize(nodes, NONE);
        Ok(Network {
            first,
            next: Vec::new(),
            to: Vec::new(),
            left: Vec::new(),
        })
    }

    /// The number of nodes.
    #[must_use]
    pub fn nodes(&self) -> usize {
        self.first.len()
    }

    /// An arc from `from` to `to` that can carry `capacity`, and its reverse with none.
    ///
    /// # Errors
    ///
    /// [`SketchError::Internal`] for a node past the last; [`SketchError::OutOfMemory`].
    pub fn add_arc(&mut self, from: usize, to: usize, capacity: u64) -> Result<(), SketchError> {
        if from >= self.nodes() || to >= self.nodes() {
            return Err(SketchError::Internal);
        }
        let forward = self.to.len();
        let back = forward.checked_add(1).ok_or(SketchError::Internal)?;
        push(&mut self.to, to)?;
        push(&mut self.left, capacity)?;
        push(&mut self.next, get(&self.first, from)?)?;
        *slot(&mut self.first, from)? = forward;
        push(&mut self.to, from)?;
        push(&mut self.left, 0)?;
        push(&mut self.next, get(&self.first, to)?)?;
        *slot(&mut self.first, to)? = back;
        Ok(())
    }

    /// The levels of the residual graph from `source`: the length of the shortest path of arcs with
    /// room, `NONE` where there is none. `true` if `sink` is reached.
    fn levels(
        &self,
        source: usize,
        sink: usize,
        level: &mut [usize],
        queue: &mut Vec<usize>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<bool, SketchError> {
        level.fill(NONE);
        queue.clear();
        *slot(level, source)? = 0;
        push(queue, source)?;
        let mut head = 0;
        while let Some(&u) = queue.get(head) {
            head = head.checked_add(1).ok_or(SketchError::Internal)?;
            let depth = get(level, u)?.checked_add(1).ok_or(SketchError::Internal)?;
            let mut arc = get(&self.first, u)?;
            while arc != NONE {
                meter.charge(1)?;
                let v = get(&self.to, arc)?;
                if get(&self.left, arc)? > 0 && get(level, v)? == NONE {
                    *slot(level, v)? = depth;
                    push(queue, v)?;
                }
                arc = get(&self.next, arc)?;
            }
        }
        Ok(get(level, sink)? != NONE)
    }

    /// One augmenting path of the level graph from `source` to `sink`, saturated, by an explicit
    /// stack with a current-arc pointer per node. The flow it pushed, or 0 if there is no path.
    fn augment(
        &mut self,
        source: usize,
        sink: usize,
        level: &[usize],
        current: &mut [usize],
        path: &mut Vec<usize>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<u64, SketchError> {
        path.clear();
        let mut at = source;
        loop {
            meter.charge(1)?;
            if at == sink {
                let mut push_flow = INFINITE;
                for &arc in path.iter() {
                    push_flow = push_flow.min(get(&self.left, arc)?);
                }
                for &arc in path.iter() {
                    let slot_forward = slot(&mut self.left, arc)?;
                    *slot_forward = slot_forward
                        .checked_sub(push_flow)
                        .ok_or(SketchError::Internal)?;
                    let reverse = slot(&mut self.left, arc ^ 1)?;
                    *reverse = reverse
                        .checked_add(push_flow)
                        .ok_or(SketchError::Internal)?;
                }
                return Ok(push_flow);
            }
            let mut arc = get(current, at)?;
            let mut found = NONE;
            while arc != NONE {
                let v = get(&self.to, arc)?;
                if get(&self.left, arc)? > 0
                    && get(level, v)?
                        == get(level, at)?
                            .checked_add(1)
                            .ok_or(SketchError::Internal)?
                {
                    found = arc;
                    break;
                }
                arc = get(&self.next, arc)?;
                *slot(current, at)? = arc;
            }
            if found == NONE {
                // A dead end: step back and skip the arc that led here.
                let Some(back) = path.pop() else {
                    return Ok(0);
                };
                at = get(&self.to, back ^ 1)?;
                let skip = get(current, at)?;
                if skip != NONE {
                    *slot(current, at)? = get(&self.next, skip)?;
                }
            } else {
                push(path, found)?;
                at = get(&self.to, found)?;
            }
        }
    }

    /// The largest flow from `source` to `sink`, which it leaves in place: the arcs' remaining
    /// capacities are the residual graph [`Network::source_side`] reads.
    ///
    /// # Errors
    ///
    /// [`SketchError::Internal`] for a node past the last or a flow that would overflow;
    /// [`SketchError::OutOfMemory`]; [`SketchError::Stopped`] when the meter stops it.
    pub fn max_flow(
        &mut self,
        source: usize,
        sink: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<u64, SketchError> {
        if source >= self.nodes() || sink >= self.nodes() || source == sink {
            return Err(SketchError::Internal);
        }
        let n = self.nodes();
        let mut level = Vec::new();
        level
            .try_reserve_exact(n)
            .map_err(|_| SketchError::OutOfMemory)?;
        level.resize(n, NONE);
        let mut current = Vec::new();
        current
            .try_reserve_exact(n)
            .map_err(|_| SketchError::OutOfMemory)?;
        current.resize(n, NONE);
        let (mut queue, mut path) = (Vec::new(), Vec::new());
        let mut total = 0_u64;
        while self.levels(source, sink, &mut level, &mut queue, meter)? {
            current.copy_from_slice(&self.first);
            loop {
                let pushed = self.augment(source, sink, &level, &mut current, &mut path, meter)?;
                if pushed == 0 {
                    break;
                }
                total = total.checked_add(pushed).ok_or(SketchError::Internal)?;
            }
        }
        meter.charge(cost(self.to.len()))?;
        Ok(total)
    }

    /// The nodes `source` still reaches through arcs with room, after [`Network::max_flow`]: the
    /// smallest source side of a minimum cut.
    ///
    /// # Errors
    ///
    /// [`SketchError::Internal`] for a source past the last; [`SketchError::OutOfMemory`];
    /// [`SketchError::Stopped`] when the meter stops it.
    pub fn source_side(
        &self,
        source: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<bool>, SketchError> {
        let mut seen = Vec::new();
        seen.try_reserve_exact(self.nodes())
            .map_err(|_| SketchError::OutOfMemory)?;
        seen.resize(self.nodes(), false);
        let mut stack = Vec::new();
        *slot(&mut seen, source)? = true;
        push(&mut stack, source)?;
        while let Some(u) = stack.pop() {
            let mut arc = get(&self.first, u)?;
            while arc != NONE {
                meter.charge(1)?;
                let v = get(&self.to, arc)?;
                if get(&self.left, arc)? > 0 && !get(&seen, v)? {
                    *slot(&mut seen, v)? = true;
                    push(&mut stack, v)?;
                }
                arc = get(&self.next, arc)?;
            }
        }
        Ok(seen)
    }
}

#[cfg(test)]
mod tests {
    use geom_run::{Budget, WorkMeter};

    use super::{INFINITE, Network};

    fn meter() -> WorkMeter<'static> {
        WorkMeter::new(Budget::UNLIMITED)
    }

    #[test]
    fn the_textbook_network_carries_23() {
        // CLRS figure 26.1: source 0, sink 5.
        let mut net = Network::new(6).unwrap();
        for (a, b, c) in [
            (0, 1, 16),
            (0, 2, 13),
            (1, 2, 10),
            (2, 1, 4),
            (1, 3, 12),
            (3, 2, 9),
            (2, 4, 14),
            (4, 3, 7),
            (3, 5, 20),
            (4, 5, 4),
        ] {
            net.add_arc(a, b, c).unwrap();
        }
        assert_eq!(net.max_flow(0, 5, &mut meter()), Ok(23));
    }

    #[test]
    fn the_source_side_is_the_smallest_minimum_cut() {
        // 0 -> 1 (3), 1 -> 2 (3), 2 -> 3 (5): every arc of capacity 3 is a minimum cut, and the
        // source side of the smallest one is {0}... only if 0 -> 1 saturates first.
        let mut net = Network::new(4).unwrap();
        net.add_arc(0, 1, 3).unwrap();
        net.add_arc(1, 2, 3).unwrap();
        net.add_arc(2, 3, 5).unwrap();
        assert_eq!(net.max_flow(0, 3, &mut meter()), Ok(3));
        let side = net.source_side(0, &mut meter()).unwrap();
        assert_eq!(side, [true, false, false, false]);
    }

    #[test]
    fn no_path_is_no_flow_and_infinite_arcs_are_never_cut() {
        let mut net = Network::new(4).unwrap();
        net.add_arc(0, 1, INFINITE).unwrap();
        net.add_arc(2, 3, 7).unwrap();
        assert_eq!(net.max_flow(0, 3, &mut meter()), Ok(0));
        assert_eq!(
            net.source_side(0, &mut meter()).unwrap(),
            [true, true, false, false]
        );
    }

    #[test]
    fn a_node_past_the_last_is_an_error() {
        let mut net = Network::new(2).unwrap();
        assert!(net.add_arc(0, 2, 1).is_err());
        assert!(net.max_flow(0, 0, &mut meter()).is_err());
    }
}
