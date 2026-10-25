// Records the prototype's network editing, junction sites, assessment scenarios, register, project upgrade (migrate), ids and name
// ordering for the Rust port's edit module (projects/spare-construct/crates/construct-transport/src/edit,
// tests/edit_truth.rs). Writes expected/edit-*.json:
//   edit-ids.json       U.uid for three seeds, Number.toString(36), Date.toISOString and localeCompare
//   edit-build.json     a network built node by node, queried, split, joined to, deleted from (edit-scripts.mjs buildScript)
//   edit-sites.json     Sites.detect with every rule, AT points, hand-made sites, merge and split
//   edit-scenarios.json createSet, sensitivityOf, copy, describe, table, groups, check, the register
//   edit-history.json   Store snapshot, undo and redo, and the 60-step cap
//   edit-migrate.json   migrate() on saved projects of every kind, and Store.adoptDefaults / keepSettings
// The model is loaded with the harness's seeded generator (js-core.mjs) and fixed clock, so every id is the same on every run.
// Loading model.js draws the interface's tab id, so the generator is one number in and the id counter at 1 (measured in
// edit-ops.mjs idStart and written to each file as rngSkip and seq0: the Rust test replays the same).
import { writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCore } from './js-core.mjs';
import { OPS, run, plain, clone, generator, idStart, makeCtx } from './edit-ops.mjs';
import { buildScript, sitesScript, scenariosScript, historyScript, fuzzScript } from './edit-scripts.mjs';
import { migrateCases, adoptCases } from './edit-migrate-cases.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const dir = join(here, 'expected');
mkdirSync(dir, { recursive: true });
const NOW = Date.UTC(2026, 9, 21);
const write = (name, data) => { writeFileSync(join(dir, name), JSON.stringify(data)); console.log(name, JSON.stringify(data).length, 'bytes'); };

/* ---------- the scripts ---------- */
for (const [file, make] of [['edit-build.json', buildScript], ['edit-sites.json', sitesScript], ['edit-scenarios.json', scenariosScript], ['edit-history.json', historyScript]]) {
  const s = make();
  // Operations that read every scenario the script made
  if (s.describeAll) s.ops.push({ op: 'describeEach' }, { op: 'checkEach' });
  const steps = run(s.seed, s.ops);
  write(file, { seed: s.seed, now: NOW, ...idStart(s.seed), ops: s.ops, steps });
}

// Random networks: three seeds, each a few hundred operations of every kind (selectors pick roads and junctions by position)
{
  const cases = [];
  for (const [seed, p] of [[7001, { clusters: 9, extra: 150 }], [7002, { clusters: 6, extra: 150 }], [7003, { clusters: 14, extra: 200 }], [7004, { clusters: 4, extra: 120 }]]) {
    const s = fuzzScript(seed, p);
    cases.push({ seed, now: NOW, ...idStart(seed), ops: s.ops, steps: run(seed, s.ops) });
  }
  write('edit-fuzz.json', cases);
}

/* ---------- ids, number text, dates, name order ---------- */
{
  const ids = [];
  for (const seed of [1, 7, 20261021]) {
    const ctx = makeCtx(seed), prefixes = ['n', 'l', 'z', 's', 'j', 'r', 'p', 'set', 't', ''];
    const list = [];
    for (let i = 0; i < 1500; i++) list.push(ctx.core.U.uid(prefixes[i % prefixes.length]));
    ids.push({ seed, ...idStart(seed), now: NOW, prefixes, ids: list });
  }
  // The clock's other values change only the middle of an id
  const later = (() => {
    const core = loadCore({ seed: 3, now: Date.UTC(2031, 0, 5, 13, 14, 15, 678) });
    return { seed: 3, ...idStart(3), now: Date.UTC(2031, 0, 5, 13, 14, 15, 678), ids: Array.from({ length: 40 }, (_, i) => core.U.uid('x' + i)) };
  })();
  const g = generator(20261021);
  const rnd53 = () => (Math.floor(g() * 2 ** 26) * 2 ** 27 + Math.floor(g() * 2 ** 27)) / 2 ** 53;
  const radix = [0.5, 0.25, 1 / 3, 2 / 3, 0.1, 0.2, 0.3, 1e-10, 1e-300, 5e-324, 0.9999999999999999, 0.99999999, 1 / 36, 1 / 1296, 35 / 36, 2 ** -32, 1 - 2 ** -32, 2 ** -53, 0.123456789, 0.999, 1e-5, 36 ** -3, 36 ** -3 * 0.9999999999]
    .concat(Array.from({ length: 300 }, () => Math.floor(g() * 2 ** 32) / 2 ** 32))
    .concat(Array.from({ length: 600 }, rnd53))
    .concat(Array.from({ length: 100 }, () => Math.exp(-g() * 40)))
    .map(x => [x, x.toString(36)]);
  const msList = [0, 1, -1, 951782400000, 951782400000 + 86399999, 4102444800000, NOW, NOW + 86399999, -1.5, -0.5, 0.9, -86400000.5, 1e12 + 0.999, -62198755200000, -62198755200001, 253402300799999, 253402300800000, 1e12, 1.7e12 + 0.5]
    .concat(Array.from({ length: 200 }, () => Math.floor((g() - 0.3) * 6e12)));
  const dates = msList.map(ms => [ms, new Date(ms).toISOString()]);
  // localeCompare: real names, then random strings over a repertoire of letters, digits, accents and punctuation
  const NAMES = ['Great South Road', 'Waihoehoe Road', 'Alfriston Road', 'Manukau Road', 'Māngere Road', 'Mangere Road', 'Mangere Bridge', 'Māngere Bridge', 'State Highway 1', 'State Highway 20', 'SH1', 'SH 1',
    '1st Avenue', '2nd Avenue', '10th Avenue', 'K Road', 'Karangahape Road', 'Queen Street', 'queen street', 'QUEEN STREET', 'Queen St', 'Queens Road', "O'Connell Street", 'O’Connell Street', 'OConnell Street',
    'Grey Lynn Road', 'Grey-Lynn Road', 'Grey_Lynn', 'Te Atatū Road', 'Te Atatu Road', 'Te Atatū', 'Ōtāhuhu Road', 'Otahuhu Road', 'Ōtara Road', 'Otara Road', 'Café Road', 'Cafe Road', 'Zealand Road',
    'Álvarez Street', 'Alvarez Street', 'Ångström Lane', 'Angstrom Lane', 'Ñandú Way', 'Nandu Way', 'Æther Place', 'Aether Place', 'Straße', 'Strasse', 'Øresund Drive', 'Oresund Drive',
    'Łódź Road', 'Lodz Road', 'Bell Road', 'bell road', 'Bell-Road', 'Bell Rd', 'Bell  Road', 'Bell Road ', '', 'a', 'A', 'b', 'B', 'z', 'Z', 'é', 'e', 'E', 'É', 'ē', 'Ē', 'ē', '1', '10', '2', 'a1', 'a10', 'a2',
    'Main Rd (north)', 'Main Rd (south)', 'Main Rd [north]', 'Main Rd /north', 'Main Rd #1', 'Main Rd & Co', 'Main Rd + Co', 'Main Rd @ Mill', 'Main Rd 100%', 'Main Rd $5', 'Side 4.1S', 'Side 4N', 'Side 4 N', 'Side 4-N', 'Side 4_N',
    '東京', 'Ελλάδα', 'Ωmega', 'Привет', 'über', 'uber', 'Uber', 'Über', 'mañana', 'manana', 'Manana', 'å', 'ä', 'ö', 'o', 'p'];
  const pairs = [];
  for (const a of NAMES.slice(0, 70)) for (const b of NAMES) pairs.push([a, b, Math.sign(a.localeCompare(b))]);
  const REP = [...'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 -_.,\'"()/&#%+'].concat(['ā', 'ē', 'ī', 'ō', 'ū', 'Ā', 'Ō', 'é', 'è', 'ü', 'ñ', 'ç', 'à', 'Å', 'ø', 'ł', '’']);
  const rstr = () => Array.from({ length: Math.floor(g() * 6) }, () => REP[Math.floor(g() * REP.length)]).join('');
  for (let i = 0; i < 4000; i++) { const a = rstr(), b = g() < 0.3 ? a.toUpperCase() : g() < 0.3 ? a.normalize('NFD') : rstr(); pairs.push([a, b, Math.sign(a.localeCompare(b))]); }
  write('edit-ids.json', { ids, later, radix, dates, collate: pairs });
}

/* ---------- migrate ---------- */
{
  const cases = [];
  for (const k of migrateCases()) {
    const c = makeCtx(k.seed), start = idStart(k.seed);
    let P;
    try { P = c.core.migrate(JSON.parse(k.text)); } catch (e) { console.log("migrate throws for", k.name, String(e.message)); continue; }
    const orders = {
      settings: Object.keys(P.settings), rates: Object.keys(P.settings.rates || {}),
      scenarios: P.scenarios.map(s => ({ id: s.id, nodes: Object.keys(s.nodes), links: Object.keys(s.links), zones: Object.keys(s.zones), routes: Object.keys(s.routes), sites: Object.keys(s.sites || {}), res: Object.keys(s.res || {}) })),
    };
    cases.push({ name: k.name, text: k.text, seed: k.seed, ...start, expected: JSON.parse(JSON.stringify(P)), orders });
  }
  const adopt = [];
  for (const k of adoptCases()) {
    const c = makeCtx(k.seed), start = idStart(k.seed), { Store } = c.core;
    Store.P = c.core.migrate(JSON.parse(k.text));
    const rets = [];
    const doIt = a => { if ('adopt' in a) rets.push(Store.adoptDefaults(a.adopt)); else rets.push(Store.keepSettings()); };
    doIt(k.action);
    if (k.action.again) doIt(k.action.again);
    adopt.push({ name: k.name, text: k.text, action: k.action, seed: k.seed, ...start, rets, expected: JSON.parse(JSON.stringify(Store.P)) });
  }
  write('edit-migrate.json', { now: NOW, cases, adopt });
}
