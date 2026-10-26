// Records the prototype's answers for the results queries, appraisal and assessment functions on inputs the two
// truth-test projects do not reach: other economics settings, analysis periods and discount schedules, validation
// records, data vintages, MSM forecasts, Future Connect classes, cycle lanes, proposed links and junction upgrades,
// options of the extent test, difference plots and the with/without table. Inputs are built from the projects and
// their recorded results; each case carries what the Rust port needs to make the same call
// (projects/spare-construct/crates/construct-transport/tests/analysis_cases.rs). Results are referred to as
// { scenario, period } in expected/<project>.json, with an optional patch of top-level numbers (year, gap, gapTol,
// noise95, kpi.vht, kpi.co2) or { paths: 'missing' }. Settings are the project's with a patch: settingsPatch replaces
// top-level keys, and its econ key (when given) replaces keys of the econ block (null removes one). Writes
// expected/analysis-cases.json.
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCore } from './js-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));
// As gen-expected.mjs: typed arrays as plain arrays, Maps and Sets as objects and arrays, non-finite numbers as strings
const plain = v => {
  if (ArrayBuffer.isView(v)) return Array.from(v, plain);
  if (v instanceof Map) return Object.fromEntries([...v].map(([k, x]) => [String(k), plain(x)]));
  if (v instanceof Set) return [...v].map(plain);
  if (typeof v === 'number') return Number.isFinite(v) ? v : String(v);
  if (Array.isArray(v)) return v.map(plain);
  if (v && typeof v === 'object') { const o = {}; for (const [k, x] of Object.entries(v)) if (typeof x !== 'function') o[k] = plain(x); return o; }
  return v;
};
const clone = o => JSON.parse(JSON.stringify(o));
const cases = [];
const add = (fn, args, out) => cases.push({ fn, ...args, out: plain(out) });

// The projects with their results as gen-expected.mjs leaves them
const load = name => {
  const c = loadCore({ seed: 11 });
  const P = c.migrate(JSON.parse(readFileSync(join(here, 'fixtures', name + '.json'), 'utf8')));
  c.Store.P = P;
  const rec = JSON.parse(readFileSync(join(here, 'expected', name + '.json'), 'utf8'));
  // results as the engine's own objects, from the recorded JSON (non-finite numbers back to numbers)
  const num = v => (v === 'NaN' ? NaN : v === 'Infinity' ? Infinity : v === '-Infinity' ? -Infinity : v);
  const revive = v => (Array.isArray(v) ? v.map(revive) : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, revive(x)])) : num(v));
  const res = {};
  for (const [sid, s] of Object.entries(rec.scenarios)) res[sid] = { AM: revive(s.periods.AM.res), PM: revive(s.periods.PM.res) };
  return { c, P, res, E: c.Engine, Store: c.Store };
};
const patchRes = (r, patch) => {
  if (!r || !patch) return r;
  const x = { ...r, kpi: { ...r.kpi } };
  for (const [k, v] of Object.entries(patch)) {
    if (k === 'paths' && v === 'missing') delete x.paths;
    else if (k.startsWith('kpi.')) x.kpi[k.slice(4)] = v;
    else x[k] = v;
  }
  return x;
};
const ref = (scenario, period, patch) => ({ scenario, period, ...(patch ? { patch } : {}) });
// The project's settings with a patch (see above)
const patched = (S0, patch) => {
  const S = clone(S0);
  for (const [k, v] of Object.entries(patch || {})) {
    if (k !== 'econ') { S[k] = v; continue; }
    for (const [ek, ev] of Object.entries(v)) { if (ev === null) delete S.econ[ek]; else S.econ[ek] = ev; }
  }
  return S;
};

/* ---------------- pure functions ---------------- */
{
  const { E } = load('grid');
  for (const i of [0, 0.02, 0.015, 0.08, -0.01]) for (const n of [0, 10, 30.5, 40, -1]) {
    add('uspwf', { i, n }, E.uspwf(i, n));
    add('agpwf', { i, n }, E.agpwf(i, n));
  }
  const schedules = [[{ to: 30, r: 2 }, { to: 100, r: 1.5 }, { to: null, r: 1 }], [{ to: 30, r: 2 }], [{ to: null, r: 8 }], 5, '3', [], 'x',
    [{ to: '', r: 2 }, { to: 10, r: 4 }], [{ to: 20, r: 'x' }, { to: 50, r: 3 }], [null, { r: 2 }, 7], [{ to: -5, r: 2 }, { to: 25, r: 6 }],
    [{ to: 60, r: 3 }, { to: 20, r: 5 }, { to: 40, r: 4 }], [[1, 2], { to: '45', r: '2.5' }]];
  for (const rates of schedules) for (const n of [40, 50, 0, 100, 130, 12.5]) add('pwf', { rates, n }, E.pwf(rates, n));
  const periods = [{}, { years: 0 }, { years: 25 }, { years: 40 }, { years: 40.4 }, { years: 40.5 }, { years: 45 }, { years: 45, longLifeNote: '   ' },
    { years: 45, longLifeNote: 'Bridge' }, { years: 60, longLifeNote: 'x' }, { years: 70 }, { years: 70, longLifeNote: 'Bridge' },
    { years: 70, approval: 'NZTA letter' }, { years: 70, longLifeNote: 'a', approval: ' b ' }, { years: '50' }, { years: -3 }, { years: 'abc' }];
  for (const e of periods) add('econPeriod', { E: e }, E.econPeriod(e));
  for (const bcr of [null, 0, 0.5, 1, 1.4, 1.5, 2.9, 3, 5.99, 6, 12, -2]) for (const edition of [undefined, '2027-30', '2024-27', 'zzz']) add('ipmBand', { bcr, edition: edition ?? null }, E.ipmBand(bcr, edition));
  for (const x of [null, 0, 0.04, 0.05, 0.25, 0.95, 9.94, 9.95, 9.96, 10, 12.5, -12.5, 123.4, -0.04, 1e21]) add('bcrText', { x }, E.bcrText(x));
  const fcs = [undefined, { gt: 'Primary arterial' }, { gt10: 'Strategic', gt: 'Collector', user: true }, { pt: 'FTN', src: 'conflated' },
    { freight: 'Level 1A' }, { gt: '', pt: null }, { gt: 'Local', gt10: null, pt10: 'Rapid transit', freight10: 'Level 2' }, { pt10: 'RTN' }, { gt: 0, user: 0, src: '' }];
  for (const cls of ['motorway', 'ramp', 'arterial', 'collector', 'local', 'lane', undefined]) for (const fc of fcs) for (const decade of [false, true]) {
    const L = { id: 'l1', a: 'n1', b: 'n2', ...(cls ? { cls } : {}), ...(fc ? { fc } : {}) };
    add('fcOf', { link: L, decade }, E.fcOf(L, decade));
  }
  const gts = ['MOTORWAY', 'strategic', 'Strategic Arterial', 'primary arterial', 'Secondary arterial', 'Arterial (from road class)', 'arterial (from road class) ', 'Collector road', 'collectors', 'Local', 'motorway\n', 5, null];
  const pts = ['Rapid transit network', 'irtn', 'IRTN line', 'xrtn', 'rtn_1', 'ftn-2', 'frequent  transit', 'Frequent Transit', 'interim RTN', 'aftn', null];
  const fr = ['level 1b', 'Level 2', 'level 3', 'LEVEL 1A', 'level 1a ', null];
  for (const gt of gts) add('extentClass', { fc: { gt, pt: null, freight: null } }, E.extentClass({ gt, pt: null, freight: null }));
  for (const pt of pts) add('extentClass', { fc: { gt: 'Collector (from road class)', pt, freight: null } }, E.extentClass({ gt: 'Collector (from road class)', pt, freight: null }));
  for (const freight of fr) add('extentClass', { fc: { gt: null, pt: 'rtn', freight } }, E.extentClass({ gt: null, pt: 'rtn', freight }));
}

/* ---------------- results and settings ---------------- */
for (const name of ['grid', 'corridor']) {
  const { P, res, E, Store } = load(name);
  const S0 = P.settings, ids = P.scenarios.map(s => s.id), base = name === 'grid' ? 's-dm' : ids[0], opt = 's-opt';
  const withSettings = (S, f) => { const keep = P.settings; P.settings = S; try { return f(); } finally { P.settings = keep; } };
  // travel and isochrone between nodes and zone centroids ("z:<zone id>"), on each result's times and on empty-network times
  for (const sc of P.scenarios) {
    const nodes = Object.keys(sc.nodes), zs = Object.values(sc.zones).map(z => 'z:' + z.id);
    const pairs = [[nodes[0], nodes[nodes.length - 1]], [zs[0], zs[1]], [zs[zs.length - 1], nodes[1]], [nodes[2], zs[0]], [nodes[0], nodes[0]], ['nope', nodes[0]]];
    for (const period of ['AM', 'PM', null]) {
      const r = period ? res[sc.id][period] : null, rr = period ? ref(sc.id, period) : null;
      for (const [from, to] of pairs) add('travel', { project: name, scenario: sc.id, res: rr, from, to }, E.travel(sc, r, from, to));
      for (const from of [nodes[0], zs[0], 'nope']) add('isochrone', { project: name, scenario: sc.id, res: rr, from }, E.isochrone(sc, r, from));
    }
  }
  // snr and exportReady
  const snrCases = [[null, null, undefined], [{ gap: 0 }, { gap: 0 }, undefined], [{ gap: 0.05 }, null, undefined], [{ 'kpi.vht': 0 }, null, undefined],
    [{ gap: 1e-3 }, { gap: 2e-3 }, 0], [null, null, '0.0001'], [null, null, 2e-6], [{ gap: 1e-6 }, { gap: 0 }, 1e-3], [{ 'kpi.vht': 500 }, { 'kpi.vht': 499.9 }, undefined]];
  for (const [pb, po, gapTol] of snrCases) {
    const sp = gapTol !== undefined ? { gapTol } : {}, S = patched(S0, sp);
    const out = withSettings(S, () => E.snr(patchRes(res[base].AM, pb), patchRes(res[opt].AM, po)));
    add('snr', { project: name, settingsPatch: sp, b: ref(base, 'AM', pb), o: ref(opt, 'AM', po) }, out);
  }
  add('exportReady', { project: name, res: null }, E.exportReady(null));
  for (const gap of [0.5, 1e-4, 0.00015, 0.000123456, 2]) add('exportReady', { project: name, res: ref(opt, 'PM', { gap }) }, E.exportReady(patchRes(res[opt].PM, { gap })));
  // lts on varied links, with and without results
  const L0 = Object.values(P.scenarios.find(s => s.id === opt).links)[0];
  const variants = [{}, { cycle: 'separated' }, { cycle: 'painted' }, { cycle: 'painted', speed: 60 }, { cycle: 'painted', lanesAB: 2 }, { speed: 30, lanesAB: 1, lanesBA: 1 },
    { speed: 40, lanesAB: 1, lanesBA: 0 }, { speed: 30, lanesAB: '1', lanesBA: 1.7 }, { cls: 'motorway' }, { cls: 'ramp', cycle: 'separated' }, { speed: null }, { speed: '45' }, { cls: 'nope', speed: 0 }];
  for (const v of variants) for (const withRes of [true, false]) for (const ptd of [10, '8']) {
    const L = { ...clone(L0), ...v };
    const sp = { peakToDaily: ptd }, S = patched(S0, sp);
    const out = withSettings(S, () => E.lts(L, withRes ? res[opt].AM : null));
    add('lts', { project: name, settingsPatch: sp, scenario: opt, linkPatch: v, res: withRes ? ref(opt, 'AM') : null }, out);
  }
  // capital with proposed links and junction upgrades
  {
    const sc = clone(P.scenarios.find(s => s.id === opt));
    const nodes = Object.values(sc.nodes);
    const controls = ['signals', 'roundabout', 'weird', undefined];
    nodes.slice(0, 4).forEach((n, k) => { n.upgrade = true; if (controls[k]) n.control = controls[k]; else delete n.control; });
    const links = Object.values(sc.links);
    links.slice(0, 3).forEach((L, k) => { L.status = 'proposed'; L.cls = ['arterial', 'mystery', 'local'][k]; });
    for (const sp of [{}, { econ: { laneKm: { arterial: null, local: '4' }, node: { signals: 2.5, roundabout: null } } }]) {
      const S = patched(S0, sp);
      add('capital', { project: name, settingsPatch: sp, scenario: sc }, withSettings(S, () => E.capital(sc)));
    }
  }
  // econ under other settings
  const E0 = S0.econ;
  const econs = [
    { method: 'uniform', discount: 4, years: 30 }, { method: 'uniform', discount: 0, years: 25 }, { method: 'uniform', activity: 'commercial', sensRate: 6 },
    { method: undefined }, { activity: 'commercial' }, { years: 50, longLifeNote: 'Bridge' }, { years: 70 }, { years: 70, approval: 'Letter', longLifeNote: 'x' },
    { benefitGrowth: 2.5 }, { benefitGrowth: '' }, { benefitGrowth: 'abc' }, { votUpdate: true, vot: '30' }, { periodHours: { AM: 600, PM: 0, IP: 500 } },
    { periodHours: { AM: 600, PM: 600, IP: 300, SAT: 200 } }, { periodHours: { IP: 0, PM: '700' } }, { rates: [] }, { rates: [{ to: 10, r: 3 }, { to: null, r: 2 }] }, { rates: 4 },
    { sensRate: 0 }, { sensRate: 6 }, { laneKm: { ...E0.laneKm, collector: 5000, arterial: 5000, local: 5000 } },
    { laneKm: { ...E0.laneKm, collector: 5000, arterial: 5000, local: 5000 }, p95: true }, { vot: 0 },
  ];
  for (const ex of econs) for (const growthMode of [undefined, 'compound']) {
    const sp = { econ: 'method' in ex && ex.method === undefined ? { ...ex, method: null } : ex, ...(growthMode ? { growthMode } : {}) };
    const S = patched(S0, sp);
    const A = P.scenarios.find(s => s.id === base), B = P.scenarios.find(s => s.id === opt);
    const keepA = A.res, keepB = B.res;
    A.res = res[base]; B.res = res[opt];
    for (const period of ['AM', 'PM']) add('econ', { project: name, settingsPatch: sp, base, opt, period, results: {} }, withSettings(S, () => E.econ(A, B, period)));
    A.res = keepA; B.res = keepB;
  }
  // econ: different years, one period only, the same scenario, no results
  {
    const A = P.scenarios.find(s => s.id === base), B = P.scenarios.find(s => s.id === opt);
    const runs = [
      { [opt]: { AM: { year: 2041 }, PM: { year: 2041 } } }, { [opt]: { AM: { year: 0 } } }, { [base]: { PM: { year: -1 } } },
      { drop: [[opt, 'PM']] }, { drop: [[opt, 'AM']] }, { drop: [[base, 'AM'], [base, 'PM']] }, { [opt]: { AM: { gap: 0.05 }, PM: { gap: 0.01 } } },
    ];
    for (const r of runs) {
      const pick = sid => {
        const out = {};
        for (const p of ['AM', 'PM']) if (!(r.drop || []).some(([s, q]) => s === sid && q === p)) out[p] = patchRes(res[sid][p], (r[sid] || {})[p]);
        return out;
      };
      const results = {};
      for (const sid of [base, opt]) {
        results[sid] = {};
        for (const p of ['AM', 'PM']) {
          if ((r.drop || []).some(([s, q]) => s === sid && q === p)) results[sid][p] = null;
          else if ((r[sid] || {})[p]) results[sid][p] = (r[sid] || {})[p];
        }
      }
      A.res = pick(base); B.res = pick(opt);
      for (const period of ['AM', 'PM']) add('econ', { project: name, settingsPatch: {}, base, opt, period, results }, E.econ(A, B, period));
    }
    A.res = res[base]; B.res = res[base];
    add('econ', { project: name, settingsPatch: {}, base, opt: base, period: 'AM', results: {} }, E.econ(A, A, 'AM'));
  }
  // model status and metadata
  {
    const variants = [
      {}, { me: { info: { AM: { baseYear: 2026, date: '2026-05-01' } } } }, { validation: { AM: { pass: true, independent: true } } },
      { validation: { AM: { pass: true, independent: true, category: 'd', standard: 'tmdg', date: '2026-06-01' } } },
      { validation: { PM: { pass: true, independent: true, category: 'e' } }, me: { info: { PM: { date: 1760000000000 } } } },
      { validation: { AM: { pass: true, independent: true, standard: 'tag' } } }, { validation: { AM: { pass: 'yes', independent: true } } },
      { validation: { AM: { pass: true, independent: false } }, me: { info: { AM: {} } } }, { validation: { AM: { pass: true, independent: 1, category: '' } } },
      { settings: { tmdg: { category: 'c', justification: ' because ' }, calib: { standard: 'rms' } }, validation: { AM: { pass: true, independent: true, standard: 'rms' } } },
      { settings: { tmdg: { category: 'c' }, calib: { standard: 'atap' } }, validation: { AM: { pass: true, independent: true, standard: 'tag', category: 'B' } } },
      { settings: { tmdg: null, calib: { standard: 'xyz' } }, validation: { AM: { pass: true, independent: true, standard: 'xyz' } } },
      { settings: { tmdg: { category: 'b' } }, validation: { AM: { pass: true, independent: true, category: 'A' } } },
      { validation: { AM: null }, me: { info: { AM: null } } },
    ];
    for (const v of variants) {
      const Pv = { ...P, settings: patched(S0, v.settings), ...(v.me ? { me: v.me } : {}), ...(v.validation ? { validation: v.validation } : {}) };
      for (const period of ['AM', 'PM']) add('modelStatus', { project: name, settingsPatch: v.settings || {}, me: v.me ?? null, validation: v.validation ?? null, period }, E.modelStatus(Pv, period));
    }
    const loadeds = [
      { nzta: { at: 1760000000000, n: 12 }, at: { at: '2026-03-01', source: 'AT counts', n: 0, licence: 'CC BY 4.0' }, user: { at: 0, n: 3 },
        fc: { at: '2026-10-21T10:00:00Z', n: null }, controlled: { at: '2026-10-21T23:30:00-02:00', licence: '' }, cas: { at: 1700000000000, n: 50, label: '2019-2023' } },
      { cas: { at: 1700000000001, n: 5 }, nzta: { n: 4 }, at: { at: '+010000-01-01', n: 1 }, user: { at: -1 }, fc: { at: '1969-12-31T23:59:59.999Z' } },
      { controlled: { at: '2024-02-29T12:00:00.5+13:00', source: '' }, user: { at: true } },
    ];
    for (const loaded of loadeds) {
      const Pv = { ...P, data: { loaded } };
      Store.P = Pv;
      const sc = P.scenarios.find(s => s.id === opt), r = res[opt].PM;
      for (const sp of [{}, { baseYear: '2020', growthMode: 'compound', growth: 2.25, junction: { losMethod: 'rta', capMethod: 'movement' } }])
        add('modelMeta', { project: name, data: { loaded }, settingsPatch: sp, scenario: opt, period: 'PM', c: ref(opt, 'PM') }, E.modelMeta(sc, 'PM', patched(S0, sp), r));
      Store.P = P;
    }
  }
  // MSM consistency (a base-year result against a later one)
  {
    const b0 = name === 'grid' ? ids[0] : base, f0 = opt;
    const links = Object.keys(P.scenarios.find(s => s.id === opt).links);
    // links in sorted order: the Rust project model holds P.data.msm as a JSON value with sorted keys
    const sorted = o => Object.fromEntries(Object.entries(o).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
    const msms = [null, {}, { links: {} }, { links: sorted({ [links[0]]: { growth: 1.5 }, [links[1]]: { growth: 0 }, [links[2]]: { growth: 'x' }, nolink: { growth: 2 }, [links[3]]: { growth: -1 }, [links[4]]: { growth: 0 }, [links[5]]: 3 }) }];
    for (const msm of msms) for (const [pb, pf] of [[{ year: 2026 }, { year: 2036 }], [{ year: 2026 }, { year: 2026 }], [null, { year: 2031 }]]) for (const pct of [undefined, 5]) {
      const out = E.msmCheck(patchRes(res[b0].AM, pb), patchRes(res[f0].AM, pf), msm, pct === undefined ? {} : { pct });
      add('msmCheck', { project: name, base: ref(b0, 'AM', pb), fut: ref(f0, 'AM', pf), msm, pct: pct ?? null }, out);
    }
    add('msmCheck', { project: name, base: ref(b0, 'AM'), fut: null, msm: msms[3], pct: null }, E.msmCheck(res[b0].AM, null, msms[3]));
  }
  // extent test, difference and with / without with options
  {
    // Future Connect classes on the first links of the with scenario; the without scenario's links (opts.scWithout) for links only it has
    const fcs = [{ gt: 'Primary arterial' }, { pt: 'FTN', gt10: 'Collector', src: 'conflated' }, { freight: 'Level 1B', user: true }, { gt: 'Collector' }, { gt10: 'Motorway', gt: 'Local' }];
    const scB = clone(P.scenarios.find(s => s.id === opt));
    Object.values(scB.links).slice(0, fcs.length).forEach((L, k) => { L.fc = fcs[k]; });
    // the with scenario without its first link, so that link is found in the without scenario only
    const firstLink = Object.keys(scB.links)[0], scB2 = clone(scB); delete scB2.links[firstLink];
    const scW = P.scenarios.find(s => s.id === base);
    for (const opts of [{}, { floor: 0 }, { floor: 0, decade: 'first' }, { decade: 'current', floor: '2' }]) {
      add('extentTest', { project: name, scenario: opt, fc: fcs, dropFirst: false, without: ref(base, 'AM'), with: ref(opt, 'AM'), opts }, E.extentTest(scB, res[base].AM, res[opt].AM, opts));
      add('extentTest', { project: name, scenario: opt, fc: fcs, dropFirst: true, scWithout: base, without: ref(base, 'PM', { gapTol: 1e-3 }), with: ref(opt, 'PM'), opts },
        E.extentTest(scB2, patchRes(res[base].PM, { gapTol: 1e-3 }), res[opt].PM, { ...opts, scWithout: scW }));
    }
    add('extentTest', { project: name, scenario: opt, fc: fcs, dropFirst: false, without: null, with: ref(opt, 'AM'), opts: {} }, E.extentTest(scB, null, res[opt].AM));
    for (const [o, pb, po] of [[{ k: 0.5, zoom: 12 }, null, null], [{ k: '0.1', zoom: 15 }, { noise95: 15 }, { noise95: null }], [{ zoom: 14.9 }, { noise95: 'NaN' }, { noise95: 4 }], [{}, { noise95: 40 }, null]]) {
      const b = patchRes(res[base].AM, pb && { ...pb, noise95: pb.noise95 === 'NaN' ? NaN : pb.noise95 }), q = patchRes(res[opt].AM, po);
      add('difference', { project: name, base: ref(base, 'AM', pb), opt: ref(opt, 'AM', po), o }, E.difference(b, q, o));
    }
    add('difference', { project: name, base: null, opt: ref(opt, 'AM'), o: {} }, E.difference(null, res[opt].AM));
    for (const named of [false, true]) for (const pb of [null, { gapTol: 1e-3 }]) {
      const opts = named ? { name: id => 'J ' + id.toUpperCase() } : {};
      add('withWithout', { project: name, without: ref(base, 'PM', pb), with: ref(opt, 'PM'), named }, E.withWithout(patchRes(res[base].PM, pb), res[opt].PM, opts));
    }
    add('withWithout', { project: name, without: ref(opt, 'AM'), with: ref(base, 'AM'), named: false }, E.withWithout(res[opt].AM, res[base].AM));
    const sc = P.scenarios.find(s => s.id === opt);
    for (const list of [['PM', 'AM'], ['AM'], ['AM', 'AM'], []]) add('devDistribution', { project: name, scenario: opt, list }, E.devDistribution(sc, list.map(p => res[opt][p])));
    add('devDistribution', { project: name, scenario: null, list: ['AM'] }, E.devDistribution(null, [res[opt].AM]));
    const key = Object.keys(res[opt].AM.arcs)[3];
    add('selectLink', { project: name, res: ref(opt, 'AM'), key }, E.selectLink(res[opt].AM, key));
    add('selectLink', { project: name, res: ref(opt, 'AM'), key: 'nope:ab' }, E.selectLink(res[opt].AM, 'nope:ab'));
    add('selectLink', { project: name, res: ref(opt, 'AM', { paths: 'missing' }), key }, E.selectLink(patchRes(res[opt].AM, { paths: 'missing' }), key));
  }
}

writeFileSync(join(here, 'expected', 'analysis-cases.json'), JSON.stringify({ cases }));
console.log(cases.length, 'analysis cases written');
