// Records the prototype's route queries on results between real graph nodes (gen-expected.mjs asked with zone ids,
// which are not graph nodes, so those recordings are null): Engine.travel between pairs of network nodes and zone
// centroids ("z:<zone id>", the graph's own ids), Engine.isochrone from nodes and centroids, and Engine.selectLink on
// the busiest arcs, an arc without flow and a key that is not in the network. Each answer is the prototype's own result
// (for the result the prototype's assign gave) and the Rust test (tests/routes_truth.rs) asks the same questions of
// the result the Rust assignment gives. Writes expected/routes-<project>.json:
// { project, scenarios: { <id>: { <period>: { travel: [{ from, to, out }], isochrone: [{ from, out }], selectLink: [{ key, out }] } } } }.
import { readFileSync, writeFileSync, readdirSync } from 'node:fs';
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

const only = process.argv[2];
for (const file of readdirSync(join(here, 'fixtures')).filter(f => f.endsWith('.json'))) {
  const name = file.replace(/\.json$/, '');
  if (only && only !== name) continue;
  const c = loadCore({ seed: 11 });
  const { Engine, Store } = c;
  const P = c.migrate(JSON.parse(readFileSync(join(here, 'fixtures', file), 'utf8')));
  Store.P = P;
  const out = { project: name, scenarios: {} };
  for (const sc of P.scenarios) {
    P.active = sc.id;
    sc.res = {};
    const rec = out.scenarios[sc.id] = {};
    const nodes = Object.keys(sc.nodes), zones = Object.values(sc.zones).map(z => 'z:' + z.id);
    const mid = nodes[nodes.length >> 1];
    const pairs = [[nodes[0], nodes[nodes.length - 1]], [mid, nodes[0]], [nodes[nodes.length - 1], mid], [nodes[1], nodes[nodes.length - 2]], [mid, mid],
      [nodes[0], zones[0]], [zones[0], zones[1]], [zones[zones.length - 1], nodes[2]], [nodes[0], 'nope'], ['nope', nodes[0]]];
    const sources = [nodes[0], mid, nodes[nodes.length - 1], zones[0], zones[zones.length - 1], 'nope'];
    for (const period of ['AM', 'PM']) {
      const res = await Engine.assign(sc, { period });
      sc.res[period] = res;
      const r = rec[period] = { travel: [], isochrone: [], selectLink: [] };
      for (const [from, to] of pairs) r.travel.push({ from, to, out: plain(Engine.travel(sc, res, from, to)) });
      for (const from of sources) r.isochrone.push({ from, out: plain(Engine.isochrone(sc, res, from)) });
      const busiest = Object.entries(res.arcs).sort((a, b) => (b[1].v || 0) - (a[1].v || 0));
      const keys = [...busiest.slice(0, 5).map(e => e[0]), busiest[busiest.length - 1][0], busiest[Math.floor(busiest.length / 2)][0], 'no such arc'];
      for (const key of keys) r.selectLink.push({ key, out: plain(Engine.selectLink(res, key)) });
    }
  }
  writeFileSync(join(here, 'expected', `routes-${name}.json`), JSON.stringify(out));
  console.log(name, 'routes written');
}
