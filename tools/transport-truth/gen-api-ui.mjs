// Records what the prototype's interface does for each editing action, for the Rust command layer
// (projects/spare-construct/crates/construct-api, tests/ui_truth.rs). Each case opens a truth-test project, then does a list of steps
// through the interface's own code (ui.js finishRoad, finishBus and the zone and gateway click handlers, deleteSelected,
// duplicateScenario, setYear and undo; ui-inspector.js applyField; the reverse and move lines), recording after each step the
// active scenario, the ids it made, the undo and redo labels and whether each result is out of date. Writes expected/api-ui.json.
//
// Ids: the model is loaded with the harness's seeded generator and a fixed clock, so every id is the same on every run. Loading
// the scripts draws some numbers first, so each case also records how many (rngSkip) and where the id counter starts (seq0); the
// Rust test replays the same.
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadUi } from './ui-core.mjs';
import { plain } from './edit-ops.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const NOW = Date.UTC(2026, 9, 21);

// How many numbers loading the scripts draws, and where the id counter stands: one probe id from a fresh load (as edit-ops.mjs idStart)
function idStartUi(seed) {
  const c = loadUi({ seed, now: NOW });
  const id = c.U.uid('t');
  let s = seed >>> 0;
  const draws = Array.from({ length: 12 }, () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; });
  const m = /^t(.{4})([0-9a-z]+?)([0-9a-z]{3})$/.exec(id);
  const skip = draws.findIndex(d => d.toString(36).slice(2, 5) === m[3]);
  if (skip < 0) throw new Error('the id does not come from the seeded generator: ' + id);
  return { rngSkip: skip, seq0: parseInt(m[2], 36) };
}

const TEMPLATE = { cls: 'arterial', lanes: 1, oneway: false, speed: 50, status: 'proposed' };

async function runCase(def) {
  const c = loadUi({ seed: def.seed, now: NOW });
  const { Store, View, UI, Kit, Net, U, Inspector, Engine } = c;
  const P = c.migrate(JSON.parse(readFileSync(join(here, 'inputs', def.project + '.json'), 'utf8')));
  Store.P = P; Store.undoStack = []; Store.redoStack = [];
  UI.__x.installUndo();
  // the stand-in map: it records the handlers the interface binds, so a click can be sent to them
  const handlers = {};
  const el = { addEventListener() {}, closest() { return null; }, classList: { add() {}, remove() {}, toggle() {} }, setPointerCapture() {}, releasePointerCapture() {} };
  UI.__x.setMap({ getContainer: () => el, on: (n, f) => { handlers[n] = f; }, mouseEventToContainerPoint: () => ({}), dragging: { enable() {}, disable() {} } });
  UI.__x.bindMap();

  const refs = {};
  const sc = () => Store.sc;
  const keysOf = (o) => new Set(Object.keys(o));
  const added = (before, o) => Object.keys(o).filter(k => !before.has(k));
  // '@n:3' is the node at position 3 of the active scenario, '@l' a link, '@z' a zone, '@r' a route; '$name.path' reads an earlier result
  const pick = (v) => {
    if (typeof v !== 'string') return v;
    if (v[0] === '$') return v.slice(1).split('.').reduce((a, k) => a[k], refs);
    if (v[0] === '@') { const [k, i] = v.slice(1).split(':'); return Object.keys({ n: sc().nodes, l: sc().links, z: sc().zones, r: sc().routes }[k])[+i]; }
    return v;
  };
  const resolve = (v) => (Array.isArray(v) ? v.map(resolve) : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, resolve(x)])) : pick(v));
  const nodeLL = id => [sc().nodes[id].lat, sc().nodes[id].lng];

  const DO = {
    async run(st) { for (const p of st.periods) sc().res[p] = await Engine.assign(sc(), { period: p }); return {}; },
    addRoad(st) {
      const b = { l: keysOf(sc().links), n: keysOf(sc().nodes) };
      Object.assign(View.template, TEMPLATE, st.template);
      View.draft = { kind: 'road', pts: st.points.map(p => ({ type: p.type, id: p.id, ll: [p.lat, p.lng] })) };
      UI.__x.finishRoad();
      return { links: added(b.l, sc().links), nodes: added(b.n, sc().nodes) };
    },
    addBus(st) {
      const b = keysOf(sc().routes);
      View.draft = { kind: 'bus', pts: st.stops.map(id => ({ type: 'node', id, ll: nodeLL(id) })) };
      UI.__x.finishBus();
      return { routes: added(b, sc().routes) };
    },
    addZone(st) { const b = keysOf(sc().zones); View.tool = 'zone'; handlers.click({ latlng: { lat: st.lat, lng: st.lng }, containerPoint: {} }); View.tool = 'select'; return { zones: added(b, sc().zones) }; },
    addGateway(st) { const b = keysOf(sc().zones); View.tool = 'gateway'; handlers.click({ latlng: { lat: st.lat, lng: st.lng }, containerPoint: {} }); View.tool = 'select'; return { zones: added(b, sc().zones) }; },
    delete(st) { View.sel = { type: st.type, id: st.id }; UI.deleteSelected(); return {}; },
    field(st) {
      const t = { dataset: { obj: st.kind, id: st.id, k: st.k, t: st.t }, type: st.t === 'bool' ? 'checkbox' : 'text', value: st.v, checked: st.v, validity: {} };
      const r = Inspector.applyField(t);
      return { rejected: r === false };
    },
    reverse(st) { Store.snapshot(); Net.reverse(sc(), st.id); Store.edited(); return {}; },
    move(st) {
      const o = st.kind === 'node' ? sc().nodes[st.id] : sc().zones[st.id];
      Kit.snap(st.kind === 'node' ? `moving ${Kit.junctionName(st.id)}` : `moving ${(o && o.name) || 'a zone'}`);
      o.lat = U.r7(st.lat); o.lng = U.r7(st.lng);
      Store.edited(st.kind === 'zone' ? 'demand' : 'net');
      return {};
    },
    copy() { UI.__x.duplicateScenario(); return { id: Store.P.active }; },
    newScenario() { const s = c.newScenario('Scenario ' + (Store.P.scenarios.length + 1)); Store.P.scenarios.push(s); Store.P.active = s.id; return { id: s.id }; },
    setYear(st) { UI.__x.setYear(st.year); return {}; },
    select(st) { Store.P.active = st.id; return {}; },
    setBase() { Store.P.base = Store.sc.id; return {}; },
    undo(st) { UI.undo(!!st.redo); return {}; },
  };

  const steps = [];
  for (const raw of def.steps) {
    const st = resolve(raw);
    // node points of a road carry their position, so the recorded step is complete
    if (st.points) for (const p of st.points) if (p.type === 'node' && p.lat === undefined) [p.lat, p.lng] = nodeLL(p.id);
    const out = await DO[st.do](st);
    if (raw.as) refs[raw.as] = out;
    const s = sc();
    const { res, ...state } = s;
    steps.push({
      step: st, result: out,
      active: P.active, base: P.base,
      scenarios: P.scenarios.map(x => ({ id: x.id, name: x.name, year: x.year ?? null, nodes: Object.keys(x.nodes).length, links: Object.keys(x.links).length, zones: Object.keys(x.zones).length, routes: Object.keys(x.routes).length })),
      scenario: plain(state),
      undo: Store.undoStack.map(u => u.label || ''), redo: Store.redoStack.map(u => u.label || ''),
      stale: Object.fromEntries(P.scenarios.map(x => [x.id, Object.fromEntries(Object.entries(x.res || {}).map(([p, r]) => [p, !!(r && r.stale)]))])),
    });
  }
  return { name: def.name, project: def.project, seed: def.seed, now: NOW, ...idStartUi(def.seed), steps };
}

const node = i => ({ type: 'node', id: `@n:${i}` });
const free = (lat, lng) => ({ type: 'free', lat, lng });
const onLink = (lat, lng) => ({ type: 'link', lat, lng });

const GRID = {
  name: 'grid-editing', project: 'grid', seed: 31,
  steps: [
    { do: 'run', periods: ['AM', 'PM'] },
    // a road from a junction, with a shape point and a free end: junctions, a link, default control
    { do: 'addRoad', as: 'r1', points: [node(2), free(-36.8975, 174.767), free(-36.895, 174.766)], template: { cls: 'collector', lanes: 2, speed: 60 } },
    // both ends on existing roads: each is split, then the road joins the two new junctions
    { do: 'addRoad', as: 'r2', points: [onLink(-36.9025, 174.7645), onLink(-36.905, 174.7645)], template: {} },
    // a one-way road from a junction to a free end
    { do: 'addRoad', as: 'r3', points: [node(0), free(-36.8975, 174.7585)], template: { cls: 'local', oneway: true, speed: 40, status: 'existing' } },
    // the same junction twice: no road, no undo step
    { do: 'addRoad', as: 'r4', points: [node(7), node(7)], template: {} },
    // a road ending on a junction through three points, one of them a click on a road
    { do: 'addRoad', as: 'r5', points: [node(5), onLink(-36.9, 174.7615), free(-36.8985, 174.7615), node(10)], template: { cls: 'ramp', lanes: 1, speed: 70 } },
    { do: 'addBus', as: 'b1', stops: ['@n:3', '@n:8', '@n:13'] },
    { do: 'addZone', as: 'z1', lat: -36.9075, lng: 174.771 },
    { do: 'addGateway', as: 'g1', lat: -36.9078, lng: 174.7722 },
    { do: 'addGateway', as: 'g2', lat: -36.895, lng: 174.7661 },
    // fields of a road: a class change takes the class's speed only while the speed was the old class's
    { do: 'field', kind: 'link', id: '$r1.links.0', k: 'cls', t: 'str', v: 'motorway' },
    { do: 'field', kind: 'link', id: '@l:3', k: 'speed', t: 'int', v: 60 },
    { do: 'field', kind: 'link', id: '@l:3', k: 'cls', t: 'str', v: 'collector' },
    { do: 'field', kind: 'link', id: '@l:3', k: 'lanesAB', t: 'int', v: 3 },
    { do: 'field', kind: 'link', id: '@l:3', k: 'busLane', t: 'bool', v: true },
    { do: 'field', kind: 'link', id: '@l:3', k: 'lanesAB', t: 'int', v: '1' },
    { do: 'field', kind: 'link', id: '@l:4', k: 'lanesAB', t: 'int', v: 0 },
    { do: 'field', kind: 'link', id: '@l:4', k: 'lanesBA', t: 'int', v: 0 },
    { do: 'field', kind: 'link', id: '@l:5', k: 'capPerLane', t: 'numnull', v: '1,150' },
    { do: 'field', kind: 'link', id: '@l:5', k: 'capPerLane', t: 'numnull', v: '' },
    { do: 'field', kind: 'link', id: '@l:5', k: 'status', t: 'str', v: 'closed' },
    { do: 'field', kind: 'link', id: '@l:5', k: 'cycle', t: 'str', v: 'separated' },
    { do: 'field', kind: 'link', id: '@l:5', k: 'name', t: 'str', v: '  Renamed Road  ' },
    // a junction control sets the upgrade flag
    { do: 'field', kind: 'node', id: '@n:6', k: 'control', t: 'str', v: 'signals' },
    { do: 'field', kind: 'node', id: '@n:6', k: 'cycle', t: 'numnull', v: 90 },
    { do: 'field', kind: 'node', id: '@n:6', k: 'fixedTiming', t: 'bool', v: true },
    { do: 'field', kind: 'node', id: '@n:6', k: 'control', t: 'str', v: 'priority' },
    // zones, gateways and routes
    { do: 'field', kind: 'zone', id: '@z:0', k: 'lu.residential', t: 'num', v: '1,200' },
    { do: 'field', kind: 'zone', id: '@z:0', k: 'lu.retail', t: 'num', v: '5 000 m2' },
    { do: 'field', kind: 'zone', id: '@z:0', k: 'connectors', t: 'numnull', v: 2 },
    { do: 'field', kind: 'zone', id: '@z:0', k: 'connectors', t: 'numnull', v: '' },
    { do: 'field', kind: 'zone', id: '@z:0', k: 'role', t: 'str', v: 'development' },
    { do: 'field', kind: 'zone', id: '@z:3', k: 'gw.AMin', t: 'num', v: 720 },
    { do: 'field', kind: 'zone', id: '@z:3', k: 'lu.office', t: 'num', v: -5 },
    { do: 'field', kind: 'zone', id: '@z:0', k: 'lu.residential', t: 'num', v: 'lots' },
    { do: 'field', kind: 'route', id: '@r:0', k: 'freq', t: 'num', v: '12' },
    { do: 'field', kind: 'route', id: '@r:0', k: 'color', t: 'str', v: '#123456' },
    // results are current again, then a name (which changes nothing in the model) and the forecast year (which does)
    { do: 'run', periods: ['AM'] },
    { do: 'field', kind: 'scenario', id: null, k: 'name', t: 'str', v: 'Renamed scenario' },
    { do: 'field', kind: 'scenario', id: null, k: 'notes', t: 'str', v: 'A note' },
    { do: 'field', kind: 'route', id: '@r:0', k: 'name', t: 'str', v: 'Route A' },
    { do: 'setYear', year: 2031 },
    { do: 'run', periods: ['AM'] },
    { do: 'reverse', id: '$r3.links.0' },
    { do: 'move', kind: 'node', id: '@n:12', lat: -36.90751234567, lng: 174.77212345678 },
    { do: 'move', kind: 'zone', id: '@z:1', lat: -36.9, lng: 174.77 },
    // deleting: a road, a junction with a gateway and a bus route on it, a zone, a gateway, a bus route
    { do: 'delete', type: 'link', id: '$r1.links.0' },
    { do: 'delete', type: 'node', id: '@n:2' },
    { do: 'delete', type: 'zone', id: '@z:1' },
    { do: 'delete', type: 'zone', id: '$g1.zones.0' },
    { do: 'delete', type: 'route', id: '@r:0' },
    // scenarios
    { do: 'copy', as: 'c1' },
    { do: 'addZone', lat: -36.91, lng: 174.762 },
    { do: 'newScenario' },
    { do: 'setYear', year: 2041 },
    { do: 'setBase' },
    { do: 'select', id: '$c1.id' },
    { do: 'undo' }, { do: 'undo' }, { do: 'undo' }, { do: 'undo' }, { do: 'undo' },
    { do: 'undo', redo: true }, { do: 'undo', redo: true },
    { do: 'undo' }, { do: 'undo' }, { do: 'undo' }, { do: 'undo' }, { do: 'undo' }, { do: 'undo' }, { do: 'undo' }, { do: 'undo' },
    { do: 'undo', redo: true }, { do: 'undo', redo: true }, { do: 'undo', redo: true },
    // a new edit clears the redo steps
    { do: 'addZone', lat: -36.92, lng: 174.77 },
    { do: 'undo', redo: true },
  ],
};

const CORRIDOR = {
  name: 'corridor-editing', project: 'corridor', seed: 77,
  steps: [
    { do: 'run', periods: ['AM'] },
    { do: 'addRoad', as: 'a', points: [node(0), free(-36.9, 174.75)], template: { cls: 'collector', lanes: 1, speed: 50 } },
    { do: 'addRoad', as: 'b', points: [node(1), node(3), node(1)], template: { cls: 'local', lanes: 1, speed: 30, status: 'proposed' } },
    { do: 'addZone', lat: -36.9, lng: 174.76 },
    { do: 'addGateway', lat: -36.9, lng: 174.75 },
    { do: 'field', kind: 'link', id: '$a.links.0', k: 'lanesAB', t: 'int', v: 2 },
    { do: 'field', kind: 'link', id: '$a.links.0', k: 'busLane', t: 'bool', v: true },
    { do: 'reverse', id: '$a.links.0' },
    { do: 'delete', type: 'node', id: '$a.nodes.0' },
    { do: 'undo' }, { do: 'undo' }, { do: 'undo', redo: true },
  ],
};

// The walk and cycle catchments (Kit.catchment, kit.js), for several junctions of every scenario of both truth projects
{
  const rec = [];
  for (const project of ['grid', 'corridor']) {
    const c = loadUi({ seed: 1, now: NOW });
    const P = c.migrate(JSON.parse(readFileSync(join(here, 'inputs', project + '.json'), 'utf8')));
    c.Store.P = P;
    for (const sc of P.scenarios) {
      const ids = Object.keys(sc.nodes);
      for (const id of [ids[0], ids[Math.floor(ids.length / 2)], ids[ids.length - 1]]) {
        for (const mode of ['walk', 'cycle']) {
          const r = c.Kit.catchment(sc, id, mode);
          rec.push({ project, scenario: sc.id, from: id, mode, result: plain({ from: r.from, times: r.times, mode: r.mode, basis: r.basis }) });
        }
      }
    }
  }
  writeFileSync(join(here, 'expected', 'api-catchment.json'), JSON.stringify(rec));
  console.log('api-catchment.json', rec.length, 'catchments');
}

const out = [];
for (const def of [GRID, CORRIDOR]) { const r = await runCase(def); out.push(r); console.log(def.name, r.steps.length, 'steps'); }
writeFileSync(join(here, 'expected', 'api-ui.json'), JSON.stringify(out));
console.log('api-ui.json written');
process.exit(0);
