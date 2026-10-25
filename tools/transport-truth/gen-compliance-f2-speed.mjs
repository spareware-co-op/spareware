// Part f2, speed limits and the cost benefit disclosure statement (compliance-extra.js section 9)
// The project: the saved corridor project with a divided road (two one-way carriageways on the first Main Road link), one-way roads far apart and a
// proposed new road in the option, results from the engine.
import { reportOf, exportBuild, exportEnabled } from './gen-compliance-f2-options.mjs';
import { record } from './gen-compliance-lib.mjs';

const clone = o => JSON.parse(JSON.stringify(o));
const MAIN = ['lidc0chmq', 'lidc0dlsv', 'lidc0eysc', 'lidc0fkhj', 'lidc0gzyt', 'lidc0hcs9', 'lidc0ijt9', 'lidc0jdqb'];

export async function buildSpeedFixture(env) {
  const { c, withResults, saveFixture } = env;
  const P = await withResults(c, 'corridor', { rerun: true, mutate: (P) => {
    for (const sc of P.scenarios) {
      // a divided road: the first Main Road link is one carriageway and a second runs back between the same junctions
      const L = sc.links.lidc0chmq; L.lanesBA = 0;
      sc.links['lidc0chmq-r'] = { ...clone(L), id: 'lidc0chmq-r', a: L.b, b: L.a };
      // one-way roads far apart, opposite ways: not a divided road
      sc.nodes.f1 = { id: 'f1', lat: -37.1, lng: 174.972, control: 'none' }; sc.nodes.f2 = { id: 'f2', lat: -37.1, lng: 174.9745, control: 'none' };
      sc.nodes.f3 = { id: 'f3', lat: -37.1035, lng: 174.972, control: 'none' }; sc.nodes.f4 = { id: 'f4', lat: -37.1035, lng: 174.9745, control: 'none' };
      sc.links.far1 = { id: 'far1', a: 'f1', b: 'f2', name: 'Far Road', cls: 'local', lanesAB: 1, lanesBA: 0, speed: 50, capPerLane: null, status: 'existing', busLane: false, cycle: 'none', pts: [] };
      sc.links.far2 = { id: 'far2', a: 'f4', b: 'f3', name: 'Far Road', cls: 'local', lanesAB: 1, lanesBA: 0, speed: 50, capPerLane: null, status: 'existing', busLane: false, cycle: 'none', pts: [] };
      sc.links.far3 = { id: 'far3', a: 'f2', b: 'f1', name: 'Far Road', cls: 'local', lanesAB: 0, lanesBA: 1, speed: 50, capPerLane: null, status: 'existing', busLane: false, cycle: 'none', pts: [] };
      // a bent road with shape points, two-way
      sc.links.bend = { id: 'bend', a: 'f2', b: 'f4', name: 'Bend Road', cls: 'collector', lanesAB: 1, lanesBA: 1, speed: 60, capPerLane: null, status: 'existing', busLane: false, cycle: 'none', pts: [[-37.1015, 174.9752], [-37.102, 174.9748]] };
    }
    // a new road, in the option only
    P.scenarios[1].links.lnew = { id: 'lnew', a: 'nidc0kim3', b: 'nidc0olv4', name: 'New Road', cls: 'collector', lanesAB: 1, lanesBA: 1, speed: 50, capPerLane: null, status: 'proposed', busLane: false, cycle: 'none', pts: [] };
  } });
  return saveFixture('f2-speed', P);
}

export async function runSpeed(env) {
  const { c, X, add, rctx } = env;
  await buildSpeedFixture(env);
  const F = 'f2-speed';
  const O = { active: 's-opt' };
  const cxs = (ids, cx, sc = 1) => Object.fromEntries(ids.map(id => [`scenarios.${sc}.links.${id}.cx`, cx]));
  const sets = (...parts) => Object.assign({}, ...parts);
  const SP = c.ComplianceExtra.STD.speed;

  /* ---------- Schedule 1 minimum lengths ---------- */
  for (const v of [-5, 0, 10, 20, 39.9, 40, 40.1, 50, 59, 60, 60.5, 61, 70, 71, 80, 81, 90, 100, 101, 110, 120, 121, 200]) await add('speed', `minLenFor ${v}`, 'minLenFor', 'corridor', {}, () => X.minLenFor(v), { v });

  /* ---------- sections ---------- */
  const secSets = {
    none: O,
    mainRoad: sets(O, cxs(MAIN, { ruleClass: 'urban-connector', limit: 60 })),
    mainRoadSplit: sets(O, cxs(MAIN.slice(0, 4), { ruleClass: 'urban-connector', limit: 60 }), cxs(MAIN.slice(4), { ruleClass: 'urban-connector', limit: 80 })),
    gap: sets(O, cxs([MAIN[0], MAIN[1], MAIN[3], MAIN[4]], { ruleClass: 'urban-connector', limit: 60 })),
    named: sets(O, { 'scenarios.1.links.lidc0eysc.name': '' }, cxs(MAIN.slice(0, 4), { limit: 60 })),
    explicitSection: sets(O, cxs([MAIN[0], MAIN[5], 'lidc0lb8r'], { ruleClass: 'urban-connector', limit: 60, section: 'S1' }), cxs([MAIN[1]], { limit: 60, section: 'S2' })),
    variableOnly: sets(O, cxs([MAIN[2]], { variable: true, varReason: 'school' })),
    limitOnly: sets(O, cxs(['lidc0ngt6', 'lidc0lb8r'], { limit: 40 })),
    limitZero: sets(O, cxs(['lidc0ngt6'], { limit: 0, ruleClass: 'urban-street' })),
    closed: sets(O, cxs(MAIN, { limit: 60 }), { 'scenarios.1.links.lidc0eysc.status': 'closed' }),
    divided: sets(O, cxs(['lidc0chmq', 'lidc0chmq-r'], { ruleClass: 'urban-connector', limit: 60 })),
    dividedOneSide: sets(O, cxs(['lidc0chmq'], { ruleClass: 'urban-connector', limit: 60 })),
    dividedMain: sets(O, cxs(['lidc0chmq', 'lidc0chmq-r', ...MAIN.slice(1, 4)], { ruleClass: 'urban-connector', limit: 60 })),
    far: sets(O, cxs(['far1', 'far2', 'far3'], { ruleClass: 'urban-street', limit: 50 })),
    bent: sets(O, cxs(['bend'], { ruleClass: 'urban-connector', limit: 60 })),
    nameless: sets(O, { 'scenarios.1.links.far1.name': '', 'scenarios.1.links.far2.name': '' }, cxs(['far1', 'far2'], { limit: 50 })),
    lanesStrings: sets(O, { 'scenarios.1.links.far1.lanesAB': '1', 'scenarios.1.links.far1.lanesBA': '0', 'scenarios.1.links.far3.lanesAB': 0, 'scenarios.1.links.far3.lanesBA': '1' }, cxs(['far1', 'far3'], { limit: 50 })),
  };
  for (const [k, s] of Object.entries(secSets)) {
    await add('speed', `speedSections ${k}`, 'speedSections', F, s, P => X.speedSections(P.scenarios[1]).map(Ls => Ls.map(l => l.id)), { sc: 1 });
    await add('speed', `speedCheck ${k}`, 'speedCheck', F, s, P => X.speedCheck(P));
  }
  for (const ids of [['lidc0chmq'], ['lidc0chmq-r'], ['lidc0chmq', 'lidc0chmq-r'], ['lidc0chmq', 'lidc0dlsv'], ['lidc0chmq', 'lidc0chmq-r', 'lidc0dlsv'], ['far1'], ['far1', 'far2'], ['far1', 'far3'], ['far1', 'far2', 'far3'], ['bend'], ['bend', 'far1'],
    MAIN, ['lidc0lb8r', 'lidc0ngt6'], []]) {
    await add('speed', `roadLen ${ids.join(',') || 'none'}`, 'roadLen', F, {}, P => X.roadLen(P.scenarios[1], ids.map(id => P.scenarios[1].links[id])), { sc: 1, ids });
  }
  await add('speed', 'roadLen in the base', 'roadLen', F, {}, P => X.roadLen(P.scenarios[0], ['lidc0chmq', 'lidc0chmq-r'].map(id => P.scenarios[0].links[id])), { sc: 0, ids: ['lidc0chmq', 'lidc0chmq-r'] });

  /* ---------- speedCheck: every Schedule 3 class at and around its range, the flags, exceptions, authority, ONF ---------- */
  for (const [k, cls] of Object.entries(SP.classes)) {
    for (const limit of [cls.min - 10, cls.min, cls.max, cls.max + 10]) {
      if (limit <= 0) continue;
      await add('speed', `speedCheck class ${k} at ${limit}`, 'speedCheck', F, sets(O, cxs(MAIN, { ruleClass: k, limit })), P => X.speedCheck(P));
    }
  }
  const one = (cx, extra = {}, id = 'lidc0eysc') => sets(O, cxs([id], cx), extra);
  const flagSets = {
    noClass: one({ limit: 60 }),
    unknownClass: one({ ruleClass: 'bogus', limit: 60 }),
    isz: one({ ruleClass: 'isz-urban', limit: 50 }),
    iszHistory: one({ ruleClass: 'isz-urban', limit: 50, crashHistory: true }),
    iszRural: one({ ruleClass: 'isz-rural', limit: 60 }),
    short60: one({ ruleClass: 'urban-connector', limit: 60 }),
    short100: one({ ruleClass: 'transit', limit: 100 }),
    short120: one({ ruleClass: 'expressway', limit: 120 }),
    short30: one({ ruleClass: 'low-volume', limit: 30 }),
    short30Class: one({ ruleClass: 'urban-street', limit: 30 }),
    excImpracticable: one({ ruleClass: 'urban-connector', limit: 60, exception: 'impracticable' }),
    excBogus: one({ ruleClass: 'urban-connector', limit: 60, exception: 'bogus' }),
    excSchool: one({ ruleClass: 'urban-connector', limit: 60, school: true }),
    excVariable: one({ ruleClass: 'urban-connector', limit: 60, variable: true, varReason: 'users' }),
    varNoReason: one({ ruleClass: 'urban-connector', limit: 60, variable: true }),
    varBadReason: one({ ruleClass: 'urban-connector', limit: 60, variable: true, varReason: 'because' }),
    varEveryReason: Object.fromEntries(Object.keys(SP.varReasons).map((k, i) => [`scenarios.1.links.${MAIN[i]}.cx`, { ruleClass: 'urban-connector', limit: 60, variable: true, varReason: k }])),
    nzta110: one({ ruleClass: 'transit', limit: 110 }),
    nzta120Sh: one({ ruleClass: 'expressway', limit: 120 }, { 'scenarios.1.links.lidc0eysc.cls': 'motorway' }),
    nzta120UserSh: one({ ruleClass: 'expressway', limit: 120 }, { 'compliance.net.shUser.lidc0eysc': true }),
    nzta110Sh: one({ ruleClass: 'transit', limit: 110 }, { 'scenarios.1.links.lidc0eysc.cls': 'ramp' }),
    authorityAfter: one({ ruleClass: 'urban-connector', limit: 60 }, { 'complianceExtra.ctx.reportDate': '2026-06-01' }),
    authorityBefore: one({ ruleClass: 'urban-connector', limit: 60 }, { 'complianceExtra.ctx.reportDate': '2026-01-01' }),
    onfLocal: one({ ruleClass: 'urban-street', limit: 50, onf: 'local street' }),
    onfActivity: one({ ruleClass: 'urban-street', limit: 50, onf: 'ACTIVITY STREET' }),
    onfMain: one({ ruleClass: 'urban-connector', limit: 50, onf: 'Main Street' }),
    onfNone: one({ ruleClass: 'urban-street', limit: 50, onf: 'Nothing like it' }),
    onfNumber: one({ ruleClass: 'urban-street', limit: 50, onf: 5 }),
    limitString: one({ ruleClass: 'urban-street', limit: '50' }),
    limitBlank: one({ ruleClass: 'urban-street', limit: '' }),
    limitText: one({ ruleClass: 'urban-street', limit: 'fast' }),
    posted: sets(O, cxs(['lidc0eysc'], { ruleClass: 'urban-connector' }), { 'scenarios.1.links.lidc0eysc.speedLimit': 70 }),
    postedNslr: sets(O, cxs(['lidc0eysc'], { ruleClass: 'urban-connector' }), { 'scenarios.1.links.lidc0eysc.nslr': { v: 80 }, 'scenarios.0.links.lidc0eysc.nslr': { v: 40 } }),
    currentFromBase: sets(O, cxs(['lidc0eysc'], { ruleClass: 'urban-connector', limit: 60 }), { 'scenarios.0.links.lidc0eysc.speedLimit': 40, 'scenarios.0.links.lidc0eysc.speed': 45 }),
    currentNewRoad: sets(O, cxs(['lnew'], { ruleClass: 'urban-connector', limit: 60 })),
    allFlags: sets(O, cxs(['lidc0eysc'], { limit: 110 }), cxs(['lidc0ngt6'], { ruleClass: 'isz-urban', limit: 90, variable: true }), cxs(['lidc0phd9'], { ruleClass: 'urban-street', limit: 30, onf: 'civic space' })),
    activeBase: sets({ active: 'sidc01d0p' }, cxs(MAIN, { ruleClass: 'urban-connector', limit: 60 }, 0)),
    cxNotObject: sets(O, { 'scenarios.1.links.lidc0eysc.cx': 5 }),
    cxEmpty: sets(O, { 'scenarios.1.links.lidc0eysc.cx': {} }),
  };
  for (const [k, s] of Object.entries(flagSets)) await add('speed', `speedCheck ${k}`, 'speedCheck', F, s, P => X.speedCheck(P));
  await add('speed', 'speedCheck scenario argument', 'speedCheck', F, sets(O, cxs(MAIN, { ruleClass: 'urban-connector', limit: 60 }, 0), cxs(['lidc0ngt6'], { limit: 30 }, 1)), P => X.speedCheck(P, P.scenarios[0]), { sc: 0 });
  await add('speed', 'speedCheck grid none', 'speedCheck', 'grid', {}, P => X.speedCheck(P));

  /* ---------- school speed records ---------- */
  const school = {
    none: {},
    some: { 'scenarios.1.links.lidc0eysc.varSpeed': { v: 30, desc: 'School zone 8-9am and 2:30-3:30pm' }, 'scenarios.1.links.lidc0fkhj.varSpeed': { v: '60', desc: 'SCHOOL' }, 'scenarios.1.links.lidc0gzyt.varSpeed': { v: 80, desc: 'school buses' },
      'scenarios.1.links.lidc0hcs9.varSpeed': { v: 40, desc: 'Roadworks' }, 'scenarios.1.links.lidc0ijt9.varSpeed': { v: 'school' }, 'scenarios.1.links.lidc0jdqb.varSpeed': { v: 20 }, 'scenarios.1.links.lidc0lb8r.varSpeed': { v: 'x', desc: 'school' },
      'scenarios.1.links.lidc0ngt6.varSpeed': {}, 'scenarios.1.links.lidc0phd9.varSpeed': { v: 50, desc: '' } },
    unnamed: { 'scenarios.1.links.lidc0eysc.name': '', 'scenarios.1.links.lidc0eysc.varSpeed': { v: 30, desc: 'school' } },
  };
  for (const [k, s] of Object.entries(school)) await add('speed', `schoolSpeeds ${k}`, 'schoolSpeeds', F, sets(O, s), P => X.schoolSpeeds(P.scenarios[1]), { sc: 1 });

  /* ---------- crash history by section ---------- */
  // crashes beside the Main Road links (lat -37.1, lng 174.95 to 174.9696) and the side roads: years, severities and locations, some outside 25 m
  const at = (lng, lat, sev, year, loc) => (loc === undefined ? [lat, lng, sev, year] : [lat, lng, sev, year, loc]);
  const crashes = [];
  const mainLat = -37.1;
  const xs = [174.9514, 174.9542, 174.957, 174.9598, 174.9625, 174.9653, 174.968, 174.9556, 174.9612];
  let i = 0;
  for (const year of [2021, 2022, 2023, 2024, 2025, 2026]) for (const sev of ['F', 'S', 'M', 'N', 'N']) crashes.push(at(xs[i++ % 7], mainLat + 0.00008, sev, year, 'Main Road'));
  crashes.push(at(174.9556, -37.0989, 'S', 2023, 'Side 2N / Main Road'), at(174.9556, -37.0989, 'M', 2023, 'Main Road / Side 2N'), at(174.9556, -37.1011, 'F', 2024, 'Side 2S'), at(174.9612, -37.0989, 'S', 2022, 'Side 4N'),
    at(174.9556, -37.0997, 'M', 2025, 'Side 2N / Main Road'), at(174.9556, -37.1, 'N', 2025, 'MAIN ROAD (SH1)'), at(174.9556, -37.1, 'N', 2025, 'Main Rd'), at(174.9, -37.2, 'F', 2025, 'Nowhere'), at(174.9514, mainLat + 0.0002, 'F', 2025, 'Main Road'),
    at(174.9514, mainLat + 0.00005, 'X', 2022, 'Main Road'), at(174.9514, mainLat + 0.00005, '', 2022, 'Main Road'), at(174.9514, mainLat + 0.00005, 'FS', 2022, 'Main Road'), at(174.9514, mainLat + 0.00005, 'S', 0, 'Main Road'),
    at(174.9514, mainLat + 0.00005, 'S', '2023', 'Main Road'), at(174.9514, mainLat + 0.00005, 'M', 'x', 'Main Road'), at(174.9514, mainLat + 0.00005, 'S', 2023, ''), at(174.9514, mainLat + 0.00005, 'S', 2023), at(174.9514, mainLat + 0.00005, 'M', 2027, 'Main Road'));
  const cas = o => ({ 'data.loaded.cas': o });
  const crashSets = {
    none: {},
    empty: { 'data.crashes': [] },
    notArray: { 'data.crashes': 'x' },
    all: { 'data.crashes': crashes },
    cas: { 'data.crashes': crashes, ...cas({ from: 2021, to: 2025, ytd: 2026, lagYear: 2025 }) },
    casNoYtd: { 'data.crashes': crashes, ...cas({ from: 2022, to: 2025 }) },
    casToOnly: { 'data.crashes': crashes, ...cas({ to: 2024 }) },
    casFromOnly: { 'data.crashes': crashes, ...cas({ from: 2023 }) },
    casStrings: { 'data.crashes': crashes, ...cas({ from: '2022', to: '2025', ytd: '2026', lagYear: 2025 }) },
    casBackwards: { 'data.crashes': crashes, ...cas({ from: 2026, to: 2021 }) },
    casYtdInside: { 'data.crashes': crashes, ...cas({ from: 2021, to: 2025, ytd: 2024 }) },
    casNone: { 'data.crashes': crashes, 'data.loaded': {} },
    one: { 'data.crashes': [at(174.9514, mainLat + 0.00005, 'F', 2024, 'Main Road')] },
  };
  const idsets = [['lidc0chmq'], MAIN.slice(0, 3), MAIN, ['lidc0lb8r'], ['lidc0ngt6', 'lidc0lb8r'], ['lidc0phd9'], []];
  for (const [k, s] of Object.entries(crashSets)) {
    for (const ids of idsets) {
      if (ids.length < 1 && k !== 'all') continue;
      await add('speed', `sectionCrashes ${k} ${ids.length ? ids.join(',') : 'none'}`, 'sectionCrashes', F, sets(O, s), P => X.sectionCrashes(P, P.scenarios[0], ids.map(id => P.scenarios[0].links[id])), { sc: 0, ids });
    }
    for (const since of [null, 2022, 2024, 2025, 2030, 0]) {
      await add('speed', `crashHistory ${k} since ${since}`, 'crashHistory', F, sets(O, s), P => X.crashHistory(P, P.scenarios[0], MAIN.map(id => P.scenarios[0].links[id]), since), { sc: 0, ids: MAIN, since });
    }
    await add('speed', `crashHistory ${k} one link`, 'crashHistory', F, sets(O, s), P => X.crashHistory(P, P.scenarios[0], [P.scenarios[0].links.lidc0chmq], 2023), { sc: 0, ids: ['lidc0chmq'], since: 2023 });
  }

  /* ---------- the cost benefit disclosure statement ---------- */
  const lim = (ids, limit, cx = {}, sc = 1) => cxs(ids, { ruleClass: 'urban-connector', limit, ...cx }, sc);
  const cbSets = {
    main: sets(O, lim(MAIN, 40)),
    mainUp: sets(O, lim(MAIN, 60)),
    same: sets(O, lim(MAIN, 50)),
    measured: sets(O, lim(MAIN, 40), cxs(MAIN, { limit: 40, meanSpeed: 46.5, meanSpeedSrc: 'Radar survey 2025' }, 0)),
    measuredOpt: sets(O, lim(MAIN, 40, { meanSpeed: 44 })),
    measuredNoSrc: sets(O, lim(MAIN, 40), cxs(MAIN.slice(0, 2), { meanSpeed: 48 }, 0)),
    measuredSrcOnBase: sets(O, lim(MAIN, 40), cxs(MAIN, { meanSpeed: 48, meanSpeedSrc: 'Base src' }, 0)),
    measuredZero: sets(O, lim(MAIN, 40, { meanSpeed: 0 })),
    megamaps: sets(O, lim(MAIN, 40), { 'scenarios.0.links.lidc0chmq.megamaps': { mean: 41.5 }, 'scenarios.0.links.lidc0dlsv.megamaps': { mean: 39 }, 'scenarios.0.links.lidc0eysc.megamaps': { mean: '' }, 'scenarios.0.links.lidc0fkhj.megamaps': 0 }),
    megamapsAndMeasured: sets(O, lim(MAIN, 40, { meanSpeed: 47 }), { 'scenarios.0.links.lidc0chmq.megamaps': { mean: 41.5 } }),
    since: sets(O, lim(MAIN, 40, { since: 2024 })),
    sinceMixed: sets(O, lim(MAIN.slice(0, 4), 40, { since: 2023 }), cxs(MAIN.slice(4), { since: 2025, limit: 40 }), { 'data.crashes': [], 'data.loaded.cas': { from: 2021, to: 2025 } }),
    consultation: sets(O, lim(MAIN, 40), { 'complianceExtra.speed.consultationChanged': true }),
    cost: sets(O, lim(MAIN, 40), { 'complianceExtra.speed.cost': 250000, 'complianceExtra.speed.tool': 'Excel model' }),
    costString: sets(O, lim(MAIN, 40), { 'complianceExtra.speed.cost': '12000.4', 'complianceExtra.speed.tool': '' }),
    costNegative: sets(O, lim(MAIN, 40), { 'complianceExtra.speed.cost': -5 }),
    assumptions: sets(O, lim(MAIN, 40), { 'settings.complianceExtra.meanShare': 0.8, 'settings.complianceExtra.dsiExponent': 4, 'settings.complianceExtra.injuryExponent': 2.5 }),
    lowLimit: sets(O, lim(MAIN, 10)),
    baseLimits: sets(O, lim(MAIN, 40), { 'scenarios.0.links.lidc0chmq.speedLimit': 70 }),
    dividedRoad: sets(O, lim(['lidc0chmq', 'lidc0chmq-r'], 40)),
    dividedOneWay: sets(O, lim(['lidc0chmq'], 40)),
    fewLinks: sets(O, lim(MAIN.slice(0, 2), 40)),
    noModelResults: sets(O, lim(MAIN, 40), { 'scenarios.0.res': {} }),
    pmOnly: sets(O, lim(MAIN, 40), { 'scenarios.0.res.AM': null }),
    newRoadLink: sets(O, lim(['lnew'], 40)),
    newRoadFlag: sets(O, lim(MAIN, 40, { newRoad: true })),
    baseProposed: sets(O, lim(MAIN, 40), { 'scenarios.0.links.lidc0chmq.status': 'proposed' }),
    school: sets(O, lim(MAIN, 40, { school: true })),
    schoolBase: sets(O, lim(MAIN, 40), cxs(MAIN.slice(2, 3), { school: true }, 0)),
    shAuthority: sets(O, lim(MAIN, 100), { 'scenarios.1.links.lidc0chmq.cls': 'motorway' }),
  };
  const cbIds = { main: MAIN, one: ['lidc0chmq'], pair: ['lidc0chmq', 'lidc0chmq-r'], few: MAIN.slice(0, 2), mix: [MAIN[0], 'nope', MAIN[1]], none: [], missing: ['nope'], newRoad: ['lnew', MAIN[0]], side: ['lidc0lb8r', 'lidc0ngt6'] };
  for (const [k, s] of Object.entries(cbSets)) {
    const idsKeys = k === 'dividedRoad' || k === 'dividedOneWay' ? ['pair', 'one'] : k === 'newRoadLink' ? ['newRoad', 'main'] : ['main', 'one', 'few'];
    for (const ik of idsKeys) await add('speed', `cbds ${k} ${ik}`, 'cbds', F, s, P => X.cbds(P, cbIds[ik]), { ids: cbIds[ik] });
  }
  for (const [ik, ids] of Object.entries(cbIds)) await add('speed', `cbds ids ${ik}`, 'cbds', F, sets(O, lim(MAIN, 40), lim(['lidc0lb8r', 'lidc0ngt6'], 30)), P => X.cbds(P, ids), { ids });
  await add('speed', 'cbds ids undefined', 'cbds', F, sets(O, lim(MAIN, 40)), P => X.cbds(P, undefined), { ids: null });
  await add('speed', 'cbds scenario option', 'cbds', F, sets(O, lim(MAIN, 40, { meanSpeed: 45 })), P => X.cbds(P, MAIN, { sc: P.scenarios[1] }), { ids: MAIN, sc: 1 });
  await add('speed', 'cbds active is the base', 'cbds', F, sets({ active: 'sidc01d0p' }, lim(MAIN, 40, {}, 0)), P => X.cbds(P, MAIN), { ids: MAIN });
  await add('speed', 'cbds other scenario', 'cbds', F, sets(O, lim(MAIN, 40, {}, 2)), P => X.cbds(P, MAIN, { sc: P.scenarios[2] }), { ids: MAIN, sc: 2 });
  await add('speed', 'cbds grid', 'cbds', 'grid', { active: 's-opt', ...Object.fromEntries(['lidc0r4yn', 'lidc0sold'].map(id => [`scenarios.2.links.${id}.cx`, { limit: 40, ruleClass: 'urban-connector' }])) }, P => X.cbds(P, ['lidc0r4yn', 'lidc0sold']), { ids: ['lidc0r4yn', 'lidc0sold'] });

  /* ---------- CSV, report, export (with a stored statement) ---------- */
  const val = async (fixture, s, f) => (await record(c, fixture, s, f)).ok;
  const stored = {
    main: await val(F, cbSets.measured, P => X.cbds(P, MAIN)),
    notOk: await val(F, cbSets.newRoadLink, P => X.cbds(P, ['lnew'])),
    warn: await val(F, cbSets.same, P => X.cbds(P, MAIN)),
    cost: await val(F, cbSets.cost, P => X.cbds(P, ['lidc0chmq'])),
  };
  const outSets = {
    none: O,
    checks: secSets.mainRoad,
    allFlags: flagSets.allFlags,
    divided: secSets.divided,
    storedMain: sets(secSets.mainRoad, { 'complianceExtra.speed.cbds': stored.main }),
    storedNotOk: sets(secSets.mainRoad, { 'complianceExtra.speed.cbds': stored.notOk }),
    storedWarn: sets(flagSets.allFlags, { 'complianceExtra.speed.cbds': stored.warn }),
    storedOnly: sets(O, { 'complianceExtra.speed.cbds': stored.main }),
    storedCost: sets(secSets.divided, { 'complianceExtra.speed.cbds': stored.cost }),
    quoting: sets(O, cxs(['lidc0ngt6'], { ruleClass: 'urban-street', limit: 30 }), { 'scenarios.1.links.lidc0ngt6.name': 'Quote, "Road" =1' }),
    oddCbds: sets(secSets.mainRoad, { 'complianceExtra.speed.cbds': { ok: true, name: 'N', headings: [{ title: 'T', rule: 'R', rows: [['a', 1], ['b', null], ['c', 'x<y']], basis: 'B & B' }, { title: 'U', rule: 'R2', rows: [] }], warn: ['w <1>'], watermark: 'WM', disclaimer: 'D <9>', decision: 'Dec' } }),
    cbdsFalse: sets(secSets.mainRoad, { 'complianceExtra.speed.cbds': { ok: false, why: 'x' } }),
  };
  for (const [k, s] of Object.entries(outSets)) {
    await add('speed', `speedCSV ${k}`, 'speedCSV', F, s, P => X.speedCSV(P));
    await add('speed', `export speed ${k}`, 'export-speed', F, s, P => ({ text: exportBuild(c, 'speed', P), enabled: exportEnabled(c, 'speed') }));
    await add('speed', `report speed ${k}`, 'report-speed', F, s, P => reportOf(c, 'compliance-extra-speed', P, undefined));
    await add('speed', `report speed ${k} with context`, 'report-speed-ctx', F, s, P => reportOf(c, 'compliance-extra-speed', P, rctx));
  }
}
