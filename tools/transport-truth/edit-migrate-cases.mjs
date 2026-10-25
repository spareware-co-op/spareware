// Saved projects of every older kind for the migrate truth test (gen-edit.mjs): settings versions 1 to 3 and beyond, every
// SETTINGS_V2 key saved or missing, years as text, the old discount schedule, malformed sites, registers, scenarios and ids.
// Each case is the text of a project file; the prototype's migrate() answers are recorded for it.
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const clone = o => JSON.parse(JSON.stringify(o));
const corridor = JSON.parse(readFileSync(join(here, 'inputs', 'corridor.json'), 'utf8'));
const grid = JSON.parse(readFileSync(join(here, 'inputs', 'grid.json'), 'utf8'));

// The keys of SETTINGS_V2 with a value that is the old default, one that is the current default, and another one
const KEYS = [
  { key: 'growthMode', old: 'compound', now: 'linear', other: 'weird' },
  { key: 'gapTol', old: 0.0005, now: 0.0001, other: 0.002 },
  { key: 'maxIter', old: 60, now: 200, other: 120 },
  { key: 'conv.stable', old: false, now: true, other: 'yes' },
  { key: 'conv.capDamp', old: 'none', now: 'msa', other: 'other' },
  { key: 'econ.method', old: 'uniform', now: 'mbcm', other: 'custom' },
  { key: 'junction.pff', old: 1, now: 0.95, other: 0.9 },
  { key: 'junction.T', old: 1, now: 0.5, other: 0.25 },
  { key: 'busPcu', old: 3, now: 2, other: 2.5 },
];
const setPath = (o, k, v) => { const p = k.split('.'), last = p.pop(); let a = o; for (const q of p) a = a[q] ??= {}; a[last] = v; };

// A project as an older version would have saved it: the corridor project with its own settings object
const withSettings = (base, settings, extra = {}) => ({ ...clone(base), settings, ...extra });

// A small saved project (two scenarios) that most cases start from, so the recorded file stays small
const node = (id, lat, lng, control = 'none') => ({ id, lat, lng, control });
const link = (id, a, b, name, cls = 'collector') => ({ id, a, b, name, cls, lanesAB: 1, lanesBA: 1, speed: 50, capPerLane: null, status: 'existing', busLane: false, cycle: 'none', pts: [] });
const tinyScenario = (id, name, extra = {}) => ({ id, name, notes: '', nodes: { n1: node('n1', -36.9, 174.76), n2: node('n2', -36.9, 174.763, 'priority'), n3: node('n3', -36.9, 174.766), n4: node('n4', -36.898, 174.763) },
  links: { l1: link('l1', 'n1', 'n2', 'Main Road', 'arterial'), l2: link('l2', 'n2', 'n3', 'Main Road', 'arterial'), l3: link('l3', 'n2', 'n4', 'Side Street', 'local') },
  zones: { z1: { id: 'z1', kind: 'zone', name: 'Houses', lat: -36.8985, lng: 174.762, lu: { residential: 120 }, connectors: null }, gw: { id: 'gw', kind: 'gateway', name: 'West', lat: -36.9, lng: 174.7585, node: 'n1', gw: { AMin: 300, AMout: 200, PMin: 200, PMout: 300 } } },
  routes: { r1: { id: 'r1', name: 'Bus route 1', color: '#7048e8', freq: 6, wp: ['n1', 'n3'] } }, odManual: null, res: {}, ...extra });
const tiny = { v: 1, id: 'ptiny', name: 'Tiny', settings: { sv: 3, baseYear: 2026, year: 2026 }, scenarios: [tinyScenario('s1', 'Base case'), tinyScenario('s2', 'Option', { year: 2036 })], active: 's1', base: 's1', view: { c: [-36.8485, 174.7633], z: 14 } };
const small = { baseYear: 2026, year: 2031, growth: 2, period: 'AM', hvPct: 7 };

export function migrateCases() {
  const cases = [];
  const add = (name, v, seed) => cases.push({ name, text: typeof v === 'string' ? v : JSON.stringify(v), seed: seed ?? 500 + cases.length });
  add('empty object', {});
  add('no scenarios', { id: 'p1', settings: { ...small, sv: 3 }, scenarios: [] });
  add('scenarios not a list', { id: 'p1', scenarios: { a: 1 } });
  add('no settings', withSettings(tiny, undefined));
  add('settings array', withSettings(tiny, []));
  add('settings text', withSettings(tiny, 'hello'));
  add('settings null', withSettings(tiny, null));
  add('settings sv text', withSettings(tiny, { ...small, sv: 'x' }));
  // Settings versions
  for (const sv of [undefined, 0, -1, 1, 1.9, 2, 2.5, 3, 4, 99, '2', '3', ' 2 ', null, true, false, [], 'abc']) {
    add(`sv ${JSON.stringify(sv) ?? 'missing'}`, withSettings(tiny, { ...small, ...(sv === undefined ? {} : { sv }) }));
  }
  // Each SETTINGS_V2 key saved as the old value, the current one and another, at settings versions 1 and 2, and alone
  for (const sv of [1, 2]) for (const k of KEYS) for (const which of ['old', 'now', 'other']) {
    const s = { ...small, sv };
    setPath(s, k.key, k[which]);
    add(`sv ${sv} ${k.key} ${which}`, withSettings(tiny, s));
  }
  // Every key saved with the current values at version 1: nothing to review
  {
    const s = { ...small, sv: 1 };
    for (const k of KEYS) setPath(s, k.key, k.now);
    add('sv 1 all current', withSettings(tiny, s));
    const t = { ...small, sv: 1 };
    for (const k of KEYS) setPath(t, k.key, k.old);
    add('sv 1 all old', withSettings(tiny, t));
    const u = { ...small, sv: 2 };
    for (const k of KEYS) setPath(u, k.key, k.other);
    add('sv 2 all other', withSettings(tiny, u));
    const w = { ...small };
    for (const k of KEYS) if (k.key.includes('.')) setPath(w, k.key, k.other);
    add('nested only', withSettings(tiny, w));
  }
  // Groups saved partly, or saved as something that is not an object
  add('conv partial', withSettings(tiny, { ...small, sv: 1, conv: { chg: 0.02 } }));
  add('conv null', withSettings(tiny, { ...small, sv: 1, conv: null }));
  add('econ partial', withSettings(tiny, { ...small, sv: 1, econ: { vot: 40, discount: 4 } }));
  add('econ discount missing', withSettings(tiny, { ...small, sv: 1, econ: { vot: 40, method: 'uniform' } }));
  add('junction partial', withSettings(tiny, { ...small, sv: 2, junction: { losMethod: 'sidra' } }));
  add('growth text', withSettings(tiny, { ...small, sv: 3, growth: '1.5', maxIter: '80' }));
  add('unknown keys', withSettings(tiny, { ...small, sv: 3, futureKey: { a: [1, 2, { b: 3 }] }, rates: { custom: { AM: { rate: 1, inPct: 50 } }, residential: { PM: { rate: 2, inPct: 10 } } } }));
  add('rates reordered', withSettings(tiny, { ...small, sv: 3, rates: { tertiary: { AM: { rate: 0.3, inPct: 80 } }, residential: { AM: { rate: 0.9, inPct: 25 } } } }));
  // Years
  for (const [b, y] of [['2026', ' 2031 '], [' ', ''], ['abc', 'abc'], [0, -5], [2030.6, 2031.4], [null, null], [2026, '0'], [2040, undefined], ['  ', 2035], [Infinity, 'Infinity'], ['1e3', '2.5e3'], [true, [2031]]]) {
    add(`years ${JSON.stringify(b)} ${JSON.stringify(y)}`, withSettings(tiny, { ...small, sv: 3, baseYear: b, year: y }));
  }
  // The old discount schedule
  const OLD = [{ to: 30, r: 2 }, { to: 60, r: 1.5 }];
  for (const [name, rates, years] of [['old 40', OLD, 40], ['old 100', OLD, 100], ['old 101', OLD, 101], ['old 120', OLD, 120], ['old text years', OLD, '90'], ['old no years', OLD, undefined],
    ['old reordered', [{ r: 2, to: 30 }, { r: 1.5, to: 60 }], 40], ['new', [{ to: 30, r: 2 }, { to: 100, r: 1.5 }, { to: null, r: 1 }], 40], ['single rate', 4, 40], ['old three', [{ to: 30, r: 2 }, { to: 60, r: 1.5 }, { to: null, r: 1 }], 40]]) {
    add(`rates ${name}`, withSettings(tiny, { ...small, sv: 2, econ: { method: 'mbcm', rates, ...(years === undefined ? {} : { years }) } }));
  }
  // Scenarios
  const sc0 = clone(tiny.scenarios[0]);
  const mk = (patch) => withSettings(tiny, { ...small, sv: 3 }, { scenarios: [{ ...clone(sc0), ...patch }, ...clone(tiny.scenarios.slice(1))] });
  add('scenario missing containers', withSettings(tiny, { ...small, sv: 3 }, { scenarios: [{ id: 's1', name: 'Bare' }, { id: 's2', nodes: null, links: 0, zones: '', routes: false, res: null }] }));
  add('results without paths', mk({ res: { AM: { period: 'AM', stale: false, arcs: {} }, PM: { period: 'PM', paths: [], arcs: {} }, IP: null, X: { paths: [{ i: 0, j: 1, list: [] }], period: 'X' }, Y: {} } }));
  for (const y of ['2036', ' 2036 ', '', ' ', 'x', 0, -1, null, 2040.4, 2036, [2030], true, '1e3']) add(`scenario year ${JSON.stringify(y)}`, mk({ year: y }));
  const good = { id: 'j1', nodes: ['a', 'b'], name: 'Good' };
  for (const [name, sites] of [['array', []], ['text', 'x'], ['number', 5], ['zero', 0], ['false', false], ['null', null], ['empty', {}],
    ['mixed', { j1: good, j2: { nodes: ['c', 'd'] }, j3: { id: 'wrong', nodes: ['e', 'f'] }, j4: null, j5: { nodes: 'ab' }, j6: 5, j7: {}, j8: [], j9: { id: 'j9' } }]]) add(`sites ${name}`, mk({ sites }));
  for (const b of ['abc', '50', 50, null, '', ' ', true, [], {}, '1e999', 120, -5, '0x10']) add(`buildout ${JSON.stringify(b)}`, mk({ buildout: b }));
  // Register
  const presetRows = [
    { id: 'preset:mill-road-1', ref: '', name: 'Mill Road Stage 1', type: 'network', preset: true, funding: 'unfunded' },
    { id: 'preset:mill-road-2-3', name: 'Mill Road Stages 2-3', type: 'network', preset: true, designation: { id: 'old' } },
    { id: 'preset:mill-road-1', name: 'Again', preset: true, designation: null },
    { id: 'preset:sunfield', name: 'Sunfield', type: 'ftaa', preset: true },
    { id: 'preset:mill-road-1', name: 'Not a preset', preset: false },
    { id: 'mill-road-1', name: 'No prefix', preset: true },
    null, 5, 'text', [], { name: 'no id', preset: true }, { id: 'preset:mill-road-2-3', preset: 1 },
  ];
  add('register presets', withSettings(tiny, { ...small, sv: 3 }, { register: presetRows }));
  for (const [name, r] of [['object', { a: 1 }], ['text', 'x'], ['null', null], ['number', 7], ['empty', []], ['false', false]]) add(`register ${name}`, withSettings(tiny, { ...small, sv: 3 }, { register: r }));
  // me, ids, active, base, view
  for (const [name, me] of [['text', 'x'], ['number', 3], ['null', null], ['object', { AM: { 'a>b': 1.2 }, info: { AM: { baseYear: 2026 } } }], ['array', [1]], ['false', false]]) add(`me ${name}`, withSettings(tiny, { ...small, sv: 3 }, { me }));
  for (const [name, patch] of [['no id', { id: undefined }], ['empty id', { id: '' }], ['number id', { id: 5 }], ['array id', { id: ['x'] }], ['null id', { id: null }],
    ['active missing', { active: undefined }], ['active bad', { active: 'zzz' }], ['active second', { active: tiny.scenarios[1].id }], ['base bad', { base: 'zzz' }], ['base number', { base: 5 }],
    ['base missing', { base: undefined }], ['view missing', { view: undefined }], ['view null', { view: null }], ['view empty', { view: {} }], ['view zero', { view: 0 }],
    ['notice left', { settingsNotice: { from: 1, to: 3, text: 'old', changed: [] } }]]) add(`project ${name}`, withSettings(tiny, { ...small, sv: 3 }, patch));
  add('notice left sv 1', withSettings(tiny, { ...small, sv: 1 }, { settingsNotice: { from: 1, to: 3, text: 'old', changed: [{ key: 'busPcu' }] } }));
  add('notice left sv 3 clears', withSettings(tiny, { ...small, sv: 3 }, { settingsNotice: { from: 1, to: 3, text: 'old', changed: [{ key: 'busPcu' }] } }));
  // Whole saved projects
  add('corridor as saved', corridor);
  add('grid as saved', grid);
  add('corridor v1', withSettings(corridor, { baseYear: 2026, year: 2031, growth: 2, growthMode: 'compound', junction: { capMethod: 'movement', losMethod: 'hcm6', queueSpace: 7, f95: 1.6 }, econ: { vot: 32, years: 40, discount: 6 } }));
  add('grid v2', withSettings(grid, { ...clone(grid.settings), sv: 2, busPcu: undefined }));
  // A project whose scenario carries results (stale marking) and extra keys at every level
  add('results and extras', withSettings(tiny, { ...small, sv: 1 }, { extraTop: { z: 1, a: 2 }, scenarios: [{ ...clone(sc0), res: { AM: { period: 'AM', stale: false, arcs: { 'l:ab': { v: 5 } } }, PM: { period: 'PM', stale: false } }, extraSc: [1, 2] }, clone(tiny.scenarios[1])] }));
  return cases;
}

// Cases for Store.adoptDefaults and Store.keepSettings: a project saved at version 1 or 2, then adopt (all, some keys, none) or keep
export function adoptCases() {
  const out = [];
  const s1 = { ...small, sv: 1, junction: { pff: 0.9 } };
  const res = { AM: { period: 'AM', stale: false }, PM: { period: 'PM', stale: false, paths: [] } };
  const base = withSettings(tiny, s1, { scenarios: clone(tiny.scenarios).map(s => ({ ...s, res: clone(res) })) });
  for (const [name, action] of [['adopt all', { adopt: null }], ['adopt busPcu', { adopt: ['busPcu'] }], ['adopt two', { adopt: ['growthMode', 'junction.T'] }], ['adopt none listed', { adopt: ['nonsense'] }],
    ['adopt empty list', { adopt: [] }], ['keep', { keep: true }], ['adopt then adopt', { adopt: null, again: { adopt: null } }], ['keep then keep', { keep: true, again: { keep: true } }]]) out.push({ name, text: JSON.stringify(base), action, seed: 900 + out.length });
  out.push({ name: 'adopt at sv 3', text: JSON.stringify(withSettings(tiny, { ...small, sv: 3 })), action: { adopt: null }, seed: 950 });
  out.push({ name: 'keep at sv 3', text: JSON.stringify(withSettings(tiny, { ...small, sv: 3 })), action: { keep: true }, seed: 951 });
  return out;
}
