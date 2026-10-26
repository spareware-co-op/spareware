// Runs the prototype's engine on every truth-test project and records what it returns. The Rust port must reproduce
// these: projects/spare-construct/crates/construct-transport/tests read inputs/<name>.json and expected/<name>.json.
import { readFileSync, writeFileSync, mkdirSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCore } from './js-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));
for (const d of ['inputs', 'expected']) mkdirSync(join(here, d), { recursive: true });

// JSON for results: typed arrays as plain arrays, Maps and Sets as objects and arrays, non-finite numbers as strings
const plain = v => {
  if (ArrayBuffer.isView(v)) return Array.from(v, plain);
  if (v instanceof Map) return Object.fromEntries([...v].map(([k, x]) => [String(k), plain(x)]));
  if (v instanceof Set) return [...v].map(plain);
  if (typeof v === 'number') return Number.isFinite(v) ? v : String(v);
  if (Array.isArray(v)) return v.map(plain);
  if (v && typeof v === 'object') { const o = {}; for (const [k, x] of Object.entries(v)) if (typeof x !== 'function') o[k] = plain(x); return o; }
  return v;
};

const only = process.argv[2];
for (const file of readdirSync(join(here, 'fixtures')).filter(f => f.endsWith('.json'))) {
  const name = file.replace(/\.json$/, '');
  if (only && only !== name) continue;
  const c = loadCore({ seed: 11 });
  const { Engine, Store } = c;
  const P = c.migrate(JSON.parse(readFileSync(join(here, 'fixtures', file), 'utf8')));
  Store.P = P;
  writeFileSync(join(here, 'inputs', file), Store.serialise(P));
  const S = P.settings, out = { project: name, scenarios: {} };
  for (const sc of P.scenarios) {
    P.active = sc.id;
    const rec = out.scenarios[sc.id] = { yearOf: Engine.yearOf(sc), capital: plain(Engine.capital(sc)), periods: {} };
    sc.res = {};
    for (const period of ['AM', 'PM']) {
      const res = await Engine.assign(sc, { period });
      sc.res[period] = res;
      const zones = Object.values(sc.zones);
      const r = rec.periods[period] = { res: plain(res) };
      r.tripEnds = Object.fromEntries(zones.filter(z => z.kind !== 'gateway').map(z => [z.id, plain(Engine.tripEnds(z, S, period, sc))]));
      const zl = zones.filter(z => z.kind !== 'gateway');
      if (zl.length >= 2) r.travel = plain(Engine.travel(sc, res, zl[0].id, zl[1].id));
      if (zl.length >= 1) r.isochrone = plain(Engine.isochrone(sc, res, zl[0].id));
      const busiest = Object.entries(res.arcs || {}).sort((a, b) => (b[1].v || 0) - (a[1].v || 0))[0];
      if (busiest) { r.selectLinkKey = busiest[0]; r.selectLink = plain(Engine.selectLink(res, busiest[0])); }
      r.lts = Object.fromEntries(Object.values(sc.links).map(L => [L.id, Engine.lts(L, res)]));
      r.exportReady = plain(Engine.exportReady(res));
      r.modelMeta = plain(Engine.modelMeta(sc, period, S, res.conv ? res : undefined));
    }
    rec.devDistribution = plain(Engine.devDistribution(sc, [sc.res.AM, sc.res.PM]));
  }
  // Option against its comparison scenario, in each peak
  const pairs = name === 'grid' ? [['s-dm', 's-opt']] : [[P.scenarios[0].id, 's-opt']];
  out.pairs = {};
  for (const [a, b] of pairs) {
    const A = P.scenarios.find(s => s.id === a), B = P.scenarios.find(s => s.id === b), pr = out.pairs[a + '|' + b] = {};
    for (const period of ['AM', 'PM']) {
      pr[period] = {
        econ: plain(Engine.econ(A, B, period)),
        difference: plain(Engine.difference(A.res[period], B.res[period])),
        withWithout: plain(Engine.withWithout(A.res[period], B.res[period])),
        extentTest: plain(Engine.extentTest(B, A.res[period], B.res[period])),
      };
    }
  }
  P.active = P.scenarios[0].id;
  out.sensitivity = plain(await Engine.sensitivity(P.scenarios[0], { period: 'AM' }));
  out.modelStatus = { AM: plain(Engine.modelStatus(P, 'AM')), PM: plain(Engine.modelStatus(P, 'PM')) };
  out.standards = plain(Engine.STANDARDS);
  writeFileSync(join(here, 'expected', file), JSON.stringify(out));
  console.log(name, 'expected written');
}
