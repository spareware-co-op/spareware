// Loads the prototype's calculation scripts AND its editing interface scripts (kit.js, ui-inspector.js, ui.js) into Node, with the
// browser stubbed inertly, so the compositions the interface makes from engine calls (drawing a road, adding a zone or a gateway,
// deleting, changing a field, copying a scenario, undo) can be run and recorded as truth for the Rust command layer
// (projects/spare-construct/crates/construct-api, tests/ui_truth.rs).
//
// ui.js keeps its functions private, so the one line that returns its public object is extended to also return the private
// functions the recording needs (finishRoad, finishBus, bindMap, duplicateScenario, setYear, deleteSelected, installUndo). The code of
// every function is the prototype's own; nothing is rewritten. The map the interface binds its click handlers to is a stand-in that
// records them, so a click on the zone or gateway tool runs the interface's own handler.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import vm from 'node:vm';
import { JS_DIR } from './js-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));
void here;
const CORE = ['util', 'model', 'registry', 'engine', 'data', 'landuse', 'turns', 'calibrate', 'sim', 'sample'];

const inert = () => new Proxy(function () {}, { get: (t, k) => (k === Symbol.toPrimitive ? () => '' : inert()), apply: () => undefined });

// Onboarding.parseNum, read from the feature file so the interface's real number parser is used (ui-inspector.js falls back to a
// copy that is only used when the feature is not loaded)
function onboardingParseNum() {
  const t = readFileSync(join(JS_DIR, 'features', 'onboarding.js'), 'utf8');
  const m = /function parseNum\(v\) \{[\s\S]*?\n  \}/.exec(t);
  if (!m) throw new Error('Onboarding.parseNum not found');
  return m[0];
}

export function loadUi({ seed = 1, now = Date.UTC(2026, 9, 21) } = {}) {
  let s = seed >>> 0;
  const rand = () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  const M = Object.create(Math); M.random = rand;
  const store = new Map();
  const ctx = {
    console, JSON, Promise, Map, Set, WeakMap, Array, Object, Number, String, Boolean, Symbol, Error, TypeError, RangeError,
    Float64Array, Float32Array, Int32Array, Uint32Array, Uint8Array, Int8Array, Uint16Array, ArrayBuffer, DataView, Intl, RegExp,
    Math: M, Date: class extends Date { constructor(...a) { super(...(a.length ? a : [now])); } static now() { return now; } },
    setTimeout, clearTimeout, queueMicrotask, structuredClone, TextEncoder, TextDecoder, URL, URLSearchParams, AbortController,
    performance: { now: () => 0 }, fetch: () => { throw new Error('no network in the truth harness'); }, isFinite, isNaN, parseFloat, parseInt, Infinity, NaN,
    encodeURIComponent, decodeURIComponent,
    localStorage: { getItem: k => store.get(k) ?? null, setItem: (k, v) => store.set(k, String(v)), removeItem: k => store.delete(k) },
    // the drawing code asks Render what is under the pointer: nothing, so every click is a free click
    Render: new Proxy({ hit: () => null }, { get: (t, k) => (k in t ? t[k] : inert()) }),
    Dialogs: inert(), Drawer: inert(), L: inert(), CSS: { escape: x => String(x) },
    // the interface asks the page for its panels after an edit: any panel is a stand-in that takes and ignores what it is given
    document: { querySelector: () => inert(), querySelectorAll: () => [], getElementById: () => inert(), body: inert(), documentElement: inert() },
  };
  ctx.window = ctx; ctx.globalThis = ctx; ctx.self = ctx;
  vm.createContext(ctx);
  const read = n => readFileSync(join(JS_DIR, n + '.js'), 'utf8');
  const parts = CORE.map(n => `/* ${n}.js */\n` + read(n));
  parts.push('/* kit.js */\n' + read('kit'));
  // dialogs, toasts and menus do nothing without a screen (ui.js takes them from Kit when it is built)
  parts.push('/* harness */\nKit.toast = Kit.busy = Kit.menu = Kit.ask = Kit.confirmBox = () => undefined;\n' + onboardingParseNum() + '\nvar Onboarding = { parseNum };');
  parts.push('/* ui-inspector.js */\n' + read('ui-inspector'));
  let ui = read('ui');
  const hook = 'return {\n    init, toast,';
  if (!ui.includes(hook)) throw new Error('ui.js no longer returns its public object where the harness expects');
  ui = ui.replace(hook, 'return { __x: { finishRoad, finishBus, bindMap, duplicateScenario, setYear, deleteSelected, installUndo, setMap: m => { map = m; } },\n    init, toast,');
  parts.push('/* ui.js */\n' + ui);
  vm.runInContext(parts.join('\n;\n') + '\n;globalThis.__core = { U, Store, Net, Kit, View, Engine, Inspector, UI, CLASSES, ROUTE_COLOURS, newProject, newScenario, migrate };', ctx, { filename: 'ui-core.js' });
  return ctx.__core;
}
