// The harness the compliance generators share (gen-compliance.mjs and its parts gen-compliance-*.mjs): the prototype's
// compliance scripts loaded into the sandbox js-core.mjs uses (plus kit.js, which they call for a few pure helpers),
// project fixtures with results attached, and the writers. The interface objects are inert and there is no network. The
// clock is fixed (21 Oct 2026, UTC midnight) and the random stream is seeded, so every run gives the same files.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import vm from 'node:vm';

export const here = dirname(fileURLToPath(import.meta.url));
export const JS_DIR = join(here, '..', '..', 'projects', 'spare-construct', 'demo', 'transport', 'js');
export const RUST_DATA = join(here, '..', '..', 'projects', 'spare-construct', 'crates', 'construct-transport', 'src', 'compliance', 'data');
export const NOW = Date.UTC(2026, 9, 21);
const CORE = ['util', 'model', 'registry', 'kit', 'engine', 'data', 'landuse', 'turns', 'calibrate', 'sim', 'sample'];
const FEATURES = ['packs/nz/compliance', 'packs/nz/compliance-extra'];

const inert = () => new Proxy(function () {}, { get: (t, k) => (k === Symbol.toPrimitive ? () => '' : inert()), apply: () => undefined });

// The top-level constants of an IIFE module (two-space indent) are exposed as __internal, so the rule tables that are not
// in the module's return value can be dumped too
function exposeInternals(src) {
  const names = [...src.matchAll(/^  const ([A-Za-z_][A-Za-z_0-9]*) = /gm)].map(m => m[1]);
  const body = names.map(n => `try { if (typeof ${n} !== 'function') __i.${n} = ${n}; } catch (e) { }`).join('\n  ');
  const i = src.lastIndexOf('\n  return {');
  if (i < 0) throw new Error('no return block');
  return src.slice(0, i) + `\n  const __i = {};\n  ${body}\n  return { __internal: __i,` + src.slice(i + '\n  return {'.length);
}

// The prototype's calculation scripts and the two compliance scripts in one sandbox. Returns the objects the checks use
// (Store, Engine, Scenarios, Compliance, ComplianceExtra ...); Compliance.__internal and ComplianceExtra.__internal hold each
// script's rule tables.
export function loadCompliance({ seed = 1, now = NOW } = {}) {
  let s = seed >>> 0;
  const rand = () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  const M = Object.create(Math); M.random = rand;
  const store = new Map();
  const ctx = {
    console, JSON, Promise, Map, Set, WeakMap, WeakSet, Array, Object, Number, String, Boolean, Symbol, Error, TypeError, RangeError,
    Float64Array, Float32Array, Int32Array, Uint32Array, Uint8Array, Int8Array, Uint16Array, ArrayBuffer, DataView, Intl, RegExp,
    Math: M, Date: class extends Date { constructor(...a) { super(...(a.length ? a : [now])); } static now() { return now; } },
    setTimeout, clearTimeout, queueMicrotask, structuredClone, TextEncoder, TextDecoder, URL, URLSearchParams, AbortController,
    performance: { now: () => 0 }, fetch: () => { throw new Error('no network'); }, isFinite, isNaN, parseFloat, parseInt, Infinity, NaN, encodeURIComponent, decodeURIComponent,
    localStorage: { getItem: k => store.get(k) ?? null, setItem: (k, v) => store.set(k, String(v)), removeItem: k => store.delete(k) },
    UI: inert(), Render: inert(), Inspector: inert(), Dialogs: inert(), Drawer: inert(), CSS: { escape: x => String(x) },
  };
  ctx.window = ctx; ctx.globalThis = ctx; ctx.self = ctx;
  vm.createContext(ctx);
  const read = n => readFileSync(join(JS_DIR, n + '.js'), 'utf8');
  const src = CORE.map(n => `/* ${n}.js */\n` + read(n)).concat(FEATURES.map(n => `/* ${n}.js */\n` + exposeInternals(read(n)))).join('\n;\n');
  vm.runInContext(src + `\n;globalThis.__core = { U, Store, Net, Importers, Scenarios, Sites, Engine, DataHub, Register, Kit, Registry, Calibrate, Turns, Compliance, ComplianceExtra,
    newProject, newScenario, defaultSettings, migrate, CLASSES, LANDUSES, CONTROLS, STATUS, CYCLE, SCENARIO_ROLES, FUNDING, REGISTER_TYPES };`, ctx, { filename: 'compliance-core.js' });
  return ctx.__core;
}

// JSON for results and records: typed arrays as arrays, Maps and Sets as objects and arrays, non-finite numbers as strings,
// regular expressions as { $re, $flags }, functions dropped
// A value's tag tells a Map, Set, RegExp or Date made inside the sandbox from the host's (they are another realm's objects)
const tag = v => Object.prototype.toString.call(v);
export const plain = v => {
  if (v === undefined || typeof v === 'function') return undefined;
  if (ArrayBuffer.isView(v)) return Array.from(v, plain);
  const t = typeof v === 'object' && v !== null ? tag(v) : '';
  if (t === '[object Map]') return Object.fromEntries([...v].map(([k, x]) => [String(k), plain(x)]));
  if (t === '[object Set]') return [...v].map(plain);
  if (t === '[object RegExp]') return { $re: v.source, $flags: v.flags };
  if (t === '[object Date]') return Number.isNaN(v.getTime()) ? 'Invalid Date' : v.toISOString();
  if (typeof v === 'number') return Number.isFinite(v) ? v : String(v);
  if (Array.isArray(v)) return v.map(x => { const p = plain(x); return p === undefined ? null : p; });
  if (v && typeof v === 'object') {
    const o = {};
    for (const k of Object.keys(v)) { const p = plain(v[k]); if (p !== undefined) o[k] = p; }
    return o;
  }
  return v;
};
export const clone = v => JSON.parse(JSON.stringify(plain(v)));
export const write = (rel, data) => { const t = JSON.stringify(data); writeFileSync(join(here, rel), t); console.log(rel, t.length, 'bytes'); };

/* ---------- projects with results ---------- */
// The saved test projects (inputs/corridor.json, inputs/grid.json) as the prototype loads them
export const input = name => JSON.parse(readFileSync(join(here, 'inputs', name + '.json'), 'utf8'));

// A fresh copy of a saved project, migrated and open in the store (Store.P), with no results yet
export function open(c, name) {
  const P = c.migrate(input(name));
  c.Store.P = P;
  return P;
}

// Runs the engine on every scenario of the open project for AM and PM (as gen-expected.mjs does) and keeps the results in sc.res
export async function run(c, P = c.Store.P, periods = ['AM', 'PM']) {
  c.Store.P = P;
  for (const sc of P.scenarios) {
    P.active = sc.id;
    sc.res = {};
    for (const period of periods) sc.res[period] = await c.Engine.assign(sc, { period });
  }
  P.active = P.scenarios[0].id;
  return P;
}

// A saved project with its results; cached results are copied, so a variant that changes nothing the engine reads does not run
// it again. mutate(P, c) changes the project before the results are attached; pass rerun: true when the change is one the engine reads
const cache = new Map();
export async function withResults(c, name, { rerun = false, mutate } = {}) {
  const P = open(c, name);
  if (mutate) mutate(P, c);
  if (rerun) { await run(c, P); return P; }
  if (!cache.has(name)) { await run(c, P); cache.set(name, P.scenarios.map(sc => [sc.id, sc.res])); return P; }
  for (const [id, res] of cache.get(name)) P.scenarios.find(s => s.id === id).res = structuredClone(res);
  return P;
}

// Writes a project (with its results) as fixtures/data/compliance-<name>.json and returns the name the Rust test reads it by
export function saveFixture(name, P) {
  mkdirSync(join(here, 'fixtures', 'data'), { recursive: true });
  writeFileSync(join(here, 'fixtures', 'data', `compliance-${name}.json`), JSON.stringify(plain(P)));
  return name;
}


/* ---------- variants of a project ---------- */
// o[path] = v for a dotted path ('compliance.ctx.pathway', 'scenarios.0.zones.z1.lu.residential'): a step that is not there yet
// becomes an object; a step on an array is an index
export function setPath(o, path, v) {
  const p = path.split('.');
  let t = o;
  for (let i = 0; i < p.length - 1; i++) {
    if (t[p[i]] === undefined || t[p[i]] === null || typeof t[p[i]] !== 'object') t[p[i]] = {};
    t = t[p[i]];
  }
  t[p[p.length - 1]] = clone(v);
  return o;
}
export const setPaths = (o, sets) => { for (const [k, v] of Object.entries(sets || {})) setPath(o, k, v); return o; };

// The project a case runs on: the saved project 'corridor' or 'grid' (results from the engine) or a fixture written by saveFixture,
// with the case's sets (dotted path: value) applied. The Rust test builds the same project from the fixture and the same sets.
export async function variant(c, name, sets) {
  let P;
  if (name === 'corridor' || name === 'grid') P = await withResults(c, name);
  else { P = JSON.parse(readFileSync(join(here, 'fixtures', 'data', `compliance-${name}.json`), 'utf8')); c.Store.P = P; }
  c.Store.P = P;
  return setPaths(P, sets);
}

// One recorded case: the project (fixture name and sets), and what the function gave (or the error it threw)
export async function record(c, fixture, sets, f) {
  const P = await variant(c, fixture, sets);
  let out;
  try { out = { ok: plain(await f(P, c)) }; } catch (e) { out = { error: String(e && e.message || e) }; }
  return { fixture, sets: sets || {}, ...out };
}

// Writes the saved projects with their results as the fixtures 'base-corridor' and 'base-grid' (the Rust tests read the results from them)
export async function baseFixtures(c) {
  for (const name of ['corridor', 'grid']) saveFixture('base-' + name, await withResults(c, name));
}

// An answer as the prototype gave it, or the error it threw (the Rust port must refuse the same inputs)
export const attempt = fn => { try { return { ok: plain(fn()) }; } catch (e) { return { error: String(e && e.message || e) }; } };
export const attemptAsync = async fn => { try { return { ok: plain(await fn()) }; } catch (e) { return { error: String(e && e.message || e) }; } };

// Generators register themselves here; gen-compliance.mjs runs them
export const PARTS = {};
export const part = (name, fn) => { PARTS[name] = fn; };
