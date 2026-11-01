//! Shared network search: edge-based shortest paths for any program.
//!
//! A network of nodes and directed arcs, with a turn table: one entry for every pair of an
//! arriving arc and an arc leaving the same node, holding a penalty (infinity bans the turn).
//! Searches put their labels on arcs, not nodes, so a path can pay for its turns and avoid
//! banned ones; nodes marked as stops are reached but never passed through (zone centroids in a
//! traffic model, doors or lifts in a building). The same search serves road traffic, public
//! transport and people walking.
//!
//! Port of the Spare Construct transport prototype's engine.js shortest paths (workspace, spt,
//! pathTo, dijkstra, trace) and the turn table of its buildTurns, operation for operation: the
//! same binary heap ([`Heap`], util.js `U.Heap`), the same relaxation order and the same strict
//! comparisons, so equal-cost ties break exactly as the prototype breaks them. Arrays keep the
//! prototype's integer types and fill values (-1 for "none").

use crate::heap::Heap;
use std::ops::Range;

/// A directed network in compressed rows: the arcs leaving each node, grouped by node, each group in arc order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Network {
    /// Number of nodes.
    pub n: usize,
    /// Number of arcs.
    pub m: usize,
    /// Row offsets (n + 1 entries): the arcs leaving node v are `out[head[v]..head[v + 1]]`.
    pub head: Vec<i32>,
    /// Arc indices grouped by start node.
    pub out: Vec<i32>,
    /// End node of each arc.
    pub to: Vec<i32>,
}

impl Network {
    /// Builds the rows from each arc's start and end node; arcs keep their indices.
    pub fn new(n: usize, from: &[usize], to: &[usize]) -> Network {
        assert_eq!(from.len(), to.len(), "one start and one end node per arc");
        let m = from.len();
        let mut head = vec![0i32; n + 1];
        for &f in from {
            head[f + 1] += 1;
        }
        for i in 0..n {
            head[i + 1] += head[i];
        }
        let mut fill: Vec<i32> = head[..n].to_vec();
        let mut out = vec![0i32; m];
        for (i, &f) in from.iter().enumerate() {
            out[fill[f] as usize] = i as i32;
            fill[f] += 1;
        }
        Network {
            n,
            m,
            head,
            out,
            to: to.iter().map(|&t| t as i32).collect(),
        }
    }
    /// Positions in `out` of the arcs leaving node v.
    pub fn out_range(&self, v: usize) -> Range<usize> {
        self.head[v] as usize..self.head[v + 1] as usize
    }
}

/// The turn table, aligned with the out-lists: the turn from arc a into `out[k]` (k in node `to[a]`'s row) is `off[a] + k - head[to[a]]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Turns {
    /// Offset of each arc's turns (m + 1 entries).
    pub off: Vec<i32>,
    /// Penalty of each turn; infinity bans it.
    pub pen: Vec<f64>,
    /// The arc each turn enters.
    pub to: Vec<i32>,
}

impl Turns {
    /// Builds the table, asking pen(a, b) for every turn from arc a into an arc b leaving a's end node (infinity bans it).
    pub fn new(net: &Network, mut pen: impl FnMut(usize, usize) -> f64) -> Turns {
        let m = net.m;
        let mut off = vec![0i32; m + 1];
        for a in 0..m {
            let n = net.to[a] as usize;
            off[a + 1] = off[a] + net.head[n + 1] - net.head[n];
        }
        let nt = off[m] as usize;
        let mut p = vec![0.0f64; nt];
        let mut t_to = vec![0i32; nt];
        for a in 0..m {
            let v = net.to[a] as usize;
            let h = net.head[v];
            let o = off[a] - h;
            for k in net.out_range(v) {
                let t = (o + k as i32) as usize;
                let b = net.out[k];
                t_to[t] = b;
                p[t] = pen(a, b as usize);
            }
        }
        Turns {
            off,
            pen: p,
            to: t_to,
        }
    }
    /// Number of turns.
    pub fn len(&self) -> usize {
        self.pen.len()
    }
    /// True when the table has no turns.
    pub fn is_empty(&self) -> bool {
        self.pen.is_empty()
    }
    /// Indices of the turns out of arc a.
    pub fn of(&self, a: usize) -> Range<usize> {
        self.off[a] as usize..self.off[a + 1] as usize
    }
    /// Index of the turn from arc a into arc b, or -1 when b does not leave a's end node (turnIdx).
    pub fn index(&self, net: &Network, a: usize, b: usize) -> i32 {
        let v = net.to[a] as usize;
        let h = net.head[v];
        for k in net.out_range(v) {
            if net.out[k] == b as i32 {
                return self.off[a] + k as i32 - h;
            }
        }
        -1
    }
    /// True when there is no turn from arc a into arc b, or it is banned (turnBanned).
    pub fn banned(&self, net: &Network, a: usize, b: usize) -> bool {
        let t = self.index(net, a, b);
        t < 0 || self.pen[t as usize] == f64::INFINITY
    }
}

/// A network with its turn table and its stop nodes: everything a search needs besides the arc costs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EdgeGraph {
    /// Nodes and arcs.
    pub net: Network,
    /// Turn penalties and bans.
    pub turns: Turns,
    /// Per node, nonzero for a node a search reaches but never passes through.
    pub stop: Vec<u8>,
}

impl EdgeGraph {
    /// Puts the parts together (stop has one entry per node).
    pub fn new(net: Network, turns: Turns, stop: Vec<u8>) -> EdgeGraph {
        assert_eq!(stop.len(), net.n, "one stop flag per node");
        EdgeGraph { net, turns, stop }
    }

    /// Edge-based Dijkstra from node src (spt): entering arc b from arc a costs the turn penalty plus `cost[b]`.
    ///
    /// Banned turns are skipped and stop nodes are not passed through. from_arc (when >= 0, an arc
    /// arriving at src) makes the first turn obey the same rules, for chained waypoints. The search
    /// ends once node target (when >= 0) is reached. Results are left in the workspace: per arc the
    /// best cost `dist`, the arc before it `pred` and the turn used `pt`; per node the best arrival
    /// cost `nd` and the arc it arrives on `na`.
    pub fn spt(&self, cost: &[f64], src: usize, w: &mut Workspace, target: i32, from_arc: i32) {
        let net = &self.net;
        let (head, out, to) = (&net.head, &net.out, &net.to);
        let (t_off, tpen) = (&self.turns.off, &self.turns.pen);
        w.dist.fill(f64::INFINITY);
        w.pred.fill(-1);
        w.pt.fill(-1);
        w.nd.fill(f64::INFINITY);
        w.na.fill(-1);
        w.heap.clear();
        w.src = src as i32;
        w.nd[src] = 0.0;
        let h0 = head[src];
        for k in net.out_range(src) {
            let p = if from_arc >= 0 {
                tpen[(t_off[from_arc as usize] + k as i32 - h0) as usize]
            } else {
                0.0
            };
            if p == f64::INFINITY {
                continue;
            }
            let b = out[k] as usize;
            let d = p + cost[b];
            if d < w.dist[b] {
                w.dist[b] = d;
                w.heap.push(d, b as i32);
            }
        }
        while !w.heap.is_empty() {
            let a = w.heap.pop() as usize;
            let da = w.heap.top;
            if da > w.dist[a] {
                continue;
            }
            let v = to[a] as usize;
            if da < w.nd[v] {
                w.nd[v] = da;
                w.na[v] = a as i32;
            }
            if v as i32 == target {
                break;
            }
            if self.stop[v] != 0 {
                continue;
            }
            let h = head[v];
            let o = t_off[a] - h;
            for k in net.out_range(v) {
                let t = (o + k as i32) as usize;
                let p = tpen[t];
                if p == f64::INFINITY {
                    continue;
                }
                let b = out[k] as usize;
                let d = da + p + cost[b];
                if d < w.dist[b] {
                    w.dist[b] = d;
                    w.pred[b] = a as i32;
                    w.pt[b] = t as i32;
                    w.heap.push(d, b as i32);
                }
            }
        }
    }
}

/// Scratch arrays for edge-based searches, reusable from one search to the next (workspace).
#[derive(Clone, Debug, Default)]
pub struct Workspace {
    /// Best cost to the end of each arc.
    pub dist: Vec<f64>,
    /// Arc before each arc on its best path (-1 for none).
    pub pred: Vec<i32>,
    /// Turn used to enter each arc (-1 for none).
    pub pt: Vec<i32>,
    /// Best arrival cost at each node.
    pub nd: Vec<f64>,
    /// Arc each node is best reached by (-1 for none).
    pub na: Vec<i32>,
    /// The search's heap.
    pub heap: Heap,
    /// Source node of the last search (-1 before any).
    pub src: i32,
}

impl Workspace {
    /// Arrays sized for a network.
    pub fn new(net: &Network) -> Workspace {
        Workspace {
            dist: vec![0.0; net.m],
            pred: vec![0; net.m],
            pt: vec![0; net.m],
            nd: vec![0.0; net.n],
            na: vec![0; net.n],
            heap: Heap::with_capacity(net.m + 16),
            src: -1,
        }
    }
    /// Arcs from the last search's source to node v (pathTo): empty for the source itself, None when v was not reached.
    pub fn path_to(&self, v: usize) -> Option<Vec<usize>> {
        if v as i32 == self.src {
            return Some(Vec::new());
        }
        let mut a = self.na[v];
        if a < 0 {
            return None;
        }
        let m = self.dist.len();
        let mut p = Vec::new();
        while a >= 0 && p.len() <= m {
            p.push(a as usize);
            a = self.pred[a as usize];
        }
        p.reverse();
        Some(p)
    }
}

/// A search tree kept with the node arrays it filled: the form of the prototype's dijkstra() and trace().
///
/// In the prototype each node-predecessor array keeps its own search, so trace() follows the
/// search that filled that array; here the tree owns both. Node predecessors alone do not make a
/// valid tree when routing is edge-based, so tracing follows the arc labels.
#[derive(Clone, Debug, Default)]
pub struct Tree {
    /// Best arrival cost at each node after the last search.
    pub dist: Vec<f64>,
    /// Arc each node is best reached by after the last search (-1 for none).
    pub pred: Vec<i32>,
    ws: Workspace,
}

impl Tree {
    /// An empty tree for a network.
    pub fn new(net: &Network) -> Tree {
        Tree {
            dist: vec![0.0; net.n],
            pred: vec![0; net.n],
            ws: Workspace::new(net),
        }
    }
    /// Searches from src (dijkstra), stopping at target when it is >= 0, and copies the node results into `dist` and `pred`.
    pub fn search(&mut self, g: &EdgeGraph, cost: &[f64], src: usize, target: i32) {
        g.spt(cost, src, &mut self.ws, target, -1);
        self.dist.copy_from_slice(&self.ws.nd);
        self.pred.copy_from_slice(&self.ws.na);
    }
    /// Arcs from src to dst on the last search (trace); None when dst was not reached or the last search did not start at src.
    ///
    /// The tree must be used with the graph that filled it (the prototype also returns null for a
    /// tree made on another graph).
    pub fn trace(&self, src: usize, dst: usize) -> Option<Vec<usize>> {
        if self.ws.src != src as i32 {
            return None;
        }
        self.ws.path_to(dst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A square 0-1-2-3 with a diagonal arc 0-2; arcs: 0:0>1 1:1>2 2:0>3 3:3>2 4:0>2 5:2>4
    fn square(ban_3_5: bool, pen: f64) -> EdgeGraph {
        let from = [0, 1, 0, 3, 0, 2];
        let to = [1, 2, 3, 2, 2, 4];
        let net = Network::new(5, &from, &to);
        let turns = Turns::new(&net, |a, b| {
            if ban_3_5 && a == 3 && b == 5 {
                f64::INFINITY
            } else if a == 4 && b == 5 {
                pen
            } else {
                0.0
            }
        });
        EdgeGraph::new(net, turns, vec![0; 5])
    }

    #[test]
    fn rows_and_turns_follow_arc_order() {
        let g = square(false, 0.0);
        assert_eq!(g.net.head, vec![0, 3, 4, 5, 6, 6]);
        assert_eq!(g.net.out, vec![0, 2, 4, 1, 5, 3]);
        assert_eq!(g.turns.index(&g.net, 1, 5), g.turns.off[1]);
        assert_eq!(g.turns.index(&g.net, 0, 5), -1);
        assert!(g.turns.banned(&g.net, 0, 5));
        assert!(square(true, 0.0).turns.banned(&g.net, 3, 5));
    }

    #[test]
    fn penalties_and_bans_steer_the_path() {
        let cost = [1.0, 1.0, 1.0, 1.0, 1.5, 1.0];
        let mut w = Workspace::new(&square(false, 0.0).net);
        // The diagonal is shortest to node 4
        let g = square(false, 0.0);
        g.spt(&cost, 0, &mut w, -1, -1);
        assert_eq!(w.path_to(4), Some(vec![4, 5]));
        assert_eq!(w.nd[4], 2.5);
        // A turn penalty on the diagonal sends it round the square; of the two equal sides the heap
        // order settles arc 3 first (util.js U.Heap's sift steps)
        let g = square(false, 1.0);
        g.spt(&cost, 0, &mut w, -1, -1);
        assert_eq!(w.path_to(4), Some(vec![2, 3, 5]));
        // Banning the turn from arc 3 leaves the other side
        let g = square(true, 1.0);
        g.spt(&cost, 0, &mut w, -1, -1);
        assert_eq!(w.path_to(4), Some(vec![0, 1, 5]));
        assert_eq!(w.path_to(0), Some(vec![]));
        let mut t = Tree::new(&g.net);
        t.search(&g, &cost, 0, -1);
        assert_eq!(t.trace(0, 4), Some(vec![0, 1, 5]));
        assert_eq!(t.trace(1, 4), None);
    }

    #[test]
    fn heap_breaks_ties_as_util_js() {
        let mut h = Heap::new();
        for (k, v) in [(2.0, 0), (1.0, 1), (1.0, 2), (3.0, 3), (1.0, 4)] {
            h.push(k, v);
        }
        let order: Vec<i32> = (0..5).map(|_| h.pop()).collect();
        assert_eq!(order, vec![1, 4, 2, 0, 3]);
        assert_eq!(h.top, 3.0);
    }
}
