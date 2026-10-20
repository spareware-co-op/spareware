//! The steps of a run: releases, lane choice, junction rules, acceleration, movement and hand-over.

use super::run::{MicroSim, Pass};
use super::{
    Control, Kind, Light, Permit, Rng, Sample, Snapshot, Vehicle, ang_diff, js_max, js_min,
};

/// Position, length and speed of the vehicle ahead.
type Lead = (f64, f64, f64);

impl<R: Rng> MicroSim<R> {
    /// Advances the run by one time step (0.5 s unless the settings say otherwise).
    pub fn step(&mut self) {
        let dt = self.params.dt;
        self.t += dt;
        let t = self.t;
        // Release vehicles and let queued ones in
        for r in 0..self.routes.len() {
            let mut guard = 0;
            while self.routes[r].next <= t {
                if guard >= 50 {
                    break;
                }
                guard += 1;
                if self.routes[r].queue.len() < 60 {
                    let veh = self.new_vehicle(r);
                    self.routes[r].queue.push_back(veh);
                }
                self.stats.spawned += 1;
                let gap = if self.routes[r].route.bus {
                    1.0 / self.routes[r].route.rate
                } else {
                    self.gap_for(r)
                };
                self.routes[r].next += gap;
            }
            if !self.routes[r].queue.is_empty() {
                self.try_enter(r);
            }
        }
        let m = self.net.arcs.len();
        // Every vehicle chooses its acceleration from the positions at the start of the step
        for ai in 0..m {
            if self.count[ai] == 0 {
                continue;
            }
            for li in 0..self.lanes[ai].len() {
                for k in 0..self.lanes[ai][li].len() {
                    let lead = (k > 0).then(|| {
                        let l = &self.lanes[ai][li][k - 1];
                        (l.s, l.len, l.v)
                    });
                    let mut veh = self.lanes[ai][li][k];
                    self.accel(&mut veh, ai, lead);
                    self.lanes[ai][li][k] = veh;
                }
            }
        }
        // Every vehicle moves
        for ai in 0..m {
            if self.count[ai] == 0 {
                continue;
            }
            for lane in &mut self.lanes[ai] {
                for veh in lane.iter_mut() {
                    let nv = veh.v + veh.acc * dt;
                    veh.s += if nv < 0.0 {
                        veh.v * veh.v / (-2.0 * veh.acc)
                    } else {
                        (veh.v + nv) / 2.0 * dt
                    };
                    veh.v = js_max(0.0, nv);
                    veh.wait = if veh.v < 0.3 { veh.wait + dt } else { 0.0 };
                }
            }
        }
        // Front vehicles at the end of their arc move on or leave
        for ai in 0..m {
            if self.count[ai] == 0 {
                continue;
            }
            let a_len = self.net.arcs[ai].len;
            for li in 0..self.lanes[ai].len() {
                let mut guard = 0;
                while !self.lanes[ai][li].is_empty() {
                    if guard >= 20 {
                        break;
                    }
                    guard += 1;
                    let f = self.lanes[ai][li][0];
                    if f.s < a_len {
                        // A vehicle held at a line for two minutes is in a gridlock loop: move it on
                        let on_last_arc = f.k + 1 >= self.routes[f.route].route.arcs.len();
                        if f.wait > 120.0 && a_len - f.s < 10.0 && !on_last_arc {
                            let front = &mut self.lanes[ai][li][0];
                            front.s = a_len;
                            front.stuck = true;
                        } else {
                            break;
                        }
                    }
                    if !self.transfer(ai, li) {
                        break;
                    }
                }
            }
        }
        if (t % 5.0).abs() < dt / 2.0 {
            self.sample();
        }
    }

    // A new vehicle for route r: heavy or not by chance, a headway and a speed factor of its own
    fn new_vehicle(&mut self, r: usize) -> Vehicle {
        let bus = self.routes[r].route.bus;
        let heavy = if bus {
            false
        } else {
            self.rng.next_f64() < self.params.heavy_share
        };
        let kind = if bus {
            Kind::Bus
        } else if heavy {
            Kind::Heavy
        } else {
            Kind::Car
        };
        let car = kind == Kind::Car;
        let headway = if car {
            1.3 + self.rng.next_f64() * 0.4
        } else {
            1.7
        };
        let speed_factor = 0.9 + self.rng.next_f64() * 0.2;
        let first = self.routes[r].route.arcs[0];
        let id = self.nid;
        self.nid += 1;
        Vehicle {
            id,
            route: r,
            k: 0,
            arc: first,
            lane: 0,
            s: 0.0,
            v: 0.0,
            acc: 0.0,
            kind,
            len: if car { 4.6 } else { 12.0 },
            a_max: if car { 1.6 } else { 0.9 },
            headway,
            speed_factor,
            t0: self.t,
            ff: self.net.arcs[first].ff,
            permit: None,
            wait: 0.0,
            stopped: false,
            stuck: false,
        }
    }

    // The lane of arc ai the vehicle can enter, or None if no lane has `need` metres of room
    fn pick_lane(&self, ai: usize, kind: Kind, need: f64, pref: Option<i64>) -> Option<usize> {
        let a = &self.net.arcs[ai];
        let lanes = &self.lanes[ai];
        let mut lo: i64 = 0;
        let mut hi: i64 = lanes.len() as i64 - 1;
        if a.bus_lane {
            if kind == Kind::Bus {
                hi = 0;
            } else {
                lo = 1;
            }
        }
        let space = |l: i64| -> f64 {
            match usize::try_from(l)
                .ok()
                .and_then(|l| lanes.get(l))
                .and_then(|lane| lane.back())
            {
                Some(back) => back.s - back.len,
                None => 1e9,
            }
        };
        let mut best: i64 = -1;
        let mut bs = f64::NEG_INFINITY;
        for l in lo..=hi {
            let sp = space(l);
            if sp > bs {
                bs = sp;
                best = l;
            }
        }
        // Keep to the same lane position where it has nearly as much room (lane discipline through joins)
        if let Some(pref) = pref {
            let x = lo + pref;
            let p = if x < lo {
                lo
            } else if x > hi {
                hi
            } else {
                x
            };
            let sp = space(p);
            if sp >= need && sp >= bs - 15.0 {
                best = p;
                bs = sp;
            }
        }
        if bs >= need {
            usize::try_from(best).ok()
        } else {
            None
        }
    }

    // Is any vehicle on a conflicting arc due at the junction within tc seconds?
    fn gap_ok(&self, ai: usize, tc: f64) -> bool {
        let a = &self.net.arcs[ai];
        let last = self.node_last[a.to];
        for &j in &a.conflicts {
            if let Some(l) = last
                && l.arc == j
                && self.t - l.t < 1.6
            {
                return false;
            }
            let b = &self.net.arcs[j];
            for lane in &self.lanes[j] {
                let Some(f) = lane.front() else {
                    continue;
                };
                let d = b.len - f.s;
                if d > 150.0 {
                    continue;
                }
                if d / js_max(f.v, 1.5) < tc && (f.v > 1.5 || d < 6.0) {
                    return false;
                }
            }
        }
        true
    }

    // The lane of the next arc the vehicle may enter now, or None to hold at the line
    fn permission(&self, veh: &mut Vehicle, ai: usize, nxt: usize, to_end: f64) -> Option<usize> {
        let a = &self.net.arcs[ai];
        let nb = &self.net.arcs[nxt];
        if let Some(p) = veh.permit
            && p.arc == nxt
            && to_end < js_max(3.0, veh.v * veh.v / 8.0)
        {
            return Some(p.lane);
        }
        // At junctions a vehicle waits until it fits beyond the junction; on plain joins it simply follows the car ahead
        let junction = !matches!(a.control, Control::Free)
            || (!a.major
                && nb.lanes == 1
                && self.net.node_in[a.to] > 1
                && self.net.node_out[a.to] == 1);
        let pos = veh.lane as i64 - i64::from(a.bus_lane);
        let need = if junction { veh.len + 1.5 } else { -1e9 };
        let Some(li) = self.pick_lane(nxt, veh.kind, need, Some(pos.max(0))) else {
            veh.permit = None;
            return None;
        };
        let mut ok = true;
        match a.control {
            Control::Signal { .. } => {
                let st = self.light(ai);
                ok = st == Light::Green
                    || (st == Light::Amber && to_end < veh.v * veh.v / 6.0 + 1.0);
            }
            Control::Minor | Control::Roundabout => {
                let rbt = a.control == Control::Roundabout;
                let forced = veh.wait > if rbt { 8.0 } else { 25.0 };
                if a.stop && !veh.stopped {
                    ok = false;
                } else if forced {
                    ok = match self.node_last[a.to] {
                        None => true,
                        Some(l) => self.t - l.t > 2.0,
                    };
                } else {
                    ok = to_end < 30.0 && self.gap_ok(ai, if rbt { 2.6 } else { 4.2 });
                }
            }
            Control::Free => {
                if !a.major && nb.lanes == 1 {
                    // Uncontrolled merge into a single lane: approaches take turns (zip merge)
                    ok = match self.merge_last[nxt] {
                        None => true,
                        Some(l) => l.arc == ai || self.t - l.t > 1.2,
                    };
                }
            }
        }
        if !ok {
            veh.permit = None;
            return None;
        }
        veh.permit = Some(Permit { arc: nxt, lane: li });
        Some(li)
    }

    // The Intelligent Driver Model: the vehicle's acceleration from its speed, the gap and speed of what is ahead
    fn accel(&self, veh: &mut Vehicle, ai: usize, leader: Option<Lead>) {
        let a = &self.net.arcs[ai];
        let route = &self.routes[veh.route].route.arcs;
        let mut v0 = a.speed / 3.6 * veh.speed_factor;
        if veh.kind != Kind::Car {
            v0 = js_min(v0, 25.0);
        }
        let mut gap = f64::INFINITY;
        let mut lv = 0.0;
        let to_end = a.len - veh.s;
        if let Some((ls, ll, lvel)) = leader {
            gap = ls - ll - veh.s;
            lv = lvel;
        } else if veh.k + 1 < route.len() {
            let nxt = route[veh.k + 1];
            let b = &self.net.arcs[nxt];
            let turn = ang_diff(a.in_bearing, b.out_bearing);
            if turn > 50.0 && to_end < 45.0 {
                v0 = js_min(v0, if turn > 140.0 { 3.5 } else { 7.0 });
            }
            if a.control == Control::Roundabout && to_end < 40.0 {
                v0 = js_min(v0, 7.0);
            }
            if to_end < js_max(70.0, veh.v * 7.0) {
                match self.permission(veh, ai, nxt, to_end) {
                    None => {
                        gap = to_end - 1.0;
                        lv = 0.0;
                    }
                    Some(li) => {
                        if let Some(back) = self.lanes[nxt][li].back() {
                            gap = to_end + back.s - back.len;
                            lv = back.v;
                        }
                    }
                }
            }
        }
        let am = veh.a_max;
        let b = 2.4;
        let s0 = 2.0;
        let mut acc = am * (1.0 - (js_max(veh.v, 0.0) / v0).powf(4.0));
        if gap < f64::INFINITY {
            let ss = s0
                + js_max(
                    0.0,
                    veh.v * veh.headway + veh.v * (veh.v - lv) / (2.0 * (am * b).sqrt()),
                );
            let q = ss / js_max(gap, 0.25);
            acc -= am * (q * q);
        }
        veh.acc = js_max(acc, -9.0);
        if leader.is_none() && to_end < 6.0 && veh.v < 0.6 {
            veh.stopped = true;
        }
    }

    // Lets queued vehicles of route r onto its first arc while there is room and the vehicle limit allows
    fn try_enter(&mut self, r: usize) {
        while !self.routes[r].queue.is_empty() && (self.n as f64) < self.params.max_vehicles {
            let mut veh = self.routes[r].queue[0];
            let ai = self.routes[r].route.arcs[0];
            let Some(li) = self.pick_lane(ai, veh.kind, veh.len + 3.0, None) else {
                return;
            };
            let back_v = self.lanes[ai][li].back().map(|b| b.v);
            veh.lane = li;
            veh.s = 0.0;
            veh.v = js_min(self.net.arcs[ai].speed / 3.6 * 0.6, back_v.unwrap_or(99.0));
            self.lanes[ai][li].push_back(veh);
            self.count[ai] += 1;
            self.n += 1;
            self.routes[r].queue.pop_front();
        }
    }

    // A vehicle has left the network at the end of its route
    fn finish(&mut self, veh: &Vehicle) {
        self.n -= 1;
        if veh.kind == Kind::Bus {
            self.stats.bus_done += 1;
            self.stats.bus_tt += self.t - veh.t0;
            return;
        }
        self.stats.done += 1;
        self.stats.tt += self.t - veh.t0;
        self.stats.ff += veh.ff;
    }

    // Moves the front vehicle of a lane onto the next arc (or out of the network); false if it has to wait
    fn transfer(&mut self, ai: usize, li: usize) -> bool {
        let a_len = self.net.arcs[ai].len;
        let a_to = self.net.arcs[ai].to;
        let mut veh = self.lanes[ai][li][0];
        let route_len = self.routes[veh.route].route.arcs.len();
        if veh.k + 1 == route_len {
            self.lanes[ai][li].pop_front();
            self.count[ai] -= 1;
            self.passed[ai] += 1.0;
            self.finish(&veh);
            return true;
        }
        let nxt = self.routes[veh.route].route.arcs[veh.k + 1];
        let mut nl: Option<usize> = match veh.permit {
            Some(p) if p.arc == nxt => Some(p.lane),
            _ => None,
        };
        if nl.is_none() && veh.stuck {
            nl = self.pick_lane(nxt, veh.kind, -1e9, None);
            self.stats.teleports += 1;
        }
        veh.stuck = false;
        let Some(nl) = nl else {
            veh.s = a_len - 0.01;
            veh.v = 0.0;
            self.lanes[ai][li][0] = veh;
            return false;
        };
        self.lanes[ai][li].pop_front();
        self.count[ai] -= 1;
        self.passed[ai] += 1.0;
        let back = self.lanes[nxt][nl].back().map(|b| (b.s, b.len));
        veh.s = js_max(0.0, veh.s - a_len);
        if let Some((bs, bl)) = back {
            veh.s = js_min(veh.s, js_max(0.0, bs - bl - 0.5));
        }
        veh.k += 1;
        veh.arc = nxt;
        veh.lane = nl;
        veh.permit = None;
        veh.stopped = false;
        veh.wait = 0.0;
        veh.ff += self.net.arcs[nxt].ff;
        self.lanes[nxt][nl].push_back(veh);
        self.count[nxt] += 1;
        self.node_last[a_to] = Some(Pass { t: self.t, arc: ai });
        self.merge_last[nxt] = Some(Pass { t: self.t, arc: ai });
        true
    }

    // Records the whole network every 5 s, and the flow on every arc every minute
    fn sample(&mut self) {
        let (mut q, mut vs, mut n) = (0usize, 0.0f64, 0usize);
        for ai in 0..self.net.arcs.len() {
            if self.count[ai] == 0 {
                continue;
            }
            for lane in &self.lanes[ai] {
                for v in lane {
                    n += 1;
                    vs += v.v;
                    if v.v < 1.5 {
                        q += 1;
                    }
                }
            }
        }
        let waiting: usize = self.routes.iter().map(|r| r.queue.len()).sum();
        self.stats.series.push(Sample {
            t: self.t,
            n,
            q,
            speed: if n != 0 { vs / n as f64 * 3.6 } else { 0.0 },
            done: self.stats.done,
            waiting,
        });
        if self.stats.series.len() > 1440 {
            self.stats.series.remove(0);
        }
        if (self.t % 60.0).abs() < self.params.dt / 2.0 {
            self.snaps.push(Snapshot {
                t: self.t,
                passed: self.passed.clone(),
            });
            if self.snaps.len() > 12 {
                self.snaps.remove(0);
            }
        }
    }
}
