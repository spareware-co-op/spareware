//! The run: release vehicles, decide each vehicle's acceleration, move them and hand them on.
//!
//! Port of the transport prototype's sim.js `Sim`, operation for operation. Each step (0.5 s):
//! routes release vehicles into their queues and queued vehicles enter where there is room;
//! every vehicle chooses an acceleration with the Intelligent Driver Model, following the vehicle
//! ahead or, at the front of its lane, the line of the next arc (held back by a red signal, a
//! give-way rule or a full lane beyond); every vehicle moves; then the front vehicle of each lane
//! that has reached the end of its arc moves on to the next arc or leaves. Left-hand traffic.

use super::{
    Control, Light, MicroNet, MicroParams, MicroRoute, Rng, RouteState, Snapshot, Stats, Vehicle,
    js_max,
};
use std::collections::VecDeque;

/// The last vehicle to pass a node or join an arc: when, and from which arc.
#[derive(Clone, Copy, Debug)]
pub(super) struct Pass {
    pub(super) t: f64,
    pub(super) arc: usize,
}

/// A running microsimulation of vehicles on a [`MicroNet`].
#[derive(Clone, Debug)]
pub struct MicroSim<R: Rng> {
    pub(super) net: MicroNet,
    pub(super) params: MicroParams,
    pub(super) rng: R,
    pub(super) t: f64,
    pub(super) routes: Vec<RouteState>,
    // Vehicles by arc, then lane; the front of a lane (furthest along) first
    pub(super) lanes: Vec<Vec<VecDeque<Vehicle>>>,
    pub(super) count: Vec<i32>,
    pub(super) passed: Vec<f64>,
    // The last vehicle to pass each node, and to join each arc
    pub(super) node_last: Vec<Option<Pass>>,
    pub(super) merge_last: Vec<Option<Pass>>,
    pub(super) n: usize,
    pub(super) nid: u64,
    pub(super) stats: Stats,
    pub(super) snaps: Vec<Snapshot>,
}

impl<R: Rng> MicroSim<R> {
    /// Starts a run at time 0: no vehicles yet, each route's first release drawn from the random source in route order.
    pub fn new(net: MicroNet, routes: Vec<MicroRoute>, params: MicroParams, rng: R) -> Self {
        let m = net.arcs.len();
        let nodes = net.node_in.len();
        let lanes: Vec<Vec<VecDeque<Vehicle>>> = net
            .arcs
            .iter()
            .map(|a| {
                if a.virtual_arc {
                    Vec::new()
                } else {
                    (0..a.lanes + usize::from(a.bus_lane))
                        .map(|_| VecDeque::new())
                        .collect()
                }
            })
            .collect();
        let mut sim = MicroSim {
            net,
            params,
            rng,
            t: 0.0,
            routes: routes
                .into_iter()
                .map(|route| RouteState {
                    route,
                    next: 0.0,
                    queue: VecDeque::new(),
                })
                .collect(),
            lanes,
            count: vec![0; m],
            passed: vec![0.0; m],
            node_last: vec![None; nodes],
            merge_last: vec![None; m],
            n: 0,
            nid: 0,
            stats: Stats {
                done: 0,
                tt: 0.0,
                ff: 0.0,
                spawned: 0,
                teleports: 0,
                series: Vec::new(),
                bus_done: 0,
                bus_tt: 0.0,
            },
            snaps: Vec::new(),
        };
        for r in 0..sim.routes.len() {
            let next = if sim.routes[r].route.bus {
                // a random first bus within one headway
                let u = sim.rng.next_f64();
                u * 3600.0 / (sim.routes[r].route.rate * 3600.0)
            } else {
                sim.gap_for(r)
            };
            sim.routes[r].next = next;
        }
        sim
    }

    /// Simulated time (s).
    pub fn time(&self) -> f64 {
        self.t
    }

    /// The network.
    pub fn net(&self) -> &MicroNet {
        &self.net
    }

    /// The settings of the run.
    pub fn params(&self) -> &MicroParams {
        &self.params
    }

    /// The routes with their release times and entry queues.
    pub fn routes(&self) -> &[RouteState] {
        &self.routes
    }

    /// Vehicles on the network now.
    pub fn vehicles_on_road(&self) -> usize {
        self.n
    }

    /// Vehicles made so far (the number the next vehicle gets as its id).
    pub fn vehicles_made(&self) -> u64 {
        self.nid
    }

    /// Vehicles on each arc now.
    pub fn arc_counts(&self) -> &[i32] {
        &self.count
    }

    /// Vehicles that have left each arc so far.
    pub fn arc_passed(&self) -> &[f64] {
        &self.passed
    }

    /// Running totals and the 5 s series.
    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    /// The once-a-minute flow snapshots, the latest 12.
    pub fn snapshots(&self) -> &[Snapshot] {
        &self.snaps
    }

    /// The random source.
    pub fn rng_mut(&mut self) -> &mut R {
        &mut self.rng
    }

    /// Calls f for every vehicle on the network (arc by arc, lane by lane, front to back) with its arc, lane and the arc's lane count.
    pub fn for_each_vehicle(&self, mut f: impl FnMut(&Vehicle, usize, usize, usize)) {
        for ai in 0..self.net.arcs.len() {
            if self.count[ai] == 0 {
                continue;
            }
            let lanes = &self.lanes[ai];
            for (li, lane) in lanes.iter().enumerate() {
                for v in lane {
                    f(v, ai, li, lanes.len());
                }
            }
        }
    }

    /// What the signal shows the approach on this arc now; an arc without signals shows green.
    pub fn light(&self, arc: usize) -> Light {
        let Control::Signal { plan, phase } = self.net.arcs[arc].control else {
            return Light::Green;
        };
        let p = &self.net.signals[plan];
        let c = p.cycle;
        let lost_each = p.lost / 2.0;
        let gtot = c - p.lost;
        let g0 = gtot * p.split[0];
        let g1 = gtot * p.split[1];
        let tc = (self.t + p.offset) % c;
        let start = if phase == 0 { 0.0 } else { g0 + lost_each };
        let g = if phase == 0 { g0 } else { g1 };
        let u = (tc - start + c) % c;
        if u < g {
            Light::Green
        } else if u < g + 3.0 {
            Light::Amber
        } else {
            Light::Red
        }
    }

    // The gap before the next release on a car route: exponential with the route's rate times the demand scale
    pub(super) fn gap_for(&mut self, r: usize) -> f64 {
        let u = self.rng.next_f64();
        -(1.0 - u).ln() / js_max(1e-9, self.routes[r].route.rate * self.params.scale)
    }
}
