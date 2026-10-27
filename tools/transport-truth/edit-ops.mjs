// The operations the editing truth tests replay: each one runs a model.js function (Net, Sites, Scenarios, Register, Store's undo)
// on a project, and the same operation list is replayed by the Rust port (crates/construct-transport/tests/edit_truth.rs), which
// must give the same return value and the same project afterwards. Used by gen-edit.mjs.
//
// An operation is { op, ...args }. A text argument that starts with '$' names an earlier result (`as: 'n1'` binds it; '$proj.sc'
// reads a key of it), so a network can be built node by node. Every op returns a plain value; mutating ops also record the
// project's state (rec: false leaves it out, rec: 'sc' or 'project' asks for one).
import { loadCore } from './js-core.mjs';

// As gen-expected.mjs: typed arrays as plain arrays, Maps and Sets as objects and arrays, non-finite numbers as strings
export const plain = v => {
  if (ArrayBuffer.isView(v)) return Array.from(v, plain);
  if (v instanceof Map) return Object.fromEntries([...v].map(([k, x]) => [String(k), plain(x)]));
  if (v instanceof Set) return [...v].map(plain);
  if (typeof v === 'number') return Number.isFinite(v) ? v : String(v);
  if (Array.isArray(v)) return v.map(plain);
  if (v && typeof v === 'object') { const o = {}; for (const [k, x] of Object.entries(v)) if (typeof x !== 'function' && x !== undefined) o[k] = plain(x); return o; }
  return v;
};
export const clone = o => JSON.parse(JSON.stringify(o));

// The harness's generator (js-core.mjs): to find how many numbers loading the scripts used
export const generator = seed => {
  let s = seed >>> 0;
  return () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
};

// Loading model.js draws the interface's tab id: one random number and one step of the id counter. Measured here, not assumed.
export function idStart(seed) {
  const core = loadCore({ seed });
  const id = core.U.uid('t'), draws = Array.from({ length: 8 }, generator(seed));
  const m = /^t(.{4})([0-9a-z]+?)([0-9a-z]{3})$/.exec(id);
  const skip = draws.findIndex(d => d.toString(36).slice(2, 5) === m[3]);
  if (skip < 0) throw new Error('the id does not come from the seeded generator: ' + id);
  return { rngSkip: skip, seq0: parseInt(m[2], 36) };
}

export function makeCtx(seed) {
  const core = loadCore({ seed });
  // Register is a top-level const of model.js that js-core.mjs does not export: a function made by that realm's own
  // Function constructor sees its global declarations
  const Register = core.U.dist.constructor('return Register')();
  return { core, Register, P: null, env: {}, seed };
}

const sc = c => c.P.scenarios.find(s => s.id === c.P.active) || c.P.scenarios[0];
// '$name' is an earlier result; '@n:3' is the node at position 3 (counting round) of the active scenario, '@l' a link, '@z' a zone, '@r' a route, '@j' a site
const lookup = (c, v) => {
  if (typeof v !== 'string') return v;
  if (v[0] === '$') return v.slice(1).split('.').reduce((a, k) => a[k], c.env);
  if (v[0] === '@') {
    const [k, i] = v.slice(1).split(':'), s = sc(c);
    const keys = Object.keys({ n: s.nodes, l: s.links, z: s.zones, r: s.routes, j: s.sites || {} }[k]);
    return keys.length ? keys[+i % keys.length] : undefined;
  }
  return v;
};
const resolve = (c, v) => (Array.isArray(v) ? v.map(x => resolve(c, x)) : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, resolve(c, x)])) : lookup(c, v));

// Named filters, so an operation list stays plain data (the Rust test has the same table)
const linkFilters = c => ({
  none: undefined,
  arterial: L => (c.core.CLASSES[L.cls] || c.core.CLASSES.local).rank >= 4,
  proposed: L => L.status === 'proposed',
  notProposed: L => L.status !== 'proposed',
  hasName: L => !!L.name,
  fundedOnly: L => !c.core.Scenarios.unfunded(L) && !c.core.Scenarios.programmed(L),
  keepAll: () => true,
});
const nodeFilters = c => ({
  none: undefined,
  signals: n => n.control === 'signals',
  connected: n => (c.core.Net.degree(sc(c)).get(n.id) || 0) > 0,
});
const zoneFilters = () => ({
  none: undefined,
  noGateways: z => z.kind !== 'gateway',
  residential: z => !!(z.lu && z.lu.residential),
});

const stateOf = c => {
  const s = sc(c), o = JSON.parse(JSON.stringify(s));
  delete o.res;
  return plain({ active: c.P.active, scenarios: c.P.scenarios.map(x => x.id), sc: o,
    order: { nodes: Object.keys(s.nodes), links: Object.keys(s.links), zones: Object.keys(s.zones), routes: Object.keys(s.routes), sites: Object.keys(s.sites || {}) } });
};
// Every scenario in full (without results), for the operations that make several
const projectState = c => {
  const P = JSON.parse(JSON.stringify(c.P));
  for (const s of P.scenarios) delete s.res;
  delete P.settings;
  return plain({ P, order: c.P.scenarios.map(s => ({ id: s.id, nodes: Object.keys(s.nodes), links: Object.keys(s.links), zones: Object.keys(s.zones), routes: Object.keys(s.routes), sites: Object.keys(s.sites || {}) })) });
};

const patch = (o, a) => { Object.assign(o, a.set || {}); for (const k of a.unset || []) delete o[k]; };

// name: [what to record after it (null, 'sc' the active scenario, 'project' all of it), (ctx, args) => return value]
export const OPS = {
  newProject: ['project', (c, a) => { c.P = c.core.newProject(a.name); c.core.Store.P = c.P; return { id: c.P.id, sc: c.P.scenarios[0].id }; }],
  settings: ['project', (c, a) => { Object.assign(c.P.settings, a.set); return null; }],
  data: ['project', (c, a) => { c.P.data = { ...(c.P.data || {}), ...a.set }; return null; }],
  register: ['project', (c, a) => { c.P.register = clone(a.rows); return null; }],
  presets: ['project', (c, a) => { c.P.register = c.Register.presets(a.asAt); return clone(c.P.register); }],
  selectScenario: ['project', (c, a) => { c.P.active = a.id; return null; }],
  addNode: ['sc', (c, a) => c.core.Net.addNode(sc(c), a.ll, a.props || {})],
  addLink: ['sc', (c, a) => c.core.Net.addLink(sc(c), a.a, a.b, a.pts || [], a.props || {})],
  splitLink: ['sc', (c, a) => c.core.Net.splitLink(sc(c), a.id, a.ll)],
  autoControl: ['sc', (c, a) => { c.core.Net.autoControl(sc(c), a.ids); return null; }],
  deleteLink: ['sc', (c, a) => { c.core.Net.deleteLink(sc(c), a.id); return null; }],
  deleteNode: ['sc', (c, a) => { c.core.Net.deleteNode(sc(c), a.id); return null; }],
  reverse: ['sc', (c, a) => { c.core.Net.reverse(sc(c), a.id); return null; }],
  setNode: ['sc', (c, a) => { patch(sc(c).nodes[a.id], a); return null; }],
  setLink: ['sc', (c, a) => { patch(sc(c).links[a.id], a); return null; }],
  setZone: ['sc', (c, a) => { patch(sc(c).zones[a.id], a); return null; }],
  setRoute: ['sc', (c, a) => { patch(sc(c).routes[a.id], a); return null; }],
  setScenario: ['sc', (c, a) => { patch(sc(c), a); return null; }],
  addZone: ['sc', (c, a) => { sc(c).zones[a.zone.id] = clone(a.zone); return null; }],
  addRoute: ['sc', (c, a) => { sc(c).routes[a.route.id] = clone(a.route); return null; }],
  nearestLink: [null, (c, a) => c.core.Net.nearestLink(sc(c), a.ll, a.maxM, linkFilters(c)[a.filter || 'none'])],
  nearestNode: [null, (c, a) => c.core.Net.nearestNode(sc(c), a.ll, a.maxM, nodeFilters(c)[a.filter || 'none'])],
  project: [null, (c, a) => { const s = sc(c), L = s.links[a.id]; return c.core.Net.project(s, L, a.ll); }],
  linksAt: [null, (c, a) => c.core.Net.linksAt(sc(c), a.id).map(L => L.id)],
  degree: [null, c => c.core.Net.degree(sc(c))],
  geomLen: [null, (c, a) => { const s = sc(c), L = s.links[a.id]; return { geom: c.core.Net.geom(s, L), len: c.core.Net.len(s, L) }; }],
  detect: ['sc', (c, a) => c.core.Sites.detect(sc(c), a.rules ? { rules: a.rules } : {})],
  merge: ['sc', (c, a) => c.core.Sites.merge(sc(c), a.ids)],
  splitSite: ['sc', (c, a) => c.core.Sites.split(sc(c), a.id)],
  siteName: [null, (c, a) => c.core.Sites.name(sc(c), { name: '', nodes: a.nodes })],
  siteOf: [null, (c, a) => { const s = c.core.Sites.of(sc(c), a.node); return s ? s.id : null; }],
  copy: ['project', (c, a) => {
    const f = {};
    if (a.links) f.links = linkFilters(c)[a.links];
    if (a.zones) f.zones = zoneFilters()[a.zones];
    const src = c.P.scenarios.find(s => s.id === (a.from || c.P.active));
    const out = c.core.Scenarios.copy(src, f);
    if (a.push) c.P.scenarios.push(out);
    return { id: out.id };
  }],
  sensitivityOf: ['project', (c, a) => { const s = c.core.Scenarios.sensitivityOf(c.P, a.sc || c.P.active, { unfunded: !!a.unfunded }); return s ? s.id : null; }],
  createSet: ['project', (c, a) => {
    const r = c.core.Scenarios.createSet(c.P, a.opts || {});
    return { scenarios: r.scenarios.map(s => s.id), excluded: r.excluded, warnings: r.warnings, set: r.set };
  }],
  describe: [null, (c, a) => c.core.Scenarios.describe(c.P.scenarios.find(s => s.id === (a.sc || c.P.active)), c.P)],
  describeEach: [null, c => c.P.scenarios.map(s => ({ id: s.id, d: c.core.Scenarios.describe(s, c.P) }))],
  checkEach: [null, c => c.P.scenarios.map(s => ({ id: s.id, c: c.core.Scenarios.check(s, c.P) }))],
  table: [null, c => c.core.Scenarios.table(c.P)],
  groups: [null, c => c.core.Scenarios.groups(c.P)],
  check: [null, (c, a) => c.core.Scenarios.check(c.P.scenarios.find(s => s.id === (a.sc || c.P.active)), c.P)],
  registerTable: [null, c => c.Register.table(c.P)],
  inBaseline: [null, (c, a) => c.Register.inBaseline(a.row)],
  zoneRole: [null, (c, a) => c.core.Scenarios.zoneRole(sc(c).zones[a.id])],
  linkStage: [null, (c, a) => c.core.Scenarios.linkStage(sc(c).links[a.id])],
  designYear: [null, (c, a) => c.core.Scenarios.designYear(a.type, a.opening)],
  snapshot: ['project', c => { c.core.Store.snapshot(); return null; }],
  undo: ['project', c => c.core.Store.undo()],
  redo: ['project', c => c.core.Store.redo()],
  historyLength: [null, c => ({ undo: c.core.Store.undoStack.length, redo: c.core.Store.redoStack.length })],
};

// Runs a list of operations on a fresh model loaded with the seed; returns the steps to record
export function run(seed, ops, extra = {}) {
  const c = makeCtx(seed), steps = [];
  c.core.Store.undoStack = []; c.core.Store.redoStack = [];
  for (const o of ops) {
    const def = extra[o.op] || OPS[o.op];
    if (!def) throw new Error('unknown operation ' + o.op);
    const args = resolve(c, o);
    const ret = def[1](c, args);
    if (o.as) c.env[o.as] = ret;
    const step = { ret: plain(ret === undefined ? null : ret) };
    const mode = o.rec === false ? null : typeof o.rec === 'string' ? o.rec : def[0];
    if (mode === 'sc') step.state = stateOf(c);
    else if (mode === 'project') step.state = projectState(c);
    steps.push(step);
  }
  return steps;
}
