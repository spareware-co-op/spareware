// Records the prototype's data import and export (model.js Importers) for the Rust port's importers module
// (projects/spare-construct/crates/construct-transport/src/importers, tests/import_truth.rs): an OpenStreetMap (Overpass) answer
// read into a scenario, gateways added where roads leave the area, GeoJSON lines read as roads, and the network written as
// GeoJSON and CSV with and without a run's results. Writes expected/import-cases.json: for each case the operation list (the
// same format as edit-ops.mjs, with the operations osm, mark, gateways, geojson, openProject and export added), what each one
// returned and the scenario afterwards. Inputs are the made-up files in fixtures/ (make-osm-fixtures.mjs and the GeoJSON files):
// nothing is downloaded. Ids come from the harness's seeded generator, as in gen-edit.mjs.
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { run, plain, clone, idStart } from './edit-ops.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const NOW = Date.UTC(2026, 9, 21);
const read = rel => readFileSync(join(here, rel), 'utf8');
// Non-finite numbers are strings in the recorded files; the prototype is given the numbers
const revive = v => (v === 'NaN' ? NaN : v === 'Infinity' ? Infinity : v === '-Infinity' ? -Infinity : Array.isArray(v) ? v.map(revive) : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, revive(x)])) : v);

const sc = c => c.P.scenarios.find(s => s.id === c.P.active) || c.P.scenarios[0];
const bounds = ([s, w, n, e]) => ({ getSouth: () => s, getWest: () => w, getNorth: () => n, getEast: () => e });

// The operations this file adds to edit-ops.mjs
const EXTRA = {
  osm: ['sc', (c, a) => c.core.Importers.osmToScenario(sc(c), JSON.parse(read('fixtures/' + a.file)), a.sites ? { sites: true } : {})],
  mark: [null, c => { c.mark = new Set(Object.keys(sc(c).nodes)); return null; }],
  gateways: ['sc', (c, a) => {
    const only = a.only === 'fresh' ? new Set(Object.keys(sc(c).nodes).filter(id => !c.mark.has(id))) : undefined;
    return c.core.Importers.autoGateways(sc(c), bounds(a.bounds), only);
  }],
  geojson: ['sc', (c, a) => c.core.Importers.importGeoJSON(sc(c), JSON.parse(read('fixtures/' + a.file)), a.template)],
  openProject: ['project', (c, a) => { c.P = c.core.migrate(JSON.parse(read(`inputs/${a.name}.json`))); c.core.Store.P = c.P; return null; }],
  export: [null, (c, a) => {
    const s = sc(c);
    const res = a.period ? revive(JSON.parse(read(`expected/${a.name}.json`)).scenarios[s.id].periods[a.period].res) : null;
    return { geojson: c.core.Importers.geojson(s, res), csv: c.core.Importers.linksCSV(s, res) };
  }],
};

const ops = [];
const cases = [];
const begin = (name, seed) => { ops.length = 0; cases.push({ name, seed, ops: [] }); };
const op = (name, a = {}) => { cases[cases.length - 1].ops.push({ op: name, ...a }); };

const TIGHT = [-37.0682, 174.935, -37.0612, 174.948];   // just around the made-up suburb, so roads running to its edge are at the box's edge
const WIDE = [-37.09, 174.91, -37.04, 174.97];          // far out: no road is near the edge

// 1. The whole suburb, no sites; again (nothing added); a wide box; a tight box; exports
begin('papakura', 601);
op('newProject', { name: 'Import truth: Papakura' });
op('osm', { file: 'overpass-papakura.json' });
op('export');
op('osm', { file: 'overpass-papakura.json' });
op('gateways', { bounds: WIDE });
op('gateways', { bounds: TIGHT });
op('gateways', { bounds: TIGHT });
op('export');
op('osm', { file: 'overpass-papakura-east.json' });
op('osm', { file: 'overpass-bare.json' });
op('export');

// 2. With sites, and AT points near the two signalised junctions
begin('papakura sites', 602);
op('newProject', { name: 'Import truth: sites' });
op('data', { set: { controlled: [{ lat: -37.0648282, lng: 174.9388742, no: 'AT-GSR1' }, { lat: -37.0647649, lng: 174.9416887, id: 'AT-GSR2' }, { lat: -37.0645478, lng: 174.9443342, no: 3 }] } });
op('osm', { file: 'overpass-papakura.json', sites: true });
op('export');
op('osm', { file: 'overpass-papakura-east.json', sites: true });

// 3. Incremental: the eastern half first, then the whole, with gateways only at the new ends each time
begin('papakura incremental', 603);
op('newProject', { name: 'Import truth: incremental' });
op('mark');
op('osm', { file: 'overpass-papakura-east.json' });
op('gateways', { bounds: TIGHT, only: 'fresh' });
op('mark');
op('osm', { file: 'overpass-papakura.json' });
op('gateways', { bounds: TIGHT, only: 'fresh' });
op('gateways', { bounds: TIGHT });
op('export');

// 4. The odd elements on their own
begin('bare', 604);
op('newProject', { name: 'Import truth: bare' });
op('osm', { file: 'overpass-bare.json' });
op('osm', { file: 'overpass-bare.json', sites: true });
op('gateways', { bounds: [-37.0715, 174.9395, -37.0698, 174.9425] });
op('export');

// 5. GeoJSON lines onto the imported suburb, with two templates, and the other file shapes
begin('geojson', 605);
op('newProject', { name: 'Import truth: GeoJSON' });
op('osm', { file: 'overpass-papakura.json' });
op('geojson', { file: 'papakura-proposals.geojson', template: { cls: 'arterial', lanesAB: 2, lanesBA: 2, speed: 50, status: 'proposed' } });
op('export');
op('geojson', { file: 'papakura-proposals.geojson', template: { cls: 'local', lanesAB: 1, lanesBA: 0, speed: 30, status: 'existing', name: 'Template name', busLane: true, cycle: 'painted', stage: 'committed', funding: 'funded' } });
op('geojson', { file: 'single-feature.geojson', template: { cls: 'collector', lanesAB: 1, lanesBA: 1, speed: 50, status: 'proposed' } });
op('geojson', { file: 'line-geometry.geojson', template: { cls: 'collector', lanesAB: 1, lanesBA: 1, speed: 50, status: 'proposed' } });
op('geojson', { file: 'geometry-collection.geojson', template: { cls: 'collector', lanesAB: 1, lanesBA: 1, speed: 50, status: 'proposed' } });
op('export');

// 6. A scenario of GeoJSON roads alone (no OpenStreetMap), controls from degree
begin('geojson alone', 606);
op('newProject', { name: 'Import truth: GeoJSON alone' });
op('geojson', { file: 'papakura-proposals.geojson', template: { cls: 'collector', lanesAB: 1, lanesBA: 1, speed: 50, status: 'existing' } });
op('geojson', { file: 'papakura-proposals.geojson', template: { cls: 'collector', lanesAB: 1, lanesBA: 1, speed: 50, status: 'existing' } });
op('export');

// 7. The saved test projects, written out with each run's results and without
for (const name of ['corridor', 'grid']) {
  const ex = JSON.parse(read(`expected/${name}.json`));
  begin(`export ${name}`, 610 + name.length);
  op('openProject', { name });
  for (const [id, s] of Object.entries(ex.scenarios)) {
    op('selectScenario', { id });
    op('export');
    for (const period of Object.keys(s.periods)) op('export', { name, period });
  }
}

// The Overpass query the prototype sends (its fetch is replaced by one that keeps the request and refuses)
const queries = [];
{
  const { loadCore } = await import('./js-core.mjs');
  const core = loadCore({ seed: 1 });
  const real = globalThis.fetch;
  let body = null;
  globalThis.fetch = async (url, init) => { body = init.body; throw new Error('no network in the truth tests'); };
  for (const b of [[-37.0682, 174.935, -37.0612, 174.948], [-36.8, 174.6, -36.7, 174.7], [0, 0, 0.000001, 0.0000005], [-90, -180, 90, 180], [-37.06820049, 174.93500051, -37.0612, 174.9480004]]) {
    for (const opts of [{}, { service: true }, { local: false }, { service: true, local: false }, { local: true }]) {
      body = null;
      try { await core.Importers.overpass(bounds(b), opts); } catch { /* every server refused */ }
      queries.push({ b, opts, q: decodeURIComponent(body.slice(5)) });
    }
  }
  globalThis.fetch = real;
}
const out = cases.map(c => ({ name: c.name, seed: c.seed, now: NOW, ...idStart(c.seed), ops: c.ops, steps: run(c.seed, c.ops, EXTRA) }));
const text = JSON.stringify(out);
writeFileSync(join(here, 'expected', 'import-cases.json'), text);
writeFileSync(join(here, 'expected', 'import-queries.json'), JSON.stringify(queries));
console.log('import-cases.json', text.length, 'bytes');
for (const c of out) {
  const rets = c.steps.map((s, i) => [c.ops[i].op, s.ret]).filter(([o]) => ['osm', 'gateways', 'geojson'].includes(o)).map(([o, r]) => `${o}:${r}`);
  console.log(c.name, rets.join(' '));
}
