// Records the prototype's calibration end to end, so the Rust port (projects/spare-construct/crates/construct-transport/src/calibrate
// and datahub, tests/calibrate_truth.rs, calibrate_cases.rs) can be checked against it.
//
// The two truth-test projects are copied and given what a real study has: traffic counts of every kind (the user's own
// AM and PM hours, two-way and directional, daily-only counts, Auckland Transport counts with a counted peak hour,
// NZTA counts with TMS term-time means and their own peak shares, NZTA virtual sites, RAMM centreline estimates, one
// count switched off, one far from any road, one that disagrees with the road's name), offset from the modelled flows
// by fixed factors so that some sites pass GEH 5 and some fail; screenlines (calibration and validation, a cordon, a core
// area) and turning counts (long format, 15-minute intervals, with a peak hour). Everything is deterministic (seeded).
//
// Part 1 (this file's first half): DataHub (periodVol, obsMethod, countWeight, norm, matches, inUse, calibration,
// regression, jtCheck) and calibrate.js (sites, units, parallel, stats, matrixChange, baseView, fit with every branch,
// clear, report). Part 2 (second half): features/calibration.js, loaded beside the core with the interface objects inert.
//
// Part 3: the pieces that run on their own: turning-count file reading and Furness (calibrate-turns.json), screenline suggestions from an
// Overpass answer (calibrate-suggest-*.json), gateway flows from counts (calibrate-gateways-*.json), the stress test (calibrate-stress-*.json)
// and the CSV import of your own counts (calibrate-import.json).
//
// Writes expected/calibrate-corridor.json, calibrate-grid.json, calibrate-grid-existing.json, calibrate-special.json, calibrate-cases.json,
// calibrate-panel-corridor.json, calibrate-panel-corridor-forecast.json, calibrate-panel-grid.json, calibrate-turns.json, calibrate-suggest-*.json,
// calibrate-gateways-*.json, calibrate-stress-*.json and calibrate-import.json. Run: node gen-calibrate.mjs [fits|cases|panel|turns|suggest|gateways|stress|import]
// (no argument runs everything; the same input always gives the same files).
// The Rust side: projects/spare-construct/crates/construct-transport/tests/calibrate_*.rs.
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import vm from 'node:vm';
import { JS_DIR } from './js-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const CORE = ['util', 'model', 'registry', 'engine', 'data', 'landuse', 'turns', 'calibrate', 'sim', 'sample', 'kit', 'features/calibration'];
const NOW = Date.UTC(2026, 9, 21);
const ISO_NOW = new Date(NOW).toISOString();
const inert = () => new Proxy(function () {}, { get: (t, k) => (k === Symbol.toPrimitive ? () => '' : inert()), apply: () => undefined });

// js-core.mjs's loader, copied: the same context, with kit.js and features/calibration.js loaded after the core
export function loadAll({ seed = 1, now = NOW, fetchImpl = null } = {}) {
  let s = seed >>> 0;
  const rand = () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  const M = Object.create(Math); M.random = rand;
  const store = new Map();
  const ctx = {
    console, JSON, Promise, Map, Set, WeakMap, WeakSet, Array, Object, Number, String, Boolean, Symbol, Error, TypeError, RangeError,
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
  vm.runInContext(src + `\n;globalThis.__core = { U, Store, Net, Sites, Engine, DataHub, Turns, Calibrate, Calibration, Kit, newProject, newScenario, defaultSettings, migrate, CLASSES, LANDUSES };`, ctx, { filename: 'transport-calibrate.js' });
  return ctx.__core;
}

export const plain = v => {
  if (ArrayBuffer.isView(v)) return Array.from(v, plain);
  if (v instanceof Map) return Object.fromEntries([...v].map(([k, x]) => [String(k), plain(x)]));
  if (v instanceof Set) return [...v].map(plain);
  if (typeof v === 'number') return Number.isFinite(v) ? v : String(v);
  if (Array.isArray(v)) return v.map(plain);
  if (v && typeof v === 'object') { const o = {}; for (const [k, x] of Object.entries(v)) if (typeof x !== 'function') o[k] = plain(x); return o; }
  return v;
};

let seed = 20261021;
const rnd = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed / 4294967296; };
const round = (x, d = 0) => { const k = 10 ** d; return Math.round(x * k) / k; };

/* ---------------- the counts ---------------- */
// Offsets from the modelled flow: near 1 pass GEH 5, the far ones fail it
const OFFSETS = [1.0, 0.92, 1.1, 0.58, 1.38, 0.81, 1.04, 1.24, 0.7, 1.0, 1.3, 0.95, 0.63, 1.07, 1.16, 0.88];
const DIRS = ['NB', 'EB', 'SB', 'WB'];
const compass = b => DIRS[Math.round((((b % 360) + 360) % 360) / 90) % 4];

// Counts for a scenario from the modelled base-year flows (base: { AM, PM } results of the base-year view)
function buildCounts(c, sc, base, opts = {}) {
  const { U, Net, CLASSES } = c, S = c.Store.S, counts = [], byLink = {};
  const off = opts.offset || 1;
  let n = 0;
  const factor = () => opts.flat ? 1 : round(OFFSETS[(n++ * 5 + 3) % OFFSETS.length] * off, 3);
  const links = Object.values(sc.links);
  links.forEach((L, i) => {
    const g = Net.geom(sc, L), cum = U.cumLen(g), len = cum[cum.length - 1], mid = U.along(g, cum, len / 2).p;
    const b0 = U.bearing(g[0], g[g.length - 1]);
    const vol = (per, d) => (base[per].arcs[L.id + ':' + d] || { v: 0 }).v;
    const both = per => vol(per, 'ab') + vol(per, 'ba');
    const at = (dLat = 0, dLng = 0) => [round(mid[0] + dLat, 7), round(mid[1] + dLng, 7)];
    const road = L.name || ('Unnamed ' + i);
    const add = o => { counts.push(o); (byLink[L.id] ||= []).push(o.id); };
    switch (i % 7) {
      case 0:
        for (const d of ['ab', 'ba']) {
          const bearing = d === 'ab' ? b0 : (b0 + 180) % 360, ll = at(0.00002, 0.00002);
          if ((d === 'ab' ? L.lanesAB : L.lanesBA) > 0) add({ id: `user:${i}:${d}`, src: 'user', name: `${road} ${d}`, road, lat: ll[0], lng: ll[1], dir: compass(bearing), directional: true,
            am: round(vol('AM', d) * factor()), pm: round(vol('PM', d) * factor()), adt: null, note: 'Imported from your file' });
        }
        break;
      case 1: { const ll = at(-0.00003, 0.00001); add({ id: `user:${i}`, src: 'user', name: road, road, lat: ll[0], lng: ll[1], dir: null, directional: false, am: round(both('AM') * factor()), pm: round(both('PM') * factor()), adt: null, note: 'Imported from your file' }); break; }
      case 2: { const ll = at(0.00001, -0.00002); add({ id: `user:${i}`, src: 'user', name: road, road, lat: ll[0], lng: ll[1], dir: null, directional: false, am: null, pm: null, adt: round(both('AM') * factor() / (S.data.amShare / 100)), note: 'Imported from your file' }); break; }
      case 3: { const ll = at(0.00002, 0); add({ id: `at:${100 + i}`, src: 'at', ref: String(100 + i), name: `${road} · A to B`, road, lat: ll[0], lng: ll[1], adt: round(both('AM') * 9.5 * factor()), peak: round(both('AM') * factor()), peakTime: '07:45', hv: 5.2, date: '2025-03-12', dur: '7 day', directional: false, note: '7 day count, Mar 2025' }); break; }
      case 4: { const ll = at(-0.00002, 0); add({ id: `at:${100 + i}`, src: 'at', ref: String(100 + i), name: `${road} · B to C`, road, lat: ll[0], lng: ll[1], adt: round(both('PM') * 10 * factor()), peak: round(both('PM') * factor()), peakTime: '17:00', hv: 4.1, date: '2025-04-02', dur: '7 day', directional: false, note: '7 day count, Apr 2025' }); break; }
      case 5: {
        const d = L.lanesAB > 0 ? 'ab' : 'ba', bearing = d === 'ab' ? b0 : (b0 + 180) % 360, ll = at(0, 0.00003);
        const rec = { id: `nzta:SH${i}`, src: 'nzta', ref: `SH${i}`, name: `SH site ${i}`, road: 'SH' + i, lat: ll[0], lng: ll[1], adt: round(vol('AM', d) * factor() / (S.data.amShare / 100)), hv: 8, dir: compass(bearing), directional: true, virtual: false, ramp: false, note: 'Telemetry site' };
        if (i % 2) { rec.tms = { target: { mean: round(vol('AM', d) * factor() / 0.09), ok: true, ci95: 140, n: 41 } }; rec.share = { AM: 0.092, PM: 0.101 }; }
        else if (i % 3 === 0) { rec.tms = { target: { mean: 9000, ok: false, ci95: 800, n: 6 } }; }
        add(rec); break;
      }
      case 6: { const ll = at(0.00004, 0.00004); add({ id: `ramm:${i}`, src: 'ramm', kind: 'ramm_estimate', name: `${road} (RAMM)`, road, lat: ll[0], lng: ll[1], adt: round(both('AM') * 10 * factor()), note: 'NZTA RCA centreline estimate' }); break; }
    }
  });
  // special counts: switched off, nowhere near a road, a different road's name, virtual, a ramp, an estimate under a real count
  const L0 = links[0], g0 = Net.geom(sc, L0), mid0 = U.along(g0, U.cumLen(g0), U.cumLen(g0).slice(-1)[0] * 0.3).p;
  counts.push({ id: 'user:off', src: 'user', name: 'Switched off', road: L0.name, lat: round(mid0[0], 7), lng: round(mid0[1], 7), am: 700, pm: 650, off: true, directional: false });
  counts.push({ id: 'user:far', src: 'user', name: 'Far away', road: 'Nowhere Road', lat: -35.5, lng: 174.1, am: 400, pm: 380, directional: false });
  counts.push({ id: 'user:other-name', src: 'user', name: 'Other street count', road: 'Other Street', lat: round(mid0[0] + 0.00004, 7), lng: round(mid0[1] + 0.00002, 7), am: 520, pm: 480, directional: false });
  counts.push({ id: 'nzta:virtual', src: 'nzta', ref: 'V1', name: 'Virtual site', road: 'SH1', lat: round(mid0[0], 7), lng: round(mid0[1] + 0.00004, 7), adt: 12000, virtual: true, directional: true, dir: 'EB', ramp: false });
  counts.push({ id: 'nzta:ramp', src: 'nzta', ref: 'R1', name: 'A ramp', road: 'SH1', lat: round(mid0[0], 7), lng: round(mid0[1] - 0.00004, 7), adt: 6000, directional: true, dir: 'NB', ramp: true });
  const atc = counts.find(x => x.src === 'at');
  if (atc) counts.push({ id: 'ramm:shadowed', src: 'ramm', kind: 'ramm_estimate', name: 'Estimate under an AT count', road: atc.road, lat: atc.lat + 0.0004, lng: atc.lng, adt: 7777 });
  return { counts, byLink };
}

// A project ready for calibration: the fixture, its zones marked existing when asked, and counts built on the base-year flows
async function prepare(fixture, { existing = false, offset = 1, flat = false, year = null } = {}) {
  const c = loadAll({ seed: 11 });
  const P = c.migrate(JSON.parse(readFileSync(join(here, 'fixtures', fixture + '.json'), 'utf8')));
  c.Store.P = P;
  if (year) P.settings.year = year;
  const sc = P.scenarios[0];
  if (existing) for (const z of Object.values(sc.zones)) if (z.kind !== 'gateway') z.existing = true;
  const { view } = c.Calibrate.baseView(sc);
  const base = { AM: await c.Engine.assign(view, { period: 'AM' }), PM: await c.Engine.assign(view, { period: 'PM' }) };
  const { counts, byLink } = buildCounts(c, sc, base, { offset, flat });
  const d = c.DataHub.D();
  d.counts = counts;
  d.loaded.user = { at: NOW, n: counts.filter(x => x.src === 'user').length };
  d.loaded.at = { at: NOW, n: counts.filter(x => x.src === 'at').length };
  d.loaded.nzta = { at: NOW, n: counts.filter(x => x.src === 'nzta').length };
  // results for the other scenarios, so a fit has something to mark out of date
  for (const o of P.scenarios.slice(1)) o.res = { AM: await c.Engine.assign(o, { period: 'AM' }) };
  return { c, P, sc, base, byLink, text: c.Store.serialise(P) };
}

// A fresh context with the project loaded from its text (what the Rust test reads), the caches empty
function open(text) {
  const c = loadAll({ seed: 11 });
  c.Store.P = c.migrate(JSON.parse(text));
  return { c, P: c.Store.P, sc: c.Store.P.scenarios[0] };
}

/* ---------------- calibrate.js: fits ---------------- */
const rowOut = r => ({ id: r.c.id, link: r.mt.link, dir: r.mt.dir, d: r.mt.d, obs: r.obs, model: r.model, geh: r.geh, arcs: r.arcs, w: r.w });
// The parts of a result worth comparing: convergence, every arc's flow, and what the paths say (arcs carried, flows)
const resOut = res => res ? {
  period: res.period, year: res.year, it: res.it, gap: res.gap, converged: res.converged, stale: res.stale, scenario: res.scenario.id, note: res.note, manual: res.manual,
  O: res.O, D: res.D, O0: res.O0, D0: res.D0, arcsV: Object.fromEntries(Object.entries(res.arcs).map(([k, a]) => [k, a.v])),
  pathsN: res.paths ? res.paths.reduce((s, od) => s + od.list.length, 0) : null,
} : null;
// What a save writes: a number that is not finite is null
const saved = v => (v === undefined ? null : JSON.parse(JSON.stringify(v)));
// A report: its matrix record is the saved one
const reportOut = r => ({ ...plain({ ...r, matrix: undefined }), matrix: saved(r.matrix) });
const fitOut = out => plain({ ...out, res: resOut(out.res) });

async function runFit(name, text, fixtureName, { opts = {}, setup = null, before = false, after = true, scIndex = 0, clear = null } = {}) {
  const { c, P } = open(text), sc = P.scenarios[scIndex];
  if (setup) await c.Calibrate.fit(sc, { period: setup === 'fit-pm' ? 'PM' : 'AM' });
  const period = opts.period || P.settings.period;
  const rec = { name, fixture: fixtureName, scenario: sc.id, setup, period, opts: plain({ ...opts, weight: undefined, weightFrom: undefined, groupsFrom: undefined, countIdsFrom: undefined }), date: ISO_NOW };
  const fo = { ...opts, onProgress: null };
  delete fo.onProgress;
  if (opts.groupsFrom) { fo.groups = opts.groupsFrom(c, P, sc); delete fo.groupsFrom; rec.groups = plain(fo.groups); }
  if (opts.weightFrom) { fo.weight = opts.weightFrom(c); delete fo.weightFrom; }
  if (opts.countIdsFrom) { fo.countIds = opts.countIdsFrom(c, P, sc); delete fo.countIdsFrom; rec.countIds = fo.countIds; }
  rec.weightKind = fo.weights === true ? 'method' : typeof fo.weight === 'function' ? 'caller' : 'none';
  rec.meBefore = saved(P.me);
  if (before) {
    // the user has run the model and looks at the validation first
    sc.res[period] = await c.Engine.assign(sc, { period });
    rec.reportBefore = reportOut(c.Calibrate.report(sc, period));
  }
  // the prior run on the base-year view: its counted sites, the fitting units and the parallel check of any groups
  {
    const { view, same } = c.Calibrate.baseView(sc), meKeep = P.me && P.me[period];
    if (P.me) delete P.me[period];
    const res0 = await c.Engine.assign(view, { period });
    if (meKeep) P.me[period] = meKeep;
    const so = { countIds: fo.countIds, estimates: true, weights: fo.weights, weight: fo.weight };
    const rows = c.Calibrate.sites(view, res0, period, so);
    rec.prior = { same, viewId: view.id, rows: rows.map(rowOut), stats: plain(c.Calibrate.stats(rows)), plainRows: c.Calibrate.sites(view, res0, period).map(rowOut) };
    if (fo.groups) { const chk = res0.paths ? c.Calibrate.parallel(res0, rows, fo.groups) : null; rec.prior.parallel = plain(chk); rec.prior.units = plain(c.Calibrate.units(rows, chk)); }
    else rec.prior.units = plain(c.Calibrate.units(rows, null));
    c.Store.ver++;
  }
  const progress = [];
  const out = await c.Calibrate.fit(sc, { ...fo, onProgress: (k, n, m) => progress.push([k, n, m]) });
  rec.progress = plain(progress);
  rec.result = fitOut(out);
  rec.me = saved(P.me);
  rec.scRes = resOut(sc.res[period] || null);
  rec.scResKeys = Object.keys(sc.res);
  const stale = () => Object.fromEntries(P.scenarios.slice(1).map(s => [s.id, Object.fromEntries(Object.entries(s.res || {}).map(([p, r]) => [p, r ? !!r.stale : null]))]));
  rec.otherStale = stale();
  if (after) {
    rec.report = {};
    for (const p of ['AM', 'PM']) { if (!sc.res[p]) { sc.res[p] = await c.Engine.assign(sc, { period: p }); } rec.report[p] = reportOut(c.Calibrate.report(sc, p)); }
  }
  if (clear) {
    rec.clears = [];
    for (const pd of clear) { const n = c.Calibrate.clear(pd === 'both' ? undefined : pd); rec.clears.push({ period: pd, n, me: saved(P.me), otherStale: stale(), scStale: Object.fromEntries(Object.entries(sc.res).map(([p, r]) => [p, r ? !!r.stale : null])) }); }
  }
  return rec;
}

// Count groups from the counts' links: link names to the counts on them
const groupOf = (byLink, sc, names, id, label) => ({ id, label, countIds: Object.entries(byLink).filter(([lid]) => names.includes(sc.links[lid].name)).flatMap(([, ids]) => ids) });

async function fitCases(fixture, tag, prepOpts) {
  const prep = await prepare(fixture, prepOpts);
  const { text, byLink } = prep;
  const scLinks = prep.sc.links;
  const names = [...new Set(Object.values(scLinks).map(l => l.name))];
  const out = { fixture: tag, projectText: text, cases: [] };
  const run = async (name, o) => { out.cases.push(await runFit(name, text, tag, o)); };
  await run('default-am', { opts: { period: 'AM' }, before: true });
  await run('default-pm', { opts: { period: 'PM' } });
  await run('weights-am', { opts: { period: 'AM', weights: true } });
  await run('weights-pm-iters', { opts: { period: 'PM', weights: true, iters: 3, damp: 0.5 } });
  await run('options', { opts: { period: 'AM', iters: 8, damp: 1, maxFactor: 2, maxStep: 1.5, minVol: 20, constrain: true } });
  await run('unconstrained', { opts: { period: 'AM', constrain: false, iters: 4 } });
  await run('freeze-gg', { opts: { period: 'AM', freezeGG: true, weights: true } });
  await run('count-ids', { opts: { period: 'AM', weights: true, countIdsFrom: (c, P, sc) => { const d = c.DataHub.D(); return d.counts.filter((x, i) => i % 3 !== 1).map(x => x.id); } } });
  await run('count-ids-groups', { opts: { period: 'PM', weights: true, freezeGG: true,
    countIdsFrom: (c, P, sc) => c.DataHub.D().counts.map(x => x.id),
    groupsFrom: (c, P, sc) => names.slice(0, 4).map((nm, k) => groupOf(byLink, sc, [nm], 'g' + k, 'Group ' + nm)).concat([groupOf(byLink, sc, names, 'all', 'Everything'), groupOf(byLink, sc, names.slice(0, 2), 99, null)]) } });
  await run('groups-empty', { opts: { period: 'AM', groupsFrom: () => [] } });
  // a caller's own weights (the calibration panel's: by observation method, without RAMM and with virtual 0)
  await run('caller-weight', { opts: { period: 'AM', weightFrom: ctx => (cnt, p) => ({ measured_hour: 1, weekday_factored: 0.7, adt_factored: 0.4, virtual: 0 })[ctx.Calibration.methodOf(cnt, p)] ?? 0.4 } });
  // the second fit starts from the trip rates and gateway counts again, and replaces the first
  await run('refit', { opts: { period: 'AM', weights: true }, setup: 'fit-am' });
  return { out, prep };
}

/* ---------------- main, part 1 ---------------- */
const write = (name, data) => { const t = JSON.stringify(data); writeFileSync(join(here, 'expected', name), t); console.log(name, t.length, 'bytes'); };
const only = process.argv[2];

const specials = { cases: [] };
async function special(name, fixture, prepOpts, mutate, runOpts) {
  if (only && !name.includes(only)) return;
  const prep = await prepare(fixture, prepOpts);
  if (mutate) await mutate(prep.c, prep.P, prep.sc, prep);
  const text = prep.c.Store.serialise(prep.P);
  const rec = await runFit(name, text, fixture, runOpts);
  rec.projectText = text;
  specials.cases.push(rec);
}

if (!only || only === 'fits') {
  for (const [fixture, tag, popts, file] of [['corridor', 'corridor', {}, 'calibrate-corridor.json'], ['grid', 'grid', {}, 'calibrate-grid.json'], ['grid', 'grid-existing', { existing: true }, 'calibrate-grid-existing.json']]) {
    const { out } = await fitCases(fixture, tag, popts);
    write(file, out);
  }
}
const NOCOUNTS = (c, P) => { c.DataHub.D().counts = []; };
for (const fixture of ['corridor', 'grid']) {
  await special(fixture + '-big-offsets', fixture, { offset: 1.9, existing: true }, null, { opts: { period: 'AM' } });
  await special(fixture + '-big-offsets-low', fixture, { offset: 0.45, existing: true }, null, { opts: { period: 'PM', weights: true } });
  await special(fixture + '-flat', fixture, { flat: true, existing: true }, null, { opts: { period: 'AM' } });
  await special(fixture + '-flat-keeps-old', fixture, { flat: true, existing: true }, (c, P, sc) => {
    P.me = { AM: { [Object.values(sc.zones).find(z => z.kind === 'gateway').id + '>' + Object.values(sc.zones).filter(z => z.kind === 'gateway')[1].id]: 1.2 }, info: { AM: { baseYear: 2026, iters: 2, date: '2026-01-01T00:00:00.000Z' } } };
  }, { opts: { period: 'AM' }, before: true });
  await special(fixture + '-no-counts', fixture, { existing: true }, NOCOUNTS, { opts: { period: 'AM' }, before: true });
  await special(fixture + '-no-counts-keeps-old', fixture, { existing: true }, (c, P, sc) => { c.DataHub.D().counts = []; P.me = { PM: { 'a>b': 1.1 }, info: { PM: { baseYear: 2026 } } }; }, { opts: { period: 'AM' } });
  await special(fixture + '-zero-weight', fixture, { existing: true }, (c, P) => { const d = c.DataHub.D(); d.counts = d.counts.filter(x => x.virtual); d.use.virtual = true; }, { opts: { period: 'AM' } });
  await special(fixture + '-no-eligible', fixture, { existing: true }, (c, P, sc) => { for (const z of Object.values(sc.zones)) if (z.kind === 'gateway') delete sc.zones[z.id]; for (const z of Object.values(sc.zones)) z.existing = false; }, { opts: { period: 'AM' } });
  await special(fixture + '-clear', fixture, { existing: true }, null, { opts: { period: 'AM' }, clear: ['PM', 'AM', undefined] });
  await special(fixture + '-clear-both', fixture, { existing: true }, null, { opts: { period: 'AM' }, setup: 'fit-pm', clear: ['both', 'both'] });
}
await special('grid-too-many-zones', 'grid', { existing: true }, (c, P, sc) => {
  const ns = Object.values(sc.nodes);
  for (let i = 0; i < 301; i++) { const n = ns[i % ns.length]; sc.zones['zx' + i] = { id: 'zx' + i, kind: 'zone', name: 'Extra ' + i, lat: n.lat + 0.00001 * (i % 7), lng: n.lng + 0.00001 * (i % 5), lu: { residential: 0.4 }, existing: true }; }
}, { opts: { period: 'AM' } });
if (specials.cases.length) write('calibrate-special.json', specials);

/* ---------------- DataHub and calibrate.js, function by function ---------------- */
const cases = { rules: null, norm: [], periodVol: [], inUse: [], matches: [], calibration: [], regression: [], jtCheck: [], origin: [], stats: [], matrixChange: [], baseView: [], eligible: [], report: {}, log: [], weightOf: [] };
if (!only || only === 'cases') {
  const { c: c0 } = await prepare('corridor', {});
  cases.rules = plain(c0.Calibrate.RULES);
  cases.obsMethods = plain(Object.fromEntries(Object.entries(c0.DataHub.OBS_METHODS).filter(([k, v]) => typeof v === 'object')));
  cases.jtCriteria = plain(c0.DataHub.JT_CRITERIA);
  cases.regLabels = plain(c0.DataHub.REG_LABELS);

  // road names
  const names = ['Great South Road', 'GREAT SOUTH RD', 'Great South Rd (SH1)', 'SH 16', 'SH16 Northwestern Motorway', 'Mt Eden Rd', 'St Heliers Bay Rd', 'Main St.', 'Manukau Hwy NB', 'Queen Ave', 'Queen Aves', 'Tamaki Dr, Orakei', "O'Connell Street", 'Pl Pl', 'Pt Chevalier Pde', 'Tce Ln Mwy', 'Cres', 'abc (def) ghi (jkl', '(x)(y)', 'a (b\nc) d', 'Rd', 'rd rd rd', 'Karangahape Road – Newton', 'Ōrakei Road', 'Hobson   Street', '  leading and trailing  ', '', 'SH1', 'SH 1N', 'Ash1 Rd', 'sh', 'sh x', 'Avenue St Rd Dr Pl Cres Hwy Pde Tce Ln Mwy', 'Albert-Street/Wellesley St', 'ST', 'RD.', 'x_rd_y', 'rd_', '123 Rd 45', 'Dr. Dre Ave.', 'Highway 16 (north) bypass', 'a)(b', 'Kitchener St (Auckland Central) / Wellesley St East', 'Great South Rd (a b)', 'Sh\t16', 'AVES', 'ave aves avex'];
  for (const s of names) cases.norm.push({ s, out: c0.DataHub.norm(s) });
  cases.normLoose = [null, undefined, 0, 12, '0', false, true].map(s => ({ raw: s === undefined ? '__undefined__' : s, out: c0.DataHub.norm(s) }));

  // periodVol, obsMethod, countWeight, isEstimate, validationOK
  const settingsVariants = [{ amShare: 8.5, pmShare: 9.5, matchRadius: 40 }, { amShare: '9', pmShare: '10.5' }, { amShare: 0, pmShare: 12 }, { amShare: 7.25, pmShare: 11.125, matchRadius: 55 }];
  const pvCounts = [
    { id: 'u1', src: 'user', am: 810, pm: 760, adt: 9000 }, { id: 'u2', src: 'user', am: 0, pm: 540, adt: 9000 }, { id: 'u3', src: 'user', adt: 9000 }, { id: 'u4', src: 'user', am: 700 }, { id: 'u5', src: 'user' },
    { id: 'u6', src: 'user', am: '650', pm: '600' }, { id: 'u7', src: 'user', am: null, pm: null, adt: '8000' }, { id: 'u8', src: 'user', obs_method: 'adt_factored', am: 500, pm: 400 }, { id: 'u9', src: 'user', obs_method: 'bogus', am: 500, pm: 400 },
    { id: 'a1', src: 'at', adt: 12000, peak: 900, peakTime: '07:45' }, { id: 'a2', src: 'at', adt: 12000, peak: 900, peakTime: '09:59' }, { id: 'a3', src: 'at', adt: 12000, peak: 900, peakTime: '10:00' },
    { id: 'a4', src: 'at', adt: 12000, peak: 950, peakTime: '14:30' }, { id: 'a5', src: 'at', adt: 12000, peak: 950, peakTime: '18:59' }, { id: 'a6', src: 'at', adt: 12000, peak: 950, peakTime: '19:00' }, { id: 'a7', src: 'at', adt: 12000, peak: 950, peakTime: '0745' },
    { id: 'a8', src: 'at', adt: 12000 }, { id: 'a9', src: 'at', peak: 800, peakTime: '06:00' }, { id: 'a10', src: 'at', adt: 12000, peak: 0, peakTime: '07:45' }, { id: 'a11', src: 'at', adt: 12000, peak: 800, peakTime: '5:30' }, { id: 'a12', src: 'at', adt: 12000, peak: 800, peakTime: '06:00' },
    { id: 'r1', src: 'ramm', kind: 'ramm_estimate', adt: 5000 }, { id: 'r2', src: 'ramm', adt: 5000 }, { id: 'r3', kind: 'ramm_estimate', adt: 0 }, { id: 'r4', src: 'nzta', obs_method: 'ramm_estimate', adt: 4000 }, { id: 'r5', src: 'ramm' }, { id: 'r6', src: 'user', kind: 'ramm_estimate', adt: 3000 },
    { id: 'n1', src: 'nzta', adt: 20000 }, { id: 'n2', src: 'nzta', adt: 20000, virtual: true }, { id: 'n3', src: 'nzta', adt: 20000, tms: { target: { mean: 18000, ok: true } } }, { id: 'n4', src: 'nzta', adt: 20000, tms: { target: { mean: 18000, ok: false } } },
    { id: 'n5', src: 'nzta', adt: 20000, tms: { target: { mean: 18000 } }, share: { AM: 0.09, PM: 0.1 } }, { id: 'n6', src: 'nzta', adt: 20000, share: { AM: 0.09, PM: 1 } }, { id: 'n7', src: 'nzta', adt: 20000, share: { AM: 0, PM: 1.2 } },
    { id: 'n8', src: 'nzta', adt: 20000, share: { AM: '0.081', PM: null } }, { id: 'n9', src: 'nzta', adt: 0, tms: { target: { mean: 15000 } } }, { id: 'n10', src: 'nzta', tms: { target: { mean: -5 } }, adt: 100 }, { id: 'n11', src: 'nzta', adt: 20000, tms: { target: { mean: 'x' } } },
    { id: 'n12', src: 'nzta', adt: 20000, tms: { target: null }, share: [0.1] }, { id: 'n13', src: 'nzta', adt: 20000, tms: 'x', share: 'y' }, { id: 'n14', src: 'nzta', adt: 20000, tms: { target: { mean: 17000, ok: true } }, share: { AM: 0.0834 }, virtual: true }, { id: 'n15', src: 'nzta', adt: 20000, tms: { target: { mean: 17000.5, ok: 0 } } },
    { id: 'n16', src: 'nzta', adt: null }, { id: 'n17', src: 'nzta', adt: '15000', tms: [1, 2] }, { id: 'n18', src: 'nzta', adt: 1234567.891, share: { AM: 0.123456 } },
    { id: 'x1', src: 'other', adt: 5000 }, { id: 'x2', adt: 5000 }, { id: 'x3', src: 'other', adt: 5000, virtual: true }, { id: 'x4', src: 'other' }, { id: 'x5', src: null, adt: 100 },
    { id: 'm1', src: 'user', obs_method: 'virtual', am: 100 }, { id: 'm2', src: 'user', obs_method: 'ramm_estimate', am: 100 }, { id: 'm3', src: 'user', obs_method: 'weekday_factored', am: 100 }, { id: 'm4', src: 'user', obs_method: 'measured_hour' },
  ];
  for (const sv of settingsVariants) {
    const c = loadAll({ seed: 1 });
    c.Store.P = c.newProject('t'); Object.assign(c.Store.P.settings.data, sv);
    for (const cnt of pvCounts) {
      const rec = { settings: sv, count: cnt, periods: {} };
      for (const per of ['AM', 'PM']) {
        const pv = c.DataHub.periodVol(cnt, per);
        rec.periods[per] = { pv: plain(pv && { ...pv, v: +pv.v }), method: c.DataHub.obsMethod(cnt, per), weight: c.DataHub.countWeight(cnt, per) };
      }
      rec.isEstimate = c.DataHub.isEstimate(cnt); rec.validationOK = c.DataHub.validationOK(cnt);
      cases.periodVol.push(rec);
    }
  }
  // inUse and shadowed: counts and switches
  const useVariants = [null, { virtual: true }, { ramm: false }, { nzta: false }, { user: 0, at: 'yes' }, { virtual: 1, ramm: null }, { weird: true }, { nzta: false, at: false, user: false, ramm: false, virtual: false }];
  const iuCounts = [
    { id: 'i1', src: 'nzta', lat: -37.1, lng: 174.95, adt: 100 }, { id: 'i2', src: 'at', lat: -37.1, lng: 174.9501, adt: 100 }, { id: 'i3', src: 'user', lat: -37.1, lng: 174.95, am: 5 }, { id: 'i4', src: 'nzta', virtual: true, lat: -37.1, lng: 174.96, adt: 5 },
    { id: 'i5', src: 'ramm', kind: 'ramm_estimate', lat: -37.1, lng: 174.9503, adt: 5 }, { id: 'i6', src: 'ramm', kind: 'ramm_estimate', lat: -37.1, lng: 174.953, adt: 5 }, { id: 'i7', src: 'ramm', kind: 'ramm_estimate', lat: -37.1005, lng: 174.95, adt: 5 },
    { id: 'i8', src: 'ramm', kind: 'ramm_estimate', lat: -37.1, lng: 174.9507, adt: 5, off: true }, { id: 'i9', src: 'nzta', off: true, lat: -37.1, lng: 174.9514, adt: 1 }, { id: 'i10', src: 'ramm', kind: 'ramm_estimate', lat: -37.1, lng: 174.9514, adt: 5 },
    { id: 'i11', src: 'user', off: 0 }, { id: 'i12', src: 'weird', adt: 3 }, { id: 'i13', lat: 1, lng: 2 }, { id: 'i14', src: 'ramm', kind: 'ramm_estimate', lat: 'x', lng: 174.95 }, { id: 'i15', src: 'nzta', lat: 'x', lng: 174.95 },
    { id: 'i16', src: 'at', virtual: true, lat: -37.1, lng: 174.9602, adt: 5 }, { id: 'i17', src: 'ramm', kind: 'ramm_estimate', lat: -37.1, lng: 174.9602, adt: 5 }, { id: 'i18', src: 'nzta', ramp: true, lat: -37.1, lng: 174.9701, adt: 5 }, { id: 'i19', src: 'ramm', kind: 'ramm_estimate', lat: -37.1, lng: 174.9702, adt: 5 },
  ];
  for (const use of useVariants) {
    const c = loadAll({ seed: 1 });
    c.Store.P = c.newProject('t');
    const d = c.DataHub.D(); d.counts = iuCounts.map(x => ({ ...x })); if (use) d.use = { ...d.use, ...use };
    cases.inUse.push({ use: use ? { ...use } : null, counts: iuCounts, inUse: Object.fromEntries(d.counts.map(x => [x.id, !!c.DataHub.inUse(x)])), shadowed: Object.fromEntries(d.counts.map(x => [x.id, c.DataHub.shadowed(x)])) });
  }
  // regression, origin, ols, stats
  const xs = r => Array.from({ length: r }, () => round(rnd() * 900 + 50));
  const mk = (n, f) => { const x = xs(n); return { x, y: x.map(f) }; };
  const sets = [{ x: [], y: [] }, { x: [5], y: [6] }, { x: [0, 0, 0], y: [1, 2, 3] }, { x: [1, 2, 3], y: [5, 5, 5] }, { x: [100, 200, 300, 400], y: [100, 200, 300, 400] }, { x: [100, 200, 300, 400], y: [110, 190, 330, 380] },
    mk(12, v => round(v * (0.8 + rnd() * 0.4))), mk(30, v => round(v * 1.1 + rnd() * 80 - 40)), mk(7, v => round(v * 0.5)), { x: [10, 20, 30], y: [-5, 0, 5] }, { x: [-10, 10], y: [3, 4] }, { x: [1e6, 2e6, 3e6], y: [1.1e6, 1.9e6, 3.2e6] }, { x: [3, 3, 3], y: [1, 2, 4] }, mk(50, v => round(v + (rnd() - 0.5) * 300))];
  for (const s of sets) {
    cases.regression.push({ ...s, out: plain(c0.DataHub.regression(s.x, s.y)) });
    cases.origin.push({ ...s, origin: plain(c0.Calibrate.origin(s.x, s.y)), ols: plain(c0.Calibrate.ols(s.x, s.y)) });
    const rows = s.x.map((o, i) => ({ obs: o, model: s.y[i], geh: Math.sqrt(2 * (s.y[i] - o) ** 2 / Math.max(1, s.y[i] + o)) }));
    cases.stats.push({ rows: s.x.map((o, i) => [o, s.y[i]]), out: plain(c0.Calibrate.stats(rows)) });
  }
  { // sites at the band edges
    const pairs = [[399, 349], [400, 351], [2000, 1750], [2000, 1749], [2001, 2251], [699, 599], [700, 600], [2700, 3100], [2701, 3100], [10, 12], [0, 0], [0, 30], [5000, 5000]];
    const rows = pairs.map(([o, m]) => ({ obs: o, model: m, geh: Math.sqrt(2 * (m - o) ** 2 / Math.max(1, m + o)) }));
    cases.stats.push({ rows: pairs, out: plain(c0.Calibrate.stats(rows)) });
  }
  for (const [m, o] of [[0, 100], [50, 60], [200, 100], [1000, 900], [-1, 10], [10, 0], [10, -5], [NaN, 10], [10, NaN], [95, 100], [115, 100], [160, 100], [64, 60], [75, 60], [0, 30], [900, 1000], [1500, 1200], [70, 60], [100, 150], [0, 0], [10, 11]]) cases.jtCheck.push({ model: plain(m), obs: plain(o), out: plain(c0.DataHub.jtCheck(m, o)) });
  // matrixChange
  for (const [Z, density] of [[4, 0.7], [5, 0.5], [3, 1], [6, 0.3], [2, 1], [1, 1]]) {
    const prior = new Float64Array(Z * Z), est = new Float64Array(Z * Z);
    for (let i = 0; i < Z * Z; i++) { prior[i] = rnd() < density ? round(rnd() * 400, 1) : 0; est[i] = prior[i] > 0 || rnd() < 0.1 ? round(prior[i] * (0.6 + rnd() * 0.9) + rnd() * 5, 1) : 0; }
    cases.matrixChange.push({ Z, prior: Array.from(prior), est: Array.from(est), out: plain(c0.Calibrate.matrixChange({ zones: new Array(Z).fill({}) }, prior, est)) });
  }
  {
    const prior = Float64Array.from([0, 10, 20, 0, 0, 0, 5, 0, 0]), est = Float64Array.from(prior);
    cases.matrixChange.push({ Z: 3, prior: Array.from(prior), est: Array.from(est), out: plain(c0.Calibrate.matrixChange({ zones: [0, 0, 0] }, prior, est)) });
    const z0 = new Float64Array(4); cases.matrixChange.push({ Z: 2, prior: Array.from(z0), est: Array.from(z0), out: plain(c0.Calibrate.matrixChange({ zones: [0, 0] }, z0.slice(), z0.slice())) });
    const p2 = Float64Array.from([0, 100, 100, 0]), e2 = Float64Array.from([0, 120, 80, 0]); cases.matrixChange.push({ Z: 2, prior: Array.from(p2), est: Array.from(e2), out: plain(c0.Calibrate.matrixChange({ zones: [0, 0] }, p2, e2)) });
  }
  // eligible
  for (const z of [null, undefined, { kind: 'gateway' }, { kind: 'zone', existing: true }, { kind: 'zone', existing: 'true' }, { kind: 'zone' }, { kind: 'zone', existing: false }, { kind: 'gateway', existing: false }]) cases.eligible.push({ z: z === undefined ? '__undefined__' : z, out: c0.Calibrate.eligible(z) });
  // Math.log: the ratios of counts to model, the whole range, near 1, special values
  {
    const xs = [1, 2, 0.5, 10, 0.1, Math.E, 1e-300, 1e300, 5e-324, 1e-310, 2.2250738585072014e-308, 2.225073858507201e-308, 0, -0, -1, Infinity, NaN, 1.0000001, 0.9999999, 1 + 1e-12, 1 - 1e-12, 1 + 2 ** -21, 1 - 2 ** -21, 1.5, 3, 7, 100, 1 / 3, 2 / 3, 4 / 3, 0.35355339059327373, 0.7071067811865476, 1.4142135623730951];
    for (let i = 0; i < 1500; i++) xs.push(Math.exp((rnd() - 0.5) * 8));
    for (let i = 0; i < 500; i++) xs.push(Math.exp((rnd() - 0.5) * 1400));
    for (let i = 0; i < 300; i++) xs.push(1 + (rnd() - 0.5) * 1e-5);
    for (let i = 0; i < 100; i++) xs.push(rnd() * 3 + 0.2);
    cases.log = xs.map(x => [plain(x), plain(Math.log(x))]);
  }
}

/* ---------------- DataHub and calibrate.js on the projects ---------------- */
const matchOut = m => Object.fromEntries([...m].map(([id, x]) => [id, { link: x.link, dir: x.dir, d: x.d, score: x.score, named: !!x.named }]));
const calOut = r => r ? ({ n: r.n, rows: r.rows.map(x => ({ id: x.c.id, link: x.mt.link, dir: x.mt.dir, obs: x.obs, model: x.model, geh: x.geh, how: x.how })),
  ...(r.n ? { pass: r.pass, r2: r.r2, r2Ols: r.r2Ols, olsSlope: r.olsSlope, olsIntercept: r.olsIntercept, b0: r.b0, slope: r.slope, r2OriginCentred: r.r2OriginCentred, r2OriginExcel: r.r2OriginExcel, totObs: r.totObs, totMod: r.totMod, labels: r.labels } : {}) }) : null;
const viewOut = (bv) => ({ same: bv.same, id: bv.view.id, name: bv.view.name, year: bv.view.year ?? null, zones: Object.keys(bv.view.zones), links: Object.keys(bv.view.links), resKeys: Object.keys(bv.view.res || {}) });
if (!only || only === 'cases') {
  cases.project = [];
  for (const [fixture, tag, popts] of [['corridor', 'corridor', {}], ['grid', 'grid', {}], ['grid', 'grid-existing', { existing: true }]]) {
    const prep = await prepare(fixture, popts), { c, P, sc } = prep;
    const rec = { fixture: tag, projectText: prep.text, matches: matchOut(c.DataHub.matches(sc)), calibration: {}, baseViews: [] };
    for (const period of ['AM', 'PM']) {
      sc.res[period] = await c.Engine.assign(sc, { period });
      c.Store.ver++;
      rec.calibration[period] = plain(calOut(c.DataHub.calibration(sc, period)));
    }
    // matches with a different match radius and with the counts' states changed
    P.settings.data.matchRadius = 15; c.Store.ver++;
    rec.matchesR15 = matchOut(c.DataHub.matches(sc));
    P.settings.data.matchRadius = 120; c.Store.ver++;
    rec.matchesR120 = matchOut(c.DataHub.matches(sc));
    P.settings.data.matchRadius = 40; c.Store.ver++;
    Object.values(sc.links)[1].status = 'closed'; Object.values(sc.links)[2].lanesBA = 0; Object.values(sc.links)[3].cls = 'ramp'; c.Store.ver++;
    rec.matchesEdited = matchOut(c.DataHub.matches(sc));
    // base views: the scenario as it is, with a proposed link, with a zone opening after the base year, with year before
    const bvs = [['as-is', () => {}]];
    bvs.push(['proposed-link', s => { Object.values(s.links)[0].status = 'proposed'; }]);
    bvs.push(['late-zone', s => { const z = Object.values(s.zones).find(z => z.kind !== 'gateway'); z.year = 2030; z.existing = true; }]);
    bvs.push(['zone-year-ok', s => { const z = Object.values(s.zones).find(z => z.kind !== 'gateway'); z.year = 2020; z.existing = true; }]);
    bvs.push(['scenario-year', s => { s.year = 2026; }]);
    bvs.push(['scenario-year-text', s => { s.year = 2031; s.name = null; }]);
    for (const [bn, f] of bvs) {
      const o = open(prep.text); f(o.sc); o.c.Store.ver++;
      rec.baseViews.push({ name: bn, out: plain(viewOut(o.c.Calibrate.baseView(o.sc))) });
    }
    // the report with no result, with too few counts and for the project as loaded
    { const o = open(prep.text); rec.reportNoRes = plain(reportOut(o.c.Calibrate.report(o.sc, 'AM')));
      o.sc.res.AM = await o.c.Engine.assign(o.sc, { period: 'AM' }); rec.reportRun = reportOut(o.c.Calibrate.report(o.sc, 'AM'));
      const d = o.c.DataHub.D(); d.counts = d.counts.filter(x => x.src === 'user').slice(0, 5); o.c.Store.ver++;
      rec.reportFew = reportOut(o.c.Calibrate.report(o.sc, 'AM'));
      const r1 = o.sc.res.AM; r1.stale = true; r1.converged = false; r1.convNote = 'Stopped at 20 iterations (gap 0.5%).'; o.c.Store.ver++;
      rec.reportStale = reportOut(o.c.Calibrate.report(o.sc, 'AM'));
      d.counts = d.counts.slice(0, 1); o.c.Store.ver++;
      rec.reportOne = reportOut(o.c.Calibrate.report(o.sc, 'AM'));
      r1.converged = true; r1.convNote = ''; r1.stale = false; o.P.settings.baseYear = '2026'; o.c.Store.ver++;
      rec.reportTextBase = reportOut(o.c.Calibrate.report(o.sc, 'AM'));
    }
    cases.project.push(rec);
  }
  // GEH, the TMDG and TAG flow bands (edges included) and the count needed for a share target
  {
    const { c: cs } = await prepare('corridor', {}), C = cs.Calibration;
    const flows = [0, 1, 5, 50, 99, 100, 101, 150, 199, 200, 699, 700, 701, 1000, 1500, 2000, 2699, 2700, 2701, 3000, 5000];
    const pairs = [];
    for (const c of flows) for (const dm of [-600, -400, -401, -300, -150, -100, -99, -15, -1, 0, 1, 15, 100, 101, 400, 401, 600]) pairs.push([Math.max(0, c + dm), c]);
    cases.geh = pairs.map(([m, c]) => [m, c, plain(C.geh(m, c))]);
    cases.bands = pairs.map(([m, c]) => [m, c, C.tmdgBand(m, c), C.tagBand(m, c)]);
    cases.gehOdd = [[NaN, 10], [10, NaN], [Infinity, 5], [-5, 5], [0.5, 0.25], [1e9, 1e9 + 1]].map(([m, c]) => [plain(m), plain(c), plain(C.geh(m, c)), C.tmdgBand(m, c), C.tagBand(m, c)]);
    cases.need = [[0.85, 20], [0.85, 7], [0.5, 1], [1, 5], [0.95, 20], [0.75, 4], [0.1, 0], [0, 10], [0.999999999, 10], [0.85, 1], [0.85, 100], [0.9, 10], [0.7, 10], [0.5, 2], [1.5, 4], [0.3, 3]].map(([t, n]) => [t, n, C.need(t, n)]);
  }
  write('calibrate-cases.json', cases);
}

/* ================= Part 2: features/calibration.js ================= */
// Screenlines, journey checks and turning counts added to a prepared project, then every panel function recorded
// on it in several states (as held out, after matrix estimation, after trip-end factors, another scenario, a drawn core).

const FIXTURE_LINES = {
  corridor: [
    { id: 'slc1', name: 'North side roads', type: 'screen', motorways: true, pts: [[-37.0990, 174.9540], [-37.0990, 174.9680]] },
    { id: 'slc2', name: 'South side roads', type: 'screen', role: 'valid', motorways: true, pts: [[-37.1010, 174.9540], [-37.1010, 174.9680]] },
    { id: 'slc3', name: 'Central cordon', type: 'cordon', role: 'valid', motorways: true, pts: [[-37.0990, 174.9570], [-37.0990, 174.9650], [-37.1010, 174.9650], [-37.1010, 174.9570]] },
    { id: 'slc4', name: 'East of centre', type: 'screen', motorways: false, pts: [[-37.0985, 174.9650], [-37.1015, 174.9650]] },
    { id: 'slc5', name: 'Whole corridor', type: 'screen', role: 'valid', motorways: true, pts: [[-37.0985, 174.9535], [-37.1015, 174.9535]] },
  ],
  grid: [
    { id: 'slg1', name: 'West to east', type: 'screen', motorways: true, pts: [[-36.8990, 174.7645], [-36.9082, 174.7645]] },
    { id: 'slg2', name: 'North to south', type: 'screen', role: 'valid', motorways: true, pts: [[-36.9012, 174.7590], [-36.9012, 174.7730]] },
    { id: 'slg3', name: 'Centre cordon', type: 'cordon', motorways: true, pts: [[-36.9010, 174.7625], [-36.9010, 174.7695], [-36.9065, 174.7695], [-36.9065, 174.7625]] },
    { id: 'slg4', name: 'Second cordon', type: 'cordon', role: 'valid', motorways: true, pts: [[-36.9065, 174.7625], [-36.9065, 174.7695], [-36.9010, 174.7695], [-36.9010, 174.7625]] },
    { id: 'slg5', name: 'South edge', type: 'screen', motorways: true, pts: [[-36.9078, 174.7590], [-36.9078, 174.7730]] },
  ],
};
const CORE_AREA = {
  corridor: { id: 'slcore', name: 'Zone area', type: 'core', pts: [[-37.0970, 174.9660], [-37.0970, 174.9700], [-37.0995, 174.9700], [-37.0995, 174.9660]] },
  grid: { id: 'slcore', name: 'Northern block', type: 'core', pts: [[-36.8995, 174.7595], [-36.8995, 174.7690], [-36.9040, 174.7690], [-36.9040, 174.7595]] },
};

const LEG_NAMES8 = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'];
const legOf = b => LEG_NAMES8[Math.round((((b % 360) + 360) % 360) / 45) % 8];
const PROFILE = [0.105, 0.12, 0.135, 0.145, 0.13, 0.12, 0.125, 0.12];
const hm = m => `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;

// CSV text of a turning count at a junction from the modelled turns, offset movement by movement (long layout)
function turnCsv(c, sc, res, nodeId, { layout = 'long', site = 'J1', name, date = '12/03/2025', only = null, scale = 1, legOnly = false } = {}) {
  const { Turns, U } = c, n = sc.nodes[nodeId];
  const list = (res.turns && res.turns[nodeId]) || [], A = Turns.arms(sc, nodeId, list), rows = Turns.movements(sc, res, nodeId);
  const arm = k => A.find(x => x.inKey === k);
  const mv = rows.filter(r => r.v > 0).map(r => ({ leg: legOf(arm(r.approach).b), road: arm(r.approach).road, mv: r.mv, v: r.v }));
  const facs = [1.0, 0.88, 1.18, 0.74, 1.3, 0.95, 1.07, 0.6, 1.12, 0.92];
  const out = [];
  const periods = [[7 * 60, 8], [16 * 60, 8]];
  const f = k => round(facs[k % facs.length] * scale, 3);
  if (layout === 'wide') {
    const legs = [...new Set(mv.map(m => m.leg + '_' + m.mv))];
    out.push(['site_id', 'site_name', 'lat', 'lng', 'date', 'time_start', 'interval_min', ...legs.flatMap(k => [k + '_LV', k + '_HV'])].join(','));
    for (const [t0, nb] of periods) for (let b = 0; b < nb; b++) out.push([site, name || `Junction ${site}`, n.lat, n.lng, date, hm(t0 + b * 15), 15, ...legs.flatMap((k, i) => { const m = mv.find(x => x.leg + '_' + x.mv === k); const q = m.v * PROFILE[b] * f(i); return [Math.round(q * 0.92), Math.round(q * 0.08)]; })].join(','));
    return out.join('\r\n');
  }
  const head = ['site_id', 'site_name', 'lat', 'lng', 'date', 'time_start', 'interval_min', 'from_leg', 'movement', 'class', 'count', 'from_road'];
  if (layout === 'scats') head.splice(4, 0, 'detector');
  out.push(head.join(','));
  const push = (b, t0, leg, mvv, cls, q, road) => out.push(layout === 'scats'
    ? [site, name || `Junction ${site}`, n.lat, n.lng, 'D' + (1 + (b % 3)), date, hm(t0 + b * 15), 15, leg, mvv, cls, q, road].join(',')
    : [site, name || `Junction ${site}`, n.lat, n.lng, date, hm(t0 + b * 15), 15, leg, mvv, cls, q, road].join(','));
  if (legOnly) {
    const ins = {}, outs = {};
    for (const m of mv) ins[m.leg] = (ins[m.leg] || 0) + m.v;
    // out totals: vehicles leaving each leg (the exit arm's leg)
    for (const r of rows.filter(r => r.v > 0)) { const x = A.find(a => a.outKey === r.to); if (x) outs[legOf(x.b)] = (outs[legOf(x.b)] || 0) + r.v; }
    for (const [t0, nb] of periods) for (let b = 0; b < nb; b++) {
      for (const [leg, v] of Object.entries(ins)) push(b, t0, leg, 'IN', 'LV', Math.round(v * PROFILE[b] * 0.93 * scale), '');
      for (const [leg, v] of Object.entries(outs)) push(b, t0, leg, 'OUT', 'LV', Math.round(v * PROFILE[b] * 0.93 * scale), '');
    }
    return out.join('\n');
  }
  for (const [t0, nb] of periods) for (let b = 0; b < nb; b++) mv.forEach((m, i) => {
    if (only && !only.includes(m.mv)) return;
    const q = m.v * PROFILE[b] * f(i);
    if (layout === 'scats' && m.mv === 'L') { push(b, t0, m.leg, 'LT', 'LV', Math.round(q * 0.9 + (mv.find(x => x.leg === m.leg && x.mv === 'T') ? mv.find(x => x.leg === m.leg && x.mv === 'T').v * PROFILE[b] : 0)), m.road); return; }
    if (layout === 'scats' && m.mv === 'T' && mv.some(x => x.leg === m.leg && x.mv === 'L')) return;
    push(b, t0, m.leg, m.mv, 'LV', Math.round(q * 0.92), m.road);
    push(b, t0, m.leg, m.mv, 'HV', Math.round(q * 0.08), m.road);
  });
  return out.join('\n');
}

// Journey checks between the gateways' nodes: observed times at fixed offsets from the model, some only Google, one with nothing
async function addJourneys(c, P, sc) {
  const gws = Object.values(sc.zones).filter(z => z.kind === 'gateway' && z.node);
  const d = c.DataHub.D(); d.jt = [];
  const res = { AM: await c.Engine.assign(sc, { period: 'AM' }), PM: await c.Engine.assign(sc, { period: 'PM' }) };
  const pairs = [[0, 1], [1, 0], [0, gws.length - 1], [2 % gws.length, 1], [gws.length - 1, 0]].filter(([a, b]) => a !== b && gws[a] && gws[b]);
  pairs.forEach(([a, b], k) => {
    const j = { id: 'jt' + k, name: `${gws[a].name || gws[a].id} to ${gws[b].name || gws[b].id}`, from: gws[a].node, to: gws[b].node };
    const tAM = c.Engine.travel(sc, res.AM, j.from, j.to), tPM = c.Engine.travel(sc, res.PM, j.from, j.to);
    if (k % 5 === 0) j.obs = { AM: tAM && tAM.time ? round(tAM.time * 1.08) : null, PM: tPM && tPM.time ? round(tPM.time * 0.8) : null, src: 'GPS survey' };
    else if (k % 5 === 1) j.obs = { AM: tAM && tAM.time ? round(tAM.time * 1.4) : null, PM: 0 };
    else if (k % 5 === 2) j.g = { AM: { t: tAM && tAM.time ? round(tAM.time * 1.05) : 0, when: 'Tue 10 Mar, 8:00 am' }, PM: { t: tPM && tPM.time ? round(tPM.time * 1.3) : 0, when: 'Tue 10 Mar, 5:00 pm' } };
    else if (k % 5 === 3) { j.obs = { AM: tAM && tAM.time ? round(tAM.time * 0.97) : null }; j.g = { PM: { t: 800, when: 'x' } }; }
    d.jt.push(j);
  });
  d.jt.push({ id: 'jtx', name: 'Nowhere', from: 'nope', to: 'nada', obs: { AM: 300, PM: 300 } });
}

// What the panel adds to a prepared project: screenlines, a core area (kept in a separate copy), journeys, turning counts
async function addCalibration(prep, fixture) {
  const { c, P, sc } = prep;
  const st = c.Calibration.store();
  st.screenlines = FIXTURE_LINES[fixture].map(x => JSON.parse(JSON.stringify(x)));
  await addJourneys(c, P, sc);
  const res = { AM: await c.Engine.assign(sc, { period: 'AM' }), PM: await c.Engine.assign(sc, { period: 'PM' }) };
  const deg = c.Net.degree(sc), junctions = Object.keys(sc.nodes).filter(id => (deg.get(id) || 0) >= 3);
  const siteNodes = Object.values(sc.sites || {}).flatMap(s => s.nodes);
  const picks = [...new Set([...siteNodes.slice(0, 2), ...junctions.filter((id, i) => i % 2 === 0)])].slice(0, 6);
  const texts = [];
  picks.forEach((id, k) => {
    const layout = ['long', 'wide', 'scats', 'long', 'long', 'wide'][k], legOnly = k === 3, date = ['12/03/2025', '2025-03-15', '11-3-25', '15/04/2025', '12.03.2025', '2025-03-12'][k];
    texts.push({ file: `site${k}.csv`, node: id, layout, legOnly, text: turnCsv(c, sc, res.AM, id, { layout, legOnly, site: 'J' + k, date, scale: 1 + k * 0.04 }) });
  });
  // turning counts are imported through the panel's own importer (the ids it makes come from the seeded generator)
  const imported = [];
  for (const t of texts) { const r = c.Calibration.importTurnCSV(t.text, t.file); imported.push({ file: t.file, sites: r.sites.map(s => s.id), skipped: r.skipped, reasons: r.reasons }); }
  c.Calibration.matchSites(sc);
  // a model volumes site (not a count) and a site matched to nothing
  const far = turnCsv(c, sc, res.AM, picks[0], { site: 'FAR', date: '12/03/2025' }).split('\n').map((l, i) => i === 0 ? l : l.replace(/,[-\d.]+,[-\d.]+,/, ',-35.5,174.1,')).join('\n');
  const rFar = c.Calibration.importTurnCSV(far, 'far.csv');
  c.Calibration.matchSites(sc);
  return { texts, imported, picks, far, farIds: rFar.sites.map(s => s.id) };
}

/* ---------- projections (what is compared): the same shapes the Rust test builds ---------- */
const P_ = plain;
const countRowOut = r => ({ id: r.c.id, link: r.mt.link, dir: r.mt.dir, d: r.mt.d, obs: r.obs, model: r.model, how: r.how, geh: r.geh, method: r.method, role: r.role, scored: r.scored, inCore: r.inCore, mw: r.mw, cls: r.L ? r.L.cls : null });
const slRowOut = r => P_({ ...r, sl: undefined });
const slObsOut = s => P_({ n: s.n, cnt: s.cnt, rows: s.rows.map(r => ({ slId: r.sl.id, ...r, sl: undefined })), dropped: s.dropped, ...Object.fromEntries(['sgeh5', 'sgeh75', 'sgeh10', 'sgeh4', 'sw5', 'sw10', 'sw15'].map(k => [k, s[k]])) });
const jrOut = r => P_({ name: r.j.name, model: r.model, len: r.len, obs: r.obs ? { t: r.obs.t, label: r.obs.label } : null, google: r.google ? { t: r.google.t, when: r.google.when ?? null } : null, chk: r.chk, refObserved: !!r.obs, short: r.short, long: r.long });
const convOut = v => P_(v.n ? { n: v.n, gapPct: v.gapPct, gap: v.gap, it: v.it, converged: v.converged, gapTol: v.gapTol, note: v.note, p4: v.p4, lastP: v.lastP } : { n: 0 });
const meOut = m => P_(m.n ? { n: m.n, totalAbs: m.totalAbs, totalPct: m.totalPct, cells: m.cells, ends: m.ends, tld: m.tld, cr: m.cr } : { n: 0 });
const turnRowOut = r => P_({ siteId: r.s.id, leg: r.leg, mv: r.mv, key: r.key, obs: r.obs, model: r.model, geh: r.geh, arm: r.arm, estimated: r.estimated, balanced: r.balanced });
const coreOut = k => P_({ kind: k.kind, label: k.label, r: k.r ?? null, lines: k.lines ? k.lines.length : null, polyN: k.poly ? k.poly.length : null });
const evalOut = e => { const o = { id: e.t.id, n: e.n, value: e.value ?? null, got: e.got, need: e.need, ok: e.ok, status: e.status, shown: e.shown }; for (const k of ['predicted', 'clarify', 'verdict', 'indicative']) if (e[k] !== undefined) o[k] = e[k]; return P_(o); };
const testOut = t => P_({ id: t.id, group: t.group, on: t.on, stat: t.stat, kind: t.kind, min: t.min ?? null, max: t.max ?? null, lo: t.lo ?? null, hi: t.hi ?? null, clarify: t.clarify ?? null, label: t.label, crit: t.crit, table: t.table, src: t.src, use: t.use, verified: t.verified, useVerified: t.useVerified ?? null });
const evOut = (ev, withAll = false) => P_({
  period: ev.period, set: ev.set, requested: ev.requested, area: ev.area, core: coreOut(ev.core), infoOnly: ev.infoOnly, fallback: ev.fallback, validN: ev.validN,
  rows: ev.rows.map(r => r.c.id), all: withAll ? ev.all.map(countRowOut) : ev.all.map(r => r.c.id), link: ev.link, measured: ev.measured, turn: ev.turn, turnRows: ev.turnRows.map(turnRowOut), screen: slObsOut(ev.screen),
  jt: ev.jt, jtRows: ev.jtRows.map(jrOut), conv: convOut(ev.conv), me: meOut(ev.me), baseYear: ev.baseYear, year: ev.year, hasRes: !!ev.res,
});
const assessOut = a => P_({ period: a.period, standard: a.standard, category: a.category, status: a.status, pass: a.pass, line: a.line, also: a.also, fails: a.fails, rows: a.rows.map(evalOut) });
// The Furness matrix keyed by leg (the Rust port keeps its stored keys in another order, so rows and columns are compared by name)
const legsOf = (s, period) => { const pk = s.peak && s.peak[period]; return pk ? [...new Set(Object.keys(pk.mv).map(k => k.split('-')[0]))] : []; };
const furnessOut = (f, legs) => ({ it: f.it, err: f.err, converged: f.converged, outsScaled: f.outsScaled, m: Object.fromEntries(legs.map((li, i) => [li, Object.fromEntries(legs.map((lj, j) => [lj, f.m[i][j]]))])) });
const siteRowsOut = (r, s, period) => P_({ siteId: s.id, node: r.node, rows: r.rows.map(turnRowOut), flags: r.flags ?? null, note: r.note, kind: r.view ? r.view.kind : null, members: r.view ? [...r.view.members] : null,
  arms: r.view ? r.view.arms.map(a => ({ inKey: a.inKey, outKey: a.outKey, b: a.b, road: a.road, links: [...a.links] })) : null, vrows: r.view ? r.view.rows.map(m => ({ approach: m.approach, to: m.to, mv: m.mv, turn: m.turn ?? null, v: m.v })) : null,
  vol: r.vol ? { mv: r.vol.mv, estimated: r.vol.estimated, balanced: r.vol.balanced, furness: r.vol.furness ? furnessOut(r.vol.furness, legsOf(s, period)) : null } : null });
const roleMap = (c, P, period, raw) => Object.fromEntries(c.DataHub.D().counts.map(x => [x.id, c.Calibration.roleOf(x, period, raw)]));
const storeOut = c => P_({ roles: c.Calibration.store().roles });

// Everything the panel computes for a scenario's results, in one record
async function snapshot(c, P, sc, label, { full = true, level = 'mid' } = {}) {
  const C = c.Calibration, rec = { label, scenario: sc.id, periods: {} };
  c.Store.ver++;
  rec.roles = { AM: roleMap(c, P, 'AM', false), PM: roleMap(c, P, 'PM', false), rawAM: roleMap(c, P, 'AM', true) };
  rec.methods = Object.fromEntries(c.DataHub.D().counts.map(x => [x.id, { AM: C.methodOf(x, 'AM'), PM: C.methodOf(x, 'PM') }]));
  rec.gatewayIds = [...C.gatewayIds()];
  rec.core = coreOut(C.coreArea(sc));
  rec.store = JSON.parse(JSON.stringify(C.store()));
  for (const period of ['AM', 'PM']) {
    const res = c.Store.res(sc, period), pr = rec.periods[period] = { hasRes: !!res };
    if (!res) continue;
    pr.rows = C.countRows(sc, period, res).map(countRowOut);
    pr.rowsRaw = C.countRows(sc, period, res, true).map(countRowOut);
    pr.stats = { all: P_(C.statsOf(C.countRows(sc, period, res))), scored: P_(C.statsOf(C.countRows(sc, period, res).filter(r => r.scored))) };
    pr.sl = {};
    for (const sl of C.store().screenlines) {
      pr.sl[sl.id] = { crossings: C.slCrossings(sc, sl).map(x => P_({ link: x.link, side: x.side, ll: x.ll, bAB: x.bAB, mw: x.mw })),
        rows: C.slRows(sc, sl, period, res).map(slRowOut), rowsNoMw: C.slRows(sc, sl, period, res, { motorways: false }).map(slRowOut) };
    }
    pr.slObs = Object.fromEntries(['valid', 'calib', 'all'].map(set => [set, slObsOut(C.slObservations(sc, period, set))]));
    pr.journeys = C.journeys(sc, period, res).map(jrOut);
    pr.jtStats = P_(C.jtStats(C.journeys(sc, period, res)));
    pr.turn = {};
    for (const s of C.store().turnSites) pr.turn[s.id] = siteRowsOut(C.siteRows(sc, s, period, res), s, period);
    pr.network = P_({ far: C.networkChecks(sc, period).far.map(countRowOut), worst: C.networkChecks(sc, period).worst.map(countRowOut), mwZones: C.networkChecks(sc, period).mwZones.map(z => z.id), noDelay: C.networkChecks(sc, period).noDelay.map(x => ({ key: x.key, link: x.L.id, vc: x.vc })) });
    if (!full) continue;
    pr.evidence = {};
    const sets = level === 'max' ? ['valid', 'calib', 'all'] : level === 'mid' ? ['valid', 'calib'] : ['valid'];
    const areas = level === 'max' ? ['auto', 'all', 'inside', 'outside'] : level === 'mid' ? ['auto', 'all'] : ['auto'];
    for (const set of sets) for (const area of areas) {
      pr.evidence[`${set}|${area}`] = evOut(C.evidence(sc, period, { set, area }), set === 'valid' && area === 'auto');
      if (set === 'valid' && area === 'auto') pr.evidence['valid|auto|strict'] = evOut(C.evidence(sc, period, { set, area, strict: true }));
    }
    if (level !== 'min') pr.evidence['valid|auto|raw'] = evOut(C.evidence(sc, period, { set: 'valid', raw: true }));
    pr.assess = {};
    const stds = level === 'max' ? ['tmdg', 'rms', 'atap'] : ['tmdg', 'rms'];
    for (const std of stds) for (const cat of (std === 'tmdg' ? (level === 'max' ? ['A', 'B', 'C', 'D', 'E', 'F', 'G'] : ['A', 'E', 'F']) : ['E'])) {
      for (const set of std === 'tmdg' && cat === 'E' ? ['valid', 'calib', 'all'] : ['valid']) pr.assess[`${std}|${cat}|${set}`] = assessOut(C.assess(sc, period, { standard: std, category: cat, set }));
    }
    pr.assess['default'] = assessOut(C.assess(sc, period));
    pr.assess['valid|strict|auto'] = assessOut(C.assess(sc, period, { set: 'valid', strict: true, area: 'auto' }));
  }
  if (full && level === 'max') {
    rec.tests = {};
    for (const std of ['tmdg', 'rms', 'atap']) for (const cat of (std === 'tmdg' ? ['A', 'B', 'C', 'D', 'E', 'F', 'G'] : ['E'])) rec.tests[`${std}|${cat}`] = C.tests(std, cat).map(testOut);
    rec.tagPeers = Object.fromEntries(C.tests('tmdg', 'E').map(t => [t.id, (() => { const p = C.tagPeer(t); return p ? testOut(p) : null; })()]));
    rec.targetText = Object.fromEntries(C.tests('tmdg', 'E').map(t => [t.id, C.targetText(t)]));
    rec.tooltips = Object.fromEntries(C.tests('tmdg', 'E').concat(C.tests('tmdg', 'A')).map(t => [t.id, C.tooltip(t)]));
  }
  if (full) rec.csv = { counts: C.countsCSV(sc), summary: C.summaryCSV(sc), screenlines: C.screenlinesCSV(sc), turns: C.turnsCSV(sc) };
  return rec;
}

/* ---------- the states a project goes through ---------- */
const savedStr = v => JSON.parse(JSON.stringify(v ?? null));
const runBoth = async (c, sc) => { for (const p of ['AM', 'PM']) sc.res[p] = await c.Engine.assign(sc, { period: p }); c.Store.ver++; };

async function calibrationStates(fixture, tag, popts, only = null) {
  const prep = await prepare(fixture, popts), { c, P, sc } = prep;
  const added = await addCalibration(prep, fixture.replace('-existing', ''));
  const text0 = c.Store.serialise(P);
  const out = { fixture: tag, scenario: sc.id, projectText: text0, texts: added.texts, imported: added.imported, far: added.far, farIds: added.farIds, coreArea: CORE_AREA[fixture.replace('-existing', '')], states: [] };
  const state = async (name, fn, level = 'mid') => {
    if (only && !only.includes(name)) return;
    const rec = { name };
    // each state starts from the text again in its own context, so nothing leaks between them
    const o = open(text0), C = o.c.Calibration;
    await runBoth(o.c, o.sc);
    await fn(o.c, o.P, o.sc, rec, C);
    rec.level = level;
    rec.snapshot = await snapshot(o.c, o.P, o.sc, name, { level });
    out.states.push(rec);
  };
  await state('as-prepared', async (c, P, sc, rec, C) => { rec.setup = []; });
  await state('held-out', async (c, P, sc, rec, C) => { rec.holdOut = C.holdOut(sc); rec.setup = ['holdOut']; rec.afterRoles = savedStr(C.store().roles); }, 'max');
  await state('roles-by-hand', async (c, P, sc, rec, C) => {
    const ids = c.DataHub.D().counts.map(x => x.id);
    C.setRole(ids[0], 'valid'); C.setRole(ids[1], 'off'); C.setRole(ids[2], 'valid'); C.setRole(ids[3], 'gateway'); C.setRole(ids[4], 'me'); C.setRole(ids[5], 'bogus'); C.setRole(ids[2], 'valid');
    rec.setup = ['roles']; rec.afterRoles = savedStr(C.store().roles);
    rec.counts = c.DataHub.D().counts.map(x => [x.id, x.role ?? null]);
    C.holdOut(sc); const r1 = savedStr(C.store().roles); C.setRole(ids[1], 'off'); C.holdOut(sc); rec.afterHoldOutKeepingOff = savedStr(C.store().roles); rec.holdOutOnlyOff = r1;
    C.resetRoles(); rec.afterReset = savedStr(C.store().roles); rec.countsAfterReset = c.DataHub.D().counts.map(x => [x.id, x.role ?? null]);
    C.holdOut(sc, 3); rec.afterHoldOut3 = savedStr(C.store().roles);
  });
  await state('after-me', async (c, P, sc, rec, C) => {
    C.holdOut(sc);
    const prog = [];
    const r = await C.runME(sc, { period: 'AM', onProgress: (k, n, m) => prog.push([k, n, m]) });
    rec.runMe = { ok: r.ok, note: r.note, out: r.out ? fitOut(r.out) : null, progress: P_(prog) };
    rec.me = savedStr(P.me);
    rec.meReport = savedStr(C.meReport(sc, 'AM'));
    rec.meStats = meOut(C.meStats('AM'));
    const r2 = await C.reduceME(sc, { period: 'PM', from: 2 });
    rec.reduceMe = { ok: r2.ok, note: r2.note, tried: r2.tried, out: r2.out ? fitOut(r2.out) : null };
    rec.me2 = savedStr(P.me);
    rec.setup = ['holdOut', 'runME AM', 'reduceME PM'];
    for (const p of ['AM', 'PM']) sc.res[p] = await c.Engine.assign(sc, { period: p });
    c.Store.ver++;
    rec.validation = {};
    rec.validation.recorded = { AM: savedStr(C.recordValidation(sc, 'AM')), PM: savedStr(C.recordValidation(sc, 'PM')) };
    rec.validation.refreshed = C.refreshValidation(P); rec.validation.P = savedStr(P.validation);
    rec.reportScenario = C.reportScenario(P).id;
  });
  await state('after-tef', async (c, P, sc, rec, C) => {
    C.holdOut(sc);
    const prog = [];
    const r = await C.tripEndFactors(sc, { period: 'AM', onProgress: (k, n, m) => prog.push([k, n, m]) });
    rec.tef = savedStr(r); rec.tefProgress = P_(prog); rec.sc_calib = savedStr(sc.calib);
    rec.canApply = savedStr(C.canApply(sc, 'AM'));
    rec.applied = C.applyFactors(sc, 'AM'); rec.luFactors = savedStr(P.luFactors); rec.scCalibApplied = savedStr(sc.calib);
    rec.canApplyAfter = savedStr(C.canApply(sc, 'AM'));
    rec.staleAfter = P.scenarios.map(s => Object.fromEntries(Object.entries(s.res || {}).map(([k, v]) => [k, v ? !!v.stale : null])));
    rec.setup = ['holdOut', 'tripEndFactors AM', 'applyFactors AM'];
    for (const p of ['AM', 'PM']) sc.res[p] = await c.Engine.assign(sc, { period: p });
    c.Store.ver++;
    rec.revert = C.revertFactors(sc, 'AM'); rec.luAfterRevert = savedStr(P.luFactors); rec.scCalibReverted = savedStr(sc.calib);
    C.applyFactors(sc, 'AM'); rec.setupApplyAgain = true;
    for (const p of ['AM', 'PM']) sc.res[p] = await c.Engine.assign(sc, { period: p });
    // trip-end factors with no calibration role left
    const tef2 = await C.tripEndFactors(sc, { period: 'PM' }); rec.tefPM = savedStr(tef2); rec.canApplyPM = savedStr(C.canApply(sc, 'PM'));
    rec.canApplyNone = savedStr(C.canApply(sc, 'PM')); rec.canApplyNoRec = C.canApply(P.scenarios[P.scenarios.length - 1], 'AM');
    c.Store.ver++;
  });
  await state('tef-screenlines-off', async (c, P, sc, rec, C) => {
    // no calibration screenlines: count groups; then none at all
    C.store().screenlines = C.store().screenlines.filter(s => s.type === 'core' || s.role === 'valid');
    const r = await C.tripEndFactors(sc, { period: 'AM' }); rec.tef = savedStr(r);
    for (const id of c.DataHub.D().counts.map(x => x.id)) C.setRole(id, 'valid');
    const r2 = await C.tripEndFactors(sc, { period: 'PM' }); rec.tefNone = savedStr(r2);
    for (const id of c.DataHub.D().counts.map(x => x.id)) C.setRole(id, 'off');
    const r3 = await C.runME(sc, { period: 'AM' }); rec.runMeNone = { ok: r3.ok, note: r3.note };
    const r4 = await C.reduceME(sc, { period: 'AM' }); rec.reduceNone = { ok: r4.ok, note: r4.note, tried: r4.tried };
    rec.setup = ['screenlines: validation only', 'tripEndFactors', 'all valid', 'tripEndFactors', 'all off', 'runME', 'reduceME'];
    C.resetRoles();
  });
  await state('other-scenario', async (c, P, sc, rec, C) => {
    const s2 = P.scenarios[1];
    Object.values(s2.links)[2].lanesAB = 3; Object.values(s2.links)[2].speed = 70; delete s2.links[Object.keys(s2.links).pop()];
    s2.year = c.Store.S.baseYear;
    for (const p of ['AM', 'PM']) s2.res[p] = await c.Engine.assign(s2, { period: p });
    c.Store.ver++;
    rec.scenarioIndex = 1; rec.setup = ['scenario 1 edited'];
    rec.core2 = coreOut(C.coreArea(s2));
    rec.snap2 = await snapshot(c, P, s2, 'scenario 1', { full: true });
    rec.cordon = {};
    for (const sl of C.store().screenlines) { const r = C.cordonOD(s2, c.Store.res(s2, 'AM'), sl); rec.cordon[sl.id] = r.error ? { error: r.error } : { o: r.o, csv: r.csv, geojson: JSON.parse(r.geojson), zones: r.zones, boundary: r.boundary, trips: r.trips }; }
    rec.cordonNoRes = C.cordonOD(s2, null, C.store().screenlines.find(s => s.type === 'cordon'));
    rec.cordonNoCordon = C.cordonOD(s2, c.Store.res(s2, 'AM'), null);
  });
  await state('core-drawn', async (c, P, sc, rec, C) => {
    C.store().screenlines.push(JSON.parse(JSON.stringify(CORE_AREA[fixture.replace('-existing', '')])));
    C.holdOut(sc);
    c.Store.P.settings.calib = { coreBuffer: 300 };
    rec.setup = ['core area drawn', 'holdOut', 'coreBuffer 300'];
  });
  await state('settings', async (c, P, sc, rec, C) => {
    P.settings.tmdg = { category: 'c', justification: 'Corridor assessment' };
    P.settings.calib = { standard: 'rms', set: 'all', area: 'all', minComplete: 0.95, priorQuality: 'low', xamax: 1.5, freezeGG: false, windows: { AM: ['07:30', '08:30'], PM: ['16:30', '17:30'] } };
    P.settings.data.amShare = 9; P.settings.data.pmShare = 10;
    C.holdOut(sc); rec.setup = ['settings changed', 'holdOut', 'repeak'];
    rec.repeaked = C.repeak(P); rec.peaks = savedStr(C.store().turnSites.map(s => ({ id: s.id, peak: s.peak, peakWin: s.peakWin, bal: s.bal ?? null })));
    rec.repeakedAgain = C.repeak(P);
  }, 'max');
  await state('balanced', async (c, P, sc, rec, C) => {
    rec.balance = savedStr(C.balance(sc, 'AM'));
    rec.balStore = savedStr(C.store().turnSites.map(s => ({ id: s.id, node: s.node, bal: s.bal, audit: s.audit })));
    rec.balanceNoSinks = savedStr(C.balance(sc, 'PM', { noSinks: true }));
    rec.balStore2 = savedStr(C.store().turnSites.map(s => ({ id: s.id, bal: s.bal, audit: s.audit })));
    rec.balanceAgain = savedStr(C.balance(sc, 'AM', { noSinks: false }));
    rec.phf = Object.fromEntries(Object.keys(sc.nodes).map(id => [id, { AM: C.phf(id, 'AM', sc), PM: C.phf(id, 'PM', sc) }]).filter(([, v]) => v.AM != null || v.PM != null));
    for (const s of C.store().turnSites.slice(0, 1)) s.role = 'off';
    rec.phfOff = Object.fromEntries(Object.keys(sc.nodes).map(id => [id, C.phf(id, 'AM', sc)]).filter(([, v]) => v != null));
    C.store().turnSites[0].role = 'valid';
    rec.setup = ['balance AM', 'balance PM noSinks', 'balance AM'];
  });
  await state('no-sites', async (c, P, sc, rec, C) => {
    sc.sites = {}; C.matchSites(sc); rec.setup = ['sites removed', 'matchSites'];
    rec.nodes = savedStr(C.store().turnSites.map(s => [s.id, s.node, s.site]));
  });
  await state('few-counts', async (c, P, sc, rec, C) => {
    const d = c.DataHub.D(); d.counts = d.counts.filter(x => x.src === 'user').slice(0, 6);
    for (const s of C.store().turnSites) s.role = 'off';
    C.holdOut(sc); rec.setup = ['six user counts', 'turn sites off', 'holdOut'];
  });
  await state('gateway-counts', async (c, P, sc, rec, C) => {
    // gateway flows set from the nearest counts (DataHub.gatewaysFromCounts writes each gateway's gwSrc)
    rec.gatewaysSet = c.DataHub.gatewaysFromCounts(sc);
    rec.gwSrc = Object.fromEntries(Object.values(sc.zones).filter(z => z.gwSrc).map(z => [z.id, z.gwSrc]));
    C.holdOut(sc); rec.setup = ['gatewaysFromCounts', 'holdOut'];
    for (const p of ['AM', 'PM']) sc.res[p] = await c.Engine.assign(sc, { period: p });
    c.Store.ver++;
  });
  return out;
}

if (!only || only === 'panel') {
  for (const [fixture, tag, popts, file, only] of [['corridor', 'corridor', { existing: true, year: 2026 }, 'calibrate-panel-corridor.json', null], ['corridor', 'corridor-forecast', { existing: true }, 'calibrate-panel-corridor-forecast.json', ['as-prepared', 'held-out', 'after-me']], ['grid', 'grid-existing', { existing: true }, 'calibrate-panel-grid.json', null]]) {
    write(file, await calibrationStates(fixture, tag, popts, only));
  }
}

/* ---------- turning-count files, one function at a time ---------- */
// Reading counts: long, wide, SCATS and leg-total files; 5, 10, 15, 30 and 60 minute intervals; date forms; bad rows
function longCsv(rows, head = ['site_id', 'site_name', 'lat', 'lng', 'date', 'time_start', 'interval_min', 'from_leg', 'movement', 'class', 'count', 'from_road']) {
  return [head.join(','), ...rows.map(r => r.join(','))].join('\n');
}
function fullHour(site, name, lat, lng, date, t0, legs, { interval = 15, cls = 'LV', road = true, steps = 8 } = {}) {
  const rows = [];
  for (let b = 0; b < steps; b++) for (const [leg, mv, q] of legs) rows.push([site, name, lat, lng, date, hm(t0 + b * interval), interval, leg, mv, cls, Math.round(q * PROFILE[b % 8]), road ? 'Road ' + leg : '']);
  return rows;
}
const LEGS3 = [['N', 'L', 120], ['N', 'T', 540], ['N', 'R', 90], ['S', 'L', 60], ['S', 'T', 480], ['S', 'R', 140], ['E', 'L', 70], ['E', 'T', 210], ['E', 'R', 85], ['W', 'L', 110], ['W', 'T', 190], ['W', 'R', 40]];

function turnImportCases() {
  const out = { toMin: [], isoDate: [], classOf: [], parse: [], peaks: [], furness: [], windows: [] };
  const c = loadAll({ seed: 5 }), C = c.Calibration;
  for (const t of ['07:15', '7.45', '0745', '745', '7:15 am', '7:15 pm', '12:05 am', '12:05 pm', '13:30 pm', '0:30 am', '2026-03-10 07:15:00', '10/03/2026 7:15', '2026-03-10T08:45:10.5', '25:00', '07:60', '', 'x', '8', '24:00', '23:59', '1.30', 'a 07:15 b 08:30 c', '7:15:30 PM', '7:15 a.m.', '07:15\t', null, undefined, 745, 7.3, '1234', '12345', '9:5']) out.toMin.push({ t: t === undefined ? '__undefined__' : t, out: C.toMin(t) });
  for (const v of ['2026-03-10', '2026/3/10', '10/03/2026', '10-3-26', '10.03.2026', '2026-02-30', '31/04/2026', '29/02/2024', '29/02/2025', '', null, undefined, 'x', '2026-3-1x', '2026-03-101', '1/1/99', '1/1/1999', '01/01/0026', '2026-13-01', '0/1/2026', '10/03/2026 7:15', '10 March 2026', '5/6/123', '5/6/12345', ' 2026-03-10 ']) out.isoDate.push({ v: v === undefined ? '__undefined__' : v, out: C.isoDate(v) });
  for (const v of ['LV', 'car', ' Cars ', 'Light', 'HCV-II', 'hcv_i', 'MCV', 'Bus', 'COACH', 'cycle', 'Bike', 'motor cycle', 'truck', 'x', '', null, 'hcvii', 'taxi', 'MC', 'H.V']) out.classOf.push({ v, out: C.classOf(v) });
  const P = (name, text, extra = {}) => {
    const rec = { name, text, ...extra };
    const ids = [];
    c.U.uid = (p = '') => { const id = p + 'x' + ids.length; ids.push(id); return id; };
    try { const r = C.parseTurnCSV(text, 'f.csv'); rec.out = { sites: JSON.parse(JSON.stringify(r.sites)), skipped: r.skipped, skippedRows: r.skippedRows, reasons: r.reasons, fromModel: r.fromModel }; }
    catch (e) { rec.error = e.message; }
    rec.ids = [...ids];
    out.parse.push(rec);
  };
  c.Store.P = c.newProject('t');
  const H = (extra = '') => ['site_id', 'site_name', 'lat', 'lng', 'date', 'time_start', 'interval_min', 'from_leg', 'movement', 'class', 'count', 'from_road'].join(',') + extra;
  P('long-15', longCsv(fullHour('A', 'Great South Rd / Mill Rd', -36.9, 174.76, '12/03/2025', 7 * 60, LEGS3)));
  P('long-15-pm-too', longCsv([...fullHour('A', 'Site A', -36.9, 174.76, '12/03/2025', 7 * 60, LEGS3), ...fullHour('A', 'Site A', -36.9, 174.76, '12/03/2025', 16 * 60, LEGS3, { cls: 'HV' })]));
  P('long-5min', longCsv(fullHour('B', 'Five minute', -36.9, 174.76, '2025-03-12', 7 * 60, LEGS3.map(([a, b, q]) => [a, b, q / 3]), { interval: 5, steps: 24 })));
  P('long-10min', longCsv(fullHour('B', 'Ten minute', -36.9, 174.76, '2025-03-12', 7 * 60, LEGS3.map(([a, b, q]) => [a, b, q / 1.5]), { interval: 10, steps: 12 })));
  P('long-30min', longCsv(fullHour('B', 'Half hour', -36.9, 174.76, '2025-03-12', 7 * 60, LEGS3.map(([a, b, q]) => [a, b, q * 2]), { interval: 30, steps: 4 })));
  P('long-60min', longCsv(fullHour('B', 'Hourly', -36.9, 174.76, '2025-03-12', 7 * 60, LEGS3.map(([a, b, q]) => [a, b, q * 4]), { interval: 60, steps: 3 })));
  P('weekend', longCsv(fullHour('C', 'Saturday', -36.9, 174.76, '2025-03-15', 7 * 60, LEGS3)));
  P('holidays', longCsv(fullHour('C', 'Easter', -36.9, 174.76, '15/04/2025', 7 * 60, LEGS3)));
  P('no-term-table', longCsv(fullHour('C', 'Old', -36.9, 174.76, '15/04/2019', 7 * 60, LEGS3)));
  P('unreadable-date', longCsv(fullHour('C', 'Odd date', -36.9, 174.76, 'March 12th', 7 * 60, LEGS3)));
  P('no-date', longCsv(fullHour('C', 'No date', -36.9, 174.76, '', 7 * 60, LEGS3)));
  P('date-in-time', ['site_id,lat,lng,time_start,from_leg,movement,count', ...[0, 1, 2, 3].flatMap(b => [['S1', -36.9, 174.76, `12/03/2025 ${hm(7 * 60 + b * 15)}`, 'N', 'T', 100 + b], ['S1', -36.9, 174.76, `12/03/2025 ${hm(7 * 60 + b * 15)}`, 'S', 'T', 90 + b]].map(r => r.join(',')))].join('\n'));
  P('two-sites-two-days', longCsv([...fullHour('A', 'A', -36.9, 174.76, '12/03/2025', 7 * 60, LEGS3.slice(0, 6)), ...fullHour('A', 'A', -36.9, 174.76, '13/03/2025', 7 * 60, LEGS3.slice(0, 6)), ...fullHour('B', 'B', -36.91, 174.77, '12/03/2025', 7 * 60, LEGS3.slice(6))]));
  P('wide', ['site_id,site_name,lat,lng,date,time_start,N_L_LV,N_T_LV,N_T_HV,N_R,S_T_LV,S_in,E_out_hv,nw_u_bus,bogus_col', ...[0, 1, 2, 3, 4, 5, 6, 7].map(b => ['W1', 'Wide site', -36.9, 174.76, '12/03/2025', hm(7 * 60 + b * 15), 12 + b, 80 + b, 6, 9, 70 + b, 100, 0, 1, 5].join(','))].join('\r\n'));
  P('wide-blank-cells', ['site_id,lat,lng,time_start,N_L,N_T', ...[0, 1, 2, 3].map(b => ['W2', -36.9, 174.76, hm(7 * 60 + b * 15), b === 2 ? '' : 20 + b, b === 1 ? 'x' : 100].join(','))].join('\n'));
  P('scats', ['site_id,site_name,lat,lng,detector,date,time_start,interval_min,from_leg,movement,class,count', ...[0, 1, 2, 3, 4, 5, 6, 7].flatMap(b => [['D1', 'SCATS 1', -36.9, 174.76, 'D1', '12/03/2025', hm(7 * 60 + b * 15), 15, 'N', 'LT', 'LV', 100 + b], ['D1', 'SCATS 1', -36.9, 174.76, 'D2', '12/03/2025', hm(7 * 60 + b * 15), 15, 'N', 'R', 'LV', 30], ['D1', 'SCATS 1', -36.9, 174.76, 'D3', '12/03/2025', hm(7 * 60 + b * 15), 15, 'S', 'LTR', 'LV', 90], ['D1', 'SCATS 1', -36.9, 174.76, 'D3', '12/03/2025', hm(7 * 60 + b * 15), 15, 'E', 'TRUL', 'LV', 40]].map(r => r.join(',')))].join('\n'));
  P('leg-totals', longCsv([...[0, 1, 2, 3, 4, 5, 6, 7].flatMap(b => [['L', 'Totals', -36.9, 174.76, '12/03/2025', hm(7 * 60 + b * 15), 15, 'N', 'IN', 'LV', 200 + b, ''], ['L', 'Totals', -36.9, 174.76, '12/03/2025', hm(7 * 60 + b * 15), 15, 'N', 'OUT', 'LV', 180, ''], ['L', 'Totals', -36.9, 174.76, '12/03/2025', hm(7 * 60 + b * 15), 15, 'S', 'IN', 'LV', 150, ''], ['L', 'Totals', -36.9, 174.76, '12/03/2025', hm(7 * 60 + b * 15), 15, 'S', 'OUT', 'HV', 170, ''], ['L', 'Totals', -36.9, 174.76, '12/03/2025', hm(7 * 60 + b * 15), 15, 'E', 'IN', 'LV', 90, 'Mill Road'], ['L', 'Totals', -36.9, 174.76, '12/03/2025', hm(7 * 60 + b * 15), 15, 'E', 'OUT', 'LV', 100, 'Mill Road']])]));
  P('classes', longCsv(['LV', 'car', 'HCV-II', 'bus', 'Cycle', 'motorcycle', 'Bike'].flatMap((cl, k) => [0, 1, 2, 3].map(b => ['K', 'Classes', -36.9, 174.76, '12/03/2025', hm(7 * 60 + b * 15), 15, 'N', 'T', cl, 10 + k, 'Main']))));
  P('bad-rows', [H(), 'B,Bad,-36.9,174.76,12/03/2025,07:00,15,N,T,LV,100,', 'B,Bad,,,12/03/2025,07:15,15,N,T,LV,100,', 'B,Bad,-36.9,174.76,12/03/2025,quarter past,15,N,T,LV,100,', 'B,Bad,-36.9,174.76,12/03/2025,07:30,15,Q,T,LV,100,', 'B,Bad,-36.9,174.76,12/03/2025,07:45,15,N,T,spaceship,100,', 'B,Bad,-36.9,174.76,12/03/2025,08:00,15,N,T,LV,lots,', 'B,Bad,-36.9,174.76,12/03/2025,08:15,15,N,sideways,LV,100,', 'B,Bad,-36.9,174.76,12/03/2025,08:30,15,N,T,LV,-5,', '', ',,,,,,,,,,,', 'B,Bad,-36.9,174.76,12/03/2025,08:45,15,N,L,LV,10,', 'B,Bad,-36.9,174.76,12/03/2025,09:00,0,N,L1,LV,10,Main St', 'B,Bad,-36.9,174.76,12/03/2025,09:15,x,NE,R3,LV,10,', ...Array.from({ length: 12 }, (_, i) => `B,Bad,-36.9,174.76,12/03/2025,10:${String(i).padStart(2, '0')},15,N,T,LV,oops,`)].join('\n'));
  P('quoted-and-bom', '﻿' + ['"site_id","site_name","lat","lng","date","time_start","from_leg","movement","count"', ...[0, 1, 2, 3].map(b => `Q,"Smith, Jones & ""Sons"" Rd",-36.9,174.76,12/03/2025,${hm(7 * 60 + b * 15)},N,T,${100 + b}`)].join('\r\n'));
  P('header-spaces', ['Site ID, Site Name ,LAT,Lon,Count Date,Time Start,From Leg,Mov,Vehicles', ...[0, 1, 2, 3].map(b => `H,Spaced,-36.9,174.76,12/03/2025,${hm(7 * 60 + b * 15)},n,t,${100 + b}`)].join('\n'));
  P('no-site-id', ['lat,lng,time_start,from_leg,movement,count', ...[0, 1, 2, 3].map(b => `-36.9,174.76,${hm(7 * 60 + b * 15)},E,R,${50 + b}`)].join('\n'));
  P('short-rows', ['site_id,lat,lng,time_start,from_leg,movement,count', 'S,-36.9,174.76,07:00,N,T', 'S,-36.9,174.76,07:15,N,T,50', 'S,-36.9', 'S,-36.9,174.76,07:30,N,T,60,extra,more'].join('\n'));
  P('err-no-rows', 'site_id,lat,lng,time_start,from_leg,movement,count');
  P('err-no-time', 'site_id,lat,lng,from_leg,movement,count\nA,-36,174,N,T,5');
  P('err-no-latlng', 'site_id,time_start,from_leg,movement,count\nA,07:00,N,T,5');
  P('err-no-layout', 'site_id,lat,lng,time_start,volume\nA,-36,174,07:00,5');
  P('err-nothing-read', 'site_id,lat,lng,time_start,from_leg,movement,count\nA,,,07:00,N,T,5\nA,-36,174,xx,N,T,5\nA,-36,174,07:00,Z,T,5');
  P('err-empty', '');
  P('err-blank-only', '\n\n,,\n');
  P('time-edge-bins', longCsv([0, 4, 7, 14, 15, 16, 29, 30, 44, 45, 59].map(m => ['T', 'Bins', -36.9, 174.76, '12/03/2025', hm(7 * 60 + m), 5, 'N', 'T', 'LV', 10 + m, ''])));
  P('gaps-in-hour', longCsv([0, 1, 3, 4, 5, 6, 7].flatMap(b => [['G', 'Gap', -36.9, 174.76, '12/03/2025', hm(7 * 60 + b * 15), 15, 'N', 'T', 'LV', 100 + b * 3, '']])));
  P('only-pm', longCsv(fullHour('M', 'Pm only', -36.9, 174.76, '12/03/2025', 16 * 60, LEGS3.slice(0, 4))));
  P('zero-counts', longCsv(fullHour('Z', 'Zeros', -36.9, 174.76, '12/03/2025', 7 * 60, [['N', 'T', 0], ['S', 'T', 0]])));
  // peaks on stored sites: windows that change what is found
  const base = C.parseTurnCSV(longCsv([...fullHour('A', 'A', -36.9, 174.76, '12/03/2025', 6 * 60 + 30, LEGS3, { steps: 16 }), ...fullHour('A', 'A', -36.9, 174.76, '12/03/2025', 15 * 60 + 30, LEGS3, { steps: 16 })]), 'p.csv').sites[0];
  const winsets = [{ AM: ['07:00', '09:00'], IP: ['09:00', '15:00'], PM: ['16:00', '18:00'] }, { AM: ['06:00', '10:00'] }, { AM: ['7:30', '8:30'], PM: ['16:00', '17:00'] }, { AM: ['08:00', '07:00'] }, { AM: ['x', '09:00'], PM: ['15:30', '19:30'] }, { AM: ['07:00', '08:00'], AM2: ['07:15', '08:15'], X: ['00:00', '23:59'] }, {}];
  for (const w of winsets) { const s = JSON.parse(JSON.stringify(base)); c.Calibration.peaks(s, w); out.peaks.push({ windows: w, site: JSON.parse(JSON.stringify(base)), peak: JSON.parse(JSON.stringify(s.peak)) }); }
  // Furness
  const F = [
    [[300, 200, 100], [250, 250, 100], [[0, .5, .5], [.5, 0, .5], [.5, .5, 0]], {}], [[300, 200, 100, 150], [250, 250, 100, 150], [[0, .15, .7, .15], [.15, 0, .15, .7], [.7, .15, 0, .15], [.15, .7, .15, 0]], {}],
    [[100, 0, 50], [60, 60, 30], [[0, 1, 1], [1, 0, 1], [1, 1, 0]], {}], [[0, 0], [0, 0], [[0, 1], [1, 0]], {}], [[100, 100], [100, 100], [[0, 0], [0, 0]], {}], [[100, 200], [150, 150], [[0, 1], [1, 0]], { tol: 1e-9, maxIt: 5 }],
    [[500, 120, 340, 80], [60, 400, 200, 380], [[0, .3, .6, .1], [.2, 0, .3, .5], [.6, .2, 0, .2], [.5, .3, .2, 0]], { tol: 1e-6, maxIt: 100 }], [[10, 20, 30], [20, 20, 20], [[0, 1, 1], [1, 0, -1], ['x', 1, 0]], {}], [[100], [100], [[0]], {}], [[100, 50], [0, 0], [[1, 1], [1, 1]], {}],
  ];
  for (const [ins, outs, seed, o] of F) out.furness.push({ ins, outs, seed, opts: o, out: P_(C.furness(ins, outs, seed, o)) });
  out.legBearings = LEG_NAMES8;
  return out;
}

if (!only || only === 'turns') write('calibrate-turns.json', turnImportCases());

/* ---------- screenline suggestions from OpenStreetMap: the pure parts (joining ways, grouping, counting crossings) ---------- */
// The Overpass answer is made up here (rivers, canals and railways of several ways, some reversed, some not crossing enough roads);
// the network fetch itself is replaced by a stub that returns it.
async function suggestCases(fixture) {
  const way = (id, kind, name, pts, extra = {}) => ({ type: 'way', id, tags: { ...(kind === 'railway' ? { railway: 'rail' } : { waterway: kind }), ...(name ? { name } : {}), ...extra }, geometry: pts.map(([lat, lon]) => ({ lat, lon })) });
  const L = FIXTURE_LINES[fixture].map(x => x.pts);
  const lerp = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
  const split = (pts, n) => Array.from({ length: n }, (_, i) => [lerp(pts[0], pts[1], i / n), lerp(pts[0], pts[1], (i + 1) / n)]);
  const mid = pts => lerp(pts[0], pts[1], 0.5);
  const [a0, a1, a2, a3, a4] = L;
  const [r1, r2, r3] = split(a0, 3);
  const small = [[mid(a1)[0], mid(a1)[1] - 0.0001], [mid(a1)[0], mid(a1)[1] + 0.0001]];
  const elements = [
    way(1003, 'river', 'Alpha River', r3),                        // the three ways are out of order and one is reversed
    way(1001, 'river', 'Alpha River', r1),
    way(1002, 'river', 'Alpha River', [r2[1], r2[0]]),
    way(1100, 'river', ' Alpha River ', a1),                     // the same name (trimmed): a second, separate line
    way(1200, 'canal', '', split(a3, 2)[0]), way(1201, 'canal', '', split(a3, 2)[1]),  // unnamed canal in two ways
    way(1300, 'railway', '', a2),                                 // unnamed railway (a four-point line)
    way(1301, 'railway', 'Yard', a2, { service: 'yard' }),        // a yard: left out
    way(1302, 'railway', 'Siding', a2, { service: 'siding' }),    // a siding: left out
    { ...way(1401, 'river', 'Brook', a2), tags: { waterway: 'stream' } },   // not a river or canal
    way(1500, 'river', 'Small Creek', small),                     // too few crossings: dropped
    way(1600, 'river', 'Bad Point', [[a3[0][0], a3[0][1]], ['x', 174.9]]),  // a point that is not a number: dropped by joinWays
    { type: 'node', id: 1700, lat: a0[0][0], lon: a0[0][1], tags: { name: 'A node' } },
    { type: 'way', id: 1701, geometry: a0.map(([lat, lon]) => ({ lat, lon })) },             // no tags
    way(1702, 'river', 'One point', [a0[0]]),                     // one point
    { ...way(1703, 'river', 'Empty tags', a0), tags: {} },
    way(1800, 'railway', 'Main Trunk', split(a1, 4)[0]), way(1801, 'railway', 'Main Trunk', split(a1, 4)[1]),
    way(1802, 'railway', 'Main Trunk', split(a1, 4)[2]), way(1803, 'railway', 'Main Trunk', split(a1, 4)[3]),
  ];
  const stub = async () => ({ ok: true, json: async () => ({ elements }) });
  const c = loadAll({ seed: 11, fetchImpl: stub }), C = c.Calibration;
  const P = c.migrate(JSON.parse(readFileSync(join(here, 'fixtures', fixture + '.json'), 'utf8')));
  c.Store.P = P;
  const sc = P.scenarios[0], text = c.Store.serialise(P);
  const ids = [], uid0 = c.U.uid; c.U.uid = p => { const id = uid0(p); ids.push(id); return id; };
  const out = { fixture, projectText: text, elements, ids: null, joinWays: [], steps: [] };
  const listOut = r => P_({ ok: r.ok, note: r.note, cached: r.cached ?? null, list: r.list });
  const step = async (name, fn) => { out.steps.push({ name, ...(await fn()) }); };
  await step('first', async () => ({ r: listOut(await C.suggestScreenlines(sc)) }));
  await step('cached', async () => ({ r: listOut(await C.suggestScreenlines(sc)) }));
  await step('added', async () => {
    const first = await C.suggestScreenlines(sc), got = [];
    const pick = [first.list[0], first.list[first.list.length - 1], { name: 'x'.repeat(100) + 'end', kind: 'river', pts: [[-37, 174], [-37.1, 174.1]], osm: [1, 2, 3] }, { name: 'short', kind: 'canal', pts: [[-37, 174]], osm: [] }, null];
    pick.forEach((item, k) => got.push(P_({ item, sl: C.addSuggested(item, k === 1 ? 'me' : 'valid') ?? null, added: item ? item.added ?? null : null })));
    const again = await C.suggestScreenlines(sc);
    return { got, store: P_(C.store().screenlines), again: listOut(again) };
  });
  await step('no roads', async () => ({ r: listOut(await C.suggestScreenlines({ id: 'x', nodes: { a: { id: 'a', lat: -37, lng: 174 } }, links: {} })) }));
  await step('no features', async () => {
    const mk = async body => { const c2 = loadAll({ seed: 11, fetchImpl: async () => ({ ok: true, json: async () => body }) }); c2.Store.P = c2.migrate(JSON.parse(text)); return listOut(await c2.Calibration.suggestScreenlines(c2.Store.P.scenarios[0])); };
    return { r: await mk({ elements: [] }), noElements: await mk({}) };
  });
  out.ids = ids;
  // joinWays on its own: chains in any order and direction, rings, separate lines, bad points, equal lengths
  const w = (id, pts) => ({ id, geometry: pts.map(([lat, lon]) => ({ lat, lon })) });
  const A = [-37, 174], B = [-37, 174.001], Cc = [-37, 174.002], D = [-37.001, 174.002], E = [-37.001, 174.001], F = [-37.001, 174];
  const cases = [
    ['single', [w(1, [A, B])]], ['chain in order', [w(1, [A, B]), w(2, [B, Cc]), w(3, [Cc, D])]], ['chain shuffled', [w(3, [Cc, D]), w(1, [A, B]), w(2, [B, Cc])]],
    ['chain reversed ways', [w(1, [B, A]), w(2, [Cc, B]), w(3, [D, Cc])]], ['joins at the start', [w(2, [B, Cc]), w(1, [A, B])]], ['joins at the start reversed', [w(2, [B, Cc]), w(1, [B, A])]],
    ['ring', [w(1, [A, B, Cc]), w(2, [Cc, D, E]), w(3, [E, F, A])]], ['two lines, longer first', [w(1, [A, B]), w(2, [D, E, F])]], ['equal lengths keep order', [w(1, [A, B]), w(2, [D, E]), w(3, [F, [-37.002, 174]])]],
    ['near points join', [w(1, [A, B]), w(2, [[B[0] + 5e-8, B[1] - 5e-8], Cc])]], ['gap too wide', [w(1, [A, B]), w(2, [[B[0] + 2e-7, B[1]], Cc])]],
    ['bad point dropped', [w(1, [A, B]), w(2, [B, [NaN, 174]]), w(3, [B, Cc])]], ['string coordinates', [w(1, [A, B]), { id: 2, geometry: [{ lat: '-37', lon: '174.001' }, { lat: '-37', lon: '174.002' }] }]],
    ['ids as text and missing', [{ id: 'w1', geometry: [{ lat: -37, lon: 174 }, { lat: -37, lon: 174.001 }] }, { geometry: [{ lat: -37, lon: 174.001 }, { lat: -37, lon: 174.002 }] }]], ['no ways', []],
    ['three-way junction takes first', [w(1, [A, B]), w(2, [B, Cc]), w(3, [B, E])]], ['branches kept apart', [w(1, [A, B]), w(2, [B, Cc]), w(3, [Cc, D]), w(4, [F, E]), w(5, [E, [-37.002, 174.001]])]],
  ];
  for (const [name, ways] of cases) out.joinWays.push({ name, ways: P_(ways), out: P_(C.joinWays(ways)) });
  return out;
}

if (!only || only === 'suggest') for (const fx of ['corridor', 'grid']) write(`calibrate-suggest-${fx}.json`, await suggestCases(fx));

/* ---------- gateway flows from counts (DataHub.gatewaysFromCounts), with the counts and switches changed in several ways ---------- */
async function gatewayCases(fixture) {
  const prep = await prepare(fixture, { existing: true, year: 2026 });
  const text = prep.text;
  const out = { fixture, projectText: text, cases: [] };
  const isEst = c => c.obs_method === 'ramm_estimate' || c.kind === 'ramm_estimate' || c.src === 'ramm';
  const variants = [
    ['all counts', () => ({})],
    ['no counts', d => { d.counts = []; return { keep: [] }; }],
    ['RAMM estimates only', d => { d.counts = d.counts.filter(isEst); return { keep: d.counts.map(x => x.id) }; }],
    ['NZTA virtual sites only, virtual on', d => { d.counts = d.counts.filter(x => x.virtual); d.use.virtual = true; return { keep: d.counts.map(x => x.id), use: { virtual: true } }; }],
    ['NZTA virtual sites only, virtual off', d => { d.counts = d.counts.filter(x => x.virtual); d.use.virtual = false; return { keep: d.counts.map(x => x.id), use: { virtual: false } }; }],
    ['NZTA virtual sites only, NZTA off', d => { d.counts = d.counts.filter(x => x.virtual); d.use.virtual = true; d.use.nzta = false; return { keep: d.counts.map(x => x.id), use: { virtual: true, nzta: false } }; }],
    ['user and AT counts off', d => { d.use.user = false; d.use.at = false; return { use: { user: false, at: false } }; }],
    ['directional counts only', d => { d.counts = d.counts.filter(x => x.directional && x.dir); return { keep: d.counts.map(x => x.id) }; }],
    ['two-way counts only', d => { d.counts = d.counts.filter(x => !x.directional && !isEst(x)); return { keep: d.counts.map(x => x.id) }; }],
    ['RAMM switched off', d => { d.use.ramm = false; return { use: { ramm: false } }; }],
    ['NZTA switched off', d => { d.use.nzta = false; return { use: { nzta: false } }; }],
    ['every count off', d => { d.counts.forEach(x => { x.off = true; }); return { off: d.counts.map(x => x.id) }; }],
    ['some counts off', d => { const ids = d.counts.filter((x, i) => i % 3 === 0).map(x => x.id); d.counts.forEach(x => { if (ids.includes(x.id)) x.off = true; }); return { off: ids }; }],
  ];
  const gws = sc => Object.fromEntries(Object.values(sc.zones).filter(z => z.kind === 'gateway').map(z => [z.id, { gw: z.gw ?? null, gwSrc: z.gwSrc ?? null }]));
  for (const [name, fn] of variants) {
    const o = open(text), d = o.c.DataHub.D();
    const spec = fn(d) || {};
    const before = P_(gws(o.sc));
    const n = o.c.DataHub.gatewaysFromCounts(o.sc);
    out.cases.push({ name, spec, n, before, after: P_(gws(o.sc)) });
  }
  // gateways that already have flows (kept, overwritten where a count gives that flow), and one with a null gw
  {
    const o = open(text), ids = Object.values(o.sc.zones).filter(z => z.kind === 'gateway').map(z => z.id);
    const pre = {};
    ids.forEach((id, i) => { pre[id] = i % 2 ? { AMin: 5, AMout: 6, PMin: 7, PMout: 8, extra: 1 } : null; o.sc.zones[id].gw = pre[id] ? { ...pre[id] } : null; });
    const before = P_(gws(o.sc)), n = o.c.DataHub.gatewaysFromCounts(o.sc);
    out.cases.push({ name: 'gateways with flows already', spec: { preGw: pre }, n, before, after: P_(gws(o.sc)) });
  }
  return out;
}

if (!only || only === 'gateways') for (const fx of ['corridor', 'grid']) write(`calibrate-gateways-${fx}.json`, await gatewayCases(fx));

/* ---------- the stress test: demand times 1.1 and 1.2 (or the factors given), junctions whose saturation rises faster than demand ---------- */
async function stressCases(fixture) {
  const prep = await prepare(fixture, { existing: true, year: 2026 });
  const text = prep.text;
  const out = { fixture, projectText: text, cases: [] };
  const cases = [
    ['default AM', { period: 'AM' }, {}],
    ['default PM', { period: 'PM' }, {}],
    ['one factor', { period: 'AM', factors: [1.5] }, {}],
    ['three factors PM', { period: 'PM', factors: [1.1, 1.3, 1.6] }, {}],
    ['heavy demand', { period: 'AM', factors: [2, 4] }, {}],
    ['no factors', { period: 'AM', factors: [] }, {}],
    ['no stored results', { period: 'AM' }, { noRes: true }],
    ['stale base result', { period: 'AM' }, { stale: true }],
    ['period from the settings', {}, {}],
    ['forecast scenario', { period: 'AM' }, { scIndex: 1 }],
  ];
  for (const [name, opts, how] of cases) {
    const o = open(text);
    if (!how.noRes) await runBoth(o.c, o.sc);
    if (how.stale) o.sc.res.AM.stale = true;
    const sc = o.P.scenarios[how.scIndex || 0];
    if (how.scIndex) { sc.res = { AM: await o.c.Engine.assign(sc, { period: 'AM' }) }; o.c.Store.ver++; }
    const prog = [];
    const r = await o.c.Calibration.stressTest(sc, { ...opts, onProgress: (k, n, m) => prog.push([k, n, m]) });
    out.cases.push({ name, opts, how, scId: sc.id, progress: P_(prog),
      state: P_({ scId: r.scId, period: r.period, factors: r.factors, top: r.top ? r.top.id : null,
        nodes: r.nodes.map(n => ({ id: n.id, name: n.name, dos0: n.dos0, runs: n.runs, sens: n.sens, flag: n.flag })),
        links: r.links.map(l => ({ key: l.key, link: l.L.id, vc: l.vc, f: l.f })) }) });
  }
  return out;
}

if (!only || only === 'stress') for (const fx of ['corridor', 'grid']) write(`calibrate-stress-${fx}.json`, await stressCases(fx));

/* ---------- your own counts from a CSV file (DataHub.importCSV): headers, directions, bad rows, errors, replacing earlier counts ---------- */
async function importCountsCases() {
  const prep = await prepare('corridor', { existing: true, year: 2026 });
  const bareC = loadAll({ seed: 3 });
  bareC.Store.P = bareC.newProject('t');
  const bareText = bareC.Store.serialise(bareC.Store.P);
  const out = { prepared: prep.text, bare: bareText, cases: [] };
  const H = 'name,lat,lng,am,pm,adt,dir';
  const cases = [
    ['basic', [H, 'Main Rd,-36.9,174.76,650,700,9000,NB', 'Side St,-36.91,174.77,120,150,2000,', 'Two way,-36.92,174.78,0,300,,'].join('\n')],
    ['aliases and case', ['Site, Latitude ,LONGITUDE,AM Peak,PM_PEAK,AADT,Direction', 'A,-36.9,174.76,100,110,1500,sb', 'B,-36.8,174.7,,,3000,WB'].join('\r\n')],
    ['x and y columns', ['x,y,am', '174.76,-36.9,500'].join('\n')],
    ['long and lon', ['road,lat,long,daily', 'R1,-36.9,174.76,4000', 'R2,-36.8,174.7,'].join('\n')],
    ['location column', ['location,lat,lon,pm_vph', 'L1,-36.9,174.76,800'].join('\n')],
    ['direction words', [H, ...['NB', 'northbound', 'eb sb', 'XNB', 'nb.', 'a_nb', 'WB-1', 'Eastbound', '(SB)', 'sbnb', 'NB/SB', ' wb ', '', 'N B'].map((d, i) => `D${i},-36.9,174.76,100,,,${d}`)].join('\n')],
    ['blank and bad coordinates', [H, 'blank lat,,174.76,100,,,', 'bad lat,x,174.76,100,,,', 'bad lng,-36.9,y,100,,,', 'ok,-36.9,174.76,100,,,', 'inf,Infinity,174.76,100,,,', 'short', ',,,,,,'].join('\n')],
    ['volumes', [H, 'zero,-36.9,174.76,0,0,0,', 'blank,-36.9,174.76,,,,', 'text,-36.9,174.76,abc,def,ghi,', 'comma,-36.9,174.76,"1,200",,"1,234",', 'spaces,-36.9,174.76, 500 , 600 , 7000 ,', 'negative,-36.9,174.76,-5,,,', 'exp,-36.9,174.76,1e3,,2.5e3,', 'hex,-36.9,174.76,0x10,,,', 'decimal,-36.9,174.76,12.5,,,'].join('\n')],
    ['quoted names', [H, '"Smith, Jones & ""Sons"" Rd",-36.9,174.76,100,,,', '"Line\nbreak",-36.9,174.76,100,,,', '  padded  ,-36.9,174.76,100,,,'].join('\n')],
    ['no name column', ['lat,lng,am', '-36.9,174.76,100', 'x,174.76,100', '-36.91,174.77,200'].join('\n')],
    ['short rows', ['name,lat,lng,am,pm', 'A,-36.9,174.76', 'B,-36.9,174.76,100,200', 'C'].join('\n')],
    ['name past the end', ['lat,lng,am,name', '-36.9,174.76,100'].join('\n')],
    ['bom on the first column', '﻿' + ['name,lat,lng,am', 'A,-36.9,174.76,100'].join('\n')],
    ['bom on lat', '﻿' + ['lat,lng,am', '-36.9,174.76,100'].join('\n')],
    ['blank lines', ['', 'name,lat,lng,am', '', 'A,-36.9,174.76,100', ',,,', '', 'B,-36.91,174.77,200', ''].join('\n')],
    ['duplicate headers', ['name,lat,lat,lng,am,am', 'A,-36.9,-40,174.76,100,200'].join('\n')],
    ['error: header only', 'name,lat,lng,am'],
    ['error: empty', ''],
    ['error: no lat', ['name,lng,am', 'A,174.76,100'].join('\n')],
    ['error: no lng', ['name,lat,am', 'A,-36.9,100'].join('\n')],
    ['error: no volumes', ['name,lat,lng', 'A,-36.9,174.76'].join('\n')],
    ['no usable rows', ['name,lat,lng,am', 'A,x,y,100'].join('\n')],
  ];
  for (const kind of ['prepared', 'bare']) {
    for (const [name, text] of cases) {
      const o = kind === 'prepared' ? open(prep.text) : (() => { const c = loadAll({ seed: 3 }); c.Store.P = c.migrate(JSON.parse(bareText)); return { c, P: c.Store.P }; })();
      const ids = [], uid0 = o.c.U.uid; o.c.U.uid = (p = '') => { const id = p + 'x' + ids.length; ids.push(id); return id; };
      const before = kind === 'prepared' ? o.c.DataHub.D().counts.filter(x => x.src !== 'user').length : 0;
      let n = null, error = null;
      try { n = o.c.DataHub.importCSV(text); } catch (e) { error = e.message; }
      const d = o.P.data || null;
      out.cases.push({ kind, name, text, n, error, ids, othersBefore: before,
        data: d ? P_({ counts: d.counts.filter(x => x.src === 'user'), others: d.counts.filter(x => x.src !== 'user').map(x => x.id), crashes: d.crashes, loaded: d.loaded, jt: d.jt, use: d.use }) : null });
    }
  }
  return out;
}

if (!only || only === 'import') write('calibrate-import.json', await importCountsCases());
