// Runs the prototype's helpers (util.js U, the JavaScript number rules the engine relies on, and the model.js pieces the
// Rust port types) on a fixed set of inputs and records the answers in expected/util-cases.json. The Rust port's
// tests (projects/spare-construct/crates/construct-transport/tests/util_cases.rs and project_roundtrip.rs) check every case.
// Numbers that JSON cannot hold are written as strings: "NaN", "Infinity", "-Infinity" and "-0".
import { writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCore } from './js-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));
mkdirSync(join(here, 'expected'), { recursive: true });
const c = loadCore({ seed: 1 });
const { U, Net, Scenarios } = c;
// Register is a top-level const of model.js that js-core.mjs does not export: a function made by that realm's own
// Function constructor sees its global declarations
const Register = U.dist.constructor('return Register')();

// Seeded random numbers, so the file is the same on every run
let seed = 20261021;
const rnd = () => { seed = (seed + 0x6D2B79F5) >>> 0; let t = seed; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
const uni = (a, b) => a + (b - a) * rnd();
const pick = a => a[Math.floor(rnd() * a.length)];
const enc = x => (typeof x !== 'number' ? x : Object.is(x, -0) ? '-0' : Number.isFinite(x) ? x : String(x));
const encDeep = v => (typeof v === 'number' ? enc(v) : Array.isArray(v) ? v.map(encDeep) : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, encDeep(x)])) : v);

// Numbers of every kind: rounding edges, signs, tiny and huge, non-finite, and random ones at many scales
const EDGES = [0, -0, 0.5, 1.5, 2.5, -0.5, -1.5, -2.5, 0.49999999999999994, -0.49999999999999994, 0.5000000000000001, 2.4999999999999996,
  4503599627370497, -4503599627370497, 9007199254740993, 1e21, -1e21, 1e300, -1e300, 5e-324, -5e-324, 2.2250738585072014e-308, 1.7976931348623157e308,
  1.005, 1.255, 8.345, 0.05, 0.15, 0.25, 0.35, -0.05, -0.15, 1.45, 2.675, 999.995, 9.95, 99.95, 0.1, 0.2, 0.30000000000000004, 1 / 3, 2 / 3,
  123.456, -123.456, 1234.5, -1234.5, 999.5, 999999.5, 1e-7, 1.23e-7, 0.000001, 0.0000001, 1e20, 1.23e20, 123456789012345680000, 2 ** 31, 2 ** 32,
  -(2 ** 31), 2 ** 53, 174.7633, -36.8485, 174.76330000000002, NaN, Infinity, -Infinity,
  // exact ties between two shortest digit strings: JavaScript takes the even one
  1642031110231315.25, 1234567890123456.25, 1234567890123456.75, 1125899906842624.75, 562949953421312.125, 281474976710656.375, -1642031110231315.25];
const randoms = n => Array.from({ length: n }, () => {
  const k = rnd();
  if (k < 0.25) return uni(-10, 10);
  if (k < 0.5) return uni(-1e6, 1e6);
  if (k < 0.65) return Math.round(uni(-1e4, 1e4)) / pick([1, 2, 4, 8, 10, 20, 100, 1000]);
  if (k < 0.8) return Math.exp(uni(-60, 60)) * (rnd() < 0.5 ? -1 : 1);
  return Math.exp(uni(-700, 700));
});
const NUMS = [...EDGES, ...randoms(250)];
const FINITE = NUMS.filter(Number.isFinite);

const out = { note: 'Written by gen-util-cases.mjs; numbers JSON cannot hold are strings ("NaN", "Infinity", "-Infinity", "-0").' };

/* ---------- JavaScript number rules ---------- */
const STRINGS = ['', ' ', '  12  ', '12px', '0x1F', '0X1f', '0o17', '0O17', '0b101', '0B11', '-0x10', '+0x10', '0x', '0b', '0o', '0x1G', '0xFFFFFFFFFFFFFFFFFF',
  '1e3', '1E-3', '1e+3', '.5', '5.', '.', '+.5', '-.5', '-', '+', '+-1', 'Infinity', '-Infinity', '+Infinity', 'infinity', 'INFINITY', 'inf', 'NaN', 'nan',
  '1_000', '1,000', '\t\n 42  ', '﻿7', '  3  ', '\u0085 3', '　-8　', '1e', '1e+', 'e5', '0.1', '-0', '+0', '00012', '012.50',
  '1.7976931348623157e308', '1.7976931348623159e308', '1e400', '-1e400', '5e-324', '2e-324', '3e-324', '123456789012345678901234567890', '0.30000000000000004',
  ' - 5', '12 34', '١٢', '2036', ' 2036 ', 'true', 'null', '1.5.2', '9007199254740993', '0.1e-1', '1.e2', '.e2'];
out.stringToNumber = STRINGS.map(s => [s, enc(Number(s))]);
const VALUES = [null, true, false, [], [5], ['7'], [1, 2], {}, [null], [[3]], [[1, 2]], ['  '], '3', '', ' 4.5 ', 4.5, -0.25, 1e21, 'abc', { a: 1 }];
out.valueToNumber = VALUES.map(v => [v, enc(Number(v))]);
out.valueToString = VALUES.map(v => [v, String(v)]);
out.truthy = VALUES.map(v => [v, !!v]);
out.toInt32 = NUMS.map(x => [enc(x), enc(x | 0)]);
out.toUint32 = NUMS.map(x => [enc(x), enc(x >>> 0)]);
out.round = NUMS.map(x => [enc(x), enc(Math.round(x))]);
out.numberToString = NUMS.map(x => [enc(x), String(x)]);
const digitsFor = () => [0, 1, 2, 3, pick([4, 5, 6, 7, 10, 15, 20])];
out.toFixed = [...NUMS.flatMap(x => digitsFor().map(d => [enc(x), d, x.toFixed(d)])),
  ...[[0.1, 20], [1e-10, 5], [123.456, 10], [5e-324, 100], [1.5, 100], [2 ** 70, 2], [999999999999999999999, 0]].map(([x, d]) => [enc(x), d, x.toFixed(d)])];
out.toPrecision = [...NUMS.flatMap(x => [1, 2, 3, 4, pick([5, 6, 8, 12, 17, 21])].map(p => [enc(x), p, x.toPrecision(p)])),
  ...[[0.1, 21], [5e-324, 100], [123456, 6], [123456, 5], [1e21, 22], [99.95, 3], [0.00001, 1], [1e-7, 3]].map(([x, p]) => [enc(x), p, x.toPrecision(p)])];
const intl = (x, d) => new Intl.NumberFormat('en-NZ', { minimumFractionDigits: d, maximumFractionDigits: d }).format(x);
out.intl = FINITE.flatMap(x => [0, 1, 2, pick([3, 4, 6, 10, 20])].map(d => [enc(x), d, intl(x, d)]));
const PAIRS = [[3, 4], [0, 0], [-0, 0], [0, -0], [-0, -0], [NaN, 1], [1, NaN], [Infinity, NaN], [NaN, -Infinity], [1e300, 1e300], [1e-300, 1e-300], [5e-324, 0],
  [-3, 4], [1e308, 1e308], [2, -2], ...Array.from({ length: 200 }, () => [pick(NUMS), pick(NUMS)])];
out.hypot = [...PAIRS.map(a => [a.map(enc), enc(Math.hypot(...a))]), ...[[], [5], [1, 2, 3], [3, 4, 12], [-0], [1e-200, 1e-200, 1e-200]].map(a => [a.map(enc), enc(Math.hypot(...a))])];
out.minMax = PAIRS.map(([a, b]) => [enc(a), enc(b), enc(Math.min(a, b)), enc(Math.max(a, b))]);

/* ---------- util.js ---------- */
const ll = () => {
  const k = rnd();
  if (k < 0.6) return [uni(-37.2, -36.7), uni(174.6, 175.1)];   // around Auckland, where the projects are
  if (k < 0.9) return [uni(-90, 90), uni(-180, 180)];
  return pick([[0, 0], [90, 0], [-90, 0], [0, 180], [0, -180], [-36.8485, 174.7633]]);
};
const LLPAIRS = [...Array.from({ length: 300 }, () => [ll(), ll()]), ...Array.from({ length: 100 }, () => { const a = ll(); return [a, [a[0] + uni(-1e-3, 1e-3), a[1] + uni(-1e-3, 1e-3)]]; }),
  [[0, 0], [0, 180]], [[10, 20], [-10, -160]], [[-36.8485, 174.7633], [-36.8485, 174.7633]], [[90, 0], [-90, 0]]];
out.dist = LLPAIRS.map(([a, b]) => [a, b, enc(U.dist(a, b))]);
out.bearing = LLPAIRS.map(([a, b]) => [a, b, enc(U.bearing(a, b))]);
const line = () => {
  const n = 1 + Math.floor(rnd() * 8), p = [ll()];
  for (let i = 1; i < n; i++) { const q = p[i - 1]; p.push(rnd() < 0.1 ? q.slice() : [q[0] + uni(-0.003, 0.003), q[1] + uni(-0.003, 0.003)]); }
  return p;
};
const LINES = Array.from({ length: 150 }, line);
out.cumLen = LINES.map(p => [p, U.cumLen(p).map(enc), enc(U.polyLen(p))]);
out.along = LINES.flatMap(p => {
  const cum = U.cumLen(p), L = cum[cum.length - 1];
  // a NaN distance on a one-point line throws in the JavaScript (it reads a second point that is not there)
  const at = [-5, 0, L * rnd(), L * rnd(), cum[Math.floor(rnd() * cum.length)], L, L + 3, NaN].filter(s => !(p.length === 1 && Number.isNaN(s)))
    .map(s => { const r = U.along(p, cum, s); return [enc(s), r.p.map(enc), r.i, enc(r.f)]; });
  // the JavaScript's own cumulative lengths go with the line, so along is checked on its own
  return [[p, cum.map(enc), at]];
});
const ANGLES = [0, 180, -180, 360, 540, -540, 90, 270, 179.99999999999997, 180.00000000000003, NaN, ...Array.from({ length: 200 }, () => uni(-720, 720))];
out.angDiff = Array.from({ length: 300 }, () => [pick(ANGLES), pick(ANGLES)]).map(([a, b]) => [enc(a), enc(b), enc(U.angDiff(a, b))]);
out.turn = Array.from({ length: 300 }, () => [pick(ANGLES), pick(ANGLES)]).map(([a, b]) => [enc(a), enc(b), enc(U.turn(a, b))]);
out.flat = [...Array.from({ length: 100 }, () => uni(-90, 90)), 0, 90, -90, -36.8485, -37.1].map(lat0 => {
  const F = U.flat(lat0), p = ll();
  return [enc(lat0), enc(F.kx), enc(F.ky), p, enc(F.x(p)), enc(F.y(p))];
});
const segCase = () => {
  const s = pick([1, 100, 1e5, 1e7]), a = [uni(-s, s), uni(-s, s)], b = rnd() < 0.1 ? a.slice() : [uni(-s, s), uni(-s, s)], p = [uni(-s, s), uni(-s, s)];
  return [...p, ...a, ...b];
};
out.segDist = Array.from({ length: 300 }, segCase).map(a => { const r = U.segDist(...a); return [a.map(enc), enc(r.d), enc(r.t), enc(r.x), enc(r.y)]; });
out.clamp = Array.from({ length: 200 }, () => [pick(NUMS), pick([-1, 0, 5, -Infinity]), pick([1, 5, 100, Infinity])]).map(([x, a, b]) => [enc(x), enc(a), enc(b), enc(U.clamp(x, a, b))]);
const RVALS = [...NUMS, ...Array.from({ length: 300 }, () => Math.round(uni(-1e5, 1e5)) / pick([2, 20, 200, 2000, 2e4, 2e7])), ...Array.from({ length: 200 }, () => uni(-180, 180))];
out.r7 = RVALS.map(x => [enc(x), enc(U.r7(x))]);
out.r1 = RVALS.map(x => [enc(x), enc(U.r1(x))]);
out.r3 = RVALS.map(x => [enc(x), enc(U.r3(x))]);
// Heap: pushes (many equal keys) and pops in a random order; every pop's value and top
out.heap = Array.from({ length: 8 }, () => {
  const h = new U.Heap(4), ops = [], pops = [];
  for (let i = 0; i < 300; i++) {
    if (h.n && rnd() < 0.4) { const v = h.pop(); ops.push(['pop']); pops.push([v, enc(h.top)]); }
    else { const k = rnd() < 0.5 ? Math.floor(rnd() * 8) : uni(0, 50), v = Math.floor(rnd() * 1000); h.push(k, v); ops.push(['push', enc(k), v]); }
  }
  while (h.n) { const v = h.pop(); ops.push(['pop']); pops.push([v, enc(h.top)]); }
  return { ops, pops };
});
const F = U.fmt, FMTV = [...NUMS, ...Array.from({ length: 150 }, () => uni(-2e5, 2e5)), ...Array.from({ length: 80 }, () => uni(-2, 2)), 89.5, 90, 5399, 5400, 999.5, 999.4999, 1000, 0.0999, 0.1, 9.99, 10, 999.999, 0.00049, 86399.9, 86400, 90061, -5];
// One row per value: [x, num(x, 0..2), int, pct(x, 0..1), money, dur, len, signed(x, 0..2), signedPct(x, 0..2), clock]
out.fmt = FMTV.map(x => [enc(x), [0, 1, 2].map(d => F.num(x, d)), F.int(x), [0, 1].map(d => F.pct(x, d)), F.money(x), F.dur(x), F.len(x),
  [0, 1, 2].map(d => F.signed(x, d)), [0, 1, 2].map(d => F.signedPct(x, d)), F.clock(x)]);
const TEXTS = ['', 'plain', 'a & b', '<b>"x"</b>', "it's", '&amp;', 'Main Rd / Side St', '=SUM(A1:A3)', '+1.5e3', '-5', '-abc', '@x', '\tx', '\rx', ' =x', 'a,b', 'say "hi"', 'line\nbreak', 'cr\ronly', '+', '-', '-.5', '+.5e-3', '1e5', '=1', '  -7  '];
out.esc = TEXTS.map(s => [s, U.esc(s)]);
const CELLS = [...TEXTS, null, 0, 12.5, -3, 1e21, 0.1, true, false, [1, 2], { a: 1 }];
out.csvCell = CELLS.map(v => [v, U.csvCell(v)]);
out.csv = [[CELLS.slice(0, 9), CELLS.slice(9, 18), CELLS.slice(18)], [[]], [], [['a'], ['b', 'c']]].map(rows => [rows, U.csv(rows)]);

/* ---------- model.js pieces the Rust port types ---------- */
const P = c.newProject('Untitled project'), s0 = P.scenarios[0];
const sc = c.newScenario('Test');
const n1 = Net.addNode(sc, [-36.84851234567, 174.76331234567]), n2 = Net.addNode(sc, [-36.85, 174.77], { control: 'signals' });
const links = [undefined, 'arterial', 'motorway', 'bogus'].map(cls => Net.addLink(sc, n1, n2, [[-36.8491111111111, 174.765555555555], [-36.8495, 174.7688]], cls ? { cls } : {}));
const rename = (o, id) => ({ ...o, id });
const geoSc = {
  id: 's1', name: 'Geometry', notes: '', zones: {}, routes: {}, odManual: null, res: {},
  nodes: { n1: { id: 'n1', lat: -36.9, lng: 174.76, control: 'none' }, n2: { id: 'n2', lat: -36.903, lng: 174.765, control: 'none' } },
  links: {
    l1: { id: 'l1', a: 'n1', b: 'n2', name: '', cls: 'local', lanesAB: 1, lanesBA: 1, speed: 50, capPerLane: null, status: 'existing', busLane: false, cycle: 'none', pts: [[-36.901, 174.761], [-36.902, 174.7635]] },
    l2: { id: 'l2', a: 'n2', b: 'n1', name: 'Back', cls: 'collector', lanesAB: 1, lanesBA: 0, speed: 50, capPerLane: null, status: 'proposed', busLane: false, cycle: 'none', pts: [] },
  },
};
const stageLinks = [{ id: 'a' }, { id: 'b', status: 'proposed' }, { id: 'c', stage: 'committed', funding: 'unfunded' }, { id: 'd', stage: 'committed', funding: 'programmed' },
  { id: 'e', stage: 'unfunded' }, { id: 'f', stage: 'bogus', status: 'proposed' }, { id: 'g', stage: 'mitigation' }, { id: 'h', stage: 'committed', funding: 'funded' }]
  .map(L => ({ ...L, a: 'n1', b: 'n2' }));
const rows = [null, { id: 'r1', include: true, note: 'kept' }, { id: 'r2', include: false }, { id: 'r3', type: 'network', funding: 'funded' }, { id: 'r4', type: 'network', funding: 'programmed' },
  { id: 'r5', type: 'network', funding: 'unfunded' }, { id: 'r6', type: 'network' }, { id: 'r7', type: 'consent', status: 'Granted 2025', likely: 'likely' },
  { id: 'r8', type: 'consent', status: 'Lodged' }, { id: 'r9', type: 'consent', status: 'APPROVED', likely: 'possible' }, { id: 'r10', type: 'planchange', status: 'Operative in part' },
  { id: 'r11', type: 'consent' }, { id: 'r12', type: 'consent', status: 'granted', likely: '' }];
out.model = {
  defaultSettings: c.defaultSettings(),
  newProject: { ...P, id: 'p1', scenarios: [{ ...s0, id: 's1' }], active: 's1', base: 's1' },
  newScenario: { ...c.newScenario('Option'), id: 's2' },
  addNode: [rename(sc.nodes[n1], 'n1'), rename(sc.nodes[n2], 'n2')],
  addLink: links.map((id, i) => ({ cls: [null, 'arterial', 'motorway', 'bogus'][i], link: { ...sc.links[id], id: 'l' + i, a: 'n1', b: 'n2' } })),
  geomScenario: geoSc,
  geom: Object.values(geoSc.links).map(L => ({ link: L.id, geom: Net.geom(geoSc, L), len: Net.len(geoSc, L) })),
  linkStage: stageLinks.map(L => ({ link: L, stage: Scenarios.linkStage(L), unfunded: Scenarios.unfunded(L), programmed: Scenarios.programmed(L) })),
  inBaseline: rows.map(r => ({ row: r, ...Register.inBaseline(r) })),
};

writeFileSync(join(here, 'expected', 'util-cases.json'), JSON.stringify(encDeep(out)));
console.log('util-cases written:', Object.keys(out).length, 'groups');
