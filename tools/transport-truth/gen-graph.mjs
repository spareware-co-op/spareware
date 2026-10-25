// Records the prototype engine's intermediate results for the Rust port's graph and junction modules
// (projects/spare-construct/crates/construct-transport: graph, junction; projects/engines/crates/engine-sim).
// For every project in inputs/ and every scenario and period: the built graph (arcs, nodes, zone connectors, turn
// table, site movement slots, junction controllers after setupControls and applySignal), turn penalties and bans,
// shortest-path trees on free-flow costs, resolved bus routes, link and junction performance at several flow levels
// (arcTime, effCap, the gap-acceptance capacities, the movement method) and junction results. Variants of the
// projects (written into the file as JSON text) cover lane models, bans, rings, sites and other branches the
// projects do not reach. Writes expected/graph-<name>.json, and expected/junction-functions.json for the
// movement-capacity functions, the text helpers and the settings readers over grids of representative inputs.
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
const clone = o => JSON.parse(JSON.stringify(o));
const nn = v => (v === undefined ? null : v);

/* ---------------- the graph, as plain data ---------------- */
function arcOut(a) {
  return plain({
    i: a.i, key: a.key, link: nn(a.link), dir: nn(a.dir), conn: a.conn, from: a.from, to: a.to, len: a.len, lanes: a.lanes, gl: a.gl,
    bus: nn(a.bus), two: nn(a.two), ff: a.ff, capMid: a.capMid, cls: nn(a.cls), rank: a.rank, speed: nn(a.speed), lw: nn(a.lw), ring: nn(a.ring),
    name: nn(a.name), pts: a.pts, cum: nn(a.cum), inB: a.inB, outB: a.outB, ctrl: a.ctrl, internal: !!a.internal, toll: nn(a.toll), zone: nn(a.zone),
    node: nn(a.node), phase: nn(a.phase), C: nn(a.C), gC: nn(a.gC), major: !!a.major, stop: !!a.stop, conf: nn(a.conf), ringIn: nn(a.ringIn),
    ringOut: nn(a.ringOut), lmSrc: nn(a.lmSrc),
  });
}
function entOut(e) {
  return plain({ i: e.i, major: e.major, inB: e.inB, leg: e.leg, gl: e.gl, sLane: e.sLane, stop: e.stop, gaps: e.gaps, lanes: e.lanes, median: e.median,
    slip: e.slip, lmSrc: e.lmSrc, thru: e.thru, mvT: nn(e.mvT), nc: nn(e.nc) });
}
function ctlOut(G, n, c) {
  return plain({
    n, type: c.type, arcs: c.arcs, single: nn(c.single), C: nn(c.C), lost: nn(c.lost), split: nn(c.split), offset: nn(c.offset), major: nn(c.major),
    src: c.src && c.src.id, site: nn(c.site), members: nn(c.members), exits: nn(c.exits),
    legs: c.legs ? c.legs.map(l => ({ k: l.k, b: l.b, name: l.name, rank: l.rank, ins: l.ins, outs: l.outs })) : null,
    legIn: c.legIn ? plain(c.legIn) : null, legOut: c.legOut ? plain(c.legOut) : null, ent: c.ent ? c.ent.map(entOut) : null,
  });
}
function graphOut(G) {
  const T = G.turnS;
  return {
    ids: G.ids, N: G.N, M: G.M, ll: plain(G.ll), head: plain(G.head), out: plain(G.out), to: plain(G.to), ffc: plain(G.ffc), isC: plain(G.isC),
    zoneNode: G.zoneNode, zones: G.zones.map(z => z.id), deg: plain(G.deg), into: G.into, outR: G.outR, conns: G.conns, keyIdx: plain(G.keyIdx),
    nodes: G.nodes.map(x => (x ? x.id : null)), siteOf: plain(G.siteOf), sites: G.sites.map(s => ({ id: s.site.id, members: s.members })),
    capMethod: G.capMethod, period: G.period, turnS: plain({ left: T.left, right: T.right, rightSignals: T.rightSignals, uturn: T.uturn }),
    tOff: plain(G.tOff), tpen: plain(G.tpen), tTo: plain(G.tTo), tmv: plain(G.tmv), nT: G.nT,
    sm: G.sm ? plain({ list: G.sm.list, of: G.sm.of, entS: G.sm.entS, entI: G.sm.entI, extS: G.sm.extS, extI: G.sm.extI, n: G.sm.n }) : null,
    arcs: G.arcs.map(arcOut), ctl: [...G.ctl.entries()].map(([n, c]) => ctlOut(G, n, c)),
  };
}
// Per-arc state the performance functions change
const stateOut = (G, S, Engine, xt, mv) => G.arcs.map(a => plain({
  capN: nn(a.capN), eff: Engine.effCap(a, S), t: Engine.arcTime(a, xt[a.i], S), ts: [0, 1200, 2400].map(x => Engine.arcTime(a, x, S)),
  ...(mv ? { mj: nn(a.mj), mvRes: nn(a.mvRes) } : {}),
}));
const entStateOut = G => [...G.ctl.values()].filter(c => c.ent).map(c => ({
  n: c.n, ent: c.ent.map(e => plain({ v: e.v, Lto: e.Lto, vc: nn(e.vc), nc: nn(e.nc), n: e._n ? e._n.i : null, f: e._f ? e._f.i : null, o: e._o ? e._o.i : null, opp: e._opp ? e._opp.i : null })),
}));

/* ---------------- one graph: structure, turns, searches, routes, performance, junctions ---------------- */
function graphCase(c, sc, S, period) {
  const { Engine, U } = c;
  const G = Engine.buildGraph(sc, S, period);
  const out = { scenario: sc.id, period, graph: graphOut(G) };
  // Turn penalties, bans and movements for every arc pair at each node, in turn-table order
  const tp = [], tb = [], mv = [];
  for (let a = 0; a < G.M; a++) {
    const A = G.arcs[a], h = G.head[A.to];
    for (let k = h; k < G.head[A.to + 1]; k++) {
      const B = G.arcs[G.out[k]];
      tp.push(Engine.turnPenalty(G, A, B)); tb.push(Engine.turnBanned(G, A, B)); mv.push(Engine.movement(G, A, B));
    }
  }
  out.turns = plain({ tp, tb, mv });
  // Shortest-path trees on free-flow costs: from every zone and three road nodes; with a target; leaving along an arc
  const N0 = Object.keys(sc.nodes).length, srcs = [...G.zoneNode, 0, N0 >> 1, Math.max(0, N0 - 1)];
  const W = Engine.workspace(G), trees = [];
  const rec = (src, target, fromArc) => {
    Engine.spt(G, G.ffc, src, W, target, fromArc);
    const paths = []; for (let v = 0; v < G.N; v++) paths.push(Engine.pathTo(G, W, v));
    trees.push(plain({ src, target, fromArc, dist: W.dist, pred: W.pred, pt: W.pt, nd: W.nd, na: W.na, paths }));
  };
  for (const s of srcs) rec(s, -1, -1);
  if (G.zoneNode.length >= 2) rec(G.zoneNode[0], G.zoneNode[G.zoneNode.length - 1], -1);
  for (const s of [0, N0 >> 1]) { const a = G.into[s] && G.into[s][0]; if (a != null) rec(s, -1, a); }
  out.spt = trees;
  // The node-array interface and trace
  const dist = new Float64Array(G.N), pred = new Int32Array(G.N), s0 = G.zoneNode[0] ?? 0;
  Engine.dijkstra(G, G.ffc, s0, dist, pred, null);
  const tr = []; for (let v = 0; v < G.N; v++) tr.push(Engine.trace(G, pred, s0, v));
  out.dijkstra = plain({ src: s0, dist, pred, trace: tr, wrongSrc: Engine.trace(G, pred, s0 + 1, 0), foreign: Engine.trace(G, new Int32Array(G.N), s0, 0) });
  out.routes = Object.values(sc.routes || {}).map(r => Engine.resolveRoute(G, r));
  out.connectorMap = Engine.connectorMap(sc);
  // Traced site flows rebuilt from a result's siteTurns
  if (G.sm) {
    const st = G.sm.list[0], siteTurns = { [st.id]: [] };
    st.inc.forEach((a, e) => st.exits.forEach((b, x) => { siteTurns[st.id].push((e + x) % 3 === 0 ? { from: G.arcs[a].key, to: G.arcs[b].key, v: 10 * (e + 1) + x, pcu: (e + x) % 2 ? null : 12.5 * (x + 1) } : { from: G.arcs[a].key, to: G.arcs[b].key, v: 7 + e }); }));
    siteTurns[st.id].push({ from: 'nope', to: G.arcs[st.exits[0]].key, v: 99 });
    siteTurns.unknownSite = [{ from: 'a', to: 'b', v: 1 }];
    out.siteFlows = { siteTurns, f1: plain(Engine.siteFlows(G, { siteTurns }, 1)), f2: plain(Engine.siteFlows(G, { siteTurns }, 1.5)) };
  }
  // Performance on the AM graph only (PM changes free-flow times, which the graph structure already shows)
  if (period === 'AM') out.perf = perfCase(c, sc, S, period);
  return out;
}

// A deterministic flow pattern at level L (arcs, turns, site movement slots)
const flowsAt = (G, L) => ({
  xt: Float64Array.from({ length: G.M }, (_, j) => L * (0.4 + ((j * 37) % 11) / 10)),
  xT: Float64Array.from({ length: G.nT }, (_, t) => L * (0.2 + ((t * 13) % 7) / 10) / 3),
  smX: G.sm ? Float64Array.from({ length: G.sm.n }, (_, k) => L * (0.3 + ((k * 17) % 5) / 10) / 2) : null,
});
const LEVELS = [[0, 1], [250, 1], [800, 0.5], [1600, 0.25], [2600, 1]];

/* Performance: a sequence of capacity updates (damped as the assignment damps them) with the arc method on one graph
   and, when the settings choose it, the movement method on another; after each update every arc's capN, effCap and
   arcTime, the movement detail, and junctionResult for every controller. */
function perfCase(c, sc, S, period) {
  const { Engine, U } = c, pf = Engine.pcuF(S), J0 = Engine.junctionSet(S), out = {};
  const run = (G, mv) => {
    const steps = [];
    LEVELS.forEach(([L, w], li) => {
      const f = flowsAt(G, L);
      if (mv) { if (li >= 2 && f.smX) G.smX = f.smX; else delete G.smX; G.mvDetail = li === 0 ? false : undefined; }
      Engine.updateNodeCaps(G, f.xt, w, mv ? f.xT : null);
      const step = { L, w, smX: !!G.smX, detail: G.mvDetail !== false, arcs: stateOut(G, S, Engine, f.xt, mv) };
      if (mv) step.ent = entStateOut(G);
      if (G.mvDetail !== false) {
        const arcsRes = {};
        for (const a of G.arcs) arcsRes[a.key] = { v: U.r1(f.xt[a.i] / pf), nd: U.r1(Engine.arcTime(a, f.xt[a.i], S) - a.ff * (1 + 0.15 * (a.i % 3))) };
        const methods = li === LEVELS.length - 1 ? ['hcm6', 'sidra', 'rta'] : [J0.method];
        step.junctions = methods.map(method => {
          const J = { ...J0, method, capMethod: mv ? 'movement' : 'arc' };
          return { method, nodes: [...G.ctl.values()].map(ctl => plain(Engine.junctionResult(G, ctl, f.xt, arcsRes, S, J, pf))) };
        });
        step.arcsRes = arcsRes;
      }
      steps.push(step);
    });
    return steps;
  };
  const Ga = Engine.buildGraph(sc, S, period); Ga.capMethod = 'arc';
  out.arc = run(Ga, false);
  if (Engine.capMethodOf(S) === 'movement') out.movement = run(Engine.buildGraph(sc, S, period), true);
  return out;
}

/* ---------------- variants that reach the branches the projects do not ---------------- */
function variants(name, P, c) {
  const { Engine } = c, base = P.scenarios[0], S = P.settings, list = [];
  const linksAt = (sc, nid) => Object.values(sc.links).filter(L => L.a === nid || L.b === nid);
  const byControl = (sc, ctl) => Object.values(sc.nodes).filter(n => n.control === ctl);
  if (name === 'grid') {
    const sc = clone(base); sc.id = 'v-mods';
    const S2 = clone(S); S2.junction.capMethod = 'movement'; S2.turns.uturn = '7'; S2.cycle = '90';
    const sig = byControl(sc, 'signals')[0], pri = byControl(sc, 'priority')[0], stp = byControl(sc, 'stop')[0], rbt = byControl(sc, 'roundabout')[0];
    sig.split = [0.6, 0.4]; sig.cycle = 80;
    const pl = linksAt(sc, pri.id);
    pri.lanes = { [pl[0].id]: { lanes: [{ d: 'L', len: 25 }, { d: 'TR' }], median: true, src: 'survey' }, [pl[1].id]: { lanes: ['l+t', { mv: 'R', len: 40 }] } };
    if (pl[2]) pri.lanes[pl[2].id] = { lanes: [{ d: 'T' }, { d: 'R', len: 30 }] };
    pri.gaps = { minorR: { tc: 7.5 }, majorR: { tf: '2.5' } };
    const sl = linksAt(sc, stp.id);
    stp.lanes = { [sl[0].id]: { lanes: ['L+T', 'R'], slip: true }, [sl[1].id]: { lanes: [{ d: 'LTR', len: 0 }, { d: 'R', len: 20 }] } };
    const rl = linksAt(sc, rbt.id);
    rbt.lanes = { [rl[0].id]: { lanes: [{ d: 'LT' }, { d: 'TR' }] }, [rl[1].id]: { lanes: [{ d: 'LT' }, { d: 'T' }, { d: 'TR' }], slip: true } };
    // A ban and an allowed U-turn at plain 4-link nodes
    const plainNodes = Object.values(sc.nodes).filter(n => n.control === 'none' && linksAt(sc, n.id).length === 4);
    if (plainNodes[0]) { const ls = linksAt(sc, plainNodes[0].id); plainNodes[0].bans = [ls[0].id + '>' + ls[1].id, ls[2].id + '>' + ls[0].id]; }
    if (plainNodes[1]) plainNodes[1].allowU = true;
    const art = Object.values(sc.links).filter(L => L.cls === 'arterial');
    art[0].busLane = true; art[1].toll = 15; art[2].varSpeed = { v: 30, AM: true, PM: true }; art[3].capPerLane = 900; art[4].lanesBA = 0; art[5].speed = '40';
    const loc = Object.values(sc.links).filter(L => L.cls === 'local');
    loc[loc.length - 1].status = 'closed';
    loc[0].cls = 'ramp';
    list.push({ sc, S: S2 });
    const sc2 = clone(base); sc2.id = 'v-uturn0';
    const S3 = clone(S); S3.turns = { left: 3, right: null, uturn: 0 };
    list.push({ sc: sc2, S: S3 });
  } else {
    const sid = Object.keys(base.sites || {})[0];
    // The two-node site as one give-way controller over legs, with site lanes and gaps
    const sc = clone(base); sc.id = 'v-site-gw';
    const site = sc.sites[sid]; site.control = 'priority';
    const inLinks = Object.values(sc.links).filter(L => site.nodes.includes(L.a) !== site.nodes.includes(L.b));
    site.lanes = { [inLinks[0].id]: { lanes: [{ d: 'L' }, { d: 'TR', len: 0 }], median: true }, [inLinks[1].id]: { lanes: [{ d: 'LT' }, { d: 'R', len: 35 }] } };
    site.gaps = { minorT: { tc: 6.9, tf: 4.1 } };
    list.push({ sc, S });
    // Ring links at the roundabout, U-turns at no penalty, a ban, and lane models on the priority node
    const sc2 = clone(base); sc2.id = 'v-ring';
    const rbt = byControl(sc2, 'roundabout')[0], pri = byControl(sc2, 'priority')[0];
    for (const L of Object.values(sc2.links)) if ((L.a === rbt.id || L.b === rbt.id) && L.name === 'Main Road') L.ring = true;
    const pl = linksAt(sc2, pri.id);
    pri.lanes = { [pl[0].id]: { lanes: [{ d: 'LT' }, { d: 'R', len: 30 }], src: 'plan' }, [pl[2].id]: { lanes: [{ d: 'LTU', len: 0 }], slip: true, median: 1 } };
    pri.bans = [pl[0].id + '>' + pl[1].id];
    const S2 = clone(S); S2.turns.uturn = 0;
    list.push({ sc: sc2, S: S2 });
    // The site as a roundabout: per-node controllers, internal links free
    const sc3 = clone(base); sc3.id = 'v-rbt-site';
    sc3.sites[sid].control = 'roundabout';
    for (const id of sc3.sites[sid].nodes) sc3.nodes[id].control = 'roundabout';
    list.push({ sc: sc3, S });
    // Stop control at the priority node, arc method, sidra LoS
    const sc4 = clone(base); sc4.id = 'v-stop-arc';
    byControl(sc4, 'priority')[0].control = 'stop';
    const S4 = clone(S); S4.junction = { ...S4.junction, capMethod: 'arc', losMethod: 'sidra' };
    list.push({ sc: sc4, S: S4 });
  }
  return list;
}

/* ---------------- per project ---------------- */
const only = process.argv[2];
for (const file of readdirSync(join(here, 'inputs')).filter(f => f.endsWith('.json'))) {
  const name = file.replace(/\.json$/, '');
  if (only && only !== name) continue;
  const c = loadCore({ seed: 11 });
  const P = JSON.parse(readFileSync(join(here, 'inputs', file), 'utf8'));
  c.Store.P = P;
  const S = P.settings, out = { project: name, cases: [], variants: {} };
  for (const sc of P.scenarios) {
    P.active = sc.id;
    const periods = sc === P.scenarios[0] ? ['AM', 'PM', null] : ['AM', 'PM'];
    for (const period of periods) out.cases.push(graphCase(c, sc, S, period));
  }
  P.active = P.scenarios[0].id;
  for (const v of variants(name, P, c)) {
    out.variants[v.sc.id] = { scenario: JSON.stringify(v.sc), settings: JSON.stringify(v.S) };
    for (const period of ['AM', 'PM']) out.cases.push({ variant: true, ...graphCase(c, v.sc, v.S, period) });
  }
  writeFileSync(join(here, 'expected', 'graph-' + name + '.json'), JSON.stringify(out));
  console.log(name, 'graph cases', out.cases.length);
}

/* ---------------- functions over grids of representative inputs ---------------- */
{
  const c = loadCore({ seed: 11 }), { Engine } = c, M = Engine.mov, F = {};
  F.cpot = [];
  for (const vc of [0, 0.005, 0.01, 0.02, 50, 300, 900, 1800, 3000]) for (const [tc, tf] of [[4.1, 2.2], [6.2, 3.3], [7.1, 3.5], [5.2, 4]]) F.cpot.push({ in: [vc, tc, tf], out: M.cpot(vc, tc, tf) });
  F.pPrime = [-0.5, 0, 0.1, 0.3, 0.5, 0.8, 1, 1.5].map(p => ({ in: p, out: M.pPrime(p) }));
  F.twoStage = [];
  for (const cI of [0, 300, 650]) for (const cII of [200, 500, 800]) for (const cm of [150, 300]) for (const vL of [0, 100]) for (const k of [0, 1, 2]) F.twoStage.push({ in: [cI, cII, cm, vL, k], out: M.twoStage(cI, cII, cm, vL, k) });
  F.twoStage.push({ in: [500, 400, 300, 100, 1], out: M.twoStage(500, 400, 300, 100, 1) }, { in: [400, 500, 300, 100, 1], out: M.twoStage(400, 500, 300, 100, 1) },
    { in: [300.0000001, 400, 300, 100, 1], out: M.twoStage(300.0000001, 400, 300, 100, 1) });
  F.q95 = []; F.dHCM = [];
  for (const v of [0, 50, 400, 800, 1200]) for (const cc of [0, 0.5, 300, 900]) for (const T of [0.25, 0.5, 1]) { F.q95.push({ in: [v, cc, T], out: M.q95(v, cc, T) }); F.dHCM.push({ in: [v, cc, T], out: M.delay(v, cc, T) }); }
  const Ssets = [{ junction: { pff: 0.95, T: 0.5 } }, { junction: { pff: 1.2, T: 0 } }, {}, { junction: null }, { junction: { pff: '0.9', T: '0.25' } }];
  F.majorDelay = [];
  for (const mj of [{ sR: 0.2, shTL: 0.3, cmR: 400, rb: 0.5 }, { sR: 0, shTL: 0.3, cmR: 400, rb: 0 }, { sR: 0.1, shTL: 0, cmR: 0, rb: 1 }, { sR: 0.4, shTL: 0.5, cmR: 150, rb: 0 }])
    for (const x of [0, 300, 1200]) for (const S of Ssets) F.majorDelay.push({ in: { mj, x, S: JSON.stringify(S) }, out: M.majorDelay(mj, x, S) });
  const raws = [null, {}, { lanes: [] }, { lanes: ['L+T', 'R'] }, { lanes: [{ d: 'lt', len: '30' }, { mv: 'U', len: -5 }, { d: '' }, null, 7, true] }, { lanes: ['LTR'], slip: true, median: 1, src: 'survey' },
    { lanes: [{ d: 'L' }, { d: 'T' }], slip: 1 }, { slip: true }, { lanes: [{ d: 'TTRRL', len: 12 }], src: '' }, { lanes: 'LT' }, 'x', { median: true }];
  F.laneModel = [];
  for (const raw of raws) for (const gl of [0, 1, 2, 3, 4.7]) F.laneModel.push({ in: { raw: JSON.stringify(raw), gl }, out: plain(M.laneModel(raw, gl)) });
  const laneSets = [
    [{ d: 'LTR', len: 0, c: 600 }],
    [{ d: 'LT', len: 0, c: 700 }, { d: 'TR', len: 0, c: 650 }],
    [{ d: 'L', len: 30, c: 800 }, { d: 'T', len: 0, c: 700 }, { d: 'TR', len: 0, c: 600 }],
    [{ d: 'L', len: 0, c: 500, slip: true }, { d: 'T', len: 0, c: 700 }, { d: 'R', len: 45, c: 300 }],
    [{ d: 'T', len: 0, c: 700 }, { d: 'T', len: 0, c: 650 }],
    [{ d: 'LT', len: 20, c: 400 }, { d: 'R', len: 15, c: 300 }],
    [{ d: 'L', len: 10, c: 500 }, { d: 'LT', len: 0, c: 700 }, { d: 'T', len: 0, c: 700 }, { d: 'R', len: 60, c: 350 }],
  ];
  const flows = [{ L: 0, T: 0, R: 0 }, { L: 120, T: 400, R: 80 }, { L: 0, T: 900, R: 0 }, { L: 300, T: 0, R: 250 }, { L: 50, T: 1500, R: 400 }, { L: -10, T: 200, R: 0 }];
  const capF = (l, m) => l.c * (m === 'L' ? 0.9 : m === 'R' ? 0.8 : 1);
  F.allocLanes = []; F.approachCap = [];
  for (const lanes of laneSets) for (const v of flows) {
    F.allocLanes.push({ in: { lanes, v }, out: plain(M.allocLanes(lanes, v, capF)) });
    for (const [T, qs, detail] of [[0.25, 7, true], [0.5, 6, false]]) F.approachCap.push({ in: { lanes, v, T, qs, detail }, out: plain(M.approachCap(lanes, v, capF, T, qs, detail)) });
  }
  // Priority junctions: T and cross shapes, lane models, medians, overrides, stops; flows from free to over capacity
  const lm = (raw, gl) => M.laneModel(raw, gl);
  const ent = (major, inB, leg, raw, gl, o = {}) => { const e = { major, inB, leg, gl, sLane: 1800 / 1.1 * 0.5 + 300, stop: !!o.stop, gaps: o.gaps || null, ...lm(raw, gl), v: { L: 0, T: 0, R: 0 }, Lto: {} }; e.thru = Math.max(1, e.lanes.filter(l => l.d.includes('T') && !(l.len > 0)).length); return e; };
  const shapes = [
    () => [ent(true, 90, 'W', null, 1), ent(true, 270, 'E', null, 1), ent(false, 0, 'S', null, 1)],
    () => [ent(true, 92, 'W', null, 2), ent(true, 268, 'E', { lanes: [{ d: 'LT' }, { d: 'T' }, { d: 'R', len: 40 }] }, 2), ent(false, 359, 'S', { lanes: [{ d: 'L', len: 30 }, { d: 'TR' }], median: true }, 1, { stop: true })],
    () => [ent(true, 90, 'W', null, 1), ent(true, 270, 'E', null, 1), ent(false, 0, 'S', null, 1), ent(false, 180, 'N', { lanes: ['L', 'TR'], median: true }, 2, { gaps: { minorT: { tc: 7 }, minorR: { tf: '3.9' } } })],
    () => [ent(true, 45, 'SW', { lanes: [{ d: 'T' }, { d: 'R', len: 25 }] }, 1), ent(true, 230, 'NE', { lanes: [{ d: 'LT' }, { d: 'TR' }] }, 2), ent(false, 140, 'NW', { lanes: ['LTR'], slip: true }, 1, { stop: true }), ent(false, 320, 'SE', null, 2)],
    () => [ent(false, 0, 'S', null, 1), ent(false, 120, 'A', null, 1), ent(false, 240, 'B', null, 1)],
    () => [ent(true, 90, 'W', { lanes: [{ d: 'R', len: 30 }] }, 1), ent(false, 0, 'S', null, 1), ent(false, 180, 'N', null, 1)],
  ];
  const fill = (es, k) => es.forEach((e, i) => {
    const s = [0, 0.4, 1, 2.2][k];
    e.v = { L: (60 + 25 * i) * s, T: (e.major ? 500 : 90) * s + 10 * i, R: (40 + 15 * i) * s };
    e.Lto = {}; es.forEach((o, j) => { if (j !== i) e.Lto[o.leg] = (20 + 5 * j) * s; });
  });
  F.priorityCaps = [];
  shapes.forEach((mk, si) => { for (let k = 0; k < 4; k++) for (const [T, qs, detail] of [[0.25, 7, true], [0.5, 6.5, false]]) {
    const es = mk(); fill(es, k);
    const input = es.map(e => plain({ major: e.major, inB: e.inB, leg: e.leg, gl: e.gl, sLane: e.sLane, stop: e.stop, gaps: e.gaps, lanes: e.lanes, median: e.median, slip: e.slip, lmSrc: e.lmSrc, thru: e.thru, v: e.v, Lto: e.Lto }));
    M.priorityCaps(es, { T, qs, detail });
    F.priorityCaps.push({ shape: si, in: { ent: input, T, qs, detail }, out: es.map(e => plain({ res: e.res, g: e.g, n: e._n ? es.indexOf(e._n) : null, f: e._f ? es.indexOf(e._f) : null, o: e._o ? es.indexOf(e._o) : null, opp: e._opp ? es.indexOf(e._opp) : null })) });
  } });
  F.rbtCaps = [];
  const rents = [[null, 1], [{ lanes: [{ d: 'LT' }, { d: 'TR' }] }, 2], [{ lanes: [{ d: 'LT' }, { d: 'T' }, { d: 'TR', len: 30 }] }, 3], [{ lanes: ['LTR'], slip: true }, 1], [{ lanes: [{ d: 'L', len: 20 }, { d: 'TR' }] }, 2]];
  for (const [raw, gl] of rents) for (const nc of [0, 1, 2]) for (const vc of [0, 300, 900, 1500]) for (const [T, detail] of [[0.25, true], [1, false]]) {
    const e = { i: 0, gl, sLane: 900, ...lm(raw, gl), v: { L: 120 + vc / 10, T: 300, R: 90 }, Lto: {}, nc, vc };
    const input = plain({ gl: e.gl, sLane: e.sLane, lanes: e.lanes, slip: e.slip, v: e.v, nc, vc });
    M.rbtCaps([e], { T, qs: 7, detail });
    F.rbtCaps.push({ in: { e: input, T, detail }, out: plain(e.res) });
  }
  F.delayText = [];
  for (const d of [null, NaN, Infinity, 0, 0.4, 12.5, -0.4, 299.6, 300, 300.2, 1234]) for (const dos of [null, 0.5, 1, 1.004, 1.425, 3.335]) F.delayText.push({ in: [nn(plain(d)), dos], out: Engine.delayText(d, dos) });
  F.convText = [];
  for (const converged of [true, false]) for (const hitMax of [true, false]) for (const gap of [0, 0.00002, 0.00005, 0.0001234, 0.002, 0.25]) for (const stable of [true, false])
    F.convText.push({ in: { converged, hitMax, gap, it: 37, gapTol: 0.0001, stable, share: 0.98, consec: 4 }, out: Engine.convText({ converged, hitMax, gap, it: 37, gapTol: 0.0001, stable, share: 0.98, consec: 4 }) });
  F.approachName = [];
  for (const inB of [0, 44.9, 45, 90, 135, 180, 225, 270, 314.9, 315, 359.9, -30, 400, null, 'x']) for (const eight of [false, true]) F.approachName.push({ in: [nn(inB), eight], out: Engine.approachName(inB, eight) });
  F.turnType = [];
  for (const a of [0, 90, 181, 359]) for (const b of [0, 29, 31, 89, 150, 151, 210, 270, 330, 359]) F.turnType.push({ in: [a, b], out: Engine.turnType(a, b) });
  // Settings readers
  const settingsSets = [
    {}, { junction: { losMethod: 'sidra', pff: 0.9, T: 0.25, queueSpace: 6.5, f95: 1.5, capMethod: 'movement' } }, { junction: { losMethod: 'nope', pff: 1.5, T: -1, queueSpace: 0, f95: '2' } },
    { junction: null }, { gapTol: 0.001, maxIter: 50.6, conv: { stable: false, capDamp: 'none', chg: 0.02, share: 0.9, consec: 3.4 } }, { gapTol: -1, maxIter: 0, conv: { capDamp: null, chg: '0.05' } },
    { conv: { stable: 0 } }, { hvPct: 6, pcuHV: 2.3, co2Factor: 1.2 }, { hvPct: 10, pcuHV: '2', co2Factor: 'x' }, { hvPct: 0, pcuHV: 2.3, co2Factor: -1 },
  ];
  F.settings = settingsSets.map(S => ({
    in: JSON.stringify(S), junctionSet: plain(Engine.junctionSet(S)), capMethod: Engine.capMethodOf(S), convSet: plain(Engine.convSet({ gapTol: 0.0001, maxIter: 200, ...S })),
    convSetOpts: plain(Engine.convSet({ gapTol: 0.0001, maxIter: 200, ...S }, { gapTol: 0.002, maxIter: 33.5 })), pcuF: Engine.pcuF({ hvPct: 6, pcuHV: 2.3, ...S }),
    ef: [3, 5, 20, 50, 80, 130, 150].map(v => Engine.ef(v, { hvPct: 6, ...S })),
  }));
  F.growth = [];
  for (const S of [{ baseYear: 2026, year: 2026, growth: 1.5, growthMode: 'linear' }, { baseYear: 2026, year: 2031, growth: 2, growthMode: 'compound' }, { baseYear: '2024', year: 2030, growth: '3' }, { baseYear: 2026, year: 2026, growth: null }, { baseYear: 2030, year: 2026, growth: 50 }])
    for (const sc of [{}, { year: 2036 }, { year: '2041' }, { year: 0 }, { year: 'x' }].map(x => ({ id: 's', ...x })))
      F.growth.push({ in: { S: JSON.stringify(S), sc: JSON.stringify(sc) }, yearOf: Engine.yearOf(sc, S), growth: Engine.growth(S, sc), basis: Engine.growthBasis(S) });
  F.zoneClass = [{ kind: 'gateway' }, { role: 'committed' }, { role: 'development' }, { role: 'existing' }, { role: 'other' }, { existing: true }, { existing: 'true' }, {}].map(z => ({ id: 'z', ...z })).map(z => ({ in: JSON.stringify(z), out: Engine.zoneClass(z) }));
  F.tollSpeed = [];
  for (const L of [{ speed: 50 }, { speed: 0 }, { speed: '3' }, { speed: 80, varSpeed: { v: 60, AM: true } }, { speed: 80, varSpeed: { v: 0, AM: true } }, { varSpeed: { v: 100, PM: true } },
    { toll: 20 }, { toll: '15' }, { toll: '' }, { toll: -3 }, { toll: { AM: 12, PM: 'x' } }, { toll: [5] }, { toll: null }, { toll: true }].map(x => ({ id: 'l', a: 'n1', b: 'n2', ...x })))
    for (const period of ['AM', 'PM', null, '']) F.tollSpeed.push({ in: { L: JSON.stringify(L), period }, speed: Engine.speedOf(L, c.CLASSES.ramp, period), toll: Engine.tollOf(L, period) });
  // Array.prototype.sort as V8 does it (graph::js_sort_by): the order of element ids after sorting by key with
  // (a, b) => a.k - b.k and its reverse; keys with NaN (an inconsistent comparator) only below V8's 64-element run
  F.sort = [];
  {
    let s = 12345;
    const rnd = () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
    for (const len of [0, 1, 2, 3, 5, 8, 13, 21, 34, 63, 64, 65, 100, 150]) for (const withNaN of [false, true]) for (const rep of [0, 1, 2]) {
      if (withNaN && len >= 64) continue;
      const keys = Array.from({ length: len }, () => (withNaN && rnd() < 0.2 ? NaN : Math.floor(rnd() * (rep === 0 ? 5 : 1000))));
      const items = keys.map((k, i) => ({ k, i }));
      const up = items.slice().sort((a, b) => a.k - b.k).map(x => x.i), down = items.slice().sort((a, b) => b.k - a.k).map(x => x.i);
      F.sort.push({ keys: plain(keys), up, down });
    }
  }
  F.standards = plain(Engine.STANDARDS);
  writeFileSync(join(here, 'expected', 'junction-functions.json'), JSON.stringify(F));
  console.log('junction functions written');
}
