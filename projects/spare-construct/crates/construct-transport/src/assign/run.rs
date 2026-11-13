//! The equilibrium loop: all-or-nothing loading, conjugate Frank-Wolfe steps, junction capacity
//! updates, convergence, signal re-timing and the tracked flows that ride along with the main
//! flows. Port of engine.js `assign`, up to the point where the results are made.
//!
//! The JavaScript keeps all of this in the closures of one function; here it is one struct, with
//! its arrays (flows, costs, directions) as plain vectors in the same order and the same loops,
//! so the floating-point sums come out in the same order.

use super::Progress;
use super::paths::{NewPath, PathSet, add_paths, merge_paths, scale_paths};
use crate::demand::Demand;
use crate::graph::{Graph, zc_idx};
use crate::jsnum::{math_max, math_min, math_round, or_default, string_to_number, to_precision, truthy_number, value_truthy};
use crate::junction::{PerfSet, arc_time, update_node_caps};
use crate::project::{Route, Scenario, Settings};
use crate::results::{AssignOpts, ConvRow};
use crate::routes::{bus_freq, bus_pcu_of};
use crate::standards::{ConvOpts, ConvSet, conv_set, pcu_f};
use crate::util::{clamp, r1, r3};
use engine_sim::Workspace;
use serde_json::Value;
use std::collections::VecDeque;

/// One OD pair with trips: destination zone, trips (pcu/h), class (0 existing, 1 committed, 2 development) and its number.
#[derive(Clone, Copy)]
pub(super) struct Od {
    pub j: usize,
    pub q: f64,
    pub c: usize,
    /// The pair's number among all OD pairs with trips (its slot in the selection's OD flows).
    pub pair: usize,
}

/// A flow that is averaged with the main flows' steps: the flow x, the conjugate direction s and the all-or-nothing flow y.
pub(super) struct Trk {
    pub x: Vec<f64>,
    pub s: Vec<f64>,
    pub y: Vec<f64>,
}

impl Trk {
    fn new(n: usize) -> Trk {
        Trk {
            x: vec![0.0; n],
            s: vec![0.0; n],
            y: vec![0.0; n],
        }
    }
}

/// The select-link, select-turn or select-zone request as the run reads it.
pub(super) struct Sel {
    /// The request as given (`opts.select`).
    pub req: Value,
    /// Index of the zone selected, or -1.
    pub zone: i64,
    /// Index of the arc selected, or -1.
    pub arc: i64,
    /// Index of the turn selected, or -1.
    pub turn: i64,
    /// Something in the network was selected.
    pub on: bool,
}

/// Where the optional tracked flows sit in `Run::trk`.
#[derive(Default)]
pub(super) struct TrkIx {
    pub s: Option<usize>,
    pub ts: Option<usize>,
    pub od: Option<usize>,
    pub m: Option<usize>,
    pub md: Option<usize>,
}

pub(super) const T_C1: usize = 0;
pub(super) const T_C2: usize = 1;
pub(super) const T_TD: usize = 2;

/// A development trip row: zone index, trips from it and trips to it.
pub(super) type DevRow = (usize, f64, f64);

/// The OD pairs with trips by origin, with their class totals and development rows.
struct OdSet {
    list: Vec<Vec<Od>>,
    cls_trips: [f64; 3],
    dev_gw: Vec<DevRow>,
    dev_zone: Vec<DevRow>,
    trips: f64,
    n_od: usize,
}

/// One assignment in progress.
///
/// Paths are stored (`paths` and `sp` are set) for up to 300 zones, and the junction-site
/// movements are traced (`ix.m` and `ix.md` are set) when the graph has site movement slots.
pub(super) struct Run<'a, 'p> {
    pub g: Graph,
    pub s: &'a Settings,
    pub period: String,
    pub perf: PerfSet,
    pub cv: ConvSet,
    pub m: usize,
    pub nt: usize,
    pub zn: usize,
    pub pf: f64,
    pub bus_pcu: f64,
    pub routes: Vec<Route>,
    pub route_arcs: Vec<Vec<usize>>,
    pub pre: Vec<f64>,
    pub apen: Vec<f64>,
    pub has_pen: bool,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub cost: Vec<f64>,
    pub xt: Vec<f64>,
    pub x_t: Vec<f64>,
    pub y_t: Vec<f64>,
    pub pc: Vec<f64>,
    pub w: Workspace,
    pub od_list: Vec<Vec<Od>>,
    pub cls_trips: [f64; 3],
    pub dev_gw: Vec<DevRow>,
    pub dev_zone: Vec<DevRow>,
    pub sel: Option<Sel>,
    pub trips: f64,
    pub paths: Option<PathSet>,
    sp: Option<PathSet>,
    pub unserved: f64,
    pub trk: Vec<Trk>,
    pub ix: TrkIx,
    pend: Vec<i32>,
    opened: Vec<usize>,
    s_dir: Vec<f64>,
    s_t: Vec<f64>,
    has_s: bool,
    pub conv: Vec<ConvRow>,
    x_prev: Vec<f64>,
    pub hist: VecDeque<Vec<f64>>,
    consec_p: f64,
    k_cap: u32,
    last_p: Option<f64>,
    pub hit_max: bool,
    pub gap: f64,
    pub it: u32,
    progress: Option<Progress<'p>>,
}

// An arc, turn or slot index from one of the search's i32 arrays: -1 (none) and a missing entry give None
fn at(v: &[i32], i: usize) -> Option<usize> {
    v.get(i).and_then(|&e| usize::try_from(e).ok())
}

// Adds q to v[i]; every caller passes an index from the run's own tables, so it is always there
fn add_at(v: &mut [f64], i: usize, q: f64) {
    if let Some(e) = v.get_mut(i) {
        *e += q;
    }
}

// Adds q to the all-or-nothing flow of tracked flow t at index i (both from the run's own tables)
fn add_trk(trk: &mut [Trk], t: usize, i: usize, q: f64) {
    if let Some(e) = trk.get_mut(t).and_then(|tr| tr.y.get_mut(i)) {
        *e += q;
    }
}

// The row of a zone in a development table, added last (with no trips) when it is not there yet
fn dev_row(rows: &mut Vec<DevRow>, zone: usize) -> Option<&mut DevRow> {
    if let Some(found) = rows.iter().position(|r| r.0 == zone) {
        rows.get_mut(found)
    } else {
        rows.push((zone, 0.0, 0.0));
        rows.last_mut()
    }
}

// Bus preloads (pcu/h) on the arcs of each route, except arcs with a bus lane
fn bus_preload(g: &Graph, routes: &[Route], route_arcs: &[Vec<usize>], period: &str, bus_pcu: f64) -> Vec<f64> {
    let mut pre = vec![0.0f64; g.m];
    for (route, arcs) in routes.iter().zip(route_arcs) {
        let freq = bus_freq(route, Some(period));
        for &ai in arcs {
            if g.arcs.get(ai).is_some_and(|arc| !arc.bus())
                && let Some(slot) = pre.get_mut(ai)
            {
                *slot += freq * bus_pcu;
            }
        }
    }
    pre
}

/* Additive generalised cost per arc (s): time-of-use charges (L.toll via buildGraph, and opts.arcPenalty keyed by
arc key or link id). Fixed per traversal, so they add no curvature; they are not time (see results). */
fn charges(g: &Graph, opts: &AssignOpts) -> (Vec<f64>, bool) {
    let mut apen = vec![0.0f64; g.m];
    let ap = match &opts.arc_penalty {
        Some(Value::Object(o)) => Some(o),
        _ => None,
    };
    let mut has_pen = false;
    for (arc, slot) in g.arcs.iter().zip(apen.iter_mut()) {
        if arc.conn() {
            continue;
        }
        let mut charge = arc.toll;
        if let Some(ap) = ap {
            let pick = |k: &str| ap.get(k).filter(|v| !v.is_null());
            let extra = pick(&arc.key)
                .or_else(|| pick(&arc.link))
                .map_or(0.0, crate::jsnum::value_to_number);
            if extra > 0.0 && extra.is_finite() {
                charge += extra;
            }
        }
        if charge > 0.0 {
            *slot = charge;
            has_pen = true;
        }
    }
    (apen, has_pen)
}

// Selection: a zone (OD pairs with either end there), an arc or a turn (paths that use it)
fn selection(g: &Graph, opts: &AssignOpts) -> Option<Sel> {
    let req = opts.select.clone().filter(value_truthy)?;
    let get = |k: &str| req.get(k).filter(|v| !v.is_null());
    // Bound: zone and arc indexes are counts of things in memory, far below i64::MAX
    let zone: i64 = match get("zone") {
        Some(z) => g
            .zones
            .iter()
            .position(|zz| z.as_str() == Some(zz.id.as_str()))
            .and_then(|found| i64::try_from(found).ok())
            .unwrap_or(-1),
        None => -1,
    };
    let arc: i64 = match get("arc").and_then(|v| v.as_str()) {
        Some(k) => g.key_idx.get(k).and_then(|&ai| i64::try_from(ai).ok()).unwrap_or(-1),
        None => -1,
    };
    let mut turn: i64 = -1;
    if let Some(Value::Array(t)) = req.get("turn") {
        let from = t.first().and_then(|v| v.as_str()).and_then(|k| g.key_idx.get(k));
        let to = t.get(1).and_then(|v| v.as_str()).and_then(|k| g.key_idx.get(k));
        if let (Some(&from), Some(&to)) = (from, to) {
            turn = i64::from(g.turn_idx(from, to));
        }
    }
    let on = zone >= 0 || arc >= 0 || turn >= 0;
    Some(Sel { req, zone, arc, turn, on })
}

// The OD list with its classes: every pair with more than 0.0005 trips, by origin
fn od_classes(g: &Graph, dem: &Demand, pf: f64) -> OdSet {
    let zn = dem.z;
    let zc: Vec<usize> = g.zones.iter().map(|z| zc_idx(Some(z))).collect();
    let is_gw: Vec<bool> = g.zones.iter().map(|z| z.kind.is("gateway")).collect();
    let mut set = OdSet {
        list: Vec::with_capacity(zn),
        cls_trips: [0.0f64; 3],
        dev_gw: Vec::new(),
        dev_zone: Vec::new(),
        trips: 0.0,
        n_od: 0,
    };
    for i in 0..zn {
        // The matrix is zn by zn, row by row; a missing entry reads as no trips (undefined in the JavaScript)
        let row: &[f64] = i.checked_mul(zn).and_then(|start| dem.t.get(start..)).unwrap_or_default();
        // zc and is_gw have an entry for every zone of the matrix
        let (cls_i, gw_i) = (zc.get(i).copied().unwrap_or(0), is_gw.get(i).copied().unwrap_or(false));
        let mut ods = Vec::new();
        for j in 0..zn {
            let Some(&q) = row.get(j) else {
                continue;
            };
            if !(q > 0.0005) {
                continue;
            }
            let (cls_j, gw_j) = (zc.get(j).copied().unwrap_or(0), is_gw.get(j).copied().unwrap_or(false));
            let c = cls_i.max(cls_j);
            ods.push(Od {
                j,
                q: q * pf,
                c,
                pair: set.n_od,
            });
            // Bound: one per OD pair, at most zn * zn of them
            set.n_od = set.n_od.saturating_add(1);
            set.trips += q;
            if let Some(total) = set.cls_trips.get_mut(c) {
                *total += q;
            }
            if c == 2 {
                // development trips: where they cross the boundary, and each development zone's own trips
                let gi = if gw_i {
                    Some(i)
                } else if gw_j {
                    Some(j)
                } else {
                    None
                };
                if let Some(gi) = gi
                    && let Some(r) = dev_row(&mut set.dev_gw, gi)
                {
                    if gi == i {
                        r.1 += q;
                    } else {
                        r.2 += q;
                    }
                }
                if cls_i == 2
                    && let Some(r) = dev_row(&mut set.dev_zone, i)
                {
                    r.1 += q;
                }
                if cls_j == 2
                    && let Some(r) = dev_row(&mut set.dev_zone, j)
                {
                    r.2 += q;
                }
            }
        }
        set.list.push(ods);
    }
    set
}

// Tracked flows [x, s (conjugate direction), y (all-or-nothing)]: committed and development link flows, development
// turn flows, and the selection's link, turn and OD flows; all are averaged with the main flows' steps
fn tracked(n_arcs: usize, nt: usize, n_od: usize, sel_on: bool, n_slots: usize) -> (Vec<Trk>, TrkIx) {
    let mut trk: Vec<Trk> = vec![Trk::new(n_arcs), Trk::new(n_arcs), Trk::new(nt)];
    let mut ix = TrkIx::default();
    if sel_on {
        ix.s = Some(trk.len());
        trk.push(Trk::new(n_arcs));
        ix.ts = Some(trk.len());
        trk.push(Trk::new(nt));
        ix.od = Some(trk.len());
        trk.push(Trk::new(n_od));
    }
    /* Junction-site movements traced along each path (entry arc to exit arc), all traffic and development traffic.
    Walking a path backwards, an exit arc opens its site and the site's entry arc closes it; an arc between two
    sites closes the downstream one before it opens the upstream one. */
    if n_slots > 0 {
        ix.m = Some(trk.len());
        trk.push(Trk::new(n_slots));
        ix.md = Some(trk.len());
        trk.push(Trk::new(n_slots));
    }
    (trk, ix)
}

impl<'a, 'p> Run<'a, 'p> {
    /// The start of `assign` up to the first all-or-nothing loading: bus preloads, charges, the OD list with its classes, the selection and the tracked flows.
    pub fn new(
        sc: &Scenario,
        s: &'a Settings,
        period: &str,
        opts: &AssignOpts,
        mut g: Graph,
        dem: &Demand,
        progress: Option<Progress<'p>>,
    ) -> Run<'a, 'p> {
        let pf = pcu_f(s);
        let zn = dem.z;
        let n_arcs = g.m;
        let bus_pcu = bus_pcu_of(s);
        let routes: Vec<Route> = sc.routes.values().cloned().collect();
        let route_arcs: Vec<Vec<usize>> = routes.iter().map(|r| g.resolve_route(r, None)).collect();
        let pre = bus_preload(&g, &routes, &route_arcs, period, bus_pcu);
        let (apen, has_pen) = charges(&g, opts);
        let nt = g.n_t;
        let pc: Vec<f64> = g.tpen().iter().map(|&p| if p == f64::INFINITY { 0.0 } else { p }).collect();
        let sel = selection(&g, opts);
        let sel_on = sel.as_ref().is_some_and(|x| x.on);
        let ods = od_classes(&g, dem, pf);
        let store_paths = zn <= 300;
        let n_slots = g.sm.as_ref().map_or(0, |sm| sm.n);
        let (trk, ix) = tracked(n_arcs, nt, ods.n_od, sel_on, n_slots);
        let pend = vec![-1i32; g.sm.as_ref().map_or(0, |sm| sm.list.len())];
        if n_slots > 0 {
            g.sm_x = Some(vec![0.0; n_slots]);
        }
        g.mv_detail = false; // lane rows for the junction tables are built on the last capacity update only
        let cv = conv_set(
            s,
            &ConvOpts {
                gap_tol: Some(opts.gap_tol.num()),
                max_iter: Some(opts.max_iter.num()),
            },
        );
        let workspace = g.workspace();
        Run {
            perf: PerfSet::new(s),
            s,
            period: period.to_string(),
            cv,
            m: n_arcs,
            nt,
            zn,
            pf,
            bus_pcu,
            routes,
            route_arcs,
            pre,
            apen,
            has_pen,
            x: vec![0.0; n_arcs],
            y: vec![0.0; n_arcs],
            cost: vec![0.0; n_arcs],
            xt: vec![0.0; n_arcs],
            x_t: vec![0.0; nt],
            y_t: vec![0.0; nt],
            pc,
            w: workspace,
            od_list: ods.list,
            cls_trips: ods.cls_trips,
            dev_gw: ods.dev_gw,
            dev_zone: ods.dev_zone,
            sel,
            trips: ods.trips,
            paths: store_paths.then(PathSet::new),
            sp: store_paths.then(PathSet::new),
            unserved: 0.0,
            trk,
            ix,
            pend,
            opened: Vec::new(),
            s_dir: vec![0.0; n_arcs],
            s_t: vec![0.0; nt],
            has_s: false,
            conv: Vec::new(),
            x_prev: vec![0.0; n_arcs],
            hist: VecDeque::new(),
            consec_p: 0.0,
            k_cap: 0,
            last_p: None,
            hit_max: false,
            gap: 1.0,
            it: 1,
            progress,
            g,
        }
    }

    fn prog(&mut self, it: u32, gap: f64, msg: &str) {
        if let Some(f) = self.progress.as_mut() {
            f(it, gap, msg);
        }
    }
}

impl Run<'_, '_> {
    fn set_xt(&mut self) {
        for ((xt, &xa), &pa) in self.xt.iter_mut().zip(&self.x).zip(&self.pre) {
            *xt = xa + pa;
        }
    }

    fn costs(&mut self) {
        let arcs = self.g.arcs.iter().zip(&self.apen);
        for (((ct, &xa), &pa), (arc, &ap)) in self.cost.iter_mut().zip(&self.x).zip(&self.pre).zip(arcs) {
            *ct = arc_time(arc, xa + pa, &self.perf) + ap;
        }
    }

    // `G.smX` is the traced site flows xM, live in the JavaScript; here it is copied in before each capacity update
    fn sync_sm(&mut self) {
        if let (Some(tr), Some(buf)) = (self.ix.m.and_then(|i| self.trk.get(i)), self.g.sm_x.as_mut()) {
            buf.copy_from_slice(&tr.x);
        }
    }

    // `caps()`: junction capacities from the current flows; MSA damping (1 / kCap) after the first update of a phase
    fn caps(&mut self) {
        // Bound: one per iteration of a phase, far below u32::MAX
        self.k_cap = self.k_cap.saturating_add(1);
        let w = if self.cv.msa && self.k_cap > 1 { 1.0 / f64::from(self.k_cap) } else { 1.0 };
        self.sync_sm();
        update_node_caps(&mut self.g, &self.xt, w, Some(&self.x_t));
    }

    // The junction-site movement step of a path walked backwards at arc a: an entry arc closes its site's open
    // movement (adding q to it), an exit arc opens one. Does nothing unless site movements are traced.
    fn trace_site(&mut self, a: usize, q: f64, dev: bool) {
        let (Some(sm), Some(xm), Some(xd)) = (self.g.sm.as_ref(), self.ix.m, self.ix.md) else {
            return;
        };
        if let Some(site) = at(&sm.ent_s, a)
            && let Some(open) = at(&self.pend, site)
        {
            // Bound: the slot lies in the site's block of sm.n slots, far below usize::MAX
            if let (Some(st), Some(ei)) = (sm.list.get(site), at(&sm.ent_i, a)) {
                let kk = st.off.saturating_add(ei.saturating_mul(st.nx)).saturating_add(open);
                add_trk(&mut self.trk, xm, kk, q);
                if dev {
                    add_trk(&mut self.trk, xd, kk, q);
                }
            }
            if let Some(slot) = self.pend.get_mut(site) {
                *slot = -1;
            }
        }
        if let Some(site) = at(&sm.ext_s, a) {
            if let Some(slot) = self.pend.get_mut(site) {
                *slot = sm.ext_i.get(a).copied().unwrap_or(-1);
            }
            self.opened.push(site);
        }
    }

    // The selection's flows for OD pair (i, j) ending at node dn: when the pair touches the selected zone or its
    // path uses the selected arc or turn, its trips go on the selection's OD, link and turn flows
    fn select_track(&mut self, i: usize, j: usize, dn: usize, q: f64, pair: usize) {
        let Some((zone, arc, turn)) = self.sel.as_ref().filter(|sel| sel.on).map(|sel| (sel.zone, sel.arc, sel.turn)) else {
            return;
        };
        let mut hit = i64::try_from(i).is_ok_and(|v| v == zone) || i64::try_from(j).is_ok_and(|v| v == zone);
        if !hit {
            let mut cur = at(&self.w.na, dn);
            while let Some(ai) = cur {
                if i64::try_from(ai).is_ok_and(|v| v == arc)
                    || (turn >= 0 && self.w.pt.get(ai).is_some_and(|&t| i64::from(t) == turn))
                {
                    hit = true;
                    break;
                }
                cur = at(&self.w.pred, ai);
            }
        }
        // The selection's tracked flows are made whenever the selection is on
        let (Some(xs), Some(xts), Some(xod)) = (self.ix.s, self.ix.ts, self.ix.od) else {
            return;
        };
        if !hit {
            return;
        }
        add_trk(&mut self.trk, xod, pair, q);
        let mut cur = at(&self.w.na, dn);
        while let Some(ai) = cur {
            add_trk(&mut self.trk, xs, ai, q);
            if let Some(t) = at(&self.w.pt, ai) {
                add_trk(&mut self.trk, xts, t, q);
            }
            cur = at(&self.w.pred, ai);
        }
    }

    /// `aon(np)`: all-or-nothing loading on the current costs; np (when given) collects each OD pair's path.
    fn aon(&mut self, mut np: Option<&mut Vec<NewPath>>) {
        let zn = self.zn;
        self.y.fill(0.0);
        self.y_t.fill(0.0);
        self.unserved = 0.0;
        for t in &mut self.trk {
            t.y.fill(0.0);
        }
        for i in 0..zn {
            let n_dest = self.od_list.get(i).map_or(0, Vec::len);
            if n_dest == 0 {
                continue;
            }
            // zone_node has an entry for every zone of the matrix
            let Some(&src) = self.g.zone_node.get(i) else {
                continue;
            };
            self.g.spt(&self.cost, src, &mut self.w, -1, -1);
            for k in 0..n_dest {
                let Some(&Od { j, q, c, pair }) = self.od_list.get(i).and_then(|l| l.get(k)) else {
                    continue;
                };
                // A destination not reached (or with no node: nd[undefined] in the JavaScript) is unserved
                let Some(dn) = self
                    .g
                    .zone_node
                    .get(j)
                    .copied()
                    .filter(|&d| self.w.nd.get(d).is_some_and(|nd| nd.is_finite()))
                else {
                    self.unserved += q;
                    continue;
                };
                let mut path: Option<Vec<usize>> = np.is_some().then(Vec::new);
                let ycl = match c {
                    2 => Some(T_C2),
                    1 => Some(T_C1),
                    _ => None,
                };
                let mut cur = at(&self.w.na, dn);
                while let Some(a) = cur {
                    add_at(&mut self.y, a, q);
                    if let Some(t) = at(&self.w.pt, a) {
                        add_at(&mut self.y_t, t, q);
                        if c == 2 {
                            add_trk(&mut self.trk, T_TD, t, q);
                        }
                    }
                    if let Some(t) = ycl {
                        add_trk(&mut self.trk, t, a, q);
                    }
                    if let Some(p) = path.as_mut() {
                        p.push(a);
                    }
                    self.trace_site(a, q, c == 2);
                    cur = at(&self.w.pred, a);
                }
                if !self.opened.is_empty() {
                    for &site in &self.opened {
                        if let Some(slot) = self.pend.get_mut(site) {
                            *slot = -1;
                        }
                    }
                    self.opened.clear();
                }
                self.select_track(i, j, dn, q, pair);
                if let Some(np) = np.as_mut()
                    && let Some(mut p) = path
                {
                    p.reverse();
                    // Bound: paths are stored for up to 300 zones only, so i * zn + j stays small
                    np.push((i.saturating_mul(zn).saturating_add(j), p, q));
                }
            }
        }
    }

    /// `direction(np)`: the conjugate Frank-Wolfe search direction (Mitradjieva and Lindberg 2013).
    ///
    /// The direction s mixes the previous one with the new all-or-nothing flows y, using the
    /// diagonal of the cost Jacobian, which converges much faster than plain Frank-Wolfe near
    /// equilibrium. Turn penalties are fixed costs, so they add no curvature.
    fn direction(&mut self, np: Option<Vec<NewPath>>) {
        let mut al = 0.0f64;
        if self.has_s {
            let (mut num, mut den) = (0.0f64, 0.0f64);
            let arcs = self.g.arcs.iter().zip(&self.apen).zip(&self.cost);
            for ((((&sd, &xa), &pa), &ya), ((arc, &ap), &ct)) in
                self.s_dir.iter().zip(&self.x).zip(&self.pre).zip(&self.y).zip(arcs)
            {
                let ds = sd - xa;
                if ds == 0.0 {
                    continue;
                }
                let q = xa + pa;
                let h = math_max(1.0, q * 0.01);
                let hs = (arc_time(arc, q + h, &self.perf) + ap - ct) / h;
                num += ds * hs * (ya - xa);
                den += ds * hs * (ya - sd);
            }
            al = if den == 0.0 { 0.0 } else { num / den };
            al = if al > 0.99 {
                0.99
            } else if al > 0.0 {
                al
            } else {
                0.0
            };
        }
        for (sd, &ya) in self.s_dir.iter_mut().zip(&self.y) {
            *sd = al * *sd + (1.0 - al) * ya;
        }
        for (st, &yt) in self.s_t.iter_mut().zip(&self.y_t) {
            *st = al * *st + (1.0 - al) * yt;
        }
        for tr in &mut self.trk {
            for (sv, &yv) in tr.s.iter_mut().zip(&tr.y) {
                *sv = al * *sv + (1.0 - al) * yv;
            }
        }
        // np is given only when paths are stored, so sp is there
        if let Some(np) = np
            && let Some(sp) = self.sp.as_mut()
        {
            if al > 0.0 {
                scale_paths(sp, al);
            } else {
                *sp = PathSet::new();
            }
            add_paths(np, 1.0 - al, sp);
        }
        self.has_s = true;
    }

    // `derivative(lam)`: the slope of the objective along the direction at step lam
    fn derivative(&self, lam: f64) -> f64 {
        let mut r = 0.0f64;
        let arcs = self.g.arcs.iter().zip(&self.apen);
        for (((&sd, &xa), &pa), (arc, &ap)) in self.s_dir.iter().zip(&self.x).zip(&self.pre).zip(arcs) {
            let d = sd - xa;
            if d != 0.0 {
                r += d * (arc_time(arc, xa + lam * d + pa, &self.perf) + ap);
            }
        }
        for ((&pcv, &st), &xt) in self.pc.iter().zip(&self.s_t).zip(&self.x_t) {
            if truthy_number(pcv) {
                r += (st - xt) * pcv;
            }
        }
        r
    }

    // `lineSearch()`: bisection on the derivative, 20 halvings
    fn line_search(&self) -> f64 {
        if self.derivative(1.0) <= 0.0 {
            return 1.0;
        }
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        for _ in 0..20 {
            let m = f64::midpoint(lo, hi);
            if self.derivative(m) > 0.0 {
                hi = m;
            } else {
                lo = m;
            }
        }
        f64::midpoint(lo, hi)
    }

    /// `iterate(maxIt, tol, final)`: equilibrium iterations until the gap is under tol (and, for the final phase under the stability rule, the flows are stable) or maxIt is reached.
    ///
    /// Convergence (docs/spec/convergence): relative gap, plus the TAG stability measure P = share
    /// of road arcs (flow over 1 pcu/h) whose flow changed by less than 1% in the last update.
    /// With the stability rule on, a final phase stops only when gap < tol and P stays above 98%
    /// for 4 consecutive iterations.
    fn iterate(&mut self, max_it: f64, tol: f64, final_phase: bool) {
        self.has_s = false; // costs changed (signal timings): start the conjugate directions afresh
        self.consec_p = 0.0;
        self.hit_max = false;
        self.k_cap = 0;
        let mut k = 0.0f64;
        while k < max_it {
            self.set_xt();
            self.caps();
            self.costs();
            let mut np: Option<Vec<NewPath>> = self.paths.is_some().then(Vec::new);
            self.aon(np.as_mut());
            let (mut num, mut den, mut vh) = (0.0f64, 0.0f64, 0.0f64);
            let arcs = self.g.arcs.iter().zip(&self.apen);
            for (((&ct, &xa), &ya), (arc, &ap)) in self.cost.iter().zip(&self.x).zip(&self.y).zip(arcs) {
                num += ct * (xa - ya);
                den += ct * xa;
                if !arc.conn() {
                    vh += xa * (ct - ap);
                }
            }
            for ((&pcv, &xt), &yt) in self.pc.iter().zip(&self.x_t).zip(&self.y_t) {
                if truthy_number(pcv) {
                    num += pcv * (xt - yt);
                    den += pcv * xt;
                    vh += pcv * xt;
                }
            }
            self.gap = if den > 0.0 { (num / den).abs() } else { 0.0 };
            // Bound: one per iteration, far below u32::MAX
            self.it = self.it.saturating_add(1);
            self.conv.push(ConvRow {
                it: f64::from(self.it),
                gap: string_to_number(&to_precision(self.gap, 3)),
                p: self.last_p.map(r3),
                vht: r1(vh / self.pf / 3600.0),
            });
            if self.gap < tol && k > 1.0 && (!self.cv.stable || !final_phase || self.consec_p >= self.cv.consec) {
                break;
            }
            if k == max_it - 1.0 {
                self.hit_max = true;
            }
            self.direction(np);
            let lam = self.line_search();
            self.x_prev.copy_from_slice(&self.x);
            for (xa, &sd) in self.x.iter_mut().zip(&self.s_dir) {
                *xa += lam * (sd - *xa);
            }
            for (xt, &st) in self.x_t.iter_mut().zip(&self.s_t) {
                *xt += lam * (st - *xt);
            }
            for tr in &mut self.trk {
                for (xv, &sv) in tr.x.iter_mut().zip(&tr.s) {
                    *xv += lam * (sv - *xv);
                }
            }
            let (mut n_p, mut ok_p) = (0u32, 0u32);
            for ((arc, &xp), &xa) in self.g.arcs.iter().zip(&self.x_prev).zip(&self.x) {
                if arc.conn() || !(xp > 1.0) {
                    continue;
                }
                // Bound: counts of arcs, far below u32::MAX
                n_p = n_p.saturating_add(1);
                if (xa - xp).abs() < self.cv.chg * xp {
                    ok_p = ok_p.saturating_add(1);
                }
            }
            let last_p = if n_p > 0 { f64::from(ok_p) / f64::from(n_p) } else { 1.0 };
            self.last_p = Some(last_p);
            self.consec_p = if last_p > self.cv.share { self.consec_p + 1.0 } else { 0.0 };
            self.hist.push_back(self.x.clone());
            if self.hist.len() > 6 {
                self.hist.pop_front();
            }
            if let (Some(paths), Some(sp)) = (self.paths.as_mut(), self.sp.as_ref()) {
                scale_paths(paths, 1.0 - lam);
                merge_paths(paths, sp, lam);
            }
            if self.it.is_multiple_of(2) {
                let (it, gap) = (self.it, self.gap);
                self.prog(it, gap, "");
            }
            k += 1.0;
        }
    }

    // Webster timings for signal controller k from the current flows: the cycle from the critical flow ratios of the
    // two phases, and the green split between them
    fn retime(&mut self, k: usize) {
        let Some(ctl) = self.g.ctl.get(k) else {
            return;
        };
        // a site's own cycle and fixed timing, or the node's
        if ctl.src.fixed_timing.truthy() {
            return;
        }
        let mut yv = [0.02f64, 0.02f64];
        for &ai in &ctl.arcs {
            // A controller's entry arcs are arcs of the graph, with flows
            let (Some(arc), Some(&xt)) = (self.g.arcs.get(ai), self.xt.get(ai)) else {
                continue;
            };
            // Phases are 0 and 1; the JavaScript would set an unused property for any other
            if let Some(slot) = yv.get_mut(usize::from(arc.phase.unwrap_or(0))) {
                *slot = math_max(*slot, xt / (f64::from(arc.gl) * self.s.sat_flow.num()));
            }
        }
        let [y0, y1] = yv;
        let yy = math_min(0.92, y0 + y1);
        let cycle = ctl.src.cycle.num();
        // Only controllers with a signal timing are re-timed
        let Some(timing) = self.g.ctl.get_mut(k).and_then(|c| c.signal.as_mut()) else {
            return;
        };
        timing.c = or_default(cycle, math_round(clamp((1.5 * timing.lost + 5.0) / (1.0 - yy), 50.0, 140.0)));
        let s0 = clamp(y0 / (y0 + y1), 0.2, 0.8);
        timing.split = vec![s0, 1.0 - s0];
        if let Some(ctl) = self.g.ctl.get(k) {
            crate::graph::apply_signal(ctl, &mut self.g.arcs);
        }
    }

    /// The equilibrium: a first all-or-nothing loading, the iterations, signal re-timing and a last capacity update on the final flows.
    ///
    /// Signal optimisation re-times the signals twice after the first equilibrium. Under the v2
    /// rule (stability on) the first two phases only need a loose gap (they are followed by new
    /// timings) and the whole run shares one budget of maxIter iterations, the last phase getting
    /// what is left; before v2 each phase had its own budget (maxIter, then maxIter / 3 twice) and
    /// the same target.
    pub fn solve(&mut self) {
        self.set_xt();
        self.caps();
        self.costs();
        let mut np: Option<Vec<NewPath>> = self.paths.is_some().then(Vec::new);
        self.aon(np.as_mut());
        self.x.copy_from_slice(&self.y);
        self.x_t.copy_from_slice(&self.y_t);
        for tr in &mut self.trk {
            tr.x.copy_from_slice(&tr.y);
        }
        if let Some(np) = np
            && let Some(paths) = self.paths.as_mut()
        {
            add_paths(np, 1.0, paths);
        }
        self.gap = 1.0;
        self.it = 1;
        let cv = self.cv;
        let sig_nodes: Vec<usize> = self
            .g
            .ctl
            .iter()
            .enumerate()
            .filter(|(_, c)| c.kind == "signals" && c.signal.as_ref().is_some_and(|t| !t.single))
            .map(|(k, _)| k)
            .collect();
        let sig_opt = self.s.signal_opt.truthy() && !sig_nodes.is_empty();
        let loose = math_max(cv.gap_tol, 1e-3);
        let third = math_max(3.0, math_round(cv.max_iter / 3.0));
        if cv.stable && sig_opt {
            self.iterate(third, loose, false);
        } else {
            self.iterate(cv.max_iter, cv.gap_tol, true);
        }
        // Optimise signal cycle and green splits to the assigned flows (Webster), then re-equilibrate
        if sig_opt {
            for round in 0..2 {
                self.set_xt();
                for &k in &sig_nodes {
                    self.retime(k);
                }
                let (it, gap) = (self.it, self.gap);
                self.prog(it, gap, "Optimising signal timings");
                if !cv.stable {
                    self.iterate(math_round(cv.max_iter / 3.0), cv.gap_tol, true);
                } else if round == 0 {
                    self.iterate(third, loose, false);
                } else {
                    self.iterate(math_max(third, cv.max_iter - f64::from(self.it) + 1.0), cv.gap_tol, true);
                }
            }
        }
        // Reported capacities follow the final flows exactly (no damping)
        self.g.mv_detail = true;
        self.set_xt();
        self.sync_sm();
        update_node_caps(&mut self.g, &self.xt, 1.0, Some(&self.x_t));
        self.costs();
    }
}
