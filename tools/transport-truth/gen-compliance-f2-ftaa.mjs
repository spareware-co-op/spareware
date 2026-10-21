// Part f2, the Fast-track package (compliance-extra.js section 8, second half): invitees, timeline, package, files, JSON bundle, export and report section
import { reportOf, exportBuild, exportEnabled } from './gen-compliance-f2-options.mjs';

// FNV-1a over a string (two seeds, 16 hex characters), the sign-off fingerprint of compliance.js: stands in for the model inputs text in recorded files
function fnv(str) {
  const imul = Math.imul, B = 2246822519 | 0;
  let a = 0x811c9dc5 | 0, b = (0x01000193 ^ 0x5bd1e995) | 0;
  for (let i = 0; i < str.length; i++) { const c = str.charCodeAt(i); a = imul(a ^ c, 16777619); b = imul(b ^ c, B); }
  return (a >>> 0).toString(16).padStart(8, '0') + (b >>> 0).toString(16).padStart(8, '0');
}
const model = 'reviewer/model-inputs.json';
// the files with the (very long) model inputs text replaced by its length and fingerprint
const lite = files => Object.fromEntries(Object.entries(files).map(([k, v]) => [k, k === model ? `<${v.length} characters ${fnv(v)}>` : v]));

export async function runFtaa(env) {
  const { c, X, C, add, rctx } = env;
  const F = 'f2-opt';
  const refs = C.DATA.POLICY.map(p => p.ref);
  const O = { active: 's-opt' };
  const FT = { 'compliance.ctx.pathway': 'ftaa-consent', ...O };

  /* ---------- invitees ---------- */
  const inv = {
    none: {},
    shFar: { 'scenarios.0.links.lidc0eysc.cls': 'motorway' },
    shNear: { 'scenarios.0.links.lidc0eysc.cls': 'motorway', 'settings.complianceExtra.shBuffer': 500 },
    shNearRamp: { 'scenarios.0.links.lidc0fkhj.cls': 'ramp', 'settings.complianceExtra.shBuffer': '600' },
    shMarked: { 'scenarios.0.links.lidc0eysc.cls': 'motorway', 'settings.complianceExtra.shBuffer': 500, 'complianceExtra.ftaa.nztaDesignation': true },
    shClosed: { 'scenarios.0.links.lidc0eysc.cls': 'motorway', 'scenarios.0.links.lidc0eysc.status': 'closed', 'settings.complianceExtra.shBuffer': 500 },
    twoSh: { 'scenarios.0.links.lidc0eysc.cls': 'motorway', 'scenarios.0.links.lidc0ngt6.cls': 'motorway', 'scenarios.0.links.lidc0gzyt.cls': 'ramp', 'settings.complianceExtra.shBuffer': 800 },
    shUser: { 'compliance.net.shUser.lidc0lb8r': true, 'settings.complianceExtra.shBuffer': 600 },
    shUserNot: { 'scenarios.0.links.lidc0eysc.cls': 'motorway', 'compliance.net.shUser.lidc0eysc': false, 'settings.complianceExtra.shBuffer': 500 },
    shLayer: { 'compliance.net.sh.lidc0fkhj': '16', 'settings.complianceExtra.shBuffer': 600 },
    shLayerBlank: { 'compliance.net.sh.lidc0fkhj': '', 'settings.complianceExtra.shBuffer': 600 },
    nztaOnly: { 'complianceExtra.ftaa.nztaDesignation': true },
    kiwirail: { 'complianceExtra.ftaa.kiwirailDesignation': true },
    both: { 'complianceExtra.ftaa.nztaDesignation': true, 'complianceExtra.ftaa.kiwirailDesignation': true },
    unnamedZone: { 'scenarios.0.links.lidc0eysc.cls': 'motorway', 'settings.complianceExtra.shBuffer': 500, 'scenarios.0.zones.zDev.name': '' },
    noDevZones: { 'scenarios.0.links.lidc0eysc.cls': 'motorway', 'settings.complianceExtra.shBuffer': 5000, 'scenarios.0.zones.zDev.role': 'existing' },
    activeOpt: { active: 's-bus', 'scenarios.4.links.lidc0eysc.cls': 'motorway', 'scenarios.4.links.lidc0eysc.name': 'State Highway 1', 'settings.complianceExtra.shBuffer': 500 },
  };
  for (const [k, sets] of Object.entries(inv)) await add('ftaa', `invitees ${k}`, 'invitees', F, sets, P => X.invitees(P));

  /* ---------- timeline ---------- */
  const inviteDates = ['', 'not a date', '2026-10-21', '2026-12-18', '2026-04-01', '2027-01-29', '2026-06-25', '2026-10-30T10:00:00Z', '2026-02-30', '2024-12-20', '2026-09-30', '2026-12-31'];
  const approvals = ['', 'bad', '2026-08-31', '2024-02-29', '2026-12-31', '2027-01-31', '2026-10-21', '2025-12-31T00:00:00'];
  for (const d of inviteDates) await add('ftaa', `ftaaTimeline invitation ${d || 'blank'}`, 'ftaaTimeline', F, { 'complianceExtra.ftaa.invitation': d }, P => X.ftaaTimeline(P));
  for (const d of approvals) await add('ftaa', `ftaaTimeline approval ${d || 'blank'}`, 'ftaaTimeline', F, { 'complianceExtra.ftaa.approval': d }, P => X.ftaaTimeline(P));
  await add('ftaa', 'ftaaTimeline both', 'ftaaTimeline', F, { 'complianceExtra.ftaa.invitation': '2026-11-02', 'complianceExtra.ftaa.approval': '2027-03-31' }, P => X.ftaaTimeline(P));

  /* ---------- the package ---------- */
  const recorded = {};
  refs.slice(0, 8).forEach((r, i) => { recorded[r] = [{ result: 'consistent', note: 'Meets it' }, { result: 'inconsistent' }, { result: 'not-relevant', note: 'n/a' }, { result: 'bogus', note: 'x' }, { note: 'only a note' }, {}, 'text', null][i]; });
  const trig = { rows: [{ label: 'Option (without upgrade)', upgrade: false, first: { pct: 40, dwellings: 800, before: 799.6, name: 'Main Road / Side 2', period: 'AM', why: 'DoS 0.96' } },
    { label: 'Mitigation (with upgrade)', upgrade: true, first: null }, { label: 'Other', upgrade: false, first: { pct: 75, before: 1200 } }] };
  const mit = [{ id: 'm1', ref: 'M1', measure: 'Signals at Side 2', effect: '', trigger: 'Before 800 dwellings', options: 'x', responsible: 'applicant', funder: 'applicant', mechanism: 'condition', committed: true },
    { id: 'm2', ref: '', measure: 'Bus lane', effect: 'J:nidc05pnh:AM', trigger: '', responsible: 'at', funder: 'council', mechanism: 's84a', committed: false }];
  const cond = [{ id: 'c1', mitigation: 'm1', metric: 'dwellings occupied', threshold: '800 dwellings', survey: 'Counts', frequency: 'Annual', response: 'Signals operational', effect: '', agreed: true },
    { id: 'c2', mitigation: 'm2', metric: '', threshold: '', agreed: false }];
  const pkg = {
    plain: O,
    consent: FT,
    nor: { ...O, 'compliance.ctx.pathway': 'ftaa-nor' },
    notFtaa: { ...O, 'compliance.ctx.pathway': 'rc-landuse' },
    sameScenario: {},
    matrix: { ...FT, 'complianceExtra.ftaa.matrix': recorded },
    matrixNone: { ...FT, 'complianceExtra.ftaa.matrix': null },
    reviewer: { ...FT, 'complianceExtra.ftaa.reliability': '85th/median 1.3', 'complianceExtra.ftaa.jobs': '12,000 within 30 min' },
    triggers: { ...FT, 'complianceExtra.triggers': trig },
    mitigation: { ...FT, 'complianceExtra.mitigation': mit, 'complianceExtra.conditions': cond, 'complianceExtra.triggers': trig },
    mitScenario: { ...FT, active: 's-bus' },
    noMitScenario: { ...FT, 'scenarios.3.role': '' },
    mismatchYears: { ...FT, 'scenarios.1.year': 2040 },
    notRun: { ...FT, active: 's-none' },
    unfunded: { ...FT, active: 's-unf' },
    precinct: { ...FT, 'complianceExtra.precinct.id': 'I450', 'complianceExtra.precinct.proposal.dwellings': 900, 'complianceExtra.precinct.upgrades.a1': 'operational' },
    precinctBreach: { ...FT, 'complianceExtra.precinct.id': 'I450', 'complianceExtra.precinct.proposal.dwellings': 5000, 'complianceExtra.precinct.proposal.retail': 99999 },
    parking: { ...FT, 'complianceExtra.parking.rows': [{ activity: 'office', qty: 3000, maxCars: 40, cars: 30, accessible: 2, cycleVisitor: 2, cycleSecure: 10, showers: 2 }, { activity: 'residential', qty: 100, cars: 60 }] },
    parkingBreach: { ...FT, 'complianceExtra.parking.rows': [{ activity: 'retail', qty: 5000, maxCars: 40, cars: 80, accessible: 0 }] },
    sensitivity: { ...FT, 'report.sens.rows': [{ label: 'Growth +20%', input: '+20%', trips: 3000.46, vht: 111.123, dVht: 0.0714, sentence: 'VHT up by 7.1%' }, { test: 'No bus', trips: 2999, vht: 98.7, dVht: null },
      { id: 'x', input: 5, trips: 3000, vht: 110, dVht: 'abc' }, { label: '', test: '', id: '' }, { label: 'Odd', trips: 'x', dVht: -0.12345 }] },
    sensitivityEmpty: { ...FT, 'report.sens.rows': [] },
    loaded: { ...FT, 'data.loaded': { cas: { at: 1790000000000, n: 120, licence: 'CC-BY 4.0' }, counts: { at: '2026-10-05', n: 4, source: 'AT counts' }, tmp: null, zero: { at: 0, n: 0 }, flow: { at: 1760000000000, source: 'TomTom' }, odd: 5, nl: { n: null, at: 1 } } },
    assumptions: { ...FT, 'settings.complianceExtra': { vocPerKm: 0.5, tripsPerSpace: '0.4', walkSpeed: 'x', bogus: 5, crashElasticity: 4 } },
    named: { ...FT, name: 'The "Truth" test, project' },
    unnamed: { ...FT, name: '' },
    dates: { ...FT, 'complianceExtra.ftaa.invitation': '2026-11-02', 'complianceExtra.ftaa.approval': '2027-03-31', 'complianceExtra.ftaa.nztaDesignation': true },
    shNear: { ...FT, 'scenarios.0.links.lidc0eysc.cls': 'motorway', 'settings.complianceExtra.shBuffer': 500 },
    allIn: { ...FT, 'complianceExtra.ftaa.matrix': recorded, 'complianceExtra.mitigation': mit, 'complianceExtra.conditions': cond, 'complianceExtra.triggers': trig, 'report.sens.rows': [{ label: 'a', trips: 1, vht: 2, dVht: 0.1 }],
      'complianceExtra.precinct.id': 'I451', 'complianceExtra.ftaa.reliability': 'r', 'complianceExtra.parking.rows': [{ activity: 'office', qty: 3000, maxCars: 40, cars: 30 }], 'complianceExtra.ftaa.kiwirailDesignation': true },
  };
  for (const [k, sets] of Object.entries(pkg)) {
    await add('ftaa', `ftaaPackage ${k}`, 'ftaaPackage', F, sets, P => X.ftaaPackage(P));
  }
  await add('ftaa', 'ftaaPackage grid', 'ftaaPackage', 'grid', { active: 's-opt', base: 's-dm', 'compliance.ctx.pathway': 'ftaa-nor' }, P => X.ftaaPackage(P));

  /* ---------- files, bundle, export, report ---------- */
  const fileSets = ['plain', 'consent', 'nor', 'matrix', 'mitigation', 'sensitivity', 'sensitivityEmpty', 'loaded', 'assumptions', 'named', 'unnamed', 'dates', 'allIn', 'parkingBreach', 'precinct'];
  for (const k of fileSets) {
    const sets = pkg[k];
    await add('ftaa', `ftaaFiles ${k}`, 'ftaaFiles', F, sets, P => { const { files } = X.ftaaFiles(P); return { manifest: Object.keys(files), files: lite(files) }; });
    await add('ftaa', `ftaaBundle ${k}`, 'ftaaBundle', F, sets, P => {
      const text = X.ftaaBundle(P), model_ = X.ftaaFiles(P).files[model];
      // the model inputs as the bundle quotes them are replaced by their length and fingerprint
      return text.split(JSON.stringify(model_)).join(JSON.stringify(`<${model_.length} characters ${fnv(model_)}>`));
    });
    await add('ftaa', `ftaaZip ${k}`, 'ftaaZip', F, sets, P => { const t = X.ftaaZip(P), m = X.ftaaFiles(P).files[model]; return t.split(JSON.stringify(m)).join(JSON.stringify(`<${m.length} characters ${fnv(m)}>`)); });
    // the README of the ZIP, built as ftaaZip builds it when the report lane's zip is loaded
    await add('ftaa', `ftaaReadme ${k}`, 'ftaaReadme', F, sets, P => {
      const list = Object.keys(X.ftaaFiles(P).files), prepared = JSON.parse(X.ftaaBundle(P)).prepared;
      return [`Spare Construct Fast-track package (${P.name || 'project'}), prepared ${prepared}.`, `Version stamp: ${X.stamp(P)}`, '', `D1. ${X.disclaimer('D1', P)}`, `D3. ${X.disclaimer('D3', P)}`,
        `D12. ${X.disclaimer('D12', P, 'a Fast-track application')}`, '', 'Manifest:', ...list.map(n => `  ${n}`)].join('\r\n');
    });
  }
  for (const k of ['plain', 'consent', 'nor', 'notFtaa', 'matrix', 'mitigation', 'triggers', 'precinct', 'parking', 'sensitivity', 'shNear', 'dates', 'allIn', 'unfunded', 'notRun']) {
    const sets = pkg[k];
    await add('ftaa', `report ftaa ${k}`, 'report-ftaa', F, sets, P => reportOf(c, 'compliance-extra-ftaa', P, undefined));
    await add('ftaa', `report ftaa ${k} with context`, 'report-ftaa-ctx', F, sets, P => reportOf(c, 'compliance-extra-ftaa', P, rctx));
  }
  await add('ftaa', 'export ftaa enabled', 'export-ftaa-enabled', F, pkg.plain, () => exportEnabled(c, 'ftaa'));
  await add('ftaa', 'export ftaa extension', 'export-ftaa-ext', F, pkg.plain, P => ({ ext: c.ComplianceExtra.__internal.EXPORTS.find(e => e[0] === 'ftaa')[2], text: exportBuild(c, 'ftaa', P).slice(0, 60) }));
}
