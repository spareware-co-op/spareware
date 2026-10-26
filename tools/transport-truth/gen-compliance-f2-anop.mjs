// Part f2, ANOP indicative multimodal level of service and fit (compliance-extra.js section 10)
// The project: f2-opt (the corridor project with a bus lane and cycleway option, a mitigation package, a later-year scenario ...), with Future Connect data,
// posted limits and corridors entered by the sets.
import { reportOf, exportBuild, exportEnabled } from './gen-compliance-f2-options.mjs';

const MAIN = ['lidc0chmq', 'lidc0dlsv', 'lidc0eysc', 'lidc0fkhj', 'lidc0gzyt', 'lidc0hcs9', 'lidc0ijt9', 'lidc0jdqb'];
const LETTERS = ['A+', 'A', 'B', 'C', 'D', 'E', 'F', 'F-'];

// A small seeded random stream for the combinations
function stream(seed) { let s = seed >>> 0; return () => { s = (Math.imul(s, 1664525) + 1013904223) >>> 0; return s / 4294967296; }; }

export async function runAnop(env) {
  const { c, X, add, rctx } = env;
  const F = 'f2-opt';
  const O = { active: 's-opt' };
  const sets = (...parts) => Object.assign({}, ...parts);
  const fcs = (ids, fc, sc = 1) => Object.fromEntries(ids.map(id => [`scenarios.${sc}.links.${id}.fc`, fc]));
  const lim = (ids, v, sc = 1) => Object.fromEntries(ids.map(id => [`scenarios.${sc}.links.${id}.speedLimit`, v]));
  const FC = { gt: 'Arterial', pt: 'Rapid Transit Network', freight: 'Level 1A', cycle: 'Regional cycle route' };
  const corridor = (o = {}) => ({ id: 'c1', name: 'Main Road', links: MAIN, landUse: 'city-metro', walk: 'primary', crossings: 3, crossDelay: 25, centre: true, school: false, rel: { gt: 1.3, freight: 1.6, pt: 2.2 }, ...o });
  const cors = (...list) => ({ 'complianceExtra.anop.corridors': list });

  /* ---------- the preferred level of service table: every mode, hierarchy and land use ---------- */
  {
    const modes = [...X.MODES, 'bogus'], hs = [...Object.keys(X.PREF.gt), ...Object.keys(X.PREF.freight), ...Object.keys(X.PREF.pt), ...Object.keys(X.PREF.cycle), ...Object.keys(X.PREF.walk), 'bogus', ''];
    const lus = ['city-metro', 'town-local', 'special', 'other', '', 'x'];
    const rows = [];
    for (const m of modes) for (const h of [...new Set(hs)]) for (const lu of lus) rows.push({ mode: m, h, lu, pref: X.anopPref(m, h, lu) });
    await add('anop', 'anopPref grid', 'anopPref', 'corridor', {}, () => rows, { n: rows.length });
  }
  /* ---------- the fit rule ---------- */
  {
    const letters = [...LETTERS, null, ''];
    const rows = [];
    for (const b of letters) for (const p of letters) for (const j of letters) rows.push({ modes: [{ base: b, pref: p, proj: j }], fit: X.anopFit([{ base: b, pref: p, proj: j }]) });
    const r = stream(7), pick = () => letters[Math.floor(r() * letters.length)];
    for (let i = 0; i < 600; i++) { const n = 1 + Math.floor(r() * 5), modes = Array.from({ length: n }, () => ({ base: pick(), pref: pick(), proj: pick() })); rows.push({ modes, fit: X.anopFit(modes) }); }
    rows.push({ modes: [], fit: X.anopFit([]) });
    await add('anop', 'anopFit combinations', 'anopFit', 'corridor', {}, () => rows, { n: rows.length });
  }

  /* ---------- the hierarchy of a corridor ---------- */
  const hier = {
    noFc: {},
    fcAll: fcs(MAIN, FC),
    fcPartial: fcs(MAIN.slice(0, 4), FC),
    fcShare: fcs(MAIN.slice(0, 7), FC),
    fcMixedGt: { ...fcs(MAIN.slice(0, 3), { gt: 'Motorway' }), ...fcs(MAIN.slice(3, 5), { gt: 'Arterial' }), ...fcs(MAIN.slice(5), { gt: 'Collector' }) },
    fcGtLocal: fcs(MAIN, { gt: 'Local street' }),
    fcGtBlank: fcs(MAIN, { gt: '', pt: '', freight: '', cycle: '' }),
    fcGtBlankSome: { ...fcs(MAIN.slice(0, 4), { gt: '' }), ...fcs(MAIN.slice(4), { gt: 'Arterial' }) },
    fcRail: fcs(MAIN, { freight: 'Rail freight', gt: 'Arterial' }),
    fcLevel: { ...fcs(MAIN.slice(0, 2), { freight: 'Level 1B' }), ...fcs(MAIN.slice(2), { freight: 'Level 3' }) },
    fcPt: { ...fcs(MAIN.slice(0, 1), { pt: 'frequent transit' }), ...fcs(MAIN.slice(1, 2), { pt: 'FTN' }), ...fcs(MAIN.slice(2, 3), { pt: 'IRTN' }), ...fcs(MAIN.slice(3, 4), { pt: 'Strategic' }), ...fcs(MAIN.slice(4, 5), { pt: 'Connector route' }),
      ...fcs(MAIN.slice(5, 6), { pt: 'Peak only' }), ...fcs(MAIN.slice(6, 7), { pt: 'Local bus' }), ...fcs(MAIN.slice(7), { pt: 'nothing' }) },
    fcPtTie: { ...fcs(MAIN.slice(0, 4), { pt: 'Rapid' }), ...fcs(MAIN.slice(4), { pt: 'Connector' }) },
    fcCycle: { ...fcs(MAIN.slice(0, 1), { cycle: 'Major cycleway' }), ...fcs(MAIN.slice(1, 3), { cycle: 'Connector' }), ...fcs(MAIN.slice(3, 4), { cycle: 'Local' }), ...fcs(MAIN.slice(4, 5), { cycle: 'Sports route' }), ...fcs(MAIN.slice(5), { cycle: 'none' }) },
    fcOnOneLink: fcs(MAIN.slice(0, 1), FC),
    fcOtherScenario: fcs(MAIN, FC, 0),
    activeBase: sets({ active: 'sidc01d0p' }, fcs(MAIN, FC, 0)),
  };
  const corrs = {
    main: corridor(), nowalk: corridor({ walk: undefined }), walkBogus: corridor({ walk: 'bogus' }), walkSecondary: corridor({ walk: 'secondary' }), walkTertiary: corridor({ walk: 'tertiary' }), walkNone: corridor({ walk: 'none' }),
    side: corridor({ id: 'c2', name: 'Side', links: ['lidc0lb8r', 'lidc0ngt6'], landUse: 'town-local' }), one: corridor({ id: 'c3', links: ['lidc0eysc'], landUse: 'special' }),
    missing: corridor({ id: 'c4', links: ['nope', 'lidc0eysc', 'zzz'] }), none: corridor({ id: 'c5', links: [] }), unknownOnly: corridor({ id: 'c6', links: ['nope'] }),
  };
  for (const [hk, hs] of Object.entries(hier)) for (const [ck, cr] of Object.entries(corrs)) {
    if (!['main', 'missing'].includes(ck) && hk !== 'fcAll' && hk !== 'noFc') continue;
    for (const mode of X.MODES) await add('anop', `hierOf ${hk} ${ck} ${mode}`, 'hierOf', F, sets(O, hs), P => X.hierOf(P.scenarios[1], cr, mode), { sc: 1, corridor: cr, mode });
  }
  await add('anop', 'hierOf unknown mode', 'hierOf', F, sets(O, hier.fcAll), P => X.hierOf(P.scenarios[1], corrs.main, 'bogus'), { sc: 1, corridor: corrs.main, mode: 'bogus' });

  /* ---------- the corridor axis and speed ---------- */
  const idLists = { main: MAIN, one: ['lidc0eysc'], two: ['lidc0eysc', 'lidc0fkhj'], side: ['lidc0lb8r', 'lidc0ngt6'], missing: ['nope', 'lidc0eysc'], none: [], unknown: ['nope'], mixed: ['lidc0eysc', 'lidc0lb8r'], shortDiag: ['lidc0r8fd', 'lidc0gzyt'] };
  for (const [k, ids] of Object.entries(idLists)) await add('anop', `corridorAxis ${k}`, 'corridorAxis', F, {}, P => X.corridorAxis(P.scenarios[1], ids), { sc: 1, ids });
  const speedSets = {
    plain: O,
    legal: sets(O, lim(MAIN, 60)),
    legalBase: sets(O, lim(MAIN, 40, 0)),
    nslr: sets(O, Object.fromEntries(MAIN.map(id => [`scenarios.1.links.${id}.nslr`, { v: 70 }]))),
    mixed: sets(O, lim(MAIN.slice(0, 3), 60), Object.fromEntries(MAIN.slice(3, 5).map(id => [`scenarios.1.links.${id}.nslr`, { v: 30 }]))),
    classDefault: sets(O, Object.fromEntries(MAIN.map(id => [`scenarios.1.links.${id}.speed`, 0]))),
    speedStrings: sets(O, Object.fromEntries(MAIN.map(id => [`scenarios.1.links.${id}.speedLimit`, '45']))),
    negative: sets(O, lim(MAIN, -5)),
  };
  for (const [sk, ss] of Object.entries(speedSets)) for (const per of ['AM', 'PM']) for (const [ik, ids] of Object.entries(idLists)) {
    if (!['main', 'side', 'one', 'missing', 'none'].includes(ik) && sk !== 'plain') continue;
    for (const pt of [false, true]) await add('anop', `corridorSpeed ${sk} ${per} ${ik} pt=${pt}`, 'corridorSpeed', F, ss, P => X.corridorSpeed(P.scenarios[1], P.scenarios[1].res[per], ids, pt), { sc: 1, per, ids, pt });
  }
  // a corridor whose junctions are within 1 m of each other has no axis: both directions are one
  const squash = { 'scenarios.1.nodes.nidc0kim3.lat': -37.1, 'scenarios.1.nodes.nidc0kim3.lng': 174.95560001 };
  await add('anop', 'corridorAxis degenerate', 'corridorAxis', F, squash, P => X.corridorAxis(P.scenarios[1], ['lidc0lb8r']), { sc: 1, ids: ['lidc0lb8r'] });
  for (const pt of [false, true]) await add('anop', `corridorSpeed degenerate pt=${pt}`, 'corridorSpeed', F, squash, P => X.corridorSpeed(P.scenarios[1], P.scenarios[1].res.AM, ['lidc0lb8r'], pt), { sc: 1, per: 'AM', ids: ['lidc0lb8r'], pt });
  await add('anop', 'anopLoS degenerate', 'anopLoS', F, squash, P => X.anopLoS(P.scenarios[1], P.scenarios[1].res.AM, corridor({ links: ['lidc0lb8r'] })), { sc: 1, per: 'AM', corridor: corridor({ links: ['lidc0lb8r'] }) });
  await add('anop', 'corridorSpeed bus lane scenario', 'corridorSpeed', F, O, P => X.corridorSpeed(P.scenarios[2], P.scenarios[2].res.AM, MAIN, true), { sc: 2, per: 'AM', ids: MAIN, pt: true });
  await add('anop', 'corridorSpeed no result', 'corridorSpeed', F, O, P => X.corridorSpeed(P.scenarios[1], null, MAIN, false), { sc: 1, per: null, ids: MAIN, pt: false });
  await add('anop', 'corridorSpeed no result scenario', 'corridorSpeed', F, O, P => X.corridorSpeed(P.scenarios[5], P.scenarios[5].res.AM, MAIN, false), { sc: 5, per: 'AM', ids: MAIN, pt: false });
  await add('anop', 'corridorSpeed grid', 'corridorSpeed', 'grid', {}, P => X.corridorSpeed(P.scenarios[0], P.scenarios[0].res.AM, ['lidc0r4yn', 'lidc0sold', 'lidc0tzws', 'lidc0unhw', 'lidc014d68'], false), { sc: 0, per: 'AM', ids: ['lidc0r4yn', 'lidc0sold', 'lidc0tzws', 'lidc0unhw', 'lidc014d68'], pt: false });

  /* ---------- the level of service of a corridor ---------- */
  const losSets = { plain: O, bus: { active: 's-bus' }, mit: { active: 's-mit' }, speeds: sets(O, lim(MAIN, 60)), cycle: { active: 's-bus', ...fcs(MAIN, FC, 2) } };
  const losCors = {
    main: corridor(), noRel: corridor({ rel: {} }), relNone: corridor({ rel: undefined }), relBands: corridor({ rel: { gt: 1.05, freight: 1.12, pt: 1.3 } }), relHigh: corridor({ rel: { gt: 1.6, freight: 1.9, pt: 2.5 } }),
    relOdd: corridor({ rel: { gt: '1.4', freight: 'x', pt: 0 } }), relEdges: corridor({ rel: { gt: 1.1, freight: 2, pt: 1.75 } }), relNeg: corridor({ rel: { gt: -1, freight: '', pt: null } }),
    walkOnly: corridor({ crossings: 0, crossDelay: 5 }), manyCross: corridor({ crossings: 12, crossDelay: 70 }), crossStr: corridor({ crossings: '2', crossDelay: '35' }), crossBlank: corridor({ crossings: '', crossDelay: '' }), crossNull: corridor({ crossings: null, crossDelay: null }),
    crossText: corridor({ crossings: 'many', crossDelay: 'long' }), crossOne: corridor({ crossings: 1, crossDelay: 20 }), crossDelayEdge: corridor({ crossings: 5, crossDelay: 10 }), crossDelayHigh: corridor({ crossings: 5, crossDelay: 60 }),
    school: corridor({ centre: false, school: true }), noHalf: corridor({ centre: false, school: false }), side: corridor({ id: 'c2', name: 'Side', links: ['lidc0lb8r', 'lidc0ngt6'], landUse: 'town-local' }), signals: corridor({ crossings: undefined, links: ['lidc0eysc', 'lidc0fkhj', 'lidc0gzyt'] }),
    missing: corridor({ links: ['nope', 'lidc0eysc'] }), none: corridor({ links: [] }),
  };
  for (const [sk, ss] of Object.entries(losSets)) for (const [ck, cr] of Object.entries(losCors)) {
    if (sk !== 'plain' && !['main', 'side', 'signals', 'none'].includes(ck)) continue;
    for (const per of ['AM', 'PM']) await add('anop', `anopLoS ${sk} ${ck} ${per}`, 'anopLoS', F, ss, P => { const sc = P.scenarios[sk === 'bus' || sk === 'cycle' ? 2 : sk === 'mit' ? 3 : 1]; return X.anopLoS(sc, sc.res[per], cr); },
      { sc: sk === 'bus' || sk === 'cycle' ? 2 : sk === 'mit' ? 3 : 1, per, corridor: cr });
  }
  await add('anop', 'anopLoS cycle facilities', 'anopLoS', F, { 'scenarios.1.links.lidc0eysc.cycle': 'separated', 'scenarios.1.links.lidc0fkhj.cycle': 'painted', 'scenarios.1.links.lidc0lb8r.cycle': 'other', 'scenarios.1.links.lidc0ngt6.cycle': 'separated' }, P => X.anopLoS(P.scenarios[1], P.scenarios[1].res.AM, corridor({ links: ['lidc0eysc', 'lidc0fkhj', 'lidc0lb8r', 'lidc0ngt6', 'lidc0gzyt'] })),
    { sc: 1, per: 'AM', corridor: corridor({ links: ['lidc0eysc', 'lidc0fkhj', 'lidc0lb8r', 'lidc0ngt6', 'lidc0gzyt'] }) });

  /* ---------- the corridors, the assessment, the CSV, the report and the export ---------- */
  const anopSets = {
    auto: O,
    sameScenario: {},
    autoBus: { active: 's-bus' },
    autoMit: { active: 's-mit' },
    autoLate: { active: 's-late' },
    autoNone: { active: 's-none' },
    autoUnf: { active: 's-unf' },
    author: sets(O, cors(corridor())),
    authorTwo: sets(O, cors(corridor(), corridor({ id: 'c2', name: 'Side', links: ['lidc0lb8r', 'lidc0ngt6'], landUse: 'town-local', walk: 'secondary', crossings: null, crossDelay: null, centre: false, rel: { gt: 1.2 } }))),
    authorFc: sets(O, cors(corridor()), fcs(MAIN, FC)),
    authorFcBus: sets({ active: 's-bus' }, cors(corridor()), fcs(MAIN, FC, 2), lim(MAIN, 60, 0)),
    authorSame: sets({}, cors(corridor())),
    authorLandUses: sets(O, cors(corridor({ landUse: 'city-metro' }), corridor({ id: 'c2', landUse: 'town-local' }), corridor({ id: 'c3', landUse: 'special' }), corridor({ id: 'c4', landUse: 'other' }), corridor({ id: 'c5', landUse: undefined }), corridor({ id: 'c6', landUse: 'bogus' }))),
    authorNames: sets(O, cors(corridor({ name: 'Name, with "quotes"' }), corridor({ id: 'c2', name: '=Main' }))),
    authorMissing: sets(O, cors(corridor({ links: ['nope'] }))),
    emptyList: sets(O, { 'complianceExtra.anop.corridors': [] }),
    meta: sets(O, cors(corridor()), { 'complianceExtra.anop.project': 'Project, one', 'complianceExtra.anop.assessor': 'A. Assessor', 'complianceExtra.anop.option': 'Option A', name: 'Named project' }),
    metaBlank: sets(O, cors(corridor()), { name: 'Only the project name' }),
    metaNone: sets(O, cors(corridor()), { name: '' }),
    shortStreets: sets(O, { 'scenarios.0.links.lidc0eysc.name': 'Short Rd', 'scenarios.0.links.lidc0fkhj.name': 'Short Rd', 'scenarios.0.links.lidc0eysc.cls': 'collector', 'scenarios.0.links.lidc0fkhj.cls': 'collector' }),
    autoCorridorLimits: sets(O, { 'scenarios.0.links.lidc0lb8r.name': 'Side 2N', 'scenarios.0.links.lidc0lb8r.lanesAB': 1 }),
    manyCorridors: sets(O, Object.fromEntries([...Array(14).keys()].flatMap(i => [[`scenarios.0.links.${MAIN[i % 8]}.name`, `Road ${i % 7}`], [`scenarios.0.links.${MAIN[i % 8]}.cls`, i % 2 ? 'collector' : 'arterial']]))),
    closedLinks: sets(O, { 'scenarios.0.links.lidc0eysc.status': 'closed', 'scenarios.0.links.lidc0fkhj.status': 'closed' }),
    noPm: sets(O, { 'scenarios.0.res.PM': null }),
    mismatchPm: sets(O, { 'scenarios.1.res.PM.mismatch': true }),
  };
  for (const [k, s] of Object.entries(anopSets)) {
    await add('anop', `anopCorridors ${k}`, 'anopCorridors', F, s, P => X.anopCorridors(P));
    await add('anop', `anop ${k}`, 'anop', F, s, P => X.anop(P));
    await add('anop', `anopCSV ${k}`, 'anopCSV', F, s, P => X.anopCSV(P));
    await add('anop', `export anop ${k}`, 'export-anop', F, s, P => ({ text: exportBuild(c, 'anop', P), enabled: exportEnabled(c, 'anop') }));
    await add('anop', `report anop ${k}`, 'report-anop', F, s, P => reportOf(c, 'compliance-extra-anop', P, undefined));
    await add('anop', `report anop ${k} with context`, 'report-anop-ctx', F, s, P => reportOf(c, 'compliance-extra-anop', P, rctx));
  }
  await add('anop', 'anop grid', 'anop', 'grid', { active: 's-opt', base: 's-dm' }, P => X.anop(P));
  await add('anop', 'anopCorridors grid', 'anopCorridors', 'grid', {}, P => X.anopCorridors(P));
  await add('anop', 'anopCSV grid', 'anopCSV', 'grid', { active: 's-opt', base: 's-dm' }, P => X.anopCSV(P));
  await add('anop', 'report anop grid', 'report-anop', 'grid', { active: 's-opt', base: 's-dm' }, P => reportOf(c, 'compliance-extra-anop', P, undefined));
}
