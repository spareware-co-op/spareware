// Part 'f3': Compliance+ parking, cycle parking, end-of-trip and loading (compliance-extra.js section 11), urban subdivision (12), zone-to-zone
// times and policy alignment (13, 14), IPM stage 2 GPS metrics and funding attribution (15, 16), time-of-use screening (17), climate hazards (18),
// the hand-over packs (19), the CSV exports of these and their report sections and exports
// -> expected/compliance-f3-parking.json, -subdivision.json, -policy.json, -gps.json, -tou.json, -packs.json
// Fixtures: fixtures/data/compliance-f3-*.json
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import vm from 'node:vm';
import { part, write, record, variant, withResults, saveFixture, plain, JS_DIR, NOW } from './gen-compliance-lib.mjs';

// The prototype in the same sandbox gen-compliance-lib.mjs builds, with the private functions of compliance-extra.js this part checks exposed as
// ComplianceExtra.__fn (the shared loader only exposes the module's constants)
const PRIVATE = ['jobsOf', 'placeOf', 'limitOf', 'affectedLinks', 'roadsToLinks', 'round9', 'distSkim', 'signalNodes', 'packCSV', 'r1', 'r3', 'postedOf', 'blankPark'];
function loadF3({ seed = 1, now = NOW } = {}) {
  const inert = () => new Proxy(function () {}, { get: (t, k) => (k === Symbol.toPrimitive ? () => '' : inert()), apply: () => undefined });
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
  const expose = src => {
    const i = src.lastIndexOf('\n  return {');
    const fns = PRIVATE.map(n => `try { __f.${n} = ${n}; } catch (e) { }`).join('\n  ');
    return src.slice(0, i) + `\n  const __f = {};\n  ${fns}\n  return { __fn: __f,` + src.slice(i + '\n  return {'.length);
  };
  const CORE = ['util', 'model', 'registry', 'kit', 'engine', 'data', 'landuse', 'turns', 'calibrate', 'sim', 'sample'];
  const src = CORE.map(n => `/* ${n}.js */\n` + read(n)).concat(['packs/nz/compliance', 'packs/nz/compliance-extra'].map(n => `/* ${n}.js */\n` + (n.endsWith('extra') ? expose(read(n)) : read(n)))).join('\n;\n');
  vm.runInContext(src + `\n;globalThis.__core = { U, Store, Net, Importers, Scenarios, Sites, Engine, DataHub, Register, Kit, Registry, Calibrate, Turns, Compliance, ComplianceExtra,
    newProject, newScenario, defaultSettings, migrate, CLASSES, LANDUSES, CONTROLS, STATUS, CYCLE, SCENARIO_ROLES, FUNDING, REGISTER_TYPES };`, ctx, { filename: 'compliance-core.js' });
  return ctx.__core;
}

// The report lane's table block, as a stand-in the Rust test builds the same way (a report context has table(title, html, { note }))
const mockCtx = { table: (title, html, o) => `<section data-title="${title}">${html}<aside>${(o && o.note) || ''}</aside></section>` };
// Random ids (U.uid) cannot match: every id that looks like one is replaced
const normUid = v => {
  if (Array.isArray(v)) return v.map(normUid);
  if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, k === 'id' && typeof x === 'string' && /^pk[0-9a-z]{3,}$/.test(x) ? '<uid>' : normUid(x)]));
  return v;
};

part('f3', async () => {
  const c = loadF3();
  const X = c.ComplianceExtra, R = c.Registry, F = X.__fn;
  const files = {};
  // A set to '$DELETE' removes the entry (the saved projects cannot hold a missing result as a null); the case lists the paths in `del`
  const delPath = (o, path) => { const p = path.split('.'); let t = o; for (const k of p.slice(0, -1)) { if (t == null || typeof t !== 'object') return; t = t[k]; } if (t != null && typeof t === 'object') delete t[p[p.length - 1]]; };
  const add = async (file, id, call, fixture, sets, f, extra = {}) => {
    const keep = {}, del = [];
    for (const [k, v] of Object.entries(sets || {})) { if (v === '$DELETE') del.push(k); else keep[k] = v; }
    // the policy alignment calls the Future Development Strategy check of another part: its answer is kept so a build that has not got that part can still
    // check the rest (the Rust test ignores it when the part is there)
    const needFds = call === 'policy' || call === 'policyCSV' || extra.report === 'compliance-extra-policy' || extra.export === 'compliance-extra-policy';
    let fds;
    const r = await record(c, fixture, keep, (P, cc) => { for (const d of del) delPath(P, d); const out = f(P, cc); if (needFds) fds = plain(c.Compliance.fdsCheck(P)); return out; });
    if (r.ok !== undefined) r.ok = normUid(r.ok);
    (files[file] ??= []).push({ id, call, ...extra, ...(del.length ? { del } : {}), ...(needFds ? { fds } : {}), ...r });
  };
  const flush = () => { for (const [name, cases] of Object.entries(files)) write(`expected/compliance-f3-${name}.json`, { cases }); };
  const report = id => (P, ctx) => R.get('report', id).render(P, ctx);
  const exp = id => R.get('exports', 'compliance-extra-' + id);

  /* ====================================================================================
     11. Parking, cycle parking, end-of-trip and loading
     ==================================================================================== */
  const PK = 'parking';
  const acts = Object.keys(X.PARK_ACT);
  const rowsSet = rows => ({ 'complianceExtra.parking.rows': rows });
  // the pure helpers
  for (const q of [0, 19, 20, 200, 201, 1000, 1001, 10001, 20000, 12.5]) {
    for (const act of ['residential', 'office', 'retail', 'nothing']) {
      for (const row of [{}, { garage: true }, { noGarage: 3 }, { noGarage: '4', garage: true }, { cycleVisitorReq: 2, cycleSecureReq: 6 }, { cycleVisitorReq: '1.5' }]) {
        await add(PK, `cycleRates ${act} ${JSON.stringify(q)} ${JSON.stringify(row)}`, 'cycleRates', 'corridor', {}, () => X.cycleRates(act, +q || 0, row), { args: { act, q: +q || 0, row } });
      }
    }
  }
  for (const g of [0, 1, 300, 500, 501, 1000, 2500, 2501, 5000, 7500, 7501, 15000, 15001, 22500, 100000, 1e6, -1]) {
    await add(PK, `showersFor ${g}`, 'showersFor', 'corridor', {}, () => X.showersFor(g), { args: { g } });
    for (const grp of ['retail', 'other', 'res', 'x']) await add(PK, `loadingFor ${grp} ${g}`, 'loadingFor', 'corridor', {}, () => X.loadingFor(grp, g), { args: { grp, g } });
  }
  for (const n of [0, 1, 2, 19, 20, 21, 49, 50, 51, 100, 101, 150, 151, 200, 201, 1000, '30', 'x', null, -3, 20.5, 1e5]) {
    await add(PK, `accessibleFor ${JSON.stringify(n)}`, 'accessibleFor', 'corridor', {}, () => X.accessibleFor(n), { args: { n } });
  }
  // the compliance table
  const cases = [
    ['no rows', []],
    ['office complies', [{ id: 'a', activity: 'office', qty: 1500, maxCars: 30, cars: 20, accessible: 1, cycleVisitor: 2, cycleSecure: 6, showers: 1, loading: 0 }]],
    ['office over the maximum', [{ id: 'a', activity: 'office', qty: 1500, maxCars: 10, cars: 25, accessible: 2, cycleVisitor: 2, cycleSecure: 6, showers: 1, loading: 0 }]],
    ['office no accessible', [{ id: 'a', activity: 'office', qty: 3000, maxCars: 100, cars: 40, accessible: 0, cycleVisitor: 3, cycleSecure: 10, showers: 2, loading: 0 }]],
    ['office too few accessible', [{ id: 'a', activity: 'office', qty: 3000, maxCars: 100, cars: 120, accessible: 2, cycleVisitor: 3, cycleSecure: 10, showers: 2, loading: 0 }]],
    ['office big', [{ id: 'a', activity: 'office', qty: 30000, maxCars: 400, cars: 200, accessible: 6, cycleVisitor: 40, cycleSecure: 100, showers: 10, loading: 2 }]],
    ['office big short of everything', [{ id: 'a', activity: 'office', qty: 30000, maxCars: 400, cars: 200, accessible: 6 }]],
    ['residential 60 dwellings', [{ id: 'a', activity: 'residential', qty: 60, cars: 0, cycleVisitor: 3, cycleSecure: 60 }]],
    ['residential garages', [{ id: 'a', activity: 'residential', qty: 60, garage: true, cars: 60, maxCars: 60, accessible: 3, cycleVisitor: 3 }]],
    ['residential no garage rate', [{ id: 'a', activity: 'residential', qty: 60, noGarage: 24, cycleVisitor: 3, cycleSecure: 24 }]],
    ['residential 19', [{ id: 'a', activity: 'residential', qty: 19 }]],
    ['retail no rate entered', [{ id: 'a', activity: 'retail', qty: 4000, maxCars: 100, cars: 90, accessible: 3, loading: 1 }]],
    ['retail rates entered', [{ id: 'a', activity: 'retail', qty: 4000, cycleVisitorReq: 4, cycleSecureReq: 8, cycleVisitor: 4, cycleSecure: 8, loading: 1 }]],
    ['retail big', [{ id: 'a', activity: 'retail', qty: 15000, cycleVisitorReq: 10, cycleSecureReq: 20, cars: 300, accessible: 7, loading: 2 }]],
    ['food', [{ id: 'a', activity: 'food', qty: 400, cars: 5, accessible: 1 }]],
    ['industrial', [{ id: 'a', activity: 'industrial', qty: 12000, cars: 50, accessible: 2, loading: 3 }]],
    ['childcare listed zone no parking', [{ id: 'a', activity: 'childcare', qty: 400, cars: 0, listedZone: true, accessible: 0 }]],
    ['childcare listed zone with accessible', [{ id: 'a', activity: 'childcare', qty: 400, cars: 0, listedZone: true, accessible: 1 }]],
    ['childcare unlisted', [{ id: 'a', activity: 'childcare', qty: 400, cars: 0 }]],
    ['school', [{ id: 'a', activity: 'school', qty: 8000, cars: 30, accessible: 2 }]],
    ['tertiary', [{ id: 'a', activity: 'tertiary', qty: 25000, cars: 0, listedZone: true, accessible: 1, loading: 1 }]],
    ['medical', [{ id: 'a', activity: 'medical', qty: 600, cars: 12, accessible: 1 }]],
    ['hospital', [{ id: 'a', activity: 'hospital', qty: 100000, cars: 400, accessible: 10, loading: 3 }]],
    ['community', [{ id: 'a', activity: 'community', qty: 700, cars: 0 }]],
    ['other and unknown activity', [{ id: 'a', activity: 'other', qty: 800, cars: 0 }, { id: 'b', activity: 'spaceport', qty: 6000, cars: 3 }]],
    ['mixed development', [
      { id: 'a', activity: 'residential', qty: 120, garage: true, cars: 120, maxCars: 150, accessible: 3, cycleVisitor: 6 },
      { id: 'b', activity: 'office', qty: 2400, maxCars: 20, cars: 35, accessible: 2, cycleVisitor: 2, cycleSecure: 8, showers: 1 },
      { id: 'c', activity: 'retail', qty: 900, cycleVisitorReq: 1, cars: 10, accessible: 1 },
      { id: 'd', activity: 'food', qty: 300, cars: 0 }]],
    ['totals met only in total', [
      { id: 'a', activity: 'office', qty: 600, cars: 5, accessible: 1, cycleVisitor: 2, cycleSecure: 3, showers: 1 },
      { id: 'b', activity: 'office', qty: 600, cars: 5, accessible: 1, cycleVisitor: 0, cycleSecure: 1, showers: 1 }]],
    ['entered as text', [{ id: 'a', activity: 'office', qty: '1500', maxCars: '10', cars: '25', accessible: '1', cycleVisitor: '2', cycleSecure: '6', showers: '1', loading: '0' }]],
    ['non-numeric entries', [{ id: 'a', activity: 'office', qty: 'lots', maxCars: 'n/a', cars: 'many', accessible: 'x', cycleVisitor: null, cycleSecure: '', showers: 'y', loading: [] }]],
    ['blank max', [{ id: 'a', activity: 'office', qty: 500, maxCars: '', cars: 50 }, { id: 'b', activity: 'office', qty: 500, maxCars: null, cars: 50 }, { id: 'c', activity: 'office', qty: 500, maxCars: 0, cars: 4 }]],
    ['negative and fractional', [{ id: 'a', activity: 'office', qty: -40, cars: -5, maxCars: -1, accessible: -1 }, { id: 'b', activity: 'residential', qty: 22.5, cars: 7.5, maxCars: 5.25, accessible: 0.5 }]],
    ['row without an id', [{ activity: 'office', qty: 300, cars: 2 }, {}]],
    ['row with extra fields', [{ id: 'a', activity: 'office', qty: 300, label: 'mine', cyc: 1, notes: 'x', over: 'no' }]],
    ['null row', [null, { id: 'a', activity: 'office', qty: 100 }]],
  ];
  for (const [name, rows] of cases) {
    await add(PK, `parking ${name}`, 'parking', 'corridor', rowsSet(rows), P => X.parking(P));
    await add(PK, `parking ${name} grid`, 'parking', 'grid', rowsSet(rows), P => X.parking(P));
  }
  // every activity at several sizes, with and without car parking
  for (const a of acts) for (const q of [0, 90, 450, 3200, 12000, 95000]) for (const cars of [0, 25]) {
    await add(PK, `parking ${a} ${q} m2 ${cars} cars`, 'parking', 'corridor', rowsSet([{ id: 'a', activity: a, qty: q, cars, maxCars: 10, listedZone: cars === 0 }]), P => X.parking(P));
  }
  // the induced trips setting and the user's assumptions
  for (const v of [0.5, 0.8, 1.25, '0.3', 'x', '', null, 0]) {
    await add(PK, `parking trips per space ${JSON.stringify(v)}`, 'parking', 'corridor', { ...rowsSet([{ id: 'a', activity: 'office', qty: 1000, maxCars: 10, cars: 30 }]), 'settings.complianceExtra.tripsPerSpace': v }, P => X.parking(P));
  }
  // CSV, report section and export
  const csvRows = cases.filter(([n]) => ['no rows', 'office over the maximum', 'office big', 'residential garages', 'retail no rate entered', 'childcare listed zone no parking', 'mixed development', 'totals met only in total', 'entered as text', 'non-numeric entries', 'blank max'].includes(n));
  for (const [name, rows] of csvRows) {
    await add(PK, `parkingCSV ${name}`, 'parkingCSV', 'corridor', rowsSet(rows), P => X.parkingCSV(P));
    await add(PK, `parking report ${name}`, 'report', 'corridor', rowsSet(rows), P => report('compliance-extra-parking')(P), { report: 'compliance-extra-parking', ctx: false });
    await add(PK, `parking report ctx ${name}`, 'report', 'corridor', rowsSet(rows), P => report('compliance-extra-parking')(P, mockCtx), { report: 'compliance-extra-parking', ctx: true });
    await add(PK, `parking export ${name}`, 'export', 'corridor', rowsSet(rows), P => ({ build: exp('parking').build(P), enabled: exp('parking').enabled(), label: exp('parking').label }), { export: 'compliance-extra-parking' });
  }
  /* ====================================================================================
     12. Urban subdivision
     ==================================================================================== */
  const SD = 'subdivision';
  const sub = (id, fixture, sets) => add(SD, id, 'subdivision', fixture, sets, P => X.subdivision(P));
  for (const [n, extra] of [['corridor', {}], ['grid', {}], ['corridor', { active: 's-opt' }], ['grid', { base: 's-dm', active: 's-opt' }]]) {
    for (const lots of [0, 25, 40, 41, 99.5, 100, 101, 400, '60', 'x', null, -3, '']) {
      await sub(`subdivision ${n} ${JSON.stringify(extra)} lots ${JSON.stringify(lots)}`, n, { ...extra, 'complianceExtra.subdivision.lots': lots });
    }
  }
  for (const [n, extra] of [['corridor', { active: 's-opt' }], ['grid', { base: 's-dm', active: 's-opt' }]]) {
    for (const v of [0, 1, 5, 50, 500, '3', 'x', '', null]) await sub(`subdivision ${n} affectedPct ${JSON.stringify(v)}`, n, { ...extra, 'complianceExtra.subdivision.lots': 150, 'settings.complianceExtra.affectedPct': v });
    for (const v of [0.8, 1.33, 2, '1.2', 'x', 0]) await sub(`subdivision ${n} walkSpeed ${JSON.stringify(v)}`, n, { ...extra, 'complianceExtra.subdivision.lots': 150, 'settings.complianceExtra.walkSpeed': v });
  }
  // development zones, stops and schools
  await sub('subdivision corridor no development zone', 'corridor', { 'scenarios.0.zones.zDev.role': 'existing', 'complianceExtra.subdivision.lots': 80 });
  await sub('subdivision corridor no zones at all', 'corridor', { 'scenarios.0.zones': {}, 'complianceExtra.subdivision.lots': 80 });
  await sub('subdivision corridor no routes no stops', 'corridor', { 'scenarios.0.routes': {} });
  await sub('subdivision corridor stop marked', 'corridor', { 'scenarios.0.routes': {}, 'scenarios.0.nodes.nidc05pnh.stop': true });
  await sub('subdivision corridor stop and routes', 'corridor', { 'scenarios.0.nodes.nidc0qw7h.stop': true, 'scenarios.0.nodes.nidc0szce.stop': true });
  await sub('subdivision corridor routes wp not nodes', 'corridor', { 'scenarios.0.routes.r1.wp': ['nope', 'nidc0aav9'] });
  await sub('subdivision corridor routes no wp', 'corridor', { 'scenarios.0.routes.r1.wp': null });
  await sub('subdivision corridor no schools', 'corridor', { 'scenarios.0.zones.zEx.lu.school': 0 });
  await sub('subdivision corridor school in the development', 'corridor', { 'scenarios.0.zones.zDev.lu.school': 300 });
  await sub('subdivision corridor school and gateway', 'corridor', { 'scenarios.0.zones.gW.lu': { school: 100 } });
  await sub('subdivision corridor far zone', 'corridor', { 'scenarios.0.zones.zCom.lat': -37.2 });
  await sub('subdivision corridor near zone', 'corridor', { 'scenarios.0.zones.zCom.lat': -37.0966, 'scenarios.0.zones.zCom.lng': 174.9585 });
  await sub('subdivision grid school', 'grid', { 'scenarios.0.zones.zB.lu.school': 200 });
  await sub('subdivision grid two schools', 'grid', { 'scenarios.0.zones.zB.lu.school': 200, 'scenarios.0.zones.zC.lu.school': 50 });
  await sub('subdivision grid no routes', 'grid', { 'scenarios.0.routes': {} });
  await sub('subdivision grid one development zone', 'grid', { 'scenarios.0.zones.zA.existing': true, 'scenarios.0.zones.zB.existing': true });
  await sub('subdivision grid no results', 'grid', { 'scenarios.0.res': {}, 'scenarios.1.res': {}, 'scenarios.2.res': {}, active: 's-opt', base: 's-dm' });
  await sub('subdivision grid opt AM only', 'grid', { base: 's-dm', active: 's-opt', 'scenarios.2.res.PM': '$DELETE' });
  await sub('subdivision grid base PM only', 'grid', { base: 's-dm', active: 's-opt', 'scenarios.1.res.AM': '$DELETE' });
  // links that gain traffic: a proposed road is left out, flows made up
  await sub('subdivision corridor flows up', 'corridor', { active: 's-opt', 'scenarios.0.links.lidc0hcs9.status': 'proposed', 'complianceExtra.subdivision.lots': 50 });
  // rear accesses
  const acc = (name, a) => a;
  const accessSets = [
    [{ name: 'A', sites: 1, legal: 3, formed: 2.5, len: 30 }],
    [{ name: 'B', sites: 1, legal: 2.5, formed: 2, len: 60 }],
    [{ name: 'C', sites: 3, legal: 3.4, formed: 2.9, len: 50 }, { name: 'D', sites: 5, legal: 3.5, formed: 3, len: 51 }],
    [{ name: 'E', sites: 6, legal: 6.9, formed: 5.5, len: 100, ped: 1.4 }, { name: 'F', sites: 6, legal: 6.5, formed: 5, len: 101 }],
    [{ name: 'G', sites: 10, legal: 7, formed: 6, len: 100, ped: 1.0 }, { name: 'H', sites: 11, legal: 7, formed: 6, len: 100, ped: 2 }],
    [{ name: '', sites: 0 }, { sites: '2', formed: '2.9', legal: '3.2', len: '40' }, { name: 'text', sites: 'x', formed: 'y' }],
    [{ name: 'I', sites: 2.5, formed: null, legal: '', len: null, ped: null }, { name: 'J', sites: -1, formed: 1, legal: 1, len: 1000 }],
    [{ name: 'K', sites: 8, formed: 5.4, ped: 1.39 }, { name: 'L', sites: 7, formed: 5.5, ped: 1.4 }],
  ];
  for (const [i, a] of accessSets.entries()) {
    await sub(`subdivision accesses ${i}`, 'corridor', { 'complianceExtra.subdivision.accesses': a, 'complianceExtra.subdivision.lots': i * 30 });
    await sub(`subdivision accesses ${i} only`, 'grid', { 'complianceExtra.subdivision.accesses': a });
  }
  for (const sites of [0, 1, 2, 3, 4, 5, 6, 7, 10, 11, 20, 2.5, '4', 'x', null, -2]) for (const o of [{}, { formed: 2, legal: 2, len: 200 }, { formed: 3.2, legal: 3.4, len: 50, ped: 1.5 }, { formed: 5.5, legal: 6.9, len: 100, ped: 1.4 }, { formed: 5.4, legal: 6.8, len: 101, ped: 1 }, { formed: '', legal: 'x', len: '', ped: 'y' }]) {
    await add(SD, `rearAccess sites ${JSON.stringify(sites)} ${JSON.stringify(o)}`, 'rearAccess', 'corridor', {}, () => X.rearAccess({ sites, ...o }), { args: { a: { sites, ...o } } });
  }
  for (const [id, sets] of [
    ['lots', { 'complianceExtra.subdivision.lots': 120, 'complianceExtra.subdivision.accesses': accessSets[2] }],
    ['no lots, an access', { 'complianceExtra.subdivision.accesses': accessSets[0] }],
    ['no lots, no accesses', {}],
    ['lots no accesses', { 'complianceExtra.subdivision.lots': 20 }],
  ]) {
    await add(SD, `subdivision report ${id}`, 'report', 'corridor', sets, P => report('compliance-extra-subdivision')(P), { report: 'compliance-extra-subdivision', ctx: false });
    await add(SD, `subdivision report ctx ${id}`, 'report', 'grid', sets, P => report('compliance-extra-subdivision')(P, mockCtx), { report: 'compliance-extra-subdivision', ctx: true });
  }
  /* ====================================================================================
     13. Zone-to-zone times; 14. Policy alignment
     ==================================================================================== */
  const PO = 'policy';
  // Projects the engine has to run again because the network changes, kept as fixtures
  const makeFixture = async (name, base, mutate, after) => {
    const P = await withResults(c, base, { rerun: true, mutate });
    if (after) after(P);
    saveFixture(name, P);
    return name;
  };
  const OPT = 's-opt';
  // corridor option: an extra lane, a faster collector, a bus lane, Future Connect and RASF data, slower arcs on a rapid transit road
  const fxCorr = await makeFixture('f3-corr-opt', 'corridor', P => {
    const o = P.scenarios[1];
    o.links.lidc0dlsv.lanesAB = 3;
    o.links.lidc0lb8r.speed = 70;
    o.links.lidc0ngt6.busLane = true;
    o.links.lidc0dlsv.fc = { gt: 'Local road', pt: 'Rapid transit', freight: 'Level 1A', cycle: 'Regional cycleway' };
    o.links.lidc0lb8r.fc = { gt: 'Collector', pt: 'Frequent Transit Network', freight: 'Level 2', cycle: 'Local' };
    o.links.lidc0lb8r.cx = { rasf: 'P3 M2 (advisory)', limit: 70 };
    o.links.lidc0eysc.fc = { gt: 'Arterial', pt: 'Rapid', freight: '', cycle: 'Connector' };
    o.links.lidc0eysc.cx = { place: 2, onf: 'Primary' };
    o.links.lidc0ngt6.fc = { pt: 'Local' };
  }, P => {
    P.active = OPT;
    P.scenarios[1].res.AM.arcs['lidc0eysc:ab'].t = P.scenarios[0].res.AM.arcs['lidc0eysc:ab'].t * 1.3;
  });
  // grid option: a wider link, signals and a new link, with the base the do-minimum
  const fxGrid = await makeFixture('f3-grid-opt', 'grid', P => {
    const o = P.scenarios[2];
    o.links.lidc0sold.lanesAB = 3;
    o.links.lidc0sold.fc = { gt: 'Local', pt: 'Frequent', cycle: 'Local' };
    o.links.lidc01h5wz.fc = { gt: 'Collector road', pt: 'Peak only', freight: 'Level 2' };
    o.links.lidc01h5wz.cx = { rasf: 'place P2' };
    o.nodes.nidc0krpm.control = 'signals';
  }, P => { P.base = 's-dm'; P.active = OPT; });
  // names the sets refer to, per network: links, nodes, scenario positions (option and do-minimum), zones
  const K = {
    corridor: { A: 'lidc0lb8r', B: 'lidc0dlsv', C: 'lidc0hcs9', D: 'lidc0tv6u', E: 'lidc0eysc', N1: 'nidc0942h', N2: 'nidc0qw7h', OPT: 1, BASE: 0, dz: 'zDev', z1: 'zCom', z2: 'zEx' },
    grid: { A: 'lidc0sold', B: 'lidc0tzws', C: 'lidc01efq0', D: 'lidc0r4yn', E: 'lidc0unhw', N1: 'nidc0fa0z', N2: 'nidc0krpm', OPT: 2, BASE: 1, dz: 'zA', z1: 'zB', z2: 'zC' },
  };
  // a set with placeholders ($A links, $N1 nodes, $OPT $BASE scenario positions, $dz $z1 $z2 zones, $SCID the option's id) made specific to a network
  const subst = (t, n) => t.replace(/\$(\w+)/g, (m, w) => (w === 'SCID' ? OPT : w in K[n] ? K[n][w] : m));
  const specific = (sets, n) => Object.fromEntries(Object.entries(sets).map(([k, v]) => [subst(k, n), JSON.parse(subst(JSON.stringify(v ?? null), n))]));
  const worlds = [['corridor', 'corridor', {}], ['corridor', 'corridor', { active: OPT }], ['grid', 'grid', { base: 's-dm', active: OPT }], [fxCorr, 'corridor', {}], [fxGrid, 'grid', {}]];

  // skim: shortest times between zones on a result
  for (const [fx, n, sci] of [['corridor', 'corridor', 0], ['corridor', 'corridor', 1], ['grid', 'grid', 0], ['grid', 'grid', 2], [fxCorr, 'corridor', 1], [fxGrid, 'grid', 2]]) {
    for (const per of ['AM', 'PM', null]) {
      for (const [lab, from, to] of [['all', null, null], ['some', ['zDev', 'zCom', 'nope'], ['zEx', 'zDev', 'zZ']], ['one', ['zA', 'zDev'], ['zB', 'zC', 'zEx']], ['none', [], ['zEx']], ['gateways', ['gW', 'gE'], ['gW', 'gE', 'zEx', 'zA']]]) {
        await add(PO, `skim ${fx} ${sci} ${per} ${lab}`, 'skim', fx, {}, P => {
          const sc = P.scenarios[sci], ids = Object.keys(sc.zones);
          return X.skim(sc, per ? sc.res[per] : null, from || ids, to || ids);
        }, { args: { sci, per, from, to } });
      }
    }
  }
  for (const [lab, z] of [['none', {}], ['jobs', { jobs: { n: 120 } }], ['jobs zero', { jobs: { n: 0 }, employees: 30 }], ['jobs text', { jobs: { n: '45' } }], ['employees', { employees: 80 }], ['employees text', { employees: '7' }],
    ['jobs nan', { jobs: { n: 'x' }, employees: 5 }], ['jobs empty', { jobs: {} }], ['jobs null', { jobs: null, employees: 9 }], ['both zero', { jobs: { n: 0 }, employees: 0 }], ['negative', { employees: -4 }]]) {
    await add(PO, `jobsOf ${lab}`, 'jobsOf', 'corridor', {}, () => F.jobsOf(z), { args: { z } });
  }
  for (const [lab, L] of [['none', {}], ['cx empty', { cx: {} }], ['place 2', { cx: { place: 2 } }], ['place text', { cx: { place: '3' } }], ['place zero', { cx: { place: 0 } }], ['place x', { cx: { place: 'x' } }],
    ['rasf P3', { cx: { rasf: 'P3 M2' } }], ['rasf p 1', { cx: { rasf: 'place p1' } }], ['rasf P 2', { cx: { rasf: 'P 2' } }], ['rasf P4', { cx: { rasf: 'P4' } }], ['rasf none', { cx: { rasf: 'M2' } }],
    ['rasf number', { cx: { rasf: 3 } }], ['place wins', { cx: { place: 1, rasf: 'P3' } }], ['place blank', { cx: { place: '', rasf: 'P2' } }], ['place null', { cx: { place: null, rasf: 'P2' } }], ['rasf first', { cx: { rasf: 'P1 then P3' } }]]) {
    await add(PO, `placeOf ${lab}`, 'placeOf', 'corridor', {}, () => F.placeOf(L), { args: { L } });
  }
  // the links a proposal affects
  const affSets = [
    ['saved', {}],
    ['pct 1', { 'settings.complianceExtra.affectedPct': 1 }],
    ['pct 60', { 'settings.complianceExtra.affectedPct': 60 }],
    ['pct text', { 'settings.complianceExtra.affectedPct': 'x' }],
    ['extent strings', { 'scenarios.$OPT.extent': { links: ['$A:ab', '$B', '$A:ba'] } }],
    ['extent objects', { 'scenarios.$OPT.extent': { links: [{ link: '$E' }, { id: '$D' }, { link: '$E' }, {}] } }],
    ['extent empty', { 'scenarios.$OPT.extent': { links: [] } }],
    ['extent sites only', { 'scenarios.$OPT.extent': { sites: ['$N1'] } }],
    ['extent not a list', { 'scenarios.$OPT.extent': { links: 'abc' } }],
    ['changed road', { 'scenarios.$OPT.links.$C.lanesAB': 3, 'scenarios.$OPT.links.$D.speed': 30 }],
    ['changed node', { 'scenarios.$OPT.nodes.$N1.control': 'signals' }],
    ['upgraded node', { 'scenarios.$OPT.nodes.$N2.upgrade': 'signals' }],
    ['no base results', { 'scenarios.$BASE.res': {} }],
    ['no option results', { 'scenarios.$OPT.res': {} }],
    ['AM only', { 'scenarios.$OPT.res.PM': '$DELETE' }],
    ['base without flow', { 'scenarios.$BASE.res.AM.arcs.$A:ab.v': 0 }],
    ['base arc missing', { 'scenarios.$BASE.res.AM.arcs.$A:ab': '$DELETE' }],
  ];
  for (const [fx, n, extra] of worlds.slice(1)) {
    for (const [lab, sets] of affSets) await add(PO, `affectedLinks ${fx} ${JSON.stringify(extra)} ${lab}`, 'affectedLinks', fx, { ...extra, ...specific(sets, n) }, P => F.affectedLinks(P));
  }
  await add(PO, 'affectedLinks corridor same scenario', 'affectedLinks', 'corridor', {}, P => F.affectedLinks(P));
  await add(PO, 'affectedLinks grid same scenario', 'affectedLinks', 'grid', {}, P => F.affectedLinks(P));
  // accessibility
  const accSets = [
    ['saved', {}],
    ['jobs', { 'scenarios.$OPT.zones.$z1.jobs': { n: 300 }, 'scenarios.$OPT.zones.$z2.employees': 50 }],
    ['catch 5', { 'settings.complianceExtra.catchMin': 5 }],
    ['catch 2', { 'settings.complianceExtra.catchMin': 2 }],
    ['catch 90', { 'settings.complianceExtra.catchMin': 90, 'settings.complianceExtra.cycleSpeed': 25 }],
    ['walk 0.5', { 'settings.complianceExtra.walkSpeed': 0.5, 'settings.complianceExtra.catchMin': 10 }],
    ['no results', { 'scenarios.$OPT.res': {} }],
    ['PM only', { 'scenarios.$OPT.res.AM': '$DELETE' }],
    ['no dev zone', { 'scenarios.$OPT.zones.$dz.role': 'existing' }],
    ['services', { 'scenarios.$OPT.zones.$z2.lu.medical': 40, 'scenarios.$OPT.zones.$z1.lu.supermarket': 800 }],
    ['school', { 'scenarios.$OPT.zones.$z1.lu.school': 40 }],
  ];
  for (const [fx, n, extra] of worlds) for (const [lab, sets] of accSets) await add(PO, `accessibility ${fx} ${JSON.stringify(extra)} ${lab}`, 'accessibility', fx, { ...extra, ...specific(sets, n) }, P => X.accessibility(P));
  // policy alignment
  const polSets = [
    ['saved', {}],
    ['npsi document', { 'complianceExtra.policy.npsiDoc': 'Auckland Plan 2050 (strategic planning document)' }],
    ['no results', { 'scenarios.$OPT.res': {} }],
    ['AM only', { 'scenarios.$OPT.res.PM': '$DELETE', 'scenarios.$BASE.res.PM': '$DELETE' }],
    ['speed up setting', { 'settings.complianceExtra.speedUp': 0.0001 }],
    ['speed up high', { 'settings.complianceExtra.speedUp': 1000 }],
    ['hazards', { 'feeds.hazards.$SCID.rows': [{ link: '$A', name: 'Road A', fc: 'Collector', len: 120, layer: 'Flood plain' }, { link: '$A', name: 'Road A', fc: 'Collector', len: 40, layer: 'Coastal', only: true }] }],
    ['hazards empty', { 'feeds.hazards.$SCID.rows': [] }],
    ['rltp', { 'complianceExtra.rltp.objective': 'Improve access', 'complianceExtra.policy.rltp': 'Drury RLTP project' }],
    ['limit raised', { 'scenarios.$OPT.links.$E.cx': { limit: 80, place: 3 } }],
    ['lane added arterial fc', { 'scenarios.$OPT.links.$A.lanesAB': 2, 'scenarios.$OPT.links.$A.fc': { gt: 'Arterial' } }],
    ['lane added local fc', { 'scenarios.$OPT.links.$A.lanesAB': 2, 'scenarios.$OPT.links.$A.fc': { gt: 'Local road' } }],
    ['bus priority', { 'scenarios.$OPT.links.$E.busLane': true }],
    ['arc slow', { 'scenarios.$OPT.res.PM.arcs.$A:ab.t': 99, 'scenarios.$OPT.links.$A.fc': { pt: 'Strategic transit' } }],
    ['arc slow irtn', { 'scenarios.$OPT.res.PM.arcs.$A:ab.t': 99, 'scenarios.$OPT.links.$A.fc': { pt: 'IRTN' } }],
    ['arc slow local', { 'scenarios.$OPT.res.PM.arcs.$A:ab.t': 99, 'scenarios.$OPT.links.$A.fc': { pt: 'Local bus' } }],
    ['year mismatch', { 'scenarios.$OPT.res.AM.year': 2040 }],
    ['no development', { 'scenarios.$OPT.zones.$dz.role': 'existing' }],
  ];
  for (const [fx, n, extra] of [['corridor', 'corridor', { active: OPT }], [fxCorr, 'corridor', {}], ['grid', 'grid', { base: 's-dm', active: OPT }], [fxGrid, 'grid', {}]]) {
    for (const [lab, sets] of polSets) await add(PO, `policy ${fx} ${JSON.stringify(extra)} ${lab}`, 'policy', fx, { ...extra, ...specific(sets, n) }, P => X.policy(P));
  }
  await add(PO, 'policy corridor same scenario', 'policy', 'corridor', {}, P => X.policy(P));
  await add(PO, 'policy grid same scenario', 'policy', 'grid', {}, P => X.policy(P));
  // the CSV, the report section and the export
  for (const [fx, extra] of [[fxCorr, {}], ['corridor', { active: OPT }], [fxGrid, {}], ['grid', {}], ['corridor', {}]]) {
    for (const [lab, sets] of [['saved', {}], ['no results', { 'scenarios.0.res': {}, 'scenarios.1.res': {} }], ['levy and polygon', { 'complianceExtra.funding.levy': 0.5, 'complianceExtra.funding.polygon': [[-37.09, 174.94], [-37.09, 174.96], [-37.11, 174.96], [-37.11, 174.94]] }]]) {
      const s = { ...extra, ...sets };
      await add(PO, `policyCSV ${fx} ${JSON.stringify(extra)} ${lab}`, 'policyCSV', fx, s, P => X.policyCSV(P));
      await add(PO, `policy report ${fx} ${JSON.stringify(extra)} ${lab}`, 'report', fx, s, P => report('compliance-extra-policy')(P), { report: 'compliance-extra-policy', ctx: false });
      await add(PO, `policy report ctx ${fx} ${JSON.stringify(extra)} ${lab}`, 'report', fx, s, P => report('compliance-extra-policy')(P, mockCtx), { report: 'compliance-extra-policy', ctx: true });
      await add(PO, `policy export ${fx} ${JSON.stringify(extra)} ${lab}`, 'export', fx, s, P => ({ build: exp('policy').build(P), enabled: exp('policy').enabled(), label: exp('policy').label }), { export: 'compliance-extra-policy' });
    }
  }
  /* ====================================================================================
     15. IPM stage 2 GPS metrics; 16. Funding attribution
     ==================================================================================== */
  const GP = 'gps';
  for (const s of [-5, 0, 3, 5, 5.01, 10, 10.1, 30, 30.5, 100]) {
    await add(GP, `gtRating ${s}`, 'gtRating', 'corridor', {}, () => X.gtRating(s), { args: { s } });
    for (const sig of ['road', 'regional', 'national', 'x', '']) await add(GP, `freightRating ${s} ${sig}`, 'freightRating', 'corridor', {}, () => X.freightRating(s, sig), { args: { s, sig } });
  }
  for (const [lab, names] of [['one', 'Main Road'], ['case', 'main ROAD'], ['two', 'Main Road; Side 2N'], ['commas', 'side 2n, Side 4N,\nSide 6N'], ['blank', ' , ;; '], ['empty', ''], ['null', null], ['number', 5],
    ['unknown', 'Nowhere Road'], ['trim', '   Side 2S   '], ['row', 'Row 1 0, Row 1 1'], ['array', ['Main Road']]]) {
    for (const n of ['corridor', 'grid']) await add(GP, `roadsToLinks ${n} ${lab}`, 'roadsToLinks', n, {}, P => F.roadsToLinks(P.scenarios[0], names), { args: { sci: 0, names } });
  }
  await add(GP, 'roadsToLinks closed road', 'roadsToLinks', 'corridor', { 'scenarios.0.links.lidc0lb8r.status': 'closed' }, P => F.roadsToLinks(P.scenarios[0], 'Side 2N, Side 4N'), { args: { sci: 0, names: 'Side 2N, Side 4N' } });
  // the point in polygon test
  const polys = {
    square: [[0, 0], [0, 10], [10, 10], [10, 0]], triangle: [[0, 0], [10, 5], [0, 10]], concave: [[0, 0], [0, 10], [5, 5], [10, 10], [10, 0]], text: [['0', '0'], ['0', '10'], ['10', '10'], ['10', '0']],
    flat: [[0, 0], [0, 10]], empty: [], one: [[3, 3]], same: [[2, 2], [2, 2], [2, 2]], horizontal: [[5, 0], [5, 10], [5, 5]],
  };
  for (const [pn, poly] of Object.entries(polys)) for (const pt of [[5, 5], [0, 0], [10, 10], [-1, 5], [5, 11], [5, 0], [5, 10], [2.5, 7.5], [9.99, 0.01], [7.5, 5], [5, 7.5], [1e9, 1e9]]) {
    await add(GP, `inPoly ${pn} ${pt}`, 'inPoly', 'corridor', {}, () => X.inPoly(pt, poly), { args: { pt, poly } });
  }
  // routes
  const routeSets = [
    ['default', {}],
    ['named roads', { 'complianceExtra.gps.routes': [{ name: 'Main', roads: 'Main Road', significance: 'regional' }] }],
    ['road names only', { 'complianceExtra.gps.routes': [{ roads: 'Side 2N; Side 4N' }] }],
    ['by id', { 'complianceExtra.gps.routes': [{ name: 'By id', links: ['$A', '$B'], significance: 'national' }] }],
    ['empty links use names', { 'complianceExtra.gps.routes': [{ name: 'Names', links: [], roads: 'Main Road, Side 2N' }] }],
    ['blank routes dropped', { 'complianceExtra.gps.routes': [{}, null, { name: '' }, { links: [] }, { name: 'Kept', links: ['$C'] }] }],
    ['only blank routes', { 'complianceExtra.gps.routes': [{}, null, { name: '' }] }],
    ['null list', { 'complianceExtra.gps.routes': null }],
    ['links not a list', { 'complianceExtra.gps.routes': [{ name: 'T', links: 'abc', roads: 'Main Road' }] }],
    ['no arterial', { 'scenarios.$OPT.links.$A.cls': 'local', 'scenarios.$OPT.links.$B.cls': 'local', 'scenarios.$OPT.links.$C.cls': 'local', 'scenarios.$OPT.links.$D.cls': 'local', 'scenarios.$OPT.links.$E.cls': 'local' }],
    ['motorway', { 'scenarios.$OPT.links.$A.cls': 'motorway', 'scenarios.$OPT.links.$B.status': 'closed' }],
  ];
  for (const [fx, n, extra] of worlds) for (const [lab, sets] of routeSets) await add(GP, `gpsRoutes ${fx} ${JSON.stringify(extra)} ${lab}`, 'gpsRoutes', fx, { ...extra, ...specific(sets, n) }, P => X.gpsRoutes(P));
  // the metrics
  const gpsSets = [
    ['saved', {}],
    ['regional route', { 'complianceExtra.gps.routes': [{ name: 'R1', links: ['$A', '$B', '$E'], significance: 'regional' }] }],
    ['national and regional', { 'complianceExtra.gps.routes': [{ name: 'R1', links: ['$A', '$B'], significance: 'regional' }, { name: 'R2', links: ['$C', '$E'], significance: 'national' }, { name: 'R3', roads: 'Main Road', significance: 'road' }] }],
    ['regional after national', { 'complianceExtra.gps.routes': [{ name: 'R2', links: ['$C'], significance: 'national' }, { name: 'R1', links: ['$A'], significance: 'regional' }] }],
    ['unknown significance', { 'complianceExtra.gps.routes': [{ name: 'R1', links: ['$A', '$B'], significance: 'local' }, { name: 'R2', links: ['$C'] }] }],
    ['route without valid links', { 'complianceExtra.gps.routes': [{ name: 'R1', links: ['nope'], significance: 'national' }, { name: 'R2', links: ['$A'], significance: 'regional' }] }],
    ['no valid links', { 'complianceExtra.gps.routes': [{ name: 'R1', links: ['nope'] }] }],
    ['roads only name', { 'complianceExtra.gps.routes': [{ roads: 'Main Road', significance: 'national' }] }],
    ['no results', { 'scenarios.$OPT.res': {} }],
    ['AM only', { 'scenarios.$OPT.res.PM': '$DELETE' }],
    ['destinations', { 'complianceExtra.gps.destinations': [{ zoneId: '$z1', significance: 'regional' }, { zoneId: '$z2' }, { zoneId: 'nope', significance: 'national' }, { zoneId: '$dz', significance: 'x' }] }],
    ['destination named', { 'complianceExtra.gps.destinations': [{ zoneId: '$z1', significance: 'national' }], 'scenarios.$OPT.zones.$z1.name': 'Town centre' }],
    ['destination no matrix', { 'complianceExtra.gps.destinations': [{ zoneId: '$z1' }], 'scenarios.$OPT.res.AM.T': null }],
    ['destination no zone ids', { 'complianceExtra.gps.destinations': [{ zoneId: '$z1' }], 'scenarios.$OPT.res.AM.zoneIds': [] }],
    ['stages', { 'complianceExtra.gps.stages': [{ name: 'S1', dwellings: 100, year75: 2030 }, { name: 'S2', dwellings: '50', year75: '2036' }, { name: 'S3', dwellings: 20 }, { name: 'S4', dwellings: 30, year75: 2035 }, { name: 'S5', dwellings: 'x', year75: 2020 }] }],
    ['dsi', { 'complianceExtra.gps.dsiReduction': 4, 'complianceExtra.gps.cost': 80, 'complianceExtra.gps.collective': 'Collective risk falls' }],
    ['dsi entered no cost', { 'complianceExtra.gps.dsiReduction': 4 }],
    ['dsi zero cost', { 'complianceExtra.gps.dsiReduction': 4, 'complianceExtra.gps.cost': 0 }],
    ['dsi text', { 'complianceExtra.gps.dsiReduction': '2.5', 'complianceExtra.gps.cost': '50' }],
    ['dsi not a number', { 'complianceExtra.gps.dsiReduction': 'x', 'complianceExtra.gps.cost': 'y' }],
    ['dsi blank', { 'complianceExtra.gps.dsiReduction': '', 'complianceExtra.gps.cost': '' }],
    ['zone year text', { 'scenarios.$OPT.zones.$dz.year': '2031' }],
    ['zone year late', { 'scenarios.$OPT.zones.$dz.year': 2036 }],
    ['zone year none', { 'scenarios.$OPT.zones.$dz.year': null }],
    ['zone year zero', { 'scenarios.$OPT.zones.$dz.year': 0 }],
    ['no development', { 'scenarios.$OPT.zones.$dz.role': 'existing' }],
  ];
  for (const [fx, n, extra] of worlds) for (const [lab, sets] of gpsSets) await add(GP, `gpsMetrics ${fx} ${JSON.stringify(extra)} ${lab}`, 'gpsMetrics', fx, { ...extra, ...specific(sets, n) }, P => X.gpsMetrics(P));
  // growth share, funding assets
  const growSets = [
    ['saved', {}],
    ['employment entered', { 'complianceExtra.funding.employment': { existing: 100, future: 400 } }],
    ['employment text', { 'complianceExtra.funding.employment': { existing: '120', future: '300' } }],
    ['employment partly', { 'complianceExtra.funding.employment': { existing: 100, future: null } }],
    ['employment blank', { 'complianceExtra.funding.employment': { existing: '', future: 'x' } }],
    ['employment missing', { 'complianceExtra.funding.employment': null }],
    ['jobs in zones', { 'scenarios.$OPT.zones.$z1.jobs': { n: 200 }, 'scenarios.$OPT.zones.$z2.employees': 80 }],
    ['all existing', { 'scenarios.$OPT.zones.$dz.role': 'existing', 'scenarios.$OPT.zones.$z1.role': 'existing' }],
    ['no households', { 'scenarios.$OPT.zones.$dz.lu': {}, 'scenarios.$OPT.zones.$z1.lu': {}, 'scenarios.$OPT.zones.$z2.lu': {} }],
    ['build out', { 'scenarios.$OPT.zones.$dz.buildout': 50, 'scenarios.$OPT.buildout': 60 }],
  ];
  for (const [fx, n, extra] of worlds) for (const [lab, sets] of growSets) {
    await add(GP, `growthShare ${fx} ${JSON.stringify(extra)} ${lab}`, 'growthShare', fx, { ...extra, ...specific(sets, n) }, P => X.growthShare(P));
    await add(GP, `growthShare other scenario ${fx} ${JSON.stringify(extra)} ${lab}`, 'growthShare', fx, { ...extra, ...specific(sets, n) }, P => X.growthShare(P, P.scenarios[0]), { args: { sci: 0 } });
  }
  const assetSets = [
    ['saved', {}],
    ['chosen', { 'complianceExtra.funding.assets': ['$A', '$B', 'nope', '$A'] }],
    ['chosen none exist', { 'complianceExtra.funding.assets': ['nope'] }],
    ['road names', { 'complianceExtra.funding.assetRoads': 'Main Road' }],
    ['road names and chosen', { 'complianceExtra.funding.assets': ['$D'], 'complianceExtra.funding.assetRoads': 'Side 2N; Main Road' }],
    ['road names unknown', { 'complianceExtra.funding.assetRoads': 'Elm Street' }],
    ['changed roads', { 'scenarios.$OPT.links.$C.lanesAB': 3, 'scenarios.$OPT.nodes.$N1.control': 'signals' }],
  ];
  for (const [fx, n, extra] of worlds) for (const [lab, sets] of assetSets) await add(GP, `fundingAssets ${fx} ${JSON.stringify(extra)} ${lab}`, 'fundingAssets', fx, { ...extra, ...specific(sets, n) }, P => X.fundingAssets(P));
  // the funding attribution
  const poly = { corridor: [[-37.105, 174.945], [-37.105, 174.9618], [-37.095, 174.9618], [-37.095, 174.945]], grid: [[-36.899, 174.759], [-36.899, 174.7675], [-36.906, 174.7675], [-36.906, 174.759]] };
  const fundSets = [
    ['saved', {}],
    ['levy', { 'complianceExtra.funding.levy': 0.4 }],
    ['levy covers', { 'complianceExtra.funding.levy': 5000 }],
    ['levy text', { 'complianceExtra.funding.levy': '1.5' }],
    ['levy x', { 'complianceExtra.funding.levy': 'x' }],
    ['polygon', { 'complianceExtra.funding.polygon': '$POLY' }],
    ['polygon and levy', { 'complianceExtra.funding.polygon': '$POLY', 'complianceExtra.funding.levy': 0.2 }],
    ['polygon two points', { 'complianceExtra.funding.polygon': [[1, 2], [3, 4]] }],
    ['polygon with junk', { 'complianceExtra.funding.polygon': ['$POLY0', 5, [1], '$POLY1', null, '$POLY2', '$POLY3', [1, 2, 3]] }],
    ['polygon away', { 'complianceExtra.funding.polygon': [[10, 10], [10, 11], [11, 11]] }],
    ['polygon no matrix', { 'complianceExtra.funding.polygon': '$POLY', 'scenarios.$OPT.res.AM.T': null }],
    ['polygon no base matrix', { 'complianceExtra.funding.polygon': '$POLY', 'scenarios.$BASE.res.AM.T': null }],
    ['polygon PM only', { 'complianceExtra.funding.polygon': '$POLY', 'scenarios.$BASE.res.AM': '$DELETE' }],
    ['approval', { 'complianceExtra.funding.approval': '2026-11-02' }],
    ['approval with time', { 'complianceExtra.funding.approval': '2027-01-31T10:00:00' }],
    ['approval bad', { 'complianceExtra.funding.approval': 'soon' }],
    ['ftaa approval', { 'complianceExtra.ftaa.approval': '2026-12-15' }],
    ['both approvals', { 'complianceExtra.funding.approval': '2026-11-02', 'complianceExtra.ftaa.approval': '2026-12-15' }],
    ['no class flows', { 'scenarios.$OPT.res.AM.cls.arcs': {} }],
    ['class flows on some arcs', { 'scenarios.$OPT.res.AM.cls.arcs': { '$A:ab': [10, 20, 30], '$B:ba': [0, 5, 0], '$B:ab': [0, 0, 0], '$C:ab': [7, 0, 0] }, 'complianceExtra.funding.assets': ['$A', '$B', '$C'] }],
    ['no results', { 'scenarios.$OPT.res': {} }],
    ['assets chosen', { 'complianceExtra.funding.assets': ['$A', '$B'], 'complianceExtra.funding.polygon': '$POLY' }],
    ['employment', { 'complianceExtra.funding.employment': { existing: 80, future: 500 } }],
  ];
  for (const [fx, n, extra] of worlds) for (const [lab, sets] of fundSets) {
    const ss = specific(sets, n);
    for (const [k, v] of Object.entries(ss)) {
      if (v === '$POLY') ss[k] = poly[n];
      else if (Array.isArray(v)) ss[k] = v.map(x => (typeof x === 'string' && /^\$POLY\d$/.test(x) ? poly[n][+x[5]] : x));
    }
    await add(GP, `funding ${fx} ${JSON.stringify(extra)} ${lab}`, 'funding', fx, { ...extra, ...ss }, P => X.funding(P));
  }
  // select-link shares by development area: one assignment per zone and period (the engine runs here)
  const shareSets = [
    ['two assets', { 'complianceExtra.funding.assets': ['$A', '$B'] }],
    ['changed roads', {}],
    ['one period', { 'complianceExtra.funding.assets': ['$A', '$B', '$E'] }, { periods: ['AM'] }],
    ['PM and AM', { 'complianceExtra.funding.assets': ['$B'] }, { periods: ['PM', 'AM'] }],
    ['no save', { 'complianceExtra.funding.assets': ['$A'] }, { save: false }],
    ['gap and iterations', { 'complianceExtra.funding.assets': ['$A'] }, { periods: ['AM'], gapTol: 0.001, maxIter: 30 }],
    ['no periods', { 'complianceExtra.funding.assets': ['$A'] }, { periods: [] }],
    ['by road name', { 'complianceExtra.funding.assetRoads': 'Main Road' }, { periods: ['PM'] }],
    ['no dev zones', { 'complianceExtra.funding.assets': ['$A'], 'scenarios.$OPT.zones.$dz.role': 'existing' }],
    ['unrun period', { 'complianceExtra.funding.assets': ['$A'], 'scenarios.$OPT.res': {} }, { periods: ['AM'] }],
    ['all periods', { 'complianceExtra.funding.assets': ['$A'] }, { periods: ['AM', 'PM', 'IP'] }],
  ];
  for (const [fx, n, extra] of worlds.slice(1)) {
    for (const [lab, sets, opts = {}] of shareSets) {
      await add(GP, `fundingShares ${fx} ${JSON.stringify(extra)} ${lab}`, 'fundingShares', fx, { ...extra, ...specific(sets, n) }, async P => {
        const res = await X.fundingShares(P, opts);
        return { res, saved: P.complianceExtra.funding.shares ?? null };
      }, { opts });
    }
  }
  await add(GP, 'fundingShares corridor same scenario', 'fundingShares', 'corridor', {}, async P => ({ res: await X.fundingShares(P, {}), saved: P.complianceExtra.funding.shares ?? null }), { opts: {} });
  // funding with the select-link shares already in the project (as the run above stores them)
  for (const [fx, n] of [[fxCorr, 'corridor'], [fxGrid, 'grid']]) {
    const sets = { 'complianceExtra.funding.assets': n === 'corridor' ? ['lidc0dlsv', 'lidc0lb8r'] : ['lidc0sold', 'lidc01h5wz'] };
    const P = await variant(c, fx, sets);
    const res = await X.fundingShares(P, {});
    const name = saveFixture(`f3-${n}-shares`, P);
    for (const [lab, extra] of [['saved', {}], ['other scenario', { 'complianceExtra.funding.shares.scId': 'sidc01d0p' }], ['not ok', { 'complianceExtra.funding.shares.ok': false }], ['levy', { 'complianceExtra.funding.levy': 0.3, 'complianceExtra.funding.polygon': poly[n] }]]) {
      await add(GP, `funding with shares ${n} ${lab}`, 'funding', name, extra, Q => X.funding(Q));
      await add(GP, `policyCSV with shares ${n} ${lab}`, 'policyCSV', name, extra, Q => X.policyCSV(Q));
    }
    for (const [lab, extra] of [['dc', { 'compliance.ctx.pathway': 'dc-iff' }], ['dc approval', { 'compliance.ctx.pathway': 'dc-iff', 'complianceExtra.funding.approval': '2026-11-02', 'complianceExtra.funding.polygon': poly[n] }], ['not dc', { 'compliance.ctx.pathway': 'rc-landuse' }], ['no pathway', {}]]) {
      await add(GP, `funding report ${n} ${lab}`, 'report', name, extra, Q => report('compliance-extra-funding')(Q), { report: 'compliance-extra-funding', ctx: false });
    }
  }
  // the report sections of the GPS metrics and the funding attribution
  for (const [fx, n, extra] of worlds.slice(1)) {
    for (const [lab, sets] of [['fund', { 'compliance.ctx.pathway': 'nltf' }], ['fund routes', { 'compliance.ctx.pathway': 'nltf', 'complianceExtra.gps.routes': [{ name: 'R1', links: ['$A', '$B'], significance: 'regional' }], 'complianceExtra.gps.dsiReduction': 3, 'complianceExtra.gps.cost': 60, 'complianceExtra.gps.collective': 'Falls' }],
      ['not fund', { 'compliance.ctx.pathway': 'rc-landuse' }], ['no results', { 'compliance.ctx.pathway': 'nltf', 'scenarios.$OPT.res': {} }]]) {
      await add(GP, `gps report ${fx} ${JSON.stringify(extra)} ${lab}`, 'report', fx, { ...extra, ...specific(sets, n) }, P => report('compliance-extra-gps')(P), { report: 'compliance-extra-gps', ctx: false });
      await add(GP, `gps report ctx ${fx} ${JSON.stringify(extra)} ${lab}`, 'report', fx, { ...extra, ...specific(sets, n) }, P => report('compliance-extra-gps')(P, mockCtx), { report: 'compliance-extra-gps', ctx: true });
    }
    for (const [lab, sets] of [['dc', { 'compliance.ctx.pathway': 'dc-iff' }], ['dc polygon levy', { 'compliance.ctx.pathway': 'dc-iff', 'complianceExtra.funding.levy': 0.2, 'complianceExtra.funding.polygon': '$POLY', 'complianceExtra.funding.approval': '2026-11-02' }], ['not dc', { 'compliance.ctx.pathway': 'nltf' }]]) {
      const ss = specific(sets, n);
      for (const [k, v] of Object.entries(ss)) if (v === '$POLY') ss[k] = poly[n];
      await add(GP, `funding report ${fx} ${JSON.stringify(extra)} ${lab}`, 'report', fx, { ...extra, ...ss }, P => report('compliance-extra-funding')(P), { report: 'compliance-extra-funding', ctx: false });
      await add(GP, `funding report ctx ${fx} ${JSON.stringify(extra)} ${lab}`, 'report', fx, { ...extra, ...ss }, P => report('compliance-extra-funding')(P, mockCtx), { report: 'compliance-extra-funding', ctx: true });
    }
  }
  /* ====================================================================================
     17. Time-of-use screening
     ==================================================================================== */
  const TO = 'tou';
  const TD = '2026-12-01';
  // charging areas around the corridor (west of Side 4) and the grid (the west columns)
  const polyOf = { corridor: [[-37.105, 174.945], [-37.105, 174.9600], [-37.095, 174.9600], [-37.095, 174.945]], grid: [[-36.897, 174.758], [-36.897, 174.7675], [-36.906, 174.7675], [-36.906, 174.758]] };
  const polyAll = { corridor: [[-37.2, 174.9], [-37.2, 175], [-37, 175], [-37, 174.9]], grid: [[-37, 174.7], [-37, 174.8], [-36.8, 174.8], [-36.8, 174.7]] };
  const polyAway = [[-35, 170], [-35, 171], [-34, 171]];
  // touCharged
  for (const n of ['corridor', 'grid']) {
    for (const [lab, pl] of [['west', polyOf[n]], ['all', polyAll[n]], ['away', polyAway], ['two points', polyOf[n].slice(0, 2)], ['triangle', polyOf[n].slice(0, 3)]]) {
      for (const kind of ['cordon', 'area', 'x']) await add(TO, `touCharged ${n} ${lab} ${kind}`, 'touCharged', n, {}, P => X.touCharged(P.scenarios[0], pl, kind), { args: { poly: pl, kind, sci: 0 } });
    }
  }
  await add(TO, 'touCharged closed road', 'touCharged', 'corridor', { 'scenarios.0.links.lidc0fkhj.status': 'closed' }, P => X.touCharged(P.scenarios[0], polyOf.corridor, 'cordon'), { args: { poly: polyOf.corridor, kind: 'cordon', sci: 0 } });
  await add(TO, 'touCharged missing node', 'touCharged', 'corridor', { 'scenarios.0.links.lidc0fkhj.b': 'nowhere' }, P => X.touCharged(P.scenarios[0], polyOf.corridor, 'cordon'), { args: { poly: polyOf.corridor, kind: 'cordon', sci: 0 } });
  // touSkim: the times between zones with and without the charge
  for (const n of ['corridor', 'grid']) {
    for (const [lab, charged, applied, pen] of [['no charge', false, null, 0], ['charged', true, null, 120], ['applied', true, { 'lidc0lb8r:ab': 5, 'lidc0dlsv:ab': 12, 'lidc0r4yn:ab': 8 }, 90], ['applied only', false, { 'lidc0dlsv:ab': 12, 'lidc0sold:ab': 6, 'lidc0lb8r:ab': 4 }, 0]]) {
      for (const per of ['AM', 'PM']) {
        await add(TO, `touSkim ${n} ${lab} ${per}`, 'touSkim', n, {}, P => {
          const sc = P.scenarios[0], G = c.Engine.buildGraph(sc, P.settings), zs = Object.keys(sc.zones), r = sc.res[per];
          const ch = charged ? X.touCharged(sc, polyOf[n], 'cordon') : null;
          return X.touSkim(G, r, ch, applied ? new Map(Object.entries(applied)) : null, pen, zs, P.settings);
        }, { args: { sci: 0, per, poly: polyOf[n], charged, applied, pen } });
      }
    }
  }
  // touRun: the whole screening (the engine runs three times)
  const touCx = (n, o = {}) => ({ 'complianceExtra.tou.polygon': polyOf[n], ...Object.fromEntries(Object.entries(o).map(([k, v]) => ['complianceExtra.tou.' + k, v])) });
  const touRun = (P, opts) => X.touRun(P, opts).then(res => ({ res, saved: !!(P.complianceExtra.tou.result) }));
  const touRuns = [
    ['before commencement', {}, {}],
    ['day before commencement', {}, { today: '2026-11-17' }],
    ['commencement day', {}, { today: '2026-11-18' }],
    ['cordon', { kind: 'cordon' }, { today: TD }],
    ['area', { kind: 'area' }, { today: TD }],
    ['charges', { kind: 'cordon', light: 5, heavy: 12, cost: 40 }, { today: TD }],
    ['no charge', { kind: 'cordon', light: 0, heavy: 0 }, { today: TD }],
    ['charge text', { kind: 'cordon', light: '3.5', heavy: 'x', cost: '25' }, { today: TD }],
    ['PM period', { kind: 'cordon', period: 'PM' }, { today: TD }],
    ['period option', { kind: 'cordon', period: 'PM' }, { today: TD, period: 'AM' }],
    ['cost covered', { kind: 'cordon', light: 9, heavy: 20, cost: 0.001 }, { today: TD }],
    ['cost large', { kind: 'area', light: 1, heavy: 2, cost: 5000 }, { today: TD }],
    ['unknown kind', { kind: 'zone' }, { today: TD }],
    ['no save', { kind: 'cordon' }, { today: TD, save: false }],
  ];
  for (const n of ['corridor', 'grid']) {
    for (const [lab, o, opts] of touRuns) await add(TO, `touRun ${n} ${lab}`, 'touRun', n, touCx(n, o), P => touRun(P, opts), { opts });
    await add(TO, `touRun ${n} two points`, 'touRun', n, { 'complianceExtra.tou.polygon': polyOf[n].slice(0, 2) }, P => touRun(P, { today: TD }), { opts: { today: TD } });
    await add(TO, `touRun ${n} polygon junk`, 'touRun', n, { 'complianceExtra.tou.polygon': [[1, 2], 'x', [3], null, [4, 5]] }, P => touRun(P, { today: TD }), { opts: { today: TD } });
    await add(TO, `touRun ${n} nothing crosses`, 'touRun', n, { 'complianceExtra.tou.polygon': polyAway }, P => touRun(P, { today: TD }), { opts: { today: TD } });
    await add(TO, `touRun ${n} whole area`, 'touRun', n, { 'complianceExtra.tou.polygon': polyAll[n], 'complianceExtra.tou.kind': 'area' }, P => touRun(P, { today: TD }), { opts: { today: TD } });
  }
  // settings the screening reads
  const touSettings = [
    ['elasticity', { 'settings.complianceExtra.elasticity': -0.8 }],
    ['elasticity zero', { 'settings.complianceExtra.elasticity': 0 }],
    ['values', { 'settings.complianceExtra.vocPerKm': 0.5, 'settings.complianceExtra.co2PerT': 200, 'settings.complianceExtra.crashPerKm': 0.1 }],
    ['vot', { 'settings.econ.vot': 20 }],
    ['vot text', { 'settings.econ.vot': '45' }],
    ['vot zero', { 'settings.econ.vot': 0 }],
    ['hv share', { 'settings.hvPct': 20 }],
    ['hv none', { 'settings.hvPct': 0 }],
    ['period hours', { 'settings.econ.periodHours': { AM: 300, PM: 900, IP: 0 } }],
    ['period hours missing', { 'settings.econ.periodHours': {} }],
    ['period hours zero', { 'settings.econ.periodHours': { AM: 0, PM: 0 } }],
    ['rates flat', { 'settings.econ.rates': [{ to: 40, r: 3 }] }],
    ['rates three', { 'settings.econ.rates': [{ to: 10, r: 4 }, { to: 30, r: 2 }, { to: null, r: 1 }] }],
    ['rates empty', { 'settings.econ.rates': [] }],
    ['years 20', { 'settings.econ.years': 20 }],
    ['years text', { 'settings.econ.years': '25' }],
    ['years zero', { 'settings.econ.years': 0 }],
    ['income', { 'scenarios.0.zones.zDev.cx': { income: 85000 }, 'scenarios.0.zones.zEx.cx': { income: 62000 } }],
    ['sh links', { 'compliance.net.shUser.lidc0chmq': true, 'compliance.net.shUser.lidc0hcs9': true }],
    ['no results', { 'scenarios.0.res': {} }],
    ['PM only', { 'scenarios.0.res.AM': '$DELETE' }],
    ['no matrix', { 'scenarios.0.res.AM.T': null }],
    ['pathway', { 'compliance.ctx.pathway': 'tou' }],
  ];
  for (const [lab, sets] of touSettings) {
    await add(TO, `touRun corridor ${lab}`, 'touRun', 'corridor', { ...touCx('corridor', { kind: 'cordon', light: 3, heavy: 9, cost: 30 }), ...sets }, P => touRun(P, { today: TD }), { opts: { today: TD } });
  }
  // the saved screening: CSV, report section and export
  const touFx = {};
  for (const [name, n, o] of [['cordon', 'corridor', { kind: 'cordon', light: 3, heavy: 9, cost: 30 }], ['area', 'corridor', { kind: 'area', light: 2, heavy: 6, cost: 0.001 }], ['grid', 'grid', { kind: 'cordon', light: 4, heavy: 8, cost: 0 }]]) {
    const sets = { ...touCx(n, o), ...(n === 'corridor' ? { 'scenarios.0.zones.zDev.cx': { income: 85000 }, 'compliance.net.shUser.lidc0chmq': true } : {}) };
    const P = await variant(c, n, sets);
    await X.touRun(P, { today: TD });
    touFx[name] = saveFixture(`f3-tou-${name}`, P);
  }
  for (const [name, fx] of [...Object.entries(touFx), ['not run', 'corridor']]) {
    await add(TO, `touCSV ${name}`, 'touCSV', fx, {}, P => X.touCSV(P));
    await add(TO, `tou report ${name}`, 'report', fx, {}, P => report('compliance-extra-tou')(P), { report: 'compliance-extra-tou', ctx: false });
    await add(TO, `tou report ctx ${name}`, 'report', fx, {}, P => report('compliance-extra-tou')(P, mockCtx), { report: 'compliance-extra-tou', ctx: true });
    await add(TO, `tou export ${name}`, 'export', fx, {}, P => ({ build: exp('tou').build(P), enabled: exp('tou').enabled(), label: exp('tou').label }), { export: 'compliance-extra-tou' });
  }
  await add(TO, 'touCSV refused run', 'touCSV', 'corridor', { 'complianceExtra.tou.result': { ok: false, why: 'x' } }, P => X.touCSV(P));
  await add(TO, 'tou export refused run', 'export', 'corridor', { 'complianceExtra.tou.result': { ok: false, why: 'x' } }, P => ({ build: exp('tou').build(P), enabled: exp('tou').enabled(), label: exp('tou').label }), { export: 'compliance-extra-tou' });
  flush();

  /* ====================================================================================
     18. Climate hazards; 19. Hand-over packs
     ==================================================================================== */
  const PK2 = 'packs';
  const hz = [
    { link: 'lidc0lb8r', name: 'Side 2N', fc: 'Collector', len: 120.4, layer: 'Flood plain' },
    { link: 'lidc0dlsv', name: 'Main Road', fc: 'Arterial', len: 15, layer: 'Coastal inundation', only: true },
    { link: 'nope', name: 'Ghost road', fc: '', len: 3, layer: 'Flood plain' },
    { link: 'lidc0hcs9', name: 'Main Road', fc: 'Arterial', len: 0, layer: '' },
  ];
  for (const [fx, n, extra] of worlds) {
    for (const [lab, sets] of [['not loaded', {}], ['rows', { 'feeds.hazards.$SCID.rows': hz }], ['empty', { 'feeds.hazards.$SCID.rows': [] }],
      ['other scenario', { 'feeds.hazards.sidc01d0p.rows': hz }], ['proposed road', { 'feeds.hazards.$SCID.rows': hz, 'scenarios.$OPT.links.lidc0lb8r.status': 'proposed' }], ['rows not a list', { 'feeds.hazards.$SCID.rows': 'x' }],
      ['feed without rows', { 'feeds.hazards.$SCID': { at: 1 } }], ['vest in row', { 'feeds.hazards.$SCID.rows': [{ link: 'lidc0lb8r', name: 'N', vest: 'yes', only: false }] }]]) {
      if (n === 'grid' && /lidc0lb8r|lidc0dlsv|lidc0hcs9/.test(JSON.stringify(sets))) continue;
      const ss = { ...extra, ...specific(sets, n) };
      await add(PK2, `climate ${fx} ${JSON.stringify(extra)} ${lab}`, 'climate', fx, ss, P => X.climate(P));
      await add(PK2, `climate other scenario ${fx} ${JSON.stringify(extra)} ${lab}`, 'climate', fx, ss, P => X.climate(P, P.scenarios[0]), { args: { sci: 0 } });
      await add(PK2, `climateCSV ${fx} ${JSON.stringify(extra)} ${lab}`, 'climateCSV', fx, ss, P => X.climateCSV(P));
      await add(PK2, `climate report ${fx} ${JSON.stringify(extra)} ${lab}`, 'report', fx, ss, P => report('compliance-extra-climate')(P), { report: 'compliance-extra-climate', ctx: false });
      await add(PK2, `climate report ctx ${fx} ${JSON.stringify(extra)} ${lab}`, 'report', fx, ss, P => report('compliance-extra-climate')(P, mockCtx), { report: 'compliance-extra-climate', ctx: true });
      await add(PK2, `climate export ${fx} ${JSON.stringify(extra)} ${lab}`, 'export', fx, ss, P => ({ build: exp('climate').build(P), enabled: exp('climate').enabled(), label: exp('climate').label }), { export: 'compliance-extra-climate' });
    }
  }
  // helpers of the packs
  for (const v of [0, 1, 1.25, 1.26, 0.05, 1.005, 2.5, -1.25, 1234.5678, '3.14159', 'x', null, undefined, '', NaN, Infinity, 1e-7, 1e21, true, [], {}, '  7 ']) {
    const a = v === undefined ? null : typeof v === 'number' && !isFinite(v) ? String(v) : v;
    await add(PK2, `r1 ${String(v)}`, 'r1', 'corridor', {}, () => F.r1(v), { args: { v: a } });
    await add(PK2, `r3 ${String(v)}`, 'r3', 'corridor', {}, () => F.r3(v), { args: { v: a } });
  }
  for (const [n, extra] of [['corridor', {}], ['corridor', { 'scenarios.0.nodes.nidc05pnh.control': 'signals' }], ['corridor', { 'scenarios.0.nodes.nidc07wsv.control': 'none', 'scenarios.0.nodes.nidc0bhhv.control': 'none' }], ['grid', {}], ['grid', { 'scenarios.0.nodes.nidc0ajz2.control': 'none' }]]) {
    await add(PK2, `signalNodes ${n} ${JSON.stringify(extra)}`, 'signalNodes', n, extra, P => F.signalNodes(P.scenarios[0]), { args: { sci: 0 } });
  }
  const sections = [
    { title: 'One', head: ['A', 'B'], rows: [[1, 'x'], [null, 'y,z']] },
    { title: 'Two with note', note: 'Some note', head: ['A'], rows: [] },
    { title: 'Three empty text', head: ['A'], rows: [], empty: 'Nothing here' },
    { title: 'Four blank note', note: '', head: ['A', 'B', 'C'], rows: [['=1+1', '"quoted"', 'multi\nline']], empty: '' },
  ];
  for (const [lab, secs] of [['one', sections.slice(0, 1)], ['all', sections], ['empty ones', sections.slice(1, 4)], ['none', []]]) {
    for (const [tool, nm] of [['SIDRA', 'A project, with comma'], ['the tool', ''], ['x', null]]) {
      await add(PK2, `packCSV ${lab} ${tool} ${nm}`, 'packCSV', 'corridor', nm === null ? {} : { name: nm }, P => F.packCSV(P, 'A title', tool, secs), { args: { sections: secs, tool } });
    }
  }
  // the design-life run
  const lifeOpts = [['default', {}], ['pct 20', { pct: 20 }], ['pct text', { pct: '15' }], ['AM only', { periods: ['AM'] }], ['PM AM', { periods: ['PM', 'AM'] }], ['no save', { pct: 5, save: false }], ['pct zero', { pct: 0 }], ['pct negative', { pct: -10 }]];
  for (const [fx, extra] of [['corridor', { active: OPT }], ['grid', { base: 's-dm', active: OPT }], [fxCorr, {}]]) {
    for (const [lab, opts] of lifeOpts) {
      await add(PK2, `runDesignLife ${fx} ${JSON.stringify(extra)} ${lab}`, 'runDesignLife', fx, extra, async P => ({ res: await X.runDesignLife(P, opts), saved: P.complianceExtra.tsdg.result ?? null }), { opts });
    }
    await add(PK2, `runDesignLife ${fx} ${JSON.stringify(extra)} cx entries`, 'runDesignLife', fx, { ...extra, 'complianceExtra.tsdg.pct': 25, 'complianceExtra.tsdg.years': 20 }, async P => ({ res: await X.runDesignLife(P, {}), saved: P.complianceExtra.tsdg.result ?? null }), { opts: {} });
  }
  // a project with a saved design-life run
  const lifeFx = {};
  for (const [name, fx, extra] of [['corridor', 'corridor', { active: OPT }], ['grid', 'grid', { base: 's-dm', active: OPT }]]) {
    const P = await variant(c, fx, extra);
    await X.runDesignLife(P, { pct: 15 });
    lifeFx[name] = saveFixture(`f3-life-${name}`, P);
  }
  const countsC = [{ id: 'c1', src: 'user', name: 'Main Road', lat: -37.1, lng: 174.9570, am: 1800, pm: 1900, adt: 20000, peak: 1850, peakTime: '07:45', hv: 5, date: '2025-03-01' },
    { id: 'c2', src: 'user', name: 'Main Road', lat: -37.1, lng: 174.9680, am: 1500, pm: 1500, adt: 15000, hv: 0 }, { id: 'c3', src: 'user', name: 'Side 2N', lat: -37.0990, lng: 174.9556, am: 200, pm: 250, adt: 2000 },
    { id: 'c4', src: 'at', name: 'Far away', lat: -37.3, lng: 175.1, adt: 500 }];
  const countsG = [{ id: 'g1', src: 'user', name: 'Row 1 1', lat: -36.9025, lng: 174.7645, am: 900, pm: 950, adt: 9000, hv: 3 }, { id: 'g2', src: 'user', name: 'Col 2 0', lat: -36.9012, lng: 174.766, am: 400, pm: 420, adt: 4000 }, { id: 'g3', src: 'user', name: 'Row 2 0', lat: -36.905, lng: 174.7615, am: 120, pm: 90 }];
  const packWorlds = [['corridor', 'corridor', { active: OPT }], [fxCorr, 'corridor', {}], ['grid', 'grid', { base: 's-dm', active: OPT }], [fxGrid, 'grid', {}], ['corridor', 'corridor', {}], ['grid', 'grid', {}]];
  const packSets = n => [
    ['saved', {}],
    ['counts', { 'data.counts': n === 'corridor' ? countsC : countsG }],
    ['no results', { 'scenarios.0.res': {}, 'scenarios.1.res': {}, 'scenarios.$OPT.res': {} }],
    ['AM only', { 'scenarios.$OPT.res.PM': '$DELETE', 'scenarios.$BASE.res.PM': '$DELETE' }],
    ['mitigation', { 'scenarios.0.role': 'mitigation' }],
    ['baseline', { 'scenarios.$BASE.role': 'baseline' }],
    ['sh', { 'compliance.net.shUser.$A': true, 'compliance.net.shUser.$B': true, 'compliance.net.shUser.$C': true, 'compliance.net.sh.$E': '16' }],
    ['not sh', { 'compliance.net.shUser.$A': false, 'scenarios.$OPT.links.$A.cls': 'motorway' }],
    ['motorway class', { 'scenarios.$OPT.links.$A.cls': 'motorway', 'scenarios.$BASE.links.$A.cls': 'motorway', 'scenarios.$OPT.links.$B.cls': 'ramp', 'scenarios.$BASE.links.$B.cls': 'ramp' }],
    ['peds', { 'compliance.peds.$N1': 240, 'compliance.peds.$N2': 0, 'compliance.peds.nidc07wsv': 80, 'compliance.peds.nidc0fa0z': '15' }],
    ['changed roads', { 'scenarios.$OPT.links.$C.lanesAB': 3, 'scenarios.$OPT.links.$D.speed': 30, 'scenarios.$OPT.links.$E.cx': { limit: 60, meanSpeed: 47.5 }, 'scenarios.$OPT.links.$D.cx': { limit: 0, meanSpeed: 0 } }],
    ['junction changes', { 'scenarios.$OPT.nodes.$N1.control': 'roundabout', 'scenarios.$OPT.nodes.$N2.upgrade': 'signals' }],
    ['new link', { 'scenarios.$OPT.links.zzNew': { id: 'zzNew', a: '$N1', b: '$N2', name: 'New road', cls: 'collector', lanesAB: 1, lanesBA: 1, speed: 50, status: 'proposed', busLane: false, cycle: 'none', pts: [] } }],
    ['rltp', { 'complianceExtra.rltp': { objective: 'Improve access', measures: 'New link', duration: '2027-2030', costPerYear: '2,000,000', sources: 'NLTF; council' } }],
    ['region other', { 'compliance.ctx.region': 'other', 'complianceExtra.rltp.objective': 'x' }],
    ['no name', { name: '' }],
    ['zone data', { 'scenarios.$OPT.zones.$z1.jobs': { n: 120 }, 'scenarios.$OPT.zones.$z1.year': 2030, 'scenarios.$OPT.zones.$z2.lu.hotel': 12.5, 'scenarios.$OPT.zones.$dz.buildout': 75 }],
    ['tsdg and signals', { 'scenarios.$OPT.nodes.$N1.control': 'signals', 'scenarios.$OPT.nodes.$N2.control': 'signals', 'scenarios.$OPT.nodes.$N1.cycle': 90 }],
    ['no signals', { 'scenarios.$OPT.nodes.nidc07wsv.control': 'none', 'scenarios.$OPT.nodes.nidc0bhhv.control': 'none', 'scenarios.$OPT.nodes.nidc05pnh.control': 'none', 'scenarios.$OPT.nodes.nidc0ajz2.control': 'none' }],
    ['extent', { 'scenarios.$OPT.extent': { links: ['$A:ab', '$B'] } }],
    ['anop corridors', { 'complianceExtra.anop.corridors': [{ id: 'k1', name: 'Corridor 1', links: ['$A', '$B', '$C'], landUse: 'town-local', walk: 'primary', rel: { gt: 1.3, freight: 1.6, pt: 2.2 } }, { id: 'k2', name: 'Corridor 2', links: ['$D', '$E', 'nope'], landUse: 'other', crossings: 2 }] }],
  ];
  const packFns = {
    tsdg: ['tsdgPack', P => X.tsdgPack(P)],
    ssa: ['ssaPack', P => X.ssaPack(P)],
    sh: ['shPack', P => X.shPack(P)],
    mbcm: ['mbcmPack', P => X.mbcmPack(P)],
    rltp: ['rltpSheet', P => X.rltpSheet(P)],
    msm: ['msmPack', P => X.msmPack(P)],
    'anop-inputs': ['anopInputsCSV', P => X.anopInputsCSV(P)],
  };
  for (const [fx, n, extra] of packWorlds) {
    for (const [lab, sets] of packSets(n)) {
      const ss = { ...extra, ...specific(sets, n) };
      // the junction and signal names of the corridor are not the grid's
      if (n === 'grid' && /nidc07wsv|nidc05pnh|nidc0bhhv/.test(JSON.stringify(ss))) continue;
      if (n === 'corridor' && /nidc0ajz2/.test(JSON.stringify(ss))) continue;
      for (const [call, fn] of Object.values(packFns)) await add(PK2, `${call} ${fx} ${JSON.stringify(extra)} ${lab}`, call, fx, ss, fn);
      await add(PK2, `handoverManifest ${fx} ${JSON.stringify(extra)} ${lab}`, 'handoverManifest', fx, ss, P => X.handoverManifest(P));
    }
  }
  // the packs with a saved design-life run, and the exports and report of the packs
  for (const [name, fx] of Object.entries(lifeFx)) {
    await add(PK2, `tsdgPack with design life ${name}`, 'tsdgPack', fx, {}, P => X.tsdgPack(P));
    await add(PK2, `tsdgPack with design life ${name} counts`, 'tsdgPack', fx, { 'data.counts': name === 'corridor' ? countsC : countsG }, P => X.tsdgPack(P));
    await add(PK2, `tsdgPack with design life ${name} no signals`, 'tsdgPack', fx, name === 'corridor' ? { 'scenarios.1.nodes.nidc05pnh.control': 'none', 'scenarios.1.nodes.nidc07wsv.control': 'none', 'scenarios.1.nodes.nidc0bhhv.control': 'none' } : { 'scenarios.2.nodes.nidc0ajz2.control': 'none', 'scenarios.2.nodes.nidc0krpm.control': 'none' }, P => X.tsdgPack(P));
  }
  for (const [fx, n, extra] of [['corridor', 'corridor', { active: OPT }], [fxCorr, 'corridor', {}], [fxGrid, 'grid', {}], ['grid', 'grid', {}], ['corridor', 'corridor', {}]]) {
    for (const [lab, sets] of [['saved', {}], ['no results', { 'scenarios.0.res': {}, 'scenarios.1.res': {}, 'scenarios.$OPT.res': {} }]]) {
      const ss = { ...extra, ...specific(sets, n) };
      for (const id of ['tsdg', 'ssa', 'sh', 'mbcm', 'rltp', 'msm', 'anop-inputs']) {
        await add(PK2, `pack export ${id} ${fx} ${JSON.stringify(extra)} ${lab}`, 'export', fx, ss, P => ({ build: exp(id).build(P), enabled: exp(id).enabled(), label: exp(id).label }), { export: 'compliance-extra-' + id });
      }
      await add(PK2, `anop-inputs report ${fx} ${JSON.stringify(extra)} ${lab}`, 'report', fx, ss, P => report('compliance-extra-anop-inputs')(P), { report: 'compliance-extra-anop-inputs', ctx: false });
    }
  }
  flush();
});
