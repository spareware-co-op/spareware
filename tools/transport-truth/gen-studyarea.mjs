// Records the prototype's study area setup (studyarea.js: roads from OpenStreetMap, gateways, counts and crashes, existing land use, gateway flows from counts,
// the first model runs and the fit to counts) for the Rust port's studyarea module (projects/spare-construct/crates/construct-transport/src/studyarea,
// tests/studyarea_*.rs). Also records DataHub.loadAll (the count and crash downloads the setup, and the Drury sample, ask for).
//
// No network is used. The downloaded answers are made up here (deterministic): the Overpass roads of the made-up Papakura in fixtures/overpass-papakura.json,
// buildings and land-use areas of the made-up suburb in fixtures/landuse-papatoetoe.json moved to Papakura, NZTA and Auckland Transport count sites and NZTA CAS crash
// records in ArcGIS's answer format. The prototype runs with `fetch` stubbed; every request it makes is recorded with its answer (the answers are kept once, by request,
// in fixtures/data/studyarea-exchanges.json). Writes:
//   fixtures/data/studyarea-answers.json, studyarea-exchanges.json, studyarea-project-*.json
//   expected/studyarea-estimate.json   estimate() on many boxes
//   expected/studyarea-load.json       DataHub.loadAll for each source: requests, results, messages, failures, and what it leaves in the project
//   expected/studyarea-setup.json      setup() in many ways: the summary, progress messages, requests, and the project afterwards
// Run: node gen-studyarea.mjs (the same input always gives the same files). Dates: the prototype works out New Zealand dates and prints months in New Zealand time,
// so the script runs in that time zone.
process.env.TZ = 'Pacific/Auckland';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import vm from 'node:vm';
import { JS_DIR } from './js-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const NOW = Date.UTC(2026, 9, 21);
const CORE = ['util', 'model', 'registry', 'engine', 'data', 'landuse', 'turns', 'calibrate', 'sim', 'sample', 'studyarea'];
const inert = () => new Proxy(function () {}, { get: (t, k) => (k === Symbol.toPrimitive ? () => '' : inert()), apply: () => undefined });
mkdirSync(join(here, 'fixtures', 'data'), { recursive: true });
mkdirSync(join(here, 'expected'), { recursive: true });
const readJson = rel => JSON.parse(readFileSync(join(here, rel), 'utf8'));
const writeData = (name, data) => { const t = JSON.stringify(data); writeFileSync(join(here, 'fixtures', 'data', name), t); console.log('fixtures/data/' + name, t.length, 'bytes'); };
const writeExpected = (name, data) => { const t = JSON.stringify(data); writeFileSync(join(here, 'expected', name), t); console.log('expected/' + name, t.length, 'bytes'); };
export const jsonSafe = v => (v === undefined ? undefined : JSON.parse(JSON.stringify(v)));

// Leaflet's LatLngBounds (the parts the prototype uses), written from Leaflet's own source: extend, pad, contains, getCenter and the four edges
class LatLngBounds {
  constructor(a, b) {
    if (!a) return;
    const list = b ? [a, b] : a;
    for (const p of list) this.extend(p);
  }
  extend(p) {
    const ll = Array.isArray(p) ? { lat: p[0], lng: p[1] } : p;
    if (!this._sw && !this._ne) { this._sw = { lat: ll.lat, lng: ll.lng }; this._ne = { lat: ll.lat, lng: ll.lng }; }
    else { this._sw.lat = Math.min(ll.lat, this._sw.lat); this._sw.lng = Math.min(ll.lng, this._sw.lng); this._ne.lat = Math.max(ll.lat, this._ne.lat); this._ne.lng = Math.max(ll.lng, this._ne.lng); }
    return this;
  }
  pad(r) {
    const sw = this._sw, ne = this._ne, hb = Math.abs(sw.lat - ne.lat) * r, wb = Math.abs(sw.lng - ne.lng) * r;
    return new LatLngBounds({ lat: sw.lat - hb, lng: sw.lng - wb }, { lat: ne.lat + hb, lng: ne.lng + wb });
  }
  getCenter() { return { lat: (this._sw.lat + this._ne.lat) / 2, lng: (this._sw.lng + this._ne.lng) / 2 }; }
  getSouth() { return this._sw.lat; }
  getWest() { return this._sw.lng; }
  getNorth() { return this._ne.lat; }
  getEast() { return this._ne.lng; }
  contains(p) { const ll = Array.isArray(p) ? { lat: p[0], lng: p[1] } : p; return ll.lat >= this._sw.lat && ll.lat <= this._ne.lat && ll.lng >= this._sw.lng && ll.lng <= this._ne.lng; }
}
export const Leaflet = { latLngBounds: (a, b) => new LatLngBounds(a, b) };

// js-core.mjs's loader, with studyarea.js after the core and Leaflet's bounds; fetchImpl stands in for the network
export function load({ seed = 1, now = NOW, fetchImpl = null, files = CORE } = {}) {
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
    CSS: { escape: x => String(x) }, L: Leaflet,
  };
  ctx.window = ctx; ctx.globalThis = ctx; ctx.self = ctx;
  vm.createContext(ctx);
  const src = files.map(n => `/* ${n}.js */\n` + readFileSync(join(JS_DIR, n + '.js'), 'utf8')).join('\n;\n');
  vm.runInContext(src + `\n;globalThis.__core = { U, Store, Net, Sites, Scenarios, Importers, Engine, DataHub, Calibrate, Register, ExistingLU, StudyArea, Sample, newProject, newScenario, defaultSettings, migrate, CLASSES, LANDUSES };`, ctx, { filename: 'transport-studyarea.js' });
  return ctx.__core;
}

// How many numbers loading the scripts draws, and where the id counter stands (one probe id from a fresh load)
export function idStart(seed) {
  const c = load({ seed, now: NOW });
  const id = c.U.uid('t');
  let s = seed >>> 0;
  const draws = Array.from({ length: 12 }, () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; });
  const m = /^t(.{4})([0-9a-z]+?)([0-9a-z]{3})$/.exec(id);
  const skip = draws.findIndex(d => d.toString(36).slice(2, 5) === m[3]);
  if (skip < 0) throw new Error('the id does not come from the seeded generator: ' + id);
  return { rngSkip: skip, seq0: parseInt(m[2], 36) };
}

/* ------------------------------------------------------------------------------------------------------------------------------------
   Made-up downloaded answers around the made-up Papakura of fixtures/overpass-papakura.json
   ------------------------------------------------------------------------------------------------------------------------------------ */
let seed = 20261021;
const rnd = () => { seed = (seed + 0x6D2B79F5) >>> 0; let t = seed; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
const reseed = n => { seed = n >>> 0; };
const uni = (a, b) => a + (b - a) * rnd();
const pickOne = a => a[Math.floor(rnd() * a.length)];
export const STUDY = [[-37.0683, 174.935], [-37.0612, 174.9485]];       // the study box as [[south, west], [north, east]]
const roadsAnswer = readJson('fixtures/overpass-papakura.json');
const eastAnswer = readJson('fixtures/overpass-papakura-east.json');
const suburb = readJson('fixtures/landuse-papatoetoe.json');

// The suburb of fixtures/landuse-papatoetoe.json moved onto Papakura (its origin to the middle of the study box)
const SHIFT = { lat: -37.065 - -36.97, lon: 174.9415 - 174.86 };
const move = e => {
  const g = p => ({ ...p, lat: Math.round((p.lat + SHIFT.lat) * 1e7) / 1e7, lon: Math.round((p.lon + SHIFT.lon) * 1e7) / 1e7 });
  const o = { ...e };
  if (e.lat !== undefined) { o.lat = g(e).lat; o.lon = g(e).lon; }
  if (e.geometry) o.geometry = e.geometry.map(g);
  if (e.bounds) o.bounds = { minlat: e.bounds.minlat + SHIFT.lat, minlon: e.bounds.minlon + SHIFT.lon, maxlat: e.bounds.maxlat + SHIFT.lat, maxlon: e.bounds.maxlon + SHIFT.lon };
  if (e.members) o.members = e.members.map(m => ({ ...m, geometry: m.geometry && m.geometry.map(g) }));
  return o;
};
const buildings = suburb.elements.filter(e => e.tags && e.tags.building).map(move);
const areas = suburb.elements.filter(e => !(e.tags && e.tags.building)).map(move);

// NZTA state highway count sites, Auckland Transport counts and NZTA CAS crashes, as ArcGIS answers them (geometry x = longitude, y = latitude)
const pt = (lat, lng) => ({ x: lng, y: lat });
const NZTA = [
  { siteref: '01N00610', description: 'SH1 Southern Motorway north of Opaheke Road NB', lane: 'NB', sh: '1N', sitetype: 'Telemetry', type: 'Permanent', percentheavy: 9.1, aadt1yearago: 61200, aadt2yearsago: 59800, aadt3yearsago: 58100, aadt4yearsago: 57000, aadt5yearsago: 56200, accepteddays: 358, at: [-37.0676, 174.9378] },
  { siteref: '01S00611', description: 'SH1 Southern Motorway north of Opaheke Road SB', lane: 'SB', sh: '1S', sitetype: 'Telemetry', type: 'Permanent', percentheavy: 8.7, aadt1yearago: 60400, aadt2yearsago: 59100, aadt3yearsago: 57600, aadt4yearsago: 56800, aadt5yearsago: 55900, accepteddays: 361, at: [-37.0674, 174.9378] },
  { siteref: '01N00640', description: 'SH1 Opaheke Road off ramp', lane: 'NB', sh: '1N', sitetype: 'Short Term', type: 'Short Term', percentheavy: 4.2, aadt1yearago: 9800, aadt2yearsago: 9500, accepteddays: 12, at: [-37.0673, 174.9410] },
  { siteref: '22N00131', description: 'SH22 Great South Road to Drury EB', lane: 'EB', sh: '22', sitetype: 'Telemetry', type: 'Permanent', percentheavy: 11.4, aadt1yearago: 14300, aadt2yearsago: 14000, accepteddays: 340, at: [-37.0645, 174.9462] },
  { siteref: 'V0001', description: 'SH1 virtual site Papakura', lane: 'NB', sh: '1N', sitetype: 'Virtual', type: 'Virtual', percentheavy: 8, aadt1yearago: 58800, at: [-37.0660, 174.9370] },
  { siteref: '01N00699', description: 'SH1 No count recorded', lane: 'NB', sh: '1N', sitetype: 'Short Term', type: 'Short Term', aadt1yearago: null, aadt2yearsago: null, at: [-37.0676, 174.9440] },
  { siteref: '01N00700', description: 'SH1 far away', lane: 'NB', sh: '1N', sitetype: 'Short Term', type: 'Short Term', aadt1yearago: 40000, at: [-37.2, 175.1] },
];
const NZTA_FEATURES = NZTA.map((s, i) => ({ attributes: { OBJECTID: i + 1, ...Object.fromEntries(Object.entries(s).filter(([k]) => k !== 'at')) }, geometry: pt(...s.at) }));
const AT = [
  { road_name: 'GREAT SOUTH ROAD', start_name: 'WOOD STREET', end_name: 'BELL ROAD', count_date: Date.UTC(2025, 2, 11, 11), peak_hour: '745', count_duration: '7 days', adt: 18300, peaktraffic: 1450, pcheavy: 5.2, carr_way_no: 1042, at: [-37.0648, 174.9402] },
  { road_name: 'GREAT SOUTH ROAD', start_name: 'BELL ROAD', end_name: 'STATE HIGHWAY 22', count_date: Date.UTC(2024, 8, 24, 11), peak_hour: '1630', count_duration: '5 days', adt: 16100, peaktraffic: 1390, pcheavy: 4.8, carr_way_no: 1043, at: [-37.0647, 174.9448] },
  { road_name: 'BELL ROAD', start_name: 'GREAT SOUTH ROAD', end_name: 'RED HILL ROAD', count_date: Date.UTC(2025, 5, 3, 11), peak_hour: '800', count_duration: '7 days', adt: 7400, peaktraffic: 640, pcheavy: 3.9, carr_way_no: 1051, at: [-37.0660, 174.9443] },
  { road_name: 'OPAHEKE ROAD', start_name: 'GREAT SOUTH ROAD', end_name: 'SOUTHERN MOTORWAY', count_date: Date.UTC(2023, 10, 14, 11), peak_hour: '1700', count_duration: '7 days', adt: 12900, peaktraffic: 1100, pcheavy: 6.0, carr_way_no: 1060, at: [-37.0660, 174.9417] },
  { road_name: 'WOOD STREET', start_name: 'GREAT SOUTH ROAD', end_name: 'ELLERSLIE ROAD', count_date: null, peak_hour: null, count_duration: null, adt: 3200, peaktraffic: null, pcheavy: 2.1, carr_way_no: null, at: [-37.0630, 174.9389] },
  { road_name: 'SH 22', start_name: 'GREAT SOUTH ROAD', end_name: 'DRURY', count_date: Date.UTC(2025, 0, 20, 11), peak_hour: '730', count_duration: '7 days', adt: 14100, peaktraffic: 1300, pcheavy: 9.9, carr_way_no: 1070, at: [-37.0640, 174.9468] },
  { road_name: 'QUIET LANE', start_name: 'A', end_name: 'B', count_date: Date.UTC(2025, 3, 2, 11), peak_hour: '700', count_duration: '7 days', adt: 0, peaktraffic: 0, pcheavy: 0, carr_way_no: 1080, at: [-37.0650, 174.9440] },
];
const AT_FEATURES = AT.map((s, i) => ({ attributes: { OBJECTID: 5001 + i, latest: 'Yes', ...Object.fromEntries(Object.entries(s).filter(([k]) => k !== 'at')) }, geometry: pt(...s.at) })).concat([
  { attributes: { OBJECTID: 5900, latest: 'No', road_name: 'GREAT SOUTH ROAD', start_name: 'OLD', end_name: 'COUNT', count_date: Date.UTC(2019, 5, 1, 11), peak_hour: '745', count_duration: '7 days', adt: 15000, peaktraffic: 1200, pcheavy: 5, carr_way_no: 1042 }, geometry: pt(-37.0648, 174.9402) }]);
reseed(7700);
const CAS_FEATURES = Array.from({ length: 46 }, (_, i) => {
  const road = pickOne([['GREAT SOUTH ROAD', 'BELL ROAD', -37.0647, 174.9402], ['GREAT SOUTH ROAD', 'WOOD STREET', -37.0648, 174.9389], ['OPAHEKE ROAD', 'GREAT SOUTH ROAD', -37.0660, 174.9417], ['STATE HIGHWAY 1', 'OPAHEKE ROAD', -37.0676, 174.9402], ['BELL ROAD', null, -37.0662, 174.9443]]);
  const sev = pickOne(['Non-Injury Crash', 'Non-Injury Crash', 'Minor Crash', 'Minor Crash', 'Serious Crash', 'Fatal Crash']);
  const attrs = { OBJECTID: 9000 + i, crashSeverity: sev, crashYear: pickOne([2019, 2021, 2021, 2022, 2023, 2024, 2025, 2026]), crashLocation1: road[0], crashLocation2: road[1], speedLimit: pickOne([50, 50, 60, 100, null]),
    pedestrian: rnd() < 0.08 ? 1 : 0, bicycle: rnd() < 0.06 ? 1 : 0, motorcycle: rnd() < 0.07 ? 1 : 0, fatalCount: sev === 'Fatal Crash' ? 1 : 0, seriousInjuryCount: sev === 'Serious Crash' ? 1 : 0, minorInjuryCount: sev === 'Minor Crash' ? pickOne([1, 2]) : 0,
    light: pickOne(['Dark', 'Overcast', 'Bright sun', null]), weatherA: pickOne(['Fine', 'Light rain', 'Heavy rain', null]), trafficControl: pickOne(['Nil', 'Traffic Signals', 'Give way', null]), urban: pickOne(['Urban', 'Urban', 'Open', null]) };
  if (i === 3) { attrs.crashSeverity = null; attrs.crashLocation1 = null; attrs.crashLocation2 = ''; attrs.fatalCount = ''; attrs.light = ''; }
  return { attributes: attrs, geometry: pt(road[2] + uni(-0.0004, 0.0004), road[3] + uni(-0.0006, 0.0006)) };
}).concat([{ attributes: { OBJECTID: 9100, crashSeverity: 'Minor Crash', crashYear: 2025, crashLocation1: 'NO POSITION' }, geometry: undefined }, { attributes: { OBJECTID: 9101, crashSeverity: 'Minor Crash', crashYear: 2025, crashLocation1: 'FAR AWAY' }, geometry: pt(-37.3, 175.3) }]).map(f => (f.geometry === undefined ? { attributes: f.attributes } : f));
const ANSWERS = { study: STUDY, nzta: NZTA_FEATURES, at: AT_FEATURES, cas: CAS_FEATURES };
writeData('studyarea-answers.json', ANSWERS);

// The answers every request got, by request (kept once, shared by every case)
const EXCHANGE_FILE = join(here, 'fixtures', 'data', 'studyarea-exchanges.json');
export const EXCHANGES = (() => { try { return JSON.parse(readFileSync(EXCHANGE_FILE, 'utf8')); } catch { return {}; } })();
const exchangeKey = (method, url, body) => `${method} ${url}${body ? ' | ' + body : ''}`;
export const saveExchanges = () => { const t = JSON.stringify(EXCHANGES); writeFileSync(EXCHANGE_FILE, t); console.log('fixtures/data/studyarea-exchanges.json', t.length, 'bytes'); };

// A stubbed network. Overpass (POST): roads, buildings and land-use areas. ArcGIS (GET): NZTA sites, AT counts and CAS crashes by envelope and page.
// Every request is recorded in log. fail: requests whose address or body contains a key fail ('network', 'abort', 'unreadable', a status number or { json }).
export function makeNet({ pageSize = 4, fail = {}, roads = roadsAnswer, east = false, answers = {} } = {}) {
  const A = { ...ANSWERS, ...answers };
  const log = [], state = { east };
  const inEnvelope = (g, env) => g && g.x >= env[0] && g.x <= env[2] && g.y >= env[1] && g.y <= env[3];
  const fetchImpl = async (url, init) => {
    const method = init && init.method ? init.method : 'GET', body = init && init.body ? String(init.body) : undefined;
    const entry = { method, url: String(url) };
    if (body) entry.body = body;
    log.push(entry);
    const key = exchangeKey(method, entry.url, body);
    const answer = (status, a) => { entry.status = status; if (a !== undefined) { if (pageSize === 4 && !Object.keys(answers).length && !state.east) { EXCHANGES[key] = a; entry.ans = key; } else entry.answer = a; } return { ok: status >= 200 && status < 300, status, json: async () => JSON.parse(JSON.stringify(a)) }; };
    const f = Object.entries(fail).find(([pat]) => String(url).includes(pat) || (body && decodeURIComponent(body).includes(pat)));
    if (f) {
      const how = f[1];
      if (how === 'network') { entry.failure = 'network'; throw new TypeError('fetch failed'); }
      if (how === 'abort') { entry.failure = 'cancelled'; throw Object.assign(new Error('The operation was aborted'), { name: 'AbortError' }); }
      if (how === 'hang') { entry.failure = 'timeout'; return new Promise((res, rej) => { const sig = init && init.signal; const stop = () => rej(Object.assign(new Error('The operation was aborted'), { name: 'AbortError' })); if (sig) { if (sig.aborted) stop(); else sig.addEventListener('abort', stop, { once: true }); } }); }
      if (typeof how === 'number') return answer(how);
      if (how === 'unreadable') { entry.failure = 'unreadable'; entry.status = 200; return { ok: true, status: 200, json: async () => { throw new SyntaxError('Unexpected token < in JSON at position 0'); } }; }
      if (how && how.json !== undefined) { entry.status = 200; entry.answer = how.json; return { ok: true, status: 200, json: async () => JSON.parse(JSON.stringify(how.json)) }; }
    }
    if (method === 'POST') {
      const q = decodeURIComponent(String(body).replace(/^data=/, ''));
      if (q.includes('["highway"~')) return answer(200, state.east ? eastAnswer : roads);
      // the elements whose extent meets the tile asked for (the first box in the query)
      const m = /\(([-\d.]+),([-\d.]+),([-\d.]+),([-\d.]+)\)/.exec(q), [bs, bw, bn, be] = m ? m.slice(1).map(Number) : [-90, -180, 90, 180];
      const ext = e => { const g = e.geometry || (e.members || []).flatMap(x => x.geometry || []); if (e.lat !== undefined) return [e.lat, e.lon, e.lat, e.lon]; const la = g.map(p => p.lat), lo = g.map(p => p.lon); return [Math.min(...la), Math.min(...lo), Math.max(...la), Math.max(...lo)]; };
      const meets = e => { const [a, b, c2, d] = ext(e); return a <= bn && c2 >= bs && b <= be && d >= bw; };
      const all = q.includes('["building"]') ? buildings : areas;
      return answer(200, { version: 0.6, generator: 'Overpass API (made up)', elements: all.filter(meets) });
    }
    const u = new URL(String(url));
    const src = u.pathname.includes('Assets_SHTrafficMonitoringSites') ? 'nzta' : u.pathname.includes('TrafficService') ? 'at' : u.pathname.includes('CAS_Data_Public') ? 'cas' : null;
    if (!src) return answer(404);
    const [w, s, e, n] = u.searchParams.get('geometry').split(',').map(Number), offset = +u.searchParams.get('resultOffset') || 0, where = u.searchParams.get('where') || '1=1';
    let list = A[src].filter(x => !x.geometry || inEnvelope(x.geometry, [w, s, e, n]));
    if (src === 'at') list = list.filter(x => where.includes("latest='Yes'") ? x.attributes.latest === 'Yes' : true);
    if (src === 'cas') { const m = /crashYear>=(\d+)/.exec(where); if (m) list = list.filter(x => x.attributes.crashYear >= +m[1]); }
    const want = (u.searchParams.get('outFields') || '*').split(',');
    const page = list.slice(offset, offset + pageSize).map(x => ({ attributes: want[0] === '*' ? x.attributes : Object.fromEntries(Object.entries(x.attributes).filter(([k]) => want.includes(k))), ...(x.geometry ? { geometry: x.geometry } : {}) }));
    return answer(200, { objectIdFieldName: 'OBJECTID', features: page, ...(offset + pageSize < list.length ? { exceededTransferLimit: true } : {}) });
  };
  return { fetchImpl, log, state };
}


/* ------------------------------------------------------------------------------------------------------------------------------------
   Part 1: estimate
   ------------------------------------------------------------------------------------------------------------------------------------ */
async function partEstimate() {
  const c = load({ seed: 1 }), S = c.StudyArea, out = { constants: { WARN_KM2: S.WARN_KM2, MAX_KM2: S.MAX_KM2 }, cases: [] };
  const box = (s, w, n, e) => [[s, w], [n, e]];
  const around = (lat, lng, wKm, hKm) => { const ky = 110574, kx = 111320 * Math.cos(lat * Math.PI / 180); return box(lat - hKm * 500 / ky, lng - wKm * 500 / kx, lat + hKm * 500 / ky, lng + wKm * 500 / kx); };
  const cases = [STUDY, box(-37.07, 174.934, -37.0699, 174.9341), around(-36.85, 174.76, 0.7, 0.7), around(-36.85, 174.76, 3, 3), around(-36.85, 174.76, 5.9, 6), around(-36.85, 174.76, 6.1, 6), around(-36.85, 174.76, 10, 11.9), around(-36.85, 174.76, 10, 12.1), around(-36.85, 174.76, 40, 40),
    box(-37.07, 174.934, -37.07, 174.949), box(-37.07, 174.934, -37.06, 174.934), box(-37.06, 174.949, -37.07, 174.934), around(0, 0, 5, 5), around(-45.9, 170.5, 5, 5), around(70, 20, 5, 5), around(-36.85, 174.76, 0.05, 0.05), around(-36.85, 174.76, 1.5, 10)];
  for (const b of cases) { let r, error = null; try { r = S.estimate(b); } catch (e) { error = e.message; } out.cases.push({ bounds: b, out: jsonSafe(r) ?? null, error }); }
  writeExpected('studyarea-estimate.json', jsonSafe(out));
}

/* ------------------------------------------------------------------------------------------------------------------------------------
   Part 2: DataHub.loadAll (counts and crashes)
   ------------------------------------------------------------------------------------------------------------------------------------ */
const emptyProject = (c, scenarios = 1) => {
  const P = c.newProject('Study area test');
  P.scenarios[0].name = 'Base case';
  for (let i = 1; i < scenarios; i++) { const s = c.newScenario('Option ' + i); P.scenarios.push(s); }
  return P;
};
async function partLoad() {
  const cases = [], c0 = load({ seed: 1 });
  const run = async (name, { which = ['nzta', 'at', 'cas'], netOpts = {}, opts = {}, pre = null, bounds = STUDY, calls = 1 } = {}) => {
    const net = makeNet(netOpts), c = load({ seed: 1, fetchImpl: net.fetchImpl }), P = emptyProject(c);
    c.Store.P = P;
    if (pre) pre(P);
    const b = Leaflet.latLngBounds(bounds), status = [], res = [];
    for (let i = 0; i < calls; i++) {
      let msgs = null, error = null;
      const mark = net.log.length, smark = status.length;
      try { msgs = await c.DataHub.loadAll(b, which, t => status.push(t), opts); } catch (e) { error = e.message; }
      res.push({ msgs, error, requests: [mark, net.log.length], status: [smark, status.length] });
    }
    cases.push({ name, which, bounds, opts, fail: netOpts.fail || {}, pageSize: netOpts.pageSize ?? 4, pre: pre ? pre.label : null, log: net.log, status, res, data: jsonSafe(P.data) });
  };
  const withOff = P => { P.data = { counts: [{ id: 'nzta:01N00610', src: 'nzta', name: 'old', off: true, lat: 0, lng: 0 }, { id: 'at:5001', src: 'at', name: 'old', off: false }, { id: 'user:1', src: 'user', name: 'Mine', lat: -37.065, lng: 174.94, am: 100 }], loaded: { user: { at: 5, n: 1 } } }; };
  withOff.label = 'counts with switches already set';
  await run('all three sources');
  await run('NZTA only', { which: ['nzta'] });
  await run('AT only', { which: ['at'] });
  await run('crashes only', { which: ['cas'] });
  await run('ten years of crashes', { which: ['cas10'] });
  await run('five and ten years', { which: ['cas', 'cas10'], opts: { casYears: 5 } });
  await run('crash years option', { which: ['cas'], opts: { casYears: 10 } });
  await run('crash years option text', { which: ['cas'], opts: { casYears: '3' } });
  await run('counts reloaded keep their switches', { which: ['nzta', 'at'], pre: withOff, calls: 2 });
  await run('pages of two', { netOpts: { pageSize: 2 } });
  await run('everything on one page', { netOpts: { pageSize: 500 } });
  await run('a source that is not downloaded here', { which: ['nzta', 'zzz', 'cas'] });
  await run('NZTA fails', { netOpts: { fail: { Assets_SHTrafficMonitoringSites: 500 } } });
  await run('AT unreachable', { netOpts: { fail: { TrafficService: 'network' } } });
  await run('the user cancels', { netOpts: { fail: { CAS_Data_Public: 'abort' } } });
  await run('a source does not answer in time', { opts: { timeout: 1200 }, netOpts: { fail: { TrafficService: 'hang' } } });
  await run('crashes unreadable', { netOpts: { fail: { CAS_Data_Public: 'unreadable' } } });
  await run('a service error', { netOpts: { fail: { TrafficService: { json: { error: { code: 400, message: 'Invalid query' } } } } } });
  await run('a service error without a message', { netOpts: { fail: { TrafficService: { json: { error: { code: 400 } } } } } });
  await run('an answer that is not an object', { netOpts: { fail: { CAS_Data_Public: { json: 'oops' } } } });
  await run('every source fails', { netOpts: { fail: { FeatureServer: 503 } } });
  await run('a box with nothing in it', { bounds: [[-36.5, 174.0], [-36.49, 174.01]] });
  // Auckland Transport counts dated at the edges of every month, in and out of daylight saving time, so the month printed in each note is checked
  const atDates = [];
  for (let y = 2024; y <= 2026; y++) for (let m = 0; m < 12; m++) for (const [d, h] of [[1, 10], [1, 11], [1, 12], [1, 13], [15, 4], [28, 11], [28, 12]]) atDates.push(Date.UTC(y, m, d, h, 59));
  const atFeatures = atDates.map((ms, i) => ({ attributes: { OBJECTID: 8000 + i, latest: 'Yes', road_name: i % 3 ? "O'NEILL-WOOD ROAD 4TH" : 'GREAT SOUTH ROAD', start_name: "ST HELIER'S BAY", end_name: 'a b-c', count_date: ms, peak_hour: String(700 + (i % 60)), count_duration: i % 2 ? '7 days' : null, adt: 1000 + i, peaktraffic: 100 + i, pcheavy: 5, carr_way_no: i % 4 ? 2000 + i : null }, geometry: pt(-37.065 + (i % 7) * 0.0002, 174.940 + (i % 5) * 0.0002) }))
    .concat([{ attributes: { OBJECTID: 8999, latest: 'Yes', road_name: 'SH1 NORTH', adt: 5000, count_date: Date.UTC(2025, 1, 1) }, geometry: pt(-37.065, 174.94) }, { attributes: { OBJECTID: 8998, latest: 'Yes', road_name: 'SH 22', adt: 5000 }, geometry: pt(-37.065, 174.94) }, { attributes: { OBJECTID: 8997, latest: 'Yes', road_name: 'SHEFFIELD STREET', adt: 5000 }, geometry: pt(-37.065, 174.94) }]);
  {
    const net = makeNet({ pageSize: 1000, answers: { at: atFeatures } }), c = load({ seed: 1, fetchImpl: net.fetchImpl }), P = emptyProject(c);
    c.Store.P = P;
    const status = [];
    const msgs = await c.DataHub.loadAll(Leaflet.latLngBounds(STUDY), ['at'], t => status.push(t), {});
    cases.push({ name: 'Auckland Transport counts across the calendar', which: ['at'], bounds: STUDY, opts: {}, fail: {}, pageSize: 1000, atAnswers: true, log: net.log, status, res: [{ msgs, error: null, requests: [0, net.log.length], status: [0, status.length] }], data: { counts: P.data.counts.map(k => ({ id: k.id, ref: k.ref, name: k.name, date: k.date, note: k.note, peakTime: k.peakTime })) } });
  }
  // Auckland Transport counts with a date that is not a date
  {
    const net = makeNet({ pageSize: 1000, answers: { at: [{ attributes: { OBJECTID: 7000, latest: 'Yes', road_name: 'BAD ROAD', adt: 5, count_date: 'tomorrow' }, geometry: pt(-37.065, 174.94) }] } }), c = load({ seed: 1, fetchImpl: net.fetchImpl }), P = emptyProject(c);
    c.Store.P = P;
    const status = [];
    const msgs = await c.DataHub.loadAll(Leaflet.latLngBounds(STUDY), ['at'], t => status.push(t), {});
    cases.push({ name: 'Auckland Transport count with a bad date', which: ['at'], bounds: STUDY, opts: {}, fail: {}, pageSize: 1000, atAnswers: 'bad date', log: net.log, status, res: [{ msgs, error: null, requests: [0, net.log.length], status: [0, status.length] }], data: jsonSafe(P.data) });
  }
  // the crash window and New Zealand's dates, at the edges of daylight saving time and of the year
  const windows = [];
  const dstEdges = [];
  for (let y = 2019; y <= 2034; y++) {
    for (const [m, d] of [[8, 24], [8, 25], [8, 26], [8, 27], [8, 28], [8, 29], [8, 30], [3, 1], [3, 2], [3, 3], [3, 4], [3, 5], [3, 6], [3, 7]]) for (const h of [0, 1, 11, 12, 13, 14, 15, 16]) dstEdges.push(Date.UTC(y, m, d, h, 0));
    for (const [m, d, h] of [[11, 31, 10], [11, 31, 11], [11, 31, 12], [11, 31, 13], [0, 1, 0], [2, 31, 10], [2, 31, 11], [2, 31, 12], [2, 31, 13], [2, 31, 14], [3, 1, 11], [3, 1, 12], [3, 1, 13], [5, 15, 3], [6, 20, 5]]) dstEdges.push(Date.UTC(y, m, d, h, 0));
  }
  for (const ms of dstEdges) for (const years of (new Date(ms).getUTCFullYear() === 2026 ? [undefined, 10, 3] : [undefined])) windows.push({ now: ms, years: years ?? null, out: jsonSafe(c0.DataHub.casWindow(new Date(ms), years)) });
  writeExpected('studyarea-load.json', jsonSafe({ loadAll: cases, casWindow: windows }));
}

/* ------------------------------------------------------------------------------------------------------------------------------------
   Part 3: setup
   ------------------------------------------------------------------------------------------------------------------------------------ */
const resSummary = P => jsonSafe(P.scenarios.map(s => ({ id: s.id, res: Object.fromEntries(Object.entries(s.res || {}).map(([k, r]) => [k, r && { period: r.period, it: r.it, gap: r.gap, converged: r.converged, stale: r.stale, kpi: r.kpi }])) })));
const stateOf = P => { const o = jsonSafe(P); for (const sc of o.scenarios) delete sc.res; return { scenarios: o.scenarios, data: o.data ?? null, me: o.me ?? null, active: o.active }; };

// Things done to the project between two runs of one case, by name (the Rust test does the same)
const HOOKS = {
  // a zone of the user's own with the name of a built zone, a few metres away, a zone made by an old setup, and matrix factors for a zone the next run replaces
  dupUserZone(c) {
    const P = c.Store.P, sc = P.scenarios[0], z0 = Object.values(sc.zones).find(z => z.existing === true && z.src === 'existing-lu' && z.kind === 'zone');
    sc.zones.zUserDup = { id: 'zUserDup', kind: 'zone', name: '  ' + z0.name.toUpperCase() + ' ', lat: z0.lat + 0.0004, lng: z0.lng, lu: { residential: 5 }, connectors: null };
    sc.zones.zUserFar = { id: 'zUserFar', kind: 'zone', name: z0.name, lat: z0.lat + 0.01, lng: z0.lng, lu: { residential: 5 }, connectors: null };
    sc.zones.zOld = { id: 'zOld', kind: 'zone', existing: true, name: 'Existing · Old Road', lat: z0.lat - 0.0003, lng: z0.lng, lu: { residential: 7 }, connectors: null };
    sc.zones.zOutside = { id: 'zOutside', kind: 'zone', existing: true, src: 'existing-lu', name: 'Outside zone', lat: -36.5, lng: 174.5, lu: { residential: 7 }, connectors: null };
    P.me = { AM: { [z0.id + '>gateway']: 1.1 }, info: { AM: { baseYear: 2026 } } };
  },
  meUnrelated(c) { c.Store.P.me = { AM: { 'zzz>yyy': 1.1 } }; },
};

async function partSetup() {
  const cases = [];
  const PROJECTS = { 'studyarea-project-one': jsonSafe(emptyProject(load({ seed: 3 }))), 'studyarea-project-two': jsonSafe(emptyProject(load({ seed: 3 }), 2)) };
  for (const [k, v] of Object.entries(PROJECTS)) writeData(k + '.json', v);
  const run = async (name, { project = 'studyarea-project-one', bounds = STUDY, steps = [{ opts: {} }], netOpts = {}, active = 0 } = {}) => {
    const seedNo = 400 + cases.length, net = makeNet(netOpts), c = load({ seed: seedNo, fetchImpl: net.fetchImpl });
    c.Store.P = JSON.parse(JSON.stringify(PROJECTS[project]));
    const P = c.Store.P;
    P.active = P.scenarios[active].id;
    const b = Leaflet.latLngBounds(bounds), status = [], out = [];
    for (const st of steps) {
      if (st.before) HOOKS[st.before](c);
      if (st.east !== undefined) net.state.east = st.east;
      let r = null, error = null;
      const mark = net.log.length, smark = status.length;
      try { r = await c.StudyArea.setup(b, st.opts, (t, f) => status.push([t, f])); } catch (e) { error = e.message; }
      // the fitted results are the scenario's own (compared through the project); the fit's other answers are kept
      if (r && r.calibration) for (const k of Object.keys(r.calibration)) { r.calibration[k] = { ...r.calibration[k], hasRes: !!r.calibration[k].res }; delete r.calibration[k].res; }
      out.push({ ...st, out: jsonSafe(r), error, requests: [mark, net.log.length], status: [smark, status.length], res: resSummary(P) });
    }
    cases.push({ name, project, active, bounds, seed: seedNo, now: NOW, ...idStart(seedNo), steps: out, fail: netOpts.fail || {}, log: net.log, status, after: stateOf(P) });
  };
  // Counts that sit close to the modelled flows (found by a first setup without counts), so the fit to counts can improve them
  const tuned = await (async () => {
    const net = makeNet({}), c = load({ seed: 2, fetchImpl: net.fetchImpl });
    c.Store.P = JSON.parse(JSON.stringify(PROJECTS['studyarea-project-one']));
    await c.StudyArea.setup(Leaflet.latLngBounds(STUDY), { counts: false, crashes: false }, () => {});
    const sc = c.Store.sc, res = sc.res.AM, list = [];
    const factors = [0.55, 0.7, 1.45, 1.6, 0.75, 1.3, 0.62, 1.38, 0.8, 1.5];
    for (const L of Object.values(sc.links)) {
      if (!L.name || !['arterial', 'collector', 'motorway'].includes(L.cls)) continue;
      const v = ((res.arcs[L.id + ':ab'] || {}).v || 0) + ((res.arcs[L.id + ':ba'] || {}).v || 0);
      if (v < 120 || list.some(x => x.road_name === L.name.toUpperCase())) continue;
      const g = c.Net.geom(sc, L), a = g[0], b = g[g.length - 1], f = factors[list.length % factors.length];
      list.push({ attributes: { OBJECTID: 6000 + list.length, latest: 'Yes', road_name: L.name.toUpperCase(), start_name: 'A', end_name: 'B', count_date: Date.UTC(2025, 2, 11, 11), peak_hour: null, count_duration: '7 days', adt: Math.round(v / 0.085 * f / 10) * 10, peaktraffic: null, pcheavy: 5, carr_way_no: 6000 + list.length }, geometry: pt((a[0] + b[0]) / 2, (a[1] + b[1]) / 2) });
    }
    return list.slice(0, 10);
  })();
  await run('everything, then again', { steps: [{ opts: {} }, { opts: {} }] });
  await run('fitted to counts that fit', { netOpts: { answers: { at: tuned, nzta: [] } }, steps: [{ opts: { fitCounts: true } }] });
  await run('fitted to counts that fit, not run', { netOpts: { answers: { at: tuned, nzta: [] } }, steps: [{ opts: { fitCounts: true, run: false } }] });
  await run('two scenarios', { project: 'studyarea-project-two', steps: [{ opts: {} }] });
  await run('two scenarios, the second active', { project: 'studyarea-project-two', active: 1, steps: [{ opts: {} }, { opts: { all: false } }] });
  await run('one of two scenarios', { project: 'studyarea-project-two', steps: [{ opts: { all: false } }] });
  await run('replaced zones, duplicates and matrix factors', { steps: [{ opts: {} }, { opts: {}, before: 'dupUserZone' }, { opts: {}, before: 'meUnrelated' }] });
  await run('the east roads on top of the first', { steps: [{ opts: { existing: false, run: false } }, { opts: { existing: false, run: false }, east: true }] });
  await run('no roads asked for', { steps: [{ opts: { roads: false } }] });
  await run('no gateways', { steps: [{ opts: { gateways: false } }] });
  await run('no counts, crashes only', { steps: [{ opts: { counts: false } }] });
  await run('counts only', { steps: [{ opts: { crashes: false } }] });
  await run('no counts, no crashes', { steps: [{ opts: { counts: false, crashes: false } }] });
  await run('no existing land use', { steps: [{ opts: { existing: false } }] });
  await run('not run', { steps: [{ opts: { run: false } }] });
  await run('no local roads', { steps: [{ opts: { local: false } }] });
  await run('with service lanes', { steps: [{ opts: { service: true } }] });
  await run('fitted to counts', { steps: [{ opts: { fitCounts: true } }] });
  await run('fitted to counts, not run', { steps: [{ opts: { fitCounts: true, run: false } }] });
  await run('fitted to counts without counts', { steps: [{ opts: { fitCounts: true, counts: false } }] });
  await run('the roads do not download', { netOpts: { fail: { 'overpass-api.de': 504, 'maps.mail.ru': 'network', 'overpass.private.coffee': 'unreadable', 'overpass.kumi.systems': 500 } }, steps: [{ opts: {} }] });
  await run('the roads download from the second server', { netOpts: { fail: { 'overpass-api.de': 504 } }, steps: [{ opts: {} }] });
  await run('the buildings do not download', { netOpts: { fail: { '"building"': 500 } }, steps: [{ opts: {} }] });
  await run('NZTA counts fail', { netOpts: { fail: { Assets_SHTrafficMonitoringSites: 500 } }, steps: [{ opts: {} }] });
  await run('every count and crash source fails', { netOpts: { fail: { FeatureServer: 503 } }, steps: [{ opts: {} }] });
  await run('a large area', { bounds: [[-37.1, 174.9], [-37.02, 174.98]], steps: [{ opts: { run: false } }] });
  await run('an area too large', { bounds: [[-37.3, 174.7], [-36.8, 175.2]], steps: [{ opts: {} }] });
  await run('an area with no size', { bounds: [[-37.0683, 174.935], [-37.0683, 174.9485]], steps: [{ opts: {} }] });
  writeExpected('studyarea-setup.json', jsonSafe(cases));
}

const parts = process.argv.slice(2);
const want = n => parts.length === 0 || parts.includes(n);
if (want('estimate')) await partEstimate();
if (want('load')) await partLoad();
if (want('setup')) await partSetup();
saveExchanges();
