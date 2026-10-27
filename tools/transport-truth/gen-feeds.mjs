// Records the prototype's official data feeds (features/feeds.js) and the data hub's count and live-traffic loaders (data.js) for the
// Rust port's feeds module (projects/spare-construct/crates/construct-transport/src/feeds, tests/feeds_*.rs).
//
// Nothing is downloaded. Every answer is made up in the shape the real service gives (ArcGIS feature queries, an Overpass answer, the
// Mobility Database list, GTFS zip files, a Nominatim-style answer is in gen-search.mjs) around the two test projects, and written to
// fixtures/data/. The prototype loads them through a stand-in for fetch that answers like the server (where clause, envelope, order by,
// pages), and every request it makes is recorded with a description of the answer it got, so the Rust test can replay the same run:
// it sends nothing, and answers each request the recorded way. Everything is deterministic (seeded); run twice, same files.
//
// Sections (node gen-feeds.mjs <section>; no argument runs all): time, regex, consts, loaders, nslr, control, bans, tms, ags, gtfs,
// council, hazards, fleet, live, vintage. Each writes expected/feeds-<section>.json.
process.env.TZ = 'Pacific/Auckland';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import vm from 'node:vm';
import zlib from 'node:zlib';
import { JS_DIR } from './js-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const CORE = ['util', 'model', 'registry', 'engine', 'data', 'landuse', 'turns', 'calibrate', 'sim', 'sample', 'kit', 'features/feeds'];
const NOW = Date.UTC(2026, 9, 21);
const inert = () => new Proxy(function () {}, { get: (t, k) => (k === Symbol.toPrimitive ? () => '' : inert()), apply: () => undefined });

// js-core.mjs's loader with kit.js and features/feeds.js after the core, the clock and fetch as arguments, and a Worker stand-in that
// records what the GTFS reader is given and fails, so the reader runs inline (feeds.js falls back to that when a worker fails)
function loadCtx({ seed = 1, now = NOW, fetchImpl = null, workerLog = null } = {}) {
  let s = seed >>> 0;
  const rand = () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  const M = Object.create(Math); M.random = rand;
  const store = new Map();
  class FakeWorker { constructor() { this.onmessage = null; this.onerror = null; } postMessage(m) { if (workerLog) workerLog.push(JSON.parse(JSON.stringify(m.o))); setTimeout(() => this.onerror && this.onerror({}), 0); } terminate() {} }
  const ctx = {
    console, JSON, Promise, Map, Set, WeakMap, WeakSet, Array, Object, Number, String, Boolean, Symbol, Error, TypeError, RangeError,
    Float64Array, Float32Array, Int32Array, Uint32Array, Uint8Array, Int8Array, Uint16Array, ArrayBuffer, DataView, Intl, RegExp, Blob, DecompressionStream,
    Worker: FakeWorker,
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
  vm.runInContext(src + `\n;globalThis.__core = { U, Store, Net, Sites, Engine, DataHub, Feeds, Kit, Landuse: typeof Landuse !== 'undefined' ? Landuse : null, newProject, newScenario, defaultSettings, migrate, CLASSES, LANDUSES };`, ctx, { filename: 'transport-feeds.js' });
  ctx.__core.ctx = ctx;
  return ctx.__core;
}

// Typed arrays as plain arrays, Maps and Sets as objects and arrays, non-finite numbers as strings (as the other generators); undefined dropped
const plain = v => {
  if (ArrayBuffer.isView(v)) return Array.from(v, plain);
  if (v instanceof Map) return Object.fromEntries([...v].map(([k, x]) => [String(k), plain(x)]));
  if (v instanceof Set) return [...v].map(plain);
  if (typeof v === 'number') return Number.isFinite(v) ? v : String(v);
  if (Array.isArray(v)) return v.map(plain);
  if (v && typeof v === 'object') { const o = {}; for (const [k, x] of Object.entries(v)) if (typeof x !== 'function' && x !== undefined) o[k] = plain(x); return o; }
  return v;
};
// What a save writes: undefined and non-finite numbers are left out or null
const saved = v => (v === undefined ? null : JSON.parse(JSON.stringify(v)));
const clone = o => JSON.parse(JSON.stringify(o));
const write = (rel, data) => { const t = JSON.stringify(data); writeFileSync(join(here, rel), t); console.log(rel, t.length, 'bytes'); };
const dataDir = join(here, 'fixtures', 'data');
mkdirSync(dataDir, { recursive: true });
const writeFixture = (name, data) => writeFileSync(join(dataDir, name), typeof data === 'string' || Buffer.isBuffer(data) ? data : JSON.stringify(data));
const only = process.argv[2];
const wants = k => !only || only === k;

let seed = 20261021;
const rnd = () => { seed = (seed + 0x6D2B79F5) >>> 0; let t = seed; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
const uni = (a, b) => a + (b - a) * rnd();
const pick = a => a[Math.floor(rnd() * a.length)];
const r7 = x => Math.round(x * 1e7) / 1e7;

/* ---------------- projects ---------------- */
const NAMES = ['corridor', 'grid'];
function open(name, o = {}) {
  const c = loadCtx({ seed: 11, ...o });
  const P = c.migrate(JSON.parse(readFileSync(join(here, 'inputs', name + '.json'), 'utf8')));
  c.Store.P = P;
  return { c, P, sc: P.scenarios[0] };
}
function reopen(c, text) { c.Store.P = c.migrate(JSON.parse(text)); return c.Store.P; }
const boundsOf = (sc, pad = 0.05) => {
  const ns = Object.values(sc.nodes), s = Math.min(...ns.map(n => n.lat)), n = Math.max(...ns.map(n => n.lat)), w = Math.min(...ns.map(n => n.lng)), e = Math.max(...ns.map(n => n.lng));
  const h = (n - s) * pad, wd = (e - w) * pad, b = { south: s - h, west: w - wd, north: n + h, east: e + wd };
  return { ...b, getSouth: () => b.south, getWest: () => b.west, getNorth: () => b.north, getEast: () => b.east };
};
const boxOf = b => ({ south: b.south, west: b.west, north: b.north, east: b.east });

/* ---------------- a server that answers like the services, and records the run ---------------- */
// route(test, answer): test(url, rec) → boolean; answer(url, rec, opts) → { desc, response }. The log keeps every request in order with the
// description of its answer: { kind: 'json', file } (a whole saved file), { kind: 'features', file, idx, exceeded } (the features at these
// positions of a saved file's list), { kind: 'literal', value }, { kind: 'text', file }, { kind: 'bytes', file, lastModified },
// { kind: 'head', lastModified } or { kind: 'fail', failure: 'http' | 'unreachable' | 'unreadable' | 'timeout' | 'timeout-reading', ... }.
function makeServer() {
  const log = [], routes = [];
  const ok = (body, extra = {}) => ({ ok: true, status: 200, headers: { get: n => (extra.lastModified && /last-modified/i.test(n) ? extra.lastModified : null) }, json: async () => JSON.parse(JSON.stringify(body)), text: async () => extra.text ?? JSON.stringify(body), arrayBuffer: async () => extra.buf });
  const fetchImpl = async (url, opts = {}) => {
    const rec = { method: opts.method || 'GET', url: String(url), ...(opts.body != null ? { body: opts.body } : {}), ...(opts.headers ? { headers: opts.headers } : {}) };
    for (const r of routes) {
      if (!r.test(String(url), rec)) continue;
      let a;
      try { a = await r.answer(String(url), rec, opts); } catch (e) { if (e.desc) { rec.answer = e.desc; log.push(rec); } throw e; }
      rec.answer = a.desc; log.push(rec);
      return a.response;
    }
    rec.answer = { kind: 'fail', failure: 'http', status: 404 }; log.push(rec);
    return { ok: false, status: 404, headers: { get: () => null }, json: async () => ({}), text: async () => '' };
  };
  const server = { log, routes, fetchImpl, ok, route: (test, answer) => { routes.push({ test, answer }); return server; } };
  return server;
}
const aborting = (opts, what) => new Promise((_, rej) => { const e = Object.assign(new Error('aborted'), { name: 'AbortError' }); if (opts.signal) { if (opts.signal.aborted) rej(e); else opts.signal.addEventListener('abort', () => rej(e), { once: true }); } });
const failing = (srv, desc) => {
  const d = { kind: 'fail', ...desc };
  return async (url, rec, opts) => {
    if (desc.failure === 'http') return { desc: d, response: { ok: false, status: desc.status, headers: { get: () => null }, json: async () => ({}), text: async () => '' } };
    if (desc.failure === 'unreachable') throw Object.assign(new TypeError(desc.message), { desc: d });
    if (desc.failure === 'unreadable') return { desc: d, response: { ok: true, status: 200, headers: { get: () => null }, json: async () => { throw new SyntaxError(desc.message); }, text: async () => { throw new SyntaxError(desc.message); }, arrayBuffer: async () => { throw new SyntaxError(desc.message); } } };
    if (desc.failure === 'timeout' || desc.failure === 'cancel') { try { await aborting(opts); } catch (e) { e.desc = d; throw e; } }
    if (desc.failure === 'timeout-reading') return { desc: d, response: { ok: true, status: 200, headers: { get: () => null }, json: () => aborting(opts), text: () => aborting(opts), arrayBuffer: () => aborting(opts) } };
    throw new Error('unknown failure');
  };
};
// An ArcGIS feature layer over a saved list: where is applied by the filter, the envelope to the features' boxes, then order and paging
function layerRoute(srv, { match, file, features, page = 120, filter = null, order = null, fieldsFilter = false, inline = false, box = f => { const g = f.geometry; if (!g) return null; if (g.x != null) return [g.x, g.y, g.x, g.y]; const pts = (g.paths || g.rings || []).flat(); if (!pts.length) return null; return [Math.min(...pts.map(p => p[0])), Math.min(...pts.map(p => p[1])), Math.max(...pts.map(p => p[0])), Math.max(...pts.map(p => p[1]))]; }, envelope = true, noBox = false }) {
  srv.route((url, rec) => match(url, rec), async (url, rec, opts) => {
    const u = new URL(url), p = u.searchParams;
    let idx = features.map((_, i) => i);
    if (filter) idx = idx.filter(i => filter(features[i], p, i));
    const [x0, y0, x1, y1] = (p.get('geometry') || '').split(',').map(Number);
    if (envelope && p.get('geometry') && !noBox) idx = idx.filter(i => { const b = box(features[i]); return b ? !(b[2] < x0 || b[0] > x1 || b[3] < y0 || b[1] > y1) : false; });
    if (order) idx.sort((a, b) => order(features[a], features[b]));
    const off = +p.get('resultOffset') || 0, rc = +p.get('resultRecordCount') || page, pg = Math.min(page, rc), part = idx.slice(off, off + pg), more = off + pg < idx.length;
    // the server sends only the fields asked for (outFields), when the layer is set to
    const of = p.get('outFields') || '*', want = fieldsFilter && of !== '*' ? of.split(',').map(s => s.toLowerCase()) : null;
    const shape = f => (want ? { ...f, attributes: Object.fromEntries(Object.entries(f.attributes || {}).filter(([k]) => want.includes(k.toLowerCase()))) } : f);
    return { desc: { kind: 'features', file, idx: part, exceeded: more, ...(want ? { fields: want } : {}), ...(inline ? { inline: part.map(i => features[i]) } : {}) }, response: srv.ok({ features: part.map(i => shape(features[i])), ...(more ? { exceededTransferLimit: true } : {}) }) };
  });
}
const jsonRoute = (srv, match, file, body) => srv.route(match, async () => ({ desc: { kind: 'json', file }, response: srv.ok(body) }));
const literalRoute = (srv, match, value) => srv.route(match, async () => ({ desc: { kind: 'literal', value }, response: srv.ok(value) }));
const failRoute = (srv, match, desc) => srv.route(match, failing(srv, desc));
const layerPath = frag => url => url.includes(frag) && url.includes('/query?');
const metaPath = frag => url => url.includes(frag) && url.endsWith('?f=json') && !url.includes('/query?');

// a layer description (?f=json) with these field names
const metaOf = (name, fields) => ({ name, fields: fields.map(f => (typeof f === 'string' ? { name: f, type: f === 'OBJECTID' ? 'esriFieldTypeOID' : 'esriFieldTypeString', alias: f } : f)) });

/* ====================================================================================================
   time: New Zealand time and dates
   ==================================================================================================== */
async function timeCases() {
  const out = {};
  // NZ offset (hours ahead of UTC): every transition from 1996 to 2040 (found by walking days and then halving), and the offset at 00:00 and 12:00 UTC of every day
  const DTF = new Intl.DateTimeFormat('en-US', { timeZone: 'Pacific/Auckland', year: 'numeric', month: 'numeric', day: 'numeric', hour: 'numeric', minute: 'numeric', hourCycle: 'h23' });
  const offsetAt = t => { const p = Object.fromEntries(DTF.formatToParts(new Date(t)).map(x => [x.type, x.value])); return (Date.UTC(+p.year, +p.month - 1, +p.day, +p.hour, +p.minute) - t) / 3600000; };
  const t0 = Date.UTC(1996, 0, 1), t1 = Date.UTC(2041, 0, 1), DAY = 86400000;
  const transitions = [];
  let prev = offsetAt(t0), lo = t0;
  for (let t = t0 + DAY; t <= t1; t += DAY) {
    const o = offsetAt(t);
    if (o !== prev) {
      let a = lo, b = t;
      while (b - a > 60000) { const m = Math.floor((a + b) / 2 / 60000) * 60000; if (offsetAt(m) === prev) a = m; else b = m; }
      transitions.push({ at: b, before: prev, after: o });
      prev = o;
    }
    lo = t;
  }
  out.transitions = transitions;
  let s = '';
  for (let t = t0; t < t1; t += DAY) s += (offsetAt(t) === 13 ? 'S' : 'W') + (offsetAt(t + DAY / 2) === 13 ? 'S' : 'W');
  out.dayOffsets = { from: t0, days: (t1 - t0) / DAY, text: s };
  // the New Zealand date (the en-CA text the prototype reads) and its year, around every change and at random instants
  const instants = [];
  for (const tr of transitions.filter(x => x.at > Date.UTC(2020, 0, 1) && x.at < Date.UTC(2032, 0, 1))) for (const d of [-3 * 3600000, -1, 0, 1, 3 * 3600000, 12 * 3600000]) instants.push(tr.at + d);
  for (let y = 2018; y <= 2032; y++) for (const [m, d] of [[0, 1], [11, 31], [5, 30], [2, 31], [3, 1], [8, 26], [8, 27]]) for (const h of [0, 10, 11, 12, 13, 23]) instants.push(Date.UTC(y, m, d, h, 59, 59));
  for (let k = 0; k < 200; k++) instants.push(Math.floor(uni(Date.UTC(1999, 0, 1), Date.UTC(2038, 0, 1))));
  const CA = new Intl.DateTimeFormat('en-CA', { timeZone: 'Pacific/Auckland', year: 'numeric', month: '2-digit', day: '2-digit' });
  out.today = instants.map(t => ({ at: t, date: CA.format(new Date(t)) }));
  // local calendar: the start of the year in New Zealand (the part of crashYears that reads the local year)
  out.localYears = instants.slice(0, 300).map(t => { const d = new Date(t); return { at: t, year: d.getFullYear(), start: new Date(d.getFullYear(), 0, 1).getTime(), next: new Date(d.getFullYear() + 1, 0, 1).getTime() }; });
  // dateText through the vintage table: the date as the table prints it
  const c = open('corridor').c, F = c.Feeds;
  const ats = ['2026-10-21T00:00:00.000Z', '2026-10-09T11:00:00.000Z', '2026-09-30T12:30:00Z', '2026-09-25T11:59:59.999Z', 1792540800000, 1790000000000, 0, '', null, undefined, 'not a date', '2026-10-21', '2026-10', '2026', '2026-10-21T08:15', '2026-10-21 08:15:30', '2026-10-21T08:15:30+13:00', '2026-10-21T08:15:30-05:00',
    'Wed, 21 Oct 2026 07:28:00 GMT', '21 Oct 2026 07:28:00 GMT', 'Wed, 9 Sep 2026 23:59:59 GMT', 'Mon, 05 Jan 2026 00:00:00 +1300', 'Wed, 21 Oct 2026 07:28 UTC', 1e20, -1e12, '2026-13-45', '2026-02-30', true, 12345678, '1999-12-31T23:59:59.999Z', '2008-04-05T13:59:59Z', '2008-04-05T14:00:00Z', '2026-12-31T11:00:00Z', '2026-12-31T10:59:59Z'];
  out.dateText = [];
  for (const at of ats) {
    F.F().vintage = { x: { source: 'S', url: 'https://a.example/x', at, n: 1, licence: 'L' } };
    const row = F.vintageRows()[0], un = s => String(s).replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, '&');
    out.dateText.push({ at: at === undefined ? { undefined: true } : at, text: un(row[2]) });
  }
  // Date.parse of the forms the port reads
  const forms = ['2026', '2026-10', '2026-10-21', '2026-10-21T08:15', '2026-10-21T08:15:30', '2026-10-21T08:15:30.5', '2026-10-21T08:15:30.123Z', '2026-10-21T08:15:30+13:00', '2026-10-21T08:15:30-05:30', '2026-10-21 08:15', '2026-03-29T02:30:00', '2026-09-27T02:30:00', '2026-04-05T02:30:00', '2026-04-05T03:30:00', '2026-12-25T00:00:00',
    'Wed, 21 Oct 2026 07:28:00 GMT', '21 Oct 2026 07:28:00 GMT', 'Thu, 22 Oct 2026 00:00:00 +1300', 'Wed, 21 Oct 2026 07:28 UTC', 'Wed, 21 Oct 2026', '2026-13-01', '2026-00-10', '2026-10-32', '2026-10-21T25:00', '2026-10-21T24:00:00', '2026-10-21T24:00:01', '2026-10-21T08:60', '', 'abc'];
  out.parse = forms.map(f => { const v = Date.parse(f); return { text: f, ms: Number.isNaN(v) ? null : v }; });
  // en-NZ day, month and year (September is "Sept")
  const dm = new Intl.DateTimeFormat('en-NZ', { day: 'numeric', month: 'short', year: 'numeric' }), my = new Intl.DateTimeFormat('en-NZ', { month: 'short', year: 'numeric' });
  out.local = []; for (let m = 0; m < 12; m++) for (const d of [1, 15, 31]) { const t = Date.UTC(2026, m, d, 5); out.local.push({ at: t, dayMonthYear: new Date(t).toLocaleDateString('en-NZ', { day: 'numeric', month: 'short', year: 'numeric' }), monthYear: new Date(t).toLocaleDateString('en-NZ', { month: 'short', year: 'numeric' }) }); }
  void dm; void my;
  return out;
}

/* ====================================================================================================
   regex: the matcher against JavaScript's own regular expressions
   ==================================================================================================== */
const PATTERNS = [
  ['^adt$', 'i'], ['aadt', 'i'], ['average.*daily', 'i'], ['^vpd$', 'i'], ['peak.*(traffic|vol)', 'i'], ['peak_?hour_?vol', 'i'], ['peak_?hour$', 'i'], ['peak_?time', 'i'], ['heavy|hcv|pc_?hv', 'i'], ['count_?date', 'i'], ['^date$', 'i'], ['survey_?date', 'i'],
  ['road_?name', 'i'], ['^name$', 'i'], ['street', 'i'], ['location', 'i'], ['site_?desc', 'i'], ['description', 'i'], ['^dir', 'i'], ['direction', 'i'], ['^latest$', 'i'], ['^year_?(\\d{4})$', 'i'], ['oid|geometry|globalid', 'i'],
  ['\\bmotorcar\\b|\\bmotor_vehicle\\b', ''], ['general|all', 'i'], ['variable', 'i'], ['^RG ?(5|6|6R)$', 'i'], ['^\\(\\d+\\)\\s*', ''], ['\\s+', ''], ['\\s+', 'g'], ['heavy', 'i'], ['light', 'i'], ['^[0-9A-Z]{8}$', 'i'], ['auckland', 'i'],
  ['^MVR_([A-Z][a-z]{2})(\\d{2})$', ''], ['^ELECTRIC$', 'i'], ['(\\d{4})', ''], ['\\/query.*$', 'i'], ['\\?.*$', ''], ['\\/+$', ''], ['^https:\\/\\/[^/]+\\/.+\\/(FeatureServer|MapServer)\\/\\d+$', 'i'], ['\\b(NB|SB|EB|WB)\\b', 'i'], ['\\D', 'g'],
  ['^none$', 'i'], ['^no\\b', 'i'], ['pedestrian', 'i'], ['\\b[a-z]', 'g'], ['^SH\\s*\\d', 'i'], ['\\bramp\\b', 'i'], ['^\\s*(\\d+):(\\d{2})(?::(\\d{2}))?', ''], ['^https?:\\/\\/[^\\s"\'<>]+$', 'i'], ['^[\\w][\\w./-]*$', ''],
  ['(\\d{1,2})(?:[:.](\\d{2}))?\\s*(am|pm)?\\s*(?:–|-|to)\\s*(\\d{1,2})(?:[:.](\\d{2}))?\\s*(am|pm)?', 'gi'],
  ['^\\s*([-+]?\\d{1,3}(?:\\.\\d+)?)\\s*°?\\s*([NSEW])?\\s*[,;\\s]\\s*([-+]?\\d{1,3}(?:\\.\\d+)?)\\s*°?\\s*([NSEW])?\\s*$', 'i'],
  ['(\\S\\s+)rd\\.?(?=\\s|,|$)', 'gi'], ['(\\S\\s+)st\\.?(?=\\s*(?:,|$))', 'gi'], ['(\\S\\s+)ave\\.?(?=\\s|,|$)', 'gi'], ['^new zealand(\\s*\\/\\s*aotearoa)?$', 'i'], ['^aotearoa(\\s*\\/\\s*new zealand)?$', 'i'], ['^\\d{4}$', ''], ['^\\d+[a-z]?(?:[-/]\\d+[a-z]?)?$', 'i'],
  // engine coverage: backtracking, lazy quantifiers, groups, lookahead, classes, repeats, alternation order
  ['(a+)+b', ''], ['a+?b', ''], ['(a|ab)(c|bcd)(d*)', ''], ['x*', 'g'], ['(?:ab)*c', ''], ['(z)((a+)?(b+)?(c))*', ''], ['a{2,3}', 'g'], ['a{2,}', ''], ['a{,2}', ''], ['[^a-c]+', 'g'], ['[a\\-z]+', 'g'], ['[\\d.]+', 'g'], ['\\w+@\\w+\\.com', 'i'],
  ['(?=ab)a', ''], ['(?!ab)a', 'g'], ['^$', ''], ['\\Bb', 'g'], ['.', 'g'], ['[\\s\\S]+', ''], ['(?:(?:a|b)*)c', ''], ['é+', 'gi'], ['ä', 'i'], ['[à-ü]+', 'gi'],
];
const INPUTS = ['', 'AADT', 'adt', 'Average_Daily_Traffic', 'PEAK_HOUR', 'peak_hour_vol', 'Peak Traffic Volume', 'Count_Date', 'DATE', 'Site_Desc', 'Description', 'Direction', 'dir', 'Latest', 'year_2022', 'Year2024', 'year_20245', 'OBJECTID', 'GlobalID', 'esriFieldTypeGeometry', 'HCV %', 'pc_hv', 'General traffic', 'bus lane', 'Variable Speed', 'RG 5', 'rg6r', 'RG6', 'RG  5', 'RG 7', '(123) Great South Road / Mill Road', '(12)x', '( 1) a', '  a  b   c ',
  '01N00201', '0AB1C2D3', 'abcdefghi', 'Auckland Transport', 'auckland council', 'MVR_Oct26', 'MVR_oct26', 'MVR_Oct2', 'ELECTRIC', 'electric ', 'from 1999 to 2024', 'abc', 'https://services1.arcgis.com/x/arcgis/rest/services/Count%20Layer/FeatureServer/0', 'https://x.example/rest/Count/FeatureServer/0/query?where=1%3D1', 'https://x/rest/MapServer/12', 'http://x/y/FeatureServer/0', 'https://x//FeatureServer/0', 'Layer/FeatureServer/0///', 'Main Rd NB', 'sb and nb', 'EASTBOUND', 'no left', 'No_left_turn', 'no', 'none', 'None ', 'Pedestrian crossing', 'ramp', 'On-Ramp', 'ramps', 'SH 16', 'sh16', 'SH', 'sh 1a', 'St Heliers', 'The Strand', 'Great South Rd, Manukau, Auckland', 'Remuera Rd.', 'Great North St', 'Queen Street', 'Wellesley st., Auckland', 'Karangahape Road ave', '12 St Marks Rd, Remuera', 'Rd 2', 'Mt Eden Rd Rd', 'a rd b rd', 'x Rd,', 'x Rd', 'x  Dr.', 'Smith Dr, Ellerslie',
  '07:45:00', '25:10:05', '7:5', '  8:30', '24:00', 'noon', '08:30 – 9:15am and 2:45 – 3:30pm', '8am – 9.15am', '7.30 - 9am', '40 - 50', '8:00 to 9:00', '9:00-10:00 and 14:30-15:30', '24:00 - 25:00', '12:00pm - 1:00pm', '8:30am-9:15am', '6 to 7', 'school zone 8.15 am - 9.15 am, 2.45 pm - 3.45 pm', 'Mon-Fri 8am - 9am',
  '-37.10, 174.95', '174.95, -37.10', '37.1 S, 174.95 E', '-37.10 174.95', '45', '-37.1;174.9', '-37.1°, 174.9°', '95, 50', '-37.1, 181', '1,2', '12.5 N 100 W', '37S 174E',
  '12, St Marks Road, Remuera, Auckland, 1050, New Zealand / Aotearoa', 'New Zealand', 'new zealand / aotearoa', 'Aotearoa / New Zealand', 'Aotearoa', '1050', '12-14', '12a', '3/45', '12', 'aaab', 'aaaaaaaaaaaaaaaaaaaaaaab', 'ab', 'abcd', 'abc d', 'zaacbbbcac', 'xxx', 'aaaa', 'aa', 'ZaBc', 'a-b', 'a.b', 'foo@bar.com', 'FOO@BAR.COM', 'Ünïcode café ÀÉÎ', 'aéEé', 'ÄÖü', 'AAbb', 'AA', '\nline', 'x\ty', 'ab\ncd', 'a\r\nb',
];
function regexCases() {
  const c = loadCtx();
  const out = { cases: [] };
  const vm2 = s => vm.runInContext(s, c.ctx);
  for (const [pat, flags] of PATTERNS) {
    const re = new RegExp(pat, flags), ncaps = new RegExp(re.source + '|').exec('').length - 1;
    for (const input of INPUTS) {
      const r = new RegExp(pat, flags.replace('g', ''));
      const m = r.exec(input);
      const rec = { pattern: pat, flags, input };
      rec.exec = m ? { index: m.index, groups: Array.from({ length: ncaps + 1 }, (_, i) => (m[i] === undefined ? null : m[i])) } : null;
      rec.test = new RegExp(pat, flags.replace('g', '')).test(input);
      rec.replace = input.replace(new RegExp(pat, flags), (...a) => { const m0 = a[0], gs = a.slice(1, 1 + ncaps); return `[${m0}|${gs.map(x => (x === undefined ? '' : x)).join(',')}]`; });
      rec.matchAll = flags.includes('g') ? [...input.matchAll(new RegExp(pat, flags))].map(x => ({ index: x.index, text: x[0] })) : null;
      out.cases.push(rec);
    }
  }
  void vm2;
  return out;
}

/* ====================================================================================================
   consts: the sources and thresholds as data
   ==================================================================================================== */
function constCases() {
  const c = loadCtx(), F = c.Feeds, D = c.DataHub;
  const regexToText = (k, v) => (Object.prototype.toString.call(v) === '[object RegExp]' ? String(v) : v);
  return {
    SRC: plain(F.SRC), RULES: JSON.parse(JSON.stringify(F.RULES, regexToText)), TERMS: plain(F.TERMS), MATARIKI: plain(F.MATARIKI), COUNCILS: plain(F.COUNCILS), NZ_FEEDS: plain(F.NZ_FEEDS), DEFAULTS: plain(F.DEFAULTS),
    SOURCES: plain(D.SOURCES), AT_LICENCE: D.AT_LICENCE, ARCGIS_TIMEOUT: D.ARCGIS_TIMEOUT, ROUTE_COLOURS: vm.runInContext('ROUTE_COLOURS', c.ctx),
    safeHref: ['https://a.example/x', 'http://a.example', 'HTTPS://A.EXAMPLE/X?y=1', 'ftp://a.example', 'javascript:alert(1)', 'data/tms_profiles.json', 'x', '', ' https://a.example/with space', 'https://a.example/"q"', "https://a.example/'q'", 'https://a.example/<b>', '  https://a.example/trim  ', '/abs/path', '../up', 'a/b-c.d_e', '_x', '-x', 'a b', undefined, null, 12].map(v => ({ in: v === undefined ? { undefined: true } : v, out: F.safeHref(v) })),
  };
}

/* ====================================================================================================
   loaders: NZTA sites, AT counts and AT controlled intersections through DataHub.loadAll, and how it fails
   ==================================================================================================== */
function makeCountFixtures(sc) {
  const c0 = Object.values(sc.nodes)[0];
  const at = (dx, dy) => ({ x: r7(c0.lng + dx / 88000), y: r7(c0.lat + dy / 110574) });
  const nzta = [], atc = [], ctl = [];
  const desc = ['SH1 NB Papakura to Takanini', 'SH1 SB Papakura to Takanini', 'SH20 EB at Mangere Bridge', 'SH16 WB ramp to Lincoln Rd', 'On-ramp NB at Greenhithe', 'SH1 Southern Motorway between Ellerslie and Greenlane', undefined, '', 'SH22 Drury (both directions)', 'SH1 SB OFF RAMP', 'Virtual site near Bombay', 'SH2 Pokeno'];
  desc.forEach((d, i) => {
    const virtual = i === 10;
    const geom = i === 7 ? null : at(uni(-300, 300), uni(-200, 200));
    nzta.push({
      attributes: { OBJECTID: 100 + i, siteref: i === 6 ? '0' + (5200000 + i) : `0${1 + i % 3}${'NSEW'[i % 4]}00${200 + i}`, description: d, lane: i % 2 ? 'L1' : 'L2', sh: ['1N', '1S', '20', '16', '1', '1', '22', '2', '22', '1S', '1', '2N'][i], sitetype: virtual ? 'Virtual' : pick(['Telemetry', 'Telemetry', 'Short Term']), type: i === 11 ? 'Virtual' : 'Counter',
        percentheavy: pick([4.5, 7, 12.25, null]), aadt1yearago: i === 3 ? 0 : [38210, 41200, 22100, 8800, 6100, 57800, 5400, 3000, 9900, 12000, 15500, 4100][i], aadt2yearsago: i === 3 ? 8200 : i === 8 ? 0 : 36000 + i * 100, aadt3yearsago: 35000, aadt4yearsago: null, aadt5yearsago: 33000 + i, accepteddays: pick([null, 211, 320]) },
      ...(geom ? { geometry: geom } : {}),
    });
  });
  nzta.push({ attributes: { OBJECTID: 120, siteref: '09E00999', description: 'SH3 NB nothing counted', sh: '3', sitetype: 'Telemetry', aadt1yearago: 0, aadt2yearsago: 0 }, geometry: at(10, 10) });
  nzta.push({ attributes: { OBJECTID: 121, siteref: '09E01000', description: 'SH3 Ramp ramp name', sh: '3S', sitetype: 'Telemetry', aadt1yearago: '4500', aadt2yearsago: null, percentheavy: '9.5', accepteddays: '12' }, geometry: at(-80, 60) });
  const roads = ['GREAT SOUTH ROAD', 'Main Road', "O'CONNOR RD", 'MĀNUKA STREET', 'sh1 southern motorway', 'SH 16', 'Sh20', 'Queen Street', 'Side 2N', 'Tamaki Drive'];
  const dates = [Date.UTC(2025, 2, 12), Date.UTC(2025, 8, 3), Date.UTC(2024, 11, 31, 14), Date.UTC(2023, 0, 1), Date.UTC(2025, 3, 2, 12), null, Date.UTC(2022, 5, 14), Date.UTC(2025, 9, 28)];
  roads.forEach((r, i) => atc.push({
    attributes: { OBJECTID: 5000 + i, road_name: r, start_name: pick(['ALBERT STREET', 'main road', "St Mary's Ave"]), end_name: pick(['CONSTABLE RD', 'Great North Rd', null, '']), count_date: dates[i % dates.length], peak_hour: [745, 1730, 5, 0, '800', null, 1530, '0645', 815, 1700][i], count_duration: pick(['7 day', '2 day', '', null]),
      adt: [9500, 14200, 3100, 0, 17800, 5200, 7600, null, 4400, 6200][i], peaktraffic: pick([820, 1210, 260, null]), pcheavy: pick([3.1, 5.5, null]), carr_way_no: pick([null, 12000 + i, '']) },
    geometry: at(uni(-300, 300), uni(-200, 200)),
  }));
  atc.push({ attributes: { OBJECTID: 5100, road_name: 'No Geometry Rd', adt: 4000 } });
  ['(2345) Great South Road / Mill Road', '(77) Main Road and Side 2', 'Plain description', '', null, '(x) not numbered', '(12)   Spaced out'].forEach((d, i) => ctl.push({
    attributes: { OBJECTID: 900 + i, intersection_id: i === 4 ? null : 7000 + i, intersection_no: i % 2 ? null : 300 + i, intersection_desc: d, intersection_type: pick(['Signalised Intersection', 'Pedestrian Crossing', 'Roundabout', 'Signalised T', '']), controlled: i === 5 ? 'No' : 'Yes', ped_crossing_flg: i === 1 ? 'Yes' : 'No' },
    geometry: at(uni(-300, 300), uni(-200, 200)),
  }));
  ctl.push({ attributes: { OBJECTID: 950, intersection_id: 7777, intersection_desc: 'No geometry' } });
  return { nzta: { features: nzta }, at: { features: atc }, ctl: { features: ctl } };
}

async function loaderCases() {
  const out = { cases: [] };
  const base = open('corridor');
  const FX = makeCountFixtures(base.sc);
  writeFixture('nzta-sites.json', FX.nzta); writeFixture('at-counts.json', FX.at); writeFixture('at-controlled.json', FX.ctl);
  const setup = (srv, { nzta = FX.nzta.features, atc = FX.at.features, ctl = FX.ctl.features, page = 5 } = {}) => {
    layerRoute(srv, { match: layerPath('Assets_SHTrafficMonitoringSites'), file: 'nzta-sites.json', features: nzta, page, inline: nzta !== FX.nzta.features });
    layerRoute(srv, { match: layerPath('TrafficService'), file: 'at-counts.json', features: atc, page, inline: atc !== FX.at.features, filter: (f, p) => p.get('where') === "latest='Yes'" });
    layerRoute(srv, { match: layerPath('RoadingService/FeatureServer/2'), file: 'at-controlled.json', features: ctl, page, inline: ctl !== FX.ctl.features, filter: (f, p) => p.get('where') === '1=1' || (p.get('where') === "controlled='Yes'" && f.attributes.controlled === 'Yes') });
  };
  const run = async (label, which, { prep = null, mutate = null, opts = {}, cancelAfter = null, srvOpts = {}, custom = null } = {}) => {
    const srv = makeServer(); setup(srv, srvOpts); if (custom) custom(srv);
    const o = open('corridor', { fetchImpl: srv.fetchImpl, now: NOW }), c = o.c, P = o.P, d = c.DataHub.D();
    if (prep) prep(c, P, d);
    const b = boundsOf(o.sc), st = [];
    const ac = new AbortController();
    if (cancelAfter === 0) ac.abort(); else if (cancelAfter) setTimeout(() => ac.abort(), cancelAfter);
    const rec = { label, which, opts: { timeout: opts.timeout ?? null, cancel: cancelAfter === null ? null : cancelAfter }, bounds: boxOf(b), startCounts: prep ? plain(clone(d.counts)) : null };
    try { rec.msgs = await c.DataHub.loadAll(b, which ?? undefined, t => st.push(t), { ...opts, ...(cancelAfter !== null ? { signal: ac.signal } : {}) }); }
    catch (e) { rec.thrown = e.name === 'AbortError' ? 'Cancelled' : e.message; }
    rec.statuses = st; rec.requests = srv.log;
    rec.data = plain(JSON.parse(JSON.stringify({ counts: d.counts, controlled: d.controlled, loaded: d.loaded, crashes: d.crashes })));
    out.cases.push(rec);
  };
  await run('nzta', ['nzta']);
  await run('at', ['at']);
  await run('both', ['nzta', 'at']);
  await run('controlled', ['controlled']);
  await run('all-three', ['nzta', 'at', 'controlled']);
  await run('one-page', ['nzta', 'at', 'controlled'], { srvOpts: { page: 500 } });
  await run('unknown', ['nzta', 'rail', 'AT', 'x']);
  await run('default-sources', null);
  // counts already there: the user's own, an earlier load switched off by the user, an earlier load unchanged
  await run('reload-keeps-off', ['nzta', 'at'], { prep: (c, P, d) => { d.counts = [{ id: 'user:keep', src: 'user', name: 'Mine', lat: -37.1, lng: 174.95, adt: 100 }, { id: 'nzta:01N00200', src: 'nzta', off: true, adt: 1 }, { id: 'nzta:02E00201', src: 'nzta', adt: 2 }, { id: 'nzta:gone', src: 'nzta', off: true, adt: 3 }, { id: 'at:5000', src: 'at', off: false }, { id: 'at:5001', src: 'at' }]; } });
  // failures of a source: the rest still load
  for (const [label, desc] of [['http-500', { failure: 'http', status: 500 }], ['http-404', { failure: 'http', status: 404 }], ['unreachable', { failure: 'unreachable', message: 'Failed to fetch' }], ['unreadable', { failure: 'unreadable', message: 'Unexpected token < in JSON' }]]) {
    await run('nzta-' + label, ['nzta', 'at'], { custom: srv => srv.routes.unshift({ test: layerPath('Assets_SHTrafficMonitoringSites'), answer: failing(srv, desc) }) });
  }
  await run('timeout', ['nzta', 'at'], { opts: { timeout: 1200 }, custom: srv => srv.routes.unshift({ test: layerPath('Assets_SHTrafficMonitoringSites'), answer: failing(srv, { failure: 'timeout' }) }) });
  await run('timeout-reading', ['nzta'], { opts: { timeout: 1200 }, custom: srv => srv.routes.unshift({ test: layerPath('Assets_SHTrafficMonitoringSites'), answer: failing(srv, { failure: 'timeout-reading' }) }) });
  await run('service-error', ['nzta', 'at'], { custom: srv => srv.routes.unshift({ test: layerPath('Assets_SHTrafficMonitoringSites'), answer: async () => ({ desc: { kind: 'literal', value: { error: { code: 400, message: 'Invalid query parameters' } } }, response: srv.ok({ error: { code: 400, message: 'Invalid query parameters' } }) }) }) });
  await run('service-error-no-message', ['nzta'], { custom: srv => srv.routes.unshift({ test: layerPath('Assets_SHTrafficMonitoringSites'), answer: async () => ({ desc: { kind: 'literal', value: { error: {} } }, response: srv.ok({ error: {} }) }) }) });
  await run('answer-is-null', ['nzta'], { custom: srv => srv.routes.unshift({ test: layerPath('Assets_SHTrafficMonitoringSites'), answer: async () => ({ desc: { kind: 'literal', value: null }, response: srv.ok(null) }) }) });
  await run('answer-is-text', ['nzta'], { custom: srv => srv.routes.unshift({ test: layerPath('Assets_SHTrafficMonitoringSites'), answer: async () => ({ desc: { kind: 'literal', value: 'hello' }, response: srv.ok('hello') }) }) });
  await run('no-features-key', ['nzta'], { custom: srv => srv.routes.unshift({ test: layerPath('Assets_SHTrafficMonitoringSites'), answer: async () => ({ desc: { kind: 'literal', value: { exceededTransferLimit: true } }, response: srv.ok({ exceededTransferLimit: true }) }) }) });
  await run('twenty-five-pages', ['nzta'], { srvOpts: { page: 1 }, custom: srv => { srv.routes.length = 0; layerRoute(srv, { match: layerPath('Assets_SHTrafficMonitoringSites'), file: 'nzta-sites.json', features: FX.nzta.features.concat(FX.nzta.features).concat(FX.nzta.features), page: 1, envelope: false, inline: true }); } });
  // records that fail to read
  await run('sh-is-a-number', ['nzta', 'at'], { srvOpts: { nzta: [{ attributes: { OBJECTID: 1, siteref: 'X', sh: 1, aadt1yearago: 1000 }, geometry: { x: 174.95, y: -37.1 } }] } });
  await run('nzta-no-attributes', ['nzta'], { srvOpts: { nzta: [{ geometry: { x: 174.95, y: -37.1 } }] } });
  await run('at-no-attributes', ['at'], { srvOpts: { atc: [{ geometry: { x: 174.95, y: -37.1 } }] } });
  await run('at-bad-date', ['at'], { srvOpts: { atc: [{ attributes: { OBJECTID: 1, road_name: 'A Road', adt: 1000, count_date: 'garbage' }, geometry: { x: 174.95, y: -37.1 } }] } });
  await run('at-odd-values', ['at'], { srvOpts: { atc: [{ attributes: { OBJECTID: 1, road_name: 'A Road', start_name: 5, end_name: 'x', adt: '1000', peak_hour: 12345, count_date: '2025-03-12T00:00:00Z', count_duration: 7, carr_way_no: 0 }, geometry: { x: 174.95, y: -37.1 } }, { attributes: { OBJECTID: 2, road_name: 'B Road', adt: 500, peak_hour: '', count_date: 1e20 }, geometry: { x: 174.95, y: -37.1 } }] } });
  // the user cancels: before the load, and while a request is waiting
  await run('cancel-before', ['nzta', 'at'], { cancelAfter: 0 });
  await run('cancel-during', ['nzta', 'at'], { cancelAfter: 60, custom: srv => srv.routes.unshift({ test: layerPath('Assets_SHTrafficMonitoringSites'), answer: failing(srv, { failure: 'cancel' }) }) });
  return out;
}

/* ====================================================================================================
   nslr: legal speed limits
   ==================================================================================================== */
const bboxOf = sc => { const ns = Object.values(sc.nodes); return { s: Math.min(...ns.map(n => n.lat)), n: Math.max(...ns.map(n => n.lat)), w: Math.min(...ns.map(n => n.lng)), e: Math.max(...ns.map(n => n.lng)) }; };
// a rectangle as an ArcGIS ring [lng, lat] (clockwise), by fractions of a box widened a little
const rect = (bb, fx0, fy0, fx1, fy1, pad = 0.0005, z = false) => {
  const W = bb.e - bb.w + 2 * pad, H = bb.n - bb.s + 2 * pad, x0 = r7(bb.w - pad + fx0 * W), x1 = r7(bb.w - pad + fx1 * W), y0 = r7(bb.s - pad + fy0 * H), y1 = r7(bb.s - pad + fy1 * H);
  const q = (x, y) => (z ? [x, y, 0] : [x, y]);
  return [q(x0, y0), q(x0, y1), q(x1, y1), q(x1, y0), q(x0, y0)];
};
const anti = ring => ring.slice().reverse();
function makeNslr(sc) {
  const bb = bboxOf(sc), D = (y, m = 0, d = 1) => Date.UTC(y, m, d);
  const f = [];
  const zone = (o, rings) => f.push({ attributes: { OBJECTID: 3000 + f.length, speedLimitZoneValue: o.v, speedCategoryName: o.cat ?? 'Permanent', speedLimitZoneVariableSpeed: o.vv ?? null, speedLimitZoneVarPrdDesc: o.desc ?? null, speedLimitZoneDesc: 'Zone ' + f.length,
    speedLimitZoneStartDate: o.start === undefined ? D(2015) : o.start, speedLimitZoneEndDate: o.end ?? null, speedLimitZoneLanePurposeName: o.lane === undefined ? 'General traffic' : o.lane, rcaZoneReferenceName: o.rca ?? 'Auckland Transport', legalReference: o.ref ?? 'Land Transport Rule: Setting of Speed Limits 2022', legalInstrumentURL: o.url ?? 'https://at.govt.nz/bylaws/speed-limits' }, geometry: { rings } });
  zone({ v: 50, cat: 'Urban Traffic Area' }, [rect(bb, -0.2, -0.2, 0.85, 1.2, 0, true)]);
  zone({ v: 60, start: D(2019, 5, 1), rca: 'NZTA Waka Kotahi' }, [rect(bb, -0.1, -0.1, 0.45, 1.1)]);
  zone({ v: 80, start: D(2010), end: D(2019, 5, 1) }, [rect(bb, -0.1, -0.1, 0.45, 1.1)]);                      // an ended version
  zone({ v: 50, cat: 'Variable', vv: 40, desc: '08:00 – 09:15am and 2:45 – 3:30pm', start: D(2021, 2, 1) }, [rect(bb, 0.4, -0.1, 0.7, 1.1)]);   // school zone
  zone({ v: 50, cat: 'Variable', vv: null, desc: '', start: D(2022, 2, 1) }, [rect(bb, 0.62, 0.3, 0.8, 0.7)]);  // variable, no variable speed
  zone({ v: 30, lane: 'Bus lane' }, [rect(bb, 0.1, 0.1, 0.9, 0.9)]);                                            // a lane purpose that is not general traffic
  zone({ v: 0 }, [rect(bb, 0.3, 0.3, 0.6, 0.6)]);                                                               // no limit
  zone({ v: '70 km/h', start: D(2020, 0, 1), cat: 'Permanent' }, [rect(bb, 0.7, -0.1, 1.1, 0.4)]);              // a limit as text
  zone({ v: 40, start: D(2028, 0, 1) }, [rect(bb, -0.1, 0.5, 0.5, 1.1)]);                                       // not yet started
  zone({ v: 100, end: D(2029, 11, 31), start: D(2018), cat: 'Open Road', lane: null }, [rect(bb, 0.8, 0.5, 1.2, 1.2)]); // ends before 2031
  zone({ v: 30, start: D(2023, 0, 1), cat: 'Permanent' }, [rect(bb, 0.2, 0.2, 0.8, 0.8), anti(rect(bb, 0.4, 0.4, 0.6, 0.6))]);   // with a hole
  zone({ v: 45, start: undefined }, [rect(bb, 0.9, 0.9, 1.0, 1.0)]);                                            // no start date at all
  f[f.length - 1].attributes.speedLimitZoneStartDate = undefined;
  zone({ v: 110, cat: 'Open Road', start: D(2016) }, [rect(bb, -0.3, -0.3, -0.2, -0.2)]);                      // outside the network
  f.push({ attributes: { OBJECTID: 3999, speedLimitZoneValue: 50 } });                                          // no geometry
  f.push({ attributes: { OBJECTID: 3998, speedLimitZoneValue: 50 }, geometry: { rings: [] } });                 // no rings
  return { features: f };
}
const NSLR_META_FIELDS = ['OBJECTID', 'speedLimitZoneValue', 'speedCategoryName', 'speedLimitZoneVariableSpeed', 'speedLimitZoneVarPrdDesc', 'speedLimitZoneDesc', 'speedLimitZoneStartDate', 'speedLimitZoneEndDate', 'speedLimitZoneLanePurposeName', 'RcaZoneReferenceName', 'legalReference', 'Shape__Area', 'Shape__Length'];

const linkFields = sc => Object.fromEntries(Object.values(sc.links).map(L => [L.id, { speed: L.speed, speedLimit: L.speedLimit, speedSource: L.speedSource, nslr: L.nslr, varSpeed: L.varSpeed, speedOSM: L.speedOSM, speedApplied: L.speedApplied }]));

async function nslrCases() {
  const out = { varPeriods: [], where: [], loads: [], assigns: [], proposals: [], alls: [] };
  // the register for each test project
  const FX = {};
  for (const name of NAMES) { const o = open(name); FX[name] = makeNslr(o.sc); writeFixture(`nslr-zones-${name}.json`, FX[name]); }
  writeFixture('nslr-meta.json', metaOf('SpeedLimitZoneFull__View', NSLR_META_FIELDS));
  const metaBody = metaOf('SpeedLimitZoneFull__View', NSLR_META_FIELDS);
  const serve = (srv, name, { meta = true, page = 6 } = {}) => {
    if (meta) jsonRoute(srv, metaPath('SpeedLimitZoneFull__View'), 'nslr-meta.json', metaBody);
    else failRoute(srv, metaPath('SpeedLimitZoneFull__View'), { failure: 'http', status: 500 });
    layerRoute(srv, { match: layerPath('SpeedLimitZoneFull__View'), file: `nslr-zones-${name}.json`, features: FX[name].features, page });
  };
  // variable limit texts
  const texts = ['08:30 – 9:15am and 2:45 – 3:30pm', '8am – 9.15am', '7.30 - 9am', '40 - 50', '8:00 to 9:00', '9:00-10:00 and 14:30-15:30', '24:00 - 25:00', '12:00pm - 1:00pm', '12:00am - 1:00am', '8:30am-9:15am', '6 to 7', 'school zone 8.15 am - 9.15 am, 2.45 pm - 3.45 pm', 'Mon-Fri 8am - 9am', '', null, undefined, '3pm - 4pm', '2:30 - 3:30pm', '7:00 - 9:00', '15:00 - 17:30', '11:30 - 12:30', '9am - 8am', '23:00 - 07:00', '8:00 - 9:00 and 8:30 - 9:30', '07:45–08:45', '12 - 1pm', '12am - 1am', '1:00 - 2:00', '17:00 – 18:00 weekdays', '10 - 11 am', '15:15 to 16:15 Mon to Fri'];
  { const c = loadCtx(); for (const t of texts) out.varPeriods.push({ text: t === undefined ? { undefined: true } : t, result: plain(c.Feeds.nslr.varPeriods(t)) }); }
  // where and as-at dates
  { const c = loadCtx();
    for (const now of [NOW, Date.UTC(2027, 5, 1), Date.UTC(2026, 11, 31, 22), Date.UTC(2025, 11, 31, 10, 59)]) for (const year of [undefined, null, 0, 2026, 2027, 2031, '2031', 2040, NaN, 'x']) out.where.push({ now, year: year === undefined ? { undefined: true } : Number.isNaN(year) ? 'NaN' : year, where: c.Feeds.nslr.where(year, new Date(now)) });
  }
  // the downloads
  for (const name of NAMES) for (const [label, year, meta, now] of [['now', null, true, NOW], ['2031', 2031, true, NOW], ['no-meta', null, false, NOW], ['year-now', 2026, true, NOW], ['2027-in-2027', 2027, true, Date.UTC(2027, 5, 1)], ['2031-no-meta', 2031, false, NOW]]) {
    const srv = makeServer(); serve(srv, name, { meta });
    const o = open(name, { fetchImpl: srv.fetchImpl, now });
    const set = await o.c.Feeds.nslr.load(boundsOf(o.sc), year, new Date(now));
    out.loads.push({ project: name, label, year, now, meta, bounds: boxOf(boundsOf(o.sc)), requests: srv.log, set: plain(set) });
  }
  // assign: each scenario and set, then proposals and apply
  for (const name of NAMES) {
    for (const [label, year] of [['now', null], ['2031', 2031]]) {
      const srv = makeServer(); serve(srv, name);
      const o = open(name, { fetchImpl: srv.fetchImpl });
      const set = await o.c.Feeds.nslr.load(boundsOf(o.sc), year, new Date(NOW));
      for (const [si, sc] of o.P.scenarios.entries()) {
        const rec = { project: name, label, scenario: sc.id, setYear: year };
        // some links the user has changed or that already carry an old load
        const links = Object.values(sc.links);
        links.forEach((L, i) => { if (i % 9 === 0) { L.speedSource = 'NSLR'; L.speedLimit = 40; L.nslr = { v: 40 }; L.varSpeed = { v: 20 }; } if (i % 11 === 1) L.speed = 77; });
        rec.before = plain(linkFields(sc));
        rec.stats = plain(o.c.Feeds.nslr.assign(sc, set));
        rec.after = plain(linkFields(sc));
        rec.proposals = plain(o.c.Feeds.nslr.proposals(sc));
        // apply to some of the proposed roads, then change one speed by hand and look again
        const ids = rec.proposals.filter((_, i) => i % 2 === 0).map(p => p.id).concat(['no-such-link', links[0].id]);
        rec.applyIds = ids; rec.applied = o.c.Feeds.nslr.apply(sc, ids);
        rec.afterApply = plain(linkFields(sc));
        if (links[2]) { links[2].speed = 31; rec.manualSpeed = { id: links[2].id, speed: 31 }; }
        rec.proposals2 = plain(o.c.Feeds.nslr.proposals(sc));
        rec.applied2 = o.c.Feeds.nslr.apply(sc, rec.proposals2.map(p => p.id));
        rec.afterApply2 = plain(linkFields(sc));
        out.assigns.push(rec);
        void si;
      }
    }
  }
  // the whole download for every scenario: the years the scenarios have, with and without gazetted future limits
  for (const name of NAMES) for (const [label, setup] of [['all-2031', null], ['first-2026', P => { P.scenarios[0].year = 2026; }], ['no-future', P => { P.settings.feeds = { nslrFuture: false }; }], ['years-mixed', P => { P.scenarios.forEach((s, i) => { s.year = [2026, 2031, 2041][i % 3]; }); }], ['future-and-cut', P => { P.settings.feeds = { nslrFuture: true, timeout: 12 }; }]]) {
    const srv = makeServer(); serve(srv, name);
    const o = open(name, { fetchImpl: srv.fetchImpl });
    if (setup) setup(o.P);
    const st = [];
    const rec = { project: name, label, bounds: boxOf(boundsOf(o.sc)), startFeeds: o.P.settings.feeds ?? null, years: o.P.scenarios.map(s => s.year ?? null) };
    const res = await o.c.Feeds.nslr.all(boundsOf(o.sc), o.P.scenarios, t => st.push(t));
    rec.result = plain(res); rec.statuses = st; rec.requests = srv.log;
    rec.feeds = plain(clone(o.c.Feeds.F())); rec.links = plain(Object.fromEntries(o.P.scenarios.map(s => [s.id, linkFields(s)])));
    out.alls.push(rec);
  }
  return out;
}

/* ====================================================================================================
   control: AT signal sites and signs; bans: OpenStreetMap turn restrictions
   ==================================================================================================== */
function incident(sc) {
  const at = new Map();
  for (const L of Object.values(sc.links)) if (L.status !== 'closed' && sc.nodes[L.a] && sc.nodes[L.b]) for (const n of [L.a, L.b]) { const a = at.get(n); if (a) a.push(L); else at.set(n, [L]); }
  return at;
}
function offsetPoint(F, ll, bearingDeg, m) { const b = bearingDeg * Math.PI / 180; return [ll[0] + m * Math.cos(b) / F.ky, ll[1] + m * Math.sin(b) / F.kx]; }

function makeControl(c, sc) {
  const { U, Net } = c, at = incident(sc), F = U.flat(Object.values(sc.nodes)[0].lat);
  const sites = [], signs = [];
  let no = 100, oid = 800, sid = 6000;
  const site = (ll, o = {}) => { sites.push({ attributes: { OBJECTID: oid++, intersection_id: 7000 + oid, intersection_no: o.no ?? no++, intersection_desc: o.desc ?? `(${no}) ${o.name || 'Site'}`, intersection_type: o.type ?? 'Signalised Intersection', controlled: o.controlled ?? 'Yes', ped_crossing_flg: o.ped ? 'Yes' : 'No' }, geometry: { x: r7(ll[1]), y: r7(ll[0]) } }); };
  const sign = (ll, type, road) => signs.push({ attributes: { OBJECTID: sid++, sign_id: 9000 + sid, sign_type: type, side: pick(['Left', 'Right', null]), road_name: road, legend_note: null }, geometry: { x: r7(ll[1]), y: r7(ll[0]) } });
  const junctions = [...at].filter(([, ls]) => ls.length >= 3).map(([id]) => id), twos = [...at].filter(([, ls]) => ls.length === 2).map(([id]) => id);
  junctions.forEach((id, i) => {
    const n = sc.nodes[id], ls = at.get(id);
    if (i % 4 !== 3) site(offsetPoint(F, [n.lat, n.lng], uni(0, 360), uni(3, 34)), { name: ls.map(l => l.name).filter(Boolean).slice(0, 2).join(' / ') });
    if (i % 5 === 1) site(offsetPoint(F, [n.lat, n.lng], uni(0, 360), uni(42, 80)), { name: 'Too far' });             // beyond 40 m
    if (i % 3 === 0) site(offsetPoint(F, [n.lat, n.lng], uni(0, 360), uni(3, 30)), { name: 'Second site at the same junction' });
    // signs on the approaches: the last 30 m of each link into the node, three metres to the left or right of travel
    ls.slice(0, 3).forEach((L, k) => {
      let g = Net.geom(sc, L); if (L.a === id) g = g.slice().reverse();
      const cum = U.cumLen(g), len = cum[cum.length - 1], p = U.along(g, cum, Math.max(0, len - uni(5, 25))).p, brg = U.bearing(U.along(g, cum, Math.max(0, len - 30)).p, g[g.length - 1]);
      const side = (i + k) % 3 === 0 ? -1 : 1;       // -1: left of the direction of travel, +1: right
      sign(offsetPoint(F, p, brg + 90 * side, 3), ['RG 5', 'RG 6', 'RG 6R', 'rg 5', 'RG6', 'RG  6', 'RG 6'][(i * 3 + k) % 7], L.name || 'Unnamed');
    });
  });
  twos.forEach((id, i) => { const n = sc.nodes[id]; if (i % 2 === 0) site(offsetPoint(F, [n.lat, n.lng], uni(0, 360), uni(2, 30)), { ped: true, type: 'Pedestrian Crossing', name: 'Crossing at ' + id.slice(-3) }); });
  // a pedestrian crossing in the middle of a road (it splits the road) and one with no road near
  const L0 = Object.values(sc.links).find(l => l.status !== 'closed' && !l.ring);
  const g0 = Net.geom(sc, L0), cum0 = U.cumLen(g0), mid = U.along(g0, cum0, cum0[cum0.length - 1] * 0.5).p;
  site(offsetPoint(F, mid, 90, 4), { ped: true, type: 'Pedestrian crossing', name: 'Mid-block crossing', desc: '(55) Mid-block crossing' });
  site([sc.nodes[Object.keys(sc.nodes)[0]].lat + 0.01, sc.nodes[Object.keys(sc.nodes)[0]].lng], { ped: true, name: 'Far away crossing' });
  site(offsetPoint(F, mid, 0, 500), { name: 'Nowhere' });
  site(offsetPoint(F, [sc.nodes[junctions[0]].lat, sc.nodes[junctions[0]].lng], 45, 10), { controlled: 'No', name: 'Not controlled' });
  sign(offsetPoint(F, mid, 0, 300), 'RG 5', 'Far sign');
  sign(offsetPoint(F, [sc.nodes[junctions[0]].lat, sc.nodes[junctions[0]].lng], 90, 5), 'RG 1', 'Other sign');
  sign(offsetPoint(F, [sc.nodes[junctions[0]].lat, sc.nodes[junctions[0]].lng], 180, 6), 'RG 12', 'Other sign');
  signs.push({ attributes: { OBJECTID: sid++, sign_type: 'RG 5', road_name: 'No geometry' } });
  return { sites: { features: sites }, signs: { features: signs } };
}
function prepControl(c, sc, variant) {
  const at = incident(sc), junctions = [...at].filter(([, ls]) => ls.length >= 3).map(([id]) => id);
  if (variant === 'states') {
    junctions.forEach((id, i) => {
      const n = sc.nodes[id];
      if (i % 6 === 0) n.control = 'signals';
      if (i % 6 === 1) n.control = 'roundabout';
      if (i % 6 === 2) { n.control = 'stop'; n.controlSrc = 'AT signs'; n.controlAuto = 'priority'; n.atSiteNo = 4; }       // changed by the user afterwards
      if (i % 6 === 3) { n.control = 'priority'; n.controlSrc = 'AT signs'; n.controlAuto = 'priority'; n.atSiteNo = 999; }
      if (i % 6 === 4) { n.control = 'signals'; n.controlSrc = 'AT'; n.controlAuto = 'signals'; }
    });
  }
  if (variant === 'sites' || variant === 'states') {
    const ids = junctions.slice(0, 4);
    sc.sites = { sA: { id: 'sA', nodes: ids.slice(0, 2), name: 'Site A' }, sB: { id: 'sB', nodes: ids.slice(2, 3), name: 'Site B', atSiteNo: 31 } };
  }
  if (variant === 'ring') {
    // a roundabout ring on the first junction: three short links round a new node
    const id = junctions[0], n = sc.nodes[id], F = c.U.flat(n.lat), mk = (dx, dy) => c.Net.addNode(sc, [n.lat + dy / F.ky, n.lng + dx / F.kx]);
    const a = mk(8, 0), b = mk(-4, 7), d = mk(-4, -7);
    c.Net.addLink(sc, a, b, [], { name: 'Ring', cls: 'collector', ring: true, lanesAB: 1, lanesBA: 0 }); c.Net.addLink(sc, b, d, [], { name: 'Ring', cls: 'collector', ring: true, lanesAB: 1, lanesBA: 0 }); c.Net.addLink(sc, d, a, [], { name: 'Ring', cls: 'collector', ring: true, lanesAB: 1, lanesBA: 0 });
    c.Net.addLink(sc, id, a, [], { name: 'Ring feeder', cls: 'local' });
  }
  if (variant === 'closed') { Object.values(sc.links).forEach((L, i) => { if (i % 5 === 2) L.status = 'closed'; }); }
}
const nodeFields = sc => Object.fromEntries(Object.values(sc.nodes).map(n => [n.id, n]));

async function controlCases() {
  const out = { loads: [], proposals: [], banLoads: [], bans: [] };
  const FX = {};
  for (const name of NAMES) { const o = open(name); FX[name] = makeControl(o.c, o.sc); writeFixture(`control-sites-${name}.json`, FX[name].sites); writeFixture(`control-signs-${name}.json`, FX[name].signs); }
  const metaC = metaOf('RoadingService', ['OBJECTID', 'intersection_id', 'intersection_no', 'intersection_desc', 'intersection_type', 'controlled', 'ped_crossing_flg', 'last_edited_date']);
  const metaS = metaOf('RoadingService0', ['OBJECTID', 'sign_id', 'SIGN_TYPE', 'side', 'road_name', 'legend_note']);
  const serve = (srv, name, { metas = true, page = 7 } = {}) => {
    if (metas) { jsonRoute(srv, metaPath('RoadingService/FeatureServer/2'), 'control-meta-sites.json', metaC); jsonRoute(srv, metaPath('RoadingService/FeatureServer/0'), 'control-meta-signs.json', metaS); }
    else { failRoute(srv, metaPath('RoadingService/FeatureServer/2'), { failure: 'http', status: 503 }); failRoute(srv, metaPath('RoadingService/FeatureServer/0'), { failure: 'unreachable', message: 'Failed to fetch' }); }
    layerRoute(srv, { match: layerPath('RoadingService/FeatureServer/2'), file: `control-sites-${name}.json`, features: FX[name].sites.features, page, filter: (f, p) => (p.get('where') !== "controlled='Yes'" || f.attributes.controlled === 'Yes') });
    layerRoute(srv, { match: layerPath('RoadingService/FeatureServer/0'), file: `control-signs-${name}.json`, features: FX[name].signs.features, page, filter: (f, p) => { const m = /sign_type IN \((.*)\)/.exec(p.get('where') || ''); return !m || m[1].split(',').map(s => s.trim().replace(/'/g, '')).includes(f.attributes.sign_type); } });
  };
  writeFixture('control-meta-sites.json', metaC); writeFixture('control-meta-signs.json', metaS);
  for (const name of NAMES) {
    for (const [label, metas] of [['with-meta', true], ['no-meta', false]]) {
      const srv = makeServer(); serve(srv, name, { metas });
      const o = open(name, { fetchImpl: srv.fetchImpl });
      const data = await o.c.Feeds.control.load(boundsOf(o.sc));
      out.loads.push({ project: name, label, bounds: boxOf(boundsOf(o.sc)), requests: srv.log, data: plain(data), dataD: plain(clone({ controlled: o.c.DataHub.D().controlled, loaded: o.c.DataHub.D().loaded })), feeds: plain(clone(o.c.Feeds.F())) });
    }
    // an error in the second layer: the whole load fails
    { const srv = makeServer(); serve(srv, name); srv.routes.unshift({ test: layerPath('RoadingService/FeatureServer/0'), answer: failing(srv, { failure: 'http', status: 500 }) });
      const o = open(name, { fetchImpl: srv.fetchImpl }); const rec = { project: name, label: 'signs-fail', bounds: boxOf(boundsOf(o.sc)) };
      try { await o.c.Feeds.control.load(boundsOf(o.sc)); } catch (e) { rec.thrown = e.message; }
      rec.requests = srv.log; rec.feeds = plain(clone(o.c.Feeds.F())); out.loads.push(rec); }
    // proposals and applying, in several situations of the network
    for (const variant of ['plain', 'states', 'sites', 'ring', 'closed']) {
      const srv = makeServer(); serve(srv, name);
      const o = open(name, { fetchImpl: srv.fetchImpl });
      const data = await o.c.Feeds.control.load(boundsOf(o.sc));
      for (const [si, sc] of o.P.scenarios.entries()) {
        if (si > 1) continue;
        prepControl(o.c, sc, variant);
        const rec = { project: name, variant, scenario: sc.id, startNodes: plain(clone(nodeFields(sc))), startLinks: plain(clone(sc.links)), startSites: plain(clone(sc.sites ?? null)) };
        const prop = o.c.Feeds.control.proposals(sc, data);
        rec.rows = plain(prop.rows); rec.unmatched = plain(prop.unmatched);
        rec.noData = plain(o.c.Feeds.control.proposals(sc, null));
        // applying: everything, then (on a fresh copy) a subset with the date given, recording the ids the prototype made for split roads
        const text = o.c.Store.serialise(o.P);
        rec.startFile = `control-start-${name}-${variant}-${si}.json`; writeFixture(rec.startFile, text);
        for (const [pickLabel, pick] of [['all', r => true], ['some', r => (rec.rows.indexOf(r) % 2 === 0)], ['none', () => false], ['splits', r => !!r.split]]) {
          const c2 = loadCtx({ seed: 11 }); const P2 = reopen(c2, text); const sc2 = P2.scenarios[si];
          const ids = []; const uid = c2.U.uid; c2.U.uid = p => { const id = uid(p); ids.push(id); return id; };
          const prop2 = c2.Feeds.control.proposals(sc2, clone(data));
          const chosen = rec.rows.filter(pick).map(r => r.id).concat(['unknown-row']);
          const n = c2.Feeds.control.apply(sc2, prop2.rows, chosen, '2026-10-21');
          rec['apply_' + pickLabel] = { ids: chosen, n, uids: ids, nodes: plain(clone(nodeFields(sc2))), links: plain(clone(sc2.links)), sites: plain(clone(sc2.sites ?? null)) };
        }
        out.proposals.push(rec);
      }
    }
  }
  /* ---- turn bans ---- */
  const BFX = {};
  for (const name of NAMES) {
    const o = open(name), sc = o.sc, at = incident(sc);
    // OSM ids on the nodes and ways on the links: two links of a name share a way where they meet (a way running through a junction)
    const nodes = Object.values(sc.nodes); nodes.forEach((n, i) => { n.osm = 5000000 + i * 7; });
    const links = Object.values(sc.links); let way = 400000; const wayOf = new Map();
    links.forEach((L, i) => { const key = L.name && i % 3 === 0 ? L.name : 'w' + i; if (!wayOf.has(key)) wayOf.set(key, way += 13); L.osm = `${wayOf.get(key)}:${i % 4}`; });
    { const nodeOsm = new Map(nodes.map(n => [n.id, n.osm])), linkOsm = new Map(links.map(l => [l.id, l.osm]));
      for (const s of o.P.scenarios.slice(1)) { for (const n of Object.values(s.nodes)) if (nodeOsm.has(n.id)) n.osm = nodeOsm.get(n.id); for (const l of Object.values(s.links)) if (linkOsm.has(l.id)) l.osm = linkOsm.get(l.id); } }
    const text = o.c.Store.serialise(o.P);
    const rels = []; let rid = 12000;
    const kinds = ['no_left_turn', 'no_right_turn', 'no_straight_on', 'no_u_turn', 'only_left_turn', 'only_right_turn', 'only_straight_on', 'only_u_turn'];
    const rel = (from, via, to, tags, extra = {}) => rels.push({ type: 'relation', id: rid++, members: [{ type: extra.fromType ?? 'way', ref: from, role: 'from' }, { type: extra.viaType ?? 'node', ref: via, role: 'via' }, { type: 'way', ref: to, role: 'to' }, ...(extra.more || [])], tags: { type: 'restriction', ...tags } });
    let k = 0;
    for (const [id, ls] of at) {
      if (ls.length < 3) continue;
      const n = sc.nodes[id], other = ls.filter(l => l.osm.split(':')[0] !== ls[0].osm.split(':')[0]);
      if (!other.length) continue;
      const a = ls[0], b = other[0];
      rel(+a.osm.split(':')[0], n.osm, +b.osm.split(':')[0], { restriction: kinds[k++ % kinds.length] });
      if (k % 3 === 0) rel(+b.osm.split(':')[0], n.osm, +a.osm.split(':')[0], { restriction: kinds[k++ % kinds.length], except: 'bicycle' });
    }
    const n0 = nodes[0], l0 = links[0], l1 = links[1];
    rel(+l0.osm.split(':')[0], n0.osm, +l1.osm.split(':')[0], { restriction: 'no_left_turn', 'restriction:conditional': 'no_left_turn @ (Mo-Fr 07:00-09:00)' });
    rel(+l0.osm.split(':')[0], n0.osm, +l1.osm.split(':')[0], { restriction: 'no_right_turn', except: 'motorcar;bicycle' });
    rel(+l0.osm.split(':')[0], n0.osm, +l1.osm.split(':')[0], { restriction: 'no_entry' });
    rel(+l0.osm.split(':')[0], n0.osm, +l1.osm.split(':')[0], { 'restriction:hgv': 'no_left_turn' });
    rel(+l0.osm.split(':')[0], n0.osm, +l1.osm.split(':')[0], {});
    rel(+l0.osm.split(':')[0], 12345, +l1.osm.split(':')[0], { restriction: 'no_left_turn' });                       // a junction outside the network
    rel(+l0.osm.split(':')[0], 7, +l1.osm.split(':')[0], { restriction: 'no_left_turn' }, { viaType: 'way' });          // via a way
    rel(+l0.osm.split(':')[0], n0.osm, +l1.osm.split(':')[0], { restriction: 'no_left_turn' }, { more: [{ type: 'way', ref: 99, role: 'from' }] });   // two from ways
    rel(999999, n0.osm, 888888, { restriction: 'no_left_turn' });                                                    // roads not in the model
    rel(+l0.osm.split(':')[0], n0.osm, +l1.osm.split(':')[0], { restriction: 'no_left_turn' }, { fromType: 'relation' });
    rels.push({ type: 'relation', id: rid++, members: 'not a list', tags: { type: 'restriction', restriction: 'no_left_turn' } });
    rels.push({ type: 'relation', id: rid++, members: [] });
    rels.push({ type: 'node', id: 5000000, lat: -37, lon: 174 }, { type: 'way', id: 4, nodes: [1, 2] });
    BFX[name] = { text, answer: { version: 0.6, generator: 'Overpass API 0.7.62', elements: rels } };
    writeFixture(`bans-${name}.json`, BFX[name].answer); writeFixture(`bans-${name}-project.json`, text);
  }
  for (const name of NAMES) {
    for (const [label, route] of [['first-server', 'ok'], ['second-server', 'second'], ['all-fail', 'fail'], ['no-elements', 'empty']]) {
      const srv = makeServer();
      const okAnswer = async () => ({ desc: { kind: 'json', file: `bans-${name}.json` }, response: srv.ok(BFX[name].answer) });
      if (route === 'ok') srv.route(url => url.includes('overpass-api.de'), okAnswer);
      if (route === 'second') { failRoute(srv, url => url.includes('overpass-api.de'), { failure: 'http', status: 504 }); srv.route(url => url.includes('mail.ru'), okAnswer); }
      if (route === 'fail') { failRoute(srv, url => url.includes('overpass-api.de'), { failure: 'http', status: 504 }); failRoute(srv, url => url.includes('mail.ru'), { failure: 'unreachable', message: 'Failed to fetch' }); failRoute(srv, url => url.includes('coffee'), { failure: 'unreadable', message: 'Unexpected token' }); failRoute(srv, url => url.includes('kumi'), { failure: 'http', status: 429 }); }
      if (route === 'empty') { literalRoute(srv, url => url.includes('overpass-api.de'), {}); literalRoute(srv, url => url.includes('mail.ru'), { elements: 'x' }); srv.route(url => url.includes('coffee'), okAnswer); }
      const o = open(name, { fetchImpl: srv.fetchImpl }); o.P = reopen(o.c, BFX[name].text);
      const rec = { project: name, label, bounds: boxOf(boundsOf(o.P.scenarios[0])) };
      try { const d = await o.c.Feeds.control.loadBans(boundsOf(o.P.scenarios[0])); rec.data = plain(d); } catch (e) { rec.thrown = e.message; }
      rec.requests = srv.log; rec.feeds = plain(clone(o.c.Feeds.F()));
      out.banLoads.push(rec);
    }
    // proposals and apply, with bans already on some nodes
    for (const variant of ['plain', 'existing', 'closed']) {
      const srv = makeServer(); srv.route(url => url.includes('overpass-api.de'), async () => ({ desc: { kind: 'json', file: `bans-${name}.json` }, response: srv.ok(BFX[name].answer) }));
      const o = open(name, { fetchImpl: srv.fetchImpl }); o.P = reopen(o.c, BFX[name].text);
      const data = await o.c.Feeds.control.loadBans(boundsOf(o.P.scenarios[0]));
      for (const [si, sc] of o.P.scenarios.entries()) {
        if (si > 1) continue;
        const at2 = incident(sc);
        if (variant === 'closed') Object.values(sc.links).forEach((L, i) => { if (i % 4 === 1) L.status = 'closed'; });
        if (variant === 'existing') [...at2].filter(([, ls]) => ls.length >= 3).forEach(([id, ls], i) => { const n = sc.nodes[id]; const x = `${ls[0].id}>${ls[1].id}`; if (i % 3 === 0) n.bans = [x]; if (i % 3 === 1) { n.bans = ['q>r']; n.bansAuto = [x, `${ls[1].id}>${ls[0].id}`]; } if (i % 3 === 2) { n.bansAuto = [x]; } });
        const text = o.c.Store.serialise(o.P);
        const rec = { project: name, variant, scenario: sc.id, startNodes: plain(clone(nodeFields(sc))), startLinks: plain(clone(sc.links)) };
        rec.startFile = `bans-start-${name}-${variant}-${si}.json`; writeFixture(rec.startFile, text);
        const prop = o.c.Feeds.control.bans(sc, data);
        rec.rows = plain(prop.rows); rec.skipped = plain(prop.skipped); rec.noData = plain(o.c.Feeds.control.bans(sc, null));
        for (const [pickLabel, pick] of [['all', () => true], ['some', r => rec.rows.indexOf(r) % 2 === 0]]) {
          const c2 = loadCtx({ seed: 11 }); const P2 = reopen(c2, text); const sc2 = P2.scenarios[si];
          const prop2 = c2.Feeds.control.bans(sc2, clone(data));
          const chosen = rec.rows.filter(pick).map(r => r.id).concat(['unknown']);
          const n = c2.Feeds.control.applyBans(sc2, prop2.rows, chosen, '2026-10-21');
          rec['apply_' + pickLabel] = { ids: chosen, n, nodes: plain(clone(nodeFields(sc2))) };
        }
        out.bans.push(rec);
      }
    }
  }
  return out;
}

/* ====================================================================================================
   tms: NZTA daily counts
   ==================================================================================================== */
const TMS_COUNTS = [
  { id: 'nzta:01N00201', src: 'nzta', ref: '01N00201', name: 'SH1 NB A', road: 'SH1', lat: -37.1, lng: 174.95, adt: 31000, dir: 'NB', directional: true, virtual: false, ramp: false, hist: [30000, 31000, 32500, 33000, 34000] },
  { id: 'nzta:02E00310', src: 'nzta', ref: '02E00310', name: 'SH2 EB B', road: 'SH2', lat: -37.101, lng: 174.951, adt: 12000, dir: 'EB', directional: true, virtual: false, ramp: false, hist: [null, null, 10000, null, 12000] },
  { id: 'nzta:03S00099', src: 'nzta', ref: '03S00099', name: 'SH3 SB C', road: 'SH3', lat: -37.102, lng: 174.952, adt: 8000, dir: 'SB', directional: true, virtual: false, ramp: false, hist: [8000, 8000, 8000, 8000, 8000] },
  { id: 'nzta:04W00007', src: 'nzta', ref: '04W00007', name: 'SH4 WB D', road: 'SH4', lat: -37.103, lng: 174.953, adt: 5000, dir: 'WB', directional: true, virtual: false, ramp: false, hist: [0, 0, 0, 0, 0] },
  { id: 'nzta:05N00555', src: 'nzta', ref: '05N00555', name: 'SH5 NB E', road: 'SH5', lat: -37.104, lng: 174.954, adt: 6000, dir: 'NB', directional: true, virtual: false, ramp: true, hist: [null, null, null, null, 5900] },
  { id: 'nzta:V0000001', src: 'nzta', ref: 'V0000001', name: 'Virtual', road: 'SH1', lat: -37.1, lng: 174.95, adt: 9000, virtual: true, directional: false },
  { id: 'nzta:SHORT', src: 'nzta', ref: 'SHORT', name: 'Short ref', road: 'SH1', lat: -37.1, lng: 174.95, adt: 9000, directional: false },
  { id: 'nzta:noref', src: 'nzta', name: 'No ref', road: 'SH1', lat: -37.1, lng: 174.95, adt: 9000, directional: false },
  { id: 'user:u1', src: 'user', ref: '01N00201', name: 'User with an NZTA ref', lat: -37.1, lng: 174.95, am: 500, directional: false },
  { id: 'at:a1', src: 'at', ref: '0AB1C2D3', name: 'AT with a ref', lat: -37.1, lng: 174.95, adt: 9000, directional: false },
];
function makeTmsRows(c) {
  const F = c.Feeds, dn = F.tms.dnum, di = F.tms.diso, DAY = 86400000;
  const today = dn(new Intl.DateTimeFormat('en-CA', { timeZone: 'Pacific/Auckland', year: 'numeric', month: '2-digit', day: '2-digit' }).format(new Date(NOW)));
  const hol = new Set(); for (let y = 2024; y <= 2026; y++) for (const d of F.tms.holidays(y, 'Auckland')) hol.add(d);
  const rows = [];
  const site = (ref, { days, from = 0, base, hv = 0.08, startAs = 'utc', classes = ['Light', 'Heavy'], lanes = [1, 2], dirs = [1], gaps = [], faulty = [], noise = 0.03 }) => {
    const dowF = [0.7, 1.0, 1.05, 1.07, 1.06, 1.1, 0.85];
    for (let k = from; k < from + days; k++) {
      const n = today - (days - 1 - (k - from)) - (from ? 0 : 0);
      if (gaps.includes(k)) continue;
      const dw = (n + 4) % 7, m = +di(n).slice(5, 7);
      let v = base * dowF[dw] * (1 + 0.05 * Math.sin(m / 12 * 6.28)) * (1 + noise * (rnd() * 2 - 1));
      if (hol.has(n)) v *= 0.65;
      if (faulty.includes(k)) v *= 0.1;
      const t = startAs === 'utc' ? n * DAY : n * DAY - 13 * 3600000 + (m >= 4 && m <= 9 ? 1 * 3600000 : 0);
      for (const lane of lanes) for (const cw of classes) for (const dir of dirs) {
        const share = cw === 'Heavy' ? hv : cw === 'Light' ? 1 - hv : 1, v1 = Math.round(v * share / lanes.length);
        rows.push({ attributes: { SiteRef: ref, startDate: t, laneNumber: lane, flowDirection: dir, classWeight: cw, v: v1 } });
      }
    }
  };
  site('01N00201', { days: 440, base: 31000, startAs: 'nz', faulty: [60, 61, 200], gaps: [100, 101, 102, 250] });
  site('02E00310', { days: 40, base: 12000, dirs: ['E'], lanes: [1] });
  site('03S00099', { days: 200, base: 8000, classes: ['All'], lanes: [1], dirs: [2], noise: 0.08 });
  site('05N00555', { days: 100, base: 6000, lanes: [1], dirs: ['N'], gaps: [5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20], faulty: [30, 31, 40, 41, 42] });
  rows.push({ attributes: { SiteRef: 'ZZ999999', startDate: today * DAY, laneNumber: 1, flowDirection: 1, classWeight: 'Light', v: 100 } });   // a site nobody asked for
  rows.push({ attributes: { siteref: '04W00007', startDate: today * DAY, laneNumber: 1, flowDirection: 1, classWeight: 'Light', v: 5000 } });   // a lower-case field name
  rows.push({ attributes: { SITEREF: '04W00007', startDate: (today - 1) * DAY, laneNumber: 1, flowDirection: 1, classWeight: 'Light', v: null } });
  rows.sort((a, b) => String(a.attributes.SiteRef ?? '').localeCompare(String(b.attributes.SiteRef ?? '')) || a.attributes.startDate - b.attributes.startDate || a.attributes.laneNumber - b.attributes.laneNumber);
  return { features: rows };
}

async function tmsCases() {
  const out = { holidays: [], dates: [], stats: [], loads: [], profiles: [], sites: [] };
  { const c = loadCtx(), F = c.Feeds;
    for (let y = 2010; y <= 2032; y++) for (const region of ['Auckland', 'Wellington', undefined, '', 'North Auckland', 'AUCKLAND']) out.holidays.push({ year: y, region: region === undefined ? { undefined: true } : region, days: [...(region === undefined ? F.tms.holidays(y) : F.tms.holidays(y, region))] });
    for (const iso of ['1970-01-01', '2000-02-29', '2024-02-29', '2026-10-21', '2026-12-31', '2025-04-18', '2022-02-07', '2022-04-14', '2022-04-15', '2026-02-06', '2026-10-12', '2026-12-18', '2026-12-19', '2027-02-02', '2021-06-01', '2019-12-25'])
      out.dates.push({ iso, dnum: F.tms.dnum(iso), diso: F.tms.diso(F.tms.dnum(iso)), inTerm: F.tms.inTerm(F.tms.dnum(iso)) ?? null });
    for (let n = 18900; n < 20800; n += 37) out.dates.push({ n, diso: F.tms.diso(n), inTerm: F.tms.inTerm(n) ?? null });
  }
  const o0 = open('corridor'); const ROWS = makeTmsRows(o0.c);
  writeFixture('tms-rows.json', ROWS);
  const profiles = { source: 'NZTA TMS quarter-hourly archive 2025', built: '2026-09-30', sites: { '01N00201': { amShare: 0.0921, pmShare: 0.1013, amStart: '07:15', pmStart: '16:45', phf: 0.93, hcvPeak: 0.06, k30: 0.11 }, '03S00099': { amShare: 0.088, pmShare: 0.097, amStart: '07:30', pmStart: '17:00' }, '77X00000': { amShare: 0.1, pmShare: 0.1 } } };
  writeFixture('tms-profiles.json', profiles);
  const bySite = ref => ROWS.features.filter(f => (f.attributes.SiteRef ?? f.attributes.siteref ?? f.attributes.SITEREF) === ref).map(f => f.attributes);
  // statistics directly from rows
  { const c = loadCtx(), F = c.Feeds;
    const cases = [
      ['A', bySite('01N00201'), { ref: '01N00201', hist: [30000, 31000, 32500, 33000, 34000] }], ['A-no-hist', bySite('01N00201'), { ref: '01N00201' }], ['B-short', bySite('02E00310'), { ref: '02E00310', hist: [null, null, 10000, null, 12000] }],
      ['C-totals', bySite('03S00099'), { ref: '03S00099', hist: [8000, 8000, 8000, 8000, 8000] }], ['E-gappy', bySite('05N00555'), { ref: '05N00555', hist: [null, null, null, null, 5900] }], ['empty', [], { ref: 'X' }],
      ['A-region', bySite('01N00201'), { ref: '01N00201', region: 'Wellington' }], ['A-zero-hist', bySite('01N00201'), { ref: '01N00201', hist: [0, 0, 0, 0, 0] }], ['A-flat-hist', bySite('01N00201'), { ref: '01N00201', hist: [100, 100, 100, 100, 100] }],
      ['A-one-hist', bySite('01N00201'), { ref: '01N00201', hist: [null, null, null, null, 7] }], ['A-later-now', bySite('01N00201'), { ref: '01N00201', hist: [1, 2, 3, 4, 5], now: Date.UTC(2028, 5, 1) }],
      ['one-row', [{ SiteRef: 'X', startDate: Date.UTC(2026, 9, 14), laneNumber: 1, flowDirection: 'N', classWeight: 'Light', v: 1000 }], { ref: 'X' }],
      ['two-days', [{ SiteRef: 'X', startDate: Date.UTC(2026, 9, 14), laneNumber: 1, flowDirection: 'N', classWeight: 'Light', v: 1000 }, { SiteRef: 'X', startDate: Date.UTC(2026, 9, 15), laneNumber: 1, flowDirection: 'N', classWeight: 'Heavy', v: 100 }], { ref: 'X' }],
      ['odd-values', [{ startDate: Date.UTC(2026, 9, 14), classWeight: 'HEAVY vehicles', v: '50' }, { startDate: Date.UTC(2026, 9, 14), classWeight: 'light', v: 'x' }, { startDate: Date.UTC(2026, 9, 15), classWeight: null, v: 800, flowDirection: 10 }, { startDate: '2026-10-16', classWeight: 'Total', v: 700, flowDirection: 2 }, { startDate: Date.UTC(2026, 9, 17), v: 0, flowDirection: 'b' }], { ref: 'X' }],
      ['bad-date', [{ startDate: 'garbage', v: 1 }], { ref: 'X' }], ['no-date', [{ v: 1 }], { ref: 'X' }],
    ];
    for (const [label, rows, opts] of cases) {
      const rec = { label, rows: label.startsWith('A') || label.startsWith('B') || label.startsWith('C') || label.startsWith('E') ? { fixtureSite: opts.ref } : rows, opts: { ...opts, now: opts.now ?? NOW } };
      try { rec.result = plain(F.tms.stats(rows, { ...opts, now: new Date(opts.now ?? NOW) })); } catch (e) { rec.thrown = e.message; }
      out.stats.push(rec);
    }
  }
  // selecting the sites
  { const o = open('corridor'); o.c.DataHub.D().counts = clone(TMS_COUNTS); out.sites = o.c.Feeds.tms.sites().map(x => x.id); }
  // the whole download
  const run = async (label, { days, feeds, setup, noProfiles = true, fail = null, counts = TMS_COUNTS } = {}) => {
    const srv = makeServer();
    layerRoute(srv, { match: layerPath('TMS_Telemetry_Sites'), file: 'tms-rows.json', features: ROWS.features, page: 700, envelope: false, filter: (f, p) => {
      const m = /SiteRef IN \((.*)\) AND startDate >= DATE '(\d{4}-\d{2}-\d{2})'/.exec(p.get('where') || ''); if (!m) return false;
      const refs = m[1].split(',').map(s => s.replace(/'/g, '')), since = o0.c.Feeds.tms.dnum(m[2]) * 86400000, a = f.attributes, ref = a.SiteRef ?? a.siteref ?? a.SITEREF;
      return refs.includes(ref) && a.startDate >= since - 86400000 * 0;
    }, order: (x, y) => (x.attributes.startDate - y.attributes.startDate) });
    if (fail) srv.routes.unshift({ test: layerPath('TMS_Telemetry_Sites'), answer: failing(srv, fail) });
    const o = open('corridor', { fetchImpl: srv.fetchImpl });
    const d = o.c.DataHub.D(); d.counts = clone(counts);
    if (feeds) o.P.settings.feeds = feeds;
    if (setup) setup(o);
    const st = [], rec = { label, days: days === undefined ? { undefined: true } : days, startCounts: plain(clone(d.counts)), startFeeds: feeds ?? null, startSettingsData: plain(o.P.settings.data) };
    try { const r = await o.c.Feeds.tms.load(o.c.Feeds.tms.sites(), { ...(days === undefined ? {} : { days }), status: t => st.push(t), now: new Date(NOW) }); rec.result = plain(r); } catch (e) { rec.thrown = e.message; }
    rec.statuses = st; rec.requests = srv.log; rec.feeds = plain(clone(o.c.Feeds.F())); rec.counts = plain(clone(d.counts));
    { const c2 = loadCtx({ seed: 11 }); reopen(c2, o.c.Store.serialise(o.P)); rec.attachFresh = c2.Feeds.tms.attach(); rec.countsFresh = plain(clone(c2.DataHub.D().counts)); }
    out.loads.push(rec);
  };
  await run('default'); await run('days-90', { days: 90 }); await run('days-text', { days: 'x' }); await run('days-zero', { days: 0 }); await run('days-negative', { days: -5 }); await run('days-fraction', { days: 28.4 });
  await run('setting-days', { feeds: { tmsDays: 200 } }); await run('setting-text', { feeds: { tmsDays: 'abc' } }); await run('http-error', { fail: { failure: 'http', status: 502 } }); await run('unreachable', { fail: { failure: 'unreachable', message: 'Failed to fetch' } });
  await run('one-site', { counts: TMS_COUNTS.slice(0, 1) }); await run('no-sites', { counts: TMS_COUNTS.slice(5) }); await run('many-sites', { counts: Array.from({ length: 11 }, (_, i) => ({ id: 'nzta:x' + i, src: 'nzta', ref: `0${(i % 9) + 1}N${String(i).padStart(5, '0')}`, name: 'x', adt: 1000, directional: true })).concat(TMS_COUNTS.slice(0, 2)) });
  await run('counts-with-old-fields', { counts: TMS_COUNTS.slice(0, 3).map((c, i) => ({ ...c, obsMethod: 'weekday_factored', ...(i === 1 ? { obs_method: 'weekday_factored', n_days: 5 } : {}), share: { AM: 0.1, PM: 0.1, src: 'old' } })) });
  // peak profiles, with the counts already holding results
  for (const [label, answer] of [['ok', profiles], ['no-sites', { source: 'x' }], ['null', null], ['no-source', { sites: profiles.sites }], ['http', 'fail']]) {
    const srv = makeServer();
    if (answer === 'fail') failRoute(srv, url => url.includes('tms_profiles.json'), { failure: 'http', status: 404 });
    else srv.route(url => url.includes('tms_profiles.json'), async () => ({ desc: { kind: answer && answer.sites && answer.source && answer.built ? 'json' : 'literal', ...(answer && answer.sites && answer.source && answer.built ? { file: 'tms-profiles.json' } : { value: answer }) }, response: srv.ok(answer) }));
    const o = open('corridor', { fetchImpl: srv.fetchImpl }), d = o.c.DataHub.D(); d.counts = clone(TMS_COUNTS);
    o.c.Feeds.F().tms = { '01N00201': { target: { mean: 31000, n: 30, ok: true } }, '02E00310': { target: { mean: 11000, n: 3, ok: false } }, '03S00099': { target: null } };
    const rec = { label };
    try { rec.n = await o.c.Feeds.tms.loadProfiles(); } catch (e) { rec.thrown = e.message; }
    rec.requests = srv.log; rec.feeds = plain(clone(o.c.Feeds.F())); rec.counts = plain(clone(d.counts));
    { const c2 = loadCtx({ seed: 11 }); reopen(c2, o.c.Store.serialise(o.P)); rec.attachFresh = c2.Feeds.tms.attach(); rec.countsFresh = plain(clone(c2.DataHub.D().counts)); }
    out.profiles.push(rec);
  }
  return out;
}

/* ====================================================================================================
   ags: AGS23 growth
   ==================================================================================================== */
function makeAgs(sc) {
  const bb = bboxOf(sc), cy = (bb.s + bb.n) / 2, cx = (bb.w + bb.e) / 2, dy = 0.04, dx = 0.05, f = [];
  const ring = (x0, y0, x1, y1) => [[r7(x0), r7(y0)], [r7(x0), r7(y1)], [r7(x1), r7(y1)], [r7(x1), r7(y0)], [r7(x0), r7(y0)]];
  const attrs = (i, k) => {
    const a = { OBJECTID: 1 + f.length, MSM: 100 + f.length, LocalBoard: ['Papakura', 'Franklin', 'Manurewa', 'Otara-Papatoetoe', ''][i % 5] };
    const base = { hh: 800 + 150 * (i % 7), emp: 400 + 90 * (i % 5), pop: 2400 + 400 * (i % 6) }, g = { hh: 0.01 + 0.002 * (i % 4), emp: 0.008 + 0.003 * (i % 3), pop: 0.012 - 0.001 * (i % 5) };
    for (let y = 2022; y <= 2052; y++) for (const q of ['hh', 'emp', 'pop']) a[`${q.toUpperCase()}_${y}`] = Math.round(base[q] * (1 + g[q] * (y - 2022)));
    if (k === 'zero-hh') for (let y = 2022; y <= 2052; y++) a[`HH_${y}`] = 0;
    if (k === 'null-2031') { a.HH_2031 = null; a.EMP_2031 = null; }
    if (k === 'text') { a.HH_2031 = '1234'; a.POP_2031 = 'abc'; }
    if (k === 'declining') for (let y = 2022; y <= 2052; y++) { a[`HH_${y}`] = 1000 - (y - 2022) * 20; a[`EMP_${y}`] = 500 - (y - 2022) * 10; }
    return a;
  };
  let i = 0;
  for (let r = -3; r < 3; r++) for (let q = -3; q < 3; q++, i++) {
    const k = i === 7 ? 'zero-hh' : i === 12 ? 'null-2031' : i === 15 ? 'text' : i === 20 ? 'declining' : '';
    f.push({ attributes: attrs(i, k), geometry: { rings: [ring(cx + q * dx, cy + r * dy, cx + (q + 1) * dx, cy + (r + 1) * dy)] } });
  }
  f.push({ attributes: { ...attrs(40, ''), MSM: 'X1', LocalBoard: 'Overlap' }, geometry: { rings: [ring(cx - 0.02, cy - 0.015, cx + 0.02, cy + 0.015)] } });                                   // overlaps the cells under the network
  f.push({ attributes: { ...attrs(41, ''), msm: 77, MSM: undefined }, geometry: { rings: [ring(cx + 0.2, cy + 0.2, cx + 0.25, cy + 0.24), ring(cx + 0.21, cy + 0.21, cx + 0.22, cy + 0.22).reverse()] } });   // lower-case msm key, with a hole
  f.push({ attributes: { OBJECTID: 999, MSM: 999 } });                                                      // no geometry
  f.push({ attributes: { OBJECTID: 998 }, geometry: { x: 174.9, y: -37.1 } });                              // a point, not an area
  return { features: f };
}
const AGS_FIELDS = ['OBJECTID', 'MSM', 'LocalBoard', ...Array.from({ length: 31 }, (_, i) => 2022 + i).flatMap(y => [`HH_${y}`, `EMP_${y}`, `POP_${y}`])];

async function agsCases() {
  const out = { loads: [], factors: [], proposals: [], gateways: [] };
  const FX = {};
  for (const name of NAMES) { const o = open(name); FX[name] = makeAgs(o.sc); writeFixture(`ags-zones-${name}.json`, FX[name]); }
  const metaBody = metaOf('Auckland_Growth_Scenario_v1_1', AGS_FIELDS.map(f => (f === 'MSM' ? f : f)));
  writeFixture('ags-meta.json', metaBody);
  const serve = (srv, name, { meta = true } = {}) => {
    if (meta) jsonRoute(srv, metaPath('Auckland_Growth_Scenario_v1_1'), 'ags-meta.json', metaBody);
    else failRoute(srv, metaPath('Auckland_Growth_Scenario_v1_1'), { failure: 'http', status: 404 });
    // the server sends only the fields asked for
    layerRoute(srv, { match: layerPath('Auckland_Growth_Scenario_v1_1'), file: `ags-zones-${name}.json`, features: FX[name].features, page: 9, fieldsFilter: true });
  };
  const combos = [[[2031], 2026], [[2031, 2041, 2060], 2026], [[2010], 2026], [[], 2026], [[2031], undefined], [['2031'], '2026'], [[2031, 2031, 2041], 2031], [[2052, 2022], 2022], [[2031.6], 2026.4]];
  for (const name of NAMES) for (const meta of [true, false]) for (const [years, base] of combos) {
    if (!meta && years.length !== 1) continue;
    const srv = makeServer(); serve(srv, name, { meta });
    const o = open(name, { fetchImpl: srv.fetchImpl });
    const data = await o.c.Feeds.ags.load(boundsOf(o.sc), years, base);
    out.loads.push({ project: name, meta, years, base: base === undefined ? { undefined: true } : base, bounds: boxOf(boundsOf(o.sc)), requests: srv.log, data: plain({ zones: data.zones, years: data.years }), feeds: plain(clone(o.c.Feeds.F())) });
  }
  // zone factors for points in and out of the zones
  for (const name of NAMES) {
    const srv = makeServer(); serve(srv, name);
    const o = open(name, { fetchImpl: srv.fetchImpl }), b = boundsOf(o.sc), bb = bboxOf(o.sc);
    const data = await o.c.Feeds.ags.load(b, [2031, 2041, 2052], 2026);
    const pts = [[(bb.s + bb.n) / 2, (bb.w + bb.e) / 2], [bb.s, bb.w], [bb.n, bb.e], [bb.s - 0.08, bb.w - 0.1], [bb.n + 0.15, bb.e + 0.1], [bb.s + 0.2, bb.e + 0.215], [bb.s + 0.205, bb.e + 0.255], [0, 0]];
    const rec = { project: name, bounds: boxOf(b), requests: srv.log, cases: [] };
    for (const ll of pts) for (const [base, year] of [[2026, 2031], [2026, 2041], [2026, 2060], [2010, 2031], [2026, 2026], [2026, 2022], [2031, 2026], ['2026', '2031'], [undefined, 2031], [2026, undefined]]) rec.cases.push({ ll, base: base === undefined ? { undefined: true } : base, year: year === undefined ? { undefined: true } : year, f: plain(o.c.Feeds.ags.zoneFactor(ll, base, year, data)) });
    rec.noData = plain(o.c.Feeds.ags.zoneFactor(pts[0], 2026, 2031, null));
    out.factors.push(rec);
  }
  // proposals, applying, and growth beyond gateways, with the scenarios in several years
  const variants = [
    ['years', P => { P.scenarios.forEach((s, i) => { s.year = [2026, 2031, 2041, 2060, 2010][i % 5]; }); }], ['default', null], ['base-text', P => { P.settings.baseYear = '2026'; P.scenarios.forEach((s, i) => { s.year = [2031, 2041][i % 2]; }); }],
    ['existing-lu', P => { P.scenarios.forEach((s, i) => { s.year = 2031 + 10 * i; for (const z of Object.values(s.zones)) if (z.kind === 'zone') { delete z.existing; z.src = 'existing-lu'; } }); }],
    ['edited', P => { P.scenarios.forEach(s => { s.year = 2036; for (const z of Object.values(s.zones)) if (z.kind === 'zone') { z.existing = true; z.luBase = { ...z.lu }; z.lu = Object.fromEntries(Object.entries(z.lu).map(([k, v]) => [k, v * 2])); z.ags = { msm: '1', lu: Object.fromEntries(Object.keys(z.lu).map((k, j) => [k, j ? z.lu[k] : z.lu[k] + 1])) }; } }); }],
    ['no-lu', P => { P.scenarios.forEach(s => { s.year = 2031; for (const z of Object.values(s.zones)) if (z.kind === 'zone') { z.existing = true; delete z.lu; } }); }],
  ];
  for (const name of NAMES) for (const [label, setup] of variants) {
    const srv = makeServer(); serve(srv, name);
    const o = open(name, { fetchImpl: srv.fetchImpl });
    if (label !== 'existing-lu' && label !== 'edited' && label !== 'no-lu') for (const s of o.P.scenarios) for (const z of Object.values(s.zones)) if (z.kind === 'zone' && !z.existing) z.existing = true;
    if (setup) setup(o.P);
    const S = o.P.settings, years = o.P.scenarios.map(s => o.c.Engine.yearOf(s, S));
    const data = await o.c.Feeds.ags.load(boundsOf(o.sc), years, S.baseYear);
    const text = o.c.Store.serialise(o.P);
    const rec = { project: name, label, text, years, baseYear: S.baseYear, bounds: boxOf(boundsOf(o.sc)), requests: srv.log };
    rec.rows = plain(o.c.Feeds.ags.proposals(o.P, data));
    for (const [pickLabel, pick] of [['all', () => true], ['some', (r, i) => i % 2 === 0], ['none', () => false]]) {
      const c2 = loadCtx({ seed: 11 }); const P2 = reopen(c2, text);
      const rows = c2.Feeds.ags.proposals(P2, clone(data));
      const chosen = rec.rows.filter((r, i) => pick(r, i)).map(r => r.id).concat(['unknown|row']);
      const n = c2.Feeds.ags.apply(P2, rows, chosen, '2026-10-21');
      rec['apply_' + pickLabel] = { ids: chosen, n, zones: plain(Object.fromEntries(P2.scenarios.map(s => [s.id, clone(s.zones)]))) };
    }
    // gateways: every gateway of every scenario, then one by one with other years
    { const c2 = loadCtx({ seed: 11, now: NOW }); const P2 = reopen(c2, text);
      const gw = c2.Feeds.ags.gateways(P2, clone(data));
      rec.gateways = plain(gw); rec.gatewayZones = plain(Object.fromEntries(P2.scenarios.map(s => [s.id, Object.fromEntries(Object.values(s.zones).filter(z => z.kind === 'gateway').map(z => [z.id, z.agsGrowth ?? null]))])));
      rec.gateway = [];
      for (const sc of P2.scenarios) for (const z of Object.values(sc.zones).filter(z => z.kind === 'gateway')) for (const [base, year] of [[2026, 2031], [2026, 2060], [2026, 2026], [2010, 2031], ['2026', 2036], [2026, undefined]]) rec.gateway.push({ scenario: sc.id, zone: z.id, base: base === undefined ? { undefined: true } : base, year: year === undefined ? { undefined: true } : year, g: plain(c2.Feeds.ags.gateway(sc, z, base, year, clone(data))) });
      rec.gatewayNoData = plain(c2.Feeds.ags.gateway(P2.scenarios[0], Object.values(P2.scenarios[0].zones).find(z => z.kind === 'gateway'), 2026, 2031, null)); }
    out.proposals.push(rec);
  }
  void out.gateways;
  return out;
}

/* ====================================================================================================
   gtfs: bus timetables
   ==================================================================================================== */
// a zip file by hand: members [{ name, data (Buffer), method }]
function zipOf(members, { zip64 = false, badMethodFor = null } = {}) {
  const local = [], central = []; let off = 0;
  for (const m of members) {
    const name = Buffer.from(m.name), raw = Buffer.isBuffer(m.data) ? m.data : Buffer.from(m.data), method = m.method ?? 8;
    const data = method === 8 ? zlib.deflateRawSync(raw) : raw, crc = zlib.crc32(raw);
    const lh = Buffer.alloc(30); lh.writeUInt32LE(0x04034b50, 0); lh.writeUInt16LE(20, 4); lh.writeUInt16LE(0, 6); lh.writeUInt16LE(method, 8); lh.writeUInt32LE(crc, 14); lh.writeUInt32LE(data.length, 18); lh.writeUInt32LE(raw.length, 22); lh.writeUInt16LE(name.length, 26); lh.writeUInt16LE(0, 28);
    local.push(lh, name, data);
    const ch = Buffer.alloc(46); ch.writeUInt32LE(0x02014b50, 0); ch.writeUInt16LE(20, 4); ch.writeUInt16LE(20, 6); ch.writeUInt16LE(0, 8); ch.writeUInt16LE(badMethodFor === m.name ? 12 : method, 10); ch.writeUInt32LE(crc, 16); ch.writeUInt32LE(data.length, 20); ch.writeUInt32LE(raw.length, 24); ch.writeUInt16LE(name.length, 28); ch.writeUInt32LE(off, 42);
    central.push(ch, name);
    off += 30 + name.length + data.length;
  }
  const cd = Buffer.concat(central), e = Buffer.alloc(22);
  e.writeUInt32LE(0x06054b50, 0); e.writeUInt16LE(zip64 ? 0xFFFF : members.length, 8); e.writeUInt16LE(zip64 ? 0xFFFF : members.length, 10); e.writeUInt32LE(cd.length, 12); e.writeUInt32LE(off, 16);
  return Buffer.concat([...local, cd, e]);
}
const csvLine = cells => cells.map(v => { const s = v == null ? '' : String(v); return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s; }).join(',');
const table = (head, rows, { eol = '\n', bom = false, blank = false } = {}) => (bom ? '﻿' : '') + [csvLine(head), ...rows.map(csvLine)].join(eol) + eol + (blank ? eol + eol : '');
const hhmmss = s => `${String(Math.floor(s / 3600)).padStart(2, '0')}:${String(Math.floor(s / 60) % 60).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;

// stops and bus routes along the network's roads: chains of links, the stops 40% along each link, a few metres to the side
function gtfsLayout(c, sc) {
  const { U, Net } = c, F = U.flat(Object.values(sc.nodes)[0].lat);
  const links = Object.values(sc.links).filter(L => L.status !== 'closed' && L.cls !== 'motorway' && L.cls !== 'ramp' && sc.nodes[L.a] && sc.nodes[L.b]);
  const chain = []; const used = new Set(); let cur = links.find(l => l.cls === 'arterial') || links[0];
  while (cur && chain.length < 8) {
    chain.push(cur); used.add(cur.id);
    cur = links.find(l => !used.has(l.id) && (l.a === cur.b || l.b === cur.b || l.a === cur.a) && l.name === cur.name) || links.find(l => !used.has(l.id) && (l.a === cur.b || l.b === cur.b));
  }
  const stopAt = (L, f, side = 6) => { const g = Net.geom(sc, L), cum = U.cumLen(g), len = cum[cum.length - 1], a = U.along(g, cum, len * f), b = U.bearing(g[Math.max(0, a.i)], g[Math.min(g.length - 1, a.i + 1)]) + 90, t = b * Math.PI / 180; return [r7(a.p[0] + side * Math.cos(t) / F.ky), r7(a.p[1] + side * Math.sin(t) / F.kx)]; };
  const stops = chain.map((L, i) => ({ id: `S${100 + i}`, code: `${5000 + i}`, name: `Stop ${i + 1} on ${L.name || 'road'}`, ll: stopAt(L, 0.4) }));
  chain.forEach((L, i) => stops.push({ id: `T${100 + i}`, code: `${6000 + i}`, name: `Opposite ${i + 1}`, ll: stopAt(L, 0.6, -6) }));
  const other = links.filter(l => !used.has(l.id)).slice(0, 5);
  const stopsB = other.map((L, i) => ({ id: `U${100 + i}`, code: `${7000 + i}`, name: `Other ${i + 1}`, ll: stopAt(L, 0.5) }));
  const n0 = Object.values(sc.nodes)[0];
  const far = [{ id: 'F1', code: '9001', name: 'Far away stop, "north" of here', ll: [n0.lat + 0.5, n0.lng] }, { id: 'F2', code: '9002', name: 'Off the road', ll: [stops[0].ll[0] + 0.0009, stops[0].ll[1]] }];
  return { stops: stops.concat(stopsB, far), chain, other };
}
function gtfsFiles(layout, v = {}) {
  const { stops, chain } = layout, ids = stops.map(s => s.id);
  const A = chain.map((_, i) => `S${100 + i}`), Ar = chain.map((_, i) => `T${100 + chain.length - 1 - i}`), B = ids.filter(i => i.startsWith('U')), Fz = ['S100', 'F1', 'S101', 'F2', 'S102'];
  const files = {};
  const stopHead = ['stop_id', 'stop_code', 'stop_name', 'stop_lat', 'stop_lon', 'zone_id'];
  const sH = v.noStopCode ? stopHead.filter(h => h !== 'stop_code') : v.shuffle ? ['zone_id', 'stop_lon', 'stop_name', 'stop_id', 'stop_lat', 'stop_code'] : stopHead;
  const sRow = s => { const m = { stop_id: s.id, stop_code: s.code, stop_name: s.name, stop_lat: s.ll[0], stop_lon: s.ll[1], zone_id: 'Z1' }; return sH.map(h => m[h]); };
  files['stops.txt'] = table(sH, stops.map(sRow), v);
  const routes = [['R1', '25', 'Papakura to Manukau', 3], ['R2', '25X', 'Papakura Express', 3], ['R3', '', 'Airporter Express', 3], ['R4', '314', 'Extended bus', 714], ['R5', 'RAIL', 'Southern Line', 2], ['R6', 'N1', 'Night bus', 700], ['R7', '7', 'Gap', 799], ['R8', '8', 'Bad', 800]];
  const rH = v.shuffle ? ['route_type', 'route_long_name', 'route_id', 'agency_id', 'route_short_name'] : ['route_id', 'agency_id', 'route_short_name', 'route_long_name', 'route_type'];
  files['routes.txt'] = table(rH, routes.map(([id, sn, ln, ty]) => { const m = { route_id: id, agency_id: 'AT', route_short_name: sn, route_long_name: ln, route_type: ty }; return rH.map(h => m[h]); }), v);
  // trips and stop_times
  const trips = [], st = [];
  let tid = 0;
  const addTrips = (route, dir, stopIds, svc, times, head, o = {}) => {
    for (const t0 of times) {
      const id = `${route}-${dir}-${tid++}`;
      trips.push({ route_id: route, service_id: svc, trip_id: id, direction_id: dir, trip_headsign: head });
      let t = t0;
      stopIds.forEach((s, k) => {
        const arr = hhmmss(t), dep = hhmmss(t + 20);
        const row = { trip_id: id, arrival_time: o.depOnly ? '' : o.oneDigit && k === 0 ? ` ${arr.slice(1)}` : arr, departure_time: o.arrOnly ? '' : o.blankMid && k === 2 ? '' : dep, stop_id: s, stop_sequence: k + 1 };
        if (o.blankMid && k === 2) row.arrival_time = '';
        st.push(row); t += 90;
      });
    }
  };
  const peak = []; for (let t = 7 * 3600; t < 9 * 3600; t += 600) peak.push(t);
  const day = []; for (let t = 6 * 3600; t < 21 * 3600; t += 1800) day.push(t);
  const pm = []; for (let t = 16 * 3600; t < 18 * 3600; t += 900) pm.push(t);
  addTrips('R1', 0, A, 'WK', day.concat(peak, pm), 'Manukau');
  addTrips('R1', 1, Ar, 'WK', day.concat(peak), 'Papakura');
  addTrips('R1', 0, A.slice(1, 5), 'WK', [8 * 3600 + 300, 8 * 3600 + 1500, 17 * 3600], 'Takanini', { arrOnly: true });        // a shorter pattern, arrival times only
  addTrips('R2', 0, A.filter((_, i) => i % 2 === 0), 'WK', [7 * 3600 + 900, 8 * 3600 + 900, 17 * 3600 + 900], 'Manukau Express', { oneDigit: true });
  addTrips('R2', 0, A, 'SAT', [9 * 3600, 10 * 3600], 'Saturdays');                                                           // a service that does not run on Tuesdays
  addTrips('R3', 0, Fz, 'WK', [7 * 3600, 16 * 3600 + 1800], 'Airport', { blankMid: true });
  addTrips('R3', 1, B, 'WK', [7 * 3600 + 1200, 8 * 3600 + 600, 16 * 3600 + 600], '');
  addTrips('R4', 0, B, 'WK', [6 * 3600 + 1800, 7 * 3600 + 1800], 'Extended');
  addTrips('R4', 1, B.slice().reverse(), 'EVE', [7 * 3600 + 1800], 'Evening only');
  addTrips('R5', 0, A, 'WK', [7 * 3600], 'Rail');                                                                        // not a bus
  addTrips('R6', 0, A.slice(0, 3), 'WK', [24 * 3600 + 600, 25 * 3600 + 600, 3600], 'Late');                              // times past midnight and a very early one
  addTrips('R7', 0, [B[0], B[1]], 'WK', [7 * 3600 + 300, 7 * 3600 + 3000, 16 * 3600 + 3500], 'Gap Route');
  addTrips('R8', 0, A, 'WK', [7 * 3600], 'Nope');
  addTrips('R1', 0, [A[0]], 'WK', [7 * 3600], 'One stop');                                                               // a pattern of one stop: left out
  addTrips('R1', 0, A, 'EXP', [7 * 3600 + 30], 'Expired');                                                               // an expired service
  addTrips('NOROUTE', 0, A, 'WK', [7 * 3600], 'x');                                                                      // a route that is not listed
  const tH = v.noDir ? ['route_id', 'service_id', 'trip_id'] : v.noHeadsign ? ['route_id', 'service_id', 'trip_id', 'direction_id'] : ['route_id', 'service_id', 'trip_id', 'trip_headsign', 'direction_id'];
  files['trips.txt'] = table(tH, trips.map(t => tH.map(h => t[h])), v);
  const stH = ['trip_id', 'arrival_time', 'departure_time', 'stop_id', 'stop_sequence'];
  // stop_times unordered within a trip: reversed rows for every third trip
  const order = []; const byTrip = new Map(); st.forEach(r => { if (!byTrip.has(r.trip_id)) byTrip.set(r.trip_id, []); byTrip.get(r.trip_id).push(r); });
  [...byTrip.values()].forEach((rows, i) => order.push(...(i % 3 === 1 ? rows.slice().reverse() : rows)));
  files['stop_times.txt'] = table(stH, order.map(r => stH.map(h => r[h])), v);
  const dow = ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday'];
  const calRows = [['WK', 1, 1, 1, 1, 1, 0, 0, '20260101', v.calEnd || '20261231'], ['SAT', 0, 0, 0, 0, 0, 1, 0, '20260101', '20261231'], ['EXP', 1, 1, 1, 1, 1, 0, 0, '20250101', '20250630']];
  if (v.mondaysOnly) calRows[0] = ['WK', 1, 0, 1, 1, 1, 0, 0, '20260101', '20261231'];
  if (!v.noCalendar) files['calendar.txt'] = table(['service_id', ...dow, 'start_date', 'end_date'], calRows, v);
  const cdRows = v.noEve || v.removeTue ? [] : [['EVE', '20261027', 1]];
  if (v.removeTue) cdRows.push(['WK', '20261027', 2], ['WK', '20261103', 2], ['WK', '20261110', 1], ['WK', '20261110', 2], ['WK', '20261110', 1]);
  if (v.noCalendar) cdRows.push(['WK', '20261027', 1], ['WK', '20261103', 1], ['WK', '20261103', 2], ['WK', '20261110', 1], ['WK', '20261117', 1], ['WK', '20261117', 3]);
  files['calendar_dates.txt'] = table(['service_id', 'date', 'exception_type'], cdRows, v);
  if (!v.noFeedInfo) files['feed_info.txt'] = table(['feed_publisher_name', 'feed_publisher_url', 'feed_lang', 'feed_start_date', 'feed_end_date', 'feed_version'], [['Auckland Transport', 'https://at.govt.nz', 'en', '20260101', '20261231', v.version ?? 'v2026-10-15']], v);
  files['agency.txt'] = table(['agency_id', 'agency_name', 'agency_url', 'agency_timezone'], [['AT', 'Auckland Transport', 'https://at.govt.nz', 'Pacific/Auckland'], ['XX', 'Second, "Agency"', '', '']], v);
  if (v.noPublisher) files['feed_info.txt'] = table(['feed_lang'], [['en']], v);
  return files;
}
function gtfsZip(files, o = {}) {
  const names = Object.keys(files);
  return zipOf(names.map((n, i) => ({ name: (o.folder ? 'feed/' : '') + n, data: files[n], method: o.stored ? 0 : (i % 5 === 4 ? 0 : 8) })), o);
}

const CAT_HEAD = ['id', 'data_type', 'entity_type', 'location.country_code', 'location.subdivision_name', 'location.municipality', 'provider', 'name', 'note', 'feed_contact_email', 'static_reference', 'urls.direct_download', 'urls.authentication_type', 'urls.authentication_info', 'urls.api_key_parameter_name', 'urls.latest', 'urls.license', 'location.bounding_box.minimum_latitude', 'location.bounding_box.maximum_latitude', 'location.bounding_box.minimum_longitude', 'location.bounding_box.maximum_longitude', 'status'];
function makeCatalogue(bb) {
  const row = (id, type, prov, name, box, o = {}) => { const m = { id, data_type: type, 'location.country_code': o.cc ?? 'NZ', provider: prov, name, 'urls.latest': o.latest ?? `https://files.mobilitydatabase.org/${id}/latest.zip`, 'urls.license': o.lic ?? 'https://at.govt.nz/licence', status: o.status ?? 'active', 'location.bounding_box.minimum_latitude': box[0], 'location.bounding_box.minimum_longitude': box[1], 'location.bounding_box.maximum_latitude': box[2], 'location.bounding_box.maximum_longitude': box[3] }; return CAT_HEAD.map(h => m[h]); };
  const rows = [
    row('mdb-1029', 'gtfs', 'Auckland Transport', 'AT GTFS', [-44.1, 172.0, -35.9, -176.0].map((v, i) => (i === 3 ? 175.9 : v))),
    row('mdb-9999', 'gtfs', 'Small Council Shuttle', 'Shuttle', [bb.s - 0.01, bb.w - 0.01, bb.n + 0.01, bb.e + 0.01], { lic: '' }),
    row('mdb-8888', 'gtfs', 'Wide Regional, "Bus" Co', 'Regional', [-38, 174, -36, 176], { latest: '' }),
    row('mdb-1132', 'gtfs', 'Metlink', 'Wellington', [-41.65, 174.55, -40.6, 176.1]),
    row('mdb-7777', 'gtfs-rt', 'Auckland Transport', 'Realtime', [-37.45, 174.15, -36.05, 175.6]),
    row('mdb-6666', 'gtfs', 'Old Operator', 'Old', [-37.45, 174.15, -36.05, 175.6], { status: 'inactive' }),
    row('mdb-5555', 'gtfs', 'Deprecated', 'No status', [-37.45, 174.15, -36.05, 175.6], { status: '' }),
    row('mdb-4444', 'gtfs', 'Broken box', 'Broken', ['x', 174.15, -36.05, 175.6]),
    row('mdb-3333', 'gtfs', 'Aussie Feed', 'AU', [-35.0, 144.0, -33.0, 146.0], { cc: 'AU' }),
    row('tld-2222', 'gtfs', 'Same size A', 'A', [-37.5, 174.0, -36.0, 175.5]), row('tld-2223', 'gtfs', 'Same size B', 'B', [-37.5, 174.0, -36.0, 175.5]),
  ];
  return table(CAT_HEAD, rows);
}

async function gtfsCases() {
  const out = { catalogues: [], parses: [], loads: [], routes: [], errors: [] };
  const o0 = open('corridor'), o1 = open('grid');
  const LAY = { corridor: gtfsLayout(o0.c, o0.sc), grid: gtfsLayout(o1.c, o1.sc) };
  // catalogue
  for (const name of NAMES) {
    const o = open(name), bb = bboxOf(o.sc), csv = makeCatalogue(bb);
    writeFixture(`gtfs-catalogue-${name}.csv`, csv);
    const mk = (label, answerFn, b, { twice = false, cfg = null } = {}) => ({ label, answerFn, b, twice, cfg });
    const cases = [mk('covers', srv => srv.route(u => u.includes('feeds_v2.csv'), async () => ({ desc: { kind: 'text', file: `gtfs-catalogue-${name}.csv` }, response: srv.ok(null, { text: csv }) })), boundsOf(o.sc)),
      mk('twice', srv => srv.route(u => u.includes('feeds_v2.csv'), async () => ({ desc: { kind: 'text', file: `gtfs-catalogue-${name}.csv` }, response: srv.ok(null, { text: csv }) })), boundsOf(o.sc), { twice: true }),
      mk('time-limit-60', srv => srv.route(u => u.includes('feeds_v2.csv'), async () => ({ desc: { kind: 'text', file: `gtfs-catalogue-${name}.csv` }, response: srv.ok(null, { text: csv }) })), boundsOf(o.sc), { cfg: { timeout: 60 } }),
      mk('far-away', srv => srv.route(u => u.includes('feeds_v2.csv'), async () => ({ desc: { kind: 'text', file: `gtfs-catalogue-${name}.csv` }, response: srv.ok(null, { text: csv }) })), { getSouth: () => -46.2, getWest: () => 168.5, getNorth: () => -46.1, getEast: () => 168.7, south: -46.2, west: 168.5, north: -46.1, east: 168.7 }),
      mk('http-error', srv => failRoute(srv, u => u.includes('feeds_v2.csv'), { failure: 'http', status: 503 }), boundsOf(o.sc)),
      mk('unreachable', srv => failRoute(srv, u => u.includes('feeds_v2.csv'), { failure: 'unreachable', message: 'Failed to fetch' }), boundsOf(o.sc)),
      mk('empty-file', srv => srv.route(u => u.includes('feeds_v2.csv'), async () => ({ desc: { kind: 'literal-text', value: '' }, response: srv.ok(null, { text: '' }) })), boundsOf(o.sc)),
      mk('header-only', srv => srv.route(u => u.includes('feeds_v2.csv'), async () => ({ desc: { kind: 'literal-text', value: CAT_HEAD.join(',') + '\n' }, response: srv.ok(null, { text: CAT_HEAD.join(',') + '\n' }) })), boundsOf(o.sc)),
      mk('no-columns', srv => srv.route(u => u.includes('feeds_v2.csv'), async () => ({ desc: { kind: 'literal-text', value: 'a,b\n1,2\n' }, response: srv.ok(null, { text: 'a,b\n1,2\n' }) })), boundsOf(o.sc))];
    for (const k of cases) {
      const srv = makeServer(); k.answerFn(srv);
      const o2 = open(name, { fetchImpl: srv.fetchImpl }); if (k.cfg) o2.P.settings.feeds = k.cfg;
      const rec = { project: name, label: k.label, bounds: boxOf(k.b), cfg: k.cfg };
      let r = await o2.c.Feeds.gtfs.catalogue(k.b); if (k.twice) r = await o2.c.Feeds.gtfs.catalogue(k.b);
      rec.result = plain(r); rec.requests = srv.log; rec.feeds = plain(clone(o2.c.Feeds.F()));
      out.catalogues.push(rec);
    }
  }
  // the feeds: parse the zip directly and through the whole download; routes on the network
  const variants = [
    ['normal', {}, {}], ['folder', {}, { folder: true }], ['holiday-exception', { removeTue: true }, {}], ['stored', {}, { stored: true }], ['no-calendar', { noCalendar: true }, {}], ['quirks', { eol: '\r\n', bom: true, blank: true, shuffle: true, noDir: false }, {}],
    ['no-stop-code', { noStopCode: true, noHeadsign: true }, {}], ['no-dir', { noDir: true, noFeedInfo: true }, {}], ['stale', { calEnd: '20260630', noEve: true }, {}], ['school-holidays', {}, { now: Date.UTC(2026, 9, 1) }], ['other-day', {}, { now: Date.UTC(2026, 9, 27) }], ['no-publisher', { noPublisher: true }, {}],
    ['year-end', {}, { now: Date.UTC(2026, 11, 20) }], ['christmas-closed', { removeTue: true }, { now: Date.UTC(2026, 11, 22) }],
  ];
  for (const name of NAMES) for (const [label, v, o] of variants) {
    const layout = LAY[name], files = gtfsFiles(layout, v), zip = gtfsZip(files, o);
    const fname = `gtfs-${name}-${label}.zip`; writeFixture(fname, zip);
    const now = o.now ?? NOW, feed = { id: 'mdb-1029', provider: 'Auckland Transport', name: 'AT GTFS', latest: 'https://files.mobilitydatabase.org/mdb-1029/latest.zip', licence: 'https://at.govt.nz/licence' };
    const srv = makeServer();
    const lm = 'Wed, 14 Oct 2026 02:30:00 GMT';
    srv.route((u, rec) => u.includes('mdb-1029/latest.zip') && rec.method === 'HEAD', async () => ({ desc: { kind: 'head', lastModified: lm }, response: srv.ok(null, { lastModified: lm }) }));
    srv.route((u, rec) => u.includes('mdb-1029/latest.zip') && rec.method === 'GET', async () => ({ desc: { kind: 'bytes', file: fname, lastModified: lm }, response: srv.ok(null, { buf: zip.buffer.slice(zip.byteOffset, zip.byteOffset + zip.length), lastModified: lm }) }));
    const workerLog = [];
    const c = loadCtx({ seed: 11, now, fetchImpl: srv.fetchImpl, workerLog }); const P = c.migrate(JSON.parse(readFileSync(join(here, 'inputs', name + '.json'), 'utf8'))); c.Store.P = P;
    const b = boundsOf(P.scenarios[0]), st = [];
    const rec = { project: name, label, file: fname, now, feed, bounds: boxOf(b) };
    let g = null;
    try { g = await c.Feeds.gtfs.load(feed, b, { now: new Date(now), status: t => st.push(t) }); rec.loaded = plain({ ...g }); } catch (e) { rec.thrown = e.message; }
    rec.opts = workerLog[0] ?? null; rec.statuses = st; rec.requests = srv.log;
    if (rec.loaded) {
      rec.routes = [];
      for (const sc of P.scenarios.slice(0, 2)) {
        const r = c.Feeds.gtfs.routes(sc, g);
        const sc2 = clone(sc.routes); sc.routes = { oldGtfs: { id: 'oldGtfs', name: 'Old', src: 'GTFS', freq: 3, wp: [] }, mine: { id: 'mine', name: 'Mine', freq: 4, wp: [] } };
        const n = c.Feeds.gtfs.apply(sc, r);
        rec.routes.push({ scenario: sc.id, routes: plain(r), applied: n, scRoutes: plain(clone(sc.routes)), before: plain(sc2) });
      }
      c.Feeds.gtfs.keep(g, c.Feeds.gtfs.routes(P.scenarios[0], g)); rec.feeds = plain(clone(c.Feeds.F()));
      c.Feeds.gtfs.keep(g, []); rec.feedsNoRoutes = plain(clone(c.Feeds.F()));
    }
    out.loads.push(rec);
  }
  // errors: files that cannot be read
  const good = gtfsFiles(LAY.corridor, {}), mkErr = (label, buf, extra = {}) => { writeFixture(`gtfs-error-${label}.zip`, buf); return { label, file: `gtfs-error-${label}.zip`, ...extra }; };
  const errs = [
    mkErr('not-a-zip', Buffer.from('<html><body>Not Found</body></html>')), mkErr('empty', Buffer.alloc(0)), mkErr('zip64', gtfsZip(good, { zip64: true })),
    mkErr('no-stops', gtfsZip(Object.fromEntries(Object.entries(good).filter(([k]) => k !== 'stops.txt')))), mkErr('no-routes', gtfsZip(Object.fromEntries(Object.entries(good).filter(([k]) => k !== 'routes.txt')))), mkErr('no-trips', gtfsZip(Object.fromEntries(Object.entries(good).filter(([k]) => k !== 'trips.txt')))),
    mkErr('no-stop-times', gtfsZip(Object.fromEntries(Object.entries(good).filter(([k]) => k !== 'stop_times.txt')))), mkErr('no-calendar-at-all', gtfsZip(Object.fromEntries(Object.entries(good).filter(([k]) => k !== 'calendar.txt' && k !== 'calendar_dates.txt')))),
    mkErr('bad-method', zipOf(Object.entries(good).map(([name, data]) => ({ name, data, method: 8 })), { badMethodFor: 'routes.txt' })), mkErr('mondays-only', gtfsZip(gtfsFiles(LAY.corridor, { mondaysOnly: true, noEve: true }))),
    mkErr('zip-with-junk-before', Buffer.concat([Buffer.from('JUNKJUNKJUNK'), gtfsZip(good)])), mkErr('empty-members', gtfsZip({ 'stops.txt': '', 'routes.txt': 'route_id\n', 'trips.txt': 'trip_id\n', 'stop_times.txt': '', 'calendar.txt': 'service_id\n' })),
  ];
  const optsFor = { box: [-37.2, 174.8, -37.0, 175.1], today: '20261021', holidays: ['20261026'], terms: [['20261012', '20261218']], am: [7, 9], pm: [16, 18] };
  for (const e of errs) {
    const buf = readFileSync(join(dataDir, e.file)); const c = loadCtx();
    try { const r = await c.Feeds.gtfs.parse(new Uint8Array(buf), optsFor); e.result = plain(r); e.result.ms = 0; } catch (x) { e.thrown = x.message; }
    e.opts = optsFor; out.errors.push(e);
  }
  // parse the good feed with other options directly: windows, boxes, terms and holidays
  for (const [label, o] of [['am-only-hours', { am: [6, 10], pm: [14, 19] }], ['small-box', { box: [-37.1, 174.94, -37.09, 174.96] }], ['no-terms', { terms: [] }], ['terms-other-year', { terms: [['20271012', '20271218']] }], ['holiday-on-tuesday', { holidays: ['20261027'] }], ['today-bad', { today: 'xxxx' }], ['wide-box', { box: [-38, 174, -36, 176] }], ['empty-box', { box: [0, 0, 1, 1] }], ['only-past', { today: '20270301' }]]) {
    const buf = gtfsZip(good, {}); const c = loadCtx(); const o2 = { ...optsFor, ...o };
    const rec = { label, opts: o2, file: 'gtfs-corridor-normal.zip' };
    try { const r = await c.Feeds.gtfs.parse(new Uint8Array(buf), o2); rec.result = plain(r); rec.result.ms = 0; } catch (x) { rec.thrown = x.message; }
    out.parses.push(rec);
  }
  return out;
}

/* ====================================================================================================
   council: ArcGIS count layers of other councils
   ==================================================================================================== */
const HAM = 'https://services1.arcgis.com/R6s0QqCMQdwKY6yp/arcgis/rest/services/Hamilton%20City%20Traffic%20Counts/FeatureServer/0';
const QLDC = 'https://services1.arcgis.com/9YyqaQtDdDR8tupG/arcgis/rest/services/Traffic_by_Site_Latest_Counts/FeatureServer/0';
const GEN = 'https://gis.example.govt.nz/arcgis/rest/services/Roads/Counts/MapServer/3';
function makeCouncilLayers(sc) {
  const bb = bboxOf(sc), at = () => ({ x: r7(uni(bb.w, bb.e)), y: r7(uni(bb.s, bb.n)) });
  const ham = [], qldc = [], gen = [];
  const D = (y, m, d) => Date.UTC(y, m, d);
  for (let i = 0; i < 14; i++) ham.push({ attributes: { OBJECTID: 40 + i, Road_Name: pick(['Victoria Street', 'Anglesea St', 'Ulster Road NB', 'Bridge Rd SB', null]), AADT: [12000, 0, 8800, null, 15000.6, '7200', 4100, 9000, 0, 3300, 6100, 2500, 18000, 700][i], Count_Date: [D(2025, 4, 6), D(2024, 8, 1), D(2023, 2, 2), null, D(2019, 0, 1), D(2025, 10, 5), D(2022, 6, 9), '2021-05-04', 'n/a', D(2025, 5, 1), D(2018, 0, 1), D(2026, 0, 3), D(2020, 3, 3), D(2017, 4, 4)][i], Peak_Hour: ['0745', '1700', '745', '5', '', null, '17:30', '8', '1530', '0600', '1000', '1430', '1859', '0630'][i], Peak_Volume: [1100, 900, 0, 800, '650', null, 1200, 400, 77, 300, 500, 250, 1500, 70][i], Pct_Heavy: [5.2, 8, null, 'x', 3, '', 4, 6, 7, 1, 0, 2.5, 9, null][i], Direction: ['NB', 'both', 'sb', 'Eastbound EB', null, '', 'WB lane', 'nb/sb', 'N', 'EB', 'x', 'NB', 'WB', 'SB'][i] }, geometry: at() });
  for (let i = 0; i < 9; i++) qldc.push({ attributes: { OBJECTID: 700 + i, Site_Desc: pick(['Frankton Rd', 'Man St', "Queenstown Hill Rd"]), Latest: ['Y', 'Yes', 'YES', 'y', 'N', 'No', null, 'Y', 'Y'][i], Year_2022: [5000, 0, null, 3000, 100, 9000, 800, null, 200][i], Year_2023: [5200, 4000, 0, null, 100, 9100, null, 1500, 250][i], Year_2024: [0, 4100, null, 3300, 100, 9300, 900, 1600, 300][i], Heavy_pct: pick([5, 8, null]) }, geometry: { paths: [[[r7(uni(bb.w, bb.e)), r7(uni(bb.s, bb.n))], [r7(uni(bb.w, bb.e)), r7(uni(bb.s, bb.n))], [r7(uni(bb.w, bb.e)), r7(uni(bb.s, bb.n))]], [[r7(uni(bb.w, bb.e)), r7(uni(bb.s, bb.n))]]] } });
  for (let i = 0; i < 8; i++) gen.push({ attributes: { OBJECTID: i === 5 ? null : 9000 + i, SITE_DESCRIPTION: `Site ${i}`, VPD: [1000, '2000', 0, null, 500, 800, 1200, 40][i], SURVEY_DATE: ['2025-02-03', D(2024, 1, 1), '2012', null, 'last year', D(2026, 6, 6), D(2000, 0, 1), '2023-03-09T00:00:00Z'][i], DIRECTION_OF_TRAVEL: ['EB', 'N', 'WB', null, 'sb', 'NB', 'EB', ''][i], PEAK_HOUR_VOL: [90, 0, null, 70, 60, 50, 40, 30][i], PEAK_TIME: ['0815', 7, '17:00', null, '', 1830, '0700', 'x'][i], PC_HV: [4, 5, 6, 7, null, 'abc', 0, 3][i] }, geometry: i % 3 === 0 ? { points: [[r7(uni(bb.w, bb.e)), r7(uni(bb.s, bb.n))], [0, 0]] } : i === 7 ? {} : at() });
  gen.push({ attributes: { OBJECTID: 9100, VPD: 5000 } });
  return { ham: { features: ham }, qldc: { features: qldc }, gen: { features: gen } };
}
const metaHam = metaOf('Hamilton City Traffic Counts', [{ name: 'OBJECTID', type: 'esriFieldTypeOID', alias: 'OBJECTID' }, { name: 'Road_Name', type: 'esriFieldTypeString', alias: 'Road name' }, { name: 'AADT', type: 'esriFieldTypeInteger', alias: 'AADT' }, { name: 'Count_Date', type: 'esriFieldTypeDate', alias: 'Count date' }, { name: 'Peak_Hour', type: 'esriFieldTypeString', alias: 'Peak hour' }, { name: 'Peak_Volume', type: 'esriFieldTypeInteger', alias: 'Peak volume' }, { name: 'Pct_Heavy', type: 'esriFieldTypeDouble', alias: 'Heavy %' }, { name: 'Direction', type: 'esriFieldTypeString', alias: 'Direction' }, { name: 'Shape', type: 'esriFieldTypeGeometry', alias: 'Shape' }, { name: 'GlobalID', type: 'esriFieldTypeGlobalID', alias: 'GlobalID' }]);
const metaQ = metaOf('Traffic by Site Latest Counts', ['OBJECTID', 'Site_Desc', 'Latest', 'Year_2022', 'Year_2023', 'Year_2024', 'year_2021', 'Heavy_pct', { name: 'Geometry', type: 'esriFieldTypeGeometry', alias: '' }]);
const metaG = metaOf('Counts', [{ name: 'OBJECTID', type: 'esriFieldTypeOID' }, { name: 'SITE_DESCRIPTION', type: 'esriFieldTypeString', alias: 'Site' }, { name: 'VPD', type: 'esriFieldTypeInteger' }, { name: 'SURVEY_DATE', type: 'esriFieldTypeString' }, { name: 'DIRECTION_OF_TRAVEL', type: 'esriFieldTypeString' }, { name: 'PEAK_HOUR_VOL', type: 'esriFieldTypeInteger' }, { name: 'PEAK_TIME', type: 'esriFieldTypeString' }, { name: 'PC_HV', type: 'esriFieldTypeDouble', alias: 'Percent heavy vehicles' }]);

async function councilCases() {
  const out = { urls: [], guesses: [], reads: [], imports: [] };
  { const c = loadCtx(), F = c.Feeds;
    for (const u of [HAM, ' ' + HAM + '  ', HAM + '/', HAM + '/query?where=1%3D1', HAM + '?f=json', HAM + '/QUERY', QLDC + '///', GEN, GEN + '/query', 'http://x.example/rest/Layer/FeatureServer/0', 'https://x.example/rest/Layer/FeatureServer', 'https://x.example/FeatureServer/0', 'https://x/a/FeatureServer/0', 'https://x.example/a/b/featureserver/12', 'ftp://x.example/a/FeatureServer/0', '', undefined, null, 'https://x.example/a/FeatureServer/0/query?x=1', 'https://x.example/a/MapServer/0/queryRelatedRecords', 'https://x.example/a/FeatureServer/0 /'])
      { const rec = { url: u === undefined ? { undefined: true } : u }; const srv = makeServer(); jsonRoute(srv, () => true, 'x', metaHam); const c2 = loadCtx({ fetchImpl: srv.fetchImpl }); try { const r = await c2.Feeds.council.read(u); rec.cleaned = r.url; } catch (e) { rec.thrown = e.message; } rec.requests = srv.log; out.urls.push(rec); }
    for (const [label, meta] of [['ham', metaHam], ['qldc', metaQ], ['gen', metaG], ['empty', { fields: [] }], ['none', {}], ['odd', metaOf('Odd', [{ name: 'adt_total', alias: 'Average Daily Traffic' }, { name: 'Peak', alias: 'peak hour volume' }, { name: 'OBJECTID_1', type: 'esriFieldTypeOID' }, { name: 'year_2019' }, { name: 'YEAR2021', type: 'x' }, { name: 'year_20255' }, { name: 'dir', alias: 'x' }, { name: 'Date_of_count' }, { name: 'Street_Name' }, { name: 'LocationID', alias: 'Location' }])]]) out.guesses.push({ label, meta, guess: plain(F.council.guess(meta)) });
  }
  const FX = {}; for (const name of NAMES) { const o = open(name); FX[name] = makeCouncilLayers(o.sc); writeFixture(`council-${name}-ham.json`, FX[name].ham); writeFixture(`council-${name}-qldc.json`, FX[name].qldc); writeFixture(`council-${name}-gen.json`, FX[name].gen); }
  writeFixture('council-meta-ham.json', metaHam); writeFixture('council-meta-qldc.json', metaQ); writeFixture('council-meta-gen.json', metaG);
  const layers = (name) => [[HAM, 'ham', metaHam, null], [QLDC, 'qldc', metaQ, null], [GEN, 'gen', metaG, null]];
  const serve = (srv, name) => {
    for (const [url, key, meta] of layers(name)) {
      const frag = url.replace(/^https:\/\/[^/]+/, '');
      srv.route(u => u.startsWith(url + '?f=json'), async () => ({ desc: { kind: 'json', file: `council-meta-${key}.json` }, response: srv.ok(meta) }));
      layerRoute(srv, { match: u => u.startsWith(url + '/query?'), file: `council-${name}-${key}.json`, features: FX[name][key].features, page: 5, filter: key === 'qldc' ? (f, p) => { const m = /(\w+) IN \('Y','Yes','YES','y'\)/.exec(p.get('where') || ''); return !m || ['Y', 'Yes', 'YES', 'y'].includes(f.attributes[m[1]]); } : null });
      void frag;
    }
  };
  // reading each layer
  for (const name of NAMES) {
    const srv = makeServer(); serve(srv, name);
    const o = open(name, { fetchImpl: srv.fetchImpl });
    for (const [url] of layers(name)) { const r = await o.c.Feeds.council.read(url); out.reads.push({ project: name, url, result: plain({ url: r.url, name: r.name, map: r.map, fields: r.fields }) }); }
    // an error: not a feature layer; a failing layer
    for (const [label, route] of [['http', { failure: 'http', status: 500 }], ['error-body', 'err'], ['no-fields', 'nofields']]) {
      const s2 = makeServer(); if (route === 'err') literalRoute(s2, () => true, { error: { message: 'Layer is gone' } }); else if (route === 'nofields') literalRoute(s2, () => true, { name: 'Just a name' }); else failRoute(s2, () => true, route);
      const o2 = open(name, { fetchImpl: s2.fetchImpl }); const rec = { project: name, url: HAM, label };
      try { await o2.c.Feeds.council.read(HAM); } catch (e) { rec.thrown = e.message; } rec.requests = s2.log; out.reads.push(rec);
    }
  }
  // importing, with the fields the prototype guessed and with the user's own choices (an ordered object)
  const maps = {
    ham: [['guessed', null], ['no-peak', { adt: 'AADT', date: 'Count_Date', name: 'Road_Name', dir: 'Direction' }], ['all-and-order', { years: [], adt: 'AADT', peak: 'Peak_Volume', peakTime: 'Peak_Hour', hcv: 'Pct_Heavy', date: 'Count_Date', name: 'Road_Name', dir: 'Direction', latest: '' }]],
    qldc: [['guessed', null], ['year-columns', { years: ['Year_2024', 'Year_2023', 'Year_2022'], adt: '', peak: '', peakTime: '', hcv: 'Heavy_pct', date: '', name: 'Site_Desc', dir: '', latest: 'Latest' }], ['year-columns-no-latest', { years: ['Year_2024', 'Year_2023'], adt: '', name: 'Site_Desc' }]],
    gen: [['guessed', null], ['custom', { years: [], adt: 'VPD', peak: 'PEAK_HOUR_VOL', peakTime: 'PEAK_TIME', hcv: 'PC_HV', date: 'SURVEY_DATE', name: 'SITE_DESCRIPTION', dir: 'DIRECTION_OF_TRAVEL', latest: '' }]],
  };
  for (const name of NAMES) for (const [url, key] of layers(name)) for (const [mlabel, mp] of maps[key]) for (const [preset, now] of key === 'gen' ? [[null, NOW]] : [[null, NOW], [key === 'ham' ? 'hamilton' : 'qldc', NOW], [key === 'qldc' ? 'qldc' : 'hamilton', Date.UTC(2031, 5, 1)]]) {
    const srv = makeServer(); serve(srv, name);
    const o = open(name, { fetchImpl: srv.fetchImpl, now });
    const d = o.c.DataHub.D(); d.counts = [{ id: 'user:mine', src: 'user', name: 'Mine', lat: -37.1, lng: 174.95, am: 400, directional: false }, { id: 'feeds:old', src: 'user', name: 'Old', lat: -37.1, lng: 174.95, adt: 1, feed: { url, source: 'old' } }, { id: 'feeds:other', src: 'user', name: 'Other layer', lat: -37.1, lng: 174.95, adt: 1, feed: { url: 'https://other.example/x/FeatureServer/0', source: 'other' } }];
    const rd = await o.c.Feeds.council.read(url), st = [];
    const map = mp ?? { ...rd.map };
    const rec = { project: name, url, mapLabel: mlabel, map, preset, now, bounds: boxOf(boundsOf(o.sc)), startCounts: plain(clone(d.counts)) };
    srv.log.length = 0;
    try { rec.result = plain(await o.c.Feeds.council.import(rd, map, boundsOf(o.sc), { preset, now: new Date(now), status: t => st.push(t) })); } catch (e) { rec.thrown = e.message; }
    rec.statuses = st; rec.requests = srv.log; rec.counts = plain(clone(d.counts)); rec.feeds = plain(clone(o.c.Feeds.F()));
    out.imports.push(rec);
  }
  return out;
}

/* ====================================================================================================
   hazards: flood plains and coastal inundation
   ==================================================================================================== */
// a polygon around part of a link (fractions of its length) with a half width in metres, or a fraction box
function bandAround(c, sc, L, f0, f1, half, hole = false) {
  const { U, Net } = c, F = U.flat(Object.values(sc.nodes)[0].lat), g = Net.geom(sc, L), cum = U.cumLen(g), len = cum[cum.length - 1];
  const a = U.along(g, cum, len * f0), b = U.along(g, cum, len * f1), brg = U.bearing(a.p, b.p) + 90, t = brg * Math.PI / 180;
  const dx = half * Math.sin(t) / F.kx, dy = half * Math.cos(t) / F.ky;
  const ring = [[a.p[1] + dx, a.p[0] + dy], [b.p[1] + dx, b.p[0] + dy], [b.p[1] - dx, b.p[0] - dy], [a.p[1] - dx, a.p[0] - dy], [a.p[1] + dx, a.p[0] + dy]].map(p => [r7(p[0]), r7(p[1])]);
  return hole ? ring.slice().reverse() : ring;
}
function makeHazards(c, sc) {
  const links = Object.values(sc.links).filter(l => l.status !== 'closed' && sc.nodes[l.a] && sc.nodes[l.b]), bb = bboxOf(sc);
  const flood = [], coastal = {};
  const fl = (rings, a = {}) => flood.push({ attributes: { OBJECTID: 200 + flood.length, Hazard: 'Flood plain', RAINFALL_EVENT: 100, CLIMATE_CHANGE_ADJUSTED: 'None', Superseded_Date: null, YEAR_PRODUCED: 2020, DOCUMENT_NAME: 'Model', ...a }, geometry: { rings } });
  links.forEach((L, i) => {
    if (i % 3 === 0) fl([bandAround(c, sc, L, 0.2, 0.8, 18)], { RAINFALL_EVENT: [100, 50, 200, 10, null, 'abc', 20][i % 7], CLIMATE_CHANGE_ADJUSTED: ['None', 'RCP8.5 2100', 'No change', 'Yes', null, ' none ', 'no'][i % 7] });
    if (i % 3 === 0) fl([bandAround(c, sc, L, 0.5, 0.95, 14)], { RAINFALL_EVENT: [100, 50, 200, 10, null, 'abc', 20][i % 7], CLIMATE_CHANGE_ADJUSTED: ['None', 'RCP8.5 2100', 'No change', 'Yes', null, ' none ', 'no'][i % 7] });   // overlaps the first: merged, never counted twice
    if (i % 5 === 1) fl([bandAround(c, sc, L, 0.0, 1.0, 25), bandAround(c, sc, L, 0.4, 0.6, 5, true)], { RAINFALL_EVENT: 100, CLIMATE_CHANGE_ADJUSTED: 'Yes' });  // a hole in the middle
    if (i % 7 === 2) fl([bandAround(c, sc, L, 0.1, 0.3, 20)], { Superseded_Date: Date.UTC(2023, 0, 1) });
    if (i % 4 === 3) fl([bandAround(c, sc, L, 0.0, 0.02, 30)], { RAINFALL_EVENT: 100 });  // a sliver under half a metre of a long link
  });
  links.forEach(L => fl([bandAround(c, sc, L, 0.05, 0.95, 16)], { RAINFALL_EVENT: 50, CLIMATE_CHANGE_ADJUSTED: 'None', DOCUMENT_NAME: 'Every road' }));   // every road has some flood
  flood.push({ attributes: { OBJECTID: 900, Hazard: 'x' } }); flood.push({ attributes: { OBJECTID: 901, RAINFALL_EVENT: 5 }, geometry: { x: 1, y: 2 } });
  const name = ['Coastal_Inundation_1_AEP', 'Mean_High_Water_Springs_05m_sea_level_rise', 'Mean_High_Water_Springs_10m_sea_level_rise', 'Mean_High_Water_Springs_15m_sea_level_rise'];
  name.forEach((n, k) => { coastal[n] = { features: [] }; links.forEach((L, i) => { if ((i + k) % 4 === 0) coastal[n].features.push({ attributes: { OBJECTID: 1 + coastal[n].features.length }, geometry: { rings: [bandAround(c, sc, L, 0.1 + 0.1 * k, 0.6 + 0.1 * k, 20 + 5 * k)] } }); }); coastal[n].features.push({ attributes: { OBJECTID: 99 } }); });
  void bb;
  return { flood: { features: flood }, coastal };
}
// the test project with a dead-end spur off the middle of an arterial or collector and a development zone that connects to its end only:
// that spur is the zone's only access
function hazardProject(name) {
  const o = open(name);
  for (const sc of o.P.scenarios) {
    const L1 = Object.values(sc.links).find(l => (l.cls === 'arterial' || l.cls === 'collector') && sc.nodes[l.a] && sc.nodes[l.b]);
    const g = o.c.Net.geom(sc, L1), cum = o.c.U.cumLen(g), mid = o.c.U.along(g, cum, cum[cum.length - 1] / 2).p, F = o.c.U.flat(mid[0]);
    const j = o.c.Net.splitLink(sc, L1.id, mid), x = o.c.Net.addNode(sc, [r7(mid[0] + 60 / F.ky), mid[1]]);
    o.c.Net.addLink(sc, j, x, [], { name: 'Cut spur', cls: 'local' });
    sc.zones.zCut = { id: 'zCut', kind: 'zone', name: 'Cut zone', lat: r7(mid[0] + 60 / F.ky), lng: mid[1], lu: { residential: 100 }, connectors: 1 };
  }
  o.P.settings.connectors = 1;
  return { ...o, text: o.c.Store.serialise(o.P) };
}
const FLOOD_META = metaOf('Flood_Plains', ['OBJECTID', 'Hazard', 'rainfall_event', 'CLIMATE_CHANGE_ADJUSTED', 'Superseded_Date', 'YEAR_PRODUCED', 'DOCUMENT_NAME', 'Shape__Area']);
const COASTAL_NAMES = ['Coastal_Inundation_1_AEP', 'Mean_High_Water_Springs_05m_sea_level_rise', 'Mean_High_Water_Springs_10m_sea_level_rise', 'Mean_High_Water_Springs_15m_sea_level_rise'];
async function hazardCases() {
  const out = { labels: [], loads: [], links: [] };
  { const c = loadCtx(); for (const A of [{}, { RAINFALL_EVENT: 100 }, { RAINFALL_EVENT: 50 }, { RAINFALL_EVENT: 200 }, { RAINFALL_EVENT: 7 }, { RAINFALL_EVENT: 0 }, { RAINFALL_EVENT: -5 }, { RAINFALL_EVENT: 'abc' }, { RAINFALL_EVENT: '25' }, { RAINFALL_EVENT: 100.5, CLIMATE_CHANGE_ADJUSTED: 'Yes' }, { CLIMATE_CHANGE_ADJUSTED: 'None' }, { CLIMATE_CHANGE_ADJUSTED: 'none' }, { CLIMATE_CHANGE_ADJUSTED: ' NONE ' }, { CLIMATE_CHANGE_ADJUSTED: 'No' }, { CLIMATE_CHANGE_ADJUSTED: 'no change' }, { CLIMATE_CHANGE_ADJUSTED: 'Nothing' }, { CLIMATE_CHANGE_ADJUSTED: 'RCP' }, { CLIMATE_CHANGE_ADJUSTED: '' }, { CLIMATE_CHANGE_ADJUSTED: null }, { RAINFALL_EVENT: 1000, CLIMATE_CHANGE_ADJUSTED: 'Y' }, { RAINFALL_EVENT: 99.9 }, { RAINFALL_EVENT: 100.1 }, { RAINFALL_EVENT: 3 }, { RAINFALL_EVENT: 30 }]) {
    const srv = makeServer(); const o = open('corridor', { fetchImpl: srv.fetchImpl });
    jsonRoute(srv, metaPath('Flood_Plains'), 'flood-meta.json', FLOOD_META); literalRoute(srv, layerPath('Flood_Plains'), { features: [{ attributes: A, geometry: { rings: [[[174.9, -37.1], [174.9, -37.2], [175, -37.2], [174.9, -37.1]]] } }] });
    srv.route(url => url.endsWith('?f=json') && url.includes('rest/services') && !url.includes('Flood_Plains'), async () => ({ desc: { kind: 'literal', value: { services: [] } }, response: srv.ok({ services: [] }) }));
    const r = await o.c.Feeds.hazards.load(boundsOf(o.sc), {}); void r;
    out.labels.push({ attrs: A, label: Object.keys(o.c.Feeds.mem.hazards.layers.length ? { x: 1 } : {}) && o.c.Feeds.mem.hazards.layers[0].label });
  } void c; }
  const FX = {}, HP = {}; for (const name of NAMES) { const o = hazardProject(name); HP[name] = o.text; FX[name] = makeHazards(o.c, o.sc); writeFixture(`hazards-flood-${name}.json`, FX[name].flood); writeFixture(`hazards-${name}-project.json`, o.text); for (const n of COASTAL_NAMES) writeFixture(`hazards-coastal-${name}-${n}.json`, FX[name].coastal[n]); }
  writeFixture('flood-meta.json', FLOOD_META);
  const list = names => ({ services: [{ name: 'Hosted/Other_Layer', type: 'FeatureServer' }, ...names.map(n => ({ name: 'Hosted/' + n, type: 'FeatureServer' })), { name: 'Roads', type: 'MapServer' }] });
  const serve = (srv, name, { flood = true, listNames = COASTAL_NAMES, failCoastal = [], meta = true, page = 8 } = {}) => {
    if (meta) jsonRoute(srv, metaPath('Flood_Plains'), 'flood-meta.json', FLOOD_META); else failRoute(srv, metaPath('Flood_Plains'), { failure: 'http', status: 500 });
    if (flood) layerRoute(srv, { match: layerPath('Flood_Plains'), file: `hazards-flood-${name}.json`, features: FX[name].flood.features, page });
    srv.route(url => url.endsWith('/rest/services?f=json'), async () => ({ desc: { kind: 'literal', value: list(listNames) }, response: srv.ok(list(listNames)) }));
    COASTAL_NAMES.forEach(n => { if (failCoastal.includes(n)) failRoute(srv, url => url.includes(`/${n}/FeatureServer/0/query?`), { failure: 'http', status: 500 }); else layerRoute(srv, { match: url => url.includes(`/${n}/FeatureServer/0/query?`), file: `hazards-coastal-${name}-${n}.json`, features: FX[name].coastal[n].features, page }); });
  };
  for (const name of NAMES) for (const [label, o2] of [['all', {}], ['no-meta', { meta: false }], ['one-missing', { listNames: COASTAL_NAMES.slice(1) }], ['none-listed', { listNames: [] }], ['one-fails', { failCoastal: [COASTAL_NAMES[1]] }], ['all-fail', { failCoastal: COASTAL_NAMES }], ['flood-fails', { flood: false }], ['list-fails', 'listfail'], ['no-flood-route', { flood: false, noRoute: true }]]) {
    const srv = makeServer();
    if (o2 === 'listfail') { serve(srv, name); srv.routes.unshift({ test: url => url.endsWith('/rest/services?f=json'), answer: failing(srv, { failure: 'unreachable', message: 'Failed to fetch' }) }); }
    else { serve(srv, name, o2); if (o2.flood === false && !o2.noRoute) srv.routes.unshift({ test: layerPath('Flood_Plains'), answer: failing(srv, { failure: 'http', status: 500 }) }); }
    const o = open(name, { fetchImpl: srv.fetchImpl }), st = []; o.P = reopen(o.c, HP[name]); o.sc = o.P.scenarios[0];
    const r = await o.c.Feeds.hazards.load(boundsOf(o.sc), { status: t => st.push(t) });
    out.loads.push({ project: name, label, bounds: boxOf(boundsOf(o.sc)), requests: srv.log, statuses: st, result: plain(r), layers: plain(o.c.Feeds.mem.hazards.layers), feeds: plain(clone(o.c.Feeds.F())) });
  }
  // the roads inside the hazards: every scenario in turn with one shared data set (the first scenario's projection is kept), and each alone
  for (const name of NAMES) {
    const srv = makeServer(); serve(srv, name);
    const text = HP[name];
    for (const order of [[0, 1, 2], [1, 0, 2], [2]]) {
      const c2 = loadCtx({ seed: 11, fetchImpl: srv.fetchImpl }); const P2 = reopen(c2, text); c2.Store.P = P2;
      if (name === 'grid') Object.values(P2.scenarios[0].links).forEach((L, i) => { if (i % 6 === 0) L.fc = { gt: 'Primary' }; if (i % 8 === 3) L.fc = { gt: '' }; });
      const text2 = c2.Store.serialise(P2);
      await c2.Feeds.hazards.load(boundsOf(P2.scenarios[0]), {});
      const rec = { project: name, order, text: text2, rows: [] };
      for (const si of order) { const sc = P2.scenarios[si]; if (!sc) continue; rec.rows.push({ scenario: sc.id, rows: plain(c2.Feeds.hazards.links(sc, c2.Feeds.mem.hazards)) }); }
      rec.empty = plain(c2.Feeds.hazards.links(P2.scenarios[0], null));
      out.links.push(rec);
    }
  }
  return out;
}

/* ====================================================================================================
   fleet: the Motor Vehicle Register
   ==================================================================================================== */
async function fleetCases() {
  const out = { runs: [], co2: [] };
  const NZ = 'https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services';
  const svc = ['MVR_Aug26', 'Hosted/MVR_Sep26', 'MVR_Oct26', 'MVR_Jun26', 'Other', 'MVR_oct26', 'MVR_Xxx26', 'MVR_Dec25', 'MVR_Oct2'];
  const list = { services: svc.map(n => ({ name: n, type: 'FeatureServer' })) };
  const meta = metaOf('MVR_Oct26', ['OBJECTID', 'tla', 'VEHICLE_TYPE', 'Motive_Power', 'MAKE']);
  const stats = (rows, mp) => ({ features: rows.map(([k, n, extra]) => ({ attributes: { [mp]: k, ...(extra || {}), n } })) });
  const AK = [['PETROL', 640000], ['DIESEL', 180000], ['ELECTRIC', 52000], ['HYBRID', 61000], ['PLUG-IN PETROL HYBRID', 9000], ['OTHER', 800], [null, 120], ['LPG', 15]];
  writeFixture('fleet-list.json', list); writeFixture('fleet-meta.json', meta);
  const cases = [
    ['auckland', 'AUCKLAND', {}], ['lower-case', ' hamilton city ', {}], ['quote', "O'BRIEN DISTRICT", {}], ['empty', '   ', {}], ['no-meta-fields', 'AUCKLAND', { meta: { name: 'x' } }], ['meta-fails', 'AUCKLAND', { metaFail: true }], ['no-layer', 'AUCKLAND', { list: { services: [{ name: 'x' }] } }], ['list-fails', 'AUCKLAND', { listFail: true }], ['no-cars', 'NOWHERE', { rows: [] }], ['zero-cars', 'NOWHERE', { rows: [['PETROL', 0], ['DIESEL', null]] }],
    ['query-fails', 'AUCKLAND', { queryFail: true }], ['duplicate-keys', 'AUCKLAND', { rows: [['PETROL', 100], ['PETROL', 300], ['ELECTRIC', 50]] }], ['lower-field', 'AUCKLAND', { lowerField: true }], ['n-capital', 'AUCKLAND', { capitalN: true }], ['ties', 'AUCKLAND', { rows: [['A', 10], ['B', 10], ['C', 30], ['D', 10]] }],
  ];
  for (const [label, district, o] of cases) {
    const srv = makeServer();
    const listAns = o.list ?? list;
    if (o.listFail) failRoute(srv, url => url === NZ + '?f=json', { failure: 'unreachable', message: 'Failed to fetch' }); else literalRoute(srv, url => url === NZ + '?f=json', listAns);
    const m = o.meta ?? meta;
    if (o.metaFail) failRoute(srv, url => url.endsWith('MVR_Oct26/FeatureServer/0?f=json'), { failure: 'http', status: 500 }); else literalRoute(srv, url => url.endsWith('MVR_Oct26/FeatureServer/0?f=json'), m);
    const mp = o.lowerField ? 'motive_power' : 'Motive_Power';
    const body = stats(o.rows ?? AK, mp); if (o.capitalN) body.features.forEach(f => { f.attributes.N = f.attributes.n; delete f.attributes.n; });
    if (o.queryFail) failRoute(srv, url => url.includes('MVR_Oct26/FeatureServer/0/query?'), { failure: 'http', status: 504 }); else literalRoute(srv, url => url.includes('MVR_Oct26/FeatureServer/0/query?'), body);
    const o2 = open('corridor', { fetchImpl: srv.fetchImpl }); const rec = { label, district, now: NOW };
    try { rec.result = plain(await o2.c.Feeds.fleet.load(district, { now: NOW })); } catch (e) { rec.thrown = e.message; }
    rec.requests = srv.log; rec.feeds = plain(clone(o2.c.Feeds.F()));
    rec.shareOrder = rec.result ? Object.keys(rec.result.shares) : null;
    out.runs.push(rec);
  }
  // kept results: asked again 10 and 100 days later
  { const srv = makeServer(); literalRoute(srv, url => url === NZ + '?f=json', list); literalRoute(srv, url => url.endsWith('MVR_Oct26/FeatureServer/0?f=json'), meta); literalRoute(srv, url => url.includes('MVR_Oct26/FeatureServer/0/query?'), stats(AK, 'Motive_Power'));
    const o = open('corridor', { fetchImpl: srv.fetchImpl }); const rec = { label: 'kept', district: 'AUCKLAND', steps: [] };
    for (const days of [0, 10, 89, 91, 100]) { const n0 = srv.log.length; const r = await o.c.Feeds.fleet.load('AUCKLAND', { now: NOW + days * 86400000 }); rec.steps.push({ days, cached: !!r.cached, requests: srv.log.length - n0, result: plain(r), vintage: plain(clone(o.c.Feeds.F().vintage.mvr)) }); }
    rec.keptText = o.c.ctx.localStorage.getItem('corridor.feeds.fleet.AUCKLAND'); rec.requests = srv.log; out.runs.push(rec); }
  // CO2 from the fleet
  for (const [label, shares] of [['auckland', Object.fromEntries(AK.map(([k, n]) => [k ?? 'UNKNOWN', n / 960000]))], ['no-electric', { PETROL: 0.9, DIESEL: 0.1 }], ['all-electric', { ELECTRIC: 1 }], ['spaced', { ' electric ': 0.2, PETROL: 0.8 }], ['two-electric', { ELECTRIC: 0.1, electric: 0.05, PETROL: 0.85 }], ['text-share', { ELECTRIC: '0.25', PETROL: 0.75 }], ['nan-share', { ELECTRIC: 'x', PETROL: 1 }], ['over-one', { ELECTRIC: 1.2 }], ['tiny', { ELECTRIC: 0.0004 }]]) {
    const o = open('corridor'), F = o.c.Feeds; F.F().fleet = { district: 'AUCKLAND', service: 'MVR_Oct26', url: 'u', at: 'a', total: 1, shares };
    const rec = { label, shares };
    try { await F.act('fleetco2', null); } catch (e) { rec.actError = e.message; }
    rec.co2Factor = o.P.settings.co2Factor ?? null; rec.co2FactorSrc = o.P.settings.co2FactorSrc ?? null;
    try { await F.act('fleetco2off', null); } catch (e) { rec.offError = e.message; }
    rec.afterOff = { co2Factor: o.P.settings.co2Factor ?? null, co2FactorSrc: o.P.settings.co2FactorSrc ?? null, has: 'co2Factor' in o.P.settings };
    out.co2.push(rec);
  }
  return out;
}

/* ====================================================================================================
   live: TomTom and Google
   ==================================================================================================== */
async function liveCases() {
  const out = { tiles: [], speeds: [], google: [], weekdays: [] };
  const key = ' ab c/1&2?#é ';
  for (const k of [null, '', key, 'plainkey']) { const c = loadCtx(); if (k !== null) c.DataHub.setKey('tomtom', k); out.tiles.push({ key: k, url: c.DataHub.tomtomTiles() }); }
  const answers = [['ok', { ok: true, status: 200, body: { flowSegmentData: { currentSpeed: 38, freeFlowSpeed: 52, confidence: 0.93, roadClosure: false, frc: 'FRC3' } } }], ['ok-sparse', { ok: true, status: 200, body: { flowSegmentData: { currentSpeed: 38 } } }], ['403', { ok: false, status: 403, body: {} }], ['401', { ok: false, status: 401, body: {} }], ['500', { ok: false, status: 500, body: {} }], ['no-segment', { ok: true, status: 200, body: {} }], ['network', 'net'], ['no-key', 'nokey']];
  for (const ll of [[-37.1, 174.95], [-36.123456789, 174.1], [-41.2, 175.5], [0, 0]]) for (const [label, a] of answers) {
    const log = [], c = loadCtx({ fetchImpl: async (url, opts) => { log.push({ url, opts: opts ?? null }); if (a === 'net') throw new TypeError('Failed to fetch'); return { ok: a.ok, status: a.status, json: async () => a.body }; } });
    if (a !== 'nokey') c.DataHub.setKey('tomtom', key);
    const rec = { label, ll }; try { rec.result = plain(await c.DataHub.tomtomSpeed(ll)); } catch (e) { rec.thrown = e.message; } rec.requests = log; out.speeds.push(rec);
  }
  const gans = [['ok', { ok: true, status: 200, body: { routes: [{ duration: '512s', staticDuration: '388s', distanceMeters: 4821 }] } }], ['float', { ok: true, status: 200, body: { routes: [{ duration: '51.25s', staticDuration: '38s', distanceMeters: 100 }] } }], ['no-routes', { ok: true, status: 200, body: { routes: [] } }], ['no-routes-key', { ok: true, status: 200, body: {} }], ['error-body', { ok: true, status: 200, body: { error: { message: 'API key not valid' } } }], ['error-status', { ok: false, status: 403, body: { error: { message: 'Forbidden here' } } }], ['error-no-message', { ok: false, status: 500, body: { error: {} } }], ['bad-json', { ok: false, status: 502, body: 'throw' }], ['not-ok-no-error', { ok: false, status: 429, body: {} }], ['odd-duration', { ok: true, status: 200, body: { routes: [{ duration: 'abc', staticDuration: 12, distanceMeters: null }] } }], ['network', 'net'], ['no-key', 'nokey']];
  for (const [nowLabel, now] of [['wed', NOW], ['sat', Date.UTC(2026, 9, 24, 3)], ['sun-late', Date.UTC(2026, 9, 25, 12)]]) for (const period of ['AM', 'PM', 'x', undefined]) for (const [label, a] of gans) {
    const log = [], c = loadCtx({ now, fetchImpl: async (url, opts) => { log.push({ url, method: opts.method, headers: opts.headers, body: opts.body }); if (a === 'net') throw new TypeError('Failed to fetch'); return { ok: a.ok, status: a.status, json: async () => { if (a.body === 'throw') throw new SyntaxError('bad'); return a.body; } }; } });
    if (a !== 'nokey') c.DataHub.setKey('google', 'gkey-123');
    const rec = { label, nowLabel, now, period: period === undefined ? { undefined: true } : period }; try { rec.result = plain(await c.DataHub.googleTime([-37.1, 174.95], [-37.2, 175.01234567], period)); } catch (e) { rec.thrown = e.message; } rec.requests = log; out.google.push(rec);
  }
  // the next Tuesday to Thursday morning or evening, from many moments (around daylight saving changes and new year too)
  const nows = []; for (let d = 0; d < 16; d++) for (const h of [0, 5, 11, 12, 13, 23]) nows.push(Date.UTC(2026, 9, 15 + d, h, 30));
  for (let d = 0; d < 12; d++) for (const h of [3, 11, 14, 22]) { nows.push(Date.UTC(2026, 8, 20 + d, h)); nows.push(Date.UTC(2027, 2, 30 + d, h)); nows.push(Date.UTC(2026, 11, 26 + d, h)); }
  for (const now of nows) { const c = loadCtx({ now }); out.weekdays.push({ now, am: plain(c.DataHub.nextWeekdayNZ(8)), pm: plain(c.DataHub.nextWeekdayNZ(17)), noon: plain(c.DataHub.nextWeekdayNZ(12)), midnight: plain(c.DataHub.nextWeekdayNZ(0)), late: plain(c.DataHub.nextWeekdayNZ(23)) }); }
  return out;
}

/* ====================================================================================================
   vintage: the data-vintage table and stamps
   ==================================================================================================== */
async function vintageCases() {
  const out = { stamps: [], rows: [] };
  const un = s => String(s).replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, '&');
  const strip = s => un(String(s).replace(/<[^>]+>/g, ''));
  const href = s => { const m = /href="([^"]*)"/.exec(String(s)); return m ? un(m[1]) : ''; };
  const o = open('corridor'), F = o.c.Feeds;
  const extras = [['nslr', {}], ['nslr', { n: 5, note: 'x' }], ['tms', { n: 0 }], ['unknown', { n: 1 }], ['nslr', { url: 'u2', licence: 'L2', n: 2 }], ['nslr', { url: '', licence: '' }], ['council', { key: 'council:abc', url: 'https://c.example/FeatureServer/0', source: 'My council', n: 4, note: 'n' }], ['gtfs', { key: 'gtfs', url: 'g', licence: '', n: 3 }], ['mvr', { error: 'bad thing', n: 0 }], ['flood', { error: 'Layer & <gone>', n: 0, note: 'ignored' }], ['osm', { page: 'https://p.example', n: 7 }], ['ags', { at: '2020-01-01T00:00:00.000Z', n: 9 }], ['profiles', { key: 0, n: 1 }]];
  for (const [id, extra] of extras) out.stamps.push({ id, extra, result: plain(F.stamp(id, extra)), vintage: plain(clone(F.F().vintage)) });
  F.F().vintage = {};
  for (const [id, e] of [['nslr', { n: 12000, note: 'Current versions only. 80% of roads matched.' }], ['controlled', { n: 4 }], ['tms', { n: 3, note: 'a' }], ['gtfs', { key: 'gtfs', url: 'https://files.mobilitydatabase.org/mdb-1029/latest.zip', n: 400, note: 'b' }], ['council', { key: 'council:x', url: 'https://services1.arcgis.com/R6s0QqCMQdwKY6yp/x', source: 'Hamilton City Council', n: 5 }], ['mvr', { error: 'No <b>cars</b> here', n: 0 }], ['council', { key: 'council:y', url: 'javascript:alert(1)', source: 'Hamilton City Council', n: 0 }], ['osm', { n: 1234567, url: 'data/x.json', page: 'data/x.json' }], ['ags', { n: 3, note: "It's \"quoted\" & <tagged>" }], ['flood', { n: 6, source: 'Zed' }], ['coastal', { n: 6, source: 'Alpha' }], ['unknown', { n: 1, note: 'x' }]])
    F.stamp(id, e);
  F.F().vintage.nslr.at = '2025-09-09T12:00:00.000Z'; F.F().vintage.tms.at = 'bad'; delete F.F().vintage.controlled.n; F.F().vintage.unknown.source = undefined;
  const rows = F.vintageRows();
  out.vintage = plain(clone(F.F().vintage));
  out.rows = rows.map(r => ({ source: un(r[0]), href: href(r[1]), host: strip(r[1]), queried: un(r[2]), licence: un(r[3]), records: String(r[4]), notes: strip(r[5]), error: /color:var\(--danger\)/.test(String(r[5])) }));
  out.html = rows.map(r => r.map(String));
  return out;
}

/* ====================================================================================================
   main
   ==================================================================================================== */
const SECTIONS = { time: timeCases, regex: regexCases, consts: constCases, loaders: loaderCases, nslr: nslrCases, control: controlCases, tms: tmsCases, ags: agsCases, gtfs: gtfsCases, council: councilCases, hazards: hazardCases, fleet: fleetCases, live: liveCases, vintage: vintageCases };
for (const [name, fn] of Object.entries(SECTIONS)) {
  if (!wants(name)) continue;
  const t0 = Date.now();
  write(`expected/feeds-${name}.json`, await fn());
  console.log(`  ${name}: ${Date.now() - t0} ms`);
}
