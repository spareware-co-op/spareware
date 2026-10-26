//! The equilibrium loop: all-or-nothing loading, conjugate Frank-Wolfe steps, junction capacity
//! updates, convergence, signal re-timing and the tracked flows that ride along with the main
//! flows. Port of engine.js `assign`, up to the point where the results are made.
//!
//! The JavaScript keeps all of this in the closures of one function; here it is one struct, with
//! its arrays (flows, costs, directions) as plain vectors in the same order and the same loops,
//! so the floating-point sums come out in the same order.

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
    pub od: usize,
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

/// The progress callback: iteration, gap and a message ("" for none).
pub(super) type Progress<'p> = &'p mut (dyn FnMut(u32, f64, &str) + 'p);

/// One assignment in progress.
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
    pub dev_gw: Vec<(usize, f64, f64)>,
    pub dev_zone: Vec<(usize, f64, f64)>,
    pub sel: Option<Sel>,
    pub trips: f64,
    pub store_paths: bool,
    pub paths: Option<PathSet>,
    sp: Option<PathSet>,
    pub unserved: f64,
    pub trk: Vec<Trk>,
    pub ix: TrkIx,
    sm_on: bool,
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
        let m = g.m;
        let mut pre = vec![0.0f64; m];
        let bus_pcu = bus_pcu_of(s);
        let routes: Vec<Route> = sc.routes.values().cloned().collect();
        let route_arcs: Vec<Vec<usize>> = routes.iter().map(|r| g.resolve_route(r, None)).collect();
        for (k, r) in routes.iter().enumerate() {
            let f = bus_freq(r, Some(period));
            for &ai in &route_arcs[k] {
                if !g.arcs[ai].bus {
                    pre[ai] += f * bus_pcu;
                }
            }
        }
        /* Additive generalised cost per arc (s): time-of-use charges (L.toll via buildGraph, and opts.arcPenalty keyed by
        arc key or link id). Fixed per traversal, so they add no curvature; they are not time (see results). */
        let mut apen = vec![0.0f64; m];
        let ap = match &opts.arc_penalty {
            Some(Value::Object(o)) => Some(o),
            _ => None,
        };
        let mut has_pen = false;
        for a in 0..m {
            let arc = &g.arcs[a];
            if arc.conn {
                continue;
            }
            let mut p = arc.toll;
            if let Some(ap) = ap {
                let pick = |k: &str| ap.get(k).filter(|v| !v.is_null());
                let v = match (pick(&arc.key), pick(&arc.link)) {
                    (Some(v), _) => crate::jsnum::value_to_number(v),
                    (None, Some(v)) => crate::jsnum::value_to_number(v),
                    (None, None) => 0.0,
                };
                if v > 0.0 && v.is_finite() {
                    p += v;
                }
            }
            if p > 0.0 {
                apen[a] = p;
                has_pen = true;
            }
        }
        let nt = g.n_t;
        let pc: Vec<f64> = g.tpen().iter().map(|&p| if p == f64::INFINITY { 0.0 } else { p }).collect();
        let zc: Vec<usize> = g.zones.iter().map(|z| zc_idx(Some(z))).collect();
        let is_gw: Vec<bool> = g.zones.iter().map(|z| z.kind.is("gateway")).collect();
        // Selection: a zone (OD pairs with either end there), an arc or a turn (paths that use it)
        let req = opts.select.clone().filter(value_truthy);
        let get = |k: &str| req.as_ref().and_then(|r| r.get(k)).filter(|v| !v.is_null());
        let sel_zone: i64 = match get("zone") {
            Some(z) => g
                .zones
                .iter()
                .position(|zz| z.as_str() == Some(zz.id.as_str()))
                .map_or(-1, |p| p as i64),
            None => -1,
        };
        let sel_arc: i64 = match get("arc").and_then(|v| v.as_str()) {
            Some(k) => g.key_idx.get(k).map_or(-1, |&a| a as i64),
            None => -1,
        };
        let mut sel_turn: i64 = -1;
        if let Some(Value::Array(t)) = req.as_ref().and_then(|r| r.get("turn")) {
            let a = t.first().and_then(|v| v.as_str()).and_then(|k| g.key_idx.get(k));
            let b = t.get(1).and_then(|v| v.as_str()).and_then(|k| g.key_idx.get(k));
            if let (Some(&a), Some(&b)) = (a, b) {
                sel_turn = g.turn_idx(a, b) as i64;
            }
        }
        let sel_on = req.is_some() && (sel_zone >= 0 || sel_arc >= 0 || sel_turn >= 0);
        let sel = req.map(|req| Sel {
            req,
            zone: sel_zone,
            arc: sel_arc,
            turn: sel_turn,
            on: sel_on,
        });
        let mut od_list: Vec<Vec<Od>> = Vec::with_capacity(zn);
        let mut cls_trips = [0.0f64; 3];
        let mut dev_gw: Vec<(usize, f64, f64)> = Vec::new();
        let mut dev_zone: Vec<(usize, f64, f64)> = Vec::new();
        let (mut trips, mut n_od) = (0.0f64, 0usize);
        for i in 0..zn {
            let mut l = Vec::new();
            for j in 0..zn {
                let q = dem.t[i * zn + j];
                if !(q > 0.0005) {
                    continue;
                }
                let c = zc[i].max(zc[j]);
                l.push(Od {
                    j,
                    q: q * pf,
                    c,
                    od: n_od,
                });
                n_od += 1;
                trips += q;
                cls_trips[c] += q;
                if c == 2 {
                    // development trips: where they cross the boundary, and each development zone's own trips
                    let gi = if is_gw[i] {
                        Some(i)
                    } else if is_gw[j] {
                        Some(j)
                    } else {
                        None
                    };
                    if let Some(gi) = gi {
                        let at = dev_gw.iter().position(|r| r.0 == gi).unwrap_or_else(|| {
                            dev_gw.push((gi, 0.0, 0.0));
                            dev_gw.len() - 1
                        });
                        if gi == i {
                            dev_gw[at].1 += q;
                        } else {
                            dev_gw[at].2 += q;
                        }
                    }
                    if zc[i] == 2 {
                        let at = dev_zone.iter().position(|r| r.0 == i).unwrap_or_else(|| {
                            dev_zone.push((i, 0.0, 0.0));
                            dev_zone.len() - 1
                        });
                        dev_zone[at].1 += q;
                    }
                    if zc[j] == 2 {
                        let at = dev_zone.iter().position(|r| r.0 == j).unwrap_or_else(|| {
                            dev_zone.push((j, 0.0, 0.0));
                            dev_zone.len() - 1
                        });
                        dev_zone[at].2 += q;
                    }
                }
            }
            od_list.push(l);
        }
        let store_paths = zn <= 300;
        // Tracked flows [x, s (conjugate direction), y (all-or-nothing)]: committed and development link flows, development
        // turn flows, and the selection's link, turn and OD flows; all are averaged with the main flows' steps
        let mut trk: Vec<Trk> = vec![Trk::new(m), Trk::new(m), Trk::new(nt)];
        let mut ix = TrkIx::default();
        if sel_on {
            ix.s = Some(trk.len());
            trk.push(Trk::new(m));
            ix.ts = Some(trk.len());
            trk.push(Trk::new(nt));
            ix.od = Some(trk.len());
            trk.push(Trk::new(n_od));
        }
        /* Junction-site movements traced along each path (entry arc to exit arc), all traffic and development traffic.
        Walking a path backwards, an exit arc opens its site and the site's entry arc closes it; an arc between two
        sites closes the downstream one before it opens the upstream one. */
        let sm_n = g.sm.as_ref().map_or(0, |sm| sm.n);
        let sm_on = sm_n > 0;
        if sm_on {
            ix.m = Some(trk.len());
            trk.push(Trk::new(sm_n));
            ix.md = Some(trk.len());
            trk.push(Trk::new(sm_n));
        }
        let pend = vec![-1i32; g.sm.as_ref().map_or(0, |sm| sm.list.len())];
        if sm_on {
            g.sm_x = Some(vec![0.0; sm_n]);
        }
        g.mv_detail = false; // lane rows for the junction tables are built on the last capacity update only
        let cv = conv_set(
            s,
            &ConvOpts {
                gap_tol: Some(opts.gap_tol.num()),
                max_iter: Some(opts.max_iter.num()),
            },
        );
        let w = g.workspace();
        Run {
            perf: PerfSet::new(s),
            s,
            period: period.to_string(),
            cv,
            m,
            nt,
            zn,
            pf,
            bus_pcu,
            routes,
            route_arcs,
            pre,
            apen,
            has_pen,
            x: vec![0.0; m],
            y: vec![0.0; m],
            cost: vec![0.0; m],
            xt: vec![0.0; m],
            x_t: vec![0.0; nt],
            y_t: vec![0.0; nt],
            pc,
            w,
            od_list,
            cls_trips,
            dev_gw,
            dev_zone,
            sel,
            trips,
            store_paths,
            paths: store_paths.then(PathSet::new),
            sp: store_paths.then(PathSet::new),
            unserved: 0.0,
            trk,
            ix,
            sm_on,
            pend,
            opened: Vec::new(),
            s_dir: vec![0.0; m],
            s_t: vec![0.0; nt],
            has_s: false,
            conv: Vec::new(),
            x_prev: vec![0.0; m],
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
        for a in 0..self.m {
            self.xt[a] = self.x[a] + self.pre[a];
        }
    }

    fn costs(&mut self) {
        for a in 0..self.m {
            self.cost[a] = arc_time(&self.g.arcs[a], self.x[a] + self.pre[a], &self.perf) + self.apen[a];
        }
    }

    // `G.smX` is the traced site flows xM, live in the JavaScript; here it is copied in before each capacity update
    fn sync_sm(&mut self) {
        if let (Some(i), Some(buf)) = (self.ix.m, self.g.sm_x.as_mut()) {
            buf.copy_from_slice(&self.trk[i].x);
        }
    }

    // `caps()`: junction capacities from the current flows; MSA damping (1 / kCap) after the first update of a phase
    fn caps(&mut self) {
        self.k_cap += 1;
        let w = if self.cv.msa && self.k_cap > 1 { 1.0 / self.k_cap as f64 } else { 1.0 };
        self.sync_sm();
        update_node_caps(&mut self.g, &self.xt, w, Some(&self.x_t));
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
        let (sel_on, sel_zone, sel_arc, sel_turn) = match &self.sel {
            Some(s) => (s.on, s.zone, s.arc, s.turn),
            None => (false, -1, -1, -1),
        };
        for i in 0..zn {
            if self.od_list[i].is_empty() {
                continue;
            }
            self.g.spt(&self.cost, self.g.zone_node[i], &mut self.w, -1, -1);
            for k in 0..self.od_list[i].len() {
                let Od { j, q, c, od } = self.od_list[i][k];
                let dn = self.g.zone_node[j];
                if !self.w.nd[dn].is_finite() {
                    self.unserved += q;
                    continue;
                }
                let mut p: Option<Vec<usize>> = np.is_some().then(Vec::new);
                let ycl = match c {
                    2 => Some(T_C2),
                    1 => Some(T_C1),
                    _ => None,
                };
                let mut ai = self.w.na[dn];
                while ai >= 0 {
                    let a = ai as usize;
                    self.y[a] += q;
                    let pt = self.w.pt[a];
                    if pt >= 0 {
                        self.y_t[pt as usize] += q;
                        if c == 2 {
                            self.trk[T_TD].y[pt as usize] += q;
                        }
                    }
                    if let Some(t) = ycl {
                        self.trk[t].y[a] += q;
                    }
                    if let Some(p) = p.as_mut() {
                        p.push(a);
                    }
                    if self.sm_on {
                        let sm = self.g.sm.as_ref().expect("site moves");
                        let (xm, xd) = (self.ix.m.expect("traced flows"), self.ix.md.expect("traced flows"));
                        let s = sm.ent_s[a];
                        if s >= 0 && self.pend[s as usize] >= 0 {
                            let st = &sm.list[s as usize];
                            let kk = st.off + sm.ent_i[a] as usize * st.nx + self.pend[s as usize] as usize;
                            self.trk[xm].y[kk] += q;
                            if c == 2 {
                                self.trk[xd].y[kk] += q;
                            }
                            self.pend[s as usize] = -1;
                        }
                        let s = sm.ext_s[a];
                        if s >= 0 {
                            self.pend[s as usize] = sm.ext_i[a];
                            self.opened.push(s as usize);
                        }
                    }
                    ai = self.w.pred[a];
                }
                if !self.opened.is_empty() {
                    for &s in &self.opened {
                        self.pend[s] = -1;
                    }
                    self.opened.clear();
                }
                if sel_on {
                    let mut hit = i as i64 == sel_zone || j as i64 == sel_zone;
                    if !hit {
                        let mut a2 = self.w.na[dn];
                        while a2 >= 0 {
                            if a2 as i64 == sel_arc || (sel_turn >= 0 && self.w.pt[a2 as usize] as i64 == sel_turn) {
                                hit = true;
                                break;
                            }
                            a2 = self.w.pred[a2 as usize];
                        }
                    }
                    if hit {
                        let (xs, xts, xod) = (
                            self.ix.s.expect("selection"),
                            self.ix.ts.expect("selection"),
                            self.ix.od.expect("selection"),
                        );
                        self.trk[xod].y[od] += q;
                        let mut a2 = self.w.na[dn];
                        while a2 >= 0 {
                            let a = a2 as usize;
                            self.trk[xs].y[a] += q;
                            let pt = self.w.pt[a];
                            if pt >= 0 {
                                self.trk[xts].y[pt as usize] += q;
                            }
                            a2 = self.w.pred[a];
                        }
                    }
                }
                if let Some(np) = np.as_mut()
                    && let Some(mut p) = p
                {
                    p.reverse();
                    np.push((i * zn + j, p, q));
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
            for a in 0..self.m {
                let ds = self.s_dir[a] - self.x[a];
                if ds == 0.0 {
                    continue;
                }
                let q = self.x[a] + self.pre[a];
                let h = math_max(1.0, q * 0.01);
                let hs = (arc_time(&self.g.arcs[a], q + h, &self.perf) + self.apen[a] - self.cost[a]) / h;
                num += ds * hs * (self.y[a] - self.x[a]);
                den += ds * hs * (self.y[a] - self.s_dir[a]);
            }
            al = if den != 0.0 { num / den } else { 0.0 };
            al = if al > 0.99 {
                0.99
            } else if al > 0.0 {
                al
            } else {
                0.0
            };
        }
        for a in 0..self.m {
            self.s_dir[a] = al * self.s_dir[a] + (1.0 - al) * self.y[a];
        }
        for t in 0..self.nt {
            self.s_t[t] = al * self.s_t[t] + (1.0 - al) * self.y_t[t];
        }
        for tr in &mut self.trk {
            for k in 0..tr.s.len() {
                tr.s[k] = al * tr.s[k] + (1.0 - al) * tr.y[k];
            }
        }
        if let Some(np) = np {
            let sp = self.sp.as_mut().expect("paths are stored");
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
        for a in 0..self.m {
            let d = self.s_dir[a] - self.x[a];
            if d != 0.0 {
                r += d * (arc_time(&self.g.arcs[a], self.x[a] + lam * d + self.pre[a], &self.perf) + self.apen[a]);
            }
        }
        for t in 0..self.nt {
            if truthy_number(self.pc[t]) {
                r += (self.s_t[t] - self.x_t[t]) * self.pc[t];
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
            let m = (lo + hi) / 2.0;
            if self.derivative(m) > 0.0 {
                hi = m;
            } else {
                lo = m;
            }
        }
        (lo + hi) / 2.0
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
            let mut np: Option<Vec<NewPath>> = self.store_paths.then(Vec::new);
            self.aon(np.as_mut());
            let (mut num, mut den, mut vh) = (0.0f64, 0.0f64, 0.0f64);
            for a in 0..self.m {
                num += self.cost[a] * (self.x[a] - self.y[a]);
                den += self.cost[a] * self.x[a];
                if !self.g.arcs[a].conn {
                    vh += self.x[a] * (self.cost[a] - self.apen[a]);
                }
            }
            for t in 0..self.nt {
                if truthy_number(self.pc[t]) {
                    num += self.pc[t] * (self.x_t[t] - self.y_t[t]);
                    den += self.pc[t] * self.x_t[t];
                    vh += self.pc[t] * self.x_t[t];
                }
            }
            self.gap = if den > 0.0 { (num / den).abs() } else { 0.0 };
            self.it += 1;
            self.conv.push(ConvRow {
                it: self.it as f64,
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
            for a in 0..self.m {
                self.x[a] += lam * (self.s_dir[a] - self.x[a]);
            }
            for t in 0..self.nt {
                self.x_t[t] += lam * (self.s_t[t] - self.x_t[t]);
            }
            for tr in &mut self.trk {
                for kk in 0..tr.x.len() {
                    tr.x[kk] += lam * (tr.s[kk] - tr.x[kk]);
                }
            }
            let (mut n_p, mut ok_p) = (0u32, 0u32);
            for a in 0..self.m {
                if self.g.arcs[a].conn || !(self.x_prev[a] > 1.0) {
                    continue;
                }
                n_p += 1;
                if (self.x[a] - self.x_prev[a]).abs() < self.cv.chg * self.x_prev[a] {
                    ok_p += 1;
                }
            }
            let last_p = if n_p > 0 { ok_p as f64 / n_p as f64 } else { 1.0 };
            self.last_p = Some(last_p);
            self.consec_p = if last_p > self.cv.share { self.consec_p + 1.0 } else { 0.0 };
            self.hist.push_back(self.x.clone());
            if self.hist.len() > 6 {
                self.hist.pop_front();
            }
            if self.store_paths {
                let paths = self.paths.as_mut().expect("paths are stored");
                scale_paths(paths, 1.0 - lam);
                merge_paths(paths, self.sp.as_ref().expect("paths are stored"), lam);
            }
            if self.it.is_multiple_of(2) {
                let (it, gap) = (self.it, self.gap);
                self.prog(it, gap, "");
            }
            k += 1.0;
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
        let mut np: Option<Vec<NewPath>> = self.store_paths.then(Vec::new);
        self.aon(np.as_mut());
        self.x.copy_from_slice(&self.y);
        self.x_t.copy_from_slice(&self.y_t);
        for tr in &mut self.trk {
            tr.x.copy_from_slice(&tr.y);
        }
        if let Some(np) = np {
            add_paths(np, 1.0, self.paths.as_mut().expect("paths are stored"));
        }
        self.gap = 1.0;
        self.it = 1;
        let cv = self.cv;
        let sig_nodes: Vec<usize> = (0..self.g.ctl.len())
            .filter(|&k| {
                let c = &self.g.ctl[k];
                c.kind == "signals" && c.signal.as_ref().is_some_and(|t| !t.single)
            })
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
                    // a site's own cycle and fixed timing, or the node's
                    if self.g.ctl[k].src.fixed_timing.truthy() {
                        continue;
                    }
                    let mut yv = [0.02f64, 0.02f64];
                    for &ai in &self.g.ctl[k].arcs {
                        let a = &self.g.arcs[ai];
                        let ph = a.phase.unwrap_or(0) as usize;
                        yv[ph] = math_max(yv[ph], self.xt[ai] / (a.gl as f64 * self.s.sat_flow.num()));
                    }
                    let yy = math_min(0.92, yv[0] + yv[1]);
                    let cycle = self.g.ctl[k].src.cycle.num();
                    let ctl = &mut self.g.ctl[k];
                    let timing = ctl.signal.as_mut().expect("signal timing");
                    timing.c = or_default(cycle, math_round(clamp((1.5 * timing.lost + 5.0) / (1.0 - yy), 50.0, 140.0)));
                    let s0 = clamp(yv[0] / (yv[0] + yv[1]), 0.2, 0.8);
                    timing.split = vec![s0, 1.0 - s0];
                    crate::graph::apply_signal(&self.g.ctl[k], &mut self.g.arcs);
                }
                let (it, gap) = (self.it, self.gap);
                self.prog(it, gap, "Optimising signal timings");
                if !cv.stable {
                    self.iterate(math_round(cv.max_iter / 3.0), cv.gap_tol, true);
                } else if round == 0 {
                    self.iterate(third, loose, false);
                } else {
                    self.iterate(math_max(third, cv.max_iter - self.it as f64 + 1.0), cv.gap_tol, true);
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
