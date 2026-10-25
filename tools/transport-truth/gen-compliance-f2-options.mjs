// Part f2, options and alternatives, plan change mode, full build-out (compliance-extra.js section 8, first half)
// The projects: the saved corridor project with extra scenarios (a bus lane and cycleway option, a mitigation package, a later-year scenario, a
// scenario with no results, a scenario with unfunded and committed projects), results from the engine for each.

const lite = a => (a && a.base ? { ...a, base: a.base.name } : a);
const norm = s => (s && s.id ? { ...s, id: '<uid>' } : s);

export async function buildOptFixture(env) {
  const { c, withResults, saveFixture } = env;
  const P = await withResults(c, 'corridor', { rerun: true, mutate: (P, c) => {
    const opt = P.scenarios[1];
    const copy = (src, id, name, f) => { const s = c.Scenarios.copy(src); s.id = id; s.name = name; if (f) f(s); P.scenarios.push(s); return s; };
    copy(opt, 's-bus', 'Bus lane and cycleway', s => {
      s.links.lidc0eysc.busLane = true;
      s.links.lidc0fkhj.cycle = 'separated'; s.links.lidc0gzyt.cycle = 'painted'; s.links.lidc0lb8r.cycle = 'separated';
      s.nodes.nidc07wsv.upgrade = true; s.nodes.nidc0942h.control = 'signals';
    });
    copy(P.scenarios[2], 's-mit', 'Mitigation package', s => { s.role = 'mitigation'; s.links.lidc0chmq.lanesAB = 3; s.nodes.nidc0bhhv.upgrade = true; });
    copy(P.scenarios[0], 's-late', 'Later year', s => { s.year = 2040; s.zones.zDev.lu.residential = 900; });
    copy(opt, 's-none', 'Not run', () => { });
    copy(opt, 's-unf', 'Unfunded and committed projects', s => {
      s.links.lnorth = { id: 'lnorth', a: 'nidc0kim3', b: 'nidc0olv4', name: 'North Link', cls: 'collector', lanesAB: 1, lanesBA: 1, speed: 50, capPerLane: null, status: 'proposed', busLane: false, cycle: 'none', pts: [], stage: 'unfunded' };
      s.links.lsouth = { id: 'lsouth', a: 'nidc0mlwa', b: 'nidc0qw7h', name: 'South Link', cls: 'local', lanesAB: 1, lanesBA: 1, speed: 50, capPerLane: null, status: 'proposed', busLane: false, cycle: 'none', pts: [], stage: 'committed', funding: 'unfunded' };
      s.links.lwest = { id: 'lwest', a: 'nidc0szce', b: 'nidc0olv4', name: 'North Link', cls: 'collector', lanesAB: 1, lanesBA: 1, speed: 50, capPerLane: null, status: 'proposed', busLane: false, cycle: 'none', pts: [], stage: 'unfunded' };
      s.links.lnone = { id: 'lnone', a: 'nidc0szce', b: 'nidc0kim3', status: 'proposed', lanesAB: 1, lanesBA: 1, speed: 50, cls: 'bogus', busLane: false, cycle: 'none', pts: [], stage: 'unfunded' };
    });
  } });
  P.scenarios.find(s => s.id === 's-none').res = {};
  return saveFixture('f2-opt', P);
}

export async function runOptions(env) {
  const { c, X, add, rctx } = env;
  await buildOptFixture(env);
  const F = 'f2-opt';
  const ids = ['sidc01d0p', 's-opt', 's-bus', 's-mit', 's-late', 's-none', 's-unf'];
  const NOW_ACTIVE = { active: 's-opt' };

  /* ---------- optionList and alternatives ---------- */
  const optSets = {
    auto: {},
    autoOpt: NOW_ACTIVE,
    autoBus: { active: 's-bus' },
    autoMit: { active: 's-mit' },
    late: { active: 's-late' },
    none: { active: 's-none' },
    unf: { active: 's-unf' },
    baseAsOpt: { active: 'sidc01d0p', base: 's-opt' },
    baselineRole: { active: 's-opt', base: 's-opt', 'scenarios.0.role': 'baseline' },
    existingRole: { active: 's-opt', 'scenarios.2.role': 'existing' },
    badActive: { active: 'nope' },
    tagged: { active: 's-opt', 'complianceExtra.options': [
      { id: 'o1', name: 'Signals at Side 2', scId: 's-opt', hierarchy: 'optimise', cost: 2.5, timing: '2028', dsi: -0.4, notes: 'Signalise the Side 2 junction' },
      { id: 'o2', name: 'Bus lane', scId: 's-bus', hierarchy: 'demand', cost: '', dsi: '', notes: '' },
      { id: 'o3', name: '', scId: 's-mit', hierarchy: 'capacity', cost: 0, dsi: 0.3 },
      { id: 'o4', scId: 's-late', hierarchy: 'integrate' },
      { id: 'o5', name: 'Maintain', scId: 's-none', hierarchy: 'maintain' },
      { id: 'o6', name: 'Missing scenario', scId: 'nope', hierarchy: 'optimise' },
      { id: 'o7', name: 'Unfunded', scId: 's-unf', hierarchy: '', cost: 12.5, timing: '2031' },
    ] },
    taggedNoDemand: { active: 's-opt', 'complianceExtra.options': [
      { id: 'o1', name: 'Capacity', scId: 's-bus', hierarchy: 'capacity' }, { id: 'o2', name: 'Integrate', scId: 's-mit', hierarchy: 'integrate' },
    ] },
    taggedAll: { active: 's-opt', 'complianceExtra.options': [
      { id: 'o1', name: 'A', scId: 's-bus', hierarchy: 'demand' }, { id: 'o2', name: 'B', scId: 's-mit', hierarchy: 'optimise' }, { id: 'o3', name: 'C', scId: 's-unf', hierarchy: 'capacity' },
    ] },
    noIds: { active: 's-opt', 'complianceExtra.options': [{ name: 'No id', scId: 's-bus', hierarchy: 'optimise' }, { name: 'No id 2', scId: 's-mit', hierarchy: 'demand' }] },
    emptyList: { active: 's-opt', 'complianceExtra.options': [] },
    onlyBase: { active: 'sidc01d0p', scenarios: null },
    yearsDiffer: { active: 's-opt', 'scenarios.1.year': 2040 },
    yearsDifferBase: { active: 's-opt', 'scenarios.0.year': 2027 },
  };
  delete optSets.onlyBase;
  for (const [k, sets] of Object.entries(optSets)) {
    await add('options', `alternatives ${k}`, 'alternatives', F, sets, P => lite(X.alternatives(P)));
    await add('options', `optionList ${k}`, 'optionList', F, sets, P => X.optionList(P).map(o => ({ ...o, sc: o.sc && o.sc.id })));
  }
  await add('options', 'alternatives corridor same scenario', 'alternatives', 'corridor', {}, P => lite(X.alternatives(P)));
  await add('options', 'alternatives grid', 'alternatives', 'grid', {}, P => lite(X.alternatives(P)));
  await add('options', 'alternatives grid option', 'alternatives', 'grid', { active: 's-opt' }, P => lite(X.alternatives(P)));
  await add('options', 'alternatives grid option vs do-minimum', 'alternatives', 'grid', { active: 's-opt', base: 's-dm' }, P => lite(X.alternatives(P)));
  await add('options', 'alternatives grid dm vs option', 'alternatives', 'grid', { active: 's-dm', base: 's-opt' }, P => lite(X.alternatives(P)));
  await add('options', 'alternatives grid tagged', 'alternatives', 'grid', { active: 's-opt', base: 's-dm', 'complianceExtra.options': [{ id: 'g1', name: 'Grid option', scId: 's-opt', hierarchy: 'optimise', cost: '3.2', dsi: '-1.5' }] }, P => lite(X.alternatives(P)));
  await add('options', 'alternatives grid settings econ', 'alternatives', 'grid', { active: 's-opt', base: 's-dm', 'settings.econ.activity': 'commercial' }, P => lite(X.alternatives(P)));

  /* ---------- alternative sections by pathway ---------- */
  const pathways = ['', 'rc-landuse', 'rc-subdivision', 'precinct', 'pc-private', 'pc-council', 'nor', 'outline-plan', 'ftaa-consent', 'ftaa-nor', 'nltf', 'speed-limit', 'dc-iff', 'tou'];
  for (const pw of pathways) for (const sig of [false, true]) for (const li of ['', 'yes', 'no']) {
    if (li && !['nor', 'outline-plan'].includes(pw)) continue;
    if (sig && !['rc-landuse', 'rc-subdivision', 'precinct', 'ftaa-consent', 'nor', 'outline-plan'].includes(pw)) continue;
    await add('options', `alternativeSections ${pw || 'none'} sig=${sig} land=${li || 'blank'}`, 'alternativeSections', 'corridor',
      { 'compliance.ctx.pathway': pw, 'complianceExtra.ctx.sigEffect': sig, 'complianceExtra.ctx.landInterest': li }, P => X.alternativeSections(P));
  }
  // the land interest from the Compliance tab beats this tab's
  await add('options', 'alternativeSections land interest from the Compliance tab', 'alternativeSections', 'corridor',
    { 'compliance.ctx.pathway': 'nor', 'compliance.ctx.landInterest': 'no', 'complianceExtra.ctx.landInterest': 'yes' }, P => X.alternativeSections(P));

  /* ---------- zoned capacity: mergeZoned, fullBuildout, createFullBuildout ---------- */
  // createFullBuildout and what it did to the project (the new scenario's id is random: replaced)
  const cfb = (P, i) => {
    const before = P.scenarios.length, ns = X.createFullBuildout(P, i === undefined ? undefined : P.scenarios[i]);
    return { ns: norm(ns), n: P.scenarios.length - before, names: P.scenarios.map(s => s.name), last: ns ? P.scenarios[P.scenarios.length - 1] === ns : null };
  };
  const merges = [
    [{ residential: 300, school: 400 }, { apartment: 800 }],
    [{ residential: 300, school: 400 }, { residential: 500, retail: 1200 }],
    [{ residential: 300, apartment: 50, retirement: 20, school: 400 }, { apartment: 800 }],
    [{ residential: 300, apartment: 50, retirement: 20 }, { retirement: 0 }],
    [{ residential: 300, apartment: 0, school: 400 }, { retail: 1200 }],
    [{ residential: 300 }, { residential: '', apartment: null, retail: '  ', office: '0' }],
    [{ residential: 300, retail: 100 }, { retail: '1200', bogus: 5, office: 'abc', supermarket: '2500.5', medical: 'Infinity', industrial: true }],
    [{ residential: 300, retail: 100 }, { retail: -50 }],
    [null, { apartment: 800, retail: 20 }],
    [{}, {}],
    [{ residential: 300 }, {}],
    [{ residential: 300 }, null],
    [{ residential: '300', retail: 'x' }, { apartment: '12' }],
    [{ 5: 100, residential: 3 }, { apartment: 1 }],
  ];
  for (const [i, [cur, zoned]] of merges.entries()) await add('options', `mergeZoned ${i}`, 'mergeZoned', 'corridor', {}, () => X.mergeZoned(cur, zoned), { current: cur, zoned });
  const zc = { 'scenarios.0.zones.zDev.cx.zoned': { apartment: 1200, retail: 800 }, 'scenarios.0.zones.zCom.cx.zoned': { residential: 100, supermarket: 3000 } };
  const fbSets = {
    none: {},
    dev: { 'scenarios.0.zones.zDev.cx.zoned': { apartment: 1200, retail: 800 } },
    devAndCom: zc,
    existing: { 'scenarios.0.zones.zEx.cx.zoned': { residential: 500 } },
    gateway: { 'scenarios.0.zones.gW.cx.zoned': { residential: 500 } },
    blankOnly: { 'scenarios.0.zones.zDev.cx.zoned': { residential: '', retail: null } },
    notObject: { 'scenarios.0.zones.zDev.cx.zoned': 5, 'scenarios.0.zones.zCom.cx.zoned': [1, 2] },
    proposedOnly: { 'complianceExtra.planChange.useProposed': true, 'scenarios.0.zones.zDev.cx.zonedProposed': { residential: 2000 } },
    proposedFallback: { 'complianceExtra.planChange.useProposed': true, 'scenarios.0.zones.zDev.cx.zoned': { apartment: 1200 }, 'scenarios.0.zones.zCom.cx.zonedProposed': { retail: 5 } },
    proposedNotUsed: { 'scenarios.0.zones.zDev.cx.zonedProposed': { residential: 2000 }, 'scenarios.0.zones.zCom.cx.zoned': { retail: 9 } },
    proposedBoth: { 'complianceExtra.planChange.useProposed': true, 'scenarios.0.zones.zDev.cx.zoned': { apartment: 1200 }, 'scenarios.0.zones.zDev.cx.zonedProposed': { apartment: 2400, retail: 100 } },
    named: { 'scenarios.0.zones.zDev.cx.zoned': { apartment: 1200 }, 'scenarios.0.zones.zDev.name': '', 'scenarios.0.zones.zCom.cx.zoned': { retail: 4 } },
  };
  for (const [k, sets] of Object.entries(fbSets)) {
    await add('options', `fullBuildout ${k}`, 'fullBuildout', F, sets, P => X.fullBuildout(P));
    await add('options', `fullBuildout ${k} scenario 1`, 'fullBuildout', F, { ...sets, 'scenarios.1.zones.zDev.cx.zoned': { residential: 77 } }, P => X.fullBuildout(P, P.scenarios[1]), { sc: 1 });
    await add('options', `createFullBuildout ${k}`, 'createFullBuildout', F, sets, P => cfb(P));
  }
  await add('options', 'createFullBuildout other scenario with year and role', 'createFullBuildout', F, { 'scenarios.2.zones.zDev.cx.zoned': { apartment: 5 }, 'scenarios.2.year': 2033, 'scenarios.2.role': 'proposal', 'scenarios.2.zones.zDev.buildout': 40 }, P => cfb(P, 2), { sc: 2 });
  await add('options', 'createFullBuildout proposed', 'createFullBuildout', F, { 'complianceExtra.planChange.useProposed': true, 'scenarios.0.zones.zDev.cx.zonedProposed': { residential: 2000 }, 'scenarios.0.role': 'mitigation' }, P => cfb(P));

  /* ---------- rltpRelied ---------- */
  const rltp = {
    none: {},
    committed: { 'scenarios.0.links.lidc0chmq.stage': 'committed', 'scenarios.0.links.lidc0chmq.funding': 'funded', 'scenarios.0.links.lidc0dlsv.stage': 'committed', 'scenarios.0.links.lidc0dlsv.funding': 'programmed',
      'scenarios.0.links.lidc0eysc.stage': 'committed', 'scenarios.0.links.lidc0fkhj.stage': 'committed', 'scenarios.0.links.lidc0fkhj.funding': 'bogus', 'scenarios.0.links.lidc0gzyt.stage': 'committed', 'scenarios.0.links.lidc0gzyt.status': 'closed' },
    register: { register: [{ id: 'r1', name: 'Mill Road Stage 1', type: 'network', status: 'Fast-track' }, { id: 'r2', name: 'School expansion', type: 'education' }, { id: 'r3', name: 'Rail station', type: 'Transport hub', status: '' },
      { id: 'r4', name: 'Local road', type: 'ROAD upgrade', status: 'Consented' }, { id: 'r5', type: 'network' }, { id: 'r6', name: 'Housing', type: 'housing', status: 'Built' }] },
    registerOnly: { register: [{ id: 'r1', name: 'Mill Road Stage 1', type: 'network', status: 'Fast-track' }, { id: 'r1b', name: 'Mill Road Stage 1', type: 'network', status: 'Fast-track' }] },
    many: { register: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11].map(i => ({ id: 'r' + i, name: 'Project ' + i, type: 'network', status: 'S' + i })) },
    exactlyEight: { register: [1, 2, 3, 4, 5, 6, 7, 8].map(i => ({ id: 'r' + i, name: 'Project ' + i, type: 'network', status: 'S' + i })) },
    activeUnf: { active: 's-unf' },
  };
  for (const [k, sets] of Object.entries(rltp)) await add('options', `rltpRelied ${k}`, 'rltpRelied', F, sets, P => X.planChange(P).policy[0].text);

  /* ---------- plan change ---------- */
  const pcSets = {
    none: {},
    councilBefore: { 'compliance.ctx.pathway': 'pc-council', 'compliance.ctx.notificationDate': '2026-09-01' },
    councilAfter: { 'compliance.ctx.pathway': 'pc-council', 'compliance.ctx.notificationDate': '2026-11-01' },
    councilNone: { 'compliance.ctx.pathway': 'pc-council' },
    privateBefore: { 'compliance.ctx.pathway': 'pc-private', 'compliance.ctx.notificationDate': '2027-06-01' },
    privateAfter: { 'compliance.ctx.pathway': 'pc-private', 'compliance.ctx.notificationDate': '2028-02-01' },
    privateBar: { 'compliance.ctx.pathway': 'pc-private', 'compliance.ctx.notificationDate': '2028-01-01' },
    privateNone: { 'compliance.ctx.pathway': 'pc-private' },
    kindOwn: { 'complianceExtra.planChange.kind': 'pc-council', 'complianceExtra.planChange.notification': '2026-12-25' },
    kindOwnPrivate: { 'complianceExtra.planChange.kind': 'pc-private', 'complianceExtra.planChange.notification': '2028-03-04' },
    kindOwnBadDate: { 'complianceExtra.planChange.kind': 'pc-private', 'complianceExtra.planChange.notification': 'tomorrow' },
    notPc: { 'compliance.ctx.pathway': 'rc-landuse', 'complianceExtra.planChange.kind': 'pc-private' },
    options: { active: 's-opt', 'compliance.ctx.pathway': 'pc-private', 'compliance.ctx.notificationDate': '2028-02-01' },
    optionsTagged: { active: 's-opt', 'compliance.ctx.pathway': 'pc-council', 'compliance.ctx.notificationDate': '2026-09-01', 'complianceExtra.options': [
      { id: 'o1', name: 'Signals at Side 2', scId: 's-opt', hierarchy: 'optimise', cost: 2.5, timing: '2028', dsi: -0.4 }, { id: 'o2', name: 'Bus lane', scId: 's-bus', hierarchy: 'demand', cost: 0.05, dsi: 1.25 },
      { id: 'o3', name: 'Unfunded', scId: 's-unf', hierarchy: 'capacity', timing: 'tbc' }, { id: 'o4', name: 'Not run', scId: 's-none', hierarchy: 'maintain' }, { id: 'o5', name: 'Later', scId: 's-late', cost: 150 }] },
    unfunded: { active: 's-bus', 'compliance.ctx.pathway': 'pc-private', 'complianceExtra.options': [{ id: 'o3', name: 'Unfunded', scId: 's-unf', hierarchy: 'capacity' }, { id: 'o4', name: 'Unfunded 2', scId: 's-unf', hierarchy: 'demand', timing: '2030' }] },
    roads: { 'compliance.ctx.pathway': 'pc-council', 'scenarios.0.links.lidc0chmq.status': 'proposed', 'scenarios.0.links.lidc0dlsv.cx': { rasf: 'P3 M2', fn: 'Draft arterial' }, 'scenarios.0.links.lidc0eysc.cx': { fn: 'Main street' },
      'scenarios.0.links.lidc0fkhj.cx': { rasf: 'P2 M3' }, 'scenarios.0.links.lidc0gzyt.fc': { gt: 'Arterial' }, 'scenarios.0.links.lidc0gzyt.cx': { rasf: 'x' }, 'scenarios.0.links.lidc0lb8r.cx': {},
      'scenarios.0.links.lidc0ngt6.cx': { fn: 'f' }, 'scenarios.0.links.lidc0phd9.cx': { rasf: 'r' }, 'scenarios.0.links.lidc0r8fd.cx': { rasf: 'r' }, 'scenarios.0.links.lidc0r8fd.cycle': 'separated', 'scenarios.0.links.lidc0r8fd.status': 'proposed',
      'scenarios.0.links.lidc0tv6u.cx': { rasf: 'r' }, 'scenarios.0.links.lidc0tv6u.cycle': 'weird', 'scenarios.0.links.lidc0tv6u.status': 'closed', 'scenarios.0.links.lidc0tv6u.cls': 'motorway', 'scenarios.0.links.lidc0phd9.cls': 'ramp',
      'scenarios.0.links.lidc0phd9.speedLimit': 70, 'scenarios.0.links.lidc0phd9.lanesAB': '3', 'scenarios.0.links.lidc0phd9.lanesBA': 1.9 },
    buildout: { 'compliance.ctx.pathway': 'pc-private', 'scenarios.0.zones.zDev.cx.zoned': { apartment: 1200, retail: 800 } },
    sens: { 'compliance.ctx.pathway': 'pc-private', 'report.sens.rows': [{ label: 'a' }, { label: 'b' }, { label: 'c' }] },
    sensEmpty: { 'compliance.ctx.pathway': 'pc-private', 'report.sens.rows': [] },
    triggers: { 'compliance.ctx.pathway': 'pc-private', 'complianceExtra.triggers': { rows: [] } },
    econHours: { active: 's-bus', 'compliance.ctx.pathway': 'pc-council', 'settings.econ.periodHours': { AM: 100, PM: 200 }, 'settings.complianceExtra.vocPerKm': 0.5 },
    econHoursOdd: { active: 's-bus', 'compliance.ctx.pathway': 'pc-council', 'settings.econ.periodHours': { AM: 'x' }, 'settings.complianceExtra.vocPerKm': '0.25' },
    lateNoCost: { active: 's-opt', 'compliance.ctx.pathway': 'pc-council', 'complianceExtra.options': [{ id: 'o1', scId: 's-late', hierarchy: 'demand' }, { id: 'o2', name: 'Not run', scId: 's-none' }] },
    s32Empty: { active: 's-opt', 'compliance.ctx.pathway': 'pc-council', 'complianceExtra.options': [{ id: 'o1', name: 'Not run', scId: 's-none' }] },
    unfStage: { active: 's-unf', 'compliance.ctx.pathway': 'pc-private', 'complianceExtra.options': [{ id: 'o1', name: 'Bus', scId: 's-bus', hierarchy: 'demand' }],
      'scenarios.6.links.lsouth.cx': { fn: 'Quiet street', rasf: 'P4 M1' }, 'scenarios.6.links.lwest.status': 'weird', 'scenarios.6.links.lwest.cx': { rasf: 'r' }, 'scenarios.6.links.lnone.cx': { fn: 'Bogus class road' } },
    unfStageNoOptions: { active: 's-unf', 'compliance.ctx.pathway': 'pc-private' },
    rltp: { 'compliance.ctx.pathway': 'pc-council', 'scenarios.0.links.lidc0chmq.stage': 'committed', 'scenarios.0.links.lidc0chmq.funding': 'funded', register: [{ id: 'r1', name: 'Mill Road Stage 1', type: 'network', status: 'Fast-track' }] },
  };
  for (const [k, sets] of Object.entries(pcSets)) {
    await add('options', `planChange ${k}`, 'planChange', F, sets, P => X.planChange(P));
    if (['none', 'kindOwn', 'optionsTagged', 'rltp', 'unfStage'].includes(k)) await add('options', `rltp text in planChange ${k}`, 'rltpRelied', F, sets, P => X.planChange(P).policy[0].text);
  }
  await add('options', 'planChange grid', 'planChange', 'grid', { active: 's-opt', base: 's-dm', 'compliance.ctx.pathway': 'pc-private', 'compliance.ctx.notificationDate': '2028-02-01' }, P => X.planChange(P));

  /* ---------- CSV exports and report sections ---------- */
  const csvSets = { auto: NOW_ACTIVE, tagged: optSets.tagged, bus: { active: 's-bus', 'complianceExtra.options': [{ id: 'o1', name: 'Quote, "comma"', scId: 's-opt', hierarchy: 'optimise', cost: 1.23456, dsi: 0.55555, notes: '=SUM(A1)' }] },
    none: {}, unf: { active: 's-unf' }, noRuns: { active: 's-none' } };
  for (const [k, sets] of Object.entries(csvSets)) {
    await add('options', `optionsCSV ${k}`, 'optionsCSV', F, sets, P => X.optionsCSV(P));
    await add('options', `export options ${k}`, 'export-options', F, sets, P => ({ text: exportBuild(c, 'options', P), enabled: exportEnabled(c, 'options') }));
    await add('options', `report alternatives ${k}`, 'report-alternatives', F, sets, P => reportOf(c, 'compliance-extra-alternatives', P, undefined));
    await add('options', `report alternatives ${k} with context`, 'report-alternatives-ctx', F, sets, P => reportOf(c, 'compliance-extra-alternatives', P, rctx));
  }
  await add('options', 'report alternatives nor significant', 'report-alternatives', F, { active: 's-opt', 'compliance.ctx.pathway': 'nor', 'complianceExtra.ctx.landInterest': 'no' }, P => reportOf(c, 'compliance-extra-alternatives', P, undefined));
  await add('options', 'report alternatives rc significant', 'report-alternatives-ctx', F, { active: 's-opt', 'compliance.ctx.pathway': 'rc-landuse', 'complianceExtra.ctx.sigEffect': true, 'complianceExtra.options': optSets.taggedAll['complianceExtra.options'] }, P => reportOf(c, 'compliance-extra-alternatives', P, rctx));
  await add('options', 'report alternatives grid', 'report-alternatives', 'grid', { active: 's-opt', base: 's-dm' }, P => reportOf(c, 'compliance-extra-alternatives', P, undefined));
  for (const [k, sets] of Object.entries({ ...pcSets, grid: {} })) {
    if (k === 'grid') continue;
    await add('options', `planChangeCSV ${k}`, 'planChangeCSV', F, sets, P => X.planChangeCSV(P));
    await add('options', `report planchange ${k}`, 'report-planchange', F, sets, P => reportOf(c, 'compliance-extra-planchange', P, undefined));
    if (!['none', 'councilBefore', 'privateAfter', 'optionsTagged', 'unfStage', 'roads', 'buildout', 's32Empty'].includes(k)) continue;
    await add('options', `export planchange ${k}`, 'export-planchange', F, sets, P => ({ text: exportBuild(c, 'planchange', P), enabled: exportEnabled(c, 'planchange') }));
    await add('options', `report planchange ${k} with context`, 'report-planchange-ctx', F, sets, P => reportOf(c, 'compliance-extra-planchange', P, rctx));
  }
}

// REPORT[id].render(P, ctx) of the Compliance+ module
export function reportOf(c, id, P, ctx) {
  const s = c.ComplianceExtra.REPORT.find(x => x.id === id);
  if (!s) throw new Error('no report section ' + id);
  return s.render(P, ctx);
}

// The registered export with this id (without the prefix): [id, label, ext, build, enabled]
export const exportDef = (c, id) => c.ComplianceExtra.__internal.EXPORTS.find(e => e[0] === id);
export const exportBuild = (c, id, P) => exportDef(c, id)[3](P);
export const exportEnabled = (c, id) => !!exportDef(c, id)[4]();
