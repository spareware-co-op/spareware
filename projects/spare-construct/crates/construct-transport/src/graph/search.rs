//! Searches on the graph (engine.js "shortest paths"), on the shared engine's edge-based search.

use super::Graph;
use crate::project::Route;
use engine_sim::{Tree, Workspace};

impl Graph {
    /// `workspace(G)`: scratch arrays for searches on this graph.
    pub fn workspace(&self) -> Workspace {
        Workspace::new(&self.search.net)
    }
    /// `spt(G, cost, src, W, target, fromArc)`: edge-based Dijkstra from node src (see [`engine_sim::EdgeGraph::spt`]).
    ///
    /// Entering arc b from arc a costs the turn penalty plus `cost[b]`; banned turns are skipped
    /// and zone centroids are never routed through. from_arc (>= 0, arriving at src) makes the
    /// first turn obey the same rules; the search stops once target (>= 0) is reached.
    pub fn spt(&self, cost: &[f64], src: usize, w: &mut Workspace, target: i32, from_arc: i32) {
        self.search.spt(cost, src, w, target, from_arc);
    }
    /// `pathTo(G, W, v)`: arcs from the last search's source to node v; empty for the source, None if v was not reached.
    pub fn path_to(&self, w: &Workspace, v: usize) -> Option<Vec<usize>> {
        w.path_to(v)
    }
    /// A tree for `dijkstra` and `trace`: the node-array interface, each tree keeping its own search.
    pub fn tree(&self) -> Tree {
        Tree::new(&self.search.net)
    }
    /// `dijkstra(G, cost, src, dist, pred, heap, target)`: searches into the tree, filling its node arrays `dist` and `pred`.
    pub fn dijkstra(&self, cost: &[f64], src: usize, tree: &mut Tree, target: i32) {
        tree.search(&self.search, cost, src, target);
    }
    /// `trace(G, pred, src, dst)`: arcs from src to dst on the tree's last search; None if unreached or the tree searched from elsewhere.
    pub fn trace(&self, tree: &Tree, src: usize, dst: usize) -> Option<Vec<usize>> {
        tree.trace(src, dst)
    }
    /// `resolveRoute(G, r, cost)`: the arcs of a bus route through its waypoints (cost: free-flow times when None).
    ///
    /// Each leg continues from the arc the previous one arrived on, so its first turn obeys the
    /// turn rules; a waypoint that can only be left by a banned turn starts its leg afresh.
    pub fn resolve_route(&self, r: &Route, cost: Option<&[f64]>) -> Vec<usize> {
        let cost = cost.unwrap_or(&self.ffc);
        let wp: Vec<usize> = r
            .wp
            .get()
            .map(|w| w.iter().filter_map(|id| self.idx.get(id).copied()).collect())
            .unwrap_or_default();
        let mut w = self.workspace();
        let mut arcs: Vec<usize> = Vec::new();
        for k in 1..wp.len() {
            let from = match arcs.last() {
                Some(&last) if self.arcs[last].to == wp[k - 1] => last as i32,
                _ => -1,
            };
            self.spt(cost, wp[k - 1], &mut w, wp[k] as i32, from);
            if from >= 0 && !w.nd[wp[k]].is_finite() {
                self.spt(cost, wp[k - 1], &mut w, wp[k] as i32, -1);
            }
            if w.nd[wp[k]].is_finite()
                && let Some(p) = w.path_to(wp[k])
            {
                arcs.extend(p);
            }
        }
        arcs
    }
}
