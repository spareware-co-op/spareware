// Records the prototype's Drury sample (sample.js: the sample project built from downloaded data, its assessment set, the runs and the report tests)
// for the Rust port's sample module (projects/spare-construct/crates/construct-transport/src/sample, tests/sample_*.rs).
//
// No network is used. The downloaded answers are made up here (deterministic): a Drury road network (Overpass format), the made-up suburb of
// fixtures/landuse-papatoetoe.json and the Census, employees, school and zoning answers of fixtures/data/census-answers.json (run gen-census.mjs first), all moved
// to Drury, and NZTA and Auckland Transport counts and CAS crashes on the made-up Drury roads. The prototype runs with `fetch` stubbed; every request is recorded
// with its answer (kept once by request in fixtures/data/sample-exchanges.json).
// Loaded for real: landuse-plus.js, calibration.js and junctions.js. Stood in for (made-up behaviour, written below and mirrored in the Rust test): the feeds,
// compliance, compliance-extra and report features, which other ports cover. Every call into the stand-ins and every answer of Junctions.approaches is recorded.
// Writes
//   expected/sample-constants.json  BOUNDS, ILLUSTRATIVE, COMMITTED, PROPOSAL, TG ... and turnCountCSV on every junction site of the built network
//   expected/sample-helpers.json    clipWays, simplify, pruneStubs, optionA, optionB, nodeOn, siteWith, names, compass on made-up inputs
//   expected/sample-build.json      build(): requests, status, notes, failed, the project afterwards, the stand-in calls; and analyse()
// Run: node gen-sample.mjs (the same input always gives the same files). The prototype works out New Zealand dates, so the script runs in that time zone.
process.env.TZ = 'Pacific/Auckland';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import vm from 'node:vm';
import { JS_DIR } from './js-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const NOW = Date.UTC(2026, 9, 21);
const FILES = ['util', 'model', 'registry', 'engine', 'data', 'landuse', 'turns', 'calibrate', 'sim', 'sample', 'kit', 'features/junctions', 'features/landuse-plus', 'features/calibration'];
const CORE_ONLY = ['util', 'model', 'registry', 'engine', 'data', 'landuse', 'turns', 'calibrate', 'sim', 'sample'];
const inert = () => new Proxy(function () {}, { get: (t, k) => (k === Symbol.toPrimitive ? () => '' : inert()), apply: () => undefined });
mkdirSync(join(here, 'fixtures', 'data'), { recursive: true });
mkdirSync(join(here, 'expected'), { recursive: true });
const readJson = rel => JSON.parse(readFileSync(join(here, rel), 'utf8'));
const writeExpected = (name, data) => { const t = JSON.stringify(data); writeFileSync(join(here, 'expected', name), t); console.log('expected/' + name, t.length, 'bytes'); };
export const jsonSafe = v => (v === undefined ? undefined : JSON.parse(JSON.stringify(v)));
const r7 = x => Math.round(x * 1e7) / 1e7;

class LatLngBounds {
  constructor(a, b) { if (!a) return; for (const p of (b ? [a, b] : a)) this.extend(p); }
  extend(p) {
    const ll = Array.isArray(p) ? { lat: p[0], lng: p[1] } : p;
    if (!this._sw && !this._ne) { this._sw = { lat: ll.lat, lng: ll.lng }; this._ne = { lat: ll.lat, lng: ll.lng }; }
    else { this._sw.lat = Math.min(ll.lat, this._sw.lat); this._sw.lng = Math.min(ll.lng, this._sw.lng); this._ne.lat = Math.max(ll.lat, this._ne.lat); this._ne.lng = Math.max(ll.lng, this._ne.lng); }
    return this;
  }
  pad(r) { const sw = this._sw, ne = this._ne, hb = Math.abs(sw.lat - ne.lat) * r, wb = Math.abs(sw.lng - ne.lng) * r; return new LatLngBounds({ lat: sw.lat - hb, lng: sw.lng - wb }, { lat: ne.lat + hb, lng: ne.lng + wb }); }
  getCenter() { return { lat: (this._sw.lat + this._ne.lat) / 2, lng: (this._sw.lng + this._ne.lng) / 2 }; }
  getSouth() { return this._sw.lat; } getWest() { return this._sw.lng; } getNorth() { return this._ne.lat; } getEast() { return this._ne.lng; }
  contains(p) { const ll = Array.isArray(p) ? { lat: p[0], lng: p[1] } : p; return ll.lat >= this._sw.lat && ll.lat <= this._ne.lat && ll.lng >= this._sw.lng && ll.lng <= this._ne.lng; }
}
const Leaflet = { latLngBounds: (a, b) => new LatLngBounds(a, b) };

/* ------------------------------------------------------------------------------------------------------------------------------------
   The stand-ins for the features other ports cover. Behaviour is made up; each call is recorded in __calls (name and plain arguments) and is steered by __fake.
   ------------------------------------------------------------------------------------------------------------------------------------ */
const FAKES = `
globalThis.__calls = []; globalThis.__fake = globalThis.__fake || {};
const __log = (...a) => { globalThis.__calls.push(a); };
const Feeds = {
  nslr: { all: async (near, scs, status) => { __log('nslr', scs.length); if (__fake.nslr) throw new Error(__fake.nslr); status('Reading speed limits'); let n = 0; for (const sc of scs) for (const L of Object.values(sc.links)) if (L.cls === 'arterial' && L.speed === 60) { L.speed = 50; n++; } return n; } },
  tms: { sites: () => ['A', 'B'], load: async (sites, o) => { __log('tms', sites.length); if (__fake.tms) throw new Error(__fake.tms); o.status('Reading TMS'); for (const c of DataHub.D().counts) if (c.src === 'nzta') c.tms = { n: 1 }; } },
  hazards: {
    load: async (near, o) => { __log('hazards'); if (__fake.hazards === 'none') return { layers: [], errors: ['flood service said no'] }; if (__fake.hazards === 'none-silent') return { layers: [], errors: [] }; if (__fake.hazards) throw new Error(__fake.hazards); o.status('Reading flood layers'); return { layers: [{ id: 'flood', name: 'Flood plains' }, { id: 'coast', name: 'Coastal inundation' }], errors: [] }; },
    links: sc => { __log('hazard-links'); return Object.values(sc.links).slice(0, 3).map(L => ({ link: L.id, flood: 120, coast: 0 })); },
  },
  gtfs: {
    catalogue: async near => { __log('gtfs-catalogue'); if (__fake.gtfs === 'throw') throw new Error('catalogue down'); return __fake.gtfs === 'none' ? { feeds: [] } : { feeds: [{ id: 'at-gtfs' }, { id: 'other' }] }; },
    load: async (feed, near, o) => { __log('gtfs-load', feed.id); o.status('Reading timetable'); return { id: feed.id }; },
    routes: (sc, g) => { __log('gtfs-routes'); return [{ id: 'r1' }, { id: 'r2' }, { id: 'r3' }]; },
    apply: (sc, routes) => { __log('gtfs-apply', routes.length); sc.routes.rGtfs = { id: 'rGtfs', name: 'Bus 3', color: '#7048e8', freq: 4, wp: [] }; },
    keep: (g, routes) => { __log('gtfs-keep', routes.length); },
  },
};
const Compliance = {
  state: P => { P.compliance ||= { ctx: {}, site: {} }; return P.compliance; },
  lookupSite: async P => { __log('lookup-site'); if (__fake.co === 'lookup') throw new Error('council lookup down'); P.compliance.site.zone = 'Future Urban'; },
  loadSH: async (P, o) => { __log('load-sh', o.sc.code); if (__fake.co === 'sh') throw new Error('state highway layer down'); P.compliance.sh = o.sc.id; },
  loadOverlays: async (P, o) => { __log('load-overlays', o.sc.code); if (__fake.co === 'overlays') throw new Error(''); P.compliance.overlays = 2; },
  recordOutputs: P => { __log('record-outputs'); P.compliance.outputs = P.scenarios.length; },
};
const ComplianceExtra = {
  data: P => (P.cx ||= { precinct: { id: null, register: [], proposal: null }, mitigation: [], conditions: null }),
  seedMitigation: P => { __log('seed-mitigation'); if (__fake.cx === 'seed') throw new Error('no effects yet'); const n = __fake.cxN ?? 14; const d = ComplianceExtra.data(P); d.mitigation = [{ id: 'm1', seed: 'diff:1', effect: 'e0' }, { id: 'm2', seed: 'diff:2' }, { id: 'm3', seed: 'hand' }];
    for (let k = 0; k < n; k++) d.mitigation.push({ id: 'x' + k, seed: 'eff:' + k, effect: 'e' + (k % 12) }); },
  effects: P => { __log('effects'); return Array.from({ length: 12 }, (_, k) => (k % 3 === 0 ? { id: 'e' + k, dos: 0.4 + (k % 5) * 0.15 } : k % 3 === 1 ? { id: 'e' + k, vc: 0.6 + (k % 4) * 0.1 } : { id: 'e' + k, dos: 0, vc: 0 })); },
  seedConditions: P => { __log('seed-conditions'); if (__fake.cx === 'conditions') throw new Error('conditions table missing'); ComplianceExtra.data(P).conditions = { n: ComplianceExtra.data(P).mitigation.length }; },
};
const Report = {
  staging: async (P, o) => { __log('staging', o.steps.join(',')); if (__fake.rp === 'staging') throw new Error('staging failed'); for (let k = 0; k < o.steps.length; k++) o.onProgress(k + 1, o.steps.length, o.steps[k] + '% built'); (P.report.staging = o.steps); },
  sensitivity: async (P, o) => { __log('sensitivity', o.tests.join(',')); if (__fake.rp === 'sens') throw new Error('sensitivity failed'); for (let k = 0; k < o.tests.length; k++) o.onProgress(k + 1, o.tests.length, o.tests[k]); (P.report.sens = o.tests); },
};
`;

// js-core.mjs's loader plus the features, the stand-ins and Leaflet's bounds; sample.js also hands out its helpers (as __h) so they can be tested directly
export function load({ seed = 1, now = NOW, fetchImpl = null, features = true, fakes = true, fake = {} } = {}) {
  let s = seed >>> 0;
  const rand = () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  const M = Object.create(Math); M.random = rand;
  const store = new Map();
  const ctx = {
    console, JSON, Promise, Map, Set, WeakMap, WeakSet, Array, Object, Number, String, Boolean, Symbol, Error, TypeError, RangeError,
    Float64Array, Float32Array, Int32Array, Uint32Array, Uint8Array, Int8Array, Uint16Array, ArrayBuffer, DataView, Intl, RegExp,
    Math: M, Date: class extends Date { constructor(...a) { super(...(a.length ? a : [now])); } static now() { return now; } },
    setTimeout, clearTimeout, queueMicrotask, structuredClone, TextEncoder, TextDecoder, URL, URLSearchParams, AbortController,
    performance: { now: () => 0 }, fetch: fetchImpl || ((...a) => { throw new Error('no network in the truth tests: ' + a[0]); }), isFinite, isNaN, parseFloat, parseInt, Infinity, NaN, encodeURIComponent, decodeURIComponent,
    localStorage: { getItem: k => store.get(k) ?? null, setItem: (k, v) => store.set(k, String(v)), removeItem: k => store.delete(k) },
    UI: inert(), Render: inert(), Inspector: inert(), Dialogs: inert(), Drawer: inert(),
    CSS: { escape: x => String(x) }, L: Leaflet, __fake: fake,
  };
  ctx.window = ctx; ctx.globalThis = ctx; ctx.self = ctx;
  vm.createContext(ctx);
  const src = (features ? FILES : CORE_ONLY).map(n => {
    let t = readFileSync(join(JS_DIR, n + '.js'), 'utf8');
    if (n === 'sample') t = t.replace('return { build, analyse, drury, turnCountCSV,', 'return { __h: { clipWays, pruneStubs, simplify, optionA, optionB, nodeOn, siteWith, names, compass }, build, analyse, drury, turnCountCSV,');
    return `/* ${n}.js */\n` + t;
  }).join('\n;\n') + (features && fakes ? '\n;' + FAKES : '');
  vm.runInContext(src + `\n;globalThis.__core = { U, Store, Net, Sites, Scenarios, Importers, Engine, DataHub, Calibrate, Register, ExistingLU, Sample, newProject, newScenario, defaultSettings, migrate, CLASSES, LANDUSES${features ? ', Junctions, LandusePlus, Calibration' : ''} };`, ctx, { filename: 'transport-sample.js' });
  const core = ctx.__core;
  core.ctx = ctx;
  if (features) {
    // record what Junctions.approaches answers each time the sample asks (the sample's own calls only, not the editor's)
    const J = core.Junctions, orig = J.approaches;
    ctx.__approaches = [];
    J.approaches = (sc, nodeId) => { const r = orig(sc, nodeId); ctx.__approaches.push({ node: nodeId, out: r.map(a => ({ link: a.link.id, gl: a.gl })) }); return r; };
  }
  return core;
}

export function idStart(seed, opts = {}) {
  const c = load({ seed, now: NOW, ...opts });
  const id = c.U.uid('t');
  let s = seed >>> 0;
  const draws = Array.from({ length: 12 }, () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; });
  const m = /^t(.{4})([0-9a-z]+?)([0-9a-z]{3})$/.exec(id);
  const skip = draws.findIndex(d => d.toString(36).slice(2, 5) === m[3]);
  if (skip < 0) throw new Error('the id does not come from the seeded generator: ' + id);
  return { rngSkip: skip, seq0: parseInt(m[2], 36) };
}

/* ------------------------------------------------------------------------------------------------------------------------------------
   A made-up Drury: roads in Overpass's answer format, with the names the sample looks for
   ------------------------------------------------------------------------------------------------------------------------------------ */
const KM = 88700, KY = 110574;
function drury({ withFitzgerald = true, withSutton = true, withWaihoehoe = true, only = null } = {}) {
  let nid = 3100000000, wid = 41200000;
  const els = [], J = {}, JLL = {};
  const node = (lat, lon, tags) => { const n = { type: 'node', id: ++nid, lat: r7(lat), lon: r7(lon) }; if (tags) n.tags = tags; els.push(n); return n.id; };
  const jn = (key, lat, lon, tags) => { JLL[key] = [lat, lon]; J[key] = node(lat, lon, tags); };
  jn('GSR_N', -37.0935, 174.9510); jn('RBN', -37.103365, 174.9500); jn('RBE', -37.1035, 174.95017); jn('RBS', -37.103635, 174.9500); jn('RBW', -37.1035, 174.94983);
  jn('WHI', -37.1030, 174.9462, { highway: 'traffic_signals' }); jn('S1A', -37.1020, 174.9441); jn('S1B', -37.1046, 174.9443);
  jn('WFa', -37.1042, 174.9588, { highway: 'traffic_signals' }); jn('WFb', -37.10442, 174.9589); jn('TW', -37.10475, 174.96005, { highway: 'give_way' }); jn('APJ', -37.1052, 174.9700);
  jn('APE', -37.1006, 174.9722); jn('GSR_S', -37.1132, 174.9490); jn('TKN', -37.0990, 174.96000); jn('SUTE', -37.0936, 174.9552); jn('QF', -37.1134, 174.9595); jn('QT', -37.1135, 174.9601); jn('FN', -37.0960, 174.9586);
  jn('RRS', -37.09286, 174.9400); jn('RRW', -37.10268, 174.9400); jn('RRQ', -37.11307, 174.9400); jn('JA', -37.0996, 174.9800); jn('JW', -37.10586, 174.9800); jn('JQ', -37.1138, 174.9800);
  jn('GB', -37.0850, 174.9515); jn('FB', -37.0850, 174.9590); jn('FLB', -37.0850, 174.9730); jn('JB', -37.0850, 174.9801);
  const ways = [];
  const way = (tags, pts) => {
    const ids = []; let prev = null;
    pts.forEach((p, i) => {
      const ll = typeof p === 'string' ? JLL[p] : p;
      if (prev) {
        const dy = (ll[0] - prev[0]) * KY, dx = (ll[1] - prev[1]) * KM, d = Math.hypot(dx, dy), k = Math.floor(d / 85);
        for (let j = 1; j <= k; j++) { const f = j / (k + 1), amp = 1.4 * Math.sin(j * 1.7 + i); ids.push(node(prev[0] + (ll[0] - prev[0]) * f + (dx / d) * amp / KY, prev[1] + (ll[1] - prev[1]) * f + (-dy / d) * amp / KM)); }
      }
      ids.push(typeof p === 'string' ? J[p] : node(ll[0], ll[1]));
      prev = ll;
    });
    ways.push({ type: 'way', id: ++wid, nodes: ids, tags });
  };
  const T = (highway, name, extra = {}) => ({ highway, name, ...extra });
  way(T('motorway', 'Southern Motorway', { ref: 'SH1', lanes: '3', maxspeed: '100' }), [[-37.0750, 174.9430], [-37.0900, 174.9436], 'S1A', 'S1B', [-37.1150, 174.9450], [-37.1300, 174.9455]]);
  way(T('motorway_link', '', { oneway: 'yes', lanes: '1' }), ['WHI', [-37.1026, 174.9452], 'S1A']);
  way(T('motorway_link', '', { oneway: 'yes', lanes: '1' }), ['S1B', [-37.1034, 174.9452], 'WHI']);
  way(T('primary', 'Great South Road', { lanes: '2', maxspeed: '60', 'cycleway:both': 'lane' }), [[-37.0800, 174.9520], 'GB', 'GSR_N', [-37.0985, 174.9505], 'RBN']);
  way(T('primary', 'Great South Road', { junction: 'roundabout', lanes: '2', maxspeed: '30' }), ['RBN', 'RBE', 'RBS', 'RBW', 'RBN']);
  way(T('primary', 'Great South Road', { lanes: '2', maxspeed: '60' }), ['RBS', [-37.1080, 174.9495], 'GSR_S', [-37.1250, 174.9480]]);
  {
    const WN = withWaihoehoe ? 'Waihoehoe Road' : 'Whau Road';
    way(T('secondary', WN, { lanes: '2', maxspeed: '60', 'bus:lanes': 'designated|yes' }), ['RBW', 'WHI', 'RRW', [-37.1025, 174.9300]]);
    way(T('secondary', WN, { lanes: '2', maxspeed: '60' }), ['RBE', [-37.1038, 174.9540], 'WFa', 'WFb', 'TW', 'APJ', [-37.1058, 174.9770], 'JW', [-37.1062, 174.9850], [-37.1068, 174.9990]]);
  }
  way(T('secondary', 'Takotowhenua Avenue', { lanes: '2', maxspeed: '50' }), ['TKN', [-37.1020, 174.9600], 'TW', [-37.1100, 174.9601], 'QT', [-37.1260, 174.9602]]);
  if (withFitzgerald) {
    way(T('tertiary', 'Fitzgerald Road', { maxspeed: '50' }), ['FB', 'FN', 'WFa']);
    way(T('tertiary', 'Fitzgerald Road', { maxspeed: '50' }), ['WFb', [-37.1100, 174.9592], 'QF', [-37.1260, 174.9600]]);
  } else {
    way(T('tertiary', 'Ellis Road'), ['FB', 'FN', 'WFa']);
    way(T('tertiary', 'Ellis Road'), ['WFb', [-37.1100, 174.9592], 'QF', [-37.1260, 174.9600]]);
  }
  way(T('tertiary', 'Appleby Road', { maxspeed: '60' }), [[-37.0985, 174.9850], 'JA', [-37.1000, 174.9770], 'APE', [-37.1030, 174.9705], 'APJ']);
  way(T('tertiary', 'Flanagan Road', { maxspeed: '60' }), ['APE', [-37.0960, 174.9725], 'FLB']);
  way(T('secondary', withSutton ? 'Sutton Road' : 'Sinton Road', { lanes: '2', maxspeed: '60' }), [[-37.0925, 174.9300], 'RRS', [-37.0930, 174.9440], 'GSR_N', 'SUTE']);
  way(T('unclassified', 'Quarry Road', { maxspeed: '80' }), [[-37.1128, 174.9300], 'RRQ', 'GSR_S', 'QF', 'QT', 'JQ', [-37.1140, 174.9990]]);
  way(T('secondary', 'Hunua Road', { lanes: '2', maxspeed: '80' }), ['GSR_S', [-37.1190, 174.9620], [-37.1260, 174.9700]]);
  way(T('tertiary', 'Burtt Road', {}), ['FN', [-37.0975, 174.9595], 'TKN']);
  way(T('secondary', 'Ramarama Road', { lanes: '2', maxspeed: '80' }), [[-37.0800, 174.9398], 'RRS', 'RRW', 'RRQ', [-37.1300, 174.9405]]);
  way(T('tertiary', 'Jesmond Road', { maxspeed: '80' }), [[-37.0800, 174.9802], 'JB', 'JA', 'JW', 'JQ', [-37.1300, 174.9798]]);
  way(T('tertiary', 'Burnside Road', { maxspeed: '60' }), [[-37.0850, 174.9300], 'GB', 'FB', 'FLB', 'JB', [-37.0850, 174.9990]]);
  way(T('residential', 'Hillview Place'), ['WFa', [-37.1030, 174.9620]]);
  way(T('residential', 'Oakland Drive'), ['APJ', [-37.1070, 174.9690]]);
  way(T('living_street', 'Kowhai Lane'), ['TW', [-37.1060, 174.9625]]);
  way(T('residential', 'Kauri Close', { access: 'private' }), ['RBE', [-37.1045, 174.9510]]);
  const all = [...els, ...ways];
  const out = only ? all.filter(only) : all;
  return { version: 0.6, generator: 'Overpass API (made up)', elements: out };
}

/* ---- counts and crashes on the made-up roads, as ArcGIS answers them (geometry x = longitude, y = latitude) ---- */
const pt = (lat, lng) => ({ x: lng, y: lat });
const NZTA = [
  { siteref: '01N00480', description: 'SH1 Southern Motorway at Drury NB', lane: 'NB', sh: '1N', sitetype: 'Telemetry', type: 'Permanent', percentheavy: 10.4, aadt1yearago: 71200, aadt2yearsago: 70100, aadt3yearsago: 68000, accepteddays: 360, at: [-37.0990, 174.9433] },
  { siteref: '01S00481', description: 'SH1 Southern Motorway at Drury SB', lane: 'SB', sh: '1S', sitetype: 'Telemetry', type: 'Permanent', percentheavy: 10.1, aadt1yearago: 70400, aadt2yearsago: 69300, aadt3yearsago: 67500, accepteddays: 362, at: [-37.0992, 174.9434] },
  { siteref: '01N00490', description: 'SH1 Drury interchange off ramp', lane: 'NB', sh: '1N', sitetype: 'Short Term', type: 'Short Term', percentheavy: 5.0, aadt1yearago: 11800, aadt2yearsago: 11200, accepteddays: 14, at: [-37.1034, 174.9452] },
  { siteref: '01N00510', description: 'SH1 south of Drury NB', lane: 'NB', sh: '1N', sitetype: 'Telemetry', type: 'Permanent', percentheavy: 11.2, aadt1yearago: 64000, accepteddays: 340, at: [-37.1200, 174.9458] },
];
const NZTA_FEATURES = NZTA.map((s, i) => ({ attributes: { OBJECTID: i + 1, ...Object.fromEntries(Object.entries(s).filter(([k]) => k !== 'at')) }, geometry: pt(...s.at) }));
const AT_DEFS = [
  ['GREAT SOUTH ROAD', 'SUTTON ROAD', 'WAIHOEHOE ROAD', -37.0960, 174.9507, 18300, 1450], ['GREAT SOUTH ROAD', 'WAIHOEHOE ROAD', 'QUARRY ROAD', -37.1085, 174.9495, 16100, 1390],
  ['GREAT SOUTH ROAD', 'QUARRY ROAD', 'RAMARAMA ROAD', -37.1190, 174.9484, 12100, 1010], ['GREAT SOUTH ROAD', 'HUNUA ROAD', 'SUTTON ROAD', -37.0840, 174.9516, 14500, 1250],
  ['WAIHOEHOE ROAD', 'GREAT SOUTH ROAD', 'FITZGERALD ROAD', -37.1038, 174.9550, 7400, 640], ['WAIHOEHOE ROAD', 'FITZGERALD ROAD', 'APPLEBY ROAD', -37.1052, 174.9650, 5200, 470],
  ['WAIHOEHOE ROAD', 'SH1', 'GREAT SOUTH ROAD', -37.1028, 174.9420, 8800, 790], ['FITZGERALD ROAD', 'WAIHOEHOE ROAD', 'QUARRY ROAD', -37.1100, 174.9592, 3900, 360],
  ['TAKOTOWHENUA AVENUE', 'WAIHOEHOE ROAD', 'QUARRY ROAD', -37.1085, 174.9601, 4600, 410], ['TAKOTOWHENUA AVENUE', 'WAIHOEHOE ROAD', 'BURTT ROAD', -37.1020, 174.9600, 2800, 260],
  ['SUTTON ROAD', 'GREAT SOUTH ROAD', 'SH1', -37.0929, 174.9400, 6200, 560], ['QUARRY ROAD', 'GREAT SOUTH ROAD', 'FITZGERALD ROAD', -37.1133, 174.9540, 2100, 210],
  ['HUNUA ROAD', 'GREAT SOUTH ROAD', 'RAMARAMA ROAD', -37.1190, 174.9620, 5400, 480], ['APPLEBY ROAD', 'WAIHOEHOE ROAD', 'FLANAGAN ROAD', -37.1030, 174.9705, 1900, 180],
];
const AT_FEATURES = AT_DEFS.map(([road, a, b, lat, lng, adt, peak], i) => ({ attributes: { OBJECTID: 5001 + i, latest: 'Yes', road_name: road, start_name: a, end_name: b, count_date: Date.UTC(2025, 2 + (i % 8), 11 + (i % 10), 11), peak_hour: i % 2 ? '745' : '1630', count_duration: '7 days', adt, peaktraffic: peak, pcheavy: 4 + (i % 5), carr_way_no: 2000 + i }, geometry: pt(lat, lng) }));
let seed = 20261022;
const rnd = () => { seed = (seed + 0x6D2B79F5) >>> 0; let t = seed; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
const CAS_FEATURES = Array.from({ length: 40 }, (_, i) => {
  const d = AT_DEFS[i % AT_DEFS.length], sev = ['Non-Injury Crash', 'Non-Injury Crash', 'Minor Crash', 'Minor Crash', 'Serious Crash', 'Fatal Crash'][Math.floor(rnd() * 6)];
  return { attributes: { OBJECTID: 9000 + i, crashSeverity: sev, crashYear: [2021, 2022, 2023, 2024, 2025, 2026][Math.floor(rnd() * 6)], crashLocation1: d[0], crashLocation2: d[1], speedLimit: [50, 60, 80, 100][Math.floor(rnd() * 4)], pedestrian: rnd() < 0.08 ? 1 : 0, bicycle: rnd() < 0.06 ? 1 : 0, motorcycle: rnd() < 0.07 ? 1 : 0,
    fatalCount: sev === 'Fatal Crash' ? 1 : 0, seriousInjuryCount: sev === 'Serious Crash' ? 1 : 0, minorInjuryCount: sev === 'Minor Crash' ? 1 : 0, light: 'Overcast', weatherA: 'Fine', trafficControl: 'Nil', urban: 'Open' }, geometry: pt(d[3] + (rnd() - 0.5) * 0.0008, d[4] + (rnd() - 0.5) * 0.001) };
});
const ANSWERS = { nzta: NZTA_FEATURES, at: AT_FEATURES, cas: CAS_FEATURES };

/* ---- the made-up suburb and the Census, employees, school and zoning answers, moved to the middle of the Drury study box ---- */
const suburb = readJson('fixtures/landuse-papatoetoe.json'), census = readJson('fixtures/data/census-answers.json');
const SHIFT = { lat: -37.103 - -36.97, lon: 174.961 - 174.86 };
const mv = (lat, lon) => [r7(lat + SHIFT.lat), r7(lon + SHIFT.lon)];
const moveEl = e => {
  const g = p => { const [la, lo] = mv(p.lat, p.lon); return { ...p, lat: la, lon: lo }; };
  const o = { ...e };
  if (e.lat !== undefined) { o.lat = g(e).lat; o.lon = g(e).lon; }
  if (e.geometry) o.geometry = e.geometry.map(g);
  if (e.bounds) o.bounds = { minlat: e.bounds.minlat + SHIFT.lat, minlon: e.bounds.minlon + SHIFT.lon, maxlat: e.bounds.maxlat + SHIFT.lat, maxlon: e.bounds.maxlon + SHIFT.lon };
  if (e.members) o.members = e.members.map(m => ({ ...m, geometry: m.geometry && m.geometry.map(g) }));
  return o;
};
const buildings = suburb.elements.filter(e => e.tags && e.tags.building).map(moveEl), areas = suburb.elements.filter(e => !(e.tags && e.tags.building)).map(moveEl);
const moveRings = f => (f.geometry ? { ...f, geometry: { rings: f.geometry.rings.map(r => r.map(([x, y]) => { const [la, lo] = mv(y, x); return [r6(lo), r6(la)]; })) } } : f);
const r6 = x => Math.round(x * 1e6) / 1e6;
const moveLayer = L => ({ ...L, features: L.features.map(moveRings) });
const CENSUS = { census: Object.fromEntries(Object.entries(census.census).map(([k, v]) => [k, moveLayer(v)])), employees: moveLayer(census.employees), aup: moveLayer(census.aup),
  schools: census.schools.map(r => (r.Latitude == null ? r : { ...r, Latitude: mv(r.Latitude, r.Longitude)[0], Longitude: mv(r.Latitude, r.Longitude)[1] })),
  ece: census.ece.map(r => (r.Latitude == null ? r : { ...r, Latitude: mv(r.Latitude, r.Longitude)[0], Longitude: mv(r.Latitude, r.Longitude)[1] })) };

/* ---- the stubbed network ---- */
const EXCHANGE_FILE = join(here, 'fixtures', 'data', 'sample-exchanges.json');
const EXCHANGES = {};
const exchangeKey = (method, url, body) => `${method} ${url}${body ? ' | ' + body : ''}`;
export function makeNet({ fail = {}, roads = null, answers = {}, shared = true, pageSize = 6 } = {}) {
  const A = { ...ANSWERS, ...answers }, log = [];
  const layerOf = url => (url.includes('dwellings_by_SA1') ? 'dw' : url.includes('households_by_SA1') ? 'hh' : url.includes('individuals_by_SA1/FeatureServer/0') ? 'ind0' : url.includes('individuals_by_SA1/FeatureServer/2') ? 'ind2' : url.includes('employees_by_statistical') ? 'employees' : url.includes('Unitary_Plan_Base_Zone') ? 'aup' : null);
  const inEnvelope = (g, env) => g && g.x >= env[0] && g.x <= env[2] && g.y >= env[1] && g.y <= env[3];
  const fetchImpl = async (url, init) => {
    const method = init && init.method ? init.method : 'GET', body = init && init.body ? String(init.body) : undefined;
    const entry = { method, url: String(url) };
    if (body) entry.body = body;
    log.push(entry);
    const key = exchangeKey(method, entry.url, body);
    const answer = (status, a) => { entry.status = status; if (a !== undefined) { if (shared && !roads && !Object.keys(answers).length) { EXCHANGES[key] = a; entry.ans = key; } else entry.answer = a; } return { ok: status >= 200 && status < 300, status, json: async () => JSON.parse(JSON.stringify(a)) }; };
    const f = Object.entries(fail).find(([pat]) => String(url).includes(pat) || (body && decodeURIComponent(body).includes(pat)));
    if (f) {
      const how = f[1];
      if (how === 'network') { entry.failure = 'network'; throw new TypeError('fetch failed'); }
      if (how === 'abort') { entry.failure = 'cancelled'; throw Object.assign(new Error('The operation was aborted'), { name: 'AbortError' }); }
      if (typeof how === 'number') return answer(how);
      if (how === 'unreadable') { entry.failure = 'unreadable'; entry.status = 200; return { ok: true, status: 200, json: async () => { throw new SyntaxError('Unexpected token < in JSON at position 0'); } }; }
    }
    const u = new URL(String(url));
    if (method === 'POST') {
      const q = decodeURIComponent(String(body).replace(/^data=/, ''));
      if (q.includes('["highway"~')) return answer(200, roads || ROADS);
      const m = /\(([-\d.]+),([-\d.]+),([-\d.]+),([-\d.]+)\)/.exec(q), [bs, bw, bn, be] = m ? m.slice(1).map(Number) : [-90, -180, 90, 180];
      const ext = e => { const g = e.geometry || (e.members || []).flatMap(x => x.geometry || []); if (e.lat !== undefined) return [e.lat, e.lon, e.lat, e.lon]; const la = g.map(p => p.lat), lo = g.map(p => p.lon); return [Math.min(...la), Math.min(...lo), Math.max(...la), Math.max(...lo)]; };
      const meets = e => { const [a, b, c2, d] = ext(e); return a <= bn && c2 >= bs && b <= be && d >= bw; };
      return answer(200, { version: 0.6, generator: 'Overpass API (made up)', elements: (q.includes('["building"]') ? buildings : areas).filter(meets) });
    }
    if (u.host === 'catalogue.data.govt.nz') { const sql = u.searchParams.get('sql') || ''; return answer(200, { success: true, result: { records: sql.includes('a9d65b07') ? CENSUS.ece : CENSUS.schools } }); }
    const layer = layerOf(String(url));
    if (layer) {
      const L = layer === 'employees' || layer === 'aup' ? CENSUS[layer] : CENSUS.census[layer];
      if (!u.pathname.endsWith('/query')) return answer(200, { currentVersion: 11.1, name: layer, fields: L.fields });
      const offset = +u.searchParams.get('resultOffset') || 0, geom = u.searchParams.get('returnGeometry') === 'true', want = (u.searchParams.get('outFields') || '').split(',');
      const size = layer === 'dw' ? pageSize : 1000, page = L.features.slice(offset, offset + size);
      const features = page.map(f => ({ attributes: Object.fromEntries(Object.entries(f.attributes).filter(([k]) => want.includes(k))), ...(geom && f.geometry ? { geometry: f.geometry } : {}) }));
      return answer(200, { objectIdFieldName: 'OBJECTID', features, ...(offset + size < L.features.length ? { exceededTransferLimit: true } : {}) });
    }
    const src = u.pathname.includes('Assets_SHTrafficMonitoringSites') ? 'nzta' : u.pathname.includes('TrafficService') ? 'at' : u.pathname.includes('CAS_Data_Public') ? 'cas' : null;
    if (!src) return answer(404);
    const [w, s, e, n] = u.searchParams.get('geometry').split(',').map(Number), offset = +u.searchParams.get('resultOffset') || 0, where = u.searchParams.get('where') || '1=1';
    let list = A[src].filter(x => !x.geometry || inEnvelope(x.geometry, [w, s, e, n]));
    if (src === 'at') list = list.filter(x => (where.includes("latest='Yes'") ? x.attributes.latest === 'Yes' : true));
    if (src === 'cas') { const mm = /crashYear>=(\d+)/.exec(where); if (mm) list = list.filter(x => x.attributes.crashYear >= +mm[1]); }
    const want = (u.searchParams.get('outFields') || '*').split(',');
    const page = list.slice(offset, offset + 5).map(x => ({ attributes: want[0] === '*' ? x.attributes : Object.fromEntries(Object.entries(x.attributes).filter(([k]) => want.includes(k))), ...(x.geometry ? { geometry: x.geometry } : {}) }));
    return answer(200, { objectIdFieldName: 'OBJECTID', features: page, ...(offset + 5 < list.length ? { exceededTransferLimit: true } : {}) });
  };
  return { fetchImpl, log };
}
const ROADS = drury();

/* ------------------------------------------------------------------------------------------------------------------------------------
   The harness
   ------------------------------------------------------------------------------------------------------------------------------------ */
const resSummary = P => jsonSafe(P.scenarios.map(s => ({ id: s.id, res: Object.fromEntries(Object.entries(s.res || {}).map(([k, r]) => [k, r && { period: r.period, it: r.it, gap: r.gap, converged: r.converged, stale: r.stale, kpi: r.kpi }])) })));
const stateOf = P => { const o = jsonSafe(P); for (const sc of o.scenarios) delete sc.res; if (o.sample) delete o.sample.size; return o; };
const sizeOf = P => (P.sample && P.sample.size) ?? null;

let caseNo = 0;
const cases = [];
// One build (and, with `analyse`, a second step on the project it made). cancelOn: the status text that cancels the work.
async function run(name, { features = true, fake = {}, opts = {}, netOpts = {}, cancelOn = null, analyse = null, helperTurns = false } = {}) {
  const seedNo = 600 + caseNo++, net = makeNet(netOpts), c = load({ seed: seedNo, fetchImpl: net.fetchImpl, features, fake });
  const ac = new AbortController(), status = [];
  const onStatus = t => { status.push(t); if (cancelOn && t.includes(cancelOn)) ac.abort(); };
  const rec = { name, seed: seedNo, now: NOW, ...idStart(seedNo, { features, fake }), features, fake, opts, fail: netOpts.fail || {}, roads: netOpts.roads ? 'inline' : 'shared', cancelOn };
  let r = null;
  try { r = await c.Sample.build({ ...opts, onStatus, signal: ac.signal }); } catch (e) { rec.error = { message: e.message, stage: e.sampleStage ?? null, abort: e.name === 'AbortError' }; }
  rec.log = net.log; rec.status = status; rec.calls = jsonSafe(c.ctx.__calls || []); rec.approaches = jsonSafe(c.ctx.__approaches || []);
  if (r) {
    Object.assign(rec, { notes: [...r.notes], failed: r.failed, ms: r.ms, scenarios: Object.fromEntries(Object.entries(r.scenarios).map(([k, s]) => [k, s.id])) });
    if (helperTurns) {
      const S0 = r.P.scenarios.find(s => s.role === 'existing'), out = {};
      for (const st of Object.values(S0.sites || {})) { const t = c.Sample.turnCountCSV(S0, st, {}); out[st.id] = jsonSafe(t); }
      out.named = Object.values(S0.sites || {}).slice(0, 1).map(st => jsonSafe(c.Sample.turnCountCSV(S0, st, { name: 'Named junction', date: '2026-04-01' })));
      rec.turnCsv = out;
    }
    rec.size = sizeOf(r.P); rec.res = resSummary(r.P); rec.after = stateOf(r.P);
    if (analyse) {
      const a = analyse, calls0 = c.ctx.__calls.length, st0 = status.length, ac2 = new AbortController();
      const onS2 = t => { status.push(t); if (a.cancelOn && t.includes(a.cancelOn)) ac2.abort(); };
      const ao = { notes: r.notes, failed: r.failed, onStatus: onS2, signal: ac2.signal, extras: a.extras };
      if (a.ownRuns) ao.runs = async scs => { for (const sc of scs) for (const p of ['AM', 'PM']) sc.res[p] = await c.Engine.assign(sc, { period: p }); };
      const arec = { opts: { extras: a.extras ?? null, ownRuns: !!a.ownRuns, cancelOn: a.cancelOn ?? null }, statusFrom: st0 };
      try { const o = await c.Sample.analyse(r.P, ao); arec.out = { ms: o.ms, size: o.size }; } catch (e) { arec.error = { message: e.message, abort: e.name === 'AbortError' }; }
      Object.assign(arec, { notes: r.notes, calls: jsonSafe(c.ctx.__calls.slice(calls0)), res: resSummary(r.P), after: stateOf(r.P), size: sizeOf(r.P) });
      rec.analyse = arec;
    }
  }
  cases.push(rec);
}

async function partBuild() {
  await run('full', { helperTurns: true });
  await run('full with the runs', { opts: { run: true } });
  await run('built, then analysed with the host runs', { analyse: { ownRuns: true, extras: false } });
  await run('built, then analysed with the default runs', { analyse: {} });
  await run('built, then analysis cancelled in the second peak', { analyse: { cancelOn: 'Fitting the PM peak' } });
  await run('no optional modules', { features: false, fake: {} });
  await run('no bus timetable asked for', { opts: { gtfs: false } });
  await run('the feeds fail', { fake: { nslr: 'speed limit register is down', tms: 'TMS is down', hazards: 'none', gtfs: 'none' } });
  await run('flood answers with no layers and no errors', { fake: { hazards: 'none-silent', gtfs: 'throw' } });
  await run('compliance lookups fail', { fake: { co: 'lookup' } });
  await run('compliance state highways fail', { fake: { co: 'sh' } });
  await run('compliance overlays fail with no message', { fake: { co: 'overlays' } });
  await run('the mitigation register cannot be seeded', { opts: { run: true }, fake: { cx: 'seed' } });
  await run('the conditions cannot be seeded, many open effects', { opts: { run: true }, fake: { cx: 'conditions', cxN: 25 } });
  await run('the report tests fail', { opts: { run: true }, fake: { rp: 'staging' } });
  await run('the sensitivity tests fail', { opts: { run: true }, fake: { rp: 'sens' } });
  await run('the run without extras', { opts: { run: true, extras: false } });
  await run('NZTA counts fail', { netOpts: { fail: { Assets_SHTrafficMonitoringSites: 500 } } });
  await run('AT unreachable and crashes refused', { netOpts: { fail: { TrafficService: 'network', CAS_Data_Public: 503 } } });
  await run('the buildings do not download', { netOpts: { fail: { '"building"': 500 } } });
  await run('the Census layers, employees and zoning fail', { netOpts: { fail: { dwellings_by_SA1: 500, households_by_SA1: 'network', employees_by_statistical: 'unreadable', Unitary_Plan_Base_Zone: 503, 'catalogue.data.govt.nz': 500 } } });
  await run('every download except the roads fails', { netOpts: { fail: { FeatureServer: 503, 'catalogue.data.govt.nz': 500, '"building"': 500, '"landuse"': 500 } } });
  await run('the roads do not download', { netOpts: { fail: { 'overpass-api.de': 504, 'maps.mail.ru': 'network', 'overpass.private.coffee': 'unreadable', 'overpass.kumi.systems': 500 } } });
  await run('the roads download from the second server', { netOpts: { fail: { 'overpass-api.de': 504 } } });
  await run('too few roads', { netOpts: { roads: drury({ only: e => e.type === 'node' || (e.tags && e.tags.name === 'Waihoehoe Road') }) } });
  await run('no named roads for the sites and options', { netOpts: { roads: drury({ withFitzgerald: false, withSutton: false, withWaihoehoe: false }) } });
  await run('no Fitzgerald Road', { netOpts: { roads: drury({ withFitzgerald: false }) } });
  await run('cancelled while grouping buildings', { cancelOn: 'Grouping buildings' });
  await run('cancelled before the Census', { cancelOn: 'Downloading Census' });
  await run('cancelled before the assessment scenarios', { cancelOn: 'Setting up the assessment' });
  await run('cancelled in the first run', { opts: { run: true }, cancelOn: ' peak…' });
  await run('cancelled in the staging test', { opts: { run: true }, cancelOn: 'Staging test: the proposal' });
  await run('cancelled in the second staging step', { opts: { run: true }, cancelOn: 'Staging test: 50' });
  writeExpected('sample-build.json', jsonSafe(cases));
}

/* ------------------------------------------------------------------------------------------------------------------------------------
   Constants and the helpers on their own
   ------------------------------------------------------------------------------------------------------------------------------------ */
async function partHelpers() {
  const net = makeNet(), seedNo = 700, c = load({ seed: seedNo, fetchImpl: net.fetchImpl });
  const S = c.Sample, H = S.__h;
  writeExpected('sample-constants.json', jsonSafe({ BOUNDS: S.BOUNDS, ILLUSTRATIVE: S.ILLUSTRATIVE, LODGEMENT: S.LODGEMENT, JUSTIFY: S.JUSTIFY, COMMITTED: S.COMMITTED, PROPOSAL: S.PROPOSAL, TG: S.TG }));
  const out = { seed: seedNo, now: NOW, ...idStart(seedNo) };
  const roads = drury();
  writeFileSync(join(here, 'fixtures', 'data', 'sample-roads.json'), JSON.stringify(roads));
  const fresh = () => { const P = c.newProject('helpers'); c.Store.P = P; c.Importers.osmToScenario(P.scenarios[0], roads, { sites: true }); return P; };
  // clipWays on the whole network, on boxes that cut it in different ways, and on made-up elements
  const AREA = { s: -37.12475, w: 174.92312, n: -37.08125, e: 174.99737 };
  const boxes = [AREA, { s: -37.11, w: 174.94, n: -37.095, e: 174.975 }, { s: -37.2, w: 174.8, n: -37.0, e: 175.1 }, { s: -37.1, w: 174.96, n: -37.1, e: 174.96 }, { s: -38, w: 175, n: -37.9, e: 175.1 }];
  const nodes = [{ type: 'node', id: 1, lat: -37.1, lon: 174.95 }, { type: 'node', id: 2, lat: -37.1, lon: 174.96 }, { type: 'node', id: 3, lat: -37.1, lon: 174.97 }, { type: 'node', id: 4, lat: -37.1, lon: 174.98 }, { type: 'node', id: '5', lat: -37.11, lon: 174.96 }, { type: 'node', id: 6, lat: -37.09, lon: 174.96 }, { type: 'node', id: 7 }, { type: 'node', id: 8, lat: null, lon: null }];
  const odd = [...nodes,
    { type: 'way', id: 100, nodes: [1, 2, 3, 4], tags: { highway: 'primary' } },
    { type: 'way', id: 101, nodes: [2, 3], tags: { highway: 'primary' } },
    { type: 'way', id: 102, nodes: [1, 4, 2, 3, 1], tags: { highway: 'secondary' } },
    { type: 'way', id: 103, nodes: [9, 10] },
    { type: 'way', id: 104, nodes: [] }, { type: 'way', id: 105 }, { type: 'way', id: 106, nodes: [5, 2, 6], tags: { highway: 'tertiary' } },
    { type: 'way', id: 107, nodes: [7, 2, 8, 3], tags: {} }, { type: 'way', id: '108', nodes: ['5', 5, 2], tags: {} }, { type: 'relation', id: 9, members: [] }];
  out.clip = [];
  const more = [...boxes, { s: -37.105, w: 174.955, n: -37.095, e: 174.975 }];
  for (const b of more) for (const els of [roads.elements, odd]) { const r = H.clipWays(els, b); out.clip.push({ box: b, input: els === odd ? 'odd' : 'roads', out: jsonSafe(r) }); }
  out.compass = [0, 44.9, 45, 89.9, 90, 135, 180, 224.9, 225, 270, 315, 359.9, 360, 405, -10, -45, -46, -90, -360, 1e9, NaN].map(b => ({ b: Number.isNaN(b) ? 'NaN' : b, out: H.compass(b) }));
  {
    const P = fresh(), sc = P.scenarios[0];
    out.names = Object.keys(sc.nodes).map(id => ({ id, out: H.names(sc, id) }));
    out.nodeOn = [];
    for (const [ll, road, max] of [[[-37.0988, 174.9600], 'Takotowhenua'], [[-37.1006, 174.9722], 'Appleby'], [[-37.0936, 174.9552], 'Sutton'], [[-37.1035, 174.95], 'great south', 5], [[-37.1035, 174.95], 'Waihoehoe', 20], [[-37.1, 174.96], 'Nowhere'], [[-37.1, 174.96], ''], [[-37.0, 174.0], 'Sutton', 600], [[-37.1042, 174.9588], 'Fitzgerald', 1e9]]) out.nodeOn.push({ ll, road, max: max ?? null, out: H.nodeOn(sc, ll, road, max) ?? null });
    out.siteWith = [];
    for (const [pats, ll] of [[['Great South', 'Waihoehoe'], [-37.1035, 174.95]], [['Waihoehoe', 'Fitzgerald'], [-37.1042, 174.9588]], [['Waihoehoe'], [-37.1042, 174.9588]], [['Waihoehoe'], [-37.1035, 174.9500]], [['Nowhere'], [-37.1, 174.96]], [[], [-37.1, 174.96]], [['great'], [-37.2, 174.9]]]) { const r = H.siteWith(sc, pats, ll); out.siteWith.push({ pats, ll, out: r ? { site: r.s.id, c: r.c } : null }); }
    out.inputSites = Object.keys(sc.sites);
  }
  {
    const P = fresh(), sc = P.scenarios[0];
    const before = Object.values(sc.links).map(L => L.pts.length), n = H.simplify(sc, 2);
    out.simplify = { tol: 2, dropped: n, before, after: Object.values(sc.links).map(L => L.pts.length), pts: jsonSafe(Object.values(sc.links).slice(0, 6).map(L => L.pts)) };
    out.simplifyTol = [0.5, 5, 50].map(t => ({ tol: t, dropped: H.simplify(fresh().scenarios[0], t) }));
  }
  {
    const P = fresh(), sc = P.scenarios[0]; c.Store.P = P;
    const before = Object.keys(sc.links).length, n = H.pruneStubs(sc);
    out.prune = { before, removed: n, links: Object.keys(sc.links), nodes: Object.keys(sc.nodes), control: jsonSafe(Object.values(sc.nodes).map(x => x.control)) };
    const q = fresh(), sc2 = q.scenarios[0]; c.Store.P = q;
    const ends = [...c.Net.degree(sc2)].filter(([, d]) => d === 1).map(([id]) => id);
    sc2.zones.zKeep = { id: 'zKeep', kind: 'zone', node: ends[0], lu: { residential: 5 }, connectors: null, lat: 0, lng: 0 };
    sc2.routes.rKeep = { id: 'rKeep', name: 'Bus', wp: [ends[1]], freq: 4 };
    q.data = { counts: [{ id: 'user:1', src: 'user', name: 'GREAT SOUTH ROAD', lat: -37.0985, lng: 174.9505, am: 100, pm: 100 }] };
    const n2 = H.pruneStubs(sc2);
    out.pruneKept = { ends: ends.length, removed: n2, links: Object.keys(sc2.links) };
  }
  {
    const P = fresh(), sc = P.scenarios[0], r = H.optionA(sc);
    out.optionA = { made: r.map(L => (typeof L === 'string' ? L : L.id)), links: jsonSafe(Object.values(sc.links).filter(L => L.stage === 'proposal')), nodes: jsonSafe(Object.values(sc.nodes).filter(n => n.upgrade || n.control === 'roundabout').map(n => ({ id: n.id, control: n.control, upgrade: n.upgrade }))) };
    const w = fresh(), s2 = w.scenarios[0];
    out.optionB = { n: H.optionB(s2), links: jsonSafe(Object.values(s2.links).filter(L => L.stage === 'mitigation')) };
    const noRoads = c.newProject('empty');
    out.optionAEmpty = H.optionA(noRoads.scenarios[0]).length; out.optionBEmpty = H.optionB(noRoads.scenarios[0]);
  }
  out.after = { id: c.U.uid('t') };
  writeExpected('sample-helpers.json', jsonSafe(out));
}

const parts = process.argv.slice(2);
const want = n => parts.length === 0 || parts.includes(n);
if (want('helpers')) await partHelpers();
if (want('build')) await partBuild();
if (parts.length === 0) { const t = JSON.stringify(EXCHANGES); writeFileSync(EXCHANGE_FILE, t); console.log('fixtures/data/sample-exchanges.json', t.length, 'bytes'); }
