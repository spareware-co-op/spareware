//! Traffic microsimulation from sim.js: the transport wrapper over the shared engine.
//!
//! [`Sim`] is the prototype's `Sim` class. It builds the network graph for the result's period,
//! applies the assignment's optimised signal timings, releases cars on the assigned paths (their
//! flows divided by the passenger car units per vehicle) and buses on the bus routes (at the
//! period's frequency), and runs them on the shared microsimulation core ([`engine_sim::micro`]):
//! Intelligent Driver Model car-following, signals on their timings, gap acceptance at give-way
//! junctions, roundabout entries yielding to circulating traffic, left-hand traffic. It also
//! answers what the interface shows: the HUD numbers ([`Sim::hud`]) and the comparison of
//! simulated with modelled link flows ([`Sim::validation`]).
//!
//! The random source is injected ([`Rng`]): a browser passes `Math.random`, a test passes the
//! seeded generator of the truth-test harness ([`engine_sim::Mulberry32`]).
//!
//! Where the prototype would throw, this does not: a signal approach arriving at a node other than
//! its controller's own (a signalised site of several nodes) takes the signal of the controller it
//! belongs to (the prototype looks the controller up by the arc's end node and fails there).
//! A project saved without `settings.sim` runs with a scale of 1 and a limit of 6,000 vehicles.

mod results;

pub use results::{SignalResult, SimPath, SimResults};

use crate::graph::{Arc, Ctrl, Graph, apply_signal, build_graph, js_sort_by};
use crate::jsnum::math_max;
use crate::project::{Scenario, Settings};
use crate::routes::bus_freq;
use crate::standards::pcu_f;
use crate::util::fmt;
use engine_sim::Rng;
use engine_sim::micro::{
    Control, Light, MicroArc, MicroNet, MicroParams, MicroRoute, MicroSim, SignalPlan, Stats,
    Vehicle,
};

/// Options a run is started with (`opts` of `new Sim(sc, res, opts)`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SimOptions {
    /// Demand scale for this run (`opts.scale`); None uses `settings.sim.scale`.
    pub scale: Option<f64>,
}

/// What a route is, beside the simulation's own view of it.
#[derive(Clone, Debug, PartialEq)]
pub struct SimRoute {
    /// The route runs buses (otherwise cars along an assigned path).
    pub bus: bool,
    /// The bus route's map colour (`r.color`).
    pub colour: Option<String>,
    /// The bus route's id (`r.id`).
    pub route_id: Option<String>,
}

/// The numbers the simulation HUD shows (`renderHud` in ui.js and the Simulation tab's header).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hud {
    /// Clock time (s since midnight): the period's start plus the simulated time.
    pub clock: f64,
    /// Vehicles on the road at the last sample.
    pub on_road: usize,
    /// Vehicles slower than 1.5 m/s at the last sample ("queued").
    pub queued: usize,
    /// Average speed at the last sample (km/h).
    pub speed: f64,
    /// Trips finished.
    pub trips_done: u64,
    /// Average delay per finished trip (s): travel time less free-flow time.
    pub delay_per_trip: f64,
    /// Vehicles waiting to enter the network at the last sample.
    pub waiting: usize,
    /// Vehicles released.
    pub released: u64,
    /// Vehicles moved on after two minutes held at a line in a gridlock.
    pub teleports: u64,
}

impl Hud {
    /// The clock as "hh:mm:ss".
    pub fn clock_text(&self) -> String {
        fmt::clock(self.clock)
    }
}

/// One road in the comparison of simulated with modelled flow.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidationRow {
    /// Arc key.
    pub key: String,
    /// Link id.
    pub link: String,
    /// Road name.
    pub name: String,
    /// Modelled flow (veh/h).
    pub model: f64,
    /// Simulated flow over the last 5 to 10 minutes (veh/h).
    pub sim: f64,
    /// GEH statistic of the two flows (below 5 is the usual calibration target).
    pub geh: f64,
}

/// Simulated flow against the assignment, by road, busiest first.
#[derive(Clone, Debug, PartialEq)]
pub struct Validation {
    /// The roads the model loads with at least 50 veh/h.
    pub rows: Vec<ValidationRow>,
    /// Share of the rows with a GEH below 5.
    pub pass: f64,
}

/// A running traffic simulation (the prototype's `Sim`).
pub struct Sim<R: Rng> {
    graph: Graph,
    period: Option<String>,
    start_clock: f64,
    scale: f64,
    max_veh: f64,
    hv: f64,
    pf: f64,
    results: Option<SimResults>,
    routes: Vec<SimRoute>,
    core: MicroSim<R>,
}

impl<R: Rng> Sim<R> {
    /// Starts a run for scenario `sc` and settings `s` from an assignment result (None: no car routes).
    ///
    /// Runs in the result's period (else the settings' period) on the graph for that period, and
    /// draws each route's first release from the random source in route order: cars first, then buses.
    pub fn new(
        sc: &Scenario,
        s: &Settings,
        res: Option<&SimResults>,
        opts: SimOptions,
        rng: R,
    ) -> Self {
        let period = res
            .and_then(|r| r.period.clone())
            .or_else(|| s.period.get().cloned());
        let mut g = build_graph(sc, s, period.as_deref());
        // The optimised signal timings of the assignment
        if let Some(res) = res {
            for k in 0..g.ctl.len() {
                if g.ctl[k].kind != "signals" {
                    continue;
                }
                let Some(nr) = res.signals.get(&g.ids[g.ctl[k].n]) else {
                    continue;
                };
                if !nr.has_cycle() {
                    continue;
                }
                if let Some(t) = g.ctl[k].signal.as_mut() {
                    t.c = nr.cycle.unwrap_or(f64::NAN);
                    if let Some(split) = &nr.split {
                        t.split = split.clone();
                    }
                }
                apply_signal(&g.ctl[k], &mut g.arcs);
            }
        }
        let sim_settings = s.sim.get();
        let scale = opts
            .scale
            .unwrap_or_else(|| sim_settings.map_or(1.0, |x| x.scale.num()));
        let max_veh = sim_settings.map_or(6000.0, |x| x.max_veh.num());
        let hv = s.hv_pct.num() / 100.0;
        let pf = pcu_f(s);
        let start_clock = if period.as_deref() == Some("AM") {
            7.5 * 3600.0
        } else {
            16.5 * 3600.0
        };
        let mut routes: Vec<MicroRoute> = Vec::new();
        let mut info: Vec<SimRoute> = Vec::new();
        // Cars on the assigned paths
        if let Some(res) = res
            && let (Some(paths), Some(keys)) = (&res.paths, &res.arc_keys)
        {
            let map: Vec<Option<usize>> = keys.iter().map(|k| g.key_idx.get(k).copied()).collect();
            for p in paths {
                let mut arcs: Vec<usize> = Vec::new();
                let mut ok = true;
                for &ai in &p.arcs {
                    match map.get(ai).copied().flatten() {
                        None => {
                            ok = false;
                            break;
                        }
                        Some(m) => {
                            if !g.arcs[m].conn {
                                arcs.push(m);
                            }
                        }
                    }
                }
                if !ok || arcs.is_empty() {
                    continue;
                }
                let rate = p.flow / pf / 3600.0;
                if rate > 0.0 {
                    routes.push(MicroRoute {
                        arcs,
                        rate,
                        bus: false,
                    });
                    info.push(SimRoute {
                        bus: false,
                        colour: None,
                        route_id: None,
                    });
                }
            }
        }
        // Buses on the bus routes
        for r in sc.routes.values() {
            let arcs: Vec<usize> = g
                .resolve_route(r, None)
                .into_iter()
                .filter(|&i| !g.arcs[i].conn)
                .collect();
            let freq = bus_freq(r, period.as_deref());
            if !arcs.is_empty() && freq > 0.0 {
                routes.push(MicroRoute {
                    arcs,
                    rate: freq / 3600.0,
                    bus: true,
                });
                info.push(SimRoute {
                    bus: true,
                    colour: r.color.get().cloned(),
                    route_id: Some(r.id.clone()),
                });
            }
        }
        let net = micro_net(&g);
        let core = MicroSim::new(net, routes, MicroParams::new(scale, max_veh, hv), rng);
        Sim {
            graph: g,
            period,
            start_clock,
            scale,
            max_veh,
            hv,
            pf,
            results: res.cloned(),
            routes: info,
            core,
        }
    }

    /// The network graph the run is on (`sim.G`), with the signal timings applied.
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// The shared simulation core, for its vehicles, counts, routes and statistics.
    pub fn core(&self) -> &MicroSim<R> {
        &self.core
    }

    /// The period it runs in ("AM", "PM"), if any.
    pub fn period(&self) -> Option<&str> {
        self.period.as_deref()
    }

    /// Clock time (s since midnight) at time 0: 07:30 in the AM peak, otherwise 16:30.
    pub fn start_clock(&self) -> f64 {
        self.start_clock
    }

    /// Demand scale in use.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// Vehicle limit in use.
    pub fn max_vehicles(&self) -> f64 {
        self.max_veh
    }

    /// Heavy vehicle share in use (0 to 1).
    pub fn heavy_share(&self) -> f64 {
        self.hv
    }

    /// Passenger car units per vehicle (`Engine.pcuF`): the divisor from path flows (pcu/h) to vehicles.
    pub fn pcu_factor(&self) -> f64 {
        self.pf
    }

    /// What each route of the core is (same order as `core().routes()`).
    pub fn routes(&self) -> &[SimRoute] {
        &self.routes
    }

    /// Simulated time (s).
    pub fn time(&self) -> f64 {
        self.core.time()
    }

    /// Advances one time step (0.5 s).
    pub fn step(&mut self) {
        self.core.step();
    }

    /// Running totals and the 5 s series.
    pub fn stats(&self) -> &Stats {
        self.core.stats()
    }

    /// What the signal shows the approach on this arc now (`sim.signal(A)`).
    pub fn signal(&self, arc: usize) -> Light {
        self.core.light(arc)
    }

    /// Calls f for every vehicle (arc by arc, lane by lane) with its graph arc, lane and the arc's lane count.
    pub fn for_each_vehicle(&self, mut f: impl FnMut(&Vehicle, &Arc, usize, usize)) {
        let g = &self.graph;
        self.core
            .for_each_vehicle(|v, ai, li, nl| f(v, &g.arcs[ai], li, nl));
    }

    /// The numbers the HUD shows now.
    pub fn hud(&self) -> Hud {
        let st = self.core.stats();
        let (on_road, queued, speed, waiting) = match st.series.last() {
            Some(l) => (l.n, l.q, l.speed, l.waiting),
            None => (0, 0, 0.0, 0),
        };
        Hud {
            clock: self.start_clock + self.core.time(),
            on_road,
            queued,
            speed,
            trips_done: st.done,
            delay_per_trip: if st.done != 0 {
                (st.tt - st.ff) / st.done as f64
            } else {
                0.0
            },
            waiting,
            released: st.spawned,
            teleports: st.teleports,
        }
    }

    /// Simulated flow through each road over the last 5 to 10 minutes, against the assignment (None before 7 minutes).
    pub fn validation(&self) -> Option<Validation> {
        let res = self.results.as_ref()?;
        let snaps = self.core.snapshots();
        let t = self.core.time();
        if snaps.is_empty() || t < 420.0 {
            return None;
        }
        let reference = snaps
            .iter()
            .find(|s| s.t <= t - 300.0 && s.t >= 120.0)
            .unwrap_or(&snaps[0]);
        let h = (t - reference.t) / 3600.0;
        if h <= 0.0 {
            return None;
        }
        let passed = self.core.arc_passed();
        let mut rows: Vec<ValidationRow> = Vec::new();
        for (ai, a) in self.graph.arcs.iter().enumerate() {
            if a.conn {
                continue;
            }
            let Some(&v) = res.arc_flow.get(&a.key) else {
                continue;
            };
            if v < 50.0 {
                continue;
            }
            let sim = (passed[ai] - reference.passed[ai]) / h;
            rows.push(ValidationRow {
                key: a.key.clone(),
                link: a.link.clone(),
                name: a.name.clone(),
                model: v,
                sim,
                geh: 0.0,
            });
        }
        if rows.is_empty() {
            return None;
        }
        // GEH statistic: below 5 is the usual calibration target
        for r in &mut rows {
            let d = r.sim - r.model;
            r.geh = (2.0 * (d * d) / math_max(1.0, r.sim + r.model)).sqrt();
        }
        js_sort_by(&mut rows, |a, b| b.model - a.model);
        let pass = rows.iter().filter(|r| r.geh < 5.0).count() as f64 / rows.len() as f64;
        Some(Validation { rows, pass })
    }
}

/// The shared core's network for a graph: arc lengths and speeds, control, conflicts and signal timings.
pub fn micro_net(g: &Graph) -> MicroNet {
    // One timing plan per signal controller
    let mut plan_of: Vec<Option<usize>> = vec![None; g.ctl.len()];
    let mut signals: Vec<SignalPlan> = Vec::new();
    for (k, ctl) in g.ctl.iter().enumerate() {
        if let Some(t) = &ctl.signal {
            plan_of[k] = Some(signals.len());
            signals.push(SignalPlan {
                cycle: t.c,
                lost: t.lost,
                split: [
                    t.split.first().copied().unwrap_or(f64::NAN),
                    t.split.get(1).copied().unwrap_or(f64::NAN),
                ],
                offset: f64::from(t.offset),
            });
        }
    }
    // The controller of a signal approach: the one at the arc's end node, else the one it belongs to
    let ctl_index = |n: usize| {
        g.ctl_at
            .get(n)
            .copied()
            .filter(|&k| k >= 0)
            .map(|k| k as usize)
    };
    let arcs = g
        .arcs
        .iter()
        .map(|a| {
            let control = match a.ctrl {
                Ctrl::Free => Control::Free,
                Ctrl::Minor => Control::Minor,
                Ctrl::Rbt => Control::Roundabout,
                Ctrl::Sig => ctl_index(a.to)
                    .or_else(|| a.node.and_then(ctl_index))
                    .and_then(|k| plan_of[k])
                    .map_or(Control::Free, |plan| Control::Signal {
                        plan,
                        phase: a.phase.unwrap_or(0),
                    }),
            };
            MicroArc {
                len: a.len,
                speed: a.speed,
                ff: a.ff,
                lanes: usize::try_from(a.gl).unwrap_or(0),
                bus_lane: a.bus,
                virtual_arc: a.conn,
                to: a.to,
                in_bearing: a.in_b,
                out_bearing: a.out_b,
                control,
                major: a.major,
                stop: a.stop,
                conflicts: a
                    .conf
                    .as_ref()
                    .map(|c| c.iter().map(|&(j, _)| j).collect())
                    .unwrap_or_default(),
            }
        })
        .collect();
    MicroNet {
        arcs,
        node_in: g.into.iter().map(Vec::len).collect(),
        node_out: g.out_r.iter().map(Vec::len).collect(),
        signals,
    }
}
