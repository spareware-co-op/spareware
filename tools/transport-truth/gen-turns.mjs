// Records the prototype's junction outputs (turns.js `Turns`) for the Rust port's turns module
// (projects/spare-construct/crates/construct-transport/src/turns, tests/turns_truth.rs). Writes expected/turns-*.json:
//   turns-functions.json  sideName, approachName, classify, assignLegs, legCodeOf, turnCode, movementId, legLabel, rank over grids
//                         and random inputs (including more than eight legs)
//   turns-<project>.json  for every scenario and period of the saved test projects, with the engine's recorded results: each node's
//                         arms, movement rows, name, empty text, turn-count sheet and diagrams; each junction site's rows and legs; the
//                         SIDRA CSV in every form (default, listed junctions, codes, all, both periods, extent sites) and the export gate
//   turns-synthetic.json  made-up junctions that reach every branch the saved projects do not: more than eight legs, equal names, ramps,
//                         one-way arms, closed roads, zone connectors, stale and malformed turn rows, other settings for heavy vehicles
// Results are the ones in expected/<project>.json (written by gen-expected.mjs); non-finite numbers there are strings, converted back
// to numbers for the prototype and read back by the Rust test from the same file.
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCore } from './js-core.mjs';
import { plain, clone } from './edit-ops.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const read = rel => JSON.parse(readFileSync(join(here, rel), 'utf8'));
const write = (name, data) => { const t = JSON.stringify(data); writeFileSync(join(here, 'expected', name), t); console.log(name, t.length, 'bytes'); };
const revive = v => (v === 'NaN' ? NaN : v === 'Infinity' ? Infinity : v === '-Infinity' ? -Infinity : Array.isArray(v) ? v.map(revive) : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, revive(x)])) : v);

const core = loadCore({ seed: 1 });
const { Turns, Net, Store, U, migrate, newProject, Engine } = core;

let seed = 20261021;
const rnd = () => { seed = (seed + 0x6D2B79F5) >>> 0; let t = seed; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
const uni = (a, b) => a + (b - a) * rnd();
const pick = a => a[Math.floor(rnd() * a.length)];

const armJson = a => plain({ id: a.id, linkId: a.link ? a.link.id : null, conn: a.conn, road: a.road, b: a.b, ...(a.k === undefined ? { inKey: a.inKey, outKey: a.outKey } : {}), isIn: a.isIn, isOut: a.isOut, side: a.side, name: a.name,
  ...(a.k !== undefined ? { k: a.k, ins: a.ins, outs: a.outs, o: a.o, leg: a.leg, legName: a.legName, legLabel: a.legLabel } : {}) });

/* ---------- the pure functions ---------- */
{
  const f = {};
  const bearings = [...Array.from({ length: 73 }, (_, i) => i * 5 - 360), 22.5, 67.5, 112.5, 157.5, 202.5, 247.5, 292.5, 337.5, 44.999, 45, 45.001, 359.999, 360, 719.5, -719.5, 0.5, -0.5, 22.4999, 22.5001, 1e6 + 0.3, ...Array.from({ length: 300 }, () => uni(-900, 900))];
  f.sideName = bearings.flatMap(b => [false, true].map(e => [b, e, Turns.sideName(b, e)]));
  f.approachName = bearings.flatMap(b => [false, true].map(e => [b, e, Turns.approachName(b, e)]));
  f.legCodeOf = bearings.map(b => [b, Turns.legCodeOf(b)]);
  f.classify = Array.from({ length: 600 }, () => { const a = uni(0, 360), b = pick([a + uni(-40, 40), a + uni(-200, 200), uni(0, 360), a + 30, a - 30, a + 150, a - 150, a + 180]); return [a, b, Turns.classify(a, b)]; });
  const legs = [];
  for (let n = 0; n <= 16; n++) for (let rep = 0; rep < 12; rep++) {
    const bs = Array.from({ length: n }, () => pick([uni(0, 360), Math.round(uni(0, 8)) * 45, Math.round(uni(0, 8)) * 45 + uni(-5, 5)]));
    const prio = Array.from({ length: Math.max(0, n - Math.floor(rnd() * 3)) }, () => pick([0, 1, 2, 3, 4, 5, 6, 1, 2]));
    legs.push({ bearings: bs, prio, out: Turns.assignLegs(bs, prio) });
  }
  legs.push({ bearings: [10, 10, 10], prio: [], out: Turns.assignLegs([10, 10, 10], []) });
  legs.push({ bearings: [0, 45, 90, 135, 180, 225, 270, 315, 20, 200], prio: [5, 5, 5, 5, 5, 5, 5, 5, 1, 0], out: Turns.assignLegs([0, 45, 90, 135, 180, 225, 270, 315, 20, 200], [5, 5, 5, 5, 5, 5, 5, 5, 1, 0]) });
  f.assignLegs = plain(legs);
  f.turnCode = []; f.movementId = []; f.legLabel = [];
  for (let i = 0; i < 8; i++) for (let j = 0; j < 8; j++) { f.turnCode.push([i, j, Turns.turnCode(i, j)]); f.movementId.push([i, j, Turns.movementId(i, j)]); }
  for (let o = 0; o < 8; o++) for (const road of ['', 'Great South Road', 'Unnamed ramp']) f.legLabel.push([o, road, Turns.legLabel(o, road)]);
  f.rank = []; for (const mv of ['L', 'T', 'R', 'U', 'X']) for (const major of [true, false]) f.rank.push([mv, major, Turns.rank(mv, major)]);
  f.constants = { LEG_CODES: Turns.LEG_CODES, LEG_NAMES: Turns.LEG_NAMES, TURN_CODES: Turns.TURN_CODES, SIDRA_HEAD: Turns.SIDRA_HEAD, CODE_HEAD: Turns.CODE_HEAD };
  write('turns-functions.json', plain(f));
}

/* ---------- one scenario, one period ---------- */
// Everything the Turns functions give for a scenario and a result
function recordCase(P, sc, res, extra = {}, only = null) {
  Store.P = P;
  const out = { scenario: sc.id, period: res.period, nodes: {}, sites: {}, csv: {} };
  const turnList = id => (res && res.turns && res.turns[id]) || [];
  for (const id of only || Object.keys(sc.nodes)) {
    const has = Array.isArray(res.turns && res.turns[id]);
    const rec = { has, arms: Turns.arms(sc, id, turnList(id)).map(armJson), armsNoTurns: Turns.arms(sc, id, []).map(armJson), rows: plain(Turns.movements(sc, res, id)), name: Turns.junctionName(sc, id), empty: Turns.emptyText(sc, res, id), count: Turns.turnCountCSV(sc, res, id) };
    rec.diagram = { total: Turns.diagram(sc, res, id) };
    if (has) {
      if (!only || rec.name.length % 3 === 0) rec.diagram.base = Turns.diagram(sc, res, id, { layer: 'base', size: 300 });
      if (!only) {
        rec.diagram.dev = Turns.diagram(sc, res, id, { layer: 'dev', size: 180.5, idSuffix: '-x 1<2>' });
        rec.diagram.small = Turns.diagram(sc, res, id, { size: 60 });
      }
    }
    out.nodes[id] = rec;
  }
  out.nodes.missing = { arms: Turns.arms(sc, 'missing', []).map(armJson), rows: plain(Turns.movements(sc, res, 'missing')), name: Turns.junctionName(sc, 'missing'), empty: Turns.emptyText(sc, res, 'missing'), count: Turns.turnCountCSV(sc, res, 'missing'), diagram: { total: Turns.diagram(sc, res, 'missing') } };
  for (const sid of Object.keys(res.sites || {})) {
    const sr = Turns.siteRows(sc, res, sid);
    out.sites[sid] = { rows: plain(sr.rows), arms: sr.A.map(armJson), movements: plain(Turns.movements(sc, res, sid)), name: res.sites[sid].name };
  }
  const ids = Object.keys(res.turns || {});
  const sites = Object.keys(res.sites || {});
  const some = [...ids.slice(0, 2), ...sites, 'nope'];
  out.csv = {
    default: Turns.sidraCSV(sc, res), codes: Turns.sidraCSV(sc, res, undefined, { codes: true }), all: Turns.sidraCSV(sc, res, undefined, { all: true, codes: true }),
    some: Turns.sidraCSV(sc, res, some), someCodes: Turns.sidraCSV(sc, res, some, { codes: true }), empty: Turns.sidraCSV(sc, res, []), arrayOne: Turns.sidraCSV(sc, [res]),
    nullScenario: Turns.sidraCSV(null, res), noRes: Turns.sidraCSV(sc, null), noTurns: Turns.sidraCSV(sc, { period: 'AM' }),
    gate: plain(Turns.sidraExport(sc, res)), gateCodes: plain(Turns.sidraExport(sc, res, undefined, { codes: true })), gateNone: plain(Turns.sidraExport(sc, [])),
  };
  // The extent test's sites: only those are exported
  const withExtent = clone(sc);
  withExtent.extent = { sites: sites.length ? sites : ids.slice(0, 1) };
  out.csv.extent = Turns.sidraCSV(withExtent, res);
  out.csv.extentAll = Turns.sidraCSV(withExtent, res, undefined, { all: true });
  out.extentSites = withExtent.extent.sites;
  // Not ready: a loose gap, and out of date
  const loose = { ...clone(res), gap: 0.005 }, stale = { ...clone(res), stale: true };
  out.csv.gateLoose = plain(Turns.sidraExport(sc, loose)); out.csv.gateLooseForce = plain(Turns.sidraExport(sc, loose, undefined, { force: true, codes: true }));
  out.csv.gateStale = plain(Turns.sidraExport(sc, stale)); out.csv.gateStaleForce = plain(Turns.sidraExport(sc, stale, undefined, { force: true }));
  out.csv.gateStaleEmpty = Turns.emptyText(sc, stale, ids[0] || 'x');
  out.looseGap = 0.005;
  if (only) for (const k of ['all', 'someCodes', 'empty', 'arrayOne', 'nullScenario', 'noRes', 'noTurns', 'extentAll', 'gateCodes', 'gateNone', 'gateLooseForce', 'gateStaleForce', 'gateStaleEmpty']) delete out.csv[k];
  Object.assign(out, extra);
  return plain(out);
}

/* ---------- the saved projects, with their recorded results ---------- */
for (const name of ['corridor', 'grid']) {
  const P = migrate(read(`inputs/${name}.json`));
  const ex = read(`expected/${name}.json`);
  const cases = [];
  for (const sc of P.scenarios) {
    const periods = Object.keys(ex.scenarios[sc.id].periods);
    const results = {};
    for (const period of periods) {
      const res = revive(ex.scenarios[sc.id].periods[period].res);
      results[period] = res;
      cases.push(recordCase(P, sc, res));
    }
    // Both periods in one export, with and without codes
    if (periods.length > 1) {
      Store.P = P;
      const both = periods.map(p => results[p]);
      cases.push({ scenario: sc.id, period: 'both', csv: plain({ both: Turns.sidraCSV(sc, both), bothCodes: Turns.sidraCSV(sc, both, undefined, { codes: true }), bothGate: Turns.sidraExport(sc, both), bothLoose: Turns.sidraExport(sc, [both[0], { ...clone(both[1]), gap: 0.5 }]), bothForce: Turns.sidraExport(sc, [both[0], { ...clone(both[1]), gap: 0.5, period: '' }], undefined, { force: true }) }) });
    }
  }
  write(`turns-${name}.json`, { cases });
}

/* ---------- made-up junctions ---------- */
{
  const out = { cases: [] };
  const KM = 111320, KY = 110574;
  const O = [-36.9, 174.75];
  const at = (dx, dy) => [O[0] + dy / KY, O[1] + dx / (KM * Math.cos(O[0] * Math.PI / 180))];
  const bearingToXY = (b, r) => [r * Math.sin(b * Math.PI / 180), r * Math.cos(b * Math.PI / 180)];
  // A star junction: arms at the given bearings with the given class, name, lanes
  const star = (name, arms, { settings = {}, zones = [], period = 'AM', turnsFor = null, extra = {} } = {}) => {
    const P = newProject('Turns truth: ' + name);
    Object.assign(P.settings, settings);
    const sc = P.scenarios[0];
    const c = Net.addNode(sc, O, { control: 'priority' });
    const ends = [];
    arms.forEach((a, i) => {
      const [dx, dy] = bearingToXY(a.b, a.len ?? 150);
      const e = Net.addNode(sc, at(dx, dy));
      const bend = a.bend ? [at(...bearingToXY(a.b + a.bend, (a.len ?? 150) / 2))] : [];
      const id = a.out ? Net.addLink(sc, e, c, bend.reverse(), { cls: a.cls || 'local', name: a.name || '', lanesAB: a.lanesIn ?? 1, lanesBA: a.lanesOut ?? 1, status: a.status || 'existing' })
        : Net.addLink(sc, c, e, bend, { cls: a.cls || 'local', name: a.name || '', lanesAB: a.lanesOut ?? 1, lanesBA: a.lanesIn ?? 1, status: a.status || 'existing' });
      ends.push({ id, e, flip: !!a.out });
    });
    for (const z of zones) {
      const [dx, dy] = bearingToXY(z.b, 60);
      const zid = z.id;
      sc.zones[zid] = { id: zid, kind: 'zone', name: z.name, lat: at(dx, dy)[0], lng: at(dx, dy)[1], lu: { residential: 50 }, node: c, connectors: null };
    }
    // Turning movements: every approach to every other exit, volumes from a ramp of numbers
    const L = ends.map((x, i) => ({ ...x, i, a: arms[i] }));
    const key = (x, dir) => x.id + ':' + (dir === 'in' ? (x.flip ? 'ab' : 'ba') : (x.flip ? 'ba' : 'ab'));
    const list = [];
    let v = 40;
    for (const x of L) for (const y of L) {
      if (x === y) continue;
      v = (v * 7 + 13) % 211;
      const mv = pick(['L', 'T', 'R', 'U']);
      const row = { from: key(x, 'in'), to: key(y, 'out'), mv, v: Math.round(v * 10) / 10, pcu: Math.round(v * 1.08 * 10) / 10 };
      if (v % 3 === 0) row.dev = Math.round(v / 3 * 10) / 10;
      list.push(row);
    }
    for (const z of zones) { list.push({ from: `c:${z.id}:${c}:o`, to: key(L[0], 'out'), mv: 'T', v: 25, pcu: 27 }); list.push({ from: key(L[0], 'in'), to: `c:${z.id}:${c}:d`, mv: 'T', v: 15, pcu: 16 }); }
    const turns = { [c]: turnsFor ? turnsFor(list, L) : list };
    const res = { period, stale: false, gap: 0.00005, turns, ...extra };
    return { P, sc, c, res, arms: L };
  };
  const cases = [
    ['cross', [{ b: 0, cls: 'arterial', name: 'Great South Road', lanesOut: 2, lanesIn: 2 }, { b: 90, cls: 'collector', name: 'Waihoehoe Road' }, { b: 180, cls: 'arterial', name: 'Great South Road', lanesOut: 2, lanesIn: 2 }, { b: 270, cls: 'local', name: 'Elliot Street' }]],
    ['T', [{ b: 270, cls: 'arterial', name: 'Main Road' }, { b: 90, cls: 'arterial', name: 'Main Road' }, { b: 190, cls: 'local', name: 'Side Street' }]],
    ['two ramps same side', [{ b: 10, cls: 'ramp', lanesOut: 1, lanesIn: 1 }, { b: 22, cls: 'ramp' }, { b: 180, cls: 'arterial', name: 'Highway' }]],
    ['same quadrant named', [{ b: 5, cls: 'collector', name: 'North Road' }, { b: 35, cls: 'collector', name: 'Northeast Road' }, { b: 200, cls: 'local', name: 'South Road' }]],
    ['same quadrant same road', [{ b: 5, cls: 'collector', name: 'Twin Road' }, { b: 35, cls: 'collector', name: 'Twin Road' }, { b: 200, cls: 'local', name: 'South Road' }, { b: 300, cls: 'local' }]],
    ['one way arms', [{ b: 0, cls: 'collector', name: 'One Way In', lanesIn: 2, lanesOut: 0 }, { b: 120, cls: 'collector', name: 'One Way Out', lanesIn: 0, lanesOut: 2 }, { b: 240, cls: 'arterial', name: 'Two Way', lanesIn: 2, lanesOut: 2 }]],
    ['ten legs', Array.from({ length: 10 }, (_, i) => ({ b: i * 36 + 4, cls: i === 0 ? 'arterial' : i % 3 === 0 ? 'collector' : 'local', name: i % 2 ? `Street ${i}` : '' }))],
    ['nine legs ramps', [...Array.from({ length: 8 }, (_, i) => ({ b: i * 45, cls: 'ramp' })), { b: 20, cls: 'local', name: 'Driveway' }]],
    ['closed and zone', [{ b: 0, cls: 'collector', name: 'Open' }, { b: 90, cls: 'collector', name: 'Closed', status: 'closed' }, { b: 200, cls: 'local', name: 'Back' }], { zones: [{ id: 'zN', b: 330, name: 'Estate' }, { id: 'zE', b: 100, name: '' }] }],
    ['curved arms', [{ b: 0, cls: 'collector', name: 'Bend', bend: 40, len: 300 }, { b: 180, cls: 'collector', name: 'Bend', bend: -50, len: 200, out: true }, { b: 90, cls: 'local', name: 'Short', len: 8 }]],
    ['heavy settings 1', [{ b: 0, cls: 'collector', name: 'A' }, { b: 120, cls: 'collector', name: 'B' }, { b: 240, cls: 'collector', name: 'C' }], { settings: { pcuHV: 1, hvPct: 8 } }],
    ['heavy settings 2', [{ b: 0, cls: 'collector', name: 'A' }, { b: 120, cls: 'collector', name: 'B' }, { b: 240, cls: 'collector', name: 'C' }], { settings: { pcuHV: 3, hvPct: 0 } }],
    ['heavy settings 3', [{ b: 0, cls: 'collector', name: 'A' }, { b: 120, cls: 'collector', name: 'B' }, { b: 240, cls: 'collector', name: 'C' }], { settings: { pcuHV: '2.5', hvPct: 'x' } }],
    ['odd rows', [{ b: 0, cls: 'collector', name: 'A' }, { b: 120, cls: 'collector', name: 'B' }, { b: 240, cls: 'collector', name: 'C' }], { turnsFor: (list, L) => {
      const mixed = list.map((r, i) => (i % 5 === 0 ? { ...r, mv: 'X' } : i % 7 === 0 ? { ...r, pcu: NaN } : i % 4 === 0 ? { ...r, v: -5 } : i % 6 === 0 ? { ...r, dev: 1e6 } : i % 11 === 0 ? { ...r, dev: -3, v: NaN } : r));
      return [...mixed, { from: 'gone:ab', to: 'gone:ba', mv: 'T', v: 5, pcu: 5 }, { from: list[0].from, to: 'gone:ba', mv: 'T', v: 5, pcu: 5 }, { from: 'c:nozone:x:o', to: list[0].to, mv: 'T', v: 5, pcu: 5 }];
    } }],
    ['stale', [{ b: 0, cls: 'collector', name: 'A' }, { b: 120, cls: 'collector', name: 'B' }], { extra: { stale: true } }],
    ['no turns', [{ b: 0, cls: 'collector', name: 'A' }, { b: 120, cls: 'collector', name: 'B' }, { b: 240, cls: 'collector', name: 'C' }], { turnsFor: () => [] }],
    ['two arms', [{ b: 0, cls: 'collector', name: 'A' }, { b: 180, cls: 'collector', name: 'A' }]],
    ['unnamed all', [{ b: 0, cls: 'motorway' }, { b: 90, cls: 'arterial' }, { b: 180, cls: 'collector' }, { b: 270, cls: 'local' }]],
    ['PM named marks', [{ b: 0, cls: 'arterial', name: 'Māngere Road' }, { b: 90, cls: 'arterial', name: 'Mangere Bridge' }, { b: 180, cls: 'arterial', name: 'mangere road' }, { b: 270, cls: 'arterial', name: 'Z Road' }], { period: 'PM' }],
  ];
  for (const [name, arms, opts] of cases) {
    const s = star(name, arms, opts || {});
    // Reload through migrate so the project is what a saved file gives
    const rec = recordCase(s.P, s.sc, s.res, { name, project: JSON.parse(JSON.stringify(s.P)), res: plain(s.res), node: s.c });
    out.cases.push(rec);
  }
  // Random junctions: two to twelve arms at random bearings, with random classes, names, lanes, one-way arms, closed roads and zones
  {
    let st = 424242;
    const r = () => { st = (st + 0x6D2B79F5) >>> 0; let t = st; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
    const u = (a, b) => a + (b - a) * r(), p = a => a[Math.floor(r() * a.length)];
    const NAMES = ['Great South Road', 'Station Road', 'Elliot Street', '', '', 'Hunter Street', 'Māngere Road', 'station road'];
    for (let k = 0; k < 24; k++) {
      const n = 2 + Math.floor(r() * 11);
      const base = r() < 0.4 ? u(0, 360) : 0;
      const arms = Array.from({ length: n }, (_, i) => {
        const b = r() < 0.35 ? (base + p([0, 45, 90, 135, 180, 225, 270, 315]) + u(-12, 12)) % 360 : u(0, 360);
        const oneWay = r() < 0.2;
        return { b: b < 0 ? b + 360 : b, cls: p(['motorway', 'ramp', 'arterial', 'arterial', 'collector', 'collector', 'local', 'local']), name: p(NAMES), len: u(30, 220),
          ...(oneWay ? (r() < 0.5 ? { lanesIn: 2, lanesOut: 0 } : { lanesIn: 0, lanesOut: 2 }) : { lanesIn: 1 + Math.floor(r() * 3), lanesOut: 1 + Math.floor(r() * 3) }),
          ...(r() < 0.1 ? { status: 'closed' } : {}), ...(r() < 0.2 ? { bend: u(-60, 60) } : {}), ...(r() < 0.3 ? { out: true } : {}) };
      });
      const zones = r() < 0.25 ? [{ id: 'zA', b: u(0, 360), name: p(['Estate', '', 'Mall']) }, ...(r() < 0.4 ? [{ id: 'zB', b: u(0, 360), name: 'Depot' }] : [])] : [];
      const s = star('random ' + k, arms, { zones, period: p(['AM', 'PM']), settings: r() < 0.3 ? { hvPct: p([0, 4, 12]), pcuHV: p([1, 2.3, 3]) } : {}, extra: r() < 0.1 ? { stale: true } : {} });
      out.cases.push(recordCase(s.P, s.sc, s.res, { name: 'random ' + k, project: JSON.parse(JSON.stringify(s.P)), res: plain(s.res), node: s.c }, [s.c]));
    }
  }
  // A junction with a different name set on the node and no links at all
  {
    const s = star('node name', [{ b: 0, cls: 'collector', name: 'A' }, { b: 180, cls: 'collector', name: 'A' }, { b: 90, cls: 'local', name: 'B' }]);
    s.sc.nodes[s.c].name = 'Town Centre';
    out.cases.push(recordCase(s.P, s.sc, s.res, { name: 'node name', project: JSON.parse(JSON.stringify(s.P)), res: plain(s.res), node: s.c }));
  }
  write('turns-synthetic.json', out);
}
