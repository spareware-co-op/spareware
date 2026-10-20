'use strict';
/* Spare Construct · microsimulation. Vehicles are released on the assigned paths and follow the
   Intelligent Driver Model; signals run their optimised timings, minor roads give way by
   gap acceptance, roundabout entries yield to circulating traffic. Left-hand traffic. */

class Sim {
  constructor(sc, res, opts = {}) {
    const S = Store.S;
    this.S = S; this.sc = sc; this.res = res;
    // The period's own speeds (variable limits) and bus frequencies (r.freqs) apply, as in the assignment
    this.period = (res && res.period) || S.period;
    this.G = Engine.buildGraph(sc, S, this.period);
    const G = this.G;
    if (res) for (const ctl of G.ctl.values()) {
      const nr = res.nodes[G.ids[ctl.n]];
      if (ctl.type === 'signals' && nr && nr.C) { ctl.C = nr.C; if (nr.split) ctl.split = nr.split; Engine.applySignal(ctl, G); }
    }
    this.t = 0; this.dt = 0.5;
    this.scale = opts.scale ?? S.sim.scale;
    this.maxVeh = S.sim.maxVeh;
    this.hv = S.hvPct / 100;
    this.pf = Engine.pcuF(S);
    this.startClock = this.period === 'AM' ? 7.5 * 3600 : 16.5 * 3600;
    this.lanes = G.arcs.map(a => a.conn ? null : Array.from({ length: a.gl + (a.bus ? 1 : 0) }, () => []));
    this.count = new Int32Array(G.M);   // vehicles on each arc
    this.passed = new Float64Array(G.M); // vehicles that left each arc
    this.nodeLast = new Map();           // node → { t, ai } of the last vehicle to enter it
    this.mergeLast = new Map();          // arc → { t, ai } of the last vehicle to join it
    this.routes = [];
    this.n = 0; this.nid = 0;
    this.stats = { done: 0, tt: 0, ff: 0, spawned: 0, teleports: 0, series: [], busDone: 0, busTT: 0 };
    this.buildRoutes();
  }

  buildRoutes() {
    const G = this.G, res = this.res;
    if (res && res.paths && res.arcKeys) {
      const map = res.arcKeys.map(k => G.keyIdx.has(k) ? G.keyIdx.get(k) : -1);
      for (const od of res.paths) for (const p of od.list) {
        const arcs = [];
        let ok = true;
        for (const ai of p.arcs) { const m = map[ai]; if (m < 0) { ok = false; break; } if (!G.arcs[m].conn) arcs.push(m); }
        if (!ok || !arcs.length) continue;
        const rate = p.w / this.pf / 3600;
        if (rate > 0) this.routes.push({ arcs, rate, next: 0, q: [], type: 'car' });
      }
    }
    for (const r of Object.values(this.sc.routes || {})) {
      const arcs = Engine.resolveRoute(G, r).filter(i => !G.arcs[i].conn);
      const freq = Engine.busFreq ? Engine.busFreq(r, this.period) : +r.freq || 0;
      if (arcs.length && freq > 0) this.routes.push({ arcs, rate: freq / 3600, next: 0, q: [], type: 'bus', color: r.color, rid: r.id });
    }
    for (const r of this.routes) r.next = r.type === 'bus' ? Math.random() * 3600 / (r.rate * 3600) : this.gapFor(r);
  }
  gapFor(r) { return -Math.log(1 - Math.random()) / Math.max(1e-9, r.rate * this.scale); }

  newVeh(r) {
    const hv = r.type === 'bus' ? false : Math.random() < this.hv;
    const type = r.type === 'bus' ? 'bus' : hv ? 'hv' : 'car';
    return {
      id: this.nid++, r, k: 0, ai: r.arcs[0], li: 0, s: 0, v: 0, acc: 0, type,
      len: type === 'car' ? 4.6 : 12, amax: type === 'car' ? 1.6 : 0.9, T: type === 'car' ? 1.3 + Math.random() * 0.4 : 1.7,
      v0f: 0.9 + Math.random() * 0.2, t0: this.t, ff: this.G.arcs[r.arcs[0]].ff, perm: null, wait: 0, stopped: false,
    };
  }

  // Lane on arc ai that the vehicle can enter, or -1 if none has room
  pickLane(ai, veh, need, pref = null) {
    const A = this.G.arcs[ai], lanes = this.lanes[ai];
    let lo = 0, hi = lanes.length - 1;
    if (A.bus) { if (veh.type === 'bus') hi = 0; else lo = 1; }
    const space = l => { const L = lanes[l], back = L.length ? L[L.length - 1] : null; return back ? back.s - back.len : 1e9; };
    let best = -1, bs = -Infinity;
    for (let l = lo; l <= hi; l++) { const sp = space(l); if (sp > bs) { bs = sp; best = l; } }
    // Keep to the same lane position where it has nearly as much room (lane discipline through joins)
    if (pref != null) {
      const p = U.clamp(lo + pref, lo, hi), sp = space(p);
      if (sp >= need && sp >= bs - 15) { best = p; bs = sp; }
    }
    return bs >= need ? best : -1;
  }

  signal(A) {
    const ctl = this.G.ctl.get(A.to);
    const C = ctl.C, lostEach = ctl.lost / 2, gtot = C - ctl.lost;
    const g0 = gtot * ctl.split[0], g1 = gtot * ctl.split[1];
    const tc = (this.t + ctl.offset) % C, ph = A.phase || 0;
    const start = ph === 0 ? 0 : g0 + lostEach, g = ph === 0 ? g0 : g1;
    const u = (tc - start + C) % C;
    return u < g ? 'G' : u < g + 3 ? 'A' : 'R';
  }

  // Is any conflicting vehicle due at the junction within tc seconds?
  gapOK(A, ai, tc) {
    const n = A.to, last = this.nodeLast.get(n);
    for (const [j] of A.conf || []) {
      if (last && last.ai === j && this.t - last.t < 1.6) return false;
      const B = this.G.arcs[j], lanes = this.lanes[j];
      if (!lanes) continue;
      for (const L of lanes) {
        const f = L[0]; if (!f) continue;
        const d = B.len - f.s;
        if (d > 150) continue;
        if (d / Math.max(f.v, 1.5) < tc && (f.v > 1.5 || d < 6)) return false;
      }
    }
    return true;
  }

  permission(veh, ai, nxt, toEnd) {
    const A = this.G.arcs[ai];
    if (veh.perm && veh.perm.ai === nxt && toEnd < Math.max(3, veh.v * veh.v / 8)) return veh.perm.li;
    // At junctions a vehicle waits until it fits beyond the junction; on plain joins it simply follows the car ahead
    const junction = A.ctrl !== 'free' || (!A.major && this.G.arcs[nxt].gl === 1 && this.G.into[A.to].length > 1 && this.G.outR[A.to].length === 1);
    const pos = veh.li - (A.bus ? 1 : 0);
    const li = this.pickLane(nxt, veh, junction ? veh.len + 1.5 : -1e9, Math.max(0, pos));
    if (li < 0) { veh.perm = null; return -1; }
    let ok = true;
    if (A.ctrl === 'sig') {
      const st = this.signal(A);
      ok = st === 'G' || (st === 'A' && toEnd < veh.v * veh.v / 6 + 1);
    } else if (A.ctrl === 'minor' || A.ctrl === 'rbt') {
      const forced = veh.wait > (A.ctrl === 'rbt' ? 8 : 25);
      if (A.stop && !veh.stopped) ok = false;
      else if (forced) { const last = this.nodeLast.get(A.to); ok = !last || this.t - last.t > 2; }
      else ok = toEnd < 30 && this.gapOK(A, ai, A.ctrl === 'rbt' ? 2.6 : 4.2);
    } else if (!A.major && this.G.arcs[nxt].gl === 1) {
      // Uncontrolled merge into a single lane: approaches take turns (zip merge)
      const last = this.mergeLast.get(nxt);
      ok = !last || last.ai === ai || this.t - last.t > 1.2;
    }
    if (!ok) { veh.perm = null; return -1; }
    veh.perm = { ai: nxt, li };
    return li;
  }

  accel(veh, ai, leader) {
    const G = this.G, A = G.arcs[ai];
    let v0 = A.speed / 3.6 * veh.v0f;
    if (veh.type !== 'car') v0 = Math.min(v0, 25);
    let gap = Infinity, lv = 0;
    const toEnd = A.len - veh.s;
    if (leader) { gap = leader.s - leader.len - veh.s; lv = leader.v; }
    else if (veh.k < veh.r.arcs.length - 1) {
      const nxt = veh.r.arcs[veh.k + 1], B = G.arcs[nxt];
      const turn = U.angDiff(A.inB, B.outB);
      if (turn > 50 && toEnd < 45) v0 = Math.min(v0, turn > 140 ? 3.5 : 7);
      if (A.ctrl === 'rbt' && toEnd < 40) v0 = Math.min(v0, 7);
      if (toEnd < Math.max(70, veh.v * 7)) {
        const li = this.permission(veh, ai, nxt, toEnd);
        if (li < 0) { gap = toEnd - 1; lv = 0; }
        else {
          const tl = this.lanes[nxt][li], back = tl.length ? tl[tl.length - 1] : null;
          if (back) { gap = toEnd + back.s - back.len; lv = back.v; }
        }
      }
    }
    const a = veh.amax, b = 2.4, s0 = 2;
    let acc = a * (1 - Math.pow(Math.max(veh.v, 0) / v0, 4));
    if (gap < Infinity) {
      const ss = s0 + Math.max(0, veh.v * veh.T + veh.v * (veh.v - lv) / (2 * Math.sqrt(a * b)));
      acc -= a * Math.pow(ss / Math.max(gap, 0.25), 2);
    }
    veh.acc = Math.max(acc, -9);
    if (!leader && toEnd < 6 && veh.v < 0.6) veh.stopped = true;
  }

  tryEnter(r) {
    while (r.q.length && this.n < this.maxVeh) {
      const veh = r.q[0], ai = r.arcs[0];
      const li = this.pickLane(ai, veh, veh.len + 3);
      if (li < 0) return;
      const L = this.lanes[ai][li], back = L.length ? L[L.length - 1] : null;
      veh.li = li; veh.s = 0;
      veh.v = Math.min(this.G.arcs[ai].speed / 3.6 * 0.6, back ? back.v : 99);
      L.push(veh); this.count[ai]++; this.n++;
      r.q.shift();
    }
  }

  finish(veh) {
    this.n--;
    if (veh.type === 'bus') { this.stats.busDone++; this.stats.busTT += this.t - veh.t0; return; }
    this.stats.done++; this.stats.tt += this.t - veh.t0; this.stats.ff += veh.ff;
  }

  transfer(veh, ai, li) {
    const G = this.G, A = G.arcs[ai], lane = this.lanes[ai][li];
    if (veh.k === veh.r.arcs.length - 1) {
      lane.shift(); this.count[ai]--; this.passed[ai]++; this.finish(veh);
      return true;
    }
    const nxt = veh.r.arcs[veh.k + 1];
    let nl = veh.perm && veh.perm.ai === nxt ? veh.perm.li : -1;
    if (nl < 0 && veh.stuck) { nl = this.pickLane(nxt, veh, -1e9); this.stats.teleports++; }
    veh.stuck = false;
    if (nl < 0) { veh.s = A.len - 0.01; veh.v = 0; return false; }
    lane.shift(); this.count[ai]--; this.passed[ai]++;
    const tl = this.lanes[nxt][nl], back = tl.length ? tl[tl.length - 1] : null;
    veh.s = Math.max(0, veh.s - A.len);
    if (back) veh.s = Math.min(veh.s, Math.max(0, back.s - back.len - 0.5));
    veh.k++; veh.ai = nxt; veh.li = nl; veh.perm = null; veh.stopped = false; veh.wait = 0;
    veh.ff += G.arcs[nxt].ff;
    tl.push(veh); this.count[nxt]++;
    this.nodeLast.set(A.to, { t: this.t, ai });
    this.mergeLast.set(nxt, { t: this.t, ai });
    return true;
  }

  step() {
    const dt = this.dt, G = this.G;
    this.t += dt;
    const t = this.t;
    for (const r of this.routes) {
      let guard = 0;
      while (r.next <= t && guard++ < 50) {
        if (r.q.length < 60) r.q.push(this.newVeh(r));
        this.stats.spawned++;
        r.next += r.type === 'bus' ? 1 / r.rate : this.gapFor(r);
      }
      if (r.q.length) this.tryEnter(r);
    }
    for (let ai = 0; ai < G.M; ai++) {
      if (!this.count[ai]) continue;
      for (const lane of this.lanes[ai]) for (let k = 0; k < lane.length; k++) this.accel(lane[k], ai, k ? lane[k - 1] : null);
    }
    for (let ai = 0; ai < G.M; ai++) {
      if (!this.count[ai]) continue;
      for (const lane of this.lanes[ai]) for (const veh of lane) {
        const nv = veh.v + veh.acc * dt;
        veh.s += nv < 0 ? veh.v * veh.v / (-2 * veh.acc) : (veh.v + nv) / 2 * dt;
        veh.v = Math.max(0, nv);
        veh.wait = veh.v < 0.3 ? veh.wait + dt : 0;
      }
    }
    for (let ai = 0; ai < G.M; ai++) {
      if (!this.count[ai]) continue;
      const A = G.arcs[ai], lanes = this.lanes[ai];
      for (let li = 0; li < lanes.length; li++) {
        const lane = lanes[li];
        let guard = 0;
        while (lane.length && guard++ < 20) {
          const f = lane[0];
          // A vehicle held at a line for two minutes is in a gridlock loop: move it on (counted as "unstuck")
          if (f.s < A.len) { if (f.wait > 120 && A.len - f.s < 10 && f.k < f.r.arcs.length - 1) { f.s = A.len; f.stuck = true; } else break; }
          if (!this.transfer(f, ai, li)) break;
        }
      }
    }
    if (Math.abs(t % 5) < dt / 2) this.sample();
  }

  sample() {
    let q = 0, vs = 0, n = 0;
    for (let ai = 0; ai < this.G.M; ai++) {
      if (!this.count[ai]) continue;
      for (const lane of this.lanes[ai]) for (const v of lane) { n++; vs += v.v; if (v.v < 1.5) q++; }
    }
    let waiting = 0;
    for (const r of this.routes) waiting += r.q.length;
    this.stats.series.push({ t: this.t, n, q, spd: n ? vs / n * 3.6 : 0, done: this.stats.done, waiting });
    if (this.stats.series.length > 1440) this.stats.series.shift();
    if (Math.abs(this.t % 60) < this.dt / 2) {
      (this.snaps ||= []).push({ t: this.t, passed: this.passed.slice() });
      if (this.snaps.length > 12) this.snaps.shift();
    }
  }

  // Simulated flow through each arc over the last 5–10 minutes (veh/h), compared with the assignment
  validation() {
    if (!this.res || !this.snaps || this.t < 420) return null;
    const ref = this.snaps.find(s => s.t <= this.t - 300 && s.t >= 120) || this.snaps[0];
    const h = (this.t - ref.t) / 3600;
    if (h <= 0) return null;
    const rows = [];
    for (let ai = 0; ai < this.G.M; ai++) {
      const A = this.G.arcs[ai]; if (A.conn) continue;
      const r = this.res.arcs[A.key]; if (!r || r.v < 50) continue;
      rows.push({ key: A.key, link: A.link, name: A.name, model: r.v, sim: (this.passed[ai] - ref.passed[ai]) / h });
    }
    if (!rows.length) return null;
    // GEH statistic: < 5 is the usual calibration target
    for (const r of rows) r.geh = Math.sqrt(2 * (r.sim - r.model) ** 2 / Math.max(1, r.sim + r.model));
    rows.sort((a, b) => b.model - a.model);
    return { rows, pass: rows.filter(r => r.geh < 5).length / rows.length };
  }

  forEachVehicle(fn) {
    for (let ai = 0; ai < this.G.M; ai++) {
      if (!this.count[ai]) continue;
      const lanes = this.lanes[ai];
      for (let li = 0; li < lanes.length; li++) for (const v of lanes[li]) fn(v, this.G.arcs[ai], li, lanes.length);
    }
  }
}
