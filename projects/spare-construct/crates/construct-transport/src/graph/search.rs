//! Searches on the graph (engine.js "shortest paths"), on the shared engine's edge-based search.

use super::Graph;
use crate::project::Route;
use engine_sim::{Tree, Workspace};

impl Graph {
    /// `workspace(G)`: scratch arrays for searches on this graph.
    #[must_use]
    pub fn workspace(&self) -> Workspace {
        Workspace::new(&self.search.net)
    }
    /// `spt(G, cost, src, W, target, fromArc)`: edge-based Dijkstra from node src (see [`engine_sim::EdgeGraph::spt`]).
    ///
    /// Entering arc b from arc a costs the turn penalty plus `cost[b]`; banned turns are skipped
    /// and zone centroids are never routed through. `from_arc` (>= 0, arriving at src) makes the
    /// first turn obey the same rules; the search stops once target (>= 0) is reached.
    pub fn spt(&self, cost: &[f64], src: usize, w: &mut Workspace, target: i32, from_arc: i32) {
        self.search.spt(cost, src, w, target, from_arc);
    }
    /// `pathTo(G, W, v)`: arcs from the last search's source to node v; empty for the source, None if v was not reached.
    #[must_use]
    pub fn path_to(&self, w: &Workspace, v: usize) -> Option<Vec<usize>> {
        w.path_to(v)
    }
    /// A tree for `dijkstra` and `trace`: the node-array interface, each tree keeping its own search.
    #[must_use]
    pub fn tree(&self) -> Tree {
        Tree::new(&self.search.net)
    }
    /// `dijkstra(G, cost, src, dist, pred, heap, target)`: searches into the tree, filling its node arrays `dist` and `pred`.
    pub fn dijkstra(&self, cost: &[f64], src: usize, tree: &mut Tree, target: i32) {
        tree.search(&self.search, cost, src, target);
    }
    /// `trace(G, pred, src, dst)`: arcs from src to dst on the tree's last search; None if unreached or the tree searched from elsewhere.
    #[must_use]
    pub fn trace(&self, tree: &Tree, src: usize, dst: usize) -> Option<Vec<usize>> {
        tree.trace(src, dst)
    }
    /// `resolveRoute(G, r, cost)`: the arcs of a bus route through its waypoints (cost: free-flow times when None).
    ///
    /// Each leg continues from the arc the previous one arrived on, so its first turn obeys the
    /// turn rules; a waypoint that can only be left by a banned turn starts its leg afresh.
    #[must_use]
    pub fn resolve_route(&self, r: &Route, cost: Option<&[f64]>) -> Vec<usize> {
        let cost = cost.unwrap_or(&self.ffc);
        let wp: Vec<usize> = r
            .wp
            .get()
            .map(|w| w.iter().filter_map(|id| self.idx.get(id).copied()).collect())
            .unwrap_or_default();
        let mut w = self.workspace();
        let mut arcs: Vec<usize> = Vec::new();
        // Bound: node and arc indexes are far below i32::MAX (the search's own index type)
        let as_i32 = |i: usize| i32::try_from(i).unwrap_or(-1);
        for pair in wp.windows(2) {
            let &[prev, next] = pair else {
                continue;
            };
            let from = match arcs.last() {
                Some(&last) if self.arcs.get(last).is_some_and(|a| a.to == prev) => as_i32(last),
                _ => -1,
            };
            let reached = |w: &Workspace| w.nd.get(next).is_some_and(|d| d.is_finite());
            self.spt(cost, prev, &mut w, as_i32(next), from);
            if from >= 0 && !reached(&w) {
                self.spt(cost, prev, &mut w, as_i32(next), -1);
            }
            if reached(&w)
                && let Some(p) = w.path_to(next)
            {
                arcs.extend(p);
            }
        }
        arcs
    }
}
