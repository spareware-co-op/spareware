// Loads the transport prototype's calculation scripts plus the two feature scripts the junction tools and the ITA report
// port covers (js/kit.js, js/features/junctions.js, js/features/report.js) into Node, with no browser. A copy of js-core.mjs
// with a longer script list: the interface objects (UI, Render, Inspector ...) are inert stand-ins, the clock is fixed and Math.random
// is seeded, so every output is repeatable. The compliance, calibration, land-use and data-feed feature scripts are not loaded
// (the report treats them as not installed); a generator may define stand-ins for their globals through `globals`.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import vm from 'node:vm';

const here = dirname(fileURLToPath(import.meta.url));
export const JS_DIR = join(here, '..', '..', 'projects', 'spare-construct', 'demo', 'transport', 'js');
const SCRIPTS = ['util', 'model', 'registry', 'engine', 'data', 'landuse', 'turns', 'calibrate', 'sim', 'kit', 'features/junctions', 'features/report'];

const inert = () => new Proxy(function () {}, { get: (t, k) => (k === Symbol.toPrimitive ? () => '' : inert()), apply: () => undefined });

export const NOW = Date.UTC(2026, 9, 21);

export function loadReportCore({ seed = 1, now = NOW, globals = {}, scripts = SCRIPTS } = {}) {
  let s = seed >>> 0;
  const rand = () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  const M = Object.create(Math); M.random = rand;
  const store = new Map();
  const ctx = {
    console, JSON, Promise, Map, Set, WeakMap, Array, Object, Number, String, Boolean, Symbol, Error, TypeError, RangeError,
    Float64Array, Float32Array, Int32Array, Uint32Array, Uint8Array, Int8Array, Uint16Array, ArrayBuffer, DataView, Intl, RegExp,
    Math: M, Date: class extends Date { constructor(...a) { super(...(a.length ? a : [now])); } static now() { return now; } },
    setTimeout, clearTimeout, queueMicrotask, structuredClone, TextEncoder, TextDecoder, URL, URLSearchParams, AbortController,
    performance: { now: () => 0 }, fetch: (...a) => fetch(...a), isFinite, isNaN, parseFloat, parseInt, Infinity, NaN, encodeURIComponent, decodeURIComponent,
    localStorage: { getItem: k => store.get(k) ?? null, setItem: (k, v) => store.set(k, String(v)), removeItem: k => store.delete(k) },
    UI: inert(), Render: inert(), Inspector: inert(), Dialogs: inert(), Drawer: inert(),
    CSS: { escape: x => String(x) },
    ...globals,
  };
  ctx.window = ctx; ctx.globalThis = ctx; ctx.self = ctx;
  vm.createContext(ctx);
  const src = scripts.map(n => `/* ${n}.js */\n` + readFileSync(join(JS_DIR, n + '.js'), 'utf8')).join('\n;\n');
  vm.runInContext(src + `\n;globalThis.__core = { U, Store, Net, Importers, Scenarios, Sites, Register: typeof Register !== 'undefined' ? Register : null, Engine, DataHub, Landuse: typeof Landuse !== 'undefined' ? Landuse : null, Turns, Calibrate: typeof Calibrate !== 'undefined' ? Calibrate : null, Kit, Registry, Junctions, Report, newProject, newScenario, defaultSettings, migrate, CLASSES, LANDUSES, CONTROLS };`, ctx, { filename: 'transport-report.js' });
  const core = ctx.__core;
  core.ctx = ctx;
  return core;
}

// JSON for results: typed arrays as plain arrays, Maps and Sets as objects and arrays, non-finite numbers as strings
export const plain = v => {
  if (ArrayBuffer.isView(v)) return Array.from(v, plain);
  if (v instanceof Map) return Object.fromEntries([...v].map(([k, x]) => [String(k), plain(x)]));
  if (v instanceof Set) return [...v].map(plain);
  if (typeof v === 'number') return Number.isFinite(v) ? v : String(v);
  if (Array.isArray(v)) return v.map(plain);
  if (v && typeof v === 'object') { const o = {}; for (const [k, x] of Object.entries(v)) if (typeof x !== 'function' && x !== undefined) o[k] = plain(x); return o; }
  return v;
};
export const revive = v => (v === 'NaN' ? NaN : v === 'Infinity' ? Infinity : v === '-Infinity' ? -Infinity : Array.isArray(v) ? v.map(revive) : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, revive(x)])) : v);
export const clone = o => JSON.parse(JSON.stringify(o));
