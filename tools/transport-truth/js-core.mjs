// Loads the transport prototype's calculation scripts into Node, with no browser, so its results can be used as truth tests
// for the Rust port (projects/spare-construct/crates/construct-transport). Only the calculation scripts are loaded; the
// interface objects they mention (UI, Render, Inspector) are inert stand-ins.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import vm from 'node:vm';

const here = dirname(fileURLToPath(import.meta.url));
export const JS_DIR = join(here, '..', '..', 'projects', 'spare-construct', 'demo', 'transport', 'js');
const CORE = ['util', 'model', 'registry', 'engine', 'data', 'landuse', 'turns', 'calibrate', 'sim', 'sample'];

const inert = () => new Proxy(function () {}, { get: (t, k) => (k === Symbol.toPrimitive ? () => '' : inert()), apply: () => undefined });

export function loadCore({ seed = 1, now = Date.UTC(2026, 9, 21) } = {}) {
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
  };
  ctx.window = ctx; ctx.globalThis = ctx; ctx.self = ctx;
  vm.createContext(ctx);
  const src = CORE.map(n => `/* ${n}.js */\n` + readFileSync(join(JS_DIR, n + '.js'), 'utf8')).join('\n;\n');
  vm.runInContext(src + `\n;globalThis.__core = { U, Store, Net, Importers, Scenarios, Sites, Engine, DataHub, Landuse: typeof Landuse !== 'undefined' ? Landuse : null, Turns: typeof Turns !== 'undefined' ? Turns : null, Calibrate: typeof Calibrate !== 'undefined' ? Calibrate : null, Sim: typeof Sim !== 'undefined' ? Sim : null, Sample: typeof Sample !== 'undefined' ? Sample : null, newProject, newScenario, defaultSettings, migrate, CLASSES, LANDUSES };`, ctx, { filename: 'transport-core.js' });
  return ctx.__core;
}
