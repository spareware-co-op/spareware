// Records the prototype's demand model on variants of the two truth-test projects, so the Rust port
// (projects/spare-construct/crates/construct-transport/src/demand.rs, tests/demand_truth.rs) can be checked on every branch:
// growth (linear and compound, before and after the base year), staging, opening years (zones and gateways), reductions,
// extra trips, trip-generation tables, fitted land-use factors, the manual OD matrix, matrix estimation by factors and by
// additive deltas, gateways missing or counts that do not balance, and the gravity parameter.
//
// Each case carries the project as text (Store.serialise, one scenario, so the key order the engine read is kept), the
// scenario and, for each period, what Engine.tripEnds gives for every zone, what Engine.demand gives (O, D, T, Z, note,
// manual, O0, D0, unrounded) and what Engine.applyME gives on seeded random matrices for several scenario years.
// Writes expected/demand-cases.json.
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
let seed = 77;
const rnd = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed / 4294967296; };

const cases = [];
// fixture: 'grid' or 'corridor'; mutate(P, sc, c) changes the project (sc is the scenario at index 0 unless scIndex says otherwise)
const make = (name, fixture, mutate = () => {}, scIndex = 0) => {
  const c = loadCore({ seed: 11 });
  const P = c.migrate(JSON.parse(readFileSync(join(here, 'fixtures', fixture + '.json'), 'utf8')));
  const sc = P.scenarios[scIndex];
  mutate(P, sc, c);
  P.scenarios = [sc];
  P.active = sc.id;
  c.Store.P = P;
  const { Engine } = c, S = P.settings;
  const k = { name, fixture, scenario: sc.id, projectText: c.Store.serialise(P), periods: {} };
  for (const period of ['AM', 'PM']) {
    const G = Engine.buildGraph(sc, S, period);
    const zones = Object.values(sc.zones);
    const rec = k.periods[period] = {
      tripEnds: Object.fromEntries(zones.map(z => [z.id, plain(Engine.tripEnds(z, S, period, sc))])),
      demand: plain(Engine.demand(sc, G, period, S)),
      applyME: [],
    };
    const Z = G.zones.length;
    for (const year of [null, Engine.yearOf(sc), S.baseYear, 2040]) {
      const T = new Float64Array(Z * Z);
      for (let i = 0; i < T.length; i++) T[i] = rnd() < 0.2 ? 0 : Math.round(rnd() * 4000) / 10;
      const T0 = Array.from(T), note = Engine.applyME(G, period, T, year);
      rec.applyME.push({ year, T0, T: plain(T), note });
    }
  }
  cases.push(k);
};
const zs = sc => sc.zones;
const gateways = sc => Object.values(sc.zones).filter(z => z.kind === 'gateway');
const ids = sc => Object.values(sc.zones).map(z => z.id);

make('grid-base', 'grid');
make('grid-base-s-opt', 'grid', () => {}, 2);
make('corridor-base', 'corridor');
make('grid-compound-growth', 'grid', (P, sc) => { P.settings.growthMode = 'compound'; P.settings.growth = 2.5; sc.year = 2034; });
make('grid-year-before-base', 'grid', (P, sc) => { sc.year = 2024; P.settings.growth = 4; });
make('grid-gravity-beta', 'grid', P => { P.settings.gravityBeta = 0.35; });
make('grid-staging', 'grid', (P, sc) => { sc.buildout = 40; zs(sc).zA.role = 'development'; zs(sc).zB.buildout = 75; zs(sc).zC.role = 'committed'; });
make('grid-staging-text', 'grid', (P, sc) => { sc.buildout = '55'; zs(sc).zA.role = 'development'; zs(sc).zB.buildout = ''; zs(sc).zC.buildout = 'x'; });
make('grid-opening-years', 'grid', (P, sc) => { sc.year = 2030; zs(sc).zB.year = 2032; zs(sc).zA.year = 2029; zs(sc).gE.year = 2031; });
make('grid-reduction', 'grid', (P, sc) => { zs(sc).zA.reduction = 30; zs(sc).zC.reduction = 150; zs(sc).zB.reduction = 'abc'; });
make('grid-extra-trips', 'grid', (P, sc) => { zs(sc).zA.extra = { AMin: 40, AMout: 25, PMin: 10, PMout: 60 }; zs(sc).zC.extra = { AMin: 'x', AMout: '12' }; zs(sc).zA.reduction = 10; });
make('grid-trip-generation', 'grid', (P, sc) => {
  zs(sc).zB.tg = { on: true }; zs(sc).zB.tgLoad = { AMin: 300, AMout: 260, PMin: 210, PMout: 330 }; zs(sc).zB.reduction = 10;
  zs(sc).zC.tg = { on: false }; zs(sc).zC.tgLoad = { AMin: 1, AMout: 2, PMin: 3, PMout: 4 };
  zs(sc).zA.tg = { on: true };
});
make('grid-lu-factors', 'grid', (P, sc) => {
  P.luFactors = { AM: { residential: 1.25, apartment: 0.8, retail: 'abc', gateways: 1.15, office: -2 }, PM: { gateways: 0.9, residential: 1.1, supermarket: 1.4 } };
  zs(sc).zA.existing = true; zs(sc).zB.existing = true; zs(sc).zC.existing = false; zs(sc).gN.gwSrc = 'count 2024';
});
make('grid-lu-factors-development', 'grid', (P, sc) => {
  P.luFactors = { AM: { residential: 2, retail: 0.5, gateways: 1.3 }, PM: null };
  zs(sc).zA.existing = true; zs(sc).zA.role = 'development'; zs(sc).zB.existing = true; zs(sc).zB.role = 'existing';
});
make('grid-manual-od', 'grid', (P, sc) => {
  const m = n => Array.from({ length: n }, () => Array.from({ length: n }, () => Math.round(rnd() * 300) / 10));
  sc.odManual = { AM: { zones: ['zA', 'zB', 'zC', 'gW', 'gE', 'gN'], m: m(6) }, PM: { zones: ['gE', 'zA', 'zX', 'gW', 'zC'], m: [[1, 2, 3, 4, 5], [6, 7, 8, 9], [10, 11, 12, 13, 14, 15], null, ['x', 3, null, 9, 8]] } };
  zs(sc).zA.year = 2040; zs(sc).zB.reduction = 20; sc.year = 2030;
});
make('grid-manual-od-null-period', 'grid', (P, sc) => { sc.odManual = { AM: null, PM: { zones: [], m: [] } }; });
make('grid-me-factors', 'grid', (P, sc) => {
  P.me = { AM: { 'zA>gW': 1.4, 'gW>zA': 0.7, 'zB>zC': 2, 'zA>zA': 3, bad: 2, 'gE>gN': -1, 'gN>gE': 'x', 'zA>gE': 0, 'gE>gW': 1.5, 'gW>gE': 0.5 }, PM: { 'zA>gW': 2 }, info: { AM: { baseYear: 2026, date: '2026-01-01' } } };
  zs(sc).zA.existing = true; zs(sc).zB.existing = true;
});
make('grid-me-delta', 'grid', (P, sc) => {
  P.me = { AM: { 'zA>gW': 1.1 }, PM: { 'zA>gW': 1.2 }, info: { AM: { baseYear: 2020, delta: { 'zA>gW': 25, 'gW>zA': -9999, 'gE>gN': 12.5, 'zB>zC': 3, bad: 1, 'zA>zA': 5, 'zB>gE': '', 'gN>gW': null, 'gW>gN': '4', 'gN>gE': 'abc' } }, PM: { baseYear: 2026, delta: { 'zA>gW': 5 } } } };
  zs(sc).zA.existing = true; zs(sc).zB.existing = true; sc.year = 2026;
});
make('grid-me-delta-empty', 'grid', (P, sc) => { P.me = { AM: { 'zA>gW': 1.1 }, info: { AM: { baseYear: 2020, delta: {} } } }; zs(sc).zA.existing = true; sc.year = 2030; });
make('grid-me-bad-shapes', 'grid', (P, sc) => { P.me = { AM: null, PM: { 'zA>gW': 2 }, info: { AM: { baseYear: 'x', delta: [1, 2] }, PM: { baseYear: 2026 } } }; zs(sc).zA.existing = true; });
make('grid-me-manual', 'grid', (P, sc) => {
  sc.odManual = { AM: { zones: ['zA', 'zB', 'zC', 'gW', 'gE', 'gN'], m: Array.from({ length: 6 }, () => Array.from({ length: 6 }, () => Math.round(rnd() * 300) / 10)) } };
  P.me = { AM: { 'zA>gW': 1.5, 'gW>gE': 0.4 }, info: { AM: { baseYear: 2026 } } }; zs(sc).zA.existing = true;
});
make('grid-no-gateways', 'grid', (P, sc) => { for (const g of gateways(sc)) delete sc.zones[g.id]; });
make('grid-lost-trips', 'grid', (P, sc) => { for (const g of gateways(sc)) delete sc.zones[g.id]; zs(sc).zB.lu = {}; zs(sc).zC.lu = {}; });
make('grid-single-zone', 'grid', (P, sc) => { for (const id of ids(sc)) if (id !== 'zA') delete sc.zones[id]; });
make('grid-over-gateway', 'grid', (P, sc) => { zs(sc).gW.gw = { AMin: 600, AMout: 5, PMin: 500, PMout: 6 }; zs(sc).gE.gw = { AMin: 380, AMout: 4, PMin: 540, PMout: 3 }; zs(sc).gN.gw = { AMin: 300, AMout: 3, PMin: 220, PMout: 2 }; zs(sc).zA.lu = { residential: 4000 }; });
make('grid-unbalanced-through', 'grid', (P, sc) => { zs(sc).gW.gw = { AMin: 3000, AMout: 450, PMin: 500, PMout: 650 }; zs(sc).gE.gw = { AMin: 380, AMout: 100, PMin: 540, PMout: 360 }; });
make('grid-no-trips-zone', 'grid', (P, sc) => { zs(sc).zA.lu = {}; zs(sc).zB.lu = { retail: 0 }; });
make('grid-one-gateway', 'grid', (P, sc) => { delete sc.zones.gE; delete sc.zones.gN; });
make('corridor-year-2040', 'corridor', (P, sc) => { sc.year = 2040; });
make('corridor-year-2026', 'corridor', (P, sc) => { sc.year = 2026; });
make('corridor-staging', 'corridor', (P, sc) => { sc.buildout = 50; });
make('corridor-lu-factors-me', 'corridor', (P, sc) => {
  P.luFactors = { AM: { residential: 1.1, school: 0.9, gateways: 1.2 }, PM: { residential: 0.95 } };
  P.me = { AM: { 'zEx>gW': 1.3, 'gE>zEx': 0.8, 'gW>gE': 1.1 }, info: { AM: { baseYear: 2026 } } };
});
make('corridor-me-delta', 'corridor', (P, sc) => {
  P.me = { AM: { 'zEx>gW': 1.3 }, info: { AM: { baseYear: 2026, delta: { 'zEx>gW': 30, 'gE>zEx': -15, 'gW>gE': 20 } } } };
});

writeFileSync(join(here, 'expected', 'demand-cases.json'), JSON.stringify(cases));
console.log('demand-cases written:', cases.length, 'cases');
