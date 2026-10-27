// Records the prototype's traffic microsimulation (sim.js `Sim`) for the Rust port's sim module
// (projects/spare-construct/crates/construct-transport/src/sim.rs, engine-sim micro).
// For every project in inputs/, every scenario and both periods (and a few busier variants that reach gridlock, the
// vehicle limit, heavy vehicles and the queue limit), the harness does what the app does when Space is pressed:
// Engine.assign for the period, then `new Sim(Store.sc, res)` and `sim.step()` a fixed number of times. The random source
// is the harness's seeded generator (js-core.mjs `loadCore({ seed })`), which is untouched until the Sim is made (checked
// below), so the Rust test replays the run with the same generator from the same seed.
// (The prototype's Sim.signal throws at a signalised junction site's other member nodes; the harness patches that one lookup
// in its loaded copy, see runCase, and the Rust port does the same.)
// Each case records: the opening state (period, scale, limit, signal timings after the assignment's optimised cycles and
// splits, every route with its first release time), then at several steps the clock, vehicle counts, per-arc counts and
// passed flows, the totals, the route queues, the signal lights, the HUD numbers, the model comparison (validation())
// and, at some steps, every vehicle (in forEachVehicle order). The case also keeps the part of the assignment result the
// Sim reads, so the Rust test needs no assignment. Also writes expected/sim-functions.json: busFreq, busPcuOf and pcuF
// over grids of inputs, and the generator's first numbers. Writes expected/sim-<name>.json.
import { readFileSync, writeFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCore } from './js-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));

// As gen-expected.mjs: typed arrays as plain arrays, Maps and Sets as objects and arrays, non-finite numbers as strings
const plain = v => {
  if (ArrayBuffer.isView(v)) return Array.from(v, plain);
  if (v instanceof Map) return Object.fromEntries([...v].map(([k, x]) => [String(k), plain(x)]));
  if (v instanceof Set) return [...v].map(plain);
  if (typeof v === 'number') return Number.isFinite(v) ? v : String(v);
  if (Array.isArray(v)) return v.map(plain);
  if (v && typeof v === 'object') { const o = {}; for (const [k, x] of Object.entries(v)) if (typeof x !== 'function') o[k] = plain(x); return o; }
  return v;
};
// The same with finite numbers kept to 10 significant digits, for the recorded states (far finer than the test's tolerance, and
// about half the file); the inputs the Rust test replays from (the result, the opening state) keep every digit.
const plainShort = v => {
  if (typeof v === 'number') return Number.isFinite(v) ? Number(v.toPrecision(10)) : String(v);
  if (Array.isArray(v)) return v.map(plainShort);
  if (v && typeof v === 'object') { const o = {}; for (const [k, x] of Object.entries(v)) o[k] = plainShort(x); return o; }
  return v;
};
const clone = o => JSON.parse(JSON.stringify(o));

// The harness's generator (js-core.mjs), to check it has not been used before the Sim starts
const generator = seed => {
  let s = seed >>> 0;
  return () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
};

// Steps at which the state is recorded; vehicles in full at the DETAIL steps
const STEPS = [1, 2, 3, 5, 10, 30, 60, 120, 240, 480, 720, 840, 960, 1200, 1800, 2400];
const DETAIL = new Set([10, 480]);

// The cases: the default settings on every scenario and period, then variants of one scenario each: busier demand (gridlock,
// heavy vehicles), a vehicle limit, a bus lane on the bus routes' links, give-way junctions made stops, 100 km/h roads, a nearly empty network and
// a run of over two hours (the 5 s series keeps only its latest 1,440 samples)
const variants = name => name === 'corridor'
  ? [{ label: 'busy', scenario: 's-opt', period: 'AM', scale: 2.5, hvPct: 14, seed: 5, steps: 4800 },
     { label: 'limit', scenario: 'sidc01d0p', period: 'PM', scale: 1.5, maxVeh: 60, seed: 9, steps: 2400 },
     { label: 'jam', scenario: 'sidc01d0p', period: 'AM', scale: 6, hvPct: 25, seed: 13, steps: 6000 },
     { label: 'buslane', scenario: 's-opt', period: 'AM', edit: 'buslane', seed: 17, steps: 2400 },
     { label: 'stops', scenario: 'sidc01d0p', period: 'AM', edit: 'stops', scale: 2, seed: 21, steps: 2400 },
     { label: 'fast', scenario: 's-opt', period: 'PM', edit: 'fast', scale: 2, hvPct: 20, seed: 33, steps: 2400 }]
  : [{ label: 'busy', scenario: 's-dm', period: 'PM', scale: 3, hvPct: 12, seed: 5, steps: 4800 },
     { label: 'limit', scenario: 's-opt', period: 'AM', scale: 5, maxVeh: 70, seed: 9, steps: 2400 },
     { label: 'quiet', scenario: 's-opt', period: 'PM', scale: 0.02, seed: 13, steps: 1200 },
     { label: 'long', scenario: 'sidc01pnz', period: 'AM', seed: 17, steps: 14600 },
     { label: 'buslane', scenario: 's-dm', period: 'AM', edit: 'buslane', scale: 2, seed: 21, steps: 2400 },
     { label: 'stops', scenario: 's-dm', period: 'AM', edit: 'stops', scale: 3, seed: 25, steps: 2400 },
     { label: 'jam', scenario: 's-dm', period: 'PM', scale: 10, hvPct: 20, seed: 29, steps: 4800 },
     { label: 'fast', scenario: 's-opt', period: 'PM', edit: 'fast', scale: 3, hvPct: 20, seed: 33, steps: 2400 }];

// Edits of the scenario before the assignment (the Rust test reads the edited scenario from the case)
const EDITS = {
  // A bus lane on every link a bus route runs along, with at least two lanes each way
  buslane: (sc, S, period, { Engine }) => {
    const G = Engine.buildGraph(sc, S, period);
    for (const r of Object.values(sc.routes)) for (const ai of Engine.resolveRoute(G, r)) {
      const L = sc.links[G.arcs[ai].link]; if (!L) continue;
      L.busLane = true; if (L.lanesAB > 0) L.lanesAB = Math.max(2, L.lanesAB); if (L.lanesBA > 0) L.lanesBA = Math.max(2, L.lanesBA);
    }
  },
  // Every road has a 100 km/h limit (heavy vehicles and buses are held to 25 m/s)
  fast: sc => { for (const L of Object.values(sc.links)) L.speed = 100; },
  // Every give-way junction is a stop junction
  stops: sc => { for (const n of Object.values(sc.nodes)) if (n.control === 'priority') n.control = 'stop'; },
};

function snapshot(sim, step, detail) {
  const G = sim.G, st = sim.stats, idx = new Map(sim.routes.map((r, i) => [r, i]));
  const last = st.series[st.series.length - 1] || { n: 0, q: 0, spd: 0 };
  const o = {
    step, t: sim.t, n: sim.n, nid: sim.nid, count: Array.from(sim.count), passed: Array.from(sim.passed),
    stats: { done: st.done, tt: st.tt, ff: st.ff, spawned: st.spawned, teleports: st.teleports, busDone: st.busDone, busTT: st.busTT, seriesLen: st.series.length, last: st.series.length ? last : null },
    routes: sim.routes.map(r => ({ next: r.next, q: r.q.length, qIds: r.q.map(v => v.id) })),
    // Lights of every signal approach
    signals: [...G.ctl.values()].filter(c => c.type === 'signals').flatMap(c => c.arcs.map(ai => ({ ai, light: sim.signal(G.arcs[ai]) }))),
    hud: { clock: sim.startClock + sim.t, n: last.n, q: last.q, spd: last.spd, done: st.done, avgDelay: st.done ? (st.tt - st.ff) / st.done : 0, waiting: last.waiting || 0, spawned: st.spawned, teleports: st.teleports },
    validation: sim.validation(),
    snaps: (sim.snaps || []).map(s => s.t),
  };
  if (detail) {
    o.vehicles = [];
    sim.forEachVehicle((v, A, li, nl) => o.vehicles.push({
      id: v.id, route: idx.get(v.r), k: v.k, ai: v.ai, li, nl, s: v.s, v: v.v, acc: v.acc, type: v.type, len: v.len, amax: v.amax, T: v.T, v0f: v.v0f,
      t0: v.t0, ff: v.ff, wait: v.wait, stopped: v.stopped, stuck: !!v.stuck, perm: v.perm ? [v.perm.ai, v.perm.li] : null,
    }));
  }
  return plainShort(plain(o));
}

async function runCase(file, P0, c) {
  const core = loadCore({ seed: c.seed });
  const { Engine, Store, Sim } = core;
  // The prototype looks a signal's controller up by the approach's end node (`this.G.ctl.get(A.to)`), but a signalised
  // junction site of several nodes keeps its controller under one node (A.node), so an approach ending at another member
  // throws and the simulation stops. In this loaded copy only, the lookup falls back on A.node, as the Rust port does.
  const signal0 = Sim.prototype.signal;
  Sim.prototype.signal = function (A) { return signal0.call(this, this.G.ctl.has(A.to) ? A : { ...A, to: A.node }); };
  const P = clone(P0);
  Store.P = P;
  const sc = P.scenarios.find(s => s.id === c.scenario);
  P.active = sc.id;
  const S = P.settings;
  if (c.edit) EDITS[c.edit](sc, S, c.period, core);
  if (c.scale != null) S.sim = { ...S.sim, scale: c.scale };
  if (c.maxVeh != null) S.sim = { ...S.sim, maxVeh: c.maxVeh };
  if (c.hvPct != null) S.hvPct = c.hvPct;
  // What the app does before it simulates: the assignment for the period
  const res = await Engine.assign(sc, { period: c.period });
  const sim = new Sim(sc, res);
  // Loading the scripts draws a few numbers (the interface's tab id), so the Sim starts some numbers into the stream. Find how
  // many: every route's first release is drawn in route order from consecutive numbers, and the Rust test skips the same count.
  const draws = Array.from({ length: 40 + sim.routes.length }, generator(c.seed));
  let skip = -1;
  for (let k = 0; k < 40 && skip < 0; k++) {
    const ok = sim.routes.every((r, i) => r.next === (r.type === 'bus' ? draws[k + i] * 3600 / (r.rate * 3600) : -Math.log(1 - draws[k + i]) / Math.max(1e-9, r.rate * sim.scale)));
    if (ok) skip = k;
  }
  if (skip < 0) throw new Error(`${file} ${c.scenario} ${c.period}: the releases are not consecutive numbers of the seeded generator`);
  const G = sim.G, idx = new Map(sim.routes.map((r, i) => [r, i]));
  const init = plain({
    period: sim.period, scale: sim.scale, maxVeh: sim.maxVeh, hv: sim.hv, pf: sim.pf, startClock: sim.startClock, M: G.M, dt: sim.dt,
    signals: [...G.ctl.values()].filter(x => x.type === 'signals').map(x => ({ n: x.n, C: x.C, lost: x.lost, split: x.split, offset: x.offset, arcs: x.arcs.map(i => ({ i, phase: G.arcs[i].phase ?? null, gC: G.arcs[i].gC, C: G.arcs[i].C })) })),
    routes: sim.routes.map(r => ({ arcs: r.arcs, rate: r.rate, type: r.type, color: r.color ?? null, rid: r.rid ?? null, next: r.next })),
    lanes: sim.lanes.map(l => (l ? l.length : null)),
  });
  // The part of the result the Sim reads
  const resIn = plain({
    period: res.period,
    nodes: Object.fromEntries(Object.entries(res.nodes).map(([k, n]) => [k, { C: n.C ?? null, split: n.split ?? null }])),
    arcKeys: res.arcKeys, paths: res.paths,
    arcs: Object.fromEntries(Object.entries(res.arcs).map(([k, a]) => [k, { v: a.v }])),
  });
  const steps = c.steps ?? STEPS[STEPS.length - 1];
  const marks = [...STEPS.filter(s => s <= steps), ...[3600, 7200, 10800].filter(s => s > STEPS[STEPS.length - 1] && s < steps), ...(steps > STEPS[STEPS.length - 1] ? [steps] : [])];
  const cps = [];
  let done = 0;
  for (const m of marks) {
    while (done < m) { sim.step(); done++; }
    cps.push(snapshot(sim, m, DETAIL.has(m) || m === steps));
  }
  const st = sim.stats;
  return {
    project: file, scenario: c.scenario, period: c.period, label: c.label ?? 'default', seed: c.seed, rngSkip: skip,
    settings: { scale: c.scale ?? null, maxVeh: c.maxVeh ?? null, hvPct: c.hvPct ?? null },
    scenarioText: c.edit ? JSON.stringify(sc) : null,
    steps, init, res: resIn, checkpoints: cps, series: plainShort(plain(st.series)),
    routeIndex: sim.routes.length,
  };
}

// busFreq, busPcuOf and pcuF over grids of inputs, and the generator's first numbers
function functionCases() {
  const { Engine } = loadCore({ seed: 1 });
  const freqs = [undefined, null, {}, [], 'x', 5, { AM: 10 }, { AM: 10, PM: 8 }, { AM: '12', PM: '' }, { AM: null, PM: 3 }, { AM: 'abc' }, { AM: -4 }, { AM: 0 }, { AM: [7] }, { AM: true }, { AM: {} }, { AM: Infinity }, { PM: 6 }, { AM: '0x10' }, { AM: ' 9 ' }, { undefined: 4 }, { AM: 1e400 }];
  const freq = [undefined, null, 0, 6, -3, '8', 'x', '', NaN, true, 12.5, Infinity];
  const periods = ['AM', 'PM', undefined, 'NZ'];
  const busFreq = [];
  for (const f of freqs) for (const q of freq) for (const p of periods) {
    const r = { id: 'r', freq: q }; if (f !== undefined) r.freqs = f;
    // JSON text so undefined, Infinity and NaN survive (the Rust test reads each route from its text)
    busFreq.push({ route: JSON.stringify(r, (k, v) => (typeof v === 'number' && !Number.isFinite(v) ? String(v) : v)), period: p ?? null, out: Engine.busFreq(r, p) });
  }
  const jt = o => JSON.stringify(o, (k, v) => (typeof v === 'number' && !Number.isFinite(v) ? String(v) : v));
  const busPcu = [undefined, null, 0, -1, 2, 2.5, '3', 'x', NaN, Infinity, '', true].map(v => {
    const S = v === undefined ? {} : { busPcu: v };
    return { settings: jt(S), out: Engine.busPcuOf(S) };
  });
  const pcu = [[0, 1], [6, 2.3], [100, 2], [12, 1], [3.5, 2.5], [0, 5]].map(([hvPct, pcuHV]) => ({ settings: jt({ hvPct, pcuHV }), out: Engine.pcuF({ hvPct, pcuHV }) }));
  const g = generator(1), g2 = generator(11), g3 = generator(4294967295);
  const rng = { 1: Array.from({ length: 40 }, g), 11: Array.from({ length: 40 }, g2), 4294967295: Array.from({ length: 40 }, g3) };
  return plain({ busFreq, busPcu, pcu, rng });
}

const only = process.argv[2];
for (const file of readdirSync(join(here, 'inputs')).filter(f => f.endsWith('.json'))) {
  const name = file.replace(/\.json$/, '');
  if (only && only !== name && only !== 'all') continue;
  const P0 = JSON.parse(readFileSync(join(here, 'inputs', file), 'utf8'));
  const cases = [];
  let seed = 1;
  for (const sc of P0.scenarios) for (const period of ['AM', 'PM']) cases.push({ scenario: sc.id, period, seed: seed++, steps: 2400 });
  cases.push(...variants(name));
  const out = { project: name, steps: STEPS, detail: [...DETAIL], cases: [] };
  for (const c of cases) {
    const t0 = Date.now();
    out.cases.push(await runCase(name, P0, c));
    const k = out.cases[out.cases.length - 1], last = k.checkpoints[k.checkpoints.length - 1];
    console.log(name, c.scenario, c.period, c.label ?? 'default', `routes ${k.init.routes.length}`, `n ${last.n} done ${last.stats.done} spawned ${last.stats.spawned} teleports ${last.stats.teleports} busDone ${last.stats.busDone}`, `${Date.now() - t0} ms`);
  }
  writeFileSync(join(here, 'expected', `sim-${name}.json`), JSON.stringify(out));
  console.log(name, 'sim expected written');
}
if (!only || only === 'all' || only === 'functions') {
  writeFileSync(join(here, 'expected', 'sim-functions.json'), JSON.stringify(functionCases()));
  console.log('sim-functions expected written');
}
