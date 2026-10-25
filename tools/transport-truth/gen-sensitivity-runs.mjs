// Records every assignment run the prototype's sensitivity pack makes, so the Rust port can replay them: for each
// project in fixtures/, Engine.sensitivity(first scenario, { period: 'AM' }) exactly as gen-expected.mjs calls it, with
// each inner assign call's inputs (the changed scenario copy, the changed settings copy, the options) and its result.
// The Rust test (projects/spare-construct/crates/construct-transport/tests/sensitivity_truth.rs) runs its own
// sensitivity with an assignment that checks those inputs and returns the recorded results, then compares the table
// with expected/<name>.json "sensitivity". Writes expected/sensitivity-runs-<name>.json.
//
// sensitivity calls the engine's own assign (inside the Engine closure), not Engine.assign, so the recorder goes in
// through the source text: js-core.mjs reads engine.js with node:fs readFileSync, which is wrapped here (and the
// change pushed to ES module imports with syncBuiltinESMExports) to route the two assign calls of sensitivity through
// Engine.__record. Nothing else in the engine changes.
import fs from 'node:fs';
import { syncBuiltinESMExports } from 'node:module';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const read0 = fs.readFileSync;
const CALLS = [
  ['out.with[p] = await assign(s1, ', 'out.with[p] = await Engine.__record(assign, label, t, \'with\', s1, '],
  ['out.without[p] = await assign(w1, ', 'out.without[p] = await Engine.__record(assign, label, t, \'without\', w1, '],
];
fs.readFileSync = function (path, ...rest) {
  const text = read0.call(this, path, ...rest);
  if (!String(path).endsWith('engine.js')) return text;
  let s = String(text);
  for (const [from, to] of CALLS) {
    if (s.split(from).length !== 2) throw new Error(`engine.js: expected one "${from}" in sensitivity`);
    s = s.replace(from, to);
  }
  return s;
};
syncBuiltinESMExports();
const { loadCore } = await import('./js-core.mjs');

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
const json = v => JSON.parse(JSON.stringify(v));

const only = process.argv[2];
for (const file of fs.readdirSync(join(here, 'fixtures')).filter(f => f.endsWith('.json'))) {
  const name = file.replace(/\.json$/, '');
  if (only && only !== name) continue;
  const c = loadCore({ seed: 11 });
  const { Engine, Store } = c;
  const P = c.migrate(JSON.parse(read0(join(here, 'fixtures', file), 'utf8')));
  Store.P = P;
  // The runs gen-expected.mjs makes before the sensitivity pack, so the project is in the same state
  for (const sc of P.scenarios) {
    P.active = sc.id;
    sc.res = {};
    for (const period of ['AM', 'PM']) sc.res[period] = await Engine.assign(sc, { period });
  }
  P.active = P.scenarios[0].id;
  const calls = [];
  Engine.__record = async (assign, label, t, which, sc, opts) => {
    const call = { label, test: t ? t.id : null, which, period: opts.period, sc: json(sc), settings: json(opts.settings),
      opts: json({ gapTol: opts.gapTol, maxIter: opts.maxIter, period: opts.period, demandFactor: opts.demandFactor }) };
    const res = await assign(sc, opts);
    call.res = plain(res);
    calls.push(call);
    return res;
  };
  const table = plain(await Engine.sensitivity(P.scenarios[0], { period: 'AM' }));
  // The recorded runs must give the table gen-expected.mjs recorded (run time aside)
  const want = JSON.parse(read0(join(here, 'expected', file), 'utf8')).sensitivity;
  const noMs = x => JSON.stringify({ ...x, ms: 0 });
  if (noMs(table) !== noMs(want)) throw new Error(`${name}: the sensitivity table differs from expected/${file}; run gen-expected.mjs first`);
  fs.writeFileSync(join(here, 'expected', `sensitivity-runs-${name}.json`), JSON.stringify({ project: name, scenario: P.scenarios[0].id, opts: { period: 'AM' }, calls }));
  console.log(name, calls.length, 'sensitivity runs written');
}
