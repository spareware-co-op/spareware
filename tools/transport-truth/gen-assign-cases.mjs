// Records the prototype's assignment on variants of the two truth-test projects that reach what the base runs do not: the
// options (demand factor, gap target, iteration limit, period), select-link, select-turn and select-zone (found and not
// found), charges by arc key and link id and time-of-use tolls, convergence rules (stability rule off, capacity damping
// off, custom thresholds), signal optimisation off, fixed timings, the junction methods and capacity methods, level of
// service methods, vehicle mix, BPR and signal constants, connectors, turn penalties and bans, development and committed
// zones, bus routes and lanes, closed links, the manual matrix with matrix estimation, junction sites of each control
// type, a zone no road reaches (unserved trips), more than 300 zones (no paths kept) and a settings object that is not
// the project's.
//
// Each case has the project as text (Store.serialise, one scenario, so the key order the engine read is kept), the
// scenario, the period, the options passed to Engine.assign (opts.settings, when the case has one, as text in
// settingsText), the calls the engine made to opts.onProgress (iteration, gap, message or null) and the result. The Rust test is tests/assign_cases.rs. Writes expected/assign-cases.json.
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCore } from './js-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const plain = v => {
  if (ArrayBuffer.isView(v)) return Array.from(v, plain);
  if (v instanceof Map) return Object.fromEntries([...v].map(([k, x]) => [String(k), plain(x)]));
  if (v instanceof Set) return [...v].map(plain);
  if (typeof v === 'number') return Number.isFinite(v) ? v : String(v);
  if (Array.isArray(v)) return v.map(plain);
  if (v && typeof v === 'object') { const o = {}; for (const [k, x] of Object.entries(v)) if (typeof x !== 'function') o[k] = plain(x); return o; }
  return v;
};
let seed = 5;
const rnd = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed / 4294967296; };

const cases = [];
const only = process.argv[2];
/* make(name, fixture, { mutate(P, sc, c), period, scIndex, opts(ctx) | opts, settings(S) }):
   omit lists keys of the result left out of the record (the Rust test leaves them out too); mutate changes the project; opts is an object or a function of { sc, P, base, G, c } (base = the plain assignment of the
   same scenario and period, G = its graph) giving the options; settings(S) gives the settings object to pass as
   opts.settings (a changed copy of the project's). */
const make = async (name, fixture, { mutate = () => {}, period = 'AM', scIndex = 0, opts = {}, settings = null, omit = [] } = {}) => {
  if (only && !name.includes(only)) return;
  const c = loadCore({ seed: 11 });
  const P = c.migrate(JSON.parse(readFileSync(join(here, 'fixtures', fixture + '.json'), 'utf8')));
  const sc = P.scenarios[scIndex];
  mutate(P, sc, c);
  P.scenarios = [sc];
  P.active = sc.id;
  c.Store.P = P;
  const { Engine } = c;
  const projectText = c.Store.serialise(P);
  let o = opts;
  if (typeof opts === 'function') {
    const base = await Engine.assign(sc, { period }), G = Engine.buildGraph(sc, P.settings, period);
    o = opts({ sc, P, base, G, c });
  }
  const callOpts = { period, ...o };
  let settingsText = null;
  if (settings) { const S2 = JSON.parse(JSON.stringify(P.settings)); settings(S2); settingsText = JSON.stringify(S2); callOpts.settings = S2; }
  const progress = [];
  callOpts.onProgress = (it, gap, msg) => progress.push([it, gap, msg ?? null]);
  const res = await Engine.assign(sc, callOpts);
  const { settings: _s, onProgress: _p, ...optsOut } = callOpts;
  const out = plain(res);
  for (const k of omit) delete out[k];
  cases.push({ name, fixture, scenario: sc.id, period, opts: optsOut, settingsText, projectText, omit, progress: plain(progress), res: out });
};
const busiest = base => Object.entries(base.arcs).sort((a, b) => (b[1].v || 0) - (a[1].v || 0))[0][0];
const firstTurn = (base, minV = 20) => { for (const rows of Object.values(base.turns)) for (const r of rows) if (r.v >= minV) return [r.from, r.to]; return ['none', 'none']; };
const nodeOf = (sc, control) => Object.values(sc.nodes).find(n => n.control === control);
const S = (patch) => s => Object.assign(s, patch);
const withS = patch => (P) => { Object.assign(P.settings, patch); };

// ---- options
await make('opt-demand-factor', 'grid', { opts: { demandFactor: 1.35 } });
await make('opt-demand-factor-text-pm', 'grid', { period: 'PM', opts: { demandFactor: '0.6' } });
await make('opt-demand-factor-zero', 'grid', { opts: { demandFactor: 0 } });
await make('opt-gap-and-iterations', 'grid', { opts: { gapTol: 1e-7, maxIter: 30 } });
await make('opt-max-iter-5', 'grid', { opts: { maxIter: 5 } });
await make('opt-max-iter-4-corridor', 'corridor', { opts: { maxIter: 4 } });
await make('opt-loose-gap-corridor', 'corridor', { opts: { gapTol: 0.02, maxIter: 40 } });
await make('opt-no-period-uses-settings', 'grid', { mutate: withS({ period: 'PM' }), opts: { period: undefined } });
// ---- select-link, select-turn and select-zone
await make('select-zone', 'grid', { opts: { select: { zone: 'zA' } } });
await make('select-zone-gateway', 'grid', { period: 'PM', opts: { select: { zone: 'gW' } } });
await make('select-zone-unknown', 'grid', { opts: { select: { zone: 'nope' } } });
await make('select-arc', 'grid', { opts: ({ base }) => ({ select: { arc: busiest(base) } }) });
await make('select-arc-connector', 'grid', { opts: ({ G }) => ({ select: { arc: G.arcs.find(a => a.conn).key } }) });
await make('select-arc-unknown', 'grid', { opts: { select: { arc: 'nope' } } });
await make('select-turn', 'grid', { opts: ({ base }) => ({ select: { turn: firstTurn(base) } }) });
await make('select-turn-unknown', 'grid', { opts: { select: { turn: ['a', 'b'] } } });
await make('select-turn-one-key', 'grid', { opts: ({ base }) => ({ select: { turn: [firstTurn(base)[0]] } }) });
await make('select-zone-and-arc', 'grid', { opts: ({ base }) => ({ select: { zone: 'nope', arc: busiest(base) } }) });
await make('select-empty', 'grid', { opts: { select: {} } });
await make('select-zone-corridor', 'corridor', { opts: { select: { zone: 'zDev' } } });
await make('select-arc-corridor', 'corridor', { period: 'PM', opts: ({ base }) => ({ select: { arc: busiest(base) } }) });
await make('select-turn-corridor', 'corridor', { opts: ({ base }) => ({ select: { turn: firstTurn(base) } }) });
await make('select-zone-corridor-site', 'corridor', { opts: { select: { zone: 'zCom' } } });
// ---- charges and tolls
await make('arc-penalty', 'grid', { opts: ({ base, sc }) => ({ arcPenalty: { [busiest(base)]: 60, [Object.keys(sc.links)[3]]: 30, bad: -5, x: 'abc', [Object.keys(sc.links)[5]]: null } }) });
await make('arc-penalty-corridor', 'corridor', { opts: ({ sc }) => ({ arcPenalty: { [Object.keys(sc.links)[2]]: 20 } }) });
await make('link-tolls', 'grid', { mutate: (P, sc) => { const L = Object.values(sc.links); L[2].toll = '25'; L[4].toll = { AM: '40', PM: 0 }; L[6].toll = -3; L[8].toll = true; L[10].toll = { AM: 15 }; } });
await make('link-tolls-pm', 'grid', { period: 'PM', mutate: (P, sc) => { const L = Object.values(sc.links); L[2].toll = 25; L[4].toll = { AM: 40, PM: 12 }; } });
// ---- convergence
await make('conv-stable-off', 'grid', { mutate: P => { P.settings.conv.stable = false; } });
await make('conv-stable-off-corridor', 'corridor', { mutate: P => { P.settings.conv.stable = false; } });
await make('conv-damp-none', 'grid', { mutate: P => { P.settings.conv.capDamp = 'none'; } });
await make('conv-damp-none-corridor', 'corridor', { mutate: P => { P.settings.conv.capDamp = 'none'; } });
await make('conv-custom', 'grid', { mutate: P => { P.settings.conv = { stable: true, chg: 0.02, share: 0.95, consec: 2, capDamp: 'msa' }; } });
await make('signal-opt-off', 'grid', { mutate: withS({ signalOpt: false }) });
await make('signal-opt-off-stable-off', 'grid', { mutate: P => { P.settings.signalOpt = false; P.settings.conv.stable = false; } });
await make('small-max-iter-with-signal-opt', 'grid', { mutate: withS({ maxIter: 12 }) });
await make('stable-off-small-max-iter', 'grid', { mutate: P => { P.settings.conv.stable = false; P.settings.maxIter = 12; } });
// ---- junction methods
await make('capacity-method-movement', 'grid', { mutate: P => { P.settings.junction.capMethod = 'movement'; } });
await make('capacity-method-movement-pm', 'grid', { period: 'PM', mutate: P => { P.settings.junction.capMethod = 'movement'; } });
await make('capacity-method-arc-corridor', 'corridor', { mutate: P => { P.settings.junction.capMethod = 'arc'; } });
await make('los-sidra', 'grid', { mutate: P => { P.settings.junction.losMethod = 'sidra'; } });
await make('los-rta-movement', 'corridor', { mutate: P => { P.settings.junction.losMethod = 'rta'; } });
await make('junction-pff-t', 'grid', { mutate: P => { Object.assign(P.settings.junction, { pff: 0.9, T: 1, queueSpace: 6, f95: 1.5 }); } });
await make('junction-pff-t-corridor', 'corridor', { mutate: P => { Object.assign(P.settings.junction, { pff: 0.85, T: 0.25, queueSpace: 8, f95: 1.8 }); } });
await make('fixed-timing-and-cycle', 'grid', { mutate: (P, sc) => { const n = nodeOf(sc, 'signals'); n.fixedTiming = true; n.cycle = 80; n.split = [0.6, 0.4]; } });
await make('cycle-set-not-fixed', 'grid', { mutate: (P, sc) => { const n = nodeOf(sc, 'signals'); n.cycle = '90'; } });
// ---- vehicles and link performance
await make('vehicle-mix', 'grid', { mutate: withS({ hvPct: 10, pcuHV: 2, busPcu: 3 }) });
await make('bpr-beta-3', 'grid', { mutate: withS({ bprAlpha: 0.2, bprBeta: 3 }) });
await make('bpr-beta-3-5', 'corridor', { mutate: withS({ bprAlpha: 0.2, bprBeta: 3.5 }) });
await make('signal-constants', 'grid', { mutate: withS({ satFlow: 1900, cycle: 90, lostPerPhase: 4 }) });
await make('connectors-and-speed', 'grid', { mutate: withS({ connectorSpeed: 30, connectors: 3 }) });
await make('co2-factor', 'grid', { mutate: withS({ co2Factor: 0.8 }) });
await make('gravity-beta', 'corridor', { mutate: withS({ gravityBeta: 0.3 }) });
// ---- turns
await make('turn-penalties-and-bans', 'grid', { mutate: (P, sc) => {
  P.settings.turns = { left: 8, right: 5, rightSignals: 6, uturn: 'ban' };
  const n = nodeOf(sc, 'priority'), ls = Object.values(sc.links).filter(l => l.a === n.id || l.b === n.id);
  n.bans = [ls[0].id + '>' + ls[1].id, ls[1].id + '>' + ls[0].id];
} });
await make('turn-uturn-penalty', 'grid', { mutate: (P, sc) => { P.settings.turns = { left: 6, right: 3, rightSignals: 4, uturn: 20 }; nodeOf(sc, 'stop').allowU = true; } });
await make('turn-uturn-text', 'corridor', { mutate: P => { P.settings.turns = { left: 4, right: 2, rightSignals: 3, uturn: '15' }; } });
// ---- zones and development tracking
await make('development-and-committed-zones', 'grid', { mutate: (P, sc) => { sc.zones.zA.role = 'development'; sc.zones.zB.role = 'committed'; } });
await make('development-zones-staged-pm', 'grid', { period: 'PM', mutate: (P, sc) => { sc.zones.zA.role = 'development'; sc.zones.zC.role = 'development'; sc.buildout = 60; sc.zones.zB.role = 'committed'; sc.zones.zB.buildout = 50; } });
await make('corridor-staged', 'corridor', { mutate: (P, sc) => { sc.buildout = 40; } });
await make('corridor-year-2040', 'corridor', { period: 'PM', mutate: (P, sc) => { sc.year = 2040; } });
await make('manual-od-and-estimation', 'grid', { mutate: (P, sc) => {
  sc.odManual = { AM: { zones: ['zA', 'zB', 'zC', 'gW', 'gE', 'gN'], m: Array.from({ length: 6 }, () => Array.from({ length: 6 }, () => Math.round(rnd() * 300) / 10)) } };
  P.me = { AM: { 'zA>gW': 1.5, 'gW>gE': 0.4 }, info: { AM: { baseYear: 2026 } } }; sc.zones.zA.existing = true;
} });
await make('estimation-factors-and-lu-factors', 'grid', { mutate: (P, sc) => {
  P.luFactors = { AM: { residential: 1.2, retail: 0.8, gateways: 1.1 } }; sc.zones.zA.existing = true; sc.zones.zB.existing = true;
  P.me = { AM: { 'zA>gW': 1.3, 'gE>zA': 0.7, 'gW>gN': 1.2 }, info: { AM: { baseYear: 2026, date: '2026-02-01' } } };
} });
await make('estimation-deltas-corridor', 'corridor', { mutate: (P, sc) => {
  P.me = { AM: { 'zEx>gW': 1.3 }, info: { AM: { baseYear: 2026, delta: { 'zEx>gW': 30, 'gE>zEx': -15, 'gW>gE': 20 } } } };
} });
await make('validated-status', 'corridor', { mutate: P => { P.me = { AM: { 'zEx>gW': 1.1 }, info: { AM: { baseYear: 2026, date: '2026-03-01' } } }; P.validation = { AM: { pass: true, independent: true, category: 'D', date: '2026-04-01' } }; P.settings.tmdg = { category: 'D', justification: 'Counts at each gateway' }; } });
await make('data-vintages', 'grid', { mutate: P => { P.data = { loaded: { nzta: { at: Date.UTC(2026, 0, 5), n: 12 }, user: { at: '2026-02-03T10:00:00Z', source: 'Council counts', n: 4, licence: 'CC BY' }, cas: { at: Date.UTC(2026, 5, 1), n: 55, label: '2021-2025' } } }; } });
// ---- network
await make('bus-lane-and-second-route', 'grid', { mutate: (P, sc) => {
  const L = Object.values(sc.links); L[2].busLane = true; L[3].busLane = true; L[11].busLane = true;
  sc.routes.r2 = { id: 'r2', name: 'Bus route 2', color: '#0c8599', freqs: { AM: '12', PM: '' }, freq: '6', wp: [L[1].a, L[2].b, L[3].b] };
} });
await make('route-frequencies', 'corridor', { period: 'PM', mutate: (P, sc) => { sc.routes.r1.freqs = { AM: 10, PM: 'x' }; sc.routes.r1.freq = 5; } });
await make('closed-link', 'grid', { mutate: (P, sc) => { Object.values(sc.links)[7].status = 'closed'; } });
await make('lanes-and-speeds', 'grid', { mutate: (P, sc) => {
  const L = Object.values(sc.links); L[1].lanesAB = 2; L[1].speed = '60'; L[4].cls = 'arterial'; L[4].lanesAB = 2; L[4].lanesBA = 2; L[9].capPerLane = 1500; L[12].speed = 30; L[12].varSpeed = { AM: true, v: 20 };
} });
await make('site-roundabout', 'corridor', { mutate: (P, sc) => { for (const id of Object.values(sc.sites)[0].nodes) sc.nodes[id].control = 'roundabout'; } });
await make('site-roundabout-pm', 'corridor', { period: 'PM', mutate: (P, sc) => { for (const id of Object.values(sc.sites)[0].nodes) sc.nodes[id].control = 'roundabout'; } });
await make('site-priority', 'corridor', { mutate: (P, sc) => { for (const id of Object.values(sc.sites)[0].nodes) sc.nodes[id].control = 'priority'; } });
await make('site-stop', 'corridor', { period: 'PM', mutate: (P, sc) => { for (const id of Object.values(sc.sites)[0].nodes) sc.nodes[id].control = 'stop'; } });
await make('site-signals-own-timing', 'corridor', { mutate: (P, sc) => { const s = Object.values(sc.sites)[0]; s.cycle = 90; s.control = 'signals'; s.name = 'Main Road signals'; } });
await make('site-signals-fixed', 'corridor', { mutate: (P, sc) => { const s = Object.values(sc.sites)[0]; s.fixedTiming = true; s.cycle = 70; s.atSiteNo = 4521; } });
await make('site-with-lanes', 'corridor', { mutate: (P, sc) => { const s = Object.values(sc.sites)[0]; s.lanes = {}; } });
await make('no-sites', 'corridor', { mutate: (P, sc) => { sc.sites = {}; } });
await make('isolated-gateway-unserved', 'grid', { mutate: (P, sc) => {
  sc.nodes.nIso = { id: 'nIso', lat: -36.915, lng: 174.75, control: 'none' };
  sc.zones.gX = { id: 'gX', kind: 'gateway', name: 'Cut off', lat: -36.915, lng: 174.7505, node: 'nIso', gw: { AMin: 150, AMout: 100, PMin: 120, PMout: 90 } };
  // no gravity trips reach a zone no road does (its free-flow time is infinite); a manual matrix sends trips there
  sc.odManual = { AM: { zones: ['zA', 'zB', 'zC', 'gW', 'gE', 'gN', 'gX'], m: Array.from({ length: 7 }, () => Array.from({ length: 7 }, () => Math.round(rnd() * 200) / 10)) } };
} });
await make('large-zone-count', 'grid', { mutate: (P, sc) => {
  const nodes = Object.keys(sc.nodes);
  for (let i = 0; i < 305; i++) {
    const id = 'zz' + i;
    sc.zones[id] = { id, kind: 'zone', name: 'Z' + i, lat: -36.9 - (i % 20) * 0.0004, lng: 174.762 + Math.floor(i / 20) * 0.0004, lu: { residential: 3 + (i % 5) }, connectors: null };
  }
} });
await make('more-than-150-zones', 'grid', { omit: ['paths'], mutate: (P, sc) => {
  for (let i = 0; i < 150; i++) {
    const id = 'zz' + i;
    sc.zones[id] = { id, kind: 'zone', name: 'Z' + i, lat: -36.9 - (i % 15) * 0.0005, lng: 174.762 + Math.floor(i / 15) * 0.0005, lu: { residential: 2 + (i % 4) }, connectors: null };
  }
} });
// ---- degenerate scenarios
await make('no-zones', 'grid', { mutate: (P, sc) => { sc.zones = {}; sc.routes = {}; } });
await make('one-zone-only', 'grid', { mutate: (P, sc) => { for (const id of Object.keys(sc.zones)) if (id !== 'zA') delete sc.zones[id]; } });
await make('no-links', 'grid', { mutate: (P, sc) => { sc.links = {}; sc.zones = {}; sc.routes = {}; } });
await make('zones-without-network', 'grid', { mutate: (P, sc) => { sc.links = {}; sc.nodes = {}; sc.routes = {}; for (const z of Object.values(sc.zones)) delete z.node; } });
// ---- settings passed in that are not the project's
await make('settings-argument-differs', 'grid', { settings: s => { s.bprBeta = 3; s.gravityBeta = 0.2; s.growth = 3; s.year = 2031; s.conv.stable = false; } });
await make('settings-argument-differs-corridor', 'corridor', { period: 'PM', settings: s => { s.junction.capMethod = 'arc'; s.hvPct = 12; s.signalOpt = false; } });

writeFileSync(join(here, 'expected', 'assign-cases.json'), JSON.stringify(cases));
console.log('assign-cases written:', cases.length, 'cases');
