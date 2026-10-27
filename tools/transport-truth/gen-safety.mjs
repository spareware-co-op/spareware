// Records the prototype's crash analysis (data.js: casWindow, casTuple, loadAll for the crash and Future Connect layers,
// conflateFC, crashesNear, linkRisk, nodeRisk, junctionOf, riskBand, linkBand, safetySummary) for the Rust port's safety module
// (projects/spare-construct/crates/construct-transport/src/safety, feeds/load.rs, tests/safety_*.rs).
//
// Nothing is downloaded. The crash records and Future Connect sections are made up around the two test projects (the corridor and
// the grid, with a divided road added to each) in the shape the services answer (ArcGIS feature queries), and written to
// fixtures/data/ (cas-<name>.json, fc-<name>.json, cas-odd.json); the prototype loads them through a stand-in for fetch that
// answers like the server (where clause, envelope, order by OBJECTID, pages of 120 features with exceededTransferLimit).
// The Rust test serves the same pages from the recorded request log. Everything is deterministic (seeded); run twice, same files.
//
// Writes fixtures/data/cas-*.json, fc-*.json and expected/safety-cas.json (windows, tuples, loads), safety-bands.json,
// safety-conflate.json, safety-<name>.json (risk and summary variants for each test project).
// Run: node gen-safety.mjs [cas|bands|conflate|risk]   (no argument runs everything)
process.env.TZ = 'Pacific/Auckland';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import vm from 'node:vm';
import { JS_DIR } from './js-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const CORE = ['util', 'model', 'registry', 'engine', 'data', 'landuse', 'turns', 'calibrate', 'sim', 'sample'];
const NOW = Date.UTC(2026, 9, 21);
const inert = () => new Proxy(function () {}, { get: (t, k) => (k === Symbol.toPrimitive ? () => '' : inert()), apply: () => undefined });

// js-core.mjs's loader, with the clock and the fetch stand-in as arguments
function loadCtx({ seed = 1, now = NOW, fetchImpl = null } = {}) {
  let s = seed >>> 0;
  const rand = () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  const M = Object.create(Math); M.random = rand;
  const store = new Map();
  const ctx = {
    console, JSON, Promise, Map, Set, WeakMap, Array, Object, Number, String, Boolean, Symbol, Error, TypeError, RangeError,
    Float64Array, Float32Array, Int32Array, Uint32Array, Uint8Array, Int8Array, Uint16Array, ArrayBuffer, DataView, Intl, RegExp,
    Math: M, Date: class extends Date { constructor(...a) { super(...(a.length ? a : [now])); } static now() { return now; } },
    setTimeout, clearTimeout, queueMicrotask, structuredClone, TextEncoder, TextDecoder, URL, URLSearchParams, AbortController,
    performance: { now: () => 0 }, fetch: fetchImpl || ((...a) => fetch(...a)), isFinite, isNaN, parseFloat, parseInt, Infinity, NaN, encodeURIComponent, decodeURIComponent,
    localStorage: { getItem: k => store.get(k) ?? null, setItem: (k, v) => store.set(k, String(v)), removeItem: k => store.delete(k) },
    UI: inert(), Render: inert(), Inspector: inert(), Dialogs: inert(), Drawer: inert(),
    CSS: { escape: x => String(x) },
  };
  ctx.window = ctx; ctx.globalThis = ctx; ctx.self = ctx;
  vm.createContext(ctx);
  const src = CORE.map(n => `/* ${n}.js */\n` + readFileSync(join(JS_DIR, n + '.js'), 'utf8')).join('\n;\n');
  vm.runInContext(src + `\n;globalThis.__core = { U, Store, Net, Sites, Engine, DataHub, newProject, newScenario, defaultSettings, migrate, CLASSES, LANDUSES };`, ctx, { filename: 'transport-safety.js' });
  return ctx.__core;
}

// Typed arrays as plain arrays, Maps and Sets as objects and arrays, non-finite numbers as strings (as the other generators)
const plain = v => {
  if (ArrayBuffer.isView(v)) return Array.from(v, plain);
  if (v instanceof Map) return Object.fromEntries([...v].map(([k, x]) => [String(k), plain(x)]));
  if (v instanceof Set) return [...v].map(plain);
  if (typeof v === 'number') return Number.isFinite(v) ? v : String(v);
  if (Array.isArray(v)) return v.map(plain);
  if (v && typeof v === 'object') { const o = {}; for (const [k, x] of Object.entries(v)) if (typeof x !== 'function' && x !== undefined) o[k] = plain(x); return o; }
  return v;
};
const clone = o => JSON.parse(JSON.stringify(o));
const write = (rel, data) => { const t = JSON.stringify(data); writeFileSync(join(here, rel), t); console.log(rel, t.length, 'bytes'); };
const only = process.argv[2];
const wants = k => !only || only === k;
mkdirSync(join(here, 'fixtures', 'data'), { recursive: true });

let seed = 20261021;
const rnd = () => { seed = (seed + 0x6D2B79F5) >>> 0; let t = seed; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
const uni = (a, b) => a + (b - a) * rnd();
const pick = a => a[Math.floor(rnd() * a.length)];
const wpick = list => { const tot = list.reduce((s, [, w]) => s + w, 0); let x = rnd() * tot; for (const [v, w] of list) { if ((x -= w) < 0) return v; } return list[list.length - 1][0]; };
const r7 = x => Math.round(x * 1e7) / 1e7;

/* ---------------- the projects: the fixture and a divided road ---------------- */
const NAMES = ['corridor', 'grid'];
function prepare(name, opts = {}) {
  const c = loadCtx({ seed: 11, ...opts });
  const P = c.migrate(JSON.parse(readFileSync(join(here, 'inputs', name + '.json'), 'utf8')));
  c.Store.P = P;
  return { c, P, sc: P.scenarios[0] };
}
// A divided road (two one-way carriageways of one name, 14 m apart) in the first scenario, beside the first node
function addDivided(c, sc) {
  const { Net, U } = c, n0 = Object.values(sc.nodes)[0], F = U.flat(n0.lat);
  const at = (dx, dy) => [r7(n0.lat + 120 / F.ky + dy / F.ky), r7(n0.lng - 220 / F.kx + dx / F.kx)];
  const a = Net.addNode(sc, at(0, 0)), b = Net.addNode(sc, at(700, 0)), d = Net.addNode(sc, at(700, 14)), e = Net.addNode(sc, at(0, 14));
  const mid = Net.addNode(sc, at(350, 0));
  Net.addLink(sc, a, mid, [], { name: 'Divided Road', cls: 'arterial', lanesAB: 2, lanesBA: 0 });
  Net.addLink(sc, mid, b, [], { name: 'Divided Road', cls: 'arterial', lanesAB: 2, lanesBA: 0 });
  Net.addLink(sc, d, e, [at(350, 14)], { name: 'Divided Road', cls: 'arterial', lanesAB: 2, lanesBA: 0 });
  // a short stub off the divided road with a roundabout-like ring of three short links
  const r1 = Net.addNode(sc, at(350, -60)), r2 = Net.addNode(sc, at(362, -66)), r3 = Net.addNode(sc, at(338, -66));
  Net.addLink(sc, mid, r1, [], { name: 'Ring Street', cls: 'collector', lanesAB: 1, lanesBA: 1 });
  Net.addLink(sc, r1, r2, [], { name: 'Ring Street', cls: 'collector', lanesAB: 1, lanesBA: 0, ring: true });
  Net.addLink(sc, r2, r3, [], { name: 'Ring Street', cls: 'collector', lanesAB: 1, lanesBA: 0, ring: true });
  Net.addLink(sc, r3, r1, [], { name: 'Ring Street', cls: 'collector', lanesAB: 1, lanesBA: 0, ring: true });
  Net.addLink(sc, r2, a, [], { name: 'Spur Lane', cls: 'local', lanesAB: 1, lanesBA: 1 });
  Net.addLink(sc, r3, e, [], { name: 'Spur Lane', cls: 'local', lanesAB: 1, lanesBA: 1 });
  Net.addLink(sc, b, d, [], { name: 'Motorway Link', cls: 'ramp', lanesAB: 1, lanesBA: 0 });
  Net.addLink(sc, mid, d, [], { name: 'Motorway Link', cls: 'motorway', lanesAB: 2, lanesBA: 2 });
  // a diverge: three motorway and ramp links meet (an interchange merge, not a junction), the first one 900 m long
  const m1 = Net.addNode(sc, at(-700, 300)), m2 = Net.addNode(sc, at(200, 300)), m3 = Net.addNode(sc, at(380, 330)), m4 = Net.addNode(sc, at(420, 300));
  Net.addLink(sc, m1, m2, [], { name: 'Southern Motorway', cls: 'motorway', lanesAB: 3, lanesBA: 3 });
  Net.addLink(sc, m2, m4, [], { name: 'Southern Motorway', cls: 'motorway', lanesAB: 3, lanesBA: 3 });
  Net.addLink(sc, m2, m3, [], { name: 'Greenhithe Off Ramp', cls: 'ramp', lanesAB: 1, lanesBA: 0 });
}

/* ---------------- the crash records and Future Connect sections ---------------- */
const SEVERITIES = [['Fatal Crash', 0.05], ['Serious Crash', 0.12], ['Minor Crash', 0.28], ['Non-Injury Crash', 0.55]];
const upper = s => String(s || '').toUpperCase();
function makeCas(c, sc) {
  const { U, Net } = c, F = U.flat(Object.values(sc.nodes)[0].lat);
  let oid = 2000, feats = [];
  const years = [[2016, 0.7], [2017, 0.8], [2018, 0.9], [2019, 1], [2020, 0.7], [2021, 1], [2022, 1.1], [2023, 1.2], [2024, 1], [2025, 1], [2026, 0.7]];
  const mk = (ll, loc1, loc2) => {
    const sev = wpick(SEVERITIES);
    feats.push({
      attributes: {
        OBJECTID: oid++, crashSeverity: sev, crashYear: wpick(years), crashLocation1: upper(loc1), crashLocation2: loc2 ? upper(loc2) : null, speedLimit: pick([50, 50, 60, 80, 100]),
        pedestrian: rnd() < 0.06 ? 1 : 0, bicycle: rnd() < 0.05 ? 1 : 0, motorcycle: rnd() < 0.07 ? 1 : 0,
        fatalCount: sev === 'Fatal Crash' ? 1 : 0, seriousInjuryCount: sev === 'Serious Crash' ? pick([1, 1, 2]) : 0, minorInjuryCount: sev === 'Minor Crash' ? pick([1, 2, 3]) : pick([0, 0, 1]),
        light: pick(['Bright sun', 'Overcast', 'Twilight', 'Dark', 'Bright sun']), weatherA: pick(['Fine', 'Fine', 'Light rain', 'Heavy rain', 'Mist or Fog']),
        trafficControl: pick(['Nil', 'Nil', 'Give way', 'Stop', 'Traffic signals']), urban: pick(['Urban', 'Urban', 'Open']),
      },
      geometry: { x: r7(ll[1]), y: r7(ll[0]) },
    });
  };
  const off = (ll, bearingDeg, m) => { const b = bearingDeg * Math.PI / 180; return [ll[0] + m * Math.cos(b) / F.ky, ll[1] + m * Math.sin(b) / F.kx]; };
  const links = Object.values(sc.links).filter(L => sc.nodes[L.a] && sc.nodes[L.b]);
  const names = [...new Set(links.map(L => L.name).filter(Boolean))];
  for (const L of links) {
    const g = Net.geom(sc, L), cum = U.cumLen(g), len = cum[cum.length - 1];
    const n = L.cls === 'arterial' || L.cls === 'motorway' ? 3 + Math.floor(rnd() * 7) : L.cls === 'collector' ? 1 + Math.floor(rnd() * 5) : Math.floor(rnd() * 3);
    for (let k = 0; k < n; k++) {
      const t = uni(0.05, 0.95), a = U.along(g, cum, len * t), segB = U.bearing(g[Math.max(0, a.i)], g[Math.min(g.length - 1, a.i + 1)]);
      const r = rnd(), d = r < 0.7 ? uni(-15, 15) : r < 0.9 ? pick([-1, 1]) * uni(15, 25) : pick([-1, 1]) * uni(26, 45);
      const other = rnd() < 0.08 ? pick(names) : L.name, cross = rnd() < 0.5 ? pick(names) : null;
      mk(off(a.p, segB + 90, d), other || L.name, cross);
    }
  }
  // crashes at junctions, some just beyond 30 m
  const inc = new Map();
  for (const L of links) for (const nid of [L.a, L.b]) { const a = inc.get(nid); if (a) a.push(L); else inc.set(nid, [L]); }
  for (const [nid, ls] of inc) {
    if (ls.length < 3) continue;
    const nd = sc.nodes[nid], n = 2 + Math.floor(rnd() * 4);
    for (let k = 0; k < n; k++) {
      const m = rnd() < 0.85 ? uni(0, 28) : uni(31, 45), a = off([nd.lat, nd.lng], uni(0, 360), m), L1 = pick(ls), L2 = pick(ls);
      mk(a, L1.name || 'Unnamed', L2.name);
    }
  }
  for (let k = 0; k < 6; k++) { const n0 = Object.values(sc.nodes)[0]; mk([n0.lat + uni(-0.02, 0.02), n0.lng + uni(-0.02, 0.02)], pick(names), null); }
  return feats;
}
// Records that are missing, odd or unusual, for the tuple reader only
function oddCas() {
  const g = (y, x) => ({ x, y });
  return [
    { attributes: { OBJECTID: 9001, crashSeverity: 'fatal', crashYear: 2024, crashLocation1: "O'CONNOR RD", crashLocation2: 'MĀNUKA ROAD', speedLimit: 60, pedestrian: 1, fatalCount: '1', seriousInjuryCount: '', minorInjuryCount: null, light: '', weatherA: 'Fine', trafficControl: null, urban: 'Urban' }, geometry: g(-37.1, 174.95) },
    { attributes: { OBJECTID: '77', crashSeverity: 'Unknown', crashYear: '2024', crashLocation1: 'st mary\'s avenue', crashLocation2: '', bicycle: 2, motorcycle: true, speedLimit: null }, geometry: g(-37.10001, 174.95002) },
    { attributes: { crashSeverity: null, crashLocation1: '2nd avenue', crashLocation2: 'a_b road' }, geometry: g(-37.2, 174.9) },
    { attributes: { crashSeverity: '', crashYear: 2020, crashLocation1: 0, crashLocation2: 'x' }, geometry: g(-37.15, 174.91) },
    { geometry: g(-37.1234567891, 174.9876543219) },
    { attributes: { crashSeverity: 'Minor Crash', crashYear: 2023, fatalCount: 'x', seriousInjuryCount: 2.5, minorInjuryCount: 0, light: 5, weatherA: 'Fine', OBJECTID: 12, trafficControl: 'Nil', urban: 'Open' }, geometry: g(-37.18, 174.97) },
    { attributes: { crashSeverity: 'Serious Crash', crashYear: 2025, crashLocation1: 'Great  South   Road', crashLocation2: 'ÉCOLE STREET', pedestrian: 0, bicycle: 0, motorcycle: 0, speedLimit: 100, OBJECTID: 4 }, geometry: g(-37.0, 175.0) },
    { attributes: { crashSeverity: 'Non-Injury Crash', crashYear: 2021, crashLocation1: 'MAIN ROAD' }, geometry: g('-37.5', '174.5') },
    { attributes: { crashSeverity: 'Minor Crash', crashYear: 2022, urban: 'Urban' }, geometry: { x: 174.9, y: undefined } },
  ];
}

// FC sections: the link's polyline moved a few metres, with decoys (rail only, other names, crossing roads, parallel roads)
const GT = { motorway: 'Motorway', ramp: 'Strategic', arterial: 'Primary', collector: 'Secondary', local: 'Local' };
function makeFc(c, sc) {
  const { U, Net } = c, F = U.flat(Object.values(sc.nodes)[0].lat);
  let oid = 500, feats = [];
  const links = Object.values(sc.links).filter(L => sc.nodes[L.a] && sc.nodes[L.b]);
  const shift = (g, m) => g.map((p, i) => { const b = U.bearing(g[Math.max(0, i - 1)], g[Math.min(g.length - 1, i + 1)]) + 90, t = b * Math.PI / 180; return [p[0] + m * Math.cos(t) / F.ky, p[1] + m * Math.sin(t) / F.kx]; });
  const section = (paths, name, o = {}) => feats.push({
    attributes: { OBJECTID: oid++, street_name: name, generaltraffic_current: o.gt ?? null, publictransport_current: o.pt ?? null, freight_current: o.freight ?? null, cycle_current: o.cycle ?? null,
      generaltraffic_first_decade: o.gt10 ?? o.gt ?? null, publictransport_first_decade: o.pt10 ?? o.pt ?? null, freight_first_decade: o.freight10 ?? o.freight ?? null, cycle_first_decade: o.cycle10 ?? o.cycle ?? null },
    geometry: { paths: paths.map(p => p.map(q => [r7(q[1]), r7(q[0])])) },
  });
  const abbr = s => String(s || '').replace(/\bRoad\b/, 'Rd').replace(/\bStreet\b/, 'St').replace(/\bAvenue\b/, 'Ave');
  for (const L of links) {
    const g = Net.geom(sc, L), r = rnd();
    const cls = { gt: GT[L.cls] || 'Local', pt: pick([null, 'Frequent', 'Local', null]), freight: pick([null, null, 'Secondary', 'Primary']), cycle: pick([null, 'Local', 'Secondary']) };
    if (r < 0.1) continue;                                           // no section for this road
    if (r < 0.25) { const cum = U.cumLen(g), len = cum[cum.length - 1]; section([[U.along(g, cum, 0).p, U.along(g, cum, len * 0.4).p]], abbr(L.name), cls); continue; }   // covers only the start: one vote
    if (r < 0.32) { section([shift(g, 40)], L.name, cls); continue; }  // runs 40 m away: out of range
    if (r < 0.4) { section([shift(g, 5)], 'Some Other Street', cls); continue; }   // another street's name
    section([shift(g, uni(-6, 6))], r < 0.6 ? abbr(L.name) : L.name || '', cls);
  }
  // rail only, nothing set, a crossing road (bearing 90 degrees off), a road the other way round, a MultiLineString with a gap
  const L0 = links[0], g0 = Net.geom(sc, L0), cum0 = U.cumLen(g0), mid = U.along(g0, cum0, cum0[cum0.length - 1] / 2).p;
  section([shift(g0, 2)], L0.name, { freight: 'Rail' });
  section([shift(g0, 3)], L0.name, {});
  const b0 = U.bearing(g0[0], g0[g0.length - 1]) + 90, t0 = b0 * Math.PI / 180;
  section([[[mid[0] - 80 * Math.cos(t0) / F.ky, mid[1] - 80 * Math.sin(t0) / F.kx], [mid[0] + 80 * Math.cos(t0) / F.ky, mid[1] + 80 * Math.sin(t0) / F.kx]]], L0.name, { gt: 'Strategic' });
  section([shift(g0, 1).reverse()], L0.name, { gt: 'Primary', pt: 'Rapid' });
  section([shift(g0, 1).slice(0, 1 + Math.floor(g0.length / 2)), shift(g0, 1).slice(Math.floor(g0.length / 2))], L0.name, { gt: 'Primary' });
  // a feature with a missing geometry and one with no paths (left out by the reader)
  feats.push({ attributes: { OBJECTID: oid++, street_name: 'No Geometry', generaltraffic_current: 'Local' } });
  feats.push({ attributes: { OBJECTID: oid++, street_name: 'No Paths', generaltraffic_current: 'Local' }, geometry: { x: 174.9, y: -37.1 } });
  return feats;
}

// fixtures/data/cas-<name>.json, fc-<name>.json and cas-odd.json
const FIX = {};
for (const name of NAMES) {
  const { c, sc } = prepare(name);
  addDivided(c, sc);
  const proj = join(here, 'fixtures', 'data', `safety-${name}-project.json`);
  writeFileSync(proj, c.Store.serialise(c.Store.P));
  FIX[name] = { project: readFileSync(proj, 'utf8'), cas: { features: makeCas(c, sc) }, fc: { features: makeFc(c, sc) } };
  writeFileSync(join(here, 'fixtures', 'data', `cas-${name}.json`), JSON.stringify(FIX[name].cas));
  writeFileSync(join(here, 'fixtures', 'data', `fc-${name}.json`), JSON.stringify(FIX[name].fc));
  console.log(name, 'crashes', FIX[name].cas.features.length, 'fc sections', FIX[name].fc.features.length);
}
const ODD = { features: oddCas() };
writeFileSync(join(here, 'fixtures', 'data', 'cas-odd.json'), JSON.stringify(ODD));

/* ---------------- a fetch that answers like the services ---------------- */
// Pages of PAGE features; the where clause (crashYear>=N, 1=1), the envelope and order by OBJECTID are applied. The log keeps each request's
// address and the positions (in the fixture's own list) of the features it answered, and whether the server said more was waiting.
const PAGE = 120;
function server(layers, log) {
  return async url => {
    const u = new URL(url), p = u.searchParams;
    const key = Object.keys(layers).find(k => u.pathname.includes(k));
    if (!key) { log.push({ url, idx: [], none: true }); return { ok: false, status: 404, json: async () => ({}) }; }
    const all = layers[key].features;
    let idx = all.map((_, i) => i);
    const w = p.get('where') || '1=1', m = /^crashYear>=(\d+)$/.exec(w);
    if (m) idx = idx.filter(i => all[i].attributes && all[i].attributes.crashYear >= +m[1]);
    const [x0, y0, x1, y1] = (p.get('geometry') || '').split(',').map(Number);
    const inside = (x, y) => x >= x0 && x <= x1 && y >= y0 && y <= y1;
    idx = idx.filter(i => { const g = all[i].geometry; if (!g) return false; if (g.x != null) return inside(g.x, g.y); return (g.paths || []).some(pa => pa.some(q => inside(q[0], q[1]))); });
    idx.sort((a, b) => (all[a].attributes.OBJECTID ?? 0) - (all[b].attributes.OBJECTID ?? 0));
    const off = +p.get('resultOffset') || 0, page = idx.slice(off, off + PAGE), more = off + PAGE < idx.length;
    log.push({ url, idx: page, exceeded: more });
    return { ok: true, status: 200, json: async () => ({ features: page.map(i => all[i]), ...(more ? { exceededTransferLimit: true } : {}) }) };
  };
}
const boundsOf = (sc, pad = 0.05) => {
  const ns = Object.values(sc.nodes), s = Math.min(...ns.map(n => n.lat)), n = Math.max(...ns.map(n => n.lat)), w = Math.min(...ns.map(n => n.lng)), e = Math.max(...ns.map(n => n.lng));
  const h = (n - s) * pad, wd = (e - w) * pad, b = { south: s - h, west: w - wd, north: n + h, east: e + wd };
  return { ...b, getSouth: () => b.south, getWest: () => b.west, getNorth: () => b.north, getEast: () => b.east };
};
const boxOf = b => ({ south: b.south, west: b.west, north: b.north, east: b.east });

// A project from the fixture, loaded the way the app loads it (fresh context so no cache carries over)
function open(name, { now = NOW, log = [] } = {}) {
  const layers = { CAS_Data_Public: FIX[name].cas, FutureConnect_IntegratedNetwork: FIX[name].fc };
  const c = loadCtx({ seed: 11, now, fetchImpl: server(layers, log) });
  c.Store.P = c.migrate(JSON.parse(FIX[name].project));
  return { c, P: c.Store.P, log };
}

/* ---------------- casWindow, casTuple ---------------- */
async function casCases() {
  const c = loadCtx();
  const D = c.DataHub, out = { window: [], tuples: [], fields: { fields: D.CAS_FIELDS, pos: D.CAS_POS, window: D.CAS_WINDOW }, loads: [] };
  // instants around the new year, 1 April, daylight saving changes and the end of the year in New Zealand
  const dates = [Date.UTC(2026, 9, 21), Date.UTC(2026, 0, 10), Date.UTC(2025, 11, 31, 10, 59), Date.UTC(2025, 11, 31, 11, 0), Date.UTC(2026, 2, 31, 10, 59, 59), Date.UTC(2026, 2, 31, 11, 0), Date.UTC(2026, 3, 1, 10, 59), Date.UTC(2026, 3, 1, 11, 0),
    Date.UTC(2027, 0, 1), Date.UTC(2024, 1, 29, 12), Date.UTC(2020, 5, 15), Date.UTC(2030, 11, 31, 23, 59), Date.UTC(2026, 8, 26, 13, 59), Date.UTC(2026, 8, 26, 14, 0), Date.UTC(2027, 3, 2, 13, 59), Date.UTC(2027, 3, 2, 14, 0), Date.UTC(2008, 3, 5, 13, 59), Date.UTC(2008, 3, 5, 14, 0)];
  for (const t of dates) for (const years of [undefined, 5, 10, 1, 3.4, 2.5, 0, -2, null, 'x', '7', NaN]) {
    out.window.push({ now: t, years: years === undefined ? 'undefined' : years === null ? 'null' : Number.isNaN(years) ? 'NaN' : years, result: plain(years === undefined ? D.casWindow(new Date(t)) : D.casWindow(new Date(t), years)) });
  }
  for (const [label, list] of [['odd', ODD.features], ...NAMES.map(n => [n, FIX[n].cas.features])]) {
    out.tuples.push({ label, tuples: list.filter(f => f.geometry).map(f => { try { return JSON.parse(JSON.stringify(D.casTuple(f))); } catch (e) { return { error: e.message }; } }) });
  }
  // the loads: the crash layer, ten years, Future Connect, together; the time of the load moves the window
  for (const name of NAMES) {
    const loads = [
      { label: 'cas', which: ['cas'], now: NOW }, { label: 'cas-10-years', which: ['cas'], opts: { casYears: 10 }, now: NOW }, { label: 'cas-3-years', which: ['cas'], opts: { casYears: 3 }, now: NOW },
      { label: 'cas-and-10', which: ['cas', 'cas10'], now: NOW }, { label: 'cas-january', which: ['cas', 'cas10'], now: Date.UTC(2026, 0, 10) },
      { label: 'fc', which: ['fc'], now: NOW }, { label: 'fc-and-cas', which: ['cas', 'fc', 'cas10'], now: NOW }, { label: 'unknown-source', which: ['cas', 'rail', 'fc'], now: NOW },
    ];
    for (const L of loads) {
      const o = open(name, { now: L.now });
      const st = [], msgs = await o.c.DataHub.loadAll(boundsOf(o.P.scenarios[0]), L.which, t => st.push(t), L.opts || {});
      const d = o.P.data;
      out.loads.push({ project: name, ...L, bounds: boxOf(boundsOf(o.P.scenarios[0])), requests: o.log, msgs, statuses: st, data: plain(JSON.parse(JSON.stringify({ crashes: d.crashes, crashesLong: d.crashesLong, loaded: d.loaded, fc: d.fc }))),
        fcLinks: plain(Object.fromEntries(o.P.scenarios.map(s => [s.id, Object.fromEntries(Object.values(s.links).filter(l => l.fc).map(l => [l.id, l.fc]))]))) });
    }
  }
  return out;
}

/* ---------------- risk bands ---------------- */
function bandCases() {
  const c = loadCtx(), D = c.DataHub, out = { risk: [], link: [] };
  const values = [null, undefined, NaN, Infinity, -Infinity, -1, 0, 0.0399999, 0.04, 0.0699, 0.07, 0.1099, 0.11, 0.1899, 0.19, 0.5, 1, 2, 3.99, 4, 4.99, 5, 6.99, 7, 8.99, 9, 20, 100, 0.0999, 0.1, 0.2, 0.39, 0.4, 0.59, 0.6, 0.99, 1.99, 3.9999, 3.99999];
  for (const kind of ['collective', 'personal', 'injury', 'node', 'unknown', undefined]) for (const v of values) {
    out.risk.push({ value: v === undefined ? 'undefined' : v, kind: kind === undefined ? 'undefined' : kind, result: plain(kind === undefined ? D.riskBand(v) : D.riskBand(v, kind)) });
  }
  const rs = [null, { short: true, lowVolume: false, personal: 3, perKmYr: 1, dsiPerKmYr: 0.2 }, { short: false, lowVolume: true, aadt: 0, personal: 5, perKmYr: 2, dsiPerKmYr: 0.08 },
    { short: false, lowVolume: true, aadt: 150, personal: 5, perKmYr: 2, dsiPerKmYr: 0.08 }, { short: false, lowVolume: true, aadt: null, personal: null, perKmYr: null, dsiPerKmYr: null },
    { short: false, lowVolume: false, aadt: 4000, personal: 8, perKmYr: 0.7, dsiPerKmYr: 0.12 }, { short: false, lowVolume: false, aadt: 4000, personal: null, perKmYr: 5, dsiPerKmYr: null }];
  for (const kind of ['collective', 'personal', 'injury', 'node', undefined]) for (const r of rs) {
    out.link.push({ r, kind: kind === undefined ? 'undefined' : kind, result: plain(kind === undefined ? D.linkBand(r) : D.linkBand(r, kind)) });
  }
  return out;
}

/* ---------------- conflateFC ---------------- */
async function conflateCases() {
  const out = { cases: [] };
  for (const name of NAMES) {
    const cases = [
      { label: 'default' }, { label: 'radius-8', opts: { radius: 8 } }, { label: 'tight-bearing', opts: { radius: 30, bearing: 10 } }, { label: 'zero-options', opts: { radius: 0, bearing: 0 } },
      { label: 'user-and-stale', setup: 'user' }, { label: 'closed-links', setup: 'closed' }, { label: 'scenario-1', scIndex: 1 }, { label: 'second-pass-after-first', setup: 'twice' },
    ];
    for (const k of cases) {
      const o = open(name), sc = o.P.scenarios[k.scIndex || 0];
      await o.c.DataHub.loadAll(boundsOf(o.P.scenarios[0]), ['fc'], () => {}, {});
      // drop what the load put on the links, so the call below is the one recorded
      for (const s of o.P.scenarios) for (const L of Object.values(s.links)) delete L.fc;
      const links = Object.values(sc.links);
      if (k.setup === 'user' || k.setup === 'closed') {
        links.forEach((L, i) => {
          if (k.setup === 'user') { if (i % 7 === 0) L.fc = { user: true, gt: 'Collector', src: 'user' }; else if (i % 7 === 1) L.fc = { src: 'AT Future Connect', gt: 'Local', fid: 1 }; else if (i % 7 === 2) L.fc = { src: 'Someone else', gt: 'Local' }; }
          else if (i % 4 === 0) L.status = 'closed';
        });
      }
      const feats = o.P.data.fc;
      const input = clone({ links: Object.fromEntries(links.map(L => [L.id, L.fc ?? null])), statuses: Object.fromEntries(links.map(L => [L.id, L.status])) });
      let res = o.c.DataHub.conflateFC(sc, feats, k.opts || {});
      if (k.setup === 'twice') res = o.c.DataHub.conflateFC(sc, feats, k.opts || {});
      out.cases.push({ project: name, ...k, scenario: sc.id, input, result: plain(res), fcs: plain(Object.fromEntries(Object.values(sc.links).map(L => [L.id, L.fc ?? null]))) });
    }
  }
  return out;
}

/* ---------------- risk, junctions, summary ---------------- */
// Counts for the first scenario's links: two-way and directional, one direction, on the one-way carriageways, switched off, virtual, RAMM
const DIRS = ['NB', 'EB', 'SB', 'WB'];
const compass = b => DIRS[Math.round((((b % 360) + 360) % 360) / 90) % 4];
function addCounts(c, sc) {
  const { U, Net } = c, counts = [], links = Object.values(sc.links);
  links.forEach((L, i) => {
    const g = Net.geom(sc, L), cum = U.cumLen(g), len = cum[cum.length - 1], mid = U.along(g, cum, len / 2).p, b0 = U.bearing(g[0], g[g.length - 1]);
    const at = (dLat = 0, dLng = 0) => [r7(mid[0] + dLat), r7(mid[1] + dLng)];
    const two = L.lanesAB > 0 && L.lanesBA > 0, road = L.name || 'Unnamed ' + i;
    if (two) {
      switch (i % 5) {
        case 0: { const ll = at(0.00002, 0); counts.push({ id: `at:${i}`, src: 'at', ref: String(i), name: `${road} · A to B`, road, lat: ll[0], lng: ll[1], adt: 9000 + i * 130, peak: 900, peakTime: '07:45', directional: false }); break; }
        case 1: for (const [d, b] of [['ab', b0], ['ba', (b0 + 180) % 360]]) { const ll = at(0.00001, d === 'ab' ? 0.00002 : -0.00002); counts.push({ id: `nzta:${i}${d}`, src: 'nzta', ref: `SH${i}${d}`, name: `${road} ${d}`, road, lat: ll[0], lng: ll[1], adt: 5000 + i * 40 + (d === 'ab' ? 0 : 700), dir: compass(b), directional: true, virtual: false, ramp: false }); } break;
        case 2: { const ll = at(-0.00001, 0.00001); counts.push({ id: `nzta:${i}`, src: 'nzta', ref: `SH${i}`, name: `${road} one way`, road, lat: ll[0], lng: ll[1], adt: 3000 + i * 25, dir: compass(b0), directional: true, virtual: false, ramp: false }); break; }
        case 3: { const ll = at(0, 0.00001); counts.push({ id: `user:${i}`, src: 'user', name: road, road, lat: ll[0], lng: ll[1], am: 400, pm: 380, adt: 150 + i, directional: false }); break; }
        case 4: { const ll = at(0.00003, 0.00003); counts.push({ id: `ramm:${i}`, src: 'ramm', kind: 'ramm_estimate', name: `${road} (RAMM)`, road, lat: ll[0], lng: ll[1], adt: 2000 + i * 11 }); counts.push({ id: `nzta:v${i}`, src: 'nzta', ref: `V${i}`, name: 'Virtual', road, lat: at(0, -0.00002)[0], lng: at(0, -0.00002)[1], adt: 12000, virtual: true, directional: false, ramp: false }); break; }
      }
    } else {
      // one-way carriageways: a count in its own direction, or a two-way total with no direction
      const ll = at(0.00001, 0);
      if (i % 2) counts.push({ id: `nzta:ow${i}`, src: 'nzta', ref: `OW${i}`, name: road, road, lat: ll[0], lng: ll[1], adt: 11000, dir: compass(L.lanesAB > 0 ? b0 : (b0 + 180) % 360), directional: true, virtual: false, ramp: false });
      else counts.push({ id: `at:ow${i}`, src: 'at', ref: `OW${i}`, name: `${road} · C to D`, road, lat: ll[0], lng: ll[1], adt: 14000, directional: false });
    }
  });
  counts.push({ id: 'user:off', src: 'user', name: 'Off', road: links[0].name, lat: Object.values(sc.nodes)[0].lat, lng: Object.values(sc.nodes)[0].lng, adt: 50000, off: true, directional: false });
  counts.push({ id: 'at:far', src: 'at', ref: 'far', name: 'Nowhere', road: 'Nowhere Road', lat: -35.5, lng: 174.1, adt: 7000, directional: false });
  return counts;
}

const NODE_R_LIST = [25, 40, 10];
async function riskVariants(name) {
  const variants = [
    { label: 'plain', now: NOW, load: ['cas', 'cas10'] },
    { label: 'counts', now: NOW, load: ['cas'], counts: true },
    { label: 'model-current', now: NOW, load: ['cas'], model: { year: 2026 } },
    { label: 'model-current-and-counts', now: NOW, load: ['cas'], model: { year: 2026 }, counts: true },
    { label: 'model-forecast', now: NOW, load: ['cas'], model: { year: 2031 } },
    { label: 'model-forecast-base-current', now: NOW, load: ['cas'], model: { year: 2031, baseYear: 2026 } },
    { label: 'no-load-time', now: NOW, load: ['cas'], mutate: P => { delete P.data.loaded.cas.at; } },
    { label: 'no-window', now: NOW, load: ['cas'], mutate: P => { delete P.data.loaded.cas; } },
    { label: 'january', now: Date.UTC(2026, 0, 10), load: ['cas'] },
    { label: 'december', now: Date.UTC(2025, 11, 31, 10, 30), load: ['cas'] },
    { label: 'ten-years', now: NOW, load: ['cas10'], mutate: P => { P.data.crashes = P.data.crashesLong; P.data.loaded.cas = P.data.loaded.casLong; } },
    { label: 'no-crashes', now: NOW, load: [] },
  ];
  const out = { project: name, variants: [] };
  for (const v of variants) {
    const o = open(name, { now: v.now }), P = o.P;
    if (v.load.length) await o.c.DataHub.loadAll(boundsOf(P.scenarios[0]), v.load, () => {}, {});
    if (v.counts) { const d = o.c.DataHub.D(); d.counts = addCounts(o.c, P.scenarios[0]); d.loaded.nzta = { at: NOW, n: 1 }; d.loaded.at = { at: NOW, n: 1 }; d.loaded.user = { at: NOW, n: 1 }; }
    if (v.mutate) v.mutate(P);
    if (v.model) {
      P.settings.year = v.model.year;
      if (v.model.baseYear) P.scenarios[0].year = v.model.baseYear;
      for (const s of P.scenarios) for (const p of ['AM', 'PM']) s.res[p] = await o.c.Engine.assign(s, { period: p });
    }
    const projectText = o.c.Store.serialise(P);
    const rec = { label: v.label, now: v.now, projectText, scenarios: [] };
    for (const sc of P.scenarios) {
      // a fresh context per scenario: the prototype keeps one cache
      const q = open(name, { now: v.now }); q.c.Store.P = q.c.migrate(JSON.parse(projectText));
      const D = q.c.DataHub, s2 = q.c.Store.P.scenarios.find(s => s.id === sc.id);
      q.c.Store.P.active = s2.id;
      const r = { id: sc.id, linkRisk: plain(D.linkRisk(s2)), nodeRisk: plain(D.nodeRisk(s2)), summary: plain(D.safetySummary(s2)) };
      r.junctionOf = plain(Object.fromEntries(Object.keys(s2.nodes).map(id => [id, D.junctionOf(s2, id)])));
      if (v.model) { r.linkRiskAM = plain(D.linkRisk(s2, s2.res.AM)); r.summaryPM = plain(D.safetySummary(s2, s2.res.PM)); }
      if (v.label === 'plain' || v.label === 'ten-years') {
        r.near = [];
        for (const R of NODE_R_LIST) {
          for (const id of Object.keys(s2.nodes)) r.near.push({ type: 'node', id, R, result: plain(D.crashesNear(s2, { type: 'node', id }, R)) });
          for (const id of Object.keys(s2.links)) r.near.push({ type: 'link', id, R, result: plain(D.crashesNear(s2, { type: 'link', id }, R)) });
        }
        r.near.push({ type: 'node', id: 'missing', R: 25, result: plain(D.crashesNear(s2, { type: 'node', id: 'missing' })) });
        r.near.push({ type: 'link', id: 'missing', R: 25, result: plain(D.crashesNear(s2, { type: 'link', id: 'missing' })) });
        r.near.push({ type: 'other', id: 'x', R: 25, result: plain(D.crashesNear(s2, { type: 'zone', id: 'x' })) });
        r.nearDefault = plain(D.crashesNear(s2, { type: 'link', id: Object.keys(s2.links)[3] }));
        if (v.label === 'plain') r.nearLong = Object.keys(s2.links).slice(0, 12).map(id => ({ id, result: plain(D.crashesNear(s2, { type: 'link', id }, 25, { long: true })) }));
      }
      rec.scenarios.push(r);
    }
    out.variants.push(rec);
  }
  return out;
}

/* ---------------- main ---------------- */
if (wants('cas')) write('expected/safety-cas.json', await casCases());
if (wants('bands')) write('expected/safety-bands.json', bandCases());
if (wants('conflate')) write('expected/safety-conflate.json', await conflateCases());
if (wants('risk')) for (const name of NAMES) write(`expected/safety-risk-${name}.json`, await riskVariants(name));
